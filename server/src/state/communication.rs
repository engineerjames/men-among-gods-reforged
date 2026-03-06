use crate::game_state::GameState;
use crate::god::God;
use crate::network_manager::NetworkManager;
use crate::{driver, helpers};
use core::constants::{CharacterFlags, CT_LGUARD};
use core::string_operations::c_string_to_str;
use core::types::FontColor;

impl GameState {
    /// Notifies all characters in an area about an event, excluding `cn` and `co`.
    ///
    /// # Arguments
    ///
    /// * `cn` - Character to exclude from notification
    /// * `co` - Another character to exclude
    /// * `xs`, `ys` - Center coordinates of the area
    /// * `notify_type` - Type of notification
    /// * `dat1`, `dat2`, `dat3`, `dat4` - Additional data for the notification
    pub(crate) fn do_area_notify(
        &mut self,
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
        for y in std::cmp::max(0, ys - core::constants::AREA_SIZE)
            ..std::cmp::min(
                core::constants::SERVER_MAPY,
                ys + core::constants::AREA_SIZE + 1,
            )
        {
            let m = y * core::constants::SERVER_MAPX;
            for x in std::cmp::max(0, xs - core::constants::AREA_SIZE)
                ..std::cmp::min(
                    core::constants::SERVER_MAPX,
                    xs + core::constants::AREA_SIZE + 1,
                )
            {
                let cc = self.map[(x + m) as usize].ch;

                if cc != 0 && cc != cn as u32 && cc != co as u32 {
                    self.do_notify_character(cc, notify_type, dat1, dat2, dat3, dat4);
                }
            }
        }
    }

    /// Sends a notification message to a specific character.
    ///
    /// # Arguments
    ///
    /// * `character_id` - Target character ID
    /// * `notify_type` - Type of notification
    /// * `dat1`, `dat2`, `dat3`, `dat4` - Additional data for the notification
    pub(crate) fn do_notify_character(
        &mut self,
        character_id: u32,
        notify_type: i32,
        dat1: i32,
        dat2: i32,
        dat3: i32,
        dat4: i32,
    ) {
        if character_id == 0 || character_id as usize >= core::constants::MAXCHARS {
            return;
        }
        driver::driver_msg(
            self,
            character_id as usize,
            notify_type,
            dat1,
            dat2,
            dat3,
            dat4,
        );
    }

    /// Finds the 3 closest NPCs to the shouter and notifies them.
    ///
    /// # Arguments
    ///
    /// * `cn` - NPC character number (shouter)
    /// * `shout_type` - Type of shout
    /// * `dat1`, `dat2`, `dat3`, `dat4` - Additional data for the shout
    pub(crate) fn do_npc_shout(
        &mut self,
        cn: usize,
        shout_type: i32,
        dat1: i32,
        dat2: i32,
        dat3: i32,
        dat4: i32,
    ) {
        let mut best: [i32; 3] = [99; 3];
        let mut bestn: [i32; 3] = [0; 3];

        if self.characters[cn].data[52] == 3 {
            for co in 1..core::constants::MAXCHARS {
                if co != cn
                    && self.characters[co].used == core::constants::USE_ACTIVE
                    && self.characters[co].flags & CharacterFlags::Body.bits() == 0
                {
                    if self.characters[co].flags
                        & (CharacterFlags::Player | CharacterFlags::Usurp).bits()
                        != 0
                    {
                        continue;
                    }

                    if self.characters[co].data[53] != self.characters[cn].data[52] {
                        continue;
                    }

                    // TODO: This distance calculation seems incorrect potentially -- doublecheck
                    let distance = (self.characters[cn].x as i32 - self.characters[co].x as i32)
                        .abs()
                        + (self.characters[cn].y as i32 - self.characters[co].y as i32).abs();

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
                    self.do_notify_character(bestn[i] as u32, shout_type, dat1, dat2, dat3, dat4);
                }
            }
        } else {
            for co in 1..core::constants::MAXCHARS {
                if co != cn
                    && self.characters[co].used == core::constants::USE_ACTIVE
                    && self.characters[co].flags & CharacterFlags::Body.bits() == 0
                {
                    if self.characters[co].flags
                        & (CharacterFlags::Player | CharacterFlags::Usurp).bits()
                        != 0
                    {
                        continue;
                    }

                    if self.characters[co].data[53] != self.characters[cn].data[52] {
                        continue;
                    }

                    self.do_notify_character(co as u32, shout_type, dat1, dat2, dat3, dat4);
                }
            }
        }
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
    pub fn do_look_char(
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
        let (is_body, co_x, co_y) = (
            self.characters[co].flags & CharacterFlags::Body.bits() != 0,
            self.characters[co].x,
            self.characters[co].y,
        );

        if is_body {
            let (cn_x, cn_y) = (self.characters[cn].x, self.characters[cn].y);
            let distance = (cn_x - co_x).abs() + (cn_y - co_y).abs();
            if distance > 1 {
                return;
            }
            if lootflag == 0 {
                return;
            }
        }

        // Check visibility
        let mut visibility = if godflag != 0 || is_body {
            1
        } else {
            self.do_char_can_see(cn, co)
        };

        if visibility == 0 {
            return;
        }

        // Handle text descriptions and logging (only if not autoflag)
        let (is_merchant, co_temp) = (
            self.characters[co].flags & CharacterFlags::Merchant.bits() != 0,
            self.characters[co].temp,
        );

        if autoflag == 0 && !is_merchant && !is_body {
            // Rate limiting for players
            let is_player = self.characters[cn].flags & CharacterFlags::Player.bits() != 0;

            if is_player {
                self.characters[cn].data[71] += core::constants::CNTSAY;
                let can_proceed = self.characters[cn].data[71] <= core::constants::MAXSAY;

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
            let has_desc = self.characters[co].description[0] != 0;
            let description = c_string_to_str(&mut self.characters[co].description).to_string();
            let reference = c_string_to_str(&mut self.characters[co].reference).to_string();

            if has_desc {
                self.do_character_log(cn, FontColor::Yellow, &format!("{}\n", description));
            } else {
                self.do_character_log(cn, FontColor::Yellow, &format!("You see {}.\n", reference));
            }

            // Check if target is AFK (away from keyboard)
            let co_is_player = self.characters[co].is_player();
            let co_data0 = self.characters[co].data[0];
            let co_text0 = c_string_to_str(&mut self.characters[co].text[0]).to_string();

            if co_is_player && co_data0 != 0 {
                let co_name = c_string_to_str(&mut self.characters[co].name).to_string();

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
            let co_kindred = self.characters[co].kindred;
            let co_reference = c_string_to_str(&mut self.characters[co].reference).to_string();

            if co_is_player && (co_kindred as u32 & core::constants::KIN_PURPLE) != 0 {
                self.do_character_log(
                    cn,
                    FontColor::Yellow,
                    &format!("{} is a follower of the Purple One.\n", co_reference),
                );
            }

            // Reciprocal "looks at you" message
            let cn_is_player = self.characters[cn].flags & CharacterFlags::Player.bits() != 0;
            let cn_is_invisible = self.characters[cn].flags & CharacterFlags::Invisible.bits() != 0;
            let cn_is_shutup = self.characters[cn].flags & CharacterFlags::ShutUp.bits() != 0;

            if godflag == 0 && cn != co && cn_is_player && !cn_is_invisible && !cn_is_shutup {
                let cn_name = c_string_to_str(&mut self.characters[cn].name).to_string();

                self.do_character_log(
                    co,
                    FontColor::Yellow,
                    &format!("{} looks at you.\n", cn_name),
                );
            }

            // Show death information for players
            let co_data14 = self.characters[co].data[14];
            let co_data15 = self.characters[co].data[15];
            let co_data16 = self.characters[co].data[16];
            let co_data17 = self.characters[co].data[17];
            let co_is_god = self.characters[co].flags & CharacterFlags::God.bits() != 0;

            if co_is_player && co_data14 != 0 && !co_is_god {
                let killer = if co_data15 == 0 {
                    "unknown causes".to_string()
                } else if co_data15 >= core::constants::MAXCHARS as i32 {
                    let killer_idx = (co_data15 & 0xFFFF) as usize;
                    c_string_to_str(&mut self.characters[killer_idx].reference).to_string()
                } else {
                    let idx = co_data15 as usize;
                    c_string_to_str(&mut self.character_templates[idx].reference).to_string()
                };

                let area = {
                    let map_x = co_data17 % core::constants::SERVER_MAPX;
                    let map_y = co_data17 / core::constants::SERVER_MAPX;
                    crate::area::get_area_m(map_x, map_y, true)
                };

                self.do_character_log(
                    cn,
                    FontColor::Yellow,
                    &format!(
                        "{} died {} times, the last time on the day {} of the year {}, killed by {} {}.\n",
                        co_reference,
                        co_data14,
                        co_data16 % 300,
                        co_data16 / 300,
                        killer,
                        area
                    ),
                );
            }

            // Show "saved from death" count
            let co_data44 = self.characters[co].data[44];
            if co_is_player && co_data44 != 0 && !co_is_god {
                self.do_character_log(
                    cn,
                    FontColor::Yellow,
                    &format!(
                        "{} was saved from death {} times.\n",
                        co_reference, co_data44
                    ),
                );
            }

            // Show Purple of Honor status
            let co_is_poh = self.characters[co].flags & CharacterFlags::Poh.bits() != 0;
            let co_is_poh_leader =
                self.characters[co].flags & CharacterFlags::PohLeader.bits() != 0;

            if co_is_player && co_is_poh {
                if co_is_poh_leader {
                    self.do_character_log(
                        cn,
                        FontColor::Red,
                        &format!("{} is a Leader among the Purples of Honor.\n", co_reference),
                    );
                } else {
                    self.do_character_log(
                        cn,
                        FontColor::Red,
                        &format!("{} is a Purple of Honor.\n", co_reference),
                    );
                }
            }

            // Show custom text[3] (player description/title)
            let co_text3 = c_string_to_str(&mut self.characters[co].text[3]).to_string();

            if !co_text3.is_empty() && co_is_player {
                self.do_character_log(cn, FontColor::Yellow, &format!("{}\n", co_text3));
            }
        }

        // Get player_id for sending packets
        let player_id = self.characters[cn].player;
        if player_id == 0 {
            return;
        }

        // If visibility > 75, obscure equipment details
        if visibility > 75 {
            visibility = 100;
        }

        // Shared random diffs used for visibility-obscured displays (match original C++ behaviour)
        let mut hp_diff: i32 = 0;
        let mut end_diff: i32 = 0;
        let mut mana_diff: i32 = 0;

        // Send SV_LOOK1 packet (main equipment slots)
        let mut buf = [0u8; 16];
        buf[0] = core::constants::SV_LOOK1;

        if visibility <= 75 {
            let worn_sprites = {
                let mut sprites = [0u16; 7];
                let worn_indices = [0, 2, 3, 5, 6, 7, 8];
                for (i, &slot) in worn_indices.iter().enumerate() {
                    if self.characters[co].worn[slot] != 0 {
                        sprites[i] =
                            self.items[self.characters[co].worn[slot] as usize].sprite[0] as u16;
                    }
                }
                sprites
            };

            for (i, sprite) in worn_sprites.iter().enumerate() {
                let offset = 1 + i * 2;
                buf[offset] = (*sprite & 0xFF) as u8;
                buf[offset + 1] = (*sprite >> 8) as u8;
            }
        } else {
            // Obscured - use sprite 35 for all slots
            for i in 0..7 {
                let offset = 1 + i * 2;
                buf[offset] = 35;
                buf[offset + 1] = 0;
            }
        }
        buf[15] = autoflag as u8;

        NetworkManager::with(|network| {
            network.xsend_gs(self, player_id as usize, &buf, 16);
        });

        // Send SV_LOOK2 packet
        buf[0] = core::constants::SV_LOOK2;

        if visibility <= 75 {
            let worn9 = if self.characters[co].worn[9] != 0 {
                self.items[self.characters[co].worn[9] as usize].sprite[0]
            } else {
                0
            };
            let worn10 = if self.characters[co].worn[10] != 0 {
                self.items[self.characters[co].worn[10] as usize].sprite[0]
            } else {
                0
            };
            let sprite = self.characters[co].sprite;
            let points_tot = self.characters[co].points_tot;
            let hp5 = self.characters[co].hp[5];
            let end5 = self.characters[co].end[5];
            let mana5 = self.characters[co].mana[5];

            buf[1] = (worn9 & 0xFF) as u8;
            buf[2] = (worn9 >> 8) as u8;
            buf[13] = (worn10 & 0xFF) as u8;
            buf[14] = (worn10 >> 8) as u8;

            buf[3] = (sprite & 0xFF) as u8;
            buf[4] = (sprite >> 8) as u8;

            let points_bytes = points_tot.to_le_bytes();
            buf[5..9].copy_from_slice(&points_bytes);

            // Apply random variation if visibility is poor (populate shared diffs)
            if visibility > 75 {
                hp_diff = (hp5 as i32) / 2 - helpers::random_mod_i32(hp5 as i32 + 1);
                end_diff = (end5 as i32) / 2 - helpers::random_mod_i32(end5 as i32 + 1);
                mana_diff = (mana5 as i32) / 2 - helpers::random_mod_i32(mana5 as i32 + 1);
            } else {
                hp_diff = 0;
                end_diff = 0;
                mana_diff = 0;
            }

            let hp_display = ((hp5 as i32 + hp_diff) as u32).to_le_bytes();
            buf[9..13].copy_from_slice(&hp_display);
        } else {
            // Obscured
            buf[1] = 35;
            buf[2] = 0;
            buf[13] = 35;
            buf[14] = 0;
        }

        NetworkManager::with(|network| {
            network.xsend_gs(self, player_id as usize, &buf, 16);
        });

        // Send SV_LOOK3 packet
        buf[0] = core::constants::SV_LOOK3;

        let end5 = self.characters[co].end[5];
        let a_hp = self.characters[co].a_hp;
        let a_end = self.characters[co].a_end;
        let mana5 = self.characters[co].mana[5];
        let a_mana = self.characters[co].a_mana;
        let co_id = helpers::char_id(co);

        // reuse previously computed hp_diff, end_diff, mana_diff (populated in SV_LOOK2)

        let end_display = (end5 as i32 + end_diff) as u16;
        buf[1] = (end_display & 0xFF) as u8;
        buf[2] = (end_display >> 8) as u8;

        let ahp_display = ((a_hp + 500) / 1000 + hp_diff) as u16;
        buf[3] = (ahp_display & 0xFF) as u8;
        buf[4] = (ahp_display >> 8) as u8;

        let aend_display = ((a_end + 500) / 1000 + end_diff) as u16;
        buf[5] = (aend_display & 0xFF) as u8;
        buf[6] = (aend_display >> 8) as u8;

        let co_u16 = co as u16;
        buf[7] = (co_u16 & 0xFF) as u8;
        buf[8] = (co_u16 >> 8) as u8;

        let co_id_u16 = co_id as u16;
        buf[9] = (co_id_u16 & 0xFF) as u8;
        buf[10] = (co_id_u16 >> 8) as u8;

        let mana_display = (mana5 as i32 + mana_diff) as u16;
        buf[11] = (mana_display & 0xFF) as u8;
        buf[12] = (mana_display >> 8) as u8;

        let amana_display = ((a_mana + 500) / 1000 + mana_diff) as u16;
        buf[13] = (amana_display & 0xFF) as u8;
        buf[14] = (amana_display >> 8) as u8;

        NetworkManager::with(|network| {
            network.xsend_gs(self, player_id as usize, &buf, 16);
        });

        // Send SV_LOOK4 packet
        buf[0] = core::constants::SV_LOOK4;

        if visibility <= 75 {
            let worn1 = if self.characters[co].worn[1] != 0 {
                self.items[self.characters[co].worn[1] as usize].sprite[0]
            } else {
                0
            };
            let worn4 = if self.characters[co].worn[4] != 0 {
                self.items[self.characters[co].worn[4] as usize].sprite[0]
            } else {
                0
            };
            let worn11 = if self.characters[co].worn[11] != 0 {
                self.items[self.characters[co].worn[11] as usize].sprite[0]
            } else {
                0
            };
            let worn12 = if self.characters[co].worn[12] != 0 {
                self.items[self.characters[co].worn[12] as usize].sprite[0]
            } else {
                0
            };
            let worn13 = if self.characters[co].worn[13] != 0 {
                self.items[self.characters[co].worn[13] as usize].sprite[0]
            } else {
                0
            };

            buf[1] = (worn1 & 0xFF) as u8;
            buf[2] = (worn1 >> 8) as u8;
            buf[3] = (worn4 & 0xFF) as u8;
            buf[4] = (worn4 >> 8) as u8;
            buf[10] = (worn11 & 0xFF) as u8;
            buf[11] = (worn11 >> 8) as u8;
            buf[12] = (worn12 & 0xFF) as u8;
            buf[13] = (worn12 >> 8) as u8;
            buf[14] = (worn13 & 0xFF) as u8;
            buf[15] = (worn13 >> 8) as u8;
        } else {
            buf[1] = 35;
            buf[2] = 0;
            buf[3] = 35;
            buf[4] = 0;
            buf[10] = 35;
            buf[11] = 0;
            buf[12] = 35;
            buf[13] = 0;
            buf[14] = 35;
            buf[15] = 0;
        }

        // Check if this is a merchant or corpse to show shop interface
        if (is_merchant || is_body) && autoflag == 0 {
            buf[5] = 1;

            // Show price for carried item if applicable
            let citem = self.characters[cn].citem;
            let price = if citem != 0 {
                if is_merchant {
                    let item_val = self.do_item_value(citem as usize) as i32;
                    self.barter(cn, item_val, 0)
                } else {
                    0
                }
            } else {
                0
            };

            let price_bytes = (price as u32).to_le_bytes();
            buf[6..10].copy_from_slice(&price_bytes);
        } else {
            buf[5] = 0;
        }

        NetworkManager::with(|network| {
            network.xsend_gs(self, player_id as usize, &buf, 16);
        });

        // Send SV_LOOK5 packet (character name)
        buf[0] = core::constants::SV_LOOK5;

        let co_name = {
            let mut name = [0u8; 15];
            name.copy_from_slice(&mut self.characters[co].name[0..15]);
            name
        };

        buf[1..16].copy_from_slice(&co_name);

        NetworkManager::with(|network| {
            network.xsend_gs(self, player_id as usize, &buf, 16);
        });

        // Send SV_LOOK6 packets (shop inventory) if merchant or corpse
        if (is_merchant || is_body) && autoflag == 0 {
            // Send inventory slots 0-39 in pairs
            for n in (0..40).step_by(2) {
                buf[0] = core::constants::SV_LOOK6;
                buf[1] = n as u8;

                for m in n..std::cmp::min(40, n + 2) {
                    let item_idx = self.characters[co].item[m];
                    let (sprite, price) = if item_idx != 0 {
                        let spr = self.items[item_idx as usize].sprite[0];
                        let pr = if is_merchant {
                            let item_val = self.do_item_value(item_idx as usize) as i32;
                            self.barter(cn, item_val, 1)
                        } else {
                            0
                        };
                        (spr, pr)
                    } else {
                        (0, 0)
                    };

                    let offset = 2 + (m - n) * 6;
                    buf[offset] = (sprite & 0xFF) as u8;
                    buf[offset + 1] = (sprite >> 8) as u8;

                    let price_bytes = (price as u32).to_le_bytes();
                    buf[offset + 2..offset + 6].copy_from_slice(&price_bytes);
                }

                NetworkManager::with(|network| {
                    network.xsend_gs(self, player_id as usize, &buf, 16);
                });
            }

            // Send worn slots 0-19 (displayed as slots 40-59) if corpse
            for n in (0..20).step_by(2) {
                buf[0] = core::constants::SV_LOOK6;
                buf[1] = (n + 40) as u8;

                for m in n..std::cmp::min(20, n + 2) {
                    let item_idx = self.characters[co].worn[m];
                    let (sprite, price) = if item_idx != 0 && is_body {
                        let spr = self.items[item_idx as usize].sprite[0];
                        (spr, 0)
                    } else {
                        (0, 0)
                    };

                    let offset = 2 + (m - n) * 6;
                    buf[offset] = (sprite & 0xFF) as u8;
                    buf[offset + 1] = (sprite >> 8) as u8;

                    let price_bytes = (price as u32).to_le_bytes();
                    buf[offset + 2..offset + 6].copy_from_slice(&price_bytes);
                }

                NetworkManager::with(|network| {
                    network.xsend_gs(self, player_id as usize, &buf, 16);
                });
            }

            // Send citem and gold (slots 60-61)
            buf[0] = core::constants::SV_LOOK6;
            buf[1] = 60;

            // Slot 60: citem
            let citem_idx = self.characters[co].citem;
            let citem_sprite = if citem_idx != 0 && is_body {
                self.items[citem_idx as usize].sprite[0]
            } else {
                0
            };
            let gold = self.characters[co].gold;

            buf[2] = (citem_sprite & 0xFF) as u8;
            buf[3] = (citem_sprite >> 8) as u8;
            let price_bytes = [0u8; 4];
            buf[4..8].copy_from_slice(&price_bytes);

            // Slot 61: gold
            let gold_sprite = if gold > 0 && is_body {
                if gold > 999999 {
                    121
                } else if gold > 99999 {
                    120
                } else if gold > 9999 {
                    41
                } else if gold > 999 {
                    40
                } else if gold > 99 {
                    39
                } else if gold > 9 {
                    38
                } else {
                    37
                }
            } else {
                0
            };

            buf[8] = (gold_sprite & 0xFF) as u8;
            buf[9] = (gold_sprite >> 8) as u8;
            buf[10..14].copy_from_slice(&[0u8; 4]);

            NetworkManager::with(|network| {
                network.xsend_gs(self, player_id as usize, &buf, 16);
            });
        }

        // God/IMP/USURP debug information
        let cn_is_god_imp_usurp = self.characters[cn].flags
            & (CharacterFlags::God | CharacterFlags::Imp | CharacterFlags::Usurp).bits()
            != 0;

        let co_is_god = self.characters[co].flags & CharacterFlags::God.bits() != 0;

        if cn_is_god_imp_usurp && autoflag == 0 && !is_merchant && !is_body && !co_is_god {
            let (co_x, co_y) = (self.characters[co].x, self.characters[co].y);
            self.do_character_log(
                cn,
                FontColor::Green,
                &format!(
                    "This is char {}, created from template {}, pos {},{}\n",
                    co, co_temp, co_x, co_y
                ),
            );

            let co_is_golden = self.characters[co].flags & CharacterFlags::Golden.bits() != 0;
            let co_is_black = self.characters[co].flags & CharacterFlags::Black.bits() != 0;

            if co_is_golden {
                self.do_character_log(cn, FontColor::Green, "Golden List.\n");
            }
            if co_is_black {
                self.do_character_log(cn, FontColor::Green, "Black List.\n");
            }
        }
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
            let is_player =
                (self.characters[cn].flags & core::constants::CharacterFlags::Player.bits()) != 0;
            if is_player {
                let mut c = 1;
                for n in 1..10 {
                    let co = self.characters[cn].data[n] as usize;
                    if co != 0 {
                        // mutual membership and visible
                        let mutual = {
                            let mut found = false;
                            for m in 1..10 {
                                if self.characters[co].data[m] as usize == cn {
                                    found = true;
                                    break;
                                }
                            }
                            found
                        };
                        if mutual && self.do_char_can_see(cn, co) != 0 {
                            c += 1;
                        }
                    }
                }

                // distribute
                let mut s = 0;
                for n in 1..10 {
                    let co = self.characters[cn].data[n] as usize;
                    if co != 0 {
                        let mutual = {
                            let mut found = false;
                            for m in 1..10 {
                                if self.characters[co].data[m] as usize == cn {
                                    found = true;
                                    break;
                                }
                            }
                            found
                        };
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
                let co = self.characters[cn].data[63];
                if co != 0 {
                    self.do_give_exp(cn, p, 0, rank);
                    let master = self.characters[cn].data[63];
                    if master > 0
                        && (master as usize) < core::constants::MAXCHARS
                        && self.characters[master as usize].points_tot
                            > self.characters[cn].points_tot
                    {
                        self.characters[cn].data[28] += helpers::scale_exps2(master, rank, p);
                    } else {
                        self.characters[cn].data[28] += helpers::scale_exps2(cn as i32, rank, p);
                    }
                }
            }
            return;
        }

        // Non-grouped experience
        let mut p = p;
        if (0..=24).contains(&rank) {
            let master = self.characters[cn].data[63];
            if master > 0
                && (master as usize) < core::constants::MAXCHARS
                && self.characters[master as usize].points_tot > self.characters[cn].points_tot
            {
                p = helpers::scale_exps2(master, rank, p);
            } else {
                p = helpers::scale_exps2(cn as i32, rank, p);
            }
        }

        if p != 0 {
            self.characters[cn].points += p * 10;
            self.characters[cn].points_tot += p * 10;
            self.do_character_log(
                cn,
                core::types::FontColor::Green,
                &format!("You get {} experience points.\n", p),
            );
            self.do_notify_character(cn as u32, core::constants::NT_GOTEXP as i32, p, 0, 0, 0);
            chlog!(
                cn,
                "Gets {} EXP (total {})",
                p,
                self.characters[cn].points_tot
            );
            self.do_update_char(cn);
            self.do_check_new_level(cn);
        }
    }

    /// Port of `do_say(int cn, const char *text)` from `svr_do.cpp`
    ///
    /// Handle when a character says something.
    pub(crate) fn do_say(&mut self, cn: usize, text: &str) {
        log::debug!("do_say: cn={}, text={}", cn, text);
        // Rate limiting for players (skip for direct '|' logs)
        if (self.characters[cn].flags & CharacterFlags::Player.bits()) != 0
            && !text.starts_with('|')
        {
            self.characters[cn].data[71] += core::constants::CNTSAY;
            let can_proceed = self.characters[cn].data[71] <= core::constants::MAXSAY;

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
            self.characters[cn].flags |= (CharacterFlags::GreaterGod
                | CharacterFlags::God
                | CharacterFlags::Immortal
                | CharacterFlags::Creator
                | CharacterFlags::Staff
                | CharacterFlags::Imp)
                .bits();

            self.do_character_log(cn, FontColor::Red, "Yes, Sire, I recognise you!\n");

            let (x, y) = (self.characters[cn].x as i32, self.characters[cn].y as i32);
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
        {
            let kindred = self.characters[cn].kindred;
            let is_skua = text == "Skua!" && (kindred & core::constants::KIN_PURPLE as i32) == 0;
            let is_purple =
                text == "Purple!" && (kindred & core::constants::KIN_PURPLE as i32) != 0;
            if (is_skua || is_purple) && self.characters[cn].luck > 100 {
                if self.characters[cn].a_hp < self.characters[cn].hp[5] as i32 * 200 {
                    self.characters[cn].a_hp += 50000 + helpers::random_mod_i32(100000);
                    let cap = self.characters[cn].hp[5] as i32 * 1000;
                    if self.characters[cn].a_hp > cap {
                        self.characters[cn].a_hp = cap;
                    }
                    self.characters[cn].luck -= 25;
                }
                if self.characters[cn].a_end < self.characters[cn].end[5] as i32 * 200 {
                    self.characters[cn].a_end += 50000 + helpers::random_mod_i32(100000);
                    let cap = self.characters[cn].end[5] as i32 * 1000;
                    if self.characters[cn].a_end > cap {
                        self.characters[cn].a_end = cap;
                    }
                    self.characters[cn].luck -= 10;
                }
                if self.characters[cn].a_mana < self.characters[cn].mana[5] as i32 * 200 {
                    self.characters[cn].a_mana += 50000 + helpers::random_mod_i32(100000);
                    let cap = self.characters[cn].mana[5] as i32 * 1000;
                    if self.characters[cn].a_mana > cap {
                        self.characters[cn].a_mana = cap;
                    }
                    self.characters[cn].luck -= 50;
                }
            }
        }

        if text == "help" {
            self.do_character_log(cn, FontColor::Red, "Use #help instead.\n");
        }

        // direct log write from client
        if text.starts_with('|') {
            chlog!(
                cn,
                "{}",
                &text[1..] // skip '|'
            );
            return;
        }

        if text.starts_with('#') || text.starts_with('/') {
            self.do_command(cn, &text[1..]);
            return;
        }

        // shutup check
        if (self.characters[cn].flags & CharacterFlags::ShutUp.bits()) != 0 {
            self.do_character_log(
                cn,
                FontColor::Red,
                "You try to say something, but you only produce a croaking sound.\n",
            );
            return;
        }

        // Underwater: replace with "Blub!" unless blue pill (temp==648) is present
        let mut ptr: &str = text;
        let m_idx = self.characters[cn].x as usize
            + self.characters[cn].y as usize * core::constants::SERVER_MAPX as usize;
        let is_underwater = self.map[m_idx].flags & core::constants::MF_UWATER as u64 != 0;

        if is_underwater {
            let mut found_blue = false;
            for n in 0..20usize {
                let in_idx = self.characters[cn].spell[n] as usize;
                if in_idx != 0 && in_idx < self.items.len() && self.items[in_idx].temp == 648 {
                    found_blue = true;
                    break;
                }
            }

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
        let is_player_or_usurp = (self.characters[cn].flags
            & (CharacterFlags::Player.bits() | CharacterFlags::Usurp.bits()))
            != 0;

        let cx = self.characters[cn].x as usize;
        let cy = self.characters[cn].y as usize;
        let name = self.characters[cn].get_name().to_string();

        if is_player_or_usurp {
            self.do_area_say1(cn, cx, cy, ptr);
        } else {
            let msg = format!("{:.30}: \"{}\"\n", name, ptr);
            self.do_area_log(0, 0, cx as i32, cy as i32, FontColor::Red, &msg);
        }

        if m_val == 4 {
            God::slap(0, cn);
            chlog!(cn, "Punished for trying to fake another character");
        }

        if is_player_or_usurp {
            chlog!(cn, "Says \"{}\"", ptr);
        }

        // Lab 9 support
        crate::lab9::Labyrinth9::with_mut(|lab9| {
            let _ = lab9.lab9_guesser_says(cn, text);
        });
    }

    /// Port of `do_tell(int cn, const char *con, const char *text)` from `svr_do.cpp`
    ///
    /// Send a private message to another character.
    pub(crate) fn do_tell(&mut self, cn: usize, con: &str, text: &str) {
        if (self.characters[cn].flags & CharacterFlags::ShutUp.bits()) != 0 {
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
        let co_flags = self.characters[co].flags;
        let co_used = self.characters[co].used;
        let co_invis = (self.characters[co].flags & CharacterFlags::Invisible.bits()) != 0;
        let cn_flags = self.characters[cn].flags;
        let co_name = self.characters[co].get_name().to_string();
        let cn_name = self.characters[cn].get_name().to_string();
        let cn_is_god = (cn_flags & CharacterFlags::God.bits()) != 0;
        let cn_is_player = (cn_flags & CharacterFlags::Player.bits()) != 0;
        let co_is_player = (co_flags & CharacterFlags::Player.bits()) != 0;
        let co_notell = (co_flags & CharacterFlags::NoTell.bits()) != 0;
        let co_active = co_used == core::constants::USE_ACTIVE;
        let cn_invis = (cn_flags & CharacterFlags::Invisible.bits()) != 0;
        let cn_invis_level = crate::helpers::invis_level(cn);
        let co_invis_level = crate::helpers::invis_level(co);
        // do_is_ignore
        let is_ignored = !cn_is_god && self.do_is_ignore(cn, co, 0) != 0;
        // C++: ! (player) || not active || (invis && invis_level) || (not god && (notell || ignore))
        if !co_is_player
            || !co_active
            || (co_invis && cn_invis_level < co_invis_level)
            || (!cn_is_god && (co_notell || is_ignored))
        {
            self.do_character_log(
                cn,
                core::types::FontColor::Red,
                &format!("{} is not listening\n", co_name),
            );
            return;
        }
        // AFK message
        let co_afk = self.characters[co].data[0] != 0;
        if co_afk {
            let co_afk_msg = self.characters[co].text[0][0] != 0;
            if co_afk_msg {
                let msg = c_string_to_str(&mut self.characters[co].text[0]).to_string();
                self.do_character_log(
                    cn,
                    core::types::FontColor::Red,
                    &format!("{} is away from keyboard; Message:\n", co_name),
                );
                self.do_character_log(
                    cn,
                    core::types::FontColor::Yellow,
                    &format!("  \"{}\"\n", msg),
                );
            } else {
                self.do_character_log(
                    cn,
                    core::types::FontColor::Red,
                    &format!("{} is away from keyboard.\n", co_name),
                );
            }
        }
        if text.is_empty() {
            self.do_character_log(
                cn,
                core::types::FontColor::Red,
                &format!(
                    "I understand that you want to tell {} something. But what?\n",
                    co_name
                ),
            );
            return;
        }
        let buf = if cn_invis && cn_invis_level > co_invis_level {
            format!("Somebody tells you: \"{:.200}\"\n", text)
        } else {
            format!("{} tells you: \"{:.200}\"\n", cn_name, text)
        };
        self.do_character_log(co, core::types::FontColor::Yellow, &buf);
        // ccp_tell omitted
        self.do_character_log(
            cn,
            core::types::FontColor::Yellow,
            &format!("Told {}: \"{:.200}\"\n", co_name, text),
        );
        if cn == co {
            self.do_character_log(
                cn,
                core::types::FontColor::Yellow,
                "Do you like talking to yourself?\n",
            );
        }
        if cn_is_player {
            log::info!("Told {}: \"{}\"", co_name, text);
        }
    }

    /// Port of `do_gtell(int cn, const char *text)` from `svr_do.cpp`
    ///
    /// Send a message to all group members.
    pub(crate) fn do_gtell(&mut self, cn: usize, text: &str) {
        if text.is_empty() {
            self.do_character_log(cn, core::types::FontColor::Red, "Group-Tell. Yes. group-tell it will be. But what do you want to tell the other group members?\n");
            return;
        }
        if (self.characters[cn].flags & CharacterFlags::ShutUp.bits()) != 0 {
            self.do_character_log(
                cn,
                core::types::FontColor::Red,
                "You try to group-tell, but you only produce a croaking sound.\n",
            );
            return;
        }
        let mut found = false;
        for n in core::constants::CHD_MINGROUP..=core::constants::CHD_MAXGROUP {
            let co = self.characters[cn].data[n] as usize;
            if co != 0 {
                let mut is_member = false;
                for m in core::constants::CHD_MINGROUP..=core::constants::CHD_MAXGROUP {
                    if self.characters[co].data[m] as usize == cn {
                        is_member = true;
                        break;
                    }
                }
                if !is_member {
                    self.characters[cn].data[n] = 0;
                } else {
                    let name = self.characters[cn].get_name().to_string();
                    self.do_character_log(
                        co,
                        core::types::FontColor::Green,
                        &format!("{} group-tells: \"{}\"\n", name, text),
                    );
                    found = true;
                }
            }
        }
        if found {
            self.do_character_log(
                cn,
                core::types::FontColor::Green,
                &format!("Told the group: \"{}\"\n", text),
            );
            if (self.characters[cn].flags & CharacterFlags::Player.bits()) != 0 {
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
    pub(crate) fn do_stell(&mut self, cn: usize, text: &str) {
        if text.is_empty() {
            self.do_character_log(cn, core::types::FontColor::Red, "Staff-Tell. Yes. staff-tell it will be. But what do you want to tell the other staff members?\n");
            return;
        }
        if (self.characters[cn].flags & CharacterFlags::ShutUp.bits()) != 0 {
            self.do_character_log(
                cn,
                core::types::FontColor::Red,
                "You try to staff-tell, but you only produce a croaking sound.\n",
            );
            return;
        }
        let name = self.characters[cn].get_name().to_string();
        self.do_staff_log(
            core::types::FontColor::Blue,
            &format!("{:.30} staff-tells: \"{:.200}\"\n", name, text),
        );
        if (self.characters[cn].flags & CharacterFlags::Player.bits()) != 0 {
            log::info!("staff-tells \"{}\"", text);
        }
    }

    /// Port of `do_itell(int cn, const char *text)` from `svr_do.cpp`
    ///
    /// Send a message to all imp members.
    pub(crate) fn do_itell(&mut self, cn: usize, text: &str) {
        if text.is_empty() {
            self.do_character_log(cn, core::types::FontColor::Red, "Imp-Tell. Yes. imp-tell it will be. But what do you want to tell the other imps?\n");
            return;
        }
        if (self.characters[cn].flags & CharacterFlags::ShutUp.bits()) != 0 {
            self.do_character_log(
                cn,
                core::types::FontColor::Red,
                "You try to imp-tell, but you only produce a croaking sound.\n",
            );
            return;
        }
        if (self.characters[cn].flags & CharacterFlags::Usurp.bits()) != 0 {
            // simplified
            let name = self.characters[cn].get_name().to_string();
            self.do_imp_log(
                core::types::FontColor::Blue,
                &format!("{:.30} (usurp) imp-tells: \"{:.170}\"\n", name, text),
            );
        } else {
            let name = self.characters[cn].get_name().to_string();
            self.do_imp_log(
                core::types::FontColor::Blue,
                &format!("{:.30} imp-tells: \"{:.200}\"\n", name, text),
            );
        }
        if (self.characters[cn].flags & CharacterFlags::Player.bits()) != 0 {
            log::info!("imp-tells \"{}\"", text);
        }
    }

    /// Port of `do_shout(int cn, const char *text)` from `svr_do.cpp`
    ///
    /// Shout a message to all players.
    pub(crate) fn do_shout(&mut self, cn: usize, text: &str) {
        if text.is_empty() {
            self.do_character_log(
                cn,
                core::types::FontColor::Red,
                "Shout. Yes. Shout it will be. But what do you want to shout?\n",
            );
            return;
        }
        if self.characters[cn].a_end < 50000 {
            self.do_character_log(
                cn,
                core::types::FontColor::Red,
                "You're too exhausted to shout!\n",
            );
            return;
        }
        if (self.characters[cn].flags & CharacterFlags::ShutUp.bits()) != 0 {
            self.do_character_log(
                cn,
                core::types::FontColor::Red,
                "You try to shout, but you only produce a croaking sound.\n",
            );
            return;
        }
        self.characters[cn].a_end -= 50000;
        let buf = if (self.characters[cn].flags & CharacterFlags::Invisible.bits()) != 0 {
            format!("Somebody shouts: \"{}\"\n", text)
        } else {
            let name = self.characters[cn].get_name().to_string();
            format!("{} shouts: \"{}\"\n", name, text)
        };

        for n in 1..core::constants::MAXCHARS {
            let send = ((self.characters[n].flags
                & (CharacterFlags::Player.bits() | CharacterFlags::Usurp.bits()))
                != 0
                || self.characters[n].temp == CT_LGUARD as u16)
                && self.characters[n].used == core::constants::USE_ACTIVE;
            if send {
                self.do_character_log(n, core::types::FontColor::Blue, &buf);
            }
        }
        if (self.characters[cn].flags & CharacterFlags::Player.bits()) != 0 {
            log::info!("Shouts \"{}\"", text);
        }
    }

    /// Port of `do_noshout(int cn)` from `svr_do.cpp`
    ///
    /// Toggle whether character hears shouts.
    pub(crate) fn do_noshout(&mut self, cn: usize) {
        self.characters[cn].flags ^= CharacterFlags::NoShout.bits();
        if (self.characters[cn].flags & CharacterFlags::NoShout.bits()) != 0 {
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
    pub(crate) fn do_notell(&mut self, cn: usize) {
        self.characters[cn].flags ^= CharacterFlags::NoTell.bits();
        if (self.characters[cn].flags & CharacterFlags::NoTell.bits()) != 0 {
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
    pub fn do_nostaff(&mut self, cn: usize) {
        self.characters[cn].flags ^= CharacterFlags::NoStaff.bits();
        if (self.characters[cn].flags & CharacterFlags::NoStaff.bits()) != 0 {
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
        if (self.characters[cn].flags & CharacterFlags::Player.bits()) != 0 {
            log::info!(
                "Set nostaff to {}",
                if (self.characters[cn].flags & CharacterFlags::NoStaff.bits()) != 0 {
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
    pub(crate) fn do_is_ignore(&mut self, cn: usize, co: usize, flag: i32) -> i32 {
        if flag == 0 {
            for n in 30..39 {
                if self.characters[co].data[n] as usize == cn {
                    return 1;
                }
            }
        }
        for n in 50..59 {
            if self.characters[co].data[n] as usize == cn {
                return 1;
            }
        }
        0
    }

    /// Port of `do_lookup_char_self(const char *name, int cn)` from `svr_do.cpp`
    ///
    /// Lookup a character by name, supporting "self" keyword.
    pub(crate) fn do_lookup_char_self(&mut self, name: &str, cn: usize) -> i32 {
        if name.eq_ignore_ascii_case("self") {
            return cn as i32;
        }
        self.do_lookup_char(name)
    }

    /// Port of `do_lookup_char(const char *name)` from `svr_do.cpp`
    ///
    /// Lookup a character by name (partial match supported).
    pub(crate) fn do_lookup_char(&mut self, name: &str) -> i32 {
        let len = name.len();
        if len < 2 {
            return 0;
        }
        let matchname = name.to_lowercase();
        let mut bestmatch = 0;
        let mut quality = 0;
        for n in 1..core::constants::MAXCHARS {
            let used = self.characters[n].used;
            if used != core::constants::USE_ACTIVE && used != core::constants::USE_NONACTIVE {
                continue;
            }
            if (self.characters[n].flags & CharacterFlags::Body.bits()) != 0 {
                continue;
            }
            let nm = self.characters[n].get_name().to_lowercase();
            if !nm.starts_with(&matchname) {
                continue;
            }
            if nm.len() == len {
                bestmatch = n;
                break;
            }
            let q = if (self.characters[n].flags
                & (CharacterFlags::Player.bits() | CharacterFlags::Usurp.bits()))
                != 0
            {
                if self.characters[n].x != 0 {
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
