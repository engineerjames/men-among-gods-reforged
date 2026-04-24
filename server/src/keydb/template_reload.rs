//! Background watcher that polls KeyDB for admin-issued template-reload
//! requests and applies them in the tick loop.
//!
//! The API writes a JSON-shaped payload to
//! [`core::template_store::RELOAD_REQUEST_KEY`]. This watcher polls that key
//! once per second, drains the value via `GETDEL`, parses the request id and
//! kinds list, and forwards a [`ReloadRequest`] over an `mpsc::channel`. The
//! main tick loop calls [`TemplateReloadWatcher::try_recv`] between ticks to
//! drain pending requests, swap the affected slices in `GameState`, and
//! write a status entry under
//! [`core::template_store::reload_status_key`].

use core::template_store::{self, RELOAD_REQUEST_KEY};
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
pub const DISABLE_ENV: &str = "MAG_ADMIN_RELOAD_DISABLED";

/// Polling interval for the reload-request key.
const POLL_INTERVAL: Duration = Duration::from_secs(1);

/// Status string written when the request has been enqueued but not yet
/// applied by the tick loop.
pub const STATUS_PENDING: &str = "pending";

/// Status string written after the tick loop swaps templates in.
pub const STATUS_APPLIED: &str = "applied";

/// Per-status TTL so stale status keys self-prune.
const STATUS_TTL_SECS: u64 = 300;

/// Drained reload request handed to the tick loop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReloadRequest {
    /// Opaque identifier the API generated and returned to the caller.
    pub request_id: String,
    /// Whether item templates should be reloaded.
    pub reload_items: bool,
    /// Whether character templates should be reloaded.
    pub reload_characters: bool,
}

/// Handle for the watcher thread.
pub struct TemplateReloadWatcher {
    rx: Receiver<ReloadRequest>,
    shutdown: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl TemplateReloadWatcher {
    /// Spawn the watcher thread.
    ///
    /// Returns `None` when the watcher has been disabled via [`DISABLE_ENV`]
    /// or when the initial KeyDB connection cannot be established.
    ///
    /// # Returns
    ///
    /// * `Some(TemplateReloadWatcher)` on success.
    /// * `None` when disabled or unable to connect.
    pub fn spawn() -> Option<Self> {
        if std::env::var(DISABLE_ENV)
            .map(|v| matches!(v.to_lowercase().as_str(), "1" | "true" | "yes"))
            .unwrap_or(false)
        {
            log::info!(
                "Template reload watcher disabled via {} env var",
                DISABLE_ENV
            );
            return None;
        }

        let (tx, rx) = mpsc::channel();
        let shutdown = Arc::new(AtomicBool::new(false));
        let shutdown_thread = Arc::clone(&shutdown);

        let handle = thread::Builder::new()
            .name("template-reload-watcher".into())
            .spawn(move || watcher_loop(tx, shutdown_thread))
            .ok()?;

        log::info!("Template reload watcher started");
        Some(Self {
            rx,
            shutdown,
            handle: Some(handle),
        })
    }

    /// Try to receive the next pending reload request without blocking.
    ///
    /// # Returns
    ///
    /// * `Some(ReloadRequest)` when a request was queued.
    /// * `None` when the queue is empty (or the watcher has shut down).
    pub fn try_recv(&self) -> Option<ReloadRequest> {
        match self.rx.try_recv() {
            Ok(req) => Some(req),
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

impl Drop for TemplateReloadWatcher {
    fn drop(&mut self) {
        if self.handle.is_some() {
            self.shutdown();
        }
    }
}

fn watcher_loop(tx: Sender<ReloadRequest>, shutdown: Arc<AtomicBool>) {
    // Connect lazily so the watcher survives a transient KeyDB outage.
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
                    log::warn!("template reload watcher: keydb connect failed: {}", e);
                    continue;
                }
            }
        }

        let conn = con.as_mut().expect("connection just initialised");
        let payload: Option<String> = match redis::cmd("GETDEL")
            .arg(RELOAD_REQUEST_KEY)
            .query::<Option<String>>(conn)
        {
            Ok(v) => v,
            Err(e) => {
                log::warn!("template reload watcher: GETDEL failed: {}", e);
                con = None; // Force reconnect on next iteration.
                continue;
            }
        };

        let Some(payload) = payload else {
            continue;
        };

        match parse_reload_payload(&payload) {
            Some(req) => {
                let status_key = template_store::reload_status_key(&req.request_id);
                if let Err(e) =
                    conn.set_ex::<_, _, ()>(&status_key, STATUS_PENDING, STATUS_TTL_SECS)
                {
                    log::warn!("template reload watcher: status SET failed: {}", e);
                }
                if tx.send(req).is_err() {
                    log::info!("template reload watcher: receiver dropped, exiting");
                    return;
                }
            }
            None => {
                log::warn!(
                    "template reload watcher: unparseable payload: {:?}",
                    payload
                );
            }
        }
    }
}

/// Parse the simple JSON payload the API writes for reload requests.
///
/// The payload always has the shape:
/// `{"request_id":"...","kinds":["items","characters"],"requested_at":...}`.
/// Rather than depend on a JSON parser, we extract the fields we care about
/// via substring matching, which is sufficient for this internal protocol.
///
/// # Arguments
///
/// * `raw` - Payload string read from KeyDB.
///
/// # Returns
///
/// * `Some(ReloadRequest)` when a `request_id` is present and `kinds`
///   contains at least one recognised value.
/// * `None` otherwise.
fn parse_reload_payload(raw: &str) -> Option<ReloadRequest> {
    let request_id = extract_string_field(raw, "request_id")?;
    if request_id.is_empty() {
        return None;
    }

    let reload_items = raw.contains("\"items\"");
    let reload_characters = raw.contains("\"characters\"");
    if !reload_items && !reload_characters {
        return None;
    }

    Some(ReloadRequest {
        request_id,
        reload_items,
        reload_characters,
    })
}

fn extract_string_field(raw: &str, field: &str) -> Option<String> {
    let needle = format!("\"{}\":\"", field);
    let start = raw.find(&needle)? + needle.len();
    let rest = &raw[start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

/// Write the `applied` status entry for a reload request.
///
/// Called by the tick loop after templates have been swapped in.
///
/// # Arguments
///
/// * `con`         - Open KeyDB connection.
/// * `request_id`  - Identifier of the request that was processed.
///
/// # Returns
///
/// * `Ok(())` on success.
/// * `Err(String)` on KeyDB failure (caller should log and continue).
pub fn write_applied_status(con: &mut redis::Connection, request_id: &str) -> Result<(), String> {
    let key = template_store::reload_status_key(request_id);
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
    fn parse_payload_extracts_id_and_both_kinds() {
        let raw = r#"{"request_id":"abc","kinds":["items","characters"],"requested_at":1}"#;
        let req = parse_reload_payload(raw).expect("parsed");
        assert_eq!(req.request_id, "abc");
        assert!(req.reload_items);
        assert!(req.reload_characters);
    }

    #[test]
    fn parse_payload_extracts_items_only() {
        let raw = r#"{"request_id":"x","kinds":["items"],"requested_at":1}"#;
        let req = parse_reload_payload(raw).expect("parsed");
        assert!(req.reload_items);
        assert!(!req.reload_characters);
    }

    #[test]
    fn parse_payload_extracts_characters_only() {
        let raw = r#"{"request_id":"y","kinds":["characters"],"requested_at":1}"#;
        let req = parse_reload_payload(raw).expect("parsed");
        assert!(!req.reload_items);
        assert!(req.reload_characters);
    }

    #[test]
    fn parse_payload_rejects_unknown_kinds() {
        let raw = r#"{"request_id":"y","kinds":["maps"],"requested_at":1}"#;
        assert!(parse_reload_payload(raw).is_none());
    }

    #[test]
    fn parse_payload_rejects_missing_id() {
        let raw = r#"{"kinds":["items"],"requested_at":1}"#;
        assert!(parse_reload_payload(raw).is_none());
    }

    #[test]
    fn parse_payload_rejects_empty_id() {
        let raw = r#"{"request_id":"","kinds":["items"],"requested_at":1}"#;
        assert!(parse_reload_payload(raw).is_none());
    }

    #[test]
    fn extract_string_field_handles_missing() {
        assert_eq!(extract_string_field("{}", "request_id"), None);
    }

    #[test]
    fn extract_string_field_handles_unterminated() {
        assert_eq!(
            extract_string_field(r#"{"request_id":"abc"#, "request_id"),
            None
        );
    }
}
