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
/// At 36 TPS this is 10 seconds.
pub const SAVE_INTERVAL_TICKS: u32 = 360;

/// Number of save cycles in a full rotation.
pub const SAVE_CYCLE_COUNT: u32 = 6;

/// A unit of work sent to the background saver thread.
pub enum SaveJob {
    Characters(Vec<core::types::Character>),
    /// Items with the starting index for the slice.
    Items(Vec<core::types::Item>, usize),
    /// Map tiles with the starting linear index for the slice.
    MapTiles(Vec<core::types::Map>, usize),
    /// Small/combined: effects, globals, character templates, item templates.
    SmallData {
        effects: Vec<core::types::Effect>,
        globals: core::types::Global,
        character_templates: Vec<core::types::Character>,
        item_templates: Vec<core::types::Item>,
    },
    /// Request a synchronous flush — the saver thread will ack via the
    /// provided one-shot channel once the write completes.
    #[allow(dead_code)]
    Flush(mpsc::Sender<Result<(), String>>),
    /// Shut down the background thread cleanly.
    Shutdown,
}

/// Handle returned by [`spawn`] — stores the channel sender and the thread
/// join handle so the owner can enqueue jobs and join on shutdown.
pub struct BackgroundSaver {
    tx: mpsc::Sender<SaveJob>,
    handle: Option<JoinHandle<()>>,
}

impl BackgroundSaver {
    /// Send a save job to the background thread.
    pub fn send(&self, job: SaveJob) {
        if let Err(e) = self.tx.send(job) {
            log::error!("Failed to send save job to background saver: {e}");
        }
    }

    /// Request a synchronous flush: blocks the caller until the background
    /// thread has completed its current queue.
    #[allow(dead_code)]
    pub fn flush(&self) -> Result<(), String> {
        let (ack_tx, ack_rx) = mpsc::channel();
        self.send(SaveJob::Flush(ack_tx));
        ack_rx
            .recv()
            .map_err(|_| "Background saver flush: channel closed".to_string())?
    }

    /// Signal the background thread to stop and wait for it to finish.
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

/// Spawn the background saver thread.  Returns a [`BackgroundSaver`] handle.
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
