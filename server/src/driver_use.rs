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
            let allowed = Repository::with_characters(|characters| characters[owner].data[65] as usize);
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
                (characters[owner].is_living_character(owner), characters[owner].x)
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
        (item.data[0] as i32, item.data[1] as usize, item.data[2] as i32)
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
            state.do_character_log(cn, core::types::FontColor::Green, "What do you want to do with it?\\n");
        });
        return 0;
    }

    // Check if rat eye is carried (not on ground)
    let carried = Repository::with_items(|items| items[item_idx].carried);
    if carried == 0 {
        State::with(|state| {
            state.do_character_log(cn, core::types::FontColor::Green, "Too difficult to do on the ground.\\n");
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
    let is_wielded = Repository::with_characters(|characters| {
        characters[cn].worn[WN_RHAND] == item_idx as u32
    });

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
    let has_skua_kindred = Repository::with_characters(|characters| {
        (characters[cn].kindred & 0x00000002) != 0
    });

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
    let is_wielded = Repository::with_characters(|characters| {
        characters[cn].worn[WN_RHAND] == item_idx as u32
    });

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
    let has_purple_kindred = Repository::with_characters(|characters| {
        (characters[cn].kindred & 0x00000001) != 0
    });

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
        361, 361, 339, 342, 345, 339, 342, 345, 359, 359,
        361, 361, 339, 342, 345, 339, 342, 345, 359, 359,
        361, 361, 339, 342, 345, 339, 342, 345, 359, 359,
        // Level 1 (30-59): silver, med jewels, golem
        361, 361, 361, 340, 343, 346, 371, 371, 371, 371,
        361, 361, 361, 340, 343, 346, 371, 371, 371, 371,
        361, 361, 361, 340, 343, 346, 371, 371, 371, 371,
        // Level 2 (60-89): gold, big jewels, gargoyle
        360, 341, 344, 347, 372, 372, 372, 487, 372, 372,
        360, 341, 344, 347, 372, 372, 372, 488, 372, 372,
        360, 341, 344, 347, 372, 372, 372, 489, 372, 372,
    ];

    // Check if already active (already searched)
    let is_active = Repository::with_items(|items| items[item_idx].active != 0);
    if is_active {
        return 0;
    }

    // Get pile info
    let (x, y, level) = Repository::with_items(|items| {
        (items[item_idx].x, items[item_idx].y, items[item_idx].data[0] as i32)
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
    if luck < 0 { chance += 1; }
    if luck <= -100 { chance += 1; }
    if luck <= -500 { chance += 1; }
    if luck <= -1000 { chance += 1; }
    if luck <= -2000 { chance += 1; }
    if luck <= -3000 { chance += 1; }
    if luck <= -4000 { chance += 1; }
    if luck <= -6000 { chance += 1; }
    if luck <= -8000 { chance += 1; }
    if luck <= -10000 { chance += 1; }

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
        let is_takeable = Repository::with_items(|items| {
            items[in2].flags.contains(ItemFlags::IF_TAKE)
        });

        if is_takeable {
            // Give to player
            if God::give_character_item(cn, in2) {
                let reference = Repository::with_items(|items| {
                    items[in2].reference.clone()
                });
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
        (items[in_idx].data[0] as usize, 
         items[in_idx].x, 
         items[in_idx].y, 
         items[in_idx].carried)
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
    use core::constants::{USE_EMPTY, SERVER_MAPX};

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
            state.do_character_log(
                cn,
                core::types::FontColor::Yellow,
                "Nothing happens.\\n",
            );
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
            state.do_character_log(
                cn,
                core::types::FontColor::Yellow,
                "That doesn't fit.\\n",
            );
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
    let (ch_x, ch_y) = Repository::with_characters(|characters| {
        (characters[cn].x, characters[cn].y)
    });

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
        characters[cc].data[63] = cn as i32;         // obey and protect char
        characters[cc].data[69] = cn as i32;         // follow char
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
    let (ch_x, ch_y) = Repository::with_characters(|characters| {
        (characters[cn].x, characters[cn].y)
    });

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
        characters[cc].data[63] = cn as i32;         // obey and protect char
        characters[cc].data[69] = cn as i32;         // follow char
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
            items[in_idx].description =
                format!("Level {} soulstone, holding {} exp.", rank, exp);
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

pub fn solved_pentagram(cn: usize, item_idx: usize) -> i32 {}

pub fn is_in_pentagram_quest(cn: usize) -> bool {}

pub fn use_pentagram(cn: usize, item_idx: usize) -> i32 {}

pub fn use_shrine(cn: usize, item_idx: usize) -> i32 {}

pub fn use_kill_undead(cn: usize, item_idx: usize) -> i32 {}

pub fn teleport3(cn: usize, item_idx: usize) -> i32 {}

pub fn use_seyan_shrine(cn: usize, item_idx: usize) -> i32 {}

pub fn use_seyan_door(cn: usize, item_idx: usize) -> i32 {}

pub fn use_seyan_portal(cn: usize, item_idx: usize) -> i32 {}

pub fn spell_scroll(cn: usize, item_idx: usize) -> i32 {}

pub fn use_blook_pentagram(cn: usize, item_idx: usize) -> i32 {}

pub fn use_create_npc(cn: usize, item_idx: usize) -> i32 {}

pub fn use_rotate(cn: usize, item_idx: usize) -> i32 {}

pub fn use_lab8_key(cn: usize, item_idx: usize) -> i32 {}

pub fn use_lab8_shrine(cn: usize, item_idx: usize) -> i32 {}

pub fn use_lab8_moneyshrine(cn: usize, item_idx: usize) -> i32 {}

pub fn change_to_archtemplar(cn: usize) {}

pub fn change_to_archharakim(cn: usize) {}

pub fn change_to_warrior(cn: usize) {}

pub fn change_to_sorcerer(cn: usize) {}

pub fn shrine_of_change(cn: usize, item_idx: usize) -> i32 {}

pub fn explorer_point(cn: usize, item_idx: usize) -> i32 {}

pub fn use_garbage(cn: usize, item_idx: usize) -> i32 {}

pub fn use_driver(cn: usize, item_idx: usize, carried: bool) {}

pub fn item_age(item_idx: usize) -> i32 {}

pub fn item_damage_worn(cn: usize, n: usize, damage: i32) {}

pub fn item_damage_citem(cn: usize, damage: i32) {}

pub fn item_damage_armor(cn: usize, damage: i32) {}

pub fn item_damage_weapon(cn: usize, damage: i32) {}

pub fn lightage(item_idx: usize, multi: i32) {}

pub fn age_message(cn: usize, item_idx: usize, where_is: &str) {}

pub fn char_item_expire(cn: usize) {}

pub fn may_deactivate(item_idx: usize) -> bool {}

pub fn pentagram(item_idx: usize) {}

pub fn spiderweb(item_idx: usize) {}

pub fn greenlingball(item_idx: usize) {}

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
