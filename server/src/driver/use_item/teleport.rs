use crate::driver;
use crate::effect::EffectManager;
use crate::game_state::GameState;
use crate::god::God;
use crate::helpers::{self};
use core::constants::{CharacterFlags, ItemFlags, TICKS, USE_EMPTY};
use core::skills::{self};
use core::string_operations::c_string_to_str;
use core::traits;

/// Handles the legacy `finish_laby_teleport` item-use hook.
///
/// # Arguments
///
/// * `gs` - Active game state used by this legacy driver hook.
/// * `cn` - Character index executing this legacy driver hook.
/// * `nr` - Legacy numeric identifier passed to the hook.
/// * `exp` - Value passed through to the legacy `finish_laby_teleport` hook.
///
/// # Returns
///
/// * `true` when `finish_laby_teleport` reports that it handled the legacy action, otherwise `false`.
///
/// # Panics
///
/// * Panics if any legacy id or index parameter used by `finish_laby_teleport` is outside the corresponding game-state collection.
pub fn finish_laby_teleport(gs: &mut GameState, cn: usize, nr: usize, exp: usize) -> bool {
    let current_progress = gs.characters[cn].data[20];
    let (x, y) = (gs.characters[cn].x, gs.characters[cn].y);

    if (current_progress as usize) < nr {
        gs.characters[cn].data[20] = nr as i32;

        let ordinal = match nr {
            1 => "st",
            2 => "nd",
            3 => "rd",
            _ => "th",
        };

        gs.do_character_log(
            cn,
            core::types::FontColor::Green,
            &format!(
                "You have solved the {}{} part of the Labyrinth.\n",
                nr, ordinal
            ),
        );

        gs.do_give_exp(cn, exp as i32, 0, -1);
        crate::player::commands::resend_completion_data_for_character(gs, cn);
    }

    let citem = gs.characters[cn].citem;
    if citem != 0 && (citem & 0x80000000) == 0 {
        let citem = citem as usize;
        if (gs.items[citem].flags & ItemFlags::IF_LABYDESTROY.bits()) != 0 {
            let item_ref = c_string_to_str(&gs.items[citem].reference).to_owned();
            gs.characters[cn].citem = 0;
            gs.items[citem].used = USE_EMPTY;
            gs.do_character_log(
                cn,
                core::types::FontColor::Yellow,
                &format!("Your {} vanished.\n", item_ref),
            );
        }
    }

    for n in 0..40 {
        let item_idx = gs.characters[cn].item[n] as usize;
        if item_idx != 0 && (gs.items[item_idx].flags & ItemFlags::IF_LABYDESTROY.bits()) != 0 {
            let item_ref = c_string_to_str(&gs.items[item_idx].reference).to_owned();
            gs.characters[cn].item[n] = 0;
            gs.items[item_idx].used = USE_EMPTY;
            gs.do_character_log(
                cn,
                core::types::FontColor::Yellow,
                &format!("Your {} vanished.\n", item_ref),
            );
        }
    }

    for n in 0..20 {
        let item_idx = gs.characters[cn].worn[n] as usize;
        if item_idx != 0 && (gs.items[item_idx].flags & ItemFlags::IF_LABYDESTROY.bits()) != 0 {
            let item_ref = c_string_to_str(&gs.items[item_idx].reference).to_owned();
            gs.characters[cn].worn[n] = 0;
            gs.items[item_idx].used = USE_EMPTY;
            gs.do_character_log(
                cn,
                core::types::FontColor::Yellow,
                &format!("Your {} vanished.\n", item_ref),
            );
        }
    }

    for n in 0..20 {
        let spell_idx = gs.characters[cn].spell[n] as usize;
        if spell_idx != 0 {
            let item_name = c_string_to_str(&gs.items[spell_idx].name).to_owned();
            gs.characters[cn].spell[n] = 0;
            gs.items[spell_idx].used = USE_EMPTY;
            gs.do_character_log(
                cn,
                core::types::FontColor::Yellow,
                &format!("Your {} vanished.\n", item_name),
            );
        }
    }

    // Add effects and transfer character
    EffectManager::fx_add_effect(gs, 6, 0, i32::from(x), i32::from(y), 0);
    God::transfer_char(gs, cn, 512, 512); // TODO: Shouldn't this be their temple coords?
    EffectManager::fx_add_effect(gs, 6, 0, 512_i32, 512_i32, 0);

    let (x, y) = (gs.characters[cn].x, gs.characters[cn].y);
    gs.characters[cn].temple_x = x as u16;
    gs.characters[cn].temple_y = y as u16;
    gs.characters[cn].tavern_x = x as u16;
    gs.characters[cn].tavern_y = y as u16;

    true
}

/// Handles the legacy `is_nolab_item` item-use hook.
///
/// # Arguments
///
/// * `gs` - Active game state used by this legacy driver hook.
/// * `item_idx` - Value passed through to the legacy `is_nolab_item` hook.
///
/// # Returns
///
/// * `true` when `is_nolab_item` reports that it handled the legacy action, otherwise `false`.
///
/// # Panics
///
/// * Panics if any legacy id or index parameter used by `is_nolab_item` is outside the corresponding game-state collection.
pub fn is_nolab_item(gs: &mut GameState, item_idx: usize) -> bool {
    if !core::types::Item::is_sane_item(item_idx) {
        return false;
    }
    let temp = gs.items[item_idx].temp;
    matches!(
        temp,
        331   // tavern scroll
        | 500   // lag scroll
        | 592   // gorn scroll
        | 903   // forest scroll
        | 1114  // staffers corner scroll
        | 1118  // inn scroll
        | 1144 // arena scroll
    )
}

/// Handles the legacy `teleport` item-use hook.
///
/// # Arguments
///
/// * `gs` - Active game state used by this legacy driver hook.
/// * `cn` - Character index executing this legacy driver hook.
/// * `item_idx` - Value passed through to the legacy `teleport` hook.
///
/// # Returns
///
/// * `true` when `teleport` reports that it handled the legacy action, otherwise `false`.
///
/// # Panics
///
/// * Panics if any legacy id or index parameter used by `teleport` is outside the corresponding game-state collection.
pub fn teleport(gs: &mut GameState, cn: usize, item_idx: usize) -> bool {
    if cn == 0 {
        return true;
    }

    let has_useactivate = (gs.items[item_idx].flags & ItemFlags::IF_USEACTIVATE.bits()) != 0;
    let is_active = gs.items[item_idx].active != 0;

    if has_useactivate && !is_active {
        return true;
    }

    let citem = gs.characters[cn].citem as usize;
    let (x, y) = (gs.characters[cn].x, gs.characters[cn].y);
    if citem != 0 && is_nolab_item(gs, citem) {
        let item_ref = c_string_to_str(&gs.items[citem].reference).to_owned();
        gs.characters[cn].citem = 0;
        gs.items[citem].used = USE_EMPTY;
        gs.do_character_log(
            cn,
            core::types::FontColor::Yellow,
            &format!("Your {} vanished.\n", item_ref),
        );
    }

    for n in 0..40 {
        let inv_item = gs.characters[cn].item[n] as usize;
        if inv_item != 0 && is_nolab_item(gs, inv_item) {
            let item_ref = c_string_to_str(&gs.items[inv_item].reference).to_owned();
            gs.characters[cn].item[n] = 0;
            gs.items[inv_item].used = USE_EMPTY;
            gs.do_character_log(
                cn,
                core::types::FontColor::Yellow,
                &format!("Your {} vanished.\n", item_ref),
            );
        }
    }

    for n in 0..20 {
        let spell_idx = gs.characters[cn].spell[n] as usize;
        if spell_idx != 0 && gs.items[spell_idx].temp == skills::SK_RECALL as u16 {
            gs.characters[cn].spell[n] = 0;
            gs.items[spell_idx].used = USE_EMPTY;
        }
    }

    let data2 = gs.items[item_idx].data[2];
    if data2 != 0 {
        let data3 = gs.items[item_idx].data[3];
        helpers::use_labtransfer(gs, cn, data2 as i32, data3 as i32);
        return true;
    }

    let dest_x = gs.items[item_idx].data[0] as usize;
    let dest_y = gs.items[item_idx].data[1] as usize;

    EffectManager::fx_add_effect(gs, 6, 0, i32::from(x), i32::from(y), 0);
    God::transfer_char(gs, cn, dest_x, dest_y);
    EffectManager::fx_add_effect(gs, 6, 0, dest_x as i32, dest_y as i32, 0);

    true
}

/// Handles the legacy `teleport2` item-use hook.
///
/// # Arguments
///
/// * `gs` - Active game state used by this legacy driver hook.
/// * `cn` - Character index executing this legacy driver hook.
/// * `item_idx` - Value passed through to the legacy `teleport2` hook.
///
/// # Returns
///
/// * `true` when `teleport2` reports that it handled the legacy action, otherwise `false`.
///
/// # Panics
///
/// * Panics if any legacy id or index parameter used by `teleport2` is outside the corresponding game-state collection.
pub fn teleport2(gs: &mut GameState, cn: usize, item_idx: usize) -> bool {
    if cn == 0 {
        return true;
    }

    let dest_x = gs.items[item_idx].data[0];
    let dest_y = gs.items[item_idx].data[1];
    let area_name = format!("({}, {})", dest_x, dest_y);
    log::info!(
        "Used teleport scroll to {},{} ({})",
        dest_x,
        dest_y,
        area_name
    );

    let scroll_time = gs.items[item_idx].data[2];
    let power = gs.items[item_idx].power;
    let ticker = gs.globals.ticker;
    if scroll_time != 0 && scroll_time + TICKS as u32 * 60 * 4 < ticker as u32 {
        let diff = ticker - scroll_time as i32;
        log::info!(
            "Lag Scroll Time Difference: {} ticks ({:.2}s)",
            diff,
            f64::from(diff) / f64::from(TICKS)
        );
        gs.do_character_log(
            cn,
            core::types::FontColor::Yellow,
            "Sorry, this lag scroll was too old. You need to use it four minutes after lagging out or earlier!\n",
        );
        return true;
    }

    // Create a recall spell item
    let spell_item = match God::create_item(gs, 1) {
        Some(id) => id,
        None => {
            log::error!("god_create_item failed in teleport2");
            return false;
        }
    };

    {
        let spell = &mut gs.items[spell_item];
        let name = b"Teleport";
        let recall_duration = core::constants::scale_legacy_ticks_u32(180);
        spell.name[..name.len()].copy_from_slice(name);
        spell.flags |= ItemFlags::IF_SPELL.bits();
        spell.sprite[1] = 90;
        spell.duration = recall_duration;
        spell.active = recall_duration;
        spell.temp = skills::SK_RECALL as u16;
        spell.power = power;
        spell.data[0] = dest_x;
        spell.data[1] = dest_y;
    }

    let added = driver::add_spell(gs, cn, spell_item);
    if added == 0 {
        let spell_name = c_string_to_str(&gs.items[spell_item].name).to_owned();
        gs.do_character_log(
            cn,
            core::types::FontColor::Yellow,
            &format!(
                "Magical interference neutralised the {}'s effect.\n",
                spell_name
            ),
        );
        return false;
    }

    let (x, y) = (gs.characters[cn].x, gs.characters[cn].y);
    crate::effect::EffectManager::fx_add_effect(gs, 7, 0, i32::from(x), i32::from(y), 0);
    true
}

/// Handles the legacy `use_labyrinth` item-use hook.
///
/// # Arguments
///
/// * `gs` - Active game state used by this legacy driver hook.
/// * `cn` - Character index executing this legacy driver hook.
/// * `_item_idx` - Value passed through to the legacy `use_labyrinth` hook.
///
/// # Returns
///
/// * `true` when `use_labyrinth` reports that it handled the legacy action, otherwise `false`.
///
/// # Panics
///
/// * Panics if any legacy id or index parameter used by `use_labyrinth` is outside the corresponding game-state collection.
pub fn use_labyrinth(gs: &mut GameState, cn: usize, _item_idx: usize) -> bool {
    let citem = gs.characters[cn].citem as usize;
    if citem != 0 && is_nolab_item(gs, citem) {
        let item_ref = c_string_to_str(&gs.items[citem].reference).to_owned();
        gs.characters[cn].citem = 0;
        gs.items[citem].used = USE_EMPTY;
        gs.do_character_log(
            cn,
            core::types::FontColor::Yellow,
            &format!("Your {} vanished.\n", item_ref),
        );
    }

    for n in 0..40 {
        let inv_item = gs.characters[cn].item[n] as usize;
        if inv_item != 0 && is_nolab_item(gs, inv_item) {
            let item_ref = c_string_to_str(&gs.items[inv_item].reference).to_owned();
            gs.characters[cn].item[n] = 0;
            gs.items[inv_item].used = USE_EMPTY;
            gs.do_character_log(
                cn,
                core::types::FontColor::Yellow,
                &format!("Your {} vanished.\n", item_ref),
            );
        }
    }

    for n in 0..20 {
        let spell_idx = gs.characters[cn].spell[n] as usize;
        if spell_idx != 0 && gs.items[spell_idx].temp == skills::SK_RECALL as u16 {
            gs.characters[cn].spell[n] = 0;
            gs.items[spell_idx].used = USE_EMPTY;
        }
    }

    let progress = gs.characters[cn].data[20];
    let (x, y) = (gs.characters[cn].x, gs.characters[cn].y);
    let destination = match progress {
        0 => Some((64, 56)),
        1 => Some((95, 207)),
        2 => Some((74, 240)),
        3 => Some((37, 370)),
        4 => Some((114, 390)),
        5 => Some((28, 493)),
        6 => Some((24, 534)),
        7 => Some((118, 667)),
        8 => Some((63, 720)),
        9 => Some((33, 597)),
        _ => None,
    };

    let flag = if let Some((dest_x, dest_y)) = destination {
        EffectManager::fx_add_effect(gs, 6, 0, i32::from(x), i32::from(y), 0);
        let result = God::transfer_char(gs, cn, dest_x, dest_y);
        let (new_x, new_y) = (gs.characters[cn].x, gs.characters[cn].y);
        EffectManager::fx_add_effect(gs, 6, 0, i32::from(new_x), i32::from(new_y), 0);
        result
    } else {
        gs.do_character_log(
            cn,
            core::types::FontColor::Green,
            "You have already solved all existing parts of the labyrinth. Please come back later.\n",
        );
        false
    };

    if flag {
        let (x, y) = (gs.characters[cn].x, gs.characters[cn].y);
        gs.characters[cn].temple_x = x as u16;
        gs.characters[cn].temple_y = y as u16;
        gs.characters[cn].tavern_x = x as u16;
        gs.characters[cn].tavern_y = y as u16;
    }

    true
}

/// Handles the legacy `use_ladder` item-use hook.
///
/// # Arguments
///
/// * `gs` - Active game state used by this legacy driver hook.
/// * `cn` - Character index executing this legacy driver hook.
/// * `item_idx` - Value passed through to the legacy `use_ladder` hook.
///
/// # Returns
///
/// * `true` when `use_ladder` reports that it handled the legacy action, otherwise `false`.
///
/// # Panics
///
/// * Panics if any legacy id or index parameter used by `use_ladder` is outside the corresponding game-state collection.
pub fn use_ladder(gs: &mut GameState, cn: usize, item_idx: usize) -> bool {
    use crate::god::God;
    let item = &gs.items[item_idx];
    let item_x = item.x as usize;
    let item_y = item.y as usize;
    let offset_x = item.data[0] as i32;
    let offset_y = item.data[1] as i32;
    let dest_x = (item_x as i32 + offset_x) as usize;
    let dest_y = (item_y as i32 + offset_y) as usize;

    God::transfer_char(gs, cn, dest_x, dest_y);

    true
}

/// Handles the legacy `teleport3` item-use hook.
///
/// # Arguments
///
/// * `gs` - Active game state used by this legacy driver hook.
/// * `cn` - Character index executing this legacy driver hook.
/// * `item_idx` - Value passed through to the legacy `teleport3` hook.
///
/// # Returns
///
/// * `true` when `teleport3` reports that it handled the legacy action, otherwise `false`.
///
/// # Panics
///
/// * Panics if any legacy id or index parameter used by `teleport3` is outside the corresponding game-state collection.
pub fn teleport3(gs: &mut GameState, cn: usize, item_idx: usize) -> bool {
    if cn == 0 {
        return true;
    }

    // Check if requires activation
    let (needs_activation, is_active) = {
        (
            (gs.items[item_idx].flags & ItemFlags::IF_USEACTIVATE.bits()) != 0,
            gs.items[item_idx].active != 0,
        )
    };

    if needs_activation && !is_active {
        return true;
    }

    // Remove nolab items from citem
    let citem = gs.characters[cn].citem as usize;
    if citem != 0 && is_nolab_item(gs, citem) {
        let item_ref = gs.items[citem].reference;
        gs.characters[cn].citem = 0;
        gs.items[citem].used = USE_EMPTY;

        gs.do_character_log(
            cn,
            core::types::FontColor::Yellow,
            &format!("Your {} vanished.\n", c_string_to_str(&item_ref)),
        );
    }

    // Remove nolab items from inventory
    for n in 0..40 {
        let in2 = gs.characters[cn].item[n] as usize;
        if in2 != 0 && is_nolab_item(gs, in2) {
            let item_ref = gs.items[in2].reference;
            gs.characters[cn].item[n] = 0;
            gs.items[in2].used = USE_EMPTY;

            gs.do_character_log(
                cn,
                core::types::FontColor::Yellow,
                &format!("Your {} vanished.\n", c_string_to_str(&item_ref)),
            );
        }
    }

    // Remove recall spells
    for n in 0..20 {
        let in2 = gs.characters[cn].spell[n] as usize;
        if in2 != 0 {
            let temp = gs.items[in2].temp;
            if temp as usize == skills::SK_RECALL {
                gs.characters[cn].spell[n] = 0;
                gs.items[in2].used = USE_EMPTY;
            }
        }
    }

    // Teleport
    EffectManager::fx_add_effect(
        gs,
        6,
        0,
        i32::from(gs.characters[cn].x),
        i32::from(gs.characters[cn].y),
        0,
    );
    let (dest_x, dest_y) = {
        (
            gs.items[item_idx].data[0] as usize,
            gs.items[item_idx].data[1] as usize,
        )
    };
    God::transfer_char(gs, cn, dest_x, dest_y);
    EffectManager::fx_add_effect(
        gs,
        6,
        0,
        i32::from(gs.characters[cn].x),
        i32::from(gs.characters[cn].y),
        0,
    );

    // Remove IF_LABYDESTROY items from citem
    let citem = gs.characters[cn].citem as usize;
    if citem != 0 && (citem & 0x80000000) == 0 {
        let has_flag = { (gs.items[citem].flags & ItemFlags::IF_LABYDESTROY.bits()) != 0 };
        if has_flag {
            let item_ref = { c_string_to_str(&gs.items[citem].reference).to_owned() };
            gs.characters[cn].citem = 0;
            gs.items[citem].used = USE_EMPTY;
            gs.do_character_log(
                cn,
                core::types::FontColor::Yellow,
                &format!("Your {} vanished.\n", item_ref),
            );
        }
    }

    // Remove IF_LABYDESTROY items from inventory
    for n in 0..40 {
        let in2 = gs.characters[cn].item[n] as usize;
        if in2 != 0 {
            let (has_flag, item_ref) = {
                (
                    (gs.items[in2].flags & ItemFlags::IF_LABYDESTROY.bits()) != 0,
                    gs.items[in2].reference,
                )
            };
            if has_flag {
                gs.characters[cn].item[n] = 0;
                gs.items[in2].used = USE_EMPTY;
                gs.do_character_log(
                    cn,
                    core::types::FontColor::Yellow,
                    &format!("Your {} vanished.\n", c_string_to_str(&item_ref)),
                );
            }
        }
    }

    // Remove IF_LABYDESTROY items from worn
    for n in 0..20 {
        let in2 = gs.characters[cn].worn[n] as usize;
        if in2 != 0 {
            let (has_flag, item_ref) = {
                (
                    (gs.items[in2].flags & ItemFlags::IF_LABYDESTROY.bits()) != 0,
                    gs.items[in2].reference,
                )
            };
            if has_flag {
                gs.characters[cn].worn[n] = 0;
                gs.items[in2].used = USE_EMPTY;
                gs.do_character_log(
                    cn,
                    core::types::FontColor::Yellow,
                    &format!("Your {} vanished.\n", c_string_to_str(&item_ref)),
                );
            }
        }
    }

    // Update temple/tavern coordinates
    let (kindred, is_staff) = {
        (
            gs.characters[cn].kindred,
            (gs.characters[cn].flags & CharacterFlags::Staff.bits()) != 0,
        )
    };

    if (kindred as u32 & core::traits::KIN_PURPLE) != 0 {
        gs.characters[cn].temple_x = 558;
        gs.characters[cn].temple_y = 542;
        gs.characters[cn].tavern_x = 558;
        gs.characters[cn].tavern_y = 542;
    } else if is_staff {
        gs.characters[cn].temple_x = 813;
        gs.characters[cn].temple_y = 165;
        gs.characters[cn].tavern_x = 813;
        gs.characters[cn].tavern_y = 165;
    } else {
        gs.characters[cn].temple_x = 512;
        gs.characters[cn].temple_y = 512;
        gs.characters[cn].tavern_x = 512;
        gs.characters[cn].tavern_y = 512;
    }

    true
}

/// Handles the legacy `use_seyan_portal` item-use hook.
///
/// # Arguments
///
/// * `gs` - Active game state used by this legacy driver hook.
/// * `cn` - Character index executing this legacy driver hook.
/// * `item_idx` - Value passed through to the legacy `use_seyan_portal` hook.
///
/// # Returns
///
/// * `true` when `use_seyan_portal` reports that it handled the legacy action, otherwise `false`.
///
/// # Panics
///
/// * Panics if any legacy id or index parameter used by `use_seyan_portal` is outside the corresponding game-state collection.
pub fn use_seyan_portal(gs: &mut GameState, cn: usize, item_idx: usize) -> bool {
    if cn == 0 {
        return false;
    }

    let (is_seyan, is_male, cn_name) = {
        (
            (gs.characters[cn].kindred & traits::KIN_SEYAN_DU as i32) != 0,
            (gs.characters[cn].kindred & traits::KIN_MALE as i32) != 0,
            gs.characters[cn].get_name().to_owned(),
        )
    };

    if is_seyan {
        gs.do_character_log(
            cn,
            core::types::FontColor::Blue,
            "You're already Seyan'Du, aren't you?\n",
        );
    } else {
        gs.do_character_log(
            cn,
            core::types::FontColor::Blue,
            &format!("The Seyan'Du welcome you among their ranks, {}!\n", cn_name),
        );

        // Change race: 13 for male Seyan'Du, 79 for female Seyan'Du
        if is_male {
            God::racechange(gs, cn, 13);
        } else {
            God::racechange(gs, cn, 79);
        }

        // Give Seyan'Du sword (template 682)
        let in2 = match God::create_item(gs, 682) {
            Some(id) => id,
            None => return false,
        };
        God::give_character_item(gs, cn, in2);
        gs.items[in2].data[0] = cn as u32;
    }

    // Remove IF_LABYDESTROY items from citem
    let citem = gs.characters[cn].citem as usize;
    if citem != 0 && (citem & 0x80000000) == 0 {
        let has_flag = { (gs.items[citem].flags & ItemFlags::IF_LABYDESTROY.bits()) != 0 };
        if has_flag {
            let item_ref = gs.items[citem].reference;
            gs.characters[cn].citem = 0;
            gs.items[citem].used = USE_EMPTY;

            gs.do_character_log(
                cn,
                core::types::FontColor::Green,
                &format!("Your {} vanished.\n", c_string_to_str(&item_ref)),
            );
        }
    }

    // Remove IF_LABYDESTROY items from inventory
    for n in 0..40 {
        let in2 = gs.characters[cn].item[n] as usize;
        if in2 != 0 {
            let (has_flag, item_ref) = {
                (
                    (gs.items[in2].flags & ItemFlags::IF_LABYDESTROY.bits()) != 0,
                    gs.items[in2].reference,
                )
            };
            if has_flag {
                gs.characters[cn].item[n] = 0;
                gs.items[in2].used = USE_EMPTY;
                gs.do_character_log(
                    cn,
                    core::types::FontColor::Green,
                    &format!("Your {} vanished.\n", c_string_to_str(&item_ref)),
                );
            }
        }
    }

    // Remove IF_LABYDESTROY items from worn
    for n in 0..20 {
        let in2 = gs.characters[cn].worn[n] as usize;
        if in2 != 0 {
            let (has_flag, item_ref) = {
                (
                    (gs.items[in2].flags & ItemFlags::IF_LABYDESTROY.bits()) != 0,
                    gs.items[in2].reference,
                )
            };
            if has_flag {
                gs.characters[cn].worn[n] = 0;
                gs.items[in2].used = USE_EMPTY;
                gs.do_character_log(
                    cn,
                    core::types::FontColor::Green,
                    &format!("Your {} vanished.\n", c_string_to_str(&item_ref)),
                );
            }
        }
    }

    // Teleport
    EffectManager::fx_add_effect(
        gs,
        6,
        0,
        i32::from(gs.characters[cn].x),
        i32::from(gs.characters[cn].y),
        0,
    );
    let (dest_x, dest_y) = {
        (
            gs.items[item_idx].data[0] as usize,
            gs.items[item_idx].data[1] as usize,
        )
    };
    God::transfer_char(gs, cn, dest_x, dest_y);
    EffectManager::fx_add_effect(
        gs,
        6,
        0,
        i32::from(gs.characters[cn].x),
        i32::from(gs.characters[cn].y),
        0,
    );

    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::{add_test_player, with_test_gs};
    use core::{
        constants::{ItemFlags, TICKS, USE_ACTIVE, USE_EMPTY},
        string_operations::write_ascii_into_fixed,
    };

    /// Mark a runtime item as active with a readable reference for log paths.
    fn active_item(gs: &mut GameState, item_idx: usize, temp: u16, reference: &str) {
        gs.items[item_idx] = core::types::Item::default();
        gs.items[item_idx].used = USE_ACTIVE;
        gs.items[item_idx].temp = temp;
        write_ascii_into_fixed(&mut gs.items[item_idx].name, reference);
        write_ascii_into_fixed(&mut gs.items[item_idx].reference, reference);
    }

    #[test]
    fn is_nolab_item_recognizes_blocked_scroll_templates_only() {
        with_test_gs(|gs| {
            active_item(gs, 1, 331, "tavern scroll");
            active_item(gs, 2, 332, "ordinary item");

            assert!(is_nolab_item(gs, 1));
            assert!(!is_nolab_item(gs, 2));
            assert!(!is_nolab_item(gs, 0));
        });
    }

    #[test]
    fn teleport_requiring_activation_returns_before_consuming_nolab_cursor() {
        with_test_gs(|gs| {
            let (cn, _nr) = add_test_player(gs);
            active_item(gs, 1, 0, "inactive portal");
            gs.items[1].flags = ItemFlags::IF_USEACTIVATE.bits();
            gs.items[1].data[0] = 512;
            gs.items[1].data[1] = 512;
            active_item(gs, 2, 331, "tavern scroll");
            gs.characters[cn].citem = 2;
            let start = (gs.characters[cn].x, gs.characters[cn].y);

            assert!(teleport(gs, cn, 1));

            assert_eq!((gs.characters[cn].x, gs.characters[cn].y), start);
            assert_eq!(gs.characters[cn].citem, 2);
            assert_eq!(gs.items[2].used, USE_ACTIVE);
        });
    }

    #[test]
    fn teleport_removes_nolab_items_and_recall_spell_before_moving() {
        with_test_gs(|gs| {
            let (cn, _nr) = add_test_player(gs);
            active_item(gs, 1, 0, "portal");
            gs.items[1].data[0] = 512;
            gs.items[1].data[1] = 512;
            active_item(gs, 2, 331, "cursor scroll");
            active_item(gs, 3, 500, "inventory scroll");
            active_item(gs, 4, skills::SK_RECALL as u16, "recall");
            gs.characters[cn].citem = 2;
            gs.characters[cn].item[0] = 3;
            gs.characters[cn].spell[0] = 4;

            assert!(teleport(gs, cn, 1));

            assert_eq!((gs.characters[cn].x, gs.characters[cn].y), (512, 512));
            assert_eq!(gs.characters[cn].citem, 0);
            assert_eq!(gs.characters[cn].item[0], 0);
            assert_eq!(gs.characters[cn].spell[0], 0);
            assert_eq!(gs.items[2].used, USE_EMPTY);
            assert_eq!(gs.items[3].used, USE_EMPTY);
            assert_eq!(gs.items[4].used, USE_EMPTY);
        });
    }

    #[test]
    fn teleport2_stale_lag_scroll_does_not_add_recall_spell() {
        with_test_gs(|gs| {
            let (cn, _nr) = add_test_player(gs);
            active_item(gs, 1, 0, "lag scroll");
            gs.items[1].data[0] = 512;
            gs.items[1].data[1] = 512;
            gs.items[1].data[2] = 1;
            gs.globals.ticker = TICKS * 60 * 5;

            assert!(teleport2(gs, cn, 1));

            assert!(gs.characters[cn].spell.iter().all(|spell| *spell == 0));
        });
    }

    #[test]
    fn use_ladder_moves_by_item_relative_offsets() {
        with_test_gs(|gs| {
            let (cn, _nr) = add_test_player(gs);
            active_item(gs, 1, 0, "ladder");
            gs.items[1].x = 20;
            gs.items[1].y = 30;
            gs.items[1].data[0] = 3;
            gs.items[1].data[1] = 4;

            assert!(use_ladder(gs, cn, 1));

            assert_eq!((gs.characters[cn].x, gs.characters[cn].y), (23, 34));
        });
    }

    #[test]
    fn finish_laby_teleport_updates_progress_and_removes_laby_destroy_items() {
        with_test_gs(|gs| {
            let (cn, _nr) = add_test_player(gs);
            active_item(gs, 1, 0, "cursor relic");
            active_item(gs, 2, 0, "bag relic");
            active_item(gs, 3, 0, "worn relic");
            active_item(gs, 4, 0, "spell");
            for item_idx in 1..=3 {
                gs.items[item_idx].flags = ItemFlags::IF_LABYDESTROY.bits();
            }
            gs.characters[cn].citem = 1;
            gs.characters[cn].item[0] = 2;
            gs.characters[cn].worn[0] = 3;
            gs.characters[cn].spell[0] = 4;

            assert!(finish_laby_teleport(gs, cn, 2, 17));

            assert_eq!(gs.characters[cn].data[20], 2);
            assert_eq!(gs.characters[cn].points_tot, 17);
            assert_eq!((gs.characters[cn].x, gs.characters[cn].y), (512, 512));
            assert_eq!(
                (gs.characters[cn].temple_x, gs.characters[cn].temple_y),
                (512, 512)
            );
            assert_eq!(
                (gs.characters[cn].tavern_x, gs.characters[cn].tavern_y),
                (512, 512)
            );
            assert_eq!(gs.characters[cn].citem, 0);
            assert_eq!(gs.characters[cn].item[0], 0);
            assert_eq!(gs.characters[cn].worn[0], 0);
            assert_eq!(gs.characters[cn].spell[0], 0);
            for item_idx in 1..=4 {
                assert_eq!(gs.items[item_idx].used, USE_EMPTY);
            }
        });
    }

    #[test]
    fn magic_portal_moves_starting_area_home_to_the_mercenary_temple() {
        with_test_gs(|gs| {
            let (cn, _nr) = add_test_player(gs);
            gs.characters[cn].temple_x = core::constants::HOME_START_X as u16;
            gs.characters[cn].temple_y = core::constants::HOME_START_Y as u16;
            gs.characters[cn].tavern_x = core::constants::HOME_START_X as u16;
            gs.characters[cn].tavern_y = core::constants::HOME_START_Y as u16;

            let item_idx = 1;
            gs.items[item_idx] = core::types::Item::default();
            gs.items[item_idx].used = USE_ACTIVE;
            gs.items[item_idx].data[0] = core::constants::HOME_MERCENARY_X as u32;
            gs.items[item_idx].data[1] = core::constants::HOME_MERCENARY_Y as u32;

            assert!(teleport3(gs, cn, item_idx));

            assert_eq!(
                gs.characters[cn].temple_x,
                core::constants::HOME_MERCENARY_X as u16
            );
            assert_eq!(
                gs.characters[cn].temple_y,
                core::constants::HOME_MERCENARY_Y as u16
            );
            assert_eq!(
                gs.characters[cn].tavern_x,
                core::constants::HOME_MERCENARY_X as u16
            );
            assert_eq!(
                gs.characters[cn].tavern_y,
                core::constants::HOME_MERCENARY_Y as u16
            );
        });
    }

    #[test]
    fn teleport3_sets_mercenary_temple_not_purple() {
        with_test_gs(|gs| {
            let (cn, _nr) = add_test_player(gs);
            gs.characters[cn].kindred = core::traits::KIN_MERCENARY as i32;
            gs.items[1] = core::types::Item::default();
            gs.items[1].used = USE_ACTIVE;
            gs.items[1].data[0] = 512;
            gs.items[1].data[1] = 512;

            assert!(teleport3(gs, cn, 1));

            let ch = &gs.characters[cn];
            assert_eq!((ch.temple_x, ch.temple_y), (512, 512));
            assert_eq!((ch.tavern_x, ch.tavern_y), (512, 512));
        });
    }
}
