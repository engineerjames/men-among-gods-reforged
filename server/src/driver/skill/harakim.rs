use super::common::{
    add_exhaust, add_spell, chance_base, effective_resist, is_exhausted, spell_immunity,
    spell_race_mod, spellcost,
};
use super::stun::spell_stun;
use super::{hostile_cast_preflight, resolve_offensive_target};
use crate::chlog;
use crate::effect::EffectManager;
use crate::game_state::{ElementSwitchState, GameState};
use crate::god::God;
use crate::helpers;
use core::constants::{CharacterFlags, ItemFlags, NT_DIDHIT, NT_GOTHIT, TICKS, USE_EMPTY};
use core::skills::{
    SK_ANGUISH_EARTH, SK_ANGUISH_ICE, SK_ANGUISH_LAVA, SK_CURSE, SK_DISPEL, SK_ELEMENT_SWITCHING,
    SK_ICE_STUN, SK_IMMUN, SK_LAVA_BLAST, SK_RESIST, SK_SENSE, SK_WIMPY,
};
use core::string_operations::c_string_to_str;
use core::talent_trees::harakim;
use core::traits::{Class, KIN_ARCHHARAKIM, KIN_HARAKIM};
use core::types::FontColor;

/// Element marker value for Lava-element Harakim casts.
const HARAKIM_ELEMENT_LAVA: u32 = 1;
/// Element marker value for Ice-element Harakim casts.
const HARAKIM_ELEMENT_ICE: u32 = 2;
/// Element marker value for Earth-element Harakim casts.
const HARAKIM_ELEMENT_EARTH: u32 = 3;
/// Percent damage boost granted when Element Switching alternates elements.
const ELEMENT_SWITCH_DAMAGE_PERCENT: i32 = 20;
/// Duration, in ticks, that Element Switching remembers the last cast element.
const ELEMENT_SWITCH_MEMORY_TICKS: i32 = TICKS * 60;

/// Returns whether an elemental cast can use the Harakim Element Switching modifier.
///
/// # Arguments
///
/// * `gs` - Active game state containing caster talent state.
/// * `cn` - Caster character index.
///
/// # Returns
///
/// * `true` when the caster is a Harakim player with Element Switching learned.
fn has_element_switching_modifier(gs: &GameState, cn: usize) -> bool {
    (gs.characters[cn].flags & CharacterFlags::Player.bits()) != 0
        && matches!(
            Class::from(gs.characters[cn].kindred),
            Class::Harakim | Class::ArchHarakim
        )
        && harakim::has_element_switching(&gs.characters[cn].future1)
}

/// Records a Harakim elemental cast and reports whether it changed element.
///
/// # Arguments
///
/// * `gs` - Active game state containing runtime Element Switching state.
/// * `cn` - Caster character index.
/// * `element` - Element marker value being cast now.
///
/// # Returns
///
/// * `true` when the caster has Element Switching and the previous element differs.
fn record_harakim_element_cast(gs: &mut GameState, cn: usize, element: u32) -> bool {
    if !has_element_switching_modifier(gs, cn) {
        return false;
    }

    let current_tick = gs.globals.ticker;
    let changed = matches!(
        gs.element_switch_states.get(&cn),
        Some(state) if state.expires_at_tick > current_tick
            && state.last_element != 0
            && state.last_element != element
    );

    gs.element_switch_states.insert(
        cn,
        ElementSwitchState {
            last_element: element,
            expires_at_tick: current_tick + ELEMENT_SWITCH_MEMORY_TICKS,
        },
    );

    if changed {
        show_element_switching_proc_icon(gs, cn);
    }

    changed
}

/// Refreshes the client-visible Element Switching proc icon on the caster.
///
/// # Arguments
///
/// * `gs` - Active game state containing spell items and caster state.
/// * `cn` - Caster character index.
fn show_element_switching_proc_icon(gs: &mut GameState, cn: usize) {
    for n in 0..20 {
        let in_idx = gs.characters[cn].spell[n] as usize;
        if in_idx != 0 && gs.items[in_idx].temp == SK_ELEMENT_SWITCHING as u16 {
            gs.items[in_idx].active = gs.items[in_idx].duration;
            return;
        }
    }

    let in_opt = God::create_item(gs, 1);
    if in_opt.is_none() {
        log::error!("god_create_item failed in show_element_switching_proc_icon");
        return;
    }
    let in_idx = in_opt.unwrap();
    {
        let item = &mut gs.items[in_idx];
        let mut name_bytes = [0u8; 40];
        let name = b"Element Switching";
        let nlen = name.len().min(40);
        name_bytes[..nlen].copy_from_slice(&name[..nlen]);
        item.name = name_bytes;
        item.flags |= ItemFlags::IF_SPELL.bits();
        item.duration = (TICKS * 10) as u32;
        item.active = item.duration;
        item.temp = SK_ELEMENT_SWITCHING as u16;
        item.power = ELEMENT_SWITCH_DAMAGE_PERCENT as u32;
    }
    if add_spell(gs, cn, in_idx) == 0 {
        for slot in 0..20 {
            if gs.characters[cn].spell[slot] == 0 {
                gs.characters[cn].spell[slot] = in_idx as u32;
                break;
            }
        }
    }
}

/// Applies Element Switching's damage bonus when the caster alternates elements.
///
/// # Arguments
///
/// * `gs` - Active game state containing runtime Element Switching state.
/// * `cn` - Caster character index.
/// * `element` - Element marker value being cast now.
/// * `damage` - Base damage or damage-driving power.
///
/// # Returns
///
/// * Boosted damage when the element changed, otherwise `damage` unchanged.
fn apply_harakim_element_damage_bonus(
    gs: &mut GameState,
    cn: usize,
    element: u32,
    damage: i32,
) -> i32 {
    if record_harakim_element_cast(gs, cn, element) {
        damage * (100 + ELEMENT_SWITCH_DAMAGE_PERCENT) / 100
    } else {
        damage
    }
}

/// Attaches Lava Blast's burning damage-over-time marker to an impacted enemy.
///
/// # Arguments
///
/// * `gs` - Active game state used to allocate and attach the spell item.
/// * `caster` - Character index of the Lava Blast caster.
/// * `co` - Character index of the impacted enemy.
/// * `power` - Effective Lava Blast power recorded for tick damage.
fn apply_lava_blast_dot(gs: &mut GameState, caster: usize, co: usize, power: i32) {
    let in_opt = God::create_item(gs, 1);
    if in_opt.is_none() {
        log::error!("god_create_item failed in apply_lava_blast_dot");
        return;
    }
    let in_idx = in_opt.unwrap();
    {
        let item = &mut gs.items[in_idx];
        let mut name_bytes = [0u8; 40];
        let name = b"Lava Burn";
        let nlen = name.len().min(40);
        name_bytes[..nlen].copy_from_slice(&name[..nlen]);
        item.name = name_bytes;
        item.flags |= ItemFlags::IF_SPELL.bits();
        item.sprite[1] = 89;
        item.duration = (TICKS * 15) as u32;
        item.active = item.duration;
        item.temp = SK_LAVA_BLAST as u16;
        item.power = power.max(1) as u32;
        item.data[0] = caster as u32;
    }
    add_spell(gs, co, in_idx);
}

/// Active hostile cast: Lava Blast. Deals Blast-like damage and burns every
/// impacted enemy for a short duration.
///
/// # Arguments
///
/// * `gs` - Active game state used for target validation, mana costs, damage, and effects.
/// * `cn` - Caster character index.
pub fn skill_lava_blast(gs: &mut GameState, cn: usize) {
    let co = resolve_offensive_target(gs, cn);
    if !hostile_cast_preflight(gs, cn, co, "You cannot lava blast yourself.\n") {
        return;
    }
    if is_exhausted(gs, cn) {
        return;
    }

    let mut power = i32::from(gs.characters[cn].skill[SK_LAVA_BLAST][5]);
    power = spell_immunity(gs, power, i32::from(gs.characters[co].skill[SK_IMMUN][5]));
    power = spell_race_mod(gs, power, gs.characters[cn].kindred);
    let mut cost = ((power * 3) / 2) / 8 + 8;
    if (gs.characters[cn].flags & CharacterFlags::Player.bits()) != 0
        && ((gs.characters[cn].kindred as u32) & (KIN_HARAKIM | KIN_ARCHHARAKIM) != 0)
    {
        cost /= 3;
    }
    if spellcost(gs, cn, cost) != 0 {
        return;
    }

    // Direct hit matches Blast (`compute_blast_damage`); the burn is the extra.
    let mut dam = power * 2;

    if chance_base(
        gs,
        cn,
        i32::from(gs.characters[cn].skill[SK_LAVA_BLAST][5]),
        12,
        effective_resist(gs, cn, i32::from(gs.characters[co].skill[SK_RESIST][5])),
    ) != 0
    {
        return;
    }

    dam = apply_harakim_element_damage_bonus(gs, cn, HARAKIM_ELEMENT_LAVA, dam);

    let tmp = gs.do_hurt(cn, co, dam, 1);
    if tmp < 1 {
        gs.do_character_log(
            cn,
            FontColor::Green,
            "You cannot penetrate your target's armor.\n",
        );
    } else {
        gs.do_character_log(
            cn,
            FontColor::Green,
            &format!("You lava blast your target for {} HP.\n", tmp),
        );
        apply_lava_blast_dot(gs, cn, co, power);
    }
    EffectManager::fx_add_effect(
        gs,
        5,
        0,
        i32::from(gs.characters[co].x),
        i32::from(gs.characters[co].y),
        0,
    );

    let co_orig = co;
    dam = dam / 2 + dam / 4;
    let aoe_base = i32::from(gs.characters[cn].skill[SK_LAVA_BLAST][0]);
    let use_legacy_cross = helpers::skill_aoe_uses_legacy_cross(aoe_base);
    let caster_x = i32::from(gs.characters[cn].x);
    let caster_y = i32::from(gs.characters[cn].y);

    for maybe_co in helpers::skill_aoe_targets(gs, Some(cn), caster_x, caster_y, aoe_base) {
        if maybe_co == cn || maybe_co == co_orig {
            continue;
        }
        if use_legacy_cross && gs.characters[maybe_co].attack_cn != cn as u16 {
            continue;
        }
        if !gs.may_aoe_attack(cn, maybe_co) {
            continue;
        }
        gs.remember_pvp(cn, maybe_co);
        let tmp2 = gs.do_hurt(cn, maybe_co, dam, 1);
        if tmp2 >= 1 {
            gs.do_character_log(
                cn,
                FontColor::Green,
                &format!("You lava blast your target for {} HP.\n", tmp2),
            );
            apply_lava_blast_dot(gs, cn, maybe_co, power);
        }
        EffectManager::fx_add_effect(
            gs,
            5,
            0,
            i32::from(gs.characters[maybe_co].x),
            i32::from(gs.characters[maybe_co].y),
            0,
        );
    }

    add_exhaust(gs, cn, TICKS * 6);
    EffectManager::fx_add_effect(
        gs,
        7,
        0,
        i32::from(gs.characters[cn].x),
        i32::from(gs.characters[cn].y),
        0,
    );
}

/// Handles direct player/NPC use of the Ice Stun skill.
///
/// Ice Stun wholly replaces Stun for Harakim who learn the talent: it stuns the
/// primary target and adjacent attackers exactly like Stun, but draws its power
/// from `SK_ICE_STUN` (including the Harakim ice element bonus) and marks every
/// frozen target so it bursts with ice when it dies.
///
/// # Arguments
///
/// * `gs` - Active game state used for target validation, combat checks, and spell application.
/// * `cn` - Caster character index.
///
/// # Panics
///
/// * Panics if `cn`, the selected target index, or an adjacent target index is invalid.
pub fn skill_ice_stun(gs: &mut GameState, cn: usize) {
    let co = resolve_offensive_target(gs, cn);
    if !hostile_cast_preflight(gs, cn, co, "You cannot freeze yourself!\n") {
        return;
    }
    if is_exhausted(gs, cn) {
        return;
    }
    if spellcost(gs, cn, 20) != 0 {
        return;
    }

    if chance_base(
        gs,
        cn,
        i32::from(gs.characters[cn].skill[SK_ICE_STUN][5]),
        12,
        effective_resist(gs, cn, i32::from(gs.characters[co].skill[SK_RESIST][5])),
    ) != 0
    {
        if gs.characters[co].skill[SK_SENSE][5] > gs.characters[cn].skill[SK_ICE_STUN][5] + 5 {
            gs.do_character_log(
                co,
                FontColor::Green,
                &format!(
                    "{} tried to cast ice stun on you but failed.\n",
                    c_string_to_str(&gs.characters[cn].reference)
                ),
            );
            if gs.characters[co].flags & CharacterFlags::SpellIgnore.bits() == 0 {
                gs.do_notify_character(
                    co as u32,
                    i32::from(core::constants::NT_GOTMISS),
                    cn as i32,
                    0,
                    0,
                    0,
                );
            }
        }
        return;
    }

    if (gs.characters[co].flags & CharacterFlags::Immortal.bits()) != 0 {
        gs.do_character_log(cn, FontColor::Red, "You lost your focus.\n");
        return;
    }

    let power = i32::from(gs.characters[cn].skill[SK_ICE_STUN][5]);
    let burst_power = apply_harakim_element_damage_bonus(gs, cn, HARAKIM_ELEMENT_ICE, power);
    if spell_stun(gs, cn, co, power) {
        attach_ice_stun_marker(gs, cn, co, burst_power);
    }

    let co_orig = co;
    let m: usize = gs.characters[cn].x as usize
        + gs.characters[cn].y as usize * core::constants::SERVER_MAPX as usize;

    let adj = [
        1isize,
        -1isize,
        core::constants::SERVER_MAPX as isize,
        -(core::constants::SERVER_MAPX as isize),
    ];
    for delta in adj.iter() {
        let idx = (m as isize + *delta) as usize;
        let maybe_co = gs.map.get(idx).map(|m| m.ch).unwrap_or(0) as usize;
        if maybe_co != 0 && gs.characters[maybe_co].attack_cn == cn as u16 && maybe_co != co_orig {
            let s_rand = helpers::random_mod_i32(20);
            let o_rand = helpers::random_mod_i32(20);
            if power + s_rand
                > effective_resist(
                    gs,
                    cn,
                    i32::from(gs.characters[maybe_co].skill[SK_RESIST][5]),
                ) + o_rand
                && spell_stun(gs, cn, maybe_co, power)
            {
                attach_ice_stun_marker(gs, cn, maybe_co, burst_power);
            }
        }
    }

    EffectManager::fx_add_effect(
        gs,
        7,
        0,
        i32::from(gs.characters[cn].x),
        i32::from(gs.characters[cn].y),
        0,
    );
    add_exhaust(gs, cn, core::constants::TICKS * 3);
}

/// Attaches Ice Stun's on-death burst marker to a stunned target.
///
/// # Arguments
///
/// * `gs` - Active game state used to allocate and attach the marker.
/// * `caster` - Character index of the Ice Stun caster.
/// * `co` - Character index of the stunned target.
/// * `power` - Effective Ice Stun power recorded for death-burst damage.
fn attach_ice_stun_marker(gs: &mut GameState, caster: usize, co: usize, power: i32) {
    let in_opt = God::create_item(gs, 1);
    if in_opt.is_none() {
        log::error!("god_create_item failed in attach_ice_stun_marker");
        return;
    }
    let in_idx = in_opt.unwrap();
    {
        let item = &mut gs.items[in_idx];
        let mut name_bytes = [0u8; 40];
        let name = b"Ice Stun";
        let nlen = name.len().min(40);
        name_bytes[..nlen].copy_from_slice(&name[..nlen]);
        item.name = name_bytes;
        item.flags |= ItemFlags::IF_SPELL.bits();
        item.sprite[1] = 91;
        item.duration = (power + TICKS).max(TICKS) as u32;
        item.active = item.duration;
        item.temp = SK_ICE_STUN as u16;
        item.power = power.max(1) as u32;
        item.data[0] = caster as u32;
    }
    if add_spell(gs, co, in_idx) != 0 {
        gs.do_character_log(co, FontColor::Green, "Ice encases your body!\n");
        gs.do_character_log(
            caster,
            FontColor::Green,
            &format!(
                "{} was encased in ice.\n",
                c_string_to_str(&gs.characters[co].reference)
            ),
        );
        EffectManager::fx_add_effect(
            gs,
            5,
            0,
            i32::from(gs.characters[co].x),
            i32::from(gs.characters[co].y),
            0,
        );
    }
}

/// Handles direct player/NPC use of the Dispel Magic skill.
///
/// Chooses a curse first, then the first non-Guardian-Angel spell, and treats
/// hostile dispels as attacks where appropriate.
///
/// # Arguments
///
/// * `gs` - Active game state used for target validation, spell removal, and feedback.
/// * `cn` - Caster character index.
///
/// # Panics
///
/// * Panics if `cn`, the selected target index, or an active spell item index is invalid.
pub fn skill_dispel(gs: &mut GameState, cn: usize) {
    // Port of C `skill_dispel(int cn)`.
    let target = gs.characters[cn].skill_target1 as usize;
    let co = if target != 0 { target } else { cn };

    if gs.do_char_can_see(cn, co) == 0 {
        gs.do_character_log(cn, FontColor::Red, "You cannot see your target.\n");
        return;
    }

    if is_exhausted(gs, cn) {
        return;
    }

    // Select which spell slot to remove.
    let mut slot: Option<usize> = None;

    // 1) Prefer removing curse from target.
    for n in 0..20usize {
        let in_idx = gs.characters[co].spell[n] as usize;
        if in_idx == 0 {
            continue;
        }
        if gs.items[in_idx].temp == SK_CURSE as u16 {
            slot = Some(n);
            break;
        }
    }

    // 2) If no curse found, remove first non-wimpy spell.
    if slot.is_none() {
        for n in 0..20usize {
            let in_idx = gs.characters[co].spell[n] as usize;
            if in_idx == 0 {
                continue;
            }
            let temp = gs.items[in_idx].temp;
            if temp == SK_WIMPY as u16 {
                continue;
            }
            slot = Some(n);
            break;
        }

        // No target spell found.
        if slot.is_none() {
            if co == cn {
                gs.do_character_log(cn, FontColor::Red, "But you aren't spelled!\n");
            } else {
                let name = gs.characters[co].get_name().to_owned();
                gs.do_character_log(cn, FontColor::Red, &format!("{} isn't spelled!\n", name));
            }
            return;
        }

        // Dispelling someone else's non-curse spell is treated like an attack.
        if target != 0 && !gs.may_attack_msg(cn, co, true) {
            chlog!(
                cn,
                "Prevented from dispelling {}",
                gs.characters[co].get_name().to_owned()
            );
            return;
        }
    }

    let slot = slot.expect("slot must be set");
    let in_idx = gs.characters[co].spell[slot] as usize;
    if in_idx == 0 {
        return;
    }

    let pwr = gs.items[in_idx].power as i32;

    if spellcost(gs, cn, 25) != 0 {
        return;
    }

    let dispel_skill = i32::from(gs.characters[cn].skill[SK_DISPEL][5]);
    let kindred = gs.characters[cn].kindred;
    if chance_base(gs, cn, spell_race_mod(gs, dispel_skill, kindred), 12, pwr) != 0 {
        if cn != co {
            let sense = i32::from(gs.characters[co].skill[SK_SENSE][5]);
            if sense > dispel_skill + 5 {
                let reference = gs.characters[cn].reference;
                gs.do_character_log(
                    co,
                    FontColor::Green,
                    &format!(
                        "{} tried to cast dispel magic on you but failed.\n",
                        c_string_to_str(&reference)
                    ),
                );
            }
        }
        return;
    }

    let removed_temp = gs.items[in_idx].temp;
    let removed_name = gs.items[in_idx].get_name().to_owned();

    // Remove the spell item and unlink it from the target.
    gs.items[in_idx].used = USE_EMPTY;
    gs.characters[co].spell[slot] = 0;
    gs.do_update_char(co);

    // Remember PvP attacks when dispelling non-curse from someone else.
    if target != 0 && removed_temp != SK_CURSE as u16 {
        gs.remember_pvp(cn, co);
    }

    let sound = i32::from(gs.characters[cn].sound);

    if target != 0 {
        let sense = i32::from(gs.characters[co].skill[SK_SENSE][5]);
        if sense + 10 > dispel_skill {
            let reference = gs.characters[cn].reference;
            gs.do_character_log(
                co,
                FontColor::Green,
                &format!(
                    "{} cast dispel magic on you.\n",
                    c_string_to_str(&reference)
                ),
            );
        } else {
            gs.do_character_log(
                co,
                FontColor::Green,
                &format!("{} has been removed.\n", removed_name),
            );
        }

        let target_name = gs.characters[co].get_name().to_owned();
        gs.do_character_log(
            cn,
            FontColor::Green,
            &format!("Removed {} from {}.\n", removed_name, target_name),
        );

        let target_is_player = (gs.characters[co].flags & CharacterFlags::Player.bits()) != 0;
        if removed_temp != SK_CURSE as u16 && !target_is_player {
            if (gs.characters[co].flags & CharacterFlags::SpellIgnore.bits()) == 0 {
                gs.do_notify_character(co as u32, i32::from(NT_GOTHIT), cn as i32, 0, 0, 0);
            }
            gs.do_notify_character(cn as u32, i32::from(NT_DIDHIT), co as i32, 0, 0, 0);
        }

        GameState::char_play_sound(gs, co, sound + 1, -150, 0);
        GameState::char_play_sound(gs, cn, sound + 1, -150, 0);
        chlog!(
            cn,
            "Cast Dispel on {}",
            gs.characters[co].get_name().to_owned()
        );
        EffectManager::fx_add_effect(
            gs,
            6,
            0,
            i32::from(gs.characters[co].x),
            i32::from(gs.characters[co].y),
            0,
        );
    } else {
        gs.do_character_log(
            cn,
            FontColor::Green,
            &format!("{} has been removed.\n", removed_name),
        );
        GameState::char_play_sound(gs, cn, sound + 1, -150, 0);
        chlog!(cn, "Cast Dispel");
        EffectManager::fx_add_effect(
            gs,
            6,
            0,
            i32::from(gs.characters[cn].x),
            i32::from(gs.characters[cn].y),
            0,
        );
    }

    add_exhaust(gs, cn, TICKS * 2);
    EffectManager::fx_add_effect(
        gs,
        7,
        0,
        i32::from(gs.characters[cn].x),
        i32::from(gs.characters[cn].y),
        0,
    );
}

/// Internal: shared preflight + resistance roll for the three Anguish casts.
///
/// Returns the resolved target on success, otherwise `None` (with any user
/// messaging already emitted).
fn anguish_preflight(
    gs: &mut GameState,
    cn: usize,
    skill_const: usize,
    cost: i32,
) -> Option<usize> {
    let co = resolve_offensive_target(gs, cn);
    if !hostile_cast_preflight(gs, cn, co, "You cannot anguish yourself.\n") {
        return None;
    }
    if is_exhausted(gs, cn) {
        return None;
    }
    if spellcost(gs, cn, cost) != 0 {
        return None;
    }
    if chance_base(
        gs,
        cn,
        i32::from(gs.characters[cn].skill[skill_const][5]),
        10,
        effective_resist(gs, cn, i32::from(gs.characters[co].skill[SK_RESIST][5])),
    ) != 0
    {
        return None;
    }
    if (gs.characters[co].flags & CharacterFlags::Immortal.bits()) != 0 {
        gs.do_character_log(cn, FontColor::Red, "You lost your focus.\n");
        return None;
    }
    Some(co)
}

/// Internal: attach a generic Anguish debuff spell-item to `co`.
#[allow(clippy::too_many_arguments)]
fn attach_anguish(
    gs: &mut GameState,
    cn: usize,
    co: usize,
    name: &[u8],
    temp: u16,
    power: u32,
    duration_ticks: i32,
    armor_mod: i8,
    weapon_mod: i8,
) {
    let in_opt = God::create_item(gs, 1);
    if in_opt.is_none() {
        log::error!("god_create_item failed in attach_anguish");
        return;
    }
    let in_idx = in_opt.unwrap();

    let item = &mut gs.items[in_idx];
    let mut name_bytes = [0u8; 40];
    let nlen = name.len().min(40);
    name_bytes[..nlen].copy_from_slice(&name[..nlen]);
    item.name = name_bytes;
    item.flags |= ItemFlags::IF_SPELL.bits();
    item.sprite[1] = 89;
    item.duration = duration_ticks as u32;
    item.active = duration_ticks as u32;
    item.temp = temp;
    item.power = power.max(1);
    item.armor[1] = armor_mod;
    item.weapon[1] = weapon_mod;
    item.data[0] = cn as u32;

    add_spell(gs, co, in_idx);
    EffectManager::fx_add_effect(
        gs,
        5,
        0,
        i32::from(gs.characters[co].x),
        i32::from(gs.characters[co].y),
        0,
    );
}

/// Active hostile cast: Anguish (Lava). Attaches a marker that empowers the
/// caster's next Blast against the target.
pub fn skill_anguish_lava(gs: &mut GameState, cn: usize) {
    let Some(co) = anguish_preflight(gs, cn, SK_ANGUISH_LAVA, 30) else {
        return;
    };
    let power = i32::from(gs.characters[cn].skill[SK_ANGUISH_LAVA][5]);
    attach_anguish(
        gs,
        cn,
        co,
        b"Anguish - Lava",
        SK_ANGUISH_LAVA as u16,
        power.max(1) as u32,
        TICKS * 30,
        0,
        0,
    );
    let name = gs.characters[co].get_name().to_owned();
    gs.do_character_log(
        cn,
        FontColor::Green,
        &format!("{} burns with hidden anguish.\n", name),
    );
    chlog!(cn, "Cast Anguish-Lava on {}", name);
}

/// Active hostile AoE: Anguish (Earth). Attaches a move-block debuff to every
/// hostile inside a 7x7 area around the resolved primary target.
pub fn skill_anguish_earth(gs: &mut GameState, cn: usize) {
    let Some(co) = anguish_preflight(gs, cn, SK_ANGUISH_EARTH, 22) else {
        return;
    };
    let power = i32::from(gs.characters[cn].skill[SK_ANGUISH_EARTH][5]);
    record_harakim_element_cast(gs, cn, HARAKIM_ELEMENT_EARTH);

    attach_anguish(
        gs,
        cn,
        co,
        b"Anguish - Earth",
        SK_ANGUISH_EARTH as u16,
        power.max(1) as u32,
        TICKS * 18,
        0,
        0,
    );

    let cx = i32::from(gs.characters[co].x);
    let cy = i32::from(gs.characters[co].y);
    // Force 7x7 (aoe_base = 10 maps to a 7x7 cross-pattern in
    // `helpers::skill_aoe_targets`).
    for maybe_co in helpers::skill_aoe_targets(gs, Some(cn), cx, cy, 10) {
        if maybe_co == cn || maybe_co == co {
            continue;
        }
        if !gs.may_aoe_attack(cn, maybe_co) {
            continue;
        }
        let resist = effective_resist(
            gs,
            cn,
            i32::from(gs.characters[maybe_co].skill[SK_RESIST][5]),
        );
        if power + helpers::random_mod_i32(20) <= resist + helpers::random_mod_i32(20) {
            continue;
        }
        gs.remember_pvp(cn, maybe_co);
        attach_anguish(
            gs,
            cn,
            maybe_co,
            b"Anguish - Earth",
            SK_ANGUISH_EARTH as u16,
            power.max(1) as u32,
            TICKS * 18,
            0,
            0,
        );
    }

    let name = gs.characters[co].get_name().to_owned();
    gs.do_character_log(
        cn,
        FontColor::Green,
        &format!("The earth swallows {}'s footing.\n", name),
    );
    chlog!(cn, "Cast Anguish-Earth around {}", name);
    add_exhaust(gs, cn, TICKS * 4);
}

/// Active hostile cast: Anguish (Ice). Attaches a debuff that shreds the
/// target's armor and weapon values for the duration.
pub fn skill_anguish_ice(gs: &mut GameState, cn: usize) {
    let Some(co) = anguish_preflight(gs, cn, SK_ANGUISH_ICE, 35) else {
        return;
    };
    let power = i32::from(gs.characters[cn].skill[SK_ANGUISH_ICE][5]);
    let armor_drop = -((power / 8).clamp(2, 30) as i8);
    let weapon_drop = -((power / 8).clamp(2, 30) as i8);
    attach_anguish(
        gs,
        cn,
        co,
        b"Anguish - Ice",
        SK_ANGUISH_ICE as u16,
        power.max(1) as u32,
        TICKS * 20,
        armor_drop,
        weapon_drop,
    );
    let name = gs.characters[co].get_name().to_owned();
    gs.do_character_log(
        cn,
        FontColor::Green,
        &format!("Glacial anguish bites into {}.\n", name),
    );
    chlog!(cn, "Cast Anguish-Ice on {}", name);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::driver::skill::common::has_active_spell_temp;
    use crate::driver::skill::common::remove_active_spell_temp;
    use crate::driver::skill::tests::{active_spell_item, attach_marker, caster_and_target};
    use crate::test_helpers::{add_test_player, with_test_gs};
    use core::constants::USE_ACTIVE;
    use core::skills::{SK_BLAST, SK_BLESS};

    #[test]
    fn element_switching_boosts_changed_element_damage() {
        with_test_gs(|gs| {
            let (cn, _nr) = add_test_player(gs);
            gs.item_templates[1].used = USE_ACTIVE;
            gs.characters[cn].kindred = KIN_HARAKIM as i32;
            gs.characters[cn].future1[7] |= 0b0000_0001;

            let first = apply_harakim_element_damage_bonus(gs, cn, HARAKIM_ELEMENT_LAVA, 100);
            let same = apply_harakim_element_damage_bonus(gs, cn, HARAKIM_ELEMENT_LAVA, 100);
            let changed = apply_harakim_element_damage_bonus(gs, cn, HARAKIM_ELEMENT_ICE, 100);

            assert_eq!(first, 100);
            assert_eq!(same, 100);
            assert_eq!(changed, 120);
            assert!(has_active_spell_temp(gs, cn, SK_ELEMENT_SWITCHING as u16));
            let state = gs
                .element_switch_states
                .get(&cn)
                .expect("element state should be tracked outside spell items");
            assert_eq!(state.last_element, HARAKIM_ELEMENT_ICE);
        });
    }

    #[test]
    fn element_switching_first_cast_tracks_state_without_icon() {
        with_test_gs(|gs| {
            let (cn, _nr) = add_test_player(gs);
            gs.item_templates[1].used = USE_ACTIVE;
            gs.characters[cn].kindred = KIN_HARAKIM as i32;
            gs.characters[cn].future1[7] |= 0b0000_0001;

            let damage = apply_harakim_element_damage_bonus(gs, cn, HARAKIM_ELEMENT_LAVA, 100);

            assert_eq!(damage, 100);
            assert!(!has_active_spell_temp(gs, cn, SK_ELEMENT_SWITCHING as u16));
            let state = gs
                .element_switch_states
                .get(&cn)
                .expect("first elemental cast should be remembered");
            assert_eq!(state.last_element, HARAKIM_ELEMENT_LAVA);
        });
    }

    #[test]
    fn element_switching_state_expires_on_tick_cleanup() {
        with_test_gs(|gs| {
            let (cn, _nr) = add_test_player(gs);
            gs.characters[cn].kindred = KIN_HARAKIM as i32;
            gs.characters[cn].future1[7] |= 0b0000_0001;
            gs.globals.ticker = 10;

            assert_eq!(
                apply_harakim_element_damage_bonus(gs, cn, HARAKIM_ELEMENT_LAVA, 100),
                100
            );
            assert!(gs.element_switch_states.contains_key(&cn));

            gs.tick_element_switch_states(10 + ELEMENT_SWITCH_MEMORY_TICKS);

            assert!(!gs.element_switch_states.contains_key(&cn));
        });
    }

    #[test]
    fn element_switching_modifier_requires_harakim_player_talent() {
        with_test_gs(|gs| {
            let (cn, _nr) = add_test_player(gs);
            gs.item_templates[1].used = USE_ACTIVE;
            gs.characters[cn].kindred = KIN_HARAKIM as i32;

            assert_eq!(
                apply_harakim_element_damage_bonus(gs, cn, HARAKIM_ELEMENT_LAVA, 100),
                100
            );
            assert_eq!(
                apply_harakim_element_damage_bonus(gs, cn, HARAKIM_ELEMENT_ICE, 100),
                100
            );

            gs.characters[cn].future1[7] |= 0b0000_0001;
            assert_eq!(
                apply_harakim_element_damage_bonus(gs, cn, HARAKIM_ELEMENT_LAVA, 100),
                100
            );
            assert_eq!(
                apply_harakim_element_damage_bonus(gs, cn, HARAKIM_ELEMENT_ICE, 100),
                120
            );
        });
    }

    #[test]
    fn ice_stun_marker_records_caster_and_power() {
        with_test_gs(|gs| {
            // The marker is allocated from item template 1, which must be in
            // use for `God::create_item` to succeed.
            gs.item_templates[1].used = USE_ACTIVE;

            let (cn, _nr) = add_test_player(gs);
            let (co, _nr2) = add_test_player(gs);

            attach_ice_stun_marker(gs, cn, co, 40);

            let marker = gs.characters[co]
                .spell
                .iter()
                .map(|&in_idx| in_idx as usize)
                .find(|&in_idx| in_idx != 0 && gs.items[in_idx].temp == SK_ICE_STUN as u16)
                .expect("expected an Ice Stun marker on the target");

            assert_eq!(gs.items[marker].data[0] as usize, cn);
            assert_eq!(gs.items[marker].power, 40);
            assert!(gs.items[marker].active > 0);
        });
    }

    #[test]
    fn anguish_earth_blocks_player_movement() {
        with_test_gs(|gs| {
            let (cn, _nr) = add_test_player(gs);
            let start_x = gs.characters[cn].x;
            let start_y = gs.characters[cn].y;
            attach_marker(gs, cn, 0, 12, SK_ANGUISH_EARTH as u16);
            crate::player::commands::plr_move_right(gs, cn);
            assert_eq!(gs.characters[cn].x, start_x);
            assert_eq!(gs.characters[cn].y, start_y);
            assert_eq!(gs.characters[cn].cerrno, core::constants::ERR_FAILED as u16);
        });
    }

    /// Cast Dispel until it succeeds, refunding mana and clearing exhaustion each try.
    fn dispel_until_success(gs: &mut GameState, cn: usize, co: usize, temp: usize) {
        for _ in 0..20 {
            gs.characters[cn].a_mana = 100_000;
            skill_dispel(gs, cn);
            if active_spell_item(gs, co, temp).is_none() {
                return;
            }
            remove_active_spell_temp(gs, cn, SK_BLAST as u16);
        }
    }

    #[test]
    fn dispel_removes_curse_before_other_spells() {
        with_test_gs(|gs| {
            let (cn, _nr) = add_test_player(gs);
            gs.item_templates[1].used = USE_ACTIVE;
            gs.characters[cn].skill[SK_DISPEL][5] = 100;
            attach_marker(gs, cn, 0, 500, SK_BLESS as u16);
            attach_marker(gs, cn, 1, 501, SK_CURSE as u16);

            dispel_until_success(gs, cn, cn, SK_CURSE);

            assert!(active_spell_item(gs, cn, SK_CURSE).is_none());
            assert!(active_spell_item(gs, cn, SK_BLESS).is_some());
            assert_eq!(gs.items[501].used, USE_EMPTY);
            assert_eq!(gs.characters[cn].a_mana, 75_000);
            assert!(is_exhausted(gs, cn));
        });
    }

    #[test]
    fn dispel_removes_curse_from_another_character() {
        with_test_gs(|gs| {
            let (cn, co) = caster_and_target(gs);
            gs.characters[cn].skill[SK_DISPEL][5] = 100;
            attach_marker(gs, co, 0, 500, SK_CURSE as u16);

            dispel_until_success(gs, cn, co, SK_CURSE);

            assert!(active_spell_item(gs, co, SK_CURSE).is_none());
        });
    }

    #[test]
    fn dispel_ignores_wimpy_and_reports_unspelled_target() {
        with_test_gs(|gs| {
            let (cn, _nr) = add_test_player(gs);
            gs.characters[cn].skill[SK_DISPEL][5] = 100;
            gs.characters[cn].a_mana = 100_000;
            attach_marker(gs, cn, 0, 500, SK_WIMPY as u16);

            skill_dispel(gs, cn);

            assert_eq!(gs.characters[cn].a_mana, 100_000);
            assert!(active_spell_item(gs, cn, SK_WIMPY).is_some());
        });
    }

    #[test]
    fn dispel_is_refused_while_exhausted() {
        with_test_gs(|gs| {
            let (cn, _nr) = add_test_player(gs);
            gs.item_templates[1].used = USE_ACTIVE;
            gs.characters[cn].skill[SK_DISPEL][5] = 100;
            gs.characters[cn].a_mana = 100_000;
            attach_marker(gs, cn, 0, 500, SK_CURSE as u16);
            add_exhaust(gs, cn, TICKS);

            skill_dispel(gs, cn);

            assert_eq!(gs.characters[cn].a_mana, 100_000);
            assert!(active_spell_item(gs, cn, SK_CURSE).is_some());
        });
    }

    #[test]
    fn lava_blast_dot_records_caster_and_burns_for_15_seconds() {
        with_test_gs(|gs| {
            let (cn, co) = caster_and_target(gs);

            apply_lava_blast_dot(gs, cn, co, 0);

            let item = active_spell_item(gs, co, SK_LAVA_BLAST).expect("lava burn attached");
            assert_eq!(gs.items[item].data[0] as usize, cn);
            assert_eq!(gs.items[item].power, 1);
            assert_eq!(gs.items[item].active, (TICKS * 15) as u32);
        });
    }

    #[test]
    fn anguish_ice_lowers_armor_and_weapon() {
        with_test_gs(|gs| {
            let (cn, co) = caster_and_target(gs);
            gs.characters[cn].skill[SK_ANGUISH_ICE][5] = 80;

            for _ in 0..20 {
                gs.characters[cn].a_mana = 100_000;
                skill_anguish_ice(gs, cn);
                assert_eq!(gs.characters[cn].a_mana, 65_000);
                if active_spell_item(gs, co, SK_ANGUISH_ICE).is_some() {
                    break;
                }
            }

            let item = active_spell_item(gs, co, SK_ANGUISH_ICE).expect("anguish attached");
            assert_eq!(gs.items[item].armor[1], -10);
            assert_eq!(gs.items[item].weapon[1], -10);
            assert_eq!(gs.items[item].active, (TICKS * 20) as u32);
            assert_eq!(gs.items[item].data[0] as usize, cn);
        });
    }

    #[test]
    fn anguish_ice_modifiers_have_a_floor() {
        with_test_gs(|gs| {
            let (cn, co) = caster_and_target(gs);
            gs.characters[cn].skill[SK_ANGUISH_ICE][5] = 8;

            for _ in 0..20 {
                gs.characters[cn].a_mana = 100_000;
                skill_anguish_ice(gs, cn);
                if active_spell_item(gs, co, SK_ANGUISH_ICE).is_some() {
                    break;
                }
            }

            let item = active_spell_item(gs, co, SK_ANGUISH_ICE).expect("anguish attached");
            assert_eq!(gs.items[item].armor[1], -2);
            assert_eq!(gs.items[item].weapon[1], -2);
        });
    }

    #[test]
    fn anguish_preflight_rejects_self_target_without_cost() {
        with_test_gs(|gs| {
            let (cn, _co) = caster_and_target(gs);
            gs.characters[cn].skill_target1 = cn as u16;
            gs.characters[cn].a_mana = 100_000;

            assert!(anguish_preflight(gs, cn, SK_ANGUISH_LAVA, 30).is_none());
            assert_eq!(gs.characters[cn].a_mana, 100_000);
        });
    }

    #[test]
    fn anguish_preflight_is_refused_while_exhausted() {
        with_test_gs(|gs| {
            let (cn, _co) = caster_and_target(gs);
            gs.characters[cn].a_mana = 100_000;
            add_exhaust(gs, cn, TICKS);

            assert!(anguish_preflight(gs, cn, SK_ANGUISH_LAVA, 30).is_none());
            assert_eq!(gs.characters[cn].a_mana, 100_000);
        });
    }
}
