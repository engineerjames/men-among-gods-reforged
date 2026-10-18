use super::common::{
    add_exhaust, add_spell, chance_base, effective_resist, is_exhausted, spellcost,
};
use crate::chlog;
use crate::driver;
use crate::effect::EffectManager;
use crate::game_state::GameState;
use crate::god::God;
use crate::helpers;
use core::constants::{ItemFlags, LEGACY_TICKS, TICKS};
use core::skills::{SK_IDENT, SK_RECALL, SK_REPAIR, SK_RESIST, attribute_name, get_skill_name};
use core::string_operations::c_string_to_str;
use core::types::FontColor;

/// Writes detailed item statistics to a character's log.
///
/// # Arguments
///
/// * `gs` - Active game state containing the target item and output character.
/// * `cn` - Character index receiving the item information.
/// * `in_` - Item index to inspect.
/// * `_look` - Legacy look-mode argument retained for signature compatibility.
///
/// # Panics
///
/// * Panics if `cn` or `in_` is not a valid index.
pub fn item_info(gs: &mut GameState, cn: usize, in_: usize, _look: i32) {
    // Name
    let name = gs.items[in_].name;
    gs.do_character_log(
        cn,
        FontColor::Green,
        &format!("{}:\n", c_string_to_str(&name)),
    );

    gs.do_character_log(cn, FontColor::Green, "Stat         Mod0 Mod1 Min\n");

    // Attributes
    for n in 0..5 {
        let (a0, a1, a2) = (
            gs.items[in_].attrib[n][0],
            gs.items[in_].attrib[n][1],
            gs.items[in_].attrib[n][2],
        );
        if a0 == 0 && a1 == 0 && a2 == 0 {
            continue;
        }
        gs.do_character_log(
            cn,
            FontColor::Green,
            &format!(
                "{:<12.12} {:+4} {:+4} {:3}\n",
                attribute_name(n),
                a0,
                a1,
                a2
            ),
        );
    }

    // HP/End/Mana
    let (hp0, hp1, hp2) = (
        gs.items[in_].hp[0],
        gs.items[in_].hp[1],
        gs.items[in_].hp[2],
    );
    if hp0 != 0 || hp1 != 0 || hp2 != 0 {
        gs.do_character_log(
            cn,
            FontColor::Green,
            &format!("{:<12.12} {:+4} {:+4} {:3}\n", "Hitpoints", hp0, hp1, hp2),
        );
    }
    let (end0, end1, end2) = (
        gs.items[in_].end[0],
        gs.items[in_].end[1],
        gs.items[in_].end[2],
    );
    if end0 != 0 || end1 != 0 || end2 != 0 {
        gs.do_character_log(
            cn,
            FontColor::Green,
            &format!(
                "{:<12.12} {:+4} {:+4} {:3}\n",
                "Endurance", end0, end1, end2
            ),
        );
    }
    let (mana0, mana1, mana2) = (
        gs.items[in_].mana[0],
        gs.items[in_].mana[1],
        gs.items[in_].mana[2],
    );
    if mana0 != 0 || mana1 != 0 || mana2 != 0 {
        gs.do_character_log(
            cn,
            FontColor::Green,
            &format!("{:<12.12} {:+4} {:+4} {:3}\n", "Mana", mana0, mana1, mana2),
        );
    }

    for n in 0..core::skills::MAX_SKILLS {
        let (s0, s1, s2) = (
            gs.items[in_].skill[n][0],
            gs.items[in_].skill[n][1],
            gs.items[in_].skill[n][2],
        );
        if s0 == 0 && s1 == 0 && s2 == 0 {
            continue;
        }
        let skill_label = get_skill_name(n);
        gs.do_character_log(
            cn,
            FontColor::Green,
            &format!("{:<12.12} {:+4} {:+4} {:3}\n", skill_label, s0, s1, s2),
        );
    }

    let (w0, w1) = (gs.items[in_].weapon[0], gs.items[in_].weapon[1]);
    if w0 != 0 || w1 != 0 {
        gs.do_character_log(
            cn,
            FontColor::Green,
            &format!("{:<12.12} {:+4} {:+4}\n", "Weapon", w0, w1),
        );
    }
    let (ar0, ar1) = (gs.items[in_].armor[0], gs.items[in_].armor[1]);
    if ar0 != 0 || ar1 != 0 {
        gs.do_character_log(
            cn,
            FontColor::Green,
            &format!("{:<12.12} {:+4} {:+4}\n", "Armor", ar0, ar1),
        );
    }
    let (l0, l1) = (gs.items[in_].light[0], gs.items[in_].light[1]);
    if l0 != 0 || l1 != 0 {
        gs.do_character_log(
            cn,
            FontColor::Green,
            &format!("{:<12.12} {:+4} {:+4}\n", "Light", l0, l1),
        );
    }

    let power = gs.items[in_].power;
    if power != 0 {
        gs.do_character_log(
            cn,
            FontColor::Green,
            &format!("{:<12.12} {:+4}\n", "Power", power),
        );
    }

    let min_rank = gs.items[in_].min_rank;
    if min_rank != 0 {
        gs.do_character_log(
            cn,
            FontColor::Green,
            &format!("{:<12.12} {:+4}\n", "Min. Rank", min_rank),
        );
    }
}

/// Writes detailed character spell, skill, and attribute information to a character's log.
///
/// # Arguments
///
/// * `gs` - Active game state containing both characters and active spell items.
/// * `cn` - Character index receiving the information.
/// * `co` - Character index being inspected.
///
/// # Panics
///
/// * Panics if `cn`, `co`, or an active spell item index is invalid.
pub fn char_info(gs: &mut GameState, cn: usize, co: usize) {
    // Header
    let name_bytes = gs.characters[co].name;
    gs.do_character_log(
        cn,
        FontColor::Green,
        &format!("{}:\n", c_string_to_str(&name_bytes)),
    );
    gs.do_character_log(cn, FontColor::Green, " \n");

    // Active spells (0..19)
    let mut flag = false;
    for n in 0..20 {
        let in_idx = gs.characters[co].spell[n] as usize;
        if in_idx != 0 {
            let item_name = gs.items[in_idx].get_name().to_owned();
            let active = gs.items[in_idx].active;
            let minutes = active / (TICKS as u32 * 60);
            let seconds = (active / TICKS as u32) % 60;
            let power = gs.items[in_idx].power;
            gs.do_character_log(
                cn,
                FontColor::Green,
                &format!(
                    "{} for {}m {}s power of {}\n",
                    item_name, minutes, seconds, power
                ),
            );
            flag = true;
        }
    }
    if !flag {
        gs.do_character_log(cn, FontColor::Green, "No spells active.\n");
    }
    gs.do_character_log(cn, FontColor::Green, " \n");

    // Skills two-column using static SKILL_NAMES
    let mut n1: i32 = -1;
    let mut n2: i32 = -1;
    for n in 0..core::skills::MAX_SKILLS {
        let s0 = gs.characters[co].skill[n][0];
        if s0 != 0 && n1 == -1 {
            n1 = n as i32;
        } else if s0 != 0 && n2 == -1 {
            n2 = n as i32;
        }

        if n1 != -1 && n2 != -1 {
            let s1_0 = gs.characters[co].skill[n1 as usize][0];
            let s1_5 = gs.characters[co].skill[n1 as usize][5];
            let s2_0 = gs.characters[co].skill[n2 as usize][0];
            let s2_5 = gs.characters[co].skill[n2 as usize][5];
            let name1 = get_skill_name(n1 as usize);
            let name2 = get_skill_name(n2 as usize);
            gs.do_character_log(
                cn,
                FontColor::Green,
                &format!(
                    "{:<12.12} {:3}/{:3}  !  {:<12.12} {:3}/{:3}\n",
                    name1, s1_0, s1_5, name2, s2_0, s2_5
                ),
            );
            n1 = -1;
            n2 = -1;
        }
    }

    if n1 != -1 {
        let s1_0 = gs.characters[co].skill[n1 as usize][0];
        let s1_5 = gs.characters[co].skill[n1 as usize][5];
        let name1 = get_skill_name(n1 as usize);
        gs.do_character_log(
            cn,
            FontColor::Green,
            &format!("{:<12.12} {:3}/{:3}\n", name1, s1_0, s1_5),
        );
    }

    // Attributes
    let a0_0 = gs.characters[co].attrib[0][0];
    let a0_5 = gs.characters[co].attrib[0][5];
    let a1_0 = gs.characters[co].attrib[1][0];
    let a1_5 = gs.characters[co].attrib[1][5];
    gs.do_character_log(
        cn,
        FontColor::Green,
        &format!(
            "{:<12.12} {:3}/{:3}  !  {:<12.12} {:3}/{:3}\n",
            attribute_name(0),
            a0_0,
            a0_5,
            attribute_name(1),
            a1_0,
            a1_5
        ),
    );
    let a2_0 = gs.characters[co].attrib[2][0];
    let a2_5 = gs.characters[co].attrib[2][5];
    let a3_0 = gs.characters[co].attrib[3][0];
    let a3_5 = gs.characters[co].attrib[3][5];
    gs.do_character_log(
        cn,
        FontColor::Green,
        &format!(
            "{:<12.12} {:3}/{:3}  !  {:<12.12} {:3}/{:3}\n",
            attribute_name(2),
            a2_0,
            a2_5,
            attribute_name(3),
            a3_0,
            a3_5
        ),
    );
    let a4_0 = gs.characters[co].attrib[4][0];
    let a4_5 = gs.characters[co].attrib[4][5];
    gs.do_character_log(
        cn,
        FontColor::Green,
        &format!("{:<12.12} {:3}/{:3}\n", attribute_name(4), a4_0, a4_5),
    );

    gs.do_character_log(cn, FontColor::Green, " \n");
}

/// Handles direct player/NPC use of the Identify skill.
///
/// Identifies or hides identify data for a carried item, or reports character
/// information when no valid carried item is selected.
///
/// # Arguments
///
/// * `gs` - Active game state used for costs, focus checks, item flags, and feedback.
/// * `cn` - Caster character index.
///
/// # Panics
///
/// * Panics if `cn`, the carried item index, or the selected character target is invalid.
pub fn skill_identify(gs: &mut GameState, cn: usize) {
    if is_exhausted(gs, cn) {
        return;
    }

    if spellcost(gs, cn, 25) != 0 {
        return;
    }

    let citem = gs.characters[cn].citem as usize;
    let in_idx: usize;
    let mut co = 0usize;
    let power: i32;

    let sane_item = if citem != 0 {
        citem < gs.items.len() && gs.items[citem].used != core::constants::USE_EMPTY
    } else {
        false
    };

    if citem != 0 && sane_item {
        in_idx = citem;
        power = gs.items[in_idx].power as i32;
    } else {
        let target = gs.characters[cn].skill_target1 as usize;
        if target != 0 {
            co = target;
            power = effective_resist(gs, cn, i32::from(gs.characters[co].skill[SK_RESIST][5]));
        } else {
            co = cn;
            power = 10;
        }
        in_idx = 0;
    }

    if chance_base(
        gs,
        cn,
        i32::from(gs.characters[cn].skill[SK_IDENT][5]),
        18,
        power,
    ) != 0
    {
        return;
    }

    let sound = gs.characters[cn].sound;
    GameState::char_play_sound(gs, cn, i32::from(sound) + 1, -150, 0);
    chlog!(
        cn,
        "Cast Identify on {}",
        if in_idx != 0 {
            gs.items[in_idx].get_name().to_owned()
        } else {
            gs.characters[co].get_name().to_owned()
        }
    );

    if in_idx != 0 {
        item_info(gs, cn, in_idx, 0);
        gs.items[in_idx].flags ^= ItemFlags::IF_IDENTIFIED.bits();
        let identified = (gs.items[in_idx].flags & ItemFlags::IF_IDENTIFIED.bits()) != 0;
        if !identified {
            gs.do_character_log(
                cn,
                core::types::FontColor::Green,
                "Identify data removed from item.\n",
            );
        }
    } else {
        char_info(gs, cn, co);
        EffectManager::fx_add_effect(
            gs,
            6,
            0,
            i32::from(gs.characters[co].x),
            i32::from(gs.characters[co].y),
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

/// Attempts to repair the item currently carried under a character's cursor.
///
/// # Arguments
///
/// * `gs` - Active game state used for item validation, endurance costs, and replacement item creation.
/// * `cn` - Character index attempting the repair.
///
/// # Panics
///
/// * Panics if `cn`, the carried item index, or the created replacement item index is invalid.
pub fn skill_repair(gs: &mut GameState, cn: usize) {
    let in_idx = gs.characters[cn].citem as usize;
    if in_idx == 0 {
        gs.do_character_log(cn, core::types::FontColor::Green, "Repair. Repair what?\n");
        return;
    }

    if gs.items[in_idx].damage_state == 0 {
        gs.do_character_log(cn, core::types::FontColor::Green, "That isn't damaged.\n");
        return;
    }

    if gs.items[in_idx].power as i32 > i32::from(gs.characters[cn].skill[SK_REPAIR][5])
        || (gs.items[in_idx].flags & ItemFlags::IF_NOREPAIR.bits()) != 0
    {
        gs.do_character_log(
            cn,
            core::types::FontColor::Green,
            "That's too difficult for you.\n",
        );
        return;
    }

    if gs.characters[cn].a_end < gs.items[in_idx].power as i32 * 1000 {
        gs.do_character_log(
            cn,
            core::types::FontColor::Green,
            "You're too exhausted to repair that.\n",
        );
        return;
    }

    let cost = gs.items[in_idx].power as i32;
    gs.characters[cn].a_end -= cost * 1000;

    let mut chan: i32 = if gs.items[in_idx].power != 0 {
        let skill = i32::from(gs.characters[cn].skill[SK_REPAIR][5]);
        let power = gs.items[in_idx].power as i32;
        skill * 15 / power
    } else {
        18
    };

    if chan > 18 {
        chan = 18;
    }

    let die = helpers::random_mod_i32(20);

    if die <= chan {
        let in2_opt = God::create_item(gs, gs.items[in_idx].temp as usize);
        if in2_opt.is_none() {
            gs.do_character_log(cn, core::types::FontColor::Green, "You failed.\n");
            return;
        }
        let in2 = in2_opt.unwrap();
        gs.items[in_idx].used = core::constants::USE_EMPTY;
        gs.characters[cn].citem = in2 as u32;
        gs.items[in2].carried = cn as u16;
        gs.do_character_log(cn, core::types::FontColor::Green, "Success!\n");
    } else {
        gs.do_character_log(cn, core::types::FontColor::Green, "You failed.\n");
        driver::item_damage_citem(gs, cn, 1000000);
        if die - chan > 3 {
            driver::item_damage_citem(gs, cn, 1000000);
        }
        if die - chan > 6 {
            driver::item_damage_citem(gs, cn, 1000000);
        }
    }
    chlog!(
        cn,
        "Cast Repair on {}",
        gs.items[in_idx].get_name().to_owned()
    );
}

/// Handles direct player/NPC use of the Recall skill.
///
/// Creates a temporary Recall spell tied to the caster's temple coordinates.
///
/// # Arguments
///
/// * `gs` - Active game state used for costs, item creation, and spell attachment.
/// * `cn` - Caster character index.
///
/// # Panics
///
/// * Panics if `cn` or the created spell item index is invalid.
pub fn skill_recall(gs: &mut GameState, cn: usize) {
    if is_exhausted(gs, cn) {
        return;
    }

    if spellcost(gs, cn, 15) != 0 {
        return;
    }

    let in_opt = God::create_item(gs, 1);
    if in_opt.is_none() {
        gs.do_character_log(cn, core::types::FontColor::Green, "You failed.\n");
        return;
    }
    let in_idx = in_opt.unwrap();

    {
        let mut name_bytes = [0u8; 40];
        let name = b"Recall";
        let len = name.len().min(40);
        name_bytes[..len].copy_from_slice(&name[..len]);
        gs.items[in_idx].name = name_bytes;
        gs.items[in_idx].flags |= ItemFlags::IF_SPELL.bits();
        gs.items[in_idx].sprite[1] = 90;
        let base_dur = 60 - i32::from(gs.characters[cn].skill[SK_RECALL][5] / 4);
        let dur = std::cmp::max(TICKS / 2, base_dur * TICKS / LEGACY_TICKS);
        gs.items[in_idx].duration = dur as u32;
        gs.items[in_idx].active = gs.items[in_idx].duration;
        gs.items[in_idx].temp = SK_RECALL as u16;
        gs.items[in_idx].power = u32::from(gs.characters[cn].skill[SK_RECALL][5]);
        gs.items[in_idx].data[0] = u32::from(gs.characters[cn].temple_x);
        gs.items[in_idx].data[1] = u32::from(gs.characters[cn].temple_y);
    }

    if add_spell(gs, cn, in_idx) == 0 {
        gs.do_character_log(cn, core::types::FontColor::Green, "You failed.\n");
        return;
    }

    chlog!(cn, "Cast Recall");
    add_exhaust(gs, cn, TICKS);
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
    use crate::driver::skill::tests::{active_spell_item, caster_and_target};
    use crate::test_helpers::with_test_gs;
    use core::constants::{USE_ACTIVE, USE_EMPTY};

    /// Put a damaged item of the given power in the caster's hand at index 500.
    fn damaged_citem(gs: &mut GameState, cn: usize, power: u32) {
        gs.items[500] = core::types::Item::default();
        gs.items[500].used = USE_ACTIVE;
        gs.items[500].temp = 1;
        gs.items[500].power = power;
        gs.items[500].damage_state = 2;
        gs.items[500].carried = cn as u16;
        gs.characters[cn].citem = 500;
    }

    #[test]
    fn skill_repair_rejects_items_it_cannot_work_on() {
        with_test_gs(|gs| {
            let (cn, _co) = caster_and_target(gs);
            gs.characters[cn].skill[SK_REPAIR][5] = 50;
            gs.characters[cn].a_end = 100_000;

            gs.characters[cn].citem = 0;
            skill_repair(gs, cn);

            damaged_citem(gs, cn, 10);
            gs.items[500].damage_state = 0;
            skill_repair(gs, cn);

            damaged_citem(gs, cn, 60);
            skill_repair(gs, cn);

            damaged_citem(gs, cn, 10);
            gs.items[500].flags |= ItemFlags::IF_NOREPAIR.bits();
            skill_repair(gs, cn);

            damaged_citem(gs, cn, 10);
            gs.characters[cn].a_end = 9_000;
            skill_repair(gs, cn);

            assert_eq!(gs.characters[cn].a_end, 9_000);
            assert_eq!(gs.characters[cn].citem, 500);
            assert_eq!(gs.items[500].used, USE_ACTIVE);
        });
    }

    #[test]
    fn skill_repair_replaces_the_item_with_a_fresh_copy() {
        with_test_gs(|gs| {
            let (cn, _co) = caster_and_target(gs);
            gs.characters[cn].skill[SK_REPAIR][5] = 100;

            // Success is 95% per attempt; retry so the test is not flaky.
            for _ in 0..20 {
                damaged_citem(gs, cn, 1);
                gs.characters[cn].a_end = 100_000;
                skill_repair(gs, cn);
                assert_eq!(gs.characters[cn].a_end, 99_000);
                if gs.characters[cn].citem != 500 {
                    break;
                }
            }

            let repaired = gs.characters[cn].citem as usize;
            assert_ne!(repaired, 500);
            assert_eq!(gs.items[500].used, USE_EMPTY);
            assert_eq!(gs.items[repaired].carried, cn as u16);
            assert_eq!(gs.items[repaired].damage_state, 0);
        });
    }

    #[test]
    fn skill_recall_stores_the_temple_and_exhausts() {
        with_test_gs(|gs| {
            let (cn, _co) = caster_and_target(gs);
            gs.characters[cn].temple_x = 512;
            gs.characters[cn].temple_y = 256;
            gs.characters[cn].skill[SK_RECALL][5] = 40;
            gs.characters[cn].a_mana = 20_000;

            skill_recall(gs, cn);

            let in_ = active_spell_item(gs, cn, SK_RECALL).expect("recall");
            assert_eq!(gs.items[in_].data[0], 512);
            assert_eq!(gs.items[in_].data[1], 256);
            assert_eq!(gs.items[in_].power, 40);
            // (60 - 40/4) legacy ticks, converted to server ticks.
            assert_eq!(gs.items[in_].active, (50 * TICKS / LEGACY_TICKS) as u32);
            assert_eq!(gs.characters[cn].a_mana, 5_000);
            assert!(is_exhausted(gs, cn));
        });
    }

    #[test]
    fn skill_recall_delay_never_drops_below_half_a_second() {
        with_test_gs(|gs| {
            let (cn, _co) = caster_and_target(gs);
            gs.characters[cn].skill[SK_RECALL][5] = 255;
            gs.characters[cn].a_mana = 20_000;

            skill_recall(gs, cn);

            let in_ = active_spell_item(gs, cn, SK_RECALL).expect("recall");
            assert_eq!(gs.items[in_].active, (TICKS / 2) as u32);
        });
    }

    #[test]
    fn skill_recall_and_identify_are_refused_while_exhausted() {
        with_test_gs(|gs| {
            let (cn, _co) = caster_and_target(gs);
            gs.characters[cn].a_mana = 100_000;
            add_exhaust(gs, cn, 10);

            skill_recall(gs, cn);
            skill_identify(gs, cn);

            assert_eq!(gs.characters[cn].a_mana, 100_000);
            assert!(active_spell_item(gs, cn, SK_RECALL).is_none());
        });
    }

    #[test]
    fn skill_identify_toggles_identified_flag_on_held_item() {
        with_test_gs(|gs| {
            let (cn, _co) = caster_and_target(gs);
            gs.characters[cn].skill[SK_IDENT][5] = 100;
            damaged_citem(gs, cn, 1);

            // Success is 95% per attempt; retry so the test is not flaky.
            for _ in 0..20 {
                gs.characters[cn].a_mana = 100_000;
                skill_identify(gs, cn);
                assert_eq!(gs.characters[cn].a_mana, 75_000);
                if gs.items[500].flags & ItemFlags::IF_IDENTIFIED.bits() != 0 {
                    break;
                }
            }

            assert_ne!(gs.items[500].flags & ItemFlags::IF_IDENTIFIED.bits(), 0);
            assert!(is_exhausted(gs, cn));
        });
    }
}
