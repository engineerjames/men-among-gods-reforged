use crate::driver::Driver;
use crate::god::God;
use crate::helpers;
use crate::repository::Repository;
use crate::state::State;
use core::constants::{CharacterFlags, MAXCHARS};
use core::types::FontColor;
use rand::Rng;

impl State {
    /// Port of `do_area_notify(int cn, int co, int xs, int ys, int notify_type, int dat1, int dat2, int dat3, int dat4)` from `svr_do.cpp`
    ///
    /// Notify all characters in an area about something, excluding cn and co.
    pub(crate) fn do_area_notify(
        &self,
        cn: i32,
        co: i32,
        xs: i32,
        ys: i32,
        notify_type: i32,
        dat1: i32,
        dat2: i32,
        dat3: i32,
        dat4: i32,
    ) {
        Repository::with_map(|map| {
            for y in std::cmp::max(0, ys - core::constants::AREA_SIZE)
                ..std::cmp::min(
                    core::constants::SERVER_MAPY,
                    ys + core::constants::AREA_SIZE + 1,
                )
            {
                let m = y * core::constants::SERVER_MAPX as i32;
                for x in std::cmp::max(0, xs - core::constants::AREA_SIZE)
                    ..std::cmp::min(
                        core::constants::SERVER_MAPX,
                        xs + core::constants::AREA_SIZE + 1,
                    )
                {
                    let cc = map[(x + m) as usize].ch;

                    if cc != 0 && cc != cn as u32 && cc != co as u32 {
                        self.do_notify_character(cc, notify_type, dat1, dat2, dat3, dat4);
                    }
                }
            }
        });
    }

    /// Port of `do_notify_character(int character_id, int notify_type, int dat1, int dat2, int dat3, int dat4)` from `svr_do.cpp`
    ///
    /// Send a notification message to a specific character.
    pub(crate) fn do_notify_character(
        &self,
        character_id: u32,
        notify_type: i32,
        dat1: i32,
        dat2: i32,
        dat3: i32,
        dat4: i32,
    ) {
        Driver::msg(character_id, notify_type, dat1, dat2, dat3, dat4);
    }

    /// Port of `do_npc_shout(int cn, int shout_type, int dat1, int dat2, int dat3, int dat4)` from `svr_do.cpp`
    ///
    /// This routine finds the 3 closest NPCs to the one doing the shouting,
    /// so that they can come to the shouter's rescue or something.
    /// Use this one sparingly! It uses quite a bit of computation time!
    pub(crate) fn do_npc_shout(
        &self,
        cn: usize,
        shout_type: i32,
        dat1: i32,
        dat2: i32,
        dat3: i32,
        dat4: i32,
    ) {
        Repository::with_characters(|characters| {
            let mut best: [i32; 3] = [99; 3];
            let mut bestn: [i32; 3] = [0; 3];

            if characters[cn].data[52] == 3 {
                for co in 1..core::constants::MAXCHARS {
                    if co != cn
                        && characters[co].used == core::constants::USE_ACTIVE
                        && characters[co].flags & CharacterFlags::CF_BODY.bits() == 0
                    {
                        if characters[co].flags
                            & (CharacterFlags::CF_PLAYER | CharacterFlags::CF_USURP).bits()
                            != 0
                        {
                            continue;
                        }

                        if characters[co].data[53] != characters[cn].data[52] {
                            continue;
                        }

                        // TODO: This distance calculation seems incorrect potentially -- doublecheck
                        let distance = (characters[cn].x as i32 - characters[co].x as i32).abs()
                            + (characters[cn].y as i32 - characters[co].y as i32).abs();

                        if distance < best[0] {
                            best[2] = best[1];
                            bestn[2] = bestn[1];
                            best[1] = best[0];
                            bestn[1] = bestn[0];
                            best[0] = distance;
                            bestn[0] = co as i32;
                        } else if distance < best[1] {
                            best[2] = best[1];
                            bestn[2] = bestn[1];
                            best[1] = distance;
                            bestn[1] = co as i32;
                        }
                    }
                }

                for i in 0..bestn.len() {
                    if bestn[i] != 0 {
                        self.do_notify_character(
                            bestn[i] as u32,
                            shout_type,
                            dat1,
                            dat2,
                            dat3,
                            dat4,
                        );
                    }
                }
            } else {
                for co in 1..core::constants::MAXCHARS {
                    if co != cn
                        && characters[co].used == core::constants::USE_ACTIVE
                        && characters[co].flags & CharacterFlags::CF_BODY.bits() == 0
                    {
                        if characters[co].flags
                            & (CharacterFlags::CF_PLAYER | CharacterFlags::CF_USURP).bits()
                            != 0
                        {
                            continue;
                        }

                        if characters[co].data[53] != characters[cn].data[52] {
                            continue;
                        }

                        self.do_notify_character(co as u32, shout_type, dat1, dat2, dat3, dat4);
                    }
                }
            }
        });
    }

    /// Port of `do_look_char(int cn, int co, int godflag, int autoflag, int lootflag)` from `svr_do.cpp`
    ///
    /// Displays detailed information about a character (merchant, corpse, or other player/NPC).
    /// This function sends multiple binary packets to the client to display:
    /// - Character description and status messages
    /// - Character equipment and stats
    /// - Shop/corpse inventory if applicable
    ///
    /// # Arguments
    /// * `cn` - Character doing the looking
    /// * `co` - Character being looked at
    /// * `godflag` - If set, bypasses visibility checks
    /// * `autoflag` - If set, suppresses descriptive text (for repeated/automatic looks)
    /// * `lootflag` - If set, allows looking at corpses
    pub(crate) fn do_look_char(
        &mut self,
        cn: usize,
        co: usize,
        godflag: i32,
        autoflag: i32,
        lootflag: i32,
    ) {
        // Validate parameters
        if co == 0 || co >= core::constants::MAXCHARS {
            return;
        }

        // Check if target is a corpse and distance
        let (is_body, co_x, co_y) = Repository::with_characters(|ch| {
            (
                ch[co].flags & CharacterFlags::CF_BODY.bits() != 0,
                ch[co].x,
                ch[co].y,
            )
        });

        if is_body {
            let (cn_x, cn_y) = Repository::with_characters(|ch| (ch[cn].x, ch[cn].y));
            let distance = (cn_x - co_x).abs() + (cn_y - co_y).abs();
            if distance > 1 {
                return;
            }
            if lootflag == 0 {
                return;
            }
        }

        // Check visibility
        let visibility = if godflag != 0 || is_body {
            1
        } else {
            self.do_char_can_see(cn, co)
        };

        if visibility == 0 {
            return;
        }

        // Handle text descriptions and logging (only if not autoflag)
        let is_merchant = Repository::with_characters(|ch| {
            ch[co].flags & CharacterFlags::CF_MERCHANT.bits() != 0
        });

        if autoflag == 0 && !is_merchant && !is_body {
            // Rate limiting for players
            let is_player = Repository::with_characters(|ch| {
                ch[cn].flags & CharacterFlags::CF_PLAYER.bits() != 0
            });

            if is_player {
                let can_proceed = Repository::with_characters_mut(|ch| {
                    ch[cn].data[71] += core::constants::CNTSAY;
                    if ch[cn].data[71] > core::constants::MAXSAY {
                        false
                    } else {
                        true
                    }
                });

                if !can_proceed {
                    self.do_character_log(
                        cn,
                        FontColor::Green,
                        "Oops, you're a bit too fast for me!\n",
                    );
                    return;
                }
            }

            // Show description or reference
            let (has_desc, description, reference) = Repository::with_characters(|ch| {
                let has_desc = ch[co].description[0] != 0;
                let description = String::from_utf8_lossy(&ch[co].description).to_string();
                let reference = String::from_utf8_lossy(&ch[co].reference).to_string();
                (has_desc, description, reference)
            });

            if has_desc {
                self.do_character_log(cn, FontColor::Yellow, &format!("{}\n", description));
            } else {
                self.do_character_log(cn, FontColor::Yellow, &format!("You see {}.\n", reference));
            }

            // Check if target is AFK (away from keyboard)
            let (co_is_player, co_data0, co_text0) = Repository::with_characters(|ch| {
                let is_player = ch[co].is_player();
                let data0 = ch[co].data[0];
                let text0 = String::from_utf8_lossy(&ch[co].text[0]).to_string();
                (is_player, data0, text0)
            });

            if co_is_player && co_data0 != 0 {
                let co_name = Repository::with_characters(|ch| {
                    String::from_utf8_lossy(&ch[co].name).to_string()
                });

                if !co_text0.is_empty() {
                    self.do_character_log(
                        cn,
                        FontColor::Yellow,
                        &format!("{} is away from keyboard; Message:\n", co_name),
                    );
                    self.do_character_log(cn, FontColor::Green, &format!("  \"{}\"\n", co_text0));
                } else {
                    self.do_character_log(
                        cn,
                        FontColor::Yellow,
                        &format!("{} is away from keyboard.\n", co_name),
                    );
                }
            }

            // Check for Purple One follower
            let (co_kindred, co_reference) = Repository::with_characters(|ch| {
                (
                    ch[co].kindred,
                    String::from_utf8_lossy(&ch[co].reference).to_string(),
                )
            });

            if co_is_player && (co_kindred as u32 & core::constants::KIN_PURPLE) != 0 {
                self.do_character_log(
                    cn,
                    FontColor::Yellow,
                    &format!("{} is a follower of the Purple One.\n", co_reference),
                );
            }

            // Reciprocal "looks at you" message
            let (cn_is_player, cn_is_invisible, cn_is_shutup) = Repository::with_characters(|ch| {
                (
                    ch[cn].flags & CharacterFlags::CF_PLAYER.bits() != 0,
                    ch[cn].flags & CharacterFlags::CF_INVISIBLE.bits() != 0,
                    ch[cn].flags & CharacterFlags::CF_SHUTUP.bits() != 0,
                )
            });

            if godflag == 0 && cn != co && cn_is_player && !cn_is_invisible && !cn_is_shutup {
                let cn_name = Repository::with_characters(|ch| {
                    String::from_utf8_lossy(&ch[cn].name).to_string()
                });

                // TODO: Implement do_char_log to send message to co
                log::info!("TODO: do_char_log({}, '{} looks at you.')", co, cn_name);
            }

            // Show death information for players
            let (co_data14, co_data15, co_data16, co_data17, co_is_god) =
                Repository::with_characters(|ch| {
                    (
                        ch[co].data[14],
                        ch[co].data[15],
                        ch[co].data[16],
                        ch[co].data[17],
                        ch[co].flags & CharacterFlags::CF_GOD.bits() != 0,
                    )
                });

            if co_is_player && co_data14 != 0 && !co_is_god {
                let killer = if co_data15 == 0 {
                    "something".to_string()
                } else {
                    Repository::with_characters(|ch| {
                        if co_data15 as usize >= MAXCHARS || ch[co_data15 as usize].used == 0 {
                            "something".to_string()
                        } else {
                            String::from_utf8_lossy(&ch[co_data15 as usize].name)
                                .trim_matches('\0')
                                .to_string()
                        }
                    })
                };

                let victim_name = Repository::with_characters(|ch| {
                    String::from_utf8_lossy(&ch[co].name)
                        .trim_matches('\0')
                        .to_string()
                });

                let minute = co_data16 / 60;
                let hour = minute / 60;
                let minute_remainder = minute % 60;

                self.do_character_log(
                    cn,
                    FontColor::Yellow,
                    &format!(
                        "{} died {}h {}m ago, killed by {}. Level: {}\n",
                        victim_name, hour, minute_remainder, killer, co_data17
                    ),
                );
            }
        }

        // TODO: Merchant/corpse shop viewing implementation would continue here
        // This is the complete player information viewing portion
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
    pub(crate) fn may_attack_msg(&self, cn: usize, co: usize, msg: bool) -> i32 {
        use core::constants::*;

        Repository::with_characters(|characters| {
            // Sanity checks
            if cn == 0 || cn >= MAXCHARS || co == 0 || co >= MAXCHARS {
                return 1;
            }
            if characters[cn].used == 0 || characters[co].used == 0 {
                return 1;
            }

            // Unsafe gods may attack anyone
            if (characters[cn].flags & CharacterFlags::CF_GOD.bits()) != 0
                && (characters[cn].flags & CharacterFlags::CF_SAFE.bits()) == 0
            {
                return 1;
            }

            // Unsafe gods may be attacked by anyone
            if (characters[co].flags & CharacterFlags::CF_GOD.bits()) != 0
                && (characters[co].flags & CharacterFlags::CF_SAFE.bits()) == 0
            {
                return 1;
            }

            let mut cn_actual = cn;
            let mut co_actual = co;

            // Player companion? Act as if trying to attack the master instead
            if (characters[cn].flags & CharacterFlags::CF_BODY.bits()) != 0
                && characters[cn].data[64] == 0
            {
                cn_actual = characters[cn].data[CHD_MASTER] as usize;
                if cn_actual == 0 || cn_actual >= MAXCHARS || characters[cn_actual].used == 0 {
                    return 1; // Bad values, let them try
                }
            }

            // NPCs may attack anyone, anywhere
            if (characters[cn_actual].flags & CharacterFlags::CF_PLAYER.bits()) == 0 {
                return 1;
            }

            // Check for NOFIGHT
            Repository::with_map(|map| {
                let m1 = (characters[cn_actual].x as i32
                    + characters[cn_actual].y as i32 * SERVER_MAPX as i32)
                    as usize;
                let m2 = (characters[co_actual].x as i32
                    + characters[co_actual].y as i32 * SERVER_MAPX as i32)
                    as usize;

                if ((map[m1].flags | map[m2].flags) & MF_NOFIGHT) != 0 {
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
                if (characters[co_actual].flags & CharacterFlags::CF_BODY.bits()) != 0
                    && characters[co_actual].data[64] == 0
                {
                    co_actual = characters[co_actual].data[CHD_MASTER] as usize;
                    if co_actual == 0 || co_actual >= MAXCHARS || characters[co_actual].used == 0 {
                        return 1; // Bad values, let them try
                    }
                }

                // Check for player-npc (OK)
                if (characters[cn_actual].flags & CharacterFlags::CF_PLAYER.bits()) == 0
                    || (characters[co_actual].flags & CharacterFlags::CF_PLAYER.bits()) == 0
                {
                    return 1;
                }

                // Both are players. Check for Arena (OK)
                if ((map[m1].flags & map[m2].flags) & MF_ARENA as u64) != 0 {
                    return 1;
                }

                // Check if aggressor is purple
                if (characters[cn_actual].kindred & KIN_PURPLE as i32) == 0 {
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
                if (characters[co_actual].kindred & KIN_PURPLE as i32) == 0 {
                    if msg {
                        let co_name = String::from_utf8_lossy(&characters[co_actual].name)
                            .trim_matches('\0')
                            .to_string();
                        let pronoun = if (characters[co_actual].kindred & KIN_MALE as i32) != 0 {
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

                1
            })
        })
    }

    /// Port of `do_give_exp(int cn, int p, int gflag, int rank)` from `svr_do.cpp`
    ///
    /// Give experience points to a character, with optional group distribution.
    pub(crate) fn do_give_exp(&mut self, cn: usize, p: i32, gflag: i32, rank: i32) {
        if p < 0 {
            log::error!("PANIC: do_give_exp got negative amount");
            return;
        }

        if gflag != 0 {
            // Group distribution for players
            let is_player = Repository::with_characters(|ch| {
                (ch[cn].flags & core::constants::CharacterFlags::CF_PLAYER.bits()) != 0
            });
            if is_player {
                let mut c = 1;
                for n in 1..10 {
                    let co = Repository::with_characters(|ch| ch[cn].data[n] as usize);
                    if co != 0 {
                        // mutual membership and visible
                        let mutual = Repository::with_characters(|ch| {
                            let mut found = false;
                            for m in 1..10 {
                                if ch[co].data[m] as usize == cn {
                                    found = true;
                                    break;
                                }
                            }
                            found
                        });
                        if mutual && self.do_char_can_see(cn, co) != 0 {
                            c += 1;
                        }
                    }
                }

                // distribute
                let mut s = 0;
                for n in 1..10 {
                    let co = Repository::with_characters(|ch| ch[cn].data[n] as usize);
                    if co != 0 {
                        let mutual = Repository::with_characters(|ch| {
                            let mut found = false;
                            for m in 1..10 {
                                if ch[co].data[m] as usize == cn {
                                    found = true;
                                    break;
                                }
                            }
                            found
                        });
                        if mutual && self.do_char_can_see(cn, co) != 0 {
                            let share = p / c;
                            self.do_give_exp(co, share, 0, rank);
                            s += share;
                        }
                    }
                }
                self.do_give_exp(cn, p - s, 0, rank);
            } else {
                // NPC follower handling
                let co = Repository::with_characters(|ch| ch[cn].data[63] as i32);
                if co != 0 {
                    self.do_give_exp(cn, p, 0, rank);
                    let master = Repository::with_characters(|ch| ch[cn].data[63] as i32);
                    if master > 0
                        && (master as usize) < core::constants::MAXCHARS
                        && Repository::with_characters(|ch| ch[master as usize].points_tot)
                            > Repository::with_characters(|ch| ch[cn].points_tot)
                    {
                        Repository::with_characters_mut(|characters| {
                            characters[cn].data[28] += helpers::scale_exps2(master, rank, p);
                        });
                    } else {
                        Repository::with_characters_mut(|characters| {
                            characters[cn].data[28] += helpers::scale_exps2(cn as i32, rank, p);
                        });
                    }
                }
            }
            return;
        }

        // Non-grouped experience
        let mut p = p;
        if rank >= 0 && rank <= 24 {
            let master = Repository::with_characters(|ch| ch[cn].data[63] as i32);
            if master > 0
                && (master as usize) < core::constants::MAXCHARS
                && Repository::with_characters(|ch| ch[master as usize].points_tot)
                    > Repository::with_characters(|ch| ch[cn].points_tot)
            {
                p = helpers::scale_exps2(master, rank, p);
            } else {
                p = helpers::scale_exps2(cn as i32, rank, p);
            }
        }

        if p != 0 {
            Repository::with_characters_mut(|characters| {
                characters[cn].points += p;
                characters[cn].points_tot += p;
            });
            self.do_character_log(
                cn,
                core::types::FontColor::Green,
                &format!("You get {} experience points.\n", p),
            );
            self.do_notify_character(cn as u32, core::constants::NT_GOTEXP as i32, p, 0, 0, 0);
            log::info!("TODO: chlog({}, 'Gets {} EXP')", cn, p);
            self.do_update_char(cn);
            // TODO: self.do_check_new_level(cn);
            log::info!("TODO: do_check_new_level({})", cn);
        }
    }

    /// Port of `do_say(int cn, const char *text)` from `svr_do.cpp`
    ///
    /// Handle when a character says something.
    pub(crate) fn do_say(&mut self, cn: usize, text: &str) {
        // Rate limiting for players (skip for direct '|' logs)
        if Repository::with_characters(|ch| {
            (ch[cn].flags & CharacterFlags::CF_PLAYER.bits()) != 0 && !text.starts_with('|')
        }) {
            let can_proceed = Repository::with_characters_mut(|ch| {
                ch[cn].data[71] += core::constants::CNTSAY;
                ch[cn].data[71] <= core::constants::MAXSAY
            });

            if !can_proceed {
                self.do_character_log(
                    cn,
                    FontColor::Green,
                    "Oops, you're a bit too fast for me!\n",
                );
                return;
            }
        }

        // GOD password: grant god flags
        if text == core::constants::GODPASSWORD {
            Repository::with_characters_mut(|ch| {
                ch[cn].flags |= (CharacterFlags::CF_GREATERGOD
                    | CharacterFlags::CF_GOD
                    | CharacterFlags::CF_IMMORTAL
                    | CharacterFlags::CF_CREATOR
                    | CharacterFlags::CF_STAFF
                    | CharacterFlags::CF_IMP)
                    .bits();
            });

            self.do_character_log(cn, FontColor::Red, "Yes, Sire, I recognise you!\n");

            let (x, y) = Repository::with_characters(|ch| (ch[cn].x as i32, ch[cn].y as i32));
            self.do_area_log(
                cn,
                0,
                x,
                y,
                FontColor::Red,
                "ASTONIA RECOGNISES ITS CREATOR!\n",
            );

            return;
        }

        // Special "Skua!/Purple!" behaviour
        Repository::with_characters_mut(|ch| {
            let kindred = ch[cn].kindred as i32;
            let is_skua = text == "Skua!" && (kindred & core::constants::KIN_PURPLE as i32) == 0;
            let is_purple =
                text == "Purple!" && (kindred & core::constants::KIN_PURPLE as i32) != 0;
            if (is_skua || is_purple) && ch[cn].luck > 100 {
                let mut rng = rand::thread_rng();
                if ch[cn].a_hp < ch[cn].hp[5] as i32 * 200 {
                    ch[cn].a_hp += 50000 + rng.gen_range(0..100000);
                    let cap = ch[cn].hp[5] as i32 * 1000;
                    if ch[cn].a_hp > cap {
                        ch[cn].a_hp = cap;
                    }
                    ch[cn].luck -= 25;
                }
                if ch[cn].a_end < ch[cn].end[5] as i32 * 200 {
                    ch[cn].a_end += 50000 + rng.gen_range(0..100000);
                    let cap = ch[cn].end[5] as i32 * 1000;
                    if ch[cn].a_end > cap {
                        ch[cn].a_end = cap;
                    }
                    ch[cn].luck -= 10;
                }
                if ch[cn].a_mana < ch[cn].mana[5] as i32 * 200 {
                    ch[cn].a_mana += 50000 + rng.gen_range(0..100000);
                    let cap = ch[cn].mana[5] as i32 * 1000;
                    if ch[cn].a_mana > cap {
                        ch[cn].a_mana = cap;
                    }
                    ch[cn].luck -= 50;
                }
            }
        });

        if text == "help" {
            self.do_character_log(cn, FontColor::Red, "Use #help instead.\n");
        }

        // direct log write from client
        if text.starts_with('|') {
            log::info!("TODO: chlog({}, '%s')", cn);
            return;
        }

        if text.starts_with('#') || text.starts_with('/') {
            // TODO: self.do_command(cn, &text[1..]);
            log::info!("TODO: do_command({}, '{}')", cn, &text[1..]);
            return;
        }

        // shutup check
        if Repository::with_characters(|ch| (ch[cn].flags & CharacterFlags::CF_SHUTUP.bits()) != 0)
        {
            self.do_character_log(
                cn,
                FontColor::Red,
                "You try to say something, but you only produce a croaking sound.\n",
            );
            return;
        }

        // Underwater: replace with "Blub!" unless blue pill (temp==648) is present
        let mut ptr: &str = text;
        let is_underwater = Repository::with_characters(|ch| {
            let m = ch[cn].x as usize + ch[cn].y as usize * core::constants::SERVER_MAPX as usize;
            Repository::with_map(|map| map[m].flags & core::constants::MF_UWATER as u64 != 0)
        });

        if is_underwater {
            let mut found_blue = false;
            Repository::with_characters(|ch| {
                Repository::with_items(|items| {
                    for n in 0..20usize {
                        let in_idx = ch[cn].spell[n] as usize;
                        if in_idx != 0 && in_idx < items.len() && items[in_idx].temp == 648 {
                            found_blue = true;
                            break;
                        }
                    }
                })
            });

            if !found_blue {
                ptr = "Blub!";
            }
        }

        // detect "name: \"quote\"" fake pattern
        let mut m_val = 0i32;
        for c in text.chars() {
            if m_val == 0 && c.is_alphabetic() {
                m_val = 1;
                continue;
            }
            if m_val == 1 && c.is_alphabetic() {
                continue;
            }
            if m_val == 1 && c == ':' {
                m_val = 2;
                continue;
            }
            if m_val == 2 && c == ' ' {
                m_val = 3;
                continue;
            }
            if m_val == 3 && c == '"' {
                m_val = 4;
                break;
            }
            m_val = 0;
        }

        // Show to area (selective for players/usurp)
        let is_player_or_usurp = Repository::with_characters(|ch| {
            (ch[cn].flags & (CharacterFlags::CF_PLAYER.bits() | CharacterFlags::CF_USURP.bits()))
                != 0
        });

        let (cx, cy, name) = Repository::with_characters(|ch| {
            (
                ch[cn].x as usize,
                ch[cn].y as usize,
                ch[cn].get_name().to_string(),
            )
        });

        if is_player_or_usurp {
            self.do_area_say1(cn, cx, cy, ptr);
        } else {
            let msg = format!("{:.30}: \"{}\"\n", name, ptr);
            self.do_area_log(0, 0, cx as i32, cy as i32, FontColor::Red, &msg);
        }

        if m_val == 4 {
            God::slap(0, cn);
            log::info!(
                "TODO: chlog({}, 'Punished for trying to fake another character')",
                cn
            );
        }

        if is_player_or_usurp {
            log::info!("TODO: chlog({}, 'Says \"{}\"', ptr != text)", cn, text);
        }

        // Lab 9 support
        crate::lab9::Labyrinth9::with_mut(|lab9| {
            let _ = lab9.lab9_guesser_says(cn, text);
        });
    }

    /// Port of `do_tell(int cn, const char *con, const char *text)` from `svr_do.cpp`
    ///
    /// Send a private message to another character.
    pub(crate) fn do_tell(&self, cn: usize, con: &str, text: &str) {
        if Repository::with_characters(|ch| (ch[cn].flags & CharacterFlags::CF_SHUTUP.bits()) != 0)
        {
            self.do_character_log(
                cn,
                core::types::FontColor::Red,
                "You try to speak, but you only produce a croaking sound.\n",
            );
            return;
        }
        let co = self.do_lookup_char(con) as usize;
        if co == 0 {
            self.do_character_log(
                cn,
                core::types::FontColor::Red,
                &format!("Unknown name: {}\n", con),
            );
            return;
        }
        let ok = Repository::with_characters(|ch| {
            (ch[co].flags & CharacterFlags::CF_PLAYER.bits()) != 0
                && ch[co].used == core::constants::USE_ACTIVE
                && !((ch[co].flags & CharacterFlags::CF_INVISIBLE.bits()) != 0 && /* invis_level */ false)
                && (!((ch[co].flags & CharacterFlags::CF_NOTELL.bits()) != 0))
        });
        if !ok {
            self.do_character_log(
                cn,
                core::types::FontColor::Red,
                &format!(
                    "{} is not listening\n",
                    Repository::with_characters(|ch| ch[co].get_name().to_string())
                ),
            );
            return;
        }
        if Repository::with_characters(|ch| ch[co].data[0] != 0) {
            if Repository::with_characters(|ch| ch[co].text[0][0] != 0) {
                self.do_character_log(
                    cn,
                    core::types::FontColor::Red,
                    &format!(
                        "{} is away from keyboard; Message:\n",
                        Repository::with_characters(|ch| ch[co].get_name().to_string())
                    ),
                );
                self.do_character_log(
                    cn,
                    core::types::FontColor::Blue,
                    &format!(
                        "  \"{}\"\n",
                        Repository::with_characters(
                            |ch| String::from_utf8_lossy(&ch[co].text[0]).to_string()
                        )
                    ),
                );
            } else {
                self.do_character_log(
                    cn,
                    core::types::FontColor::Red,
                    &format!(
                        "{} is away from keyboard.\n",
                        Repository::with_characters(|ch| ch[co].get_name().to_string())
                    ),
                );
            }
        }
        if text.is_empty() {
            self.do_character_log(
                cn,
                core::types::FontColor::Red,
                &format!(
                    "I understand that you want to tell {} something. But what?\n",
                    Repository::with_characters(|ch| ch[co].get_name().to_string())
                ),
            );
            return;
        }
        let buf = if Repository::with_characters(
            |ch| (ch[cn].flags & CharacterFlags::CF_INVISIBLE.bits()) != 0 && /*invis_level*/ false,
        ) {
            format!("Somebody tells you: \"{}\"\n", text)
        } else {
            format!(
                "{} tells you: \"{}\"\n",
                Repository::with_characters(|ch| ch[cn].get_name().to_string()),
                text
            )
        };
        self.do_character_log(co, core::types::FontColor::Blue, &buf);
        // ccp_tell omitted
        self.do_character_log(
            cn,
            core::types::FontColor::Yellow,
            &format!(
                "Told {}: \"{}\"\n",
                Repository::with_characters(|ch| ch[co].get_name().to_string()),
                text
            ),
        );
        if cn == co {
            self.do_character_log(
                cn,
                core::types::FontColor::Yellow,
                "Do you like talking to yourself?\n",
            );
        }
        if Repository::with_characters(|ch| (ch[cn].flags & CharacterFlags::CF_PLAYER.bits()) != 0)
        {
            log::info!(
                "Told {}: \"{}\"",
                Repository::with_characters(|ch| ch[co].get_name().to_string()),
                text
            );
        }
    }

    /// Port of `do_gtell(int cn, const char *text)` from `svr_do.cpp`
    ///
    /// Send a message to all group members.
    pub(crate) fn do_gtell(&self, cn: usize, text: &str) {
        if text.is_empty() {
            self.do_character_log(cn, core::types::FontColor::Red, "Group-Tell. Yes. group-tell it will be. But what do you want to tell the other group members?\n");
            return;
        }
        if Repository::with_characters(|ch| (ch[cn].flags & CharacterFlags::CF_SHUTUP.bits()) != 0)
        {
            self.do_character_log(
                cn,
                core::types::FontColor::Red,
                "You try to group-tell, but you only produce a croaking sound.\n",
            );
            return;
        }
        let mut found = false;
        for n in core::constants::CHD_MINGROUP..=core::constants::CHD_MAXGROUP {
            let co = Repository::with_characters(|ch| ch[cn].data[n] as usize);
            if co != 0 {
                if true
                /* isgroup */
                {
                    Repository::with_characters(|ch| {
                        self.do_character_log(
                            co,
                            core::types::FontColor::Blue,
                            &format!("{} group-tells: \"{}\"\n", ch[cn].get_name(), text),
                        );
                    });
                    found = true;
                }
            }
        }
        if found {
            self.do_character_log(
                cn,
                core::types::FontColor::Blue,
                &format!("Told the group: \"{}\"\n", text),
            );
            if Repository::with_characters(|ch| {
                (ch[cn].flags & CharacterFlags::CF_PLAYER.bits()) != 0
            }) {
                log::info!("group-tells \"{}\"", text);
            }
        } else {
            self.do_character_log(
                cn,
                core::types::FontColor::Red,
                "You don't have a group to talk to!\n",
            );
        }
    }

    /// Port of `do_stell(int cn, const char *text)` from `svr_do.cpp`
    ///
    /// Send a message to all staff members.
    pub(crate) fn do_stell(&self, cn: usize, text: &str) {
        if text.is_empty() {
            self.do_character_log(cn, core::types::FontColor::Red, "Staff-Tell. Yes. staff-tell it will be. But what do you want to tell the other staff members?\n");
            return;
        }
        if Repository::with_characters(|ch| (ch[cn].flags & CharacterFlags::CF_SHUTUP.bits()) != 0)
        {
            self.do_character_log(
                cn,
                core::types::FontColor::Red,
                "You try to staff-tell, but you only produce a croaking sound.\n",
            );
            return;
        }
        self.do_staff_log(
            core::types::FontColor::Blue,
            &format!(
                "{:.30} staff-tells: \"{:.200}\"\n",
                Repository::with_characters(|ch| ch[cn].get_name().to_string()),
                text
            ),
        );
        if Repository::with_characters(|ch| (ch[cn].flags & CharacterFlags::CF_PLAYER.bits()) != 0)
        {
            log::info!("staff-tells \"{}\"", text);
        }
    }

    /// Port of `do_itell(int cn, const char *text)` from `svr_do.cpp`
    ///
    /// Send a message to all imp members.
    pub(crate) fn do_itell(&self, cn: usize, text: &str) {
        if text.is_empty() {
            self.do_character_log(cn, core::types::FontColor::Red, "Imp-Tell. Yes. imp-tell it will be. But what do you want to tell the other imps?\n");
            return;
        }
        if Repository::with_characters(|ch| (ch[cn].flags & CharacterFlags::CF_SHUTUP.bits()) != 0)
        {
            self.do_character_log(
                cn,
                core::types::FontColor::Red,
                "You try to imp-tell, but you only produce a croaking sound.\n",
            );
            return;
        }
        if Repository::with_characters(|ch| (ch[cn].flags & CharacterFlags::CF_USURP.bits()) != 0) {
            // simplified
            self.do_imp_log(
                core::types::FontColor::Blue,
                &format!(
                    "{:.30} (usurp) imp-tells: \"{:.170}\"\n",
                    Repository::with_characters(|ch| ch[cn].get_name().to_string()),
                    text
                ),
            );
        } else {
            self.do_imp_log(
                core::types::FontColor::Blue,
                &format!(
                    "{:.30} imp-tells: \"{:.200}\"\n",
                    Repository::with_characters(|ch| ch[cn].get_name().to_string()),
                    text
                ),
            );
        }
        if Repository::with_characters(|ch| (ch[cn].flags & CharacterFlags::CF_PLAYER.bits()) != 0)
        {
            log::info!("imp-tells \"{}\"", text);
        }
    }

    /// Port of `do_shout(int cn, const char *text)` from `svr_do.cpp`
    ///
    /// Shout a message to all players.
    pub(crate) fn do_shout(&self, cn: usize, text: &str) {
        if text.is_empty() {
            self.do_character_log(
                cn,
                core::types::FontColor::Red,
                "Shout. Yes. Shout it will be. But what do you want to shout?\n",
            );
            return;
        }
        if Repository::with_characters(|ch| ch[cn].a_end) < 50000 {
            self.do_character_log(
                cn,
                core::types::FontColor::Red,
                "You're too exhausted to shout!\n",
            );
            return;
        }
        if Repository::with_characters(|ch| (ch[cn].flags & CharacterFlags::CF_SHUTUP.bits()) != 0)
        {
            self.do_character_log(
                cn,
                core::types::FontColor::Red,
                "You try to shout, but you only produce a croaking sound.\n",
            );
            return;
        }
        Repository::with_characters_mut(|ch| ch[cn].a_end -= 50000);
        let buf = if Repository::with_characters(|ch| {
            (ch[cn].flags & CharacterFlags::CF_INVISIBLE.bits()) != 0
        }) {
            format!("Somebody shouts: \"{}\"\n", text)
        } else {
            format!(
                "{} shouts: \"{}\"\n",
                Repository::with_characters(|ch| ch[cn].get_name().to_string()),
                text
            )
        };

        for n in 1..core::constants::MAXCHARS as usize {
            let send = Repository::with_characters(|ch| {
                ((ch[n].flags
                    & (CharacterFlags::CF_PLAYER.bits() | CharacterFlags::CF_USURP.bits()))
                    != 0
                    || ch[n].temp == 15)
                    && ch[n].used == core::constants::USE_ACTIVE
            });
            if send {
                self.do_character_log(n, core::types::FontColor::Blue, &buf);
            }
        }
        if Repository::with_characters(|ch| (ch[cn].flags & CharacterFlags::CF_PLAYER.bits()) != 0)
        {
            log::info!("Shouts \"{}\"", text);
        }
    }

    /// Port of `do_noshout(int cn)` from `svr_do.cpp`
    ///
    /// Toggle whether character hears shouts.
    pub(crate) fn do_noshout(&self, cn: usize) {
        Repository::with_characters_mut(|ch| ch[cn].flags ^= CharacterFlags::CF_NOSHOUT.bits());
        if Repository::with_characters(|ch| (ch[cn].flags & CharacterFlags::CF_NOSHOUT.bits()) != 0)
        {
            self.do_character_log(
                cn,
                core::types::FontColor::Yellow,
                "You will no longer hear people #shout.\n",
            );
        } else {
            self.do_character_log(
                cn,
                core::types::FontColor::Yellow,
                "You will hear people #shout.\n",
            );
        }
    }

    /// Port of `do_notell(int cn)` from `svr_do.cpp`
    ///
    /// Toggle whether character receives tells.
    pub(crate) fn do_notell(&self, cn: usize) {
        Repository::with_characters_mut(|ch| ch[cn].flags ^= CharacterFlags::CF_NOTELL.bits());
        if Repository::with_characters(|ch| (ch[cn].flags & CharacterFlags::CF_NOTELL.bits()) != 0)
        {
            self.do_character_log(
                cn,
                core::types::FontColor::Yellow,
                "You will no longer hear people #tell you something.\n",
            );
        } else {
            self.do_character_log(
                cn,
                core::types::FontColor::Yellow,
                "You will hear if people #tell you something.\n",
            );
        }
    }

    /// Port of `do_nostaff(int cn)` from `svr_do.cpp`
    ///
    /// Toggle whether character hears staff messages.
    pub(crate) fn do_nostaff(&self, cn: usize) {
        Repository::with_characters_mut(|ch| {
            ch[cn].flags ^= CharacterFlags::CF_NOSTAFF.bits();
        });
        if Repository::with_characters(|ch| (ch[cn].flags & CharacterFlags::CF_NOSTAFF.bits()) != 0)
        {
            self.do_character_log(
                cn,
                core::types::FontColor::Yellow,
                "You will no longer hear people using #stell.\n",
            );
        } else {
            self.do_character_log(
                cn,
                core::types::FontColor::Yellow,
                "You will hear people using #stell.\n",
            );
        }
        if Repository::with_characters(|ch| (ch[cn].flags & CharacterFlags::CF_PLAYER.bits()) != 0)
        {
            log::info!(
                "Set nostaff to {}",
                if (Repository::with_characters(|ch| ch[cn].flags)
                    & CharacterFlags::CF_NOSTAFF.bits())
                    != 0
                {
                    "on"
                } else {
                    "off"
                }
            );
        }
    }

    /// Port of `do_is_ignore(int cn, int co, int flag)` from `svr_do.cpp`
    ///
    /// Check if cn is ignoring co.
    pub(crate) fn do_is_ignore(&self, cn: usize, co: usize, flag: i32) -> i32 {
        if flag == 0 {
            for n in 30..39 {
                if Repository::with_characters(|ch| ch[co].data[n] as usize) == cn {
                    return 1;
                }
            }
        }
        for n in 50..59 {
            if Repository::with_characters(|ch| ch[co].data[n] as usize) == cn {
                return 1;
            }
        }
        0
    }

    /// Port of `do_lookup_char_self(const char *name, int cn)` from `svr_do.cpp`
    ///
    /// Lookup a character by name, supporting "self" keyword.
    pub(crate) fn do_lookup_char_self(&self, name: &str, cn: usize) -> i32 {
        if name.eq_ignore_ascii_case("self") {
            return cn as i32;
        }
        self.do_lookup_char(name)
    }

    /// Port of `do_lookup_char(const char *name)` from `svr_do.cpp`
    ///
    /// Lookup a character by name (partial match supported).
    pub(crate) fn do_lookup_char(&self, name: &str) -> i32 {
        let len = name.len();
        if len < 2 {
            return 0;
        }
        let matchname = name.to_lowercase();
        let mut bestmatch = 0;
        let mut quality = 0;
        for n in 1..core::constants::MAXCHARS as usize {
            let used = Repository::with_characters(|ch| ch[n].used);
            if used != core::constants::USE_ACTIVE && used != core::constants::USE_NONACTIVE {
                continue;
            }
            if Repository::with_characters(|ch| (ch[n].flags & CharacterFlags::CF_BODY.bits()) != 0)
            {
                continue;
            }
            let nm = Repository::with_characters(|ch| ch[n].get_name().to_lowercase());
            if !nm.starts_with(&matchname) {
                continue;
            }
            if nm.len() == len {
                bestmatch = n;
                break;
            }
            let q = if Repository::with_characters(|ch| {
                (ch[n].flags & (CharacterFlags::CF_PLAYER.bits() | CharacterFlags::CF_USURP.bits()))
                    != 0
            }) {
                if Repository::with_characters(|ch| ch[n].x) != 0 {
                    3
                } else {
                    2
                }
            } else {
                1
            };
            if q > quality {
                bestmatch = n;
                quality = q;
            }
        }
        bestmatch as i32
    }
}
