//! Server-side talent-tree dispatch.
//!
//! Shared talent metadata, effects, byte-array mutation, and derived stat
//! bonus calculation live in [`core::talent_trees`].  This module layers
//! runtime behaviour on top:
//!
//! * Immediate effect helpers for effects that must permanently alter
//!   character data when learned.
//! * A high-level [`learn_talent`] orchestrator that resolves the
//!   player's class, looks up the requested node, validates
//!   prerequisites and cost, and debits a point.

use core::{
    skills::{Skill, SkillIndex},
    string_operations::c_string_to_str,
    talent_trees::{
        TalentEffect, TalentNode, TalentRef, apply_talent_point, available_talent_points,
        find_node, talent_prereqs_met, tree_for,
    },
    types::Class,
};

use crate::game_state::GameState;

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
/// * `slot` - Packed slot of the node being learned.
///
/// # Returns
///
/// * `Ok(())` on a successful learn.
/// * `Err(reason)` on any validation failure.
pub fn learn_talent(gs: &mut GameState, cn: usize, slot: TalentRef) -> Result<(), String> {
    let class = Class::from(gs.characters[cn].kindred);
    let tree = tree_for(class).ok_or_else(|| format!("No talent tree for class {:?}", class))?;
    let node: &TalentNode =
        find_node(tree, slot).ok_or_else(|| format!("Unknown talent slot {:?}", slot))?;

    // MVP: only cost-1 nodes are supported.
    if node.cost != 1 {
        return Err(format!(
            "Talent '{}' has unsupported cost {} (MVP only handles cost == 1)",
            node.name, node.cost,
        ));
    }

    {
        let talents = &mut gs.characters[cn].future1;

        if !talent_prereqs_met(talents, node) {
            return Err(format!(
                "Talent '{}' requires a learned talent in a prerequisite layer",
                node.name
            ));
        }

        if available_talent_points(talents) < node.cost {
            return Err(format!(
                "Not enough points to learn '{}' (need {}, have {})",
                node.name,
                node.cost,
                available_talent_points(talents)
            ));
        }

        apply_talent_point(talents, node.slot)?;
    }

    dispatch_immediate_effect(cn, gs, node.effect)?;

    gs.do_update_char(cn);

    Ok(())
}

/// Apply the learning-time portion of a [`TalentEffect`] to the named character.
///
/// Stat effects are intentionally not written into base attributes or skills;
/// they are recalculated from learned talent bits in
/// [`core::talent_trees::talent_stat_bonuses`]
/// during `really_update_char`. Only effects that must permanently alter the
/// character record at learn time are dispatched here.
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
fn dispatch_immediate_effect(
    cn: usize,
    gs: &mut GameState,
    effect: TalentEffect,
) -> Result<(), String> {
    match effect {
        TalentEffect::GrantSkill { skill } => grant_skill(cn, gs, skill),
        TalentEffect::SkillsFlat { .. }
        | TalentEffect::SkillsPercent { .. }
        | TalentEffect::AttributesFlat { .. }
        | TalentEffect::AttributesPercent { .. }
        | TalentEffect::DodgeChancePercent { .. }
        | TalentEffect::ArmorPercent { .. }
        | TalentEffect::WeaponPercent { .. }
        | TalentEffect::HpManaEndFlat { .. } => Ok(()),
    }
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

    let idx = skill as usize;
    let ch = &mut game_state.characters[cn];
    ch.skill[idx][SkillIndex::BaseValue as usize] = 1;
    // Talent-granted skills aren't present in any character template, so seed
    // their MaxValue and RaiseDifficulty here. Without this the per-skill UI
    // refuses to spend skill points on them (RaiseDifficulty == 0 means
    // "not raisable", MaxValue == 0 caps progression at the base value).
    if ch.skill[idx][SkillIndex::MaxValue as usize] == 0 {
        ch.skill[idx][SkillIndex::MaxValue as usize] = 100;
    }
    if ch.skill[idx][SkillIndex::RaiseDifficulty as usize] == 0 {
        ch.skill[idx][SkillIndex::RaiseDifficulty as usize] = 5;
    }

    log::info!(
        "Granted new skill {:?} to character {}",
        skill,
        c_string_to_str(&ch.name)
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::with_test_gs;
    use core::constants::CharacterFlags;
    use core::skills::Attribute;
    use core::talent_trees::{
        TALENT_LAYER_END, TALENT_LAYER_START, TALENT_POINTS_INDEX, grant_talent_points,
        is_talent_spent, reset_talent_points, talent_stat_bonuses,
    };
    use core::traits::{Class, KIN_MERCENARY};

    fn empty_talents() -> [u8; 25] {
        [0; 25]
    }

    fn slot(layer: u8, mask: u8) -> TalentRef {
        TalentRef { layer, mask }
    }

    fn mercenary_slot(name: &str) -> TalentRef {
        tree_for(Class::Mercenary)
            .unwrap()
            .nodes
            .iter()
            .find(|node| node.name == name)
            .unwrap_or_else(|| panic!("missing mercenary talent '{name}'"))
            .slot
    }

    // ---- apply_talent_point ---------------------------------------------

    #[test]
    fn apply_talent_point_spends_one_point_and_sets_bit() {
        let mut talents = empty_talents();
        talents[0] = 3;
        apply_talent_point(&mut talents, slot(1, 0b0000_0010)).expect("spend");
        assert_eq!(talents[0], 2);
        assert_eq!(talents[1], 0b0000_0010);
    }

    #[test]
    fn apply_talent_point_rejects_layer_zero() {
        let mut talents = empty_talents();
        talents[0] = 1;
        let err = apply_talent_point(&mut talents, slot(0, 1)).unwrap_err();
        assert!(err.contains("Invalid talent layer"));
        assert_eq!(talents[0], 1);
    }

    #[test]
    fn apply_talent_point_rejects_layer_out_of_range() {
        let mut talents = empty_talents();
        talents[0] = 1;
        let err = apply_talent_point(&mut talents, slot(TALENT_LAYER_END as u8, 1)).unwrap_err();
        assert!(err.contains("Invalid talent layer"));
    }

    #[test]
    fn apply_talent_point_accepts_highest_valid_layer() {
        let mut talents = empty_talents();
        talents[0] = 1;
        let last = TALENT_LAYER_END - 1;
        apply_talent_point(&mut talents, slot(last as u8, 0b1000_0000)).expect("spend");
        assert_eq!(talents[last], 0b1000_0000);
    }

    #[test]
    fn apply_talent_point_rejects_multi_bit_mask() {
        let mut talents = empty_talents();
        talents[0] = 5;
        let err = apply_talent_point(&mut talents, slot(1, 0b0000_0011)).unwrap_err();
        assert!(err.contains("exactly one bit"));
        assert_eq!(talents[0], 5);
        assert_eq!(talents[1], 0);
    }

    #[test]
    fn apply_talent_point_rejects_zero_mask() {
        let mut talents = empty_talents();
        talents[0] = 1;
        let err = apply_talent_point(&mut talents, slot(1, 0)).unwrap_err();
        assert!(err.contains("exactly one bit"));
    }

    #[test]
    fn apply_talent_point_rejects_already_learned_talent() {
        let mut talents = empty_talents();
        talents[0] = 2;
        apply_talent_point(&mut talents, slot(2, 0b0000_0100)).expect("first");
        let err = apply_talent_point(&mut talents, slot(2, 0b0000_0100)).unwrap_err();
        assert!(err.contains("already learned"));
        assert_eq!(talents[0], 1, "rejected spend must not consume a point");
    }

    #[test]
    fn apply_talent_point_rejects_second_pick_in_same_layer() {
        let mut talents = empty_talents();
        talents[0] = 2;
        apply_talent_point(&mut talents, slot(1, 0b0000_0001)).expect("first");
        let err = apply_talent_point(&mut talents, slot(1, 0b0000_0010)).unwrap_err();
        assert!(err.contains("already learned in this layer"));
        assert_eq!(talents[0], 1, "rejected spend must not consume a point");
    }

    #[test]
    fn apply_talent_point_rejects_when_no_points_available() {
        let mut talents = empty_talents();
        let err = apply_talent_point(&mut talents, slot(1, 1)).unwrap_err();
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

    // ---- effect helpers (skill / attribute bonuses) ---------------------

    #[test]
    fn talent_stat_bonuses_reads_learned_bits_without_mutating_base() {
        with_test_gs(|gs| {
            let cn = 1;
            gs.characters[cn].kindred = KIN_MERCENARY as i32;
            gs.characters[cn].attrib[Attribute::Strength as usize]
                [SkillIndex::BaseValue as usize] = 50;
            gs.characters[cn].future1[10] |= 0b0000_0001;

            let bonuses = talent_stat_bonuses(
                gs.characters[cn].kindred,
                &gs.characters[cn].future1,
                &gs.characters[cn].attrib,
                &gs.characters[cn].skill,
            );

            assert_eq!(bonuses.attrib[Attribute::Strength as usize], 5);
            assert_eq!(
                gs.characters[cn].attrib[Attribute::Strength as usize]
                    [SkillIndex::BaseValue as usize],
                50,
                "derived talent bonuses must not rewrite saved base stats"
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
        gs.characters[cn].future1[TALENT_POINTS_INDEX] = points;
    }

    #[test]
    fn learn_talent_succeeds_for_root_node() {
        with_test_gs(|gs| {
            let cn = 1;
            give_class_and_points(gs, cn, KIN_MERCENARY, 1);
            learn_talent(gs, cn, mercenary_slot("Distract")).expect("root learn");
            let t = &gs.characters[cn].future1;
            assert_eq!(t[TALENT_POINTS_INDEX], 0);
            assert!(is_talent_spent(t, 0b0000_0001, 1));
        });
    }

    #[test]
    fn learn_talent_rejects_missing_prereqs() {
        with_test_gs(|gs| {
            let cn = 1;
            give_class_and_points(gs, cn, KIN_MERCENARY, 1);
            let err = learn_talent(gs, cn, mercenary_slot("Dodge Boost I")).unwrap_err();
            assert!(err.to_lowercase().contains("prereq"), "got: {err}");
            let t = &gs.characters[cn].future1;
            assert_eq!(t[TALENT_POINTS_INDEX], 1, "point must not be consumed");
        });
    }

    #[test]
    fn learn_talent_succeeds_when_prereqs_met() {
        with_test_gs(|gs| {
            let cn = 1;
            give_class_and_points(gs, cn, KIN_MERCENARY, 2);
            learn_talent(gs, cn, mercenary_slot("Distract")).unwrap();
            learn_talent(gs, cn, mercenary_slot("Dodge Boost I")).unwrap();
            let t = &gs.characters[cn].future1;
            assert_eq!(t[TALENT_POINTS_INDEX], 0);
            assert!(is_talent_spent(t, 0b0000_0001, 2));
        });
    }

    #[test]
    fn learn_talent_rejects_second_pick_in_same_layer() {
        with_test_gs(|gs| {
            let cn = 1;
            give_class_and_points(gs, cn, KIN_MERCENARY, 2);
            learn_talent(gs, cn, mercenary_slot("Distract")).unwrap();
            let err = learn_talent(gs, cn, mercenary_slot("Parasite")).unwrap_err();
            assert!(err.contains("already learned in this layer"), "got: {err}");
            let t = &gs.characters[cn].future1;
            assert_eq!(t[TALENT_POINTS_INDEX], 1);
        });
    }

    #[test]
    fn learn_talent_rejects_when_no_points() {
        with_test_gs(|gs| {
            let cn = 1;
            give_class_and_points(gs, cn, KIN_MERCENARY, 0);
            let err = learn_talent(gs, cn, mercenary_slot("Distract")).unwrap_err();
            assert!(err.to_lowercase().contains("not enough"), "got: {err}");
        });
    }

    #[test]
    fn learn_talent_rejects_already_learned() {
        with_test_gs(|gs| {
            let cn = 1;
            give_class_and_points(gs, cn, KIN_MERCENARY, 2);
            learn_talent(gs, cn, mercenary_slot("Distract")).unwrap();
            let err = learn_talent(gs, cn, mercenary_slot("Distract")).unwrap_err();
            assert!(err.to_lowercase().contains("already learned"), "got: {err}");
            let t = &gs.characters[cn].future1;
            assert_eq!(t[TALENT_POINTS_INDEX], 1);
        });
    }

    #[test]
    fn learn_talent_rejects_unknown_slot() {
        with_test_gs(|gs| {
            let cn = 1;
            give_class_and_points(gs, cn, KIN_MERCENARY, 1);
            let err = learn_talent(gs, cn, slot(23, 0b1000_0000)).unwrap_err();
            assert!(err.to_lowercase().contains("unknown talent"), "got: {err}");
        });
    }

    #[test]
    fn learn_talent_recomputes_effect_without_mutating_base() {
        with_test_gs(|gs| {
            let cn = 1;
            give_class_and_points(gs, cn, KIN_MERCENARY, 1);
            // STRENGTH_BOOST_1's effect is `AttributesPercent { [Strength], [+10%] }`.
            // Manually seed the layer 1-9 prereq chain so the root-most learn is layer 10.
            for layer in 1..=9 {
                gs.characters[cn].future1[layer] |= 0b0000_0001;
            }
            gs.characters[cn].attrib[Attribute::Strength as usize]
                [SkillIndex::BaseValue as usize] = 50;
            learn_talent(gs, cn, mercenary_slot("Strength Boost I")).unwrap();
            assert_eq!(
                gs.characters[cn].attrib[Attribute::Strength as usize]
                    [SkillIndex::BaseValue as usize],
                50,
                "learning a stat talent must leave saved base value unchanged"
            );

            gs.really_update_char(cn);
            assert_eq!(
                gs.characters[cn].attrib[Attribute::Strength as usize]
                    [SkillIndex::TotalValue as usize],
                55,
                "expected +10% of 50 (+5 -> 55) after recompute"
            );
        });
    }

    #[test]
    fn learned_talent_bonus_survives_restart_style_recompute() {
        with_test_gs(|gs| {
            let cn = 1;
            gs.characters[cn].kindred = KIN_MERCENARY as i32;
            gs.characters[cn].attrib[Attribute::Strength as usize]
                [SkillIndex::BaseValue as usize] = 50;
            gs.characters[cn].future1[10] |= 0b0000_0001;

            gs.really_update_char(cn);

            assert_eq!(
                gs.characters[cn].attrib[Attribute::Strength as usize]
                    [SkillIndex::TotalValue as usize],
                55,
                "persisted talent bits must be enough to restore derived bonuses"
            );
        });
    }

    #[test]
    fn attribute_percent_bonus_recalculates_after_base_raise() {
        with_test_gs(|gs| {
            let cn = 1;
            gs.characters[cn].kindred = KIN_MERCENARY as i32;
            gs.characters[cn].future1[10] |= 0b0000_0001;

            gs.characters[cn].attrib[Attribute::Strength as usize]
                [SkillIndex::BaseValue as usize] = 55;
            gs.really_update_char(cn);
            assert_eq!(
                gs.characters[cn].attrib[Attribute::Strength as usize]
                    [SkillIndex::TotalValue as usize],
                61
            );

            gs.characters[cn].attrib[Attribute::Strength as usize]
                [SkillIndex::BaseValue as usize] = 56;
            gs.really_update_char(cn);
            assert_eq!(
                gs.characters[cn].attrib[Attribute::Strength as usize]
                    [SkillIndex::TotalValue as usize],
                62
            );
        });
    }

    #[test]
    fn learn_talent_marks_character_for_stat_recompute() {
        with_test_gs(|gs| {
            let cn = 1;
            give_class_and_points(gs, cn, KIN_MERCENARY, 1);
            gs.characters[cn].flags &= !CharacterFlags::Update.bits();

            learn_talent(gs, cn, mercenary_slot("Distract")).unwrap();

            assert_ne!(gs.characters[cn].flags & CharacterFlags::Update.bits(), 0);
        });
    }

    #[test]
    fn reset_after_learn_clears_bits_and_refunds_points() {
        with_test_gs(|gs| {
            let cn = 1;
            give_class_and_points(gs, cn, KIN_MERCENARY, 2);
            learn_talent(gs, cn, mercenary_slot("Distract")).unwrap();
            learn_talent(gs, cn, mercenary_slot("Dodge Boost I")).unwrap();
            let t = &mut gs.characters[cn].future1;
            assert_eq!(t[TALENT_POINTS_INDEX], 0);
            reset_talent_points(t);
            assert_eq!(t[TALENT_POINTS_INDEX], 2);
            assert_eq!(t[1], 0);
            assert_eq!(t[2], 0);
        });
    }

    #[test]
    fn core_node_effects_have_distinct_class_flavors() {
        assert_effect(
            tree_for(Class::Templar).unwrap().nodes[0].effect,
            Attribute::Braveness,
            10,
        );
        assert_effect(
            tree_for(Class::Harakim).unwrap().nodes[0].effect,
            Attribute::Intuition,
            10,
        );
        assert_effect(
            tree_for(Class::SeyanDu).unwrap().nodes[17].effect,
            Attribute::Intuition,
            18,
        );
    }

    fn assert_effect(effect: TalentEffect, expected_attr: Attribute, expected_percent: i32) {
        match effect {
            TalentEffect::AttributesPercent { attrs, percents } => {
                assert_eq!(attrs.len(), 1, "expected single-attribute talent");
                assert_eq!(percents.len(), 1, "expected single-percent talent");
                assert_eq!(attrs[0], expected_attr);
                assert_eq!(percents[0], expected_percent);
            }
            other => panic!("expected AttributesPercent, got {other:?}"),
        }
    }
}
