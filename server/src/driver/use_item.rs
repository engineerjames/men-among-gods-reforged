use crate::area;
use crate::core::types::skilltab;
use crate::effect::EffectManager;
use crate::game_state::GameState;
use crate::god::God;
use crate::helpers::{self};
use crate::lab9::Labyrinth9;
use crate::{chlog, driver, player, populate};
use core::constants::{
    CharacterFlags, ItemFlags, AT_AGIL, AT_INT, AT_STREN, AT_WILL, DX_RIGHT, KIN_HARAKIM, KIN_MALE,
    KIN_MERCENARY, KIN_SEYAN_DU, KIN_SORCERER, KIN_TEMPLAR, KIN_WARRIOR, MAXITEM, MAXSKILL,
    MAXTITEM, MF_NOEXPIRE, NT_HITME, SERVER_MAPX, SERVER_MAPY, SK_BLAST, SK_BLESS, SK_CONCEN,
    SK_CURSE, SK_DAGGER, SK_ENHANCE, SK_GHOST, SK_HEAL, SK_IMMUN, SK_LIGHT, SK_LOCK, SK_MSHIELD,
    SK_PROTECT, SK_RECALL, SK_RESIST, SK_STEALTH, SK_STUN, SK_SURROUND, SK_SWORD, SK_TWOHAND,
    SK_WARCRY, TICKS, USE_ACTIVE, USE_EMPTY, WN_RHAND,
};
use core::string_operations::c_string_to_str;
use core::types::FontColor;

// Helper function to take an item from a character
fn take_item_from_char(gs: &mut GameState, item_idx: usize, cn: usize) {
    {
        let ch = &mut gs.characters[cn];

        if ch.citem as usize == item_idx {
            ch.citem = 0;
        } else {
            for n in 0..40 {
                if ch.item[n] as usize == item_idx {
                    ch.item[n] = 0;
                    break;
                }
            }

            for n in 0..20 {
                if ch.worn[n] as usize == item_idx {
                    ch.worn[n] = 0;
                    break;
                }
            }
        }
    }

    gs.items[item_idx].x = 0;
    gs.items[item_idx].y = 0;
    gs.items[item_idx].carried = 0;
    gs.do_update_char(cn);
}

pub fn sub_door_driver(gs: &mut GameState, _cn: usize, item_idx: usize) -> i32 {
    let item = &gs.items[item_idx];

    if item.data[0] == 65500 {
        return 0;
    }

    if item.data[0] == 65501 || item.data[0] == 65502 {
        let mut empty = 0;
        let mut star = 0;
        let mut circle = 0;
        let loctab: [usize; 4] = [344487, 343463, 344488, 343464];

        for (n, map_idx) in loctab.iter().copied().enumerate() {
            let in2 = gs.map[map_idx].it as usize;
            if in2 == 0 || gs.items[in2].data[1] != n as u32 {
                continue;
            }

            match gs.items[in2].temp {
                761 => star += 1,
                762 => circle += 1,
                763 => empty += 1,
                _ => {}
            }
        }

        return if item.data[0] == 65501 && empty == 3 && star == 1 {
            1
        } else if item.data[0] == 65502 && empty == 3 && circle == 1 {
            1
        } else {
            0
        };
    }

    0
}

pub fn use_door(gs: &mut GameState, cn: usize, item_idx: usize) -> i32 {
    let map_idx =
        gs.items[item_idx].x as usize + gs.items[item_idx].y as usize * SERVER_MAPX as usize;
    let blocked = gs.map[map_idx].ch != 0;
    if blocked {
        return 0;
    }

    let mut lock = 0;
    let mut key_vanishes = false;
    let mut key_slot: Option<usize> = None;

    // Check lock requirements
    let locked_without_key = {
        let lock_code = gs.items[item_idx].data[0];
        if lock_code != 0 {
            if cn == 0 {
                lock = 1;
            } else if lock_code >= 65500 {
                lock = sub_door_driver(gs, cn, item_idx);
            } else {
                let character = &gs.characters[cn];
                let citem = character.citem as usize;

                if citem != 0
                    && (citem & 0x80000000) == 0
                    && gs.items[citem].temp == lock_code as u16
                {
                    lock = 1;
                    if gs.items[item_idx].data[3] != 0 {
                        key_vanishes = true;
                        key_slot = None;
                    }
                } else {
                    for n in 0..40 {
                        let in2 = character.item[n] as usize;
                        if in2 != 0 && gs.items[in2].temp == lock_code as u16 {
                            lock = 1;
                            if gs.items[item_idx].data[3] != 0 {
                                key_vanishes = true;
                                key_slot = Some(n);
                            }
                            break;
                        }
                    }
                }

                if lock == 0 {
                    let citem = character.citem as usize;
                    if citem != 0 && (citem & 0x80000000) == 0 && gs.items[citem].driver == 3 {
                        let skill = character.skill[SK_LOCK][5] + gs.items[citem].data[0] as u8;
                        let power = gs.items[item_idx].data[2];

                        if power == 0 || skill >= (power + helpers::random_mod(20)) as u8 {
                            lock = 1;
                        } else {
                            gs.do_character_log(
                                cn,
                                core::types::FontColor::Blue,
                                "You failed to pick the lock.\n",
                            );
                        }
                        item_damage_citem(gs, cn, 1);
                    }
                }
            }

            gs.items[item_idx].data[1] != 0 && lock == 0
        } else {
            false
        }
    };

    // If door is locked and player doesn't have key, exit early
    if locked_without_key {
        gs.do_character_log(
            cn,
            core::types::FontColor::Blue,
            "It's locked and you don't have the right key.\n",
        );
        return 0;
    }

    if key_vanishes {
        if let Some(slot) = key_slot {
            let consumed_item_idx = gs.characters[cn].item[slot] as usize;
            gs.characters[cn].item[slot] = 0;
            gs.items[consumed_item_idx].used = USE_EMPTY;
        } else {
            let consumed_item_idx = gs.characters[cn].citem as usize;
            gs.characters[cn].citem = 0;
            gs.items[consumed_item_idx].used = USE_EMPTY;
        }
        gs.do_character_log(cn, core::types::FontColor::Yellow, "The key vanished.\n");
    }

    let (item_x, item_y, temp, active) = {
        let item = &gs.items[item_idx];
        (
            item.x as i32,
            item.y as i32,
            item.temp as usize,
            item.active,
        )
    };

    gs.reset_go(item_x, item_y);
    gs.remove_lights(item_x, item_y);
    gs.do_area_sound(0, 0, item_x, item_y, 10);

    if active == 0 {
        let item = &mut gs.items[item_idx];
        item.flags &= !(ItemFlags::IF_MOVEBLOCK.bits() | ItemFlags::IF_SIGHTBLOCK.bits());
        item.data[1] = 0;
    } else {
        let sight_flags = gs.item_templates[temp].flags & ItemFlags::IF_SIGHTBLOCK.bits();
        let item = &mut gs.items[item_idx];
        item.flags |= ItemFlags::IF_MOVEBLOCK.bits() | sight_flags;
        if lock != 0 {
            item.data[1] = 1;
        }
    }

    gs.reset_go(item_x, item_y);
    gs.add_lights(item_x, item_y);
    let ch = &gs.characters[cn];
    gs.do_area_notify(
        cn as i32,
        0,
        ch.x as i32,
        ch.y as i32,
        core::constants::NT_SEE as i32,
        cn as i32,
        0,
        0,
        0,
    );

    1
}

pub fn use_create_item(gs: &mut GameState, cn: usize, item_idx: usize) -> i32 {
    if cn == 0 {
        return 0;
    }

    let (active, template_id) = (
        gs.items[item_idx].active,
        gs.items[item_idx].data[0] as usize,
    );

    if active != 0 {
        return 0;
    }

    if template_id <= 0 || template_id >= MAXTITEM {
        return 0;
    }

    let in2 = match God::create_item(gs, template_id) {
        Some(id) => id,
        None => return 0,
    };

    if !God::give_character_item(gs, cn, in2) {
        let item_ref = gs.items[in2].reference;
        gs.do_character_log(
            cn,
            core::types::FontColor::Blue,
            &format!(
                "Your backpack is full, so you can't take the {}.\n",
                c_string_to_str(&item_ref)
            ),
        );
        gs.items[in2].used = core::constants::USE_EMPTY;
        return 0;
    }

    let item_ref = gs.items[in2].reference;
    let item_name = gs.items[in2].get_name().to_string();
    let source_name = gs.items[item_idx].get_name().to_string();
    gs.do_character_log(
        cn,
        core::types::FontColor::Green,
        &format!("You got a {}.\n", c_string_to_str(&item_ref)),
    );
    log::info!("Character {} got {} from {}", cn, item_name, source_name);

    let driver = gs.items[item_idx].driver;
    let data1 = gs.items[item_idx].data[1];

    if data1 != 0 && driver == 53 {
        let char_name = gs.characters[cn].get_name().to_string();
        let item_ref = c_string_to_str(&gs.items[in2].reference).to_string();
        gs.do_character_log(
            cn,
            core::types::FontColor::Blue,
            &format!(
                "You feel yourself form a magical connection with the {}.\n",
                item_ref
            ),
        );

        let item = &mut gs.items[in2];
        item.data[0] = cn as u32;

        let new_desc = format!(
            "{} Engraved in it are the letters \"{}\".",
            c_string_to_str(&item.description),
            char_name,
        );
        if new_desc.len() < 200 {
            let bytes = new_desc.as_bytes();
            item.description[..bytes.len()].copy_from_slice(bytes);
            if bytes.len() < 200 {
                item.description[bytes.len()..].fill(0);
            }
        }
    }

    if driver == 54 {
        let (x, y) = (gs.items[item_idx].x as i32, gs.items[item_idx].y as i32);
        gs.do_area_notify(
            cn as i32,
            0,
            x,
            y,
            core::constants::NT_HITME as i32,
            cn as i32,
            0,
            0,
            0,
        );
    }

    1
}

pub fn use_create_gold(gs: &mut GameState, cn: usize, item_idx: usize) -> i32 {
    if cn == 0 {
        return 0;
    }

    let (active, gold_amount) = (gs.items[item_idx].active, gs.items[item_idx].data[0]);

    if active != 0 {
        return 0;
    }

    let gold_to_add = gold_amount * 100;
    gs.characters[cn].gold += gold_to_add as i32;
    gs.do_character_log(
        cn,
        core::types::FontColor::Green,
        &format!("You got a {}G.\n", gold_amount),
    );
    log::info!(
        "Character {} got {}G from {}",
        cn,
        gold_amount,
        gs.items[item_idx]
            .name
            .iter()
            .take_while(|&&c| c != 0)
            .map(|&c| c as char)
            .collect::<String>()
    );

    1
}

pub fn use_create_item2(gs: &mut GameState, cn: usize, item_idx: usize) -> i32 {
    if cn == 0 {
        return 0;
    }

    let (active, required_temp, template_id) = (
        gs.items[item_idx].active,
        gs.items[item_idx].data[1],
        gs.items[item_idx].data[0] as usize,
    );

    if active != 0 {
        return 0;
    }

    // Check if character has the required item in citem
    let citem = gs.characters[cn].citem as usize;

    if citem == 0 || (citem & 0x80000000) != 0 {
        return 0;
    }

    let citem_temp = gs.items[citem].temp;

    if citem_temp as u32 != required_temp {
        return 0;
    }

    if template_id <= 0 || template_id >= MAXTITEM {
        return 0;
    }

    let in2 = match God::create_item(gs, template_id) {
        Some(id) => id,
        None => return 0,
    };

    if !God::give_character_item(gs, cn, in2) {
        let item_ref = c_string_to_str(&gs.items[in2].reference);
        gs.do_character_log(
            cn,
            core::types::FontColor::Blue,
            &format!(
                "Your backpack is full, so you can't take the {}.\n",
                item_ref
            ),
        );
        gs.items[in2].used = USE_EMPTY;
        return 0;
    }

    let item_ref = c_string_to_str(&gs.items[in2].reference);
    gs.do_character_log(
        cn,
        core::types::FontColor::Green,
        &format!("You got a {}.\n", item_ref),
    );
    chlog!(
        cn,
        "Got {} from {}",
        gs.items[in2].get_name(),
        gs.items[item_idx].get_name()
    );

    gs.items[citem].used = USE_EMPTY;
    gs.characters[cn].citem = 0;

    1
}

pub fn use_create_item3(gs: &mut GameState, cn: usize, item_idx: usize) -> i32 {
    if cn == 0 {
        return 0;
    }

    let active = gs.items[item_idx].active;

    if active != 0 {
        return 0;
    }

    // Find how many data entries are non-zero
    let data_entries = {
        let item_data = gs.items[item_idx].data;
        let mut count = 0;
        for n in 0..10 {
            if item_data[n] == 0 {
                break;
            }
            count += 1;
        }
        if count == 0 {
            None
        } else {
            Some((count, item_data))
        }
    };

    let (count, data) = match data_entries {
        Some(v) => v,
        None => return 0,
    };

    // Pick a random entry
    let n = helpers::random_mod_usize(count);
    let template_id = data[n] as usize;

    if template_id <= 0 || template_id >= MAXTITEM {
        return 0;
    }

    // Check if this is a special item template
    let in2 = match template_id {
        57 | 59 | 63 | 65 | 69 | 71 | 75 | 76 | 94 | 95 | 981 | 982 => {
            helpers::create_special_item(gs, template_id)
        }
        _ => God::create_item(gs, template_id),
    };

    let in2 = match in2 {
        Some(id) => id,
        None => {
            gs.do_character_log(cn, core::types::FontColor::Green, "It's empty...\n");
            return 1;
        }
    };

    if !God::give_character_item(gs, cn, in2) {
        gs.do_character_log(
            cn,
            core::types::FontColor::Blue,
            "Your backpack is full, so you can't take anything.\n",
        );
        gs.items[in2].used = USE_EMPTY;
        return 0;
    }

    let item_ref = c_string_to_str(&gs.items[in2].reference);
    gs.do_character_log(
        cn,
        core::types::FontColor::Green,
        &format!("You got a {}.\n", item_ref),
    );
    chlog!(
        cn,
        "Got {} from {}",
        gs.items[in2].get_name(),
        gs.items[item_idx].get_name()
    );

    1
}

pub fn use_mix_potion(gs: &mut GameState, cn: usize, item_idx: usize) -> i32 {
    if cn == 0 {
        return 0;
    }

    let citem = gs.characters[cn].citem as usize;

    if citem == 0 || (citem & 0x80000000) != 0 {
        gs.do_character_log(
            cn,
            core::types::FontColor::Blue,
            "What do you want to do with it?",
        );
        return 0;
    }

    let carried = gs.items[item_idx].carried;
    if carried == 0 {
        gs.do_character_log(
            cn,
            core::types::FontColor::Blue,
            "Too difficult to do on the ground.\n",
        );
        return 0;
    }

    let (base_temp, ingredient_temp) = (gs.items[item_idx].temp, gs.items[citem].temp);

    let result_template: Option<usize> = match base_temp {
        100 => match ingredient_temp {
            18 => Some(101),
            46 => Some(102),
            141 => Some(145),
            140 => Some(144),
            142 => Some(143),
            197 => Some(219),
            198 => Some(220),
            199 => Some(218),
            294 => Some(295),
            _ => None,
        },
        143 | 145 | 146 => match ingredient_temp {
            18 | 46 | 140 | 141 | 142 | 197 | 198 | 199 | 294 => Some(146),
            _ => None,
        },
        144 => match ingredient_temp {
            18 | 46 | 140 | 141 | 197 | 198 | 199 | 294 => Some(146),
            142 => Some(147),
            _ => None,
        },
        147 => match ingredient_temp {
            18 | 46 | 140 | 142 | 197 | 198 | 199 | 294 => Some(146),
            141 => Some(148),
            _ => None,
        },
        218 => match ingredient_temp {
            18 | 46 | 141 | 140 | 142 | 199 | 294 => Some(146),
            197 => Some(223),
            198 => Some(221),
            _ => None,
        },
        219 => match ingredient_temp {
            18 | 46 | 141 | 140 | 142 | 197 | 294 => Some(146),
            198 => Some(222),
            199 => Some(223),
            _ => None,
        },
        220 => match ingredient_temp {
            18 | 46 | 141 | 140 | 142 | 198 | 294 => Some(146),
            197 => Some(222),
            199 => Some(221),
            _ => None,
        },
        221 => match ingredient_temp {
            18 | 46 | 141 | 140 | 142 | 198 | 199 | 294 => Some(146),
            197 => Some(224),
            _ => None,
        },
        222 => match ingredient_temp {
            18 | 46 | 141 | 140 | 142 | 197 | 198 | 294 => Some(146),
            199 => Some(224),
            _ => None,
        },
        223 => match ingredient_temp {
            18 | 46 | 141 | 140 | 142 | 197 | 199 | 294 => Some(146),
            198 => Some(224),
            _ => None,
        },
        295 => match ingredient_temp {
            18 | 46 | 141 | 140 | 142 | 197 | 198 | 199 | 294 => Some(146),
            _ => None,
        },
        _ => None,
    };

    let result_template = match result_template {
        Some(t) => t,
        None => {
            gs.do_character_log(cn, core::types::FontColor::Blue, "Sorry?\n");
            return 0;
        }
    };

    let in3 = match God::create_item(gs, result_template) {
        Some(id) => id,
        None => return 0,
    };

    gs.items[in3].flags |= ItemFlags::IF_UPDATE.bits();
    gs.items[citem].used = USE_EMPTY;
    gs.items[item_idx].used = USE_EMPTY;
    gs.characters[cn].citem = 0;

    take_item_from_char(gs, item_idx, cn);
    God::give_character_item(gs, cn, in3);

    1
}

pub fn use_chain(gs: &mut GameState, cn: usize, item_idx: usize) -> i32 {
    if cn == 0 {
        return 0;
    }

    let citem = gs.characters[cn].citem as usize;

    if citem == 0 || (citem & 0x80000000) != 0 {
        gs.do_character_log(
            cn,
            core::types::FontColor::Blue,
            "What do you want to do with it?\n",
        );
        return 0;
    }

    let carried = gs.items[item_idx].carried;
    if carried == 0 {
        gs.do_character_log(
            cn,
            core::types::FontColor::Blue,
            "Too difficult to do on the ground.\n",
        );
        return 0;
    }

    let citem_temp = gs.items[citem].temp;
    if citem_temp != 206 {
        gs.do_character_log(cn, core::types::FontColor::Blue, "Sorry?\n");
        return 0;
    }

    let (current_temp, max_data) = (gs.items[item_idx].temp as i32, gs.items[item_idx].data[0]);

    if current_temp as u32 >= max_data {
        gs.do_character_log(cn, core::types::FontColor::Blue, "It won't fit anymore.\n");
        return 0;
    }

    let in3 = match God::create_item(gs, (current_temp + 1) as usize) {
        Some(id) => id,
        None => return 0,
    };

    gs.items[in3].flags |= ItemFlags::IF_UPDATE.bits();
    gs.items[citem].used = USE_EMPTY;
    gs.items[item_idx].used = USE_EMPTY;
    gs.characters[cn].citem = 0;

    take_item_from_char(gs, item_idx, cn);
    God::give_character_item(gs, cn, in3);

    1
}

pub fn stone_sword(gs: &mut GameState, cn: usize, item_idx: usize) -> i32 {
    if cn == 0 {
        log::error!("stone_sword called with cn=0");
        return 0;
    }

    let (active, template_id) = (
        gs.items[item_idx].active,
        gs.items[item_idx].data[0] as usize,
    );

    if active != 0 {
        log::error!("stone_sword called on active item");
        return 0;
    }

    if template_id <= 0 || template_id >= MAXTITEM {
        log::error!(
            "stone_sword called with invalid template_id: {}",
            template_id
        );
        return 0;
    }

    // Check if character has enough strength (100+)
    let strength = gs.characters[cn].attrib[AT_STREN as usize][5];

    if strength < 100 {
        gs.do_character_log(
            cn,
            core::types::FontColor::Blue,
            "You're not strong enough.\n",
        );
        return 0;
    }

    let in2 = match God::create_item(gs, template_id) {
        Some(id) => id,
        None => return 0,
    };

    God::give_character_item(gs, cn, in2);

    let item_ref = c_string_to_str(&gs.items[in2].reference).to_string();
    gs.do_character_log(
        cn,
        core::types::FontColor::Green,
        &format!("You got a {}.\n", item_ref),
    );

    1
}

pub fn finish_laby_teleport(gs: &mut GameState, cn: usize, nr: usize, exp: usize) -> i32 {
    let current_progress = gs.characters[cn].data[20];
    let (x, y) = (gs.characters[cn].x, gs.characters[cn].y);

    if (current_progress as usize) < nr {
        gs.characters[cn].data[20] = nr as i32;

        let ordinal = match nr {
            1 => "st",
            2 => "nd",
            3 => "rd",
            _ => "th",
        };

        gs.do_character_log(
            cn,
            core::types::FontColor::Green,
            &format!(
                "You have solved the {}{} part of the Labyrinth.\n",
                nr, ordinal
            ),
        );

        gs.do_give_exp(cn, exp as i32, 0, -1)
    }

    let citem = gs.characters[cn].citem;
    if citem != 0 && (citem & 0x80000000) == 0 {
        let citem = citem as usize;
        if (gs.items[citem].flags & ItemFlags::IF_LABYDESTROY.bits()) != 0 {
            let item_ref = c_string_to_str(&gs.items[citem].reference).to_string();
            gs.characters[cn].citem = 0;
            gs.items[citem].used = USE_EMPTY;
            gs.do_character_log(
                cn,
                core::types::FontColor::Yellow,
                &format!("Your {} vanished.\n", item_ref),
            );
        }
    }

    for n in 0..40 {
        let item_idx = gs.characters[cn].item[n] as usize;
        if item_idx != 0 {
            if (gs.items[item_idx].flags & ItemFlags::IF_LABYDESTROY.bits()) != 0 {
                let item_ref = c_string_to_str(&gs.items[item_idx].reference).to_string();
                gs.characters[cn].item[n] = 0;
                gs.items[item_idx].used = USE_EMPTY;
                gs.do_character_log(
                    cn,
                    core::types::FontColor::Yellow,
                    &format!("Your {} vanished.\n", item_ref),
                );
            }
        }
    }

    for n in 0..20 {
        let item_idx = gs.characters[cn].worn[n] as usize;
        if item_idx != 0 {
            if (gs.items[item_idx].flags & ItemFlags::IF_LABYDESTROY.bits()) != 0 {
                let item_ref = c_string_to_str(&gs.items[item_idx].reference).to_string();
                gs.characters[cn].worn[n] = 0;
                gs.items[item_idx].used = USE_EMPTY;
                gs.do_character_log(
                    cn,
                    core::types::FontColor::Yellow,
                    &format!("Your {} vanished.\n", item_ref),
                );
            }
        }
    }

    for n in 0..20 {
        let spell_idx = gs.characters[cn].spell[n] as usize;
        if spell_idx != 0 {
            let item_name = c_string_to_str(&gs.items[spell_idx].name).to_string();
            gs.characters[cn].spell[n] = 0;
            gs.items[spell_idx].used = USE_EMPTY;
            gs.do_character_log(
                cn,
                core::types::FontColor::Yellow,
                &format!("Your {} vanished.\n", item_name),
            );
        }
    }

    // Add effects and transfer character
    EffectManager::fx_add_effect(6, 0, x as i32, y as i32, 0);
    God::transfer_char(gs, cn, 512, 512); // TODO: Shouldn't this be their temple coords?
    EffectManager::fx_add_effect(6, 0, 512_i32, 512_i32, 0);

    let (x, y) = (gs.characters[cn].x, gs.characters[cn].y);
    gs.characters[cn].temple_x = x as u16;
    gs.characters[cn].temple_y = y as u16;
    gs.characters[cn].tavern_x = x as u16;
    gs.characters[cn].tavern_y = y as u16;

    1
}

pub fn is_nolab_item(gs: &mut GameState, item_idx: usize) -> bool {
    if !core::types::Item::is_sane_item(item_idx) {
        return false;
    }
    let temp = gs.items[item_idx].temp;
    matches!(
        temp,
        331   // tavern scroll
        | 500   // lag scroll
        | 592   // gorn scroll
        | 903   // forest scroll
        | 1114  // staffers corner scroll
        | 1118  // inn scroll
        | 1144 // arena scroll
    )
}

pub fn teleport(gs: &mut GameState, cn: usize, item_idx: usize) -> i32 {
    if cn == 0 {
        return 1;
    }

    let has_useactivate = (gs.items[item_idx].flags & ItemFlags::IF_USEACTIVATE.bits()) != 0;
    let is_active = gs.items[item_idx].active != 0;

    if has_useactivate && !is_active {
        return 1;
    }

    let citem = gs.characters[cn].citem as usize;
    let (x, y) = (gs.characters[cn].x, gs.characters[cn].y);
    if citem != 0 && is_nolab_item(gs, citem) {
        let item_ref = c_string_to_str(&gs.items[citem].reference).to_string();
        gs.characters[cn].citem = 0;
        gs.items[citem].used = USE_EMPTY;
        gs.do_character_log(
            cn,
            core::types::FontColor::Yellow,
            &format!("Your {} vanished.\n", item_ref),
        );
    }

    for n in 0..40 {
        let inv_item = gs.characters[cn].item[n] as usize;
        if inv_item != 0 && is_nolab_item(gs, inv_item) {
            let item_ref = c_string_to_str(&gs.items[inv_item].reference).to_string();
            gs.characters[cn].item[n] = 0;
            gs.items[inv_item].used = USE_EMPTY;
            gs.do_character_log(
                cn,
                core::types::FontColor::Yellow,
                &format!("Your {} vanished.\n", item_ref),
            );
        }
    }

    for n in 0..20 {
        let spell_idx = gs.characters[cn].spell[n] as usize;
        if spell_idx != 0 {
            if gs.items[spell_idx].temp == core::constants::SK_RECALL as u16 {
                gs.characters[cn].spell[n] = 0;
                gs.items[spell_idx].used = USE_EMPTY;
            }
        }
    }

    let data2 = gs.items[item_idx].data[2];
    if data2 != 0 {
        let data3 = gs.items[item_idx].data[3];
        helpers::use_labtransfer(gs, cn, data2 as i32, data3 as i32);
        return 1;
    }

    let dest_x = gs.items[item_idx].data[0] as usize;
    let dest_y = gs.items[item_idx].data[1] as usize;

    EffectManager::fx_add_effect(6, 0, x as i32, y as i32, 0);
    God::transfer_char(gs, cn, dest_x, dest_y);
    EffectManager::fx_add_effect(6, 0, dest_x as i32, dest_y as i32, 0);

    1
}

pub fn teleport2(gs: &mut GameState, cn: usize, item_idx: usize) -> i32 {
    if cn == 0 {
        return 1;
    }

    let dest_x = gs.items[item_idx].data[0];
    let dest_y = gs.items[item_idx].data[1];
    let area_name = format!("({}, {})", dest_x, dest_y);
    log::info!(
        "Used teleport scroll to {},{} ({})",
        dest_x,
        dest_y,
        area_name
    );

    let scroll_time = gs.items[item_idx].data[2];
    let power = gs.items[item_idx].power;
    let ticker = gs.globals.ticker;
    if scroll_time != 0 && scroll_time + TICKS as u32 * 60 * 4 < ticker as u32 {
        let diff = ticker - scroll_time as i32;
        log::info!(
            "Lag Scroll Time Difference: {} ticks ({:.2}s)",
            diff,
            diff as f64 / TICKS as f64
        );
        gs.do_character_log(
            cn,
            core::types::FontColor::Yellow,
            "Sorry, this lag scroll was too old. You need to use it four minutes after lagging out or earlier!\n",
        );
        return 1;
    }

    // Create a recall spell item
    let spell_item = match God::create_item(gs, 1) {
        Some(id) => id,
        None => {
            log::error!("god_create_item failed in teleport2");
            return 0;
        }
    };

    {
        let spell = &mut gs.items[spell_item];
        let name = b"Teleport";
        spell.name[..name.len()].copy_from_slice(name);
        spell.flags |= ItemFlags::IF_SPELL.bits();
        spell.sprite[1] = 90;
        spell.duration = 180;
        spell.active = 180;
        spell.temp = SK_RECALL as u16;
        spell.power = power;
        spell.data[0] = dest_x;
        spell.data[1] = dest_y;
    }

    let added = driver::add_spell(gs, cn, spell_item);
    if added == 0 {
        let spell_name = c_string_to_str(&gs.items[spell_item].name).to_string();
        gs.do_character_log(
            cn,
            core::types::FontColor::Yellow,
            &format!(
                "Magical interference neutralised the {}'s effect.\n",
                spell_name
            ),
        );
        return 0;
    }

    let (x, y) = (gs.characters[cn].x, gs.characters[cn].y);
    crate::effect::EffectManager::fx_add_effect(7, 0, x as i32, y as i32, 0);
    1
}

pub fn use_labyrinth(gs: &mut GameState, cn: usize, _item_idx: usize) -> i32 {
    let citem = gs.characters[cn].citem as usize;
    if citem != 0 && is_nolab_item(gs, citem) {
        let item_ref = c_string_to_str(&gs.items[citem].reference).to_string();
        gs.characters[cn].citem = 0;
        gs.items[citem].used = USE_EMPTY;
        gs.do_character_log(
            cn,
            core::types::FontColor::Yellow,
            &format!("Your {} vanished.\n", item_ref),
        );
    }

    for n in 0..40 {
        let inv_item = gs.characters[cn].item[n] as usize;
        if inv_item != 0 && is_nolab_item(gs, inv_item) {
            let item_ref = c_string_to_str(&gs.items[inv_item].reference).to_string();
            gs.characters[cn].item[n] = 0;
            gs.items[inv_item].used = USE_EMPTY;
            gs.do_character_log(
                cn,
                core::types::FontColor::Yellow,
                &format!("Your {} vanished.\n", item_ref),
            );
        }
    }

    for n in 0..20 {
        let spell_idx = gs.characters[cn].spell[n] as usize;
        if spell_idx != 0 {
            if gs.items[spell_idx].temp == SK_RECALL as u16 {
                gs.characters[cn].spell[n] = 0;
                gs.items[spell_idx].used = USE_EMPTY;
            }
        }
    }

    let progress = gs.characters[cn].data[20];
    let (x, y) = (gs.characters[cn].x, gs.characters[cn].y);
    let destination = match progress {
        0 => Some((64, 56)),
        1 => Some((95, 207)),
        2 => Some((74, 240)),
        3 => Some((37, 370)),
        4 => Some((114, 390)),
        5 => Some((28, 493)),
        6 => Some((24, 534)),
        7 => Some((118, 667)),
        8 => Some((63, 720)),
        9 => Some((33, 597)),
        _ => None,
    };

    let flag = if let Some((dest_x, dest_y)) = destination {
        EffectManager::fx_add_effect(6, 0, x as i32, y as i32, 0);
        let result = God::transfer_char(gs, cn, dest_x, dest_y);
        let (new_x, new_y) = (gs.characters[cn].x, gs.characters[cn].y);
        EffectManager::fx_add_effect(6, 0, new_x as i32, new_y as i32, 0);
        result
    } else {
        gs.do_character_log(
            cn,
            core::types::FontColor::Green,
            "You have already solved all existing parts of the labyrinth. Please come back later.\n",
        );
        false
    };

    if flag {
        let (x, y) = (gs.characters[cn].x, gs.characters[cn].y);
        gs.characters[cn].temple_x = x as u16;
        gs.characters[cn].temple_y = y as u16;
        gs.characters[cn].tavern_x = x as u16;
        gs.characters[cn].tavern_y = y as u16;
    }

    1
}

pub fn use_ladder(gs: &mut GameState, cn: usize, item_idx: usize) -> i32 {
    use crate::god::God;
    let item = &gs.items[item_idx];
    let item_x = item.x as usize;
    let item_y = item.y as usize;
    let offset_x = item.data[0] as i32;
    let offset_y = item.data[1] as i32;
    let dest_x = (item_x as i32 + offset_x) as usize;
    let dest_y = (item_y as i32 + offset_y) as usize;

    God::transfer_char(gs, cn, dest_x, dest_y);

    1
}

pub fn use_bag(gs: &mut GameState, cn: usize, item_idx: usize) -> i32 {
    let co = gs.items[item_idx].data[0] as usize;

    if !core::types::Character::is_sane_character(co) {
        return 0;
    }

    let owner = gs.characters[co].data[core::constants::CHD_CORPSEOWNER] as usize;

    if owner != 0 && owner != cn {
        let may_attack = gs.may_attack_msg(cn, owner, false);
        let allowed_cn = gs.characters[owner].data[core::constants::CHD_ALLOW] as usize;

        if may_attack == 0 && allowed_cn != cn {
            let owner_name = c_string_to_str(&gs.characters[owner].name).to_string();
            let owner_is_male =
                (gs.characters[owner].kindred & core::constants::KIN_MALE as i32) != 0;
            let owner_pronoun = if owner_is_male { "his" } else { "her" };

            gs.do_character_log(
                cn,
                core::types::FontColor::Green,
                &format!(
                    "This is {}'s grave, not yours. You may only search it with {} permission.\n",
                    owner_name, owner_pronoun
                ),
            );

            let corpse_active = gs.characters[co].used == USE_ACTIVE;
            let owner_x = gs.characters[owner].x;

            if corpse_active && owner_x != 0 {
                let cn_name = c_string_to_str(&gs.characters[cn].name).to_string();
                gs.do_character_log(
                    owner,
                    core::types::FontColor::Green,
                    &format!(
                        "{} just tried to search your grave. You must #ALLOW {} if you want them to.\n",
                        cn_name, cn_name
                    ),
                );
            }

            return 0;
        }
    }

    let co_ref = c_string_to_str(&gs.characters[co].reference).to_string();
    gs.do_character_log(
        cn,
        core::types::FontColor::Yellow,
        &format!("You search the remains of {}.\n", co_ref),
    );
    gs.do_look_char(cn, co, 0, 0, 1);

    1
}

pub fn use_scroll(gs: &mut GameState, cn: usize, item_idx: usize) -> i32 {
    let skill_nr = gs.items[item_idx].data[0] as usize;
    let teaches_only = gs.items[item_idx].data[1] != 0;

    if skill_nr >= MAXSKILL {
        return 0;
    }

    let current_val = gs.characters[cn].skill[skill_nr][0];
    let max_val = gs.characters[cn].skill[skill_nr][2];
    let difficulty = gs.characters[cn].skill[skill_nr][3];

    if current_val != 0 {
        if teaches_only {
            let name = skilltab::get_skill_name(skill_nr);
            gs.do_character_log(
                cn,
                core::types::FontColor::Yellow,
                &format!("You already know {}.\n", name),
            );
            return 0;
        }

        if current_val >= max_val {
            let name = skilltab::get_skill_name(skill_nr);
            gs.do_character_log(
                cn,
                core::types::FontColor::Yellow,
                &format!("You cannot raise skill {} any higher.\n", name),
            );
            return 0;
        }

        let name = skilltab::get_skill_name(skill_nr);
        gs.do_character_log(
            cn,
            core::types::FontColor::Green,
            &format!("Raised {} by one.\n", name),
        );

        let v = current_val as i32;
        let diff = difficulty as i32;
        let pts = helpers::skill_needed(v, diff);
        gs.characters[cn].points_tot += pts;
        gs.characters[cn].skill[skill_nr][0] += 1;
        gs.do_check_new_level(cn);
        log::info!(
            "Used scroll to raise skill {} for {} (pts={})",
            skill_nr,
            cn,
            pts
        );
    } else if max_val == 0 {
        let name = skilltab::get_skill_name(skill_nr);
        gs.do_character_log(
            cn,
            core::types::FontColor::Yellow,
            &format!("This scroll teaches {}, which you cannot learn.\n", name),
        );
        return 0;
    } else {
        gs.characters[cn].skill[skill_nr][0] = 1;
        let name = skilltab::get_skill_name(skill_nr);
        gs.do_character_log(
            cn,
            core::types::FontColor::Green,
            &format!("You learned {}!\n", name),
        );
        log::info!("Used scroll to learn {} (cn={})", skill_nr, cn);
    }

    gs.items[item_idx].used = USE_EMPTY;
    God::take_from_char(gs, item_idx, cn);
    gs.characters[cn].set_do_update_flags();

    1
}

pub fn use_scroll2(gs: &mut GameState, cn: usize, item_idx: usize) -> i32 {
    // TODO: Move these to core library
    const AT_NAME: [&str; 5] = ["Braveness", "Willpower", "Intuition", "Agility", "Strength"];

    let attrib_nr = gs.items[item_idx].data[0] as usize;
    let current_val = gs.characters[cn].attrib[attrib_nr][0];
    let max_val = gs.characters[cn].attrib[attrib_nr][2];
    let difficulty = gs.characters[cn].attrib[attrib_nr][3];

    if current_val >= max_val {
        gs.do_character_log(
            cn,
            core::types::FontColor::Green,
            &format!(
                "You cannot raise attribute {} any higher.\n",
                AT_NAME[attrib_nr]
            ),
        );
        return 0;
    }

    // Calculate points needed: v*v*v*diff/20
    let v = current_val as i32;
    let diff = difficulty as i32;
    let pts = (v * v * v * diff) / 20;

    gs.do_character_log(
        cn,
        core::types::FontColor::Yellow,
        &format!("Raised attribute {} by one.\n", AT_NAME[attrib_nr]),
    );
    chlog!(
        cn,
        "used a scroll to raise attribute {} (pts={})",
        AT_NAME[attrib_nr],
        pts
    );

    gs.characters[cn].points_tot += pts;
    gs.characters[cn].attrib[attrib_nr][0] += 1;
    gs.do_check_new_level(cn);
    gs.items[item_idx].used = USE_EMPTY;

    take_item_from_char(gs, item_idx, cn);
    gs.characters[cn].set_do_update_flags();

    1
}

pub fn use_scroll3(gs: &mut GameState, cn: usize, item_idx: usize) -> i32 {
    let amount = gs.items[item_idx].data[0] as i32;
    let current_hp = gs.characters[cn].hp[0];
    let max_hp = gs.characters[cn].hp[2];
    let difficulty = gs.characters[cn].hp[3];

    if current_hp >= max_hp {
        gs.do_character_log(
            cn,
            core::types::FontColor::Green,
            "You cannot raise Hitpoints any higher.\n",
        );
        return 0;
    }

    gs.do_character_log(
        cn,
        core::types::FontColor::Yellow,
        &format!("Raised Hitpoints by {}.\n", amount),
    );

    let v = current_hp as i32;
    let diff = difficulty as i32;
    let mut pts = 0;
    for n in 0..amount {
        pts += (n + v) * diff;
    }

    gs.characters[cn].points_tot += pts;
    gs.characters[cn].hp[0] += amount as u16;
    gs.do_check_new_level(cn);
    gs.items[item_idx].used = USE_EMPTY;

    take_item_from_char(gs, item_idx, cn);
    gs.characters[cn].set_do_update_flags();

    1
}

pub fn use_scroll4(gs: &mut GameState, cn: usize, item_idx: usize) -> i32 {
    let amount = gs.items[item_idx].data[0] as i32;
    let current_end = gs.characters[cn].end[0];
    let max_end = gs.characters[cn].end[2];
    let difficulty = gs.characters[cn].end[3];

    if current_end >= max_end {
        gs.do_character_log(
            cn,
            core::types::FontColor::Green,
            "You cannot raise Endurance any higher.\n",
        );
        return 0;
    }

    gs.do_character_log(
        cn,
        core::types::FontColor::Yellow,
        &format!("Raised Endurance by {}.\n", amount),
    );

    let v = current_end as i32;
    let diff = difficulty as i32;
    let mut pts = 0;
    for n in 0..amount {
        pts += ((n + v) * diff) / 2;
    }

    gs.characters[cn].points_tot += pts;
    gs.characters[cn].end[0] += amount as u16;
    gs.do_check_new_level(cn);
    gs.items[item_idx].used = USE_EMPTY;

    take_item_from_char(gs, item_idx, cn);
    gs.characters[cn].set_do_update_flags();

    1
}

pub fn use_scroll5(gs: &mut GameState, cn: usize, item_idx: usize) -> i32 {
    let amount = gs.items[item_idx].data[0] as i32;
    let current_mana = gs.characters[cn].mana[0];
    let max_mana = gs.characters[cn].mana[2];
    let difficulty = gs.characters[cn].mana[3];

    if current_mana >= max_mana {
        gs.do_character_log(
            cn,
            core::types::FontColor::Green,
            "You cannot raise Mana any higher.\n",
        );
        return 0;
    }

    gs.do_character_log(
        cn,
        core::types::FontColor::Yellow,
        &format!("Raised Mana by {}.\n", amount),
    );

    let v = current_mana as i32;
    let diff = difficulty as i32;
    let mut pts = 0;

    for n in 0..amount {
        pts += (n + v) * diff;
    }

    gs.characters[cn].points_tot += pts;
    gs.characters[cn].mana[0] += amount as u16;
    gs.do_check_new_level(cn);
    gs.items[item_idx].used = USE_EMPTY;

    take_item_from_char(gs, item_idx, cn);
    gs.characters[cn].set_do_update_flags();

    1
}

pub fn use_crystal_sub(gs: &mut GameState, _cn: usize, item_idx: usize) -> i32 {
    // Get group id
    let group = gs.items[item_idx].data[0];

    // Count existing NPCs in the same group and bucket their base values
    let mut baseg = [0i32; 20];
    let mut cnt = 0i32;
    {
        for n in 1..core::constants::MAXCHARS {
            let ch = &gs.characters[n];
            if ch.used == core::constants::USE_ACTIVE
                && (ch.flags & CharacterFlags::Body.bits()) == 0
                && ch.data[42] == group as i32
            {
                let mut base = ch.data[0];
                if base > 99 {
                    base = 99;
                }
                let idx = (base / 5) as usize;
                if idx < baseg.len() {
                    baseg[idx] += 1;
                }
                cnt += 1;
            }
        }
    };

    // compute how many are still missing
    let need = gs.items[item_idx].data[1] as i32 - cnt;
    if need <= 0 {
        return 0;
    }

    // find smallest bucket
    let mut sbase = 0usize;
    let mut tmpmin = i32::MAX;
    for i in 0..baseg.len() {
        if baseg[i] < tmpmin {
            tmpmin = baseg[i];
            sbase = i;
        }
    }

    log::info!(
        "Randoms: smallest base is {} with {} (miss={})",
        sbase,
        tmpmin,
        need
    );

    // choose a random template from temps
    let temps: [usize; 6] = [2, 4, 76, 78, 150, 151];
    let tmp = temps[helpers::random_mod_usize(temps.len())];

    // create char
    let cc_i32 = match God::create_char(gs, tmp, false) {
        Some(id) => id,
        None => return 0,
    };
    let cc = cc_i32 as usize;

    // attempt to drop at crystal position
    let (item_x, item_y) = (gs.items[item_idx].x, gs.items[item_idx].y);
    if !God::drop_char_fuzzy(gs, cc, item_x as usize, item_y as usize) {
        God::destroy_items(gs, cc);
        {
            gs.characters[cc].used = core::constants::USE_EMPTY;
        };
        return 0;
    }

    // set group
    {
        gs.characters[cc].data[42] = group as i32;
    };

    // pick spawn tile until valid
    let m = loop {
        let m_try = (helpers::random_mod_usize(64) + 128)
            + (helpers::random_mod_usize(64) + 64) * core::constants::SERVER_MAPX as usize;
        if player::plr_check_target(gs, m_try) {
            break m_try;
        }
    };

    // configure the character
    {
        let ch = &mut gs.characters[cc];
        ch.goto_x = (m % core::constants::SERVER_MAPX as usize) as u16;
        ch.goto_y = (m / core::constants::SERVER_MAPX as usize) as u16;
        ch.data[60] = TICKS * 20;
        ch.data[62] = 1;

        // texts (format strings)
        let t0 = b"Yes! Die, %s!";
        ch.text[0][..t0.len()].copy_from_slice(t0);
        ch.text[0][t0.len()..].fill(0);
        let t1 = b"Yahoo! An enemy! Prepare to die, %s!";
        ch.text[1][..t1.len()].copy_from_slice(t1);
        ch.text[1][t1.len()..].fill(0);
        let t3 = b"Thank you %s! Everything is better than being here.";
        ch.text[3][..t3.len()].copy_from_slice(t3);
        ch.text[3][t3.len()..].fill(0);
        ch.data[48] = 33;

        // base and attributes
        let base = (sbase * 5) as i32 + helpers::random_mod_i32(5);
        ch.data[0] = base;

        for n in 0..5 {
            let mut t = base + helpers::random_mod_i32(15);
            let diff = std::cmp::max(1, ch.attrib[n][3] as i32);
            t = t * 3 / diff;
            let maxv = ch.attrib[n][2] as i32;
            let v = std::cmp::max(10, std::cmp::min(maxv, t));
            ch.attrib[n][0] = v as u8;
        }

        for n in 0..50 {
            let mut t = base + helpers::random_mod_i32(15);
            let diff = std::cmp::max(1, ch.skill[n][3] as i32);
            t = t * 3 / diff;
            if ch.skill[n][2] != 0 {
                let maxv = ch.skill[n][2] as i32;
                ch.skill[n][0] = std::cmp::min(maxv, t) as u8;
            }
        }

        ch.hp[0] = std::cmp::max(
            50,
            std::cmp::min(ch.hp[2] as i32, base * 5 + helpers::random_mod_i32(50)),
        ) as u16;
        ch.end[0] = std::cmp::max(
            50,
            std::cmp::min(ch.end[2] as i32, base * 5 + helpers::random_mod_i32(50)),
        ) as u16;
        ch.mana[0] = std::cmp::max(
            50,
            std::cmp::min(ch.mana[2] as i32, base * 5 + helpers::random_mod_i32(50)),
        ) as u16;

        // calculate experience points
        let mut pts = 0i32;
        for z in 0..5 {
            for m in 10..(ch.attrib[z][0] as i32) {
                pts += helpers::attrib_needed(m, 3);
            }
        }
        for m in 50..(ch.hp[0] as i32) {
            pts += helpers::hp_needed(m, 3);
        }
        for m in 50..(ch.end[0] as i32) {
            pts += helpers::end_needed(m, 2);
        }
        for m in 50..(ch.mana[0] as i32) {
            pts += helpers::mana_needed(m, 3);
        }
        for z in 0..50 {
            for m in 1..(ch.skill[z][0] as i32) {
                pts += helpers::skill_needed(m, 2);
            }
        }

        ch.points_tot = pts;
        ch.gold = base * base + 1;
        ch.a_hp = 999999;
        ch.a_end = 999999;
        if ch.skill[core::constants::SK_MEDIT][0] > 0 {
            ch.a_mana = 1000000;
        } else {
            let mut v = 1i32;
            for _ in 0..6 {
                v *= helpers::random_mod_i32(4);
            }
            ch.a_mana = v * 100;
        }

        ch.alignment = -(helpers::random_mod_i32(7500) as i16);
    };

    log::info!("Created random dungeon NPC from crystal (template {})", tmp);

    // Equip character based on attributes/skills
    {
        {
            let _ch = gs.characters[cc];
        };

        {
            if gs.characters[cc].attrib[core::constants::AT_AGIL as usize][0] as i32 >= 90
                && gs.characters[cc].attrib[core::constants::AT_STREN as usize][0] as i32 >= 90
            {
                let tmp = populate::pop_create_item(gs, 94, cc);
                if tmp != 0 {
                    gs.characters[cc].worn[core::constants::WN_HEAD] = tmp as u32;
                    gs.items[tmp].carried = cc as u16;
                }
                let tmp = populate::pop_create_item(gs, 95, cc);
                if tmp != 0 {
                    gs.characters[cc].worn[core::constants::WN_BODY] = tmp as u32;
                    gs.items[tmp].carried = cc as u16;
                }
                let tmp = populate::pop_create_item(gs, 98, cc);
                if tmp != 0 {
                    gs.characters[cc].worn[core::constants::WN_ARMS] = tmp as u32;
                    gs.items[tmp].carried = cc as u16;
                }
                let tmp = populate::pop_create_item(gs, 97, cc);
                if tmp != 0 {
                    gs.characters[cc].worn[core::constants::WN_LEGS] = tmp as u32;
                    gs.items[tmp].carried = cc as u16;
                }
                let tmp = populate::pop_create_item(gs, 99, cc);
                if tmp != 0 {
                    gs.characters[cc].worn[core::constants::WN_FEET] = tmp as u32;
                    gs.items[tmp].carried = cc as u16;
                }
                let tmp = populate::pop_create_item(gs, 96, cc);
                if tmp != 0 {
                    gs.characters[cc].worn[core::constants::WN_CLOAK] = tmp as u32;
                    gs.items[tmp].carried = cc as u16;
                }
            } else if gs.characters[cc].attrib[core::constants::AT_AGIL as usize][0] as i32 >= 72
                && gs.characters[cc].attrib[core::constants::AT_STREN as usize][0] as i32 >= 72
            {
                let tmp = populate::pop_create_item(gs, 75, cc);
                if tmp != 0 {
                    gs.characters[cc].worn[core::constants::WN_HEAD] = tmp as u32;
                    gs.items[tmp].carried = cc as u16;
                }
                let tmp = populate::pop_create_item(gs, 76, cc);
                if tmp != 0 {
                    gs.characters[cc].worn[core::constants::WN_BODY] = tmp as u32;
                    gs.items[tmp].carried = cc as u16;
                }
                let tmp = populate::pop_create_item(gs, 79, cc);
                if tmp != 0 {
                    gs.characters[cc].worn[core::constants::WN_ARMS] = tmp as u32;
                    gs.items[tmp].carried = cc as u16;
                }
                let tmp = populate::pop_create_item(gs, 78, cc);
                if tmp != 0 {
                    gs.characters[cc].worn[core::constants::WN_LEGS] = tmp as u32;
                    gs.items[tmp].carried = cc as u16;
                }
                let tmp = populate::pop_create_item(gs, 80, cc);
                if tmp != 0 {
                    gs.characters[cc].worn[core::constants::WN_FEET] = tmp as u32;
                    gs.items[tmp].carried = cc as u16;
                }
                let tmp = populate::pop_create_item(gs, 77, cc);
                if tmp != 0 {
                    gs.characters[cc].worn[core::constants::WN_CLOAK] = tmp as u32;
                    gs.items[tmp].carried = cc as u16;
                }
            } else if gs.characters[cc].attrib[core::constants::AT_AGIL as usize][0] as i32 >= 40
                && gs.characters[cc].attrib[core::constants::AT_STREN as usize][0] as i32 >= 40
            {
                let tmp = populate::pop_create_item(gs, 69, cc);
                if tmp != 0 {
                    gs.characters[cc].worn[core::constants::WN_HEAD] = tmp as u32;
                    gs.items[tmp].carried = cc as u16;
                }
                let tmp = populate::pop_create_item(gs, 71, cc);
                if tmp != 0 {
                    gs.characters[cc].worn[core::constants::WN_BODY] = tmp as u32;
                    gs.items[tmp].carried = cc as u16;
                }
                let tmp = populate::pop_create_item(gs, 73, cc);
                if tmp != 0 {
                    gs.characters[cc].worn[core::constants::WN_ARMS] = tmp as u32;
                    gs.items[tmp].carried = cc as u16;
                }
                let tmp = populate::pop_create_item(gs, 72, cc);
                if tmp != 0 {
                    gs.characters[cc].worn[core::constants::WN_LEGS] = tmp as u32;
                    gs.items[tmp].carried = cc as u16;
                }
                let tmp = populate::pop_create_item(gs, 74, cc);
                if tmp != 0 {
                    gs.characters[cc].worn[core::constants::WN_FEET] = tmp as u32;
                    gs.items[tmp].carried = cc as u16;
                }
                let tmp = populate::pop_create_item(gs, 70, cc);
                if tmp != 0 {
                    gs.characters[cc].worn[core::constants::WN_CLOAK] = tmp as u32;
                    gs.items[tmp].carried = cc as u16;
                }
            } else if gs.characters[cc].attrib[core::constants::AT_AGIL as usize][0] as i32 >= 24
                && gs.characters[cc].attrib[core::constants::AT_STREN as usize][0] as i32 >= 24
            {
                let tmp = populate::pop_create_item(gs, 63, cc);
                if tmp != 0 {
                    gs.characters[cc].worn[core::constants::WN_HEAD] = tmp as u32;
                    gs.items[tmp].carried = cc as u16;
                }
                let tmp = populate::pop_create_item(gs, 65, cc);
                if tmp != 0 {
                    gs.characters[cc].worn[core::constants::WN_BODY] = tmp as u32;
                    gs.items[tmp].carried = cc as u16;
                }
                let tmp = populate::pop_create_item(gs, 67, cc);
                if tmp != 0 {
                    gs.characters[cc].worn[core::constants::WN_ARMS] = tmp as u32;
                    gs.items[tmp].carried = cc as u16;
                }
                let tmp = populate::pop_create_item(gs, 66, cc);
                if tmp != 0 {
                    gs.characters[cc].worn[core::constants::WN_LEGS] = tmp as u32;
                    gs.items[tmp].carried = cc as u16;
                }
                let tmp = populate::pop_create_item(gs, 68, cc);
                if tmp != 0 {
                    gs.characters[cc].worn[core::constants::WN_FEET] = tmp as u32;
                    gs.items[tmp].carried = cc as u16;
                }
                let tmp = populate::pop_create_item(gs, 64, cc);
                if tmp != 0 {
                    gs.characters[cc].worn[core::constants::WN_CLOAK] = tmp as u32;
                    gs.items[tmp].carried = cc as u16;
                }
            } else if gs.characters[cc].attrib[core::constants::AT_AGIL as usize][0] as i32 >= 16
                && gs.characters[cc].attrib[core::constants::AT_STREN as usize][0] as i32 >= 16
            {
                let tmp = populate::pop_create_item(gs, 57, cc);
                if tmp != 0 {
                    gs.characters[cc].worn[core::constants::WN_HEAD] = tmp as u32;
                    gs.items[tmp].carried = cc as u16;
                }
                let tmp = populate::pop_create_item(gs, 59, cc);
                if tmp != 0 {
                    gs.characters[cc].worn[core::constants::WN_BODY] = tmp as u32;
                    gs.items[tmp].carried = cc as u16;
                }
                let tmp = populate::pop_create_item(gs, 61, cc);
                if tmp != 0 {
                    gs.characters[cc].worn[core::constants::WN_ARMS] = tmp as u32;
                    gs.items[tmp].carried = cc as u16;
                }
                let tmp = populate::pop_create_item(gs, 60, cc);
                if tmp != 0 {
                    gs.characters[cc].worn[core::constants::WN_LEGS] = tmp as u32;
                    gs.items[tmp].carried = cc as u16;
                }
                let tmp = populate::pop_create_item(gs, 62, cc);
                if tmp != 0 {
                    gs.characters[cc].worn[core::constants::WN_FEET] = tmp as u32;
                    gs.items[tmp].carried = cc as u16;
                }
                let tmp = populate::pop_create_item(gs, 58, cc);
                if tmp != 0 {
                    gs.characters[cc].worn[core::constants::WN_CLOAK] = tmp as u32;
                    gs.items[tmp].carried = cc as u16;
                }
            } else if gs.characters[cc].attrib[core::constants::AT_AGIL as usize][0] as i32 >= 12
                && gs.characters[cc].attrib[core::constants::AT_STREN as usize][0] as i32 >= 12
            {
                let tmp = populate::pop_create_item(gs, 51, cc);
                if tmp != 0 {
                    gs.characters[cc].worn[core::constants::WN_HEAD] = tmp as u32;
                    gs.items[tmp].carried = cc as u16;
                }
                let tmp = populate::pop_create_item(gs, 53, cc);
                if tmp != 0 {
                    gs.characters[cc].worn[core::constants::WN_BODY] = tmp as u32;
                    gs.items[tmp].carried = cc as u16;
                }
                let tmp = populate::pop_create_item(gs, 55, cc);
                if tmp != 0 {
                    gs.characters[cc].worn[core::constants::WN_ARMS] = tmp as u32;
                    gs.items[tmp].carried = cc as u16;
                }
                let tmp = populate::pop_create_item(gs, 54, cc);
                if tmp != 0 {
                    gs.characters[cc].worn[core::constants::WN_LEGS] = tmp as u32;
                    gs.items[tmp].carried = cc as u16;
                }
                let tmp = populate::pop_create_item(gs, 56, cc);
                if tmp != 0 {
                    gs.characters[cc].worn[core::constants::WN_FEET] = tmp as u32;
                    gs.items[tmp].carried = cc as u16;
                }
                let tmp = populate::pop_create_item(gs, 52, cc);
                if tmp != 0 {
                    gs.characters[cc].worn[core::constants::WN_CLOAK] = tmp as u32;
                    gs.items[tmp].carried = cc as u16;
                }
            } else if gs.characters[cc].attrib[core::constants::AT_AGIL as usize][0] as i32 >= 10
                && gs.characters[cc].attrib[core::constants::AT_STREN as usize][0] as i32 >= 10
            {
                let tmp = populate::pop_create_item(gs, 39, cc);
                if tmp != 0 {
                    gs.characters[cc].worn[core::constants::WN_HEAD] = tmp as u32;
                    gs.items[tmp].carried = cc as u16;
                }
                let tmp = populate::pop_create_item(gs, 42, cc);
                if tmp != 0 {
                    gs.characters[cc].worn[core::constants::WN_BODY] = tmp as u32;
                    gs.items[tmp].carried = cc as u16;
                }
                let tmp = populate::pop_create_item(gs, 44, cc);
                if tmp != 0 {
                    gs.characters[cc].worn[core::constants::WN_ARMS] = tmp as u32;
                    gs.items[tmp].carried = cc as u16;
                }
                let tmp = populate::pop_create_item(gs, 43, cc);
                if tmp != 0 {
                    gs.characters[cc].worn[core::constants::WN_LEGS] = tmp as u32;
                    gs.items[tmp].carried = cc as u16;
                }
                let tmp = populate::pop_create_item(gs, 41, cc);
                if tmp != 0 {
                    gs.characters[cc].worn[core::constants::WN_FEET] = tmp as u32;
                    gs.items[tmp].carried = cc as u16;
                }
                let tmp = populate::pop_create_item(gs, 40, cc);
                if tmp != 0 {
                    gs.characters[cc].worn[core::constants::WN_CLOAK] = tmp as u32;
                    gs.items[tmp].carried = cc as u16;
                }
            }

            // choose weapon based on skills/attribs (partial port of original logic)
            if gs.characters[cc].skill[core::constants::SK_TWOHAND][0] as i32 >= 60
                && gs.characters[cc].attrib[core::constants::AT_AGIL as usize][0] as i32 >= 50
                && gs.characters[cc].attrib[core::constants::AT_STREN as usize][0] as i32 >= 75
            {
                let tmp = populate::pop_create_item(gs, 125, cc);
                if tmp != 0 {
                    gs.characters[cc].worn[core::constants::WN_RHAND] = tmp as u32;
                    gs.items[tmp].carried = cc as u16;
                }
            } else if gs.characters[cc].skill[core::constants::SK_TWOHAND][0] as i32 >= 45
                && gs.characters[cc].attrib[core::constants::AT_AGIL as usize][0] as i32 >= 40
                && gs.characters[cc].attrib[core::constants::AT_STREN as usize][0] as i32 >= 60
            {
                let tmp = populate::pop_create_item(gs, 38, cc);
                if tmp != 0 {
                    gs.characters[cc].worn[core::constants::WN_RHAND] = tmp as u32;
                    gs.items[tmp].carried = cc as u16;
                }
            } else if gs.characters[cc].skill[core::constants::SK_TWOHAND][0] as i32 >= 30
                && gs.characters[cc].attrib[core::constants::AT_AGIL as usize][0] as i32 >= 30
                && gs.characters[cc].attrib[core::constants::AT_STREN as usize][0] as i32 >= 40
            {
                let tmp = populate::pop_create_item(gs, 37, cc);
                if tmp != 0 {
                    gs.characters[cc].worn[core::constants::WN_RHAND] = tmp as u32;
                    gs.items[tmp].carried = cc as u16;
                }
            } else if gs.characters[cc].skill[core::constants::SK_TWOHAND][0] as i32 >= 15
                && gs.characters[cc].attrib[core::constants::AT_AGIL as usize][0] as i32 >= 20
                && gs.characters[cc].attrib[core::constants::AT_STREN as usize][0] as i32 >= 24
            {
                let tmp = populate::pop_create_item(gs, 36, cc);
                if tmp != 0 {
                    gs.characters[cc].worn[core::constants::WN_RHAND] = tmp as u32;
                    gs.items[tmp].carried = cc as u16;
                }
            } else if gs.characters[cc].skill[core::constants::SK_TWOHAND][0] as i32 >= 1
                && gs.characters[cc].attrib[core::constants::AT_AGIL as usize][0] as i32 >= 10
                && gs.characters[cc].attrib[core::constants::AT_STREN as usize][0] as i32 >= 12
            {
                let tmp = populate::pop_create_item(gs, 35, cc);
                if tmp != 0 {
                    gs.characters[cc].worn[core::constants::WN_RHAND] = tmp as u32;
                    gs.items[tmp].carried = cc as u16;
                }
            }
        };
    }

    // occasional extra items (partial port)
    if helpers::random_mod_i32(30) == 0 && gs.characters[cc].data[0] > 5 {
        if let Some(it) = God::create_item(gs, helpers::random_mod_usize(2) + 273) {
            God::give_character_item(gs, cc, it);
        }
    }
    if helpers::random_mod_i32(60) == 0 && gs.characters[cc].data[0] > 15 {
        if let Some(it) = God::create_item(gs, helpers::random_mod_usize(2) + 192) {
            God::give_character_item(gs, cc, it);
        }
    }
    if helpers::random_mod_i32(150) == 0 && gs.characters[cc].data[0] > 20 {
        if let Some(it) = God::create_item(gs, helpers::random_mod_usize(9) + 181) {
            God::give_character_item(gs, cc, it);
        }
    }

    // update character state
    gs.do_update_char(cc);

    need
}

pub fn use_crystal(gs: &mut GameState, cn: usize, item_idx: usize) -> i32 {
    let mut cnt = 0;

    // Call use_crystal_sub until it returns <= 4, up to 4 times
    while use_crystal_sub(gs, cn, item_idx) > 4 && cnt < 4 {
        cnt += 1;
    }

    if cnt == 0 {
        1
    } else {
        0
    }
}

pub fn use_mine_respawn(gs: &mut GameState, _cn: usize, item_idx: usize) -> i32 {
    // Get group, template, and max count from item data
    let (group, template, max_cnt) = {
        let item = &gs.items[item_idx];
        (
            item.data[0] as i32,
            item.data[1] as usize,
            item.data[2] as i32,
        )
    };

    // Check if mine wall items exist (data[3-9])
    for n in 3..10 {
        let map_idx = gs.items[item_idx].data[n] as usize;
        if map_idx == 0 {
            break;
        }

        // Check if there's a mine wall item at that location
        let in2 = gs.map[map_idx].it as usize;
        if in2 == 0 {
            return 0;
        }

        let driver = gs.items[in2].driver;
        if driver != 26 {
            return 0;
        }
    }

    // Count active NPCs in this group
    let cnt = {
        let mut count = 0;
        for n in 1..core::constants::MAXCHARS {
            if gs.characters[n].used == core::constants::USE_ACTIVE
                && (gs.characters[n].flags & CharacterFlags::Body.bits()) == 0
                && gs.characters[n].data[42] == group
            {
                count += 1;
            }
        }
        count
    };

    // Don't spawn if too many NPCs already
    if cnt > max_cnt {
        return 0;
    }

    // create the NPC from template
    let cc = match populate::pop_create_char(gs, template, false) {
        Some(cc) => cc,
        None => return 0,
    };

    // drop the character near the mine item
    let (item_x, item_y) = (gs.items[item_idx].x, gs.items[item_idx].y);
    if !God::drop_char_fuzzy(gs, cc, item_x as usize, item_y as usize) {
        log::warn!("use_mine_respawn ({},{}): drop_char failed", item_x, item_y);
        // cleanup on failure
        God::destroy_items(gs, cc);
        {
            gs.characters[cc].used = core::constants::USE_EMPTY;
        };
        return 0;
    }

    // ensure the new character is visible/updated
    gs.do_update_char(cc);

    1
}

pub fn rat_eye(gs: &mut GameState, cn: usize, item_idx: usize) -> i32 {
    if cn == 0 {
        return 0;
    }

    let citem = gs.characters[cn].citem;

    if citem == 0 || (citem & 0x80000000) != 0 {
        gs.do_character_log(
            cn,
            core::types::FontColor::Green,
            "What do you want to do with it?\n",
        );
        return 0;
    }

    // Check if rat eye is carried (not on ground)
    let carried = gs.items[item_idx].carried;
    if carried == 0 {
        gs.do_character_log(
            cn,
            core::types::FontColor::Green,
            "Too difficult to do on the ground.\n",
        );
        return 0;
    }

    // Check if citem matches any of the required templates in data[0-8]
    let citem_temp = gs.items[citem as usize].temp;

    let mut slot = None;
    for n in 0..9 {
        let required_temp = gs.items[item_idx].data[n] as u16;
        if required_temp != 0 && required_temp == citem_temp {
            slot = Some(n);
            break;
        }
    }

    let slot = match slot {
        Some(s) => s,
        None => {
            gs.do_character_log(cn, core::types::FontColor::Green, "This doesnt fit.\n");
            return 0;
        }
    };

    chlog!(cn, "added item to rat eye in slot {}", slot);
    log::info!("Character {} added item to rat eye", cn);

    // Mark the slot as filled
    {
        gs.items[item_idx].data[slot] = 0;
        gs.items[item_idx].sprite[0] += 1;
        gs.items[item_idx].flags |= ItemFlags::IF_UPDATE.bits();
        gs.items[item_idx].temp = 0;
    };

    // Remove the citem
    {
        gs.items[citem as usize].used = USE_EMPTY;
    };
    {
        gs.characters[cn].citem = 0;
    };

    // Check if all slots are filled
    let all_filled = (0..9).all(|n| gs.items[item_idx].data[n] == 0);

    if all_filled {
        // Create the final item from data[9]
        let result_template = gs.items[item_idx].data[9] as usize;

        let in3 = match God::create_item(gs, result_template) {
            Some(id) => id,
            None => return 1,
        };

        {
            gs.items[in3].flags |= ItemFlags::IF_UPDATE.bits();
        };

        // Remove the rat eye item
        take_item_from_char(gs, item_idx, cn);
        {
            gs.items[item_idx].used = USE_EMPTY;
        };

        // Give the completed item to the character
        God::give_character_item(gs, cn, in3);
    }

    1
}

pub fn skua_protect(gs: &mut GameState, cn: usize, item_idx: usize) -> i32 {
    // Check if the weapon is wielded
    let is_wielded = gs.characters[cn].worn[WN_RHAND] == item_idx as u32;

    if !is_wielded {
        gs.do_character_log(
            cn,
            core::types::FontColor::Green,
            "You cannot use Skua's weapon if you're not wielding it.\n",
        );
        return 0;
    }

    // Check if character has Skua's kindred (KIN_SKUA = 0x00000002)
    let has_skua_kindred = (gs.characters[cn].kindred & 0x00000002) != 0;

    if !has_skua_kindred {
        gs.do_character_log(
            cn,
            core::types::FontColor::Green,
            "How dare you to call on Skua to help you? Slave of the Purple One!\n",
        );
        gs.do_character_log(cn, core::types::FontColor::Green, "Your weapon vanished.\n");

        {
            gs.characters[cn].worn[WN_RHAND] = 0;
        };

        {
            gs.items[item_idx].used = core::constants::USE_EMPTY;
        };
    } else {
        gs.do_character_log(
            cn,
            core::types::FontColor::Green,
            "You feel Skua's presence protect you.\n",
        );
        gs.do_character_log(
            cn,
            core::types::FontColor::Green,
            "He takes away His weapon and replaces it by a common one.\n",
        );

        driver::spell_from_item(gs, cn, item_idx);

        // Get replacement weapon template from data[2]
        let replacement_template = gs.items[item_idx].data[2] as usize;

        // Remove the Skua weapon
        {
            gs.items[item_idx].used = core::constants::USE_EMPTY;
        };

        // Create replacement weapon
        if let Some(new_weapon) = crate::god::God::create_item(gs, replacement_template) {
            {
                gs.items[new_weapon].carried = cn as u16;
                gs.items[new_weapon].flags |= core::constants::ItemFlags::IF_UPDATE.bits();
            };

            {
                gs.characters[cn].worn[WN_RHAND] = new_weapon as u32;
            };
        }
    }

    1
}

pub fn purple_protect(gs: &mut GameState, cn: usize, item_idx: usize) -> i32 {
    // Check if the weapon is wielded
    let is_wielded = { gs.characters[cn].worn[core::constants::WN_RHAND] == item_idx as u32 };

    if !is_wielded {
        gs.do_character_log(
            cn,
            core::types::FontColor::Green,
            "You cannot use the Purple One's weapon if you're not wielding it.\n",
        );
        return 0;
    }

    // Check if character has Purple One's kindred (KIN_PURPLE = 0x00000001)
    let has_purple_kindred = (gs.characters[cn].kindred & 0x00000001) != 0;

    if !has_purple_kindred {
        gs.do_character_log(
            cn,
            core::types::FontColor::Green,
            "How dare you to call on the Purple One to help you? Slave of Skua!\n",
        );
        gs.do_character_log(cn, core::types::FontColor::Green, "Your weapon vanished.\n");

        {
            gs.characters[cn].worn[core::constants::WN_RHAND] = 0;
        };

        {
            gs.items[item_idx].used = core::constants::USE_EMPTY;
        };
    } else {
        gs.do_character_log(
            cn,
            core::types::FontColor::Green,
            "You feel the Purple One's presence protect you.\n",
        );
        gs.do_character_log(
            cn,
            core::types::FontColor::Green,
            "He takes away His weapon and replaces it by a common one.\n",
        );

        driver::spell_from_item(gs, cn, item_idx);

        // Get replacement weapon template from data[2]
        let replacement_template = gs.items[item_idx].data[2] as usize;

        // Remove the Purple One's weapon
        {
            gs.items[item_idx].used = core::constants::USE_EMPTY;
        };

        // Create replacement weapon
        if let Some(new_weapon) = crate::god::God::create_item(gs, replacement_template) {
            {
                gs.items[new_weapon].carried = cn as u16;
                gs.items[new_weapon].flags |= core::constants::ItemFlags::IF_UPDATE.bits();
            };

            {
                gs.characters[cn].worn[core::constants::WN_RHAND] = new_weapon as u32;
            };
        }
    }

    1
}

pub fn use_lever(gs: &mut GameState, _cn: usize, item_idx: usize) -> i32 {
    // Get the map coordinate from item data[0]
    let m = gs.items[item_idx].data[0] as usize;

    // Get the item at that map location
    let in2 = gs.map[m].it;

    if in2 == 0 {
        return 0;
    }

    // Check if the item is already active
    let is_active = gs.items[in2 as usize].active != 0;
    if is_active {
        return 0;
    }

    // Activate the linked item
    use_driver(gs, 0, in2 as usize, false);

    // Set active to duration and handle lighting changes
    gs.items[in2 as usize].active = gs.items[in2 as usize].duration;

    let light0 = gs.items[in2 as usize].light[0];
    let light1 = gs.items[in2 as usize].light[1];
    if light0 != light1 {
        let x = gs.items[in2 as usize].x as i32;
        let y = gs.items[in2 as usize].y as i32;
        gs.do_add_light(x, y, light1 as i32 - light0 as i32);
    }

    1
}

pub fn use_spawn(gs: &mut GameState, cn: usize, item_idx: usize) -> i32 {
    // Check if already active
    let is_active = gs.items[item_idx].active != 0;
    if is_active {
        return 0;
    }

    // Check if player needs to provide an item (data[1])
    if cn != 0 {
        let required_template = gs.items[item_idx].data[1] as usize;

        if required_template != 0 {
            let citem = gs.characters[cn].citem;

            if citem == 0 || (citem & 0x80000000) != 0 {
                return 0;
            }

            let citem_template = gs.items[citem as usize].temp;
            if citem_template as usize != required_template {
                return 0;
            }

            // Remove the required item
            {
                gs.items[citem as usize].used = USE_EMPTY;
            };
            {
                gs.characters[cn].citem = 0;
            };
        }
    }

    // Add effect if data[2] contains a character template
    let temp = gs.items[item_idx].data[2] as usize;
    if temp != 0 {
        {
            EffectManager::fx_add_effect(
                2,
                core::constants::TICKS * 10,
                gs.character_templates[temp].x as i32,
                gs.character_templates[temp].y as i32,
                temp as i32,
            )
        };
        log::info!("use_spawn: would add effect for template {}", temp);
    }

    1
}

pub fn use_pile(gs: &mut GameState, cn: usize, item_idx: usize) -> i32 {
    // Item templates for rewards at different levels
    const FIND: [usize; 90] = [
        // Level 0 (0-29): silver, small jewels, skeleton
        361, 361, 339, 342, 345, 339, 342, 345, 359, 359, 361, 361, 339, 342, 345, 339, 342, 345,
        359, 359, 361, 361, 339, 342, 345, 339, 342, 345, 359, 359,
        // Level 1 (30-59): silver, med jewels, golem
        361, 361, 361, 340, 343, 346, 371, 371, 371, 371, 361, 361, 361, 340, 343, 346, 371, 371,
        371, 371, 361, 361, 361, 340, 343, 346, 371, 371, 371, 371,
        // Level 2 (60-89): gold, big jewels, gargoyle
        360, 341, 344, 347, 372, 372, 372, 487, 372, 372, 360, 341, 344, 347, 372, 372, 372, 488,
        372, 372, 360, 341, 344, 347, 372, 372, 372, 489, 372, 372,
    ];

    // Check if already active (already searched)
    let is_active = gs.items[item_idx].active != 0;
    if is_active {
        return 0;
    }

    // Get pile info
    let (x, y, level) = {
        (
            gs.items[item_idx].x,
            gs.items[item_idx].y,
            gs.items[item_idx].data[0] as i32,
        )
    };

    // Destroy this object
    {
        gs.items[item_idx].used = USE_EMPTY;
    };

    let m = (x as usize) + (y as usize) * core::constants::SERVER_MAPX as usize;
    {
        gs.map[m].it = 0;
    };

    // Calculate chance based on player's luck
    let luck = gs.characters[cn].luck;

    let mut chance = 6;
    if luck < 0 {
        chance += 1;
    }
    if luck <= -100 {
        chance += 1;
    }
    if luck <= -500 {
        chance += 1;
    }
    if luck <= -1000 {
        chance += 1;
    }
    if luck <= -2000 {
        chance += 1;
    }
    if luck <= -3000 {
        chance += 1;
    }
    if luck <= -4000 {
        chance += 1;
    }
    if luck <= -6000 {
        chance += 1;
    }
    if luck <= -8000 {
        chance += 1;
    }
    if luck <= -10000 {
        chance += 1;
    }

    // Roll for loot
    if crate::helpers::random_mod(chance) != 0 {
        return 1; // Nothing found
    }

    // Determine what to give based on level
    let tmp_idx = crate::helpers::random_mod_usize(30) + (level as usize * 30);
    let tmp_idx = tmp_idx.min(89); // Clamp to valid range
    let tmp = FIND[tmp_idx];

    // Create item
    if let Some(in2) = God::create_item(gs, tmp) {
        let (is_takeable, data_0) = {
            (
                (gs.items[in2].flags & ItemFlags::IF_TAKE.bits()) != 0,
                gs.items[in2].data[0],
            )
        };

        if is_takeable {
            // Give to player
            if God::give_character_item(gs, cn, in2) {
                let reference = gs.items[in2].reference;
                gs.do_character_log(
                    cn,
                    core::types::FontColor::Green,
                    &format!("You've found a {}!\n", c_string_to_str(&reference)),
                );
            }
        } else {
            // It's a monster spawner
            God::drop_item(gs, in2, x as usize, y as usize);
            EffectManager::fx_add_effect(9, 16, in2 as i32, data_0 as i32, 0);
            log::info!("use_pile: spawning monster at ({}, {})", x, y);
        }
    }

    1
}

pub fn use_grave(gs: &mut GameState, _cn: usize, item_idx: usize) -> i32 {
    // Get previously spawned character
    let cc = gs.items[item_idx].data[0] as usize;

    // Check if still alive and linked
    if cc > 0 {
        let is_alive = cc < gs.characters.len() && {
            let ch = &gs.characters[cc];
            ch.data[0] as usize == item_idx
                && (ch.flags & CharacterFlags::Body.bits()) == 0
                && ch.used != USE_EMPTY
        };

        if is_alive {
            return 1; // Still alive, don't spawn new one
        }
    }

    let cc = match populate::pop_create_char(gs, 328, false) {
        Some(cc) => cc,
        None => return 1,
    };

    let (item_x, item_y) = (gs.items[item_idx].x, gs.items[item_idx].y);

    if !God::drop_char_fuzzy(gs, cc, item_x as usize, item_y as usize) {
        God::destroy_items(gs, cc);
        {
            gs.characters[cc].used = USE_EMPTY;
        };
        return 1;
    }

    // Create link between item and character
    {
        gs.characters[cc].data[0] = item_idx as i32;
    };

    {
        gs.items[item_idx].data[0] = cc as u32;
    };

    1
}

pub fn mine_wall(gs: &mut GameState, cn: usize, item_idx: usize) -> i32 {
    // If no item provided, get it from the map
    let in_idx = if item_idx == 0 {
        let (x, y) = {
            if cn == 0 {
                (0, 0)
            } else {
                (gs.characters[cn].x as usize, gs.characters[cn].y as usize)
            }
        };
        let m = x + y * core::constants::SERVER_MAPX as usize;
        let map_item = gs.map[m].it;
        if map_item == 0 {
            return 0;
        }
        map_item as usize
    } else {
        item_idx
    };

    // Get original template, position, and carried status
    let (temp, item_x, item_y, carried, should_rebuild) = {
        (
            gs.items[in_idx].data[0] as usize,
            gs.items[in_idx].x as i32,
            gs.items[in_idx].y as i32,
            gs.items[in_idx].carried,
            gs.items[in_idx].data[3] != 0,
        )
    };

    // Add rebuild wall effect if data[3] is set
    if should_rebuild {
        // Use the template id as the effect parameter (matches original server behavior)
        let temp_id = gs.items[in_idx].temp;
        EffectManager::fx_add_effect(
            10,
            core::constants::TICKS * 60 * 15,
            item_x,
            item_y,
            temp_id as i32,
        );
        log::info!("mine_wall: added rebuild effect (temp={})", temp_id);
    }

    // Replace the item with a copy of the item template (it_temp[temp]) and
    // restore position/carried/temp fields (this mirrors the original C++ behavior).
    let template_copy = gs.item_templates[temp];
    {
        gs.items[in_idx] = template_copy;
        gs.items[in_idx].x = item_x as u16;
        gs.items[in_idx].y = item_y as u16;
        gs.items[in_idx].carried = carried;
        gs.items[in_idx].temp = temp as u16;
        if carried != 0 {
            gs.items[in_idx].flags |= ItemFlags::IF_UPDATE.bits();
        }
    };

    gs.items[in_idx].data[2] as i32
}

// Un-called in the original code
#[allow(dead_code)]
pub fn mine_state(gs: &mut GameState, _cn: usize, item_idx: usize) -> i32 {
    if item_idx == 0 {
        return 0;
    }

    // Check if item is a mine wall (driver 25)
    let is_mine_wall = gs.items[item_idx].driver == 25;
    if !is_mine_wall {
        return 0;
    }

    // Return state from data[2]
    gs.items[item_idx].data[2] as i32
}

pub fn use_mine(gs: &mut GameState, cn: usize, item_idx: usize) -> i32 {
    use core::constants::{AT_STREN, WN_RHAND};

    // Get character strength
    let mut str = { gs.characters[cn].attrib[AT_STREN as usize][5] as i32 };

    // Check and subtract endurance
    let insufficient_endurance = {
        if gs.characters[cn].a_end < 1500 {
            true
        } else {
            gs.characters[cn].a_end -= 1000;
            false
        }
    };

    if insufficient_endurance {
        gs.do_character_log(
            cn,
            core::types::FontColor::Green,
            "You're too exhausted to continue digging.\n",
        );
        {
            gs.characters[cn].misc_action = 0; // DR_IDLE
        };
        return 0;
    }

    // Check for proper tools in right hand
    let (has_pickaxe, has_weapon) = {
        let in2 = gs.characters[cn].worn[WN_RHAND] as usize;
        if in2 != 0 {
            {
                let temp = gs.items[in2].temp;
                (temp == 458, true) // 458 is pickaxe
            }
        } else {
            (false, false)
        }
    };

    if has_weapon {
        if has_pickaxe {
            item_damage_weapon(gs, cn, str / 10);
            str *= 2;
        } else {
            item_damage_weapon(gs, cn, str * 10);
            str /= 4;
        }
        GameState::char_play_sound(cn, 11, -150, 0);
        gs.do_area_sound(
            cn,
            0,
            gs.characters[cn].x as i32,
            gs.characters[cn].y as i32,
            11,
        );
    } else {
        str /= 10;
        let low_health = {
            if gs.characters[cn].a_hp < 10000 {
                true
            } else {
                gs.characters[cn].a_hp -= 500;
                false
            }
        };

        if low_health {
            gs.do_character_log(
                cn,
                core::types::FontColor::Green,
                "You don't want to kill yourself beating at this wall with your bare hands, so you stop.\n",
            );
            {
                gs.characters[cn].misc_action = 0; // DR_IDLE
            };
            return 0;
        }
    }

    // Apply damage to mine wall
    let tmp = {
        let current = gs.items[item_idx].data[1] as i32;
        let new_val = current - str;
        if new_val > 0 {
            gs.items[item_idx].data[1] = new_val as u32;
        }
        new_val
    };

    if tmp <= 0 {
        // Wall destroyed
        let (x, y) = (gs.items[item_idx].x, gs.items[item_idx].y);
        gs.reset_go(x as i32, y as i32);
        gs.remove_lights(x as i32, y as i32);

        let _result = mine_wall(gs, cn, item_idx);

        gs.reset_go(x as i32, y as i32);
        gs.add_lights(x as i32, y as i32);
    }

    0
}

pub fn use_mine_fast(gs: &mut GameState, cn: usize, item_idx: usize) -> i32 {
    if cn == 0 {
        return 0;
    }

    let carried = gs.items[item_idx].carried;
    if carried != 0 {
        return 0;
    }

    // Get item position and template
    let (x, y, temp) = {
        (
            gs.items[item_idx].x,
            gs.items[item_idx].y,
            gs.items[item_idx].temp,
        )
    };

    EffectManager::fx_add_effect(
        10,
        core::constants::TICKS * 60 * 15,
        x as i32,
        y as i32,
        temp as i32,
    );

    gs.reset_go(x as i32, y as i32);
    gs.remove_lights(x as i32, y as i32);

    // Remove item from map
    {
        gs.map[(x + y * SERVER_MAPX as u16) as usize].it = 0;
    };

    {
        gs.items[item_idx].used = USE_EMPTY;
    };

    gs.reset_go(x as i32, y as i32);
    gs.add_lights(x as i32, y as i32);

    1
}

pub fn build_ring(gs: &mut GameState, cn: usize, item_idx: usize) -> i32 {
    use crate::god::God;
    use core::constants::{ItemFlags, USE_EMPTY};

    // Get ring base template
    let t1 = gs.items[item_idx].temp;

    // Get citem template
    let (in2, t2) = {
        let in2 = gs.characters[cn].citem as usize;
        if in2 == 0 {
            (0, 0)
        } else {
            let t2 = gs.items[in2].temp;
            (in2, t2)
        }
    };

    // Determine result template
    let r = if t1 == 360 && t2 == 0 {
        337 // plain golden ring
    } else if t1 == 361 && t2 == 0 {
        338 // plain silver ring
    } else if t1 == 337 {
        // golden ring with gem
        match t2 {
            339 => 362, // small ruby
            340 => 363, // med ruby
            341 => 364, // big ruby
            342 => 365, // small emerald
            343 => 366, // med emerald
            344 => 367, // big emerald
            345 => 368, // small saphire
            346 => 369, // med saphire
            347 => 370, // big saphire
            487 => 490, // huge ruby
            488 => 491, // huge emerald
            489 => 492, // huge saphire
            _ => return 0,
        }
    } else if t1 == 338 {
        // silver ring with gem
        match t2 {
            339 => 348, // small ruby
            340 => 349, // med ruby
            341 => 350, // big ruby
            342 => 351, // small emerald
            343 => 352, // med emerald
            344 => 353, // big emerald
            345 => 354, // small saphire
            346 => 355, // med saphire
            347 => 356, // big saphire
            487..=489 => {
                // Huge gems too powerful for silver
                gs.do_character_log(
                    cn,
                    core::types::FontColor::Green,
                    "This stone is too powerful for a silver ring.\n",
                );
                return 0;
            }
            _ => return 0,
        }
    } else {
        return 0;
    };

    // Create result item
    if let Some(in3) = God::create_item(gs, r) {
        {
            gs.items[in3].flags |= ItemFlags::IF_UPDATE.bits();
        };

        // Remove gem if used
        if in2 != 0 {
            {
                gs.characters[cn].citem = 0;
            };
            {
                gs.items[in2].used = USE_EMPTY;
            };
        }

        // Remove ring base
        take_item_from_char(gs, item_idx, cn);
        {
            gs.items[item_idx].used = USE_EMPTY;
        };

        // Give result to character
        God::give_character_item(gs, cn, in3);

        return 1;
    }

    0
}

pub fn build_amulet(gs: &mut GameState, cn: usize, item_idx: usize) -> i32 {
    // Get amulet piece template
    let t1 = gs.items[item_idx].temp;

    // Get citem
    let in2 = gs.characters[cn].citem as usize;

    if in2 == 0 || (in2 & 0x80000000) != 0 {
        gs.do_character_log(cn, core::types::FontColor::Yellow, "Nothing happens.\n");
        return 0;
    }

    let t2 = gs.items[in2].temp;

    // Determine result based on combination
    let r = if (t1 == 471 && t2 == 472) || (t1 == 472 && t2 == 471) {
        476
    } else if (t1 == 471 && t2 == 473) || (t1 == 473 && t2 == 471) {
        474
    } else if (t1 == 472 && t2 == 473) || (t1 == 473 && t2 == 472) {
        475
    } else if (t1 == 471 && t2 == 475) || (t1 == 475 && t2 == 471) {
        466
    } else if (t1 == 472 && t2 == 474) || (t1 == 474 && t2 == 472) {
        466
    } else if (t1 == 473 && t2 == 476) || (t1 == 476 && t2 == 473) {
        466
    } else {
        gs.do_character_log(cn, core::types::FontColor::Yellow, "That doesn't fit.\n");
        return 0;
    };

    // Create result item
    if let Some(in3) = God::create_item(gs, r) {
        {
            gs.items[in3].flags |= ItemFlags::IF_UPDATE.bits();
        };

        // Remove components
        {
            gs.characters[cn].citem = 0;
        };
        {
            gs.items[in2].used = USE_EMPTY;
        };

        take_item_from_char(gs, item_idx, cn);
        {
            gs.items[item_idx].used = USE_EMPTY;
        };

        // Give result to character
        God::give_character_item(gs, cn, in3);

        return 1;
    }

    0
}

pub fn use_gargoyle(gs: &mut GameState, cn: usize, item_idx: usize) -> i32 {
    use crate::god::God;
    use core::constants::USE_EMPTY;

    if cn == 0 {
        return 0;
    }

    let carried = gs.items[item_idx].carried;
    if carried == 0 {
        return 0;
    }

    // Create gargoyle character (template 325)
    let cc = match God::create_char(gs, 325, true) {
        Some(cc) => cc as usize,
        None => return 0,
    };

    // Get character position
    let (ch_x, ch_y) = (gs.characters[cn].x, gs.characters[cn].y);

    // Try to drop near character
    if !God::drop_char_fuzzy(gs, cc, ch_x as usize, ch_y as usize) {
        {
            gs.characters[cc].used = USE_EMPTY;
        };
        God::destroy_items(gs, cc);
        gs.do_character_log(
            cn,
            core::types::FontColor::Green,
            "The Gargoyle could not materialize.\n",
        );
        return 0;
    }

    // Remove item
    take_item_from_char(gs, item_idx, cn);
    {
        gs.items[item_idx].used = USE_EMPTY;
    };

    // Configure gargoyle
    let ticker = gs.globals.ticker;
    {
        gs.characters[cc].data[42] = 65536 + cn as i32; // set group
        gs.characters[cc].data[59] = 65536 + cn as i32; // protect all members
        gs.characters[cc].data[63] = cn as i32; // obey and protect char
        gs.characters[cc].data[69] = cn as i32; // follow char
        gs.characters[cc].data[64] = ticker + (TICKS * 60 * 15);
    };

    1
}

pub fn use_grolm(gs: &mut GameState, cn: usize, item_idx: usize) -> i32 {
    use crate::god::God;
    use core::constants::USE_EMPTY;

    if cn == 0 {
        return 0;
    }

    let carried = gs.items[item_idx].carried;
    if carried == 0 {
        return 0;
    }

    // Create grolm character (template 577)
    let cc = match God::create_char(gs, 577, true) {
        Some(cc) => cc as usize,
        None => return 0,
    };

    // Get character position
    let (ch_x, ch_y) = (gs.characters[cn].x, gs.characters[cn].y);

    // Try to drop near character
    if !God::drop_char_fuzzy(gs, cc, ch_x as usize, ch_y as usize) {
        {
            gs.characters[cc].used = USE_EMPTY;
        };
        God::destroy_items(gs, cc);
        gs.do_character_log(
            cn,
            core::types::FontColor::Green,
            "The Grolm could not materialize.\n",
        );
        return 0;
    }

    // Remove item
    take_item_from_char(gs, item_idx, cn);
    {
        gs.items[item_idx].used = USE_EMPTY;
    };

    // Configure grolm
    let ticker = gs.globals.ticker;
    {
        gs.characters[cc].data[42] = 65536 + cn as i32; // set group
        gs.characters[cc].data[59] = 65536 + cn as i32; // protect all members
        gs.characters[cc].data[63] = cn as i32; // obey and protect char
        gs.characters[cc].data[69] = cn as i32; // follow char
        gs.characters[cc].data[64] = ticker + (TICKS * 60 * 15);
    };

    1
}

pub fn boost_char(gs: &mut GameState, cn: usize, divi: usize) -> i32 {
    // Boost attributes
    {
        for n in 0..5 {
            if gs.characters[cn].attrib[n][0] as i32 > divi as i32 {
                let boost =
                    crate::helpers::random_mod(gs.characters[cn].attrib[n][0] as u32 / divi as u32)
                        as u8;
                gs.characters[cn].attrib[n][0] =
                    gs.characters[cn].attrib[n][0].saturating_add(boost);
            }
        }

        // Boost skills
        for n in 0..MAXSKILL {
            if gs.characters[cn].skill[n][0] as i32 > divi as i32 {
                let boost =
                    crate::helpers::random_mod(gs.characters[cn].skill[n][0] as u32 / divi as u32)
                        as u8;
                gs.characters[cn].skill[n][0] = gs.characters[cn].skill[n][0].saturating_add(boost);
            }
        }

        // Update name
        let old_name = gs.characters[cn].get_name();
        let new_name = format!("Strong {}", old_name);
        let new_name_bytes = new_name.as_bytes();
        let len = new_name_bytes.len().min(39);
        gs.characters[cn].name[..len].copy_from_slice(&new_name_bytes[..len]);
        gs.characters[cn].name[len..].fill(0);
        gs.characters[cn].reference = gs.characters[cn].name;
    };

    // Create soulstone
    if let Some(in_idx) = God::create_item(gs, 1146) {
        let (exp, rank) = {
            let exp = gs.characters[cn].points_tot as u32 / 10
                + crate::helpers::random_mod(gs.characters[cn].points_tot as u32 / 20 + 1);
            let rank = core::ranks::points2rank(exp);
            (exp, rank)
        };

        {
            let name = b"Soulstone";
            gs.items[in_idx].name[..name.len()].copy_from_slice(name);
            gs.items[in_idx].name[name.len()..].fill(0);

            let reference = b"soulstone";
            gs.items[in_idx].reference[..reference.len()].copy_from_slice(reference);
            gs.items[in_idx].reference[reference.len()..].fill(0);

            let description = format!("Level {} soulstone, holding {} exp.", rank, exp);
            let desc_bytes = description.as_bytes();
            let len = desc_bytes.len().min(gs.items[in_idx].description.len());
            gs.items[in_idx].description[..len].copy_from_slice(&desc_bytes[..len]);
            gs.items[in_idx].description[len..].fill(0);

            gs.items[in_idx].data[0] = rank;
            gs.items[in_idx].data[1] = exp;
            gs.items[in_idx].temp = 0;
            gs.items[in_idx].driver = 68;
        };

        God::give_character_item(gs, cn, in_idx);
    }

    0
}

pub fn spawn_penta_enemy(gs: &mut GameState, item_idx: usize) -> i32 {
    // Determine enemy type from data[9]
    let data9 = gs.items[item_idx].data[9];

    let mut tmp = if data9 == 10 {
        crate::helpers::random_mod(2) + 9
    } else if data9 == 11 {
        crate::helpers::random_mod(2) + 11
    } else if data9 == 17 {
        crate::helpers::random_mod(2) + 17
    } else if data9 == 18 {
        crate::helpers::random_mod(2) + 18
    } else if data9 == 21 {
        22
    } else if data9 == 22 {
        23
    } else if data9 == 23 {
        24
    } else {
        crate::helpers::random_mod(3) + data9 - 1
    };

    // Create appropriate character template
    let spawned = if tmp >= 22 {
        tmp -= 22;
        if tmp > 3 {
            tmp = 3;
        }
        populate::pop_create_char(gs, (1094 + tmp) as usize, false)
    } else if tmp > 17 {
        tmp -= 17;
        if tmp > 4 {
            tmp = 4;
        }
        populate::pop_create_char(gs, (538 + tmp) as usize, false)
    } else {
        populate::pop_create_char(gs, (364 + tmp) as usize, false)
    };

    let cn = match spawned {
        Some(cn) => cn,
        None => return 0,
    };

    // Configure character
    {
        gs.characters[cn].flags &= !CharacterFlags::Respawn.bits();
    };

    let (x, y) = (gs.items[item_idx].x, gs.items[item_idx].y);

    {
        gs.characters[cn].data[0] = item_idx as i32;
        gs.characters[cn].data[29] = x as i32 + y as i32 * core::constants::SERVER_MAPX;
        gs.characters[cn].data[60] = TICKS * 60 * 2;
        gs.characters[cn].data[73] = 8;
        gs.characters[cn].dir = DX_RIGHT;
    };

    // Randomly boost character (1 in 25 chance)
    if crate::helpers::random_mod(25) == 0 {
        boost_char(gs, cn, 5);
    }

    // Try to drop character
    if !God::drop_char_fuzzy(gs, cn, x as usize, y as usize) {
        God::destroy_items(gs, cn);
        {
            gs.characters[cn].used = USE_EMPTY;
        };
        return 0;
    }

    cn as i32
}

pub fn solved_pentagram(gs: &mut GameState, cn: usize, item_idx: usize) -> i32 {
    // Calculate bonus
    let bonus = {
        let data0 = gs.items[item_idx].data[0];
        (data0 * data0 * 3) / 7 + 1
    };

    // Add bonus to character's pending exp
    {
        gs.characters[cn].data[18] += bonus as i32;
    };

    // Log to character
    gs.do_character_log(
        cn,
        core::types::FontColor::Green,
        &format!(
            "You solved the pentagram quest. Congratulations! You will get {} bonus experience points.\n",
            bonus
        ),
    );

    log::info!("Character {} solved pentagram quest", cn);

    let cn_name = gs.characters[cn].get_name().to_string();
    let mut characters_in_pents: usize = 0;

    // Notify all players and award pending exp
    for n in 1..core::constants::MAXCHARS {
        if n >= gs.characters.len() {
            continue;
        }
        let (used, flags, active, has_bonus) = (
            gs.characters[n].used,
            gs.characters[n].flags,
            if gs.characters[n].used == core::constants::USE_ACTIVE {
                1
            } else {
                0
            },
            gs.characters[n].data[18],
        );

        if used == core::constants::USE_EMPTY {
            continue;
        }
        if (flags & (CharacterFlags::Player.bits() | CharacterFlags::Usurp.bits())) == 0 {
            continue;
        }

        // Notify other active players
        if active != 0 && n != cn {
            gs.do_character_log(
                n,
                core::types::FontColor::Green,
                &format!("{} solved the pentagram quest!\n", cn_name),
            );
        }

        // Award pending bonus exp
        if has_bonus != 0 {
            gs.do_give_exp(n, has_bonus, 0, -1);
            {
                gs.characters[n].data[18] = 0;
            };
        }

        if area::is_in_pentagram_quest_gs(gs, n) {
            characters_in_pents += 1;
        }
    }

    // Activate all pentagram items (driver 33)
    {
        for n in 1..gs.items.len() {
            if gs.items[n].used == core::constants::USE_EMPTY {
                continue;
            }
            if gs.items[n].driver != 33 {
                continue;
            }
            if gs.items[n].active == 0 {
                if gs.items[n].light[0] != gs.items[n].light[1] && gs.items[n].x > 0 {
                    gs.do_add_light(
                        gs.items[n].x as i32,
                        gs.items[n].y as i32,
                        gs.items[n].light[1] as i32 - gs.items[n].light[0] as i32,
                    );
                }
            }
            gs.items[n].duration = 10 * 60 + crate::helpers::random_mod(20 * 60);
            gs.items[n].active = gs.items[n].duration;
        }
    };

    let new_solve = {
        let state = gs;
        state.penta_needed = characters_in_pents * 5 + crate::helpers::random_mod_usize(6);

        // Ensure at least 5 are needed
        state.penta_needed.max(5)
    };

    log::info!(
        "Pentagram quest solved. Characters in pents: {}, new penta_needed: {}",
        characters_in_pents,
        new_solve
    );

    0
}

pub fn use_pentagram(gs: &mut GameState, cn: usize, item_idx: usize) -> i32 {
    // Check if already active
    let active = gs.items[item_idx].active;
    if active != 0 {
        if cn != 0 {
            gs.do_character_log(
                cn,
                core::types::FontColor::Green,
                "This pentagram is already active.\n",
            );
        } else {
            // Respawn enemies if needed
            for m in 1..4 {
                let needs_spawn = {
                    let co = gs.items[item_idx].data[m] as usize;
                    let needs_spawn = if co == 0 {
                        true
                    } else {
                        {
                            if co >= gs.characters.len() || gs.characters[co].used == USE_EMPTY {
                                true
                            } else if gs.characters[co].data[0] != item_idx as i32 {
                                true
                            } else {
                                (gs.characters[co].flags & CharacterFlags::Body.bits()) != 0
                            }
                        }
                    };
                    needs_spawn
                };

                if needs_spawn {
                    let new_enemy = spawn_penta_enemy(gs, item_idx);
                    {
                        gs.items[item_idx].data[m] = new_enemy as u32;
                    };
                }
            }
        }
        return 0;
    }

    if cn == 0 {
        return 0;
    }

    // Check rank restriction
    let (r1, r2) = {
        let r1 = core::ranks::points2rank(gs.characters[cn].points_tot as u32) as i32;
        let r2 = {
            let mut r2 = gs.items[item_idx].data[9];
            if r2 < 5 {
                r2 += 5;
            } else if r2 < 7 {
                r2 += 6;
            } else if r2 < 9 {
                r2 += 7;
            } else if r2 < 11 {
                r2 += 8;
            } else {
                r2 += 9;
            }
            r2
        };
        (r1, r2)
    };

    if r1 as u32 > r2 {
        gs.do_character_log(
            cn,
            core::types::FontColor::Green,
            &format!(
                "You cannot use this pentagram. It is reserved for rank {} and below.\n",
                r2
            ),
        );
        return 0;
    }

    // Activate pentagram
    let v = {
        let v = gs.items[item_idx].data[0];
        gs.items[item_idx].data[8] = cn as u32;
        gs.items[item_idx].duration = u32::MAX; // TODO: What should this be? Max int?
        v
    };

    let exp_points = (v * v) / 7 + 1;
    gs.do_character_log(
        cn,
        core::types::FontColor::Yellow,
        &format!(
            "You activated the pentagram with the value {}. It is worth {} experience point{}.\n",
            v,
            exp_points,
            if v == 1 { "" } else { "s" }
        ),
    );

    // Count active pentagrams and find top 5
    let mut tot = 0;
    let mut act = 0;
    let mut exp = 0;
    let mut b = [0usize; 5];
    let mut bv = [0i32; 5];

    for n in 1..MAXITEM {
        if n >= gs.items.len() {
            break;
        }
        let (item_used, item_driver, item_active, item_data8, item_data0) = (
            gs.items[n].used,
            gs.items[n].driver,
            gs.items[n].active,
            gs.items[n].data[8],
            gs.items[n].data[0],
        );

        if item_used == USE_EMPTY {
            continue;
        }
        if item_driver != 33 {
            continue;
        }
        tot += 1;
        if n != item_idx && item_active != u32::MAX {
            // TODO: This was -1 vs. 0 (now u32::MAX) before but I'm not sure how it worked? Need to re-evaluate...
            continue;
        }
        act += 1;
        if item_data8 as usize != cn {
            continue;
        }

        let v = item_data0;
        // Insert into sorted top 5 list
        if v > bv[0] as u32 {
            bv[4] = bv[3];
            bv[3] = bv[2];
            bv[2] = bv[1];
            bv[1] = bv[0];
            bv[0] = v as i32;
            b[4] = b[3];
            b[3] = b[2];
            b[2] = b[1];
            b[1] = b[0];
            b[0] = n;
        } else if v > bv[1] as u32 {
            bv[4] = bv[3];
            bv[3] = bv[2];
            bv[2] = bv[1];
            bv[1] = v as i32;
            b[4] = b[3];
            b[3] = b[2];
            b[2] = b[1];
            b[1] = n;
        } else if v > bv[2] as u32 {
            bv[4] = bv[3];
            bv[3] = bv[2];
            bv[2] = v as i32;
            b[4] = b[3];
            b[3] = b[2];
            b[2] = n;
        } else if v > bv[3] as u32 {
            bv[4] = bv[3];
            bv[3] = v as i32;
            b[4] = b[3];
            b[3] = n;
        } else if v > bv[4] as u32 {
            bv[4] = v as i32;
            b[4] = n;
        }
    }

    // Display top 5 pentagrams
    if b[0] != 0 {
        gs.do_character_log(cn, core::types::FontColor::Yellow, "You're holding:\n");
    }

    for n in 0..5 {
        if b[n] != 0 {
            let points = (bv[n] * bv[n]) / 7 + 1;
            gs.do_character_log(
                cn,
                core::types::FontColor::Yellow,
                &format!(
                    "Pentagram {:3}, worth {:5} point{}.\n",
                    bv[n],
                    points,
                    if bv[n] == 1 { "" } else { "s" }
                ),
            );
            exp += points;
        }
    }

    {
        gs.characters[cn].data[18] = exp;
    };

    gs.do_character_log(
        cn,
        core::types::FontColor::Yellow,
        &format!(
            "Your pentagrammas are worth a total of {} experience points. Note that only the highest 5 pentagrammas count towards your experience bonus.\n",
            exp
        ),
    );
    gs.do_character_log(
        cn,
        core::types::FontColor::Yellow,
        &format!(
            "There are {} pentagrammas total, of which {} are active.\n",
            tot, act
        ),
    );

    log::info!(
        "Character {} activated pentagram {} ({} of needed)",
        cn,
        v,
        act
    );

    // Check if quest solved
    let penta_needed = gs.penta_needed;
    if act >= penta_needed {
        solved_pentagram(gs, cn, item_idx);
        return 0;
    }

    // Spawn enemies
    for m in 1..4 {
        let needs_spawn = {
            let co = gs.items[item_idx].data[m] as usize;
            let needs_spawn = if co == 0 {
                true
            } else {
                {
                    if co >= gs.characters.len() || gs.characters[co].used == USE_EMPTY {
                        true
                    } else if gs.characters[co].data[0] != item_idx as i32 {
                        true
                    } else {
                        (gs.characters[co].flags & CharacterFlags::Body.bits()) != 0
                    }
                }
            };
            needs_spawn
        };

        if needs_spawn {
            let new_enemy = spawn_penta_enemy(gs, item_idx);
            {
                gs.items[item_idx].data[m] = new_enemy as u32;
            };
        }
    }

    1
}

pub fn use_shrine(gs: &mut GameState, cn: usize, _item_idx: usize) -> i32 {
    if cn == 0 {
        return 0;
    }

    let in2 = gs.characters[cn].citem as usize;

    if in2 == 0 {
        gs.do_character_log(
            cn,
            core::types::FontColor::Yellow,
            "You get the feeling that it would be apropriate to give the gods a present.\n",
        );
        return 0;
    }

    // Special-case: ONE FIRE POINT / ONE FAKE POINT
    if (in2 & 0x80000000) == 0 {
        let desc = {
            if in2 >= gs.items.len() {
                String::new()
            } else {
                c_string_to_str(&gs.items[in2].description).to_string()
            }
        };

        if desc == "ONE FIRE POINT" || desc == "ONE FAKE POINT" {
            let is_fire = desc == "ONE FIRE POINT";

            if is_fire {
                {
                    gs.characters[cn].data[70] += 1;
                };
                let fp = gs.characters[cn].data[70];
                gs.do_character_log(
                    cn,
                    core::types::FontColor::Yellow,
                    &format!(
                        "One fire point accounted for. You now have {} fire points.\n",
                        fp
                    ),
                );
            } else {
                let fp = gs.characters[cn].data[70];
                gs.do_character_log(
                    cn,
                    core::types::FontColor::Yellow,
                    &format!("Err, that's a fake point. You have {} fire points.\n", fp),
                );
            }

            {
                if in2 < gs.items.len() {
                    gs.items[in2].used = USE_EMPTY;
                }
            };
            {
                gs.characters[cn].citem = 0;
            };

            let (better, worse, equal, bestval, bestcn, bestcount) = {
                let mut better = 0;
                let mut worse = 0;
                let mut equal = 0;
                let mut bestval = 0;
                let mut bestcn = 0;
                let mut bestcount = 0;

                for m in 1..core::constants::MAXCHARS {
                    if gs.characters[m].used == core::constants::USE_EMPTY {
                        continue;
                    }
                    if (gs.characters[m].flags & CharacterFlags::Player.bits()) == 0 {
                        continue;
                    }
                    if gs.characters[m].data[70] == 0 {
                        continue;
                    }

                    if gs.characters[m].data[70] > gs.characters[cn].data[70] {
                        better += 1;
                    } else if gs.characters[m].data[70] < gs.characters[cn].data[70] {
                        worse += 1;
                    } else {
                        equal += 1;
                    }

                    if gs.characters[m].data[70] > bestval {
                        bestval = gs.characters[m].data[70];
                        bestcn = m;
                        bestcount = 0;
                    }
                    if gs.characters[m].data[70] == bestval {
                        bestcount += 1;
                    }
                }

                (better, worse, equal, bestval, bestcn, bestcount)
            };

            if equal > 1 {
                gs.do_character_log(
                    cn,
                    core::types::FontColor::Yellow,
                    &format!(
                        "Your rank is {}, there are {} participating players of the same rank, {} are worse.\n",
                        better + 1,
                        equal - 1,
                        worse
                    ),
                );
            } else {
                gs.do_character_log(
                    cn,
                    core::types::FontColor::Yellow,
                    &format!(
                        "Your rank is {} and {} participating players are worse.\n",
                        better + 1,
                        worse
                    ),
                );
            }

            gs.do_character_log(cn, core::types::FontColor::Yellow, " \n");

            if bestcount == 1 {
                let name = { gs.characters[bestcn].get_name().to_string() };
                gs.do_character_log(
                    cn,
                    core::types::FontColor::Yellow,
                    &format!(
                        "First place holder is {} with {} fire points.\n",
                        name, bestval
                    ),
                );
            } else {
                gs.do_character_log(
                    cn,
                    core::types::FontColor::Yellow,
                    &format!(
                        "First place is shared by {} players, all with {} fire points:\n",
                        bestcount, bestval
                    ),
                );
                gs.do_character_log(cn, core::types::FontColor::Yellow, " \n");

                {
                    for m in 1..core::constants::MAXCHARS {
                        if gs.characters[m].used == core::constants::USE_EMPTY {
                            continue;
                        }
                        if (gs.characters[m].flags & CharacterFlags::Player.bits()) == 0 {
                            continue;
                        }
                        if gs.characters[m].data[70] == 0 {
                            continue;
                        }
                        if gs.characters[m].data[70] == bestval {
                            let name = gs.characters[m].get_name();
                            gs.do_character_log(
                                cn,
                                core::types::FontColor::Yellow,
                                &format!("{}\n", name),
                            );
                        }
                    }
                };
            }

            gs.do_character_log(cn, core::types::FontColor::Yellow, " \n");

            return 0;
        }
    }

    // Calculate item value
    let val = if (in2 & 0x80000000) != 0 {
        // Money
        let val = (in2 & 0x7fffffff) as i32;
        {
            gs.characters[cn].citem = 0;
        };
        val
    } else {
        // Item
        let value = {
            let mut val = gs.items[in2].value;
            if (gs.items[in2].flags & ItemFlags::IF_UNIQUE.bits()) != 0 {
                val *= 4;
            }
            val
        };

        {
            gs.items[in2].used = USE_EMPTY;
        };
        {
            gs.characters[cn].citem = 0;
        };
        value as i32
    };

    let val = val + crate::helpers::random_mod(val as u32 + 1) as i32;

    // Calculate rank threshold
    let rank = {
        let r = core::ranks::points2rank(gs.characters[cn].points_tot as u32) as i32 + 1;
        r * r * r * 4
    };

    // Check if offering is acceptable
    if val >= rank {
        // Restore mana
        let mana_restored = {
            if gs.characters[cn].a_mana < gs.characters[cn].mana[5] as i32 * 1000 {
                gs.characters[cn].a_mana = gs.characters[cn].mana[5] as i32 * 1000;
                true
            } else {
                false
            }
        };

        if mana_restored {
            gs.do_character_log(
                cn,
                core::types::FontColor::Green,
                "You feel the hand of the Goddess of Magic touch your mind.\n",
            );
            let (x, y) = { (gs.characters[cn].x as i32, gs.characters[cn].y as i32) };

            EffectManager::fx_add_effect(6, 0, x, y, 0);
        }

        // Determine message based on value
        let message = if val >= rank * 64 {
            "The gods are madly in love with your offer.\n"
        } else if val >= rank * 32 {
            "The gods love your offer very much.\n"
        } else if val >= rank * 16 {
            "The gods love your offer.\n"
        } else if val >= rank * 8 {
            "The gods are very pleased with your offer.\n"
        } else if val >= rank * 4 {
            "The gods are pleased with your offer.\n"
        } else if val >= rank * 2 {
            "The gods deemed your offer apropriate.\n"
        } else {
            "The gods accepted your offer.\n"
        };

        gs.do_character_log(cn, core::types::FontColor::Yellow, message);

        // Increase luck
        if val != 0 && rank != 0 {
            let m = val / rank;
            {
                gs.characters[cn].luck += m;
            };
        }
    } else {
        // Offering not good enough
        let (message, luck_change) = if val < rank / 8 {
            ("You have angered the gods with your unworthy gift.\n", -2)
        } else if val < rank / 4 {
            ("The gods sneer at your gift.\n", -1)
        } else if val < rank / 2 {
            ("The gods think you're cheap.\n", 0)
        } else {
            (
                "You feel that it takes more than this to please the gods.\n",
                0,
            )
        };

        gs.do_character_log(cn, core::types::FontColor::Yellow, message);

        if luck_change != 0 {
            {
                gs.characters[cn].luck += luck_change;
            };
        }
    }

    // Show luck status
    gs.do_character_log(cn, core::types::FontColor::Yellow, " \n");

    let luck = gs.characters[cn].luck;
    let luck_message = if luck < -10000 {
        "You feel that the gods are mad at you.\n"
    } else if luck < 0 {
        "You feel that the gods are angry at you.\n"
    } else if luck < 100 {
        "You feel that the gods stance towards you is neutral.\n"
    } else if luck < 1000 {
        "You feel that the gods are pleased with you.\n"
    } else {
        "You feel that the gods are very fond of you.\n"
    };

    gs.do_character_log(cn, core::types::FontColor::Yellow, luck_message);

    1
}

pub fn use_kill_undead(gs: &mut GameState, cn: usize, item_idx: usize) -> i32 {
    if cn == 0 {
        return 0;
    }

    // Check if wielding the item
    let is_wielded = { gs.characters[cn].worn[core::constants::WN_RHAND] as usize == item_idx };

    if !is_wielded {
        return 0;
    }

    {
        EffectManager::fx_add_effect(
            7,
            0,
            gs.characters[cn].x as i32,
            gs.characters[cn].y as i32,
            0,
        )
    };

    // Get character position
    let (ch_x, ch_y) = { (gs.characters[cn].x as i32, gs.characters[cn].y as i32) };

    // Damage all undead in 8x8 area
    for y in (ch_y - 8)..(ch_y + 8) {
        if !(1..core::constants::SERVER_MAPY).contains(&y) {
            continue;
        }
        for x in (ch_x - 8)..(ch_x + 8) {
            if !(1..core::constants::SERVER_MAPX).contains(&x) {
                continue;
            }

            let co = { gs.map[(x + y * core::constants::SERVER_MAPX) as usize].ch as usize };

            if co != 0 {
                let is_undead = { (gs.characters[co].flags & CharacterFlags::Undead.bits()) != 0 };

                if is_undead {
                    gs.do_hurt(cn, co, 500, 2);
                    EffectManager::fx_add_effect(5, 0, x, y, 0);
                }
            }
        }
    }

    item_damage_worn(gs, cn, core::constants::WN_RHAND, 500);

    1
}

pub fn teleport3(gs: &mut GameState, cn: usize, item_idx: usize) -> i32 {
    if cn == 0 {
        return 1;
    }

    // Check if requires activation
    let (needs_activation, is_active) = {
        (
            (gs.items[item_idx].flags & ItemFlags::IF_USEACTIVATE.bits()) != 0,
            gs.items[item_idx].active != 0,
        )
    };

    if needs_activation && !is_active {
        return 1;
    }

    // Remove nolab items from citem
    let citem = gs.characters[cn].citem as usize;
    if citem != 0 && is_nolab_item(gs, citem) {
        let item_ref = gs.items[citem].reference;
        {
            gs.characters[cn].citem = 0;
        };
        {
            gs.items[citem].used = USE_EMPTY;
        };
        gs.do_character_log(
            cn,
            core::types::FontColor::Yellow,
            &format!("Your {} vanished.\n", c_string_to_str(&item_ref)),
        );
    }

    // Remove nolab items from inventory
    for n in 0..40 {
        let in2 = gs.characters[cn].item[n] as usize;
        if in2 != 0 && is_nolab_item(gs, in2) {
            let item_ref = gs.items[in2].reference;
            {
                gs.characters[cn].item[n] = 0;
            };
            {
                gs.items[in2].used = USE_EMPTY;
            };
            gs.do_character_log(
                cn,
                core::types::FontColor::Yellow,
                &format!("Your {} vanished.\n", c_string_to_str(&item_ref)),
            );
        }
    }

    // Remove recall spells
    for n in 0..20 {
        let in2 = gs.characters[cn].spell[n] as usize;
        if in2 != 0 {
            let temp = gs.items[in2].temp;
            if temp as usize == SK_RECALL {
                {
                    gs.characters[cn].spell[n] = 0;
                };
                {
                    gs.items[in2].used = USE_EMPTY;
                };
            }
        }
    }

    // Teleport
    {
        EffectManager::fx_add_effect(
            6,
            0,
            gs.characters[cn].x as i32,
            gs.characters[cn].y as i32,
            0,
        )
    };
    let (dest_x, dest_y) = {
        (
            gs.items[item_idx].data[0] as usize,
            gs.items[item_idx].data[1] as usize,
        )
    };
    God::transfer_char(gs, cn, dest_x, dest_y);
    {
        EffectManager::fx_add_effect(
            6,
            0,
            gs.characters[cn].x as i32,
            gs.characters[cn].y as i32,
            0,
        )
    };

    // Remove IF_LABYDESTROY items from citem
    let citem = gs.characters[cn].citem as usize;
    if citem != 0 && (citem & 0x80000000) == 0 {
        let has_flag = { (gs.items[citem].flags & ItemFlags::IF_LABYDESTROY.bits()) != 0 };
        if has_flag {
            let item_ref = { c_string_to_str(&gs.items[citem].reference).to_string() };
            {
                gs.characters[cn].citem = 0;
            };
            {
                gs.items[citem].used = USE_EMPTY;
            };
            gs.do_character_log(
                cn,
                core::types::FontColor::Yellow,
                &format!("Your {} vanished.\n", item_ref),
            );
        }
    }

    // Remove IF_LABYDESTROY items from inventory
    for n in 0..40 {
        let in2 = gs.characters[cn].item[n] as usize;
        if in2 != 0 {
            let (has_flag, item_ref) = {
                (
                    (gs.items[in2].flags & ItemFlags::IF_LABYDESTROY.bits()) != 0,
                    gs.items[in2].reference,
                )
            };
            if has_flag {
                {
                    gs.characters[cn].item[n] = 0;
                };
                {
                    gs.items[in2].used = USE_EMPTY;
                };
                gs.do_character_log(
                    cn,
                    core::types::FontColor::Yellow,
                    &format!("Your {} vanished.\n", c_string_to_str(&item_ref)),
                );
            }
        }
    }

    // Remove IF_LABYDESTROY items from worn
    for n in 0..20 {
        let in2 = gs.characters[cn].worn[n] as usize;
        if in2 != 0 {
            let (has_flag, item_ref) = {
                (
                    (gs.items[in2].flags & ItemFlags::IF_LABYDESTROY.bits()) != 0,
                    gs.items[in2].reference,
                )
            };
            if has_flag {
                {
                    gs.characters[cn].worn[n] = 0;
                };
                {
                    gs.items[in2].used = USE_EMPTY;
                };
                gs.do_character_log(
                    cn,
                    core::types::FontColor::Yellow,
                    &format!("Your {} vanished.\n", c_string_to_str(&item_ref)),
                );
            }
        }
    }

    // Update temple/tavern coordinates
    let (kindred, is_staff) = {
        (
            gs.characters[cn].kindred,
            (gs.characters[cn].flags & CharacterFlags::Staff.bits()) != 0,
        )
    };

    if (kindred & 0x00000001) != 0 {
        // KIN_PURPLE
        {
            gs.characters[cn].temple_x = 558;
            gs.characters[cn].temple_y = 542;
            gs.characters[cn].tavern_x = 558;
            gs.characters[cn].tavern_y = 542;
        };
    } else if is_staff {
        {
            gs.characters[cn].temple_x = 813;
            gs.characters[cn].temple_y = 165;
            gs.characters[cn].tavern_x = 813;
            gs.characters[cn].tavern_y = 165;
        };
    } else {
        {
            gs.characters[cn].temple_x = 512;
            gs.characters[cn].temple_y = 512;
            gs.characters[cn].tavern_x = 512;
            gs.characters[cn].tavern_y = 512;
        };
    }

    1
}

pub fn use_seyan_shrine(gs: &mut GameState, cn: usize, item_idx: usize) -> i32 {
    if cn == 0 {
        return 0;
    }

    // Check if character is Seyan'Du
    let is_seyan = { (gs.characters[cn].kindred & KIN_SEYAN_DU as i32) != 0 };

    if !is_seyan {
        gs.do_character_log(
            cn,
            core::types::FontColor::Yellow,
            "You have the feeling you're in the wrong place here.\n",
        );
        return 0;
    }

    // Check for existing Seyan'Du sword (driver 40)
    let mut in2 = gs.characters[cn].worn[WN_RHAND] as usize;

    let sword_valid = if in2 != 0 {
        gs.items[in2].driver == 40 && gs.items[in2].data[0] == cn as u32
    } else {
        false
    };

    // If no valid sword, replace old ones and create new one
    if !sword_valid {
        // Remove old swords (driver 40 for this character)
        for n in 1..MAXITEM {
            if n >= gs.items.len() {
                break;
            }
            let should_replace = gs.items[n].used == USE_ACTIVE
                && gs.items[n].driver == 40
                && gs.items[n].data[0] == cn as u32;

            if should_replace {
                // Replace with broken sword (template 683)
                let (x, y, carried) = (gs.items[n].x, gs.items[n].y, gs.items[n].carried);

                if let Some(broken_sword) = God::create_item(gs, 683) {
                    if broken_sword == 0 {
                        continue;
                    }
                    {
                        gs.items[broken_sword].x = x;
                        gs.items[broken_sword].y = y;
                        gs.items[broken_sword].carried = carried;
                        gs.items[broken_sword].temp = 683;
                        gs.items[broken_sword].flags |= ItemFlags::IF_UPDATE.bits();
                    };
                    {
                        gs.items[n].used = USE_EMPTY;
                    };
                }
            }
        }

        // Check luck requirement
        let luck = gs.characters[cn].luck;
        if luck < 50 {
            gs.do_character_log(
                cn,
                core::types::FontColor::Yellow,
                "Kwai, the great goddess of war, deemed you unworthy to receive a new blade.\n",
            );
            return 0;
        }

        // Create new Seyan'Du sword (template 682)
        let Some(new_sword) = God::create_item(gs, 682) else {
            return 0;
        };
        if new_sword == 0 {
            return 0;
        }
        in2 = new_sword;
        God::give_character_item(gs, cn, in2);
        {
            gs.items[in2].data[0] = cn as u32;
        };
        gs.do_character_log(
            cn,
            core::types::FontColor::Yellow,
            "Kwai, the great goddess of war, deemed you worthy to receive a new blade.\n",
        );
        {
            gs.characters[cn].luck -= 50;
        };
    }

    // Mark this shrine as visited
    let shrine_bit = gs.items[item_idx].data[0];
    let already_visited = { (gs.characters[cn].data[21] as u32 & shrine_bit) != 0 };

    if !already_visited {
        {
            gs.characters[cn].data[21] |= shrine_bit as i32;
        };
        gs.do_character_log(
            cn,
            core::types::FontColor::Yellow,
            "You found a new shrine of Kwai!\n",
        );
        {
            gs.characters[cn].luck += 10;
        };
    }

    // Count visited shrines
    let visited_bits = {
        let mut count = 0;
        let mut bit = 1u32;
        while bit != 0 {
            if (gs.characters[cn].data[21] & bit as i32) != 0 {
                count += 1;
            }
            bit = bit.wrapping_shl(1);
        }
        count
    };

    gs.do_character_log(
        cn,
        core::types::FontColor::Green,
        &format!(
            "You have visited {} of the 20 shrines of Kwai.\n",
            visited_bits
        ),
    );

    // Update sword weapon power based on shrines visited
    let cn_name = gs.characters[cn].name;
    {
        gs.items[in2].weapon[0] = 15 + visited_bits * 4;
        gs.items[in2].flags |= ItemFlags::IF_UPDATE.bits();
        gs.items[in2].temp = 0;
        let description = format!(
            "A huge, two-handed sword, engraved with runes and magic symbols. It bears the name {}.",
            c_string_to_str(&cn_name)
        );
        let desc_bytes = description.as_bytes();
        let len = desc_bytes.len().min(gs.items[in2].description.len());
        gs.items[in2].description[..len].copy_from_slice(&desc_bytes[..len]);
        if len < gs.items[in2].description.len() {
            gs.items[in2].description[len..].fill(0);
        }
    };

    gs.do_update_char(cn);

    0
}

pub fn use_seyan_door(gs: &mut GameState, cn: usize, item_idx: usize) -> i32 {
    if cn != 0 {
        // Check if character is Seyan'Du
        let is_seyan = { (gs.characters[cn].kindred & KIN_SEYAN_DU as i32) != 0 };
        if !is_seyan {
            gs.do_character_log(
                cn,
                core::types::FontColor::Red,
                "You have the feeling this isn't meant for you.\n",
            );
            return 0;
        }
    }

    use_door(gs, cn, item_idx)
}

pub fn use_seyan_portal(gs: &mut GameState, cn: usize, item_idx: usize) -> i32 {
    if cn == 0 {
        return 0;
    }

    let (is_seyan, is_male, cn_name) = {
        (
            (gs.characters[cn].kindred & KIN_SEYAN_DU as i32) != 0,
            (gs.characters[cn].kindred & KIN_MALE as i32) != 0,
            gs.characters[cn].get_name().to_string(),
        )
    };

    if is_seyan {
        gs.do_character_log(
            cn,
            core::types::FontColor::Blue,
            "You're already Seyan'Du, aren't you?\n",
        );
    } else {
        gs.do_character_log(
            cn,
            core::types::FontColor::Blue,
            &format!("The Seyan'Du welcome you among their ranks, {}!\n", cn_name),
        );

        // Change race: 13 for male Seyan'Du, 79 for female Seyan'Du
        if is_male {
            God::racechange(gs, cn, 13);
        } else {
            God::racechange(gs, cn, 79);
        }

        // Give Seyan'Du sword (template 682)
        let in2 = match God::create_item(gs, 682) {
            Some(id) => id,
            None => return 0,
        };
        God::give_character_item(gs, cn, in2);
        {
            gs.items[in2].data[0] = cn as u32;
        };
    }

    // Remove IF_LABYDESTROY items from citem
    let citem = gs.characters[cn].citem as usize;
    if citem != 0 && (citem & 0x80000000) == 0 {
        let has_flag = { (gs.items[citem].flags & ItemFlags::IF_LABYDESTROY.bits()) != 0 };
        if has_flag {
            let item_ref = gs.items[citem].reference;
            {
                gs.characters[cn].citem = 0;
            };
            {
                gs.items[citem].used = USE_EMPTY;
            };
            gs.do_character_log(
                cn,
                core::types::FontColor::Green,
                &format!("Your {} vanished.\n", c_string_to_str(&item_ref)),
            );
        }
    }

    // Remove IF_LABYDESTROY items from inventory
    for n in 0..40 {
        let in2 = gs.characters[cn].item[n] as usize;
        if in2 != 0 {
            let (has_flag, item_ref) = {
                (
                    (gs.items[in2].flags & ItemFlags::IF_LABYDESTROY.bits()) != 0,
                    gs.items[in2].reference,
                )
            };
            if has_flag {
                {
                    gs.characters[cn].item[n] = 0;
                };
                {
                    gs.items[in2].used = USE_EMPTY;
                };
                gs.do_character_log(
                    cn,
                    core::types::FontColor::Green,
                    &format!("Your {} vanished.\n", c_string_to_str(&item_ref)),
                );
            }
        }
    }

    // Remove IF_LABYDESTROY items from worn
    for n in 0..20 {
        let in2 = gs.characters[cn].worn[n] as usize;
        if in2 != 0 {
            let (has_flag, item_ref) = {
                (
                    (gs.items[in2].flags & ItemFlags::IF_LABYDESTROY.bits()) != 0,
                    gs.items[in2].reference,
                )
            };
            if has_flag {
                {
                    gs.characters[cn].worn[n] = 0;
                };
                {
                    gs.items[in2].used = USE_EMPTY;
                };
                gs.do_character_log(
                    cn,
                    core::types::FontColor::Green,
                    &format!("Your {} vanished.\n", c_string_to_str(&item_ref)),
                );
            }
        }
    }

    // Teleport
    {
        EffectManager::fx_add_effect(
            6,
            0,
            gs.characters[cn].x as i32,
            gs.characters[cn].y as i32,
            0,
        )
    };
    let (dest_x, dest_y) = {
        (
            gs.items[item_idx].data[0] as usize,
            gs.items[item_idx].data[1] as usize,
        )
    };
    God::transfer_char(gs, cn, dest_x, dest_y);
    {
        EffectManager::fx_add_effect(
            6,
            0,
            gs.characters[cn].x as i32,
            gs.characters[cn].y as i32,
            0,
        )
    };

    1
}

pub fn spell_scroll(gs: &mut GameState, cn: usize, item_idx: usize) -> i32 {
    // Read scroll data
    let (spell, power, charges) = {
        (
            gs.items[item_idx].data[0],
            gs.items[item_idx].data[1],
            gs.items[item_idx].data[2],
        )
    };

    if charges == 0 {
        gs.do_character_log(cn, core::types::FontColor::Yellow, "Nothing happened!\n");
        return 0;
    }

    // Get target (skill_target1 or self)
    let mut co = gs.characters[cn].skill_target1 as usize;
    if co == 0 {
        co = cn;
    }

    // Check if can see target
    let can_see = gs.do_char_can_see(cn, co) != 0;
    if !can_see {
        gs.do_character_log(
            cn,
            core::types::FontColor::Yellow,
            "You cannot see your target.\n",
        );
        return 0;
    }

    // Check attack spells for may_attack
    if spell as usize == SK_CURSE || spell as usize == SK_STUN {
        if gs.may_attack_msg(cn, co, true) == 0 {
            log::info!("Prevented from attacking target {}", co);
            return 0;
        }
    } else {
        if driver::player_or_ghost(gs, cn, co) == 0 {
            // Change target to self
            co = cn;
        }
    }

    // Cast spell
    let ret = match spell as usize {
        SK_LIGHT => {
            driver::spell_light(gs, cn, co, power as i32);
            1
        }
        SK_ENHANCE => {
            driver::spell_enhance(gs, cn, co, power as i32);
            1
        }
        SK_PROTECT => {
            driver::spell_protect(gs, cn, co, power as i32);
            1
        }
        SK_BLESS => {
            driver::spell_bless(gs, cn, co, power as i32);
            1
        }
        SK_MSHIELD => {
            driver::spell_mshield(gs, cn, co, power as i32);
            1
        }
        SK_CURSE => {
            let target_resistance = gs.characters[co].skill[SK_RESIST][5];
            if driver::chance_base(gs, cn, power as i32, 10, target_resistance as i32) != 0 {
                1
            } else {
                driver::spell_curse(gs, cn, co, power as i32)
            }
        }
        SK_STUN => {
            let target_resistance = gs.characters[co].skill[SK_RESIST][5];
            if driver::chance_base(gs, cn, power as i32, 12, target_resistance as i32) != 0 {
                1
            } else {
                driver::spell_stun(gs, cn, co, power as i32)
            }
        }
        _ => 0,
    };

    // Decrement charges if spell succeeded
    if ret != 0 {
        let new_charges = charges - 1;
        {
            gs.items[item_idx].data[2] = new_charges;
            gs.items[item_idx].value /= 2;
        };
        if new_charges < 1 {
            return 1; // Scroll consumed
        }
    }

    0
}

pub fn use_blook_pentagram(gs: &mut GameState, cn: usize, item_idx: usize) -> i32 {
    if cn == 0 {
        return 0;
    }

    gs.do_character_log(
        cn,
        core::types::FontColor::Green,
        "You try to wipe off the blood, but it seems to be coming back slowly.\n",
    );

    // Set blood state and update sprite
    {
        gs.items[item_idx].data[0] = 1;
        gs.items[item_idx].sprite[0] =
            gs.items[item_idx].data[1] as i16 + gs.items[item_idx].data[0] as i16;
    };

    1
}

pub fn use_create_npc(gs: &mut GameState, cn: usize, item_idx: usize) -> i32 {
    // Check if already active
    let active = gs.items[item_idx].active;
    if active != 0 {
        return 0;
    }

    if cn == 0 {
        return 0;
    }

    // Create NPC from template
    let template = gs.items[item_idx].data[0];
    let co = match populate::pop_create_char(gs, template as usize, false) {
        Some(co) => co,
        None => return 0,
    };

    // Drop NPC near item location
    let (x, y) = (gs.items[item_idx].x as usize, gs.items[item_idx].y as usize);
    if !God::drop_char_fuzzy(gs, co, x, y) {
        God::destroy_items(gs, co);
        {
            gs.characters[co].used = USE_EMPTY;
        };
        return 0;
    }

    // Link NPC to creator
    {
        gs.characters[co].data[0] = cn as i32;
    };

    1
}

pub fn use_rotate(gs: &mut GameState, cn: usize, item_idx: usize) -> i32 {
    if cn == 0 {
        return 0;
    }

    // Rotate item: increment data[1] (0-3), update sprite
    {
        gs.items[item_idx].data[1] += 1;
        if gs.items[item_idx].data[1] > 3 {
            gs.items[item_idx].data[1] = 0;
        }
        gs.items[item_idx].sprite[0] =
            gs.items[item_idx].data[0] as i16 + gs.items[item_idx].data[1] as i16;
        gs.items[item_idx].flags |= ItemFlags::IF_UPDATE.bits();
    };

    1
}

pub fn use_lab8_key(gs: &mut GameState, cn: usize, item_idx: usize) -> i32 {
    // data[0] = matching key part
    // data[1] = resulting key part
    // data[2] = (optional) other matching key part
    // data[3] = (optional) other resulting key part

    if cn == 0 {
        return 0;
    }

    let citem = gs.characters[cn].citem as usize;
    if citem == 0 || (citem & 0x80000000) != 0 {
        gs.do_character_log(cn, core::types::FontColor::Yellow, "Nothing happens.\n");
        return 0;
    }

    let carried = gs.items[item_idx].carried;
    if carried == 0 {
        gs.do_character_log(
            cn,
            core::types::FontColor::Red,
            "Too difficult to do on the ground.\n",
        );
        return 0;
    }

    // Check for matching parts
    let (data0, data1, data2, data3, citem_temp) = {
        (
            gs.items[item_idx].data[0],
            gs.items[item_idx].data[1],
            gs.items[item_idx].data[2],
            gs.items[item_idx].data[3],
            gs.items[citem].temp,
        )
    };

    let result_template = if data0 as u16 == citem_temp {
        data1
    } else if data2 as u16 == citem_temp {
        data3
    } else {
        0
    };

    if result_template == 0 {
        gs.do_character_log(
            cn,
            core::types::FontColor::Red,
            "Those don't fit together.\n",
        );
        return 0;
    }

    // Log the assembly
    let (item_name, citem_name) = {
        (
            gs.items[item_idx].get_name().to_string(),
            gs.items[citem].get_name().to_string(),
        )
    };
    log::info!("Added {} to {}", citem_name, item_name);

    // Remove both old parts
    God::take_from_char(gs, item_idx, cn);
    {
        gs.items[item_idx].used = USE_EMPTY;
    };

    {
        gs.characters[cn].citem = 0;
    };
    {
        gs.items[citem].used = USE_EMPTY;
    };

    // Create and give new key
    let new_key = God::create_item(gs, result_template as usize);
    {
        gs.items[new_key.unwrap()].flags |= core::constants::ItemFlags::IF_UPDATE.bits();
    };
    God::give_character_item(gs, cn, new_key.unwrap());

    1
}

pub fn use_lab8_shrine(gs: &mut GameState, cn: usize, item_idx: usize) -> i32 {
    // data[0] = item accepted as offering
    // data[1] = item returned as gift

    if cn == 0 {
        return 0;
    }

    let offer = gs.characters[cn].citem as usize;
    if offer == 0 {
        gs.do_character_log(
            cn,
            core::types::FontColor::Yellow,
            "You get the feeling that it would be appropriate to give the Goddess a present.\n",
        );
        return 0;
    }

    // Check if offering is money or wrong item
    let (offer_temp, expected_temp) = (gs.items[offer].temp, gs.items[item_idx].data[0]);

    if (offer & 0x80000000) != 0 || offer_temp as u32 != expected_temp {
        gs.do_character_log(
            cn,
            core::types::FontColor::Yellow,
            "The Goddess only wants her property back, and rejects your offer.\n",
        );
        return 0;
    }

    // Accept offering
    {
        gs.characters[cn].citem = 0;
    };
    {
        gs.items[offer].used = USE_EMPTY;
    };

    // Log the offering
    let (offer_ref, shrine_ref) = {
        (
            c_string_to_str(&gs.items[offer].reference).to_string(),
            c_string_to_str(&gs.items[item_idx].reference).to_string(),
        )
    };
    log::info!("Offered {} at {}", offer_ref, shrine_ref);

    // Create and give gift
    let gift_template = gs.items[item_idx].data[1];
    let gift = God::create_item(gs, gift_template as usize);

    if !God::give_character_item(gs, cn, gift.unwrap()) {
        // If inventory full, put in carried
        {
            gs.characters[cn].citem = gift.unwrap() as u32;
        };
        {
            gs.items[gift.unwrap()].carried = cn as u16;
        };
    }

    let gift_ref = { c_string_to_str(&gs.items[gift.unwrap()].reference).to_string() };
    gs.do_character_log(
        cn,
        core::types::FontColor::Yellow,
        &format!("The Goddess has given you a {} in return!\n", gift_ref),
    );

    1
}

pub fn use_lab8_moneyshrine(gs: &mut GameState, cn: usize, item_idx: usize) -> i32 {
    // data[0] = minimum offering accepted
    // data[1] = teleport coordinate x
    // data[2] = teleport coordinate y

    if cn == 0 {
        return 0;
    }

    let offer = gs.characters[cn].citem;
    if offer == 0 {
        gs.do_character_log(
            cn,
            core::types::FontColor::Yellow,
            "You get the feeling that it would be appropriate to give the Goddess a present.\n",
        );
        return 0;
    }

    // Check if it's money
    if (offer & 0x80000000) == 0 {
        gs.do_character_log(
            cn,
            core::types::FontColor::Yellow,
            "Only money is accepted at this shrine.\n",
        );
        return 0;
    }

    let amount = offer & 0x7fffffff;
    let min_offering = gs.items[item_idx].data[0];

    if amount < min_offering {
        gs.do_character_log(
            cn,
            core::types::FontColor::Yellow,
            "Your offering is not sufficient, and was rejected.\n",
        );
        return 0;
    }

    // Log offering
    let shrine_ref = c_string_to_str(&gs.items[item_idx].reference).to_string();
    log::info!("offered {}G at {}", amount / 100, shrine_ref);

    // Accept money and teleport
    {
        gs.characters[cn].citem = 0;
    };

    let (dest_x, dest_y) = (gs.items[item_idx].data[1], gs.items[item_idx].data[2]);
    God::transfer_char(gs, cn, dest_x as usize, dest_y as usize);

    // Restore mana if needed
    let (a_mana, max_mana) = { (gs.characters[cn].a_mana, gs.characters[cn].mana[5] * 1000) };

    if a_mana < max_mana as i32 {
        {
            gs.characters[cn].a_mana = gs.characters[cn].mana[5] as i32 * 1000;
        };
        gs.do_character_log(
            cn,
            core::types::FontColor::Red,
            "You feel the hand of the Goddess of Magic touch your mind.\n",
        );
        {
            EffectManager::fx_add_effect(
                6,
                0,
                gs.characters[cn].x as i32,
                gs.characters[cn].y as i32,
                0,
            )
        };
    }

    1
}

pub fn change_to_archtemplar(gs: &mut GameState, cn: usize) {
    // Check agility requirement
    let agility = gs.characters[cn].attrib[AT_AGIL as usize][0];
    if agility < 90 {
        gs.do_character_log(
            cn,
            core::types::FontColor::Yellow,
            "Your agility is too low. There is still room for improvement.\n",
        );
        return;
    }

    // Check strength requirement
    let strength = gs.characters[cn].attrib[AT_STREN as usize][0];
    if strength < 90 {
        gs.do_character_log(
            cn,
            core::types::FontColor::Yellow,
            "Your strength is too low. There is still room for improvement.\n",
        );
        return;
    }

    // Change race based on gender
    let (is_male, name) = {
        (
            (gs.characters[cn].kindred as u32 & KIN_MALE) != 0,
            gs.characters[cn].get_name().to_string(),
        )
    };

    let new_race = if is_male { 544 } else { 549 };
    God::minor_racechange(gs, cn, new_race);
    gs.do_character_log(
        cn,
        core::types::FontColor::Yellow,
        &format!(
            "You are truly worthy to become a Archtemplar. Congratulations, {}.\n",
            name
        ),
    );
}

pub fn change_to_archharakim(gs: &mut GameState, cn: usize) {
    // Check willpower requirement
    let willpower = gs.characters[cn].attrib[AT_WILL as usize][0];
    if willpower < 90 {
        gs.do_character_log(
            cn,
            core::types::FontColor::Yellow,
            "Your willpower is too low. There is still room for improvement.\n",
        );
        return;
    }

    // Check intuition requirement
    let intuition = gs.characters[cn].attrib[AT_INT as usize][0];
    if intuition < 90 {
        gs.do_character_log(
            cn,
            core::types::FontColor::Yellow,
            "Your intuition is too low. There is still room for improvement.\n",
        );
        return;
    }

    // Change race based on gender
    let (is_male, name) = {
        (
            (gs.characters[cn].kindred as u32 & KIN_MALE) != 0,
            gs.characters[cn].get_name().to_string(),
        )
    };

    let new_race = if is_male { 545 } else { 550 };
    God::minor_racechange(gs, cn, new_race);

    gs.do_character_log(
        cn,
        core::types::FontColor::Yellow,
        &format!(
            "You are truly worthy to become a Archharakim. Congratulations, {}.\n",
            name
        ),
    );
}

pub fn change_to_warrior(gs: &mut GameState, cn: usize) {
    // Check agility requirement
    let agility = gs.characters[cn].attrib[AT_AGIL as usize][0];
    if agility < 60 {
        gs.do_character_log(
            cn,
            core::types::FontColor::Yellow,
            "Your agility is too low. There is still room for improvement.\n",
        );
        return;
    }

    // Check strength requirement
    let strength = gs.characters[cn].attrib[AT_STREN as usize][0];
    if strength < 60 {
        gs.do_character_log(
            cn,
            core::types::FontColor::Yellow,
            "Your strength is too low. There is still room for improvement.\n",
        );
        return;
    }

    // Change race based on gender
    let (is_male, name) = {
        (
            (gs.characters[cn].kindred as u32 & KIN_MALE) != 0,
            gs.characters[cn].get_name().to_string(),
        )
    };

    let new_race = if is_male { 547 } else { 552 };
    God::minor_racechange(gs, cn, new_race);

    gs.do_character_log(
        cn,
        core::types::FontColor::Yellow,
        &format!(
            "You are truly worthy to become a Warrior. Congratulations, {}.\n",
            name
        ),
    );
}

pub fn change_to_sorcerer(gs: &mut GameState, cn: usize) {
    // Check willpower requirement
    let willpower = gs.characters[cn].attrib[AT_WILL as usize][0];
    if willpower < 60 {
        gs.do_character_log(
            cn,
            core::types::FontColor::Yellow,
            "Your willpower is too low. There is still room for improvement.\n",
        );
        return;
    }

    // Check intuition requirement
    let intuition = gs.characters[cn].attrib[AT_INT as usize][0];
    if intuition < 60 {
        gs.do_character_log(
            cn,
            core::types::FontColor::Yellow,
            "Your intuition is too low. There is still room for improvement.\n",
        );
        return;
    }

    // Change race based on gender
    let (is_male, name) = {
        (
            (gs.characters[cn].kindred as u32 & KIN_MALE) != 0,
            gs.characters[cn].get_name().to_string(),
        )
    };

    let new_race = if is_male { 546 } else { 551 };
    God::minor_racechange(gs, cn, new_race);

    gs.do_character_log(
        cn,
        core::types::FontColor::Yellow,
        &format!(
            "You are truly worthy to become a Sorcerer. Congratulations, {}.\n",
            name
        ),
    );
}

pub fn shrine_of_change(gs: &mut GameState, cn: usize, _item_idx: usize) -> i32 {
    // Requires specific potions to change character class
    // Potion of Life (148) -> Archtemplar/Archharakim
    // Greater Healing Potion (127/274) -> Warrior
    // Greater Mana Potion (131/273) -> Sorcerer

    if cn == 0 {
        return 0;
    }

    let citem = gs.characters[cn].citem as usize;
    if citem == 0 || (citem & 0x80000000) != 0 {
        gs.do_character_log(
            cn,
            core::types::FontColor::Yellow,
            "Read the notes, my friend.\n",
        );
        return 0;
    }

    let (citem_temp, kindred) = {
        let temp = gs.items[citem].temp;
        let kindred = gs.characters[cn].kindred;
        (temp, kindred)
    };

    // Potion of life -> Archtemplar/Archharakim
    if citem_temp == 148 {
        if (kindred as u32 & KIN_TEMPLAR) != 0 {
            change_to_archtemplar(gs, cn);
        } else if (kindred as u32 & KIN_HARAKIM) != 0 {
            change_to_archharakim(gs, cn);
        } else {
            gs.do_character_log(
                cn,
                core::types::FontColor::Yellow,
                "You are neither Templar nor Harakim.\n",
            );
        }
        return 0;
    }

    // Greater healing potion -> Warrior
    if citem_temp == 127 || citem_temp == 274 {
        if (kindred as u32 & KIN_MERCENARY) != 0 {
            change_to_warrior(gs, cn);
        } else {
            gs.do_character_log(
                cn,
                core::types::FontColor::Yellow,
                "You are not a Mercenary.\n",
            );
        }
        return 0;
    }

    // Greater mana potion -> Sorcerer
    if citem_temp == 131 || citem_temp == 273 {
        if (kindred as u32 & KIN_MERCENARY) != 0 {
            change_to_sorcerer(gs, cn);
        } else {
            gs.do_character_log(
                cn,
                core::types::FontColor::Yellow,
                "You are not a Mercenary.\n",
            );
        }
        return 0;
    }

    gs.do_character_log(
        cn,
        core::types::FontColor::Yellow,
        "Read the notes, my friend.\n",
    );
    0
}

pub fn explorer_point(gs: &mut GameState, cn: usize, item_idx: usize) -> i32 {
    // data[0-3] = bitmask for visited flags (stored in character data[46-49])
    // data[4] = base experience reward

    // Check if already visited
    let (data0, data1, data2, data3, char_data46, char_data47, char_data48, char_data49) = {
        let d0 = gs.items[item_idx].data[0];
        let d1 = gs.items[item_idx].data[1];
        let d2 = gs.items[item_idx].data[2];
        let d3 = gs.items[item_idx].data[3];
        {
            (
                d0,
                d1,
                d2,
                d3,
                gs.characters[cn].data[46],
                gs.characters[cn].data[47],
                gs.characters[cn].data[48],
                gs.characters[cn].data[49],
            )
        }
    };

    if ((char_data46 & data0 as i32) == 0)
        && ((char_data47 & data1 as i32) == 0)
        && ((char_data48 & data2 as i32) == 0)
        && ((char_data49 & data3 as i32) == 0)
    {
        // Mark as visited
        {
            gs.characters[cn].data[46] |= data0 as i32;
            gs.characters[cn].data[47] |= data1 as i32;
            gs.characters[cn].data[48] |= data2 as i32;
            gs.characters[cn].data[49] |= data3 as i32;
            gs.characters[cn].luck += 10;
        };

        gs.do_character_log(
            cn,
            core::types::FontColor::Red,
            "You found a new exploration point!\n",
        );

        // Calculate experience reward
        let (base_exp, points_tot) = {
            let base = gs.items[item_idx].data[4];
            let pts = gs.characters[cn].points_tot;
            (base, pts)
        };

        let mut exp = base_exp / 2 + helpers::random_mod(base_exp);
        exp = std::cmp::min(points_tot as u32 / 10, exp); // Not more than 10% of total experience
        exp += helpers::random_mod(exp / 10 + 1); // Some more randomness

        log::info!(
            "exp point giving {} ({}) exp, char has {} exp",
            exp,
            base_exp,
            points_tot
        );

        gs.do_give_exp(cn, exp as i32, 0, -1)
    } else {
        gs.do_character_log(
            cn,
            core::types::FontColor::Yellow,
            "Hmm. Seems somewhat familiar. You've been here before...\n",
        );
    }

    1
}

pub fn use_garbage(gs: &mut GameState, cn: usize, _item_idx: usize) -> i32 {
    if cn == 0 {
        return 0;
    }

    let citem = gs.characters[cn].citem;
    if citem == 0 {
        gs.do_character_log(
            cn,
            core::types::FontColor::Yellow,
            "You feel that you could dispose of unwanted items in this digusting garbage can.\n",
        );
        return 0;
    }

    if (citem & 0x80000000) != 0 {
        // Money
        let val = citem & 0x7fffffff;
        {
            gs.characters[cn].citem = 0;
        };

        gs.do_character_log(
            cn,
            core::types::FontColor::Yellow,
            &format!(
                "You disposed of {} gold and {} silver.\n",
                val / 100,
                val % 100
            ),
        );
    } else {
        // Item
        let reference = { c_string_to_str(&gs.items[citem as usize].reference).to_string() };

        {
            gs.characters[cn].citem = 0;
        };
        {
            gs.items[citem as usize].used = USE_EMPTY;
        };

        gs.do_character_log(
            cn,
            core::types::FontColor::Yellow,
            &format!("You disposed of the {}.\n", reference),
        );
    }

    1
}

pub fn use_driver(gs: &mut GameState, cn: usize, item_idx: usize, carried: bool) {
    if item_idx == 0 || cn >= 10000 {
        return;
    }

    // Check if character is in build mode
    if cn != 0 {
        let in_build_mode = { (gs.characters[cn].flags & CharacterFlags::BuildMode.bits()) != 0 };
        if in_build_mode {
            return;
        }
    }

    // Default to failed action for non-carried use; will be updated on success
    if cn != 0 && !carried {
        {
            gs.characters[cn].cerrno = core::constants::ERR_FAILED as u16;
        };
    }

    let has_use_flag =
        { (gs.items[item_idx].flags & core::constants::ItemFlags::IF_USE.bits()) != 0 };

    if !has_use_flag && cn != 0 {
        return;
    }

    // Check if tile is occupied (for non-carried items)
    if !carried {
        let (it_x, it_y) = (gs.items[item_idx].x as i32, gs.items[item_idx].y as i32);
        if it_x > 0 {
            let m = (it_x + it_y * core::constants::SERVER_MAPX) as usize;
            let occupied = gs.map[m].ch != 0 || gs.map[m].to_ch != 0;
            if occupied {
                return;
            }
        }
    }

    let has_usespecial =
        { (gs.items[item_idx].flags & core::constants::ItemFlags::IF_USESPECIAL.bits()) != 0 };

    if has_usespecial {
        let driver = gs.items[item_idx].driver;
        let ret = match driver {
            1 => use_create_item(gs, cn, item_idx),
            2 => use_door(gs, cn, item_idx),
            3 => {
                // Lock-pick - special message
                if cn != 0 {
                    gs.do_character_log(
                        cn,
                        core::types::FontColor::Red,
                        "You use cannot the lock-pick directly. Hold it under your mouse cursor and click on the door...\n",
                    );
                }
                0
            }
            4 => use_mix_potion(gs, cn, item_idx),
            5 => stone_sword(gs, cn, item_idx),
            6 => teleport(gs, cn, item_idx),
            7 => use_bag(gs, cn, item_idx),
            8 => use_scroll(gs, cn, item_idx),
            9 => use_crystal(gs, cn, item_idx),
            10 => use_scroll2(gs, cn, item_idx),
            11 => use_scroll3(gs, cn, item_idx),
            12 => use_scroll4(gs, cn, item_idx),
            13 => use_scroll5(gs, cn, item_idx),
            14 => use_chain(gs, cn, item_idx),
            15 => use_labyrinth(gs, cn, item_idx),
            16 => use_ladder(gs, cn, item_idx),
            17 => rat_eye(gs, cn, item_idx),
            18 => skua_protect(gs, cn, item_idx),
            19 => use_lever(gs, cn, item_idx),
            20 => use_door(gs, cn, item_idx),
            21 => use_spawn(gs, cn, item_idx),
            22 => use_pile(gs, cn, item_idx),
            23 => teleport2(gs, cn, item_idx),
            24 => build_ring(gs, cn, item_idx),
            25 => use_mine(gs, cn, item_idx),
            26 => use_mine_fast(gs, cn, item_idx),
            27 => use_mine_respawn(gs, cn, item_idx),
            28 => use_gargoyle(gs, cn, item_idx),
            29 => use_grave(gs, cn, item_idx),
            30 => use_create_item2(gs, cn, item_idx),
            31 => 0, // empty, hole water
            32 => build_amulet(gs, cn, item_idx),
            33 => use_pentagram(gs, cn, item_idx),
            34 => use_seyan_shrine(gs, cn, item_idx),
            35 => use_seyan_door(gs, cn, item_idx),
            36 => 0, // magic portal 1 in lab13
            37 => 0, // traps
            38 => 0, // magic portal 2 in lab13
            39 => purple_protect(gs, cn, item_idx),
            40 => 0, // seyan'du sword
            41 => use_shrine(gs, cn, item_idx),
            42 => use_create_item3(gs, cn, item_idx),
            43 => 0, // spiderweb
            44 => use_kill_undead(gs, cn, item_idx),
            45 => use_seyan_portal(gs, cn, item_idx),
            46 => teleport3(gs, cn, item_idx),
            47 => 0, // arena portal
            48 => spell_scroll(gs, cn, item_idx),
            49 => use_blook_pentagram(gs, cn, item_idx),
            50 => use_create_npc(gs, cn, item_idx),
            51 => use_rotate(gs, cn, item_idx),
            52 => 0, // personal item
            53 => use_create_item(gs, cn, item_idx),
            54 => use_create_item(gs, cn, item_idx),
            55 => shrine_of_change(gs, cn, item_idx),
            56 => 0, // greenling green ball
            57 => explorer_point(gs, cn, item_idx),
            58 => use_grolm(gs, cn, item_idx),
            59 => use_create_gold(gs, cn, item_idx),
            61 => use_lab8_key(gs, cn, item_idx),
            63 => use_lab8_shrine(gs, cn, item_idx),
            64 => use_lab8_moneyshrine(gs, cn, item_idx),
            65 => Labyrinth9::with(|lab9| lab9.use_lab9_switch(cn, item_idx as i32)) as i32,
            66 => Labyrinth9::with_mut(|lab9| lab9.use_lab9_door(cn, item_idx as i32)) as i32,
            67 => use_garbage(gs, cn, item_idx),
            68 => use_soulstone(gs, cn, item_idx),
            69 => 0,
            _ => {
                log::warn!(
                    "use_driver: Unknown use_driver {} for item {}",
                    driver,
                    item_idx
                );
                0
            }
        };

        if cn != 0 {
            if ret == 0 {
                if !carried {
                    {
                        gs.characters[cn].cerrno = core::constants::ERR_FAILED as u16;
                    };
                }
                return;
            }

            if !carried {
                {
                    gs.characters[cn].cerrno = core::constants::ERR_SUCCESS as u16;
                };
            }

            // Ensure client update for the acting character
            gs.do_update_char(cn);
        }
    }

    if cn == 0 {
        return; // item_tick does activate and deactivate as well
    }

    // Handle activation/deactivation
    let (active, has_usedeactivate, has_useactivate) = {
        (
            gs.items[item_idx].active,
            (gs.items[item_idx].flags & ItemFlags::IF_USEDEACTIVATE.bits()) != 0,
            (gs.items[item_idx].flags & ItemFlags::IF_USEACTIVATE.bits()) != 0,
        )
    };

    if active != 0 && has_usedeactivate {
        // deactivate: set active=0 and adjust lighting
        let (light0, light1, it_x, it_y) = {
            (
                gs.items[item_idx].light[0],
                gs.items[item_idx].light[1],
                gs.items[item_idx].x,
                gs.items[item_idx].y,
            )
        };

        gs.items[item_idx].active = 0;

        if light0 != light1 && it_x > 0 {
            gs.do_add_light(it_x as i32, it_y as i32, light0 as i32 - light1 as i32);
        }

        if carried {
            {
                gs.items[item_idx].flags |= core::constants::ItemFlags::IF_UPDATE.bits();
            };
            gs.do_update_char(cn);
        }

        if cn != 0 && !carried {
            {
                gs.characters[cn].cerrno = core::constants::ERR_SUCCESS as u16;
            };
        }
    } else if active == 0 && has_useactivate {
        // activate: set active=duration and adjust lighting
        let duration = gs.items[item_idx].duration;
        let (light0, light1, it_x, it_y) = {
            (
                gs.items[item_idx].light[0],
                gs.items[item_idx].light[1],
                gs.items[item_idx].x,
                gs.items[item_idx].y,
            )
        };

        gs.items[item_idx].active = duration;

        if light0 != light1 && it_x > 0 {
            gs.do_add_light(it_x as i32, it_y as i32, light1 as i32 - light0 as i32);
        }

        if carried {
            {
                gs.items[item_idx].flags |= core::constants::ItemFlags::IF_UPDATE.bits();
            };
            gs.do_update_char(cn);
        }

        if cn != 0 && !carried {
            {
                gs.characters[cn].cerrno = core::constants::ERR_SUCCESS as u16;
            };
        }
    }

    // Handle IF_USEDESTROY items (potions, etc.)
    if carried {
        let has_usedestroy =
            { (gs.items[item_idx].flags & core::constants::ItemFlags::IF_USEDESTROY.bits()) != 0 };

        if has_usedestroy {
            // Check min_rank requirement
            let min_rank = gs.items[item_idx].min_rank;
            let curr_rank = { core::ranks::points2rank(gs.characters[cn].points_tot as u32) };
            if min_rank as i32 > curr_rank as i32 {
                gs.do_character_log(
                    cn,
                    core::types::FontColor::Red,
                    "You're not experienced enough to use this.\n",
                );
                return;
            }

            // Log usage
            let item_name = gs.items[item_idx].get_name().to_string();
            log::info!("Used {}", item_name);

            // Apply hp/end/mana changes
            {
                (
                    gs.items[item_idx].hp[0],
                    gs.items[item_idx].end[0],
                    gs.items[item_idx].mana[0],
                )
            };
            {
                let hp = gs.items[item_idx].hp[0];
                let end = gs.items[item_idx].end[0];
                let mana = gs.items[item_idx].mana[0];

                gs.characters[cn].a_hp += hp as i32 * 1000;
                if gs.characters[cn].a_hp < 0 {
                    gs.characters[cn].a_hp = 0;
                }
                gs.characters[cn].a_end += end as i32 * 1000;
                if gs.characters[cn].a_end < 0 {
                    gs.characters[cn].a_end = 0;
                }
                gs.characters[cn].a_mana += mana as i32 * 1000;
                if gs.characters[cn].a_mana < 0 {
                    gs.characters[cn].a_mana = 0;
                }
            };

            // If item grants a spell-like effect, apply it
            let duration = gs.items[item_idx].duration;
            if duration != 0 {
                driver::spell_from_item(gs, cn, item_idx);
            }

            // Remove item from character
            God::take_from_char(gs, item_idx, cn);
            gs.items[item_idx].used = USE_EMPTY;

            // If character died as a result, announce and handle death
            let a_hp = gs.characters[cn].a_hp;
            if a_hp < 500 {
                let (x, y) = { (gs.characters[cn].x as i32, gs.characters[cn].y as i32) };
                gs.do_area_log(
                    cn,
                    0,
                    x,
                    y,
                    core::types::FontColor::Yellow,
                    &format!(
                        "{} was killed by {}.\n",
                        gs.characters[cn].get_name().to_string(),
                        c_string_to_str(&gs.items[item_idx].reference).to_string()
                    ),
                );
                gs.do_character_log(
                    cn,
                    core::types::FontColor::Yellow,
                    &format!(
                        "You were killed by {}.\n",
                        c_string_to_str(&gs.items[item_idx].reference).to_string()
                    ),
                );
                gs.do_character_killed(cn, 0, true);
            }

            gs.do_update_char(cn);
        }
    }
}

pub fn use_soulstone(gs: &mut GameState, cn: usize, item_idx: usize) -> i32 {
    if !core::types::Character::is_sane_character(cn) {
        return 0;
    }
    if !core::types::Item::is_sane_item(item_idx) {
        return 0;
    }

    let citem = gs.characters[cn].citem;
    if citem == 0 {
        gs.do_character_log(
            cn,
            core::types::FontColor::Blue,
            "Try using something with the soulstone. That is, click on the stone with an item under your cursor.",
        );
        return 0;
    }

    let in2 = citem as usize;
    if !core::types::Item::is_sane_item(in2) {
        return 0;
    }

    // Check if the item is another soulstone (driver 68)
    let in2_driver = gs.items[in2].driver;
    if in2_driver == 68 {
        // Absorb the second soulstone into the first
        {
            let exp_gain = helpers::random_mod(gs.items[in2].data[1].saturating_add(1));
            gs.items[item_idx].data[1] += exp_gain;
            let rank = core::ranks::points2rank(gs.items[item_idx].data[1]);
            gs.items[item_idx].data[0] = rank;

            // Update description - read data value first to avoid packed field reference
            let data1_value = gs.items[item_idx].data[1];
            let description = format!("Level {} soulstone, holding {} exp.", rank, data1_value);
            gs.items[item_idx].description.copy_from_slice(&[0u8; 120]);
            let bytes = description.as_bytes();
            let len = bytes.len().min(119);
            gs.items[item_idx].description[..len].copy_from_slice(&bytes[..len]);

            if rank > 20 {
                gs.do_character_log(
                    cn,
                    core::types::FontColor::Blue,
                    "That's as high as they go.",
                );
            }
        };

        soul_destroy(gs, cn, in2);
        return 1;
    }

    let in2_temp = gs.items[in2].temp;

    // Handle different item types based on temp value
    match in2_temp {
        18 => {
            // Red flower -> healing potion
            soul_transform(gs, cn, item_idx, in2, 101);
            {
                gs.items[item_idx].hp[0] += 10;
            };
            1
        }
        46 => {
            // Purple flower -> mana potion
            soul_transform(gs, cn, item_idx, in2, 102);
            {
                gs.items[item_idx].mana[0] += 10;
            };
            1
        }
        91 => {
            // Torch -> repair
            soul_repair(gs, cn, item_idx, in2);
            {
                gs.items[item_idx].max_age[1] *= 4;
            };
            1
        }
        100 => {
            // Flask -> mana potion
            soul_transform(gs, cn, item_idx, in2, 102);
            1
        }
        101 => {
            // Healing potion
            soul_destroy(gs, cn, item_idx);
            {
                gs.items[in2].hp[0] += 10;
            };
            1
        }
        102 => {
            // Mana potion
            soul_destroy(gs, cn, item_idx);
            {
                gs.items[in2].mana[0] += 10;
            };
            1
        }
        // Equipment items that can be enhanced
        27..=44 | 51..=80 | 94..=99 | 116 | 125 | 158 | 501..=503 | 523..=524 | 813 | 981..=986 => {
            soul_trans_equipment(gs, cn, item_idx, in2);
            1
        }
        _ => {
            gs.do_character_log(cn, core::types::FontColor::Blue, "Nothing happened.\n");
            0
        }
    }
}

/// Transform soulstone and item into a new item
fn soul_transform(
    gs: &mut GameState,
    cn: usize,
    soulstone_idx: usize,
    item_idx: usize,
    new_temp: usize,
) -> usize {
    use crate::god::God;

    God::take_from_char(gs, soulstone_idx, cn);
    God::take_from_char(gs, item_idx, cn);

    {
        gs.items[soulstone_idx].used = core::constants::USE_EMPTY;
        gs.items[item_idx].used = core::constants::USE_EMPTY;
    };

    let new_item = God::create_item(gs, new_temp);
    if let Some(new_item_idx) = new_item {
        God::give_character_item(gs, cn, new_item_idx);
        new_item_idx
    } else {
        0
    }
}

/// Repair an item using soulstone
fn soul_repair(gs: &mut GameState, cn: usize, soulstone_idx: usize, item_idx: usize) -> usize {
    use crate::god::God;

    God::take_from_char(gs, soulstone_idx, cn);

    {
        gs.items[soulstone_idx].used = core::constants::USE_EMPTY;
    };

    let item_temp = gs.items[item_idx].temp as usize;

    {
        {
            gs.items[item_idx] = gs.item_templates[item_temp];
            gs.items[item_idx].carried = cn as u16;
            gs.items[item_idx].flags |= core::constants::ItemFlags::IF_UPDATE.bits();
            gs.items[item_idx].temp = 0;
        };
    };

    item_idx
}

/// Destroy an item and remove it from character
fn soul_destroy(gs: &mut GameState, cn: usize, item_idx: usize) {
    use crate::god::God;

    God::take_from_char(gs, item_idx, cn);
    {
        gs.items[item_idx].used = core::constants::USE_EMPTY;
    };
}

/// Transfer soulstone power to equipment
fn soul_trans_equipment(gs: &mut GameState, cn: usize, soulstone_idx: usize, item_idx: usize) {
    let mut rank = gs.items[soulstone_idx].data[0];

    let is_weapon =
        { (gs.items[soulstone_idx].flags & core::constants::ItemFlags::IF_WEAPON.bits()) != 0 };

    while rank > 0 {
        let stren = helpers::random_mod(rank.saturating_add(1));
        rank -= stren;

        let ran = if is_weapon {
            helpers::random_mod_usize(27)
        } else {
            helpers::random_mod_usize(26)
        };

        {
            let item = &mut gs.items[item_idx];

            match ran {
                0 => {
                    item.hp[2] = item.hp[2].saturating_add((stren * 25) as i16);
                    item.hp[0] = item.hp[0].saturating_add((stren * 5) as i16);
                }
                1 => {
                    item.mana[2] = item.mana[2].saturating_add((stren * 25) as i16);
                    item.mana[0] = item.mana[0].saturating_add((stren * 5) as i16);
                }
                2..=6 => {
                    let attr_idx = ran - 2;
                    let current = item.attrib[attr_idx][2] as u32;
                    item.attrib[attr_idx][2] = std::cmp::min(120, current + (stren * 3)) as i8;
                    item.attrib[attr_idx][0] =
                        item.attrib[attr_idx][0].saturating_add((stren / 2) as i8);
                }
                7 => {
                    let current = item.skill[SK_DAGGER][2] as u32;
                    item.skill[SK_DAGGER][2] = std::cmp::min(120, current + (stren * 5)) as i8;
                    item.skill[SK_DAGGER][0] = item.skill[SK_DAGGER][0].saturating_add(stren as i8);
                }
                8 => {
                    let current = item.skill[SK_SWORD][2] as u32;
                    item.skill[SK_SWORD][2] = std::cmp::min(120, current + (stren * 5)) as i8;
                    item.skill[SK_SWORD][0] = item.skill[SK_SWORD][0].saturating_add(stren as i8);
                }
                9 => {
                    let current = item.skill[SK_TWOHAND][2] as u32;
                    item.skill[SK_TWOHAND][2] = std::cmp::min(120, current + (stren * 5)) as i8;
                    item.skill[SK_TWOHAND][0] =
                        item.skill[SK_TWOHAND][0].saturating_add(stren as i8);
                }
                10 => {
                    let current = item.skill[SK_STEALTH][2] as u32;
                    item.skill[SK_STEALTH][2] = std::cmp::min(120, current + (stren * 5)) as i8;
                    item.skill[SK_STEALTH][0] =
                        item.skill[SK_STEALTH][0].saturating_add(stren as i8);
                }
                11 => {
                    let current = item.skill[SK_MSHIELD][2] as u32;
                    item.skill[SK_MSHIELD][2] = std::cmp::min(120, current + (stren * 5)) as i8;
                    item.skill[SK_MSHIELD][0] =
                        item.skill[SK_MSHIELD][0].saturating_add(stren as i8);
                }
                12 => {
                    let current = item.skill[SK_PROTECT][2] as u32;
                    item.skill[SK_PROTECT][2] = std::cmp::min(120, current + (stren * 5)) as i8;
                    item.skill[SK_PROTECT][0] =
                        item.skill[SK_PROTECT][0].saturating_add(stren as i8);
                }
                13 => {
                    let current = item.skill[SK_ENHANCE][2] as u32;
                    item.skill[SK_ENHANCE][2] = std::cmp::min(120, current + (stren * 5)) as i8;
                    item.skill[SK_ENHANCE][0] =
                        item.skill[SK_ENHANCE][0].saturating_add(stren as i8);
                }
                14 => {
                    let current = item.skill[SK_STUN][2] as u32;
                    item.skill[SK_STUN][2] = std::cmp::min(120, current + (stren * 5)) as i8;
                    item.skill[SK_STUN][0] = item.skill[SK_STUN][0].saturating_add(stren as i8);
                }
                15 => {
                    let current = item.skill[SK_CURSE][2] as u32;
                    item.skill[SK_CURSE][2] = std::cmp::min(120, current + (stren * 5)) as i8;
                    item.skill[SK_CURSE][0] = item.skill[SK_CURSE][0].saturating_add(stren as i8);
                }
                16 => {
                    let current = item.skill[SK_BLESS][2] as u32;
                    item.skill[SK_BLESS][2] = std::cmp::min(120, current + (stren * 5)) as i8;
                    item.skill[SK_BLESS][0] = item.skill[SK_BLESS][0].saturating_add(stren as i8);
                }
                17 => {
                    let current = item.skill[SK_RESIST][2] as u32;
                    item.skill[SK_RESIST][2] = std::cmp::min(120, current + (stren * 5)) as i8;
                    item.skill[SK_RESIST][0] = item.skill[SK_RESIST][0].saturating_add(stren as i8);
                }
                18 => {
                    let current = item.skill[SK_BLAST][2] as u32;
                    item.skill[SK_BLAST][2] = std::cmp::min(120, current + (stren * 5)) as i8;
                    item.skill[SK_BLAST][0] = item.skill[SK_BLAST][0].saturating_add(stren as i8);
                }
                19 => {
                    let current = item.skill[SK_HEAL][2] as u32;
                    item.skill[SK_HEAL][2] = std::cmp::min(120, current + (stren * 5)) as i8;
                    item.skill[SK_HEAL][0] = item.skill[SK_HEAL][0].saturating_add(stren as i8);
                }
                20 => {
                    let current = item.skill[SK_GHOST][2] as u32;
                    item.skill[SK_GHOST][2] = std::cmp::min(120, current + (stren * 5)) as i8;
                    item.skill[SK_GHOST][0] = item.skill[SK_GHOST][0].saturating_add(stren as i8);
                }
                21 => {
                    let current = item.skill[SK_IMMUN][2] as u32;
                    item.skill[SK_IMMUN][2] = std::cmp::min(120, current + (stren * 5)) as i8;
                    item.skill[SK_IMMUN][0] = item.skill[SK_IMMUN][0].saturating_add(stren as i8);
                }
                22 => {
                    let current = item.skill[SK_SURROUND][2] as u32;
                    item.skill[SK_SURROUND][2] = std::cmp::min(120, current + (stren * 5)) as i8;
                    item.skill[SK_SURROUND][0] =
                        item.skill[SK_SURROUND][0].saturating_add(stren as i8);
                }
                23 => {
                    let current = item.skill[SK_CONCEN][2] as u32;
                    item.skill[SK_CONCEN][2] = std::cmp::min(120, current + (stren * 5)) as i8;
                    item.skill[SK_CONCEN][0] = item.skill[SK_CONCEN][0].saturating_add(stren as i8);
                }
                24 => {
                    let current = item.skill[SK_WARCRY][2] as u32;
                    item.skill[SK_WARCRY][2] = std::cmp::min(120, current + (stren * 5)) as i8;
                    item.skill[SK_WARCRY][0] = item.skill[SK_WARCRY][0].saturating_add(stren as i8);
                }
                25 => {
                    item.armor[0] = item.armor[0].saturating_add((stren / 2) as i8);
                }
                26 => {
                    item.weapon[0] = item.weapon[0].saturating_add((stren / 2) as i8);
                }
                _ => {
                    log::error!("should never happen in soul_trans_equipment(gs, )");
                }
            }
        };
    }

    // Finalize the enhancement
    {
        let soulstone_rank = gs.items[soulstone_idx].data[0];
        gs.items[item_idx].temp = 0;
        gs.items[item_idx].flags |= core::constants::ItemFlags::IF_UPDATE.bits()
            | core::constants::ItemFlags::IF_IDENTIFIED.bits()
            | core::constants::ItemFlags::IF_NOREPAIR.bits()
            | core::constants::ItemFlags::IF_SOULSTONE.bits();

        gs.items[item_idx].min_rank =
            std::cmp::max(soulstone_rank as i8, gs.items[item_idx].min_rank);

        if gs.items[item_idx].max_damage == 0 {
            gs.items[item_idx].max_damage = 60000;
        }

        // Get item name before destruction
        let item_name = gs.items[item_idx].get_name().to_string();

        // Update description
        let description = format!(
            "A {} enhanced by a rank {} soulstone.",
            item_name, soulstone_rank
        );
        gs.items[item_idx].description.fill(0);
        let bytes = description.as_bytes();
        let len = bytes.len().min(199);
        gs.items[item_idx].description[..len].copy_from_slice(&bytes[..len]);
    };

    soul_destroy(gs, cn, soulstone_idx);
}

pub fn item_age(gs: &mut GameState, item_idx: usize) -> i32 {
    let (current_age_act, max_age_act, current_damage, max_damage) = {
        let act = if gs.items[item_idx].active != 0 { 1 } else { 0 };
        (
            gs.items[item_idx].current_age[act],
            gs.items[item_idx].max_age[act],
            gs.items[item_idx].current_damage,
            gs.items[item_idx].max_damage,
        )
    };

    if (max_age_act != 0 && current_age_act > max_age_act)
        || (max_damage != 0 && current_damage > max_damage)
    {
        {
            gs.items[item_idx].flags |= core::constants::ItemFlags::IF_UPDATE.bits();
            gs.items[item_idx].current_damage = 0;
            gs.items[item_idx].current_age[0] = 0;
            gs.items[item_idx].current_age[1] = 0;
            gs.items[item_idx].damage_state += 1;
            gs.items[item_idx].value /= 2;

            if gs.items[item_idx].damage_state > 1 {
                let st = std::cmp::max(0, 4 - gs.items[item_idx].damage_state as i32);

                if gs.items[item_idx].armor[0] > st as i8 {
                    gs.items[item_idx].armor[0] -= 1;
                }
                if gs.items[item_idx].armor[1] > st as i8 {
                    gs.items[item_idx].armor[1] -= 1;
                }

                if gs.items[item_idx].weapon[0] > st as i8 * 2 {
                    gs.items[item_idx].weapon[0] -= 1;
                    if gs.items[item_idx].weapon[0] > 0 {
                        gs.items[item_idx].weapon[0] -= 1;
                    }
                }
                if gs.items[item_idx].weapon[1] > st as i8 * 2 {
                    gs.items[item_idx].weapon[1] -= 1;
                    if gs.items[item_idx].weapon[1] > 0 {
                        gs.items[item_idx].weapon[1] -= 1;
                    }
                }
            }

            if gs.items[item_idx].max_age[0] != 0 {
                gs.items[item_idx].sprite[0] += 1;
            }
            if gs.items[item_idx].max_age[1] != 0 {
                gs.items[item_idx].sprite[1] += 1;
            }
        };

        return 1;
    }

    // Expire no-age items after 30 minutes (lag scrolls after 2 minutes)
    if max_age_act == 0 {
        let is_lag_scroll = gs.items[item_idx].temp == 500;
        let expire_time = if is_lag_scroll {
            TICKS * 60 * 2
        } else {
            TICKS * 60 * 30
        };

        if current_age_act > expire_time as u32 {
            {
                gs.items[item_idx].damage_state = 5;
            };
            return 1;
        }
    }

    0
}

pub fn item_damage_worn(gs: &mut GameState, cn: usize, n: usize, damage: i32) {
    use core::constants::USE_EMPTY;

    let worn_idx = gs.characters[cn].worn[n] as usize;
    if worn_idx == 0 {
        return;
    }

    let has_max_damage = gs.items[worn_idx].max_damage != 0;
    if !has_max_damage {
        return;
    }

    {
        gs.items[worn_idx].current_damage += damage as u32;
    };

    if item_age(gs, worn_idx) != 0 {
        let (damage_state, reference) = {
            (
                gs.items[worn_idx].damage_state,
                c_string_to_str(&gs.items[worn_idx].reference).to_string(),
            )
        };

        match damage_state {
            1 => {
                gs.do_character_log(
                    cn,
                    core::types::FontColor::Yellow,
                    &format!("The {} you are using is showing signs of use.\n", reference),
                );
            }
            2 => {
                gs.do_character_log(
                    cn,
                    core::types::FontColor::Yellow,
                    &format!("The {} you are using was slightly damaged.\n", reference),
                );
            }
            3 => {
                gs.do_character_log(
                    cn,
                    core::types::FontColor::Yellow,
                    &format!("The {} you are using was damaged.\n", reference),
                );
            }
            4 => {
                gs.do_character_log(
                    cn,
                    core::types::FontColor::Red,
                    &format!("The {} you are using was badly damaged.\n", reference),
                );
            }
            5 => {
                {
                    gs.characters[cn].worn[n] = 0;
                };
                {
                    gs.items[worn_idx].used = USE_EMPTY;
                };
                gs.do_character_log(
                    cn,
                    core::types::FontColor::Red,
                    &format!("The {} you were using was destroyed.\n", reference),
                );
            }
            _ => {}
        }
        gs.do_update_char(cn);
    }
}

pub fn item_damage_citem(gs: &mut GameState, cn: usize, damage: i32) {
    use core::constants::USE_EMPTY;

    let citem = gs.characters[cn].citem;
    if citem == 0 || (citem & 0x80000000) != 0 {
        return;
    }

    let citem_idx = citem as usize;
    let has_max_damage = gs.items[citem_idx].max_damage != 0;
    if !has_max_damage {
        return;
    }

    {
        gs.items[citem_idx].current_damage += damage as u32;
    };

    if item_age(gs, citem_idx) != 0 {
        let (damage_state, reference) = {
            (
                gs.items[citem_idx].damage_state,
                c_string_to_str(&gs.items[citem_idx].reference).to_string(),
            )
        };

        match damage_state {
            1 => {
                gs.do_character_log(
                    cn,
                    core::types::FontColor::Yellow,
                    &format!("The {} you are using is showing signs of use.\n", reference),
                );
            }
            2 => {
                gs.do_character_log(
                    cn,
                    core::types::FontColor::Yellow,
                    &format!("The {} you are using was slightly damaged.\n", reference),
                );
            }
            3 => {
                gs.do_character_log(
                    cn,
                    core::types::FontColor::Yellow,
                    &format!("The {} you are using was damaged.\n", reference),
                );
            }
            4 => {
                gs.do_character_log(
                    cn,
                    core::types::FontColor::Red,
                    &format!("The {} you are using was badly damaged.\n", reference),
                );
            }
            5 => {
                {
                    gs.characters[cn].citem = 0;
                };
                {
                    gs.items[citem_idx].used = USE_EMPTY;
                };
                gs.do_character_log(
                    cn,
                    core::types::FontColor::Red,
                    &format!("The {} you were using was destroyed.\n", reference),
                );
            }
            _ => {}
        }
    }
}

pub fn item_damage_armor(gs: &mut GameState, cn: usize, damage: i32) {
    let dam = damage / 4 + 1;

    const WN_RHAND: usize = 8;
    const WN_LHAND: usize = 17;

    for n in 0..20 {
        if n != WN_RHAND && n != WN_LHAND {
            if helpers::random_mod_i32(3) != 0 {
                item_damage_worn(gs, cn, n, dam);
            }
        }
    }
}

pub fn item_damage_weapon(gs: &mut GameState, cn: usize, damage: i32) {
    const WN_RHAND: usize = 8;
    let dam = damage / 4 + 1;
    item_damage_worn(gs, cn, WN_RHAND, dam);
}

pub fn lightage(gs: &mut GameState, item_idx: usize, multi: i32) {
    // Read basic item info
    let (carried, it_x, it_y, active) = {
        (
            gs.items[item_idx].carried,
            gs.items[item_idx].x as i32,
            gs.items[item_idx].y as i32,
            gs.items[item_idx].active,
        )
    };

    // Determine map coordinates: if carried by a character, use that character's position
    let (mx, my) = if carried != 0 {
        let cn = carried as usize;
        {
            if cn >= core::constants::MAXCHARS || gs.characters[cn].used == USE_EMPTY {
                (it_x, it_y)
            } else {
                (gs.characters[cn].x as i32, gs.characters[cn].y as i32)
            }
        }
    } else {
        (it_x, it_y)
    };

    // Validate coordinates
    if mx < 0 || my < 0 || mx >= SERVER_MAPX || my >= SERVER_MAPY {
        return;
    }

    let m = (mx + my * SERVER_MAPX) as usize;

    // Read map light
    let mut light = gs.map[m].light as i32;
    if light < 1 {
        return;
    }
    if light > 250 {
        light = 250;
    }

    light *= multi;

    let act = if active != 0 { 1usize } else { 0usize };

    {
        gs.items[item_idx].current_age[act] =
            gs.items[item_idx].current_age[act].wrapping_add((light as u32) * 2);
    };
}

pub fn age_message(gs: &mut GameState, cn: usize, item_idx: usize, where_is: &str) {
    let (driver, damage_state, reference) = {
        (
            gs.items[item_idx].driver,
            gs.items[item_idx].damage_state,
            c_string_to_str(&gs.items[item_idx].reference).to_string(),
        )
    };

    let (msg, font) = if driver == 60 {
        // Ice egg or cloak
        match damage_state {
            1 => ("The {ref} {where} is beginning to melt.\n", FontColor::Red),
            2 => ("The {ref} {where} is melting fairly rapidly.\n", FontColor::Red),
            3 => (
                "The {ref} {where} is melting down as you look and dripping water everywhere.\n",
                FontColor::Red,
            ),
            4 => (
                "The {ref} {where} has melted down to a small icy lump and large puddles of water.\n",
                FontColor::Yellow,
            ),
            5 => (
                "The {ref} {where} has completely melted away, leaving you all wet.\n",
                FontColor::Yellow,
            ),
            _ => ("The {ref} {where} is changing.\n", FontColor::Red),
        }
    } else {
        // Anything else
        match damage_state {
            1 => (
                "The {ref} {where} is showing signs of age.\n",
                FontColor::Red,
            ),
            2 => ("The {ref} {where} is getting fairly old.\n", FontColor::Red),
            3 => ("The {ref} {where} is getting old.\n", FontColor::Red),
            4 => (
                "The {ref} {where} is getting very old and battered.\n",
                FontColor::Yellow,
            ),
            5 => (
                "The {ref} {where} was so old and battered that it finally vanished.\n",
                FontColor::Yellow,
            ),
            _ => ("The {ref} {where} is aging.\n", FontColor::Red),
        }
    };

    let formatted_msg = msg
        .replace("{ref}", &reference)
        .replace("{where}", where_is);

    gs.do_character_log(cn, font, &formatted_msg);
}

pub fn char_item_expire(gs: &mut GameState, cn: usize) {
    if (gs.characters[cn].flags & CharacterFlags::BuildMode.bits()) != 0 {
        return;
    }

    let mut must_update = false;

    let current_ice_cloak_clock = gs.ice_cloak_clock;
    gs.ice_cloak_clock = current_ice_cloak_clock + 1;

    // Age items in backpack (40 slots)
    for n in 0..40 {
        let item_idx = gs.characters[cn].item[n] as usize;
        if item_idx == 0 {
            continue;
        }

        let (active, has_alwaysexp1, has_alwaysexp2, driver, has_lightage) = {
            let act = if gs.items[item_idx].active != 0 { 1 } else { 0 };
            (
                act,
                (gs.items[item_idx].flags & core::constants::ItemFlags::IF_ALWAYSEXP1.bits()) != 0,
                (gs.items[item_idx].flags & core::constants::ItemFlags::IF_ALWAYSEXP2.bits()) != 0,
                gs.items[item_idx].driver,
                (gs.items[item_idx].flags & core::constants::ItemFlags::IF_LIGHTAGE.bits()) != 0,
            )
        };

        let should_age = (active == 0 && has_alwaysexp1) || (active == 1 && has_alwaysexp2);
        if !should_age {
            continue;
        }

        // Ice cloak ages more slowly when not worn or held
        if driver == 60 && gs.ice_cloak_clock.is_multiple_of(4) {
            continue;
        }

        {
            gs.items[item_idx].current_age[active] += 1;
        };

        if has_lightage {
            lightage(gs, item_idx, 1);
        }

        if item_age(gs, item_idx) != 0 {
            must_update = true;
            age_message(gs, cn, item_idx, "in your backpack");

            let damage_state = gs.items[item_idx].damage_state;
            if damage_state == 5 {
                {
                    gs.characters[cn].item[n] = 0;
                };
                {
                    gs.items[item_idx].used = USE_EMPTY;
                };
            }
        }
    }

    // Age items in worn slots (20 slots)
    for n in 0..20 {
        let item_idx = gs.characters[cn].worn[n] as usize;
        if item_idx == 0 {
            continue;
        }

        let (active, has_alwaysexp1, has_alwaysexp2, has_lightage) = {
            let act = if gs.items[item_idx].active != 0 { 1 } else { 0 };
            (
                act,
                (gs.items[item_idx].flags & core::constants::ItemFlags::IF_ALWAYSEXP1.bits()) != 0,
                (gs.items[item_idx].flags & core::constants::ItemFlags::IF_ALWAYSEXP2.bits()) != 0,
                (gs.items[item_idx].flags & core::constants::ItemFlags::IF_LIGHTAGE.bits()) != 0,
            )
        };

        let should_age = (active == 0 && has_alwaysexp1) || (active == 1 && has_alwaysexp2);
        if !should_age {
            continue;
        }

        {
            gs.items[item_idx].current_age[active] += 1;
        };

        if has_lightage {
            lightage(gs, item_idx, 1);
        }

        if item_age(gs, item_idx) != 0 {
            must_update = true;
            let damage_state = gs.items[item_idx].damage_state;

            if damage_state == 5 {
                age_message(gs, cn, item_idx, "you were using");
                {
                    gs.characters[cn].worn[n] = 0;
                };
                {
                    gs.items[item_idx].used = USE_EMPTY;
                };
            } else {
                age_message(gs, cn, item_idx, "you are using");
            }
        }
    }

    // Age item under mouse cursor (citem)
    let citem = gs.characters[cn].citem;
    if citem != 0 && (citem & 0x80000000) == 0 {
        let item_idx = citem as usize;
        let (active, has_alwaysexp1, has_alwaysexp2, has_lightage) = {
            let act = if gs.items[item_idx].active != 0 { 1 } else { 0 };
            (
                act,
                (gs.items[item_idx].flags & ItemFlags::IF_ALWAYSEXP1.bits()) != 0,
                (gs.items[item_idx].flags & ItemFlags::IF_ALWAYSEXP2.bits()) != 0,
                (gs.items[item_idx].flags & ItemFlags::IF_LIGHTAGE.bits()) != 0,
            )
        };

        let should_age = (active == 0 && has_alwaysexp1) || (active == 1 && has_alwaysexp2);
        if should_age {
            {
                gs.items[item_idx].current_age[active] += 1;
            };

            if has_lightage {
                lightage(gs, item_idx, 1);
            }

            if item_age(gs, item_idx) != 0 {
                must_update = true;
                let damage_state = gs.items[item_idx].damage_state;

                if damage_state == 5 {
                    age_message(gs, cn, item_idx, "you were using");
                    {
                        gs.characters[cn].citem = 0;
                    };
                    {
                        gs.items[item_idx].used = USE_EMPTY;
                    };
                } else {
                    age_message(gs, cn, item_idx, "you are using");
                }
            }
        }
    }

    if must_update {
        gs.do_update_char(cn);
    }
}

pub fn may_deactivate(gs: &mut GameState, item_idx: usize) -> bool {
    // Special check for driver 1 (create_item with mines)
    let driver = gs.items[item_idx].driver;
    if driver != 1 {
        return true;
    }

    // Check data[1..9] for mine states; each stores a map tile index.
    let max_tiles = (SERVER_MAPX * SERVER_MAPY) as usize;
    for n in 1..10 {
        let m = gs.items[item_idx].data[n] as usize;
        if m == 0 {
            // empty slot => can deactivate
            return true;
        }

        // Validate tile index bounds
        if m >= max_tiles {
            return false;
        }

        // Check if there is an item on the stored map tile and that
        // the item has driver 26 (the mine driver). If not, cannot deactivate.
        let it_idx = gs.map[m].it;
        if it_idx == 0 {
            return false;
        }

        let it_driver = gs.items[it_idx as usize].driver;
        if it_driver != 26 {
            return false;
        }
    }

    true
}

pub fn pentagram(gs: &mut GameState, item_idx: usize) {
    let active = gs.items[item_idx].active;
    if active != 0 {
        return;
    }

    if helpers::random_mod_i32(18) != 0 {
        return;
    }

    // Check data[1-3] for spawned enemies
    for n in 1..4 {
        let stored_cn = gs.items[item_idx].data[n];

        // Check if slot is empty or enemy is dead
        let should_spawn = if stored_cn == 0 {
            true
        } else {
            {
                let cn = stored_cn as usize;
                let dead_or_mismatch = gs.characters[cn].data[0] != item_idx as i32
                    || gs.characters[cn].used == USE_EMPTY
                    || (gs.characters[cn].flags & CharacterFlags::Body.bits()) != 0;
                dead_or_mismatch
            }
        };

        if should_spawn {
            // Use the dedicated spawn helper which encapsulates template selection
            let new_cn = spawn_penta_enemy(gs, item_idx);
            if new_cn != 0 {
                {
                    gs.items[item_idx].data[n] = new_cn as u32;
                };
            }
            break;
        }
    }
}

pub fn spiderweb(gs: &mut GameState, item_idx: usize) {
    let active = gs.items[item_idx].active;
    if active != 0 {
        return;
    }

    if helpers::random_mod_i32(60) != 0 {
        return;
    }

    // Check data[1-3] for spawned spiders
    for n in 1..4 {
        let stored_cn = gs.items[item_idx].data[n];

        // Check if slot is empty or spider is dead
        let should_spawn = if stored_cn == 0 {
            true
        } else {
            {
                let cn = stored_cn as usize;
                let dead_or_mismatch = gs.characters[cn].data[0] != item_idx as i32
                    || gs.characters[cn].used == USE_EMPTY
                    || (gs.characters[cn].flags & CharacterFlags::Body.bits()) != 0;
                dead_or_mismatch
            }
        };

        if should_spawn {
            // Create spider (template 390-392)
            let spider_template = 390usize + helpers::random_mod_usize(3);
            let cn = match populate::pop_create_char(gs, spider_template, false) {
                Some(cn) => cn,
                None => continue,
            };

            let (x, y) = { (gs.items[item_idx].x as usize, gs.items[item_idx].y as usize) };

            {
                // Ensure respawn flag is cleared for this spawned instance
                gs.characters[cn].flags &= !CharacterFlags::Respawn.bits();
                gs.characters[cn].data[0] = item_idx as i32;
                gs.characters[cn].data[29] = (x + y * core::constants::SERVER_MAPX as usize) as i32;
                gs.characters[cn].data[60] = TICKS * 60 * 2;
                gs.characters[cn].data[73] = 8;
                gs.characters[cn].dir = DX_RIGHT;
            };

            if !God::drop_char_fuzzy(gs, cn, x, y) {
                God::destroy_items(gs, cn);
                {
                    gs.characters[cn].used = USE_EMPTY;
                };
            } else {
                {
                    gs.items[item_idx].data[n] = cn as u32;
                };
            }
            break;
        }
    }
}

pub fn greenlingball(gs: &mut GameState, item_idx: usize) {
    let active = gs.items[item_idx].active;
    if active != 0 {
        return;
    }

    if helpers::random_mod_i32(20) != 0 {
        return;
    }

    // Check data[1-3] for spawned greenlings
    for n in 1..4 {
        let stored_cn = gs.items[item_idx].data[n];

        // Check if slot is empty or greenling is dead
        let should_spawn = if stored_cn == 0 {
            true
        } else {
            {
                let cn = stored_cn as usize;
                let dead_or_mismatch = gs.characters[cn].data[0] != item_idx as i32
                    || gs.characters[cn].used == USE_EMPTY
                    || (gs.characters[cn].flags & CharacterFlags::Body.bits()) != 0;
                dead_or_mismatch
            }
        };

        if should_spawn {
            // Create greenling (template 553 + data[0])
            let greenling_type = gs.items[item_idx].data[0];
            let cn = match populate::pop_create_char(gs, 553 + greenling_type as usize, false) {
                Some(cn) => cn,
                None => continue,
            };

            let (x, y) = { (gs.items[item_idx].x as usize, gs.items[item_idx].y as usize) };

            {
                // Ensure respawn flag is cleared for this spawned instance
                gs.characters[cn].flags &= !CharacterFlags::Respawn.bits();
                gs.characters[cn].data[0] = item_idx as i32;
                gs.characters[cn].data[29] = (x + y * core::constants::SERVER_MAPX as usize) as i32;
                gs.characters[cn].data[60] = TICKS * 60 * 2;
                gs.characters[cn].data[73] = 8;
                gs.characters[cn].dir = DX_RIGHT;
            };

            if !God::drop_char_fuzzy(gs, cn, x, y) {
                God::destroy_items(gs, cn);
                {
                    gs.characters[cn].used = USE_EMPTY;
                };
            } else {
                {
                    gs.items[item_idx].data[n] = cn as u32;
                };
            }
            break;
        }
    }
}

pub fn expire_blood_penta(gs: &mut GameState, item_idx: usize) {
    {
        let item = &mut gs.items[item_idx];
        if item.data[0] != 0 {
            item.data[0] += 1;
            if item.data[0] > 7 {
                item.data[0] = 0;
            }
            item.sprite[0] = item.data[1] as i16 + item.data[0] as i16;
        }
    };
}

pub fn expire_driver(gs: &mut GameState, item_idx: usize) {
    let driver = gs.items[item_idx].driver;

    match driver {
        49 => expire_blood_penta(gs, item_idx),
        _ => {
            {
                log::error!(
                    "unknown expire driver {} for item {} ({})",
                    gs.items[item_idx].driver,
                    gs.items[item_idx].get_name(),
                    item_idx
                );
            };
        }
    }
}

pub fn item_tick_expire(gs: &mut GameState) {
    const EXP_TIME: i32 = SERVER_MAPY / 4;

    // Conform to the original C++ semantics:
    // - process the current row `y`
    // - then increment and wrap `y` afterwards
    let mut y = gs.item_tick_expire_counter;
    if y >= SERVER_MAPY as u32 {
        y = 0;
    }

    let y_usize = y as usize;
    for x in 0..SERVER_MAPX as usize {
        let m = x + y_usize * SERVER_MAPX as usize;

        // Process items on this tile
        let in_idx = gs.map[m].it;
        if in_idx != 0 {
            let in_idx = in_idx as usize;

            // Snapshot core fields, but keep `active` as a local variable we update
            // to mirror the C++ behavior (reactivation affects same-tick expiration).
            let (mut flags, driver, mut active, duration, light_diff) = {
                let item = &gs.items[in_idx];
                (
                    item.flags,
                    item.driver,
                    item.active,
                    item.duration,
                    item.light[1] as i32 - item.light[0] as i32,
                )
            };

            let (ch_present, to_ch_present) = (gs.map[m].ch != 0, gs.map[m].to_ch != 0);

            if (flags & ItemFlags::IF_REACTIVATE.bits()) != 0 && active == 0 {
                if !ch_present && !to_ch_present {
                    {
                        gs.items[in_idx].active = duration;
                    };
                    active = duration;
                    if light_diff != 0 {
                        gs.do_add_light(x as i32, y as i32, light_diff);
                    }
                }
            }

            // Handle active expiration
            if active != 0 && active != 0xffffffff {
                if active <= EXP_TIME as u32 {
                    if may_deactivate(gs, in_idx) && !ch_present && !to_ch_present {
                        use_driver(gs, 0, in_idx, false);
                        {
                            gs.items[in_idx].active = 0;
                        };
                        active = 0;
                        if light_diff != 0 {
                            gs.do_add_light(x as i32, y as i32, -light_diff);
                        }
                    }
                } else {
                    {
                        gs.items[in_idx].active -= EXP_TIME as u32;
                    };
                    active -= EXP_TIME as u32;
                }
            }

            // Legacy drivers
            if driver == 33 {
                pentagram(gs, in_idx);
            }
            if driver == 43 {
                spiderweb(gs, in_idx);
            }
            if driver == 56 {
                greenlingball(gs, in_idx);
            }

            // IF_EXPIREPROC
            if (flags & ItemFlags::IF_EXPIREPROC.bits()) != 0 {
                expire_driver(gs, in_idx);
            }

            // Refresh flags in case any of the above mutated them.
            flags = gs.items[in_idx].flags;

            // Check if item should expire
            let map_flags = gs.map[m].flags;
            if ((flags & ItemFlags::IF_TAKE.bits()) == 0 && driver != 7)
                || ((map_flags & MF_NOEXPIRE as u64) != 0 && driver != 7)
                || driver == 37
                || (flags & ItemFlags::IF_NOEXPIRE.bits()) != 0
            {
                // Skip expiration
            } else {
                let act = if active != 0 { 1 } else { 0 };

                {
                    gs.items[in_idx].current_age[act] += EXP_TIME as u32;
                };

                if (flags & ItemFlags::IF_LIGHTAGE.bits()) != 0 {
                    lightage(gs, in_idx, EXP_TIME);
                }

                if item_age(gs, in_idx) != 0 {
                    let damage_state = gs.items[in_idx].damage_state;
                    if damage_state == 5 {
                        let light = gs.items[in_idx].light[act];
                        if light != 0 {
                            gs.do_add_light(x as i32, y as i32, -(light as i32));
                        }

                        {
                            gs.map[m].it = 0;
                        };
                        {
                            gs.items[in_idx].used = USE_EMPTY;
                        };
                        {
                            gs.globals.expire_cnt += 1;
                        };

                        // Handle tomb (driver == 7)
                        if driver == 7 {
                            let co = gs.items[in_idx].data[0] as usize;
                            // Validate character index
                            if co != 0 && co < core::constants::MAXCHARS {
                                // Remember template and name for logging
                                let (temp, name, respawn_flag) = {
                                    (
                                        gs.characters[co].temp as usize,
                                        gs.characters[co].get_name().to_string(),
                                        (gs.characters[co].flags & CharacterFlags::Respawn.bits())
                                            != 0,
                                    )
                                };

                                // Remove the character and its items
                                God::destroy_items(gs, co);
                                {
                                    gs.characters[co].used = core::constants::USE_EMPTY;
                                };

                                if temp != 0 && respawn_flag {
                                    // Schedule a respawn effect (type 2 = RSPAWN)
                                    let dur = if temp == 189 || temp == 561 {
                                        TICKS * 60 * 20 + helpers::random_mod_i32(TICKS * 60 * 5)
                                    } else {
                                        (TICKS * 60) + helpers::random_mod_i32(TICKS * 60)
                                    };

                                    // Use the template's coordinates for the respawn location
                                    let (tx, ty) = {
                                        (
                                            gs.character_templates[temp].x as i32,
                                            gs.character_templates[temp].y as i32,
                                        )
                                    };

                                    EffectManager::fx_add_effect(2, dur, tx, ty, temp as i32);
                                    log::info!("respawn {} ({}): YES", co, name);
                                } else {
                                    log::info!("respawn {} ({}): NO", co, name);
                                }
                            }
                        }
                    }
                }
            }
        }

        // Checker: validate map references
        let cn = gs.map[m].ch;
        if cn != 0 {
            let cn = cn as usize;
            let (ch_x, ch_y, ch_used) = {
                (
                    gs.characters[cn].x,
                    gs.characters[cn].y,
                    gs.characters[cn].used,
                )
            };
            if ch_x != x as i16 || ch_y != y as i16 || ch_used != USE_ACTIVE {
                log::error!("map[{},{}].ch reset from {} to 0", x, y, cn);
                {
                    gs.map[m].ch = 0;
                };
                {
                    gs.globals.lost_cnt += 1;
                };
            }
        }

        let cn = gs.map[m].to_ch;
        if cn != 0 {
            let cn = cn as usize;
            let (tox, toy, ch_used) = {
                (
                    gs.characters[cn].tox,
                    gs.characters[cn].toy,
                    gs.characters[cn].used,
                )
            };
            if tox != x as i16 || toy != y as i16 || ch_used != USE_ACTIVE {
                log::error!("map[{},{}].to_ch reset from {} to 0", x, y, cn);
                {
                    gs.map[m].to_ch = 0;
                };
                {
                    gs.globals.lost_cnt += 1;
                };
            }
        }

        let in_idx = gs.map[m].it;
        if in_idx != 0 {
            let in_idx = in_idx as usize;
            let (it_x, it_y, it_used) = {
                (
                    gs.items[in_idx].x,
                    gs.items[in_idx].y,
                    gs.items[in_idx].used,
                )
            };
            if it_x != x as u16 || it_y != y as u16 || it_used != USE_ACTIVE {
                {
                    if in_idx < gs.items.len() {
                        let item = &gs.items[in_idx];
                        let temp = item.temp;
                        let carried = item.carried;
                        let used = item.used;
                        let item_x = item.x;
                        let item_y = item.y;
                        log::error!(
                            "map[{},{}].it invalid -> item {} (temp={}, name='{}', carried={}, used={}, pos=({},{})); clearing map reference",
                            x,
                            y,
                            in_idx,
                            temp,
                            item.get_name(),
                            carried,
                            used,
                            item_x,
                            item_y,
                        );
                    } else {
                        log::error!(
                            "map[{},{}].it invalid -> item index {} out of bounds; clearing map reference",
                            x,
                            y,
                            in_idx
                        );
                    }
                };
                log::error!("map[{},{}].it reset from {} to 0", x, y, in_idx);
                {
                    gs.map[m].it = 0;
                };
                {
                    gs.globals.lost_cnt += 1;
                };
            }
        }
    }

    // Advance to next row after processing the current row.
    y += 1;
    if y >= SERVER_MAPY as u32 {
        {
            gs.globals.expire_run += 1;
            gs.globals.lost_run += 1;
        };
        y = 0;
    }
    gs.item_tick_expire_counter = y;
}

pub fn item_tick_gc(gs: &mut GameState) {
    let (off, m) = {
        let current_off = gs.item_tick_gc_off as usize;
        let current_m = std::cmp::min(current_off + 256, MAXITEM);
        (current_off, current_m)
    };

    for n in off..m {
        let used = gs.items[n].used;
        if used == USE_EMPTY {
            continue;
        }

        let current_count = gs.item_tick_gc_count;
        gs.item_tick_gc_count = current_count + 1;

        // Hack: make reset seyan swords unusable
        let (driver, data0) = (gs.items[n].driver, gs.items[n].data[0]);
        if driver == 40 && data0 == 0 {
            // Reset to template 683
            {
                {
                    let (x, y, carried) = (gs.items[n].x, gs.items[n].y, gs.items[n].carried);
                    gs.items[n] = gs.item_templates[683];
                    gs.items[n].x = x;
                    gs.items[n].y = y;
                    gs.items[n].carried = carried;
                    gs.items[n].temp = 683;
                    gs.items[n].flags |= ItemFlags::IF_UPDATE.bits();
                };
            };

            let cn = gs.items[n].carried;
            if cn != 0 {
                gs.do_update_char(cn as usize);
                log::info!("reset sword from character {}", cn);
            }
        }

        let cn = gs.items[n].carried as usize;
        if cn != 0 {
            let is_sane = cn < core::constants::MAXCHARS;
            if is_sane {
                let ch_used = gs.characters[cn].used;
                if ch_used != 0 {
                    // Check if item is in character's inventory
                    let mut found = false;

                    // Check item slots
                    for z in 0..40 {
                        let item_id = gs.characters[cn].item[z];
                        if item_id == n as u32 {
                            found = true;
                            break;
                        }
                    }

                    // Check worn/spell slots
                    if !found {
                        for z in 0..20 {
                            let worn_id = gs.characters[cn].worn[z];
                            let spell_id = gs.characters[cn].spell[z];
                            if worn_id == n as u32 || spell_id == n as u32 {
                                found = true;
                                break;
                            }
                        }
                    }

                    // Check citem
                    if !found {
                        let citem = gs.characters[cn].citem;
                        if citem == n as u32 {
                            found = true;
                        }
                    }

                    // Check depot for players
                    if !found {
                        let is_player =
                            { (gs.characters[cn].flags & CharacterFlags::Player.bits()) != 0 };
                        if is_player {
                            for z in 0..62 {
                                let depot_id = { gs.characters[cn].depot[z] };
                                if depot_id == n as u32 {
                                    found = true;
                                    break;
                                }
                            }
                        }
                    }

                    if found {
                        continue;
                    }
                }
            }
        } else {
            // Check if item is on the map
            let (x, y) = (gs.items[n].x, gs.items[n].y);
            let in2 = {
                let idx = x as usize + y as usize * SERVER_MAPX as usize;
                gs.map[idx].it
            };
            if in2 == n as u32 {
                continue;
            }
        }

        // Item is garbage - remove it
        {
            gs.items[n].used = USE_EMPTY;
        };

        {
            gs.globals.gc_cnt += 1;
        };
    }

    // Update OFF and possibly reset
    let mut current_off = gs.item_tick_gc_off;
    current_off += 256;
    gs.item_tick_gc_off = current_off;

    if current_off >= MAXITEM as u32 {
        gs.item_tick_gc_off = 0;
        let count = gs.item_tick_gc_count;
        {
            gs.globals.item_cnt = count as i32;
            gs.globals.gc_run += 1;
        };
        gs.item_tick_gc_count = 0;
    }
}

pub fn item_tick(gs: &mut GameState) {
    item_tick_expire(gs);
    item_tick_expire(gs);
    item_tick_expire(gs);
    item_tick_expire(gs);
    item_tick_gc(gs);
}

pub fn trap1(gs: &mut GameState, cn: usize, item_idx: usize) {
    let n = gs.items[item_idx].data[1] as usize;
    if n != 0 {
        let in2 = gs.map[n].it as usize;
        if in2 != 0 {
            let (active, data0) = (gs.items[in2].active, gs.items[in2].data[0]);
            if active != 0 || data0 != 0 {
                gs.do_character_log(
                    cn,
                    core::types::FontColor::Red,
                    "You stepped on a trap, but nothing happened!",
                );
                return;
            }
        }
    }

    let slot = helpers::random_mod_usize(12);
    let in_worn = gs.characters[cn].worn[slot];

    if in_worn != 0 {
        let item_name = gs.items[in_worn as usize].get_name().to_string();
        gs.do_character_log(
            cn,
            core::types::FontColor::Red,
            &format!(
                "You triggered an acid attack. Your {} desintegrated.",
                item_name
            ),
        );
        log::info!(
            "Character {} stepped on Acid Trap, {} vanished",
            cn,
            item_name
        );

        {
            gs.items[in_worn as usize].used = USE_EMPTY;
        };
        {
            gs.characters[cn].worn[slot] = 0;
        };
        gs.do_update_char(cn);
    } else {
        gs.do_character_log(
            cn,
            core::types::FontColor::Red,
            "You triggered an acid attack, but it hit only your skin.",
        );
        log::info!("Character {} stepped on Acid Trap", cn);
        gs.do_hurt(0, cn, 350, 0);
    }
}

pub fn trap2(gs: &mut GameState, cn: usize, tmp: usize) {
    use crate::god::God;
    use crate::populate::pop_create_char;
    use core::constants::USE_EMPTY;

    let cc = match pop_create_char(gs, tmp, false) {
        Some(cc) => cc,
        None => return,
    };

    let (ch_x, ch_y) = { (gs.characters[cn].x as usize, gs.characters[cn].y as usize) };

    if !God::drop_char_fuzzy(gs, cc, ch_x, ch_y) {
        log::error!("trap2: drop failed");
        {
            gs.characters[cc].used = USE_EMPTY;
        };
        return;
    }

    {
        gs.characters[cc].attack_cn = cn as u16;
    };
    gs.do_update_char(cc);
}

pub fn start_trap(gs: &mut GameState, cn: usize, item_idx: usize) {
    let (duration, light0, light1, x, y) = {
        let item = &gs.items[item_idx];
        (item.duration, item.light[0], item.light[1], item.x, item.y)
    };

    if duration != 0 {
        {
            gs.items[item_idx].active = duration;
        };
        if light0 != light1 && x > 0 {
            gs.do_add_light(x as i32, y as i32, light1 as i32 - light0 as i32);
        }
    }

    let trap_type = gs.items[item_idx].data[0];

    match trap_type {
        0 => {
            log::info!("Character {} stepped on Arrow Trap", cn);
            gs.do_character_log(cn, core::types::FontColor::Red, "You feel a sudden pain!\n");
            gs.do_hurt(0, cn, 250, 0);
        }
        1 => {
            log::info!("Character {} stepped on Attack Trigger Trap", cn);
            gs.do_character_log(
                cn,
                core::types::FontColor::Yellow,
                "You hear a loud croaking noise!",
            );
            gs.do_area_notify(
                cn as i32,
                0,
                x as i32,
                y as i32,
                NT_HITME as i32,
                cn as i32,
                0,
                0,
                0,
            );
        }
        2 => trap1(gs, cn, item_idx),
        3 => trap2(gs, cn, 323),
        4 => trap2(gs, cn, 324),
        _ => {
            gs.do_character_log(
                cn,
                core::types::FontColor::Yellow,
                "Phew. Must be your lucky day today.",
            );
        }
    }
}

pub fn step_trap(gs: &mut GameState, cn: usize, item_idx: usize) -> i32 {
    let is_player = { (gs.characters[cn].flags & CharacterFlags::Player.bits()) != 0 };

    if is_player {
        start_trap(gs, cn, item_idx);
    } else {
        gs.do_character_log(
            cn,
            core::types::FontColor::Green,
            "You stepped on a trap. Fortunately, nothing happened.",
        );
    }

    0
}

pub fn step_trap_remove(gs: &mut GameState, _cn: usize, item_idx: usize) {
    let (active, light0, light1, x, y) = {
        let item = &gs.items[item_idx];
        (item.active, item.light[0], item.light[1], item.x, item.y)
    };

    if active != 0 {
        {
            gs.items[item_idx].active = 0;
        };
        if light0 != light1 && x > 0 {
            gs.do_add_light(x as i32, y as i32, light0 as i32 - light1 as i32);
        }
    }
}

pub fn step_portal1_lab13(gs: &mut GameState, cn: usize, _item_idx: usize) -> i32 {
    // Check kindred
    let kindred = gs.characters[cn].kindred as u32;
    if (kindred & KIN_HARAKIM) == 0
        && (kindred & KIN_TEMPLAR) == 0
        && (kindred & KIN_MERCENARY) == 0
        && (kindred & KIN_SORCERER) == 0
        && (kindred & KIN_WARRIOR) == 0
    {
        gs.do_character_log(
            cn,
            core::types::FontColor::Red,
            "This portal opens only for Harakim, Templars, Mercenaries, Warrior and Sorcerers.",
        );
        return -1;
    }

    // Check for items
    let mut has_items = false;

    let citem = gs.characters[cn].citem;
    if citem != 0 {
        has_items = true;
    }

    if !has_items {
        for n in 0..40 {
            let item_id = gs.characters[cn].item[n];
            if item_id != 0 {
                has_items = true;
                break;
            }
        }
    }

    if !has_items {
        for n in 0..20 {
            let worn_id = gs.characters[cn].worn[n];
            if worn_id != 0 {
                has_items = true;
                break;
            }
        }
    }

    if has_items {
        gs.do_character_log(
            cn,
            core::types::FontColor::Red,
            "You may not pass unless you leave all your items behind.",
        );
        return -1;
    }

    // Remove all spells
    for n in 0..20 {
        let spell_id = gs.characters[cn].spell[n];
        if spell_id != 0 {
            {
                gs.items[spell_id as usize].used = USE_EMPTY;
            };
            {
                gs.characters[cn].spell[n] = 0;
            };
        }
    }

    gs.do_update_char(cn);

    1
}

pub fn step_portal2_lab13(gs: &mut GameState, cn: usize, _item_idx: usize) -> i32 {
    let is_player = { (gs.characters[cn].flags & CharacterFlags::Player.bits()) != 0 };
    if !is_player {
        return -1;
    }

    // Check area 1: x=48-80, y=594-608
    let mut flag = 0;
    for x in 48..=80 {
        for y in 594..=608 {
            let m = x + y * SERVER_MAPX as usize;
            let co = gs.map[m].ch;
            if co != 0 && co != cn as u32 {
                let is_other_player =
                    { (gs.characters[co as usize].flags & CharacterFlags::Player.bits()) != 0 };
                if is_other_player {
                    flag = 1;
                }
            }

            let in2 = gs.map[m].it;
            if in2 != 0 {
                let temp = gs.items[in2 as usize].temp;
                if temp == 664 || temp == 170 {
                    flag = 2;
                    break;
                }
            }

            // Match original C: if we saw another player on this tile (flag=1),
            // we still check for an expiring item (flag=2) on the same tile.
            // Only after the item check do we stop scanning.
            if flag == 1 {
                break;
            }
        }
        if flag != 0 {
            break;
        }
    }

    // Check area 2: x=38-48, y=593-602
    if flag == 0 {
        for x in 38..=48 {
            for y in 593..=602 {
                let m = x + y * SERVER_MAPX as usize;
                let co = gs.map[m].ch;
                if co != 0 && co != cn as u32 {
                    let is_other_player =
                        { (gs.characters[co as usize].flags & CharacterFlags::Player.bits()) != 0 };
                    if is_other_player {
                        flag = 1;
                    }
                }

                let in2 = gs.map[m].it;
                if in2 != 0 {
                    let temp = gs.items[in2 as usize].temp;
                    if temp == 664 || temp == 170 {
                        flag = 2;
                        break;
                    }
                }

                if flag == 1 {
                    break;
                }
            }
            if flag != 0 {
                break;
            }
        }
    }

    if flag == 2 {
        gs.do_character_log(
            cn,
            core::types::FontColor::Red,
            "The Final Test is waiting for a certain item to expire, please try again later.",
        );
        return -1;
    }

    if flag == 1 {
        gs.do_character_log(
            cn,
            core::types::FontColor::Red,
            "You may not pass while another player is inside.",
        );
        return -1;
    }

    // Check for gatekeeper (character template 51)
    flag = 0;
    for n in 0..core::constants::MAXCHARS {
        let (used, flags, temp, a_hp, hp5, a_mana, mana5) = {
            (
                gs.characters[n].used,
                gs.characters[n].flags,
                gs.characters[n].temp,
                gs.characters[n].a_hp,
                gs.characters[n].hp[5],
                gs.characters[n].a_mana,
                gs.characters[n].mana[5],
            )
        };

        if used != core::constants::USE_ACTIVE || (flags & CharacterFlags::Body.bits()) != 0 {
            continue;
        }
        if temp != 51 {
            continue;
        }
        if a_hp > (hp5 as i32 * 900) && a_mana > (mana5 as i32 * 900) {
            flag = 1;
        }
        break;
    }

    if flag == 0 {
        gs.do_character_log(
            cn,
            core::types::FontColor::Red,
            "The Gatekeeper is currently busy. Please try again in a few minutes.",
        );
        return -1;
    }

    // Check if doors are closed (item 15220)
    let door_data = gs.items[15220].data[1];
    if door_data == 0 {
        gs.do_character_log(
            cn,
            core::types::FontColor::Red,
            "The doors aren't closed again yet. Please try again in a few minutes.",
        );
        return -1;
    }

    // Remove all spells
    for n in 0..20 {
        let spell_id = gs.characters[cn].spell[n];
        if spell_id != 0 {
            {
                gs.items[spell_id as usize].used = USE_EMPTY;
            };
            {
                gs.characters[cn].spell[n] = 0;
            };
        }
    }

    // Remove items with temp 664
    for n in 0..40 {
        let item_id = gs.characters[cn].item[n];
        if item_id != 0 {
            let temp = gs.items[item_id as usize].temp;
            if temp == 664 {
                {
                    gs.characters[cn].item[n] = 0;
                };
                {
                    gs.items[item_id as usize].used = USE_EMPTY;
                };
            }
        }
    }

    let citem = gs.characters[cn].citem;
    if citem != 0 && (citem & 0x80000000) == 0 {
        let temp = gs.items[citem as usize].temp;
        if temp == 664 {
            {
                gs.characters[cn].citem = 0;
            };
            {
                gs.items[citem as usize].used = USE_EMPTY;
            };
        }
    }
    gs.do_update_char(cn);

    1
}

pub fn step_portal_arena(gs: &mut GameState, cn: usize, item_idx: usize) -> i32 {
    // Check for arena token (temp 687) in citem
    let citem = gs.characters[cn].citem;
    let mut flag = 0;
    if citem != 0 && (citem & 0x80000000) == 0 {
        let temp = gs.items[citem as usize].temp;
        if temp == 687 {
            {
                gs.characters[cn].citem = 0;
            };
            {
                gs.items[citem as usize].used = USE_EMPTY;
            };
            flag = 1;
        }
    }

    // Check inventory for token
    for n in 0..40 {
        let item_id = gs.characters[cn].item[n];
        if item_id != 0 {
            let temp = gs.items[item_id as usize].temp;
            if temp == 687 {
                flag = 1;
                {
                    gs.characters[cn].item[n] = 0;
                };
                {
                    gs.items[item_id as usize].used = USE_EMPTY;
                };
            }
        }
    }

    if flag == 1 {
        gs.do_character_log(
            cn,
            core::types::FontColor::Yellow,
            "A winner! You gain one arena-rank!",
        );
        {
            gs.characters[cn].data[22] += 1;
            gs.characters[cn].data[23] = 1;
        };
        return 1;
    }

    // Get arena rank
    let nr = gs.characters[cn].data[22] as usize;
    let nr = nr + 364;
    if nr > 381 {
        gs.do_character_log(
            cn,
            core::types::FontColor::Yellow,
            "Please tell the gods to add more potent monsters to the arena.",
        );
        return -1;
    }

    // Get arena bounds
    let (data1, data2) = (gs.items[item_idx].data[1], gs.items[item_idx].data[2]);
    let xs = (data1 as usize) % SERVER_MAPX as usize;
    let ys = (data1 as usize) / SERVER_MAPX as usize;
    let xe = (data2 as usize) % SERVER_MAPX as usize;
    let ye = (data2 as usize) / SERVER_MAPX as usize;

    // Check if character is forfeiting
    let (frx, fry) = {
        (
            gs.characters[cn].frx as usize,
            gs.characters[cn].fry as usize,
        )
    };
    if frx >= xs && frx <= xe && fry >= ys && fry <= ye {
        gs.do_character_log(
            cn,
            core::types::FontColor::Yellow,
            "You forfeit this fight.",
        );
        return 1;
    }

    // Check if arena is occupied
    for x in xs..=xe {
        for y in ys..=ye {
            let m = x + y * SERVER_MAPX as usize;
            let occupied = gs.map[m].ch != 0;
            if occupied {
                gs.do_character_log(
                    cn,
                    core::types::FontColor::Yellow,
                    "The arena is busy. Please come back later.",
                );
                return -1;
            }
        }
    }

    // Create enemy
    let co = match populate::pop_create_char(gs, nr, false) {
        Some(co) => co,
        None => {
            gs.do_character_log(
                cn,
                core::types::FontColor::Red,
                "Please tell the gods that the arena isn't working.",
            );
            return -1;
        }
    };

    let data0 = gs.items[item_idx].data[0];
    let drop_x = (data0 as usize) % SERVER_MAPX as usize;
    let drop_y = (data0 as usize) / SERVER_MAPX as usize;

    if !God::drop_char_fuzzy(gs, co, drop_x, drop_y) {
        gs.do_character_log(
            cn,
            core::types::FontColor::Red,
            "Please tell the gods that the arena isn't working.",
        );
        return -1;
    }

    {
        {
            gs.characters[co].data[64] = gs.globals.ticker + (core::constants::TICKS * 60 * 5);
        };
    };

    // Create arena token
    if let Some(in2) = God::create_item(gs, 687) {
        God::give_character_item(gs, co, in2);
    }

    {
        gs.characters[cn].data[23] = 0;
    };

    1
}

pub fn step_teleport(gs: &mut GameState, cn: usize, item_idx: usize) -> i32 {
    if cn == 0 {
        log::error!("step_teleport(gs, ): cn = 0");
        return -1;
    }

    let (x, y) = {
        (
            gs.items[item_idx].data[0] as usize,
            gs.items[item_idx].data[1] as usize,
        )
    };

    if x >= SERVER_MAPX as usize || y >= (SERVER_MAPX * 2) as usize {
        log::error!("step_teleport(gs, ): bad coordinates in item {}", item_idx);
        return -1;
    }

    let m = x + y * SERVER_MAPX as usize;

    // Check for unoccupied landing spot
    let loc_off: [isize; 5] = [0, -(SERVER_MAPX as isize), SERVER_MAPX as isize, 1, -1];
    let mut m3 = 0;

    for offset in loc_off.iter() {
        let m2 = (m as isize + offset) as usize;
        if m2 >= (SERVER_MAPX * SERVER_MAPX * 2) as usize {
            continue;
        }

        let (map_flags, ch, to_ch, it) = (
            gs.map[m2].flags,
            gs.map[m2].ch,
            gs.map[m2].to_ch,
            gs.map[m2].it,
        );

        if (map_flags & core::constants::MF_MOVEBLOCK as u64) != 0 {
            continue;
        }
        if ch != 0 {
            continue;
        }
        if to_ch != 0 {
            continue;
        }
        if it != 0 {
            let it_flags = gs.items[it as usize].flags;
            if (it_flags & ItemFlags::IF_MOVEBLOCK.bits()) != 0 {
                continue;
            }
        }
        if (map_flags & ((core::constants::MF_TAVERN | core::constants::MF_DEATHTRAP) as u64)) != 0
        {
            continue;
        }

        m3 = m2;
        break;
    }

    if m3 == 0 {
        // Target occupied: fail silently
        return -1;
    }

    // Add departure effect
    {
        EffectManager::fx_add_effect(
            6,
            0,
            gs.characters[cn].x as i32,
            gs.characters[cn].y as i32,
            0,
        )
    };

    player::plr_map_remove(gs, cn);

    // Update character position
    {
        gs.characters[cn].status = 0;
        gs.characters[cn].attack_cn = 0;
        gs.characters[cn].skill_nr = 0;
        gs.characters[cn].goto_x = 0;
    };

    // Set new position
    {
        gs.map[m3].ch = cn as u32;
        gs.map[m3].to_ch = 0;
    };
    {
        gs.characters[cn].x = (m3 % SERVER_MAPX as usize) as i16;
        gs.characters[cn].y = (m3 / SERVER_MAPX as usize) as i16;
    };

    let (new_x, new_y) = (gs.characters[cn].x, gs.characters[cn].y);
    gs.do_area_notify(
        cn as i32,
        0,
        new_x as i32,
        new_y as i32,
        1,
        cn as i32,
        0,
        0,
        0,
    );
    // NT_SEE = 1

    // Add arrival effect
    {
        EffectManager::fx_add_effect(
            6,
            0,
            gs.characters[cn].x as i32,
            gs.characters[cn].y as i32,
            0,
        )
    };

    2 // TELEPORT_SUCCESS
}

pub fn step_firefloor(gs: &mut GameState, cn: usize, item_idx: usize) -> i32 {
    gs.do_character_log(cn, core::types::FontColor::Red, "Outch!\n");

    let in2 = match God::create_item(gs, 1) {
        Some(idx) => idx,
        None => return 0,
    };

    {
        gs.items[in2].name[..4].copy_from_slice(b"Fire");
        gs.items[in2].reference[..4].copy_from_slice(b"fire");
        gs.items[in2].description[..5].copy_from_slice(b"Fire.");
        gs.items[in2].hp[0] = -5000;
        gs.items[in2].active = 1;
        gs.items[in2].duration = 1;
        gs.items[in2].flags = ItemFlags::IF_SPELL.bits() | ItemFlags::IF_PERMSPELL.bits();
    };

    let (temp, sprite1) = (gs.items[item_idx].temp, gs.items[item_idx].sprite[1]);
    {
        gs.items[in2].temp = temp;
        gs.items[in2].sprite[1] = sprite1;
    };

    driver::add_spell(gs, cn, in2);

    0
}

pub fn step_firefloor_remove(gs: &mut GameState, cn: usize, item_idx: usize) {
    let temp = gs.items[item_idx].temp;

    for n in 0..20 {
        let in2 = gs.characters[cn].spell[n];
        if in2 != 0 {
            let spell_temp = gs.items[in2 as usize].temp;
            if spell_temp == temp {
                {
                    gs.items[in2 as usize].used = USE_EMPTY;
                };
                {
                    gs.characters[cn].spell[n] = 0;
                };
                return;
            }
        }
    }
}

pub fn step_driver(gs: &mut GameState, cn: usize, item_idx: usize) -> i32 {
    let driver = gs.items[item_idx].driver;

    let ret = match driver {
        36 => step_portal1_lab13(gs, cn, item_idx),
        37 => step_trap(gs, cn, item_idx),
        38 => step_portal2_lab13(gs, cn, item_idx),
        47 => step_portal_arena(gs, cn, item_idx),
        62 => step_teleport(gs, cn, item_idx),
        69 => step_firefloor(gs, cn, item_idx),
        _ => {
            {
                log::error!(
                    "unknown step driver {} for item {} ({})",
                    gs.items[item_idx].driver,
                    gs.items[item_idx].get_name(),
                    item_idx
                );
            };
            0
        }
    };

    ret
}

pub fn step_driver_remove(gs: &mut GameState, cn: usize, item_idx: usize) {
    let driver = gs.items[item_idx].driver;

    match driver {
        36 => {}
        37 => step_trap_remove(gs, cn, item_idx),
        38 => {}
        47 => {}
        62 => {}
        69 => step_firefloor_remove(gs, cn, item_idx),
        _ => {
            {
                log::error!(
                    "unknown step driver {} for item {} ({})",
                    gs.items[item_idx].driver,
                    gs.items[item_idx].get_name(),
                    item_idx
                );
            };
        }
    }
}
