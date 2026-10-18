use super::common::{
    add_exhaust, add_spell, has_active_spell_temp, is_exhausted, remove_active_spell_temp,
    spell_race_mod, spellcost,
};
use crate::chlog;
use crate::effect::EffectManager;
use crate::game_state::GameState;
use crate::god::God;
use crate::helpers;
use crate::points;
use crate::populate;
use core::constants::{
    AT_AGIL, AT_STREN, CHD_COMPANION, CHD_COMPANION2, CHD_TALKATIVE, COMPANION_TIMEOUT,
    CT_COMPANION, CharacterFlags, ItemFlags, NT_DIDHIT, NT_GOTHIT, TICKS, USE_EMPTY,
};
use core::skills::{
    SK_BLAST, SK_GHOST, SK_KINDRED_SPIRIT, SK_REVENANT_CONDUIT, SK_REVENANT_CONDUIT2,
    SK_SPECTRAL_PACT, SK_SPECTRAL_PACT2, SK_SPELLCASTER_KINDRED_SPIRIT,
};
use core::string_operations::c_string_to_str;
use core::traits::KIN_MONSTER;
use core::types::{Character, FontColor};

/// Recomputes a ghost companion's attributes, skills, and vital pools from its
/// owner's current Ghost Companion skill, applying any active Revenant Conduit
/// boost the owner currently has.
///
/// This is shared between [`skill_ghost`] (at summon time) and
/// [`skill_revenant_conduit`] (so toggling the conduit on or off immediately
/// re-stats existing companions instead of only affecting future summons).
///
/// # Arguments
///
/// * `gs` - Game state.
/// * `cn` - Owner (caster) character index.
/// * `cc` - Companion character index to re-stat.
///
/// # Returns
///
/// * The derived `base` value used to scale the companion's stats.
fn recompute_companion_stats(gs: &mut GameState, cn: usize, cc: usize) -> i32 {
    // Revenant Conduit: while the buff is active, the effective Ghost
    // Companion skill is boosted by `item.power` percent.
    let mut effective_ghost = i32::from(gs.characters[cn].skill[SK_GHOST][5]);
    let mut conduit_boost = 0i32;
    for n in 0..20 {
        let in_ = gs.characters[cn].spell[n] as usize;
        if in_ != 0 && gs.items[in_].temp == SK_REVENANT_CONDUIT2 as u16 {
            conduit_boost = gs.items[in_].power as i32;
            break;
        }
    }
    if conduit_boost > 0 {
        effective_ghost = effective_ghost * (100 + conduit_boost) / 100;
    }
    let mut base = (effective_ghost * 4) / 11;
    let kindred = gs.characters[cn].kindred;
    base = spell_race_mod(gs, base, kindred);

    for n in 0..5 {
        let mut tmp = base;
        tmp = tmp * 3 / std::cmp::max(1, i32::from(gs.characters[cc].attrib[n][3]));
        gs.characters[cc].attrib[n][0] = std::cmp::max(
            10,
            std::cmp::min(i32::from(gs.characters[cc].attrib[n][2]), tmp) as u16,
        );
    }

    for n in 0..core::skills::MAX_SKILLS {
        let mut tmp = base;
        tmp = tmp * 3 / std::cmp::max(1, i32::from(gs.characters[cc].skill[n][3]));
        if gs.characters[cc].skill[n][2] != 0 {
            gs.characters[cc].skill[n][0] =
                std::cmp::min(gs.characters[cc].skill[n][2], tmp as u16);
        }
    }

    let has_spellcaster_kindred_spirit = gs.characters[cn].skill[SK_SPELLCASTER_KINDRED_SPIRIT][0]
        != 0
        && (gs.characters[cn].flags & CharacterFlags::Player.bits()) != 0;
    if has_spellcaster_kindred_spirit {
        gs.characters[cc].skill[SK_BLAST][0] = gs.characters[cc].skill[SK_BLAST][0].max(1);
        gs.characters[cc].skill[SK_BLAST][2] = gs.characters[cc].skill[SK_BLAST][2].max(100);
        gs.characters[cc].skill[SK_BLAST][3] = gs.characters[cc].skill[SK_BLAST][3].max(5);
    }

    gs.characters[cc].hp[0] = std::cmp::max(
        50,
        std::cmp::min(i32::from(gs.characters[cc].hp[2]), base * 5),
    ) as u16;
    gs.characters[cc].end[0] = std::cmp::max(
        50,
        std::cmp::min(i32::from(gs.characters[cc].end[2]), base * 5),
    ) as u16;
    gs.characters[cc].mana[0] = 0;

    base
}

/// Re-stats every live ghost companion owned by `cn`.
///
/// Used when the owner's Revenant Conduit buff is toggled on or off so the
/// boost (or its removal) applies to companions that were summoned earlier.
///
/// # Arguments
///
/// * `gs` - Game state.
/// * `cn` - Owner (caster) character index.
pub(crate) fn restat_owned_companions(gs: &mut GameState, cn: usize) {
    for cc in 1..gs.characters.len() {
        if gs.characters[cc].used == USE_EMPTY {
            continue;
        }
        // Ghost companions store their owner in data[63] (see `skill_ghost`).
        if gs.characters[cc].data[63] as usize != cn {
            continue;
        }
        if gs.characters[cc].temp != CT_COMPANION as u16 {
            continue;
        }
        recompute_companion_stats(gs, cn, cc);
        gs.do_update_char(cc);
    }
}

/// Handles direct player/NPC use of the Ghost Companion skill.
///
/// Creates and initializes a companion NPC for the caster, optionally setting
/// it to attack the selected target.
///
/// # Arguments
///
/// * `gs` - Active game state used for target validation, companion creation, and initialization.
/// * `cn` - Caster character index.
///
/// # Panics
///
/// * Panics if `cn`, the selected target index, or the created companion index is invalid.
pub fn skill_ghost(gs: &mut GameState, cn: usize) {
    // Check if in build mode
    if (gs.characters[cn].flags & CharacterFlags::BuildMode.bits()) != 0 {
        gs.do_character_log(cn, FontColor::Red, "Not in build mode.\n");
        return;
    }

    // Inspect both companion slots. Kindred Spirit grants access to the
    // second slot; without it, only `CHD_COMPANION` is usable.
    let has_kindred_spirit = gs.characters[cn].skill[SK_KINDRED_SPIRIT][0] != 0
        && (gs.characters[cn].flags & CharacterFlags::Player.bits()) != 0;

    let companion_slot_state = |gs: &GameState, slot: usize| -> Option<usize> {
        if (gs.characters[cn].flags & CharacterFlags::Player.bits()) == 0 {
            return None;
        }
        let co = gs.characters[cn].data[slot] as usize;
        if co == 0 {
            return None;
        }
        if Character::is_sane_character(co)
            && gs.characters[co].data[63] == cn as i32
            && (gs.characters[co].flags & CharacterFlags::Body.bits()) == 0
            && gs.characters[co].used != USE_EMPTY
        {
            Some(co)
        } else {
            None
        }
    };

    let slot1 = companion_slot_state(gs, CHD_COMPANION);
    let slot2 = if has_kindred_spirit {
        companion_slot_state(gs, CHD_COMPANION2)
    } else {
        None
    };

    let target_slot = if slot1.is_none() {
        Some(CHD_COMPANION)
    } else if has_kindred_spirit && slot2.is_none() {
        Some(CHD_COMPANION2)
    } else {
        None
    };

    if target_slot.is_none() {
        let limit = if has_kindred_spirit { 2 } else { 1 };
        let existing = slot1.or(slot2).unwrap_or(0);
        gs.do_character_log(
            cn,
            FontColor::Red,
            &format!(
                "You may not have more than {} Ghost Companion(s) ({}).\n",
                limit, existing
            ),
        );
        return;
    }
    let target_slot = target_slot.unwrap();

    // Get target
    let mut co = gs.characters[cn].skill_target1 as usize;
    if co == cn {
        co = 0;
    }

    // Check visibility
    if co != 0 && gs.do_char_can_see(cn, co) == 0 {
        gs.do_character_log(cn, FontColor::Red, "You cannot see your target.\n");
        return;
    }

    if is_exhausted(gs, cn) {
        return;
    }

    // Check if can attack target
    if co != 0 && !gs.may_attack_msg(cn, co, true) {
        chlog!(
            cn,
            "Prevented from attacking {} ({})",
            gs.characters[co].get_name().to_owned(),
            co
        );
        return;
    }

    if spellcost(gs, cn, 45) != 0 {
        return;
    }

    // No GC in Gatekeeper's room
    let (cx, cy) = (gs.characters[cn].x, gs.characters[cn].y);
    if (39..=47).contains(&cx) && (594..=601).contains(&cy) {
        gs.do_character_log(cn, FontColor::Red, "You must fight this battle alone.\n");
        return;
    }

    // Create companion
    let cc_opt = populate::pop_create_char(gs, CT_COMPANION as usize, true);
    if cc_opt.is_none() {
        gs.do_character_log(
            cn,
            FontColor::Red,
            "The ghost companion could not materialize.\n",
        );
        return;
    }
    let cc = cc_opt.unwrap();

    let (cc_x, cc_y) = (gs.characters[cn].x as usize, gs.characters[cn].y as usize);
    if !God::drop_char_fuzzy(gs, cc, cc_x, cc_y) {
        gs.characters[cc].used = USE_EMPTY;
        gs.do_character_log(
            cn,
            FontColor::Red,
            "The ghost companion could not materialize.\n",
        );
        return;
    }

    // Assign a randomized name to the companion
    let random_name = {
        let mut candidate = None;
        for _ in 0..100 {
            let name = core::names::randomly_generate_name();
            let name_exists = gs.characters.iter().enumerate().any(|(idx, other)| {
                idx != cc && other.used != USE_EMPTY && other.get_name().eq_ignore_ascii_case(&name)
            });
            if !name_exists {
                candidate = Some(name);
                break;
            }
        }
        candidate.unwrap_or_else(core::names::randomly_generate_name)
    };

    {
        let companion = &mut gs.characters[cc];

        let mut name_bytes = [0u8; 40];
        let name_src = random_name.as_bytes();
        let name_len = name_src.len().min(name_bytes.len());
        name_bytes[..name_len].copy_from_slice(&name_src[..name_len]);
        companion.name = name_bytes;

        let mut reference_bytes = [0u8; 40];
        reference_bytes[..name_len].copy_from_slice(&name_src[..name_len]);
        companion.reference = reference_bytes;

        let desc = companion.get_default_description();
        let desc_src = desc.as_bytes();
        let mut desc_bytes = [0u8; 200];
        let desc_len = desc_src.len().min(desc_bytes.len());
        desc_bytes[..desc_len].copy_from_slice(&desc_src[..desc_len]);
        companion.description = desc_bytes;
    }

    if co != 0 {
        if (gs.characters[co].flags & CharacterFlags::SpellIgnore.bits()) == 0 {
            gs.do_notify_character(co as u32, i32::from(NT_GOTHIT), cn as i32, 0, 0, 0);
        }
        gs.do_notify_character(cn as u32, i32::from(NT_DIDHIT), co as i32, 0, 0, 0);
    }

    if (gs.characters[cn].flags & CharacterFlags::Player.bits()) != 0 {
        gs.characters[cn].data[target_slot] = cc as i32;
    }

    let ticker = gs.globals.ticker;

    let co_id = if co != 0 {
        helpers::char_id(&gs.characters[co])
    } else {
        0
    };

    gs.characters[cc].data[29] = 0;
    gs.characters[cc].data[42] = 65536 + cn as i32;
    gs.characters[cc].kindred &= !(KIN_MONSTER as i32);
    gs.characters[cc].flags &= !CharacterFlags::Player.bits();

    if co != 0 {
        gs.characters[cc].attack_cn = co as u16;
        let idx = co as i32 | (co_id << 16);
        gs.characters[cc].data[80] = idx;
    }

    gs.characters[cc].data[63] = cn as i32;
    gs.characters[cc].data[69] = cn as i32;

    if (gs.characters[cn].flags & CharacterFlags::Player.bits()) != 0 {
        gs.characters[cc].data[CHD_COMPANION] = 0;
    } else {
        gs.characters[cc].data[CHD_COMPANION] = ticker + TICKS * 60 * 5;
    }
    gs.characters[cc].data[98] = ticker + COMPANION_TIMEOUT;

    // Clear all text slots to prevent leftover bytes from previous content bleeding through.
    for slot in &mut gs.characters[cc].text {
        slot.fill(0);
    }

    let text0 = b"#14#Yes! %s buys the farm!";
    gs.characters[cc].text[0][..text0.len()].copy_from_slice(text0);
    let text1 = b"#13#Yahoo! An enemy! Prepare to die, %s!";
    gs.characters[cc].text[1][..text1.len()].copy_from_slice(text1);
    let text3 = b"My successor will avenge me, %s!";
    gs.characters[cc].text[3][..text3.len()].copy_from_slice(text3);

    gs.characters[cc].data[48] = 33;

    gs.characters[cc].data[CHD_TALKATIVE] =
        gs.character_templates[CT_COMPANION as usize].data[CHD_TALKATIVE];

    let base = recompute_companion_stats(gs, cn, cc);

    let mut pts = 0i32;

    let attribs = gs.characters[cc].attrib;
    let hp0 = gs.characters[cc].hp[0];
    let end0 = gs.characters[cc].end[0];
    let mana0 = gs.characters[cc].mana[0];
    let skills = gs.characters[cc].skill;

    for attrib in &attribs[..5] {
        for m in 10..i32::from(attrib[0]) {
            pts += points::attrib_needed(m, 3);
        }
    }

    for m in 50..i32::from(hp0) {
        pts += points::hp_needed(m, 3);
    }

    for m in 50..i32::from(end0) {
        pts += points::end_needed(m, 2);
    }

    for m in 50..i32::from(mana0) {
        pts += points::mana_needed(m, 3);
    }

    for skill in &skills[..50] {
        for m in 1..i32::from(skill[0]) {
            pts += points::skill_needed(m, 2);
        }
    }

    gs.characters[cc].points_tot = pts;
    gs.characters[cc].gold = 0;
    gs.characters[cc].a_hp = 999999;
    gs.characters[cc].a_end = 999999;
    gs.characters[cc].a_mana = 999999;
    gs.characters[cc].alignment = gs.characters[cn].alignment / 2;

    let agil = gs.characters[cc].attrib[AT_AGIL as usize][0];
    let stren = gs.characters[cc].attrib[AT_STREN as usize][0];

    if agil >= 90 && stren >= 90 {
        gs.characters[cc].armor_bonus = 48 + 32;
        gs.characters[cc].weapon_bonus = 40 + 32;
    } else if agil >= 72 && stren >= 72 {
        gs.characters[cc].armor_bonus = 36 + 28;
        gs.characters[cc].weapon_bonus = 32 + 28;
    } else if agil >= 40 && stren >= 40 {
        gs.characters[cc].armor_bonus = 30 + 24;
        gs.characters[cc].weapon_bonus = 24 + 24;
    } else if agil >= 24 && stren >= 24 {
        gs.characters[cc].armor_bonus = 24 + 20;
        gs.characters[cc].weapon_bonus = 16 + 20;
    } else if agil >= 16 && stren >= 16 {
        gs.characters[cc].armor_bonus = 18 + 16;
        gs.characters[cc].weapon_bonus = 8 + 16;
    } else if agil >= 12 && stren >= 12 {
        gs.characters[cc].armor_bonus = 12 + 12;
        gs.characters[cc].weapon_bonus = 8 + 12;
    } else if agil >= 10 && stren >= 10 {
        gs.characters[cc].armor_bonus = 6 + 8;
        gs.characters[cc].weapon_bonus = 8 + 8;
    }

    let (cc_name, cn_ref) = (gs.characters[cc].name, gs.characters[cn].reference);
    log::info!(
        "Created {} ({}) with base {} as Ghost Companion for {}",
        c_string_to_str(&cc_name),
        cc,
        base,
        c_string_to_str(&cn_ref)
    );

    // Make companion speak
    if co != 0 {
        let co_name = gs.characters[co].get_name().to_owned();
        gs.do_sayx(
            cc,
            &format!("#13#Yahoo! An enemy! Prepare to die, {}!", co_name),
        );
    } else {
        let rank = core::ranks::points2rank(pts as u32);
        let cn_name = gs.characters[cn].get_name().to_owned();
        if rank < 6 {
            // GC not yet Master Sergeant
            gs.do_sayx(cc, &format!("I shall defend you and obey your commands, {}. I will WAIT, FOLLOW , be QUIET or ATTACK for you and tell you WHAT TIME. You may also command me to TRANSFER my experience to you, though I'd rather you didn't.\n", cn_name));
        } else {
            gs.do_sayx(cc, &format!("Thank you for creating me, {}!\n", cn_name));
        }
    }

    gs.do_update_char(cc);

    add_exhaust(gs, cn, TICKS * 4);

    let (cc_x, cc_y) = (
        i32::from(gs.characters[cc].x),
        i32::from(gs.characters[cc].y),
    );
    EffectManager::fx_add_effect(gs, 6, 0, cc_x, cc_y, 0);
    let (cn_x, cn_y) = (
        i32::from(gs.characters[cn].x),
        i32::from(gs.characters[cn].y),
    );
    EffectManager::fx_add_effect(gs, 7, 0, cn_x, cn_y, 0);
}

/// Returns whether `owner_id` still has a live Ghost Companion in either slot.
///
/// # Arguments
///
/// * `gs` - Game state.
/// * `owner_id` - Character id to check for a live companion.
///
/// # Returns
///
/// * `true` if either companion slot points to a live, owned companion.
fn owner_has_live_companion(gs: &GameState, owner_id: usize) -> bool {
    for slot in [CHD_COMPANION, CHD_COMPANION2] {
        let cc = gs.characters[owner_id].data[slot] as usize;
        if cc == 0 || !Character::is_sane_character(cc) {
            continue;
        }
        if gs.characters[cc].data[63] == owner_id as i32
            && gs.characters[cc].temp == CT_COMPANION as u16
            && gs.characters[cc].used != USE_EMPTY
            && (gs.characters[cc].flags & CharacterFlags::Body.bits()) == 0
        {
            return true;
        }
    }
    false
}

/// Clears Ghost-Companion-dependent Harakim buffs (Revenant Conduit,
/// Spectral Pact) from `owner_id` once they have no live Ghost Companion
/// left.
///
/// Companions can be lost through the `transfer` talk command, combat
/// death, or self-destruct from neglect. Without this, a stale Revenant
/// Conduit buff would keep boosting the effective Ghost Companion skill
/// used to stat the next summoned companion, letting a player ratchet up
/// companion power well beyond what a single cast of the conduit should
/// grant. Spectral Pact is cleared alongside it since it is equally
/// meaningless without a companion to redirect damage to.
///
/// # Arguments
///
/// * `gs` - Game state.
/// * `owner_id` - Character id that just lost a companion.
pub(crate) fn clear_companion_dependent_buffs_if_none_left(gs: &mut GameState, owner_id: usize) {
    if !Character::is_sane_character(owner_id) || owner_has_live_companion(gs, owner_id) {
        return;
    }
    if remove_active_spell_temp(gs, owner_id, SK_REVENANT_CONDUIT2 as u16) {
        gs.do_character_log(
            owner_id,
            FontColor::Green,
            "With your ghost companion gone, the conduit closes.\n",
        );
        chlog!(owner_id, "Revenant Conduit closed (companion lost)");
    }
    if remove_active_spell_temp(gs, owner_id, SK_SPECTRAL_PACT2 as u16) {
        gs.do_character_log(
            owner_id,
            FontColor::Green,
            "With your ghost companion gone, the spectral pact fades.\n",
        );
        chlog!(owner_id, "Spectral Pact faded (companion lost)");
    }
}

/// Active self-buff: Revenant Conduit. Spends mana up front and attaches a
/// spell item that boosts the caster's effective Ghost Companion skill while
/// draining endurance over time (the drain is processed in `state/stats.rs`).
///
/// The boost applies both to companions summoned while the conduit is open
/// (checked in `skill_ghost`) and to any companions the caster already has
/// (re-statted on cast). Recasting while the conduit is open toggles it off
/// and re-stats existing companions back down.
///
/// # Arguments
///
/// * `gs` - Game state.
/// * `cn` - Caster character index.
pub fn skill_revenant_conduit(gs: &mut GameState, cn: usize) {
    // Recasting while the conduit is open closes it (toggle off). The boost is
    // removed, so re-stat existing companions back down immediately.
    if has_active_spell_temp(gs, cn, SK_REVENANT_CONDUIT2 as u16) {
        remove_active_spell_temp(gs, cn, SK_REVENANT_CONDUIT2 as u16);
        restat_owned_companions(gs, cn);
        gs.do_character_log(cn, FontColor::Green, "The conduit closes.\n");
        chlog!(cn, "Uncast Revenant Conduit");
        return;
    }
    if gs.characters[cn].a_end < 25 * 1000 {
        gs.do_character_log(cn, FontColor::Red, "You're too exhausted!\n");
        return;
    }
    if spellcost(gs, cn, 17) != 0 {
        return;
    }

    let power = i32::from(gs.characters[cn].skill[SK_REVENANT_CONDUIT][5]);
    // +X% effective SK_GHOST per power tier, capped at +150%.
    let boost_pct = (((power / 10) * 10) * 3).clamp(30, 150);
    let duration = TICKS * 360;

    let in_opt = God::create_item(gs, 1);
    if in_opt.is_none() {
        log::error!("god_create_item failed in skill_revenant_conduit");
        return;
    }
    let in_idx = in_opt.unwrap();

    let item = &mut gs.items[in_idx];
    let mut name_bytes = [0u8; 40];
    let name = b"Revenant Conduit";
    let nlen = name.len().min(40);
    name_bytes[..nlen].copy_from_slice(&name[..nlen]);
    item.name = name_bytes;
    item.flags |= ItemFlags::IF_SPELL.bits();
    item.sprite[1] = 96;
    item.duration = duration as u32;
    item.active = duration as u32;
    item.temp = SK_REVENANT_CONDUIT2 as u16;
    item.power = boost_pct.max(1) as u32;

    if add_spell(gs, cn, in_idx) == 0 {
        gs.do_character_log(
            cn,
            FontColor::Green,
            "Magical interference closes the conduit.\n",
        );
        return;
    }

    // Boost any companions the caster already has, not just future summons.
    restat_owned_companions(gs, cn);

    gs.do_character_log(
        cn,
        FontColor::Green,
        "Cold spectral energy flows through you.\n",
    );
    chlog!(cn, "Cast Revenant Conduit (+{}% Ghost)", boost_pct);
    EffectManager::fx_add_effect(
        gs,
        7,
        0,
        i32::from(gs.characters[cn].x),
        i32::from(gs.characters[cn].y),
        0,
    );
}

/// Passive: Kindred Spirit. Cannot be directly invoked; checked by
/// `skill_ghost` to allow a second simultaneous companion.
///
/// # Arguments
///
/// * `gs` - Game state.
/// * `cn` - Caster character index (used only for the help-text reply).
pub fn skill_kindred_spirit(gs: &mut GameState, cn: usize) {
    gs.do_character_log(
        cn,
        FontColor::Green,
        "You use this skill automatically when you summon a ghost companion.\n",
    );
}

/// Passive: Spellcaster Kindred Spirit. Cannot be directly invoked; ghost
/// companion AI checks this skill automatically.
///
/// # Arguments
///
/// * `gs` - Game state.
/// * `cn` - Caster character index (used only for the help-text reply).
pub fn skill_spellcaster_kindred_spirit(gs: &mut GameState, cn: usize) {
    gs.do_character_log(
        cn,
        FontColor::Green,
        "You use this skill automatically through your ghost companions.\n",
    );
}

/// Active self-buff: Spectral Pact. Attaches a spell item that redirects a
/// percentage of incoming damage from the caster to their ghost companion(s)
/// (handled in `do_hurt`).
///
/// # Arguments
///
/// * `gs` - Game state.
/// * `cn` - Caster character index.
pub fn skill_spectral_pact(gs: &mut GameState, cn: usize) {
    if has_active_spell_temp(gs, cn, SK_SPECTRAL_PACT2 as u16) {
        gs.do_character_log(cn, FontColor::Red, "The pact is already in force.\n");
        return;
    }
    if spellcost(gs, cn, 20) != 0 {
        return;
    }
    let power = i32::from(gs.characters[cn].skill[SK_SPECTRAL_PACT][5]);
    let redirect_pct = ((10 + (power * 40) / 100) * 3).clamp(30, 150);
    let duration = TICKS * 180;

    let in_opt = God::create_item(gs, 1);
    if in_opt.is_none() {
        log::error!("god_create_item failed in skill_spectral_pact");
        return;
    }
    let in_idx = in_opt.unwrap();

    let item = &mut gs.items[in_idx];
    let mut name_bytes = [0u8; 40];
    let name = b"Spectral Pact";
    let nlen = name.len().min(40);
    name_bytes[..nlen].copy_from_slice(&name[..nlen]);
    item.name = name_bytes;
    item.flags |= ItemFlags::IF_SPELL.bits();
    item.sprite[1] = 88; // TODO: assign unique spell-effect sprite ID once buff sprite assets are finalized
    item.duration = duration as u32;
    item.active = duration as u32;
    item.temp = SK_SPECTRAL_PACT2 as u16;
    item.power = redirect_pct.max(1) as u32;

    if add_spell(gs, cn, in_idx) == 0 {
        gs.do_character_log(
            cn,
            FontColor::Green,
            "Magical interference breaks the pact.\n",
        );
        return;
    }

    gs.do_character_log(
        cn,
        FontColor::Green,
        &format!(
            "You bind your fate to your ghosts ({}% redirect).\n",
            redirect_pct
        ),
    );
    chlog!(cn, "Cast Spectral Pact ({}% redirect)", redirect_pct);
    EffectManager::fx_add_effect(
        gs,
        7,
        0,
        i32::from(gs.characters[cn].x),
        i32::from(gs.characters[cn].y),
        0,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::driver::skill::blast::spellcost_blast;
    use crate::driver::skill::tests::{active_spell_item, attach_marker};
    use crate::test_helpers::{add_test_player, with_test_gs};
    use core::constants::USE_ACTIVE;

    /// Player owner with neutral spell modifiers and a usable spell template.
    fn owner(gs: &mut GameState) -> usize {
        let (cn, _nr) = add_test_player(gs);
        gs.item_templates[1].used = USE_ACTIVE;
        gs.globals.newmoon = 0;
        gs.globals.fullmoon = 0;
        gs.characters[cn].kindred = 0;
        cn
    }

    /// Live companion at index 2 owned by `owner`, with stat caps high enough
    /// that `recompute_companion_stats` results are not clamped.
    fn add_companion(gs: &mut GameState, owner: usize) -> usize {
        let cc = 2;
        gs.characters[cc] = core::types::Character::default();
        gs.characters[cc].used = USE_ACTIVE;
        gs.characters[cc].temp = CT_COMPANION as u16;
        gs.characters[cc].data[63] = owner as i32;
        for n in 0..5 {
            gs.characters[cc].attrib[n][2] = 250;
            gs.characters[cc].attrib[n][3] = 1;
        }
        gs.characters[cc].hp[2] = 1000;
        gs.characters[cc].end[2] = 1000;
        gs.characters[owner].data[CHD_COMPANION] = cc as i32;
        cc
    }

    #[test]
    fn spellcaster_kindred_blast_spends_companion_endurance() {
        with_test_gs(|gs| {
            let (owner, _nr) = add_test_player(gs);
            let companion = 2;
            gs.characters[owner].skill[SK_SPELLCASTER_KINDRED_SPIRIT][0] = 1;
            gs.characters[companion] = core::types::Character::default();
            gs.characters[companion].used = USE_ACTIVE;
            gs.characters[companion].temp = core::constants::CT_COMPANION as u16;
            gs.characters[companion].data[63] = owner as i32;
            gs.characters[companion].a_end = 100_000;
            gs.characters[companion].a_mana = 100_000;

            assert_eq!(spellcost_blast(gs, companion, 10), 0);

            assert_eq!(gs.characters[companion].a_end, 90_000);
            assert_eq!(gs.characters[companion].a_mana, 100_000);
        });
    }

    #[test]
    fn clear_companion_dependent_buffs_removed_when_no_companion_left() {
        with_test_gs(|gs| {
            let (owner, _nr) = add_test_player(gs);
            attach_marker(gs, owner, 0, 10, SK_REVENANT_CONDUIT2 as u16);
            attach_marker(gs, owner, 1, 11, SK_SPECTRAL_PACT2 as u16);

            clear_companion_dependent_buffs_if_none_left(gs, owner);

            assert!(!has_active_spell_temp(
                gs,
                owner,
                SK_REVENANT_CONDUIT2 as u16
            ));
            assert!(!has_active_spell_temp(gs, owner, SK_SPECTRAL_PACT2 as u16));
        });
    }

    #[test]
    fn clear_companion_dependent_buffs_kept_while_a_companion_is_alive() {
        with_test_gs(|gs| {
            let (owner, _nr) = add_test_player(gs);
            attach_marker(gs, owner, 0, 10, SK_REVENANT_CONDUIT2 as u16);

            let companion = 2;
            gs.characters[companion] = core::types::Character::default();
            gs.characters[companion].used = USE_ACTIVE;
            gs.characters[companion].temp = CT_COMPANION as u16;
            gs.characters[companion].data[63] = owner as i32;
            gs.characters[owner].data[CHD_COMPANION] = companion as i32;

            clear_companion_dependent_buffs_if_none_left(gs, owner);

            assert!(has_active_spell_temp(
                gs,
                owner,
                SK_REVENANT_CONDUIT2 as u16
            ));
        });
    }

    #[test]
    fn owner_has_live_companion_ignores_bodies_and_foreign_companions() {
        with_test_gs(|gs| {
            let cn = owner(gs);
            assert!(!owner_has_live_companion(gs, cn));

            let cc = add_companion(gs, cn);
            assert!(owner_has_live_companion(gs, cn));

            gs.characters[cc].flags |= CharacterFlags::Body.bits();
            assert!(!owner_has_live_companion(gs, cn));

            gs.characters[cc].flags &= !CharacterFlags::Body.bits();
            gs.characters[cc].data[63] = 99;
            assert!(!owner_has_live_companion(gs, cn));
        });
    }

    #[test]
    fn restat_owned_companions_scales_stats_from_ghost_skill() {
        with_test_gs(|gs| {
            let cn = owner(gs);
            let cc = add_companion(gs, cn);
            gs.characters[cn].skill[SK_GHOST][5] = 110;

            restat_owned_companions(gs, cn);

            // base = 110 * 4 / 11 = 40; attributes are base * 3 / attrib[3].
            assert!(gs.characters[cc].attrib.iter().all(|a| a[0] == 120));
            assert_eq!(gs.characters[cc].hp[0], 200);
            assert_eq!(gs.characters[cc].end[0], 200);
            assert_eq!(gs.characters[cc].mana[0], 0);
        });
    }

    #[test]
    fn revenant_conduit_boosts_existing_companions_and_toggles_off() {
        with_test_gs(|gs| {
            let cn = owner(gs);
            let cc = add_companion(gs, cn);
            gs.characters[cn].skill[SK_GHOST][5] = 110;
            gs.characters[cn].skill[SK_REVENANT_CONDUIT][5] = 20;
            gs.characters[cn].a_end = 30_000;
            gs.characters[cn].a_mana = 50_000;

            skill_revenant_conduit(gs, cn);

            let in_ = active_spell_item(gs, cn, SK_REVENANT_CONDUIT2).expect("conduit");
            assert_eq!(gs.items[in_].power, 60);
            assert_eq!(gs.items[in_].active, (TICKS * 360) as u32);
            assert_eq!(gs.characters[cn].a_mana, 33_000);
            // Endurance is only checked here; the drain happens over time.
            assert_eq!(gs.characters[cn].a_end, 30_000);
            // 110 * 160% = 176 -> base 64 -> attributes 192.
            assert_eq!(gs.characters[cc].attrib[0][0], 192);

            skill_revenant_conduit(gs, cn);

            assert!(active_spell_item(gs, cn, SK_REVENANT_CONDUIT2).is_none());
            assert_eq!(gs.characters[cn].a_mana, 33_000);
            assert_eq!(gs.characters[cc].attrib[0][0], 120);
        });
    }

    #[test]
    fn revenant_conduit_boost_is_clamped() {
        with_test_gs(|gs| {
            let cn = owner(gs);
            gs.characters[cn].a_end = 30_000;
            gs.characters[cn].a_mana = 100_000;

            gs.characters[cn].skill[SK_REVENANT_CONDUIT][5] = 5;
            skill_revenant_conduit(gs, cn);
            let in_ = active_spell_item(gs, cn, SK_REVENANT_CONDUIT2).expect("conduit");
            assert_eq!(gs.items[in_].power, 30);
            skill_revenant_conduit(gs, cn);

            gs.characters[cn].skill[SK_REVENANT_CONDUIT][5] = 200;
            skill_revenant_conduit(gs, cn);
            let in_ = active_spell_item(gs, cn, SK_REVENANT_CONDUIT2).expect("conduit");
            assert_eq!(gs.items[in_].power, 150);
        });
    }

    #[test]
    fn revenant_conduit_requires_endurance() {
        with_test_gs(|gs| {
            let cn = owner(gs);
            gs.characters[cn].skill[SK_REVENANT_CONDUIT][5] = 20;
            gs.characters[cn].a_end = 24_000;
            gs.characters[cn].a_mana = 50_000;

            skill_revenant_conduit(gs, cn);

            assert!(active_spell_item(gs, cn, SK_REVENANT_CONDUIT2).is_none());
            assert_eq!(gs.characters[cn].a_mana, 50_000);
        });
    }

    #[test]
    fn spectral_pact_sets_redirect_percent_and_cannot_stack() {
        with_test_gs(|gs| {
            let cn = owner(gs);
            gs.characters[cn].skill[SK_SPECTRAL_PACT][5] = 50;
            gs.characters[cn].a_mana = 50_000;

            skill_spectral_pact(gs, cn);

            let in_ = active_spell_item(gs, cn, SK_SPECTRAL_PACT2).expect("pact");
            assert_eq!(gs.items[in_].power, 90);
            assert_eq!(gs.items[in_].active, (TICKS * 180) as u32);
            assert_eq!(gs.characters[cn].a_mana, 30_000);

            skill_spectral_pact(gs, cn);
            assert_eq!(gs.characters[cn].a_mana, 30_000);
            assert_eq!(active_spell_item(gs, cn, SK_SPECTRAL_PACT2), Some(in_));
        });
    }

    #[test]
    fn skill_ghost_is_refused_in_build_mode_and_when_slots_are_full() {
        with_test_gs(|gs| {
            let cn = owner(gs);
            gs.characters[cn].a_mana = 100_000;

            gs.characters[cn].flags |= CharacterFlags::BuildMode.bits();
            skill_ghost(gs, cn);
            assert_eq!(gs.characters[cn].a_mana, 100_000);
            gs.characters[cn].flags &= !CharacterFlags::BuildMode.bits();

            add_companion(gs, cn);
            skill_ghost(gs, cn);
            assert_eq!(gs.characters[cn].a_mana, 100_000);
            assert_eq!(gs.characters[cn].data[CHD_COMPANION], 2);
        });
    }

    #[test]
    fn skill_ghost_is_refused_in_the_gatekeeper_room() {
        with_test_gs(|gs| {
            let cn = owner(gs);
            gs.characters[cn].x = 43;
            gs.characters[cn].y = 597;
            gs.characters[cn].a_mana = 100_000;

            skill_ghost(gs, cn);

            // The room check runs after the mana cost, so the cast is paid for.
            assert_eq!(gs.characters[cn].a_mana, 55_000);
            assert_eq!(gs.characters[cn].data[CHD_COMPANION], 0);
            assert_eq!(gs.characters[cn].data[CHD_COMPANION2], 0);
        });
    }
}
