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

pub fn plr_misc(cn: usize) {}

pub fn plr_check_target(m: usize) {}

pub fn plr_set_target(m: usize, cn: usize) {}

pub fn plr_drop(cn: usize) {}

pub fn plr_skill(cn: usize) {}

pub fn plr_use(cn: usize) {}

pub fn plr_wave(cn: usize) {}

pub fn plr_bow(cn: usize) {}

pub fn plr_pickup(cn: usize) {}

pub fn plr_give(cn: usize) {}

pub fn plr_attack(cn: usize, surround: i32) {}

pub fn plr_turn_right(cn: usize) {}

pub fn plr_turn_left(cn: usize) {}

pub fn plr_turn_rightup(cn: usize) {}

pub fn plr_turn_rightdown(cn: usize) {}

pub fn plr_turn_down(cn: usize) {}

pub fn plr_turn_leftdown(cn: usize) {}

pub fn plr_turn_leftup(cn: usize) {}

pub fn plr_turn_up(cn: usize) {}

pub fn plr_move_rightdown(cn: usize) {}

pub fn plr_move_rightup(cn: usize) {}

pub fn plr_move_leftdown(cn: usize) {}

pub fn plr_move_leftup(cn: usize) {}

pub fn plr_move_right(cn: usize) {}

pub fn plr_move_left(cn: usize) {}

pub fn plr_move_down(cn: usize) {}

pub fn plr_move_up(cn: usize) {}

pub fn plr_map_set(cn: usize) {}

pub fn plr_map_remove(cn: usize) {}
