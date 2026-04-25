use core::constants::{CharacterFlags, ItemFlags};
use core::string_operations::c_string_to_str;
use core::talent_trees::{is_talent_slot_spent, mercenary};
use core::types::FontColor;
use core::{skills, traits};

use crate::driver;
use crate::game_state::GameState;
use crate::helpers;

const MERCENARY_BASE_DODGE_PERCENT: i32 = 10;
const MERCENARY_DODGE_TALENT_PERCENT: i32 = 5;
const MERCENARY_MAX_DODGE_CHANCE: i32 = 100;

impl GameState {
    /// Port of `get_fight_skill(int cn)` from `svr_do.cpp`
    ///
    /// Calculate effective fighting skill based on character's skills and attributes.
    ///
    /// # Arguments
    ///
    /// * `cn` - Character index whose fight skill should be read.
    ///
    /// # Returns
    ///
    /// * Effective weapon/fight skill value used by melee combat resolution.
    pub(crate) fn get_fight_skill(&mut self, cn: usize) -> i32 {
        self.characters[cn].skill[skills::SK_WEAPON][5] as i32
    }

    /// Calculates the physical dodge chance for a defender.
    ///
    /// Only player characters in the Mercenary, Warrior, or Sorcerer lineage
    /// receive this short-term dodge chance.
    ///
    /// # Arguments
    ///
    /// * `co` - Defender character index.
    ///
    /// # Returns
    ///
    /// * Dodge chance as a percent in `0..=100`.
    fn physical_dodge_percent(&self, co: usize) -> i32 {
        let character = &self.characters[co];
        let is_player = (character.flags & CharacterFlags::Player.bits()) != 0;
        if !is_player || !traits::is_mercenary_line(character.kindred) {
            return 0;
        }

        let mut percent = MERCENARY_BASE_DODGE_PERCENT;
        if is_talent_slot_spent(&character.future1, mercenary::DODGE_BOOST_1) {
            percent += MERCENARY_DODGE_TALENT_PERCENT;
        }
        if is_talent_slot_spent(&character.future1, mercenary::DODGE_BOOST_2) {
            percent += MERCENARY_DODGE_TALENT_PERCENT;
        }

        percent.clamp(0, MERCENARY_MAX_DODGE_CHANCE)
    }

    /// Returns whether a percent roll succeeds.
    ///
    /// # Arguments
    ///
    /// * `percent` - Chance in percent.
    /// * `roll` - Roll value in `0..100`.
    ///
    /// # Returns
    ///
    /// * `true` when `roll` lands inside the percent chance.
    fn percent_roll_succeeds(percent: i32, roll: i32) -> bool {
        roll < percent.clamp(0, MERCENARY_MAX_DODGE_CHANCE)
    }

    /// Rolls whether a defender dodges a physical attack.
    ///
    /// # Arguments
    ///
    /// * `co` - Defender character index.
    ///
    /// # Returns
    ///
    /// * `true` when the defender's physical dodge chance succeeds.
    fn dodges_physical_attack(&self, co: usize) -> bool {
        let percent = self.physical_dodge_percent(co);
        Self::percent_roll_succeeds(percent, helpers::random_mod_i32(MERCENARY_MAX_DODGE_CHANCE))
    }

    /// Emits the existing miss feedback for a physical attack.
    ///
    /// # Arguments
    ///
    /// * `cn` - Attacker character index.
    /// * `co` - Defender character index.
    fn emit_attack_miss(&mut self, cn: usize, co: usize) {
        let base_sound = self.characters[cn].sound as i32;
        self.do_area_sound(
            co,
            0,
            self.characters[co].x as i32,
            self.characters[co].y as i32,
            base_sound + 5,
        );
        Self::char_play_sound(self, co, base_sound + 5, -150, 0);

        let ax = self.characters[cn].x as i32;
        let ay = self.characters[cn].y as i32;
        self.do_area_notify(
            cn as i32,
            co as i32,
            ax,
            ay,
            core::constants::NT_SEEMISS as i32,
            cn as i32,
            co as i32,
            0,
            0,
        );
        self.do_notify_character(
            co as u32,
            core::constants::NT_GOTMISS as i32,
            cn as i32,
            0,
            0,
            0,
        );
        self.do_notify_character(
            cn as u32,
            core::constants::NT_DIDMISS as i32,
            co as i32,
            0,
            0,
            0,
        );
    }

    /// Port of `do_enemy(int cn, char* npc, char* victim)` from `svr_do.cpp`.
    ///
    /// Adds or removes an NPC enemy relationship from the god command flow.
    /// When `victim` is empty, the NPC's current enemy list is shown instead.
    ///
    /// # Arguments
    ///
    /// * `cn` - Character index of the caller issuing the command.
    /// * `npc` - Textual character index for the NPC whose enemy list is being changed.
    /// * `victim` - Textual character index for the target to add or remove.
    pub(crate) fn do_enemy(&mut self, cn: usize, npc: &str, victim: &str) {
        if npc.is_empty() {
            self.do_character_log(cn, FontColor::Red, "Make whom the enemy of whom?\n");
            return;
        }

        let co = npc.parse::<usize>().unwrap_or(0);
        if co == 0 {
            self.do_character_log(
                cn,
                FontColor::Red,
                &format!("No such character: '{}'.\n", npc),
            );
            return;
        }

        if !core::types::Character::is_sane_character(co) {
            self.do_character_log(
                cn,
                FontColor::Red,
                "That character is currently not in use.\n",
            );
            return;
        }

        // Only works on NPCs
        let is_player = (self.characters[co].flags & CharacterFlags::Player.bits()) != 0;
        if is_player {
            let name = self.characters[co].get_name().to_string();
            self.do_character_log(
                cn,
                FontColor::Red,
                &format!("#ENEMY only works on NPCs; {} is a player.\n", name),
            );
            return;
        }

        if victim.is_empty() {
            // list enemies
            driver::npc_list_enemies(self, co, cn);
            return;
        }

        let cv = victim.parse::<usize>().unwrap_or(0);
        if cv == 0 {
            self.do_character_log(
                cn,
                FontColor::Red,
                &format!("No such character: '{}'.\n", victim),
            );
            return;
        }

        if !core::types::Character::is_sane_character(cv) {
            self.do_character_log(
                cn,
                FontColor::Red,
                "That character is currently not in use.\n",
            );
            return;
        }

        if driver::npc_is_enemy(&self.characters[co], &self.characters[cv], cv) {
            if !driver::npc_remove_enemy(self, co, cv) {
                let vname = self.characters[cv].get_name().to_string();
                let cname = self.characters[co].get_name().to_string();
                self.do_character_log(
                    cn,
                    FontColor::Red,
                    &format!("Can't remove {} from {}'s enemy list!\n", vname, cname),
                );
                log::error!(
                    "#ENEMY failed to remove {} from {}'s enemy list.",
                    vname,
                    cname
                );
            } else {
                let vname = self.characters[cv].get_name().to_string();
                let cname = self.characters[co].get_name().to_string();
                self.do_character_log(
                    cn,
                    FontColor::Yellow,
                    &format!("Removed {} from {}'s enemy list.\n", vname, cname),
                );
                log::info!("IMP: Removed {} from {}'s enemy list.", vname, cname);
            }
            return;
        }

        // Refuse if same group
        let same_group = self.characters[co].data[core::constants::CHD_GROUP]
            == self.characters[cv].data[core::constants::CHD_GROUP];
        if same_group {
            let cname = self.characters[co].get_name().to_string();
            let vname = self.characters[cv].get_name().to_string();
            self.do_character_log(
                cn,
                FontColor::Red,
                &format!("{} refuses to fight {}.\n", cname, vname),
            );
            return;
        }

        if !driver::npc_add_enemy(self, co, cv, true) {
            let cname = self.characters[co].get_name().to_string();
            self.do_character_log(
                cn,
                FontColor::Red,
                &format!("{} can't handle any more enemies.\n", cname),
            );
            return;
        }

        // If caller has text[1], make NPC say its text[1] with victim name substitution
        let caller_has_text1 = !c_string_to_str(&mut self.characters[cn].text[1]).is_empty();
        if caller_has_text1 {
            let victim_name = self.characters[cv].get_name().to_string();
            driver::npc_saytext_n(self, co, 1, Some(&victim_name));
        }

        // Log chlogs via info for now
        let vname = self.characters[cv].get_name().to_string();
        let cname = self.characters[co].get_name().to_string();
        log::info!("IMP: Made {} an enemy of {}", vname, cname);
        log::info!(
            "Added {} to kill list (#ENEMY by {})",
            vname,
            self.characters[cn].get_name().to_string()
        );

        self.do_character_log(
            cn,
            FontColor::Yellow,
            &format!("{} is now an enemy of {}.\n", vname, cname),
        );
    }

    /// Resolves a physical melee attack between two characters.
    ///
    /// Handles attack permission checks, enemy bookkeeping, hit and dodge
    /// rolls, damage calculation, weapon wear, miss notifications, and
    /// optional Surround Hit secondary strikes.
    ///
    /// # Arguments
    ///
    /// * `cn` - Attacker character index.
    /// * `co` - Primary defender character index.
    /// * `is_surround` - Whether learned Surround Hit should be evaluated after the primary strike.
    pub(crate) fn do_attack(&mut self, cn: usize, co: usize, is_surround: bool) {
        if !self.may_attack_msg(cn, co, true) {
            self.characters[cn].attack_cn = 0;
            self.characters[cn].cerrno = core::constants::ERR_FAILED as u16;
            return;
        }

        let co_stoned =
            (self.characters[co].flags & core::constants::CharacterFlags::Stoned.bits()) != 0;
        if co_stoned {
            self.characters[cn].attack_cn = 0;
            self.characters[cn].cerrno = core::constants::ERR_FAILED as u16;
            return;
        }

        // Update current_enemy if it changed (for logging purposes in original C)
        let current_enemy = self.characters[cn].current_enemy;
        if current_enemy as usize != co {
            self.characters[cn].current_enemy = co as u16;
            let co_name = self.characters[co].get_name().to_string();
            log::info!("Character {} attacks {} ({})", cn, co_name, co);
        }

        // Port of add_enemy(co, cn) from C - this only updates the enemy array,
        // it does NOT set attack_cn. The fightback behavior is handled in driver_msg
        // when NT_GOTHIT/NT_GOTMISS messages are processed.
        self.add_enemy(co, cn);

        self.remember_pvp(cn, co);

        // Read base fight skills
        let mut s1 = self.get_fight_skill(cn);
        let mut s2 = self.get_fight_skill(co);

        // GF_MAYHEM: In mayhem mode, non-player characters get a skill bonus.
        let mayhem = (self.globals.flags & core::constants::GF_MAYHEM) != 0;
        if mayhem {
            let cn_is_player = (self.characters[cn].flags & CharacterFlags::Player.bits()) != 0;
            let co_is_player = (self.characters[co].flags & CharacterFlags::Player.bits()) != 0;
            if !cn_is_player {
                s1 += 10;
            }
            if !co_is_player {
                s2 += 10;
            }
        }

        // Apply negative luck adjustments if present (C++: luck < 0 -> luck/250 - 1)
        // Only applies to players in the original C code
        let cn_is_player = (self.characters[cn].flags & CharacterFlags::Player.bits()) != 0;
        let cn_luck = self.characters[cn].luck;
        let co_is_player = (self.characters[co].flags & CharacterFlags::Player.bits()) != 0;
        let co_luck = self.characters[co].luck;
        if cn_is_player && cn_luck < 0 {
            s1 += cn_luck / 250 - 1;
        }
        if co_is_player && co_luck < 0 {
            s2 += co_luck / 250 - 1;
        }

        // Use canonical helpers for facing/back checks
        if !driver::is_facing(&self.characters[co], &self.characters[cn]) {
            s2 -= 10;
        }

        if driver::is_back(&self.characters[co], &self.characters[cn]) {
            s2 -= 10;
        }

        // Reduce defender skill if stunned or not currently attacking
        let def_stunned_or_no_attack =
            self.characters[co].stunned != 0 || self.characters[co].attack_cn == 0;
        if def_stunned_or_no_attack {
            s2 -= 10;
        }

        // Now compute diff -> chance/bonus mapping per original C++ table
        let diff = s1 - s2;
        let chance: i32;
        let mut bonus: i32 = 0;
        if diff < -40 {
            chance = 1;
            bonus = -16;
        } else if diff < -36 {
            chance = 2;
            bonus = -8;
        } else if diff < -32 {
            chance = 3;
            bonus = -4;
        } else if diff < -28 {
            chance = 4;
            bonus = -2;
        } else if diff < -24 {
            chance = 5;
            bonus = -1;
        } else if diff < -20 {
            chance = 6;
        } else if diff < -16 {
            chance = 7;
        } else if diff < -12 {
            chance = 8;
        } else if diff < -8 {
            chance = 9;
        } else if diff < -4 {
            chance = 10;
        } else if diff < 0 {
            chance = 11;
        } else if diff == 0 {
            chance = 12;
        } else if diff < 4 {
            chance = 13;
        } else if diff < 8 {
            chance = 14;
        } else if diff < 12 {
            chance = 15;
        } else if diff < 16 {
            chance = 16;
            bonus = 1;
        } else if diff < 20 {
            chance = 17;
            bonus = 2;
        } else if diff < 24 {
            chance = 18;
            bonus = 3;
        } else if diff < 28 {
            chance = 19;
            bonus = 4;
        } else if diff < 32 {
            chance = 19;
            bonus = 5;
        } else if diff < 36 {
            chance = 19;
            bonus = 10;
        } else if diff < 40 {
            chance = 19;
            bonus = 15;
        } else {
            chance = 19;
            bonus = 20;
        }

        let die = helpers::random_mod_i32(20) + 1;
        let hit = die <= chance;

        if hit {
            if self.dodges_physical_attack(co) {
                self.emit_attack_miss(cn, co);

                log::info!(
                    "Character {} dodged the attack from {}!",
                    self.characters[co].get_name(),
                    self.characters[cn].get_name()
                );
                return;
            }

            // Damage calculation follows original pattern
            let strn = self.characters[cn].attrib[core::constants::AT_STREN as usize][5] as i32;

            // Base damage uses character.weapon
            let base_weapon = self.characters[cn].weapon as i32;
            let mut dam = base_weapon + helpers::random_mod_i32(6) + 1;
            if strn > 3 {
                let extra_max = strn / 2;
                if extra_max > 0 {
                    dam += helpers::random_mod_i32(extra_max);
                }
            }
            if die == 2 {
                dam += helpers::random_mod_i32(6) + 1;
            }
            if die == 1 {
                dam += helpers::random_mod_i32(6) + helpers::random_mod_i32(6) + 2;
            }

            let odam = dam;
            dam += bonus;

            // Apply weapon wear if wielding (only for players in original)
            let cn_is_player = (self.characters[cn].flags & CharacterFlags::Player.bits()) != 0;
            if cn_is_player {
                let rhand = self.characters[cn].worn[core::constants::WN_RHAND] as usize;
                if rhand != 0 {
                    driver::item_damage_weapon(self, cn, dam);
                }
            }

            // Apply damage and capture actual applied damage
            let applied = self.do_hurt(cn, co, dam, 0);

            // Play sounds depending on whether damage occurred (match original behaviour)
            let tx = self.characters[co].x as i32;
            let ty = self.characters[co].y as i32;
            let base_sound = self.characters[cn].sound as i32;
            if applied < 1 {
                self.do_area_sound(co, 0, tx, ty, base_sound + 3);
                Self::char_play_sound(self, co, base_sound + 3, -150, 0);
            } else {
                self.do_area_sound(co, 0, tx, ty, base_sound + 4);
                Self::char_play_sound(self, co, base_sound + 4, -150, 0);
            }

            // Surrounding strikes grow from the original cross into larger AoE footprints.
            if is_surround {
                // Match original C++ behavior: surround hits only happen if the
                // character actually *has learned* Surround Hit.
                //
                // Note: In this codebase `skill[z][5]` is a derived value and is
                // clamped to at least 1 for *all* skills (see `really_update_char`),
                // so using `[5] > 0` would incorrectly enable surround for everyone.
                let surround_base = self.characters[cn].skill[skills::SK_SURROUND][0] as i32;
                let surround_eff = self.characters[cn].skill[skills::SK_SURROUND][5] as i32;
                if surround_base != 0 {
                    let aoe_base = if cn_is_player { surround_base } else { 1 };
                    let use_legacy_cross = helpers::skill_aoe_uses_legacy_cross(aoe_base);
                    let attacker_x = self.characters[cn].x as i32;
                    let attacker_y = self.characters[cn].y as i32;

                    for co2 in
                        helpers::skill_aoe_targets(self, Some(cn), attacker_x, attacker_y, aoe_base)
                    {
                        if co2 == cn || co2 == co {
                            continue;
                        }
                        if use_legacy_cross && self.characters[co2].attack_cn as usize != cn {
                            continue;
                        }
                        if !self.may_attack_msg(cn, co2, false) {
                            continue;
                        }
                        if surround_eff + helpers::random_mod_i32(20) > self.get_fight_skill(co2) {
                            let sdam = odam - odam / 4;
                            self.remember_pvp(cn, co2);
                            if self.dodges_physical_attack(co2) {
                                self.emit_attack_miss(cn, co2);
                            } else {
                                self.do_hurt(cn, co2, sdam, 0);
                            }
                        }
                    }
                }
            }
        } else {
            self.emit_attack_miss(cn, co);
        }
    }

    /// Port of `do_char_can_flee(int cn)` from `svr_do.cpp`
    ///
    /// Check if a character can flee from combat.
    ///
    /// # Arguments
    ///
    /// * `cn` - Character index attempting to flee.
    ///
    /// # Returns
    ///
    /// * `true` if the character can flee and enemies were cleared.
    /// * `false` if combat pressure or the escape timer prevents fleeing.
    pub(crate) fn do_char_can_flee(&mut self, cn: usize) -> bool {
        // First, remove stale enemy entries where the relation is not mutual
        for m in 0..4 {
            let co = self.characters[cn].enemy[m] as usize;
            if co != 0 && self.characters[co].current_enemy as usize != cn {
                self.characters[cn].enemy[m] = 0;
            }
        }
        for m in 0..4 {
            let co = self.characters[cn].enemy[m] as usize;
            if co != 0 && self.characters[co].attack_cn as usize != cn {
                self.characters[cn].enemy[m] = 0;
            }
        }

        // If no enemies remain, fleeing succeeds
        let no_enemies = {
            let e0 = self.characters[cn].enemy[0];
            let e1 = self.characters[cn].enemy[1];
            let e2 = self.characters[cn].enemy[2];
            let e3 = self.characters[cn].enemy[3];
            e0 == 0 && e1 == 0 && e2 == 0 && e3 == 0
        };
        if no_enemies {
            return true;
        }

        // If escape timer active, can't flee
        let escape_timer = self.characters[cn].escape_timer;
        if escape_timer != 0 {
            return false;
        }

        // Sum perception of enemies
        let per = {
            let mut per = 0i32;
            for m in 0..4 {
                let co = self.characters[cn].enemy[m] as usize;
                if co != 0 {
                    per += self.characters[co].skill[skills::SK_PERCEPT][5] as i32;
                }
            }
            per
        };

        let ste = self.characters[cn].skill[skills::SK_STEALTH][5] as i32;

        let mut chance = if per == 0 { 0 } else { ste * 15 / per };

        chance = chance.clamp(0, 18);

        if helpers::random_mod_i32(20) <= chance {
            self.do_character_log(cn, core::types::FontColor::Green, "You manage to escape!\n");
            for m in 0..4 {
                self.characters[cn].enemy[m] = 0;
            }
            self.remove_enemy(cn);
            return true;
        }

        self.characters[cn].escape_timer = core::constants::TICKS as u16;
        self.do_character_log(cn, core::types::FontColor::Red, "You cannot escape!\n");

        false
    }

    /// Port of `do_ransack_corpse(int cn, int co, char *msg)` from `svr_do.cpp`
    ///
    /// Handle looting a corpse.
    ///
    /// # Arguments
    ///
    /// * `cn` - Character index inspecting the corpse.
    /// * `co` - Corpse owner character index whose inventory is being inspected.
    /// * `msg` - Message template used when reporting discovered loot hints.
    pub(crate) fn do_ransack_corpse(&mut self, cn: usize, co: usize, msg: &str) {
        let sense_skill = self.characters[cn].skill[skills::SK_SENSE][5] as i32;

        // Check for unique weapon in right hand
        let rhand = self.characters[co].worn[core::constants::WN_RHAND];
        if rhand != 0 {
            let unique = if (rhand as usize) < self.items.len() {
                self.items[rhand as usize].is_unique()
            } else {
                false
            };
            if unique && sense_skill > helpers::random_mod_i32(200) {
                let message = msg.replacen("%s", "a rare weapon", 1);
                self.do_character_log(cn, FontColor::Yellow, &message);
            }
        }

        // Iterate inventory slots
        for n in 0..40 {
            let in_idx = self.characters[co].item[n];
            if in_idx == 0 {
                continue;
            }

            let (flags, temp, placement, unique) = if (in_idx as usize) < self.items.len() {
                let it = &mut self.items[in_idx as usize];
                (it.flags, it.temp, it.placement, it.is_unique())
            } else {
                (0u64, 0u16, 0u16, false)
            };

            if (flags & ItemFlags::IF_MAGIC.bits()) == 0 {
                continue;
            }

            if unique && sense_skill > helpers::random_mod_i32(200) {
                let message = msg.replacen("%s", "a rare weapon", 1);
                self.do_character_log(cn, FontColor::Yellow, &message);
                continue;
            }

            // scrolls: ranges 699-716, 175-178, 181-189
            let is_scroll = (699..=716).contains(&(temp as i32))
                || (175..=178).contains(&(temp as i32))
                || (181..=189).contains(&(temp as i32));
            if is_scroll && sense_skill > helpers::random_mod_i32(200) {
                let message = msg.replacen("%s", "a magical scroll", 1);
                self.do_character_log(cn, FontColor::Yellow, &message);
                continue;
            }

            // potions: explicit list
            let is_potion = matches!(
                temp as i32,
                101 | 102 | 127 | 131 | 135 | 148 | 224 | 273 | 274 | 449
            );
            if is_potion && sense_skill > helpers::random_mod_i32(200) {
                let message = msg.replacen("%s", "a magical potion", 1);
                self.do_character_log(cn, FontColor::Yellow, &message);
                continue;
            }

            // belt / placement check
            if (placement & core::constants::PL_BELT) != 0
                && sense_skill > helpers::random_mod_i32(200)
            {
                let message = msg.replacen("%s", "a magical belt", 1);
                self.do_character_log(cn, FontColor::Yellow, &message);
                continue;
            }
        }
    }

    /// Port of `add_enemy(int cn, int co)` from `svr_do.c`
    ///
    /// Simple function to add `co` to `cn`'s enemy array (NOT the same as npc_add_enemy).
    /// This does NOT set attack_cn or any other state - just tracks who is fighting whom.
    ///
    /// # Arguments
    ///
    /// * `cn` - Character index receiving the enemy entry.
    /// * `co` - Character index to add as an enemy.
    pub(crate) fn add_enemy(&mut self, cn: usize, co: usize) {
        // Check if co is already in the enemy list
        if self.characters[cn].enemy[0] as usize != co
            && self.characters[cn].enemy[1] as usize != co
            && self.characters[cn].enemy[2] as usize != co
            && self.characters[cn].enemy[3] as usize != co
        {
            // Add to first empty slot
            if self.characters[cn].enemy[0] == 0 {
                self.characters[cn].enemy[0] = co as u16;
            } else if self.characters[cn].enemy[1] == 0 {
                self.characters[cn].enemy[1] = co as u16;
            } else if self.characters[cn].enemy[2] == 0 {
                self.characters[cn].enemy[2] = co as u16;
            } else if self.characters[cn].enemy[3] == 0 {
                self.characters[cn].enemy[3] = co as u16;
            }
        }
    }

    /// Removes a character from every enemy list in the world.
    ///
    /// # Arguments
    ///
    /// * `co` - Character index to remove from all enemy arrays.
    pub(crate) fn remove_enemy(&mut self, co: usize) {
        for n in 1..core::constants::MAXCHARS {
            for m in 0..4 {
                if self.characters[n].enemy[m] as usize == co {
                    self.characters[n].enemy[m] = 0;
                }
            }
        }
    }

    /// Port of `may_attack_msg(int cn, int co, int msg)` from `svr_do.cpp`
    ///
    /// Check if character cn may attack character co.
    /// If msg is true, tell cn why they can't attack (if applicable).
    ///
    /// # Arguments
    ///
    /// * `cn` - Attacker character index
    /// * `co` - Target character index  
    /// * `msg` - Whether to display messages explaining why attack is not allowed
    ///
    /// # Returns
    ///
    /// * `true` if the attack is allowed.
    /// * `false` if PvP, safety, map, or rank rules block the attack.
    pub(crate) fn may_attack_msg(&mut self, cn: usize, co: usize, msg: bool) -> bool {
        use core::constants::*;

        // Sanity checks
        if cn == 0 || cn >= MAXCHARS || co == 0 || co >= MAXCHARS {
            return true;
        }
        if self.characters[cn].used == 0 || self.characters[co].used == 0 {
            return true;
        }

        // Unsafe gods may attack anyone
        if (self.characters[cn].flags & CharacterFlags::God.bits()) != 0
            && (self.characters[cn].flags & CharacterFlags::Safe.bits()) == 0
        {
            return true;
        }

        // Unsafe gods may be attacked by anyone
        if (self.characters[co].flags & CharacterFlags::God.bits()) != 0
            && (self.characters[co].flags & CharacterFlags::Safe.bits()) == 0
        {
            return true;
        }

        let mut cn_actual = cn;
        let mut co_actual = co;

        // Player companion? Act as if trying to attack the master instead
        if self.characters[cn].temp as i32 == CT_COMPANION
            && self.characters[cn].data[CHD_COMPANION] == 0
        {
            cn_actual = self.characters[cn].data[CHD_MASTER] as usize;
            if cn_actual == 0 || cn_actual >= MAXCHARS || self.characters[cn_actual].used == 0 {
                return true;
            }
        }

        // NPCs may attack anyone, anywhere
        if (self.characters[cn_actual].flags & CharacterFlags::Player.bits()) == 0 {
            return true;
        }

        // Check for NOFIGHT
        let m1 = (self.characters[cn_actual].x as i32
            + self.characters[cn_actual].y as i32 * SERVER_MAPX) as usize;
        let m2 = (self.characters[co_actual].x as i32
            + self.characters[co_actual].y as i32 * SERVER_MAPX) as usize;

        if ((self.map[m1].flags | self.map[m2].flags) & MF_NOFIGHT) != 0 {
            if msg {
                self.do_character_log(
                    cn,
                    core::types::FontColor::Red,
                    "You can't attack anyone here!\n",
                );
            }
            return false;
        }

        // Player companion target? Act as if trying to attack the master instead
        if self.characters[co_actual].temp as i32 == CT_COMPANION
            && self.characters[co_actual].data[CHD_COMPANION] == 0
        {
            co_actual = self.characters[co_actual].data[CHD_MASTER] as usize;
            if co_actual == 0 || co_actual >= MAXCHARS || self.characters[co_actual].used == 0 {
                return true;
            }
        }

        // Check for player-npc (OK)
        if (self.characters[cn_actual].flags & CharacterFlags::Player.bits()) == 0
            || (self.characters[co_actual].flags & CharacterFlags::Player.bits()) == 0
        {
            return true;
        }

        // Both are players. Check for Arena (OK)
        if ((self.map[m1].flags & self.map[m2].flags) & MF_ARENA as u64) != 0 {
            return true;
        }

        // Check if aggressor is purple
        if (self.characters[cn_actual].kindred & traits::KIN_PURPLE as i32) == 0 {
            if msg {
                self.do_character_log(
                    cn,
                    core::types::FontColor::Red,
                    "You can't attack other players! You're not a follower of the Purple One.\n",
                );
            }
            return false;
        }

        // Check if victim is purple
        if (self.characters[co_actual].kindred & traits::KIN_PURPLE as i32) == 0 {
            if msg {
                let co_name = self.characters[co_actual].get_name();
                let pronoun = if (self.characters[co_actual].kindred & traits::KIN_MALE as i32) != 0
                {
                    "He"
                } else {
                    "She"
                };
                self.do_character_log(
                    cn,
                    core::types::FontColor::Red,
                    &format!(
                        "{} is not a follower of the Purple One. {} is protected.\n",
                        co_name, pronoun
                    ),
                );
            }
            return false;
        }

        if helpers::absrankdiff(&self.characters[cn_actual], &self.characters[co_actual])
            > core::constants::ATTACK_RANGE as u32
        {
            if msg {
                let co_name = self.characters[co_actual].get_name();
                self.do_character_log(
                    cn,
                    core::types::FontColor::Red,
                    &format!(
                        "You're not allowed to attack {}. The rank difference is too large.\n",
                        co_name
                    ),
                );
            }
            return false;
        }

        true
    }

    /// Port of `remember_pvp(int cn, int co)` from `svr_do.cpp`
    ///
    /// Remember PvP attacks for tracking purposes.
    /// Stores the victim and time of attack in the attacker's data fields.
    /// Arena attacks don't count.
    ///
    /// # Arguments
    ///
    /// * `cn` - Attacker character index
    /// * `co` - Victim character index
    pub fn remember_pvp(&mut self, cn: usize, co: usize) {
        let m = (self.characters[cn].x as i32
            + self.characters[cn].y as i32 * core::constants::SERVER_MAPX) as usize;

        // Arena attacks don't count
        if (self.map[m].flags & core::constants::MF_ARENA as u64) != 0 {
            return;
        }

        // Sanity checks for cn
        if cn == 0 || cn >= core::constants::MAXCHARS || self.characters[cn].used == 0 {
            return;
        }

        let mut cn_actual = cn;

        // Substitute master for companion
        if (self.characters[cn].flags & CharacterFlags::Body.bits()) != 0 {
            cn_actual = self.characters[cn].data[core::constants::CHD_MASTER] as usize;
        }

        // Must be a valid player
        if cn_actual == 0 || cn_actual >= core::constants::MAXCHARS {
            return;
        }
        if (self.characters[cn_actual].flags & CharacterFlags::Player.bits()) == 0 {
            return;
        }
        if (self.characters[cn_actual].kindred & traits::KIN_PURPLE as i32) == 0 {
            return;
        }

        // Sanity checks for co
        if co == 0 || co >= core::constants::MAXCHARS || self.characters[co].used == 0 {
            return;
        }

        let mut co_actual = co;

        // Substitute master for companion
        if (self.characters[co].flags & CharacterFlags::Body.bits()) != 0 {
            co_actual = self.characters[co].data[core::constants::CHD_MASTER] as usize;
        }

        // Must be a valid player
        if co_actual == 0 || co_actual >= core::constants::MAXCHARS {
            return;
        }
        if (self.characters[co_actual].flags & CharacterFlags::Player.bits()) == 0 {
            return;
        }

        // Can't attack self
        if cn_actual == co_actual {
            return;
        }

        // Record the attack
        let ticker = self.globals.ticker;
        self.characters[cn_actual].data[core::constants::CHD_ATTACKTIME] = ticker;
        self.characters[cn_actual].data[core::constants::CHD_ATTACKVICT] = co_actual as i32;
    }

    /// Port of `do_spellignore(int cn)` from `svr_do.cpp`
    ///
    /// Toggle the CF_SPELLIGNORE flag for a character.
    /// When set, the character will not fight back if spelled.
    ///
    /// # Arguments
    ///
    /// * `cn` - Character index
    pub(crate) fn do_spellignore(&mut self, cn: usize) {
        if (self.characters[cn].flags & CharacterFlags::SpellIgnore.bits()) != 0 {
            self.characters[cn].flags &= !CharacterFlags::SpellIgnore.bits();
            self.do_character_log(
                cn,
                FontColor::Green,
                "You will now fight back if someone attacks you with a spell.\n",
            );
        } else {
            self.characters[cn].flags |= CharacterFlags::SpellIgnore.bits();
            self.do_character_log(
                cn,
                FontColor::Green,
                "You will no longer fight back if someone attacks you with a spell.\n",
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::with_test_gs;
    use core::constants::{CharacterFlags, USE_ACTIVE};
    use core::talent_trees::mercenary;

    fn seed_character(gs: &mut GameState, cn: usize, flags: u64, kindred: i32) {
        gs.characters[cn] = core::types::Character::default();
        gs.characters[cn].used = USE_ACTIVE;
        gs.characters[cn].flags = flags;
        gs.characters[cn].kindred = kindred;
    }

    fn spend_talent(gs: &mut GameState, cn: usize, slot: core::talent_trees::TalentRef) {
        gs.characters[cn].future1[slot.layer as usize] |= slot.mask;
    }

    #[test]
    fn physical_dodge_percent_applies_to_player_mercenary_line_only() {
        with_test_gs(|gs| {
            seed_character(
                gs,
                1,
                CharacterFlags::Player.bits(),
                traits::KIN_MERCENARY as i32,
            );
            seed_character(
                gs,
                2,
                CharacterFlags::Player.bits(),
                traits::KIN_WARRIOR as i32,
            );
            seed_character(
                gs,
                3,
                CharacterFlags::Player.bits(),
                traits::KIN_SORCERER as i32,
            );
            seed_character(
                gs,
                4,
                CharacterFlags::Player.bits(),
                traits::KIN_TEMPLAR as i32,
            );
            seed_character(gs, 5, 0, traits::KIN_MERCENARY as i32);

            assert_eq!(gs.physical_dodge_percent(1), 10);
            assert_eq!(gs.physical_dodge_percent(2), 10);
            assert_eq!(gs.physical_dodge_percent(3), 10);
            assert_eq!(gs.physical_dodge_percent(4), 0);
            assert_eq!(gs.physical_dodge_percent(5), 0);
        });
    }

    #[test]
    fn physical_dodge_percent_includes_dodge_boost_talents() {
        with_test_gs(|gs| {
            seed_character(
                gs,
                1,
                CharacterFlags::Player.bits(),
                traits::KIN_MERCENARY as i32,
            );

            assert_eq!(gs.physical_dodge_percent(1), 10);

            spend_talent(gs, 1, mercenary::DODGE_BOOST_1);
            assert_eq!(gs.physical_dodge_percent(1), 15);

            spend_talent(gs, 1, mercenary::DODGE_BOOST_2);
            assert_eq!(gs.physical_dodge_percent(1), 20);
        });
    }

    #[test]
    fn physical_dodge_percent_ignores_npc_talent_bits() {
        with_test_gs(|gs| {
            seed_character(gs, 1, 0, traits::KIN_SORCERER as i32);
            spend_talent(gs, 1, mercenary::DODGE_BOOST_1);
            spend_talent(gs, 1, mercenary::DODGE_BOOST_2);

            assert_eq!(gs.physical_dodge_percent(1), 0);
        });
    }

    #[test]
    fn percent_roll_succeeds_inside_percent_boundary() {
        assert!(GameState::percent_roll_succeeds(10, 0));
        assert!(GameState::percent_roll_succeeds(10, 9));
        assert!(!GameState::percent_roll_succeeds(10, 10));
        assert!(!GameState::percent_roll_succeeds(0, 0));
        assert!(GameState::percent_roll_succeeds(100, 99));
    }
}
