use core::types::{Character, Map};

use crate::{
    enums::{CharacterFlags, LogoutReason},
    player,
    repository::Repository,
    state::State,
};
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

    pub fn build(character_id: usize, build_type: u32) {
        let (character_is_building, name) = Repository::with_characters(|characters| {
            let character = &characters[character_id];
            (character.is_building(), character.get_name().to_string())
        });
        if !character_is_building {
            if Self::build_start(character_id) {
                Self::build_equip(character_id, build_type);
            } else {
                log::error!("Failed to start build mode for character {}", name);
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
                player::plr_map_remove(companion_id);
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
        if !core::types::Item::is_sane_item(item_id) {
            return false;
        }

        Repository::with_characters_mut(|characters| {
            if !characters[cn].is_living_character(cn) {
                return false;
            }

            Repository::with_items_mut(|items| {
                // Remove from citem
                if characters[cn].citem as usize == item_id {
                    characters[cn].citem = 0;
                } else {
                    // Try inventory
                    let mut found = false;
                    for n in 0..40 {
                        if characters[cn].item[n] as usize == item_id {
                            characters[cn].item[n] = 0;
                            found = true;
                            break;
                        }
                    }
                    if !found {
                        // Try worn
                        for n in 0..20 {
                            if characters[cn].worn[n] as usize == item_id {
                                characters[cn].worn[n] = 0;
                                found = true;
                                break;
                            }
                        }
                        if !found {
                            return false;
                        }
                    }
                }

                // Clear item carriage
                items[item_id].x = 0;
                items[item_id].y = 0;
                items[item_id].carried = 0;

                // Mark character for update
                characters[cn].set_do_update_flags();

                // Call update hook in State so that network/clients can be informed
                State::with(|state| {
                    state.do_update_char(cn);
                });

                true
            })
        })
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

    pub fn change_pass(cn: usize, co: usize, pass: &str) -> i32 {
        if !Character::is_sane_character(cn) || !Character::is_sane_character(co) {
            return 0;
        }

        Repository::with_characters_mut(|characters| {
            let character = &mut characters[co];

            // Set new password (simplified - original used crypt)
            let pass_hash = pass.as_bytes();
            character.pass1 = pass_hash
                .iter()
                .take(4)
                .fold(0u32, |acc, &b| (acc << 8) | b as u32);
            character.pass2 = pass_hash
                .iter()
                .skip(4)
                .take(4)
                .fold(0u32, |acc, &b| (acc << 8) | b as u32);

            character.set_do_update_flags();

            State::with(|state| {
                log::info!("Password changed for character {}", character.get_name());
            });

            1
        })
    }

    pub fn remove_item(cn: usize, item_id: usize) -> i32 {
        if !Character::is_sane_character(cn) || !core::types::Item::is_sane_item(item_id) {
            return 0;
        }

        Repository::with_characters_mut(|characters| {
            let character = &mut characters[cn];

            // Check inventory slots
            for n in 0..40 {
                if character.item[n] == item_id as u32 {
                    character.item[n] = 0;
                    Repository::with_items_mut(|items| {
                        items[item_id].carried = 0;
                    });
                    character.set_do_update_flags();
                    return 1;
                }
            }

            // Check worn/wielded slots
            for n in 0..20 {
                if character.worn[n] == item_id as u32 {
                    character.worn[n] = 0;
                    Repository::with_items_mut(|items| {
                        items[item_id].carried = 0;
                    });
                    character.set_do_update_flags();
                    return 1;
                }
            }

            0
        })
    }

    pub fn drop_item_fuzzy(nr: usize, x: usize, y: usize) -> bool {
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
            if Self::drop_item(nr, *try_x, *try_y) {
                return true;
            }
        }

        false
    }

    pub fn goto(cn: usize, co: usize, cx: &str, cy: &str) {
        if !Character::is_sane_character(cn) || !Character::is_sane_character(co) {
            return;
        }

        let (x, y) = Repository::with_characters(|characters| {
            let character = &characters[cn];
            let mut target_x = character.x as usize;
            let mut target_y = character.y as usize;

            // Parse direction modifiers
            if cx.starts_with('n') || cx.starts_with('N') {
                if let Ok(val) = cx[1..].parse::<i32>() {
                    target_y = (target_y as i32 - val).max(1) as usize;
                }
            } else if cx.starts_with('s') || cx.starts_with('S') {
                if let Ok(val) = cx[1..].parse::<i32>() {
                    target_y = (target_y as i32 + val)
                        .min((core::constants::SERVER_MAPY - 2) as i32)
                        as usize;
                }
            } else if cx.starts_with('e') || cx.starts_with('E') {
                if let Ok(val) = cx[1..].parse::<i32>() {
                    target_x = (target_x as i32 + val)
                        .min((core::constants::SERVER_MAPX - 2) as i32)
                        as usize;
                }
            } else if cx.starts_with('w') || cx.starts_with('W') {
                if let Ok(val) = cx[1..].parse::<i32>() {
                    target_x = (target_x as i32 - val).max(1) as usize;
                }
            } else if let Ok(val) = cx.parse::<usize>() {
                target_x = val.clamp(1, (core::constants::SERVER_MAPX - 2) as usize);
            }

            if let Ok(val) = cy.parse::<usize>() {
                target_y = val.clamp(1, (core::constants::SERVER_MAPY - 2) as usize);
            }

            (target_x, target_y)
        });

        Self::transfer_char(co, x, y);
        State::with(|state| {
            state.do_character_log(
                cn,
                core::types::FontColor::Green,
                &format!("Character {} teleported to ({}, {})", co, x, y),
            );
        });
    }

    pub fn info(cn: usize, co: usize) {
        if !Character::is_sane_character(cn) || !Character::is_sane_character(co) {
            return;
        }

        Repository::with_characters(|characters| {
            let target = &characters[co];

            State::with(|state| {
                let sprite_to_print = target.sprite;
                let hp_current_to_print = target.hp[5];
                let hp_max_to_print = target.hp[0];
                let end_current_to_print = target.end[5];
                let end_max_to_print = target.end[0];
                let mana_current_to_print = target.mana[5];
                let mana_max_to_print = target.mana[0];
                state.do_character_log(
                    cn,
                    core::types::FontColor::Green,
                    &format!(
                        "Character Info for {}: ID={}, Sprite={}, HP={}/{}, End={}/{}, Mana={}/{}",
                        target.get_name(),
                        co,
                        sprite_to_print,
                        hp_current_to_print,
                        hp_max_to_print,
                        end_current_to_print,
                        end_max_to_print,
                        mana_current_to_print,
                        mana_max_to_print
                    ),
                );

                let x_to_print = target.x;
                let y_to_print = target.y;
                let flags_to_print = target.flags;
                let points_to_print = target.points;
                state.do_character_log(
                    cn,
                    core::types::FontColor::Green,
                    &format!(
                        "  Position: ({}, {}), Flags: {:x}, Points: {}",
                        x_to_print, y_to_print, flags_to_print, points_to_print
                    ),
                );
            });
        });
    }

    pub fn iinfo(cn: usize, item_index: usize) {
        if !Character::is_sane_character(cn) || !core::types::Item::is_sane_item(item_index) {
            return;
        }

        Repository::with_items(|items| {
            let item = &items[item_index];

            State::with(|state| {
                let sprite_0_to_print = item.sprite[0];
                let sprite_1_to_print = item.sprite[1];
                let carried_to_print = item.carried;
                state.do_character_log(
                    cn,
                    core::types::FontColor::Green,
                    &format!(
                        "Item Info: ID={}, Sprite=[{},{}], Carried={}, Used={}",
                        item_index,
                        sprite_0_to_print,
                        sprite_1_to_print,
                        carried_to_print,
                        item.used
                    ),
                );
            });
        });
    }

    pub fn tinfo(cn: usize, template: usize) {
        if !Character::is_sane_character(cn) || !core::types::Item::is_sane_item_template(template)
        {
            return;
        }

        Repository::with_item_templates(|templates| {
            let tmpl = &templates[template];

            State::with(|state| {
                let sprite_0_to_print = tmpl.sprite[0];
                let sprite_1_to_print = tmpl.sprite[1];
                let used_to_print = tmpl.used;
                state.do_character_log(
                    cn,
                    core::types::FontColor::Green,
                    &format!(
                        "Template Info: ID={}, Sprite=[{},{}], Used={}",
                        template, sprite_0_to_print, sprite_1_to_print, used_to_print
                    ),
                );
            });
        });
    }

    pub fn unique(cn: usize) {
        if !Character::is_sane_character(cn) {
            return;
        }

        Repository::with_items(|items| {
            State::with(|state| {
                state.do_character_log(cn, core::types::FontColor::Green, "Listing unique items:");
                for i in 1..core::constants::MAXITEM as usize {
                    if items[i].used != core::constants::USE_EMPTY && items[i].is_unique() {
                        let sprite_0_to_print = items[i].sprite[0];
                        let sprite_1_to_print = items[i].sprite[1];
                        let carried_to_print = items[i].carried;
                        state.do_character_log(
                            cn,
                            core::types::FontColor::Green,
                            &format!(
                                "  Item {}: Sprite=[{},{}], Carried={}",
                                i, sprite_0_to_print, sprite_1_to_print, carried_to_print
                            ),
                        );
                    }
                }
            });
        });
    }

    pub fn who(cn: usize) {
        if !Character::is_sane_character(cn) {
            return;
        }

        Repository::with_characters(|characters| {
            State::with(|state| {
                state.do_character_log(cn, core::types::FontColor::Green, "Online characters:");
                for i in 1..core::constants::MAXCHARS as usize {
                    if characters[i].is_living_character(i) && characters[i].is_player() {
                        let points_to_print = characters[i].points;
                        state.do_character_log(
                            cn,
                            core::types::FontColor::Green,
                            &format!(
                                "  {}: Points={} {}",
                                characters[i].get_name(),
                                points_to_print,
                                if characters[i].flags
                                    & core::constants::CharacterFlags::CF_GOD.bits()
                                    != 0
                                {
                                    "[GOD]"
                                } else {
                                    ""
                                }
                            ),
                        );
                    }
                }
            });
        });
    }

    pub fn implist(cn: usize) {
        if !Character::is_sane_character(cn) {
            return;
        }

        Repository::with_characters(|characters| {
            State::with(|state| {
                state.do_character_log(cn, core::types::FontColor::Green, "Immortal characters:");
                for i in 1..core::constants::MAXCHARS as usize {
                    if characters[i].is_living_character(i) {
                        if characters[i].flags & core::constants::CharacterFlags::CF_IMMORTAL.bits()
                            != 0
                            || characters[i].flags & core::constants::CharacterFlags::CF_GOD.bits()
                                != 0
                        {
                            let flags_to_print = characters[i].flags;
                            state.do_character_log(
                                cn,
                                core::types::FontColor::Green,
                                &format!(
                                    "  {}: Flags={:x}",
                                    characters[i].get_name(),
                                    flags_to_print
                                ),
                            );
                        }
                    }
                }
            });
        });
    }

    pub fn user_who(cn: usize) {
        if !Character::is_sane_character(cn) {
            return;
        }

        let mut count = 0;
        Repository::with_characters(|characters| {
            State::with(|state| {
                state.do_character_log(cn, core::types::FontColor::Green, "Players online:");
                for i in 1..core::constants::MAXCHARS as usize {
                    if characters[i].is_living_character(i) && characters[i].is_player() {
                        count += 1;
                        let points_to_print = characters[i].points;
                        state.do_character_log(
                            cn,
                            core::types::FontColor::Green,
                            &format!(
                                "  {} - Points: {}",
                                characters[i].get_name(),
                                points_to_print
                            ),
                        );
                    }
                }
                state.do_character_log(
                    cn,
                    core::types::FontColor::Green,
                    &format!("Total: {} players", count),
                );
            });
        });
    }

    pub fn top(cn: usize) {
        if !Character::is_sane_character(cn) {
            return;
        }

        // Simple top players list - would need proper ranking system
        Repository::with_characters(|characters| {
            State::with(|state| {
                state.do_character_log(cn, core::types::FontColor::Green, "Top players by points:");
                // This is simplified - original had more complex ranking
                for i in 1..core::constants::MAXCHARS as usize {
                    if characters[i].is_living_character(i) && characters[i].is_player() {
                        if characters[i].points > 100000 {
                            let points_to_print = characters[i].points;
                            state.do_character_log(
                                cn,
                                core::types::FontColor::Green,
                                &format!(
                                    "  {}: Points={}",
                                    characters[i].get_name(),
                                    points_to_print
                                ),
                            );
                        }
                    }
                }
            });
        });
    }

    pub fn create(cn: usize, x: i32) {
        if !Character::is_sane_character(cn) {
            return;
        }

        let item_id = Self::create_item(x as usize);

        if let Some(item_id) = item_id {
            if Self::give_character_item(cn, item_id) {
                State::with(|state| {
                    state.do_character_log(
                        cn,
                        core::types::FontColor::Green,
                        &format!("Created item {} and gave to character {}", item_id, cn),
                    );
                });
            } else {
                State::with(|state| {
                    state.do_character_log(
                        cn,
                        core::types::FontColor::Red,
                        &format!("Failed to give item {} to character {}", item_id, cn),
                    );
                });
            }
        } else {
            State::with(|state| {
                state.do_character_log(
                    cn,
                    core::types::FontColor::Red,
                    &format!("Failed to create item from template {}", x),
                );
            });
        }
    }

    pub fn find_next_char(start_index: usize, spec1: &str, spec2: &str) -> i32 {
        Repository::with_characters(|characters| {
            for i in start_index..core::constants::MAXCHARS as usize {
                if !characters[i].is_living_character(i) {
                    continue;
                }

                let name = characters[i].get_name().to_lowercase();
                let reference = String::from_utf8_lossy(&characters[i].reference)
                    .trim_end_matches('\0')
                    .to_lowercase();

                let spec1_lower = spec1.to_lowercase();
                let spec2_lower = spec2.to_lowercase();

                if !spec1.is_empty()
                    && !name.contains(&spec1_lower)
                    && !reference.contains(&spec1_lower)
                {
                    continue;
                }

                if !spec2.is_empty()
                    && !name.contains(&spec2_lower)
                    && !reference.contains(&spec2_lower)
                {
                    continue;
                }

                return i as i32;
            }
            0
        })
    }

    pub fn invis(looker: usize, target: usize) -> i32 {
        if !Character::is_sane_character(looker) || !Character::is_sane_character(target) {
            return 1;
        }

        Repository::with_characters(|characters| {
            let looker_char = &characters[looker];
            let target_char = &characters[target];

            // Check if target is invisible
            if target_char.flags & core::constants::CharacterFlags::CF_INVISIBLE.bits() != 0 {
                // Check if looker can see invisible
                if looker_char.flags & core::constants::CharacterFlags::CF_INFRARED.bits() == 0 {
                    return 1;
                }
            }

            0
        })
    }

    pub fn summon(cn: usize, spec1: &str, spec2: &str, spec3: &str) {
        if !Character::is_sane_character(cn) {
            return;
        }

        let target_cn = Self::find_next_char(1, spec1, spec2);

        if target_cn <= 0 {
            State::with(|state| {
                state.do_character_log(
                    cn,
                    core::types::FontColor::Red,
                    &format!("Could not find character matching '{}' '{}'", spec1, spec2),
                );
            });
            return;
        }

        let (x, y) = Repository::with_characters(|characters| {
            let summoner = &characters[cn];
            let mut target_x = summoner.x as i32;
            let mut target_y = summoner.y as i32;

            // Calculate position in front of summoner based on direction
            match summoner.dir {
                0 => target_x += 1, // DX_RIGHT
                1 => {
                    target_x += 1;
                    target_y -= 1;
                } // DX_RIGHTUP
                2 => target_y -= 1, // DX_UP
                3 => {
                    target_x -= 1;
                    target_y -= 1;
                } // DX_LEFTUP
                4 => target_x -= 1, // DX_LEFT
                5 => {
                    target_x -= 1;
                    target_y += 1;
                } // DX_LEFTDOWN
                6 => target_y += 1, // DX_DOWN
                7 => {
                    target_x += 1;
                    target_y += 1;
                } // DX_RIGHTDOWN
                _ => {}
            }

            (
                target_x
                    .max(1)
                    .min((core::constants::SERVER_MAPX - 2) as i32) as usize,
                target_y
                    .max(1)
                    .min((core::constants::SERVER_MAPY - 2) as i32) as usize,
            )
        });

        Self::transfer_char(target_cn as usize, x, y);

        State::with(|state| {
            state.do_character_log(
                cn,
                core::types::FontColor::Green,
                &format!(
                    "Summoned character {} to position ({}, {})",
                    target_cn, x, y
                ),
            );
        });
    }

    pub fn mirror(cn: usize, spec1: &str, spec2: &str) {
        if !Character::is_sane_character(cn) {
            return;
        }

        // Parse bonus from spec2
        let bonus = if !spec2.is_empty() {
            spec2.parse::<i32>().unwrap_or(0)
        } else {
            0
        };

        // Parse character number or find by name
        let co = if spec1.is_empty() {
            State::with(|state| {
                state.do_character_log(
                    cn,
                    core::types::FontColor::Red,
                    "create mirror-enemy of whom?",
                );
            });
            return;
        } else if spec1.chars().all(|c| c.is_ascii_digit()) {
            spec1.parse::<usize>().unwrap_or(0)
        } else {
            Self::find_next_char(1, spec1, "") as usize
        };

        if !Character::is_sane_character(co) {
            State::with(|state| {
                state.do_character_log(cn, core::types::FontColor::Red, "No such character.");
            });
            return;
        }

        Repository::with_characters(|characters| {
            if characters[co].flags & core::constants::CharacterFlags::CF_BODY.bits() != 0 {
                State::with(|state| {
                    state.do_character_log(
                        cn,
                        core::types::FontColor::Red,
                        "Character recently deceased.",
                    );
                });
                return;
            }

            if !characters[co].is_player() {
                State::with(|state| {
                    state.do_character_log(
                        cn,
                        core::types::FontColor::Red,
                        &format!(
                            "{} is not a player, and you can't mirror monsters!",
                            characters[co].get_name()
                        ),
                    );
                });
                return;
            }

            if co == cn {
                State::with(|state| {
                    state.do_character_log(
                        cn,
                        core::types::FontColor::Yellow,
                        "You want an enemy? Here it is...!",
                    );
                });
            }
        });

        // Create mirror character with template 968
        let cc = match Self::create_char(968, false) {
            Some(cc) => cc as usize,
            None => {
                State::with(|state| {
                    state.do_character_log(
                        cn,
                        core::types::FontColor::Red,
                        "god_create_char() failed.",
                    );
                });
                return;
            }
        };

        // Copy attributes from target to mirror
        Repository::with_characters_mut(|characters| {
            let target_name_bytes = characters[co].name.clone();
            let target_sprite = characters[co].sprite;
            let target_attrib = characters[co].attrib.clone();
            let target_hp = characters[co].hp;
            let target_end = characters[co].end;
            let target_mana = characters[co].mana;
            let target_skill = characters[co].skill.clone();
            let target_kindred = characters[co].kindred as u32;
            let caster_weapon = characters[cn].weapon;
            let caster_armor = characters[cn].armor;
            let caster_x = characters[cn].x;
            let caster_y = characters[cn].y;

            let mirror = &mut characters[cc];
            mirror.name = target_name_bytes;
            mirror.sprite = target_sprite;

            // Copy attributes
            for i in 0..5 {
                mirror.attrib[i][0] = target_attrib[i][0];
            }

            // Copy max HP/END/MANA
            mirror.hp[0] = target_hp[0];
            mirror.end[0] = target_end[0];
            mirror.mana[0] = target_mana[0];

            // Copy skills
            for i in 1..35 {
                mirror.skill[i][0] = target_skill[i][0];
            }

            // Calculate hand-to-hand skill based on kindred
            if target_kindred
                & (core::constants::KIN_TEMPLAR
                    | core::constants::KIN_ARCHTEMPLAR
                    | core::constants::KIN_SEYAN_DU)
                != 0
            {
                // TH -> hand2hand (str,str,agi)
                mirror.skill[0][0] = (target_skill[6][0] as i32
                    + bonus
                    + (target_attrib[4][0] as i32 - target_attrib[0][0] as i32) / 5)
                    .clamp(0, 255) as u8;
            } else if target_kindred
                & (core::constants::KIN_HARAKIM | core::constants::KIN_ARCHHARAKIM)
                != 0
            {
                // Dag-> hand2hand (wil,agi,int)
                mirror.skill[0][0] = (target_skill[2][0] as i32
                    + bonus
                    + (target_attrib[2][0] as i32 - target_attrib[4][0] as i32) / 5)
                    .clamp(0, 255) as u8;
            } else if target_kindred
                & (core::constants::KIN_MERCENARY
                    | core::constants::KIN_SORCERER
                    | core::constants::KIN_WARRIOR)
                != 0
            {
                // Swo-> hand2hand (wil,agi,str)
                mirror.skill[0][0] = (target_skill[3][0] as i32 + bonus).clamp(0, 255) as u8;
            }

            mirror.weapon = caster_weapon;
            mirror.armor = caster_armor;
            mirror.set_do_update_flags();

            // Drop the mirror at caster's position
            Self::drop_char_fuzzy(cc, caster_x as usize, caster_y as usize);

            // Add target as enemy
            crate::driver::npc_add_enemy(cc, co, true);

            let target_name = String::from_utf8_lossy(&target_name_bytes)
                .trim_end_matches('\0')
                .to_string();
            State::with(|state| {
                state.do_character_log(
                    cn,
                    core::types::FontColor::Green,
                    &format!("Mirror of {} active (bonus: {})", target_name, bonus),
                );
            });
        });
    }

    pub fn thrall(cn: usize, spec1: &str, spec2: &str) -> i32 {
        if !Character::is_sane_character(cn) {
            return 0;
        }

        // Check for arguments
        if spec1.is_empty() {
            State::with(|state| {
                state.do_character_log(cn, core::types::FontColor::Red, "enthrall whom?");
            });
            return 0;
        }

        let co = if spec2.is_empty() {
            // Only one argument - parse character number
            let co = spec1.parse::<usize>().unwrap_or(0);

            if !Character::is_sane_character(co) {
                State::with(|state| {
                    state.do_character_log(cn, core::types::FontColor::Red, "No such character.");
                });
                return 0;
            }

            Repository::with_characters(|characters| {
                if characters[co].flags & core::constants::CharacterFlags::CF_BODY.bits() != 0 {
                    let corpse_owner = characters[co].data[core::constants::CHD_COMPANION];
                    State::with(|state| {
                        state.do_character_log(
                            cn,
                            core::types::FontColor::Red,
                            &format!("Character recently deceased; try {}.", corpse_owner),
                        );
                    });
                    return 0;
                }

                if co == cn {
                    State::with(|state| {
                        state.do_character_log(
                            cn,
                            core::types::FontColor::Red,
                            "You can't enthrall yourself!",
                        );
                    });
                    return 0;
                }

                co
            })
        } else {
            // At least 2 arguments - find character by name/rank
            let mut co = 0usize;
            loop {
                co = Self::find_next_char(co, spec1, spec2) as usize;
                if co == 0 {
                    break;
                }
                if co == cn {
                    continue; // ignore self
                }
                let should_continue = Repository::with_characters(|characters| {
                    characters[co].flags & core::constants::CharacterFlags::CF_BODY.bits() != 0
                });
                if should_continue {
                    continue; // ignore bodies
                }
                break;
            }

            if co == 0 {
                State::with(|state| {
                    state.do_character_log(
                        cn,
                        core::types::FontColor::Red,
                        &format!("Couldn't find a {} {}.", spec1, spec2),
                    );
                });
                return 0;
            }
            co
        };

        // Validate target
        let validation_failed = Repository::with_characters(|characters| {
            if characters[co].is_player() {
                State::with(|state| {
                    state.do_character_log(
                        cn,
                        core::types::FontColor::Red,
                        &format!(
                            "{} is a player, and you can't enthrall players!",
                            characters[co].get_name()
                        ),
                    );
                });
                return true;
            }

            // Check if already a companion/thrall (data[42] is group, companions have group 65536+cn)
            if characters[co].data[42] > 65536 {
                State::with(|state| {
                    state.do_character_log(
                        cn,
                        core::types::FontColor::Red,
                        &format!(
                            "{} is a companion/thrall, and you can't enthrall them!",
                            characters[co].get_name()
                        ),
                    );
                });
                return true;
            }

            false
        });

        if validation_failed {
            return 0;
        }

        // Calculate position in front of summoner
        let (x, y) = Repository::with_characters(|characters| {
            let summoner = &characters[cn];
            let mut x = summoner.x as i32;
            let mut y = summoner.y as i32;

            match summoner.dir {
                0 => x += 1, // DX_RIGHT
                1 => {
                    x += 1;
                    y -= 1;
                } // DX_RIGHTUP
                2 => y -= 1, // DX_UP
                3 => {
                    x -= 1;
                    y -= 1;
                } // DX_LEFTUP
                4 => x -= 1, // DX_LEFT
                5 => {
                    x -= 1;
                    y += 1;
                } // DX_LEFTDOWN
                6 => y += 1, // DX_DOWN
                7 => {
                    x += 1;
                    y += 1;
                } // DX_RIGHTDOWN
                _ => {}
            }

            (x as usize, y as usize)
        });

        // Get target template and create thrall
        let target_template = Repository::with_characters(|characters| characters[co].temp);

        let ct = match Self::create_char(target_template as usize, true) {
            Some(ct) => ct as usize,
            None => {
                State::with(|state| {
                    state.do_character_log(
                        cn,
                        core::types::FontColor::Red,
                        "god_create_char() failed.",
                    );
                });
                return 0;
            }
        };

        // Configure the thrall
        Repository::with_characters_mut(|characters| {
            let target_name_bytes = characters[co].name.clone();
            let target_reference = characters[co].reference.clone();
            let target_description = characters[co].description.clone();

            let thrall = &mut characters[ct];
            thrall.name = target_name_bytes;
            thrall.reference = target_reference;
            thrall.description = target_description;

            // Make thrall act like a ghost companion
            thrall.temp = core::constants::CT_COMPANION as u16;
            let ticker = Repository::with_globals(|globals| globals.ticker);
            thrall.data[64] = (ticker + 7 * 24 * 3600 * core::constants::TICKS) as i32; // die in one week
            thrall.data[42] = (65536 + cn) as i32; // set group
            thrall.data[59] = (65536 + cn) as i32; // protect all other members of this group

            // Make thrall harmless
            thrall.data[24] = 0; // do not interfere in fights
            thrall.data[36] = 0; // no walking around
            thrall.data[43] = 0; // don't attack anyone
            thrall.data[80] = 0; // no enemies
            thrall.data[63] = cn as i32; // obey and protect enthraller

            thrall.flags |= core::constants::CharacterFlags::CF_SHUTUP.bits()
                | core::constants::CharacterFlags::CF_THRALL.bits();

            // Remove labyrinth items from worn slots
            for n in 0..20 {
                if thrall.worn[n] != 0 {
                    let item_id = thrall.worn[n] as usize;
                    Repository::with_items_mut(|items| {
                        if items[item_id].flags & core::constants::ItemFlags::IF_LABYDESTROY.bits()
                            != 0
                        {
                            items[item_id].used = 0;
                            thrall.worn[n] = 0;
                        }
                    });
                }
            }

            // Remove labyrinth items from inventory
            for n in 0..40 {
                if thrall.item[n] != 0 {
                    let item_id = thrall.item[n] as usize;
                    Repository::with_items_mut(|items| {
                        if items[item_id].flags & core::constants::ItemFlags::IF_LABYDESTROY.bits()
                            != 0
                        {
                            items[item_id].used = 0;
                            thrall.item[n] = 0;
                        }
                    });
                }
            }

            // Remove labyrinth item from carried item
            if thrall.citem != 0 {
                let item_id = thrall.citem as usize;
                Repository::with_items_mut(|items| {
                    if items[item_id].flags & core::constants::ItemFlags::IF_LABYDESTROY.bits() != 0
                    {
                        items[item_id].used = 0;
                        thrall.citem = 0;
                    }
                });
            }

            // Drop thrall at calculated position
            if !Self::drop_char_fuzzy(ct, x, y) {
                State::with(|state| {
                    state.do_character_log(
                        cn,
                        core::types::FontColor::Red,
                        "god_drop_char_fuzzy() called from god_thrall() failed.",
                    );
                });
                Self::destroy_items(ct);
                thrall.used = core::constants::USE_EMPTY;
                return 0;
            }

            let target_name = String::from_utf8_lossy(&target_name_bytes)
                .trim_end_matches('\0')
                .to_string();
            State::with(|state| {
                state.do_character_log(
                    cn,
                    core::types::FontColor::Green,
                    &format!("{} was enthralled.", target_name),
                );
            });

            ct as i32
        })
    }

    pub fn tavern(cn: usize) {
        if !Character::is_sane_character(cn) {
            return;
        }

        Repository::with_characters_mut(|characters| {
            let character = &mut characters[cn];
            character.hp[5] = character.hp[0];
            character.end[5] = character.end[0];
            character.mana[5] = character.mana[0];
            character.set_do_update_flags();
        });

        State::with(|state| {
            state.do_character_log(
                cn,
                core::types::FontColor::Green,
                &format!("Character {} fully healed at tavern", cn),
            );
        });
    }

    pub fn raise_char(cn: usize, co: usize, value: i32) {
        if !Character::is_sane_character(cn) || !Character::is_sane_character(co) {
            return;
        }

        if value < 1 || value > 5 {
            State::with(|state| {
                state.do_character_log(
                    cn,
                    core::types::FontColor::Red,
                    &format!("Invalid raise value: {}", value),
                );
            });
            return;
        }

        Repository::with_characters_mut(|characters| {
            let target = &mut characters[co];

            // Raise stats based on value
            for _ in 0..value {
                target.attrib[0][0] = (target.attrib[0][0] + 1).min(127);
                target.attrib[0][1] = (target.attrib[0][1] + 1).min(127);
                target.attrib[0][2] = (target.attrib[0][2] + 1).min(127);
                target.attrib[0][3] = (target.attrib[0][3] + 1).min(127);
                target.attrib[0][4] = (target.attrib[0][4] + 1).min(127);
            }

            target.set_do_update_flags();

            State::with(|state| {
                state.do_character_log(
                    cn,
                    core::types::FontColor::Green,
                    &format!("Raised character {} stats by {}", target.get_name(), value),
                );
            });
        });
    }

    pub fn lower_char(cn: usize, co: usize, value: i32) {
        if !Character::is_sane_character(cn) || !Character::is_sane_character(co) {
            return;
        }

        if value < 1 || value > 5 {
            State::with(|state| {
                state.do_character_log(
                    cn,
                    core::types::FontColor::Red,
                    &format!("Invalid lower value: {}", value),
                );
            });
            return;
        }

        Repository::with_characters_mut(|characters| {
            let target = &mut characters[co];

            // Lower stats based on value
            for _ in 0..value {
                target.attrib[0][0] = (target.attrib[0][0] - 1).max(1);
                target.attrib[0][1] = (target.attrib[0][1] - 1).max(1);
                target.attrib[0][2] = (target.attrib[0][2] - 1).max(1);
                target.attrib[0][3] = (target.attrib[0][3] - 1).max(1);
                target.attrib[0][4] = (target.attrib[0][4] - 1).max(1);
            }

            target.set_do_update_flags();

            State::with(|state| {
                state.do_character_log(
                    cn,
                    core::types::FontColor::Green,
                    &format!("Lowered character {} stats by {}", target.get_name(), value),
                );
            });
        });
    }

    pub fn gold_char(cn: usize, co: usize, value: i32, silver: &str) {
        if !Character::is_sane_character(cn) || !Character::is_sane_character(co) {
            return;
        }

        let mut total_silver = value * 100; // value is in gold

        // If silver string is provided, parse additional silver
        if !silver.is_empty() {
            if let Ok(silver_val) = silver.parse::<i32>() {
                total_silver += silver_val;
            }
        }

        Repository::with_characters_mut(|characters| {
            let target = &mut characters[co];
            target.gold = (target.gold + total_silver).max(0);
            target.set_do_update_flags();

            State::with(|state| {
                state.do_character_log(
                    cn,
                    core::types::FontColor::Green,
                    &format!(
                        "Gave {} silver to character {}",
                        total_silver,
                        target.get_name()
                    ),
                );
            });
        });
    }

    pub fn erase(cn: usize, co: usize, erase_player: i32) {
        if co == 0 {
            State::with(|state| {
                state.do_character_log(cn, core::types::FontColor::Red, "No such character.");
            });
            return;
        }

        // Check if character is sane
        if !Character::is_sane_character(co) {
            State::with(|state| {
                state.do_character_log(
                    cn,
                    core::types::FontColor::Red,
                    &format!("Bad character number: {}", co),
                );
            });
            return;
        }

        // Check if character is used
        let (is_used, is_player_or_usurp, character_name) =
            Repository::with_characters(|characters| {
                let character = &characters[co];
                let is_used = character.used != core::constants::USE_EMPTY;
                let is_player_or_usurp = (character.flags
                    & (core::constants::CharacterFlags::CF_PLAYER.bits()
                        | core::constants::CharacterFlags::CF_USURP.bits()))
                    != 0;
                let name = character.name.clone();
                (is_used, is_player_or_usurp, name)
            });

        if !is_used {
            State::with(|state| {
                state.do_character_log(
                    cn,
                    core::types::FontColor::Red,
                    &format!("Character {} is unused anyway.", co),
                );
            });
            return;
        }

        // Check if player/QM but erase_player is false
        if is_player_or_usurp && erase_player == 0 {
            let name_str = String::from_utf8_lossy(&character_name)
                .trim_end_matches('\0')
                .to_string();
            State::with(|state| {
                state.do_character_log(
                    cn,
                    core::types::FontColor::Red,
                    &format!("{} is a player or QM; use #PERASE if you insist.", name_str),
                );
            });
            return;
        }

        // Check if erase_player is true but character is not player/usurp
        if erase_player != 0 && !is_player_or_usurp {
            let name_str = String::from_utf8_lossy(&character_name)
                .trim_end_matches('\0')
                .to_string();
            State::with(|state| {
                state.do_character_log(
                    cn,
                    core::types::FontColor::Red,
                    &format!("{} is not a player; use #ERASE for NPCs.", name_str),
                );
            });
            return;
        }

        if erase_player != 0 {
            // Erasing a player
            let name_str = String::from_utf8_lossy(&character_name)
                .trim_end_matches('\0')
                .to_string();

            Repository::with_characters(|ch| {
                player::plr_logout(co as usize, ch[co].player as usize, LogoutReason::Shutdown);
            });

            Repository::with_characters_mut(|characters| {
                characters[co].used = core::constants::USE_EMPTY;
            });

            // TODO: chlog(cn, "IMP: Erased player %d (%-.20s).", co, ch[co].name);
            State::with(|state| {
                state.do_character_log(
                    cn,
                    core::types::FontColor::Yellow,
                    &format!("Player {} ({}) is no more.", co, name_str),
                );
            });
        } else {
            // Erasing an NPC
            let name_str = String::from_utf8_lossy(&character_name)
                .trim_end_matches('\0')
                .to_string();

            // Call do_char_killed(0, co)
            State::with(|state| {
                state.do_character_killed(co, 0);
            });

            // TODO: chlog(cn, "IMP: Erased NPC %d (%-.20s).", co, ch[co].name);
            State::with(|state| {
                state.do_character_log(
                    cn,
                    core::types::FontColor::Yellow,
                    &format!("NPC {} ({}) is no more.", co, name_str),
                );
            });
        }
    }

    pub fn kick(cn: usize, co: usize) {
        // Check co == 0
        if co == 0 {
            State::with(|state| {
                state.do_character_log(cn, core::types::FontColor::Red, "No such character.");
            });
            return;
        }

        // Check if character is sane and used
        if !Character::is_sane_character(co) {
            State::with(|state| {
                state.do_character_log(
                    cn,
                    core::types::FontColor::Red,
                    &format!("Bad character number: {}", co),
                );
            });
            return;
        }

        let (is_used, character_name) = Repository::with_characters(|characters| {
            let character = &characters[co];
            let is_used = character.used != core::constants::USE_EMPTY;
            let name = character.name.clone();
            (is_used, name)
        });

        if !is_used {
            State::with(|state| {
                state.do_character_log(
                    cn,
                    core::types::FontColor::Red,
                    &format!("Character {} is unused anyway.", co),
                );
            });
            return;
        }

        let name_str = String::from_utf8_lossy(&character_name)
            .trim_end_matches('\0')
            .to_string();

        Repository::with_characters(|ch| {
            player::plr_logout(
                co as usize,
                ch[co].player as usize,
                LogoutReason::IdleTooLong,
            );
        });

        State::with(|state| {
            state.do_character_log(
                cn,
                core::types::FontColor::Yellow,
                &format!("Kicked {}.", name_str),
            );
        });

        // TODO: chlog(cn, "IMP: kicked %s (%d)", ch[co].name, co);

        // Set CF_KICKED flag
        Repository::with_characters_mut(|characters| {
            characters[co].flags |= core::constants::CharacterFlags::CF_KICKED.bits();
        });
    }

    pub fn skill(cn: usize, co: usize, n: i32, val: i32) {
        if !Character::is_sane_character(cn) || !Character::is_sane_character(co) {
            return;
        }

        if n < 0 || n >= 50 {
            State::with(|state| {
                state.do_character_log(
                    cn,
                    core::types::FontColor::Red,
                    &format!("Invalid skill number: {}", n),
                );
            });
            return;
        }

        let val = val.clamp(0, 127);

        Repository::with_characters_mut(|characters| {
            let target = &mut characters[co];
            target.skill[n as usize][0] = val as u8;
            target.skill[n as usize][1] = val as u8;
            target.set_do_update_flags();

            State::with(|state| {
                state.do_character_log(
                    cn,
                    core::types::FontColor::Green,
                    &format!(
                        "Set skill {} to {} for character {}",
                        n,
                        val,
                        target.get_name()
                    ),
                );
            });
        });
    }

    pub fn donate_item(item_id: usize, place: i32) {
        // Donation locations:
        // Temple of Skua: (497, 512)
        // Temple of the Purple One: (560, 542)
        const DON_X: [usize; 2] = [497, 560];
        const DON_Y: [usize; 2] = [512, 542];

        if !core::types::Item::is_sane_item(item_id) {
            log::warn!("Attempt to god_donate_item {}", item_id);
            return;
        }

        // If place is not 1 or 2, pick randomly
        let place = if place < 1 || place > 2 {
            use rand::Rng;
            rand::thread_rng().gen_range(1..=2)
        } else {
            place
        };

        let x = DON_X[(place - 1) as usize];
        let y = DON_Y[(place - 1) as usize];

        // Try to drop the item at the donation location
        if !Self::drop_item_fuzzy(item_id, x, y) {
            // If drop fails, destroy the item
            Repository::with_items_mut(|items| {
                items[item_id].used = core::constants::USE_EMPTY;
            });
        }
    }

    pub fn set_flag(cn: usize, co: usize, flag: u64) {
        if !Character::is_sane_character(cn) || !Character::is_sane_character(co) {
            return;
        }

        Repository::with_characters_mut(|characters| {
            let target = &mut characters[co];

            // Toggle the flag
            if target.flags & flag != 0 {
                target.flags &= !flag;
                State::with(|state| {
                    state.do_character_log(
                        cn,
                        core::types::FontColor::Green,
                        &format!(
                            "Removed flag {:x} from character {}",
                            flag,
                            target.get_name()
                        ),
                    );
                });
            } else {
                target.flags |= flag;
                State::with(|state| {
                    state.do_character_log(
                        cn,
                        core::types::FontColor::Green,
                        &format!("Added flag {:x} to character {}", flag, target.get_name()),
                    );
                });
            }

            target.set_do_update_flags();
        });
    }

    pub fn set_gflag(cn: usize, flag: i32) {
        if !Character::is_sane_character(cn) {
            return;
        }

        Repository::with_globals_mut(|globals| {
            let flag_bit = 1i32 << flag;

            if globals.flags & flag_bit != 0 {
                globals.flags &= !flag_bit;
                State::with(|state| {
                    state.do_character_log(
                        cn,
                        core::types::FontColor::Green,
                        &format!("Removed global flag {}", flag),
                    );
                });
            } else {
                globals.flags |= flag_bit;
                State::with(|state| {
                    state.do_character_log(
                        cn,
                        core::types::FontColor::Green,
                        &format!("Added global flag {}", flag),
                    );
                });
            }
        });
    }

    pub fn set_purple(cn: usize, co: usize) {
        if !Character::is_sane_character(cn) || !Character::is_sane_character(co) {
            return;
        }

        Repository::with_characters_mut(|characters| {
            let target = &mut characters[co];

            // Toggle purple (PK) status
            // Assuming there's a PK flag in constants
            let pk_flag = 0x1000000u64; // Example PK flag

            if target.flags & pk_flag != 0 {
                target.flags &= !pk_flag;
                State::with(|state| {
                    state.do_character_log(
                        cn,
                        core::types::FontColor::Green,
                        &format!("Removed PK status from character {}", target.get_name()),
                    );
                });
            } else {
                target.flags |= pk_flag;
                State::with(|state| {
                    state.do_character_log(
                        cn,
                        core::types::FontColor::Green,
                        &format!("Added PK status to character {}", target.get_name()),
                    );
                });
            }

            target.set_do_update_flags();
        });
    }

    pub fn racechange(co: usize, temp: i32) {
        if !Character::is_sane_character(co) {
            return;
        }

        if temp < 0 || temp >= core::constants::MAXTCHARS as i32 {
            State::with(|state| {
                log::error!("Invalid character template: {}", temp);
            });
            return;
        }

        Repository::with_character_templates(|templates| {
            let template = &templates[temp as usize];

            if template.used == core::constants::USE_EMPTY {
                State::with(|state| {
                    log::error!("Template {} is not in use", temp);
                });
                return;
            }

            Repository::with_characters_mut(|characters| {
                let character = &mut characters[co];

                // Preserve important data
                let old_name = character.name;
                let old_items = character.item;
                let old_worn = character.worn;
                let old_gold = character.gold;

                // Apply template
                character.sprite = template.sprite;
                character.kindred = template.kindred;

                // Restore preserved data
                character.name = old_name;
                character.item = old_items;
                character.worn = old_worn;
                character.gold = old_gold;

                character.set_do_update_flags();

                State::with(|state| {
                    log::info!(
                        "Changed race of character {} to template {}",
                        character.get_name(),
                        temp
                    );
                });
            });
        });
    }

    pub fn save(cn: usize, co: usize) -> i32 {
        if !Character::is_sane_character(cn) || !Character::is_sane_character(co) {
            return 0;
        }

        Repository::with_characters(|characters| {
            if !characters[co].is_player() {
                State::with(|state| {
                    state.do_character_log(
                        cn,
                        core::types::FontColor::Red,
                        "Cannot save non-player character",
                    );
                });
                return 0;
            }

            State::with(|state| {
                state.do_character_log(
                    cn,
                    core::types::FontColor::Green,
                    &format!("Saving character {}", characters[co].get_name()),
                );
                // TODO: Actual save logic would write to disk
            });

            1
        })
    }

    pub fn mail_pass(cn: usize, co: usize) {
        if !Character::is_sane_character(cn) || !Character::is_sane_character(co) {
            return;
        }

        Repository::with_characters(|characters| {
            let character = &characters[co];

            State::with(|state| {
                state.do_character_log(
                    cn,
                    core::types::FontColor::Green,
                    &format!("Mailing password for character {}", character.get_name()),
                );
                // TODO: Actual mail logic
            });
        });
    }

    pub fn slap(cn: usize, co: usize) {
        if !Character::is_sane_character(cn) || !Character::is_sane_character(co) {
            return;
        }

        Repository::with_characters_mut(|characters| {
            let target = &mut characters[co];

            // Damage the target (hp[5] is total, hp[0] is max)
            let damage = (target.hp[0] / 10).max(1);
            target.hp[5] = (target.hp[5] as i32 - damage as i32).max(1) as u16;

            target.set_do_update_flags();

            State::with(|state| {
                state.do_character_log(
                    cn,
                    core::types::FontColor::Green,
                    &format!(
                        "Slapped character {} for {} damage",
                        target.get_name(),
                        damage
                    ),
                );
            });
        });
    }

    pub fn spritechange(cn: usize, co: usize, sprite: i32) {
        if !Character::is_sane_character(cn) || !Character::is_sane_character(co) {
            return;
        }

        if sprite < 0 || sprite > 10000 {
            State::with(|state| {
                state.do_character_log(
                    cn,
                    core::types::FontColor::Red,
                    &format!("Invalid sprite number: {}", sprite),
                );
            });
            return;
        }

        Repository::with_characters_mut(|characters| {
            let target = &mut characters[co];
            target.sprite = sprite as u16;
            target.set_do_update_flags();

            State::with(|state| {
                state.do_character_log(
                    cn,
                    core::types::FontColor::Green,
                    &format!(
                        "Changed sprite of character {} to {}",
                        target.get_name(),
                        sprite
                    ),
                );
            });
        });
    }

    pub fn luck(cn: usize, co: usize, value: i32) {
        if !Character::is_sane_character(cn) || !Character::is_sane_character(co) {
            return;
        }

        let value = value.clamp(-127, 127);

        Repository::with_characters_mut(|characters| {
            let target = &mut characters[co];
            target.luck = value;
            target.set_do_update_flags();

            State::with(|state| {
                state.do_character_log(
                    cn,
                    core::types::FontColor::Green,
                    &format!("Set luck of character {} to {}", target.get_name(), value),
                );
            });
        });
    }

    pub fn reset_description(cn: usize, co: usize) {
        if !Character::is_sane_character(cn) || !Character::is_sane_character(co) {
            return;
        }

        Repository::with_characters_mut(|characters| {
            let target = &mut characters[co];

            // Reset to default description
            let default_desc = format!(
                "{} is a character. They look somewhat nondescript.",
                target.get_name()
            );
            target.description = default_desc
                .bytes()
                .take(200)
                .collect::<Vec<u8>>()
                .try_into()
                .unwrap_or([0; 200]);
            target.set_do_update_flags();

            State::with(|state| {
                state.do_character_log(
                    cn,
                    core::types::FontColor::Green,
                    &format!("Reset description for character {}", target.get_name()),
                );
            });
        });
    }

    pub fn set_name(cn: usize, co: usize, name: &str) {
        if !Character::is_sane_character(cn) || !Character::is_sane_character(co) {
            return;
        }

        if name.len() > 16 || name.is_empty() {
            State::with(|state| {
                state.do_character_log(
                    cn,
                    core::types::FontColor::Red,
                    &format!("Invalid name length: {}", name.len()),
                );
            });
            return;
        }

        Repository::with_characters_mut(|characters| {
            let target = &mut characters[co];
            let old_name = target.get_name().to_string();
            target.name = name
                .bytes()
                .take(40)
                .collect::<Vec<u8>>()
                .try_into()
                .unwrap_or([0; 40]);
            target.set_do_update_flags();

            State::with(|state| {
                state.do_character_log(
                    cn,
                    core::types::FontColor::Green,
                    &format!("Changed name of character from {} to {}", old_name, name),
                );
            });
        });
    }

    pub fn usurp(cn: usize, co: usize) {
        // Check co == 0
        if co == 0 {
            State::with(|state| {
                state.do_character_log(cn, core::types::FontColor::Red, "No such character.");
            });
            return;
        }

        // Check if character is sane
        if !Character::is_sane_character(co) {
            State::with(|state| {
                state.do_character_log(
                    cn,
                    core::types::FontColor::Red,
                    &format!("Bad character number: {}", co),
                );
            });
            return;
        }

        // Check if character is used and is an NPC (not a player)
        let (is_used, is_player_or_usurp, character_name, co_temp) =
            Repository::with_characters(|characters| {
                let character = &characters[co];
                let is_used = character.used != core::constants::USE_EMPTY;
                let is_player_or_usurp = (character.flags
                    & (core::constants::CharacterFlags::CF_PLAYER.bits()
                        | core::constants::CharacterFlags::CF_USURP.bits()))
                    != 0;
                let name = character.name.clone();
                let temp = character.temp;
                (is_used, is_player_or_usurp, name, temp)
            });

        if !is_used || is_player_or_usurp {
            State::with(|state| {
                state.do_character_log(
                    cn,
                    core::types::FontColor::Red,
                    &format!("Character {} is not an NPC.", co),
                );
            });
            return;
        }

        let name_str = String::from_utf8_lossy(&character_name)
            .trim_end_matches('\0')
            .to_string();

        // TODO: chlog(cn, "IMP: Usurping %s (%d, t=%d)", ch[co].name, co, ch[co].temp);

        // Get player number from cn
        let nr = Repository::with_characters(|characters| characters[cn].player);

        Repository::with_characters_mut(|characters| {
            // Set CF_USURP flag on target
            characters[co].flags |= core::constants::CharacterFlags::CF_USURP.bits();

            // Set player number on target
            characters[co].player = nr;

            // TODO: player[nr].usnr = co; (when player array is implemented)

            // Handle nested usurp: if cn is already usurping someone
            if characters[cn].flags & core::constants::CharacterFlags::CF_USURP.bits() != 0 {
                // Transfer the original character reference
                characters[co].data[97] = characters[cn].data[97];
                characters[cn].data[97] = 0;
            } else {
                // Save original character (cn) in co's data[97]
                characters[co].data[97] = cn as i32;
                // Set CCP flag on original character
                characters[cn].flags |= core::constants::CharacterFlags::CF_CCP.bits();
            }

            // If cn is a player, save position and transfer
            if characters[cn].flags & core::constants::CharacterFlags::CF_PLAYER.bits() != 0 {
                // Save tavern position
                characters[cn].tavern_x = characters[cn].x as u16;
                characters[cn].tavern_y = characters[cn].y as u16;

                // Transfer character to (10, 10)
                let cn_x = characters[cn].x;
                let cn_y = characters[cn].y;
                // TODO: god_transfer_char(cn, 10, 10) when implemented
                // For now, just set the position
                characters[cn].x = 10;
                characters[cn].y = 10;

                // Set AFK if not already AFK
                if characters[cn].data[core::constants::CHD_AFK] == 0 {
                    // TODO: do_afk(cn, NULL) when implemented
                }
            }

            // TODO: plr_logout(cn, nr, LO_USURP) when implemented

            characters[co].set_do_update_flags();
        });
    }

    pub fn exit_usurp(cn: usize) {
        if !Character::is_sane_character(cn) {
            return;
        }

        Repository::with_characters_mut(|characters| {
            // Clear usurp-related flags from cn
            characters[cn].flags &= !(core::constants::CharacterFlags::CF_USURP.bits()
                | core::constants::CharacterFlags::CF_STAFF.bits()
                | core::constants::CharacterFlags::CF_IMMORTAL.bits()
                | core::constants::CharacterFlags::CF_GOD.bits()
                | core::constants::CharacterFlags::CF_CREATOR.bits());

            // Get original character from data[97]
            let co = characters[cn].data[97] as usize;

            // Clear CCP flag from original character
            if Character::is_sane_character(co) {
                characters[co].flags &= !core::constants::CharacterFlags::CF_CCP.bits();

                // Get player number
                let nr = characters[cn].player;

                // Transfer player back to original character
                characters[co].player = nr;

                // TODO: player[nr].usnr = co; (when player array is implemented)

                // Transfer character back to recall position (512, 512)
                // TODO: god_transfer_char(co, 512, 512) when implemented
                characters[co].x = 512;
                characters[co].y = 512;

                // TODO: do_afk(co, NULL) when implemented

                characters[cn].set_do_update_flags();
            }
        });
    }

    pub fn grolm(cn: usize) {
        if !Character::is_sane_character(cn) {
            return;
        }

        // Create character from template 386 with items
        let co = crate::populate::pop_create_char(386, true);

        if co != 0 {
            let character_name =
                Repository::with_characters(|characters| characters[co].name.clone());

            let name_str = String::from_utf8_lossy(&character_name)
                .trim_end_matches('\0')
                .to_string();

            // TODO: chlog(cn, "IMP: is now playing %s (%d)", ch[co].name, co);

            Self::usurp(cn, co);
        }
    }

    pub fn grolm_info(cn: usize) {
        if !Character::is_sane_character(cn) {
            return;
        }

        // Find character with template 498 (Grolmy)
        let co = Repository::with_characters(|characters| {
            for co in 1..core::constants::MAXCHARS {
                if characters[co].temp == 498 {
                    return co;
                }
            }
            core::constants::MAXCHARS
        });

        // Check if found, active, and not a corpse
        let (is_valid, data_22, data_40, data_23) = Repository::with_characters(|characters| {
            if co == core::constants::MAXCHARS {
                return (false, 0, 0, 0);
            }
            let character = &characters[co];
            let is_valid = character.used == core::constants::USE_ACTIVE
                && (character.flags & core::constants::CharacterFlags::CF_BODY.bits()) == 0;
            (
                is_valid,
                character.data[22],
                character.data[40],
                character.data[23],
            )
        });

        if !is_valid || co == core::constants::MAXCHARS {
            State::with(|state| {
                state.do_character_log(cn, core::types::FontColor::Yellow, "Grolmy is dead.");
            });
            return;
        }

        // Display state info
        let state_name = match data_22 {
            0 => "at_home",
            1 => "moving_out",
            2 => "moving_in",
            _ => "unknown",
        };

        let ticker = Repository::with_globals(|globals| globals.ticker);
        let timer_minutes = (ticker - data_23) as f64 / (core::constants::TICKS as f64 * 60.0);

        State::with(|state| {
            state.do_character_log(
                cn,
                core::types::FontColor::Green,
                &format!(
                    "Current state={}, runs={}, timer={:.2}m, id={}.",
                    state_name, data_40, timer_minutes, co
                ),
            );
        });
    }

    pub fn grolm_start(cn: usize) {
        if !Character::is_sane_character(cn) {
            return;
        }

        // Find character with template 498 (Grolmy)
        let co = Repository::with_characters(|characters| {
            for co in 1..core::constants::MAXCHARS {
                if characters[co].temp == 498 {
                    return co;
                }
            }
            core::constants::MAXCHARS
        });

        // Check if found, active, and not a corpse
        let (is_valid, data_22) = Repository::with_characters(|characters| {
            if co == core::constants::MAXCHARS {
                return (false, 0);
            }
            let character = &characters[co];
            let is_valid = character.used == core::constants::USE_ACTIVE
                && (character.flags & core::constants::CharacterFlags::CF_BODY.bits()) == 0;
            (is_valid, character.data[22])
        });

        if !is_valid || co == core::constants::MAXCHARS {
            State::with(|state| {
                state.do_character_log(cn, core::types::FontColor::Yellow, "Grolmy is dead.");
            });
            return;
        }

        // Check if already moving
        if data_22 != 0 {
            State::with(|state| {
                state.do_character_log(
                    cn,
                    core::types::FontColor::Yellow,
                    "Grolmy is already moving.",
                );
            });
            return;
        }

        // Start movement
        Repository::with_characters_mut(|characters| {
            characters[co].data[22] = 1;
        });
    }

    pub fn gargoyle(cn: usize) {
        if !Character::is_sane_character(cn) {
            return;
        }

        // Create character from template 495 with items
        let co = crate::populate::pop_create_char(495, true);

        if co != 0 {
            let character_name =
                Repository::with_characters(|characters| characters[co].name.clone());

            let name_str = String::from_utf8_lossy(&character_name)
                .trim_end_matches('\0')
                .to_string();

            // TODO: chlog(cn, "is now playing %s (%d)", ch[co].name, co);

            Self::usurp(cn, co);
        }
    }

    pub fn minor_racechange(cn: usize, temp: i32) {
        if !Character::is_sane_character(cn) {
            return;
        }

        if temp < 0 || temp >= core::constants::MAXTCHARS as i32 {
            log::error!("Invalid character template: {}", temp);
            return;
        }

        Repository::with_character_templates(|templates| {
            let template = &templates[temp as usize];

            if template.used == core::constants::USE_EMPTY {
                log::error!("Template {} is not in use", temp);

                return;
            }

            let template_name = String::from_utf8_lossy(&template.name)
                .trim_end_matches('\0')
                .to_string();

            Repository::with_characters_mut(|characters| {
                let character = &mut characters[cn];

                // Log the change
                State::with(|state| {
                    state.do_character_log(
                        cn,
                        core::types::FontColor::Green,
                        &format!("Changed into {}", template_name),
                    );
                });

                // Set HP, END, MANA from template
                character.hp[1] = template.hp[1];
                character.hp[2] = template.hp[2];
                character.hp[3] = template.hp[3];
                character.end[1] = template.end[1];
                character.end[2] = template.end[2];
                character.end[3] = template.end[3];
                character.mana[1] = template.mana[1];
                character.mana[2] = template.mana[2];
                character.mana[3] = template.mana[3];

                // Set sprite
                character.sprite = template.sprite;

                // Set kindred, preserving KIN_PURPLE
                if character.kindred & (core::constants::KIN_PURPLE as i32) != 0 {
                    character.kindred = template.kindred | (core::constants::KIN_PURPLE as i32);
                } else {
                    character.kindred = template.kindred;
                }

                // Set temp
                character.temp = temp as u16;

                // Set bonuses
                character.weapon_bonus = template.weapon_bonus;
                character.armor_bonus = template.armor_bonus;
                character.gethit_bonus = template.gethit_bonus;

                // Copy attributes
                for n in 0..5 {
                    character.attrib[n][1] = template.attrib[n][1];
                    character.attrib[n][2] = template.attrib[n][2];
                    character.attrib[n][3] = template.attrib[n][3];
                }

                // Copy skills
                for n in 0..50 {
                    if character.skill[n][0] == 0 && template.skill[n][0] != 0 {
                        character.skill[n][0] = template.skill[n][0];
                        // Log added skill
                        log::info!("added skill {} to {}", n, character.get_name());
                    }
                    character.skill[n][1] = template.skill[n][1];
                    character.skill[n][2] = template.skill[n][2];
                    character.skill[n][3] = template.skill[n][3];
                }

                // Reset level
                character.data[45] = 0;

                character.set_do_update_flags();
            });

            // Check for new level
            State::with(|state| {
                state.do_check_new_level(cn);
            });
        });
    }

    pub fn force(cn: usize, whom: &str, text: &str) {
        // Check cn <= 0
        if cn == 0 {
            return;
        }

        // Check if whom is empty
        if whom.is_empty() {
            State::with(|state| {
                state.do_character_log(cn, core::types::FontColor::Red, "#FORCE whom?");
            });
            return;
        }

        // Find the character
        let co = Self::find_next_char(1, whom, "");

        if co <= 0 {
            State::with(|state| {
                state.do_character_log(cn, core::types::FontColor::Red, "No such character.");
            });
            return;
        }

        let co_usize = co as usize;

        // Check if character is used
        let (is_used, is_player, character_name) = Repository::with_characters(|characters| {
            let is_used = characters[co_usize].used == core::constants::USE_ACTIVE;
            let is_player =
                characters[co_usize].flags & core::constants::CharacterFlags::CF_PLAYER.bits() != 0;
            let name = characters[co_usize].name.clone();
            (is_used, is_player, name)
        });

        if !is_used {
            State::with(|state| {
                state.do_character_log(cn, core::types::FontColor::Red, "Character is not active.");
            });
            return;
        }

        // Check if trying to force a player when not a god
        let is_cn_god = Repository::with_characters(|characters| {
            characters[cn].flags & core::constants::CharacterFlags::CF_GOD.bits() != 0
        });

        if is_player && !is_cn_god {
            State::with(|state| {
                state.do_character_log(
                    cn,
                    core::types::FontColor::Red,
                    "Not allowed to #FORCE players.",
                );
            });
            return;
        }

        // Check if text is empty
        if text.is_empty() {
            let name_str = String::from_utf8_lossy(&character_name)
                .trim_end_matches('\0')
                .to_string();

            State::with(|state| {
                state.do_character_log(
                    cn,
                    core::types::FontColor::Red,
                    &format!("#FORCE {} to what?", name_str),
                );
            });
            return;
        }

        let name_str = String::from_utf8_lossy(&character_name)
            .trim_end_matches('\0')
            .to_string();

        // TODO: chlog(cn, "IMP: Forced %s (%d) to \"%s\"", ch[co].name, co, text);

        // Make the character say the text
        State::with(|state| {
            state.do_sayx(co_usize, text);
        });

        // Show success message
        State::with(|state| {
            state.do_character_log(
                cn,
                core::types::FontColor::Green,
                &format!("{} was forced.", name_str),
            );
        });
    }

    pub fn enemy(cn: usize, npc: &str, victim: &str) {
        if !Character::is_sane_character(cn) {
            return;
        }

        let npc_cn = Self::find_next_char(1, npc, "");
        let victim_cn = Self::find_next_char(1, victim, "");

        if npc_cn <= 0 {
            State::with(|state| {
                state.do_character_log(
                    cn,
                    core::types::FontColor::Red,
                    &format!("Could not find NPC '{}'", npc),
                );
            });
            return;
        }

        if victim_cn <= 0 {
            State::with(|state| {
                state.do_character_log(
                    cn,
                    core::types::FontColor::Red,
                    &format!("Could not find victim '{}'", victim),
                );
            });
            return;
        }

        Repository::with_characters_mut(|characters| {
            let npc_char = &mut characters[npc_cn as usize];
            npc_char.attack_cn = victim_cn as u16;

            State::with(|state| {
                state.do_character_log(
                    cn,
                    core::types::FontColor::Green,
                    &format!("Set NPC {} to attack character {}", npc_cn, victim_cn),
                );
            });
        });
    }

    pub fn is_banned(addr: i32) -> bool {
        let addr = addr as u32;

        Repository::with_ban_list(|ban_list| {
            for ban in ban_list.iter() {
                if ban.address() == addr {
                    return true;
                }
            }
            false
        })
    }

    pub fn add_single_ban(cn: usize, co: usize, addr: u32) {
        if !Character::is_sane_character(cn) || !Character::is_sane_character(co) {
            return;
        }

        let (creator_name, victim_name) = Repository::with_characters(|characters| {
            (
                characters[cn].get_name().to_string(),
                characters[co].get_name().to_string(),
            )
        });

        Repository::with_ban_list_mut(|ban_list| {
            if ban_list.len() >= 250 {
                State::with(|state| {
                    state.do_character_log(cn, core::types::FontColor::Red, "Ban list is full");
                });
                return;
            }

            let mut ban = core::types::Ban::new();
            ban.set_address(addr);
            ban.set_creator(&creator_name);
            ban.set_victim(&victim_name);

            ban_list.push(ban);

            State::with(|state| {
                state.do_character_log(
                    cn,
                    core::types::FontColor::Green,
                    &format!("Added ban for address {} by {}", addr, creator_name),
                );
            });
        });
    }

    pub fn add_ban(cn: usize, co: usize) {
        if !Character::is_sane_character(cn) || !Character::is_sane_character(co) {
            return;
        }

        // Get player address - would need actual connection info
        // For now using placeholder logic
        let addr = 0u32; // TODO: Get actual player IP address

        Self::add_single_ban(cn, co, addr);
    }

    pub fn del_ban(cn: usize, nr: usize) {
        if !Character::is_sane_character(cn) {
            return;
        }

        Repository::with_ban_list_mut(|ban_list| {
            if nr >= ban_list.len() {
                State::with(|state| {
                    state.do_character_log(
                        cn,
                        core::types::FontColor::Red,
                        &format!("Invalid ban number: {}", nr),
                    );
                });
                return;
            }

            ban_list.remove(nr);

            State::with(|state| {
                state.do_character_log(
                    cn,
                    core::types::FontColor::Green,
                    &format!("Removed ban entry {}", nr),
                );
            });
        });
    }

    pub fn list_ban(cn: usize) {
        if !Character::is_sane_character(cn) {
            return;
        }

        Repository::with_ban_list(|ban_list| {
            State::with(|state| {
                state.do_character_log(
                    cn,
                    core::types::FontColor::Green,
                    &format!("Ban list ({} entries):", ban_list.len()),
                );
                for (i, ban) in ban_list.iter().enumerate() {
                    state.do_character_log(
                        cn,
                        core::types::FontColor::Green,
                        &format!(
                            "  {}: Address={}, Creator={}, Victim={}",
                            i,
                            ban.address(),
                            ban.creator(),
                            ban.victim()
                        ),
                    );
                }
            });
        });
    }

    pub fn shutup(cn: usize, co: usize) {
        if !Character::is_sane_character(cn) || !Character::is_sane_character(co) {
            return;
        }

        Repository::with_characters_mut(|characters| {
            let target = &mut characters[co];

            // Toggle shutup flag
            if target.flags & core::constants::CharacterFlags::CF_SHUTUP.bits() != 0 {
                target.flags &= !core::constants::CharacterFlags::CF_SHUTUP.bits();
                State::with(|state| {
                    state.do_character_log(
                        cn,
                        core::types::FontColor::Green,
                        &format!("Removed shutup from character {}", target.get_name()),
                    );
                });
            } else {
                target.flags |= core::constants::CharacterFlags::CF_SHUTUP.bits();
                State::with(|state| {
                    state.do_character_log(
                        cn,
                        core::types::FontColor::Green,
                        &format!("Added shutup to character {}", target.get_name()),
                    );
                });
            }

            target.set_do_update_flags();
        });
    }
}
