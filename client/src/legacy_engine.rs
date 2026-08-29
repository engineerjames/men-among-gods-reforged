use mag_core::constants::{MAX_SPEEDTAB_SPEED_INDEX, SPEEDTAB, STUNNED};

use crate::player_state::PlayerState;
use crate::types::map::{CMapTile, SUBPIXEL_UNIT};

/// Look-up table mapping `ch_stat_off` to a sprite-row offset used by
/// attack/emote animation frames (status range 160–191).
const STATTAB: [i32; 11] = [0, 1, 1, 6, 6, 2, 3, 4, 5, 7, 4];

/// Returns `true` if the given `ch_speed` index says the character should
/// advance its animation frame on `ctick`.
///
/// # Arguments
/// * `ch_speed` - Speed table row (0 = every tick, higher = slower).
/// * `ctick` - The current local animation tick counter.
///
/// # Returns
/// * `true` when the speed table entry is non-zero.
#[inline]
fn speedo(ch_speed: u8, ctick: usize) -> bool {
    let speed = (ch_speed as usize).min(MAX_SPEEDTAB_SPEED_INDEX);
    let tick = ctick.min(SPEEDTAB[0].len() - 1);
    SPEEDTAB[speed][tick] != 0
}

/// Computes the smooth sub-tile offset for a moving character.
///
/// Implements the C client's `speedstep()` which interpolates between discrete
/// tile positions based on the speed table, producing smooth 32-pixel-range
/// offsets for in-between frames.
///
/// The result is expressed in [`SUBPIXEL_UNIT`] units rather than whole pixels.
/// The original C client rounded here, which made every character land on its
/// own independently rounded pixel; two characters walking in the same
/// direction a constant sub-pixel distance apart then alternated between two
/// pixel positions every frame. Keeping the fractional part and rounding once
/// at the final screen coordinate removes that shimmer.
///
/// # Arguments
/// * `ch_speed` - Speed table row.
/// * `ch_status` - Current animation status.
/// * `d` - Base status value for this direction.
/// * `s` - Number of frames in one movement cycle.
/// * `update` - `false` when the character is stunned (hard step only).
/// * `ctick` - Current animation tick.
///
/// # Returns
/// * A sub-pixel offset in the range `[0, 32 * SUBPIXEL_UNIT)`.
fn speedstep(
    ch_speed: u8,
    ch_status: u8,
    d: i32,
    s: i32,
    update: bool,
    ctick: usize,
    start_lead_ticks: i32,
) -> i32 {
    let speed = (ch_speed as usize).min(MAX_SPEEDTAB_SPEED_INDEX);
    let max_tick = (SPEEDTAB[0].len() - 1) as i32;

    let hard_step = i32::from(ch_status) - d;

    if !update {
        return 32 * SUBPIXEL_UNIT * hard_step / s;
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

    let adjusted_start = (total_step_start - start_lead_ticks).max(0);
    let adjusted_total = (total_step + 1 - start_lead_ticks).max(1);
    32 * SUBPIXEL_UNIT * adjusted_start / adjusted_total
}

/// Counts cadence ticks inferred before the current animation status.
///
/// # Arguments
/// * `ch_speed` - Speed table row.
/// * `ctick` - Current animation tick.
///
/// # Returns
/// * Number of non-advancing ticks since the preceding speed-table event.
fn movement_start_lead_ticks(ch_speed: u8, ctick: usize) -> u8 {
    let speed = (ch_speed as usize).min(MAX_SPEEDTAB_SPEED_INDEX);
    let max_tick = SPEEDTAB[0].len();
    let mut lead_ticks = 0u8;
    let mut tick = ctick;

    loop {
        tick = if tick == 0 { max_tick - 1 } else { tick - 1 };
        if SPEEDTAB[speed][tick] != 0 {
            return lead_ticks;
        }
        lead_ticks = lead_ticks.saturating_add(1);
    }
}

/// Computes a movement offset while removing cadence time that predates the action.
///
/// # Arguments
/// * `tile` - Character tile carrying the movement-onset state.
/// * `d` - Base status value for this direction.
/// * `s` - Number of frames in one movement cycle.
/// * `update` - `false` when the character is stunned.
/// * `ctick` - Current animation tick.
///
/// # Returns
/// * The corrected movement offset in sub-pixel units.
fn movement_speedstep(tile: &mut CMapTile, d: i32, s: i32, update: bool, ctick: usize) -> i32 {
    if tile.movement_start_pending && update {
        let hard_step = i32::from(tile.ch_status) - d;

        if hard_step == 0 {
            tile.movement_start_lead_ticks = movement_start_lead_ticks(tile.ch_speed, ctick);
        }
        tile.movement_start_pending = false;
    }

    speedstep(
        tile.ch_speed,
        tile.ch_status,
        d,
        s,
        update,
        ctick,
        i32::from(tile.movement_start_lead_ticks),
    )
}

/// Returns a small frame offset for the idle animation of specific sprites.
///
/// # Arguments
/// * `idle_ani` - The current idle animation counter (0–7).
/// * `sprite` - The base character sprite ID.
///
/// # Returns
/// * `idle_ani` for sprite 22480, `0` for all others.
#[inline]
fn do_idle(idle_ani: i32, sprite: u16) -> i32 {
    if sprite == 22480 { idle_ani } else { 0 }
}

/// Advances an item's animation state machine and returns the display sprite.
///
/// # Arguments
/// * `it_sprite` - Base item sprite ID.
/// * `it_status` - Current animation status (mutated to advance the state).
/// * `ctick` - Current animation tick.
/// * `ticker` - Global frame counter (used for continuous-scroll items).
///
/// # Returns
/// * The sprite ID to render this frame.
fn eng_item(it_sprite: u16, it_status: &mut u8, ctick: usize, ticker: u32) -> i32 {
    let base = i32::from(it_sprite);
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

/// Advances a character's animation state machine and returns the display
/// sprite, also computing sub-tile offsets (`obj_xoff_sub`, `obj_yoff_sub`) for
/// smooth movement interpolation.
///
/// # Arguments
/// * `tile` - The map tile containing the character (mutated in place).
/// * `ctick` - Current animation tick.
///
/// # Returns
/// * The sprite ID to render this frame.
fn eng_char(tile: &mut CMapTile, ctick: usize) -> i32 {
    let update = (tile.flags & STUNNED) == 0;

    let ch_status = tile.ch_status;
    let base = i32::from(tile.ch_sprite);

    match ch_status {
        0..=7 => {
            tile.obj_xoff_sub = 0;
            tile.obj_yoff_sub = 0;
            if ch_status == 0 || (speedo(tile.ch_speed, ctick) && update) {
                tile.idle_ani += 1;
                if tile.idle_ani > 7 {
                    tile.idle_ani = 0;
                }
            }
            base + i32::from(ch_status) * 8 + do_idle(tile.idle_ani, tile.ch_sprite)
        }

        16..=23 => {
            let step = movement_speedstep(tile, 16, 8, update, ctick);
            tile.obj_xoff_sub = -step / 2;
            tile.obj_yoff_sub = step / 4;
            let tmp = base + (i32::from(tile.ch_status) - 16) + 64;
            if speedo(tile.ch_speed, ctick) && update {
                tile.ch_status = if tile.ch_status == 23 {
                    tile.movement_start_lead_ticks = 0;
                    16
                } else {
                    tile.ch_status + 1
                };
            }
            tmp
        }
        24..=31 => {
            let step = movement_speedstep(tile, 24, 8, update, ctick);
            tile.obj_xoff_sub = step / 2;
            tile.obj_yoff_sub = -step / 4;
            let tmp = base + (i32::from(tile.ch_status) - 24) + 72;
            if speedo(tile.ch_speed, ctick) && update {
                tile.ch_status = if tile.ch_status == 31 {
                    tile.movement_start_lead_ticks = 0;
                    24
                } else {
                    tile.ch_status + 1
                };
            }
            tmp
        }
        32..=39 => {
            let step = movement_speedstep(tile, 32, 8, update, ctick);
            tile.obj_xoff_sub = -step / 2;
            tile.obj_yoff_sub = -step / 4;
            let tmp = base + (i32::from(tile.ch_status) - 32) + 80;
            if speedo(tile.ch_speed, ctick) && update {
                tile.ch_status = if tile.ch_status == 39 {
                    tile.movement_start_lead_ticks = 0;
                    32
                } else {
                    tile.ch_status + 1
                };
            }
            tmp
        }
        40..=47 => {
            let step = movement_speedstep(tile, 40, 8, update, ctick);
            tile.obj_xoff_sub = step / 2;
            tile.obj_yoff_sub = step / 4;
            let tmp = base + (i32::from(tile.ch_status) - 40) + 88;
            if speedo(tile.ch_speed, ctick) && update {
                tile.ch_status = if tile.ch_status == 47 {
                    tile.movement_start_lead_ticks = 0;
                    40
                } else {
                    tile.ch_status + 1
                };
            }
            tmp
        }

        48..=59 => {
            tile.obj_xoff_sub = -movement_speedstep(tile, 48, 12, update, ctick);
            tile.obj_yoff_sub = 0;
            let tmp = base + ((i32::from(tile.ch_status) - 48) * 8 / 12) + 96;
            if speedo(tile.ch_speed, ctick) && update {
                tile.ch_status = if tile.ch_status == 59 {
                    tile.movement_start_lead_ticks = 0;
                    48
                } else {
                    tile.ch_status + 1
                };
            }
            tmp
        }
        60..=71 => {
            tile.obj_xoff_sub = 0;
            tile.obj_yoff_sub = -movement_speedstep(tile, 60, 12, update, ctick) / 2;
            let tmp = base + ((i32::from(tile.ch_status) - 60) * 8 / 12) + 104;
            if speedo(tile.ch_speed, ctick) && update {
                tile.ch_status = if tile.ch_status == 71 {
                    tile.movement_start_lead_ticks = 0;
                    60
                } else {
                    tile.ch_status + 1
                };
            }
            tmp
        }
        72..=83 => {
            tile.obj_xoff_sub = 0;
            tile.obj_yoff_sub = movement_speedstep(tile, 72, 12, update, ctick) / 2;
            let tmp = base + ((i32::from(tile.ch_status) - 72) * 8 / 12) + 112;
            if speedo(tile.ch_speed, ctick) && update {
                tile.ch_status = if tile.ch_status == 83 {
                    tile.movement_start_lead_ticks = 0;
                    72
                } else {
                    tile.ch_status + 1
                };
            }
            tmp
        }
        84..=95 => {
            tile.obj_xoff_sub = movement_speedstep(tile, 84, 12, update, ctick);
            tile.obj_yoff_sub = 0;
            let tmp = base + ((i32::from(tile.ch_status) - 84) * 8 / 12) + 120;
            if speedo(tile.ch_speed, ctick) && update {
                tile.ch_status = if tile.ch_status == 95 {
                    tile.movement_start_lead_ticks = 0;
                    84
                } else {
                    tile.ch_status + 1
                };
            }
            tmp
        }

        96..=191 => {
            tile.obj_xoff_sub = 0;
            tile.obj_yoff_sub = 0;

            let status = i32::from(tile.ch_status);
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
            let is_misc_action = (160..=191).contains(&tile.ch_status);
            let stat_add = if is_misc_action {
                STATTAB[stat_off] << 5
            } else {
                0
            };

            let frame = status - start;
            let tmp = base + frame + base_add + stat_add;

            // Misc/attack states (160..=191) pace off the independently-derived
            // attack/action speed; turn states (96..=159) keep movement speed.
            let advance_speed = if is_misc_action {
                tile.ch_aspeed
            } else {
                tile.ch_speed
            };
            if speedo(advance_speed, ctick) && update {
                let max = if is_misc_action { start + 7 } else { start + 3 };
                if i32::from(tile.ch_status) >= max {
                    tile.ch_status = wrap;
                } else {
                    tile.ch_status = tile.ch_status.saturating_add(1);
                }
            }

            tmp
        }

        _ => {
            tile.obj_xoff_sub = 0;
            tile.obj_yoff_sub = 0;
            base
        }
    }
}

/// Runs one engine tick over the entire visible map, updating animation
/// frames and sub-tile offsets for every tile's background, item, and
/// character layers.
///
/// # Arguments
/// * `player_state` - The player state whose map will be updated.
/// * `ticker` - Global frame counter.
/// * `ctick` - Current animation tick.
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
        tile.obj_xoff_sub = 0;
        tile.obj_yoff_sub = 0;
        tile.ovl_xoff = 0;
        tile.ovl_yoff = 0;
    }

    for i in 0..len {
        let Some(tile) = map.tile_at_index_mut(i) else {
            continue;
        };

        tile.back = i32::from(tile.ba_sprite);

        if tile.it_sprite != 0 {
            tile.obj1 = eng_item(tile.it_sprite, &mut tile.it_status, ctick, ticker);
        }

        if tile.ch_sprite != 0 {
            tile.obj2 = eng_char(tile, ctick);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::map::SUBPIXEL_UNIT;

    /// Builds a tile holding a character mid-walk (status range 48..=59).
    fn walking_tile(ch_status: u8, ch_speed: u8) -> CMapTile {
        CMapTile {
            ch_sprite: 1000,
            ch_status,
            ch_speed,
            ..CMapTile::default()
        }
    }

    #[test]
    fn same_direction_walkers_hold_a_stable_pixel_gap() {
        // Regression: rounding each character's movement offset to whole pixels
        // on its own made the gap between the camera-anchored player and
        // another walker alternate between two pixel positions every frame,
        // which showed up as shimmering nameplates on nearby characters.
        let mut own = walking_tile(48, 0);
        let mut other = walking_tile(49, 0);

        let mut gaps = Vec::new();
        for ctick in 0..18 {
            eng_char(&mut own, ctick);
            eng_char(&mut other, ctick);
            let cam_xoff_sub = -own.obj_xoff_sub;
            gaps.push((cam_xoff_sub + other.obj_xoff_sub).div_euclid(SUBPIXEL_UNIT));
        }

        assert!(
            gaps.windows(2).all(|w| w[0] == w[1]),
            "screen gap jittered between frames: {gaps:?}"
        );
        assert_ne!(
            gaps[0], 0,
            "expected a non-zero gap between the two walkers"
        );
    }

    #[test]
    fn smoothed_speed_rows_do_not_toggle_the_walker_gap() {
        for speed in [1, 2, 7] {
            let mut own = walking_tile(48, speed);
            let mut other = walking_tile(49, speed);
            let mut gaps = Vec::new();

            for ctick in 0..SPEEDTAB[0].len() {
                eng_char(&mut own, ctick);
                eng_char(&mut other, ctick);
                gaps.push((-own.obj_xoff_sub + other.obj_xoff_sub).div_euclid(SUBPIXEL_UNIT));
            }

            assert!(
                gaps.windows(3)
                    .all(|window| window[0] != window[2] || window[0] == window[1]),
                "speed row {speed} toggled the screen gap between pixels: {gaps:?}"
            );
        }
    }

    #[test]
    fn walk_offset_stays_within_one_tile() {
        let mut tile = walking_tile(48, 0);
        for ctick in 0..24 {
            eng_char(&mut tile, ctick);
            assert!(
                tile.obj_xoff_sub <= 0 && tile.obj_xoff_sub > -32 * SUBPIXEL_UNIT,
                "offset {} left the tile at ctick {ctick}",
                tile.obj_xoff_sub
            );
            assert_eq!(tile.obj_yoff_sub, 0);
        }
    }

    #[test]
    fn stunned_character_uses_the_hard_step() {
        assert_eq!(
            speedstep(0, 54, 48, 12, false, 7, 0),
            32 * SUBPIXEL_UNIT * 6 / 12
        );
    }

    #[test]
    fn idle_character_has_no_movement_offset() {
        let mut tile = walking_tile(0, 0);
        eng_char(&mut tile, 0);
        assert_eq!(tile.obj_xoff_sub, 0);
        assert_eq!(tile.obj_yoff_sub, 0);
    }

    #[test]
    fn newly_started_movement_does_not_inherit_pre_action_progress() {
        let ctick = (0..SPEEDTAB[0].len())
            .find(|&tick| speedstep(1, 48, 48, 12, true, tick, 0) > 0)
            .expect("speed row should contain a phase with inferred progress");
        let mut tile = walking_tile(48, 1);
        tile.movement_start_pending = true;

        eng_char(&mut tile, ctick);

        assert_eq!(tile.obj_xoff_sub, 0);
        assert!(!tile.movement_start_pending);
        assert!(tile.movement_start_lead_ticks > 0);
    }

    #[test]
    fn misc_action_status_advances_using_ch_aspeed_not_ch_speed() {
        // ch_speed is pinned to the slowest row while ch_aspeed is the
        // fastest row, proving misc/attack states (160..=191) gate on the
        // independent attack/action speed field, not movement speed.
        let mut tile = CMapTile {
            ch_sprite: 1000,
            ch_status: 160,
            ch_speed: MAX_SPEEDTAB_SPEED_INDEX as u8,
            ch_aspeed: 0,
            ..CMapTile::default()
        };

        eng_char(&mut tile, 0);

        assert_eq!(
            tile.ch_status, 161,
            "misc/attack status must advance using ch_aspeed"
        );
    }

    #[test]
    fn turn_status_advances_using_ch_speed_not_ch_aspeed() {
        // ch_aspeed is pinned to the slowest row while ch_speed is the
        // fastest row, proving turn states (96..=159) still gate on
        // movement speed.
        let mut tile = CMapTile {
            ch_sprite: 1000,
            ch_status: 96,
            ch_speed: 0,
            ch_aspeed: MAX_SPEEDTAB_SPEED_INDEX as u8,
            ..CMapTile::default()
        };

        eng_char(&mut tile, 0);

        assert_eq!(
            tile.ch_status, 97,
            "turn status must advance using ch_speed"
        );
    }
}
