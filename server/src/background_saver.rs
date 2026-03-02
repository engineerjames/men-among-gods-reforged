/// Background persistence thread for writing game data to KeyDB.
///
/// The main game loop (single-threaded) periodically clones slices of
/// in-memory data and sends them to this background thread via an `mpsc`
/// channel.  The background thread owns a persistent `redis::Connection`
/// and writes the data using pipelined commands.
///
/// # Save rotation
///
/// A full rotation saves all data types over multiple intervals:
///
/// | Cycle | Data                                                      |
/// |-------|-----------------------------------------------------------|
/// | 0     | Characters (all 8,192)                                    |
/// | 1     | Items first half (0 .. MAXITEM/2)                         |
/// | 2     | Items second half (MAXITEM/2 .. MAXITEM)                  |
/// | 3     | Effects + Globals + Character templates + Item templates   |
/// | 4     | Map first half (linear 0 .. total/2)                      |
/// | 5     | Map second half (linear total/2 .. total)                 |
///
/// At default settings (`SAVE_INTERVAL_TICKS = 360`, 36 TPS) each cycle
/// fires every ~10 seconds, so a full rotation ≈ 60 seconds.
use std::sync::mpsc;
use std::thread::{self, JoinHandle};

use crate::keydb;
use crate::keydb_store;

/// Ticks between each background save job.
///
/// At the server's target rate of 36 TPS this corresponds to
/// approximately 10 seconds between save cycles.
pub const SAVE_INTERVAL_TICKS: u32 = 360;

/// Number of save cycles in a full rotation.
///
/// A full rotation visits every data type once (characters, items
/// first/second half, small data, map first/second half).  At default
/// settings the full rotation takes approximately 60 seconds.
pub const SAVE_CYCLE_COUNT: u32 = 6;

/// A unit of work sent to the background saver thread via
/// [`BackgroundSaver::send`].
///
/// Each variant carries the cloned data needed for one write operation
/// so the game loop can hand off ownership and continue immediately.
pub enum SaveJob {
    /// Persist all character slots (`game:char:*`).
    Characters(Vec<core::types::Character>),
    /// Persist a sub-range of item slots (`game:item:*`).
    ///
    /// The `usize` is the absolute starting index used in the key.
    Items(Vec<core::types::Item>, usize),
    /// Persist a sub-range of map tiles (`game:map:*`).
    ///
    /// The `usize` is the absolute starting linear index.
    MapTiles(Vec<core::types::Map>, usize),
    /// Persist the smaller/combined data sets in one batch:
    /// effects, globals, character templates, and item templates.
    SmallData {
        /// All effect slots (`game:effect:*`).
        effects: Vec<core::types::Effect>,
        /// The single global state value (`game:global`).
        globals: core::types::Global,
        /// All character templates (`game:tchar:*`).
        character_templates: Vec<core::types::Character>,
        /// All item templates (`game:titem:*`).
        item_templates: Vec<core::types::Item>,
    },
    /// Request a synchronous flush — the saver thread will ack via the
    /// provided one-shot channel once the write completes.
    #[allow(dead_code)]
    Flush(mpsc::Sender<Result<(), String>>),
    /// Shut down the background thread cleanly.
    Shutdown,
}

/// Handle for the background saver thread.
///
/// Returned by [`spawn`].  Stores the `mpsc` sender and the thread join
/// handle so the owner can enqueue [`SaveJob`]s and join on shutdown.
pub struct BackgroundSaver {
    tx: mpsc::Sender<SaveJob>,
    handle: Option<JoinHandle<()>>,
}

impl BackgroundSaver {
    /// Enqueue a save job on the background thread.
    ///
    /// This call is non-blocking — the data is sent through the `mpsc`
    /// channel and processed asynchronously.
    ///
    /// # Arguments
    ///
    /// * `job` - The [`SaveJob`] to send.
    pub fn send(&self, job: SaveJob) {
        if let Err(e) = self.tx.send(job) {
            log::error!("Failed to send save job to background saver: {e}");
        }
    }

    /// Request a synchronous flush: blocks the caller until the
    /// background thread has drained its entire job queue.
    ///
    /// Primarily useful in tests and clean-shutdown paths where you
    /// need to guarantee all queued writes have completed.
    ///
    /// # Returns
    ///
    /// * `Ok(())` once the flush is acknowledged.
    /// * `Err` if the background thread has already exited.
    #[allow(dead_code)]
    pub fn flush(&self) -> Result<(), String> {
        let (ack_tx, ack_rx) = mpsc::channel();
        self.send(SaveJob::Flush(ack_tx));
        ack_rx
            .recv()
            .map_err(|_| "Background saver flush: channel closed".to_string())?
    }

    /// Signal the background thread to stop and block until it exits.
    ///
    /// Safe to call multiple times — subsequent calls are no-ops after
    /// the join handle has been consumed.  Also called automatically by
    /// the [`Drop`] implementation.
    pub fn shutdown(&mut self) {
        let _ = self.tx.send(SaveJob::Shutdown);
        if let Some(handle) = self.handle.take() {
            if let Err(e) = handle.join() {
                log::error!("Background saver thread panicked: {e:?}");
            }
        }
    }
}

impl Drop for BackgroundSaver {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// Spawn the background saver thread.
///
/// Creates an `mpsc` channel and starts a dedicated thread that
/// listens for [`SaveJob`] messages.  The thread maintains its own
/// [`redis::Connection`] and reconnects automatically on failure.
///
/// # Returns
///
/// * A [`BackgroundSaver`] handle for sending jobs and shutting down.
///
/// # Panics
///
/// Panics if the OS thread cannot be spawned.
pub fn spawn() -> BackgroundSaver {
    let (tx, rx) = mpsc::channel::<SaveJob>();

    let handle = thread::Builder::new()
        .name("bg-saver".into())
        .spawn(move || {
            saver_thread_main(rx);
        })
        .expect("Failed to spawn background saver thread");

    BackgroundSaver {
        tx,
        handle: Some(handle),
    }
}

// ---------------------------------------------------------------------------
//  Background thread main loop
// ---------------------------------------------------------------------------

/// Establish a KeyDB connection, retrying every 5 seconds on failure.
///
/// # Returns
///
/// * A live [`redis::Connection`].  This function never returns `Err`;
///   it loops until a connection succeeds.
fn connect_with_retry() -> redis::Connection {
    loop {
        match keydb::connect() {
            Ok(con) => return con,
            Err(e) => {
                log::error!("Background saver: KeyDB connect failed ({e}), retrying in 5s...");
                thread::sleep(std::time::Duration::from_secs(5));
            }
        }
    }
}

/// Entry point for the background saver thread.
///
/// Blocks on the `mpsc` receiver, processing [`SaveJob`] messages in
/// FIFO order until a [`SaveJob::Shutdown`] is received or the channel
/// is closed.
///
/// # Arguments
///
/// * `rx` - The receiving end of the job channel.
fn saver_thread_main(rx: mpsc::Receiver<SaveJob>) {
    log::info!("Background saver thread started.");
    let mut con = connect_with_retry();

    loop {
        let job = match rx.recv() {
            Ok(job) => job,
            Err(_) => {
                log::info!("Background saver: channel closed, shutting down.");
                break;
            }
        };

        match job {
            SaveJob::Characters(data) => {
                let t = std::time::Instant::now();
                if let Err(e) = keydb_store::save_characters(&mut con, &data) {
                    log::error!("Background save characters failed: {e}");
                    con = connect_with_retry();
                } else {
                    log::debug!(
                        "Background save: {} characters in {:.2?}",
                        data.len(),
                        t.elapsed()
                    );
                }
            }
            SaveJob::Items(data, start_idx) => {
                let t = std::time::Instant::now();
                if let Err(e) = keydb_store::save_indexed_entities_range(
                    &mut con,
                    "game:item:",
                    &data,
                    start_idx,
                ) {
                    log::error!("Background save items failed: {e}");
                    con = connect_with_retry();
                } else {
                    log::debug!(
                        "Background save: {} items (start {start_idx}) in {:.2?}",
                        data.len(),
                        t.elapsed()
                    );
                }
            }
            SaveJob::MapTiles(data, start_linear) => {
                let t = std::time::Instant::now();
                if let Err(e) = keydb_store::save_map_range(&mut con, &data, start_linear) {
                    log::error!("Background save map tiles failed: {e}");
                    con = connect_with_retry();
                } else {
                    log::debug!(
                        "Background save: {} map tiles (start {start_linear}) in {:.2?}",
                        data.len(),
                        t.elapsed()
                    );
                }
            }
            SaveJob::SmallData {
                effects,
                globals,
                character_templates,
                item_templates,
            } => {
                let t = std::time::Instant::now();
                let mut ok = true;
                if let Err(e) = keydb_store::save_effects(&mut con, &effects) {
                    log::error!("Background save effects failed: {e}");
                    ok = false;
                }
                if let Err(e) = keydb_store::save_globals(&mut con, &globals) {
                    log::error!("Background save globals failed: {e}");
                    ok = false;
                }
                if let Err(e) =
                    keydb_store::save_character_templates(&mut con, &character_templates)
                {
                    log::error!("Background save char templates failed: {e}");
                    ok = false;
                }
                if let Err(e) = keydb_store::save_item_templates(&mut con, &item_templates) {
                    log::error!("Background save item templates failed: {e}");
                    ok = false;
                }
                if !ok {
                    con = connect_with_retry();
                } else {
                    log::debug!("Background save: small data in {:.2?}", t.elapsed());
                }
            }
            SaveJob::Flush(ack) => {
                // All prior jobs have already been processed (channel is FIFO).
                let _ = ack.send(Ok(()));
            }
            SaveJob::Shutdown => {
                log::info!("Background saver: shutdown requested.");
                break;
            }
        }
    }

    log::info!("Background saver thread exiting.");
}

// ---------------------------------------------------------------------------
//  Unit Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Verify the save interval constant matches the 10-second design target
    /// at 36 TPS.
    #[test]
    fn save_interval_matches_design() {
        assert_eq!(SAVE_INTERVAL_TICKS, 360);
    }

    /// Verify the rotation has exactly 6 cycles (characters, items×2,
    /// small data, map×2).
    #[test]
    fn save_cycle_count_is_six() {
        assert_eq!(SAVE_CYCLE_COUNT, 6);
    }

    /// `SaveJob::Characters` can be constructed with an empty vec.
    #[test]
    fn save_job_characters_empty() {
        let _job = SaveJob::Characters(vec![]);
    }

    /// `SaveJob::Items` carries both the data and the start index.
    #[test]
    fn save_job_items_with_offset() {
        let _job = SaveJob::Items(vec![core::types::Item::default()], 42);
    }

    /// `SaveJob::MapTiles` carries both the data and the start linear index.
    #[test]
    fn save_job_map_tiles_with_offset() {
        let _job = SaveJob::MapTiles(vec![core::types::Map::default()], 100);
    }

    /// `SaveJob::SmallData` can bundle all four data types.
    #[test]
    fn save_job_small_data_construction() {
        let _job = SaveJob::SmallData {
            effects: vec![core::types::Effect::default()],
            globals: core::types::Global::default(),
            character_templates: vec![core::types::Character::default()],
            item_templates: vec![core::types::Item::default()],
        };
    }

    /// Dropping a `BackgroundSaver` before calling `shutdown()` should not
    /// panic — the `Drop` impl calls `shutdown()` internally.
    ///
    /// Note: this test relies on the saver thread attempting to connect to
    /// KeyDB.  We bypass `spawn()` and manually wire up the channel so the
    /// thread exits immediately on `Shutdown` without needing a connection.
    #[test]
    fn drop_without_explicit_shutdown_does_not_panic() {
        let (tx, rx) = mpsc::channel::<SaveJob>();

        let handle = std::thread::Builder::new()
            .name("test-bg-saver".into())
            .spawn(move || {
                // Minimal loop: just wait for shutdown
                while let Ok(job) = rx.recv() {
                    if matches!(job, SaveJob::Shutdown) {
                        break;
                    }
                }
            })
            .unwrap();

        let saver = BackgroundSaver {
            tx,
            handle: Some(handle),
        };

        // Dropping without calling shutdown() — must not panic
        drop(saver);
    }

    /// Calling `shutdown()` twice should not panic.
    #[test]
    fn double_shutdown_does_not_panic() {
        let (tx, rx) = mpsc::channel::<SaveJob>();

        let handle = std::thread::Builder::new()
            .name("test-bg-saver".into())
            .spawn(move || {
                while let Ok(job) = rx.recv() {
                    if matches!(job, SaveJob::Shutdown) {
                        break;
                    }
                }
            })
            .unwrap();

        let mut saver = BackgroundSaver {
            tx,
            handle: Some(handle),
        };

        saver.shutdown();
        saver.shutdown(); // second call is a no-op
    }
}
