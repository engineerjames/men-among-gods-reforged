use std::time::Duration;

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

#[derive(Default)]
pub(crate) struct GameplayPerfAccum {
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
    pub fn reset_counters(&mut self) {
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
    pub runs: u32,
    pub total: Duration,
    pub entities: u32,
    pub glyph_spawned: u32,
    pub glyph_despawned: u32,
}

impl BitmapTextPerfAccum {
    pub fn reset_counters(&mut self) {
        self.runs = 0;
        self.total = Duration::ZERO;
        self.entities = 0;
        self.glyph_spawned = 0;
        self.glyph_despawned = 0;
    }
}
