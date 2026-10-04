use core::constants::{CharacterFlags, ItemFlags};
use core::skills;
use core::types::{Character, SeeMap};
use std::cmp;

use crate::game_state::GameState;
use crate::player::update::{self, Deferred, WorldView};

impl GameState {
    /// Run `f` against the shared visibility code with character `cn`'s
    /// see-map, folding its hit/miss counters back into `self`.
    ///
    /// # Panics
    ///
    /// * Panics if `cn` is out of range for `see_map`.
    fn with_char_see<R>(
        &mut self,
        cn: usize,
        f: impl FnOnce(&WorldView, &mut SeeMap, &mut Deferred) -> R,
    ) -> R {
        let mut counters = Deferred::default();
        let result = {
            let (world, _, see_map) = update::split_world(self);
            f(&world, &mut see_map[cn], &mut counters)
        };

        if counters.see_hit + counters.see_miss > 0 {
            // Legacy per-character lookups invalidated the global/path cache.
            self.vis_is_global = false;
        }
        self.see_hit += counters.see_hit;
        self.see_miss += counters.see_miss;
        result
    }

    /// Borrow the read-only world plus the global vis buffer and origin.
    fn global_vis_parts(
        &mut self,
    ) -> (
        WorldView<'_>,
        &mut [i8; core::constants::VISI_BUFFER_LEN],
        &mut i32,
        &mut i32,
    ) {
        let GameState {
            map,
            characters,
            items,
            globals,
            _visi,
            ox,
            oy,
            ..
        } = self;
        let world = WorldView {
            map: map.as_slice(),
            characters: characters.as_slice(),
            items: items.as_slice(),
            globals,
        };
        (world, _visi, ox, oy)
    }

    /// Port of `do_add_light(x, y, strength)` from the original `helper.cpp`.
    ///
    /// Adds light originating at `(x_center, y_center)` and spreads it to
    /// nearby tiles according to line-of-sight. Negative `strength` values
    /// remove light. Uses `can_see` to attenuate contribution based on
    /// obstructions.
    ///
    /// # Arguments
    /// * `x_center, y_center` - Source coordinates for the light
    /// * `strength` - Light strength (negative to subtract)
    pub(crate) fn do_add_light(&mut self, x_center: i32, y_center: i32, strength: i32) {
        // First add light to the center
        let center_map_index =
            (y_center as usize) * core::constants::SERVER_MAPX as usize + (x_center as usize);

        self.map[center_map_index].add_light(strength);

        let mut strength = strength;
        let flag = if strength < 0 {
            strength = -strength;
            1
        } else {
            0
        };

        let xs = cmp::max(0, x_center - core::constants::LIGHTDIST);
        let ys = cmp::max(0, y_center - core::constants::LIGHTDIST);
        let xe = cmp::min(
            core::constants::SERVER_MAPX - 1,
            x_center + 1 + core::constants::LIGHTDIST,
        );
        let ye = cmp::min(
            core::constants::SERVER_MAPY - 1,
            y_center + 1 + core::constants::LIGHTDIST,
        );

        for y in ys..ye {
            for x in xs..xe {
                if x == x_center && y == y_center {
                    continue;
                }

                let dx = (x - x_center).abs();
                let dy = (y - y_center).abs();

                if (dx * dx + dy * dy)
                    > (core::constants::LIGHTDIST * core::constants::LIGHTDIST + 1)
                {
                    continue;
                }

                let v = self.can_see(None, x_center, y_center, x, y, core::constants::LIGHTDIST);

                if v != 0 {
                    let d = strength / (v * ((x_center - x).abs() + (y_center - y).abs()));
                    let map_index =
                        (y as usize) * core::constants::SERVER_MAPX as usize + (x as usize);

                    if flag == 1 {
                        self.map[map_index].add_light(-d);
                    } else {
                        self.map[map_index].add_light(d);
                    }
                }
            }
        }
    }

    /// Port of `compute_dlight(xc, yc)` from the original helper code.
    ///
    /// For indoor tiles, computes a daylight contribution derived from nearby
    /// outdoor tiles. Writes the computed `dlight` value into the map tile
    /// at `(xc, yc)`.
    ///
    /// # Arguments
    /// * `xc, yc` - Coordinates of the indoor tile to compute
    pub(crate) fn compute_dlight(&mut self, xc: i32, yc: i32) {
        let xs = cmp::max(0, xc - core::constants::LIGHTDIST);
        let ys = cmp::max(0, yc - core::constants::LIGHTDIST);
        let xe = cmp::min(
            core::constants::SERVER_MAPX - 1,
            xc + 1 + core::constants::LIGHTDIST,
        );
        let ye = cmp::min(
            core::constants::SERVER_MAPY - 1,
            yc + 1 + core::constants::LIGHTDIST,
        );

        let mut best: i32 = 0;

        for y in ys..ye {
            for x in xs..xe {
                let dx = xc - x;
                let dy = yc - y;
                if dx * dx + dy * dy > (core::constants::LIGHTDIST * core::constants::LIGHTDIST + 1)
                {
                    continue;
                }

                let m = (x + y * core::constants::SERVER_MAPX) as usize;

                let should_continue =
                    self.map[m].flags & u64::from(core::constants::MF_INDOORS) != 0;

                if should_continue {
                    continue;
                }

                let v = self.can_see(None, xc, yc, x, y, core::constants::LIGHTDIST);
                if v == 0 {
                    continue;
                }

                let denom = v * (dx.abs() + dy.abs());
                if denom <= 0 {
                    continue;
                }

                let d = 256 / denom;
                if d > best {
                    best = d;
                }
            }
        }

        if best > 256 {
            best = 256;
        }

        let center_index = (xc + yc * core::constants::SERVER_MAPX) as usize;

        if center_index < self.map.len() {
            self.map[center_index].dlight = best as u16;
        }
    }

    /// Port of `add_lights(x, y)` from the original `helper.cpp`.
    ///
    /// Scans a local neighborhood around `(x, y)` and applies light sources
    /// contributed by items and characters. For indoor tiles it also invokes
    /// `compute_dlight` to derive daylight contribution.
    ///
    /// # Arguments
    /// * `x, y` - Center coordinates to scan for light sources
    pub(crate) fn add_lights(&mut self, x: i32, y: i32) {
        let x0 = x;
        let y0 = y;

        let xs = cmp::max(1, x0 - core::constants::LIGHTDIST);
        let ys = cmp::max(1, y0 - core::constants::LIGHTDIST);
        let xe = cmp::min(
            core::constants::SERVER_MAPX - 2,
            x0 + 1 + core::constants::LIGHTDIST,
        );
        let ye = cmp::min(
            core::constants::SERVER_MAPY - 2,
            y0 + 1 + core::constants::LIGHTDIST,
        );

        for yy in ys..ye {
            for xx in xs..xe {
                let m = (xx + yy * core::constants::SERVER_MAPX) as usize;

                let item_idx = self.map[m].it as usize;
                let light_value_from_item = if item_idx != 0 && item_idx < self.items.len() {
                    let it = &mut self.items[item_idx];
                    if it.active != 0 {
                        it.light[1]
                    } else {
                        it.light[0]
                    }
                } else {
                    0
                };

                if light_value_from_item != 0 {
                    self.do_add_light(xx, yy, i32::from(light_value_from_item));
                }

                let cn = self.map[m].ch as usize;

                let light_value_from_character = if !Character::is_sane_character(cn) {
                    0
                } else {
                    self.characters[cn].light
                };

                if light_value_from_character != 0 {
                    self.do_add_light(xx, yy, i32::from(light_value_from_character));
                }

                let is_indoors = self.map[m].flags & u64::from(core::constants::MF_INDOORS) != 0;
                if is_indoors {
                    self.compute_dlight(xx, yy);
                }
            }
        }
    }

    /// Port of `can_see(fx,fy,tx,ty,max_distance)` from original code.
    ///
    /// Checks line-of-sight from `(fx,fy)` to `(tx,ty)` and returns a
    /// visibility metric: `1` indicates perfect/very close visibility, larger
    /// values indicate worse visibility, and `0` means not visible. When
    /// `cn` is provided the function may reuse a cached see-map and
    /// update the per-character visibility cache.
    ///
    /// # Arguments
    /// * `cn` - Optional character id whose see cache to use/update
    /// * `fx, fy` - Origin coordinates
    /// * `tx, ty` - Target coordinates to check
    /// * `max_distance` - Maximum radius to compute
    pub(crate) fn can_see(
        &mut self,
        cn: Option<usize>,
        fx: i32,
        fy: i32,
        tx: i32,
        ty: i32,
        max_distance: i32,
    ) -> i32 {
        if let Some(cn) = cn {
            return self.with_char_see(cn, |world, see, counters| {
                update::can_see(world, see, cn, fx, fy, tx, ty, max_distance, counters)
            });
        }

        // Global visibility buffer (used by lighting and non-character LOS checks)
        if !self.vis_is_global {
            self.vis_is_global = true;
            self.ox = 0;
            self.oy = 0;
        }

        if self.ox != fx || self.oy != fy {
            self.can_map_see(fx, fy, max_distance);
        }

        self.check_vis(tx, ty)
    }

    /// Port of `can_map_go(fx,fy,max_distance)` from original helper code.
    ///
    /// Builds a visibility map used for pathfinding from origin `(fx,fy)`,
    /// filling the internal `_visi` array with reachability values up to the
    /// supplied `max_distance`.
    ///
    /// # Arguments
    /// * `fx, fy` - Origin coordinates
    /// * `max_distance` - Maximum radius to build
    pub(crate) fn can_map_go(&mut self, fx: i32, fy: i32, max_distance: i32) {
        // `can_go` always uses the global buffer.
        self.vis_is_global = true;
        self._visi.fill(0);

        self.ox = fx;
        self.oy = fy;

        self.add_vis(fx, fy, 1);

        for dist in 1..(max_distance + 1) {
            let xc = fx;
            let yc = fy;

            // Top and bottom horizontal lines
            for x in (xc - dist)..=(xc + dist) {
                let y = yc - dist;
                if self.close_vis_go(x, y, dist as i8) {
                    self.add_vis(x, y, dist + 1);
                }

                let y = yc + dist;
                if self.close_vis_go(x, y, dist as i8) {
                    self.add_vis(x, y, dist + 1);
                }
            }

            // Left and right vertical lines (excluding corners already done)
            for y in (yc - dist + 1)..=(yc + dist - 1) {
                let x = xc - dist;
                if self.close_vis_go(x, y, dist as i8) {
                    self.add_vis(x, y, dist + 1);
                }

                let x = xc + dist;
                if self.close_vis_go(x, y, dist as i8) {
                    self.add_vis(x, y, dist + 1);
                }
            }
        }
    }

    /// Port of `can_map_see(fx,fy,max_distance)` from original helper code.
    ///
    /// Builds a line-of-sight visibility map from origin `(fx,fy)` and fills
    /// the internal `_visi` buffer with visibility strength values used by
    /// `can_see` and related checks.
    ///
    /// # Arguments
    /// * `fx, fy` - Origin coordinates
    /// * `max_distance` - Maximum radius to compute
    fn can_map_see(&mut self, fx: i32, fy: i32, max_distance: i32) {
        let (world, vis, ox, oy) = self.global_vis_parts();
        update::build_vis(&world, vis, fx, fy, false, max_distance);
        *ox = fx;
        *oy = fy;
    }

    /// Port of `can_go(fx,fy,target_x,target_y)` from original helper code.
    ///
    /// Determines whether a valid path exists from `(fx,fy)` to
    /// `(target_x,target_y)` using the internal visibility/path map. Returns
    /// `true` when reachable, `false` otherwise.
    ///
    /// # Arguments
    /// * `fx, fy` - Start coordinates
    /// * `target_x, target_y` - Destination coordinates
    pub(crate) fn can_go(&mut self, fx: i32, fy: i32, target_x: i32, target_y: i32) -> i32 {
        if !self.vis_is_global {
            self.vis_is_global = true;
            self.ox = 0;
            self.oy = 0;
        }

        if self.ox != fx || self.oy != fy {
            self.can_map_go(fx, fy, 15);
        }

        self.check_vis(target_x, target_y)
    }

    /// Port of `check_dlight(x,y)` from original helper code.
    ///
    /// Returns the computed daylight value at tile `(x,y)`, taking into
    /// account whether the tile is indoor or outdoor.
    ///
    /// # Arguments
    /// * `x, y` - Tile coordinates
    pub(crate) fn check_dlight(&mut self, x: usize, y: usize) -> i32 {
        let map_index = x + y * core::constants::SERVER_MAPX as usize;

        self.check_dlightm(map_index)
    }

    /// Port of `check_dlightm(map_index)` from original helper code.
    ///
    /// Returns daylight for a tile given its flat map index, considering the
    /// global daylight value and the tile's indoor multiplier.
    ///
    /// # Arguments
    /// * `map_index` - Linear map index
    pub(crate) fn check_dlightm(&mut self, map_index: usize) -> i32 {
        update::tile_daylight(&self.map[map_index], self.globals.dlight)
    }

    /// Port of `do_character_calculate_light(cn, light)` from original code.
    ///
    /// Adjusts a raw light value according to the character's perception
    /// skill and infrared ability, clamping to valid bounds. Returns the
    /// adjusted light value used in visibility calculations.
    ///
    /// # Arguments
    /// * `cn` - Character id
    /// * `light` - Raw light value
    pub(crate) fn do_character_calculate_light(&mut self, cn: usize, light: i32) -> i32 {
        update::calculate_light(&self.characters[cn], light)
    }

    /// Port of `do_char_can_see(cn, co)` from original server logic.
    ///
    /// Determines whether character `cn` can perceive character `co`, using
    /// distance, stealth/perception skills, ambient light, and line-of-sight
    /// checks. Returns `0` when not visible, `1` for immediate/very close
    /// visibility, or a distance-derived metric otherwise.
    ///
    /// # Arguments
    /// * `cn` - Observer character id
    /// * `co` - Target character id
    pub(crate) fn do_char_can_see(&mut self, cn: usize, co: usize) -> i32 {
        if cn == co {
            return 1;
        }

        self.with_char_see(cn, |world, see, counters| {
            update::char_can_see(world, see, cn, co, counters)
        })
    }

    /// Port of `do_char_can_see_item(cn, in_idx)` from original server logic.
    ///
    /// Determines whether the character `cn` can see the item `in_idx` by
    /// considering distance, perception, ambient light, and item hiddenness.
    /// Returns 0 if not visible, 1 when very close, or a positive distance
    /// metric otherwise.
    ///
    /// # Arguments
    /// * `cn` - Observer character id
    /// * `in_idx` - Item index to test
    pub(crate) fn do_char_can_see_item(&mut self, cn: usize, in_idx: usize) -> i32 {
        // Check if item is active
        if self.items[in_idx].used != core::constants::USE_ACTIVE {
            return 0;
        }

        // Calculate raw distance (squared)
        let d1 = i32::from((self.characters[cn].x - self.items[in_idx].x as i16).abs());
        let d2 = i32::from((self.characters[cn].y - self.items[in_idx].y as i16).abs());

        let rd = d1 * d1 + d2 * d2;
        let mut d = rd;

        // Early exit for far distances
        if d > 1000 {
            return 0;
        }

        // Modify by perception
        d += 50 - i32::from(self.characters[cn].skill[skills::SK_PERCEPT][5]) * 2;

        // Modify by light (unless character has infrared)
        if self.characters[cn].flags & CharacterFlags::Infrared.bits() == 0 {
            let map_index = self.items[in_idx].x as usize
                + self.items[in_idx].y as usize * core::constants::SERVER_MAPX as usize;
            let mut light = std::cmp::max(
                i32::from(self.map[map_index].light),
                self.check_dlight(self.items[in_idx].x as usize, self.items[in_idx].y as usize),
            );

            light = self.do_character_calculate_light(cn, light);

            if light == 0 {
                return 0;
            }

            if light > 64 {
                light = 64;
            }

            d += (64 - light) * 3;
        }

        // Check for hidden items
        if self.items[in_idx].flags & core::constants::ItemFlags::IF_HIDDEN.bits() != 0 {
            d += self.items[in_idx].data[9] as i32;
        } else if rd < 3 && d > 200 {
            d = 200;
        }

        // Check distance threshold
        if d > 200 {
            return 0;
        }

        // Check line of sight
        let can_see = self.can_see(
            Some(cn),
            i32::from(self.characters[cn].x),
            i32::from(self.characters[cn].y),
            i32::from(self.items[in_idx].x),
            i32::from(self.items[in_idx].y),
            (core::constants::TILEX / 2) as i32,
        );

        if can_see == 0 {
            return 0;
        }

        // Return 1 for very close items, otherwise return distance
        if d < 1 { 1 } else { d }
    }

    /// Returns the visibility value for `(tx,ty)` from the current origin.
    ///
    /// Uses the internal `_visi` buffer (built by `can_map_see`/`can_map_go`) to
    /// inspect adjacent cells and return the best visibility metric. `0`
    /// indicates not visible; otherwise a positive integer (1 = best).
    ///
    /// # Arguments
    /// * `x, y` - Target coordinates relative to current origin
    fn check_vis(&mut self, x: i32, y: i32) -> i32 {
        update::check_vis(&self._visi, self.ox, self.oy, x, y)
    }

    /// Port of `add_vis(x,y,value)` from original helper code.
    ///
    /// Writes a visibility value into the internal `_visi` buffer at the
    /// position `(x,y)` relative to the current origin if the slot is empty.
    ///
    /// # Arguments
    /// * `x, y` - World coordinates to write
    /// * `value` - Visibility value to store
    pub(crate) fn add_vis(&mut self, x: i32, y: i32, value: i32) {
        update::add_vis(&mut self._visi, self.ox, self.oy, x, y, value);
    }

    /// Port of `check_map_go(x,y)` from original helper code.
    ///
    /// Returns `true` when the map tile at `(x,y)` is traversable for
    /// pathfinding. Checks map movement-block flags and items with
    /// `IF_MOVEBLOCK`.
    fn check_map_go(&mut self, x: i32, y: i32) -> bool {
        if x <= 0
            || x >= core::constants::SERVER_MAPX
            || y <= 0
            || y >= core::constants::SERVER_MAPY
        {
            return false;
        }

        let m = (x + y * core::constants::SERVER_MAPX) as usize;

        if self.map[m].flags & u64::from(core::constants::MF_MOVEBLOCK) != 0 {
            return false;
        }

        let map_item_idx = self.map[m].it as usize;
        if map_item_idx != 0
            && map_item_idx < self.items.len()
            && self.items[map_item_idx].flags & ItemFlags::IF_MOVEBLOCK.bits() != 0
        {
            return false;
        }

        true
    }

    /// Port of `close_vis_go(x,y,value)` from original helper code.
    ///
    /// Returns `true` if tile `(x,y)` is traversable and is adjacent to a
    /// reachable tile with the specified `value`.
    fn close_vis_go(&mut self, x: i32, y: i32, value: i8) -> bool {
        self.check_map_go(x, y) && update::neighbour_has(&self._visi, self.ox, self.oy, x, y, value)
    }

    /// Port of `reset_go(xc,yc)` from original helper code.
    ///
    /// Clears per-character see-map caches for characters in the area around
    /// `(xc,yc)` so that subsequent visibility checks will be recomputed.
    ///
    /// # Arguments
    /// * `xc, yc` - Center coordinates for the reset region
    pub(crate) fn reset_go(&mut self, xc: i32, yc: i32) {
        for y in std::cmp::max(0, yc - 18)..std::cmp::min(core::constants::SERVER_MAPY - 1, yc + 18)
        {
            for x in
                std::cmp::max(0, xc - 18)..std::cmp::min(core::constants::SERVER_MAPX - 1, xc + 18)
            {
                let cn = self.map[(x + y * core::constants::SERVER_MAPX) as usize].ch as usize;

                if cn != 0 {
                    self.see_map[cn].x = 0;
                    self.see_map[cn].y = 0;
                }
            }
        }

        self.ox = 0;
        self.oy = 0;
    }

    /// Port of `remove_lights(x,y)` from the original `helper.cpp`.
    ///
    /// Removes light contributions created by items and characters within the
    /// local neighborhood of `(x,y)`. This is the inverse of `add_lights` and
    /// writes negative light contributions back into the map.
    ///
    /// # Arguments
    /// * `x, y` - Center coordinates of the area to clear lights from
    pub(crate) fn remove_lights(&mut self, x: i32, y: i32) {
        let xs = cmp::max(1, x - core::constants::LIGHTDIST);
        let ys = cmp::max(1, y - core::constants::LIGHTDIST);
        let xe = cmp::min(
            core::constants::SERVER_MAPX - 2,
            x + 1 + core::constants::LIGHTDIST,
        );
        let ye = cmp::min(
            core::constants::SERVER_MAPY - 2,
            y + 1 + core::constants::LIGHTDIST,
        );

        for yy in ys..ye {
            for xx in xs..xe {
                let m = (xx + yy * core::constants::SERVER_MAPX) as usize;

                let item_idx = self.map[m].it as usize;
                let light_value_from_item = if item_idx != 0 && item_idx < self.items.len() {
                    let it = &mut self.items[item_idx];
                    if it.active != 0 {
                        it.light[1]
                    } else {
                        it.light[0]
                    }
                } else {
                    0
                };

                if light_value_from_item != 0 {
                    self.do_add_light(xx, yy, -i32::from(light_value_from_item));
                }

                let cn = self.map[m].ch as usize;

                let light_value_from_character = if !Character::is_sane_character(cn) {
                    0
                } else {
                    self.characters[cn].light
                };

                if light_value_from_character != 0 {
                    self.do_add_light(xx, yy, -i32::from(light_value_from_character));
                }

                self.map[m].dlight = 0;
            }
        }
    }
}
