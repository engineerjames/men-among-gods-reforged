use crate::{
    enums, god::God, network_manager::NetworkManager, repository::Repository, server::Server,
    state::State,
};

pub fn plr_logout(character_id: usize, player_id: usize, reason: enums::LogoutReason) {
    // Logic to log out the player
    Repository::with_characters(|characters| {
        log::debug!(
            "Logging out character '{}' for reason: {:?}",
            characters[character_id].get_name(),
            reason
        );
    });

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
                    God::build(character, character_id, 0);
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

        Repository::with_globals(|globals| {
            player_exit(globals.ticker as u32, character_id, player_id);
        });
    }
}

pub fn player_exit(ticker: u32, character_id: usize, player_id: usize) {
    Repository::with_characters_mut(|characters| {
        let ch = &mut characters[character_id];
        log::info!(
            "Player {} exiting for character '{}'",
            player_id,
            ch.get_name()
        );

        Server::with_players_mut(|players| {
            players[player_id].state = core::constants::ST_EXIT;
            players[player_id].lasttick = ticker;

            if players[player_id].usnr > 0
                && players[player_id].usnr < core::constants::MAXCHARS
                && ch.player as usize == player_id
            {
                ch.player = 0;
            }
        });
    });
}

/// Port of `plr_map_remove` from `svr_act.cpp`
/// Remove character from map
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
                            // TODO: Call step_driver_remove when implemented
                        }
                    });
                }
            }
        });
    });
}

/// Port of `plr_map_set` from `svr_act.cpp`
/// Set character to map and remove target character
pub fn plr_map_set(cn: usize) {
    let (x, y, flags, dir, light) = Repository::with_characters(|characters| {
        (
            characters[cn].x,
            characters[cn].y,
            characters[cn].flags,
            characters[cn].dir,
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
                // TODO: Call step_driver and handle return values
                // For now, just set the character on the map
            }
        }

        // Check for tavern
        let is_tavern =
            Repository::with_map(|map| (map[m].flags & core::constants::MF_TAVERN as u64) != 0);

        if is_tavern && is_player {
            Repository::with_characters_mut(|characters| {
                if characters[cn].is_building() {
                    God::build(&mut characters[cn].clone(), cn, 0);
                }
                characters[cn].tavern_x = characters[cn].x as u16;
                characters[cn].tavern_y = characters[cn].y as u16;
            });

            log::info!("Character {} entered tavern", cn);

            let player_id = Repository::with_characters(|characters| characters[cn].player);
            plr_logout(cn, player_id as usize, enums::LogoutReason::Tavern);
            return;
        }

        // Check for no magic zone
        // TODO: Implement char_wears_item checks for items 466 and 481
        let is_nomagic =
            Repository::with_map(|map| (map[m].flags & core::constants::MF_NOMAGIC as u64) != 0);

        if is_nomagic {
            Repository::with_characters_mut(|characters| {
                if (characters[cn].flags & enums::CharacterFlags::NoMagic.bits()) == 0 {
                    characters[cn].flags |= enums::CharacterFlags::NoMagic.bits();
                    // TODO: Call remove_spells when implemented
                    State::with(|state| {
                        state.do_character_log(
                            cn,
                            core::types::FontColor::Red,
                            "You feel your magic fail.\n",
                        );
                    });
                }
            });
        } else {
            Repository::with_characters_mut(|characters| {
                if (characters[cn].flags & enums::CharacterFlags::NoMagic.bits()) != 0 {
                    characters[cn].flags &= !enums::CharacterFlags::NoMagic.bits();
                    // TODO: Call do_update_char when implemented
                    State::with(|state| {
                        state.do_character_log(
                            cn,
                            core::types::FontColor::Red,
                            "You feel your magic return.\n",
                        );
                    });
                }
            });
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
        // TODO: Call do_attack when implemented
        log::debug!("Would call do_attack({}, {}, {})", cn, co, surround);
    }
}

/// Port of `plr_give` from `svr_act.cpp`
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

    // TODO: Call do_give when implemented
    log::debug!("Would call do_give({}, {})", cn, co);
}

/// Port of `plr_pickup` from `svr_act.cpp`
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

    let (m, x, y, dir) = Repository::with_characters(|characters| {
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
        (m, x, y, dir)
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

    // TODO: Call do_update_char when implemented

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

    let (m, dir) = Repository::with_characters(|characters| {
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
        (m, dir)
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

    // TODO: Call use_driver when implemented
    log::debug!("Would call use_driver({}, {}, 0)", cn, in_id);
}

/// Port of `plr_skill` from `svr_act.cpp`
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

    // TODO: Call skill_driver when implemented
    log::debug!("Would call skill_driver({}, {})", cn, skill_target);
}

/// Port of `plr_drop` from `svr_act.cpp`
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
            // TODO: Call step_driver when implemented
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

    // TODO: Call do_update_char when implemented

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
        // TODO: Call do_maygive when implemented
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
