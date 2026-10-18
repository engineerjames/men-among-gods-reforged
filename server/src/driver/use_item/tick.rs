use super::{expire_blood_penta, item_age, lightage, pentagram, use_driver};
use crate::effect::EffectManager;
use crate::game_state::GameState;
use crate::god::God;
use crate::helpers::{self};
use crate::populate;
use core::constants::{
    CharacterFlags, DX_RIGHT, ItemFlags, MAXITEM, MF_NOEXPIRE, SERVER_MAPX, SERVER_MAPY, TICKS,
    USE_ACTIVE, USE_EMPTY,
};

/// Handles the legacy `may_deactivate` item-use hook.
///
/// # Arguments
///
/// * `gs` - Active game state used by this legacy driver hook.
/// * `item_idx` - Value passed through to the legacy `may_deactivate` hook.
///
/// # Returns
///
/// * `true` when `may_deactivate` reports that it handled the legacy action, otherwise `false`.
///
/// # Panics
///
/// * Panics if any legacy id or index parameter used by `may_deactivate` is outside the corresponding game-state collection.
pub fn may_deactivate(gs: &mut GameState, item_idx: usize) -> bool {
    // Special check for driver 1 (create_item with mines)
    let driver = gs.items[item_idx].driver;
    if driver != 1 {
        return true;
    }

    // Check data[1..9] for mine states; each stores a map tile index.
    let max_tiles = (SERVER_MAPX * SERVER_MAPY) as usize;
    for n in 1..10 {
        let m = gs.items[item_idx].data[n] as usize;
        if m == 0 {
            // empty slot => can deactivate
            return true;
        }

        // Validate tile index bounds
        if m >= max_tiles {
            return false;
        }

        // Check if there is an item on the stored map tile and that
        // the item has driver 26 (the mine driver). If not, cannot deactivate.
        let it_idx = gs.map[m].it;
        if it_idx == 0 {
            return false;
        }

        let it_driver = gs.items[it_idx as usize].driver;
        if it_driver != 26 {
            return false;
        }
    }

    true
}

/// Handles the legacy `spiderweb` item-use hook.
///
/// # Arguments
///
/// * `gs` - Active game state used by this legacy driver hook.
/// * `item_idx` - Value passed through to the legacy `spiderweb` hook.
///
/// # Panics
///
/// * Panics if any legacy id or index parameter used by `spiderweb` is outside the corresponding game-state collection.
pub fn spiderweb(gs: &mut GameState, item_idx: usize) {
    let active = gs.items[item_idx].active;
    if active != 0 {
        return;
    }

    if helpers::random_mod_i32(60) != 0 {
        return;
    }

    // Check data[1-3] for spawned spiders
    for n in 1..4 {
        let stored_cn = gs.items[item_idx].data[n];

        // Check if slot is empty or spider is dead
        let should_spawn = if stored_cn == 0 {
            true
        } else {
            let cn = stored_cn as usize;

            gs.characters[cn].data[0] != item_idx as i32
                || gs.characters[cn].used == USE_EMPTY
                || (gs.characters[cn].flags & CharacterFlags::Body.bits()) != 0
        };

        if should_spawn {
            // Create spider (template 390-392)
            let spider_template = 390usize + helpers::random_mod_usize(3);
            let cn = match populate::pop_create_char(gs, spider_template, false) {
                Some(cn) => cn,
                None => continue,
            };

            let (x, y) = { (gs.items[item_idx].x as usize, gs.items[item_idx].y as usize) };

            // Ensure respawn flag is cleared for this spawned instance
            gs.characters[cn].flags &= !CharacterFlags::Respawn.bits();
            gs.characters[cn].data[0] = item_idx as i32;
            gs.characters[cn].data[29] = (x + y * core::constants::SERVER_MAPX as usize) as i32;
            gs.characters[cn].data[60] = TICKS * 60 * 2;
            gs.characters[cn].data[73] = 8;
            gs.characters[cn].dir = DX_RIGHT;

            if !God::drop_char_fuzzy(gs, cn, x, y) {
                God::destroy_items(gs, cn);
                gs.characters[cn].used = USE_EMPTY;
            } else {
                gs.items[item_idx].data[n] = cn as u32;
            }
            break;
        }
    }
}

/// Handles the legacy `greenlingball` item-use hook.
///
/// # Arguments
///
/// * `gs` - Active game state used by this legacy driver hook.
/// * `item_idx` - Value passed through to the legacy `greenlingball` hook.
///
/// # Panics
///
/// * Panics if any legacy id or index parameter used by `greenlingball` is outside the corresponding game-state collection.
pub fn greenlingball(gs: &mut GameState, item_idx: usize) {
    let active = gs.items[item_idx].active;
    if active != 0 {
        return;
    }

    if helpers::random_mod_i32(20) != 0 {
        return;
    }

    // Check data[1-3] for spawned greenlings
    for n in 1..4 {
        let stored_cn = gs.items[item_idx].data[n];

        // Check if slot is empty or greenling is dead
        let should_spawn = if stored_cn == 0 {
            true
        } else {
            let cn = stored_cn as usize;

            gs.characters[cn].data[0] != item_idx as i32
                || gs.characters[cn].used == USE_EMPTY
                || (gs.characters[cn].flags & CharacterFlags::Body.bits()) != 0
        };

        if should_spawn {
            // Create greenling (template 553 + data[0])
            let greenling_type = gs.items[item_idx].data[0];
            let cn = match populate::pop_create_char(gs, 553 + greenling_type as usize, false) {
                Some(cn) => cn,
                None => continue,
            };

            let (x, y) = { (gs.items[item_idx].x as usize, gs.items[item_idx].y as usize) };

            // Ensure respawn flag is cleared for this spawned instance
            gs.characters[cn].flags &= !CharacterFlags::Respawn.bits();
            gs.characters[cn].data[0] = item_idx as i32;
            gs.characters[cn].data[29] = (x + y * core::constants::SERVER_MAPX as usize) as i32;
            gs.characters[cn].data[60] = TICKS * 60 * 2;
            gs.characters[cn].data[73] = 8;
            gs.characters[cn].dir = DX_RIGHT;

            if !God::drop_char_fuzzy(gs, cn, x, y) {
                God::destroy_items(gs, cn);
                gs.characters[cn].used = USE_EMPTY;
            } else {
                gs.items[item_idx].data[n] = cn as u32;
            }
            break;
        }
    }
}

/// Handles the legacy `expire_driver` item-use hook.
///
/// # Arguments
///
/// * `gs` - Active game state used by this legacy driver hook.
/// * `item_idx` - Value passed through to the legacy `expire_driver` hook.
///
/// # Panics
///
/// * Panics if any legacy id or index parameter used by `expire_driver` is outside the corresponding game-state collection.
pub fn expire_driver(gs: &mut GameState, item_idx: usize) {
    let driver = gs.items[item_idx].driver;

    match driver {
        49 => expire_blood_penta(gs, item_idx),
        _ => {
            log::error!(
                "unknown expire driver {} for item {} ({})",
                gs.items[item_idx].driver,
                gs.items[item_idx].get_name(),
                item_idx
            );
        }
    }
}

/// Handles the legacy `item_tick_expire` item-use hook.
///
/// # Arguments
///
/// * `gs` - Active game state used by this legacy driver hook.
///
/// # Panics
///
/// * Panics if any legacy id or index parameter used by `item_tick_expire` is outside the corresponding game-state collection.
pub fn item_tick_expire(gs: &mut GameState) {
    const EXP_TIME: i32 = SERVER_MAPY / 4;

    // Conform to the original C++ semantics:
    // - process the current row `y`
    // - then increment and wrap `y` afterwards
    let mut y = gs.item_tick_expire_counter;
    if y >= SERVER_MAPY as u32 {
        y = 0;
    }

    let y_usize = y as usize;
    for x in 0..SERVER_MAPX as usize {
        let m = x + y_usize * SERVER_MAPX as usize;

        // Process items on this tile
        let in_idx = gs.map[m].it;
        if in_idx != 0 {
            let in_idx = in_idx as usize;

            // Snapshot core fields, but keep `active` as a local variable we update
            // to mirror the C++ behavior (reactivation affects same-tick expiration).
            let (mut flags, driver, mut active, duration, light_diff) = {
                let item = &gs.items[in_idx];
                (
                    item.flags,
                    item.driver,
                    item.active,
                    item.duration,
                    i32::from(item.light[1]) - i32::from(item.light[0]),
                )
            };

            let (ch_present, to_ch_present) = (gs.map[m].ch != 0, gs.map[m].to_ch != 0);

            if (flags & ItemFlags::IF_REACTIVATE.bits()) != 0
                && active == 0
                && !ch_present
                && !to_ch_present
            {
                gs.items[in_idx].active = duration;

                active = duration;
                if light_diff != 0 {
                    gs.do_add_light(x as i32, y as i32, light_diff);
                }
            }

            // Handle active expiration
            if active != 0 && active != 0xffffffff {
                if active <= EXP_TIME as u32 {
                    if may_deactivate(gs, in_idx) && !ch_present && !to_ch_present {
                        use_driver(gs, 0, in_idx, false);
                        gs.items[in_idx].active = 0;
                        active = 0;
                        if light_diff != 0 {
                            gs.do_add_light(x as i32, y as i32, -light_diff);
                        }
                    }
                } else {
                    gs.items[in_idx].active -= EXP_TIME as u32;
                    active -= EXP_TIME as u32;
                }
            }

            // Legacy drivers
            if driver == 33 {
                pentagram(gs, in_idx);
            }
            if driver == 43 {
                spiderweb(gs, in_idx);
            }
            if driver == 56 {
                greenlingball(gs, in_idx);
            }

            // IF_EXPIREPROC
            if (flags & ItemFlags::IF_EXPIREPROC.bits()) != 0 {
                expire_driver(gs, in_idx);
            }

            // Refresh flags in case any of the above mutated them.
            flags = gs.items[in_idx].flags;

            // Check if item should expire
            let map_flags = gs.map[m].flags;
            if (((flags & ItemFlags::IF_TAKE.bits()) == 0
                || (map_flags & u64::from(MF_NOEXPIRE)) != 0)
                && driver != 7)
                || driver == 37
                || (flags & ItemFlags::IF_NOEXPIRE.bits()) != 0
            {
                // Skip expiration
            } else {
                let act = if active != 0 { 1 } else { 0 };
                gs.items[in_idx].current_age[act] += EXP_TIME as u32;

                if (flags & ItemFlags::IF_LIGHTAGE.bits()) != 0 {
                    lightage(gs, in_idx, EXP_TIME);
                }

                if item_age(gs, in_idx) {
                    let damage_state = gs.items[in_idx].damage_state;
                    if damage_state == 5 {
                        let light = gs.items[in_idx].light[act];
                        if light != 0 {
                            gs.do_add_light(x as i32, y as i32, -i32::from(light));
                        }

                        gs.map[m].it = 0;
                        gs.items[in_idx].used = USE_EMPTY;
                        gs.globals.expire_cnt += 1;

                        // Handle tomb (driver == 7)
                        if driver == 7 {
                            let co = gs.items[in_idx].data[0] as usize;
                            // Validate character index
                            if co != 0 && co < core::constants::MAXCHARS {
                                // Remember template and name for logging
                                let (temp, name, respawn_flag) = {
                                    (
                                        gs.characters[co].temp as usize,
                                        gs.characters[co].get_name().to_owned(),
                                        (gs.characters[co].flags & CharacterFlags::Respawn.bits())
                                            != 0,
                                    )
                                };

                                // Remove the character and its items
                                God::destroy_items(gs, co);
                                gs.characters[co].used = core::constants::USE_EMPTY;

                                if temp != 0 && respawn_flag {
                                    // Schedule a respawn effect (type 2 = RSPAWN)
                                    let dur = if temp == 189 || temp == 561 {
                                        TICKS * 60 * 20 + helpers::random_mod_i32(TICKS * 60 * 5)
                                    } else {
                                        (TICKS * 60) + helpers::random_mod_i32(TICKS * 60)
                                    };

                                    // Use the template's coordinates for the respawn location
                                    let (tx, ty) = {
                                        (
                                            i32::from(gs.character_templates[temp].x),
                                            i32::from(gs.character_templates[temp].y),
                                        )
                                    };

                                    EffectManager::fx_add_effect(gs, 2, dur, tx, ty, temp as i32);
                                    log::info!("respawn {} ({}): YES", co, name);
                                } else {
                                    log::info!("respawn {} ({}): NO", co, name);
                                }
                            }
                        }
                    }
                }
            }
        }

        // Checker: validate map references
        let cn = gs.map[m].ch;
        if cn != 0 {
            let cn = cn as usize;
            let (ch_x, ch_y, ch_used) = {
                (
                    gs.characters[cn].x,
                    gs.characters[cn].y,
                    gs.characters[cn].used,
                )
            };
            if ch_x != x as i16 || ch_y != y as i16 || ch_used != USE_ACTIVE {
                log::error!("map[{},{}].ch reset from {} to 0", x, y, cn);
                gs.map[m].ch = 0;
                gs.globals.lost_cnt += 1;
            }
        }

        let cn = gs.map[m].to_ch;
        if cn != 0 {
            let cn = cn as usize;
            let (tox, toy, ch_used) = {
                (
                    gs.characters[cn].tox,
                    gs.characters[cn].toy,
                    gs.characters[cn].used,
                )
            };
            if tox != x as i16 || toy != y as i16 || ch_used != USE_ACTIVE {
                log::error!("map[{},{}].to_ch reset from {} to 0", x, y, cn);
                gs.map[m].to_ch = 0;
                gs.globals.lost_cnt += 1;
            }
        }

        let in_idx = gs.map[m].it;
        if in_idx != 0 {
            let in_idx = in_idx as usize;
            let (it_x, it_y, it_used) = {
                (
                    gs.items[in_idx].x,
                    gs.items[in_idx].y,
                    gs.items[in_idx].used,
                )
            };
            if it_x != x as u16 || it_y != y as u16 || it_used != USE_ACTIVE {
                if in_idx < gs.items.len() {
                    let item = &gs.items[in_idx];
                    let temp = item.temp;
                    let carried = item.carried;
                    let used = item.used;
                    let item_x = item.x;
                    let item_y = item.y;
                    log::error!(
                        "map[{},{}].it invalid -> item {} (temp={}, name='{}', carried={}, used={}, pos=({},{})); clearing map reference",
                        x,
                        y,
                        in_idx,
                        temp,
                        item.get_name(),
                        carried,
                        used,
                        item_x,
                        item_y,
                    );
                } else {
                    log::error!(
                        "map[{},{}].it invalid -> item index {} out of bounds; clearing map reference",
                        x,
                        y,
                        in_idx
                    );
                }

                log::error!("map[{},{}].it reset from {} to 0", x, y, in_idx);
                gs.map[m].it = 0;
                gs.globals.lost_cnt += 1;
            }
        }
    }

    // Advance to next row after processing the current row.
    y += 1;
    if y >= SERVER_MAPY as u32 {
        gs.globals.expire_run += 1;
        gs.globals.lost_run += 1;

        y = 0;
    }
    gs.item_tick_expire_counter = y;
}

/// Handles the legacy `item_tick_gc` item-use hook.
///
/// # Arguments
///
/// * `gs` - Active game state used by this legacy driver hook.
///
/// # Panics
///
/// * Panics if any legacy id or index parameter used by `item_tick_gc` is outside the corresponding game-state collection.
pub fn item_tick_gc(gs: &mut GameState) {
    let (off, m) = {
        let current_off = gs.item_tick_gc_off as usize;
        let current_m = std::cmp::min(current_off + 256, MAXITEM);
        (current_off, current_m)
    };

    for n in off..m {
        let used = gs.items[n].used;
        if used == USE_EMPTY {
            continue;
        }

        let current_count = gs.item_tick_gc_count;
        gs.item_tick_gc_count = current_count + 1;

        // Hack: make reset seyan swords unusable
        let (driver, data0) = (gs.items[n].driver, gs.items[n].data[0]);
        if driver == 40 && data0 == 0 {
            // Reset to template 683

            let (x, y, carried) = (gs.items[n].x, gs.items[n].y, gs.items[n].carried);
            gs.items[n] = gs.item_templates[683];
            gs.items[n].x = x;
            gs.items[n].y = y;
            gs.items[n].carried = carried;
            gs.items[n].temp = 683;
            gs.items[n].flags |= ItemFlags::IF_UPDATE.bits();

            let cn = gs.items[n].carried;
            if cn != 0 {
                gs.do_update_char(cn as usize);
                log::info!("reset sword from character {}", cn);
            }
        }

        let cn = gs.items[n].carried as usize;
        if cn != 0 {
            let is_sane = cn < core::constants::MAXCHARS;
            if is_sane {
                let ch_used = gs.characters[cn].used;
                if ch_used != 0 {
                    // Check if item is in character's inventory
                    let mut found = false;

                    // Check item slots
                    for z in 0..40 {
                        let item_id = gs.characters[cn].item[z];
                        if item_id == n as u32 {
                            found = true;
                            break;
                        }
                    }

                    // Check worn/spell slots
                    if !found {
                        for z in 0..20 {
                            let worn_id = gs.characters[cn].worn[z];
                            let spell_id = gs.characters[cn].spell[z];
                            if worn_id == n as u32 || spell_id == n as u32 {
                                found = true;
                                break;
                            }
                        }
                    }

                    // Check citem
                    if !found {
                        let citem = gs.characters[cn].citem;
                        if citem == n as u32 {
                            found = true;
                        }
                    }

                    // Check depot for players
                    if !found {
                        let is_player =
                            (gs.characters[cn].flags & CharacterFlags::Player.bits()) != 0;
                        if is_player {
                            for z in 0..62 {
                                let depot_id = { gs.characters[cn].depot[z] };
                                if depot_id == n as u32 {
                                    found = true;
                                    break;
                                }
                            }
                        }
                    }

                    if found {
                        continue;
                    }
                }
            }
        } else {
            // Check if item is on the map
            let (x, y) = (gs.items[n].x, gs.items[n].y);
            let in2 = {
                let idx = x as usize + y as usize * SERVER_MAPX as usize;
                gs.map[idx].it
            };
            if in2 == n as u32 {
                continue;
            }
        }

        // Item is garbage - remove it
        gs.items[n].used = USE_EMPTY;
        gs.globals.gc_cnt += 1;
    }

    // Update OFF and possibly reset
    let mut current_off = gs.item_tick_gc_off;
    current_off += 256;
    gs.item_tick_gc_off = current_off;

    if current_off >= MAXITEM as u32 {
        gs.item_tick_gc_off = 0;
        let count = gs.item_tick_gc_count;
        gs.globals.item_cnt = count as i32;
        gs.globals.gc_run += 1;
        gs.item_tick_gc_count = 0;
    }
}

/// Handles the legacy `item_tick` item-use hook.
///
/// # Arguments
///
/// * `gs` - Active game state used by this legacy driver hook.
///
/// # Panics
///
/// * Panics if any legacy id or index parameter used by `item_tick` is outside the corresponding game-state collection.
pub fn item_tick(gs: &mut GameState) {
    // Preserve legacy wall-clock expire pacing (4 passes at 18 TPS) while
    // allowing arbitrary server tick rates.
    const LEGACY_EXPIRE_PASSES_PER_TICK: i32 = 4;
    gs.item_tick_expire_budget += LEGACY_EXPIRE_PASSES_PER_TICK * core::constants::LEGACY_TICKS;

    while gs.item_tick_expire_budget >= core::constants::TICKS {
        item_tick_expire(gs);
        gs.item_tick_expire_budget -= core::constants::TICKS;
    }

    item_tick_gc(gs);
}
