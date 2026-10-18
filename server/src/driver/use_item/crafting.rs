use super::take_item_from_char;
use crate::chlog;
use crate::game_state::GameState;
use crate::god::God;
use crate::helpers::{self};
use core::constants::{
    AT_STREN, IT_AGILITY_POTION, IT_BLACK_POTION, IT_BLUE_AND_GREEN_POTION, IT_BLUE_FLOWER,
    IT_BLUE_POTION, IT_BLUE_RED_AND_GREEN_POTION, IT_BLUE_YELLOW_AND_GREEN_POTION, IT_FLASK,
    IT_GREEN_FLOWER, IT_GREEN_POTION, IT_HEALING_POTION, IT_JUNGLE_FLOWER_BLUE,
    IT_JUNGLE_FLOWER_PINK, IT_JUNGLE_FLOWER_YELLOW, IT_MANA_POTION, IT_ORANGE_POTION,
    IT_POTION_OF_LIFE, IT_PURPLE_FLOWER, IT_RED_AND_GREEN_POTION, IT_RED_FLOWER,
    IT_RED_YELLOW_AND_GREEN_POTION, IT_YELLOW_AND_GREEN_POTION, IT_YELLOW_FLOWER, IT_YELLOW_POTION,
    IT_YELLOW_TULIP, IT_YELLOW_TULIP_POTION, ItemFlags, MAXTITEM, USE_EMPTY,
};
use core::skills::{self};
use core::string_operations::c_string_to_str;

/// Handles the legacy `use_mix_potion` item-use hook.
///
/// # Arguments
///
/// * `gs` - Active game state used by this legacy driver hook.
/// * `cn` - Character index executing this legacy driver hook.
/// * `item_idx` - Value passed through to the legacy `use_mix_potion` hook.
///
/// # Returns
///
/// * `true` when `use_mix_potion` reports that it handled the legacy action, otherwise `false`.
///
/// # Panics
///
/// * Panics if any legacy id or index parameter used by `use_mix_potion` is outside the corresponding game-state collection.
pub fn use_mix_potion(gs: &mut GameState, cn: usize, item_idx: usize) -> bool {
    if cn == 0 {
        return false;
    }

    let citem = gs.characters[cn].citem as usize;

    if citem == 0 || (citem & 0x80000000) != 0 {
        gs.do_character_log(
            cn,
            core::types::FontColor::Blue,
            "What do you want to do with it?",
        );
        return false;
    }

    let carried = gs.items[item_idx].carried;
    if carried == 0 {
        gs.do_character_log(
            cn,
            core::types::FontColor::Blue,
            "Too difficult to do on the ground.\n",
        );
        return false;
    }

    let (base_temp, ingredient_temp) = (gs.items[item_idx].temp, gs.items[citem].temp);

    let result_template: Option<usize> = match base_temp as usize {
        // TODO: Evaluate usage of usize vs. u16 vs. u32, etc., etc.
        IT_FLASK => match ingredient_temp as usize {
            IT_RED_FLOWER => Some(IT_HEALING_POTION),
            IT_PURPLE_FLOWER => Some(IT_MANA_POTION),
            IT_BLUE_FLOWER => Some(IT_BLUE_POTION),
            IT_YELLOW_FLOWER => Some(IT_YELLOW_POTION),
            IT_GREEN_FLOWER => Some(IT_GREEN_POTION),
            IT_JUNGLE_FLOWER_PINK => Some(IT_RED_AND_GREEN_POTION),
            IT_JUNGLE_FLOWER_BLUE => Some(IT_BLUE_AND_GREEN_POTION),
            IT_JUNGLE_FLOWER_YELLOW => Some(IT_YELLOW_AND_GREEN_POTION),
            IT_YELLOW_TULIP => Some(IT_YELLOW_TULIP_POTION),
            _ => None,
        },
        IT_GREEN_POTION | IT_BLUE_POTION | IT_BLACK_POTION => match ingredient_temp as usize {
            IT_RED_FLOWER
            | IT_PURPLE_FLOWER
            | IT_YELLOW_FLOWER
            | IT_BLUE_FLOWER
            | IT_GREEN_FLOWER
            | IT_JUNGLE_FLOWER_PINK
            | IT_JUNGLE_FLOWER_BLUE
            | IT_JUNGLE_FLOWER_YELLOW
            | IT_YELLOW_TULIP => Some(IT_BLACK_POTION),
            _ => None,
        },
        IT_YELLOW_POTION => match ingredient_temp as usize {
            IT_RED_FLOWER
            | IT_PURPLE_FLOWER
            | IT_YELLOW_FLOWER
            | IT_BLUE_FLOWER
            | IT_JUNGLE_FLOWER_PINK
            | IT_JUNGLE_FLOWER_BLUE
            | IT_JUNGLE_FLOWER_YELLOW
            | IT_YELLOW_TULIP => Some(IT_BLACK_POTION),
            IT_GREEN_FLOWER => Some(IT_ORANGE_POTION),
            _ => None,
        },
        IT_ORANGE_POTION => match ingredient_temp as usize {
            IT_RED_FLOWER
            | IT_PURPLE_FLOWER
            | IT_YELLOW_FLOWER
            | IT_GREEN_FLOWER
            | IT_JUNGLE_FLOWER_PINK
            | IT_JUNGLE_FLOWER_BLUE
            | IT_JUNGLE_FLOWER_YELLOW
            | IT_YELLOW_TULIP => Some(IT_BLACK_POTION),
            IT_BLUE_FLOWER => Some(IT_POTION_OF_LIFE),
            _ => None,
        },
        IT_YELLOW_AND_GREEN_POTION => match ingredient_temp as usize {
            IT_RED_FLOWER
            | IT_PURPLE_FLOWER
            | IT_BLUE_FLOWER
            | IT_YELLOW_FLOWER
            | IT_GREEN_FLOWER
            | IT_JUNGLE_FLOWER_YELLOW
            | IT_YELLOW_TULIP => Some(IT_BLACK_POTION),
            IT_JUNGLE_FLOWER_PINK => Some(IT_RED_YELLOW_AND_GREEN_POTION),
            IT_JUNGLE_FLOWER_BLUE => Some(IT_BLUE_YELLOW_AND_GREEN_POTION),
            _ => None,
        },
        IT_RED_AND_GREEN_POTION => match ingredient_temp as usize {
            IT_RED_FLOWER
            | IT_PURPLE_FLOWER
            | IT_BLUE_FLOWER
            | IT_YELLOW_FLOWER
            | IT_GREEN_FLOWER
            | IT_JUNGLE_FLOWER_PINK
            | IT_YELLOW_TULIP => Some(IT_BLACK_POTION),
            IT_JUNGLE_FLOWER_BLUE => Some(IT_BLUE_RED_AND_GREEN_POTION),
            IT_JUNGLE_FLOWER_YELLOW => Some(IT_RED_YELLOW_AND_GREEN_POTION),
            _ => None,
        },
        IT_BLUE_AND_GREEN_POTION => match ingredient_temp as usize {
            IT_RED_FLOWER
            | IT_PURPLE_FLOWER
            | IT_BLUE_FLOWER
            | IT_YELLOW_FLOWER
            | IT_GREEN_FLOWER
            | IT_JUNGLE_FLOWER_BLUE
            | IT_YELLOW_TULIP => Some(IT_BLACK_POTION),
            IT_JUNGLE_FLOWER_PINK => Some(IT_BLUE_RED_AND_GREEN_POTION),
            IT_JUNGLE_FLOWER_YELLOW => Some(IT_BLUE_YELLOW_AND_GREEN_POTION),
            _ => None,
        },
        IT_BLUE_YELLOW_AND_GREEN_POTION => match ingredient_temp as usize {
            IT_RED_FLOWER
            | IT_PURPLE_FLOWER
            | IT_BLUE_FLOWER
            | IT_YELLOW_FLOWER
            | IT_GREEN_FLOWER
            | IT_JUNGLE_FLOWER_BLUE
            | IT_JUNGLE_FLOWER_YELLOW
            | IT_YELLOW_TULIP => Some(IT_BLACK_POTION),
            IT_JUNGLE_FLOWER_PINK => Some(IT_AGILITY_POTION),
            _ => None,
        },
        IT_BLUE_RED_AND_GREEN_POTION => match ingredient_temp as usize {
            IT_RED_FLOWER
            | IT_PURPLE_FLOWER
            | IT_BLUE_FLOWER
            | IT_YELLOW_FLOWER
            | IT_GREEN_FLOWER
            | IT_JUNGLE_FLOWER_PINK
            | IT_JUNGLE_FLOWER_BLUE
            | IT_YELLOW_TULIP => Some(IT_BLACK_POTION),
            IT_JUNGLE_FLOWER_YELLOW => Some(IT_AGILITY_POTION),
            _ => None,
        },
        IT_RED_YELLOW_AND_GREEN_POTION => match ingredient_temp as usize {
            IT_RED_FLOWER
            | IT_PURPLE_FLOWER
            | IT_BLUE_FLOWER
            | IT_YELLOW_FLOWER
            | IT_GREEN_FLOWER
            | IT_JUNGLE_FLOWER_PINK
            | IT_JUNGLE_FLOWER_YELLOW
            | IT_YELLOW_TULIP => Some(IT_BLACK_POTION),
            IT_JUNGLE_FLOWER_BLUE => Some(IT_AGILITY_POTION),
            _ => None,
        },
        IT_YELLOW_TULIP_POTION => match ingredient_temp as usize {
            IT_RED_FLOWER
            | IT_PURPLE_FLOWER
            | IT_BLUE_FLOWER
            | IT_YELLOW_FLOWER
            | IT_GREEN_FLOWER
            | IT_JUNGLE_FLOWER_PINK
            | IT_JUNGLE_FLOWER_BLUE
            | IT_JUNGLE_FLOWER_YELLOW
            | IT_YELLOW_TULIP => Some(IT_BLACK_POTION),
            _ => None,
        },
        _ => None,
    };

    let result_template = match result_template {
        Some(t) => t,
        None => {
            gs.do_character_log(cn, core::types::FontColor::Blue, "Sorry?\n");
            return false;
        }
    };

    let in3 = match God::create_item(gs, result_template) {
        Some(id) => id,
        None => return false,
    };

    gs.items[in3].flags |= ItemFlags::IF_UPDATE.bits();
    gs.items[citem].used = USE_EMPTY;
    gs.items[item_idx].used = USE_EMPTY;
    gs.characters[cn].citem = 0;

    take_item_from_char(gs, item_idx, cn);
    God::give_character_item(gs, cn, in3);

    true
}

/// Handles the legacy `use_chain` item-use hook.
///
/// # Arguments
///
/// * `gs` - Active game state used by this legacy driver hook.
/// * `cn` - Character index executing this legacy driver hook.
/// * `item_idx` - Value passed through to the legacy `use_chain` hook.
///
/// # Returns
///
/// * `true` when `use_chain` reports that it handled the legacy action, otherwise `false`.
///
/// # Panics
///
/// * Panics if any legacy id or index parameter used by `use_chain` is outside the corresponding game-state collection.
pub fn use_chain(gs: &mut GameState, cn: usize, item_idx: usize) -> bool {
    if cn == 0 {
        return false;
    }

    let citem = gs.characters[cn].citem as usize;

    if citem == 0 || (citem & 0x80000000) != 0 {
        gs.do_character_log(
            cn,
            core::types::FontColor::Blue,
            "What do you want to do with it?\n",
        );
        return false;
    }

    let carried = gs.items[item_idx].carried;
    if carried == 0 {
        gs.do_character_log(
            cn,
            core::types::FontColor::Blue,
            "Too difficult to do on the ground.\n",
        );
        return false;
    }

    let citem_temp = gs.items[citem].temp;
    if citem_temp != 206 {
        gs.do_character_log(cn, core::types::FontColor::Blue, "Sorry?\n");
        return false;
    }

    let (current_temp, max_data) = (
        i32::from(gs.items[item_idx].temp),
        gs.items[item_idx].data[0],
    );

    if current_temp as u32 >= max_data {
        gs.do_character_log(cn, core::types::FontColor::Blue, "It won't fit anymore.\n");
        return false;
    }

    let in3 = match God::create_item(gs, (current_temp + 1) as usize) {
        Some(id) => id,
        None => return false,
    };

    gs.items[in3].flags |= ItemFlags::IF_UPDATE.bits();
    gs.items[citem].used = USE_EMPTY;
    gs.items[item_idx].used = USE_EMPTY;
    gs.characters[cn].citem = 0;

    take_item_from_char(gs, item_idx, cn);
    God::give_character_item(gs, cn, in3);

    true
}

/// Handles the legacy `stone_sword` item-use hook.
///
/// # Arguments
///
/// * `gs` - Active game state used by this legacy driver hook.
/// * `cn` - Character index executing this legacy driver hook.
/// * `item_idx` - Value passed through to the legacy `stone_sword` hook.
///
/// # Returns
///
/// * `true` when `stone_sword` reports that it handled the legacy action, otherwise `false`.
///
/// # Panics
///
/// * Panics if any legacy id or index parameter used by `stone_sword` is outside the corresponding game-state collection.
pub fn stone_sword(gs: &mut GameState, cn: usize, item_idx: usize) -> bool {
    if cn == 0 {
        log::error!("stone_sword called with cn=0");
        return false;
    }

    let (active, template_id) = (
        gs.items[item_idx].active,
        gs.items[item_idx].data[0] as usize,
    );

    if active != 0 {
        log::error!("stone_sword called on active item");
        return false;
    }

    if template_id == 0 || template_id >= MAXTITEM {
        log::error!(
            "stone_sword called with invalid template_id: {}",
            template_id
        );
        return false;
    }

    // Check if character has enough strength (100+)
    let strength = gs.characters[cn].attrib[AT_STREN as usize][5];

    if strength < 100 {
        gs.do_character_log(
            cn,
            core::types::FontColor::Blue,
            "You're not strong enough.\n",
        );
        return false;
    }

    let in2 = match God::create_item(gs, template_id) {
        Some(id) => id,
        None => return false,
    };

    God::give_character_item(gs, cn, in2);

    let item_ref = c_string_to_str(&gs.items[in2].reference).to_owned();
    gs.do_character_log(
        cn,
        core::types::FontColor::Green,
        &format!("You got a {}.\n", item_ref),
    );

    true
}

/// Handles the legacy `rat_eye` item-use hook.
///
/// # Arguments
///
/// * `gs` - Active game state used by this legacy driver hook.
/// * `cn` - Character index executing this legacy driver hook.
/// * `item_idx` - Value passed through to the legacy `rat_eye` hook.
///
/// # Returns
///
/// * `true` when `rat_eye` reports that it handled the legacy action, otherwise `false`.
///
/// # Panics
///
/// * Panics if any legacy id or index parameter used by `rat_eye` is outside the corresponding game-state collection.
pub fn rat_eye(gs: &mut GameState, cn: usize, item_idx: usize) -> bool {
    if cn == 0 {
        return false;
    }

    let citem = gs.characters[cn].citem;

    if citem == 0 || (citem & 0x80000000) != 0 {
        gs.do_character_log(
            cn,
            core::types::FontColor::Green,
            "What do you want to do with it?\n",
        );
        return false;
    }

    // Check if rat eye is carried (not on ground)
    let carried = gs.items[item_idx].carried;
    if carried == 0 {
        gs.do_character_log(
            cn,
            core::types::FontColor::Green,
            "Too difficult to do on the ground.\n",
        );
        return false;
    }

    // Check if citem matches any of the required templates in data[0-8]
    let citem_temp = gs.items[citem as usize].temp;

    let mut slot = None;
    for n in 0..9 {
        let required_temp = gs.items[item_idx].data[n] as u16;
        if required_temp != 0 && required_temp == citem_temp {
            slot = Some(n);
            break;
        }
    }

    let slot = match slot {
        Some(s) => s,
        None => {
            gs.do_character_log(cn, core::types::FontColor::Green, "This doesnt fit.\n");
            return false;
        }
    };

    chlog!(cn, "added item to rat eye in slot {}", slot);
    log::info!("Character {} added item to rat eye", cn);

    // Mark the slot as filled
    {
        gs.items[item_idx].data[slot] = 0;
        gs.items[item_idx].sprite[0] += 1;
        gs.items[item_idx].flags |= ItemFlags::IF_UPDATE.bits();
        gs.items[item_idx].temp = 0;
    };

    // Remove the citem
    {
        gs.items[citem as usize].used = USE_EMPTY;
    };
    {
        gs.characters[cn].citem = 0;
    };

    // Check if all slots are filled
    let all_filled = (0..9).all(|n| gs.items[item_idx].data[n] == 0);

    if all_filled {
        // Create the final item from data[9]
        let result_template = gs.items[item_idx].data[9] as usize;

        let in3 = match God::create_item(gs, result_template) {
            Some(id) => id,
            None => return true,
        };

        {
            gs.items[in3].flags |= ItemFlags::IF_UPDATE.bits();
        };

        // Remove the rat eye item
        take_item_from_char(gs, item_idx, cn);
        {
            gs.items[item_idx].used = USE_EMPTY;
        };

        // Give the completed item to the character
        God::give_character_item(gs, cn, in3);
    }

    true
}

/// Handles the legacy `build_ring` item-use hook.
///
/// # Arguments
///
/// * `gs` - Active game state used by this legacy driver hook.
/// * `cn` - Character index executing this legacy driver hook.
/// * `item_idx` - Value passed through to the legacy `build_ring` hook.
///
/// # Returns
///
/// * `true` when `build_ring` reports that it handled the legacy action, otherwise `false`.
///
/// # Panics
///
/// * Panics if any legacy id or index parameter used by `build_ring` is outside the corresponding game-state collection.
pub fn build_ring(gs: &mut GameState, cn: usize, item_idx: usize) -> bool {
    use crate::god::God;
    use core::constants::{ItemFlags, USE_EMPTY};

    // Get ring base template
    let t1 = gs.items[item_idx].temp;

    // Get citem template
    let (in2, t2) = {
        let in2 = gs.characters[cn].citem as usize;
        if in2 == 0 {
            (0, 0)
        } else {
            let t2 = gs.items[in2].temp;
            (in2, t2)
        }
    };

    // Determine result template
    let r = if t1 == 360 && t2 == 0 {
        337 // plain golden ring
    } else if t1 == 361 && t2 == 0 {
        338 // plain silver ring
    } else if t1 == 337 {
        // golden ring with gem
        match t2 {
            339 => 362, // small ruby
            340 => 363, // med ruby
            341 => 364, // big ruby
            342 => 365, // small emerald
            343 => 366, // med emerald
            344 => 367, // big emerald
            345 => 368, // small saphire
            346 => 369, // med saphire
            347 => 370, // big saphire
            487 => 490, // huge ruby
            488 => 491, // huge emerald
            489 => 492, // huge saphire
            _ => return false,
        }
    } else if t1 == 338 {
        // silver ring with gem
        match t2 {
            339 => 348, // small ruby
            340 => 349, // med ruby
            341 => 350, // big ruby
            342 => 351, // small emerald
            343 => 352, // med emerald
            344 => 353, // big emerald
            345 => 354, // small saphire
            346 => 355, // med saphire
            347 => 356, // big saphire
            487..=489 => {
                // Huge gems too powerful for silver
                gs.do_character_log(
                    cn,
                    core::types::FontColor::Green,
                    "This stone is too powerful for a silver ring.\n",
                );
                return false;
            }
            _ => return false,
        }
    } else {
        return false;
    };

    // Create result item
    if let Some(in3) = God::create_item(gs, r) {
        {
            gs.items[in3].flags |= ItemFlags::IF_UPDATE.bits();
        };

        // Remove gem if used
        if in2 != 0 {
            {
                gs.characters[cn].citem = 0;
            };
            {
                gs.items[in2].used = USE_EMPTY;
            };
        }

        // Remove ring base
        take_item_from_char(gs, item_idx, cn);
        {
            gs.items[item_idx].used = USE_EMPTY;
        };

        // Give result to character
        God::give_character_item(gs, cn, in3);

        return true;
    }

    false
}

/// Handles the legacy `build_amulet` item-use hook.
///
/// # Arguments
///
/// * `gs` - Active game state used by this legacy driver hook.
/// * `cn` - Character index executing this legacy driver hook.
/// * `item_idx` - Value passed through to the legacy `build_amulet` hook.
///
/// # Returns
///
/// * `true` when `build_amulet` reports that it handled the legacy action, otherwise `false`.
///
/// # Panics
///
/// * Panics if any legacy id or index parameter used by `build_amulet` is outside the corresponding game-state collection.
pub fn build_amulet(gs: &mut GameState, cn: usize, item_idx: usize) -> bool {
    // Get amulet piece template
    let t1 = gs.items[item_idx].temp;

    // Get citem
    let in2 = gs.characters[cn].citem as usize;

    if in2 == 0 || (in2 & 0x80000000) != 0 {
        gs.do_character_log(cn, core::types::FontColor::Yellow, "Nothing happens.\n");
        return false;
    }

    let t2 = gs.items[in2].temp;

    // Determine result based on combination
    let r = if (t1 == 471 && t2 == 472) || (t1 == 472 && t2 == 471) {
        476
    } else if (t1 == 471 && t2 == 473) || (t1 == 473 && t2 == 471) {
        474
    } else if (t1 == 472 && t2 == 473) || (t1 == 473 && t2 == 472) {
        475
    } else if (t1 == 471 && t2 == 475)
        || (t1 == 475 && t2 == 471)
        || (t1 == 472 && t2 == 474)
        || (t1 == 474 && t2 == 472)
        || (t1 == 473 && t2 == 476)
        || (t1 == 476 && t2 == 473)
    {
        466
    } else {
        gs.do_character_log(cn, core::types::FontColor::Yellow, "That doesn't fit.\n");
        return false;
    };

    // Create result item
    if let Some(in3) = God::create_item(gs, r) {
        {
            gs.items[in3].flags |= ItemFlags::IF_UPDATE.bits();
        };

        // Remove components
        {
            gs.characters[cn].citem = 0;
        };
        {
            gs.items[in2].used = USE_EMPTY;
        };

        take_item_from_char(gs, item_idx, cn);
        {
            gs.items[item_idx].used = USE_EMPTY;
        };

        // Give result to character
        God::give_character_item(gs, cn, in3);

        return true;
    }

    false
}

/// Handles the legacy `use_soulstone` item-use hook.
///
/// # Arguments
///
/// * `gs` - Active game state used by this legacy driver hook.
/// * `cn` - Character index executing this legacy driver hook.
/// * `item_idx` - Value passed through to the legacy `use_soulstone` hook.
///
/// # Returns
///
/// * `true` when `use_soulstone` reports that it handled the legacy action, otherwise `false`.
///
/// # Panics
///
/// * Panics if any legacy id or index parameter used by `use_soulstone` is outside the corresponding game-state collection.
pub fn use_soulstone(gs: &mut GameState, cn: usize, item_idx: usize) -> bool {
    if !core::types::Character::is_sane_character(cn) {
        return false;
    }
    if !core::types::Item::is_sane_item(item_idx) {
        return false;
    }

    let citem = gs.characters[cn].citem;
    if citem == 0 {
        gs.do_character_log(
            cn,
            core::types::FontColor::Blue,
            "Try using something with the soulstone. That is, click on the stone with an item under your cursor.",
        );
        return false;
    }

    let in2 = citem as usize;
    if !core::types::Item::is_sane_item(in2) {
        return false;
    }

    // Check if the item is another soulstone (driver 68)
    let in2_driver = gs.items[in2].driver;
    if in2_driver == 68 {
        // Absorb the smaller stone into the larger one so click order can never
        // leave the result below the better of the two input stones.
        let (a, b) = (gs.items[item_idx].data[1], gs.items[in2].data[1]);
        let exp_gain = helpers::random_mod(a.min(b).saturating_add(1));
        gs.items[item_idx].data[1] = a.max(b).saturating_add(exp_gain);
        let rank = core::ranks::points2rank(gs.items[item_idx].data[1]);
        gs.items[item_idx].data[0] = rank;

        // Update description - read data value first to avoid packed field reference
        let data1_value = gs.items[item_idx].data[1];
        let description = format!("Level {} soulstone, holding {} exp.", rank, data1_value);
        gs.items[item_idx].description.fill(0);
        let bytes = description.as_bytes();
        let len = bytes.len().min(119);
        gs.items[item_idx].description[..len].copy_from_slice(&bytes[..len]);

        if rank > 20 {
            gs.do_character_log(
                cn,
                core::types::FontColor::Blue,
                "That's as high as they go.",
            );
        }

        soul_destroy(gs, cn, in2);
        return true;
    }

    let in2_temp = gs.items[in2].temp;

    // Handle different item types based on temp value
    match in2_temp as usize {
        IT_RED_FLOWER => {
            // Red flower -> healing potion
            soul_transform(gs, cn, item_idx, in2, 101);
            {
                gs.items[item_idx].hp[0] += 10;
            };
            true
        }
        IT_PURPLE_FLOWER => {
            // Purple flower -> mana potion
            soul_transform(gs, cn, item_idx, in2, 102);
            {
                gs.items[item_idx].mana[0] += 10;
            };
            true
        }
        91 => {
            // Torch -> repair
            let torch = soul_repair(gs, cn, item_idx, in2);
            gs.items[torch].max_age[1] *= 4;
            true
        }
        IT_FLASK => {
            soul_transform(gs, cn, item_idx, in2, 102);
            true
        }
        IT_HEALING_POTION => {
            soul_destroy(gs, cn, item_idx);
            {
                gs.items[in2].hp[0] += 10;
            };
            true
        }
        IT_MANA_POTION => {
            soul_destroy(gs, cn, item_idx);
            {
                gs.items[in2].mana[0] += 10;
            };
            true
        }
        // Equipment items that can be enhanced
        27..=44 | 51..=80 | 94..=99 | 116 | 125 | 158 | 501..=503 | 523..=524 | 813 | 981..=986 => {
            soul_trans_equipment(gs, cn, item_idx, in2);
            true
        }
        _ => {
            gs.do_character_log(cn, core::types::FontColor::Blue, "Nothing happened.\n");
            false
        }
    }
}

/// Transform soulstone and item into a new item
fn soul_transform(
    gs: &mut GameState,
    cn: usize,
    soulstone_idx: usize,
    item_idx: usize,
    new_temp: usize,
) -> usize {
    God::take_from_char(gs, soulstone_idx, cn);
    God::take_from_char(gs, item_idx, cn);

    gs.items[soulstone_idx].used = core::constants::USE_EMPTY;
    gs.items[item_idx].used = core::constants::USE_EMPTY;

    let new_item = God::create_item(gs, new_temp);
    if let Some(new_item_idx) = new_item {
        God::give_character_item(gs, cn, new_item_idx);
        new_item_idx
    } else {
        0
    }
}

/// Repair an item using soulstone
fn soul_repair(gs: &mut GameState, cn: usize, soulstone_idx: usize, item_idx: usize) -> usize {
    God::take_from_char(gs, soulstone_idx, cn);
    gs.items[soulstone_idx].used = core::constants::USE_EMPTY;

    let item_temp = gs.items[item_idx].temp as usize;
    gs.items[item_idx] = gs.item_templates[item_temp];
    gs.items[item_idx].carried = cn as u16;
    gs.items[item_idx].flags |= core::constants::ItemFlags::IF_UPDATE.bits();
    gs.items[item_idx].temp = 0;

    item_idx
}

/// Destroy an item and remove it from character
fn soul_destroy(gs: &mut GameState, cn: usize, item_idx: usize) {
    God::take_from_char(gs, item_idx, cn);
    gs.items[item_idx].used = core::constants::USE_EMPTY;
}

/// Transfer soulstone power to equipment
fn soul_trans_equipment(gs: &mut GameState, cn: usize, soulstone_idx: usize, item_idx: usize) {
    let mut rank = gs.items[soulstone_idx].data[0];

    let is_weapon =
        { (gs.items[soulstone_idx].flags & core::constants::ItemFlags::IF_WEAPON.bits()) != 0 };

    while rank > 0 {
        let stren = helpers::random_mod(rank.saturating_add(1));
        rank -= stren;

        let ran = if is_weapon {
            helpers::random_mod_usize(27)
        } else {
            helpers::random_mod_usize(26)
        };

        let item = &mut gs.items[item_idx];

        match ran {
            0 => {
                item.hp[2] = item.hp[2].saturating_add((stren * 25) as i16);
                item.hp[0] = item.hp[0].saturating_add((stren * 5) as i16);
            }
            1 => {
                item.mana[2] = item.mana[2].saturating_add((stren * 25) as i16);
                item.mana[0] = item.mana[0].saturating_add((stren * 5) as i16);
            }
            2..=6 => {
                let attr_idx = ran - 2;
                let current = item.attrib[attr_idx][2] as u32;
                item.attrib[attr_idx][2] = std::cmp::min(120, current + (stren * 3)) as i16;
                item.attrib[attr_idx][0] =
                    item.attrib[attr_idx][0].saturating_add((stren / 2) as i16);
            }
            7..=9 => {
                let current = item.skill[skills::SK_WEAPON][2] as u32;
                item.skill[skills::SK_WEAPON][2] = std::cmp::min(120, current + (stren * 5)) as i16;
                item.skill[skills::SK_WEAPON][0] =
                    item.skill[skills::SK_WEAPON][0].saturating_add(stren as i16);
            }
            10 => {
                let current = item.skill[skills::SK_STEALTH][2] as u32;
                item.skill[skills::SK_STEALTH][2] =
                    std::cmp::min(120, current + (stren * 5)) as i16;
                item.skill[skills::SK_STEALTH][0] =
                    item.skill[skills::SK_STEALTH][0].saturating_add(stren as i16);
            }
            11 => {
                let current = item.skill[skills::SK_MSHIELD][2] as u32;
                item.skill[skills::SK_MSHIELD][2] =
                    std::cmp::min(120, current + (stren * 5)) as i16;
                item.skill[skills::SK_MSHIELD][0] =
                    item.skill[skills::SK_MSHIELD][0].saturating_add(stren as i16);
            }
            12 => {
                let current = item.skill[skills::SK_PROTECT][2] as u32;
                item.skill[skills::SK_PROTECT][2] =
                    std::cmp::min(120, current + (stren * 5)) as i16;
                item.skill[skills::SK_PROTECT][0] =
                    item.skill[skills::SK_PROTECT][0].saturating_add(stren as i16);
            }
            13 => {
                let current = item.skill[skills::SK_ENHANCE][2] as u32;
                item.skill[skills::SK_ENHANCE][2] =
                    std::cmp::min(120, current + (stren * 5)) as i16;
                item.skill[skills::SK_ENHANCE][0] =
                    item.skill[skills::SK_ENHANCE][0].saturating_add(stren as i16);
            }
            14 => {
                let current = item.skill[skills::SK_STUN][2] as u32;
                item.skill[skills::SK_STUN][2] = std::cmp::min(120, current + (stren * 5)) as i16;
                item.skill[skills::SK_STUN][0] =
                    item.skill[skills::SK_STUN][0].saturating_add(stren as i16);
            }
            15 => {
                let current = item.skill[skills::SK_CURSE][2] as u32;
                item.skill[skills::SK_CURSE][2] = std::cmp::min(120, current + (stren * 5)) as i16;
                item.skill[skills::SK_CURSE][0] =
                    item.skill[skills::SK_CURSE][0].saturating_add(stren as i16);
            }
            16 => {
                let current = item.skill[skills::SK_BLESS][2] as u32;
                item.skill[skills::SK_BLESS][2] = std::cmp::min(120, current + (stren * 5)) as i16;
                item.skill[skills::SK_BLESS][0] =
                    item.skill[skills::SK_BLESS][0].saturating_add(stren as i16);
            }
            17 => {
                let current = item.skill[skills::SK_RESIST][2] as u32;
                item.skill[skills::SK_RESIST][2] = std::cmp::min(120, current + (stren * 5)) as i16;
                item.skill[skills::SK_RESIST][0] =
                    item.skill[skills::SK_RESIST][0].saturating_add(stren as i16);
            }
            18 => {
                let current = item.skill[skills::SK_BLAST][2] as u32;
                item.skill[skills::SK_BLAST][2] = std::cmp::min(120, current + (stren * 5)) as i16;
                item.skill[skills::SK_BLAST][0] =
                    item.skill[skills::SK_BLAST][0].saturating_add(stren as i16);
            }
            19 => {
                let current = item.skill[skills::SK_HEAL][2] as u32;
                item.skill[skills::SK_HEAL][2] = std::cmp::min(120, current + (stren * 5)) as i16;
                item.skill[skills::SK_HEAL][0] =
                    item.skill[skills::SK_HEAL][0].saturating_add(stren as i16);
            }
            20 => {
                let current = item.skill[skills::SK_GHOST][2] as u32;
                item.skill[skills::SK_GHOST][2] = std::cmp::min(120, current + (stren * 5)) as i16;
                item.skill[skills::SK_GHOST][0] =
                    item.skill[skills::SK_GHOST][0].saturating_add(stren as i16);
            }
            21 => {
                let current = item.skill[skills::SK_IMMUN][2] as u32;
                item.skill[skills::SK_IMMUN][2] = std::cmp::min(120, current + (stren * 5)) as i16;
                item.skill[skills::SK_IMMUN][0] =
                    item.skill[skills::SK_IMMUN][0].saturating_add(stren as i16);
            }
            22 => {
                let current = item.skill[skills::SK_SURROUND][2] as u32;
                item.skill[skills::SK_SURROUND][2] =
                    std::cmp::min(120, current + (stren * 5)) as i16;
                item.skill[skills::SK_SURROUND][0] =
                    item.skill[skills::SK_SURROUND][0].saturating_add(stren as i16);
            }
            23 => {
                let current = item.skill[skills::SK_CONCEN][2] as u32;
                item.skill[skills::SK_CONCEN][2] = std::cmp::min(120, current + (stren * 5)) as i16;
                item.skill[skills::SK_CONCEN][0] =
                    item.skill[skills::SK_CONCEN][0].saturating_add(stren as i16);
            }
            24 => {
                let current = item.skill[skills::SK_WARCRY][2] as u32;
                item.skill[skills::SK_WARCRY][2] = std::cmp::min(120, current + (stren * 5)) as i16;
                item.skill[skills::SK_WARCRY][0] =
                    item.skill[skills::SK_WARCRY][0].saturating_add(stren as i16);
            }
            25 => {
                item.armor[0] = item.armor[0].saturating_add((stren / 2) as i8);
            }
            26 => {
                item.weapon[0] = item.weapon[0].saturating_add((stren / 2) as i8);
            }
            _ => {
                log::error!("should never happen in soul_trans_equipment(gs, )");
            }
        }
    }

    // Finalize the enhancement
    let soulstone_rank = gs.items[soulstone_idx].data[0];
    gs.items[item_idx].temp = 0;
    gs.items[item_idx].flags |= core::constants::ItemFlags::IF_UPDATE.bits()
        | core::constants::ItemFlags::IF_IDENTIFIED.bits()
        | core::constants::ItemFlags::IF_NOREPAIR.bits()
        | core::constants::ItemFlags::IF_SOULSTONE.bits();

    gs.items[item_idx].min_rank = std::cmp::max(soulstone_rank as i8, gs.items[item_idx].min_rank);

    if gs.items[item_idx].max_damage == 0 {
        gs.items[item_idx].max_damage = 60000;
    }

    // Get item name before destruction
    let item_name = gs.items[item_idx].get_name().to_owned();

    // Update description
    let description = format!(
        "A {} enhanced by a rank {} soulstone.",
        item_name, soulstone_rank
    );
    gs.items[item_idx].description.fill(0);
    let bytes = description.as_bytes();
    let len = bytes.len().min(199);
    gs.items[item_idx].description[..len].copy_from_slice(&bytes[..len]);

    soul_destroy(gs, cn, soulstone_idx);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::{add_test_player, with_test_gs};
    use core::{
        constants::{AT_STREN, ItemFlags, USE_ACTIVE, USE_EMPTY},
        string_operations::write_ascii_into_fixed,
    };

    /// Seed the item template slot needed by `God::create_item`.
    fn seed_template(gs: &mut GameState, template_id: usize) {
        gs.item_templates[template_id] = core::types::Item::default();
        gs.item_templates[template_id].used = USE_ACTIVE;
        write_ascii_into_fixed(
            &mut gs.item_templates[template_id].name,
            &format!("template {template_id}"),
        );
        write_ascii_into_fixed(
            &mut gs.item_templates[template_id].reference,
            &format!("template {template_id}"),
        );
    }

    /// Place an item in a character inventory slot with the given template id.
    fn carried_item(
        gs: &mut GameState,
        cn: usize,
        item_idx: usize,
        template_id: usize,
        slot: usize,
    ) {
        gs.items[item_idx] = core::types::Item::default();
        gs.items[item_idx].used = USE_ACTIVE;
        gs.items[item_idx].temp = template_id as u16;
        gs.items[item_idx].carried = cn as u16;
        gs.characters[cn].item[slot] = item_idx as u32;
    }

    #[test]
    fn mix_potion_flask_and_red_flower_creates_healing_potion_and_consumes_inputs() {
        with_test_gs(|gs| {
            let (cn, _nr) = add_test_player(gs);
            let (flask, flower) = (1, 2);
            seed_template(gs, IT_HEALING_POTION);
            carried_item(gs, cn, flask, IT_FLASK, 0);
            carried_item(gs, cn, flower, IT_RED_FLOWER, 1);
            gs.characters[cn].citem = flower as u32;

            assert!(use_mix_potion(gs, cn, flask));

            assert_eq!(gs.items[flask].used, USE_EMPTY);
            assert_eq!(gs.items[flower].used, USE_EMPTY);
            assert_eq!(gs.characters[cn].citem, 0);
            let result = gs.characters[cn].item[0] as usize;
            assert_ne!(result, 0);
            assert_eq!(gs.items[result].temp, IT_HEALING_POTION as u16);
            assert!((gs.items[result].flags & ItemFlags::IF_UPDATE.bits()) != 0);
        });
    }

    #[test]
    fn mix_potion_invalid_combo_leaves_items_unchanged() {
        with_test_gs(|gs| {
            let (cn, _nr) = add_test_player(gs);
            let (flask, ingredient) = (1, 2);
            carried_item(gs, cn, flask, IT_FLASK, 0);
            carried_item(gs, cn, ingredient, 206, 1);
            gs.characters[cn].citem = ingredient as u32;

            assert!(!use_mix_potion(gs, cn, flask));

            assert_eq!(gs.items[flask].used, USE_ACTIVE);
            assert_eq!(gs.items[ingredient].used, USE_ACTIVE);
            assert_eq!(gs.characters[cn].citem, ingredient as u32);
        });
    }

    #[test]
    fn chain_rejects_ground_chain_before_consuming_link() {
        with_test_gs(|gs| {
            let (cn, _nr) = add_test_player(gs);
            let (chain, link) = (1, 2);
            carried_item(gs, cn, link, 206, 0);
            gs.items[chain] = core::types::Item::default();
            gs.items[chain].used = USE_ACTIVE;
            gs.items[chain].temp = 100;
            gs.items[chain].data[0] = 101;
            gs.characters[cn].citem = link as u32;

            assert!(!use_chain(gs, cn, chain));

            assert_eq!(gs.items[chain].used, USE_ACTIVE);
            assert_eq!(gs.items[link].used, USE_ACTIVE);
            assert_eq!(gs.characters[cn].citem, link as u32);
        });
    }

    #[test]
    fn chain_with_link_advances_to_next_template() {
        with_test_gs(|gs| {
            let (cn, _nr) = add_test_player(gs);
            let (chain, link) = (1, 2);
            seed_template(gs, 101);
            carried_item(gs, cn, chain, 100, 0);
            gs.items[chain].data[0] = 101;
            carried_item(gs, cn, link, 206, 1);
            gs.characters[cn].citem = link as u32;

            assert!(use_chain(gs, cn, chain));

            assert_eq!(gs.items[chain].used, USE_EMPTY);
            assert_eq!(gs.items[link].used, USE_EMPTY);
            let result = gs.characters[cn].item[0] as usize;
            assert_eq!(gs.items[result].temp, 101);
            assert!((gs.items[result].flags & ItemFlags::IF_UPDATE.bits()) != 0);
        });
    }

    #[test]
    fn stone_sword_requires_strength_before_creating_reward() {
        with_test_gs(|gs| {
            let (cn, _nr) = add_test_player(gs);
            let stone = 1;
            seed_template(gs, 27);
            gs.items[stone] = core::types::Item::default();
            gs.items[stone].used = USE_ACTIVE;
            gs.items[stone].data[0] = 27;
            gs.characters[cn].attrib[AT_STREN as usize][5] = 99;

            assert!(!stone_sword(gs, cn, stone));
            assert!(gs.characters[cn].item.iter().all(|item| *item == 0));

            gs.characters[cn].attrib[AT_STREN as usize][5] = 100;
            assert!(stone_sword(gs, cn, stone));
            let result = gs.characters[cn].item[0] as usize;
            assert_eq!(gs.items[result].temp, 27);
        });
    }

    #[test]
    fn rat_eye_wrong_component_leaves_slots_unchanged() {
        with_test_gs(|gs| {
            let (cn, _nr) = add_test_player(gs);
            let (eye, component) = (1, 2);
            carried_item(gs, cn, eye, 900, 0);
            gs.items[eye].data[0] = IT_RED_FLOWER as u32;
            carried_item(gs, cn, component, IT_BLUE_FLOWER, 1);
            gs.characters[cn].citem = component as u32;

            assert!(!rat_eye(gs, cn, eye));

            assert_eq!(gs.items[eye].data[0], IT_RED_FLOWER as u32);
            assert_eq!(gs.items[component].used, USE_ACTIVE);
            assert_eq!(gs.characters[cn].citem, component as u32);
        });
    }

    #[test]
    fn build_ring_and_amulet_consume_components_and_create_results() {
        with_test_gs(|gs| {
            let (cn, _nr) = add_test_player(gs);
            let (ring, gem) = (1, 2);
            seed_template(gs, 362);
            carried_item(gs, cn, ring, 337, 0);
            carried_item(gs, cn, gem, 339, 1);
            gs.characters[cn].citem = gem as u32;

            assert!(build_ring(gs, cn, ring));

            assert_eq!(gs.items[ring].used, USE_EMPTY);
            assert_eq!(gs.items[gem].used, USE_EMPTY);
            let result_ring = gs.characters[cn].item[0] as usize;
            assert_eq!(gs.items[result_ring].temp, 362);

            let (amulet_a, amulet_b) = (4, 5);
            seed_template(gs, 476);
            carried_item(gs, cn, amulet_a, 471, 1);
            carried_item(gs, cn, amulet_b, 472, 2);
            gs.characters[cn].citem = amulet_b as u32;

            assert!(build_amulet(gs, cn, amulet_a));

            assert_eq!(gs.items[amulet_a].used, USE_EMPTY);
            assert_eq!(gs.items[amulet_b].used, USE_EMPTY);
            let result_amulet = gs.characters[cn].item[1] as usize;
            assert_eq!(gs.items[result_amulet].temp, 476);
        });
    }

    #[test]
    fn soulstone_repairs_torch_from_template_and_consumes_stone() {
        with_test_gs(|gs| {
            let (cn, _nr) = add_test_player(gs);
            let (soulstone, torch) = (1, 2);
            seed_template(gs, 91);
            gs.item_templates[91].max_age[1] = 7;
            carried_item(gs, cn, soulstone, 1146, 0);
            gs.items[soulstone].driver = 68;
            carried_item(gs, cn, torch, 91, 1);
            gs.items[torch].current_age[1] = 5;
            gs.characters[cn].citem = torch as u32;

            assert!(use_soulstone(gs, cn, soulstone));

            assert_eq!(gs.items[soulstone].used, USE_EMPTY);
            assert_eq!(gs.items[torch].used, USE_ACTIVE);
            assert_eq!(gs.items[torch].carried, cn as u16);
            assert_eq!(gs.items[torch].current_age[1], 0);
            assert_eq!(gs.items[torch].max_age[1], 28);
            assert_eq!(gs.items[torch].temp, 0);
            assert!((gs.items[torch].flags & ItemFlags::IF_UPDATE.bits()) != 0);
        });
    }

    #[test]
    fn combining_soulstones_rewrites_description_and_consumes_cursor_stone() {
        with_test_gs(|gs| {
            let (cn, _nr) = add_test_player(gs);
            let soulstone_idx = 1;
            let cursor_soulstone_idx = 2;

            for item_idx in [soulstone_idx, cursor_soulstone_idx] {
                gs.items[item_idx] = core::types::Item::default();
                gs.items[item_idx].used = USE_ACTIVE;
                gs.items[item_idx].driver = 68;
                gs.items[item_idx].carried = cn as u16;
            }
            gs.items[soulstone_idx].description.fill(b'x');
            gs.items[cursor_soulstone_idx].data[1] = 0;
            gs.characters[cn].item[0] = soulstone_idx as u32;
            gs.characters[cn].citem = cursor_soulstone_idx as u32;

            assert!(use_soulstone(gs, cn, soulstone_idx));

            let expected = b"Level 0 soulstone, holding 0 exp.";
            assert_eq!(
                &gs.items[soulstone_idx].description[..expected.len()],
                expected
            );
            assert!(
                gs.items[soulstone_idx].description[expected.len()..]
                    .iter()
                    .all(|byte| *byte == 0)
            );
            assert_eq!(gs.characters[cn].citem, 0);
            assert_eq!(gs.items[cursor_soulstone_idx].used, USE_EMPTY);
        });
    }

    #[test]
    fn combining_small_stone_with_big_cursor_stone_never_loses_exp() {
        with_test_gs(|gs| {
            let (cn, _nr) = add_test_player(gs);
            let (small, big) = (1, 2);
            for _ in 0..50 {
                for item_idx in [small, big] {
                    gs.items[item_idx] = core::types::Item::default();
                    gs.items[item_idx].used = USE_ACTIVE;
                    gs.items[item_idx].driver = 68;
                    gs.items[item_idx].carried = cn as u16;
                }
                gs.items[small].data[1] = 100;
                gs.items[big].data[1] = 1_000_000;
                gs.characters[cn].item[0] = small as u32;
                gs.characters[cn].citem = big as u32;

                assert!(use_soulstone(gs, cn, small));

                let exp = gs.items[small].data[1];
                assert!((1_000_000..=1_000_100).contains(&exp), "exp {exp}");
                assert_eq!(gs.items[small].data[0], core::ranks::points2rank(exp));
            }
        });
    }
}
