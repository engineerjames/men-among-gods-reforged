use core::constants::{CharacterFlags, ItemFlags};
use core::string_operations::c_string_to_str;
use core::types::FontColor;

use crate::driver;
use crate::helpers;
use crate::game_state::GameState;

impl GameState {
    /// Port of `get_fight_skill(int cn)` from `svr_do.cpp`
    ///
    /// Calculate effective fighting skill based on character's skills and attributes.
    pub(crate) fn get_fight_skill(&mut self, cn: usize) -> i32 {
        // Read worn right-hand item index and the relevant skill values.
        let in_idx = self.characters[cn].worn[core::constants::WN_RHAND] as usize;
        let s_hand = self.characters[cn].skill[core::constants::SK_HAND][5] as i32;
        let s_karate = self.characters[cn].skill[core::constants::SK_KARATE][5] as i32;
        let s_sword = self.characters[cn].skill[core::constants::SK_SWORD][5] as i32;
        let s_dagger = self.characters[cn].skill[core::constants::SK_DAGGER][5] as i32;
        let s_axe = self.characters[cn].skill[core::constants::SK_AXE][5] as i32;
        let s_staff = self.characters[cn].skill[core::constants::SK_STAFF][5] as i32;
        let s_twohand = self.characters[cn].skill[core::constants::SK_TWOHAND][5] as i32;

        if in_idx == 0 {
            return std::cmp::max(s_karate, s_hand);
        }

        // Get item flags for the item in right hand.
        let flags = self.items[in_idx].flags;

        if (flags & core::constants::ItemFlags::IF_WP_SWORD.bits()) != 0 {
            return s_sword;
        }
        if (flags & core::constants::ItemFlags::IF_WP_DAGGER.bits()) != 0 {
            return s_dagger;
        }
        if (flags & core::constants::ItemFlags::IF_WP_AXE.bits()) != 0 {
            return s_axe;
        }
        if (flags & core::constants::ItemFlags::IF_WP_STAFF.bits()) != 0 {
            return s_staff;
        }
        if (flags & core::constants::ItemFlags::IF_WP_TWOHAND.bits()) != 0 {
            return s_twohand;
        }

        std::cmp::max(s_karate, s_hand)
    }

    pub(crate) fn do_enemy(&mut self, cn: usize, npc: &str, victim: &str) {
        // Port of do_enemy(int cn, char* npc, char* victim)
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
        let is_player =
            (self.characters[co].flags & CharacterFlags::Player.bits()) != 0;
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
            driver::npc_list_enemies(co, cn);
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

        if driver::npc_is_enemy(co, cv) {
            if !driver::npc_remove_enemy(co, cv) {
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
        let same_group =
            self.characters[co].data[core::constants::CHD_GROUP] == self.characters[cv].data[core::constants::CHD_GROUP];
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

        if !driver::npc_add_enemy(co, cv, true) {
            let cname = self.characters[co].get_name().to_string();
            self.do_character_log(
                cn,
                FontColor::Red,
                &format!("{} can't handle any more enemies.\n", cname),
            );
            return;
        }

        // If caller has text[1], make NPC say its text[1] with victim name substitution
        let caller_has_text1 =
            !c_string_to_str(&mut self.characters[cn].text[1]).is_empty();
        if caller_has_text1 {
            let victim_name = self.characters[cv].get_name().to_string();
            driver::npc_saytext_n(co, 1, Some(&victim_name));
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

    pub(crate) fn do_attack(&mut self, cn: usize, co: usize, is_surround: bool) {
        // Basic attack handling: permission checks, enemy bookkeeping,
        // hit/miss roll, damage calculation, item damage and surround hits.

        if self.may_attack_msg(cn, co, true) == 0 {
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
        let mayhem =
            (self.globals.flags & core::constants::GF_MAYHEM) != 0;
        if mayhem {
            let cn_is_player =
                (self.characters[cn].flags & CharacterFlags::Player.bits()) != 0;
            let co_is_player =
                (self.characters[co].flags & CharacterFlags::Player.bits()) != 0;
            if !cn_is_player {
                s1 += 10;
            }
            if !co_is_player {
                s2 += 10;
            }
        }

        // Apply negative luck adjustments if present (C++: luck < 0 -> luck/250 - 1)
        // Only applies to players in the original C code
        let cn_is_player =
            (self.characters[cn].flags & CharacterFlags::Player.bits()) != 0;
        let cn_luck = self.characters[cn].luck;
        let co_is_player =
            (self.characters[co].flags & CharacterFlags::Player.bits()) != 0;
        let co_luck = self.characters[co].luck;
        if cn_is_player && cn_luck < 0 {
            s1 += cn_luck / 250 - 1;
        }
        if co_is_player && co_luck < 0 {
            s2 += co_luck / 250 - 1;
        }

        // Use canonical helpers for facing/back checks
        if driver::is_facing(co, cn) == 0 {
            s2 -= 10;
        }

        if driver::is_back(co, cn) != 0 {
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
            // Damage calculation follows original pattern
            let strn =
                self.characters[cn].attrib[core::constants::AT_STREN as usize][5] as i32;

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
            let cn_is_player =
                (self.characters[cn].flags & CharacterFlags::Player.bits()) != 0;
            if cn_is_player {
                let rhand =
                    self.characters[cn].worn[core::constants::WN_RHAND] as usize;
                if rhand != 0 {
                    driver::item_damage_weapon(cn, dam);
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
                Self::char_play_sound(co, base_sound + 3, -150, 0);
            } else {
                self.do_area_sound(co, 0, tx, ty, base_sound + 4);
                Self::char_play_sound(co, base_sound + 4, -150, 0);
            }

            // Surrounding strikes (cardinal neighbors around attacker)
            if is_surround {
                // Match original C++ behavior: surround hits only happen if the
                // character actually *has learned* Surround Hit.
                //
                // Note: In this codebase `skill[z][5]` is a derived value and is
                // clamped to at least 1 for *all* skills (see `really_update_char`),
                // so using `[5] > 0` would incorrectly enable surround for everyone.
                let surround_base =
                    self.characters[cn].skill[core::constants::SK_SURROUND][0] as i32;
                let surround_eff =
                    self.characters[cn].skill[core::constants::SK_SURROUND][5] as i32;
                if surround_base != 0 {
                    let ax = self.characters[cn].x as i32;
                    let ay = self.characters[cn].y as i32;
                    // cardinal neighbor offsets: +1, -1, +MAPX, -MAPX -> translate to coords
                    let neighbors = [(ax + 1, ay), (ax - 1, ay), (ax, ay + 1), (ax, ay - 1)];
                    for (nx, ny) in neighbors.iter() {
                        if *nx < 0
                            || *ny < 0
                            || *nx >= core::constants::SERVER_MAPX
                            || *ny >= core::constants::SERVER_MAPY
                        {
                            continue;
                        }
                        let idx = (*nx + *ny * core::constants::SERVER_MAPX) as usize;
                        let co2 = self.map[idx].ch as usize;
                        if co2 == 0 || co2 == cn || co2 == co {
                            continue;
                        }
                        if self.characters[co2].attack_cn as usize != cn
                        {
                            continue;
                        }
                        if surround_eff + helpers::random_mod_i32(20) > self.get_fight_skill(co2) {
                            let sdam = odam - odam / 4;
                            self.do_hurt(cn, co2, sdam, 0);
                        }
                    }
                }
            }
        } else {
            // Miss: play miss sound and notify
            // Miss: play miss sound and notify observers and participants
            let base_sound = self.characters[cn].sound as i32;
            self.do_area_sound(
                co,
                0,
                self.characters[co].x as i32,
                self.characters[co].y as i32,
                base_sound + 5,
            );
            Self::char_play_sound(co, base_sound + 5, -150, 0);

            // Notify area that attacker missed
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
    }

    /// Port of `do_char_can_flee(int cn)` from `svr_do.cpp`
    ///
    /// Check if a character can flee from combat.
    pub(crate) fn do_char_can_flee(&mut self, cn: usize) -> i32 {
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
            return 1;
        }

        // If escape timer active, can't flee
        let escape_timer = self.characters[cn].escape_timer;
        if escape_timer != 0 {
            return 0;
        }

        // Sum perception of enemies
        let per = {
            let mut per = 0i32;
            for m in 0..4 {
                let co = self.characters[cn].enemy[m] as usize;
                if co != 0 {
                    per += self.characters[co].skill[core::constants::SK_PERCEPT][5] as i32;
                }
            }
            per
        };

        let ste = self.characters[cn].skill[core::constants::SK_STEALTH][5] as i32;

        let mut chance = if per == 0 { 0 } else { ste * 15 / per };

        chance = chance.clamp(0, 18);

        if helpers::random_mod_i32(20) <= chance {
            self.do_character_log(cn, core::types::FontColor::Green, "You manage to escape!\n");
            for m in 0..4 {
                self.characters[cn].enemy[m] = 0;
            }
            self.remove_enemy(cn);
            return 1;
        }

        self.characters[cn].escape_timer = core::constants::TICKS as u16;
        self.do_character_log(cn, core::types::FontColor::Red, "You cannot escape!\n");

        0
    }

    /// Port of `do_ransack_corpse(int cn, int co, char *msg)` from `svr_do.cpp`
    ///
    /// Handle looting a corpse.
    pub(crate) fn do_ransack_corpse(&mut self, cn: usize, co: usize, msg: &str) {
        let sense_skill =
            self.characters[cn].skill[core::constants::SK_SENSE][5] as i32;

        // Check for unique weapon in right hand
        let rhand =
            self.characters[co].worn[core::constants::WN_RHAND];
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
    /// * `cn` - Attacker character index
    /// * `co` - Target character index  
    /// * `msg` - Whether to display messages explaining why attack is not allowed
    ///
    /// # Returns
    /// * 1 if attack is allowed
    /// * 0 if attack is not allowed
    pub(crate) fn may_attack_msg(&mut self, cn: usize, co: usize, msg: bool) -> i32 {
        use core::constants::*;

        // Sanity checks
        if cn == 0 || cn >= MAXCHARS || co == 0 || co >= MAXCHARS {
            return 1;
        }
        if self.characters[cn].used == 0 || self.characters[co].used == 0 {
            return 1;
        }

        // Unsafe gods may attack anyone
        if (self.characters[cn].flags & CharacterFlags::God.bits()) != 0
            && (self.characters[cn].flags & CharacterFlags::Safe.bits()) == 0
        {
            return 1;
        }

        // Unsafe gods may be attacked by anyone
        if (self.characters[co].flags & CharacterFlags::God.bits()) != 0
            && (self.characters[co].flags & CharacterFlags::Safe.bits()) == 0
        {
            return 1;
        }

        let mut cn_actual = cn;
        let mut co_actual = co;

        // Player companion? Act as if trying to attack the master instead
        if self.characters[cn].temp as i32 == CT_COMPANION && self.characters[cn].data[CHD_COMPANION] == 0
        {
            cn_actual = self.characters[cn].data[CHD_MASTER] as usize;
            if cn_actual == 0 || cn_actual >= MAXCHARS || self.characters[cn_actual].used == 0 {
                return 1; // Bad values, let them try
            }
        }

        // NPCs may attack anyone, anywhere
        if (self.characters[cn_actual].flags & CharacterFlags::Player.bits()) == 0 {
            return 1;
        }

        // Check for NOFIGHT
        let m1 = (self.characters[cn_actual].x as i32
            + self.characters[cn_actual].y as i32 * SERVER_MAPX)
            as usize;
        let m2 = (self.characters[co_actual].x as i32
            + self.characters[co_actual].y as i32 * SERVER_MAPX)
            as usize;

        if ((self.map[m1].flags | self.map[m2].flags) & MF_NOFIGHT) != 0 {
            if msg {
                self.do_character_log(
                    cn,
                    core::types::FontColor::Red,
                    "You can't attack anyone here!\n",
                );
            }
            return 0;
        }

        // Player companion target? Act as if trying to attack the master instead
        if self.characters[co_actual].temp as i32 == CT_COMPANION
            && self.characters[co_actual].data[CHD_COMPANION] == 0
        {
            co_actual = self.characters[co_actual].data[CHD_MASTER] as usize;
            if co_actual == 0 || co_actual >= MAXCHARS || self.characters[co_actual].used == 0 {
                return 1; // Bad values, let them try
            }
        }

        // Check for player-npc (OK)
        if (self.characters[cn_actual].flags & CharacterFlags::Player.bits()) == 0
            || (self.characters[co_actual].flags & CharacterFlags::Player.bits()) == 0
        {
            return 1;
        }

        // Both are players. Check for Arena (OK)
        if ((self.map[m1].flags & self.map[m2].flags) & MF_ARENA as u64) != 0 {
            return 1;
        }

        // Check if aggressor is purple
        if (self.characters[cn_actual].kindred & KIN_PURPLE as i32) == 0 {
            if msg {
                self.do_character_log(
                    cn,
                    core::types::FontColor::Red,
                    "You can't attack other players! You're not a follower of the Purple One.\n",
                );
            }
            return 0;
        }

        // Check if victim is purple
        if (self.characters[co_actual].kindred & KIN_PURPLE as i32) == 0 {
            if msg {
                let co_name = self.characters[co_actual].get_name();
                let pronoun = if (self.characters[co_actual].kindred & KIN_MALE as i32) != 0 {
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
            return 0;
        }

        if helpers::absrankdiff(cn_actual as i32, co_actual as i32)
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
            return 0;
        }

        1
    }

    /// Port of `remember_pvp(int cn, int co)` from `svr_do.cpp`
    ///
    /// Remember PvP attacks for tracking purposes.
    /// Stores the victim and time of attack in the attacker's data fields.
    /// Arena attacks don't count.
    ///
    /// # Arguments
    /// * `cn` - Attacker character index
    /// * `co` - Victim character index
    pub fn remember_pvp(&mut self, cn: usize, co: usize) {
        let m = (self.characters[cn].x as i32
            + self.characters[cn].y as i32 * core::constants::SERVER_MAPX)
            as usize;

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
        if (self.characters[cn_actual].kindred & core::constants::KIN_PURPLE as i32) == 0 {
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
