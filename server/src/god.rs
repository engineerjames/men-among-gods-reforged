use core::{
    constants::{
        character_flags_name, ArmorType, CharacterFlags, MagicArmorType, DX_DOWN, DX_LEFT,
        DX_LEFTDOWN, DX_LEFTUP, DX_RIGHT, DX_RIGHTDOWN, DX_RIGHTUP, DX_UP,
    },
    ranks,
    string_operations::c_string_to_str,
    traits,
    types::{Character, Map},
};

use crate::{
    area, chlog, driver, effect::EffectManager, enums::LogoutReason, game_state::GameState,
    helpers, player, populate,
};

pub struct God {}
impl God {
    /// Drop a character near the target using an explicit game-state borrow.
    pub fn drop_char_fuzzy_large(
        gs: &mut GameState,
        character_id: usize,
        x: usize,
        y: usize,
        backup_x: usize,
        backup_y: usize,
    ) -> bool {
        let positions_to_try: [(usize, usize); 25] = [
            (x, y),
            (x + 1, y),
            (x - 1, y),
            (x, y + 1),
            (x, y - 1),
            (x + 1, y + 1),
            (x + 1, y - 1),
            (x - 1, y + 1),
            (x - 1, y - 1),
            (x + 2, y - 2),
            (x + 2, y - 1),
            (x + 2, y),
            (x + 2, y + 1),
            (x + 2, y + 2),
            (x - 2, y - 2),
            (x - 2, y - 1),
            (x - 2, y),
            (x - 2, y + 1),
            (x - 2, y + 2),
            (x - 1, y + 2),
            (x, y + 2),
            (x + 1, y + 2),
            (x - 1, y - 2),
            (x, y - 2),
            (x + 1, y - 2),
        ];

        for (try_x, try_y) in positions_to_try.iter() {
            let early_return = gs.can_go(
                backup_x as i32,
                backup_y as i32,
                *try_x as i32,
                *try_y as i32,
            ) != 0
                && Self::drop_char(gs, character_id, *try_x, *try_y);

            if early_return {
                return true;
            }
        }

        false
    }

    /// Create an item using an explicit game-state borrow.
    pub(crate) fn create_item(gs: &mut GameState, template_id: usize) -> Option<usize> {
        if !core::types::Item::is_sane_item_template(template_id) {
            return None;
        }

        if gs.item_templates[template_id].used == core::constants::USE_EMPTY {
            log::error!(
                "Attempted to create item with an unused template ID: {}",
                template_id
            );
            return None;
        }

        if gs.item_templates[template_id].is_unique() {
            // Check if the unique item already exists
            for item in gs.items.iter() {
                if item.used != core::constants::USE_EMPTY && item.temp as usize == template_id {
                    log::error!(
                        "Attempted to create unique item with template ID {} but it already exists.",
                        template_id
                    );
                    return None;
                }
            }
        }

        let free_item_id = Self::get_free_item_slot(gs).unwrap_or_else(|| {
            log::error!("No free item slots available to create new item.");
            0
        });

        let template_copy = gs.item_templates[template_id];
        gs.items[free_item_id] = template_copy;
        gs.items[free_item_id].temp = template_id as u16;

        Some(free_item_id)
    }

    // TODO: Optimize this later
    /// Find a free item slot in the global item array.
    ///
    /// Returns `Some(index)` when a free slot is found, otherwise `None`.
    fn get_free_item_slot(gs: &mut GameState) -> Option<usize> {
        for item_id in 1..core::constants::MAXITEM {
            if gs.items[item_id].used != core::constants::USE_EMPTY {
                continue;
            }

            // Safety net: if an item was destroyed but left referenced by a character,
            // reusing this slot would make the client briefly show it as a different
            // template before server-side validity checks clear it.
            let carried = gs.items[item_id].carried as usize;
            if carried != 0 {
                if Character::is_sane_character(carried) {
                    let ch = &mut gs.characters[carried];
                    if ch.citem as usize == item_id {
                        ch.citem = 0;
                    }
                    for slot in 0..40 {
                        if ch.item[slot] as usize == item_id {
                            ch.item[slot] = 0;
                        }
                    }
                    for slot in 0..20 {
                        if ch.worn[slot] as usize == item_id {
                            ch.worn[slot] = 0;
                        }
                        if ch.spell[slot] as usize == item_id {
                            ch.spell[slot] = 0;
                        }
                    }
                    for slot in 0..62 {
                        if ch.depot[slot] as usize == item_id {
                            ch.depot[slot] = 0;
                        }
                    }
                    ch.set_do_update_flags();
                }

                gs.items[item_id].carried = 0;
                gs.items[item_id].x = 0;
                gs.items[item_id].y = 0;
            }

            return Some(item_id);
        }
        None
    }

    // Implementation of god_give_char from svr_god.cpp

    /// Give an item to a character using an explicit game-state borrow.
    pub fn give_character_item(gs: &mut GameState, character_id: usize, item_id: usize) -> bool {
        if !core::types::Item::is_sane_item(item_id) {
            log::error!("Invalid item ID {} when giving item.", item_id);
            return false;
        }

        if !gs.characters[character_id].is_living_character(character_id) {
            log::error!("Invalid character ID {} when giving item.", character_id);
            return false;
        }

        let (old_x, old_y, old_carried, old_active, old_light_inactive, old_light_active) = {
            let item = &gs.items[item_id];
            (
                item.x,
                item.y,
                item.carried,
                item.active,
                item.light[0],
                item.light[1],
            )
        };

        // If the item is currently on the ground, ensure the map no longer references it
        // before we move it into inventory. Otherwise, the item GC will later notice the
        // map->item mismatch and clear the tile, which can produce visible sprite glitches.
        if old_carried == 0 && Map::is_sane_coordinates(old_x as usize, old_y as usize) {
            let map_index =
                (old_x as usize) + (old_y as usize) * core::constants::SERVER_MAPX as usize;

            let map_it = gs.map[map_index].it;
            if map_it == item_id as u32 {
                let light_value = if old_active != 0 {
                    old_light_active
                } else {
                    old_light_inactive
                };

                if light_value != 0 {
                    gs.do_add_light(old_x as i32, old_y as i32, -(light_value as i32));
                }

                gs.map[map_index].it = 0;
            }
        }

        let item_name = gs.items[item_id].get_name().to_string();
        let char_name = gs.characters[character_id].get_name().to_string();
        log::debug!(
            "Attempting to give item '{}' to character '{}'",
            item_name,
            char_name,
        );

        let slot = gs.characters[character_id].get_next_inventory_slot();
        if let Some(slot) = slot {
            gs.characters[character_id].item[slot] = item_id as u32;
            gs.items[item_id].x = 0;
            gs.items[item_id].y = 0;
            gs.items[item_id].carried = character_id as u16;
            gs.characters[character_id].set_do_update_flags();
            true
        } else {
            log::error!(
                "No free inventory slots available for character '{}' (ID {}).",
                char_name,
                character_id
            );
            false
        }
    }

    /// Manage build mode for a character.
    ///
    /// Starts, stops, or equips build-mode resources depending on
    /// `build_type` and the character's current build state.
    ///
    /// # Arguments
    /// * `character_id` - Character index
    /// * `build_type` - Build action selector
    pub fn build(gs: &mut GameState, character_id: usize, build_type: u32) {
        let character_is_building = gs.characters[character_id].is_building();
        let name = gs.characters[character_id].get_name().to_string();
        if !character_is_building {
            if Self::build_start(gs, character_id) {
                Self::build_equip(gs, character_id, build_type);
            } else {
                log::error!("Failed to start build mode for character {}", name);
            }
        } else if build_type != 0 {
            Self::build_stop(gs, character_id);
        } else {
            Self::build_equip(gs, character_id, build_type);
        }
    }

    /// Equip builder-only items and map flags for a builder character.
    ///
    /// Populates the character's temporary item slots with map flags and
    /// sprite ids used while in build mode.
    ///
    /// # Arguments
    /// * `character_id` - Character index
    /// * `build_type` - Equipment variant
    fn build_equip(gs: &mut GameState, character_id: usize, build_type: u32) {
        let mut m = 0;
        let char_name = {
            let character = &mut gs.characters[character_id];
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
            for n in build_type as usize..core::constants::MAXTITEM {
                if m >= 40 {
                    break;
                }

                if gs.item_templates[n].used == core::constants::USE_EMPTY {
                    continue;
                }

                if gs.item_templates[n].flags & core::constants::ItemFlags::IF_TAKE.bits() != 0 {
                    continue;
                }

                if gs.item_templates[n].driver == 25 && gs.item_templates[n].data[3] == 0 {
                    continue;
                }

                if gs.item_templates[n].driver == 22 {
                    continue;
                }

                character.item[m] = n as u32;
                m += 1;
            }

            character.get_name().to_string()
        };

        log::info!("Build mode {} set for character {}", build_type, char_name);

        gs.do_character_log(
            character_id,
            core::types::FontColor::Blue,
            "You are now in build mode. To exit, use the build command again.\n",
        );
    }

    /// Start build mode for a character, creating helper companion state.
    ///
    /// Allocates a temporary helper character to hold items and prepares the
    /// player to place map objects. Returns `true` on success.
    ///
    /// # Arguments
    /// * `character_id` - Character entering build mode
    fn build_start(gs: &mut GameState, character_id: usize) -> bool {
        let companion = {
            let character = &gs.characters[character_id];
            if character.data[core::constants::CHD_COMPANION] != 0 {
                Some(character.data[core::constants::CHD_COMPANION] as usize)
            } else {
                None
            }
        };

        if let Some(companion_id) = companion {
            let companion_name = gs.characters[companion_id].get_name().to_string();
            gs.do_character_log(
                character_id,
                core::types::FontColor::Red,
                &format!("Get rid of your companion '{}' first.\n", companion_name),
            );

            return false;
        }

        let character_id_to_hold_inventory = Self::create_char(gs, 1, false);

        if character_id_to_hold_inventory.is_none() {
            gs.do_character_log(
                character_id,
                core::types::FontColor::Red,
                "Failed to create temporary character to hold your items for build mode.\n",
            );
            log::error!(
                "Failed to create temporary character to hold items for build mode for character ID {}",
                character_id
            );
            return false;
        }

        let holder_id = character_id_to_hold_inventory.unwrap() as usize;
        for i in 0..40 {
            let item_id = gs.characters[character_id].item[i] as usize;
            if item_id != 0 {
                gs.characters[character_id].item[i] = 0;
                gs.characters[holder_id].item[i] = item_id as u32;
                gs.items[item_id].carried = holder_id as u16;
            }
        }
        let citem = gs.characters[character_id].citem;
        gs.characters[holder_id].citem = citem;
        gs.characters[character_id].citem = 0;

        // TODO: This function looks very ugly... refactor later
        let holder_name = {
            let char_name = gs.characters[character_id].get_name().to_string();
            format!("{}'s holder", char_name)
                .bytes()
                .take(40)
                .collect::<Vec<u8>>()
                .try_into()
                .unwrap_or([0; 40])
        };
        gs.characters[holder_id].name = holder_name;

        Self::drop_char(gs, holder_id, 10, 10);

        gs.characters[character_id].flags |= CharacterFlags::BuildMode.bits();
        gs.characters[character_id].set_do_update_flags();
        true
    }

    /// Stop build mode and restore player's inventory from the helper.
    ///
    /// Transfers items back to the player and cleans up temporary state.
    ///
    /// # Arguments
    /// * `character_id` - Character exiting build mode
    fn build_stop(gs: &mut GameState, character_id: usize) {
        if !core::types::Character::is_sane_character(character_id) {
            log::error!("Invalid character ID {} in build_stop", character_id);
            return;
        }

        // Empty builder's inventory
        {
            let character = &mut gs.characters[character_id];
            for n in 0..40 {
                character.item[n] = 0;
            }
            character.citem = 0;
            character.flags &= !CharacterFlags::BuildMode.bits();
            character.misc_action = 0; // DR_IDLE
            let char_name = character.get_name().to_string();
            log::info!("Character {} now out of build mode", char_name);
        }
        gs.do_character_log(
            character_id,
            core::types::FontColor::Blue,
            "You are now out of build mode.\n",
        );

        // Retrieve inventory from item holder
        let companion_id =
            gs.characters[character_id].data[core::constants::CHD_COMPANION] as usize;

        if companion_id == 0 {
            log::error!(
                "Could not find item holder for character {} when stopping build mode",
                character_id
            );

            gs.do_character_log(
                character_id,
                core::types::FontColor::Red,
                "Could not find your item holder!\n",
            );
            return;
        }

        let mut items_to_transfer = Vec::new();
        let companion_citem;
        {
            let companion = &mut gs.characters[companion_id];
            for n in 0..40 {
                items_to_transfer.push((n, companion.item[n]));
                companion.item[n] = 0;
            }
            companion_citem = companion.citem;
            companion.citem = 0;
        }
        player::plr_map_remove(gs, companion_id);
        gs.characters[companion_id].used = core::constants::USE_EMPTY;
        gs.characters[character_id].data[core::constants::CHD_COMPANION] = 0;

        // Transfer inventory from companion to builder
        for (n, item_id) in items_to_transfer {
            if item_id != 0 {
                gs.characters[character_id].item[n] = item_id;
                if core::types::Item::is_sane_item(item_id as usize) {
                    gs.items[item_id as usize].carried = character_id as u16;
                }
            }
        }

        // Transfer citem from companion to builder
        gs.characters[character_id].citem = companion_citem;
        if companion_citem != 0 {
            if core::types::Item::is_sane_item(companion_citem as usize) {
                gs.items[companion_citem as usize].carried = character_id as u16;
            }
        }
        gs.characters[character_id].set_do_update_flags();
    }

    /// Transfer a character using an explicit game-state borrow.
    pub fn transfer_char(gs: &mut GameState, character_id: usize, x: usize, y: usize) -> bool {
        if !Character::is_sane_character(character_id) || !Map::is_sane_coordinates(x, y) {
            log::error!(
                "Invalid character ID {} or coordinates ({}, {}) in transfer_char",
                character_id,
                x,
                y
            );
            return false;
        }

        let character = &mut gs.characters[character_id];
        character.status = 0;
        character.attack_cn = 0;
        character.skill_nr = 0;
        character.goto_x = x as u16;
        character.goto_y = y as u16; // TODO: This was missing before... should this be here?

        let positions_to_try: [(usize, usize); 5] =
            [(x, y), (x + 3, y), (x, y + 3), (x - 3, y), (x, y - 3)];

        for (try_x, try_y) in positions_to_try.iter() {
            if Self::drop_char_fuzzy_large(gs, character_id, *try_x, *try_y, x, y) {
                return true;
            }
        }

        false
    }

    /// Place a character near a tile using an explicit game-state borrow.
    pub fn drop_char_fuzzy(gs: &mut GameState, character_id: usize, x: usize, y: usize) -> bool {
        let positions_to_try: [(usize, usize); 25] = [
            (x, y),
            (x + 1, y),
            (x - 1, y),
            (x, y + 1),
            (x, y - 1),
            (x + 1, y + 1),
            (x + 1, y - 1),
            (x - 1, y + 1),
            (x - 1, y - 1),
            (x + 2, y - 2),
            (x + 2, y - 1),
            (x + 2, y),
            (x + 2, y + 1),
            (x + 2, y + 2),
            (x - 2, y - 2),
            (x - 2, y - 1),
            (x - 2, y),
            (x - 2, y + 1),
            (x - 2, y + 2),
            (x - 1, y + 2),
            (x, y + 2),
            (x + 1, y + 2),
            (x - 1, y - 2),
            (x, y - 2),
            (x + 1, y - 2),
        ];

        for (try_x, try_y) in positions_to_try.iter() {
            let early_return =
                gs.can_go(*try_x as i32, *try_y as i32, *try_x as i32, *try_y as i32) != 0
                    && Self::drop_char(gs, character_id, *try_x, *try_y);

            if early_return {
                return true;
            }
        }

        false
    }

    /// Create a character using an explicit game-state borrow.
    pub fn create_char(gs: &mut GameState, template_id: usize, with_items: bool) -> Option<i32> {
        let unused_index = (1..core::constants::MAXCHARS)
            .find(|&i| gs.characters[i].used == core::constants::USE_EMPTY);

        let char_index = match unused_index {
            Some(index) => index,
            None => {
                log::error!("No free character slots available to create new character.");
                return None;
            }
        };

        // Copy template into character slot
        let template_copy = gs.character_templates[template_id];
        gs.characters[char_index] = template_copy;

        // Templates can carry runtime fields like `player`; never inherit a player binding.
        gs.characters[char_index].player = 0;
        gs.characters[char_index].pass1 = crate::helpers::random_mod(0x3fffffff);
        gs.characters[char_index].pass2 = crate::helpers::random_mod(0x3fffffff);
        gs.characters[char_index].temp = template_id as u16;

        loop {
            log::info!("Generating random name for new character...");
            let potential_new_name = core::names::randomly_generate_name();

            let name_exists = gs.characters.iter().any(|existing_char| {
                existing_char.used != core::constants::USE_EMPTY
                    && existing_char
                        .get_name()
                        .eq_ignore_ascii_case(&potential_new_name)
            });
            if !name_exists {
                let mut name_arr = [0u8; 40];
                let name_bytes = potential_new_name.as_bytes();
                let copy_len = name_bytes.len().min(40);
                name_arr[..copy_len].copy_from_slice(&name_bytes[..copy_len]);
                gs.characters[char_index].name = name_arr;
                log::info!(
                    "Assigned name '{}' to new character (ID {})",
                    gs.characters[char_index].get_name(),
                    char_index
                );
                break;
            }

            log::info!(
                "Generated name '{}' already exists. Retrying...",
                potential_new_name
            );
        }

        let character = &mut gs.characters[char_index];
        character.reference = character.name;
        character.description = character
            .get_default_description()
            .as_bytes()
            .iter()
            .take(200)
            .copied()
            .collect::<Vec<u8>>()
            .try_into()
            .unwrap_or([0; 200]); // TODO: Is this really the right way to do this?

        for i in 0..100_usize {
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
            let mut tmp = gs.characters[char_index].item[i];
            if tmp == 0 {
                continue;
            }

            if with_items {
                tmp = Self::create_item(gs, tmp as usize).unwrap_or(0) as u32;
                if tmp == 0 {
                    log::error!(
                        "Failed to create item from template new character ID {}",
                        char_index
                    );
                    flag = 1;
                }
                if tmp != 0 && gs.items[tmp as usize].used != core::constants::USE_EMPTY {
                    gs.items[tmp as usize].carried = char_index as u16;
                }
            } else {
                tmp = 0;
            }

            gs.characters[char_index].item[i] = tmp;
        }

        for i in 0..20 {
            let mut tmp_worn = gs.characters[char_index].worn[i];
            if tmp_worn == 0 {
                continue;
            }

            if with_items {
                tmp_worn = Self::create_item(gs, tmp_worn as usize).unwrap_or(0) as u32;
                if tmp_worn == 0 {
                    log::error!(
                        "Failed to create worn item from template for new character ID {}",
                        char_index
                    );
                    flag = 1;
                }
                if core::types::Item::is_sane_item(tmp_worn as usize) {
                    gs.items[tmp_worn as usize].carried = char_index as u16;
                }
            } else {
                tmp_worn = 0;
            }

            gs.characters[char_index].worn[i] = tmp_worn;
        }

        for i in 0..20 {
            if gs.characters[char_index].spell[i] != 0 {
                gs.characters[char_index].spell[i] = 0;
            }
        }

        let mut tmp_citem = gs.characters[char_index].citem;
        if tmp_citem != 0 {
            if with_items {
                tmp_citem = Self::create_item(gs, tmp_citem as usize).unwrap_or(0) as u32;
                if tmp_citem == 0 {
                    log::error!(
                        "Failed to create citem from template for new character ID {}",
                        char_index
                    );
                    flag = 1;
                }
                if core::types::Item::is_sane_item(tmp_citem as usize) {
                    gs.items[tmp_citem as usize].carried = char_index as u16;
                }
            } else {
                tmp_citem = 0;
            }

            gs.characters[char_index].citem = tmp_citem;
        }

        if flag != 0 {
            log::error!(
                "One or more items failed to be created for new character ID {}",
                char_index
            );
            Self::destroy_items(gs, char_index);
            gs.characters[char_index].used = core::constants::USE_EMPTY;
            return None;
        }

        gs.characters[char_index].a_end = 1000000;
        gs.characters[char_index].a_hp = 1000000;
        gs.characters[char_index].a_mana = 1000000;
        gs.characters[char_index].set_do_update_flags();

        Some(char_index as i32)
    }

    /// Destroy a character's items using an explicit game-state borrow.
    pub fn destroy_items(gs: &mut GameState, char_id: usize) {
        if !core::types::Character::is_sane_character(char_id) {
            log::error!("Invalid character ID {} in destroy_items", char_id);
            return;
        }

        // Destroy all inventory items (40 slots)
        for n in 0..40 {
            let item_id = gs.characters[char_id].item[n] as usize;
            if item_id != 0 {
                gs.characters[char_id].item[n] = 0;
                if core::types::Item::is_sane_item(item_id) {
                    gs.items[item_id].used = core::constants::USE_EMPTY;
                }
            }
        }

        // Destroy all worn items (20 slots)
        for n in 0..20 {
            let worn_id = gs.characters[char_id].worn[n] as usize;
            if worn_id != 0 {
                gs.characters[char_id].worn[n] = 0;
                if core::types::Item::is_sane_item(worn_id) {
                    gs.items[worn_id].used = core::constants::USE_EMPTY;
                }
            }

            let spell_id = gs.characters[char_id].spell[n] as usize;
            if spell_id != 0 {
                gs.characters[char_id].spell[n] = 0;
                if core::types::Item::is_sane_item(spell_id) {
                    gs.items[spell_id].used = core::constants::USE_EMPTY;
                }
            }
        }

        // Destroy carried item (citem)
        let citem_id = gs.characters[char_id].citem as usize;
        if citem_id != 0 {
            gs.characters[char_id].citem = 0;
            // TODO: Refactor this check--it is duplicated due to the != 0
            // check above anyway.
            if core::types::Item::is_sane_item(citem_id) {
                gs.items[citem_id].used = core::constants::USE_EMPTY;
            }
        }

        // If player, destroy depot/storage items (62 slots)
        if gs.characters[char_id].is_player() {
            for n in 0..62 {
                let depot_id = gs.characters[char_id].depot[n] as usize;
                if depot_id != 0 {
                    gs.characters[char_id].depot[n] = 0;
                    if core::types::Item::is_sane_item(depot_id) {
                        gs.items[depot_id].used = core::constants::USE_EMPTY;
                    }
                }
            }
        }

        gs.characters[char_id].set_do_update_flags();
    }

    /// Take an item from a character using an explicit game-state borrow.
    pub fn take_from_char(gs: &mut GameState, item_id: usize, cn: usize) -> bool {
        if !core::types::Item::is_sane_item(item_id) {
            return false;
        }

        if !gs.characters[cn].is_living_character(cn) {
            return false;
        }

        // Remove from citem
        if gs.characters[cn].citem as usize == item_id {
            gs.characters[cn].citem = 0;
        } else {
            // Try inventory
            let mut found = false;
            for n in 0..40 {
                if gs.characters[cn].item[n] as usize == item_id {
                    gs.characters[cn].item[n] = 0;
                    found = true;
                    break;
                }
            }
            if !found {
                // Try worn
                for n in 0..20 {
                    if gs.characters[cn].worn[n] as usize == item_id {
                        gs.characters[cn].worn[n] = 0;
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
        gs.items[item_id].x = 0;
        gs.items[item_id].y = 0;
        gs.items[item_id].carried = 0;

        // Mark character for update
        gs.characters[cn].set_do_update_flags();

        // Call update hook in GameState so network/clients can be informed.
        gs.do_update_char(cn);

        true
    }

    /// Drop an item using an explicit game-state borrow.
    pub fn drop_item(gs: &mut GameState, item_id: usize, x: usize, y: usize) -> bool {
        if !Map::is_sane_coordinates(x, y) {
            return false;
        }

        let map_index = x + y * core::constants::SERVER_MAPX as usize;

        let can_drop = !(gs.map[map_index].ch != 0
            || gs.map[map_index].to_ch != 0
            || gs.map[map_index].it != 0
            || (gs.map[map_index].flags
                & (core::constants::MF_MOVEBLOCK | core::constants::MF_DEATHTRAP) as u64)
                != 0
            || gs.map[map_index].fsprite != 0);

        if !can_drop {
            return false;
        }

        // Update the item first so if other systems validate map->item consistency
        // concurrently, they won't observe a map reference to an item with stale coordinates.
        gs.items[item_id].x = x as u16;
        gs.items[item_id].y = y as u16;
        gs.items[item_id].carried = 0;
        let light_value = if gs.items[item_id].active != 0 {
            gs.items[item_id].light[1]
        } else {
            gs.items[item_id].light[0]
        };
        if light_value != 0 {
            gs.do_add_light(x as i32, y as i32, light_value as i32);
        }

        // Write the map reference last.
        gs.map[map_index].it = item_id as u32;

        true
    }

    /// Place a character using an explicit game-state borrow.
    pub(crate) fn drop_char(gs: &mut GameState, character_id: usize, x: usize, y: usize) -> bool {
        if !Map::is_sane_coordinates(x, y) {
            return false;
        }

        let map_index = x + y * core::constants::SERVER_MAPX as usize;

        let item_on_tile = gs.map[map_index].it;
        let move_is_valid = !(gs.map[map_index].ch != 0
            || (item_on_tile != 0
                && gs.items[item_on_tile as usize].flags
                    & core::constants::ItemFlags::IF_MOVEBLOCK.bits()
                    != 0)
            || gs.map[map_index].flags & core::constants::MF_MOVEBLOCK as u64 != 0
            || gs.map[map_index].flags & core::constants::MF_TAVERN as u64 != 0
            || gs.map[map_index].flags & core::constants::MF_DEATHTRAP as u64 != 0);

        if !move_is_valid {
            return false;
        }

        // Remove from previous tile (if any), update coords and insert into map
        player::plr_map_remove(gs, character_id);
        gs.characters[character_id].x = x as i16;
        gs.characters[character_id].y = y as i16;
        gs.characters[character_id].tox = x as i16;
        gs.characters[character_id].toy = y as i16;
        player::plr_map_set(gs, character_id);

        true
    }

    /// Change the password for character `co` as requested by `cn`.
    ///
    /// Validates permission and updates the stored password fields.
    ///
    /// # Arguments
    /// * `cn` - Requesting character
    /// * `co` - Target character
    /// * `pass` - New password string
    pub fn change_pass(gs: &mut GameState, cn: usize, co: usize, pass: &str) -> bool {
        if !Character::is_sane_character(cn) || !Character::is_sane_character(co) {
            return false;
        }

        let pass_hash = pass.as_bytes();
        let pass1 = pass_hash
            .iter()
            .take(4)
            .fold(0u32, |acc, &b| (acc << 8) | b as u32);
        let pass2 = pass_hash
            .iter()
            .skip(4)
            .take(4)
            .fold(0u32, |acc, &b| (acc << 8) | b as u32);
        let char_name = {
            let character = &mut gs.characters[co];
            character.pass1 = pass1;
            character.pass2 = pass2;
            character.set_do_update_flags();
            character.get_name().to_string()
        };
        log::info!("Password changed for character {}", char_name);
        let target_name = gs.characters[co].get_name().to_string();
        gs.do_character_log(
            cn,
            core::types::FontColor::Green,
            &format!(
                "You have changed the password for character '{}'.\n",
                target_name
            ),
        );
        true
    }

    // This function is unused in the original implementation

    /// Remove an item from a character while reusing an existing game-state borrow.
    fn remove_item(gs: &mut GameState, cn: usize, item_id: usize) -> bool {
        if !Character::is_sane_character(cn) || !core::types::Item::is_sane_item(item_id) {
            return false;
        }

        // Check inventory slots
        for n in 0..40 {
            if gs.characters[cn].item[n] == item_id as u32 {
                gs.characters[cn].item[n] = 0;
                gs.items[item_id].carried = 0;
                gs.characters[cn].set_do_update_flags();
                return true;
            }
        }

        // Check worn/wielded slots
        for n in 0..20 {
            if gs.characters[cn].worn[n] == item_id as u32 {
                gs.characters[cn].worn[n] = 0;
                gs.items[item_id].carried = 0;
                gs.characters[cn].set_do_update_flags();
                return true;
            }
        }

        false
    }

    /// Try to drop an item near a tile while reusing an existing game-state borrow.
    fn drop_item_fuzzy(gs: &mut GameState, nr: usize, x: usize, y: usize) -> bool {
        let positions_to_try: [(usize, usize); 25] = [
            (x, y),
            (x + 1, y),
            (x - 1, y),
            (x, y + 1),
            (x, y - 1),
            (x + 1, y + 1),
            (x + 1, y - 1),
            (x - 1, y + 1),
            (x - 1, y - 1),
            (x + 2, y - 2),
            (x + 2, y - 1),
            (x + 2, y),
            (x + 2, y + 1),
            (x + 2, y + 2),
            (x - 2, y - 2),
            (x - 2, y - 1),
            (x - 2, y),
            (x - 2, y + 1),
            (x - 2, y + 2),
            (x - 1, y + 2),
            (x, y + 2),
            (x + 1, y + 2),
            (x - 1, y - 2),
            (x, y - 2),
            (x + 1, y - 2),
        ];

        for (try_x, try_y) in positions_to_try.iter() {
            if Self::drop_item(gs, nr, *try_x, *try_y) {
                return true;
            }
        }

        false
    }

    /// Teleport `co` to coordinates parsed from `cx`/`cy` at the request of `cn`.
    /// `cx` can contain direction modifiers (n/s/e/w) or absolute values.
    ///
    /// Parses the coordinate strings and delegates to transfer logic.
    ///
    /// # Arguments
    /// * `cn` - Requesting character
    /// * `co` - Target character
    /// * `cx`, `cy` - Coordinate strings
    pub fn goto(gs: &mut GameState, cn: usize, co: usize, cx: &str, cy: &str) {
        log::debug!(
            "goto() called by character {} to move character {} to '{},{}'",
            cn,
            co,
            cx,
            cy
        );
        if !Character::is_sane_character(cn) || !Character::is_sane_character(co) {
            return;
        }

        // We expect at least one of the values passed in to be non-empty
        if cx.is_empty() && cy.is_empty() {
            gs.do_character_log(
                cn,
                core::types::FontColor::Red,
                &format!(
                    "Invalid coordinates provided for goto command: '{},{}'.\n",
                    cx, cy
                ),
            );
            return;
        }

        let character_name = gs.characters[co].get_name().to_string();
        let character_visible = gs.characters[co].flags & CharacterFlags::Invisible.bits() == 0
            && gs.characters[co].flags & CharacterFlags::GreaterInv.bits() == 0;
        let target = Self::goto_cardinal_length(gs, cn, cx, cy)
            .or_else(|| Self::goto_target_coordinates(cn, cx, cy))
            .or_else(|| Self::goto_character_by_name(gs, cn, cx, cy));

        if let Some((x, y)) = target {
            let orig_pos = {
                let character = &gs.characters[co];
                (character.x as usize, character.y as usize)
            };

            if character_visible {
                EffectManager::fx_add_effect(gs, 12, 0, orig_pos.0 as i32, orig_pos.1 as i32, 0);
            }

            if !Self::transfer_char(gs, co, x, y) {
                gs.do_character_log(
                    cn,
                    core::types::FontColor::Red,
                    "GOTO failed. Dykstra was right (Elrac will explain this comment if you ask nicely).\n",
                );
                return;
            }

            let new_pos = {
                let character = &gs.characters[co];
                (character.x as i32, character.y as i32)
            };
            if character_visible {
                EffectManager::fx_add_effect(gs, 12, 0, new_pos.0, new_pos.1, 0);
            }
            gs.do_character_log(
                cn,
                core::types::FontColor::Green,
                &format!(
                    "{} teleported to ({}, {})\n",
                    if cn == co { "You" } else { &character_name },
                    new_pos.0,
                    new_pos.1
                ),
            );
            return;
        }

        log::error!(
            "Failed to execute goto command for character {} with input '{},{}'",
            cn,
            cx,
            cy
        );

        gs.do_character_log(
            cn,
            core::types::FontColor::Red,
            &format!("goto() failed with input '{},{}'\n", cx, cy),
        );
    }

    fn goto_target_coordinates(cn: usize, cx: &str, cy: &str) -> Option<(usize, usize)> {
        let target_x = match cx.parse::<usize>() {
            Ok(val) => val,
            Err(_) => {
                log::error!(
                    "Failed to parse X coordinate '{}' in goto command for character {}",
                    cx,
                    cn
                );
                return None;
            }
        };

        let target_y = match cy.parse::<usize>() {
            Ok(val) => val,
            Err(_) => {
                log::error!(
                    "Failed to parse Y coordinate '{}' in goto command for character {}",
                    cy,
                    cn
                );
                return None;
            }
        };

        Some((target_x, target_y))
    }

    fn goto_cardinal_length(
        gs: &mut GameState,
        cn: usize,
        cx: &str,
        cy: &str,
    ) -> Option<(usize, usize)> {
        if cx.chars().next().unwrap_or_default().is_alphabetic()
            && !cy.chars().next().unwrap_or_default().is_numeric()
        {
            log::debug!("Not a cardinal direction + length formatted goto command");
            return None;
        }

        if cx.chars().next().unwrap_or_default().is_numeric()
            && cy.chars().next().unwrap_or_default().is_numeric()
        {
            log::debug!("Not a cardinal direction + length formatted goto command");
            return None;
        }

        // Attempting to use format like "n 10" or "s 5", etc;
        // but we didn't use N/S/E/W as the first character
        if !["n", "s", "e", "w"].contains(&cx.to_lowercase().as_str()) {
            gs.do_character_log(
                cn,
                core::types::FontColor::Red,
                &format!(
                    "Invalid coordinate format provided for goto command: '{},{}'.\n",
                    cx, cy
                ),
            );
            return None;
        }

        let character = &gs.characters[cn];
        let (current_x, current_y) = (character.x as usize, character.y as usize);

        // North - decrease x
        // South - increase x
        // East  - increase y
        // West  - decrease y
        let (target_x, target_y) = match cx.to_lowercase().as_str() {
            "n" => {
                if let Ok(val) = cy.parse::<i32>() {
                    let new_x = (current_x as i32 - val).max(1) as usize;
                    (new_x, current_y)
                } else {
                    log::error!(
                        "Failed to parse X coordinate '{}' in goto command for character {}",
                        cy,
                        cn
                    );
                    return None;
                }
            }
            "s" => {
                if let Ok(val) = cy.parse::<i32>() {
                    let new_x =
                        (current_x as i32 + val).min(core::constants::SERVER_MAPX - 2) as usize;
                    (new_x, current_y)
                } else {
                    log::error!(
                        "Failed to parse X coordinate '{}' in goto command for character {}",
                        cy,
                        cn
                    );
                    return None;
                }
            }
            "e" => {
                if let Ok(val) = cy.parse::<i32>() {
                    let new_y =
                        (current_y as i32 + val).min(core::constants::SERVER_MAPY - 2) as usize;
                    (current_x, new_y)
                } else {
                    log::error!(
                        "Failed to parse Y coordinate '{}' in goto command for character {}",
                        cy,
                        cn
                    );
                    return None;
                }
            }
            "w" => {
                if let Ok(val) = cy.parse::<i32>() {
                    let new_y = (current_y as i32 - val).max(1) as usize;
                    (current_x, new_y)
                } else {
                    log::error!(
                        "Failed to parse Y coordinate '{}' in goto command for character {}",
                        cy,
                        cn
                    );
                    return None;
                }
            }
            _ => {
                log::error!(
                    "Invalid cardinal direction '{}' in goto command for character {} - this should've been filtered out already",
                    cx,
                    cn
                );
                return None;
            }
        };

        Some((target_x, target_y))
    }

    fn goto_character_by_name(
        gs: &mut GameState,
        cn: usize,
        cx: &str,
        cy: &str,
    ) -> Option<(usize, usize)> {
        if cx.chars().next().unwrap().is_numeric() || !cy.is_empty() {
            log::debug!("Not a character name formatted goto command");
            return None;
        }

        let target_name = cx;
        let target_location: Option<(usize, usize)> = gs
            .characters
            .iter()
            .find(|char| {
                char.used != core::constants::USE_EMPTY
                    && char.get_name().eq_ignore_ascii_case(target_name)
            })
            .map(|target_char| (target_char.x as usize, target_char.y as usize));

        if target_location.is_none() {
            log::error!("Character name '{}' not found in goto command", target_name);

            gs.do_character_log(
                cn,
                core::types::FontColor::Red,
                &format!("No such character found with name '{}'.\n", target_name),
            );
            return None;
        }

        Some((target_location.unwrap().0, target_location.unwrap().1))
    }

    /// Show comprehensive information about character `co` to `cn`.
    ///
    /// Mirrors the admin `info` command, revealing attributes, positions,
    /// flags and privileged data depending on caller permissions.
    ///
    /// # Arguments
    /// * `cn` - Requesting character
    /// * `co` - Target character
    pub fn info(gs: &mut GameState, cn: usize, co: usize) {
        if !Character::is_sane_character(cn) {
            return;
        }
        if !Character::is_sane_character(co) {
            gs.do_character_log(
                cn,
                core::types::FontColor::Red,
                "There's no such character.\n",
            );
            return;
        }

        // Access checks: sane NPCs are hidden from non-gods/imp/usurp; gods hidden from non-gods
        let denied = {
            let target = &gs.characters[co];
            let caller = &gs.characters[cn];
            let is_sane_npc = (target.flags & CharacterFlags::Player.bits()) == 0;
            let caller_is_priv = (caller.flags & CharacterFlags::God.bits()) != 0
                || (caller.flags & CharacterFlags::Imp.bits()) != 0
                || (caller.flags & CharacterFlags::Usurp.bits()) != 0;
            (is_sane_npc && !caller_is_priv)
                || (((target.flags & CharacterFlags::God.bits()) != 0)
                    && (caller.flags & CharacterFlags::God.bits()) == 0)
        };
        if denied {
            gs.do_character_log(cn, core::types::FontColor::Red, "Access denied.\n");
            return;
        }

        // Print detailed character info via char_info first (matches C++ flow)
        driver::char_info(gs, cn, co);

        // cnum_str: only visible to IMP/USURP
        let cnum_str = {
            let caller = &gs.characters[cn];
            if (caller.flags & CharacterFlags::Imp.bits()) != 0
                || (caller.flags & CharacterFlags::Usurp.bits()) != 0
            {
                format!(" ({})", co)
            } else {
                String::new()
            }
        };

        // Determine position visibility
        let (
            pos_x,
            pos_y,
            pts,
            need,
            player_flag,
            temp_val,
            hp_max,
            end_max,
            mana_max,
            speed,
            gold,
            gold_data13,
            kindred,
            data_vals,
            luck,
            gethit_dam,
            current_online_time,
            total_online_time,
            alignment,
            armor,
            weapon,
            a_hp,
            a_end,
            a_mana,
        ) = {
            let t = &gs.characters[co];
            let posx = t.x as i32;
            let posy = t.y as i32;
            let p = t.points_tot;
            let need = helpers::points_tolevel(t.points_tot as u32) as i32;
            let player_flag = (t.flags & CharacterFlags::Player.bits()) != 0;
            (
                posx,
                posy,
                p,
                need,
                player_flag,
                t.temp as i32,
                t.hp[5] as i32,
                t.end[5] as i32,
                t.mana[5] as i32,
                t.speed as i32,
                t.gold,
                t.data[13],
                t.kindred,
                t.data,
                t.luck,
                t.gethit_dam as i32,
                t.current_online_time as i32,
                t.total_online_time as i32,
                t.alignment as i32,
                t.armor as i32,
                t.weapon as i32,
                t.a_hp,
                t.a_end,
                t.a_mana,
            )
        };

        let hp_cur = a_hp / 1000;
        let end_cur = a_end / 1000;
        let mana_cur = a_mana / 1000;

        fn int2str(val: i32) -> String {
            let val = val.max(0);
            if val < 99_000 {
                format!("{}", val)
            } else if val < 99_000_000 {
                format!("{}K", val / 1000)
            } else {
                format!("{}M", val / 1_000_000)
            }
        }

        // Hide position if invisible to caller (match original invis_level check)
        let mut px = pos_x;
        let mut py = pos_y;
        let (hide_pos, caller_priv) = {
            let tflags = gs.characters[co].flags;
            let caller = &gs.characters[cn];
            let invis_or_nowho = (tflags & CharacterFlags::Invisible.bits()) != 0
                || (tflags & CharacterFlags::NoWho.bits()) != 0;
            let caller_priv = (caller.flags & CharacterFlags::Imp.bits()) != 0
                || (caller.flags & CharacterFlags::Usurp.bits()) != 0;
            (invis_or_nowho, caller_priv)
        };
        if hide_pos && !caller_priv {
            let cn_invis_level = helpers::invis_level(&gs.characters[cn]);
            let co_invis_level = helpers::invis_level(&gs.characters[co]);
            if co_invis_level > cn_invis_level {
                px = 0;
                py = 0;
            }
        }

        let pos_str = if px != 0 || py != 0 {
            format!(" Pos={},{}.", px, py)
        } else {
            String::new()
        };

        // Print header line depending on player or NPC
        if player_flag {
            let rank_short = ranks::rank_name_shortened(pts as u32);
            let target_name = gs.characters[co].get_name().to_string();
            gs.do_character_log(
                cn,
                core::types::FontColor::Yellow,
                &format!(
                    "{} {}{}{} Pts/need={}/{}.\n",
                    rank_short,
                    target_name,
                    cnum_str,
                    pos_str,
                    int2str(pts),
                    int2str(need)
                ),
            );
        } else {
            // NPC
            let temp_str = {
                let caller = &gs.characters[cn];
                if (caller.flags & CharacterFlags::Imp.bits()) != 0
                    || (caller.flags & CharacterFlags::Usurp.bits()) != 0
                {
                    format!(" Temp={}", temp_val)
                } else {
                    String::new()
                }
            };
            let rank_short = ranks::rank_name_shortened(pts as u32);
            let target_name = gs.characters[co].get_name().to_string();
            gs.do_character_log(
                cn,
                core::types::FontColor::Yellow,
                &format!(
                    "{} {}{}{}{}.\n",
                    rank_short, target_name, cnum_str, pos_str, temp_str
                ),
            );
        }

        // HP/End/Mana line
        gs.do_character_log(
            cn,
            core::types::FontColor::Yellow,
            &format!(
                "HP={}/{}, End={}/{}, Mana={}/{}.\n",
                hp_cur, hp_max, end_cur, end_max, mana_cur, mana_max
            ),
        );

        // Speed/Gold line
        gs.do_character_log(
            cn,
            core::types::FontColor::Yellow,
            &format!(
                "Speed={}. Gold={}.{:02}G ({}.{:02}G).\n",
                speed,
                gold / 100,
                gold % 100,
                gold_data13 / 100,
                gold_data13 % 100
            ),
        );

        // Last PvP attack for purple players
        if player_flag
            && (kindred & traits::KIN_PURPLE as i32) != 0
            && data_vals[core::constants::CHD_ATTACKTIME] != 0
        {
            let dt = gs.globals.ticker - gs.characters[co].data[core::constants::CHD_ATTACKTIME];
            if (gs.characters[cn].flags & CharacterFlags::Imp.bits()) != 0 {
                let victim = gs.characters[co].data[core::constants::CHD_ATTACKVICT] as usize;
                if Character::is_sane_character(victim) {
                    let victim_name = gs.characters[victim].get_name().to_string();
                    gs.do_character_log(
                        cn,
                        core::types::FontColor::Yellow,
                        &format!(
                            "Last PvP attack: {}, against {}.\n",
                            helpers::ago_string(dt as u128),
                            victim_name
                        ),
                    );
                }
            } else {
                gs.do_character_log(
                    cn,
                    core::types::FontColor::Yellow,
                    &format!("Last PvP attack: {}.\n", helpers::ago_string(dt as u128)),
                );
            }
        }

        // Additional info for IMP/USURP
        let caller_priv = {
            let c = &gs.characters[cn];
            (c.flags & CharacterFlags::Imp.bits()) != 0
                || (c.flags & CharacterFlags::Usurp.bits()) != 0
        };
        if caller_priv {
            // Print several data fields similar to C++ output
            {
                gs.do_character_log(
                    cn,
                    core::types::FontColor::Yellow,
                    &format!(
                        "Killed {} NPCs below rank, {} NPCs at rank, {} NPCs above rank.\n",
                        data_vals[23], data_vals[24], data_vals[25]
                    ),
                );
                gs.do_character_log(
                    cn,
                    core::types::FontColor::Yellow,
                    &format!(
                        "Killed {} players outside arena, killed {} shopkeepers.\n",
                        data_vals[29], data_vals[40]
                    ),
                );
                gs.do_character_log(
                    cn,
                    core::types::FontColor::Yellow,
                    &format!(
                        "BS: Killed {} NPCs below rank, {} NPCs at rank, {} NPCs above rank, {} candles returned.\n",
                        data_vals[26], data_vals[27], data_vals[28], data_vals[43]
                    ),
                );
                gs.do_character_log(
                    cn,
                    core::types::FontColor::Yellow,
                    &format!(
                        "Armor={}, Weapon={}. Alignment={}.\n",
                        armor, weapon, alignment
                    ),
                );
                // Group/Single Awake/Spells
                let group_count = if gs.characters[co].group_active() {
                    1
                } else {
                    0
                };
                let single_awake = data_vals[92];
                let spells = data_vals[96];
                gs.do_character_log(
                    cn,
                    core::types::FontColor::Yellow,
                    &format!(
                        "Group={} ({}), Single Awake={}, Spells={}.\n",
                        data_vals[42], group_count, single_awake, spells
                    ),
                );
                gs.do_character_log(
                    cn,
                    core::types::FontColor::Yellow,
                    &format!("Luck={}, Gethit_Dam={}.\n", luck, gethit_dam),
                );
                gs.do_character_log(
                    cn,
                    core::types::FontColor::Yellow,
                    &format!(
                        "Current Online Time: {}d {}h {}m {}s, Total Online Time: {}d {}h {}m {}s.\n",
                        current_online_time / (core::constants::TICKS * 60 * 60 * 24),
                        (current_online_time / (core::constants::TICKS * 60 * 60)) % 24,
                        (current_online_time / (core::constants::TICKS * 60)) % 60,
                        (current_online_time / core::constants::TICKS) % 60,
                        total_online_time / (core::constants::TICKS * 60 * 60 * 24),
                        (total_online_time / (core::constants::TICKS * 60 * 60)) % 24,
                        (total_online_time / (core::constants::TICKS * 60)) % 60,
                        (total_online_time / core::constants::TICKS) % 60
                    ),
                );
            }

            // Self-destruct time for sane NPCs
            if (gs.characters[co].flags & CharacterFlags::Player.bits()) == 0
                && gs.characters[co].data[64] != 0
            {
                let t = gs.characters[co].data[64] - gs.globals.ticker;
                let t_secs = t / core::constants::TICKS;
                let mins = t_secs / 60;
                let secs = t_secs % 60;
                gs.do_character_log(
                    cn,
                    core::types::FontColor::Yellow,
                    &format!("Will self destruct in {}m {}s.\n", mins, secs),
                );
            }
        }
    }

    /// Inspect a concrete item instance and display details to `cn`.
    ///
    /// # Arguments
    /// * `cn` - Requesting character
    /// * `item_index` - Item instance index
    pub fn iinfo(gs: &mut GameState, cn: usize, item_index: usize) {
        if !Character::is_sane_character(cn) || !core::types::Item::is_sane_item(item_index) {
            return;
        }

        let item = &gs.items[item_index];
        let sprite_0_to_print = item.sprite[0];
        let sprite_1_to_print = item.sprite[1];
        let carried_to_print = item.carried;
        let used = item.used;
        gs.do_character_log(
            cn,
            core::types::FontColor::Green,
            &format!(
                "Item Info: ID={}, Sprite=[{},{}], Carried={}, Used={}\n",
                item_index, sprite_0_to_print, sprite_1_to_print, carried_to_print, used
            ),
        );
    }

    /// Inspect an item template and display its fields to `cn`.
    ///
    /// # Arguments
    /// * `cn` - Requesting character
    /// * `template` - Item template id
    pub fn tinfo(gs: &mut GameState, cn: usize, template: usize) {
        if !Character::is_sane_character(cn) || !core::types::Item::is_sane_item_template(template)
        {
            return;
        }

        let tmpl = &gs.item_templates[template];
        let sprite_0_to_print = tmpl.sprite[0];
        let sprite_1_to_print = tmpl.sprite[1];
        let used_to_print = tmpl.used;
        gs.do_character_log(
            cn,
            core::types::FontColor::Green,
            &format!(
                "Template Info: ID={}, Sprite=[{},{}], Used={}\n",
                template, sprite_0_to_print, sprite_1_to_print, used_to_print
            ),
        );
    }

    /// List or check unique items on the server for admin inspection.
    ///
    /// # Arguments
    /// * `cn` - Requesting character
    pub fn unique(gs: &mut GameState, cn: usize) {
        if !Character::is_sane_character(cn) {
            return;
        }

        gs.do_character_log(cn, core::types::FontColor::Green, "Listing unique items:");
        for i in 1..core::constants::MAXITEM {
            if gs.items[i].used != core::constants::USE_EMPTY && gs.items[i].is_unique() {
                let sprite_0_to_print = gs.items[i].sprite[0];
                let sprite_1_to_print = gs.items[i].sprite[1];
                let carried_to_print = gs.items[i].carried;
                gs.do_character_log(
                    cn,
                    core::types::FontColor::Green,
                    &format!(
                        "  Item {}: Sprite=[{},{}], Carried={}\n",
                        i, sprite_0_to_print, sprite_1_to_print, carried_to_print
                    ),
                );
            }
        }
    }

    /// Produce a 'who' listing visible to `cn`.
    ///
    /// Formatting and visibility respects flags and privacy levels.
    ///
    /// # Arguments
    /// * `cn` - Requesting character
    pub fn who(gs: &mut GameState, cn: usize) {
        if !Character::is_sane_character(cn) {
            return;
        }

        let cn_flags = gs.characters[cn].flags;
        let cn_is_god = (cn_flags & CharacterFlags::God.bits()) != 0;
        let cn_is_imp_or_god =
            (cn_flags & (CharacterFlags::God.bits() | CharacterFlags::Imp.bits())) != 0;
        let cn_is_god_imp_or_usurp = (cn_flags
            & (CharacterFlags::God.bits()
                | CharacterFlags::Imp.bits()
                | CharacterFlags::Usurp.bits()))
            != 0;

        let cn_invis_level = helpers::invis_level(&gs.characters[cn]);

        let mut players = 0;
        gs.do_character_log(
            cn,
            core::types::FontColor::Blue,
            "-----------------------------------------------\n",
        );

        for n in 1..core::constants::MAXCHARS {
            let c = &gs.characters[n];
            if c.used != core::constants::USE_ACTIVE {
                continue;
            }

            let n_flags = c.flags;
            let n_is_player = (n_flags & CharacterFlags::Player.bits()) != 0;
            let n_is_usurp = (n_flags & CharacterFlags::Usurp.bits()) != 0;
            let n_is_invisible = (n_flags & CharacterFlags::Invisible.bits()) != 0;
            let n_is_nowho = (n_flags & CharacterFlags::NoWho.bits()) != 0;
            let n_is_staff = (n_flags & CharacterFlags::Staff.bits()) != 0;
            let n_is_god = (n_flags & CharacterFlags::God.bits()) != 0;

            let font = if !n_is_player {
                if !n_is_usurp {
                    continue;
                }
                if !cn_is_imp_or_god {
                    continue;
                }
                core::types::FontColor::Blue
            } else if n_is_invisible {
                let n_invis_level = helpers::invis_level(&gs.characters[n]);
                if cn_invis_level < n_invis_level {
                    continue;
                }
                core::types::FontColor::Red
            } else if n_is_nowho {
                if !cn_is_imp_or_god {
                    continue;
                }
                core::types::FontColor::Blue
            } else if n_is_staff || n_is_god {
                core::types::FontColor::Green
            } else {
                core::types::FontColor::Yellow
            };

            players += 1;

            let mut showarea = true;
            if n_is_god && !cn_is_god {
                showarea = false;
            }
            let n_is_purple = (c.kindred as u32 & traits::KIN_PURPLE) != 0;
            if n_is_purple && !cn_is_god_imp_or_usurp {
                showarea = false;
            }

            let name = c.get_name().to_string();
            let points_str = helpers::format_number(c.points_tot);
            let area_str = if showarea {
                area::get_area_m(c.x as i32, c.y as i32, false)
            } else {
                "--------".to_string()
            };

            let is_poh = (n_flags & CharacterFlags::Poh.bits()) != 0;
            let is_poh_leader = (n_flags & CharacterFlags::PohLeader.bits()) != 0;

            gs.do_character_log(
                cn,
                font,
                &format!(
                    "{:4}: {:<10.10}{}{}{} {:<8.8} {:<18.18}\n",
                    n,
                    name,
                    if n_is_purple { '*' } else { ' ' },
                    if is_poh { '+' } else { ' ' },
                    if is_poh_leader { '+' } else { ' ' },
                    points_str,
                    area_str,
                ),
            );
        }

        for n in 1..core::constants::MAXCHARS {
            let c = &gs.characters[n];
            let n_flags = c.flags;
            if (n_flags & CharacterFlags::Player.bits()) != 0 {
                continue;
            }
            if c.data[63] != cn as i32 {
                continue;
            }

            let rank_short = ranks::rank_name_shortened(c.points_tot as u32);
            let name = c.get_name().to_string();
            let area_str = area::get_area_m(c.x as i32, c.y as i32, false);
            let n_is_purple = (c.kindred as u32 & traits::KIN_PURPLE) != 0;
            let is_poh = (n_flags & CharacterFlags::Poh.bits()) != 0;
            let is_poh_leader = (n_flags & CharacterFlags::PohLeader.bits()) != 0;

            gs.do_character_log(
                cn,
                core::types::FontColor::Blue,
                &format!(
                    "{:.5} {:<10.10}{}{}{} {:<23.23}\n",
                    rank_short,
                    name,
                    if n_is_purple { '*' } else { ' ' },
                    if is_poh { '+' } else { ' ' },
                    if is_poh_leader { '+' } else { ' ' },
                    area_str,
                ),
            );
        }

        gs.do_character_log(
            cn,
            core::types::FontColor::Blue,
            "-----------------------------------------------\n",
        );
        gs.do_character_log(
            cn,
            core::types::FontColor::Yellow,
            &format!(
                "{:3} player{} online.\n",
                players,
                if players > 1 { "s" } else { "" }
            ),
        );
    }

    /// Show implemented admin commands or privileges.
    ///
    /// # Arguments
    /// * `cn` - Requesting character
    pub fn implist(gs: &mut GameState, cn: usize) {
        if !Character::is_sane_character(cn) {
            return;
        }

        let cn_flags = gs.characters[cn].flags;
        let cn_is_god = (cn_flags & CharacterFlags::God.bits()) != 0;
        let cn_is_god_imp_or_usurp = (cn_flags
            & (CharacterFlags::God.bits()
                | CharacterFlags::Imp.bits()
                | CharacterFlags::Usurp.bits()))
            != 0;

        let mut imps = 0;
        gs.do_character_log(
            cn,
            core::types::FontColor::Blue,
            "-----------------------------------------------\n",
        );

        for n in 1..core::constants::MAXCHARS {
            let c = &gs.characters[n];
            if c.used == core::constants::USE_EMPTY {
                continue;
            }

            let n_flags = c.flags;
            if (n_flags & CharacterFlags::Player.bits()) == 0 {
                continue;
            }
            if (n_flags & CharacterFlags::Imp.bits()) == 0 {
                continue;
            }

            imps += 1;

            let mut showarea = true;
            let n_is_god = (n_flags & CharacterFlags::God.bits()) != 0;
            if n_is_god && !cn_is_god {
                showarea = false;
            }
            let n_is_purple = (c.kindred as u32 & traits::KIN_PURPLE) != 0;
            if n_is_purple && !cn_is_god_imp_or_usurp {
                showarea = false;
            }

            let name = c.get_name().to_string();
            let points_str = helpers::format_number(c.points_tot);
            let area_str = if showarea {
                area::get_area_m(c.x as i32, c.y as i32, false)
            } else {
                "--------".to_string()
            };

            let is_poh = (n_flags & CharacterFlags::Poh.bits()) != 0;
            let is_poh_leader = (n_flags & CharacterFlags::PohLeader.bits()) != 0;

            gs.do_character_log(
                cn,
                core::types::FontColor::Yellow,
                &format!(
                    "{:4}: {:<10.10}{}{}{} {:<8.8} {:.18}\n",
                    n,
                    name,
                    if n_is_purple { '*' } else { ' ' },
                    if is_poh { '+' } else { ' ' },
                    if is_poh_leader { '+' } else { ' ' },
                    points_str,
                    area_str
                ),
            );
        }

        gs.do_character_log(
            cn,
            core::types::FontColor::Blue,
            "-----------------------------------------------\n",
        );
        gs.do_character_log(
            cn,
            core::types::FontColor::Yellow,
            &format!("{:3} imp{}.\n", imps, if imps > 1 { "s" } else { "" }),
        );
    }

    /// Show a compact user 'who' listing to `cn` (counts/brief data).
    ///
    /// # Arguments
    /// * `cn` - Requesting character
    pub fn user_who(gs: &mut GameState, cn: usize) {
        if !Character::is_sane_character(cn) {
            return;
        }

        let cn_flags = gs.characters[cn].flags;
        let cn_is_god = (cn_flags & CharacterFlags::God.bits()) != 0;
        let cn_is_god_imp_or_usurp = (cn_flags
            & (CharacterFlags::God.bits()
                | CharacterFlags::Imp.bits()
                | CharacterFlags::Usurp.bits()))
            != 0;

        let mut players = 0;
        gs.do_character_log(
            cn,
            core::types::FontColor::Yellow,
            "-----------------------------------------------\n",
        );

        for n in 1..core::constants::MAXCHARS {
            let c = &gs.characters[n];
            let n_flags = c.flags;
            if (n_flags & CharacterFlags::Player.bits()) == 0 {
                continue;
            }
            let n_is_invisible = (n_flags & CharacterFlags::Invisible.bits()) != 0;
            let n_is_nowho = (n_flags & CharacterFlags::NoWho.bits()) != 0;
            if c.used != core::constants::USE_ACTIVE || n_is_invisible || n_is_nowho {
                continue;
            }

            players += 1;

            let n_is_staff = (n_flags & CharacterFlags::Staff.bits()) != 0;
            let n_is_god = (n_flags & CharacterFlags::God.bits()) != 0;
            let font = if n_is_staff || n_is_god {
                core::types::FontColor::Green
            } else {
                core::types::FontColor::Yellow
            };

            let mut showarea = true;
            if n_is_god && !cn_is_god {
                showarea = false;
            }
            let n_is_purple = (c.kindred as u32 & traits::KIN_PURPLE) != 0;
            if n_is_purple && !cn_is_god_imp_or_usurp {
                showarea = false;
            }

            let rank_short = ranks::rank_name_shortened(c.points_tot as u32);
            let name = c.get_name().to_string();
            let area_str = if showarea {
                area::get_area_m(c.x as i32, c.y as i32, false)
            } else {
                "--------".to_string()
            };

            let is_poh = (n_flags & CharacterFlags::Poh.bits()) != 0;
            let is_poh_leader = (n_flags & CharacterFlags::PohLeader.bits()) != 0;

            gs.do_character_log(
                cn,
                font,
                &format!(
                    "{:.5} {:<10.10}{}{}{} {:<23.23}\n",
                    rank_short,
                    name,
                    if n_is_purple { '*' } else { ' ' },
                    if is_poh { '+' } else { ' ' },
                    if is_poh_leader { '+' } else { ' ' },
                    area_str,
                ),
            );
        }

        let gc = gs.characters[cn].data[core::constants::CHD_COMPANION] as usize;
        if Character::is_sane_character(gc) && gs.characters[gc].is_living_character(gc) {
            let gc_name = gs.characters[gc].get_name().to_string();
            let points_str = helpers::format_number(gs.characters[gc].points_tot);
            let area_str = area::get_area_m(
                gs.characters[gc].x as i32,
                gs.characters[gc].y as i32,
                false,
            );
            gs.do_character_log(
                cn,
                core::types::FontColor::Blue,
                &format!(
                    "{:4}: {:<10.10}@ {:<8.8} {:<20.20}\n",
                    gc, gc_name, points_str, area_str
                ),
            );
        }

        gs.do_character_log(
            cn,
            core::types::FontColor::Yellow,
            "-----------------------------------------------\n",
        );
        gs.do_character_log(
            cn,
            core::types::FontColor::Yellow,
            &format!(
                "{:3} player{} online.\n",
                players,
                if players > 1 { "s" } else { "" }
            ),
        );
    }

    /// Display a simple top-players leaderboard to `cn`.
    ///
    /// # Arguments
    /// * `cn` - Requesting character
    pub fn top(gs: &mut GameState, cn: usize) {
        if !Character::is_sane_character(cn) {
            return;
        }

        // Simple top players list - would need proper ranking system
        gs.do_character_log(cn, core::types::FontColor::Green, "Top players by points:");
        // This is simplified - original had more complex ranking
        for i in 1..core::constants::MAXCHARS {
            let c = &gs.characters[i];
            if c.is_living_character(i) && c.is_player() {
                if c.points > 100000 {
                    let points_to_print = c.points;
                    let name = c.get_name().to_string();
                    gs.do_character_log(
                        cn,
                        core::types::FontColor::Green,
                        &format!("  {}: Points={}\n", name, points_to_print),
                    );
                }
            }
        }
    }

    /// Admin create item command: spawn item template `x` for `cn`.
    ///
    /// Attempts to create from template and deliver it to the caller.
    ///
    /// # Arguments
    /// * `cn` - Requesting character
    /// * `x` - Template id
    pub fn create(gs: &mut GameState, cn: usize, x: i32) {
        if !Character::is_sane_character(cn) {
            return;
        }

        // Match original behavior: require a sane, take-able template.
        if x == 0 {
            gs.do_character_log(cn, core::types::FontColor::Red, "No such item.\n");
            return;
        }

        let template_id = x as usize;
        if !core::types::Item::is_sane_item_template(template_id) {
            gs.do_character_log(
                cn,
                core::types::FontColor::Red,
                &format!("Bad item number: {}.\n", x),
            );
            return;
        }

        let is_takeable = (gs.item_templates[template_id].flags
            & core::constants::ItemFlags::IF_TAKE.bits())
            != 0;

        if !is_takeable {
            gs.do_character_log(
                cn,
                core::types::FontColor::Red,
                &format!("Item template {} is not take-able.\n", x),
            );
            return;
        }

        let item_id = Self::create_item(gs, template_id);

        if let Some(item_id) = item_id {
            if !Self::give_character_item(gs, cn, item_id) {
                gs.do_character_log(cn, core::types::FontColor::Red, "Your inventory is full!\n");
                return;
            }

            let item_name = gs.items[item_id].get_name().to_string();
            gs.do_character_log(
                cn,
                core::types::FontColor::Green,
                &format!("Created one {}.\n", item_name),
            );
            chlog!(cn, "IMP: created one {}.", item_name);
        } else {
            gs.do_character_log(
                cn,
                core::types::FontColor::Red,
                &format!("god_create_item() failed for {}.\n", x),
            );
        }
    }

    /// Admin create item command: spawn special armor template for `cn`.
    ///
    /// Attempts to create and deliver it to the caller.
    ///
    /// # Arguments
    /// * `cn` - Requesting character
    /// * `armor` - Armor type (Titanium, Steel, etc.)
    /// * `animal` - Animal type (Bear, Lion, etc.)
    /// * `godly` - 'godly' or not provided
    pub fn create_special(gs: &mut GameState, cn: usize, armor: &str, animal: &str, godly: &str) {
        if !Character::is_sane_character(cn) {
            return;
        }

        let armor_type = ArmorType::from_str(armor).unwrap_or_else(|| ArmorType::Cloth);

        if armor_type == ArmorType::Cloth || armor_type == ArmorType::Leather {
            gs.do_character_log(
                cn,
                core::types::FontColor::Red,
                "Invalid armor type specified.\n",
            );
            return;
        }

        let animal_type = MagicArmorType::from_str(animal).unwrap_or_else(|| MagicArmorType::Bear);
        let is_godly = godly.to_lowercase().starts_with("go");

        let (helmet_temp, armor_temp) = match armor_type {
            ArmorType::Bronze => (57usize, 59usize),
            ArmorType::Steel => (63usize, 65usize),
            ArmorType::Gold => (69usize, 71usize),
            ArmorType::Crystal => (75usize, 76usize),
            ArmorType::Titanium => (94usize, 95usize),
            ArmorType::Emerald => (981usize, 982usize),
            ArmorType::Cloth | ArmorType::Leather => {
                // Already filtered above, but keep an explicit guard.
                gs.do_character_log(
                    cn,
                    core::types::FontColor::Red,
                    "Invalid armor type specified.\n",
                );
                return;
            }
        };

        let mut created: [usize; 2] = [0, 0];
        for (idx, temp) in [helmet_temp, armor_temp].iter().copied().enumerate() {
            let item_id = match Self::create_item(gs, temp) {
                Some(item_id) => item_id,
                None => {
                    // Clean up any items already created.
                    for &id in created.iter() {
                        if id != 0 {
                            gs.items[id].used = core::constants::USE_EMPTY;
                        }
                    }
                    gs.do_character_log(
                        cn,
                        core::types::FontColor::Red,
                        &format!("god_create_item() failed for {}.\n", temp),
                    );
                    return;
                }
            };

            created[idx] = item_id;

            // Apply the same logic as helpers::create_special_item, but deterministically based on args.
            {
                let item = &mut gs.items[item_id];

                // Match C: the resulting item should not be linked to its original template.
                item.temp = 0;

                let mul: i16 = if is_godly { 2 } else { 1 };
                let pref: &str = if is_godly { "Godly " } else { "" };

                let suffix: &str = match animal_type {
                    MagicArmorType::Lion => {
                        item.attrib[core::constants::AT_BRAVE as usize][0] += 4 * mul as i8;
                        " of the Lion"
                    }
                    MagicArmorType::Snake => {
                        item.attrib[core::constants::AT_WILL as usize][0] += 4 * mul as i8;
                        " of the Snake"
                    }
                    MagicArmorType::Owl => {
                        item.attrib[core::constants::AT_INT as usize][0] += 4 * mul as i8;
                        " of the Owl"
                    }
                    MagicArmorType::Weasel => {
                        item.attrib[core::constants::AT_AGIL as usize][0] += 4 * mul as i8;
                        " of the Weasel"
                    }
                    MagicArmorType::Bear => {
                        item.attrib[core::constants::AT_STREN as usize][0] += 4 * mul as i8;
                        " of the Bear"
                    }
                    MagicArmorType::Magic => {
                        item.mana[0] += 10 * mul;
                        " of Magic"
                    }
                    MagicArmorType::Life => {
                        item.hp[0] += 10 * mul;
                        " of Life"
                    }
                    MagicArmorType::Defence => {
                        item.armor[0] += 2 * mul as i8;
                        " of Defence"
                    }
                };

                let spr: i16 = match temp {
                    57 => 840,    // Bronze Helmet
                    59 => 845,    // Bronze Armor
                    63 => 830,    // Steel Helmet
                    65 => 835,    // Steel Armor
                    69 => 870,    // Golden Helmet
                    71 => 875,    // Golden Armor
                    75 => 850,    // Crystal Helmet
                    76 => 855,    // Crystal Armor
                    94 => 860,    // Titanium Helmet
                    95 => 865,    // Titanium Armor
                    981 => 16860, // Emerald Helmet
                    982 => 16865, // Emerald Armor
                    _ => item.sprite[0],
                };
                item.sprite[0] = spr;

                item.max_damage = 0;

                let base_name = c_string_to_str(&item.name);
                let combined = format!("{}{}{}", pref, base_name, suffix);

                helpers::write_c_string(&mut item.name, &combined);
                // Match C: titlecase first letter of *name* only.
                if let Some(b0) = item.name.first_mut() {
                    *b0 = b0.to_ascii_uppercase();
                }

                helpers::write_c_string(&mut item.reference, &combined);
                helpers::write_c_string(&mut item.description, &format!("A {}.", combined));
            }
        }

        // Deliver both items (and roll back cleanly if we can't give the full pair).
        if !Self::give_character_item(gs, cn, created[0]) {
            gs.items[created[0]].used = core::constants::USE_EMPTY;
            gs.do_character_log(cn, core::types::FontColor::Red, "Your inventory is full!\n");
            return;
        }
        if !Self::give_character_item(gs, cn, created[1]) {
            // Remove the first item again to keep behavior consistent (we create a pair).
            let _ = Self::remove_item(gs, cn, created[0]);
            gs.items[created[0]].used = core::constants::USE_EMPTY;
            gs.items[created[1]].used = core::constants::USE_EMPTY;
            gs.do_character_log(cn, core::types::FontColor::Red, "Your inventory is full!\n");
            return;
        }

        let helmet_name = gs.items[created[0]].get_name().to_string();
        let armor_name = gs.items[created[1]].get_name().to_string();

        gs.do_character_log(
            cn,
            core::types::FontColor::Green,
            &format!("Created special items: {}, {}.\n", helmet_name, armor_name),
        );
        chlog!(
            cn,
            "IMP: created special items: {}, {}.",
            helmet_name,
            armor_name
        );
    }

    /// Find the next matching character using an explicit game-state borrow.
    fn find_next_char(gs: &mut GameState, start_index: usize, spec1: &str, spec2: &str) -> i32 {
        let spec1_lower = spec1.to_lowercase();
        let spec2_lower = spec2.to_lowercase();

        for i in start_index..core::constants::MAXCHARS {
            let c = &gs.characters[i];
            if !c.is_living_character(i) {
                continue;
            }

            let name = c.get_name().to_lowercase();
            let reference = &c.get_reference().to_lowercase();

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
    }

    /// Determine effective invisibility level between `looker` and `target`.
    ///
    /// Returns an integer representing the invisibility relationship used
    /// for access checks and hiding positional data.
    ///
    /// # Arguments
    /// * `looker` - Character performing the check
    /// * `target` - Target character
    fn invis(gs: &mut GameState, looker: usize, target: usize) -> bool {
        if !Character::is_sane_character(looker) || !Character::is_sane_character(target) {
            return true;
        }

        let looker_char = &gs.characters[looker];
        let target_char = &gs.characters[target];

        // Check if target is invisible
        if target_char.flags & CharacterFlags::Invisible.bits() != 0 {
            // Check if looker can see invisible
            if looker_char.flags & CharacterFlags::Infrared.bits() == 0 {
                return true;
            }
        }

        false
    }

    /// Summon another character to the caller's location.
    ///
    /// Supports direct numeric summon or name/rank based lookup.
    ///
    /// # Arguments
    /// * `cn` - Summoning character
    /// * `spec1`, `spec2`, `spec3` - Summon parameters
    pub fn summon(gs: &mut GameState, cn: usize, spec1: &str, spec2: &str, spec3: &str) {
        if !Character::is_sane_character(cn) {
            return;
        }

        if spec1.is_empty() {
            gs.do_character_log(cn, core::types::FontColor::Red, "summon whom?\n");
            return;
        }

        // Two modes: single-arg numeric (direct char id) or name/rank search (spec2 present)
        let mut co: usize = 0;

        if spec2.is_empty() {
            // single-arg: treat spec1 as character number
            co = spec1.parse::<usize>().unwrap_or(0);

            if co == 0 || !Character::is_sane_character(co) || Self::invis(gs, cn, co) {
                gs.do_character_log(cn, core::types::FontColor::Red, "No such character.\n");
                return;
            }

            // check for recently-dead/corpse
            let corpse_owner = if (gs.characters[co].flags & CharacterFlags::Body.bits()) != 0 {
                Some(gs.characters[co].data[core::constants::CHD_CORPSEOWNER])
            } else {
                None
            };

            if let Some(owner) = corpse_owner {
                gs.do_character_log(
                    cn,
                    core::types::FontColor::Red,
                    &format!("Character recently deceased; try {}.\n", owner),
                );
                return;
            }

            if co == cn {
                gs.do_character_log(
                    cn,
                    core::types::FontColor::Red,
                    "You can't summon yourself!\n",
                );
                return;
            }
        } else {
            // at least 2 args: find by name/rank, support spec3 (which)
            let mut count = 0usize;

            // validate numeric rank if spec2 starts with digit
            if spec2
                .chars()
                .next()
                .map(|c| c.is_ascii_digit())
                .unwrap_or(false)
            {
                if let Ok(rank) = spec2.parse::<usize>() {
                    if rank >= ranks::TOTAL_RANKS {
                        gs.do_character_log(
                            cn,
                            core::types::FontColor::Red,
                            &format!("No such rank: {}\n", spec2),
                        );
                        return;
                    }
                }
            }

            let which = spec3.parse::<usize>().unwrap_or(1).max(1);

            while count < which {
                let found = Self::find_next_char(gs, co, spec1, spec2) as usize;
                if found == 0 {
                    break;
                }
                co = found;

                // ignore self
                if co == cn {
                    continue;
                }

                // ignore bodies
                let is_body = (gs.characters[co].flags & CharacterFlags::Body.bits()) != 0;
                if is_body {
                    continue;
                }

                // ignore sleeping players
                let skip_sleeping = gs.characters[co].is_player()
                    && gs.characters[co].used != core::constants::USE_ACTIVE;
                if skip_sleeping {
                    continue;
                }

                // invisibility check: ignore whom we can't see
                if Self::invis(gs, cn, co) {
                    continue;
                }

                count += 1;
            }

            if co == 0 {
                // Not found — produce message similar to original C++ but simpler here
                gs.do_character_log(
                    cn,
                    core::types::FontColor::Red,
                    &format!("Couldn't find a {} {}.\n", spec1, spec2),
                );
                return;
            }
        }

        // At this point we have a target `co` to summon
        let (x, y, xo, yo) = {
            let summoner = &gs.characters[cn];
            let mut target_x = summoner.x as i32;
            let mut target_y = summoner.y as i32;

            // position in front of summoner based on direction
            match summoner.dir {
                DX_RIGHT => target_x += 1,
                DX_RIGHTUP => {
                    target_x += 1;
                    target_y -= 1;
                }
                DX_UP => target_y -= 1,
                DX_LEFTUP => {
                    target_x -= 1;
                    target_y -= 1;
                }
                DX_LEFT => target_x -= 1,
                DX_LEFTDOWN => {
                    target_x -= 1;
                    target_y += 1;
                }
                DX_DOWN => target_y += 1,
                DX_RIGHTDOWN => {
                    target_x += 1;
                    target_y += 1;
                }
                _ => {}
            }

            let tx = (target_x).clamp(1, core::constants::SERVER_MAPX - 2) as usize;
            let ty = (target_y).clamp(1, core::constants::SERVER_MAPY - 2) as usize;

            let xo = gs.characters[co].x as i32;
            let yo = gs.characters[co].y as i32;

            (tx, ty, xo, yo)
        };

        if !Self::transfer_char(gs, co, x, y) {
            gs.do_character_log(
                cn,
                core::types::FontColor::Red,
                "god_transfer_char() failed.\n",
            );

            // show effects at original and current position
            EffectManager::fx_add_effect(gs, 12, 0, xo, yo, 0);
            EffectManager::fx_add_effect(gs, 12, 0, xo, yo, 0);

            return;
        }

        let character_name = gs.characters[co].get_name().to_string();
        gs.do_character_log(
            cn,
            core::types::FontColor::Green,
            &format!("{} was summoned.\n", character_name),
        );

        log::info!("IMP: summoned character {}.", co);
    }

    /// Create a temporary mirror copy of a target character for inspection.
    ///
    /// Duplicates attributes into a new temporary character instance.
    ///
    /// # Arguments
    /// * `cn` - Requesting character
    /// * `spec1`, `spec2` - Mirror parameters
    pub fn mirror(gs: &mut GameState, cn: usize, spec1: &str, spec2: &str) {
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
            gs.do_character_log(
                cn,
                core::types::FontColor::Red,
                "create mirror-enemy of whom?\n",
            );
            return;
        } else if spec1.chars().all(|c| c.is_ascii_digit()) {
            spec1.parse::<usize>().unwrap_or(0)
        } else {
            Self::find_next_char(gs, 1, spec1, "") as usize
        };

        if !Character::is_sane_character(co) {
            gs.do_character_log(cn, core::types::FontColor::Red, "No such character.\n");
            return;
        }

        if gs.characters[co].flags & CharacterFlags::Body.bits() != 0 {
            gs.do_character_log(
                cn,
                core::types::FontColor::Red,
                "Character recently deceased.\n",
            );
            return;
        }

        if !gs.characters[co].is_player() {
            let target_name = gs.characters[co].get_name().to_string();
            gs.do_character_log(
                cn,
                core::types::FontColor::Red,
                &format!(
                    "{} is not a player, and you can't mirror monsters!\n",
                    target_name
                ),
            );
            return;
        }

        if co == cn {
            gs.do_character_log(
                cn,
                core::types::FontColor::Yellow,
                "You want an enemy? Here it is...!\n",
            );
        }

        // Create mirror character with template 968
        let cc = match Self::create_char(gs, 968, false) {
            Some(cc) => cc as usize,
            None => {
                gs.do_character_log(
                    cn,
                    core::types::FontColor::Red,
                    "god_create_char() failed.\n",
                );
                return;
            }
        };

        // Copy attributes from target to mirror
        {
            let target_name_bytes = gs.characters[co].name;
            let target_sprite = gs.characters[co].sprite;
            let target_attrib = gs.characters[co].attrib;
            let target_hp = gs.characters[co].hp;
            let target_end = gs.characters[co].end;
            let target_mana = gs.characters[co].mana;
            let target_skill = gs.characters[co].skill;
            let target_kindred = gs.characters[co].kindred as u32;
            let caster_weapon = gs.characters[cn].weapon;
            let caster_armor = gs.characters[cn].armor;
            let caster_x = gs.characters[cn].x;
            let caster_y = gs.characters[cn].y;

            let mirror = &mut gs.characters[cc];
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
                & (traits::KIN_TEMPLAR | traits::KIN_ARCHTEMPLAR | traits::KIN_SEYAN_DU)
                != 0
            {
                // TH -> hand2hand (str,str,agi)
                mirror.skill[0][0] = (target_skill[6][0] as i32
                    + bonus
                    + (target_attrib[4][0] as i32 - target_attrib[0][0] as i32) / 5)
                    .clamp(0, 255) as u8;
            } else if target_kindred & (traits::KIN_HARAKIM | traits::KIN_ARCHHARAKIM) != 0 {
                // Dag-> hand2hand (wil,agi,int)
                mirror.skill[0][0] = (target_skill[2][0] as i32
                    + bonus
                    + (target_attrib[2][0] as i32 - target_attrib[4][0] as i32) / 5)
                    .clamp(0, 255) as u8;
            } else if target_kindred
                & (traits::KIN_MERCENARY | traits::KIN_SORCERER | traits::KIN_WARRIOR)
                != 0
            {
                // Swo-> hand2hand (wil,agi,str)
                mirror.skill[0][0] = (target_skill[3][0] as i32 + bonus).clamp(0, 255) as u8;
            }

            mirror.weapon = caster_weapon;
            mirror.armor = caster_armor;
            mirror.set_do_update_flags();

            // Drop the mirror at caster's position
            Self::drop_char_fuzzy(gs, cc, caster_x as usize, caster_y as usize);

            // Add target as enemy
            driver::npc_add_enemy(gs, cc, co, true);

            let target_name = c_string_to_str(&target_name_bytes);
            gs.do_character_log(
                cn,
                core::types::FontColor::Green,
                &format!("Mirror of {} active (bonus: {})\n", target_name, bonus),
            );
        }
    }

    /// Create a thrall (controlled NPC) bound to the caller.
    ///
    /// Returns the thrall character index or 0 on failure.
    ///
    /// # Arguments
    /// * `cn` - Requesting character
    /// * `spec1`, `spec2` - Target and options
    pub fn thrall(gs: &mut GameState, cn: usize, spec1: &str, spec2: &str) -> i32 {
        if !Character::is_sane_character(cn) {
            return 0;
        }

        // Check for arguments
        if spec1.is_empty() {
            gs.do_character_log(cn, core::types::FontColor::Red, "enthrall whom?\n");
            return 0;
        }

        let co = if spec2.is_empty() {
            // Only one argument - parse character number
            let co = spec1.parse::<usize>().unwrap_or(0);

            if !Character::is_sane_character(co) {
                gs.do_character_log(cn, core::types::FontColor::Red, "No such character.\n");
                return 0;
            }

            if gs.characters[co].flags & CharacterFlags::Body.bits() != 0 {
                let corpse_owner = gs.characters[co].data[core::constants::CHD_COMPANION];
                gs.do_character_log(
                    cn,
                    core::types::FontColor::Red,
                    &format!("Character recently deceased; try {}.\n", corpse_owner),
                );
                return 0;
            }

            if co == cn {
                gs.do_character_log(
                    cn,
                    core::types::FontColor::Red,
                    "You can't enthrall yourself!\n",
                );
                return 0;
            }

            co
        } else {
            // At least 2 arguments - find character by name/rank
            let mut co = 0usize;
            loop {
                co = Self::find_next_char(gs, co, spec1, spec2) as usize;
                if co == 0 {
                    break;
                }
                if co == cn {
                    continue; // ignore self
                }
                let should_continue = gs.characters[co].flags & CharacterFlags::Body.bits() != 0;
                if should_continue {
                    continue; // ignore bodies
                }
                break;
            }

            if co == 0 {
                gs.do_character_log(
                    cn,
                    core::types::FontColor::Red,
                    &format!("Couldn't find a {} {}.\n", spec1, spec2),
                );
                return 0;
            }
            co
        };

        // Validate target
        let validation_failed = {
            if gs.characters[co].is_player() {
                let target_name = gs.characters[co].get_name().to_string();
                gs.do_character_log(
                    cn,
                    core::types::FontColor::Red,
                    &format!(
                        "{} is a player, and you can't enthrall players!\n",
                        target_name
                    ),
                );
                true
            } else if gs.characters[co].data[42] > 65536 {
                // Check if already a companion/thrall (data[42] is group, companions have group 65536+cn)
                let target_name = gs.characters[co].get_name().to_string();
                gs.do_character_log(
                    cn,
                    core::types::FontColor::Red,
                    &format!(
                        "{} is a companion/thrall, and you can't enthrall them!\n",
                        target_name
                    ),
                );
                true
            } else {
                false
            }
        };

        if validation_failed {
            return 0;
        }

        // Calculate position in front of summoner
        let (x, y) = {
            let summoner = &gs.characters[cn];
            let mut x = summoner.x as i32;
            let mut y = summoner.y as i32;

            match summoner.dir {
                DX_RIGHT => x += 1,
                DX_RIGHTUP => {
                    x += 1;
                    y -= 1;
                }
                DX_UP => y -= 1,
                DX_LEFTUP => {
                    x -= 1;
                    y -= 1;
                }
                DX_LEFT => x -= 1,
                DX_LEFTDOWN => {
                    x -= 1;
                    y += 1;
                }
                DX_DOWN => y += 1,
                DX_RIGHTDOWN => {
                    x += 1;
                    y += 1;
                }
                _ => {}
            }

            (x as usize, y as usize)
        };

        // Get target template and create thrall
        let target_template = gs.characters[co].temp;

        let ct = match Self::create_char(gs, target_template as usize, true) {
            Some(ct) => ct as usize,
            None => {
                gs.do_character_log(
                    cn,
                    core::types::FontColor::Red,
                    "god_create_char() failed.\n",
                );
                return 0;
            }
        };

        // Configure the thrall
        {
            let target_name_bytes = gs.characters[co].name;
            let target_reference = gs.characters[co].reference;
            let target_description = gs.characters[co].description;

            gs.characters[ct].name = target_name_bytes;
            gs.characters[ct].reference = target_reference;
            gs.characters[ct].description = target_description;

            // Make thrall act like a ghost companion
            gs.characters[ct].temp = core::constants::CT_COMPANION as u16;
            let ticker = gs.globals.ticker;
            gs.characters[ct].data[64] = ticker + 7 * 24 * 3600 * core::constants::TICKS; // die in one week
            gs.characters[ct].data[42] = (65536 + cn) as i32; // set group
            gs.characters[ct].data[59] = (65536 + cn) as i32; // protect all other members of this group

            // Make thrall harmless
            gs.characters[ct].data[24] = 0; // do not interfere in fights
            gs.characters[ct].data[36] = 0; // no walking around
            gs.characters[ct].data[43] = 0; // don't attack anyone
            gs.characters[ct].data[80] = 0; // no enemies
            gs.characters[ct].data[63] = cn as i32; // obey and protect enthraller

            gs.characters[ct].flags |=
                CharacterFlags::ShutUp.bits() | CharacterFlags::Thrall.bits();

            // Remove labyrinth items from worn slots
            for n in 0..20 {
                let item_id = gs.characters[ct].worn[n] as usize;
                if item_id != 0
                    && gs.items[item_id].flags & core::constants::ItemFlags::IF_LABYDESTROY.bits()
                        != 0
                {
                    gs.items[item_id].used = 0;
                    gs.characters[ct].worn[n] = 0;
                }
            }

            // Remove labyrinth items from inventory
            for n in 0..40 {
                let item_id = gs.characters[ct].item[n] as usize;
                if item_id != 0
                    && gs.items[item_id].flags & core::constants::ItemFlags::IF_LABYDESTROY.bits()
                        != 0
                {
                    gs.items[item_id].used = 0;
                    gs.characters[ct].item[n] = 0;
                }
            }

            // Remove labyrinth item from carried item
            let citem = gs.characters[ct].citem as usize;
            if citem != 0
                && gs.items[citem].flags & core::constants::ItemFlags::IF_LABYDESTROY.bits() != 0
            {
                gs.items[citem].used = 0;
                gs.characters[ct].citem = 0;
            }

            target_name_bytes
        };

        // Drop thrall at calculated position
        if !Self::drop_char_fuzzy(gs, ct, x, y) {
            gs.do_character_log(
                cn,
                core::types::FontColor::Red,
                "god_drop_char_fuzzy() called from god_thrall() failed.\n",
            );
            Self::destroy_items(gs, ct);
            gs.characters[ct].used = core::constants::USE_EMPTY;
            return 0;
        }

        let target_name_bytes = gs.characters[ct].name;
        let target_name = c_string_to_str(&target_name_bytes);
        gs.do_character_log(
            cn,
            core::types::FontColor::Green,
            &format!("{} was enthralled.\n", target_name),
        );

        ct as i32
    }

    /// Logs out the player when they walk into a tavern.
    ///
    /// # Arguments
    /// * `cn` - Target character
    pub fn tavern(gs: &mut GameState, cn: usize) {
        if !Character::is_sane_character(cn) {
            log::error!("god_tavern() called with invalid character number: {}", cn);
            return;
        }

        if gs.characters[cn].is_usurp_or_thrall() {
            gs.do_character_log(
                cn,
                core::types::FontColor::Red,
                "NPCs cannot use the tavern.\n",
            );
            return;
        }

        if gs.characters[cn].is_building() {
            log::info!("god_tavern() called for building character: {}", cn);
            God::build_stop(gs, cn);
        }

        gs.characters[cn].tavern_x = gs.characters[cn].x as u16;
        gs.characters[cn].tavern_y = gs.characters[cn].y as u16;
        let player_id = gs.characters[cn].player as usize;

        chlog!(cn, "Entered tavern and will be logged out.");
        player::plr_logout(gs, cn, player_id, LogoutReason::Tavern);
    }

    /// Admin command used to adjust a character's experience. Only
    /// dispatched from administrative commands in-game.
    ///
    /// # Arguments
    /// * `cn` - Requesting character
    /// * `arg1` - Target character or can be the amount if arg2 is empty (apply to self)
    /// * `arg2` - Increase amount
    pub fn raise_char(gs: &mut GameState, cn: usize, arg1: &str, arg2: &str) {
        log::debug!(
            "god_raise_char() called with arg1='{}', arg2='{}'",
            arg1,
            arg2
        );

        if !Character::is_sane_character(cn) {
            log::error!(
                "god_raise_char() called with invalid character number: {}",
                cn
            );
            return;
        }

        let target_arg_storage;
        let (target_arg, value_arg) = if arg2.is_empty() {
            log::debug!(
                "god_raise_char(): single-argument mode, applying to self: {}",
                cn
            );
            target_arg_storage = cn.to_string();
            (target_arg_storage.as_str(), arg1)
        } else {
            (arg1, arg2)
        };

        let (co, name) =
            if let Some((co, name)) = Self::find_character_by_name_or_id(gs, target_arg) {
                (co, name)
            } else {
                gs.do_character_log(
                    cn,
                    core::types::FontColor::Red,
                    &format!("No such character: {}\n", target_arg),
                );
                return;
            };

        let value = match value_arg.parse::<i32>() {
            Ok(v) => v,
            Err(_) => {
                gs.do_character_log(
                    cn,
                    core::types::FontColor::Red,
                    &format!("Invalid raise value: {}\n", value_arg),
                );
                return;
            }
        };

        if !Character::is_sane_character(co) {
            gs.do_character_log(
                cn,
                core::types::FontColor::Red,
                &format!("Invalid character number: {}\n", co),
            );
            return;
        }

        if value < 0 {
            gs.do_character_log(
                cn,
                core::types::FontColor::Red,
                &format!("Invalid raise value - must be positive: {}\n", value),
            );
            return;
        }

        gs.characters[co].points += value;
        gs.characters[co].points_tot += value;

        chlog!(cn, "Raised character {} experience by {}\n", name, value);

        gs.do_check_new_level(co);
        gs.do_character_log(
            co,
            core::types::FontColor::Green,
            format!(
                "You have been rewarded by the gods. You receive {} experience points.\n",
                value
            )
            .as_str(),
        );
    }

    /// Admin command used to adjust character experience downward.
    ///
    /// # Arguments
    /// * `cn` - Requesting character
    /// * `arg1` - Target character or can be the amount of arg2 isn't provided.
    /// * `arg2` - Decrease amount
    pub fn lower_char(gs: &mut GameState, cn: usize, arg1: &str, arg2: &str) {
        log::debug!(
            "god_lower_char() called with arg1='{}', arg2='{}'",
            arg1,
            arg2
        );

        if !Character::is_sane_character(cn) {
            log::error!(
                "god_lower_char() called with invalid character number: {}",
                cn
            );
            return;
        }

        let target_arg_storage;
        let (target_arg, value_arg) = if arg2.is_empty() {
            log::debug!(
                "god_lower_char(): single-argument mode, applying to self: {}",
                cn
            );
            target_arg_storage = cn.to_string();
            (target_arg_storage.as_str(), arg1)
        } else {
            (arg1, arg2)
        };

        let (co, name) =
            if let Some((co, name)) = Self::find_character_by_name_or_id(gs, target_arg) {
                (co, name)
            } else {
                gs.do_character_log(
                    cn,
                    core::types::FontColor::Red,
                    &format!("No such character: {}\n", target_arg),
                );
                return;
            };

        let value = match value_arg.parse::<i32>() {
            Ok(v) => v,
            Err(_) => {
                gs.do_character_log(
                    cn,
                    core::types::FontColor::Red,
                    &format!("Invalid lower value: {}\n", value_arg),
                );
                return;
            }
        };

        if !Character::is_sane_character(co) {
            gs.do_character_log(
                cn,
                core::types::FontColor::Red,
                &format!("Invalid character number: {}\n", co),
            );
            return;
        }

        if value < 0 {
            gs.do_character_log(
                cn,
                core::types::FontColor::Red,
                &format!("Invalid lower value - must be positive: {}\n", value),
            );
            return;
        }

        gs.characters[co].points -= value;
        gs.characters[co].points_tot -= value;

        chlog!(cn, "Lowered character {} experience by {}\n", name, value);

        gs.do_character_log(
            co,
            core::types::FontColor::Red,
            format!(
                "You have been punished by the gods. You lose {} experience points.\n",
                value
            )
            .as_str(),
        );
    }

    /// Add gold/silver to a character's coin purse.
    ///
    /// `value` is the gold amount; `silver` can add extra silver pieces.
    ///
    /// # Arguments
    /// * `cn` - Requesting character
    /// * `arg` - Recipient character or empty (if self)
    /// * `value` - Gold amount
    /// * `silver` - Silver amount
    pub fn gold_char(gs: &mut GameState, cn: usize, arg: &str, gold: u32, silver: u32) {
        log::debug!(
            "gold_char() called with arg='{}', gold='{}', silver='{}'",
            arg,
            gold,
            silver
        );

        if !Character::is_sane_character(cn) {
            log::error!("gold_char() called with invalid character number: {}", cn);
            return;
        }

        let total_silver = gold * 100 + silver;

        let target_arg_storage;
        let target_arg = if arg.is_empty() {
            log::debug!(
                "gold_char(): single-argument mode, applying to self: {}",
                cn
            );
            target_arg_storage = cn.to_string();
            target_arg_storage.as_str()
        } else {
            arg
        };

        let (co, name) =
            if let Some((co, name)) = Self::find_character_by_name_or_id(gs, target_arg) {
                (co, name)
            } else {
                gs.do_character_log(
                    cn,
                    core::types::FontColor::Red,
                    &format!("No such character: {}\n", target_arg),
                );
                return;
            };

        let target = &mut gs.characters[co];
        target.gold = (target.gold + total_silver as i32).max(0);
        target.set_do_update_flags();
        gs.do_character_log(
            cn,
            core::types::FontColor::Green,
            &format!("Gave {} silver to character {}\n", total_silver, name),
        );
    }

    /// Permanently erase a character or NPC from the world.
    ///
    /// With `erase_player` set, player accounts may be removed; safety
    /// checks prevent accidental deletion of important characters.
    ///
    /// # Arguments
    /// * `cn` - Requesting character
    /// * `co` - Target character
    /// * `erase_player` - If non-zero, allow player erasure
    pub fn erase(gs: &mut GameState, cn: usize, co: usize, erase_player: i32) {
        if co == 0 {
            gs.do_character_log(cn, core::types::FontColor::Red, "No such character.\n");
            return;
        }

        // Check if character is sane
        if !Character::is_sane_character(co) {
            gs.do_character_log(
                cn,
                core::types::FontColor::Red,
                &format!("Bad character number: {}\n", co),
            );
            return;
        }

        // Check if character is used
        let is_used = gs.characters[co].used != core::constants::USE_EMPTY;
        let is_player_or_usurp = (gs.characters[co].flags
            & (CharacterFlags::Player.bits() | CharacterFlags::Usurp.bits()))
            != 0;
        let character_name = gs.characters[co].name;

        if !is_used {
            gs.do_character_log(
                cn,
                core::types::FontColor::Red,
                &format!("Character {} is unused anyway.\n", co),
            );
            return;
        }

        // Check if player/QM but erase_player is false
        if is_player_or_usurp && erase_player == 0 {
            let name_str = c_string_to_str(&character_name);
            gs.do_character_log(
                cn,
                core::types::FontColor::Red,
                &format!(
                    "{} is a player or QM; use #PERASE if you insist.\n",
                    name_str
                ),
            );
            return;
        }

        // Check if erase_player is true but character is not player/usurp
        if erase_player != 0 && !is_player_or_usurp {
            let name_str = c_string_to_str(&character_name);
            gs.do_character_log(
                cn,
                core::types::FontColor::Red,
                &format!("{} is not a player; use #ERASE for NPCs.\n", name_str),
            );
            return;
        }

        if erase_player != 0 {
            // Erasing a player
            let name_str = c_string_to_str(&character_name);
            let player_id = gs.characters[co].player as usize;

            player::plr_logout(gs, co, player_id, LogoutReason::Shutdown);

            gs.characters[co].used = core::constants::USE_EMPTY;

            chlog!(cn, "IMP: Erased player {} ({}).", co, name_str);

            gs.do_character_log(
                cn,
                core::types::FontColor::Yellow,
                &format!("Player {} ({}) is no more.\n", co, name_str),
            );
        } else {
            // Erasing an NPC
            let name_str = c_string_to_str(&character_name);

            // Call do_char_killed(0, co)
            gs.do_character_killed(co, 0, false);

            gs.do_character_log(
                cn,
                core::types::FontColor::Yellow,
                &format!("NPC {} ({}) is no more.\n", co, name_str),
            );
        }
    }

    /// Kick a character from the server (mark as kicked and perform cleanup).
    ///
    /// Administrative action that ensures the target is disconnected and
    /// flagged appropriately.
    ///
    /// # Arguments
    /// * `cn` - Requesting character
    /// * `co` - Target character to kick
    pub fn kick(gs: &mut GameState, cn: usize, co: usize) {
        // Check co == 0
        if co == 0 {
            gs.do_character_log(cn, core::types::FontColor::Red, "No such character.\n");
            return;
        }

        // Check if character is sane and used
        if !Character::is_sane_character(co) {
            gs.do_character_log(
                cn,
                core::types::FontColor::Red,
                &format!("Bad character number: {}\n", co),
            );
            return;
        }

        let is_used = gs.characters[co].used != core::constants::USE_EMPTY;
        let character_name = gs.characters[co].name;

        if !is_used {
            gs.do_character_log(
                cn,
                core::types::FontColor::Red,
                &format!("Character {} is unused anyway.\n", co),
            );
            return;
        }

        let name_str = c_string_to_str(&character_name);
        let player_id = gs.characters[co].player as usize;

        player::plr_logout(gs, co, player_id, LogoutReason::IdleTooLong);

        gs.do_character_log(
            cn,
            core::types::FontColor::Yellow,
            &format!("Kicked {}.\n", name_str),
        );

        chlog!(cn, "IMP: Kicked {} ({}).", name_str, co);

        // Set CF_KICKED flag
        gs.characters[co].flags |= CharacterFlags::Kicked.bits();
    }

    /// Set a specific skill value for target character `co`.
    ///
    /// Validates the skill index and clamps the value before assignment.
    ///
    /// # Arguments
    /// * `cn` - Requesting character
    /// * `co` - Target character
    /// * `n` - Skill index
    /// * `val` - New skill value
    pub fn skill(gs: &mut GameState, cn: usize, co: usize, n: i32, val: i32) {
        if !Character::is_sane_character(cn) || !Character::is_sane_character(co) {
            return;
        }

        if !(0..50).contains(&n) {
            gs.do_character_log(
                cn,
                core::types::FontColor::Red,
                &format!("Invalid skill number: {}\n", n),
            );
            return;
        }

        let val = val.clamp(0, 127);

        let skill_name = core::types::skilltab::get_skill_name(n as usize);

        let target_name = gs.characters[co].get_name().to_string();
        let target = &mut gs.characters[co];
        target.skill[n as usize][0] = val as u8;
        target.skill[n as usize][1] = val as u8;
        target.set_do_update_flags();
        gs.do_character_log(
            cn,
            core::types::FontColor::Green,
            &format!(
                "Set skill {} ({}) to {} for character {}\n",
                n, skill_name, val, target_name
            ),
        );
    }

    /// Donate an item to one of the server's donation locations.
    ///
    /// Drops the item at the configured temple coordinates; `place` selects
    /// which temple to use.
    ///
    /// # Arguments
    /// * `item_id` - Item instance index
    /// * `place` - Donation site selector
    pub fn donate_item(gs: &mut GameState, item_id: usize, place: i32) {
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
        let place = if !(1..=2).contains(&place) {
            1 + helpers::random_mod_i32(2)
        } else {
            place
        };

        let x = DON_X[(place - 1) as usize];
        let y = DON_Y[(place - 1) as usize];

        // Try to drop the item at the donation location
        if !Self::drop_item_fuzzy(gs, item_id, x, y) {
            // If drop fails, destroy the item. Clear carried field to prevent
            // stale references (though drop_item_fuzzy should have already done this).
            gs.items[item_id].carried = 0;
            gs.items[item_id].x = 0;
            gs.items[item_id].y = 0;
            gs.items[item_id].used = core::constants::USE_EMPTY;
        }
    }

    /// Set raw flag bits on a target character. These are only dispatched
    /// via administrative commands in-game.
    ///
    /// Administrative helper to OR the provided `flag` into the target's
    /// flag field.
    pub fn set_flag(gs: &mut GameState, cn: usize, arg1: &str, flag: u64) {
        log::debug!(
            "god_set_flag() called with arg1='{}', flag={:x}",
            arg1,
            flag
        );
        if !Character::is_sane_character(cn) {
            return;
        }

        // Ensure we have an owned string in case we need to use the numeric id as a name
        let query = if arg1.is_empty() {
            // Default to own character if argument wasn't provided
            cn.to_string()
        } else {
            arg1.to_string()
        };

        if let Some((co, name)) = Self::find_character_by_name_or_id(gs, &query) {
            // Toggle the flag
            if gs.characters[co].flags & flag != 0 {
                gs.characters[co].flags &= !flag;
                gs.do_character_log(
                    cn,
                    core::types::FontColor::Green,
                    &format!(
                        "Removed flag {} ({:x}) from character {}\n",
                        character_flags_name(CharacterFlags::from_bits_truncate(flag)),
                        flag,
                        name
                    ),
                );
            } else {
                gs.characters[co].flags |= flag;
                gs.do_character_log(
                    cn,
                    core::types::FontColor::Green,
                    &format!(
                        "Added flag {} ({:x}) to character '{}'\n",
                        character_flags_name(CharacterFlags::from_bits_truncate(flag)),
                        flag,
                        name
                    ),
                );
            }

            gs.characters[co].set_do_update_flags();

            if flag == CharacterFlags::Invisible.bits() {
                let x = gs.characters[co].x as i32;
                let y = gs.characters[co].y as i32;
                EffectManager::fx_add_effect(gs, 12, 0, x, y, 0);
            }
        } else {
            gs.do_character_log(
                cn,
                core::types::FontColor::Red,
                &format!("No such character (id or name): '{}'\n", arg1),
            );
        }
    }

    /// Find a character by name (case-insensitive).
    ///
    /// Returns the character index and name string if found, or None if not.
    fn find_character_by_name_or_id(gs: &mut GameState, arg: &str) -> Option<(usize, String)> {
        if arg.chars().all(|c| c.is_numeric()) {
            // Search by character number
            let co = arg.parse::<usize>().unwrap_or(0);
            if Character::is_sane_character(co) {
                let name_str = c_string_to_str(&gs.characters[co].name);
                Some((co, name_str.to_string()))
            } else {
                None
            }
        } else {
            // Search by name
            let arg_lower = arg.to_lowercase();
            for (i, character) in gs.characters.iter().enumerate() {
                let name_str = c_string_to_str(&character.name);
                if name_str.to_lowercase() == arg_lower {
                    return Some((i, name_str.to_string()));
                }
            }
            None
        }
    }

    /// Set a global server flag (admin operation).
    ///
    /// Modifies server-level flags used to enable or disable features.
    pub fn set_gflag(gs: &mut GameState, cn: usize, flag: i32) {
        if !Character::is_sane_character(cn) {
            return;
        }

        let flag_bit = 1i32 << flag;
        if gs.globals.flags & flag_bit != 0 {
            gs.globals.flags &= !flag_bit;
            gs.do_character_log(
                cn,
                core::types::FontColor::Green,
                &format!("Removed global flag {}\n", flag),
            );
        } else {
            gs.globals.flags |= flag_bit;
            gs.do_character_log(
                cn,
                core::types::FontColor::Green,
                &format!("Added global flag {}\n", flag),
            );
        }
    }

    /// Toggle the purple (privileged) status for a character.
    ///
    /// Grants or removes purple display/privileges from `co`.
    pub fn set_purple(gs: &mut GameState, cn: usize, co: usize) {
        if !Character::is_sane_character(cn) || !Character::is_sane_character(co) {
            return;
        }

        // Toggle purple (PK) status
        // Assuming there's a PK flag in constants
        let pk_flag = 0x1000000u64; // Example PK flag
        let target_name = gs.characters[co].get_name().to_string();

        if gs.characters[co].flags & pk_flag != 0 {
            gs.characters[co].flags &= !pk_flag;
            gs.do_character_log(
                cn,
                core::types::FontColor::Green,
                &format!("Removed PK status from character {}\n", target_name),
            );
        } else {
            gs.characters[co].flags |= pk_flag;
            gs.do_character_log(
                cn,
                core::types::FontColor::Green,
                &format!("Added PK status to character {}\n", target_name),
            );
        }

        gs.characters[co].set_do_update_flags();
    }

    /// Change the race/template of character `co` to `temp`.
    ///
    /// Completely replaces the character with the template, preserving only
    /// essential account information like name, passwords, gold, and depot.
    /// This resets all stats, skills, and experience to template defaults.
    pub fn racechange(gs: &mut GameState, co: usize, temp: i32) {
        if !Character::is_sane_character(co) {
            return;
        }

        // Only allow for players
        let is_player = gs.characters[co].is_player();
        if !is_player {
            return;
        }

        if temp < 0 || temp >= core::constants::MAXTCHARS as i32 {
            gs.do_character_log(
                co,
                core::types::FontColor::Red,
                &format!("Invalid character template: {}\n", temp),
            );
            log::error!("Invalid character template: {}", temp);
            return;
        }

        let template = gs.character_templates[temp as usize];

        if template.used == core::constants::USE_EMPTY {
            gs.do_character_log(
                co,
                core::types::FontColor::Red,
                &format!("Template {} is not in use\n", temp),
            );
            log::error!("Template {} is not in use", temp);
            return;
        }

        // First destroy all items
        Self::destroy_items(gs, co);

        {
            let character = &mut gs.characters[co];

            // Preserve important data before replacing
            let old_pass1 = character.pass1;
            let old_pass2 = character.pass2;
            let old_gold = character.gold;
            let old_name = character.name;
            let old_reference = character.reference;
            let old_description = character.description;
            let old_dir = character.dir;
            let old_creation_date = character.creation_date;
            let old_login_date = character.login_date;
            let old_flags = character.flags;
            let old_kindred = character.kindred;
            let old_total_online_time = character.total_online_time;
            let old_current_online_time = character.current_online_time;
            let old_comp_volume = character.comp_volume;
            let old_raw_volume = character.raw_volume;
            let old_idle = character.idle;
            let old_x = character.x;
            let old_y = character.y;
            let old_tox = character.tox;
            let old_toy = character.toy;
            let old_frx = character.frx;
            let old_fry = character.fry;
            let old_mode = character.mode;
            let old_player = character.player;
            let old_luck = character.luck;
            let old_light = character.light;
            let old_status = character.status;
            let old_status2 = character.status2;
            let old_data = character.data;
            let old_depot = character.depot;

            // Replace character with template
            *character = template;

            // Restore preserved fields
            character.temp = temp as u16;
            character.pass1 = old_pass1;
            character.pass2 = old_pass2;
            character.gold = old_gold;
            character.name = old_name;
            character.reference = old_reference;
            character.description = old_description;
            character.dir = old_dir;

            // Set temple/tavern to mercenary home by default
            character.temple_x = 512;
            character.temple_y = 512;
            character.tavern_x = 512;
            character.tavern_y = 512;

            character.creation_date = old_creation_date;
            character.login_date = old_login_date;
            character.flags = old_flags;

            // Preserve purple kindred if they had it
            if (old_kindred & 0x00000001) != 0 {
                character.kindred |= 0x00000001; // KIN_PURPLE
                character.temple_x = 558;
                character.temple_y = 542;
            }

            character.total_online_time = old_total_online_time;
            character.current_online_time = old_current_online_time;
            character.comp_volume = old_comp_volume;
            character.raw_volume = old_raw_volume;
            character.idle = old_idle;

            // Set action times to max (full health/mana/endurance)
            character.a_end = 1000000;
            character.a_hp = 1000000;
            character.a_mana = 1000000;

            // Restore position
            character.x = old_x;
            character.y = old_y;
            character.tox = old_tox;
            character.toy = old_toy;
            character.frx = old_frx;
            character.fry = old_fry;

            character.mode = old_mode;
            character.used = core::constants::USE_ACTIVE;
            character.player = old_player;
            character.alignment = 0;
            character.luck = old_luck;
            character.light = old_light;
            character.status = old_status;
            character.status2 = old_status2;

            // Clear inventory, worn, and spell arrays (already done by destroy_items)
            for n in 0..40 {
                character.item[n] = 0;
            }
            for n in 0..20 {
                character.worn[n] = 0;
                character.spell[n] = 0;
            }

            // Restore data array but reset specific fields
            character.data = old_data;
            character.data[18] = 0; // pentagram experience
            character.data[20] = 0; // highest gorge solved
            character.data[21] = 0; // seyan'du sword bits
            character.data[22] = 0; // arena monster reset
            character.data[45] = 0; // current rank

            // Restore depot
            character.depot = old_depot;

            character.set_do_update_flags();

            log::info!(
                "Changed race of character {} to template {}",
                character.get_name(),
                temp
            );
        }

        gs.do_update_char(co);
    }

    /// Save character `co` to persistent storage.
    ///
    /// Returns `1` on success and performs necessary write operations.
    pub fn save(gs: &mut GameState, cn: usize, co: usize) -> bool {
        if !Character::is_sane_character(cn) || !Character::is_sane_character(co) {
            return false;
        }

        if !gs.characters[co].is_player() {
            gs.do_character_log(
                cn,
                core::types::FontColor::Red,
                "Cannot save non-player character\n",
            );
            return false;
        }

        let target_name = gs.characters[co].get_name().to_string();
        gs.do_character_log(
            cn,
            core::types::FontColor::Green,
            &format!("Saving character {}\n", target_name),
        );
        // TODO: Actual save logic would write to disk

        true
    }

    /// Command to make `co` perform a slap animation (cosmetic/admin).
    pub fn slap(gs: &mut GameState, cn: usize, co: usize) {
        if !Character::is_sane_character(cn) || !Character::is_sane_character(co) {
            return;
        }

        let (damage, target_name) = {
            let target = &mut gs.characters[co];

            let damage = (target.hp[0] / 10).max(1);
            target.hp[5] = (target.hp[5] as i32 - damage as i32).max(1) as u16;
            target.set_do_update_flags();

            (damage, target.get_name().to_string())
        };

        gs.do_character_log(
            cn,
            core::types::FontColor::Green,
            &format!("Slapped character {} for {} damage\n", target_name, damage),
        );
    }

    /// Change a character's sprite id.
    ///
    /// Performs validation of the sprite id before updating the character.
    pub fn spritechange(gs: &mut GameState, cn: usize, co: usize, sprite: i32) {
        if !Character::is_sane_character(cn) || !Character::is_sane_character(co) {
            return;
        }

        if !(0..=10000).contains(&sprite) {
            gs.do_character_log(
                cn,
                core::types::FontColor::Red,
                &format!("Invalid sprite number: {}\n", sprite),
            );
            return;
        }

        let target_name = {
            let target = &mut gs.characters[co];
            target.sprite = sprite as u16;
            target.set_do_update_flags();
            target.get_name().to_string()
        };

        gs.do_character_log(
            cn,
            core::types::FontColor::Green,
            &format!(
                "Changed sprite of character {} to {}\n",
                target_name, sprite
            ),
        );
    }

    /// Adjust the `luck` stat for a character.
    pub fn luck(gs: &mut GameState, cn: usize, co: usize, value: i32) {
        if !Character::is_sane_character(cn) || !Character::is_sane_character(co) {
            return;
        }

        let value = value.clamp(-10000, 10000);

        let target_name = {
            let target = &mut gs.characters[co];
            target.luck = value;
            target.set_do_update_flags();
            target.get_name().to_string()
        };

        gs.do_character_log(
            cn,
            core::types::FontColor::Green,
            &format!("Set luck of character {} to {}\n", target_name, value),
        );
    }

    /// Reset a character's description to a blank/default value.
    pub fn reset_description(gs: &mut GameState, cn: usize, co: usize) {
        if !Character::is_sane_character(cn) || !Character::is_sane_character(co) {
            return;
        }

        let target_name = {
            let target = &mut gs.characters[co];

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
            target.get_name().to_string()
        };

        gs.do_character_log(
            cn,
            core::types::FontColor::Green,
            &format!("Reset description for character {}\n", target_name),
        );
    }

    /// Set or change the visible name of a character, with validation.
    ///
    /// Ensures the new name meets length and character constraints.
    pub fn set_name(gs: &mut GameState, cn: usize, co: usize, name: &str) {
        if !Character::is_sane_character(cn) || !Character::is_sane_character(co) {
            return;
        }

        if name.len() > 16 || name.is_empty() {
            gs.do_character_log(
                cn,
                core::types::FontColor::Red,
                &format!("Invalid name length: {}\n", name.len()),
            );
            return;
        }

        let old_name = {
            let target = &mut gs.characters[co];
            let old_name = target.get_name().to_string();
            target.name = name
                .bytes()
                .take(40)
                .collect::<Vec<u8>>()
                .try_into()
                .unwrap_or([0; 40]);
            target.set_do_update_flags();
            old_name
        };

        gs.do_character_log(
            cn,
            core::types::FontColor::Green,
            &format!("Changed name of character from {} to {}\n", old_name, name),
        );
    }

    /// Usurp an NPC: take control of its slot as an admin operation.
    ///
    /// Transfers the caller into the NPC slot and preserves relevant state.
    ///
    /// # Arguments
    /// * `cn` - Requesting character
    /// * `co` - NPC to usurp
    pub fn usurp(gs: &mut GameState, cn: usize, co: usize) {
        // Check co == 0
        if co == 0 {
            gs.do_character_log(cn, core::types::FontColor::Red, "No such character.\n");
            return;
        }

        // Check if character is sane
        if !Character::is_sane_character(co) {
            gs.do_character_log(
                cn,
                core::types::FontColor::Red,
                &format!("Bad character number: {}\n", co),
            );
            return;
        }

        // Check if character is used and is an NPC (not a player)
        let is_used = gs.characters[co].used != core::constants::USE_EMPTY;
        let is_player_or_usurp = (gs.characters[co].flags
            & (CharacterFlags::Player.bits() | CharacterFlags::Usurp.bits()))
            != 0;
        let character_name = gs.characters[co].name;
        let co_temp = gs.characters[co].temp;

        if !is_used || is_player_or_usurp {
            gs.do_character_log(
                cn,
                core::types::FontColor::Red,
                &format!("Character {} is not an NPC.\n", co),
            );
            return;
        }

        let name_str = c_string_to_str(&character_name);

        log::info!("Usurping {} ({} , t={})", name_str, co, co_temp);

        // Get player number from cn
        let nr = gs.characters[cn].player;
        let was_already_usurping = gs.characters[cn].flags & CharacterFlags::Usurp.bits() != 0;
        let caller_is_player = gs.characters[cn].flags & CharacterFlags::Player.bits() != 0;
        let should_set_afk =
            caller_is_player && gs.characters[cn].data[core::constants::CHD_AFK] == 0;

        gs.characters[co].flags |= CharacterFlags::Usurp.bits();
        gs.characters[co].player = nr;

        if let Some(player) = gs.players.get_mut(nr as usize) {
            player.usnr = co;
        }

        if was_already_usurping {
            gs.characters[co].data[97] = gs.characters[cn].data[97];
            gs.characters[cn].data[97] = 0;
        } else {
            gs.characters[co].data[97] = cn as i32;
            gs.characters[cn].flags |= CharacterFlags::ComputerControlledPlayer.bits();
        }

        if caller_is_player {
            gs.characters[cn].tavern_x = gs.characters[cn].x as u16;
            gs.characters[cn].tavern_y = gs.characters[cn].y as u16;
            God::transfer_char(gs, cn, 10, 10);
            if should_set_afk {
                gs.do_afk(cn, "");
            }
        }

        player::plr_logout(gs, cn, nr as usize, LogoutReason::Usurp);
        gs.characters[co].set_do_update_flags();
    }

    /// Exit usurpation mode and restore the original player character.
    ///
    /// # Arguments
    /// * `cn` - Character exiting usurp mode
    pub fn exit_usurp(gs: &mut GameState, cn: usize) {
        if !Character::is_sane_character(cn) {
            return;
        }

        gs.characters[cn].flags &= !(CharacterFlags::Usurp.bits()
            | CharacterFlags::Staff.bits()
            | CharacterFlags::Immortal.bits()
            | CharacterFlags::God.bits()
            | CharacterFlags::Creator.bits());
        let co = gs.characters[cn].data[97] as usize;

        if Character::is_sane_character(co) {
            gs.characters[co].flags &= !CharacterFlags::ComputerControlledPlayer.bits();
            let nr = gs.characters[cn].player;
            gs.characters[co].player = nr;

            if let Some(player) = gs.players.get_mut(nr as usize) {
                player.usnr = co;
            }

            God::transfer_char(gs, co, 512, 512);
            gs.do_afk(co, "");
            gs.characters[cn].set_do_update_flags();
        }
    }

    /// Spawn a Grolm NPC near the caller for testing or event purposes.
    pub fn grolm(gs: &mut GameState, cn: usize) {
        if !Character::is_sane_character(cn) {
            return;
        }

        // Create character from template 386 with items
        if let Some(co) = populate::pop_create_char(gs, 386, true) {
            let character_name = gs.characters[co].name;

            let name_str = c_string_to_str(&character_name);

            log::info!("IMP: {} is now playing {} ({})", cn, name_str, co);

            Self::usurp(gs, cn, co);
        }
    }

    /// Show internal debug/state information for the Grolm NPC.
    pub fn grolm_info(gs: &mut GameState, cn: usize) {
        if !Character::is_sane_character(cn) {
            return;
        }

        // Find character with template 498 (Grolmy)
        let co = {
            let mut found = core::constants::MAXCHARS;
            for idx in 1..core::constants::MAXCHARS {
                if gs.characters[idx].temp == 498 {
                    found = idx;
                    break;
                }
            }
            found
        };

        // Check if found, active, and not a corpse
        let (is_valid, data_22, data_40, data_23) = if co == core::constants::MAXCHARS {
            (false, 0, 0, 0)
        } else {
            let character = &gs.characters[co];
            let is_valid = character.used == core::constants::USE_ACTIVE
                && (character.flags & CharacterFlags::Body.bits()) == 0;
            (
                is_valid,
                character.data[22],
                character.data[40],
                character.data[23],
            )
        };

        if !is_valid || co == core::constants::MAXCHARS {
            gs.do_character_log(cn, core::types::FontColor::Yellow, "Grolmy is dead.\n");
            return;
        }

        // Display state info
        let state_name = match data_22 {
            0 => "at_home",
            1 => "moving_out",
            2 => "moving_in",
            _ => "unknown",
        };

        let ticker = gs.globals.ticker;
        let timer_minutes = (ticker - data_23) as f64 / (core::constants::TICKS as f64 * 60.0);

        gs.do_character_log(
            cn,
            core::types::FontColor::Green,
            &format!(
                "Current state={}, runs={}, timer={:.2}m, id={}.\n",
                state_name, data_40, timer_minutes, co
            ),
        );
    }

    /// Start scripted movement or behaviour for the Grolm NPC.
    pub fn grolm_start(gs: &mut GameState, cn: usize) {
        if !Character::is_sane_character(cn) {
            return;
        }

        // Find character with template 498 (Grolmy)
        let co = {
            let mut found = core::constants::MAXCHARS;
            for idx in 1..core::constants::MAXCHARS {
                if gs.characters[idx].temp == 498 {
                    found = idx;
                    break;
                }
            }
            found
        };

        // Check if found, active, and not a corpse
        let (is_valid, data_22) = if co == core::constants::MAXCHARS {
            (false, 0)
        } else {
            let character = &gs.characters[co];
            let is_valid = character.used == core::constants::USE_ACTIVE
                && (character.flags & CharacterFlags::Body.bits()) == 0;
            (is_valid, character.data[22])
        };

        if !is_valid || co == core::constants::MAXCHARS {
            gs.do_character_log(cn, core::types::FontColor::Yellow, "Grolmy is dead.\n");
            return;
        }

        // Check if already moving
        if data_22 != 0 {
            gs.do_character_log(
                cn,
                core::types::FontColor::Yellow,
                "Grolmy is already moving.\n",
            );
            return;
        }

        // Start movement
        gs.characters[co].data[22] = 1;
    }

    /// Spawn a Gargoyle NPC near the caller for testing or event purposes.
    pub fn gargoyle(gs: &mut GameState, cn: usize) {
        if !Character::is_sane_character(cn) {
            return;
        }

        // Create character from template 495 with items
        if let Some(co) = populate::pop_create_char(gs, 495, true) {
            let character_name = gs.characters[co].name;

            let name_str = c_string_to_str(&character_name);

            log::info!("IMP: {} is now playing {} ({})", cn, name_str, co);

            Self::usurp(gs, cn, co);
        }
    }

    /// Perform a minor race/template change on the caller while preserving
    /// key attributes.
    pub fn minor_racechange(gs: &mut GameState, cn: usize, temp: i32) {
        if !Character::is_sane_character(cn) {
            return;
        }

        if temp < 0 || temp >= core::constants::MAXTCHARS as i32 {
            log::error!("Invalid character template: {}", temp);
            return;
        }

        let template = gs.character_templates[temp as usize];

        if template.used == core::constants::USE_EMPTY {
            log::error!("Template {} is not in use", temp);
            return;
        }

        {
            let character = &mut gs.characters[cn];

            character.hp[1] = template.hp[1];
            character.hp[2] = template.hp[2];
            character.hp[3] = template.hp[3];
            character.end[1] = template.end[1];
            character.end[2] = template.end[2];
            character.end[3] = template.end[3];
            character.mana[1] = template.mana[1];
            character.mana[2] = template.mana[2];
            character.mana[3] = template.mana[3];
            character.sprite = template.sprite;

            if character.kindred & (traits::KIN_PURPLE as i32) != 0 {
                character.kindred = template.kindred | (traits::KIN_PURPLE as i32);
            } else {
                character.kindred = template.kindred;
            }

            character.temp = temp as u16;
            character.weapon_bonus = template.weapon_bonus;
            character.armor_bonus = template.armor_bonus;
            character.gethit_bonus = template.gethit_bonus;

            for n in 0..5 {
                character.attrib[n][1] = template.attrib[n][1];
                character.attrib[n][2] = template.attrib[n][2];
                character.attrib[n][3] = template.attrib[n][3];
            }

            for n in 0..50 {
                if character.skill[n][0] == 0 && template.skill[n][0] != 0 {
                    character.skill[n][0] = template.skill[n][0];
                    log::info!("added skill {} to {}", n, character.get_name());
                }
                character.skill[n][1] = template.skill[n][1];
                character.skill[n][2] = template.skill[n][2];
                character.skill[n][3] = template.skill[n][3];
            }

            character.data[45] = 0;
            character.set_do_update_flags();
        }

        gs.do_check_new_level(cn);
    }

    /// Force a target to say text as if they had typed it.
    ///
    /// Administrative command used to make NPCs or players speak.
    ///
    /// # Arguments
    /// * `cn` - Requesting character
    /// * `whom` - Target specification
    /// * `text` - Text to force
    pub fn force(gs: &mut GameState, cn: usize, whom: &str, text: &str) {
        // Check cn <= 0
        if cn == 0 {
            return;
        }

        // Check if whom is empty
        if whom.is_empty() {
            gs.do_character_log(cn, core::types::FontColor::Red, "#FORCE whom?\n");
            return;
        }

        // Find the character
        let co = Self::find_next_char(gs, 1, whom, "");

        if co <= 0 {
            gs.do_character_log(cn, core::types::FontColor::Red, "No such character.\n");
            return;
        }

        let co_usize = co as usize;

        // Check if character is used
        let is_used = gs.characters[co_usize].used == core::constants::USE_ACTIVE;
        let is_player = gs.characters[co_usize].flags & CharacterFlags::Player.bits() != 0;
        let character_name = gs.characters[co_usize].name;

        if !is_used {
            gs.do_character_log(
                cn,
                core::types::FontColor::Red,
                "Character is not active.\n",
            );
            return;
        }

        // Check if trying to force a player when not a god
        let is_cn_god = gs.characters[cn].flags & CharacterFlags::God.bits() != 0;

        if is_player && !is_cn_god {
            gs.do_character_log(
                cn,
                core::types::FontColor::Red,
                "Not allowed to #FORCE players.\n",
            );
            return;
        }

        // Check if text is empty
        if text.is_empty() {
            let name_str = c_string_to_str(&character_name);

            gs.do_character_log(
                cn,
                core::types::FontColor::Red,
                &format!("#FORCE {} to what?\n", name_str),
            );
            return;
        }

        let name_str = c_string_to_str(&character_name);

        log::info!("IMP: {} forced {} ({}) to \"{}\"", cn, name_str, co, text);

        // Make the character say the text
        gs.do_sayx(co_usize, text);

        // Show success message
        gs.do_character_log(
            cn,
            core::types::FontColor::Green,
            &format!("{} was forced.\n", name_str),
        );
    }

    /// Check whether an IP address is present in the ban list.
    ///
    /// # Arguments
    /// * `addr` - IPv4 address as integer
    pub fn is_banned(gs: &mut GameState, addr: i32) -> bool {
        let addr = addr as u32;

        for ban in gs.ban_list.iter() {
            if ban.address() == addr {
                return true;
            }
        }

        false
    }

    /// Add a single ban entry for a specific address.
    ///
    /// Records the issuer `cn` and optionally the victim `co`.
    pub fn add_single_ban(gs: &mut GameState, cn: usize, co: usize, addr: u32) {
        if !Character::is_sane_character(cn) || !Character::is_sane_character(co) {
            return;
        }

        let creator_name = gs.characters[cn].get_name().to_string();
        let victim_name = gs.characters[co].get_name().to_string();

        if gs.ban_list.len() >= 250 {
            gs.do_character_log(cn, core::types::FontColor::Red, "Ban list is full\n");
            return;
        }

        let mut ban = core::types::Ban::new();
        ban.set_address(addr);
        ban.set_creator(&creator_name);
        ban.set_victim(&victim_name);

        gs.ban_list.push(ban);

        gs.do_character_log(
            cn,
            core::types::FontColor::Green,
            &format!("Added ban for address {} by {}\n", addr, creator_name),
        );
    }

    /// Ban the current address of the target character `co`.
    pub fn add_ban(gs: &mut GameState, cn: usize, co: usize) {
        if !Character::is_sane_character(cn) || !Character::is_sane_character(co) {
            return;
        }

        // Get player address - would need actual connection info
        // For now using placeholder logic
        let addr = 0u32; // TODO: Get actual player IP address

        Self::add_single_ban(gs, cn, co, addr);
    }

    /// Delete a ban list entry by its index `nr`.
    pub fn del_ban(gs: &mut GameState, cn: usize, nr: usize) {
        if !Character::is_sane_character(cn) {
            return;
        }

        if nr >= gs.ban_list.len() {
            gs.do_character_log(
                cn,
                core::types::FontColor::Red,
                &format!("Invalid ban number: {}\n", nr),
            );
            return;
        }

        gs.ban_list.remove(nr);

        gs.do_character_log(
            cn,
            core::types::FontColor::Green,
            &format!("Removed ban entry {}\n", nr),
        );
    }

    /// List all active ban entries to the requesting character.
    pub fn list_ban(gs: &mut GameState, cn: usize) {
        if !Character::is_sane_character(cn) {
            return;
        }

        gs.do_character_log(
            cn,
            core::types::FontColor::Green,
            &format!("Ban list ({} entries):\n", gs.ban_list.len()),
        );
        let entries: Vec<String> = gs
            .ban_list
            .iter()
            .enumerate()
            .map(|(i, ban)| {
                format!(
                    "  {}: Address={}, Creator={}, Victim={}\n",
                    i,
                    ban.address(),
                    ban.creator(),
                    ban.victim()
                )
            })
            .collect();

        for entry in entries {
            gs.do_character_log(cn, core::types::FontColor::Green, &entry);
        }
    }

    /// Mute a character `co` so they cannot speak publicly.
    pub fn shutup(gs: &mut GameState, cn: usize, co: usize) {
        if !Character::is_sane_character(cn) || !Character::is_sane_character(co) {
            return;
        }

        let (target_name, was_shut_up) = {
            let target = &mut gs.characters[co];
            let was_shut_up = target.flags & CharacterFlags::ShutUp.bits() != 0;
            if was_shut_up {
                target.flags &= !CharacterFlags::ShutUp.bits();
            } else {
                target.flags |= CharacterFlags::ShutUp.bits();
            }
            target.set_do_update_flags();
            (target.get_name().to_string(), was_shut_up)
        };

        let msg = if was_shut_up {
            format!("Removed shutup from character {}\n", target_name)
        } else {
            format!("Added shutup to character {}\n", target_name)
        };
        gs.do_character_log(cn, core::types::FontColor::Green, &msg);
    }

    /// Display basic network timing for a character.
    ///
    /// Note: the current protocol does not provide a true RTT measurement.
    /// The client periodically sends its own tick counter (`CL_CMD_CTICK`),
    /// and the server maintains a per-connection tick counter (`ltick`).
    /// The difference `ltick - rtick` is a *tick lag* / backlog indicator
    /// (how far behind the client is), which we report as an approximate
    /// latency in milliseconds.
    pub fn show_network_info(gs: &mut GameState, cn: usize, target: &str) {
        if !Character::is_sane_character(cn) {
            return;
        }

        // Determine target character
        let target_cn = if target.is_empty() {
            // No target specified, show info for self
            cn
        } else if target.chars().all(|c| c.is_ascii_digit()) {
            // Target is a number, parse it as character ID
            match target.parse::<usize>() {
                Ok(id) => id,
                Err(_) => {
                    gs.do_character_log(
                        cn,
                        core::types::FontColor::Red,
                        "Invalid character number.\n",
                    );
                    return;
                }
            }
        } else {
            // Target is a name, search for matching player
            let target_lower = target.to_lowercase();
            let found = {
                let mut out = None;
                for co in 1..core::constants::MAXCHARS {
                    let ch = &gs.characters[co];
                    if ch.used == core::constants::USE_EMPTY {
                        continue;
                    }
                    if (ch.flags & CharacterFlags::Player.bits()) == 0 {
                        continue;
                    }
                    let name = ch.get_name().to_lowercase();
                    if name.contains(&target_lower) {
                        out = Some(co);
                        break;
                    }
                }
                out
            };

            match found {
                Some(co) => co,
                None => {
                    gs.do_character_log(
                        cn,
                        core::types::FontColor::Red,
                        &format!("No player found matching '{}'.\n", target),
                    );
                    return;
                }
            }
        };

        if !Character::is_sane_character(target_cn) {
            gs.do_character_log(
                cn,
                core::types::FontColor::Red,
                "Invalid character number.\n",
            );
            return;
        }

        let target_name = gs.characters[target_cn].get_name().to_string();
        let player_id = gs.characters[target_cn].player;
        let is_player = (gs.characters[target_cn].flags & CharacterFlags::Player.bits()) != 0;

        if !is_player {
            gs.do_character_log(
                cn,
                core::types::FontColor::Red,
                "Target is not a player character.\n",
            );
            return;
        }

        if player_id <= 0 || player_id >= core::constants::MAXPLAYER as i32 {
            gs.do_character_log(
                cn,
                core::types::FontColor::Red,
                "Character has no active player connection.\n",
            );
            return;
        }

        let player_id = player_id as usize;

        // Get timing counters from the player slot.
        // - `ltick` is server-maintained (increments each server tick)
        // - `rtick` is client-maintained (sent via CL_CMD_CTICK)
        let (ltick, rtick) = (gs.players[player_id].ltick, gs.players[player_id].rtick);

        // `rtick` starts at 0 and only updates when we receive CTICK.
        // Until then, we can't compute a meaningful lag.
        let lag_ms: Option<f64> = if rtick == 0 {
            None
        } else {
            let lag_ticks = ltick.wrapping_sub(rtick);
            Some((lag_ticks as f64 * 1000.0) / core::constants::TICKS as f64)
        };

        gs.do_character_log(
            cn,
            core::types::FontColor::Yellow,
            "Name               Lag(ms)\n",
        );
        gs.do_character_log(
            cn,
            core::types::FontColor::Yellow,
            "-------------------------\n",
        );

        let lag_str = match lag_ms {
            Some(ms) => format!("{ms:>7.0}"),
            None => "   n/a".to_string(),
        };

        gs.do_character_log(
            cn,
            core::types::FontColor::Yellow,
            &format!("{:<18} {}\n", target_name, lag_str),
        );
    }

    /// Display basic network timing for all connected player characters.
    ///
    /// This is intended for god/imp usage; it enumerates active player
    /// connections and prints the same columns as `show_network_info`.
    pub fn show_network_info_all(gs: &mut GameState, cn: usize) {
        if !Character::is_sane_character(cn) {
            return;
        }

        // Snapshot active player slots first, then resolve character names.
        // We treat a slot as "connected" when it has an open socket and a
        // sane controlled character (`usnr`).
        let connected: Vec<(usize, usize, u32, u32)> = {
            let mut v = Vec::new();
            for player_id in 1..core::constants::MAXPLAYER {
                if gs.players[player_id].sock.is_none() {
                    continue;
                }
                let usnr = gs.players[player_id].usnr;
                if usnr == 0 || !Character::is_sane_character(usnr) {
                    continue;
                }
                v.push((
                    player_id,
                    usnr,
                    gs.players[player_id].ltick,
                    gs.players[player_id].rtick,
                ));
            }
            v
        };

        let rows: Vec<(String, Option<f64>)> = {
            let mut out = Vec::new();
            for (_player_id, usnr, ltick, rtick) in connected.iter().copied() {
                if gs.characters[usnr].used == core::constants::USE_EMPTY {
                    continue;
                }
                if (gs.characters[usnr].flags & CharacterFlags::Player.bits()) == 0 {
                    continue;
                }

                let lag_ms: Option<f64> = if rtick == 0 {
                    None
                } else {
                    let lag_ticks = ltick.wrapping_sub(rtick);
                    Some((lag_ticks as f64 * 1000.0) / core::constants::TICKS as f64)
                };

                out.push((gs.characters[usnr].get_name().to_string(), lag_ms));
            }
            out
        };

        gs.do_character_log(
            cn,
            core::types::FontColor::Yellow,
            "Name               Lag(ms)\n",
        );
        gs.do_character_log(
            cn,
            core::types::FontColor::Yellow,
            "-------------------------\n",
        );

        for (name, lag_ms) in rows {
            let lag_str = match lag_ms {
                Some(ms) => format!("{ms:>7.0}"),
                None => "   n/a".to_string(),
            };
            gs.do_character_log(
                cn,
                core::types::FontColor::Yellow,
                &format!("{:<18} {}\n", name, lag_str),
            );
        }
    }
}
