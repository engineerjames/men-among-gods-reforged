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
/// | 3     | Effects + Globals                                          |
/// | 4     | Map first half (linear 0 .. total/2)                      |
/// | 5     | Map second half (linear total/2 .. total)                 |
///
/// At default settings (`SAVE_INTERVAL_TICKS = 4_320`, 36 TPS) each cycle
/// fires every ~2 minutes, so a full rotation ≈ 12 minutes.
use std::sync::mpsc;
use std::thread::{self, JoinHandle};

use super::{connection, store};

/// Ticks between each background save job.
///
/// At the server's target rate of 36 TPS this corresponds to
/// approximately 2 minutes between save cycles.
pub const SAVE_INTERVAL_TICKS: u32 = 4_320;

/// Number of save cycles in a full rotation.
///
/// A full rotation visits every data type once (characters, items
/// first/second half, small data, map first/second half).  At default
/// settings the full rotation takes approximately 12 minutes.
pub const SAVE_CYCLE_COUNT: u32 = 6;

/// A unit of work sent to the background saver thread via
/// [`BackgroundSaver::send`].
///
/// Each variant carries the cloned data needed for one write operation
/// so the game loop can hand off ownership and continue immediately.
#[allow(clippy::large_enum_variant)]
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
    /// Persist the smaller/combined mutable data sets in one batch:
    /// effects and globals.
    SmallData {
        /// All effect slots (`game:effect:*`).
        effects: Vec<core::types::Effect>,
        /// The single global state value (`game:global`).
        globals: core::types::Global,
    },
    /// Persist a complete runtime snapshot and report completion asynchronously.
    RuntimeData {
        /// All map tiles.
        map: Vec<core::types::Map>,
        /// All runtime items.
        items: Vec<core::types::Item>,
        /// All runtime characters.
        characters: Vec<core::types::Character>,
        /// All effects.
        effects: Vec<core::types::Effect>,
        /// Global runtime state.
        globals: core::types::Global,
        /// Completion channel carrying the request id, message, and result.
        completion: mpsc::Sender<SaveCompletion>,
        /// Admin request identifier.
        request_id: String,
        /// Outcome message to publish after the save succeeds.
        message: String,
    },
    /// Request a synchronous flush — the saver thread will ack via the
    /// provided one-shot channel once the write completes.
    Flush(mpsc::Sender<Result<(), String>>),
    /// Shut down the background thread cleanly.
    Shutdown,
}

/// Completion payload for an asynchronously persisted full snapshot.
pub struct SaveCompletion {
    /// Admin request identifier associated with the snapshot.
    pub request_id: String,
    /// Outcome message produced by the tick-thread mutation.
    pub message: String,
    /// Result of the KeyDB write.
    pub result: Result<(), String>,
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

    /// Enqueue a save job and report whether the saver accepted it.
    pub fn send_checked(&self, job: SaveJob) -> Result<(), String> {
        self.tx
            .send(job)
            .map_err(|_| "background saver channel closed".to_owned())
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
    pub fn flush(&self) -> Result<(), String> {
        let (ack_tx, ack_rx) = mpsc::channel();
        self.send(SaveJob::Flush(ack_tx));
        ack_rx
            .recv()
            .map_err(|_| "Background saver flush: channel closed".to_owned())?
    }

    /// Signal the background thread to stop and block until it exits.
    ///
    /// Safe to call multiple times — subsequent calls are no-ops after
    /// the join handle has been consumed.  Also called automatically by
    /// the [`Drop`] implementation.
    pub fn shutdown(&mut self) {
        let _ = self.tx.send(SaveJob::Shutdown);
        if let Some(handle) = self.handle.take()
            && let Err(e) = handle.join()
        {
            log::error!("Background saver thread panicked: {e:?}");
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

/// Make a bounded number of KeyDB connection attempts for one save job.
fn connect_for_job() -> Option<redis::Connection> {
    for attempt in 1..=3 {
        match connection::connect() {
            Ok(con) => return Some(con),
            Err(error) => {
                log::error!("Background saver: KeyDB connect attempt {attempt}/3 failed: {error}");
                if attempt < 3 {
                    thread::sleep(std::time::Duration::from_millis(100));
                }
            }
        }
    }
    None
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
    let mut con: Option<redis::Connection> = None;

    loop {
        let job = match rx.recv() {
            Ok(job) => job,
            Err(_) => {
                log::info!("Background saver: channel closed, shutting down.");
                break;
            }
        };

        if !matches!(&job, SaveJob::Flush(_) | SaveJob::Shutdown) && con.is_none() {
            con = connect_for_job();
        }

        match job {
            SaveJob::Characters(data) => {
                let t = std::time::Instant::now();
                if let Some(connection) = con.as_mut() {
                    if let Err(e) = store::save_characters(connection, &data) {
                        log::error!("Background save characters failed: {e}");
                        con = None;
                    } else {
                        log::debug!(
                            "Background save: {} characters in {:.2?}",
                            data.len(),
                            t.elapsed()
                        );
                    }
                } else {
                    log::warn!("Background save characters skipped: KeyDB unavailable");
                }
            }
            SaveJob::Items(data, start_idx) => {
                let t = std::time::Instant::now();
                if let Some(connection) = con.as_mut() {
                    if let Err(e) = store::save_indexed_entities_range(
                        connection,
                        "game:item:",
                        &data,
                        start_idx,
                    ) {
                        log::error!("Background save items failed: {e}");
                        con = None;
                    } else {
                        log::debug!(
                            "Background save: {} items (start {start_idx}) in {:.2?}",
                            data.len(),
                            t.elapsed()
                        );
                    }
                } else {
                    log::warn!("Background save items skipped: KeyDB unavailable");
                }
            }
            SaveJob::MapTiles(data, start_linear) => {
                let t = std::time::Instant::now();
                if let Some(connection) = con.as_mut() {
                    if let Err(e) = store::save_map_range(connection, &data, start_linear) {
                        log::error!("Background save map tiles failed: {e}");
                        con = None;
                    } else {
                        log::debug!(
                            "Background save: {} map tiles (start {start_linear}) in {:.2?}",
                            data.len(),
                            t.elapsed()
                        );
                    }
                } else {
                    log::warn!("Background save map tiles skipped: KeyDB unavailable");
                }
            }
            SaveJob::SmallData { effects, globals } => {
                let t = std::time::Instant::now();
                if let Some(connection) = con.as_mut() {
                    let effects_result = store::save_effects(connection, &effects);
                    let globals_result = store::save_globals(connection, &globals);
                    if let Err(error) = effects_result {
                        log::error!("Background save effects failed: {error}");
                        con = None;
                    } else if let Err(error) = globals_result {
                        log::error!("Background save globals failed: {error}");
                        con = None;
                    } else {
                        log::debug!("Background save: small data in {:.2?}", t.elapsed());
                    }
                } else {
                    log::warn!("Background save small data skipped: KeyDB unavailable");
                }
            }
            SaveJob::RuntimeData {
                map,
                items,
                characters,
                effects,
                globals,
                completion,
                request_id,
                message,
            } => {
                let result = if let Some(connection) = con.as_mut() {
                    store::save_runtime_data(
                        connection,
                        &map,
                        &items,
                        &characters,
                        &effects,
                        &globals,
                    )
                } else {
                    Err("KeyDB unavailable for full runtime save".to_owned())
                };
                if let Err(error) = &result {
                    log::error!("Background full runtime save failed: {error}");
                    con = None;
                }
                let _ = completion.send(SaveCompletion {
                    request_id,
                    message,
                    result,
                });
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

    /// Verify the save interval constant matches the 2-minute design target
    /// at 36 TPS.
    #[test]
    fn save_interval_matches_design() {
        assert_eq!(SAVE_INTERVAL_TICKS, 4_320);
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

    /// `SaveJob::SmallData` can bundle effects and globals.
    #[test]
    fn save_job_small_data_construction() {
        let _job = SaveJob::SmallData {
            effects: vec![core::types::Effect::default()],
            globals: core::types::Global::default(),
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
