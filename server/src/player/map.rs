use core::{
    constants::{
        CharacterFlags, INFRARED, INJURED, INJURED1, INJURED2, INVIS, IS_GRAVE, ISCHAR, ISITEM,
        ISUSABLE, ItemFlags, MF_GFX_CMAGIC, MF_GFX_DEATH, MF_GFX_EMAGIC, MF_GFX_GMAGIC,
        MF_GFX_INJURED, MF_GFX_INJURED1, MF_GFX_INJURED2, MF_GFX_TOMB, MF_UWATER, STONED, STUNNED,
        UWATER,
    },
    logout_reasons::LogoutReason,
    server_commands::ServerCommandType,
};

use crate::{
    driver, game_state::GameState, god::God, helpers, network_manager,
    player::connection::plr_logout, types::cmap::CMap,
};

/// Port of `plr_map_remove` from `svr_act.cpp`
///
/// Removes a character from the world map tile and clears any transient
/// references associated with that tile (to_ch, step-action items, lights).
/// It also undoes light contributions for the character and clears step
/// drivers for stepped-on items when appropriate.
///
/// # Arguments
/// * `gs` - Active game state used to update map occupancy and lighting.
/// * `cn` - Character index to remove from the map
pub fn plr_map_remove(gs: &mut GameState, cn: usize) {
    let ch = gs.characters[cn];
    let m = (ch.x as usize) + (ch.y as usize) * core::constants::SERVER_MAPX as usize;
    let to_m = (ch.tox as usize) + (ch.toy as usize) * core::constants::SERVER_MAPX as usize;
    let light = ch.light;
    let (x, y) = (ch.x, ch.y);
    let is_body = (ch.flags & CharacterFlags::Body.bits()) != 0;

    gs.map[m].ch = 0;
    gs.map[to_m].to_ch = 0;
    if light != 0 {
        gs.do_add_light(x as i32, y as i32, -(light as i32));
    }
    if !is_body {
        let in_id = gs.map[m].it;
        if in_id != 0 {
            let has_step_action = (gs.items[in_id as usize].flags
                & core::constants::ItemFlags::IF_STEPACTION.bits())
                != 0;
            if has_step_action {
                driver::step_driver_remove(gs, cn, in_id as usize);
            }
        }
    }
}

/// Port of `plr_map_set` from `svr_act.cpp`
///
/// Places a character on the map and handles tile interactions that occur
/// on arrival. This checks for step-action items (calling the step driver),
/// taverns (triggering logout/tavern logic), "no magic" zones (removing
/// spells and flagging the character), death traps (killing the character),
/// and finally notifies nearby clients of the character's presence.
///
/// The function will also restore the character to a previous tile when
/// teleport/step-driver returns special values, and updates lighting.
///
/// # Arguments
/// * `gs` - Active game state used for map transitions and tile effects.
/// * `cn` - Character index to place on the map
pub fn plr_map_set(gs: &mut GameState, cn: usize) {
    let (x, y, flags, light) = (
        gs.characters[cn].x,
        gs.characters[cn].y,
        gs.characters[cn].flags,
        gs.characters[cn].light,
    );

    let m = (x as usize) + (y as usize) * core::constants::SERVER_MAPX as usize;
    let is_body = (flags & CharacterFlags::Body.bits()) != 0;
    let is_player = (flags & CharacterFlags::Player.bits()) != 0;

    if !is_body {
        // Check for step action
        let in_id = gs.map[m].it;
        if in_id != 0 {
            let has_step_action = (gs.items[in_id as usize].flags
                & core::constants::ItemFlags::IF_STEPACTION.bits())
                != 0;

            if has_step_action {
                // Call step_driver and handle return values per original C++ logic
                let ret = driver::step_driver(gs, cn, in_id as usize);

                if ret == 1 {
                    gs.map[m].to_ch = 0;

                    // compute destination: x + (x - frx), y + (y - fry)
                    let (cx, cy, frx, fry, light) = (
                        gs.characters[cn].x as i32,
                        gs.characters[cn].y as i32,
                        gs.characters[cn].frx as i32,
                        gs.characters[cn].fry as i32,
                        gs.characters[cn].light,
                    );

                    let nx = cx + (cx - frx);
                    let ny = cy + (cy - fry);

                    let idx = (nx as usize) + (ny as usize) * core::constants::SERVER_MAPX as usize;
                    let target_empty = gs.map[idx].ch == 0;

                    if target_empty {
                        gs.characters[cn].x = nx as i16;
                        gs.characters[cn].y = ny as i16;
                        gs.characters[cn].use_nr = 0;
                        gs.characters[cn].skill_nr = 0;
                        gs.characters[cn].attack_cn = 0;
                        gs.characters[cn].goto_x = 0;
                        gs.characters[cn].goto_y = 0;
                        gs.characters[cn].misc_action = 0;

                        let idx =
                            (nx as usize) + (ny as usize) * core::constants::SERVER_MAPX as usize;
                        gs.map[idx].ch = cn as u32;

                        if light != 0 {
                            gs.do_add_light(nx, ny, light as i32);
                        }

                        return;
                    }
                }

                if ret == -1 {
                    gs.map[m].to_ch = 0;

                    let (frx, fry, light) = (
                        gs.characters[cn].frx as i32,
                        gs.characters[cn].fry as i32,
                        gs.characters[cn].light,
                    );

                    gs.characters[cn].x = frx as i16;
                    gs.characters[cn].y = fry as i16;
                    gs.characters[cn].use_nr = 0;
                    gs.characters[cn].skill_nr = 0;
                    gs.characters[cn].attack_cn = 0;
                    gs.characters[cn].goto_x = 0;
                    gs.characters[cn].goto_y = 0;
                    gs.characters[cn].misc_action = 0;

                    let idx =
                        (frx as usize) + (fry as usize) * core::constants::SERVER_MAPX as usize;
                    gs.map[idx].ch = cn as u32;

                    if light != 0 {
                        gs.do_add_light(frx, fry, light as i32);
                    }

                    return;
                }

                if ret == 2 {
                    // TELEPORT_SUCCESS: just add light and return
                    let (tx, ty, current_light) = (
                        gs.characters[cn].x,
                        gs.characters[cn].y,
                        gs.characters[cn].light,
                    );

                    if current_light != 0 {
                        gs.do_add_light(tx as i32, ty as i32, current_light as i32);
                    }
                    return;
                }
            }
        }

        // Check for tavern
        let is_tavern = (gs.map[m].flags & core::constants::MF_TAVERN as u64) != 0;

        if is_tavern && is_player {
            if gs.characters[cn].is_building() {
                God::build(gs, cn, 0);
            }
            gs.characters[cn].tavern_x = gs.characters[cn].x as u16;
            gs.characters[cn].tavern_y = gs.characters[cn].y as u16;

            log::info!("Character {} entered tavern", cn);

            let player_id = gs.characters[cn].player;
            plr_logout(gs, cn, player_id as usize, LogoutReason::Tavern);
            return;
        }

        // Check for no magic zone, respect items that exempt char from nomagic
        let is_nomagic = (gs.map[m].flags & core::constants::MF_NOMAGIC as u64) != 0;

        let wears_466 = gs.char_wears_item(cn, 466);
        let wears_481 = gs.char_wears_item(cn, 481);

        if is_nomagic && !wears_466 && !wears_481 {
            // Match original behavior: only apply/remove spells and log once
            // when entering a no-magic tile (i.e., on flag transition).
            let mut became_nomagic = false;
            if (gs.characters[cn].flags & CharacterFlags::NoMagic.bits()) == 0 {
                gs.characters[cn].flags |= CharacterFlags::NoMagic.bits();
                became_nomagic = true;
            }

            if became_nomagic {
                driver::remove_spells(gs, cn);
                gs.do_character_log(
                    cn,
                    core::types::FontColor::Red,
                    "You feel your magic fail.\n",
                );
            }
        } else {
            let mut was_nomagic = false;
            if (gs.characters[cn].flags & CharacterFlags::NoMagic.bits()) != 0 {
                gs.characters[cn].flags &= !CharacterFlags::NoMagic.bits();
                gs.characters[cn].set_do_update_flags();
                was_nomagic = true;
            }

            if was_nomagic {
                gs.do_character_log(
                    cn,
                    core::types::FontColor::Red,
                    "You feel your magic return.\n",
                );
            }
        }
    }

    // Set character on map
    gs.map[m].ch = cn as u32;
    gs.map[m].to_ch = 0;

    if !is_body {
        if light != 0 {
            gs.do_add_light(x as i32, y as i32, light as i32);
        }

        // Check for death trap
        let is_deathtrap = (gs.map[m].flags & core::constants::MF_DEATHTRAP as u64) != 0;

        if is_deathtrap {
            gs.do_character_log(
                cn,
                core::types::FontColor::Red,
                "You entered a Deathtrap!\n",
            );
            log::info!("Character {} entered a Deathtrap", cn);
            gs.do_character_killed(cn, 0, true);
            return;
        }
    }

    gs.do_area_notify(
        cn as i32,
        0,
        x as i32,
        y as i32,
        core::constants::NT_SEE as i32,
        cn as i32,
        0,
        0,
        0,
    );
}

/// Clear the saved small map for all players to force a full resend
/// TODO: Do we need this for any reason?
#[allow(dead_code)]
pub fn plr_clear_map(gs: &mut GameState) {
    for n in 1..gs.players.len() {
        gs.players[n].smap = std::array::from_fn(|_| CMap::default());
        gs.players[n].vx = 0; // force do_all in map generation
    }
}

/// Choose and dispatch the appropriate map update implementation.
///
/// Decides between the full (`plr_getmap_complete`) or fast (`plr_getmap_fast`)
/// small-map generation based on server load and global flags. When entering
/// or leaving "speed savings" mode the function clears map caches and
/// announces the mode change.
///
/// # Arguments
/// * `nr` - Player slot index requesting the map update
pub fn plr_getmap(gs: &mut GameState, nr: usize) {
    plr_getmap_complete(gs, nr);
}

pub fn plr_getmap_complete(gs: &mut GameState, nr: usize) {
    let cn = gs.players[nr].usnr;

    // We copy it out here so we HAVE to write it back.
    let mut smap = gs.players[nr].smap;

    const YSCUT: i32 = 3;
    const YECUT: i32 = 1;
    const XSCUT: i32 = 2;
    const XECUT: i32 = 2;

    let ys = gs.characters[cn].y as i32 - (core::constants::TILEY as i32 / 2) + YSCUT;
    let ye = gs.characters[cn].y as i32 + (core::constants::TILEY as i32 / 2) - YECUT;
    let xs = gs.characters[cn].x as i32 - (core::constants::TILEX as i32 / 2) + XSCUT;
    let xe = gs.characters[cn].x as i32 + (core::constants::TILEX as i32 / 2) - XECUT;

    let current_x = gs.characters[cn].x as i32;
    let current_y = gs.characters[cn].y as i32;
    gs.can_see(
        Some(cn),
        current_x,
        current_y,
        current_x + 1,
        current_y + 1,
        (core::constants::TILEX / 2) as i32, // TODO: Re-evaluate if this is the right size...
    );

    let player_vx = gs.players[nr].vx;
    let player_vy = gs.players[nr].vy;
    let player_visi = gs.players[nr].visi;

    let see_x = gs.see_map[cn].x;
    let see_y = gs.see_map[cn].y;
    let see_vis = gs.see_map[cn].vis;

    let mut do_all = false;
    if player_vx != see_x || player_vy != see_y || player_visi != see_vis || player_visi != see_vis
    {
        gs.players[nr].vx = see_x;
        gs.players[nr].vy = see_y;
        gs.players[nr].visi = see_vis;
        do_all = true;
    }

    if gs.characters[cn].is_building() {
        do_all = true;
    }

    let empty_cmap = {
        let mut tile = CMap::default();
        tile.ba_sprite = core::constants::SPR_EMPTY as i16;
        tile
    };

    let empty_map = {
        let mut tile = core::types::Map::default();
        tile.sprite = core::constants::SPR_EMPTY;
        tile
    };

    let mut n = (YSCUT * core::constants::TILEX as i32 + XSCUT) as usize;
    let mut y = ys;
    let mut infra;
    while y < ye {
        let mut x = xs;
        while x < xe {
            // If we're outside the map, render the default empty tile and never touch map[]
            if x < 0
                || y < 0
                || x >= core::constants::SERVER_MAPX
                || y >= core::constants::SERVER_MAPY
            {
                let needs_update = do_all
                    || gs.players[nr].xmap[n] != empty_map
                    || gs.players[nr].smap[n] != empty_cmap;
                if needs_update {
                    gs.players[nr].xmap[n] = empty_map;
                    gs.players[nr].smap[n] = empty_cmap;
                }

                x += 1;
                n += 1;
                continue;
            }

            let mi = (x + y * core::constants::SERVER_MAPX) as usize;

            let map_m = gs.map[mi];
            if do_all || map_m.it != 0 || map_m.ch as usize != 0 || gs.players[nr].xmap[n] != map_m
            {
                gs.players[nr].xmap[n] = map_m;
            } else {
                // Still need to advance indices
                x += 1;
                n += 1;
                continue;
            }

            let tmp = gs.check_dlightm(mi);

            let mut light = std::cmp::max(gs.map[mi].light as i32, tmp);
            light = gs.do_character_calculate_light(cn, light);

            if light <= 5 && (gs.characters[cn].flags & CharacterFlags::Infrared.bits()) != 0 {
                infra = true;
            } else {
                infra = false;
            }

            // Everyone sees themselves at least
            if light == 0 && gs.map[mi].ch as usize == cn {
                light = 1;
            }

            // no light, nothing visible
            if light == 0 {
                gs.players[nr].smap[n] = empty_cmap;
                x += 1;
                n += 1;
                continue;
            }

            // Begin of flags
            smap[n].flags = 0;

            {
                let map_flags = gs.map[mi].flags;
                if map_flags
                    & (MF_GFX_INJURED
                        | MF_GFX_INJURED1
                        | MF_GFX_INJURED2
                        | MF_GFX_DEATH
                        | MF_GFX_TOMB
                        | MF_GFX_EMAGIC
                        | MF_GFX_GMAGIC
                        | MF_GFX_CMAGIC
                        | MF_UWATER as u64)
                    != 0
                {
                    if map_flags & core::constants::MF_GFX_INJURED != 0 {
                        smap[n].flags |= INJURED;
                    }

                    if map_flags & core::constants::MF_GFX_INJURED1 != 0 {
                        smap[n].flags |= INJURED1;
                    }

                    if map_flags & core::constants::MF_GFX_INJURED2 != 0 {
                        smap[n].flags |= INJURED2;
                    }

                    if map_flags & core::constants::MF_GFX_DEATH != 0 {
                        // TODO: Confirm shift
                        smap[n].flags |= ((map_flags & MF_GFX_DEATH) >> 23) as u32;
                    }

                    if map_flags & core::constants::MF_GFX_TOMB != 0 {
                        smap[n].flags |= ((map_flags & MF_GFX_TOMB) >> 23) as u32;
                    }

                    if map_flags & core::constants::MF_GFX_EMAGIC != 0 {
                        smap[n].flags |= ((map_flags & MF_GFX_EMAGIC) >> 23) as u32;
                    }

                    if map_flags & core::constants::MF_GFX_GMAGIC != 0 {
                        smap[n].flags |= ((map_flags & MF_GFX_GMAGIC) >> 23) as u32;
                    }

                    if map_flags & core::constants::MF_GFX_CMAGIC != 0 {
                        smap[n].flags |= ((map_flags & MF_GFX_CMAGIC) >> 23) as u32;
                    }

                    if map_flags & core::constants::MF_UWATER as u64 != 0 {
                        smap[n].flags |= UWATER;
                    }
                }

                if infra {
                    smap[n].flags |= INFRARED;
                }

                if gs.characters[cn].is_building() {
                    smap[n].flags2 = map_flags as u32;
                } else {
                    smap[n].flags2 = 0;
                }

                let rel_x = x - current_x + core::constants::VISI_CENTER;
                let rel_y = y - current_y + core::constants::VISI_CENTER;
                let edge = core::constants::VISI_STRIDE as i32 - 1;

                let visible = if rel_x <= 0 || rel_x >= edge || rel_y <= 0 || rel_y >= edge {
                    false
                } else {
                    let tmp_vis = rel_x as usize + rel_y as usize * core::constants::VISI_STRIDE;
                    let see = &gs.see_map[cn];
                    see.vis[tmp_vis] != 0
                        || see.vis[tmp_vis + core::constants::VISI_STRIDE] != 0
                        || see.vis[tmp_vis - core::constants::VISI_STRIDE] != 0
                        || see.vis[tmp_vis + 1] != 0
                        || see.vis[tmp_vis + 1 + core::constants::VISI_STRIDE] != 0
                        || see.vis[tmp_vis + 1 - core::constants::VISI_STRIDE] != 0
                        || see.vis[tmp_vis - 1] != 0
                        || see.vis[tmp_vis - 1 + core::constants::VISI_STRIDE] != 0
                        || see.vis[tmp_vis - 1 - core::constants::VISI_STRIDE] != 0
                };

                if !visible {
                    smap[n].flags |= INVIS;
                }

                // Begin of the light bucketing
                if light > 64 {
                    smap[n].light = 0;
                } else if light > 52 {
                    smap[n].light = 1;
                } else if light > 40 {
                    smap[n].light = 2;
                } else if light > 32 {
                    smap[n].light = 3;
                } else if light > 28 {
                    smap[n].light = 4;
                } else if light > 24 {
                    smap[n].light = 5;
                } else if light > 20 {
                    smap[n].light = 6;
                } else if light > 16 {
                    smap[n].light = 7;
                } else if light > 14 {
                    smap[n].light = 8;
                } else if light > 12 {
                    smap[n].light = 9;
                } else if light > 10 {
                    smap[n].light = 10;
                } else if light > 8 {
                    smap[n].light = 11;
                } else if light > 6 {
                    smap[n].light = 12;
                } else if light > 4 {
                    smap[n].light = 13;
                } else if light > 2 {
                    smap[n].light = 14;
                } else {
                    smap[n].light = 15;
                }

                smap[n].ba_sprite = map_m.sprite as i16;

                // Begin of character
                let co = map_m.ch as usize;
                let tmp_see = if visible && co != 0 {
                    gs.do_char_can_see(cn, co)
                } else {
                    0
                };

                if tmp_see != 0 {
                    let char_co = gs.characters[co];
                    if char_co.sprite_override != 0 {
                        smap[n].ch_sprite = char_co.sprite_override;
                    } else {
                        smap[n].ch_sprite = char_co.sprite as i16;
                    }
                    smap[n].ch_status = char_co.status as u8;
                    smap[n].ch_status2 = char_co.status2 as u8;
                    smap[n].ch_speed = char_co.speed as u8;
                    smap[n].ch_nr = co as u16;
                    smap[n].ch_id = helpers::char_id(&char_co) as u16;

                    if tmp_see <= 75 && char_co.hp[5] > 0 {
                        smap[n].ch_proz = (((char_co.a_hp + 5) / 10) / char_co.hp[5] as i32) as u8;
                    } else {
                        smap[n].ch_proz = 0;
                    }

                    smap[n].flags |= ISCHAR;

                    if char_co.stunned != 0 {
                        smap[n].flags |= STUNNED;
                    }

                    if char_co.flags & CharacterFlags::Stoned.bits() != 0 {
                        smap[n].flags |= STUNNED | STONED;
                    }
                } else {
                    // Just clear character flags
                    smap[n].ch_sprite = 0;
                    smap[n].ch_status = 0;
                    smap[n].ch_status2 = 0;
                    smap[n].ch_speed = 0;
                    smap[n].ch_nr = 0;
                    smap[n].ch_id = 0;
                    smap[n].ch_proz = 0;
                }

                // Begin of item
                let item_on_m = if map_m.it == 0 {
                    None
                } else {
                    Some(gs.items[map_m.it as usize])
                };
                if map_m.fsprite != 0 {
                    smap[n].it_sprite = map_m.fsprite as i16;
                    smap[n].it_status = 0;
                } else if item_on_m.is_some()
                    && (item_on_m.unwrap().flags & ItemFlags::IF_HIDDEN.bits()) == 0
                {
                    let item = item_on_m.unwrap();

                    if item.active != 0 {
                        smap[n].it_sprite = item.sprite[1];
                        smap[n].it_status = item.status[1];
                    } else {
                        smap[n].it_sprite = item.sprite[0];
                        smap[n].it_status = item.status[0];
                    }

                    if item.flags & ItemFlags::IF_LOOK.bits() != 0
                        || item.flags & ItemFlags::IF_LOOKSPECIAL.bits() != 0
                    {
                        smap[n].flags |= ISITEM;
                    }

                    if item.flags & ItemFlags::IF_TAKE.bits() == 0
                        && item.flags & (ItemFlags::IF_USE.bits() | ItemFlags::IF_USESPECIAL.bits())
                            != 0
                    {
                        smap[n].flags |= ISUSABLE;
                    }

                    if item.temp == core::constants::IT_TOMBSTONE as u16 {
                        smap[n].flags |= IS_GRAVE;
                    }
                } else {
                    // Just clear item flags
                    smap[n].it_sprite = 0;
                    smap[n].it_status = 0;
                }
            }

            gs.players[nr].smap[n] = smap[n];

            x += 1;
            n += 1;
        }

        y += 1;
        n += (XSCUT + XECUT) as usize;
    }

    gs.players[nr].vx = gs.see_map[cn].x;
    gs.players[nr].vy = gs.see_map[cn].y;
}

/// Light update functions - calculate efficiency of batch updates

/// Updates a single light tile (least efficient)
fn cl_light_one(gs: &mut GameState, n: usize, dosend: usize, update_only: bool) -> usize {
    if !update_only {
        // Return efficiency score: 50 * 1 / 3
        return 50 / 3;
    }

    let smap_light = gs.players[dosend].smap[n].light;
    gs.players[dosend].cmap[n].light = smap_light;

    // Packet layout: [cmd, idx_lo, idx_hi, light]
    // index is a full u16 (supports up to 65535 tiles; TILEX*TILEY=6400).
    let mut buf: [u8; 4] = [0; 4];
    buf[0] = ServerCommandType::SetMap4 as u8;
    buf[1] = (n & 0xff) as u8;
    buf[2] = ((n >> 8) & 0xff) as u8;
    buf[3] = smap_light & 0x0f;

    network_manager::xsend(gs, dosend, &buf, 4);
    1
}

/// Updates three light tiles
fn cl_light_three(gs: &mut GameState, n: usize, dosend: usize, update_only: bool) -> usize {
    if !update_only {
        let mut count = 0;
        let total = core::constants::TILEX * core::constants::TILEY;
        for m in n..std::cmp::min(n + 3, total) {
            if gs.players[dosend].cmap[m].light != gs.players[dosend].smap[m].light {
                count += 1;
            }
        }
        return 50 * count / 4;
    }

    // Packet layout: [cmd, idx_lo, idx_hi, light, nibble_pairs...]
    let mut buf: [u8; 5] = [0; 5];
    buf[0] = ServerCommandType::SetMap5 as u8;

    let smap_light = gs.players[dosend].smap[n].light;
    gs.players[dosend].cmap[n].light = smap_light;
    buf[1] = (n & 0xff) as u8;
    buf[2] = ((n >> 8) & 0xff) as u8;
    buf[3] = smap_light & 0x0f;

    let total = core::constants::TILEX * core::constants::TILEY;
    let mut p = 4;
    let mut m = n + 2;
    while m < std::cmp::min(n + 2 + 2, total) {
        let light_m = gs.players[dosend].smap[m].light;
        let light_m1 = gs.players[dosend].smap[m - 1].light;
        buf[p] = light_m | (light_m1 << 4);
        gs.players[dosend].cmap[m].light = light_m;
        gs.players[dosend].cmap[m - 1].light = light_m1;
        m += 2;
        p += 1;
    }

    network_manager::xsend(gs, dosend, &buf, 5);
    1
}

/// Updates seven light tiles
fn cl_light_seven(gs: &mut GameState, n: usize, dosend: usize, update_only: bool) -> usize {
    if !update_only {
        let mut count = 0;
        let total = core::constants::TILEX * core::constants::TILEY;
        for m in n..std::cmp::min(n + 7, total) {
            if gs.players[dosend].cmap[m].light != gs.players[dosend].smap[m].light {
                count += 1;
            }
        }
        return 50 * count / 6;
    }

    // Packet layout: [cmd, idx_lo, idx_hi, light, nibble_pairs...]
    let mut buf: [u8; 7] = [0; 7];
    buf[0] = ServerCommandType::SetMap6 as u8;

    let smap_light = gs.players[dosend].smap[n].light;
    gs.players[dosend].cmap[n].light = smap_light;
    buf[1] = (n & 0xff) as u8;
    buf[2] = ((n >> 8) & 0xff) as u8;
    buf[3] = smap_light & 0x0f;

    let total = core::constants::TILEX * core::constants::TILEY;
    let mut p = 4;
    let mut m = n + 2;
    while m < std::cmp::min(n + 6 + 2, total) {
        let light_m = gs.players[dosend].smap[m].light;
        let light_m1 = gs.players[dosend].smap[m - 1].light;
        buf[p] = light_m | (light_m1 << 4);
        gs.players[dosend].cmap[m].light = light_m;
        gs.players[dosend].cmap[m - 1].light = light_m1;
        m += 2;
        p += 1;
    }

    network_manager::xsend(gs, dosend, &buf, 7);
    1
}

/// Updates 27 light tiles (most efficient for large batches)
fn cl_light_26(gs: &mut GameState, n: usize, dosend: usize, update_only: bool) -> usize {
    if !update_only {
        let mut count = 0;
        let total = core::constants::TILEX * core::constants::TILEY;
        for m in n..std::cmp::min(n + 27, total) {
            if gs.players[dosend].cmap[m].light != gs.players[dosend].smap[m].light {
                count += 1;
            }
        }
        return 50 * count / 16;
    }

    // Packet layout: [cmd, idx_lo, idx_hi, light, nibble_pairs...]
    let mut buf: [u8; 17] = [0; 17];
    buf[0] = ServerCommandType::SetMap3 as u8;

    let smap_light = gs.players[dosend].smap[n].light;
    gs.players[dosend].cmap[n].light = smap_light;
    buf[1] = (n & 0xff) as u8;
    buf[2] = ((n >> 8) & 0xff) as u8;
    buf[3] = smap_light & 0x0f;

    let total = core::constants::TILEX * core::constants::TILEY;
    let mut p = 4;
    let mut m = n + 2;
    while m < std::cmp::min(n + 26 + 2, total) {
        let light_m = gs.players[dosend].smap[m].light;
        let light_m1 = gs.players[dosend].smap[m - 1].light;
        buf[p] = light_m | (light_m1 << 4);
        gs.players[dosend].cmap[m].light = light_m;
        gs.players[dosend].cmap[m - 1].light = light_m1;
        m += 2;
        p += 1;
    }

    network_manager::xsend(gs, dosend, &buf, 17);
    1
}

/// Send light updates for all changed tiles
pub fn plr_change_light(gs: &mut GameState, nr: usize) {
    let total = core::constants::TILEX * core::constants::TILEY;

    for n in 0..total {
        let light_changed = gs.players[nr].cmap[n].light != gs.players[nr].smap[n].light;

        if light_changed {
            // Try each light update function and pick the most efficient
            let mut best_efficiency = 0;
            let mut best_func = 0;

            let lfuncs: [fn(&mut GameState, usize, usize, bool) -> usize; 4] =
                [cl_light_one, cl_light_three, cl_light_seven, cl_light_26];

            for (idx, func) in lfuncs.iter().enumerate() {
                let efficiency = func(gs, n, nr, false);
                if efficiency >= best_efficiency {
                    best_efficiency = efficiency;
                    best_func = idx;
                }
            }

            // Execute the best function
            lfuncs[best_func](gs, n, nr, true);
        }
    }
}

/// Send map tile content updates for all changed tiles
pub fn plr_change_map(gs: &mut GameState, nr: usize) {
    let total = core::constants::TILEX * core::constants::TILEY;
    let mut lastn: i32 = -1;
    let mut n = 0;

    while n < total {
        // Find next difference (matching C++ fdiff behavior)
        let next_diff = gs.players[nr].cmap[n..]
            .iter()
            .zip(gs.players[nr].smap[n..].iter())
            .position(|(c, s)| c != s);

        match next_diff {
            Some(offset) => {
                n += offset;
            }
            None => {
                break; // No more differences
            }
        }

        // Build update packet and modify player data
        let updated = {
            let mut buf: [u8; 256] = [0; 256];
            let mut p: usize;

            if lastn >= 0 && (n as i32 - lastn) < 127 && n as i32 > lastn {
                buf[0] = ServerCommandType::SetMap as u8 | ((n as i32 - lastn) as u8);
                buf[1] = 0;
                p = 2;
            } else {
                buf[0] = ServerCommandType::SetMap as u8;
                buf[1] = 0;
                let n_bytes = (n as u16).to_le_bytes();
                buf[2] = n_bytes[0];
                buf[3] = n_bytes[1];
                p = 4;
            }

            let cmap = &gs.players[nr].cmap[n];
            let smap = &gs.players[nr].smap[n];

            // Check each field and add to update if changed
            if cmap.ba_sprite != smap.ba_sprite {
                buf[1] |= 1;
                let bytes = smap.ba_sprite.to_le_bytes();
                buf[p] = bytes[0];
                buf[p + 1] = bytes[1];
                p += 2;
            }

            if cmap.flags != smap.flags {
                buf[1] |= 2;
                let bytes = smap.flags.to_le_bytes();
                buf[p..p + 4].copy_from_slice(&bytes);
                p += 4;
            }

            if cmap.flags2 != smap.flags2 {
                buf[1] |= 4;
                let bytes = smap.flags2.to_le_bytes();
                buf[p..p + 4].copy_from_slice(&bytes);
                p += 4;
            }

            if cmap.it_sprite != smap.it_sprite {
                buf[1] |= 8;
                let bytes = smap.it_sprite.to_le_bytes();
                buf[p] = bytes[0];
                buf[p + 1] = bytes[1];
                p += 2;
            }

            if cmap.it_status != smap.it_status
                && helpers::it_base_status(cmap.it_status)
                    != helpers::it_base_status(smap.it_status)
            {
                buf[1] |= 16;
                buf[p] = smap.it_status;
                p += 1;
            }

            if cmap.ch_sprite != smap.ch_sprite
                || (cmap.ch_status != smap.ch_status
                    && helpers::ch_base_status(cmap.ch_status)
                        != helpers::ch_base_status(smap.ch_status))
                || cmap.ch_status2 != smap.ch_status2
            {
                buf[1] |= 32;
                let bytes = smap.ch_sprite.to_le_bytes();
                buf[p] = bytes[0];
                buf[p + 1] = bytes[1];
                p += 2;
                buf[p] = smap.ch_status;
                p += 1;
                buf[p] = smap.ch_status2;
                p += 1;
            }

            if cmap.ch_speed != smap.ch_speed
                || cmap.ch_nr != smap.ch_nr
                || cmap.ch_id != smap.ch_id
            {
                buf[1] |= 64;
                let nr_bytes = smap.ch_nr.to_le_bytes();
                buf[p] = nr_bytes[0];
                buf[p + 1] = nr_bytes[1];
                p += 2;
                let id_bytes = smap.ch_id.to_le_bytes();
                buf[p] = id_bytes[0];
                buf[p + 1] = id_bytes[1];
                p += 2;
                buf[p] = smap.ch_speed;
                p += 1;
            }

            if cmap.ch_proz != smap.ch_proz {
                buf[1] |= 128;
                buf[p] = smap.ch_proz;
                p += 1;
            }

            // Only send if we actually found changes (matching C++ if (buf[1]))
            let did_update = buf[1] != 0;
            if did_update {
                network_manager::xsend(gs, nr, &buf, p as u8);
            }

            gs.players[nr].cmap[n] = gs.players[nr].smap[n];

            did_update
        };

        // Update lastn after the modification (matching C++ behavior)
        if updated {
            lastn = n as i32;
        }

        n += 1;
    }
}

/// Send position change to player with map scrolling
pub fn plr_change_position(gs: &mut GameState, nr: usize, cn: usize) {
    let x = gs.characters[cn].x;
    let y = gs.characters[cn].y;
    let cpl_x = gs.players[nr].cpl.x;
    let cpl_y = gs.players[nr].cpl.y;

    if x as i32 != cpl_x || y as i32 != cpl_y {
        let mut buf: [u8; 16] = [0; 16];
        let tilex = core::constants::TILEX;
        let total = core::constants::TILEX * core::constants::TILEY;

        if cpl_x == (x as i32 - 1) && cpl_y == y as i32 {
            buf[0] = ServerCommandType::ScrollRight as u8;
            network_manager::xsend(gs, nr, &buf, 1);
            gs.players[nr].cmap.copy_within(1..total, 0);
        } else if cpl_x == (x as i32 + 1) && cpl_y == y as i32 {
            buf[0] = ServerCommandType::ScrollLeft as u8;
            network_manager::xsend(gs, nr, &buf, 1);
            gs.players[nr].cmap.copy_within(0..(total - 1), 1);
        } else if cpl_x == x as i32 && cpl_y == (y as i32 - 1) {
            buf[0] = ServerCommandType::ScrollDown as u8;
            network_manager::xsend(gs, nr, &buf, 1);
            gs.players[nr].cmap.copy_within(tilex..total, 0);
        } else if cpl_x == x as i32 && cpl_y == (y as i32 + 1) {
            buf[0] = ServerCommandType::ScrollUp as u8;
            network_manager::xsend(gs, nr, &buf, 1);
            gs.players[nr].cmap.copy_within(0..(total - tilex), tilex);
        } else if cpl_x == (x as i32 + 1) && cpl_y == (y as i32 + 1) {
            buf[0] = ServerCommandType::ScrollLeftUp as u8;
            network_manager::xsend(gs, nr, &buf, 1);
            gs.players[nr]
                .cmap
                .copy_within(0..(total - tilex - 1), tilex + 1);
        } else if cpl_x == (x as i32 + 1) && cpl_y == (y as i32 - 1) {
            buf[0] = ServerCommandType::ScrollLeftDown as u8;
            network_manager::xsend(gs, nr, &buf, 1);
            gs.players[nr].cmap.copy_within((tilex - 1)..total, 0);
        } else if cpl_x == (x as i32 - 1) && cpl_y == (y as i32 + 1) {
            buf[0] = ServerCommandType::ScrollRightUp as u8;
            network_manager::xsend(gs, nr, &buf, 1);
            gs.players[nr]
                .cmap
                .copy_within(0..(total - tilex + 1), tilex - 1);
        } else if cpl_x == (x as i32 - 1) && cpl_y == (y as i32 - 1) {
            buf[0] = ServerCommandType::ScrollRightDown as u8;
            network_manager::xsend(gs, nr, &buf, 1);
            let src_start = tilex + 1;
            let count = total - tilex - 1;
            gs.players[nr]
                .cmap
                .copy_within(src_start..(src_start + count), 0);
        }

        gs.players[nr].cpl.x = x as i32;
        gs.players[nr].cpl.y = y as i32;

        buf[0] = ServerCommandType::SetOrigin as u8;
        let ox: i16 = (x as i32 - (core::constants::TILEX as i32 / 2)) as i16;
        let oy: i16 = (y as i32 - (core::constants::TILEY as i32 / 2)) as i16;
        let ox_b = ox.to_le_bytes();
        let oy_b = oy.to_le_bytes();
        buf[1] = ox_b[0];
        buf[2] = ox_b[1];
        buf[3] = oy_b[0];
        buf[4] = oy_b[1];
        network_manager::xsend(gs, nr, &buf, 5);
    }
}
