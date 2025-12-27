use core::constants::ItemFlags;

use crate::{effect::EffectManager, enums, god::God, player, repository::Repository, state::State};

pub fn friend_is_enemy(cn: usize, cc: usize) -> i32 {
    // Rust port of C++ friend_is_enemy
    let co = Repository::with_characters(|ch| ch[cn].attack_cn as usize);
    if co == 0 {
        return 0;
    }

    if State::with(|state| state.may_attack_msg(cc, co, false)) != 0 {
        return 1;
    }
    0
}
pub fn player_or_ghost(cn: usize, co: usize) -> i32 {
    // Rust port of C++ player_or_ghost
    let cn_flags = Repository::with_characters(|ch| ch[cn].flags);
    if (cn_flags & core::constants::CharacterFlags::CF_PLAYER.bits() as u64) == 0 {
        return 0;
    }
    let co_flags = Repository::with_characters(|ch| ch[co].flags);
    if (co_flags & core::constants::CharacterFlags::CF_PLAYER.bits() as u64) != 0 {
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
        Repository::with_characters(|ch| ch[cn].skill[core::constants::SK_CONCEN as usize][0]);
    if concen_skill != 0 {
        let concen_val =
            Repository::with_characters(|ch| ch[cn].skill[core::constants::SK_CONCEN as usize][5]);
        let t = cost * concen_val as i32 / 300;
        if t > cost {
            cost = 1;
        } else {
            cost -= t;
        }
    }
    let a_mana = Repository::with_characters(|ch| ch[cn].a_mana as i32);
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
    if (flags & core::constants::CharacterFlags::CF_PLAYER.bits() as u64) != 0 {
        if luck < 0 {
            chance += luck / 500 - 1;
        }
    }
    if chance < 0 {
        chance = 0;
    }
    if chance > 18 {
        chance = 18;
    }
    let roll = rand::random::<u8>() % 20;
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
    if (flags & core::constants::CharacterFlags::CF_PLAYER.bits() as u64) != 0 {
        if luck < 0 {
            d20 += luck / 500 - 1;
        }
    }
    if d20 < 0 {
        d20 = 0;
    }
    if d20 > 18 {
        d20 = 18;
    }
    let roll = rand::random::<u8>() % 20;
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
    let mut modf = 1.0;
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
    let mut in2 = 0;
    let mut weak = 999;
    let mut weakest = 99;
    let m = Repository::with_characters(|ch| {
        ch[cn].x as usize + ch[cn].y as usize * core::constants::SERVER_MAPX as usize
    });
    let nomagic =
        Repository::with_map(|map| map[m].flags & enums::CharacterFlags::NoMagic.bits() != 0);
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

pub fn add_exhaust(cn: usize, len: i32) {
    // Ported from C++ add_exhaust(int cn, int len)
    use core::constants::*;
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
        item.duration = len as u32;
        item.active = len as u32;
        item.temp = SK_BLAST as u16;
        item.power = 255;
    });
    add_spell(cn, in_.unwrap());
}

pub fn spell_from_item(cn: usize, in2: usize) {
    // Ported from C++ spell_from_item(int cn, int in2)
    let flags = Repository::with_characters(|ch| ch[cn].flags);
    if (flags & enums::CharacterFlags::NoMagic.bits() as u64) != 0 {
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
    Repository::with_items_mut(|it| {
        it[in_.unwrap()].name = Repository::with_items(|it2| it2[in2].name.clone());
        it[in_.unwrap()].flags |= ItemFlags::IF_SPELL.bits();
        it[in_.unwrap()].armor[1] = Repository::with_items(|it2| it2[in2].armor[1]);
        it[in_.unwrap()].weapon[1] = Repository::with_items(|it2| it2[in2].weapon[1]);
        it[in_.unwrap()].hp[1] = Repository::with_items(|it2| it2[in2].hp[1]);
        it[in_.unwrap()].end[1] = Repository::with_items(|it2| it2[in2].end[1]);
        it[in_.unwrap()].mana[1] = Repository::with_items(|it2| it2[in2].mana[1]);
        it[in_.unwrap()].sprite_override = Repository::with_items(|it2| it2[in2].sprite_override);
        for n in 0..5 {
            it[in_.unwrap()].attrib[n][1] = Repository::with_items(|it2| it2[in2].attrib[n][1]);
        }
        for n in 0..50 {
            it[in_.unwrap()].skill[n][1] = Repository::with_items(|it2| it2[in2].skill[n][1]);
        }
        let data0 = Repository::with_items(|it2| it2[in2].data[0]);
        if data0 != 0 {
            it[in_.unwrap()].sprite[1] = data0 as i16;
        } else {
            it[in_.unwrap()].sprite[1] = 93;
        }
        let duration = Repository::with_items(|it2| it2[in2].duration);
        it[in_.unwrap()].duration = duration;
        it[in_.unwrap()].active = duration;
        let data1 = Repository::with_items(|it2| it2[in2].data[1]);
        if data1 != 0 {
            it[in_.unwrap()].temp = data1 as u16;
        } else {
            it[in_.unwrap()].temp = 101;
        }
        it[in_.unwrap()].power = Repository::with_items(|it2| it2[in2].power);
    });
    if add_spell(cn, in_.unwrap()) == 0 {
        let name = Repository::with_items(|it| it[in_.unwrap()].name.clone());
        State::with(|state| {
            state.do_character_log(
                cn,
                core::types::FontColor::Green,
                &format!(
                    "Magical interference neutralised the {}'s effect.\n",
                    String::from_utf8_lossy(&name)
                ),
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
    let power = spell_race_mod(
        power,
        Repository::with_characters(|ch| ch[cn].kindred as i32),
    );
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
            let name = Repository::with_items(|it| it[in_.unwrap()].name.clone());
            State::with(|state| {
                state.do_character_log(
                    cn,
                    core::types::FontColor::Green,
                    &format!(
                        "Magical interference neutralised the {}'s effect.\n",
                        String::from_utf8_lossy(&name)
                    ),
                );
            });
            return 0;
        }
        let sense =
            Repository::with_characters(|ch| ch[co].skill[core::constants::SK_SENSE as usize][5]);
        if sense + 10 > power as u8 {
            let reference = Repository::with_characters(|ch| ch[cn].reference.clone());
            State::with(|state| {
                state.do_character_log(
                    co,
                    core::types::FontColor::Green,
                    &format!(
                        "{} cast light on you.\n",
                        String::from_utf8_lossy(&reference)
                    ),
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
        let name = Repository::with_characters(|ch| ch[co].name.clone());
        let (x, y) = Repository::with_characters(|ch| (ch[co].x, ch[co].y));
        State::with(|state| {
            state.do_area_log(
                co,
                0,
                x as i32,
                y as i32,
                core::types::FontColor::Green,
                &format!("{} starts to emit light.\n", String::from_utf8_lossy(&name)),
            )
        });
        let sound = Repository::with_characters(|ch| ch[cn].sound);
        State::char_play_sound(co, sound as i32 + 1, -150, 0);
        State::char_play_sound(cn, sound as i32 + 1, -150, 0);
        let (x, y) = Repository::with_characters(|ch| (ch[co].x, ch[co].y));
        EffectManager::fx_add_effect(7, 0, x as i32, y as i32, 0);
    } else {
        if add_spell(cn, in_.unwrap()) == 0 {
            let name = Repository::with_items(|it| it[in_.unwrap()].name.clone());
            State::with(|state| {
                state.do_character_log(
                    cn,
                    core::types::FontColor::Green,
                    &format!(
                        "Magical interference neutralised the {}'s effect.\n",
                        String::from_utf8_lossy(&name)
                    ),
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
        if (flags & enums::CharacterFlags::Player.bits() as u64) != 0 {
            // TODO State::with(|state| state.chlog(cn, "Cast Light"));
        }
        let (x, y) = Repository::with_characters(|ch| (ch[cn].x, ch[cn].y));
        EffectManager::fx_add_effect(7, 0, x as i32, y as i32, 0);
    }
    let (x, y) = Repository::with_characters(|ch| (ch[cn].x, ch[cn].y));
    EffectManager::fx_add_effect(7, 0, x as i32, y as i32, 0);
    1
}

pub fn skill_light(cn: usize) {
    unimplemented!()
}

pub fn spellpower(cn: usize) -> i32 {
    unimplemented!()
}

pub fn spell_protect(cn: usize, co: usize, power: i32) -> i32 {
    unimplemented!()
}

pub fn skill_protect(cn: usize) {
    unimplemented!()
}

pub fn spell_enhance(cn: usize, co: usize, power: i32) -> i32 {
    unimplemented!()
}

pub fn skill_enhance(cn: usize) {
    unimplemented!()
}

pub fn spell_bless(cn: usize, co: usize, power: i32) -> i32 {
    unimplemented!()
}

pub fn skill_bless(cn: usize) {
    unimplemented!()
}

pub fn skill_wimp(cn: usize) {
    unimplemented!()
}

pub fn spell_mshield(cn: usize, co: usize, power: i32) -> i32 {
    unimplemented!()
}

pub fn skill_mshield(cn: usize) {
    unimplemented!()
}

pub fn spell_heal(cn: usize, co: usize, power: i32) -> i32 {
    unimplemented!()
}

pub fn skill_heal(cn: usize) {
    unimplemented!()
}

pub fn spell_curse(cn: usize, co: usize, power: i32) -> i32 {
    unimplemented!()
}

pub fn skill_curse(cn: usize) {
    unimplemented!()
}

pub fn warcry(cn: usize, co: usize, power: i32) -> i32 {
    unimplemented!()
}

pub fn skill_warcry(cn: usize) {
    unimplemented!()
}

pub fn item_info(cn: usize, in_: usize, look: i32) {
    unimplemented!()
}

pub fn char_info(cn: usize, co: usize) {
    unimplemented!()
}

pub fn skill_identify(cn: usize) {
    unimplemented!()
}

pub fn skill_blast(cn: usize) {
    unimplemented!()
}

pub fn skill_repair(cn: usize) {
    unimplemented!()
}

pub fn skill_recall(cn: usize) {
    unimplemented!()
}

pub fn spell_stun(cn: usize, co: usize, power: i32) -> i32 {
    unimplemented!()
}

pub fn skill_stun(cn: usize) {
    unimplemented!()
}

pub fn remove_spells(cn: usize) {
    unimplemented!()
}

pub fn skill_dispel(cn: usize) {
    unimplemented!()
}

pub fn skill_ghost(cn: usize) {
    unimplemented!()
}

pub fn is_facing(cn: usize, co: usize) -> i32 {
    unimplemented!()
}

pub fn is_back(cn: usize, co: usize) -> i32 {
    unimplemented!()
}

pub fn nomagic(cn: usize) {
    unimplemented!()
}

pub fn skill_lookup(name: &str) -> i32 {
    unimplemented!()
}

pub fn skill_driver(cn: usize, nr: i32) {
    unimplemented!()
}
