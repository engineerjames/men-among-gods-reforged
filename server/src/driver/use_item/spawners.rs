use super::take_item_from_char;
use crate::effect::EffectManager;
use crate::game_state::GameState;
use crate::god::God;
use crate::helpers::{self};
use crate::{chlog, populate};
use core::constants::{
    CharacterFlags, IT_BRONZE_ARMOR, IT_BRONZE_HELMET, IT_CRYSTAL_ARMOR, IT_CRYSTAL_HELMET,
    IT_EMERALD_ARMOR, IT_EMERALD_HELMET, IT_GOLD_ARMOR, IT_GOLD_HELMET, IT_STEEL_ARMOR,
    IT_STEEL_HELMET, IT_TITANIUM_ARMOR, IT_TITANIUM_HELMET, ItemFlags, MAXTITEM, TICKS, USE_ACTIVE,
    USE_EMPTY,
};
use core::string_operations::c_string_to_str;
use core::traits;

/// Handles the legacy `use_create_item` item-use hook.
///
/// # Arguments
///
/// * `gs` - Active game state used by this legacy driver hook.
/// * `cn` - Character index executing this legacy driver hook.
/// * `item_idx` - Value passed through to the legacy `use_create_item` hook.
///
/// # Returns
///
/// * `true` when `use_create_item` reports that it handled the legacy action, otherwise `false`.
///
/// # Panics
///
/// * Panics if any legacy id or index parameter used by `use_create_item` is outside the corresponding game-state collection.
pub fn use_create_item(gs: &mut GameState, cn: usize, item_idx: usize) -> bool {
    if cn == 0 {
        return false;
    }

    let (active, template_id) = (
        gs.items[item_idx].active,
        gs.items[item_idx].data[0] as usize,
    );

    if active != 0 {
        return false;
    }

    if template_id == 0 || template_id >= MAXTITEM {
        return false;
    }

    let in2 = match God::create_item(gs, template_id) {
        Some(id) => id,
        None => return false,
    };

    if !God::give_character_item(gs, cn, in2) {
        let item_ref = gs.items[in2].reference;
        gs.do_character_log(
            cn,
            core::types::FontColor::Blue,
            &format!(
                "Your backpack is full, so you can't take the {}.\n",
                c_string_to_str(&item_ref)
            ),
        );
        gs.items[in2].used = core::constants::USE_EMPTY;
        return false;
    }

    let item_ref = gs.items[in2].reference;
    let item_name = gs.items[in2].get_name().to_owned();
    let source_name = gs.items[item_idx].get_name().to_owned();
    gs.do_character_log(
        cn,
        core::types::FontColor::Green,
        &format!("You got a {}.\n", c_string_to_str(&item_ref)),
    );
    log::info!("Character {} got {} from {}", cn, item_name, source_name);

    let driver = gs.items[item_idx].driver;
    let data1 = gs.items[item_idx].data[1];

    if data1 != 0 && driver == 53 {
        let char_name = gs.characters[cn].get_name().to_owned();
        let item_ref = c_string_to_str(&gs.items[in2].reference).to_owned();
        gs.do_character_log(
            cn,
            core::types::FontColor::Blue,
            &format!(
                "You feel yourself form a magical connection with the {}.\n",
                item_ref
            ),
        );

        let item = &mut gs.items[in2];
        item.data[0] = cn as u32;

        let new_desc = format!(
            "{} Engraved in it are the letters \"{}\".",
            c_string_to_str(&item.description),
            char_name,
        );
        if new_desc.len() < 200 {
            let bytes = new_desc.as_bytes();
            item.description[..bytes.len()].copy_from_slice(bytes);
            if bytes.len() < 200 {
                item.description[bytes.len()..].fill(0);
            }
        }
    }

    if driver == 54 {
        let (x, y) = (
            i32::from(gs.items[item_idx].x),
            i32::from(gs.items[item_idx].y),
        );
        gs.do_area_notify(
            cn as i32,
            0,
            x,
            y,
            i32::from(core::constants::NT_HITME),
            cn as i32,
            0,
            0,
            0,
        );
    }

    true
}

/// Handles the legacy `use_create_gold` item-use hook.
///
/// # Arguments
///
/// * `gs` - Active game state used by this legacy driver hook.
/// * `cn` - Character index executing this legacy driver hook.
/// * `item_idx` - Value passed through to the legacy `use_create_gold` hook.
///
/// # Returns
///
/// * `true` when `use_create_gold` reports that it handled the legacy action, otherwise `false`.
///
/// # Panics
///
/// * Panics if any legacy id or index parameter used by `use_create_gold` is outside the corresponding game-state collection.
pub fn use_create_gold(gs: &mut GameState, cn: usize, item_idx: usize) -> bool {
    if cn == 0 {
        return false;
    }

    let (active, gold_amount) = (gs.items[item_idx].active, gs.items[item_idx].data[0]);

    if active != 0 {
        return false;
    }

    let gold_to_add = gold_amount * 100;
    gs.characters[cn].gold += gold_to_add as i32;
    gs.do_character_log(
        cn,
        core::types::FontColor::Green,
        &format!("You got a {}G.\n", gold_amount),
    );
    log::info!(
        "Character {} got {}G from {}",
        cn,
        gold_amount,
        gs.items[item_idx]
            .name
            .iter()
            .take_while(|&&c| c != 0)
            .map(|&c| c as char)
            .collect::<String>()
    );

    true
}

/// Handles the legacy `use_create_item2` item-use hook.
///
/// # Arguments
///
/// * `gs` - Active game state used by this legacy driver hook.
/// * `cn` - Character index executing this legacy driver hook.
/// * `item_idx` - Value passed through to the legacy `use_create_item2` hook.
///
/// # Returns
///
/// * `true` when `use_create_item2` reports that it handled the legacy action, otherwise `false`.
///
/// # Panics
///
/// * Panics if any legacy id or index parameter used by `use_create_item2` is outside the corresponding game-state collection.
pub fn use_create_item2(gs: &mut GameState, cn: usize, item_idx: usize) -> bool {
    if cn == 0 {
        return false;
    }

    let (active, required_temp, template_id) = (
        gs.items[item_idx].active,
        gs.items[item_idx].data[1],
        gs.items[item_idx].data[0] as usize,
    );

    if active != 0 {
        return false;
    }

    // Check if character has the required item in citem
    let citem = gs.characters[cn].citem as usize;

    if citem == 0 || (citem & 0x80000000) != 0 {
        return false;
    }

    let citem_temp = gs.items[citem].temp;

    if u32::from(citem_temp) != required_temp {
        return false;
    }

    if template_id == 0 || template_id >= MAXTITEM {
        return false;
    }

    let in2 = match God::create_item(gs, template_id) {
        Some(id) => id,
        None => return false,
    };

    if !God::give_character_item(gs, cn, in2) {
        let item_ref = c_string_to_str(&gs.items[in2].reference);
        gs.do_character_log(
            cn,
            core::types::FontColor::Blue,
            &format!(
                "Your backpack is full, so you can't take the {}.\n",
                item_ref
            ),
        );
        gs.items[in2].used = USE_EMPTY;
        return false;
    }

    let item_ref = c_string_to_str(&gs.items[in2].reference);
    gs.do_character_log(
        cn,
        core::types::FontColor::Green,
        &format!("You got a {}.\n", item_ref),
    );
    chlog!(
        cn,
        "Got {} from {}",
        gs.items[in2].get_name(),
        gs.items[item_idx].get_name()
    );

    gs.items[citem].used = USE_EMPTY;
    gs.characters[cn].citem = 0;

    true
}

/// Handles the legacy `use_create_item3` item-use hook.
///
/// # Arguments
///
/// * `gs` - Active game state used by this legacy driver hook.
/// * `cn` - Character index executing this legacy driver hook.
/// * `item_idx` - Value passed through to the legacy `use_create_item3` hook.
///
/// # Returns
///
/// * `true` when `use_create_item3` reports that it handled the legacy action, otherwise `false`.
///
/// # Panics
///
/// * Panics if any legacy id or index parameter used by `use_create_item3` is outside the corresponding game-state collection.
pub fn use_create_item3(gs: &mut GameState, cn: usize, item_idx: usize) -> bool {
    if cn == 0 {
        return false;
    }

    let active = gs.items[item_idx].active;

    if active != 0 {
        return false;
    }

    // Find how many data entries are non-zero
    let data_entries = {
        let item_data = gs.items[item_idx].data;
        let mut count = 0;
        for &val in &item_data[..10] {
            if val == 0 {
                break;
            }
            count += 1;
        }
        if count == 0 {
            None
        } else {
            Some((count, item_data))
        }
    };

    let (count, data) = match data_entries {
        Some(v) => v,
        None => return false,
    };

    // Pick a random entry
    let n = helpers::random_mod_usize(count);
    let template_id = data[n] as usize;

    if template_id == 0 || template_id >= MAXTITEM {
        return false;
    }

    // Check if this is a special item template
    let in2 = match template_id {
        IT_BRONZE_HELMET | IT_BRONZE_ARMOR | IT_STEEL_HELMET | IT_STEEL_ARMOR | IT_GOLD_HELMET
        | IT_GOLD_ARMOR | IT_CRYSTAL_HELMET | IT_CRYSTAL_ARMOR | IT_TITANIUM_HELMET
        | IT_TITANIUM_ARMOR | IT_EMERALD_HELMET | IT_EMERALD_ARMOR => {
            helpers::create_special_item(gs, template_id)
        }
        _ => God::create_item(gs, template_id),
    };

    let in2 = match in2 {
        Some(id) => id,
        None => {
            gs.do_character_log(cn, core::types::FontColor::Green, "It's empty...\n");
            return true;
        }
    };

    if !God::give_character_item(gs, cn, in2) {
        gs.do_character_log(
            cn,
            core::types::FontColor::Blue,
            "Your backpack is full, so you can't take anything.\n",
        );
        gs.items[in2].used = USE_EMPTY;
        return false;
    }

    let item_ref = c_string_to_str(&gs.items[in2].reference);
    gs.do_character_log(
        cn,
        core::types::FontColor::Green,
        &format!("You got a {}.\n", item_ref),
    );
    chlog!(
        cn,
        "Got {} from {}",
        gs.items[in2].get_name(),
        gs.items[item_idx].get_name()
    );

    true
}

/// Handles the legacy `use_bag` item-use hook.
///
/// # Arguments
///
/// * `gs` - Active game state used by this legacy driver hook.
/// * `cn` - Character index executing this legacy driver hook.
/// * `item_idx` - Value passed through to the legacy `use_bag` hook.
///
/// # Returns
///
/// * `true` when `use_bag` reports that it handled the legacy action, otherwise `false`.
///
/// # Panics
///
/// * Panics if any legacy id or index parameter used by `use_bag` is outside the corresponding game-state collection.
pub fn use_bag(gs: &mut GameState, cn: usize, item_idx: usize) -> bool {
    let co = gs.items[item_idx].data[0] as usize;

    if !core::types::Character::is_sane_character(co) {
        return false;
    }

    let owner = gs.characters[co].data[core::constants::CHD_CORPSEOWNER] as usize;

    if owner != 0 && owner != cn {
        let may_attack = gs.may_attack_msg(cn, owner, false);
        let allowed_cn = gs.characters[owner].data[core::constants::CHD_ALLOW] as usize;

        if !may_attack && allowed_cn != cn {
            let owner_name = c_string_to_str(&gs.characters[owner].name).to_owned();
            let owner_is_male = (gs.characters[owner].kindred & traits::KIN_MALE as i32) != 0;
            let owner_pronoun = if owner_is_male { "his" } else { "her" };

            gs.do_character_log(
                cn,
                core::types::FontColor::Green,
                &format!(
                    "This is {}'s grave, not yours. You may only search it with {} permission.\n",
                    owner_name, owner_pronoun
                ),
            );

            let corpse_active = gs.characters[co].used == USE_ACTIVE;
            let owner_x = gs.characters[owner].x;

            if corpse_active && owner_x != 0 {
                let cn_name = c_string_to_str(&gs.characters[cn].name).to_owned();
                gs.do_character_log(
                    owner,
                    core::types::FontColor::Green,
                    &format!(
                        "{} just tried to search your grave. You must #ALLOW {} if you want them to.\n",
                        cn_name, cn_name
                    ),
                );
            }

            return false;
        }
    }

    let co_ref = c_string_to_str(&gs.characters[co].reference).to_owned();
    gs.do_character_log(
        cn,
        core::types::FontColor::Yellow,
        &format!("You search the remains of {}.\n", co_ref),
    );
    gs.do_look_char(cn, co, 0, 0, 1);

    true
}

/// Handles the legacy `use_spawn` item-use hook.
///
/// # Arguments
///
/// * `gs` - Active game state used by this legacy driver hook.
/// * `cn` - Character index executing this legacy driver hook.
/// * `item_idx` - Value passed through to the legacy `use_spawn` hook.
///
/// # Returns
///
/// * `true` when `use_spawn` reports that it handled the legacy action, otherwise `false`.
///
/// # Panics
///
/// * Panics if any legacy id or index parameter used by `use_spawn` is outside the corresponding game-state collection.
pub fn use_spawn(gs: &mut GameState, cn: usize, item_idx: usize) -> bool {
    // Check if already active
    let is_active = gs.items[item_idx].active != 0;
    if is_active {
        return false;
    }

    // Check if player needs to provide an item (data[1])
    if cn != 0 {
        let required_template = gs.items[item_idx].data[1] as usize;

        if required_template != 0 {
            let citem = gs.characters[cn].citem;

            if citem == 0 || (citem & 0x80000000) != 0 {
                return false;
            }

            let citem_template = gs.items[citem as usize].temp;
            if citem_template as usize != required_template {
                return false;
            }

            // Remove the required item
            {
                gs.items[citem as usize].used = USE_EMPTY;
            };
            {
                gs.characters[cn].citem = 0;
            };
        }
    }

    // Add effect if data[2] contains a character template
    let temp = gs.items[item_idx].data[2] as usize;
    if temp != 0 {
        EffectManager::fx_add_effect(
            gs,
            2,
            core::constants::TICKS * 10,
            i32::from(gs.character_templates[temp].x),
            i32::from(gs.character_templates[temp].y),
            temp as i32,
        );
        log::info!("use_spawn: would add effect for template {}", temp);
    }

    true
}

/// Handles the legacy `use_pile` item-use hook.
///
/// # Arguments
///
/// * `gs` - Active game state used by this legacy driver hook.
/// * `cn` - Character index executing this legacy driver hook.
/// * `item_idx` - Value passed through to the legacy `use_pile` hook.
///
/// # Returns
///
/// * `true` when `use_pile` reports that it handled the legacy action, otherwise `false`.
///
/// # Panics
///
/// * Panics if any legacy id or index parameter used by `use_pile` is outside the corresponding game-state collection.
pub fn use_pile(gs: &mut GameState, cn: usize, item_idx: usize) -> bool {
    // Item templates for rewards at different levels
    const FIND: [usize; 90] = [
        // Level 0 (0-29): silver, small jewels, skeleton
        361, 361, 339, 342, 345, 339, 342, 345, 359, 359, 361, 361, 339, 342, 345, 339, 342, 345,
        359, 359, 361, 361, 339, 342, 345, 339, 342, 345, 359, 359,
        // Level 1 (30-59): silver, med jewels, golem
        361, 361, 361, 340, 343, 346, 371, 371, 371, 371, 361, 361, 361, 340, 343, 346, 371, 371,
        371, 371, 361, 361, 361, 340, 343, 346, 371, 371, 371, 371,
        // Level 2 (60-89): gold, big jewels, gargoyle
        360, 341, 344, 347, 372, 372, 372, 487, 372, 372, 360, 341, 344, 347, 372, 372, 372, 488,
        372, 372, 360, 341, 344, 347, 372, 372, 372, 489, 372, 372,
    ];

    // Check if already active (already searched)
    let is_active = gs.items[item_idx].active != 0;
    if is_active {
        return false;
    }

    // Get pile info
    let (x, y, level) = {
        (
            gs.items[item_idx].x,
            gs.items[item_idx].y,
            gs.items[item_idx].data[0] as i32,
        )
    };

    // Destroy this object
    {
        gs.items[item_idx].used = USE_EMPTY;
    };

    let m = (x as usize) + (y as usize) * core::constants::SERVER_MAPX as usize;
    {
        gs.map[m].it = 0;
    };

    // Calculate chance based on player's luck
    let luck = gs.characters[cn].luck;

    let mut chance = 6;
    if luck < 0 {
        chance += 1;
    }
    if luck <= -100 {
        chance += 1;
    }
    if luck <= -500 {
        chance += 1;
    }
    if luck <= -1000 {
        chance += 1;
    }
    if luck <= -2000 {
        chance += 1;
    }
    if luck <= -3000 {
        chance += 1;
    }
    if luck <= -4000 {
        chance += 1;
    }
    if luck <= -6000 {
        chance += 1;
    }
    if luck <= -8000 {
        chance += 1;
    }
    if luck <= -10000 {
        chance += 1;
    }

    // Roll for loot
    if crate::helpers::random_mod(chance) != 0 {
        return true; // Nothing found
    }

    // Determine what to give based on level
    let tmp_idx = crate::helpers::random_mod_usize(30) + (level as usize * 30);
    let tmp_idx = tmp_idx.min(89); // Clamp to valid range
    let tmp = FIND[tmp_idx];

    // Create item
    if let Some(in2) = God::create_item(gs, tmp) {
        let (is_takeable, data_0) = {
            (
                (gs.items[in2].flags & ItemFlags::IF_TAKE.bits()) != 0,
                gs.items[in2].data[0],
            )
        };

        if is_takeable {
            // Give to player
            if God::give_character_item(gs, cn, in2) {
                let reference = gs.items[in2].reference;
                gs.do_character_log(
                    cn,
                    core::types::FontColor::Green,
                    &format!("You've found a {}!\n", c_string_to_str(&reference)),
                );
            }
        } else {
            // It's a monster spawner
            God::drop_item(gs, in2, x as usize, y as usize);
            EffectManager::fx_add_effect(gs, 9, 16, in2 as i32, data_0 as i32, 0);
            log::info!("use_pile: spawning monster at ({}, {})", x, y);
        }
    }

    true
}

/// Handles the legacy `use_grave` item-use hook.
///
/// # Arguments
///
/// * `gs` - Active game state used by this legacy driver hook.
/// * `_cn` - Character index reserved for this legacy driver hook.
/// * `item_idx` - Value passed through to the legacy `use_grave` hook.
///
/// # Returns
///
/// * `true` when `use_grave` reports that it handled the legacy action, otherwise `false`.
///
/// # Panics
///
/// * Panics if any legacy id or index parameter used by `use_grave` is outside the corresponding game-state collection.
pub fn use_grave(gs: &mut GameState, _cn: usize, item_idx: usize) -> bool {
    // Get previously spawned character
    let cc = gs.items[item_idx].data[0] as usize;

    // Check if still alive and linked
    if cc > 0 {
        let is_alive = cc < gs.characters.len() && {
            let ch = &gs.characters[cc];
            ch.data[0] as usize == item_idx
                && (ch.flags & CharacterFlags::Body.bits()) == 0
                && ch.used != USE_EMPTY
        };

        if is_alive {
            return true; // Still alive, don't spawn new one
        }
    }

    let cc = match populate::pop_create_char(gs, 328, false) {
        Some(cc) => cc,
        None => return true,
    };

    let (item_x, item_y) = (gs.items[item_idx].x, gs.items[item_idx].y);

    if !God::drop_char_fuzzy(gs, cc, item_x as usize, item_y as usize) {
        God::destroy_items(gs, cc);
        {
            gs.characters[cc].used = USE_EMPTY;
        };
        return true;
    }

    // Create link between item and character
    {
        gs.characters[cc].data[0] = item_idx as i32;
    };

    {
        gs.items[item_idx].data[0] = cc as u32;
    };

    true
}

/// Handles the legacy `use_gargoyle` item-use hook.
///
/// # Arguments
///
/// * `gs` - Active game state used by this legacy driver hook.
/// * `cn` - Character index executing this legacy driver hook.
/// * `item_idx` - Value passed through to the legacy `use_gargoyle` hook.
///
/// # Returns
///
/// * `true` when `use_gargoyle` reports that it handled the legacy action, otherwise `false`.
///
/// # Panics
///
/// * Panics if any legacy id or index parameter used by `use_gargoyle` is outside the corresponding game-state collection.
pub fn use_gargoyle(gs: &mut GameState, cn: usize, item_idx: usize) -> bool {
    use crate::god::God;
    use core::constants::USE_EMPTY;

    if cn == 0 {
        return false;
    }

    let carried = gs.items[item_idx].carried;
    if carried == 0 {
        return false;
    }

    // Create gargoyle character (template 325)
    let cc = match God::create_char(gs, 325, true) {
        Some(cc) => cc as usize,
        None => return false,
    };

    // Get character position
    let (ch_x, ch_y) = (gs.characters[cn].x, gs.characters[cn].y);

    // Try to drop near character
    if !God::drop_char_fuzzy(gs, cc, ch_x as usize, ch_y as usize) {
        {
            gs.characters[cc].used = USE_EMPTY;
        };
        God::destroy_items(gs, cc);
        gs.do_character_log(
            cn,
            core::types::FontColor::Green,
            "The Gargoyle could not materialize.\n",
        );
        return false;
    }

    // Remove item
    take_item_from_char(gs, item_idx, cn);
    {
        gs.items[item_idx].used = USE_EMPTY;
    };

    // Configure gargoyle
    let ticker = gs.globals.ticker;
    {
        gs.characters[cc].data[42] = 65536 + cn as i32; // set group
        gs.characters[cc].data[59] = 65536 + cn as i32; // protect all members
        gs.characters[cc].data[63] = cn as i32; // obey and protect char
        gs.characters[cc].data[69] = cn as i32; // follow char
        gs.characters[cc].data[64] = ticker + (TICKS * 60 * 15);
    };

    true
}

/// Handles the legacy `use_grolm` item-use hook.
///
/// # Arguments
///
/// * `gs` - Active game state used by this legacy driver hook.
/// * `cn` - Character index executing this legacy driver hook.
/// * `item_idx` - Value passed through to the legacy `use_grolm` hook.
///
/// # Returns
///
/// * `true` when `use_grolm` reports that it handled the legacy action, otherwise `false`.
///
/// # Panics
///
/// * Panics if any legacy id or index parameter used by `use_grolm` is outside the corresponding game-state collection.
pub fn use_grolm(gs: &mut GameState, cn: usize, item_idx: usize) -> bool {
    use crate::god::God;
    use core::constants::USE_EMPTY;

    if cn == 0 {
        return false;
    }

    let carried = gs.items[item_idx].carried;
    if carried == 0 {
        return false;
    }

    // Create grolm character (template 577)
    let cc = match God::create_char(gs, 577, true) {
        Some(cc) => cc as usize,
        None => return false,
    };

    // Get character position
    let (ch_x, ch_y) = (gs.characters[cn].x, gs.characters[cn].y);

    // Try to drop near character
    if !God::drop_char_fuzzy(gs, cc, ch_x as usize, ch_y as usize) {
        {
            gs.characters[cc].used = USE_EMPTY;
        };
        God::destroy_items(gs, cc);
        gs.do_character_log(
            cn,
            core::types::FontColor::Green,
            "The Grolm could not materialize.\n",
        );
        return false;
    }

    // Remove item
    take_item_from_char(gs, item_idx, cn);
    {
        gs.items[item_idx].used = USE_EMPTY;
    };

    // Configure grolm
    let ticker = gs.globals.ticker;
    {
        gs.characters[cc].data[42] = 65536 + cn as i32; // set group
        gs.characters[cc].data[59] = 65536 + cn as i32; // protect all members
        gs.characters[cc].data[63] = cn as i32; // obey and protect char
        gs.characters[cc].data[69] = cn as i32; // follow char
        gs.characters[cc].data[64] = ticker + (TICKS * 60 * 15);
    };

    true
}

/// Handles the legacy `use_create_npc` item-use hook.
///
/// # Arguments
///
/// * `gs` - Active game state used by this legacy driver hook.
/// * `cn` - Character index executing this legacy driver hook.
/// * `item_idx` - Value passed through to the legacy `use_create_npc` hook.
///
/// # Returns
///
/// * `true` when `use_create_npc` reports that it handled the legacy action, otherwise `false`.
///
/// # Panics
///
/// * Panics if any legacy id or index parameter used by `use_create_npc` is outside the corresponding game-state collection.
pub fn use_create_npc(gs: &mut GameState, cn: usize, item_idx: usize) -> bool {
    // Check if already active
    let active = gs.items[item_idx].active;
    if active != 0 {
        return false;
    }

    if cn == 0 {
        return false;
    }

    // Create NPC from template
    let template = gs.items[item_idx].data[0];
    let co = match populate::pop_create_char(gs, template as usize, false) {
        Some(co) => co,
        None => return false,
    };

    // Drop NPC near item location
    let (x, y) = (gs.items[item_idx].x as usize, gs.items[item_idx].y as usize);
    if !God::drop_char_fuzzy(gs, co, x, y) {
        God::destroy_items(gs, co);
        gs.characters[co].used = USE_EMPTY;

        return false;
    }

    // Link NPC to creator
    gs.characters[co].data[0] = cn as i32;

    true
}

/// Handles the legacy `use_garbage` item-use hook.
///
/// # Arguments
///
/// * `gs` - Active game state used by this legacy driver hook.
/// * `cn` - Character index executing this legacy driver hook.
/// * `_item_idx` - Value passed through to the legacy `use_garbage` hook.
///
/// # Returns
///
/// * `true` when `use_garbage` reports that it handled the legacy action, otherwise `false`.
///
/// # Panics
///
/// * Panics if any legacy id or index parameter used by `use_garbage` is outside the corresponding game-state collection.
pub fn use_garbage(gs: &mut GameState, cn: usize, _item_idx: usize) -> bool {
    if cn == 0 {
        return false;
    }

    let citem = gs.characters[cn].citem;
    if citem == 0 {
        gs.do_character_log(
            cn,
            core::types::FontColor::Yellow,
            "You feel that you could dispose of unwanted items in this digusting garbage can.\n",
        );
        return false;
    }

    if (citem & 0x80000000) != 0 {
        // Money
        let val = citem & 0x7fffffff;
        gs.characters[cn].citem = 0;

        gs.do_character_log(
            cn,
            core::types::FontColor::Yellow,
            &format!(
                "You disposed of {} gold and {} silver.\n",
                val / 100,
                val % 100
            ),
        );
    } else {
        // Item
        let reference = { c_string_to_str(&gs.items[citem as usize].reference).to_owned() };
        gs.characters[cn].citem = 0;
        gs.items[citem as usize].used = USE_EMPTY;

        gs.do_character_log(
            cn,
            core::types::FontColor::Yellow,
            &format!("You disposed of the {}.\n", reference),
        );
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_state::GameState;
    use crate::test_helpers::{add_test_player, with_test_gs};
    use core::constants::{ItemFlags, SERVER_MAPX, USE_ACTIVE, USE_EMPTY};
    use core::string_operations::write_ascii_into_fixed;

    /// Installs a minimal active, takeable item template for create-item tests.
    fn make_takeable_template(gs: &mut GameState, template_id: usize, reference: &str) {
        gs.item_templates[template_id] = core::types::Item::default();
        gs.item_templates[template_id].used = USE_ACTIVE;
        gs.item_templates[template_id].flags = ItemFlags::IF_TAKE.bits();
        write_ascii_into_fixed(&mut gs.item_templates[template_id].name, reference);
        write_ascii_into_fixed(&mut gs.item_templates[template_id].reference, reference);
    }

    /// Places a minimal active source item at a stable map coordinate.
    fn make_source_item(gs: &mut GameState, item_idx: usize) {
        gs.items[item_idx] = core::types::Item::default();
        gs.items[item_idx].used = USE_ACTIVE;
        gs.items[item_idx].x = 10;
        gs.items[item_idx].y = 10;
        write_ascii_into_fixed(&mut gs.items[item_idx].name, "source");
        write_ascii_into_fixed(&mut gs.items[item_idx].reference, "source");
    }

    #[test]
    fn use_create_gold_adds_gold_as_silver_units() {
        with_test_gs(|gs| {
            let (cn, _nr) = add_test_player(gs);
            let source = 1;
            make_source_item(gs, source);
            gs.items[source].data[0] = 7;

            assert!(use_create_gold(gs, cn, source));

            assert_eq!(gs.characters[cn].gold, 700);
        });
    }

    #[test]
    fn use_create_item_gives_template_copy_to_inventory() {
        with_test_gs(|gs| {
            let (cn, _nr) = add_test_player(gs);
            let source = 1;
            let template_id = 42;
            make_source_item(gs, source);
            make_takeable_template(gs, template_id, "a test trinket");
            gs.items[source].data[0] = template_id as u32;

            assert!(use_create_item(gs, cn, source));

            let created = gs.characters[cn].item[0] as usize;
            assert_ne!(created, 0);
            assert_eq!(gs.items[created].temp as usize, template_id);
            assert_eq!(gs.items[created].carried as usize, cn);
        });
    }

    #[test]
    fn use_create_item_refuses_active_source_without_creating_item() {
        with_test_gs(|gs| {
            let (cn, _nr) = add_test_player(gs);
            let source = 1;
            let template_id = 43;
            make_source_item(gs, source);
            make_takeable_template(gs, template_id, "an active reward");
            gs.items[source].active = 1;
            gs.items[source].data[0] = template_id as u32;

            assert!(!use_create_item(gs, cn, source));

            assert_eq!(gs.characters[cn].item[0], 0);
        });
    }

    #[test]
    fn use_create_item2_consumes_required_cursor_item_on_success() {
        with_test_gs(|gs| {
            let (cn, _nr) = add_test_player(gs);
            let source = 1;
            let cursor = 2;
            let required_template = 99;
            let reward_template = 44;
            make_source_item(gs, source);
            make_takeable_template(gs, reward_template, "a gated reward");
            gs.items[source].data[0] = reward_template as u32;
            gs.items[source].data[1] = required_template as u32;
            gs.items[cursor] = core::types::Item::default();
            gs.items[cursor].used = USE_ACTIVE;
            gs.items[cursor].temp = required_template as u16;
            gs.characters[cn].citem = cursor as u32;

            assert!(use_create_item2(gs, cn, source));

            assert_eq!(gs.characters[cn].citem, 0);
            assert_eq!(gs.items[cursor].used, USE_EMPTY);
            let created = gs.characters[cn].item[0] as usize;
            assert_eq!(gs.items[created].temp as usize, reward_template);
        });
    }

    #[test]
    fn use_spawn_requires_matching_cursor_template() {
        with_test_gs(|gs| {
            let (cn, _nr) = add_test_player(gs);
            let source = 1;
            let cursor = 2;
            let required_template = 77;
            make_source_item(gs, source);
            gs.items[source].data[1] = required_template as u32;

            assert!(!use_spawn(gs, cn, source));

            gs.items[cursor] = core::types::Item::default();
            gs.items[cursor].used = USE_ACTIVE;
            gs.items[cursor].temp = (required_template - 1) as u16;
            gs.characters[cn].citem = cursor as u32;
            assert!(!use_spawn(gs, cn, source));
            assert_eq!(gs.characters[cn].citem, cursor as u32);

            gs.items[cursor].temp = required_template as u16;
            assert!(use_spawn(gs, cn, source));
            assert_eq!(gs.characters[cn].citem, 0);
            assert_eq!(gs.items[cursor].used, USE_EMPTY);
        });
    }

    #[test]
    fn use_pile_removes_original_pile_and_releases_its_tile() {
        with_test_gs(|gs| {
            let (cn, _nr) = add_test_player(gs);
            let pile = 1;
            make_source_item(gs, pile);
            gs.items[pile].data[0] = 0;
            let map_idx =
                gs.items[pile].x as usize + gs.items[pile].y as usize * SERVER_MAPX as usize;
            gs.map[map_idx].it = pile as u32;

            assert!(use_pile(gs, cn, pile));

            assert_eq!(gs.items[pile].used, USE_EMPTY);
            assert_ne!(gs.map[map_idx].it, pile as u32);
        });
    }

    #[test]
    fn use_garbage_disposes_cursor_item() {
        with_test_gs(|gs| {
            let (cn, _nr) = add_test_player(gs);
            let garbage = 1;
            let cursor = 2;
            make_source_item(gs, garbage);
            gs.items[cursor] = core::types::Item::default();
            gs.items[cursor].used = USE_ACTIVE;
            write_ascii_into_fixed(&mut gs.items[cursor].reference, "old boot");
            gs.characters[cn].citem = cursor as u32;

            assert!(use_garbage(gs, cn, garbage));

            assert_eq!(gs.characters[cn].citem, 0);
            assert_eq!(gs.items[cursor].used, USE_EMPTY);
        });
    }

    #[test]
    fn use_garbage_disposes_cursor_money() {
        with_test_gs(|gs| {
            let (cn, _nr) = add_test_player(gs);
            let garbage = 1;
            make_source_item(gs, garbage);
            gs.characters[cn].citem = 0x8000_0123;

            assert!(use_garbage(gs, cn, garbage));

            assert_eq!(gs.characters[cn].citem, 0);
        });
    }
}
