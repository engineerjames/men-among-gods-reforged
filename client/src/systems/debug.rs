use std::{
    sync::OnceLock,
    time::{Duration, Instant},
};

use bevy::prelude::*;

use crate::GameState;

/// Log all gameplay state transitions for debugging.
pub fn run_on_any_transition(mut transitions: MessageReader<StateTransitionEvent<GameState>>) {
    for ev in transitions.read() {
        log::info!("State Transition from {:?} to {:?}", ev.exited, ev.entered);
    }
}

#[derive(Resource, Clone, Copy, Debug)]
pub(crate) struct GameplayDebugSettings {
    /// Enables tile flag overlay entities (MoveBlock/Indoors/etc).
    /// These are useful for debugging but expensive if spawned for every tile.
    pub(crate) tile_flag_overlays: bool,
}

impl Default for GameplayDebugSettings {
    /// Reads debug settings from environment variables.
    fn default() -> Self {
        // Set `MAG_DEBUG_TILE_OVERLAYS=1` to enable.
        let enabled = env_flag("MAG_DEBUG_TILE_OVERLAYS");

        Self {
            tile_flag_overlays: enabled,
        }
    }
}

#[inline]
/// Reads an environment variable as a boolean feature flag.
///
/// Accepts common false-y values like "0", "false", and "no" (case-insensitive).
pub fn env_flag(name: &str) -> bool {
    std::env::var(name)
        .ok()
        .map(|v| {
            let v = v.trim().to_ascii_lowercase();
            !(v.is_empty() || v == "0" || v == "false" || v == "no")
        })
        .unwrap_or(false)
}

#[inline]
/// Returns whether gameplay rendering profiling is enabled.
///
/// This uses a `OnceLock` to read and cache the `MAG_PROFILE_RENDERING` env var once.
pub fn profile_rendering_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| env_flag("MAG_PROFILE_RENDERING"))
}

#[derive(Default)]
pub(crate) struct GameplayPerfAccum {
    pub last_report: Option<Instant>,
    pub frames: u32,

    pub total: Duration,
    pub engine_tick: Duration,
    pub send_opt: Duration,
    pub minimap: Duration,
    pub world_shadows: Duration,
    pub world_tiles: Duration,
    pub world_overlays: Duration,
    pub ui: Duration,
}

impl GameplayPerfAccum {
    /// Emits periodic gameplay performance logs and resets the counters.
    ///
    /// Only active in debug builds when `MAG_PROFILE_RENDERING` is enabled.
    pub fn maybe_report_and_reset(&mut self) {
        if !cfg!(debug_assertions) || !profile_rendering_enabled() {
            return;
        }

        let now = Instant::now();
        let Some(last) = self.last_report else {
            self.last_report = Some(now);
            return;
        };

        if now.duration_since(last) < Duration::from_secs(2) {
            return;
        }

        let frames = self.frames.max(1) as f64;
        let to_ms = |d: Duration| d.as_secs_f64() * 1000.0;

        info!(
            "perf gameplay: total={:.2}ms/f (engine={:.2} send_opt={:.2} minimap={:.2} shadows={:.2} tiles={:.2} ovl={:.2} ui={:.2}) over {} frames",
            to_ms(self.total) / frames,
            to_ms(self.engine_tick) / frames,
            to_ms(self.send_opt) / frames,
            to_ms(self.minimap) / frames,
            to_ms(self.world_shadows) / frames,
            to_ms(self.world_tiles) / frames,
            to_ms(self.world_overlays) / frames,
            to_ms(self.ui) / frames,
            self.frames,
        );

        self.last_report = Some(now);
        self.frames = 0;
        self.total = Duration::ZERO;
        self.engine_tick = Duration::ZERO;
        self.send_opt = Duration::ZERO;
        self.minimap = Duration::ZERO;
        self.world_shadows = Duration::ZERO;
        self.world_tiles = Duration::ZERO;
        self.world_overlays = Duration::ZERO;
        self.ui = Duration::ZERO;
    }
}

#[derive(Default)]
pub(crate) struct BitmapTextPerfAccum {
    pub last_report: Option<Instant>,
    pub runs: u32,
    pub total: Duration,
    pub entities: u32,
    pub glyph_spawned: u32,
    pub glyph_despawned: u32,
}

impl BitmapTextPerfAccum {
    /// Emits periodic bitmap-text performance logs and resets the counters.
    ///
    /// Only active in debug builds when `MAG_PROFILE_RENDERING` is enabled.
    pub fn maybe_report_and_reset(&mut self) {
        if !cfg!(debug_assertions) || !profile_rendering_enabled() {
            return;
        }

        let now = Instant::now();
        let Some(last) = self.last_report else {
            self.last_report = Some(now);
            return;
        };

        if now.duration_since(last) < Duration::from_secs(2) {
            return;
        }

        let runs = self.runs.max(1) as f64;
        let ms_per_run = (self.total.as_secs_f64() * 1000.0) / runs;

        info!(
            "perf bitmap_text: {:.3}ms/run (runs={} entities={} spawned={} despawned={})",
            ms_per_run, self.runs, self.entities, self.glyph_spawned, self.glyph_despawned,
        );

        self.last_report = Some(now);
        self.runs = 0;
        self.total = Duration::ZERO;
        self.entities = 0;
        self.glyph_spawned = 0;
        self.glyph_despawned = 0;
    }
}
