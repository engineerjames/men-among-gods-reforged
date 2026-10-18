use super::templar::{has_active_seeing_red, is_seeing_red_blocked_temp};
use crate::game_state::GameState;
use crate::god::God;
use crate::player_logging;
use core::constants::{CharacterFlags, DX_DOWN, DX_LEFT, DX_RIGHT, DX_UP, ItemFlags};
use core::skills::{SK_BLAST, SK_CONCEN};
use core::traits::{
    KIN_ARCHHARAKIM, KIN_ARCHTEMPLAR, KIN_HARAKIM, KIN_MERCENARY, KIN_SEYAN_DU, KIN_SORCERER,
    KIN_TEMPLAR, KIN_WARRIOR,
};
use core::types::{Character, FontColor};

/// Returns whether `co` is a player or the ghost companion owned by `cn`.
///
/// # Arguments
///
/// * `cn` - Candidate owner character.
/// * `cn_idx` - Character index for `cn`.
/// * `co` - Candidate target character.
///
/// # Returns
///
/// * `true` when `cn` is a player and `co` is either a player or `cn`'s ghost companion.
pub fn player_or_ghost(cn: &Character, cn_idx: usize, co: &Character) -> bool {
    if (cn.flags & CharacterFlags::Player.bits()) == 0 {
        return false;
    }
    if (co.flags & CharacterFlags::Player.bits()) != 0 {
        return true;
    }
    if co.data[63] as usize == cn_idx {
        return true;
    }
    false
}

/// Consumes mana for a spell after applying Concentration cost reduction.
///
/// # Arguments
///
/// * `gs` - Active game state containing character mana and skills.
/// * `cn` - Caster character index.
/// * `cost` - Base spell cost in whole mana units.
///
/// # Returns
///
/// * `0` when mana was consumed successfully, or `-1` when the caster lacks mana.
///
/// # Panics
///
/// * Panics if `cn` is not a valid character index.
pub fn spellcost(gs: &mut GameState, cn: usize, cost: i32) -> i32 {
    // Ported from C++ spellcost(int cn, int cost)
    // concentrate:
    let mut cost = cost;
    let concen_skill = gs.characters[cn].skill[SK_CONCEN][0];
    if concen_skill != 0 {
        let concen_val = gs.characters[cn].skill[SK_CONCEN][5];
        let t = cost * i32::from(concen_val) / 300;
        if t > cost {
            cost = 1;
        } else {
            cost -= t;
        }
    }
    let a_mana = gs.characters[cn].a_mana;
    if cost * 1000 > a_mana {
        player_logging::log_event(
            cn,
            player_logging::PlayerLogCategory::Spell,
            player_logging::PlayerLogOutcome::Rejected,
            "insufficient_mana",
        );
        gs.do_character_log(
            cn,
            core::types::FontColor::Red,
            "You don't have enough mana.\n",
        );
        return -1;
    }
    gs.characters[cn].a_mana = a_mana - cost * 1000;
    0
}

/// Consumes endurance for a spell-like action.
///
/// # Arguments
///
/// * `gs` - Active game state containing character endurance.
/// * `cn` - Caster character index.
/// * `cost` - Base endurance cost in whole endurance units.
///
/// # Returns
///
/// * `0` when endurance was consumed successfully, or `-1` when the caster lacks endurance.
pub fn spellcost_endurance(gs: &mut GameState, cn: usize, cost: i32) -> i32 {
    let a_end = gs.characters[cn].a_end;
    if cost * 1000 > a_end {
        player_logging::log_event(
            cn,
            player_logging::PlayerLogCategory::Spell,
            player_logging::PlayerLogOutcome::Rejected,
            "insufficient_endurance",
        );
        gs.do_character_log(cn, FontColor::Red, "You're too exhausted!\n");
        return -1;
    }
    gs.characters[cn].a_end = a_end - cost * 1000;
    0
}

/// Performs a focus check against an opposing skill or resistance power.
///
/// # Arguments
///
/// * `gs` - Active game state used for caster state and failure messages.
/// * `cn` - Caster character index.
/// * `skill` - Caster skill value used for the check.
/// * `d20` - Base d20 threshold before luck adjustment.
/// * `power` - Opposing power or resistance value.
///
/// # Returns
///
/// * `0` when focus succeeds, or `-1` when focus is lost.
///
/// # Panics
///
/// * Panics if `cn` is not a valid character index.
pub fn chance_base(gs: &mut GameState, cn: usize, skill: i32, d20: i32, power: i32) -> i32 {
    // Ported from C++ chance_base(int cn, int skill, int d20, int power)
    let mut chance = d20 * skill / std::cmp::max(1, power);
    let (flags, luck) = (gs.characters[cn].flags, gs.characters[cn].luck);
    if (flags & CharacterFlags::Player.bits()) != 0 && luck < 0 {
        chance += luck / 500 - 1;
    }

    chance = chance.clamp(0, 18);

    let roll = crate::helpers::random_mod(20);
    if roll as i32 > chance || power > skill + (skill / 2) {
        gs.do_character_log(cn, core::types::FontColor::Red, "You lost your focus!\n");
        return -1;
    }
    0
}

/// Reduces spell power by a target immunity value.
///
/// # Arguments
///
/// * `_gs` - Game state reserved for compatibility with the legacy signature.
/// * `power` - Incoming spell power.
/// * `immun` - Target immunity skill value.
///
/// # Returns
///
/// * Effective spell power after immunity mitigation, with a minimum of `1`.
pub fn spell_immunity(_gs: &GameState, power: i32, immun: i32) -> i32 {
    // Ported from C++ spell_immunity(int power, int immun)
    let immun = immun / 2;
    if power <= immun { 1 } else { power - immun }
}

/// Reduces a target's effective resistance value by the caster's talent-derived
/// spell penetration percent.
///
/// Only affects whether a spell *lands* (the `SK_RESIST` opposed rolls and
/// `chance_base` checks); it deliberately does not touch `spell_immunity`, so
/// damage-scaling spells like Blast keep their power unaffected by this talent.
///
/// # Arguments
///
/// * `gs` - Active game state used to read the caster's cached talent bonuses.
/// * `caster_cn` - Caster character index.
/// * `raw_resist` - Target's raw `SK_RESIST` value before penetration.
///
/// # Returns
///
/// * `raw_resist` scaled down by `100 / (100 + penetration_percent)`.
pub fn effective_resist(gs: &GameState, caster_cn: usize, raw_resist: i32) -> i32 {
    let pct = gs.talent_runtime[caster_cn].spell_penetration_percent;
    if pct == 0 {
        return raw_resist;
    }
    raw_resist * 100 / (100 + pct)
}

/// Applies caster kindred and moon-phase modifiers to spell power.
///
/// # Arguments
///
/// * `gs` - Active game state containing global moon-phase flags.
/// * `power` - Base spell power.
/// * `kindred` - Caster kindred bitfield.
///
/// # Returns
///
/// * Modified spell power after kindred and moon-phase adjustments.
pub fn spell_race_mod(gs: &GameState, power: i32, kindred: i32) -> i32 {
    // Ported from C++ spell_race_mod(int power, int kindred)

    let mut modf;
    if (kindred & KIN_ARCHHARAKIM as i32) != 0 {
        modf = 1.05;
    } else if (kindred & KIN_ARCHTEMPLAR as i32) != 0 {
        modf = 0.95;
    } else if (kindred & KIN_SORCERER as i32) != 0 || (kindred & KIN_WARRIOR as i32) != 0 {
        modf = 1.10;
    } else if (kindred & KIN_SEYAN_DU as i32) != 0 {
        modf = 0.95;
    } else if (kindred & KIN_HARAKIM as i32) != 0 {
        modf = 1.00;
    } else if (kindred & KIN_MERCENARY as i32) != 0 {
        modf = 1.05;
    } else if (kindred & KIN_TEMPLAR as i32) != 0 {
        modf = 0.90;
    } else {
        modf = 1.00;
    }

    if gs.globals.newmoon != 0 {
        modf -= 0.15;
    }
    if gs.globals.fullmoon != 0 {
        modf += 0.15;
    }

    (f64::from(power) * modf) as i32
}

/// Adds a spell item to a character's active spell slots.
///
/// Replaces weaker duplicate spells, rejects weaker overwrites, and evicts the
/// weakest active spell when all slots are full and the new spell is stronger.
///
/// # Arguments
///
/// * `gs` - Active game state containing characters, items, and map flags.
/// * `cn` - Target character index receiving the spell.
/// * `in_` - Spell item index to attach.
///
/// # Returns
///
/// * `1` when the spell was attached, or `0` when it was rejected or neutralized.
///
/// # Panics
///
/// * Panics if `cn`, `in_`, an active spell item index, or the character's map position is invalid.
pub fn add_spell(gs: &mut GameState, cn: usize, in_: usize) -> i32 {
    // Ported from C++ add_spell(int cn, int in)
    let mut n = 0;
    let in2: usize;
    let mut weak = 999;
    let mut weakest = 99;
    let mut rejected = false;
    let m = gs.characters[cn].x as usize
        + gs.characters[cn].y as usize * core::constants::SERVER_MAPX as usize;
    let nomagic = gs.map[m].flags & CharacterFlags::NoMagic.bits() != 0;
    if nomagic {
        return 0;
    }
    // Seeing Red grants immunity to new stun/slow/curse/disarm style debuffs
    // while active. Cooldown / passive temps fall outside this list, so the
    // recipient's own Seeing Red marker is never rejected here.
    if is_seeing_red_blocked_temp(gs.items[in_].temp) && has_active_seeing_red(gs, cn) {
        gs.items[in_].used = core::constants::USE_EMPTY;
        return 0;
    }
    // Overwrite spells if same spell is cast twice and the new spell is more powerful
    let mut found = false;

    for i in 0..20 {
        if gs.characters[cn].spell[i] != 0 {
            let it_in2 = gs.characters[cn].spell[i] as usize;
            let temp_in2 = gs.items[it_in2].temp;
            let temp_in = gs.items[in_].temp;
            if temp_in2 == temp_in {
                let power_in = gs.items[in_].power;
                let power_in2 = gs.items[it_in2].power;
                let active_in2 = gs.items[it_in2].active;
                if power_in < power_in2 && active_in2 > core::constants::TICKS as u32 * 60 {
                    gs.items[in_].used = core::constants::USE_EMPTY;
                    rejected = true;
                    found = true;
                    break;
                }
                gs.items[it_in2].used = core::constants::USE_EMPTY;
                n = i;
                found = true;
                break;
            }
        }
    }
    if rejected {
        return 0;
    }
    if found {
        // n is set by the loop above
    } else {
        // Find empty slot or weakest spell
        let mut empty_found = false;
        for i in 0..20 {
            if gs.characters[cn].spell[i] == 0 {
                n = i;
                empty_found = true;
                break;
            }
            let it_in2 = gs.characters[cn].spell[i] as usize;
            let power_in2 = gs.items[it_in2].power;
            if power_in2 < weak {
                weak = power_in2;
                weakest = i;
            }
        }
        if !empty_found {
            let power_in = gs.items[in_].power;
            if weak < 999 && weak < power_in {
                n = weakest;
                in2 = gs.characters[cn].spell[n] as usize;
                gs.items[in2].used = core::constants::USE_EMPTY;
            } else {
                gs.items[in_].used = core::constants::USE_EMPTY;
                return 0;
            }
        }
    }
    // Assign spell
    gs.characters[cn].spell[n] = in_ as u32;
    gs.items[in_].carried = cn as u16;
    gs.do_update_char(cn);
    1
}

/// Returns whether a character currently has the spell-exhaustion marker.
///
/// # Arguments
///
/// * `gs` - Active game state containing character spell slots and items.
/// * `cn` - Character index to inspect.
///
/// # Returns
///
/// * `true` when a spell-exhaustion marker is active, otherwise `false`.
///
/// # Panics
///
/// * Panics if `cn` or an active spell item index is invalid.
pub fn is_exhausted(gs: &mut GameState, cn: usize) -> bool {
    for n in 0..20 {
        let in_ = gs.characters[cn].spell[n] as usize;
        if in_ != 0 {
            let temp = gs.items[in_].temp;
            if temp == SK_BLAST as u16 {
                gs.do_character_log(
                    cn,
                    core::types::FontColor::Red,
                    "You are still exhausted from your last spell!\n",
                );
                return true;
            }
        }
    }
    false
}

/// Adds a temporary spell-exhaustion marker to a character.
///
/// # Arguments
///
/// * `gs` - Active game state used to create and attach the exhaustion item.
/// * `cn` - Character index receiving exhaustion.
/// * `exhaust_length` - Exhaustion duration in ticks.
///
/// # Panics
///
/// * Panics if `cn` is invalid or the created item index becomes invalid.
pub fn add_exhaust(gs: &mut GameState, cn: usize, exhaust_length: i32) {
    // Ported from C++ add_exhaust(int cn, int len)
    let in_ = God::create_item(gs, 1);
    if in_.is_none() {
        log::error!("god_create_item failed in add_exhaust");
        return;
    }
    {
        let item = &mut gs.items[in_.unwrap()];
        let mut name_bytes = [0u8; 40];
        let name = b"Spell Exhaustion";
        let len = name.len().min(40);
        name_bytes[..len].copy_from_slice(&name[..len]);
        item.name = name_bytes;
        item.flags |= ItemFlags::IF_SPELL.bits();
        item.sprite[1] = 97;
        item.duration = exhaust_length as u32;
        item.active = exhaust_length as u32;
        item.temp = SK_BLAST as u16;
        item.power = 255;
    }
    add_spell(gs, cn, in_.unwrap());
}

/// Converts an item template effect into an active spell on a character.
///
/// # Arguments
///
/// * `gs` - Active game state used to copy item bonuses and attach the spell.
/// * `cn` - Character index receiving the copied spell effect.
/// * `in2` - Source item index providing spell metadata and bonuses.
///
/// # Panics
///
/// * Panics if `cn`, `in2`, or the created spell item index is invalid.
pub fn spell_from_item(gs: &mut GameState, cn: usize, in2: usize) {
    // Ported from C++ spell_from_item(int cn, int in2)
    let flags = gs.characters[cn].flags;
    if (flags & CharacterFlags::NoMagic.bits()) != 0 {
        gs.do_character_log(
            cn,
            core::types::FontColor::Red,
            "The magic didn't work! Must be external influences.\n",
        );
        return;
    }
    let in_ = God::create_item(gs, 1);
    if in_.is_none() {
        log::error!("god_create_item failed in skill_from_item");
        return;
    }

    let in_ = in_.unwrap();
    {
        gs.items[in_].name = gs.items[in2].name;
        gs.items[in_].flags |= ItemFlags::IF_SPELL.bits();
        gs.items[in_].armor[1] = gs.items[in2].armor[1];
        gs.items[in_].weapon[1] = gs.items[in2].weapon[1];
        gs.items[in_].hp[1] = gs.items[in2].hp[1];
        gs.items[in_].end[1] = gs.items[in2].end[1];
        gs.items[in_].mana[1] = gs.items[in2].mana[1];
        gs.items[in_].sprite_override = gs.items[in2].sprite_override;
        for n in 0..5 {
            gs.items[in_].attrib[n][1] = gs.items[in2].attrib[n][1];
        }
        for n in 0..core::skills::MAX_SKILLS {
            gs.items[in_].skill[n][1] = gs.items[in2].skill[n][1];
        }
        let data0 = gs.items[in2].data[0];
        if data0 != 0 {
            gs.items[in_].sprite[1] = data0 as i16;
        } else {
            gs.items[in_].sprite[1] = 93;
        }
        let duration = gs.items[in2].duration;
        gs.items[in_].duration = duration;
        gs.items[in_].active = duration;
        let data1 = gs.items[in2].data[1];
        if data1 != 0 {
            gs.items[in_].temp = data1 as u16;
        } else {
            gs.items[in_].temp = 101;
        }
        gs.items[in_].power = gs.items[in2].power;
    }
    if add_spell(gs, cn, in_) == 0 {
        let name = gs.items[in_].get_name().to_owned();
        gs.do_character_log(
            cn,
            core::types::FontColor::Green,
            &format!("Magical interference neutralised the {}'s effect.\n", name,),
        );
        return;
    }
    gs.do_character_log(cn, core::types::FontColor::Green, "You feel changed.\n");
    let sound = gs.characters[cn].sound;
    GameState::char_play_sound(gs, cn, i32::from(sound) + 1, -150, 0);
}

/// Final skill value with the attribute bonuses of active spell items removed.
///
/// `skill[5]` folds in attributes buffed by Bless / Sun's Blessing, so reading it
/// when recasting those buffs lets them feed back into their own power.
///
/// # Arguments
///
/// * `gs` - Active game state.
/// * `cn` - Character whose skill is read.
/// * `skill` - Skill index.
///
/// # Returns
///
/// * Skill power as if no spell item were granting attribute bonuses.
pub(super) fn unbuffed_skill_power(gs: &GameState, cn: usize, skill: usize) -> i32 {
    let ch = &gs.characters[cn];
    let (mut buffed, mut base) = (0, 0);
    for a in core::skills::get_skill_attribs(skill) {
        let spell_bonus: i32 = ch.spell[..20]
            .iter()
            .filter(|&&s| s != 0)
            .map(|&s| i32::from(gs.items[s as usize].attrib[a][1]))
            .sum();
        buffed += i32::from(ch.attrib[a][5]);
        base += i32::from(ch.attrib[a][5]) - spell_bonus;
    }
    i32::from(ch.skill[skill][5]) - (buffed / 5 - base.max(0) / 5)
}

/// Computes the aggregate spellpower cap for a character.
///
/// # Arguments
///
/// * `cn` - Character whose primary attributes define the cap.
///
/// # Returns
///
/// * Sum of agility, strength, intelligence, willpower, and bravery base attributes.
pub fn spellpower(cn: &Character) -> i32 {
    let a = i32::from(cn.attrib[core::constants::AT_AGIL as usize][0]);
    let b = i32::from(cn.attrib[core::constants::AT_STREN as usize][0]);
    let c = i32::from(cn.attrib[core::constants::AT_INT as usize][0]);
    let d = i32::from(cn.attrib[core::constants::AT_WILL as usize][0]);
    let e = i32::from(cn.attrib[core::constants::AT_BRAVE as usize][0]);
    a + b + c + d + e
}

/// Removes all active spell items from a character.
///
/// # Arguments
///
/// * `gs` - Active game state containing character spell slots and spell items.
/// * `cn` - Character index whose spells should be removed.
///
/// # Panics
///
/// * Panics if `cn` or an active spell item index is invalid.
pub fn remove_spells(gs: &mut GameState, cn: usize) {
    for n in 0..20usize {
        let in_idx = gs.characters[cn].spell[n] as usize;
        if in_idx == 0 {
            continue;
        }
        gs.items[in_idx].used = core::constants::USE_EMPTY;
        gs.characters[cn].spell[n] = 0;
    }
    gs.do_update_char(cn);
}

/// Returns whether one character is facing another adjacent character.
///
/// # Arguments
///
/// * `cn` - Character whose facing direction is tested.
/// * `co` - Character whose position is compared against `cn`.
///
/// # Returns
///
/// * `true` when `co` is directly in front of `cn`, otherwise `false`.
pub fn is_facing(cn: &Character, co: &Character) -> bool {
    let dir = cn.dir;
    let cx = cn.x;
    let cy = cn.y;
    let ox = co.x;
    let oy = co.y;

    match dir {
        DX_RIGHT => cx + 1 == ox && cy == oy,
        DX_LEFT => cx - 1 == ox && cy == oy,
        DX_UP => cx == ox && cy - 1 == oy,
        DX_DOWN => cx == ox && cy + 1 == oy,
        _ => false,
    }
}

/// Returns whether one character has another adjacent character behind them.
///
/// # Arguments
///
/// * `cn` - Character whose facing direction is tested.
/// * `co` - Character whose position is compared against `cn`.
///
/// # Returns
///
/// * `true` when `co` is directly behind `cn`, otherwise `false`.
pub fn is_back(cn: &Character, co: &Character) -> bool {
    let dir = cn.dir;
    let cx = cn.x;
    let cy = cn.y;
    let ox = co.x;
    let oy = co.y;

    match dir {
        DX_LEFT => cx + 1 == ox && cy == oy,
        DX_RIGHT => cx - 1 == ox && cy == oy,
        DX_DOWN => cx == ox && cy - 1 == oy,
        DX_UP => cx == ox && cy + 1 == oy,
        _ => false,
    }
}

/// Logs the standard no-magic failure message for a character.
///
/// # Arguments
///
/// * `gs` - Active game state used for character logging.
/// * `cn` - Character index receiving the failure message.
///
/// # Panics
///
/// * Panics if `cn` is not a valid character index.
pub fn nomagic(gs: &mut GameState, cn: usize) {
    player_logging::log_event(
        cn,
        player_logging::PlayerLogCategory::Spell,
        player_logging::PlayerLogOutcome::Rejected,
        "magic_disabled",
    );
    gs.do_character_log(
        cn,
        FontColor::Green,
        "Your magic fails. You seem to be unable to cast spells.\n",
    );
}

/// Returns true if the character currently has an active spell-item with the
/// given skill `temp`, i.e. that skill is still on cooldown.
///
/// # Arguments
///
/// * `gs` - Mutable reference to the game state.
/// * `cn` - Character index to inspect.
/// * `skill_temp` - The `temp` value of the cooldown spell-item to look for
///   (typically the originating skill constant such as `SK_DELIVER_DEATH`).
///
/// # Returns
///
/// * `true` if a matching cooldown spell-item is still present.
pub(super) fn skill_on_cooldown(gs: &mut GameState, cn: usize, skill_temp: u16) -> bool {
    for n in 0..20 {
        let in_ = gs.characters[cn].spell[n] as usize;
        if in_ != 0 && gs.items[in_].temp == skill_temp {
            gs.do_character_log(cn, FontColor::Red, "That ability is still recharging.\n");
            return true;
        }
    }
    false
}

/// Adds a per-skill cooldown spell-item to the caster.
///
/// The created item has no stat effect; its sole purpose is to remain in
/// `character.spell[]` for `len` ticks so that `skill_on_cooldown` can detect
/// it. Mirrors the pattern of `add_exhaust` but uses a caller-supplied skill
/// `temp` so each ability tracks its own recharge timer independently.
///
/// # Arguments
///
/// * `gs` - Mutable reference to the game state.
/// * `cn` - Character index to receive the cooldown marker.
/// * `len` - Cooldown duration in ticks.
/// * `skill_temp` - Originating skill constant stored in `item.temp`.
/// * `name` - Display name used when the cooldown is reported (must fit 40 bytes).
pub(super) fn add_skill_cooldown(
    gs: &mut GameState,
    cn: usize,
    len: i32,
    skill_temp: u16,
    name: &[u8],
) {
    let in_ = match God::create_item(gs, 1) {
        Some(i) => i,
        None => {
            log::error!("god_create_item failed in add_skill_cooldown");
            return;
        }
    };

    let item = &mut gs.items[in_];
    let mut name_bytes = [0u8; 40];
    let nlen = name.len().min(40);
    name_bytes[..nlen].copy_from_slice(&name[..nlen]);
    item.name = name_bytes;
    item.flags |= ItemFlags::IF_SPELL.bits();
    item.sprite[1] = 97;
    item.duration = len as u32;
    item.active = len as u32;
    item.temp = skill_temp;
    item.power = 255;

    add_spell(gs, cn, in_);
}

/// Returns whether `cn` currently has an active spell-item whose `temp`
/// matches `skill_temp`.
///
/// Used by the new Harakim abilities (Revenant Conduit, Spectral Pact,
/// Anguish-Earth) to gate behaviour on whether the relevant buff/debuff is
/// still in effect.
///
/// # Arguments
///
/// * `gs` - Game state holding spell slots and item table.
/// * `cn` - Character index to inspect.
/// * `skill_temp` - The `temp` value to look for in `character.spell[]`.
///
/// # Returns
///
/// * `true` when an entry with a matching temp is present.
pub(crate) fn has_active_spell_temp(gs: &GameState, cn: usize, skill_temp: u16) -> bool {
    for n in 0..20 {
        let in_ = gs.characters[cn].spell[n] as usize;
        if in_ != 0 && gs.items[in_].temp == skill_temp {
            return true;
        }
    }
    false
}

/// Removes every active spell slot whose item `temp` matches `skill_temp`.
///
/// Unlike [`remove_spells`], this only clears the matching buff(s) and leaves
/// the caster's other active spells intact. Used to support toggling a
/// self-buff off by recasting it.
///
/// # Arguments
///
/// * `gs` - Game state.
/// * `cn` - Caster character index.
/// * `skill_temp` - The `item.temp` marker to remove.
///
/// # Returns
///
/// * `true` if at least one matching spell slot was removed.
pub(crate) fn remove_active_spell_temp(gs: &mut GameState, cn: usize, skill_temp: u16) -> bool {
    let mut removed = false;
    for n in 0..20 {
        let in_ = gs.characters[cn].spell[n] as usize;
        if in_ != 0 && gs.items[in_].temp == skill_temp {
            gs.items[in_].used = core::constants::USE_EMPTY;
            gs.characters[cn].spell[n] = 0;
            removed = true;
        }
    }
    if removed {
        gs.do_update_char(cn);
    }
    removed
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::driver::skill::tests::attach_marker;
    use crate::test_helpers::{add_test_player, with_test_gs};
    use core::skills::{SK_REVENANT_CONDUIT2, SK_SPECTRAL_PACT2};

    #[test]
    fn has_active_spell_temp_detects_present_marker() {
        with_test_gs(|gs| {
            let (cn, _nr) = add_test_player(gs);
            assert!(!has_active_spell_temp(gs, cn, SK_REVENANT_CONDUIT2 as u16));
            attach_marker(gs, cn, 0, 10, SK_REVENANT_CONDUIT2 as u16);
            assert!(has_active_spell_temp(gs, cn, SK_REVENANT_CONDUIT2 as u16));
            assert!(!has_active_spell_temp(gs, cn, SK_SPECTRAL_PACT2 as u16));
        });
    }

    #[test]
    fn effective_resist_scales_down_with_penetration_percent() {
        with_test_gs(|gs| {
            let (cn, _nr) = add_test_player(gs);
            assert_eq!(effective_resist(gs, cn, 100), 100);

            gs.talent_runtime[cn].spell_penetration_percent = 25;
            assert_eq!(effective_resist(gs, cn, 100), 80);
        });
    }
}
