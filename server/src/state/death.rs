use core::constants::{CharacterFlags, MAXCHARS};
use core::types::{Character, FontColor};
use rand::Rng;

use crate::effect::EffectManager;
use crate::god::God;
use crate::repository::Repository;
use crate::server::Server;
use crate::{helpers, player};

use super::State;

impl State {
    /// Port of `do_character_killed(character_id, killer_id)` from the original
    /// server sources.
    ///
    /// Top-level handler invoked when a character dies. Responsibilities:
    /// - Send death notifications to nearby characters
    /// - Play appropriate death sound effects
    /// - Log the kill and update killer/player statistics
    /// - Apply alignment, luck and penalty changes for killers
    /// - Handle special-case followers and companion cleanup
    /// - Create a grave/body clone and schedule respawn effects
    /// - Route player vs NPC death handling (resurrection, respawn)
    ///
    /// # Arguments
    /// * `character_id` - The character who died
    /// * `killer_id` - The character who killed them (0 if none)
    pub(crate) fn do_character_killed(&self, character_id: usize, killer_id: usize) {
        // Send death notification
        self.do_notify_character(
            character_id as u32,
            core::constants::NT_DIED as i32,
            killer_id as i32,
            0,
            0,
            0,
        );

        // Log the kill
        if killer_id != 0 {
            log::info!(
                "Character {} killed character {} ({})",
                killer_id,
                character_id,
                Repository::with_characters(|ch| ch[character_id].get_name().to_string())
            );
        } else {
            log::info!("Character {} died", character_id);
        }

        // Get map flags for both characters
        let (co_x, co_y, co_temp, co_sound) = Repository::with_characters(|characters| {
            let co = &characters[character_id];
            (co.x, co.y, co.temp, co.sound)
        });

        let mut map_flags = Repository::with_map(|map| {
            let idx = (co_x + co_y * core::constants::SERVER_MAPX as i16) as usize;
            map[idx].flags
        });

        if killer_id != 0 {
            let cn_flags = Repository::with_characters(|characters| {
                let cn = &characters[killer_id];
                let idx = (cn.x + cn.y * core::constants::SERVER_MAPX as i16) as usize;
                Repository::with_map(|map| map[idx].flags)
            });
            map_flags &= cn_flags;
        }

        // Play death sound effects
        // Hack for grolms (templates 364-374)
        if co_temp >= 364 && co_temp <= 374 {
            Self::do_area_sound(character_id, 0, co_x as i32, co_y as i32, 17);
            Self::char_play_sound(character_id, 17, -150, 0);
        }
        // Hack for gargoyles (templates 375-381)
        else if co_temp >= 375 && co_temp <= 381 {
            Self::do_area_sound(character_id, 0, co_x as i32, co_y as i32, 18);
            Self::char_play_sound(character_id, 18, -150, 0);
        }
        // Normal death sound
        else {
            let sound = co_sound + 2;
            Self::do_area_sound(character_id, 0, co_x as i32, co_y as i32, sound as i32);
            Self::char_play_sound(character_id, sound as i32, -150, 0);
        }

        // Cleanup for ghost companions
        if co_temp == core::constants::CT_COMPANION as u16 {
            Repository::with_characters_mut(|characters| {
                let cc = characters[character_id].data[63] as usize;
                if Character::is_sane_character(cc)
                    && characters[cc].data[64] == character_id as i32
                {
                    characters[cc].data[64] = 0;
                }
                characters[character_id].data[63] = 0;
            });
        }

        // A player killed someone or something
        if killer_id != 0 && killer_id != character_id {
            let (is_killer_player, is_arena, co_alignment, co_temp, co_is_player) =
                Repository::with_characters(|characters| {
                    let is_killer_player =
                        characters[killer_id].flags & CharacterFlags::CF_PLAYER.bits() != 0;
                    let co_alignment = characters[character_id].alignment;
                    let co_temp = characters[character_id].temp;
                    let co_is_player =
                        characters[character_id].flags & CharacterFlags::CF_PLAYER.bits() != 0;
                    (
                        is_killer_player,
                        map_flags & core::constants::MF_ARENA as u64 == 0,
                        co_alignment,
                        co_temp,
                        co_is_player,
                    )
                });

            if is_killer_player && is_arena {
                // Adjust alignment
                Repository::with_characters_mut(|characters| {
                    characters[killer_id].alignment -= co_alignment / 50;
                    if characters[killer_id].alignment > 7500 {
                        characters[killer_id].alignment = 7500;
                    }
                    if characters[killer_id].alignment < -7500 {
                        characters[killer_id].alignment = -7500;
                    }
                });

                // Check for killing priests (becoming purple)
                if co_temp == core::constants::CT_PRIEST as u16 {
                    let killer_kindred = Repository::with_characters(|ch| ch[killer_id].kindred);

                    if killer_kindred as u32 & core::constants::KIN_PURPLE != 0 {
                        self.do_character_log(
                            killer_id,
                            core::types::FontColor::Yellow,
                            "Ahh, that felt good!\n",
                        );
                    } else {
                        Repository::with_characters_mut(|characters| {
                            Repository::with_globals_mut(|globals| {
                                characters[killer_id].data[67] = globals.ticker;
                            });
                        });
                        self.do_character_log(
                            killer_id,
                            core::types::FontColor::Red,
                            "So, you want to be a player killer, right?\n",
                        );
                        self.do_character_log(
                            killer_id,
                            core::types::FontColor::Red,
                            "To join the purple one and be a killer, type #purple now.\n",
                        );
                        Repository::with_characters(|ch| {
                            EffectManager::fx_add_effect(
                                6,
                                co_x as i32,
                                co_y as i32,
                                0,
                                ch[killer_id].player,
                            );
                        });
                    }
                }

                // Check for killing shopkeepers & questgivers (alignment 10000)
                if !co_is_player && co_alignment == 10000 {
                    self.do_character_log(
                        killer_id,
                        core::types::FontColor::Red,
                        "You feel a god look into your soul. He seems to be angry.\n",
                    );

                    Repository::with_characters_mut(|characters| {
                        characters[killer_id].data[40] += 1;
                        let penalty = if characters[killer_id].data[40] < 50 {
                            -characters[killer_id].data[40] * 100
                        } else {
                            -5000
                        };
                        characters[killer_id].luck += penalty;

                        let luck_to_print = characters[killer_id].luck;
                        log::info!(
                            "Reduced luck by {} to {} for killing {} (t={})",
                            penalty,
                            luck_to_print,
                            characters[character_id].get_name(),
                            co_temp
                        );
                    });
                }

                Repository::with_characters_mut(|characters| {
                    // Update statistics
                    let r1: u32 = helpers::points2rank(characters[killer_id].points_tot as u32);
                    let r2: u32 = helpers::points2rank(characters[character_id].points_tot as u32);

                    if (r1 as i32 - r2 as i32).abs() < 3 {
                        // Approximately own rank
                        characters[killer_id].data[24] += 1; // overall counter
                        if characters[character_id].data[42] == 27 {
                            characters[killer_id].data[27] += 1; // black stronghold counter
                        }
                    } else if r2 > r1 {
                        // Above own rank
                        characters[killer_id].data[25] += 1;
                        if characters[character_id].data[42] == 27 {
                            characters[killer_id].data[28] += 1;
                        }
                    } else {
                        // Below own rank
                        characters[killer_id].data[23] += 1;
                        if characters[character_id].data[42] == 27 {
                            characters[killer_id].data[26] += 1;
                        }
                    }

                    if co_is_player {
                        characters[killer_id].data[29] += 1;
                    } else {
                        // Check for first kill of this monster class
                        let monster_class = characters[character_id].monster_class;
                        if monster_class != 0 {
                            // killed_class: returns true if already killed, false if first kill
                            if !helpers::killed_class(killer_id, monster_class) {
                                let class_name = helpers::get_class_name(monster_class);
                                State::with_mut(|state| {
                                    state.do_character_log(
                                        killer_id,
                                        core::types::FontColor::Yellow,
                                        &format!(
                                            "You just killed your first {}. Good job.\n",
                                            class_name
                                        ),
                                    );
                                    state.do_give_exp(
                                        killer_id,
                                        state.do_char_score(character_id) * 25,
                                        0,
                                        -1,
                                    );
                                });
                            }
                        }
                    }
                });
            }

            // A follower (gargoyle, ghost companion) killed someone
            let follower_owner = Repository::with_characters(|characters| {
                if characters[killer_id].flags & CharacterFlags::CF_PLAYER.bits() == 0 {
                    let cc = characters[killer_id].data[63] as usize;
                    if cc != 0 && Character::is_sane_character(cc) {
                        Some(cc)
                    } else {
                        None
                    }
                } else {
                    None
                }
            });

            if let Some(cc) = follower_owner {
                let is_owner_player = Repository::with_characters(|ch| {
                    ch[cc].flags & CharacterFlags::CF_PLAYER.bits() != 0
                });

                if is_owner_player && !co_is_player && co_alignment == 10000 {
                    self.do_character_log(cc, core::types::FontColor::Red,
                        "A goddess is about to turn your follower into a frog, but notices that you are responsible. You feel her do something to you. Nothing good, that's for sure.\n");

                    Repository::with_characters_mut(|characters| {
                        characters[cc].data[40] += 1;
                        let penalty = if characters[cc].data[40] < 50 {
                            -characters[cc].data[40] * 100
                        } else {
                            -5000
                        };
                        characters[cc].luck += penalty;

                        let luck_to_print = characters[cc].luck;
                        log::info!(
                            "Reduced luck by {} to {} for follower killing {} (t={})",
                            penalty,
                            luck_to_print,
                            characters[character_id].get_name(),
                            co_temp
                        );
                    });
                }

                // Notify area about the kill
                let (cc_x, cc_y) = Repository::with_characters(|ch| (ch[cc].x, ch[cc].y));
                self.do_area_notify(
                    cc as i32,
                    character_id as i32,
                    cc_x as i32,
                    cc_y as i32,
                    core::constants::NT_SEEHIT as i32,
                    cc as i32,
                    character_id as i32,
                    0,
                    0,
                );
            }
        }

        // Handle player death
        let is_player = Repository::with_characters(|ch| {
            ch[character_id].flags & CharacterFlags::CF_PLAYER.bits() != 0
        });

        if is_player {
            // Update player death statistics
            Repository::with_globals_mut(|globals| {
                globals.players_died += 1;
            });

            // Adjust luck if negative
            Repository::with_characters_mut(|characters| {
                if characters[character_id].luck < 0 {
                    characters[character_id].luck =
                        std::cmp::min(0, characters[character_id].luck + 10);
                }

                // Set killed by message
                characters[character_id].data[14] += 1;
                if killer_id != 0 {
                    let is_killer_player =
                        characters[killer_id].flags & CharacterFlags::CF_PLAYER.bits() != 0;
                    if is_killer_player {
                        characters[character_id].data[15] = killer_id as i32 | 0x10000;
                    } else {
                        characters[character_id].data[15] = characters[killer_id].temp as i32;
                    }
                } else {
                    characters[character_id].data[15] = 0;
                }

                Repository::with_globals(|globals| {
                    characters[character_id].data[16] = globals.mdday + globals.mdyear * 300;
                });
                characters[character_id].data[17] =
                    (co_x + co_y * core::constants::SERVER_MAPX as i16) as i32;
            });

            self.handle_player_death(character_id, killer_id, map_flags);
        } else {
            // Handle NPC death
            let is_labkeeper = Repository::with_characters(|ch| {
                ch[character_id].flags & CharacterFlags::CF_LABKEEPER.bits() != 0
            });

            if is_labkeeper {
                self.handle_labkeeper_death(character_id, killer_id);
            } else {
                self.handle_npc_death(character_id, killer_id);
            }
        }

        // Remove from enemy lists
        State::remove_enemy(character_id);

        // Schedule respawn and show death animation
        let fn_idx = EffectManager::fx_add_effect(
            3,
            co_x as i32,
            co_y as i32,
            character_id as i32,
            Repository::with_characters(|ch| ch[character_id].player),
        );
        // Set data[3] = killer_id for the effect, if possible
        Repository::with_effects_mut(|effects| {
            if fn_idx.unwrap() < effects.len() {
                effects[fn_idx.unwrap()].data[3] = killer_id as u32;
            }
        });
    }

    /// Port of `handle_player_death(co, cn, map_flags)` from the original server
    /// sources.
    ///
    /// Handles player-specific death processing:
    /// - Check for Guardian Angel / wimpy skill and compute `wimp` chance
    /// - Allocate a free character slot and clone the dead character into a grave/body
    /// - Drop items and money into the grave according to `wimp` chance
    /// - Transfer the player to their temple and resurrect with minimal HP
    /// - Reset status and apply permanent stat penalties when applicable
    ///
    /// # Arguments
    /// * `co` - Character id of the dead player
    /// * `cn` - Killer id
    /// * `map_flags` - Map flags at the death location (used for arena/wimp checks)
    pub(crate) fn handle_player_death(&self, co: usize, cn: usize, map_flags: u64) {
        // Remember template if we're to respawn this character
        // TODO: Re-evaluate if we need to do anything here.

        // Check for Guardian Angel (Wimpy skill)
        let wimp = Repository::with_characters(|characters| {
            let mut wimp_power = 0;
            for n in 0..20 {
                let item_idx = characters[co].spell[n] as usize;
                if item_idx != 0 {
                    Repository::with_items(|items| {
                        let power_to_print = items[item_idx].power;
                        if item_idx < items.len() {
                            log::info!(
                                "spell active: {}, power of {}",
                                items[item_idx].get_name(),
                                power_to_print
                            );
                            if items[item_idx].temp == core::constants::SK_WIMPY as u16 {
                                wimp_power = items[item_idx].power / 2;
                            }
                        }
                    });
                }
            }
            wimp_power
        });

        let wimp = if map_flags & core::constants::MF_ARENA as u64 != 0 {
            205
        } else {
            wimp
        };

        // Find free character slot for body/grave
        let cc = Repository::with_characters(|characters| {
            for cc in 1..MAXCHARS {
                if characters[cc].used == core::constants::USE_EMPTY {
                    return Some(cc);
                }
            }
            None
        });

        let Some(cc) = cc else {
            log::error!(
                "Could not clone character {} for grave, all char slots full!",
                co
            );
            return;
        };

        // Clone character to create grave
        Repository::with_characters_mut(|characters| {
            characters[cc] = characters[co].clone();
        });

        // Drop items and money based on wimp chance
        self.handle_item_drops(co, cc, wimp as i32, cn);

        // Move player to temple
        let (temple_x, temple_y, cur_x, cur_y) = Repository::with_characters(|ch| {
            (ch[co].temple_x, ch[co].temple_y, ch[co].x, ch[co].y)
        });

        if cur_x as u16 == temple_x && cur_y as u16 == temple_y {
            God::transfer_char(co, (temple_x + 4) as usize, (temple_y + 4) as usize);
        } else {
            God::transfer_char(co, temple_x as usize, temple_y as usize);
        }

        // Resurrect player with 10 HP
        Repository::with_characters_mut(|characters| {
            characters[co].a_hp = 10000; // 10 HP (stored as 10000)
            characters[co].status = 0;
            characters[co].attack_cn = 0;
            characters[co].skill_nr = 0;
            characters[co].goto_x = 0;
            characters[co].use_nr = 0;
            characters[co].misc_action = 0;
            characters[co].stunned = 0;
            characters[co].retry = 0;
            characters[co].current_enemy = 0;
            for m in 0..4 {
                characters[co].enemy[m] = 0;
            }
        });

        player::plr_reset_status(co);

        // Apply permanent stat loss if not a god and no guardian angel
        let is_god =
            Repository::with_characters(|ch| ch[co].flags & CharacterFlags::CF_GOD.bits() != 0);

        if !is_god && wimp == 0 {
            self.apply_death_penalties(co);
        } else if wimp != 0 && map_flags & core::constants::MF_ARENA as u64 == 0 {
            self.do_character_log(
                co,
                core::types::FontColor::Yellow,
                "Sometimes a Guardian Angel is really helpful...\n",
            );
        }

        // Update player character
        Repository::with_characters_mut(|ch| {
            ch[co].set_do_update_flags();
        });

        // Setup the grave (body)
        Repository::with_characters_mut(|characters| {
            player::plr_reset_status(cc);

            characters[cc].player = 0;
            characters[cc].flags = CharacterFlags::CF_BODY.bits();
            characters[cc].a_hp = 0;
            characters[cc].data[core::constants::CHD_CORPSEOWNER] = co as i32;
            characters[cc].data[99] = 1;
            characters[cc].data[98] = 0;

            characters[cc].attack_cn = 0;
            characters[cc].skill_nr = 0;
            characters[cc].goto_x = 0;
            characters[cc].use_nr = 0;
            characters[cc].misc_action = 0;
            characters[cc].stunned = 0;
            characters[cc].retry = 0;
            characters[cc].current_enemy = 0;
            for m in 0..4 {
                characters[cc].enemy[m] = 0;
            }
        });

        // Update grave character
        Repository::with_characters_mut(|ch| {
            ch[cc].set_do_update_flags();
        });

        player::plr_map_set(cc);
    }

    /// Port of `handle_npc_death(co, cn)` from the original server sources.
    ///
    /// Handles non-player character (NPC) death processing:
    /// - Increment NPC death counters
    /// - Reset NPC status and active actions
    /// - Handle `USURP` (player controlling an NPC) transfer if present
    /// - Convert the NPC into a corpse/body and set respawn flags where appropriate
    /// - Destroy active spells and allow player ransack when killer is a player
    ///
    /// # Arguments
    /// * `co` - NPC character id who died
    /// * `cn` - Killer id
    pub(crate) fn handle_npc_death(&self, co: usize, cn: usize) {
        // Update NPC death statistics
        Repository::with_globals_mut(|globals| {
            globals.npcs_died += 1;
        });

        player::plr_reset_status(co);

        // Check for USURP flag (player controlling NPC)
        let usurp_player = Repository::with_characters(|characters| {
            if characters[co].flags & CharacterFlags::CF_USURP.bits() != 0 {
                let c2 = characters[co].data[97] as usize;
                if Character::is_sane_character(c2) {
                    Some((c2, characters[co].player))
                } else {
                    None
                }
            } else {
                None
            }
        });

        if let Some((c2, player_nr)) = usurp_player {
            Repository::with_characters_mut(|characters| {
                characters[c2].player = player_nr;
                Server::with_players_mut(|players| {
                    players[player_nr as usize].usnr = c2;
                });
                characters[c2].flags &= !CharacterFlags::CF_CCP.bits();
            });
        } else if let Some((_, player_nr)) = usurp_player {
            player::player_exit(player_nr as usize);
        }

        log::info!("new npc body");

        // Convert to body
        let should_respawn = Repository::with_characters(|characters| {
            characters[co].flags & CharacterFlags::CF_RESPAWN.bits() != 0
        });

        Repository::with_characters_mut(|characters| {
            if should_respawn {
                characters[co].flags =
                    CharacterFlags::CF_BODY.bits() | CharacterFlags::CF_RESPAWN.bits();
            } else {
                characters[co].flags = CharacterFlags::CF_BODY.bits();
            }

            characters[co].a_hp = 0;

            // Set corpse owner (killer only mode vs all can loot)
            #[cfg(feature = "KILLERONLY")]
            {
                let cc = Repository::with_characters(|ch| {
                    if cn != 0 && !(ch[cn].flags & CharacterFlags::CF_PLAYER.bits() != 0) {
                        let cc = ch[cn].data[63] as usize;
                        if cc != 0 && (ch[cc].flags & CharacterFlags::CF_PLAYER.bits() != 0) {
                            Some(cc)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                });

                if let Some(cc) = cc {
                    characters[co].data[CHD_CORPSEOWNER] = cc as i32;
                } else if cn != 0 {
                    let is_cn_player = characters[cn].flags & CharacterFlags::CF_PLAYER.bits() != 0;
                    if is_cn_player {
                        characters[co].data[CHD_CORPSEOWNER] = cn as i32;
                    } else {
                        characters[co].data[CHD_CORPSEOWNER] = 0;
                    }
                } else {
                    characters[co].data[CHD_CORPSEOWNER] = 0;
                }
            }
            #[cfg(not(feature = "KILLERONLY"))]
            {
                characters[co].data[core::constants::CHD_CORPSEOWNER] = 0;
            }

            characters[co].data[99] = 0;
            characters[co].data[98] = 0;

            characters[co].attack_cn = 0;
            characters[co].skill_nr = 0;
            characters[co].goto_x = 0;
            characters[co].use_nr = 0;
            characters[co].misc_action = 0;
            characters[co].stunned = 0;
            characters[co].retry = 0;
            characters[co].current_enemy = 0;
            for m in 0..4 {
                characters[co].enemy[m] = 0;
            }

            // Destroy active spells
            for n in 0..20 {
                if characters[co].spell[n] != 0 {
                    let item_idx = characters[co].spell[n] as usize;
                    characters[co].spell[n] = 0;
                    Repository::with_items_mut(|items| {
                        if item_idx < items.len() {
                            items[item_idx].used = core::constants::USE_EMPTY;
                        }
                    });
                }
            }
        });

        // If killer is a player, check for special items in grave
        let is_cn_player = if cn != 0 {
            Repository::with_characters(|ch| {
                Character::is_sane_character(cn)
                    && ch[cn].flags & CharacterFlags::CF_PLAYER.bits() != 0
            })
        } else {
            false
        };

        if is_cn_player {
            State::with(|state| {
                state.do_ransack_corpse(
                    co,
                    cn,
                    "You notice %s tumble into the grave of your victim.\n",
                );
            });
        }

        // Update character
        Repository::with_characters_mut(|ch| {
            ch[co].set_do_update_flags();
        });
    }

    /// Port of `handle_labkeeper_death(co, cn)` from the original server sources.
    ///
    /// Special-case handling for laboratory/shop keepers:
    /// - Remove player mapping for the killer
    /// - Destroy labkeeper items and clear inventory
    /// - Free the character slot and perform lab transfer logic
    ///
    /// # Arguments
    /// * `co` - Labkeeper character id who died
    /// * `cn` - Killer id
    pub(crate) fn handle_labkeeper_death(&self, co: usize, cn: usize) {
        player::plr_map_remove(cn);

        // Destroy all items
        // TODO: Seems like we're getting rid of the items twice?
        God::destroy_items(co);
        Repository::with_characters_mut(|characters| {
            characters[co].citem = 0;
            characters[co].gold = 0;
            for z in 0..40 {
                characters[co].item[z] = 0;
            }
            for z in 0..20 {
                characters[co].worn[z] = 0;
            }
            characters[co].used = core::constants::USE_EMPTY;
        });

        self.use_labtransfer2(cn, co);
    }

    /// Port of `handle_item_drops(co, cc, wimp, cn)` from the original server sources.
    ///
    /// Determines and performs which items/money are dropped into the grave when a
    /// character dies. Behavior:
    /// - Gold may be dropped based on `wimp` chance
    /// - Inventory, carried, and worn items are considered for dropping or keeping
    /// - Respects `do_maygive` to determine whether an item can be transferred to killer
    /// - Active spells are always destroyed on death
    ///
    /// # Arguments
    /// * `co` - Original (dead) character id
    /// * `cc` - Clone/grave character id (where dropped items are carried)
    /// * `wimp` - Guardian angel / wimpy chance (0-255). Higher means less dropping
    /// * `cn` - Killer id
    pub(crate) fn handle_item_drops(&self, co: usize, cc: usize, wimp: i32, cn: usize) {
        use core::constants::*;

        // Handle gold
        Repository::with_characters_mut(|characters| {
            if characters[co].gold != 0 {
                let mut rng = rand::thread_rng();
                if wimp < rng.gen_range(0..100) {
                    characters[co].gold = 0;
                } else {
                    characters[cc].gold = 0;
                }
            }
        });

        // Handle inventory items
        for n in 0..40 {
            let item_idx = Repository::with_characters(|ch| ch[co].item[n]);
            if item_idx == 0 {
                continue;
            }

            // Check if item may be given
            if !self.do_maygive(cn, 0, item_idx as usize) {
                Repository::with_items_mut(|items| {
                    if (item_idx as usize) < items.len() {
                        items[item_idx as usize].used = USE_EMPTY;
                    }
                });
                Repository::with_characters_mut(|characters| {
                    characters[co].item[n] = 0;
                    characters[cc].item[n] = 0;
                });
                continue;
            }

            let mut rng = rand::thread_rng();
            if wimp <= rng.gen_range(0..100) {
                // Drop in grave
                Repository::with_characters_mut(|characters| {
                    characters[co].item[n] = 0;
                });
                Repository::with_items_mut(|items| {
                    if (item_idx as usize) < items.len() {
                        items[item_idx as usize].carried = cc as u16;

                        let item_template_to_print = items[item_idx as usize].temp;
                        log::info!(
                            "Dropped {} (t={}) in Grave",
                            items[item_idx as usize].get_name(),
                            item_template_to_print,
                        );
                    }
                });
            } else {
                // Player keeps it
                Repository::with_characters_mut(|characters| {
                    characters[cc].item[n] = 0;
                });
            }
        }

        // Handle carried item (citem)
        let citem = Repository::with_characters(|ch| ch[co].citem);
        if citem != 0 {
            if !self.do_maygive(cn, 0, citem as usize) {
                Repository::with_items_mut(|items| {
                    if (citem as usize) < items.len() {
                        items[citem as usize].used = USE_EMPTY;
                    }
                });
                Repository::with_characters_mut(|characters| {
                    characters[co].citem = 0;
                    characters[cc].citem = 0;
                });
            } else {
                let mut rng = rand::thread_rng();
                if wimp <= rng.gen_range(0..100) {
                    Repository::with_characters_mut(|characters| {
                        characters[co].citem = 0;
                    });
                    Repository::with_items_mut(|items| {
                        if (citem as usize) < items.len() {
                            items[citem as usize].carried = cc as u16;
                            let item_template_to_print = items[citem as usize].temp;
                            log::info!(
                                "Dropped {} (t={}) in Grave",
                                items[citem as usize].get_name(),
                                item_template_to_print,
                            );
                        }
                    });
                } else {
                    Repository::with_characters_mut(|characters| {
                        characters[cc].citem = 0;
                    });
                }
            }
        }

        // Handle worn items
        for n in 0..20 {
            let item_idx = Repository::with_characters(|ch| ch[co].worn[n]);
            if item_idx == 0 {
                continue;
            }

            if !self.do_maygive(cn, 0, item_idx as usize) {
                Repository::with_items_mut(|items| {
                    if (item_idx as usize) < items.len() {
                        items[item_idx as usize].used = USE_EMPTY;
                    }
                });
                Repository::with_characters_mut(|characters| {
                    characters[co].worn[n] = 0;
                    characters[cc].worn[n] = 0;
                });
                continue;
            }

            let mut rng = rand::thread_rng();
            if wimp <= rng.gen_range(0..100) {
                Repository::with_characters_mut(|characters| {
                    characters[co].worn[n] = 0;
                });
                Repository::with_items_mut(|items| {
                    if (item_idx as usize) < items.len() {
                        items[item_idx as usize].carried = cc as u16;
                        let item_template = items[item_idx as usize].temp;
                        log::info!(
                            "Dropped {} (t={}) in Grave",
                            items[item_idx as usize].get_name(),
                            item_template,
                        );
                    }
                });
            } else {
                Repository::with_characters_mut(|characters| {
                    characters[cc].worn[n] = 0;
                });
            }
        }

        // Handle active spells - always destroy
        for n in 0..20 {
            let spell_idx = Repository::with_characters(|ch| ch[co].spell[n]);
            if spell_idx != 0 {
                Repository::with_characters_mut(|characters| {
                    characters[co].spell[n] = 0;
                    characters[cc].spell[n] = 0;
                });
                Repository::with_items_mut(|items| {
                    if (spell_idx as usize) < items.len() {
                        items[spell_idx as usize].used = USE_EMPTY;
                    }
                });
            }
        }
    }

    /// Port of `apply_death_penalties(co)` from the original server sources.
    ///
    /// Applies permanent penalties to a character after death:
    /// - Decreases permanent hitpoints according to configured rules
    /// - Decreases permanent mana according to configured rules
    /// - Notifies the player and invokes internal lowering helpers
    ///
    /// # Arguments
    /// * `co` - Character id to apply permanent penalties to
    pub(crate) fn apply_death_penalties(&self, co: usize) {
        Repository::with_characters_mut(|characters| {
            // HP penalty
            let mut hp_tmp = characters[co].hp[0] / 10;
            if characters[co].hp[0] - hp_tmp < 50 {
                hp_tmp = characters[co].hp[0] - 50;
            }
            if hp_tmp > 0 {
                self.do_character_log(
                    co,
                    FontColor::Red,
                    &format!("You lost {} hitpoints permanently.\n", hp_tmp),
                );
                log::info!("Character {} lost {} permanent hitpoints.", co, hp_tmp);
                for _ in 0..hp_tmp {
                    self.do_lower_hp(co);
                }
            } else {
                self.do_character_log(
                    co,
                    FontColor::Red,
                    "You would have lost permanent hitpoints, but you're already at the minimum.\n",
                );
            }

            // TODO: Endurance penalty?

            // Mana penalty
            let mut mana_tmp = characters[co].mana[0] / 10;
            if characters[co].mana[0] - mana_tmp < 50 {
                mana_tmp = characters[co].mana[0] - 50;
            }
            if mana_tmp > 0 {
                self.do_character_log(
                    co,
                    FontColor::Red,
                    &format!("You lost {} mana permanently.\n", mana_tmp),
                );
                log::info!("Character {} lost {} permanent mana.", co, mana_tmp);
                for _ in 0..mana_tmp {
                    self.do_lower_mana(co);
                }
            } else {
                self.do_character_log(
                    co,
                    FontColor::Red,
                    "You would have lost permanent mana, but you're already at the minimum.\n",
                );
            }
        });
    }
}
