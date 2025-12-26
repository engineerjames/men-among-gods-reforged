use core::constants::{CharacterFlags, ItemFlags};
use core::types::FontColor;

use rand::Rng;

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
