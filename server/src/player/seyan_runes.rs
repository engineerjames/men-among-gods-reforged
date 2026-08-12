//! Server-side Seyan'Du rune selection.
//!
//! Runes are a Seyan'Du-only loadout mechanic, separate from the
//! point-based talent tree: all four runes are always unlocked, only one is
//! active at a time, and switching has a fixed cooldown. Active rune and
//! swap-cooldown state are persisted in the character's spare `future3`
//! slots (see the doc comment on `Character::future3`). On-hit proc dispatch
//! lives in `server/src/state/combat.rs`.

use core::seyan_runes::{RUNE_SWAP_COOLDOWN_TICKS, SeyanRune};
use core::types::{Character, Class};

use crate::game_state::GameState;

/// `future3` index storing the active rune (`0..=3`, see [`core::seyan_runes::SeyanRune`]).
const RUNE_ACTIVE_IDX: usize = 3;

/// `future3` index storing the ticker value when the next rune swap is allowed.
const RUNE_COOLDOWN_IDX: usize = 4;

/// Reads the currently active rune from a character's persisted state.
///
/// Does not check class or player flags — callers that need the "only
/// Seyan'Du players have an active rune" gate should check that themselves
/// (see `server/src/state/combat.rs::active_seyan_rune`).
///
/// # Arguments
///
/// * `character` - Character to read rune state from.
///
/// # Returns
///
/// * `Some(rune)` when `future3[RUNE_ACTIVE_IDX]` holds a valid rune index.
pub fn active_rune(character: &Character) -> Option<SeyanRune> {
    let idx = character.future3[RUNE_ACTIVE_IDX].clamp(0, 3) as u8;
    SeyanRune::from_index(idx)
}

/// Reads the active rune index and remaining swap cooldown for a character.
///
/// # Arguments
///
/// * `gs` - Active game state.
/// * `cn` - Character slot index.
///
/// # Returns
///
/// * `(active_rune, cooldown_remaining_ticks)`. `cooldown_remaining_ticks` is
///   `0` when a swap is currently allowed.
pub fn rune_state(gs: &GameState, cn: usize) -> (u8, u16) {
    let active = gs.characters[cn].future3[RUNE_ACTIVE_IDX].clamp(0, 3) as u8;
    let ready_at = gs.characters[cn].future3[RUNE_COOLDOWN_IDX];
    let remaining = (ready_at - gs.globals.ticker).max(0);
    (active, remaining.min(i32::from(u16::MAX)) as u16)
}

/// Sets the caller's active Seyan'Du rune, enforcing the class gate and swap
/// cooldown.
///
/// # Arguments
///
/// * `gs` - Mutable game state.
/// * `cn` - Character slot index of the player switching runes.
/// * `rune_idx` - Requested rune slot index.
///
/// # Returns
///
/// * `Ok(())` when the rune was activated.
/// * `Err(reason)` when the class, index, or cooldown check fails.
pub fn set_active_rune(gs: &mut GameState, cn: usize, rune_idx: u8) -> Result<(), String> {
    let class = Class::from(gs.characters[cn].kindred);
    if class != Class::SeyanDu {
        return Err(format!("Class {:?} has no rune loadout", class));
    }

    if core::seyan_runes::SeyanRune::from_index(rune_idx).is_none() {
        return Err(format!("Unknown rune index {rune_idx}"));
    }

    let (_, remaining) = rune_state(gs, cn);
    if remaining > 0 {
        return Err(format!(
            "Rune swap still on cooldown for {remaining} more ticks"
        ));
    }

    gs.characters[cn].future3[RUNE_ACTIVE_IDX] = i32::from(rune_idx);
    gs.characters[cn].future3[RUNE_COOLDOWN_IDX] = gs.globals.ticker + RUNE_SWAP_COOLDOWN_TICKS;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::with_test_gs;
    use core::constants::USE_ACTIVE;

    fn seed_seyan_du(gs: &mut GameState, cn: usize) {
        gs.characters[cn] = core::types::Character::default();
        gs.characters[cn].used = USE_ACTIVE;
        gs.characters[cn].flags = core::constants::CharacterFlags::Player.bits();
        gs.characters[cn].kindred = core::traits::KIN_SEYAN_DU as i32;
    }

    #[test]
    fn set_active_rune_rejects_wrong_class() {
        with_test_gs(|gs| {
            seed_seyan_du(gs, 1);
            gs.characters[1].kindred = core::traits::KIN_MERCENARY as i32;
            assert!(set_active_rune(gs, 1, 0).is_err());
        });
    }

    #[test]
    fn set_active_rune_rejects_out_of_range_index() {
        with_test_gs(|gs| {
            seed_seyan_du(gs, 1);
            assert!(set_active_rune(gs, 1, 4).is_err());
        });
    }

    #[test]
    fn first_pick_is_immediate() {
        with_test_gs(|gs| {
            seed_seyan_du(gs, 1);
            assert_eq!(rune_state(gs, 1), (0, 0));
            assert!(set_active_rune(gs, 1, 2).is_ok());
            assert_eq!(rune_state(gs, 1).0, 2);
        });
    }

    #[test]
    fn swap_is_gated_by_cooldown() {
        with_test_gs(|gs| {
            seed_seyan_du(gs, 1);
            assert!(set_active_rune(gs, 1, 1).is_ok());
            let err = set_active_rune(gs, 1, 2).unwrap_err();
            assert!(err.contains("cooldown"));
            assert_eq!(rune_state(gs, 1).0, 1);
        });
    }

    #[test]
    fn swap_succeeds_after_cooldown_elapses() {
        with_test_gs(|gs| {
            seed_seyan_du(gs, 1);
            assert!(set_active_rune(gs, 1, 1).is_ok());
            gs.globals.ticker += RUNE_SWAP_COOLDOWN_TICKS;
            assert!(set_active_rune(gs, 1, 2).is_ok());
            assert_eq!(rune_state(gs, 1).0, 2);
        });
    }
}
