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

/// Transitional compatibility alias.
///
/// Legacy callsites using `State::...` resolve directly to `GameState::...`.
pub type State = GameState;
