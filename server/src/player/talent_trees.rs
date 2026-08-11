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
    skills::{MAX_SKILLS, Skill, SkillIndex},
    string_operations::c_string_to_str,
    talent_trees::{
        TalentEffect, TalentNode, TalentRef, TalentSkillProfile, apply_talent_point,
        available_talent_points, find_node, is_talent_spent, reset_talent_points,
        talent_prereqs_met, talent_skill_ownership, tree_for,
    },
    types::{Character, Class},
};

use crate::game_state::GameState;
use crate::points;

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

    let mut updated_talents = gs.characters[cn].future1;
    {
        let talents = &mut updated_talents;

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
    gs.characters[cn].future1 = updated_talents;

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
        TalentEffect::GrantSkill { skill, profile } => grant_skill(cn, gs, skill, profile),
        TalentEffect::ReplaceSkill { from, to, profile } => {
            replace_skill(cn, gs, from, to, profile)
        }
        TalentEffect::Composite { effects } => {
            for effect in effects {
                dispatch_immediate_effect(cn, gs, *effect)?;
            }
            Ok(())
        }
        TalentEffect::Passive
        | TalentEffect::SkillsFlat { .. }
        | TalentEffect::SkillsPercent { .. }
        | TalentEffect::AttributesFlat { .. }
        | TalentEffect::AttributesPercent { .. }
        | TalentEffect::DodgeChancePercent { .. }
        | TalentEffect::ArmorPercent { .. }
        | TalentEffect::WeaponPercent { .. }
        | TalentEffect::HpManaEndFlat { .. }
        | TalentEffect::PrimaryHitProc { .. }
        | TalentEffect::RegenPercent { .. }
        | TalentEffect::SpellPenetrationPercent { .. }
        | TalentEffect::CriticalStrike { .. } => Ok(()),
    }
}

/// Grant a skill, raising it to at least the profile's base value.
///
/// Existing higher investment is preserved, so a node may grant a skill the
/// character already knows without the rollback hazard of failing mid-learn.
/// The profile's `MaxValue` and `RaiseDifficulty` are only written when the
/// row does not already carry one, so a live character's tuned row is never
/// lowered by a later profile edit.
///
/// # Arguments
///
/// * `cn` - Character slot index.
/// * `game_state` - Mutable game state.
/// * `skill` - Skill to grant or raise.
/// * `profile` - Base floor, max value and raise difficulty declared by the
///   granting talent node.
///
/// # Returns
///
/// * `Ok(())` — granting is always accepted.
fn grant_skill(
    cn: usize,
    game_state: &mut GameState,
    skill: Skill,
    profile: TalentSkillProfile,
) -> Result<(), String> {
    let idx = skill as usize;
    let ch = &mut game_state.characters[cn];

    let base_idx = SkillIndex::BaseValue as usize;
    if ch.skill[idx][base_idx] > 0 {
        log::warn!(
            "Character {} already has skill {:?} at base {}; talent grant keeps the higher value",
            c_string_to_str(&ch.name),
            skill,
            ch.skill[idx][base_idx],
        );
    }

    if ch.skill[idx][base_idx] < profile.base {
        ch.skill[idx][base_idx] = profile.base;
    }

    // Talent-granted skills aren't present in any character template, so seed
    // their MaxValue and RaiseDifficulty here. Without this the per-skill UI
    // refuses to spend skill points on them (RaiseDifficulty == 0 means
    // "not raisable", MaxValue == 0 caps progression at the base value).
    if ch.skill[idx][SkillIndex::MaxValue as usize] == 0 {
        ch.skill[idx][SkillIndex::MaxValue as usize] = profile.max_value;
    }
    if ch.skill[idx][SkillIndex::RaiseDifficulty as usize] == 0 {
        ch.skill[idx][SkillIndex::RaiseDifficulty as usize] = profile.raise_difficulty;
    }

    log::info!(
        "Granted skill {:?} at base {} (max {}, difficulty {}) to character {}",
        skill,
        profile.base,
        profile.max_value,
        profile.raise_difficulty,
        c_string_to_str(&ch.name)
    );

    Ok(())
}

/// Re-stamp a character template's skill rows onto a character without
/// destroying talent-owned skill rows.
///
/// Character templates only describe the skills a race natively knows.
/// Talent-granted skills live in reserved slots that no template declares,
/// so a naive template copy zeroes their `MaxValue` and `RaiseDifficulty`,
/// leaving the player holding a skill they can never raise. Likewise, a
/// learned [`TalentEffect::ReplaceSkill`] clears the replaced row, which a
/// naive copy happily re-adds so the player ends up with both halves of the
/// pair.
///
/// This helper applies the template row-by-row while honouring
/// [`core::talent_trees::talent_skill_ownership`]:
///
/// * Rows a learned talent replaced away stay cleared.
/// * Rows a learned talent granted keep their talent-seeded base, max value
///   and raise difficulty; template values are only adopted when the
///   template actually declares the skill.
/// * Every other row is stamped from the template as before.
///
/// # Arguments
///
/// * `character` - Character whose skill rows are being re-stamped.
/// * `template_skills` - Skill rows of the character template to apply.
/// * `clamp_to_template_max` - When `true`, a base value above the
///   template's max value is reduced to it and the spent experience is
///   refunded into `points`. Talent-owned rows are never clamped.
pub fn apply_template_skills(
    character: &mut Character,
    template_skills: &[[u16; SkillIndex::MaxIndex as usize]; MAX_SKILLS],
    clamp_to_template_max: bool,
) {
    let ownership = talent_skill_ownership(Class::from(character.kindred), &character.future1);
    let name = character.get_name().to_owned();

    let base_idx = SkillIndex::BaseValue as usize;
    let preset_idx = SkillIndex::PresetModifier as usize;
    let max_idx = SkillIndex::MaxValue as usize;
    let diff_idx = SkillIndex::RaiseDifficulty as usize;

    for (n, template_row) in template_skills.iter().enumerate() {
        if ownership.is_replaced(n) {
            character.skill[n] = [0; SkillIndex::MaxIndex as usize];
            continue;
        }

        if ownership.is_granted(n) {
            let profile = ownership.profile_for(n).unwrap_or_default();

            if character.skill[n][base_idx] < profile.base {
                character.skill[n][base_idx] = profile.base;
            }
            if template_row[preset_idx] != 0 {
                character.skill[n][preset_idx] = template_row[preset_idx];
            }
            // Never lower a live character's tuned row, but a template's own
            // declared row must not override a talent-granted skill's
            // authored cap — only the character's prior value and the
            // granting talent's current profile compete here.
            character.skill[n][max_idx] = character.skill[n][max_idx].max(profile.max_value);
            if character.skill[n][diff_idx] == 0 {
                character.skill[n][diff_idx] = if template_row[diff_idx] != 0 {
                    template_row[diff_idx]
                } else {
                    profile.raise_difficulty
                };
            }
            continue;
        }

        if character.skill[n][base_idx] == 0 && template_row[base_idx] != 0 {
            character.skill[n][base_idx] = template_row[base_idx];
            log::info!("added {} to {}", core::skills::get_skill_name(n), name);
        }

        if clamp_to_template_max && template_row[max_idx] < character.skill[n][base_idx] {
            let refund = crate::populate::skillcost(
                i32::from(character.skill[n][base_idx]),
                i32::from(character.skill[n][diff_idx]),
                i32::from(template_row[max_idx]),
            );
            log::info!(
                "reduced {} on {} from {} to {}, added {} exp",
                core::skills::get_skill_name(n),
                name,
                character.skill[n][base_idx],
                template_row[max_idx],
                refund
            );
            character.skill[n][base_idx] = template_row[max_idx];
            character.points += refund;
        }

        character.skill[n][preset_idx] = template_row[preset_idx];
        character.skill[n][max_idx] = template_row[max_idx];
        character.skill[n][diff_idx] = template_row[diff_idx];
    }
}

/// Reset every learned talent back to an unspent state.
///
/// Reverses the full effect of each learned node, not just the spent points:
///
/// * Talent-granted skills are removed and the experience spent raising them
///   above their granted base is refunded into the character's spendable
///   point pool (see [`ungrant_skill`]).
/// * Stat, attribute, dodge/armor/weapon, HP/mana/endurance, passive, and
///   primary-hit-proc effects are recomputed from the learned talent bits in
///   `really_update_char`, so clearing the bits and re-running
///   [`GameState::do_update_char`] reverses them automatically.
/// * The packed talent bits are cleared and the spent talent points refunded
///   via [`core::talent_trees::reset_talent_points`].
/// * Runtime proc bookkeeping (`talent_primary_hit_counts`) is cleared.
///
/// # Arguments
///
/// * `gs` - Mutable game state.
/// * `cn` - Character slot index whose talents are being reset.
pub fn reset_talents(gs: &mut GameState, cn: usize) {
    let class = Class::from(gs.characters[cn].kindred);

    if let Some(tree) = tree_for(class) {
        for node in tree.nodes {
            if is_talent_spent(
                &gs.characters[cn].future1,
                node.slot.mask,
                node.slot.layer as usize,
            ) {
                dispatch_undo_effect(cn, gs, node.effect);
            }
        }
    }

    reset_talent_points(&mut gs.characters[cn].future1);

    if let Some(count) = gs.talent_primary_hit_counts.get_mut(cn) {
        *count = 0;
    }

    gs.do_update_char(cn);
}

/// Erase every trace of talent progression from a character.
///
/// This is the "start your character over" counterpart to
/// [`reset_talents`], used when a character is rebuilt from scratch on a
/// new race template (see [`crate::god::God::racechange`]), most notably
/// when a player becomes a Seyan'Du. Unlike [`reset_talents`], **nothing is
/// paid back**:
///
/// * Talent-granted and talent-replaced skill rows are cleared outright and
///   the experience spent raising them is *not* refunded into `points`.
/// * Skills that a learned [`TalentEffect::ReplaceSkill`] superseded are
///   *not* restored; the new template is the sole source of skill rows.
/// * The packed talent bytes — learned bits *and* the unspent-point pool —
///   are zeroed rather than refunded, so the character re-earns talent
///   points from rank 0 like a brand new character.
/// * Runtime proc bookkeeping (`talent_primary_hit_counts`) is cleared.
///
/// Must be called **before** the character's `kindred` is changed so the
/// old class's talent tree is consulted when locating talent-owned skills.
///
/// # Arguments
///
/// * `gs` - Mutable game state.
/// * `cn` - Character slot index whose talent progression is being erased.
pub fn wipe_talents(gs: &mut GameState, cn: usize) {
    let class = Class::from(gs.characters[cn].kindred);
    let ownership = talent_skill_ownership(class, &gs.characters[cn].future1);

    for idx in 0..MAX_SKILLS {
        if ownership.is_granted(idx) || ownership.is_replaced(idx) {
            gs.characters[cn].skill[idx] = [0; SkillIndex::MaxIndex as usize];
        }
    }

    gs.characters[cn].future1 = [0; 25];

    if let Some(count) = gs.talent_primary_hit_counts.get_mut(cn) {
        *count = 0;
    }

    gs.characters[cn].set_do_update_flags();
}

/// Reverse the learning-time portion of a [`TalentEffect`].
///
/// Mirror of [`dispatch_immediate_effect`]: only effects that permanently
/// mutate the character record at learn time need an explicit undo. Effects
/// that are recomputed from learned talent bits during `really_update_char`
/// are reversed simply by clearing those bits, so they are intentional no-ops
/// here.
///
/// # Arguments
///
/// * `cn` - Character slot index.
/// * `gs` - Mutable game state.
/// * `effect` - The effect to reverse.
fn dispatch_undo_effect(cn: usize, gs: &mut GameState, effect: TalentEffect) {
    match effect {
        TalentEffect::GrantSkill { skill, profile } => ungrant_skill(cn, gs, skill, profile),
        TalentEffect::ReplaceSkill { from, to, .. } => {
            restore_replaced_skill(cn, gs, to, from);
        }
        TalentEffect::Composite { effects } => {
            for effect in effects {
                dispatch_undo_effect(cn, gs, *effect);
            }
        }
        TalentEffect::Passive
        | TalentEffect::SkillsFlat { .. }
        | TalentEffect::SkillsPercent { .. }
        | TalentEffect::AttributesFlat { .. }
        | TalentEffect::AttributesPercent { .. }
        | TalentEffect::DodgeChancePercent { .. }
        | TalentEffect::ArmorPercent { .. }
        | TalentEffect::WeaponPercent { .. }
        | TalentEffect::HpManaEndFlat { .. }
        | TalentEffect::PrimaryHitProc { .. }
        | TalentEffect::RegenPercent { .. }
        | TalentEffect::SpellPenetrationPercent { .. }
        | TalentEffect::CriticalStrike { .. } => {}
    }
}

/// Replace one skill row with another while preserving trainable investment.
///
/// This is used for talents that turn an existing skill into a class-specific
/// replacement. The replacement's `MaxValue` and `RaiseDifficulty` come from
/// `profile` and are deliberately *not* inherited from `from`, so a
/// replacement ability can be tuned independently of the skill it supersedes.
/// Base value investment does carry over: the replacement starts at whichever
/// is higher of `from`'s base and `profile.base`.
///
/// Computed values are cleared and recalculated by the normal character update
/// path after the talent mutation completes.
///
/// # Arguments
///
/// * `cn` - Character slot index.
/// * `game_state` - Mutable game state.
/// * `from` - Skill row to remove while the talent state is active.
/// * `to` - Skill row to populate from `from`.
/// * `profile` - Base floor, max value and raise difficulty for `to`.
///
/// # Returns
///
/// * `Ok(())` when the replacement succeeds.
/// * `Err` when `to` already has a base value and would be overwritten.
fn replace_skill(
    cn: usize,
    game_state: &mut GameState,
    from: Skill,
    to: Skill,
    profile: TalentSkillProfile,
) -> Result<(), String> {
    let from_idx = from as usize;
    let to_idx = to as usize;
    if from_idx == to_idx {
        return Ok(());
    }

    let base_idx = SkillIndex::BaseValue as usize;
    let preset_idx = SkillIndex::PresetModifier as usize;
    let max_idx = SkillIndex::MaxValue as usize;
    let diff_idx = SkillIndex::RaiseDifficulty as usize;
    let dynamic_idx = SkillIndex::DynamicModifier as usize;
    let total_idx = SkillIndex::TotalValue as usize;

    let ch = &mut game_state.characters[cn];
    if ch.skill[to_idx][base_idx] > 0 {
        return Err(format!(
            "Character {} already has replacement skill {:?}",
            c_string_to_str(&ch.name),
            to
        ));
    }

    let from_base = ch.skill[from_idx][base_idx];
    let from_preset = ch.skill[from_idx][preset_idx];

    ch.skill[to_idx][base_idx] = from_base.max(profile.base);
    ch.skill[to_idx][preset_idx] = from_preset;
    ch.skill[to_idx][max_idx] = profile.max_value;
    ch.skill[to_idx][diff_idx] = profile.raise_difficulty;
    ch.skill[to_idx][dynamic_idx] = 0;
    ch.skill[to_idx][total_idx] = 0;

    ch.skill[from_idx] = [0; SkillIndex::MaxIndex as usize];
    ch.set_do_update_flags();

    log::info!(
        "Replaced skill {:?} with {:?} (max {}, difficulty {}) for character {}",
        from,
        to,
        profile.max_value,
        profile.raise_difficulty,
        c_string_to_str(&ch.name)
    );

    Ok(())
}

/// Reverse a talent skill replacement, restoring the superseded skill row.
///
/// This is not simply [`replace_skill`] with swapped arguments. The forward
/// replacement overwrites the replacement row's `MaxValue` and
/// `RaiseDifficulty` with the talent's profile, so the superseded skill's own
/// tuning is no longer recoverable from the character record. Those values are
/// instead re-read from the character's template, which is where a
/// non-talent skill's tuning normally comes from. When the template does not
/// declare the skill (a race that never natively knows it), the replacement
/// row's current values are reused as a last resort so the restored skill is
/// never left unraisable.
///
/// Accumulated investment carries back: the restored row keeps the base value
/// the player trained on the replacement, clamped to the template's max value
/// when the template declares one.
///
/// # Arguments
///
/// * `cn` - Character slot index.
/// * `game_state` - Mutable game state.
/// * `to` - Replacement skill row being removed.
/// * `from` - Superseded skill row being restored.
fn restore_replaced_skill(cn: usize, game_state: &mut GameState, to: Skill, from: Skill) {
    let to_idx = to as usize;
    let from_idx = from as usize;
    if to_idx == from_idx {
        return;
    }

    let base_idx = SkillIndex::BaseValue as usize;
    let preset_idx = SkillIndex::PresetModifier as usize;
    let max_idx = SkillIndex::MaxValue as usize;
    let diff_idx = SkillIndex::RaiseDifficulty as usize;

    let template_row = game_state
        .character_templates
        .get(game_state.characters[cn].temp as usize)
        .map(|template| template.skill[from_idx]);

    let ch = &mut game_state.characters[cn];
    let to_row = ch.skill[to_idx];

    let restored_max = match template_row {
        Some(row) if row[max_idx] != 0 => row[max_idx],
        _ => to_row[max_idx],
    };
    let restored_diff = match template_row {
        Some(row) if row[diff_idx] != 0 => row[diff_idx],
        _ => to_row[diff_idx],
    };
    let restored_preset = match template_row {
        Some(row) if row[preset_idx] != 0 => row[preset_idx],
        _ => to_row[preset_idx],
    };

    let mut restored_base = to_row[base_idx].max(1);
    if restored_max != 0 {
        restored_base = restored_base.min(restored_max);
    }

    ch.skill[from_idx] = [0; SkillIndex::MaxIndex as usize];
    ch.skill[from_idx][base_idx] = restored_base;
    ch.skill[from_idx][preset_idx] = restored_preset;
    ch.skill[from_idx][max_idx] = restored_max;
    ch.skill[from_idx][diff_idx] = restored_diff;

    ch.skill[to_idx] = [0; SkillIndex::MaxIndex as usize];
    ch.set_do_update_flags();

    log::info!(
        "Restored skill {:?} from replacement {:?} for character {}",
        from,
        to,
        c_string_to_str(&ch.name)
    );
}

/// Remove a talent-granted skill and refund the experience spent raising it.
///
/// Talent-granted skills occupy reserved skill slots that are not present in
/// any character template, so a reset removes them entirely. The experience
/// the player spent raising the skill above its granted base
/// (`profile.base`) is summed using the same per-level cost as
/// [`GameState::do_raise_skill`] and returned to the spendable point pool.
/// `points_tot` is left untouched, matching the raise path which only debits
/// `points`.
///
/// # Arguments
///
/// * `cn` - Character slot index.
/// * `game_state` - Mutable game state.
/// * `skill` - Talent-granted skill to remove.
/// * `profile` - Profile the granting node declared, whose `base` marks the
///   floor below which no experience was ever spent.
fn ungrant_skill(cn: usize, game_state: &mut GameState, skill: Skill, profile: TalentSkillProfile) {
    let idx = skill as usize;
    let base_idx = SkillIndex::BaseValue as usize;
    let diff_idx = SkillIndex::RaiseDifficulty as usize;

    let base = i32::from(game_state.characters[cn].skill[idx][base_idx]);
    if base == 0 {
        return;
    }

    let diff = i32::from(game_state.characters[cn].skill[idx][diff_idx]);

    // Refund the experience spent raising the skill from its granted base
    // value up to its current base, matching `do_raise_skill`'s per-level
    // `skill_needed` cost.
    let mut refund: i32 = 0;
    for value in i32::from(profile.base).max(1)..base {
        refund = refund.saturating_add(points::skill_needed(value, diff));
    }

    let ch = &mut game_state.characters[cn];
    ch.points = ch.points.saturating_add(refund);
    ch.skill[idx][base_idx] = 0;
    ch.skill[idx][SkillIndex::MaxValue as usize] = 0;
    ch.skill[idx][diff_idx] = 0;
    ch.set_do_update_flags();

    log::info!(
        "Removed talent-granted skill {:?} from character {} (refunded {} experience)",
        skill,
        c_string_to_str(&ch.name),
        refund
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::god::God;
    use crate::test_helpers::{add_test_player, with_test_gs};
    use core::constants::{CharacterFlags, USE_ACTIVE};
    use core::skills::{
        Attribute, SK_AURA_CURSE, SK_AURA_WAR_BANNER, SK_BLAST, SK_ELEMENT_SWITCHING, SK_ICE_STUN,
        SK_INNER_STRENGTH, SK_LAVA_BLAST, SK_THUNDEROUS_FURY,
    };
    use core::talent_trees::{
        TALENT_LAYER_END, TALENT_LAYER_START, TALENT_POINTS_INDEX, grant_talent_points,
        is_talent_spent, reset_talent_points, talent_stat_bonuses,
    };
    use core::traits::{
        Class, KIN_ARCHHARAKIM, KIN_ARCHTEMPLAR, KIN_HARAKIM, KIN_MERCENARY, KIN_PURPLE,
        KIN_SEYAN_DU, KIN_TEMPLAR,
    };

    /// Character template id used for the base-templar template in tests.
    const TEMPLAR_TEMPLATE: usize = 540;
    /// Character template id used for the arch-templar template in tests.
    const ARCH_TEMPLAR_TEMPLATE: usize = 544;
    /// Character template id used for the arch-harakim template in tests.
    const ARCH_HARAKIM_TEMPLATE: usize = 545;
    /// Character template id used for the base-harakim template in tests.
    const HARAKIM_TEMPLATE: usize = 541;
    /// Character template id used for the male Seyan'Du template in tests.
    const SEYAN_DU_TEMPLATE: usize = 13;

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

    fn templar_slot(name: &str) -> TalentRef {
        tree_for(Class::Templar)
            .unwrap()
            .nodes
            .iter()
            .find(|node| node.name == name)
            .unwrap_or_else(|| panic!("missing templar talent '{name}'"))
            .slot
    }

    fn harakim_slot(name: &str) -> TalentRef {
        tree_for(Class::Harakim)
            .unwrap()
            .nodes
            .iter()
            .find(|node| node.name == name)
            .unwrap_or_else(|| panic!("missing harakim talent '{name}'"))
            .slot
    }

    fn seyan_du_slot(name: &str) -> TalentRef {
        tree_for(Class::SeyanDu)
            .unwrap()
            .nodes
            .iter()
            .find(|node| node.name == name)
            .unwrap_or_else(|| panic!("missing seyan'du talent '{name}'"))
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
    fn grant_skill_sets_base_to_profile_base_when_unset() {
        with_test_gs(|gs| {
            let cn = 1;
            grant_skill(cn, gs, Skill::Stealth, TalentSkillProfile::DEFAULT_NON_MERC).unwrap();
            assert_eq!(
                gs.characters[cn].skill[Skill::Stealth as usize][SkillIndex::BaseValue as usize],
                1
            );
        });
    }

    #[test]
    fn grant_skill_writes_profile_max_and_difficulty() {
        with_test_gs(|gs| {
            let cn = 1;
            let profile = TalentSkillProfile::DEFAULT_NON_MERC
                .with_max(60)
                .with_difficulty(9);

            grant_skill(cn, gs, Skill::Stealth, profile).unwrap();

            let skill = &gs.characters[cn].skill[Skill::Stealth as usize];
            assert_eq!(skill[SkillIndex::MaxValue as usize], 60);
            assert_eq!(skill[SkillIndex::RaiseDifficulty as usize], 9);
        });
    }

    #[test]
    fn grant_skill_keeps_existing_max_and_difficulty() {
        with_test_gs(|gs| {
            let cn = 1;
            let idx = Skill::Stealth as usize;
            gs.characters[cn].skill[idx][SkillIndex::MaxValue as usize] = 42;
            gs.characters[cn].skill[idx][SkillIndex::RaiseDifficulty as usize] = 3;

            grant_skill(
                cn,
                gs,
                Skill::Stealth,
                TalentSkillProfile::DEFAULT_NON_MERC
                    .with_max(60)
                    .with_difficulty(9),
            )
            .unwrap();

            assert_eq!(
                gs.characters[cn].skill[idx][SkillIndex::MaxValue as usize],
                42
            );
            assert_eq!(
                gs.characters[cn].skill[idx][SkillIndex::RaiseDifficulty as usize],
                3
            );
        });
    }

    #[test]
    fn grant_skill_raises_lower_base_and_seeds_progression() {
        with_test_gs(|gs| {
            let cn = 1;

            grant_skill(
                cn,
                gs,
                Skill::Meditate,
                TalentSkillProfile::DEFAULT_NON_MERC.with_base(5),
            )
            .unwrap();

            let skill = &gs.characters[cn].skill[Skill::Meditate as usize];
            assert_eq!(skill[SkillIndex::BaseValue as usize], 5);
            assert_eq!(
                skill[SkillIndex::MaxValue as usize],
                TalentSkillProfile::DEFAULT_NON_MERC.max_value
            );
            assert_eq!(
                skill[SkillIndex::RaiseDifficulty as usize],
                TalentSkillProfile::DEFAULT_NON_MERC.raise_difficulty
            );
        });
    }

    #[test]
    fn grant_skill_preserves_higher_existing_base() {
        with_test_gs(|gs| {
            let cn = 1;
            gs.characters[cn].skill[Skill::Meditate as usize][SkillIndex::BaseValue as usize] = 8;

            grant_skill(
                cn,
                gs,
                Skill::Meditate,
                TalentSkillProfile::DEFAULT_NON_MERC.with_base(5),
            )
            .unwrap();

            assert_eq!(
                gs.characters[cn].skill[Skill::Meditate as usize][SkillIndex::BaseValue as usize],
                8
            );
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
    fn learn_templar_meditative_discipline_grants_meditate_base_five() {
        with_test_gs(|gs| {
            let cn = 1;
            give_class_and_points(gs, cn, KIN_TEMPLAR, 1);
            for layer in 1..=3 {
                gs.characters[cn].future1[layer] |= 0b0000_0001;
            }

            learn_talent(gs, cn, templar_slot("Meditative Discipline")).unwrap();

            assert_eq!(
                gs.characters[cn].skill[Skill::Meditate as usize][SkillIndex::BaseValue as usize],
                5
            );
            assert_eq!(
                gs.characters[cn].skill[Skill::Meditate as usize][SkillIndex::MaxValue as usize],
                60,
                "talent-granted Meditate must cap at 60 regardless of template baseline"
            );
        });
    }

    #[test]
    fn learn_templar_inner_strength_replaces_warcry_with_existing_investment() {
        with_test_gs(|gs| {
            let cn = 1;
            give_class_and_points(gs, cn, KIN_TEMPLAR, 1);
            for layer in 1..=8 {
                gs.characters[cn].future1[layer] |= 0b0000_0001;
            }
            gs.characters[cn].skill[Skill::Warcry as usize][SkillIndex::BaseValue as usize] = 12;
            gs.characters[cn].skill[Skill::Warcry as usize][SkillIndex::MaxValue as usize] = 100;
            gs.characters[cn].skill[Skill::Warcry as usize][SkillIndex::RaiseDifficulty as usize] =
                5;

            learn_talent(gs, cn, templar_slot("Inner Strength")).unwrap();

            assert_eq!(
                gs.characters[cn].skill[Skill::Warcry as usize][SkillIndex::BaseValue as usize],
                0,
                "Inner Strength should wholly replace Warcry"
            );
            assert_eq!(
                gs.characters[cn].skill[SK_INNER_STRENGTH][SkillIndex::BaseValue as usize],
                12
            );
            assert_eq!(
                gs.characters[cn].skill[SK_INNER_STRENGTH][SkillIndex::MaxValue as usize],
                TalentSkillProfile::DEFAULT_NON_MERC.max_value
            );
        });
    }

    #[test]
    fn learn_templar_thunderous_fury_replaces_warcry_with_existing_investment() {
        with_test_gs(|gs| {
            let cn = 1;
            give_class_and_points(gs, cn, KIN_TEMPLAR, 1);
            for layer in 1..=8 {
                gs.characters[cn].future1[layer] |= 0b0000_0001;
            }
            gs.characters[cn].skill[Skill::Warcry as usize][SkillIndex::BaseValue as usize] = 12;

            learn_talent(gs, cn, templar_slot("Holy Fury")).unwrap();

            assert_eq!(
                gs.characters[cn].skill[Skill::Warcry as usize][SkillIndex::BaseValue as usize],
                0,
                "Thunderous Fury should wholly replace Warcry"
            );
            assert_eq!(
                gs.characters[cn].skill[SK_THUNDEROUS_FURY][SkillIndex::BaseValue as usize],
                12
            );
        });
    }

    #[test]
    fn reset_talents_swaps_inner_strength_back_to_warcry() {
        with_test_gs(|gs| {
            let cn = 1;
            give_class_and_points(gs, cn, KIN_TEMPLAR, 1);
            for layer in 1..=8 {
                gs.characters[cn].future1[layer] |= 0b0000_0001;
            }
            gs.characters[cn].skill[Skill::Warcry as usize][SkillIndex::BaseValue as usize] = 12;
            gs.characters[cn].skill[Skill::Warcry as usize][SkillIndex::MaxValue as usize] = 100;
            gs.characters[cn].skill[Skill::Warcry as usize][SkillIndex::RaiseDifficulty as usize] =
                5;
            learn_talent(gs, cn, templar_slot("Inner Strength")).unwrap();
            gs.characters[cn].skill[SK_INNER_STRENGTH][SkillIndex::BaseValue as usize] = 20;

            reset_talents(gs, cn);

            assert_eq!(
                gs.characters[cn].skill[Skill::Warcry as usize][SkillIndex::BaseValue as usize],
                20
            );
            assert_eq!(
                gs.characters[cn].skill[SK_INNER_STRENGTH][SkillIndex::BaseValue as usize],
                0
            );
        });
    }

    #[test]
    fn learn_harakim_lava_blast_replaces_blast_with_existing_investment() {
        with_test_gs(|gs| {
            let cn = 1;
            give_class_and_points(gs, cn, KIN_HARAKIM, 1);
            gs.characters[cn].skill[SK_BLAST][SkillIndex::BaseValue as usize] = 4;
            gs.characters[cn].skill[SK_BLAST][SkillIndex::MaxValue as usize] = 100;
            gs.characters[cn].skill[SK_BLAST][SkillIndex::RaiseDifficulty as usize] = 5;

            learn_talent(gs, cn, harakim_slot("Lava Blast")).unwrap();

            assert_eq!(
                gs.characters[cn].skill[SK_BLAST][SkillIndex::BaseValue as usize],
                0
            );
            assert_eq!(
                gs.characters[cn].skill[SK_LAVA_BLAST][SkillIndex::BaseValue as usize],
                4
            );
            assert_eq!(
                gs.characters[cn].skill[SK_LAVA_BLAST][SkillIndex::MaxValue as usize],
                TalentSkillProfile::DEFAULT_NON_MERC.max_value
            );
            assert_eq!(
                gs.characters[cn].skill[SK_LAVA_BLAST][SkillIndex::RaiseDifficulty as usize],
                TalentSkillProfile::DEFAULT_NON_MERC.raise_difficulty
            );
        });
    }

    #[test]
    fn learn_harakim_lava_blast_seeds_replacement_when_blast_is_absent() {
        with_test_gs(|gs| {
            let cn = 1;
            give_class_and_points(gs, cn, KIN_HARAKIM, 1);

            learn_talent(gs, cn, harakim_slot("Lava Blast")).unwrap();

            assert_eq!(
                gs.characters[cn].skill[SK_LAVA_BLAST][SkillIndex::BaseValue as usize],
                1
            );
            assert_eq!(
                gs.characters[cn].skill[SK_LAVA_BLAST][SkillIndex::MaxValue as usize],
                TalentSkillProfile::DEFAULT_NON_MERC.max_value
            );
            assert_eq!(
                gs.characters[cn].skill[SK_LAVA_BLAST][SkillIndex::RaiseDifficulty as usize],
                TalentSkillProfile::DEFAULT_NON_MERC.raise_difficulty
            );
        });
    }

    #[test]
    fn learn_harakim_lava_blast_ignores_source_max_and_difficulty() {
        with_test_gs(|gs| {
            let cn = 1;
            give_class_and_points(gs, cn, KIN_HARAKIM, 1);
            gs.characters[cn].skill[SK_BLAST][SkillIndex::BaseValue as usize] = 4;
            // Deliberately unlike the Lava Blast node's profile.
            gs.characters[cn].skill[SK_BLAST][SkillIndex::MaxValue as usize] = 250;
            gs.characters[cn].skill[SK_BLAST][SkillIndex::RaiseDifficulty as usize] = 9;

            learn_talent(gs, cn, harakim_slot("Lava Blast")).unwrap();

            // Investment carries over, but tuning comes from the talent node.
            assert_eq!(
                gs.characters[cn].skill[SK_LAVA_BLAST][SkillIndex::BaseValue as usize],
                4
            );
            assert_eq!(
                gs.characters[cn].skill[SK_LAVA_BLAST][SkillIndex::MaxValue as usize],
                TalentSkillProfile::DEFAULT_NON_MERC.max_value,
                "replacement max value must come from the node profile, not the source skill"
            );
            assert_eq!(
                gs.characters[cn].skill[SK_LAVA_BLAST][SkillIndex::RaiseDifficulty as usize],
                TalentSkillProfile::DEFAULT_NON_MERC.raise_difficulty,
                "replacement difficulty must come from the node profile, not the source skill"
            );
        });
    }

    #[test]
    fn reset_talents_restores_replaced_skill_tuning_from_template() {
        with_test_gs(|gs| {
            let (cn, _nr) = add_test_player(gs);
            give_class_and_points(gs, cn, KIN_HARAKIM, 1);

            install_template(gs, HARAKIM_TEMPLATE, KIN_HARAKIM);
            let template = &mut gs.character_templates[HARAKIM_TEMPLATE];
            template.skill[SK_BLAST][SkillIndex::BaseValue as usize] = 1;
            template.skill[SK_BLAST][SkillIndex::MaxValue as usize] = 80;
            template.skill[SK_BLAST][SkillIndex::RaiseDifficulty as usize] = 7;
            gs.characters[cn].temp = HARAKIM_TEMPLATE as u16;

            gs.characters[cn].skill[SK_BLAST][SkillIndex::BaseValue as usize] = 4;
            gs.characters[cn].skill[SK_BLAST][SkillIndex::MaxValue as usize] = 80;
            gs.characters[cn].skill[SK_BLAST][SkillIndex::RaiseDifficulty as usize] = 7;

            learn_talent(gs, cn, harakim_slot("Lava Blast")).unwrap();
            // Train the replacement past the template's cap.
            gs.characters[cn].skill[SK_LAVA_BLAST][SkillIndex::BaseValue as usize] = 90;

            reset_talents(gs, cn);

            assert_eq!(
                gs.characters[cn].skill[SK_LAVA_BLAST][SkillIndex::BaseValue as usize],
                0,
                "the replacement row must be cleared on reset"
            );
            assert_eq!(
                gs.characters[cn].skill[SK_BLAST][SkillIndex::MaxValue as usize],
                80,
                "the restored skill's max value must come from the character template"
            );
            assert_eq!(
                gs.characters[cn].skill[SK_BLAST][SkillIndex::RaiseDifficulty as usize],
                7,
                "the restored skill's difficulty must come from the character template"
            );
            assert_eq!(
                gs.characters[cn].skill[SK_BLAST][SkillIndex::BaseValue as usize],
                80,
                "restored investment is clamped to the template's max value"
            );
        });
    }

    #[test]
    fn learn_harakim_lava_blast_rejects_existing_replacement_without_spending_point() {
        with_test_gs(|gs| {
            let cn = 1;
            give_class_and_points(gs, cn, KIN_HARAKIM, 1);
            gs.characters[cn].skill[SK_BLAST][SkillIndex::BaseValue as usize] = 4;
            gs.characters[cn].skill[SK_LAVA_BLAST][SkillIndex::BaseValue as usize] = 2;

            let err = learn_talent(gs, cn, harakim_slot("Lava Blast")).unwrap_err();

            assert!(err.contains("already has replacement skill"), "got: {err}");
            assert_eq!(gs.characters[cn].future1[TALENT_POINTS_INDEX], 1);
            assert_eq!(gs.characters[cn].future1[1], 0);
            assert_eq!(
                gs.characters[cn].skill[SK_BLAST][SkillIndex::BaseValue as usize],
                4
            );
            assert_eq!(
                gs.characters[cn].skill[SK_LAVA_BLAST][SkillIndex::BaseValue as usize],
                2
            );
        });
    }

    #[test]
    fn reset_talents_swaps_lava_blast_back_to_blast() {
        with_test_gs(|gs| {
            let cn = 1;
            give_class_and_points(gs, cn, KIN_HARAKIM, 1);
            gs.characters[cn].skill[SK_BLAST][SkillIndex::BaseValue as usize] = 4;
            gs.characters[cn].skill[SK_BLAST][SkillIndex::MaxValue as usize] = 100;
            gs.characters[cn].skill[SK_BLAST][SkillIndex::RaiseDifficulty as usize] = 5;
            learn_talent(gs, cn, harakim_slot("Lava Blast")).unwrap();
            gs.characters[cn].skill[SK_LAVA_BLAST][SkillIndex::BaseValue as usize] = 6;

            reset_talents(gs, cn);

            assert_eq!(
                gs.characters[cn].skill[SK_BLAST][SkillIndex::BaseValue as usize],
                6
            );
            assert_eq!(
                gs.characters[cn].skill[SK_BLAST][SkillIndex::MaxValue as usize],
                TalentSkillProfile::DEFAULT_NON_MERC.max_value,
                "without a template row the replacement's own tuning is reused"
            );
            assert_eq!(
                gs.characters[cn].skill[SK_BLAST][SkillIndex::RaiseDifficulty as usize],
                TalentSkillProfile::DEFAULT_NON_MERC.raise_difficulty
            );
            assert_eq!(
                gs.characters[cn].skill[SK_LAVA_BLAST][SkillIndex::BaseValue as usize],
                0
            );
        });
    }

    #[test]
    fn learn_harakim_ice_stun_replaces_stun_with_existing_investment() {
        with_test_gs(|gs| {
            let cn = 1;
            give_class_and_points(gs, cn, KIN_HARAKIM, 1);
            for layer in 1..=4 {
                gs.characters[cn].future1[layer] |= 0b0000_0001;
            }
            gs.characters[cn].skill[Skill::Stun as usize][SkillIndex::BaseValue as usize] = 12;
            gs.characters[cn].skill[Skill::Stun as usize][SkillIndex::MaxValue as usize] = 100;
            gs.characters[cn].skill[Skill::Stun as usize][SkillIndex::RaiseDifficulty as usize] = 5;

            learn_talent(gs, cn, harakim_slot("Ice Stun")).unwrap();

            assert_eq!(
                gs.characters[cn].skill[Skill::Stun as usize][SkillIndex::BaseValue as usize],
                0,
                "Ice Stun should wholly replace Stun"
            );
            assert_eq!(
                gs.characters[cn].skill[SK_ICE_STUN][SkillIndex::BaseValue as usize],
                12
            );
            assert_eq!(
                gs.characters[cn].skill[SK_ICE_STUN][SkillIndex::MaxValue as usize],
                TalentSkillProfile::DEFAULT_NON_MERC.max_value
            );
            assert_eq!(
                gs.characters[cn].skill[SK_ICE_STUN][SkillIndex::RaiseDifficulty as usize],
                TalentSkillProfile::DEFAULT_NON_MERC.raise_difficulty
            );
            assert!(core::talent_trees::harakim::has_ice_stun(
                &gs.characters[cn].future1
            ));
        });
    }

    #[test]
    fn learn_harakim_element_switching_sets_talent_without_granting_new_skill() {
        with_test_gs(|gs| {
            let cn = 1;
            give_class_and_points(gs, cn, KIN_HARAKIM, 1);
            for layer in 1..=6 {
                gs.characters[cn].future1[layer] |= 0b0000_0001;
            }

            learn_talent(gs, cn, harakim_slot("Element Switching")).unwrap();

            assert_eq!(
                gs.characters[cn].skill[SK_ELEMENT_SWITCHING][SkillIndex::BaseValue as usize],
                0,
                "Element Switching should not become a separate castable skill"
            );
            assert!(core::talent_trees::harakim::has_element_switching(
                &gs.characters[cn].future1
            ));
        });
    }

    #[test]
    fn learn_harakim_spellcaster_kindred_grants_passive_after_prereqs() {
        with_test_gs(|gs| {
            let cn = 1;
            give_class_and_points(gs, cn, KIN_HARAKIM, 1);
            for layer in 1..=6 {
                gs.characters[cn].future1[layer] |= 0b0000_0001;
            }

            learn_talent(gs, cn, harakim_slot("Spellcaster Kindred Spirit")).unwrap();

            assert_eq!(
                gs.characters[cn].skill[Skill::SpellcasterKindredSpirit as usize]
                    [SkillIndex::BaseValue as usize],
                1
            );
        });
    }

    #[test]
    fn learn_templar_warlord_composite_recomputes_stat_bonuses() {
        with_test_gs(|gs| {
            let cn = 1;
            give_class_and_points(gs, cn, KIN_TEMPLAR, 1);
            for layer in 1..=11 {
                gs.characters[cn].future1[layer] |= 0b0000_0001;
            }
            gs.characters[cn].attrib[Attribute::Strength as usize]
                [SkillIndex::BaseValue as usize] = 50;
            gs.characters[cn].attrib[Attribute::Agility as usize][SkillIndex::BaseValue as usize] =
                40;

            learn_talent(gs, cn, templar_slot("Warlord Ascendancy")).unwrap();
            gs.really_update_char(cn);

            assert_eq!(
                gs.characters[cn].attrib[Attribute::Strength as usize]
                    [SkillIndex::TotalValue as usize],
                61
            );
            assert_eq!(
                gs.characters[cn].attrib[Attribute::Agility as usize]
                    [SkillIndex::TotalValue as usize],
                48
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
    fn reset_talents_refunds_points_and_clears_bits_via_game_state() {
        with_test_gs(|gs| {
            let cn = 1;
            give_class_and_points(gs, cn, KIN_MERCENARY, 2);
            learn_talent(gs, cn, mercenary_slot("Distract")).unwrap();
            learn_talent(gs, cn, mercenary_slot("Dodge Boost I")).unwrap();
            assert_eq!(gs.characters[cn].future1[TALENT_POINTS_INDEX], 0);

            reset_talents(gs, cn);

            let t = &gs.characters[cn].future1;
            assert_eq!(t[TALENT_POINTS_INDEX], 2);
            assert_eq!(t[1], 0);
            assert_eq!(t[2], 0);
        });
    }

    #[test]
    fn reset_talents_removes_granted_skill() {
        with_test_gs(|gs| {
            let cn = 1;
            give_class_and_points(gs, cn, KIN_TEMPLAR, 1);
            learn_talent(gs, cn, templar_slot("Renewal")).unwrap();

            let idx = Skill::RainsOfRenewal as usize;
            assert_eq!(
                gs.characters[cn].skill[idx][SkillIndex::BaseValue as usize],
                1
            );

            reset_talents(gs, cn);

            assert_eq!(
                gs.characters[cn].skill[idx][SkillIndex::BaseValue as usize],
                0
            );
            assert_eq!(
                gs.characters[cn].skill[idx][SkillIndex::MaxValue as usize],
                0
            );
            assert_eq!(
                gs.characters[cn].skill[idx][SkillIndex::RaiseDifficulty as usize],
                0
            );
            // The talent point spent on the grant is also refunded.
            assert_eq!(gs.characters[cn].future1[TALENT_POINTS_INDEX], 1);
        });
    }

    // ---- template re-stamping (racechange / pop_skill) -------------------

    /// Install a minimal, in-use character template for template-copy tests.
    fn install_template(gs: &mut GameState, temp: usize, kindred: u32) {
        let template = &mut gs.character_templates[temp];
        *template = Character::default();
        template.used = USE_ACTIVE;
        template.kindred = kindred as i32;
        // A native skill the race knows, to prove normal rows still copy.
        template.skill[Skill::Warcry as usize][SkillIndex::BaseValue as usize] = 1;
        template.skill[Skill::Warcry as usize][SkillIndex::MaxValue as usize] = 100;
        template.skill[Skill::Warcry as usize][SkillIndex::RaiseDifficulty as usize] = 3;
    }

    /// Learn Renewal + Sun's Blessing and train both to `base`.
    fn templar_with_trained_talent_skills(gs: &mut GameState, cn: usize, base: u16) {
        give_class_and_points(gs, cn, KIN_TEMPLAR, 2);
        learn_talent(gs, cn, templar_slot("Renewal")).unwrap();
        // Satisfy the prerequisite layers leading up to Sun's Blessing.
        for layer in 2..=4 {
            gs.characters[cn].future1[layer] |= 0b0000_0001;
        }
        learn_talent(gs, cn, templar_slot("Sun's Blessing")).unwrap();

        for idx in [Skill::RainsOfRenewal as usize, Skill::SunsBlessing as usize] {
            gs.characters[cn].skill[idx][SkillIndex::BaseValue as usize] = base;
        }
    }

    #[test]
    fn arching_keeps_talent_granted_skills_raisable() {
        with_test_gs(|gs| {
            let (cn, _nr) = add_test_player(gs);
            templar_with_trained_talent_skills(gs, cn, 30);
            install_template(gs, ARCH_TEMPLAR_TEMPLATE, KIN_ARCHTEMPLAR);

            God::minor_racechange(gs, cn, ARCH_TEMPLAR_TEMPLATE as i32);

            gs.characters[cn].points = 10_000_000;
            for idx in [Skill::RainsOfRenewal as usize, Skill::SunsBlessing as usize] {
                assert_eq!(
                    gs.characters[cn].skill[idx][SkillIndex::BaseValue as usize],
                    30,
                    "arching must not reduce talent-granted skill {idx}"
                );
                assert!(
                    gs.characters[cn].skill[idx][SkillIndex::MaxValue as usize] > 30,
                    "arching must not cap talent-granted skill {idx}"
                );
                assert_ne!(
                    gs.characters[cn].skill[idx][SkillIndex::RaiseDifficulty as usize],
                    0,
                    "arching must leave talent-granted skill {idx} raisable"
                );
                assert!(
                    gs.do_raise_skill(cn, idx as i32),
                    "talent-granted skill {idx} must still be raisable after arching"
                );
            }
        });
    }

    #[test]
    fn arching_does_not_restore_a_replaced_skill() {
        with_test_gs(|gs| {
            let (cn, _nr) = add_test_player(gs);
            give_class_and_points(gs, cn, KIN_HARAKIM, 1);
            gs.characters[cn].skill[SK_BLAST][SkillIndex::BaseValue as usize] = 20;
            gs.characters[cn].skill[SK_BLAST][SkillIndex::MaxValue as usize] = 100;
            gs.characters[cn].skill[SK_BLAST][SkillIndex::RaiseDifficulty as usize] = 5;
            learn_talent(gs, cn, harakim_slot("Lava Blast")).unwrap();

            install_template(gs, ARCH_HARAKIM_TEMPLATE, KIN_ARCHHARAKIM);
            // The arch template natively knows Blast.
            let template = &mut gs.character_templates[ARCH_HARAKIM_TEMPLATE];
            template.skill[SK_BLAST][SkillIndex::BaseValue as usize] = 1;
            template.skill[SK_BLAST][SkillIndex::MaxValue as usize] = 100;
            template.skill[SK_BLAST][SkillIndex::RaiseDifficulty as usize] = 5;

            God::minor_racechange(gs, cn, ARCH_HARAKIM_TEMPLATE as i32);

            assert_eq!(
                gs.characters[cn].skill[SK_BLAST][SkillIndex::BaseValue as usize],
                0,
                "a replaced skill must not be restored by the arch template"
            );
            assert_eq!(
                gs.characters[cn].skill[SK_LAVA_BLAST][SkillIndex::BaseValue as usize],
                20
            );
            gs.characters[cn].points = 10_000_000;
            assert!(gs.do_raise_skill(cn, SK_LAVA_BLAST as i32));
        });
    }

    #[test]
    fn pop_skill_keeps_talent_granted_skills_raisable() {
        with_test_gs(|gs| {
            let (cn, _nr) = add_test_player(gs);
            templar_with_trained_talent_skills(gs, cn, 30);
            install_template(gs, TEMPLAR_TEMPLATE, KIN_TEMPLAR);
            gs.characters[cn].temp = TEMPLAR_TEMPLATE as u16;

            crate::populate::pop_skill(gs);

            gs.characters[cn].points = 10_000_000;
            for idx in [Skill::RainsOfRenewal as usize, Skill::SunsBlessing as usize] {
                assert_eq!(
                    gs.characters[cn].skill[idx][SkillIndex::BaseValue as usize],
                    30,
                    "pop_skill must not wipe talent-granted skill {idx}"
                );
                assert!(
                    gs.do_raise_skill(cn, idx as i32),
                    "talent-granted skill {idx} must still be raisable after pop_skill"
                );
            }
        });
    }

    #[test]
    fn template_restamp_still_applies_to_non_talent_skills() {
        with_test_gs(|gs| {
            let (cn, _nr) = add_test_player(gs);
            templar_with_trained_talent_skills(gs, cn, 30);
            install_template(gs, ARCH_TEMPLAR_TEMPLATE, KIN_ARCHTEMPLAR);

            God::minor_racechange(gs, cn, ARCH_TEMPLAR_TEMPLATE as i32);

            let idx = Skill::Warcry as usize;
            assert_eq!(
                gs.characters[cn].skill[idx][SkillIndex::BaseValue as usize],
                1
            );
            assert_eq!(
                gs.characters[cn].skill[idx][SkillIndex::MaxValue as usize],
                100
            );
            assert_eq!(
                gs.characters[cn].skill[idx][SkillIndex::RaiseDifficulty as usize],
                3
            );
        });
    }

    #[test]
    fn reset_talents_refunds_experience_spent_raising_granted_skill() {
        with_test_gs(|gs| {
            let cn = 1;
            give_class_and_points(gs, cn, KIN_TEMPLAR, 1);
            gs.characters[cn].points = 100;
            learn_talent(gs, cn, templar_slot("Renewal")).unwrap();

            let idx = Skill::RainsOfRenewal as usize;
            // Raise the granted skill from base 1 -> 3, spending 1 + 2 = 3 exp.
            assert!(gs.do_raise_skill(cn, idx as i32));
            assert!(gs.do_raise_skill(cn, idx as i32));
            assert_eq!(
                gs.characters[cn].skill[idx][SkillIndex::BaseValue as usize],
                3
            );
            assert_eq!(gs.characters[cn].points, 97);

            reset_talents(gs, cn);

            // Skill removed and the 3 experience returned to the pool.
            assert_eq!(
                gs.characters[cn].skill[idx][SkillIndex::BaseValue as usize],
                0
            );
            assert_eq!(gs.characters[cn].points, 100);
        });
    }

    #[test]
    fn reset_talents_clears_primary_hit_counts() {
        with_test_gs(|gs| {
            let cn = 1;
            give_class_and_points(gs, cn, KIN_TEMPLAR, 1);
            gs.talent_primary_hit_counts[cn] = 4;

            reset_talents(gs, cn);

            assert_eq!(gs.talent_primary_hit_counts[cn], 0);
        });
    }

    #[test]
    fn reset_talents_leaves_no_residual_for_stat_only_talent() {
        with_test_gs(|gs| {
            let cn = 1;
            give_class_and_points(gs, cn, KIN_MERCENARY, 2);
            let points_before = gs.characters[cn].points;
            learn_talent(gs, cn, mercenary_slot("Distract")).unwrap();
            learn_talent(gs, cn, mercenary_slot("Dodge Boost I")).unwrap();

            reset_talents(gs, cn);

            // Stat/dodge talents are recomputed from bits, so resetting must not
            // touch the experience pool.
            assert_eq!(gs.characters[cn].points, points_before);
            assert_eq!(gs.characters[cn].future1[TALENT_POINTS_INDEX], 2);
        });
    }

    #[test]
    fn ungrant_skill_refunds_cubic_raise_cost() {
        with_test_gs(|gs| {
            let cn = 1;
            let idx = Skill::RainsOfRenewal as usize;
            gs.characters[cn].skill[idx][SkillIndex::BaseValue as usize] = 4;
            gs.characters[cn].skill[idx][SkillIndex::MaxValue as usize] = 100;
            gs.characters[cn].skill[idx][SkillIndex::RaiseDifficulty as usize] = 5;
            gs.characters[cn].points = 0;

            ungrant_skill(
                cn,
                gs,
                Skill::RainsOfRenewal,
                TalentSkillProfile::DEFAULT_NON_MERC,
            );

            // skill_needed(value, 5) for value in 1..4 -> 1 + 2 + 3 = 6.
            assert_eq!(gs.characters[cn].points, 6);
            assert_eq!(
                gs.characters[cn].skill[idx][SkillIndex::BaseValue as usize],
                0
            );
        });
    }

    #[test]
    fn ungrant_skill_refund_starts_at_profile_base() {
        with_test_gs(|gs| {
            let cn = 1;
            let idx = Skill::Meditate as usize;
            gs.characters[cn].skill[idx][SkillIndex::BaseValue as usize] = 4;
            gs.characters[cn].skill[idx][SkillIndex::MaxValue as usize] = 100;
            gs.characters[cn].skill[idx][SkillIndex::RaiseDifficulty as usize] = 5;
            gs.characters[cn].points = 0;

            ungrant_skill(
                cn,
                gs,
                Skill::Meditate,
                TalentSkillProfile::DEFAULT_NON_MERC.with_base(3),
            );

            // Only value 3 was ever paid for; skill_needed(3, 5) == 3.
            assert_eq!(gs.characters[cn].points, 3);
        });
    }

    #[test]
    fn ungrant_skill_is_noop_for_absent_skill() {
        with_test_gs(|gs| {
            let cn = 1;
            gs.characters[cn].points = 42;
            ungrant_skill(
                cn,
                gs,
                Skill::RainsOfRenewal,
                TalentSkillProfile::DEFAULT_NON_MERC,
            );
            assert_eq!(gs.characters[cn].points, 42);
        });
    }

    #[test]
    fn becoming_seyan_du_wipes_every_talent_trace() {
        with_test_gs(|gs| {
            let (cn, _nr) = add_test_player(gs);
            templar_with_trained_talent_skills(gs, cn, 30);
            gs.characters[cn].points = 0;
            gs.talent_primary_hit_counts[cn] = 4;
            install_template(gs, SEYAN_DU_TEMPLATE, KIN_SEYAN_DU);

            God::racechange(gs, cn, SEYAN_DU_TEMPLATE as i32);

            assert_eq!(
                gs.characters[cn].future1, [0u8; 25],
                "learned talents and the unspent-point pool must both be zeroed"
            );
            for idx in [Skill::RainsOfRenewal as usize, Skill::SunsBlessing as usize] {
                assert_eq!(
                    gs.characters[cn].skill[idx],
                    [0; SkillIndex::MaxIndex as usize],
                    "talent-granted skill {idx} must be wiped"
                );
            }
            assert_eq!(
                gs.characters[cn].points, 0,
                "becoming Seyan'Du must not pay experience back"
            );
            assert_eq!(gs.talent_primary_hit_counts[cn], 0);
        });
    }

    #[test]
    fn racechange_to_seyan_du_leaves_only_the_seyan_du_class_bit() {
        with_test_gs(|gs| {
            let (cn, _nr) = add_test_player(gs);
            give_class_and_points(gs, cn, KIN_MERCENARY, 0);
            install_template(gs, SEYAN_DU_TEMPLATE, KIN_SEYAN_DU);

            God::racechange(gs, cn, SEYAN_DU_TEMPLATE as i32);

            let kindred = gs.characters[cn].kindred as u32;
            assert_eq!(
                kindred & KIN_MERCENARY,
                0,
                "the old class bit must not survive a race change"
            );
            assert_ne!(kindred & KIN_SEYAN_DU, 0);
            assert_eq!(
                Class::from(gs.characters[cn].kindred),
                Class::SeyanDu,
                "the server must resolve the same tree the client renders"
            );
        });
    }

    #[test]
    fn racechange_preserves_purple_kindred() {
        with_test_gs(|gs| {
            let (cn, _nr) = add_test_player(gs);
            give_class_and_points(gs, cn, KIN_MERCENARY, 0);
            gs.characters[cn].kindred |= KIN_PURPLE as i32;
            install_template(gs, SEYAN_DU_TEMPLATE, KIN_SEYAN_DU);

            God::racechange(gs, cn, SEYAN_DU_TEMPLATE as i32);

            assert_ne!(gs.characters[cn].kindred as u32 & KIN_PURPLE, 0);
            assert_eq!(gs.characters[cn].temple_x, 558);
            assert_eq!(gs.characters[cn].temple_y, 542);
        });
    }

    #[test]
    fn seyan_du_talent_bonuses_apply_after_a_race_change() {
        with_test_gs(|gs| {
            let (cn, _nr) = add_test_player(gs);
            give_class_and_points(gs, cn, KIN_MERCENARY, 0);
            install_template(gs, SEYAN_DU_TEMPLATE, KIN_SEYAN_DU);
            God::racechange(gs, cn, SEYAN_DU_TEMPLATE as i32);

            gs.characters[cn].attrib[Attribute::Braveness as usize]
                [SkillIndex::BaseValue as usize] = 50;
            gs.characters[cn].attrib[Attribute::Agility as usize][SkillIndex::BaseValue as usize] =
                50;
            grant_talent_points(&mut gs.characters[cn].future1, 2);

            learn_talent(gs, cn, seyan_du_slot("Wellspring")).unwrap();
            learn_talent(gs, cn, seyan_du_slot("Fleet Hands")).unwrap();

            let bonuses = talent_stat_bonuses(
                gs.characters[cn].kindred,
                &gs.characters[cn].future1,
                &gs.characters[cn].attrib,
                &gs.characters[cn].skill,
            );

            assert_eq!(bonuses.attrib[Attribute::Agility as usize], 5);
        });
    }

    #[test]
    fn seyan_du_wipe_does_not_restore_a_replaced_skill() {
        with_test_gs(|gs| {
            let (cn, _nr) = add_test_player(gs);
            give_class_and_points(gs, cn, KIN_HARAKIM, 1);
            gs.characters[cn].skill[SK_BLAST][SkillIndex::BaseValue as usize] = 20;
            gs.characters[cn].skill[SK_BLAST][SkillIndex::MaxValue as usize] = 100;
            gs.characters[cn].skill[SK_BLAST][SkillIndex::RaiseDifficulty as usize] = 5;
            learn_talent(gs, cn, harakim_slot("Lava Blast")).unwrap();

            install_template(gs, SEYAN_DU_TEMPLATE, KIN_SEYAN_DU);
            God::racechange(gs, cn, SEYAN_DU_TEMPLATE as i32);

            assert_eq!(
                gs.characters[cn].skill[SK_LAVA_BLAST][SkillIndex::BaseValue as usize],
                0
            );
            assert_eq!(
                gs.characters[cn].skill[SK_BLAST][SkillIndex::BaseValue as usize],
                0,
                "the superseded skill must not come back with its old investment"
            );
        });
    }

    #[test]
    fn seyan_du_aura_of_despair_is_raisable() {
        with_test_gs(|gs| {
            let (cn, _nr) = add_test_player(gs);
            install_template(gs, SEYAN_DU_TEMPLATE, KIN_SEYAN_DU);
            give_class_and_points(gs, cn, KIN_SEYAN_DU, 3);

            learn_talent(gs, cn, seyan_du_slot("Wellspring")).unwrap();
            learn_talent(gs, cn, seyan_du_slot("Piercing Will")).unwrap();
            learn_talent(gs, cn, seyan_du_slot("Aura of Despair")).unwrap();

            let skill = &gs.characters[cn].skill[SK_AURA_CURSE];
            assert_eq!(skill[SkillIndex::BaseValue as usize], 1);
            assert!(
                skill[SkillIndex::MaxValue as usize] > 1,
                "Aura of Despair must have a non-zero raise cap"
            );
            assert_ne!(
                skill[SkillIndex::RaiseDifficulty as usize],
                0,
                "Aura of Despair must be raisable"
            );

            gs.characters[cn].points = 10_000_000;
            assert!(
                gs.do_raise_skill(cn, SK_AURA_CURSE as i32),
                "Aura of Despair should accept skill-point raises"
            );
        });
    }

    #[test]
    fn seyan_du_war_banner_is_raisable() {
        with_test_gs(|gs| {
            let (cn, _nr) = add_test_player(gs);
            install_template(gs, SEYAN_DU_TEMPLATE, KIN_SEYAN_DU);
            give_class_and_points(gs, cn, KIN_SEYAN_DU, 3);

            learn_talent(gs, cn, seyan_du_slot("Second Wind")).unwrap();
            learn_talent(gs, cn, seyan_du_slot("Fleet Hands")).unwrap();
            learn_talent(gs, cn, seyan_du_slot("War Banner")).unwrap();

            let skill = &gs.characters[cn].skill[SK_AURA_WAR_BANNER];
            assert_eq!(skill[SkillIndex::BaseValue as usize], 1);
            assert!(
                skill[SkillIndex::MaxValue as usize] > 1,
                "War Banner must have a non-zero raise cap"
            );
            assert_ne!(
                skill[SkillIndex::RaiseDifficulty as usize],
                0,
                "War Banner must be raisable"
            );

            gs.characters[cn].points = 10_000_000;
            assert!(
                gs.do_raise_skill(cn, SK_AURA_WAR_BANNER as i32),
                "War Banner should accept skill-point raises"
            );
        });
    }

    #[test]
    fn core_node_effects_have_distinct_class_flavors() {
        assert_grant_effect(
            tree_for(Class::Templar).unwrap().nodes[0].effect,
            Skill::RainsOfRenewal,
        );
        assert_replace_effect(
            tree_for(Class::Harakim).unwrap().nodes[0].effect,
            Skill::Blast,
            Skill::LavaBlast,
        );
        assert_effect(
            tree_for(Class::SeyanDu).unwrap().nodes[6].effect,
            Attribute::Agility,
            15,
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

    fn assert_grant_effect(effect: TalentEffect, expected_skill: Skill) {
        match effect {
            TalentEffect::GrantSkill { skill, .. } => assert_eq!(skill, expected_skill),
            other => panic!("expected GrantSkill, got {other:?}"),
        }
    }

    fn assert_replace_effect(effect: TalentEffect, expected_from: Skill, expected_to: Skill) {
        match effect {
            TalentEffect::ReplaceSkill { from, to, .. } => {
                assert_eq!(from, expected_from);
                assert_eq!(to, expected_to);
            }
            other => panic!("expected ReplaceSkill, got {other:?}"),
        }
    }
}
