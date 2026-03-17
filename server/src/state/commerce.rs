use core::constants::{CharacterFlags, ItemFlags, TICKS};
use core::skills;
use core::string_operations::c_string_to_str;
use core::types::FontColor;

use crate::driver;
use crate::game_state::GameState;
use crate::god::God;

impl GameState {
    /// Calculates adjusted price based on character's barter skill.
    ///
    /// # Arguments
    ///
    /// * `cn` - Character index
    /// * `opr` - Original price of the item
    /// * `flag` - 1 if merchant is selling (player buying), 0 if merchant is buying (player selling)
    ///
    /// # Returns
    ///
    /// Adjusted price after applying barter skill.
    pub(crate) fn barter(&mut self, cn: usize, opr: i32, flag: i32) -> i32 {
        let barter_skill = self.characters[cn].skill[skills::SK_BARTER][5] as i32;

        let pr = if flag != 0 {
            // Merchant is selling (player is buying)
            // Higher skill = lower price
            let calculated = opr * 4 - (opr * barter_skill) / 50;
            // Price can't go below original price
            if calculated < opr {
                opr
            } else {
                calculated
            }
        } else {
            // Merchant is buying (player is selling)
            // Higher skill = higher price for player
            let calculated = opr / 4 + (opr * barter_skill) / 200;
            // Price can't go above original price
            if calculated > opr {
                opr
            } else {
                calculated
            }
        };

        pr
    }

    /// Handles shopping interactions between a character and a merchant or corpse.
    ///
    /// # Arguments
    ///
    /// * `cn` - Character performing the action (player)
    /// * `co` - Target character (merchant or corpse)
    /// * `nr` - Action selector:
    ///   - 0-39: Buy/take from merchant/corpse inventory
    ///   - 40-59: Take from corpse worn items
    ///   - 60: Take carried item from corpse
    ///   - 61: Take gold from corpse
    ///   - 62+: Examine item descriptions (nr-62 gives item slot)
    pub(crate) fn do_shop_char(&mut self, cn: usize, co: usize, nr: i32) {
        // Validate parameters
        if co == 0 || co >= core::constants::MAXCHARS || !(0..124).contains(&nr) {
            return;
        }

        // Check if target is a merchant or corpse (body)
        let is_merchant = self.characters[co].flags & CharacterFlags::Merchant.bits() != 0;
        let is_body = self.characters[co].flags & CharacterFlags::Body.bits() != 0;

        if !is_merchant && !is_body {
            return;
        }

        // For living merchants, check visibility
        if !is_body {
            if self.do_char_can_see(cn, co) == 0 {
                return;
            }
        }

        // For corpses, check distance (must be adjacent)
        if is_body {
            let cn_x = self.characters[cn].x as i32;
            let cn_y = self.characters[cn].y as i32;
            let co_x = self.characters[co].x as i32;
            let co_y = self.characters[co].y as i32;

            let distance = (cn_x - co_x).abs() + (cn_y - co_y).abs();
            if distance > 1 {
                return;
            }
        }

        // Handle selling to merchant (player has citem)
        let citem = self.characters[cn].citem;

        if citem != 0 && is_merchant {
            // Check if trying to sell money
            if citem & 0x80000000 != 0 {
                self.do_character_log(cn, FontColor::Green, "You want to sell money? Weird!\n");
                return;
            }

            let item_idx = citem as usize;

            // Check if merchant accepts this type of item
            let merchant_template = self.characters[co].data[0] as usize;

            let item_flags = self.items[item_idx].flags;
            let template_flags = self.item_templates[merchant_template].flags;

            let mut accepts = false;
            if (item_flags & ItemFlags::IF_ARMOR.bits() != 0)
                && (template_flags & ItemFlags::IF_ARMOR.bits() != 0)
            {
                accepts = true;
            }
            if (item_flags & ItemFlags::IF_WEAPON.bits() != 0)
                && (template_flags & ItemFlags::IF_WEAPON.bits() != 0)
            {
                accepts = true;
            }
            if (item_flags & ItemFlags::IF_MAGIC.bits() != 0)
                && (template_flags & ItemFlags::IF_MAGIC.bits() != 0)
            {
                accepts = true;
            }
            if (item_flags & ItemFlags::IF_MISC.bits() != 0)
                && (template_flags & ItemFlags::IF_MISC.bits() != 0)
            {
                accepts = true;
            }

            if !accepts {
                let merchant_name = self.characters[co].get_name().to_string();
                self.do_character_log(
                    cn,
                    FontColor::Green,
                    &format!("{} doesn't buy those.\n", merchant_name),
                );
                return;
            }

            // Calculate price with barter
            let value = self.do_item_value(item_idx);
            let price = self.barter(cn, value as i32, 0);

            // Check if merchant can afford it
            let merchant_gold = self.characters[co].gold;
            if merchant_gold < price {
                let merchant_ref = self.characters[co].get_reference().to_string();
                self.do_character_log(
                    cn,
                    FontColor::Green,
                    &format!("{} cannot afford that.\n", merchant_ref),
                );
                return;
            }

            // Complete the sale
            self.characters[cn].citem = 0;
            self.characters[cn].gold += price;

            // Transfer item to merchant
            if !God::give_character_item(self, co, item_idx) {
                log::error!(
                    "do_shop_char: god_give_character_item({}, {}) failed",
                    item_idx,
                    co
                );
                return;
            }

            let item_name = self.items[item_idx].get_name().to_string();

            let item_ref = c_string_to_str(&mut self.items[item_idx].reference).to_string();

            chlog!(
                cn,
                "Sold {} for {}G {}S",
                item_name,
                price / 100,
                price % 100
            );

            self.do_character_log(
                cn,
                FontColor::Yellow,
                &format!(
                    "You sold a {} for {}G {}S.\n",
                    item_ref,
                    price / 100,
                    price % 100
                ),
            );

            // Update item template statistics
            let temp_id = self.items[item_idx].temp as usize;
            if temp_id > 0 && temp_id < core::constants::MAXTITEM {
                log::warn!(
                    "TODO: Should update t_sold for template {} after selling item {}",
                    temp_id,
                    item_name
                );
                // self.item_templates[temp_id].t_sold += 1;
            }
        } else {
            // Handle buying/taking/examining items
            if nr < 62 {
                // Buying or taking items
                if nr < 40 {
                    // Inventory slot
                    let item_idx = self.characters[co].item[nr as usize] as usize;

                    if item_idx != 0 {
                        let price = if is_merchant {
                            let value = self.do_item_value(item_idx);
                            let pr = self.barter(cn, value as i32, 1);

                            let player_gold = self.characters[cn].gold;
                            if player_gold < pr {
                                self.do_character_log(
                                    cn,
                                    FontColor::Green,
                                    "You cannot afford that.\n",
                                );
                                return;
                            }
                            pr
                        } else {
                            0
                        };

                        if !God::take_from_char(self, item_idx, co) {
                            log::error!(
                                "do_shop_char: god_take_from_char({}, {}) failed",
                                item_idx,
                                co
                            );
                        }

                        let gave_success = God::give_character_item(self, cn, item_idx);

                        if gave_success {
                            if is_merchant {
                                self.characters[cn].gold -= price;
                                self.characters[co].gold += price;

                                let item_name = self.items[item_idx].get_name().to_string();
                                let item_ref = c_string_to_str(&mut self.items[item_idx].reference)
                                    .to_string();

                                chlog!(
                                    cn,
                                    "Bought {} for {}G {}S",
                                    item_name,
                                    price / 100,
                                    price % 100
                                );

                                self.do_character_log(
                                    cn,
                                    FontColor::Yellow,
                                    &format!(
                                        "You bought a {} for {}G {}S.\n",
                                        item_ref,
                                        price / 100,
                                        price % 100
                                    ),
                                );

                                // Update template statistics
                                let temp_id = self.items[item_idx].temp as usize;
                                if temp_id > 0 && temp_id < core::constants::MAXTITEM {
                                    log::warn!("TODO: Should update t_bought for template {} after buying item {}", temp_id, item_name);
                                    // self.item_templates[temp_id].t_bought += 1;
                                }
                            } else {
                                let item_name = self.items[item_idx].get_name().to_string();

                                self.do_character_log(
                                    cn,
                                    FontColor::Yellow,
                                    &format!("You took a {}.\n", item_name),
                                );
                            }
                        } else {
                            // Failed to give item - put it back
                            God::give_character_item(self, co, item_idx);

                            let item_ref =
                                c_string_to_str(&mut self.items[item_idx].reference).to_string();

                            if is_merchant {
                                self.do_character_log(
                                    cn,
                                    FontColor::Green,
                                    &format!(
                                        "You cannot buy the {} because your inventory is full.\n",
                                        item_ref
                                    ),
                                );
                            } else {
                                self.do_character_log(
                                    cn,
                                    FontColor::Green,
                                    &format!(
                                        "You cannot take the {} because your inventory is full.\n",
                                        item_ref
                                    ),
                                );
                            }
                        }
                    }
                } else if nr < 60 {
                    // Worn items (only for corpses)
                    if is_body {
                        let worn_slot = (nr - 40) as usize;
                        let item_idx = self.characters[co].worn[worn_slot] as usize;

                        if item_idx != 0 {
                            God::take_from_char(self, item_idx, co);

                            let gave_success = God::give_character_item(self, cn, item_idx);

                            if gave_success {
                                let item_name = self.items[item_idx].get_name().to_string();
                                let item_ref = c_string_to_str(&mut self.items[item_idx].reference)
                                    .to_string();

                                chlog!(cn, "Took {} from corpse", item_name);

                                self.do_character_log(
                                    cn,
                                    FontColor::Yellow,
                                    &format!("You took a {}.\n", item_ref),
                                );
                            } else {
                                // Failed to give item - put it back
                                God::give_character_item(self, co, item_idx);

                                let item_ref = c_string_to_str(&mut self.items[item_idx].reference)
                                    .to_string();

                                self.do_character_log(
                                    cn,
                                    FontColor::Green,
                                    &format!(
                                        "You cannot take the {} because your inventory is full.\n",
                                        item_ref
                                    ),
                                );
                            }
                        }
                    }
                } else if nr == 60 {
                    // Carried item (only for corpses)
                    if is_body {
                        let item_idx = self.characters[co].citem as usize;

                        if item_idx != 0 {
                            if !God::take_from_char(self, item_idx, co) {
                                log::error!(
                                    "do_shop_char: god_take_from_char({}, {}) failed",
                                    item_idx,
                                    co
                                );
                                return;
                            }

                            let gave_success = God::give_character_item(self, cn, item_idx);

                            if gave_success {
                                let item_name = self.items[item_idx].get_name().to_string();
                                let item_ref = c_string_to_str(&mut self.items[item_idx].reference)
                                    .to_string();

                                chlog!(cn, "Took {} from corpse", item_name);

                                self.do_character_log(
                                    cn,
                                    FontColor::Yellow,
                                    &format!("You took a {}.\n", item_ref),
                                );
                            } else {
                                if !God::give_character_item(self, co, item_idx) {
                                    log::error!(
                                        "do_shop_char: god_give_character_item({}, {}) failed",
                                        item_idx,
                                        co
                                    );
                                }

                                let item_ref = c_string_to_str(&mut self.items[item_idx].reference)
                                    .to_string();

                                self.do_character_log(
                                    cn,
                                    FontColor::Green,
                                    &format!(
                                        "You cannot take the {} because your inventory is full.\n",
                                        item_ref
                                    ),
                                );
                            }
                        }
                    }
                } else {
                    // nr == 61: Take gold (only for corpses)
                    if is_body {
                        let corpse_gold = self.characters[co].gold;

                        if corpse_gold > 0 {
                            self.characters[cn].gold += corpse_gold;
                            self.characters[co].gold = 0;

                            chlog!(
                                cn,
                                "Took {}G {}S from corpse",
                                corpse_gold / 100,
                                corpse_gold % 100
                            );

                            self.do_character_log(
                                cn,
                                FontColor::Yellow,
                                &format!(
                                    "You took {}G {}S.\n",
                                    corpse_gold / 100,
                                    corpse_gold % 100
                                ),
                            );
                        }
                    }
                }
            } else {
                // Examine item descriptions (nr >= 62)
                let exam_nr = nr - 62;

                if exam_nr < 40 {
                    // Inventory item description
                    let item_idx = self.characters[co].item[exam_nr as usize] as usize;

                    if item_idx != 0 {
                        let item_name = self.items[item_idx].get_name().to_string();
                        let item_desc =
                            c_string_to_str(&mut self.items[item_idx].description).to_string();

                        self.do_character_log(cn, FontColor::Yellow, &format!("{}:\n", item_name));
                        self.do_character_log(cn, FontColor::Yellow, &format!("{}\n", item_desc));
                    }
                } else if exam_nr < 60 {
                    // Worn item description (only for corpses), slots 0-19
                    // Note: original C used < 61 (exam_nr 40-60 → worn_slot 0-20), which is a
                    // buffer overread; citem description is handled in the else branch below.
                    if is_body {
                        let worn_slot = (exam_nr - 40) as usize;
                        let item_idx = self.characters[co].worn[worn_slot] as usize;

                        if item_idx != 0 {
                            let item_name = self.items[item_idx].get_name().to_string();
                            let item_desc =
                                c_string_to_str(&mut self.items[item_idx].description).to_string();

                            self.do_character_log(
                                cn,
                                FontColor::Yellow,
                                &format!("{}:\n", item_name),
                            );
                            self.do_character_log(
                                cn,
                                FontColor::Yellow,
                                &format!("{}\n", item_desc),
                            );
                        }
                    }
                } else {
                    // Carried item description (only for corpses)
                    if is_body {
                        let item_idx = self.characters[co].citem as usize;

                        if item_idx != 0 {
                            let item_name = self.items[item_idx].get_name().to_string();
                            let item_desc =
                                c_string_to_str(&mut self.items[item_idx].description).to_string();

                            self.do_character_log(
                                cn,
                                FontColor::Yellow,
                                &format!("{}:\n", item_name),
                            );
                            self.do_character_log(
                                cn,
                                FontColor::Yellow,
                                &format!("{}\n", item_desc),
                            );
                        }
                    }
                }
            }
        }

        // Update merchant shop display if applicable
        if is_merchant {
            driver::update_shop(self, co);
        }

        // Refresh the character/corpse display
        self.do_look_char(cn, co, 0, 0, 1);
    }

    /// Port of `do_depot_cost(int in)` from `svr_do.cpp`
    ///
    /// Calculates the storage cost for depositing an item in the depot.
    /// Cost is based on item value, power, and special flags.
    ///
    /// # Arguments
    /// * `item_idx` - The index of the item to calculate depot cost for
    ///
    /// # Returns
    /// * Storage cost in gold per tick
    pub(crate) fn do_depot_cost(&mut self, item_idx: usize) -> i32 {
        if item_idx == 0 || item_idx >= core::constants::MAXITEM {
            return 0;
        }

        let item = &mut self.items[item_idx];

        let mut cost = 1;

        // Add cost based on item value
        cost += item.value as i32 / 1600;

        // Add cost based on item power (cubic formula)
        let power = item.power as i32;
        cost += (power * power * power) / 16000;

        // Items that are destroyed in labyrinth have much higher storage cost
        if item.flags & ItemFlags::IF_LABYDESTROY.bits() != 0 {
            cost += 20000;
        }

        cost
    }

    /// Port of `do_add_depot(int cn, int in)` from `svr_do.cpp`
    ///
    /// Adds an item to a character's depot storage.
    /// Finds the first empty slot in the depot and stores the item there.
    ///
    /// # Arguments
    /// * `cn` - Character index
    /// * `item_idx` - The index of the item to add to depot
    ///
    /// # Returns
    /// * `true` - Item was successfully added to depot
    /// * `false` - Depot is full (all 62 slots occupied)
    pub(crate) fn do_add_depot(&mut self, cn: usize, item_idx: usize) -> bool {
        // Find first empty depot slot
        let empty_slot = (0..62).find(|&n| self.characters[cn].depot[n] == 0);

        // If no empty slot found, depot is full
        let slot = match empty_slot {
            Some(n) => n,
            None => return false,
        };

        // Add item to depot slot
        self.characters[cn].depot[slot] = item_idx as u32;
        self.characters[cn].set_do_update_flags();

        true
    }

    /// Port of `do_pay_depot(int cn)` from `svr_do.cpp`
    ///
    /// Handles depot storage fee payment. If the character doesn't have enough gold in
    /// their bank account (data[13]), this function automatically sells the least valuable
    /// items from the depot to cover the storage costs.
    ///
    /// # Arguments
    /// * `cn` - Character index
    ///
    /// # Process
    /// 1. Calculate total depot storage cost
    /// 2. If not enough gold in bank account, sell cheapest depot items until enough funds
    /// 3. Deduct storage cost from bank account
    /// 4. Track total depot costs paid
    pub(crate) fn do_pay_depot(&mut self, cn: usize) {
        loop {
            // Calculate total cost for all items in depot
            let total_cost = self.get_depot_cost(cn);

            let bank_balance = self.characters[cn].data[13];

            if total_cost > bank_balance {
                // Not enough money - find and sell cheapest item
                let mut cheapest_value = 99999999;
                let mut cheapest_slot = None;

                for n in 0..62 {
                    let item_idx = self.characters[cn].depot[n];
                    if item_idx != 0 {
                        let value = self.do_item_value(item_idx as usize);
                        if value < cheapest_value {
                            cheapest_value = value;
                            cheapest_slot = Some(n);
                        }
                    }
                }

                // If no items to sell, panic
                let slot = match cheapest_slot {
                    Some(n) => n,
                    None => {
                        log::error!("PANIC: depot forced sale failed for cn={}", cn);
                        return;
                    }
                };

                // Sell the item for half its value
                let sell_value = cheapest_value / 2;

                let item_idx = self.characters[cn].depot[slot];

                // Add proceeds to bank account
                self.characters[cn].data[13] += sell_value as i32;

                // Mark item as empty (destroyed)
                self.items[item_idx as usize].used = core::constants::USE_EMPTY;

                // Remove item from depot
                self.characters[cn].depot[slot] = 0;
                self.characters[cn].depot_sold += 1;

                let item_name = self.items[item_idx as usize].get_name().to_string();

                chlog!(
                    cn,
                    "Bank sold {} for {}G {}S to pay for depot (slot {})",
                    item_name,
                    sell_value / 100,
                    sell_value % 100,
                    slot
                );
            } else {
                // Enough money - pay the cost
                self.characters[cn].data[13] -= total_cost;
                self.characters[cn].depot_cost += total_cost;
                break;
            }
        }
    }

    /// Helper function to calculate total depot storage cost
    ///
    /// Sums up the storage cost for all items currently in the depot.
    ///
    /// # Arguments
    /// * `cn` - Character index
    ///
    /// # Returns
    /// * Total storage cost for all depot items
    pub(crate) fn get_depot_cost(&mut self, cn: usize) -> i32 {
        let mut total = 0;
        for n in 0..62 {
            let item_idx = self.characters[cn].depot[n];
            if item_idx != 0 {
                total += self.do_depot_cost(item_idx as usize);
            }
        }
        total
    }

    /// Port of `do_depot_char(int cn, int co, int nr)` from `svr_do.cpp`
    ///
    /// Handles depot (bank storage) interactions for a character.
    /// Allows depositing items into depot, withdrawing items, and examining items.
    ///
    /// # Arguments
    /// * `cn` - Character performing the action
    /// * `co` - Target character (must be same as cn for depot)
    /// * `nr` - Action selector:
    ///   - 0-61: Withdraw item from depot slot
    ///   - 62+: Examine item in depot (nr-62 gives slot)
    ///   - If character has citem: Deposit that item
    pub(crate) fn do_depot_char(&mut self, cn: usize, co: usize, nr: i32) {
        // Validate parameters
        if co == 0 || co >= core::constants::MAXCHARS || !(0..124).contains(&nr) {
            return;
        }

        // Can only access own depot
        if cn != co {
            return;
        }

        // Check if in a bank or is god
        let char_x = self.characters[cn].x;
        let char_y = self.characters[cn].y;
        let is_god = self.characters[cn].flags & CharacterFlags::God.bits() != 0;

        if !is_god {
            let map_idx = char_x as usize + char_y as usize * core::constants::SERVER_MAPX as usize;
            let in_bank = self.map[map_idx].flags & core::constants::MF_BANK as u64 != 0;

            if !in_bank {
                self.do_character_log(
                    cn,
                    FontColor::Green,
                    "You cannot access your depot outside a bank.\n",
                );
                return;
            }
        }

        let citem = self.characters[cn].citem;

        if citem != 0 {
            // Depositing an item
            if citem & 0x80000000 != 0 {
                self.do_character_log(
                    cn,
                    FontColor::Green,
                    "Use #deposit to put money in the bank!\n",
                );
                return;
            }

            let item_idx = citem as usize;

            // Check if allowed to deposit
            if !self.do_maygive(cn, 0, item_idx) {
                self.do_character_log(cn, FontColor::Green, "You are not allowed to do that!\n");
                return;
            }

            let has_nodepot = self.items[item_idx].flags & ItemFlags::IF_NODEPOT.bits() != 0;

            if has_nodepot {
                self.do_character_log(cn, FontColor::Green, "You are not allowed to do that!\n");
                return;
            }

            // Calculate storage cost
            let storage_cost = self.do_depot_cost(item_idx);

            // Try to add to depot
            if self.do_add_depot(co, item_idx) {
                self.characters[cn].citem = 0;

                let item_ref = c_string_to_str(&mut self.items[item_idx].reference).to_string();

                let item_name = self.items[item_idx].get_name().to_string();

                // Calculate costs per day (Astonian and Earth)
                let astonian_cost = storage_cost;
                let earth_cost = storage_cost * TICKS; // TICKS*Astonian days per Earth day

                self.do_character_log(
                    cn,
                    FontColor::Yellow,
                    &format!(
                        "You deposited {}. The rent is {}G {}S per Astonian day or {}G {}S per Earth day.\n",
                        item_ref,
                        astonian_cost / 100,
                        astonian_cost % 100,
                        earth_cost / 100,
                        earth_cost % 100
                    ),
                );

                chlog!(
                    cn,
                    "Deposited {} into depot (cost {}G {}S per tick)",
                    item_name,
                    storage_cost / 100,
                    storage_cost % 100
                );
            }
        } else {
            // Withdrawing or examining items
            if nr < 62 {
                // Withdraw item from depot
                let item_idx = self.characters[co].depot[nr as usize];

                if item_idx != 0 {
                    let gave_success = God::give_character_item(self, cn, item_idx as usize);

                    if gave_success {
                        self.characters[co].depot[nr as usize] = 0;

                        let item_ref =
                            c_string_to_str(&mut self.items[item_idx as usize].reference)
                                .to_string();

                        let item_name = self.items[item_idx as usize].get_name().to_string();

                        self.do_character_log(
                            cn,
                            FontColor::Yellow,
                            &format!("You took the {} from your depot.\n", item_ref),
                        );

                        chlog!(cn, "Took {} from depot", item_name);
                    } else {
                        let item_ref =
                            c_string_to_str(&mut self.items[item_idx as usize].reference)
                                .to_string();

                        self.do_character_log(
                            cn,
                            FontColor::Green,
                            &format!(
                                "You cannot take the {} because your inventory is full.\n",
                                item_ref
                            ),
                        );
                    }
                }
            } else {
                // Examine item in depot
                let exam_slot = (nr - 62) as usize;
                let item_idx = self.characters[co].depot[exam_slot];

                if item_idx != 0 {
                    let item_name = self.items[item_idx as usize].get_name().to_string();
                    let item_desc =
                        c_string_to_str(&mut self.items[item_idx as usize].description).to_string();

                    self.do_character_log(cn, FontColor::Yellow, &format!("{}:\n", item_name));
                    self.do_character_log(cn, FontColor::Yellow, &format!("{}\n", item_desc));
                }
            }
        }
    }

    /// Displays depot information banner to the player.
    ///
    /// Shows introductory text about the depot storage system.
    ///
    /// # Arguments
    /// * `cn` - Character index
    pub(crate) fn do_depot(&mut self, cn: usize) {
        self.do_character_log(cn, core::types::FontColor::Yellow, "This is your bank depot. You can store up to 62 items here. But you have to pay a rent for each item.\n");
        // Match original `do_depot`: immediately open the depot (shop-style) UI.
        self.do_look_depot(cn, cn);
    }
}
