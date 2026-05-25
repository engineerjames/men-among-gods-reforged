//! Background watcher for admin-issued live ban enforcement actions.

use core::ban_action_store::{
    BAN_ACTION_QUEUE_KEY, BAN_ACTION_STATUS_TTL_SECS, BanActionRequest, STATUS_APPLIED,
    STATUS_FAILED, STATUS_PENDING, STATUS_RUNNING, ban_action_status_key,
};
use redis::Commands;
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::thread::{self, JoinHandle};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Environment variable that disables the watcher entirely when true-ish.
pub const DISABLE_ENV: &str = "MAG_ADMIN_BAN_ACTION_DISABLED";

const POLL_INTERVAL: Duration = Duration::from_millis(250);
const DRAIN_BATCH: usize = 64;

/// Handle for the live ban-action watcher thread.
pub struct BanActionWatcher {
    rx: Receiver<BanActionRequest>,
    shutdown: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl BanActionWatcher {
    /// Spawn the watcher thread.
    ///
    /// # Returns
    ///
    /// * `Some(watcher)` on success.
    /// * `None` when disabled or spawning fails.
    pub fn spawn() -> Option<Self> {
        if std::env::var(DISABLE_ENV)
            .map(|value| matches!(value.to_lowercase().as_str(), "1" | "true" | "yes"))
            .unwrap_or(false)
        {
            log::info!("Ban action watcher disabled via {} env var", DISABLE_ENV);
            return None;
        }

        let (tx, rx) = mpsc::channel();
        let shutdown = Arc::new(AtomicBool::new(false));
        let shutdown_thread = Arc::clone(&shutdown);
        let handle = thread::Builder::new()
            .name("ban-action-watcher".into())
            .spawn(move || watcher_loop(tx, shutdown_thread))
            .ok()?;

        log::info!("Ban action watcher started");
        Some(Self {
            rx,
            shutdown,
            handle: Some(handle),
        })
    }

    /// Try to receive the next pending live action without blocking.
    ///
    /// # Returns
    ///
    /// * `Some(request)` when an action is ready.
    /// * `None` when the queue is empty or the watcher has stopped.
    pub fn try_recv(&self) -> Option<BanActionRequest> {
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

impl Drop for BanActionWatcher {
    fn drop(&mut self) {
        if self.handle.is_some() {
            self.shutdown();
        }
    }
}

fn watcher_loop(tx: Sender<BanActionRequest>, shutdown: Arc<AtomicBool>) {
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
                    log::warn!("ban action watcher: keydb connect failed: {}", error);
                    continue;
                }
            }
        }

        let conn = con.as_mut().expect("connection just initialised");
        let batch = match drain_batch(conn) {
            Ok(batch) => batch,
            Err(error) => {
                log::warn!("ban action watcher: drain failed: {}", error);
                con = None;
                continue;
            }
        };

        for request in batch {
            if tx.send(request).is_err() {
                log::info!("ban action watcher: receiver dropped, exiting");
                return;
            }
        }
    }
}

fn drain_batch(conn: &mut redis::Connection) -> Result<Vec<BanActionRequest>, String> {
    let raw: Vec<Vec<u8>> = redis::cmd("LPOP")
        .arg(BAN_ACTION_QUEUE_KEY)
        .arg(DRAIN_BATCH)
        .query(conn)
        .map_err(|error| format!("LPOP {}: {}", BAN_ACTION_QUEUE_KEY, error))?;

    let mut out = Vec::with_capacity(raw.len());
    for bytes in raw {
        match BanActionRequest::from_bytes(&bytes) {
            Ok(request) => out.push(request),
            Err(error) => log::warn!("ban action watcher: dropping undecodable action: {}", error),
        }
    }
    Ok(out)
}

/// Write a running status entry for a live ban action.
///
/// # Arguments
///
/// * `con` - Value passed to `write_running_status`.
/// * `request` - Value passed to `write_running_status`.
pub fn write_running_status(
    con: &mut redis::Connection,
    request: &BanActionRequest,
) -> Result<(), String> {
    write_status(con, request, STATUS_RUNNING, "running")
}

/// Write an applied status entry for a live ban action.
///
/// # Arguments
///
/// * `con` - Value passed to `write_applied_status`.
/// * `request` - Value passed to `write_applied_status`.
/// * `message` - Value passed to `write_applied_status`.
pub fn write_applied_status(
    con: &mut redis::Connection,
    request: &BanActionRequest,
    message: &str,
) -> Result<(), String> {
    write_status(con, request, STATUS_APPLIED, message)
}

/// Write a failed status entry for a live ban action.
///
/// # Arguments
///
/// * `con` - Value passed to `write_failed_status`.
/// * `request` - Value passed to `write_failed_status`.
/// * `message` - Value passed to `write_failed_status`.
pub fn write_failed_status(
    con: &mut redis::Connection,
    request: &BanActionRequest,
    message: &str,
) -> Result<(), String> {
    write_status(con, request, STATUS_FAILED, message)
}

/// Write a pending status entry for a live ban action.
///
/// # Arguments
///
/// * `con` - Value passed to `write_pending_status`.
/// * `request` - Value passed to `write_pending_status`.
pub fn write_pending_status(
    con: &mut redis::Connection,
    request: &BanActionRequest,
) -> Result<(), String> {
    write_status(con, request, STATUS_PENDING, "queued")
}

fn write_status(
    con: &mut redis::Connection,
    request: &BanActionRequest,
    status: &str,
    message: &str,
) -> Result<(), String> {
    let key = ban_action_status_key(&request.request_id);
    let value = format_status_value(status, request.action.name(), message, now_secs());
    con.set_ex::<_, _, ()>(key, value, BAN_ACTION_STATUS_TTL_SECS)
        .map_err(|error| format!("ban action status SET failed: {}", error))
}

fn format_status_value(status: &str, action: &str, message: &str, updated_at: u64) -> String {
    format!(
        "{}|{}|{}|{}",
        status,
        action,
        updated_at,
        message.replace(['|', '\n', '\r'], " ")
    )
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}
