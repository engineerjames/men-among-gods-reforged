use core::constants::{CHD_CORPSEOWNER, CharacterFlags, USE_EMPTY};
use core::types::{Character, FontColor};
use core::{skills, traits};

use crate::effect::EffectManager;
use crate::god::God;
use crate::{helpers, player};

use crate::game_state::GameState;

/// Percentage of carried money lost on an eligible player death.
const PLAYER_DEATH_MONEY_LOSS_PERCENT: i64 = 25;
const _: () =
    assert!(PLAYER_DEATH_MONEY_LOSS_PERCENT >= 0 && PLAYER_DEATH_MONEY_LOSS_PERCENT <= 100);

/// Bit marking a cursor-held value as money rather than an item index.
const CURSOR_MONEY_FLAG: u32 = 0x8000_0000;

/// Mask containing the value of cursor-held money in silver.
const CURSOR_MONEY_VALUE_MASK: u32 = 0x7fff_ffff;

impl GameState {
    /// Removes the configured percentage of a player's carried money.
    ///
    /// Carried money includes purse gold and cursor-held money, but excludes
    /// the bank balance. The loss is rounded down to the nearest silver and
    /// is taken from the purse before cursor-held money.
    ///
    /// # Arguments
    ///
    /// * `cn` - Character id whose carried money is penalized.
    ///
    /// # Returns
    ///
    /// * Amount removed in silver.
    fn apply_player_death_money_loss(&mut self, cn: usize) -> i64 {
        let purse = i64::from(self.characters[cn].gold.max(0));
        let citem = self.characters[cn].citem;
        let cursor = if citem & CURSOR_MONEY_FLAG != 0 {
            i64::from(citem & CURSOR_MONEY_VALUE_MASK)
        } else {
            0
        };
        let loss = (purse + cursor) * PLAYER_DEATH_MONEY_LOSS_PERCENT / 100;

        if loss == 0 {
            log::info!("Player {} has no money, therefore none will be taken.", cn);
            return 0;
        }

        let purse_loss = loss.min(purse);
        self.characters[cn].gold -= purse_loss as i32;

        let cursor_loss = loss - purse_loss;
        if cursor_loss != 0 {
            let remaining = cursor - cursor_loss;
            self.characters[cn].citem = if remaining == 0 {
                0
            } else {
                CURSOR_MONEY_FLAG | remaining as u32
            };
        }

        self.characters[cn].set_do_update_flags();
        loss
    }

    /// Port of `do_character_killed(character_id, killer_id)` from the original
    /// server sources.
    ///
    /// Top-level handler invoked when a character dies. Responsibilities:
    /// - Send death notifications to nearby characters
    /// - Play appropriate death sound effects
    /// - Log the kill and update killer/player statistics
    /// - Apply alignment, luck and penalty changes for killers
    /// - Handle special-case followers and companion cleanup
    /// - Apply the player death-money penalty and schedule death effects
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
            i32::from(core::constants::NT_DIED),
            killer_id as i32,
            0,
            0,
            0,
        );

        // Contagion / Parasite: if the dying character was infected, the
        // infestation leaps to adjacent enemies sharing the caster's faction
        // enmity.
        self.spread_contagion_on_death(character_id);

        // Ice Stun: if the dying character was marked by empowered stun,
        // nearby enemies may be cut by the collapsing ice.
        self.trigger_ice_stun_burst_on_death(character_id);

        // Log the kill
        if killer_id != 0 {
            log::info!(
                "Character {} killed character {} ({})",
                killer_id,
                character_id,
                self.characters[character_id].get_name().to_owned()
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
            self.do_area_sound(character_id, 0, i32::from(co_x), i32::from(co_y), 17);
            Self::char_play_sound(self, character_id, 17, -150, 0);
        }
        // Hack for gargoyles (templates 375-381)
        else if (375..=381).contains(&co_temp) {
            self.do_area_sound(character_id, 0, i32::from(co_x), i32::from(co_y), 18);
            Self::char_play_sound(self, character_id, 18, -150, 0);
        }
        // Normal death sound
        else {
            let sound = co_sound + 2;
            self.do_area_sound(
                character_id,
                0,
                i32::from(co_x),
                i32::from(co_y),
                i32::from(sound),
            );
            Self::char_play_sound(self, character_id, i32::from(sound), -150, 0);
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
            let is_arena = map_flags & u64::from(core::constants::MF_ARENA) != 0;
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
                            i32::from(self.characters[killer_id].x),
                            i32::from(self.characters[killer_id].y),
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
                            crate::player::commands::resend_completion_data_for_character(
                                self, killer_id,
                            );
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
                    i32::from(cc_x),
                    i32::from(cc_y),
                    i32::from(core::constants::NT_SEEHIT),
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
                    self.characters[character_id].data[15] =
                        i32::from(self.characters[killer_id].temp);
                }
            } else {
                self.characters[character_id].data[15] = 0;
            }

            self.characters[character_id].data[16] = self.globals.mdday + self.globals.mdyear * 300;
            self.characters[character_id].data[17] =
                i32::from(co_x) + i32::from(co_y) * core::constants::SERVER_MAPX;

            self.handle_player_death(character_id, map_flags, force_save);
            if force_save {
                return;
            }
            corpse_id = 0;
        } else {
            // Handle NPC death
            let is_labkeeper =
                self.characters[character_id].flags & CharacterFlags::LabKeeper.bits() != 0;

            if is_labkeeper {
                self.globals.npcs_died += 1;

                self.handle_labkeeper_death(character_id, killer_id);
                return;
            }
            self.handle_npc_death(character_id, killer_id);

            corpse_id = character_id;
        }

        // Schedule respawn and show death animation

        let fn_idx = EffectManager::fx_add_effect(
            self,
            3,
            0,
            i32::from(co_x),
            i32::from(co_y),
            corpse_id as i32,
        );
        // Set data[3] = killer_id for the effect, if possible
        if let Some(fn_idx) = fn_idx {
            self.effects[fn_idx].data[3] = killer_id as u32;
        }
    }

    /// Handles player-specific death processing.
    ///
    /// Players retain their items and do not leave a grave. Eligible deaths
    /// remove a percentage of carried money before the player is returned to
    /// their temple. Arena, Guardian Angel, God, and forced-save deaths are
    /// exempt from the money loss.
    ///
    /// This also destroys active spell items, restores minimal health, and
    /// resets transient combat state.
    ///
    /// # Arguments
    ///
    /// * `co` - Character id of the dead player.
    /// * `map_flags` - Map flags at the death location.
    /// * `force_save` - Whether a deathtrap or similar effect saves the player.
    pub(crate) fn handle_player_death(&mut self, co: usize, map_flags: u64, force_save: bool) {
        let has_guardian_angel = self.characters[co].spell.iter().any(|&item_idx| {
            let item_idx = item_idx as usize;
            item_idx != 0
                && item_idx < self.items.len()
                && self.items[item_idx].temp == skills::SK_WIMPY as u16
        });
        let is_arena = map_flags & u64::from(core::constants::MF_ARENA) != 0;
        let is_god = self.characters[co].flags & CharacterFlags::God.bits() != 0;

        if !force_save && !is_arena && !has_guardian_angel && !is_god {
            let loss = self.apply_player_death_money_loss(co);
            if loss != 0 {
                self.do_character_log(
                    co,
                    FontColor::Red,
                    &format!(
                        "You lost {}G {}S from the money you were carrying.\n",
                        loss / 100,
                        loss % 100
                    ),
                );
                log::info!(
                    "Character {} lost {}G {}S on death.",
                    co,
                    loss / 100,
                    loss % 100
                );
            }
        } else if has_guardian_angel && !is_arena {
            self.do_character_log(
                co,
                FontColor::Yellow,
                "Sometimes a Guardian Angel is really helpful...\n",
            );
        }

        self.destroy_player_death_spells(co);

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

        self.characters[co].a_hp = 10000;
        self.characters[co].status = 0;
        self.characters[co].attack_cn = 0;
        self.characters[co].skill_nr = 0;
        self.characters[co].goto_x = 0;
        self.characters[co].use_nr = 0;
        self.characters[co].misc_action = 0;
        self.characters[co].stunned = 0;
        self.characters[co].retry = 0;
        self.characters[co].current_enemy = 0;
        self.characters[co].enemy.fill(0);

        player::commands::plr_reset_status(self, co);

        if force_save {
            self.do_character_log(
                co,
                FontColor::Red,
                "You feel a sudden force saving you from death! You have been spared, but something feels different...\n",
            );
        }

        self.characters[co].set_do_update_flags();
    }

    /// Destroys temporary spell items attached to a player at death.
    ///
    /// # Arguments
    ///
    /// * `co` - Character id whose active spells are cleared.
    fn destroy_player_death_spells(&mut self, co: usize) {
        for spell in &mut self.characters[co].spell {
            let item_idx = *spell as usize;
            *spell = 0;
            if item_idx != 0 && item_idx < self.items.len() {
                self.items[item_idx].used = USE_EMPTY;
            }
        }
    }

    /// Handles non-player character death processing.
    ///
    /// # Arguments
    ///
    /// * `co` - NPC character id that died.
    /// * `cn` - Character id credited with the kill, or zero.
    pub(crate) fn handle_npc_death(&mut self, co: usize, cn: usize) {
        self.globals.npcs_died += 1;

        player::commands::plr_reset_status(self, co);

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
                player::connection::player_exit(self, player_nr);
            }
        }

        log::info!("new npc body");

        let should_respawn = self.characters[co].flags & CharacterFlags::Respawn.bits() != 0;

        if should_respawn {
            self.characters[co].flags =
                CharacterFlags::Body.bits() | CharacterFlags::Respawn.bits();
        } else {
            self.characters[co].flags = CharacterFlags::Body.bits();
        }

        self.characters[co].a_hp = 0;

        // Set corpse owner (killer only mode vs all can loot)
        let cc = if cn != 0 && (self.characters[cn].flags & CharacterFlags::Player.bits() == 0) {
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
        player::map::plr_map_remove(self, co);

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

    /// On-death helper for the Parasite-family DoTs. If the dying character
    /// carries an active Parasite and/or Contagion spell-item, each of them
    /// spreads a fresh infection to up to four adjacent enemies
    /// (8-neighborhood). Each spread carries the original caster's identity so
    /// lifesteal continues to feed the source of the infection.
    ///
    /// # Arguments
    ///
    /// * `dying` - Character index of the host that is about to die.
    fn spread_contagion_on_death(&mut self, dying: usize) {
        if !Character::is_sane_character(dying) {
            return;
        }
        // Collect the active Parasite-family DoTs on the dying character. A host
        // carrying both spreads both, so at most one entry per spell type is
        // gathered here.
        let mut dots: Vec<(u16, usize, i32)> = Vec::with_capacity(2);
        for n in 0..20 {
            let in_idx = self.characters[dying].spell[n] as usize;
            if in_idx == 0 {
                continue;
            }
            let temp = self.items[in_idx].temp;
            if (temp != skills::SK_CONTAGION as u16 && temp != skills::SK_PARASITE as u16)
                || self.items[in_idx].active == 0
            {
                continue;
            }
            if dots.iter().any(|&(seen, _, _)| seen == temp) {
                continue;
            }
            let caster = self.items[in_idx].data[0] as usize;
            if !Character::is_sane_character(caster) {
                continue;
            }
            dots.push((temp, caster, self.items[in_idx].power as i32));
        }
        if dots.is_empty() {
            return;
        }

        let dx0 = i32::from(self.characters[dying].x);
        let dy0 = i32::from(self.characters[dying].y);

        for (dot_temp, caster, dot_power) in dots {
            let is_contagion = dot_temp == skills::SK_CONTAGION as u16;
            let (duration, spell_name, spread_message) = if is_contagion {
                (
                    core::constants::TICKS * 60 * 8,
                    &b"Contagion"[..],
                    "The contagion spreads to you!\n",
                )
            } else {
                (
                    core::constants::TICKS * 8,
                    &b"Parasite"[..],
                    "The parasites burrow into you!\n",
                )
            };

            let mut spread = 0;
            'spread: for dy in -1..=1i32 {
                for dx in -1..=1i32 {
                    if dx == 0 && dy == 0 {
                        continue;
                    }
                    let nx = dx0 + dx;
                    let ny = dy0 + dy;
                    if nx < 0
                        || ny < 0
                        || nx >= core::constants::SERVER_MAPX
                        || ny >= core::constants::SERVER_MAPY
                    {
                        continue;
                    }
                    let m = (nx + ny * core::constants::SERVER_MAPX) as usize;
                    let neighbor = self.map[m].ch as usize;
                    if neighbor == 0 || neighbor == caster {
                        continue;
                    }
                    if !Character::is_sane_character(neighbor) {
                        continue;
                    }
                    if !self.may_attack_msg(caster, neighbor, false) {
                        continue;
                    }
                    if crate::driver::skill::apply_parasitic_dot(
                        self, caster, neighbor, dot_power, dot_temp, duration, spell_name,
                    ) {
                        spread += 1;
                        self.do_character_log(neighbor, FontColor::Green, spread_message);
                    }
                    if spread >= 4 {
                        break 'spread;
                    }
                }
            }
        }
    }

    /// On-death helper for Ice Stun. If the dying character carries an active
    /// Ice Stun marker, it has a 25% chance to damage enemies in the adjacent
    /// 3x3 area. The marker stores the original caster in `data[0]` so attack
    /// legality and kill credit remain tied to the source of the stun.
    ///
    /// # Arguments
    ///
    /// * `dying` - Character index of the host that is about to die.
    fn trigger_ice_stun_burst_on_death(&mut self, dying: usize) {
        if !Character::is_sane_character(dying) {
            return;
        }

        let mut marker_idx = 0usize;
        let mut caster = 0usize;
        let mut power = 0i32;
        for n in 0..20 {
            let in_idx = self.characters[dying].spell[n] as usize;
            if in_idx == 0 {
                continue;
            }
            if self.items[in_idx].temp == skills::SK_ICE_STUN as u16
                && self.items[in_idx].active > 0
            {
                marker_idx = in_idx;
                caster = self.items[in_idx].data[0] as usize;
                power = self.items[in_idx].power as i32;
                break;
            }
        }
        if marker_idx == 0 || !Character::is_sane_character(caster) {
            return;
        }

        self.items[marker_idx].active = 0;
        if helpers::random_mod(100) >= 75 {
            return;
        }

        let dx0 = i32::from(self.characters[dying].x);
        let dy0 = i32::from(self.characters[dying].y);
        let damage = ((power * 3) / 2).max(1);
        let damage_unit = damage * 1000;

        for dy in -1..=1i32 {
            for dx in -1..=1i32 {
                if dx == 0 && dy == 0 {
                    continue;
                }
                let nx = dx0 + dx;
                let ny = dy0 + dy;
                if nx < 0
                    || ny < 0
                    || nx >= core::constants::SERVER_MAPX
                    || ny >= core::constants::SERVER_MAPY
                {
                    continue;
                }

                let map_idx = (nx + ny * core::constants::SERVER_MAPX) as usize;
                let neighbor = self.map[map_idx].ch as usize;
                if neighbor == 0 || neighbor == dying || neighbor == caster {
                    continue;
                }
                if !Character::is_sane_character(neighbor) {
                    continue;
                }
                if self.characters[neighbor].flags & CharacterFlags::Body.bits() != 0 {
                    continue;
                }
                if !self.may_attack_msg(caster, neighbor, false) {
                    continue;
                }

                self.remember_pvp(caster, neighbor);
                self.characters[neighbor].a_hp -= damage_unit;
                self.do_character_log(
                    neighbor,
                    FontColor::Green,
                    "Shattering ice cuts into you!\n",
                );
                EffectManager::fx_add_effect(
                    self,
                    5,
                    0,
                    i32::from(self.characters[neighbor].x),
                    i32::from(self.characters[neighbor].y),
                    0,
                );

                if self.characters[neighbor].a_hp < 500 {
                    self.characters[neighbor].a_hp = 500;
                    self.do_character_killed(neighbor, caster, false);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::{add_test_player, with_test_gs};
    use core::constants::{MAXCHARS, MF_ARENA, USE_ACTIVE};

    fn prepare_player_death(gs: &mut GameState) -> usize {
        let (cn, _) = add_test_player(gs);
        gs.characters[cn].temple_x = 20;
        gs.characters[cn].temple_y = 20;
        let map_index = 10 + 10 * core::constants::SERVER_MAPX as usize;
        gs.map[map_index].ch = cn as u32;
        cn
    }

    #[test]
    fn death_money_loss_uses_combined_carried_money_and_rounds_down() {
        with_test_gs(|gs| {
            let (cn, _) = add_test_player(gs);
            gs.characters[cn].gold = 901;
            gs.characters[cn].citem = CURSOR_MONEY_FLAG | 99;

            let loss = gs.apply_player_death_money_loss(cn);

            assert_eq!(loss, 250);
            assert_eq!(gs.characters[cn].gold, 651);
            assert_eq!(gs.characters[cn].citem, CURSOR_MONEY_FLAG | 99);
        });
    }

    #[test]
    fn death_money_loss_uses_cursor_money_after_purse() {
        with_test_gs(|gs| {
            let (cn, _) = add_test_player(gs);
            gs.characters[cn].gold = 5;
            gs.characters[cn].citem = CURSOR_MONEY_FLAG | 995;

            let loss = gs.apply_player_death_money_loss(cn);

            assert_eq!(loss, 250);
            assert_eq!(gs.characters[cn].gold, 0);
            assert_eq!(gs.characters[cn].citem, CURSOR_MONEY_FLAG | 750);
        });
    }

    #[test]
    fn death_money_loss_preserves_small_balances_and_ordinary_cursor_items() {
        with_test_gs(|gs| {
            let (cn, _) = add_test_player(gs);
            gs.characters[cn].gold = 9;
            gs.characters[cn].citem = 42;

            let loss = gs.apply_player_death_money_loss(cn);

            assert_eq!(loss, 2);
            assert_eq!(gs.characters[cn].gold, 7);
            assert_eq!(gs.characters[cn].citem, 42);
        });
    }

    #[test]
    fn death_money_loss_handles_maximum_combined_value() {
        with_test_gs(|gs| {
            let (cn, _) = add_test_player(gs);
            gs.characters[cn].gold = i32::MAX;
            gs.characters[cn].citem = CURSOR_MONEY_FLAG | i32::MAX as u32;

            let loss = gs.apply_player_death_money_loss(cn);

            assert_eq!(loss, 1_073_741_823);
            assert_eq!(gs.characters[cn].gold, 1_073_741_824);
            assert_eq!(gs.characters[cn].citem, CURSOR_MONEY_FLAG | i32::MAX as u32);
        });
    }

    #[test]
    fn player_death_loses_money_but_retains_items_bank_and_permanent_stats() {
        with_test_gs(|gs| {
            let cn = prepare_player_death(gs);
            gs.characters[cn].gold = 1_000;
            gs.characters[cn].citem = CURSOR_MONEY_FLAG | 100;
            gs.characters[cn].data[13] = 7_777;
            gs.characters[cn].item[0] = 41;
            gs.characters[cn].worn[0] = 42;
            gs.characters[cn].hp[0] = 100;
            gs.characters[cn].mana[0] = 80;

            gs.handle_player_death(cn, 0, false);

            assert_eq!(gs.characters[cn].gold, 725);
            assert_eq!(gs.characters[cn].citem, CURSOR_MONEY_FLAG | 100);
            assert_eq!(gs.characters[cn].data[13], 7_777);
            assert_eq!(gs.characters[cn].item[0], 41);
            assert_eq!(gs.characters[cn].worn[0], 42);
            assert_eq!(gs.characters[cn].hp[0], 100);
            assert_eq!(gs.characters[cn].mana[0], 80);
            assert_eq!(gs.characters[cn].a_hp, 10_000);
        });
    }

    #[test]
    fn player_death_money_loss_honors_all_exemptions() {
        with_test_gs(|gs| {
            for case in 0..4 {
                let cn = prepare_player_death(gs);
                gs.characters[cn].gold = 1_000;
                gs.characters[cn].flags = CharacterFlags::Player.bits();
                gs.characters[cn].spell.fill(0);

                let mut map_flags = 0;
                let mut force_save = false;
                match case {
                    0 => map_flags = u64::from(MF_ARENA),
                    1 => {
                        gs.items[50].used = USE_ACTIVE;
                        gs.items[50].temp = skills::SK_WIMPY as u16;
                        gs.characters[cn].spell[0] = 50;
                    }
                    2 => gs.characters[cn].flags |= CharacterFlags::God.bits(),
                    3 => force_save = true,
                    _ => unreachable!(),
                }

                gs.handle_player_death(cn, map_flags, force_save);

                assert_eq!(gs.characters[cn].gold, 1_000, "exemption case {case}");
                if case == 1 {
                    assert_eq!(gs.characters[cn].spell[0], 0);
                    assert_eq!(gs.items[50].used, USE_EMPTY);
                }
            }
        });
    }

    #[test]
    fn player_death_does_not_require_a_free_character_slot() {
        with_test_gs(|gs| {
            let cn = prepare_player_death(gs);
            for character in &mut gs.characters[1..MAXCHARS] {
                character.used = USE_ACTIVE;
            }
            gs.characters[cn].gold = 1_000;
            gs.characters[cn].item[0] = 41;

            gs.handle_player_death(cn, 0, false);

            assert_eq!(gs.characters[cn].gold, 750);
            assert_eq!(gs.characters[cn].item[0], 41);
            assert_eq!(gs.characters[cn].a_hp, 10_000);
        });
    }
}
