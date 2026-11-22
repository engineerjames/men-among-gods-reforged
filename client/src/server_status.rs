//! Background polling of the game server's live status for the pre-game screens.

use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::time::{Duration, Instant};

/// How often the status is re-checked while the API URL stays the same.
const REFRESH_INTERVAL: Duration = Duration::from_secs(30);

/// Tracks whether the game server behind an API base URL is live.
///
/// Checks run on a background thread so the UI never blocks on HTTP.
pub struct ServerStatus {
    /// API base URL the current status belongs to.
    base_url: String,
    /// Last known status; `None` until the first check for `base_url` finishes.
    live: Option<bool>,
    /// Receiver for the in-flight check, if any.
    pending: Option<Receiver<bool>>,
    /// Earliest time the next check may start.
    next_check: Instant,
}

impl Default for ServerStatus {
    fn default() -> Self {
        Self::new()
    }
}

impl ServerStatus {
    /// Creates a tracker that will check on its first [`ServerStatus::update`].
    ///
    /// # Returns
    ///
    /// * A tracker with unknown status.
    pub fn new() -> Self {
        Self {
            base_url: String::new(),
            live: None,
            pending: None,
            next_check: Instant::now(),
        }
    }

    /// Returns the last known status.
    ///
    /// # Returns
    ///
    /// * `None` while the first check for the current URL is running,
    ///   otherwise whether the server was live.
    pub fn live(&self) -> Option<bool> {
        self.live
    }

    /// Collects a finished check and starts a new one when `base_url` changed
    /// or the refresh interval elapsed.
    ///
    /// # Arguments
    ///
    /// * `base_url` - API base URL that should be checked.
    /// * `check` - Blocking check run on a background thread.
    pub fn update(&mut self, base_url: &str, check: fn(&str) -> bool) {
        if base_url != self.base_url {
            self.base_url = base_url.to_owned();
            self.live = None;
            // Abandon any check for the previous URL; its send just fails.
            self.pending = None;
            self.next_check = Instant::now();
        }

        if let Some(rx) = &self.pending {
            match rx.try_recv() {
                Ok(live) => self.live = Some(live),
                Err(TryRecvError::Disconnected) => self.live = Some(false),
                Err(TryRecvError::Empty) => return,
            }
            self.pending = None;
        }

        let now = Instant::now();
        if now >= self.next_check {
            let (tx, rx) = mpsc::channel();
            let url = self.base_url.clone();
            std::thread::spawn(move || {
                let _ = tx.send(check(&url));
            });
            self.pending = Some(rx);
            self.next_check = now + REFRESH_INTERVAL;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Calls `update` until a status is known, failing after one second.
    fn settle(status: &mut ServerStatus, url: &str, check: fn(&str) -> bool) -> bool {
        let deadline = Instant::now() + Duration::from_secs(1);
        while Instant::now() < deadline {
            status.update(url, check);
            if let Some(live) = status.live() {
                return live;
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        panic!("status check did not complete");
    }

    #[test]
    fn reports_check_result() {
        let mut status = ServerStatus::new();
        assert!(settle(&mut status, "https://a", |_| true));
    }

    #[test]
    fn url_change_resets_and_rechecks() {
        let mut status = ServerStatus::new();
        assert!(settle(&mut status, "https://a", |_| true));

        status.update("https://b", |url| url == "https://a");
        assert_eq!(status.live(), None);
        assert!(!settle(&mut status, "https://b", |url| url == "https://a"));
    }
}
