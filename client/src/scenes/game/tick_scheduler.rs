use std::time::{Duration, Instant};

use mag_core::constants::TICKS;

use crate::scenes::scene::FramePresentation;

use super::QSIZE;

/// Legacy guard that forces a visible frame after prolonged catch-up.
const MAX_CONSECUTIVE_SKIPS: u32 = 100;

/// Schedules gameplay iterations using the legacy client's queue-depth rule.
pub(super) struct LegacyTickScheduler {
    next_deadline: Instant,
    consecutive_skips: u32,
}

impl LegacyTickScheduler {
    /// Creates a scheduler whose first iteration is due immediately.
    ///
    /// # Arguments
    ///
    /// * `now` - Initial presentation deadline.
    ///
    /// # Returns
    ///
    /// * A scheduler ready to complete its first iteration.
    pub(super) fn new(now: Instant) -> Self {
        Self {
            next_deadline: now,
            consecutive_skips: 0,
        }
    }

    /// Resets the scheduler for a new gameplay session.
    ///
    /// # Arguments
    ///
    /// * `now` - Deadline for the first iteration of the new session.
    pub(super) fn reset(&mut self, now: Instant) {
        self.next_deadline = now;
        self.consecutive_skips = 0;
    }

    /// Completes one gameplay iteration and schedules the next one.
    ///
    /// The queue depth is measured after consuming at most one tick, matching
    /// `tick=TICK*QSIZE/t_size` in the C client.
    ///
    /// # Arguments
    ///
    /// * `now` - Time after network application and simulation work.
    /// * `remaining_queue_depth` - Complete tick batches still queued.
    ///
    /// # Returns
    ///
    /// * Whether this iteration should be presented or skipped.
    pub(super) fn complete_iteration(
        &mut self,
        now: Instant,
        remaining_queue_depth: usize,
    ) -> FramePresentation {
        let deadline = self.next_deadline;
        let should_present = now < deadline || self.consecutive_skips > MAX_CONSECUTIVE_SKIPS;

        let presentation = if should_present {
            self.consecutive_skips = 0;
            FramePresentation::PresentAt(deadline)
        } else {
            self.consecutive_skips = self.consecutive_skips.saturating_add(1);
            FramePresentation::Skip
        };

        self.next_deadline += interval_for_queue_depth(remaining_queue_depth);
        presentation
    }
}

/// Computes the next iteration interval from post-consumption queue depth.
fn interval_for_queue_depth(queue_depth: usize) -> Duration {
    let base_nanos = 1_000_000_000u128 / TICKS as u128;
    let interval_nanos = if queue_depth == 0 {
        base_nanos
    } else {
        (base_nanos * u128::from(QSIZE) / queue_depth as u128).max(1)
    };
    Duration::from_nanos(interval_nanos.min(u128::from(u64::MAX)) as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_queue_uses_base_interval() {
        assert_eq!(
            interval_for_queue_depth(0),
            Duration::from_nanos(1_000_000_000 / TICKS as u64)
        );
    }

    #[test]
    fn target_queue_depth_uses_base_interval() {
        assert_eq!(
            interval_for_queue_depth(QSIZE as usize),
            interval_for_queue_depth(0)
        );
    }

    #[test]
    fn shallow_queue_slows_and_deep_queue_accelerates() {
        let base = interval_for_queue_depth(0);
        assert!(interval_for_queue_depth(1) > base);
        assert!(interval_for_queue_depth(16) < base);
    }

    #[test]
    fn late_iterations_skip_until_deadline_recovers() {
        let start = Instant::now();
        let mut scheduler = LegacyTickScheduler::new(start);

        assert_eq!(
            scheduler.complete_iteration(start, 0),
            FramePresentation::Skip
        );
        assert_eq!(
            scheduler.complete_iteration(start, 0),
            FramePresentation::PresentAt(start + interval_for_queue_depth(0))
        );
    }

    #[test]
    fn prolonged_lateness_forces_a_presentation() {
        let start = Instant::now();
        let mut scheduler = LegacyTickScheduler::new(start);
        let late = start + Duration::from_secs(60);

        for _ in 0..=MAX_CONSECUTIVE_SKIPS {
            assert_eq!(
                scheduler.complete_iteration(late, 0),
                FramePresentation::Skip
            );
        }
        assert!(matches!(
            scheduler.complete_iteration(late, 0),
            FramePresentation::PresentAt(_)
        ));
    }

    #[test]
    fn extreme_queue_depth_never_produces_zero_interval() {
        assert!(interval_for_queue_depth(usize::MAX) > Duration::ZERO);
    }
}
