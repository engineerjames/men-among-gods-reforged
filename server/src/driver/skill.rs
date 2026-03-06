use core::{
    constants::{
        CharacterFlags, ItemFlags, AT_AGIL, AT_STREN, CHD_COMPANION, CHD_TALKATIVE, CNTSAY,
        COMPANION_TIMEOUT, CT_COMPANION, DX_DOWN, DX_LEFT, DX_RIGHT, DX_UP, KIN_MONSTER, MAXSAY,
        NT_DIDHIT, NT_GOTHIT, NT_GOTMISS, SERVER_MAPX, SK_AXE, SK_BLAST, SK_BLESS, SK_CONCEN,
        SK_CURSE, SK_DAGGER, SK_DISPEL, SK_ENHANCE, SK_GHOST, SK_HEAL, SK_IDENT, SK_IMMUN,
        SK_LIGHT, SK_LOCK, SK_MEDIT, SK_MSHIELD, SK_PROTECT, SK_RECALL, SK_REGEN, SK_REPAIR,
        SK_RESIST, SK_REST, SK_SENSE, SK_STAFF, SK_STUN, SK_SURROUND, SK_SWORD, SK_TWOHAND,
        SK_WARCRY, SK_WIMPY, TICKS, USE_EMPTY,
    },
    string_operations::c_string_to_str,
    types::FontColor,
};

use crate::{
    chlog, core::types::Character, driver, effect::EffectManager, game_state::GameState, god::God,
    helpers, populate,
};

use core::constants::LEGACY_TICKS;

// Static skill table (taken from server/original_source/SkillTab.cpp)
const SKILL_NAMES: [&str; 50] = [
    "Hand to Hand",
    "Karate",
    "Dagger",
    "Sword",
    "Axe",
    "Staff",
    "Two-Handed",
    "Lock-Picking",
    "Stealth",
    "Perception",
    "Swimming",
    "Magic Shield",
    "Bartering",
    "Repair",
    "Light",
    "Recall",
    "Guardian Angel",
    "Protection",
    "Enhance Weapon",
    "Stun",
    "Curse",
    "Bless",
    "Identify",
    "Resistance",
    "Blast",
    "Dispel Magic",
    "Heal",
    "Ghost Companion",
    "Regenerate",
    "Rest",
    "Meditate",
    "Sense Magic",
    "Immunity",
    "Surround Hit",
    "Concentrate",
    "Warcry",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
];

/// Returns the skill name for a given index, or an empty string if out of bounds.
///
/// # Arguments
///
/// * `n` - Index of the skill
///
/// # Returns
///
/// The skill name as a string slice, or an empty string if out of bounds.
pub fn skill_name(n: usize) -> &'static str {
    if n < SKILL_NAMES.len() {
        SKILL_NAMES[n]
    } else {
        ""
    }
}

pub fn player_or_ghost(gs: &GameState, cn: usize, co: usize) -> i32 {
    // Rust port of C++ player_or_ghost
    let cn_flags = gs.characters[cn].flags;
    if (cn_flags & CharacterFlags::Player.bits()) == 0 {
        return 0;
    }
    let co_flags = gs.characters[co].flags;
    if (co_flags & CharacterFlags::Player.bits()) != 0 {
        return 1;
    }
    let co_data_63 = gs.characters[co].data[63] as usize;
    if co_data_63 == cn {
        return 1;
    }
    0
}
pub fn spellcost(gs: &mut GameState, cn: usize, cost: i32) -> i32 {
    // Ported from C++ spellcost(int cn, int cost)
    // concentrate:
    let mut cost = cost;
    let concen_skill = gs.characters[cn].skill[core::constants::SK_CONCEN][0];
    if concen_skill != 0 {
        let concen_val = gs.characters[cn].skill[core::constants::SK_CONCEN][5];
        let t = cost * concen_val as i32 / 300;
        if t > cost {
            cost = 1;
        } else {
            cost -= t;
        }
    }
    let a_mana = gs.characters[cn].a_mana;
    if cost * 1000 > a_mana {
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

pub fn chance_base(gs: &mut GameState, cn: usize, skill: i32, d20: i32, power: i32) -> i32 {
    // Ported from C++ chance_base(int cn, int skill, int d20, int power)
    let mut chance = d20 * skill / std::cmp::max(1, power);
    let (flags, luck) = (gs.characters[cn].flags, gs.characters[cn].luck);
    if (flags & CharacterFlags::Player.bits()) != 0 {
        if luck < 0 {
            chance += luck / 500 - 1;
        }
    }

    chance = chance.clamp(0, 18);

    let roll = crate::helpers::random_mod(20);
    if roll as i32 > chance || power > skill + (skill / 2) {
        gs.do_character_log(cn, core::types::FontColor::Red, "You lost your focus!\n");
        return -1;
    }
    0
}
pub fn chance(gs: &mut GameState, cn: usize, d20: i32) -> i32 {
    // Ported from C++ chance(int cn, int d20)
    let mut d20 = d20;
    let (flags, luck) = (gs.characters[cn].flags, gs.characters[cn].luck);
    if (flags & CharacterFlags::Player.bits()) != 0 {
        if luck < 0 {
            d20 += luck / 500 - 1;
        }
    }

    d20 = d20.clamp(0, 18);

    let roll = crate::helpers::random_mod(20);
    if roll as i32 > d20 {
        gs.do_character_log(cn, core::types::FontColor::Red, "You lost your focus!\n");
        return -1;
    }
    0
}
pub fn spell_immunity(_gs: &GameState, power: i32, immun: i32) -> i32 {
    // Ported from C++ spell_immunity(int power, int immun)
    let immun = immun / 2;
    if power <= immun {
        1
    } else {
        power - immun
    }
}
pub fn spell_race_mod(gs: &GameState, power: i32, kindred: i32) -> i32 {
    // Ported from C++ spell_race_mod(int power, int kindred)

    let mut modf;
    if (kindred & core::constants::KIN_ARCHHARAKIM as i32) != 0 {
        modf = 1.05;
    } else if (kindred & core::constants::KIN_ARCHTEMPLAR as i32) != 0 {
        modf = 0.95;
    } else if (kindred & core::constants::KIN_SORCERER as i32) != 0 {
        modf = 1.10;
    } else if (kindred & core::constants::KIN_WARRIOR as i32) != 0 {
        modf = 1.10;
    } else if (kindred & core::constants::KIN_SEYAN_DU as i32) != 0 {
        modf = 0.95;
    } else if (kindred & core::constants::KIN_HARAKIM as i32) != 0 {
        modf = 1.00;
    } else if (kindred & core::constants::KIN_MERCENARY as i32) != 0 {
        modf = 1.05;
    } else if (kindred & core::constants::KIN_TEMPLAR as i32) != 0 {
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

    (power as f64 * modf) as i32
}

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

pub fn is_exhausted(gs: &mut GameState, cn: usize) -> i32 {
    // Ported from C++ is_exhausted(int cn)
    for n in 0..20 {
        let in_ = gs.characters[cn].spell[n] as usize;
        if in_ != 0 {
            let temp = gs.items[in_].temp;
            if temp == core::constants::SK_BLAST as u16 {
                gs.do_character_log(
                    cn,
                    core::types::FontColor::Red,
                    "You are still exhausted from your last spell!\n",
                );
                return 1;
            }
        }
    }
    0
}

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
        for n in 0..50 {
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
        let name = gs.items[in_].get_name().to_string();
        gs.do_character_log(
            cn,
            core::types::FontColor::Green,
            &format!("Magical interference neutralised the {}'s effect.\n", name,),
        );
        return;
    }
    gs.do_character_log(cn, core::types::FontColor::Green, "You feel changed.\n");
    let sound = gs.characters[cn].sound;
    GameState::char_play_sound(gs, cn, sound as i32 + 1, -150, 0);
}

pub fn spell_light(gs: &mut GameState, cn: usize, co: usize, power: i32) -> i32 {
    // Ported from C++ spell_light(int cn, int co, int power)
    let in_ = God::create_item(gs, 1);
    if in_.is_none() {
        log::error!("god_create_item failed in spell_light");
        return 0;
    }
    let power = spell_race_mod(gs, power, gs.characters[cn].kindred);
    {
        let in_idx = in_.unwrap();
        let mut name_bytes = [0u8; 40];
        let name = b"Light";
        let len = name.len().min(40);
        name_bytes[..len].copy_from_slice(&name[..len]);
        gs.items[in_idx].name = name_bytes;
        gs.items[in_idx].flags |= ItemFlags::IF_SPELL.bits();
        gs.items[in_idx].light[1] = std::cmp::min(250, power * 4) as i16;
        gs.items[in_idx].sprite[1] = 85;
        gs.items[in_idx].duration = (TICKS * 60 * 30) as u32;
        gs.items[in_idx].active = (TICKS * 60 * 30) as u32;
        gs.items[in_idx].temp = core::constants::SK_LIGHT as u16;
        gs.items[in_idx].power = power as u32;
    }
    if cn != co {
        if add_spell(gs, co, in_.unwrap()) == 0 {
            let name = gs.items[in_.unwrap()].get_name().to_string();
            gs.do_character_log(
                cn,
                core::types::FontColor::Green,
                &format!("Magical interference neutralised the {}'s effect.\n", name),
            );
            return 0;
        }
        let sense = gs.characters[co].skill[core::constants::SK_SENSE][5];
        if sense + 10 > power as u8 {
            let reference = gs.characters[cn].reference;
            gs.do_character_log(
                co,
                core::types::FontColor::Green,
                &format!("{} cast light on you.\n", c_string_to_str(&reference)),
            );
        } else {
            gs.do_character_log(
                co,
                core::types::FontColor::Green,
                "You start to emit light.\n",
            );
        }
        let name = gs.characters[co].name;
        let (x, y) = (gs.characters[co].x, gs.characters[co].y);
        gs.do_area_log(
            co,
            0,
            x as i32,
            y as i32,
            core::types::FontColor::Green,
            &format!("{} starts to emit light.\n", c_string_to_str(&name)),
        );
        let sound = gs.characters[cn].sound;
        GameState::char_play_sound(gs, co, sound as i32 + 1, -150, 0);
        GameState::char_play_sound(gs, cn, sound as i32 + 1, -150, 0);
        let (x, y) = (gs.characters[co].x, gs.characters[co].y);
        EffectManager::fx_add_effect(gs, 7, 0, x as i32, y as i32, 0);
    } else {
        if add_spell(gs, cn, in_.unwrap()) == 0 {
            let name = gs.items[in_.unwrap()].get_name().to_string();
            gs.do_character_log(
                cn,
                core::types::FontColor::Green,
                &format!("Magical interference neutralised the {}'s effect.\n", name),
            );
            return 0;
        }
        gs.do_character_log(
            cn,
            core::types::FontColor::Green,
            "You start to emit light.\n",
        );
        let sound = gs.characters[cn].sound;
        GameState::char_play_sound(gs, cn, sound as i32 + 1, -150, 0);
        let flags = gs.characters[cn].flags;
        if (flags & CharacterFlags::Player.bits()) != 0 {
            chlog!(cn, "Cast Light");
        }
        let (x, y) = (gs.characters[cn].x, gs.characters[cn].y);
        EffectManager::fx_add_effect(gs, 7, 0, x as i32, y as i32, 0);
    }
    let (x, y) = (gs.characters[cn].x, gs.characters[cn].y);
    EffectManager::fx_add_effect(gs, 7, 0, x as i32, y as i32, 0);
    1
}

pub fn skill_light(gs: &mut GameState, cn: usize) {
    // rate limit for player
    let is_player = (gs.characters[cn].flags & CharacterFlags::Player.bits()) != 0;
    if is_player {
        gs.characters[cn].data[71] += CNTSAY;
        let over = gs.characters[cn].data[71] > MAXSAY;
        if over {
            gs.do_character_log(cn, FontColor::Red, "Oops, you're a bit too fast for me!\n");
            return;
        }
    }

    let co = if gs.characters[cn].skill_target1 != 0 {
        gs.characters[cn].skill_target1 as usize
    } else {
        cn
    };

    if gs.do_char_can_see(cn, co) == 0 {
        gs.do_character_log(cn, FontColor::Red, "You cannot see your target.\n");
        return;
    }

    if is_exhausted(gs, cn) != 0 {
        return;
    }

    if spellcost(gs, cn, 5) != 0 {
        return;
    }

    if chance(gs, cn, 18) != 0 {
        if cn != co {
            let sense = gs.characters[co].skill[SK_SENSE][5];
            let light_skill = gs.characters[cn].skill[SK_LIGHT][5];
            if sense > (light_skill + 5) {
                let reference = gs.characters[cn].reference;
                gs.do_character_log(
                    co,
                    FontColor::Green,
                    &format!(
                        "{} tried to cast light on you but failed.\n",
                        c_string_to_str(&reference)
                    ),
                );
            }
        }
        return;
    }

    let light_skill = gs.characters[cn].skill[SK_LIGHT][5];
    spell_light(gs, cn, co, light_skill as i32);

    add_exhaust(gs, cn, TICKS / 4);
}

pub fn spellpower(gs: &GameState, cn: usize) -> i32 {
    let a = gs.characters[cn].attrib[core::constants::AT_AGIL as usize][0] as i32;
    let b = gs.characters[cn].attrib[core::constants::AT_STREN as usize][0] as i32;
    let c = gs.characters[cn].attrib[core::constants::AT_INT as usize][0] as i32;
    let d = gs.characters[cn].attrib[core::constants::AT_WILL as usize][0] as i32;
    let e = gs.characters[cn].attrib[core::constants::AT_BRAVE as usize][0] as i32;
    a + b + c + d + e
}

pub fn spell_protect(gs: &mut GameState, cn: usize, co: usize, power: i32) -> i32 {
    let in_opt = God::create_item(gs, 1);
    if in_opt.is_none() {
        log::error!("god_create_item failed in skill_protect");
        return 0;
    }
    let in_ = in_opt.unwrap();

    // cap power to target's spellpower
    let mut power = power;
    let target_spellpower = spellpower(gs, co);
    if power > target_spellpower {
        if cn != co {
            let reference = gs.characters[co].reference;
            gs.do_character_log(
                cn,
                FontColor::Yellow,
                &format!(
                    "Seeing that {} is not powerful enough for your spell, you reduced its strength.\n",
                    c_string_to_str(&reference)
                ),
            );
        } else {
            gs.do_character_log(
                cn,
                FontColor::Green,
                "You are not powerful enough to use the full strength of this spell.\n",
            );
        }
        power = target_spellpower;
    }

    let power = spell_race_mod(gs, power, gs.characters[cn].kindred);

    {
        let mut name_bytes = [0u8; 40];
        let name = b"Protection";
        let len = name.len().min(40);
        name_bytes[..len].copy_from_slice(&name[..len]);
        gs.items[in_].name = name_bytes;
        gs.items[in_].flags |= ItemFlags::IF_SPELL.bits();
        gs.items[in_].armor[1] = (power / 4 + 4) as i8;
        gs.items[in_].sprite[1] = 86;
        gs.items[in_].duration = (TICKS * 60 * 10) as u32;
        gs.items[in_].active = (TICKS * 60 * 10) as u32;
        gs.items[in_].temp = SK_PROTECT as u16;
        gs.items[in_].power = power as u32;
    }

    if cn != co {
        if add_spell(gs, co, in_) == 0 {
            let name = gs.items[in_].get_name().to_string();
            gs.do_character_log(
                cn,
                FontColor::Green,
                &format!("Magical interference neutralised the {}'s effect.\n", name),
            );
            return 0;
        }

        let sense = gs.characters[co].skill[SK_SENSE][5];
        if sense as i32 + 10 > power {
            let reference = gs.characters[cn].reference;
            gs.do_character_log(
                co,
                FontColor::Green,
                &format!("{} cast protect on you.\n", c_string_to_str(&reference)),
            );
        } else {
            gs.do_character_log(co, FontColor::Red, "You feel protected.\n");
        }

        let name = gs.characters[co].get_name().to_string();
        gs.do_character_log(
            cn,
            FontColor::Yellow,
            &format!("{} is now protected.\n", name),
        );
        let sound = gs.characters[cn].sound;
        GameState::char_play_sound(gs, co, sound as i32 + 1, -150, 0);
        GameState::char_play_sound(gs, cn, sound as i32 + 1, -150, 0);
        let target_name = gs.characters[co].get_name().to_string();
        chlog!(cn, "Cast Protect on {}", target_name);
        EffectManager::fx_add_effect(
            gs,
            6,
            0,
            gs.characters[co].x as i32,
            gs.characters[co].y as i32,
            0,
        );
    } else {
        if add_spell(gs, cn, in_) == 0 {
            let name = gs.items[in_].get_name().to_string();
            gs.do_character_log(
                cn,
                FontColor::Green,
                &format!("Magical interference neutralised the {}'s effect.\n", name),
            );
            return 0;
        }
        gs.do_character_log(cn, FontColor::Green, "You feel protected.\n");
        let sound = gs.characters[cn].sound;
        GameState::char_play_sound(gs, cn, sound as i32 + 1, -150, 0);
        let flags = gs.characters[cn].flags;
        if (flags & CharacterFlags::Player.bits()) != 0 {
            chlog!(cn, "Cast Protect");
        }
        let (x, y) = (gs.characters[cn].x, gs.characters[cn].y);
        EffectManager::fx_add_effect(gs, 6, 0, x as i32, y as i32, 0);
    }

    EffectManager::fx_add_effect(
        gs,
        7,
        0,
        gs.characters[cn].x as i32,
        gs.characters[cn].y as i32,
        0,
    );

    1
}

pub fn skill_protect(gs: &mut GameState, cn: usize) {
    let has_skill = gs.characters[cn].skill[SK_PROTECT][5] != 0;
    if !has_skill {
        return;
    }

    let mut co = if gs.characters[cn].skill_target1 != 0 {
        gs.characters[cn].skill_target1 as usize
    } else {
        cn
    };

    if gs.do_char_can_see(cn, co) == 0 {
        gs.do_character_log(cn, FontColor::Red, "You cannot see your target.\n");
        return;
    }

    if is_exhausted(gs, cn) != 0 {
        return;
    }

    if player_or_ghost(gs, cn, co) == 0 {
        let name_from = gs.characters[co].get_name().to_string();
        let name_to = gs.characters[cn].get_name().to_string();
        gs.do_character_log(
            cn,
            FontColor::Red,
            &format!(
                "Changed target of spell from {} to {}.\n",
                name_from, name_to
            ),
        );
        co = cn;
    }

    if spellcost(gs, cn, 15) != 0 {
        return;
    }
    if chance(gs, cn, 18) != 0 {
        if cn != co {
            let sense = gs.characters[co].skill[SK_SENSE][5];
            let prot_skill = gs.characters[cn].skill[SK_PROTECT][5];
            if sense > (prot_skill + 5) {
                let reference = gs.characters[cn].reference;
                gs.do_character_log(
                    co,
                    FontColor::Green,
                    &format!(
                        "{} tried to cast protect on you but failed.\n",
                        c_string_to_str(&reference)
                    ),
                );
            }
        }
        return;
    }

    let power = gs.characters[cn].skill[SK_PROTECT][5] as i32;
    spell_protect(gs, cn, co, power);

    add_exhaust(gs, cn, TICKS / 2);
}

pub fn spell_enhance(gs: &mut GameState, cn: usize, co: usize, power: i32) -> i32 {
    let in_opt = God::create_item(gs, 1);
    if in_opt.is_none() {
        log::error!("god_create_item failed in skill_enhance");
        return 0;
    }
    let in_ = in_opt.unwrap();

    // cap power to target's spellpower
    let mut power = power;
    let target_spellpower = spellpower(gs, co);
    if power > target_spellpower {
        if cn != co {
            let reference = gs.characters[co].reference;
            gs.do_character_log(
                cn,
                FontColor::Yellow,
                &format!(
                    "Seeing that {} is not powerful enough for your spell, you reduced its strength.\n",
                    c_string_to_str(&reference)
                ),
            );
        } else {
            gs.do_character_log(
                cn,
                FontColor::Yellow,
                "You are not powerful enough to use the full strength of this spell.\n",
            );
        }
        power = target_spellpower;
    }

    let power = spell_race_mod(gs, power, gs.characters[cn].kindred);

    {
        let mut name_bytes = [0u8; 40];
        let name = b"Enhance Weapon";
        let len = name.len().min(40);
        name_bytes[..len].copy_from_slice(&name[..len]);
        gs.items[in_].name = name_bytes;
        gs.items[in_].flags |= ItemFlags::IF_SPELL.bits();
        gs.items[in_].weapon[1] = (power / 4 + 4) as i8;
        gs.items[in_].sprite[1] = 87;
        gs.items[in_].duration = (TICKS * 60 * 10) as u32;
        gs.items[in_].active = (TICKS * 60 * 10) as u32;
        gs.items[in_].temp = SK_ENHANCE as u16;
        gs.items[in_].power = power as u32;
    }

    if cn != co {
        if add_spell(gs, co, in_) == 0 {
            let name = gs.items[in_].get_name().to_string();
            gs.do_character_log(
                cn,
                FontColor::Yellow,
                &format!("Magical interference neutralised the {}'s effect.\n", name),
            );
            return 0;
        }
        let sense = gs.characters[co].skill[SK_SENSE][5];
        if sense as i32 + 10 > power {
            let reference = gs.characters[cn].reference;
            gs.do_character_log(
                co,
                FontColor::Yellow,
                &format!(
                    "{} cast enhance weapon on you.\n",
                    c_string_to_str(&reference)
                ),
            );
        } else {
            gs.do_character_log(co, FontColor::Red, "Your weapon feels stronger.\n");
        }
        gs.do_character_log(
            cn,
            FontColor::Yellow,
            &format!(
                "{}'s weapon is now stronger.\n",
                gs.characters[co].get_name().to_string()
            ),
        );
        let sound = gs.characters[cn].sound;
        GameState::char_play_sound(gs, co, sound as i32 + 1, -150, 0);
        GameState::char_play_sound(gs, cn, sound as i32 + 1, -150, 0);
        let target_name = gs.characters[co].get_name().to_string();
        chlog!(cn, "Cast Enhance on {}", target_name);

        EffectManager::fx_add_effect(
            gs,
            6,
            0,
            gs.characters[co].x as i32,
            gs.characters[co].y as i32,
            0,
        );
    } else {
        if add_spell(gs, cn, in_) == 0 {
            let name = gs.items[in_].get_name().to_string();
            gs.do_character_log(
                cn,
                FontColor::Yellow,
                &format!("Magical interference neutralised the {}'s effect.\n", name),
            );
            return 0;
        }
        gs.do_character_log(cn, FontColor::Green, "Your weapon feels stronger.\n");
        let sound = gs.characters[cn].sound;
        GameState::char_play_sound(gs, cn, sound as i32 + 1, -150, 0);
        let flags = gs.characters[cn].flags;
        if (flags & CharacterFlags::Player.bits()) != 0 {
            chlog!(cn, "Cast Enhance");
        }
        EffectManager::fx_add_effect(
            gs,
            6,
            0,
            gs.characters[cn].x as i32,
            gs.characters[cn].y as i32,
            0,
        );
    }

    EffectManager::fx_add_effect(
        gs,
        7,
        0,
        gs.characters[cn].x as i32,
        gs.characters[cn].y as i32,
        0,
    );

    1
}

pub fn skill_enhance(gs: &mut GameState, cn: usize) {
    let co = if gs.characters[cn].skill_target1 != 0 {
        gs.characters[cn].skill_target1 as usize
    } else {
        cn
    };

    if gs.do_char_can_see(cn, co) == 0 {
        gs.do_character_log(cn, FontColor::Red, "You cannot see your target.\n");
        return;
    }

    if is_exhausted(gs, cn) != 0 {
        return;
    }

    if player_or_ghost(gs, cn, co) == 0 {
        let name_from = gs.characters[co].get_name().to_string();
        let name_to = gs.characters[cn].get_name().to_string();
        gs.do_character_log(
            cn,
            FontColor::Red,
            &format!(
                "Changed target of spell from {} to {}.\n",
                name_from, name_to
            ),
        );
        // change target to self
        let co = cn;
        // continue with self
        if spellcost(gs, cn, 15) != 0 {
            return;
        }
        if chance(gs, cn, 18) != 0 {
            if cn != co {
                let sense = gs.characters[co].skill[SK_SENSE][5];
                let enh_skill = gs.characters[cn].skill[SK_ENHANCE][5];
                if sense > (enh_skill + 5) {
                    let reference = gs.characters[cn].reference;
                    gs.do_character_log(
                        co,
                        FontColor::Yellow,
                        &format!(
                            "{} tried to cast enhance weapon on you but failed.\n",
                            c_string_to_str(&reference)
                        ),
                    );
                }
            }
            return;
        }
        let power = gs.characters[cn].skill[SK_ENHANCE][5] as i32;
        spell_enhance(gs, cn, co, power);
        add_exhaust(gs, cn, TICKS / 2);
        return;
    }

    if spellcost(gs, cn, 15) != 0 {
        return;
    }
    if chance(gs, cn, 18) != 0 {
        if cn != co {
            let sense = gs.characters[co].skill[SK_SENSE][5];
            let enh_skill = gs.characters[cn].skill[SK_ENHANCE][5];
            if sense > (enh_skill + 5) {
                let reference = gs.characters[cn].reference;
                gs.do_character_log(
                    co,
                    FontColor::Yellow,
                    &format!(
                        "{} tried to cast enhance weapon on you but failed.\n",
                        c_string_to_str(&reference)
                    ),
                );
            }
        }
        return;
    }

    let power = gs.characters[cn].skill[SK_ENHANCE][5] as i32;
    spell_enhance(gs, cn, co, power);
    add_exhaust(gs, cn, TICKS / 2);
}

pub fn spell_bless(gs: &mut GameState, cn: usize, co: usize, power: i32) -> i32 {
    let in_opt = God::create_item(gs, 1);
    if in_opt.is_none() {
        log::error!("god_create_item failed in skill_bless");
        return 0;
    }
    let in_ = in_opt.unwrap();

    let mut power = power;
    let tmp = spellpower(gs, co);
    if power > tmp {
        if cn != co {
            let reference = gs.characters[co].reference;
            gs.do_character_log(
                cn,
                FontColor::Yellow,
                &format!(
                    "Seeing that {} is not powerful enough for your spell, you reduced its strength.\n",
                    c_string_to_str(&reference)
                ),
            );
        } else {
            gs.do_character_log(
                cn,
                FontColor::Yellow,
                "You are not powerful enough to use the full strength of this spell.\n",
            );
        }
        power = tmp;
    }

    let power = spell_race_mod(gs, power, gs.characters[cn].kindred);

    {
        let mut name_bytes = [0u8; 40];
        let name = b"Bless";
        let len = name.len().min(40);
        name_bytes[..len].copy_from_slice(&name[..len]);
        gs.items[in_].name = name_bytes;
        gs.items[in_].flags |= ItemFlags::IF_SPELL.bits();
        for n in 0..5 {
            gs.items[in_].attrib[n][1] = (power / 5 + 3) as i8;
        }
        gs.items[in_].sprite[1] = 88;
        gs.items[in_].duration = (TICKS * 60 * 10) as u32;
        gs.items[in_].active = (TICKS * 60 * 10) as u32;
        gs.items[in_].temp = SK_BLESS as u16;
        gs.items[in_].power = power as u32;
    }

    if cn != co {
        if add_spell(gs, co, in_) == 0 {
            let name = gs.items[in_].get_name().to_string();
            gs.do_character_log(
                cn,
                FontColor::Yellow,
                &format!("Magical interference neutralised the {}'s effect.\n", name),
            );
            return 0;
        }
        let sense = gs.characters[co].skill[SK_SENSE][5];
        if sense as i32 + 10 > power {
            let reference = gs.characters[cn].reference;
            gs.do_character_log(
                co,
                FontColor::Yellow,
                &format!("{} cast bless on you.\n", c_string_to_str(&reference)),
            );
        } else {
            gs.do_character_log(co, FontColor::Red, "You have been blessed.\n");
        }
        gs.do_character_log(
            cn,
            FontColor::Yellow,
            &format!(
                "{} was blessed.\n",
                gs.characters[co].get_name().to_string()
            ),
        );
        let sound = gs.characters[cn].sound;
        GameState::char_play_sound(gs, co, sound as i32 + 1, -150, 0);
        GameState::char_play_sound(gs, cn, sound as i32 + 1, -150, 0);
        chlog!(
            cn,
            "Cast Bless on {}",
            gs.characters[co].get_name().to_string()
        );
        EffectManager::fx_add_effect(
            gs,
            6,
            0,
            gs.characters[co].x as i32,
            gs.characters[co].y as i32,
            0,
        );
    } else {
        if add_spell(gs, cn, in_) == 0 {
            let name = gs.items[in_].get_name().to_string();
            gs.do_character_log(
                cn,
                FontColor::Yellow,
                &format!("Magical interference neutralised the {}'s effect.\n", name),
            );
            return 0;
        }
        gs.do_character_log(cn, FontColor::Green, "You have been blessed.\n");
        let sound = gs.characters[cn].sound;
        GameState::char_play_sound(gs, cn, sound as i32 + 1, -150, 0);
        let flags = gs.characters[cn].flags;
        if (flags & CharacterFlags::Player.bits()) != 0 {
            chlog!(cn, "Cast Bless");
        }
        EffectManager::fx_add_effect(
            gs,
            6,
            0,
            gs.characters[cn].x as i32,
            gs.characters[cn].y as i32,
            0,
        );
    }

    EffectManager::fx_add_effect(
        gs,
        7,
        0,
        gs.characters[cn].x as i32,
        gs.characters[cn].y as i32,
        0,
    );

    1
}

pub fn skill_bless(gs: &mut GameState, cn: usize) {
    let co = if gs.characters[cn].skill_target1 != 0 {
        gs.characters[cn].skill_target1 as usize
    } else {
        cn
    };

    if gs.do_char_can_see(cn, co) == 0 {
        gs.do_character_log(cn, FontColor::Red, "You cannot see your target.\n");
        return;
    }

    if is_exhausted(gs, cn) != 0 {
        return;
    }

    if player_or_ghost(gs, cn, co) == 0 {
        let name_from = gs.characters[co].get_name().to_string();
        let name_to = gs.characters[cn].get_name().to_string();
        gs.do_character_log(
            cn,
            FontColor::Red,
            &format!(
                "Changed target of spell from {} to {}.\n",
                name_from, name_to
            ),
        );
        // change target to self
        let co = cn;
        if spellcost(gs, cn, 35) != 0 {
            return;
        }
        if chance(gs, cn, 18) != 0 {
            if cn != co {
                let sense = gs.characters[co].skill[SK_SENSE][5];
                let bless_skill = gs.characters[cn].skill[SK_BLESS][5];
                if sense > (bless_skill + 5) {
                    let reference = gs.characters[cn].reference;
                    gs.do_character_log(
                        co,
                        FontColor::Yellow,
                        &format!(
                            "{} tried to cast bless on you but failed.\n",
                            c_string_to_str(&reference)
                        ),
                    );
                }
            }
            return;
        }
        spell_bless(gs, cn, co, gs.characters[cn].skill[SK_BLESS][5] as i32);
        add_exhaust(gs, cn, TICKS);
        return;
    }

    if spellcost(gs, cn, 35) != 0 {
        return;
    }
    if chance(gs, cn, 18) != 0 {
        if cn != co {
            let sense = gs.characters[co].skill[SK_SENSE][5];
            let bless_skill = gs.characters[cn].skill[SK_BLESS][5];
            if sense > (bless_skill + 5) {
                let reference = gs.characters[cn].reference;
                gs.do_character_log(
                    co,
                    FontColor::Yellow,
                    &format!(
                        "{} tried to cast bless on you but failed.\n",
                        c_string_to_str(&reference)
                    ),
                );
            }
        }
        return;
    }

    spell_bless(gs, cn, co, gs.characters[cn].skill[SK_BLESS][5] as i32);
    add_exhaust(gs, cn, TICKS);
}

pub fn skill_wimp(gs: &mut GameState, cn: usize) {
    // If Guardian Angel already active, remove it
    for n in 0..20 {
        let in_idx = gs.characters[cn].spell[n];
        if in_idx != 0 {
            let temp = gs.items[in_idx as usize].temp;
            if temp == SK_WIMPY as u16 {
                gs.do_character_log(
                    cn,
                    core::types::FontColor::Green,
                    "Guardian Angel no longer active.\n",
                );
                gs.items[in_idx as usize].used = core::constants::USE_EMPTY;
                gs.characters[cn].spell[n] = 0;
                gs.do_update_char(cn);
                chlog!(cn, "Dismissed Guardian Angel");
                return;
            }
        }
    }

    let a_end = gs.characters[cn].a_end;
    if a_end < 20000 {
        gs.do_character_log(
            cn,
            core::types::FontColor::Red,
            "You're too exhausted to call on your Guardian Angel.\n",
        );
        return;
    }

    gs.characters[cn].a_end -= 20000;

    let in_opt = God::create_item(gs, 1);
    if in_opt.is_none() {
        log::error!("god_create_item failed in skill_wimp");
        return;
    }
    let in_idx = in_opt.unwrap();

    {
        let mut name_bytes = [0u8; 40];
        let name = b"Guardian Angel";
        let len = name.len().min(40);
        name_bytes[..len].copy_from_slice(&name[..len]);
        gs.items[in_idx].name = name_bytes;
        gs.items[in_idx].flags |= ItemFlags::IF_SPELL.bits() | ItemFlags::IF_PERMSPELL.bits();
        gs.items[in_idx].hp[0] = -1;
        gs.items[in_idx].end[0] = -1;
        gs.items[in_idx].mana[0] = -1;
        gs.items[in_idx].sprite[1] = 94;
        gs.items[in_idx].duration = (TICKS * 60 * 60 * 2) as u32;
        gs.items[in_idx].active = (TICKS * 60 * 60 * 2) as u32;
        gs.items[in_idx].temp = SK_WIMPY as u16;
        gs.items[in_idx].power = gs.characters[cn].skill[SK_WIMPY][5] as u32;
    }

    if add_spell(gs, cn, in_idx) == 0 {
        gs.do_character_log(
            cn,
            core::types::FontColor::Green,
            &format!(
                "Magical interference neutralised the {}'s effect.\n",
                gs.items[in_idx].get_name().to_string()
            ),
        );
        return;
    }
    gs.do_character_log(
        cn,
        core::types::FontColor::Green,
        "Guardian Angel active!\n",
    );
    let sound = gs.characters[cn].sound;
    GameState::char_play_sound(gs, cn, sound as i32 + 1, -150, 0);
    chlog!(cn, "Cast Guardian Angel");
    EffectManager::fx_add_effect(
        gs,
        7,
        0,
        gs.characters[cn].x as i32,
        gs.characters[cn].y as i32,
        0,
    );
    EffectManager::fx_add_effect(
        gs,
        6,
        0,
        gs.characters[cn].x as i32,
        gs.characters[cn].y as i32,
        0,
    );
}

pub fn spell_mshield(gs: &mut GameState, cn: usize, co: usize, power: i32) -> i32 {
    let in_opt = God::create_item(gs, 1);
    if in_opt.is_none() {
        log::error!("god_create_item failed in skill_mshield");
        return 0;
    }
    let in_ = in_opt.unwrap();

    {
        let mut name_bytes = [0u8; 40];
        let name = b"Magic Shield";
        let len = name.len().min(40);
        name_bytes[..len].copy_from_slice(&name[..len]);
        gs.items[in_].name = name_bytes;
        gs.items[in_].flags |= ItemFlags::IF_SPELL.bits();
        gs.items[in_].sprite[1] = 95;
        let dur = spell_race_mod(gs, power * 256, gs.characters[cn].kindred);
        gs.items[in_].duration = dur as u32;
        gs.items[in_].active = dur as u32;
        gs.items[in_].armor[1] = (gs.items[in_].active / 1024) as i8 + 1;
        gs.items[in_].temp = SK_MSHIELD as u16;
        gs.items[in_].power = gs.items[in_].active / 256;
    }

    if cn != co {
        if add_spell(gs, co, in_) == 0 {
            let name = gs.items[in_].get_name().to_string();
            gs.do_character_log(
                cn,
                FontColor::Green,
                &format!("Magical interference neutralised the {}'s effect.\n", name),
            );
            return 0;
        }
        let sense = gs.characters[co].skill[SK_SENSE][5];
        if sense as i32 + 10 > power {
            let reference = gs.characters[cn].reference;
            gs.do_character_log(
                co,
                FontColor::Green,
                &format!(
                    "{} cast magic shield on you.\n",
                    c_string_to_str(&reference)
                ),
            );
        } else {
            gs.do_character_log(co, FontColor::Red, "Magic Shield active!\n");
        }
        gs.do_character_log(
            cn,
            FontColor::Green,
            &format!(
                "{}'s Magic Shield activated.\n",
                gs.characters[co].get_name().to_string()
            ),
        );
        let sound = gs.characters[cn].sound;
        GameState::char_play_sound(gs, co, sound as i32 + 1, -150, 0);
        GameState::char_play_sound(gs, cn, sound as i32 + 1, -150, 0);
        chlog!(
            cn,
            "Cast Magic Shield on {}",
            gs.characters[co].get_name().to_string()
        );
        EffectManager::fx_add_effect(
            gs,
            6,
            0,
            gs.characters[co].x as i32,
            gs.characters[co].y as i32,
            0,
        );
    } else {
        if add_spell(gs, cn, in_) == 0 {
            let name = gs.items[in_].get_name().to_string();
            gs.do_character_log(
                cn,
                FontColor::Green,
                &format!("Magical interference neutralised the {}'s effect.\n", name),
            );
            return 0;
        }
        gs.do_character_log(cn, FontColor::Green, "Magic Shield active!\n");
        let sound = gs.characters[cn].sound;
        GameState::char_play_sound(gs, cn, sound as i32 + 1, -150, 0);
        let flags = gs.characters[cn].flags;
        if (flags & CharacterFlags::Player.bits()) != 0 {
            chlog!(cn, "Cast Magic Shield");
        }
        EffectManager::fx_add_effect(
            gs,
            6,
            0,
            gs.characters[cn].x as i32,
            gs.characters[cn].y as i32,
            0,
        );
    }

    EffectManager::fx_add_effect(
        gs,
        7,
        0,
        gs.characters[cn].x as i32,
        gs.characters[cn].y as i32,
        0,
    );

    1
}

pub fn skill_mshield(gs: &mut GameState, cn: usize) {
    if is_exhausted(gs, cn) != 0 {
        return;
    }

    if spellcost(gs, cn, 25) != 0 {
        return;
    }
    if chance(gs, cn, 18) != 0 {
        return;
    }

    spell_mshield(
        gs,
        cn,
        cn,
        gs.characters[cn].skill[core::constants::SK_MSHIELD][5] as i32,
    );
    add_exhaust(gs, cn, core::constants::TICKS * 3);
}

pub fn spell_heal(gs: &mut GameState, cn: usize, co: usize, power: i32) -> i32 {
    if cn != co {
        gs.characters[co].a_hp += spell_race_mod(gs, power * 2500, gs.characters[cn].kindred);
        if gs.characters[co].a_hp > (gs.characters[co].hp[5] as i32) * 1000 {
            gs.characters[co].a_hp = (gs.characters[co].hp[5] as i32) * 1000;
        }
        let sense = gs.characters[co].skill[core::constants::SK_SENSE][5];
        if sense as i32 + 10 > power {
            let reference = gs.characters[cn].reference;
            gs.do_character_log(
                co,
                FontColor::Green,
                &format!("{} cast heal on you.\n", c_string_to_str(&reference)),
            );
        } else {
            gs.do_character_log(co, FontColor::Red, "You have been healed.\n");
        }
        gs.do_character_log(
            cn,
            FontColor::Green,
            &format!("{} was healed.\n", gs.characters[co].get_name().to_string()),
        );
        let sound = gs.characters[cn].sound;
        GameState::char_play_sound(gs, co, sound as i32 + 1, -150, 0);
        GameState::char_play_sound(gs, cn, sound as i32 + 1, -150, 0);
        chlog!(
            cn,
            "Cast Heal on {}",
            gs.characters[co].get_name().to_string()
        );
        EffectManager::fx_add_effect(
            gs,
            6,
            0,
            gs.characters[co].x as i32,
            gs.characters[co].y as i32,
            0,
        );
    } else {
        gs.characters[cn].a_hp += power * 2500;
        if gs.characters[cn].a_hp > (gs.characters[cn].hp[5] as i32) * 1000 {
            gs.characters[cn].a_hp = (gs.characters[cn].hp[5] as i32) * 1000;
        }
        gs.do_character_log(cn, FontColor::Green, "You have been healed.\n");
        let sound = gs.characters[cn].sound;
        GameState::char_play_sound(gs, cn, sound as i32 + 1, -150, 0);
        let flags = gs.characters[cn].flags;
        if (flags & CharacterFlags::Player.bits()) != 0 {
            chlog!(cn, "Cast Heal");
        }
        EffectManager::fx_add_effect(
            gs,
            6,
            0,
            gs.characters[cn].x as i32,
            gs.characters[cn].y as i32,
            0,
        );
    }

    EffectManager::fx_add_effect(
        gs,
        7,
        0,
        gs.characters[cn].x as i32,
        gs.characters[cn].y as i32,
        0,
    );

    1
}

pub fn skill_heal(gs: &mut GameState, cn: usize) {
    let mut co = if gs.characters[cn].skill_target1 != 0 {
        gs.characters[cn].skill_target1 as usize
    } else {
        cn
    };

    if gs.do_char_can_see(cn, co) == 0 {
        gs.do_character_log(cn, FontColor::Red, "You cannot see your target.\n");
        return;
    }

    if is_exhausted(gs, cn) != 0 {
        return;
    }

    if player_or_ghost(gs, cn, co) == 0 {
        let name_from = gs.characters[co].get_name().to_string();
        let name_to = gs.characters[cn].get_name().to_string();
        gs.do_character_log(
            cn,
            FontColor::Red,
            &format!(
                "Changed target of spell from {} to {}.\n",
                name_from, name_to
            ),
        );
        co = cn;
        if spellcost(gs, cn, 25) != 0 {
            return;
        }
        if chance(gs, cn, 18) != 0 {
            if cn != co {
                let sense = gs.characters[co].skill[SK_SENSE][5];
                let heal_skill = gs.characters[cn].skill[SK_HEAL][5];
                if sense > (heal_skill + 5) {
                    let reference = gs.characters[cn].reference;
                    gs.do_character_log(
                        co,
                        FontColor::Green,
                        &format!(
                            "{} tried to cast heal on you but failed.\n",
                            c_string_to_str(&reference)
                        ),
                    );
                }
            }
            return;
        }
        spell_heal(gs, cn, co, gs.characters[cn].skill[SK_HEAL][5] as i32);
        add_exhaust(gs, cn, TICKS * 2);
        return;
    }

    if spellcost(gs, cn, 25) != 0 {
        return;
    }
    if chance(gs, cn, 18) != 0 {
        if cn != co {
            let sense = gs.characters[co].skill[SK_SENSE][5];
            let heal_skill = gs.characters[cn].skill[SK_HEAL][5];
            if sense > (heal_skill + 5) {
                let reference = gs.characters[cn].reference;
                gs.do_character_log(
                    co,
                    FontColor::Green,
                    &format!(
                        "{} tried to cast heal on you but failed.\n",
                        c_string_to_str(&reference)
                    ),
                );
            }
        }
        return;
    }

    spell_heal(gs, cn, co, gs.characters[cn].skill[SK_HEAL][5] as i32);

    add_exhaust(gs, cn, TICKS * 2);
}

pub fn spell_curse(gs: &mut GameState, cn: usize, co: usize, power: i32) -> i32 {
    let flags = gs.characters[co].flags;
    if (flags & CharacterFlags::Immortal.bits()) != 0 {
        return 0;
    }

    let in_opt = God::create_item(gs, 1);
    if in_opt.is_none() {
        log::error!("god_create_item failed in spell_curse");
        return 0;
    }
    let in_idx = in_opt.unwrap();

    let mut power = power;
    power = spell_immunity(gs, power, gs.characters[co].skill[SK_IMMUN][5] as i32);
    power = spell_race_mod(gs, power, gs.characters[cn].kindred);

    {
        let mut name_bytes = [0u8; 40];
        let name = b"Curse";
        let len = name.len().min(40);
        name_bytes[..len].copy_from_slice(&name[..len]);
        gs.items[in_idx].name = name_bytes;
        gs.items[in_idx].flags |= ItemFlags::IF_SPELL.bits();
        for n in 0..5 {
            gs.items[in_idx].attrib[n][1] = -((power / 3) as i8);
        }
        gs.items[in_idx].sprite[1] = 89;
        gs.items[in_idx].duration = (TICKS * 60 * 2) as u32;
        gs.items[in_idx].active = (TICKS * 60 * 2) as u32;
        gs.items[in_idx].temp = SK_CURSE as u16;
        gs.items[in_idx].power = power as u32;
    }

    if add_spell(gs, co, in_idx) == 0 {
        gs.do_character_log(
            cn,
            FontColor::Green,
            &format!(
                "Magical interference neutralised the {}'s effect.\n",
                gs.items[in_idx].get_name().to_string()
            ),
        );
        return 0;
    }

    let sense = gs.characters[co].skill[SK_SENSE][5];
    if (sense as i32 + 10) > power {
        let reference = gs.characters[cn].reference;
        gs.do_character_log(
            co,
            FontColor::Green,
            &format!("{} cast curse on you.\n", c_string_to_str(&reference)),
        );
    } else {
        gs.do_character_log(co, FontColor::Green, "You have been cursed.\n");
    }

    let name = gs.characters[co].get_name().to_string();
    gs.do_character_log(cn, FontColor::Green, &format!("{} was cursed.\n", name));

    // Match C: don't generate spell-attack notifications when the target is ignoring spells.
    if (gs.characters[co].flags & CharacterFlags::SpellIgnore.bits()) == 0 {
        gs.do_notify_character(co as u32, NT_GOTHIT as i32, cn as i32, 0, 0, 0);
    }
    gs.do_notify_character(cn as u32, NT_DIDHIT as i32, co as i32, 0, 0, 0);

    let sound = gs.characters[cn].sound;
    GameState::char_play_sound(gs, co, sound as i32 + 7, -150, 0);
    GameState::char_play_sound(gs, cn, sound as i32 + 1, -150, 0);
    chlog!(
        cn,
        "Cast Curse on {}",
        gs.characters[co].get_name().to_string()
    );
    EffectManager::fx_add_effect(
        gs,
        5,
        0,
        gs.characters[co].x as i32,
        gs.characters[co].y as i32,
        0,
    );

    1
}

pub fn skill_curse(gs: &mut GameState, cn: usize) {
    let co = if gs.characters[cn].skill_target1 != 0 {
        gs.characters[cn].skill_target1 as usize
    } else if gs.characters[cn].attack_cn != 0 {
        gs.characters[cn].attack_cn as usize
    } else {
        cn
    };

    if cn == co {
        gs.do_character_log(
            cn,
            core::types::FontColor::Red,
            "You cannot curse yourself.\n",
        );
        return;
    }

    if gs.do_char_can_see(cn, co) == 0 {
        gs.do_character_log(
            cn,
            core::types::FontColor::Red,
            "You cannot see your target.\n",
        );
        return;
    }

    gs.remember_pvp(cn, co);
    if is_exhausted(gs, cn) != 0 {
        return;
    }

    if spellcost(gs, cn, 35) != 0 {
        return;
    }

    if gs.may_attack_msg(cn, co, true) == 0 {
        chlog!(
            cn,
            "Prevented from attacking {}",
            gs.characters[co].get_name().to_string()
        );
        return;
    }

    if chance_base(
        gs,
        cn,
        gs.characters[cn].skill[core::constants::SK_CURSE][5] as i32,
        10,
        gs.characters[co].skill[core::constants::SK_RESIST][5] as i32,
    ) != 0
    {
        if cn != co
            && gs.characters[co].skill[core::constants::SK_SENSE][5]
                > (gs.characters[cn].skill[core::constants::SK_CURSE][5] + 5)
        {
            let reference = gs.characters[cn].reference;
            gs.do_character_log(
                co,
                core::types::FontColor::Green,
                &format!(
                    "{} tried to cast curse on you but failed.\n",
                    c_string_to_str(&reference)
                ),
            );
            if gs.characters[co].flags & CharacterFlags::SpellIgnore.bits() == 0 {
                gs.do_notify_character(
                    co as u32,
                    core::constants::NT_GOTMISS as i32,
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
        gs.do_character_log(cn, core::types::FontColor::Red, "You lost your focus.\n");
        return;
    }

    spell_curse(
        gs,
        cn,
        co,
        gs.characters[cn].skill[core::constants::SK_CURSE][5] as i32,
    );

    let co_orig = co;
    let (x, y) = (gs.characters[cn].x as i32, gs.characters[cn].y as i32);
    let adj: [(i32, i32); 4] = [(1, 0), (-1, 0), (0, 1), (0, -1)];

    for (dx, dy) in adj {
        let nx = x + dx;
        let ny = y + dy;

        // Prevent negative/out-of-bounds coords from wrapping into huge usize indices.
        if nx < 0
            || ny < 0
            || nx >= core::constants::SERVER_MAPX
            || ny >= core::constants::SERVER_MAPY
        {
            continue;
        }

        let idx = (nx + ny * core::constants::SERVER_MAPX) as usize;
        let maybe_co = gs.map[idx].ch as usize;
        if maybe_co == 0 || maybe_co >= core::constants::MAXCHARS {
            continue;
        }
        if maybe_co != 0 && gs.characters[maybe_co].attack_cn as usize == cn && co_orig != maybe_co
        {
            if gs.characters[cn].skill[core::constants::SK_CURSE][5] as i32
                + helpers::random_mod_i32(20)
                > gs.characters[maybe_co].skill[core::constants::SK_RESIST][5] as i32
                    + helpers::random_mod_i32(20)
            {
                spell_curse(
                    gs,
                    cn,
                    maybe_co,
                    gs.characters[cn].skill[core::constants::SK_CURSE][5] as i32,
                );
            }
        }
    }

    EffectManager::fx_add_effect(
        gs,
        7,
        0,
        gs.characters[cn].x as i32,
        gs.characters[cn].y as i32,
        0,
    );

    add_exhaust(gs, cn, core::constants::TICKS * 4);
}

pub fn warcry(gs: &mut GameState, cn: usize, co: usize, power: i32) -> i32 {
    if gs.characters[cn].attack_cn as usize != co && gs.characters[co].alignment == 10000 {
        return 0;
    }

    if gs.may_attack_msg(cn, co, false) == 0 {
        return 0;
    }

    if power < gs.characters[co].skill[core::constants::SK_RESIST][5] as i32 {
        return 0;
    }

    for n in 1..10 {
        if gs.characters[cn].data[n] as usize == co {
            return 0;
        }
    }

    if (gs.characters[co].flags & CharacterFlags::Immortal.bits()) != 0 {
        return 0;
    }

    if gs.characters[co].flags & CharacterFlags::SpellIgnore.bits() == 0 {
        gs.do_notify_character(
            co as u32,
            core::constants::NT_GOTHIT as i32,
            cn as i32,
            0,
            0,
            0,
        );
    }

    let in_opt = God::create_item(gs, 1);
    if in_opt.is_none() {
        log::error!("god_create_item failed in skill_warcry");
        return 0;
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
        gs.items[in_idx].temp = core::constants::SK_WARCRY2 as u16;
        gs.items[in_idx].power = power as u32;
    }

    add_spell(gs, co, in_idx);

    let in2_opt = God::create_item(gs, 1);
    if in2_opt.is_none() {
        log::error!("god_create_item failed in skill_warcry");
        return 0;
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
        gs.items[in2].temp = core::constants::SK_WARCRY as u16;
        gs.items[in2].power = (power / 2) as u32;
    }

    add_spell(gs, co, in2);

    let co_name = gs.characters[co].get_name().to_string();
    log::info!("Character {} cast Warcry on {}", cn, co_name);

    EffectManager::fx_add_effect(
        gs,
        5,
        0,
        gs.characters[co].x as i32,
        gs.characters[co].y as i32,
        0,
    );

    1
}

pub fn skill_warcry(gs: &mut GameState, cn: usize) {
    if gs.characters[cn].a_end < 150 * 1000 {
        gs.do_character_log(cn, core::types::FontColor::Red, "You're too exhausted!\n");
        return;
    }

    gs.characters[cn].a_end -= 150 * 1000;

    let power = gs.characters[cn].skill[core::constants::SK_WARCRY][5] as i32;

    let xf = std::cmp::max(1, gs.characters[cn].x as i32 - 10);
    let yf = std::cmp::max(1, gs.characters[cn].y as i32 - 10);
    let xt = std::cmp::min(
        core::constants::SERVER_MAPX - 1,
        gs.characters[cn].x as i32 + 10,
    );
    let yt = std::cmp::min(
        core::constants::SERVER_MAPY - 1,
        gs.characters[cn].y as i32 + 10,
    );

    let mut hit = 0;
    let mut miss = 0;
    for x in xf..xt {
        for y in yf..yt {
            let m = (x + y * core::constants::SERVER_MAPX) as usize;
            let co = gs.map[m].ch as usize;
            if co != 0 {
                if warcry(gs, cn, co, power) != 0 {
                    gs.remember_pvp(cn, co);
                    let name = gs.characters[cn].get_name().to_string();
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
                    let name = gs.characters[cn].get_name().to_string();
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

pub fn item_info(gs: &mut GameState, cn: usize, in_: usize, _look: i32) {
    let at_name = ["Braveness", "Willpower", "Intuition", "Agility", "Strength"];

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
            &format!("{:<12.12} {:+4} {:+4} {:3}\n", at_name[n], a0, a1, a2),
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

    for n in 0..50 {
        let (s0, s1, s2) = (
            gs.items[in_].skill[n][0],
            gs.items[in_].skill[n][1],
            gs.items[in_].skill[n][2],
        );
        if s0 == 0 && s1 == 0 && s2 == 0 {
            continue;
        }
        let skill_label = skill_name(n);
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

pub fn char_info(gs: &mut GameState, cn: usize, co: usize) {
    let at_name = ["Braveness", "Willpower", "Intuition", "Agility", "Strength"];

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
            let item_name = gs.items[in_idx].get_name().to_string();
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
    for n in 0..50 {
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
            let name1 = SKILL_NAMES[n1 as usize];
            let name2 = SKILL_NAMES[n2 as usize];
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
        let name1 = SKILL_NAMES[n1 as usize];
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
            at_name[0], a0_0, a0_5, at_name[1], a1_0, a1_5
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
            at_name[2], a2_0, a2_5, at_name[3], a3_0, a3_5
        ),
    );
    let a4_0 = gs.characters[co].attrib[4][0];
    let a4_5 = gs.characters[co].attrib[4][5];
    gs.do_character_log(
        cn,
        FontColor::Green,
        &format!("{:<12.12} {:3}/{:3}\n", at_name[4], a4_0, a4_5),
    );

    gs.do_character_log(cn, FontColor::Green, " \n");
}

pub fn skill_identify(gs: &mut GameState, cn: usize) {
    if is_exhausted(gs, cn) != 0 {
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
            power = gs.characters[co].skill[SK_RESIST][5] as i32;
        } else {
            co = cn;
            power = 10;
        }
        in_idx = 0;
    }

    if chance_base(
        gs,
        cn,
        gs.characters[cn].skill[SK_IDENT][5] as i32,
        18,
        power,
    ) != 0
    {
        return;
    }

    let sound = gs.characters[cn].sound;
    GameState::char_play_sound(gs, cn, sound as i32 + 1, -150, 0);
    chlog!(
        cn,
        "Cast Identify on {}",
        if in_idx != 0 {
            gs.items[in_idx].get_name().to_string()
        } else {
            gs.characters[co].get_name().to_string()
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
            gs.characters[co].x as i32,
            gs.characters[co].y as i32,
            0,
        );
    }

    add_exhaust(gs, cn, TICKS * 2);
    EffectManager::fx_add_effect(
        gs,
        7,
        0,
        gs.characters[cn].x as i32,
        gs.characters[cn].y as i32,
        0,
    );
}

pub fn skill_blast(gs: &mut GameState, cn: usize) {
    let co = if gs.characters[cn].skill_target1 != 0 {
        gs.characters[cn].skill_target1 as usize
    } else if gs.characters[cn].attack_cn != 0 {
        gs.characters[cn].attack_cn as usize
    } else {
        cn
    };

    if gs.do_char_can_see(cn, co) == 0 {
        gs.do_character_log(cn, FontColor::Green, "You cannot see your target.\n");
        return;
    }

    if cn == co {
        gs.do_character_log(cn, FontColor::Green, "You cannot blast yourself!\n");
        return;
    }

    if (gs.characters[co].flags & CharacterFlags::Stoned.bits()) != 0 {
        gs.do_character_log(
            cn,
            FontColor::Green,
            "Your target is lagging. Try again later.\n",
        );
        return;
    }

    if gs.may_attack_msg(cn, co, true) == 0 {
        chlog!(
            cn,
            "Prevented from attacking {}",
            gs.characters[co].get_name().to_string()
        );
        return;
    }

    gs.remember_pvp(cn, co);

    if is_exhausted(gs, cn) != 0 {
        return;
    }

    let mut power = gs.characters[cn].skill[core::constants::SK_BLAST][5] as i32;
    power = spell_immunity(
        gs,
        power,
        gs.characters[co].skill[core::constants::SK_IMMUN][5] as i32,
    );
    power = spell_race_mod(gs, power, gs.characters[cn].kindred);

    let mut dam = power * 2;

    let mut cost = dam / 8 + 5;
    if (gs.characters[cn].flags & CharacterFlags::Player.bits()) != 0
        && ((gs.characters[cn].kindred as u32)
            & (core::constants::KIN_HARAKIM | core::constants::KIN_ARCHHARAKIM)
            != 0)
    {
        cost /= 3;
    }

    if spellcost(gs, cn, cost) != 0 {
        return;
    }

    if chance(gs, cn, 18) != 0 {
        if cn != co
            && gs.characters[co].skill[core::constants::SK_SENSE][5]
                > gs.characters[cn].skill[core::constants::SK_BLAST][5] + 5
        {
            gs.do_character_log(
                co,
                FontColor::Green,
                &format!(
                    "{} tried to cast blast on you but failed.\n",
                    c_string_to_str(&gs.characters[cn].reference)
                ),
            );
            if gs.characters[co].flags & CharacterFlags::SpellIgnore.bits() == 0 {
                gs.do_notify_character(
                    co as u32,
                    core::constants::NT_GOTMISS as i32,
                    cn as i32,
                    0,
                    0,
                    0,
                );
            }
        }
        return;
    }

    gs.do_area_sound(
        co,
        0,
        gs.characters[co].x as i32,
        gs.characters[co].y as i32,
        gs.characters[cn].sound as i32 + 6,
    );
    GameState::char_play_sound(gs, co, gs.characters[cn].sound as i32 + 6, -150, 0);

    chlog!(
        cn,
        "Cast Blast on {} for {} power",
        gs.characters[co].get_name().to_string(),
        power
    );
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
            &format!("You blast your target for {} HP.\n", tmp),
        );
    }

    EffectManager::fx_add_effect(
        gs,
        5,
        0,
        gs.characters[co].x as i32,
        gs.characters[co].y as i32,
        0,
    );

    let co_orig = co;
    dam = dam / 2 + dam / 4;

    let (cx, cy) = (gs.characters[cn].x as i32, gs.characters[cn].y as i32);
    let mut neighbors: [(i32, i32); 4] = [(0, 0); 4];
    let mut neighbor_count = 0usize;

    if cx + 1 < SERVER_MAPX {
        neighbors[neighbor_count] = (cx + 1, cy);
        neighbor_count += 1;
    }
    if cx - 1 >= 0 {
        neighbors[neighbor_count] = (cx - 1, cy);
        neighbor_count += 1;
    }
    if cy + 1 < core::constants::SERVER_MAPY {
        neighbors[neighbor_count] = (cx, cy + 1);
        neighbor_count += 1;
    }
    if cy - 1 >= 0 {
        neighbors[neighbor_count] = (cx, cy - 1);
        neighbor_count += 1;
    }

    // Check four adjacent tiles
    for (nx, ny) in neighbors.into_iter().take(neighbor_count) {
        let idx = (nx as usize) + (ny as usize) * SERVER_MAPX as usize;
        let maybe_co = gs.map[idx].ch as usize;
        if maybe_co == 0 || maybe_co >= core::constants::MAXCHARS {
            continue;
        }
        if maybe_co == co_orig {
            continue;
        }
        if gs.characters[maybe_co].attack_cn != cn as u16 {
            continue;
        }

        let tmp2 = gs.do_hurt(cn, maybe_co, dam, 1);
        if tmp2 < 1 {
            gs.do_character_log(
                cn,
                FontColor::Green,
                "You cannot penetrate your target's armor.\n",
            );
        } else {
            gs.do_character_log(
                cn,
                FontColor::Green,
                &format!("You blast your target for {} HP.\n", tmp2),
            );
        }
        EffectManager::fx_add_effect(
            gs,
            5,
            0,
            gs.characters[maybe_co].x as i32,
            gs.characters[maybe_co].y as i32,
            0,
        );
    }

    add_exhaust(gs, cn, core::constants::TICKS * 6);
    EffectManager::fx_add_effect(
        gs,
        7,
        0,
        gs.characters[cn].x as i32,
        gs.characters[cn].y as i32,
        0,
    );
}

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

    if gs.items[in_idx].power as i32 > gs.characters[cn].skill[SK_REPAIR][5] as i32
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
        let skill = gs.characters[cn].skill[SK_REPAIR][5] as i32;
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
        gs.items[in_idx].get_name().to_string()
    );
}

pub fn skill_recall(gs: &mut GameState, cn: usize) {
    if is_exhausted(gs, cn) != 0 {
        return;
    }

    if spellcost(gs, cn, 15) != 0 {
        return;
    }

    if chance(gs, cn, 18) != 0 {
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
        let base_dur = 60 - (gs.characters[cn].skill[SK_RECALL][5] / 4) as i32;
        let dur = std::cmp::max(TICKS / 2, base_dur * TICKS / LEGACY_TICKS);
        gs.items[in_idx].duration = dur as u32;
        gs.items[in_idx].active = gs.items[in_idx].duration;
        gs.items[in_idx].temp = SK_RECALL as u16;
        gs.items[in_idx].power = gs.characters[cn].skill[SK_RECALL][5] as u32;
        gs.items[in_idx].data[0] = gs.characters[cn].temple_x as u32;
        gs.items[in_idx].data[1] = gs.characters[cn].temple_y as u32;
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
        gs.characters[cn].x as i32,
        gs.characters[cn].y as i32,
        0,
    );
}

pub fn spell_stun(gs: &mut GameState, cn: usize, co: usize, power: i32) -> i32 {
    if (gs.characters[co].flags & CharacterFlags::Immortal.bits()) != 0 {
        return 0;
    }

    let in_opt = God::create_item(gs, 1);
    if in_opt.is_none() {
        return 0;
    }
    let in_idx = in_opt.unwrap();

    let mut power = spell_immunity(
        gs,
        power,
        gs.characters[co].skill[core::constants::SK_IMMUN][5] as i32,
    );
    power = spell_race_mod(gs, power, gs.characters[cn].kindred);

    {
        let mut name_bytes = [0u8; 40];
        let name = b"Stun";
        let len = name.len().min(40);
        name_bytes[..len].copy_from_slice(&name[..len]);
        gs.items[in_idx].name = name_bytes;
        gs.items[in_idx].flags |= ItemFlags::IF_SPELL.bits();
        gs.items[in_idx].sprite[1] = 91;
        gs.items[in_idx].duration = (power + core::constants::TICKS) as u32;
        gs.items[in_idx].active = gs.items[in_idx].duration;
        gs.items[in_idx].temp = core::constants::SK_STUN as u16;
        gs.items[in_idx].power = power as u32;
    }

    if gs.characters[co].skill[core::constants::SK_SENSE][5] + 10 > power as u8 {
        gs.do_character_log(
            co,
            FontColor::Green,
            &format!(
                "{} cast stun on you.\n",
                c_string_to_str(&gs.characters[cn].reference)
            ),
        );
    } else {
        gs.do_character_log(co, FontColor::Green, "You have been stunned.\n");
    }

    gs.do_character_log(
        cn,
        FontColor::Green,
        &format!(
            "{} was stunned.\n",
            c_string_to_str(&gs.characters[co].reference)
        ),
    );

    if gs.characters[co].flags & CharacterFlags::SpellIgnore.bits() == 0 {
        gs.do_notify_character(
            co as u32,
            core::constants::NT_GOTHIT as i32,
            cn as i32,
            0,
            0,
            0,
        );
    }
    gs.do_notify_character(
        cn as u32,
        core::constants::NT_DIDHIT as i32,
        co as i32,
        0,
        0,
        0,
    );

    GameState::char_play_sound(gs, co, gs.characters[cn].sound as i32 + 7, -150, 0);
    GameState::char_play_sound(gs, cn, gs.characters[cn].sound as i32 + 1, -150, 0);
    chlog!(
        cn,
        "Cast Stun on {} for {} power",
        gs.characters[co].get_name().to_string(),
        power
    );

    if add_spell(gs, co, in_idx) == 0 {
        gs.do_character_log(
            cn,
            FontColor::Green,
            &format!(
                "Magical interference neutralised the {}'s effect.\n",
                "stun"
            ),
        );
        return 0;
    }

    EffectManager::fx_add_effect(
        gs,
        5,
        0,
        gs.characters[co].x as i32,
        gs.characters[co].y as i32,
        0,
    );

    1
}

pub fn skill_stun(gs: &mut GameState, cn: usize) {
    let co = if gs.characters[cn].skill_target1 != 0 {
        gs.characters[cn].skill_target1 as usize
    } else if gs.characters[cn].attack_cn != 0 {
        gs.characters[cn].attack_cn as usize
    } else {
        cn
    };

    if cn == co {
        gs.do_character_log(
            cn,
            core::types::FontColor::Green,
            "You cannot stun yourself!\n",
        );
        return;
    }

    if gs.do_char_can_see(cn, co) == 0 {
        gs.do_character_log(
            cn,
            core::types::FontColor::Green,
            "You cannot see your target.\n",
        );
        return;
    }

    gs.remember_pvp(cn, co);
    if is_exhausted(gs, cn) != 0 {
        return;
    }

    if gs.may_attack_msg(cn, co, true) == 0 {
        chlog!(
            cn,
            "Prevented from attacking {}",
            gs.characters[co].get_name().to_string()
        );
        return;
    }

    if spellcost(gs, cn, 20) != 0 {
        return;
    }

    if chance_base(
        gs,
        cn,
        gs.characters[cn].skill[core::constants::SK_STUN][5] as i32,
        12,
        gs.characters[co].skill[core::constants::SK_RESIST][5] as i32,
    ) != 0
    {
        if cn != co
            && gs.characters[co].skill[core::constants::SK_SENSE][5]
                > gs.characters[cn].skill[core::constants::SK_STUN][5] + 5
        {
            gs.do_character_log(
                co,
                core::types::FontColor::Green,
                &format!(
                    "{} tried to cast stun on you but failed.\n",
                    c_string_to_str(&gs.characters[cn].reference)
                ),
            );
            if gs.characters[co].flags & CharacterFlags::SpellIgnore.bits() == 0 {
                gs.do_notify_character(
                    co as u32,
                    core::constants::NT_GOTMISS as i32,
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
        gs.do_character_log(cn, core::types::FontColor::Red, "You lost your focus.\n");
        return;
    }

    let power = gs.characters[cn].skill[core::constants::SK_STUN][5] as i32;
    spell_stun(gs, cn, co, power);

    let co_orig = co;
    let m = gs.characters[cn].x + gs.characters[cn].y * core::constants::SERVER_MAPX as i16;

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
            if gs.characters[cn].skill[core::constants::SK_STUN][5] as i32 + s_rand
                > gs.characters[maybe_co].skill[core::constants::SK_RESIST][5] as i32 + o_rand
            {
                spell_stun(
                    gs,
                    cn,
                    maybe_co,
                    gs.characters[cn].skill[core::constants::SK_STUN][5] as i32,
                );
            }
        }
    }

    EffectManager::fx_add_effect(
        gs,
        7,
        0,
        gs.characters[cn].x as i32,
        gs.characters[cn].y as i32,
        0,
    );
    add_exhaust(gs, cn, core::constants::TICKS * 3);
}

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

pub fn skill_dispel(gs: &mut GameState, cn: usize) {
    // Port of C `skill_dispel(int cn)`.
    let target = gs.characters[cn].skill_target1 as usize;
    let co = if target != 0 { target } else { cn };

    if gs.do_char_can_see(cn, co) == 0 {
        gs.do_character_log(cn, FontColor::Red, "You cannot see your target.\n");
        return;
    }

    if is_exhausted(gs, cn) != 0 {
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
                let name = gs.characters[co].get_name().to_string();
                gs.do_character_log(cn, FontColor::Red, &format!("{} isn't spelled!\n", name));
            }
            return;
        }

        // Dispelling someone else's non-curse spell is treated like an attack.
        if target != 0 {
            if gs.may_attack_msg(cn, co, true) == 0 {
                chlog!(
                    cn,
                    "Prevented from dispelling {}",
                    gs.characters[co].get_name().to_string()
                );
                return;
            }
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

    let dispel_skill = gs.characters[cn].skill[SK_DISPEL][5] as i32;
    let kindred = gs.characters[cn].kindred;
    if chance_base(gs, cn, spell_race_mod(gs, dispel_skill, kindred), 12, pwr) != 0 {
        if cn != co {
            let sense = gs.characters[co].skill[SK_SENSE][5] as i32;
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
    let removed_name = gs.items[in_idx].get_name().to_string();

    // Remove the spell item and unlink it from the target.
    gs.items[in_idx].used = core::constants::USE_EMPTY;
    gs.characters[co].spell[slot] = 0;
    gs.do_update_char(co);

    // Remember PvP attacks when dispelling non-curse from someone else.
    if target != 0 && removed_temp != SK_CURSE as u16 {
        gs.remember_pvp(cn, co);
    }

    let sound = gs.characters[cn].sound as i32;

    if target != 0 {
        let sense = gs.characters[co].skill[SK_SENSE][5] as i32;
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

        let target_name = gs.characters[co].get_name().to_string();
        gs.do_character_log(
            cn,
            FontColor::Green,
            &format!("Removed {} from {}.\n", removed_name, target_name),
        );

        let target_is_player = (gs.characters[co].flags & CharacterFlags::Player.bits()) != 0;
        if removed_temp != SK_CURSE as u16 && !target_is_player {
            if (gs.characters[co].flags & CharacterFlags::SpellIgnore.bits()) == 0 {
                gs.do_notify_character(co as u32, NT_GOTHIT as i32, cn as i32, 0, 0, 0);
            }
            gs.do_notify_character(cn as u32, NT_DIDHIT as i32, co as i32, 0, 0, 0);
        }

        GameState::char_play_sound(gs, co, sound + 1, -150, 0);
        GameState::char_play_sound(gs, cn, sound + 1, -150, 0);
        chlog!(
            cn,
            "Cast Dispel on {}",
            gs.characters[co].get_name().to_string()
        );
        EffectManager::fx_add_effect(
            gs,
            6,
            0,
            gs.characters[co].x as i32,
            gs.characters[co].y as i32,
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
            gs.characters[cn].x as i32,
            gs.characters[cn].y as i32,
            0,
        );
    }

    add_exhaust(gs, cn, TICKS * 2);
    EffectManager::fx_add_effect(
        gs,
        7,
        0,
        gs.characters[cn].x as i32,
        gs.characters[cn].y as i32,
        0,
    );
}

pub fn skill_ghost(gs: &mut GameState, cn: usize) {
    // Check if in build mode
    if (gs.characters[cn].flags & CharacterFlags::BuildMode.bits()) != 0 {
        gs.do_character_log(cn, FontColor::Red, "Not in build mode.\n");
        return;
    }

    let existing_companion = {
        if (gs.characters[cn].flags & CharacterFlags::Player.bits()) != 0 {
            let co = gs.characters[cn].data[CHD_COMPANION] as usize;
            if co != 0 {
                if Character::is_sane_character(co)
                    && gs.characters[co].data[63] == cn as i32
                    && (gs.characters[co].flags & CharacterFlags::Body.bits()) == 0
                    && gs.characters[co].used != USE_EMPTY
                {
                    Some(co)
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        }
    };

    if let Some(co) = existing_companion {
        gs.do_character_log(
            cn,
            FontColor::Red,
            &format!("You may not have more than one Ghost Companion ({}).\n", co),
        );
        return;
    }

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

    if is_exhausted(gs, cn) != 0 {
        return;
    }

    // Check if can attack target
    if co != 0 && gs.may_attack_msg(cn, co, true) == 0 {
        chlog!(
            cn,
            "Prevented from attacking {} ({})",
            gs.characters[co].get_name().to_string(),
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

    // Chance check
    if chance(gs, cn, 15) != 0 {
        if co != 0 && cn != co {
            let sense = gs.characters[co].skill[SK_SENSE][5] as i32;
            let ghost_skill = gs.characters[cn].skill[SK_GHOST][5] as i32;
            if sense > ghost_skill + 5 {
                let cn_ref = gs.characters[cn].reference;
                gs.do_character_log(
                    co,
                    FontColor::Green,
                    &format!(
                        "{} tried to cast ghost companion on you but failed.\n",
                        c_string_to_str(&cn_ref)
                    ),
                );
                if (gs.characters[co].flags & CharacterFlags::SpellIgnore.bits()) == 0 {
                    gs.do_notify_character(co as u32, NT_GOTMISS as i32, cn as i32, 0, 0, 0);
                }
            }
        }
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
            gs.do_notify_character(co as u32, NT_GOTHIT as i32, cn as i32, 0, 0, 0);
        }
        gs.do_notify_character(cn as u32, NT_DIDHIT as i32, co as i32, 0, 0, 0);
    }

    if (gs.characters[cn].flags & CharacterFlags::Player.bits()) != 0 {
        gs.characters[cn].data[CHD_COMPANION] = cc as i32;
    }

    let mut base = (gs.characters[cn].skill[SK_GHOST][5] as i32 * 4) / 11;
    let kindred = gs.characters[cn].kindred;
    base = spell_race_mod(gs, base, kindred);

    let ticker = gs.globals.ticker;

    let co_id = if co != 0 {
        helpers::char_id(&gs.characters[co as usize])
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

    let text0 = b"#14#Yes! %s buys the farm!";
    gs.characters[cc].text[0][..text0.len()].copy_from_slice(text0);
    let text1 = b"#13#Yahoo! An enemy! Prepare to die, %s!";
    gs.characters[cc].text[1][..text1.len()].copy_from_slice(text1);
    let text3 = b"My successor will avenge me, %s!";
    gs.characters[cc].text[3][..text3.len()].copy_from_slice(text3);

    gs.characters[cc].data[48] = 33;

    gs.characters[cc].data[CHD_TALKATIVE] =
        gs.character_templates[CT_COMPANION as usize].data[CHD_TALKATIVE];

    for n in 0..5 {
        let mut tmp = base;
        tmp = tmp * 3 / std::cmp::max(1, gs.characters[cc].attrib[n][3] as i32);
        gs.characters[cc].attrib[n][0] = std::cmp::max(
            10,
            std::cmp::min(gs.characters[cc].attrib[n][2] as i32, tmp) as u8,
        );
    }

    for n in 0..50 {
        let mut tmp = base;
        tmp = tmp * 3 / std::cmp::max(1, gs.characters[cc].skill[n][3] as i32);
        if gs.characters[cc].skill[n][2] != 0 {
            gs.characters[cc].skill[n][0] = std::cmp::min(gs.characters[cc].skill[n][2], tmp as u8);
        }
    }

    gs.characters[cc].hp[0] =
        std::cmp::max(50, std::cmp::min(gs.characters[cc].hp[2] as i32, base * 5)) as u16;
    gs.characters[cc].end[0] =
        std::cmp::max(50, std::cmp::min(gs.characters[cc].end[2] as i32, base * 5)) as u16;
    gs.characters[cc].mana[0] = 0;

    let mut pts = 0i32;

    let attribs = gs.characters[cc].attrib;
    let hp0 = gs.characters[cc].hp[0];
    let end0 = gs.characters[cc].end[0];
    let mana0 = gs.characters[cc].mana[0];
    let skills = gs.characters[cc].skill;

    for z in 0..5 {
        for m in 10..(attribs[z][0] as i32) {
            pts += helpers::attrib_needed(m, 3);
        }
    }

    for m in 50..(hp0 as i32) {
        pts += helpers::hp_needed(m, 3);
    }

    for m in 50..(end0 as i32) {
        pts += helpers::end_needed(m, 2);
    }

    for m in 50..(mana0 as i32) {
        pts += helpers::mana_needed(m, 3);
    }

    for z in 0..50 {
        for m in 1..(skills[z][0] as i32) {
            pts += helpers::skill_needed(m, 2);
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
        let co_name = gs.characters[co].get_name().to_string();
        gs.do_sayx(
            cc,
            &format!("#13#Yahoo! An enemy! Prepare to die, {}!", co_name),
        );
    } else {
        let rank = core::ranks::points2rank(pts as u32);
        let cn_name = gs.characters[cn].get_name().to_string();
        if rank < 6 {
            // GC not yet Master Sergeant
            gs.do_sayx(cc, &format!("I shall defend you and obey your commands, {}. I will WAIT, FOLLOW , be QUIET or ATTACK for you and tell you WHAT TIME. You may also command me to TRANSFER my experience to you, though I'd rather you didn't.\n", cn_name));
        } else {
            gs.do_sayx(cc, &format!("Thank you for creating me, {}!\n", cn_name));
        }
    }

    gs.do_update_char(cc);

    add_exhaust(gs, cn, TICKS * 4);

    let (cc_x, cc_y) = (gs.characters[cc].x as i32, gs.characters[cc].y as i32);
    EffectManager::fx_add_effect(gs, 6, 0, cc_x, cc_y, 0);
    let (cn_x, cn_y) = (gs.characters[cn].x as i32, gs.characters[cn].y as i32);
    EffectManager::fx_add_effect(gs, 7, 0, cn_x, cn_y, 0);
}

pub fn is_facing(gs: &GameState, cn: usize, co: usize) -> i32 {
    let dir = gs.characters[cn].dir;
    let cx = gs.characters[cn].x;
    let cy = gs.characters[cn].y;
    let ox = gs.characters[co].x;
    let oy = gs.characters[co].y;

    match dir {
        DX_RIGHT => {
            if cx + 1 == ox && cy == oy {
                1
            } else {
                0
            }
        }
        DX_LEFT => {
            if cx - 1 == ox && cy == oy {
                1
            } else {
                0
            }
        }
        DX_UP => {
            if cx == ox && cy - 1 == oy {
                1
            } else {
                0
            }
        }
        DX_DOWN => {
            if cx == ox && cy + 1 == oy {
                1
            } else {
                0
            }
        }
        _ => 0,
    }
}

pub fn is_back(gs: &GameState, cn: usize, co: usize) -> i32 {
    let dir = gs.characters[cn].dir;
    let cx = gs.characters[cn].x;
    let cy = gs.characters[cn].y;
    let ox = gs.characters[co].x;
    let oy = gs.characters[co].y;

    match dir {
        DX_LEFT => {
            if cx + 1 == ox && cy == oy {
                1
            } else {
                0
            }
        }
        DX_RIGHT => {
            if cx - 1 == ox && cy == oy {
                1
            } else {
                0
            }
        }
        DX_DOWN => {
            if cx == ox && cy - 1 == oy {
                1
            } else {
                0
            }
        }
        DX_UP => {
            if cx == ox && cy + 1 == oy {
                1
            } else {
                0
            }
        }
        _ => 0,
    }
}

pub fn nomagic(gs: &mut GameState, cn: usize) {
    gs.do_character_log(
        cn,
        FontColor::Green,
        "Your magic fails. You seem to be unable to cast spells.\n",
    );
}

pub fn skill_lookup(name: &str) -> i32 {
    // Full implementation ported from original C++ skill_lookup
    let name = name.trim();
    if name.is_empty() {
        return -1;
    }
    if name == "0" {
        return 0;
    }

    // Try numeric
    if let Ok(n) = name.parse::<i32>() {
        if n >= 0 && (n as usize) < SKILL_NAMES.len() {
            if n > 0 {
                return n;
            }
        } else {
            return -1;
        }
    }

    // Determine the number of meaningful skills (stop at first empty name)
    let max = SKILL_NAMES
        .iter()
        .position(|s| s.is_empty())
        .unwrap_or(SKILL_NAMES.len());

    // Try tolerant alpha matching: succeed when input matches prefix of skill name
    for (j, &skill) in SKILL_NAMES.iter().enumerate().take(max) {
        let mut name_iter = name.chars().map(|c| c.to_ascii_lowercase());
        let mut skill_iter = skill.chars().map(|c| c.to_ascii_lowercase());
        let mut matched = true;

        loop {
            match (name_iter.next(), skill_iter.next()) {
                (Some(pc), Some(sc)) => {
                    if sc == ' ' {
                        break; // skill name reached a space -> accept match
                    }
                    if pc != sc {
                        matched = false;
                        break;
                    }
                }
                (Some(_), None) | (None, Some(_)) | (None, None) => {
                    // either string ended -> accept if no mismatch so far
                    break;
                }
            }
        }

        if matched {
            return j as i32;
        }
    }

    -1
}

pub fn skill_driver(gs: &mut GameState, cn: usize, nr: i32) {
    // Check whether the character can use this skill/spell
    if gs.characters[cn].skill[nr as usize][0] == 0 {
        gs.do_character_log(cn, FontColor::Green, "You cannot use this skill/spell.\n");
        return;
    }

    match nr {
        x if x == SK_LIGHT as i32 => {
            if (gs.characters[cn].flags & CharacterFlags::NoMagic.bits()) != 0 {
                nomagic(gs, cn)
            } else {
                skill_light(gs, cn)
            }
        }
        x if x == SK_PROTECT as i32 => {
            if (gs.characters[cn].flags & CharacterFlags::NoMagic.bits()) != 0 {
                nomagic(gs, cn)
            } else {
                skill_protect(gs, cn)
            }
        }
        x if x == SK_ENHANCE as i32 => {
            if (gs.characters[cn].flags & CharacterFlags::NoMagic.bits()) != 0 {
                nomagic(gs, cn)
            } else {
                skill_enhance(gs, cn)
            }
        }
        x if x == SK_BLESS as i32 => {
            if (gs.characters[cn].flags & CharacterFlags::NoMagic.bits()) != 0 {
                nomagic(gs, cn)
            } else {
                skill_bless(gs, cn)
            }
        }
        x if x == SK_CURSE as i32 => {
            if (gs.characters[cn].flags & CharacterFlags::NoMagic.bits()) != 0 {
                nomagic(gs, cn)
            } else {
                skill_curse(gs, cn)
            }
        }
        x if x == SK_IDENT as i32 => {
            if (gs.characters[cn].flags & CharacterFlags::NoMagic.bits()) != 0 {
                nomagic(gs, cn)
            } else {
                skill_identify(gs, cn)
            }
        }
        x if x == SK_BLAST as i32 => {
            if (gs.characters[cn].flags & CharacterFlags::NoMagic.bits()) != 0 {
                nomagic(gs, cn)
            } else {
                skill_blast(gs, cn)
            }
        }
        x if x == SK_REPAIR as i32 => skill_repair(gs, cn),
        x if x == SK_LOCK as i32 => gs.do_character_log(
            cn,
            FontColor::Green,
            "You cannot use this skill directly. Hold a lock-pick under your mouse cursor and click on the door.\n",
        ),
        x if x == SK_RECALL as i32 => {
            if (gs.characters[cn].flags & CharacterFlags::NoMagic.bits()) != 0 {
                nomagic(gs, cn)
            } else {
                skill_recall(gs, cn)
            }
        }
        x if x == SK_STUN as i32 => {
            if (gs.characters[cn].flags & CharacterFlags::NoMagic.bits()) != 0 {
                nomagic(gs, cn)
            } else {
                skill_stun(gs, cn)
            }
        }
        x if x == SK_DISPEL as i32 => {
            if (gs.characters[cn].flags & CharacterFlags::NoMagic.bits()) != 0 {
                nomagic(gs, cn)
            } else {
                skill_dispel(gs, cn)
            }
        }
        x if x == SK_WIMPY as i32 => {
            if (gs.characters[cn].flags & CharacterFlags::NoMagic.bits()) != 0 {
                nomagic(gs, cn)
            } else {
                skill_wimp(gs, cn)
            }
        }
        x if x == SK_HEAL as i32 => {
            if (gs.characters[cn].flags & CharacterFlags::NoMagic.bits()) != 0 {
                nomagic(gs, cn)
            } else {
                skill_heal(gs, cn)
            }
        }
        x if x == SK_GHOST as i32 => {
            if (gs.characters[cn].flags & CharacterFlags::NoMagic.bits()) != 0 {
                nomagic(gs, cn)
            } else {
                skill_ghost(gs, cn)
            }
        }
        x if x == SK_MSHIELD as i32 => {
            if (gs.characters[cn].flags & CharacterFlags::NoMagic.bits()) != 0 {
                nomagic(gs, cn)
            } else {
                skill_mshield(gs, cn)
            }
        }
        x if x == SK_IMMUN as i32 => gs.do_character_log(
            cn,
            FontColor::Green,
            "You use this skill automatically when someone casts evil spells on you.\n",
        ),
        x if x == SK_REGEN as i32 || x == SK_REST as i32 || x == SK_MEDIT as i32 => {
            gs.do_character_log(
                cn,
                FontColor::Green,
                "You use this skill automatically when you stand still.\n",
            );
        }
        x if x == SK_DAGGER as i32
            || x == SK_SWORD as i32
            || x == SK_AXE as i32
            || x == SK_STAFF as i32
            || x == SK_TWOHAND as i32
            || x == SK_SURROUND as i32 =>
        {
            gs.do_character_log(
                cn,
                FontColor::Green,
                "You use this skill automatically when you fight.\n",
            );
        }
        x if x == SK_CONCEN as i32 => gs.do_character_log(
            cn,
            FontColor::Green,
            "You use this skill automatically when you cast spells.\n",
        ),
        x if x == SK_WARCRY as i32 => skill_warcry(gs, cn),
        _ => {
            gs.do_character_log(cn, FontColor::Green, "You cannot use this skill/spell.\n");
        }
    }
}
