use core::constants::{CharacterFlags, CHD_CORPSEOWNER, MAXCHARS, USE_EMPTY};
use core::types::{Character, FontColor};
use core::{skills, traits};

use crate::effect::EffectManager;
use crate::god::God;
use crate::{helpers, player};

use crate::game_state::GameState;

impl GameState {
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
    /// * `force_save` - Whether to force the character to be saved from his death (used for deathtraps)
    pub(crate) fn do_character_killed(
        &mut self,
        character_id: usize,
        killer_id: usize,
        force_save: bool,
    ) {
        if !Character::is_sane_character(character_id) {
            log::warn!("do_character_killed: invalid character_id {}", character_id);
            return;
        }

        let killer_id = if killer_id != 0 && Character::is_sane_character(killer_id) {
            killer_id
        } else {
            0
        };

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
                self.characters[character_id].get_name().to_string()
            );
        } else {
            log::info!("Character {} died", character_id);
        }

        // Get map flags for both characters
        let (co_x, co_y, co_temp, co_sound) = {
            let co = &mut self.characters[character_id];
            (co.x, co.y, co.temp, co.sound)
        };

        let mut map_flags = {
            let idx = co_x as usize + co_y as usize * core::constants::SERVER_MAPX as usize;
            self.map[idx].flags
        };

        if killer_id != 0 {
            let idx = self.characters[killer_id].x as usize
                + self.characters[killer_id].y as usize * core::constants::SERVER_MAPX as usize;
            let cn_flags = self.map[idx].flags;
            map_flags |= cn_flags;
        }

        // Play death sound effects
        // Hack for grolms (templates 364-374)
        if (364..=374).contains(&co_temp) {
            self.do_area_sound(character_id, 0, co_x as i32, co_y as i32, 17);
            Self::char_play_sound(self, character_id, 17, -150, 0);
        }
        // Hack for gargoyles (templates 375-381)
        else if (375..=381).contains(&co_temp) {
            self.do_area_sound(character_id, 0, co_x as i32, co_y as i32, 18);
            Self::char_play_sound(self, character_id, 18, -150, 0);
        }
        // Normal death sound
        else {
            let sound = co_sound + 2;
            self.do_area_sound(character_id, 0, co_x as i32, co_y as i32, sound as i32);
            Self::char_play_sound(self, character_id, sound as i32, -150, 0);
        }

        // Cleanup for ghost companions
        if co_temp == core::constants::CT_COMPANION as u16 {
            let cc = self.characters[character_id].data[63] as usize;
            if Character::is_sane_character(cc)
                && self.characters[cc].data[64] == character_id as i32
            {
                self.characters[cc].data[64] = 0;
            }
            self.characters[character_id].data[63] = 0;
        }

        // A player killed someone or something
        if killer_id != 0 && killer_id != character_id {
            let is_killer_player =
                self.characters[killer_id].flags & CharacterFlags::Player.bits() != 0;
            let is_arena = map_flags & core::constants::MF_ARENA as u64 != 0;
            let co_alignment = self.characters[character_id].alignment;
            let co_temp = self.characters[character_id].temp;
            let co_is_player =
                self.characters[character_id].flags & CharacterFlags::Player.bits() != 0;

            if is_killer_player && !is_arena {
                // Adjust alignment
                self.characters[killer_id].alignment -= co_alignment / 50;

                self.characters[killer_id].alignment =
                    self.characters[killer_id].alignment.clamp(-7500, 7500);

                // Check for killing priests (becoming purple)
                if co_temp == core::constants::CT_PRIEST as u16 {
                    let killer_kindred = self.characters[killer_id].kindred;

                    if killer_kindred as u32 & traits::KIN_PURPLE != 0 {
                        self.do_character_log(
                            killer_id,
                            core::types::FontColor::Yellow,
                            "Ahh, that felt good!\n",
                        );
                    } else {
                        self.characters[killer_id].data[67] = self.globals.ticker;
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

                        EffectManager::fx_add_effect(
                            self,
                            6,
                            0,
                            self.characters[killer_id].x as i32,
                            self.characters[killer_id].y as i32,
                            0,
                        );
                    }
                }

                // Check for killing shopkeepers & questgivers (alignment 10000)
                if !co_is_player && co_alignment == 10000 {
                    self.do_character_log(
                        killer_id,
                        core::types::FontColor::Red,
                        "You feel a god look into your soul. He seems to be angry.\n",
                    );

                    self.characters[killer_id].data[40] += 1;
                    let penalty = if self.characters[killer_id].data[40] < 50 {
                        -self.characters[killer_id].data[40] * 100
                    } else {
                        -5000
                    };
                    self.characters[killer_id].luck += penalty;

                    let luck_to_print = self.characters[killer_id].luck;
                    log::info!(
                        "Reduced luck by {} to {} for killing {} (t={})",
                        penalty,
                        luck_to_print,
                        self.characters[character_id].get_name(),
                        co_temp
                    );
                }

                // Update statistics
                let r1: u32 =
                    core::ranks::points2rank(self.characters[killer_id].points_tot as u32);
                let r2: u32 =
                    core::ranks::points2rank(self.characters[character_id].points_tot as u32);

                if (r1 as i32 - r2 as i32).abs() < 3 {
                    // Approximately own rank
                    self.characters[killer_id].data[24] += 1; // overall counter
                    if self.characters[character_id].data[42] == 27 {
                        self.characters[killer_id].data[27] += 1; // black stronghold counter
                    }
                } else if r2 > r1 {
                    // Above own rank
                    self.characters[killer_id].data[25] += 1;
                    if self.characters[character_id].data[42] == 27 {
                        self.characters[killer_id].data[28] += 1;
                    }
                } else {
                    // Below own rank
                    self.characters[killer_id].data[23] += 1;
                    if self.characters[character_id].data[42] == 27 {
                        self.characters[killer_id].data[26] += 1;
                    }
                }

                if co_is_player {
                    self.characters[killer_id].data[29] += 1;
                } else {
                    // Check for first kill of this monster class
                    let monster_class = self.characters[character_id].monster_class;
                    if monster_class != 0 {
                        // killed_class: returns true if already killed, false if first kill
                        if !helpers::killed_class(self, killer_id, monster_class) {
                            let class_name = helpers::get_class_name(monster_class);
                            self.do_character_log(
                                killer_id,
                                core::types::FontColor::Yellow,
                                &format!("You just killed your first {}. Good job.\n", class_name),
                            );
                            let score = self.do_char_score(character_id) * 25;
                            self.do_give_exp(killer_id, score, 0, -1);
                        }
                    }
                }
            }

            // A follower (gargoyle, ghost companion) killed someone
            let follower_owner =
                if self.characters[killer_id].flags & CharacterFlags::Player.bits() == 0 {
                    let cc = self.characters[killer_id].data[63] as usize;
                    if cc != 0 && Character::is_sane_character(cc) {
                        Some(cc)
                    } else {
                        None
                    }
                } else {
                    None
                };

            if let Some(cc) = follower_owner {
                let is_owner_player =
                    self.characters[cc].flags & CharacterFlags::Player.bits() != 0;

                if is_owner_player && !co_is_player && co_alignment == 10000 {
                    self.do_character_log(cc, core::types::FontColor::Red,
                        "A goddess is about to turn your follower into a frog, but notices that you are responsible. You feel her do something to you. Nothing good, that's for sure.\n");

                    self.characters[cc].data[40] += 1;
                    let penalty = if self.characters[cc].data[40] < 50 {
                        -self.characters[cc].data[40] * 100
                    } else {
                        -5000
                    };
                    self.characters[cc].luck += penalty;

                    let luck_to_print = self.characters[cc].luck;
                    log::info!(
                        "Reduced luck by {} to {} for follower killing {} (t={})",
                        penalty,
                        luck_to_print,
                        self.characters[character_id].get_name(),
                        co_temp
                    );
                }

                // Notify area about the kill
                let (cc_x, cc_y) = (self.characters[cc].x, self.characters[cc].y);
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

        self.remove_enemy(character_id);

        // Handle player death
        let is_player = self.characters[character_id].flags & CharacterFlags::Player.bits() != 0;

        let corpse_id: usize;
        if is_player {
            // Update player death statistics
            self.globals.players_died += 1;

            // Adjust luck if negative
            if self.characters[character_id].luck < 0 {
                self.characters[character_id].luck =
                    std::cmp::min(0, self.characters[character_id].luck + 10);
            }

            // Set killed by message
            self.characters[character_id].data[14] += 1;
            if killer_id != 0 {
                let is_killer_player =
                    self.characters[killer_id].flags & CharacterFlags::Player.bits() != 0;
                if is_killer_player {
                    self.characters[character_id].data[15] = killer_id as i32 | 0x10000;
                } else {
                    self.characters[character_id].data[15] = self.characters[killer_id].temp as i32;
                }
            } else {
                self.characters[character_id].data[15] = 0;
            }

            self.characters[character_id].data[16] = self.globals.mdday + self.globals.mdyear * 300;
            self.characters[character_id].data[17] =
                co_x as i32 + co_y as i32 * core::constants::SERVER_MAPX;

            corpse_id = self.handle_player_death(character_id, killer_id, map_flags, force_save);
            if force_save {
                return;
            }
        } else {
            // Handle NPC death
            let is_labkeeper =
                self.characters[character_id].flags & CharacterFlags::LabKeeper.bits() != 0;

            if is_labkeeper {
                self.globals.npcs_died += 1;

                self.handle_labkeeper_death(character_id, killer_id);
                return;
            } else {
                self.handle_npc_death(character_id, killer_id);
            }

            corpse_id = character_id;
        }

        // Schedule respawn and show death animation

        let fn_idx =
            EffectManager::fx_add_effect(self, 3, 0, co_x as i32, co_y as i32, corpse_id as i32);
        // Set data[3] = killer_id for the effect, if possible
        if fn_idx.unwrap() < self.effects.len() {
            self.effects[fn_idx.unwrap()].data[3] = killer_id as u32;
        }
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
    /// * `force_save` - Whether to force the character to be saved from his death (used for deathtraps)
    pub(crate) fn handle_player_death(
        &mut self,
        co: usize,
        cn: usize,
        map_flags: u64,
        force_save: bool,
    ) -> usize {
        // Remember template if we're to respawn this character
        // TODO: Re-evaluate if we need to do anything here.

        // Check for Guardian Angel (Wimpy skill)
        let wimp = {
            let mut wimp_power = 0;
            for n in 0..20 {
                let item_idx = self.characters[co].spell[n] as usize;
                if item_idx != 0 {
                    let power_to_print = self.items[item_idx].power;
                    if item_idx < self.items.len() {
                        log::info!(
                            "spell active: {}, power of {}",
                            self.items[item_idx].get_name(),
                            power_to_print
                        );
                        if self.items[item_idx].temp == skills::SK_WIMPY as u16 {
                            wimp_power = self.items[item_idx].power / 2;
                        }
                    }
                }
            }
            wimp_power
        };

        let wimp = if map_flags & core::constants::MF_ARENA as u64 != 0 {
            205
        } else {
            wimp
        };

        // Find free character slot for body/grave
        let cc = (1..MAXCHARS).find(|&cc| self.characters[cc].used == core::constants::USE_EMPTY);

        let Some(cc) = cc else {
            log::error!(
                "Could not clone character {} for grave, all char slots full!",
                co
            );
            return co;
        };

        // Clone character to create grave
        self.characters[cc] = self.characters[co];

        // Drop items and money based on wimp chance
        self.handle_item_drops(co, cc, wimp as i32, cn, force_save);

        if force_save {
            let (cc_x, cc_y) = (self.characters[cc].x, self.characters[cc].y);
            let idx = cc_x as usize + cc_y as usize * core::constants::SERVER_MAPX as usize;
            if idx < self.map.len() {
                if self.map[idx].ch == cc as u32 {
                    self.map[idx].ch = 0;
                }
                if self.map[idx].to_ch == cc as u32 {
                    self.map[idx].to_ch = 0;
                }
            }
            self.characters[cc].used = USE_EMPTY;
            self.characters[cc].player = 0;
            self.characters[cc].flags = 0;
        }

        // Move player to temple
        let (temple_x, temple_y, cur_x, cur_y) = (
            self.characters[co].temple_x,
            self.characters[co].temple_y,
            self.characters[co].x,
            self.characters[co].y,
        );

        if cur_x as u16 == temple_x && cur_y as u16 == temple_y {
            God::transfer_char(self, co, (temple_x + 4) as usize, (temple_y + 4) as usize);
        } else {
            God::transfer_char(self, co, temple_x as usize, temple_y as usize);
        }

        // Resurrect player with 10 HP
        self.characters[co].a_hp = 10000; // 10 HP (stored as 10000)
        self.characters[co].status = 0;
        self.characters[co].attack_cn = 0;
        self.characters[co].skill_nr = 0;
        self.characters[co].goto_x = 0;
        self.characters[co].use_nr = 0;
        self.characters[co].misc_action = 0;
        self.characters[co].stunned = 0;
        self.characters[co].retry = 0;
        self.characters[co].current_enemy = 0;
        for m in 0..4 {
            self.characters[co].enemy[m] = 0;
        }

        player::plr_reset_status(self, co);

        // Apply permanent stat loss if not a god and no guardian angel
        let is_god = self.characters[co].flags & CharacterFlags::God.bits() != 0;

        if !is_god && wimp == 0 && !force_save {
            self.apply_death_penalties(co);
        } else if wimp != 0 && map_flags & core::constants::MF_ARENA as u64 == 0 {
            self.do_character_log(
                co,
                core::types::FontColor::Yellow,
                "Sometimes a Guardian Angel is really helpful...\n",
            );
        }

        if force_save {
            self.do_character_log(
                co,
                core::types::FontColor::Red,
                "You feel a sudden force saving you from death! You have been spared, but something feels different...\n",
            );
        }

        // Update player character
        self.characters[co].set_do_update_flags();

        // Setup the grave (body) - but only if we didn't force the save
        if !force_save {
            player::plr_reset_status(self, cc);

            self.characters[cc].player = 0;
            self.characters[cc].flags = CharacterFlags::Body.bits();
            self.characters[cc].a_hp = 0;
            self.characters[cc].data[core::constants::CHD_CORPSEOWNER] = co as i32;
            self.characters[cc].data[99] = 1;
            self.characters[cc].data[98] = 0;

            self.characters[cc].attack_cn = 0;
            self.characters[cc].skill_nr = 0;
            self.characters[cc].goto_x = 0;
            self.characters[cc].use_nr = 0;
            self.characters[cc].misc_action = 0;
            self.characters[cc].stunned = 0;
            self.characters[cc].retry = 0;
            self.characters[cc].current_enemy = 0;
            for m in 0..4 {
                self.characters[cc].enemy[m] = 0;
            }

            // Update grave character
            self.characters[cc].set_do_update_flags();

            player::plr_map_set(self, cc);

            // After player death, `co` is reassigned to `cc` for corpse effects.
            cc
        } else {
            co
        }
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
    pub(crate) fn handle_npc_death(&mut self, co: usize, cn: usize) {
        // Update NPC death statistics
        self.globals.npcs_died += 1;

        player::plr_reset_status(self, co);

        // Check for USURP flag (player controlling NPC)
        let usurp_info = if self.characters[co].flags & CharacterFlags::Usurp.bits() != 0 {
            Some((
                self.characters[co].player as usize,
                self.characters[co].data[97] as usize,
            ))
        } else {
            None
        };

        if let Some((player_nr, c2)) = usurp_info {
            if Character::is_sane_character(c2) {
                self.characters[c2].player = player_nr as i32;
                self.players[player_nr].usnr = c2;
                self.characters[c2].flags &= !CharacterFlags::ComputerControlledPlayer.bits();
            } else {
                player::player_exit(self, player_nr);
            }
        }

        log::info!("new npc body");

        // Convert to body
        let should_respawn = self.characters[co].flags & CharacterFlags::Respawn.bits() != 0;

        if should_respawn {
            self.characters[co].flags =
                CharacterFlags::Body.bits() | CharacterFlags::Respawn.bits();
        } else {
            self.characters[co].flags = CharacterFlags::Body.bits();
        }

        self.characters[co].a_hp = 0;

        // Set corpse owner (killer only mode vs all can loot)
        let cc = if cn != 0 && !(self.characters[cn].flags & CharacterFlags::Player.bits() != 0) {
            let cc = self.characters[cn].data[63] as usize;
            if cc != 0 && (self.characters[cc].flags & CharacterFlags::Player.bits() != 0) {
                Some(cc)
            } else {
                None
            }
        } else {
            None
        };

        if let Some(cc) = cc {
            self.characters[co].data[CHD_CORPSEOWNER] = cc as i32;
        } else if cn != 0 {
            let is_cn_player = self.characters[cn].flags & CharacterFlags::Player.bits() != 0;
            if is_cn_player {
                self.characters[co].data[CHD_CORPSEOWNER] = cn as i32;
            } else {
                self.characters[co].data[CHD_CORPSEOWNER] = 0;
            }
        } else {
            self.characters[co].data[CHD_CORPSEOWNER] = 0;
        }

        self.characters[co].data[99] = 0;
        self.characters[co].data[98] = 0;

        self.characters[co].attack_cn = 0;
        self.characters[co].skill_nr = 0;
        self.characters[co].goto_x = 0;
        self.characters[co].use_nr = 0;
        self.characters[co].misc_action = 0;
        self.characters[co].stunned = 0;
        self.characters[co].retry = 0;
        self.characters[co].current_enemy = 0;
        for m in 0..4 {
            self.characters[co].enemy[m] = 0;
        }

        // Destroy active spells
        for n in 0..20 {
            if self.characters[co].spell[n] != 0 {
                let item_idx = self.characters[co].spell[n] as usize;
                self.characters[co].spell[n] = 0;
                if item_idx < self.items.len() {
                    self.items[item_idx].used = core::constants::USE_EMPTY;
                }
            }
        }

        // If killer is a player, check for special items in grave
        let is_cn_player = if cn != 0 {
            Character::is_sane_character(cn)
                && self.characters[cn].flags & CharacterFlags::Player.bits() != 0
        } else {
            false
        };

        if is_cn_player {
            self.do_ransack_corpse(
                cn,
                co,
                "You notice %s tumble into the grave of your victim.\n",
            );
        }

        // Update character
        self.characters[co].set_do_update_flags();
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
    pub(crate) fn handle_labkeeper_death(&mut self, co: usize, cn: usize) {
        player::plr_map_remove(self, co);

        // Destroy all items
        // TODO: Seems like we're getting rid of the items twice?
        God::destroy_items(self, co);
        self.characters[co].citem = 0;
        self.characters[co].gold = 0;
        for z in 0..40 {
            self.characters[co].item[z] = 0;
        }
        for z in 0..20 {
            self.characters[co].worn[z] = 0;
        }
        self.characters[co].used = core::constants::USE_EMPTY;

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
    /// * `force_save` - Whether to force the character to be saved from his death (used for deathtraps)
    pub(crate) fn handle_item_drops(
        &mut self,
        co: usize,
        cc: usize,
        wimp: i32,
        cn: usize,
        force_save: bool,
    ) {
        if force_save {
            // If we're forcing a save (e.g. deathtrap), don't drop anything but still destroy spells
            // Handle active spells - always destroy
            for n in 0..20 {
                let spell_idx = self.characters[co].spell[n];
                if spell_idx != 0 {
                    self.characters[co].spell[n] = 0;
                    self.characters[cc].spell[n] = 0;
                    if (spell_idx as usize) < self.items.len() {
                        self.items[spell_idx as usize].used = USE_EMPTY;
                    }
                }
            }
            return;
        }

        // Handle gold
        if self.characters[co].gold != 0 {
            if wimp < helpers::random_mod_i32(100) {
                self.characters[co].gold = 0;
            } else {
                self.characters[cc].gold = 0;
            }
        }

        // Handle inventory items
        for n in 0..40 {
            let item_idx = self.characters[co].item[n];
            if item_idx == 0 {
                continue;
            }

            // Check if item may be given
            if !self.do_maygive(cn, 0, item_idx as usize) {
                if (item_idx as usize) < self.items.len() {
                    self.items[item_idx as usize].used = USE_EMPTY;
                }
                self.characters[co].item[n] = 0;
                self.characters[cc].item[n] = 0;
                continue;
            }

            if wimp <= helpers::random_mod_i32(100) {
                // Drop in grave
                self.characters[co].item[n] = 0;
                if (item_idx as usize) < self.items.len() {
                    self.items[item_idx as usize].carried = cc as u16;

                    let item_template_to_print = self.items[item_idx as usize].temp;
                    log::info!(
                        "Dropped {} (t={}) in Grave",
                        self.items[item_idx as usize].get_name(),
                        item_template_to_print,
                    );
                }
            } else {
                // Player keeps it
                self.characters[cc].item[n] = 0;
            }
        }

        // Handle carried item (citem)
        let citem = self.characters[co].citem;
        if citem != 0 {
            if !self.do_maygive(cn, 0, citem as usize) {
                if (citem as usize) < self.items.len() {
                    self.items[citem as usize].used = USE_EMPTY;
                }
                self.characters[co].citem = 0;
                self.characters[cc].citem = 0;
            } else {
                if wimp <= helpers::random_mod_i32(100) {
                    self.characters[co].citem = 0;
                    if (citem as usize) < self.items.len() {
                        self.items[citem as usize].carried = cc as u16;
                        let item_template_to_print = self.items[citem as usize].temp;
                        log::info!(
                            "Dropped {} (t={}) in Grave",
                            self.items[citem as usize].get_name(),
                            item_template_to_print,
                        );
                    }
                } else {
                    self.characters[cc].citem = 0;
                }
            }
        }

        // Handle worn items
        for n in 0..20 {
            let item_idx = self.characters[co].worn[n];
            if item_idx == 0 {
                continue;
            }

            if !self.do_maygive(cn, 0, item_idx as usize) {
                if (item_idx as usize) < self.items.len() {
                    self.items[item_idx as usize].used = USE_EMPTY;
                }
                self.characters[co].worn[n] = 0;
                self.characters[cc].worn[n] = 0;
                continue;
            }

            if wimp <= helpers::random_mod_i32(100) {
                self.characters[co].worn[n] = 0;
                if (item_idx as usize) < self.items.len() {
                    self.items[item_idx as usize].carried = cc as u16;
                    let item_template = self.items[item_idx as usize].temp;
                    log::info!(
                        "Dropped {} (t={}) in Grave",
                        self.items[item_idx as usize].get_name(),
                        item_template,
                    );
                }
            } else {
                self.characters[cc].worn[n] = 0;
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
    pub(crate) fn apply_death_penalties(&mut self, co: usize) {
        if !Character::is_sane_character(co) {
            log::warn!("apply_death_penalties: invalid character {}", co);
            return;
        }

        let perm_hp = self.characters[co].hp[0] as i32;
        let perm_mana = self.characters[co].mana[0] as i32;

        // HP penalty
        let mut hp_tmp = perm_hp / 10;
        if perm_hp - hp_tmp < 50 {
            hp_tmp = perm_hp - 50;
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
        let mut mana_tmp = perm_mana / 10;
        if perm_mana - mana_tmp < 50 {
            mana_tmp = perm_mana - 50;
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
    }
}
