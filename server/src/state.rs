use core::constants::{CharacterFlags, ItemFlags, MAXCHARS, MAXPLAYER};
use core::types::{Character, FontColor, ServerPlayer};
use rand::Rng;
use std::cmp;
use std::sync::{OnceLock, RwLock};

use crate::driver::{self, Driver};
use crate::effect::EffectManager;
use crate::god::God;
use crate::network_manager::NetworkManager;
use crate::path_finding::PathFinder;
use crate::repository::Repository;
use crate::server::Server;
use crate::{enums, helpers};

static STATE: OnceLock<RwLock<State>> = OnceLock::new();

pub struct State {
    pathfinder: PathFinder,
    _visi: [i8; 40 * 40],
    visi: [i8; 40 * 40],
    see_miss: u64,
    see_hit: u64,
    ox: i32,
    oy: i32,
    is_monster: bool,
}

impl State {
    fn new() -> Self {
        State {
            pathfinder: PathFinder::new(),
            _visi: [0; 40 * 40],
            visi: [0; 40 * 40],
            see_miss: 0,
            see_hit: 0,
            ox: 0,
            oy: 0,
            is_monster: false,
        }
    }

    pub fn initialize() -> Result<(), String> {
        let state = State::new();
        STATE
            .set(RwLock::new(state))
            .map_err(|_| "State already initialized".to_string())?;
        Ok(())
    }

    pub fn with<F, R>(f: F) -> R
    where
        F: FnOnce(&State) -> R,
    {
        let state = STATE.get().expect("State not initialized").read().unwrap();
        f(&*state)
    }

    pub fn with_mut<F, R>(f: F) -> R
    where
        F: FnOnce(&mut State) -> R,
    {
        let mut state = STATE.get().expect("State not initialized").write().unwrap();
        f(&mut *state)
    }

    pub fn do_character_log(
        &self,
        character_id: usize,
        font: core::types::FontColor,
        message: &str,
    ) {
        Repository::with_characters(|characters| {
            let ch = &characters[character_id];
            if ch.player == 0 && ch.temp != 15 {
                log::warn!(
                    "do_character_log: Character '{}' has no associated player.",
                    ch.get_name(),
                );
                return;
            }

            self.do_log(character_id, font, message);
        });
    }

    pub fn do_log(&self, character_id: usize, font: core::types::FontColor, message: &str) {
        let mut buffer: [u8; 16] = [0; 16];

        Repository::with_characters(|characters| {
            let ch = &characters[character_id];

            if !ServerPlayer::is_sane_player(ch.player as usize)
                || (ch.flags & CharacterFlags::CF_PLAYER.bits()) == 0
            {
                let id = ch.player;
                log::error!(
                    "do_log: Invalid player ID {} for character '{}'",
                    id,
                    ch.get_name(),
                );
                return;
            }

            let matching_player_id = Server::with_players(|players| {
                for i in 0..MAXPLAYER as usize {
                    if players[i].usnr == character_id {
                        return Some(i);
                    }
                }

                None
            });

            if matching_player_id.is_none() {
                log::error!(
                    "do_log: No matching player found for character '{}'",
                    ch.get_name(),
                );
                return;
            }

            let mut bytes_sent: usize = 0;
            let len = message.len() - 1;

            while bytes_sent <= len {
                buffer[0] = core::constants::SV_LOG + font as u8;

                for i in 0..15 {
                    if bytes_sent + i > len {
                        buffer[i + 1] = 0;
                    } else {
                        buffer[i + 1] = message.as_bytes()[bytes_sent + i];
                    }
                }

                NetworkManager::with(|network| {
                    network.xsend(matching_player_id.unwrap() as usize, &buffer, 16);
                });

                bytes_sent += 15;
            }
        });
    }

    pub fn do_area_log(
        &self,
        cn: usize,
        co: usize,
        xs: i32,
        ys: i32,
        font: core::types::FontColor,
        message: &str,
    ) {
        let x_min = cmp::max(0, xs - 12);
        let x_max = cmp::min(core::constants::SERVER_MAPX as i32, xs + 13);
        let y_min = cmp::max(0, ys - 12);
        let y_max = cmp::min(core::constants::SERVER_MAPY as i32, ys + 13);

        let mut recipients: Vec<usize> = Vec::new();

        Repository::with_map(|map| {
            for y in y_min..y_max {
                let row_base = y * core::constants::SERVER_MAPX as i32;
                for x in x_min..x_max {
                    let idx = (x + row_base) as usize;
                    let cc = map[idx].ch as usize;
                    if cc == 0 || cc == cn || cc == co {
                        continue;
                    }
                    recipients.push(cc);
                }
            }
        });

        let recipients: Vec<usize> = Repository::with_characters(|characters| {
            recipients
                .into_iter()
                .filter(|cc| {
                    *cc < MAXCHARS as usize
                        && characters[*cc].used == core::constants::USE_ACTIVE
                        && characters[*cc].player != 0
                        && (characters[*cc].flags & CharacterFlags::CF_PLAYER.bits()) != 0
                })
                .collect()
        });

        for cc in recipients {
            self.do_character_log(cc, font, message);
        }
    }

    pub fn do_add_light(&mut self, x_center: i32, y_center: i32, mut strength: i32) {
        // First add light to the center
        let center_map_index =
            (y_center as usize) * core::constants::SERVER_MAPX as usize + (x_center as usize);

        Repository::with_map_mut(|map_tiles| {
            map_tiles[center_map_index].add_light(strength);
        });

        let flag = if strength < 0 {
            strength = -strength;
            1
        } else {
            0
        };

        let xs = cmp::max(0, x_center - core::constants::LIGHTDIST);
        let ys = cmp::max(0, y_center - core::constants::LIGHTDIST);
        let xe = cmp::min(
            core::constants::SERVER_MAPX as i32 - 1,
            x_center + 1 + core::constants::LIGHTDIST,
        );
        let ye = cmp::min(
            core::constants::SERVER_MAPY as i32 - 1,
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
                    let d = strength / (v * (x_center - x).abs() + (y_center - y).abs());
                    let map_index =
                        (y as usize) * core::constants::SERVER_MAPX as usize + (x as usize);

                    Repository::with_map_mut(|map_tiles| {
                        if flag == 1 {
                            map_tiles[map_index].add_light(-d);
                        } else {
                            map_tiles[map_index].add_light(d);
                        }
                    });
                }
            }
        }
    }

    fn compute_dlight(&mut self, xc: i32, yc: i32) {
        let xs = cmp::max(0, xc - core::constants::LIGHTDIST);
        let ys = cmp::max(0, yc - core::constants::LIGHTDIST);
        let xe = cmp::min(
            core::constants::SERVER_MAPX as i32 - 1,
            xc + 1 + core::constants::LIGHTDIST,
        );
        let ye = cmp::min(
            core::constants::SERVER_MAPY as i32 - 1,
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

                let m = (x + y * core::constants::SERVER_MAPX as i32) as usize;

                let should_continue = Repository::with_map(|map| {
                    map[m].flags & core::constants::MF_INDOORS as u64 != 0
                });

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

        let center_index = (xc + yc * core::constants::SERVER_MAPX as i32) as usize;

        Repository::with_map_mut(|map| {
            if center_index < map.len() {
                map[center_index].dlight = best as u16;
            }
        });
    }

    /// Port of `add_lights(int x, int y)` from the original `helper.cpp`.
    pub fn add_lights(&mut self, x: i32, y: i32) {
        let x0 = x;
        let y0 = y;

        let xs = cmp::max(1, x0 - core::constants::LIGHTDIST);
        let ys = cmp::max(1, y0 - core::constants::LIGHTDIST);
        let xe = cmp::min(
            core::constants::SERVER_MAPX as i32 - 2,
            x0 + 1 + core::constants::LIGHTDIST,
        );
        let ye = cmp::min(
            core::constants::SERVER_MAPY as i32 - 2,
            y0 + 1 + core::constants::LIGHTDIST,
        );

        for yy in ys..ye {
            for xx in xs..xe {
                let m = (xx + yy * core::constants::SERVER_MAPX as i32) as usize;

                let item_idx = Repository::with_map(|map| map[m].it as usize);
                let light_value_from_item = Repository::with_items(|items| {
                    if item_idx != 0 && item_idx < items.len() {
                        let it = &items[item_idx];
                        if it.active != 0 {
                            it.light[1]
                        } else {
                            it.light[0]
                        }
                    } else {
                        0
                    }
                });

                if light_value_from_item != 0 {
                    self.do_add_light(xx, yy, light_value_from_item as i32);
                }

                let cn = Repository::with_map(|map| map[m].ch as usize);

                let light_value_from_character = Repository::with_characters(|characters| {
                    if !Character::is_sane_character(cn) {
                        0
                    } else {
                        characters[cn].light
                    }
                });

                if light_value_from_character != 0 {
                    self.do_add_light(xx, yy, light_value_from_character as i32);
                }

                let is_indoors = Repository::with_map(|map| {
                    map[m].flags & core::constants::MF_INDOORS as u64 != 0
                });
                if is_indoors {
                    self.compute_dlight(xx, yy);
                }
            }
        }
    }

    pub fn can_see(
        &mut self,
        character_id: Option<usize>,
        fx: i32,
        fy: i32,
        tx: i32,
        ty: i32,
        max_distance: i32,
    ) -> i32 {
        Repository::with_see_map_mut(|see_map| {
            Repository::with_characters(|characters| {
                match character_id {
                    Some(cn) => {
                        if (fx != see_map[cn].x) || (fy != see_map[cn].y) {
                            self.is_monster =
                                characters[cn].is_monster() && !characters[cn].is_usurp_or_thrall();

                            // Copy the visibility data from see_map to our working buffer
                            self._visi.copy_from_slice(&see_map[cn].vis);

                            self.can_map_see(fx, fy, max_distance);

                            // Copy the updated visibility data back to see_map
                            see_map[cn].vis.copy_from_slice(&self._visi);
                            see_map[cn].x = fx;
                            see_map[cn].y = fy;
                            self.see_miss += 1;
                        } else {
                            // Copy the visibility data from see_map for checking
                            self._visi.copy_from_slice(&see_map[cn].vis);
                            self.see_hit += 1;
                            self.ox = fx;
                            self.oy = fy;
                        }
                    }
                    None => {
                        if (self.ox != fx) || (self.oy != fy) {
                            self.is_monster = false;
                            self.can_map_see(fx, fy, max_distance);
                        }
                    }
                }
            })
        });

        self.check_vis(tx, ty)
    }

    pub fn can_map_go(&mut self, fx: i32, fy: i32, max_distance: i32) {
        // Clear the visibility array
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
                if self.close_vis_see(x, y, dist as i8) {
                    self.add_vis(x, y, dist + 1);
                }

                let y = yc + dist;
                if self.close_vis_see(x, y, dist as i8) {
                    self.add_vis(x, y, dist + 1);
                }
            }

            // Left and right vertical lines (excluding corners already done)
            for y in (yc - dist + 1)..=(yc + dist - 1) {
                let x = xc - dist;
                if self.close_vis_see(x, y, dist as i8) {
                    self.add_vis(x, y, dist + 1);
                }

                let x = xc + dist;
                if self.close_vis_see(x, y, dist as i8) {
                    self.add_vis(x, y, dist + 1);
                }
            }
        }
    }

    pub fn can_map_see(&mut self, fx: i32, fy: i32, max_distance: i32) {
        // Clear the visibility array
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
                if self.close_vis_see(x, y, dist as i8) {
                    self.add_vis(x, y, dist + 1);
                }

                let y = yc + dist;
                if self.close_vis_see(x, y, dist as i8) {
                    self.add_vis(x, y, dist + 1);
                }
            }

            // Left and right vertical lines (excluding corners already done)
            for y in (yc - dist + 1)..=(yc + dist - 1) {
                let x = xc - dist;
                if self.close_vis_see(x, y, dist as i8) {
                    self.add_vis(x, y, dist + 1);
                }

                let x = xc + dist;
                if self.close_vis_see(x, y, dist as i8) {
                    self.add_vis(x, y, dist + 1);
                }
            }
        }
    }

    pub fn can_go(&mut self, fx: i32, fy: i32, target_x: i32, target_y: i32) -> bool {
        if self.visi != self._visi {
            self.visi = self._visi.clone();
            self.ox = 0;
            self.oy = 0;
        }

        if self.ox != fx || self.oy != fy {
            self.can_map_go(fx, fy, 15);
        }

        let tmp = self.check_vis(target_x, target_y);

        tmp != 0
    }

    pub fn check_dlight(x: usize, y: usize) -> i32 {
        let map_index = x + y * core::constants::SERVER_MAPX as usize;

        Self::check_dlightm(map_index)
    }

    pub fn check_dlightm(map_index: usize) -> i32 {
        Repository::with_map(|map| {
            Repository::with_globals(|globals| {
                if map[map_index].flags & core::constants::MF_INDOORS as u64 == 0 {
                    globals.dlight
                } else {
                    (globals.dlight * map[map_index].dlight as i32) / 256
                }
            })
        })
    }

    pub fn do_character_calculate_light(&self, cn: usize, light: i32) -> i32 {
        Repository::with_characters(|characters| {
            let character = &characters[cn];
            let mut adjusted_light = light;

            if light == 0 && character.skill[core::constants::SK_PERCEPT][5] > 150 {
                adjusted_light = 1;
            }

            adjusted_light = adjusted_light
                * std::cmp::min(character.skill[core::constants::SK_PERCEPT][5] as i32, 10)
                / 10;

            if adjusted_light > 255 {
                adjusted_light = 255;
            }

            if character.flags & CharacterFlags::CF_INFRARED.bits() != 0 && adjusted_light < 5 {
                adjusted_light = 5;
            }

            adjusted_light
        })
    }

    pub fn do_char_can_see(&self, cn: usize, co: usize) -> i32 {
        if cn == co {
            return 1;
        }

        Repository::with_characters(|characters| {
            Repository::with_map(|map| {
                if characters[co].used != core::constants::USE_ACTIVE {
                    return 0;
                }

                if characters[co].flags & CharacterFlags::CF_INVISIBLE.bits() != 0
                    && (characters[cn].get_invisibility_level()
                        < characters[co].get_invisibility_level())
                {
                    return 0;
                }

                if characters[co].flags & CharacterFlags::CF_BODY.bits() != 0 {
                    return 0;
                }

                let d1 = (characters[cn].x - characters[co].x).abs() as i32;
                let d2 = (characters[cn].y - characters[co].y).abs() as i32;

                let rd = d1 * d1 + d2 * d2;
                let mut d = rd;

                if d > 1000 {
                    return 0;
                }

                // Modify by perception and stealth
                match characters[co].mode {
                    0 => {
                        d = (d
                            * (characters[co].skill[core::constants::SK_STEALTH][5] as i32 + 20))
                            / 20;
                    }
                    1 => {
                        d = (d
                            * (characters[co].skill[core::constants::SK_STEALTH][5] as i32 + 50))
                            / 50;
                    }
                    _ => {
                        d = (d
                            * (characters[co].skill[core::constants::SK_STEALTH][5] as i32 + 100))
                            / 100;
                    }
                }

                d -= characters[cn].skill[core::constants::SK_PERCEPT][5] as i32 * 2;

                // Modify by light
                if characters[cn].flags & CharacterFlags::CF_INFRARED.bits() == 0 {
                    let map_index = characters[co].x as usize
                        + characters[co].y as usize * core::constants::SERVER_MAPX as usize;
                    let mut light = std::cmp::max(
                        map[map_index].light as i32,
                        State::check_dlight(characters[co].x as usize, characters[co].y as usize),
                    );

                    light = self.do_character_calculate_light(cn, light);

                    if light == 0 {
                        return 0;
                    }

                    if light > 64 {
                        light = 64;
                    }

                    d += (64 - light) * 2;
                }

                if rd < 3 && d > 70 {
                    d = 70;
                }

                if d > 200 {
                    return 0;
                }

                if self.can_see(
                    Some(cn),
                    characters[cn].x as i32,
                    characters[cn].y as i32,
                    characters[co].x as i32,
                    characters[co].y as i32,
                    15,
                ) == 0
                {
                    return 0;
                }

                if d < 1 {
                    return 1;
                }

                d
            })
        })
    }

    pub fn do_char_can_see_item(&self, cn: usize, in_idx: usize) -> i32 {
        Repository::with_characters(|characters| {
            Repository::with_items(|items| {
                Repository::with_map(|map| {
                    // Check if item is active
                    if items[in_idx].used != core::constants::USE_ACTIVE {
                        return 0;
                    }

                    // Calculate raw distance (squared)
                    let d1 = (characters[cn].x - items[in_idx].x as i16).abs() as i32;
                    let d2 = (characters[cn].y - items[in_idx].y as i16).abs() as i32;

                    let rd = d1 * d1 + d2 * d2;
                    let mut d = rd;

                    // Early exit for far distances
                    if d > 1000 {
                        return 0;
                    }

                    // Modify by perception
                    d += 50 - characters[cn].skill[core::constants::SK_PERCEPT][5] as i32 * 2;

                    // Modify by light (unless character has infrared)
                    if characters[cn].flags & CharacterFlags::CF_INFRARED.bits() == 0 {
                        let map_index = items[in_idx].x as usize
                            + items[in_idx].y as usize * core::constants::SERVER_MAPX as usize;
                        let mut light = std::cmp::max(
                            map[map_index].light as i32,
                            State::check_dlight(items[in_idx].x as usize, items[in_idx].y as usize),
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
                    if items[in_idx].flags & core::constants::ItemFlags::IF_HIDDEN.bits() != 0 {
                        d += items[in_idx].data[9] as i32;
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
                        characters[cn].x as i32,
                        characters[cn].y as i32,
                        items[in_idx].x as i32,
                        items[in_idx].y as i32,
                        15,
                    );

                    if can_see == 0 {
                        return 0;
                    }

                    // Return 1 for very close items, otherwise return distance
                    if d < 1 {
                        1
                    } else {
                        d
                    }
                })
            })
        })
    }

    pub fn check_vis(&self, tx: i32, ty: i32) -> i32 {
        let mut best = 99;

        let x = tx - self.ox + 20;
        let y = ty - self.oy + 20;

        // Check all 8 adjacent cells for the best (lowest) visibility value
        let offsets = [
            (1, 0),
            (-1, 0),
            (0, 1),
            (0, -1),
            (1, 1),
            (1, -1),
            (-1, 1),
            (-1, -1),
        ];

        for (dx, dy) in offsets.iter() {
            let nx = x + dx;
            let ny = y + dy;

            if nx >= 0 && nx < 40 && ny >= 0 && ny < 40 {
                let idx = (nx + ny * 40) as usize;
                let val = self._visi[idx];
                if val != 0 && val < best {
                    best = val;
                }
            }
        }

        if best == 99 {
            0
        } else {
            best as i32
        }
    }

    pub fn add_vis(&mut self, x: i32, y: i32, value: i32) {
        let vx = x - self.ox + 20;
        let vy = y - self.oy + 20;

        if vx >= 0 && vx < 40 && vy >= 0 && vy < 40 {
            let idx = (vx + vy * 40) as usize;
            if self._visi[idx] == 0 {
                self._visi[idx] = value as i8;
            }
        }
    }

    pub fn close_vis_see(&self, x: i32, y: i32, value: i8) -> bool {
        if !self.check_map_see(x, y) {
            return false;
        }

        let vx = x - self.ox + 20;
        let vy = y - self.oy + 20;

        if vx < 0 || vx >= 40 || vy < 0 || vy >= 40 {
            return false;
        }

        // Check all 8 adjacent cells
        let offsets = [
            (1, 0),
            (-1, 0),
            (0, 1),
            (0, -1),
            (1, 1),
            (1, -1),
            (-1, 1),
            (-1, -1),
        ];

        for (dx, dy) in offsets.iter() {
            let nx = vx + dx;
            let ny = vy + dy;

            if nx >= 0 && nx < 40 && ny >= 0 && ny < 40 {
                let idx = (nx + ny * 40) as usize;
                if self._visi[idx] == value {
                    return true;
                }
            }
        }

        false
    }

    fn check_map_see(&self, x: i32, y: i32) -> bool {
        // Check boundaries
        if x <= 0
            || x >= core::constants::SERVER_MAPX as i32
            || y <= 0
            || y >= core::constants::SERVER_MAPY as i32
        {
            return false;
        }

        let m = (x + y * core::constants::SERVER_MAPX as i32) as usize;

        // Check if it's a monster and the map blocks monsters
        if self.is_monster {
            let blocked = Repository::with_map(|map| {
                map[m].flags & (core::constants::MF_SIGHTBLOCK | core::constants::MF_NOMONST) as u64
                    != 0
            });
            if blocked {
                return false;
            }
        } else {
            // Check for sight blocking flags
            let blocked = Repository::with_map(|map| {
                map[m].flags & core::constants::MF_SIGHTBLOCK as u64 != 0
            });
            if blocked {
                return false;
            }
        }

        // Check if there's an item that blocks sight
        let blocks_sight = Repository::with_map(|map| {
            let item_idx = map[m].it as usize;
            if item_idx != 0 {
                Repository::with_items(|items| {
                    item_idx < items.len()
                        && items[item_idx].flags & core::constants::ItemFlags::IF_SIGHTBLOCK.bits()
                            != 0
                })
            } else {
                false
            }
        });

        if blocks_sight {
            return false;
        }

        true
    }

    /// Port of original `do_sayx(int cn, char* format, ...)` from `svr_do.cpp`.
    ///
    /// The C++ version formats a message into a local buffer, runs `process_options`,
    /// and then sends a local area log message with different fonts for player/NPC.
    pub fn do_sayx(&self, character_id: usize, message: &str) {
        let mut buf = message.to_string();
        Self::process_options(character_id, &mut buf);

        let (x, y, is_player, name) = Repository::with_characters(|characters| {
            let ch = &characters[character_id];
            (
                ch.x as i32,
                ch.y as i32,
                (ch.flags & CharacterFlags::CF_PLAYER.bits()) != 0,
                ch.get_name().to_string(),
            )
        });

        let name_short: String = name.chars().take(30).collect();
        let msg_short: String = buf.chars().take(300).collect();

        let line = format!("{}: \"{}\"\n", name_short, msg_short);

        let font = if is_player {
            core::types::FontColor::Blue
        } else {
            core::types::FontColor::Yellow
        };

        self.do_area_log(0, 0, x, y, font, &line);
    }

    fn char_play_sound(character_id: usize, sound: i32, vol: i32, pan: i32) {
        let matching_player_id = Server::with_players(|players| {
            for i in 0..MAXPLAYER as usize {
                if players[i].usnr == character_id {
                    return Some(i);
                }
            }
            None
        });

        let Some(player_id) = matching_player_id else {
            return;
        };

        let mut buf: [u8; 16] = [0; 16];
        buf[0] = core::constants::SV_PLAYSOUND;
        buf[1..5].copy_from_slice(&sound.to_le_bytes());
        buf[5..9].copy_from_slice(&vol.to_le_bytes());
        buf[9..13].copy_from_slice(&pan.to_le_bytes());

        NetworkManager::with(|network| {
            network.xsend(player_id, &buf, 13);
        });
    }

    pub fn do_area_sound(cn: usize, co: usize, xs: i32, ys: i32, nr: i32) {
        let x_min = cmp::max(0, xs - 8);
        let x_max = cmp::min(core::constants::SERVER_MAPX as i32, xs + 9);
        let y_min = cmp::max(0, ys - 8);
        let y_max = cmp::min(core::constants::SERVER_MAPY as i32, ys + 9);

        let mut recipients: Vec<(usize, i32, i32)> = Vec::new();

        Repository::with_map(|map| {
            for y in y_min..y_max {
                let row_base = y * core::constants::SERVER_MAPX as i32;
                for x in x_min..x_max {
                    let idx = (x + row_base) as usize;
                    let cc = map[idx].ch as usize;
                    if cc == 0 || cc == cn || cc == co {
                        continue;
                    }

                    let s = ys - y + xs - x;
                    let xpan = if s < 0 {
                        -500
                    } else if s > 0 {
                        500
                    } else {
                        0
                    };

                    let dist2 = (ys - y) * (ys - y) + (xs - x) * (xs - x);
                    let mut xvol = -150 - dist2 * 30;
                    if xvol < -5000 {
                        xvol = -5000;
                    }

                    recipients.push((cc, xvol, xpan));
                }
            }
        });

        let recipients_with_player: Vec<(usize, i32, i32)> =
            Repository::with_characters(|characters| {
                recipients
                    .into_iter()
                    .filter(|(cc, _, _)| characters[*cc].player != 0)
                    .collect()
            });

        for (cc, vol, pan) in recipients_with_player {
            Self::char_play_sound(cc, nr, vol, pan);
        }
    }

    /// Port of original `process_options(int cn, char* buf)` from `svr_do.cpp`.
    ///
    /// Supports a leading `#<digits>###` option prefix:
    /// - Parses the integer sound id after the first '#'
    /// - Strips the `#<digits>` and any additional leading '#' characters
    /// - If the parsed sound id is non-zero, plays it to nearby players (excluding the speaker)
    pub fn process_options(character_id: usize, buf: &mut String) {
        if !buf.starts_with('#') {
            return;
        }

        let bytes = buf.as_bytes();
        let mut idx: usize = 1; // skip initial '#'

        while idx < bytes.len() && bytes[idx].is_ascii_digit() {
            idx += 1;
        }

        let sound_id: i32 = if idx > 1 {
            buf[1..idx].parse::<i32>().unwrap_or(0)
        } else {
            0
        };

        while idx < bytes.len() && bytes[idx] == b'#' {
            idx += 1;
        }

        buf.drain(..idx);

        if sound_id != 0 {
            let (x, y) = Repository::with_characters(|characters| {
                let ch = &characters[character_id];
                (ch.x as i32, ch.y as i32)
            });
            Self::do_area_sound(character_id, 0, x, y, sound_id);
        }
    }

    pub fn reset_go(&mut self, xc: i32, yc: i32) {
        Repository::with_see_map_mut(|see_map| {
            for y in
                std::cmp::max(0, yc - 18)..std::cmp::min(core::constants::SERVER_MAPY - 1, yc + 18)
            {
                for x in std::cmp::max(0, xc - 18)
                    ..std::cmp::min(core::constants::SERVER_MAPX - 1, xc + 18)
                {
                    let cn = Repository::with_map(|map| {
                        map[(x + y * core::constants::SERVER_MAPX) as usize].ch as usize
                    });

                    see_map[cn].x = 0;
                    see_map[cn].y = 0;
                }
            }
        });

        self.ox = 0;
        self.oy = 0;
    }

    pub fn remove_lights(&mut self, x: i32, y: i32) {
        let xs = cmp::max(1, x - core::constants::LIGHTDIST);
        let ys = cmp::max(1, y - core::constants::LIGHTDIST);
        let xe = cmp::min(
            core::constants::SERVER_MAPX as i32 - 2,
            x + 1 + core::constants::LIGHTDIST,
        );
        let ye = cmp::min(
            core::constants::SERVER_MAPY as i32 - 2,
            y + 1 + core::constants::LIGHTDIST,
        );

        for yy in ys..ye {
            for xx in xs..xe {
                let m = (xx + yy * core::constants::SERVER_MAPX as i32) as usize;

                let item_idx = Repository::with_map(|map| map[m].it as usize);
                let light_value_from_item = Repository::with_items(|items| {
                    if item_idx != 0 && item_idx < items.len() {
                        let it = &items[item_idx];
                        if it.active != 0 {
                            it.light[1]
                        } else {
                            it.light[0]
                        }
                    } else {
                        0
                    }
                });

                if light_value_from_item != 0 {
                    self.do_add_light(xx, yy, -(light_value_from_item as i32));
                }

                let cn = Repository::with_map(|map| map[m].ch as usize);

                let light_value_from_character = Repository::with_characters(|characters| {
                    if !Character::is_sane_character(cn) {
                        0
                    } else {
                        characters[cn].light
                    }
                });

                if light_value_from_character != 0 {
                    self.do_add_light(xx, yy, -(light_value_from_character as i32));
                }

                Repository::with_map_mut(|map| {
                    map[m].dlight = 0;
                });
            }
        }
    }

    pub fn do_area_notify(
        &self,
        cn: i32,
        co: i32,
        xs: i32,
        ys: i32,
        notify_type: i32,
        dat1: i32,
        dat2: i32,
        dat3: i32,
        dat4: i32,
    ) {
        Repository::with_map(|map| {
            for y in std::cmp::max(0, ys - core::constants::AREA_SIZE)
                ..std::cmp::min(
                    core::constants::SERVER_MAPY,
                    ys + core::constants::AREA_SIZE + 1,
                )
            {
                let m = y * core::constants::SERVER_MAPX as i32;
                for x in std::cmp::max(0, xs - core::constants::AREA_SIZE)
                    ..std::cmp::min(
                        core::constants::SERVER_MAPX,
                        xs + core::constants::AREA_SIZE + 1,
                    )
                {
                    let cc = map[(x + m) as usize].ch;

                    if cc != 0 && cc != cn as u32 && cc != co as u32 {
                        self.do_notify_character(cc, notify_type, dat1, dat2, dat3, dat4);
                    }
                }
            }
        });
    }

    /// Store the carried item (citem) into the first available inventory slot
    /// Port of `do_store_item(int cn)` from `svr_do.cpp`
    ///
    /// Returns:
    /// - The inventory slot number (0-39) where the item was stored on success
    /// - -1 if the operation failed (citem is invalid or inventory is full)
    pub fn do_store_item(&self, cn: usize) -> i32 {
        Repository::with_characters_mut(|characters| {
            let ch = &mut characters[cn];

            // Check if citem has the high bit set (0x80000000), which indicates it's invalid
            if (ch.citem & 0x80000000) != 0 {
                return -1;
            }

            // Find first empty inventory slot
            let mut slot = -1;
            for n in 0..40 {
                if ch.item[n] == 0 {
                    slot = n as i32;
                    break;
                }
            }

            // If no empty slot found, return failure
            if slot == -1 {
                return -1;
            }

            // Store the carried item in the empty slot
            ch.item[slot as usize] = ch.citem;
            ch.citem = 0;

            // Update character to sync with client
            ch.set_do_update_flags();

            slot
        })
    }

    /// Port of `do_char_killed(int cn, int co)` from `svr_do.cpp`
    ///
    /// Handles all aspects of character death including:
    /// - Notifications and logging
    /// - Sound effects
    /// - Alignment changes for killers
    /// - Item dropping and grave creation
    /// - Player resurrection
    /// - NPC respawn
    ///
    /// # Arguments
    /// * `character_id` - The character who died (co in C++)
    /// * `killer_id` - The character who did the killing (cn in C++, can be 0)
    pub fn do_character_killed(&self, character_id: usize, killer_id: usize) {
        // Send death notification
        self.do_notify_character(
            character_id as u32,
            core::constants::NT_DIED as i32,
            killer_id as i32,
            0,
            0,
            0,
        );

        // Log the kill
        if killer_id != 0 {
            log::info!(
                "Character {} killed character {} ({})",
                killer_id,
                character_id,
                Repository::with_characters(|ch| ch[character_id].get_name().to_string())
            );
        } else {
            log::info!("Character {} died", character_id);
        }

        // Get map flags for both characters
        let (co_x, co_y, co_flags, co_temp, co_sound) = Repository::with_characters(|characters| {
            let co = &characters[character_id];
            (co.x, co.y, co.flags, co.temp, co.sound)
        });

        let mut map_flags = Repository::with_map(|map| {
            let idx = (co_x + co_y * core::constants::SERVER_MAPX as i16) as usize;
            map[idx].flags
        });

        if killer_id != 0 {
            let cn_flags = Repository::with_characters(|characters| {
                let cn = &characters[killer_id];
                let idx = (cn.x + cn.y * core::constants::SERVER_MAPX as i16) as usize;
                Repository::with_map(|map| map[idx].flags)
            });
            map_flags &= cn_flags;
        }

        // Play death sound effects
        // Hack for grolms (templates 364-374)
        if co_temp >= 364 && co_temp <= 374 {
            Self::do_area_sound(character_id, 0, co_x as i32, co_y as i32, 17);
            Self::char_play_sound(character_id, 17, -150, 0);
        }
        // Hack for gargoyles (templates 375-381)
        else if co_temp >= 375 && co_temp <= 381 {
            Self::do_area_sound(character_id, 0, co_x as i32, co_y as i32, 18);
            Self::char_play_sound(character_id, 18, -150, 0);
        }
        // Normal death sound
        else {
            let sound = co_sound + 2;
            Self::do_area_sound(character_id, 0, co_x as i32, co_y as i32, sound as i32);
            Self::char_play_sound(character_id, sound as i32, -150, 0);
        }

        // Cleanup for ghost companions
        if co_temp == core::constants::CT_COMPANION as u16 {
            Repository::with_characters_mut(|characters| {
                let cc = characters[character_id].data[63] as usize;
                if Character::is_sane_character(cc)
                    && characters[cc].data[64] == character_id as i32
                {
                    characters[cc].data[64] = 0;
                }
                characters[character_id].data[63] = 0;
            });
        }

        // A player killed someone or something
        if killer_id != 0 && killer_id != character_id {
            let (is_killer_player, is_arena, co_alignment, co_temp, co_is_player) =
                Repository::with_characters(|characters| {
                    let is_killer_player =
                        characters[killer_id].flags & CharacterFlags::CF_PLAYER.bits() != 0;
                    let co_alignment = characters[character_id].alignment;
                    let co_temp = characters[character_id].temp;
                    let co_is_player =
                        characters[character_id].flags & CharacterFlags::CF_PLAYER.bits() != 0;
                    (
                        is_killer_player,
                        map_flags & core::constants::MF_ARENA as u64 == 0,
                        co_alignment,
                        co_temp,
                        co_is_player,
                    )
                });

            if is_killer_player && is_arena {
                // Adjust alignment
                Repository::with_characters_mut(|characters| {
                    characters[killer_id].alignment -= co_alignment / 50;
                    if characters[killer_id].alignment > 7500 {
                        characters[killer_id].alignment = 7500;
                    }
                    if characters[killer_id].alignment < -7500 {
                        characters[killer_id].alignment = -7500;
                    }
                });

                // Check for killing priests (becoming purple)
                if co_temp == core::constants::CT_PRIEST as u16 {
                    let killer_kindred = Repository::with_characters(|ch| ch[killer_id].kindred);

                    if killer_kindred as u32 & core::constants::KIN_PURPLE != 0 {
                        self.do_character_log(
                            killer_id,
                            core::types::FontColor::Yellow,
                            "Ahh, that felt good!\n",
                        );
                    } else {
                        Repository::with_characters_mut(|characters| {
                            Repository::with_globals_mut(|globals| {
                                characters[killer_id].data[67] = globals.ticker;
                            });
                        });
                        self.do_character_log(
                            killer_id,
                            core::types::FontColor::Red,
                            "So, you want to be a player killer, right?\n",
                        );
                        self.do_character_log(
                            killer_id,
                            core::types::FontColor::Red,
                            "To join the purple one and be a killer, type #purple now.\n",
                        );
                        // TODO: Implement fx_add_effect
                        log::info!("TODO: Add effect 6 at position ({}, {})", co_x, co_y);
                    }
                }

                // Check for killing shopkeepers & questgivers (alignment 10000)
                if !co_is_player && co_alignment == 10000 {
                    self.do_character_log(
                        killer_id,
                        core::types::FontColor::Red,
                        "You feel a god look into your soul. He seems to be angry.\n",
                    );

                    Repository::with_characters_mut(|characters| {
                        characters[killer_id].data[40] += 1;
                        let penalty = if characters[killer_id].data[40] < 50 {
                            -characters[killer_id].data[40] * 100
                        } else {
                            -5000
                        };
                        characters[killer_id].luck += penalty;

                        let luck_to_print = characters[killer_id].luck;
                        log::info!(
                            "Reduced luck by {} to {} for killing {} (t={})",
                            penalty,
                            luck_to_print,
                            characters[character_id].get_name(),
                            co_temp
                        );
                    });
                }

                Repository::with_characters_mut(|characters| {
                    // Update statistics
                    let r1: u32 = helpers::points2rank(characters[killer_id].points_tot as u32);
                    let r2: u32 = helpers::points2rank(characters[character_id].points_tot as u32);

                    if (r1 as i32 - r2 as i32).abs() < 3 {
                        // Approximately own rank
                        characters[killer_id].data[24] += 1; // overall counter
                        if characters[character_id].data[42] == 27 {
                            characters[killer_id].data[27] += 1; // black stronghold counter
                        }
                    } else if r2 > r1 {
                        // Above own rank
                        characters[killer_id].data[25] += 1;
                        if characters[character_id].data[42] == 27 {
                            characters[killer_id].data[28] += 1;
                        }
                    } else {
                        // Below own rank
                        characters[killer_id].data[23] += 1;
                        if characters[character_id].data[42] == 27 {
                            characters[killer_id].data[26] += 1;
                        }
                    }

                    if co_is_player {
                        characters[killer_id].data[29] += 1;
                    } else {
                        // TODO: Implement killed_class and get_class_name
                        // Check for first kill of this monster class
                        if characters[character_id].monster_class != 0 {
                            let monster_class_to_log = characters[character_id].monster_class;
                            log::info!(
                                "TODO: Check if first kill of monster class {}",
                                monster_class_to_log,
                            );
                        }
                    }
                });
            }

            // A follower (gargoyle, ghost companion) killed someone
            let follower_owner = Repository::with_characters(|characters| {
                if characters[killer_id].flags & CharacterFlags::CF_PLAYER.bits() == 0 {
                    let cc = characters[killer_id].data[63] as usize;
                    if cc != 0 && Character::is_sane_character(cc) {
                        Some(cc)
                    } else {
                        None
                    }
                } else {
                    None
                }
            });

            if let Some(cc) = follower_owner {
                let is_owner_player = Repository::with_characters(|ch| {
                    ch[cc].flags & CharacterFlags::CF_PLAYER.bits() != 0
                });

                if is_owner_player && !co_is_player && co_alignment == 10000 {
                    self.do_character_log(cc, core::types::FontColor::Red,
                        "A goddess is about to turn your follower into a frog, but notices that you are responsible. You feel her do something to you. Nothing good, that's for sure.\n");

                    Repository::with_characters_mut(|characters| {
                        characters[cc].data[40] += 1;
                        let penalty = if characters[cc].data[40] < 50 {
                            -characters[cc].data[40] * 100
                        } else {
                            -5000
                        };
                        characters[cc].luck += penalty;

                        let luck_to_print = characters[cc].luck;
                        log::info!(
                            "Reduced luck by {} to {} for follower killing {} (t={})",
                            penalty,
                            luck_to_print,
                            characters[character_id].get_name(),
                            co_temp
                        );
                    });
                }

                // Notify area about the kill
                let (cc_x, cc_y) = Repository::with_characters(|ch| (ch[cc].x, ch[cc].y));
                self.do_area_notify(
                    cc as i32,
                    character_id as i32,
                    cc_x as i32,
                    cc_y as i32,
                    core::constants::NT_SEEHIT as i32,
                    cc as i32,
                    character_id as i32,
                    0,
                    0,
                );
            }
        }

        // Handle player death
        let is_player = Repository::with_characters(|ch| {
            ch[character_id].flags & CharacterFlags::CF_PLAYER.bits() != 0
        });

        if is_player {
            // Update player death statistics
            Repository::with_globals_mut(|globals| {
                globals.players_died += 1;
            });

            // Adjust luck if negative
            Repository::with_characters_mut(|characters| {
                if characters[character_id].luck < 0 {
                    characters[character_id].luck =
                        std::cmp::min(0, characters[character_id].luck + 10);
                }

                // Set killed by message
                characters[character_id].data[14] += 1;
                if killer_id != 0 {
                    let is_killer_player =
                        characters[killer_id].flags & CharacterFlags::CF_PLAYER.bits() != 0;
                    if is_killer_player {
                        characters[character_id].data[15] = killer_id as i32 | 0x10000;
                    } else {
                        characters[character_id].data[15] = characters[killer_id].temp as i32;
                    }
                } else {
                    characters[character_id].data[15] = 0;
                }

                Repository::with_globals(|globals| {
                    characters[character_id].data[16] = globals.mdday + globals.mdyear * 300;
                });
                characters[character_id].data[17] =
                    (co_x + co_y * core::constants::SERVER_MAPX as i16) as i32;
            });

            self.handle_player_death(character_id, killer_id, map_flags);
        } else {
            // Handle NPC death
            let is_labkeeper = Repository::with_characters(|ch| {
                ch[character_id].flags & CharacterFlags::CF_LABKEEPER.bits() != 0
            });

            if is_labkeeper {
                self.handle_labkeeper_death(character_id, killer_id);
            } else {
                self.handle_npc_death(character_id, killer_id);
            }
        }

        // Remove from enemy lists
        // TODO: Implement remove_enemy
        log::info!(
            "TODO: Remove character {} from all enemy lists",
            character_id
        );

        // Schedule respawn and show death animation
        // TODO: Implement fx_add_effect for death animation
        log::info!("TODO: Add death effect at position ({}, {})", co_x, co_y);
    }

    /// Handle player death including resurrection and grave creation
    pub fn handle_player_death(&self, co: usize, cn: usize, map_flags: u64) {
        // Remember template if we're to respawn this character
        let temp = Repository::with_characters(|characters| {
            if characters[co].flags & CharacterFlags::CF_RESPAWN.bits() != 0 {
                characters[co].temp
            } else {
                0
            }
        });

        // Check for Guardian Angel (Wimpy skill)
        let wimp = Repository::with_characters(|characters| {
            let mut wimp_power = 0;
            for n in 0..20 {
                let item_idx = characters[co].spell[n] as usize;
                if item_idx != 0 {
                    Repository::with_items(|items| {
                        let power_to_print = items[item_idx].power;
                        if item_idx < items.len() {
                            log::info!(
                                "spell active: {}, power of {}",
                                items[item_idx].get_name(),
                                power_to_print
                            );
                            if items[item_idx].temp == core::constants::SK_WIMPY as u16 {
                                wimp_power = items[item_idx].power / 2;
                            }
                        }
                    });
                }
            }
            wimp_power
        });

        let wimp = if map_flags & core::constants::MF_ARENA as u64 != 0 {
            205
        } else {
            wimp
        };

        // Find free character slot for body/grave
        let cc = Repository::with_characters(|characters| {
            for cc in 1..MAXCHARS {
                if characters[cc].used == core::constants::USE_EMPTY {
                    return Some(cc);
                }
            }
            None
        });

        let Some(cc) = cc else {
            log::error!(
                "Could not clone character {} for grave, all char slots full!",
                co
            );
            return;
        };

        // Clone character to create grave
        Repository::with_characters_mut(|characters| {
            characters[cc] = characters[co].clone();
        });

        // Drop items and money based on wimp chance
        self.handle_item_drops(co, cc, wimp as i32, cn);

        // Move player to temple
        let (temple_x, temple_y, cur_x, cur_y) = Repository::with_characters(|ch| {
            (ch[co].temple_x, ch[co].temple_y, ch[co].x, ch[co].y)
        });

        if cur_x as u16 == temple_x && cur_y as u16 == temple_y {
            God::transfer_char(co, (temple_x + 4) as usize, (temple_y + 4) as usize);
        } else {
            God::transfer_char(co, temple_x as usize, temple_y as usize);
        }

        // Resurrect player with 10 HP
        Repository::with_characters_mut(|characters| {
            characters[co].a_hp = 10000; // 10 HP (stored as 10000)
            characters[co].status = 0;
            characters[co].attack_cn = 0;
            characters[co].skill_nr = 0;
            characters[co].goto_x = 0;
            characters[co].use_nr = 0;
            characters[co].misc_action = 0;
            characters[co].stunned = 0;
            characters[co].retry = 0;
            characters[co].current_enemy = 0;
            for m in 0..4 {
                characters[co].enemy[m] = 0;
            }
        });

        self.plr_reset_status(co);

        // Apply permanent stat loss if not a god and no guardian angel
        let is_god =
            Repository::with_characters(|ch| ch[co].flags & CharacterFlags::CF_GOD.bits() != 0);

        if !is_god && wimp == 0 {
            self.apply_death_penalties(co);
        } else if wimp != 0 && map_flags & core::constants::MF_ARENA as u64 == 0 {
            self.do_character_log(
                co,
                core::types::FontColor::Yellow,
                "Sometimes a Guardian Angel is really helpful...\n",
            );
        }

        // Update player character
        // TODO: Implement do_update_char
        Repository::with_characters_mut(|ch| {
            ch[co].set_do_update_flags();
        });

        // Setup the grave (body)
        Repository::with_characters_mut(|characters| {
            self.plr_reset_status(cc);

            characters[cc].player = 0;
            characters[cc].flags = CharacterFlags::CF_BODY.bits();
            characters[cc].a_hp = 0;
            characters[cc].data[core::constants::CHD_CORPSEOWNER] = co as i32;
            characters[cc].data[99] = 1;
            characters[cc].data[98] = 0;

            characters[cc].attack_cn = 0;
            characters[cc].skill_nr = 0;
            characters[cc].goto_x = 0;
            characters[cc].use_nr = 0;
            characters[cc].misc_action = 0;
            characters[cc].stunned = 0;
            characters[cc].retry = 0;
            characters[cc].current_enemy = 0;
            for m in 0..4 {
                characters[cc].enemy[m] = 0;
            }
        });

        // Update grave character
        Repository::with_characters_mut(|ch| {
            ch[cc].set_do_update_flags();
        });

        // TODO: Implement plr_map_set
        log::info!("TODO: Set map for grave character {}", cc);
    }

    /// Handle NPC death
    fn handle_npc_death(&self, co: usize, cn: usize) {
        // Update NPC death statistics
        Repository::with_globals_mut(|globals| {
            globals.npcs_died += 1;
        });

        // TODO: Implement plr_reset_status
        log::info!("TODO: Reset NPC status for character {}", co);

        // Check for USURP flag (player controlling NPC)
        let usurp_player = Repository::with_characters(|characters| {
            if characters[co].flags & CharacterFlags::CF_USURP.bits() != 0 {
                let c2 = characters[co].data[97] as usize;
                if Character::is_sane_character(c2) {
                    Some((c2, characters[co].player))
                } else {
                    None
                }
            } else {
                None
            }
        });

        if let Some((c2, player_nr)) = usurp_player {
            Repository::with_characters_mut(|characters| {
                characters[c2].player = player_nr;
                // TODO: Update player[nr].usnr = c2
                log::info!("TODO: Transfer player {} from {} to {}", player_nr, co, c2);
                characters[c2].flags &= !CharacterFlags::CF_CCP.bits();
            });
        } else if let Some((_, player_nr)) = usurp_player {
            // TODO: Implement player_exit
            log::info!("TODO: player_exit for player {}", player_nr);
        }

        log::info!("new npc body");

        // Convert to body
        let should_respawn = Repository::with_characters(|characters| {
            characters[co].flags & CharacterFlags::CF_RESPAWN.bits() != 0
        });

        Repository::with_characters_mut(|characters| {
            if should_respawn {
                characters[co].flags =
                    CharacterFlags::CF_BODY.bits() | CharacterFlags::CF_RESPAWN.bits();
            } else {
                characters[co].flags = CharacterFlags::CF_BODY.bits();
            }

            characters[co].a_hp = 0;

            // Set corpse owner (killer only mode vs all can loot)
            #[cfg(feature = "KILLERONLY")]
            {
                let cc = Repository::with_characters(|ch| {
                    if cn != 0 && !(ch[cn].flags & CharacterFlags::CF_PLAYER.bits() != 0) {
                        let cc = ch[cn].data[63] as usize;
                        if cc != 0 && (ch[cc].flags & CharacterFlags::CF_PLAYER.bits() != 0) {
                            Some(cc)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                });

                if let Some(cc) = cc {
                    characters[co].data[CHD_CORPSEOWNER] = cc as i32;
                } else if cn != 0 {
                    let is_cn_player = characters[cn].flags & CharacterFlags::CF_PLAYER.bits() != 0;
                    if is_cn_player {
                        characters[co].data[CHD_CORPSEOWNER] = cn as i32;
                    } else {
                        characters[co].data[CHD_CORPSEOWNER] = 0;
                    }
                } else {
                    characters[co].data[CHD_CORPSEOWNER] = 0;
                }
            }
            #[cfg(not(feature = "KILLERONLY"))]
            {
                characters[co].data[core::constants::CHD_CORPSEOWNER] = 0;
            }

            characters[co].data[99] = 0;
            characters[co].data[98] = 0;

            characters[co].attack_cn = 0;
            characters[co].skill_nr = 0;
            characters[co].goto_x = 0;
            characters[co].use_nr = 0;
            characters[co].misc_action = 0;
            characters[co].stunned = 0;
            characters[co].retry = 0;
            characters[co].current_enemy = 0;
            for m in 0..4 {
                characters[co].enemy[m] = 0;
            }

            // Destroy active spells
            for n in 0..20 {
                if characters[co].spell[n] != 0 {
                    let item_idx = characters[co].spell[n] as usize;
                    characters[co].spell[n] = 0;
                    Repository::with_items_mut(|items| {
                        if item_idx < items.len() {
                            items[item_idx].used = core::constants::USE_EMPTY;
                        }
                    });
                }
            }
        });

        // If killer is a player, check for special items in grave
        let is_cn_player = if cn != 0 {
            Repository::with_characters(|ch| {
                Character::is_sane_character(cn)
                    && ch[cn].flags & CharacterFlags::CF_PLAYER.bits() != 0
            })
        } else {
            false
        };

        if is_cn_player {
            // TODO: Implement do_ransack_corpse
            log::info!("TODO: Ransack corpse {} by player {}", co, cn);
        }

        // Update character
        Repository::with_characters_mut(|ch| {
            ch[co].set_do_update_flags();
        });
    }

    /// Handle lab keeper death (special case)
    fn handle_labkeeper_death(&self, co: usize, cn: usize) {
        // TODO: Implement plr_map_remove
        log::info!("TODO: Remove character {} from map", co);

        // Destroy all items
        // TODO: Seems like we're getting rid of the items twice?
        God::destroy_items(co);
        Repository::with_characters_mut(|characters| {
            characters[co].citem = 0;
            characters[co].gold = 0;
            for z in 0..40 {
                characters[co].item[z] = 0;
            }
            for z in 0..20 {
                characters[co].worn[z] = 0;
            }
            characters[co].used = core::constants::USE_EMPTY;
        });

        // TODO: Implement use_labtransfer2
        log::info!("TODO: Lab transfer for character {} killed by {}", co, cn);
    }

    /// Handle item drops on death based on wimpy (guardian angel) chance
    fn handle_item_drops(&self, co: usize, cc: usize, wimp: i32, cn: usize) {
        use core::constants::*;

        // Handle gold
        Repository::with_characters_mut(|characters| {
            if characters[co].gold != 0 {
                let mut rng = rand::thread_rng();
                if wimp < rng.gen_range(0..100) {
                    characters[co].gold = 0;
                } else {
                    characters[cc].gold = 0;
                }
            }
        });

        // Handle inventory items
        for n in 0..40 {
            let item_idx = Repository::with_characters(|ch| ch[co].item[n]);
            if item_idx == 0 {
                continue;
            }

            // Check if item may be given
            if !self.do_maygive(cn, 0, item_idx as usize) {
                Repository::with_items_mut(|items| {
                    if (item_idx as usize) < items.len() {
                        items[item_idx as usize].used = USE_EMPTY;
                    }
                });
                Repository::with_characters_mut(|characters| {
                    characters[co].item[n] = 0;
                    characters[cc].item[n] = 0;
                });
                continue;
            }

            let mut rng = rand::thread_rng();
            if wimp <= rng.gen_range(0..100) {
                // Drop in grave
                Repository::with_characters_mut(|characters| {
                    characters[co].item[n] = 0;
                });
                Repository::with_items_mut(|items| {
                    if (item_idx as usize) < items.len() {
                        items[item_idx as usize].carried = cc as u16;

                        let item_template_to_print = items[item_idx as usize].temp;
                        log::info!(
                            "Dropped {} (t={}) in Grave",
                            items[item_idx as usize].get_name(),
                            item_template_to_print,
                        );
                    }
                });
            } else {
                // Player keeps it
                Repository::with_characters_mut(|characters| {
                    characters[cc].item[n] = 0;
                });
            }
        }

        // Handle carried item (citem)
        let citem = Repository::with_characters(|ch| ch[co].citem);
        if citem != 0 {
            if !self.do_maygive(cn, 0, citem as usize) {
                Repository::with_items_mut(|items| {
                    if (citem as usize) < items.len() {
                        items[citem as usize].used = USE_EMPTY;
                    }
                });
                Repository::with_characters_mut(|characters| {
                    characters[co].citem = 0;
                    characters[cc].citem = 0;
                });
            } else {
                let mut rng = rand::thread_rng();
                if wimp <= rng.gen_range(0..100) {
                    Repository::with_characters_mut(|characters| {
                        characters[co].citem = 0;
                    });
                    Repository::with_items_mut(|items| {
                        if (citem as usize) < items.len() {
                            items[citem as usize].carried = cc as u16;
                            let item_template_to_print = items[citem as usize].temp;
                            log::info!(
                                "Dropped {} (t={}) in Grave",
                                items[citem as usize].get_name(),
                                item_template_to_print,
                            );
                        }
                    });
                } else {
                    Repository::with_characters_mut(|characters| {
                        characters[cc].citem = 0;
                    });
                }
            }
        }

        // Handle worn items
        for n in 0..20 {
            let item_idx = Repository::with_characters(|ch| ch[co].worn[n]);
            if item_idx == 0 {
                continue;
            }

            if !self.do_maygive(cn, 0, item_idx as usize) {
                Repository::with_items_mut(|items| {
                    if (item_idx as usize) < items.len() {
                        items[item_idx as usize].used = USE_EMPTY;
                    }
                });
                Repository::with_characters_mut(|characters| {
                    characters[co].worn[n] = 0;
                    characters[cc].worn[n] = 0;
                });
                continue;
            }

            let mut rng = rand::thread_rng();
            if wimp <= rng.gen_range(0..100) {
                Repository::with_characters_mut(|characters| {
                    characters[co].worn[n] = 0;
                });
                Repository::with_items_mut(|items| {
                    if (item_idx as usize) < items.len() {
                        items[item_idx as usize].carried = cc as u16;
                        let item_template = items[item_idx as usize].temp;
                        log::info!(
                            "Dropped {} (t={}) in Grave",
                            items[item_idx as usize].get_name(),
                            item_template,
                        );
                    }
                });
            } else {
                Repository::with_characters_mut(|characters| {
                    characters[cc].worn[n] = 0;
                });
            }
        }

        // Handle active spells - always destroy
        for n in 0..20 {
            let spell_idx = Repository::with_characters(|ch| ch[co].spell[n]);
            if spell_idx != 0 {
                Repository::with_characters_mut(|characters| {
                    characters[co].spell[n] = 0;
                    characters[cc].spell[n] = 0;
                });
                Repository::with_items_mut(|items| {
                    if (spell_idx as usize) < items.len() {
                        items[spell_idx as usize].used = USE_EMPTY;
                    }
                });
            }
        }
    }

    /// Apply permanent stat loss on death
    fn apply_death_penalties(&self, co: usize) {
        Repository::with_characters_mut(|characters| {
            // HP penalty
            let mut hp_tmp = characters[co].hp[0] / 10;
            if characters[co].hp[0] - hp_tmp < 50 {
                hp_tmp = characters[co].hp[0] - 50;
            }
            if hp_tmp > 0 {
                self.do_character_log(
                    co,
                    FontColor::Red,
                    &format!("You lost {} hitpoints permanently.\n", hp_tmp),
                );
                log::info!("Character {} lost {} permanent hitpoints.", co, hp_tmp);
                for _ in 0..hp_tmp {
                    // TODO: Implement do_lower_hp
                }
            } else {
                self.do_character_log(
                    co,
                    FontColor::Red,
                    "You would have lost permanent hitpoints, but you're already at the minimum.\n",
                );
            }

            // Mana penalty
            let mut mana_tmp = characters[co].mana[0] / 10;
            if characters[co].mana[0] - mana_tmp < 50 {
                mana_tmp = characters[co].mana[0] - 50;
            }
            if mana_tmp > 0 {
                self.do_character_log(
                    co,
                    FontColor::Red,
                    &format!("You lost {} mana permanently.\n", mana_tmp),
                );
                log::info!("Character {} lost {} permanent mana.", co, mana_tmp);
                for _ in 0..mana_tmp {
                    // TODO: Implement do_lower_mana
                }
            } else {
                self.do_character_log(
                    co,
                    FontColor::Red,
                    "You would have lost permanent mana, but you're already at the minimum.\n",
                );
            }
        });
    }

    pub fn do_notify_character(
        &self,
        character_id: u32,
        notify_type: i32,
        dat1: i32,
        dat2: i32,
        dat3: i32,
        dat4: i32,
    ) {
        Driver::msg(character_id, notify_type, dat1, dat2, dat3, dat4);
    }

    // use this one sparingly! It uses quite a bit of computation time!
    /* This routine finds the 3 closest NPCs to the one doing the shouting,
    so that they can come to the shouter's rescue or something. */
    pub fn do_npc_shout(
        &self,
        cn: usize,
        shout_type: i32,
        dat1: i32,
        dat2: i32,
        dat3: i32,
        dat4: i32,
    ) {
        Repository::with_characters(|characters| {
            let mut best: [i32; 3] = [99; 3];
            let mut bestn: [i32; 3] = [0; 3];

            if characters[cn].data[52] == 3 {
                for co in 1..core::constants::MAXCHARS {
                    if co != cn
                        && characters[co].used == core::constants::USE_ACTIVE
                        && characters[co].flags & CharacterFlags::CF_BODY.bits() == 0
                    {
                        if characters[co].flags
                            & (CharacterFlags::CF_PLAYER | CharacterFlags::CF_USURP).bits()
                            != 0
                        {
                            continue;
                        }

                        if characters[co].data[53] != characters[cn].data[52] {
                            continue;
                        }

                        // TODO: This distance calculation seems incorrect potentially -- doublecheck
                        let distance = (characters[cn].x as i32 - characters[co].x as i32).abs()
                            + (characters[cn].y as i32 - characters[co].y as i32).abs();

                        if distance < best[0] {
                            best[2] = best[1];
                            bestn[2] = bestn[1];
                            best[1] = best[0];
                            bestn[1] = bestn[0];
                            best[0] = distance;
                            bestn[0] = co as i32;
                        } else if distance < best[1] {
                            best[2] = best[1];
                            bestn[2] = bestn[1];
                            best[1] = distance;
                            bestn[1] = co as i32;
                        }
                        // } else if distance < best[3] {
                        //     // TODO: Pretty sure [3] isn't safe
                        //     best[3] = distance;
                        //     bestn[3] = co as i32;
                        // }
                    }
                }

                for i in 0..bestn.len() {
                    if bestn[i] != 0 {
                        self.do_notify_character(
                            bestn[i] as u32,
                            shout_type,
                            dat1,
                            dat2,
                            dat3,
                            dat4,
                        );
                    }
                }
            } else {
                for co in 1..core::constants::MAXCHARS {
                    if co != cn
                        && characters[co].used == core::constants::USE_ACTIVE
                        && characters[co].flags & CharacterFlags::CF_BODY.bits() == 0
                    {
                        if characters[co].flags
                            & (CharacterFlags::CF_PLAYER | CharacterFlags::CF_USURP).bits()
                            != 0
                        {
                            continue;
                        }

                        if characters[co].data[53] != characters[cn].data[52] {
                            continue;
                        }

                        self.do_notify_character(co as u32, shout_type, dat1, dat2, dat3, dat4);
                    }
                }
            }
        });
    }

    /// Sort character inventory based on order string
    /// Port of do_sort from svr_do.cpp
    pub fn do_sort(&self, cn: usize, order: &str) {
        // Check if character is in building mode
        let is_building = Repository::with_characters(|characters| characters[cn].is_building());

        if is_building {
            // TODO: Add do_char_log to send message to character
            log::info!("Character {} tried to sort while in build mode", cn);
            return;
        }

        // Get a copy of the items array to sort
        let mut items = Repository::with_characters(|characters| characters[cn].item);

        // Sort using custom comparison function based on order string
        items.sort_by(|&a, &b| self.qsort_compare(a as usize, b as usize, order));

        // Write sorted items back
        Repository::with_characters_mut(|characters| {
            characters[cn].item = items;
        });

        // Update character to send changes to client
        // TODO: Implement do_update_char equivalent
        // For now, this will at least sort the inventory in memory
        NetworkManager::with(|nm| {
            let player_id = Repository::with_characters(|characters| characters[cn].player);
            if player_id > 0 && player_id < MAXPLAYER as i32 {
                // TODO: Send character inventory update to client
            }
        });
    }

    /// Comparison function for sorting items
    /// Port of qsort_proc from svr_do.cpp
    fn qsort_compare(&self, in1: usize, in2: usize, order: &str) -> std::cmp::Ordering {
        use std::cmp::Ordering;

        // Handle empty slots - they go to the end
        if in1 == 0 && in2 == 0 {
            return Ordering::Equal;
        }
        if in1 != 0 && in2 == 0 {
            return Ordering::Less;
        }
        if in1 == 0 && in2 != 0 {
            return Ordering::Greater;
        }

        // Compare based on order string criteria
        Repository::with_items(|items| {
            let item1 = &items[in1];
            let item2 = &items[in2];

            for ch in order.chars() {
                match ch {
                    'w' => {
                        // Sort by weapon
                        let is_weapon1 =
                            item1.flags & core::constants::ItemFlags::IF_WEAPON.bits() != 0;
                        let is_weapon2 =
                            item2.flags & core::constants::ItemFlags::IF_WEAPON.bits() != 0;
                        if is_weapon1 && !is_weapon2 {
                            return Ordering::Less;
                        }
                        if !is_weapon1 && is_weapon2 {
                            return Ordering::Greater;
                        }
                    }
                    'a' => {
                        // Sort by armor
                        let is_armor1 =
                            item1.flags & core::constants::ItemFlags::IF_ARMOR.bits() != 0;
                        let is_armor2 =
                            item2.flags & core::constants::ItemFlags::IF_ARMOR.bits() != 0;
                        if is_armor1 && !is_armor2 {
                            return Ordering::Less;
                        }
                        if !is_armor1 && is_armor2 {
                            return Ordering::Greater;
                        }
                    }
                    'p' => {
                        // Sort by usable/consumable (use-destroy)
                        let is_usedestroy1 =
                            item1.flags & core::constants::ItemFlags::IF_USEDESTROY.bits() != 0;
                        let is_usedestroy2 =
                            item2.flags & core::constants::ItemFlags::IF_USEDESTROY.bits() != 0;
                        if is_usedestroy1 && !is_usedestroy2 {
                            return Ordering::Less;
                        }
                        if !is_usedestroy1 && is_usedestroy2 {
                            return Ordering::Greater;
                        }
                    }
                    'h' => {
                        // Sort by HP (higher first)
                        if item1.hp[0] > item2.hp[0] {
                            return Ordering::Less;
                        }
                        if item1.hp[0] < item2.hp[0] {
                            return Ordering::Greater;
                        }
                    }
                    'e' => {
                        // Sort by endurance (higher first)
                        if item1.end[0] > item2.end[0] {
                            return Ordering::Less;
                        }
                        if item1.end[0] < item2.end[0] {
                            return Ordering::Greater;
                        }
                    }
                    'm' => {
                        // Sort by mana (higher first)
                        if item1.mana[0] > item2.mana[0] {
                            return Ordering::Less;
                        }
                        if item1.mana[0] < item2.mana[0] {
                            return Ordering::Greater;
                        }
                    }
                    'v' => {
                        // Sort by value (higher first)
                        if item1.value > item2.value {
                            return Ordering::Less;
                        }
                        if item1.value < item2.value {
                            return Ordering::Greater;
                        }
                    }
                    _ => {
                        // Unknown character, skip
                    }
                }
            }

            // Fall back to sort by value
            if item1.value > item2.value {
                return Ordering::Less;
            }
            if item1.value < item2.value {
                return Ordering::Greater;
            }

            // Finally sort by temp (to maintain stability)
            if item1.temp > item2.temp {
                return Ordering::Greater;
            }
            if item1.temp < item2.temp {
                return Ordering::Less;
            }

            Ordering::Equal
        })
    }

    /// Port of `do_maygive(int cn, int co, int in)` from `svr_do.cpp`
    ///
    /// Checks if an item may be given from one character to another.
    /// This function validates whether an item can be transferred/dropped.
    ///
    /// # Arguments
    /// * `cn` - The giver character (not used in current implementation but kept for API compatibility)
    /// * `co` - The receiver character (not used in current implementation but kept for API compatibility)
    /// * `item_idx` - The index of the item to check
    ///
    /// # Returns
    /// * `true` - Item may be given/transferred
    /// * `false` - Item cannot be given (e.g., lag scrolls)
    pub fn do_maygive(&self, cn: usize, co: usize, item_idx: usize) -> bool {
        // Check if item index is valid
        if item_idx < 1 || item_idx >= core::constants::MAXITEM {
            return true; // Invalid items are considered "may give" (will be handled elsewhere)
        }

        // Check if item is a lag scroll - these cannot be given/dropped
        let is_lagscroll = Repository::with_items(|items| {
            if item_idx < items.len() {
                items[item_idx].temp == core::constants::IT_LAGSCROLL as u16
            } else {
                false
            }
        });

        if is_lagscroll {
            return false; // Lag scrolls cannot be given
        }

        true // All other items may be given
    }

    /// Port of `do_give(int cn, int co)` from `svr_do.cpp`
    ///
    /// Transfers the carried item (citem) from character cn to character co.
    /// Handles both gold and regular items.
    ///
    /// # Arguments
    /// * `cn` - The giver character
    /// * `co` - The receiver character
    ///
    /// # Returns
    /// * `true` - Item was successfully given
    /// * `false` - Failed to give item
    pub fn do_give(&self, cn: usize, co: usize) -> bool {
        // Check if giver has a carried item
        let citem = Repository::with_characters(|characters| characters[cn].citem);

        if citem == 0 {
            Repository::with_characters_mut(|characters| {
                characters[cn].cerrno = core::constants::ERR_FAILED as u16;
            });
            return false;
        }

        // Set success error code
        Repository::with_characters_mut(|characters| {
            characters[cn].cerrno = core::constants::ERR_SUCCESS as u16;
        });

        // Update both characters
        Repository::with_characters_mut(|characters| {
            characters[cn].set_do_update_flags();
            characters[co].set_do_update_flags();
        });

        // Check if citem is gold (high bit set)
        if (citem & 0x80000000) != 0 {
            let gold_amount = citem & 0x7FFFFFFF;

            // Transfer gold
            Repository::with_characters_mut(|characters| {
                characters[co].gold += gold_amount as i32;
                characters[cn].citem = 0;
            });

            // Log messages
            let (cn_name, co_name, cn_is_player) = Repository::with_characters(|characters| {
                (
                    characters[cn].get_name().to_string(),
                    characters[co].get_name().to_string(),
                    characters[cn].flags & CharacterFlags::CF_PLAYER.bits() != 0,
                )
            });

            self.do_character_log(
                cn,
                FontColor::Green,
                &format!("You give the gold to {}.\n", co_name),
            );
            self.do_character_log(
                co,
                FontColor::Yellow,
                &format!(
                    "You got {}G {}S from {}.\n",
                    gold_amount / 100,
                    gold_amount % 100,
                    cn_name
                ),
            );

            if cn_is_player {
                log::info!(
                    "Character {} gives {} ({}) {}G {}S",
                    cn,
                    co_name,
                    co,
                    gold_amount / 100,
                    gold_amount % 100
                );
            }

            // Notify receiver
            self.do_notify_character(
                co as u32,
                core::constants::NT_GIVE as i32,
                cn as i32,
                0,
                gold_amount as i32,
                0,
            );

            // Update giver
            Repository::with_characters_mut(|characters| {
                characters[cn].set_do_update_flags();
            });

            return true;
        }

        // Handle regular item
        let item_idx = citem as usize;

        // Check if item may be given
        if !self.do_maygive(cn, co, item_idx) {
            self.do_character_log(cn, FontColor::Red, "You're not allowed to do that!\n");
            Repository::with_characters_mut(|characters| {
                characters[cn].misc_action = core::constants::DR_IDLE as u16;
            });
            return false;
        }

        // Log the give action
        let (item_name, cn_name, co_name) = Repository::with_characters(|characters| {
            Repository::with_items(|items| {
                (
                    items[item_idx].get_name().to_string(),
                    characters[cn].get_name().to_string(),
                    characters[co].get_name().to_string(),
                )
            })
        });

        log::info!(
            "Character {} gives {} ({}) to {} ({})",
            cn,
            item_name,
            item_idx,
            co_name,
            co
        );

        // Special case: driver 31 (holy water) on undead
        let (is_holy_water, co_is_undead, cn_has_nomagic) =
            Repository::with_characters(|characters| {
                Repository::with_items(|items| {
                    (
                        items[item_idx].driver == 31,
                        characters[co].flags & CharacterFlags::CF_UNDEAD.bits() != 0,
                        characters[cn].flags & CharacterFlags::CF_NOMAGIC.bits() != 0,
                    )
                })
            });

        if is_holy_water && co_is_undead {
            if cn_has_nomagic {
                self.do_character_log(
                    cn,
                    FontColor::Red,
                    "It doesn't work! An evil aura is present.\n",
                );
                Repository::with_characters_mut(|characters| {
                    characters[cn].misc_action = core::constants::DR_IDLE as u16;
                });
                return false;
            }

            // Deal damage to undead
            let damage = Repository::with_items(|items| items[item_idx].data[0]);
            // TODO: Implement do_hurt
            log::info!("TODO: do_hurt({}, {}, {}, 2)", cn, co, damage);

            // Destroy the item
            Repository::with_items_mut(|items| {
                items[item_idx].used = core::constants::USE_EMPTY;
            });
            Repository::with_characters_mut(|characters| {
                characters[cn].citem = 0;
            });

            return true;
        }

        // Check for shop destroy flag
        let (co_is_player, has_shop_destroy) = Repository::with_characters(|characters| {
            Repository::with_items(|items| {
                (
                    characters[co].flags & CharacterFlags::CF_PLAYER.bits() != 0,
                    items[item_idx].flags & core::constants::ItemFlags::IF_SHOPDESTROY.bits() != 0,
                )
            })
        });

        if co_is_player && has_shop_destroy {
            self.do_character_log(
                cn,
                FontColor::Red,
                "Beware! The gods see what you're doing.\n",
            );
        }

        // Transfer the item
        let receiver_has_citem =
            Repository::with_characters(|characters| characters[co].citem != 0);

        if receiver_has_citem {
            // Receiver already has a carried item, try to put it in their inventory
            let success = God::give_character_item(co, item_idx);

            if success {
                Repository::with_characters_mut(|characters| {
                    characters[cn].citem = 0;
                });
                self.do_character_log(
                    cn,
                    FontColor::Green,
                    &format!("You give {} to {}.\n", item_name, co_name),
                );
            } else {
                Repository::with_characters_mut(|characters| {
                    characters[cn].misc_action = core::constants::DR_IDLE as u16;
                });
                return false;
            }
        } else {
            // Receiver doesn't have a carried item, give it directly
            Repository::with_characters_mut(|characters| {
                characters[cn].citem = 0;
                characters[co].citem = item_idx as u32;
            });

            Repository::with_items_mut(|items| {
                items[item_idx].carried = co as u16;
            });

            Repository::with_characters_mut(|characters| {
                characters[cn].set_do_update_flags();
            });

            self.do_character_log(
                cn,
                FontColor::Green,
                &format!("You give {} to {}.\n", item_name, co_name),
            );
        }

        // Notify receiver
        self.do_notify_character(
            co as u32,
            core::constants::NT_GIVE as i32,
            cn as i32,
            item_idx as i32,
            0,
            0,
        );

        true
    }

    /// Port of `really_update_char(int cn)` from `svr_do.cpp`
    ///
    /// Recalculates all character stats from base values, worn items, and active spells.
    /// This is the core stat calculation function that determines:
    /// - Final attributes (strength, agility, etc.)
    /// - HP, endurance, and mana totals
    /// - Skills with attribute bonuses
    /// - Armor, weapon, and gethit damage values
    /// - Light emission
    /// - Movement speed
    /// - Special flags (infrared, no regen flags)
    ///
    /// Called after equipment changes, spell effects, or any stat modifications.
    pub fn really_update_char(&mut self, cn: usize) {
        // Clear regeneration prevention flags and sprite override
        Repository::with_characters_mut(|characters| {
            characters[cn].flags &= !(CharacterFlags::CF_NOHPREG.bits()
                | CharacterFlags::CF_NOENDREG.bits()
                | CharacterFlags::CF_NOMANAREG.bits());
            characters[cn].sprite_override = 0;
        });

        // Check for NOMAGIC map flag
        let (char_x, char_y, wears_466, wears_481) = Repository::with_characters(|characters| {
            (
                characters[cn].x,
                characters[cn].y,
                self.char_wears_item(cn, 466),
                self.char_wears_item(cn, 481),
            )
        });

        let map_index = (char_x + char_y * core::constants::SERVER_MAPX as i16) as usize;
        let has_nomagic_flag = Repository::with_map(|map| {
            map[map_index].flags & core::constants::MF_NOMAGIC as u64 != 0
        });

        if has_nomagic_flag && !wears_466 && !wears_481 {
            let already_has_nomagic = Repository::with_characters(|ch| {
                ch[cn].flags & CharacterFlags::CF_NOMAGIC.bits() != 0
            });

            if !already_has_nomagic {
                Repository::with_characters_mut(|characters| {
                    characters[cn].flags |= CharacterFlags::CF_NOMAGIC.bits();
                });
                self.remove_spells(cn);
                self.do_character_log(cn, FontColor::Green, "You feel your magic fail.\n");
            }
        } else {
            let has_nomagic = Repository::with_characters(|ch| {
                ch[cn].flags & CharacterFlags::CF_NOMAGIC.bits() != 0
            });

            if has_nomagic {
                Repository::with_characters_mut(|characters| {
                    characters[cn].flags &= !CharacterFlags::CF_NOMAGIC.bits();
                    characters[cn].set_do_update_flags();
                });
                self.do_character_log(cn, FontColor::Green, "You feel your magic return.\n");
            }
        }

        let old_light = Repository::with_characters(|characters| characters[cn].light);

        // Initialize stat accumulators
        let mut attrib_bonus = [0i32; 5];
        let mut hp_bonus = 0i32;
        let mut end_bonus = 0i32;
        let mut mana_bonus = 0i32;
        let mut skill_bonus = [0i32; 50];
        let mut armor = 0i32;
        let mut weapon = 0i32;
        let mut gethit = 0i32;
        let mut light = 0i32;
        let mut sublight = 0i32;
        let mut infra = 0u8;

        // Reset temp bonuses in character
        Repository::with_characters_mut(|characters| {
            for n in 0..5 {
                characters[cn].attrib[n][4] = 0;
            }
            characters[cn].hp[4] = 0;
            characters[cn].end[4] = 0;
            characters[cn].mana[4] = 0;
            for n in 0..50 {
                characters[cn].skill[n][4] = 0;
            }
            characters[cn].armor = 0;
            characters[cn].weapon = 0;
            characters[cn].gethit_dam = 0;
            characters[cn].stunned = 0;
            characters[cn].light = 0;
        });

        let char_has_nomagic =
            Repository::with_characters(|ch| ch[cn].flags & CharacterFlags::CF_NOMAGIC.bits() != 0);

        // Calculate bonuses from worn items
        for n in 0..20 {
            let item_idx = Repository::with_characters(|ch| ch[cn].worn[n]);
            if item_idx == 0 {
                continue;
            }

            Repository::with_items(|items| {
                let item = &items[item_idx as usize];

                if !char_has_nomagic {
                    // Add magical bonuses
                    for z in 0..5 {
                        attrib_bonus[z] += if item.active != 0 {
                            item.attrib[z][1] as i32
                        } else {
                            item.attrib[z][0] as i32
                        };
                    }

                    hp_bonus += if item.active != 0 {
                        item.hp[1] as i32
                    } else {
                        item.hp[0] as i32
                    };

                    end_bonus += if item.active != 0 {
                        item.end[1] as i32
                    } else {
                        item.end[0] as i32
                    };

                    mana_bonus += if item.active != 0 {
                        item.mana[1] as i32
                    } else {
                        item.mana[0] as i32
                    };

                    for z in 0..50 {
                        skill_bonus[z] += if item.active != 0 {
                            item.skill[z][1] as i32
                        } else {
                            item.skill[z][0] as i32
                        };
                    }
                }

                // Add physical bonuses (always apply)
                if item.active != 0 {
                    armor += item.armor[1] as i32;
                    gethit += item.gethit_dam[1] as i32;
                    if item.weapon[1] as i32 > weapon {
                        weapon = item.weapon[1] as i32;
                    }
                    if item.light[1] as i32 > light {
                        light = item.light[1] as i32;
                    } else if item.light[1] < 0 {
                        sublight -= item.light[1] as i32;
                    }
                } else {
                    armor += item.armor[0] as i32;
                    gethit += item.gethit_dam[0] as i32;
                    if item.weapon[0] as i32 > weapon {
                        weapon = item.weapon[0] as i32;
                    }
                    if item.light[0] as i32 > light {
                        light = item.light[0] as i32;
                    } else if item.light[0] < 0 {
                        sublight -= item.light[0] as i32;
                    }
                }
            });
        }

        // Add permanent bonuses
        Repository::with_characters(|characters| {
            armor += characters[cn].armor_bonus as i32;
            weapon += characters[cn].weapon_bonus as i32;
            gethit += characters[cn].gethit_bonus as i32;
            light += characters[cn].light_bonus as i32;
        });

        // Calculate bonuses from active spells
        if !char_has_nomagic {
            for n in 0..20 {
                let spell_idx = Repository::with_characters(|ch| ch[cn].spell[n]);
                if spell_idx == 0 {
                    continue;
                }

                Repository::with_items(|items| {
                    let spell = &items[spell_idx as usize];

                    for z in 0..5 {
                        attrib_bonus[z] += spell.attrib[z][1] as i32;
                    }

                    hp_bonus += spell.hp[1] as i32;
                    end_bonus += spell.end[1] as i32;
                    mana_bonus += spell.mana[1] as i32;

                    for z in 0..50 {
                        skill_bonus[z] += spell.skill[z][1] as i32;
                    }

                    armor += spell.armor[1] as i32;
                    weapon += spell.weapon[1] as i32;
                    if spell.light[1] as i32 > light {
                        light = spell.light[1] as i32;
                    } else if spell.light[1] < 0 {
                        sublight -= spell.light[1] as i32;
                    }

                    // Check for special spell effects
                    if spell.temp == core::constants::SK_STUN as u16 || spell.temp == 59 {
                        // SK_WARCRY2 = 59
                        Repository::with_characters_mut(|characters| {
                            characters[cn].stunned = 1;
                        });
                    }

                    if spell.hp[0] < 0 {
                        Repository::with_characters_mut(|characters| {
                            characters[cn].flags |= CharacterFlags::CF_NOHPREG.bits();
                        });
                    }
                    if spell.end[0] < 0 {
                        Repository::with_characters_mut(|characters| {
                            characters[cn].flags |= CharacterFlags::CF_NOENDREG.bits();
                        });
                    }
                    if spell.mana[0] < 0 {
                        Repository::with_characters_mut(|characters| {
                            characters[cn].flags |= CharacterFlags::CF_NOMANAREG.bits();
                        });
                    }

                    if spell.sprite_override != 0 {
                        Repository::with_characters_mut(|characters| {
                            characters[cn].sprite_override = spell.sprite_override as i16;
                        });
                    }

                    // Check for infrared vision components (items 635, 637, 639, 641)
                    if spell.temp == 635 {
                        infra |= 1;
                    }
                    if spell.temp == 637 {
                        infra |= 2;
                    }
                    if spell.temp == 639 {
                        infra |= 4;
                    }
                    if spell.temp == 641 {
                        infra |= 8;
                    }
                });
            }
        }

        // Calculate final attributes
        Repository::with_characters_mut(|characters| {
            for z in 0..5 {
                let mut final_attrib = characters[cn].attrib[z][0] as i32
                    + characters[cn].attrib[z][1] as i32
                    + attrib_bonus[z];
                if final_attrib < 1 {
                    final_attrib = 1;
                }
                if final_attrib > 250 {
                    final_attrib = 250;
                }
                characters[cn].attrib[z][5] = final_attrib as u8;
            }

            // Calculate final HP
            let mut final_hp = characters[cn].hp[0] as i32 + characters[cn].hp[1] as i32 + hp_bonus;
            if final_hp < 10 {
                final_hp = 10;
            }
            if final_hp > 999 {
                final_hp = 999;
            }
            characters[cn].hp[5] = final_hp as u16;

            // Calculate final endurance
            let mut final_end =
                characters[cn].end[0] as i32 + characters[cn].end[1] as i32 + end_bonus;
            if final_end < 10 {
                final_end = 10;
            }
            if final_end > 999 {
                final_end = 999;
            }
            characters[cn].end[5] = final_end as u16;

            // Calculate final mana
            let mut final_mana =
                characters[cn].mana[0] as i32 + characters[cn].mana[1] as i32 + mana_bonus;
            if final_mana < 10 {
                final_mana = 10;
            }
            if final_mana > 999 {
                final_mana = 999;
            }
            characters[cn].mana[5] = final_mana as u16;
        });

        // Handle infrared vision
        let is_player =
            Repository::with_characters(|ch| ch[cn].flags & CharacterFlags::CF_PLAYER.bits() != 0);

        if is_player {
            let has_infrared = Repository::with_characters(|ch| {
                ch[cn].flags & CharacterFlags::CF_INFRARED.bits() != 0
            });

            if infra == 15 && !has_infrared {
                Repository::with_characters_mut(|characters| {
                    characters[cn].flags |= CharacterFlags::CF_INFRARED.bits();
                });
                self.do_character_log(cn, FontColor::Green, "You can see in the dark!\n");
            } else if infra != 15 && has_infrared {
                let is_god = Repository::with_characters(|ch| {
                    ch[cn].flags & CharacterFlags::CF_GOD.bits() != 0
                });

                if !is_god {
                    Repository::with_characters_mut(|characters| {
                        characters[cn].flags &= !CharacterFlags::CF_INFRARED.bits();
                    });
                    self.do_character_log(
                        cn,
                        FontColor::Red,
                        "You can no longer see in the dark!\n",
                    );
                }
            }
        }

        // Calculate final skills (with attribute bonuses)
        // TODO: Need access to static_skilltab to properly calculate skill bonuses
        Repository::with_characters_mut(|characters| {
            for z in 0..50 {
                let mut final_skill = characters[cn].skill[z][0] as i32
                    + characters[cn].skill[z][1] as i32
                    + skill_bonus[z];

                // Add attribute bonuses (simplified - real implementation needs skilltab)
                // For now, just add a generic attribute bonus
                let attrib_contribution = (characters[cn].attrib[core::constants::AT_AGIL as usize]
                    [5] as i32
                    + characters[cn].attrib[core::constants::AT_STREN as usize][5] as i32
                    + characters[cn].attrib[core::constants::AT_INT as usize][5] as i32)
                    / 5;
                final_skill += attrib_contribution;

                if final_skill < 1 {
                    final_skill = 1;
                }
                if final_skill > 250 {
                    final_skill = 250;
                }
                characters[cn].skill[z][5] = final_skill as u8;
            }

            // Set final armor
            if armor < 0 {
                armor = 0;
            }
            if armor > 250 {
                armor = 250;
            }
            characters[cn].armor = armor as i16;

            // Set final weapon
            if weapon < 0 {
                weapon = 0;
            }
            if weapon > 250 {
                weapon = 250;
            }
            characters[cn].weapon = weapon as i16;

            // Set final gethit damage
            if gethit < 0 {
                gethit = 0;
            }
            if gethit > 250 {
                gethit = 250;
            }
            characters[cn].gethit_dam = gethit as i8;

            // Set final light
            light -= sublight;
            if light < 0 {
                light = 0;
            }
            if light > 250 {
                light = 250;
            }
            characters[cn].light = light as u8;

            // Calculate speed based on mode
            let mut speed_calc = 10i32;
            let mode = characters[cn].mode;
            let agil = characters[cn].attrib[core::constants::AT_AGIL as usize][5] as i32;
            let stren = characters[cn].attrib[core::constants::AT_STREN as usize][5] as i32;
            let speed_mod = characters[cn].speed_mod as i32;

            if mode == 0 {
                // Sneak mode
                speed_calc = (agil + stren) / 50 + speed_mod + 12;
            } else if mode == 1 {
                // Normal mode
                speed_calc = (agil + stren) / 50 + speed_mod + 14;
            } else if mode == 2 {
                // Fast mode
                speed_calc = (agil + stren) / 50 + speed_mod + 16;
            }

            characters[cn].speed = 20 - speed_calc as i16;
            if characters[cn].speed < 0 {
                characters[cn].speed = 0;
            }
            if characters[cn].speed > 19 {
                characters[cn].speed = 19;
            }

            // Cap current stats at their maximums
            if characters[cn].a_hp > characters[cn].hp[5] as i32 * 1000 {
                characters[cn].a_hp = characters[cn].hp[5] as i32 * 1000;
            }
            if characters[cn].a_end > characters[cn].end[5] as i32 * 1000 {
                characters[cn].a_end = characters[cn].end[5] as i32 * 1000;
            }
            if characters[cn].a_mana > characters[cn].mana[5] as i32 * 1000 {
                characters[cn].a_mana = characters[cn].mana[5] as i32 * 1000;
            }
        });

        // Update light if it changed
        let new_light = Repository::with_characters(|ch| ch[cn].light);
        if old_light != new_light {
            let (used, x, y) = Repository::with_characters(|ch| (ch[cn].used, ch[cn].x, ch[cn].y));

            if used == core::constants::USE_ACTIVE
                && x > 0
                && x < core::constants::SERVER_MAPX as i16
                && y > 0
                && y < core::constants::SERVER_MAPY as i16
            {
                let map_char = Repository::with_map(|map| {
                    let idx = (x as i32 + y as i32 * core::constants::SERVER_MAPX) as usize;
                    map[idx].ch
                });

                if map_char == cn as u32 {
                    self.do_add_light(x as i32, y as i32, new_light as i32 - old_light as i32);
                }
            }
        }
    }

    /// Helper function to check if character wears a specific item
    pub fn char_wears_item(&self, cn: usize, item_template: u16) -> bool {
        Repository::with_characters(|characters| {
            for n in 0..20 {
                let item_idx = characters[cn].worn[n];
                if item_idx != 0 {
                    let matches = Repository::with_items(|items| {
                        items[item_idx as usize].temp == item_template
                    });
                    if matches {
                        return true;
                    }
                }
            }
            false
        })
    }

    /// Port of `do_regenerate(int cn)` from `svr_do.cpp`
    ///
    /// Handles HP/endurance/mana regeneration based on character status, skills, and spells.
    /// Also manages spell effects, underwater damage, and endurance drain from movement/combat.
    /// Called every tick for active characters.
    pub fn do_regenerate(&self, cn: usize) {
        // Check if character is stoned - no regeneration if stoned
        let is_stoned =
            Repository::with_characters(|ch| ch[cn].flags & CharacterFlags::CF_STONED.bits() != 0);

        if is_stoned {
            return;
        }

        // Determine moon multiplier for regen rates
        let mut moonmult = 20;

        let (is_player, globs_flags, newmoon, fullmoon) = Repository::with_globals(|globs| {
            let char_is_player = Repository::with_characters(|ch| {
                ch[cn].flags & CharacterFlags::CF_PLAYER.bits() != 0
            });
            (
                char_is_player,
                globs.flags,
                globs.newmoon != 0,
                globs.fullmoon != 0,
            )
        });

        if ((globs_flags & core::constants::GF_MAYHEM != 0) || newmoon) && is_player {
            moonmult = 10; // Slower regen during mayhem or new moon
        }
        if fullmoon && is_player {
            moonmult = 40; // Faster regen during full moon
        }

        // Check for regeneration prevention flags
        let (nohp, noend, nomana) = Repository::with_characters(|ch| {
            let nohp = ch[cn].flags & CharacterFlags::CF_NOHPREG.bits() != 0;
            let noend = ch[cn].flags & CharacterFlags::CF_NOENDREG.bits() != 0;
            let nomana = ch[cn].flags & CharacterFlags::CF_NOMANAREG.bits() != 0;
            (nohp, noend, nomana)
        });

        // Check if standing in underwater tile
        let uwater = Repository::with_characters(|ch| {
            let x = ch[cn].x as usize;
            let y = ch[cn].y as usize;
            let map_idx = x + y * core::constants::SERVER_MAPX as usize;

            Repository::with_map(|map| map[map_idx].flags & core::constants::MF_UWATER as u64 != 0)
        });

        let mut uwater_active = uwater;
        let mut hp_regen = false;
        let mut mana_regen = false;
        let mut gothp = 0i32;

        // Process regeneration based on character status (if not stunned)
        let stunned = Repository::with_characters(|ch| ch[cn].stunned != 0);

        if !stunned {
            let status = Repository::with_characters(|ch| ch[cn].status);
            let base_status = Self::ch_base_status(status as u8);

            match base_status {
                // Standing/idle states - regenerate normally
                0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 => {
                    if !noend {
                        Repository::with_characters_mut(|ch| {
                            ch[cn].a_end += moonmult * 4;

                            // Add bonus from Rest skill
                            if ch[cn].skill[core::constants::SK_REST][0] != 0 {
                                ch[cn].a_end += ch[cn].skill[core::constants::SK_REST][5] as i32
                                    * moonmult
                                    / 30;
                            }
                        });
                    }

                    if !nohp {
                        hp_regen = true;
                        Repository::with_characters_mut(|ch| {
                            ch[cn].a_hp += moonmult * 2;
                            gothp += moonmult * 2;

                            // Add bonus from Regen skill
                            if ch[cn].skill[core::constants::SK_REGEN][0] != 0 {
                                let regen_bonus = ch[cn].skill[core::constants::SK_REGEN][5] as i32
                                    * moonmult
                                    / 30;
                                ch[cn].a_hp += regen_bonus;
                                gothp += regen_bonus;
                            }
                        });
                    }

                    if !nomana {
                        let has_medit = Repository::with_characters(|ch| {
                            ch[cn].skill[core::constants::SK_MEDIT][0] != 0
                        });

                        if has_medit {
                            mana_regen = true;
                            Repository::with_characters_mut(|ch| {
                                ch[cn].a_mana += moonmult;
                                ch[cn].a_mana += ch[cn].skill[core::constants::SK_MEDIT][5] as i32
                                    * moonmult
                                    / 30;
                            });
                        }
                    }
                }

                // Walking/turning states - endurance based on mode
                16 | 24 | 32 | 40 | 48 | 60 | 72 | 84 | 96 | 100 | 104 | 108 | 112 | 116 | 120
                | 124 | 128 | 132 | 136 | 140 | 144 | 148 | 152 => {
                    let mode = Repository::with_characters(|ch| ch[cn].mode);

                    if mode == 2 {
                        // Fast mode drains endurance
                        Repository::with_characters_mut(|ch| {
                            ch[cn].a_end -= 25;
                        });
                    } else if mode == 0 {
                        // Sneak mode regenerates endurance
                        if !noend {
                            Repository::with_characters_mut(|ch| {
                                ch[cn].a_end += 25;
                            });
                        }
                    }
                }

                // Attack states - endurance drain based on status2 and mode
                160 | 168 | 176 | 184 => {
                    let (status2, mode) =
                        Repository::with_characters(|ch| (ch[cn].status2, ch[cn].mode));

                    if status2 == 0 || status2 == 5 || status2 == 6 {
                        // Attack action
                        if mode == 1 {
                            Repository::with_characters_mut(|ch| {
                                ch[cn].a_end -= 12;
                            });
                        } else if mode == 2 {
                            Repository::with_characters_mut(|ch| {
                                ch[cn].a_end -= 50;
                            });
                        }
                    } else {
                        // Misc action
                        if mode == 2 {
                            Repository::with_characters_mut(|ch| {
                                ch[cn].a_end -= 25;
                            });
                        } else if mode == 0 {
                            if !noend {
                                Repository::with_characters_mut(|ch| {
                                    ch[cn].a_end += 25;
                                });
                            }
                        }
                    }
                }

                _ => {
                    log::warn!("do_regenerate(): unknown ch_base_status {}.", base_status);
                }
            }
        }

        // Undead characters get bonus HP regeneration
        let is_undead =
            Repository::with_characters(|ch| ch[cn].flags & CharacterFlags::CF_UNDEAD.bits() != 0);

        if is_undead {
            hp_regen = true;
            Repository::with_characters_mut(|ch| {
                ch[cn].a_hp += 650;
            });
            gothp += 650;
        }

        // Amulet of Ankh (item 768) provides additional regeneration
        let worn_neck = Repository::with_characters(|ch| ch[cn].worn[core::constants::WN_NECK]);
        if worn_neck != 0 {
            let is_ankh = Repository::with_items(|items| items[worn_neck as usize].temp == 768);

            if is_ankh {
                let (has_regen, has_rest, has_medit) = Repository::with_characters(|ch| {
                    (
                        ch[cn].skill[core::constants::SK_REGEN][0] != 0,
                        ch[cn].skill[core::constants::SK_REST][0] != 0,
                        ch[cn].skill[core::constants::SK_MEDIT][0] != 0,
                    )
                });

                Repository::with_characters_mut(|ch| {
                    if has_regen {
                        ch[cn].a_hp +=
                            ch[cn].skill[core::constants::SK_REGEN][5] as i32 * moonmult / 60;
                    }
                    if has_rest {
                        ch[cn].a_end +=
                            ch[cn].skill[core::constants::SK_REST][5] as i32 * moonmult / 60;
                    }
                    if has_medit {
                        ch[cn].a_mana +=
                            ch[cn].skill[core::constants::SK_MEDIT][5] as i32 * moonmult / 60;
                    }
                });
            }
        }

        // Cap accumulated stats at their maximums (max * 1000)
        Repository::with_characters_mut(|ch| {
            if ch[cn].a_hp > ch[cn].hp[5] as i32 * 1000 {
                ch[cn].a_hp = ch[cn].hp[5] as i32 * 1000;
            }
            if ch[cn].a_end > ch[cn].end[5] as i32 * 1000 {
                ch[cn].a_end = ch[cn].end[5] as i32 * 1000;
            }
            if ch[cn].a_mana > ch[cn].mana[5] as i32 * 1000 {
                ch[cn].a_mana = ch[cn].mana[5] as i32 * 1000;
            }
        });

        // Set timer when regenerating below 90% of max
        if hp_regen {
            let needs_timer =
                Repository::with_characters(|ch| ch[cn].a_hp < ch[cn].hp[5] as i32 * 900);
            if needs_timer {
                Repository::with_characters_mut(|ch| {
                    ch[cn].data[92] = core::constants::TICKS * 60;
                });
            }
        }

        if mana_regen {
            let needs_timer =
                Repository::with_characters(|ch| ch[cn].a_mana < ch[cn].mana[5] as i32 * 900);
            if needs_timer {
                Repository::with_characters_mut(|ch| {
                    ch[cn].data[92] = core::constants::TICKS * 60;
                });
            }
        }

        // Force to sneak mode if exhausted
        let (is_exhausted, mode) =
            Repository::with_characters(|ch| (ch[cn].a_end < 1500, ch[cn].mode));

        if is_exhausted && mode != 0 {
            Repository::with_characters_mut(|ch| {
                ch[cn].mode = 0;
                ch[cn].set_do_update_flags();
            });

            self.do_character_log(cn, FontColor::Red, "You're exhausted.\n");
        }

        // Decrement escape timer
        Repository::with_characters_mut(|ch| {
            if ch[cn].escape_timer > 0 {
                ch[cn].escape_timer -= 1;
            }
        });

        // Process spell effects
        for spell_slot in 0..20 {
            let spell_item = Repository::with_characters(|ch| ch[cn].spell[spell_slot]);

            if spell_item == 0 {
                continue;
            }

            let is_permspell = Repository::with_items(|items| {
                items[spell_item as usize].flags & ItemFlags::IF_PERMSPELL.bits() != 0
            });

            if is_permspell {
                // Permanent spell - apply ongoing HP/end/mana drain/gain
                let (hp_change, end_change, mana_change) = Repository::with_items(|items| {
                    (
                        items[spell_item as usize].hp[0],
                        items[spell_item as usize].end[0],
                        items[spell_item as usize].mana[0],
                    )
                });

                let mut killed = false;
                let mut end_depleted = false;
                let mut mana_depleted = false;

                Repository::with_characters_mut(|ch| {
                    if hp_change != -1 {
                        ch[cn].a_hp += hp_change as i32;
                        if ch[cn].a_hp < 500 {
                            killed = true;
                        }
                    }
                    if end_change != -1 {
                        ch[cn].a_end += end_change as i32;
                        if ch[cn].a_end < 500 {
                            ch[cn].a_end = 500;
                            end_depleted = true;
                        }
                    }
                    if mana_change != -1 {
                        ch[cn].a_mana += mana_change as i32;
                        if ch[cn].a_mana < 500 {
                            ch[cn].a_mana = 500;
                            mana_depleted = true;
                        }
                    }
                });

                if killed {
                    let spell_name =
                        Repository::with_items(|items| items[spell_item as usize].name.clone());
                    log::info!(
                        "Character {} killed by spell: {}",
                        cn,
                        String::from_utf8_lossy(&spell_name)
                    );
                    self.do_character_log(
                        cn,
                        FontColor::Red,
                        &format!("The {} killed you!\n", String::from_utf8_lossy(&spell_name)),
                    );
                    self.do_area_log(
                        cn,
                        0,
                        0,
                        0,
                        FontColor::Red,
                        &format!(
                            "The {} killed {}.\n",
                            String::from_utf8_lossy(&spell_name),
                            cn
                        ),
                    );
                    self.do_character_killed(0, cn);
                    return;
                }

                if end_depleted {
                    let spell_name =
                        Repository::with_items(|items| items[spell_item as usize].name.clone());
                    Repository::with_items_mut(|items| {
                        items[spell_item as usize].active = 0;
                    });
                    log::info!(
                        "{} ran out due to lack of endurance for cn={}",
                        String::from_utf8_lossy(&spell_name),
                        cn
                    );
                }

                if mana_depleted {
                    let spell_name =
                        Repository::with_items(|items| items[spell_item as usize].name.clone());
                    Repository::with_items_mut(|items| {
                        items[spell_item as usize].active = 0;
                    });
                    log::info!(
                        "{} ran out due to lack of mana for cn={}",
                        String::from_utf8_lossy(&spell_name),
                        cn
                    );
                }
            } else {
                // Temporary spell - decrement timer
                Repository::with_items_mut(|items| {
                    items[spell_item as usize].active -= 1;
                });

                let active = Repository::with_items(|items| items[spell_item as usize].active);

                // Warn when spell is about to run out
                if active == core::constants::TICKS as u32 * 30 {
                    let spell_name =
                        Repository::with_items(|items| items[spell_item as usize].name.clone());
                    let (is_player_or_usurp, temp, companion_owner) =
                        Repository::with_characters(|ch| {
                            (
                                ch[cn].flags
                                    & (CharacterFlags::CF_PLAYER.bits()
                                        | CharacterFlags::CF_USURP.bits())
                                    != 0,
                                ch[cn].temp,
                                ch[cn].data[63],
                            )
                        });

                    if is_player_or_usurp {
                        self.do_character_log(
                            cn,
                            FontColor::Red,
                            &format!(
                                "{} is about to run out.\n",
                                String::from_utf8_lossy(&spell_name)
                            ),
                        );
                    } else if temp == core::constants::CT_COMPANION as u16 && companion_owner != 0 {
                        let co = companion_owner as usize;
                        if co > 0 && co < MAXCHARS {
                            let is_sane_player = Repository::with_characters(|ch| {
                                ch[co].used == core::constants::USE_ACTIVE
                                    && ch[co].flags & CharacterFlags::CF_PLAYER.bits() != 0
                            });

                            if is_sane_player {
                                let item_temp =
                                    Repository::with_items(|items| items[spell_item as usize].temp);

                                // Only inform owner about certain spell types
                                if item_temp == core::constants::SK_BLESS as u16
                                    || item_temp == core::constants::SK_PROTECT as u16
                                    || item_temp == core::constants::SK_ENHANCE as u16
                                {
                                    let cn_name =
                                        Repository::with_characters(|ch| ch[cn].name.clone());
                                    let co_name =
                                        Repository::with_characters(|ch| ch[co].name.clone());

                                    self.do_sayx(
                                        cn,
                                        format!(
                                            "My spell {} is running out, {}.",
                                            String::from_utf8_lossy(&spell_name),
                                            String::from_utf8_lossy(&co_name),
                                        )
                                        .as_str(),
                                    );
                                }
                            }
                        }
                    }
                }

                // Check item temp for special handling
                let item_temp = Repository::with_items(|items| items[spell_item as usize].temp);

                // Water breathing spell cancels underwater damage
                if item_temp == 649 {
                    uwater_active = false;
                }

                // Magic Shield spell - update armor based on remaining duration
                if item_temp == core::constants::SK_MSHIELD as u16 {
                    let old_armor =
                        Repository::with_items(|items| items[spell_item as usize].armor[1]);
                    let new_armor = active / 1024 + 1;
                    let new_power = active / 256;

                    Repository::with_items_mut(|items| {
                        items[spell_item as usize].armor[1] = new_armor as i8;
                        items[spell_item as usize].power = new_power as u32;
                    });

                    if old_armor != new_armor as i8 {
                        Repository::with_characters_mut(|ch| {
                            ch[cn].set_do_update_flags();
                        });
                    }
                }

                // Handle spell expiration
                if active == 0 {
                    let spell_name =
                        Repository::with_items(|items| items[spell_item as usize].name.clone());

                    // Recall spell - teleport character
                    if item_temp == core::constants::SK_RECALL as u16 {
                        let char_used = Repository::with_characters(|ch| ch[cn].used);

                        if char_used == core::constants::USE_ACTIVE {
                            let (old_x, old_y, dest_x, dest_y, is_invisible) =
                                Repository::with_characters(|ch| {
                                    let dest = Repository::with_items(|items| {
                                        (
                                            items[spell_item as usize].data[0],
                                            items[spell_item as usize].data[1],
                                        )
                                    });
                                    (
                                        ch[cn].x,
                                        ch[cn].y,
                                        dest.0,
                                        dest.1,
                                        ch[cn].flags & CharacterFlags::CF_INVISIBLE.bits() != 0,
                                    )
                                });

                            if God::transfer_char(cn, dest_x as usize, dest_y as usize) {
                                if !is_invisible {
                                    // TODO: Implement fx_add_effect
                                    log::info!(
                                        "TODO: fx_add_effect(12, 0, {}, {}, 0)",
                                        old_x,
                                        old_y
                                    );
                                    log::info!(
                                        "TODO: fx_add_effect(12, 0, {}, {}, 0)",
                                        dest_x,
                                        dest_y
                                    );
                                }
                            }

                            // Reset character state
                            Repository::with_characters_mut(|ch| {
                                ch[cn].status = 0;
                                ch[cn].attack_cn = 0;
                                ch[cn].skill_nr = 0;
                                ch[cn].goto_x = 0;
                                ch[cn].use_nr = 0;
                                ch[cn].misc_action = 0;
                                ch[cn].dir = core::constants::DX_DOWN;
                            });
                        }
                    } else {
                        self.do_character_log(
                            cn,
                            FontColor::Red,
                            &format!("{} ran out.\n", String::from_utf8_lossy(&spell_name)),
                        );
                    }

                    // Remove spell
                    Repository::with_items_mut(|items| {
                        items[spell_item as usize].used = core::constants::USE_EMPTY;
                    });
                    Repository::with_characters_mut(|ch| {
                        ch[cn].spell[spell_slot] = 0;
                        ch[cn].set_do_update_flags();
                    });
                }
            }
        }

        // Handle underwater damage for players
        if uwater_active {
            let (is_player, is_immortal) = Repository::with_characters(|ch| {
                (
                    ch[cn].flags & CharacterFlags::CF_PLAYER.bits() != 0,
                    ch[cn].flags & CharacterFlags::CF_IMMORTAL.bits() != 0,
                )
            });

            if is_player && !is_immortal {
                Repository::with_characters_mut(|ch| {
                    ch[cn].a_hp -= 250 + gothp;
                });

                let is_dead = Repository::with_characters(|ch| ch[cn].a_hp < 500);
                if is_dead {
                    self.do_character_killed(0, cn);
                }
            }
        }

        // Handle item tear and wear for active players
        let (used, is_player) = Repository::with_characters(|ch| {
            (
                ch[cn].used,
                ch[cn].flags & CharacterFlags::CF_PLAYER.bits() != 0,
            )
        });

        if used == core::constants::USE_ACTIVE && is_player {
            // TODO: Implement char_item_expire
            log::info!("TODO: Call char_item_expire for cn={}", cn);
        }
    }

    /// Helper function to determine base status from full status value
    /// Port of ch_base_status from svr_tick.cpp
    pub fn ch_base_status(n: u8) -> u8 {
        if n < 4 {
            return n;
        }
        if n < 16 {
            return n;
        }
        if n < 24 {
            return 16;
        } // walk up
        if n < 32 {
            return 24;
        } // walk down
        if n < 40 {
            return 32;
        } // walk left
        if n < 48 {
            return 40;
        } // walk right
        if n < 60 {
            return 48;
        } // walk left+up
        if n < 72 {
            return 60;
        } // walk left+down
        if n < 84 {
            return 72;
        } // walk right+up
        if n < 96 {
            return 84;
        } // walk right+down
        if n < 100 {
            return 96;
        }
        if n < 104 {
            return 100;
        } // turn up to left
        if n < 108 {
            return 104;
        } // turn up to right
        if n < 112 {
            return 108;
        }
        if n < 116 {
            return 112;
        }
        if n < 120 {
            return 116;
        } // turn down to left
        if n < 124 {
            return 120;
        }
        if n < 128 {
            return 124;
        } // turn down to right
        if n < 132 {
            return 128;
        }
        if n < 136 {
            return 132;
        } // turn left to up
        if n < 140 {
            return 136;
        }
        if n < 144 {
            return 140;
        } // turn left to down
        if n < 148 {
            return 144;
        }
        if n < 152 {
            return 148;
        } // turn right to up
        if n < 156 {
            return 152;
        }
        if n < 160 {
            return 160;
        } // turn right to down
        if n < 164 {
            return 160;
        }
        if n < 168 {
            return 160;
        } // attack up
        if n < 176 {
            return 168;
        } // attack down
        if n < 184 {
            return 176;
        } // attack left
        if n < 192 {
            return 184;
        } // attack right

        n // default
    }

    /// Set the update/save flags for a character (port of `do_update_char`)
    pub fn do_update_char(&self, cn: usize) {
        Repository::with_characters_mut(|characters| {
            characters[cn].set_do_update_flags();
        });
    }

    /// Remove all active spell items from a character (port of `remove_spells`)
    pub fn remove_spells(&self, cn: usize) {
        for n in 0..20 {
            let spell_item = Repository::with_characters(|ch| ch[cn].spell[n]);
            if spell_item == 0 {
                continue;
            }
            let in_idx = spell_item as usize;
            Repository::with_items_mut(|items| {
                if in_idx < items.len() {
                    items[in_idx].used = core::constants::USE_EMPTY;
                }
            });
            Repository::with_characters_mut(|ch| {
                ch[cn].spell[n] = 0;
            });
        }
        self.do_update_char(cn);
    }

    /// Reset player status based on facing direction (port of `plr_reset_status`)
    pub fn plr_reset_status(&self, cn: usize) {
        use core::constants::*;
        Repository::with_characters_mut(|ch| {
            ch[cn].status = match ch[cn].dir {
                DX_UP => 0,
                DX_DOWN => 1,
                DX_LEFT => 2,
                DX_RIGHT => 3,
                DX_LEFTUP => 4,
                DX_LEFTDOWN => 5,
                DX_RIGHTUP => 6,
                DX_RIGHTDOWN => 7,
                _ => {
                    log::error!(
                        "plr_reset_status (state.rs): illegal value for dir: {} for char {}",
                        ch[cn].dir,
                        cn
                    );
                    ch[cn].dir = DX_UP;
                    0
                }
            };
        });
    }

    /// Port of `do_raise_attrib(int cn, int nr)` from `svr_do.cpp`
    ///
    /// Attempts to raise an attribute using available character points.
    /// Checks if the attribute can be raised (not at max, not zero) and if
    /// the character has enough points to pay for the increase.
    ///
    /// # Arguments
    /// * `cn` - Character index
    /// * `attrib` - Attribute index (0-4: BRAVE, WILL, INT, AGIL, STREN)
    ///
    /// # Returns
    /// * `true` - Attribute was successfully raised
    /// * `false` - Cannot raise attribute (at max, zero, or insufficient points)
    pub fn do_raise_attrib(&self, cn: usize, attrib: i32) -> bool {
        let attrib_idx = attrib as usize;
        if attrib_idx >= 5 {
            return false;
        }

        let (current_val, max_val, diff, available_points) = Repository::with_characters(|ch| {
            (
                ch[cn].attrib[attrib_idx][0],
                ch[cn].attrib[attrib_idx][2],
                ch[cn].attrib[attrib_idx][3],
                ch[cn].points,
            )
        });

        // Can't raise if current value is 0 or already at max
        if current_val == 0 || current_val >= max_val {
            return false;
        }

        // Calculate points needed to raise this attribute
        let points_needed = helpers::attrib_needed(current_val as i32, diff as i32);

        if points_needed > available_points {
            return false;
        }

        // Spend points and raise attribute
        Repository::with_characters_mut(|ch| {
            ch[cn].points -= points_needed;
            ch[cn].attrib[attrib_idx][0] += 1;
        });

        // TODO: Implement do_update_char
        log::info!(
            "TODO: Call do_update_char for cn={} (raised attrib {})",
            cn,
            attrib
        );

        true
    }

    /// Port of `do_raise_hp(int cn)` from `svr_do.cpp`
    ///
    /// Attempts to raise base HP using available character points.
    pub fn do_raise_hp(&self, cn: usize) -> bool {
        let (current_val, max_val, diff, available_points) = Repository::with_characters(|ch| {
            (ch[cn].hp[0], ch[cn].hp[2], ch[cn].hp[3], ch[cn].points)
        });

        if current_val == 0 || current_val >= max_val {
            return false;
        }

        let points_needed = helpers::hp_needed(current_val as i32, diff as i32);

        if points_needed > available_points {
            return false;
        }

        Repository::with_characters_mut(|ch| {
            ch[cn].points -= points_needed;
            ch[cn].hp[0] += 1;
        });

        // TODO: Implement do_update_char
        log::info!("TODO: Call do_update_char for cn={} (raised hp)", cn);

        true
    }

    /// Port of `do_lower_hp(int cn)` from `svr_do.cpp`
    ///
    /// Permanently lowers base HP and removes the points from total.
    /// Used for death penalties.
    pub fn do_lower_hp(&self, cn: usize) -> bool {
        let current_val = Repository::with_characters(|ch| ch[cn].hp[0]);

        if current_val < 11 {
            return false;
        }

        Repository::with_characters_mut(|ch| {
            ch[cn].hp[0] -= 1;
        });

        let new_val = Repository::with_characters(|ch| ch[cn].hp[0]);
        let diff = Repository::with_characters(|ch| ch[cn].hp[3]);

        let points_lost = helpers::hp_needed(new_val as i32, diff as i32);

        Repository::with_characters_mut(|ch| {
            ch[cn].points_tot -= points_lost;
        });

        // TODO: Implement do_update_char
        log::info!("TODO: Call do_update_char for cn={} (lowered hp)", cn);

        true
    }

    /// Port of `do_lower_mana(int cn)` from `svr_do.cpp`
    ///
    /// Permanently lowers base mana and removes the points from total.
    /// Used for death penalties.
    pub fn do_lower_mana(&self, cn: usize) -> bool {
        let current_val = Repository::with_characters(|ch| ch[cn].mana[0]);

        if current_val < 11 {
            return false;
        }

        Repository::with_characters_mut(|ch| {
            ch[cn].mana[0] -= 1;
        });

        let new_val = Repository::with_characters(|ch| ch[cn].mana[0]);
        let diff = Repository::with_characters(|ch| ch[cn].mana[3]);

        let points_lost = helpers::mana_needed(new_val as i32, diff as i32);

        Repository::with_characters_mut(|ch| {
            ch[cn].points_tot -= points_lost;
        });

        // TODO: Implement do_update_char
        log::info!("TODO: Call do_update_char for cn={} (lowered mana)", cn);

        true
    }

    /// Port of `do_raise_end(int cn)` from `svr_do.cpp`
    ///
    /// Attempts to raise base endurance using available character points.
    pub fn do_raise_end(&self, cn: usize) -> bool {
        let (current_val, max_val, diff, available_points) = Repository::with_characters(|ch| {
            (ch[cn].end[0], ch[cn].end[2], ch[cn].end[3], ch[cn].points)
        });

        if current_val == 0 || current_val >= max_val {
            return false;
        }

        let points_needed = helpers::end_needed(current_val as i32, diff as i32);

        if points_needed > available_points {
            return false;
        }

        Repository::with_characters_mut(|ch| {
            ch[cn].points -= points_needed;
            ch[cn].end[0] += 1;
        });

        // TODO: Implement do_update_char
        log::info!("TODO: Call do_update_char for cn={} (raised end)", cn);

        true
    }

    /// Port of `do_raise_mana(int cn)` from `svr_do.cpp`
    ///
    /// Attempts to raise base mana using available character points.
    pub fn do_raise_mana(&self, cn: usize) -> bool {
        let (current_val, max_val, diff, available_points) = Repository::with_characters(|ch| {
            (
                ch[cn].mana[0],
                ch[cn].mana[2],
                ch[cn].mana[3],
                ch[cn].points,
            )
        });

        if current_val == 0 || current_val >= max_val {
            return false;
        }

        let points_needed = helpers::mana_needed(current_val as i32, diff as i32);

        if points_needed > available_points {
            return false;
        }

        Repository::with_characters_mut(|ch| {
            ch[cn].points -= points_needed;
            ch[cn].mana[0] += 1;
        });

        // TODO: Implement do_update_char
        log::info!("TODO: Call do_update_char for cn={} (raised mana)", cn);

        true
    }

    /// Port of `do_raise_skill(int cn, int nr)` from `svr_do.cpp`
    ///
    /// Attempts to raise a skill using available character points.
    ///
    /// # Arguments
    /// * `cn` - Character index
    /// * `skill` - Skill index (0-49)
    ///
    /// # Returns
    /// * `true` - Skill was successfully raised
    /// * `false` - Cannot raise skill (at max, zero, or insufficient points)
    pub fn do_raise_skill(&self, cn: usize, skill: i32) -> bool {
        let skill_idx = skill as usize;
        if skill_idx >= 50 {
            return false;
        }

        let (current_val, max_val, diff, available_points) = Repository::with_characters(|ch| {
            (
                ch[cn].skill[skill_idx][0],
                ch[cn].skill[skill_idx][2],
                ch[cn].skill[skill_idx][3],
                ch[cn].points,
            )
        });

        if current_val == 0 || current_val >= max_val {
            return false;
        }

        let points_needed = helpers::skill_needed(current_val as i32, diff as i32);

        if points_needed > available_points {
            return false;
        }

        Repository::with_characters_mut(|ch| {
            ch[cn].points -= points_needed;
            ch[cn].skill[skill_idx][0] += 1;
            ch[cn].set_do_update_flags();
        });

        true
    }

    /// Port of `do_item_value(int in)` from `svr_do.cpp`
    ///
    /// Returns the value of an item (for buying/selling/trading).
    ///
    /// # Arguments
    /// * `item_idx` - The index of the item
    ///
    /// # Returns
    /// * Item value in gold, or 0 if item index is invalid
    pub fn do_item_value(&self, item_idx: usize) -> u32 {
        if item_idx < 1 || item_idx >= core::constants::MAXITEM {
            return 0;
        }

        Repository::with_items(|items| items[item_idx].value)
    }

    /// Port of `do_look_item(int cn, int in)` from `svr_do.cpp`
    ///
    /// Displays detailed information about an item to a character.
    /// Shows description, condition, and compares with carried item if applicable.
    ///
    /// # Arguments
    /// * `cn` - Character looking at the item
    /// * `item_idx` - The item being examined
    pub fn do_look_item(&mut self, cn: usize, item_idx: usize) {
        // Determine if item is active
        let act = Repository::with_items(|items| if items[item_idx].active != 0 { 1 } else { 0 });

        // Check if character has the item in inventory or worn
        let mut has_item = false;

        Repository::with_characters(|ch| {
            // Check inventory
            for n in 0..40 {
                if ch[cn].item[n] == item_idx as u32 {
                    has_item = true;
                    break;
                }
            }

            // Check worn items if not found in inventory
            if !has_item {
                for n in 0..20 {
                    if ch[cn].worn[n] == item_idx as u32 {
                        has_item = true;
                        break;
                    }
                }
            }
        });

        // If character doesn't have item, check if they can see it
        if !has_item && self.do_char_can_see_item(cn, item_idx) == 0 {
            return;
        }

        // Check if item has special look driver
        let has_lookspecial = Repository::with_items(|items| {
            items[item_idx].flags & ItemFlags::IF_LOOKSPECIAL.bits() != 0
        });

        if has_lookspecial {
            // TODO: Implement look_driver
            log::info!("TODO: Call look_driver({}, {})", cn, item_idx);
        } else {
            // Show item description
            let description = Repository::with_items(|items| items[item_idx].description.clone());
            self.do_character_log(
                cn,
                FontColor::Green,
                &format!("{}\n", String::from_utf8_lossy(&description)),
            );

            // Show condition if item has aging or damage
            let (max_age_0, max_age_1, max_damage, damage_state) =
                Repository::with_items(|items| {
                    (
                        items[item_idx].max_age[act],
                        items[item_idx].max_age[if act == 0 { 1 } else { 0 }],
                        items[item_idx].max_damage,
                        items[item_idx].damage_state,
                    )
                });

            if max_age_0 != 0 || max_age_1 != 0 || max_damage != 0 {
                let condition_msg = match damage_state {
                    0 => "It's in perfect condition.\n",
                    1 => "It's showing signs of age.\n",
                    2 => "It's fairly old.\n",
                    3 => "It is old.\n",
                    4 => "It is very old and battered.\n",
                    _ => "",
                };

                if !condition_msg.is_empty() {
                    let color = if damage_state >= 4 {
                        FontColor::Yellow
                    } else {
                        FontColor::Green
                    };
                    self.do_character_log(cn, color, condition_msg);
                }
            }

            // Show detailed info for build mode
            let is_building = Repository::with_characters(|ch| {
                ch[cn].flags & CharacterFlags::CF_BUILDMODE.bits() != 0
            });

            if is_building {
                let (
                    temp,
                    sprite_0,
                    sprite_1,
                    curr_age_0,
                    max_age_0,
                    curr_age_1,
                    max_age_1,
                    curr_damage,
                    max_damage,
                    active,
                    duration,
                    driver,
                    data,
                ) = Repository::with_items(|items| {
                    (
                        items[item_idx].temp,
                        items[item_idx].sprite[0],
                        items[item_idx].sprite[1],
                        items[item_idx].current_age[0],
                        items[item_idx].max_age[0],
                        items[item_idx].current_age[1],
                        items[item_idx].max_age[1],
                        items[item_idx].current_damage,
                        items[item_idx].max_damage,
                        items[item_idx].active,
                        items[item_idx].duration,
                        items[item_idx].driver,
                        items[item_idx].data,
                    )
                });

                self.do_character_log(
                    cn,
                    FontColor::Green,
                    &format!("Temp: {}, Sprite: {},{}.\n", temp, sprite_0, sprite_1),
                );
                self.do_character_log(
                    cn,
                    FontColor::Green,
                    &format!("In-Active Age {} of {}.\n", curr_age_0, max_age_0),
                );
                self.do_character_log(
                    cn,
                    FontColor::Green,
                    &format!("Active Age {} of {}.\n", curr_age_1, max_age_1),
                );
                self.do_character_log(
                    cn,
                    FontColor::Green,
                    &format!("Damage {} of {}.\n", curr_damage, max_damage),
                );
                self.do_character_log(
                    cn,
                    FontColor::Green,
                    &format!("Active {} of {}.\n", active, duration),
                );
                self.do_character_log(
                    cn,
                    FontColor::Green,
                    &format!(
                        "Driver={} [{},{},{},{},{},{},{},{},{},{}].\n",
                        driver,
                        data[0],
                        data[1],
                        data[2],
                        data[3],
                        data[4],
                        data[5],
                        data[6],
                        data[7],
                        data[8],
                        data[9]
                    ),
                );
            }

            // Show god-mode info
            let is_god =
                Repository::with_characters(|ch| ch[cn].flags & CharacterFlags::CF_GOD.bits() != 0);

            if is_god {
                let (
                    temp,
                    value,
                    active,
                    sprite_0,
                    sprite_1,
                    max_age_0,
                    max_age_1,
                    curr_age_0,
                    curr_age_1,
                    max_damage,
                    curr_damage,
                ) = Repository::with_items(|items| {
                    (
                        items[item_idx].temp,
                        items[item_idx].value,
                        items[item_idx].active,
                        items[item_idx].sprite[0],
                        items[item_idx].sprite[1],
                        items[item_idx].max_age[0],
                        items[item_idx].max_age[1],
                        items[item_idx].current_age[0],
                        items[item_idx].current_age[1],
                        items[item_idx].max_damage,
                        items[item_idx].current_damage,
                    )
                });

                self.do_character_log(
                    cn,
                    FontColor::Green,
                    &format!(
                        "ID={}, Temp={}, Value: {}G {}S.\n",
                        item_idx,
                        temp,
                        value / 100,
                        value % 100
                    ),
                );
                self.do_character_log(
                    cn,
                    FontColor::Green,
                    &format!("active={}, sprite={}/{}\n", active, sprite_0, sprite_1),
                );
                self.do_character_log(
                    cn,
                    FontColor::Green,
                    &format!(
                        "max_age={}/{}, current_age={}/{}\n",
                        max_age_0, max_age_1, curr_age_0, curr_age_1
                    ),
                );
                self.do_character_log(
                    cn,
                    FontColor::Green,
                    &format!(
                        "max_damage={}, current_damage={}\n",
                        max_damage, curr_damage
                    ),
                );
            }

            // Compare with carried item if present
            let citem = Repository::with_characters(|ch| ch[cn].citem);

            if citem != 0 && (citem & 0x80000000) == 0 {
                let citem_idx = citem as usize;

                // Validate carried item
                if citem_idx > 0 && citem_idx < core::constants::MAXITEM {
                    self.do_character_log(cn, FontColor::Green, " \n");

                    let citem_name = Repository::with_items(|items| items[citem_idx].name.clone());
                    self.do_character_log(
                        cn,
                        FontColor::Green,
                        &format!(
                            "You compare it with a {}:\n",
                            String::from_utf8_lossy(&citem_name)
                        ),
                    );

                    // Compare weapon stats
                    let (weapon_this, weapon_carried, name_this) =
                        Repository::with_items(|items| {
                            (
                                items[item_idx].weapon[0],
                                items[citem_idx].weapon[0],
                                items[item_idx].name.clone(),
                            )
                        });

                    if weapon_this > weapon_carried {
                        self.do_character_log(
                            cn,
                            FontColor::Green,
                            &format!(
                                "A {} is the better weapon.\n",
                                String::from_utf8_lossy(&name_this)
                            ),
                        );
                    } else if weapon_this < weapon_carried {
                        self.do_character_log(
                            cn,
                            FontColor::Green,
                            &format!(
                                "A {} is the better weapon.\n",
                                String::from_utf8_lossy(&citem_name)
                            ),
                        );
                    } else {
                        self.do_character_log(cn, FontColor::Green, "No difference as a weapon.\n");
                    }

                    // Compare armor stats
                    let (armor_this, armor_carried) = Repository::with_items(|items| {
                        (items[item_idx].armor[0], items[citem_idx].armor[0])
                    });

                    if armor_this > armor_carried {
                        self.do_character_log(
                            cn,
                            FontColor::Green,
                            &format!(
                                "A {} is the better armor.\n",
                                String::from_utf8_lossy(&name_this)
                            ),
                        );
                    } else if armor_this < armor_carried {
                        self.do_character_log(
                            cn,
                            FontColor::Green,
                            &format!(
                                "A {} is the better armor.\n",
                                String::from_utf8_lossy(&citem_name)
                            ),
                        );
                    } else {
                        self.do_character_log(cn, FontColor::Green, "No difference as armor.\n");
                    }
                }
            } else {
                // No carried item - show item_info if identified
                let is_identified = Repository::with_items(|items| {
                    items[item_idx].flags & ItemFlags::IF_IDENTIFIED.bits() != 0
                });

                if is_identified {
                    // TODO: Implement item_info
                    log::info!("TODO: Call item_info({}, {}, 1)", cn, item_idx);
                }
            }

            // Special case: tombstone remote scan
            let (item_temp, item_data_0) =
                Repository::with_items(|items| (items[item_idx].temp, items[item_idx].data[0]));

            if item_temp == core::constants::IT_TOMBSTONE as u16 && item_data_0 != 0 {
                // TODO: Implement do_ransack_corpse
                log::info!(
                    "TODO: Call do_ransack_corpse({}, {}, 'In the tombstone you notice %s!\\n')",
                    cn,
                    item_data_0
                );
            }

            // Special case: driver 57 (career pole check)
            let item_driver = Repository::with_items(|items| items[item_idx].driver);
            if item_driver == 57 {
                let (points_tot, data_4) = Repository::with_characters(|ch| {
                    let item_data = Repository::with_items(|items| items[item_idx].data[4]);
                    (ch[cn].points_tot, item_data)
                });

                let percent = std::cmp::min(100, (100 * (points_tot / 10)) / (data_4 as i32 + 1));

                if percent < 50 {
                    self.do_character_log(
                        cn,
                        FontColor::Yellow,
                        "You sense that it's too early in your career to touch this pole.\n",
                    );
                } else if percent < 70 {
                    self.do_character_log(
                        cn,
                        FontColor::Yellow,
                        "You sense that it's a bit early in your career to touch this pole.\n",
                    );
                }
            }
        }
    }

    /// Port of `barter(int cn, int opr, int flag)` from `svr_do.cpp`
    ///
    /// Calculates adjusted price based on character's barter skill.
    /// Better barter skill gets better prices when buying or selling.
    ///
    /// # Arguments
    /// * `cn` - Character index
    /// * `opr` - Original price of the item
    /// * `flag` - 1 if merchant is selling (player buying), 0 if merchant is buying (player selling)
    ///
    /// # Returns
    /// * Adjusted price after applying barter skill
    pub fn barter(&self, cn: usize, opr: i32, flag: i32) -> i32 {
        let barter_skill =
            Repository::with_characters(|ch| ch[cn].skill[core::constants::SK_BARTER][5] as i32);

        let pr = if flag != 0 {
            // Merchant is selling (player is buying)
            // Higher skill = lower price
            let calculated = opr * 4 - (opr * barter_skill) / 50;
            // Price can't go below original price
            if calculated < opr {
                opr
            } else {
                calculated
            }
        } else {
            // Merchant is buying (player is selling)
            // Higher skill = higher price for player
            let calculated = opr / 4 + (opr * barter_skill) / 200;
            // Price can't go above original price
            if calculated > opr {
                opr
            } else {
                calculated
            }
        };

        pr
    }

    /// Port of `do_shop_char(int cn, int co, int nr)` from `svr_do.cpp`
    ///
    /// Handles shopping interactions between a character and a merchant/body.
    /// This function supports:
    /// - Selling items to merchants (when character has citem)
    /// - Buying items from merchants (nr < 62)
    /// - Looting items from corpses (CF_BODY)
    /// - Examining item descriptions (nr >= 62)
    ///
    /// # Arguments
    /// * `cn` - Character performing the action (player)
    /// * `co` - Target character (merchant or corpse)
    /// * `nr` - Action selector:
    ///   - 0-39: Buy/take from merchant/corpse inventory
    ///   - 40-59: Take from corpse worn items
    ///   - 60: Take carried item from corpse
    ///   - 61: Take gold from corpse
    ///   - 62+: Examine item descriptions (nr-62 gives item slot)
    pub fn do_shop_char(&mut self, cn: usize, co: usize, nr: i32) {
        // Validate parameters
        if co == 0 || co >= core::constants::MAXCHARS || nr < 0 || nr >= 124 {
            return;
        }

        // Check if target is a merchant or corpse (body)
        let (is_merchant, is_body) = Repository::with_characters(|ch| {
            (
                ch[co].flags & CharacterFlags::CF_MERCHANT.bits() != 0,
                ch[co].flags & CharacterFlags::CF_BODY.bits() != 0,
            )
        });

        if !is_merchant && !is_body {
            return;
        }

        // For living merchants, check visibility
        if !is_body {
            // TODO: Implement do_char_can_see
            // For now, assume visible
            log::info!("TODO: Check if cn={} can see co={}", cn, co);
        }

        // For corpses, check distance (must be adjacent)
        if is_body {
            let (cn_x, cn_y, co_x, co_y) = Repository::with_characters(|ch| {
                (
                    ch[cn].x as i32,
                    ch[cn].y as i32,
                    ch[co].x as i32,
                    ch[co].y as i32,
                )
            });

            let distance = (cn_x - co_x).abs() + (cn_y - co_y).abs();
            if distance > 1 {
                return;
            }
        }

        // Handle selling to merchant (player has citem)
        let citem = Repository::with_characters(|ch| ch[cn].citem);

        if citem != 0 && is_merchant {
            // Check if trying to sell money
            if citem & 0x80000000 != 0 {
                self.do_character_log(cn, FontColor::Green, "You want to sell money? Weird!\n");
                return;
            }

            let item_idx = citem as usize;

            // Check if merchant accepts this type of item
            let merchant_template = Repository::with_characters(|ch| ch[co].data[0] as usize);

            let (item_flags, template_flags) = Repository::with_items(|items| {
                Repository::with_item_templates(|templates| {
                    (items[item_idx].flags, templates[merchant_template].flags)
                })
            });

            let mut accepts = false;
            if (item_flags & ItemFlags::IF_ARMOR.bits() != 0)
                && (template_flags & ItemFlags::IF_ARMOR.bits() != 0)
            {
                accepts = true;
            }
            if (item_flags & ItemFlags::IF_WEAPON.bits() != 0)
                && (template_flags & ItemFlags::IF_WEAPON.bits() != 0)
            {
                accepts = true;
            }
            if (item_flags & ItemFlags::IF_MAGIC.bits() != 0)
                && (template_flags & ItemFlags::IF_MAGIC.bits() != 0)
            {
                accepts = true;
            }
            if (item_flags & ItemFlags::IF_MISC.bits() != 0)
                && (template_flags & ItemFlags::IF_MISC.bits() != 0)
            {
                accepts = true;
            }

            if !accepts {
                let merchant_name = Repository::with_characters(|ch| {
                    String::from_utf8_lossy(&ch[co].name).to_string()
                });
                self.do_character_log(
                    cn,
                    FontColor::Green,
                    &format!("{} doesn't buy those.\n", merchant_name),
                );
                return;
            }

            // Calculate price with barter
            let value = self.do_item_value(item_idx);
            let price = self.barter(cn, value as i32, 0);

            // Check if merchant can afford it
            let merchant_gold = Repository::with_characters(|ch| ch[co].gold);
            if merchant_gold < price as i32 {
                let merchant_ref = Repository::with_characters(|ch| {
                    String::from_utf8_lossy(&ch[co].reference).to_string()
                });
                self.do_character_log(
                    cn,
                    FontColor::Green,
                    &format!("{} cannot afford that.\n", merchant_ref),
                );
                return;
            }

            // Complete the sale
            Repository::with_characters_mut(|ch| {
                ch[cn].citem = 0;
                ch[cn].gold += price as i32;
            });

            // Transfer item to merchant
            // TODO: Implement god_give_char - for now just log
            log::info!("TODO: god_give_char({}, {})", item_idx, co);

            let item_name = Repository::with_items(|items| {
                String::from_utf8_lossy(&items[item_idx].name).to_string()
            });

            let item_ref = Repository::with_items(|items| {
                String::from_utf8_lossy(&items[item_idx].reference).to_string()
            });

            // TODO: Implement chlog
            log::info!("TODO: chlog({}, 'Sold {}')", cn, item_name);

            self.do_character_log(
                cn,
                FontColor::Yellow,
                &format!(
                    "You sold a {} for {}G {}S.\n",
                    item_ref,
                    price / 100,
                    price % 100
                ),
            );

            // Update item template statistics
            let temp_id = Repository::with_items(|items| items[item_idx].temp as usize);
            if temp_id > 0 && temp_id < core::constants::MAXTITEM {
                Repository::with_item_templates_mut(|templates| {
                    templates[temp_id].t_sold += 1;
                });
            }
        } else {
            // Handle buying/taking/examining items
            if nr < 62 {
                // Buying or taking items
                if nr < 40 {
                    // Inventory slot
                    let item_idx =
                        Repository::with_characters(|ch| ch[co].item[nr as usize] as usize);

                    if item_idx != 0 {
                        let price = if is_merchant {
                            let value = self.do_item_value(item_idx);
                            let pr = self.barter(cn, value as i32, 1);

                            let player_gold = Repository::with_characters(|ch| ch[cn].gold);
                            if player_gold < pr as i32 {
                                self.do_character_log(
                                    cn,
                                    FontColor::Green,
                                    "You cannot afford that.\n",
                                );
                                return;
                            }
                            pr
                        } else {
                            0
                        };

                        // TODO: Implement god_take_from_char and god_give_char
                        log::info!("TODO: god_take_from_char({}, {})", item_idx, co);

                        let gave_success = God::give_character_item(cn, item_idx);

                        if gave_success {
                            if is_merchant {
                                Repository::with_characters_mut(|ch| {
                                    ch[cn].gold -= price;
                                    ch[co].gold += price;
                                });

                                let item_name = Repository::with_items(|items| {
                                    String::from_utf8_lossy(&items[item_idx].name).to_string()
                                });
                                let item_ref = Repository::with_items(|items| {
                                    String::from_utf8_lossy(&items[item_idx].reference).to_string()
                                });

                                // TODO: Implement chlog
                                log::info!("TODO: chlog({}, 'Bought {}')", cn, item_name);

                                self.do_character_log(
                                    cn,
                                    FontColor::Yellow,
                                    &format!(
                                        "You bought a {} for {}G {}S.\n",
                                        item_ref,
                                        price / 100,
                                        price % 100
                                    ),
                                );

                                // Update template statistics
                                let temp_id =
                                    Repository::with_items(|items| items[item_idx].temp as usize);
                                if temp_id > 0 && temp_id < core::constants::MAXTITEM {
                                    Repository::with_item_templates_mut(|templates| {
                                        templates[temp_id].t_bought += 1;
                                    });
                                }
                            } else {
                                let item_name = Repository::with_items(|items| {
                                    String::from_utf8_lossy(&items[item_idx].name).to_string()
                                });
                                let item_ref = Repository::with_items(|items| {
                                    String::from_utf8_lossy(&items[item_idx].reference).to_string()
                                });

                                self.do_character_log(
                                    cn,
                                    FontColor::Yellow,
                                    &format!("You took a {}.\n", item_ref),
                                );
                            }
                        } else {
                            // Failed to give item - put it back
                            // TODO: Implement god_give_char to return item
                            log::info!("TODO: god_give_char({}, {}) to return item", item_idx, co);

                            let item_ref = Repository::with_items(|items| {
                                String::from_utf8_lossy(&items[item_idx].reference).to_string()
                            });

                            if is_merchant {
                                self.do_character_log(
                                    cn,
                                    FontColor::Green,
                                    &format!(
                                        "You cannot buy the {} because your inventory is full.\n",
                                        item_ref
                                    ),
                                );
                            } else {
                                self.do_character_log(
                                    cn,
                                    FontColor::Green,
                                    &format!(
                                        "You cannot take the {} because your inventory is full.\n",
                                        item_ref
                                    ),
                                );
                            }
                        }
                    }
                } else if nr < 60 {
                    // Worn items (only for corpses)
                    if is_body {
                        let worn_slot = (nr - 40) as usize;
                        let item_idx =
                            Repository::with_characters(|ch| ch[co].worn[worn_slot] as usize);

                        if item_idx != 0 {
                            // TODO: Implement god_take_from_char
                            log::info!("TODO: god_take_from_char({}, {})", item_idx, co);

                            let gave_success = God::give_character_item(cn, item_idx);

                            if gave_success {
                                let item_name = Repository::with_items(|items| {
                                    String::from_utf8_lossy(&items[item_idx].name).to_string()
                                });
                                let item_ref = Repository::with_items(|items| {
                                    String::from_utf8_lossy(&items[item_idx].reference).to_string()
                                });

                                // TODO: Implement chlog
                                log::info!("TODO: chlog({}, 'Took {}')", cn, item_name);

                                self.do_character_log(
                                    cn,
                                    FontColor::Yellow,
                                    &format!("You took a {}.\n", item_ref),
                                );
                            } else {
                                // TODO: Implement god_give_char to return item
                                log::info!(
                                    "TODO: god_give_char({}, {}) to return item",
                                    item_idx,
                                    co
                                );

                                let item_ref = Repository::with_items(|items| {
                                    String::from_utf8_lossy(&items[item_idx].reference).to_string()
                                });

                                self.do_character_log(
                                    cn,
                                    FontColor::Green,
                                    &format!(
                                        "You cannot take the {} because your inventory is full.\n",
                                        item_ref
                                    ),
                                );
                            }
                        }
                    }
                } else if nr == 60 {
                    // Carried item (only for corpses)
                    if is_body {
                        let item_idx = Repository::with_characters(|ch| ch[co].citem as usize);

                        if item_idx != 0 {
                            // TODO: Implement god_take_from_char
                            log::info!("TODO: god_take_from_char({}, {})", item_idx, co);

                            let gave_success = God::give_character_item(cn, item_idx);

                            if gave_success {
                                let item_name = Repository::with_items(|items| {
                                    String::from_utf8_lossy(&items[item_idx].name).to_string()
                                });
                                let item_ref = Repository::with_items(|items| {
                                    String::from_utf8_lossy(&items[item_idx].reference).to_string()
                                });

                                // TODO: Implement chlog
                                log::info!("TODO: chlog({}, 'Took {}')", cn, item_name);

                                self.do_character_log(
                                    cn,
                                    FontColor::Yellow,
                                    &format!("You took a {}.\n", item_ref),
                                );
                            } else {
                                // TODO: Implement god_give_char to return item
                                log::info!(
                                    "TODO: god_give_char({}, {}) to return item",
                                    item_idx,
                                    co
                                );

                                let item_ref = Repository::with_items(|items| {
                                    String::from_utf8_lossy(&items[item_idx].reference).to_string()
                                });

                                self.do_character_log(
                                    cn,
                                    FontColor::Green,
                                    &format!(
                                        "You cannot take the {} because your inventory is full.\n",
                                        item_ref
                                    ),
                                );
                            }
                        }
                    }
                } else {
                    // nr == 61: Take gold (only for corpses)
                    if is_body {
                        let corpse_gold = Repository::with_characters(|ch| ch[co].gold);

                        if corpse_gold > 0 {
                            Repository::with_characters_mut(|ch| {
                                ch[cn].gold += corpse_gold;
                                ch[co].gold = 0;
                            });

                            // TODO: Implement chlog
                            log::info!(
                                "TODO: chlog({}, 'Took {}G {}S')",
                                cn,
                                corpse_gold / 100,
                                corpse_gold % 100
                            );

                            self.do_character_log(
                                cn,
                                FontColor::Yellow,
                                &format!(
                                    "You took {}G {}S.\n",
                                    corpse_gold / 100,
                                    corpse_gold % 100
                                ),
                            );
                        }
                    }
                }
            } else {
                // Examine item descriptions (nr >= 62)
                let exam_nr = nr - 62;

                if exam_nr < 40 {
                    // Inventory item description
                    let item_idx =
                        Repository::with_characters(|ch| ch[co].item[exam_nr as usize] as usize);

                    if item_idx != 0 {
                        let (item_name, item_desc) = Repository::with_items(|items| {
                            (
                                String::from_utf8_lossy(&items[item_idx].name).to_string(),
                                String::from_utf8_lossy(&items[item_idx].description).to_string(),
                            )
                        });

                        self.do_character_log(cn, FontColor::Yellow, &format!("{}:\n", item_name));
                        self.do_character_log(cn, FontColor::Yellow, &format!("{}\n", item_desc));
                    }
                } else if exam_nr < 61 {
                    // Worn item description (only for corpses)
                    if is_body {
                        let worn_slot = (exam_nr - 40) as usize;
                        let item_idx =
                            Repository::with_characters(|ch| ch[co].worn[worn_slot] as usize);

                        if item_idx != 0 {
                            let (item_name, item_desc) = Repository::with_items(|items| {
                                (
                                    String::from_utf8_lossy(&items[item_idx].name).to_string(),
                                    String::from_utf8_lossy(&items[item_idx].description)
                                        .to_string(),
                                )
                            });

                            self.do_character_log(
                                cn,
                                FontColor::Yellow,
                                &format!("{}:\n", item_name),
                            );
                            self.do_character_log(
                                cn,
                                FontColor::Yellow,
                                &format!("{}\n", item_desc),
                            );
                        }
                    }
                } else {
                    // Carried item description (only for corpses)
                    if is_body {
                        let item_idx = Repository::with_characters(|ch| ch[co].citem as usize);

                        if item_idx != 0 {
                            let (item_name, item_desc) = Repository::with_items(|items| {
                                (
                                    String::from_utf8_lossy(&items[item_idx].name).to_string(),
                                    String::from_utf8_lossy(&items[item_idx].description)
                                        .to_string(),
                                )
                            });

                            self.do_character_log(
                                cn,
                                FontColor::Yellow,
                                &format!("{}:\n", item_name),
                            );
                            self.do_character_log(
                                cn,
                                FontColor::Yellow,
                                &format!("{}\n", item_desc),
                            );
                        }
                    }
                }
            }
        }

        // Update merchant shop display if applicable
        if is_merchant {
            driver::update_shop(co);
        }

        // Refresh the character/corpse display
        self.do_look_char(cn, co, 0, 0, 1);
    }

    /// Port of `do_depot_cost(int in)` from `svr_do.cpp`
    ///
    /// Calculates the storage cost for depositing an item in the depot.
    /// Cost is based on item value, power, and special flags.
    ///
    /// # Arguments
    /// * `item_idx` - The index of the item to calculate depot cost for
    ///
    /// # Returns
    /// * Storage cost in gold per tick
    pub fn do_depot_cost(&self, item_idx: usize) -> i32 {
        if item_idx == 0 || item_idx >= core::constants::MAXITEM {
            return 0;
        }

        Repository::with_items(|items| {
            let item = &items[item_idx];

            let mut cost = 1;

            // Add cost based on item value
            cost += item.value as i32 / 1600;

            // Add cost based on item power (cubic formula)
            let power = item.power as i32;
            cost += (power * power * power) / 16000;

            // Items that are destroyed in labyrinth have much higher storage cost
            if item.flags & ItemFlags::IF_LABYDESTROY.bits() != 0 {
                cost += 20000;
            }

            cost
        })
    }

    /// Port of `do_add_depot(int cn, int in)` from `svr_do.cpp`
    ///
    /// Adds an item to a character's depot storage.
    /// Finds the first empty slot in the depot and stores the item there.
    ///
    /// # Arguments
    /// * `cn` - Character index
    /// * `item_idx` - The index of the item to add to depot
    ///
    /// # Returns
    /// * `true` - Item was successfully added to depot
    /// * `false` - Depot is full (all 62 slots occupied)
    pub fn do_add_depot(&self, cn: usize, item_idx: usize) -> bool {
        // Find first empty depot slot
        let empty_slot = Repository::with_characters(|ch| {
            for n in 0..62 {
                if ch[cn].depot[n] == 0 {
                    return Some(n);
                }
            }
            None
        });

        // If no empty slot found, depot is full
        let slot = match empty_slot {
            Some(n) => n,
            None => return false,
        };

        // Add item to depot slot
        Repository::with_characters_mut(|ch| {
            ch[cn].depot[slot] = item_idx as u32;
            ch[cn].set_do_update_flags();
        });

        true
    }

    /// Port of `do_pay_depot(int cn)` from `svr_do.cpp`
    ///
    /// Handles depot storage fee payment. If the character doesn't have enough gold in
    /// their bank account (data[13]), this function automatically sells the least valuable
    /// items from the depot to cover the storage costs.
    ///
    /// # Arguments
    /// * `cn` - Character index
    ///
    /// # Process
    /// 1. Calculate total depot storage cost
    /// 2. If not enough gold in bank account, sell cheapest depot items until enough funds
    /// 3. Deduct storage cost from bank account
    /// 4. Track total depot costs paid
    pub fn do_pay_depot(&self, cn: usize) {
        loop {
            // Calculate total cost for all items in depot
            let total_cost = self.get_depot_cost(cn);

            let bank_balance = Repository::with_characters(|ch| ch[cn].data[13]);

            if total_cost > bank_balance as i32 {
                // Not enough money - find and sell cheapest item
                let (cheapest_value, cheapest_slot) = Repository::with_characters(|ch| {
                    let mut lowest_value = 99999999;
                    let mut lowest_slot = None;

                    for n in 0..62 {
                        let item_idx = ch[cn].depot[n];
                        if item_idx != 0 {
                            let value = self.do_item_value(item_idx as usize);
                            if value < lowest_value {
                                lowest_value = value;
                                lowest_slot = Some(n);
                            }
                        }
                    }

                    (lowest_value, lowest_slot)
                });

                // If no items to sell, panic
                let slot = match cheapest_slot {
                    Some(n) => n,
                    None => {
                        log::error!("PANIC: depot forced sale failed for cn={}", cn);
                        return;
                    }
                };

                // Sell the item for half its value
                let sell_value = cheapest_value / 2;

                let item_idx = Repository::with_characters(|ch| ch[cn].depot[slot]);

                // Add proceeds to bank account
                Repository::with_characters_mut(|ch| {
                    ch[cn].data[13] += sell_value as i32;
                });

                // Mark item as empty (destroyed)
                Repository::with_items_mut(|items| {
                    items[item_idx as usize].used = core::constants::USE_EMPTY;
                });

                // Remove item from depot
                Repository::with_characters_mut(|ch| {
                    ch[cn].depot[slot] = 0;
                    ch[cn].depot_sold += 1;
                });

                let item_name = Repository::with_items(|items| {
                    String::from_utf8_lossy(&items[item_idx as usize].name).to_string()
                });

                // TODO: Implement chlog
                log::info!(
                    "TODO: chlog({}, 'Bank sold {} for {}G {}S to pay for depot (slot {})')",
                    cn,
                    item_name,
                    sell_value / 100,
                    sell_value % 100,
                    slot
                );
            } else {
                // Enough money - pay the cost
                Repository::with_characters_mut(|ch| {
                    ch[cn].data[13] -= total_cost;
                    ch[cn].depot_cost += total_cost;
                });
                break;
            }
        }
    }

    /// Helper function to calculate total depot storage cost
    ///
    /// Sums up the storage cost for all items currently in the depot.
    ///
    /// # Arguments
    /// * `cn` - Character index
    ///
    /// # Returns
    /// * Total storage cost for all depot items
    fn get_depot_cost(&self, cn: usize) -> i32 {
        Repository::with_characters(|ch| {
            let mut total = 0;
            for n in 0..62 {
                let item_idx = ch[cn].depot[n];
                if item_idx != 0 {
                    total += self.do_depot_cost(item_idx as usize);
                }
            }
            total
        })
    }

    /// Port of `do_depot_char(int cn, int co, int nr)` from `svr_do.cpp`
    ///
    /// Handles depot (bank storage) interactions for a character.
    /// Allows depositing items into depot, withdrawing items, and examining items.
    ///
    /// # Arguments
    /// * `cn` - Character performing the action
    /// * `co` - Target character (must be same as cn for depot)
    /// * `nr` - Action selector:
    ///   - 0-61: Withdraw item from depot slot
    ///   - 62+: Examine item in depot (nr-62 gives slot)
    ///   - If character has citem: Deposit that item
    pub fn do_depot_char(&mut self, cn: usize, co: usize, nr: i32) {
        // Validate parameters
        if co == 0 || co >= core::constants::MAXCHARS || nr < 0 || nr >= 124 {
            return;
        }

        // Can only access own depot
        if cn != co {
            return;
        }

        // Check if in a bank or is god
        let (char_x, char_y, is_god) = Repository::with_characters(|ch| {
            (
                ch[cn].x,
                ch[cn].y,
                ch[cn].flags & CharacterFlags::CF_GOD.bits() != 0,
            )
        });

        if !is_god {
            let map_idx = char_x as usize + char_y as usize * core::constants::SERVER_MAPX as usize;
            let in_bank = Repository::with_map(|map| {
                map[map_idx].flags & core::constants::MF_BANK as u64 != 0
            });

            if !in_bank {
                self.do_character_log(
                    cn,
                    FontColor::Green,
                    "You cannot access your depot outside a bank.\n",
                );
                return;
            }
        }

        let citem = Repository::with_characters(|ch| ch[cn].citem);

        if citem != 0 {
            // Depositing an item
            if citem & 0x80000000 != 0 {
                self.do_character_log(
                    cn,
                    FontColor::Green,
                    "Use #deposit to put money in the bank!\n",
                );
                return;
            }

            let item_idx = citem as usize;

            // Check if allowed to deposit
            if !self.do_maygive(cn, 0, item_idx) {
                self.do_character_log(cn, FontColor::Green, "You are not allowed to do that!\n");
                return;
            }

            let has_nodepot = Repository::with_items(|items| {
                items[item_idx].flags & ItemFlags::IF_NODEPOT.bits() != 0
            });

            if has_nodepot {
                self.do_character_log(cn, FontColor::Green, "You are not allowed to do that!\n");
                return;
            }

            // Calculate storage cost
            let storage_cost = self.do_depot_cost(item_idx);

            // Try to add to depot
            if self.do_add_depot(co, item_idx) {
                Repository::with_characters_mut(|ch| {
                    ch[cn].citem = 0;
                });

                let item_ref = Repository::with_items(|items| {
                    String::from_utf8_lossy(&items[item_idx].reference).to_string()
                });

                let item_name = Repository::with_items(|items| {
                    String::from_utf8_lossy(&items[item_idx].name).to_string()
                });

                // Calculate costs per day (Astonian and Earth)
                let astonian_cost = storage_cost;
                let earth_cost = storage_cost * 18; // 18 Astonian days per Earth day

                self.do_character_log(
                    cn,
                    FontColor::Yellow,
                    &format!(
                        "You deposited {}. The rent is {}G {}S per Astonian day or {}G {}S per Earth day.\n",
                        item_ref,
                        astonian_cost / 100,
                        astonian_cost % 100,
                        earth_cost / 100,
                        earth_cost % 100
                    ),
                );

                // TODO: Implement chlog
                log::info!("TODO: chlog({}, 'Deposited {}')", cn, item_name);
            }
        } else {
            // Withdrawing or examining items
            if nr < 62 {
                // Withdraw item from depot
                let item_idx = Repository::with_characters(|ch| ch[co].depot[nr as usize]);

                if item_idx != 0 {
                    // TODO: Implement god_give_char
                    let gave_success = God::give_character_item(cn, item_idx as usize);

                    if gave_success {
                        Repository::with_characters_mut(|ch| {
                            ch[co].depot[nr as usize] = 0;
                        });

                        let item_ref = Repository::with_items(|items| {
                            String::from_utf8_lossy(&items[item_idx as usize].reference).to_string()
                        });

                        let item_name = Repository::with_items(|items| {
                            String::from_utf8_lossy(&items[item_idx as usize].name).to_string()
                        });

                        self.do_character_log(
                            cn,
                            FontColor::Yellow,
                            &format!("You took the {} from your depot.\n", item_ref),
                        );

                        // TODO: Implement chlog
                        log::info!("TODO: chlog({}, 'Took {} from depot')", cn, item_name);
                    } else {
                        let item_ref = Repository::with_items(|items| {
                            String::from_utf8_lossy(&items[item_idx as usize].reference).to_string()
                        });

                        self.do_character_log(
                            cn,
                            FontColor::Green,
                            &format!(
                                "You cannot take the {} because your inventory is full.\n",
                                item_ref
                            ),
                        );
                    }
                }
            } else {
                // Examine item in depot
                let exam_slot = (nr - 62) as usize;
                let item_idx = Repository::with_characters(|ch| ch[co].depot[exam_slot]);

                if item_idx != 0 {
                    let (item_name, item_desc) = Repository::with_items(|items| {
                        (
                            String::from_utf8_lossy(&items[item_idx as usize].name).to_string(),
                            String::from_utf8_lossy(&items[item_idx as usize].description)
                                .to_string(),
                        )
                    });

                    self.do_character_log(cn, FontColor::Yellow, &format!("{}:\n", item_name));
                    self.do_character_log(cn, FontColor::Yellow, &format!("{}\n", item_desc));
                }
            }
        }
    }

    /// Port of `do_look_char(int cn, int co, int godflag, int autoflag, int lootflag)` from `svr_do.cpp`
    ///
    /// Displays detailed information about a character (merchant, corpse, or other player/NPC).
    /// This function sends multiple binary packets to the client to display:
    /// - Character description and status messages
    /// - Character equipment and stats
    /// - Shop/corpse inventory if applicable
    ///
    /// # Arguments
    /// * `cn` - Character doing the looking
    /// * `co` - Character being looked at
    /// * `godflag` - If set, bypasses visibility checks
    /// * `autoflag` - If set, suppresses descriptive text (for repeated/automatic looks)
    /// * `lootflag` - If set, allows looking at corpses
    pub fn do_look_char(
        &mut self,
        cn: usize,
        co: usize,
        godflag: i32,
        autoflag: i32,
        lootflag: i32,
    ) {
        // Validate parameters
        if co == 0 || co >= core::constants::MAXCHARS {
            return;
        }

        // Check if target is a corpse and distance
        let (is_body, co_x, co_y) = Repository::with_characters(|ch| {
            (
                ch[co].flags & CharacterFlags::CF_BODY.bits() != 0,
                ch[co].x,
                ch[co].y,
            )
        });

        if is_body {
            let (cn_x, cn_y) = Repository::with_characters(|ch| (ch[cn].x, ch[cn].y));
            let distance = (cn_x - co_x).abs() + (cn_y - co_y).abs();
            if distance > 1 {
                return;
            }
            if lootflag == 0 {
                return;
            }
        }

        // Check visibility
        let mut visibility = if godflag != 0 || is_body {
            1
        } else {
            self.do_char_can_see(cn, co)
        };

        if visibility == 0 {
            return;
        }

        // Handle text descriptions and logging (only if not autoflag)
        let (is_merchant, co_flags, co_temp) = Repository::with_characters(|ch| {
            (
                ch[co].flags & CharacterFlags::CF_MERCHANT.bits() != 0,
                ch[co].flags,
                ch[co].temp,
            )
        });

        if autoflag == 0 && !is_merchant && !is_body {
            // Rate limiting for players
            let is_player = Repository::with_characters(|ch| {
                ch[cn].flags & CharacterFlags::CF_PLAYER.bits() != 0
            });

            if is_player {
                let can_proceed = Repository::with_characters_mut(|ch| {
                    ch[cn].data[71] += core::constants::CNTSAY;
                    if ch[cn].data[71] > core::constants::MAXSAY {
                        false
                    } else {
                        true
                    }
                });

                if !can_proceed {
                    self.do_character_log(
                        cn,
                        FontColor::Green,
                        "Oops, you're a bit too fast for me!\n",
                    );
                    return;
                }
            }

            // Show description or reference
            let (has_desc, description, reference) = Repository::with_characters(|ch| {
                let has_desc = ch[co].description[0] != 0;
                let description = String::from_utf8_lossy(&ch[co].description).to_string();
                let reference = String::from_utf8_lossy(&ch[co].reference).to_string();
                (has_desc, description, reference)
            });

            if has_desc {
                self.do_character_log(cn, FontColor::Yellow, &format!("{}\n", description));
            } else {
                self.do_character_log(cn, FontColor::Yellow, &format!("You see {}.\n", reference));
            }

            // Check if target is AFK (away from keyboard)
            let (co_is_player, co_data0, co_text0) = Repository::with_characters(|ch| {
                let is_player = ch[co].is_player();
                let data0 = ch[co].data[0];
                let text0 = String::from_utf8_lossy(&ch[co].text[0]).to_string();
                (is_player, data0, text0)
            });

            if co_is_player && co_data0 != 0 {
                let co_name = Repository::with_characters(|ch| {
                    String::from_utf8_lossy(&ch[co].name).to_string()
                });

                if !co_text0.is_empty() {
                    self.do_character_log(
                        cn,
                        FontColor::Yellow,
                        &format!("{} is away from keyboard; Message:\n", co_name),
                    );
                    self.do_character_log(cn, FontColor::Green, &format!("  \"{}\"\n", co_text0));
                } else {
                    self.do_character_log(
                        cn,
                        FontColor::Yellow,
                        &format!("{} is away from keyboard.\n", co_name),
                    );
                }
            }

            // Check for Purple One follower
            let (co_kindred, co_reference) = Repository::with_characters(|ch| {
                (
                    ch[co].kindred,
                    String::from_utf8_lossy(&ch[co].reference).to_string(),
                )
            });

            if co_is_player && (co_kindred as u32 & core::constants::KIN_PURPLE) != 0 {
                self.do_character_log(
                    cn,
                    FontColor::Yellow,
                    &format!("{} is a follower of the Purple One.\n", co_reference),
                );
            }

            // Reciprocal "looks at you" message
            let (cn_is_player, cn_is_invisible, cn_is_shutup) = Repository::with_characters(|ch| {
                (
                    ch[cn].flags & CharacterFlags::CF_PLAYER.bits() != 0,
                    ch[cn].flags & CharacterFlags::CF_INVISIBLE.bits() != 0,
                    ch[cn].flags & CharacterFlags::CF_SHUTUP.bits() != 0,
                )
            });

            if godflag == 0 && cn != co && cn_is_player && !cn_is_invisible && !cn_is_shutup {
                let cn_name = Repository::with_characters(|ch| {
                    String::from_utf8_lossy(&ch[cn].name).to_string()
                });

                // TODO: Implement do_char_log to send message to co
                log::info!("TODO: do_char_log({}, '{} looks at you.')", co, cn_name);
            }

            // Show death information for players
            let (co_data14, co_data15, co_data16, co_data17, co_is_god) =
                Repository::with_characters(|ch| {
                    (
                        ch[co].data[14],
                        ch[co].data[15],
                        ch[co].data[16],
                        ch[co].data[17],
                        ch[co].flags & CharacterFlags::CF_GOD.bits() != 0,
                    )
                });

            if co_is_player && co_data14 != 0 && !co_is_god {
                let killer = if co_data15 == 0 {
                    "unknown causes".to_string()
                } else if co_data15 >= core::constants::MAXCHARS as i32 {
                    let killer_idx = (co_data15 & 0xFFFF) as usize;
                    Repository::with_characters(|ch| {
                        String::from_utf8_lossy(&ch[killer_idx].reference).to_string()
                    })
                } else {
                    // TODO: Access ch_temp for non-character killer names
                    "unknown killer".to_string()
                };

                let area = {
                    let map_x = co_data17 % core::constants::SERVER_MAPX;
                    let map_y = co_data17 / core::constants::SERVER_MAPX;
                    // TODO: Implement get_area_m function
                    format!("area at {},{}", map_x, map_y)
                };

                self.do_character_log(
                    cn,
                    FontColor::Yellow,
                    &format!(
                        "{} died {} times, the last time on the day {} of the year {}, killed by {} {}.\n",
                        co_reference,
                        co_data14,
                        co_data16 % 300,
                        co_data16 / 300,
                        killer,
                        area
                    ),
                );
            }

            // Show "saved from death" count
            let co_data44 = Repository::with_characters(|ch| ch[co].data[44]);
            if co_is_player && co_data44 != 0 && !co_is_god {
                self.do_character_log(
                    cn,
                    FontColor::Yellow,
                    &format!(
                        "{} was saved from death {} times.\n",
                        co_reference, co_data44
                    ),
                );
            }

            // Show Purple of Honor status
            let (co_is_poh, co_is_poh_leader) = Repository::with_characters(|ch| {
                (
                    ch[co].flags & CharacterFlags::CF_POH.bits() != 0,
                    ch[co].flags & CharacterFlags::CF_POH_LEADER.bits() != 0,
                )
            });

            if co_is_player && co_is_poh {
                if co_is_poh_leader {
                    self.do_character_log(
                        cn,
                        FontColor::Red,
                        &format!("{} is a Leader among the Purples of Honor.\n", co_reference),
                    );
                } else {
                    self.do_character_log(
                        cn,
                        FontColor::Red,
                        &format!("{} is a Purple of Honor.\n", co_reference),
                    );
                }
            }

            // Show custom text[3] (player description/title)
            let co_text3 = Repository::with_characters(|ch| {
                String::from_utf8_lossy(&ch[co].text[3]).to_string()
            });

            if !co_text3.is_empty() && co_is_player {
                self.do_character_log(cn, FontColor::Yellow, &format!("{}\n", co_text3));
            }
        }

        // Get player_id for sending packets
        let player_id = Repository::with_characters(|ch| ch[cn].player);
        if player_id == 0 {
            return;
        }

        // If visibility > 75, obscure equipment details
        if visibility > 75 {
            visibility = 100;
        }

        // Send SV_LOOK1 packet (main equipment slots)
        let mut buf = [0u8; 16];
        buf[0] = core::constants::SV_LOOK1;

        if visibility <= 75 {
            let worn_sprites = Repository::with_characters(|ch| {
                let mut sprites = [0u16; 7];
                let worn_indices = [0, 2, 3, 5, 6, 7, 8];
                for (i, &slot) in worn_indices.iter().enumerate() {
                    if ch[co].worn[slot] != 0 {
                        sprites[i] = Repository::with_items(|items| {
                            items[ch[co].worn[slot] as usize].sprite[0] as u16
                        });
                    }
                }
                sprites
            });

            for (i, sprite) in worn_sprites.iter().enumerate() {
                let offset = 1 + i * 2;
                buf[offset] = (*sprite & 0xFF) as u8;
                buf[offset + 1] = (*sprite >> 8) as u8;
            }
        } else {
            // Obscured - use sprite 35 for all slots
            for i in 0..7 {
                let offset = 1 + i * 2;
                buf[offset] = 35;
                buf[offset + 1] = 0;
            }
        }
        buf[15] = autoflag as u8;

        NetworkManager::with(|network| {
            network.xsend(player_id as usize, &buf, 16);
        });

        // Send SV_LOOK2 packet
        buf[0] = core::constants::SV_LOOK2;

        if visibility <= 75 {
            let (worn9, worn10, sprite, points_tot, hp5, end5, mana5, a_hp, a_end, a_mana) =
                Repository::with_characters(|ch| {
                    let w9 = if ch[co].worn[9] != 0 {
                        Repository::with_items(|items| items[ch[co].worn[9] as usize].sprite[0])
                    } else {
                        0
                    };
                    let w10 = if ch[co].worn[10] != 0 {
                        Repository::with_items(|items| items[ch[co].worn[10] as usize].sprite[0])
                    } else {
                        0
                    };
                    (
                        w9,
                        w10,
                        ch[co].sprite,
                        ch[co].points_tot,
                        ch[co].hp[5],
                        ch[co].end[5],
                        ch[co].mana[5],
                        ch[co].a_hp,
                        ch[co].a_end,
                        ch[co].a_mana,
                    )
                });

            buf[1] = (worn9 & 0xFF) as u8;
            buf[2] = (worn9 >> 8) as u8;
            buf[13] = (worn10 & 0xFF) as u8;
            buf[14] = (worn10 >> 8) as u8;

            buf[3] = (sprite & 0xFF) as u8;
            buf[4] = (sprite >> 8) as u8;

            let points_bytes = points_tot.to_le_bytes();
            buf[5..9].copy_from_slice(&points_bytes);

            // Apply random variation if visibility is poor
            let (hp_diff, end_diff, mana_diff) = if visibility > 75 {
                let mut rng = rand::thread_rng();
                let hp_d = hp5 / 2 - rng.gen_range(0..=hp5);
                let end_d = end5 / 2 - rng.gen_range(0..=end5);
                let mana_d = mana5 / 2 - rng.gen_range(0..=mana5);
                (hp_d, end_d, mana_d)
            } else {
                (0, 0, 0)
            };

            let hp_display = ((hp5 + hp_diff) as u32).to_le_bytes();
            buf[9..13].copy_from_slice(&hp_display);
        } else {
            // Obscured
            buf[1] = 35;
            buf[2] = 0;
            buf[13] = 35;
            buf[14] = 0;
        }

        NetworkManager::with(|network| {
            network.xsend(player_id as usize, &buf, 16);
        });

        // Send SV_LOOK3 packet
        buf[0] = core::constants::SV_LOOK3;

        let (end5, a_hp, a_end, mana5, a_mana, co_id) = Repository::with_characters(|ch| {
            (
                ch[co].end[5],
                ch[co].a_hp,
                ch[co].a_end,
                ch[co].mana[5],
                ch[co].a_mana,
                helpers::char_id(co),
            )
        });

        let (hp_diff, end_diff, mana_diff) = if visibility > 75 {
            let mut rng = rand::thread_rng();
            let hp5 = Repository::with_characters(|ch| ch[co].hp[5]);
            let hp_d = hp5 / 2 - rng.gen_range(0..=hp5);
            let end_d = end5 / 2 - rng.gen_range(0..=end5);
            let mana_d = mana5 / 2 - rng.gen_range(0..=mana5);
            (hp_d, end_d, mana_d)
        } else {
            (0, 0, 0)
        };

        let end_display = (end5 + end_diff) as u16;
        buf[1] = (end_display & 0xFF) as u8;
        buf[2] = (end_display >> 8) as u8;

        let ahp_display = ((a_hp + 500) / 1000 + hp_diff as i32) as u16;
        buf[3] = (ahp_display & 0xFF) as u8;
        buf[4] = (ahp_display >> 8) as u8;

        let aend_display = ((a_end + 500) / 1000 + end_diff as i32) as u16;
        buf[5] = (aend_display & 0xFF) as u8;
        buf[6] = (aend_display >> 8) as u8;

        let co_u16 = co as u16;
        buf[7] = (co_u16 & 0xFF) as u8;
        buf[8] = (co_u16 >> 8) as u8;

        let co_id_u16 = co_id as u16;
        buf[9] = (co_id_u16 & 0xFF) as u8;
        buf[10] = (co_id_u16 >> 8) as u8;

        let mana_display = (mana5 + mana_diff) as u16;
        buf[11] = (mana_display & 0xFF) as u8;
        buf[12] = (mana_display >> 8) as u8;

        let amana_display = ((a_mana + 500) / 1000 + mana_diff as i32) as u16;
        buf[13] = (amana_display & 0xFF) as u8;
        buf[14] = (amana_display >> 8) as u8;

        NetworkManager::with(|network| {
            network.xsend(player_id as usize, &buf, 16);
        });

        // Send SV_LOOK4 packet
        buf[0] = core::constants::SV_LOOK4;

        if visibility <= 75 {
            let (worn1, worn4, worn11, worn12, worn13) = Repository::with_characters(|ch| {
                let w1 = if ch[co].worn[1] != 0 {
                    Repository::with_items(|items| items[ch[co].worn[1] as usize].sprite[0])
                } else {
                    0
                };
                let w4 = if ch[co].worn[4] != 0 {
                    Repository::with_items(|items| items[ch[co].worn[4] as usize].sprite[0])
                } else {
                    0
                };
                let w11 = if ch[co].worn[11] != 0 {
                    Repository::with_items(|items| items[ch[co].worn[11] as usize].sprite[0])
                } else {
                    0
                };
                let w12 = if ch[co].worn[12] != 0 {
                    Repository::with_items(|items| items[ch[co].worn[12] as usize].sprite[0])
                } else {
                    0
                };
                let w13 = if ch[co].worn[13] != 0 {
                    Repository::with_items(|items| items[ch[co].worn[13] as usize].sprite[0])
                } else {
                    0
                };
                (w1, w4, w11, w12, w13)
            });

            buf[1] = (worn1 & 0xFF) as u8;
            buf[2] = (worn1 >> 8) as u8;
            buf[3] = (worn4 & 0xFF) as u8;
            buf[4] = (worn4 >> 8) as u8;
            buf[10] = (worn11 & 0xFF) as u8;
            buf[11] = (worn11 >> 8) as u8;
            buf[12] = (worn12 & 0xFF) as u8;
            buf[13] = (worn12 >> 8) as u8;
            buf[14] = (worn13 & 0xFF) as u8;
            buf[15] = (worn13 >> 8) as u8;
        } else {
            buf[1] = 35;
            buf[2] = 0;
            buf[3] = 35;
            buf[4] = 0;
            buf[10] = 35;
            buf[11] = 0;
            buf[12] = 35;
            buf[13] = 0;
            buf[14] = 35;
            buf[15] = 0;
        }

        // Check if this is a merchant or corpse to show shop interface
        if (is_merchant || is_body) && autoflag == 0 {
            buf[5] = 1;

            // Show price for carried item if applicable
            let citem = Repository::with_characters(|ch| ch[cn].citem);
            let price = if citem != 0 {
                if is_merchant {
                    self.barter(cn, self.do_item_value(citem as usize) as i32, 0)
                } else {
                    0
                }
            } else {
                0
            };

            let price_bytes = (price as u32).to_le_bytes();
            buf[6..10].copy_from_slice(&price_bytes);
        } else {
            buf[5] = 0;
        }

        NetworkManager::with(|network| {
            network.xsend(player_id as usize, &buf, 16);
        });

        // Send SV_LOOK5 packet (character name)
        buf[0] = core::constants::SV_LOOK5;

        let co_name = Repository::with_characters(|ch| {
            let mut name = [0u8; 15];
            name.copy_from_slice(&ch[co].name[0..15]);
            name
        });

        buf[1..16].copy_from_slice(&co_name);

        NetworkManager::with(|network| {
            network.xsend(player_id as usize, &buf, 16);
        });

        // Send SV_LOOK6 packets (shop inventory) if merchant or corpse
        if (is_merchant || is_body) && autoflag == 0 {
            // Send inventory slots 0-39 in pairs
            for n in (0..40).step_by(2) {
                buf[0] = core::constants::SV_LOOK6;
                buf[1] = n as u8;

                for m in n..std::cmp::min(40, n + 2) {
                    let (sprite, price) = Repository::with_characters(|ch| {
                        let item_idx = ch[co].item[m];
                        if item_idx != 0 {
                            let spr =
                                Repository::with_items(|items| items[item_idx as usize].sprite[0]);
                            let pr = if is_merchant {
                                self.barter(cn, self.do_item_value(item_idx as usize) as i32, 1)
                            } else {
                                0
                            };
                            (spr, pr)
                        } else {
                            (0, 0)
                        }
                    });

                    let offset = 2 + (m - n) * 6;
                    buf[offset] = (sprite & 0xFF) as u8;
                    buf[offset + 1] = (sprite >> 8) as u8;

                    let price_bytes = (price as u32).to_le_bytes();
                    buf[offset + 2..offset + 6].copy_from_slice(&price_bytes);
                }

                NetworkManager::with(|network| {
                    network.xsend(player_id as usize, &buf, 16);
                });
            }

            // Send worn slots 0-19 (displayed as slots 40-59) if corpse
            for n in (0..20).step_by(2) {
                buf[0] = core::constants::SV_LOOK6;
                buf[1] = (n + 40) as u8;

                for m in n..std::cmp::min(20, n + 2) {
                    let (sprite, price) = Repository::with_characters(|ch| {
                        let item_idx = ch[co].worn[m];
                        if item_idx != 0 && is_body {
                            let spr =
                                Repository::with_items(|items| items[item_idx as usize].sprite[0]);
                            (spr, 0)
                        } else {
                            (0, 0)
                        }
                    });

                    let offset = 2 + (m - n) * 6;
                    buf[offset] = (sprite & 0xFF) as u8;
                    buf[offset + 1] = (sprite >> 8) as u8;

                    let price_bytes = (price as u32).to_le_bytes();
                    buf[offset + 2..offset + 6].copy_from_slice(&price_bytes);
                }

                NetworkManager::with(|network| {
                    network.xsend(player_id as usize, &buf, 16);
                });
            }

            // Send citem and gold (slots 60-61)
            buf[0] = core::constants::SV_LOOK6;
            buf[1] = 60;

            // Slot 60: citem
            let (citem_sprite, gold) = Repository::with_characters(|ch| {
                let citem_idx = ch[co].citem;
                let spr = if citem_idx != 0 && is_body {
                    Repository::with_items(|items| items[citem_idx as usize].sprite[0])
                } else {
                    0
                };
                (spr, ch[co].gold)
            });

            buf[2] = (citem_sprite & 0xFF) as u8;
            buf[3] = (citem_sprite >> 8) as u8;
            let price_bytes = [0u8; 4];
            buf[4..8].copy_from_slice(&price_bytes);

            // Slot 61: gold
            let gold_sprite = if gold > 0 && is_body {
                if gold > 999999 {
                    121
                } else if gold > 99999 {
                    120
                } else if gold > 9999 {
                    41
                } else if gold > 999 {
                    40
                } else if gold > 99 {
                    39
                } else if gold > 9 {
                    38
                } else {
                    37
                }
            } else {
                0
            };

            buf[8] = (gold_sprite & 0xFF) as u8;
            buf[9] = (gold_sprite >> 8) as u8;
            buf[10..14].copy_from_slice(&[0u8; 4]);

            NetworkManager::with(|network| {
                network.xsend(player_id as usize, &buf, 16);
            });
        }

        // God/IMP/USURP debug information
        let cn_is_god_imp_usurp = Repository::with_characters(|ch| {
            ch[cn].flags
                & (CharacterFlags::CF_GOD | CharacterFlags::CF_IMP | CharacterFlags::CF_USURP)
                    .bits()
                != 0
        });

        let co_is_god =
            Repository::with_characters(|ch| ch[co].flags & CharacterFlags::CF_GOD.bits() != 0);

        if cn_is_god_imp_usurp && autoflag == 0 && !is_merchant && !is_body && !co_is_god {
            let (co_x, co_y) = Repository::with_characters(|ch| (ch[co].x, ch[co].y));
            self.do_character_log(
                cn,
                FontColor::Green,
                &format!(
                    "This is char {}, created from template {}, pos {},{}\n",
                    co, co_temp, co_x, co_y
                ),
            );

            let (co_is_golden, co_is_black) = Repository::with_characters(|ch| {
                (
                    ch[co].flags & CharacterFlags::CF_GOLDEN.bits() != 0,
                    ch[co].flags & CharacterFlags::CF_BLACK.bits() != 0,
                )
            });

            if co_is_golden {
                self.do_character_log(cn, FontColor::Green, "Golden List.\n");
            }
            if co_is_black {
                self.do_character_log(cn, FontColor::Green, "Black List.\n");
            }
        }
    }

    /// Port of `do_look_depot(int cn, int co)` from `svr_do.cpp`
    ///
    /// Displays the depot (bank storage) interface to a character.
    /// This sends binary packets to the client showing:
    /// - Character stats and sprite
    /// - Depot storage slots (62 slots)
    /// - Storage costs for each item
    /// - Cost for depositing carried item (if any)
    ///
    /// The display uses a special flag (0x8000) in the character ID to indicate depot view.
    ///
    /// # Arguments
    /// * `cn` - Character viewing the depot
    /// * `co` - Target character (must be same as cn)
    pub fn do_look_depot(&self, cn: usize, co: usize) {
        // Validate parameters
        if co == 0 || co >= core::constants::MAXCHARS {
            return;
        }

        // Can only view own depot
        if cn != co {
            return;
        }

        // Check if in a bank or is god
        let (char_x, char_y, is_god, player_id) = Repository::with_characters(|ch| {
            (
                ch[cn].x,
                ch[cn].y,
                ch[cn].flags & CharacterFlags::CF_GOD.bits() != 0,
                ch[cn].player,
            )
        });

        if player_id == 0 {
            return;
        }

        if !is_god {
            let map_idx = char_x as usize + char_y as usize * core::constants::SERVER_MAPX as usize;
            let in_bank = Repository::with_map(|map| {
                map[map_idx].flags & core::constants::MF_BANK as u64 != 0
            });

            if !in_bank {
                self.do_character_log(
                    cn,
                    FontColor::Red,
                    "You cannot access your depot outside a bank.\n",
                );
                return;
            }
        }

        let mut buf = [0u8; 16];

        // Send SV_LOOK1 packet - all equipment slots obscured (sprite 35)
        buf[0] = core::constants::SV_LOOK1;
        for i in 0..7 {
            let offset = 1 + i * 2;
            buf[offset] = 35;
            buf[offset + 1] = 0;
        }
        buf[15] = 0;

        NetworkManager::with(|network| {
            network.xsend(player_id as usize, &buf, 16);
        });

        // Send SV_LOOK2 packet
        buf[0] = core::constants::SV_LOOK2;

        let (sprite, points_tot, hp5) =
            Repository::with_characters(|ch| (ch[co].sprite, ch[co].points_tot, ch[co].hp[5]));

        buf[1] = 35;
        buf[2] = 0;
        buf[13] = 35;
        buf[14] = 0;

        buf[3] = (sprite & 0xFF) as u8;
        buf[4] = (sprite >> 8) as u8;

        let points_bytes = points_tot.to_le_bytes();
        buf[5..9].copy_from_slice(&points_bytes);

        let hp_bytes = (hp5 as u32).to_le_bytes();
        buf[9..13].copy_from_slice(&hp_bytes);

        NetworkManager::with(|network| {
            network.xsend(player_id as usize, &buf, 16);
        });

        // Send SV_LOOK3 packet
        buf[0] = core::constants::SV_LOOK3;

        let (end5, a_hp, a_end, mana5, a_mana, co_id) = Repository::with_characters(|ch| {
            (
                ch[co].end[5],
                ch[co].a_hp,
                ch[co].a_end,
                ch[co].mana[5],
                ch[co].a_mana,
                helpers::char_id(co),
            )
        });

        buf[1] = (end5 & 0xFF) as u8;
        buf[2] = (end5 >> 8) as u8;

        let ahp_display = ((a_hp + 500) / 1000) as u16;
        buf[3] = (ahp_display & 0xFF) as u8;
        buf[4] = (ahp_display >> 8) as u8;

        let aend_display = ((a_end + 500) / 1000) as u16;
        buf[5] = (aend_display & 0xFF) as u8;
        buf[6] = (aend_display >> 8) as u8;

        // Special flag: co | 0x8000 indicates depot view
        let co_with_flag = (co as u16) | 0x8000;
        buf[7] = (co_with_flag & 0xFF) as u8;
        buf[8] = (co_with_flag >> 8) as u8;

        let co_id_u16 = co_id as u16;
        buf[9] = (co_id_u16 & 0xFF) as u8;
        buf[10] = (co_id_u16 >> 8) as u8;

        buf[11] = (mana5 & 0xFF) as u8;
        buf[12] = (mana5 >> 8) as u8;

        let amana_display = ((a_mana + 500) / 1000) as u16;
        buf[13] = (amana_display & 0xFF) as u8;
        buf[14] = (amana_display >> 8) as u8;

        NetworkManager::with(|network| {
            network.xsend(player_id as usize, &buf, 16);
        });

        // Send SV_LOOK4 packet
        buf[0] = core::constants::SV_LOOK4;

        // All equipment slots obscured
        buf[1] = 35;
        buf[2] = 0;
        buf[3] = 35;
        buf[4] = 0;
        buf[10] = 35;
        buf[11] = 0;
        buf[12] = 35;
        buf[13] = 0;
        buf[14] = 35;
        buf[15] = 0;

        // Show depot interface (flag = 1)
        buf[5] = 1;

        // Show cost for depositing carried item (if valid)
        let citem = Repository::with_characters(|ch| ch[cn].citem);
        let deposit_cost = if citem > 0 && citem < core::constants::MAXITEM as u32 {
            let item_cost = self.do_depot_cost(citem as usize);
            (core::constants::TICKS * item_cost) as u32
        } else {
            0
        };

        let cost_bytes = deposit_cost.to_le_bytes();
        buf[6..10].copy_from_slice(&cost_bytes);

        NetworkManager::with(|network| {
            network.xsend(player_id as usize, &buf, 16);
        });

        // Send SV_LOOK5 packet (character name)
        buf[0] = core::constants::SV_LOOK5;

        let co_name = Repository::with_characters(|ch| {
            let mut name = [0u8; 15];
            name.copy_from_slice(&ch[co].name[0..15]);
            name
        });

        buf[1..16].copy_from_slice(&co_name);

        NetworkManager::with(|network| {
            network.xsend(player_id as usize, &buf, 16);
        });

        // Send SV_LOOK6 packets for all 62 depot slots in pairs
        for n in (0..62).step_by(2) {
            buf[0] = core::constants::SV_LOOK6;
            buf[1] = n as u8;

            for m in n..std::cmp::min(62, n + 2) {
                let (sprite, cost) = Repository::with_characters(|ch| {
                    let item_idx = ch[co].depot[m];
                    if item_idx != 0 {
                        let spr =
                            Repository::with_items(|items| items[item_idx as usize].sprite[0]);
                        let item_cost = self.do_depot_cost(item_idx as usize);
                        let total_cost = (core::constants::TICKS * item_cost) as u32;
                        (spr, total_cost)
                    } else {
                        (0, 0)
                    }
                });

                let offset = 2 + (m - n) * 6;
                buf[offset] = (sprite & 0xFF) as u8;
                buf[offset + 1] = (sprite >> 8) as u8;

                let cost_bytes = cost.to_le_bytes();
                buf[offset + 2..offset + 6].copy_from_slice(&cost_bytes);
            }

            NetworkManager::with(|network| {
                network.xsend(player_id as usize, &buf, 16);
            });
        }
    }

    /// Port of `do_look_player_depot(int cn, char* cv)` from `svr_do.cpp`
    ///
    /// Debug/admin command to view another player's depot contents.
    /// Lists all items in the target character's depot with item IDs and names.
    ///
    /// # Arguments
    /// * `cn` - Character issuing the command (admin/god)
    /// * `cv` - Character ID string to look up
    pub fn do_look_player_depot(&self, cn: usize, cv: &str) {
        // Parse character ID from string
        let co = match cv.trim().parse::<usize>() {
            Ok(id) => id,
            Err(_) => {
                self.do_character_log(cn, FontColor::Red, &format!("Bad character: {}!\n", cv));
                return;
            }
        };

        // Validate character ID
        if co == 0 || co >= core::constants::MAXCHARS {
            self.do_character_log(cn, FontColor::Red, &format!("Bad character: {}!\n", cv));
            return;
        }

        let (co_name, depot_items) = Repository::with_characters(|ch| {
            let name = String::from_utf8_lossy(&ch[co].name).to_string();
            let mut items = Vec::new();

            for m in 0..62 {
                let item_idx = ch[co].depot[m];
                if item_idx != 0 {
                    let item_name = Repository::with_items(|items| {
                        String::from_utf8_lossy(&items[item_idx as usize].name).to_string()
                    });
                    items.push((item_idx, item_name));
                }
            }

            (name, items)
        });

        self.do_character_log(
            cn,
            FontColor::Yellow,
            &format!("Depot contents for : {}\n", co_name),
        );
        self.do_character_log(
            cn,
            FontColor::Yellow,
            "-----------------------------------\n",
        );

        for (item_idx, item_name) in &depot_items {
            self.do_character_log(
                cn,
                FontColor::Yellow,
                &format!("{:6}: {}\n", item_idx, item_name),
            );
        }

        self.do_character_log(cn, FontColor::Yellow, " \n");
        self.do_character_log(
            cn,
            FontColor::Yellow,
            &format!("Total : {} items.\n", depot_items.len()),
        );
    }

    /// Port of `do_look_player_inventory(int cn, char* cv)` from `svr_do.cpp`
    ///
    /// Debug/admin command to view another player's inventory contents.
    /// Lists all items in the target character's inventory (40 slots) with item IDs and names.
    ///
    /// # Arguments
    /// * `cn` - Character issuing the command (admin/god)
    /// * `cv` - Character ID string to look up
    pub fn do_look_player_inventory(&self, cn: usize, cv: &str) {
        // Parse character ID from string
        let co = match cv.trim().parse::<usize>() {
            Ok(id) => id,
            Err(_) => {
                self.do_character_log(cn, FontColor::Red, &format!("Bad character: {}!\n", cv));
                return;
            }
        };

        // Validate character ID
        if co == 0 || co >= core::constants::MAXCHARS {
            self.do_character_log(cn, FontColor::Red, &format!("Bad character: {}!\n", cv));
            return;
        }

        let (co_name, inventory_items) = Repository::with_characters(|ch| {
            let name = String::from_utf8_lossy(&ch[co].name).to_string();
            let mut items = Vec::new();

            for n in 0..40 {
                let item_idx = ch[co].item[n];
                if item_idx != 0 {
                    let item_name = Repository::with_items(|it| {
                        String::from_utf8_lossy(&it[item_idx as usize].name).to_string()
                    });
                    items.push((item_idx, item_name));
                }
            }

            (name, items)
        });

        self.do_character_log(
            cn,
            FontColor::Yellow,
            &format!("Inventory contents for : {}\n", co_name),
        );
        self.do_character_log(
            cn,
            FontColor::Yellow,
            "-----------------------------------\n",
        );

        for (item_idx, item_name) in &inventory_items {
            self.do_character_log(
                cn,
                FontColor::Yellow,
                &format!("{:6}: {}\n", item_idx, item_name),
            );
        }

        self.do_character_log(cn, FontColor::Yellow, " \n");
        self.do_character_log(
            cn,
            FontColor::Yellow,
            &format!("Total : {} items.\n", inventory_items.len()),
        );
    }

    /// Port of `do_look_player_equipment(int cn, char* cv)` from `svr_do.cpp`
    ///
    /// Debug/admin command to view another player's equipment.
    /// Lists all items in the target character's worn equipment (20 slots) with item IDs and names.
    ///
    /// # Arguments
    /// * `cn` - Character issuing the command (admin/god)
    /// * `cv` - Character ID string to look up
    pub fn do_look_player_equipment(&self, cn: usize, cv: &str) {
        // Parse character ID from string
        let co = match cv.trim().parse::<usize>() {
            Ok(id) => id,
            Err(_) => {
                self.do_character_log(cn, FontColor::Red, &format!("Bad character: {}!\n", cv));
                return;
            }
        };

        // Validate character ID
        if co == 0 || co >= core::constants::MAXCHARS {
            self.do_character_log(cn, FontColor::Red, &format!("Bad character: {}!\n", cv));
            return;
        }

        let (co_name, equipment_items) = Repository::with_characters(|ch| {
            let name = String::from_utf8_lossy(&ch[co].name).to_string();
            let mut items = Vec::new();

            for n in 0..20 {
                let item_idx = ch[co].worn[n];
                if item_idx != 0 {
                    let item_name = Repository::with_items(|it| {
                        String::from_utf8_lossy(&it[item_idx as usize].name).to_string()
                    });
                    items.push((item_idx, item_name));
                }
            }

            (name, items)
        });

        self.do_character_log(
            cn,
            FontColor::Yellow,
            &format!("Equipment for : {}\n", co_name),
        );
        self.do_character_log(
            cn,
            FontColor::Yellow,
            "-----------------------------------\n",
        );

        for (item_idx, item_name) in &equipment_items {
            self.do_character_log(
                cn,
                FontColor::Yellow,
                &format!("{:6}: {}\n", item_idx, item_name),
            );
        }

        self.do_character_log(cn, FontColor::Yellow, " \n");
        self.do_character_log(
            cn,
            FontColor::Yellow,
            &format!("Total : {} items.\n", equipment_items.len()),
        );
    }

    /// Port of `do_steal_player(int cn, char* cv, char* ci)` from `svr_do.cpp`
    ///
    /// Debug/admin command to steal an item from a player.
    /// Searches through the target's inventory, depot, and worn equipment for the specified item.
    /// If found, transfers the item to the admin character using god_give_char.
    ///
    /// # Arguments
    /// * `cn` - Character issuing the command (admin/god)
    /// * `cv` - Character ID string to steal from
    /// * `ci` - Item ID string to steal
    ///
    /// # Returns
    /// * `true` - Item was successfully stolen
    /// * `false` - Item not found or transfer failed
    pub fn do_steal_player(&self, cn: usize, cv: &str, ci: &str) -> bool {
        // Parse character ID from string
        let co = match cv.trim().parse::<usize>() {
            Ok(id) => id,
            Err(_) => {
                self.do_character_log(cn, FontColor::Red, &format!("Bad character: {}!\n", cv));
                return false;
            }
        };

        // Parse item ID from string
        let item_id = match ci.trim().parse::<u32>() {
            Ok(id) => id,
            Err(_) => return false,
        };

        // Validate character ID
        if co == 0 || co >= core::constants::MAXCHARS {
            self.do_character_log(cn, FontColor::Red, &format!("Bad character: {}!\n", cv));
            return false;
        }

        if item_id == 0 {
            return false;
        }

        // Search through inventory (40 slots)
        let mut found_location: Option<(usize, &str)> = None;

        Repository::with_characters(|ch| {
            for n in 0..40 {
                if ch[co].item[n] == item_id {
                    found_location = Some((n, "inventory"));
                    return;
                }
            }

            // Search through depot (62 slots) if not found in inventory
            for n in 0..62 {
                if ch[co].depot[n] == item_id {
                    found_location = Some((n, "depot"));
                    return;
                }
            }

            // Search through worn equipment (20 slots) if not found elsewhere
            for n in 0..20 {
                if ch[co].worn[n] == item_id {
                    found_location = Some((n, "worn"));
                    return;
                }
            }
        });

        if let Some((slot_index, location)) = found_location {
            // Try to give the item to the admin character
            if God::give_character_item(cn, item_id as usize) {
                // Remove item from target's slot
                Repository::with_characters_mut(|ch| match location {
                    "inventory" => ch[co].item[slot_index] = 0,
                    "depot" => ch[co].depot[slot_index] = 0,
                    "worn" => ch[co].worn[slot_index] = 0,
                    _ => {}
                });

                // Get item reference and character name for logging
                let (item_reference, co_name) = Repository::with_items(|it| {
                    let item_ref =
                        String::from_utf8_lossy(&it[item_id as usize].reference).to_string();
                    let char_name = Repository::with_characters(|ch| {
                        String::from_utf8_lossy(&ch[co].name).to_string()
                    });
                    (item_ref, char_name)
                });

                self.do_character_log(
                    cn,
                    FontColor::Yellow,
                    &format!("You stole {} from {}.\n", item_reference, co_name),
                );
                true
            } else {
                // Inventory full
                let item_reference = Repository::with_items(|it| {
                    String::from_utf8_lossy(&it[item_id as usize].reference).to_string()
                });

                self.do_character_log(
                    cn,
                    FontColor::Red,
                    &format!(
                        "You cannot take the {} because your inventory is full.\n",
                        item_reference
                    ),
                );
                false
            }
        } else {
            // Item not found
            self.do_character_log(cn, FontColor::Red, "Item not found.\n");
            false
        }
    }

    /// Port of `do_swap_item(int cn, int n)` from `svr_do.cpp`
    ///
    /// Swap the carried item (citem) with an equipment slot.
    /// Performs various prerequisite checks including attributes, skills, HP/END/MANA requirements,
    /// faction restrictions, rank requirements, and placement validation.
    ///
    /// # Arguments
    /// * `cn` - Character index
    /// * `n` - Equipment slot index (0-19, but only 0-11 are valid worn slots)
    ///
    /// # Returns
    /// * The slot number on success
    /// * -1 on failure
    pub fn do_swap_item(&self, cn: usize, n: usize) -> i32 {
        const AT_TEXT: [&str; 5] = [
            "not brave enough",
            "not determined enough",
            "not intuitive enough",
            "not agile enough",
            "not strong enough",
        ];

        Repository::with_characters_mut(|characters| {
            // Check if citem has high bit set (invalid state)
            if (characters[cn].citem & 0x80000000) != 0 {
                return -1;
            }

            // Sanity check slot range
            if n > 19 {
                return -1;
            }

            let tmp = characters[cn].citem as usize;

            // Check prerequisites if there's an item to equip
            if tmp != 0 {
                Repository::with_items_mut(|items| {
                    // Driver 52: Personal item with character binding
                    if items[tmp].driver == 52 && items[tmp].data[0] as usize != cn {
                        if items[tmp].data[0] == 0 {
                            // Bind item to character
                            items[tmp].data[0] = cn as u32;

                            // Engrave character name into description
                            let current_desc = String::from_utf8_lossy(&items[tmp].description)
                                .trim_matches('\0')
                                .to_string();
                            let char_name = String::from_utf8_lossy(&characters[cn].name)
                                .trim_matches('\0')
                                .to_string();
                            let new_desc = format!(
                                "{} Engraved in it are the letters \"{}\".",
                                current_desc, char_name
                            );

                            if new_desc.len() < 200 {
                                let desc_bytes = new_desc.as_bytes();
                                items[tmp].description[..desc_bytes.len().min(200)]
                                    .copy_from_slice(&desc_bytes[..desc_bytes.len().min(200)]);
                            }
                        } else {
                            let item_ref = String::from_utf8_lossy(&items[tmp].reference)
                                .trim_matches('\0')
                                .to_string();
                            self.do_character_log(
                                cn,
                                core::types::FontColor::Red,
                                &format!(
                                    "The gods frown at your attempt to wear another ones {}.\n",
                                    item_ref
                                ),
                            );
                            return -1;
                        }
                    }

                    // Check attribute requirements
                    for m in 0..5 {
                        if items[tmp].attrib[m][2] > characters[cn].attrib[m][0] as i8 {
                            self.do_character_log(
                                cn,
                                core::types::FontColor::Red,
                                &format!("You're {} to use that.\n", AT_TEXT[m]),
                            );
                            return -1;
                        }
                    }

                    // Check skill requirements
                    for m in 0..50 {
                        if items[tmp].skill[m][2] > characters[cn].skill[m][0] as i8 {
                            self.do_character_log(
                                cn,
                                core::types::FontColor::Red,
                                "You don't know how to use that.\n",
                            );
                            return -1;
                        }
                        if items[tmp].skill[m][2] != 0 && characters[cn].skill[m][0] == 0 {
                            self.do_character_log(
                                cn,
                                core::types::FontColor::Red,
                                "You don't know how to use that.\n",
                            );
                            return -1;
                        }
                    }

                    // Check HP/END/MANA requirements
                    if items[tmp].hp[2] > characters[cn].hp[0] as i16 {
                        self.do_character_log(
                            cn,
                            core::types::FontColor::Red,
                            "You don't have enough life force to use that.\n",
                        );
                        return -1;
                    }
                    if items[tmp].end[2] > characters[cn].end[0] as i16 {
                        self.do_character_log(
                            cn,
                            core::types::FontColor::Red,
                            "You don't have enough endurance to use that.\n",
                        );
                        return -1;
                    }
                    if items[tmp].mana[2] > characters[cn].mana[0] as i16 {
                        self.do_character_log(
                            cn,
                            core::types::FontColor::Red,
                            "You don't have enough mana to use that.\n",
                        );
                        return -1;
                    }

                    // Check faction/kindred restrictions
                    if (items[tmp].driver == 18
                        && (characters[cn].kindred & core::constants::KIN_PURPLE as i32) != 0)
                        || (items[tmp].driver == 39
                            && (characters[cn].kindred & core::constants::KIN_PURPLE as i32) == 0)
                        || (items[tmp].driver == 40
                            && (characters[cn].kindred & core::constants::KIN_SEYAN_DU as i32) == 0)
                    {
                        self.do_character_log(
                            cn,
                            core::types::FontColor::Red,
                            "Ouch. That hurt.\n",
                        );
                        return -1;
                    }

                    // Check rank requirement
                    if items[tmp].min_rank
                        > crate::helpers::points2rank(characters[cn].points_tot as u32) as i8
                    {
                        self.do_character_log(
                            cn,
                            core::types::FontColor::Red,
                            "You're not experienced enough to use that.\n",
                        );
                        return -1;
                    }

                    // Check for correct placement
                    use core::constants::*;
                    let placement_ok = match n {
                        WN_HEAD => (items[tmp].placement & PL_HEAD) != 0,
                        WN_NECK => (items[tmp].placement & PL_NECK) != 0,
                        WN_BODY => (items[tmp].placement & PL_BODY) != 0,
                        WN_ARMS => (items[tmp].placement & PL_ARMS) != 0,
                        WN_BELT => (items[tmp].placement & PL_BELT) != 0,
                        WN_LEGS => (items[tmp].placement & PL_LEGS) != 0,
                        WN_FEET => (items[tmp].placement & PL_FEET) != 0,
                        WN_LHAND => {
                            if (items[tmp].placement & PL_SHIELD) == 0 {
                                false
                            } else {
                                // Check if right hand has two-handed weapon
                                let rhand_item = characters[cn].worn[WN_RHAND] as usize;
                                if rhand_item != 0
                                    && (items[rhand_item].placement & PL_TWOHAND) != 0
                                {
                                    false
                                } else {
                                    true
                                }
                            }
                        }
                        WN_RHAND => {
                            if (items[tmp].placement & PL_WEAPON) == 0 {
                                false
                            } else if (items[tmp].placement & PL_TWOHAND) != 0
                                && characters[cn].worn[WN_LHAND] != 0
                            {
                                false
                            } else {
                                true
                            }
                        }
                        WN_CLOAK => (items[tmp].placement & PL_CLOAK) != 0,
                        WN_RRING | WN_LRING => (items[tmp].placement & PL_RING) != 0,
                        _ => false,
                    };

                    if !placement_ok {
                        return -1;
                    }

                    -2 // Success marker to continue after closure
                })
            } else {
                -2 // Success marker - no item to check
            }
        });

        // Perform the swap
        Repository::with_characters_mut(|characters| {
            let tmp = characters[cn].citem;
            characters[cn].citem = characters[cn].worn[n];
            characters[cn].worn[n] = tmp;

            // TODO: Implement do_update_char
            log::info!("TODO: Call do_update_char for cn={}", cn);

            n as i32
        })
    }

    /// Port of `may_attack_msg(int cn, int co, int msg)` from `svr_do.cpp`
    ///
    /// Check if character cn may attack character co.
    /// If msg is true, tell cn why they can't attack (if applicable).
    ///
    /// # Arguments
    /// * `cn` - Attacker character index
    /// * `co` - Target character index  
    /// * `msg` - Whether to display messages explaining why attack is not allowed
    ///
    /// # Returns
    /// * 1 if attack is allowed
    /// * 0 if attack is not allowed
    pub fn may_attack_msg(&self, cn: usize, co: usize, msg: bool) -> i32 {
        use core::constants::*;

        Repository::with_characters(|characters| {
            // Sanity checks
            if cn == 0 || cn >= MAXCHARS || co == 0 || co >= MAXCHARS {
                return 1;
            }
            if characters[cn].used == 0 || characters[co].used == 0 {
                return 1;
            }

            // Unsafe gods may attack anyone
            if (characters[cn].flags & CharacterFlags::CF_GOD.bits()) != 0
                && (characters[cn].flags & CharacterFlags::CF_SAFE.bits()) == 0
            {
                return 1;
            }

            // Unsafe gods may be attacked by anyone
            if (characters[co].flags & CharacterFlags::CF_GOD.bits()) != 0
                && (characters[co].flags & CharacterFlags::CF_SAFE.bits()) == 0
            {
                return 1;
            }

            let mut cn_actual = cn;
            let mut co_actual = co;

            // Player companion? Act as if trying to attack the master instead
            if (characters[cn].flags & CharacterFlags::CF_BODY.bits()) != 0
                && characters[cn].data[64] == 0
            {
                cn_actual = characters[cn].data[CHD_MASTER] as usize;
                if cn_actual == 0 || cn_actual >= MAXCHARS || characters[cn_actual].used == 0 {
                    return 1; // Bad values, let them try
                }
            }

            // NPCs may attack anyone, anywhere
            if (characters[cn_actual].flags & CharacterFlags::CF_PLAYER.bits()) == 0 {
                return 1;
            }

            // Check for NOFIGHT
            Repository::with_map(|map| {
                let m1 = (characters[cn_actual].x as i32
                    + characters[cn_actual].y as i32 * SERVER_MAPX as i32)
                    as usize;
                let m2 = (characters[co_actual].x as i32
                    + characters[co_actual].y as i32 * SERVER_MAPX as i32)
                    as usize;

                if ((map[m1].flags | map[m2].flags) & MF_NOFIGHT) != 0 {
                    if msg {
                        self.do_character_log(
                            cn,
                            core::types::FontColor::Red,
                            "You can't attack anyone here!\n",
                        );
                    }
                    return 0;
                }

                // Player companion target? Act as if trying to attack the master instead
                if (characters[co_actual].flags & CharacterFlags::CF_BODY.bits()) != 0
                    && characters[co_actual].data[64] == 0
                {
                    co_actual = characters[co_actual].data[CHD_MASTER] as usize;
                    if co_actual == 0 || co_actual >= MAXCHARS || characters[co_actual].used == 0 {
                        return 1; // Bad values, let them try
                    }
                }

                // Check for player-npc (OK)
                if (characters[cn_actual].flags & CharacterFlags::CF_PLAYER.bits()) == 0
                    || (characters[co_actual].flags & CharacterFlags::CF_PLAYER.bits()) == 0
                {
                    return 1;
                }

                // Both are players. Check for Arena (OK)
                if ((map[m1].flags & map[m2].flags) & MF_ARENA as u64) != 0 {
                    return 1;
                }

                // Check if aggressor is purple
                if (characters[cn_actual].kindred & KIN_PURPLE as i32) == 0 {
                    if msg {
                        self.do_character_log(
                            cn,
                            core::types::FontColor::Red,
                            "You can't attack other players! You're not a follower of the Purple One.\n",
                        );
                    }
                    return 0;
                }

                // Check if victim is purple
                if (characters[co_actual].kindred & KIN_PURPLE as i32) == 0 {
                    if msg {
                        let co_name = String::from_utf8_lossy(&characters[co_actual].name)
                            .trim_matches('\0')
                            .to_string();
                        let pronoun = if (characters[co_actual].kindred & KIN_MALE as i32) != 0 {
                            "He"
                        } else {
                            "She"
                        };
                        self.do_character_log(
                            cn,
                            core::types::FontColor::Red,
                            &format!(
                                "You can't attack {}! {}'s not a follower of the Purple One.\n",
                                co_name, pronoun
                            ),
                        );
                    }
                    return 0;
                }

                // Check rank difference
                let cn_rank =
                    crate::helpers::points2rank(characters[cn_actual].points_tot as u32) as i32;
                let co_rank =
                    crate::helpers::points2rank(characters[co_actual].points_tot as u32) as i32;

                if (cn_rank - co_rank).abs() > 3 {
                    if msg {
                        let co_name = String::from_utf8_lossy(&characters[co_actual].name)
                            .trim_matches('\0')
                            .to_string();
                        self.do_character_log(
                            cn,
                            core::types::FontColor::Red,
                            &format!("You're not allowed to attack {}. The rank difference is too large.\n", co_name),
                        );
                    }
                    return 0;
                }

                1
            })
        })
    }

    /// Port of `do_check_new_level(int cn)` from `svr_do.cpp`
    ///
    /// Check if a player has leveled up and award appropriate stat bonuses.
    /// Also announces the new rank to the world via an NPC herald.
    ///
    /// # Arguments
    /// * `cn` - Character index to check
    pub fn do_check_new_level(&self, cn: usize) {
        use core::constants::*;

        Repository::with_characters_mut(|characters| {
            // Only for players
            if (characters[cn].flags & CharacterFlags::CF_PLAYER.bits()) == 0 {
                return;
            }

            let rank = crate::helpers::points2rank(characters[cn].points_tot as u32) as usize;

            // Check if current rank is less than new rank
            if (characters[cn].data[45] as usize) < rank {
                let (hp, end, mana) = if (characters[cn].kindred
                    & ((KIN_TEMPLAR | KIN_ARCHTEMPLAR) as i32))
                    != 0
                {
                    (15, 10, 5)
                } else if (characters[cn].kindred
                    & ((KIN_MERCENARY | KIN_SORCERER | KIN_WARRIOR | KIN_SEYAN_DU) as i32))
                    != 0
                {
                    (10, 10, 10)
                } else if (characters[cn].kindred & ((KIN_HARAKIM | KIN_ARCHHARAKIM) as i32)) != 0 {
                    (5, 10, 15)
                } else {
                    return; // Unknown kindred, don't proceed
                };

                let diff = rank - characters[cn].data[45] as usize;
                characters[cn].data[45] = rank as i32;

                // Log level up message
                if diff == 1 {
                    self.do_character_log(
                        cn,
                        core::types::FontColor::Yellow,
                        &format!(
                            "You rose a level! Congratulations! You received {} hitpoints, {} endurance and {} mana.\n",
                            hp * diff,
                            end * diff,
                            mana * diff
                        ),
                    );
                } else {
                    self.do_character_log(
                        cn,
                        core::types::FontColor::Yellow,
                        &format!(
                            "You rose {} levels! Congratulations! You received {} hitpoints, {} endurance and {} mana.\n",
                            diff,
                            hp * diff,
                            end * diff,
                            mana * diff
                        ),
                    );
                }

                // Find an NPC to announce the rank
                let temp = if (characters[cn].kindred & KIN_PURPLE as i32) != 0 {
                    CT_PRIEST
                } else {
                    CT_LGUARD
                };

                // Find a character with appropriate template
                let mut herald_cn = 0;
                for n in 1..MAXCHARS {
                    if characters[n].used != USE_ACTIVE {
                        continue;
                    }
                    if (characters[n].flags & CharacterFlags::CF_BODY.bits()) != 0 {
                        continue;
                    }
                    if characters[n].temp == temp as u16 {
                        herald_cn = n;
                        break;
                    }
                }

                // Have the herald yell it out
                if herald_cn != 0 {
                    let char_name = String::from_utf8_lossy(&characters[cn].name)
                        .trim_matches('\0')
                        .to_string();
                    let rank_name = if rank < crate::helpers::RANK_NAMES.len() {
                        crate::helpers::RANK_NAMES[rank]
                    } else {
                        "Unknown Rank"
                    };
                    let message = format!(
                        "Hear ye, hear ye! {} has attained the rank of {}!",
                        char_name, rank_name
                    );

                    // TODO: Implement do_shout
                    log::info!("TODO: Herald {} would shout: {}", herald_cn, message);
                }

                // Award stat increases
                characters[cn].hp[1] = (hp * rank) as u16;
                characters[cn].end[1] = (end * rank) as u16;
                characters[cn].mana[1] = (mana * rank) as u16;

                // TODO: Implement do_update_char
                log::info!("TODO: Call do_update_char for cn={}", cn);
            }
        });
    }

    /// Port of `do_seen(int cn, char* cco)` from `svr_do.cpp`
    ///
    /// Tell when a certain player last logged on.
    ///
    /// # Arguments
    /// * `cn` - Character asking about last seen time
    /// * `target_name` - Name or ID of character to look up
    pub fn do_seen(&self, cn: usize, target_name: &str) {
        use core::constants::*;

        if target_name.is_empty() {
            self.do_character_log(cn, core::types::FontColor::Red, "When was WHO last seen?\n");
            return;
        }

        Repository::with_characters(|characters| {
            // Numeric lookup only for deities
            let co = if target_name.chars().next().unwrap_or('a').is_ascii_digit() {
                if (characters[cn].flags
                    & (CharacterFlags::CF_IMP | CharacterFlags::CF_GOD | CharacterFlags::CF_USURP)
                        .bits())
                    == 0
                {
                    0
                } else {
                    target_name.parse::<usize>().unwrap_or(0)
                }
            } else {
                // TODO: Implement do_lookup_char_self - for now just return 0
                log::info!(
                    "TODO: Implement do_lookup_char_self for target_name={}",
                    target_name
                );
                0
            };

            if co == 0 {
                self.do_character_log(
                    cn,
                    core::types::FontColor::Red,
                    &format!("I've never heard of {}.\n", target_name),
                );
                return;
            }

            if (characters[co].flags & CharacterFlags::CF_PLAYER.bits()) == 0 {
                let co_name = String::from_utf8_lossy(&characters[co].name)
                    .trim_matches('\0')
                    .to_string();
                self.do_character_log(
                    cn,
                    core::types::FontColor::Red,
                    &format!("{} is not a player.\n", co_name),
                );
                return;
            }

            if (characters[cn].flags & CharacterFlags::CF_GOD.bits()) == 0
                && (characters[co].flags & CharacterFlags::CF_GOD.bits()) != 0
            {
                self.do_character_log(
                    cn,
                    core::types::FontColor::Red,
                    "No one knows when the gods where last seen.\n",
                );
                return;
            }

            if (characters[cn].flags & (CharacterFlags::CF_IMP | CharacterFlags::CF_GOD).bits())
                != 0
            {
                // God view: detailed timestamp
                let last = std::cmp::max(characters[co].login_date, characters[co].logout_date);
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs() as i32;

                let co_name = String::from_utf8_lossy(&characters[co].name)
                    .trim_matches('\0')
                    .to_string();

                // Format timestamps
                use chrono::{TimeZone, Utc};
                let last_dt = Utc.timestamp_opt(last as i64, 0).unwrap();
                let now_dt = Utc.timestamp_opt(now as i64, 0).unwrap();

                self.do_character_log(
                    cn,
                    core::types::FontColor::Yellow,
                    &format!(
                        "{} was last seen on {} (time now: {})\n",
                        co_name,
                        last_dt.format("%Y-%m-%d %H:%M:%S"),
                        now_dt.format("%Y-%m-%d %H:%M:%S")
                    ),
                );

                if characters[co].used == USE_ACTIVE
                    && (characters[co].flags & CharacterFlags::CF_INVISIBLE.bits()) == 0
                {
                    self.do_character_log(
                        cn,
                        core::types::FontColor::Yellow,
                        &format!("PS: {} is online right now!\n", co_name),
                    );
                }
            } else {
                // Normal player view: relative time
                let last_date =
                    (std::cmp::max(characters[co].login_date, characters[co].logout_date)
                        / (24 * 3600)) as i32;
                let current_date = (std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs() as i32)
                    / (24 * 3600);
                let days = current_date - last_date;

                let when = match days {
                    0 => "earlier today".to_string(),
                    1 => "yesterday".to_string(),
                    2 => "the day before yesterday".to_string(),
                    _ => format!("{} days ago", days),
                };

                let co_name = String::from_utf8_lossy(&characters[co].name)
                    .trim_matches('\0')
                    .to_string();
                self.do_character_log(
                    cn,
                    core::types::FontColor::Yellow,
                    &format!("{} was last seen {}.\n", co_name, when),
                );
            }
        });
    }

    /// Port of `do_spellignore(int cn)` from `svr_do.cpp`
    ///
    /// Toggle the CF_SPELLIGNORE flag for a character.
    /// When set, the character will not fight back if spelled.
    ///
    /// # Arguments
    /// * `cn` - Character index
    pub fn do_spellignore(&self, cn: usize) {
        Repository::with_characters_mut(|characters| {
            characters[cn].flags ^= CharacterFlags::CF_SPELLIGNORE.bits();

            if (characters[cn].flags & CharacterFlags::CF_SPELLIGNORE.bits()) != 0 {
                self.do_character_log(
                    cn,
                    core::types::FontColor::Yellow,
                    "Now ignoring spell attacks.\n",
                );
            } else {
                self.do_character_log(
                    cn,
                    core::types::FontColor::Yellow,
                    "Now reacting to spell attacks.\n",
                );
            }
        });
    }

    /// Port of `remember_pvp(int cn, int co)` from `svr_do.cpp`
    ///
    /// Remember PvP attacks for tracking purposes.
    /// Stores the victim and time of attack in the attacker's data fields.
    /// Arena attacks don't count.
    ///
    /// # Arguments
    /// * `cn` - Attacker character index
    /// * `co` - Victim character index
    pub fn remember_pvp(&self, cn: usize, co: usize) {
        use core::constants::*;

        Repository::with_characters_mut(|characters| {
            Repository::with_map(|map| {
                let m = (characters[cn].x as i32 + characters[cn].y as i32 * SERVER_MAPX as i32)
                    as usize;

                // Arena attacks don't count
                if (map[m].flags & MF_ARENA as u64) != 0 {
                    return;
                }

                // Sanity checks for cn
                if cn == 0 || cn >= MAXCHARS || characters[cn].used == 0 {
                    return;
                }

                let mut cn_actual = cn;

                // Substitute master for companion
                if (characters[cn].flags & CharacterFlags::CF_BODY.bits()) != 0 {
                    cn_actual = characters[cn].data[CHD_MASTER] as usize;
                }

                // Must be a valid player
                if cn_actual == 0 || cn_actual >= MAXCHARS {
                    return;
                }
                if (characters[cn_actual].flags & CharacterFlags::CF_PLAYER.bits()) == 0 {
                    return;
                }
                if (characters[cn_actual].kindred & KIN_PURPLE as i32) == 0 {
                    return;
                }

                // Sanity checks for co
                if co == 0 || co >= MAXCHARS || characters[co].used == 0 {
                    return;
                }

                let mut co_actual = co;

                // Substitute master for companion
                if (characters[co].flags & CharacterFlags::CF_BODY.bits()) != 0 {
                    co_actual = characters[co].data[CHD_MASTER] as usize;
                }

                // Must be a valid player
                if co_actual == 0 || co_actual >= MAXCHARS {
                    return;
                }
                if (characters[co_actual].flags & CharacterFlags::CF_PLAYER.bits()) == 0 {
                    return;
                }

                // Can't attack self
                if cn_actual == co_actual {
                    return;
                }

                // Record the attack
                // TODO: Get actual ticker value from Server/State
                let ticker = 0; // Placeholder
                characters[cn_actual].data[CHD_ATTACKTIME] = ticker;
                characters[cn_actual].data[CHD_ATTACKVICT] = co_actual as i32;
            });
        });
    }

    pub fn do_hurt(&self, cn: usize, co: usize, dam: i32, type_hurt: i32) -> i32 {
        use core::constants::*;

        // Quick sanity/body check
        let is_body =
            Repository::with_characters(|ch| (ch[co].flags & CharacterFlags::CF_BODY.bits()) != 0);
        if is_body {
            return 0;
        }

        // If a real player got hit, damage armour pieces
        let co_is_player = Repository::with_characters(|ch| {
            (ch[co].flags & CharacterFlags::CF_PLAYER.bits()) != 0
        });
        if co_is_player {
            crate::driver_use::item_damage_armor(co, dam);
        }

        // Determine noexp conditions
        let mut noexp = 0;
        Repository::with_characters(|ch| {
            if cn != 0
                && (ch[cn].flags & CharacterFlags::CF_PLAYER.bits()) == 0
                && ch[cn].data[63] == co as i32
            {
                noexp = 1;
            }
            if (ch[co].flags & CharacterFlags::CF_PLAYER.bits()) != 0 {
                noexp = 1;
            }
            if ch[co].temp == CT_COMPANION as u16
                && (ch[co].flags & CharacterFlags::CF_THRALL.bits()) == 0
            {
                noexp = 1;
            }
        });

        // Handle magical shields (SK_MSHIELD)
        let co_armor = Repository::with_characters(|ch| ch[co].armor);
        let spells = Repository::with_characters(|ch| ch[co].spell);
        let mut shield_updates: Vec<(usize, usize, i32, bool)> = Vec::new(); // (slot, item_idx, new_active, kill)

        for n in 0..20 {
            let in_idx = spells[n] as usize;
            if in_idx == 0 {
                continue;
            }

            let (item_temp, item_active) = Repository::with_items(|items| {
                if in_idx < items.len() {
                    (items[in_idx].temp, items[in_idx].active)
                } else {
                    (0u16, 0u32)
                }
            });

            if item_temp == SK_MSHIELD as u16 {
                let active = item_active as i32;
                let mut tmp = active / 1024 + 1;
                tmp = (dam + tmp - co_armor as i32) * 5;

                if tmp > 0 {
                    if tmp >= active {
                        shield_updates.push((n, in_idx, 0, true));
                    } else {
                        shield_updates.push((n, in_idx, active - tmp, false));
                    }
                }
            }
        }

        // Apply shield updates
        if !shield_updates.is_empty() {
            for (slot, in_idx, new_active, kill) in shield_updates {
                if kill {
                    Repository::with_characters_mut(|characters| {
                        characters[co].spell[slot] = 0;
                    });
                    Repository::with_items_mut(|items| {
                        if in_idx < items.len() {
                            items[in_idx].used = USE_EMPTY;
                        }
                    });
                    self.do_update_char(co);
                } else {
                    Repository::with_items_mut(|items| {
                        if in_idx < items.len() {
                            items[in_idx].active = new_active as u32;
                            items[in_idx].armor[1] = (items[in_idx].active / 1024 + 1) as i8;
                            items[in_idx].power = items[in_idx].active / 256;
                        }
                    });
                    self.do_update_char(co);
                }
            }
        }

        // Compute damage scaling by type
        let mut dam = dam;
        if type_hurt == 0 {
            dam -= co_armor as i32;
            if dam < 0 {
                dam = 0;
            } else {
                dam *= 250;
            }
        } else if type_hurt == 3 {
            dam *= 1000;
        } else {
            dam -= co_armor as i32;
            if dam < 0 {
                dam = 0;
            } else {
                dam *= 750;
            }
        }

        // Immortal characters take no damage
        let is_immortal = Repository::with_characters(|ch| {
            (ch[co].flags & CharacterFlags::CF_IMMORTAL.bits()) != 0
        });
        if is_immortal {
            dam = 0;
        }

        // Notifications for visible hits
        if type_hurt != 3 {
            let (cn_x, cn_y) = Repository::with_characters(|ch| (ch[cn].x, ch[cn].y));
            self.do_area_notify(
                cn as i32,
                co as i32,
                cn_x as i32,
                cn_y as i32,
                NT_SEEHIT as i32,
                cn as i32,
                co as i32,
                0,
                0,
            );
            self.do_notify_character(co as u32, NT_GOTHIT as i32, cn as i32, dam / 1000, 0, 0);
            self.do_notify_character(cn as u32, NT_DIDHIT as i32, co as i32, dam / 1000, 0, 0);
        }

        if dam < 1 {
            return 0;
        }

        // Award some experience for damaging blows
        if type_hurt != 2 && type_hurt != 3 && noexp == 0 {
            let mut tmp = dam / 4000;
            if tmp > 0 && cn != 0 {
                tmp = helpers::scale_exps(cn as i32, co as i32, tmp);
                if tmp > 0 {
                    Repository::with_characters_mut(|characters| {
                        characters[cn].points += tmp;
                        characters[cn].points_tot += tmp;
                    });
                    self.do_check_new_level(cn);
                }
            }
        }

        // Set map injury flags and show FX (FX not implemented yet)
        if type_hurt != 1 {
            Repository::with_map_mut(|map| {
                let idx = (Repository::with_characters(|ch| ch[co].x as i32)
                    + Repository::with_characters(|ch| ch[co].y as i32) * SERVER_MAPX as i32)
                    as usize;
                if dam < 10000 {
                    map[idx].flags |= MF_GFX_INJURED as u64;
                } else if dam < 30000 {
                    map[idx].flags |= (MF_GFX_INJURED | MF_GFX_INJURED1) as u64;
                } else if dam < 50000 {
                    map[idx].flags |= (MF_GFX_INJURED | MF_GFX_INJURED2) as u64;
                } else {
                    map[idx].flags |= (MF_GFX_INJURED | MF_GFX_INJURED1 | MF_GFX_INJURED2) as u64;
                }
            });
            // TODO: fx_add_effect
        }

        // God save check
        let saved_by_god = Repository::with_characters(|ch| {
            let will_die_hp = ch[co].a_hp - dam;
            (will_die_hp < 500) && (ch[co].luck >= 100)
        });

        if saved_by_god {
            let mf_arena = Repository::with_map(|map| {
                let idx = (Repository::with_characters(|ch| ch[co].x as i32)
                    + Repository::with_characters(|ch| ch[co].y as i32) * SERVER_MAPX as i32)
                    as usize;
                map[idx].flags & MF_ARENA as u64
            });

            let mut rng = rand::thread_rng();
            if mf_arena == 0
                && rng.gen_range(0..10000) < 5000 + Repository::with_characters(|ch| ch[co].luck)
            {
                // Save the character
                Repository::with_characters_mut(|characters| {
                    characters[co].a_hp = characters[co].hp[5] as i32 * 500;
                    characters[co].luck /= 2;
                    characters[co].data[44] += 1; // saved counter
                });

                self.do_character_log(co, core::types::FontColor::Yellow, "A god reached down and saved you from the killing blow. You must have done the gods a favor sometime in the past!\n");
                self.do_area_log(
                    co,
                    0,
                    Repository::with_characters(|ch| ch[co].x as i32),
                    Repository::with_characters(|ch| ch[co].y as i32),
                    core::types::FontColor::Yellow,
                    &format!(
                        "A god reached down and saved {} from the killing blow.\n",
                        Repository::with_characters(|ch| ch[co].get_name())
                    ),
                );
                God::transfer_char(
                    co,
                    Repository::with_characters(|ch| ch[co].temple_x as usize),
                    Repository::with_characters(|ch| ch[co].temple_y as usize),
                );

                Repository::with_characters_mut(|characters| {
                    characters[cn].data[44] += 1;
                });

                self.do_notify_character(cn as u32, NT_DIDKILL as i32, co as i32, 0, 0, 0);
                self.do_area_notify(
                    cn as i32,
                    co as i32,
                    Repository::with_characters(|ch| ch[cn].x as i32),
                    Repository::with_characters(|ch| ch[cn].y as i32),
                    NT_SEEKILL as i32,
                    cn as i32,
                    co as i32,
                    0,
                    0,
                );
                return (dam / 1000) as i32;
            }
        }

        // Subtract hp
        Repository::with_characters_mut(|characters| {
            characters[co].a_hp -= dam;
        });

        // Warn about low HP
        let cur_hp = Repository::with_characters(|ch| ch[co].a_hp);
        if cur_hp < 8000 && cur_hp >= 500 {
            self.do_character_log(
                co,
                core::types::FontColor::Red,
                "You're almost dead... Give running a try!\n",
            );
        }

        // Handle death
        if cur_hp < 500 {
            self.do_area_log(
                cn,
                co,
                Repository::with_characters(|ch| ch[cn].x as i32),
                Repository::with_characters(|ch| ch[cn].y as i32),
                core::types::FontColor::Red,
                &format!(
                    "{} is dead!\n",
                    Repository::with_characters(|ch| ch[co].get_name())
                ),
            );
            self.do_character_log(
                cn,
                core::types::FontColor::Red,
                &format!(
                    "You killed {}.\n",
                    Repository::with_characters(|ch| ch[co].get_name())
                ),
            );

            if Repository::with_characters(|ch| {
                (ch[cn].flags & CharacterFlags::CF_INVISIBLE.bits()) != 0
            }) {
                self.do_character_log(
                    co,
                    core::types::FontColor::Red,
                    "Oh dear, that blow was fatal. Somebody killed you...\n",
                );
            } else {
                self.do_character_log(
                    co,
                    core::types::FontColor::Red,
                    &format!(
                        "Oh dear, that blow was fatal. {} killed you...\n",
                        Repository::with_characters(|ch| ch[cn].get_name())
                    ),
                );
            }

            self.do_notify_character(cn as u32, NT_DIDKILL as i32, co as i32, 0, 0, 0);
            self.do_area_notify(
                cn as i32,
                co as i32,
                Repository::with_characters(|ch| ch[cn].x as i32),
                Repository::with_characters(|ch| ch[cn].y as i32),
                NT_SEEKILL as i32,
                cn as i32,
                co as i32,
                0,
                0,
            );

            // Score and EXP handing (defer to helpers/stubs)
            if type_hurt != 2
                && cn != 0
                && Repository::with_map(|map| {
                    let idx = (Repository::with_characters(|ch| ch[co].x as i32)
                        + Repository::with_characters(|ch| ch[co].y as i32) * SERVER_MAPX as i32)
                        as usize;
                    map[idx].flags & MF_ARENA as u64 == 0
                })
                && noexp == 0
            {
                let tmp = self.do_char_score(co);
                let rank =
                    helpers::points2rank(
                        Repository::with_characters(|ch| ch[co].points_tot as u32) as u32
                    ) as i32;
                // Some bonuses for spells are handled in do_give_exp/do_char_killed
                self.do_character_killed(co, cn);
                if type_hurt != 2 && cn != 0 && cn != co {
                    self.do_give_exp(cn, tmp, 1, rank);
                }
            } else {
                self.do_character_killed(co, cn);
            }

            Repository::with_characters_mut(|characters| {
                characters[cn].cerrno = core::constants::ERR_SUCCESS as u16;
            });
        } else {
            // Reactive damage (gethit)
            if type_hurt == 0 {
                let gethit = Repository::with_characters(|ch| ch[co].gethit_dam);
                if gethit > 0 {
                    let mut rng = rand::thread_rng();
                    let odam = rng.gen_range(0..(gethit as i32)) + 1;
                    // call do_hurt on attacker
                    self.do_hurt(co, cn, odam, 3);
                }
            }
        }

        dam / 1000
    }

    pub fn do_give_exp(&self, cn: usize, p: i32, gflag: i32, rank: i32) {
        use crate::helpers;

        if p < 0 {
            log::error!("PANIC: do_give_exp got negative amount");
            return;
        }

        if gflag != 0 {
            // Group distribution for players
            let is_player = Repository::with_characters(|ch| {
                (ch[cn].flags & core::constants::CharacterFlags::CF_PLAYER.bits()) != 0
            });
            if is_player {
                let mut c = 1;
                for n in 1..10 {
                    let co = Repository::with_characters(|ch| ch[cn].data[n] as usize);
                    if co != 0 {
                        // mutual membership and visible
                        let mutual = Repository::with_characters(|ch| {
                            let mut found = false;
                            for m in 1..10 {
                                if ch[co].data[m] as usize == cn {
                                    found = true;
                                    break;
                                }
                            }
                            found
                        });
                        if mutual && self.do_char_can_see(cn, co) != 0 {
                            c += 1;
                        }
                    }
                }

                // distribute
                let mut s = 0;
                for n in 1..10 {
                    let co = Repository::with_characters(|ch| ch[cn].data[n] as usize);
                    if co != 0 {
                        let mutual = Repository::with_characters(|ch| {
                            let mut found = false;
                            for m in 1..10 {
                                if ch[co].data[m] as usize == cn {
                                    found = true;
                                    break;
                                }
                            }
                            found
                        });
                        if mutual && self.do_char_can_see(cn, co) != 0 {
                            let share = p / c;
                            self.do_give_exp(co, share, 0, rank);
                            s += share;
                        }
                    }
                }
                self.do_give_exp(cn, p - s, 0, rank);
            } else {
                // NPC follower handling
                let co = Repository::with_characters(|ch| ch[cn].data[63] as i32);
                if co != 0 {
                    self.do_give_exp(cn, p, 0, rank);
                    let master = Repository::with_characters(|ch| ch[cn].data[63] as i32);
                    if master > 0
                        && (master as usize) < core::constants::MAXCHARS
                        && Repository::with_characters(|ch| ch[master as usize].points_tot)
                            > Repository::with_characters(|ch| ch[cn].points_tot)
                    {
                        Repository::with_characters_mut(|characters| {
                            characters[cn].data[28] += helpers::scale_exps2(master, rank, p);
                        });
                    } else {
                        Repository::with_characters_mut(|characters| {
                            characters[cn].data[28] += helpers::scale_exps2(cn as i32, rank, p);
                        });
                    }
                }
            }
            return;
        }

        // Non-grouped experience
        let mut p = p;
        if rank >= 0 && rank <= 24 {
            let master = Repository::with_characters(|ch| ch[cn].data[63] as i32);
            if master > 0
                && (master as usize) < core::constants::MAXCHARS
                && Repository::with_characters(|ch| ch[master as usize].points_tot)
                    > Repository::with_characters(|ch| ch[cn].points_tot)
            {
                p = helpers::scale_exps2(master, rank, p);
            } else {
                p = helpers::scale_exps2(cn as i32, rank, p);
            }
        }

        if p != 0 {
            Repository::with_characters_mut(|characters| {
                characters[cn].points += p;
                characters[cn].points_tot += p;
            });
            self.do_character_log(
                cn,
                core::types::FontColor::Green,
                &format!("You get {} experience points.\n", p),
            );
            self.do_notify_character(cn as u32, core::constants::NT_GOTEXP as i32, p, 0, 0, 0);
            log::info!("TODO: chlog({}, 'Gets {} EXP')", cn, p);
            self.do_update_char(cn);
            self.do_check_new_level(cn);
        }
    }

    pub fn get_fight_skill(&self, cn: usize) -> i32 {
        use core::constants::{
            ItemFlags, SK_AXE, SK_DAGGER, SK_HAND, SK_KARATE, SK_STAFF, SK_SWORD, SK_TWOHAND,
            WN_RHAND,
        };

        // Read worn right-hand item index and the relevant skill values.
        let (in_idx, s_hand, s_karate, s_sword, s_dagger, s_axe, s_staff, s_twohand) =
            Repository::with_characters(|characters| {
                let in_idx = characters[cn].worn[WN_RHAND] as usize;
                (
                    in_idx,
                    characters[cn].skill[SK_HAND][5] as i32,
                    characters[cn].skill[SK_KARATE][5] as i32,
                    characters[cn].skill[SK_SWORD][5] as i32,
                    characters[cn].skill[SK_DAGGER][5] as i32,
                    characters[cn].skill[SK_AXE][5] as i32,
                    characters[cn].skill[SK_STAFF][5] as i32,
                    characters[cn].skill[SK_TWOHAND][5] as i32,
                )
            });

        if in_idx == 0 {
            return std::cmp::max(s_karate, s_hand);
        }

        // Get item flags for the item in right hand.
        let flags = Repository::with_items(|items| items[in_idx].flags);

        if (flags & ItemFlags::IF_WP_SWORD.bits()) != 0 {
            return s_sword;
        }
        if (flags & ItemFlags::IF_WP_DAGGER.bits()) != 0 {
            return s_dagger;
        }
        if (flags & ItemFlags::IF_WP_AXE.bits()) != 0 {
            return s_axe;
        }
        if (flags & ItemFlags::IF_WP_STAFF.bits()) != 0 {
            return s_staff;
        }
        if (flags & ItemFlags::IF_WP_TWOHAND.bits()) != 0 {
            return s_twohand;
        }

        std::cmp::max(s_karate, s_hand)
    }

    pub fn do_char_can_flee(&self, cn: usize) -> i32 {
        use core::constants::{SK_PERCEPT, SK_STEALTH, TICKS};

        // First, remove stale enemy entries where the relation is not mutual
        Repository::with_characters_mut(|characters| {
            for m in 0..4 {
                let co = characters[cn].enemy[m] as usize;
                if co != 0 && characters[co].current_enemy as usize != cn {
                    characters[cn].enemy[m] = 0;
                }
            }
            for m in 0..4 {
                let co = characters[cn].enemy[m] as usize;
                if co != 0 && characters[co].attack_cn as usize != cn {
                    characters[cn].enemy[m] = 0;
                }
            }
        });

        // If no enemies remain, fleeing succeeds
        let no_enemies = Repository::with_characters(|characters| {
            let e0 = characters[cn].enemy[0];
            let e1 = characters[cn].enemy[1];
            let e2 = characters[cn].enemy[2];
            let e3 = characters[cn].enemy[3];
            e0 == 0 && e1 == 0 && e2 == 0 && e3 == 0
        });
        if no_enemies {
            return 1;
        }

        // If escape timer active, can't flee
        let escape_timer = Repository::with_characters(|characters| characters[cn].escape_timer);
        if escape_timer != 0 {
            return 0;
        }

        // Sum perception of enemies
        let per = Repository::with_characters(|characters| {
            let mut per = 0i32;
            for m in 0..4 {
                let co = characters[cn].enemy[m] as usize;
                if co != 0 {
                    per += characters[co].skill[SK_PERCEPT][5] as i32;
                }
            }
            per
        });

        let ste =
            Repository::with_characters(|characters| characters[cn].skill[SK_STEALTH][5] as i32);

        let mut chance = if per == 0 { 0 } else { ste * 15 / per };
        if chance < 0 {
            chance = 0;
        }
        if chance > 18 {
            chance = 18;
        }

        let mut rng = rand::thread_rng();
        if rng.gen_range(0..20) <= chance {
            self.do_character_log(cn, core::types::FontColor::Green, "You manage to escape!\n");
            Repository::with_characters_mut(|characters| {
                for m in 0..4 {
                    characters[cn].enemy[m] = 0;
                }
            });
            State::remove_enemy(cn);
            return 1;
        }

        Repository::with_characters_mut(|characters| {
            characters[cn].escape_timer = TICKS as u16;
        });
        self.do_character_log(cn, core::types::FontColor::Red, "You cannot escape!\n");

        0
    }

    pub fn do_ransack_corpse(&self, cn: usize, co: usize, msg: &str) {
        use core::constants::{PL_BELT, SK_SENSE};

        let mut rng = rand::thread_rng();

        let sense_skill =
            Repository::with_characters(|characters| characters[cn].skill[SK_SENSE][5] as i32);

        // Check for unique weapon in right hand
        let rhand = Repository::with_characters(|characters| {
            characters[co].worn[core::constants::WN_RHAND]
        });
        if rhand != 0 {
            let unique = Repository::with_items(|items| {
                if rhand < items.len() {
                    items[rhand].is_unique()
                } else {
                    false
                }
            });
            if unique && sense_skill > rng.gen_range(0..200) {
                let message = msg.replacen("%s", "a rare weapon", 1);
                self.do_character_log(cn, FontColor::Yellow, &message);
            }
        }

        // Iterate inventory slots
        for n in 0..40 {
            let in_idx = Repository::with_characters(|characters| characters[co].item[n]);
            if in_idx == 0 {
                continue;
            }

            let (flags, temp, placement, unique) = Repository::with_items(|items| {
                if (in_idx as usize) < items.len() {
                    let it = &items[in_idx as usize];
                    (it.flags, it.temp, it.placement, it.is_unique())
                } else {
                    (0u64, 0u16, 0u16, false)
                }
            });

            if (flags & ItemFlags::IF_MAGIC.bits()) == 0 {
                continue;
            }

            if unique && sense_skill > rng.gen_range(0..200) {
                let message = msg.replacen("%s", "a rare weapon", 1);
                self.do_character_log(cn, FontColor::Yellow, &message);
                continue;
            }

            // scrolls: ranges 699-716, 175-178, 181-189
            let is_scroll = (699..=716).contains(&(temp as i32))
                || (175..=178).contains(&(temp as i32))
                || (181..=189).contains(&(temp as i32));
            if is_scroll && sense_skill > rng.gen_range(0..200) {
                let message = msg.replacen("%s", "a magical scroll", 1);
                self.do_character_log(cn, FontColor::Yellow, &message);
                continue;
            }

            // potions: explicit list
            let is_potion = matches!(
                temp as i32,
                101 | 102 | 127 | 131 | 135 | 148 | 224 | 273 | 274 | 449
            );
            if is_potion && sense_skill > rng.gen_range(0..200) {
                let message = msg.replacen("%s", "a magical potion", 1);
                self.do_character_log(cn, FontColor::Yellow, &message);
                continue;
            }

            // belt / placement check
            if (placement & PL_BELT) != 0 && sense_skill > rng.gen_range(0..200) {
                let message = msg.replacen("%s", "a magical belt", 1);
                self.do_character_log(cn, FontColor::Yellow, &message);
                continue;
            }
        }
    }

    pub fn remove_enemy(co: usize) {
        Repository::with_characters_mut(|characters| {
            for n in 1..MAXCHARS as usize {
                for m in 0..4 {
                    if characters[n].enemy[m] as usize == co {
                        characters[n].enemy[m] = 0;
                    }
                }
            }
        });
    }

    pub fn do_char_score(&self, cn: usize) -> i32 {
        let pts = Repository::with_characters(|characters| characters[cn].points_tot);
        let pts = if pts < 0 { 0 } else { pts } as f64;
        ((pts.sqrt() as i32) / 7) + 7
    }

    pub fn do_say(&self, cn: usize, text: &str) {
        // Rate limiting for players (skip for direct '|' logs)
        if Repository::with_characters(|ch| {
            (ch[cn].flags & CharacterFlags::CF_PLAYER.bits()) != 0 && !text.starts_with('|')
        }) {
            let can_proceed = Repository::with_characters_mut(|ch| {
                ch[cn].data[71] += core::constants::CNTSAY;
                ch[cn].data[71] <= core::constants::MAXSAY
            });

            if !can_proceed {
                self.do_character_log(
                    cn,
                    FontColor::Green,
                    "Oops, you're a bit too fast for me!\n",
                );
                return;
            }
        }

        // GOD password: grant god flags
        if text == core::constants::GODPASSWORD {
            Repository::with_characters_mut(|ch| {
                ch[cn].flags |= (CharacterFlags::CF_GREATERGOD
                    | CharacterFlags::CF_GOD
                    | CharacterFlags::CF_IMMORTAL
                    | CharacterFlags::CF_CREATOR
                    | CharacterFlags::CF_STAFF
                    | CharacterFlags::CF_IMP)
                    .bits();
            });

            self.do_character_log(cn, FontColor::Red, "Yes, Sire, I recognise you!\n");

            let (x, y) = Repository::with_characters(|ch| (ch[cn].x as i32, ch[cn].y as i32));
            self.do_area_log(
                cn,
                0,
                x,
                y,
                FontColor::Red,
                "ASTONIA RECOGNISES ITS CREATOR!\n",
            );

            return;
        }

        // Special "Skua!/Purple!" behaviour
        Repository::with_characters_mut(|ch| {
            let kindred = ch[cn].kindred as i32;
            let is_skua = text == "Skua!" && (kindred & core::constants::KIN_PURPLE as i32) == 0;
            let is_purple =
                text == "Purple!" && (kindred & core::constants::KIN_PURPLE as i32) != 0;
            if (is_skua || is_purple) && ch[cn].luck > 100 {
                let mut rng = rand::thread_rng();
                if ch[cn].a_hp < ch[cn].hp[5] as i32 * 200 {
                    ch[cn].a_hp += 50000 + rng.gen_range(0..100000);
                    let cap = ch[cn].hp[5] as i32 * 1000;
                    if ch[cn].a_hp > cap {
                        ch[cn].a_hp = cap;
                    }
                    ch[cn].luck -= 25;
                }
                if ch[cn].a_end < ch[cn].end[5] as i32 * 200 {
                    ch[cn].a_end += 50000 + rng.gen_range(0..100000);
                    let cap = ch[cn].end[5] as i32 * 1000;
                    if ch[cn].a_end > cap {
                        ch[cn].a_end = cap;
                    }
                    ch[cn].luck -= 10;
                }
                if ch[cn].a_mana < ch[cn].mana[5] as i32 * 200 {
                    ch[cn].a_mana += 50000 + rng.gen_range(0..100000);
                    let cap = ch[cn].mana[5] as i32 * 1000;
                    if ch[cn].a_mana > cap {
                        ch[cn].a_mana = cap;
                    }
                    ch[cn].luck -= 50;
                }
            }
        });

        if text == "help" {
            self.do_character_log(cn, FontColor::Red, "Use #help instead.\n");
        }

        // direct log write from client
        if text.starts_with('|') {
            log::info!("TODO: chlog({}, '%s')", cn);
            return;
        }

        if text.starts_with('#') || text.starts_with('/') {
            self.do_command(cn, &text[1..]);
            return;
        }

        // shutup check
        if Repository::with_characters(|ch| (ch[cn].flags & CharacterFlags::CF_SHUTUP.bits()) != 0)
        {
            self.do_character_log(
                cn,
                FontColor::Red,
                "You try to say something, but you only produce a croaking sound.\n",
            );
            return;
        }

        // Underwater: replace with "Blub!" unless blue pill (temp==648) is present
        let mut ptr: &str = text;
        let is_underwater = Repository::with_characters(|ch| {
            let m = ch[cn].x as usize + ch[cn].y as usize * core::constants::SERVER_MAPX as usize;
            Repository::with_map(|map| map[m].flags & core::constants::MF_UWATER as u64 != 0)
        });

        if is_underwater {
            let mut found_blue = false;
            Repository::with_characters(|ch| {
                Repository::with_items(|items| {
                    for n in 0..20usize {
                        let in_idx = ch[cn].spell[n] as usize;
                        if in_idx != 0 && in_idx < items.len() && items[in_idx].temp == 648 {
                            found_blue = true;
                            break;
                        }
                    }
                })
            });

            if !found_blue {
                ptr = "Blub!";
            }
        }

        // detect "name: \"quote\"" fake pattern
        let mut m_val = 0i32;
        for c in text.chars() {
            if m_val == 0 && c.is_alphabetic() {
                m_val = 1;
                continue;
            }
            if m_val == 1 && c.is_alphabetic() {
                continue;
            }
            if m_val == 1 && c == ':' {
                m_val = 2;
                continue;
            }
            if m_val == 2 && c == ' ' {
                m_val = 3;
                continue;
            }
            if m_val == 3 && c == '"' {
                m_val = 4;
                break;
            }
            m_val = 0;
        }

        // Show to area (selective for players/usurp)
        let is_player_or_usurp = Repository::with_characters(|ch| {
            (ch[cn].flags & (CharacterFlags::CF_PLAYER.bits() | CharacterFlags::CF_USURP.bits()))
                != 0
        });

        let (cx, cy, name) = Repository::with_characters(|ch| {
            (
                ch[cn].x as usize,
                ch[cn].y as usize,
                ch[cn].get_name().to_string(),
            )
        });

        if is_player_or_usurp {
            self.do_area_say1(cn, cx, cy, ptr);
        } else {
            let msg = format!("{:.30}: \"{}\"\n", name, ptr);
            self.do_area_log(0, 0, cx as i32, cy as i32, FontColor::Red, &msg);
        }

        if m_val == 4 {
            God::slap(0, cn);
            log::info!(
                "TODO: chlog({}, 'Punished for trying to fake another character')",
                cn
            );
        }

        if is_player_or_usurp {
            log::info!("TODO: chlog({}, 'Says \"{}\"', ptr != text)", cn, text);
        }

        // Lab 9 support
        crate::lab9::Labyrinth9::with_mut(|lab9| {
            let _ = lab9.lab9_guesser_says(cn, text);
        });
    }

    pub fn do_command(&self, cn: usize, ptr: &str) {
        // Tokenize up to 10 args. Mimics the original C++ behaviour: quoted tokens
        // or alnum tokens, and `args[n]` points to the remainder starting at next token.
        let mut arg: [String; 10] = Default::default();
        let mut args: [Option<&str>; 10] = [None; 10];

        let mut pos = 0usize;
        let bytes = ptr.as_bytes();
        let len = bytes.len();

        for n in 0..10 {
            // skip initial whitespace
            while pos < len && bytes[pos].is_ascii_whitespace() {
                pos += 1;
            }
            if pos >= len {
                break;
            }

            let mut token = String::new();
            if bytes[pos] == b'"' {
                // quoted
                pos += 1;
                while pos < len && bytes[pos] != b'"' && token.len() < 39 {
                    token.push(bytes[pos] as char);
                    pos += 1;
                }
                if pos < len && bytes[pos] == b'"' {
                    pos += 1;
                }
            } else {
                while pos < len && (bytes[pos] as char).is_ascii_alphanumeric() && token.len() < 39
                {
                    token.push(bytes[pos] as char);
                    pos += 1;
                }
            }

            // skip whitespace after token
            while pos < len && bytes[pos].is_ascii_whitespace() {
                pos += 1;
            }

            arg[n] = token;

            if pos < len {
                // Point to remainder starting at this position
                args[n] = Some(&ptr[pos..]);
            } else {
                args[n] = None;
            }

            if pos >= len {
                break;
            }
        }

        let cmd = arg[0].to_lowercase();

        // Read flags for this character
        let (f_gg, f_c, f_g, f_i, f_s, f_p, f_u, f_sh, f_poh, f_pol) =
            Repository::with_characters(|characters| {
                let flags = characters[cn].flags;
                (
                    (flags & core::constants::CharacterFlags::CF_GREATERGOD.bits()) != 0,
                    (flags & core::constants::CharacterFlags::CF_CREATOR.bits()) != 0,
                    (flags & core::constants::CharacterFlags::CF_GOD.bits()) != 0,
                    (flags & core::constants::CharacterFlags::CF_IMP.bits()) != 0,
                    (flags & core::constants::CharacterFlags::CF_STAFF.bits()) != 0,
                    (flags & core::constants::CharacterFlags::CF_PLAYER.bits()) != 0,
                    (flags & core::constants::CharacterFlags::CF_USURP.bits()) != 0,
                    (flags & core::constants::CharacterFlags::CF_SHUTUP.bits()) != 0,
                    (flags & core::constants::CharacterFlags::CF_POH.bits()) != 0,
                    (flags
                        & (core::constants::CharacterFlags::CF_POH_LEADER.bits()
                            | core::constants::CharacterFlags::CF_GOD.bits()))
                        != 0,
                )
            });

        let f_m = !f_p;
        let f_gi = f_g || f_i;
        let f_giu = f_gi || f_u;
        let f_gius = f_giu || f_s;

        // helper closures
        let starts = |s: &str| cmd.starts_with(s);
        let arg_get = |i: usize| arg.get(i).map(|s| s.as_str()).unwrap_or("");
        let args_get = |i: usize| args.get(i).and_then(|o| *o).unwrap_or("");
        let parse_usize = |s: &str| s.parse::<usize>().unwrap_or(0usize);
        let parse_i32 = |s: &str| s.parse::<i32>().unwrap_or(0i32);

        let first = cmd.chars().next().unwrap_or('\0');

        match first {
            'a' => {
                if starts("afk") && f_p {
                    self.do_afk(cn, args_get(0));
                    return;
                }
                if starts("allow") && f_p {
                    self.do_allow(cn, parse_usize(arg_get(1)));
                    return;
                }
                if starts("announce") && f_gius {
                    self.do_announce(cn, cn, args_get(0));
                    return;
                }
                if starts("addban") && f_gi {
                    God::add_ban(cn, parse_usize(arg_get(1)));
                    return;
                }
            }
            'b' => {
                if starts("bow") && !f_sh {
                    Repository::with_characters_mut(|characters| {
                        characters[cn].misc_action = core::constants::DR_BOW as u16;
                    });
                    return;
                }
                if starts("balance") && !f_m {
                    self.do_balance(cn);
                    return;
                }
                if starts("black") && f_g {
                    God::set_flag(
                        cn,
                        parse_usize(arg_get(1)),
                        core::constants::CharacterFlags::CF_BLACK.bits() as u64,
                    );
                    return;
                }
                if starts("build") && f_c {
                    God::build(cn, parse_i32(arg_get(1)) as u32);
                    return;
                }
            }
            'c' => {
                if starts("cap") && f_g {
                    // set_cap not implemented; log for now
                    self.do_character_log(cn, FontColor::Red, "cap command not implemented\n");
                    return;
                }
                if starts("caution") && f_gius {
                    self.do_caution(cn as i32, cn as i32, args_get(0));
                    return;
                }
                if starts("ccp") && f_i {
                    God::set_flag(
                        cn,
                        parse_usize(arg_get(1)),
                        core::constants::CharacterFlags::CF_CCP.bits() as u64,
                    );
                    return;
                }
                if starts("create") && f_g {
                    God::create(cn, parse_i32(arg_get(1)) as i32);
                    return;
                }
                if starts("creator") && f_gg {
                    God::set_flag(
                        cn,
                        parse_usize(arg_get(1)),
                        core::constants::CharacterFlags::CF_CREATOR.bits() as u64,
                    );
                    return;
                }
            }
            'd' => {
                if starts("deposit") && !f_m {
                    self.do_deposit(cn, parse_i32(arg_get(1)), parse_i32(arg_get(2)));
                    return;
                }
                if starts("depot") && !f_m {
                    self.do_depot(cn);
                    return;
                }
                if starts("delban") && f_giu {
                    God::del_ban(cn, parse_usize(arg_get(1)));
                    return;
                }
            }
            'e' => {
                if starts("effect") && f_g {
                    // effectlist not implemented; placeholder
                    self.do_character_log(cn, FontColor::Red, "effectlist not implemented\n");
                    return;
                }
                if starts("emote") {
                    self.do_emote(cn, args_get(0));
                    return;
                }
                if starts("enemy") && f_giu {
                    self.do_mark(cn, parse_usize(arg_get(1)), args_get(1));
                    return;
                }
                if starts("enter") && f_gi {
                    self.do_enter(cn);
                    return;
                }
                if starts("exit") && f_u {
                    God::exit_usurp(cn);
                    return;
                }
                if starts("erase") && f_g {
                    God::erase(cn, parse_usize(arg_get(1)), 0);
                    return;
                }
            }
            'f' => {
                if starts("fightback") {
                    self.do_fightback(cn);
                    return;
                }
                if starts("follow") && !f_m {
                    self.do_follow(cn, args_get(0));
                    return;
                }
                if starts("force") && f_giu {
                    God::force(cn, arg_get(1), args_get(1));
                    return;
                }
            }
            'g' => {
                if starts("gtell") && !f_m {
                    self.do_gtell(cn, args_get(0));
                    return;
                }
                if starts("gold") {
                    self.do_gold(cn, parse_i32(arg_get(1)));
                    return;
                }
                if starts("group") && !f_m {
                    self.do_group(cn, args_get(0));
                    return;
                }
                if starts("give") && f_giu {
                    self.do_god_give(cn, parse_usize(arg_get(1)));
                    return;
                }
                if starts("goto") && f_giu {
                    God::goto(cn, cn, arg_get(1), arg_get(2));
                    return;
                }
                if starts("god") && f_g {
                    God::set_flag(
                        cn,
                        parse_usize(arg_get(1)),
                        core::constants::CharacterFlags::CF_GOD.bits() as u64,
                    );
                    return;
                }
            }
            'h' => {
                if starts("help") {
                    self.do_help(cn, arg_get(1));
                    return;
                }
            }
            'i' => {
                if starts("ignore") && !f_m {
                    self.do_ignore(cn, arg_get(1), 0);
                    return;
                }
                if starts("iignore") && !f_m {
                    self.do_ignore(cn, arg_get(1), 1);
                    return;
                }
                if starts("iinfo") && f_g {
                    God::iinfo(cn, parse_usize(arg_get(1)));
                    return;
                }
                if starts("immortal") && f_u {
                    God::set_flag(
                        cn,
                        cn,
                        core::constants::CharacterFlags::CF_IMMORTAL.bits() as u64,
                    );
                    return;
                }
                if starts("immortal") && f_g {
                    God::set_flag(
                        cn,
                        parse_usize(arg_get(1)),
                        core::constants::CharacterFlags::CF_IMMORTAL.bits() as u64,
                    );
                    return;
                }
                if starts("imp") && f_g {
                    God::set_flag(
                        cn,
                        parse_usize(arg_get(1)),
                        core::constants::CharacterFlags::CF_IMP.bits() as u64,
                    );
                    return;
                }
                if starts("info") && f_giu {
                    God::info(cn, parse_usize(arg_get(1)));
                    return;
                }
                if starts("init") && f_g {
                    log::warn!("TODO: init command not implemented -- this used to init badwords but we do it differently now.");
                    self.do_character_log(cn, FontColor::Green, "Done.\n");
                    return;
                }
                if starts("infrared") && f_giu {
                    God::set_flag(
                        cn,
                        parse_usize(arg_get(1)),
                        core::constants::CharacterFlags::CF_INFRARED.bits() as u64,
                    );
                    return;
                }
                if starts("invisible") && f_giu {
                    God::set_flag(
                        cn,
                        parse_usize(arg_get(1)),
                        core::constants::CharacterFlags::CF_INVISIBLE.bits() as u64,
                    );
                    return;
                }
                if starts("itell") && f_giu {
                    self.do_itell(cn, args_get(0));
                    return;
                }
            }
            'k' => {
                if starts("kick") && f_giu {
                    God::kick(cn, parse_usize(arg_get(1)));
                    return;
                }
            }
            'l' => {
                if starts("lag") && !f_m {
                    self.do_lag(cn, parse_i32(arg_get(1)));
                    return;
                }
                if starts("leave") && f_gi {
                    self.do_leave(cn);
                    return;
                }
                if starts("look") && f_gius {
                    // do_look_char expects numbers in original; use parse
                    self.do_look_char(cn, parse_usize(arg_get(1)), 1, 0, 0);
                    return;
                }
            }
            'm' => {
                if starts("me") {
                    self.do_emote(cn, args_get(0));
                    return;
                }
                if starts("mirror") && f_giu {
                    God::mirror(cn, arg_get(1), arg_get(2));
                    return;
                }
            }
            'n' => {
                if starts("noshout") && !f_m {
                    self.do_noshout(cn);
                    return;
                }
                if starts("nostaff") && f_giu {
                    self.do_nostaff(cn);
                    return;
                }
                if starts("notell") && !f_m {
                    self.do_notell(cn);
                    return;
                }
                if starts("name") && f_giu {
                    God::set_name(cn, parse_usize(arg_get(1)), args_get(1));
                    return;
                }
                if starts("npclist") && f_giu {
                    self.do_npclist(cn, args_get(0));
                    return;
                }
            }
            'p' => {
                if starts("password") {
                    // change own password
                    God::change_pass(cn, cn, arg_get(1));
                    return;
                }
                if starts("pent") {
                    self.do_check_pent_count(cn);
                    return;
                }
                if starts("purple") && !f_g && !f_m {
                    self.do_become_purple(cn);
                    return;
                }
                if starts("purple") && f_g {
                    God::set_purple(cn, parse_usize(arg_get(1)));
                    return;
                }
            }
            'r' => {
                if starts("rank") {
                    self.do_view_exp_to_rank(cn);
                    return;
                }
                if starts("raise") && f_giu {
                    God::raise_char(cn, parse_usize(arg_get(1)), parse_i32(arg_get(2)));
                    return;
                }
                if starts("respawn") && f_giu {
                    self.do_respawn(cn, parse_usize(arg_get(1)));
                    return;
                }
            }
            's' => {
                if starts("shout") {
                    self.do_shout(cn, args_get(0));
                    return;
                }
                if starts("seen") {
                    self.do_seen(cn, arg_get(1));
                    return;
                }
                if starts("shutup") && f_gius {
                    God::shutup(cn, parse_usize(arg_get(1)));
                    return;
                }
                if starts("skua") {
                    self.do_become_skua(cn);
                    return;
                }
                if starts("slap") && f_giu {
                    God::slap(cn, parse_usize(arg_get(1)));
                    return;
                }
                if starts("summon") && f_g {
                    God::summon(cn, arg_get(1), arg_get(2), arg_get(3));
                    return;
                }
            }
            't' => {
                if starts("tell") {
                    self.do_tell(cn, arg_get(1), args_get(1));
                    return;
                }
                if starts("tavern") && f_g && !f_m {
                    God::tavern(cn);
                    return;
                }
                if starts("top") && f_g {
                    God::top(cn);
                    return;
                }
            }
            'u' => {
                if starts("unique") && f_g {
                    God::unique(cn);
                    return;
                }
                if starts("usurp") && f_giu {
                    God::usurp(cn, parse_usize(arg_get(1)));
                    return;
                }
            }
            'w' => {
                if starts("who") {
                    if f_gius {
                        God::who(cn);
                    } else {
                        God::user_who(cn);
                    }
                    return;
                }
                if starts("wave") && !f_sh {
                    Repository::with_characters_mut(|characters| {
                        characters[cn].misc_action = core::constants::DR_WAVE as u16;
                    });
                    return;
                }
                if starts("withdraw") && !f_m {
                    self.do_withdraw(cn, parse_i32(arg_get(1)), parse_i32(arg_get(2)));
                    return;
                }
                if starts("write") && f_giu {
                    self.do_create_note(cn, args_get(0));
                    return;
                }
            }
            _ => {}
        }

        // Unknown command
        self.do_character_log(cn, FontColor::Red, &format!("Unknown command #{}\n", cmd));
    }

    pub fn do_become_skua(&self, cn: usize) {
        // Ported from svr_do.cpp
        let is_purple = Repository::with_characters(|characters| {
            (characters[cn].kindred & core::constants::KIN_PURPLE as i32) != 0
        });

        if !is_purple {
            self.do_character_log(cn, FontColor::Red, "Hmm. Nothing happened.\n");
            return;
        }

        let ticker = Repository::with_globals(|globals| globals.ticker);
        let attack_time = Repository::with_characters(|characters| {
            characters[cn].data[core::constants::CHD_ATTACKTIME]
        });

        let days = (ticker - attack_time) / (60 * core::constants::TICKS) / 60 / 24;
        if days < 30 {
            self.do_character_log(
                cn,
                FontColor::Red,
                &format!("You have {} days of penance left.\n", 30 - days),
            );
            return;
        }

        self.do_character_log(cn, FontColor::Red, " \n");
        self.do_character_log(
            cn,
            FontColor::Red,
            "You feel the presence of a god again. You feel protected.  Your desire to kill subsides.\n",
        );
        self.do_character_log(cn, FontColor::Red, " \n");
        self.do_character_log(
            cn,
            FontColor::Red,
            "\"THE GOD SKUA WELCOMES YOU, MORTAL! YOUR BONDS OF SLAVERY ARE BROKEN!\"\n",
        );
        self.do_character_log(cn, FontColor::Red, " \n");
        self.do_character_log(cn, FontColor::Green, "Player killing flag cleared.\n");
        self.do_character_log(cn, FontColor::Red, " \n");

        let (x, y) = Repository::with_characters(|characters| (characters[cn].x, characters[cn].y));

        Repository::with_characters_mut(|characters| {
            characters[cn].kindred &= !(core::constants::KIN_PURPLE as i32);
            characters[cn].data[core::constants::CHD_ATTACKTIME] = 0;
            characters[cn].data[core::constants::CHD_ATTACKVICT] = 0;
            characters[cn].temple_x = 512;
            characters[cn].temple_y = 512;
        });

        log::info!(
            "TODO: chlog({}, 'Converted to skua. ({} days elapsed)')",
            cn,
            days
        );

        EffectManager::fx_add_effect(5, 0, x as i32, y as i32, 0);
    }

    pub fn do_make_soulstone(&self, cn: usize, cexp: i32) {}

    pub fn do_list_all_flags(&self, cn: usize, flag: u64) {}

    pub fn do_list_net(&self, cn: usize, co: usize) {}

    pub fn do_respawn(&self, cn: usize, co: usize) {}

    pub fn do_npclist(&self, cn: usize, name: &str) {}

    pub fn do_leave(&self, cn: usize) {}

    pub fn do_enter(&self, cn: usize) {}

    pub fn do_stat(&self, cn: usize) {}

    pub fn do_become_purple(&self, cn: usize) {
        // Ported from svr_do.cpp
        let ticker = Repository::with_globals(|globals| globals.ticker);
        let last = Repository::with_characters(|characters| {
            characters[cn].data[core::constants::CHD_RIDDLER]
        });
        let is_purple = Repository::with_characters(|characters| {
            (characters[cn].kindred & core::constants::KIN_PURPLE as i32) != 0
        });

        if ticker - last < core::constants::TICKS * 60 && !is_purple {
            self.do_character_log(cn, FontColor::Red, " \n");
            self.do_character_log(
                cn,
                FontColor::Red,
                "You feel a god leave you. You feel alone. Scared. Unprotected.\n",
            );
            self.do_character_log(cn, FontColor::Red, " \n");
            self.do_character_log(
                cn,
                FontColor::Red,
                "Another presence enters your mind. You feel hate. Lust. Rage. A Purple Cloud engulfs you.\n",
            );
            self.do_character_log(cn, FontColor::Red, " \n");
            self.do_character_log(
                cn,
                FontColor::Red,
                "\"THE GOD OF THE PURPLE WELCOMES YOU, MORTAL! MAY YOU BE A GOOD SLAVE!\"\n",
            );
            self.do_character_log(cn, FontColor::Red, " \n");
            self.do_character_log(
                cn,
                FontColor::Green,
                "Player killing flag set. May you enjoy the killing.\n",
            );
            self.do_character_log(cn, FontColor::Red, " \n");

            let (x, y) =
                Repository::with_characters(|characters| (characters[cn].x, characters[cn].y));

            Repository::with_characters_mut(|characters| {
                characters[cn].kindred |= core::constants::KIN_PURPLE as i32;
                characters[cn].temple_x = 558;
                characters[cn].temple_y = 542;
            });

            self.do_update_char(cn);

            log::info!("TODO: chlog({}, 'Converted to purple.')", cn);

            EffectManager::fx_add_effect(5, 0, x as i32, y as i32, 0);
        } else {
            self.do_character_log(cn, FontColor::Red, "Hmm. Nothing happened.\n");
        }
    }

    pub fn do_create_note(&self, cn: usize, text: &str) {}

    pub fn do_emote(&self, cn: usize, text: &str) {}

    pub fn do_check_pent_count(&self, cn: usize) {}

    pub fn do_view_exp_to_rank(&self, cn: usize) {}

    pub fn rank2points(&self, rank: i32) -> i32 {}

    pub fn do_gold(&self, cn: usize, val: i32) {}

    pub fn do_god_give(&self, cn: usize, co: usize) {}

    pub fn do_lag(&self, cn: usize, lag: i32) {}

    pub fn do_depot(&self, cn: usize) {}

    pub fn do_balance(&self, cn: usize) {}

    pub fn do_withdraw(&self, cn: usize, g: i32, s: i32) {}

    pub fn do_deposit(&self, cn: usize, g: i32, s: i32) {}

    pub fn do_fightback(&self, cn: usize) {}

    pub fn do_follow(&self, cn: usize, name: &str) {}

    pub fn do_ignore(&self, cn: usize, name: &str, flag: i32) {}

    pub fn do_group(&self, cn: usize, name: &str) {}

    pub fn do_allow(&self, cn: usize, co: usize) {}

    pub fn do_mark(&self, cn: usize, co: usize, msg: &str) {}

    pub fn do_afk(&self, cn: usize, msg: &str) {}

    pub fn do_help(&self, cn: usize, topic: &str) {}

    pub fn do_gtell(&self, cn: usize, text: &str) {}

    pub fn do_nostaff(&self, cn: usize) {}

    pub fn do_stell(&self, cn: usize, text: &str) {}

    pub fn do_itell(&self, cn: usize, text: &str) {}

    pub fn do_shout(&self, cn: usize, text: &str) {}

    pub fn do_noshout(&self, cn: usize) {}

    pub fn do_notell(&self, cn: usize) {}

    pub fn do_tell(&self, cn: usize, con: &str, text: &str) {}

    pub fn do_is_ignore(&self, cn: usize, co: usize, flag: i32) -> i32 {}

    pub fn do_lookup_char_self(&self, name: &str, cn: usize) -> i32 {}

    pub fn do_lookup_char(&self, name: &str) -> i32 {}

    pub fn do_imp_log(&self, font: core::types::FontColor, text: &str) {}

    pub fn do_caution(&self, source: usize, author: usize, text: &str) {}

    pub fn do_announce(&self, source: usize, author: usize, text: &str) {}

    pub fn do_admin_log(&self, source: i32, text: &str) {}

    pub fn do_staff_log(&self, font: core::types::FontColor, text: &str) {}

    pub fn do_area_say1(&self, cn: usize, xs: usize, ys: usize, text: &str) {}
}
