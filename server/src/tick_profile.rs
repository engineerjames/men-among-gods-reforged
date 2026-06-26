//! Lightweight thread-local profiler for sub-tick instrumentation.
//!
//! The main game loop is single-threaded, so deeply-nested driver functions
//! (which only have access to `&mut GameState`, not the `Server`) need a way to
//! record how long their sub-sections take without threading an accumulator
//! through every call. This module provides a thread-local set of millisecond
//! accumulators keyed by [`Stage`].
//!
//! Functions add time either by calling [`add`] directly or, preferably, by
//! holding a [`scope`] guard for the duration of a code region. The guard
//! records elapsed time on drop, so it correctly attributes time even when the
//! instrumented function has many early `return` paths.
//!
//! At the end of each tick the `Server` calls [`drain`] to read and reset all
//! accumulators, then folds the totals into its rolling statistics buffers.

use std::cell::Cell;
use std::time::Instant;

/// A distinct instrumented sub-section of the tick.
///
/// The discriminants are used as indices into the thread-local accumulator
/// array, so `Count` must remain the final variant.
#[derive(Clone, Copy, Debug)]
pub enum Stage {
    /// `npc_driver_high` — high-priority NPC AI (maintenance, spell decisions).
    NpcHigh,
    /// `npc_driver_low` — low-priority NPC AI (idle, patrol, wander).
    NpcLow,
    /// `drv_moveto` — movement toward a goto target (includes pathfinding).
    Moveto,
    /// `drv_attack_char` — combat approach/attack driver.
    Attack,
    /// `drv_skill` — skill/spell-cast driver.
    Skill,
    /// `drv_use` — worn-item use driver.
    UseItem,
    /// The misc-action dispatch match (drop/pickup/give/use/bow/wave/turn).
    ///
    /// Note: this region also contains the `NpcLow` call, so true misc-action
    /// time is `Misc - NpcLow`.
    Misc,
    /// `player_driver_med` — player medium-priority driver.
    PlayerMed,
    /// `npc_driver_high`'s 17x17 nearby-item scan loop.
    NhItemscan,
    /// All time spent inside `npc_try_spell` (subset of `NpcHigh`).
    NpcTrySpell,
    /// `act_idle` — per-character idle handling (incl. area notify).
    ActIdle,
    /// `do_area_notify` — area presence broadcast (cross-cutting: appears
    /// inside `ActIdle`, `MoveExec`, and `TurnExec`).
    AreaNotify,
    /// `speedo` — speed-table lookup (called every tick for moving chars).
    Speedo,
    /// `plr_move_by` — movement execution (map/visibility updates).
    MoveExec,
    /// `plr_turn` — turn execution (facing change + notify).
    TurnExec,
    /// `plr_reset_status` — status reset run before the driver in `plr_doact`.
    ResetStatus,
    /// Sentinel marking the number of stages. Must stay last.
    Count,
}

/// Number of instrumented stages.
pub const STAGE_COUNT: usize = Stage::Count as usize;

thread_local! {
    static ACC: [Cell<f32>; STAGE_COUNT] = std::array::from_fn(|_| Cell::new(0.0));
}

/// Add `ms` milliseconds to the accumulator for `stage`.
///
/// # Arguments
///
/// * `stage` - The stage to attribute the time to.
/// * `ms` - Elapsed time in milliseconds.
pub fn add(stage: Stage, ms: f32) {
    ACC.with(|acc| {
        let cell = &acc[stage as usize];
        cell.set(cell.get() + ms);
    });
}

/// RAII guard that attributes its lifetime to a [`Stage`] on drop.
///
/// Created via [`scope`]. Because the elapsed time is recorded in `drop`, the
/// guard correctly accounts for all exit paths (including early returns) of the
/// region it covers.
pub struct ScopeGuard {
    start: Instant,
    stage: Stage,
}

impl Drop for ScopeGuard {
    fn drop(&mut self) {
        add(self.stage, self.start.elapsed().as_secs_f32() * 1000.0);
    }
}

/// Begin timing a region attributed to `stage`.
///
/// Hold the returned guard for the duration of the region; the time is recorded
/// when the guard is dropped.
///
/// # Arguments
///
/// * `stage` - The stage to attribute the region's time to.
///
/// # Returns
///
/// * A [`ScopeGuard`] that records elapsed time on drop.
pub fn scope(stage: Stage) -> ScopeGuard {
    ScopeGuard {
        start: Instant::now(),
        stage,
    }
}

/// Read and reset all stage accumulators.
///
/// # Returns
///
/// * An array of per-stage millisecond totals accumulated since the last
///   `drain`, indexed by `Stage as usize`.
pub fn drain() -> [f32; STAGE_COUNT] {
    ACC.with(|acc| {
        std::array::from_fn(|i| {
            let value = acc[i].get();
            acc[i].set(0.0);
            value
        })
    })
}
