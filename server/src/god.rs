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
}
