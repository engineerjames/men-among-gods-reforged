use crate::driver;
use crate::god::God;

use crate::repository::Repository;
use crate::state::State;
use core::constants::{CharacterFlags, ItemFlags};
use core::types::FontColor;
use std::cmp::Ordering;

impl State {
    /// Port of `do_store_item(int cn)` from `svr_do.cpp`.
    ///
    /// Attempts to store the item currently on the character's cursor (`citem`)
    /// into the first free inventory slot. Performs basic validation (invalid
    /// cursor-encoded values are rejected) and marks the character for update
    /// when the operation succeeds.
    ///
    /// # Returns
    /// * `Ok(slot)` - Inventory slot index (0..39) where the item was placed
    /// * `Err(-1)` - Failure (invalid `citem` or no free slot)
    ///
    /// # Arguments
    /// * `cn` - Character id performing the store operation
    pub(crate) fn do_store_item(&self, cn: usize) -> i32 {
        Repository::with_characters_mut(|characters| {
            let ch = &mut characters[cn];

            // Check if citem has the high bit set (0x80000000), which indicates it's invalid
            if (ch.citem & 0x80000000) != 0 {
                return -1;
            }

            // Find first empty inventory slot
            let mut slot = -1;
            for n in 0..40 {
                if ch.item[n] == 0 {
                    slot = n as i32;
                    break;
                }
            }

            // If no empty slot found, return failure
            if slot == -1 {
                return -1;
            }

            // Store the carried item in the empty slot
            ch.item[slot as usize] = ch.citem;
            ch.citem = 0;

            // Update character to sync with client
            ch.set_do_update_flags();

            slot
        })
    }

    /// Port of `do_sort(cn, order)` from `svr_do.cpp`.
    ///
    /// Sorts a character's inventory according to a custom order string. The
    /// order string contains single-letter sort keys (for example 'w' for
    /// weapons, 'a' for armor, 'v' for value, etc.) which are applied in
    /// sequence. Empty inventory slots are moved to the end.
    ///
    /// # Arguments
    /// * `cn` - Character id whose inventory will be sorted
    /// * `order` - Sort order string composed of single-character keys
    pub(crate) fn do_sort(&self, cn: usize, order: &str) {
        // Check if character is in building mode
        let is_building = Repository::with_characters(|characters| characters[cn].is_building());

        if is_building {
            State::with(|state| {
                state.do_character_log(
                    cn,
                    FontColor::Red,
                    "You cannot sort your inventory while in build mode.\n",
                )
            });
            return;
        }

        // Get a copy of the items array to sort
        let mut items = Repository::with_characters(|characters| characters[cn].item);

        // Sort using custom comparison function based on order string
        items.sort_by(|&a, &b| self.qsort_compare(a as usize, b as usize, order));

        // Write sorted items back
        Repository::with_characters_mut(|characters| {
            characters[cn].item = items;
        });

        // Update character to send changes to client
        State::with(|state| state.do_update_char(cn));
    }

    /// Port of the `qsort_proc(in1, in2, order)` comparator from `svr_do.cpp`.
    ///
    /// Comparison routine used by `do_sort` to order two item indices based on
    /// the provided `order` string. Handles empty slots, type-based checks
    /// (weapon/armor/consumable), and numeric comparisons (hp/end/mana/value),
    /// with a stable fallback on `temp` to preserve determinism.
    ///
    /// # Arguments
    /// * `in1` - Item index for the first item (0 means empty slot)
    /// * `in2` - Item index for the second item (0 means empty slot)
    /// * `order` - Sort order string influencing comparison
    ///
    /// # Returns
    /// * `Ordering` indicating the sort relation of `in1` and `in2`.
    pub(crate) fn qsort_compare(&self, in1: usize, in2: usize, order: &str) -> std::cmp::Ordering {
        // Handle empty slots - they go to the end
        if in1 == 0 && in2 == 0 {
            return Ordering::Equal;
        }
        if in1 != 0 && in2 == 0 {
            return Ordering::Less;
        }
        if in1 == 0 && in2 != 0 {
            return Ordering::Greater;
        }

        // Compare based on order string criteria
        Repository::with_items(|items| {
            let item1 = &items[in1];
            let item2 = &items[in2];

            for ch in order.chars() {
                match ch {
                    'w' => {
                        // Sort by weapon
                        let is_weapon1 =
                            item1.flags & core::constants::ItemFlags::IF_WEAPON.bits() != 0;
                        let is_weapon2 =
                            item2.flags & core::constants::ItemFlags::IF_WEAPON.bits() != 0;
                        if is_weapon1 && !is_weapon2 {
                            return Ordering::Less;
                        }
                        if !is_weapon1 && is_weapon2 {
                            return Ordering::Greater;
                        }
                    }
                    'a' => {
                        // Sort by armor
                        let is_armor1 =
                            item1.flags & core::constants::ItemFlags::IF_ARMOR.bits() != 0;
                        let is_armor2 =
                            item2.flags & core::constants::ItemFlags::IF_ARMOR.bits() != 0;
                        if is_armor1 && !is_armor2 {
                            return Ordering::Less;
                        }
                        if !is_armor1 && is_armor2 {
                            return Ordering::Greater;
                        }
                    }
                    'p' => {
                        // Sort by usable/consumable (use-destroy)
                        let is_usedestroy1 =
                            item1.flags & core::constants::ItemFlags::IF_USEDESTROY.bits() != 0;
                        let is_usedestroy2 =
                            item2.flags & core::constants::ItemFlags::IF_USEDESTROY.bits() != 0;
                        if is_usedestroy1 && !is_usedestroy2 {
                            return Ordering::Less;
                        }
                        if !is_usedestroy1 && is_usedestroy2 {
                            return Ordering::Greater;
                        }
                    }
                    'h' => {
                        // Sort by HP (higher first)
                        if item1.hp[0] > item2.hp[0] {
                            return Ordering::Less;
                        }
                        if item1.hp[0] < item2.hp[0] {
                            return Ordering::Greater;
                        }
                    }
                    'e' => {
                        // Sort by endurance (higher first)
                        if item1.end[0] > item2.end[0] {
                            return Ordering::Less;
                        }
                        if item1.end[0] < item2.end[0] {
                            return Ordering::Greater;
                        }
                    }
                    'm' => {
                        // Sort by mana (higher first)
                        if item1.mana[0] > item2.mana[0] {
                            return Ordering::Less;
                        }
                        if item1.mana[0] < item2.mana[0] {
                            return Ordering::Greater;
                        }
                    }
                    'v' => {
                        // Sort by value (higher first)
                        if item1.value > item2.value {
                            return Ordering::Less;
                        }
                        if item1.value < item2.value {
                            return Ordering::Greater;
                        }
                    }
                    _ => {
                        // Unknown character, skip
                    }
                }
            }

            // Fall back to sort by value
            if item1.value > item2.value {
                return Ordering::Less;
            }
            if item1.value < item2.value {
                return Ordering::Greater;
            }

            // Finally sort by temp (to maintain stability)
            if item1.temp > item2.temp {
                return Ordering::Greater;
            }
            if item1.temp < item2.temp {
                return Ordering::Less;
            }

            Ordering::Equal
        })
    }

    /// Port of `do_maygive(cn, co, in)` from `svr_do.cpp`.
    ///
    /// Determines whether an item may be given or dropped from one character
    /// to another. Currently implements a small set of protections (for
    /// example preventing giving lag-scroll items). The function returns
    /// `true` for items that are allowed to be transferred.
    ///
    /// # Arguments
    /// * `_cn` - Giver character id (retained for API compatibility)
    /// * `_co` - Receiver character id (retained for API compatibility)
    /// * `item_idx` - Item index to check
    ///
    /// # Returns
    /// * `true` if the item may be given/dropped
    /// * `false` if the item is disallowed (e.g., lag scroll)
    pub(crate) fn do_maygive(&self, _cn: usize, _co: usize, item_idx: usize) -> bool {
        // Check if item index is valid
        if !(1..core::constants::MAXITEM).contains(&item_idx) {
            return true; // Invalid items are considered "may give" (will be handled elsewhere)
        }

        // Check if item is a lag scroll - these cannot be given/dropped
        let is_lagscroll = Repository::with_items(|items| {
            if item_idx < items.len() {
                items[item_idx].temp == core::constants::IT_LAGSCROLL as u16
            } else {
                false
            }
        });

        if is_lagscroll {
            return false; // Lag scrolls cannot be given
        }

        true // All other items may be given
    }

    /// Port of `do_give(cn, co)` from `svr_do.cpp`.
    ///
    /// Transfers the item currently on `cn`'s cursor (`citem`) to `co`.
    /// Behavior includes:
    /// - Handling encoded gold values (high-bit set) as currency transfers
    /// - Validating that an item may be given (`do_maygive`)
    /// - Special driver logic (e.g. holy water vs undead)
    /// - Respecting receiver's current cursor state (placing into their
    ///   inventory if needed via `God::give_character_item`)
    /// - Sending notifications and logging actions
    ///
    /// # Arguments
    /// * `cn` - Giver character id
    /// * `co` - Receiver character id
    ///
    /// # Returns
    /// * `true` on success, `false` on failure
    pub(crate) fn do_give(&self, cn: usize, co: usize) -> bool {
        // Check if giver has a carried item
        let citem = Repository::with_characters(|characters| characters[cn].citem);

        if citem == 0 {
            Repository::with_characters_mut(|characters| {
                characters[cn].cerrno = core::constants::ERR_FAILED as u16;
            });
            return false;
        }

        // Set success error code
        Repository::with_characters_mut(|characters| {
            characters[cn].cerrno = core::constants::ERR_SUCCESS as u16;
        });

        // Update both characters
        Repository::with_characters_mut(|characters| {
            characters[cn].set_do_update_flags();
            characters[co].set_do_update_flags();
        });

        // Check if citem is gold (high bit set)
        if (citem & 0x80000000) != 0 {
            let gold_amount = citem & 0x7FFFFFFF;

            // Transfer gold
            Repository::with_characters_mut(|characters| {
                characters[co].gold += gold_amount as i32;
                characters[cn].citem = 0;
            });

            // Log messages
            let (cn_name, co_name, cn_is_player) = Repository::with_characters(|characters| {
                (
                    characters[cn].get_name().to_string(),
                    characters[co].get_name().to_string(),
                    characters[cn].flags & CharacterFlags::CF_PLAYER.bits() != 0,
                )
            });

            self.do_character_log(
                cn,
                FontColor::Green,
                &format!("You give the gold to {}.\n", co_name),
            );
            self.do_character_log(
                co,
                FontColor::Yellow,
                &format!(
                    "You got {}G {}S from {}.\n",
                    gold_amount / 100,
                    gold_amount % 100,
                    cn_name
                ),
            );

            if cn_is_player {
                log::info!(
                    "Character {} gives {} ({}) {}G {}S",
                    cn,
                    co_name,
                    co,
                    gold_amount / 100,
                    gold_amount % 100
                );
            }

            // Notify receiver
            self.do_notify_character(
                co as u32,
                core::constants::NT_GIVE as i32,
                cn as i32,
                0,
                gold_amount as i32,
                0,
            );

            // Update giver
            Repository::with_characters_mut(|characters| {
                characters[cn].set_do_update_flags();
            });

            return true;
        }

        // Handle regular item
        let item_idx = citem as usize;

        // Check if item may be given
        if !self.do_maygive(cn, co, item_idx) {
            self.do_character_log(cn, FontColor::Red, "You're not allowed to do that!\n");
            Repository::with_characters_mut(|characters| {
                characters[cn].misc_action = core::constants::DR_IDLE as u16;
            });
            return false;
        }

        // Log the give action
        let (item_name, co_name) = Repository::with_characters(|characters| {
            Repository::with_items(|items| {
                (
                    items[item_idx].get_name().to_string(),
                    characters[co].get_name().to_string(),
                )
            })
        });

        log::info!(
            "Character {} gives {} ({}) to {} ({})",
            cn,
            item_name,
            item_idx,
            co_name,
            co
        );

        // Special case: driver 31 (holy water) on undead
        let (is_holy_water, co_is_undead, cn_has_nomagic) =
            Repository::with_characters(|characters| {
                Repository::with_items(|items| {
                    (
                        items[item_idx].driver == 31,
                        characters[co].flags & CharacterFlags::CF_UNDEAD.bits() != 0,
                        characters[cn].flags & CharacterFlags::CF_NOMAGIC.bits() != 0,
                    )
                })
            });

        if is_holy_water && co_is_undead {
            if cn_has_nomagic {
                self.do_character_log(
                    cn,
                    FontColor::Red,
                    "It doesn't work! An evil aura is present.\n",
                );
                Repository::with_characters_mut(|characters| {
                    characters[cn].misc_action = core::constants::DR_IDLE as u16;
                });
                return false;
            }

            // Deal damage to undead
            let damage = Repository::with_items(|items| items[item_idx].data[0]);
            State::with_mut(|state| {
                state.do_hurt(cn, co, damage as i32, 2);
            });

            // Destroy the item
            Repository::with_items_mut(|items| {
                items[item_idx].used = core::constants::USE_EMPTY;
            });
            Repository::with_characters_mut(|characters| {
                characters[cn].citem = 0;
            });

            return true;
        }

        // Check for shop destroy flag
        let (co_is_player, has_shop_destroy) = Repository::with_characters(|characters| {
            Repository::with_items(|items| {
                (
                    characters[co].flags & CharacterFlags::CF_PLAYER.bits() != 0,
                    items[item_idx].flags & core::constants::ItemFlags::IF_SHOPDESTROY.bits() != 0,
                )
            })
        });

        if co_is_player && has_shop_destroy {
            self.do_character_log(
                cn,
                FontColor::Red,
                "Beware! The gods see what you're doing.\n",
            );
        }

        // Transfer the item
        let receiver_has_citem =
            Repository::with_characters(|characters| characters[co].citem != 0);

        if receiver_has_citem {
            // Receiver already has a carried item, try to put it in their inventory
            let success = God::give_character_item(co, item_idx);

            if success {
                Repository::with_characters_mut(|characters| {
                    characters[cn].citem = 0;
                });
                self.do_character_log(
                    cn,
                    FontColor::Green,
                    &format!("You give {} to {}.\n", item_name, co_name),
                );
            } else {
                Repository::with_characters_mut(|characters| {
                    characters[cn].misc_action = core::constants::DR_IDLE as u16;
                });
                return false;
            }
        } else {
            // Receiver doesn't have a carried item, give it directly
            Repository::with_characters_mut(|characters| {
                characters[cn].citem = 0;
                characters[co].citem = item_idx as u32;
            });

            Repository::with_items_mut(|items| {
                items[item_idx].carried = co as u16;
            });

            Repository::with_characters_mut(|characters| {
                characters[cn].set_do_update_flags();
            });

            self.do_character_log(
                cn,
                FontColor::Green,
                &format!("You give {} to {}.\n", item_name, co_name),
            );
        }

        // Notify receiver
        self.do_notify_character(
            co as u32,
            core::constants::NT_GIVE as i32,
            cn as i32,
            item_idx as i32,
            0,
            0,
        );

        true
    }

    /// Port of `do_item_value(in)` from `svr_do.cpp`.
    ///
    /// Returns the stored value of an item used in economic calculations.
    /// Performs bounds-checking on the provided index and returns 0 for
    /// invalid indices.
    ///
    /// # Arguments
    /// * `item_idx` - Item index to query
    ///
    /// # Returns
    /// * Item value in copper/lowest unit (u32), or 0 when `item_idx` invalid
    pub(crate) fn do_item_value(&self, item_idx: usize) -> u32 {
        if !(1..core::constants::MAXITEM).contains(&item_idx) {
            return 0;
        }

        Repository::with_items(|items| items[item_idx].value)
    }

    /// Port of `do_look_item(cn, in)` from `svr_do.cpp`.
    ///
    /// Presents detailed information about an item to a character, including:
    /// - Description text and build-mode metadata
    /// - Condition/aging/damage status and colored messaging
    /// - Comparison with the currently carried item (if any)
    /// - Special-case driver behavior (tombstone ransack, career pole checks)
    ///
    /// Access checks are performed to ensure the viewer either owns or can see
    /// the item. Additional god/build-mode information is printed when
    /// applicable.
    ///
    /// # Arguments
    /// * `cn` - Viewer character id
    /// * `item_idx` - Item index to inspect
    pub(crate) fn do_look_item(&mut self, cn: usize, item_idx: usize) {
        // Determine if item is active
        let act = Repository::with_items(|items| if items[item_idx].active != 0 { 1 } else { 0 });

        // Check if character has the item in inventory or worn
        let mut has_item = false;

        Repository::with_characters(|ch| {
            // Check inventory
            for n in 0..40 {
                if ch[cn].item[n] == item_idx as u32 {
                    has_item = true;
                    break;
                }
            }

            // Check worn items if not found in inventory
            if !has_item {
                for n in 0..20 {
                    if ch[cn].worn[n] == item_idx as u32 {
                        has_item = true;
                        break;
                    }
                }
            }
        });

        // If character doesn't have item, check if they can see it
        if !has_item && self.do_char_can_see_item(cn, item_idx) == 0 {
            return;
        }

        // Check if item has special look driver
        let has_lookspecial = Repository::with_items(|items| {
            items[item_idx].flags & ItemFlags::IF_LOOKSPECIAL.bits() != 0
        });

        if has_lookspecial {
            crate::driver::look_driver(cn, item_idx);
        } else {
            // Show item description
            let description = Repository::with_items(|items| items[item_idx].description);
            self.do_character_log(
                cn,
                FontColor::Green,
                &format!("{}\n", String::from_utf8_lossy(&description)),
            );

            // Show condition if item has aging or damage
            let (max_age_0, max_age_1, max_damage, damage_state) =
                Repository::with_items(|items| {
                    (
                        items[item_idx].max_age[act],
                        items[item_idx].max_age[if act == 0 { 1 } else { 0 }],
                        items[item_idx].max_damage,
                        items[item_idx].damage_state,
                    )
                });

            if max_age_0 != 0 || max_age_1 != 0 || max_damage != 0 {
                let condition_msg = match damage_state {
                    0 => "It's in perfect condition.\n",
                    1 => "It's showing signs of age.\n",
                    2 => "It's fairly old.\n",
                    3 => "It is old.\n",
                    4 => "It is very old and battered.\n",
                    _ => "",
                };

                if !condition_msg.is_empty() {
                    let color = if damage_state >= 4 {
                        FontColor::Yellow
                    } else {
                        FontColor::Green
                    };
                    self.do_character_log(cn, color, condition_msg);
                }
            }

            // Show detailed info for build mode
            let is_building = Repository::with_characters(|ch| {
                ch[cn].flags & CharacterFlags::CF_BUILDMODE.bits() != 0
            });

            if is_building {
                let (
                    temp,
                    sprite_0,
                    sprite_1,
                    curr_age_0,
                    max_age_0,
                    curr_age_1,
                    max_age_1,
                    curr_damage,
                    max_damage,
                    active,
                    duration,
                    driver,
                    data,
                ) = Repository::with_items(|items| {
                    (
                        items[item_idx].temp,
                        items[item_idx].sprite[0],
                        items[item_idx].sprite[1],
                        items[item_idx].current_age[0],
                        items[item_idx].max_age[0],
                        items[item_idx].current_age[1],
                        items[item_idx].max_age[1],
                        items[item_idx].current_damage,
                        items[item_idx].max_damage,
                        items[item_idx].active,
                        items[item_idx].duration,
                        items[item_idx].driver,
                        items[item_idx].data,
                    )
                });

                self.do_character_log(
                    cn,
                    FontColor::Green,
                    &format!("Temp: {}, Sprite: {},{}.\n", temp, sprite_0, sprite_1),
                );
                self.do_character_log(
                    cn,
                    FontColor::Green,
                    &format!("In-Active Age {} of {}.\n", curr_age_0, max_age_0),
                );
                self.do_character_log(
                    cn,
                    FontColor::Green,
                    &format!("Active Age {} of {}.\n", curr_age_1, max_age_1),
                );
                self.do_character_log(
                    cn,
                    FontColor::Green,
                    &format!("Damage {} of {}.\n", curr_damage, max_damage),
                );
                self.do_character_log(
                    cn,
                    FontColor::Green,
                    &format!("Active {} of {}.\n", active, duration),
                );
                self.do_character_log(
                    cn,
                    FontColor::Green,
                    &format!(
                        "Driver={} [{},{},{},{},{},{},{},{},{},{}].\n",
                        driver,
                        data[0],
                        data[1],
                        data[2],
                        data[3],
                        data[4],
                        data[5],
                        data[6],
                        data[7],
                        data[8],
                        data[9]
                    ),
                );
            }

            // Show god-mode info
            let is_god =
                Repository::with_characters(|ch| ch[cn].flags & CharacterFlags::CF_GOD.bits() != 0);

            if is_god {
                let (
                    temp,
                    value,
                    active,
                    sprite_0,
                    sprite_1,
                    max_age_0,
                    max_age_1,
                    curr_age_0,
                    curr_age_1,
                    max_damage,
                    curr_damage,
                ) = Repository::with_items(|items| {
                    (
                        items[item_idx].temp,
                        items[item_idx].value,
                        items[item_idx].active,
                        items[item_idx].sprite[0],
                        items[item_idx].sprite[1],
                        items[item_idx].max_age[0],
                        items[item_idx].max_age[1],
                        items[item_idx].current_age[0],
                        items[item_idx].current_age[1],
                        items[item_idx].max_damage,
                        items[item_idx].current_damage,
                    )
                });

                self.do_character_log(
                    cn,
                    FontColor::Green,
                    &format!(
                        "ID={}, Temp={}, Value: {}G {}S.\n",
                        item_idx,
                        temp,
                        value / 100,
                        value % 100
                    ),
                );
                self.do_character_log(
                    cn,
                    FontColor::Green,
                    &format!("active={}, sprite={}/{}\n", active, sprite_0, sprite_1),
                );
                self.do_character_log(
                    cn,
                    FontColor::Green,
                    &format!(
                        "max_age={}/{}, current_age={}/{}\n",
                        max_age_0, max_age_1, curr_age_0, curr_age_1
                    ),
                );
                self.do_character_log(
                    cn,
                    FontColor::Green,
                    &format!(
                        "max_damage={}, current_damage={}\n",
                        max_damage, curr_damage
                    ),
                );
            }

            // Compare with carried item if present
            let citem = Repository::with_characters(|ch| ch[cn].citem);

            if citem != 0 && (citem & 0x80000000) == 0 {
                let citem_idx = citem as usize;

                // Validate carried item
                if citem_idx > 0 && citem_idx < core::constants::MAXITEM {
                    self.do_character_log(cn, FontColor::Green, " \n");

                    let citem_name = Repository::with_items(|items| items[citem_idx].name);
                    self.do_character_log(
                        cn,
                        FontColor::Green,
                        &format!(
                            "You compare it with a {}:\n",
                            String::from_utf8_lossy(&citem_name)
                        ),
                    );

                    // Compare weapon stats
                    let (weapon_this, weapon_carried, name_this) =
                        Repository::with_items(|items| {
                            (
                                items[item_idx].weapon[0],
                                items[citem_idx].weapon[0],
                                items[item_idx].name,
                            )
                        });

                    if weapon_this > weapon_carried {
                        self.do_character_log(
                            cn,
                            FontColor::Green,
                            &format!(
                                "A {} is the better weapon.\n",
                                String::from_utf8_lossy(&name_this)
                            ),
                        );
                    } else if weapon_this < weapon_carried {
                        self.do_character_log(
                            cn,
                            FontColor::Green,
                            &format!(
                                "A {} is the better weapon.\n",
                                String::from_utf8_lossy(&citem_name)
                            ),
                        );
                    } else {
                        self.do_character_log(cn, FontColor::Green, "No difference as a weapon.\n");
                    }

                    // Compare armor stats
                    let (armor_this, armor_carried) = Repository::with_items(|items| {
                        (items[item_idx].armor[0], items[citem_idx].armor[0])
                    });

                    if armor_this > armor_carried {
                        self.do_character_log(
                            cn,
                            FontColor::Green,
                            &format!(
                                "A {} is the better armor.\n",
                                String::from_utf8_lossy(&name_this)
                            ),
                        );
                    } else if armor_this < armor_carried {
                        self.do_character_log(
                            cn,
                            FontColor::Green,
                            &format!(
                                "A {} is the better armor.\n",
                                String::from_utf8_lossy(&citem_name)
                            ),
                        );
                    } else {
                        self.do_character_log(cn, FontColor::Green, "No difference as armor.\n");
                    }
                }
            } else {
                // No carried item - show item_info if identified
                let is_identified = Repository::with_items(|items| {
                    items[item_idx].flags & ItemFlags::IF_IDENTIFIED.bits() != 0
                });

                if is_identified {
                    driver::item_info(cn, item_idx, 1);
                }
            }

            // Special case: tombstone remote scan
            let (item_temp, item_data_0) =
                Repository::with_items(|items| (items[item_idx].temp, items[item_idx].data[0]));

            if item_temp == core::constants::IT_TOMBSTONE as u16 && item_data_0 != 0 {
                State::with(|state| {
                    state.do_ransack_corpse(
                        item_data_0 as usize,
                        cn,
                        "In the tombstone you notice %s!\n",
                    );
                });
            }

            // Special case: driver 57 (career pole check)
            let item_driver = Repository::with_items(|items| items[item_idx].driver);
            if item_driver == 57 {
                let (points_tot, data_4) = Repository::with_characters(|ch| {
                    let item_data = Repository::with_items(|items| items[item_idx].data[4]);
                    (ch[cn].points_tot, item_data)
                });

                let percent = std::cmp::min(100, (100 * (points_tot / 10)) / (data_4 as i32 + 1));

                if percent < 50 {
                    self.do_character_log(
                        cn,
                        FontColor::Yellow,
                        "You sense that it's too early in your career to touch this pole.\n",
                    );
                } else if percent < 70 {
                    self.do_character_log(
                        cn,
                        FontColor::Yellow,
                        "You sense that it's a bit early in your career to touch this pole.\n",
                    );
                }
            }
        }
    }
}
