use super::common::{
    add_skill_cooldown, add_spell, effective_resist, skill_on_cooldown, unbuffed_skill_power,
};
use super::{hostile_cast_preflight, resolve_offensive_target};
use crate::chlog;
use crate::effect::EffectManager;
use crate::game_state::GameState;
use crate::god::God;
use crate::helpers;
use core::constants::{CharacterFlags, ItemFlags, TICKS};
use core::skills::{
    SK_CONTAGION, SK_CURSE, SK_DISARM, SK_GASH, SK_GUNPOWDER_FLASH, SK_INNER_STRENGTH,
    SK_RAINS_OF_RENEWAL, SK_RESIST, SK_SEEING_RED, SK_STUN, SK_SUNS_BLESSING, SK_SUNS_BLESSING2,
    SK_THUNDEROUS_FURY, SK_WARCRY, SK_WARCRY2, SK_WEAPON,
};
use core::types::FontColor;

/// Attempts to apply Warcry effects to one target.
///
/// # Arguments
///
/// * `gs` - Active game state used for attack validation, item creation, and effects.
/// * `cn` - Caster character index.
/// * `co` - Target character index.
/// * `power` - Warcry power checked against target resistance.
///
/// # Returns
///
/// * `true` when Warcry effects were applied, otherwise `false`.
///
/// # Panics
///
/// * Panics if `cn`, `co`, or a created spell item index is invalid.
pub fn warcry(gs: &mut GameState, cn: usize, co: usize, power: i32) -> bool {
    if gs.characters[cn].attack_cn as usize != co && gs.characters[co].alignment == 10000 {
        return false;
    }

    if !gs.may_aoe_attack(cn, co) {
        return false;
    }

    if power < effective_resist(gs, cn, i32::from(gs.characters[co].skill[SK_RESIST][5])) {
        return false;
    }

    for n in 1..10 {
        if gs.characters[cn].data[n] as usize == co {
            return false;
        }
    }

    if (gs.characters[co].flags & CharacterFlags::Immortal.bits()) != 0 {
        return false;
    }

    if gs.characters[co].flags & CharacterFlags::SpellIgnore.bits() == 0 {
        gs.do_notify_character(
            co as u32,
            i32::from(core::constants::NT_GOTHIT),
            cn as i32,
            0,
            0,
            0,
        );
    }

    let in_opt = God::create_item(gs, 1);
    if in_opt.is_none() {
        log::error!("god_create_item failed in skill_warcry");
        return false;
    }
    let in_idx = in_opt.unwrap();

    {
        let mut name_bytes = [0u8; 40];
        let name = b"War-Stun";
        let len = name.len().min(40);
        name_bytes[..len].copy_from_slice(&name[..len]);
        gs.items[in_idx].name = name_bytes;
        gs.items[in_idx].flags |= ItemFlags::IF_SPELL.bits();
        gs.items[in_idx].sprite[1] = 91;
        gs.items[in_idx].duration = core::constants::TICKS as u32 * 3;
        gs.items[in_idx].active = core::constants::TICKS as u32 * 3;
        gs.items[in_idx].temp = SK_WARCRY2 as u16;
        gs.items[in_idx].power = power as u32;
    }

    add_spell(gs, co, in_idx);

    let in2_opt = God::create_item(gs, 1);
    if in2_opt.is_none() {
        log::error!("god_create_item failed in skill_warcry");
        return false;
    }
    let in2 = in2_opt.unwrap();
    {
        let mut name_bytes = [0u8; 40];
        let name = b"Warcry";
        let len = name.len().min(40);
        name_bytes[..len].copy_from_slice(&name[..len]);
        gs.items[in2].name = name_bytes;
        gs.items[in2].flags |= ItemFlags::IF_SPELL.bits();
        for n in 0..5 {
            gs.items[in2].attrib[n][1] = -15;
        }
        gs.items[in2].sprite[1] = 89;
        gs.items[in2].duration = (TICKS * 60) as u32;
        gs.items[in2].active = (TICKS * 60) as u32;
        gs.items[in2].temp = SK_WARCRY as u16;
        gs.items[in2].power = (power / 2) as u32;
    }

    add_spell(gs, co, in2);

    let co_name = gs.characters[co].get_name().to_owned();
    chlog!(cn, "Cast Warcry on {}", co_name);

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

/// Handles direct player/NPC use of the Warcry skill over nearby targets.
///
/// # Arguments
///
/// * `gs` - Active game state used for endurance costs, nearby target lookup, and spell application.
/// * `cn` - Caster character index.
///
/// # Panics
///
/// * Panics if `cn` is invalid or a scanned map index is invalid.
pub fn skill_warcry(gs: &mut GameState, cn: usize) {
    if gs.characters[cn].a_end < 150 * 1000 {
        gs.do_character_log(cn, core::types::FontColor::Red, "You're too exhausted!\n");
        return;
    }

    gs.characters[cn].a_end -= 150 * 1000;

    let power = i32::from(gs.characters[cn].skill[SK_WARCRY][5]);

    let xf = std::cmp::max(1, i32::from(gs.characters[cn].x) - 10);
    let yf = std::cmp::max(1, i32::from(gs.characters[cn].y) - 10);
    let xt = std::cmp::min(
        core::constants::SERVER_MAPX - 1,
        i32::from(gs.characters[cn].x) + 10,
    );
    let yt = std::cmp::min(
        core::constants::SERVER_MAPY - 1,
        i32::from(gs.characters[cn].y) + 10,
    );

    let mut hit = 0;
    let mut miss = 0;
    for x in xf..xt {
        for y in yf..yt {
            let m = (x + y * core::constants::SERVER_MAPX) as usize;
            let co = gs.map[m].ch as usize;
            if co != 0 {
                if warcry(gs, cn, co, power) {
                    gs.remember_pvp(cn, co);
                    let name = gs.characters[cn].get_name().to_owned();
                    gs.do_character_log(
                        co,
                        core::types::FontColor::Green,
                        &format!(
                            "You hear {}'s warcry. You feel frightened and immobilized.\n",
                            name
                        ),
                    );
                    hit += 1;
                } else {
                    let name = gs.characters[cn].get_name().to_owned();
                    gs.do_character_log(
                        co,
                        core::types::FontColor::Green,
                        &format!("You hear {}'s warcry.\n", name),
                    );
                    miss += 1;
                }
            }
        }
    }
    gs.do_character_log(
        cn,
        core::types::FontColor::Green,
        &format!(
            "You cry out loud and clear. You affected {} of {} creatures in range.\n",
            hit,
            hit + miss
        ),
    );
}

/// Returns whether `temp` corresponds to a debuff blocked by Seeing Red.
///
/// Cooldown markers and passive buffs are intentionally excluded so that the
/// caster's own Seeing Red item still attaches.
///
/// # Arguments
///
/// * `temp` - The `item.temp` of an incoming spell attachment.
///
/// # Returns
///
/// * `true` if Seeing Red should refuse this attachment.
pub(crate) fn is_seeing_red_blocked_temp(temp: u16) -> bool {
    matches!(
        temp as usize,
        SK_STUN | SK_WARCRY2 | SK_CURSE | SK_CONTAGION | SK_GUNPOWDER_FLASH | SK_DISARM
    )
}

/// Returns whether `cn` currently has an active Seeing Red spell item.
///
/// # Arguments
///
/// * `gs` - Game state holding spell slots and item table.
/// * `cn` - Character index to inspect.
///
/// # Returns
///
/// * `true` if a Seeing Red marker is present in `character.spell[]`.
pub(crate) fn has_active_seeing_red(gs: &GameState, cn: usize) -> bool {
    for n in 0..20 {
        let in_ = gs.characters[cn].spell[n] as usize;
        if in_ != 0 && gs.items[in_].temp == SK_SEEING_RED as u16 {
            return true;
        }
    }
    false
}

/// Active self-cast: Rains of Renewal. Spends endurance up front and attaches
/// a heal-over-time spell item that ticks via the spell processor in
/// `state/stats.rs`.
///
/// # Arguments
///
/// * `gs` - Game state.
/// * `cn` - Caster character index.
pub fn skill_rains_of_renewal(gs: &mut GameState, cn: usize) {
    if skill_on_cooldown(gs, cn, SK_RAINS_OF_RENEWAL as u16) {
        return;
    }
    if gs.characters[cn].a_end < 50 * 1000 {
        gs.do_character_log(cn, FontColor::Red, "You're too exhausted!\n");
        return;
    }
    gs.characters[cn].a_end -= 50 * 1000;

    let power = i32::from(gs.characters[cn].skill[SK_RAINS_OF_RENEWAL][5]);
    let duration = TICKS * 60;

    let in_opt = God::create_item(gs, 1);
    if in_opt.is_none() {
        log::error!("god_create_item failed in skill_rains_of_renewal");
        return;
    }
    let in_idx = in_opt.unwrap();

    let item = &mut gs.items[in_idx];
    let mut name_bytes = [0u8; 40];
    let name = b"Rains of Renewal";
    let nlen = name.len().min(40);
    name_bytes[..nlen].copy_from_slice(&name[..nlen]);
    item.name = name_bytes;
    item.flags |= ItemFlags::IF_SPELL.bits();
    item.sprite[1] = 88; // TODO: assign unique spell-effect sprite ID once buff sprite assets are finalized
    item.duration = duration as u32;
    item.active = duration as u32;
    item.temp = SK_RAINS_OF_RENEWAL as u16;
    item.power = power.max(1) as u32;

    if add_spell(gs, cn, in_idx) == 0 {
        gs.do_character_log(
            cn,
            FontColor::Green,
            "Magical interference neutralised your blessing.\n",
        );
        return;
    }

    gs.do_character_log(cn, FontColor::Green, "Renewing rains wash over you.\n");
    chlog!(cn, "Cast Rains of Renewal");
    EffectManager::fx_add_effect(
        gs,
        7,
        0,
        i32::from(gs.characters[cn].x),
        i32::from(gs.characters[cn].y),
        0,
    );
}

/// Active melee strike: Gash. Pays 5% of current HP and deals weapon damage
/// scaled by the Gash skill power, then enters a short cooldown.
///
/// # Arguments
///
/// * `gs` - Game state.
/// * `cn` - Caster character index.
pub fn skill_gash(gs: &mut GameState, cn: usize) {
    let co = resolve_offensive_target(gs, cn);
    if !hostile_cast_preflight(gs, cn, co, "You cannot gash yourself.\n") {
        return;
    }
    if skill_on_cooldown(gs, cn, SK_GASH as u16) {
        return;
    }
    let dx = (i32::from(gs.characters[cn].x) - i32::from(gs.characters[co].x)).abs();
    let dy = (i32::from(gs.characters[cn].y) - i32::from(gs.characters[co].y)).abs();
    if dx > 1 || dy > 1 {
        gs.do_character_log(cn, FontColor::Red, "Your target is too far away.\n");
        return;
    }

    // Self damage: 5% of current HP, halved by the talent buff (a_hp is stored in 1/1000ths).
    let self_dam_units = (gs.characters[cn].a_hp / 40).max(1);
    if gs.characters[cn].a_hp <= self_dam_units {
        gs.do_character_log(
            cn,
            FontColor::Red,
            "You are too wounded to gash yourself.\n",
        );
        return;
    }
    gs.characters[cn].a_hp -= self_dam_units;

    let weapon = i32::from(gs.characters[cn].weapon).max(1);
    let power = i32::from(gs.characters[cn].skill[SK_GASH][5]);
    // Amplification scales with skill power: +90% at power=30, +450% at power=150.
    let bonus_pct = (power * 3).clamp(30, 600);
    let dam = weapon * (100 + bonus_pct) / 100 + helpers::random_mod_i32(weapon.max(1) * 3);

    let applied = gs.do_hurt(cn, co, dam, 0);
    if applied < 1 {
        gs.do_character_log(cn, FontColor::Green, "Your blow glances off harmlessly.\n");
    } else {
        let name = gs.characters[co].get_name().to_owned();
        gs.do_character_log(
            cn,
            FontColor::Green,
            &format!("You gash {} for {} HP.\n", name, applied),
        );
        chlog!(cn, "Cast Gash on {} for {} HP", name, applied);
    }

    let tx = i32::from(gs.characters[co].x);
    let ty = i32::from(gs.characters[co].y);
    gs.do_area_sound(co, 0, tx, ty, i32::from(gs.characters[cn].sound) + 4);
    EffectManager::fx_add_effect(gs, 5, 0, tx, ty, 0);

    add_skill_cooldown(gs, cn, TICKS * 10, SK_GASH as u16, b"Gash Cooldown");
}

/// Active self-buff: Sun's Blessing. No resource cost. Long cooldown set
/// slightly shorter than the buff duration so the player can recast as the
/// effect is expiring without stacking.
///
/// # Arguments
///
/// * `gs` - Game state.
/// * `cn` - Caster character index.
pub fn skill_suns_blessing(gs: &mut GameState, cn: usize) {
    if skill_on_cooldown(gs, cn, SK_SUNS_BLESSING as u16) {
        return;
    }
    let power = unbuffed_skill_power(gs, cn, SK_SUNS_BLESSING);
    let bonus = ((power / 10 + 2) * 3).clamp(3, 90) as i16;
    let buff_duration = TICKS * 180;
    let cooldown_len = TICKS * 55;

    let in_opt = God::create_item(gs, 1);
    if in_opt.is_none() {
        log::error!("god_create_item failed in skill_suns_blessing");
        return;
    }
    let in_idx = in_opt.unwrap();

    let item = &mut gs.items[in_idx];
    let mut name_bytes = [0u8; 40];
    let name = b"Sun's Blessing";
    let nlen = name.len().min(40);
    name_bytes[..nlen].copy_from_slice(&name[..nlen]);
    item.name = name_bytes;
    item.flags |= ItemFlags::IF_SPELL.bits();
    item.sprite[1] = 88; // TODO: assign unique spell-effect sprite ID once buff sprite assets are finalized
    item.duration = buff_duration as u32;
    item.active = buff_duration as u32;
    item.temp = SK_SUNS_BLESSING2 as u16;
    item.power = power.max(1) as u32;
    for n in 0..5 {
        item.attrib[n][1] = bonus;
    }
    item.armor[1] = bonus as i8;
    item.weapon[1] = bonus as i8;

    if add_spell(gs, cn, in_idx) == 0 {
        gs.do_character_log(
            cn,
            FontColor::Green,
            "Magical interference neutralised your blessing.\n",
        );
        return;
    }

    add_skill_cooldown(
        gs,
        cn,
        cooldown_len,
        SK_SUNS_BLESSING as u16,
        b"Sun's Blessing Cooldown",
    );

    gs.do_character_log(
        cn,
        FontColor::Green,
        "The sun's blessing strengthens you.\n",
    );
    chlog!(cn, "Cast Sun's Blessing");
    EffectManager::fx_add_effect(
        gs,
        6,
        0,
        i32::from(gs.characters[cn].x),
        i32::from(gs.characters[cn].y),
        0,
    );
}

/// Active self-buff: Seeing Red. Spends endurance; while active the caster
/// gains a large temporary weapon-skill bonus and refuses to receive new
/// stun/curse/gunpowder-flash/disarm style debuff attachments.
///
/// # Arguments
///
/// * `gs` - Game state.
/// * `cn` - Caster character index.
pub fn skill_seeing_red(gs: &mut GameState, cn: usize) {
    if skill_on_cooldown(gs, cn, SK_SEEING_RED as u16) {
        return;
    }
    if gs.characters[cn].a_end < 75 * 1000 {
        gs.do_character_log(cn, FontColor::Red, "You're too exhausted!\n");
        return;
    }
    gs.characters[cn].a_end -= 75 * 1000;

    let power = i32::from(gs.characters[cn].skill[SK_SEEING_RED][5]);
    // Roughly doubles outgoing damage per unit weapon by mirroring 3x the
    // caster's current weapon value as a flat weapon[1] bonus, i8-capped.
    let weapon = (i32::from(gs.characters[cn].weapon) * 3).clamp(3, 127) as i8;
    let duration = TICKS * (15 + (power / 5).clamp(0, 75));
    let cooldown_len = TICKS * 60;

    let in_opt = God::create_item(gs, 1);
    if in_opt.is_none() {
        log::error!("god_create_item failed in skill_seeing_red");
        return;
    }
    let in_idx = in_opt.unwrap();

    let item = &mut gs.items[in_idx];
    let mut name_bytes = [0u8; 40];
    let name = b"Seeing Red";
    let nlen = name.len().min(40);
    name_bytes[..nlen].copy_from_slice(&name[..nlen]);
    item.name = name_bytes;
    item.flags |= ItemFlags::IF_SPELL.bits();
    item.sprite[1] = 88; // TODO: assign unique spell-effect sprite ID once buff sprite assets are finalized
    item.duration = duration as u32;
    item.active = duration as u32;
    item.temp = SK_SEEING_RED as u16;
    item.power = power.max(1) as u32;
    item.weapon[1] = weapon;

    if add_spell(gs, cn, in_idx) == 0 {
        gs.do_character_log(
            cn,
            FontColor::Green,
            "Magical interference neutralised your fury.\n",
        );
        return;
    }

    add_skill_cooldown(
        gs,
        cn,
        cooldown_len,
        SK_SEEING_RED as u16,
        b"Seeing Red Cooldown",
    );

    gs.do_character_log(cn, FontColor::Green, "You are seeing red!\n");
    chlog!(cn, "Cast Seeing Red");
    EffectManager::fx_add_effect(
        gs,
        5,
        0,
        i32::from(gs.characters[cn].x),
        i32::from(gs.characters[cn].y),
        0,
    );
}

/// Active AoE: Thunderous Fury. Upgraded Warcry that stuns every nearby
/// hostile and deals a weakened blast of damage to each. Long cooldown.
///
/// # Arguments
///
/// * `gs` - Game state.
/// * `cn` - Caster character index.
pub fn skill_thunderous_fury(gs: &mut GameState, cn: usize) {
    if skill_on_cooldown(gs, cn, SK_THUNDEROUS_FURY as u16) {
        return;
    }
    if gs.characters[cn].a_end < 250 * 1000 {
        gs.do_character_log(cn, FontColor::Red, "You're too exhausted!\n");
        return;
    }
    gs.characters[cn].a_end -= 250 * 1000;

    let power = i32::from(gs.characters[cn].skill[SK_THUNDEROUS_FURY][5]);
    let blast_base = ((power * 3) / 4).max(1);

    let xf = std::cmp::max(1, i32::from(gs.characters[cn].x) - 10);
    let yf = std::cmp::max(1, i32::from(gs.characters[cn].y) - 10);
    let xt = std::cmp::min(
        core::constants::SERVER_MAPX - 1,
        i32::from(gs.characters[cn].x) + 10,
    );
    let yt = std::cmp::min(
        core::constants::SERVER_MAPY - 1,
        i32::from(gs.characters[cn].y) + 10,
    );

    let mut hit = 0;
    let mut miss = 0;
    for x in xf..xt {
        for y in yf..yt {
            let m = (x + y * core::constants::SERVER_MAPX) as usize;
            let co = gs.map[m].ch as usize;
            if co == 0 || co == cn {
                continue;
            }
            if warcry(gs, cn, co, power) {
                gs.remember_pvp(cn, co);
                let name = gs.characters[cn].get_name().to_owned();
                gs.do_character_log(
                    co,
                    FontColor::Green,
                    &format!("{}'s thunderous fury crashes over you!\n", name),
                );
                let dam = blast_base + helpers::random_mod_i32(blast_base);
                let _ = gs.do_hurt(cn, co, dam, 0);
                let tx = i32::from(gs.characters[co].x);
                let ty = i32::from(gs.characters[co].y);
                EffectManager::fx_add_effect(gs, 5, 0, tx, ty, 0);
                hit += 1;
            } else {
                miss += 1;
            }
        }
    }

    gs.do_character_log(
        cn,
        FontColor::Green,
        &format!(
            "Your thunderous fury strikes {} of {} foes.\n",
            hit,
            hit + miss
        ),
    );
    chlog!(cn, "Cast Thunderous Fury ({} hits)", hit);

    add_skill_cooldown(
        gs,
        cn,
        TICKS * 30,
        SK_THUNDEROUS_FURY as u16,
        b"Thunderous Fury Cooldown",
    );
}

/// Active AoE + self buff: Inner Strength. Upgraded Warcry that stuns nearby
/// hostiles and grants the caster a temporary weapon-skill bonus.
///
/// # Arguments
///
/// * `gs` - Game state.
/// * `cn` - Caster character index.
pub fn skill_inner_strength(gs: &mut GameState, cn: usize) {
    if skill_on_cooldown(gs, cn, SK_INNER_STRENGTH as u16) {
        return;
    }
    if gs.characters[cn].a_end < 200 * 1000 {
        gs.do_character_log(cn, FontColor::Red, "You're too exhausted!\n");
        return;
    }
    gs.characters[cn].a_end -= 200 * 1000;

    let power = i32::from(gs.characters[cn].skill[SK_INNER_STRENGTH][5]);
    let buff_amount = (((power / 5) + 2) * 3).clamp(3, 150) as i16;
    let buff_duration = TICKS * 90;

    // Self weapon-skill buff item.
    let in_opt = God::create_item(gs, 1);
    if in_opt.is_none() {
        log::error!("god_create_item failed in skill_inner_strength");
        return;
    }
    let in_idx = in_opt.unwrap();

    let item = &mut gs.items[in_idx];
    let mut name_bytes = [0u8; 40];
    let name = b"Inner Strength";
    let nlen = name.len().min(40);
    name_bytes[..nlen].copy_from_slice(&name[..nlen]);
    item.name = name_bytes;
    item.flags |= ItemFlags::IF_SPELL.bits();
    item.sprite[1] = 88; // TODO: assign unique spell-effect sprite ID once buff sprite assets are finalized
    item.duration = buff_duration as u32;
    item.active = buff_duration as u32;
    item.temp = SK_INNER_STRENGTH as u16;
    item.power = power.max(1) as u32;
    item.skill[SK_WEAPON][1] = buff_amount;

    if add_spell(gs, cn, in_idx) == 0 {
        gs.do_character_log(
            cn,
            FontColor::Green,
            "Magical interference neutralised your shout.\n",
        );
        return;
    }

    // AoE stun (Warcry behaviour) over the surrounding tiles.
    let xf = std::cmp::max(1, i32::from(gs.characters[cn].x) - 10);
    let yf = std::cmp::max(1, i32::from(gs.characters[cn].y) - 10);
    let xt = std::cmp::min(
        core::constants::SERVER_MAPX - 1,
        i32::from(gs.characters[cn].x) + 10,
    );
    let yt = std::cmp::min(
        core::constants::SERVER_MAPY - 1,
        i32::from(gs.characters[cn].y) + 10,
    );

    let mut hit = 0;
    let mut miss = 0;
    for x in xf..xt {
        for y in yf..yt {
            let m = (x + y * core::constants::SERVER_MAPX) as usize;
            let co = gs.map[m].ch as usize;
            if co == 0 || co == cn {
                continue;
            }
            if warcry(gs, cn, co, power) {
                gs.remember_pvp(cn, co);
                hit += 1;
            } else {
                miss += 1;
            }
        }
    }

    gs.do_character_log(
        cn,
        FontColor::Green,
        &format!(
            "Inner strength steadies you; {} of {} foes are shaken.\n",
            hit,
            hit + miss
        ),
    );
    chlog!(cn, "Cast Inner Strength ({} hits)", hit);

    add_skill_cooldown(
        gs,
        cn,
        TICKS * 30,
        SK_INNER_STRENGTH as u16,
        b"Inner Strength Cooldown",
    );
}
