pub struct God {}
impl God {
    pub fn create_item(
        items: &mut [core::types::Item; core::constants::MAXITEM as usize],
        item_templates: &[core::types::Item; core::constants::MAXTITEM as usize],
        template_id: usize,
    ) -> Option<usize> {
        if !core::types::Item::is_sane_item_template(template_id) {
            return None;
        }

        if item_templates[template_id].used == core::constants::USE_EMPTY {
            log::error!(
                "Attempted to create item with an unused template ID: {}",
                template_id
            );
            return None;
        }

        if item_templates[template_id].is_unique() {
            // Check if the unique item already exists
            for item in items.iter() {
                if item.used != core::constants::USE_EMPTY && item.temp as usize == template_id {
                    log::error!(
                        "Attempted to create unique item with template ID {} but it already exists.",
                        template_id
                    );
                    return None;
                }
            }
        }

        let free_item_id = Self::get_free_item(items).unwrap_or_else(|| {
            log::error!("No free item slots available to create new item.");
            0
        });

        items[free_item_id] = item_templates[template_id].clone();
        items[free_item_id].temp = template_id as u16;

        Some(free_item_id)
    }

    // TODO: Optimize this later
    fn get_free_item(
        items: &[core::types::Item; core::constants::MAXITEM as usize],
    ) -> Option<usize> {
        for i in 1..core::constants::MAXITEM as usize {
            if items[i].used == core::constants::USE_EMPTY {
                return Some(i);
            }
        }
        None
    }

    pub fn give_character_item(
        character: &mut core::types::Character,
        item: &mut core::types::Item,
        char_id: usize,
        item_id: usize,
    ) -> bool {
        if !core::types::Item::is_sane_item(item_id) || !character.is_living_character(char_id) {
            log::error!(
                "Invalid item ID {} or character ID {} when giving item.",
                item_id,
                char_id
            );

            log::error!(
                "Attempting to given item '{}' to character '{}'",
                item.get_name(),
                character.get_name(),
            );
            return false;
        }

        if let Some(slot) = character.get_next_inventory_slot() {
            character.item[slot] = item_id as u32;
            item.x = 0;
            item.y = 0;
            item.carried = char_id as u16;

            character.set_do_update_flags();

            true
        } else {
            log::error!(
                "No free inventory slots available for character '{}' (ID {}).",
                character.get_name(),
                char_id
            );

            false
        }
    }

    pub fn build(character: &mut core::types::Character, character_id: usize, build_type: u8) {
        if !character.is_building() {
        } else if build_type != 0 {
        } else {
        }
    }

    pub fn build_equip(character: &mut core::types::Character, build_type: u32) {
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

        // Fill inventory with remaining items from templates
        // Note: This requires access to item templates which would need to be passed in
        // For now, implementing just the direct sprite assignments as per C++ function

        log::info!(
            "Build mode {} set for character {}",
            build_type,
            character.get_name()
        );
    }
}
