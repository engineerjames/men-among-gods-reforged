use crate::{
    driver, enums, god::God, network_manager::NetworkManager, repository::Repository,
    server::Server, state::State,
};

// Fast cut values (relative to the complete implementation)
const YSCUTF: i32 = 6; // YSCUT + 3
const YECUTF: i32 = 1; // YECUT
const XSCUTF: i32 = 2; // XSCUT
const XECUTF: i32 = 5; // XECUT + 3

const SPEEDTAB: [[u8; 20]; 20] = [
    [1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1],
    [1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1],
    [1, 1, 1, 1, 1, 0, 1, 1, 1, 1, 1, 1, 1, 1, 0, 1, 1, 1, 1, 1],
    [1, 1, 1, 0, 1, 1, 1, 1, 1, 0, 1, 1, 1, 1, 1, 1, 0, 1, 1, 1],
    [1, 1, 0, 1, 1, 1, 1, 0, 1, 1, 1, 1, 0, 1, 1, 1, 1, 0, 1, 1],
    [1, 1, 0, 1, 1, 1, 0, 1, 1, 1, 0, 1, 1, 1, 0, 1, 1, 1, 0, 1],
    [1, 0, 1, 1, 0, 1, 1, 1, 0, 1, 1, 0, 1, 1, 0, 1, 1, 0, 1, 1],
    [1, 0, 1, 1, 0, 1, 1, 0, 1, 1, 0, 1, 1, 0, 1, 1, 0, 1, 1, 0],
    [0, 1, 1, 0, 1, 1, 0, 1, 0, 1, 1, 0, 1, 1, 0, 1, 0, 1, 0, 1],
    [0, 1, 0, 1, 0, 1, 0, 1, 1, 0, 1, 0, 1, 0, 1, 0, 1, 1, 0, 1],
    [1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0],
    [1, 0, 1, 0, 1, 0, 1, 0, 0, 1, 0, 1, 0, 1, 0, 1, 0, 0, 1, 0],
    [1, 0, 0, 1, 0, 0, 1, 0, 1, 0, 0, 1, 0, 0, 1, 0, 1, 0, 1, 0],
    [0, 1, 0, 0, 1, 0, 0, 1, 0, 0, 1, 0, 0, 1, 0, 0, 1, 0, 0, 1],
    [0, 1, 0, 0, 1, 0, 0, 0, 1, 0, 0, 1, 0, 0, 1, 0, 0, 1, 0, 0],
    [0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0],
    [0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0],
    [0, 0, 0, 1, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0],
    [0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
];

/// Port of `plr_logout(int cn, int player_id, LogoutReason reason)` from `svr_tick.cpp`
///
/// Handles player logout and cleanup: saves state, removes the player
/// from maps, clears usurp/stoned flags, notifies the client (unless
/// `Usurp`), and applies any exit punishments depending on `reason`.
///
/// # Arguments
/// * `character_id` - Character index being logged out
/// * `player_id` - Associated player slot id (0 if none)
/// * `reason` - Reason for logout (enum)
pub fn plr_logout(character_id: usize, player_id: usize, reason: enums::LogoutReason) {
    if reason != enums::LogoutReason::Shutdown {
        Repository::with_characters(|characters| {
            log::debug!(
                "Logging out character '{}' for reason: {:?}",
                characters[character_id].get_name(),
                reason
            );
        });
    }

    let character_has_player = Repository::with_characters(|characters| {
        characters[character_id].player == player_id as i32
    });

    // Handle usurp flag and recursive logout
    if character_has_player {
        let should_logout_co = Repository::with_characters_mut(|characters| {
            let character = &mut characters[character_id];
            if character.flags & enums::CharacterFlags::Usurp.bits() != 0 {
                character.flags &= !(enums::CharacterFlags::ComputerControlledPlayer
                    | enums::CharacterFlags::Usurp
                    | enums::CharacterFlags::Staff
                    | enums::CharacterFlags::Immortal
                    | enums::CharacterFlags::God
                    | enums::CharacterFlags::Creator)
                    .bits();
                Some(character.data[97] as usize)
            } else {
                None
            }
        });

        if let Some(co) = should_logout_co {
            plr_logout(co, 0, enums::LogoutReason::Shutdown);
        }
    }

    // Main logout logic for active players
    if character_has_player {
        let (is_player, is_not_ccp) = Repository::with_characters(|characters| {
            let character = &characters[character_id];
            (
                character.flags & enums::CharacterFlags::Player.bits() != 0,
                character.flags & enums::CharacterFlags::ComputerControlledPlayer.bits() == 0,
            )
        });

        if is_player && is_not_ccp {
            // Handle exit punishment
            if reason == enums::LogoutReason::Exit {
                Repository::with_characters_mut(|characters| {
                    let character = &mut characters[character_id];
                    log::warn!(
                        "Character '{}' punished for leaving the game by means of F12.",
                        character.get_name(),
                    );

                    let damage_message = format!(
                        "You have been hit by a demon. You lost {} HP.\n",
                        (character.hp[5] * 8 / 10)
                    );
                    let messages_to_send = [
                        " \n",
                        "You are being punished for leaving the game without entering a tavern:\n",
                        " \n",
                        damage_message.as_str(),
                    ];

                    for i in 0..messages_to_send.len() {
                        State::with(|state| {
                            state.do_character_log(
                                character_id,
                                core::types::FontColor::Red,
                                messages_to_send[i],
                            );
                        });
                    }

                    character.a_hp -= (character.hp[5] * 800) as i32;

                    if character.a_hp < 500 {
                        State::with(|state| {
                            state.do_character_log(
                                character_id,
                                core::types::FontColor::Red,
                                String::from("The demon killed you.\n \n").as_str(),
                            );
                            state.do_character_killed(character_id, 0);
                        });
                    } else {
                        if character.gold / 10 > 0 {
                            let money_stolen_message = format!(
                                " \nA demon grabs your purse and removes {} gold, and {} silver.\n",
                                (character.gold / 10) / 100,
                                (character.gold / 10) % 100
                            );

                            State::with(|state| {
                                state.do_character_log(
                                    character_id,
                                    core::types::FontColor::Red,
                                    money_stolen_message.as_str(),
                                );
                            });
                            character.gold -= character.gold / 10;

                            if character.citem != 0 && character.citem & 0x80000000 == 0 {
                                State::with(|state| {
                                    state.do_character_log(
                                        character_id,
                                        core::types::FontColor::Red,
                                        "The demon also takes the money in your hand!\n",
                                    );
                                });

                                character.citem = 0;
                            }
                        }
                    }
                });
            }

            // Clear map positions
            let (map_index, to_map_index, light, character_x, character_y) =
                Repository::with_characters(|characters| {
                    let character = &characters[character_id];
                    let map_index = (character.y as usize) * core::constants::SERVER_MAPX as usize
                        + (character.x as usize);
                    let to_map_index = (character.toy as usize)
                        * core::constants::SERVER_MAPX as usize
                        + (character.tox as usize);
                    (
                        map_index,
                        to_map_index,
                        character.light,
                        character.x,
                        character.y,
                    )
                });

            Repository::with_map_mut(|map| {
                if map[map_index].ch == character_id as u32 {
                    map[map_index].ch = 0;
                    if light != 0 {
                        State::with_mut(|state| {
                            state.do_add_light(
                                character_x as i32,
                                character_y as i32,
                                -(light as i32),
                            );
                        });
                    }
                }

                if map[to_map_index].ch == character_id as u32 {
                    map[to_map_index].ch = 0;
                }
            });

            // Handle lag scroll
            if reason == enums::LogoutReason::IdleTooLong
                || reason == enums::LogoutReason::Shutdown
                || reason == enums::LogoutReason::Unknown
            {
                let (is_close_to_temple, map_index) = Repository::with_characters(|characters| {
                    let character = &characters[character_id];
                    let map_index = (character.y as usize) * core::constants::SERVER_MAPX as usize
                        + (character.x as usize);
                    (character.is_close_to_temple(), map_index)
                });

                let should_give = if !is_close_to_temple {
                    Repository::with_map(|map| {
                        map[map_index].flags & core::constants::MF_NOLAG as u64 == 0
                    })
                } else {
                    false
                };

                if should_give {
                    Repository::with_characters(|characters| {
                        log::info!(
                            "Giving lag scroll to character '{}' for idle/logout too long.",
                            characters[character_id].get_name(),
                        );
                    });

                    if let Some(item_id) = God::create_item(core::constants::IT_LAGSCROLL as usize)
                    {
                        let (char_x, char_y) = Repository::with_characters(|characters| {
                            (characters[character_id].x, characters[character_id].y)
                        });

                        Repository::with_items_mut(|items| {
                            items[item_id].data[0] = char_x as u32;
                            items[item_id].data[1] = char_y as u32;
                        });

                        Repository::with_globals(|globals| {
                            Repository::with_items_mut(|items| {
                                items[item_id].data[2] = globals.ticker as u32;
                            });
                        });

                        God::give_character_item(character_id, item_id);
                    } else {
                        Repository::with_characters(|characters| {
                            log::error!(
                                "Failed to create lag scroll for character '{}'.",
                                characters[character_id].get_name(),
                            );
                        });
                    }
                }
            }

            // Reset character state
            Repository::with_characters_mut(|characters| {
                let character = &mut characters[character_id];
                character.x = 0;
                character.y = 0;
                character.tox = 0;
                character.toy = 0;
                character.frx = 0;
                character.fry = 0;
                character.player = 0;
                character.status = 0;
                character.status2 = 0;
                character.dir = 0;
                character.escape_timer = 0;
                for i in 0..4 {
                    character.enemy[i] = 0;
                }
                character.attack_cn = 0;
                character.skill_nr = 0;
                character.goto_x = 0;
                character.goto_y = 0;
                character.use_nr = 0;
                character.misc_action = 0;
                character.stunned = 0;
                character.retry = 0;

                for i in 0..13 {
                    if i == 11 {
                        continue;
                    }
                    character.data[i] = 0;
                }

                character.data[96] = 0;
                character.used = core::constants::USE_NONACTIVE;
                character.logout_date = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs() as u32;

                character.flags |= enums::CharacterFlags::SaveMe.bits();

                if character.is_building() {
                    God::build(character_id, 0);
                }
            });
        }
    }

    // Send exit message to player
    if player_id != 0 && reason != enums::LogoutReason::Usurp {
        let mut buffer: [u8; 16] = [0; 16];
        buffer[0] = core::constants::SV_EXIT;
        buffer[1] = reason as u8;

        let player_state = Server::with_players(|players| players[player_id].state);

        if player_state == core::constants::ST_NORMAL {
            NetworkManager::with(|network| {
                network.xsend(player_id as usize, &buffer, 16);
            });
        } else {
            NetworkManager::with(|network| {
                network.csend(player_id as usize, &buffer, 16);
            });
        }

        player_exit(player_id);
    }
}

/// Finalize player exit operations and clear player slot state.
///
/// Called after `plr_logout` to complete exit bookkeeping: updates the
/// player's state, clears `ch.player`, and records the last tick.
///
/// # Arguments
/// * `player_id` - Player slot index
pub fn player_exit(player_id: usize) {
    if player_id == 0 || player_id >= core::constants::MAXPLAYER {
        log::error!("player_exit: Invalid player id {}", player_id);
        return;
    }

    let ticker = Repository::with_globals(|globals| globals.ticker as u32);

    Server::with_players_mut(|players| {
        players[player_id].state = core::constants::ST_EXIT;
        players[player_id].lasttick = ticker;

        Repository::with_characters_mut(|characters| {
            let char = characters
                .iter_mut()
                .find(|ch| ch.player as usize == player_id);

            if char.is_none() {
                log::error!("player_exit: No character found for player {}", player_id);
                return;
            }

            let char = char.unwrap();
            log::info!(
                "Player {} exiting for character '{}'",
                player_id,
                char.get_name()
            );

            char.player = 0;
        });
    });
}

/// Port of `plr_map_remove` from `svr_act.cpp`
///
/// Removes a character from the world map tile and clears any transient
/// references associated with that tile (to_ch, step-action items, lights).
/// It also undoes light contributions for the character and clears step
/// drivers for stepped-on items when appropriate.
///
/// # Arguments
/// * `cn` - Character index to remove from the map
pub fn plr_map_remove(cn: usize) {
    Repository::with_characters(|characters| {
        let m = (characters[cn].x as usize)
            + (characters[cn].y as usize) * core::constants::SERVER_MAPX as usize;
        let to_m = (characters[cn].tox as usize)
            + (characters[cn].toy as usize) * core::constants::SERVER_MAPX as usize;
        let light = characters[cn].light;
        let (x, y) = (characters[cn].x, characters[cn].y);
        let is_body = (characters[cn].flags & enums::CharacterFlags::Body.bits()) != 0;

        Repository::with_map_mut(|map| {
            map[m].ch = 0;
            map[to_m].to_ch = 0;

            if light != 0 {
                State::with_mut(|state| {
                    state.do_add_light(x as i32, y as i32, -(light as i32));
                });
            }

            if !is_body {
                let in_id = map[m].it;
                if in_id != 0 {
                    Repository::with_items(|items| {
                        if (items[in_id as usize].flags
                            & core::constants::ItemFlags::IF_STEPACTION.bits())
                            != 0
                        {
                            driver::step_driver_remove(cn, in_id as usize);
                        }
                    });
                }
            }
        });
    });
}

/// Port of `plr_map_set` from `svr_act.cpp`
///
/// Places a character on the map and handles tile interactions that occur
/// on arrival. This checks for step-action items (calling the step driver),
/// taverns (triggering logout/tavern logic), "no magic" zones (removing
/// spells and flagging the character), death traps (killing the character),
/// and finally notifies nearby clients of the character's presence.
///
/// The function will also restore the character to a previous tile when
/// teleport/step-driver returns special values, and updates lighting.
///
/// # Arguments
/// * `cn` - Character index to place on the map
pub fn plr_map_set(cn: usize) {
    let (x, y, flags, light) = Repository::with_characters(|characters| {
        (
            characters[cn].x,
            characters[cn].y,
            characters[cn].flags,
            characters[cn].light,
        )
    });

    let m = (x as usize) + (y as usize) * core::constants::SERVER_MAPX as usize;
    let is_body = (flags & enums::CharacterFlags::Body.bits()) != 0;
    let is_player = (flags & enums::CharacterFlags::Player.bits()) != 0;

    if !is_body {
        // Check for step action
        let in_id = Repository::with_map(|map| map[m].it);
        if in_id != 0 {
            let has_step_action = Repository::with_items(|items| {
                (items[in_id as usize].flags & core::constants::ItemFlags::IF_STEPACTION.bits())
                    != 0
            });

            if has_step_action {
                // Call step_driver and handle return values per original C++ logic
                let ret = driver::step_driver(cn, in_id as usize);

                if ret == 1 {
                    Repository::with_map_mut(|map| {
                        map[m].to_ch = 0;
                    });

                    // compute destination: x + (x - frx), y + (y - fry)
                    let (cx, cy, frx, fry, light) = Repository::with_characters(|characters| {
                        (
                            characters[cn].x as i32,
                            characters[cn].y as i32,
                            characters[cn].frx as i32,
                            characters[cn].fry as i32,
                            characters[cn].light,
                        )
                    });

                    let nx = cx + (cx - frx);
                    let ny = cy + (cy - fry);

                    let target_empty = Repository::with_map(|map| {
                        let idx =
                            (nx as usize) + (ny as usize) * core::constants::SERVER_MAPX as usize;
                        map[idx].ch == 0
                    });

                    if target_empty {
                        Repository::with_characters_mut(|characters| {
                            characters[cn].x = nx as i16;
                            characters[cn].y = ny as i16;
                            characters[cn].use_nr = 0;
                            characters[cn].skill_nr = 0;
                            characters[cn].attack_cn = 0;
                            characters[cn].goto_x = 0;
                            characters[cn].goto_y = 0;
                            characters[cn].misc_action = 0;
                        });

                        Repository::with_map_mut(|map| {
                            let idx = (nx as usize)
                                + (ny as usize) * core::constants::SERVER_MAPX as usize;
                            map[idx].ch = cn as u32;
                        });

                        if light != 0 {
                            State::with_mut(|state| {
                                state.do_add_light(nx as i32, ny as i32, light as i32);
                            });
                        }

                        return;
                    } else {
                        // fall through and handle as ret == -1
                    }
                }

                if ret == -1 {
                    Repository::with_map_mut(|map| {
                        map[m].to_ch = 0;
                    });

                    let (frx, fry, light) = Repository::with_characters(|characters| {
                        (
                            characters[cn].frx as i32,
                            characters[cn].fry as i32,
                            characters[cn].light,
                        )
                    });

                    Repository::with_characters_mut(|characters| {
                        characters[cn].x = frx as i16;
                        characters[cn].y = fry as i16;
                        characters[cn].use_nr = 0;
                        characters[cn].skill_nr = 0;
                        characters[cn].attack_cn = 0;
                        characters[cn].goto_x = 0;
                        characters[cn].goto_y = 0;
                        characters[cn].misc_action = 0;
                    });

                    Repository::with_map_mut(|map| {
                        let idx =
                            (frx as usize) + (fry as usize) * core::constants::SERVER_MAPX as usize;
                        map[idx].ch = cn as u32;
                    });

                    if light != 0 {
                        State::with_mut(|state| {
                            state.do_add_light(frx as i32, fry as i32, light as i32);
                        });
                    }

                    return;
                }

                if ret == 2 {
                    // TELEPORT_SUCCESS: just add light and return
                    if light != 0 {
                        State::with_mut(|state| {
                            state.do_add_light(x as i32, y as i32, light as i32);
                        });
                    }
                    return;
                }
            }
        }

        // Check for tavern
        let is_tavern =
            Repository::with_map(|map| (map[m].flags & core::constants::MF_TAVERN as u64) != 0);

        if is_tavern && is_player {
            Repository::with_characters_mut(|characters| {
                if characters[cn].is_building() {
                    God::build(cn, 0);
                }
                characters[cn].tavern_x = characters[cn].x as u16;
                characters[cn].tavern_y = characters[cn].y as u16;
            });

            log::info!("Character {} entered tavern", cn);

            let player_id = Repository::with_characters(|characters| characters[cn].player);
            plr_logout(cn, player_id as usize, enums::LogoutReason::Tavern);
            return;
        }

        // Check for no magic zone, respect items that exempt char from nomagic
        let is_nomagic =
            Repository::with_map(|map| (map[m].flags & core::constants::MF_NOMAGIC as u64) != 0);

        let wears_466 = State::with(|s| s.char_wears_item(cn, 466));
        let wears_481 = State::with(|s| s.char_wears_item(cn, 481));

        if is_nomagic && !wears_466 && !wears_481 {
            Repository::with_characters_mut(|characters| {
                if (characters[cn].flags & enums::CharacterFlags::NoMagic.bits()) == 0 {
                    characters[cn].flags |= enums::CharacterFlags::NoMagic.bits();
                }
            });

            // remove all spells and notify
            driver::remove_spells(cn);
            State::with(|state| {
                state.do_character_log(
                    cn,
                    core::types::FontColor::Red,
                    "You feel your magic fail.\n",
                );
            });
        } else {
            let mut was_nomagic = false;
            Repository::with_characters_mut(|characters| {
                if (characters[cn].flags & enums::CharacterFlags::NoMagic.bits()) != 0 {
                    characters[cn].flags &= !enums::CharacterFlags::NoMagic.bits();
                    characters[cn].set_do_update_flags();
                    was_nomagic = true;
                }
            });

            if was_nomagic {
                State::with(|state| {
                    state.do_character_log(
                        cn,
                        core::types::FontColor::Red,
                        "You feel your magic return.\n",
                    );
                });
            }
        }
    }

    // Set character on map
    Repository::with_map_mut(|map| {
        map[m].ch = cn as u32;
        map[m].to_ch = 0;
    });

    if !is_body {
        if light != 0 {
            State::with_mut(|state| {
                state.do_add_light(x as i32, y as i32, light as i32);
            });
        }

        // Check for death trap
        let is_deathtrap =
            Repository::with_map(|map| (map[m].flags & core::constants::MF_DEATHTRAP as u64) != 0);

        if is_deathtrap {
            State::with(|state| {
                state.do_character_log(
                    cn,
                    core::types::FontColor::Red,
                    "You entered a Deathtrap. You are dead!\n",
                );
                log::info!("Character {} entered a Deathtrap", cn);
                state.do_character_killed(cn, 0);
            });
            return;
        }
    }

    State::with(|state| {
        state.do_area_notify(
            cn as i32,
            0,
            x as i32,
            y as i32,
            core::constants::NT_SEE as i32,
            cn as i32,
            0,
            0,
            0,
        );
    });
}

/// Port of `plr_move_up` from `svr_act.cpp`
///
/// Performs a move action upwards for the given character. This removes the
/// character from its current tile, updates the previous position (frx,fry),
/// adjusts the y coordinate and target coordinates, then re-inserts the
/// character into the map via `plr_map_set` and marks the action as
/// successful.
///
/// # Arguments
/// * `cn` - Character index performing the move
pub fn plr_move_up(cn: usize) {
    plr_map_remove(cn);
    Repository::with_characters_mut(|characters| {
        characters[cn].frx = characters[cn].x;
        characters[cn].fry = characters[cn].y;
        characters[cn].y -= 1;
        characters[cn].tox = characters[cn].x;
        characters[cn].toy = characters[cn].y;
    });
    plr_map_set(cn);
    Repository::with_characters_mut(|characters| {
        characters[cn].cerrno = core::constants::ERR_SUCCESS as u16;
    });
}

/// Port of `plr_move_down` from `svr_act.cpp`
///
/// Performs a move action downwards for the given character and updates
/// internal position state similar to `plr_move_up`.
///
/// # Arguments
/// * `cn` - Character index performing the move
pub fn plr_move_down(cn: usize) {
    plr_map_remove(cn);
    Repository::with_characters_mut(|characters| {
        characters[cn].frx = characters[cn].x;
        characters[cn].fry = characters[cn].y;
        characters[cn].y += 1;
        characters[cn].tox = characters[cn].x;
        characters[cn].toy = characters[cn].y;
    });
    plr_map_set(cn);
    Repository::with_characters_mut(|characters| {
        characters[cn].cerrno = core::constants::ERR_SUCCESS as u16;
    });
}

/// Port of `plr_move_left` from `svr_act.cpp`
///
/// Performs a move action left for the given character and updates
/// position and map state as in other move helpers.
///
/// # Arguments
/// * `cn` - Character index performing the move
pub fn plr_move_left(cn: usize) {
    plr_map_remove(cn);
    Repository::with_characters_mut(|characters| {
        characters[cn].frx = characters[cn].x;
        characters[cn].fry = characters[cn].y;
        characters[cn].x -= 1;
        characters[cn].tox = characters[cn].x;
        characters[cn].toy = characters[cn].y;
    });
    plr_map_set(cn);
    Repository::with_characters_mut(|characters| {
        characters[cn].cerrno = core::constants::ERR_SUCCESS as u16;
    });
}

/// Port of `plr_move_right` from `svr_act.cpp`
///
/// Performs a move action right for the given character and updates
/// position and map state as in other move helpers.
///
/// # Arguments
/// * `cn` - Character index performing the move
pub fn plr_move_right(cn: usize) {
    plr_map_remove(cn);
    Repository::with_characters_mut(|characters| {
        characters[cn].frx = characters[cn].x;
        characters[cn].fry = characters[cn].y;
        characters[cn].x += 1;
        characters[cn].tox = characters[cn].x;
        characters[cn].toy = characters[cn].y;
    });
    plr_map_set(cn);
    Repository::with_characters_mut(|characters| {
        characters[cn].cerrno = core::constants::ERR_SUCCESS as u16;
    });
}

/// Port of `plr_move_leftup` from `svr_act.cpp`
///
/// Performs a diagonal up-left move for the character and updates map state.
///
/// # Arguments
/// * `cn` - Character index performing the move
pub fn plr_move_leftup(cn: usize) {
    plr_map_remove(cn);
    Repository::with_characters_mut(|characters| {
        characters[cn].frx = characters[cn].x;
        characters[cn].fry = characters[cn].y;
        characters[cn].x -= 1;
        characters[cn].y -= 1;
        characters[cn].tox = characters[cn].x;
        characters[cn].toy = characters[cn].y;
    });
    plr_map_set(cn);
    Repository::with_characters_mut(|characters| {
        characters[cn].cerrno = core::constants::ERR_SUCCESS as u16;
    });
}

/// Port of `plr_move_leftdown` from `svr_act.cpp`
///
/// Performs a diagonal down-left move for the character and updates map state.
///
/// # Arguments
/// * `cn` - Character index performing the move
pub fn plr_move_leftdown(cn: usize) {
    plr_map_remove(cn);
    Repository::with_characters_mut(|characters| {
        characters[cn].frx = characters[cn].x;
        characters[cn].fry = characters[cn].y;
        characters[cn].x -= 1;
        characters[cn].y += 1;
        characters[cn].tox = characters[cn].x;
        characters[cn].toy = characters[cn].y;
    });
    plr_map_set(cn);
    Repository::with_characters_mut(|characters| {
        characters[cn].cerrno = core::constants::ERR_SUCCESS as u16;
    });
}

/// Port of `plr_move_rightup` from `svr_act.cpp`
///
/// Performs a diagonal up-right move for the character and updates map state.
///
/// # Arguments
/// * `cn` - Character index performing the move
pub fn plr_move_rightup(cn: usize) {
    plr_map_remove(cn);
    Repository::with_characters_mut(|characters| {
        characters[cn].frx = characters[cn].x;
        characters[cn].fry = characters[cn].y;
        characters[cn].x += 1;
        characters[cn].y -= 1;
        characters[cn].tox = characters[cn].x;
        characters[cn].toy = characters[cn].y;
    });
    plr_map_set(cn);
    Repository::with_characters_mut(|characters| {
        characters[cn].cerrno = core::constants::ERR_SUCCESS as u16;
    });
}

/// Port of `plr_move_rightdown` from `svr_act.cpp`
///
/// Performs a diagonal down-right move for the character and updates map state.
///
/// # Arguments
/// * `cn` - Character index performing the move
pub fn plr_move_rightdown(cn: usize) {
    plr_map_remove(cn);
    Repository::with_characters_mut(|characters| {
        characters[cn].frx = characters[cn].x;
        characters[cn].fry = characters[cn].y;
        characters[cn].x += 1;
        characters[cn].y += 1;
        characters[cn].tox = characters[cn].x;
        characters[cn].toy = characters[cn].y;
    });
    plr_map_set(cn);
    Repository::with_characters_mut(|characters| {
        characters[cn].cerrno = core::constants::ERR_SUCCESS as u16;
    });
}

/// Port of `plr_turn_up` from `svr_act.cpp`
///
/// Sets the character's facing direction to up and notifies nearby
/// observers about the change via area notification.
///
/// # Arguments
/// * `cn` - Character index rotating to face up
pub fn plr_turn_up(cn: usize) {
    Repository::with_characters_mut(|characters| {
        State::with(|state| {
            state.do_area_notify(
                cn as i32,
                0,
                characters[cn].x as i32,
                characters[cn].y as i32,
                core::constants::NT_SEE as i32,
                cn as i32,
                0,
                0,
                0,
            );
        });
        characters[cn].dir = core::constants::DX_UP;
        characters[cn].cerrno = core::constants::ERR_SUCCESS as u16;
    });
}

/// Port of `plr_turn_leftup` from `svr_act.cpp`
///
/// Sets the character's facing direction to left-up and notifies nearby
/// observers about the change.
///
/// # Arguments
/// * `cn` - Character index rotating to face left-up
pub fn plr_turn_leftup(cn: usize) {
    Repository::with_characters_mut(|characters| {
        State::with(|state| {
            state.do_area_notify(
                cn as i32,
                0,
                characters[cn].x as i32,
                characters[cn].y as i32,
                core::constants::NT_SEE as i32,
                cn as i32,
                0,
                0,
                0,
            );
        });
        characters[cn].dir = core::constants::DX_LEFTUP;
        characters[cn].cerrno = core::constants::ERR_SUCCESS as u16;
    });
}

/// Port of `plr_turn_leftdown` from `svr_act.cpp`
///
/// Sets the character's facing direction to left-down and notifies nearby
/// observers about the change.
///
/// # Arguments
/// * `cn` - Character index rotating to face left-down
pub fn plr_turn_leftdown(cn: usize) {
    Repository::with_characters_mut(|characters| {
        State::with(|state| {
            state.do_area_notify(
                cn as i32,
                0,
                characters[cn].x as i32,
                characters[cn].y as i32,
                core::constants::NT_SEE as i32,
                cn as i32,
                0,
                0,
                0,
            );
        });
        characters[cn].dir = core::constants::DX_LEFTDOWN;
        characters[cn].cerrno = core::constants::ERR_SUCCESS as u16;
    });
}

/// Port of `plr_turn_down` from `svr_act.cpp`
///
/// Sets the character's facing direction to down and notifies nearby
/// observers about the change.
///
/// # Arguments
/// * `cn` - Character index rotating to face down
pub fn plr_turn_down(cn: usize) {
    Repository::with_characters_mut(|characters| {
        State::with(|state| {
            state.do_area_notify(
                cn as i32,
                0,
                characters[cn].x as i32,
                characters[cn].y as i32,
                core::constants::NT_SEE as i32,
                cn as i32,
                0,
                0,
                0,
            );
        });
        characters[cn].dir = core::constants::DX_DOWN;
        characters[cn].cerrno = core::constants::ERR_SUCCESS as u16;
    });
}

/// Port of `plr_turn_rightdown` from `svr_act.cpp`
///
/// Sets the character's facing direction to right-down and notifies nearby
/// observers about the change.
///
/// # Arguments
/// * `cn` - Character index rotating to face right-down
pub fn plr_turn_rightdown(cn: usize) {
    Repository::with_characters_mut(|characters| {
        State::with(|state| {
            state.do_area_notify(
                cn as i32,
                0,
                characters[cn].x as i32,
                characters[cn].y as i32,
                core::constants::NT_SEE as i32,
                cn as i32,
                0,
                0,
                0,
            );
        });
        characters[cn].dir = core::constants::DX_RIGHTDOWN;
        characters[cn].cerrno = core::constants::ERR_SUCCESS as u16;
    });
}

/// Port of `plr_turn_rightup` from `svr_act.cpp`
///
/// Sets the character's facing direction to right-up and notifies nearby
/// observers about the change.
///
/// # Arguments
/// * `cn` - Character index rotating to face right-up
pub fn plr_turn_rightup(cn: usize) {
    Repository::with_characters_mut(|characters| {
        State::with(|state| {
            state.do_area_notify(
                cn as i32,
                0,
                characters[cn].x as i32,
                characters[cn].y as i32,
                core::constants::NT_SEE as i32,
                cn as i32,
                0,
                0,
                0,
            );
        });
        characters[cn].dir = core::constants::DX_RIGHTUP;
        characters[cn].cerrno = core::constants::ERR_SUCCESS as u16;
    });
}

/// Port of `plr_turn_left` from `svr_act.cpp`
///
/// Sets the character's facing direction to left and notifies nearby
/// observers about the change.
///
/// # Arguments
/// * `cn` - Character index rotating to face left
pub fn plr_turn_left(cn: usize) {
    Repository::with_characters_mut(|characters| {
        State::with(|state| {
            state.do_area_notify(
                cn as i32,
                0,
                characters[cn].x as i32,
                characters[cn].y as i32,
                core::constants::NT_SEE as i32,
                cn as i32,
                0,
                0,
                0,
            );
        });
        characters[cn].dir = core::constants::DX_LEFT;
        characters[cn].cerrno = core::constants::ERR_SUCCESS as u16;
    });
}

/// Port of `plr_turn_right` from `svr_act.cpp`
///
/// Sets the character's facing direction to right and notifies nearby
/// observers about the change.
///
/// # Arguments
/// * `cn` - Character index rotating to face right
pub fn plr_turn_right(cn: usize) {
    Repository::with_characters_mut(|characters| {
        State::with(|state| {
            state.do_area_notify(
                cn as i32,
                0,
                characters[cn].x as i32,
                characters[cn].y as i32,
                core::constants::NT_SEE as i32,
                cn as i32,
                0,
                0,
                0,
            );
        });
        characters[cn].dir = core::constants::DX_RIGHT;
        characters[cn].cerrno = core::constants::ERR_SUCCESS as u16;
    });
}

/// Port of `plr_attack` from `svr_act.cpp`
///
/// Attempts to attack the tile directly in front of the character (based on
/// facing direction). If a valid target character `co` is present and matches
/// the currently set `attack_cn`, the server triggers `do_attack` to perform
/// combat logic. If the target moved away, a message is sent to the attacker.
///
/// # Arguments
/// * `cn` - Attacking character index
/// * `surround` - Surround flag passed to `do_attack` (0 or 1)
pub fn plr_attack(cn: usize, surround: i32) {
    let (mut x, mut y, dir) = Repository::with_characters(|characters| {
        State::with(|state| {
            state.do_area_notify(
                cn as i32,
                0,
                characters[cn].x as i32,
                characters[cn].y as i32,
                core::constants::NT_SEE as i32,
                cn as i32,
                0,
                0,
                0,
            );
        });
        (
            characters[cn].x as i32,
            characters[cn].y as i32,
            characters[cn].dir,
        )
    });

    match dir {
        core::constants::DX_UP => y -= 1,
        core::constants::DX_DOWN => y += 1,
        core::constants::DX_LEFT => x -= 1,
        core::constants::DX_RIGHT => x += 1,
        _ => {
            log::error!("plr_attack: unknown dir {} for char {}", dir, cn);
            Repository::with_characters_mut(|characters| {
                characters[cn].cerrno = core::constants::ERR_FAILED as u16;
            });
            return;
        }
    }

    let m = (x as usize) + (y as usize) * core::constants::SERVER_MAPX as usize;
    let mut co = Repository::with_map(|map| map[m].ch as usize);

    if co == 0 {
        co = Repository::with_map(|map| map[m].to_ch as usize);
    }

    if co == 0 {
        co = Repository::with_characters(|characters| {
            let attack_cn = characters[cn].attack_cn as usize;
            if attack_cn > 0
                && characters[attack_cn].frx == x as i16
                && characters[attack_cn].fry == y as i16
            {
                attack_cn
            } else {
                0
            }
        });
    }

    if co == 0 {
        State::with(|state| {
            state.do_character_log(cn, core::types::FontColor::Red, "Your target moved away!\n");
        });
        return;
    }

    let attack_cn = Repository::with_characters(|characters| characters[cn].attack_cn as usize);

    if attack_cn == co {
        State::with_mut(|state| {
            state.do_attack(cn, co, surround);
        });
    }
}

/// Port of `plr_give` from `svr_act.cpp`
///
/// Attempts to give the currently carried item to the character in the tile
/// in front of the actor. If the target moved away or the direction is
/// invalid, an error is set; otherwise `do_give` is invoked to handle transfer
/// rules and client updates.
///
/// # Arguments
/// * `cn` - Giver character index
pub fn plr_give(cn: usize) {
    let (mut x, mut y, dir) = Repository::with_characters(|characters| {
        State::with(|state| {
            state.do_area_notify(
                cn as i32,
                0,
                characters[cn].x as i32,
                characters[cn].y as i32,
                core::constants::NT_SEE as i32,
                cn as i32,
                0,
                0,
                0,
            );
        });
        (
            characters[cn].x as i32,
            characters[cn].y as i32,
            characters[cn].dir,
        )
    });

    match dir {
        core::constants::DX_UP => y -= 1,
        core::constants::DX_DOWN => y += 1,
        core::constants::DX_LEFT => x -= 1,
        core::constants::DX_RIGHT => x += 1,
        _ => {
            log::error!("plr_give: Unknown dir {} for char {}", dir, cn);
            Repository::with_characters_mut(|characters| {
                characters[cn].cerrno = core::constants::ERR_FAILED as u16;
            });
            return;
        }
    }

    let m = (x as usize) + (y as usize) * core::constants::SERVER_MAPX as usize;
    let mut co = Repository::with_map(|map| map[m].ch as usize);

    if co == 0 {
        co = Repository::with_map(|map| map[m].to_ch as usize);
    }

    if co == 0 {
        State::with(|state| {
            state.do_character_log(cn, core::types::FontColor::Red, "Your target moved away!\n");
        });
        return;
    }

    State::with(|state| {
        state.do_give(cn, co);
    });
}

/// Port of `plr_pickup` from `svr_act.cpp`
///
/// Handles picking up an item from the adjacent tile in the character's
/// facing direction. This checks for available slots, money vs items,
/// step-action items blocking pickup, and updates character inventory,
/// money, and lighting appropriately.
///
/// # Arguments
/// * `cn` - Character index attempting to pick up an item
pub fn plr_pickup(cn: usize) {
    Repository::with_characters(|characters| {
        State::with(|state| {
            state.do_area_notify(
                cn as i32,
                0,
                characters[cn].x as i32,
                characters[cn].y as i32,
                core::constants::NT_SEE as i32,
                cn as i32,
                0,
                0,
                0,
            );
        });
    });

    let has_citem = Repository::with_characters(|characters| characters[cn].citem != 0);

    if has_citem {
        Repository::with_characters_mut(|characters| {
            characters[cn].cerrno = core::constants::ERR_FAILED as u16;
        });
        return;
    }

    let (m, x, y) = Repository::with_characters(|characters| {
        let dir = characters[cn].dir;
        let (m, x, y) = match dir {
            core::constants::DX_UP if characters[cn].y > 0 => {
                let m = (characters[cn].x as usize)
                    + ((characters[cn].y - 1) as usize) * core::constants::SERVER_MAPX as usize;
                (Some(m), characters[cn].x, characters[cn].y - 1)
            }
            core::constants::DX_DOWN
                if characters[cn].y < (core::constants::SERVER_MAPY as i16 - 1) =>
            {
                let m = (characters[cn].x as usize)
                    + ((characters[cn].y + 1) as usize) * core::constants::SERVER_MAPX as usize;
                (Some(m), characters[cn].x, characters[cn].y + 1)
            }
            core::constants::DX_LEFT if characters[cn].x > 0 => {
                let m = ((characters[cn].x - 1) as usize)
                    + (characters[cn].y as usize) * core::constants::SERVER_MAPX as usize;
                (Some(m), characters[cn].x - 1, characters[cn].y)
            }
            core::constants::DX_RIGHT
                if characters[cn].x < (core::constants::SERVER_MAPX as i16 - 1) =>
            {
                let m = ((characters[cn].x + 1) as usize)
                    + (characters[cn].y as usize) * core::constants::SERVER_MAPX as usize;
                (Some(m), characters[cn].x + 1, characters[cn].y)
            }
            _ => (None, 0, 0),
        };
        (m, x, y)
    });

    let Some(m) = m else {
        Repository::with_characters_mut(|characters| {
            characters[cn].cerrno = core::constants::ERR_FAILED as u16;
        });
        return;
    };

    let in_id = Repository::with_map(|map| map[m].it);

    if in_id == 0 {
        Repository::with_characters_mut(|characters| {
            characters[cn].cerrno = core::constants::ERR_FAILED as u16;
        });
        return;
    }

    let can_take = Repository::with_items(|items| {
        (items[in_id as usize].flags & core::constants::ItemFlags::IF_TAKE.bits()) != 0
    });

    if !can_take {
        Repository::with_characters_mut(|characters| {
            characters[cn].cerrno = core::constants::ERR_FAILED as u16;
        });
        return;
    }

    Repository::with_characters_mut(|characters| {
        characters[cn].cerrno = core::constants::ERR_SUCCESS as u16;
    });

    State::with(|state| state.do_update_char(cn));

    // Check if it's money
    let is_money = Repository::with_items(|items| {
        (items[in_id as usize].flags & core::constants::ItemFlags::IF_MONEY.bits()) != 0
    });

    if is_money {
        let value = Repository::with_items(|items| items[in_id as usize].value);

        Repository::with_characters_mut(|characters| {
            characters[cn].gold += value as i32;
        });

        State::with(|state| {
            let message = format!("You got {}G {}S\n", value / 100, value % 100);
            state.do_character_log(cn, core::types::FontColor::Red, &message);
        });

        log::info!("Character {} took {}G {}S", cn, value / 100, value % 100);

        Repository::with_map_mut(|map| {
            map[m].it = 0;
        });

        let (active, light_active, light_inactive) = Repository::with_items(|items| {
            (
                items[in_id as usize].active,
                items[in_id as usize].light[1],
                items[in_id as usize].light[0],
            )
        });

        Repository::with_items_mut(|items| {
            items[in_id as usize].used = core::constants::USE_EMPTY;
            items[in_id as usize].x = 0;
            items[in_id as usize].y = 0;
        });

        if active != 0 && light_active != 0 {
            State::with_mut(|state| {
                state.do_add_light(x as i32, y as i32, -(light_active as i32));
            });
        } else if light_inactive != 0 {
            State::with_mut(|state| {
                state.do_add_light(x as i32, y as i32, -(light_inactive as i32));
            });
        }

        return;
    }

    // Non-money item
    Repository::with_map_mut(|map| {
        map[m].it = 0;
    });

    let is_player = Repository::with_characters(|characters| {
        (characters[cn].flags & enums::CharacterFlags::Player.bits()) != 0
    });

    if is_player {
        let slot_found = Repository::with_characters_mut(|characters| {
            for n in 0..40 {
                if characters[cn].item[n] == 0 {
                    characters[cn].item[n] = in_id as u32;
                    return Some(n);
                }
            }
            None
        });

        if slot_found.is_none() {
            Repository::with_characters_mut(|characters| {
                characters[cn].citem = in_id as u32;
            });
        }

        let item_name = Repository::with_items(|items| items[in_id as usize].name.clone());
        log::info!(
            "Character {} took {}",
            cn,
            String::from_utf8_lossy(&item_name)
        );
    } else {
        Repository::with_characters_mut(|characters| {
            characters[cn].citem = in_id as u32;
        });
    }

    let (active, light_active, light_inactive) = Repository::with_items(|items| {
        (
            items[in_id as usize].active,
            items[in_id as usize].light[1],
            items[in_id as usize].light[0],
        )
    });

    Repository::with_items_mut(|items| {
        items[in_id as usize].x = 0;
        items[in_id as usize].y = 0;
        items[in_id as usize].carried = cn as u16;
    });

    if active != 0 && light_active != 0 {
        State::with_mut(|state| {
            state.do_add_light(x as i32, y as i32, -(light_active as i32));
        });
    } else if light_inactive != 0 {
        State::with_mut(|state| {
            state.do_add_light(x as i32, y as i32, -(light_inactive as i32));
        });
    }
}

/// Port of `plr_bow` from `svr_act.cpp`
///
/// Handles a social "bow" action: notifies nearby players with an area
/// notification and logs a message for the actor and area. Sets the
/// command result status to success.
///
/// # Arguments
/// * `cn` - Character index performing the bow
pub fn plr_bow(cn: usize) {
    Repository::with_characters(|characters| {
        State::with(|state| {
            state.do_area_notify(
                cn as i32,
                0,
                characters[cn].x as i32,
                characters[cn].y as i32,
                core::constants::NT_SEE as i32,
                cn as i32,
                0,
                0,
                0,
            );
            state.do_character_log(cn, core::types::FontColor::Red, "You bow deeply.\n");
            state.do_area_log(
                cn,
                0,
                characters[cn].x as i32,
                characters[cn].y as i32,
                core::types::FontColor::Blue,
                &format!(
                    "{} bows deeply.\n",
                    String::from_utf8_lossy(&characters[cn].reference)
                ),
            );
        });
    });

    log::info!("Character {} bows", cn);

    Repository::with_characters_mut(|characters| {
        characters[cn].cerrno = core::constants::ERR_SUCCESS as u16;
    });
}

/// Port of `plr_wave` from `svr_act.cpp`
///
/// Handles a social "wave" action: notifies nearby players with an area
/// notification and logs a message for the actor and area. Sets the
/// command result status to success.
///
/// # Arguments
/// * `cn` - Character index performing the wave
pub fn plr_wave(cn: usize) {
    Repository::with_characters(|characters| {
        State::with(|state| {
            state.do_area_notify(
                cn as i32,
                0,
                characters[cn].x as i32,
                characters[cn].y as i32,
                core::constants::NT_SEE as i32,
                cn as i32,
                0,
                0,
                0,
            );
            state.do_character_log(cn, core::types::FontColor::Red, "You wave happily.\n");
            state.do_area_log(
                cn,
                0,
                characters[cn].x as i32,
                characters[cn].y as i32,
                core::types::FontColor::Blue,
                &format!(
                    "{} waves happily.\n",
                    String::from_utf8_lossy(&characters[cn].reference)
                ),
            );
        });
    });

    log::info!("Character {} waves", cn);

    Repository::with_characters_mut(|characters| {
        characters[cn].cerrno = core::constants::ERR_SUCCESS as u16;
    });
}

/// Port of `plr_use` from `svr_act.cpp`
///
/// Attempts to use an item placed on the adjacent tile in front of the
/// actor. Validates usage flags and, when implemented, would call the
/// `use_driver` to perform item-specific logic. Currently it validates
/// and logs debug information.
///
/// # Arguments
/// * `cn` - Character index using the item
pub fn plr_use(cn: usize) {
    Repository::with_characters(|characters| {
        State::with(|state| {
            state.do_area_notify(
                cn as i32,
                0,
                characters[cn].x as i32,
                characters[cn].y as i32,
                core::constants::NT_SEE as i32,
                cn as i32,
                0,
                0,
                0,
            );
        });
    });

    let m = Repository::with_characters(|characters| {
        let dir = characters[cn].dir;
        let m = match dir {
            core::constants::DX_UP if characters[cn].y > 0 => Some(
                (characters[cn].x as usize)
                    + ((characters[cn].y - 1) as usize) * core::constants::SERVER_MAPX as usize,
            ),
            core::constants::DX_DOWN
                if characters[cn].y < (core::constants::SERVER_MAPY as i16 - 1) =>
            {
                Some(
                    (characters[cn].x as usize)
                        + ((characters[cn].y + 1) as usize) * core::constants::SERVER_MAPX as usize,
                )
            }
            core::constants::DX_LEFT if characters[cn].x > 0 => Some(
                ((characters[cn].x - 1) as usize)
                    + (characters[cn].y as usize) * core::constants::SERVER_MAPX as usize,
            ),
            core::constants::DX_RIGHT
                if characters[cn].x < (core::constants::SERVER_MAPX as i16 - 1) =>
            {
                Some(
                    ((characters[cn].x + 1) as usize)
                        + (characters[cn].y as usize) * core::constants::SERVER_MAPX as usize,
                )
            }
            _ => None,
        };

        m
    });

    let Some(m) = m else {
        Repository::with_characters_mut(|characters| {
            characters[cn].cerrno = core::constants::ERR_FAILED as u16;
        });
        return;
    };

    let in_id = Repository::with_map(|map| map[m].it);

    if in_id == 0 {
        Repository::with_characters_mut(|characters| {
            characters[cn].cerrno = core::constants::ERR_FAILED as u16;
        });
        return;
    }

    let can_use = Repository::with_items(|items| {
        let flags = items[in_id as usize].flags;
        (flags & core::constants::ItemFlags::IF_USE.bits()) != 0
            || (flags & core::constants::ItemFlags::IF_USESPECIAL.bits()) != 0
    });

    if !can_use {
        Repository::with_characters_mut(|characters| {
            characters[cn].cerrno = core::constants::ERR_FAILED as u16;
        });
        return;
    }

    driver::use_driver(cn, in_id as usize, false);
}

/// Port of `plr_skill` from `svr_act.cpp`
///
/// Triggers the skill driver for the character using the current
/// `skill_target2` value. Also sends an area notify for the action.
///
/// # Arguments
/// * `cn` - Character index using the skill
pub fn plr_skill(cn: usize) {
    Repository::with_characters(|characters| {
        State::with(|state| {
            state.do_area_notify(
                cn as i32,
                0,
                characters[cn].x as i32,
                characters[cn].y as i32,
                core::constants::NT_SEE as i32,
                cn as i32,
                0,
                0,
                0,
            );
        });
    });

    let skill_target = Repository::with_characters(|characters| characters[cn].skill_target2);

    driver::skill_driver(cn, skill_target as i32);
}

/// Periodic driver invoked at a medium rate for a player.
///
/// This function uses a rate-limiter (character data[12]) to avoid running
/// too often. When appropriate it will call the follow driver if the
/// character has a follow target set in `data[10]`.
///
/// # Arguments
/// * `cn` - Character index to process
pub fn player_driver_med(cn: usize) {
    Repository::with_characters(|ch| {
        if ch[cn].data[12] + core::constants::TICKS * 15
            > Repository::with_globals(|globs| globs.ticker)
        {
            return;
        }

        let co = ch[cn].data[10];

        if co != 0 {
            driver::follow_driver(cn, co as usize);
        }
    });
}

/// Client list stub (not implemented)
///
/// Placeholder for the client list command – intended to handle listing
/// connected clients or similar functionality in the original server.
pub fn cl_list() {}

/// Port of `plr_drop` from `svr_act.cpp`
///
/// Drops the currently carried item (cursor/item in hand) onto the tile in
/// front of the character. Handles special cases for money (creates a
/// money-item template), building-mode drop semantics, step-action
/// blockages, and updates lighting and map item references accordingly.
///
/// # Arguments
/// * `cn` - Character index performing the drop
pub fn plr_drop(cn: usize) {
    Repository::with_characters(|characters| {
        State::with(|state| {
            state.do_area_notify(
                cn as i32,
                0,
                characters[cn].x as i32,
                characters[cn].y as i32,
                core::constants::NT_SEE as i32,
                cn as i32,
                0,
                0,
                0,
            );
        });
    });

    let in_id = Repository::with_characters(|characters| characters[cn].citem);

    if in_id == 0 {
        return;
    }

    let (m, x, y) = Repository::with_characters(|characters| match characters[cn].dir {
        core::constants::DX_UP if characters[cn].y > 0 => {
            let m = (characters[cn].x as usize)
                + ((characters[cn].y - 1) as usize) * core::constants::SERVER_MAPX as usize;
            (Some(m), characters[cn].x, characters[cn].y - 1)
        }
        core::constants::DX_DOWN
            if characters[cn].y < (core::constants::SERVER_MAPY as i16 - 1) =>
        {
            let m = (characters[cn].x as usize)
                + ((characters[cn].y + 1) as usize) * core::constants::SERVER_MAPX as usize;
            (Some(m), characters[cn].x, characters[cn].y + 1)
        }
        core::constants::DX_LEFT if characters[cn].x > 0 => {
            let m = ((characters[cn].x - 1) as usize)
                + (characters[cn].y as usize) * core::constants::SERVER_MAPX as usize;
            (Some(m), characters[cn].x - 1, characters[cn].y)
        }
        core::constants::DX_RIGHT
            if characters[cn].x < (core::constants::SERVER_MAPX as i16 - 1) =>
        {
            let m = ((characters[cn].x + 1) as usize)
                + (characters[cn].y as usize) * core::constants::SERVER_MAPX as usize;
            (Some(m), characters[cn].x + 1, characters[cn].y)
        }
        _ => (None, 0, 0),
    });

    let Some(m) = m else {
        Repository::with_characters_mut(|characters| {
            characters[cn].cerrno = core::constants::ERR_FAILED as u16;
        });
        return;
    };

    // Check for step action items
    let in2 = Repository::with_map(|map| map[m].it);
    if in2 != 0 {
        let has_step_action = Repository::with_items(|items| {
            (items[in2 as usize].flags & core::constants::ItemFlags::IF_STEPACTION.bits()) != 0
        });

        if has_step_action {
            driver::step_driver(cn, in2 as usize);
            Repository::with_characters_mut(|characters| {
                characters[cn].cerrno = core::constants::ERR_FAILED as u16;
            });
            return;
        }
    }

    // Check if tile is blocked
    let is_blocked = Repository::with_map(|map| {
        map[m].ch != 0
            || map[m].to_ch != 0
            || map[m].it != 0
            || (map[m].flags & core::constants::MF_MOVEBLOCK as u64) != 0
    });

    if is_blocked {
        Repository::with_characters_mut(|characters| {
            characters[cn].cerrno = core::constants::ERR_FAILED as u16;
        });
        return;
    }

    Repository::with_characters_mut(|characters| {
        characters[cn].citem = 0;
        characters[cn].cerrno = core::constants::ERR_SUCCESS as u16;
    });

    State::with(|state| state.do_update_char(cn));

    // Handle money
    let final_in_id = if in_id & 0x80000000 != 0 {
        let tmp = in_id & 0x7FFFFFFF;
        let new_in = God::create_item(1); // blank template

        if new_in.is_none() {
            Repository::with_characters_mut(|characters| {
                characters[cn].cerrno = core::constants::ERR_FAILED as u16;
            });
            return;
        }

        let new_in = new_in.unwrap();

        Repository::with_items_mut(|items| {
            items[new_in].flags |= core::constants::ItemFlags::IF_TAKE.bits()
                | core::constants::ItemFlags::IF_LOOK.bits()
                | core::constants::ItemFlags::IF_MONEY.bits();
            items[new_in].value = tmp;
            let mut reference = [0u8; 40];
            let bytes = "some money".as_bytes();
            let len = bytes.len().min(40);
            reference[..len].copy_from_slice(&bytes[..len]);
            items[new_in].reference = reference;

            let (description, sprite) = if tmp > 999999 {
                ("A huge pile of gold coins", 121)
            } else if tmp > 99999 {
                ("A very large pile of gold coins", 120)
            } else if tmp > 9999 {
                ("A large pile of gold coins", 41)
            } else if tmp > 999 {
                ("A small pile of gold coins", 40)
            } else if tmp > 99 {
                ("Some gold coins", 39)
            } else if tmp > 9 {
                ("A pile of silver coins", 38)
            } else if tmp > 2 {
                ("A few silver coins", 37)
            } else if tmp == 2 {
                ("A couple of silver coins", 37)
            } else {
                ("A lonely silver coin", 37)
            };

            let mut description_bytes = [0u8; 200];
            let bytes = description.as_bytes();
            let len = bytes.len().min(200);
            description_bytes[..len].copy_from_slice(&bytes[..len]);
            items[new_in].description = description_bytes;
            items[new_in].sprite[0] = sprite;
        });

        log::info!("Character {} dropped {}G {}S", cn, tmp / 100, tmp % 100);

        new_in as u32
    } else {
        // Check whether the item is allowed to be given/dropped
        let may_drop = State::with(|state| state.do_maygive(cn, 0, in_id as usize));
        if !may_drop {
            // Restore cursor item and indicate failure
            Repository::with_characters_mut(|characters| {
                characters[cn].citem = in_id;
                characters[cn].cerrno = core::constants::ERR_FAILED as u16;
            });
            State::with(|state| {
                state.do_character_log(
                    cn,
                    core::types::FontColor::Red,
                    "You are not allowed to do that!\n",
                );
            });
            return;
        }

        let item_name = Repository::with_items(|items| items[in_id as usize].name.clone());
        log::info!(
            "Character {} dropped {}",
            cn,
            String::from_utf8_lossy(&item_name)
        );
        in_id
    };

    Repository::with_map_mut(|map| {
        map[m].it = final_in_id;
    });

    let (active, light_active, light_inactive) = Repository::with_items(|items| {
        (
            items[final_in_id as usize].active,
            items[final_in_id as usize].light[1],
            items[final_in_id as usize].light[0],
        )
    });

    Repository::with_items_mut(|items| {
        items[final_in_id as usize].x = x as u16;
        items[final_in_id as usize].y = y as u16;
        items[final_in_id as usize].carried = 0;
    });

    if active != 0 && light_active != 0 {
        State::with_mut(|state| {
            state.do_add_light(x as i32, y as i32, light_active as i32);
        });
    } else if light_inactive != 0 {
        State::with_mut(|state| {
            state.do_add_light(x as i32, y as i32, light_inactive as i32);
        });
    }
}

/// Port of `plr_misc` from `svr_act.cpp`
///
/// Dispatches the character's misc action (`status2`) to the appropriate
/// action handler (attack, pickup, drop, give, use, bow, wave, skill, ...).
/// Sets character errno on unknown actions.
///
/// # Arguments
/// * `cn` - Character index whose misc action to process
pub fn plr_misc(cn: usize) {
    let status2 = Repository::with_characters(|characters| characters[cn].status2);

    match status2 {
        0 => plr_attack(cn, 0),
        1 => plr_pickup(cn),
        2 => plr_drop(cn),
        3 => plr_give(cn),
        4 => plr_use(cn),
        5 => plr_attack(cn, 1),
        6 => plr_attack(cn, 0),
        7 => plr_bow(cn),
        8 => plr_wave(cn),
        9 => plr_skill(cn),
        _ => {
            log::error!("plr_misc: unknown status2 {} for char {}", status2, cn);
            Repository::with_characters_mut(|characters| {
                characters[cn].cerrno = core::constants::ERR_FAILED as u16;
            });
        }
    }
}

/// Port of `plr_check_target` from `svr_act.cpp`
///
/// Checks whether a map tile is a valid target for placing a character or
/// item: it must not contain characters, and it must not be flagged as
/// movement-blocked; items on the tile are allowed only when they aren't
/// movement-blocking either.
///
/// # Arguments
/// * `m` - Map index to inspect
///
/// # Returns
/// `true` if tile is a valid empty target, `false` otherwise
pub fn plr_check_target(m: usize) -> bool {
    Repository::with_map(|map| {
        if map[m].ch != 0 || map[m].to_ch != 0 {
            return false;
        }

        if (map[m].flags & core::constants::MF_MOVEBLOCK as u64) != 0 {
            return false;
        }

        let it_id = map[m].it;
        if it_id != 0 {
            Repository::with_items(|items| {
                (items[it_id as usize].flags & core::constants::ItemFlags::IF_MOVEBLOCK.bits()) == 0
            })
        } else {
            true
        }
    })
}

/// Port of `plr_set_target` from `svr_act.cpp`
///
/// Marks the provided map tile as targeted by character `cn` by setting
/// `to_ch`. Uses `plr_check_target` to validate the tile first.
///
/// # Arguments
/// * `m` - Map index to set as target
/// * `cn` - Character index that will be the target occupant
///
/// # Returns
/// `true` on success, `false` if tile is not a valid target
pub fn plr_set_target(m: usize, cn: usize) -> bool {
    if !plr_check_target(m) {
        return false;
    }

    Repository::with_map_mut(|map| {
        map[m].to_ch = cn as u32;
    });

    true
}

/// Port of `plr_reset_status` from `svr_act.cpp`
///
/// Resets the character's `status` to the base idle status corresponding
/// to its current `dir` (direction). Performs sanity checks for illegal
/// `dir` values and logs an error if encountered.
///
/// # Arguments
/// * `cn` - Character index whose status to reset
pub fn plr_reset_status(cn: usize) {
    Repository::with_characters_mut(|characters| {
        characters[cn].status = match characters[cn].dir {
            core::constants::DX_UP => 0,
            core::constants::DX_DOWN => 1,
            core::constants::DX_LEFT => 2,
            core::constants::DX_RIGHT => 3,
            core::constants::DX_LEFTUP => 4,
            core::constants::DX_LEFTDOWN => 5,
            core::constants::DX_RIGHTUP => 6,
            core::constants::DX_RIGHTDOWN => 7,
            _ => {
                log::error!(
                    "plr_reset_status: illegal value for dir: {} for char {}",
                    characters[cn].dir,
                    cn
                );
                characters[cn].dir = core::constants::DX_UP;
                0
            }
        };
    });
}

/// Perform the character's current driving action.
///
/// Resets status bits and calls the driver for the character if their
/// action group is active. This is the main per-tick driver entry for
/// active characters.
///
/// # Arguments
/// * `cn` - Character index to perform driver actions for
pub fn plr_doact(cn: usize) {
    plr_reset_status(cn);
    if Repository::with_characters(|characters| characters[cn].group_active()) {
        driver::driver(cn);
    }
}

/// Port of `plr_act` from `svr_tick.cpp`
///
/// Per-character action state machine executed each tick. Handles stunned/
/// stoned conditions, executes idle/driver actions, advances walking/turning
/// frames based on `speedo`, and triggers move/turn/misc handlers when a
/// frame sequence completes.
///
/// # Arguments
/// * `cn` - Character index to process
pub fn plr_act(cn: usize) {
    let (stunned, flags, status) = Repository::with_characters(|characters| {
        (
            characters[cn].stunned,
            characters[cn].flags,
            characters[cn].status,
        )
    });

    if stunned != 0 {
        driver::act_idle(cn);
        return;
    }

    if flags & enums::CharacterFlags::Stoned.bits() != 0 {
        driver::act_idle(cn);
        return;
    }

    match status {
        // idle states: call idle and driver
        0..=7 => {
            driver::act_idle(cn);
            plr_doact(cn);
            return;
        }

        // walk up: 16..22 increment, 23 execute
        16..=22 => {
            if speedo(cn) != 0 {
                Repository::with_characters_mut(|ch| ch[cn].status += 1);
            }
            return;
        }
        23 => {
            if speedo(cn) != 0 {
                Repository::with_characters_mut(|ch| ch[cn].status = 16);
                plr_move_up(cn);
                plr_doact(cn);
            }
            return;
        }

        // walk down: 24..30 then 31
        24..=30 => {
            if speedo(cn) != 0 {
                Repository::with_characters_mut(|ch| ch[cn].status += 1);
            }
            return;
        }
        31 => {
            if speedo(cn) != 0 {
                Repository::with_characters_mut(|ch| ch[cn].status = 24);
                plr_move_down(cn);
                plr_doact(cn);
            }
            return;
        }

        // walk left: 32..38 then 39
        32..=38 => {
            if speedo(cn) != 0 {
                Repository::with_characters_mut(|ch| ch[cn].status += 1);
            }
            return;
        }
        39 => {
            if speedo(cn) != 0 {
                Repository::with_characters_mut(|ch| ch[cn].status = 32);
                plr_move_left(cn);
                plr_doact(cn);
            }
            return;
        }

        // walk right: 40..46 then 47
        40..=46 => {
            if speedo(cn) != 0 {
                Repository::with_characters_mut(|ch| ch[cn].status += 1);
            }
            return;
        }
        47 => {
            if speedo(cn) != 0 {
                Repository::with_characters_mut(|ch| ch[cn].status = 40);
                plr_move_right(cn);
                plr_doact(cn);
            }
            return;
        }

        // left+up: 48..58 then 59
        48..=58 => {
            if speedo(cn) != 0 {
                Repository::with_characters_mut(|ch| ch[cn].status += 1);
            }
            return;
        }
        59 => {
            if speedo(cn) != 0 {
                Repository::with_characters_mut(|ch| ch[cn].status = 48);
                plr_move_leftup(cn);
                plr_doact(cn);
            }
            return;
        }

        // left+down: 60..70 then 71
        60..=70 => {
            if speedo(cn) != 0 {
                Repository::with_characters_mut(|ch| ch[cn].status += 1);
            }
            return;
        }
        71 => {
            if speedo(cn) != 0 {
                Repository::with_characters_mut(|ch| ch[cn].status = 60);
                plr_move_leftdown(cn);
                plr_doact(cn);
            }
            return;
        }

        // right+up: 72..82 then 83
        72..=82 => {
            if speedo(cn) != 0 {
                Repository::with_characters_mut(|ch| ch[cn].status += 1);
            }
            return;
        }
        83 => {
            if speedo(cn) != 0 {
                Repository::with_characters_mut(|ch| ch[cn].status = 72);
                plr_move_rightup(cn);
                plr_doact(cn);
            }
            return;
        }

        // right+down: 84..94 then 95
        84..=94 => {
            if speedo(cn) != 0 {
                Repository::with_characters_mut(|ch| ch[cn].status += 1);
            }
            return;
        }
        95 => {
            if speedo(cn) != 0 {
                Repository::with_characters_mut(|ch| ch[cn].status = 84);
                plr_move_rightdown(cn);
                plr_doact(cn);
            }
            return;
        }

        // turns: grouped ranges mapping to final turn actions
        96..=98 => {
            if speedo(cn) != 0 {
                Repository::with_characters_mut(|ch| ch[cn].status += 1);
            }
            return;
        }
        99 => {
            if speedo(cn) != 0 {
                Repository::with_characters_mut(|ch| ch[cn].status = 96);
                plr_turn_leftup(cn);
                plr_doact(cn);
            }
            return;
        }

        100..=102 => {
            if speedo(cn) != 0 {
                Repository::with_characters_mut(|ch| ch[cn].status += 1);
            }
            return;
        }
        103 => {
            if speedo(cn) != 0 {
                Repository::with_characters_mut(|ch| ch[cn].status = 96);
                plr_turn_left(cn);
                plr_doact(cn);
            }
            return;
        }

        104..=106 => {
            if speedo(cn) != 0 {
                Repository::with_characters_mut(|ch| ch[cn].status += 1);
            }
            return;
        }
        107 => {
            if speedo(cn) != 0 {
                Repository::with_characters_mut(|ch| ch[cn].status = 104);
                plr_turn_rightup(cn);
                plr_doact(cn);
            }
            return;
        }

        108..=110 => {
            if speedo(cn) != 0 {
                Repository::with_characters_mut(|ch| ch[cn].status += 1);
            }
            return;
        }
        111 => {
            if speedo(cn) != 0 {
                Repository::with_characters_mut(|ch| ch[cn].status = 108);
                plr_turn_right(cn);
                plr_doact(cn);
            }
            return;
        }

        112..=114 => {
            if speedo(cn) != 0 {
                Repository::with_characters_mut(|ch| ch[cn].status += 1);
            }
            return;
        }
        115 => {
            if speedo(cn) != 0 {
                Repository::with_characters_mut(|ch| ch[cn].status = 112);
                plr_turn_leftdown(cn);
                plr_doact(cn);
            }
            return;
        }

        116..=118 => {
            if speedo(cn) != 0 {
                Repository::with_characters_mut(|ch| ch[cn].status += 1);
            }
            return;
        }
        119 => {
            if speedo(cn) != 0 {
                Repository::with_characters_mut(|ch| ch[cn].status = 116);
                plr_turn_left(cn);
                plr_doact(cn);
            }
            return;
        }

        120..=122 => {
            if speedo(cn) != 0 {
                Repository::with_characters_mut(|ch| ch[cn].status += 1);
            }
            return;
        }
        123 => {
            if speedo(cn) != 0 {
                Repository::with_characters_mut(|ch| ch[cn].status = 120);
                plr_turn_rightdown(cn);
                plr_doact(cn);
            }
            return;
        }

        124..=126 => {
            if speedo(cn) != 0 {
                Repository::with_characters_mut(|ch| ch[cn].status += 1);
            }
            return;
        }
        127 => {
            if speedo(cn) != 0 {
                Repository::with_characters_mut(|ch| ch[cn].status = 124);
                plr_turn_right(cn);
                plr_doact(cn);
            }
            return;
        }

        128..=130 => {
            if speedo(cn) != 0 {
                Repository::with_characters_mut(|ch| ch[cn].status += 1);
            }
            return;
        }
        131 => {
            if speedo(cn) != 0 {
                Repository::with_characters_mut(|ch| ch[cn].status = 128);
                plr_turn_leftup(cn);
                plr_doact(cn);
            }
            return;
        }

        132..=134 => {
            if speedo(cn) != 0 {
                Repository::with_characters_mut(|ch| ch[cn].status += 1);
            }
            return;
        }
        135 => {
            if speedo(cn) != 0 {
                Repository::with_characters_mut(|ch| ch[cn].status = 132);
                plr_turn_up(cn);
                plr_doact(cn);
            }
            return;
        }

        136..=138 => {
            if speedo(cn) != 0 {
                Repository::with_characters_mut(|ch| ch[cn].status += 1);
            }
            return;
        }
        139 => {
            if speedo(cn) != 0 {
                Repository::with_characters_mut(|ch| ch[cn].status = 136);
                plr_turn_leftdown(cn);
                plr_doact(cn);
            }
            return;
        }

        140..=142 => {
            if speedo(cn) != 0 {
                Repository::with_characters_mut(|ch| ch[cn].status += 1);
            }
            return;
        }
        143 => {
            if speedo(cn) != 0 {
                Repository::with_characters_mut(|ch| ch[cn].status = 140);
                plr_turn_down(cn);
                plr_doact(cn);
            }
            return;
        }

        144..=146 => {
            if speedo(cn) != 0 {
                Repository::with_characters_mut(|ch| ch[cn].status += 1);
            }
            return;
        }
        147 => {
            if speedo(cn) != 0 {
                Repository::with_characters_mut(|ch| ch[cn].status = 144);
                plr_turn_rightup(cn);
                plr_doact(cn);
            }
            return;
        }

        148..=150 => {
            if speedo(cn) != 0 {
                Repository::with_characters_mut(|ch| ch[cn].status += 1);
            }
            return;
        }
        151 => {
            if speedo(cn) != 0 {
                Repository::with_characters_mut(|ch| ch[cn].status = 148);
                plr_turn_up(cn);
                plr_doact(cn);
            }
            return;
        }

        152..=154 => {
            if speedo(cn) != 0 {
                Repository::with_characters_mut(|ch| ch[cn].status += 1);
            }
            return;
        }
        155 => {
            if speedo(cn) != 0 {
                Repository::with_characters_mut(|ch| ch[cn].status = 152);
                plr_turn_rightdown(cn);
                plr_doact(cn);
            }
            return;
        }

        156..=158 => {
            if speedo(cn) != 0 {
                Repository::with_characters_mut(|ch| ch[cn].status += 1);
            }
            return;
        }
        159 => {
            if speedo(cn) != 0 {
                Repository::with_characters_mut(|ch| ch[cn].status = 156);
                plr_turn_down(cn);
                plr_doact(cn);
            }
            return;
        }

        // misc actions: 160..166 increment, 167 execute misc then doact
        160..=166 => {
            if speedo(cn) != 0 {
                Repository::with_characters_mut(|ch| ch[cn].status += 1);
            }
            return;
        }
        167 => {
            if speedo(cn) != 0 {
                Repository::with_characters_mut(|ch| ch[cn].status = 160);
                plr_misc(cn);
                plr_doact(cn);
            }
            return;
        }

        // misc down 168..174 then 175
        168..=174 => {
            if speedo(cn) != 0 {
                Repository::with_characters_mut(|ch| ch[cn].status += 1);
            }
            return;
        }
        175 => {
            if speedo(cn) != 0 {
                Repository::with_characters_mut(|ch| ch[cn].status = 168);
                plr_misc(cn);
                plr_doact(cn);
            }
            return;
        }

        // misc left 176..182 then 183
        176..=182 => {
            if speedo(cn) != 0 {
                Repository::with_characters_mut(|ch| ch[cn].status += 1);
            }
            return;
        }
        183 => {
            if speedo(cn) != 0 {
                Repository::with_characters_mut(|ch| ch[cn].status = 176);
                plr_misc(cn);
                plr_doact(cn);
            }
            return;
        }

        // misc right 184..190 then 191
        184..=190 => {
            if speedo(cn) != 0 {
                Repository::with_characters_mut(|ch| ch[cn].status += 1);
            }
            return;
        }
        191 => {
            if speedo(cn) != 0 {
                Repository::with_characters_mut(|ch| ch[cn].status = 184);
                plr_misc(cn);
                plr_doact(cn);
            }
            return;
        }

        _ => {
            driver::act_idle(cn);
            return;
        }
    }
}

/// Fast helper to compute the per-tick movement index for a character.
///
/// Uses a precomputed `SPEEDTAB` and the global ticker modulo to determine
/// whether the character moves on the current sub-tick.
///
/// # Arguments
/// * `n` - Character index
pub fn speedo(n: usize) -> i32 {
    let speed = Repository::with_characters(|characters| characters[n].speed as usize);
    let ctick = Repository::with_globals(|globals| (globals.ticker % 20) as usize);
    SPEEDTAB[speed][ctick] as i32
}

// Static mode for plr_getmap speed savings
static PLR_GETMAP_MODE: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

/// Clear the saved small map for all players to force a full resend
pub fn plr_clear_map() {
    Server::with_players_mut(|players| {
        for n in 1..players.len() {
            players[n].smap = std::array::from_fn(|_| core::types::CMap::default());
            players[n].vx = 0; // force do_all in map generation
        }
    });
}

/// Choose and dispatch the appropriate map update implementation.
///
/// Decides between the full (`plr_getmap_complete`) or fast (`plr_getmap_fast`)
/// small-map generation based on server load and global flags. When entering
/// or leaving "speed savings" mode the function clears map caches and
/// announces the mode change.
///
/// # Arguments
/// * `nr` - Player slot index requesting the map update
pub fn plr_getmap(nr: usize) {
    use std::sync::atomic::Ordering;

    let (load_avg, flags) = Repository::with_globals(|globals| (globals.load_avg, globals.flags));

    let mode = PLR_GETMAP_MODE.load(Ordering::SeqCst) as i32;

    if load_avg > 8000 && mode == 0 && (flags & core::constants::GF_SPEEDY) != 0 {
        PLR_GETMAP_MODE.store(1, Ordering::SeqCst);
        plr_clear_map();
        State::with(|state| {
            state.do_announce(
                0,
                0,
                "Entered speed savings mode. Display will be imperfect.\n",
            );
        });
    }

    if ((flags & core::constants::GF_SPEEDY) == 0 || load_avg < 6500) && mode != 0 {
        PLR_GETMAP_MODE.store(0, Ordering::SeqCst);
        State::with(|state| {
            state.do_announce(0, 0, "Left speed savings mode.\n");
        });
    }

    // dispatch to the appropriate implementation
    let mode = PLR_GETMAP_MODE.load(Ordering::SeqCst);
    if mode == 0 {
        plr_getmap_complete(nr);
    } else {
        plr_getmap_fast(nr);
    }
}

pub fn plr_getmap_complete(nr: usize) {
    // Conservative port of the original `plr_getmap_complete`.
    // This computes the player's small-map (`smap`) for the visible window
    // around the character and stores it into `players[nr].smap`.
    // It uses existing State visibility helpers to compute line-of-sight
    // and daylight, and fills in basic fields (background, item, char,
    // flags, light) so that the later `plr_change` comparison can send
    // the appropriate updates to the client.

    // Cut values used by the original implementation
    const YSCUT: i32 = 3;
    const YECUT: i32 = 1;
    const XSCUT: i32 = 2;
    const XECUT: i32 = 2;

    // Get character number (cn) controlled by this player
    let cn = Server::with_players(|players| players[nr].usnr as usize);
    if cn == 0 || cn >= core::constants::MAXCHARS as usize {
        return;
    }

    // Trigger recomputation of the visibility map for this character
    let (ch_x, ch_y) = Repository::with_characters(|characters| {
        (characters[cn].x as i32, characters[cn].y as i32)
    });
    State::with_mut(|state| {
        let _ = state.can_see(Some(cn), ch_x, ch_y, ch_x + 1, ch_y + 1, 16);
    });

    // Snapshot see_map for this character
    let see = Repository::with_see_map(|see_map| see_map[cn]);

    // Compute region to update
    let ys = ch_y - (core::constants::TILEY as i32 / 2) + YSCUT;
    let ye = ch_y + (core::constants::TILEY as i32 / 2) - YECUT;
    let xs = ch_x - (core::constants::TILEX as i32 / 2) + XSCUT;
    let xe = ch_x + (core::constants::TILEX as i32 / 2) - XECUT;

    // Determine whether we must force full resend (visibility changed or building)
    let _visibility_changed = Server::with_players(|players| {
        let p = &players[nr];
        if p.vx != see.x || p.vy != see.y {
            return true;
        }
        // compare visual arrays
        for i in 0..(40 * 40) {
            if p.visi[i] != see.vis[i] {
                return true;
            }
        }
        false
    });
    let _do_all = _visibility_changed
        || Repository::with_characters(|characters| characters[cn].is_building());

    // Iterate window and fill smap entries
    Server::with_players_mut(|players| {
        // start indices into small map
        let mut n = (YSCUT as usize) * core::constants::TILEX + XSCUT as usize;
        let mut map_y = ys;

        for _y in ys..ye {
            let mut map_x = xs;
            for _x in xs..xe {
                let m = (map_x as usize)
                    .wrapping_add((map_y as usize) * core::constants::SERVER_MAPX as usize);

                // default empty
                let mut new_cmap = core::types::CMap::default();
                new_cmap.ba_sprite = core::constants::SPR_EMPTY as i16;

                // If outside world bounds -> leave empty
                if map_x < 0
                    || map_y < 0
                    || map_x >= core::constants::SERVER_MAPX as i32
                    || map_y >= core::constants::SERVER_MAPY as i32
                {
                    players[nr].smap[n] = new_cmap;
                    n += 1;
                    map_x += 1;
                    continue;
                }

                // Retrieve the authoritative map tile
                let map_tile = Repository::with_map(|map| map[m]);

                // compute light and visibility
                let tmp = State::check_dlightm(m);
                let mut light = std::cmp::max(map_tile.light as i32, tmp);
                light = State::with_mut(|state| state.do_character_calculate_light(cn, light));

                let infra = Repository::with_characters(|characters| {
                    (characters[cn].flags & enums::CharacterFlags::Infrared.bits()) != 0
                });
                let infra_flag = if light <= 5 && infra { 1 } else { 0 };

                if light == 0 && Repository::with_map(|map| map[m].ch as usize) == cn {
                    light = 1;
                }

                if light == 0 {
                    // nothing visible
                    players[nr].smap[n] = new_cmap;
                    n += 1;
                    map_x += 1;
                    continue;
                }

                // --- Flags ---
                let mut flags: u32 = 0;
                if (map_tile.flags & core::constants::MF_UWATER as u64) != 0 {
                    flags |= core::constants::UWATER;
                }
                if infra_flag != 0 {
                    flags |= core::constants::INFRARED;
                }
                // building copy
                let mut flags2: u32 = 0;
                if Repository::with_characters(|characters| characters[cn].is_building()) {
                    flags2 = map_tile.flags as u32;
                }

                new_cmap.flags = flags;
                new_cmap.flags2 = flags2;

                // background sprite
                new_cmap.ba_sprite = map_tile.sprite as i16;

                // --- Character ---
                let co = Repository::with_map(|map| map[m].ch as usize);
                if co != 0 {
                    let can_see = State::with_mut(|state| state.do_char_can_see(cn, co));
                    if can_see != 0 {
                        // basic character info
                        let ch = Repository::with_characters(|characters| characters[co]);
                        if ch.sprite_override != 0 {
                            new_cmap.ch_sprite = ch.sprite_override as i16;
                        } else {
                            new_cmap.ch_sprite = ch.sprite as i16;
                        }
                        new_cmap.ch_status = ch.status as u8;
                        new_cmap.ch_status2 = ch.status2 as u8;
                        new_cmap.ch_speed = ch.speed as u8;
                        new_cmap.ch_nr = co as u16;
                        new_cmap.ch_id = crate::helpers::char_id(co) as u16;
                        // health percent (best-effort)
                        new_cmap.ch_proz = if ch.hp[5] > 0 {
                            ((ch.hp[5] as u32 * 100) / ch.hp[5].max(1) as u32) as u8
                        } else {
                            0
                        };
                        new_cmap.flags |= core::constants::ISCHAR;
                        if ch.stunned != 0 {
                            new_cmap.flags |= core::constants::STUNNED;
                        }
                        if (ch.flags & enums::CharacterFlags::Stoned.bits()) != 0 {
                            new_cmap.flags |= core::constants::TOMB;
                        }
                    }
                }

                // --- Item ---
                if map_tile.fsprite != 0 {
                    new_cmap.it_sprite = map_tile.fsprite as i16;
                    new_cmap.it_status = 0;
                } else {
                    let it_id = Repository::with_map(|map| map[m].it as usize);
                    if it_id != 0 {
                        let visible_item =
                            State::with_mut(|state| state.do_char_can_see_item(cn, it_id)) != 0;
                        if visible_item {
                            let it = Repository::with_items(|items| items[it_id]);
                            new_cmap.it_sprite = if it.active != 0 {
                                it.sprite[1]
                            } else {
                                it.sprite[0]
                            };
                            let active_idx = if it.active != 0 { 1 } else { 0 };
                            new_cmap.it_status = it.status[active_idx];
                            new_cmap.flags |= core::constants::ISITEM;
                        }
                    }
                }

                players[nr].smap[n] = new_cmap;

                // remember copy of map for later diffs
                players[nr].xmap[n] = Repository::with_map(|map| map[m]);

                n += 1;
                map_x += 1;
            }
            map_y += 1;
        }

        // update vx, vy and visi copy
        players[nr].vx = see.x;
        players[nr].vy = see.y;
        players[nr].visi.copy_from_slice(&see.vis);
    });

    // Now compute diffs between the client's `cmap` and the newly computed `smap`
    // and send minimal update packets using the same packet types and
    // run-length/light-grouping heuristics as the original C++ server.
    {
        use core::constants::{SV_SETMAP, SV_SETMAP3, SV_SETMAP4, SV_SETMAP5, SV_SETMAP6};

        let tile_count = (core::constants::TILEX * core::constants::TILEY) as usize;

        // Snapshot the current client-map (`cmap`) and the computed small-map (`smap`).
        let mut cmap = Server::with_players(|players| players[nr].cmap);
        let smap = Server::with_players(|players| players[nr].smap);

        // helper: base-status for items
        fn it_base_status(n: u8) -> u8 {
            if n == 0 {
                return 0;
            }
            if n == 1 {
                return 1;
            }
            if n < 6 {
                return 2;
            }
            if n < 8 {
                return 6;
            }
            if n < 16 {
                return 8;
            }
            if n < 21 {
                return 16;
            }
            n
        }

        // helper: base-status for characters
        fn ch_base_status(n: u8) -> u8 {
            if n < 4 {
                return n;
            }
            if n < 16 {
                return n;
            }
            if n < 24 {
                return 16;
            }
            if n < 32 {
                return 24;
            }
            if n < 40 {
                return 32;
            }
            if n < 48 {
                return 40;
            }
            if n < 60 {
                return 48;
            }
            n
        }

        // equality check for two CMap entries
        fn cmap_eq(a: &core::types::CMap, b: &core::types::CMap) -> bool {
            a.ba_sprite == b.ba_sprite
                && a.light == b.light
                && a.flags == b.flags
                && a.flags2 == b.flags2
                && a.ch_sprite == b.ch_sprite
                && a.ch_status2 == b.ch_status2
                && a.ch_status == b.ch_status
                && a.ch_speed == b.ch_speed
                && a.ch_nr == b.ch_nr
                && a.ch_id == b.ch_id
                && a.ch_proz == b.ch_proz
                && a.it_sprite == b.it_sprite
                && a.it_status == b.it_status
        }

        // Light-grouping helpers: compute score (efficiency) and send packets
        let cl_sizes = [1usize, 3usize, 7usize, 26usize];
        let cl_bytes = [3usize, 4usize, 6usize, 16usize];

        // scoring: emulate original (50 * count / bytes)
        let mut n = 0usize;
        while n < tile_count {
            if cmap[n].light != smap[n].light {
                // choose best grouping
                let mut best_idx = 0usize;
                let mut best_score = 0i32;
                for (li, &size) in cl_sizes.iter().enumerate() {
                    let end = std::cmp::min(n + size, tile_count);
                    let mut changed = 0usize;
                    for m in n..end {
                        if cmap[m].light != smap[m].light {
                            changed += 1;
                        }
                    }
                    let score = if cl_bytes[li] > 0 {
                        (50 * changed) as i32 / cl_bytes[li] as i32
                    } else {
                        0
                    };
                    if score >= best_score {
                        best_score = score;
                        best_idx = li;
                    }
                }

                // perform chosen send variant
                match best_idx {
                    0 => {
                        // SV_SETMAP4 - single light (3 bytes)
                        let mut buf = [0u8; 16];
                        buf[0] = SV_SETMAP4;
                        let header = (n as u16) | ((smap[n].light as u16) << 12);
                        buf[1] = header as u8;
                        buf[2] = (header >> 8) as u8;
                        NetworkManager::with(|net| net.xsend(nr, &buf, 3));
                        cmap[n].light = smap[n].light;
                    }
                    1 => {
                        // SV_SETMAP5 - three lights (4 bytes)
                        let mut buf = [0u8; 16];
                        buf[0] = SV_SETMAP5;
                        let header = (n as u16) | ((smap[n].light as u16) << 12);
                        buf[1] = header as u8;
                        buf[2] = (header >> 8) as u8;
                        let mut p = 3usize;
                        for m in (n + 2)..std::cmp::min(n + 3, tile_count) {
                            buf[p] = (smap[m].light & 0xF) | ((smap[m - 1].light & 0xF) << 4);
                            cmap[m].light = smap[m].light;
                            cmap[m - 1].light = smap[m - 1].light;
                            p += 1;
                        }
                        NetworkManager::with(|net| net.xsend(nr, &buf, p as u8));
                    }
                    2 => {
                        // SV_SETMAP6 - seven lights (6 bytes)
                        let mut buf = [0u8; 16];
                        buf[0] = SV_SETMAP6;
                        let header = (n as u16) | ((smap[n].light as u16) << 12);
                        buf[1] = header as u8;
                        buf[2] = (header >> 8) as u8;
                        let mut p = 3usize;
                        for m in (n + 2)..std::cmp::min(n + 7, tile_count) {
                            buf[p] = (smap[m].light & 0xF) | ((smap[m - 1].light & 0xF) << 4);
                            cmap[m].light = smap[m].light;
                            cmap[m - 1].light = smap[m - 1].light;
                            p += 1;
                        }
                        NetworkManager::with(|net| net.xsend(nr, &buf, p as u8));
                    }
                    3 => {
                        // SV_SETMAP3 - up to 26 lights (16 bytes)
                        let mut buf = [0u8; 16];
                        buf[0] = SV_SETMAP3;
                        let header = (n as u16) | ((smap[n].light as u16) << 12);
                        buf[1] = header as u8;
                        buf[2] = (header >> 8) as u8;
                        let mut p = 3usize;
                        for m in (n + 2)..std::cmp::min(n + 26, tile_count) {
                            buf[p] = (smap[m].light & 0xF) | ((smap[m - 1].light & 0xF) << 4);
                            cmap[m].light = smap[m].light;
                            cmap[m - 1].light = smap[m - 1].light;
                            p += 1;
                        }
                        NetworkManager::with(|net| net.xsend(nr, &buf, 16));
                    }
                    _ => {}
                }
            }
            n += 1;
        }

        // General diffs: find first differing cmap entry and send minimal SV_SETMAP packets
        let mut lastn: i32 = -1;
        let mut idx = 0usize;
        loop {
            // find next difference
            let mut found = None;
            for i in idx..tile_count {
                if !cmap_eq(&cmap[i], &smap[i]) {
                    found = Some(i);
                    break;
                }
            }
            let nidx = match found {
                Some(v) => v,
                None => break,
            };

            let mut buf = [0u8; 256];
            let mut p: usize;

            if lastn >= 0 && (nidx as i32) > lastn && (nidx as i32 - lastn) < 127 {
                buf[0] = (SV_SETMAP | ((nidx as i32 - lastn) as u8)) as u8;
                buf[1] = 0;
                p = 2;
            } else if lastn < 0 && nidx < 127 {
                // when lastn == -1, allow short form as original did
                buf[0] = (SV_SETMAP | ((nidx as u8) & 0x7F)) as u8;
                buf[1] = 0;
                p = 2;
            } else {
                buf[0] = SV_SETMAP;
                buf[1] = 0;
                let n_u16 = nidx as u16;
                buf[2] = n_u16 as u8;
                buf[3] = (n_u16 >> 8) as u8;
                p = 4;
            }

            // ba_sprite
            if cmap[nidx].ba_sprite != smap[nidx].ba_sprite {
                buf[1] |= 1;
                let v = smap[nidx].ba_sprite as u16;
                buf[p] = v as u8;
                buf[p + 1] = (v >> 8) as u8;
                p += 2;
            }

            // flags
            if cmap[nidx].flags != smap[nidx].flags {
                buf[1] |= 2;
                let v = smap[nidx].flags as u32;
                buf[p] = v as u8;
                buf[p + 1] = (v >> 8) as u8;
                buf[p + 2] = (v >> 16) as u8;
                buf[p + 3] = (v >> 24) as u8;
                p += 4;
            }

            // flags2
            if cmap[nidx].flags2 != smap[nidx].flags2 {
                buf[1] |= 4;
                let v = smap[nidx].flags2 as u32;
                buf[p] = v as u8;
                buf[p + 1] = (v >> 8) as u8;
                buf[p + 2] = (v >> 16) as u8;
                buf[p + 3] = (v >> 24) as u8;
                p += 4;
            }

            // it_sprite
            if cmap[nidx].it_sprite != smap[nidx].it_sprite {
                buf[1] |= 8;
                let v = smap[nidx].it_sprite as u16;
                buf[p] = v as u8;
                buf[p + 1] = (v >> 8) as u8;
                p += 2;
            }

            // it_status (only if base status differs)
            if cmap[nidx].it_status != smap[nidx].it_status
                && it_base_status(cmap[nidx].it_status) != it_base_status(smap[nidx].it_status)
            {
                buf[1] |= 16;
                buf[p] = smap[nidx].it_status;
                p += 1;
            }

            // character sprite / status
            if cmap[nidx].ch_sprite != smap[nidx].ch_sprite
                || (cmap[nidx].ch_status != smap[nidx].ch_status
                    && ch_base_status(cmap[nidx].ch_status) != ch_base_status(smap[nidx].ch_status))
                || cmap[nidx].ch_status2 != smap[nidx].ch_status2
            {
                buf[1] |= 32;
                let v = smap[nidx].ch_sprite as u16;
                buf[p] = v as u8;
                buf[p + 1] = (v >> 8) as u8;
                p += 2;
                buf[p] = smap[nidx].ch_status;
                p += 1;
                buf[p] = smap[nidx].ch_status2;
                p += 1;
            }

            // character id / speed
            if cmap[nidx].ch_speed != smap[nidx].ch_speed
                || cmap[nidx].ch_nr != smap[nidx].ch_nr
                || cmap[nidx].ch_id != smap[nidx].ch_id
            {
                buf[1] |= 64;
                let vnr = smap[nidx].ch_nr as u16;
                buf[p] = vnr as u8;
                buf[p + 1] = (vnr >> 8) as u8;
                p += 2;
                let vid = smap[nidx].ch_id as u16;
                buf[p] = vid as u8;
                buf[p + 1] = (vid >> 8) as u8;
                p += 2;
                buf[p] = smap[nidx].ch_speed;
                p += 1;
            }

            // character health percent
            if cmap[nidx].ch_proz != smap[nidx].ch_proz {
                buf[1] |= 128;
                buf[p] = smap[nidx].ch_proz;
                p += 1;
            }

            if buf[1] != 0 {
                NetworkManager::with(|net| net.xsend(nr, &buf, p as u8));
                lastn = nidx as i32;
            }

            // remember copied values
            cmap[nidx] = smap[nidx];

            idx = nidx + 1;
            if idx >= tile_count {
                break;
            }
        }

        // write back the updated cmap into the player's data
        Server::with_players_mut(|players| players[nr].cmap = cmap);
    }
}

pub fn plr_getmap_fast(nr: usize) {
    // Fast-path version of `plr_getmap_complete` using reduced update window
    // (ported from original C++ `plr_getmap_fast`). Uses different cut
    // constants to update a slightly different region for speed savings.

    // Get character number (cn) controlled by this player
    let cn = Server::with_players(|players| players[nr].usnr as usize);
    if cn == 0 || cn >= core::constants::MAXCHARS as usize {
        return;
    }

    // Trigger recomputation of the visibility map for this character
    let (ch_x, ch_y) = Repository::with_characters(|characters| {
        (characters[cn].x as i32, characters[cn].y as i32)
    });
    State::with_mut(|state| {
        let _ = state.can_see(Some(cn), ch_x, ch_y, ch_x + 1, ch_y + 1, 16);
    });

    // Snapshot see_map for this character
    let see = Repository::with_see_map(|see_map| see_map[cn]);

    // Compute region to update
    let ys = ch_x - (core::constants::TILEY as i32 / 2) + YSCUTF;
    let ye = ch_x + (core::constants::TILEY as i32 / 2) - YECUTF;
    let xs = ch_y - (core::constants::TILEX as i32 / 2) + XSCUTF;
    let xe = ch_y + (core::constants::TILEX as i32 / 2) - XECUTF;

    // Fill smap entries for the fast window
    Server::with_players_mut(|players| {
        let mut n = (YSCUTF as usize) * core::constants::TILEX + XSCUTF as usize;
        let mut map_y = ys;

        for _y in ys..ye {
            let mut map_x = xs;
            for _x in xs..xe {
                let m = (map_x as usize)
                    .wrapping_add((map_y as usize) * core::constants::SERVER_MAPX as usize);

                let mut new_cmap = core::types::CMap::default();
                new_cmap.ba_sprite = core::constants::SPR_EMPTY as i16;

                if map_x < 0
                    || map_y < 0
                    || map_x >= core::constants::SERVER_MAPX as i32
                    || map_y >= core::constants::SERVER_MAPY as i32
                {
                    players[nr].smap[n] = new_cmap;
                    n += 1;
                    map_x += 1;
                    continue;
                }

                let map_tile = Repository::with_map(|map| map[m]);

                // compute light and visibility
                let tmp = State::check_dlightm(m);
                let mut light = std::cmp::max(map_tile.light as i32, tmp);
                light = State::with_mut(|state| state.do_character_calculate_light(cn, light));

                let infra = Repository::with_characters(|characters| {
                    (characters[cn].flags & enums::CharacterFlags::Infrared.bits()) != 0
                });
                let infra_flag = if light <= 5 && infra { 1 } else { 0 };

                if light == 0 && Repository::with_map(|map| map[m].ch as usize) == cn {
                    light = 1;
                }

                if light == 0 {
                    players[nr].smap[n] = new_cmap;
                    n += 1;
                    map_x += 1;
                    continue;
                }

                // flags
                let mut flags: u32 = 0;
                if (map_tile.flags & core::constants::MF_UWATER as u64) != 0 {
                    flags |= core::constants::UWATER;
                }
                if infra_flag != 0 {
                    flags |= core::constants::INFRARED;
                }

                let mut flags2: u32 = 0;
                if Repository::with_characters(|characters| characters[cn].is_building()) {
                    flags2 = map_tile.flags as u32;
                }

                new_cmap.flags = flags;
                new_cmap.flags2 = flags2;
                new_cmap.ba_sprite = map_tile.sprite as i16;

                // character
                let co = Repository::with_map(|map| map[m].ch as usize);
                if co != 0 {
                    let can_see = State::with_mut(|state| state.do_char_can_see(cn, co));
                    if can_see != 0 {
                        let ch = Repository::with_characters(|characters| characters[co]);
                        if ch.sprite_override != 0 {
                            new_cmap.ch_sprite = ch.sprite_override as i16;
                        } else {
                            new_cmap.ch_sprite = ch.sprite as i16;
                        }
                        new_cmap.ch_status = ch.status as u8;
                        new_cmap.ch_status2 = ch.status2 as u8;
                        new_cmap.ch_speed = ch.speed as u8;
                        new_cmap.ch_nr = co as u16;
                        new_cmap.ch_id = crate::helpers::char_id(co) as u16;
                        new_cmap.ch_proz = if ch.hp[5] > 0 {
                            ((ch.hp[5] as u32 * 100) / ch.hp[5].max(1) as u32) as u8
                        } else {
                            0
                        };
                        new_cmap.flags |= core::constants::ISCHAR;
                        if ch.stunned != 0 {
                            new_cmap.flags |= core::constants::STUNNED;
                        }
                        if (ch.flags & enums::CharacterFlags::Stoned.bits()) != 0 {
                            new_cmap.flags |= core::constants::TOMB;
                        }
                    }
                }

                // item
                if map_tile.fsprite != 0 {
                    new_cmap.it_sprite = map_tile.fsprite as i16;
                    new_cmap.it_status = 0;
                } else {
                    let it_id = Repository::with_map(|map| map[m].it as usize);
                    if it_id != 0 {
                        let visible_item =
                            State::with_mut(|state| state.do_char_can_see_item(cn, it_id)) != 0;
                        if visible_item {
                            let it = Repository::with_items(|items| items[it_id]);
                            new_cmap.it_sprite = if it.active != 0 {
                                it.sprite[1]
                            } else {
                                it.sprite[0]
                            };
                            let active_idx = if it.active != 0 { 1 } else { 0 };
                            new_cmap.it_status = it.status[active_idx];
                            new_cmap.flags |= core::constants::ISITEM;
                        }
                    }
                }

                players[nr].smap[n] = new_cmap;
                players[nr].xmap[n] = Repository::with_map(|map| map[m]);

                n += 1;
                map_x += 1;
            }
            map_y += 1;
        }

        // update vx, vy and visi copy
        players[nr].vx = see.x;
        players[nr].vy = see.y;
        players[nr].visi.copy_from_slice(&see.vis);
    });

    // Reuse the same diffing & light-grouping logic as the complete implementation
    // by copying the same code path (for parity). This sends the minimal
    // SV_SETMAP* packets for the changed fast-window.
    {
        use core::constants::{SV_SETMAP, SV_SETMAP3, SV_SETMAP4, SV_SETMAP5, SV_SETMAP6};

        let tile_count = (core::constants::TILEX * core::constants::TILEY) as usize;

        let mut cmap = Server::with_players(|players| players[nr].cmap);
        let smap = Server::with_players(|players| players[nr].smap);

        fn it_base_status(n: u8) -> u8 {
            if n == 0 {
                return 0;
            }
            if n == 1 {
                return 1;
            }
            if n < 6 {
                return 2;
            }
            if n < 8 {
                return 6;
            }
            if n < 16 {
                return 8;
            }
            if n < 21 {
                return 16;
            }
            n
        }

        fn ch_base_status(n: u8) -> u8 {
            if n < 4 {
                return n;
            }
            if n < 16 {
                return n;
            }
            if n < 24 {
                return 16;
            }
            if n < 32 {
                return 24;
            }
            if n < 40 {
                return 32;
            }
            if n < 48 {
                return 40;
            }
            if n < 60 {
                return 48;
            }
            n
        }

        fn cmap_eq(a: &core::types::CMap, b: &core::types::CMap) -> bool {
            a.ba_sprite == b.ba_sprite
                && a.light == b.light
                && a.flags == b.flags
                && a.flags2 == b.flags2
                && a.ch_sprite == b.ch_sprite
                && a.ch_status2 == b.ch_status2
                && a.ch_status == b.ch_status
                && a.ch_speed == b.ch_speed
                && a.ch_nr == b.ch_nr
                && a.ch_id == b.ch_id
                && a.ch_proz == b.ch_proz
                && a.it_sprite == b.it_sprite
                && a.it_status == b.it_status
        }

        let cl_sizes = [1usize, 3usize, 7usize, 26usize];
        let cl_bytes = [3usize, 4usize, 6usize, 16usize];

        let mut n = 0usize;
        while n < tile_count {
            if cmap[n].light != smap[n].light {
                let mut best_idx = 0usize;
                let mut best_score = 0i32;
                for (li, &size) in cl_sizes.iter().enumerate() {
                    let end = std::cmp::min(n + size, tile_count);
                    let mut changed = 0usize;
                    for m in n..end {
                        if cmap[m].light != smap[m].light {
                            changed += 1;
                        }
                    }
                    let score = if cl_bytes[li] > 0 {
                        (50 * changed) as i32 / cl_bytes[li] as i32
                    } else {
                        0
                    };
                    if score >= best_score {
                        best_score = score;
                        best_idx = li;
                    }
                }

                match best_idx {
                    0 => {
                        let mut buf = [0u8; 16];
                        buf[0] = SV_SETMAP4;
                        let header = (n as u16) | ((smap[n].light as u16) << 12);
                        buf[1] = header as u8;
                        buf[2] = (header >> 8) as u8;
                        NetworkManager::with(|net| net.xsend(nr, &buf, 3));
                        cmap[n].light = smap[n].light;
                    }
                    1 => {
                        let mut buf = [0u8; 16];
                        buf[0] = SV_SETMAP5;
                        let header = (n as u16) | ((smap[n].light as u16) << 12);
                        buf[1] = header as u8;
                        buf[2] = (header >> 8) as u8;
                        let mut p = 3usize;
                        for m in (n + 2)..std::cmp::min(n + 3, tile_count) {
                            buf[p] = (smap[m].light & 0xF) | ((smap[m - 1].light & 0xF) << 4);
                            cmap[m].light = smap[m].light;
                            cmap[m - 1].light = smap[m - 1].light;
                            p += 1;
                        }
                        NetworkManager::with(|net| net.xsend(nr, &buf, p as u8));
                    }
                    2 => {
                        let mut buf = [0u8; 16];
                        buf[0] = SV_SETMAP6;
                        let header = (n as u16) | ((smap[n].light as u16) << 12);
                        buf[1] = header as u8;
                        buf[2] = (header >> 8) as u8;
                        let mut p = 3usize;
                        for m in (n + 2)..std::cmp::min(n + 7, tile_count) {
                            buf[p] = (smap[m].light & 0xF) | ((smap[m - 1].light & 0xF) << 4);
                            cmap[m].light = smap[m].light;
                            cmap[m - 1].light = smap[m - 1].light;
                            p += 1;
                        }
                        NetworkManager::with(|net| net.xsend(nr, &buf, p as u8));
                    }
                    3 => {
                        let mut buf = [0u8; 16];
                        buf[0] = SV_SETMAP3;
                        let header = (n as u16) | ((smap[n].light as u16) << 12);
                        buf[1] = header as u8;
                        buf[2] = (header >> 8) as u8;
                        let mut p = 3usize;
                        for m in (n + 2)..std::cmp::min(n + 26, tile_count) {
                            buf[p] = (smap[m].light & 0xF) | ((smap[m - 1].light & 0xF) << 4);
                            cmap[m].light = smap[m].light;
                            cmap[m - 1].light = smap[m - 1].light;
                            p += 1;
                        }
                        NetworkManager::with(|net| net.xsend(nr, &buf, 16));
                    }
                    _ => {}
                }
            }
            n += 1;
        }

        let mut lastn: i32 = -1;
        let mut idx = 0usize;
        loop {
            let mut found = None;
            for i in idx..tile_count {
                if !cmap_eq(&cmap[i], &smap[i]) {
                    found = Some(i);
                    break;
                }
            }
            let nidx = match found {
                Some(v) => v,
                None => break,
            };

            let mut buf = [0u8; 256];
            let mut p: usize;

            if lastn >= 0 && (nidx as i32) > lastn && (nidx as i32 - lastn) < 127 {
                buf[0] = (SV_SETMAP | ((nidx as i32 - lastn) as u8)) as u8;
                buf[1] = 0;
                p = 2;
            } else if lastn < 0 && nidx < 127 {
                buf[0] = (SV_SETMAP | ((nidx as u8) & 0x7F)) as u8;
                buf[1] = 0;
                p = 2;
            } else {
                buf[0] = SV_SETMAP;
                buf[1] = 0;
                let n_u16 = nidx as u16;
                buf[2] = n_u16 as u8;
                buf[3] = (n_u16 >> 8) as u8;
                p = 4;
            }

            if cmap[nidx].ba_sprite != smap[nidx].ba_sprite {
                buf[1] |= 1;
                let v = smap[nidx].ba_sprite as u16;
                buf[p] = v as u8;
                buf[p + 1] = (v >> 8) as u8;
                p += 2;
            }

            if cmap[nidx].flags != smap[nidx].flags {
                buf[1] |= 2;
                let v = smap[nidx].flags as u32;
                buf[p] = v as u8;
                buf[p + 1] = (v >> 8) as u8;
                buf[p + 2] = (v >> 16) as u8;
                buf[p + 3] = (v >> 24) as u8;
                p += 4;
            }

            if cmap[nidx].flags2 != smap[nidx].flags2 {
                buf[1] |= 4;
                let v = smap[nidx].flags2 as u32;
                buf[p] = v as u8;
                buf[p + 1] = (v >> 8) as u8;
                buf[p + 2] = (v >> 16) as u8;
                buf[p + 3] = (v >> 24) as u8;
                p += 4;
            }

            if cmap[nidx].it_sprite != smap[nidx].it_sprite {
                buf[1] |= 8;
                let v = smap[nidx].it_sprite as u16;
                buf[p] = v as u8;
                buf[p + 1] = (v >> 8) as u8;
                p += 2;
            }

            if cmap[nidx].it_status != smap[nidx].it_status
                && it_base_status(cmap[nidx].it_status) != it_base_status(smap[nidx].it_status)
            {
                buf[1] |= 16;
                buf[p] = smap[nidx].it_status;
                p += 1;
            }

            if cmap[nidx].ch_sprite != smap[nidx].ch_sprite
                || (cmap[nidx].ch_status != smap[nidx].ch_status
                    && ch_base_status(cmap[nidx].ch_status) != ch_base_status(smap[nidx].ch_status))
                || cmap[nidx].ch_status2 != smap[nidx].ch_status2
            {
                buf[1] |= 32;
                let v = smap[nidx].ch_sprite as u16;
                buf[p] = v as u8;
                buf[p + 1] = (v >> 8) as u8;
                p += 2;
                buf[p] = smap[nidx].ch_status;
                p += 1;
                buf[p] = smap[nidx].ch_status2;
                p += 1;
            }

            if cmap[nidx].ch_speed != smap[nidx].ch_speed
                || cmap[nidx].ch_nr != smap[nidx].ch_nr
                || cmap[nidx].ch_id != smap[nidx].ch_id
            {
                buf[1] |= 64;
                let vnr = smap[nidx].ch_nr as u16;
                buf[p] = vnr as u8;
                buf[p + 1] = (vnr >> 8) as u8;
                p += 2;
                let vid = smap[nidx].ch_id as u16;
                buf[p] = vid as u8;
                buf[p + 1] = (vid >> 8) as u8;
                p += 2;
                buf[p] = smap[nidx].ch_speed;
                p += 1;
            }

            if cmap[nidx].ch_proz != smap[nidx].ch_proz {
                buf[1] |= 128;
                buf[p] = smap[nidx].ch_proz;
                p += 1;
            }

            if buf[1] != 0 {
                NetworkManager::with(|net| net.xsend(nr, &buf, p as u8));
                lastn = nidx as i32;
            }

            cmap[nidx] = smap[nidx];
            idx = nidx + 1;
            if idx >= tile_count {
                break;
            }
        }

        Server::with_players_mut(|players| players[nr].cmap = cmap);
    }
}

/// Port of `plr_state` from `svr_tick.cpp`
/// Handles player state transitions (login, exit, timeouts)
pub fn plr_state(nr: usize) {
    let (ticker, lasttick, state) = Repository::with_globals(|globals| {
        Server::with_players(|players| {
            (
                globals.ticker as i32,
                players[nr].lasttick as i32,
                players[nr].state,
            )
        })
    });

    // Handle ST_EXIT timeout - close connection after 15 seconds
    if ticker.wrapping_sub(lasttick) > core::constants::TICKS * 15
        && state == core::constants::ST_EXIT
    {
        Server::with_players_mut(|players| {
            log::info!("Connection closed (ST_EXIT) for player {}", nr);
            // Close socket - the actual close happens in network layer
            players[nr].sock = None;
        });
        return;
    }

    // Handle idle timeout - logout after 60 seconds
    if ticker.wrapping_sub(lasttick) > core::constants::TICKS * 60 {
        log::info!("Idle timeout for player {}", nr);
        plr_logout(0, nr, enums::LogoutReason::IdleTooLong);
        return;
    }

    match state {
        state if state == core::constants::ST_NEWLOGIN => {
            plr_newlogin(nr);
        }
        state if state == core::constants::ST_LOGIN => {
            plr_login(nr);
        }
        state if state == core::constants::ST_NEWCAP => {
            // Timeout after 10 seconds, go back to NEWLOGIN
            if ticker.wrapping_sub(lasttick) > core::constants::TICKS * 10 {
                Server::with_players_mut(|players| {
                    players[nr].state = core::constants::ST_NEWLOGIN;
                });
            }
        }
        state if state == core::constants::ST_CAP => {
            // Timeout after 10 seconds, go back to LOGIN
            if ticker.wrapping_sub(lasttick) > core::constants::TICKS * 10 {
                Server::with_players_mut(|players| {
                    players[nr].state = core::constants::ST_LOGIN;
                });
            }
        }
        state if state == core::constants::ST_NEW_CHALLENGE => {
            // Do nothing - waiting for challenge response
        }
        state if state == core::constants::ST_LOGIN_CHALLENGE => {
            // Do nothing - waiting for challenge response
        }
        state if state == core::constants::ST_CONNECT => {
            // Do nothing - initial connection state
        }
        state if state == core::constants::ST_EXIT => {
            // Do nothing - handled above
        }
        _ => {
            log::warn!("UNKNOWN ST: {} for player {}", state, nr);
        }
    }
}

/// Port of `plr_newlogin` from `svr_tick.cpp`
/// Handles new player login (stub - to be implemented)
fn plr_newlogin(nr: usize) {
    // Port of C++ `plr_newlogin` from `svr_tick.cpp`.

    // version check
    let version = Server::with_players(|players| players[nr].version as u32);
    if version < core::constants::MINVERSION as u32 {
        log::warn!("Client too old ({}). Logout demanded", version);
        plr_logout(0, nr, enums::LogoutReason::VersionMismatch);
        return;
    }

    // ban check
    let addr = Server::with_players(|players| players[nr].addr);
    if God::is_banned(addr as i32) {
        log::info!("Banned, sent away");
        plr_logout(0, nr, enums::LogoutReason::Kicked);
        return;
    }

    // TODO: `cap()` handling (player cap/queue) not implemented yet.

    // sanitize race
    let mut temp = Server::with_players(|players| players[nr].race as i32);
    if temp != 2 && temp != 3 && temp != 4 && temp != 76 && temp != 77 && temp != 78 {
        temp = 2;
    }

    // create new character from template
    let maybe_cn = God::create_char(temp as usize, true);
    let cn = match maybe_cn {
        Some(v) => v as usize,
        None => {
            log::error!("plr_newlogin: failed to create character");
            plr_logout(0, nr, enums::LogoutReason::Failure);
            return;
        }
    };

    Repository::with_characters_mut(|characters| {
        characters[cn].player = nr as i32;
        characters[cn].temple_x = core::constants::HOME_MERCENARY_X as u16;
        characters[cn].temple_y = core::constants::HOME_MERCENARY_Y as u16;
        characters[cn].tavern_x = core::constants::HOME_MERCENARY_X as u16;
        characters[cn].tavern_y = core::constants::HOME_MERCENARY_Y as u16;
        characters[cn].points = 0;
        characters[cn].points_tot = 0;
        characters[cn].luck = 205;
    });

    Repository::with_globals_mut(|globals| {
        globals.players_created += 1;
    });

    // Try dropping the character near the home temple (three attempts)
    if !God::drop_char_fuzzy_large(
        cn,
        core::constants::HOME_MERCENARY_X as usize,
        core::constants::HOME_MERCENARY_Y as usize,
        core::constants::HOME_MERCENARY_X as usize,
        core::constants::HOME_MERCENARY_Y as usize,
    ) && !God::drop_char_fuzzy_large(
        cn,
        (core::constants::HOME_MERCENARY_X + 3) as usize,
        core::constants::HOME_MERCENARY_Y as usize,
        core::constants::HOME_MERCENARY_X as usize,
        core::constants::HOME_MERCENARY_Y as usize,
    ) && !God::drop_char_fuzzy_large(
        cn,
        core::constants::HOME_MERCENARY_X as usize,
        (core::constants::HOME_MERCENARY_Y + 3) as usize,
        core::constants::HOME_MERCENARY_X as usize,
        core::constants::HOME_MERCENARY_Y as usize,
    ) {
        log::error!("plr_newlogin(): could not drop new character");
        plr_logout(cn, nr, enums::LogoutReason::NoRoom);
        Repository::with_characters_mut(|characters| {
            characters[cn].used = core::constants::USE_EMPTY;
        });
        return;
    }

    // Set creation/login dates and flags, record address and add to net history
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as u32;

    Repository::with_characters_mut(|characters| {
        let ch = &mut characters[cn];
        ch.creation_date = now;
        ch.login_date = now;
        ch.flags |= enums::CharacterFlags::NewUser.bits() | enums::CharacterFlags::Player.bits();
        ch.addr = Server::with_players(|players| players[nr].addr);

        // char_add_net behaviour: shift data[80..89] and insert lower 24 bits of addr
        let net = (ch.addr & 0x00ffffff) as i32;
        let mut n = 80usize;
        while n < 89 {
            if (ch.data[n] & 0x00ffffff) == net {
                break;
            }
            n += 1;
        }
        for m in (81..=n).rev() {
            ch.data[m] = ch.data[m - 1];
        }
        ch.data[80] = net;

        ch.mode = 1;
    });

    // update character to clients
    State::with(|state| {
        state.do_update_char(cn);
    });

    // set player mapping and send SV_NEWPLAYER + SV_TICK
    Repository::with_characters(|characters| {
        let pass1 = characters[cn].pass1;
        let pass2 = characters[cn].pass2;

        Server::with_players_mut(|players| {
            players[nr].usnr = cn;
            players[nr].pass1 = pass1;
            players[nr].pass2 = pass2;
        });

        log::info!(
            "New player logged in as character index={} (players index={})",
            cn,
            nr
        );

        let mut buf: [u8; 16] = [0; 16];
        buf[0] = core::constants::SV_NEWPLAYER;
        buf[1..5].copy_from_slice(&(cn as u32).to_le_bytes());
        buf[5..9].copy_from_slice(&pass1.to_le_bytes());
        buf[9..13].copy_from_slice(&pass2.to_le_bytes());
        let ver_bytes = core::constants::VERSION.to_le_bytes();
        buf[13] = ver_bytes[0];
        buf[14] = ver_bytes[1];
        buf[15] = ver_bytes[2];

        NetworkManager::with(|network| {
            network.csend(nr, &buf, 16);
        });
    });

    // finalize player state
    let ticker = Repository::with_globals(|globals| globals.ticker as u32);
    Server::with_players_mut(|players| {
        players[nr].state = core::constants::ST_NORMAL;
        players[nr].lasttick = ticker;
        players[nr].ltick = 0;
        players[nr].ticker_started = 1;
    });

    // send tick
    let mut tbuf: [u8; 2] = [0; 2];
    tbuf[0] = core::constants::SV_TICK;
    tbuf[1] = Repository::with_globals(|globals| (globals.ticker % 20) as u8);
    NetworkManager::with(|network| {
        network.xsend(nr, &tbuf, 2);
    });

    log::info!("Created new character");

    // intro messages
    let intro1 = "Welcome to Men Among Gods, my friend!\n";
    let intro2 = "May your visit here be... interesting.\n";
    let intro3 = " \n";
    let intro4 = "Use #help (or /help) to get a listing of the text commands.\n";

    State::with(|state| {
        state.do_character_log(cn, core::types::FontColor::Yellow, intro1);
        state.do_character_log(cn, core::types::FontColor::Yellow, intro3);
        state.do_character_log(cn, core::types::FontColor::Yellow, intro2);
        state.do_character_log(cn, core::types::FontColor::Yellow, intro3);
        state.do_character_log(cn, core::types::FontColor::Yellow, intro4);
        state.do_character_log(cn, core::types::FontColor::Yellow, intro3);
    });

    // change password if client provided one and character has no CF_PASSWD
    let needs_pass = Server::with_players(|players| players[nr].passwd[0] != 0);
    if needs_pass {
        Repository::with_characters(|characters| {
            if (characters[cn].flags & enums::CharacterFlags::Passwd.bits()) == 0 {
                // extract password string
                let pass = Server::with_players(|players| {
                    String::from_utf8_lossy(&players[nr].passwd)
                        .trim_end_matches(char::from(0))
                        .to_string()
                });
                God::change_pass(cn, cn, &pass);
            }
        });
    }

    // announce
    State::with(|state| {
        state.do_announce(cn, 0, &format!("A new player has entered the game.\n"));
    });
}

/// Port of `plr_login` from `svr_tick.cpp`
/// Handles existing player login (stub - to be implemented)
fn plr_login(nr: usize) {
    // version check
    let version = Server::with_players(|players| players[nr].version as u32);
    if version < core::constants::MINVERSION as u32 {
        log::warn!("Client too old ({}). Logout demanded", version);
        plr_logout(0, nr, enums::LogoutReason::VersionMismatch);
        return;
    }

    // get character number requested by player
    let cn = Server::with_players(|players| players[nr].usnr as usize);

    if cn == 0 || cn >= core::constants::MAXCHARS {
        log::warn!("Login as {} denied (illegal cn)", cn);
        plr_logout(0, nr, enums::LogoutReason::ParamsInvalid);
        return;
    }

    // password/pass1/pass2 check
    let pass_ok = Repository::with_characters(|characters| {
        let ch = &characters[cn];
        let p1 = ch.pass1;
        let p2 = ch.pass2;
        let player_p1 = Server::with_players(|players| players[nr].pass1);
        let player_p2 = Server::with_players(|players| players[nr].pass2);
        p1 == player_p1 && p2 == player_p2
    });

    if !pass_ok {
        log::warn!("Login as {} denied (pass1/pass2)", cn);
        plr_logout(0, nr, enums::LogoutReason::PasswordIncorrect);
        return;
    }

    // If character has explicit password flag, compare stored passwd
    let has_passwd_mismatch = Repository::with_characters(|characters| {
        let ch = &characters[cn];
        if (ch.flags & enums::CharacterFlags::Passwd.bits()) != 0 {
            let stored = ch.passwd;
            let client = Server::with_players(|players| players[nr].passwd);
            stored != client
        } else {
            false
        }
    });

    if has_passwd_mismatch {
        log::warn!("Login as {} denied (password)", cn);
        plr_logout(0, nr, enums::LogoutReason::PasswordIncorrect);
        return;
    }

    // Deleted account
    let is_deleted =
        Repository::with_characters(|characters| characters[cn].used == core::constants::USE_EMPTY);
    if is_deleted {
        log::warn!("Login as {} denied (deleted)", cn);
        plr_logout(0, nr, enums::LogoutReason::PasswordIncorrect);
        return;
    }

    // Already active
    let already_active = Repository::with_characters(|characters| {
        characters[cn].used != core::constants::USE_EMPTY as u8
            && (characters[cn].flags & enums::CharacterFlags::ComputerControlledPlayer.bits()) == 0
    });
    if already_active {
        log::warn!("Login as {} who is already active", cn);
        let active_player =
            Repository::with_characters(|characters| characters[cn].player as usize);
        plr_logout(cn, active_player, enums::LogoutReason::IdleTooLong);
        return;
    }

    // Kicked
    let is_kicked = Repository::with_characters(|characters| {
        (characters[cn].flags & enums::CharacterFlags::Kicked.bits()) != 0
    });
    if is_kicked {
        log::warn!("Login as {} denied (kicked)", cn);
        plr_logout(0, nr, enums::LogoutReason::Kicked);
        return;
    }

    // Ban check (skip golden/god)
    let banned = Server::with_players(|players| players[nr].addr);
    let exempt = Repository::with_characters(|characters| {
        (characters[cn].flags
            & (enums::CharacterFlags::Golden.bits() | enums::CharacterFlags::God.bits()))
            != 0
    });
    if !exempt && God::is_banned(banned as i32) {
        log::info!("{} is banned, sent away", cn);
        plr_logout(0, nr, enums::LogoutReason::Kicked);
        return;
    }

    // TODO: cap() handling (player cap/queue) not implemented - skip

    // attach player to character
    Repository::with_characters_mut(|characters| {
        characters[cn].player = nr as i32;
        // If not CCP and is god, mark invisible
        if (characters[cn].flags & enums::CharacterFlags::ComputerControlledPlayer.bits()) == 0
            && (characters[cn].flags & enums::CharacterFlags::God.bits()) != 0
        {
            characters[cn].flags |= enums::CharacterFlags::Invisible.bits();
        }
    });

    // finalize player state
    let ticker = Repository::with_globals(|globals| globals.ticker as u32);
    Server::with_players_mut(|players| {
        players[nr].state = core::constants::ST_NORMAL;
        players[nr].lasttick = ticker;
        players[nr].ltick = 0;
        players[nr].ticker_started = 1;
    });

    // send LOGIN_OK
    let mut buf: [u8; 16] = [0; 16];
    buf[0] = core::constants::SV_LOGIN_OK;
    buf[1..5].copy_from_slice(&core::constants::VERSION.to_le_bytes());
    NetworkManager::with(|network| {
        network.csend(nr, &buf, 16);
    });

    // send tick
    let mut tbuf: [u8; 2] = [0; 2];
    tbuf[0] = core::constants::SV_TICK;
    tbuf[1] = Repository::with_globals(|globals| (globals.ticker % 20) as u8);
    NetworkManager::with(|network| {
        network.xsend(nr, &tbuf, 2);
    });

    // mark active and set login date, addr, add net history
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as u32;

    Repository::with_characters_mut(|characters| {
        let ch = &mut characters[cn];
        ch.used = core::constants::USE_ACTIVE;
        ch.login_date = now;
        ch.addr = Server::with_players(|players| players[nr].addr);
        ch.current_online_time = 0;

        // char_add_net behaviour: shift data[80..89] and insert lower 24 bits
        let net = (ch.addr & 0x00ffffff) as i32;
        let mut nidx = 80usize;
        while nidx < 89 {
            if (ch.data[nidx] & 0x00ffffff) == net {
                break;
            }
            nidx += 1;
        }
        for m in (81..=nidx).rev() {
            ch.data[m] = ch.data[m - 1];
        }
        ch.data[80] = net;
    });

    // ensure client player mode default
    Server::with_players_mut(|players| players[nr].cpl.mode = -1);

    // Try to drop character at tavern/nearby
    let tav_x = Repository::with_characters(|characters| characters[cn].tavern_x as usize);
    let tav_y = Repository::with_characters(|characters| characters[cn].tavern_y as usize);
    if !God::drop_char_fuzzy_large(cn, tav_x, tav_y, tav_x, tav_y)
        && !God::drop_char_fuzzy_large(cn, tav_x + 3, tav_y, tav_x, tav_y)
        && !God::drop_char_fuzzy_large(cn, tav_x, tav_y + 3, tav_x, tav_y)
    {
        log::error!("plr_login(): could not drop new character");
        plr_logout(cn, nr, enums::LogoutReason::NoRoom);
        return;
    }

    // remove illegal active recall spells
    for i in 0..20usize {
        let has_recall = Repository::with_characters(|characters| characters[cn].spell[i] != 0);
        if has_recall {
            let spell_idx =
                Repository::with_characters(|characters| characters[cn].spell[i] as usize);
            let is_recall = Repository::with_items(|items| {
                items[spell_idx].temp == core::constants::SK_RECALL as u16
            });
            if is_recall {
                Repository::with_items_mut(|items| {
                    items[spell_idx].used = core::constants::USE_EMPTY;
                });
                Repository::with_characters_mut(|characters| {
                    characters[cn].spell[i] = 0;
                });
                State::with(|state| {
                    state.do_character_log(
                        cn,
                        core::types::FontColor::Red,
                        "CHEATER: removed active teleport\n",
                    );
                });
            }
        }
    }

    // update client about char
    State::with(|state| {
        state.do_update_char(cn);
    });

    log::info!("Login successful");

    // intro messages
    let intro1 = "Welcome to Men Among Gods, my friend!\n";
    let intro2 = "May your visit here be... interesting.\n";
    let intro3 = " \n";
    let intro4 = "Use #help (or /help) to get a listing of the text commands.\n";

    State::with(|state| {
        state.do_character_log(cn, core::types::FontColor::Yellow, intro1);
        state.do_character_log(cn, core::types::FontColor::Yellow, intro3);
        state.do_character_log(cn, core::types::FontColor::Yellow, intro2);
        state.do_character_log(cn, core::types::FontColor::Yellow, intro3);
        state.do_character_log(cn, core::types::FontColor::Yellow, intro4);
        state.do_character_log(cn, core::types::FontColor::Yellow, intro3);
    });

    // do password change if provided
    let needs_pass = Server::with_players(|players| players[nr].passwd[0] != 0);
    if needs_pass {
        Repository::with_characters(|characters| {
            if (characters[cn].flags & enums::CharacterFlags::Passwd.bits()) == 0 {
                let pass = Server::with_players(|players| {
                    String::from_utf8_lossy(&players[nr].passwd)
                        .trim_end_matches(char::from(0))
                        .to_string()
                });
                God::change_pass(cn, cn, &pass);
            }
        });
    }

    // If god, remind invisibility
    Repository::with_characters(|characters| {
        if (characters[cn].flags & enums::CharacterFlags::ComputerControlledPlayer.bits()) == 0
            && (characters[cn].flags & enums::CharacterFlags::God.bits()) != 0
        {
            State::with(|state| {
                state.do_character_log(
                    cn,
                    core::types::FontColor::Blue,
                    "Remember, you are invisible!\n",
                )
            });
        }
    });

    // announce
    let name = Repository::with_characters(|characters| characters[cn].get_name().to_string());
    State::with(|state| {
        state.do_announce(cn, 0, &format!("{} entered the game.\n", name));
    });
}

/// Port of `plr_change` from `svr_tick.cpp`
/// Sends changed player data to the client
pub fn plr_change(nr: usize) {
    let cn = Server::with_players(|players| players[nr].usnr as usize);

    if cn == 0 || cn >= core::constants::MAXCHARS {
        return;
    }

    let (ticker, should_update) = Repository::with_globals(|globals| {
        Repository::with_characters(|ch| {
            let has_update_flag = (ch[cn].flags & enums::CharacterFlags::Update.bits()) != 0;
            let ticker_match = (cn & 15) == (globals.ticker as usize & 15);
            (globals.ticker as i32, has_update_flag || ticker_match)
        })
    });

    if should_update {
        // Send full player stats update
        plr_change_stats(nr, cn, ticker);
    }

    // Always send combat-related updates
    plr_change_hp(nr, cn);
    plr_change_end(nr, cn);
    plr_change_mana(nr, cn);
    plr_change_dir(nr, cn);
    plr_change_points(nr, cn);
    plr_change_gold(nr, cn);
    plr_change_position(nr, cn);
    plr_change_target(nr, cn);

    // Additional updates for name, mode, attributes, skills, items, and spells
    Repository::with_characters(|characters| {
        let character = &characters[cn];
        Server::with_players_mut(|players| {
            let player = &mut players[nr];

            // Mode updates
            if player.cpl.mode != character.mode as i32 {
                player.cpl.mode = character.mode as i32;
                NetworkManager::with(|network| {
                    let mut buf = [0u8; 2];
                    buf[0] = core::constants::SV_SETCHAR_MODE;
                    buf[1] = character.mode as u8;
                    network.xsend(nr, &buf, 2);
                });
            }

            // Attributes
            for i in 0..5 {
                if player.cpl.attrib[i] != character.attrib[i] {
                    player.cpl.attrib[i] = character.attrib[i];
                    NetworkManager::with(|network| {
                        let mut buf: [u8; 8] = [0; 8];
                        buf[0] = core::constants::SV_SETCHAR_ATTRIB;
                        buf[1] = i as u8;
                        buf[2..8].copy_from_slice(&character.attrib[i]);
                        network.xsend(nr, &buf, 8);
                    });
                }
            }

            // Items
            for i in 0..40 {
                if player.cpl.item[i] != character.item[i] as i32 {
                    player.cpl.item[i] = character.item[i] as i32;
                    let in_idx = character.item[i] as usize;
                    let mut sprite: i16 = 0;
                    let mut placement: i16 = 0;
                    if in_idx != 0 && in_idx < core::constants::MAXITEM as usize {
                        Repository::with_items(|items| {
                            let it = &items[in_idx];
                            sprite = if it.active != 0 {
                                it.sprite[1]
                            } else {
                                it.sprite[0]
                            };
                            placement = it.placement as i16;
                        });
                    }
                    NetworkManager::with(|network| {
                        let mut buf: [u8; 9] = [0; 9];
                        buf[0] = core::constants::SV_SETCHAR_ITEM;
                        let idx_bytes = (i as u32).to_le_bytes();
                        buf[1..5].copy_from_slice(&idx_bytes);
                        buf[5] = (sprite & 0xff) as u8;
                        buf[6] = ((sprite >> 8) & 0xff) as u8;
                        buf[7] = (placement & 0xff) as u8;
                        buf[8] = ((placement >> 8) & 0xff) as u8;
                        network.xsend(nr, &buf, 9);
                    });
                }
            }

            // Worn Items
            for i in 0..20 {
                if player.cpl.worn[i] != character.worn[i] as i32 {
                    player.cpl.worn[i] = character.worn[i] as i32;
                    let in_idx = character.worn[i] as usize;
                    let mut sprite: i16 = 0;
                    let mut placement: i16 = 0;
                    if in_idx != 0 && in_idx < core::constants::MAXITEM as usize {
                        Repository::with_items(|items| {
                            let it = &items[in_idx];
                            sprite = if it.active != 0 {
                                it.sprite[1]
                            } else {
                                it.sprite[0]
                            };
                            placement = it.placement as i16;
                        });
                    }
                    NetworkManager::with(|network| {
                        let mut buf: [u8; 9] = [0; 9];
                        buf[0] = core::constants::SV_SETCHAR_WORN;
                        let idx_bytes = (i as u32).to_le_bytes();
                        buf[1..5].copy_from_slice(&idx_bytes);
                        buf[5] = (sprite & 0xff) as u8;
                        buf[6] = ((sprite >> 8) & 0xff) as u8;
                        buf[7] = (placement & 0xff) as u8;
                        buf[8] = ((placement >> 8) & 0xff) as u8;
                        network.xsend(nr, &buf, 9);
                    });
                }
            }

            // Spells
            for i in 0..20 {
                if player.cpl.spell[i] != character.spell[i] as i32 {
                    player.cpl.spell[i] = character.spell[i] as i32;
                    let in_idx = character.spell[i] as usize;
                    let mut sprite: i16 = 0;
                    let mut active_frac: i16 = 0;
                    if in_idx != 0 && in_idx < core::constants::MAXITEM as usize {
                        Repository::with_items(|items| {
                            let it = &items[in_idx];
                            sprite = it.sprite[1];
                            active_frac = if it.duration > 0 {
                                (it.active * 16 / it.duration) as i16
                            } else {
                                0
                            };
                        });
                    }
                    NetworkManager::with(|network| {
                        let mut buf: [u8; 9] = [0; 9];
                        buf[0] = core::constants::SV_SETCHAR_SPELL;
                        let idx_bytes = (i as u32).to_le_bytes();
                        buf[1..5].copy_from_slice(&idx_bytes);
                        buf[5] = (sprite & 0xff) as u8;
                        buf[6] = ((sprite >> 8) & 0xff) as u8;
                        buf[7] = (active_frac & 0xff) as u8;
                        buf[8] = ((active_frac >> 8) & 0xff) as u8;
                        network.xsend(nr, &buf, 9);
                    });
                }
            }
        });
    });
}

/// Send full stats update to player
fn plr_change_stats(nr: usize, cn: usize, _ticker: i32) {
    // Send name in three parts if changed
    let name_changed = Repository::with_characters(|characters| {
        let ch = &characters[cn];
        Server::with_players(|players| &players[nr].cpl.name[..] != &ch.name[..])
    });

    if name_changed {
        Repository::with_characters(|characters| {
            let ch = &characters[cn];
            // part1: 15 bytes
            let mut buf: [u8; 16] = [0; 16];
            buf[0] = core::constants::SV_SETCHAR_NAME1;
            buf[1..16].copy_from_slice(&ch.name[0..15]);
            NetworkManager::with(|network| network.xsend(nr, &buf, 16));

            // part2: next 15 bytes
            let mut buf2: [u8; 16] = [0; 16];
            buf2[0] = core::constants::SV_SETCHAR_NAME2;
            buf2[1..16].copy_from_slice(&ch.name[15..30]);
            NetworkManager::with(|network| network.xsend(nr, &buf2, 16));

            // part3: last 10 bytes + temp (u16 -> u32 slot)
            let mut buf3: [u8; 16] = [0; 16];
            buf3[0] = core::constants::SV_SETCHAR_NAME3;
            buf3[1..11].copy_from_slice(&ch.name[30..40]);
            let temp_bytes = (ch.temp as u32).to_le_bytes();
            buf3[11..15].copy_from_slice(&temp_bytes[0..4]);
            NetworkManager::with(|network| network.xsend(nr, &buf3, 16));

            // copy into cpl
            Server::with_players_mut(|players| players[nr].cpl.name.copy_from_slice(&ch.name));
        });
    }

    // mode
    Repository::with_characters(|characters| {
        let mode = characters[cn].mode as i32;
        Server::with_players(|players| players[nr].cpl.mode != mode)
    });
    // send mode if different
    let need_mode = Repository::with_characters(|characters| {
        let mode = characters[cn].mode as i32;
        Server::with_players(|players| players[nr].cpl.mode != mode)
    });
    if need_mode {
        let mode = Repository::with_characters(|characters| characters[cn].mode as u8);
        let mut buf: [u8; 2] = [0; 2];
        buf[0] = core::constants::SV_SETCHAR_MODE;
        buf[1] = mode;
        NetworkManager::with(|network| network.xsend(nr, &buf, 2));
        Server::with_players_mut(|players| players[nr].cpl.mode = mode as i32);
    }

    // attribs (5 x 6 bytes)
    for a in 0..5usize {
        let changed = Repository::with_characters(|characters| {
            let chv = &characters[cn].attrib[a];
            Server::with_players(|players| players[nr].cpl.attrib[a] != *chv)
        });
        if changed {
            let bytes = Repository::with_characters(|characters| characters[cn].attrib[a]);
            let mut buf: [u8; 8] = [0; 8];
            buf[0] = core::constants::SV_SETCHAR_ATTRIB;
            buf[1] = a as u8;
            buf[2..8].copy_from_slice(&bytes);
            NetworkManager::with(|network| network.xsend(nr, &buf, 8));
            Server::with_players_mut(|players| players[nr].cpl.attrib[a] = bytes);
        }
    }

    // hp, end, mana arrays (6 u16 each)
    let powers = [
        core::constants::SV_SETCHAR_HP,
        core::constants::SV_SETCHAR_ENDUR,
        core::constants::SV_SETCHAR_MANA,
    ];
    for (idx, code) in powers.iter().enumerate() {
        let different = Repository::with_characters(|characters| {
            let ch = &characters[cn];
            Server::with_players(|players| match idx {
                0 => {
                    let ch_hp = ch.hp;
                    players[nr].cpl.hp != ch_hp
                }
                1 => {
                    let end = ch.end;
                    players[nr].cpl.end != end
                }
                2 => {
                    let mana = ch.mana;
                    players[nr].cpl.mana != mana
                }
                _ => false,
            })
        });
        if different {
            let mut buf: [u8; 13] = [0; 13];
            buf[0] = *code;
            Repository::with_characters(|characters| {
                let ch = &characters[cn];
                let arr: [u16; 6] = match idx {
                    0 => {
                        let hp = ch.hp;
                        hp
                    }
                    1 => {
                        let end = ch.end;
                        end
                    }
                    2 => {
                        let mana = ch.mana;
                        mana
                    }
                    _ => {
                        let hp = ch.hp;
                        hp
                    }
                };
                for i in 0..6 {
                    let off = 1 + i * 2;
                    let v = arr[i];
                    buf[off] = (v & 0xff) as u8;
                    buf[off + 1] = (v >> 8) as u8;
                }
            });
            NetworkManager::with(|network| network.xsend(nr, &buf, 13));
            // copy into cpl
            Server::with_players_mut(|players| {
                Repository::with_characters(|characters| {
                    let ch = &characters[cn];
                    match idx {
                        0 => players[nr].cpl.hp = ch.hp,
                        1 => players[nr].cpl.end = ch.end,
                        2 => players[nr].cpl.mana = ch.mana,
                        _ => {}
                    }
                });
            });
        }
    }

    // skills (0..50)
    for s in 0..50usize {
        let changed = Repository::with_characters(|characters| {
            let chv = &characters[cn].skill[s];
            Server::with_players(|players| players[nr].cpl.skill[s] != *chv)
        });
        if changed {
            let bytes = Repository::with_characters(|characters| characters[cn].skill[s]);
            let mut buf: [u8; 8] = [0; 8];
            buf[0] = core::constants::SV_SETCHAR_SKILL;
            buf[1] = s as u8;
            buf[2..8].copy_from_slice(&bytes);
            NetworkManager::with(|network| network.xsend(nr, &buf, 8));
            Server::with_players_mut(|players| players[nr].cpl.skill[s] = bytes);
        }
    }

    // items (40)
    for i in 0..40usize {
        let changed = Repository::with_characters(|characters| {
            let ch_in = characters[cn].item[i] as i32;
            Server::with_players(|players| players[nr].cpl.item[i] != ch_in)
        });
        if changed {
            let in_idx = Repository::with_characters(|characters| characters[cn].item[i] as usize);
            let mut sprite: i16 = 0;
            let mut placement: i16 = 0;
            if in_idx != 0 && in_idx < core::constants::MAXITEM as usize {
                Repository::with_items(|items| {
                    let it = &items[in_idx];
                    sprite = if it.active != 0 {
                        it.sprite[1]
                    } else {
                        it.sprite[0]
                    };
                    placement = it.placement as i16;
                });
            }
            let mut buf: [u8; 9] = [0; 9];
            buf[0] = core::constants::SV_SETCHAR_ITEM;
            let idx_bytes = (i as u32).to_le_bytes();
            buf[1..5].copy_from_slice(&idx_bytes);
            buf[5] = (sprite & 0xff) as u8;
            buf[6] = ((sprite >> 8) & 0xff) as u8;
            buf[7] = (placement & 0xff) as u8;
            buf[8] = ((placement >> 8) & 0xff) as u8;
            NetworkManager::with(|network| network.xsend(nr, &buf, 9));
            Server::with_players_mut(|players| players[nr].cpl.item[i] = in_idx as i32);
        }
    }

    // worn (20)
    for i in 0..20usize {
        let changed = Repository::with_characters(|characters| {
            let ch_in = characters[cn].worn[i] as i32;
            Server::with_players(|players| players[nr].cpl.worn[i] != ch_in)
        });
        if changed {
            let in_idx = Repository::with_characters(|characters| characters[cn].worn[i] as usize);
            let mut sprite: i16 = 0;
            let mut placement: i16 = 0;
            if in_idx != 0 && in_idx < core::constants::MAXITEM as usize {
                Repository::with_items(|items| {
                    let it = &items[in_idx];
                    sprite = if it.active != 0 {
                        it.sprite[1]
                    } else {
                        it.sprite[0]
                    };
                    placement = it.placement as i16;
                });
            }
            let mut buf: [u8; 9] = [0; 9];
            buf[0] = core::constants::SV_SETCHAR_WORN;
            let idx_bytes = (i as u32).to_le_bytes();
            buf[1..5].copy_from_slice(&idx_bytes);
            buf[5] = (sprite & 0xff) as u8;
            buf[6] = ((sprite >> 8) & 0xff) as u8;
            buf[7] = (placement & 0xff) as u8;
            buf[8] = ((placement >> 8) & 0xff) as u8;
            NetworkManager::with(|network| network.xsend(nr, &buf, 9));
            Server::with_players_mut(|players| players[nr].cpl.worn[i] = in_idx as i32);
        }
    }

    // spells (20)
    for i in 0..20usize {
        let changed = Repository::with_characters(|characters| {
            let ch_in = characters[cn].spell[i] as i32;
            Server::with_players(|players| players[nr].cpl.spell[i] != ch_in)
        });
        if changed {
            let in_idx = Repository::with_characters(|characters| characters[cn].spell[i] as usize);
            let mut sprite: i16 = 0;
            let mut active_frac: i16 = 0;
            if in_idx != 0 && in_idx < core::constants::MAXITEM as usize {
                Repository::with_items(|items| {
                    let it = &items[in_idx];
                    sprite = it.sprite[1];
                    active_frac = if it.duration > 0 {
                        (it.active * 16 / it.duration) as i16
                    } else {
                        0
                    };
                });
            }
            let mut buf: [u8; 9] = [0; 9];
            buf[0] = core::constants::SV_SETCHAR_SPELL;
            let idx_bytes = (i as u32).to_le_bytes();
            buf[1..5].copy_from_slice(&idx_bytes);
            buf[5] = (sprite & 0xff) as u8;
            buf[6] = ((sprite >> 8) & 0xff) as u8;
            buf[7] = (active_frac & 0xff) as u8;
            buf[8] = ((active_frac >> 8) & 0xff) as u8;
            NetworkManager::with(|network| network.xsend(nr, &buf, 9));
            Server::with_players_mut(|players| {
                players[nr].cpl.spell[i] = in_idx as i32;
                players[nr].cpl.active[i] = active_frac as i8;
            });
        }
    }

    // citem
    let citem_changed = Repository::with_characters(|characters| {
        characters[cn].citem as i32 != Server::with_players(|players| players[nr].cpl.citem)
    });
    if citem_changed {
        let in_idx = Repository::with_characters(|characters| characters[cn].citem as usize);
        let mut sprite: i16 = 0;
        let mut placement: i16 = 0;
        if in_idx != 0 && in_idx < core::constants::MAXITEM as usize {
            Repository::with_items(|items| {
                let it = &items[in_idx];
                sprite = if it.active != 0 {
                    it.sprite[1]
                } else {
                    it.sprite[0]
                };
                placement = it.placement as i16;
            });
        }
        let mut buf: [u8; 5] = [0; 5];
        buf[0] = core::constants::SV_SETCHAR_OBJ;
        buf[1] = (sprite & 0xff) as u8;
        buf[2] = ((sprite >> 8) & 0xff) as u8;
        buf[3] = (placement & 0xff) as u8;
        buf[4] = ((placement >> 8) & 0xff) as u8;
        NetworkManager::with(|network| network.xsend(nr, &buf, 5));
        Server::with_players_mut(|players| players[nr].cpl.citem = in_idx as i32);
    }

    // a_hp, a_end, a_mana (these are scaled)
    let a_hp_val =
        Repository::with_characters(|characters| ((characters[cn].a_hp + 500) / 1000) as i32);
    if Server::with_players(|players| players[nr].cpl.a_hp) != a_hp_val {
        let mut buf: [u8; 3] = [0; 3];
        buf[0] = core::constants::SV_SETCHAR_AHP;
        buf[1] = (a_hp_val & 0xff) as u8;
        buf[2] = ((a_hp_val >> 8) & 0xff) as u8;
        NetworkManager::with(|network| network.xsend(nr, &buf, 3));
        Server::with_players_mut(|players| players[nr].cpl.a_hp = a_hp_val);
    }

    let a_end_val =
        Repository::with_characters(|characters| ((characters[cn].a_end + 500) / 1000) as i32);
    if Server::with_players(|players| players[nr].cpl.a_end) != a_end_val {
        let mut buf: [u8; 3] = [0; 3];
        buf[0] = core::constants::SV_SETCHAR_AEND;
        buf[1] = (a_end_val & 0xff) as u8;
        buf[2] = ((a_end_val >> 8) & 0xff) as u8;
        NetworkManager::with(|network| network.xsend(nr, &buf, 3));
        Server::with_players_mut(|players| players[nr].cpl.a_end = a_end_val);
    }

    let a_mana_val =
        Repository::with_characters(|characters| ((characters[cn].a_mana + 500) / 1000) as i32);
    if Server::with_players(|players| players[nr].cpl.a_mana) != a_mana_val {
        let mut buf: [u8; 3] = [0; 3];
        buf[0] = core::constants::SV_SETCHAR_AMANA;
        buf[1] = (a_mana_val & 0xff) as u8;
        buf[2] = ((a_mana_val >> 8) & 0xff) as u8;
        NetworkManager::with(|network| network.xsend(nr, &buf, 3));
        Server::with_players_mut(|players| players[nr].cpl.a_mana = a_mana_val);
    }

    // dir
    let dir_changed = Repository::with_characters(|characters| {
        characters[cn].dir as i32 != Server::with_players(|players| players[nr].cpl.dir)
    });
    if dir_changed {
        let dir = Repository::with_characters(|characters| characters[cn].dir as u8);
        let mut buf: [u8; 16] = [0; 16];
        buf[0] = core::constants::SV_SETCHAR_DIR;
        buf[1] = dir;
        NetworkManager::with(|network| network.xsend(nr, &buf, 2));
        Server::with_players_mut(|players| players[nr].cpl.dir = dir as i32);
    }

    // points and gold
    let points_changed = Repository::with_characters(|characters| {
        let ch = &characters[cn];
        Server::with_players(|players| {
            players[nr].cpl.points != ch.points
                || players[nr].cpl.points_tot != ch.points_tot
                || players[nr].cpl.kindred != ch.kindred
        })
    });
    if points_changed {
        Repository::with_characters(|characters| {
            let ch = &characters[cn];
            let mut buf: [u8; 10] = [0; 10];
            buf[0] = core::constants::SV_SETCHAR_PTS;
            buf[1..5].copy_from_slice(&ch.points.to_le_bytes());
            buf[5..9].copy_from_slice(&ch.points_tot.to_le_bytes());
            buf[9] = ch.kindred as u8;
            NetworkManager::with(|network| network.xsend(nr, &buf, 10));
            Server::with_players_mut(|players| {
                players[nr].cpl.points = ch.points;
                players[nr].cpl.points_tot = ch.points_tot;
                players[nr].cpl.kindred = ch.kindred;
            });
        });
    }

    let gold_changed = Repository::with_characters(|characters| {
        let ch = &characters[cn];
        Server::with_players(|players| {
            players[nr].cpl.gold != ch.gold
                || players[nr].cpl.armor != ch.armor as i32
                || players[nr].cpl.weapon != ch.weapon as i32
        })
    });
    if gold_changed {
        Repository::with_characters(|characters| {
            let ch = &characters[cn];
            let mut buf: [u8; 13] = [0; 13];
            buf[0] = core::constants::SV_SETCHAR_GOLD;
            buf[1..5].copy_from_slice(&ch.gold.to_le_bytes());
            buf[5..7].copy_from_slice(&(ch.armor).to_le_bytes());
            buf[7..9].copy_from_slice(&(ch.weapon).to_le_bytes());
            NetworkManager::with(|network| network.xsend(nr, &buf, 13));
            Server::with_players_mut(|players| {
                players[nr].cpl.gold = ch.gold;
                players[nr].cpl.armor = ch.armor as i32;
                players[nr].cpl.weapon = ch.weapon as i32;
            });
        });
    }
}

/// Send HP change to player
fn plr_change_hp(nr: usize, cn: usize) {
    let (current_hp, player_hp) = Repository::with_characters(|ch| {
        Server::with_players(|players| {
            let a_hp = (ch[cn].a_hp + 500) / 1000;
            (a_hp, players[nr].cpl.a_hp)
        })
    });

    if current_hp != player_hp {
        let mut buf: [u8; 16] = [0; 16];
        buf[0] = core::constants::SV_SETCHAR_AHP;
        buf[1] = current_hp as u8;
        buf[2] = (current_hp >> 8) as u8;

        NetworkManager::with(|network| {
            network.xsend(nr, &buf, 3);
        });

        Server::with_players_mut(|players| {
            players[nr].cpl.a_hp = current_hp;
        });
    }
}

/// Send endurance change to player
fn plr_change_end(nr: usize, cn: usize) {
    let (current_end, player_end) = Repository::with_characters(|ch| {
        Server::with_players(|players| {
            let a_end = (ch[cn].a_end + 500) / 1000;
            (a_end, players[nr].cpl.a_end)
        })
    });

    if current_end != player_end {
        let mut buf: [u8; 16] = [0; 16];
        buf[0] = core::constants::SV_SETCHAR_ENDUR;
        buf[1] = current_end as u8;
        buf[2] = (current_end >> 8) as u8;

        NetworkManager::with(|network| {
            network.xsend(nr, &buf, 3);
        });

        Server::with_players_mut(|players| {
            players[nr].cpl.a_end = current_end;
        });
    }
}

/// Send mana change to player
fn plr_change_mana(nr: usize, cn: usize) {
    let (current_mana, player_mana) = Repository::with_characters(|ch| {
        Server::with_players(|players| {
            let a_mana = (ch[cn].a_mana + 500) / 1000;
            (a_mana, players[nr].cpl.a_mana)
        })
    });

    if current_mana != player_mana {
        let mut buf: [u8; 16] = [0; 16];
        buf[0] = core::constants::SV_SETCHAR_MANA;
        buf[1] = current_mana as u8;
        buf[2] = (current_mana >> 8) as u8;

        NetworkManager::with(|network| {
            network.xsend(nr, &buf, 3);
        });

        Server::with_players_mut(|players| {
            players[nr].cpl.a_mana = current_mana;
        });
    }
}

/// Send direction change to player
fn plr_change_dir(nr: usize, cn: usize) {
    let (current_dir, player_dir) = Repository::with_characters(|ch| {
        Server::with_players(|players| (ch[cn].dir, players[nr].cpl.dir))
    });

    if current_dir as i32 != player_dir {
        let mut buf: [u8; 16] = [0; 16];
        buf[0] = core::constants::SV_SETCHAR_MODE;
        buf[1] = current_dir;

        NetworkManager::with(|network| {
            network.xsend(nr, &buf, 2);
        });

        Server::with_players_mut(|players| players[nr].cpl.dir = current_dir as i32);
    }
}

/// Send points/kindred change to player
fn plr_change_points(nr: usize, cn: usize) {
    let (points, points_tot, kindred, cpl_points, cpl_points_tot, cpl_kindred) =
        Repository::with_characters(|ch| {
            Server::with_players(|players| {
                (
                    ch[cn].points,
                    ch[cn].points_tot,
                    ch[cn].kindred,
                    players[nr].cpl.points,
                    players[nr].cpl.points_tot,
                    players[nr].cpl.kindred,
                )
            })
        });

    if points != cpl_points || points_tot != cpl_points_tot || kindred != cpl_kindred {
        let mut buf: [u8; 10] = [0; 10];
        buf[0] = core::constants::SV_SETCHAR_PTS;

        // points (4 bytes)
        buf[1] = points as u8;
        buf[2] = (points >> 8) as u8;
        buf[3] = (points >> 16) as u8;
        buf[4] = (points >> 24) as u8;

        // points_tot (4 bytes)
        buf[5] = points_tot as u8;
        buf[6] = (points_tot >> 8) as u8;
        buf[7] = (points_tot >> 16) as u8;
        buf[8] = (points_tot >> 24) as u8;

        // kindred (1 byte)
        buf[9] = kindred as u8;

        NetworkManager::with(|network| {
            network.xsend(nr, &buf, 10);
        });

        Server::with_players_mut(|players| {
            players[nr].cpl.points = points;
            players[nr].cpl.points_tot = points_tot;
            players[nr].cpl.kindred = kindred;
        });
    }
}

/// Send gold/armor/weapon change to player
fn plr_change_gold(nr: usize, cn: usize) {
    let (gold, armor, weapon, cpl_gold, cpl_armor, cpl_weapon) =
        Repository::with_characters(|ch| {
            Server::with_players(|players| {
                (
                    ch[cn].gold,
                    ch[cn].armor,
                    ch[cn].weapon,
                    players[nr].cpl.gold,
                    players[nr].cpl.armor,
                    players[nr].cpl.weapon,
                )
            })
        });

    if gold != cpl_gold || armor as i32 != cpl_armor || weapon as i32 != cpl_weapon {
        let mut buf: [u8; 16] = [0; 16];
        buf[0] = core::constants::SV_SETCHAR_GOLD;

        // gold (4 bytes)
        buf[1] = gold as u8;
        buf[2] = (gold >> 8) as u8;
        buf[3] = (gold >> 16) as u8;
        buf[4] = (gold >> 24) as u8;

        // armor (2 bytes)
        buf[5] = armor as u8;
        buf[6] = (armor >> 8) as u8;

        // weapon (2 bytes)
        buf[7] = weapon as u8;
        buf[8] = (weapon >> 8) as u8;

        NetworkManager::with(|network| {
            network.xsend(nr, &buf, 9);
        });

        Server::with_players_mut(|players| {
            players[nr].cpl.gold = gold;
            players[nr].cpl.armor = armor as i32;
            players[nr].cpl.weapon = weapon as i32;
        });
    }
}

/// Send position change to player
fn plr_change_position(nr: usize, cn: usize) {
    let (x, y, cpl_x, cpl_y) = Repository::with_characters(|ch| {
        Server::with_players(|players| (ch[cn].x, ch[cn].y, players[nr].cpl.x, players[nr].cpl.y))
    });

    if x as i32 != cpl_x || y as i32 != cpl_y {
        let mut buf: [u8; 16] = [0; 16];
        buf[0] = core::constants::SV_SETORIGIN;

        // x (2 bytes)
        buf[1] = x as u8;
        buf[2] = (x >> 8) as u8;

        // y (2 bytes)
        buf[3] = y as u8;
        buf[4] = (y >> 8) as u8;

        NetworkManager::with(|network| {
            network.xsend(nr, &buf, 5);
        });

        Server::with_players_mut(|players| {
            players[nr].cpl.x = x as i32;
            players[nr].cpl.y = y as i32;
        });
    }
}

/// Send target change to player
fn plr_change_target(nr: usize, cn: usize) {
    let (attack_cn, goto_x, goto_y, misc_action, misc_target1, misc_target2) =
        Repository::with_characters(|ch| {
            (
                ch[cn].attack_cn,
                ch[cn].goto_x,
                ch[cn].goto_y,
                ch[cn].misc_action,
                ch[cn].misc_target1,
                ch[cn].misc_target2,
            )
        });

    let (
        cpl_attack_cn,
        cpl_goto_x,
        cpl_goto_y,
        cpl_misc_action,
        cpl_misc_target1,
        cpl_misc_target2,
    ) = Server::with_players(|players| {
        (
            players[nr].cpl.attack_cn,
            players[nr].cpl.goto_x,
            players[nr].cpl.goto_y,
            players[nr].cpl.misc_action,
            players[nr].cpl.misc_target1,
            players[nr].cpl.misc_target2,
        )
    });

    if attack_cn as i32 != cpl_attack_cn
        || goto_x as i32 != cpl_goto_x
        || goto_y as i32 != cpl_goto_y
        || misc_action as i32 != cpl_misc_action
        || misc_target1 as i32 != cpl_misc_target1
        || misc_target2 as i32 != cpl_misc_target2
    {
        let mut buf: [u8; 16] = [0; 16];
        buf[0] = core::constants::SV_SETTARGET;

        // attack_cn (2 bytes)
        buf[1] = attack_cn as u8;
        buf[2] = (attack_cn >> 8) as u8;

        // goto_x (2 bytes)
        buf[3] = goto_x as u8;
        buf[4] = (goto_x >> 8) as u8;

        // goto_y (2 bytes)
        buf[5] = goto_y as u8;
        buf[6] = (goto_y >> 8) as u8;

        // misc_action (2 bytes)
        buf[7] = misc_action as u8;
        buf[8] = (misc_action >> 8) as u8;

        // misc_target1 (2 bytes)
        buf[9] = misc_target1 as u8;
        buf[10] = (misc_target1 >> 8) as u8;

        // misc_target2 (2 bytes)
        buf[11] = misc_target2 as u8;
        buf[12] = (misc_target2 >> 8) as u8;

        NetworkManager::with(|network| {
            network.xsend(nr, &buf, 13);
        });

        Server::with_players_mut(|players| {
            players[nr].cpl.attack_cn = attack_cn as i32;
            players[nr].cpl.goto_x = goto_x as i32;
            players[nr].cpl.goto_y = goto_y as i32;
            players[nr].cpl.misc_action = misc_action as i32;
            players[nr].cpl.misc_target1 = misc_target1 as i32;
            players[nr].cpl.misc_target2 = misc_target2 as i32;
        });
    }
}

/// Port of `plr_tick` from `svr_tick.cpp`
/// Handles player tick processing (lag detection and stoning)
pub fn plr_tick(nr: usize) {
    // Increment local tick counter
    Server::with_players_mut(|players| {
        players[nr].ltick = players[nr].ltick.wrapping_add(1);
    });

    let (state, cn) = Server::with_players(|players| (players[nr].state, players[nr].usnr));

    if state != core::constants::ST_NORMAL {
        return;
    }

    if cn == 0 {
        return;
    }

    // Check lag-based stoning conditions
    let (data_19, flags) = Repository::with_characters(|ch| (ch[cn].data[19], ch[cn].flags));

    let is_player = (flags & enums::CharacterFlags::Player.bits()) != 0;
    let is_stoned = (flags & enums::CharacterFlags::Stoned.bits()) != 0;

    if data_19 == 0 || !is_player {
        return;
    }

    let (ltick, rtick) = Server::with_players(|players| (players[nr].ltick, players[nr].rtick));

    // Check if player should be stoned due to lag
    if ltick > rtick.wrapping_add(data_19 as u32) && !is_stoned {
        Repository::with_characters_mut(|ch| {
            log::info!(
                "Character '{}' turned to stone due to lag ({:.2}s)",
                ch[cn].get_name(),
                (ltick.wrapping_sub(rtick)) as f64 / 18.0
            );
            ch[cn].flags |= enums::CharacterFlags::Stoned.bits();
        });
        stone_gc(cn, true);
    }
    // Check if player should be unstoned (lag gone)
    else if ltick
        < rtick
            .wrapping_add(data_19 as u32)
            .wrapping_sub(core::constants::TICKS as u32)
        && is_stoned
    {
        Repository::with_characters_mut(|ch| {
            log::info!("Character '{}' unstoned, lag is gone", ch[cn].get_name());
            ch[cn].flags &= !enums::CharacterFlags::Stoned.bits();
        });
        stone_gc(cn, false);
    }
}

/// Port of `stone_gc` from `svr_tick.cpp`
/// Handles stoning/unstoning of linked characters (e.g., usurped characters)
fn stone_gc(cn: usize, mode: bool) {
    let (is_player, co) = Repository::with_characters(|ch| {
        let is_player = (ch[cn].flags & enums::CharacterFlags::Player.bits()) != 0;
        let co = ch[cn].data[64] as usize;
        (is_player, co)
    });

    if !is_player {
        return;
    }

    if co == 0 {
        return;
    }

    // Check if co is a valid active character
    let is_valid = Repository::with_characters(|ch| {
        co < core::constants::MAXCHARS
            && ch[co].used == core::constants::USE_ACTIVE
            && ch[co].data[63] == cn as i32
    });

    if !is_valid {
        return;
    }

    Repository::with_characters_mut(|ch| {
        if mode {
            ch[co].flags |= enums::CharacterFlags::Stoned.bits();
        } else {
            ch[co].flags &= !enums::CharacterFlags::Stoned.bits();
        }
    });
}

/// Port of `plr_idle` from `svr_tick.cpp`
/// Handles idle timeout checking for players
pub fn plr_idle(nr: usize) {
    let (ticker, lasttick, lasttick2, state, usnr) = Repository::with_globals(|globals| {
        Server::with_players(|players| {
            (
                globals.ticker as u32,
                players[nr].lasttick,
                players[nr].lasttick2,
                players[nr].state,
                players[nr].usnr,
            )
        })
    });

    // Check protocol level idle (60 seconds)
    if ticker.wrapping_sub(lasttick) > (core::constants::TICKS * 60) as u32 {
        log::info!("Player {} idle too long (protocol level)", nr);
        plr_logout(usnr, nr, enums::LogoutReason::IdleTooLong);
    }

    if state == core::constants::ST_EXIT {
        return;
    }

    // Check player level idle (15 minutes)
    if ticker.wrapping_sub(lasttick2) > (core::constants::TICKS * 60 * 15) as u32 {
        log::info!("Player {} idle too long (player level)", nr);
        plr_logout(usnr, nr, enums::LogoutReason::IdleTooLong);
    }
}

/// Port of `plr_cmd` from `svr_tick.cpp`
/// Dispatches player commands from inbuf
pub fn plr_cmd(nr: usize) {
    let (cmd, state) = Server::with_players(|players| (players[nr].inbuf[0], players[nr].state));

    // Handle pre-login commands
    match cmd {
        core::constants::CL_NEWLOGIN => {
            plr_challenge_newlogin(nr);
            return;
        }
        core::constants::CL_CHALLENGE => {
            plr_challenge(nr);
            return;
        }
        core::constants::CL_LOGIN => {
            plr_challenge_login(nr);
            return;
        }
        core::constants::CL_CMD_UNIQUE => {
            plr_unique(nr);
            return;
        }
        core::constants::CL_PASSWD => {
            plr_passwd(nr);
            return;
        }
        _ => {}
    }

    // Only process other commands if in normal state
    if state != core::constants::ST_NORMAL {
        return;
    }

    // Update lasttick2 for non-automated commands
    if cmd != core::constants::CL_CMD_AUTOLOOK
        && cmd != core::constants::CL_PERF_REPORT
        && cmd != core::constants::CL_CMD_CTICK
    {
        let ticker = Repository::with_globals(|globals| globals.ticker as u32);
        Server::with_players_mut(|players| {
            players[nr].lasttick2 = ticker;
        });
    }

    // Handle commands that don't require stun check
    match cmd {
        core::constants::CL_PERF_REPORT => {
            plr_perf_report(nr);
            return;
        }
        core::constants::CL_CMD_LOOK => {
            plr_cmd_look(nr, false);
            return;
        }
        core::constants::CL_CMD_AUTOLOOK => {
            plr_cmd_look(nr, true);
            return;
        }
        core::constants::CL_CMD_SETUSER => {
            plr_cmd_setuser(nr);
            return;
        }
        core::constants::CL_CMD_STAT => {
            plr_cmd_stat(nr);
            return;
        }
        core::constants::CL_CMD_INPUT1 => {
            plr_cmd_input(nr, 1);
            return;
        }
        core::constants::CL_CMD_INPUT2 => {
            plr_cmd_input(nr, 2);
            return;
        }
        core::constants::CL_CMD_INPUT3 => {
            plr_cmd_input(nr, 3);
            return;
        }
        core::constants::CL_CMD_INPUT4 => {
            plr_cmd_input(nr, 4);
            return;
        }
        core::constants::CL_CMD_INPUT5 => {
            plr_cmd_input(nr, 5);
            return;
        }
        core::constants::CL_CMD_INPUT6 => {
            plr_cmd_input(nr, 6);
            return;
        }
        core::constants::CL_CMD_INPUT7 => {
            plr_cmd_input(nr, 7);
            return;
        }
        core::constants::CL_CMD_INPUT8 => {
            plr_cmd_input(nr, 8);
            return;
        }
        core::constants::CL_CMD_CTICK => {
            plr_cmd_ctick(nr);
            return;
        }
        _ => {}
    }

    let cn = Server::with_players(|players| players[nr].usnr);
    let is_stunned = Repository::with_characters(|ch| ch[cn].stunned > 0);

    if is_stunned {
        State::with(|state| {
            state.do_character_log(
                cn,
                core::types::FontColor::Red,
                "You have been stunned. You cannot move.\n",
            );
        });
    }

    // Handle commands that show stun message but still execute
    match cmd {
        core::constants::CL_CMD_LOOK_ITEM => {
            plr_cmd_look_item(nr);
            return;
        }
        core::constants::CL_CMD_GIVE => {
            plr_cmd_give(nr);
            return;
        }
        core::constants::CL_CMD_TURN => {
            plr_cmd_turn(nr);
            return;
        }
        core::constants::CL_CMD_DROP => {
            plr_cmd_drop(nr);
            return;
        }
        core::constants::CL_CMD_PICKUP => {
            plr_cmd_pickup(nr);
            return;
        }
        core::constants::CL_CMD_ATTACK => {
            plr_cmd_attack(nr);
            return;
        }
        core::constants::CL_CMD_MODE => {
            plr_cmd_mode(nr);
            return;
        }
        core::constants::CL_CMD_MOVE => {
            plr_cmd_move(nr);
            return;
        }
        core::constants::CL_CMD_RESET => {
            plr_cmd_reset(nr);
            return;
        }
        core::constants::CL_CMD_SKILL => {
            plr_cmd_skill(nr);
            return;
        }
        core::constants::CL_CMD_INV_LOOK => {
            plr_cmd_inv_look(nr);
            return;
        }
        core::constants::CL_CMD_USE => {
            plr_cmd_use(nr);
            return;
        }
        core::constants::CL_CMD_INV => {
            plr_cmd_inv(nr);
            return;
        }
        core::constants::CL_CMD_EXIT => {
            plr_cmd_exit(nr);
            return;
        }
        _ => {}
    }

    // Commands blocked by stun
    if is_stunned {
        return;
    }

    match cmd {
        core::constants::CL_CMD_SHOP => {
            plr_cmd_shop(nr);
        }
        _ => {
            log::warn!("Unknown CL command: {} for player {}", cmd, nr);
        }
    }
}

// ============================================================================
// Command handler stubs - to be implemented
// ============================================================================

/// Secret key for xcrypt function (from svr_tick.cpp)
const SECRET: &[u8] = b"Ifhjf64hH8sa,-#39ddj843tvxcv0434dvsdc40G#34Trefc349534Y5#34trecerr943\
5#erZt#eA534#5erFtw#Trwec,9345mwrxm gerte-534lMIZDN(/dn8sfn8&DBDB/D&s\
8efnsd897)DDzD'D'D''Dofs,t0943-rg-gdfg-gdf.t,e95.34u.5retfrh.wretv.56\
9v4#asf.59m(D)/ND/DDLD;gd+dsa,fw9r,x  OD(98snfsf";

/// Port of `xcrypt` from `svr_tick.cpp`
/// Encryption function for challenge verification
fn xcrypt(val: u32) -> u32 {
    let mut res: u32 = 0;

    res = res.wrapping_add(SECRET[(val & 255) as usize] as u32);
    res = res.wrapping_add((SECRET[((val >> 8) & 255) as usize] as u32) << 8);
    res = res.wrapping_add((SECRET[((val >> 16) & 255) as usize] as u32) << 16);
    res = res.wrapping_add((SECRET[((val >> 24) & 255) as usize] as u32) << 24);

    res ^= 0x5a7ce52e;

    res
}

/// Port of `send_mod` from `svr_tick.cpp`
/// Sends mod data to the client (8 packets of 15 bytes each)
fn send_mod(nr: usize) {
    // TODO: Implement mod sending when mod data is available
    // For now, this is a stub - mod data would be loaded from somewhere
    // In the original code, this sends 8 SV_MOD packets with mod data
    let _mod_data: [u8; 120] = [0; 120]; // placeholder

    for n in 0..8u8 {
        let mut buf: [u8; 16] = [0; 16];
        buf[0] = core::constants::SV_MOD1 + n;
        // Copy 15 bytes of mod data (placeholder zeros for now)
        // buf[1..16].copy_from_slice(&mod_data[(n as usize * 15)..((n as usize + 1) * 15)]);

        NetworkManager::with(|network| {
            network.csend(nr, &buf, 16);
        });
    }
}

/// Port of `plr_challenge_newlogin` from `svr_tick.cpp`
///
/// Initiates a new-login challenge for a connecting client. Generates a random
/// non-zero challenge, stores it on `players[nr]`, sets the player's state to
/// `ST_NEW_CHALLENGE`, timestamps `lasttick`, sends the `SV_CHALLENGE` packet
/// to the client, and sends mod data packets.
///
/// # Arguments
/// * `nr` - Player slot index to challenge
fn plr_challenge_newlogin(nr: usize) {
    use rand::Rng;

    // Generate random challenge value (0x3fffffff max, ensure non-zero)
    let mut tmp = rand::thread_rng().gen_range(1..0x3fffffff_u32);
    if tmp == 0 {
        tmp = 42;
    }

    let ticker = Repository::with_globals(|globals| globals.ticker as u32);

    Server::with_players_mut(|players| {
        players[nr].challenge = tmp;
        players[nr].state = core::constants::ST_NEW_CHALLENGE;
        players[nr].lasttick = ticker;
    });

    // Send challenge to client
    let mut buf: [u8; 16] = [0; 16];
    buf[0] = core::constants::SV_CHALLENGE;
    buf[1..5].copy_from_slice(&tmp.to_le_bytes());

    NetworkManager::with(|network| {
        network.csend(nr, &buf, 16);
    });

    log::debug!(
        "Player {} challenge_newlogin: sent challenge {:08X}",
        nr,
        tmp
    );

    send_mod(nr);
}

/// Port of `plr_challenge` from `svr_tick.cpp`
///
/// Verifies the client's response to a previously issued challenge. Reads the
/// response, client version, and race from the inbuf, stores version/race on
/// the player record, validates the response using `xcrypt`, and moves the
/// player through the login state machine on success (or logs them out on
/// failure).
///
/// # Arguments
/// * `nr` - Player slot index handling the challenge response
fn plr_challenge(nr: usize) {
    let (challenge, state) =
        Server::with_players(|players| (players[nr].challenge, players[nr].state));

    // Read challenge response, version, and race from inbuf
    let (response, version, race) = Server::with_players(|players| {
        let response = u32::from_le_bytes([
            players[nr].inbuf[1],
            players[nr].inbuf[2],
            players[nr].inbuf[3],
            players[nr].inbuf[4],
        ]);
        let version = i32::from_le_bytes([
            players[nr].inbuf[5],
            players[nr].inbuf[6],
            players[nr].inbuf[7],
            players[nr].inbuf[8],
        ]);
        let race = i32::from_le_bytes([
            players[nr].inbuf[9],
            players[nr].inbuf[10],
            players[nr].inbuf[11],
            players[nr].inbuf[12],
        ]);
        (response, version, race)
    });

    // Store version and race
    Server::with_players_mut(|players| {
        players[nr].version = version;
        players[nr].race = race;
    });

    log::info!(
        "Player {} challenge: response={:08X}, version={}, race={}",
        nr,
        response,
        version,
        race
    );

    // Verify the challenge response
    if response != xcrypt(challenge) {
        log::warn!("Player {} challenge failed", nr);
        let usnr = Server::with_players(|players| players[nr].usnr);
        plr_logout(usnr, nr, enums::LogoutReason::ChallengeFailed);
        return;
    }

    let ticker = Repository::with_globals(|globals| globals.ticker as u32);

    // Update state based on current state
    match state {
        state if state == core::constants::ST_NEW_CHALLENGE => {
            Server::with_players_mut(|players| {
                players[nr].state = core::constants::ST_NEWLOGIN;
                players[nr].lasttick = ticker;
                log::info!("Player {} login challenge passed for new characters", nr);
            });
        }
        state if state == core::constants::ST_LOGIN_CHALLENGE => {
            Server::with_players_mut(|players| {
                players[nr].state = core::constants::ST_LOGIN;
                players[nr].lasttick = ticker;
            });
            log::info!("Player {} login challenge passed", nr);
        }
        state if state == core::constants::ST_CHALLENGE => {
            Server::with_players_mut(|players| {
                players[nr].state = core::constants::ST_NORMAL;
                players[nr].lasttick = ticker;
                players[nr].ltick = 0;
            });
            log::info!("Player {} logged in successfully", nr);
        }
        _ => {
            log::warn!(
                "Player {} challenge reply at unexpected state {}",
                nr,
                state
            );
        }
    }

    log::debug!("Player {} challenge ok", nr);
}

/// Handle existing login challenge (port of `plr_challenge_login`)
///
/// Generates a random non-zero challenge, sets the player into the
/// `ST_LOGIN_CHALLENGE` state, validates the requested character index
/// supplied by the client, stores `pass1`/`pass2` fragments and sends the
/// challenge (and mod packets) back to the client.
fn plr_challenge_login(nr: usize) {
    use rand::Rng;

    log::debug!("Player {} challenge_login", nr);

    // Generate random challenge value (0x3fffffff max, ensure non-zero)
    let mut tmp = rand::thread_rng().gen_range(1..0x3fffffff_u32);
    if tmp == 0 {
        tmp = 42;
    }

    let ticker = Repository::with_globals(|globals| globals.ticker as u32);

    Server::with_players_mut(|players| {
        players[nr].challenge = tmp;
        players[nr].state = core::constants::ST_LOGIN_CHALLENGE;
        players[nr].lasttick = ticker;
    });

    // Send challenge to client
    let mut buf: [u8; 16] = [0; 16];
    buf[0] = core::constants::SV_CHALLENGE;
    buf[1..5].copy_from_slice(&tmp.to_le_bytes());

    NetworkManager::with(|network| {
        network.csend(nr, &buf, 16);
    });

    log::debug!("Player {} challenge_login: sent challenge {:08X}", nr, tmp);

    // Read desired character number and pass fragments from client's inbuf
    let cn = Server::with_players(|players| {
        u32::from_le_bytes([
            players[nr].inbuf[1],
            players[nr].inbuf[2],
            players[nr].inbuf[3],
            players[nr].inbuf[4],
        ]) as usize
    });

    if cn < 1 || cn >= core::constants::MAXCHARS as usize {
        log::warn!("Player {} sent wrong cn {} in challenge login", nr, cn);
        plr_logout(0, nr, enums::LogoutReason::ChallengeFailed);
        return;
    }

    // Store chosen character and pass fragments
    let (pass1, pass2) = Server::with_players(|players| {
        (
            u32::from_le_bytes([
                players[nr].inbuf[5],
                players[nr].inbuf[6],
                players[nr].inbuf[7],
                players[nr].inbuf[8],
            ]),
            u32::from_le_bytes([
                players[nr].inbuf[9],
                players[nr].inbuf[10],
                players[nr].inbuf[11],
                players[nr].inbuf[12],
            ]),
        )
    });

    Server::with_players_mut(|players| {
        players[nr].usnr = cn;
        players[nr].pass1 = pass1;
        players[nr].pass2 = pass2;
    });

    log::info!(
        "Player logged in as character index={} (players index={})",
        cn,
        nr
    );

    send_mod(nr);
}

/// Port of `plr_unique` from `svr_tick.cpp`
///
/// Receives the client's unique 8-byte identifier or generates a server-side
/// unique if the client provided none. The server stores the value in
/// `players[nr].unique` and echoes back a generated unique when applicable.
///
/// # Arguments
/// * `nr` - Player slot index sending the unique
fn plr_unique(nr: usize) {
    // Read unique ID from inbuf (8 bytes as u64)
    let unique = Server::with_players(|players| {
        u64::from_le_bytes([
            players[nr].inbuf[1],
            players[nr].inbuf[2],
            players[nr].inbuf[3],
            players[nr].inbuf[4],
            players[nr].inbuf[5],
            players[nr].inbuf[6],
            players[nr].inbuf[7],
            players[nr].inbuf[8],
        ])
    });

    Server::with_players_mut(|players| {
        players[nr].unique = unique;
    });

    log::debug!("Player {} received unique {:016X}", nr, unique);

    // If client doesn't have a unique ID, generate one
    if unique == 0 {
        let new_unique = Repository::with_globals_mut(|globals| {
            globals.unique = globals.unique.wrapping_add(1);
            globals.unique
        });

        Server::with_players_mut(|players| {
            players[nr].unique = new_unique;
        });

        // Send the new unique ID back to the client
        let mut buf: [u8; 16] = [0; 16];
        buf[0] = core::constants::SV_UNIQUE;
        buf[1..9].copy_from_slice(&new_unique.to_le_bytes());

        NetworkManager::with(|network| {
            network.xsend(nr, &buf, 9);
        });

        log::debug!("Player {} sent unique {:016X}", nr, new_unique);
    }
}

/// Port of `plr_passwd` from `svr_tick.cpp`
///
/// Receives a password fragment from the client and stores it in the
/// player's `passwd` buffer (15 bytes). Computes a lightweight hash for
/// debug/logging parity with original server behavior.
///
/// # Arguments
/// * `nr` - Player slot index sending the password fragment
fn plr_passwd(nr: usize) {
    // Copy 15 bytes of password from inbuf to player passwd
    Server::with_players_mut(|players| {
        players[nr].passwd[..15].copy_from_slice(&players[nr].inbuf[1..16]);
        players[nr].passwd[15] = 0; // null terminate
    });

    // Calculate hash for logging (same algorithm as original)
    let hash = Server::with_players(|players| {
        let mut hash: u32 = 0;
        for n in 0..15 {
            if players[nr].passwd[n] == 0 {
                break;
            }
            hash ^= (players[nr].passwd[n] as u32) << (n * 2);
        }
        hash
    });

    log::debug!("Player {} received passwd hash {}", nr, hash);
}

/// Port of `plr_perf_report` from `svr_tick.cpp`
///
/// Parses a client's performance/timing report and uses it to refresh the
/// player's network timeout (`lasttick`). The metric values are parsed for
/// completeness but currently not acted upon.
///
/// # Arguments
/// * `nr` - Player slot index reporting performance
fn plr_perf_report(nr: usize) {
    // Read performance metrics from inbuf (unused but parsed for completeness)
    let (_ticksize, _skip, _idle) = Server::with_players(|players| {
        let ticksize = u16::from_le_bytes([players[nr].inbuf[1], players[nr].inbuf[2]]);
        let skip = u16::from_le_bytes([players[nr].inbuf[3], players[nr].inbuf[4]]);
        let idle = u16::from_le_bytes([players[nr].inbuf[5], players[nr].inbuf[6]]);
        (ticksize, skip, idle)
    });

    // Update timeout - this is the important part
    let ticker = Repository::with_globals(|globals| globals.ticker as u32);
    Server::with_players_mut(|players| {
        players[nr].lasttick = ticker;
    });

    // Optional: log performance metrics (commented out in original)
    // log::trace!("Player {} perf: ticksize={}, skip={}%, idle={}%", nr, ticksize, skip, idle);
}

/// Port of `plr_cmd_look` from `svr_tick.cpp`
///
/// Handles the client's LOOK command. If the high bit of the supplied id
/// (`co`) is set, the player requested to see a depot slot (bank); otherwise
/// it requests a character/NPC look. Delegates to `do_look_depot` or
/// `do_look_char` in the server `State`.
///
/// # Arguments
/// * `nr` - Player slot index issuing the look
/// * `autoflag` - When true, treat the request as an automatic look
fn plr_cmd_look(nr: usize, autoflag: bool) {
    let (cn, co) = Server::with_players(|players| {
        let co = u16::from_le_bytes([players[nr].inbuf[1], players[nr].inbuf[2]]) as usize;
        (players[nr].usnr, co)
    });

    // Check if looking at depot (high bit set) or character
    if (co & 0x8000) != 0 {
        // Looking at depot slot
        let depot_slot = co & 0x7fff;
        State::with(|state| {
            state.do_look_depot(cn, depot_slot);
        });
    } else {
        // Looking at character
        let autoflag_int = if autoflag { 1 } else { 0 };
        State::with_mut(|state| {
            state.do_look_char(cn, co, 0, autoflag_int, 0);
        });
    }
}

/// Handle set user data command
///
/// Receives chunks of account/profile data from the client (13-byte
/// fragments) and writes them into the character's `text` buffers. When the
/// final chunk is received for the description/name update it performs
/// validation (name legality, uniqueness, description rules) and either
/// commits changes or reports why they were rejected.
///
/// # Arguments
/// * `_nr` - Player slot index sending the data
fn plr_cmd_setuser(_nr: usize) {
    // Implementation based on original svr_tick.cpp
    // Read subtype, position and 13 bytes of data from player's inbuf
    let (nr, subtype, pos, chunk): (usize, u8, usize, [u8; 13]) = Server::with_players(|players| {
        let nr = _nr;
        let subtype = players[nr].inbuf[1];
        let pos = players[nr].inbuf[2] as usize;
        let mut chunk = [0u8; 13];
        for i in 0..13 {
            chunk[i] = players[nr].inbuf[3 + i];
        }
        (nr, subtype, pos, chunk)
    });

    if pos > 65 {
        return;
    }

    // Get character index for this player
    let cn = Server::with_players(|players| players[nr].usnr);

    match subtype {
        0 | 1 => {
            // write 13 bytes into text[0] or text[1]
            let text_idx = if subtype == 0 { 0 } else { 1 };
            Repository::with_characters_mut(|ch| {
                for i in 0..13 {
                    ch[cn].text[text_idx][pos + i] = chunk[i];
                }
            });
        }
        2 => {
            // write into text[2]
            Repository::with_characters_mut(|ch| {
                for i in 0..13 {
                    ch[cn].text[2][pos + i] = chunk[i];
                }
            });

            // If this was the final chunk (pos == 65) perform validation and possibly
            // commit name/reference/description changes.
            if pos == 65 {
                // Work inside a mutable characters closure to inspect & modify
                Repository::with_characters_mut(|ch| {
                    // Name handling: examine text[0]
                    let name_bytes = &mut ch[cn].text[0];
                    let name_end = name_bytes
                        .iter()
                        .position(|&c| c == 0)
                        .unwrap_or(name_bytes.len());
                    let mut flag: i32 = 0;

                    // Check length constraints
                    if name_end > 3
                        && name_end < 38
                        && (ch[cn].flags
                            & core::constants::CharacterFlags::CF_NEWUSER.bits() as u64)
                            != 0
                    {
                        // validate letters only and lowercase
                        for n in 0..name_end {
                            let b = name_bytes[n];
                            if !((b'A'..=b'Z').contains(&b) || (b'a'..=b'z').contains(&b)) {
                                flag = 1;
                                break;
                            }
                            name_bytes[n] = name_bytes[n].to_ascii_lowercase();
                        }

                        if flag == 0 {
                            // uppercase first letter
                            if name_end > 0 {
                                name_bytes[0] = name_bytes[0].to_ascii_uppercase();
                            }

                            // check reserved name "Self"
                            let name_str = {
                                let end = name_bytes
                                    .iter()
                                    .position(|&c| c == 0)
                                    .unwrap_or(name_bytes.len());
                                String::from_utf8_lossy(&name_bytes[..end]).to_string()
                            };

                            if name_str == "Self" {
                                flag = 2;
                            }

                            // check for duplicate names
                            if flag == 0 {
                                for n in 1..core::constants::MAXCHARS {
                                    if n != cn && ch[n].used != core::constants::USE_EMPTY as u8 {
                                        let other_name_end = ch[n]
                                            .name
                                            .iter()
                                            .position(|&c| c == 0)
                                            .unwrap_or(ch[n].name.len());
                                        let other_name =
                                            String::from_utf8_lossy(&ch[n].name[..other_name_end])
                                                .to_string();
                                        if other_name == name_str {
                                            flag = 2;
                                            break;
                                        }
                                    }
                                }
                            }

                            // TODO: badname check unavailable in Rust port; skip CF_NODESC check here
                        }
                    }

                    // If flag set -> report and don't commit name change
                    if flag != 0 {
                        let name_str = {
                            let end = ch[cn].text[0]
                                .iter()
                                .position(|&c| c == 0)
                                .unwrap_or(ch[cn].text[0].len());
                            String::from_utf8_lossy(&ch[cn].text[0][..end]).to_string()
                        };
                        let reason = if flag == 1 {
                            "contains non-letters. Please choose a more normal-looking name."
                                .to_string()
                        } else if flag == 2 {
                            "is already in use. Please try to choose another name.".to_string()
                        } else {
                            "is deemed inappropriate. Please try to choose another name."
                                .to_string()
                        };

                        State::with(|state| {
                            state.do_character_log(
                                cn,
                                core::types::FontColor::Green,
                                &format!(
                                    "The name \"{}\" you have chosen for your character {}\n",
                                    name_str, reason
                                ),
                            );
                        });
                    } else {
                        // Commit name -> copy into name and reference (40 bytes)
                        let name_end = ch[cn].text[0].iter().position(|&c| c == 0).unwrap_or(40);
                        for i in 0..40 {
                            ch[cn].name[i] = if i < name_end { ch[cn].text[0][i] } else { 0 };
                            ch[cn].reference[i] = ch[cn].name[i];
                        }
                        // clear CF_NEWUSER flag
                        ch[cn].flags &=
                            !(core::constants::CharacterFlags::CF_NEWUSER.bits() as u64);
                    }

                    // Description handling: copy text[1] and possibly append text[2]
                    let mut desc = {
                        let end1 = ch[cn].text[1]
                            .iter()
                            .position(|&c| c == 0)
                            .unwrap_or(ch[cn].text[1].len());
                        String::from_utf8_lossy(&ch[cn].text[1][..end1]).to_string()
                    };
                    if desc.len() > 77 {
                        let add = {
                            let end2 = ch[cn].text[2]
                                .iter()
                                .position(|&c| c == 0)
                                .unwrap_or(ch[cn].text[2].len());
                            String::from_utf8_lossy(&ch[cn].text[2][..end2]).to_string()
                        };
                        desc.push_str(&add);
                    }

                    // Validate description
                    let mut reason: Option<String> = None;
                    if desc.len() < 10 {
                        reason = Some("is too short".to_string());
                    } else {
                        // Does description contain name?
                        let name_str = {
                            let end = ch[cn]
                                .name
                                .iter()
                                .position(|&c| c == 0)
                                .unwrap_or(ch[cn].name.len());
                            String::from_utf8_lossy(&ch[cn].name[..end]).to_string()
                        };
                        if !desc.contains(&name_str) {
                            reason = Some("does not contain your name".to_string());
                        } else if desc.contains('"') {
                            reason = Some("contains a double quote".to_string());
                        } else if (ch[cn].flags
                            & core::constants::CharacterFlags::CF_NODESC.bits() as u64)
                            != 0
                        {
                            reason = Some("was blocked because you have been known to enter inappropriate descriptions".to_string());
                        }
                    }

                    if let Some(reason) = reason {
                        // pick race name
                        let race_name = if (ch[cn].kindred & core::constants::KIN_TEMPLAR as i32)
                            != 0
                        {
                            "a Templar"
                        } else if (ch[cn].kindred & core::constants::KIN_HARAKIM as i32) != 0 {
                            "a Harakim"
                        } else if (ch[cn].kindred & core::constants::KIN_MERCENARY as i32) != 0 {
                            "a Mercenary"
                        } else if (ch[cn].kindred & core::constants::KIN_SEYAN_DU as i32) != 0 {
                            "a Seyan'Du"
                        } else if (ch[cn].kindred & core::constants::KIN_ARCHHARAKIM as i32) != 0 {
                            "an Arch Harakim"
                        } else if (ch[cn].kindred & core::constants::KIN_ARCHTEMPLAR as i32) != 0 {
                            "an Arch Templar"
                        } else if (ch[cn].kindred & core::constants::KIN_WARRIOR as i32) != 0 {
                            "a Warrior"
                        } else if (ch[cn].kindred & core::constants::KIN_SORCERER as i32) != 0 {
                            "a Sorcerer"
                        } else {
                            "a strange figure"
                        };

                        State::with(|state| {
                            state.do_character_log(
                                cn,
                                core::types::FontColor::Yellow,
                                &format!("The description you entered for your character {} was rejected.\n", reason),
                            );
                        });

                        // fallback description
                        let name_str = {
                            let end = ch[cn]
                                .name
                                .iter()
                                .position(|&c| c == 0)
                                .unwrap_or(ch[cn].name.len());
                            String::from_utf8_lossy(&ch[cn].name[..end]).to_string()
                        };
                        let pronoun = if (ch[cn].kindred & core::constants::KIN_FEMALE as i32) != 0
                        {
                            "She"
                        } else {
                            "He"
                        };
                        let fallback = format!(
                            "{} is {}. {} looks somewhat nondescript.",
                            name_str, race_name, pronoun
                        );
                        // write fallback into description (200 bytes max)
                        let bytes = fallback.as_bytes();
                        for i in 0..200 {
                            ch[cn].description[i] = if i < bytes.len() { bytes[i] } else { 0 };
                        }
                    } else {
                        // commit description
                        let bytes = desc.as_bytes();
                        for i in 0..200 {
                            ch[cn].description[i] = if i < bytes.len() { bytes[i] } else { 0 };
                        }
                    }
                    // Finally acknowledge and request character update
                    State::with(|state| {
                        state.do_character_log(
                            cn,
                            core::types::FontColor::Yellow,
                            "Account data received.\n",
                        );
                        state.do_update_char(cn);
                    });
                });
            }
        }
        _ => {
            log::warn!("Unknown setuser subtype {}", subtype);
        }
    }
}

/// Handle stat change command
///
/// Applies attribute/HP/endurance/mana/skill raises requested by the
/// client. Validates indices and performs repeated raise operations via
/// `State` helper functions, then requests a character update.
///
/// # Arguments
/// * `_nr` - Player slot index issuing the stat change
fn plr_cmd_stat(_nr: usize) {
    // Read stat index and value from inbuf and apply raises
    let (cn, n, v) = Server::with_players(|players| {
        let cn = players[_nr].usnr;
        let n = u16::from_le_bytes([players[_nr].inbuf[1], players[_nr].inbuf[2]]) as usize;
        let v = u16::from_le_bytes([players[_nr].inbuf[3], players[_nr].inbuf[4]]) as usize;
        (cn, n, v)
    });

    // sanity checks
    if n > 107 || v > 99 {
        return;
    }

    // perform raises
    if n < 5 {
        for _ in 0..v {
            State::with(|state| {
                let _ = state.do_raise_attrib(cn, n as i32);
            });
        }
    } else if n == 5 {
        for _ in 0..v {
            State::with(|state| {
                let _ = state.do_raise_hp(cn);
            });
        }
    } else if n == 6 {
        for _ in 0..v {
            State::with(|state| {
                let _ = state.do_raise_end(cn);
            });
        }
    } else if n == 7 {
        for _ in 0..v {
            State::with(|state| {
                let _ = state.do_raise_mana(cn);
            });
        }
    } else {
        for _ in 0..v {
            State::with(|state| {
                let _ = state.do_raise_skill(cn, (n - 8) as i32);
            });
        }
    }

    // request character update
    State::with(|state| state.do_update_char(cn));
}

/// Handle text input commands (1-8)
///
/// Receives a 15-byte chunk of textual input from the client. When the
/// eighth (final) chunk is received the function NUL-terminates the collected
/// input, decodes it to a UTF-8 string, and forwards it to `do_say` for
/// processing as a chat/message.
///
/// # Arguments
/// * `nr` - Player slot index sending the input
/// * `part` - Which 1..8 chunk this call contains
fn plr_cmd_input(nr: usize, part: u8) {
    // Copy 15 bytes of input from inbuf to player input buffer
    let offset = ((part - 1) as usize) * 15;
    Server::with_players_mut(|players| {
        for n in 0..15 {
            players[nr].input[offset + n] = players[nr].inbuf[1 + n];
        }
    });

    // If this is input8, process the complete message (do_say)
    if part == 8 {
        // Ensure the input buffer is NUL-terminated at the last byte (matches C behaviour)
        Server::with_players_mut(|players| {
            // 8 * 15 == 120, last index is 119
            players[nr].input[105 + 14] = 0;
        });

        // Copy the player's input buffer out so we can convert it to a Rust string
        let (cn, raw) =
            Server::with_players(|players| (players[nr].usnr, players[nr].input.to_vec()));

        // Find the first NUL and decode bytes up to that point as UTF-8 (lossy to be safe)
        let len = raw.iter().position(|&b| b == 0).unwrap_or(raw.len());
        let text = String::from_utf8_lossy(&raw[..len]).to_string();

        // Call the server state handler (port of C++ do_say)
        State::with_mut(|state| {
            state.do_say(cn, &text);
        });
    }
}

/// Handle client tick update
///
/// Updates server-side bookkeeping for client timing. Reads `rtick` from the
/// client's inbuf, stores it in `players[nr].rtick`, and refreshes the
/// player's `lasttick` timeout to avoid idle/disconnect handling.
///
/// # Arguments
/// * `nr` - Player slot index sending the tick
fn plr_cmd_ctick(nr: usize) {
    let ticker = Repository::with_globals(|globals| globals.ticker as u32);
    Server::with_players_mut(|players| {
        // Read rtick from inbuf (4 bytes at offset 1)
        let rtick = u32::from_le_bytes([
            players[nr].inbuf[1],
            players[nr].inbuf[2],
            players[nr].inbuf[3],
            players[nr].inbuf[4],
        ]);
        players[nr].rtick = rtick;
        players[nr].lasttick = ticker;
    });
}

/// Handle look at item on ground
///
/// Reads coordinates from the client's packet, validates them, and if the
/// tile contains an item calls `do_look_item` to present details to the
/// requesting character.
///
/// # Arguments
/// * `_nr` - Player slot index issuing the request
fn plr_cmd_look_item(_nr: usize) {
    // Read x,y from inbuf and call do_look_item
    let (x, y, cn) = Server::with_players(|players| {
        let x = u16::from_le_bytes([players[_nr].inbuf[1], players[_nr].inbuf[2]]) as i32;
        let y = u16::from_le_bytes([players[_nr].inbuf[3], players[_nr].inbuf[4]]) as i32;
        (x, y, players[_nr].usnr)
    });

    if x < 0
        || x >= core::constants::SERVER_MAPX as i32
        || y < 0
        || y >= core::constants::SERVER_MAPY as i32
    {
        return;
    }

    let in_idx = Repository::with_map(|map| {
        map[(x + y * core::constants::SERVER_MAPX as i32) as usize].it as usize
    });

    State::with_mut(|s| s.do_look_item(cn, in_idx));
}

/// Handle give item command
///
/// Reads a target character id from the client's packet and sets the
/// giving character's misc action (`DR_GIVE`) and `misc_target1` to
/// perform a give in the next tick.
///
/// # Arguments
/// * `_nr` - Player slot index issuing the give
fn plr_cmd_give(_nr: usize) {
    // Read target character id (4 bytes) and set give action
    let co = Server::with_players(|players| {
        u32::from_le_bytes([
            players[_nr].inbuf[1],
            players[_nr].inbuf[2],
            players[_nr].inbuf[3],
            players[_nr].inbuf[4],
        ]) as usize
    });

    if co >= core::constants::MAXCHARS as usize {
        return;
    }

    let cn = Server::with_players(|players| players[_nr].usnr);
    let ticker = Repository::with_globals(|g| g.ticker as i32);

    Repository::with_characters_mut(|ch| {
        ch[cn].attack_cn = 0;
        ch[cn].goto_x = 0;
        ch[cn].misc_action = core::constants::DR_GIVE as u16;
        ch[cn].misc_target1 = co as u16;
        ch[cn].cerrno = 0;
        ch[cn].data[12] = ticker;
    });
}

/// Handle turn command
///
/// Reads target coordinates from the client and sets a turn action
/// (`DR_TURN`) so the character will turn toward the specified point on
/// its next action tick. Ignored if the character is in building mode.
///
/// # Arguments
/// * `_nr` - Player slot index issuing the turn
fn plr_cmd_turn(_nr: usize) {
    // Read x,y and set turn action
    let (x, y, cn) = Server::with_players(|players| {
        let x = u16::from_le_bytes([players[_nr].inbuf[1], players[_nr].inbuf[2]]) as i32;
        let y = u16::from_le_bytes([players[_nr].inbuf[3], players[_nr].inbuf[4]]) as i32;
        (x, y, players[_nr].usnr)
    });

    // If building mode, ignore
    let is_building = Repository::with_characters(|ch| ch[cn].is_building());
    if is_building {
        return;
    }

    let ticker = Repository::with_globals(|g| g.ticker as i32);

    Repository::with_characters_mut(|ch| {
        ch[cn].attack_cn = 0;
        ch[cn].goto_x = 0;
        ch[cn].misc_action = core::constants::DR_TURN as u16;
        ch[cn].misc_target1 = x as u16;
        ch[cn].misc_target2 = y as u16;
        ch[cn].cerrno = 0;
        ch[cn].data[12] = ticker;
    });
}

/// Handle drop item command
///
/// Reads desired drop coordinates from the client and sets the character's
/// `misc_action` to `DR_DROP`, with target coordinates recorded in
/// `misc_target1/2`. Supports special behavior when in building mode.
///
/// # Arguments
/// * `_nr` - Player slot index performing the drop
fn plr_cmd_drop(_nr: usize) {
    let (x, y, cn) = Server::with_players(|players| {
        let x = u16::from_le_bytes([players[_nr].inbuf[1], players[_nr].inbuf[2]]) as i32;
        let y = u16::from_le_bytes([players[_nr].inbuf[3], players[_nr].inbuf[4]]) as i32;
        (x, y, players[_nr].usnr)
    });

    // Building-mode special handling
    let is_building = Repository::with_characters(|ch| ch[cn].is_building());
    if is_building {
        let (action, tx, ty) = Repository::with_characters(|ch| {
            (ch[cn].misc_action, ch[cn].misc_target1, ch[cn].misc_target2)
        });

        if action == core::constants::DR_AREABUILD2 as u16 {
            let xs = std::cmp::min(x as i32, tx as i32);
            let ys = std::cmp::min(y as i32, ty as i32);
            let xe = std::cmp::max(x as i32, tx as i32);
            let ye = std::cmp::max(y as i32, ty as i32);

            State::with(|s| {
                s.do_character_log(
                    cn,
                    core::types::FontColor::Green,
                    &format!("Areaend: {},{}\n", x, y),
                );
                s.do_character_log(
                    cn,
                    core::types::FontColor::Green,
                    &format!("Area: {},{} - {},{}\n", xs, ys, xe, ye),
                );
            });

            // Note: actual build_drop per-tile processing not implemented yet.
            Repository::with_characters_mut(|ch| {
                ch[cn].misc_action = core::constants::DR_AREABUILD1 as u16;
            });
        } else if action == core::constants::DR_AREABUILD1 as u16 {
            Repository::with_characters_mut(|ch| {
                ch[cn].misc_action = core::constants::DR_AREABUILD2 as u16;
                ch[cn].misc_target1 = x as u16;
                ch[cn].misc_target2 = y as u16;
            });
            State::with(|s| {
                s.do_character_log(
                    cn,
                    core::types::FontColor::Green,
                    &format!("Areastart: {},{}\n", x, y),
                );
            });
        } else if action == core::constants::DR_SINGLEBUILD as u16 {
            // Single build: would normally place immediately. Not implemented.
        }

        return;
    }

    let ticker = Repository::with_globals(|g| g.ticker as i32);

    Repository::with_characters_mut(|ch| {
        ch[cn].attack_cn = 0;
        ch[cn].goto_x = 0;
        ch[cn].misc_action = core::constants::DR_DROP as u16;
        ch[cn].misc_target1 = x as u16;
        ch[cn].misc_target2 = y as u16;
        ch[cn].cerrno = 0;
        ch[cn].data[12] = ticker;
    });
}

/// Handle pickup item command
///
/// Reads coordinates of the item to pick up and schedules a `DR_PICKUP`
/// misc action on the character, which will be executed by the per-tick
/// processing. Building-mode special cases are respected.
///
/// # Arguments
/// * `nr` - Player slot index issuing the pickup
fn plr_cmd_pickup(nr: usize) {
    let (x, y, cn) = Server::with_players(|players| {
        let x = u16::from_le_bytes([players[nr].inbuf[1], players[nr].inbuf[2]]) as i32;
        let y = u16::from_le_bytes([players[nr].inbuf[3], players[nr].inbuf[4]]) as i32;
        (x, y, players[nr].usnr)
    });

    // Building-mode: removal in build mode should remove the temporary build object
    let is_building = Repository::with_characters(|ch| ch[cn].is_building());
    if is_building {
        // Call the build removal helper (port of C++ build_remove)
        State::with_mut(|state| state.do_build_remove(x, y));
        return;
    }

    let ticker = Repository::with_globals(|g| g.ticker as i32);

    Repository::with_characters_mut(|ch| {
        ch[cn].attack_cn = 0;
        ch[cn].goto_x = 0;
        ch[cn].misc_action = core::constants::DR_PICKUP as u16;
        ch[cn].misc_target1 = x as u16;
        ch[cn].misc_target2 = y as u16;
        ch[cn].cerrno = 0;
        ch[cn].data[12] = ticker;
    });
}

/// Handle attack command
///
/// Parses the requested target character id and sets the attack variables on
/// the character (`attack_cn`, clears `goto_x`, and resets misc actions)
/// to attempt an attack on subsequent ticks. Also logs the attempt and
/// remembers PvP context.
///
/// # Arguments
/// * `nr` - Player slot index issuing the attack
fn plr_cmd_attack(nr: usize) {
    let co = Server::with_players(|players| {
        u32::from_le_bytes([
            players[nr].inbuf[1],
            players[nr].inbuf[2],
            players[nr].inbuf[3],
            players[nr].inbuf[4],
        ])
    });

    if co as usize >= core::constants::MAXCHARS as usize {
        return;
    }

    let cn = Server::with_players(|players| players[nr].usnr);
    let ticker = Repository::with_globals(|g| g.ticker as i32);

    Repository::with_characters_mut(|ch| {
        ch[cn].attack_cn = co as u16;
        ch[cn].goto_x = 0;
        ch[cn].misc_action = 0;
        ch[cn].cerrno = 0;
        ch[cn].data[12] = ticker;
    });

    Repository::with_characters(|ch| {
        if (co as usize) < ch.len() {
            log::info!("Trying to attack {} ({})", ch[co as usize].get_name(), co);
        }
    });

    State::with(|s| s.remember_pvp(cn, co as usize));
}

/// Handle speed mode command
///
/// Sets the character's movement mode (client-side speed preference). Valid
/// modes are 0..2; after update the character record is refreshed to other
/// clients via `do_update_char`.
///
/// # Arguments
/// * `nr` - Player slot index setting the mode
fn plr_cmd_mode(nr: usize) {
    let mode = Server::with_players(|players| {
        u16::from_le_bytes([players[nr].inbuf[1], players[nr].inbuf[2]])
    });

    if mode > 2 {
        return;
    }

    let cn = Server::with_players(|players| players[nr].usnr);

    Repository::with_characters_mut(|ch| {
        ch[cn].mode = mode as u8;
    });

    State::with(|s| s.do_update_char(cn));

    log::info!("Player {} set speed mode to {}", cn, mode);
}

/// Handle movement command
///
/// Accepts a coordinate target from the client and writes it into
/// `goto_x/goto_y` for the given character so the movement driver will try
/// to move the character towards that target in subsequent ticks.
///
/// # Arguments
/// * `nr` - Player slot index sending the movement target
fn plr_cmd_move(nr: usize) {
    let (x, y, cn) = Server::with_players(|players| {
        let x = u16::from_le_bytes([players[nr].inbuf[1], players[nr].inbuf[2]]) as i32;
        let y = u16::from_le_bytes([players[nr].inbuf[3], players[nr].inbuf[4]]) as i32;
        (x, y, players[nr].usnr)
    });

    let ticker = Repository::with_globals(|g| g.ticker as i32);

    Repository::with_characters_mut(|ch| {
        ch[cn].attack_cn = 0;
        ch[cn].goto_x = x as u16;
        ch[cn].goto_y = y as u16;
        ch[cn].misc_action = 0;
        ch[cn].cerrno = 0;
        ch[cn].data[12] = ticker;
    });
}

/// Handle reset command
///
/// Resets various action-related fields on the character (use/skill/attack/
/// goto/misc) and stamps the timestamp so that the character stops any
/// ongoing activity.
///
/// # Arguments
/// * `nr` - Player slot index requesting the reset
fn plr_cmd_reset(nr: usize) {
    let cn = Server::with_players(|players| players[nr].usnr);
    let ticker = Repository::with_globals(|g| g.ticker as i32);

    Repository::with_characters_mut(|ch| {
        ch[cn].use_nr = 0;
        ch[cn].skill_nr = 0;
        ch[cn].attack_cn = 0;
        ch[cn].goto_x = 0;
        ch[cn].goto_y = 0;
        ch[cn].misc_action = 0;
        ch[cn].cerrno = 0;
        ch[cn].data[12] = ticker;
    });
}

/// Handle skill use command
///
/// Parses the requested skill index and target character and schedules the
/// skill for execution by setting `skill_nr` and `skill_target1` on the
/// initiating character. Validates indices and existence of the skill.
///
/// # Arguments
/// * `nr` - Player slot index invoking the skill
fn plr_cmd_skill(nr: usize) {
    let (n, co, cn) = Server::with_players(|players| {
        let n = u32::from_le_bytes([
            players[nr].inbuf[1],
            players[nr].inbuf[2],
            players[nr].inbuf[3],
            players[nr].inbuf[4],
        ]) as usize;
        let co = u32::from_le_bytes([
            players[nr].inbuf[5],
            players[nr].inbuf[6],
            players[nr].inbuf[7],
            players[nr].inbuf[8],
        ]) as usize;
        (n, co, players[nr].usnr)
    });

    // sanity checks: skill index must be within available skill table
    if n >= core::types::Character::default().skill.len() {
        return;
    }
    if co >= core::constants::MAXCHARS as usize {
        return;
    }

    // ensure skill exists for this character
    let has_skill = Repository::with_characters(|ch| ch[cn].skill[n][0] != 0);
    if !has_skill {
        return;
    }

    Repository::with_characters_mut(|ch| {
        ch[cn].skill_nr = n as u16;
        ch[cn].skill_target1 = co as u16;
    });
}

/// Handle inventory look command
///
/// Allows the player to inspect their inventory slot or (if building mode)
/// set up area-building operations by selecting a slot as the carried item.
/// Otherwise delegates to `do_look_item` for the item at the selected slot.
///
/// # Arguments
/// * `nr` - Player slot index issuing the command
fn plr_cmd_inv_look(nr: usize) {
    let (n, cn) = Server::with_players(|players| {
        let n = u16::from_le_bytes([players[nr].inbuf[1], players[nr].inbuf[2]]) as usize;
        (n, players[nr].usnr)
    });

    if n > 39 {
        return;
    }

    let is_building = Repository::with_characters(|ch| ch[cn].is_building());
    if is_building {
        // set carried item to the selected inventory slot and enter area-build
        Repository::with_characters_mut(|ch| {
            ch[cn].citem = ch[cn].item[n];
            ch[cn].misc_action = core::constants::DR_AREABUILD1 as u16;
        });
        State::with(|s| s.do_character_log(cn, core::types::FontColor::Green, "Area mode\n"));
        return;
    }

    let in_idx = Repository::with_characters(|ch| ch[cn].item[n] as usize);
    if in_idx != 0 {
        State::with_mut(|s| s.do_look_item(cn, in_idx));
    }
}

/// Handle use command
///
/// Reads coordinates from the client and schedules a `DR_USE` misc action
/// so that the item on the specified tile will be used by the character on
/// the next tick.
///
/// # Arguments
/// * `nr` - Player slot index issuing the use
fn plr_cmd_use(nr: usize) {
    let (x, y, cn) = Server::with_players(|players| {
        let x = u16::from_le_bytes([players[nr].inbuf[1], players[nr].inbuf[2]]) as i32;
        let y = u16::from_le_bytes([players[nr].inbuf[3], players[nr].inbuf[4]]) as i32;
        (x, y, players[nr].usnr)
    });

    let ticker = Repository::with_globals(|g| g.ticker as i32);

    Repository::with_characters_mut(|ch| {
        ch[cn].attack_cn = 0;
        ch[cn].goto_x = 0;
        ch[cn].misc_action = core::constants::DR_USE as u16;
        ch[cn].misc_target1 = x as u16;
        ch[cn].misc_target2 = y as u16;
        ch[cn].cerrno = 0;
        ch[cn].data[12] = ticker;
    });
}

/// Handle inventory manipulation command
///
/// Multi-purpose handler for inventory operations (placing/withdrawing
/// items and gold, swapping, selecting use slots, and viewing worn/inv
/// items). The `what` parameter selects the sub-action type while `n` and
/// `co` provide action-specific parameters.
///
/// # Arguments
/// * `nr` - Player slot index issuing the inventory command
fn plr_cmd_inv(nr: usize) {
    let (what, n, mut co, cn) = Server::with_players(|players| {
        let what = u32::from_le_bytes([
            players[nr].inbuf[1],
            players[nr].inbuf[2],
            players[nr].inbuf[3],
            players[nr].inbuf[4],
        ]) as usize;
        let n = u32::from_le_bytes([
            players[nr].inbuf[5],
            players[nr].inbuf[6],
            players[nr].inbuf[7],
            players[nr].inbuf[8],
        ]) as usize;
        let co = u32::from_le_bytes([
            players[nr].inbuf[9],
            players[nr].inbuf[10],
            players[nr].inbuf[11],
            players[nr].inbuf[12],
        ]) as usize;
        (what, n, co, players[nr].usnr)
    });

    if co < 1 || co >= core::constants::MAXCHARS as usize {
        co = 0;
    }

    // what == 0 : normal inventory
    if what == 0 {
        if n > 39 {
            return;
        }

        let stunned = Repository::with_characters(|ch| ch[cn].stunned > 0);
        if stunned {
            return;
        }

        // check for lag scroll template on the item
        let tmp = Repository::with_characters(|ch| ch[cn].item[n] as usize);
        let is_lag = Repository::with_items(|items| {
            if tmp != 0 && tmp < items.len() {
                items[tmp].temp as i32 == core::constants::IT_LAGSCROLL
            } else {
                false
            }
        });
        if is_lag {
            return;
        }

        State::with(|s| s.do_update_char(cn));

        // Now handle citem/gold swap or placing citem into slot
        Repository::with_characters_mut(|ch| {
            if (ch[cn].citem & 0x80000000) != 0 {
                let tmpval = ch[cn].citem & 0x7fffffff;
                if tmpval > 0 {
                    ch[cn].gold += tmpval as i32;
                }
                ch[cn].citem = 0;
                return;
            } else {
                if !ch[cn].is_building() {
                    ch[cn].item[n] = ch[cn].citem;
                } else {
                    ch[cn].misc_action = core::constants::DR_SINGLEBUILD as u16;
                }
                ch[cn].citem = tmp as u32;
                return;
            }
        });
    }

    // what == 1 : big inventory swap
    if what == 1 {
        let stunned = Repository::with_characters(|ch| ch[cn].stunned > 0);
        if stunned {
            return;
        }
        State::with(|s| {
            let _ = s.do_swap_item(cn, n);
        });
        return;
    }

    // what == 2 : withdraw gold into cursor
    if what == 2 {
        let stunned = Repository::with_characters(|ch| ch[cn].stunned > 0);
        if stunned {
            return;
        }
        let citem = Repository::with_characters(|ch| ch[cn].citem);
        if citem != 0 {
            return;
        }
        if n as i32 > Repository::with_characters(|ch| ch[cn].gold) {
            return;
        }
        if n == 0 {
            return;
        }
        Repository::with_characters_mut(|ch| {
            ch[cn].citem = 0x80000000 | (n as u32);
            ch[cn].gold -= n as i32;
        });
        State::with(|s| s.do_update_char(cn));
        return;
    }

    // what == 5 : use_nr = n (worn slots)
    if what == 5 {
        if n > 19 {
            return;
        }
        let is_building = Repository::with_characters(|ch| ch[cn].is_building());
        if is_building {
            return;
        }
        Repository::with_characters_mut(|ch| {
            ch[cn].use_nr = n as u16;
            ch[cn].skill_target1 = co as u16;
        });
        return;
    }

    // what == 6 : use_nr = n + 20 (inventory)
    if what == 6 {
        if n > 39 {
            return;
        }
        let is_building = Repository::with_characters(|ch| ch[cn].is_building());
        if is_building {
            return;
        }
        Repository::with_characters_mut(|ch| {
            ch[cn].use_nr = (n as u16) + 20;
            ch[cn].skill_target1 = co as u16;
        });
        return;
    }

    // what == 7 : look at worn item
    if what == 7 {
        if n > 19 {
            return;
        }
        let is_building = Repository::with_characters(|ch| ch[cn].is_building());
        if is_building {
            return;
        }
        let in_idx = Repository::with_characters(|ch| ch[cn].worn[n] as usize);
        if in_idx != 0 {
            State::with_mut(|s| s.do_look_item(cn, in_idx));
        }
        return;
    }

    // what == 8 : look at inventory item
    if what == 8 {
        if n > 39 {
            return;
        }
        let is_building = Repository::with_characters(|ch| ch[cn].is_building());
        if is_building {
            return;
        }
        let in_idx = Repository::with_characters(|ch| ch[cn].item[n] as usize);
        if in_idx != 0 {
            State::with_mut(|s| s.do_look_item(cn, in_idx));
        }
        return;
    }

    log::warn!("Unknown CMD-INV-what {}", what);
}

/// Handle exit command (F12)
///
/// Performs an immediate logout for the requesting player slot by
/// calling `plr_logout` with `LogoutReason::Exit`.
///
/// # Arguments
/// * `nr` - Player slot index pressing F12
fn plr_cmd_exit(nr: usize) {
    log::info!("Player {} pressed F12", nr);
    let cn = Server::with_players(|players| players[nr].usnr);
    plr_logout(cn, nr, enums::LogoutReason::Exit);
}

/// Handle shop command
///
/// Handles buying/selling interactions with shops or depot operations when
/// the high bit of `co` is set (depot index). Delegates to `do_depot_char`
/// or `do_shop_char` to perform the actual shop/depot logic.
///
/// # Arguments
/// * `nr` - Player slot index issuing the shop command
fn plr_cmd_shop(nr: usize) {
    let (co, n, cn) = Server::with_players(|players| {
        let co = u16::from_le_bytes([players[nr].inbuf[1], players[nr].inbuf[2]]) as usize;
        let n = u16::from_le_bytes([players[nr].inbuf[3], players[nr].inbuf[4]]) as i32;
        (co, n, players[nr].usnr)
    });

    if (co & 0x8000) != 0 {
        let idx = co & 0x7fff;
        State::with_mut(|s| s.do_depot_char(cn, idx, n));
    } else {
        State::with_mut(|s| s.do_shop_char(cn, co, n));
    }
}
