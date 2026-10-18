use super::{item_damage_citem, use_driver};
use crate::game_state::GameState;
use crate::god::God;
use crate::helpers::{self};
use core::constants::{ItemFlags, SERVER_MAPX, USE_EMPTY};
use core::skills::{self};
use core::traits;

/// Handles the legacy `sub_door_driver` item-use hook.
///
/// # Arguments
///
/// * `gs` - Active game state used by this legacy driver hook.
/// * `_cn` - Character index reserved for this legacy driver hook.
/// * `item_idx` - Value passed through to the legacy `sub_door_driver` hook.
///
/// # Returns
///
/// * `true` when `sub_door_driver` reports that it handled the legacy action, otherwise `false`.
///
/// # Panics
///
/// * Panics if any legacy id or index parameter used by `sub_door_driver` is outside the corresponding game-state collection.
pub fn sub_door_driver(gs: &mut GameState, _cn: usize, item_idx: usize) -> bool {
    let item = &gs.items[item_idx];

    if item.data[0] == 65500 {
        return false;
    }

    if item.data[0] == 65501 || item.data[0] == 65502 {
        let mut empty = 0;
        let mut star = 0;
        let mut circle = 0;
        let loctab: [usize; 4] = [344487, 343463, 344488, 343464];

        for (n, map_idx) in loctab.iter().copied().enumerate() {
            let in2 = gs.map[map_idx].it as usize;
            if in2 == 0 || gs.items[in2].data[1] != n as u32 {
                continue;
            }

            match gs.items[in2].temp {
                761 => star += 1,
                762 => circle += 1,
                763 => empty += 1,
                _ => {}
            }
        }

        return (item.data[0] == 65501 && empty == 3 && star == 1)
            || (item.data[0] == 65502 && empty == 3 && circle == 1);
    }

    false
}

/// Handles the legacy `use_door` item-use hook.
///
/// # Arguments
///
/// * `gs` - Active game state used by this legacy driver hook.
/// * `cn` - Character index executing this legacy driver hook.
/// * `item_idx` - Value passed through to the legacy `use_door` hook.
///
/// # Returns
///
/// * `true` when `use_door` reports that it handled the legacy action, otherwise `false`.
///
/// # Panics
///
/// * Panics if any legacy id or index parameter used by `use_door` is outside the corresponding game-state collection.
pub fn use_door(gs: &mut GameState, cn: usize, item_idx: usize) -> bool {
    let map_idx =
        gs.items[item_idx].x as usize + gs.items[item_idx].y as usize * SERVER_MAPX as usize;
    let blocked = gs.map[map_idx].ch != 0;
    if blocked {
        return false;
    }

    let mut lock = false;
    let mut key_vanishes = false;
    let mut key_slot: Option<usize> = None;

    // Check lock requirements
    let locked_without_key = {
        let lock_code = gs.items[item_idx].data[0];
        if lock_code != 0 {
            if cn == 0 {
                lock = true;
            } else if lock_code >= 65500 {
                lock = sub_door_driver(gs, cn, item_idx);
            } else {
                let character = &gs.characters[cn];
                let citem = character.citem as usize;

                if citem != 0
                    && (citem & 0x80000000) == 0
                    && gs.items[citem].temp == lock_code as u16
                {
                    lock = true;
                    if gs.items[item_idx].data[3] != 0 {
                        key_vanishes = true;
                        key_slot = None;
                    }
                } else {
                    for n in 0..40 {
                        let in2 = character.item[n] as usize;
                        if in2 != 0 && gs.items[in2].temp == lock_code as u16 {
                            lock = true;
                            if gs.items[item_idx].data[3] != 0 {
                                key_vanishes = true;
                                key_slot = Some(n);
                            }
                            break;
                        }
                    }
                }

                if !lock {
                    let citem = character.citem as usize;
                    if citem != 0 && (citem & 0x80000000) == 0 && gs.items[citem].driver == 3 {
                        let skill =
                            character.skill[skills::SK_LOCK][5] + gs.items[citem].data[0] as u16;
                        let power = gs.items[item_idx].data[2];

                        if power == 0 || skill >= (power + helpers::random_mod(20)) as u16 {
                            lock = true;
                        } else {
                            gs.do_character_log(
                                cn,
                                core::types::FontColor::Blue,
                                "You failed to pick the lock.\n",
                            );
                        }
                        item_damage_citem(gs, cn, 1);
                    }
                }
            }

            gs.items[item_idx].data[1] != 0 && !lock
        } else {
            false
        }
    };

    // If door is locked and player doesn't have key, exit early
    if locked_without_key {
        gs.do_character_log(
            cn,
            core::types::FontColor::Blue,
            "It's locked and you don't have the right key.\n",
        );
        return false;
    }

    if key_vanishes {
        if let Some(slot) = key_slot {
            let consumed_item_idx = gs.characters[cn].item[slot] as usize;
            gs.characters[cn].item[slot] = 0;
            gs.items[consumed_item_idx].used = USE_EMPTY;
        } else {
            let consumed_item_idx = gs.characters[cn].citem as usize;
            gs.characters[cn].citem = 0;
            gs.items[consumed_item_idx].used = USE_EMPTY;
        }
        gs.do_character_log(cn, core::types::FontColor::Yellow, "The key vanished.\n");
    }

    let (item_x, item_y, temp, active) = {
        let item = &gs.items[item_idx];
        (
            i32::from(item.x),
            i32::from(item.y),
            item.temp as usize,
            item.active,
        )
    };

    gs.reset_go(item_x, item_y);
    gs.remove_lights(item_x, item_y);
    gs.do_area_sound(0, 0, item_x, item_y, 10);

    if active == 0 {
        let item = &mut gs.items[item_idx];
        item.flags &= !(ItemFlags::IF_MOVEBLOCK.bits() | ItemFlags::IF_SIGHTBLOCK.bits());
        item.data[1] = 0;
    } else {
        let sight_flags = gs.item_templates[temp].flags & ItemFlags::IF_SIGHTBLOCK.bits();
        let item = &mut gs.items[item_idx];
        item.flags |= ItemFlags::IF_MOVEBLOCK.bits() | sight_flags;
        if lock {
            item.data[1] = 1;
        }
    }

    gs.reset_go(item_x, item_y);
    gs.add_lights(item_x, item_y);
    let ch = &gs.characters[cn];
    gs.do_area_notify(
        cn as i32,
        0,
        i32::from(ch.x),
        i32::from(ch.y),
        i32::from(core::constants::NT_SEE),
        cn as i32,
        0,
        0,
        0,
    );

    true
}

/// Handles the legacy `use_lever` item-use hook.
///
/// # Arguments
///
/// * `gs` - Active game state used by this legacy driver hook.
/// * `_cn` - Character index reserved for this legacy driver hook.
/// * `item_idx` - Value passed through to the legacy `use_lever` hook.
///
/// # Returns
///
/// * `true` when `use_lever` reports that it handled the legacy action, otherwise `false`.
///
/// # Panics
///
/// * Panics if any legacy id or index parameter used by `use_lever` is outside the corresponding game-state collection.
pub fn use_lever(gs: &mut GameState, _cn: usize, item_idx: usize) -> bool {
    // Get the map coordinate from item data[0]
    let m = gs.items[item_idx].data[0] as usize;

    // Get the item at that map location
    let in2 = gs.map[m].it;

    if in2 == 0 {
        return false;
    }

    // Check if the item is already active
    let is_active = gs.items[in2 as usize].active != 0;
    if is_active {
        return false;
    }

    // Activate the linked item
    use_driver(gs, 0, in2 as usize, false);

    // Set active to duration and handle lighting changes
    gs.items[in2 as usize].active = gs.items[in2 as usize].duration;

    let light0 = gs.items[in2 as usize].light[0];
    let light1 = gs.items[in2 as usize].light[1];
    if light0 != light1 {
        let x = i32::from(gs.items[in2 as usize].x);
        let y = i32::from(gs.items[in2 as usize].y);
        gs.do_add_light(x, y, i32::from(light1) - i32::from(light0));
    }

    true
}

/// Handles the legacy `use_seyan_door` item-use hook.
///
/// # Arguments
///
/// * `gs` - Active game state used by this legacy driver hook.
/// * `cn` - Character index executing this legacy driver hook.
/// * `item_idx` - Value passed through to the legacy `use_seyan_door` hook.
///
/// # Returns
///
/// * `true` when `use_seyan_door` reports that it handled the legacy action, otherwise `false`.
///
/// # Panics
///
/// * Panics if any legacy id or index parameter used by `use_seyan_door` is outside the corresponding game-state collection.
pub fn use_seyan_door(gs: &mut GameState, cn: usize, item_idx: usize) -> bool {
    if cn != 0 {
        // Check if character is Seyan'Du
        let is_seyan = { (gs.characters[cn].kindred & traits::KIN_SEYAN_DU as i32) != 0 };
        if !is_seyan {
            gs.do_character_log(
                cn,
                core::types::FontColor::Red,
                "You have the feeling this isn't meant for you.\n",
            );
            return false;
        }
    }

    use_door(gs, cn, item_idx)
}

/// Handles the legacy `use_rotate` item-use hook.
///
/// # Arguments
///
/// * `gs` - Active game state used by this legacy driver hook.
/// * `cn` - Character index executing this legacy driver hook.
/// * `item_idx` - Value passed through to the legacy `use_rotate` hook.
///
/// # Returns
///
/// * `true` when `use_rotate` reports that it handled the legacy action, otherwise `false`.
///
/// # Panics
///
/// * Panics if any legacy id or index parameter used by `use_rotate` is outside the corresponding game-state collection.
pub fn use_rotate(gs: &mut GameState, cn: usize, item_idx: usize) -> bool {
    if cn == 0 {
        return false;
    }

    // Rotate item: increment data[1] (0-3), update sprite

    gs.items[item_idx].data[1] += 1;
    if gs.items[item_idx].data[1] > 3 {
        gs.items[item_idx].data[1] = 0;
    }
    gs.items[item_idx].sprite[0] =
        gs.items[item_idx].data[0] as i16 + gs.items[item_idx].data[1] as i16;
    gs.items[item_idx].flags |= ItemFlags::IF_UPDATE.bits();

    true
}

/// Handles the legacy `use_lab8_key` item-use hook.
///
/// # Arguments
///
/// * `gs` - Active game state used by this legacy driver hook.
/// * `cn` - Character index executing this legacy driver hook.
/// * `item_idx` - Value passed through to the legacy `use_lab8_key` hook.
///
/// # Returns
///
/// * `true` when `use_lab8_key` reports that it handled the legacy action, otherwise `false`.
///
/// # Panics
///
/// * Panics if any legacy id or index parameter used by `use_lab8_key` is outside the corresponding game-state collection.
pub fn use_lab8_key(gs: &mut GameState, cn: usize, item_idx: usize) -> bool {
    // data[0] = matching key part
    // data[1] = resulting key part
    // data[2] = (optional) other matching key part
    // data[3] = (optional) other resulting key part

    if cn == 0 {
        return false;
    }

    let citem = gs.characters[cn].citem as usize;
    if citem == 0 || (citem & 0x80000000) != 0 {
        gs.do_character_log(cn, core::types::FontColor::Yellow, "Nothing happens.\n");
        return false;
    }

    let carried = gs.items[item_idx].carried;
    if carried == 0 {
        gs.do_character_log(
            cn,
            core::types::FontColor::Red,
            "Too difficult to do on the ground.\n",
        );
        return false;
    }

    // Check for matching parts
    let (data0, data1, data2, data3, citem_temp) = {
        (
            gs.items[item_idx].data[0],
            gs.items[item_idx].data[1],
            gs.items[item_idx].data[2],
            gs.items[item_idx].data[3],
            gs.items[citem].temp,
        )
    };

    let result_template = if data0 as u16 == citem_temp {
        data1
    } else if data2 as u16 == citem_temp {
        data3
    } else {
        0
    };

    if result_template == 0 {
        gs.do_character_log(
            cn,
            core::types::FontColor::Red,
            "Those don't fit together.\n",
        );
        return false;
    }

    // Log the assembly
    let (item_name, citem_name) = {
        (
            gs.items[item_idx].get_name().to_owned(),
            gs.items[citem].get_name().to_owned(),
        )
    };
    log::info!("Added {} to {}", citem_name, item_name);

    // Remove both old parts
    God::take_from_char(gs, item_idx, cn);
    {
        gs.items[item_idx].used = USE_EMPTY;
    };

    {
        gs.characters[cn].citem = 0;
    };
    {
        gs.items[citem].used = USE_EMPTY;
    };

    // Create and give new key
    let new_key = God::create_item(gs, result_template as usize);
    {
        gs.items[new_key.unwrap()].flags |= core::constants::ItemFlags::IF_UPDATE.bits();
    };
    God::give_character_item(gs, cn, new_key.unwrap());

    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::{add_test_player, with_test_gs};
    use core::constants::{ItemFlags, SERVER_MAPX, USE_ACTIVE, USE_EMPTY};

    /// Returns the flat map index for test coordinates.
    fn map_idx(x: u16, y: u16) -> usize {
        x as usize + y as usize * SERVER_MAPX as usize
    }

    /// Creates a used door item at a safe map coordinate.
    fn add_door(gs: &mut GameState, item_idx: usize, active: u32) {
        gs.items[item_idx].used = USE_ACTIVE;
        gs.items[item_idx].x = 20;
        gs.items[item_idx].y = 20;
        gs.items[item_idx].temp = 9;
        gs.items[item_idx].active = active;
        gs.items[item_idx].flags = ItemFlags::IF_MOVEBLOCK.bits() | ItemFlags::IF_SIGHTBLOCK.bits();
        gs.item_templates[9].flags = ItemFlags::IF_SIGHTBLOCK.bits();
    }

    #[test]
    fn use_door_refuses_when_tile_is_occupied() {
        with_test_gs(|gs| {
            let (cn, _nr) = add_test_player(gs);
            let item_idx = 1;
            add_door(gs, item_idx, 0);
            gs.map[map_idx(20, 20)].ch = cn as u32;

            assert!(!use_door(gs, cn, item_idx));
            assert_ne!(gs.items[item_idx].flags & ItemFlags::IF_MOVEBLOCK.bits(), 0);
        });
    }

    #[test]
    fn use_door_opens_inactive_door_by_clearing_block_flags() {
        with_test_gs(|gs| {
            let (cn, _nr) = add_test_player(gs);
            let item_idx = 1;
            add_door(gs, item_idx, 0);
            gs.items[item_idx].data[1] = 1;

            assert!(use_door(gs, cn, item_idx));
            assert_eq!(gs.items[item_idx].flags & ItemFlags::IF_MOVEBLOCK.bits(), 0);
            assert_eq!(
                gs.items[item_idx].flags & ItemFlags::IF_SIGHTBLOCK.bits(),
                0
            );
            assert_eq!(gs.items[item_idx].data[1], 0);
        });
    }

    #[test]
    fn use_door_closes_active_door_and_restores_template_sight_block() {
        with_test_gs(|gs| {
            let (cn, _nr) = add_test_player(gs);
            let item_idx = 1;
            add_door(gs, item_idx, 1);
            gs.items[item_idx].flags = 0;

            assert!(use_door(gs, cn, item_idx));
            assert_ne!(gs.items[item_idx].flags & ItemFlags::IF_MOVEBLOCK.bits(), 0);
            assert_ne!(
                gs.items[item_idx].flags & ItemFlags::IF_SIGHTBLOCK.bits(),
                0
            );
        });
    }

    #[test]
    fn locked_door_without_key_stays_closed() {
        with_test_gs(|gs| {
            let (cn, _nr) = add_test_player(gs);
            let item_idx = 1;
            add_door(gs, item_idx, 1);
            gs.items[item_idx].data[0] = 77;
            gs.items[item_idx].data[1] = 1;
            gs.items[item_idx].flags = ItemFlags::IF_MOVEBLOCK.bits();

            assert!(!use_door(gs, cn, item_idx));
            assert_eq!(gs.items[item_idx].data[1], 1);
            assert_ne!(gs.items[item_idx].flags & ItemFlags::IF_MOVEBLOCK.bits(), 0);
        });
    }

    #[test]
    fn key_that_vanishes_unlocks_door_and_is_consumed_from_inventory() {
        with_test_gs(|gs| {
            let (cn, _nr) = add_test_player(gs);
            let door_idx = 1;
            let key_idx = 2;
            add_door(gs, door_idx, 0);
            gs.items[door_idx].data[0] = 77;
            gs.items[door_idx].data[1] = 1;
            gs.items[door_idx].data[3] = 1;
            gs.items[key_idx].used = USE_ACTIVE;
            gs.items[key_idx].temp = 77;
            gs.items[key_idx].carried = cn as u16;
            gs.characters[cn].item[0] = key_idx as u32;

            assert!(use_door(gs, cn, door_idx));
            assert_eq!(gs.characters[cn].item[0], 0);
            assert_eq!(gs.items[key_idx].used, USE_EMPTY);
            assert_eq!(gs.items[door_idx].data[1], 0);
        });
    }

    #[test]
    fn use_lever_activates_linked_inactive_item() {
        with_test_gs(|gs| {
            let lever_idx = 1;
            let linked_idx = 2;
            let target_map_idx = map_idx(30, 30);
            gs.items[lever_idx].used = USE_ACTIVE;
            gs.items[lever_idx].data[0] = target_map_idx as u32;
            gs.map[target_map_idx].it = linked_idx as u32;
            gs.items[linked_idx].used = USE_ACTIVE;
            gs.items[linked_idx].x = 30;
            gs.items[linked_idx].y = 30;
            gs.items[linked_idx].duration = 15;

            assert!(use_lever(gs, 0, lever_idx));
            assert_eq!(gs.items[linked_idx].active, 15);
        });
    }

    #[test]
    fn use_rotate_wraps_state_and_updates_sprite() {
        with_test_gs(|gs| {
            let (cn, _nr) = add_test_player(gs);
            let item_idx = 1;
            gs.items[item_idx].used = USE_ACTIVE;
            gs.items[item_idx].data[0] = 100;
            gs.items[item_idx].data[1] = 3;

            assert!(use_rotate(gs, cn, item_idx));
            assert_eq!(gs.items[item_idx].data[1], 0);
            assert_eq!(gs.items[item_idx].sprite[0], 100);
            assert_ne!(gs.items[item_idx].flags & ItemFlags::IF_UPDATE.bits(), 0);
        });
    }

    #[test]
    fn use_lab8_key_combines_matching_parts_into_result_key() {
        with_test_gs(|gs| {
            let (cn, _nr) = add_test_player(gs);
            let base_idx = 1;
            let part_idx = 2;
            let result_template = 77;
            gs.item_templates[result_template].used = USE_ACTIVE;
            gs.items[base_idx].used = USE_ACTIVE;
            gs.items[base_idx].carried = cn as u16;
            gs.items[base_idx].data[0] = 44;
            gs.items[base_idx].data[1] = result_template as u32;
            gs.characters[cn].item[0] = base_idx as u32;
            gs.items[part_idx].used = USE_ACTIVE;
            gs.items[part_idx].temp = 44;
            gs.items[part_idx].carried = cn as u16;
            gs.characters[cn].citem = part_idx as u32;

            assert!(use_lab8_key(gs, cn, base_idx));
            let new_key = gs.characters[cn].item[0] as usize;
            assert_ne!(new_key, 0);
            assert_eq!(gs.characters[cn].citem, 0);
            assert_eq!(gs.items[part_idx].used, USE_EMPTY);
            assert_eq!(gs.items[new_key].temp as usize, result_template);
        });
    }
}
