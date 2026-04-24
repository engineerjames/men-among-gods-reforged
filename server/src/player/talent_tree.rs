//! Talent-tree storage and point-spending logic.
//!
//! Talent-tree state is packed into the 25-byte `future1` slot on every
//! character so it survives persistence without requiring a schema
//! migration.  The layout is:
//!
//! * `future1[0]` — unspent talent points available to the player
//!   (0–255).
//! * `future1[1..24]` — one byte per talent rank/layer.  Each of the 8
//!   bits in a byte represents a single talent node in that layer; a
//!   `1` bit means "point spent", `0` means "not spent".
//!
//! ```text
//! Player has 0 points to spend <-- future1[0] = 0
//! [x] - [ ] <-- future1[1] = 0b00000010 (1 point spent in the first talent layer)
//!  |     |
//! [ ]   [x] <-- future1[2] = 0b00000001 (1 point spent in the second talent layer)
//!  |     |
//! [ ] - [x] <-- future1[3] = 0b00000001 (1 point spent in the third talent layer)
//!  |     |
//! [ ]   [ ] <-- future1[4] = 0b00000000 (0 points spent in the fourth talent layer)
//! ```
//!
//! This layout is deliberately generous (24 layers × 8 nodes = 192
//! potential talents) so the tree can grow without touching persistence.
//! Adding nodes to the "right" of an already-populated tree may require
//! care if it changes bit assignments, but new layers can always be
//! filled in without migration.

// Talent-tree scaffolding: public entry points and private effect helpers
// are defined here but not yet wired into the command or tick loops.
// Allow unused-item warnings module-wide until the UI and dispatcher
// layers start calling these.
#![allow(dead_code)]

use core::{
    skills::{Attribute, Skill, SkillIndex},
    string_operations::c_string_to_str,
};

use crate::game_state::GameState;

/// Index of the unspent-points byte in the packed talent-tree array.
pub const TALENT_POINTS_INDEX: usize = 0;

/// First byte index that represents a talent layer (inclusive).
pub const TALENT_LAYER_START: usize = 1;

/// One past the last valid talent-layer byte index (exclusive).
pub const TALENT_LAYER_END: usize = 24;

/// Spend a single talent point on one node in a specific layer.
///
/// The node is identified by `talent_mask`, a single bit (1, 2, 4, 8,
/// 16, 32, 64, or 128) selecting one of the 8 nodes in `talent_layer`.
/// Masks with multiple bits set are rejected so callers cannot spend
/// one point across several nodes.
///
/// # Arguments
///
/// * `_cn` - Character slot index (reserved for future per-character
///   effects such as unlocking skills).  Unused today.
/// * `talents` - Packed talent-tree state (typically `&mut future1`).
/// * `talent_mask` - Single-bit mask of the node being unlocked.
/// * `talent_layer` - Layer/rank index in `talents`, in
///   `TALENT_LAYER_START..TALENT_LAYER_END`.
///
/// # Returns
///
/// * `Ok(())` if the point was spent.
/// * `Err` describing the reason a spend was rejected (invalid layer,
///   invalid mask, node already owned, or no points available).
pub fn apply_talent_point(
    _cn: usize,
    talents: &mut [u8; 25],
    talent_mask: u8,
    talent_layer: usize,
) -> Result<(), String> {
    if !(TALENT_LAYER_START..TALENT_LAYER_END).contains(&talent_layer) {
        return Err("Invalid talent layer".to_string());
    }

    if talent_mask == 0 || talent_mask.count_ones() != 1 {
        return Err("Talent mask must have exactly one bit set".to_string());
    }

    if talents[talent_layer] & talent_mask != 0 {
        return Err("Talent already learned".to_string());
    }

    if talents[TALENT_POINTS_INDEX] < 1 {
        return Err("Not enough points to spend".to_string());
    }

    // Dispatch function to update player state based on the talent chosen
    // This is a placeholder for future implementation.
    // update_player_state(_cn, talent_layer, talent_mask);

    talents[TALENT_POINTS_INDEX] -= 1;
    talents[talent_layer] |= talent_mask;

    Ok(())
}

/// Refund every spent talent point back into the unspent-points pool.
///
/// All layer bytes are cleared to zero and the count of previously-set
/// bits is added back into `talents[0]`, saturating at `u8::MAX` so the
/// refund cannot wrap.
///
/// # Arguments
///
/// * `talents` - Packed talent-tree state to reset in place.
pub fn reset_talent_points(talents: &mut [u8; 25]) {
    let mut refunded_points: u32 = 0;
    for byte in talents
        .iter_mut()
        .take(TALENT_LAYER_END)
        .skip(TALENT_LAYER_START)
    {
        refunded_points += byte.count_ones();
        *byte = 0;
    }
    let refund = u8::try_from(refunded_points).unwrap_or(u8::MAX);
    talents[TALENT_POINTS_INDEX] = talents[TALENT_POINTS_INDEX].saturating_add(refund);
}

/// Grant additional unspent talent points to the player's pool.
///
/// Saturates at `u8::MAX` so repeated grants cannot wrap.
///
/// # Arguments
///
/// * `talents` - Packed talent-tree state to update in place.
/// * `amount` - Number of points to add to `talents[0]`.
pub fn grant_talent_points(talents: &mut [u8; 25], amount: u8) {
    talents[TALENT_POINTS_INDEX] = talents[TALENT_POINTS_INDEX].saturating_add(amount);
}

/// Return the number of talent points the player has available to spend.
///
/// # Arguments
///
/// * `talents` - Packed talent-tree state.
///
/// # Returns
///
/// * The value of `talents[0]`.
pub fn available_talent_points(talents: &[u8; 25]) -> u8 {
    talents[TALENT_POINTS_INDEX]
}

/// Count the total number of talent points that have been spent across
/// every layer.
///
/// # Arguments
///
/// * `talents` - Packed talent-tree state.
///
/// # Returns
///
/// * Sum of set bits across `talents[1..25]`.
pub fn total_points_spent(talents: &[u8; 25]) -> u32 {
    talents[TALENT_LAYER_START..TALENT_LAYER_END]
        .iter()
        .map(|b| b.count_ones())
        .sum()
}

/// Check whether a specific talent node has been unlocked.
///
/// # Arguments
///
/// * `talents` - Packed talent-tree state.
/// * `talent_mask` - Single-bit mask of the node being queried.
/// * `talent_layer` - Layer/rank index in
///   `TALENT_LAYER_START..TALENT_LAYER_END`.
///
/// # Returns
///
/// * `true` if the layer is valid, `talent_mask` is non-zero, and all
///   of its bits are set in the target layer.
/// * `false` otherwise.
pub fn is_talent_spent(talents: &[u8; 25], talent_mask: u8, talent_layer: usize) -> bool {
    if !(TALENT_LAYER_START..TALENT_LAYER_END).contains(&talent_layer) {
        return false;
    }
    talent_mask != 0 && talents[talent_layer] & talent_mask == talent_mask
}

/// Add a percentage-based bonus to a skill's base value.
///
/// The bonus is computed as `base * percentage_bonus / 100`, rounded to
/// the nearest integer, and added on top of the existing base.  The
/// result saturates at `u8::MAX` so the write cannot wrap.
///
/// # Arguments
///
/// * `cn` - Character slot index.
/// * `game_state` - Mutable game state containing the character.
/// * `skill` - Skill to modify.
/// * `percentage_bonus` - Percent bonus to apply (e.g. `10` = +10%).
///
/// # Returns
///
/// * `Ok(())` always; the `Result` return type is kept so future
///   validation (e.g. rejecting invalid skill slots) can be added
///   without breaking callers.
fn modify_base_skill_by_percentage(
    cn: usize,
    game_state: &mut GameState,
    skill: Skill,
    percentage_bonus: i32,
) -> Result<(), String> {
    let skill_base =
        game_state.characters[cn].skill[skill as usize][SkillIndex::BaseValue as usize];

    let bonus_amount = (skill_base as f32 * (percentage_bonus as f32 / 100.0)).round() as i32;
    let bonus_u8 = bonus_amount.clamp(0, u8::MAX as i32) as u8;
    let slot = &mut game_state.characters[cn].skill[skill as usize][SkillIndex::BaseValue as usize];
    *slot = slot.saturating_add(bonus_u8);

    log::info!(
        "Applied talent bonus: +{}% to {:?} ({} points) for character {}",
        percentage_bonus,
        skill,
        bonus_u8,
        c_string_to_str(&game_state.characters[cn].name)
    );

    Ok(())
}

/// Add a percentage-based bonus to an attribute's base value.
///
/// Mirrors [`modify_base_skill_by_percentage`] but targets the 5-wide
/// attribute array.  The write saturates at `u8::MAX`.
///
/// # Arguments
///
/// * `cn` - Character slot index.
/// * `game_state` - Mutable game state containing the character.
/// * `attribute_index` - Attribute to modify.
/// * `percentage_bonus` - Percent bonus to apply (e.g. `10` = +10%).
///
/// # Returns
///
/// * `Ok(())` always.
fn modify_attribute_by_percentage(
    cn: usize,
    game_state: &mut GameState,
    attribute_index: Attribute,
    percentage_bonus: i32,
) -> Result<(), String> {
    let attribute_base =
        game_state.characters[cn].attrib[attribute_index as usize][SkillIndex::BaseValue as usize];

    let bonus_amount = (attribute_base as f32 * (percentage_bonus as f32 / 100.0)).round() as i32;
    let bonus_u8 = bonus_amount.clamp(0, u8::MAX as i32) as u8;
    let slot = &mut game_state.characters[cn].attrib[attribute_index as usize]
        [SkillIndex::BaseValue as usize];
    *slot = slot.saturating_add(bonus_u8);

    log::info!(
        "Applied talent bonus: +{}% to attribute {} ({} points) for character {}",
        percentage_bonus,
        attribute_index as usize,
        bonus_u8,
        c_string_to_str(&game_state.characters[cn].name)
    );

    Ok(())
}

/// Add a flat bonus to a skill's base value.
///
/// The write saturates at `u8::MAX`.
///
/// # Arguments
///
/// * `cn` - Character slot index.
/// * `game_state` - Mutable game state containing the character.
/// * `skill` - Skill to modify.
/// * `flat_bonus` - Amount to add to the base value.
///
/// # Returns
///
/// * `Ok(())` always.
fn modify_skill_by_flat_amount(
    cn: usize,
    game_state: &mut GameState,
    skill: Skill,
    flat_bonus: u8,
) -> Result<(), String> {
    let slot = &mut game_state.characters[cn].skill[skill as usize][SkillIndex::BaseValue as usize];
    *slot = slot.saturating_add(flat_bonus);

    log::info!(
        "Applied talent bonus: +{} to {:?} for character {}",
        flat_bonus,
        skill,
        c_string_to_str(&game_state.characters[cn].name)
    );

    Ok(())
}

/// Add a flat bonus to an attribute's base value.
///
/// The write saturates at `u8::MAX`.
///
/// # Arguments
///
/// * `cn` - Character slot index.
/// * `game_state` - Mutable game state containing the character.
/// * `attribute_index` - Attribute to modify.
/// * `flat_bonus` - Amount to add to the base value.
///
/// # Returns
///
/// * `Ok(())` always.
fn modify_attribute_by_flat_amount(
    cn: usize,
    game_state: &mut GameState,
    attribute_index: Attribute,
    flat_bonus: u8,
) -> Result<(), String> {
    let slot = &mut game_state.characters[cn].attrib[attribute_index as usize]
        [SkillIndex::BaseValue as usize];
    *slot = slot.saturating_add(flat_bonus);

    log::info!(
        "Applied talent bonus: +{} to attribute {} for character {}",
        flat_bonus,
        attribute_index as usize,
        c_string_to_str(&game_state.characters[cn].name)
    );

    Ok(())
}

/// Mark a skill as granted (base value = 1) if the character does not
/// already have it.
///
/// # Arguments
///
/// * `cn` - Character slot index.
/// * `game_state` - Mutable game state containing the character.
/// * `skill` - Skill to grant.
///
/// # Returns
///
/// * `Ok(())` on a successful grant.
/// * `Err` when the character already has a non-zero base in `skill`.
fn grant_skill(cn: usize, game_state: &mut GameState, skill: Skill) -> Result<(), String> {
    let skill_base =
        game_state.characters[cn].skill[skill as usize][SkillIndex::BaseValue as usize];

    if skill_base > 0 {
        return Err(format!(
            "Character {} already has skill {:?}",
            c_string_to_str(&game_state.characters[cn].name),
            skill
        ));
    }

    game_state.characters[cn].skill[skill as usize][SkillIndex::BaseValue as usize] = 1;

    log::info!(
        "Granted new skill {:?} to character {}",
        skill,
        c_string_to_str(&game_state.characters[cn].name)
    );

    Ok(())
}

/// Clear a skill's base value back to zero if the character currently
/// has it.
///
/// # Arguments
///
/// * `cn` - Character slot index.
/// * `game_state` - Mutable game state containing the character.
/// * `skill` - Skill to remove.
///
/// # Returns
///
/// * `Ok(())` on a successful removal.
/// * `Err` when the character's base value for `skill` is already zero.
fn remove_skill(cn: usize, game_state: &mut GameState, skill: Skill) -> Result<(), String> {
    let skill_base =
        game_state.characters[cn].skill[skill as usize][SkillIndex::BaseValue as usize];

    if skill_base == 0 {
        return Err(format!(
            "Character {} does not have skill {:?} to remove",
            c_string_to_str(&game_state.characters[cn].name),
            skill
        ));
    }

    game_state.characters[cn].skill[skill as usize][SkillIndex::BaseValue as usize] = 0;

    log::info!(
        "Removed skill {:?} from character {}",
        skill,
        c_string_to_str(&game_state.characters[cn].name)
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::with_test_gs;

    fn empty_talents() -> [u8; 25] {
        [0; 25]
    }

    // ---- apply_talent_point ---------------------------------------------

    #[test]
    fn apply_talent_point_spends_one_point_and_sets_bit() {
        let mut talents = empty_talents();
        talents[0] = 3;
        apply_talent_point(0, &mut talents, 0b0000_0010, 1).expect("spend");
        assert_eq!(talents[0], 2);
        assert_eq!(talents[1], 0b0000_0010);
    }

    #[test]
    fn apply_talent_point_rejects_layer_zero() {
        let mut talents = empty_talents();
        talents[0] = 1;
        let err = apply_talent_point(0, &mut talents, 1, 0).unwrap_err();
        assert!(err.contains("Invalid talent layer"));
        assert_eq!(talents[0], 1);
    }

    #[test]
    fn apply_talent_point_rejects_layer_out_of_range() {
        let mut talents = empty_talents();
        talents[0] = 1;
        let err = apply_talent_point(0, &mut talents, 1, 25).unwrap_err();
        assert!(err.contains("Invalid talent layer"));
    }

    #[test]
    fn apply_talent_point_accepts_highest_valid_layer() {
        let mut talents = empty_talents();
        talents[0] = 1;
        apply_talent_point(0, &mut talents, 0b1000_0000, 24).expect("spend");
        assert_eq!(talents[24], 0b1000_0000);
    }

    #[test]
    fn apply_talent_point_rejects_multi_bit_mask() {
        let mut talents = empty_talents();
        talents[0] = 5;
        let err = apply_talent_point(0, &mut talents, 0b0000_0011, 1).unwrap_err();
        assert!(err.contains("exactly one bit"));
        assert_eq!(talents[0], 5);
        assert_eq!(talents[1], 0);
    }

    #[test]
    fn apply_talent_point_rejects_zero_mask() {
        let mut talents = empty_talents();
        talents[0] = 1;
        let err = apply_talent_point(0, &mut talents, 0, 1).unwrap_err();
        assert!(err.contains("exactly one bit"));
    }

    #[test]
    fn apply_talent_point_rejects_already_learned_talent() {
        let mut talents = empty_talents();
        talents[0] = 2;
        apply_talent_point(0, &mut talents, 0b0000_0100, 2).expect("first");
        let err = apply_talent_point(0, &mut talents, 0b0000_0100, 2).unwrap_err();
        assert!(err.contains("already learned"));
        assert_eq!(talents[0], 1, "rejected spend must not consume a point");
    }

    #[test]
    fn apply_talent_point_rejects_when_no_points_available() {
        let mut talents = empty_talents();
        let err = apply_talent_point(0, &mut talents, 1, 1).unwrap_err();
        assert!(err.contains("Not enough points"));
    }

    // ---- reset_talent_points --------------------------------------------

    #[test]
    fn reset_talent_points_refunds_all_spent_bits() {
        let mut talents = empty_talents();
        talents[1] = 0b0000_0011;
        talents[3] = 0b1000_0000;
        talents[24] = 0b1111_1111;
        reset_talent_points(&mut talents);
        assert_eq!(talents[0], 2 + 1 + 8);
        for byte in &talents[1..25] {
            assert_eq!(*byte, 0);
        }
    }

    #[test]
    fn reset_talent_points_preserves_existing_pool() {
        let mut talents = empty_talents();
        talents[0] = 5;
        talents[2] = 0b0000_1111;
        reset_talent_points(&mut talents);
        assert_eq!(talents[0], 5 + 4);
    }

    #[test]
    fn reset_talent_points_saturates_at_u8_max() {
        let mut talents = empty_talents();
        talents[0] = 250;
        // 24 layers * 8 bits = 192 refunded points; total would be 442.
        for byte in &mut talents[1..25] {
            *byte = 0xFF;
        }
        reset_talent_points(&mut talents);
        assert_eq!(talents[0], u8::MAX);
    }

    #[test]
    fn reset_talent_points_on_empty_tree_is_noop() {
        let mut talents = empty_talents();
        talents[0] = 7;
        reset_talent_points(&mut talents);
        assert_eq!(talents[0], 7);
    }

    // ---- grant_talent_points --------------------------------------------

    #[test]
    fn grant_talent_points_adds_to_pool() {
        let mut talents = empty_talents();
        grant_talent_points(&mut talents, 3);
        grant_talent_points(&mut talents, 4);
        assert_eq!(talents[0], 7);
    }

    #[test]
    fn grant_talent_points_saturates() {
        let mut talents = empty_talents();
        talents[0] = 250;
        grant_talent_points(&mut talents, 100);
        assert_eq!(talents[0], u8::MAX);
    }

    // ---- available_talent_points / total_points_spent ------------------

    #[test]
    fn available_talent_points_reads_slot_zero() {
        let mut talents = empty_talents();
        talents[0] = 42;
        assert_eq!(available_talent_points(&talents), 42);
    }

    #[test]
    fn total_points_spent_counts_bits_across_layers() {
        let mut talents = empty_talents();
        talents[0] = 99; // must be ignored
        talents[1] = 0b0000_0101;
        talents[5] = 0b1111_0000;
        talents[24] = 0b0000_0001;
        assert_eq!(total_points_spent(&talents), 2 + 4 + 1);
    }

    #[test]
    fn total_points_spent_on_empty_tree_is_zero() {
        let talents = empty_talents();
        assert_eq!(total_points_spent(&talents), 0);
    }

    // ---- is_talent_spent ------------------------------------------------

    #[test]
    fn is_talent_spent_reports_set_bits() {
        let mut talents = empty_talents();
        talents[3] = 0b0001_0000;
        assert!(is_talent_spent(&talents, 0b0001_0000, 3));
        assert!(!is_talent_spent(&talents, 0b0010_0000, 3));
    }

    #[test]
    fn is_talent_spent_returns_false_for_invalid_layer() {
        let mut talents = empty_talents();
        talents[1] = 0xFF;
        assert!(!is_talent_spent(&talents, 1, 0));
        assert!(!is_talent_spent(&talents, 1, 25));
    }

    #[test]
    fn is_talent_spent_returns_false_for_zero_mask() {
        let mut talents = empty_talents();
        talents[1] = 0xFF;
        assert!(!is_talent_spent(&talents, 0, 1));
    }

    // ---- effect helpers (skill / attribute mutations) -------------------

    #[test]
    fn modify_base_skill_by_percentage_rounds_and_adds() {
        with_test_gs(|gs| {
            let cn = 1;
            gs.characters[cn].skill[Skill::Sword as usize][SkillIndex::BaseValue as usize] = 50;
            modify_base_skill_by_percentage(cn, gs, Skill::Sword, 10).unwrap();
            assert_eq!(
                gs.characters[cn].skill[Skill::Sword as usize][SkillIndex::BaseValue as usize],
                55
            );
        });
    }

    #[test]
    fn modify_base_skill_by_percentage_saturates() {
        with_test_gs(|gs| {
            let cn = 1;
            gs.characters[cn].skill[Skill::Sword as usize][SkillIndex::BaseValue as usize] = 200;
            modify_base_skill_by_percentage(cn, gs, Skill::Sword, 100).unwrap();
            assert_eq!(
                gs.characters[cn].skill[Skill::Sword as usize][SkillIndex::BaseValue as usize],
                u8::MAX
            );
        });
    }

    #[test]
    fn modify_attribute_by_percentage_rounds_and_adds() {
        with_test_gs(|gs| {
            let cn = 1;
            gs.characters[cn].attrib[Attribute::Strength as usize]
                [SkillIndex::BaseValue as usize] = 40;
            modify_attribute_by_percentage(cn, gs, Attribute::Strength, 25).unwrap();
            assert_eq!(
                gs.characters[cn].attrib[Attribute::Strength as usize]
                    [SkillIndex::BaseValue as usize],
                50
            );
        });
    }

    #[test]
    fn modify_skill_by_flat_amount_adds_and_saturates() {
        with_test_gs(|gs| {
            let cn = 1;
            gs.characters[cn].skill[Skill::Axe as usize][SkillIndex::BaseValue as usize] = 250;
            modify_skill_by_flat_amount(cn, gs, Skill::Axe, 10).unwrap();
            assert_eq!(
                gs.characters[cn].skill[Skill::Axe as usize][SkillIndex::BaseValue as usize],
                u8::MAX
            );
        });
    }

    #[test]
    fn modify_attribute_by_flat_amount_adds_and_saturates() {
        with_test_gs(|gs| {
            let cn = 1;
            gs.characters[cn].attrib[Attribute::Agility as usize][SkillIndex::BaseValue as usize] =
                250;
            modify_attribute_by_flat_amount(cn, gs, Attribute::Agility, 10).unwrap();
            assert_eq!(
                gs.characters[cn].attrib[Attribute::Agility as usize]
                    [SkillIndex::BaseValue as usize],
                u8::MAX
            );
        });
    }

    // ---- grant_skill / remove_skill ------------------------------------

    #[test]
    fn grant_skill_sets_base_to_one_when_unset() {
        with_test_gs(|gs| {
            let cn = 1;
            grant_skill(cn, gs, Skill::Stealth).unwrap();
            assert_eq!(
                gs.characters[cn].skill[Skill::Stealth as usize][SkillIndex::BaseValue as usize],
                1
            );
        });
    }

    #[test]
    fn grant_skill_fails_when_already_owned() {
        with_test_gs(|gs| {
            let cn = 1;
            gs.characters[cn].skill[Skill::Stealth as usize][SkillIndex::BaseValue as usize] = 5;
            let err = grant_skill(cn, gs, Skill::Stealth).unwrap_err();
            assert!(err.contains("already has skill"));
            assert_eq!(
                gs.characters[cn].skill[Skill::Stealth as usize][SkillIndex::BaseValue as usize],
                5,
                "base value must not be clobbered"
            );
        });
    }

    #[test]
    fn remove_skill_clears_base_value() {
        with_test_gs(|gs| {
            let cn = 1;
            gs.characters[cn].skill[Skill::Perception as usize][SkillIndex::BaseValue as usize] = 7;
            remove_skill(cn, gs, Skill::Perception).unwrap();
            assert_eq!(
                gs.characters[cn].skill[Skill::Perception as usize][SkillIndex::BaseValue as usize],
                0
            );
        });
    }

    #[test]
    fn remove_skill_fails_when_absent() {
        with_test_gs(|gs| {
            let cn = 1;
            let err = remove_skill(cn, gs, Skill::Perception).unwrap_err();
            assert!(err.contains("does not have skill"));
        });
    }

    // ---- integration: spend then reset --------------------------------

    #[test]
    fn spend_then_reset_returns_to_initial_pool() {
        let mut talents = empty_talents();
        talents[0] = 5;
        apply_talent_point(0, &mut talents, 0b0000_0001, 1).unwrap();
        apply_talent_point(0, &mut talents, 0b0000_0010, 1).unwrap();
        apply_talent_point(0, &mut talents, 0b1000_0000, 7).unwrap();
        assert_eq!(talents[0], 2);
        assert_eq!(total_points_spent(&talents), 3);

        reset_talent_points(&mut talents);
        assert_eq!(talents[0], 5);
        assert_eq!(total_points_spent(&talents), 0);
    }
}
