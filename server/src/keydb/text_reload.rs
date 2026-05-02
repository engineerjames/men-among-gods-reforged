//! Background watcher that polls KeyDB for admin-issued text reload requests.
//!
//! The API writes a JSON-shaped payload to
//! [`core::text_store::TEXT_RELOAD_REQUEST_KEY`]. This watcher polls that key
//! once per second, drains it via `GETDEL`, parses the request id and requested
//! text kinds, and forwards a [`TextReloadRequest`] to the tick thread. The
//! tick loop reloads the requested text data into `GameState` and writes an
//! applied status under [`core::text_store::text_reload_status_key`].

use core::text_store::{self, TEXT_RELOAD_REQUEST_KEY};
use redis::Commands;
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::thread::{self, JoinHandle};
use std::time::Duration;

/// Environment variable that disables admin reload watchers when set to
/// `"true"`/`"1"` (case-insensitive).
pub const DISABLE_ENV: &str = "MAG_ADMIN_RELOAD_DISABLED";

/// Polling interval for the text reload-request key.
const POLL_INTERVAL: Duration = Duration::from_secs(1);

/// Status string written when a request has been enqueued but not applied.
pub const STATUS_PENDING: &str = "pending";

/// Status string written after the tick loop reloads text data.
pub const STATUS_APPLIED: &str = "applied";

/// Per-status TTL so stale status keys self-prune.
const STATUS_TTL_SECS: u64 = 300;

/// Drained text reload request handed to the tick loop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextReloadRequest {
    /// Opaque identifier the API generated and returned to the caller.
    pub request_id: String,
    /// Whether the badwords list should be reloaded.
    pub reload_badwords: bool,
}

/// Handle for the text reload watcher thread.
pub struct TextReloadWatcher {
    rx: Receiver<TextReloadRequest>,
    shutdown: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl TextReloadWatcher {
    /// Spawn the watcher thread.
    ///
    /// Returns `None` when disabled via [`DISABLE_ENV`] or when thread startup
    /// fails. KeyDB connection failures are retried inside the thread.
    ///
    /// # Returns
    ///
    /// * `Some(TextReloadWatcher)` on success.
    /// * `None` when disabled or unable to start.
    pub fn spawn() -> Option<Self> {
        if std::env::var(DISABLE_ENV)
            .map(|value| matches!(value.to_lowercase().as_str(), "1" | "true" | "yes"))
            .unwrap_or(false)
        {
            log::info!("Text reload watcher disabled via {} env var", DISABLE_ENV);
            return None;
        }

        let (tx, rx) = mpsc::channel();
        let shutdown = Arc::new(AtomicBool::new(false));
        let shutdown_thread = Arc::clone(&shutdown);

        let handle = thread::Builder::new()
            .name("text-reload-watcher".into())
            .spawn(move || watcher_loop(tx, shutdown_thread))
            .ok()?;

        log::info!("Text reload watcher started");
        Some(Self {
            rx,
            shutdown,
            handle: Some(handle),
        })
    }

    /// Try to receive the next pending text reload request without blocking.
    ///
    /// # Returns
    ///
    /// * `Some(TextReloadRequest)` when a request was queued.
    /// * `None` when the queue is empty or the watcher has shut down.
    pub fn try_recv(&self) -> Option<TextReloadRequest> {
        match self.rx.try_recv() {
            Ok(request) => Some(request),
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

impl Drop for TextReloadWatcher {
    fn drop(&mut self) {
        if self.handle.is_some() {
            self.shutdown();
        }
    }
}

fn watcher_loop(tx: Sender<TextReloadRequest>, shutdown: Arc<AtomicBool>) {
    let mut con: Option<redis::Connection> = None;

    while !shutdown.load(Ordering::SeqCst) {
        thread::sleep(POLL_INTERVAL);
        if shutdown.load(Ordering::SeqCst) {
            break;
        }

        if con.is_none() {
            match super::connection::connect() {
                Ok(connection) => con = Some(connection),
                Err(error) => {
                    log::warn!("text reload watcher: keydb connect failed: {}", error);
                    continue;
                }
            }
        }

        let connection = con.as_mut().expect("connection just initialised");
        let payload: Option<String> = match redis::cmd("GETDEL")
            .arg(TEXT_RELOAD_REQUEST_KEY)
            .query::<Option<String>>(connection)
        {
            Ok(value) => value,
            Err(error) => {
                log::warn!("text reload watcher: GETDEL failed: {}", error);
                con = None;
                continue;
            }
        };

        let Some(payload) = payload else {
            continue;
        };

        match parse_reload_payload(&payload) {
            Some(request) => {
                let status_key = text_store::text_reload_status_key(&request.request_id);
                if let Err(error) =
                    connection.set_ex::<_, _, ()>(&status_key, STATUS_PENDING, STATUS_TTL_SECS)
                {
                    log::warn!("text reload watcher: status SET failed: {}", error);
                }
                if tx.send(request).is_err() {
                    log::info!("text reload watcher: receiver dropped, exiting");
                    return;
                }
            }
            None => {
                log::warn!("text reload watcher: unparseable payload: {:?}", payload);
            }
        }
    }
}

/// Parse the simple JSON payload the API writes for text reload requests.
///
/// # Arguments
///
/// * `raw` - Payload string read from KeyDB.
///
/// # Returns
///
/// * `Some(TextReloadRequest)` when a non-empty request id and at least one
///   recognised kind are present.
/// * `None` otherwise.
fn parse_reload_payload(raw: &str) -> Option<TextReloadRequest> {
    let request_id = extract_string_field(raw, "request_id")?;
    if request_id.is_empty() {
        return None;
    }

    let reload_badwords = raw.contains("\"badwords\"");
    if !reload_badwords {
        return None;
    }

    Some(TextReloadRequest {
        request_id,
        reload_badwords,
    })
}

fn extract_string_field(raw: &str, field: &str) -> Option<String> {
    let needle = format!("\"{}\":\"", field);
    let start = raw.find(&needle)? + needle.len();
    let rest = &raw[start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

/// Write the `applied` status entry for a text reload request.
///
/// # Arguments
///
/// * `con` - Open KeyDB connection.
/// * `request_id` - Identifier of the request that was processed.
///
/// # Returns
///
/// * `Ok(())` on success.
/// * `Err(String)` on KeyDB failure.
pub fn write_applied_status(con: &mut redis::Connection, request_id: &str) -> Result<(), String> {
    let key = text_store::text_reload_status_key(request_id);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    let value = format!("{}:{}", STATUS_APPLIED, now);
    con.set_ex::<_, _, ()>(key, value, STATUS_TTL_SECS)
        .map_err(|error| format!("status SET failed: {}", error))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_payload_extracts_badwords_kind() {
        let raw = r#"{"request_id":"abc","kinds":["badwords"],"requested_at":1}"#;
        let request = parse_reload_payload(raw).expect("parsed");
        assert_eq!(request.request_id, "abc");
        assert!(request.reload_badwords);
    }

    #[test]
    fn parse_payload_rejects_unknown_kinds() {
        let raw = r#"{"request_id":"abc","kinds":["motd"],"requested_at":1}"#;
        assert!(parse_reload_payload(raw).is_none());
    }

    #[test]
    fn parse_payload_rejects_missing_id() {
        let raw = r#"{"kinds":["badwords"],"requested_at":1}"#;
        assert!(parse_reload_payload(raw).is_none());
    }

    #[test]
    fn parse_payload_rejects_empty_id() {
        let raw = r#"{"request_id":"","kinds":["badwords"],"requested_at":1}"#;
        assert!(parse_reload_payload(raw).is_none());
    }

    #[test]
    fn extract_string_field_handles_missing() {
        assert_eq!(extract_string_field("{}", "request_id"), None);
    }
}
