use core::constants::{CharacterFlags, ItemFlags};
use core::types::FontColor;

use rand::Rng;

use crate::driver;
use crate::repository::Repository;
use crate::state::State;

impl State {
    /// Port of `get_fight_skill(int cn)` from `svr_do.cpp`
    ///
    /// Calculate effective fighting skill based on character's skills and attributes.
    pub(crate) fn get_fight_skill(&self, cn: usize) -> i32 {
        // Read worn right-hand item index and the relevant skill values.
        let (in_idx, s_hand, s_karate, s_sword, s_dagger, s_axe, s_staff, s_twohand) =
            Repository::with_characters(|characters| {
                let in_idx = characters[cn].worn[core::constants::WN_RHAND] as usize;
                (
                    in_idx,
                    characters[cn].skill[core::constants::SK_HAND][5] as i32,
                    characters[cn].skill[core::constants::SK_KARATE][5] as i32,
                    characters[cn].skill[core::constants::SK_SWORD][5] as i32,
                    characters[cn].skill[core::constants::SK_DAGGER][5] as i32,
                    characters[cn].skill[core::constants::SK_AXE][5] as i32,
                    characters[cn].skill[core::constants::SK_STAFF][5] as i32,
                    characters[cn].skill[core::constants::SK_TWOHAND][5] as i32,
                )
            });

        if in_idx == 0 {
            return std::cmp::max(s_karate, s_hand);
        }

        // Get item flags for the item in right hand.
        let flags = Repository::with_items(|items| items[in_idx].flags);

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

    pub(crate) fn do_enemy(&self, cn: usize, npc: &str, victim: &str) {
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
        let is_player = Repository::with_characters(|ch| {
            (ch[co].flags & CharacterFlags::CF_PLAYER.bits()) != 0
        });
        if is_player {
            let name =
                Repository::with_characters(|ch| String::from_utf8_lossy(&ch[co].name).to_string());
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
                let vname = Repository::with_characters(|ch| {
                    String::from_utf8_lossy(&ch[cv].name).to_string()
                });
                let cname = Repository::with_characters(|ch| {
                    String::from_utf8_lossy(&ch[co].name).to_string()
                });
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
                let vname = Repository::with_characters(|ch| {
                    String::from_utf8_lossy(&ch[cv].name).to_string()
                });
                let cname = Repository::with_characters(|ch| {
                    String::from_utf8_lossy(&ch[co].name).to_string()
                });
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
        let same_group = Repository::with_characters(|ch| {
            ch[co].data[core::constants::CHD_GROUP as usize]
                == ch[cv].data[core::constants::CHD_GROUP as usize]
        });
        if same_group {
            let cname =
                Repository::with_characters(|ch| String::from_utf8_lossy(&ch[co].name).to_string());
            let vname =
                Repository::with_characters(|ch| String::from_utf8_lossy(&ch[cv].name).to_string());
            self.do_character_log(
                cn,
                FontColor::Red,
                &format!("{} refuses to fight {}.\n", cname, vname),
            );
            return;
        }

        if !driver::npc_add_enemy(co, cv, true) {
            let cname =
                Repository::with_characters(|ch| String::from_utf8_lossy(&ch[co].name).to_string());
            self.do_character_log(
                cn,
                FontColor::Red,
                &format!("{} can't handle any more enemies.\n", cname),
            );
            return;
        }

        // If caller has text[1], make NPC say its text[1] with victim name substitution
        let caller_has_text1 = Repository::with_characters(|ch| !ch[cn].text[1].is_empty());
        if caller_has_text1 {
            let victim_name =
                Repository::with_characters(|ch| String::from_utf8_lossy(&ch[cv].name).to_string());
            driver::npc_saytext_n(co, 1, Some(&victim_name));
        }

        // Log chlogs via info for now
        let vname =
            Repository::with_characters(|ch| String::from_utf8_lossy(&ch[cv].name).to_string());
        let cname =
            Repository::with_characters(|ch| String::from_utf8_lossy(&ch[co].name).to_string());
        log::info!("IMP: Made {} an enemy of {}", vname, cname);
        log::info!(
            "Added {} to kill list (#ENEMY by {})",
            vname,
            Repository::with_characters(|ch| String::from_utf8_lossy(&ch[cn].name).to_string())
        );

        self.do_character_log(
            cn,
            FontColor::Yellow,
            &format!("{} is now an enemy of {}.\n", vname, cname),
        );
    }

    pub(crate) fn do_attack(&mut self, cn: usize, co: usize, surround: i32) {
        // Basic attack handling: permission checks, enemy bookkeeping,
        // hit/miss roll, damage calculation, item damage and surround hits.

        if self.may_attack(cn, co, true) == 0 {
            Repository::with_characters_mut(|characters| {
                characters[cn].attack_cn = 0;
                characters[cn].cerrno = core::constants::ERR_FAILED as u16;
            });
            return;
        }

        let co_stoned = Repository::with_characters(|characters| {
            (characters[co].flags & core::constants::CharacterFlags::CF_STONED.bits()) != 0
        });
        if co_stoned {
            Repository::with_characters_mut(|characters| {
                characters[cn].attack_cn = 0;
                characters[cn].cerrno = core::constants::ERR_FAILED as u16;
            });
            return;
        }

        // Ensure the target remembers the attacker (npc enemy list etc.)
        driver::npc_add_enemy(co, cn, true);
        self.remember_pvp(cn, co);

        // Read base fight skills
        let mut s1 = self.get_fight_skill(cn);
        let mut s2 = self.get_fight_skill(co);

        // Apply negative luck adjustments if present (C++: luck < 0 -> luck/250 - 1)
        let (cn_luck, co_luck) =
            Repository::with_characters(|characters| (characters[cn].luck, characters[co].luck));
        if cn_luck < 0 {
            s1 += cn_luck / 250 - 1;
        }
        if co_luck < 0 {
            s2 += co_luck / 250 - 1;
        }

        // Use canonical helpers for facing/back checks
        if crate::driver_skill::is_facing(co, cn) == 0 {
            s2 -= 10;
        }

        if crate::driver_skill::is_back(co, cn) != 0 {
            s2 -= 10;
        }

        // Reduce defender skill if stunned or not currently attacking
        let def_stunned_or_no_attack = Repository::with_characters(|characters| {
            characters[co].stunned != 0 || characters[co].attack_cn == 0
        });
        if def_stunned_or_no_attack {
            s2 -= 10;
        }

        // TODO: GF_MAYHEM global flag adjustments (increase s1/s2 for NPCs)

        // Now compute diff -> chance/bonus mapping per original C++ table
        let diff = s1 - s2;
        let mut chance: i32;
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

        let mut rng = rand::thread_rng();
        let die = rng.gen_range(1..=20);
        let hit = die <= chance;

        if hit {
            // Damage calculation follows original pattern
            let strn = Repository::with_characters(|characters| {
                characters[cn].attrib[core::constants::AT_STREN as usize][5] as i32
            });

            // Base damage uses character.weapon
            let base_weapon =
                Repository::with_characters(|characters| characters[cn].weapon as i32);
            let mut dam = base_weapon + rng.gen_range(1..=6);
            if strn > 3 {
                let extra_max = (strn / 2) as i32;
                if extra_max > 0 {
                    dam += rng.gen_range(0..extra_max);
                }
            }
            if die == 2 {
                dam += rng.gen_range(1..=6);
            }
            if die == 1 {
                dam += rng.gen_range(1..=6) + rng.gen_range(1..=6) + 2;
            }

            let odam = dam;
            dam += bonus;

            // Apply weapon wear if wielding (only for players in original)
            let rhand = Repository::with_characters(|characters| {
                characters[cn].worn[core::constants::WN_RHAND] as usize
            });
            if rhand != 0 {
                crate::driver_use::item_damage_weapon(cn, dam);
            }

            // Apply damage and capture actual applied damage
            let applied = self.do_hurt(cn, co, dam, 0);

            // Play sounds depending on whether damage occurred (match original behaviour)
            let (tx, ty, base_sound) = Repository::with_characters(|characters| {
                (
                    characters[co].x as i32,
                    characters[co].y as i32,
                    characters[cn].sound as i32,
                )
            });
            if applied < 1 {
                State::do_area_sound(co, 0, tx, ty, base_sound + 3);
                State::char_play_sound(co, base_sound + 3, -150, 0);
            } else {
                State::do_area_sound(co, 0, tx, ty, base_sound + 4);
                State::char_play_sound(co, base_sound + 4, -150, 0);
            }

            // Surrounding strikes (cardinal neighbors around attacker)
            if surround != 0 {
                let surround_skill = Repository::with_characters(|characters| {
                    characters[cn].skill[core::constants::SK_SURROUND][5] as i32
                });
                if surround_skill > 0 {
                    let (ax, ay) = Repository::with_characters(|characters| {
                        (characters[cn].x as i32, characters[cn].y as i32)
                    });
                    // cardinal neighbor offsets: +1, -1, +MAPX, -MAPX -> translate to coords
                    let mapx = core::constants::SERVER_MAPX as i32;
                    let neighbors = [(ax + 1, ay), (ax - 1, ay), (ax, ay + 1), (ax, ay - 1)];
                    for (nx, ny) in neighbors.iter() {
                        if *nx < 0
                            || *ny < 0
                            || *nx >= core::constants::SERVER_MAPX as i32
                            || *ny >= core::constants::SERVER_MAPY as i32
                        {
                            continue;
                        }
                        let idx = (*nx + *ny * core::constants::SERVER_MAPX as i32) as usize;
                        let co2 = Repository::with_map(|map| map[idx].ch as usize);
                        if co2 == 0 || co2 == cn || co2 == co {
                            continue;
                        }
                        if Repository::with_characters(|characters| {
                            characters[co2].attack_cn as usize
                        }) != cn
                        {
                            continue;
                        }
                        if surround_skill + rng.gen_range(0..20) > self.get_fight_skill(co2) {
                            let sdam = odam - odam / 4;
                            self.do_hurt(cn, co2, sdam, 0);
                        }
                    }
                }
            }
        } else {
            // Miss: play miss sound and notify
            // Miss: play miss sound and notify observers and participants
            let base_sound = Repository::with_characters(|characters| characters[cn].sound as i32);
            State::do_area_sound(
                co,
                0,
                Repository::with_characters(|ch| ch[co].x) as i32,
                Repository::with_characters(|ch| ch[co].y) as i32,
                base_sound + 5,
            );
            State::char_play_sound(co, base_sound + 5, -150, 0);

            // Notify area that attacker missed
            let (ax, ay) = Repository::with_characters(|ch| (ch[cn].x as i32, ch[cn].y as i32));
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
    pub(crate) fn do_char_can_flee(&self, cn: usize) -> i32 {
        // First, remove stale enemy entries where the relation is not mutual
        Repository::with_characters_mut(|characters| {
            for m in 0..4 {
                let co = characters[cn].enemy[m] as usize;
                if co != 0 && characters[co].current_enemy as usize != cn {
                    characters[cn].enemy[m] = 0;
                }
            }
            for m in 0..4 {
                let co = characters[cn].enemy[m] as usize;
                if co != 0 && characters[co].attack_cn as usize != cn {
                    characters[cn].enemy[m] = 0;
                }
            }
        });

        // If no enemies remain, fleeing succeeds
        let no_enemies = Repository::with_characters(|characters| {
            let e0 = characters[cn].enemy[0];
            let e1 = characters[cn].enemy[1];
            let e2 = characters[cn].enemy[2];
            let e3 = characters[cn].enemy[3];
            e0 == 0 && e1 == 0 && e2 == 0 && e3 == 0
        });
        if no_enemies {
            return 1;
        }

        // If escape timer active, can't flee
        let escape_timer = Repository::with_characters(|characters| characters[cn].escape_timer);
        if escape_timer != 0 {
            return 0;
        }

        // Sum perception of enemies
        let per = Repository::with_characters(|characters| {
            let mut per = 0i32;
            for m in 0..4 {
                let co = characters[cn].enemy[m] as usize;
                if co != 0 {
                    per += characters[co].skill[core::constants::SK_PERCEPT][5] as i32;
                }
            }
            per
        });

        let ste = Repository::with_characters(|characters| {
            characters[cn].skill[core::constants::SK_STEALTH][5] as i32
        });

        let mut chance = if per == 0 { 0 } else { ste * 15 / per };
        if chance < 0 {
            chance = 0;
        }
        if chance > 18 {
            chance = 18;
        }

        let mut rng = rand::thread_rng();
        if rng.gen_range(0..20) <= chance {
            self.do_character_log(cn, core::types::FontColor::Green, "You manage to escape!\n");
            Repository::with_characters_mut(|characters| {
                for m in 0..4 {
                    characters[cn].enemy[m] = 0;
                }
            });
            State::remove_enemy(cn);
            return 1;
        }

        Repository::with_characters_mut(|characters| {
            characters[cn].escape_timer = core::constants::TICKS as u16;
        });
        self.do_character_log(cn, core::types::FontColor::Red, "You cannot escape!\n");

        0
    }

    /// Port of `do_ransack_corpse(int cn, int co, char *msg)` from `svr_do.cpp`
    ///
    /// Handle looting a corpse.
    pub(crate) fn do_ransack_corpse(&self, cn: usize, co: usize, msg: &str) {
        let mut rng = rand::thread_rng();

        let sense_skill = Repository::with_characters(|characters| {
            characters[cn].skill[core::constants::SK_SENSE][5] as i32
        });

        // Check for unique weapon in right hand
        let rhand = Repository::with_characters(|characters| {
            characters[co].worn[core::constants::WN_RHAND]
        });
        if rhand != 0 {
            let unique = Repository::with_items(|items| {
                if (rhand as usize) < items.len() {
                    items[rhand as usize].is_unique()
                } else {
                    false
                }
            });
            if unique && sense_skill > rng.gen_range(0..200) {
                let message = msg.replacen("%s", "a rare weapon", 1);
                self.do_character_log(cn, FontColor::Yellow, &message);
            }
        }

        // Iterate inventory slots
        for n in 0..40 {
            let in_idx = Repository::with_characters(|characters| characters[co].item[n]);
            if in_idx == 0 {
                continue;
            }

            let (flags, temp, placement, unique) = Repository::with_items(|items| {
                if (in_idx as usize) < items.len() {
                    let it = &items[in_idx as usize];
                    (it.flags, it.temp, it.placement, it.is_unique())
                } else {
                    (0u64, 0u16, 0u16, false)
                }
            });

            if (flags & ItemFlags::IF_MAGIC.bits()) == 0 {
                continue;
            }

            if unique && sense_skill > rng.gen_range(0..200) {
                let message = msg.replacen("%s", "a rare weapon", 1);
                self.do_character_log(cn, FontColor::Yellow, &message);
                continue;
            }

            // scrolls: ranges 699-716, 175-178, 181-189
            let is_scroll = (699..=716).contains(&(temp as i32))
                || (175..=178).contains(&(temp as i32))
                || (181..=189).contains(&(temp as i32));
            if is_scroll && sense_skill > rng.gen_range(0..200) {
                let message = msg.replacen("%s", "a magical scroll", 1);
                self.do_character_log(cn, FontColor::Yellow, &message);
                continue;
            }

            // potions: explicit list
            let is_potion = matches!(
                temp as i32,
                101 | 102 | 127 | 131 | 135 | 148 | 224 | 273 | 274 | 449
            );
            if is_potion && sense_skill > rng.gen_range(0..200) {
                let message = msg.replacen("%s", "a magical potion", 1);
                self.do_character_log(cn, FontColor::Yellow, &message);
                continue;
            }

            // belt / placement check
            if (placement & core::constants::PL_BELT) != 0 && sense_skill > rng.gen_range(0..200) {
                let message = msg.replacen("%s", "a magical belt", 1);
                self.do_character_log(cn, FontColor::Yellow, &message);
                continue;
            }
        }
    }

    pub(crate) fn remove_enemy(co: usize) {
        Repository::with_characters_mut(|characters| {
            for n in 1..core::constants::MAXCHARS as usize {
                for m in 0..4 {
                    if characters[n].enemy[m] as usize == co {
                        characters[n].enemy[m] = 0;
                    }
                }
            }
        });
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
    pub(crate) fn may_attack(&self, cn: usize, co: usize, msg: bool) -> i32 {
        // Port from state_backup.rs may_attack_msg
        Repository::with_characters(|characters| {
            let cn_flags = characters[cn].flags;
            let co_flags = characters[co].flags;

            // Can't attack yourself
            if cn == co {
                if msg {
                    self.do_character_log(cn, FontColor::Red, "You cannot attack yourself.\n");
                }
                return 0;
            }

            // Can't attack if you're a merchant
            if (cn_flags & CharacterFlags::CF_MERCHANT.bits()) != 0 {
                if msg {
                    self.do_character_log(cn, FontColor::Red, "Merchants cannot attack.\n");
                }
                return 0;
            }

            // Can't attack if target is a merchant
            if (co_flags & CharacterFlags::CF_MERCHANT.bits()) != 0 {
                if msg {
                    self.do_character_log(cn, FontColor::Red, "You cannot attack merchants.\n");
                }
                return 0;
            }

            // Can't attack corpses
            if (co_flags & CharacterFlags::CF_BODY.bits()) != 0 {
                if msg {
                    self.do_character_log(cn, FontColor::Red, "Your target is already dead.\n");
                }
                return 0;
            }

            // TODO: Add more attack validation rules as needed

            1
        })
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
    pub fn remember_pvp(&self, cn: usize, co: usize) {
        Repository::with_characters_mut(|characters| {
            Repository::with_map(|map| {
                let m = (characters[cn].x as i32
                    + characters[cn].y as i32 * core::constants::SERVER_MAPX as i32)
                    as usize;

                // Arena attacks don't count
                if (map[m].flags & core::constants::MF_ARENA as u64) != 0 {
                    return;
                }

                // Sanity checks for cn
                if cn == 0 || cn >= core::constants::MAXCHARS as usize || characters[cn].used == 0 {
                    return;
                }

                let mut cn_actual = cn;

                // Substitute master for companion
                if (characters[cn].flags & CharacterFlags::CF_BODY.bits()) != 0 {
                    cn_actual = characters[cn].data[core::constants::CHD_MASTER] as usize;
                }

                // Must be a valid player
                if cn_actual == 0 || cn_actual >= core::constants::MAXCHARS as usize {
                    return;
                }
                if (characters[cn_actual].flags & CharacterFlags::CF_PLAYER.bits()) == 0 {
                    return;
                }
                if (characters[cn_actual].kindred & core::constants::KIN_PURPLE as i32) == 0 {
                    return;
                }

                // Sanity checks for co
                if co == 0 || co >= core::constants::MAXCHARS as usize || characters[co].used == 0 {
                    return;
                }

                let mut co_actual = co;

                // Substitute master for companion
                if (characters[co].flags & CharacterFlags::CF_BODY.bits()) != 0 {
                    co_actual = characters[co].data[core::constants::CHD_MASTER] as usize;
                }

                // Must be a valid player
                if co_actual == 0 || co_actual >= core::constants::MAXCHARS as usize {
                    return;
                }
                if (characters[co_actual].flags & CharacterFlags::CF_PLAYER.bits()) == 0 {
                    return;
                }

                // Can't attack self
                if cn_actual == co_actual {
                    return;
                }

                // Record the attack
                // TODO: Get actual ticker value from Server/State
                let ticker = 0; // Placeholder
                characters[cn_actual].data[core::constants::CHD_ATTACKTIME] = ticker;
                characters[cn_actual].data[core::constants::CHD_ATTACKVICT] = co_actual as i32;
            });
        });
    }

    /// Port of `do_spellignore(int cn)` from `svr_do.cpp`
    ///
    /// Toggle the CF_SPELLIGNORE flag for a character.
    /// When set, the character will not fight back if spelled.
    ///
    /// # Arguments
    /// * `cn` - Character index
    pub(crate) fn do_spellignore(&self, cn: usize) {
        Repository::with_characters_mut(|characters| {
            let ch = &mut characters[cn];

            if (ch.flags & CharacterFlags::CF_SPELLIGNORE.bits()) != 0 {
                ch.flags &= !CharacterFlags::CF_SPELLIGNORE.bits();
                self.do_character_log(
                    cn,
                    FontColor::Green,
                    "You will now fight back if someone attacks you with a spell.\n",
                );
            } else {
                ch.flags |= CharacterFlags::CF_SPELLIGNORE.bits();
                self.do_character_log(
                    cn,
                    FontColor::Green,
                    "You will no longer fight back if someone attacks you with a spell.\n",
                );
            }
        });
    }
}
