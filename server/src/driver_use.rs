// Helper function to take an item from a character
fn take_item_from_char(item_idx: usize, cn: usize) {
    use crate::repository::Repository;

    Repository::with_characters_mut(|characters| {
        let ch = &mut characters[cn];

        // Check citem first
        if ch.citem as usize == item_idx {
            ch.citem = 0;
            return;
        }

        // Check inventory
        for n in 0..40 {
            if ch.item[n] as usize == item_idx {
                ch.item[n] = 0;
                return;
            }
        }

        // Check worn items
        for n in 0..20 {
            if ch.worn[n] as usize == item_idx {
                ch.worn[n] = 0;
                return;
            }
        }
    });

    // Clear item position
    Repository::with_items_mut(|items| {
        items[item_idx].x = 0;
        items[item_idx].y = 0;
        items[item_idx].carried = 0;
    });

    // TODO: do_update_char when implemented
}

pub fn sub_door_driver(cn: usize, item_idx: usize) -> i32 {
    use crate::repository::Repository;
    use core::constants::SERVER_MAPX;

    Repository::with_items(|items| {
        let item = &items[item_idx];

        if item.data[0] == 65500 {
            return 0;
        }

        if item.data[0] == 65501 || item.data[0] == 65502 {
            // Star door in black stronghold
            let mut empty = 0;
            let mut star = 0;
            let mut circle = 0;
            let loctab: [usize; 4] = [344487, 343463, 344488, 343464];

            for n in 0..4 {
                let map_idx = loctab[n];
                Repository::with_map(|map| {
                    let in2 = map[map_idx].it as usize;
                    if in2 == 0 {
                        return;
                    }

                    if items[in2].data[1] != n as i32 {
                        return;
                    }

                    if items[in2].temp == 761 {
                        star += 1;
                    }
                    if items[in2].temp == 762 {
                        circle += 1;
                    }
                    if items[in2].temp == 763 {
                        empty += 1;
                    }
                });
            }

            if item.data[0] == 65501 && empty == 3 && star == 1 {
                return 1;
            } else if item.data[0] == 65502 && empty == 3 && circle == 1 {
                return 1;
            } else {
                return 0;
            }
        }

        0
    })
}

pub fn use_door(cn: usize, item_idx: usize) -> i32 {
    use crate::repository::Repository;
    use crate::state::State;
    use core::constants::{ItemFlags, SERVER_MAPX, SK_LOCK, USE_EMPTY};
    use rand::Rng;

    // Check if someone is standing on the door
    let map_idx = Repository::with_items(|items| {
        let item = &items[item_idx];
        item.x as usize + item.y as usize * SERVER_MAPX as usize
    });

    let blocked = Repository::with_map(|map| map[map_idx].ch != 0);
    if blocked {
        return 0;
    }

    let mut lock = 0;

    // Check lock requirements
    Repository::with_items(|items| {
        let item = &items[item_idx];

        if item.data[0] != 0 {
            if cn == 0 {
                lock = 1;
            } else if item.data[0] >= 65500 {
                lock = sub_door_driver(cn, item_idx);
            } else {
                // Check if character has the right key
                Repository::with_characters(|characters| {
                    let character = &characters[cn];

                    // Check citem (carried item)
                    let citem = character.citem as usize;
                    if citem != 0
                        && (citem & 0x80000000) == 0
                        && items[citem].temp == item.data[0] as u32
                    {
                        lock = 1;
                        if item.data[3] != 0 {
                            // Key vanishes - will be handled in mutable section
                        }
                    } else {
                        // Check inventory
                        for n in 0..40 {
                            let in2 = character.item[n] as usize;
                            if in2 != 0 && items[in2].temp == item.data[0] as u32 {
                                lock = 1;
                                break;
                            }
                        }
                    }
                });

                // Try to pick the lock with lockpicks
                if lock == 0 {
                    Repository::with_characters(|characters| {
                        let character = &characters[cn];
                        let citem = character.citem as usize;

                        if citem != 0 && (citem & 0x80000000) == 0 && items[citem].driver == 3 {
                            let mut rng = rand::thread_rng();
                            let skill = character.skill[SK_LOCK][5] + items[citem].data[0];
                            let power = item.data[2];

                            if power == 0 || skill >= power + rng.gen_range(0..20) {
                                lock = 1;
                            } else {
                                State::with(|state| {
                                    state.do_character_log(
                                        cn,
                                        core::types::FontColor::LOG_SYSTEM,
                                        "You failed to pick the lock.",
                                    );
                                });
                            }
                            // Damage lockpick
                            item_damage_citem(cn, 1);
                        }
                    });
                }

                if item.data[1] != 0 && lock == 0 {
                    State::with(|state| {
                        state.do_character_log(
                            cn,
                            core::types::FontColor::LOG_SYSTEM,
                            "It's locked and you don't have the right key.",
                        );
                    });
                    return 0;
                }
            }
        }

        0
    });

    // Now modify the door state
    Repository::with_items_mut(|items| {
        let item = &mut items[item_idx];
        let item_x = item.x as i32;
        let item_y = item.y as i32;

        State::with_mut(|state| {
            state.reset_go(item_x, item_y);
            state.remove_lights(item_x, item_y);
        });

        State::do_area_sound(0, 0, item_x, item_y, 10);

        if item.active == 0 {
            item.flags &= !(ItemFlags::IF_MOVEBLOCK.bits() | ItemFlags::IF_SIGHTBLOCK.bits());
            item.data[1] = 0;
        } else {
            // Get template flags
            let temp = item.temp as usize;
            let flags = Repository::with_item_templates(|templates| {
                templates[temp].flags & ItemFlags::IF_SIGHTBLOCK.bits()
            });

            item.flags |= ItemFlags::IF_MOVEBLOCK.bits() | flags;
            if lock != 0 {
                item.data[1] = 1;
            }
        }

        State::with_mut(|state| {
            state.reset_go(item_x, item_y);
            state.add_lights(item_x, item_y);
        });

        Repository::with_characters(|characters| {
            let ch = &characters[cn];
            State::with_mut(|state| {
                state.do_area_notify(
                    cn,
                    0,
                    ch.x as i32,
                    ch.y as i32,
                    core::constants::NT_SEE,
                    cn,
                    0,
                    0,
                    0,
                );
            });
        });
    });

    1
}

pub fn use_create_item(cn: usize, item_idx: usize) -> i32 {
    use crate::god::God;
    use crate::repository::Repository;
    use crate::state::State;
    use core::constants::MAXTITEM;

    if cn == 0 {
        return 0;
    }

    let (active, template_id) =
        Repository::with_items(|items| (items[item_idx].active, items[item_idx].data[0] as usize));

    if active != 0 {
        return 0;
    }

    if template_id <= 0 || template_id >= MAXTITEM {
        return 0;
    }

    let in2 = match God::create_item(template_id) {
        Some(id) => id,
        None => return 0,
    };

    if !God::give_character_item(cn, in2) {
        Repository::with_items(|items| {
            let item_ref = items[in2].reference.clone();
            State::with(|state| {
                state.do_character_log(
                    cn,
                    core::types::FontColor::LOG_SYSTEM,
                    &format!("Your backpack is full, so you can't take the {}.", item_ref),
                );
            });
        });
        Repository::with_items_mut(|items| {
            items[in2].used = core::constants::USE_EMPTY;
        });
        return 0;
    }

    Repository::with_items(|items| {
        let item_ref = items[in2].reference.clone();
        let item_name = items[in2].name.clone();
        let source_name = items[item_idx].name.clone();

        State::with(|state| {
            state.do_character_log(
                cn,
                core::types::FontColor::LOG_INFO,
                &format!("You got a {}.", item_ref),
            );
        });

        log::info!("Character {} got {} from {}", cn, item_name, source_name);
    });

    // Handle special driver types
    Repository::with_items(|items| {
        let driver = items[item_idx].driver;
        let data1 = items[item_idx].data[1];

        if data1 != 0 && driver == 53 {
            Repository::with_characters(|characters| {
                let char_name = characters[cn].name.clone();
                Repository::with_items_mut(|items| {
                    let item = &mut items[in2];
                    State::with(|state| {
                        state.do_character_log(
                            cn,
                            core::types::FontColor::LOG_SYSTEM,
                            &format!(
                                "You feel yourself form a magical connection with the {}.",
                                item.reference
                            ),
                        );
                    });
                    item.data[0] = cn as i32;

                    let new_desc = format!(
                        "{} Engraved in it are the letters \"{}\".",
                        item.description, char_name
                    );
                    if new_desc.len() < 200 {
                        item.description = new_desc;
                    }
                });
            });
        }

        if driver == 54 {
            let (x, y) = (items[item_idx].x as i32, items[item_idx].y as i32);
            State::with_mut(|state| {
                state.do_area_notify(cn, 0, x, y, core::constants::NT_HITME, cn, 0, 0, 0);
            });
        }
    });

    1
}

pub fn use_create_gold(cn: usize, item_idx: usize) -> i32 {
    use crate::repository::Repository;
    use crate::state::State;

    if cn == 0 {
        return 0;
    }

    let (active, gold_amount) =
        Repository::with_items(|items| (items[item_idx].active, items[item_idx].data[0]));

    if active != 0 {
        return 0;
    }

    let gold_to_add = gold_amount * 100;

    Repository::with_characters_mut(|characters| {
        characters[cn].gold += gold_to_add;
    });

    Repository::with_items(|items| {
        State::with(|state| {
            state.do_character_log(
                cn,
                core::types::FontColor::LOG_INFO,
                &format!("You got a {}G.", gold_amount),
            );
        });

        log::info!(
            "Character {} got {}G from {}",
            cn,
            gold_amount,
            items[item_idx]
                .name
                .iter()
                .take_while(|&&c| c != 0)
                .map(|&c| c as char)
                .collect::<String>()
        );
    });

    1
}

pub fn use_create_item2(cn: usize, item_idx: usize) -> i32 {
    use crate::god::God;
    use crate::repository::Repository;
    use crate::state::State;
    use core::constants::{MAXTITEM, USE_EMPTY};

    if cn == 0 {
        return 0;
    }

    let (active, required_temp, template_id) = Repository::with_items(|items| {
        (
            items[item_idx].active,
            items[item_idx].data[1] as u32,
            items[item_idx].data[0] as usize,
        )
    });

    if active != 0 {
        return 0;
    }

    // Check if character has the required item in citem
    let citem = Repository::with_characters(|characters| characters[cn].citem as usize);

    if citem == 0 || (citem & 0x80000000) != 0 {
        return 0;
    }

    let citem_temp = Repository::with_items(|items| items[citem].temp);

    if citem_temp != required_temp {
        return 0;
    }

    if template_id <= 0 || template_id >= MAXTITEM {
        return 0;
    }

    let in2 = match God::create_item(template_id) {
        Some(id) => id,
        None => return 0,
    };

    if !God::give_character_item(cn, in2) {
        Repository::with_items(|items| {
            let item_ref = String::from_utf8_lossy(&items[in2].reference)
                .trim_end_matches('\0')
                .to_string();
            State::with(|state| {
                state.do_character_log(
                    cn,
                    core::types::FontColor::LOG_SYSTEM,
                    &format!("Your backpack is full, so you can't take the {}.", item_ref),
                );
            });
        });
        Repository::with_items_mut(|items| {
            items[in2].used = USE_EMPTY;
        });
        return 0;
    }

    Repository::with_items(|items| {
        let item_ref = String::from_utf8_lossy(&items[in2].reference)
            .trim_end_matches('\0')
            .to_string();
        State::with(|state| {
            state.do_character_log(
                cn,
                core::types::FontColor::LOG_INFO,
                &format!("You got a {}.", item_ref),
            );
        });

        log::info!("Character {} got item from source", cn);
    });

    // Remove the consumed item
    Repository::with_items_mut(|items| {
        items[citem].used = USE_EMPTY;
    });
    Repository::with_characters_mut(|characters| {
        characters[cn].citem = 0;
    });

    1
}

pub fn use_create_item3(cn: usize, item_idx: usize) -> i32 {
    use crate::god::God;
    use crate::repository::Repository;
    use crate::state::State;
    use core::constants::{MAXTITEM, USE_EMPTY};
    use rand::Rng;

    if cn == 0 {
        return 0;
    }

    let active = Repository::with_items(|items| items[item_idx].active);

    if active != 0 {
        return 0;
    }

    // Find how many data entries are non-zero
    let data_entries = Repository::with_items(|items| {
        let item = &items[item_idx];
        let mut count = 0;
        for n in 0..10 {
            if item.data[n] == 0 {
                break;
            }
            count += 1;
        }
        if count == 0 {
            return None;
        }
        Some((count, item.data.clone()))
    });

    let (count, data) = match data_entries {
        Some(v) => v,
        None => return 0,
    };

    // Pick a random entry
    let mut rng = rand::thread_rng();
    let n = rng.gen_range(0..count);
    let template_id = data[n] as usize;

    if template_id <= 0 || template_id >= MAXTITEM {
        return 0;
    }

    // Check if this is a special item template
    let in2 = match template_id {
        57 | 59 | 63 | 65 | 69 | 71 | 75 | 76 | 94 | 95 | 981 | 982 => {
            // These would call create_special_item, but we don't have that yet
            log::warn!(
                "Special item {} requested but create_special_item not implemented yet",
                template_id
            );
            God::create_item(template_id)
        }
        _ => God::create_item(template_id),
    };

    let in2 = match in2 {
        Some(id) => id,
        None => {
            State::with(|state| {
                state.do_character_log(cn, core::types::FontColor::LOG_INFO, "It's empty...");
            });
            return 1;
        }
    };

    if !God::give_character_item(cn, in2) {
        State::with(|state| {
            state.do_character_log(
                cn,
                core::types::FontColor::LOG_SYSTEM,
                "Your backpack is full, so you can't take anything.",
            );
        });
        Repository::with_items_mut(|items| {
            items[in2].used = USE_EMPTY;
        });
        return 0;
    }

    Repository::with_items(|items| {
        let item_ref = String::from_utf8_lossy(&items[in2].reference)
            .trim_end_matches('\0')
            .to_string();
        State::with(|state| {
            state.do_character_log(
                cn,
                core::types::FontColor::LOG_INFO,
                &format!("You got a {}.", item_ref),
            );
        });
    });

    1
}

pub fn use_mix_potion(cn: usize, item_idx: usize) -> i32 {
    use crate::god::God;
    use crate::repository::Repository;
    use crate::state::State;
    use core::constants::{ItemFlags, USE_EMPTY};

    if cn == 0 {
        return 0;
    }

    let citem = Repository::with_characters(|characters| characters[cn].citem as usize);

    if citem == 0 || (citem & 0x80000000) != 0 {
        State::with(|state| {
            state.do_character_log(
                cn,
                core::types::FontColor::LOG_SYSTEM,
                "What do you want to do with it?",
            );
        });
        return 0;
    }

    let carried = Repository::with_items(|items| items[item_idx].carried);
    if carried == 0 {
        State::with(|state| {
            state.do_character_log(
                cn,
                core::types::FontColor::LOG_SYSTEM,
                "Too difficult to do on the ground.",
            );
        });
        return 0;
    }

    let (base_temp, ingredient_temp) =
        Repository::with_items(|items| (items[item_idx].temp, items[citem].temp));

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
            State::with(|state| {
                state.do_character_log(cn, core::types::FontColor::LOG_SYSTEM, "Sorry?");
            });
            return 0;
        }
    };

    let in3 = match God::create_item(result_template) {
        Some(id) => id,
        None => return 0,
    };

    Repository::with_items_mut(|items| {
        items[in3].flags |= ItemFlags::IF_UPDATE.bits();
        items[citem].used = USE_EMPTY;
        items[item_idx].used = USE_EMPTY;
    });

    Repository::with_characters_mut(|characters| {
        characters[cn].citem = 0;
    });

    take_item_from_char(item_idx, cn);
    God::give_character_item(cn, in3);

    1
}

pub fn use_chain(cn: usize, item_idx: usize) -> i32 {
    use crate::god::God;
    use crate::repository::Repository;
    use crate::state::State;
    use core::constants::{ItemFlags, USE_EMPTY};

    if cn == 0 {
        return 0;
    }

    let citem = Repository::with_characters(|characters| characters[cn].citem as usize);

    if citem == 0 || (citem & 0x80000000) != 0 {
        State::with(|state| {
            state.do_character_log(
                cn,
                core::types::FontColor::LOG_SYSTEM,
                "What do you want to do with it?",
            );
        });
        return 0;
    }

    let carried = Repository::with_items(|items| items[item_idx].carried);
    if carried == 0 {
        State::with(|state| {
            state.do_character_log(
                cn,
                core::types::FontColor::LOG_SYSTEM,
                "Too difficult to do on the ground.",
            );
        });
        return 0;
    }

    let citem_temp = Repository::with_items(|items| items[citem].temp);
    if citem_temp != 206 {
        State::with(|state| {
            state.do_character_log(cn, core::types::FontColor::LOG_SYSTEM, "Sorry?");
        });
        return 0;
    }

    let (current_temp, max_data) =
        Repository::with_items(|items| (items[item_idx].temp as i32, items[item_idx].data[0]));

    if current_temp >= max_data {
        State::with(|state| {
            state.do_character_log(
                cn,
                core::types::FontColor::LOG_SYSTEM,
                "It won't fit anymore.",
            );
        });
        return 0;
    }

    let in3 = match God::create_item((current_temp + 1) as usize) {
        Some(id) => id,
        None => return 0,
    };

    Repository::with_items_mut(|items| {
        items[in3].flags |= ItemFlags::IF_UPDATE.bits();
        items[citem].used = USE_EMPTY;
        items[item_idx].used = USE_EMPTY;
    });

    Repository::with_characters_mut(|characters| {
        characters[cn].citem = 0;
    });

    take_item_from_char(item_idx, cn);
    God::give_character_item(cn, in3);

    1
}

pub fn stone_sword(cn: usize, item_idx: usize) -> i32 {
    use crate::god::God;
    use crate::repository::Repository;
    use crate::state::State;
    use core::constants::MAXTITEM;

    if cn == 0 {
        return 0;
    }

    let (active, template_id) =
        Repository::with_items(|items| (items[item_idx].active, items[item_idx].data[0] as usize));

    if active != 0 {
        return 0;
    }

    if template_id <= 0 || template_id >= MAXTITEM {
        return 0;
    }

    // Check if character has enough strength (100+)
    let strength = Repository::with_characters(|characters| {
        characters[cn].attrib[0][5] // AT_STREN = 0
    });

    if strength < 100 {
        State::with(|state| {
            state.do_character_log(
                cn,
                core::types::FontColor::LOG_SYSTEM,
                "You're not strong enough.",
            );
        });
        return 0;
    }

    let in2 = match God::create_item(template_id) {
        Some(id) => id,
        None => return 0,
    };

    God::give_character_item(cn, in2);

    Repository::with_items(|items| {
        let item_ref = String::from_utf8_lossy(&items[in2].reference)
            .trim_end_matches('\0')
            .to_string();
        State::with(|state| {
            state.do_character_log(
                cn,
                core::types::FontColor::LOG_INFO,
                &format!("You got a {}.", item_ref),
            );
        });
    });

    1
}

pub fn finish_laby_teleport(cn: usize, nr: usize, exp: usize) -> i32 {
    use crate::god::God;
    use crate::repository::Repository;
    use crate::state::State;
    use core::constants::{ItemFlags, USE_EMPTY};

    // Update labyrinth progress if this is a new level
    let current_progress = Repository::with_characters(|characters| characters[cn].data[20]);

    if (current_progress as usize) < nr {
        Repository::with_characters_mut(|characters| {
            characters[cn].data[20] = nr as u32;
        });

        let ordinal = match nr {
            1 => "st",
            2 => "nd",
            3 => "rd",
            _ => "th",
        };

        State::with(|state| {
            state.do_character_log(
                cn,
                core::types::FontColor::Green,
                &format!(
                    "You have solved the {}{} part of the Labyrinth.\n",
                    nr, ordinal
                ),
            );
        });

        // TODO: Implement do_give_exp(cn, exp, 0, -1)
        // TODO: Implement chlog(cn, "Solved Labyrinth Part %d", nr)
    }

    // Remove items with IF_LABYDESTROY flag from citem
    let citem = Repository::with_characters(|characters| characters[cn].citem);
    if citem != 0 && (citem & 0x80000000) == 0 {
        let has_labydestroy = Repository::with_items(|items| {
            items[citem as usize]
                .flags
                .contains(ItemFlags::IF_LABYDESTROY)
        });

        if has_labydestroy {
            let item_ref = Repository::with_items(|items| {
                String::from_utf8_lossy(&items[citem as usize].reference)
                    .trim_end_matches('\0')
                    .to_string()
            });

            Repository::with_characters_mut(|characters| {
                characters[cn].citem = 0;
            });

            Repository::with_items_mut(|items| {
                items[citem as usize].used = USE_EMPTY;
            });

            State::with(|state| {
                state.do_character_log(
                    cn,
                    core::types::FontColor::Yellow,
                    &format!("Your {} vanished.\n", item_ref),
                );
            });
        }
    }

    // Remove items with IF_LABYDESTROY flag from inventory (40 slots)
    for n in 0..40 {
        let item_idx = Repository::with_characters(|characters| characters[cn].item[n]);
        if item_idx != 0 {
            let has_labydestroy = Repository::with_items(|items| {
                items[item_idx as usize]
                    .flags
                    .contains(ItemFlags::IF_LABYDESTROY)
            });

            if has_labydestroy {
                let item_ref = Repository::with_items(|items| {
                    String::from_utf8_lossy(&items[item_idx as usize].reference)
                        .trim_end_matches('\0')
                        .to_string()
                });

                Repository::with_characters_mut(|characters| {
                    characters[cn].item[n] = 0;
                });

                Repository::with_items_mut(|items| {
                    items[item_idx as usize].used = USE_EMPTY;
                });

                State::with(|state| {
                    state.do_character_log(
                        cn,
                        core::types::FontColor::Yellow,
                        &format!("Your {} vanished.\n", item_ref),
                    );
                });
            }
        }
    }

    // Remove items with IF_LABYDESTROY flag from worn (20 slots)
    for n in 0..20 {
        let item_idx = Repository::with_characters(|characters| characters[cn].worn[n]);
        if item_idx != 0 {
            let has_labydestroy = Repository::with_items(|items| {
                items[item_idx as usize]
                    .flags
                    .contains(ItemFlags::IF_LABYDESTROY)
            });

            if has_labydestroy {
                let item_ref = Repository::with_items(|items| {
                    String::from_utf8_lossy(&items[item_idx as usize].reference)
                        .trim_end_matches('\0')
                        .to_string()
                });

                Repository::with_characters_mut(|characters| {
                    characters[cn].worn[n] = 0;
                });

                Repository::with_items_mut(|items| {
                    items[item_idx as usize].used = USE_EMPTY;
                });

                State::with(|state| {
                    state.do_character_log(
                        cn,
                        core::types::FontColor::Yellow,
                        &format!("Your {} vanished.\n", item_ref),
                    );
                });
            }
        }
    }

    // Remove all spells (20 slots)
    for n in 0..20 {
        let spell_idx = Repository::with_characters(|characters| characters[cn].spell[n]);
        if spell_idx != 0 {
            let item_name = Repository::with_items(|items| {
                String::from_utf8_lossy(&items[spell_idx as usize].name)
                    .trim_end_matches('\0')
                    .to_string()
            });

            Repository::with_characters_mut(|characters| {
                characters[cn].spell[n] = 0;
            });

            Repository::with_items_mut(|items| {
                items[spell_idx as usize].used = USE_EMPTY;
            });

            State::with(|state| {
                state.do_character_log(
                    cn,
                    core::types::FontColor::Yellow,
                    &format!("Your {} vanished.\n", item_name),
                );
            });
        }
    }

    // Add effects and transfer character
    // TODO: Implement fx_add_effect(6, 0, x, y, 0)
    God::transfer_char(cn, 512, 512);
    // TODO: Implement fx_add_effect(6, 0, x, y, 0)

    // Update temple and tavern coordinates
    let (x, y) = Repository::with_characters(|characters| (characters[cn].x, characters[cn].y));
    Repository::with_characters_mut(|characters| {
        characters[cn].temple_x = x;
        characters[cn].temple_y = y;
        characters[cn].tavern_x = x;
        characters[cn].tavern_y = y;
    });

    1
}

pub fn is_nolab_item(item_idx: usize) -> bool {
    use crate::repository::Repository;

    if !core::types::Item::is_sane_item(item_idx) {
        return false;
    }

    Repository::with_items(|items| {
        let temp = items[item_idx].temp;
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
    })
}

pub fn teleport(cn: usize, item_idx: usize) -> i32 {
    use crate::god::God;
    use crate::repository::Repository;
    use crate::state::State;
    use core::constants::{ItemFlags, SK_RECALL, USE_EMPTY};

    if cn == 0 {
        return 1;
    }

    // Check if item needs to be activated first
    let (has_useactivate, is_active) = Repository::with_items(|items| {
        (
            items[item_idx].flags.contains(ItemFlags::IF_USEACTIVATE),
            items[item_idx].active != 0,
        )
    });

    if has_useactivate && !is_active {
        return 1;
    }

    // Remove nolab items from citem
    let citem = Repository::with_characters(|characters| characters[cn].citem);
    if citem != 0 && is_nolab_item(citem as usize) {
        let item_ref = Repository::with_items(|items| {
            String::from_utf8_lossy(&items[citem as usize].reference)
                .trim_end_matches('\0')
                .to_string()
        });

        Repository::with_characters_mut(|characters| {
            characters[cn].citem = 0;
        });

        Repository::with_items_mut(|items| {
            items[citem as usize].used = USE_EMPTY;
        });

        State::with(|state| {
            state.do_character_log(
                cn,
                core::types::FontColor::Yellow,
                &format!("Your {} vanished.\n", item_ref),
            );
        });
    }

    // Remove nolab items from inventory (40 slots)
    for n in 0..40 {
        let inv_item = Repository::with_characters(|characters| characters[cn].item[n]);
        if inv_item != 0 && is_nolab_item(inv_item as usize) {
            let item_ref = Repository::with_items(|items| {
                String::from_utf8_lossy(&items[inv_item as usize].reference)
                    .trim_end_matches('\0')
                    .to_string()
            });

            Repository::with_characters_mut(|characters| {
                characters[cn].item[n] = 0;
            });

            Repository::with_items_mut(|items| {
                items[inv_item as usize].used = USE_EMPTY;
            });

            State::with(|state| {
                state.do_character_log(
                    cn,
                    core::types::FontColor::Yellow,
                    &format!("Your {} vanished.\n", item_ref),
                );
            });
        }
    }

    // Remove recall spells from spell slots (20 slots)
    for n in 0..20 {
        let spell_idx = Repository::with_characters(|characters| characters[cn].spell[n]);
        if spell_idx != 0 {
            let is_recall =
                Repository::with_items(|items| items[spell_idx as usize].temp == SK_RECALL as u16);
            if is_recall {
                Repository::with_characters_mut(|characters| {
                    characters[cn].spell[n] = 0;
                });

                Repository::with_items_mut(|items| {
                    items[spell_idx as usize].used = USE_EMPTY;
                });
            }
        }
    }

    // Check if this is a lab-solved teleport (data[2] != 0)
    let data2 = Repository::with_items(|items| items[item_idx].data[2]);
    if data2 != 0 {
        // TODO: Implement use_labtransfer(cn, data[2], data[3])
        let data3 = Repository::with_items(|items| items[item_idx].data[3]);
        log::warn!(
            "use_labtransfer({}, {}, {}) not yet implemented",
            cn,
            data2,
            data3
        );
        return 1;
    }

    // Regular teleport
    let (dest_x, dest_y) = Repository::with_items(|items| {
        (
            items[item_idx].data[0] as usize,
            items[item_idx].data[1] as usize,
        )
    });

    // TODO: Implement fx_add_effect(6, 0, x, y, 0)
    God::transfer_char(cn, dest_x, dest_y);
    // TODO: Implement fx_add_effect(6, 0, x, y, 0)

    1
}

pub fn teleport2(cn: usize, item_idx: usize) -> i32 {
    use crate::god::God;
    use crate::repository::Repository;
    use crate::state::State;
    use core::constants::{ItemFlags, SK_RECALL};

    if cn == 0 {
        return 1;
    }

    // TODO: Implement chlog(cn, "Used teleport scroll to %d,%d (%s)", ...)

    // Check if lag scroll is too old (more than 4 minutes)
    let scroll_time = Repository::with_items(|items| items[item_idx].data[2]);
    if scroll_time != 0 {
        // TODO: Get globs->ticker and check if scroll_time + TICKS * 60 * 4 < ticker
        // For now, skip this check
        // TODO: Implement chlog for time difference
    }

    // Create a recall spell item
    let spell_item = match God::create_item(1) {
        Some(id) => id,
        None => {
            log::error!("god_create_item failed in teleport2");
            return 0;
        }
    };

    // Configure the spell item
    let (power, dest_x, dest_y) = Repository::with_items(|items| {
        (
            items[item_idx].power,
            items[item_idx].data[0],
            items[item_idx].data[1],
        )
    });

    Repository::with_items_mut(|items| {
        let spell = &mut items[spell_item];

        // Set name
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
    });

    // Try to add spell to character
    // TODO: Implement add_spell(cn, spell_item)
    // For now, try to add to first empty spell slot
    let added = Repository::with_characters_mut(|characters| {
        for n in 0..20 {
            if characters[cn].spell[n] == 0 {
                characters[cn].spell[n] = spell_item as u32;
                return true;
            }
        }
        false
    });

    if !added {
        let spell_name = Repository::with_items(|items| {
            String::from_utf8_lossy(&items[spell_item].name)
                .trim_end_matches('\0')
                .to_string()
        });

        State::with(|state| {
            state.do_character_log(
                cn,
                core::types::FontColor::Yellow,
                &format!(
                    "Magical interference neutralised the {}'s effect.\n",
                    spell_name
                ),
            );
        });
        return 0;
    }

    // TODO: Implement fx_add_effect(7, 0, x, y, 0)

    1
}

pub fn use_labyrinth(cn: usize, item_idx: usize) -> i32 {
    use crate::god::God;
    use crate::repository::Repository;
    use crate::state::State;
    use core::constants::{SK_RECALL, USE_EMPTY};

    // Remove nolab items from citem
    let citem = Repository::with_characters(|characters| characters[cn].citem);
    if citem != 0 && is_nolab_item(citem as usize) {
        let item_ref = Repository::with_items(|items| {
            String::from_utf8_lossy(&items[citem as usize].reference)
                .trim_end_matches('\0')
                .to_string()
        });

        Repository::with_characters_mut(|characters| {
            characters[cn].citem = 0;
        });

        Repository::with_items_mut(|items| {
            items[citem as usize].used = USE_EMPTY;
        });

        State::with(|state| {
            state.do_character_log(
                cn,
                core::types::FontColor::Yellow,
                &format!("Your {} vanished.\n", item_ref),
            );
        });
    }

    // Remove nolab items from inventory (40 slots)
    for n in 0..40 {
        let inv_item = Repository::with_characters(|characters| characters[cn].item[n]);
        if inv_item != 0 && is_nolab_item(inv_item as usize) {
            let item_ref = Repository::with_items(|items| {
                String::from_utf8_lossy(&items[inv_item as usize].reference)
                    .trim_end_matches('\0')
                    .to_string()
            });

            Repository::with_characters_mut(|characters| {
                characters[cn].item[n] = 0;
            });

            Repository::with_items_mut(|items| {
                items[inv_item as usize].used = USE_EMPTY;
            });

            State::with(|state| {
                state.do_character_log(
                    cn,
                    core::types::FontColor::Yellow,
                    &format!("Your {} vanished.\n", item_ref),
                );
            });
        }
    }

    // Remove recall spells from spell slots (20 slots)
    for n in 0..20 {
        let spell_idx = Repository::with_characters(|characters| characters[cn].spell[n]);
        if spell_idx != 0 {
            let is_recall =
                Repository::with_items(|items| items[spell_idx as usize].temp == SK_RECALL as u16);
            if is_recall {
                Repository::with_characters_mut(|characters| {
                    characters[cn].spell[n] = 0;
                });

                Repository::with_items_mut(|items| {
                    items[spell_idx as usize].used = USE_EMPTY;
                });
            }
        }
    }

    // Teleport based on labyrinth progress
    let progress = Repository::with_characters(|characters| characters[cn].data[20]);

    let flag = match progress {
        0 => {
            // TODO: fx_add_effect(6, 0, x, y, 0)
            let result = God::transfer_char(cn, 64, 56);
            // TODO: fx_add_effect(6, 0, x, y, 0)
            result
        }
        1 => {
            // TODO: fx_add_effect(6, 0, x, y, 0)
            let result = God::transfer_char(cn, 95, 207);
            // TODO: fx_add_effect(6, 0, x, y, 0)
            result
        }
        2 => {
            // TODO: fx_add_effect(6, 0, x, y, 0)
            let result = God::transfer_char(cn, 74, 240);
            // TODO: fx_add_effect(6, 0, x, y, 0)
            result
        }
        3 => {
            // TODO: fx_add_effect(6, 0, x, y, 0)
            let result = God::transfer_char(cn, 37, 370);
            // TODO: fx_add_effect(6, 0, x, y, 0)
            result
        }
        4 => {
            // TODO: fx_add_effect(6, 0, x, y, 0)
            let result = God::transfer_char(cn, 114, 390);
            // TODO: fx_add_effect(6, 0, x, y, 0)
            result
        }
        5 => {
            // TODO: fx_add_effect(6, 0, x, y, 0)
            let result = God::transfer_char(cn, 28, 493);
            // TODO: fx_add_effect(6, 0, x, y, 0)
            result
        }
        6 => {
            // TODO: fx_add_effect(6, 0, x, y, 0)
            let result = God::transfer_char(cn, 24, 534);
            // TODO: fx_add_effect(6, 0, x, y, 0)
            result
        }
        7 => {
            // TODO: fx_add_effect(6, 0, x, y, 0)
            let result = God::transfer_char(cn, 118, 667);
            // TODO: fx_add_effect(6, 0, x, y, 0)
            result
        }
        8 => {
            // TODO: fx_add_effect(6, 0, x, y, 0)
            let result = God::transfer_char(cn, 63, 720);
            // TODO: fx_add_effect(6, 0, x, y, 0)
            result
        }
        9 => {
            // TODO: fx_add_effect(6, 0, x, y, 0)
            let result = God::transfer_char(cn, 33, 597);
            // TODO: fx_add_effect(6, 0, x, y, 0)
            result
        }
        _ => {
            State::with(|state| {
                state.do_character_log(
                    cn,
                    core::types::FontColor::Green,
                    "You have already solved all existing parts of the labyrinth. Please come back later.\n",
                );
            });
            false
        }
    };

    // Update temple and tavern coordinates if teleport was successful
    if flag {
        let (x, y) = Repository::with_characters(|characters| (characters[cn].x, characters[cn].y));
        Repository::with_characters_mut(|characters| {
            characters[cn].temple_x = x;
            characters[cn].temple_y = y;
            characters[cn].tavern_x = x;
            characters[cn].tavern_y = y;
        });
    }

    1
}

pub fn use_ladder(cn: usize, item_idx: usize) -> i32 {
    use crate::god::God;
    use crate::repository::Repository;

    // Get item position and offset from data
    let (item_x, item_y, offset_x, offset_y) = Repository::with_items(|items| {
        let item = &items[item_idx];
        (
            item.x as usize,
            item.y as usize,
            item.data[0] as i32,
            item.data[1] as i32,
        )
    });

    // Calculate destination (item position + offset)
    let dest_x = (item_x as i32 + offset_x) as usize;
    let dest_y = (item_y as i32 + offset_y) as usize;

    God::transfer_char(cn, dest_x, dest_y);

    1
}

pub fn use_bag(cn: usize, item_idx: usize) -> i32 {
    use crate::repository::Repository;
    use crate::state::State;

    // Get the character ID stored in the bag's data[0]
    let co = Repository::with_items(|items| items[item_idx].data[0] as usize);

    // Get the owner of the corpse (CHD_CORPSEOWNER = 66)
    let owner = Repository::with_characters(|characters| characters[co].data[66] as usize);

    // Check if grave robbing is allowed
    if owner != 0 && owner != cn {
        let (may_attack, allowed_cn) = State::with(|state| {
            let may_attack = state.may_attack_msg(cn, owner, false);
            let allowed =
                Repository::with_characters(|characters| characters[owner].data[65] as usize);
            (may_attack, allowed)
        });

        if may_attack == 0 && allowed_cn != cn {
            let owner_name = Repository::with_characters(|characters| {
                String::from_utf8_lossy(&characters[owner].name)
                    .trim_end_matches('\0')
                    .to_string()
            });

            // TODO: Implement HIS_HER macro
            State::with(|state| {
                state.do_character_log(
                    cn,
                    core::types::FontColor::Green,
                    &format!(
                        "This is {}'s grave, not yours. You may only search it with their permission.\n",
                        owner_name
                    ),
                );
            });

            // Check if owner is active and notify them
            let (is_active, owner_x) = Repository::with_characters(|characters| {
                (
                    characters[owner].is_living_character(owner),
                    characters[owner].x,
                )
            });

            if is_active && owner_x != 0 {
                let cn_name = Repository::with_characters(|characters| {
                    String::from_utf8_lossy(&characters[cn].name)
                        .trim_end_matches('\0')
                        .to_string()
                });

                State::with(|state| {
                    state.do_character_log(
                        owner,
                        core::types::FontColor::Green,
                        &format!(
                            "{} just tried to search your grave. You must #ALLOW {} if you want them to.\n",
                            cn_name, cn_name
                        ),
                    );
                });
            }

            return 0;
        }
    }

    // Allow the search
    let co_ref = Repository::with_characters(|characters| {
        String::from_utf8_lossy(&characters[co].reference)
            .trim_end_matches('\0')
            .to_string()
    });

    State::with(|state| {
        state.do_character_log(
            cn,
            core::types::FontColor::Yellow,
            &format!("You search the remains of {}.\n", co_ref),
        );
        state.do_look_char(cn, co, 0, 0, 1);
    });

    1
}

pub fn use_scroll(cn: usize, item_idx: usize) -> i32 {
    use crate::god::God;
    use crate::repository::Repository;
    use crate::state::State;
    use core::constants::{MAXSKILL, USE_EMPTY};

    // Get skill number from data[0]
    let (skill_nr, teaches_only) = Repository::with_items(|items| {
        (
            items[item_idx].data[0] as usize,
            items[item_idx].data[1] != 0,
        )
    });

    if skill_nr >= MAXSKILL {
        return 0;
    }

    let (current_val, max_val, difficulty, points_tot) =
        Repository::with_characters(|characters| {
            (
                characters[cn].skill[skill_nr][0],
                characters[cn].skill[skill_nr][2],
                characters[cn].skill[skill_nr][3],
                characters[cn].points_tot,
            )
        });

    if current_val != 0 {
        // Already know the skill
        if teaches_only {
            // TODO: Get skill name from static_skilltab
            State::with(|state| {
                state.do_character_log(
                    cn,
                    core::types::FontColor::Yellow,
                    &format!("You already know skill {}.\\n", skill_nr),
                );
            });
            return 0;
        }

        if current_val >= max_val {
            State::with(|state| {
                state.do_character_log(
                    cn,
                    core::types::FontColor::Yellow,
                    &format!("You cannot raise skill {} any higher.\\n", skill_nr),
                );
            });
            return 0;
        }

        // Raise skill by one
        State::with(|state| {
            state.do_character_log(
                cn,
                core::types::FontColor::Green,
                &format!("Raised skill {} by one.\\n", skill_nr),
            );
        });

        // TODO: Implement skill_needed calculation
        let pts = 100; // Placeholder
        Repository::with_characters_mut(|characters| {
            characters[cn].points_tot += pts;
            characters[cn].skill[skill_nr][0] += 1;
        });

        // TODO: do_check_new_level(cn);
        log::info!("TODO: do_check_new_level({})", cn);
    } else if max_val == 0 {
        // Cannot learn this skill
        State::with(|state| {
            state.do_character_log(
                cn,
                core::types::FontColor::Yellow,
                &format!(
                    "This scroll teaches skill {}, which you cannot learn.\\n",
                    skill_nr
                ),
            );
        });
        return 0;
    } else {
        // Learn the skill
        Repository::with_characters_mut(|characters| {
            characters[cn].skill[skill_nr][0] = 1;
        });
        State::with(|state| {
            state.do_character_log(
                cn,
                core::types::FontColor::Green,
                &format!("You learned skill {}!\\n", skill_nr),
            );
        });
        // TODO: chlog(cn, "Used scroll to learn skill")
    }

    // Consume scroll
    Repository::with_items_mut(|items| {
        items[item_idx].used = USE_EMPTY;
    });
    God::take_item_from_char(item_idx, cn);

    // TODO: do_update_char(cn);

    1
}

pub fn use_scroll2(cn: usize, item_idx: usize) -> i32 {
    use crate::repository::Repository;
    use crate::state::State;
    use core::constants::USE_EMPTY;

    // Get the attribute number from data[0]
    let attrib_nr = Repository::with_items(|items| items[item_idx].data[0] as usize);

    let (current_val, max_val, difficulty) = Repository::with_characters(|characters| {
        (
            characters[cn].attrib[attrib_nr][0],
            characters[cn].attrib[attrib_nr][2],
            characters[cn].attrib[attrib_nr][3],
        )
    });

    if current_val >= max_val {
        // TODO: Get attribute name from at_name array
        State::with(|state| {
            state.do_character_log(
                cn,
                core::types::FontColor::Green,
                &format!("You cannot raise attribute {} any higher.\n", attrib_nr),
            );
        });
        return 0;
    }

    // Calculate points needed: v*v*v*diff/20
    let v = current_val as i32;
    let diff = difficulty as i32;
    let pts = (v * v * v * diff) / 20;

    State::with(|state| {
        state.do_character_log(
            cn,
            core::types::FontColor::Yellow,
            &format!("Raised attribute {} by one.\n", attrib_nr),
        );
    });
    // TODO: Implement chlog

    Repository::with_characters_mut(|characters| {
        characters[cn].points_tot += pts;
        characters[cn].attrib[attrib_nr][0] += 1;
    });

    State::with(|state| {
        state.do_check_new_level(cn);
    });

    // Remove the scroll
    Repository::with_items_mut(|items| {
        items[item_idx].used = USE_EMPTY;
    });

    take_item_from_char(item_idx, cn);

    // Update character
    Repository::with_characters_mut(|characters| {
        characters[cn].set_do_update_flags();
    });

    1
}

pub fn use_scroll3(cn: usize, item_idx: usize) -> i32 {
    use crate::repository::Repository;
    use crate::state::State;
    use core::constants::USE_EMPTY;

    // Get the amount to raise from data[0]
    let amount = Repository::with_items(|items| items[item_idx].data[0] as i32);

    let (current_hp, max_hp, difficulty) = Repository::with_characters(|characters| {
        (
            characters[cn].hp[0],
            characters[cn].hp[2],
            characters[cn].hp[3],
        )
    });

    if current_hp >= max_hp {
        State::with(|state| {
            state.do_character_log(
                cn,
                core::types::FontColor::Green,
                "You cannot raise Hitpoints any higher.\n",
            );
        });
        return 0;
    }

    State::with(|state| {
        state.do_character_log(
            cn,
            core::types::FontColor::Yellow,
            &format!("Raised Hitpoints by {}.\n", amount),
        );
    });
    // TODO: Implement chlog

    // Calculate total points needed: sum of v*diff for each point
    let v = current_hp as i32;
    let diff = difficulty as i32;
    let mut pts = 0;
    for n in 0..amount {
        pts += (n + v) * diff;
    }

    Repository::with_characters_mut(|characters| {
        characters[cn].points_tot += pts;
        characters[cn].hp[0] += amount as i8;
    });

    State::with(|state| {
        state.do_check_new_level(cn);
    });

    // Remove the scroll
    Repository::with_items_mut(|items| {
        items[item_idx].used = USE_EMPTY;
    });

    take_item_from_char(item_idx, cn);

    // Update character
    Repository::with_characters_mut(|characters| {
        characters[cn].set_do_update_flags();
    });

    1
}

pub fn use_scroll4(cn: usize, item_idx: usize) -> i32 {
    use crate::repository::Repository;
    use crate::state::State;
    use core::constants::USE_EMPTY;

    // Get the amount to raise from data[0]
    let amount = Repository::with_items(|items| items[item_idx].data[0] as i32);

    let (current_end, max_end, difficulty) = Repository::with_characters(|characters| {
        (
            characters[cn].end[0],
            characters[cn].end[2],
            characters[cn].end[3],
        )
    });

    if current_end >= max_end {
        State::with(|state| {
            state.do_character_log(
                cn,
                core::types::FontColor::Green,
                "You cannot raise Endurance any higher.\n",
            );
        });
        return 0;
    }

    State::with(|state| {
        state.do_character_log(
            cn,
            core::types::FontColor::Yellow,
            &format!("Raised Endurance by {}.\n", amount),
        );
    });
    // TODO: Implement chlog

    // Calculate total points needed: sum of (v*diff)/2 for each point
    let v = current_end as i32;
    let diff = difficulty as i32;
    let mut pts = 0;
    for n in 0..amount {
        pts += ((n + v) * diff) / 2;
    }

    Repository::with_characters_mut(|characters| {
        characters[cn].points_tot += pts;
        characters[cn].end[0] += amount as i8;
    });

    State::with(|state| {
        state.do_check_new_level(cn);
    });

    // Remove the scroll
    Repository::with_items_mut(|items| {
        items[item_idx].used = USE_EMPTY;
    });

    take_item_from_char(item_idx, cn);

    // Update character
    Repository::with_characters_mut(|characters| {
        characters[cn].set_do_update_flags();
    });

    1
}

pub fn use_scroll5(cn: usize, item_idx: usize) -> i32 {
    use crate::repository::Repository;
    use crate::state::State;
    use core::constants::USE_EMPTY;

    // Get the amount to raise from data[0]
    let amount = Repository::with_items(|items| items[item_idx].data[0] as i32);

    let (current_mana, max_mana, difficulty) = Repository::with_characters(|characters| {
        (
            characters[cn].mana[0],
            characters[cn].mana[2],
            characters[cn].mana[3],
        )
    });

    if current_mana >= max_mana {
        State::with(|state| {
            state.do_character_log(
                cn,
                core::types::FontColor::Green,
                "You cannot raise Mana any higher.\n",
            );
        });
        return 0;
    }

    State::with(|state| {
        state.do_character_log(
            cn,
            core::types::FontColor::Yellow,
            &format!("Raised Mana by {}.\n", amount),
        );
    });
    // TODO: Implement chlog

    // Calculate total points needed: sum of v*diff for each point
    let v = current_mana as i32;
    let diff = difficulty as i32;
    let mut pts = 0;
    for n in 0..amount {
        pts += (n + v) * diff;
    }

    Repository::with_characters_mut(|characters| {
        characters[cn].points_tot += pts;
        characters[cn].mana[0] += amount as i8;
    });

    State::with(|state| {
        state.do_check_new_level(cn);
    });

    // Remove the scroll
    Repository::with_items_mut(|items| {
        items[item_idx].used = USE_EMPTY;
    });

    take_item_from_char(item_idx, cn);

    // Update character
    Repository::with_characters_mut(|characters| {
        characters[cn].set_do_update_flags();
    });

    1
}

pub fn use_crystal_sub(cn: usize, item_idx: usize) -> i32 {
    // This is a complex function that creates random dungeon NPCs
    // For now, just return a placeholder
    // TODO: Full implementation requires pop_create_char, god_create_char, and equipment system
    log::warn!("use_crystal_sub not fully implemented yet");
    0
}

pub fn use_crystal(cn: usize, item_idx: usize) -> i32 {
    let mut cnt = 0;

    // Call use_crystal_sub until it returns <= 4, up to 4 times
    while use_crystal_sub(cn, item_idx) > 4 && cnt < 4 {
        cnt += 1;
    }

    if cnt == 0 {
        1
    } else {
        0
    }
}

pub fn use_mine_respawn(cn: usize, item_idx: usize) -> i32 {
    use crate::repository::Repository;

    // Get group, template, and max count from item data
    let (group, template, max_cnt) = Repository::with_items(|items| {
        let item = &items[item_idx];
        (
            item.data[0] as i32,
            item.data[1] as usize,
            item.data[2] as i32,
        )
    });

    // Check if mine wall items exist (data[3-9])
    for n in 3..10 {
        let map_idx = Repository::with_items(|items| items[item_idx].data[n]);
        if map_idx == 0 {
            break;
        }

        // Check if there's a mine wall item at that location
        // TODO: Check map[m].it and verify driver == 26
        // For now, skip this validation
    }

    // Count active NPCs in this group
    let cnt = Repository::with_characters(|characters| {
        let mut count = 0;
        for n in 1..core::constants::MAXCHARS {
            if characters[n].used == core::constants::USE_ACTIVE
                && (characters[n].flags & 0x00000001) == 0 // !CF_BODY
                && characters[n].data[42] == group
            {
                count += 1;
            }
        }
        count
    });

    // Don't spawn if too many NPCs already
    if cnt > max_cnt {
        return 0;
    }

    // TODO: Implement pop_create_char and god_drop_char_fuzzy
    log::warn!("use_mine_respawn: pop_create_char not implemented yet");

    1
}

pub fn rat_eye(cn: usize, item_idx: usize) -> i32 {
    use crate::god::God;
    use crate::repository::Repository;
    use crate::state::State;
    use core::constants::{ItemFlags, USE_EMPTY};

    if cn == 0 {
        return 0;
    }

    let citem = Repository::with_characters(|characters| characters[cn].citem);

    if citem == 0 || (citem & 0x80000000) != 0 {
        State::with(|state| {
            state.do_character_log(
                cn,
                core::types::FontColor::Green,
                "What do you want to do with it?\\n",
            );
        });
        return 0;
    }

    // Check if rat eye is carried (not on ground)
    let carried = Repository::with_items(|items| items[item_idx].carried);
    if carried == 0 {
        State::with(|state| {
            state.do_character_log(
                cn,
                core::types::FontColor::Green,
                "Too difficult to do on the ground.\\n",
            );
        });
        return 0;
    }

    // Check if citem matches any of the required templates in data[0-8]
    let citem_temp = Repository::with_items(|items| items[citem as usize].temp);

    let mut slot = None;
    for n in 0..9 {
        let required_temp = Repository::with_items(|items| items[item_idx].data[n] as u16);
        if required_temp != 0 && required_temp == citem_temp {
            slot = Some(n);
            break;
        }
    }

    let slot = match slot {
        Some(s) => s,
        None => {
            State::with(|state| {
                state.do_character_log(cn, core::types::FontColor::Green, "This doesnt fit.\\n");
            });
            return 0;
        }
    };

    // TODO: Implement chlog
    log::info!("Character {} added item to rat eye", cn);

    // Mark the slot as filled
    Repository::with_items_mut(|items| {
        items[item_idx].data[slot] = 0;
        items[item_idx].sprite[0] += 1;
        items[item_idx].flags |= ItemFlags::IF_UPDATE.bits();
        items[item_idx].temp = 0;
    });

    // Remove the citem
    Repository::with_items_mut(|items| {
        items[citem as usize].used = USE_EMPTY;
    });
    Repository::with_characters_mut(|characters| {
        characters[cn].citem = 0;
    });

    // Check if all slots are filled
    let all_filled = Repository::with_items(|items| {
        for n in 0..9 {
            if items[item_idx].data[n] != 0 {
                return false;
            }
        }
        true
    });

    if all_filled {
        // Create the final item from data[9]
        let result_template = Repository::with_items(|items| items[item_idx].data[9] as usize);

        let in3 = match God::create_item(result_template) {
            Some(id) => id,
            None => return 1,
        };

        Repository::with_items_mut(|items| {
            items[in3].flags |= ItemFlags::IF_UPDATE.bits();
        });

        // Remove the rat eye item
        take_item_from_char(item_idx, cn);
        Repository::with_items_mut(|items| {
            items[item_idx].used = USE_EMPTY;
        });

        // Give the completed item to the character
        God::give_character_item(cn, in3);
    }

    1
}

pub fn skua_protect(cn: usize, item_idx: usize) -> i32 {
    use crate::repository::Repository;
    use crate::state::State;
    use core::constants::WN_RHAND;

    // Check if the weapon is wielded
    let is_wielded =
        Repository::with_characters(|characters| characters[cn].worn[WN_RHAND] == item_idx as u32);

    if !is_wielded {
        State::with(|state| {
            state.do_character_log(
                cn,
                core::types::FontColor::Green,
                "You cannot use Skua's weapon if you're not wielding it.\\n",
            );
        });
        return 0;
    }

    // Check if character has Skua's kindred (KIN_SKUA = 0x00000002)
    let has_skua_kindred =
        Repository::with_characters(|characters| (characters[cn].kindred & 0x00000002) != 0);

    if !has_skua_kindred {
        State::with(|state| {
            state.do_character_log(
                cn,
                core::types::FontColor::Green,
                "How dare you to call on Skua to help you? Slave of the Purple One!\\n",
            );
            state.do_character_log(
                cn,
                core::types::FontColor::Green,
                "Your weapon vanished.\\n",
            );
        });

        Repository::with_characters_mut(|characters| {
            characters[cn].worn[WN_RHAND] = 0;
        });

        Repository::with_items_mut(|items| {
            items[item_idx].used = core::constants::USE_EMPTY;
        });
    } else {
        State::with(|state| {
            state.do_character_log(
                cn,
                core::types::FontColor::Green,
                "You feel Skua's presence protect you.\n",
            );
            state.do_character_log(
                cn,
                core::types::FontColor::Green,
                "He takes away His weapon and replaces it by a common one.\n",
            );
        });

        // TODO: Implement spell_from_item(cn, item_idx)

        // Get replacement weapon template from data[2]
        let replacement_template = Repository::with_items(|items| items[item_idx].data[2] as usize);

        // Remove the Skua weapon
        Repository::with_items_mut(|items| {
            items[item_idx].used = core::constants::USE_EMPTY;
        });

        // Create replacement weapon
        if let Some(new_weapon) = crate::god::God::create_item(replacement_template) {
            Repository::with_items_mut(|items| {
                items[new_weapon].carried = cn as u16;
                items[new_weapon].flags |= core::constants::ItemFlags::IF_UPDATE.bits();
            });

            Repository::with_characters_mut(|characters| {
                characters[cn].worn[WN_RHAND] = new_weapon as u32;
            });
        }
    }

    1
}

pub fn purple_protect(cn: usize, item_idx: usize) -> i32 {
    use crate::repository::Repository;
    use crate::state::State;
    use core::constants::WN_RHAND;

    // Check if the weapon is wielded
    let is_wielded =
        Repository::with_characters(|characters| characters[cn].worn[WN_RHAND] == item_idx as u32);

    if !is_wielded {
        State::with(|state| {
            state.do_character_log(
                cn,
                core::types::FontColor::Green,
                "You cannot use the Purple One's weapon if you're not wielding it.\n",
            );
        });
        return 0;
    }

    // Check if character has Purple One's kindred (KIN_PURPLE = 0x00000001)
    let has_purple_kindred =
        Repository::with_characters(|characters| (characters[cn].kindred & 0x00000001) != 0);

    if !has_purple_kindred {
        State::with(|state| {
            state.do_character_log(
                cn,
                core::types::FontColor::Green,
                "How dare you to call on the Purple One to help you? Slave of Skua!\n",
            );
            state.do_character_log(
                cn,
                core::types::FontColor::Green,
                "Your weapon vanished.\\n",
            );
        });

        Repository::with_characters_mut(|characters| {
            characters[cn].worn[WN_RHAND] = 0;
        });

        Repository::with_items_mut(|items| {
            items[item_idx].used = core::constants::USE_EMPTY;
        });
    } else {
        State::with(|state| {
            state.do_character_log(
                cn,
                core::types::FontColor::Green,
                "You feel the Purple One's presence protect you.\n",
            );
            state.do_character_log(
                cn,
                core::types::FontColor::Green,
                "He takes away His weapon and replaces it by a common one.\\n",
            );
        });

        // TODO: Implement spell_from_item(cn, item_idx)

        // Get replacement weapon template from data[2]
        let replacement_template = Repository::with_items(|items| items[item_idx].data[2] as usize);

        // Remove the Purple One's weapon
        Repository::with_items_mut(|items| {
            items[item_idx].used = core::constants::USE_EMPTY;
        });

        // Create replacement weapon
        if let Some(new_weapon) = crate::god::God::create_item(replacement_template) {
            Repository::with_items_mut(|items| {
                items[new_weapon].carried = cn as u16;
                items[new_weapon].flags |= core::constants::ItemFlags::IF_UPDATE.bits();
            });

            Repository::with_characters_mut(|characters| {
                characters[cn].worn[WN_RHAND] = new_weapon as u32;
            });
        }
    }

    1
}

pub fn use_lever(cn: usize, item_idx: usize) -> i32 {
    use crate::repository::Repository;

    // Get the map coordinate from item data[0]
    let m = Repository::with_items(|items| items[item_idx].data[0] as usize);

    // Get the item at that map location
    let in2 = Repository::with_map(|map| map[m].item);

    if in2 == 0 {
        return 0;
    }

    // Check if the item is already active
    let is_active = Repository::with_items(|items| items[in2 as usize].active != 0);
    if is_active {
        return 0;
    }

    // Activate the linked item
    use_driver(0, in2 as usize, false);

    // Set active to duration and handle lighting changes
    Repository::with_items_mut(|items| {
        let item = &mut items[in2 as usize];
        item.active = item.duration as u32;

        // TODO: Implement do_add_light if light[0] != light[1]
        // if item.light[0] != item.light[1] {
        //     do_add_light(item.x, item.y, item.light[1] - item.light[0]);
        // }
    });

    1
}

pub fn use_spawn(cn: usize, item_idx: usize) -> i32 {
    use crate::repository::Repository;
    use core::constants::USE_EMPTY;

    // Check if already active
    let is_active = Repository::with_items(|items| items[item_idx].active != 0);
    if is_active {
        return 0;
    }

    // Check if player needs to provide an item (data[1])
    if cn != 0 {
        let required_template = Repository::with_items(|items| items[item_idx].data[1] as usize);

        if required_template != 0 {
            let citem = Repository::with_characters(|characters| characters[cn].citem);

            if citem == 0 || (citem & 0x80000000) != 0 {
                return 0;
            }

            let citem_template = Repository::with_items(|items| items[citem as usize].temp);
            if citem_template != required_template {
                return 0;
            }

            // Remove the required item
            Repository::with_items_mut(|items| {
                items[citem as usize].used = USE_EMPTY;
            });
            Repository::with_characters_mut(|characters| {
                characters[cn].citem = 0;
            });
        }
    }

    // Add effect if data[2] contains a character template
    let temp = Repository::with_items(|items| items[item_idx].data[2] as usize);
    if temp != 0 {
        // TODO: Implement fx_add_effect and ch_temp access
        // fx_add_effect(2, TICKS * 10, ch_temp[temp].x, ch_temp[temp].y, temp);
        log::info!("use_spawn: would add effect for template {}", temp);
    }

    1
}

pub fn use_pile(cn: usize, item_idx: usize) -> i32 {
    use crate::god::God;
    use crate::repository::Repository;
    use crate::state::State;
    use core::constants::{ItemFlags, USE_EMPTY};

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
    let is_active = Repository::with_items(|items| items[item_idx].active != 0);
    if is_active {
        return 0;
    }

    // Get pile info
    let (x, y, level) = Repository::with_items(|items| {
        (
            items[item_idx].x,
            items[item_idx].y,
            items[item_idx].data[0] as i32,
        )
    });

    // Destroy this object
    Repository::with_items_mut(|items| {
        items[item_idx].used = USE_EMPTY;
    });

    let m = (x as usize) + (y as usize) * core::constants::MAPX;
    Repository::with_map_mut(|map| {
        map[m].item = 0;
    });

    // Calculate chance based on player's luck
    let luck = Repository::with_characters(|characters| characters[cn].points.luck);

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
    if rand::random::<u32>() % chance != 0 {
        return 1; // Nothing found
    }

    // Determine what to give based on level
    let tmp_idx = (rand::random::<u32>() % 30) as usize + (level as usize * 30);
    let tmp_idx = tmp_idx.min(89); // Clamp to valid range
    let tmp = FIND[tmp_idx];

    // Create item
    if let Some(in2) = God::create_item(tmp) {
        let is_takeable =
            Repository::with_items(|items| items[in2].flags.contains(ItemFlags::IF_TAKE));

        if is_takeable {
            // Give to player
            if God::give_character_item(cn, in2) {
                let reference = Repository::with_items(|items| items[in2].reference.clone());
                State::with(|state| {
                    state.do_character_log(
                        cn,
                        core::types::FontColor::Green,
                        &format!("You've found a {}!\n", reference),
                    );
                });
            }
        } else {
            // It's a monster spawner
            God::drop_item(in2, x, y);
            // TODO: Implement fx_add_effect(9, 16, in2, items[in2].data[0], 0);
            log::info!("use_pile: spawning monster at ({}, {})", x, y);
        }
    }

    1
}

pub fn use_grave(cn: usize, item_idx: usize) -> i32 {
    use crate::repository::Repository;
    use core::constants::USE_EMPTY;

    // Get previously spawned character
    let cc = Repository::with_items(|items| items[item_idx].data[0] as usize);

    // Check if still alive and linked
    if cc > 0 {
        let is_alive = Repository::with_characters(|characters| {
            if cc >= characters.len() {
                return false;
            }
            let ch = &characters[cc];
            ch.data[0] as usize == item_idx
                && ch.flags & core::constants::CF_BODY == 0
                && ch.used != USE_EMPTY
        });

        if is_alive {
            return 1; // Still alive, don't spawn new one
        }
    }

    // TODO: Implement pop_create_char and god_drop_char_fuzzy
    // cc = pop_create_char(328, 0);
    // if !god_drop_char_fuzzy(cc, x, y) {
    //     god_destroy_items(cc);
    //     characters[cc].used = USE_EMPTY;
    //     return 1;
    // }
    //
    // Create link between item and character
    // characters[cc].data[0] = item_idx as i32;
    // items[item_idx].data[0] = cc as i32;

    log::warn!("use_grave: pop_create_char not implemented yet");

    1
}

pub fn mine_wall(cn: usize, item_idx: usize) -> i32 {
    use crate::repository::Repository;
    use core::constants::ItemFlags;

    // If no item provided, get it from the map
    let in_idx = if item_idx == 0 {
        let (x, y) = Repository::with_characters(|characters| {
            if cn == 0 {
                (0, 0)
            } else {
                (characters[cn].x as usize, characters[cn].y as usize)
            }
        });
        let m = x + y * core::constants::MAPX;
        let map_item = Repository::with_map(|map| map[m].item);
        if map_item == 0 {
            return 0;
        }
        map_item as usize
    } else {
        item_idx
    };

    // Add rebuild wall effect if data[3] is set
    let should_rebuild = Repository::with_items(|items| items[in_idx].data[3] != 0);
    if should_rebuild {
        // TODO: Implement fx_add_effect(10, TICKS * 60 * 15, x, y, temp);
        log::info!("mine_wall: would add rebuild effect");
    }

    // Get original template, position, and carried status
    let (temp, x, y, carried) = Repository::with_items(|items| {
        (
            items[in_idx].data[0] as usize,
            items[in_idx].x,
            items[in_idx].y,
            items[in_idx].carried,
        )
    });

    // Transform the item back to its original state from template
    // TODO: This requires item templates (it_temp) to be implemented
    // For now, just reset key fields
    let result_data_2 = Repository::with_items_mut(|items| {
        let item = &mut items[in_idx];
        // In the full implementation, we'd copy from it_temp[temp]
        // For now, preserve position and carried status
        item.x = x;
        item.y = y;
        item.carried = carried;
        item.temp = temp;
        if carried != 0 {
            item.flags |= ItemFlags::IF_UPDATE.bits();
        }
        item.data[2]
    });

    result_data_2
}

pub fn mine_state(cn: usize, item_idx: usize) -> i32 {
    use crate::repository::Repository;
    use core::constants::SERVER_MAPX;

    if item_idx == 0 {
        return 0;
    }

    // Check if item is a mine wall (driver 25)
    let is_mine_wall = Repository::with_items(|items| items[item_idx].driver == 25);
    if !is_mine_wall {
        return 0;
    }

    // Return state from data[2]
    Repository::with_items(|items| items[item_idx].data[2])
}

pub fn use_mine(cn: usize, item_idx: usize) -> i32 {
    use crate::repository::Repository;
    use crate::state::State;
    use core::constants::{AT_STREN, WN_RHAND};

    // Get character strength
    let mut str = Repository::with_characters(|characters| {
        characters[cn].attrib[AT_STREN as usize][5] as i32
    });

    // Check and subtract endurance
    let insufficient_endurance = Repository::with_characters_mut(|characters| {
        if characters[cn].a_end < 1500 {
            true
        } else {
            characters[cn].a_end -= 1000;
            false
        }
    });

    if insufficient_endurance {
        State::with(|state| {
            state.do_character_log(
                cn,
                core::types::FontColor::Green,
                "You're too exhausted to continue digging.\\n",
            );
        });
        Repository::with_characters_mut(|characters| {
            characters[cn].misc_action = 0; // DR_IDLE
        });
        return 0;
    }

    // Check for proper tools in right hand
    let (has_pickaxe, has_weapon) = Repository::with_characters(|characters| {
        let in2 = characters[cn].worn[WN_RHAND] as usize;
        if in2 != 0 {
            Repository::with_items(|items| {
                let temp = items[in2].temp;
                (temp == 458, true) // 458 is pickaxe
            })
        } else {
            (false, false)
        }
    });

    if has_weapon {
        if has_pickaxe {
            item_damage_weapon(cn, str / 10);
            str *= 2;
        } else {
            item_damage_weapon(cn, str * 10);
            str /= 4;
        }
        // TODO: Implement char_play_sound and do_area_sound
        // State::char_play_sound(cn, 11, -150, 0);
        // State::do_area_sound(cn, 0, characters[cn].x, characters[cn].y, 11);
    } else {
        str /= 10;
        let low_health = Repository::with_characters_mut(|characters| {
            if characters[cn].a_hp < 10000 {
                true
            } else {
                characters[cn].a_hp -= 500;
                false
            }
        });

        if low_health {
            State::with(|state| {
                state.do_character_log(
                    cn,
                    core::types::FontColor::Green,
                    "You don't want to kill yourself beating at this wall with your bare hands, so you stop.\\n",
                );
            });
            Repository::with_characters_mut(|characters| {
                characters[cn].misc_action = 0; // DR_IDLE
            });
            return 0;
        }
    }

    // Apply damage to mine wall
    let tmp = Repository::with_items_mut(|items| {
        let new_val = items[item_idx].data[1] - str;
        items[item_idx].data[1] = new_val;
        new_val
    });

    if tmp <= 0 {
        // Wall destroyed
        let (x, y) = Repository::with_items(|items| (items[item_idx].x, items[item_idx].y));
        State::with(|state| {
            state.reset_go(x as i32, y as i32);
            state.remove_lights(x as i32, y as i32);
        });

        let _result = mine_wall(cn, item_idx);

        State::with(|state| {
            state.reset_go(x as i32, y as i32);
            state.add_lights(x as i32, y as i32);
        });
    }

    0
}

pub fn use_mine_fast(cn: usize, item_idx: usize) -> i32 {
    use crate::repository::Repository;
    use crate::state::State;
    use core::constants::{SERVER_MAPX, USE_EMPTY};

    if cn == 0 {
        return 0;
    }

    let carried = Repository::with_items(|items| items[item_idx].carried);
    if carried != 0 {
        return 0;
    }

    // Get item position and template
    let (x, y, temp) = Repository::with_items(|items| {
        (items[item_idx].x, items[item_idx].y, items[item_idx].temp)
    });

    // TODO: Implement fx_add_effect
    // fx_add_effect(10, TICKS * 60 * 15, x, y, temp);

    State::with(|state| {
        state.reset_go(x as i32, y as i32);
        state.remove_lights(x as i32, y as i32);
    });

    // Remove item from map
    Repository::with_map_mut(|map| {
        map[(x + y * SERVER_MAPX) as usize].it = 0;
    });

    Repository::with_items_mut(|items| {
        items[item_idx].used = USE_EMPTY;
    });

    State::with(|state| {
        state.reset_go(x as i32, y as i32);
        state.add_lights(x as i32, y as i32);
    });

    1
}

pub fn build_ring(cn: usize, item_idx: usize) -> i32 {
    use crate::god::God;
    use crate::repository::Repository;
    use core::constants::{ItemFlags, USE_EMPTY};

    // Get ring base template
    let t1 = Repository::with_items(|items| items[item_idx].temp);

    // Get citem template
    let (in2, t2) = Repository::with_characters(|characters| {
        let in2 = characters[cn].citem as usize;
        if in2 == 0 {
            (0, 0)
        } else {
            let t2 = Repository::with_items(|items| items[in2].temp);
            (in2, t2)
        }
    });

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
            487 | 488 | 489 => {
                // Huge gems too powerful for silver
                crate::state::State::with(|state| {
                    state.do_character_log(
                        cn,
                        core::types::FontColor::Green,
                        "This stone is too powerful for a silver ring.\\n",
                    );
                });
                return 0;
            }
            _ => return 0,
        }
    } else {
        return 0;
    };

    // Create result item
    if let Some(in3) = God::create_item(r) {
        Repository::with_items_mut(|items| {
            items[in3].flags |= ItemFlags::IF_UPDATE.bits();
        });

        // Remove gem if used
        if in2 != 0 {
            Repository::with_characters_mut(|characters| {
                characters[cn].citem = 0;
            });
            Repository::with_items_mut(|items| {
                items[in2].used = USE_EMPTY;
            });
        }

        // Remove ring base
        take_item_from_char(item_idx, cn);
        Repository::with_items_mut(|items| {
            items[item_idx].used = USE_EMPTY;
        });

        // Give result to character
        God::give_character_item(cn, in3);

        return 1;
    }

    0
}

pub fn build_amulet(cn: usize, item_idx: usize) -> i32 {
    use crate::god::God;
    use crate::repository::Repository;
    use crate::state::State;
    use core::constants::{ItemFlags, USE_EMPTY};

    // Get amulet piece template
    let t1 = Repository::with_items(|items| items[item_idx].temp);

    // Get citem
    let (in2, t2) = Repository::with_characters(|characters| {
        let in2 = characters[cn].citem as usize;
        (in2, in2)
    });

    if in2 == 0 || (in2 & 0x80000000) != 0 {
        State::with(|state| {
            state.do_character_log(cn, core::types::FontColor::Yellow, "Nothing happens.\\n");
        });
        return 0;
    }

    let t2 = Repository::with_items(|items| items[in2].temp);

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
        State::with(|state| {
            state.do_character_log(cn, core::types::FontColor::Yellow, "That doesn't fit.\\n");
        });
        return 0;
    };

    // Create result item
    if let Some(in3) = God::create_item(r) {
        Repository::with_items_mut(|items| {
            items[in3].flags |= ItemFlags::IF_UPDATE.bits();
        });

        // Remove components
        Repository::with_characters_mut(|characters| {
            characters[cn].citem = 0;
        });
        Repository::with_items_mut(|items| {
            items[in2].used = USE_EMPTY;
        });

        take_item_from_char(item_idx, cn);
        Repository::with_items_mut(|items| {
            items[item_idx].used = USE_EMPTY;
        });

        // Give result to character
        God::give_character_item(cn, in3);

        return 1;
    }

    0
}

pub fn use_gargoyle(cn: usize, item_idx: usize) -> i32 {
    use crate::god::God;
    use crate::repository::Repository;
    use crate::state::State;
    use core::constants::USE_EMPTY;

    if cn == 0 {
        return 0;
    }

    let carried = Repository::with_items(|items| items[item_idx].carried);
    if carried == 0 {
        return 0;
    }

    // Create gargoyle character (template 325)
    let cc = match God::create_char(325, true) {
        Some(cc) => cc as usize,
        None => return 0,
    };

    // Get character position
    let (ch_x, ch_y) =
        Repository::with_characters(|characters| (characters[cn].x, characters[cn].y));

    // Try to drop near character
    if !God::drop_char_fuzzy(cc, ch_x as usize, ch_y as usize) {
        Repository::with_characters_mut(|characters| {
            characters[cc].used = USE_EMPTY;
        });
        God::destroy_items(cc);
        State::with(|state| {
            state.do_character_log(
                cn,
                core::types::FontColor::Green,
                "The Gargoyle could not materialize.\\n",
            );
        });
        return 0;
    }

    // Remove item
    take_item_from_char(item_idx, cn);
    Repository::with_items_mut(|items| {
        items[item_idx].used = USE_EMPTY;
    });

    // Configure gargoyle
    Repository::with_characters_mut(|characters| {
        characters[cc].data[42] = 65536 + cn as i32; // set group
        characters[cc].data[59] = 65536 + cn as i32; // protect all members
        characters[cc].data[63] = cn as i32; // obey and protect char
        characters[cc].data[69] = cn as i32; // follow char
                                             // TODO: Set self destruction timer with globs->ticker + (TICKS * 60 * 15)
                                             // characters[cc].data[64] = globs->ticker + (TICKS * 60 * 15);
    });

    1
}

pub fn use_grolm(cn: usize, item_idx: usize) -> i32 {
    use crate::god::God;
    use crate::repository::Repository;
    use crate::state::State;
    use core::constants::USE_EMPTY;

    if cn == 0 {
        return 0;
    }

    let carried = Repository::with_items(|items| items[item_idx].carried);
    if carried == 0 {
        return 0;
    }

    // Create grolm character (template 577)
    let cc = match God::create_char(577, true) {
        Some(cc) => cc as usize,
        None => return 0,
    };

    // Get character position
    let (ch_x, ch_y) =
        Repository::with_characters(|characters| (characters[cn].x, characters[cn].y));

    // Try to drop near character
    if !God::drop_char_fuzzy(cc, ch_x as usize, ch_y as usize) {
        Repository::with_characters_mut(|characters| {
            characters[cc].used = USE_EMPTY;
        });
        God::destroy_items(cc);
        State::with(|state| {
            state.do_character_log(
                cn,
                core::types::FontColor::Green,
                "The Grolm could not materialize.\\n",
            );
        });
        return 0;
    }

    // Remove item
    take_item_from_char(item_idx, cn);
    Repository::with_items_mut(|items| {
        items[item_idx].used = USE_EMPTY;
    });

    // Configure grolm
    Repository::with_characters_mut(|characters| {
        characters[cc].data[42] = 65536 + cn as i32; // set group
        characters[cc].data[59] = 65536 + cn as i32; // protect all members
        characters[cc].data[63] = cn as i32; // obey and protect char
        characters[cc].data[69] = cn as i32; // follow char
                                             // TODO: Set self destruction timer with globs->ticker + (TICKS * 60 * 15)
                                             // characters[cc].data[64] = globs->ticker + (TICKS * 60 * 15);
    });

    1
}

pub fn boost_char(cn: usize, divi: usize) -> i32 {
    use crate::god::God;
    use crate::helpers::points2rank;
    use crate::repository::Repository;
    use core::constants::MAXSKILL;

    // Boost attributes
    Repository::with_characters_mut(|characters| {
        for n in 0..5 {
            if characters[cn].attrib[n][0] as i32 > divi as i32 {
                let boost = rand::random::<u8>() % (characters[cn].attrib[n][0] / divi as u8);
                characters[cn].attrib[n][0] = characters[cn].attrib[n][0].saturating_add(boost);
            }
        }

        // Boost skills
        for n in 0..MAXSKILL {
            if characters[cn].skill[n][0] as i32 > divi as i32 {
                let boost = rand::random::<u8>() % (characters[cn].skill[n][0] / divi as u8);
                characters[cn].skill[n][0] = characters[cn].skill[n][0].saturating_add(boost);
            }
        }

        // Update name
        let old_name = characters[cn].name.clone();
        let new_name = format!("Strong {}", old_name);
        characters[cn].name = new_name[..39.min(new_name.len())].to_string();
        characters[cn].reference = characters[cn].name.clone();
    });

    // Create soulstone
    if let Some(in_idx) = God::create_item(1146) {
        let (exp, rank) = Repository::with_characters(|characters| {
            let exp = characters[cn].points_tot as u32 / 10
                + (rand::random::<u32>() % (characters[cn].points_tot as u32 / 20 + 1));
            let rank = points2rank(exp);
            (exp, rank)
        });

        Repository::with_items_mut(|items| {
            items[in_idx].name = "Soulstone".to_string();
            items[in_idx].reference = "soulstone".to_string();
            items[in_idx].description = format!("Level {} soulstone, holding {} exp.", rank, exp);
            items[in_idx].data[0] = rank as i32;
            items[in_idx].data[1] = exp as i32;
            items[in_idx].temp = 0;
            items[in_idx].driver = 68;
        });

        God::give_character_item(cn, in_idx);
    }

    0
}

pub fn spawn_penta_enemy(item_idx: usize) -> i32 {
    use crate::god::God;
    use crate::populate::pop_create_char;
    use crate::repository::Repository;
    use core::constants::{CharacterFlags, USE_EMPTY};

    // Determine enemy type from data[9]
    let data9 = Repository::with_items(|items| items[item_idx].data[9]);

    let mut tmp = if data9 == 10 {
        (rand::random::<i32>() % 2) + 9
    } else if data9 == 11 {
        (rand::random::<i32>() % 2) + 11
    } else if data9 == 17 {
        (rand::random::<i32>() % 2) + 17
    } else if data9 == 18 {
        (rand::random::<i32>() % 2) + 18
    } else if data9 == 21 {
        22
    } else if data9 == 22 {
        23
    } else if data9 == 23 {
        24
    } else {
        (rand::random::<i32>() % 3) - 1 + data9
    };

    if tmp < 0 {
        tmp = 0;
    }

    // Create appropriate character template
    let cn = if tmp >= 22 {
        tmp -= 22;
        if tmp > 3 {
            tmp = 3;
        }
        pop_create_char((1094 + tmp) as usize, false)
    } else if tmp > 17 {
        tmp -= 17;
        if tmp > 4 {
            tmp = 4;
        }
        pop_create_char((538 + tmp) as usize, false)
    } else {
        pop_create_char((364 + tmp) as usize, false)
    };

    if cn == 0 {
        return 0;
    }

    // Configure character
    Repository::with_characters_mut(|characters| {
        characters[cn].flags &= !CharacterFlags::CF_RESPAWN.bits();
    });

    let (x, y) = Repository::with_items(|items| (items[item_idx].x, items[item_idx].y));

    Repository::with_characters_mut(|characters| {
        characters[cn].data[0] = item_idx as i32;
        characters[cn].data[29] = (x + y * core::constants::SERVER_MAPX) as i32;
        characters[cn].data[60] = 60 * 60 * 2; // TICKS * 60 * 2
        characters[cn].data[73] = 8;
        characters[cn].dir = 1;
    });

    // Randomly boost character (1 in 25 chance)
    if (rand::random::<i32>() % 25) == 0 {
        boost_char(cn, 5);
    }

    // Try to drop character
    if !God::drop_char_fuzzy(cn, x as usize, y as usize) {
        God::destroy_items(cn);
        Repository::with_characters_mut(|characters| {
            characters[cn].used = USE_EMPTY;
        });
        return 0;
    }

    cn as i32
}

pub fn solved_pentagram(cn: usize, item_idx: usize) -> i32 {
    use crate::repository::Repository;
    use crate::state::State;
    use core::constants::{CharacterFlags, MAXCHARS};

    // Calculate bonus
    let bonus = Repository::with_items(|items| {
        let data0 = items[item_idx].data[0];
        (data0 * data0 * 3) / 7 + 1
    });

    // Add bonus to character's pending exp
    Repository::with_characters_mut(|characters| {
        characters[cn].data[18] += bonus;
    });

    // Log to character
    State::with(|state| {
        state.do_character_log(
            cn,
            core::types::FontColor::Green,
            &format!(
                "You solved the pentagram quest. Congratulations! You will get {} bonus experience points.\\n",
                bonus
            ),
        );
    });

    // TODO: Implement chlog
    log::info!("Character {} solved pentagram quest", cn);

    let cn_name = Repository::with_characters(|characters| characters[cn].name.clone());

    // Notify all players and award pending exp
    for n in 1..MAXCHARS {
        let (used, flags, active, has_bonus) = Repository::with_characters(|characters| {
            if n >= characters.len() {
                return (0, 0, 0, 0);
            }
            (
                characters[n].used,
                characters[n].flags,
                if characters[n].used == core::constants::USE_ACTIVE {
                    1
                } else {
                    0
                },
                characters[n].data[18],
            )
        });

        if used == core::constants::USE_EMPTY {
            continue;
        }
        if (flags & (CharacterFlags::CF_PLAYER.bits() | CharacterFlags::CF_USURP.bits())) == 0 {
            continue;
        }

        // Notify other active players
        if active != 0 && n != cn {
            State::with(|state| {
                state.do_character_log(
                    n,
                    core::types::FontColor::Green,
                    &format!("{}solved the pentagram quest!\\n", cn_name),
                );
            });
        }

        // Award pending bonus exp
        if has_bonus != 0 {
            // TODO: Implement do_give_exp
            log::info!("TODO: do_give_exp({}, {}, 0, -1)", n, has_bonus);
            Repository::with_characters_mut(|characters| {
                characters[n].data[18] = 0;
            });
        }
    }

    // Activate all pentagram items (driver 33)
    Repository::with_items_mut(|items| {
        for n in 1..items.len() {
            if items[n].used == core::constants::USE_EMPTY {
                continue;
            }
            if items[n].driver != 33 {
                continue;
            }
            if items[n].active == 0 {
                if items[n].light[0] != items[n].light[1] && items[n].x > 0 {
                    // TODO: Implement do_add_light
                    log::info!(
                        "TODO: do_add_light({}, {}, {})",
                        items[n].x,
                        items[n].y,
                        items[n].light[1] - items[n].light[0]
                    );
                }
            }
            items[n].duration = 10 * 60 + (rand::random::<i32>() % (20 * 60));
            items[n].active = items[n].duration;
        }
    });

    // TODO: Update penta_needed based on active players
    log::info!("TODO: Update penta_needed calculation");

    0
}

pub fn is_in_pentagram_quest(cn: usize) -> bool {
    use crate::repository::Repository;
    use core::constants::MAXCHARS;

    if cn < 1 || cn >= MAXCHARS {
        return false;
    }

    // Pentagram quest area indices: 67, 68, 110, 113, 123
    // TODO: These should be loaded from area configuration
    // For now, check approximate coordinates based on C++ area.cpp
    let (x, y) = Repository::with_characters(|characters| (characters[cn].x, characters[cn].y));

    // Pentagram quest areas (approximate from C++ area.cpp)
    let areas = [
        (67, 0, 0, 0, 0),          // TODO: Fill in actual coordinates
        (68, 0, 0, 0, 0),          // TODO: Fill in actual coordinates
        (110, 0, 0, 0, 0),         // TODO: Fill in actual coordinates
        (113, 0, 0, 0, 0),         // TODO: Fill in actual coordinates
        (123, 469, 457, 473, 459), // Pentagram Quest area
    ];

    for (_idx, x1, y1, x2, y2) in areas.iter() {
        if x >= *x1 as u16 && y >= *y1 as u16 && x <= *x2 as u16 && y <= *y2 as u16 {
            return true;
        }
    }

    false
}

pub fn use_pentagram(cn: usize, item_idx: usize) -> i32 {
    use crate::helpers::points2rank;
    use crate::repository::Repository;
    use crate::state::State;
    use core::constants::{CharacterFlags, MAXITEM, USE_EMPTY};

    // Check if already active
    let active = Repository::with_items(|items| items[item_idx].active);
    if active != 0 {
        if cn != 0 {
            State::with(|state| {
                state.do_character_log(
                    cn,
                    core::types::FontColor::Green,
                    "This pentagram is already active.\\n",
                );
            });
        } else {
            // Respawn enemies if needed
            for m in 1..4 {
                let (co, needs_spawn) = Repository::with_items(|items| {
                    let co = items[item_idx].data[m] as usize;
                    let needs_spawn = if co == 0 {
                        true
                    } else {
                        Repository::with_characters(|characters| {
                            if co >= characters.len() || characters[co].used == USE_EMPTY {
                                true
                            } else if characters[co].data[0] != item_idx as i32 {
                                true
                            } else if (characters[co].flags & CharacterFlags::CF_BODY.bits()) != 0 {
                                true
                            } else {
                                false
                            }
                        })
                    };
                    (co, needs_spawn)
                });

                if needs_spawn {
                    let new_enemy = spawn_penta_enemy(item_idx);
                    Repository::with_items_mut(|items| {
                        items[item_idx].data[m] = new_enemy;
                    });
                }
            }
        }
        return 0;
    }

    if cn == 0 {
        return 0;
    }

    // Check rank restriction
    let (r1, r2) = Repository::with_characters(|characters| {
        let r1 = points2rank(characters[cn].points_tot as u32) as i32;
        let r2 = Repository::with_items(|items| {
            let mut r2 = items[item_idx].data[9];
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
        });
        (r1, r2)
    });

    if r1 > r2 {
        State::with(|state| {
            state.do_character_log(
                cn,
                core::types::FontColor::Green,
                &format!(
                    "You cannot use this pentagram. It is reserved for rank {} and below.\\n",
                    r2
                ),
            );
        });
        return 0;
    }

    // Activate pentagram
    let v = Repository::with_items_mut(|items| {
        let v = items[item_idx].data[0];
        items[item_idx].data[8] = cn as i32;
        items[item_idx].duration = -1;
        v
    });

    let exp_points = (v * v) / 7 + 1;
    State::with(|state| {
        state.do_character_log(
            cn,
            core::types::FontColor::Yellow,
            &format!(
                "You activated the pentagram with the value {}. It is worth {} experience point{}.\\n",
                v,
                exp_points,
                if v == 1 { "" } else { "s" }
            ),
        );
    });

    // Count active pentagrams and find top 5
    let mut tot = 0;
    let mut act = 0;
    let mut exp = 0;
    let mut b = [0usize; 5];
    let mut bv = [0i32; 5];

    for n in 1..MAXITEM {
        let (item_used, item_driver, item_active, item_data8, item_data0) =
            Repository::with_items(|items| {
                if n >= items.len() {
                    return (USE_EMPTY, 0, 0, 0, 0);
                }
                (
                    items[n].used,
                    items[n].driver,
                    items[n].active,
                    items[n].data[8],
                    items[n].data[0],
                )
            });

        if item_used == USE_EMPTY {
            continue;
        }
        if item_driver != 33 {
            continue;
        }
        tot += 1;
        if n != item_idx && item_active != -1 {
            continue;
        }
        act += 1;
        if item_data8 != cn as i32 {
            continue;
        }

        let v = item_data0;
        // Insert into sorted top 5 list
        if v > bv[0] {
            bv[4] = bv[3];
            bv[3] = bv[2];
            bv[2] = bv[1];
            bv[1] = bv[0];
            bv[0] = v;
            b[4] = b[3];
            b[3] = b[2];
            b[2] = b[1];
            b[1] = b[0];
            b[0] = n;
        } else if v > bv[1] {
            bv[4] = bv[3];
            bv[3] = bv[2];
            bv[2] = bv[1];
            bv[1] = v;
            b[4] = b[3];
            b[3] = b[2];
            b[2] = b[1];
            b[1] = n;
        } else if v > bv[2] {
            bv[4] = bv[3];
            bv[3] = bv[2];
            bv[2] = v;
            b[4] = b[3];
            b[3] = b[2];
            b[2] = n;
        } else if v > bv[3] {
            bv[4] = bv[3];
            bv[3] = v;
            b[4] = b[3];
            b[3] = n;
        } else if v > bv[4] {
            bv[4] = v;
            b[4] = n;
        }
    }

    // Display top 5 pentagrams
    if b[0] != 0 {
        State::with(|state| {
            state.do_character_log(cn, core::types::FontColor::Yellow, "You're holding:\\n");
        });
    }

    for n in 0..5 {
        if b[n] != 0 {
            let points = (bv[n] * bv[n]) / 7 + 1;
            State::with(|state| {
                state.do_character_log(
                    cn,
                    core::types::FontColor::Yellow,
                    &format!(
                        "Pentagram {:3}, worth {:5} point{}.\\n",
                        bv[n],
                        points,
                        if bv[n] == 1 { "" } else { "s" }
                    ),
                );
            });
            exp += points;
        }
    }

    Repository::with_characters_mut(|characters| {
        characters[cn].data[18] = exp;
    });

    State::with(|state| {
        state.do_character_log(
            cn,
            core::types::FontColor::Yellow,
            &format!(
                "Your pentagrammas are worth a total of {} experience points. Note that only the highest 5 pentagrammas count towards your experience bonus.\\n",
                exp
            ),
        );
        state.do_character_log(
            cn,
            core::types::FontColor::Yellow,
            &format!(
                "There are {} pentagrammas total, of which {} are active.\\n",
                tot, act
            ),
        );
    });

    // TODO: Implement chlog
    log::info!(
        "Character {} activated pentagram {} ({} of needed)",
        cn,
        v,
        act
    );

    // Check if quest solved
    let penta_needed = 5; // TODO: Calculate based on active players
    if act >= penta_needed {
        solved_pentagram(cn, item_idx);
        return 0;
    }

    // Spawn enemies
    for m in 1..4 {
        let (co, needs_spawn) = Repository::with_items(|items| {
            let co = items[item_idx].data[m] as usize;
            let needs_spawn = if co == 0 {
                true
            } else {
                Repository::with_characters(|characters| {
                    if co >= characters.len() || characters[co].used == USE_EMPTY {
                        true
                    } else if characters[co].data[0] != item_idx as i32 {
                        true
                    } else if (characters[co].flags & CharacterFlags::CF_BODY.bits()) != 0 {
                        true
                    } else {
                        false
                    }
                })
            };
            (co, needs_spawn)
        });

        if needs_spawn {
            let new_enemy = spawn_penta_enemy(item_idx);
            Repository::with_items_mut(|items| {
                items[item_idx].data[m] = new_enemy;
            });
        }
    }

    1
}

pub fn use_shrine(cn: usize, item_idx: usize) -> i32 {
    use crate::helpers::points2rank;
    use crate::repository::Repository;
    use crate::state::State;
    use core::constants::{ItemFlags, USE_EMPTY};

    if cn == 0 {
        return 0;
    }

    let in2 = Repository::with_characters(|characters| characters[cn].citem as usize);

    if in2 == 0 {
        State::with(|state| {
            state.do_character_log(
                cn,
                core::types::FontColor::Yellow,
                "You get the feeling that it would be apropriate to give the gods a present.\\n",
            );
        });
        return 0;
    }

    // Calculate item value
    let val = if (in2 & 0x80000000) != 0 {
        // Money
        let val = (in2 & 0x7fffffff) as i32;
        Repository::with_characters_mut(|characters| {
            characters[cn].citem = 0;
        });
        val
    } else {
        // Item
        let (value, is_unique) = Repository::with_items(|items| {
            let mut val = items[in2].value;
            if (items[in2].flags & ItemFlags::IF_UNIQUE.bits()) != 0 {
                val *= 4;
            }
            (val, (items[in2].flags & ItemFlags::IF_UNIQUE.bits()) != 0)
        });

        Repository::with_items_mut(|items| {
            items[in2].used = USE_EMPTY;
        });
        Repository::with_characters_mut(|characters| {
            characters[cn].citem = 0;
        });
        value
    };

    let mut val = val + (rand::random::<i32>() % (val + 1));

    // Calculate rank threshold
    let rank = Repository::with_characters(|characters| {
        let r = points2rank(characters[cn].points_tot as u32) as i32 + 1;
        r * r * r * 4
    });

    // Check if offering is acceptable
    if val >= rank {
        // Restore mana
        let mana_restored = Repository::with_characters_mut(|characters| {
            if characters[cn].a_mana < characters[cn].mana[5] * 1000 {
                characters[cn].a_mana = characters[cn].mana[5] * 1000;
                true
            } else {
                false
            }
        });

        if mana_restored {
            State::with(|state| {
                state.do_character_log(
                    cn,
                    core::types::FontColor::Green,
                    "You feel the hand of the Goddess of Magic touch your mind.\\n",
                );
            });
            // TODO: fx_add_effect(6, 0, ch[cn].x, ch[cn].y, 0);
        }

        // Determine message based on value
        let message = if val >= rank * 64 {
            "The gods are madly in love with your offer.\\n"
        } else if val >= rank * 32 {
            "The gods love your offer very much.\\n"
        } else if val >= rank * 16 {
            "The gods love your offer.\\n"
        } else if val >= rank * 8 {
            "The gods are very pleased with your offer.\\n"
        } else if val >= rank * 4 {
            "The gods are pleased with your offer.\\n"
        } else if val >= rank * 2 {
            "The gods deemed your offer apropriate.\\n"
        } else {
            "The gods accepted your offer.\\n"
        };

        State::with(|state| {
            state.do_character_log(cn, core::types::FontColor::Yellow, message);
        });

        // Increase luck
        if val != 0 && rank != 0 {
            let m = val / rank;
            Repository::with_characters_mut(|characters| {
                characters[cn].luck += m;
            });
            // TODO: chlog for luck change
        }
    } else {
        // Offering not good enough
        let (message, luck_change) = if val < rank / 8 {
            ("You have angered the gods with your unworthy gift.\\n", -2)
        } else if val < rank / 4 {
            ("The gods sneer at your gift.\\n", -1)
        } else if val < rank / 2 {
            ("The gods think you're cheap.\\n", 0)
        } else {
            (
                "You feel that it takes more than this to please the gods.\\n",
                0,
            )
        };

        State::with(|state| {
            state.do_character_log(cn, core::types::FontColor::Yellow, message);
        });

        if luck_change != 0 {
            Repository::with_characters_mut(|characters| {
                characters[cn].luck += luck_change;
            });
        }
    }

    // Show luck status
    State::with(|state| {
        state.do_character_log(cn, core::types::FontColor::Yellow, " \\n");
    });

    let luck = Repository::with_characters(|characters| characters[cn].luck);
    let luck_message = if luck < -10000 {
        "You feel that the gods are mad at you.\\n"
    } else if luck < 0 {
        "You feel that the gods are angry at you.\\n"
    } else if luck < 100 {
        "You feel that the gods stance towards you is neutral.\\n"
    } else if luck < 1000 {
        "You feel that the gods are pleased with you.\\n"
    } else {
        "You feel that the gods are very fond of you.\\n"
    };

    State::with(|state| {
        state.do_character_log(cn, core::types::FontColor::Yellow, luck_message);
    });

    1
}

pub fn use_kill_undead(cn: usize, item_idx: usize) -> i32 {
    use crate::repository::Repository;
    use core::constants::{CharacterFlags, SERVER_MAPX, SERVER_MAPY, WN_RHAND};

    if cn == 0 {
        return 0;
    }

    // Check if wielding the item
    let is_wielded = Repository::with_characters(|characters| {
        characters[cn].worn[WN_RHAND] as usize == item_idx
    });

    if !is_wielded {
        return 0;
    }

    // TODO: fx_add_effect(7, 0, ch[cn].x, ch[cn].y, 0);

    // Get character position
    let (ch_x, ch_y) = Repository::with_characters(|characters| {
        (characters[cn].x as i32, characters[cn].y as i32)
    });

    // Damage all undead in 8x8 area
    for y in (ch_y - 8)..(ch_y + 8) {
        if y < 1 || y >= SERVER_MAPY as i32 {
            continue;
        }
        for x in (ch_x - 8)..(ch_x + 8) {
            if x < 1 || x >= SERVER_MAPX as i32 {
                continue;
            }

            let co =
                Repository::with_map(|map| map[(x + y * SERVER_MAPX as i32) as usize].ch as usize);

            if co != 0 {
                let is_undead = Repository::with_characters(|characters| {
                    (characters[co].flags & CharacterFlags::CF_UNDEAD.bits()) != 0
                });

                if is_undead {
                    // TODO: Implement do_hurt(cn, co, 500, 2);
                    log::info!("TODO: do_hurt({}, {}, 500, 2)", cn, co);
                    // TODO: fx_add_effect(5, 0, ch[co].x, ch[co].y, 0);
                }
            }
        }
    }

    item_damage_worn(cn, WN_RHAND, 500);

    1
}

pub fn teleport3(cn: usize, item_idx: usize) -> i32 {
    use crate::god::God;
    use crate::repository::Repository;
    use crate::state::State;
    use core::constants::{CharacterFlags, ItemFlags, SK_RECALL, USE_EMPTY};

    if cn == 0 {
        return 1;
    }

    // Check if requires activation
    let (needs_activation, is_active) = Repository::with_items(|items| {
        (
            (items[item_idx].flags & ItemFlags::IF_USEACTIVATE.bits()) != 0,
            items[item_idx].active != 0,
        )
    });

    if needs_activation && !is_active {
        return 1;
    }

    // Remove nolab items from citem
    let citem = Repository::with_characters(|characters| characters[cn].citem as usize);
    if citem != 0 && is_nolab_item(citem) {
        let item_ref = Repository::with_items(|items| items[citem].reference.clone());
        Repository::with_characters_mut(|characters| {
            characters[cn].citem = 0;
        });
        Repository::with_items_mut(|items| {
            items[citem].used = USE_EMPTY;
        });
        State::with(|state| {
            state.do_character_log(
                cn,
                core::types::FontColor::Yellow,
                &format!("Your {} vanished.\\n", item_ref),
            );
        });
    }

    // Remove nolab items from inventory
    for n in 0..40 {
        let in2 = Repository::with_characters(|characters| characters[cn].item[n] as usize);
        if in2 != 0 && is_nolab_item(in2) {
            let item_ref = Repository::with_items(|items| items[in2].reference.clone());
            Repository::with_characters_mut(|characters| {
                characters[cn].item[n] = 0;
            });
            Repository::with_items_mut(|items| {
                items[in2].used = USE_EMPTY;
            });
            State::with(|state| {
                state.do_character_log(
                    cn,
                    core::types::FontColor::Yellow,
                    &format!("Your {} vanished.\\n", item_ref),
                );
            });
        }
    }

    // Remove recall spells
    for n in 0..20 {
        let in2 = Repository::with_characters(|characters| characters[cn].spell[n] as usize);
        if in2 != 0 {
            let temp = Repository::with_items(|items| items[in2].temp);
            if temp == SK_RECALL {
                Repository::with_characters_mut(|characters| {
                    characters[cn].spell[n] = 0;
                });
                Repository::with_items_mut(|items| {
                    items[in2].used = USE_EMPTY;
                });
            }
        }
    }

    // Teleport
    // TODO: fx_add_effect(6, 0, ch[cn].x, ch[cn].y, 0);
    let (dest_x, dest_y) = Repository::with_items(|items| {
        (
            items[item_idx].data[0] as usize,
            items[item_idx].data[1] as usize,
        )
    });
    God::transfer_char(cn, dest_x, dest_y);
    // TODO: fx_add_effect(6, 0, ch[cn].x, ch[cn].y, 0);

    // Remove IF_LABYDESTROY items from citem
    let citem = Repository::with_characters(|characters| characters[cn].citem as usize);
    if citem != 0 && (citem & 0x80000000) == 0 {
        let has_flag = Repository::with_items(|items| {
            (items[citem].flags & ItemFlags::IF_LABYDESTROY.bits()) != 0
        });
        if has_flag {
            let item_ref = Repository::with_items(|items| items[citem].reference.clone());
            Repository::with_characters_mut(|characters| {
                characters[cn].citem = 0;
            });
            Repository::with_items_mut(|items| {
                items[citem].used = USE_EMPTY;
            });
            State::with(|state| {
                state.do_character_log(
                    cn,
                    core::types::FontColor::Yellow,
                    &format!("Your {} vanished.\\n", item_ref),
                );
            });
        }
    }

    // Remove IF_LABYDESTROY items from inventory
    for n in 0..40 {
        let in2 = Repository::with_characters(|characters| characters[cn].item[n] as usize);
        if in2 != 0 {
            let (has_flag, item_ref) = Repository::with_items(|items| {
                (
                    (items[in2].flags & ItemFlags::IF_LABYDESTROY.bits()) != 0,
                    items[in2].reference.clone(),
                )
            });
            if has_flag {
                Repository::with_characters_mut(|characters| {
                    characters[cn].item[n] = 0;
                });
                Repository::with_items_mut(|items| {
                    items[in2].used = USE_EMPTY;
                });
                State::with(|state| {
                    state.do_character_log(
                        cn,
                        core::types::FontColor::Yellow,
                        &format!("Your {} vanished.\\n", item_ref),
                    );
                });
            }
        }
    }

    // Remove IF_LABYDESTROY items from worn
    for n in 0..20 {
        let in2 = Repository::with_characters(|characters| characters[cn].worn[n] as usize);
        if in2 != 0 {
            let (has_flag, item_ref) = Repository::with_items(|items| {
                (
                    (items[in2].flags & ItemFlags::IF_LABYDESTROY.bits()) != 0,
                    items[in2].reference.clone(),
                )
            });
            if has_flag {
                Repository::with_characters_mut(|characters| {
                    characters[cn].worn[n] = 0;
                });
                Repository::with_items_mut(|items| {
                    items[in2].used = USE_EMPTY;
                });
                State::with(|state| {
                    state.do_character_log(
                        cn,
                        core::types::FontColor::Yellow,
                        &format!("Your {} vanished.\\n", item_ref),
                    );
                });
            }
        }
    }

    // Update temple/tavern coordinates
    let (kindred, is_staff) = Repository::with_characters(|characters| {
        (
            characters[cn].kindred,
            (characters[cn].flags & CharacterFlags::CF_STAFF.bits()) != 0,
        )
    });

    if (kindred & 0x00000001) != 0 {
        // KIN_PURPLE
        Repository::with_characters_mut(|characters| {
            characters[cn].temple_x = 558;
            characters[cn].temple_y = 542;
            characters[cn].tavern_x = 558;
            characters[cn].tavern_y = 542;
        });
    } else if is_staff {
        Repository::with_characters_mut(|characters| {
            characters[cn].temple_x = 813;
            characters[cn].temple_y = 165;
            characters[cn].tavern_x = 813;
            characters[cn].tavern_y = 165;
        });
    } else {
        Repository::with_characters_mut(|characters| {
            characters[cn].temple_x = 512;
            characters[cn].temple_y = 512;
            characters[cn].tavern_x = 512;
            characters[cn].tavern_y = 512;
        });
    }

    1
}

pub fn use_seyan_shrine(cn: usize, item_idx: usize) -> i32 {
    use crate::god::God;
    use crate::repository::Repository;
    use crate::state::State;
    use core::constants::{ItemFlags, MAXITEM, USE_ACTIVE, USE_EMPTY, WN_RHAND};

    if cn == 0 {
        return 0;
    }

    // Check if character is Seyan'Du (KIN_SEYAN_DU = 0x00000008)
    let is_seyan =
        Repository::with_characters(|characters| (characters[cn].kindred & 0x00000008) != 0);

    if !is_seyan {
        State::with(|state| {
            state.do_character_log(
                cn,
                core::types::FontColor::Yellow,
                "You have the feeling you're in the wrong place here.\\n",
            );
        });
        return 0;
    }

    // Check for existing Seyan'Du sword (driver 40)
    let mut in2 = Repository::with_characters(|characters| characters[cn].worn[WN_RHAND] as usize);

    let sword_valid = if in2 != 0 {
        Repository::with_items(|items| items[in2].driver == 40 && items[in2].data[0] == cn as i32)
    } else {
        false
    };

    // If no valid sword, replace old ones and create new one
    if !sword_valid {
        // Remove old swords (driver 40 for this character)
        for n in 1..MAXITEM {
            let should_replace = Repository::with_items(|items| {
                if n >= items.len() {
                    return false;
                }
                items[n].used == USE_ACTIVE
                    && items[n].driver == 40
                    && items[n].data[0] == cn as i32
            });

            if should_replace {
                // Replace with broken sword (template 683)
                let (x, y, carried) =
                    Repository::with_items(|items| (items[n].x, items[n].y, items[n].carried));

                let broken_sword = God::create_item(683);
                if broken_sword != 0 {
                    Repository::with_items_mut(|items| {
                        items[broken_sword].x = x;
                        items[broken_sword].y = y;
                        items[broken_sword].carried = carried;
                        items[broken_sword].temp = 683;
                        items[broken_sword].flags |= ItemFlags::IF_UPDATE.bits();
                    });
                    Repository::with_items_mut(|items| {
                        items[n].used = USE_EMPTY;
                    });
                }
            }
        }

        // Check luck requirement
        let luck = Repository::with_characters(|characters| characters[cn].luck);
        if luck < 50 {
            State::with(|state| {
                state.do_character_log(
                    cn,
                    core::types::FontColor::Yellow,
                    "Kwai, the great goddess of war, deemed you unworthy to receive a new blade.\\n",
                );
            });
            return 0;
        }

        // Create new Seyan'Du sword (template 682)
        in2 = God::create_item(682);
        God::give_character_item(in2, cn);
        Repository::with_items_mut(|items| {
            items[in2].data[0] = cn as i32;
        });
        State::with(|state| {
            state.do_character_log(
                cn,
                core::types::FontColor::Yellow,
                "Kwai, the great goddess of war, deemed you worthy to receive a new blade.\\n",
            );
        });
        Repository::with_characters_mut(|characters| {
            characters[cn].luck -= 50;
        });
    }

    // Mark this shrine as visited
    let shrine_bit = Repository::with_items(|items| items[item_idx].data[0]);
    let already_visited =
        Repository::with_characters(|characters| (characters[cn].data[21] & shrine_bit) != 0);

    if !already_visited {
        Repository::with_characters_mut(|characters| {
            characters[cn].data[21] |= shrine_bit;
        });
        State::with(|state| {
            state.do_character_log(
                cn,
                core::types::FontColor::Yellow,
                "You found a new shrine of Kwai!\\n",
            );
        });
        Repository::with_characters_mut(|characters| {
            characters[cn].luck += 10;
        });
    }

    // Count visited shrines
    let visited_bits = Repository::with_characters(|characters| {
        let mut count = 0;
        let mut bit = 1u32;
        while bit != 0 {
            if (characters[cn].data[21] & bit as i32) != 0 {
                count += 1;
            }
            bit = bit.wrapping_shl(1);
        }
        count
    });

    State::with(|state| {
        state.do_character_log(
            cn,
            core::types::FontColor::Green,
            &format!(
                "You have visited {} of the 20 shrines of Kwai.\\n",
                visited_bits
            ),
        );
    });

    // Update sword weapon power based on shrines visited
    let cn_name = Repository::with_characters(|characters| characters[cn].name.clone());
    Repository::with_items_mut(|items| {
        items[in2].weapon[0] = 15 + visited_bits * 4;
        items[in2].flags |= ItemFlags::IF_UPDATE.bits();
        items[in2].temp = 0;
        items[in2].description = format!(
            "A huge, two-handed sword, engraved with runes and magic symbols. It bears the name {}.",
            cn_name
        );
    });

    // TODO: do_update_char(cn);

    0
}

pub fn use_seyan_door(cn: usize, item_idx: usize) -> i32 {
    use crate::repository::Repository;

    if cn != 0 {
        // Check if character is Seyan'Du (KIN_SEYAN_DU = 0x00000008)
        let is_seyan =
            Repository::with_characters(|characters| (characters[cn].kindred & 0x00000008) != 0);
        if !is_seyan {
            return 0;
        }
    }

    use_door(cn, item_idx)
}

pub fn use_seyan_portal(cn: usize, item_idx: usize) -> i32 {
    use crate::god::God;
    use crate::repository::Repository;
    use crate::state::State;
    use core::constants::{ItemFlags, USE_EMPTY};

    if cn == 0 {
        return 0;
    }

    // Check if already Seyan'Du (KIN_SEYAN_DU = 0x00000008)
    let (is_seyan, is_male, cn_name) = Repository::with_characters(|characters| {
        (
            (characters[cn].kindred & 0x00000008) != 0,
            (characters[cn].kindred & 0x00000001) != 0, // KIN_MALE
            characters[cn].name.clone(),
        )
    });

    if is_seyan {
        State::with(|state| {
            state.do_character_log(
                cn,
                core::types::FontColor::Yellow,
                "You're already Seyan'Du, aren't you?\\n",
            );
        });
    } else {
        State::with(|state| {
            state.do_character_log(
                cn,
                core::types::FontColor::Yellow,
                &format!(
                    "The Seyan'Du welcome you among their ranks, {}!\\n",
                    cn_name
                ),
            );
        });

        // Change race: 13 for male Seyan'Du, 79 for female Seyan'Du
        // TODO: Implement god_racechange
        if is_male {
            log::info!("TODO: god_racechange({}, 13)", cn);
        } else {
            log::info!("TODO: god_racechange({}, 79)", cn);
        }

        // Give Seyan'Du sword (template 682)
        let in2 = God::create_item(682);
        God::give_character_item(in2, cn);
        Repository::with_items_mut(|items| {
            items[in2].data[0] = cn as i32;
        });
    }

    // Remove IF_LABYDESTROY items from citem
    let citem = Repository::with_characters(|characters| characters[cn].citem as usize);
    if citem != 0 && (citem & 0x80000000) == 0 {
        let has_flag = Repository::with_items(|items| {
            (items[citem].flags & ItemFlags::IF_LABYDESTROY.bits()) != 0
        });
        if has_flag {
            let item_ref = Repository::with_items(|items| items[citem].reference.clone());
            Repository::with_characters_mut(|characters| {
                characters[cn].citem = 0;
            });
            Repository::with_items_mut(|items| {
                items[citem].used = USE_EMPTY;
            });
            State::with(|state| {
                state.do_character_log(
                    cn,
                    core::types::FontColor::Yellow,
                    &format!("Your {} vanished.\\n", item_ref),
                );
            });
        }
    }

    // Remove IF_LABYDESTROY items from inventory
    for n in 0..40 {
        let in2 = Repository::with_characters(|characters| characters[cn].item[n] as usize);
        if in2 != 0 {
            let (has_flag, item_ref) = Repository::with_items(|items| {
                (
                    (items[in2].flags & ItemFlags::IF_LABYDESTROY.bits()) != 0,
                    items[in2].reference.clone(),
                )
            });
            if has_flag {
                Repository::with_characters_mut(|characters| {
                    characters[cn].item[n] = 0;
                });
                Repository::with_items_mut(|items| {
                    items[in2].used = USE_EMPTY;
                });
                State::with(|state| {
                    state.do_character_log(
                        cn,
                        core::types::FontColor::Yellow,
                        &format!("Your {} vanished.\\n", item_ref),
                    );
                });
            }
        }
    }

    // Remove IF_LABYDESTROY items from worn
    for n in 0..20 {
        let in2 = Repository::with_characters(|characters| characters[cn].worn[n] as usize);
        if in2 != 0 {
            let (has_flag, item_ref) = Repository::with_items(|items| {
                (
                    (items[in2].flags & ItemFlags::IF_LABYDESTROY.bits()) != 0,
                    items[in2].reference.clone(),
                )
            });
            if has_flag {
                Repository::with_characters_mut(|characters| {
                    characters[cn].worn[n] = 0;
                });
                Repository::with_items_mut(|items| {
                    items[in2].used = USE_EMPTY;
                });
                State::with(|state| {
                    state.do_character_log(
                        cn,
                        core::types::FontColor::Yellow,
                        &format!("Your {} vanished.\\n", item_ref),
                    );
                });
            }
        }
    }

    // Teleport
    // TODO: fx_add_effect(6, 0, ch[cn].x, ch[cn].y, 0);
    let (dest_x, dest_y) = Repository::with_items(|items| {
        (
            items[item_idx].data[0] as usize,
            items[item_idx].data[1] as usize,
        )
    });
    God::transfer_char(cn, dest_x, dest_y);
    // TODO: fx_add_effect(6, 0, ch[cn].x, ch[cn].y, 0);

    1
}

pub fn spell_scroll(cn: usize, item_idx: usize) -> i32 {
    use crate::repository::Repository;
    use crate::state::State;
    use core::constants::{
        SK_BLESS, SK_CURSE, SK_ENHANCE, SK_LIGHT, SK_MSHIELD, SK_PROTECT, SK_RESIST, SK_STUN,
    };

    // Read scroll data
    let (spell, power, charges) = Repository::with_items(|items| {
        (
            items[item_idx].data[0],
            items[item_idx].data[1],
            items[item_idx].data[2],
        )
    });

    if charges == 0 {
        State::with(|state| {
            state.do_character_log(cn, core::types::FontColor::Yellow, "Nothing happened!\\n");
        });
        return 0;
    }

    // Get target (skill_target1 or self)
    let mut co = Repository::with_characters(|characters| characters[cn].skill_target1 as usize);
    if co == 0 {
        co = cn;
    }

    // Check if can see target
    // TODO: Implement do_char_can_see
    let can_see = true; // Placeholder
    if !can_see {
        State::with(|state| {
            state.do_character_log(
                cn,
                core::types::FontColor::Yellow,
                "You cannot see your target.\\n",
            );
        });
        return 0;
    }

    // Check attack spells for may_attack
    if spell == SK_CURSE || spell == SK_STUN {
        // TODO: Implement may_attack_msg
        // if !may_attack_msg(cn, co, 1) {
        //     chlog(cn, "Prevented from attacking %s (%d)", ch[co].name, co);
        //     return 0;
        // }
        log::info!("TODO: may_attack_msg check for spell {}", spell);
    } else {
        // TODO: Implement player_or_ghost check
        // if !player_or_ghost(cn, co) {
        //     Change target to self
        //     co = cn;
        // }
    }

    // Cast spell
    let ret = match spell {
        SK_LIGHT => {
            // TODO: Implement spell_light
            log::info!("TODO: spell_light({}, {}, {})", cn, co, power);
            1
        }
        SK_ENHANCE => {
            // TODO: Implement spell_enhance
            log::info!("TODO: spell_enhance({}, {}, {})", cn, co, power);
            1
        }
        SK_PROTECT => {
            // TODO: Implement spell_protect
            log::info!("TODO: spell_protect({}, {}, {})", cn, co, power);
            1
        }
        SK_BLESS => {
            // TODO: Implement spell_bless
            log::info!("TODO: spell_bless({}, {}, {})", cn, co, power);
            1
        }
        SK_MSHIELD => {
            // TODO: Implement spell_mshield
            log::info!("TODO: spell_mshield({}, {}, {})", cn, co, power);
            1
        }
        SK_CURSE => {
            // TODO: Implement chance_base and spell_curse
            // if chance_base(cn, power, 10, ch[co].skill[SK_RESIST][5]) {
            //     1
            // } else {
            //     spell_curse(cn, co, power)
            // }
            log::info!("TODO: spell_curse({}, {}, {})", cn, co, power);
            1
        }
        SK_STUN => {
            // TODO: Implement chance_base and spell_stun
            // if chance_base(cn, power, 12, ch[co].skill[SK_RESIST][5]) {
            //     1
            // } else {
            //     spell_stun(cn, co, power)
            // }
            log::info!("TODO: spell_stun({}, {}, {})", cn, co, power);
            1
        }
        _ => 0,
    };

    // Decrement charges if spell succeeded
    if ret != 0 {
        let new_charges = charges - 1;
        Repository::with_items_mut(|items| {
            items[item_idx].data[2] = new_charges;
            items[item_idx].value /= 2;
        });
        if new_charges < 1 {
            return 1; // Scroll consumed
        }
    }

    0
}

pub fn use_blook_pentagram(cn: usize, item_idx: usize) -> i32 {
    use crate::repository::Repository;
    use crate::state::State;

    if cn == 0 {
        return 0;
    }

    State::with(|state| {
        state.do_character_log(
            cn,
            core::types::FontColor::Green,
            "You try to wipe off the blood, but it seems to be coming back slowly.\\n",
        );
    });

    // Set blood state and update sprite
    Repository::with_items_mut(|items| {
        items[item_idx].data[0] = 1;
        items[item_idx].sprite[0] = items[item_idx].data[1] + items[item_idx].data[0];
    });

    1
}

pub fn use_create_npc(cn: usize, item_idx: usize) -> i32 {
    use crate::god::God;
    use crate::populate::pop_create_char;
    use crate::repository::Repository;
    use core::constants::USE_EMPTY;

    // Check if already active
    let active = Repository::with_items(|items| items[item_idx].active);
    if active != 0 {
        return 0;
    }

    if cn == 0 {
        return 0;
    }

    // Create NPC from template
    let template = Repository::with_items(|items| items[item_idx].data[0]);
    let co = pop_create_char(template, 0);
    if co == 0 {
        return 0;
    }

    // Drop NPC near item location
    let (x, y) =
        Repository::with_items(|items| (items[item_idx].x as usize, items[item_idx].y as usize));
    if !God::drop_char_fuzzy(co, x, y) {
        God::destroy_items(co);
        Repository::with_characters_mut(|characters| {
            characters[co].used = USE_EMPTY;
        });
        return 0;
    }

    // Link NPC to creator
    Repository::with_characters_mut(|characters| {
        characters[co].data[0] = cn as i32;
    });

    1
}

pub fn use_rotate(cn: usize, item_idx: usize) -> i32 {
    use crate::repository::Repository;
    use core::constants::ItemFlags;

    if cn == 0 {
        return 0;
    }

    // Rotate item: increment data[1] (0-3), update sprite
    Repository::with_items_mut(|items| {
        items[item_idx].data[1] += 1;
        if items[item_idx].data[1] > 3 {
            items[item_idx].data[1] = 0;
        }
        items[item_idx].sprite[0] = items[item_idx].data[0] + items[item_idx].data[1];
        items[item_idx].flags |= ItemFlags::IF_UPDATE;
    });

    1
}

pub fn use_lab8_key(cn: usize, item_idx: usize) -> i32 {
    use crate::god::God;
    use crate::repository::Repository;
    use crate::state::State;
    use core::constants::{ItemFlags, USE_EMPTY};

    // data[0] = matching key part
    // data[1] = resulting key part
    // data[2] = (optional) other matching key part
    // data[3] = (optional) other resulting key part

    if cn == 0 {
        return 0;
    }

    let citem = Repository::with_characters(|characters| characters[cn].citem as usize);
    if citem == 0 || (citem & 0x80000000) != 0 {
        State::with(|state| {
            state.do_char_log(cn, 1, "Nothing happens.\n", crate::enums::FontColor::Red);
        });
        return 0;
    }

    let carried = Repository::with_items(|items| items[item_idx].carried);
    if carried == 0 {
        State::with(|state| {
            state.do_char_log(
                cn,
                0,
                "Too difficult to do on the ground.\n",
                crate::enums::FontColor::Yellow,
            );
        });
        return 0;
    }

    // Check for matching parts
    let (data0, data1, data2, data3, citem_temp) = Repository::with_items(|items| {
        (
            items[item_idx].data[0],
            items[item_idx].data[1],
            items[item_idx].data[2],
            items[item_idx].data[3],
            items[citem].temp,
        )
    });

    let result_template = if data0 == citem_temp {
        data1
    } else if data2 == citem_temp {
        data3
    } else {
        0
    };

    if result_template == 0 {
        State::with(|state| {
            state.do_char_log(
                cn,
                0,
                "Those don't fit together.\n",
                crate::enums::FontColor::Yellow,
            );
        });
        return 0;
    }

    // Log the assembly
    let (item_name, citem_name) = Repository::with_items(|items| {
        (
            String::from_utf8_lossy(&items[item_idx].name).to_string(),
            String::from_utf8_lossy(&items[citem].name).to_string(),
        )
    });
    log::info!("Added {} to {}", citem_name, item_name);

    // Remove both old parts
    God::take_item_from_char(item_idx, cn);
    Repository::with_items_mut(|items| {
        items[item_idx].used = USE_EMPTY;
    });

    Repository::with_characters_mut(|characters| {
        characters[cn].citem = 0;
    });
    Repository::with_items_mut(|items| {
        items[citem].used = USE_EMPTY;
    });

    // Create and give new key
    let new_key = God::create_item(result_template);
    Repository::with_items_mut(|items| {
        items[new_key].flags |= ItemFlags::IF_UPDATE;
    });
    God::give_character_item(new_key, cn);

    1
}

pub fn use_lab8_shrine(cn: usize, item_idx: usize) -> i32 {
    use crate::god::God;
    use crate::repository::Repository;
    use crate::state::State;
    use core::constants::USE_EMPTY;

    // data[0] = item accepted as offering
    // data[1] = item returned as gift

    if cn == 0 {
        return 0;
    }

    let offer = Repository::with_characters(|characters| characters[cn].citem as usize);
    if offer == 0 {
        State::with(|state| {
            state.do_char_log(
                cn,
                1,
                "You get the feeling that it would be apropriate to give the Goddess a present.\n",
                crate::enums::FontColor::Red,
            );
        });
        return 0;
    }

    // Check if offering is money or wrong item
    let (offer_temp, expected_temp) =
        Repository::with_items(|items| (items[offer].temp, items[item_idx].data[0]));

    if (offer & 0x80000000) != 0 || offer_temp != expected_temp {
        State::with(|state| {
            state.do_char_log(
                cn,
                1,
                "The Goddess only wants her property back, and rejects your offer.\n",
                crate::enums::FontColor::Red,
            );
        });
        return 0;
    }

    // Accept offering
    Repository::with_characters_mut(|characters| {
        characters[cn].citem = 0;
    });
    Repository::with_items_mut(|items| {
        items[offer].used = USE_EMPTY;
    });

    // Log the offering
    let (offer_ref, shrine_ref) = Repository::with_items(|items| {
        (
            String::from_utf8_lossy(&items[offer].reference).to_string(),
            String::from_utf8_lossy(&items[item_idx].reference).to_string(),
        )
    });
    log::info!("Offered {} at {}", offer_ref, shrine_ref);

    // Create and give gift
    let gift_template = Repository::with_items(|items| items[item_idx].data[1]);
    let gift = God::create_item(gift_template);

    if !God::give_character_item(gift, cn) {
        // If inventory full, put in carried
        Repository::with_characters_mut(|characters| {
            characters[cn].citem = gift as i32;
        });
        Repository::with_items_mut(|items| {
            items[gift].carried = cn as i32;
        });
    }

    let gift_ref =
        Repository::with_items(|items| String::from_utf8_lossy(&items[gift].reference).to_string());
    State::with(|state| {
        state.do_char_log(
            cn,
            1,
            &format!("The Goddess has given you a {} in return!\n", gift_ref),
            crate::enums::FontColor::Red,
        );
    });

    1
}

pub fn use_lab8_moneyshrine(cn: usize, item_idx: usize) -> i32 {
    use crate::god::God;
    use crate::repository::Repository;
    use crate::state::State;

    // data[0] = minimum offering accepted
    // data[1] = teleport coordinate x
    // data[2] = teleport coordinate y

    if cn == 0 {
        return 0;
    }

    let offer = Repository::with_characters(|characters| characters[cn].citem);
    if offer == 0 {
        State::with(|state| {
            state.do_char_log(
                cn,
                1,
                "You get the feeling that it would be apropriate to give the Goddess a present.\n",
                crate::enums::FontColor::Red,
            );
        });
        return 0;
    }

    // Check if it's money
    if (offer & 0x80000000) == 0 {
        State::with(|state| {
            state.do_char_log(
                cn,
                1,
                "Only money is accepted at this shrine.\n",
                crate::enums::FontColor::Red,
            );
        });
        return 0;
    }

    let amount = offer & 0x7fffffff;
    let min_offering = Repository::with_items(|items| items[item_idx].data[0]);

    if amount < min_offering {
        State::with(|state| {
            state.do_char_log(
                cn,
                1,
                "Your offering is not sufficient, and was rejected.\n",
                crate::enums::FontColor::Red,
            );
        });
        return 0;
    }

    // Log offering
    let shrine_ref = Repository::with_items(|items| {
        String::from_utf8_lossy(&items[item_idx].reference).to_string()
    });
    log::info!("offered {}G at {}", amount / 100, shrine_ref);

    // Accept money and teleport
    Repository::with_characters_mut(|characters| {
        characters[cn].citem = 0;
    });

    let (dest_x, dest_y) =
        Repository::with_items(|items| (items[item_idx].data[1], items[item_idx].data[2]));
    God::transfer_char(cn, dest_x, dest_y);

    // Restore mana if needed
    let (a_mana, max_mana) = Repository::with_characters(|characters| {
        (characters[cn].a_mana, characters[cn].mana[5] * 1000)
    });

    if a_mana < max_mana {
        Repository::with_characters_mut(|characters| {
            characters[cn].a_mana = characters[cn].mana[5] * 1000;
        });
        State::with(|state| {
            state.do_char_log(
                cn,
                0,
                "You feel the hand of the Goddess of Magic touch your mind.\n",
                crate::enums::FontColor::Yellow,
            );
        });
        // TODO: fx_add_effect(6, 0, ch[cn].x, ch[cn].y, 0);
    }

    1
}

pub fn change_to_archtemplar(cn: usize) {
    use crate::repository::Repository;
    use crate::state::State;

    // TODO: Implement god_minor_racechange
    const KIN_MALE: i32 = 0x00000001;

    // Check agility requirement
    let agility = Repository::with_characters(|characters| characters[cn].attrib[0][0]);
    if agility < 90 {
        State::with(|state| {
            state.do_char_log(
                cn,
                1,
                "Your agility is too low. There is still room for improvement.\n",
                crate::enums::FontColor::Red,
            );
        });
        return;
    }

    // Check strength requirement
    let strength = Repository::with_characters(|characters| characters[cn].attrib[1][0]);
    if strength < 90 {
        State::with(|state| {
            state.do_char_log(
                cn,
                1,
                "Your strength is too low. There is still room for improvement.\n",
                crate::enums::FontColor::Red,
            );
        });
        return;
    }

    // Change race based on gender
    let (is_male, name) = Repository::with_characters(|characters| {
        (
            (characters[cn].kindred & KIN_MALE) != 0,
            String::from_utf8_lossy(&characters[cn].name).to_string(),
        )
    });

    let new_race = if is_male { 544 } else { 549 };
    // TODO: god_minor_racechange(cn, new_race);
    log::info!("TODO: god_minor_racechange({}, {})", cn, new_race);

    State::with(|state| {
        state.do_char_log(
            cn,
            1,
            &format!(
                "You are truly worthy to become a Archtemplar. Congratulations, {}.\n",
                name
            ),
            crate::enums::FontColor::Red,
        );
    });
}

pub fn change_to_archharakim(cn: usize) {
    use crate::repository::Repository;
    use crate::state::State;

    // TODO: Implement god_minor_racechange

    // Check willpower requirement
    let willpower = Repository::with_characters(|characters| characters[cn].attrib[3][0]);
    if willpower < 90 {
        State::with(|state| {
            state.do_char_log(
                cn,
                1,
                "Your willpower is too low. There is still room for improvement.\n",
                crate::enums::FontColor::Red,
            );
        });
        return;
    }

    // Check intuition requirement
    let intuition = Repository::with_characters(|characters| characters[cn].attrib[4][0]);
    if intuition < 90 {
        State::with(|state| {
            state.do_char_log(
                cn,
                1,
                "Your intuition is too low. There is still room for improvement.\n",
                crate::enums::FontColor::Red,
            );
        });
        return;
    }

    // Change race based on gender
    const KIN_MALE: i32 = 0x00000001;
    let (is_male, name) = Repository::with_characters(|characters| {
        (
            (characters[cn].kindred & KIN_MALE) != 0,
            String::from_utf8_lossy(&characters[cn].name).to_string(),
        )
    });

    let new_race = if is_male { 545 } else { 550 };
    // TODO: god_minor_racechange(cn, new_race);
    log::info!("TODO: god_minor_racechange({}, {})", cn, new_race);

    State::with(|state| {
        state.do_char_log(
            cn,
            1,
            &format!(
                "You are truly worthy to become a Archharakim. Congratulations, {}.\n",
                name
            ),
            crate::enums::FontColor::Red,
        );
    });
}

pub fn change_to_warrior(cn: usize) {
    use crate::repository::Repository;
    use crate::state::State;

    // TODO: Implement god_minor_racechange
    const KIN_MALE: i32 = 0x00000001;

    // Check agility requirement
    let agility = Repository::with_characters(|characters| characters[cn].attrib[0][0]);
    if agility < 60 {
        State::with(|state| {
            state.do_char_log(
                cn,
                1,
                "Your agility is too low. There is still room for improvement.\n",
                crate::enums::FontColor::Red,
            );
        });
        return;
    }

    // Check strength requirement
    let strength = Repository::with_characters(|characters| characters[cn].attrib[1][0]);
    if strength < 60 {
        State::with(|state| {
            state.do_char_log(
                cn,
                1,
                "Your strength is too low. There is still room for improvement.\n",
                crate::enums::FontColor::Red,
            );
        });
        return;
    }

    // Change race based on gender
    let (is_male, name) = Repository::with_characters(|characters| {
        (
            (characters[cn].kindred & KIN_MALE) != 0,
            String::from_utf8_lossy(&characters[cn].name).to_string(),
        )
    });

    let new_race = if is_male { 547 } else { 552 };
    // TODO: god_minor_racechange(cn, new_race);
    log::info!("TODO: god_minor_racechange({}, {})", cn, new_race);

    State::with(|state| {
        state.do_char_log(
            cn,
            1,
            &format!(
                "You are truly worthy to become a Warrior. Congratulations, {}.\n",
                name
            ),
            crate::enums::FontColor::Red,
        );
    });
}

pub fn change_to_sorcerer(cn: usize) {
    use crate::repository::Repository;
    use crate::state::State;

    // TODO: Implement god_minor_racechange
    const KIN_MALE: i32 = 0x00000001;

    // Check willpower requirement
    let willpower = Repository::with_characters(|characters| characters[cn].attrib[3][0]);
    if willpower < 60 {
        State::with(|state| {
            state.do_char_log(
                cn,
                1,
                "Your willpower is too low. There is still room for improvement.\n",
                crate::enums::FontColor::Red,
            );
        });
        return;
    }

    // Check intuition requirement
    let intuition = Repository::with_characters(|characters| characters[cn].attrib[4][0]);
    if intuition < 60 {
        State::with(|state| {
            state.do_char_log(
                cn,
                1,
                "Your intuition is too low. There is still room for improvement.\n",
                crate::enums::FontColor::Red,
            );
        });
        return;
    }

    // Change race based on gender
    let (is_male, name) = Repository::with_characters(|characters| {
        (
            (characters[cn].kindred & KIN_MALE) != 0,
            String::from_utf8_lossy(&characters[cn].name).to_string(),
        )
    });

    let new_race = if is_male { 546 } else { 551 };
    // TODO: god_minor_racechange(cn, new_race);
    log::info!("TODO: god_minor_racechange({}, {})", cn, new_race);

    State::with(|state| {
        state.do_char_log(
            cn,
            1,
            &format!(
                "You are truly worthy to become a Sorcerer. Congratulations, {}.\n",
                name
            ),
            crate::enums::FontColor::Red,
        );
    });
}

pub fn shrine_of_change(cn: usize, item_idx: usize) -> i32 {
    use crate::repository::Repository;
    use crate::state::State;

    // Requires specific potions to change character class
    // Potion of Life (148) -> Archtemplar/Archharakim
    // Greater Healing Potion (127/274) -> Warrior
    // Greater Mana Potion (131/273) -> Sorcerer

    const KIN_TEMPLAR: i32 = 0x00000004;
    const KIN_HARAKIM: i32 = 0x00000002;
    const KIN_MERCENARY: i32 = 0x00000010;

    if cn == 0 {
        return 0;
    }

    let citem = Repository::with_characters(|characters| characters[cn].citem as usize);
    if citem == 0 || (citem & 0x80000000) != 0 {
        State::with(|state| {
            state.do_char_log(
                cn,
                1,
                "Read the notes, my friend.\n",
                crate::enums::FontColor::Red,
            );
        });
        return 0;
    }

    let (citem_temp, kindred) = Repository::with_items(|items| {
        let temp = items[citem].temp;
        let kindred = Repository::with_characters(|characters| characters[cn].kindred);
        (temp, kindred)
    });

    // Potion of life -> Archtemplar/Archharakim
    if citem_temp == 148 {
        if (kindred & KIN_TEMPLAR) != 0 {
            change_to_archtemplar(cn);
        } else if (kindred & KIN_HARAKIM) != 0 {
            change_to_archharakim(cn);
        } else {
            State::with(|state| {
                state.do_char_log(
                    cn,
                    1,
                    "You are neither Templar nor Harakim.\n",
                    crate::enums::FontColor::Red,
                );
            });
        }
        return 0;
    }

    // Greater healing potion -> Warrior
    if citem_temp == 127 || citem_temp == 274 {
        if (kindred & KIN_MERCENARY) != 0 {
            change_to_warrior(cn);
        } else {
            State::with(|state| {
                state.do_char_log(
                    cn,
                    1,
                    "You are not a Mercenary.\n",
                    crate::enums::FontColor::Red,
                );
            });
        }
        return 0;
    }

    // Greater mana potion -> Sorcerer
    if citem_temp == 131 || citem_temp == 273 {
        if (kindred & KIN_MERCENARY) != 0 {
            change_to_sorcerer(cn);
        } else {
            State::with(|state| {
                state.do_char_log(
                    cn,
                    1,
                    "You are not a Mercenary.\n",
                    crate::enums::FontColor::Red,
                );
            });
        }
        return 0;
    }

    State::with(|state| {
        state.do_char_log(
            cn,
            1,
            "Read the notes, my friend.\n",
            crate::enums::FontColor::Red,
        );
    });
    0
}

pub fn explorer_point(cn: usize, item_idx: usize) -> i32 {
    use crate::repository::Repository;
    use crate::state::State;

    // data[0-3] = bitmask for visited flags (stored in character data[46-49])
    // data[4] = base experience reward

    // Check if already visited
    let (data0, data1, data2, data3, char_data46, char_data47, char_data48, char_data49) =
        Repository::with_items(|items| {
            let d0 = items[item_idx].data[0];
            let d1 = items[item_idx].data[1];
            let d2 = items[item_idx].data[2];
            let d3 = items[item_idx].data[3];
            Repository::with_characters(|characters| {
                (
                    d0,
                    d1,
                    d2,
                    d3,
                    characters[cn].data[46],
                    characters[cn].data[47],
                    characters[cn].data[48],
                    characters[cn].data[49],
                )
            })
        });

    if ((char_data46 & data0) == 0)
        && ((char_data47 & data1) == 0)
        && ((char_data48 & data2) == 0)
        && ((char_data49 & data3) == 0)
    {
        // Mark as visited
        Repository::with_characters_mut(|characters| {
            characters[cn].data[46] |= data0;
            characters[cn].data[47] |= data1;
            characters[cn].data[48] |= data2;
            characters[cn].data[49] |= data3;
            characters[cn].luck += 10;
        });

        State::with(|state| {
            state.do_char_log(
                cn,
                0,
                "You found a new exploration point!\n",
                crate::enums::FontColor::Yellow,
            );
        });

        // Calculate experience reward
        let (base_exp, points_tot) = Repository::with_items(|items| {
            let base = items[item_idx].data[4];
            let pts = Repository::with_characters(|characters| characters[cn].points_tot);
            (base, pts)
        });

        use rand::Rng;
        let mut rng = rand::thread_rng();

        let mut exp = base_exp / 2 + rng.gen_range(0..base_exp);
        exp = std::cmp::min(points_tot / 10, exp); // Not more than 10% of total experience
        exp += rng.gen_range(0..(exp / 10 + 1)); // Some more randomness

        log::info!(
            "exp point giving {} ({}) exp, char has {} exp",
            exp,
            base_exp,
            points_tot
        );

        // TODO: do_give_exp(cn, exp, 0, -1);
        log::info!("TODO: do_give_exp({}, {}, 0, -1)", cn, exp);
    }

    0
}

pub fn use_garbage(cn: usize, item_idx: usize) -> i32 {
    use crate::repository::Repository;
    use crate::state::State;
    use core::constants::USE_EMPTY;

    if cn == 0 {
        return 0;
    }

    let citem = Repository::with_characters(|characters| characters[cn].citem);
    if citem == 0 {
        State::with(|state| {
            state.do_char_log(
                cn,
                1,
                "You feel that you could dispose of unwanted items in this digusting garbage can.\n",
                crate::enums::FontColor::Red,
            );
        });
        return 0;
    }

    if (citem & 0x80000000) != 0 {
        // Money
        let val = citem & 0x7fffffff;
        Repository::with_characters_mut(|characters| {
            characters[cn].citem = 0;
        });

        State::with(|state| {
            state.do_char_log(
                cn,
                1,
                &format!(
                    "You disposed of {} gold and {} silver.\n",
                    val / 100,
                    val % 100
                ),
                crate::enums::FontColor::Red,
            );
        });
    } else {
        // Item
        let reference = Repository::with_items(|items| {
            String::from_utf8_lossy(&items[citem as usize].reference).to_string()
        });

        Repository::with_characters_mut(|characters| {
            characters[cn].citem = 0;
        });
        Repository::with_items_mut(|items| {
            items[citem as usize].used = USE_EMPTY;
        });

        State::with(|state| {
            state.do_char_log(
                cn,
                1,
                &format!("You disposed of the {}.\n", reference),
                crate::enums::FontColor::Red,
            );
        });
    }

    1
}

pub fn use_driver(cn: usize, item_idx: usize, carried: bool) {
    use crate::repository::Repository;
    use core::constants::ItemFlags;

    // TODO: This is a massive dispatcher function with 69+ cases
    // For now, implement the basic structure and most common cases
    // The full implementation requires all the individual use_* functions to exist

    if item_idx == 0 || cn >= 10000 {
        return;
    }

    // Check if character is in build mode
    if cn != 0 {
        let in_build_mode = Repository::with_characters(|characters| {
            // TODO: Check CF_BUILDMODE flag
            false // Placeholder
        });
        if in_build_mode {
            return;
        }
    }

    // TODO: Set cerrno to ERR_FAILED if cn != 0 && !carried

    let has_use_flag =
        Repository::with_items(|items| (items[item_idx].flags & ItemFlags::IF_USE) != 0);

    if !has_use_flag && cn != 0 {
        return;
    }

    // Check if tile is occupied (for non-carried items)
    if !carried {
        // TODO: Check if map[m].ch || map[m].to_ch
        // For now, skip this check
    }

    let has_usespecial =
        Repository::with_items(|items| (items[item_idx].flags & ItemFlags::IF_USESPECIAL) != 0);

    if has_usespecial {
        let driver = Repository::with_items(|items| items[item_idx].driver);
        let ret = match driver {
            1 => use_create_item(cn, item_idx),
            2 => use_door(cn, item_idx),
            3 => {
                // Lock-pick - special message
                if cn != 0 {
                    use crate::state::State;
                    State::with(|state| {
                        state.do_char_log(
                            cn,
                            0,
                            "You use cannot the lock-pick directly. Hold it under your mouse cursor and click on the door...\n",
                            crate::enums::FontColor::Yellow,
                        );
                    });
                }
                0
            }
            4 => use_mix_potion(cn, item_idx),
            5 => stone_sword(cn, item_idx),
            6 => teleport(cn, item_idx),
            7 => use_bag(cn, item_idx),
            8 => use_scroll(cn, item_idx),
            9 => use_crystal(cn, item_idx),
            10 => use_scroll2(cn, item_idx),
            11 => use_scroll3(cn, item_idx),
            12 => use_scroll4(cn, item_idx),
            13 => use_scroll5(cn, item_idx),
            14 => use_chain(cn, item_idx),
            15 => use_labyrinth(cn, item_idx),
            16 => use_ladder(cn, item_idx),
            17 => rat_eye(cn, item_idx),
            18 => skua_protect(cn, item_idx),
            19 => use_lever(cn, item_idx),
            20 => use_door(cn, item_idx),
            21 => use_spawn(cn, item_idx),
            22 => use_pile(cn, item_idx),
            23 => teleport2(cn, item_idx),
            24 => build_ring(cn, item_idx),
            25 => use_mine(cn, item_idx),
            26 => use_mine_fast(cn, item_idx),
            27 => use_mine_respawn(cn, item_idx),
            28 => use_gargoyle(cn, item_idx),
            29 => use_grave(cn, item_idx),
            30 => use_create_item2(cn, item_idx),
            31 => 0, // empty, hole water
            32 => build_amulet(cn, item_idx),
            33 => use_pentagram(cn, item_idx),
            34 => use_seyan_shrine(cn, item_idx),
            35 => use_seyan_door(cn, item_idx),
            36 => 0, // magic portal 1 in lab13
            37 => 0, // traps
            38 => 0, // magic portal 2 in lab13
            39 => purple_protect(cn, item_idx),
            40 => 0, // seyan'du sword
            41 => use_shrine(cn, item_idx),
            42 => use_create_item3(cn, item_idx),
            43 => 0, // spiderweb
            44 => use_kill_undead(cn, item_idx),
            45 => use_seyan_portal(cn, item_idx),
            46 => teleport3(cn, item_idx),
            47 => 0, // arena portal
            48 => spell_scroll(cn, item_idx),
            49 => use_blook_pentagram(cn, item_idx),
            50 => use_create_npc(cn, item_idx),
            51 => use_rotate(cn, item_idx),
            52 => 0, // personal item
            53 => use_create_item(cn, item_idx),
            54 => use_create_item(cn, item_idx),
            55 => shrine_of_change(cn, item_idx),
            56 => 0, // greenling green ball
            57 => explorer_point(cn, item_idx),
            58 => use_grolm(cn, item_idx),
            59 => use_create_gold(cn, item_idx),
            61 => use_lab8_key(cn, item_idx),
            63 => use_lab8_shrine(cn, item_idx),
            64 => use_lab8_moneyshrine(cn, item_idx),
            65 => use_lab9_switch(cn, item_idx),
            66 => use_lab9_door(cn, item_idx),
            67 => use_garbage(cn, item_idx),
            68 => use_soulstone(cn, item_idx),
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
                // TODO: Set ch[cn].cerrno = ERR_FAILED if !carried
            } else if !carried {
                // TODO: Set ch[cn].cerrno = ERR_SUCCESS
            }
        }
        // TODO: do_update_char(cn);
    }

    if cn == 0 {
        return; // item_tick does activate and deactivate as well
    }

    // Handle activation/deactivation
    let (active, has_usedeactivate, has_useactivate) = Repository::with_items(|items| {
        (
            items[item_idx].active,
            (items[item_idx].flags & ItemFlags::IF_USEDEACTIVATE) != 0,
            (items[item_idx].flags & ItemFlags::IF_USEACTIVATE) != 0,
        )
    });

    if active != 0 && has_usedeactivate {
        Repository::with_items_mut(|items| {
            items[item_idx].active = 0;
            // TODO: Handle light changes
            if carried {
                items[item_idx].flags |= ItemFlags::IF_UPDATE;
            }
        });
        // TODO: do_update_char(cn) if carried
        // TODO: Set ch[cn].cerrno = ERR_SUCCESS if cn && !carried
    } else if active == 0 && has_useactivate {
        let duration = Repository::with_items(|items| items[item_idx].duration);
        Repository::with_items_mut(|items| {
            items[item_idx].active = duration;
            // TODO: Handle light changes
            if carried {
                items[item_idx].flags |= ItemFlags::IF_UPDATE;
            }
        });
        // TODO: do_update_char(cn) if carried
        // TODO: Set ch[cn].cerrno = ERR_SUCCESS if cn && !carried
    }

    // Handle IF_USEDESTROY items (potions, etc.)
    if carried {
        let has_usedestroy =
            Repository::with_items(|items| (items[item_idx].flags & ItemFlags::IF_USEDESTROY) != 0);

        if has_usedestroy {
            // TODO: Check min_rank requirement
            // TODO: Apply hp, end, mana changes
            // TODO: Remove item from inventory
            // For now, just log it
            log::info!("TODO: Handle IF_USEDESTROY for item {}", item_idx);
        }
    }
}

pub fn item_age(item_idx: usize) -> i32 {
    use crate::repository::Repository;
    use core::constants::ItemFlags;

    let (active, current_age_act, max_age_act, current_damage, max_damage) =
        Repository::with_items(|items| {
            let act = if items[item_idx].active != 0 { 1 } else { 0 };
            (
                act,
                items[item_idx].current_age[act],
                items[item_idx].max_age[act],
                items[item_idx].current_damage,
                items[item_idx].max_damage,
            )
        });

    if (max_age_act != 0 && current_age_act > max_age_act)
        || (max_damage != 0 && current_damage > max_damage)
    {
        Repository::with_items_mut(|items| {
            items[item_idx].flags |= ItemFlags::IF_UPDATE;
            items[item_idx].current_damage = 0;
            items[item_idx].current_age[0] = 0;
            items[item_idx].current_age[1] = 0;
            items[item_idx].damage_state += 1;
            items[item_idx].value /= 2;

            if items[item_idx].damage_state > 1 {
                let st = std::cmp::max(0, 4 - items[item_idx].damage_state as i32);

                if items[item_idx].armor[0] > st {
                    items[item_idx].armor[0] -= 1;
                }
                if items[item_idx].armor[1] > st {
                    items[item_idx].armor[1] -= 1;
                }

                if items[item_idx].weapon[0] > st * 2 {
                    items[item_idx].weapon[0] -= 1;
                    if items[item_idx].weapon[0] > 0 {
                        items[item_idx].weapon[0] -= 1;
                    }
                }
                if items[item_idx].weapon[1] > st * 2 {
                    items[item_idx].weapon[1] -= 1;
                    if items[item_idx].weapon[1] > 0 {
                        items[item_idx].weapon[1] -= 1;
                    }
                }
            }

            if items[item_idx].max_age[0] != 0 {
                items[item_idx].sprite[0] += 1;
            }
            if items[item_idx].max_age[1] != 0 {
                items[item_idx].sprite[1] += 1;
            }
        });

        return 1;
    }

    // Expire no-age items after 30 minutes (lag scrolls after 2 minutes)
    const TICKS: i32 = 12; // Assuming 12 ticks per second
    if max_age_act == 0 {
        let is_lag_scroll = Repository::with_items(|items| items[item_idx].temp == 500);
        let expire_time = if is_lag_scroll {
            TICKS * 60 * 2
        } else {
            TICKS * 60 * 30
        };

        if current_age_act > expire_time {
            Repository::with_items_mut(|items| {
                items[item_idx].damage_state = 5;
            });
            return 1;
        }
    }

    0
}

pub fn item_damage_worn(cn: usize, n: usize, damage: i32) {
    use crate::repository::Repository;
    use crate::state::State;
    use core::constants::USE_EMPTY;

    let worn_idx = Repository::with_characters(|characters| characters[cn].worn[n] as usize);
    if worn_idx == 0 {
        return;
    }

    let has_max_damage = Repository::with_items(|items| items[worn_idx].max_damage != 0);
    if !has_max_damage {
        return;
    }

    Repository::with_items_mut(|items| {
        items[worn_idx].current_damage += damage;
    });

    if item_age(worn_idx) != 0 {
        let (damage_state, reference) = Repository::with_items(|items| {
            (
                items[worn_idx].damage_state,
                String::from_utf8_lossy(&items[worn_idx].reference).to_string(),
            )
        });

        match damage_state {
            1 => {
                State::with(|state| {
                    state.do_char_log(
                        cn,
                        1,
                        &format!("The {} you are using is showing signs of use.\n", reference),
                        crate::enums::FontColor::Red,
                    );
                });
            }
            2 => {
                State::with(|state| {
                    state.do_char_log(
                        cn,
                        1,
                        &format!("The {} you are using was slightly damaged.\n", reference),
                        crate::enums::FontColor::Red,
                    );
                });
            }
            3 => {
                State::with(|state| {
                    state.do_char_log(
                        cn,
                        1,
                        &format!("The {} you are using was damaged.\n", reference),
                        crate::enums::FontColor::Red,
                    );
                });
            }
            4 => {
                State::with(|state| {
                    state.do_char_log(
                        cn,
                        0,
                        &format!("The {} you are using was badly damaged.\n", reference),
                        crate::enums::FontColor::Yellow,
                    );
                });
            }
            5 => {
                Repository::with_characters_mut(|characters| {
                    characters[cn].worn[n] = 0;
                });
                Repository::with_items_mut(|items| {
                    items[worn_idx].used = USE_EMPTY;
                });
                State::with(|state| {
                    state.do_char_log(
                        cn,
                        0,
                        &format!("The {} you were using was destroyed.\n", reference),
                        crate::enums::FontColor::Yellow,
                    );
                });
            }
            _ => {}
        }
        // TODO: do_update_char(cn);
    }
}

pub fn item_damage_citem(cn: usize, damage: i32) {
    use crate::repository::Repository;
    use crate::state::State;
    use core::constants::USE_EMPTY;

    let citem = Repository::with_characters(|characters| characters[cn].citem);
    if citem == 0 || (citem & 0x80000000) != 0 {
        return;
    }

    let citem_idx = citem as usize;
    let has_max_damage = Repository::with_items(|items| items[citem_idx].max_damage != 0);
    if !has_max_damage {
        return;
    }

    Repository::with_items_mut(|items| {
        items[citem_idx].current_damage += damage;
    });

    if item_age(citem_idx) != 0 {
        let (damage_state, reference) = Repository::with_items(|items| {
            (
                items[citem_idx].damage_state,
                String::from_utf8_lossy(&items[citem_idx].reference).to_string(),
            )
        });

        match damage_state {
            1 => {
                State::with(|state| {
                    state.do_char_log(
                        cn,
                        1,
                        &format!("The {} you are using is showing signs of use.\n", reference),
                        crate::enums::FontColor::Red,
                    );
                });
            }
            2 => {
                State::with(|state| {
                    state.do_char_log(
                        cn,
                        1,
                        &format!("The {} you are using was slightly damaged.\n", reference),
                        crate::enums::FontColor::Red,
                    );
                });
            }
            3 => {
                State::with(|state| {
                    state.do_char_log(
                        cn,
                        1,
                        &format!("The {} you are using was damaged.\n", reference),
                        crate::enums::FontColor::Red,
                    );
                });
            }
            4 => {
                State::with(|state| {
                    state.do_char_log(
                        cn,
                        0,
                        &format!("The {} you are using was badly damaged.\n", reference),
                        crate::enums::FontColor::Yellow,
                    );
                });
            }
            5 => {
                Repository::with_characters_mut(|characters| {
                    characters[cn].citem = 0;
                });
                Repository::with_items_mut(|items| {
                    items[citem_idx].used = USE_EMPTY;
                });
                State::with(|state| {
                    state.do_char_log(
                        cn,
                        0,
                        &format!("The {} you were using was destroyed.\n", reference),
                        crate::enums::FontColor::Yellow,
                    );
                });
            }
            _ => {}
        }
    }
}

pub fn item_damage_armor(cn: usize, damage: i32) {
    let dam = damage / 4 + 1;

    use rand::Rng;
    let mut rng = rand::thread_rng();

    const WN_RHAND: usize = 8;
    const WN_LHAND: usize = 17;

    for n in 0..20 {
        if n != WN_RHAND && n != WN_LHAND {
            if rng.gen_range(0..3) != 0 {
                item_damage_worn(cn, n, dam);
            }
        }
    }
}

pub fn item_damage_weapon(cn: usize, damage: i32) {
    const WN_RHAND: usize = 8;
    let dam = damage / 4 + 1;
    item_damage_worn(cn, WN_RHAND, dam);
}

pub fn lightage(item_idx: usize, multi: i32) {
    use crate::repository::Repository;

    // TODO: Need access to map data for light levels
    // For now, this is a placeholder that shows the logic structure

    let (carried, x, y) = Repository::with_items(|items| {
        (
            items[item_idx].carried,
            items[item_idx].x,
            items[item_idx].y,
        )
    });

    // Get map position
    let _m = if carried != 0 {
        // If carried by character, use character's position
        Repository::with_characters(|characters| {
            let cn = carried as usize;
            characters[cn].x as i32 + characters[cn].y as i32 * 512 // SERVER_MAPX
        })
    } else {
        // Use item's position
        x as i32 + y as i32 * 512 // SERVER_MAPX
    };

    // TODO: Access map[m].light
    // For now, assume moderate light
    let mut light = 100; // Placeholder

    if light < 1 {
        return;
    }
    if light > 250 {
        light = 250;
    }

    light *= multi;

    let act = Repository::with_items(|items| if items[item_idx].active != 0 { 1 } else { 0 });

    Repository::with_items_mut(|items| {
        items[item_idx].current_age[act] += light * 2;
    });
}

pub fn age_message(cn: usize, item_idx: usize, where_is: &str) {
    use crate::repository::Repository;
    use crate::state::State;

    let (driver, damage_state, reference) = Repository::with_items(|items| {
        (
            items[item_idx].driver,
            items[item_idx].damage_state,
            String::from_utf8_lossy(&items[item_idx].reference).to_string(),
        )
    });

    let (msg, font) = if driver == 60 {
        // Ice egg or cloak
        match damage_state {
            1 => ("The {} {} is beginning to melt.\n", 1),
            2 => ("The {} {} is melting fairly rapidly.\n", 1),
            3 => (
                "The {} {} is melting down as you look and dripping water everywhere.\n",
                1,
            ),
            4 => (
                "The {} {} has melted down to a small icy lump and large puddles of water.\n",
                0,
            ),
            5 => (
                "The {} {} has completely melted away, leaving you all wet.\n",
                0,
            ),
            _ => ("The {} {} is changing.\n", 1),
        }
    } else {
        // Anything else
        match damage_state {
            1 => ("The {} {} is showing signs of age.\n", 1),
            2 => ("The {} {} is getting fairly old.\n", 1),
            3 => ("The {} {} is getting old.\n", 1),
            4 => ("The {} {} is getting very old and battered.\n", 0),
            5 => (
                "The {} {} was so old and battered that it finally vanished.\n",
                0,
            ),
            _ => ("The {} {} is aging.\n", 1),
        }
    };

    let formatted_msg = msg.replace("{}", &reference).replace("{}", where_is);
    let color = if font == 1 {
        crate::enums::FontColor::Red
    } else {
        crate::enums::FontColor::Yellow
    };

    State::with(|state| {
        state.do_char_log(cn, font, &formatted_msg, color);
    });
}

pub fn char_item_expire(cn: usize) {
    use crate::repository::Repository;
    use core::constants::{ItemFlags, USE_EMPTY};

    // TODO: Check IS_BUILDING flag
    // Static clock for ice cloak aging (ages more slowly when not worn)
    static mut CLOCK4: i32 = 0;

    let mut must_update = false;

    unsafe {
        CLOCK4 += 1;
    }

    // Age items in backpack (40 slots)
    for n in 0..40 {
        let item_idx = Repository::with_characters(|characters| characters[cn].item[n] as usize);
        if item_idx == 0 {
            continue;
        }

        let (active, has_alwaysexp1, has_alwaysexp2, driver, has_lightage) =
            Repository::with_items(|items| {
                let act = if items[item_idx].active != 0 { 1 } else { 0 };
                (
                    act,
                    (items[item_idx].flags & ItemFlags::IF_ALWAYSEXP1) != 0,
                    (items[item_idx].flags & ItemFlags::IF_ALWAYSEXP2) != 0,
                    items[item_idx].driver,
                    (items[item_idx].flags & ItemFlags::IF_LIGHTAGE) != 0,
                )
            });

        let should_age = (active == 0 && has_alwaysexp1) || (active == 1 && has_alwaysexp2);
        if !should_age {
            continue;
        }

        // Ice cloak ages more slowly when not worn or held
        if driver == 60 && unsafe { CLOCK4 % 4 != 0 } {
            continue;
        }

        Repository::with_items_mut(|items| {
            items[item_idx].current_age[active] += 1;
        });

        if has_lightage {
            lightage(item_idx, 1);
        }

        if item_age(item_idx) != 0 {
            must_update = true;
            age_message(cn, item_idx, "in your backpack");

            let damage_state = Repository::with_items(|items| items[item_idx].damage_state);
            if damage_state == 5 {
                Repository::with_characters_mut(|characters| {
                    characters[cn].item[n] = 0;
                });
                Repository::with_items_mut(|items| {
                    items[item_idx].used = USE_EMPTY;
                });
            }
        }
    }

    // Age items in worn slots (20 slots)
    for n in 0..20 {
        let item_idx = Repository::with_characters(|characters| characters[cn].worn[n] as usize);
        if item_idx == 0 {
            continue;
        }

        let (active, has_alwaysexp1, has_alwaysexp2, has_lightage) =
            Repository::with_items(|items| {
                let act = if items[item_idx].active != 0 { 1 } else { 0 };
                (
                    act,
                    (items[item_idx].flags & ItemFlags::IF_ALWAYSEXP1) != 0,
                    (items[item_idx].flags & ItemFlags::IF_ALWAYSEXP2) != 0,
                    (items[item_idx].flags & ItemFlags::IF_LIGHTAGE) != 0,
                )
            });

        let should_age = (active == 0 && has_alwaysexp1) || (active == 1 && has_alwaysexp2);
        if !should_age {
            continue;
        }

        Repository::with_items_mut(|items| {
            items[item_idx].current_age[active] += 1;
        });

        if has_lightage {
            lightage(item_idx, 1);
        }

        if item_age(item_idx) != 0 {
            must_update = true;
            let damage_state = Repository::with_items(|items| items[item_idx].damage_state);

            if damage_state == 5 {
                age_message(cn, item_idx, "you were using");
                Repository::with_characters_mut(|characters| {
                    characters[cn].worn[n] = 0;
                });
                Repository::with_items_mut(|items| {
                    items[item_idx].used = USE_EMPTY;
                });
            } else {
                age_message(cn, item_idx, "you are using");
            }
        }
    }

    // Age item under mouse cursor (citem)
    let citem = Repository::with_characters(|characters| characters[cn].citem);
    if citem != 0 && (citem & 0x80000000) == 0 {
        let item_idx = citem as usize;
        let (active, has_alwaysexp1, has_alwaysexp2, has_lightage) =
            Repository::with_items(|items| {
                let act = if items[item_idx].active != 0 { 1 } else { 0 };
                (
                    act,
                    (items[item_idx].flags & ItemFlags::IF_ALWAYSEXP1) != 0,
                    (items[item_idx].flags & ItemFlags::IF_ALWAYSEXP2) != 0,
                    (items[item_idx].flags & ItemFlags::IF_LIGHTAGE) != 0,
                )
            });

        let should_age = (active == 0 && has_alwaysexp1) || (active == 1 && has_alwaysexp2);
        if should_age {
            Repository::with_items_mut(|items| {
                items[item_idx].current_age[active] += 1;
            });

            if has_lightage {
                lightage(item_idx, 1);
            }

            if item_age(item_idx) != 0 {
                must_update = true;
                let damage_state = Repository::with_items(|items| items[item_idx].damage_state);

                if damage_state == 5 {
                    age_message(cn, item_idx, "you were using");
                    Repository::with_characters_mut(|characters| {
                        characters[cn].citem = 0;
                    });
                    Repository::with_items_mut(|items| {
                        items[item_idx].used = USE_EMPTY;
                    });
                } else {
                    age_message(cn, item_idx, "you are using");
                }
            }
        }
    }

    if must_update {
        // TODO: do_update_char(cn);
    }
}

pub fn may_deactivate(item_idx: usize) -> bool {
    use crate::repository::Repository;

    // Special check for driver 1 (create_item with mines)
    let driver = Repository::with_items(|items| items[item_idx].driver);
    if driver != 1 {
        return true;
    }

    // Check data[1-9] for mine states
    for n in 1..10 {
        let m = Repository::with_items(|items| items[item_idx].data[n]);
        if m == 0 {
            return true;
        }

        // TODO: Access map[m].it to check if mine exists and has correct driver
        // For now, assume it can deactivate
        // Original code checks:
        // if ((in2 = map[m].it) == 0) return 0;
        // if (it[in2].driver != 26) return 0;
    }

    true
}

pub fn pentagram(item_idx: usize) {
    use crate::god::God;
    use crate::populate::pop_create_char;
    use crate::repository::Repository;
    use core::constants::USE_EMPTY;
    use rand::Rng;

    let active = Repository::with_items(|items| items[item_idx].active);
    if active != 0 {
        return;
    }

    let mut rng = rand::thread_rng();
    if rng.gen_range(0..18) != 0 {
        return;
    }

    // Check data[1-3] for spawned enemies
    for n in 1..4 {
        let stored_cn = Repository::with_items(|items| items[item_idx].data[n]);

        // Check if slot is empty or enemy is dead
        let should_spawn = if stored_cn == 0 {
            true
        } else {
            Repository::with_characters(|characters| {
                let cn = stored_cn as usize;
                characters[cn].data[0] != item_idx as i32 || characters[cn].used == USE_EMPTY
                // TODO: Check CF_BODY flag
                // || (characters[cn].flags & CF_BODY) != 0
            })
        };

        if should_spawn {
            // TODO: Implement spawn_penta_enemy function
            // For now, create a basic enemy
            let new_cn = pop_create_char(364, 0); // Basic pentagram enemy template
            if new_cn != 0 {
                let (x, y) = Repository::with_items(|items| {
                    (items[item_idx].x as usize, items[item_idx].y as usize)
                });

                Repository::with_characters_mut(|characters| {
                    // TODO: Remove CF_RESPAWN flag
                    characters[new_cn].data[0] = item_idx as i32;
                    characters[new_cn].data[29] = (x + y * 512) as i32; // SERVER_MAPX
                    characters[new_cn].data[60] = 12 * 60 * 2; // TICKS * 60 * 2
                    characters[new_cn].data[73] = 8;
                    characters[new_cn].dir = 1;
                });

                if !God::drop_char_fuzzy(new_cn, x, y) {
                    God::destroy_items(new_cn);
                    Repository::with_characters_mut(|characters| {
                        characters[new_cn].used = USE_EMPTY;
                    });
                } else {
                    Repository::with_items_mut(|items| {
                        items[item_idx].data[n] = new_cn as i32;
                    });
                }
            }
            break;
        }
    }
}

pub fn spiderweb(item_idx: usize) {
    use crate::god::God;
    use crate::populate::pop_create_char;
    use crate::repository::Repository;
    use core::constants::USE_EMPTY;
    use rand::Rng;

    let active = Repository::with_items(|items| items[item_idx].active);
    if active != 0 {
        return;
    }

    let mut rng = rand::thread_rng();
    if rng.gen_range(0..60) != 0 {
        return;
    }

    // Check data[1-3] for spawned spiders
    for n in 1..4 {
        let stored_cn = Repository::with_items(|items| items[item_idx].data[n]);

        // Check if slot is empty or spider is dead
        let should_spawn = if stored_cn == 0 {
            true
        } else {
            Repository::with_characters(|characters| {
                let cn = stored_cn as usize;
                characters[cn].data[0] != item_idx as i32 || characters[cn].used == USE_EMPTY
                // TODO: Check CF_BODY flag
            })
        };

        if should_spawn {
            // Create spider (template 390-392)
            let spider_template = 390 + rng.gen_range(0..3);
            let cn = pop_create_char(spider_template, 0);
            if cn == 0 {
                continue;
            }

            let (x, y) = Repository::with_items(|items| {
                (items[item_idx].x as usize, items[item_idx].y as usize)
            });

            Repository::with_characters_mut(|characters| {
                // TODO: Remove CF_RESPAWN flag
                characters[cn].data[0] = item_idx as i32;
                characters[cn].data[29] = (x + y * 512) as i32; // SERVER_MAPX
                characters[cn].data[60] = 12 * 60 * 2; // TICKS * 60 * 2
                characters[cn].data[73] = 8;
                characters[cn].dir = 1;
            });

            if !God::drop_char_fuzzy(cn, x, y) {
                God::destroy_items(cn);
                Repository::with_characters_mut(|characters| {
                    characters[cn].used = USE_EMPTY;
                });
            } else {
                Repository::with_items_mut(|items| {
                    items[item_idx].data[n] = cn as i32;
                });
            }
            break;
        }
    }
}

pub fn greenlingball(item_idx: usize) {
    use crate::god::God;
    use crate::populate::pop_create_char;
    use crate::repository::Repository;
    use core::constants::USE_EMPTY;
    use rand::Rng;

    let active = Repository::with_items(|items| items[item_idx].active);
    if active != 0 {
        return;
    }

    let mut rng = rand::thread_rng();
    if rng.gen_range(0..20) != 0 {
        return;
    }

    // Check data[1-3] for spawned greenlings
    for n in 1..4 {
        let stored_cn = Repository::with_items(|items| items[item_idx].data[n]);

        // Check if slot is empty or greenling is dead
        let should_spawn = if stored_cn == 0 {
            true
        } else {
            Repository::with_characters(|characters| {
                let cn = stored_cn as usize;
                characters[cn].data[0] != item_idx as i32 || characters[cn].used == USE_EMPTY
                // TODO: Check CF_BODY flag
            })
        };

        if should_spawn {
            // Create greenling (template 553 + data[0])
            let greenling_type = Repository::with_items(|items| items[item_idx].data[0]);
            let cn = pop_create_char(553 + greenling_type, 0);
            if cn == 0 {
                continue;
            }

            let (x, y) = Repository::with_items(|items| {
                (items[item_idx].x as usize, items[item_idx].y as usize)
            });

            Repository::with_characters_mut(|characters| {
                // TODO: Remove CF_RESPAWN flag
                characters[cn].data[0] = item_idx as i32;
                characters[cn].data[29] = (x + y * 512) as i32; // SERVER_MAPX
                characters[cn].data[60] = 12 * 60 * 2; // TICKS * 60 * 2
                characters[cn].data[73] = 8;
                characters[cn].dir = 1;
            });

            if !God::drop_char_fuzzy(cn, x, y) {
                God::destroy_items(cn);
                Repository::with_characters_mut(|characters| {
                    characters[cn].used = USE_EMPTY;
                });
            } else {
                Repository::with_items_mut(|items| {
                    items[item_idx].data[n] = cn as i32;
                });
            }
            break;
        }
    }
}

pub fn expire_blood_penta(item_idx: usize) {}

pub fn expire_driver(item_idx: usize) {}

pub fn item_tick_expire() {}

pub fn item_tick_gc() {}

pub fn item_tick() {}

pub fn trap1(cn: usize, item_idx: usize) {}

pub fn trap2(cn: usize, item_idx: usize) {}

pub fn start_trap(cn: usize, item_idx: usize) {}

pub fn step_trap(cn: usize, item_idx: usize) -> i32 {}

pub fn step_trap_remove(cn: usize, item_idx: usize) {}

pub fn step_portal1_lab13(cn: usize, item_idx: usize) -> i32 {}

pub fn step_portal2_lab13(cn: usize, item_idx: usize) -> i32 {}

pub fn step_portal_arena(cn: usize, item_idx: usize) -> i32 {}

pub fn step_teleport(cn: usize, item_idx: usize) -> i32 {}

pub fn step_firefloor(cn: usize, item_idx: usize) -> i32 {}

pub fn step_firefloor_remove(cn: usize, item_idx: usize) {}

pub fn step_driver(cn: usize, item_idx: usize) -> i32 {}

pub fn step_driver_remove(cn: usize, item_idx: usize) {}
