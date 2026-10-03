//! Background capacity monitor for the server's fixed-size data tables.
//!
//! Characters, items, effects, templates and player slots all live in
//! fixed-size arrays whose limits are compile-time constants. If any of them
//! fills up, the server can no longer create new entries. This module gives
//! early warning: every [`CHECK_INTERVAL_TICKS`] the tick loop counts the
//! in-use slots (a cheap linear scan) and hands the resulting
//! [`CapacityReport`] to a dedicated thread, which logs the fill percentage
//! of every table and escalates the log level as tables approach their limit.

use core::constants::{
    MAXCHARS, MAXEFFECT, MAXITEM, MAXPLAYER, MAXTCHARS, MAXTITEM, TICKS, USE_EMPTY,
};
use std::sync::mpsc::{self, Sender};
use std::thread::{self, JoinHandle};

use crate::game_state::GameState;

/// Ticks between capacity checks (5 minutes of wall-clock time).
pub const CHECK_INTERVAL_TICKS: u32 = 5 * 60 * TICKS as u32;

/// Fill percentage at or above which a table is logged as a warning.
pub const WARN_PERCENT: f64 = 75.0;

/// Fill percentage at or above which a table is logged as an error.
pub const CRITICAL_PERCENT: f64 = 90.0;

/// How full a single table is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CapacitySample {
    /// Human-readable table name.
    pub name: &'static str,
    /// Number of slots currently in use.
    pub used: usize,
    /// Number of usable slots (slot 0 is reserved and excluded).
    pub capacity: usize,
}

/// Severity of a sample relative to the warning thresholds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    /// Below [`WARN_PERCENT`].
    Ok,
    /// At or above [`WARN_PERCENT`].
    Warning,
    /// At or above [`CRITICAL_PERCENT`].
    Critical,
}

impl CapacitySample {
    /// Build a sample from a table's `used` flags, ignoring reserved slot 0.
    ///
    /// # Arguments
    ///
    /// * `name` - Table name used in log output.
    /// * `capacity` - Total slots in the table, including reserved slot 0.
    /// * `used_flags` - The `used` byte of every slot, in index order.
    ///
    /// # Returns
    ///
    /// * A sample counting slots whose flag is not `USE_EMPTY`.
    pub fn from_used_flags(
        name: &'static str,
        capacity: usize,
        used_flags: impl Iterator<Item = u8>,
    ) -> Self {
        let used = used_flags
            .skip(1)
            .take(capacity.saturating_sub(1))
            .filter(|&flag| flag != USE_EMPTY)
            .count();
        Self {
            name,
            used,
            capacity: capacity.saturating_sub(1),
        }
    }

    /// Fill level as a percentage in `0.0..=100.0`.
    ///
    /// # Returns
    ///
    /// * `used / capacity * 100`, or `0.0` for a zero-capacity table.
    pub fn percent(&self) -> f64 {
        if self.capacity == 0 {
            return 0.0;
        }
        self.used as f64 * 100.0 / self.capacity as f64
    }

    /// Classify this sample against the warning thresholds.
    ///
    /// # Returns
    ///
    /// * The [`Severity`] for this table's current fill level.
    pub fn severity(&self) -> Severity {
        let percent = self.percent();
        if percent >= CRITICAL_PERCENT {
            Severity::Critical
        } else if percent >= WARN_PERCENT {
            Severity::Warning
        } else {
            Severity::Ok
        }
    }
}

/// Fill levels for every monitored table at one point in time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapacityReport {
    /// One sample per monitored table.
    pub samples: Vec<CapacitySample>,
}

impl CapacityReport {
    /// Count in-use slots in all monitored tables.
    ///
    /// # Arguments
    ///
    /// * `gs` - Game state to inspect (read-only).
    ///
    /// # Returns
    ///
    /// * A report with one sample per table.
    pub fn sample(gs: &GameState) -> Self {
        let samples = vec![
            CapacitySample::from_used_flags(
                "characters",
                MAXCHARS,
                gs.characters.iter().map(|c| c.used),
            ),
            CapacitySample::from_used_flags("items", MAXITEM, gs.items.iter().map(|i| i.used)),
            CapacitySample::from_used_flags(
                "effects",
                MAXEFFECT,
                gs.effects.iter().map(|e| e.used),
            ),
            CapacitySample::from_used_flags(
                "character templates",
                MAXTCHARS,
                gs.character_templates.iter().map(|c| c.used),
            ),
            CapacitySample::from_used_flags(
                "item templates",
                MAXTITEM,
                gs.item_templates.iter().map(|i| i.used),
            ),
            CapacitySample::from_used_flags(
                "player connections",
                MAXPLAYER,
                gs.players.iter().map(|p| u8::from(p.sock.is_some())),
            ),
        ];
        Self { samples }
    }

    /// Log the report: one summary line, plus one line per table at or above
    /// the warning thresholds.
    fn log(&self) {
        let summary = self
            .samples
            .iter()
            .map(|s| format!("{} {}/{} ({:.1}%)", s.name, s.used, s.capacity, s.percent()))
            .collect::<Vec<_>>()
            .join(", ");
        log::info!("Capacity: {summary}");

        for sample in &self.samples {
            match sample.severity() {
                Severity::Ok => {}
                Severity::Warning => log::warn!(
                    "Capacity warning: {} at {:.1}% ({}/{}); consider raising the limit",
                    sample.name,
                    sample.percent(),
                    sample.used,
                    sample.capacity
                ),
                Severity::Critical => log::error!(
                    "Capacity CRITICAL: {} at {:.1}% ({}/{}); schedule a restart with a larger limit",
                    sample.name,
                    sample.percent(),
                    sample.used,
                    sample.capacity
                ),
            }
        }
    }
}

/// Handle for the capacity monitor thread.
pub struct CapacityMonitor {
    tx: Option<Sender<CapacityReport>>,
    handle: Option<JoinHandle<()>>,
}

impl CapacityMonitor {
    /// Spawn the monitor thread.
    ///
    /// # Returns
    ///
    /// * `Some(CapacityMonitor)` on success, `None` if the thread cannot start.
    pub fn spawn() -> Option<Self> {
        let (tx, rx) = mpsc::channel::<CapacityReport>();
        let handle = thread::Builder::new()
            .name("capacity-monitor".into())
            .spawn(move || {
                while let Ok(report) = rx.recv() {
                    report.log();
                }
            })
            .ok()?;

        log::info!(
            "Capacity monitor started (every {} s)",
            CHECK_INTERVAL_TICKS / TICKS as u32
        );
        Some(Self {
            tx: Some(tx),
            handle: Some(handle),
        })
    }

    /// Queue a report for logging; non-blocking.
    ///
    /// # Arguments
    ///
    /// * `report` - The report to log on the monitor thread.
    pub fn submit(&self, report: CapacityReport) {
        if let Some(tx) = &self.tx
            && tx.send(report).is_err()
        {
            log::warn!("Capacity monitor thread is no longer running");
        }
    }

    /// Stop the thread after it drains pending reports and join it.
    pub fn shutdown(&mut self) {
        self.tx.take();
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for CapacityMonitor {
    fn drop(&mut self) {
        self.shutdown();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::constants::USE_ACTIVE;

    fn sample(used: usize, capacity: usize) -> CapacitySample {
        CapacitySample {
            name: "test",
            used,
            capacity,
        }
    }

    #[test]
    fn from_used_flags_skips_reserved_slot_zero() {
        let flags = [USE_ACTIVE, USE_ACTIVE, USE_EMPTY, USE_ACTIVE];
        let s = CapacitySample::from_used_flags("t", flags.len(), flags.into_iter());
        assert_eq!(s.used, 2);
        assert_eq!(s.capacity, 3);
    }

    #[test]
    fn from_used_flags_ignores_entries_beyond_capacity() {
        let flags = [USE_EMPTY, USE_ACTIVE, USE_ACTIVE, USE_ACTIVE];
        let s = CapacitySample::from_used_flags("t", 3, flags.into_iter());
        assert_eq!(s.used, 2);
        assert_eq!(s.capacity, 2);
    }

    #[test]
    fn percent_handles_zero_capacity() {
        assert_eq!(sample(0, 0).percent(), 0.0);
    }

    #[test]
    fn percent_is_used_over_capacity() {
        assert_eq!(sample(50, 200).percent(), 25.0);
    }

    #[test]
    fn severity_thresholds() {
        assert_eq!(sample(74, 100).severity(), Severity::Ok);
        assert_eq!(sample(75, 100).severity(), Severity::Warning);
        assert_eq!(sample(89, 100).severity(), Severity::Warning);
        assert_eq!(sample(90, 100).severity(), Severity::Critical);
        assert_eq!(sample(100, 100).severity(), Severity::Critical);
    }

    #[test]
    fn monitor_accepts_reports_and_shuts_down_twice() {
        let mut monitor = CapacityMonitor::spawn().expect("spawn");
        monitor.submit(CapacityReport {
            samples: vec![sample(1, 10)],
        });
        monitor.shutdown();
        monitor.shutdown();
    }
}
