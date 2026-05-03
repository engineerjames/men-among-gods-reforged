//! Background watcher that polls KeyDB for admin-issued world actions.
//!
//! The API writes bincode [`core::world_action_store::WorldActionRequest`]
//! entries to [`core::world_action_store::WORLD_ACTION_QUEUE_KEY`]. This
//! watcher drains those entries and forwards them to the server tick thread,
//! which owns all mutation of `GameState`.

use core::world_action_store::{
    STATUS_APPLIED, STATUS_FAILED, STATUS_PENDING, STATUS_RUNNING, WORLD_ACTION_QUEUE_KEY,
    WORLD_ACTION_STATUS_TTL_SECS, WorldActionRequest, world_action_status_key,
};
use redis::Commands;
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::thread::{self, JoinHandle};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Environment variable that disables the watcher entirely when set to
/// `"true"`/`"1"`/`"yes"` (case-insensitive).
pub const DISABLE_ENV: &str = "MAG_ADMIN_WORLD_ACTION_DISABLED";

/// Polling interval for the action queue.
const POLL_INTERVAL: Duration = Duration::from_millis(250);

/// Maximum number of queued actions drained per `LPOP` round-trip.
const DRAIN_BATCH: usize = 64;

/// Handle for the watcher thread.
pub struct WorldActionWatcher {
    rx: Receiver<WorldActionRequest>,
    shutdown: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl WorldActionWatcher {
    /// Spawn the watcher thread.
    ///
    /// # Returns
    ///
    /// * `Some(watcher)` on success.
    /// * `None` when disabled via [`DISABLE_ENV`] or when spawning fails.
    pub fn spawn() -> Option<Self> {
        if std::env::var(DISABLE_ENV)
            .map(|value| matches!(value.to_lowercase().as_str(), "1" | "true" | "yes"))
            .unwrap_or(false)
        {
            log::info!("World action watcher disabled via {} env var", DISABLE_ENV);
            return None;
        }

        let (tx, rx) = mpsc::channel();
        let shutdown = Arc::new(AtomicBool::new(false));
        let shutdown_thread = Arc::clone(&shutdown);

        let handle = thread::Builder::new()
            .name("world-action-watcher".into())
            .spawn(move || watcher_loop(tx, shutdown_thread))
            .ok()?;

        log::info!("World action watcher started");
        Some(Self {
            rx,
            shutdown,
            handle: Some(handle),
        })
    }

    /// Try to receive the next pending action without blocking.
    ///
    /// # Returns
    ///
    /// * `Some(request)` when an action is ready.
    /// * `None` when the queue is empty or the watcher has shut down.
    pub fn try_recv(&self) -> Option<WorldActionRequest> {
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

impl Drop for WorldActionWatcher {
    fn drop(&mut self) {
        if self.handle.is_some() {
            self.shutdown();
        }
    }
}

fn watcher_loop(tx: Sender<WorldActionRequest>, shutdown: Arc<AtomicBool>) {
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
                    log::warn!("world action watcher: keydb connect failed: {}", error);
                    continue;
                }
            }
        }

        let conn = con.as_mut().expect("connection just initialised");
        let batch = match drain_batch(conn) {
            Ok(batch) => batch,
            Err(error) => {
                log::warn!("world action watcher: drain failed: {}", error);
                con = None;
                continue;
            }
        };

        for request in batch {
            if tx.send(request).is_err() {
                log::info!("world action watcher: receiver dropped, exiting");
                return;
            }
        }
    }
}

/// Drain up to [`DRAIN_BATCH`] action requests from KeyDB.
///
/// # Arguments
///
/// * `conn` - Live KeyDB connection.
///
/// # Returns
///
/// * `Ok(batch)` — possibly empty when the queue is drained.
/// * `Err(message)` on KeyDB failure.
fn drain_batch(conn: &mut redis::Connection) -> Result<Vec<WorldActionRequest>, String> {
    let raw: Vec<Vec<u8>> = redis::cmd("LPOP")
        .arg(WORLD_ACTION_QUEUE_KEY)
        .arg(DRAIN_BATCH)
        .query(conn)
        .map_err(|error| format!("LPOP {}: {}", WORLD_ACTION_QUEUE_KEY, error))?;

    let mut out = Vec::with_capacity(raw.len());
    for bytes in raw {
        match WorldActionRequest::from_bytes(&bytes) {
            Ok(request) => out.push(request),
            Err(error) => log::warn!(
                "world action watcher: dropping undecodable action: {}",
                error
            ),
        }
    }
    Ok(out)
}

/// Write a pending status entry for an accepted action request.
///
/// # Arguments
///
/// * `con`     - Open KeyDB connection.
/// * `request` - Action request being queued.
///
/// # Returns
///
/// * `Ok(())` on success.
/// * `Err(String)` on KeyDB failure.
pub fn write_pending_status(
    con: &mut redis::Connection,
    request: &WorldActionRequest,
) -> Result<(), String> {
    write_status(con, request, STATUS_PENDING, "queued")
}

/// Write a running status entry for an action request.
///
/// # Arguments
///
/// * `con`     - Open KeyDB connection.
/// * `request` - Action request being executed.
///
/// # Returns
///
/// * `Ok(())` on success.
/// * `Err(String)` on KeyDB failure.
pub fn write_running_status(
    con: &mut redis::Connection,
    request: &WorldActionRequest,
) -> Result<(), String> {
    write_status(con, request, STATUS_RUNNING, "running")
}

/// Write an applied status entry for an action request.
///
/// # Arguments
///
/// * `con`     - Open KeyDB connection.
/// * `request` - Action request that completed.
/// * `message` - Completion detail.
///
/// # Returns
///
/// * `Ok(())` on success.
/// * `Err(String)` on KeyDB failure.
pub fn write_applied_status(
    con: &mut redis::Connection,
    request: &WorldActionRequest,
    message: &str,
) -> Result<(), String> {
    write_status(con, request, STATUS_APPLIED, message)
}

/// Write a failed status entry for an action request.
///
/// # Arguments
///
/// * `con`     - Open KeyDB connection.
/// * `request` - Action request that failed.
/// * `message` - Failure detail.
///
/// # Returns
///
/// * `Ok(())` on success.
/// * `Err(String)` on KeyDB failure.
pub fn write_failed_status(
    con: &mut redis::Connection,
    request: &WorldActionRequest,
    message: &str,
) -> Result<(), String> {
    write_status(con, request, STATUS_FAILED, message)
}

fn write_status(
    con: &mut redis::Connection,
    request: &WorldActionRequest,
    status: &str,
    message: &str,
) -> Result<(), String> {
    let key = world_action_status_key(&request.request_id);
    let value = format_status_value(status, request.action.name(), message, now_secs());
    con.set_ex::<_, _, ()>(key, value, WORLD_ACTION_STATUS_TTL_SECS)
        .map_err(|error| format!("world action status SET failed: {}", error))
}

/// Format a status string stored in KeyDB.
///
/// # Arguments
///
/// * `status`     - Lifecycle status.
/// * `action`     - Stable action name.
/// * `message`    - Human-readable detail.
/// * `updated_at` - Unix timestamp.
///
/// # Returns
///
/// * Pipe-delimited status value.
pub fn format_status_value(status: &str, action: &str, message: &str, updated_at: u64) -> String {
    format!(
        "{}|{}|{}|{}",
        status,
        action,
        updated_at,
        sanitize_status_message(message)
    )
}

fn sanitize_status_message(message: &str) -> String {
    message.replace(['|', '\n', '\r'], " ")
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_status_value_sanitizes_message() {
        assert_eq!(
            format_status_value("failed", "wipe_runtime", "bad|thing\nhere", 42),
            "failed|wipe_runtime|42|bad thing here"
        );
    }
}
