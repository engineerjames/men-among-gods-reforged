use mag_core::constants::{MAX_SPEEDTAB_SPEED_INDEX, SPEEDTAB, STUNNED};

use crate::player_state::PlayerState;
use crate::types::map::CMapTile;

const STATTAB: [i32; 11] = [0, 1, 1, 6, 6, 2, 3, 4, 5, 7, 4];

#[inline]
fn speedo(ch_speed: u8, ctick: usize) -> bool {
    let speed = (ch_speed as usize).min(MAX_SPEEDTAB_SPEED_INDEX);
    let tick = ctick.min(SPEEDTAB[0].len() - 1);
    SPEEDTAB[speed][tick] != 0
}

fn speedstep(ch_speed: u8, ch_status: u8, d: i32, s: i32, update: bool, ctick: usize) -> i32 {
    let speed = (ch_speed as usize).min(MAX_SPEEDTAB_SPEED_INDEX);
    let max_tick = (SPEEDTAB[0].len() - 1) as i32;

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
            z = max_tick;
        }
        soft_step += 1;
        if SPEEDTAB[speed][z as usize] != 0 {
            m -= 1;
        }
    }

    loop {
        z -= 1;
        if z < 0 {
            z = max_tick;
        }
        if SPEEDTAB[speed][z as usize] != 0 {
            break;
        }
        soft_step += 1;
    }

    let z = ctick as i32;
    let total_step_start = soft_step;
    let mut total_step = total_step_start;
    let mut m = s - hard_step;

    let mut z2 = z;
    loop {
        if SPEEDTAB[speed][z2 as usize] != 0 {
            m -= 1;
        }
        if m < 1 {
            break;
        }
        z2 += 1;
        if z2 > max_tick {
            z2 = 0;
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

fn eng_item(it_sprite: u16, it_status: &mut u8, ctick: usize, ticker: u32) -> i32 {
    let base = it_sprite as i32;
    let tick = ctick.min(SPEEDTAB[0].len() - 1);

    match *it_status {
        0 | 1 => base,
        2 => {
            if SPEEDTAB[10][tick] != 0 {
                *it_status = 3;
            }
            base
        }
        3 => {
            if SPEEDTAB[10][tick] != 0 {
                *it_status = 4;
            }
            base + 2
        }
        4 => {
            if SPEEDTAB[10][tick] != 0 {
                *it_status = 5;
            }
            base + 4
        }
        5 => {
            if SPEEDTAB[10][tick] != 0 {
                *it_status = 2;
            }
            base + 6
        }
        6 => {
            if SPEEDTAB[10][tick] != 0 {
                *it_status = 7;
            }
            base
        }
        7 => {
            if SPEEDTAB[10][tick] != 0 {
                *it_status = 6;
            }
            base + 1
        }
        8 => {
            if SPEEDTAB[10][tick] != 0 {
                *it_status = 9;
            }
            base
        }
        9 => {
            if SPEEDTAB[10][tick] != 0 {
                *it_status = 10;
            }
            base + 1
        }
        10 => {
            if SPEEDTAB[10][tick] != 0 {
                *it_status = 11;
            }
            base + 2
        }
        11 => {
            if SPEEDTAB[10][tick] != 0 {
                *it_status = 12;
            }
            base + 3
        }
        12 => {
            if SPEEDTAB[10][tick] != 0 {
                *it_status = 13;
            }
            base + 4
        }
        13 => {
            if SPEEDTAB[10][tick] != 0 {
                *it_status = 14;
            }
            base + 5
        }
        14 => {
            if SPEEDTAB[10][tick] != 0 {
                *it_status = 15;
            }
            base + 6
        }
        15 => {
            if SPEEDTAB[10][tick] != 0 {
                *it_status = 8;
            }
            base + 7
        }
        16 => {
            if SPEEDTAB[10][tick] != 0 {
                *it_status = 17;
            }
            base
        }
        17 => {
            if SPEEDTAB[10][tick] != 0 {
                *it_status = 18;
            }
            base + 1
        }
        18 => {
            if SPEEDTAB[10][tick] != 0 {
                *it_status = 19;
            }
            base + 2
        }
        19 => {
            if SPEEDTAB[10][tick] != 0 {
                *it_status = 20;
            }
            base + 3
        }
        20 => {
            if SPEEDTAB[10][tick] != 0 {
                *it_status = 16;
            }
            base + 4
        }
        21 => base + ((ticker & 63) as i32),
        _ => base,
    }
}

fn eng_char(tile: &mut CMapTile, ctick: usize) -> i32 {
    let update = (tile.flags & STUNNED) == 0;

    let ch_status = tile.ch_status;
    let base = tile.ch_sprite as i32;

    match ch_status {
        0..=7 => {
            tile.obj_xoff = 0;
            tile.obj_yoff = 0;
            if ch_status == 0 || (speedo(tile.ch_speed, ctick) && update) {
                tile.idle_ani += 1;
                if tile.idle_ani > 7 {
                    tile.idle_ani = 0;
                }
            }
            base + (ch_status as i32) * 8 + do_idle(tile.idle_ani, tile.ch_sprite)
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

        _ => {
            tile.obj_xoff = 0;
            tile.obj_yoff = 0;
            base
        }
    }
}

pub fn engine_tick(player_state: &mut PlayerState, ticker: u32, ctick: usize) {
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
            tile.obj1 = eng_item(tile.it_sprite, &mut tile.it_status, ctick, ticker);
        }

        if tile.ch_sprite != 0 {
            tile.obj2 = eng_char(tile, ctick);
        }
    }
}
