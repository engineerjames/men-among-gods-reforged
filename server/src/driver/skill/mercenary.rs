use super::common::{
    add_skill_cooldown, add_spell, effective_resist, skill_on_cooldown, spell_immunity,
    spell_race_mod, spellcost,
};
use super::curse::cast_curse_effect;
use super::{hostile_cast_preflight, resolve_offensive_target};
use crate::chlog;
use crate::effect::EffectManager;
use crate::game_state::GameState;
use crate::god::God;
use crate::helpers;
use core::constants::{CharacterFlags, ItemFlags, NT_DIDHIT, NT_GOTHIT, TICKS, USE_EMPTY};
use core::skills::{
    SK_BLADE_DANCE, SK_CONTAGION, SK_DELIVER_DEATH, SK_DISARM, SK_GUNPOWDER_FLASH,
    SK_GUNPOWDER_FLASH2, SK_IMMUN, SK_PARASITE, SK_RESIST, SK_WEAPON,
};
use core::types::FontColor;

/// Internal helper: attach a Parasite-family DoT spell-item to `co`.
///
/// Used both for the direct Parasite/Contagion cast and for on-death spread.
///
/// # Arguments
///
/// * `gs` - Game state.
/// * `caster` - Caster character index (recorded in `item.data[0]` for lifesteal).
/// * `co` - Target character index.
/// * `power` - Caster skill power; drives damage and resistance roll.
/// * `temp` - Either `SK_PARASITE` or `SK_CONTAGION`.
/// * `duration_ticks` - Total duration in ticks.
/// * `name` - Display name shown to players.
///
/// # Returns
///
/// * `true` if the DoT was successfully attached.
pub(crate) fn apply_parasitic_dot(
    gs: &mut GameState,
    caster: usize,
    co: usize,
    power: i32,
    temp: u16,
    duration_ticks: i32,
    name: &[u8],
) -> bool {
    if (gs.characters[co].flags & CharacterFlags::Immortal.bits()) != 0 {
        return false;
    }
    let in_opt = God::create_item(gs, 1);
    if in_opt.is_none() {
        log::error!("god_create_item failed in apply_parasitic_dot");
        return false;
    }
    let in_idx = in_opt.unwrap();

    let mut power = spell_immunity(gs, power, i32::from(gs.characters[co].skill[SK_IMMUN][5]));
    power = spell_race_mod(gs, power, gs.characters[caster].kindred);
    if power < 1 {
        gs.items[in_idx].used = USE_EMPTY;
        return false;
    }

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
    item.power = power as u32;
    item.data[0] = caster as u32;
    // data[1] is reserved for Contagion to track the last tick at which it
    // spread, preventing repeat spreads within the same combat round.
    item.data[1] = 0;

    if add_spell(gs, co, in_idx) == 0 {
        return false;
    }
    true
}

/// Active spell: infest the target with parasites that drain HP over time and
/// heal the caster for a fraction of the damage dealt. The infestation also
/// jumps to enemies standing in the four tiles adjacent to the caster.
///
/// # Arguments
///
/// * `gs` - Game state.
/// * `cn` - Caster character index.
pub fn skill_parasite(gs: &mut GameState, cn: usize) {
    let co = resolve_offensive_target(gs, cn);
    if !hostile_cast_preflight(gs, cn, co, "You cannot infect yourself.\n") {
        return;
    }
    if spellcost(gs, cn, 10) != 0 {
        return;
    }

    let power = i32::from(gs.characters[cn].skill[SK_PARASITE][5]);
    if !apply_parasitic_dot(
        gs,
        cn,
        co,
        power,
        SK_PARASITE as u16,
        TICKS * 24,
        b"Parasite",
    ) {
        gs.do_character_log(
            cn,
            FontColor::Green,
            "Magical interference neutralised your parasite.\n",
        );
        return;
    }

    let name = gs.characters[co].get_name().to_owned();
    gs.do_character_log(
        cn,
        FontColor::Green,
        &format!("{} was infested with parasites.\n", name),
    );
    gs.do_character_log(co, FontColor::Green, "Parasites burrow into your flesh!\n");
    gs.do_notify_character(co as u32, i32::from(NT_GOTHIT), cn as i32, 0, 0, 0);
    gs.do_notify_character(cn as u32, i32::from(NT_DIDHIT), co as i32, 0, 0, 0);
    chlog!(cn, "Cast Parasite on {}", name);
    EffectManager::fx_add_effect(
        gs,
        5,
        0,
        i32::from(gs.characters[co].x),
        i32::from(gs.characters[co].y),
        0,
    );

    // Secondary targets: the four tiles adjacent to the caster only. Parasite
    // never grows past the legacy cross, so the AoE base is pinned to zero.
    let caster_x = i32::from(gs.characters[cn].x);
    let caster_y = i32::from(gs.characters[cn].y);
    for maybe_co in helpers::skill_aoe_targets(gs, Some(cn), caster_x, caster_y, 0) {
        if maybe_co == cn || maybe_co == co {
            continue;
        }
        if !gs.may_aoe_attack(cn, maybe_co) {
            continue;
        }
        if !apply_parasitic_dot(
            gs,
            cn,
            maybe_co,
            power,
            SK_PARASITE as u16,
            TICKS * 24,
            b"Parasite",
        ) {
            continue;
        }

        gs.remember_pvp(cn, maybe_co);
        let other_name = gs.characters[maybe_co].get_name().to_owned();
        gs.do_character_log(
            cn,
            FontColor::Green,
            &format!("{} was infested with parasites.\n", other_name),
        );
        gs.do_character_log(
            maybe_co,
            FontColor::Green,
            "Parasites burrow into your flesh!\n",
        );
        gs.do_notify_character(maybe_co as u32, i32::from(NT_GOTHIT), cn as i32, 0, 0, 0);
        gs.do_notify_character(cn as u32, i32::from(NT_DIDHIT), maybe_co as i32, 0, 0, 0);
        EffectManager::fx_add_effect(
            gs,
            5,
            0,
            i32::from(gs.characters[maybe_co].x),
            i32::from(gs.characters[maybe_co].y),
            0,
        );
    }
}

/// Action-speed points a Gunpowder Flash victim loses at the given spell power.
///
/// # Arguments
///
/// * `power` - The debuff item's `power`.
///
/// # Returns
///
/// * Points subtracted from the victim's action-speed calc, between 6 and 20.
pub(crate) fn gunpowder_flash_slow_points(power: u32) -> i32 {
    (3 + power as i32 / 40).clamp(6, 20)
}

/// Attaches the Gunpowder Flash slow to a single target.
///
/// Applies immunity and race modifiers to `power` before creating the spell
/// item, so callers can pass the caster's raw skill value.
///
/// # Arguments
///
/// * `gs` - Game state.
/// * `cn` - Caster character index.
/// * `co` - Target character index.
/// * `power` - Caster's raw Gunpowder Flash skill value.
///
/// # Returns
///
/// * `true` if the target is now slowed.
fn apply_gunpowder_flash(gs: &mut GameState, cn: usize, co: usize, power: i32) -> bool {
    let power = spell_immunity(gs, power, i32::from(gs.characters[co].skill[SK_IMMUN][5]));
    let power = spell_race_mod(gs, power, gs.characters[cn].kindred);
    if power < 1 {
        return false;
    }
    if (gs.characters[co].flags & CharacterFlags::Immortal.bits()) != 0 {
        return false;
    }

    let in_opt = God::create_item(gs, 1);
    if in_opt.is_none() {
        log::error!("god_create_item failed in skill_gunpowder_flash");
        return false;
    }
    let in_idx = in_opt.unwrap();

    let item = &mut gs.items[in_idx];
    let mut name_bytes = [0u8; 40];
    let name = b"Gunpowder Flash";
    let nlen = name.len().min(40);
    name_bytes[..nlen].copy_from_slice(&name[..nlen]);
    item.name = name_bytes;
    item.flags |= ItemFlags::IF_SPELL.bits();
    item.sprite[1] = 89;
    item.duration = (TICKS * 30) as u32;
    item.active = (TICKS * 30) as u32;
    item.temp = SK_GUNPOWDER_FLASH as u16;
    item.power = power as u32;

    add_spell(gs, co, in_idx) != 0
}

/// Attaches the Gunpowder Flash speed buff to the caster.
///
/// # Arguments
///
/// * `gs` - Game state.
/// * `cn` - Caster character index.
/// * `power` - Caster's raw Gunpowder Flash skill value.
fn apply_gunpowder_flash_haste(gs: &mut GameState, cn: usize, power: i32) {
    let Some(in_idx) = God::create_item(gs, 1) else {
        log::error!("god_create_item failed in apply_gunpowder_flash_haste");
        return;
    };

    let item = &mut gs.items[in_idx];
    let mut name_bytes = [0u8; 40];
    let name = b"Gunpowder Flash";
    name_bytes[..name.len()].copy_from_slice(name);
    item.name = name_bytes;
    item.flags |= ItemFlags::IF_SPELL.bits();
    item.sprite[1] = 88;
    item.duration = (TICKS * 30) as u32;
    item.active = (TICKS * 30) as u32;
    item.temp = SK_GUNPOWDER_FLASH2 as u16;
    item.power = power.max(1) as u32;

    add_spell(gs, cn, in_idx);
}

/// Active spell: slow the target's attacks and quicken the caster. Enemies
/// standing in the four tiles adjacent to the caster that are attacking the
/// caster are slowed as well.
///
/// # Arguments
///
/// * `gs` - Game state.
/// * `cn` - Caster character index.
pub fn skill_gunpowder_flash(gs: &mut GameState, cn: usize) {
    let co = resolve_offensive_target(gs, cn);
    if !hostile_cast_preflight(gs, cn, co, "You cannot flash yourself.\n") {
        return;
    }
    if spellcost(gs, cn, 7) != 0 {
        return;
    }

    let power = i32::from(gs.characters[cn].skill[SK_GUNPOWDER_FLASH][5]);
    if (gs.characters[co].flags & CharacterFlags::Immortal.bits()) != 0 {
        gs.do_character_log(cn, FontColor::Red, "You lost your focus.\n");
        return;
    }
    if !apply_gunpowder_flash(gs, cn, co, power) {
        gs.do_character_log(
            cn,
            FontColor::Green,
            "Magical interference neutralised your flash.\n",
        );
        return;
    }
    apply_gunpowder_flash_haste(gs, cn, power);

    let name = gs.characters[co].get_name().to_owned();
    gs.do_character_log(
        cn,
        FontColor::Green,
        &format!("{} is blinded by the flash.\n", name),
    );
    gs.do_character_log(co, FontColor::Green, "A flash of powder blinds you!\n");
    gs.do_notify_character(co as u32, i32::from(NT_GOTHIT), cn as i32, 0, 0, 0);
    gs.do_notify_character(cn as u32, i32::from(NT_DIDHIT), co as i32, 0, 0, 0);
    chlog!(cn, "Cast Gunpowder Flash on {}", name);
    EffectManager::fx_add_effect(
        gs,
        5,
        0,
        i32::from(gs.characters[co].x),
        i32::from(gs.characters[co].y),
        0,
    );

    // Secondary targets: adjacent enemies actively attacking the caster, like Stun.
    let caster_x = i32::from(gs.characters[cn].x);
    let caster_y = i32::from(gs.characters[cn].y);
    for maybe_co in helpers::skill_aoe_targets(gs, Some(cn), caster_x, caster_y, 0) {
        if maybe_co == cn || maybe_co == co || gs.characters[maybe_co].attack_cn != cn as u16 {
            continue;
        }
        if !gs.may_aoe_attack(cn, maybe_co) {
            continue;
        }
        if !apply_gunpowder_flash(gs, cn, maybe_co, power) {
            continue;
        }

        gs.remember_pvp(cn, maybe_co);
        let other_name = gs.characters[maybe_co].get_name().to_owned();
        gs.do_character_log(
            cn,
            FontColor::Green,
            &format!("{} is blinded by the flash.\n", other_name),
        );
        gs.do_character_log(
            maybe_co,
            FontColor::Green,
            "A flash of powder blinds you!\n",
        );
        gs.do_notify_character(maybe_co as u32, i32::from(NT_GOTHIT), cn as i32, 0, 0, 0);
        gs.do_notify_character(cn as u32, i32::from(NT_DIDHIT), maybe_co as i32, 0, 0, 0);
        EffectManager::fx_add_effect(
            gs,
            5,
            0,
            i32::from(gs.characters[maybe_co].x),
            i32::from(gs.characters[maybe_co].y),
            0,
        );
    }
}

/// Percent multiplier applied to weapon-based Mercenary/Seyan'Du strikes so
/// they scale with the skill used: 50% at skill 0, 100% at modified skill
/// 100, capped at 150% from modified skill 200.
///
/// # Arguments
///
/// * `power` - Modified skill value of the ability being used.
///
/// # Returns
///
/// * Damage multiplier in percent.
fn weapon_skill_scale_pct(power: i32) -> i32 {
    (50 + power / 2).clamp(50, 150)
}

/// Active melee finisher: a devastating blow against a low-health adjacent
/// enemy. Deals massively amplified weapon damage when the target is below
/// 25% HP, otherwise deals modestly increased damage, scaled by the Deliver
/// Death skill (see [`weapon_skill_scale_pct`]). Long cooldown.
///
/// # Arguments
///
/// * `gs` - Game state.
/// * `cn` - Caster character index.
pub fn skill_deliver_death(gs: &mut GameState, cn: usize) {
    let co = resolve_offensive_target(gs, cn);
    if !hostile_cast_preflight(gs, cn, co, "You cannot strike yourself down.\n") {
        return;
    }
    if skill_on_cooldown(gs, cn, SK_DELIVER_DEATH as u16) {
        return;
    }
    // Adjacency check
    let dx = (i32::from(gs.characters[cn].x) - i32::from(gs.characters[co].x)).abs();
    let dy = (i32::from(gs.characters[cn].y) - i32::from(gs.characters[co].y)).abs();
    if dx > 1 || dy > 1 {
        gs.do_character_log(cn, FontColor::Red, "Your target is too far away.\n");
        return;
    }
    if gs.characters[cn].a_end < 75 * 1000 {
        gs.do_character_log(cn, FontColor::Red, "You're too exhausted!\n");
        return;
    }
    gs.characters[cn].a_end -= 75 * 1000;

    let weapon = i32::from(gs.characters[cn].weapon).max(1);
    let max_hp = i32::from(gs.characters[co].hp[5]).max(1);
    let cur_hp_pct = gs.characters[co].a_hp / max_hp; // 0..1000
    let base_dam = if cur_hp_pct < 250 {
        weapon * 15 + helpers::random_mod_i32(weapon * 6)
    } else {
        weapon * 6 + helpers::random_mod_i32(weapon * 3)
    };
    let power = i32::from(gs.characters[cn].skill[SK_DELIVER_DEATH][5]);
    let dam = base_dam * weapon_skill_scale_pct(power) / 100;

    let applied = gs.do_hurt(cn, co, dam, 0);
    if applied < 1 {
        gs.do_character_log(cn, FontColor::Green, "Your blow glances off harmlessly.\n");
    } else {
        let name = gs.characters[co].get_name().to_owned();
        gs.do_character_log(
            cn,
            FontColor::Green,
            &format!("You deliver death to {} for {} HP.\n", name, applied),
        );
        chlog!(cn, "Cast Deliver Death on {} for {} HP", name, applied);
    }

    let tx = i32::from(gs.characters[co].x);
    let ty = i32::from(gs.characters[co].y);
    gs.do_area_sound(co, 0, tx, ty, i32::from(gs.characters[cn].sound) + 4);
    EffectManager::fx_add_effect(gs, 5, 0, tx, ty, 0);

    add_skill_cooldown(
        gs,
        cn,
        TICKS * 45,
        SK_DELIVER_DEATH as u16,
        b"Deliver Death Cooldown",
    );
}

/// Percent chance per landed melee hit that Disarm triggers.
const DISARM_PROC_PERCENT: i32 = 10;

/// Modified Disarm skill at which procs also spread to adjacent attackers.
const DISARM_AOE_MIN_POWER: i32 = 100;

/// Attaches the Disarm weapon-skill penalty to one target and reports it.
///
/// # Arguments
///
/// * `gs` - Game state.
/// * `cn` - Attacker character index.
/// * `co` - Target character index.
/// * `power` - Attacker's raw Disarm skill value.
///
/// # Returns
///
/// * `true` if the target is now disarmed.
fn apply_disarm(gs: &mut GameState, cn: usize, co: usize, power: i32) -> bool {
    let power = spell_immunity(gs, power, i32::from(gs.characters[co].skill[SK_IMMUN][5]));
    let power = spell_race_mod(gs, power, gs.characters[cn].kindred);
    if power < 1 || (gs.characters[co].flags & CharacterFlags::Immortal.bits()) != 0 {
        return false;
    }

    let Some(in_idx) = God::create_item(gs, 1) else {
        log::error!("god_create_item failed in apply_disarm");
        return false;
    };
    let weapon_penalty = -(((power / 2) * 3).clamp(3, 150)) as i16;

    let item = &mut gs.items[in_idx];
    let mut name_bytes = [0u8; 40];
    let name = b"Disarm";
    name_bytes[..name.len()].copy_from_slice(name);
    item.name = name_bytes;
    item.flags |= ItemFlags::IF_SPELL.bits();
    item.sprite[1] = 89;
    item.duration = (TICKS * 45) as u32;
    item.active = (TICKS * 45) as u32;
    item.temp = SK_DISARM as u16;
    item.power = power as u32;
    item.skill[SK_WEAPON][1] = weapon_penalty;

    if add_spell(gs, co, in_idx) == 0 {
        return false;
    }

    let name = gs.characters[co].get_name().to_owned();
    gs.do_character_log(cn, FontColor::Green, &format!("{} was disarmed.\n", name));
    gs.do_character_log(
        co,
        FontColor::Green,
        "Your weapon feels heavy and unfamiliar!\n",
    );
    gs.do_notify_character(co as u32, i32::from(NT_GOTHIT), cn as i32, 0, 0, 0);
    gs.do_notify_character(cn as u32, i32::from(NT_DIDHIT), co as i32, 0, 0, 0);
    chlog!(cn, "Disarmed {}", name);
    EffectManager::fx_add_effect(
        gs,
        5,
        0,
        i32::from(gs.characters[co].x),
        i32::from(gs.characters[co].y),
        0,
    );
    true
}

/// Passive Disarm: on a landed melee hit, may weaken the defender's weapon
/// skill. At [`DISARM_AOE_MIN_POWER`] and above, adjacent enemies attacking
/// the attacker are affected too.
///
/// # Arguments
///
/// * `gs` - Game state.
/// * `cn` - Attacker character index.
/// * `co` - Defender hit by the primary strike.
pub fn disarm_on_hit(gs: &mut GameState, cn: usize, co: usize) {
    if gs.characters[cn].skill[SK_DISARM][0] == 0 {
        return;
    }
    if helpers::random_mod_i32(100) >= DISARM_PROC_PERCENT {
        return;
    }

    let power = i32::from(gs.characters[cn].skill[SK_DISARM][5]);
    apply_disarm(gs, cn, co, power);
    if power < DISARM_AOE_MIN_POWER {
        return;
    }

    // Secondary targets mirror Stun: adjacent attackers, each with a resist roll.
    let caster_x = i32::from(gs.characters[cn].x);
    let caster_y = i32::from(gs.characters[cn].y);
    for maybe_co in helpers::skill_aoe_targets(gs, Some(cn), caster_x, caster_y, 0) {
        if maybe_co == cn || maybe_co == co || gs.characters[maybe_co].attack_cn != cn as u16 {
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
        if power + helpers::random_mod_i32(20) > resist + helpers::random_mod_i32(20) {
            apply_disarm(gs, cn, maybe_co, power);
        }
    }
}

/// Active spell: a virulent parasitic infection that lasts much longer than
/// Parasite and spreads to adjacent enemies when the host dies.
///
/// # Arguments
///
/// * `gs` - Game state.
/// * `cn` - Caster character index.
pub fn skill_contagion(gs: &mut GameState, cn: usize) {
    let co = resolve_offensive_target(gs, cn);
    if !hostile_cast_preflight(gs, cn, co, "You cannot infect yourself.\n") {
        return;
    }
    if spellcost(gs, cn, 20) != 0 {
        return;
    }

    let power = i32::from(gs.characters[cn].skill[SK_CONTAGION][5]);
    if !apply_parasitic_dot(
        gs,
        cn,
        co,
        power,
        SK_CONTAGION as u16,
        TICKS * 60,
        b"Contagion",
    ) {
        gs.do_character_log(
            cn,
            FontColor::Green,
            "Magical interference neutralised your contagion.\n",
        );
        return;
    }

    cast_curse_effect(gs, cn, co, SK_CONTAGION);

    let name = gs.characters[co].get_name().to_owned();
    gs.do_character_log(
        cn,
        FontColor::Green,
        &format!("{} was struck by a virulent contagion.\n", name),
    );
    gs.do_character_log(co, FontColor::Green, "A virulent contagion takes hold!\n");
    gs.do_notify_character(co as u32, i32::from(NT_GOTHIT), cn as i32, 0, 0, 0);
    gs.do_notify_character(cn as u32, i32::from(NT_DIDHIT), co as i32, 0, 0, 0);
    chlog!(cn, "Cast Contagion on {}", name);
    EffectManager::fx_add_effect(
        gs,
        5,
        0,
        i32::from(gs.characters[co].x),
        i32::from(gs.characters[co].y),
        0,
    );
}

/// Active melee skill: a sweeping flurry of strikes that hits every adjacent
/// hostile target for 6x the standard Surround Hit damage, scaled by the
/// Blade Dance skill (see [`weapon_skill_scale_pct`]). Costs endurance and
/// goes on a short cooldown afterwards.
///
/// # Arguments
///
/// * `gs` - Game state.
/// * `cn` - Caster character index.
pub fn skill_blade_dance(gs: &mut GameState, cn: usize) {
    if skill_on_cooldown(gs, cn, SK_BLADE_DANCE as u16) {
        return;
    }
    if gs.characters[cn].a_end < 100 * 1000 {
        gs.do_character_log(cn, FontColor::Red, "You're too exhausted!\n");
        return;
    }
    gs.characters[cn].a_end -= 100 * 1000;

    let weapon = i32::from(gs.characters[cn].weapon).max(1);
    let caster_x = i32::from(gs.characters[cn].x);
    let caster_y = i32::from(gs.characters[cn].y);
    let attacker_is_player = (gs.characters[cn].flags & CharacterFlags::Player.bits()) != 0;
    let aoe_base = if attacker_is_player { 4 } else { 1 };
    let scale_pct = weapon_skill_scale_pct(i32::from(gs.characters[cn].skill[SK_BLADE_DANCE][5]));

    let mut hits = 0;
    for co in helpers::skill_aoe_targets(gs, Some(cn), caster_x, caster_y, aoe_base) {
        if co == cn {
            continue;
        }
        if !gs.may_aoe_attack(cn, co) {
            continue;
        }
        let base_dam = weapon + helpers::random_mod_i32(weapon.max(1));
        // Mirror Surround Hit's reduction (3/4 of base) then 6x it for Blade
        // Dance, scaled by skill.
        let sdam = (base_dam - base_dam / 4) * 6 * scale_pct / 100;
        gs.remember_pvp(cn, co);
        let applied = gs.do_hurt(cn, co, sdam, 0);
        if applied > 0 {
            hits += 1;
            let name = gs.characters[co].get_name().to_owned();
            gs.do_character_log(
                cn,
                FontColor::Green,
                &format!("Your blade strikes {} for {} HP.\n", name, applied),
            );
        }
        let tx = i32::from(gs.characters[co].x);
        let ty = i32::from(gs.characters[co].y);
        EffectManager::fx_add_effect(gs, 5, 0, tx, ty, 0);
    }

    gs.do_character_log(
        cn,
        FontColor::Green,
        &format!("Your blade dance lands {} strike(s).\n", hits),
    );
    chlog!(cn, "Performed Blade Dance ({} hits)", hits);

    add_skill_cooldown(
        gs,
        cn,
        TICKS * 30,
        SK_BLADE_DANCE as u16,
        b"Blade Dance Cooldown",
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::driver::skill::curse::apply_curse_effect_after_focus;
    use crate::driver::skill::tests::{active_spell_item, caster_and_target, place_attacker};
    use crate::test_helpers::{add_test_player, with_test_gs};
    use core::constants::USE_ACTIVE;
    use core::constants::{DX_RIGHT, SERVER_MAPX};
    use core::skills::SK_CURSE;
    use core::types::Character;

    fn place_hostile_target(gs: &mut GameState, cn: usize, co: usize, x: i16, y: i16) {
        gs.characters[cn].flags |= CharacterFlags::Infrared.bits();
        gs.characters[co] = Character::default();
        gs.characters[co].used = USE_ACTIVE;
        gs.characters[co].x = x;
        gs.characters[co].y = y;
        gs.characters[co].attack_cn = cn as u16;
        gs.map[x as usize + y as usize * SERVER_MAPX as usize].ch = co as u32;
    }

    #[test]
    fn contagion_and_curse_can_coexist_using_contagion_power() {
        with_test_gs(|gs| {
            gs.item_templates[1].used = USE_ACTIVE;
            let (cn, _nr) = add_test_player(gs);
            let co = 2;
            place_hostile_target(gs, cn, co, 10, 11);
            gs.characters[cn].skill[SK_CURSE][0] = 0;
            gs.characters[cn].skill[SK_CURSE][5] = 0;
            gs.characters[cn].skill[SK_CONTAGION][0] = 4;
            gs.characters[cn].skill[SK_CONTAGION][5] = 60;

            assert!(apply_parasitic_dot(
                gs,
                cn,
                co,
                60,
                SK_CONTAGION as u16,
                TICKS * 60,
                b"Contagion",
            ));
            apply_curse_effect_after_focus(gs, cn, co, SK_CONTAGION);

            let contagion =
                active_spell_item(gs, co, SK_CONTAGION).expect("Contagion should remain attached");
            let curse = active_spell_item(gs, co, SK_CURSE).expect("Curse should also be attached");
            assert_eq!(gs.items[contagion].data[0] as usize, cn);
            assert!(gs.items[curse].power > 0);
            assert!(gs.items[curse].attrib.iter().all(|attrib| attrib[1] < 0));
        });
    }

    #[test]
    fn failed_bundled_curse_roll_leaves_contagion_attached() {
        with_test_gs(|gs| {
            gs.item_templates[1].used = USE_ACTIVE;
            let (cn, _nr) = add_test_player(gs);
            let co = 2;
            place_hostile_target(gs, cn, co, 10, 11);
            gs.characters[cn].skill[SK_CONTAGION][0] = 1;
            gs.characters[cn].skill[SK_CONTAGION][5] = 1;
            gs.characters[co].skill[SK_RESIST][5] = 100;

            assert!(apply_parasitic_dot(
                gs,
                cn,
                co,
                1,
                SK_CONTAGION as u16,
                TICKS * 60,
                b"Contagion",
            ));
            cast_curse_effect(gs, cn, co, SK_CONTAGION);

            assert!(active_spell_item(gs, co, SK_CONTAGION).is_some());
            assert!(active_spell_item(gs, co, SK_CURSE).is_none());
        });
    }

    #[test]
    fn bundled_curse_uses_contagion_base_for_area_targets() {
        with_test_gs(|gs| {
            gs.item_templates[1].used = USE_ACTIVE;
            let (cn, _nr) = add_test_player(gs);
            let primary = 2;
            let splash = 3;
            place_hostile_target(gs, cn, primary, 10, 11);
            place_hostile_target(gs, cn, splash, 11, 10);
            gs.characters[cn].skill[SK_CURSE][0] = 0;
            gs.characters[cn].skill[SK_CURSE][5] = 0;
            gs.characters[cn].skill[SK_CONTAGION][0] = 4;
            gs.characters[cn].skill[SK_CONTAGION][5] = 60;

            assert!(apply_parasitic_dot(
                gs,
                cn,
                primary,
                60,
                SK_CONTAGION as u16,
                TICKS * 60,
                b"Contagion",
            ));
            apply_curse_effect_after_focus(gs, cn, primary, SK_CONTAGION);

            assert!(active_spell_item(gs, primary, SK_CURSE).is_some());
            assert!(active_spell_item(gs, splash, SK_CURSE).is_some());
            assert!(active_spell_item(gs, splash, SK_CONTAGION).is_none());
        });
    }

    fn has_spell_temp(gs: &GameState, cn: usize, temp: usize) -> bool {
        gs.characters[cn]
            .spell
            .iter()
            .any(|&i| i != 0 && gs.items[i as usize].temp == temp as u16)
    }

    #[test]
    fn gunpowder_flash_slows_attackers_only_and_hastes_caster() {
        with_test_gs(|gs| {
            let (cn, _nr) = add_test_player(gs);
            gs.item_templates[1].used = USE_ACTIVE;
            gs.characters[cn].flags |= CharacterFlags::Infrared.bits();
            gs.characters[cn].skill[SK_GUNPOWDER_FLASH][5] = 60;
            gs.characters[cn].a_mana = 100_000;
            gs.characters[cn].skill_target1 = 0;
            gs.characters[cn].attack_cn = 0;
            gs.characters[cn].dir = DX_RIGHT;

            let (front, side, bystander) = (2, 3, 4);
            place_attacker(gs, cn, front, 1, 0);
            place_attacker(gs, cn, side, 0, 1);
            place_attacker(gs, cn, bystander, -1, 0);
            gs.characters[bystander].attack_cn = 0;

            skill_gunpowder_flash(gs, cn);

            assert!(has_spell_temp(gs, front, SK_GUNPOWDER_FLASH));
            assert!(has_spell_temp(gs, side, SK_GUNPOWDER_FLASH));
            assert!(!has_spell_temp(gs, bystander, SK_GUNPOWDER_FLASH));
            assert!(has_spell_temp(gs, cn, SK_GUNPOWDER_FLASH2));
        });
    }

    #[test]
    fn disarm_proc_hits_defender_and_adjacent_attackers_only() {
        with_test_gs(|gs| {
            let (cn, _nr) = add_test_player(gs);
            gs.item_templates[1].used = USE_ACTIVE;
            gs.characters[cn].flags |= CharacterFlags::Infrared.bits();
            gs.characters[cn].skill[SK_DISARM][0] = 1;
            gs.characters[cn].skill[SK_DISARM][5] = 100;

            let (defender, side, bystander) = (2, 3, 4);
            place_attacker(gs, cn, defender, 1, 0);
            place_attacker(gs, cn, side, 0, 1);
            place_attacker(gs, cn, bystander, -1, 0);
            gs.characters[bystander].attack_cn = 0;

            // 10% proc and a resist roll on secondaries: retry until both land.
            for _ in 0..500 {
                disarm_on_hit(gs, cn, defender);
            }

            assert!(has_spell_temp(gs, defender, SK_DISARM));
            assert!(has_spell_temp(gs, side, SK_DISARM));
            assert!(!has_spell_temp(gs, bystander, SK_DISARM));
        });
    }

    #[test]
    fn disarm_below_aoe_threshold_hits_defender_only() {
        with_test_gs(|gs| {
            let (cn, _nr) = add_test_player(gs);
            gs.item_templates[1].used = USE_ACTIVE;
            gs.characters[cn].flags |= CharacterFlags::Infrared.bits();
            gs.characters[cn].skill[SK_DISARM][0] = 1;
            gs.characters[cn].skill[SK_DISARM][5] = (DISARM_AOE_MIN_POWER - 1) as u16;

            let (defender, side) = (2, 3);
            place_attacker(gs, cn, defender, 1, 0);
            place_attacker(gs, cn, side, 0, 1);

            for _ in 0..500 {
                disarm_on_hit(gs, cn, defender);
            }

            assert!(has_spell_temp(gs, defender, SK_DISARM));
            assert!(!has_spell_temp(gs, side, SK_DISARM));
        });
    }

    #[test]
    fn disarm_never_procs_without_the_skill() {
        with_test_gs(|gs| {
            let (cn, _nr) = add_test_player(gs);
            gs.item_templates[1].used = USE_ACTIVE;
            place_attacker(gs, cn, 2, 1, 0);

            for _ in 0..200 {
                disarm_on_hit(gs, cn, 2);
            }

            assert!(!has_spell_temp(gs, 2, SK_DISARM));
        });
    }

    #[test]
    fn gunpowder_flash_slow_points_are_clamped() {
        assert_eq!(gunpowder_flash_slow_points(0), 6);
        assert_eq!(gunpowder_flash_slow_points(400), 13);
        assert_eq!(gunpowder_flash_slow_points(10_000), 20);
    }

    #[test]
    fn parasitic_dot_records_caster_power_and_duration() {
        with_test_gs(|gs| {
            let (cn, co) = caster_and_target(gs);
            gs.characters[co].skill[SK_IMMUN][5] = 20;

            assert!(apply_parasitic_dot(
                gs,
                cn,
                co,
                50,
                SK_PARASITE as u16,
                TICKS * 24,
                b"Parasite",
            ));

            let item = active_spell_item(gs, co, SK_PARASITE).expect("parasite attached");
            assert_eq!(gs.items[item].data[0] as usize, cn);
            assert_eq!(gs.items[item].data[1], 0);
            assert_eq!(gs.items[item].power, 40);
            assert_eq!(gs.items[item].active, (TICKS * 24) as u32);
        });
    }

    #[test]
    fn parasitic_dot_spares_immortal_targets() {
        with_test_gs(|gs| {
            let (cn, co) = caster_and_target(gs);
            gs.characters[co].flags |= CharacterFlags::Immortal.bits();

            assert!(!apply_parasitic_dot(
                gs,
                cn,
                co,
                50,
                SK_PARASITE as u16,
                TICKS * 24,
                b"Parasite",
            ));
            assert!(active_spell_item(gs, co, SK_PARASITE).is_none());
        });
    }

    #[test]
    fn parasite_costs_mana_and_infects_target_and_cross_neighbours() {
        with_test_gs(|gs| {
            let (cn, co) = caster_and_target(gs);
            gs.characters[cn].skill[SK_PARASITE][5] = 50;
            gs.characters[cn].a_mana = 20_000;
            let (side, diagonal) = (3, 4);
            place_attacker(gs, cn, side, 0, 1);
            place_attacker(gs, cn, diagonal, 1, 1);

            skill_parasite(gs, cn);

            assert_eq!(gs.characters[cn].a_mana, 10_000);
            assert!(active_spell_item(gs, co, SK_PARASITE).is_some());
            assert!(active_spell_item(gs, side, SK_PARASITE).is_some());
            assert!(active_spell_item(gs, diagonal, SK_PARASITE).is_none());
        });
    }

    #[test]
    fn parasite_without_mana_does_nothing() {
        with_test_gs(|gs| {
            let (cn, co) = caster_and_target(gs);
            gs.characters[cn].skill[SK_PARASITE][5] = 50;
            gs.characters[cn].a_mana = 9_000;

            skill_parasite(gs, cn);

            assert_eq!(gs.characters[cn].a_mana, 9_000);
            assert!(active_spell_item(gs, co, SK_PARASITE).is_none());
        });
    }

    #[test]
    fn contagion_costs_mana_and_attaches_long_dot() {
        with_test_gs(|gs| {
            let (cn, co) = caster_and_target(gs);
            gs.characters[cn].skill[SK_CONTAGION][5] = 60;
            gs.characters[cn].a_mana = 50_000;

            skill_contagion(gs, cn);

            assert_eq!(gs.characters[cn].a_mana, 30_000);
            let item = active_spell_item(gs, co, SK_CONTAGION).expect("contagion attached");
            assert_eq!(gs.items[item].active, (TICKS * 60) as u32);
        });
    }

    #[test]
    fn disarm_penalises_weapon_skill_for_45_seconds() {
        with_test_gs(|gs| {
            let (cn, co) = caster_and_target(gs);

            assert!(apply_disarm(gs, cn, co, 40));
            let item = active_spell_item(gs, co, SK_DISARM).expect("disarm attached");
            assert_eq!(gs.items[item].skill[SK_WEAPON][1], -60);
            assert_eq!(gs.items[item].active, (TICKS * 45) as u32);

            // The penalty is clamped to 3..=150.
            assert!(apply_disarm(gs, cn, co, 1));
            let item = active_spell_item(gs, co, SK_DISARM).unwrap();
            assert_eq!(gs.items[item].skill[SK_WEAPON][1], -3);
            assert!(apply_disarm(gs, cn, co, 500));
            let item = active_spell_item(gs, co, SK_DISARM).unwrap();
            assert_eq!(gs.items[item].skill[SK_WEAPON][1], -150);
        });
    }

    #[test]
    fn disarm_spares_immortal_targets() {
        with_test_gs(|gs| {
            let (cn, co) = caster_and_target(gs);
            gs.characters[co].flags |= CharacterFlags::Immortal.bits();

            assert!(!apply_disarm(gs, cn, co, 40));
            assert!(active_spell_item(gs, co, SK_DISARM).is_none());
        });
    }

    #[test]
    fn deliver_death_requires_an_adjacent_target() {
        with_test_gs(|gs| {
            let (cn, co) = caster_and_target(gs);
            gs.map[11 + 10 * SERVER_MAPX as usize].ch = 0;
            place_attacker(gs, cn, co, 2, 0);
            gs.characters[cn].a_end = 100_000;

            skill_deliver_death(gs, cn);

            assert_eq!(gs.characters[cn].a_end, 100_000);
            assert!(active_spell_item(gs, cn, SK_DELIVER_DEATH).is_none());
        });
    }

    #[test]
    fn deliver_death_requires_endurance() {
        with_test_gs(|gs| {
            let (cn, _co) = caster_and_target(gs);
            gs.characters[cn].a_end = 74_999;

            skill_deliver_death(gs, cn);

            assert_eq!(gs.characters[cn].a_end, 74_999);
            assert!(active_spell_item(gs, cn, SK_DELIVER_DEATH).is_none());
        });
    }

    #[test]
    fn deliver_death_is_blocked_while_recharging() {
        with_test_gs(|gs| {
            let (cn, _co) = caster_and_target(gs);
            gs.characters[cn].a_end = 100_000;
            add_skill_cooldown(gs, cn, TICKS, SK_DELIVER_DEATH as u16, b"Cooldown");

            skill_deliver_death(gs, cn);

            assert_eq!(gs.characters[cn].a_end, 100_000);
        });
    }

    #[test]
    fn weapon_skill_scale_ramps_from_half_to_one_and_a_half() {
        assert_eq!(weapon_skill_scale_pct(0), 50);
        assert_eq!(weapon_skill_scale_pct(100), 100);
        assert_eq!(weapon_skill_scale_pct(200), 150);
        assert_eq!(weapon_skill_scale_pct(400), 150);
    }

    #[test]
    fn blade_dance_requires_endurance() {
        with_test_gs(|gs| {
            let (cn, _nr) = add_test_player(gs);
            gs.item_templates[1].used = USE_ACTIVE;
            gs.characters[cn].a_end = 99_999;

            skill_blade_dance(gs, cn);

            assert_eq!(gs.characters[cn].a_end, 99_999);
            assert!(active_spell_item(gs, cn, SK_BLADE_DANCE).is_none());
        });
    }

    #[test]
    fn blade_dance_spends_endurance_and_starts_cooldown() {
        with_test_gs(|gs| {
            let (cn, _nr) = add_test_player(gs);
            gs.item_templates[1].used = USE_ACTIVE;
            gs.characters[cn].a_end = 250_000;

            skill_blade_dance(gs, cn);
            assert_eq!(gs.characters[cn].a_end, 150_000);
            let cooldown = active_spell_item(gs, cn, SK_BLADE_DANCE).expect("cooldown added");
            assert_eq!(gs.items[cooldown].active, (TICKS * 30) as u32);

            skill_blade_dance(gs, cn);
            assert_eq!(gs.characters[cn].a_end, 150_000);
        });
    }
}
