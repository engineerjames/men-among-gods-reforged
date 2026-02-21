use std::cmp::Ordering;

use mag_core::constants::{INVIS, TILEX, TILEY, XPOS, YPOS};
use mag_core::types::skilltab::{get_skill_name, get_skill_sortkey, MAX_SKILLS};

use crate::player_state::PlayerState;

use super::{GameScene, MAP_X_SHIFT};

impl GameScene {
    /// Returns `true` if the tile at `(x, y)` should be hidden in the "hide"
    /// display mode. Ported from the C `autohide()` function.
    pub(super) fn autohide(x: usize, y: usize) -> bool {
        !(x >= (TILEX / 2) || y <= (TILEX / 2))
    }

    /// Returns `true` if tile `(x, y)` is the cell directly in front of the
    /// player given facing direction `dir` (1=E, 2=W, 3=N, 4=S).
    pub(super) fn facing(x: usize, y: usize, dir: i32) -> bool {
        (dir == 1 && x == TILEX / 2 + 1 && y == TILEY / 2)
            || (dir == 2 && x == TILEX / 2 - 1 && y == TILEY / 2)
            || (dir == 4 && x == TILEX / 2 && y == TILEY / 2 + 1)
            || (dir == 3 && x == TILEX / 2 && y == TILEY / 2 - 1)
    }

    /// Computes the screen-space origin of the ground diamond for a given tile,
    /// accounting for the camera offset.
    ///
    /// # Returns
    /// `(cx, cy)` in logical screen coordinates.
    pub(super) fn tile_ground_diamond_origin(
        tile_x: usize,
        tile_y: usize,
        cam_xoff: i32,
        cam_yoff: i32,
    ) -> (i32, i32) {
        let xpos = (tile_x as i32) * 32;
        let ypos = (tile_y as i32) * 32;
        let cx = xpos / 2 + ypos / 2 + 32 + XPOS + MAP_X_SHIFT + cam_xoff;
        let cy = xpos / 4 - ypos / 4 + YPOS - 16 + cam_yoff;
        (cx, cy)
    }

    /// Returns the camera pixel offsets derived from the center tile's
    /// `obj_xoff` / `obj_yoff` (smooth scrolling between tiles).
    pub(super) fn camera_offsets(ps: &PlayerState) -> (i32, i32) {
        let map = ps.map();
        if let Some(center) = map.tile_at_xy(TILEX / 2, TILEY / 2) {
            (-center.obj_xoff, -center.obj_yoff)
        } else {
            (0, 0)
        }
    }

    /// Converts a screen pixel coordinate to the map tile `(x, y)` it lies on,
    /// using the isometric diamond geometry.
    ///
    /// # Returns
    /// `Some((mx, my))` if a valid tile is found, `None` otherwise.
    pub(super) fn screen_to_map_tile(
        screen_x: i32,
        screen_y: i32,
        cam_xoff: i32,
        cam_yoff: i32,
    ) -> Option<(usize, usize)> {
        let mut best: Option<(usize, usize, i32)> = None;

        for my in 0..TILEY {
            for mx in 0..TILEX {
                let (cx, cy_top) = Self::tile_ground_diamond_origin(mx, my, cam_xoff, cam_yoff);
                let dx = (screen_x - cx).abs();
                let dy = (screen_y - (cy_top + 8)).abs();

                // Inside 32x16 isometric floor diamond:
                // |dx|/16 + |dy|/8 <= 1  =>  dx*8 + dy*16 <= 128
                let metric = dx * 8 + dy * 16;
                if metric <= 128 {
                    match best {
                        Some((_, _, cur_metric)) if metric >= cur_metric => {}
                        _ => best = Some((mx, my, metric)),
                    }
                }
            }
        }

        best.map(|(mx, my, _)| (mx, my))
    }

    /// Returns `true` if the screen coordinate falls within the map interaction
    /// area (the isometric viewport, excluding UI panels).
    pub(super) fn cursor_in_map_interaction_area(screen_x: i32, screen_y: i32) -> bool {
        // Matches original inter.c::mouse_mapbox coordinate transform and bounds check.
        let x = screen_x + 176 - 16;
        let y = screen_y + 8;

        let mx = 2 * y + x - (YPOS * 2) - XPOS + (((TILEX as i32) - 34) / 2 * 32);
        let my = x - 2 * y + (YPOS * 2) - XPOS + (((TILEX as i32) - 34) / 2 * 32);

        !(mx < 3 * 32 + 12
            || mx > ((TILEX as i32) - 7) * 32 + 20
            || my < 7 * 32 + 12
            || my > ((TILEY as i32) - 3) * 32 + 20)
    }

    /// Maps a total experience point value to a rank index (0–23).
    pub(super) fn points_to_rank_index(points: u32) -> usize {
        let v = points as i64;
        if v < 50 {
            0
        } else if v < 850 {
            1
        } else if v < 4900 {
            2
        } else if v < 17700 {
            3
        } else if v < 48950 {
            4
        } else if v < 113750 {
            5
        } else if v < 233800 {
            6
        } else if v < 438600 {
            7
        } else if v < 766650 {
            8
        } else if v < 1266650 {
            9
        } else if v < 1998700 {
            10
        } else if v < 3035500 {
            11
        } else if v < 4463550 {
            12
        } else if v < 6384350 {
            13
        } else if v < 8915600 {
            14
        } else if v < 12192400 {
            15
        } else if v < 16368450 {
            16
        } else if v < 21617250 {
            17
        } else if v < 28133300 {
            18
        } else if v < 36133300 {
            19
        } else if v < 49014500 {
            20
        } else if v < 63000600 {
            21
        } else if v < 80977100 {
            22
        } else {
            23
        }
    }

    /// Scans visible map tiles for a character whose name is not yet known,
    /// returning its `ch_nr` for an auto-look request.
    pub(super) fn find_unknown_look_target(ps: &PlayerState) -> Option<u32> {
        let pdata = ps.player_data();
        if pdata.show_names == 0 && pdata.show_proz == 0 {
            return None;
        }

        let map = ps.map();
        for idx in 0..map.len() {
            let Some(tile) = map.tile_at_index(idx) else {
                continue;
            };

            if tile.ch_nr == 0 {
                continue;
            }

            if (tile.flags & INVIS) != 0 {
                continue;
            }

            // Mirror C lookup() behavior: unknown if we don't have a known name for this nr/id.
            if ps.lookup_name(tile.ch_nr, tile.ch_id).is_none() {
                return Some(tile.ch_nr as u32);
            }
        }

        None
    }

    /// Finds the nearest tile within a ±2-tile radius of `(mx, my)` that has
    /// the given flag bit set (e.g. `ISCHAR`, `ISITEM`).
    ///
    /// # Returns
    /// `Some((x, y))` of the nearest matching tile, or `None` if none found.
    pub(super) fn nearest_tile_with_flag(
        ps: &PlayerState,
        mx: usize,
        my: usize,
        flag: u32,
    ) -> Option<(usize, usize)> {
        let mut best: Option<(usize, usize, i32)> = None;
        for dy in -2..=2 {
            for dx in -2..=2 {
                let nx = mx as i32 + dx;
                let ny = my as i32 + dy;
                if nx < 0 || ny < 0 || nx >= TILEX as i32 || ny >= TILEY as i32 {
                    continue;
                }
                let ux = nx as usize;
                let uy = ny as usize;
                let Some(tile) = ps.map().tile_at_xy(ux, uy) else {
                    continue;
                };
                if (tile.flags & flag) == 0 {
                    continue;
                }
                let dist = dx * dx + dy * dy;
                match best {
                    Some((_, _, cur_dist)) if dist >= cur_dist => {}
                    _ => best = Some((ux, uy, dist)),
                }
            }
        }
        best.map(|(x, y, _)| (x, y))
    }

    /// Computes the experience-point cost to raise attribute `n` by one point
    /// from base value `v`.
    pub(super) fn attrib_needed(ci: &mag_core::types::ClientPlayer, n: usize, v: i32) -> i32 {
        const HIGH_VAL: i32 = i32::MAX;
        let max_v = ci.attrib[n][2] as i32;
        if v >= max_v {
            return HIGH_VAL;
        }
        let diff = ci.attrib[n][3] as i32;
        let v64 = v as i64;
        ((v64 * v64 * v64) * (diff as i64) / 20).clamp(0, i32::MAX as i64) as i32
    }

    /// Computes the experience-point cost to raise skill `n` by one point
    /// from base value `v`.
    pub(super) fn skill_needed(ci: &mag_core::types::ClientPlayer, n: usize, v: i32) -> i32 {
        const HIGH_VAL: i32 = i32::MAX;
        let max_v = ci.skill[n][2] as i32;
        if v >= max_v {
            return HIGH_VAL;
        }
        let diff = ci.skill[n][3] as i32;
        let v64 = v as i64;
        let cubic = ((v64 * v64 * v64) * (diff as i64) / 40).clamp(0, i32::MAX as i64) as i32;
        v.max(cubic)
    }

    /// Computes the experience-point cost to raise HP by one point from base value `v`.
    pub(super) fn hp_needed(ci: &mag_core::types::ClientPlayer, v: i32) -> i32 {
        const HIGH_VAL: i32 = i32::MAX;
        if v >= ci.hp[2] as i32 {
            return HIGH_VAL;
        }
        (v as i64 * ci.hp[3] as i64).clamp(0, i32::MAX as i64) as i32
    }

    /// Computes the experience-point cost to raise Endurance by one point from base value `v`.
    pub(super) fn end_needed(ci: &mag_core::types::ClientPlayer, v: i32) -> i32 {
        const HIGH_VAL: i32 = i32::MAX;
        if v >= ci.end[2] as i32 {
            return HIGH_VAL;
        }
        (v as i64 * ci.end[3] as i64 / 2).clamp(0, i32::MAX as i64) as i32
    }

    /// Computes the experience-point cost to raise Mana by one point from base value `v`.
    pub(super) fn mana_needed(ci: &mag_core::types::ClientPlayer, v: i32) -> i32 {
        const HIGH_VAL: i32 = i32::MAX;
        if v >= ci.mana[2] as i32 {
            return HIGH_VAL;
        }
        (v as i64 * ci.mana[3] as i64).clamp(0, i32::MAX as i64) as i32
    }

    /// Returns all skill indices sorted by: learned status, sort-key character,
    /// then name — with unused/empty skills pushed to the end.
    pub(super) fn sorted_skills(ci: &mag_core::types::ClientPlayer) -> Vec<usize> {
        let mut out: Vec<usize> = (0..MAX_SKILLS).collect();
        out.sort_by(|&a, &b| {
            let a_unused = get_skill_sortkey(a) == 'Z' || get_skill_name(a).is_empty();
            let b_unused = get_skill_sortkey(b) == 'Z' || get_skill_name(b).is_empty();
            if a_unused != b_unused {
                return if a_unused {
                    Ordering::Greater
                } else {
                    Ordering::Less
                };
            }

            let a_learned = ci.skill[a][0] != 0;
            let b_learned = ci.skill[b][0] != 0;
            if a_learned != b_learned {
                return if a_learned {
                    Ordering::Less
                } else {
                    Ordering::Greater
                };
            }

            let a_key = get_skill_sortkey(a);
            let b_key = get_skill_sortkey(b);
            if a_key != b_key {
                return a_key.cmp(&b_key);
            }

            get_skill_name(a).cmp(get_skill_name(b))
        });
        out
    }
}
