//! Paces the application of queued server ticks independently of the render
//! rate and exposes how far the current tick has progressed for interpolation.
//!
//! The scheduler keeps a small jitter buffer: it steers toward having one
//! complete batch still queued when a tick is applied, slowing playback
//! slightly when the buffer runs dry and speeding up when it grows. Playback
//! never runs slower than `1 + STARVATION_SLOWDOWN_PERCENT`% of the server
//! tick, so a shallow queue can no longer stall rendering.

use std::time::{Duration, Instant};

use mag_core::constants::TICKS;

use super::QSIZE;

/// Queue depth (after consuming one batch) the controller steers toward.
const TARGET_QUEUE_DEPTH: usize = 1;
/// Extra playback time, in percent, applied while the buffer is below target.
const STARVATION_SLOWDOWN_PERCENT: u32 = 5;
/// Maximum ticks applied in one frame before yielding to rendering.
pub(super) const MAX_TICKS_PER_FRAME: usize = 4;

/// Fixed-cadence tick pacer with a one-tick jitter buffer.
pub(super) struct TickScheduler {
    /// Time at which the next queued tick becomes due.
    next_deadline: Instant,
    /// Scheduled time of the most recently applied tick.
    current_tick_at: Instant,
    /// Interval the current tick is displayed for before the next one is due.
    current_interval: Duration,
}

impl TickScheduler {
    /// Creates a scheduler whose first tick is due immediately.
    ///
    /// # Arguments
    ///
    /// * `now` - Time of the first tick deadline.
    ///
    /// # Returns
    ///
    /// * A scheduler ready to apply its first tick.
    pub(super) fn new(now: Instant) -> Self {
        Self {
            next_deadline: now,
            current_tick_at: now,
            current_interval: base_interval(),
        }
    }

    /// Resets the scheduler for a new gameplay session.
    ///
    /// # Arguments
    ///
    /// * `now` - Deadline for the first tick of the new session.
    pub(super) fn reset(&mut self, now: Instant) {
        *self = Self::new(now);
    }

    /// Returns whether the next tick should be applied at `now`.
    ///
    /// # Arguments
    ///
    /// * `now` - Current time.
    ///
    /// # Returns
    ///
    /// * `true` once the next deadline has been reached.
    pub(super) fn is_due(&self, now: Instant) -> bool {
        now >= self.next_deadline
    }

    /// Records that the due tick was applied and schedules the next one.
    ///
    /// # Arguments
    ///
    /// * `remaining_queue_depth` - Complete batches still queued after this tick.
    pub(super) fn tick_applied(&mut self, remaining_queue_depth: usize) {
        self.current_tick_at = self.next_deadline;
        self.current_interval = interval_for_queue_depth(remaining_queue_depth);
        self.next_deadline += self.current_interval;
    }

    /// Records that a tick was due but no batch had arrived yet.
    ///
    /// The schedule is re-anchored to `now` so the late batch is applied as
    /// soon as it arrives and no phantom deadlines accumulate during a stall.
    ///
    /// # Arguments
    ///
    /// * `now` - Current time.
    pub(super) fn starved(&mut self, now: Instant) {
        self.next_deadline = now;
    }

    /// Returns the interpolation progress from the current tick toward the next.
    ///
    /// # Arguments
    ///
    /// * `now` - Current time.
    ///
    /// # Returns
    ///
    /// * A value in `[0, 1]`; it saturates at `1` while waiting on a late tick.
    pub(super) fn interpolation_alpha(&self, now: Instant) -> f32 {
        let elapsed = now.saturating_duration_since(self.current_tick_at);
        let interval = self.current_interval.as_secs_f32();
        if interval <= 0.0 {
            return 1.0;
        }
        (elapsed.as_secs_f32() / interval).clamp(0.0, 1.0)
    }
}

/// Returns the nominal server tick interval.
fn base_interval() -> Duration {
    Duration::from_nanos(1_000_000_000 / TICKS as u64)
}

/// Computes the playback interval from the queue depth left after applying a tick.
///
/// # Arguments
///
/// * `queue_depth` - Complete batches still queued.
///
/// # Returns
///
/// * Slightly longer than a tick below the target depth, exactly a tick at the
///   target, and progressively shorter as the backlog grows.
fn interval_for_queue_depth(queue_depth: usize) -> Duration {
    let base = base_interval();
    if queue_depth < TARGET_QUEUE_DEPTH {
        base * (100 + STARVATION_SLOWDOWN_PERCENT) / 100
    } else if queue_depth == TARGET_QUEUE_DEPTH {
        base
    } else {
        let excess = (queue_depth - TARGET_QUEUE_DEPTH) as u32;
        let divisor = QSIZE.saturating_add(excess).max(1);
        (base * QSIZE / divisor).max(Duration::from_nanos(1))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn target_depth_uses_base_interval() {
        assert_eq!(
            interval_for_queue_depth(TARGET_QUEUE_DEPTH),
            base_interval()
        );
    }

    #[test]
    fn empty_queue_slows_only_slightly() {
        let base = base_interval();
        let slowed = interval_for_queue_depth(0);
        assert!(slowed > base);
        assert!(slowed < base * 2);
    }

    #[test]
    fn backlog_accelerates_and_never_stalls() {
        let base = base_interval();
        let mut previous = base;
        for depth in 2..64 {
            let interval = interval_for_queue_depth(depth);
            assert!(interval < base, "depth {depth} must run faster than base");
            assert!(interval <= previous, "depth {depth} must not slow down");
            assert!(interval > Duration::ZERO);
            previous = interval;
        }
    }

    #[test]
    fn no_depth_ever_exceeds_the_slowdown_bound() {
        let bound = base_interval() * (100 + STARVATION_SLOWDOWN_PERCENT) / 100;
        for depth in 0..256 {
            assert!(interval_for_queue_depth(depth) <= bound);
        }
    }

    #[test]
    fn first_tick_is_due_immediately() {
        let start = Instant::now();
        let scheduler = TickScheduler::new(start);
        assert!(scheduler.is_due(start));
    }

    #[test]
    fn applied_tick_schedules_next_one_tick_later() {
        let start = Instant::now();
        let mut scheduler = TickScheduler::new(start);
        scheduler.tick_applied(TARGET_QUEUE_DEPTH);

        assert!(!scheduler.is_due(start));
        assert!(!scheduler.is_due(start + base_interval() - Duration::from_millis(1)));
        assert!(scheduler.is_due(start + base_interval()));
    }

    #[test]
    fn interpolation_alpha_progresses_and_saturates() {
        let start = Instant::now();
        let mut scheduler = TickScheduler::new(start);
        scheduler.tick_applied(TARGET_QUEUE_DEPTH);
        let half = base_interval() / 2;

        assert_eq!(scheduler.interpolation_alpha(start), 0.0);
        assert!((scheduler.interpolation_alpha(start + half) - 0.5).abs() < 0.01);
        assert_eq!(
            scheduler.interpolation_alpha(start + base_interval() * 3),
            1.0
        );
    }

    #[test]
    fn starvation_reanchors_schedule_to_now() {
        let start = Instant::now();
        let mut scheduler = TickScheduler::new(start);
        scheduler.tick_applied(TARGET_QUEUE_DEPTH);
        let late = start + Duration::from_secs(2);

        scheduler.starved(late);
        assert!(scheduler.is_due(late));
        assert!(!scheduler.is_due(late - Duration::from_millis(1)));

        scheduler.tick_applied(TARGET_QUEUE_DEPTH);
        assert_eq!(scheduler.interpolation_alpha(late), 0.0);
        assert!(scheduler.is_due(late + base_interval()));
    }

    #[test]
    fn deep_backlog_makes_several_ticks_due_in_one_frame() {
        let start = Instant::now();
        let mut scheduler = TickScheduler::new(start);
        let frame = start + Duration::from_millis(16);

        let mut applied = 0;
        while scheduler.is_due(frame) && applied < MAX_TICKS_PER_FRAME {
            scheduler.tick_applied(40 - applied);
            applied += 1;
        }
        assert_eq!(applied, MAX_TICKS_PER_FRAME);
    }
}
