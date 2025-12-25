use core::types::{Character, Map};

use crate::{enums::CharacterFlags, repository::Repository, state::State};
use rand::Rng;

pub struct God {}
impl God {
    pub fn create_item(template_id: usize) -> Option<usize> {
        if !core::types::Item::is_sane_item_template(template_id) {
            return None;
        }

        Repository::with_item_templates(|item_templates| {
            if item_templates[template_id].used == core::constants::USE_EMPTY {
                log::error!(
                    "Attempted to create item with an unused template ID: {}",
                    template_id
                );
                return None;
            }

            if item_templates[template_id].is_unique() {
                // Check if the unique item already exists
                Repository::with_items(|items| {
                    for item in items.iter() {
                        if item.used != core::constants::USE_EMPTY
                            && item.temp as usize == template_id
                        {
                            log::error!(
                                "Attempted to create unique item with template ID {} but it already exists.",
                                template_id
                            );
                            return Some(None);
                        }
                    }
                    None
                }).unwrap_or(None)?;
            }

            Repository::with_items_mut(|items| {
                let free_item_id = Self::get_free_item(items).unwrap_or_else(|| {
                    log::error!("No free item slots available to create new item.");
                    0
                });

                items[free_item_id] = item_templates[template_id].clone();
                items[free_item_id].temp = template_id as u16;

                Some(free_item_id)
            })
        })
    }

    // TODO: Optimize this later
    fn get_free_item(items: &[core::types::Item]) -> Option<usize> {
        for i in 1..core::constants::MAXITEM as usize {
            if items[i].used == core::constants::USE_EMPTY {
                return Some(i);
            }
        }
        None
    }

    // Implementation of god_give_char from svr_god.cpp
    pub fn give_character_item(character_id: usize, item_id: usize) -> bool {
        if !core::types::Item::is_sane_item(item_id) {
            log::error!("Invalid item ID {} when giving item.", item_id);
            return false;
        }

        Repository::with_characters_mut(|characters| {
            if !characters[character_id].is_living_character(character_id) {
                log::error!("Invalid character ID {} when giving item.", character_id);
                return false;
            }

            Repository::with_items_mut(|items| {
                let character = &mut characters[character_id];
                let item = &mut items[item_id];

                log::debug!(
                    "Attempting to give item '{}' to character '{}'",
                    item.get_name(),
                    character.get_name(),
                );

                if let Some(slot) = character.get_next_inventory_slot() {
                    character.item[slot] = item_id as u32;
                    item.x = 0;
                    item.y = 0;
                    item.carried = character_id as u16;

                    character.set_do_update_flags();

                    true
                } else {
                    log::error!(
                        "No free inventory slots available for character '{}' (ID {}).",
                        character.get_name(),
                        character_id
                    );

                    false
                }
            })
        })
    }

    pub fn build(character: &mut core::types::Character, character_id: usize, build_type: u32) {
        if !character.is_building() {
            if Self::build_start(character_id) {
                Self::build_equip(character_id, build_type);
            } else {
                log::error!(
                    "Failed to start build mode for character {}",
                    character.get_name()
                );
            }
        } else if build_type != 0 {
            Self::build_stop(character_id);
        } else {
            Self::build_equip(character_id, build_type);
        }
    }

    pub fn build_equip(character_id: usize, build_type: u32) {
        Repository::with_characters_mut(|characters| {
            let character = &mut characters[character_id];

            Repository::with_item_templates(|item_templates| {
                let mut m = 0;

                match build_type {
                    0 => {
                        // Map flags
                        character.item[m] = 0x40000000 | core::constants::MF_MOVEBLOCK;
                        m += 1;
                        character.item[m] = 0x40000000 | core::constants::MF_SIGHTBLOCK;
                        m += 1;
                        character.item[m] = 0x40000000 | core::constants::MF_INDOORS;
                        m += 1;
                        character.item[m] = 0x40000000 | core::constants::MF_ARENA;
                        m += 1;
                        character.item[m] = 0x40000000 | core::constants::MF_NOMONST;
                        m += 1;
                        character.item[m] = 0x40000000 | core::constants::MF_BANK;
                        m += 1;
                        character.item[m] = 0x40000000 | core::constants::MF_TAVERN;
                        m += 1;
                        character.item[m] = 0x40000000 | core::constants::MF_NOMAGIC;
                        m += 1;
                        character.item[m] = 0x40000000 | core::constants::MF_DEATHTRAP;
                        m += 1;
                        character.item[m] = 0x40000000 | core::constants::MF_UWATER;
                        m += 1;
                        character.item[m] = 0x40000000 | core::constants::MF_NOLAG;
                        m += 1;
                        character.item[m] = 0x40000000 | (core::constants::MF_NOFIGHT as u32);
                        m += 1;
                        character.item[m] = 0x40000000 | core::constants::MF_NOEXPIRE;
                        m += 1;

                        // Ground sprites
                        character.item[m] = 0x20000000 | core::constants::SPR_TUNDRA_GROUND as u32;
                        m += 1;
                        character.item[m] = 0x20000000 | core::constants::SPR_DESERT_GROUND as u32;
                        m += 1;
                        character.item[m] = 0x20000000 | core::constants::SPR_GROUND1 as u32;
                        m += 1;
                        character.item[m] = 0x20000000 | core::constants::SPR_WOOD_GROUND as u32;
                        m += 1;
                        character.item[m] = 0x20000000 | core::constants::SPR_TAVERN_GROUND as u32;
                        m += 1;
                        character.item[m] = 0x20000000 | core::constants::SPR_STONE_GROUND1 as u32;
                        m += 1;
                        character.item[m] = 0x20000000 | core::constants::SPR_STONE_GROUND2 as u32;
                        m += 1;

                        // Additional sprite IDs
                        character.item[m] = 0x20000000 | 1100;
                        m += 1;
                        character.item[m] = 0x20000000 | 1099;
                        m += 1;
                        character.item[m] = 0x20000000 | 1109;
                        m += 1;
                        character.item[m] = 0x20000000 | 1118;
                        m += 1;
                        character.item[m] = 0x20000000 | 1141;
                        m += 1;
                        character.item[m] = 0x20000000 | 1158;
                        m += 1;
                        character.item[m] = 0x20000000 | 1145;
                        m += 1;
                        character.item[m] = 0x20000000 | 1014;
                        m += 1;
                        character.item[m] = 0x20000000 | 1003;
                        m += 1;
                        character.item[m] = 0x20000000 | 1005;
                        m += 1;
                        character.item[m] = 0x20000000 | 1006;
                        m += 1;
                        character.item[m] = 0x20000000 | 1007;
                        m += 1;
                        character.item[m] = 0x20000000 | 402;
                        m += 1;
                        character.item[m] = 0x20000000 | 500;
                        m += 1;
                        character.item[m] = 0x20000000 | 558;
                        m += 1;
                        character.item[m] = 0x20000000 | 596;
                        m += 1;
                    }
                    1 => {
                        for n in 520..=541 {
                            character.item[m] = 0x20000000 | n;
                            m += 1;
                        }
                    }
                    2 => {
                        for n in 542..=554 {
                            character.item[m] = 0x20000000 | n;
                            m += 1;
                        }
                    }
                    3 => {
                        for n in 130..=145 {
                            character.item[m] = 0x20000000 | n;
                            m += 1;
                        }
                    }
                    4 => {
                        for n in 170..=175 {
                            character.item[m] = 0x20000000 | n;
                            m += 1;
                        }
                    }
                    331 => {
                        character.item[m] = 0x40000000 | core::constants::MF_INDOORS;
                        m += 1;
                        character.item[m] = 0x20000000 | 116;
                        m += 1;
                        character.item[m] = 0x20000000 | 117;
                        m += 1;
                        character.item[m] = 0x20000000 | 118;
                        m += 1;
                        character.item[m] = 0x20000000 | 704;
                        m += 1;
                    }
                    700 => {
                        // Black stronghold
                        character.item[m] = 0x40000000 | core::constants::MF_INDOORS;
                        m += 1;
                        character.item[m] = 0x20000000 | 950;
                        m += 1;
                        character.item[m] = 0x20000000 | 959;
                        m += 1;
                        character.item[m] = 0x20000000 | 16652;
                        m += 1;
                        character.item[m] = 0x20000000 | 16653;
                        m += 1;
                        character.item[m] = 0x20000000 | 16654;
                        m += 1;
                        character.item[m] = 0x20000000 | 16655;
                        m += 1;
                    }
                    701 => {
                        for n in 0..40 {
                            character.item[m] = 0x20000000 | (n + 16430);
                            m += 1;
                        }
                    }
                    702 => {
                        for n in 40..78 {
                            character.item[m] = 0x20000000 | (n + 16430);
                            m += 1;
                        }
                    }
                    703 => {
                        for n in 16584..16599 {
                            character.item[m] = 0x20000000 | n;
                            m += 1;
                        }
                    }
                    704 => {
                        for n in 985..989 {
                            character.item[m] = 0x20000000 | n;
                            m += 1;
                        }
                    }
                    705 => {
                        character.item[m] = 0x20000000 | 1118;
                        m += 1;
                        character.item[m] = 0x20000000 | 989;
                        m += 1;
                        for n in 16634..16642 {
                            character.item[m] = 0x20000000 | n;
                            m += 1;
                        }
                    }
                    819 => {
                        character.item[m] = 0x40000000 | core::constants::MF_INDOORS;
                        m += 1;
                        character.item[m] = 0x20000000 | 16728;
                        m += 1;
                    }
                    900 => {
                        // Graveyard quest
                        character.item[m] = 0x20000000 | 16933; // lost souls tile
                        m += 1;
                        character.item[m] = 0x20000000 | 16934; // grave
                        m += 1;
                        character.item[m] = 0x20000000 | 16937; // grave, other dir
                        m += 1;
                    }
                    1000 => {
                        character.item[m] = 0x40000000 | core::constants::MF_INDOORS;
                        m += 1;
                        character.item[m] = 0x20000000 | 1014;
                        m += 1;
                        character.item[m] = 0x20000000 | 704;
                        m += 1;

                        for n in 508..=519 {
                            character.item[m] = n;
                            m += 1;
                        }
                        character.item[m] = 522;
                        m += 1;
                    }
                    1001 => {
                        character.item[m] = 0x40000000 | core::constants::MF_INDOORS;
                        m += 1;
                        character.item[m] = 0x20000000 | 1118;
                        m += 1;
                        character.item[m] = 16;
                        m += 1;
                        character.item[m] = 17;
                        m += 1;
                        character.item[m] = 45;
                        m += 1;
                        character.item[m] = 47;
                        m += 1;
                        character.item[m] = 19;
                        m += 1;
                        character.item[m] = 20;
                        m += 1;
                        character.item[m] = 48;
                        m += 1;
                        character.item[m] = 49;
                        m += 1;
                        character.item[m] = 606;
                        m += 1;
                        character.item[m] = 607;
                        m += 1;
                        character.item[m] = 608;
                        m += 1;
                        character.item[m] = 609;
                        m += 1;
                        character.item[m] = 611;
                        m += 1;
                    }
                    1002 => {
                        // Ice penta
                        character.item[m] = 0x40000000 | core::constants::MF_INDOORS;
                        m += 1;
                        character.item[m] = 0x20000000 | 16670;
                        m += 1;

                        for n in 800..=812 {
                            character.item[m] = n;
                            m += 1;
                        }
                    }
                    1003 => {
                        character.item[m] = 0x20000000 | 16980;
                        m += 1;
                    }
                    1140 => {
                        character.item[m] = 0x20000000 | 17064;
                        m += 1;
                        character.item[m] = 0x20000000 | 17065;
                        m += 1;
                        character.item[m] = 0x20000000 | 17066;
                        m += 1;
                        character.item[m] = 0x20000000 | 17067;
                        m += 1;
                    }
                    _ => {}
                }

                // Fill inventory with other stuff upward from last item
                for n in build_type as usize..core::constants::MAXTITEM as usize {
                    if m >= 40 {
                        break;
                    }

                    if item_templates[n].used == core::constants::USE_EMPTY {
                        continue;
                    }

                    if item_templates[n].flags & core::constants::ItemFlags::IF_TAKE.bits() != 0 {
                        continue;
                    }

                    if item_templates[n].driver == 25 && item_templates[n].data[3] == 0 {
                        continue;
                    }

                    if item_templates[n].driver == 22 {
                        continue;
                    }

                    character.item[m] = n as u32;
                    m += 1;
                }

                log::info!(
                    "Build mode {} set for character {}",
                    build_type,
                    character.get_name()
                );

                State::with(|state| {
                    state.do_character_log(
                        character_id,
                        core::types::FontColor::Blue,
                        "You are now in build mode. To exit, use the build command again.\n",
                    );
                })
            });
        })
    }

    pub fn build_start(character_id: usize) -> bool {
        let companion = Repository::with_characters(|characters| {
            let character = &characters[character_id];
            if character.data[core::constants::CHD_COMPANION] != 0 {
                Some(character.data[core::constants::CHD_COMPANION] as usize)
            } else {
                None
            }
        });

        if let Some(companion_id) = companion {
            let companion_name = Repository::with_characters(|characters| {
                characters[companion_id].get_name().to_string()
            });
            State::with(|state| {
                state.do_character_log(
                    character_id,
                    core::types::FontColor::Red,
                    &format!("Get rid of your companion '{}' first.\n", companion_name),
                );
            });

            return false;
        }

        let character_id_to_hold_inventory = Self::create_char(1, false);

        if character_id_to_hold_inventory.is_none() {
            State::with(|state| {
                state.do_character_log(
                    character_id,
                    core::types::FontColor::Red,
                    "Failed to create temporary character to hold your items for build mode.\n",
                );
            });
            log::error!(
                "Failed to create temporary character to hold items for build mode for character ID {}",
                character_id
            );
            return false;
        }

        Repository::with_characters_mut(|characters| {
            // Transfer inventory
            for i in 0..40 {
                let item_id = characters[character_id].item[i] as usize;
                if item_id != 0 {
                    characters[character_id].item[i] = 0;
                    characters[character_id_to_hold_inventory.unwrap() as usize].item[i] =
                        item_id as u32;

                    Repository::with_items_mut(|items| {
                        items[item_id].carried = character_id_to_hold_inventory.unwrap() as u16;
                    });
                }
            }

            characters[character_id_to_hold_inventory.unwrap() as usize].citem =
                characters[character_id].citem;
            characters[character_id].citem = 0;

            // TODO: This function looks very ugly... refactor later
            characters[character_id_to_hold_inventory.unwrap() as usize].name =
                format!("{}'s holder", characters[character_id].get_name())
                    .bytes()
                    .take(40)
                    .collect::<Vec<u8>>()
                    .try_into()
                    .unwrap_or([0; 40]);

            Self::drop_char(character_id_to_hold_inventory.unwrap() as usize, 10, 10);

            characters[character_id].flags |= CharacterFlags::BuildMode.bits();
            characters[character_id].set_do_update_flags();
        });
        return true;
    }

    pub fn build_stop(character_id: usize) {
        if !core::types::Character::is_sane_character(character_id) {
            log::error!("Invalid character ID {} in build_stop", character_id);
            return;
        }

        // Empty builder's inventory
        Repository::with_characters_mut(|characters| {
            let character = &mut characters[character_id];

            for n in 0..40 {
                character.item[n] = 0;
            }
            character.citem = 0;

            // Reset build mode
            character.flags &= !core::constants::CharacterFlags::CF_BUILDMODE.bits();
            character.misc_action = 0; // DR_IDLE

            State::with(|state| {
                state.do_character_log(
                    character_id,
                    core::types::FontColor::Blue,
                    "You are now out of build mode.\n",
                );
            });

            log::info!("Character {} now out of build mode", character.get_name());
        });

        // Retrieve inventory from item holder
        let companion_id = Repository::with_characters(|characters| {
            characters[character_id].data[core::constants::CHD_COMPANION] as usize
        });

        if companion_id == 0 {
            log::error!(
                "Could not find item holder for character {} when stopping build mode",
                character_id
            );

            State::with(|state| {
                state.do_character_log(
                    character_id,
                    core::types::FontColor::Red,
                    "Could not find your item holder!\n",
                );
            });
            return;
        }

        Repository::with_characters_mut(|characters| {
            // Collect inventory data from companion first
            let mut items_to_transfer = Vec::new();
            let companion_citem;

            {
                let companion = &mut characters[companion_id];
                for n in 0..40 {
                    items_to_transfer.push((n, companion.item[n]));
                    companion.item[n] = 0;
                }
                companion_citem = companion.citem;
                companion.citem = 0;

                // Destroy item holder (companion)
                // TODO: plr_map_remove(companion_id) when map system is implemented
                companion.used = core::constants::USE_EMPTY;
                characters[character_id].data[core::constants::CHD_COMPANION] = 0;
            }

            // Transfer inventory from companion to builder
            for (n, item_id) in items_to_transfer {
                if item_id != 0 {
                    characters[character_id].item[n] = item_id;

                    // Update item's carrier
                    Repository::with_items_mut(|items| {
                        if core::types::Item::is_sane_item(item_id as usize) {
                            items[item_id as usize].carried = character_id as u16;
                        }
                    });
                }
            }

            // Transfer citem from companion to builder
            characters[character_id].citem = companion_citem;
            if companion_citem != 0 {
                Repository::with_items_mut(|items| {
                    if core::types::Item::is_sane_item(companion_citem as usize) {
                        items[companion_citem as usize].carried = character_id as u16;
                    }
                });
            }

            characters[character_id].set_do_update_flags();
        });
    }

    pub fn transfer_char(character_id: usize, x: usize, y: usize) -> bool {
        if !Character::is_sane_character(character_id) || !Map::is_sane_coordinates(x, y) {
            log::error!(
                "Invalid character ID {} or coordinates ({}, {}) in transfer_char",
                character_id,
                x,
                y
            );
            return false;
        }

        Repository::with_characters_mut(|characters| {
            let character = &mut characters[character_id];
            character.status = 0;
            character.attack_cn = 0;
            character.skill_nr = 0;
            character.goto_x = x as u16;
            character.goto_y = y as u16; // TODO: This was missing before... should this be here?
        });

        // TODO: Call plr_map_remove here when map system is implemented

        let positions_to_try: [(usize, usize); 5] =
            [(x, y), (x + 3, y), (x, y + 3), (x - 3, y), (x, y - 3)];

        for (try_x, try_y) in positions_to_try.iter() {
            if Self::drop_char_fuzzy_large(character_id, *try_x, *try_y, x, y) {
                return true;
            }
        }

        // TODO: Call plr_map_set here when map system is implemented

        return false;
    }

    pub fn drop_char_fuzzy(character_id: usize, x: usize, y: usize) -> bool {
        let positions_to_try: [(usize, usize); 25] = [
            (x + 0, y + 0),
            (x + 1, y + 0),
            (x - 1, y + 0),
            (x + 0, y + 1),
            (x + 0, y - 1),
            (x + 1, y + 1),
            (x + 1, y - 1),
            (x - 1, y + 1),
            (x - 1, y - 1),
            (x + 2, y - 2),
            (x + 2, y - 1),
            (x + 2, y + 0),
            (x + 2, y + 1),
            (x + 2, y + 2),
            (x - 2, y - 2),
            (x - 2, y - 1),
            (x - 2, y + 0),
            (x - 2, y + 1),
            (x - 2, y + 2),
            (x - 1, y + 2),
            (x + 0, y + 2),
            (x + 1, y + 2),
            (x - 1, y - 2),
            (x + 0, y - 2),
            (x + 1, y - 2),
        ];

        for (try_x, try_y) in positions_to_try.iter() {
            let early_return = State::with_mut(|state| {
                if state.can_go(*try_x as i32, *try_y as i32, *try_x as i32, *try_y as i32)
                    && Self::drop_char(character_id, *try_x, *try_y)
                {
                    return true;
                }
                false
            });

            if early_return {
                return true;
            }
        }

        false
    }

    pub fn drop_char_fuzzy_large(
        character_id: usize,
        x: usize,
        y: usize,
        center_x: usize,
        center_y: usize,
    ) -> bool {
        // TODO: Refactor this stupid function later
        let positions_to_try: [(usize, usize); 25] = [
            (x + 0, y + 0),
            (x + 1, y + 0),
            (x - 1, y + 0),
            (x + 0, y + 1),
            (x + 0, y - 1),
            (x + 1, y + 1),
            (x + 1, y - 1),
            (x - 1, y + 1),
            (x - 1, y - 1),
            (x + 2, y - 2),
            (x + 2, y - 1),
            (x + 2, y + 0),
            (x + 2, y + 1),
            (x + 2, y + 2),
            (x - 2, y - 2),
            (x - 2, y - 1),
            (x - 2, y + 0),
            (x - 2, y + 1),
            (x - 2, y + 2),
            (x - 1, y + 2),
            (x + 0, y + 2),
            (x + 1, y + 2),
            (x - 1, y - 2),
            (x + 0, y - 2),
            (x + 1, y - 2),
        ];

        for (try_x, try_y) in positions_to_try.iter() {
            // Also check can_map_go here
            let early_return = State::with_mut(|state| {
                if state.can_go(
                    center_x as i32,
                    center_y as i32,
                    *try_x as i32,
                    *try_y as i32,
                ) && Self::drop_char(character_id, *try_x, *try_y)
                {
                    return true;
                }
                false
            });

            if early_return {
                return true;
            }
        }

        false
    }

    pub fn create_char(template_id: usize, with_items: bool) -> Option<i32> {
        let unused_index = Repository::with_characters(|characters| {
            // TODO: Refactor this into its own function
            for i in 1..core::constants::MAXCHARS {
                if characters[i].used == core::constants::USE_EMPTY {
                    return Some(i);
                }
            }

            None
        });

        let char_index = match unused_index {
            Some(index) => index,
            None => {
                log::error!("No free character slots available to create new character.");
                return None;
            }
        };

        Repository::with_characters_mut(|characters| {
            let character = &mut characters[char_index];

            *character = Repository::with_character_templates(|char_templates| {
                char_templates[template_id].clone()
            });

            character.pass1 = rand::random::<u32>() % 0x3fffffff;
            character.pass2 = rand::random::<u32>() % 0x3fffffff;
            character.temp = template_id as u16;

            loop {
                log::info!("Generating random name for new character...");
                let potential_new_name = Self::randomly_generate_name();

                let name_exists = Repository::with_characters(|characters| {
                    for existing_char in characters.iter() {
                        if existing_char.used != core::constants::USE_EMPTY
                            && existing_char
                                .get_name()
                                .eq_ignore_ascii_case(&potential_new_name)
                        {
                            return true;
                        }
                    }
                    false
                });
                if !name_exists {
                    character.name = potential_new_name
                        .bytes()
                        .take(40)
                        .collect::<Vec<u8>>()
                        .try_into()
                        .unwrap_or([0; 40]);
                    log::info!(
                        "Assigned name '{}' to new character (ID {})",
                        character.get_name(),
                        char_index
                    );
                    break;
                }

                log::info!(
                    "Generated name '{}' already exists. Retrying...",
                    potential_new_name
                );
            }

            character.reference = character.name.clone();
            character.description = character
                .get_default_description()
                .as_bytes()
                .iter()
                .take(200)
                .copied()
                .collect::<Vec<u8>>()
                .try_into()
                .unwrap_or([0; 200]); // TODO: Is this really the right way to do this?

            for i in 0..100 as usize {
                character.data[i] = 0;
            }
            character.attack_cn = 0;
            character.skill_nr = 0;
            character.goto_x = 0;
            character.goto_y = 0; // TODO: This was missing before... should this be here?
            character.use_nr = 0;
            character.misc_action = 0;
            character.stunned = 0;
            character.retry = 0;
            character.dir = core::constants::DX_DOWN;

            let mut flag = 0;
            for i in 0..40 {
                let mut tmp = character.item[i];
                if tmp == 0 {
                    continue;
                }

                if with_items {
                    tmp = Self::create_item(tmp as usize).unwrap_or(0) as u32;
                    if tmp == 0 {
                        log::error!(
                            "Failed to create item from template new character ID {}",
                            char_index
                        );
                        flag = 1;
                    }
                    Repository::with_items_mut(|items| {
                        if tmp != 0 && items[tmp as usize].used != core::constants::USE_EMPTY {
                            items[tmp as usize].carried = char_index as u16;
                        }
                    });
                } else {
                    tmp = 0;
                }

                character.item[i] = tmp;
            }

            for i in 0..20 {
                let mut tmp_worn = character.worn[i];
                if tmp_worn == 0 {
                    continue;
                }

                if with_items {
                    tmp_worn = Self::create_item(tmp_worn as usize).unwrap_or(0) as u32;
                    if tmp_worn == 0 {
                        log::error!(
                            "Failed to create worn item from template for new character ID {}",
                            char_index
                        );
                        flag = 1;
                    }
                    Repository::with_items_mut(|items| {
                        items[tmp_worn as usize].carried = char_index as u16;
                    });
                } else {
                    tmp_worn = 0;
                }

                character.worn[i] = tmp_worn;
            }

            for i in 0..20 {
                if character.spell[i] != 0 {
                    character.spell[i] = 0;
                }
            }

            let mut tmp_citem = character.citem;
            if tmp_citem != 0 {
                if with_items {
                    tmp_citem = Self::create_item(tmp_citem as usize).unwrap_or(0) as u32;
                    if tmp_citem == 0 {
                        log::error!(
                            "Failed to create citem from template for new character ID {}",
                            char_index
                        );
                        flag = 1;
                    }
                    Repository::with_items_mut(|items| {
                        items[tmp_citem as usize].carried = char_index as u16;
                    });
                } else {
                    tmp_citem = 0;
                }

                character.citem = tmp_citem;
            }

            if flag != 0 {
                log::error!(
                    "One or more items failed to be created for new character ID {}",
                    char_index
                );
                Self::destroy_items(char_index);
                character.used = core::constants::USE_EMPTY;
                return None;
            }

            character.a_end = 1000000;
            character.a_hp = 1000000;
            character.a_mana = 1000000;

            character.set_do_update_flags();

            Some(char_index as i32)
        })
    }

    pub fn destroy_items(char_id: usize) {
        if !core::types::Character::is_sane_character(char_id) {
            log::error!("Invalid character ID {} in destroy_items", char_id);
            return;
        }

        Repository::with_characters_mut(|characters| {
            let character = &mut characters[char_id];

            // Destroy all inventory items (40 slots)
            for n in 0..40 {
                let item_id = character.item[n] as usize;
                if item_id != 0 {
                    character.item[n] = 0;
                    if core::types::Item::is_sane_item(item_id) {
                        Repository::with_items_mut(|items| {
                            items[item_id].used = core::constants::USE_EMPTY;
                        });
                    }
                }
            }

            // Destroy all worn items (20 slots)
            for n in 0..20 {
                let worn_id = character.worn[n] as usize;
                if worn_id != 0 {
                    character.worn[n] = 0;
                    if core::types::Item::is_sane_item(worn_id) {
                        Repository::with_items_mut(|items| {
                            items[worn_id].used = core::constants::USE_EMPTY;
                        });
                    }
                }

                let spell_id = character.spell[n] as usize;
                if spell_id != 0 {
                    character.spell[n] = 0;
                    if core::types::Item::is_sane_item(spell_id) {
                        Repository::with_items_mut(|items| {
                            items[spell_id].used = core::constants::USE_EMPTY;
                        });
                    }
                }
            }

            // Destroy carried item (citem)
            let citem_id = character.citem as usize;
            if citem_id != 0 {
                character.citem = 0;
                // TODO: Refactor this check--it is duplicated due to the != 0
                // check above anyway.
                if core::types::Item::is_sane_item(citem_id) {
                    Repository::with_items_mut(|items| {
                        items[citem_id].used = core::constants::USE_EMPTY;
                    });
                }
            }

            // If player, destroy depot/storage items (62 slots)
            if character.is_player() {
                for n in 0..62 {
                    let depot_id = character.depot[n] as usize;
                    if depot_id != 0 {
                        character.depot[n] = 0;
                        if core::types::Item::is_sane_item(depot_id) {
                            Repository::with_items_mut(|items| {
                                items[depot_id].used = core::constants::USE_EMPTY;
                            });
                        }
                    }
                }
            }

            character.set_do_update_flags();
        });
    }

    pub fn randomly_generate_name() -> String {
        let syl1 = [
            "thi", "ar", "an", "un", "iss", "ish", "urs", "ur", "ent", "esh", "ash", "jey", "jay",
            "dur", "lon", "lan", "len", "lun", "so", "lur", "gar", "cry", "au", "dau", "dei",
            "zir", "zil", "sol", "luc", "ni", "bus", "mid", "err", "doo", "do", "al", "ea", "jac",
            "ta", "bi", "vae", "rif", "tol", "nim", "ru", "li", "fro", "sam", "beut", "bil", "ga",
            "nee", "ara", "rho", "dan", "va", "lan", "cec", "cic", "cac", "cuc", "ix", "vea",
            "cya", "hie", "bo", "ni", "do", "sar", "phe", "ho", "cos", "sin", "tan", "mul", "har",
            "gur", "tar", "a", "e", "i", "o", "u", "je", "ho", "if", "jai", "coy", "ya", "pa",
            "pul", "pil", "rez", "rel", "rar", "dom", "rom", "tom", "ar", "ur", "ir", "er", "yr",
            "li", "la", "lu", "lo",
        ];
        let syl2 = [
            "tar", "tur", "kar", "kur", "kan", "tan", "gar", "gur", "run",
        ];
        let syl3 = ["a", "e", "i", "o", "u"];

        let mut rng = rand::thread_rng();
        let mut name = String::new();

        let n = rng.gen_range(0..syl1.len());
        name.push_str(syl1[n]);
        if let Some(first_char) = name.chars().next() {
            name.replace_range(0..1, &first_char.to_uppercase().to_string());
        }

        let n = rng.gen_range(0..syl2.len());
        name.push_str(syl2[n]);

        if rng.gen_bool(0.5) {
            return name;
        }

        let n = rng.gen_range(0..syl3.len());
        name.push_str(syl3[n]);

        name
    }

    pub fn take_from_char(item_id: usize, cn: usize) -> bool {
        true // TODO: Implement this
    }

    pub fn drop_item(item_id: usize, x: usize, y: usize) -> bool {
        if !Map::is_sane_coordinates(x, y) {
            return false;
        }

        let map_index = x + y * core::constants::SERVER_MAPX as usize;

        let can_drop = Repository::with_map(|map| {
            if map[map_index].ch != 0
                || map[map_index].to_ch != 0
                || map[map_index].it != 0
                || (map[map_index].flags
                    & (core::constants::MF_MOVEBLOCK | core::constants::MF_DEATHTRAP) as u64)
                    != 0
                || map[map_index].fsprite != 0
            {
                return false;
            }
            true
        });

        if !can_drop {
            return false;
        }

        Repository::with_map_mut(|map| {
            map[map_index].it = item_id as u32;
        });

        Repository::with_items_mut(|items| {
            items[item_id].x = x as u16;
            items[item_id].y = y as u16;
            items[item_id].carried = 0;

            let light_value = if items[item_id].active != 0 {
                items[item_id].light[1]
            } else {
                items[item_id].light[0]
            };

            if light_value != 0 {
                State::with_mut(|state| {
                    state.do_add_light(x as i32, y as i32, light_value as i32);
                });
            }
        });

        true
    }

    pub fn drop_char(character_id: usize, x: usize, y: usize) -> bool {
        if !Map::is_sane_coordinates(x, y) {
            return false;
        }

        let map_index = x + y * core::constants::SERVER_MAPX as usize;

        let move_is_valid = Repository::with_map(|map_tiles| {
            Repository::with_items(|items| {
                let item_on_tile = map_tiles[map_index].it;
                if map_tiles[map_index].ch != 0
                    || (item_on_tile != 0
                        && items[item_on_tile as usize].flags
                            & core::constants::ItemFlags::IF_MOVEBLOCK.bits()
                            != 0)
                    || map_tiles[map_index].flags & core::constants::MF_MOVEBLOCK as u64 != 0
                    || map_tiles[map_index].flags & core::constants::MF_TAVERN as u64 != 0
                    || map_tiles[map_index].flags & core::constants::MF_DEATHTRAP as u64 != 0
                {
                    return false;
                }

                return true;
            })
        });

        if !move_is_valid {
            return false;
        }

        Repository::with_characters_mut(|characters| {
            characters[character_id].x = x as i16;
            characters[character_id].y = y as i16;
            characters[character_id].tox = x as i16;
            characters[character_id].toy = y as i16;
        });

        // TODO: Call plr_map_set here

        true
    }

    pub fn change_pass(cn: usize, co: usize, pass: &str) -> i32 {}

    pub fn remove_item(cn: usize, item_id: usize) -> i32 {}

    pub fn drop_item_fuzzy(nr: usize, x: usize, y: usize) -> bool {}

    pub fn goto(cn: usize, co: usize, cx: &str, cy: &str) {}

    pub fn info(cn: usize, co: usize) {}
    pub fn iinfo(cn: usize, item_index: usize) {}
    pub fn tinfo(cn: usize, template: usize) {}
    pub fn unique(cn: usize) {}
    pub fn who(cn: usize) {}
    pub fn implist(cn: usize) {}

    pub fn user_who(cn: usize) {}
    pub fn top(cn: usize) {}

    pub fn create(cn: usize, x: i32) {}

    pub fn find_next_char(start_index: usize, spec1: &str, spec2: &str) -> i32 {}

    pub fn invis(looker: usize, target: usize) -> i32 {}

    pub fn summon(cn: usize, spec1: &str, spec2: &str, spec3: &str) {}

    pub fn mirror(cn: usize, spec1: &str, spec2: &str) {}

    pub fn thrall(cn: usize, spec1: &str, spec2: &str) -> i32 {}

    pub fn tavern(cn: usize) {}

    pub fn raise_char(cn: usize, co: usize, value: i32) {}

    pub fn lower_char(cn: usize, co: usize, value: i32) {}

    pub fn gold_char(cn: usize, co: usize, value: i32, silver: &str) {}

    pub fn erase(cn: usize, co: usize, erase_player: i32) {}

    pub fn kick(cn: usize, co: usize) {}

    pub fn skill(cn: usize, co: usize, n: i32, val: i32) {}

    pub fn donate_item(item_id: usize, place: i32) {}

    pub fn set_flag(cn: usize, co: usize, flag: u64) {}

    pub fn set_gflag(cn: usize, flag: i32) {}

    pub fn set_purple(cn: usize, co: usize) {}

    pub fn racechange(co: usize, temp: i32) {}

    pub fn save(cn: usize, co: usize) -> i32 {}

    pub fn mail_pass(cn: usize, co: usize) {}

    pub fn slap(cn: usize, co: usize) {}

    pub fn spritechange(cn: usize, co: usize, sprite: i32) {}

    pub fn luck(cn: usize, co: usize, value: i32) {}

    pub fn reset_description(cn: usize, co: usize) {}

    pub fn set_name(cn: usize, co: usize, name: &str) {}

    pub fn usurp(cn: usize, co: usize) {}

    pub fn exit_usurp(cn: usize) {}

    pub fn grolm(cn: usize) {}

    pub fn grolm_info(cn: usize) {}

    pub fn grolm_start(cn: usize) {}

    pub fn gargoyle(cn: usize) {}

    pub fn minor_racechange(cn: usize, temp: i32) {}

    pub fn force(cn: usize, whom: &str, text: &str) {}

    pub fn enemy(cn: usize, npc: &str, victim: &str) {}

    pub fn is_banned(addr: i32) -> bool {}

    pub fn add_single_ban(cn: usize, co: usize, addr: u32) {}

    pub fn add_ban(cn: usize, co: usize) {}

    pub fn del_ban(cn: usize, nr: usize) {}

    pub fn list_ban(cn: usize) {}

    pub fn shutup(cn: usize, co: usize) {}
}
