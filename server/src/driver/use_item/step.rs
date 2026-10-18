use crate::effect::EffectManager;
use crate::game_state::GameState;
use crate::god::God;
use crate::helpers::{self};
use crate::populate::pop_create_char;
use crate::{driver, player, populate};
use core::constants::{CharacterFlags, ItemFlags, NT_HITME, SERVER_MAPX, USE_EMPTY};
use core::traits;

/// Handles the legacy `trap1` item-use hook.
///
/// # Arguments
///
/// * `gs` - Active game state used by this legacy driver hook.
/// * `cn` - Character index executing this legacy driver hook.
/// * `item_idx` - Value passed through to the legacy `trap1` hook.
///
/// # Panics
///
/// * Panics if any legacy id or index parameter used by `trap1` is outside the corresponding game-state collection.
pub fn trap1(gs: &mut GameState, cn: usize, item_idx: usize) {
    let n = gs.items[item_idx].data[1] as usize;
    if n != 0 {
        let in2 = gs.map[n].it as usize;
        if in2 != 0 {
            let (active, data0) = (gs.items[in2].active, gs.items[in2].data[0]);
            if active != 0 || data0 != 0 {
                gs.do_character_log(
                    cn,
                    core::types::FontColor::Red,
                    "You stepped on a trap, but nothing happened!",
                );
                return;
            }
        }
    }

    let slot = helpers::random_mod_usize(12);
    let in_worn = gs.characters[cn].worn[slot];

    if in_worn != 0 {
        let item_name = gs.items[in_worn as usize].get_name().to_owned();
        gs.do_character_log(
            cn,
            core::types::FontColor::Red,
            &format!(
                "You triggered an acid attack. Your {} desintegrated.",
                item_name
            ),
        );
        log::info!(
            "Character {} stepped on Acid Trap, {} vanished",
            cn,
            item_name
        );

        gs.items[in_worn as usize].used = USE_EMPTY;
        gs.characters[cn].worn[slot] = 0;
        gs.do_update_char(cn);
    } else {
        gs.do_character_log(
            cn,
            core::types::FontColor::Red,
            "You triggered an acid attack, but it hit only your skin.",
        );
        log::info!("Character {} stepped on Acid Trap", cn);
        gs.do_hurt(0, cn, 350, 0);
    }
}

/// Handles the legacy `trap2` item-use hook.
///
/// # Arguments
///
/// * `gs` - Active game state used by this legacy driver hook.
/// * `cn` - Character index executing this legacy driver hook.
/// * `tmp` - Value passed through to the legacy `trap2` hook.
///
/// # Panics
///
/// * Panics if any legacy id or index parameter used by `trap2` is outside the corresponding game-state collection.
pub fn trap2(gs: &mut GameState, cn: usize, tmp: usize) {
    let cc = match pop_create_char(gs, tmp, false) {
        Some(cc) => cc,
        None => return,
    };

    let (ch_x, ch_y) = { (gs.characters[cn].x as usize, gs.characters[cn].y as usize) };

    if !God::drop_char_fuzzy(gs, cc, ch_x, ch_y) {
        log::error!("trap2: drop failed");
        gs.characters[cc].used = USE_EMPTY;
        return;
    }

    gs.characters[cc].attack_cn = cn as u16;
    gs.do_update_char(cc);
}

/// Handles the legacy `start_trap` item-use hook.
///
/// # Arguments
///
/// * `gs` - Active game state used by this legacy driver hook.
/// * `cn` - Character index executing this legacy driver hook.
/// * `item_idx` - Value passed through to the legacy `start_trap` hook.
///
/// # Panics
///
/// * Panics if any legacy id or index parameter used by `start_trap` is outside the corresponding game-state collection.
pub fn start_trap(gs: &mut GameState, cn: usize, item_idx: usize) {
    let (duration, light0, light1, x, y) = {
        let item = &gs.items[item_idx];
        (item.duration, item.light[0], item.light[1], item.x, item.y)
    };

    if duration != 0 {
        gs.items[item_idx].active = duration;

        if light0 != light1 && x > 0 {
            gs.do_add_light(
                i32::from(x),
                i32::from(y),
                i32::from(light1) - i32::from(light0),
            );
        }
    }

    let trap_type = gs.items[item_idx].data[0];

    match trap_type {
        0 => {
            log::info!("Character {} stepped on Arrow Trap", cn);
            gs.do_character_log(cn, core::types::FontColor::Red, "You feel a sudden pain!\n");
            gs.do_hurt(0, cn, 250, 0);
        }
        1 => {
            log::info!("Character {} stepped on Attack Trigger Trap", cn);
            gs.do_character_log(
                cn,
                core::types::FontColor::Yellow,
                "You hear a loud croaking noise!",
            );
            gs.do_area_notify(
                cn as i32,
                0,
                i32::from(x),
                i32::from(y),
                i32::from(NT_HITME),
                cn as i32,
                0,
                0,
                0,
            );
        }
        2 => trap1(gs, cn, item_idx),
        3 => trap2(gs, cn, 323),
        4 => trap2(gs, cn, 324),
        _ => {
            gs.do_character_log(
                cn,
                core::types::FontColor::Yellow,
                "Phew. Must be your lucky day today.",
            );
        }
    }
}

/// Handles the legacy `step_trap` item-use hook.
///
/// # Arguments
///
/// * `gs` - Active game state used by this legacy driver hook.
/// * `cn` - Character index executing this legacy driver hook.
/// * `item_idx` - Value passed through to the legacy `step_trap` hook.
///
/// # Returns
///
/// * Value produced by the legacy `step_trap` implementation.
///
/// # Panics
///
/// * Panics if any legacy id or index parameter used by `step_trap` is outside the corresponding game-state collection.
fn step_trap(gs: &mut GameState, cn: usize, item_idx: usize) -> i32 {
    let is_player = gs.characters[cn].flags & CharacterFlags::Player.bits() != 0;

    if is_player {
        start_trap(gs, cn, item_idx);
    } else {
        gs.do_character_log(
            cn,
            core::types::FontColor::Green,
            "You stepped on a trap. Fortunately, nothing happened.",
        );
    }

    0
}

/// Handles the legacy `step_trap_remove` item-use hook.
///
/// # Arguments
///
/// * `gs` - Active game state used by this legacy driver hook.
/// * `_cn` - Character index reserved for this legacy driver hook.
/// * `item_idx` - Value passed through to the legacy `step_trap_remove` hook.
///
/// # Panics
///
/// * Panics if any legacy id or index parameter used by `step_trap_remove` is outside the corresponding game-state collection.
fn step_trap_remove(gs: &mut GameState, _cn: usize, item_idx: usize) {
    let (active, light0, light1, x, y) = {
        let item = &gs.items[item_idx];
        (item.active, item.light[0], item.light[1], item.x, item.y)
    };

    if active != 0 {
        gs.items[item_idx].active = 0;

        if light0 != light1 && x > 0 {
            gs.do_add_light(
                i32::from(x),
                i32::from(y),
                i32::from(light0) - i32::from(light1),
            );
        }
    }
}

/// Handles the legacy `step_portal1_lab13` item-use hook.
///
/// # Arguments
///
/// * `gs` - Active game state used by this legacy driver hook.
/// * `cn` - Character index executing this legacy driver hook.
/// * `_item_idx` - Value passed through to the legacy `step_portal1_lab13` hook.
///
/// # Returns
///
/// * Value produced by the legacy `step_portal1_lab13` implementation.
///
/// # Panics
///
/// * Panics if any legacy id or index parameter used by `step_portal1_lab13` is outside the corresponding game-state collection.
fn step_portal1_lab13(gs: &mut GameState, cn: usize, _item_idx: usize) -> i32 {
    // Check kindred
    let kindred = gs.characters[cn].kindred as u32;
    if (kindred & traits::KIN_HARAKIM) == 0
        && (kindred & traits::KIN_TEMPLAR) == 0
        && (kindred & traits::KIN_MERCENARY) == 0
        && (kindred & traits::KIN_SORCERER) == 0
        && (kindred & traits::KIN_WARRIOR) == 0
    {
        gs.do_character_log(
            cn,
            core::types::FontColor::Red,
            "This portal opens only for Harakim, Templars, Mercenaries, Warrior and Sorcerers.",
        );
        return -1;
    }

    // Check for items
    let mut has_items = false;

    let citem = gs.characters[cn].citem;
    if citem != 0 {
        has_items = true;
    }

    if !has_items {
        for n in 0..40 {
            let item_id = gs.characters[cn].item[n];
            if item_id != 0 {
                has_items = true;
                break;
            }
        }
    }

    if !has_items {
        for n in 0..20 {
            let worn_id = gs.characters[cn].worn[n];
            if worn_id != 0 {
                has_items = true;
                break;
            }
        }
    }

    if has_items {
        gs.do_character_log(
            cn,
            core::types::FontColor::Red,
            "You may not pass unless you leave all your items behind.",
        );
        return -1;
    }

    // Remove all spells
    for n in 0..20 {
        let spell_id = gs.characters[cn].spell[n];
        if spell_id != 0 {
            gs.items[spell_id as usize].used = USE_EMPTY;
            gs.characters[cn].spell[n] = 0;
        }
    }

    gs.do_update_char(cn);

    1
}

/// Handles the legacy `step_portal2_lab13` item-use hook.
///
/// # Arguments
///
/// * `gs` - Active game state used by this legacy driver hook.
/// * `cn` - Character index executing this legacy driver hook.
/// * `_item_idx` - Value passed through to the legacy `step_portal2_lab13` hook.
///
/// # Returns
///
/// * Value produced by the legacy `step_portal2_lab13` implementation.
///
/// # Panics
///
/// * Panics if any legacy id or index parameter used by `step_portal2_lab13` is outside the corresponding game-state collection.
fn step_portal2_lab13(gs: &mut GameState, cn: usize, _item_idx: usize) -> i32 {
    let is_player = gs.characters[cn].flags & CharacterFlags::Player.bits() != 0;
    if !is_player {
        return -1;
    }

    // Check area 1: x=48-80, y=594-608
    let mut flag = 0;
    for x in 48..=80 {
        for y in 594..=608 {
            let m = x + y * SERVER_MAPX as usize;
            let co = gs.map[m].ch;
            if co != 0 && co != cn as u32 {
                let is_other_player =
                    gs.characters[co as usize].flags & CharacterFlags::Player.bits() != 0;
                if is_other_player {
                    flag = 1;
                }
            }

            let in2 = gs.map[m].it;
            if in2 != 0 {
                let temp = gs.items[in2 as usize].temp;
                if temp == 664 || temp == 170 {
                    flag = 2;
                    break;
                }
            }

            // Match original C: if we saw another player on this tile (flag=1),
            // we still check for an expiring item (flag=2) on the same tile.
            // Only after the item check do we stop scanning.
            if flag == 1 {
                break;
            }
        }
        if flag != 0 {
            break;
        }
    }

    // Check area 2: x=38-48, y=593-602
    if flag == 0 {
        for x in 38..=48 {
            for y in 593..=602 {
                let m = x + y * SERVER_MAPX as usize;
                let co = gs.map[m].ch;
                if co != 0 && co != cn as u32 {
                    let is_other_player =
                        gs.characters[co as usize].flags & CharacterFlags::Player.bits() != 0;
                    if is_other_player {
                        flag = 1;
                    }
                }

                let in2 = gs.map[m].it;
                if in2 != 0 {
                    let temp = gs.items[in2 as usize].temp;
                    if temp == 664 || temp == 170 {
                        flag = 2;
                        break;
                    }
                }

                if flag == 1 {
                    break;
                }
            }
            if flag != 0 {
                break;
            }
        }
    }

    if flag == 2 {
        gs.do_character_log(
            cn,
            core::types::FontColor::Red,
            "The Final Test is waiting for a certain item to expire, please try again later.",
        );
        return -1;
    }

    if flag == 1 {
        gs.do_character_log(
            cn,
            core::types::FontColor::Red,
            "You may not pass while another player is inside.",
        );
        return -1;
    }

    // Check for gatekeeper (character template 51)
    flag = 0;
    for n in 0..core::constants::MAXCHARS {
        let (used, flags, temp, a_hp, hp5, a_mana, mana5) = {
            (
                gs.characters[n].used,
                gs.characters[n].flags,
                gs.characters[n].temp,
                gs.characters[n].a_hp,
                gs.characters[n].hp[5],
                gs.characters[n].a_mana,
                gs.characters[n].mana[5],
            )
        };

        if used != core::constants::USE_ACTIVE || (flags & CharacterFlags::Body.bits()) != 0 {
            continue;
        }
        if temp != 51 {
            continue;
        }
        if a_hp > (i32::from(hp5) * 900) && a_mana > (i32::from(mana5) * 900) {
            flag = 1;
        }
        break;
    }

    if flag == 0 {
        gs.do_character_log(
            cn,
            core::types::FontColor::Red,
            "The Gatekeeper is currently busy. Please try again in a few minutes.",
        );
        return -1;
    }

    // Check if doors are closed (item 15220)
    let door_data = gs.items[15220].data[1];
    if door_data == 0 {
        gs.do_character_log(
            cn,
            core::types::FontColor::Red,
            "The doors aren't closed again yet. Please try again in a few minutes.",
        );
        return -1;
    }

    // Remove all spells
    for n in 0..20 {
        let spell_id = gs.characters[cn].spell[n];
        if spell_id != 0 {
            gs.items[spell_id as usize].used = USE_EMPTY;
            gs.characters[cn].spell[n] = 0;
        }
    }

    // Remove items with temp 664
    for n in 0..40 {
        let item_id = gs.characters[cn].item[n];
        if item_id != 0 {
            let temp = gs.items[item_id as usize].temp;
            if temp == 664 {
                gs.characters[cn].item[n] = 0;
                gs.items[item_id as usize].used = USE_EMPTY;
            }
        }
    }

    let citem = gs.characters[cn].citem;
    if citem != 0 && (citem & 0x80000000) == 0 {
        let temp = gs.items[citem as usize].temp;
        if temp == 664 {
            gs.characters[cn].citem = 0;
            gs.items[citem as usize].used = USE_EMPTY;
        }
    }
    gs.do_update_char(cn);

    1
}

/// Handles the legacy `step_portal_arena` item-use hook.
///
/// # Arguments
///
/// * `gs` - Active game state used by this legacy driver hook.
/// * `cn` - Character index executing this legacy driver hook.
/// * `item_idx` - Value passed through to the legacy `step_portal_arena` hook.
///
/// # Returns
///
/// * Value produced by the legacy `step_portal_arena` implementation.
///
/// # Panics
///
/// * Panics if any legacy id or index parameter used by `step_portal_arena` is outside the corresponding game-state collection.
fn step_portal_arena(gs: &mut GameState, cn: usize, item_idx: usize) -> i32 {
    // Check for arena token (temp 687) in citem
    let citem = gs.characters[cn].citem;
    let mut flag = 0;
    if citem != 0 && (citem & 0x80000000) == 0 {
        let temp = gs.items[citem as usize].temp;
        if temp == 687 {
            gs.characters[cn].citem = 0;
            gs.items[citem as usize].used = USE_EMPTY;

            flag = 1;
        }
    }

    // Check inventory for token
    for n in 0..40 {
        let item_id = gs.characters[cn].item[n];
        if item_id != 0 {
            let temp = gs.items[item_id as usize].temp;
            if temp == 687 {
                flag = 1;

                gs.characters[cn].item[n] = 0;
                gs.items[item_id as usize].used = USE_EMPTY;
            }
        }
    }

    if flag == 1 {
        gs.do_character_log(
            cn,
            core::types::FontColor::Yellow,
            "A winner! You gain one arena-rank!",
        );

        gs.characters[cn].data[22] += 1;
        gs.characters[cn].data[23] = 1;

        return 1;
    }

    // Get arena rank
    let nr = gs.characters[cn].data[22] as usize;
    let nr = nr + 364;
    if nr > 381 {
        gs.do_character_log(
            cn,
            core::types::FontColor::Yellow,
            "Please tell the gods to add more potent monsters to the arena.",
        );
        return -1;
    }

    // Get arena bounds
    let (data1, data2) = (gs.items[item_idx].data[1], gs.items[item_idx].data[2]);
    let xs = (data1 as usize) % SERVER_MAPX as usize;
    let ys = (data1 as usize) / SERVER_MAPX as usize;
    let xe = (data2 as usize) % SERVER_MAPX as usize;
    let ye = (data2 as usize) / SERVER_MAPX as usize;

    // Check if character is forfeiting
    let (frx, fry) = {
        (
            gs.characters[cn].frx as usize,
            gs.characters[cn].fry as usize,
        )
    };
    if frx >= xs && frx <= xe && fry >= ys && fry <= ye {
        gs.do_character_log(
            cn,
            core::types::FontColor::Yellow,
            "You forfeit this fight.",
        );
        return 1;
    }

    // Check if arena is occupied
    for x in xs..=xe {
        for y in ys..=ye {
            let m = x + y * SERVER_MAPX as usize;
            let occupied = gs.map[m].ch != 0;
            if occupied {
                gs.do_character_log(
                    cn,
                    core::types::FontColor::Yellow,
                    "The arena is busy. Please come back later.",
                );
                return -1;
            }
        }
    }

    // Create enemy
    let co = match populate::pop_create_char(gs, nr, false) {
        Some(co) => co,
        None => {
            gs.do_character_log(
                cn,
                core::types::FontColor::Red,
                "Please tell the gods that the arena isn't working.",
            );
            return -1;
        }
    };

    let data0 = gs.items[item_idx].data[0];
    let drop_x = (data0 as usize) % SERVER_MAPX as usize;
    let drop_y = (data0 as usize) / SERVER_MAPX as usize;

    if !God::drop_char_fuzzy(gs, co, drop_x, drop_y) {
        gs.do_character_log(
            cn,
            core::types::FontColor::Red,
            "Please tell the gods that the arena isn't working.",
        );
        return -1;
    }

    gs.characters[co].data[64] = gs.globals.ticker + (core::constants::TICKS * 60 * 5);

    // Create arena token
    if let Some(in2) = God::create_item(gs, 687) {
        God::give_character_item(gs, co, in2);
    }

    gs.characters[cn].data[23] = 0;

    1
}

/// Handles the legacy `step_teleport` item-use hook.
///
/// # Arguments
///
/// * `gs` - Active game state used by this legacy driver hook.
/// * `cn` - Character index executing this legacy driver hook.
/// * `item_idx` - Value passed through to the legacy `step_teleport` hook.
///
/// # Returns
///
/// * Value produced by the legacy `step_teleport` implementation.
///
/// # Panics
///
/// * Panics if any legacy id or index parameter used by `step_teleport` is outside the corresponding game-state collection.
fn step_teleport(gs: &mut GameState, cn: usize, item_idx: usize) -> i32 {
    if cn == 0 {
        log::error!("step_teleport(gs, ): cn = 0");
        return -1;
    }

    let (x, y) = {
        (
            gs.items[item_idx].data[0] as usize,
            gs.items[item_idx].data[1] as usize,
        )
    };

    if x >= SERVER_MAPX as usize || y >= (SERVER_MAPX * 2) as usize {
        log::error!("step_teleport(gs, ): bad coordinates in item {}", item_idx);
        return -1;
    }

    let m = x + y * SERVER_MAPX as usize;

    // Check for unoccupied landing spot
    let loc_off: [isize; 5] = [0, -(SERVER_MAPX as isize), SERVER_MAPX as isize, 1, -1];
    let mut m3 = 0;

    for offset in loc_off.iter() {
        let m2 = (m as isize + offset) as usize;
        if m2 >= (SERVER_MAPX * SERVER_MAPX * 2) as usize {
            continue;
        }

        let (map_flags, ch, to_ch, it) = (
            gs.map[m2].flags,
            gs.map[m2].ch,
            gs.map[m2].to_ch,
            gs.map[m2].it,
        );

        if (map_flags & u64::from(core::constants::MF_MOVEBLOCK)) != 0 {
            continue;
        }
        if ch != 0 {
            continue;
        }
        if to_ch != 0 {
            continue;
        }
        if it != 0 {
            let it_flags = gs.items[it as usize].flags;
            if (it_flags & ItemFlags::IF_MOVEBLOCK.bits()) != 0 {
                continue;
            }
        }
        if (map_flags & u64::from(core::constants::MF_TAVERN | core::constants::MF_DEATHTRAP)) != 0
        {
            continue;
        }

        m3 = m2;
        break;
    }

    if m3 == 0 {
        // Target occupied: fail silently
        return -1;
    }

    // Add departure effect
    EffectManager::fx_add_effect(
        gs,
        6,
        0,
        i32::from(gs.characters[cn].x),
        i32::from(gs.characters[cn].y),
        0,
    );

    player::map::plr_map_remove(gs, cn);

    // Update character position
    gs.characters[cn].status = 0;
    gs.characters[cn].attack_cn = 0;
    gs.characters[cn].skill_nr = 0;
    gs.characters[cn].goto_x = 0;

    // Set new position
    gs.map[m3].ch = cn as u32;
    gs.map[m3].to_ch = 0;
    gs.characters[cn].x = (m3 % SERVER_MAPX as usize) as i16;
    gs.characters[cn].y = (m3 / SERVER_MAPX as usize) as i16;

    let (new_x, new_y) = (gs.characters[cn].x, gs.characters[cn].y);
    gs.do_area_notify(
        cn as i32,
        0,
        i32::from(new_x),
        i32::from(new_y),
        1,
        cn as i32,
        0,
        0,
        0,
    );
    // NT_SEE = 1

    // Add arrival effect
    EffectManager::fx_add_effect(
        gs,
        6,
        0,
        i32::from(gs.characters[cn].x),
        i32::from(gs.characters[cn].y),
        0,
    );

    2 // TELEPORT_SUCCESS
}

/// Handles the legacy `step_firefloor` item-use hook.
///
/// # Arguments
///
/// * `gs` - Active game state used by this legacy driver hook.
/// * `cn` - Character index executing this legacy driver hook.
/// * `item_idx` - Value passed through to the legacy `step_firefloor` hook.
///
/// # Returns
///
/// * Value produced by the legacy `step_firefloor` implementation.
///
/// # Panics
///
/// * Panics if any legacy id or index parameter used by `step_firefloor` is outside the corresponding game-state collection.
fn step_firefloor(gs: &mut GameState, cn: usize, item_idx: usize) -> i32 {
    gs.do_character_log(cn, core::types::FontColor::Red, "Outch!\n");

    let in2 = match God::create_item(gs, 1) {
        Some(idx) => idx,
        None => return 0,
    };

    gs.items[in2].name[..4].copy_from_slice(b"Fire");
    gs.items[in2].reference[..4].copy_from_slice(b"fire");
    gs.items[in2].description[..5].copy_from_slice(b"Fire.");
    gs.items[in2].hp[0] = -5000;
    gs.items[in2].active = 1;
    gs.items[in2].duration = 1;
    gs.items[in2].flags = ItemFlags::IF_SPELL.bits() | ItemFlags::IF_PERMSPELL.bits();

    let (temp, sprite1) = (gs.items[item_idx].temp, gs.items[item_idx].sprite[1]);

    gs.items[in2].temp = temp;
    gs.items[in2].sprite[1] = sprite1;

    driver::add_spell(gs, cn, in2);

    0
}

/// Handles the legacy `step_firefloor_remove` item-use hook.
///
/// # Arguments
///
/// * `gs` - Active game state used by this legacy driver hook.
/// * `cn` - Character index executing this legacy driver hook.
/// * `item_idx` - Value passed through to the legacy `step_firefloor_remove` hook.
///
/// # Panics
///
/// * Panics if any legacy id or index parameter used by `step_firefloor_remove` is outside the corresponding game-state collection.
fn step_firefloor_remove(gs: &mut GameState, cn: usize, item_idx: usize) {
    let temp = gs.items[item_idx].temp;

    for n in 0..20 {
        let in2 = gs.characters[cn].spell[n];
        if in2 != 0 {
            let spell_temp = gs.items[in2 as usize].temp;
            if spell_temp == temp {
                gs.items[in2 as usize].used = USE_EMPTY;
                gs.characters[cn].spell[n] = 0;

                return;
            }
        }
    }
}

/// Handles the legacy `step_driver` item-use hook.
///
/// # Arguments
///
/// * `gs` - Active game state used by this legacy driver hook.
/// * `cn` - Character index executing this legacy driver hook.
/// * `item_idx` - Value passed through to the legacy `step_driver` hook.
///
/// # Returns
///
/// * Value produced by the legacy `step_driver` implementation.
///
/// # Panics
///
/// * Panics if any legacy id or index parameter used by `step_driver` is outside the corresponding game-state collection.
pub fn step_driver(gs: &mut GameState, cn: usize, item_idx: usize) -> i32 {
    let driver = gs.items[item_idx].driver;

    match driver {
        36 => step_portal1_lab13(gs, cn, item_idx),
        37 => step_trap(gs, cn, item_idx),
        38 => step_portal2_lab13(gs, cn, item_idx),
        47 => step_portal_arena(gs, cn, item_idx),
        62 => step_teleport(gs, cn, item_idx),
        69 => step_firefloor(gs, cn, item_idx),
        _ => {
            log::error!(
                "unknown step driver {} for item {} ({})",
                gs.items[item_idx].driver,
                gs.items[item_idx].get_name(),
                item_idx
            );

            0
        }
    }
}

/// Handles the legacy `step_driver_remove` item-use hook.
///
/// # Arguments
///
/// * `gs` - Active game state used by this legacy driver hook.
/// * `cn` - Character index executing this legacy driver hook.
/// * `item_idx` - Value passed through to the legacy `step_driver_remove` hook.
///
/// # Panics
///
/// * Panics if any legacy id or index parameter used by `step_driver_remove` is outside the corresponding game-state collection.
pub fn step_driver_remove(gs: &mut GameState, cn: usize, item_idx: usize) {
    let driver = gs.items[item_idx].driver;

    match driver {
        36 => {}
        37 => step_trap_remove(gs, cn, item_idx),
        38 => {}
        47 => {}
        62 => {}
        69 => step_firefloor_remove(gs, cn, item_idx),
        _ => {
            log::error!(
                "unknown step driver {} for item {} ({})",
                gs.items[item_idx].driver,
                gs.items[item_idx].get_name(),
                item_idx
            );
        }
    }
}
