/// Game-state method modules.
///
/// Each submodule adds `impl GameState` blocks that group related gameplay
/// logic (combat, commerce, visibility, etc.). The `GameState` struct itself
/// lives in [`crate::game_state`]; these modules extend it.
pub(crate) mod admin;
pub(crate) mod combat;
pub(crate) mod commands;
pub(crate) mod commerce;
pub(crate) mod communication;
pub(crate) mod death;
pub(crate) mod economy;
pub(crate) mod inventory;
pub(crate) mod logging;
pub(crate) mod player_actions;
pub(crate) mod stats;
pub(crate) mod visibility;

use crate::game_state::GameState;

/// Transitional compatibility shim so that external modules not yet converted
/// to receive `gs: &mut GameState` can continue using `State::with(|s| ...)`
/// and `State::with_mut(|s| ...)`.
///
/// Under the hood these delegate to [`GameState::with`] / [`GameState::with_mut`]
/// which access the global `GameState` singleton.  This shim will be removed
/// once all call sites are migrated (end of Phase 4).
pub struct State;

#[allow(dead_code)]
impl State {
    /// Mutable access to the global `GameState`, matching the old
    /// `State::with(|state| ...)` call pattern.
    ///
    /// NOTE: During the transition, `with` provides `&mut` access because
    /// GameState methods that previously used Repository closures for mutation
    /// now require `&mut self`.  The old `State::with` was effectively mutable
    /// too (via interior mutability), so this preserves the same semantics.
    pub fn with<F, R>(f: F) -> R
    where
        F: FnOnce(&mut GameState) -> R,
    {
        GameState::with_mut(f)
    }

    /// Mutable access to the global `GameState`, matching the old
    /// `State::with_mut(|state| ...)` call pattern.
    pub fn with_mut<F, R>(f: F) -> R
    where
        F: FnOnce(&mut GameState) -> R,
    {
        GameState::with_mut(f)
    }

    // -- Forwarding static methods for unconverted callers --

    /// Forward `State::char_play_sound(...)` → `GameState::char_play_sound(...)`.
    pub fn char_play_sound(character_id: usize, sound: i32, vol: i32, pan: i32) {
        GameState::char_play_sound(character_id, sound, vol, pan);
    }

    /// Forward `State::do_area_sound(...)` → global GameState method.
    pub fn do_area_sound(cn: usize, co: usize, xs: i32, ys: i32, nr: i32) {
        GameState::with_mut(|gs| gs.do_area_sound(cn, co, xs, ys, nr));
    }

    /// Forward `State::check_dlight(...)` → global GameState method.
    pub fn check_dlight(x: usize, y: usize) -> i32 {
        GameState::with_mut(|gs| gs.check_dlight(x, y))
    }

    /// Forward `State::check_dlightm(...)` → global GameState method.
    pub fn check_dlightm(map_index: usize) -> i32 {
        GameState::with_mut(|gs| gs.check_dlightm(map_index))
    }

    /// Forward `State::add_enemy(...)` → global GameState method.
    pub fn add_enemy(cn: usize, co: usize) {
        GameState::with_mut(|gs| gs.add_enemy(cn, co));
    }

    /// Forward `State::remove_enemy(...)` → global GameState method.
    pub fn remove_enemy(co: usize) {
        GameState::with_mut(|gs| gs.remove_enemy(co));
    }
}
