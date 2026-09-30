use core::{
    constants::{
        CharacterFlags, DANGER_GLYPH_MASK, DangerGlyph, INFRARED, INJURED, INJURED1, INJURED2,
        INVIS, IS_GRAVE, ISCHAR, ISITEM, ISUSABLE, ItemFlags, MF_GFX_CMAGIC, MF_GFX_DEATH,
        MF_GFX_EMAGIC, MF_GFX_GMAGIC, MF_GFX_INJURED, MF_GFX_INJURED1, MF_GFX_INJURED2,
        MF_GFX_TOMB, MF_UWATER, STONED, STUNNED, UWATER,
    },
    logout_reasons::LogoutReason,
    server_commands::ServerCommandType,
};

use crate::{
    driver,
    game_state::GameState,
    helpers,
    player::{connection::plr_logout, update::PlayerUpdateCtx},
    server::ServerPlayer,
};

#[cfg(test)]
use crate::player::update;

/// Client-side map tile
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct CMap {
    // for background
    pub ba_sprite: i16, // background image
    pub light: u8,
    pub flags: u32,
    pub flags2: u32,

    // for character
    pub ch_sprite: i16, // basic sprite of character
    pub ch_status2: u8,
    pub ch_status: u8, // what the character is doing, animation-wise
    pub ch_speed: u8,  // speed of animation (movement/turn frames)
    pub ch_aspeed: u8, // speed of attack/misc-action animation
    pub ch_nr: u16,
    pub ch_id: u16,
    pub ch_proz: u8, // health in percent

    // for item
    pub it_sprite: i16, // basic sprite of item
    pub it_status: u8,  // for items with animation (burning torches etc)
}

/// Classifies a visible NPC relative to the viewing character.
///
/// Players, merchants, and bodies never receive a danger glyph. NPCs are
/// classified from their rank difference rather than transient combat state,
/// so the nameplate remains stable while the target is visible.
fn danger_glyph_for(
    viewer: &core::types::Character,
    target: &core::types::Character,
) -> DangerGlyph {
    if target.flags
        & (CharacterFlags::Player.bits()
            | CharacterFlags::Merchant.bits()
            | CharacterFlags::Body.bits())
        != 0
    {
        return DangerGlyph::None;
    }

    let viewer_rank = core::ranks::points2rank(viewer.points_tot.max(0) as u32) as i32;
    let target_rank = core::ranks::points2rank(target.points_tot.max(0) as u32) as i32;
    DangerGlyph::from_rank_delta(target_rank - viewer_rank)
}

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
        gs.do_add_light(i32::from(x), i32::from(y), -i32::from(light));
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
                        i32::from(gs.characters[cn].x),
                        i32::from(gs.characters[cn].y),
                        i32::from(gs.characters[cn].frx),
                        i32::from(gs.characters[cn].fry),
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
                            gs.do_add_light(nx, ny, i32::from(light));
                        }

                        return;
                    }
                }

                if ret == -1 {
                    gs.map[m].to_ch = 0;

                    let (frx, fry, light) = (
                        i32::from(gs.characters[cn].frx),
                        i32::from(gs.characters[cn].fry),
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
                        gs.do_add_light(frx, fry, i32::from(light));
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
                        gs.do_add_light(i32::from(tx), i32::from(ty), i32::from(current_light));
                    }
                    return;
                }
            }
        }

        // Check for tavern
        let is_tavern = (gs.map[m].flags & u64::from(core::constants::MF_TAVERN)) != 0;

        if is_tavern && is_player {
            gs.characters[cn].tavern_x = gs.characters[cn].x as u16;
            gs.characters[cn].tavern_y = gs.characters[cn].y as u16;

            log::info!("Character {} entered tavern", cn);

            let player_id = gs.characters[cn].player;
            plr_logout(gs, cn, player_id as usize, LogoutReason::Tavern);
            return;
        }

        // Check for no magic zone, respect items that exempt char from nomagic
        let is_nomagic = (gs.map[m].flags & u64::from(core::constants::MF_NOMAGIC)) != 0;

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
            gs.do_add_light(i32::from(x), i32::from(y), i32::from(light));
        }

        // Check for death trap
        let is_deathtrap = (gs.map[m].flags & u64::from(core::constants::MF_DEATHTRAP)) != 0;

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
        i32::from(x),
        i32::from(y),
        i32::from(core::constants::NT_SEE),
        cn as i32,
        0,
        0,
        0,
    );
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
/// Legacy entry point: build and send the player's visible map.
///
/// Production runs the context form via `update::run_player_updates`; this
/// shim keeps the `(gs, nr)` call shape for unit tests.
///
/// # Arguments
/// * `nr` - Player slot index requesting the map update
#[cfg(test)]
pub fn plr_getmap(gs: &mut GameState, nr: usize) {
    plr_getmap_complete(gs, nr);
}

/// Builds and sends the complete visible map snapshot for a player.
///
/// # Arguments
///
/// * `gs` - Active game state containing player map caches and world tiles.
/// * `nr` - Player slot index requesting the complete map update.
///
/// # Panics
///
/// * Panics if `nr`, the player's character index, or a calculated map index is invalid.
#[cfg(test)]
pub fn plr_getmap_complete(gs: &mut GameState, nr: usize) {
    update::with_player_ctx(gs, nr, plr_getmap_ctx);
}

/// Context form of [`plr_getmap`]; runs on a tick worker.
///
/// # Arguments
///
/// * `ctx` - Per-player update context.
pub fn plr_getmap_ctx(ctx: &mut PlayerUpdateCtx) {
    plr_getmap_complete_ctx(ctx);
}

/// Context form of [`plr_getmap_complete`]: rebuilds the player's `smap`
/// from the shared world without touching any other player.
///
/// # Arguments
///
/// * `ctx` - Per-player update context.
///
/// # Panics
///
/// * Panics if the player's character index or a calculated map index is invalid.
pub fn plr_getmap_complete_ctx(ctx: &mut PlayerUpdateCtx) {
    let cn = ctx.cn;
    let world = ctx.world;
    let daylight_changed = ctx.player.last_dlight != world.globals.dlight;

    const YSCUT: i32 = 3;
    const YECUT: i32 = 1;
    const XSCUT: i32 = 2;
    const XECUT: i32 = 2;

    let viewer = &world.characters[cn];
    let ys = i32::from(viewer.y) - (core::constants::TILEY as i32 / 2) + YSCUT;
    let ye = i32::from(viewer.y) + (core::constants::TILEY as i32 / 2) - YECUT;
    let xs = i32::from(viewer.x) - (core::constants::TILEX as i32 / 2) + XSCUT;
    let xe = i32::from(viewer.x) + (core::constants::TILEX as i32 / 2) - XECUT;

    let current_x = i32::from(viewer.x);
    let current_y = i32::from(viewer.y);
    ctx.can_see(
        current_x,
        current_y,
        current_x + 1,
        current_y + 1,
        (core::constants::TILEX / 2) as i32, // TODO: Re-evaluate if this is the right size...
    );

    let mut do_all = false;
    if ctx.player.vx != ctx.see.x || ctx.player.vy != ctx.see.y || ctx.player.visi != ctx.see.vis {
        ctx.player.vx = ctx.see.x;
        ctx.player.vy = ctx.see.y;
        ctx.player.visi = ctx.see.vis;
        do_all = true;
    }

    let empty_cmap = CMap {
        ba_sprite: core::constants::SPR_EMPTY as i16,
        ..CMap::default()
    };

    let empty_map = core::types::Map {
        sprite: core::constants::SPR_EMPTY,
        ..core::types::Map::default()
    };

    // Light depends on the viewer only through these three values, so resolve
    // them once instead of re-reading the character on all ~5.8k tiles.
    let dlight = world.globals.dlight;
    let percept = i32::from(viewer.skill[core::skills::SK_PERCEPT][5]);
    let infrared = viewer.flags & CharacterFlags::Infrared.bits() != 0;
    let percept_scale = std::cmp::min(percept, 10);

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
                let needs_update =
                    do_all || ctx.player.xmap[n] != empty_map || ctx.player.smap[n] != empty_cmap;
                if needs_update {
                    ctx.player.xmap[n] = empty_map;
                    ctx.player.smap[n] = empty_cmap;
                }

                x += 1;
                n += 1;
                continue;
            }

            let mi = (x + y * core::constants::SERVER_MAPX) as usize;

            let map_m = world.map[mi];
            if daylight_changed
                || do_all
                || map_m.it != 0
                || map_m.ch as usize != 0
                || ctx.player.xmap[n] != map_m
            {
                ctx.player.xmap[n] = map_m;
            } else {
                // Still need to advance indices
                x += 1;
                n += 1;
                continue;
            }

            let tmp = if map_m.flags & u64::from(core::constants::MF_INDOORS) == 0 {
                dlight
            } else {
                (dlight * i32::from(map_m.dlight)) / 256
            };

            let mut light = std::cmp::max(i32::from(map_m.light), tmp);

            // Inlined `do_character_calculate_light` using the hoisted values.
            if light == 0 && percept > 150 {
                light = 1;
            }
            light = light * percept_scale / 10;
            if light > 255 {
                light = 255;
            }
            if infrared && light < 5 {
                light = 5;
            }

            infra = light <= 5 && infrared;

            // Everyone sees themselves at least
            if light == 0 && map_m.ch as usize == cn {
                light = 1;
            }

            // no light, nothing visible
            if light == 0 {
                ctx.player.smap[n] = CMap {
                    flags: INVIS,
                    ..empty_cmap
                };
                x += 1;
                n += 1;
                continue;
            }

            // Scratch copy of just this tile; committed below. Cloning the
            // player's whole 6400-tile map here instead would cost ~180 KB of
            // memcpy per player per tick.
            let mut tile = ctx.player.smap[n];

            // Begin of flags
            tile.flags = 0;

            {
                let map_flags = map_m.flags;
                if map_flags
                    & (MF_GFX_INJURED
                        | MF_GFX_INJURED1
                        | MF_GFX_INJURED2
                        | MF_GFX_DEATH
                        | MF_GFX_TOMB
                        | MF_GFX_EMAGIC
                        | MF_GFX_GMAGIC
                        | MF_GFX_CMAGIC
                        | u64::from(MF_UWATER))
                    != 0
                {
                    if map_flags & core::constants::MF_GFX_INJURED != 0 {
                        tile.flags |= INJURED;
                    }

                    if map_flags & core::constants::MF_GFX_INJURED1 != 0 {
                        tile.flags |= INJURED1;
                    }

                    if map_flags & core::constants::MF_GFX_INJURED2 != 0 {
                        tile.flags |= INJURED2;
                    }

                    if map_flags & core::constants::MF_GFX_DEATH != 0 {
                        // TODO: Confirm shift
                        tile.flags |= ((map_flags & MF_GFX_DEATH) >> 23) as u32;
                    }

                    if map_flags & core::constants::MF_GFX_TOMB != 0 {
                        tile.flags |= ((map_flags & MF_GFX_TOMB) >> 23) as u32;
                    }

                    if map_flags & core::constants::MF_GFX_EMAGIC != 0 {
                        tile.flags |= ((map_flags & MF_GFX_EMAGIC) >> 23) as u32;
                    }

                    if map_flags & core::constants::MF_GFX_GMAGIC != 0 {
                        tile.flags |= ((map_flags & MF_GFX_GMAGIC) >> 23) as u32;
                    }

                    if map_flags & core::constants::MF_GFX_CMAGIC != 0 {
                        tile.flags |= ((map_flags & MF_GFX_CMAGIC) >> 23) as u32;
                    }

                    if map_flags & u64::from(core::constants::MF_UWATER) != 0 {
                        tile.flags |= UWATER;
                    }
                }

                if infra {
                    tile.flags |= INFRARED;
                }

                // Low 32 bits of the raw map flags (MF_MOVEBLOCK, MF_INDOORS,
                // MF_TAVERN, etc.) forwarded verbatim so the client can key
                // rendering/effects off them; GFX_* bits (>=32) are dropped by
                // the truncation and are already surfaced via `tile.flags`
                // above.
                tile.flags2 = (map_flags as u32) & !DANGER_GLYPH_MASK;

                let rel_x = x - current_x + core::constants::VISI_CENTER;
                let rel_y = y - current_y + core::constants::VISI_CENTER;
                let edge = core::constants::VISI_STRIDE as i32 - 1;

                let visible = if rel_x <= 0 || rel_x >= edge || rel_y <= 0 || rel_y >= edge {
                    false
                } else {
                    let tmp_vis = rel_x as usize + rel_y as usize * core::constants::VISI_STRIDE;
                    let see = &*ctx.see;
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
                    tile.flags |= INVIS;
                }

                // Begin of the light bucketing
                if light > 64 {
                    tile.light = 0;
                } else if light > 52 {
                    tile.light = 1;
                } else if light > 40 {
                    tile.light = 2;
                } else if light > 32 {
                    tile.light = 3;
                } else if light > 28 {
                    tile.light = 4;
                } else if light > 24 {
                    tile.light = 5;
                } else if light > 20 {
                    tile.light = 6;
                } else if light > 16 {
                    tile.light = 7;
                } else if light > 14 {
                    tile.light = 8;
                } else if light > 12 {
                    tile.light = 9;
                } else if light > 10 {
                    tile.light = 10;
                } else if light > 8 {
                    tile.light = 11;
                } else if light > 6 {
                    tile.light = 12;
                } else if light > 4 {
                    tile.light = 13;
                } else if light > 2 {
                    tile.light = 14;
                } else {
                    tile.light = 15;
                }

                tile.ba_sprite = map_m.sprite as i16;

                // Begin of character
                let co = map_m.ch as usize;
                let tmp_see = if visible && co != 0 {
                    ctx.char_can_see(co)
                } else {
                    0
                };

                if tmp_see != 0 {
                    let char_co = &world.characters[co];
                    tile.flags2 |= danger_glyph_for(viewer, char_co).bits();
                    if char_co.sprite_override != 0 {
                        tile.ch_sprite = char_co.sprite_override;
                    } else {
                        tile.ch_sprite = char_co.sprite as i16;
                    }
                    tile.ch_status = char_co.status as u8;
                    tile.ch_status2 = char_co.status2 as u8;
                    tile.ch_speed = char_co.speed as u8;
                    tile.ch_aspeed = char_co.future3[2] as u8;
                    tile.ch_nr = co as u16;
                    tile.ch_id = helpers::char_id(char_co) as u16;

                    if tmp_see <= 75 && char_co.hp[5] > 0 {
                        tile.ch_proz = (((char_co.a_hp + 5) / 10) / i32::from(char_co.hp[5])) as u8;
                    } else {
                        tile.ch_proz = 0;
                    }

                    tile.flags |= ISCHAR;

                    if char_co.stunned != 0 {
                        tile.flags |= STUNNED;
                    }

                    if char_co.flags & CharacterFlags::Stoned.bits() != 0 {
                        tile.flags |= STUNNED | STONED;
                    }
                } else {
                    // Just clear character flags
                    tile.ch_sprite = 0;
                    tile.ch_status = 0;
                    tile.ch_status2 = 0;
                    tile.ch_speed = 0;
                    tile.ch_aspeed = 0;
                    tile.ch_nr = 0;
                    tile.ch_id = 0;
                    tile.ch_proz = 0;
                }

                // Begin of item
                let item_on_m = if map_m.it == 0 {
                    None
                } else {
                    Some(&world.items[map_m.it as usize])
                };
                if map_m.fsprite != 0 {
                    tile.it_sprite = map_m.fsprite as i16;
                    tile.it_status = 0;
                } else if let Some(item) =
                    item_on_m.filter(|item| item.flags & ItemFlags::IF_HIDDEN.bits() == 0)
                {
                    if item.active != 0 {
                        tile.it_sprite = item.sprite[1];
                        tile.it_status = item.status[1];
                    } else {
                        tile.it_sprite = item.sprite[0];
                        tile.it_status = item.status[0];
                    }

                    if item.flags & ItemFlags::IF_LOOK.bits() != 0
                        || item.flags & ItemFlags::IF_LOOKSPECIAL.bits() != 0
                    {
                        tile.flags |= ISITEM;
                    }

                    if item.flags & ItemFlags::IF_TAKE.bits() == 0
                        && item.flags & (ItemFlags::IF_USE.bits() | ItemFlags::IF_USESPECIAL.bits())
                            != 0
                    {
                        tile.flags |= ISUSABLE;
                    }

                    if item.temp == core::constants::IT_TOMBSTONE as u16 {
                        tile.flags |= IS_GRAVE;
                    }
                } else {
                    // Just clear item flags
                    tile.it_sprite = 0;
                    tile.it_status = 0;
                }
            }

            ctx.player.smap[n] = tile;

            x += 1;
            n += 1;
        }

        y += 1;
        n += (XSCUT + XECUT) as usize;
    }

    ctx.player.vx = ctx.see.x;
    ctx.player.vy = ctx.see.y;
    ctx.player.last_dlight = world.globals.dlight;
}

/// One light-delta packet shape: how many tiles it covers and how it is
/// scored against the alternatives.
struct LightBatch {
    /// Tiles covered starting at the changed index.
    span: usize,
    /// Wire command for this shape.
    cmd: ServerCommandType,
    /// Bytes sent for this shape.
    packet_len: usize,
    /// Efficiency divisor (`50 * changed / divisor`).
    divisor: usize,
}

/// Light batch shapes in legacy evaluation order (one, three, seven, 27).
const LIGHT_BATCHES: [LightBatch; 4] = [
    LightBatch {
        span: 1,
        cmd: ServerCommandType::SetMap4,
        packet_len: 4,
        divisor: 3,
    },
    LightBatch {
        span: 3,
        cmd: ServerCommandType::SetMap5,
        packet_len: 5,
        divisor: 4,
    },
    LightBatch {
        span: 7,
        cmd: ServerCommandType::SetMap6,
        packet_len: 7,
        divisor: 6,
    },
    LightBatch {
        span: 27,
        cmd: ServerCommandType::SetMap3,
        packet_len: 17,
        divisor: 16,
    },
];

/// Score a light batch shape at tile `n` (higher is better).
fn cl_light_efficiency(player: &ServerPlayer, n: usize, batch: &LightBatch) -> usize {
    if batch.span == 1 {
        return 50 / 3;
    }

    let total = core::constants::TILEX * core::constants::TILEY;
    let count = (n..std::cmp::min(n + batch.span, total))
        .filter(|&m| player.cmap[m].light != player.smap[m].light)
        .count();
    50 * count / batch.divisor
}

/// Emit one light batch starting at tile `n` and sync `cmap` for the tiles
/// it carries.
///
/// Packet layout: `[cmd, idx_lo, idx_hi, light, nibble_pairs...]`; the index
/// is a full u16 (`TILEX * TILEY = 6400`).
fn cl_light_send(ctx: &mut PlayerUpdateCtx, n: usize, batch: &LightBatch) {
    let mut buf = [0u8; 17];
    buf[0] = batch.cmd as u8;

    let smap_light = ctx.player.smap[n].light;
    ctx.player.cmap[n].light = smap_light;
    buf[1] = (n & 0xff) as u8;
    buf[2] = ((n >> 8) & 0xff) as u8;
    buf[3] = smap_light & 0x0f;

    if batch.span > 1 {
        let total = core::constants::TILEX * core::constants::TILEY;
        let mut p = 4;
        let mut m = n + 2;
        while m < std::cmp::min(n + batch.span + 1, total) {
            let light_m = ctx.player.smap[m].light;
            let light_m1 = ctx.player.smap[m - 1].light;
            buf[p] = light_m | (light_m1 << 4);
            ctx.player.cmap[m].light = light_m;
            ctx.player.cmap[m - 1].light = light_m1;
            m += 2;
            p += 1;
        }
    }

    ctx.xsend(&buf, batch.packet_len);
}

/// Send light updates for all changed tiles
///
/// # Arguments
///
/// * `gs` - Active game state used by this function.
/// * `nr` - Numeric identifier used by this function.
#[cfg(test)]
pub fn plr_change_light(gs: &mut GameState, nr: usize) {
    update::with_player_ctx(gs, nr, plr_change_light_ctx);
}

/// Context form of [`plr_change_light`].
///
/// # Arguments
///
/// * `ctx` - Per-player update context.
pub fn plr_change_light_ctx(ctx: &mut PlayerUpdateCtx) {
    let total = core::constants::TILEX * core::constants::TILEY;

    for n in 0..total {
        let light_changed = ctx.player.cmap[n].light != ctx.player.smap[n].light;

        if light_changed {
            // Try each light batch shape and pick the most efficient
            let mut best_efficiency = 0;
            let mut best_batch = 0;

            for (idx, batch) in LIGHT_BATCHES.iter().enumerate() {
                let efficiency = cl_light_efficiency(ctx.player, n, batch);
                if efficiency >= best_efficiency {
                    best_efficiency = efficiency;
                    best_batch = idx;
                }
            }

            cl_light_send(ctx, n, &LIGHT_BATCHES[best_batch]);
        }
    }
}

/// Send map tile content updates for all changed tiles
///
/// # Arguments
///
/// * `gs` - Active game state used by this function.
/// * `nr` - Numeric identifier used by this function.
#[cfg(test)]
pub fn plr_change_map(gs: &mut GameState, nr: usize) {
    update::with_player_ctx(gs, nr, plr_change_map_ctx);
}

/// Context form of [`plr_change_map`].
///
/// # Arguments
///
/// * `ctx` - Per-player update context.
pub fn plr_change_map_ctx(ctx: &mut PlayerUpdateCtx) {
    let total = core::constants::TILEX * core::constants::TILEY;
    let mut lastn: i32 = -1;
    let mut n = 0;

    while n < total {
        // Find next difference (matching C++ fdiff behavior)
        let next_diff = ctx.player.cmap[n..]
            .iter()
            .zip(ctx.player.smap[n..].iter())
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

            let cmap = &ctx.player.cmap[n];
            let smap = &ctx.player.smap[n];

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
                || cmap.ch_aspeed != smap.ch_aspeed
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
                buf[p] = smap.ch_aspeed;
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
                ctx.xsend(&buf, p);
            }

            ctx.player.cmap[n] = ctx.player.smap[n];

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
///
/// # Arguments
///
/// * `gs` - Active game state used by this function.
/// * `nr` - Numeric identifier used by this function.
/// * `cn` - Character index used by this function.
#[cfg(test)]
pub fn plr_change_position(gs: &mut GameState, nr: usize, cn: usize) {
    update::with_player_ctx(gs, nr, |ctx| {
        ctx.cn = cn;
        plr_change_position_ctx(ctx);
    });
}

/// Context form of [`plr_change_position`].
///
/// # Arguments
///
/// * `ctx` - Per-player update context.
pub fn plr_change_position_ctx(ctx: &mut PlayerUpdateCtx) {
    let ch = &ctx.world.characters[ctx.cn];
    let x = ch.x;
    let y = ch.y;
    let cpl_x = ctx.player.cpl.x;
    let cpl_y = ctx.player.cpl.y;

    if i32::from(x) != cpl_x || i32::from(y) != cpl_y {
        let mut buf: [u8; 16] = [0; 16];
        let tilex = core::constants::TILEX;
        let total = core::constants::TILEX * core::constants::TILEY;

        if cpl_x == (i32::from(x) - 1) && cpl_y == i32::from(y) {
            buf[0] = ServerCommandType::ScrollRight as u8;
            ctx.xsend(&buf, 1);
            ctx.player.cmap.copy_within(1..total, 0);
        } else if cpl_x == (i32::from(x) + 1) && cpl_y == i32::from(y) {
            buf[0] = ServerCommandType::ScrollLeft as u8;
            ctx.xsend(&buf, 1);
            ctx.player.cmap.copy_within(0..(total - 1), 1);
        } else if cpl_x == i32::from(x) && cpl_y == (i32::from(y) - 1) {
            buf[0] = ServerCommandType::ScrollDown as u8;
            ctx.xsend(&buf, 1);
            ctx.player.cmap.copy_within(tilex..total, 0);
        } else if cpl_x == i32::from(x) && cpl_y == (i32::from(y) + 1) {
            buf[0] = ServerCommandType::ScrollUp as u8;
            ctx.xsend(&buf, 1);
            ctx.player.cmap.copy_within(0..(total - tilex), tilex);
        } else if cpl_x == (i32::from(x) + 1) && cpl_y == (i32::from(y) + 1) {
            buf[0] = ServerCommandType::ScrollLeftUp as u8;
            ctx.xsend(&buf, 1);
            ctx.player
                .cmap
                .copy_within(0..(total - tilex - 1), tilex + 1);
        } else if cpl_x == (i32::from(x) + 1) && cpl_y == (i32::from(y) - 1) {
            buf[0] = ServerCommandType::ScrollLeftDown as u8;
            ctx.xsend(&buf, 1);
            ctx.player.cmap.copy_within((tilex - 1)..total, 0);
        } else if cpl_x == (i32::from(x) - 1) && cpl_y == (i32::from(y) + 1) {
            buf[0] = ServerCommandType::ScrollRightUp as u8;
            ctx.xsend(&buf, 1);
            ctx.player
                .cmap
                .copy_within(0..(total - tilex + 1), tilex - 1);
        } else if cpl_x == (i32::from(x) - 1) && cpl_y == (i32::from(y) - 1) {
            buf[0] = ServerCommandType::ScrollRightDown as u8;
            ctx.xsend(&buf, 1);
            let src_start = tilex + 1;
            let count = total - tilex - 1;
            ctx.player
                .cmap
                .copy_within(src_start..(src_start + count), 0);
        }

        ctx.player.cpl.x = i32::from(x);
        ctx.player.cpl.y = i32::from(y);

        buf[0] = ServerCommandType::SetOrigin as u8;
        let ox: i16 = (i32::from(x) - (core::constants::TILEX as i32 / 2)) as i16;
        let oy: i16 = (i32::from(y) - (core::constants::TILEY as i32 / 2)) as i16;
        let ox_b = ox.to_le_bytes();
        let oy_b = oy.to_le_bytes();
        buf[1] = ox_b[0];
        buf[2] = ox_b[1];
        buf[3] = oy_b[0];
        buf[4] = oy_b[1];
        ctx.xsend(&buf, 5);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{TcpListener, TcpStream};

    use crate::{
        test_helpers::{add_test_player, with_test_gs},
        tls::GameStream,
    };
    use core::{
        constants::{
            CharacterFlags, ItemFlags, MF_DEATHTRAP, MF_NOMAGIC, MF_TAVERN, ST_EXIT, TILEX, TILEY,
            USE_ACTIVE,
        },
        types::{Character, Map},
    };

    fn attach_test_socket(gs: &mut GameState, nr: usize) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind test listener");
        let addr = listener.local_addr().expect("listener addr");
        let client = TcpStream::connect(addr).expect("connect client");
        let (server, _) = listener.accept().expect("accept client");
        drop(client);
        gs.players[nr].sock = Some(GameStream::Plain(server));
    }

    fn reset_tbuf(gs: &mut GameState, nr: usize) {
        gs.players[nr].tptr = 0;
        gs.players[nr].tbuf.fill(0);
    }

    fn map_index(x: i16, y: i16) -> usize {
        x as usize + y as usize * core::constants::SERVER_MAPX as usize
    }

    fn small_map_index(gs: &GameState, cn: usize, x: i32, y: i32) -> usize {
        const YSCUT: i32 = 3;
        const XSCUT: i32 = 2;

        let ys = i32::from(gs.characters[cn].y) - (TILEY as i32 / 2) + YSCUT;
        let xs = i32::from(gs.characters[cn].x) - (TILEX as i32 / 2) + XSCUT;
        let base = (YSCUT * TILEX as i32 + XSCUT) as usize;
        base + (y - ys) as usize * TILEX + (x - xs) as usize
    }

    fn make_tile(sprite: i16, light: u8) -> CMap {
        CMap {
            ba_sprite: sprite,
            light,
            ..CMap::default()
        }
    }

    #[test]
    fn danger_glyph_for_only_marks_severe_rank_buckets() {
        let mut viewer = Character::default();
        let mut target = Character::default();

        viewer.points_tot = 48_950;
        target.points_tot = 0;
        assert_eq!(danger_glyph_for(&viewer, &target), DangerGlyph::None);

        viewer.points_tot = 850;
        target.points_tot = 17_700;
        assert_eq!(danger_glyph_for(&viewer, &target), DangerGlyph::None);

        viewer.points_tot = 0;
        target.points_tot = 17_700;
        assert_eq!(danger_glyph_for(&viewer, &target), DangerGlyph::Skull);

        target.points_tot = 48_950;
        assert_eq!(
            danger_glyph_for(&viewer, &target),
            DangerGlyph::FlamingSkull
        );
    }

    #[test]
    fn danger_glyph_for_excludes_players_merchants_and_bodies() {
        let viewer = Character::default();
        let mut target = Character {
            points_tot: i32::MAX,
            ..Character::default()
        };

        for excluded_flag in [
            CharacterFlags::Player,
            CharacterFlags::Merchant,
            CharacterFlags::Body,
        ] {
            target.flags = excluded_flag.bits();
            assert_eq!(danger_glyph_for(&viewer, &target), DangerGlyph::None);
        }
    }

    #[test]
    fn plr_map_remove_clears_tile_links_and_light() {
        with_test_gs(|gs| {
            let (cn, _) = add_test_player(gs);
            gs.characters[cn].x = 10;
            gs.characters[cn].y = 10;
            gs.characters[cn].tox = 11;
            gs.characters[cn].toy = 10;
            gs.characters[cn].light = 7;

            let here = map_index(10, 10);
            let there = map_index(11, 10);
            gs.map[here].ch = cn as u32;
            gs.map[here].light = 7;
            gs.map[there].to_ch = cn as u32;

            plr_map_remove(gs, cn);

            assert_eq!(gs.map[here].ch, 0);
            assert_eq!(gs.map[there].to_ch, 0);
            assert_eq!(gs.map[here].light, 0);
        });
    }

    #[test]
    fn plr_map_set_updates_nomagic_state_and_map_occupancy() {
        with_test_gs(|gs| {
            let (cn, _) = add_test_player(gs);
            let tile = map_index(gs.characters[cn].x, gs.characters[cn].y);
            gs.characters[cn].flags &= !CharacterFlags::NoMagic.bits();
            gs.characters[cn].light = 4;
            gs.map[tile].flags = u64::from(MF_NOMAGIC);

            plr_map_set(gs, cn);

            assert_eq!(gs.map[tile].ch, cn as u32);
            assert_eq!(gs.map[tile].to_ch, 0);
            assert_ne!(gs.characters[cn].flags & CharacterFlags::NoMagic.bits(), 0);
            assert!(gs.map[tile].light > 0);

            gs.map[tile].ch = 0;
            gs.map[tile].flags = 0;
            plr_map_set(gs, cn);

            assert_eq!(gs.map[tile].ch, cn as u32);
            assert_eq!(gs.characters[cn].flags & CharacterFlags::NoMagic.bits(), 0);
        });
    }

    #[test]
    fn plr_map_set_logs_players_out_in_taverns() {
        with_test_gs(|gs| {
            let (cn, nr) = add_test_player(gs);
            attach_test_socket(gs, nr);
            let tile = map_index(gs.characters[cn].x, gs.characters[cn].y);
            gs.map[tile].flags = u64::from(MF_TAVERN);

            plr_map_set(gs, cn);

            assert_eq!(gs.players[nr].state, ST_EXIT);
            assert_eq!(gs.characters[cn].tavern_x, 10);
            assert_eq!(gs.characters[cn].tavern_y, 10);
        });
    }

    #[test]
    fn plr_map_set_deathtrap_kills_character() {
        with_test_gs(|gs| {
            let (cn, _) = add_test_player(gs);
            let tile = map_index(gs.characters[cn].x, gs.characters[cn].y);
            gs.characters[cn].temple_x = 20;
            gs.characters[cn].temple_y = 20;
            gs.map[tile].flags = u64::from(MF_DEATHTRAP);

            plr_map_set(gs, cn);

            assert_eq!(gs.characters[cn].a_hp, 10000);
            assert_eq!(gs.characters[cn].x, 20);
            assert_eq!(gs.characters[cn].y, 20);
        });
    }

    #[test]
    fn plr_getmap_and_complete_populate_visible_tiles() {
        with_test_gs(|gs| {
            let (cn, nr) = add_test_player(gs);
            let tile = map_index(10, 10);
            gs.map[tile] = Map {
                sprite: 321,
                ch: cn as u32,
                it: 1,
                ..Map::default()
            };
            gs.items[1].used = USE_ACTIVE;
            gs.items[1].sprite[0] = 77;
            gs.items[1].status[0] = 9;
            gs.items[1].flags = (ItemFlags::IF_LOOK | ItemFlags::IF_USE).bits();
            gs.items[1].temp = core::constants::IT_TOMBSTONE as u16;

            plr_getmap(gs, nr);

            let sm_idx = small_map_index(gs, cn, 10, 10);
            let tile = gs.players[nr].smap[sm_idx];
            assert_eq!(tile.ba_sprite, 321);
            assert_ne!(tile.flags & ISCHAR, 0);
            assert_ne!(tile.flags & ISITEM, 0);
            assert_ne!(tile.flags & IS_GRAVE, 0);
            assert_eq!(tile.it_sprite, 77);
            assert_eq!(gs.players[nr].vx, gs.see_map[cn].x);
            assert_eq!(gs.players[nr].vy, gs.see_map[cn].y);
        });
    }

    #[test]
    fn plr_getmap_forwards_raw_map_flags_into_flags2() {
        with_test_gs(|gs| {
            let (cn, nr) = add_test_player(gs);
            let x = i32::from(gs.characters[cn].x);
            let y = i32::from(gs.characters[cn].y);
            let tile = map_index(x as i16, y as i16);
            gs.map[tile] = Map {
                ch: cn as u32,
                flags: u64::from(core::constants::MF_INDOORS | MF_TAVERN),
                ..Map::default()
            };

            plr_getmap(gs, nr);

            let sm_idx = small_map_index(gs, cn, x, y);
            let tile = gs.players[nr].smap[sm_idx];
            assert_ne!(tile.flags2 & core::constants::MF_INDOORS, 0);
            assert_ne!(tile.flags2 & MF_TAVERN, 0);
        });
    }

    #[test]
    fn plr_getmap_refreshes_static_tile_light_when_daylight_changes() {
        with_test_gs(|gs| {
            let (cn, nr) = add_test_player(gs);
            gs.characters[cn].skill[core::skills::SK_PERCEPT][5] = 10;
            let static_x = i32::from(gs.characters[cn].x) + 1;
            let static_y = i32::from(gs.characters[cn].y);
            let static_tile = map_index(static_x as i16, static_y as i16);
            gs.map[static_tile].sprite = 321;

            gs.globals.dlight = 1;
            plr_getmap(gs, nr);
            let sm_idx = small_map_index(gs, cn, static_x, static_y);
            let night_light = gs.players[nr].smap[sm_idx].light;

            gs.globals.dlight = 65;
            plr_getmap(gs, nr);

            assert_ne!(gs.players[nr].smap[sm_idx].light, night_light);
            assert_eq!(gs.players[nr].last_dlight, 65);
        });
    }

    #[test]
    fn plr_getmap_marks_dark_previously_visible_tiles_invisible() {
        with_test_gs(|gs| {
            let (cn, nr) = add_test_player(gs);
            gs.characters[cn].skill[core::skills::SK_PERCEPT][5] = 10;
            let static_x = i32::from(gs.characters[cn].x) + 1;
            let static_y = i32::from(gs.characters[cn].y);
            let static_tile = map_index(static_x as i16, static_y as i16);
            gs.map[static_tile].sprite = 321;

            gs.globals.dlight = 65;
            plr_getmap(gs, nr);
            let sm_idx = small_map_index(gs, cn, static_x, static_y);
            assert_eq!(gs.players[nr].smap[sm_idx].flags & INVIS, 0);

            gs.globals.dlight = 0;
            plr_getmap(gs, nr);

            assert_ne!(gs.players[nr].smap[sm_idx].flags & INVIS, 0);
        });
    }

    /// Regression test for the "black map until first move" login bug.
    ///
    /// A freshly-created character has an all-zero `skill` array until
    /// `GameState::really_update_char` populates the derived
    /// `skill[SK_PERCEPT][5]` value. `do_character_calculate_light`
    /// multiplies ambient light by that value, so without the recompute
    /// every tile except the character's own reads as unlit and gets
    /// flagged `INVIS`, even in broad daylight. `plr_login` must call
    /// `really_update_char` synchronously (before the first `plr_getmap`
    /// runs later in the same tick) to avoid this.
    #[test]
    fn plr_getmap_needs_stats_recomputed_before_first_view_is_correct() {
        with_test_gs(|gs| {
            let (cn, nr) = add_test_player(gs);
            let static_x = i32::from(gs.characters[cn].x) + 1;
            let static_y = i32::from(gs.characters[cn].y);
            let static_tile = map_index(static_x as i16, static_y as i16);
            gs.map[static_tile].sprite = 321;
            gs.globals.dlight = 65;

            // Simulate a brand-new character: skill array is all zero, so
            // `really_update_char` has never run for this character yet.
            assert_eq!(gs.characters[cn].skill[core::skills::SK_PERCEPT][5], 0);

            plr_getmap(gs, nr);
            let sm_idx = small_map_index(gs, cn, static_x, static_y);
            assert_ne!(
                gs.players[nr].smap[sm_idx].flags & INVIS,
                0,
                "tile should (incorrectly) read as invisible before stats are recomputed"
            );

            // `plr_login` now calls this synchronously; simulate that fix
            // and force a fresh view build (matching a new connection's
            // sentinel `last_dlight = -1`).
            gs.really_update_char(cn);
            gs.players[nr].last_dlight = -1;
            plr_getmap(gs, nr);

            assert_eq!(
                gs.players[nr].smap[sm_idx].flags & INVIS,
                0,
                "tile should be visible once derived stats are populated"
            );
        });
    }

    #[test]
    fn light_packet_helpers_update_cmap_and_encode_expected_payloads() {
        with_test_gs(|gs| {
            let (_, nr) = add_test_player(gs);
            attach_test_socket(gs, nr);

            let [one, three, seven, twenty_seven] = &LIGHT_BATCHES;

            gs.players[nr].smap[3].light = 5;
            assert_eq!(cl_light_efficiency(&gs.players[nr], 3, one), 16);
            reset_tbuf(gs, nr);
            update::with_player_ctx(gs, nr, |ctx| cl_light_send(ctx, 3, one));
            assert_eq!(gs.players[nr].cmap[3].light, 5);
            assert_eq!(gs.players[nr].tptr, 4);
            assert_eq!(gs.players[nr].tbuf[0], ServerCommandType::SetMap4 as u8);

            reset_tbuf(gs, nr);
            gs.players[nr].cmap[8] = CMap::default();
            gs.players[nr].cmap[9] = CMap::default();
            gs.players[nr].cmap[10] = CMap::default();
            gs.players[nr].smap[8].light = 1;
            gs.players[nr].smap[9].light = 2;
            gs.players[nr].smap[10].light = 3;
            assert_eq!(cl_light_efficiency(&gs.players[nr], 8, three), 37);
            update::with_player_ctx(gs, nr, |ctx| cl_light_send(ctx, 8, three));
            assert_eq!(gs.players[nr].tptr, 5);
            assert_eq!(gs.players[nr].tbuf[0], ServerCommandType::SetMap5 as u8);
            assert_eq!(gs.players[nr].tbuf[4], 3 | (2 << 4));

            reset_tbuf(gs, nr);
            for idx in 20..27 {
                gs.players[nr].cmap[idx].light = 0;
                gs.players[nr].smap[idx].light = (idx - 19) as u8;
            }
            assert_eq!(cl_light_efficiency(&gs.players[nr], 20, seven), 58);
            update::with_player_ctx(gs, nr, |ctx| cl_light_send(ctx, 20, seven));
            assert_eq!(gs.players[nr].tptr, 7);
            assert_eq!(gs.players[nr].tbuf[0], ServerCommandType::SetMap6 as u8);

            reset_tbuf(gs, nr);
            for idx in 40..67 {
                gs.players[nr].cmap[idx].light = 0;
                gs.players[nr].smap[idx].light = (((idx - 39) % 15) + 1) as u8;
            }
            assert_eq!(cl_light_efficiency(&gs.players[nr], 40, twenty_seven), 84);
            update::with_player_ctx(gs, nr, |ctx| cl_light_send(ctx, 40, twenty_seven));
            assert_eq!(gs.players[nr].tptr, 17);
            assert_eq!(gs.players[nr].tbuf[0], ServerCommandType::SetMap3 as u8);
        });
    }

    #[test]
    fn plr_change_light_picks_batch_update_and_syncs_cmap() {
        with_test_gs(|gs| {
            let (_, nr) = add_test_player(gs);
            attach_test_socket(gs, nr);
            for idx in 0..27 {
                gs.players[nr].cmap[idx].light = 0;
                gs.players[nr].smap[idx].light = ((idx % 15) + 1) as u8;
            }

            plr_change_light(gs, nr);

            assert_eq!(gs.players[nr].tptr, 17);
            assert_eq!(gs.players[nr].tbuf[0], ServerCommandType::SetMap3 as u8);
            for idx in 0..27 {
                assert_eq!(
                    gs.players[nr].cmap[idx].light,
                    gs.players[nr].smap[idx].light
                );
            }
        });
    }

    #[test]
    fn plr_change_map_sends_absolute_and_relative_updates() {
        with_test_gs(|gs| {
            let (_, nr) = add_test_player(gs);
            attach_test_socket(gs, nr);
            gs.players[nr].cmap[3] = CMap::default();
            gs.players[nr].smap[3] = CMap::default();
            gs.players[nr].smap[3].ba_sprite = 55;
            gs.players[nr].cmap[5] = CMap::default();
            gs.players[nr].smap[5] = CMap::default();
            gs.players[nr].smap[5].flags = ISITEM;

            plr_change_map(gs, nr);

            assert_eq!(gs.players[nr].tbuf[0], ServerCommandType::SetMap as u8);
            assert_eq!(gs.players[nr].tbuf[1], 1);
            assert_eq!(
                u16::from_le_bytes([gs.players[nr].tbuf[2], gs.players[nr].tbuf[3]]),
                3
            );
            assert_eq!(
                i16::from_le_bytes([gs.players[nr].tbuf[4], gs.players[nr].tbuf[5]]),
                55
            );

            let second_packet = 6;
            assert_eq!(
                gs.players[nr].tbuf[second_packet],
                ServerCommandType::SetMap as u8 | 2
            );
            assert_ne!(gs.players[nr].tbuf[second_packet + 1] & 2, 0);
            assert_eq!(
                u32::from_le_bytes([
                    gs.players[nr].tbuf[second_packet + 2],
                    gs.players[nr].tbuf[second_packet + 3],
                    gs.players[nr].tbuf[second_packet + 4],
                    gs.players[nr].tbuf[second_packet + 5],
                ]),
                ISITEM
            );
            assert_eq!(gs.players[nr].cmap[3], gs.players[nr].smap[3]);
            assert_eq!(gs.players[nr].cmap[5], gs.players[nr].smap[5]);
        });
    }

    #[test]
    fn plr_change_position_scrolls_and_sets_origin() {
        with_test_gs(|gs| {
            let (cn, nr) = add_test_player(gs);
            attach_test_socket(gs, nr);
            gs.players[nr].cpl.x = 9;
            gs.players[nr].cpl.y = 10;
            gs.players[nr].cmap[0] = make_tile(10, 1);
            gs.players[nr].cmap[1] = make_tile(20, 2);

            plr_change_position(gs, nr, cn);

            assert_eq!(gs.players[nr].tptr, 6);
            assert_eq!(gs.players[nr].tbuf[0], ServerCommandType::ScrollRight as u8);
            assert_eq!(gs.players[nr].tbuf[1], ServerCommandType::SetOrigin as u8);
            assert_eq!(gs.players[nr].cpl.x, 10);
            assert_eq!(gs.players[nr].cpl.y, 10);
            assert_eq!(gs.players[nr].cmap[0].ba_sprite, 20);

            reset_tbuf(gs, nr);
            plr_change_position(gs, nr, cn);
            assert_eq!(gs.players[nr].tptr, 0);
        });
    }
}
