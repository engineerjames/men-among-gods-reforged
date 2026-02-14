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
    chlog, core::types::Character, driver, effect::EffectManager, god::God, helpers, populate,
    repository::Repository, state::State,
};

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

pub fn player_or_ghost(cn: usize, co: usize) -> i32 {
    // Rust port of C++ player_or_ghost
    let cn_flags = Repository::with_characters(|ch| ch[cn].flags);
    if (cn_flags & CharacterFlags::Player.bits()) == 0 {
        return 0;
    }
    let co_flags = Repository::with_characters(|ch| ch[co].flags);
    if (co_flags & CharacterFlags::Player.bits()) != 0 {
        return 1;
    }
    let co_data_63 = Repository::with_characters(|ch| ch[co].data[63] as usize);
    if co_data_63 == cn {
        return 1;
    }
    0
}
pub fn spellcost(cn: usize, cost: i32) -> i32 {
    // Ported from C++ spellcost(int cn, int cost)
    // concentrate:
    let mut cost = cost;
    let concen_skill =
        Repository::with_characters(|ch| ch[cn].skill[core::constants::SK_CONCEN][0]);
    if concen_skill != 0 {
        let concen_val =
            Repository::with_characters(|ch| ch[cn].skill[core::constants::SK_CONCEN][5]);
        let t = cost * concen_val as i32 / 300;
        if t > cost {
            cost = 1;
        } else {
            cost -= t;
        }
    }
    let a_mana = Repository::with_characters(|ch| ch[cn].a_mana);
    if cost * 1000 > a_mana {
        State::with(|state| {
            state.do_character_log(
                cn,
                core::types::FontColor::Red,
                "You don't have enough mana.\n",
            );
        });
        return -1;
    }
    Repository::with_characters_mut(|ch| ch[cn].a_mana = a_mana - cost * 1000);
    0
}

pub fn chance_base(cn: usize, skill: i32, d20: i32, power: i32) -> i32 {
    // Ported from C++ chance_base(int cn, int skill, int d20, int power)
    let mut chance = d20 * skill / std::cmp::max(1, power);
    let (flags, luck) = Repository::with_characters(|ch| (ch[cn].flags, ch[cn].luck));
    if (flags & CharacterFlags::Player.bits()) != 0 {
        if luck < 0 {
            chance += luck / 500 - 1;
        }
    }

    chance = chance.clamp(0, 18);

    let roll = crate::helpers::random_mod(20);
    if roll as i32 > chance || power > skill + (skill / 2) {
        State::with(|state| {
            state.do_character_log(cn, core::types::FontColor::Red, "You lost your focus!\n");
        });
        return -1;
    }
    0
}
pub fn chance(cn: usize, d20: i32) -> i32 {
    // Ported from C++ chance(int cn, int d20)
    let mut d20 = d20;
    let (flags, luck) = Repository::with_characters(|ch| (ch[cn].flags, ch[cn].luck));
    if (flags & CharacterFlags::Player.bits()) != 0 {
        if luck < 0 {
            d20 += luck / 500 - 1;
        }
    }

    d20 = d20.clamp(0, 18);

    let roll = crate::helpers::random_mod(20);
    if roll as i32 > d20 {
        State::with(|state| {
            state.do_character_log(cn, core::types::FontColor::Red, "You lost your focus!\n");
        });
        return -1;
    }
    0
}
pub fn spell_immunity(power: i32, immun: i32) -> i32 {
    // Ported from C++ spell_immunity(int power, int immun)
    if power <= immun {
        1
    } else {
        power - immun
    }
}
pub fn spell_race_mod(power: i32, kindred: i32) -> i32 {
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

    Repository::with_globals(|globs| {
        if globs.newmoon != 0 {
            modf *= 0.9;
        }
        if globs.fullmoon != 0 {
            modf *= 1.1;
        }
    });

    (power as f64 * modf) as i32
}

pub fn add_spell(cn: usize, in_: usize) -> i32 {
    // Ported from C++ add_spell(int cn, int in)
    let mut n = 0;
    let in2: usize;
    let mut weak = 999;
    let mut weakest = 99;
    let mut rejected = false;
    let m = Repository::with_characters(|ch| {
        ch[cn].x as usize + ch[cn].y as usize * core::constants::SERVER_MAPX as usize
    });
    let nomagic = Repository::with_map(|map| map[m].flags & CharacterFlags::NoMagic.bits() != 0);
    if nomagic {
        return 0;
    }
    // Overwrite spells if same spell is cast twice and the new spell is more powerful
    let mut found = false;
    Repository::with_characters_mut(|ch| {
        for i in 0..20 {
            if ch[cn].spell[i] != 0 {
                let it_in2 = ch[cn].spell[i] as usize;
                let temp_in2 = Repository::with_items(|it| it[it_in2].temp);
                let temp_in = Repository::with_items(|it| it[in_].temp);
                if temp_in2 == temp_in {
                    let power_in = Repository::with_items(|it| it[in_].power);
                    let power_in2 = Repository::with_items(|it| it[it_in2].power);
                    let active_in2 = Repository::with_items(|it| it[it_in2].active);
                    if power_in < power_in2 && active_in2 > core::constants::TICKS as u32 * 60 {
                        Repository::with_items_mut(|it| it[in_].used = core::constants::USE_EMPTY);
                        rejected = true;
                        found = true;
                        return;
                    }
                    Repository::with_items_mut(|it| it[it_in2].used = core::constants::USE_EMPTY);
                    n = i;
                    found = true;
                    break;
                }
            }
        }
    });
    if rejected {
        return 0;
    }
    if found {
        // n is set by the loop above
    } else {
        // Find empty slot or weakest spell
        let mut empty_found = false;
        Repository::with_characters_mut(|ch| {
            for i in 0..20 {
                if ch[cn].spell[i] == 0 {
                    n = i;
                    empty_found = true;
                    break;
                }
                let it_in2 = ch[cn].spell[i] as usize;
                let power_in2 = Repository::with_items(|it| it[it_in2].power);
                if power_in2 < weak {
                    weak = power_in2;
                    weakest = i;
                }
            }
        });
        if !empty_found {
            let power_in = Repository::with_items(|it| it[in_].power);
            if weak < 999 && weak < power_in {
                n = weakest;
                in2 = Repository::with_characters(|ch| ch[cn].spell[n] as usize);
                Repository::with_items_mut(|it| it[in2].used = core::constants::USE_EMPTY);
            } else {
                Repository::with_items_mut(|it| it[in_].used = core::constants::USE_EMPTY);
                return 0;
            }
        }
    }
    // Assign spell
    Repository::with_characters_mut(|ch| ch[cn].spell[n] = in_ as u32);
    Repository::with_items_mut(|it| it[in_].carried = cn as u16);
    State::with(|state| state.do_update_char(cn));
    1
}

pub fn is_exhausted(cn: usize) -> i32 {
    // Ported from C++ is_exhausted(int cn)
    for n in 0..20 {
        let in_ = Repository::with_characters(|ch| ch[cn].spell[n] as usize);
        if in_ != 0 {
            let temp = Repository::with_items(|it| it[in_].temp);
            if temp == core::constants::SK_BLAST as u16 {
                State::with(|state| {
                    state.do_character_log(
                        cn,
                        core::types::FontColor::Red,
                        "You are still exhausted from your last spell!\n",
                    );
                });
                return 1;
            }
        }
    }
    0
}

pub fn add_exhaust(cn: usize, exhaust_length: i32) {
    // Ported from C++ add_exhaust(int cn, int len)
    let in_ = God::create_item(1);
    if in_.is_none() {
        log::error!("god_create_item failed in add_exhaust");
        return;
    }
    Repository::with_items_mut(|it| {
        let item = &mut it[in_.unwrap()];
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
    });
    add_spell(cn, in_.unwrap());
}

pub fn spell_from_item(cn: usize, in2: usize) {
    // Ported from C++ spell_from_item(int cn, int in2)
    let flags = Repository::with_characters(|ch| ch[cn].flags);
    if (flags & CharacterFlags::NoMagic.bits()) != 0 {
        State::with(|state| {
            state.do_character_log(
                cn,
                core::types::FontColor::Red,
                "The magic didn't work! Must be external influences.\n",
            );
        });
        return;
    }
    let in_ = God::create_item(1);
    if in_.is_none() {
        log::error!("god_create_item failed in skill_from_item");
        return;
    }

    let in_ = in_.unwrap();
    Repository::with_items_mut(|it| {
        it[in_].name = Repository::with_items(|it2| it2[in2].name);
        it[in_].flags |= ItemFlags::IF_SPELL.bits();
        it[in_].armor[1] = Repository::with_items(|it2| it2[in2].armor[1]);
        it[in_].weapon[1] = Repository::with_items(|it2| it2[in2].weapon[1]);
        it[in_].hp[1] = Repository::with_items(|it2| it2[in2].hp[1]);
        it[in_].end[1] = Repository::with_items(|it2| it2[in2].end[1]);
        it[in_].mana[1] = Repository::with_items(|it2| it2[in2].mana[1]);
        it[in_].sprite_override = Repository::with_items(|it2| it2[in2].sprite_override);
        for n in 0..5 {
            it[in_].attrib[n][1] = Repository::with_items(|it2| it2[in2].attrib[n][1]);
        }
        for n in 0..50 {
            it[in_].skill[n][1] = Repository::with_items(|it2| it2[in2].skill[n][1]);
        }
        let data0 = Repository::with_items(|it2| it2[in2].data[0]);
        if data0 != 0 {
            it[in_].sprite[1] = data0 as i16;
        } else {
            it[in_].sprite[1] = 93;
        }
        let duration = Repository::with_items(|it2| it2[in2].duration);
        it[in_].duration = duration;
        it[in_].active = duration;
        let data1 = Repository::with_items(|it2| it2[in2].data[1]);
        if data1 != 0 {
            it[in_].temp = data1 as u16;
        } else {
            it[in_].temp = 101;
        }
        it[in_].power = Repository::with_items(|it2| it2[in2].power);
    });
    if add_spell(cn, in_) == 0 {
        let name = Repository::with_items(|it| it[in_].get_name().to_string());
        State::with(|state| {
            state.do_character_log(
                cn,
                core::types::FontColor::Green,
                &format!("Magical interference neutralised the {}'s effect.\n", name,),
            );
        });
        return;
    }
    State::with(|state| {
        state.do_character_log(cn, core::types::FontColor::Green, "You feel changed.\n");
        let sound = Repository::with_characters(|ch| ch[cn].sound);
        State::char_play_sound(cn, sound as i32 + 1, -150, 0);
    });
}

pub fn spell_light(cn: usize, co: usize, power: i32) -> i32 {
    // Ported from C++ spell_light(int cn, int co, int power)
    let in_ = God::create_item(1);
    if in_.is_none() {
        log::error!("god_create_item failed in spell_light");
        return 0;
    }
    let power = spell_race_mod(power, Repository::with_characters(|ch| ch[cn].kindred));
    Repository::with_items_mut(|it| {
        let mut name_bytes = [0u8; 40];
        let name = b"Light";
        let len = name.len().min(40);
        name_bytes[..len].copy_from_slice(&name[..len]);
        it[in_.unwrap()].name = name_bytes;
        it[in_.unwrap()].flags |= ItemFlags::IF_SPELL.bits();
        it[in_.unwrap()].light[1] = std::cmp::min(250, power * 4) as i16;
        it[in_.unwrap()].sprite[1] = 85;
        it[in_.unwrap()].duration = 18 * 60 * 30;
        it[in_.unwrap()].active = 18 * 60 * 30;
        it[in_.unwrap()].temp = core::constants::SK_LIGHT as u16;
        it[in_.unwrap()].power = power as u32;
    });
    if cn != co {
        if add_spell(co, in_.unwrap()) == 0 {
            let name = Repository::with_items(|it| it[in_.unwrap()].get_name().to_string());
            State::with(|state| {
                state.do_character_log(
                    cn,
                    core::types::FontColor::Green,
                    &format!("Magical interference neutralised the {}'s effect.\n", name),
                );
            });
            return 0;
        }
        let sense = Repository::with_characters(|ch| ch[co].skill[core::constants::SK_SENSE][5]);
        if sense + 10 > power as u8 {
            let reference = Repository::with_characters(|ch| ch[cn].reference);
            State::with(|state| {
                state.do_character_log(
                    co,
                    core::types::FontColor::Green,
                    &format!("{} cast light on you.\n", c_string_to_str(&reference)),
                )
            });
        } else {
            State::with(|state| {
                state.do_character_log(
                    co,
                    core::types::FontColor::Green,
                    "You start to emit light.\n",
                )
            });
        }
        let name = Repository::with_characters(|ch| ch[co].name);
        let (x, y) = Repository::with_characters(|ch| (ch[co].x, ch[co].y));
        State::with(|state| {
            state.do_area_log(
                co,
                0,
                x as i32,
                y as i32,
                core::types::FontColor::Green,
                &format!("{} starts to emit light.\n", c_string_to_str(&name)),
            )
        });
        let sound = Repository::with_characters(|ch| ch[cn].sound);
        State::char_play_sound(co, sound as i32 + 1, -150, 0);
        State::char_play_sound(cn, sound as i32 + 1, -150, 0);
        let (x, y) = Repository::with_characters(|ch| (ch[co].x, ch[co].y));
        EffectManager::fx_add_effect(7, 0, x as i32, y as i32, 0);
    } else {
        if add_spell(cn, in_.unwrap()) == 0 {
            let name = Repository::with_items(|it| it[in_.unwrap()].get_name().to_string());
            State::with(|state| {
                state.do_character_log(
                    cn,
                    core::types::FontColor::Green,
                    &format!("Magical interference neutralised the {}'s effect.\n", name),
                );
            });
            return 0;
        }
        State::with(|state| {
            state.do_character_log(
                cn,
                core::types::FontColor::Green,
                "You start to emit light.\n",
            )
        });
        let sound = Repository::with_characters(|ch| ch[cn].sound);
        State::char_play_sound(cn, sound as i32 + 1, -150, 0);
        let flags = Repository::with_characters(|ch| ch[cn].flags);
        if (flags & CharacterFlags::Player.bits()) != 0 {
            chlog!(cn, "Cast Light");
        }
        let (x, y) = Repository::with_characters(|ch| (ch[cn].x, ch[cn].y));
        EffectManager::fx_add_effect(7, 0, x as i32, y as i32, 0);
    }
    let (x, y) = Repository::with_characters(|ch| (ch[cn].x, ch[cn].y));
    EffectManager::fx_add_effect(7, 0, x as i32, y as i32, 0);
    1
}

pub fn skill_light(cn: usize) {
    // rate limit for player
    let is_player =
        Repository::with_characters(|ch| (ch[cn].flags & CharacterFlags::Player.bits()) != 0);
    if is_player {
        Repository::with_characters_mut(|ch| {
            ch[cn].data[71] += CNTSAY;
        });
        let over = Repository::with_characters(|ch| ch[cn].data[71] > MAXSAY);
        if over {
            State::with(|state| {
                state.do_character_log(cn, FontColor::Red, "Oops, you're a bit too fast for me!\n")
            });
            return;
        }
    }

    let co = Repository::with_characters(|ch| {
        if ch[cn].skill_target1 != 0 {
            ch[cn].skill_target1 as usize
        } else {
            cn
        }
    });

    if State::with_mut(|state| state.do_char_can_see(cn, co)) == 0 {
        State::with(|state| {
            state.do_character_log(cn, FontColor::Red, "You cannot see your target.\n")
        });
        return;
    }

    if is_exhausted(cn) != 0 {
        return;
    }

    if spellcost(cn, 5) != 0 {
        return;
    }

    if chance(cn, 18) != 0 {
        if cn != co {
            let sense = Repository::with_characters(|ch| ch[co].skill[SK_SENSE][5]);
            let light_skill = Repository::with_characters(|ch| ch[cn].skill[SK_LIGHT][5]);
            if sense > (light_skill + 5) {
                let reference = Repository::with_characters(|ch| ch[cn].reference);
                State::with(|state| {
                    state.do_character_log(
                        co,
                        FontColor::Green,
                        &format!(
                            "{} tried to cast light on you but failed.\n",
                            c_string_to_str(&reference)
                        ),
                    )
                });
            }
        }
        return;
    }

    let light_skill = Repository::with_characters(|ch| ch[cn].skill[SK_LIGHT][5]);
    spell_light(cn, co, light_skill as i32);

    add_exhaust(cn, TICKS / 4);
}

pub fn spellpower(cn: usize) -> i32 {
    Repository::with_characters(|ch| {
        let a = ch[cn].attrib[core::constants::AT_AGIL as usize][0] as i32;
        let b = ch[cn].attrib[core::constants::AT_STREN as usize][0] as i32;
        let c = ch[cn].attrib[core::constants::AT_INT as usize][0] as i32;
        let d = ch[cn].attrib[core::constants::AT_WILL as usize][0] as i32;
        let e = ch[cn].attrib[core::constants::AT_BRAVE as usize][0] as i32;
        a + b + c + d + e
    })
}

pub fn spell_protect(cn: usize, co: usize, power: i32) -> i32 {
    let in_opt = God::create_item(1);
    if in_opt.is_none() {
        log::error!("god_create_item failed in skill_protect");
        return 0;
    }
    let in_ = in_opt.unwrap();

    // cap power to target's spellpower
    let mut power = power;
    let target_spellpower = spellpower(co);
    if power > target_spellpower {
        if cn != co {
            let reference = Repository::with_characters(|ch| ch[co].reference);
            State::with(|state| {
                state.do_character_log(
                    cn,
                    FontColor::Yellow,
                    &format!(
                        "Seeing that {} is not powerful enough for your spell, you reduced its strength.\n",
                        c_string_to_str(&reference)
                    ),
                )
            });
        } else {
            State::with(|state| {
                state.do_character_log(
                    cn,
                    FontColor::Green,
                    "You are not powerful enough to use the full strength of this spell.\n",
                )
            });
        }
        power = target_spellpower;
    }

    let power = spell_race_mod(power, Repository::with_characters(|ch| ch[cn].kindred));

    Repository::with_items_mut(|it| {
        let mut name_bytes = [0u8; 40];
        let name = b"Protection";
        let len = name.len().min(40);
        name_bytes[..len].copy_from_slice(&name[..len]);
        it[in_].name = name_bytes;
        it[in_].flags |= ItemFlags::IF_SPELL.bits();
        it[in_].armor[1] = (power / 4 + 4) as i8;
        it[in_].sprite[1] = 86;
        it[in_].duration = 18 * 60 * 10;
        it[in_].active = 18 * 60 * 10;
        it[in_].temp = SK_PROTECT as u16;
        it[in_].power = power as u32;
    });

    if cn != co {
        if add_spell(co, in_) == 0 {
            let name = Repository::with_items(|it| it[in_].get_name().to_string());
            State::with(|state| {
                state.do_character_log(
                    cn,
                    FontColor::Green,
                    &format!("Magical interference neutralised the {}'s effect.\n", name),
                )
            });
            return 0;
        }

        let sense = Repository::with_characters(|ch| ch[co].skill[SK_SENSE][5]);
        if sense as i32 + 10 > power {
            let reference = Repository::with_characters(|ch| ch[cn].reference);
            State::with(|state| {
                state.do_character_log(
                    co,
                    FontColor::Green,
                    &format!("{} cast protect on you.\n", c_string_to_str(&reference)),
                )
            });
        } else {
            State::with(|state| {
                state.do_character_log(co, FontColor::Red, "You feel protected.\n")
            });
        }

        let name = Repository::with_characters(|ch| ch[co].get_name().to_string());
        State::with(|state| {
            state.do_character_log(
                cn,
                FontColor::Yellow,
                &format!("{} is now protected.\n", name),
            )
        });
        let sound = Repository::with_characters(|ch| ch[cn].sound);
        State::char_play_sound(co, sound as i32 + 1, -150, 0);
        State::char_play_sound(cn, sound as i32 + 1, -150, 0);
        let target_name = Repository::with_characters(|ch| ch[co].get_name().to_string());
        chlog!(cn, "Cast Protect on {}", target_name);
        EffectManager::fx_add_effect(
            6,
            0,
            Repository::with_characters(|ch| ch[co].x) as i32,
            Repository::with_characters(|ch| ch[co].y) as i32,
            0,
        );
    } else {
        if add_spell(cn, in_) == 0 {
            let name = Repository::with_items(|it| it[in_].get_name().to_string());
            State::with(|state| {
                state.do_character_log(
                    cn,
                    FontColor::Green,
                    &format!("Magical interference neutralised the {}'s effect.\n", name),
                )
            });
            return 0;
        }
        State::with(|state| state.do_character_log(cn, FontColor::Green, "You feel protected.\n"));
        let sound = Repository::with_characters(|ch| ch[cn].sound);
        State::char_play_sound(cn, sound as i32 + 1, -150, 0);
        let flags = Repository::with_characters(|ch| ch[cn].flags);
        if (flags & CharacterFlags::Player.bits()) != 0 {
            chlog!(cn, "Cast Protect");
        }
        let (x, y) = Repository::with_characters(|ch| (ch[cn].x, ch[cn].y));
        EffectManager::fx_add_effect(6, 0, x as i32, y as i32, 0);
    }

    EffectManager::fx_add_effect(
        7,
        0,
        Repository::with_characters(|ch| ch[cn].x) as i32,
        Repository::with_characters(|ch| ch[cn].y) as i32,
        0,
    );

    1
}

pub fn skill_protect(cn: usize) {
    let has_skill = Repository::with_characters(|ch| ch[cn].skill[SK_PROTECT][5] != 0);
    if !has_skill {
        return;
    }

    let mut co = Repository::with_characters(|ch| {
        if ch[cn].skill_target1 != 0 {
            ch[cn].skill_target1 as usize
        } else {
            cn
        }
    });

    if State::with_mut(|state| state.do_char_can_see(cn, co)) == 0 {
        State::with(|state| {
            state.do_character_log(cn, FontColor::Red, "You cannot see your target.\n")
        });
        return;
    }

    if is_exhausted(cn) != 0 {
        return;
    }

    if driver::player_or_ghost(cn, co) == 0 {
        let name_from = Repository::with_characters(|ch| ch[co].get_name().to_string());
        let name_to = Repository::with_characters(|ch| ch[cn].get_name().to_string());
        State::with(|state| {
            state.do_character_log(
                cn,
                FontColor::Red,
                &format!(
                    "Changed target of spell from {} to {}.\n",
                    name_from, name_to
                ),
            )
        });
        co = cn;
    }

    if spellcost(cn, 15) != 0 {
        return;
    }
    if chance(cn, 18) != 0 {
        if cn != co {
            let sense = Repository::with_characters(|ch| ch[co].skill[SK_SENSE][5]);
            let prot_skill = Repository::with_characters(|ch| ch[cn].skill[SK_PROTECT][5]);
            if sense > (prot_skill + 5) {
                let reference = Repository::with_characters(|ch| ch[cn].reference);
                State::with(|state| {
                    state.do_character_log(
                        co,
                        FontColor::Green,
                        &format!(
                            "{} tried to cast protect on you but failed.\n",
                            c_string_to_str(&reference)
                        ),
                    )
                });
            }
        }
        return;
    }

    let power = Repository::with_characters(|ch| ch[cn].skill[SK_PROTECT][5] as i32);
    spell_protect(cn, co, power);

    add_exhaust(cn, TICKS / 2);
}

pub fn spell_enhance(cn: usize, co: usize, power: i32) -> i32 {
    let in_opt = God::create_item(1);
    if in_opt.is_none() {
        log::error!("god_create_item failed in skill_enhance");
        return 0;
    }
    let in_ = in_opt.unwrap();

    // cap power to target's spellpower
    let mut power = power;
    let target_spellpower = spellpower(co);
    if power > target_spellpower {
        if cn != co {
            let reference = Repository::with_characters(|ch| ch[co].reference);
            State::with(|state| {
                state.do_character_log(cn, FontColor::Yellow, &format!("Seeing that {} is not powerful enough for your spell, you reduced its strength.\n", c_string_to_str(&reference)))
            });
        } else {
            State::with(|state| {
                state.do_character_log(
                    cn,
                    FontColor::Yellow,
                    "You are not powerful enough to use the full strength of this spell.\n",
                )
            });
        }
        power = target_spellpower;
    }

    let power = spell_race_mod(power, Repository::with_characters(|ch| ch[cn].kindred));

    Repository::with_items_mut(|it| {
        let mut name_bytes = [0u8; 40];
        let name = b"Enhance Weapon";
        let len = name.len().min(40);
        name_bytes[..len].copy_from_slice(&name[..len]);
        it[in_].name = name_bytes;
        it[in_].flags |= ItemFlags::IF_SPELL.bits();
        it[in_].weapon[1] = (power / 4 + 4) as i8;
        it[in_].sprite[1] = 87;
        it[in_].duration = 18 * 60 * 10;
        it[in_].active = 18 * 60 * 10;
        it[in_].temp = SK_ENHANCE as u16;
        it[in_].power = power as u32;
    });

    if cn != co {
        if add_spell(co, in_) == 0 {
            let name = Repository::with_items(|it| it[in_].get_name().to_string());
            State::with(|state| {
                state.do_character_log(
                    cn,
                    FontColor::Yellow,
                    &format!("Magical interference neutralised the {}'s effect.\n", name),
                )
            });
            return 0;
        }
        let sense = Repository::with_characters(|ch| ch[co].skill[SK_SENSE][5]);
        if sense as i32 + 10 > power {
            let reference = Repository::with_characters(|ch| ch[cn].reference);
            State::with(|state| {
                state.do_character_log(
                    co,
                    FontColor::Yellow,
                    &format!(
                        "{} cast enhance weapon on you.\n",
                        c_string_to_str(&reference)
                    ),
                )
            });
        } else {
            State::with(|state| {
                state.do_character_log(co, FontColor::Red, "Your weapon feels stronger.\n")
            });
        }
        State::with(|state| {
            state.do_character_log(
                cn,
                FontColor::Yellow,
                &format!(
                    "{}'s weapon is now stronger.\n",
                    Repository::with_characters(|ch| ch[co].get_name().to_string())
                ),
            )
        });
        let sound = Repository::with_characters(|ch| ch[cn].sound);
        State::char_play_sound(co, sound as i32 + 1, -150, 0);
        State::char_play_sound(cn, sound as i32 + 1, -150, 0);
        let target_name = Repository::with_characters(|ch| ch[co].get_name().to_string());
        chlog!(cn, "Cast Enhance on {}", target_name);

        EffectManager::fx_add_effect(
            6,
            0,
            Repository::with_characters(|ch| ch[co].x) as i32,
            Repository::with_characters(|ch| ch[co].y) as i32,
            0,
        );
    } else {
        if add_spell(cn, in_) == 0 {
            let name = Repository::with_items(|it| it[in_].get_name().to_string());
            State::with(|state| {
                state.do_character_log(
                    cn,
                    FontColor::Yellow,
                    &format!("Magical interference neutralised the {}'s effect.\n", name),
                )
            });
            return 0;
        }
        State::with(|state| {
            state.do_character_log(cn, FontColor::Green, "Your weapon feels stronger.\n")
        });
        let sound = Repository::with_characters(|ch| ch[cn].sound);
        State::char_play_sound(cn, sound as i32 + 1, -150, 0);
        let flags = Repository::with_characters(|ch| ch[cn].flags);
        if (flags & CharacterFlags::Player.bits()) != 0 {
            chlog!(cn, "Cast Enhance");
        }
        EffectManager::fx_add_effect(
            6,
            0,
            Repository::with_characters(|ch| ch[cn].x) as i32,
            Repository::with_characters(|ch| ch[cn].y) as i32,
            0,
        );
    }

    EffectManager::fx_add_effect(
        7,
        0,
        Repository::with_characters(|ch| ch[cn].x) as i32,
        Repository::with_characters(|ch| ch[cn].y) as i32,
        0,
    );

    1
}

pub fn skill_enhance(cn: usize) {
    let co = Repository::with_characters(|ch| {
        if ch[cn].skill_target1 != 0 {
            ch[cn].skill_target1 as usize
        } else {
            cn
        }
    });

    if State::with_mut(|state| state.do_char_can_see(cn, co)) == 0 {
        State::with(|state| {
            state.do_character_log(cn, FontColor::Red, "You cannot see your target.\n")
        });
        return;
    }

    if is_exhausted(cn) != 0 {
        return;
    }

    if driver::player_or_ghost(cn, co) == 0 {
        let name_from = Repository::with_characters(|ch| ch[co].get_name().to_string());
        let name_to = Repository::with_characters(|ch| ch[cn].get_name().to_string());
        State::with(|state| {
            state.do_character_log(
                cn,
                FontColor::Red,
                &format!(
                    "Changed target of spell from {} to {}.\n",
                    name_from, name_to
                ),
            )
        });
        // change target to self
        let co = cn;
        // continue with self
        if spellcost(cn, 15) != 0 {
            return;
        }
        if chance(cn, 18) != 0 {
            if cn != co {
                let sense = Repository::with_characters(|ch| ch[co].skill[SK_SENSE][5]);
                let enh_skill = Repository::with_characters(|ch| ch[cn].skill[SK_ENHANCE][5]);
                if sense > (enh_skill + 5) {
                    let reference = Repository::with_characters(|ch| ch[cn].reference);
                    State::with(|state| {
                        state.do_character_log(
                            co,
                            FontColor::Yellow,
                            &format!(
                                "{} tried to cast enhance weapon on you but failed.\n",
                                c_string_to_str(&reference)
                            ),
                        )
                    });
                }
            }
            return;
        }
        let power = Repository::with_characters(|ch| ch[cn].skill[SK_ENHANCE][5] as i32);
        spell_enhance(cn, co, power);
        add_exhaust(cn, TICKS / 2);
        return;
    }

    if spellcost(cn, 15) != 0 {
        return;
    }
    if chance(cn, 18) != 0 {
        if cn != co {
            let sense = Repository::with_characters(|ch| ch[co].skill[SK_SENSE][5]);
            let enh_skill = Repository::with_characters(|ch| ch[cn].skill[SK_ENHANCE][5]);
            if sense > (enh_skill + 5) {
                let reference = Repository::with_characters(|ch| ch[cn].reference);
                State::with(|state| {
                    state.do_character_log(
                        co,
                        FontColor::Yellow,
                        &format!(
                            "{} tried to cast enhance weapon on you but failed.\n",
                            c_string_to_str(&reference)
                        ),
                    )
                });
            }
        }
        return;
    }

    let power = Repository::with_characters(|ch| ch[cn].skill[SK_ENHANCE][5] as i32);
    spell_enhance(cn, co, power);
    add_exhaust(cn, TICKS / 2);
}

pub fn spell_bless(cn: usize, co: usize, power: i32) -> i32 {
    let in_opt = God::create_item(1);
    if in_opt.is_none() {
        log::error!("god_create_item failed in skill_bless");
        return 0;
    }
    let in_ = in_opt.unwrap();

    let mut power = power;
    let tmp = spellpower(co);
    if power > tmp {
        if cn != co {
            let reference = Repository::with_characters(|ch| ch[co].reference);
            State::with(|state| {
                state.do_character_log(cn, FontColor::Yellow, &format!("Seeing that {} is not powerful enough for your spell, you reduced its strength.\n", c_string_to_str(&reference)))
            });
        } else {
            State::with(|state| {
                state.do_character_log(
                    cn,
                    FontColor::Yellow,
                    "You are not powerful enough to use the full strength of this spell.\n",
                )
            });
        }
        power = tmp;
    }

    let power = spell_race_mod(power, Repository::with_characters(|ch| ch[cn].kindred));

    Repository::with_items_mut(|it| {
        let mut name_bytes = [0u8; 40];
        let name = b"Bless";
        let len = name.len().min(40);
        name_bytes[..len].copy_from_slice(&name[..len]);
        it[in_].name = name_bytes;
        it[in_].flags |= ItemFlags::IF_SPELL.bits();
        for n in 0..5 {
            it[in_].attrib[n][1] = (power / 5 + 3) as i8;
        }
        it[in_].sprite[1] = 88;
        it[in_].duration = 18 * 60 * 10;
        it[in_].active = 18 * 60 * 10;
        it[in_].temp = SK_BLESS as u16;
        it[in_].power = power as u32;
    });

    if cn != co {
        if add_spell(co, in_) == 0 {
            let name = Repository::with_items(|it| it[in_].get_name().to_string());
            State::with(|state| {
                state.do_character_log(
                    cn,
                    FontColor::Yellow,
                    &format!("Magical interference neutralised the {}'s effect.\n", name),
                )
            });
            return 0;
        }
        let sense = Repository::with_characters(|ch| ch[co].skill[SK_SENSE][5]);
        if sense as i32 + 10 > power {
            let reference = Repository::with_characters(|ch| ch[cn].reference);
            State::with(|state| {
                state.do_character_log(
                    co,
                    FontColor::Yellow,
                    &format!("{} cast bless on you.\n", c_string_to_str(&reference)),
                )
            });
        } else {
            State::with(|state| {
                state.do_character_log(co, FontColor::Red, "You have been blessed.\n")
            });
        }
        State::with(|state| {
            state.do_character_log(
                cn,
                FontColor::Yellow,
                &format!(
                    "{} was blessed.\n",
                    Repository::with_characters(|ch| ch[co].get_name().to_string())
                ),
            )
        });
        let sound = Repository::with_characters(|ch| ch[cn].sound);
        State::char_play_sound(co, sound as i32 + 1, -150, 0);
        State::char_play_sound(cn, sound as i32 + 1, -150, 0);
        chlog!(
            cn,
            "Cast Bless on {}",
            Repository::with_characters(|ch| ch[co].get_name().to_string())
        );
        EffectManager::fx_add_effect(
            6,
            0,
            Repository::with_characters(|ch| ch[co].x) as i32,
            Repository::with_characters(|ch| ch[co].y) as i32,
            0,
        );
    } else {
        if add_spell(cn, in_) == 0 {
            let name = Repository::with_items(|it| it[in_].get_name().to_string());
            State::with(|state| {
                state.do_character_log(
                    cn,
                    FontColor::Yellow,
                    &format!("Magical interference neutralised the {}'s effect.\n", name),
                )
            });
            return 0;
        }
        State::with(|state| {
            state.do_character_log(cn, FontColor::Green, "You have been blessed.\n")
        });
        let sound = Repository::with_characters(|ch| ch[cn].sound);
        State::char_play_sound(cn, sound as i32 + 1, -150, 0);
        let flags = Repository::with_characters(|ch| ch[cn].flags);
        if (flags & CharacterFlags::Player.bits()) != 0 {
            chlog!(cn, "Cast Bless");
        }
        EffectManager::fx_add_effect(
            6,
            0,
            Repository::with_characters(|ch| ch[cn].x) as i32,
            Repository::with_characters(|ch| ch[cn].y) as i32,
            0,
        );
    }

    EffectManager::fx_add_effect(
        7,
        0,
        Repository::with_characters(|ch| ch[cn].x) as i32,
        Repository::with_characters(|ch| ch[cn].y) as i32,
        0,
    );

    1
}

pub fn skill_bless(cn: usize) {
    let co = Repository::with_characters(|ch| {
        if ch[cn].skill_target1 != 0 {
            ch[cn].skill_target1 as usize
        } else {
            cn
        }
    });

    if State::with_mut(|state| state.do_char_can_see(cn, co)) == 0 {
        State::with(|state| {
            state.do_character_log(cn, FontColor::Red, "You cannot see your target.\n")
        });
        return;
    }

    if is_exhausted(cn) != 0 {
        return;
    }

    if driver::player_or_ghost(cn, co) == 0 {
        let name_from = Repository::with_characters(|ch| ch[co].get_name().to_string());
        let name_to = Repository::with_characters(|ch| ch[cn].get_name().to_string());
        State::with(|state| {
            state.do_character_log(
                cn,
                FontColor::Red,
                &format!(
                    "Changed target of spell from {} to {}.\n",
                    name_from, name_to
                ),
            )
        });
        // change target to self
        let co = cn;
        if spellcost(cn, 35) != 0 {
            return;
        }
        if chance(cn, 18) != 0 {
            if cn != co {
                let sense = Repository::with_characters(|ch| ch[co].skill[SK_SENSE][5]);
                let bless_skill = Repository::with_characters(|ch| ch[cn].skill[SK_BLESS][5]);
                if sense > (bless_skill + 5) {
                    let reference = Repository::with_characters(|ch| ch[cn].reference);
                    State::with(|state| {
                        state.do_character_log(
                            co,
                            FontColor::Yellow,
                            &format!(
                                "{} tried to cast bless on you but failed.\n",
                                c_string_to_str(&reference)
                            ),
                        )
                    });
                }
            }
            return;
        }
        spell_bless(
            cn,
            co,
            Repository::with_characters(|ch| ch[cn].skill[SK_BLESS][5] as i32),
        );
        add_exhaust(cn, TICKS);
        return;
    }

    if spellcost(cn, 35) != 0 {
        return;
    }
    if chance(cn, 18) != 0 {
        if cn != co {
            let sense = Repository::with_characters(|ch| ch[co].skill[SK_SENSE][5]);
            let bless_skill = Repository::with_characters(|ch| ch[cn].skill[SK_BLESS][5]);
            if sense > (bless_skill + 5) {
                let reference = Repository::with_characters(|ch| ch[cn].reference);
                State::with(|state| {
                    state.do_character_log(
                        co,
                        FontColor::Yellow,
                        &format!(
                            "{} tried to cast bless on you but failed.\n",
                            c_string_to_str(&reference)
                        ),
                    )
                });
            }
        }
        return;
    }

    spell_bless(
        cn,
        co,
        Repository::with_characters(|ch| ch[cn].skill[SK_BLESS][5] as i32),
    );
    add_exhaust(cn, TICKS);
}

pub fn skill_wimp(cn: usize) {
    // If Guardian Angel already active, remove it
    for n in 0..20 {
        let in_idx = Repository::with_characters(|ch| ch[cn].spell[n]);
        if in_idx != 0 {
            let temp = Repository::with_items(|it| it[in_idx as usize].temp);
            if temp == SK_WIMPY as u16 {
                State::with(|state| {
                    state.do_character_log(
                        cn,
                        core::types::FontColor::Green,
                        "Guardian Angel no longer active.\n",
                    )
                });
                Repository::with_items_mut(|it| {
                    it[in_idx as usize].used = core::constants::USE_EMPTY;
                });
                Repository::with_characters_mut(|ch| {
                    ch[cn].spell[n] = 0;
                });
                State::with(|state| state.do_update_char(cn));
                chlog!(cn, "Dismissed Guardian Angel");
                return;
            }
        }
    }

    let a_end = Repository::with_characters(|ch| ch[cn].a_end);
    if a_end < 20000 {
        State::with(|state| {
            state.do_character_log(
                cn,
                core::types::FontColor::Red,
                "You're too exhausted to call on your Guardian Angel.\n",
            )
        });
        return;
    }

    Repository::with_characters_mut(|ch| ch[cn].a_end -= 20000);

    let in_opt = God::create_item(1);
    if in_opt.is_none() {
        log::error!("god_create_item failed in skill_wimp");
        return;
    }
    let in_idx = in_opt.unwrap();

    Repository::with_items_mut(|it| {
        let mut name_bytes = [0u8; 40];
        let name = b"Guardian Angel";
        let len = name.len().min(40);
        name_bytes[..len].copy_from_slice(&name[..len]);
        it[in_idx].name = name_bytes;
        it[in_idx].flags |= ItemFlags::IF_SPELL.bits() | ItemFlags::IF_PERMSPELL.bits();
        it[in_idx].hp[0] = -1;
        it[in_idx].end[0] = -1;
        it[in_idx].mana[0] = -1;
        it[in_idx].sprite[1] = 94;
        it[in_idx].duration = 18 * 60 * 60 * 2;
        it[in_idx].active = 18 * 60 * 60 * 2;
        it[in_idx].temp = SK_WIMPY as u16;
        it[in_idx].power = Repository::with_characters(|ch| ch[cn].skill[SK_WIMPY][5]) as u32;
    });

    if add_spell(cn, in_idx) == 0 {
        State::with(|state| {
            state.do_character_log(
                cn,
                core::types::FontColor::Green,
                &format!(
                    "Magical interference neutralised the {}'s effect.\n",
                    Repository::with_items(|it| it[in_idx].get_name().to_string())
                ),
            )
        });
        return;
    }
    State::with(|state| {
        state.do_character_log(
            cn,
            core::types::FontColor::Green,
            "Guardian Angel active!\n",
        )
    });
    let sound = Repository::with_characters(|ch| ch[cn].sound);
    State::char_play_sound(cn, sound as i32 + 1, -150, 0);
    chlog!(cn, "Cast Guardian Angel");
    EffectManager::fx_add_effect(
        7,
        0,
        Repository::with_characters(|ch| ch[cn].x) as i32,
        Repository::with_characters(|ch| ch[cn].y) as i32,
        0,
    );
    EffectManager::fx_add_effect(
        6,
        0,
        Repository::with_characters(|ch| ch[cn].x) as i32,
        Repository::with_characters(|ch| ch[cn].y) as i32,
        0,
    );
}

pub fn spell_mshield(cn: usize, co: usize, power: i32) -> i32 {
    let in_opt = God::create_item(1);
    if in_opt.is_none() {
        log::error!("god_create_item failed in skill_mshield");
        return 0;
    }
    let in_ = in_opt.unwrap();

    Repository::with_items_mut(|it| {
        let mut name_bytes = [0u8; 40];
        let name = b"Magic Shield";
        let len = name.len().min(40);
        name_bytes[..len].copy_from_slice(&name[..len]);
        it[in_].name = name_bytes;
        it[in_].flags |= ItemFlags::IF_SPELL.bits();
        it[in_].sprite[1] = 95;
        let dur = spell_race_mod(
            power * 256,
            Repository::with_characters(|ch| ch[cn].kindred),
        );
        it[in_].duration = dur as u32;
        it[in_].active = dur as u32;
        it[in_].armor[1] = (it[in_].active / 1024) as i8 + 1;
        it[in_].temp = SK_MSHIELD as u16;
        it[in_].power = it[in_].active / 256;
    });

    if cn != co {
        if add_spell(co, in_) == 0 {
            let name = Repository::with_items(|it| it[in_].get_name().to_string());
            State::with(|state| {
                state.do_character_log(
                    cn,
                    FontColor::Green,
                    &format!("Magical interference neutralised the {}'s effect.\n", name),
                )
            });
            return 0;
        }
        let sense = Repository::with_characters(|ch| ch[co].skill[SK_SENSE][5]);
        if sense as i32 + 10 > power {
            let reference = Repository::with_characters(|ch| ch[cn].reference);
            State::with(|state| {
                state.do_character_log(
                    co,
                    FontColor::Green,
                    &format!(
                        "{} cast magic shield on you.\n",
                        c_string_to_str(&reference)
                    ),
                )
            });
        } else {
            State::with(|state| {
                state.do_character_log(co, FontColor::Red, "Magic Shield active!\n")
            });
        }
        State::with(|state| {
            state.do_character_log(
                cn,
                FontColor::Green,
                &format!(
                    "{}'s Magic Shield activated.\n",
                    Repository::with_characters(|ch| ch[co].get_name().to_string())
                ),
            )
        });
        let sound = Repository::with_characters(|ch| ch[cn].sound);
        State::char_play_sound(co, sound as i32 + 1, -150, 0);
        State::char_play_sound(cn, sound as i32 + 1, -150, 0);
        chlog!(
            cn,
            "Cast Magic Shield on {}",
            Repository::with_characters(|ch| ch[co].get_name().to_string())
        );
        EffectManager::fx_add_effect(
            6,
            0,
            Repository::with_characters(|ch| ch[co].x) as i32,
            Repository::with_characters(|ch| ch[co].y) as i32,
            0,
        );
    } else {
        if add_spell(cn, in_) == 0 {
            let name = Repository::with_items(|it| it[in_].get_name().to_string());
            State::with(|state| {
                state.do_character_log(
                    cn,
                    FontColor::Green,
                    &format!("Magical interference neutralised the {}'s effect.\n", name),
                )
            });
            return 0;
        }
        State::with(|state| state.do_character_log(cn, FontColor::Green, "Magic Shield active!\n"));
        let sound = Repository::with_characters(|ch| ch[cn].sound);
        State::char_play_sound(cn, sound as i32 + 1, -150, 0);
        let flags = Repository::with_characters(|ch| ch[cn].flags);
        if (flags & CharacterFlags::Player.bits()) != 0 {
            chlog!(cn, "Cast Magic Shield");
        }
        EffectManager::fx_add_effect(
            6,
            0,
            Repository::with_characters(|ch| ch[cn].x) as i32,
            Repository::with_characters(|ch| ch[cn].y) as i32,
            0,
        );
    }

    EffectManager::fx_add_effect(
        7,
        0,
        Repository::with_characters(|ch| ch[cn].x) as i32,
        Repository::with_characters(|ch| ch[cn].y) as i32,
        0,
    );

    1
}

pub fn skill_mshield(cn: usize) {
    if is_exhausted(cn) != 0 {
        return;
    }

    if spellcost(cn, 25) != 0 {
        return;
    }
    if chance(cn, 18) != 0 {
        return;
    }

    spell_mshield(
        cn,
        cn,
        Repository::with_characters(|ch| ch[cn].skill[core::constants::SK_MSHIELD][5] as i32),
    );
    add_exhaust(cn, core::constants::TICKS * 3);
}

pub fn spell_heal(cn: usize, co: usize, power: i32) -> i32 {
    if cn != co {
        Repository::with_characters_mut(|ch| {
            ch[co].a_hp += spell_race_mod(power * 2500, ch[cn].kindred);
            if ch[co].a_hp > (ch[co].hp[5] as i32) * 1000 {
                ch[co].a_hp = (ch[co].hp[5] as i32) * 1000;
            }
        });
        let sense = Repository::with_characters(|ch| ch[co].skill[core::constants::SK_SENSE][5]);
        if sense as i32 + 10 > power {
            let reference = Repository::with_characters(|ch| ch[cn].reference);
            State::with(|state| {
                state.do_character_log(
                    co,
                    FontColor::Green,
                    &format!("{} cast heal on you.\n", c_string_to_str(&reference)),
                )
            });
        } else {
            State::with(|state| {
                state.do_character_log(co, FontColor::Red, "You have been healed.\n")
            });
        }
        State::with(|state| {
            state.do_character_log(
                cn,
                FontColor::Green,
                &format!(
                    "{} was healed.\n",
                    Repository::with_characters(|ch| ch[co].get_name().to_string())
                ),
            )
        });
        let sound = Repository::with_characters(|ch| ch[cn].sound);
        State::char_play_sound(co, sound as i32 + 1, -150, 0);
        State::char_play_sound(cn, sound as i32 + 1, -150, 0);
        chlog!(
            cn,
            "Cast Heal on {}",
            Repository::with_characters(|ch| ch[co].get_name().to_string())
        );
        EffectManager::fx_add_effect(
            6,
            0,
            Repository::with_characters(|ch| ch[co].x) as i32,
            Repository::with_characters(|ch| ch[co].y) as i32,
            0,
        );
    } else {
        Repository::with_characters_mut(|ch| {
            ch[cn].a_hp += power * 2500;
            if ch[cn].a_hp > (ch[cn].hp[5] as i32) * 1000 {
                ch[cn].a_hp = (ch[cn].hp[5] as i32) * 1000;
            }
        });
        State::with(|state| {
            state.do_character_log(cn, FontColor::Green, "You have been healed.\n")
        });
        let sound = Repository::with_characters(|ch| ch[cn].sound);
        State::char_play_sound(cn, sound as i32 + 1, -150, 0);
        let flags = Repository::with_characters(|ch| ch[cn].flags);
        if (flags & CharacterFlags::Player.bits()) != 0 {
            chlog!(cn, "Cast Heal");
        }
        EffectManager::fx_add_effect(
            6,
            0,
            Repository::with_characters(|ch| ch[cn].x) as i32,
            Repository::with_characters(|ch| ch[cn].y) as i32,
            0,
        );
    }

    EffectManager::fx_add_effect(
        7,
        0,
        Repository::with_characters(|ch| ch[cn].x) as i32,
        Repository::with_characters(|ch| ch[cn].y) as i32,
        0,
    );

    1
}

pub fn skill_heal(cn: usize) {
    let mut co = Repository::with_characters(|ch| {
        if ch[cn].skill_target1 != 0 {
            ch[cn].skill_target1 as usize
        } else {
            cn
        }
    });

    if State::with_mut(|state| state.do_char_can_see(cn, co)) == 0 {
        State::with(|state| {
            state.do_character_log(cn, FontColor::Red, "You cannot see your target.\n")
        });
        return;
    }

    if is_exhausted(cn) != 0 {
        return;
    }

    if driver::player_or_ghost(cn, co) == 0 {
        let name_from = Repository::with_characters(|ch| ch[co].get_name().to_string());
        let name_to = Repository::with_characters(|ch| ch[cn].get_name().to_string());
        State::with(|state| {
            state.do_character_log(
                cn,
                FontColor::Red,
                &format!(
                    "Changed target of spell from {} to {}.\n",
                    name_from, name_to
                ),
            )
        });
        co = cn;
        if spellcost(cn, 25) != 0 {
            return;
        }
        if chance(cn, 18) != 0 {
            if cn != co {
                let sense = Repository::with_characters(|ch| ch[co].skill[SK_SENSE][5]);
                let heal_skill = Repository::with_characters(|ch| ch[cn].skill[SK_HEAL][5]);
                if sense > (heal_skill + 5) {
                    let reference = Repository::with_characters(|ch| ch[cn].reference);
                    State::with(|state| {
                        state.do_character_log(
                            co,
                            FontColor::Green,
                            &format!(
                                "{} tried to cast heal on you but failed.\n",
                                c_string_to_str(&reference)
                            ),
                        )
                    });
                }
            }
            return;
        }
        spell_heal(
            cn,
            co,
            Repository::with_characters(|ch| ch[cn].skill[SK_HEAL][5] as i32),
        );
        add_exhaust(cn, TICKS * 2);
        return;
    }

    if spellcost(cn, 25) != 0 {
        return;
    }
    if chance(cn, 18) != 0 {
        if cn != co {
            let sense = Repository::with_characters(|ch| ch[co].skill[SK_SENSE][5]);
            let heal_skill = Repository::with_characters(|ch| ch[cn].skill[SK_HEAL][5]);
            if sense > (heal_skill + 5) {
                let reference = Repository::with_characters(|ch| ch[cn].reference);
                State::with(|state| {
                    state.do_character_log(
                        co,
                        FontColor::Green,
                        &format!(
                            "{} tried to cast heal on you but failed.\n",
                            c_string_to_str(&reference)
                        ),
                    )
                });
            }
        }
        return;
    }

    spell_heal(
        cn,
        co,
        Repository::with_characters(|ch| ch[cn].skill[SK_HEAL][5] as i32),
    );

    add_exhaust(cn, TICKS * 2);
}

pub fn spell_curse(cn: usize, co: usize, power: i32) -> i32 {
    let flags = Repository::with_characters(|ch| ch[co].flags);
    if (flags & CharacterFlags::Immortal.bits()) != 0 {
        return 0;
    }

    let in_opt = God::create_item(1);
    if in_opt.is_none() {
        log::error!("god_create_item failed in spell_curse");
        return 0;
    }
    let in_idx = in_opt.unwrap();

    let mut power = power;
    power = spell_immunity(
        power,
        Repository::with_characters(|ch| ch[co].skill[SK_IMMUN][5] as i32),
    );
    power = spell_race_mod(power, Repository::with_characters(|ch| ch[cn].kindred));

    Repository::with_items_mut(|it| {
        let mut name_bytes = [0u8; 40];
        let name = b"Curse";
        let len = name.len().min(40);
        name_bytes[..len].copy_from_slice(&name[..len]);
        it[in_idx].name = name_bytes;
        it[in_idx].flags |= ItemFlags::IF_SPELL.bits();
        for n in 0..5 {
            it[in_idx].attrib[n][1] = -((power / 3) as i8);
        }
        it[in_idx].sprite[1] = 89;
        it[in_idx].duration = 18 * 60 * 2;
        it[in_idx].active = 18 * 60 * 2;
        it[in_idx].temp = SK_CURSE as u16;
        it[in_idx].power = power as u32;
    });

    if add_spell(co, in_idx) == 0 {
        State::with(|state| {
            state.do_character_log(
                cn,
                FontColor::Green,
                &format!(
                    "Magical interference neutralised the {}'s effect.\n",
                    Repository::with_items(|it| it[in_idx].get_name().to_string())
                ),
            )
        });
        return 0;
    }

    let sense = Repository::with_characters(|ch| ch[co].skill[SK_SENSE][5]);
    if (sense as i32 + 10) > power {
        let reference = Repository::with_characters(|ch| ch[cn].reference);
        State::with(|state| {
            state.do_character_log(
                co,
                FontColor::Green,
                &format!("{} cast curse on you.\n", c_string_to_str(&reference)),
            )
        });
    } else {
        State::with(|state| {
            state.do_character_log(co, FontColor::Green, "You have been cursed.\n")
        });
    }

    let name = Repository::with_characters(|ch| ch[co].get_name().to_string());
    State::with(|state| {
        state.do_character_log(cn, FontColor::Green, &format!("{} was cursed.\n", name))
    });

    // Match C: don't generate spell-attack notifications when the target is ignoring spells.
    if Repository::with_characters(|ch| (ch[co].flags & CharacterFlags::SpellIgnore.bits()) == 0) {
        State::with(|state| {
            state.do_notify_character(co as u32, NT_GOTHIT as i32, cn as i32, 0, 0, 0)
        });
    }
    State::with(|state| state.do_notify_character(cn as u32, NT_DIDHIT as i32, co as i32, 0, 0, 0));

    let sound = Repository::with_characters(|ch| ch[cn].sound);
    State::char_play_sound(co, sound as i32 + 7, -150, 0);
    State::char_play_sound(cn, sound as i32 + 1, -150, 0);
    chlog!(
        cn,
        "Cast Curse on {}",
        Repository::with_characters(|ch| ch[co].get_name().to_string())
    );
    EffectManager::fx_add_effect(
        5,
        0,
        Repository::with_characters(|ch| ch[co].x) as i32,
        Repository::with_characters(|ch| ch[co].y) as i32,
        0,
    );

    1
}

pub fn skill_curse(cn: usize) {
    let co = Repository::with_characters(|ch| {
        if ch[cn].skill_target1 != 0 {
            ch[cn].skill_target1 as usize
        } else if ch[cn].attack_cn != 0 {
            ch[cn].attack_cn as usize
        } else {
            cn
        }
    });

    if cn == co {
        State::with(|state| {
            state.do_character_log(
                cn,
                core::types::FontColor::Red,
                "You cannot curse yourself.\n",
            )
        });
        return;
    }

    if State::with_mut(|state| state.do_char_can_see(cn, co)) == 0 {
        State::with(|state| {
            state.do_character_log(
                cn,
                core::types::FontColor::Red,
                "You cannot see your target.\n",
            )
        });
        return;
    }

    State::with(|s| s.remember_pvp(cn, co));
    if is_exhausted(cn) != 0 {
        return;
    }

    if spellcost(cn, 35) != 0 {
        return;
    }

    if State::with(|state| state.may_attack_msg(cn, co, true)) == 0 {
        chlog!(
            cn,
            "Prevented from attacking {}",
            Repository::with_characters(|ch| ch[co].get_name().to_string())
        );
        return;
    }

    if chance_base(
        cn,
        Repository::with_characters(|ch| ch[cn].skill[core::constants::SK_CURSE][5] as i32),
        10,
        Repository::with_characters(|ch| ch[co].skill[core::constants::SK_RESIST][5] as i32),
    ) != 0
    {
        if cn != co
            && Repository::with_characters(|ch| ch[co].skill[core::constants::SK_SENSE][5])
                > (Repository::with_characters(|ch| ch[cn].skill[core::constants::SK_CURSE][5]) + 5)
        {
            let reference = Repository::with_characters(|ch| ch[cn].reference);
            State::with(|state| {
                state.do_character_log(
                    co,
                    core::types::FontColor::Green,
                    &format!(
                        "{} tried to cast curse on you but failed.\n",
                        c_string_to_str(&reference)
                    ),
                )
            });
            if Repository::with_characters(|ch| ch[co].flags & CharacterFlags::SpellIgnore.bits())
                == 0
            {
                State::with(|state| {
                    state.do_notify_character(
                        co as u32,
                        core::constants::NT_GOTMISS as i32,
                        cn as i32,
                        0,
                        0,
                        0,
                    )
                });
            }
        }
        return;
    }

    if (Repository::with_characters(|ch| ch[co].flags) & CharacterFlags::Immortal.bits()) != 0 {
        State::with(|state| {
            state.do_character_log(cn, core::types::FontColor::Red, "You lost your focus.\n")
        });
        return;
    }

    spell_curse(
        cn,
        co,
        Repository::with_characters(|ch| ch[cn].skill[core::constants::SK_CURSE][5] as i32),
    );

    let co_orig = co;
    let (x, y) = Repository::with_characters(|ch| (ch[cn].x as i32, ch[cn].y as i32));
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
        let maybe_co = Repository::with_map(|map| map[idx].ch as usize);
        if maybe_co == 0 || maybe_co >= core::constants::MAXCHARS {
            continue;
        }
        if maybe_co != 0
            && Repository::with_characters(|ch| ch[maybe_co].attack_cn as usize) == cn
            && co_orig != maybe_co
        {
            if Repository::with_characters(|ch| ch[cn].skill[core::constants::SK_CURSE][5]) as i32
                + helpers::random_mod_i32(20)
                > Repository::with_characters(|ch| {
                    ch[maybe_co].skill[core::constants::SK_RESIST][5]
                }) as i32
                    + helpers::random_mod_i32(20)
            {
                spell_curse(
                    cn,
                    maybe_co,
                    Repository::with_characters(|ch| {
                        ch[cn].skill[core::constants::SK_CURSE][5] as i32
                    }),
                );
            }
        }
    }

    EffectManager::fx_add_effect(
        7,
        0,
        Repository::with_characters(|ch| ch[cn].x) as i32,
        Repository::with_characters(|ch| ch[cn].y) as i32,
        0,
    );

    add_exhaust(cn, core::constants::TICKS * 4);
}

pub fn warcry(cn: usize, co: usize, power: i32) -> i32 {
    if Repository::with_characters(|ch| ch[cn].attack_cn as usize) != co
        && Repository::with_characters(|ch| ch[co].alignment) == 10000
    {
        return 0;
    }

    if State::with(|state| state.may_attack_msg(cn, co, false)) == 0 {
        return 0;
    }

    if power < Repository::with_characters(|ch| ch[co].skill[core::constants::SK_RESIST][5]) as i32
    {
        return 0;
    }

    for n in 1..10 {
        if Repository::with_characters(|ch| ch[cn].data[n]) as usize == co {
            return 0;
        }
    }

    if (Repository::with_characters(|ch| ch[co].flags) & CharacterFlags::Immortal.bits()) != 0 {
        return 0;
    }

    if Repository::with_characters(|ch| ch[co].flags & CharacterFlags::SpellIgnore.bits()) == 0 {
        State::with(|state| {
            state.do_notify_character(
                co as u32,
                core::constants::NT_GOTHIT as i32,
                cn as i32,
                0,
                0,
                0,
            )
        });
    }

    let in_opt = God::create_item(1);
    if in_opt.is_none() {
        log::error!("god_create_item failed in skill_warcry");
        return 0;
    }
    let in_idx = in_opt.unwrap();

    Repository::with_items_mut(|it| {
        let mut name_bytes = [0u8; 40];
        let name = b"War-Stun";
        let len = name.len().min(40);
        name_bytes[..len].copy_from_slice(&name[..len]);
        it[in_idx].name = name_bytes;
        it[in_idx].flags |= ItemFlags::IF_SPELL.bits();
        it[in_idx].sprite[1] = 91;
        it[in_idx].duration = core::constants::TICKS as u32 * 3;
        it[in_idx].active = core::constants::TICKS as u32 * 3;
        it[in_idx].temp = core::constants::SK_WARCRY2 as u16;
        it[in_idx].power = power as u32;
    });

    add_spell(co, in_idx);

    let in2_opt = God::create_item(1);
    if in2_opt.is_none() {
        log::error!("god_create_item failed in skill_warcry");
        return 0;
    }
    let in2 = in2_opt.unwrap();
    Repository::with_items_mut(|it| {
        let mut name_bytes = [0u8; 40];
        let name = b"Warcry";
        let len = name.len().min(40);
        name_bytes[..len].copy_from_slice(&name[..len]);
        it[in2].name = name_bytes;
        it[in2].flags |= ItemFlags::IF_SPELL.bits();
        for n in 0..5 {
            it[in2].attrib[n][1] = -15;
        }
        it[in2].sprite[1] = 89;
        it[in2].duration = 18 * 60;
        it[in2].active = 18 * 60;
        it[in2].temp = core::constants::SK_WARCRY as u16;
        it[in2].power = (power / 2) as u32;
    });

    add_spell(co, in2);

    let co_name = Repository::with_characters(|ch| ch[co].get_name().to_string());
    log::info!("Character {} cast Warcry on {}", cn, co_name);

    EffectManager::fx_add_effect(
        5,
        0,
        Repository::with_characters(|ch| ch[co].x) as i32,
        Repository::with_characters(|ch| ch[co].y) as i32,
        0,
    );

    1
}

pub fn skill_warcry(cn: usize) {
    if Repository::with_characters(|ch| ch[cn].a_end) < 150 * 1000 {
        State::with(|state| {
            state.do_character_log(cn, core::types::FontColor::Red, "You're too exhausted!\n")
        });
        return;
    }

    Repository::with_characters_mut(|ch| ch[cn].a_end -= 150 * 1000);

    let power =
        Repository::with_characters(|ch| ch[cn].skill[core::constants::SK_WARCRY][5] as i32);

    let xf = std::cmp::max(1, Repository::with_characters(|ch| ch[cn].x as i32) - 10);
    let yf = std::cmp::max(1, Repository::with_characters(|ch| ch[cn].y as i32) - 10);
    let xt = std::cmp::min(
        core::constants::SERVER_MAPX - 1,
        Repository::with_characters(|ch| ch[cn].x as i32) + 10,
    );
    let yt = std::cmp::min(
        core::constants::SERVER_MAPY - 1,
        Repository::with_characters(|ch| ch[cn].y as i32) + 10,
    );

    let mut hit = 0;
    let mut miss = 0;
    for x in xf..xt {
        for y in yf..yt {
            let m = (x + y * core::constants::SERVER_MAPX) as usize;
            let co = Repository::with_map(|map| map[m].ch as usize);
            if co != 0 {
                if warcry(cn, co, power) != 0 {
                    State::with(|s| s.remember_pvp(cn, co));
                    let name = Repository::with_characters(|ch| ch[cn].get_name().to_string());
                    State::with(|state| {
                        state.do_character_log(
                            co,
                            core::types::FontColor::Green,
                            &format!(
                                "You hear {}'s warcry. You feel frightened and immobilized.\n",
                                name
                            ),
                        )
                    });
                    hit += 1;
                } else {
                    let name = Repository::with_characters(|ch| ch[cn].get_name().to_string());
                    State::with(|state| {
                        state.do_character_log(
                            co,
                            core::types::FontColor::Green,
                            &format!("You hear {}'s warcry.\n", name),
                        )
                    });
                    miss += 1;
                }
            }
        }
    }
    State::with(|state| {
        state.do_character_log(
            cn,
            core::types::FontColor::Green,
            &format!(
                "You cry out loud and clear. You affected {} of {} creatures in range.\n",
                hit,
                hit + miss
            ),
        )
    });
}

pub fn item_info(cn: usize, in_: usize, _look: i32) {
    let at_name = ["Braveness", "Willpower", "Intuition", "Agility", "Strength"];

    // Name
    let name = Repository::with_items(|it| it[in_].name);
    State::with(|state| {
        state.do_character_log(
            cn,
            FontColor::Green,
            &format!("{}:\n", c_string_to_str(&name)),
        )
    });

    State::with(|state| {
        state.do_character_log(cn, FontColor::Green, "Stat         Mod0 Mod1 Min\n")
    });

    // Attributes
    for n in 0..5 {
        let (a0, a1, a2) = Repository::with_items(|it| {
            (
                it[in_].attrib[n][0],
                it[in_].attrib[n][1],
                it[in_].attrib[n][2],
            )
        });
        if a0 == 0 && a1 == 0 && a2 == 0 {
            continue;
        }
        State::with(|state| {
            state.do_character_log(
                cn,
                FontColor::Green,
                &format!("{:<12.12} {:+4} {:+4} {:3}\n", at_name[n], a0, a1, a2),
            )
        });
    }

    // HP/End/Mana
    let (hp0, hp1, hp2) =
        Repository::with_items(|it| (it[in_].hp[0], it[in_].hp[1], it[in_].hp[2]));
    if hp0 != 0 || hp1 != 0 || hp2 != 0 {
        State::with(|state| {
            state.do_character_log(
                cn,
                FontColor::Green,
                &format!("{:<12.12} {:+4} {:+4} {:3}\n", "Hitpoints", hp0, hp1, hp2),
            )
        });
    }
    let (end0, end1, end2) =
        Repository::with_items(|it| (it[in_].end[0], it[in_].end[1], it[in_].end[2]));
    if end0 != 0 || end1 != 0 || end2 != 0 {
        State::with(|state| {
            state.do_character_log(
                cn,
                FontColor::Green,
                &format!(
                    "{:<12.12} {:+4} {:+4} {:3}\n",
                    "Endurance", end0, end1, end2
                ),
            )
        });
    }
    let (mana0, mana1, mana2) =
        Repository::with_items(|it| (it[in_].mana[0], it[in_].mana[1], it[in_].mana[2]));
    if mana0 != 0 || mana1 != 0 || mana2 != 0 {
        State::with(|state| {
            state.do_character_log(
                cn,
                FontColor::Green,
                &format!("{:<12.12} {:+4} {:+4} {:3}\n", "Mana", mana0, mana1, mana2),
            )
        });
    }

    // Skills on item
    for n in 0..50 {
        let (s0, s1, s2) = Repository::with_items(|it| {
            (
                it[in_].skill[n][0],
                it[in_].skill[n][1],
                it[in_].skill[n][2],
            )
        });
        if s0 == 0 && s1 == 0 && s2 == 0 {
            continue;
        }
        let skill_label = skill_name(n);
        State::with(|state| {
            state.do_character_log(
                cn,
                FontColor::Green,
                &format!("{:<12.12} {:+4} {:+4} {:3}\n", skill_label, s0, s1, s2),
            )
        });
    }

    // Weapon/Armor/Light
    let (w0, w1) = Repository::with_items(|it| (it[in_].weapon[0], it[in_].weapon[1]));
    if w0 != 0 || w1 != 0 {
        State::with(|state| {
            state.do_character_log(
                cn,
                FontColor::Green,
                &format!("{:<12.12} {:+4} {:+4}\n", "Weapon", w0, w1),
            )
        });
    }
    let (ar0, ar1) = Repository::with_items(|it| (it[in_].armor[0], it[in_].armor[1]));
    if ar0 != 0 || ar1 != 0 {
        State::with(|state| {
            state.do_character_log(
                cn,
                FontColor::Green,
                &format!("{:<12.12} {:+4} {:+4}\n", "Armor", ar0, ar1),
            )
        });
    }
    let (l0, l1) = Repository::with_items(|it| (it[in_].light[0], it[in_].light[1]));
    if l0 != 0 || l1 != 0 {
        State::with(|state| {
            state.do_character_log(
                cn,
                FontColor::Green,
                &format!("{:<12.12} {:+4} {:+4}\n", "Light", l0, l1),
            )
        });
    }

    let power = Repository::with_items(|it| it[in_].power);
    if power != 0 {
        State::with(|state| {
            state.do_character_log(
                cn,
                FontColor::Green,
                &format!("{:<12.12} {:+4}\n", "Power", power),
            )
        });
    }

    let min_rank = Repository::with_items(|it| it[in_].min_rank);
    if min_rank != 0 {
        State::with(|state| {
            state.do_character_log(
                cn,
                FontColor::Green,
                &format!("{:<12.12} {:+4}\n", "Min. Rank", min_rank),
            )
        });
    }
}

pub fn char_info(cn: usize, co: usize) {
    let at_name = ["Braveness", "Willpower", "Intuition", "Agility", "Strength"];

    // Header
    let name_bytes = Repository::with_characters(|ch| ch[co].name);
    State::with(|state| {
        state.do_character_log(
            cn,
            FontColor::Green,
            &format!("{}:\n", c_string_to_str(&name_bytes)),
        )
    });
    State::with(|state| state.do_character_log(cn, FontColor::Green, " \n"));

    // Active spells (0..19)
    let mut flag = false;
    for n in 0..20 {
        let in_idx = Repository::with_characters(|ch| ch[co].spell[n] as usize);
        if in_idx != 0 {
            let item_name = Repository::with_items(|it| it[in_idx].get_name().to_string());
            let active = Repository::with_items(|it| it[in_idx].active);
            let minutes = active / (TICKS as u32 * 60);
            let seconds = (active / TICKS as u32) % 60;
            let power = Repository::with_items(|it| it[in_idx].power);
            State::with(|state| {
                state.do_character_log(
                    cn,
                    FontColor::Green,
                    &format!(
                        "{} for {}m {}s power of {}\n",
                        item_name, minutes, seconds, power
                    ),
                )
            });
            flag = true;
        }
    }
    if !flag {
        State::with(|state| state.do_character_log(cn, FontColor::Green, "No spells active.\n"));
    }
    State::with(|state| state.do_character_log(cn, FontColor::Green, " \n"));

    // Skills two-column using static SKILL_NAMES
    let mut n1: i32 = -1;
    let mut n2: i32 = -1;
    for n in 0..50 {
        let s0 = Repository::with_characters(|ch| ch[co].skill[n][0]);
        if s0 != 0 && n1 == -1 {
            n1 = n as i32;
        } else if s0 != 0 && n2 == -1 {
            n2 = n as i32;
        }

        if n1 != -1 && n2 != -1 {
            let s1_0 = Repository::with_characters(|ch| ch[co].skill[n1 as usize][0]);
            let s1_5 = Repository::with_characters(|ch| ch[co].skill[n1 as usize][5]);
            let s2_0 = Repository::with_characters(|ch| ch[co].skill[n2 as usize][0]);
            let s2_5 = Repository::with_characters(|ch| ch[co].skill[n2 as usize][5]);
            let name1 = SKILL_NAMES[n1 as usize];
            let name2 = SKILL_NAMES[n2 as usize];
            State::with(|state| {
                state.do_character_log(
                    cn,
                    FontColor::Green,
                    &format!(
                        "{:<12.12} {:3}/{:3}  !  {:<12.12} {:3}/{:3}\n",
                        name1, s1_0, s1_5, name2, s2_0, s2_5
                    ),
                )
            });
            n1 = -1;
            n2 = -1;
        }
    }

    if n1 != -1 {
        let s1_0 = Repository::with_characters(|ch| ch[co].skill[n1 as usize][0]);
        let s1_5 = Repository::with_characters(|ch| ch[co].skill[n1 as usize][5]);
        let name1 = SKILL_NAMES[n1 as usize];
        State::with(|state| {
            state.do_character_log(
                cn,
                FontColor::Green,
                &format!("{:<12.12} {:3}/{:3}\n", name1, s1_0, s1_5),
            )
        });
    }

    // Attributes
    let a0_0 = Repository::with_characters(|ch| ch[co].attrib[0][0]);
    let a0_5 = Repository::with_characters(|ch| ch[co].attrib[0][5]);
    let a1_0 = Repository::with_characters(|ch| ch[co].attrib[1][0]);
    let a1_5 = Repository::with_characters(|ch| ch[co].attrib[1][5]);
    State::with(|state| {
        state.do_character_log(
            cn,
            FontColor::Green,
            &format!(
                "{:<12.12} {:3}/{:3}  !  {:<12.12} {:3}/{:3}\n",
                at_name[0], a0_0, a0_5, at_name[1], a1_0, a1_5
            ),
        )
    });
    let a2_0 = Repository::with_characters(|ch| ch[co].attrib[2][0]);
    let a2_5 = Repository::with_characters(|ch| ch[co].attrib[2][5]);
    let a3_0 = Repository::with_characters(|ch| ch[co].attrib[3][0]);
    let a3_5 = Repository::with_characters(|ch| ch[co].attrib[3][5]);
    State::with(|state| {
        state.do_character_log(
            cn,
            FontColor::Green,
            &format!(
                "{:<12.12} {:3}/{:3}  !  {:<12.12} {:3}/{:3}\n",
                at_name[2], a2_0, a2_5, at_name[3], a3_0, a3_5
            ),
        )
    });
    let a4_0 = Repository::with_characters(|ch| ch[co].attrib[4][0]);
    let a4_5 = Repository::with_characters(|ch| ch[co].attrib[4][5]);
    State::with(|state| {
        state.do_character_log(
            cn,
            FontColor::Green,
            &format!("{:<12.12} {:3}/{:3}\n", at_name[4], a4_0, a4_5),
        )
    });

    State::with(|state| state.do_character_log(cn, FontColor::Green, " \n"));
}

pub fn skill_identify(cn: usize) {
    if is_exhausted(cn) != 0 {
        return;
    }

    if spellcost(cn, 25) != 0 {
        return;
    }

    let citem = Repository::with_characters(|ch| ch[cn].citem as usize);
    let in_idx: usize;
    let mut co = 0usize;
    let power: i32;

    let sane_item = if citem != 0 {
        Repository::with_items(|it| {
            citem < it.len() && it[citem].used != core::constants::USE_EMPTY
        })
    } else {
        false
    };

    if citem != 0 && sane_item {
        in_idx = citem;
        power = Repository::with_items(|it| it[in_idx].power as i32);
    } else {
        let target = Repository::with_characters(|ch| ch[cn].skill_target1 as usize);
        if target != 0 {
            co = target;
            power = Repository::with_characters(|ch| ch[co].skill[SK_RESIST][5] as i32);
        } else {
            co = cn;
            power = 10;
        }
        in_idx = 0;
    }

    if chance_base(
        cn,
        Repository::with_characters(|ch| ch[cn].skill[SK_IDENT][5] as i32),
        18,
        power,
    ) != 0
    {
        return;
    }

    let sound = Repository::with_characters(|ch| ch[cn].sound);
    State::char_play_sound(cn, sound as i32 + 1, -150, 0);
    chlog!(
        cn,
        "Cast Identify on {}",
        if in_idx != 0 {
            Repository::with_items(|it| it[in_idx].get_name().to_string())
        } else {
            Repository::with_characters(|ch| ch[co].get_name().to_string())
        }
    );

    if in_idx != 0 {
        item_info(cn, in_idx, 0);
        Repository::with_items_mut(|it| it[in_idx].flags ^= ItemFlags::IF_IDENTIFIED.bits());
        let identified =
            Repository::with_items(|it| (it[in_idx].flags & ItemFlags::IF_IDENTIFIED.bits()) != 0);
        if !identified {
            State::with(|state| {
                state.do_character_log(
                    cn,
                    core::types::FontColor::Green,
                    "Identify data removed from item.\n",
                )
            });
        }
    } else {
        char_info(cn, co);
        EffectManager::fx_add_effect(
            6,
            0,
            Repository::with_characters(|ch| ch[co].x) as i32,
            Repository::with_characters(|ch| ch[co].y) as i32,
            0,
        );
    }

    add_exhaust(cn, TICKS * 2);
    EffectManager::fx_add_effect(
        7,
        0,
        Repository::with_characters(|ch| ch[cn].x) as i32,
        Repository::with_characters(|ch| ch[cn].y) as i32,
        0,
    );
}

pub fn skill_blast(cn: usize) {
    let co = Repository::with_characters(|ch| {
        if ch[cn].skill_target1 != 0 {
            ch[cn].skill_target1 as usize
        } else if ch[cn].attack_cn != 0 {
            ch[cn].attack_cn as usize
        } else {
            cn
        }
    });

    if State::with_mut(|state| state.do_char_can_see(cn, co)) == 0 {
        State::with(|state| {
            state.do_character_log(cn, FontColor::Green, "You cannot see your target.\n")
        });
        return;
    }

    if cn == co {
        State::with(|state| {
            state.do_character_log(cn, FontColor::Green, "You cannot blast yourself!\n")
        });
        return;
    }

    if Repository::with_characters(|ch| (ch[co].flags & CharacterFlags::Stoned.bits()) != 0) {
        State::with(|state| {
            state.do_character_log(
                cn,
                FontColor::Green,
                "Your target is lagging. Try again later.\n",
            )
        });
        return;
    }

    if State::with(|state| state.may_attack_msg(cn, co, true)) == 0 {
        chlog!(
            cn,
            "Prevented from attacking {}",
            Repository::with_characters(|ch| ch[co].get_name().to_string())
        );
        return;
    }

    State::with(|state| state.remember_pvp(cn, co));

    if is_exhausted(cn) != 0 {
        return;
    }

    let mut power =
        Repository::with_characters(|ch| ch[cn].skill[core::constants::SK_BLAST][5] as i32);
    power = spell_immunity(
        power,
        Repository::with_characters(|ch| ch[co].skill[core::constants::SK_IMMUN][5] as i32),
    );
    power = spell_race_mod(power, Repository::with_characters(|ch| ch[cn].kindred));

    let mut dam = power * 2;

    let mut cost = dam / 8 + 5;
    if Repository::with_characters(|ch| (ch[cn].flags & CharacterFlags::Player.bits()) != 0)
        && (Repository::with_characters(|ch| ch[cn].kindred as u32)
            & (core::constants::KIN_HARAKIM | core::constants::KIN_ARCHHARAKIM)
            != 0)
    {
        cost /= 3;
    }

    if spellcost(cn, cost) != 0 {
        return;
    }

    if driver::chance(cn, 18) != 0 {
        if cn != co
            && Repository::with_characters(|ch| ch[co].skill[core::constants::SK_SENSE][5])
                > Repository::with_characters(|ch| ch[cn].skill[core::constants::SK_BLAST][5]) + 5
        {
            State::with(|state| {
                state.do_character_log(
                    co,
                    FontColor::Green,
                    &format!(
                        "{} tried to cast blast on you but failed.\n",
                        c_string_to_str(&Repository::with_characters(|ch| ch[cn].reference))
                    ),
                )
            });
            if Repository::with_characters(|ch| ch[co].flags & CharacterFlags::SpellIgnore.bits())
                == 0
            {
                State::with(|state| {
                    state.do_notify_character(
                        co as u32,
                        core::constants::NT_GOTMISS as i32,
                        cn as i32,
                        0,
                        0,
                        0,
                    )
                });
            }
        }
        return;
    }

    State::do_area_sound(
        co,
        0,
        Repository::with_characters(|ch| ch[co].x as i32),
        Repository::with_characters(|ch| ch[co].y as i32),
        Repository::with_characters(|ch| ch[cn].sound) as i32 + 6,
    );
    State::char_play_sound(
        co,
        Repository::with_characters(|ch| ch[cn].sound) as i32 + 6,
        -150,
        0,
    );

    chlog!(
        cn,
        "Cast Blast on {} for {} power",
        Repository::with_characters(|ch| ch[co].get_name().to_string()),
        power
    );
    let tmp = State::with_mut(|state| state.do_hurt(cn, co, dam, 1));

    if tmp < 1 {
        State::with(|state| {
            state.do_character_log(
                cn,
                FontColor::Green,
                "You cannot penetrate your target's armor.\n",
            )
        });
    } else {
        State::with(|state| {
            state.do_character_log(
                cn,
                FontColor::Green,
                &format!("You blast your target for {} HP.\n", tmp),
            )
        });
    }

    EffectManager::fx_add_effect(
        5,
        0,
        Repository::with_characters(|ch| ch[co].x) as i32,
        Repository::with_characters(|ch| ch[co].y) as i32,
        0,
    );

    let co_orig = co;
    dam = dam / 2 + dam / 4;

    let (cx, cy) = Repository::with_characters(|ch| (ch[cn].x as i32, ch[cn].y as i32));
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
        let maybe_co = Repository::with_map(|map| map[idx].ch) as usize;
        if maybe_co == 0 || maybe_co >= core::constants::MAXCHARS {
            continue;
        }
        if maybe_co == co_orig {
            continue;
        }
        if Repository::with_characters(|ch| ch[maybe_co].attack_cn) != cn as u16 {
            continue;
        }

        let tmp2 = State::with_mut(|state| state.do_hurt(cn, maybe_co, dam, 1));
        if tmp2 < 1 {
            State::with(|state| {
                state.do_character_log(
                    cn,
                    FontColor::Green,
                    "You cannot penetrate your target's armor.\n",
                )
            });
        } else {
            State::with(|state| {
                state.do_character_log(
                    cn,
                    FontColor::Green,
                    &format!("You blast your target for {} HP.\n", tmp2),
                )
            });
        }
        EffectManager::fx_add_effect(
            5,
            0,
            Repository::with_characters(|ch| ch[maybe_co].x) as i32,
            Repository::with_characters(|ch| ch[maybe_co].y) as i32,
            0,
        );
    }

    add_exhaust(cn, core::constants::TICKS * 6);
    EffectManager::fx_add_effect(
        7,
        0,
        Repository::with_characters(|ch| ch[cn].x) as i32,
        Repository::with_characters(|ch| ch[cn].y) as i32,
        0,
    );
}

pub fn skill_repair(cn: usize) {
    let in_idx = Repository::with_characters(|ch| ch[cn].citem as usize);
    if in_idx == 0 {
        State::with(|state| {
            state.do_character_log(cn, core::types::FontColor::Green, "Repair. Repair what?\n")
        });
        return;
    }

    if Repository::with_items(|it| it[in_idx].damage_state) == 0 {
        State::with(|state| {
            state.do_character_log(cn, core::types::FontColor::Green, "That isn't damaged.\n")
        });
        return;
    }

    if Repository::with_items(|it| it[in_idx].power as i32)
        > Repository::with_characters(|ch| ch[cn].skill[SK_REPAIR][5] as i32)
        || Repository::with_items(|it| (it[in_idx].flags & ItemFlags::IF_NOREPAIR.bits()) != 0)
    {
        State::with(|state| {
            state.do_character_log(
                cn,
                core::types::FontColor::Green,
                "That's too difficult for you.\n",
            )
        });
        return;
    }

    if Repository::with_characters(|ch| ch[cn].a_end)
        < Repository::with_items(|it| it[in_idx].power as i32) * 1000
    {
        State::with(|state| {
            state.do_character_log(
                cn,
                core::types::FontColor::Green,
                "You're too exhausted to repair that.\n",
            )
        });
        return;
    }

    let cost = Repository::with_items(|it| it[in_idx].power as i32);
    Repository::with_characters_mut(|ch| ch[cn].a_end -= cost * 1000);

    let mut chan: i32 = if Repository::with_items(|it| it[in_idx].power) != 0 {
        let skill = Repository::with_characters(|ch| ch[cn].skill[SK_REPAIR][5]) as i32;
        let power = Repository::with_items(|it| it[in_idx].power) as i32;
        skill * 15 / power
    } else {
        18
    };

    if chan > 18 {
        chan = 18;
    }

    let die = helpers::random_mod_i32(20);

    if die <= chan {
        let in2_opt = God::create_item(Repository::with_items(|it| it[in_idx].temp) as usize);
        if in2_opt.is_none() {
            State::with(|state| {
                state.do_character_log(cn, core::types::FontColor::Green, "You failed.\n")
            });
            return;
        }
        let in2 = in2_opt.unwrap();
        Repository::with_items_mut(|it| it[in_idx].used = core::constants::USE_EMPTY);
        Repository::with_characters_mut(|ch| ch[cn].citem = in2 as u32);
        Repository::with_items_mut(|it| it[in2].carried = cn as u16);
        State::with(|state| {
            state.do_character_log(cn, core::types::FontColor::Green, "Success!\n")
        });
    } else {
        State::with(|state| {
            state.do_character_log(cn, core::types::FontColor::Green, "You failed.\n")
        });
        driver::item_damage_citem(cn, 1000000);
        if die - chan > 3 {
            driver::item_damage_citem(cn, 1000000);
        }
        if die - chan > 6 {
            driver::item_damage_citem(cn, 1000000);
        }
    }
    chlog!(
        cn,
        "Cast Repair on {}",
        Repository::with_items(|it| it[in_idx].get_name().to_string())
    );
}

pub fn skill_recall(cn: usize) {
    if is_exhausted(cn) != 0 {
        return;
    }

    if spellcost(cn, 15) != 0 {
        return;
    }

    if chance(cn, 18) != 0 {
        return;
    }

    let in_opt = God::create_item(1);
    if in_opt.is_none() {
        State::with(|state| {
            state.do_character_log(cn, core::types::FontColor::Green, "You failed.\n")
        });
        return;
    }
    let in_idx = in_opt.unwrap();

    Repository::with_items_mut(|it| {
        let mut name_bytes = [0u8; 40];
        let name = b"Recall";
        let len = name.len().min(40);
        name_bytes[..len].copy_from_slice(&name[..len]);
        it[in_idx].name = name_bytes;
        it[in_idx].flags |= ItemFlags::IF_SPELL.bits();
        it[in_idx].sprite[1] = 90;
        let dur = std::cmp::max(
            TICKS / 2,
            60 - (Repository::with_characters(|ch| ch[cn].skill[SK_RECALL][5] / 4) as i32),
        );
        it[in_idx].duration = dur as u32;
        it[in_idx].active = it[in_idx].duration;
        it[in_idx].temp = SK_RECALL as u16;
        it[in_idx].power = Repository::with_characters(|ch| ch[cn].skill[SK_RECALL][5]) as u32;
        it[in_idx].data[0] = Repository::with_characters(|ch| ch[cn].temple_x) as u32;
        it[in_idx].data[1] = Repository::with_characters(|ch| ch[cn].temple_y) as u32;
    });

    if add_spell(cn, in_idx) == 0 {
        State::with(|state| {
            state.do_character_log(cn, core::types::FontColor::Green, "You failed.\n")
        });
        return;
    }

    chlog!(cn, "Cast Recall");
    add_exhaust(cn, TICKS);
    EffectManager::fx_add_effect(
        7,
        0,
        Repository::with_characters(|ch| ch[cn].x) as i32,
        Repository::with_characters(|ch| ch[cn].y) as i32,
        0,
    );
}

pub fn spell_stun(cn: usize, co: usize, power: i32) -> i32 {
    if Repository::with_characters(|ch| (ch[co].flags & CharacterFlags::Immortal.bits()) != 0) {
        return 0;
    }

    let in_opt = God::create_item(1);
    if in_opt.is_none() {
        // xlog equivalent omitted
        return 0;
    }
    let in_idx = in_opt.unwrap();

    let mut power = spell_immunity(
        power,
        Repository::with_characters(|ch| ch[co].skill[core::constants::SK_IMMUN][5] as i32),
    );
    power = spell_race_mod(power, Repository::with_characters(|ch| ch[cn].kindred));

    Repository::with_items_mut(|it| {
        let mut name_bytes = [0u8; 40];
        let name = b"Stun";
        let len = name.len().min(40);
        name_bytes[..len].copy_from_slice(&name[..len]);
        it[in_idx].name = name_bytes;
        it[in_idx].flags |= ItemFlags::IF_SPELL.bits();
        it[in_idx].sprite[1] = 91;
        it[in_idx].duration = (power + core::constants::TICKS) as u32;
        it[in_idx].active = it[in_idx].duration;
        it[in_idx].temp = core::constants::SK_STUN as u16;
        it[in_idx].power = power as u32;
    });

    if Repository::with_characters(|ch| {
        ch[co].skill[core::constants::SK_SENSE][5] + 10 > power as u8
    }) {
        State::with(|state| {
            state.do_character_log(
                co,
                FontColor::Green,
                &format!(
                    "{} cast stun on you.\n",
                    c_string_to_str(&Repository::with_characters(|ch| ch[cn].reference))
                ),
            )
        });
    } else {
        State::with(|state| {
            state.do_character_log(co, FontColor::Green, "You have been stunned.\n")
        });
    }

    State::with(|state| {
        state.do_character_log(
            cn,
            FontColor::Green,
            &format!(
                "{} was stunned.\n",
                c_string_to_str(&Repository::with_characters(|ch| ch[co].reference))
            ),
        )
    });

    if Repository::with_characters(|ch| ch[co].flags & CharacterFlags::SpellIgnore.bits()) == 0 {
        State::with(|state| {
            state.do_notify_character(
                co as u32,
                core::constants::NT_GOTHIT as i32,
                cn as i32,
                0,
                0,
                0,
            )
        });
    }
    State::with(|state| {
        state.do_notify_character(
            cn as u32,
            core::constants::NT_DIDHIT as i32,
            co as i32,
            0,
            0,
            0,
        )
    });

    State::char_play_sound(
        co,
        Repository::with_characters(|ch| ch[cn].sound) as i32 + 7,
        -150,
        0,
    );
    State::char_play_sound(
        cn,
        Repository::with_characters(|ch| ch[cn].sound) as i32 + 1,
        -150,
        0,
    );
    chlog!(
        cn,
        "Cast Stun on {} for {} power",
        Repository::with_characters(|ch| ch[co].get_name().to_string()),
        power
    );

    if driver::add_spell(co, in_idx) == 0 {
        State::with(|state| {
            state.do_character_log(
                cn,
                FontColor::Green,
                &format!(
                    "Magical interference neutralised the {}'s effect.\n",
                    "stun"
                ),
            )
        });
        return 0;
    }

    EffectManager::fx_add_effect(
        5,
        0,
        Repository::with_characters(|ch| ch[co].x) as i32,
        Repository::with_characters(|ch| ch[co].y) as i32,
        0,
    );

    1
}

pub fn skill_stun(cn: usize) {
    let co = Repository::with_characters(|ch| {
        if ch[cn].skill_target1 != 0 {
            ch[cn].skill_target1 as usize
        } else if ch[cn].attack_cn != 0 {
            ch[cn].attack_cn as usize
        } else {
            cn
        }
    });

    if cn == co {
        State::with(|state| {
            state.do_character_log(
                cn,
                core::types::FontColor::Green,
                "You cannot stun yourself!\n",
            )
        });
        return;
    }

    if State::with_mut(|state| state.do_char_can_see(cn, co)) == 0 {
        State::with(|state| {
            state.do_character_log(
                cn,
                core::types::FontColor::Green,
                "You cannot see your target.\n",
            )
        });
        return;
    }

    State::with(|state| state.remember_pvp(cn, co));
    if is_exhausted(cn) != 0 {
        return;
    }

    if State::with(|state| state.may_attack_msg(cn, co, true)) == 0 {
        chlog!(
            cn,
            "Prevented from attacking {}",
            Repository::with_characters(|ch| ch[co].get_name().to_string())
        );
        return;
    }

    if spellcost(cn, 20) != 0 {
        return;
    }

    if chance_base(
        cn,
        Repository::with_characters(|ch| ch[cn].skill[core::constants::SK_STUN][5] as i32),
        12,
        Repository::with_characters(|ch| ch[co].skill[core::constants::SK_RESIST][5] as i32),
    ) != 0
    {
        if cn != co
            && Repository::with_characters(|ch| ch[co].skill[core::constants::SK_SENSE][5])
                > Repository::with_characters(|ch| ch[cn].skill[core::constants::SK_STUN][5]) + 5
        {
            State::with(|state| {
                state.do_character_log(
                    co,
                    core::types::FontColor::Green,
                    &format!(
                        "{} tried to cast stun on you but failed.\n",
                        c_string_to_str(&Repository::with_characters(|ch| ch[cn].reference))
                    ),
                )
            });
            if Repository::with_characters(|ch| ch[co].flags & CharacterFlags::SpellIgnore.bits())
                == 0
            {
                State::with(|state| {
                    state.do_notify_character(
                        co as u32,
                        core::constants::NT_GOTMISS as i32,
                        cn as i32,
                        0,
                        0,
                        0,
                    )
                });
            }
        }
        return;
    }

    if Repository::with_characters(|ch| (ch[co].flags & CharacterFlags::Immortal.bits()) != 0) {
        State::with(|state| {
            state.do_character_log(cn, core::types::FontColor::Red, "You lost your focus.\n")
        });
        return;
    }

    let power = Repository::with_characters(|ch| ch[cn].skill[core::constants::SK_STUN][5] as i32);
    spell_stun(cn, co, power);

    let co_orig = co;
    let m = Repository::with_characters(|ch| ch[cn].x)
        + Repository::with_characters(|ch| ch[cn].y) * core::constants::SERVER_MAPX as i16;

    let adj = [
        1isize,
        -1isize,
        core::constants::SERVER_MAPX as isize,
        -(core::constants::SERVER_MAPX as isize),
    ];
    for delta in adj.iter() {
        let idx = (m as isize + *delta) as usize;
        let maybe_co =
            Repository::with_map(|map| map.get(idx).and_then(|m| Some(m.ch))).unwrap_or(0) as usize;
        if maybe_co != 0
            && Repository::with_characters(|ch| ch[maybe_co].attack_cn) == cn as u16
            && maybe_co != co_orig
        {
            let s_rand = helpers::random_mod_i32(20);
            let o_rand = helpers::random_mod_i32(20);
            if Repository::with_characters(|ch| ch[cn].skill[core::constants::SK_STUN][5] as i32)
                + s_rand
                > Repository::with_characters(|ch| {
                    ch[maybe_co].skill[core::constants::SK_RESIST][5] as i32
                }) + o_rand
            {
                spell_stun(
                    cn,
                    maybe_co,
                    Repository::with_characters(|ch| {
                        ch[cn].skill[core::constants::SK_STUN][5] as i32
                    }),
                );
            }
        }
    }

    EffectManager::fx_add_effect(
        7,
        0,
        Repository::with_characters(|ch| ch[cn].x) as i32,
        Repository::with_characters(|ch| ch[cn].y) as i32,
        0,
    );
    add_exhaust(cn, core::constants::TICKS * 3);
}

pub fn remove_spells(cn: usize) {
    for n in 0..20usize {
        let in_idx = Repository::with_characters(|ch| ch[cn].spell[n] as usize);
        if in_idx == 0 {
            continue;
        }
        Repository::with_items_mut(|it| it[in_idx].used = core::constants::USE_EMPTY);
        Repository::with_characters_mut(|ch| ch[cn].spell[n] = 0);
    }
    State::with(|state| state.do_update_char(cn));
}

pub fn skill_dispel(cn: usize) {
    // Port of C `skill_dispel(int cn)`.
    let target = Repository::with_characters(|ch| ch[cn].skill_target1 as usize);
    let co = if target != 0 { target } else { cn };

    if State::with_mut(|state| state.do_char_can_see(cn, co)) == 0 {
        State::with(|state| {
            state.do_character_log(cn, FontColor::Red, "You cannot see your target.\n")
        });
        return;
    }

    if is_exhausted(cn) != 0 {
        return;
    }

    // Select which spell slot to remove.
    let mut slot: Option<usize> = None;

    // 1) Prefer removing curse from target.
    for n in 0..20usize {
        let in_idx = Repository::with_characters(|ch| ch[co].spell[n] as usize);
        if in_idx == 0 {
            continue;
        }
        if Repository::with_items(|it| it[in_idx].temp) == SK_CURSE as u16 {
            slot = Some(n);
            break;
        }
    }

    // 2) If no curse found, remove first non-wimpy spell.
    if slot.is_none() {
        for n in 0..20usize {
            let in_idx = Repository::with_characters(|ch| ch[co].spell[n] as usize);
            if in_idx == 0 {
                continue;
            }
            let temp = Repository::with_items(|it| it[in_idx].temp);
            if temp == SK_WIMPY as u16 {
                continue;
            }
            slot = Some(n);
            break;
        }

        // No target spell found.
        if slot.is_none() {
            if co == cn {
                State::with(|state| {
                    state.do_character_log(cn, FontColor::Red, "But you aren't spelled!\n")
                });
            } else {
                let name = Repository::with_characters(|ch| ch[co].get_name().to_string());
                State::with(|state| {
                    state.do_character_log(
                        cn,
                        FontColor::Red,
                        &format!("{} isn't spelled!\n", name),
                    )
                });
            }
            return;
        }

        // Dispelling someone else's non-curse spell is treated like an attack.
        if target != 0 {
            if State::with(|state| state.may_attack_msg(cn, co, true)) == 0 {
                chlog!(
                    cn,
                    "Prevented from dispelling {}",
                    Repository::with_characters(|ch| ch[co].get_name().to_string())
                );
                return;
            }
        }
    }

    let slot = slot.expect("slot must be set");
    let in_idx = Repository::with_characters(|ch| ch[co].spell[slot] as usize);
    if in_idx == 0 {
        return;
    }

    let pwr = Repository::with_items(|it| it[in_idx].power as i32);

    if spellcost(cn, 25) != 0 {
        return;
    }

    let dispel_skill = Repository::with_characters(|ch| ch[cn].skill[SK_DISPEL][5] as i32);
    let kindred = Repository::with_characters(|ch| ch[cn].kindred);
    if chance_base(cn, spell_race_mod(dispel_skill, kindred), 12, pwr) != 0 {
        if cn != co {
            let sense = Repository::with_characters(|ch| ch[co].skill[SK_SENSE][5] as i32);
            if sense > dispel_skill + 5 {
                let reference = Repository::with_characters(|ch| ch[cn].reference);
                State::with(|state| {
                    state.do_character_log(
                        co,
                        FontColor::Green,
                        &format!(
                            "{} tried to cast dispel magic on you but failed.\n",
                            c_string_to_str(&reference)
                        ),
                    )
                });
            }
        }
        return;
    }

    let removed_temp = Repository::with_items(|it| it[in_idx].temp);
    let removed_name = Repository::with_items(|it| it[in_idx].get_name().to_string());

    // Remove the spell item and unlink it from the target.
    Repository::with_items_mut(|it| it[in_idx].used = core::constants::USE_EMPTY);
    Repository::with_characters_mut(|ch| ch[co].spell[slot] = 0);
    State::with(|state| state.do_update_char(co));

    // Remember PvP attacks when dispelling non-curse from someone else.
    if target != 0 && removed_temp != SK_CURSE as u16 {
        State::with(|state| state.remember_pvp(cn, co));
    }

    let sound = Repository::with_characters(|ch| ch[cn].sound) as i32;

    if target != 0 {
        let sense = Repository::with_characters(|ch| ch[co].skill[SK_SENSE][5] as i32);
        if sense + 10 > dispel_skill {
            let reference = Repository::with_characters(|ch| ch[cn].reference);
            State::with(|state| {
                state.do_character_log(
                    co,
                    FontColor::Green,
                    &format!(
                        "{} cast dispel magic on you.\n",
                        c_string_to_str(&reference)
                    ),
                )
            });
        } else {
            State::with(|state| {
                state.do_character_log(
                    co,
                    FontColor::Green,
                    &format!("{} has been removed.\n", removed_name),
                )
            });
        }

        let target_name = Repository::with_characters(|ch| ch[co].get_name().to_string());
        State::with(|state| {
            state.do_character_log(
                cn,
                FontColor::Green,
                &format!("Removed {} from {}.\n", removed_name, target_name),
            )
        });

        // Match C: only notify (as an attack) when dispelling a non-curse spell from an NPC.
        let target_is_player =
            Repository::with_characters(|ch| (ch[co].flags & CharacterFlags::Player.bits()) != 0);
        if removed_temp != SK_CURSE as u16 && !target_is_player {
            if Repository::with_characters(|ch| {
                (ch[co].flags & CharacterFlags::SpellIgnore.bits()) == 0
            }) {
                State::with(|state| {
                    state.do_notify_character(co as u32, NT_GOTHIT as i32, cn as i32, 0, 0, 0)
                });
            }
            State::with(|state| {
                state.do_notify_character(cn as u32, NT_DIDHIT as i32, co as i32, 0, 0, 0)
            });
        }

        State::char_play_sound(co, sound + 1, -150, 0);
        State::char_play_sound(cn, sound + 1, -150, 0);
        chlog!(
            cn,
            "Cast Dispel on {}",
            Repository::with_characters(|ch| ch[co].get_name().to_string())
        );
        EffectManager::fx_add_effect(
            6,
            0,
            Repository::with_characters(|ch| ch[co].x) as i32,
            Repository::with_characters(|ch| ch[co].y) as i32,
            0,
        );
    } else {
        State::with(|state| {
            state.do_character_log(
                cn,
                FontColor::Green,
                &format!("{} has been removed.\n", removed_name),
            )
        });
        State::char_play_sound(cn, sound + 1, -150, 0);
        chlog!(cn, "Cast Dispel");
        EffectManager::fx_add_effect(
            6,
            0,
            Repository::with_characters(|ch| ch[cn].x) as i32,
            Repository::with_characters(|ch| ch[cn].y) as i32,
            0,
        );
    }

    add_exhaust(cn, TICKS * 2);
    EffectManager::fx_add_effect(
        7,
        0,
        Repository::with_characters(|ch| ch[cn].x) as i32,
        Repository::with_characters(|ch| ch[cn].y) as i32,
        0,
    );
}

pub fn skill_ghost(cn: usize) {
    // Check if in build mode
    if Repository::with_characters(|ch| (ch[cn].flags & CharacterFlags::BuildMode.bits()) != 0) {
        State::with(|state| state.do_character_log(cn, FontColor::Red, "Not in build mode.\n"));
        return;
    }

    // Check if player already has a companion
    let existing_companion = Repository::with_characters(|ch| {
        if (ch[cn].flags & CharacterFlags::Player.bits()) != 0 {
            let co = ch[cn].data[CHD_COMPANION] as usize;
            if co != 0 {
                // Validate companion still exists
                if Character::is_sane_character(co)
                    && ch[co].data[63] == cn as i32
                    && (ch[co].flags & CharacterFlags::Body.bits()) == 0
                    && ch[co].used != USE_EMPTY
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
    });

    if let Some(co) = existing_companion {
        State::with(|state| {
            state.do_character_log(
                cn,
                FontColor::Red,
                &format!("You may not have more than one Ghost Companion ({}).\n", co),
            )
        });
        return;
    }

    // Get target
    let mut co = Repository::with_characters(|ch| ch[cn].skill_target1 as usize);
    if co == cn {
        co = 0;
    }

    // Check visibility
    if co != 0 && State::with_mut(|state| state.do_char_can_see(cn, co)) == 0 {
        State::with(|state| {
            state.do_character_log(cn, FontColor::Red, "You cannot see your target.\n")
        });
        return;
    }

    if is_exhausted(cn) != 0 {
        return;
    }

    // Check if can attack target
    if co != 0 && State::with(|state| state.may_attack_msg(cn, co, true)) == 0 {
        chlog!(
            cn,
            "Prevented from attacking {} ({})",
            Repository::with_characters(|ch| ch[co].get_name().to_string()),
            co
        );
        return;
    }

    if spellcost(cn, 45) != 0 {
        return;
    }

    // No GC in Gatekeeper's room
    let (cx, cy) = Repository::with_characters(|ch| (ch[cn].x, ch[cn].y));
    if (39..=47).contains(&cx) && (594..=601).contains(&cy) {
        State::with(|state| {
            state.do_character_log(cn, FontColor::Red, "You must fight this battle alone.\n")
        });
        return;
    }

    // Chance check
    if chance(cn, 15) != 0 {
        if co != 0 && cn != co {
            let sense = Repository::with_characters(|ch| ch[co].skill[SK_SENSE][5] as i32);
            let ghost_skill = Repository::with_characters(|ch| ch[cn].skill[SK_GHOST][5] as i32);
            if sense > ghost_skill + 5 {
                let cn_ref = Repository::with_characters(|ch| ch[cn].reference);
                State::with(|state| {
                    state.do_character_log(
                        co,
                        FontColor::Green,
                        &format!(
                            "{} tried to cast ghost companion on you but failed.\n",
                            c_string_to_str(&cn_ref)
                        ),
                    )
                });
                if Repository::with_characters(|ch| {
                    (ch[co].flags & CharacterFlags::SpellIgnore.bits()) == 0
                }) {
                    State::with(|state| {
                        state.do_notify_character(co as u32, NT_GOTMISS as i32, cn as i32, 0, 0, 0)
                    });
                }
            }
        }
        return;
    }

    // Create companion
    let cc_opt = populate::pop_create_char(CT_COMPANION as usize, true);
    if cc_opt.is_none() {
        State::with(|state| {
            state.do_character_log(
                cn,
                FontColor::Red,
                "The ghost companion could not materialize.\n",
            )
        });
        return;
    }
    let cc = cc_opt.unwrap();

    let (cc_x, cc_y) = Repository::with_characters(|ch| (ch[cn].x as usize, ch[cn].y as usize));
    if !God::drop_char_fuzzy(cc, cc_x, cc_y) {
        Repository::with_characters_mut(|ch| {
            ch[cc].used = USE_EMPTY;
        });
        State::with(|state| {
            state.do_character_log(
                cn,
                FontColor::Red,
                "The ghost companion could not materialize.\n",
            )
        });
        return;
    }

    // Assign a randomized name to the companion
    let random_name = {
        let mut candidate = None;
        for _ in 0..100 {
            let name = core::names::randomly_generate_name();
            let name_exists = Repository::with_characters(|ch| {
                ch.iter().enumerate().any(|(idx, other)| {
                    idx != cc
                        && other.used != USE_EMPTY
                        && other.get_name().eq_ignore_ascii_case(&name)
                })
            });
            if !name_exists {
                candidate = Some(name);
                break;
            }
        }
        candidate.unwrap_or_else(core::names::randomly_generate_name)
    };

    Repository::with_characters_mut(|ch| {
        let companion = &mut ch[cc];

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
    });

    // Notify target and attacker
    if co != 0 {
        if Repository::with_characters(|ch| {
            (ch[co].flags & CharacterFlags::SpellIgnore.bits()) == 0
        }) {
            State::with(|state| {
                state.do_notify_character(co as u32, NT_GOTHIT as i32, cn as i32, 0, 0, 0)
            });
        }
        State::with(|state| {
            state.do_notify_character(cn as u32, NT_DIDHIT as i32, co as i32, 0, 0, 0)
        });
    }

    // Set player companion reference
    if Repository::with_characters(|ch| (ch[cn].flags & CharacterFlags::Player.bits()) != 0) {
        Repository::with_characters_mut(|ch| {
            ch[cn].data[CHD_COMPANION] = cc as i32;
        });
    }

    // Calculate base power
    let mut base = Repository::with_characters(|ch| (ch[cn].skill[SK_GHOST][5] as i32 * 4) / 11);
    let kindred = Repository::with_characters(|ch| ch[cn].kindred);
    base = spell_race_mod(base, kindred);

    let ticker = Repository::with_globals(|g| g.ticker);

    // Configure companion
    Repository::with_characters_mut(|ch| {
        ch[cc].data[29] = 0; // reset experience earned
        ch[cc].data[42] = 65536 + cn as i32; // set group
        ch[cc].kindred &= !(KIN_MONSTER as i32);
        ch[cc].flags &= !CharacterFlags::Player.bits();

        if co != 0 {
            ch[cc].attack_cn = co as u16;
            let idx = co as i32 | (helpers::char_id(co) << 16);
            ch[cc].data[80] = idx; // add enemy to kill list
        }

        ch[cc].data[63] = cn as i32;
        ch[cc].data[69] = cn as i32;

        if (ch[cn].flags & CharacterFlags::Player.bits()) != 0 {
            ch[cc].data[CHD_COMPANION] = 0; // player GCs stay forever
        } else {
            ch[cc].data[CHD_COMPANION] = ticker + TICKS * 60 * 5; // NPC GCs stay only for 5 minutes
        }
        ch[cc].data[98] = ticker + COMPANION_TIMEOUT;

        // Set text
        let text0 = b"#14#Yes! %s buys the farm!";
        ch[cc].text[0][..text0.len()].copy_from_slice(text0);
        let text1 = b"#13#Yahoo! An enemy! Prepare to die, %s!";
        ch[cc].text[1][..text1.len()].copy_from_slice(text1);
        let text3 = b"My successor will avenge me, %s!";
        ch[cc].text[3][..text3.len()].copy_from_slice(text3);

        ch[cc].data[48] = 33;
    });

    // Copy talkative setting from template
    Repository::with_character_templates(|templates| {
        Repository::with_characters_mut(|ch| {
            ch[cc].data[CHD_TALKATIVE] = templates[CT_COMPANION as usize].data[CHD_TALKATIVE];
        });
    });

    // Set attributes
    Repository::with_characters_mut(|ch| {
        for n in 0..5 {
            let mut tmp = base;
            tmp = tmp * 3 / std::cmp::max(1, ch[cc].attrib[n][3] as i32);
            ch[cc].attrib[n][0] =
                std::cmp::max(10, std::cmp::min(ch[cc].attrib[n][2] as i32, tmp) as u8);
        }
    });

    // Set skills
    Repository::with_characters_mut(|ch| {
        for n in 0..50 {
            let mut tmp = base;
            tmp = tmp * 3 / std::cmp::max(1, ch[cc].skill[n][3] as i32);
            if ch[cc].skill[n][2] != 0 {
                ch[cc].skill[n][0] = std::cmp::min(ch[cc].skill[n][2], tmp as u8);
            }
        }
    });

    // Set hp, end, mana
    Repository::with_characters_mut(|ch| {
        ch[cc].hp[0] = std::cmp::max(50, std::cmp::min(ch[cc].hp[2] as i32, base * 5)) as u16;
        ch[cc].end[0] = std::cmp::max(50, std::cmp::min(ch[cc].end[2] as i32, base * 5)) as u16;
        ch[cc].mana[0] = 0;
    });

    // Calculate experience points
    let mut pts = 0i32;

    let (attribs, hp0, end0, mana0, skills) = Repository::with_characters(|ch| {
        (
            ch[cc].attrib,
            ch[cc].hp[0],
            ch[cc].end[0],
            ch[cc].mana[0],
            ch[cc].skill,
        )
    });

    // Attributes
    for z in 0..5 {
        for m in 10..(attribs[z][0] as i32) {
            pts += helpers::attrib_needed(m, 3);
        }
    }

    // HP
    for m in 50..(hp0 as i32) {
        pts += helpers::hp_needed(m, 3);
    }

    // Endurance
    for m in 50..(end0 as i32) {
        pts += helpers::end_needed(m, 2);
    }

    // Mana
    for m in 50..(mana0 as i32) {
        pts += helpers::mana_needed(m, 3);
    }

    // Skills
    for z in 0..50 {
        for m in 1..(skills[z][0] as i32) {
            pts += helpers::skill_needed(m, 2);
        }
    }

    // Set points and action timers
    Repository::with_characters_mut(|ch| {
        ch[cc].points_tot = pts;
        ch[cc].gold = 0;
        ch[cc].a_hp = 999999;
        ch[cc].a_end = 999999;
        ch[cc].a_mana = 999999;

        // Set alignment
        ch[cc].alignment = ch[cn].alignment / 2;
    });

    // Set equipment bonuses based on attributes
    let (agil, stren) = Repository::with_characters(|ch| {
        (
            ch[cc].attrib[AT_AGIL as usize][0],
            ch[cc].attrib[AT_STREN as usize][0],
        )
    });

    Repository::with_characters_mut(|ch| {
        if agil >= 90 && stren >= 90 {
            // titanium
            ch[cc].armor_bonus = 48 + 32;
            ch[cc].weapon_bonus = 40 + 32;
        } else if agil >= 72 && stren >= 72 {
            // crystal
            ch[cc].armor_bonus = 36 + 28;
            ch[cc].weapon_bonus = 32 + 28;
        } else if agil >= 40 && stren >= 40 {
            // gold
            ch[cc].armor_bonus = 30 + 24;
            ch[cc].weapon_bonus = 24 + 24;
        } else if agil >= 24 && stren >= 24 {
            // steel
            ch[cc].armor_bonus = 24 + 20;
            ch[cc].weapon_bonus = 16 + 20;
        } else if agil >= 16 && stren >= 16 {
            // bronze
            ch[cc].armor_bonus = 18 + 16;
            ch[cc].weapon_bonus = 8 + 16;
        } else if agil >= 12 && stren >= 12 {
            // leather
            ch[cc].armor_bonus = 12 + 12;
            ch[cc].weapon_bonus = 8 + 12;
        } else if agil >= 10 && stren >= 10 {
            // cloth
            ch[cc].armor_bonus = 6 + 8;
            ch[cc].weapon_bonus = 8 + 8;
        }
    });

    let (cc_name, cn_ref) = Repository::with_characters(|ch| (ch[cc].name, ch[cn].reference));
    log::info!(
        "Created {} ({}) with base {} as Ghost Companion for {}",
        c_string_to_str(&cc_name),
        cc,
        base,
        c_string_to_str(&cn_ref)
    );

    // Make companion speak
    if co != 0 {
        let co_name = Repository::with_characters(|ch| ch[co].get_name().to_string());
        State::with(|state| {
            state.do_sayx(
                cc,
                &format!("#13#Yahoo! An enemy! Prepare to die, {}!", co_name),
            )
        });
    } else {
        let rank = core::ranks::points2rank(pts as u32);
        let cn_name = Repository::with_characters(|ch| ch[cn].get_name().to_string());
        if rank < 6 {
            // GC not yet Master Sergeant
            State::with(|state| {
                state.do_sayx(cc, &format!("I shall defend you and obey your commands, {}. I will WAIT, FOLLOW , be QUIET or ATTACK for you and tell you WHAT TIME. You may also command me to TRANSFER my experience to you, though I'd rather you didn't.\n", cn_name))
            });
        } else {
            State::with(|state| {
                state.do_sayx(cc, &format!("Thank you for creating me, {}!\n", cn_name))
            });
        }
    }

    State::with(|state| state.do_update_char(cc));

    add_exhaust(cn, TICKS * 4);

    let (cc_x, cc_y) = Repository::with_characters(|ch| (ch[cc].x as i32, ch[cc].y as i32));
    EffectManager::fx_add_effect(6, 0, cc_x, cc_y, 0);
    let (cn_x, cn_y) = Repository::with_characters(|ch| (ch[cn].x as i32, ch[cn].y as i32));
    EffectManager::fx_add_effect(7, 0, cn_x, cn_y, 0);
}

pub fn is_facing(cn: usize, co: usize) -> i32 {
    let dir = Repository::with_characters(|ch| ch[cn].dir);
    let cx = Repository::with_characters(|ch| ch[cn].x);
    let cy = Repository::with_characters(|ch| ch[cn].y);
    let ox = Repository::with_characters(|ch| ch[co].x);
    let oy = Repository::with_characters(|ch| ch[co].y);

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

pub fn is_back(cn: usize, co: usize) -> i32 {
    let dir = Repository::with_characters(|ch| ch[cn].dir);
    let cx = Repository::with_characters(|ch| ch[cn].x);
    let cy = Repository::with_characters(|ch| ch[cn].y);
    let ox = Repository::with_characters(|ch| ch[co].x);
    let oy = Repository::with_characters(|ch| ch[co].y);

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

pub fn nomagic(cn: usize) {
    State::with(|state| {
        state.do_character_log(
            cn,
            FontColor::Green,
            "Your magic fails. You seem to be unable to cast spells.\n",
        )
    });
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

pub fn skill_driver(cn: usize, nr: i32) {
    // Check whether the character can use this skill/spell
    if Repository::with_characters(|ch| ch[cn].skill[nr as usize][0] == 0) {
        State::with(|state| {
            state.do_character_log(cn, FontColor::Green, "You cannot use this skill/spell.\n")
        });
        return;
    }

    match nr {
        x if x == SK_LIGHT as i32 => {
            if Repository::with_characters(|ch| {
                (ch[cn].flags & CharacterFlags::NoMagic.bits()) != 0
            }) {
                nomagic(cn)
            } else {
                skill_light(cn)
            }
        }
        x if x == SK_PROTECT as i32 => {
            if Repository::with_characters(|ch| {
                (ch[cn].flags & CharacterFlags::NoMagic.bits()) != 0
            }) {
                nomagic(cn)
            } else {
                skill_protect(cn)
            }
        }
        x if x == SK_ENHANCE as i32 => {
            if Repository::with_characters(|ch| {
                (ch[cn].flags & CharacterFlags::NoMagic.bits()) != 0
            }) {
                nomagic(cn)
            } else {
                skill_enhance(cn)
            }
        }
        x if x == SK_BLESS as i32 => {
            if Repository::with_characters(|ch| {
                (ch[cn].flags & CharacterFlags::NoMagic.bits()) != 0
            }) {
                nomagic(cn)
            } else {
                skill_bless(cn)
            }
        }
        x if x == SK_CURSE as i32 => {
            if Repository::with_characters(|ch| {
                (ch[cn].flags & CharacterFlags::NoMagic.bits()) != 0
            }) {
                nomagic(cn)
            } else {
                skill_curse(cn)
            }
        }
        x if x == SK_IDENT as i32 => {
            if Repository::with_characters(|ch| {
                (ch[cn].flags & CharacterFlags::NoMagic.bits()) != 0
            }) {
                nomagic(cn)
            } else {
                skill_identify(cn)
            }
        }
        x if x == SK_BLAST as i32 => {
            if Repository::with_characters(|ch| {
                (ch[cn].flags & CharacterFlags::NoMagic.bits()) != 0
            }) {
                nomagic(cn)
            } else {
                skill_blast(cn)
            }
        }
        x if x == SK_REPAIR as i32 => skill_repair(cn),
        x if x == SK_LOCK as i32 => State::with(|state| {
            state.do_character_log(cn, FontColor::Green, "You cannot use this skill directly. Hold a lock-pick under your mouse cursor and click on the door.\n")
        }),
        x if x == SK_RECALL as i32 => {
            if Repository::with_characters(|ch| {
                (ch[cn].flags & CharacterFlags::NoMagic.bits()) != 0
            }) {
                nomagic(cn)
            } else {
                skill_recall(cn)
            }
        }
        x if x == SK_STUN as i32 => {
            if Repository::with_characters(|ch| {
                (ch[cn].flags & CharacterFlags::NoMagic.bits()) != 0
            }) {
                nomagic(cn)
            } else {
                skill_stun(cn)
            }
        }
        x if x == SK_DISPEL as i32 => {
            if Repository::with_characters(|ch| {
                (ch[cn].flags & CharacterFlags::NoMagic.bits()) != 0
            }) {
                nomagic(cn)
            } else {
                skill_dispel(cn)
            }
        }
        x if x == SK_WIMPY as i32 => {
            if Repository::with_characters(|ch| {
                (ch[cn].flags & CharacterFlags::NoMagic.bits()) != 0
            }) {
                nomagic(cn)
            } else {
                skill_wimp(cn)
            }
        }
        x if x == SK_HEAL as i32 => {
            if Repository::with_characters(|ch| {
                (ch[cn].flags & CharacterFlags::NoMagic.bits()) != 0
            }) {
                nomagic(cn)
            } else {
                skill_heal(cn)
            }
        }
        x if x == SK_GHOST as i32 => {
            if Repository::with_characters(|ch| {
                (ch[cn].flags & CharacterFlags::NoMagic.bits()) != 0
            }) {
                nomagic(cn)
            } else {
                skill_ghost(cn)
            }
        }
        x if x == SK_MSHIELD as i32 => {
            if Repository::with_characters(|ch| {
                (ch[cn].flags & CharacterFlags::NoMagic.bits()) != 0
            }) {
                nomagic(cn)
            } else {
                skill_mshield(cn)
            }
        }
        x if x == SK_IMMUN as i32 => State::with(|state| {
            state.do_character_log(
                cn,
                FontColor::Green,
                "You use this skill automatically when someone casts evil spells on you.\n",
            )
        }),
        x if x == SK_REGEN as i32 || x == SK_REST as i32 || x == SK_MEDIT as i32 => {
            State::with(|state| {
                state.do_character_log(
                    cn,
                    FontColor::Green,
                    "You use this skill automatically when you stand still.\n",
                )
            });
        }
        x if x == SK_DAGGER as i32
            || x == SK_SWORD as i32
            || x == SK_AXE as i32
            || x == SK_STAFF as i32
            || x == SK_TWOHAND as i32
            || x == SK_SURROUND as i32 =>
        {
            State::with(|state| {
                state.do_character_log(
                    cn,
                    FontColor::Green,
                    "You use this skill automatically when you fight.\n",
                )
            });
        }
        x if x == SK_CONCEN as i32 => State::with(|state| {
            state.do_character_log(
                cn,
                FontColor::Green,
                "You use this skill automatically when you cast spells.\n",
            )
        }),
        x if x == SK_WARCRY as i32 => skill_warcry(cn),
        _ => {
            State::with(|state| {
                state.do_character_log(cn, FontColor::Green, "You cannot use this skill/spell.\n")
            });
        }
    }
}
