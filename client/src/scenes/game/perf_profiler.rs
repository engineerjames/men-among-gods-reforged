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
