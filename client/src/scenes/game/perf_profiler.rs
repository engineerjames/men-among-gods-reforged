//! Lightweight wall-clock profiler for rendering functions.
//!
//! Activated from the escape menu via the **Profile Performance** button.
//! Collects per-frame timing for each top-level draw call for a fixed window
//! (default 10 s), then writes an aggregated summary (min / avg / max / p95
//! plus percentage of the 60 FPS budget) to the log file.

use std::time::{Duration, Instant};

const PROFILE_DURATION: Duration = Duration::from_secs(10);

/// The frame-time budget we compare against (60 FPS ≈ 16.667 ms).
const TARGET_FRAME_TIME: Duration = Duration::from_micros(16_667);

/// Type-safe labels for every instrumented rendering function.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum PerfLabel {
    DrawWorld,
    DrawUiFrame,
    DrawBars,
    DrawStatText,
    DrawChat,
    DrawModeIndicators,
    DrawAttributesSkills,
    DrawInventoryEquipmentSpells,
    DrawPortraitAndShop,
    DrawMinimap,
    DrawSkillButtonLabels,
    RenderUi,
}

impl PerfLabel {
    /// All variants in the order they should appear in the summary.
    const ALL: [PerfLabel; 12] = [
        Self::DrawWorld,
        Self::DrawUiFrame,
        Self::DrawBars,
        Self::DrawStatText,
        Self::DrawChat,
        Self::DrawModeIndicators,
        Self::DrawAttributesSkills,
        Self::DrawInventoryEquipmentSpells,
        Self::DrawPortraitAndShop,
        Self::DrawMinimap,
        Self::DrawSkillButtonLabels,
        Self::RenderUi,
    ];

    fn as_str(self) -> &'static str {
        match self {
            Self::DrawWorld => "draw_world",
            Self::DrawUiFrame => "draw_ui_frame",
            Self::DrawBars => "draw_bars",
            Self::DrawStatText => "draw_stat_text",
            Self::DrawChat => "draw_chat",
            Self::DrawModeIndicators => "draw_mode_indicators",
            Self::DrawAttributesSkills => "draw_attributes_skills",
            Self::DrawInventoryEquipmentSpells => "draw_inventory_equipment_spells",
            Self::DrawPortraitAndShop => "draw_portrait_and_shop",
            Self::DrawMinimap => "draw_minimap",
            Self::DrawSkillButtonLabels => "draw_skill_button_labels",
            Self::RenderUi => "render_ui",
        }
    }
}

// ── Per-frame sample ────────────────────────────────────────────────────────

/// Timing data captured for a single frame while profiling is active.
struct FrameSample {
    function_times: Vec<(PerfLabel, Duration)>,
    total_frame_time: Duration,
}

/// A zero-cost-when-inactive wall-clock profiler for rendering functions.
///
/// Usage from `GameScene`:
///
/// ```ignore
/// // In update():
/// self.perf_profiler.check_expired();
///
/// // In render_world():
/// self.perf_profiler.begin_frame();
/// self.perf_profiler.begin_sample(PerfLabel::DrawWorld);
/// self.draw_world(…);
/// self.perf_profiler.end_sample(PerfLabel::DrawWorld);
/// // … more draw calls …
/// self.perf_profiler.end_frame();
/// ```
pub(super) struct PerfProfiler {
    active: bool,
    start_time: Option<Instant>,
    frame_samples: Vec<FrameSample>,
    /// Start timestamp set by `begin_frame`, consumed by `end_frame`.
    frame_start: Option<Instant>,
    /// Start timestamp set by `begin_sample`, consumed by `end_sample`.
    pending_sample: Option<Instant>,
    /// Function timings accumulated during the current frame.
    current_times: Vec<(PerfLabel, Duration)>,
}

impl Default for PerfProfiler {
    fn default() -> Self {
        Self::new()
    }
}

impl PerfProfiler {
    pub fn new() -> Self {
        Self {
            active: false,
            start_time: None,
            frame_samples: Vec::new(),
            frame_start: None,
            pending_sample: None,
            current_times: Vec::with_capacity(16),
        }
    }

    /// Start (or restart) a profiling session.
    pub fn start(&mut self) {
        self.active = true;
        self.start_time = Some(Instant::now());
        self.frame_samples.clear();
        self.frame_samples.reserve(700); // ~600 frames in 10 s at 60 FPS
        self.frame_start = None;
        self.pending_sample = None;
        self.current_times.clear();
        log::info!(
            "Performance profiling started ({}s window)",
            PROFILE_DURATION.as_secs()
        );
    }

    /// Whether profiling is currently active.
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Returns the number of seconds remaining in the profiling window, or 0.
    pub fn remaining_secs(&self) -> u64 {
        self.start_time
            .map(|t| {
                PROFILE_DURATION
                    .checked_sub(t.elapsed())
                    .unwrap_or(Duration::ZERO)
                    .as_secs()
            })
            .unwrap_or(0)
    }

    /// Call once per frame (e.g. at the start of `update`) to check whether
    /// the capture window has elapsed. If so, logs the summary and deactivates.
    pub fn check_expired(&mut self) {
        if self.active {
            if let Some(start) = self.start_time {
                if start.elapsed() >= PROFILE_DURATION {
                    self.finish();
                }
            }
        }
    }

    // ── Per-frame API ───────────────────────────────────────────────────

    /// Begin timing a new frame. Call at the top of `render_world`.
    pub fn begin_frame(&mut self) {
        if self.active {
            self.current_times.clear();
            self.frame_start = Some(Instant::now());
        }
    }

    /// Start timing a single draw call. Pairs with [`end_sample`].
    /// No-op when profiling is inactive.
    pub fn begin_sample(&mut self, _label: PerfLabel) {
        if self.active {
            self.pending_sample = Some(Instant::now());
        }
    }

    /// Stop timing the draw call started by [`begin_sample`] and record it.
    /// No-op when profiling is inactive.
    pub fn end_sample(&mut self, label: PerfLabel) {
        if let Some(start) = self.pending_sample.take() {
            self.current_times.push((label, start.elapsed()));
        }
    }

    /// Finish timing the current frame and stash the sample.
    pub fn end_frame(&mut self) {
        if let Some(frame_start) = self.frame_start.take() {
            let sample = FrameSample {
                function_times: self.current_times.drain(..).collect(),
                total_frame_time: frame_start.elapsed(),
            };
            self.frame_samples.push(sample);
        }
    }

    // ── Internal ────────────────────────────────────────────────────────

    fn finish(&mut self) {
        self.active = false;
        self.log_summary();
        self.frame_start = None;
        self.pending_sample = None;
        self.current_times.clear();
    }

    fn log_summary(&self) {
        let n = self.frame_samples.len();
        if n == 0 {
            log::info!("Performance profiling finished — no frames captured.");
            return;
        }

        let elapsed_secs = self
            .start_time
            .map(|t| t.elapsed().as_secs_f64())
            .unwrap_or(PROFILE_DURATION.as_secs_f64());

        // Collect durations per label.
        let mut per_label: [Vec<Duration>; PerfLabel::ALL.len()] =
            std::array::from_fn(|_| Vec::with_capacity(n));
        for sample in &self.frame_samples {
            for &(label, dur) in &sample.function_times {
                let idx = PerfLabel::ALL.iter().position(|&l| l == label).unwrap();
                per_label[idx].push(dur);
            }
        }

        let mut frame_times: Vec<Duration> = self
            .frame_samples
            .iter()
            .map(|s| s.total_frame_time)
            .collect();

        let budget_us = TARGET_FRAME_TIME.as_micros() as f64;
        let frames_over = frame_times
            .iter()
            .filter(|t| **t > TARGET_FRAME_TIME)
            .count();

        log::info!(
            "=== Performance Profile ({:.1}s, {} frames) ===",
            elapsed_secs,
            n
        );
        log::info!(
            "{:<40} {:>8} {:>8} {:>8} {:>8} {:>8}",
            "Function",
            "Min",
            "Avg",
            "Max",
            "p95",
            "%Budget"
        );

        for (i, label) in PerfLabel::ALL.iter().enumerate() {
            let durations = &mut per_label[i];
            if durations.is_empty() {
                continue;
            }
            let stats = compute_stats(durations);
            log::info!(
                "{:<40} {:>8} {:>8} {:>8} {:>8} {:>7.1}%",
                label.as_str(),
                format_duration(stats.min),
                format_duration(stats.avg),
                format_duration(stats.max),
                format_duration(stats.p95),
                stats.avg.as_micros() as f64 / budget_us * 100.0,
            );
        }

        let total_stats = compute_stats(&mut frame_times);
        log::info!(
            "{:<40} {:>8} {:>8} {:>8} {:>8} {:>7.1}%",
            "TOTAL frame time",
            format_duration(total_stats.min),
            format_duration(total_stats.avg),
            format_duration(total_stats.max),
            format_duration(total_stats.p95),
            total_stats.avg.as_micros() as f64 / budget_us * 100.0,
        );
        log::info!(
            "Target budget: {:.2}ms (60 FPS) | Frames over budget: {} / {} ({:.1}%)",
            TARGET_FRAME_TIME.as_secs_f64() * 1000.0,
            frames_over,
            n,
            frames_over as f64 / n as f64 * 100.0,
        );
        log::info!("=== End Performance Profile ===");
    }
}

// ── Helpers ─────────────────────────────────────────────────────────────────

struct Stats {
    min: Duration,
    avg: Duration,
    max: Duration,
    p95: Duration,
}

fn compute_stats(durations: &mut [Duration]) -> Stats {
    durations.sort_unstable();
    let n = durations.len();
    let sum: Duration = durations.iter().sum();
    let avg = sum / n as u32;
    let p95_idx = ((n as f64) * 0.95).ceil() as usize;
    let p95 = durations[p95_idx.min(n - 1)];
    Stats {
        min: durations[0],
        avg,
        max: durations[n - 1],
        p95,
    }
}

fn format_duration(d: Duration) -> String {
    let us = d.as_micros();
    if us >= 1_000 {
        format!("{:.1}ms", us as f64 / 1000.0)
    } else {
        format!("{}µs", us)
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── format_duration ──────────────────────────────────────────────────

    #[test]
    fn format_duration_sub_millisecond() {
        assert_eq!(format_duration(Duration::from_micros(0)), "0µs");
        assert_eq!(format_duration(Duration::from_micros(1)), "1µs");
        assert_eq!(format_duration(Duration::from_micros(999)), "999µs");
    }

    #[test]
    fn format_duration_millisecond_boundary() {
        // Exactly 1000 µs should switch to ms representation.
        assert_eq!(format_duration(Duration::from_micros(1_000)), "1.0ms");
    }

    #[test]
    fn format_duration_milliseconds() {
        assert_eq!(format_duration(Duration::from_micros(1_500)), "1.5ms");
        assert_eq!(format_duration(Duration::from_micros(16_667)), "16.7ms");
        assert_eq!(format_duration(Duration::from_millis(100)), "100.0ms");
    }

    // ── compute_stats ────────────────────────────────────────────────────

    #[test]
    fn compute_stats_single_element() {
        let mut v = vec![Duration::from_micros(42)];
        let s = compute_stats(&mut v);
        assert_eq!(s.min, Duration::from_micros(42));
        assert_eq!(s.avg, Duration::from_micros(42));
        assert_eq!(s.max, Duration::from_micros(42));
        assert_eq!(s.p95, Duration::from_micros(42));
    }

    #[test]
    fn compute_stats_min_avg_max() {
        // [1, 2, 3, 4, 5] µs → min=1, avg=3, max=5
        let mut v: Vec<Duration> = (1u64..=5).map(Duration::from_micros).collect();
        let s = compute_stats(&mut v);
        assert_eq!(s.min, Duration::from_micros(1));
        assert_eq!(s.avg, Duration::from_micros(3));
        assert_eq!(s.max, Duration::from_micros(5));
    }

    #[test]
    fn compute_stats_sorted_by_function() {
        // Input out of order — function must sort before computing.
        let mut v = vec![
            Duration::from_micros(5),
            Duration::from_micros(1),
            Duration::from_micros(3),
        ];
        let s = compute_stats(&mut v);
        assert_eq!(s.min, Duration::from_micros(1));
        assert_eq!(s.max, Duration::from_micros(5));
    }

    #[test]
    fn compute_stats_p95_small_slice() {
        // n=10: p95_idx = ceil(10 * 0.95) = ceil(9.5) = 10, clamped to 9 → last element.
        let mut v: Vec<Duration> = (1u64..=10).map(Duration::from_micros).collect();
        let s = compute_stats(&mut v);
        assert_eq!(s.p95, Duration::from_micros(10));
    }

    #[test]
    fn compute_stats_p95_larger_slice() {
        // n=100: p95_idx = ceil(100 * 0.95) = 95 → durations[95] (0-based) = 96µs.
        let mut v: Vec<Duration> = (1u64..=100).map(Duration::from_micros).collect();
        let s = compute_stats(&mut v);
        assert_eq!(s.p95, Duration::from_micros(96));
    }

    #[test]
    fn compute_stats_average_truncates_not_rounds() {
        // [1ns, 2ns] → sum=3ns, 3/2=1ns (truncated at nanosecond level, not 1.5ns).
        let mut v = vec![Duration::from_nanos(1), Duration::from_nanos(2)];
        let s = compute_stats(&mut v);
        assert_eq!(s.avg, Duration::from_nanos(1));
    }

    // ── PerfLabel ────────────────────────────────────────────────────────

    #[test]
    fn perf_label_as_str_all_variants() {
        let cases = [
            (PerfLabel::DrawWorld, "draw_world"),
            (PerfLabel::DrawUiFrame, "draw_ui_frame"),
            (PerfLabel::DrawBars, "draw_bars"),
            (PerfLabel::DrawStatText, "draw_stat_text"),
            (PerfLabel::DrawChat, "draw_chat"),
            (PerfLabel::DrawModeIndicators, "draw_mode_indicators"),
            (PerfLabel::DrawAttributesSkills, "draw_attributes_skills"),
            (
                PerfLabel::DrawInventoryEquipmentSpells,
                "draw_inventory_equipment_spells",
            ),
            (PerfLabel::DrawPortraitAndShop, "draw_portrait_and_shop"),
            (PerfLabel::DrawMinimap, "draw_minimap"),
            (PerfLabel::DrawSkillButtonLabels, "draw_skill_button_labels"),
            (PerfLabel::RenderUi, "render_ui"),
        ];
        for (label, expected) in cases {
            assert_eq!(label.as_str(), expected, "as_str mismatch for {:?}", label);
        }
    }

    #[test]
    fn perf_label_all_covers_every_variant() {
        // Every variant must appear exactly once in ALL.
        let all_variants = [
            PerfLabel::DrawWorld,
            PerfLabel::DrawUiFrame,
            PerfLabel::DrawBars,
            PerfLabel::DrawStatText,
            PerfLabel::DrawChat,
            PerfLabel::DrawModeIndicators,
            PerfLabel::DrawAttributesSkills,
            PerfLabel::DrawInventoryEquipmentSpells,
            PerfLabel::DrawPortraitAndShop,
            PerfLabel::DrawMinimap,
            PerfLabel::DrawSkillButtonLabels,
            PerfLabel::RenderUi,
        ];
        assert_eq!(PerfLabel::ALL.len(), all_variants.len());
        for v in all_variants {
            assert!(
                PerfLabel::ALL.contains(&v),
                "{:?} missing from PerfLabel::ALL",
                v
            );
        }
    }

    // ── PerfProfiler — initial state ─────────────────────────────────────

    #[test]
    fn new_profiler_is_inactive() {
        let p = PerfProfiler::new();
        assert!(!p.is_active());
    }

    #[test]
    fn default_profiler_is_inactive() {
        let p = PerfProfiler::default();
        assert!(!p.is_active());
    }

    #[test]
    fn remaining_secs_without_session_is_zero() {
        let p = PerfProfiler::new();
        assert_eq!(p.remaining_secs(), 0);
    }

    // ── PerfProfiler — start / is_active / remaining_secs ────────────────

    #[test]
    fn start_makes_profiler_active() {
        let mut p = PerfProfiler::new();
        p.start();
        assert!(p.is_active());
    }

    #[test]
    fn remaining_secs_immediately_after_start_is_nonzero() {
        let mut p = PerfProfiler::new();
        p.start();
        // Should report close to PROFILE_DURATION since we just started.
        assert!(p.remaining_secs() > 0);
        assert!(p.remaining_secs() <= PROFILE_DURATION.as_secs());
    }

    #[test]
    fn start_twice_resets_frame_samples() {
        let mut p = PerfProfiler::new();
        p.start();
        // Capture one frame.
        p.begin_frame();
        p.begin_sample(PerfLabel::DrawWorld);
        p.end_sample(PerfLabel::DrawWorld);
        p.end_frame();
        assert_eq!(p.frame_samples.len(), 1);

        // Restarting should clear the accumulated samples.
        p.start();
        assert_eq!(p.frame_samples.len(), 0);
    }

    // ── PerfProfiler — no-op when inactive ──────────────────────────────

    #[test]
    fn begin_frame_noop_when_inactive() {
        let mut p = PerfProfiler::new();
        p.begin_frame();
        assert!(p.frame_start.is_none());
    }

    #[test]
    fn begin_sample_noop_when_inactive() {
        let mut p = PerfProfiler::new();
        p.begin_sample(PerfLabel::DrawWorld);
        assert!(p.pending_sample.is_none());
    }

    #[test]
    fn end_sample_noop_when_inactive() {
        let mut p = PerfProfiler::new();
        // Calling end_sample without a paired begin_sample should not panic.
        p.end_sample(PerfLabel::DrawWorld);
        assert!(p.current_times.is_empty());
    }

    #[test]
    fn end_frame_noop_when_inactive() {
        let mut p = PerfProfiler::new();
        p.end_frame();
        assert_eq!(p.frame_samples.len(), 0);
    }

    #[test]
    fn check_expired_noop_when_inactive() {
        let mut p = PerfProfiler::new();
        p.check_expired(); // Must not panic.
        assert!(!p.is_active());
    }

    // ── PerfProfiler — frame / sample lifecycle ──────────────────────────

    #[test]
    fn end_sample_without_begin_sample_is_noop() {
        let mut p = PerfProfiler::new();
        p.start();
        p.begin_frame();
        p.end_sample(PerfLabel::DrawBars); // no matching begin_sample
        assert!(p.current_times.is_empty());
    }

    #[test]
    fn end_frame_without_begin_frame_produces_no_sample() {
        let mut p = PerfProfiler::new();
        p.start();
        p.end_frame(); // no matching begin_frame
        assert_eq!(p.frame_samples.len(), 0);
    }

    #[test]
    fn single_frame_single_sample_is_recorded() {
        let mut p = PerfProfiler::new();
        p.start();
        p.begin_frame();
        p.begin_sample(PerfLabel::DrawWorld);
        p.end_sample(PerfLabel::DrawWorld);
        p.end_frame();

        assert_eq!(p.frame_samples.len(), 1);
        let frame = &p.frame_samples[0];
        assert_eq!(frame.function_times.len(), 1);
        assert_eq!(frame.function_times[0].0, PerfLabel::DrawWorld);
    }

    #[test]
    fn single_frame_multiple_samples_are_all_recorded() {
        let mut p = PerfProfiler::new();
        p.start();
        p.begin_frame();
        for label in PerfLabel::ALL {
            p.begin_sample(label);
            p.end_sample(label);
        }
        p.end_frame();

        assert_eq!(p.frame_samples.len(), 1);
        assert_eq!(
            p.frame_samples[0].function_times.len(),
            PerfLabel::ALL.len()
        );
    }

    #[test]
    fn samples_are_recorded_with_correct_labels() {
        let mut p = PerfProfiler::new();
        p.start();
        p.begin_frame();
        p.begin_sample(PerfLabel::DrawChat);
        p.end_sample(PerfLabel::DrawChat);
        p.begin_sample(PerfLabel::DrawMinimap);
        p.end_sample(PerfLabel::DrawMinimap);
        p.end_frame();

        let times = &p.frame_samples[0].function_times;
        assert_eq!(times[0].0, PerfLabel::DrawChat);
        assert_eq!(times[1].0, PerfLabel::DrawMinimap);
    }

    #[test]
    fn multiple_frames_accumulate() {
        let mut p = PerfProfiler::new();
        p.start();
        for _ in 0..5 {
            p.begin_frame();
            p.begin_sample(PerfLabel::DrawWorld);
            p.end_sample(PerfLabel::DrawWorld);
            p.end_frame();
        }
        assert_eq!(p.frame_samples.len(), 5);
    }

    #[test]
    fn each_frame_sample_has_nonzero_total_frame_time() {
        let mut p = PerfProfiler::new();
        p.start();
        p.begin_frame();
        p.begin_sample(PerfLabel::DrawBars);
        p.end_sample(PerfLabel::DrawBars);
        p.end_frame();

        // total_frame_time is measured from begin_frame to end_frame, so it
        // must be ≥ the single sample inside it.
        let frame = &p.frame_samples[0];
        let sample_dur = frame.function_times[0].1;
        assert!(frame.total_frame_time >= sample_dur);
    }

    #[test]
    fn current_times_cleared_on_begin_frame() {
        let mut p = PerfProfiler::new();
        p.start();
        // First frame — leave current_times dirty by not calling end_frame.
        p.begin_frame();
        p.begin_sample(PerfLabel::DrawWorld);
        p.end_sample(PerfLabel::DrawWorld);
        // Start a new frame — current_times should be cleared.
        p.begin_frame();
        assert!(p.current_times.is_empty());
    }

    // ── PerfProfiler — finish / log_summary ─────────────────────────────

    #[test]
    fn finish_deactivates_profiler() {
        let mut p = PerfProfiler::new();
        p.start();
        assert!(p.is_active());
        p.finish();
        assert!(!p.is_active());
    }

    #[test]
    fn finish_with_no_frames_does_not_panic() {
        let mut p = PerfProfiler::new();
        p.start();
        p.finish(); // log_summary handles n == 0 gracefully
    }

    #[test]
    fn finish_with_frames_does_not_panic() {
        let mut p = PerfProfiler::new();
        p.start();
        for _ in 0..3 {
            p.begin_frame();
            for label in PerfLabel::ALL {
                p.begin_sample(label);
                p.end_sample(label);
            }
            p.end_frame();
        }
        p.finish(); // log_summary must not panic with real frame data
    }

    #[test]
    fn finish_clears_in_flight_state() {
        let mut p = PerfProfiler::new();
        p.start();
        p.begin_frame();
        p.begin_sample(PerfLabel::DrawWorld);
        // Do not call end_sample / end_frame — simulate abrupt finish.
        p.finish();
        assert!(p.frame_start.is_none());
        assert!(p.pending_sample.is_none());
        assert!(p.current_times.is_empty());
    }
}
