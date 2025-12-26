use core::types::FontColor;

use crate::{
    enums::CharacterFlags, god::God, helpers, network_manager::NetworkManager,
    repository::Repository, state::State,
};

impl State {
    /// Port of `do_look_depot(int cn, int co)` from `svr_do.cpp`
    ///
    /// Displays the depot (bank storage) interface to a character.
    /// This sends binary packets to the client showing:
    /// - Character stats and sprite
    /// - Depot storage slots (62 slots)
    /// - Storage costs for each item
    /// - Cost for depositing carried item (if any)
    ///
    /// The display uses a special flag (0x8000) in the character ID to indicate depot view.
    ///
    /// # Arguments
    /// * `cn` - Character viewing the depot
    /// * `co` - Target character (must be same as cn)
    pub(crate) fn do_look_depot(&self, cn: usize, co: usize) {
        // Validate parameters
        if co == 0 || co >= core::constants::MAXCHARS {
            return;
        }

        // Can only view own depot
        if cn != co {
            return;
        }

        // Check if in a bank or is god
        let (char_x, char_y, is_god, player_id) = Repository::with_characters(|ch| {
            (
                ch[cn].x,
                ch[cn].y,
                ch[cn].flags & CharacterFlags::God.bits() != 0,
                ch[cn].player,
            )
        });

        if player_id == 0 {
            return;
        }

        if !is_god {
            let map_idx = char_x as usize + char_y as usize * core::constants::SERVER_MAPX as usize;
            let in_bank = Repository::with_map(|map| {
                map[map_idx].flags & core::constants::MF_BANK as u64 != 0
            });

            if !in_bank {
                self.do_character_log(
                    cn,
                    FontColor::Red,
                    "You cannot access your depot outside a bank.\n",
                );
                return;
            }
        }

        let mut buf = [0u8; 16];

        // Send SV_LOOK1 packet - all equipment slots obscured (sprite 35)
        buf[0] = core::constants::SV_LOOK1;
        for i in 0..7 {
            let offset = 1 + i * 2;
            buf[offset] = 35;
            buf[offset + 1] = 0;
        }
        buf[15] = 0;

        NetworkManager::with(|network| {
            network.xsend(player_id as usize, &buf, 16);
        });

        // Send SV_LOOK2 packet
        buf[0] = core::constants::SV_LOOK2;

        let (sprite, points_tot, hp5) =
            Repository::with_characters(|ch| (ch[co].sprite, ch[co].points_tot, ch[co].hp[5]));

        buf[1] = 35;
        buf[2] = 0;
        buf[13] = 35;
        buf[14] = 0;

        buf[3] = (sprite & 0xFF) as u8;
        buf[4] = (sprite >> 8) as u8;

        let points_bytes = points_tot.to_le_bytes();
        buf[5..9].copy_from_slice(&points_bytes);

        let hp_bytes = (hp5 as u32).to_le_bytes();
        buf[9..13].copy_from_slice(&hp_bytes);

        NetworkManager::with(|network| {
            network.xsend(player_id as usize, &buf, 16);
        });

        // Send SV_LOOK3 packet
        buf[0] = core::constants::SV_LOOK3;

        let (end5, a_hp, a_end, mana5, a_mana, co_id) = Repository::with_characters(|ch| {
            (
                ch[co].end[5],
                ch[co].a_hp,
                ch[co].a_end,
                ch[co].mana[5],
                ch[co].a_mana,
                helpers::char_id(co),
            )
        });

        buf[1] = (end5 & 0xFF) as u8;
        buf[2] = (end5 >> 8) as u8;

        let ahp_display = ((a_hp + 500) / 1000) as u16;
        buf[3] = (ahp_display & 0xFF) as u8;
        buf[4] = (ahp_display >> 8) as u8;

        let aend_display = ((a_end + 500) / 1000) as u16;
        buf[5] = (aend_display & 0xFF) as u8;
        buf[6] = (aend_display >> 8) as u8;

        // Special flag: co | 0x8000 indicates depot view
        let co_with_flag = (co as u16) | 0x8000;
        buf[7] = (co_with_flag & 0xFF) as u8;
        buf[8] = (co_with_flag >> 8) as u8;

        let co_id_u16 = co_id as u16;
        buf[9] = (co_id_u16 & 0xFF) as u8;
        buf[10] = (co_id_u16 >> 8) as u8;

        buf[11] = (mana5 & 0xFF) as u8;
        buf[12] = (mana5 >> 8) as u8;

        let amana_display = ((a_mana + 500) / 1000) as u16;
        buf[13] = (amana_display & 0xFF) as u8;
        buf[14] = (amana_display >> 8) as u8;

        NetworkManager::with(|network| {
            network.xsend(player_id as usize, &buf, 16);
        });

        // Send SV_LOOK4 packet
        buf[0] = core::constants::SV_LOOK4;

        // All equipment slots obscured
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

        // Show depot interface (flag = 1)
        buf[5] = 1;

        // Show cost for depositing carried item (if valid)
        let citem = Repository::with_characters(|ch| ch[cn].citem);
        let deposit_cost = if citem > 0 && citem < core::constants::MAXITEM as u32 {
            let item_cost = self.do_depot_cost(citem as usize);
            (core::constants::TICKS * item_cost) as u32
        } else {
            0
        };

        let cost_bytes = deposit_cost.to_le_bytes();
        buf[6..10].copy_from_slice(&cost_bytes);

        NetworkManager::with(|network| {
            network.xsend(player_id as usize, &buf, 16);
        });

        // Send SV_LOOK5 packet (character name)
        buf[0] = core::constants::SV_LOOK5;

        let co_name = Repository::with_characters(|ch| {
            let mut name = [0u8; 15];
            name.copy_from_slice(&ch[co].name[0..15]);
            name
        });

        buf[1..16].copy_from_slice(&co_name);

        NetworkManager::with(|network| {
            network.xsend(player_id as usize, &buf, 16);
        });

        // Send SV_LOOK6 packets for all 62 depot slots in pairs
        for n in (0..62).step_by(2) {
            buf[0] = core::constants::SV_LOOK6;
            buf[1] = n as u8;

            for m in n..std::cmp::min(62, n + 2) {
                let (sprite, cost) = Repository::with_characters(|ch| {
                    let item_idx = ch[co].depot[m];
                    if item_idx != 0 {
                        let spr =
                            Repository::with_items(|items| items[item_idx as usize].sprite[0]);
                        let item_cost = self.do_depot_cost(item_idx as usize);
                        let total_cost = (core::constants::TICKS * item_cost) as u32;
                        (spr, total_cost)
                    } else {
                        (0, 0)
                    }
                });

                let offset = 2 + (m - n) * 6;
                buf[offset] = (sprite & 0xFF) as u8;
                buf[offset + 1] = (sprite >> 8) as u8;

                let cost_bytes = cost.to_le_bytes();
                buf[offset + 2..offset + 6].copy_from_slice(&cost_bytes);
            }

            NetworkManager::with(|network| {
                network.xsend(player_id as usize, &buf, 16);
            });
        }
    }

    /// Port of `do_look_player_depot(int cn, char* cv)` from `svr_do.cpp`
    ///
    /// Debug/admin command to view another player's depot contents.
    /// Lists all items in the target character's depot with item IDs and names.
    ///
    /// # Arguments
    /// * `cn` - Character issuing the command (admin/god)
    /// * `cv` - Character ID string to look up
    pub(crate) fn do_look_player_depot(&self, cn: usize, cv: &str) {
        // Parse character ID from string
        let co = match cv.trim().parse::<usize>() {
            Ok(id) => id,
            Err(_) => {
                self.do_character_log(cn, FontColor::Red, &format!("Bad character: {}!\n", cv));
                return;
            }
        };

        // Validate character ID
        if co == 0 || co >= core::constants::MAXCHARS {
            self.do_character_log(cn, FontColor::Red, &format!("Bad character: {}!\n", cv));
            return;
        }

        let (co_name, depot_items) = Repository::with_characters(|ch| {
            let name = String::from_utf8_lossy(&ch[co].name).to_string();
            let mut items = Vec::new();

            for m in 0..62 {
                let item_idx = ch[co].depot[m];
                if item_idx != 0 {
                    let item_name = Repository::with_items(|items| {
                        String::from_utf8_lossy(&items[item_idx as usize].name).to_string()
                    });
                    items.push((item_idx, item_name));
                }
            }

            (name, items)
        });

        self.do_character_log(
            cn,
            FontColor::Yellow,
            &format!("Depot contents for : {}\n", co_name),
        );
        self.do_character_log(
            cn,
            FontColor::Yellow,
            "-----------------------------------\n",
        );

        for (item_idx, item_name) in &depot_items {
            self.do_character_log(
                cn,
                FontColor::Yellow,
                &format!("{:6}: {}\n", item_idx, item_name),
            );
        }

        self.do_character_log(cn, FontColor::Yellow, " \n");
        self.do_character_log(
            cn,
            FontColor::Yellow,
            &format!("Total : {} items.\n", depot_items.len()),
        );
    }

    /// Port of `do_look_player_inventory(int cn, char* cv)` from `svr_do.cpp`
    ///
    /// Debug/admin command to view another player's inventory contents.
    /// Lists all items in the target character's inventory (40 slots) with item IDs and names.
    ///
    /// # Arguments
    /// * `cn` - Character issuing the command (admin/god)
    /// * `cv` - Character ID string to look up
    pub fn do_look_player_inventory(&self, cn: usize, cv: &str) {
        // Parse character ID from string
        let co = match cv.trim().parse::<usize>() {
            Ok(id) => id,
            Err(_) => {
                self.do_character_log(cn, FontColor::Red, &format!("Bad character: {}!\n", cv));
                return;
            }
        };

        // Validate character ID
        if co == 0 || co >= core::constants::MAXCHARS {
            self.do_character_log(cn, FontColor::Red, &format!("Bad character: {}!\n", cv));
            return;
        }

        let (co_name, inventory_items) = Repository::with_characters(|ch| {
            let name = String::from_utf8_lossy(&ch[co].name).to_string();
            let mut items = Vec::new();

            for n in 0..40 {
                let item_idx = ch[co].item[n];
                if item_idx != 0 {
                    let item_name = Repository::with_items(|it| {
                        String::from_utf8_lossy(&it[item_idx as usize].name).to_string()
                    });
                    items.push((item_idx, item_name));
                }
            }

            (name, items)
        });

        self.do_character_log(
            cn,
            FontColor::Yellow,
            &format!("Inventory contents for : {}\n", co_name),
        );
        self.do_character_log(
            cn,
            FontColor::Yellow,
            "-----------------------------------\n",
        );

        for (item_idx, item_name) in &inventory_items {
            self.do_character_log(
                cn,
                FontColor::Yellow,
                &format!("{:6}: {}\n", item_idx, item_name),
            );
        }

        self.do_character_log(cn, FontColor::Yellow, " \n");
        self.do_character_log(
            cn,
            FontColor::Yellow,
            &format!("Total : {} items.\n", inventory_items.len()),
        );
    }

    /// Port of `do_look_player_equipment(int cn, char* cv)` from `svr_do.cpp`
    ///
    /// Debug/admin command to view another player's equipment.
    /// Lists all items in the target character's worn equipment (20 slots) with item IDs and names.
    ///
    /// # Arguments
    /// * `cn` - Character issuing the command (admin/god)
    /// * `cv` - Character ID string to look up
    pub(crate) fn do_look_player_equipment(&self, cn: usize, cv: &str) {
        // Parse character ID from string
        let co = match cv.trim().parse::<usize>() {
            Ok(id) => id,
            Err(_) => {
                self.do_character_log(cn, FontColor::Red, &format!("Bad character: {}!\n", cv));
                return;
            }
        };

        // Validate character ID
        if co == 0 || co >= core::constants::MAXCHARS {
            self.do_character_log(cn, FontColor::Red, &format!("Bad character: {}!\n", cv));
            return;
        }

        let (co_name, equipment_items) = Repository::with_characters(|ch| {
            let name = String::from_utf8_lossy(&ch[co].name).to_string();
            let mut items = Vec::new();

            for n in 0..20 {
                let item_idx = ch[co].worn[n];
                if item_idx != 0 {
                    let item_name = Repository::with_items(|it| {
                        String::from_utf8_lossy(&it[item_idx as usize].name).to_string()
                    });
                    items.push((item_idx, item_name));
                }
            }

            (name, items)
        });

        self.do_character_log(
            cn,
            FontColor::Yellow,
            &format!("Equipment for : {}\n", co_name),
        );
        self.do_character_log(
            cn,
            FontColor::Yellow,
            "-----------------------------------\n",
        );

        for (item_idx, item_name) in &equipment_items {
            self.do_character_log(
                cn,
                FontColor::Yellow,
                &format!("{:6}: {}\n", item_idx, item_name),
            );
        }

        self.do_character_log(cn, FontColor::Yellow, " \n");
        self.do_character_log(
            cn,
            FontColor::Yellow,
            &format!("Total : {} items.\n", equipment_items.len()),
        );
    }

    /// Port of `do_steal_player(int cn, char* cv, char* ci)` from `svr_do.cpp`
    ///
    /// Debug/admin command to steal an item from a player.
    /// Searches through the target's inventory, depot, and worn equipment for the specified item.
    /// If found, transfers the item to the admin character using god_give_char.
    ///
    /// # Arguments
    /// * `cn` - Character issuing the command (admin/god)
    /// * `cv` - Character ID string to steal from
    /// * `ci` - Item ID string to steal
    ///
    /// # Returns
    /// * `true` - Item was successfully stolen
    /// * `false` - Item not found or transfer failed
    pub fn do_steal_player(&self, cn: usize, cv: &str, ci: &str) -> bool {
        // Parse character ID from string
        let co = match cv.trim().parse::<usize>() {
            Ok(id) => id,
            Err(_) => {
                self.do_character_log(cn, FontColor::Red, &format!("Bad character: {}!\n", cv));
                return false;
            }
        };

        // Parse item ID from string
        let item_id = match ci.trim().parse::<u32>() {
            Ok(id) => id,
            Err(_) => return false,
        };

        // Validate character ID
        if co == 0 || co >= core::constants::MAXCHARS {
            self.do_character_log(cn, FontColor::Red, &format!("Bad character: {}!\n", cv));
            return false;
        }

        if item_id == 0 {
            return false;
        }

        // Search through inventory (40 slots)
        let mut found_location: Option<(usize, &str)> = None;

        Repository::with_characters(|ch| {
            for n in 0..40 {
                if ch[co].item[n] == item_id {
                    found_location = Some((n, "inventory"));
                    return;
                }
            }

            // Search through depot (62 slots) if not found in inventory
            for n in 0..62 {
                if ch[co].depot[n] == item_id {
                    found_location = Some((n, "depot"));
                    return;
                }
            }

            // Search through worn equipment (20 slots) if not found elsewhere
            for n in 0..20 {
                if ch[co].worn[n] == item_id {
                    found_location = Some((n, "worn"));
                    return;
                }
            }
        });

        if let Some((slot_index, location)) = found_location {
            // Try to give the item to the admin character
            if God::give_character_item(cn, item_id as usize) {
                // Remove item from target's slot
                Repository::with_characters_mut(|ch| match location {
                    "inventory" => ch[co].item[slot_index] = 0,
                    "depot" => ch[co].depot[slot_index] = 0,
                    "worn" => ch[co].worn[slot_index] = 0,
                    _ => {}
                });

                // Get item reference and character name for logging
                let (item_reference, co_name) = Repository::with_items(|it| {
                    let item_ref =
                        String::from_utf8_lossy(&it[item_id as usize].reference).to_string();
                    let char_name = Repository::with_characters(|ch| {
                        String::from_utf8_lossy(&ch[co].name).to_string()
                    });
                    (item_ref, char_name)
                });

                self.do_character_log(
                    cn,
                    FontColor::Yellow,
                    &format!("You stole {} from {}.\n", item_reference, co_name),
                );
                true
            } else {
                // Inventory full
                let item_reference = Repository::with_items(|it| {
                    String::from_utf8_lossy(&it[item_id as usize].reference).to_string()
                });

                self.do_character_log(
                    cn,
                    FontColor::Red,
                    &format!(
                        "You cannot take the {} because your inventory is full.\n",
                        item_reference
                    ),
                );
                false
            }
        } else {
            // Item not found
            self.do_character_log(cn, FontColor::Red, "Item not found.\n");
            false
        }
    }
}
