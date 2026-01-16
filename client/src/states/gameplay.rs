use bevy::prelude::*;

use bevy::sprite::Anchor;

use crate::constants::{TARGET_HEIGHT, TARGET_WIDTH};
use crate::gfx_cache::GraphicsCache;
use crate::map::{TILEX, TILEY};
use crate::player_state::PlayerState;

use mag_core::constants::{SPR_EMPTY, STUNNED, TICKS};

// The original client uses a 32px grid step for draw positions.
const GRID_STEP: f32 = 32.0;

// In the original client, xoff starts with `-176` (to account for UI layout).
// Keeping this makes it easier to compare screenshots while we port rendering.
const MAP_X_SHIFT: f32 = -176.0;

const Z_BG: f32 = 0.0;
const Z_OBJ: f32 = 100.0;
const Z_CHAR: f32 = 200.0;

#[derive(Component)]
pub struct GameplayRenderEntity;

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct TileRender {
    index: usize,
    layer: TileLayer,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TileLayer {
    Background,
    Object,
    Character,
}

#[derive(Component, Clone, Copy, Debug, Default)]
pub(crate) struct LastRender {
    sprite_id: i32,
    sx: i32,
    sy: i32,
}

#[derive(Default)]
pub(crate) struct EngineClock {
    accumulator: f32,
    ticker: u32,
}

#[inline]
fn screen_to_world(sx: f32, sy: f32, z: f32) -> Vec3 {
    // Treat (0,0) as top-left in "screen" pixels like the original client.
    // Convert into Bevy world coordinates (origin centered, +Y up).
    Vec3::new(sx - TARGET_WIDTH * 0.5, TARGET_HEIGHT * 0.5 - sy, z)
}

fn spawn_tile_entity(commands: &mut Commands, gfx: &GraphicsCache, render: TileRender) {
    // Always spawn with a valid sprite handle; we'll swap it during updates.
    let Some(empty) = gfx.get_sprite(SPR_EMPTY as usize) else {
        return;
    };

    let initial_visibility = match render.layer {
        TileLayer::Background => Visibility::Visible,
        TileLayer::Object | TileLayer::Character => Visibility::Hidden,
    };

    commands.spawn((
        GameplayRenderEntity,
        render,
        LastRender {
            sprite_id: i32::MIN,
            sx: i32::MIN,
            sy: i32::MIN,
        },
        empty.clone(),
        Anchor::TOP_LEFT,
        Transform::default(),
        GlobalTransform::default(),
        initial_visibility,
        InheritedVisibility::default(),
        ViewVisibility::default(),
    ));
}

// Ported from engine.c
const SPEEDTAB: [[u8; 20]; 20] = [
    [1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1],
    [1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1],
    [1, 1, 1, 1, 1, 0, 1, 1, 1, 1, 1, 1, 1, 1, 0, 1, 1, 1, 1, 1],
    [1, 1, 1, 0, 1, 1, 1, 1, 1, 0, 1, 1, 1, 1, 1, 1, 0, 1, 1, 1],
    [1, 1, 0, 1, 1, 1, 1, 0, 1, 1, 1, 1, 0, 1, 1, 1, 1, 0, 1, 1],
    [1, 1, 0, 1, 1, 1, 0, 1, 1, 1, 0, 1, 1, 1, 0, 1, 1, 1, 0, 1],
    [1, 0, 1, 1, 0, 1, 1, 1, 0, 1, 1, 0, 1, 1, 0, 1, 1, 0, 1, 1],
    [1, 0, 1, 1, 0, 1, 1, 0, 1, 1, 0, 1, 1, 0, 1, 1, 0, 1, 1, 0],
    [0, 1, 1, 0, 1, 1, 0, 1, 0, 1, 1, 0, 1, 1, 0, 1, 0, 1, 0, 1],
    [0, 1, 0, 1, 0, 1, 0, 1, 1, 0, 1, 0, 1, 0, 1, 0, 1, 1, 0, 1],
    [1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0],
    [1, 0, 1, 0, 1, 0, 1, 0, 0, 1, 0, 1, 0, 1, 0, 1, 0, 0, 1, 0],
    [1, 0, 0, 1, 0, 0, 1, 0, 1, 0, 0, 1, 0, 0, 1, 0, 1, 0, 1, 0],
    [0, 1, 0, 0, 1, 0, 0, 1, 0, 0, 1, 0, 0, 1, 0, 0, 1, 0, 0, 1],
    [0, 1, 0, 0, 1, 0, 0, 0, 1, 0, 0, 1, 0, 0, 1, 0, 0, 1, 0, 0],
    [0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0],
    [0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0],
    [0, 0, 0, 1, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0],
    [0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
];

const STATTAB: [i32; 11] = [0, 1, 1, 6, 6, 2, 3, 4, 5, 7, 4];

#[inline]
fn speedo(ch_speed: u8, ctick: usize) -> bool {
    let speed = (ch_speed as usize).min(19);
    SPEEDTAB[speed][ctick.min(19)] != 0
}

fn speedstep(ch_speed: u8, ch_status: u8, d: i32, s: i32, update: bool, ctick: usize) -> i32 {
    let speed = (ch_speed as usize).min(19);
    let hard_step = (ch_status as i32) - d;
    if !update {
        return 32 * hard_step / s;
    }

    let mut z = ctick as i32;
    let mut soft_step = 0i32;
    let mut m = hard_step;

    while m != 0 {
        z -= 1;
        if z < 0 {
            z = 19;
        }
        soft_step += 1;
        if SPEEDTAB[speed][z as usize] != 0 {
            m -= 1;
        }
    }
    loop {
        z -= 1;
        if z < 0 {
            z = 19;
        }
        if SPEEDTAB[speed][z as usize] != 0 {
            break;
        }
        soft_step += 1;
    }

    let mut z = ctick as i32;
    let total_step_start = soft_step;
    let mut total_step = total_step_start;
    let mut m = s - hard_step;

    loop {
        if SPEEDTAB[speed][z as usize] != 0 {
            m -= 1;
        }
        if m < 1 {
            break;
        }
        z += 1;
        if z > 19 {
            z = 0;
        }
        total_step += 1;
    }

    32 * total_step_start / (total_step + 1)
}

#[inline]
fn do_idle(idle_ani: i32, sprite: u16) -> i32 {
    if sprite == 22480 {
        idle_ani
    } else {
        0
    }
}

fn eng_item(it_sprite: i16, it_status: &mut u8, ctick: usize, ticker: u32) -> i32 {
    let base = it_sprite as i32;
    match *it_status {
        0 | 1 => base,

        2 => {
            if SPEEDTAB[10][ctick] != 0 {
                *it_status = 3;
            }
            base
        }
        3 => {
            if SPEEDTAB[10][ctick] != 0 {
                *it_status = 4;
            }
            base + 2
        }
        4 => {
            if SPEEDTAB[10][ctick] != 0 {
                *it_status = 5;
            }
            base + 4
        }
        5 => {
            if SPEEDTAB[10][ctick] != 0 {
                *it_status = 2;
            }
            base + 6
        }

        6 => {
            if SPEEDTAB[10][ctick] != 0 {
                *it_status = 7;
            }
            base
        }
        7 => {
            if SPEEDTAB[10][ctick] != 0 {
                *it_status = 6;
            }
            base + 1
        }

        8 => {
            if SPEEDTAB[10][ctick] != 0 {
                *it_status = 9;
            }
            base
        }
        9 => {
            if SPEEDTAB[10][ctick] != 0 {
                *it_status = 10;
            }
            base + 1
        }
        10 => {
            if SPEEDTAB[10][ctick] != 0 {
                *it_status = 11;
            }
            base + 2
        }
        11 => {
            if SPEEDTAB[10][ctick] != 0 {
                *it_status = 12;
            }
            base + 3
        }
        12 => {
            if SPEEDTAB[10][ctick] != 0 {
                *it_status = 13;
            }
            base + 4
        }
        13 => {
            if SPEEDTAB[10][ctick] != 0 {
                *it_status = 14;
            }
            base + 5
        }
        14 => {
            if SPEEDTAB[10][ctick] != 0 {
                *it_status = 15;
            }
            base + 6
        }
        15 => {
            if SPEEDTAB[10][ctick] != 0 {
                *it_status = 8;
            }
            base + 7
        }

        16 => {
            if SPEEDTAB[10][ctick] != 0 {
                *it_status = 17;
            }
            base
        }
        17 => {
            if SPEEDTAB[10][ctick] != 0 {
                *it_status = 18;
            }
            base + 1
        }
        18 => {
            if SPEEDTAB[10][ctick] != 0 {
                *it_status = 19;
            }
            base + 2
        }
        19 => {
            if SPEEDTAB[10][ctick] != 0 {
                *it_status = 20;
            }
            base + 3
        }
        20 => {
            if SPEEDTAB[10][ctick] != 0 {
                *it_status = 16;
            }
            base + 4
        }
        21 => base + ((ticker & 63) as i32),

        _ => base,
    }
}

fn eng_char(tile: &mut crate::types::map::CMapTile, ctick: usize) -> i32 {
    let mut update = true;
    if (tile.flags & STUNNED) != 0 {
        update = false;
    }

    let ch_status = tile.ch_status;
    let base = tile.ch_sprite as i32;

    match ch_status {
        0 => {
            tile.obj_xoff = 0;
            tile.obj_yoff = 0;
            tile.idle_ani += 1;
            if tile.idle_ani > 7 {
                tile.idle_ani = 0;
            }
            base + 0 + do_idle(tile.idle_ani, tile.ch_sprite)
        }
        1 => {
            tile.obj_xoff = 0;
            tile.obj_yoff = 0;
            if speedo(tile.ch_speed, ctick) && update {
                tile.idle_ani += 1;
                if tile.idle_ani > 7 {
                    tile.idle_ani = 0;
                }
            }
            base + 8 + do_idle(tile.idle_ani, tile.ch_sprite)
        }
        2 => {
            tile.obj_xoff = 0;
            tile.obj_yoff = 0;
            if speedo(tile.ch_speed, ctick) && update {
                tile.idle_ani += 1;
                if tile.idle_ani > 7 {
                    tile.idle_ani = 0;
                }
            }
            base + 16 + do_idle(tile.idle_ani, tile.ch_sprite)
        }
        3 => {
            tile.obj_xoff = 0;
            tile.obj_yoff = 0;
            if speedo(tile.ch_speed, ctick) && update {
                tile.idle_ani += 1;
                if tile.idle_ani > 7 {
                    tile.idle_ani = 0;
                }
            }
            base + 24 + do_idle(tile.idle_ani, tile.ch_sprite)
        }
        4 => {
            tile.obj_xoff = 0;
            tile.obj_yoff = 0;
            if speedo(tile.ch_speed, ctick) && update {
                tile.idle_ani += 1;
                if tile.idle_ani > 7 {
                    tile.idle_ani = 0;
                }
            }
            base + 32 + do_idle(tile.idle_ani, tile.ch_sprite)
        }
        5 => {
            tile.obj_xoff = 0;
            tile.obj_yoff = 0;
            if speedo(tile.ch_speed, ctick) && update {
                tile.idle_ani += 1;
                if tile.idle_ani > 7 {
                    tile.idle_ani = 0;
                }
            }
            base + 40 + do_idle(tile.idle_ani, tile.ch_sprite)
        }
        6 => {
            tile.obj_xoff = 0;
            tile.obj_yoff = 0;
            if speedo(tile.ch_speed, ctick) && update {
                tile.idle_ani += 1;
                if tile.idle_ani > 7 {
                    tile.idle_ani = 0;
                }
            }
            base + 48 + do_idle(tile.idle_ani, tile.ch_sprite)
        }
        7 => {
            tile.obj_xoff = 0;
            tile.obj_yoff = 0;
            if speedo(tile.ch_speed, ctick) && update {
                tile.idle_ani += 1;
                if tile.idle_ani > 7 {
                    tile.idle_ani = 0;
                }
            }
            base + 56 + do_idle(tile.idle_ani, tile.ch_sprite)
        }

        16..=23 => {
            tile.obj_xoff = -speedstep(tile.ch_speed, tile.ch_status, 16, 8, update, ctick) / 2;
            tile.obj_yoff = speedstep(tile.ch_speed, tile.ch_status, 16, 8, update, ctick) / 4;
            let tmp = base + (tile.ch_status as i32 - 16) + 64;
            if speedo(tile.ch_speed, ctick) && update {
                tile.ch_status = if tile.ch_status == 23 {
                    16
                } else {
                    tile.ch_status + 1
                };
            }
            tmp
        }
        24..=31 => {
            tile.obj_xoff = speedstep(tile.ch_speed, tile.ch_status, 24, 8, update, ctick) / 2;
            tile.obj_yoff = -speedstep(tile.ch_speed, tile.ch_status, 24, 8, update, ctick) / 4;
            let tmp = base + (tile.ch_status as i32 - 24) + 72;
            if speedo(tile.ch_speed, ctick) && update {
                tile.ch_status = if tile.ch_status == 31 {
                    24
                } else {
                    tile.ch_status + 1
                };
            }
            tmp
        }
        32..=39 => {
            tile.obj_xoff = -speedstep(tile.ch_speed, tile.ch_status, 32, 8, update, ctick) / 2;
            tile.obj_yoff = -speedstep(tile.ch_speed, tile.ch_status, 32, 8, update, ctick) / 4;
            let tmp = base + (tile.ch_status as i32 - 32) + 80;
            if speedo(tile.ch_speed, ctick) && update {
                tile.ch_status = if tile.ch_status == 39 {
                    32
                } else {
                    tile.ch_status + 1
                };
            }
            tmp
        }
        40..=47 => {
            tile.obj_xoff = speedstep(tile.ch_speed, tile.ch_status, 40, 8, update, ctick) / 2;
            tile.obj_yoff = speedstep(tile.ch_speed, tile.ch_status, 40, 8, update, ctick) / 4;
            let tmp = base + (tile.ch_status as i32 - 40) + 88;
            if speedo(tile.ch_speed, ctick) && update {
                tile.ch_status = if tile.ch_status == 47 {
                    40
                } else {
                    tile.ch_status + 1
                };
            }
            tmp
        }

        48..=59 => {
            tile.obj_xoff = -speedstep(tile.ch_speed, tile.ch_status, 48, 12, update, ctick);
            tile.obj_yoff = 0;
            let tmp = base + ((tile.ch_status as i32 - 48) * 8 / 12) + 96;
            if speedo(tile.ch_speed, ctick) && update {
                tile.ch_status = if tile.ch_status == 59 {
                    48
                } else {
                    tile.ch_status + 1
                };
            }
            tmp
        }
        60..=71 => {
            tile.obj_xoff = 0;
            tile.obj_yoff = -speedstep(tile.ch_speed, tile.ch_status, 60, 12, update, ctick) / 2;
            let tmp = base + ((tile.ch_status as i32 - 60) * 8 / 12) + 104;
            if speedo(tile.ch_speed, ctick) && update {
                tile.ch_status = if tile.ch_status == 71 {
                    60
                } else {
                    tile.ch_status + 1
                };
            }
            tmp
        }
        72..=83 => {
            tile.obj_xoff = 0;
            tile.obj_yoff = speedstep(tile.ch_speed, tile.ch_status, 72, 12, update, ctick) / 2;
            let tmp = base + ((tile.ch_status as i32 - 72) * 8 / 12) + 112;
            if speedo(tile.ch_speed, ctick) && update {
                tile.ch_status = if tile.ch_status == 83 {
                    72
                } else {
                    tile.ch_status + 1
                };
            }
            tmp
        }
        84..=95 => {
            tile.obj_xoff = speedstep(tile.ch_speed, tile.ch_status, 84, 12, update, ctick);
            tile.obj_yoff = 0;
            let tmp = base + ((tile.ch_status as i32 - 84) * 8 / 12) + 120;
            if speedo(tile.ch_speed, ctick) && update {
                tile.ch_status = if tile.ch_status == 95 {
                    84
                } else {
                    tile.ch_status + 1
                };
            }
            tmp
        }

        96..=191 => {
            // Turns + misc animations. These all have zero offsets.
            tile.obj_xoff = 0;
            tile.obj_yoff = 0;

            let status = tile.ch_status as i32;
            let (start, base_add, wrap) = if (96..=99).contains(&tile.ch_status) {
                (96, 128, 96)
            } else if (100..=103).contains(&tile.ch_status) {
                (100, 132, 100)
            } else if (104..=107).contains(&tile.ch_status) {
                (104, 136, 104)
            } else if (108..=111).contains(&tile.ch_status) {
                (108, 140, 108)
            } else if (112..=115).contains(&tile.ch_status) {
                (112, 144, 112)
            } else if (116..=119).contains(&tile.ch_status) {
                (116, 148, 116)
            } else if (120..=123).contains(&tile.ch_status) {
                (120, 152, 120)
            } else if (124..=127).contains(&tile.ch_status) {
                (124, 156, 124)
            } else if (128..=131).contains(&tile.ch_status) {
                (128, 160, 128)
            } else if (132..=135).contains(&tile.ch_status) {
                (132, 164, 132)
            } else if (136..=139).contains(&tile.ch_status) {
                (136, 168, 136)
            } else if (140..=143).contains(&tile.ch_status) {
                (140, 172, 140)
            } else if (144..=147).contains(&tile.ch_status) {
                (144, 176, 144)
            } else if (148..=151).contains(&tile.ch_status) {
                (148, 180, 148)
            } else if (152..=155).contains(&tile.ch_status) {
                (152, 184, 152)
            } else if (156..=159).contains(&tile.ch_status) {
                (156, 188, 156)
            } else if (160..=167).contains(&tile.ch_status) {
                (160, 192, 160)
            } else if (168..=175).contains(&tile.ch_status) {
                (168, 200, 168)
            } else if (176..=183).contains(&tile.ch_status) {
                (176, 208, 176)
            } else {
                (184, 216, 184)
            };

            let stat_off = (tile.ch_stat_off as usize).min(STATTAB.len() - 1);
            let stat_add = if (160..=191).contains(&tile.ch_status) {
                STATTAB[stat_off] << 5
            } else {
                0
            };

            let frame = status - start;
            let tmp = base + frame + base_add + stat_add;

            if speedo(tile.ch_speed, ctick) && update {
                // Wrap points: last frame is +3 for turns, +7 for misc.
                let max = if (160..=191).contains(&tile.ch_status) {
                    start + 7
                } else {
                    start + 3
                };
                if tile.ch_status as i32 >= max {
                    tile.ch_status = wrap;
                } else {
                    tile.ch_status = tile.ch_status.saturating_add(1);
                }
            }

            tmp
        }

        _ => base,
    }
}

fn engine_tick(player_state: &mut PlayerState, clock: &mut EngineClock) {
    // Step at 20Hz like the original client.
    clock.ticker = clock.ticker.wrapping_add(1);
    let ctick = (clock.ticker % 20) as usize;

    let map = player_state.map_mut();
    let len = map.len();

    for i in 0..len {
        let Some(tile) = map.tile_at_index_mut(i) else {
            continue;
        };
        tile.back = 0;
        tile.obj1 = 0;
        tile.obj2 = 0;
        tile.ovl_xoff = 0;
        tile.ovl_yoff = 0;
    }

    for i in 0..len {
        let Some(tile) = map.tile_at_index_mut(i) else {
            continue;
        };

        tile.back = tile.ba_sprite as i32;

        if tile.it_sprite != 0 {
            let sprite = eng_item(tile.it_sprite, &mut tile.it_status, ctick, clock.ticker);
            tile.obj1 = sprite;
        }

        if tile.ch_sprite != 0 {
            let sprite = eng_char(tile, ctick);
            tile.obj2 = sprite;
        }
    }
}

pub(crate) fn setup_gameplay(
    mut commands: Commands,
    gfx: Res<GraphicsCache>,
    player_state: Res<PlayerState>,
    existing_render: Query<Entity, With<GameplayRenderEntity>>,
) {
    log::debug!("setup_gameplay - start");

    // Clear any previous gameplay sprites (re-entering gameplay, etc.)
    for e in &existing_render {
        commands.entity(e).despawn();
    }

    if !gfx.is_initialized() {
        log::warn!("Gameplay entered before GraphicsCache initialized");
        return;
    }

    let map = player_state.map();

    // Spawn a stable set of entities once; `run_gameplay` updates them.
    for index in 0..map.len() {
        spawn_tile_entity(
            &mut commands,
            &gfx,
            TileRender {
                index,
                layer: TileLayer::Background,
            },
        );
        spawn_tile_entity(
            &mut commands,
            &gfx,
            TileRender {
                index,
                layer: TileLayer::Object,
            },
        );
        spawn_tile_entity(
            &mut commands,
            &gfx,
            TileRender {
                index,
                layer: TileLayer::Character,
            },
        );
    }

    log::debug!("setup_gameplay - end");
}

pub(crate) fn run_gameplay(
    time: Res<Time>,
    gfx: Res<GraphicsCache>,
    mut player_state: ResMut<PlayerState>,
    mut clock: Local<EngineClock>,
    mut q: Query<(
        &TileRender,
        &mut Sprite,
        &mut Transform,
        &mut Visibility,
        &mut LastRender,
    )>,
) {
    if !gfx.is_initialized() {
        return;
    }

    // Ensure we have computed `back/obj1/obj2` at least once so gameplay doesn't start
    // with a fully empty screen until the first 20Hz tick.
    if clock.ticker == 0 {
        engine_tick(&mut player_state, &mut clock);
    }

    // Fixed-step the original engine_tick at 20Hz.
    let tick_hz = TICKS as f32;
    let tick_dt = 1.0 / tick_hz;
    clock.accumulator += time.delta_secs();
    while clock.accumulator >= tick_dt {
        engine_tick(&mut player_state, &mut clock);
        clock.accumulator -= tick_dt;
    }

    let map = player_state.map();

    // Match original engine.c: xoff/yoff are based on the center tile's obj offsets.
    let (xoff, yoff) = map
        .tile_at_xy(TILEX / 2, TILEY / 2)
        .map(|center| {
            (
                -(center.obj_xoff as f32) + MAP_X_SHIFT,
                -(center.obj_yoff as f32),
            )
        })
        .unwrap_or((MAP_X_SHIFT, 0.0));

    for (render, mut sprite, mut transform, mut visibility, mut last) in &mut q {
        let Some(tile) = map.tile_at_index(render.index) else {
            continue;
        };

        let x = render.index % TILEX;
        let y = render.index / TILEX;

        let draw_order = ((TILEY - 1 - y) * TILEX + x) as f32;
        let base_sx = x as f32 * GRID_STEP + xoff;
        let base_sy = y as f32 * GRID_STEP + yoff;

        let (sprite_id, sx, sy, z) = match render.layer {
            TileLayer::Background => {
                let id = if tile.back != 0 {
                    tile.back
                } else {
                    SPR_EMPTY as i32
                };
                (id, base_sx, base_sy, Z_BG + draw_order * 0.01)
            }
            TileLayer::Object => (tile.obj1, base_sx, base_sy, Z_OBJ + draw_order * 0.01),
            TileLayer::Character => (
                tile.obj2,
                base_sx + tile.obj_xoff as f32,
                base_sy + tile.obj_yoff as f32,
                Z_CHAR + draw_order * 0.01,
            ),
        };

        let sx_i = sx.round() as i32;
        let sy_i = sy.round() as i32;
        if sprite_id == last.sprite_id && sx_i == last.sx && sy_i == last.sy {
            // Nothing changed.
            continue;
        }

        last.sprite_id = sprite_id;
        last.sx = sx_i;
        last.sy = sy_i;

        if sprite_id <= 0 {
            *visibility = Visibility::Hidden;
            continue;
        }

        let Some(src) = gfx.get_sprite(sprite_id as usize) else {
            *visibility = Visibility::Hidden;
            continue;
        };

        *sprite = src.clone();
        *visibility = Visibility::Visible;
        transform.translation = screen_to_world(sx, sy, z);
    }
}
