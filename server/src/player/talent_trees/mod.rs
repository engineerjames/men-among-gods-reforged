//! Server-side talent-tree dispatch.
//!
//! Structural metadata (per-class node tables, layout constants,
//! prerequisite resolution) lives in [`core::talent_trees`].  This
//! module layers runtime behaviour on top:
//!
//! * `TalentEffect` — the set of mutations a learned talent can apply
//!   to a `Character`.
//! * Effect helpers (`modify_*`, `grant_skill`, `remove_skill`).
//! * Mutators that update the packed `future1` byte array
//!   (`apply_talent_point`, `reset_talent_points`,
//!   `grant_talent_points`).
//! * A high-level [`learn_talent`] orchestrator that resolves the
//!   player's class, looks up the requested node, validates
//!   prerequisites and cost, debits a point, and dispatches the
//!   node's effect.

mod mercenary;

use core::{
    skills::{Attribute, Skill, SkillIndex},
    string_operations::c_string_to_str,
    talent_trees::{
        TALENT_LAYER_END, TALENT_LAYER_START, TALENT_POINTS_INDEX, TalentId, TalentNodeMeta,
        TalentTreeMeta, available_talent_points, class_for_kindred, find_node, is_talent_spent,
        talents_mut_from_future1, tree_for,
    },
};

use crate::game_state::GameState;

/// The set of mutations a learned talent can apply.
///
/// All variants other than `AttributePercent` are reserved for future
/// effect tables; they are intentionally allowed even when unused so the
/// public surface stays stable as more node types come online.
#[allow(dead_code)]
#[derive(Copy, Clone, Debug)]
pub enum TalentEffect {
    /// Add `amount` to a skill's base value.
    SkillFlat { skill: Skill, amount: u8 },
    /// Add `percent`% of the current base to a skill's base value.
    SkillPercent { skill: Skill, percent: i32 },
    /// Add `amount` to an attribute's base value.
    AttributeFlat { attr: Attribute, amount: u8 },
    /// Add `percent`% of the current base to an attribute's base value.
    AttributePercent { attr: Attribute, percent: i32 },
    /// Grant a previously-unknown skill (set base value to 1).
    GrantSkill { skill: Skill },
}

/// Look up the [`TalentEffect`] associated with `id` for `tree`.
///
/// # Arguments
///
/// * `tree` - The class's tree (used to identify which effects table to consult).
/// * `id` - The node identifier.
///
/// # Returns
///
/// * `Some(effect)` if an entry is registered.
/// * `None` if the class has no effects table or the id is unknown.
fn effect_for(tree: &'static TalentTreeMeta, id: TalentId) -> Option<TalentEffect> {
    let table: &[(TalentId, TalentEffect)] = match tree.class {
        core::traits::Class::Mercenary => mercenary::EFFECTS,
        _ => return None,
    };
    table
        .iter()
        .find(|(entry_id, _)| *entry_id == id)
        .map(|(_, e)| *e)
}

/// Spend a single talent point on one node in a specific layer.
///
/// # Arguments
///
/// * `_cn` - Character slot index (reserved for future per-character effects).
/// * `talents` - Packed talent-tree state (typically `&mut future1`).
/// * `talent_mask` - Single-bit mask of the node being unlocked.
/// * `talent_layer` - Layer index in `TALENT_LAYER_START..TALENT_LAYER_END`.
///
/// # Returns
///
/// * `Ok(())` if the point was spent.
/// * `Err` describing the rejection reason.
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

    talents[TALENT_POINTS_INDEX] -= 1;
    talents[talent_layer] |= talent_mask;

    Ok(())
}

/// Refund every spent talent point back into the unspent-points pool.
///
/// All layer bytes are cleared and the count of previously-set bits is
/// added back into `talents[0]`, saturating at `u8::MAX`.
///
/// **Limitation:** does NOT reverse the stat / skill bonuses that the
/// learned talents previously applied.
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
/// Saturates at `u8::MAX`. Reserved for the future progression hook that
/// awards a talent point on rank-up.
///
/// # Arguments
///
/// * `talents` - Packed talent-tree state to update in place.
/// * `amount` - Number of points to add to `talents[0]`.
#[allow(dead_code)]
pub fn grant_talent_points(talents: &mut [u8; 25], amount: u8) {
    talents[TALENT_POINTS_INDEX] = talents[TALENT_POINTS_INDEX].saturating_add(amount);
}

/// Top-level "spend a point on a talent" entry point.
///
/// Resolves the character's class, finds the requested node, verifies
/// prerequisites and available points, debits the cost, sets the
/// node's bit in `future1`, and dispatches the node's effect.
///
/// # Arguments
///
/// * `gs` - Mutable game state.
/// * `cn` - Character slot index of the player learning the talent.
/// * `node_id` - Stable identifier of the node being learned.
///
/// # Returns
///
/// * `Ok(())` on a successful learn.
/// * `Err(reason)` on any validation failure.
pub fn learn_talent(gs: &mut GameState, cn: usize, node_id: TalentId) -> Result<(), String> {
    let class = class_for_kindred(gs.characters[cn].kindred)
        .ok_or_else(|| "Character has no class set".to_string())?;
    let tree = tree_for(class).ok_or_else(|| format!("No talent tree for class {:?}", class))?;
    let node: &TalentNodeMeta =
        find_node(tree, node_id).ok_or_else(|| format!("Unknown talent id {}", node_id.0))?;

    // MVP: only cost-1 nodes are supported.
    if node.cost != 1 {
        return Err(format!(
            "Talent '{}' has unsupported cost {} (MVP only handles cost == 1)",
            node.name, node.cost,
        ));
    }

    {
        let talents = talents_mut_from_future1(&mut gs.characters[cn].future1);

        for prereq in node.prereqs {
            if !is_talent_spent(talents, prereq.mask, prereq.layer as usize) {
                return Err(format!(
                    "Talent '{}' requires prerequisite (layer={}, mask=0x{:02x})",
                    node.name, prereq.layer, prereq.mask
                ));
            }
        }

        if available_talent_points(talents) < node.cost {
            return Err(format!(
                "Not enough points to learn '{}' (need {}, have {})",
                node.name,
                node.cost,
                available_talent_points(talents)
            ));
        }

        apply_talent_point(cn, talents, node.mask, node.layer as usize)?;
    }

    if let Some(effect) = effect_for(tree, node_id) {
        dispatch_effect(cn, gs, effect)?;
    } else {
        log::warn!(
            "Talent '{}' has no registered effect; bit set but nothing applied",
            node.name
        );
    }

    Ok(())
}

/// Apply a single [`TalentEffect`] to the named character.
///
/// # Arguments
///
/// * `cn` - Character slot index.
/// * `gs` - Mutable game state.
/// * `effect` - The effect to dispatch.
///
/// # Returns
///
/// * `Ok(())` on success.
/// * `Err` if the underlying mutation rejects the request.
fn dispatch_effect(cn: usize, gs: &mut GameState, effect: TalentEffect) -> Result<(), String> {
    match effect {
        TalentEffect::SkillFlat { skill, amount } => {
            modify_skill_by_flat_amount(cn, gs, skill, amount)
        }
        TalentEffect::SkillPercent { skill, percent } => {
            modify_base_skill_by_percentage(cn, gs, skill, percent)
        }
        TalentEffect::AttributeFlat { attr, amount } => {
            modify_attribute_by_flat_amount(cn, gs, attr, amount)
        }
        TalentEffect::AttributePercent { attr, percent } => {
            modify_attribute_by_percentage(cn, gs, attr, percent)
        }
        TalentEffect::GrantSkill { skill } => grant_skill(cn, gs, skill),
    }
}

/// Add a percentage-based bonus to a skill's base value.
///
/// `bonus = round(base * percent / 100)`; the write saturates at `u8::MAX`.
///
/// # Arguments
///
/// * `cn` - Character slot index.
/// * `game_state` - Mutable game state.
/// * `skill` - Skill to modify.
/// * `percentage_bonus` - Percent bonus (e.g. `10` = +10%).
///
/// # Returns
///
/// * `Ok(())` always.
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
/// # Arguments
///
/// * `cn` - Character slot index.
/// * `game_state` - Mutable game state.
/// * `attribute_index` - Attribute to modify.
/// * `percentage_bonus` - Percent bonus (e.g. `10` = +10%).
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

/// Add a flat bonus to a skill's base value (saturating).
///
/// # Arguments
///
/// * `cn` - Character slot index.
/// * `game_state` - Mutable game state.
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

/// Add a flat bonus to an attribute's base value (saturating).
///
/// # Arguments
///
/// * `cn` - Character slot index.
/// * `game_state` - Mutable game state.
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

/// Mark a skill as granted (base value = 1).
///
/// # Arguments
///
/// * `cn` - Character slot index.
/// * `game_state` - Mutable game state.
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::with_test_gs;
    use core::talent_trees::mercenary::ids;
    use core::traits::{Class, KIN_MERCENARY, KIN_TEMPLAR};

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
        let err = apply_talent_point(0, &mut talents, 1, TALENT_LAYER_END).unwrap_err();
        assert!(err.contains("Invalid talent layer"));
    }

    #[test]
    fn apply_talent_point_accepts_highest_valid_layer() {
        let mut talents = empty_talents();
        talents[0] = 1;
        let last = TALENT_LAYER_END - 1;
        apply_talent_point(0, &mut talents, 0b1000_0000, last).expect("spend");
        assert_eq!(talents[last], 0b1000_0000);
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
        talents[TALENT_LAYER_END - 1] = 0b1111_1111;
        reset_talent_points(&mut talents);
        assert_eq!(talents[0], 2 + 1 + 8);
        for byte in &talents[TALENT_LAYER_START..TALENT_LAYER_END] {
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
        for byte in &mut talents[TALENT_LAYER_START..TALENT_LAYER_END] {
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
        });
    }

    // ---- learn_talent ---------------------------------------------------

    fn give_class_and_points(gs: &mut GameState, cn: usize, class_bits: u32, points: u8) {
        gs.characters[cn].kindred = class_bits as i32;
        let t = talents_mut_from_future1(&mut gs.characters[cn].future1);
        t[TALENT_POINTS_INDEX] = points;
    }

    #[test]
    fn learn_talent_succeeds_for_root_node() {
        with_test_gs(|gs| {
            let cn = 1;
            give_class_and_points(gs, cn, KIN_MERCENARY, 1);
            learn_talent(gs, cn, ids::DISTRACT).expect("root learn");
            let t = talents_mut_from_future1(&mut gs.characters[cn].future1);
            assert_eq!(t[TALENT_POINTS_INDEX], 0);
            assert!(is_talent_spent(t, 0b0000_0001, 1));
        });
    }

    #[test]
    fn learn_talent_rejects_missing_prereqs() {
        with_test_gs(|gs| {
            let cn = 1;
            give_class_and_points(gs, cn, KIN_MERCENARY, 1);
            let err = learn_talent(gs, cn, ids::DODGE_BOOST_1).unwrap_err();
            assert!(err.to_lowercase().contains("prereq"), "got: {err}");
            let t = talents_mut_from_future1(&mut gs.characters[cn].future1);
            assert_eq!(t[TALENT_POINTS_INDEX], 1, "point must not be consumed");
        });
    }

    #[test]
    fn learn_talent_succeeds_when_prereqs_met() {
        with_test_gs(|gs| {
            let cn = 1;
            give_class_and_points(gs, cn, KIN_MERCENARY, 3);
            learn_talent(gs, cn, ids::DISTRACT).unwrap();
            learn_talent(gs, cn, ids::PARASITE).unwrap();
            learn_talent(gs, cn, ids::DODGE_BOOST_1).unwrap();
            let t = talents_mut_from_future1(&mut gs.characters[cn].future1);
            assert_eq!(t[TALENT_POINTS_INDEX], 0);
            assert!(is_talent_spent(t, 0b0000_0001, 2));
        });
    }

    #[test]
    fn learn_talent_rejects_when_no_points() {
        with_test_gs(|gs| {
            let cn = 1;
            give_class_and_points(gs, cn, KIN_MERCENARY, 0);
            let err = learn_talent(gs, cn, ids::DISTRACT).unwrap_err();
            assert!(err.to_lowercase().contains("not enough"), "got: {err}");
        });
    }

    #[test]
    fn learn_talent_rejects_already_learned() {
        with_test_gs(|gs| {
            let cn = 1;
            give_class_and_points(gs, cn, KIN_MERCENARY, 2);
            learn_talent(gs, cn, ids::DISTRACT).unwrap();
            let err = learn_talent(gs, cn, ids::DISTRACT).unwrap_err();
            assert!(err.to_lowercase().contains("already learned"), "got: {err}");
            let t = talents_mut_from_future1(&mut gs.characters[cn].future1);
            assert_eq!(t[TALENT_POINTS_INDEX], 1);
        });
    }

    #[test]
    fn learn_talent_rejects_unknown_node_id() {
        with_test_gs(|gs| {
            let cn = 1;
            give_class_and_points(gs, cn, KIN_MERCENARY, 1);
            let err = learn_talent(gs, cn, TalentId(0xFFFF)).unwrap_err();
            assert!(err.to_lowercase().contains("unknown talent"), "got: {err}");
        });
    }

    #[test]
    fn learn_talent_rejects_when_class_has_no_tree() {
        with_test_gs(|gs| {
            let cn = 1;
            give_class_and_points(gs, cn, KIN_TEMPLAR, 5);
            let err = learn_talent(gs, cn, ids::DISTRACT).unwrap_err();
            assert!(err.to_lowercase().contains("no talent tree"), "got: {err}");
        });
    }

    #[test]
    fn learn_talent_rejects_when_kindred_unset() {
        with_test_gs(|gs| {
            let cn = 1;
            gs.characters[cn].kindred = 0;
            let t = talents_mut_from_future1(&mut gs.characters[cn].future1);
            t[TALENT_POINTS_INDEX] = 5;
            let err = learn_talent(gs, cn, ids::DISTRACT).unwrap_err();
            assert!(err.to_lowercase().contains("no class"), "got: {err}");
        });
    }

    #[test]
    fn learn_talent_dispatches_effect() {
        with_test_gs(|gs| {
            let cn = 1;
            give_class_and_points(gs, cn, KIN_MERCENARY, 1);
            // DISTRACT's effect is `AttributePercent { Strength, +10% }`.
            gs.characters[cn].attrib[Attribute::Strength as usize]
                [SkillIndex::BaseValue as usize] = 50;
            learn_talent(gs, cn, ids::DISTRACT).unwrap();
            assert_eq!(
                gs.characters[cn].attrib[Attribute::Strength as usize]
                    [SkillIndex::BaseValue as usize],
                55,
                "expected +10% of 50 (+5 -> 55) after learning Distract"
            );
        });
    }

    #[test]
    fn reset_after_learn_clears_bits_and_refunds_points() {
        with_test_gs(|gs| {
            let cn = 1;
            give_class_and_points(gs, cn, KIN_MERCENARY, 2);
            learn_talent(gs, cn, ids::DISTRACT).unwrap();
            learn_talent(gs, cn, ids::PARASITE).unwrap();
            let t = talents_mut_from_future1(&mut gs.characters[cn].future1);
            assert_eq!(t[TALENT_POINTS_INDEX], 0);
            reset_talent_points(t);
            assert_eq!(t[TALENT_POINTS_INDEX], 2);
            assert_eq!(t[1], 0);
        });
    }

    #[test]
    fn talents_mut_alias_is_byte_identical() {
        with_test_gs(|gs| {
            let cn = 1;
            for v in [0i8, 1, -1, i8::MIN, i8::MAX, 42, -42] {
                gs.characters[cn].future1 = [v; 25];
                let t = talents_mut_from_future1(&mut gs.characters[cn].future1);
                for b in t.iter() {
                    assert_eq!(*b, v as u8);
                }
            }
        });
    }

    // ---- effect_for -----------------------------------------------------

    #[test]
    fn effect_for_returns_some_for_every_mercenary_node() {
        let tree = tree_for(Class::Mercenary).unwrap();
        for node in tree.nodes {
            assert!(
                effect_for(tree, node.id).is_some(),
                "missing effect for mercenary node '{}'",
                node.name,
            );
        }
    }
}
