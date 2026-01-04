use core::constants::{CharacterFlags, ItemFlags};
use core::string_operations::c_string_to_str;
use core::types::FontColor;

use crate::driver;
use crate::god::God;
use crate::repository::Repository;
use crate::state::State;

impl State {
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
    pub(crate) fn barter(&self, cn: usize, opr: i32, flag: i32) -> i32 {
        let barter_skill =
            Repository::with_characters(|ch| ch[cn].skill[core::constants::SK_BARTER][5] as i32);

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
        let (is_merchant, is_body) = Repository::with_characters(|ch| {
            (
                ch[co].flags & CharacterFlags::CF_MERCHANT.bits() != 0,
                ch[co].flags & CharacterFlags::CF_BODY.bits() != 0,
            )
        });

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
            let (cn_x, cn_y, co_x, co_y) = Repository::with_characters(|ch| {
                (
                    ch[cn].x as i32,
                    ch[cn].y as i32,
                    ch[co].x as i32,
                    ch[co].y as i32,
                )
            });

            let distance = (cn_x - co_x).abs() + (cn_y - co_y).abs();
            if distance > 1 {
                return;
            }
        }

        // Handle selling to merchant (player has citem)
        let citem = Repository::with_characters(|ch| ch[cn].citem);

        if citem != 0 && is_merchant {
            // Check if trying to sell money
            if citem & 0x80000000 != 0 {
                self.do_character_log(cn, FontColor::Green, "You want to sell money? Weird!\n");
                return;
            }

            let item_idx = citem as usize;

            // Check if merchant accepts this type of item
            let merchant_template = Repository::with_characters(|ch| ch[co].data[0] as usize);

            let (item_flags, template_flags) = Repository::with_items(|items| {
                Repository::with_item_templates(|templates| {
                    (items[item_idx].flags, templates[merchant_template].flags)
                })
            });

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
                let merchant_name = Repository::with_characters(|ch| ch[co].get_name().to_string());
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
            let merchant_gold = Repository::with_characters(|ch| ch[co].gold);
            if merchant_gold < price {
                let merchant_ref =
                    Repository::with_characters(|ch| ch[co].get_reference().to_string());
                self.do_character_log(
                    cn,
                    FontColor::Green,
                    &format!("{} cannot afford that.\n", merchant_ref),
                );
                return;
            }

            // Complete the sale
            Repository::with_characters_mut(|ch| {
                ch[cn].citem = 0;
                ch[cn].gold += price;
            });

            // Transfer item to merchant
            if !God::give_character_item(co, item_idx) {
                log::error!(
                    "do_shop_char: god_give_character_item({}, {}) failed",
                    item_idx,
                    co
                );
                return;
            }

            let item_name = Repository::with_items(|items| items[item_idx].get_name().to_string());

            let item_ref = Repository::with_items(|items| {
                c_string_to_str(&items[item_idx].reference).to_string()
            });

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
            let temp_id = Repository::with_items(|items| items[item_idx].temp as usize);
            if temp_id > 0 && temp_id < core::constants::MAXTITEM {
                Repository::with_item_templates_mut(|templates| {
                    templates[temp_id].t_sold += 1;
                });
            }
        } else {
            // Handle buying/taking/examining items
            if nr < 62 {
                // Buying or taking items
                if nr < 40 {
                    // Inventory slot
                    let item_idx =
                        Repository::with_characters(|ch| ch[co].item[nr as usize] as usize);

                    if item_idx != 0 {
                        let price = if is_merchant {
                            let value = self.do_item_value(item_idx);
                            let pr = self.barter(cn, value as i32, 1);

                            let player_gold = Repository::with_characters(|ch| ch[cn].gold);
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

                        if !God::take_from_char(item_idx, co) {
                            log::error!(
                                "do_shop_char: god_take_from_char({}, {}) failed",
                                item_idx,
                                co
                            );
                        }

                        let gave_success = God::give_character_item(cn, item_idx);

                        if gave_success {
                            if is_merchant {
                                Repository::with_characters_mut(|ch| {
                                    ch[cn].gold -= price;
                                    ch[co].gold += price;
                                });

                                let item_name = Repository::with_items(|items| {
                                    items[item_idx].get_name().to_string()
                                });
                                let item_ref = Repository::with_items(|items| {
                                    c_string_to_str(&items[item_idx].reference).to_string()
                                });

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
                                let temp_id =
                                    Repository::with_items(|items| items[item_idx].temp as usize);
                                if temp_id > 0 && temp_id < core::constants::MAXTITEM {
                                    Repository::with_item_templates_mut(|templates| {
                                        templates[temp_id].t_bought += 1;
                                    });
                                }
                            } else {
                                let item_name = Repository::with_items(|items| {
                                    items[item_idx].get_name().to_string()
                                });

                                self.do_character_log(
                                    cn,
                                    FontColor::Yellow,
                                    &format!("You took a {}.\n", item_name),
                                );
                            }
                        } else {
                            // Failed to give item - put it back
                            God::give_character_item(co, item_idx);

                            let item_ref = Repository::with_items(|items| {
                                c_string_to_str(&items[item_idx].reference).to_string()
                            });

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
                        let item_idx =
                            Repository::with_characters(|ch| ch[co].worn[worn_slot] as usize);

                        if item_idx != 0 {
                            God::take_from_char(item_idx, co);

                            let gave_success = God::give_character_item(cn, item_idx);

                            if gave_success {
                                let item_name = Repository::with_items(|items| {
                                    items[item_idx].get_name().to_string()
                                });
                                let item_ref = Repository::with_items(|items| {
                                    c_string_to_str(&items[item_idx].reference).to_string()
                                });

                                chlog!(cn, "Took {} from corpse", item_name);

                                self.do_character_log(
                                    cn,
                                    FontColor::Yellow,
                                    &format!("You took a {}.\n", item_ref),
                                );
                            } else {
                                // Failed to give item - put it back
                                God::give_character_item(co, item_idx);

                                let item_ref = Repository::with_items(|items| {
                                    c_string_to_str(&items[item_idx].reference).to_string()
                                });

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
                        let item_idx = Repository::with_characters(|ch| ch[co].citem as usize);

                        if item_idx != 0 {
                            if !God::take_from_char(item_idx, co) {
                                log::error!(
                                    "do_shop_char: god_take_from_char({}, {}) failed",
                                    item_idx,
                                    co
                                );
                                return;
                            }

                            let gave_success = God::give_character_item(cn, item_idx);

                            if gave_success {
                                let item_name = Repository::with_items(|items| {
                                    items[item_idx].get_name().to_string()
                                });
                                let item_ref = Repository::with_items(|items| {
                                    c_string_to_str(&items[item_idx].reference).to_string()
                                });

                                chlog!(cn, "Took {} from corpse", item_name);

                                self.do_character_log(
                                    cn,
                                    FontColor::Yellow,
                                    &format!("You took a {}.\n", item_ref),
                                );
                            } else {
                                if !God::give_character_item(co, item_idx) {
                                    log::error!(
                                        "do_shop_char: god_give_character_item({}, {}) failed",
                                        item_idx,
                                        co
                                    );
                                }

                                let item_ref = Repository::with_items(|items| {
                                    c_string_to_str(&items[item_idx].reference).to_string()
                                });

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
                        let corpse_gold = Repository::with_characters(|ch| ch[co].gold);

                        if corpse_gold > 0 {
                            Repository::with_characters_mut(|ch| {
                                ch[cn].gold += corpse_gold;
                                ch[co].gold = 0;
                            });

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
                    let item_idx =
                        Repository::with_characters(|ch| ch[co].item[exam_nr as usize] as usize);

                    if item_idx != 0 {
                        let (item_name, item_desc) = Repository::with_items(|items| {
                            (
                                items[item_idx].get_name().to_string(),
                                c_string_to_str(&items[item_idx].description).to_string(),
                            )
                        });

                        self.do_character_log(cn, FontColor::Yellow, &format!("{}:\n", item_name));
                        self.do_character_log(cn, FontColor::Yellow, &format!("{}\n", item_desc));
                    }
                } else if exam_nr < 61 {
                    // Worn item description (only for corpses)
                    if is_body {
                        let worn_slot = (exam_nr - 40) as usize;
                        let item_idx =
                            Repository::with_characters(|ch| ch[co].worn[worn_slot] as usize);

                        if item_idx != 0 {
                            let (item_name, item_desc) = Repository::with_items(|items| {
                                (
                                    items[item_idx].get_name().to_string(),
                                    c_string_to_str(&items[item_idx].description).to_string(),
                                )
                            });

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
                        let item_idx = Repository::with_characters(|ch| ch[co].citem as usize);

                        if item_idx != 0 {
                            let (item_name, item_desc) = Repository::with_items(|items| {
                                (
                                    items[item_idx].get_name().to_string(),
                                    c_string_to_str(&items[item_idx].description).to_string(),
                                )
                            });

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
            driver::update_shop(co);
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
    pub(crate) fn do_depot_cost(&self, item_idx: usize) -> i32 {
        if item_idx == 0 || item_idx >= core::constants::MAXITEM {
            return 0;
        }

        Repository::with_items(|items| {
            let item = &items[item_idx];

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
        })
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
    pub(crate) fn do_add_depot(&self, cn: usize, item_idx: usize) -> bool {
        // Find first empty depot slot
        let empty_slot = Repository::with_characters(|ch| (0..62).find(|&n| ch[cn].depot[n] == 0));

        // If no empty slot found, depot is full
        let slot = match empty_slot {
            Some(n) => n,
            None => return false,
        };

        // Add item to depot slot
        Repository::with_characters_mut(|ch| {
            ch[cn].depot[slot] = item_idx as u32;
            ch[cn].set_do_update_flags();
        });

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
    pub(crate) fn do_pay_depot(&self, cn: usize) {
        loop {
            // Calculate total cost for all items in depot
            let total_cost = self.get_depot_cost(cn);

            let bank_balance = Repository::with_characters(|ch| ch[cn].data[13]);

            if total_cost > bank_balance {
                // Not enough money - find and sell cheapest item
                let (cheapest_value, cheapest_slot) = Repository::with_characters(|ch| {
                    let mut lowest_value = 99999999;
                    let mut lowest_slot = None;

                    for n in 0..62 {
                        let item_idx = ch[cn].depot[n];
                        if item_idx != 0 {
                            let value = self.do_item_value(item_idx as usize);
                            if value < lowest_value {
                                lowest_value = value;
                                lowest_slot = Some(n);
                            }
                        }
                    }

                    (lowest_value, lowest_slot)
                });

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

                let item_idx = Repository::with_characters(|ch| ch[cn].depot[slot]);

                // Add proceeds to bank account
                Repository::with_characters_mut(|ch| {
                    ch[cn].data[13] += sell_value as i32;
                });

                // Mark item as empty (destroyed)
                Repository::with_items_mut(|items| {
                    items[item_idx as usize].used = core::constants::USE_EMPTY;
                });

                // Remove item from depot
                Repository::with_characters_mut(|ch| {
                    ch[cn].depot[slot] = 0;
                    ch[cn].depot_sold += 1;
                });

                let item_name =
                    Repository::with_items(|items| items[item_idx as usize].get_name().to_string());

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
                Repository::with_characters_mut(|ch| {
                    ch[cn].data[13] -= total_cost;
                    ch[cn].depot_cost += total_cost;
                });
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
    pub(crate) fn get_depot_cost(&self, cn: usize) -> i32 {
        Repository::with_characters(|ch| {
            let mut total = 0;
            for n in 0..62 {
                let item_idx = ch[cn].depot[n];
                if item_idx != 0 {
                    total += self.do_depot_cost(item_idx as usize);
                }
            }
            total
        })
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
        let (char_x, char_y, is_god) = Repository::with_characters(|ch| {
            (
                ch[cn].x,
                ch[cn].y,
                ch[cn].flags & CharacterFlags::CF_GOD.bits() != 0,
            )
        });

        if !is_god {
            let map_idx = char_x as usize + char_y as usize * core::constants::SERVER_MAPX as usize;
            let in_bank = Repository::with_map(|map| {
                map[map_idx].flags & core::constants::MF_BANK as u64 != 0
            });

            if !in_bank {
                self.do_character_log(
                    cn,
                    FontColor::Green,
                    "You cannot access your depot outside a bank.\n",
                );
                return;
            }
        }

        let citem = Repository::with_characters(|ch| ch[cn].citem);

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

            let has_nodepot = Repository::with_items(|items| {
                items[item_idx].flags & ItemFlags::IF_NODEPOT.bits() != 0
            });

            if has_nodepot {
                self.do_character_log(cn, FontColor::Green, "You are not allowed to do that!\n");
                return;
            }

            // Calculate storage cost
            let storage_cost = self.do_depot_cost(item_idx);

            // Try to add to depot
            if self.do_add_depot(co, item_idx) {
                Repository::with_characters_mut(|ch| {
                    ch[cn].citem = 0;
                });

                let item_ref = Repository::with_items(|items| {
                    c_string_to_str(&items[item_idx].reference).to_string()
                });

                let item_name =
                    Repository::with_items(|items| items[item_idx].get_name().to_string());

                // Calculate costs per day (Astonian and Earth)
                let astonian_cost = storage_cost;
                let earth_cost = storage_cost * 18; // 18 Astonian days per Earth day

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
                let item_idx = Repository::with_characters(|ch| ch[co].depot[nr as usize]);

                if item_idx != 0 {
                    let gave_success = God::give_character_item(cn, item_idx as usize);

                    if gave_success {
                        Repository::with_characters_mut(|ch| {
                            ch[co].depot[nr as usize] = 0;
                        });

                        let item_ref = Repository::with_items(|items| {
                            c_string_to_str(&items[item_idx as usize].reference).to_string()
                        });

                        let item_name = Repository::with_items(|items| {
                            items[item_idx as usize].get_name().to_string()
                        });

                        self.do_character_log(
                            cn,
                            FontColor::Yellow,
                            &format!("You took the {} from your depot.\n", item_ref),
                        );

                        chlog!(cn, "Took {} from depot", item_name);
                    } else {
                        let item_ref = Repository::with_items(|items| {
                            c_string_to_str(&items[item_idx as usize].reference).to_string()
                        });

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
                let item_idx = Repository::with_characters(|ch| ch[co].depot[exam_slot]);

                if item_idx != 0 {
                    let (item_name, item_desc) = Repository::with_items(|items| {
                        (
                            items[item_idx as usize].get_name().to_string(),
                            c_string_to_str(&items[item_idx as usize].description).to_string(),
                        )
                    });

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
    pub(crate) fn do_depot(&self, cn: usize) {
        self.do_character_log(cn, core::types::FontColor::Yellow, "This is your bank depot. You can store up to 62 items here. But you have to pay a rent for each item.\n");
        // delegate to look depot helper if present
        if let Some(_) = None::<()> {
            // placeholder
        }
    }
}
