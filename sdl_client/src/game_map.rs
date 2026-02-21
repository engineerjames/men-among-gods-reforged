use mag_core::constants::{TILEX, TILEY};

use crate::types::map::CMapTile;

#[derive(Debug)]
pub struct GameMap {
    tiles: Vec<CMapTile>,
    last_setmap_index: Option<u16>,
}

impl Default for GameMap {
    fn default() -> Self {
        Self::new()
    }
}

impl GameMap {
    pub fn new() -> Self {
        let count = TILEX * TILEY;
        let mut tiles = vec![CMapTile::default(); count];

        for y in 0..TILEY {
            for x in 0..TILEX {
                let idx = x + y * TILEX;
                tiles[idx].x = x as u16;
                tiles[idx].y = y as u16;
            }
        }

        Self {
            tiles,
            last_setmap_index: None,
        }
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.tiles.len()
    }

    #[inline]
    pub fn reset_last_setmap_index(&mut self) {
        self.last_setmap_index = None;
    }

    #[inline]
    pub fn tile_index(x: usize, y: usize) -> Option<usize> {
        if x < TILEX && y < TILEY {
            Some(x + y * TILEX)
        } else {
            None
        }
    }

    #[inline]
    pub fn tile_at_index(&self, index: usize) -> Option<&CMapTile> {
        self.tiles.get(index)
    }

    #[inline]
    pub fn tile_at_index_mut(&mut self, index: usize) -> Option<&mut CMapTile> {
        self.tiles.get_mut(index)
    }

    pub fn tile_at_xy(&self, x: usize, y: usize) -> Option<&CMapTile> {
        Self::tile_index(x, y).and_then(|idx| self.tiles.get(idx))
    }

    pub fn apply_set_map(
        &mut self,
        off: u8,
        absolute_tile_index: Option<u16>,
        ba_sprite: Option<u16>,
        flags1: Option<u32>,
        flags2: Option<u32>,
        it_sprite: Option<u16>,
        it_status: Option<u8>,
        ch_sprite: Option<u16>,
        ch_status: Option<u8>,
        ch_stat_off: Option<u8>,
        ch_nr: Option<u16>,
        ch_id: Option<u16>,
        ch_speed: Option<u8>,
        ch_proz: Option<u8>,
    ) {
        let next_index = if off == 0 {
            absolute_tile_index
        } else {
            let base = self.last_setmap_index.map(|v| v as i32).unwrap_or(-1);
            let next = base + off as i32;
            if next < 0 {
                None
            } else {
                Some(next as u16)
            }
        };

        let Some(tile_index) = next_index else {
            return;
        };

        let idx = tile_index as usize;
        if idx >= self.tiles.len() {
            return;
        }

        self.last_setmap_index = Some(tile_index);
        let Some(tile) = self.tiles.get_mut(idx) else {
            return;
        };

        if let Some(v) = ba_sprite {
            tile.ba_sprite = v as i16;
        }
        if let Some(v) = flags1 {
            tile.flags = v;
        }
        if let Some(v) = flags2 {
            tile.flags2 = v;
        }
        if let Some(v) = it_sprite {
            tile.it_sprite = v;
        }
        if let Some(v) = it_status {
            tile.it_status = v;
        }
        if let Some(v) = ch_sprite {
            tile.ch_sprite = v;
        }
        if let Some(v) = ch_status {
            tile.ch_status = v;
        }
        if let Some(v) = ch_stat_off {
            tile.ch_stat_off = v;
        }
        if let Some(v) = ch_nr {
            tile.ch_nr = v;
        }
        if let Some(v) = ch_id {
            tile.ch_id = v;
        }
        if let Some(v) = ch_speed {
            tile.ch_speed = v;
        }
        if let Some(v) = ch_proz {
            tile.ch_proz = v;
        }
    }

    pub fn apply_set_map3(&mut self, start_index: u16, base_light: u8, packed: &[u8]) {
        let mut idx = start_index as usize;

        if let Some(t) = self.tiles.get_mut(idx) {
            t.light = base_light & 0x0F;
        }
        idx += 1;

        for b in packed {
            let lo = b & 0x0F;
            let hi = (b >> 4) & 0x0F;

            if idx >= self.tiles.len() {
                break;
            }
            if let Some(t) = self.tiles.get_mut(idx) {
                t.light = hi;
            }
            idx += 1;

            if idx >= self.tiles.len() {
                break;
            }
            if let Some(t) = self.tiles.get_mut(idx) {
                t.light = lo;
            }
            idx += 1;
        }
    }

    pub fn set_origin(&mut self, xp: i16, yp: i16) {
        if TILEX == 0 || TILEY == 0 {
            return;
        }

        let mut n = 0usize;
        for y in 0..TILEY {
            for x in 0..TILEX {
                if let Some(tile) = self.tiles.get_mut(n) {
                    tile.x = (x as i32 + xp as i32) as u16;
                    tile.y = (y as i32 + yp as i32) as u16;
                }
                n += 1;
                if n >= self.tiles.len() {
                    return;
                }
            }
        }
    }

    pub fn scroll_right(&mut self) {
        let len = self.tiles.len();
        if len < 2 {
            return;
        }
        self.tiles.copy_within(1..len, 0);
    }

    pub fn scroll_left(&mut self) {
        let len = self.tiles.len();
        if len < 2 {
            return;
        }
        self.tiles.copy_within(0..len - 1, 1);
    }

    pub fn scroll_down(&mut self) {
        let len = self.tiles.len();
        if TILEX == 0 || len <= TILEX {
            return;
        }
        self.tiles.copy_within(TILEX..len, 0);
    }

    pub fn scroll_up(&mut self) {
        let len = self.tiles.len();
        if TILEX == 0 || len <= TILEX {
            return;
        }
        self.tiles.copy_within(0..len - TILEX, TILEX);
    }

    pub fn scroll_left_up(&mut self) {
        let len = self.tiles.len();
        let shift = TILEX + 1;
        if shift == 0 || len <= shift {
            return;
        }
        self.tiles.copy_within(0..len - shift, shift);
    }

    pub fn scroll_left_down(&mut self) {
        let len = self.tiles.len();
        if TILEX == 0 {
            return;
        }
        let shift = TILEX.saturating_sub(1);
        if len <= shift {
            return;
        }
        self.tiles.copy_within(shift..len, 0);
    }

    pub fn scroll_right_up(&mut self) {
        let len = self.tiles.len();
        if TILEX == 0 {
            return;
        }
        let shift = TILEX.saturating_sub(1);
        let count = len.saturating_sub(TILEX).saturating_add(1);
        if shift >= len || count == 0 || count > len {
            return;
        }
        self.tiles.copy_within(0..count, shift);
    }

    pub fn scroll_right_down(&mut self) {
        let len = self.tiles.len();
        let shift = TILEX + 1;
        if shift == 0 || len <= shift {
            return;
        }
        self.tiles.copy_within(shift..len, 0);
    }
}
