//! Background watcher that polls KeyDB for admin-issued character patches
//! and applies them to the in-memory character table between ticks.
//!
//! The API writes each patch as a bincode [`CharacterPatch`] entry pushed
//! to [`core::character_store::CHARACTER_PATCH_QUEUE_KEY`] (via `RPUSH`).
//! A `POST` to the reload endpoint flushes the queue by writing
//! [`core::character_store::CHARACTER_PATCH_REQUEST_KEY`]. This watcher
//! polls both keys and forwards [`CharacterPatchEvent`]s to the tick
//! thread, which merges only the static fields into `GameState.characters`,
//! preserving the dynamic runtime fields (position, combat AI, current
//! resources, inventory, networking).

use core::character_store::{
    self, CHARACTER_PATCH_QUEUE_KEY, CHARACTER_PATCH_REQUEST_KEY, CharacterPatch,
};
use redis::Commands;
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::thread::{self, JoinHandle};
use std::time::Duration;

/// Environment variable that disables the watcher entirely when set to
/// `"true"`/`"1"` (case-insensitive).
pub const DISABLE_ENV: &str = "MAG_ADMIN_CHAR_PATCH_DISABLED";

/// Polling interval for the patch queue and request key.
const POLL_INTERVAL: Duration = Duration::from_millis(250);

/// Maximum number of queued patches drained per `LPOP` round-trip.
const DRAIN_BATCH: usize = 1024;

/// Status string written while a request is queued but not yet applied.
pub const STATUS_PENDING: &str = "pending";

/// Status string written after the tick loop applies all queued patches.
pub const STATUS_APPLIED: &str = "applied";

/// TTL on status keys so stale entries self-prune.
const STATUS_TTL_SECS: u64 = 300;

/// Event emitted by the watcher to the tick loop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CharacterPatchEvent {
    /// Apply a single patch to the in-memory character table.
    Apply(CharacterPatch),
    /// A reload request was drained. After processing any `Apply` events
    /// that precede this marker in the queue, the tick loop must write the
    /// `applied` status entry for `request_id`.
    ReloadCompleted {
        /// Identifier the API handed back to the caller.
        request_id: String,
    },
}

/// Handle for the watcher thread.
pub struct CharacterPatchWatcher {
    rx: Receiver<CharacterPatchEvent>,
    shutdown: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl CharacterPatchWatcher {
    /// Spawn the watcher thread.
    ///
    /// # Returns
    ///
    /// * `Some(watcher)` on success.
    /// * `None` when disabled via [`DISABLE_ENV`] or when the thread cannot
    ///   be spawned.
    pub fn spawn() -> Option<Self> {
        if std::env::var(DISABLE_ENV)
            .map(|v| matches!(v.to_lowercase().as_str(), "1" | "true" | "yes"))
            .unwrap_or(false)
        {
            log::info!(
                "Character patch watcher disabled via {} env var",
                DISABLE_ENV
            );
            return None;
        }

        let (tx, rx) = mpsc::channel();
        let shutdown = Arc::new(AtomicBool::new(false));
        let shutdown_thread = Arc::clone(&shutdown);

        let handle = thread::Builder::new()
            .name("char-patch-watcher".into())
            .spawn(move || watcher_loop(tx, shutdown_thread))
            .ok()?;

        log::info!("Character patch watcher started");
        Some(Self {
            rx,
            shutdown,
            handle: Some(handle),
        })
    }

    /// Try to receive the next pending event without blocking.
    ///
    /// # Returns
    ///
    /// * `Some(event)` when an event is ready.
    /// * `None` when the queue is empty (or the watcher has shut down).
    pub fn try_recv(&self) -> Option<CharacterPatchEvent> {
        match self.rx.try_recv() {
            Ok(m) => Some(m),
            Err(TryRecvError::Empty) | Err(TryRecvError::Disconnected) => None,
        }
    }

    /// Signal the watcher to stop and join its thread.
    pub fn shutdown(&mut self) {
        self.shutdown.store(true, Ordering::SeqCst);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for CharacterPatchWatcher {
    fn drop(&mut self) {
        if self.handle.is_some() {
            self.shutdown();
        }
    }
}

fn watcher_loop(tx: Sender<CharacterPatchEvent>, shutdown: Arc<AtomicBool>) {
    let mut con: Option<redis::Connection> = None;

    while !shutdown.load(Ordering::SeqCst) {
        thread::sleep(POLL_INTERVAL);
        if shutdown.load(Ordering::SeqCst) {
            break;
        }

        if con.is_none() {
            match super::connection::connect() {
                Ok(c) => con = Some(c),
                Err(e) => {
                    log::warn!("character patch watcher: keydb connect failed: {}", e);
                    continue;
                }
            }
        }

        let conn = con.as_mut().expect("connection just initialised");

        let reload_request_id = match redis::cmd("GETDEL")
            .arg(CHARACTER_PATCH_REQUEST_KEY)
            .query::<Option<String>>(conn)
        {
            Ok(v) => v.and_then(|raw| extract_string_field(&raw, "request_id")),
            Err(e) => {
                log::warn!("character patch watcher: GETDEL failed: {}", e);
                con = None;
                continue;
            }
        };

        if let Some(ref request_id) = reload_request_id {
            let status_key = character_store::character_patch_status_key(request_id);
            if let Err(e) = conn.set_ex::<_, _, ()>(&status_key, STATUS_PENDING, STATUS_TTL_SECS) {
                log::warn!("character patch watcher: status SET failed: {}", e);
            }
        }

        let drain_fully = reload_request_id.is_some();

        loop {
            let batch = match drain_batch(conn) {
                Ok(b) => b,
                Err(e) => {
                    log::warn!("character patch watcher: drain failed: {}", e);
                    con = None;
                    break;
                }
            };
            if batch.is_empty() {
                break;
            }
            for patch in batch {
                if tx.send(CharacterPatchEvent::Apply(patch)).is_err() {
                    log::info!("character patch watcher: receiver dropped, exiting");
                    return;
                }
            }
            if !drain_fully {
                break;
            }
        }

        if let Some(request_id) = reload_request_id {
            if tx
                .send(CharacterPatchEvent::ReloadCompleted { request_id })
                .is_err()
            {
                log::info!("character patch watcher: receiver dropped, exiting");
                return;
            }
        }
    }
}

/// Drain up to [`DRAIN_BATCH`] patches from the KeyDB queue in one round-trip.
///
/// # Arguments
///
/// * `conn` - Live KeyDB connection.
///
/// # Returns
///
/// * `Ok(batch)` — possibly empty when the queue is drained.
/// * `Err(message)` on KeyDB failure; caller forces reconnect.
fn drain_batch(conn: &mut redis::Connection) -> Result<Vec<CharacterPatch>, String> {
    let raw: Vec<Vec<u8>> = redis::cmd("LPOP")
        .arg(CHARACTER_PATCH_QUEUE_KEY)
        .arg(DRAIN_BATCH)
        .query(conn)
        .map_err(|e| format!("LPOP {}: {}", CHARACTER_PATCH_QUEUE_KEY, e))?;

    let mut out = Vec::with_capacity(raw.len());
    for bytes in raw {
        match CharacterPatch::from_bytes(&bytes) {
            Ok(p) => out.push(p),
            Err(e) => log::warn!("character patch watcher: dropping undecodable patch: {}", e),
        }
    }
    Ok(out)
}

/// Extract a JSON string field from the simple payload written by the API.
///
/// # Arguments
///
/// * `raw`   - Payload string read from KeyDB.
/// * `field` - Field name to search for.
///
/// # Returns
///
/// * `Some(value)` when the field is present and non-empty.
/// * `None` otherwise.
fn extract_string_field(raw: &str, field: &str) -> Option<String> {
    let needle = format!("\"{}\":\"", field);
    let start = raw.find(&needle)? + needle.len();
    let rest = &raw[start..];
    let end = rest.find('"')?;
    let value = &rest[..end];
    if value.is_empty() {
        None
    } else {
        Some(value.to_owned())
    }
}

/// Write the `applied` status entry for a reload request.
///
/// # Arguments
///
/// * `con`        - Open KeyDB connection.
/// * `request_id` - Identifier the API handed back to the caller.
///
/// # Returns
///
/// * `Ok(())` on success.
/// * `Err(String)` on KeyDB failure.
pub fn write_applied_status(con: &mut redis::Connection, request_id: &str) -> Result<(), String> {
    let key = character_store::character_patch_status_key(request_id);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let value = format!("{}:{}", STATUS_APPLIED, now);
    con.set_ex::<_, _, ()>(key, value, STATUS_TTL_SECS)
        .map_err(|e| format!("status SET failed: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_string_field_reads_request_id() {
        let raw = r#"{"request_id":"abc123","requested_at":42}"#;
        assert_eq!(
            extract_string_field(raw, "request_id"),
            Some("abc123".into())
        );
    }

    #[test]
    fn extract_string_field_rejects_empty_value() {
        let raw = r#"{"request_id":"","requested_at":0}"#;
        assert_eq!(extract_string_field(raw, "request_id"), None);
    }

    #[test]
    fn extract_string_field_missing() {
        assert_eq!(extract_string_field("{}", "request_id"), None);
    }
}
