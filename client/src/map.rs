#![allow(dead_code)]

use crate::types::map::CMapTile;

// These are the *client view* tile dimensions (matches original client: 34x34).
// Do NOT use SERVER_MAPX/Y here; those represent the full world map size.
pub const TILEX: usize = mag_core::constants::TILEX;
pub const TILEY: usize = mag_core::constants::TILEY;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TileDrawData {
    pub base_sprite: Option<i16>,
    pub item_sprite: Option<i16>,
    pub character_sprite: Option<u16>,
    pub light: u8,
    pub flags: u32,
    pub flags2: u32,
    pub character_status: u8,
    pub item_status: u8,
}

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
    pub fn is_empty(&self) -> bool {
        self.tiles.is_empty()
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

    pub fn tile_at_xy_mut(&mut self, x: usize, y: usize) -> Option<&mut CMapTile> {
        Self::tile_index(x, y).and_then(|idx| self.tiles.get_mut(idx))
    }

    pub fn iter_region(
        &self,
        min_x: usize,
        min_y: usize,
        width: usize,
        height: usize,
    ) -> impl Iterator<Item = (usize, usize, &CMapTile)> {
        let max_x = (min_x + width).min(TILEX);
        let max_y = (min_y + height).min(TILEY);

        (min_y..max_y).flat_map(move |y| {
            (min_x..max_x).filter_map(move |x| {
                let idx = x + y * TILEX;
                self.tiles.get(idx).map(|t| (x, y, t))
            })
        })
    }

    pub fn draw_data_at_index(&self, index: usize) -> Option<TileDrawData> {
        let tile = self.tiles.get(index)?;

        Some(TileDrawData {
            base_sprite: (tile.ba_sprite != 0).then_some(tile.ba_sprite),
            item_sprite: (tile.it_sprite != 0).then_some(tile.it_sprite),
            character_sprite: (tile.ch_sprite != 0).then_some(tile.ch_sprite),
            light: tile.light,
            flags: tile.flags,
            flags2: tile.flags2,
            character_status: tile.ch_status,
            item_status: tile.it_status,
        })
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
            tile.it_sprite = v as i16;
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

        for b in packed {
            let lo = b & 0x0F;
            let hi = (b >> 4) & 0x0F;

            if let Some(t) = self.tiles.get_mut(idx) {
                t.light = (base_light << 4) | lo;
            }
            idx += 1;
            if idx >= self.tiles.len() {
                break;
            }

            if let Some(t) = self.tiles.get_mut(idx) {
                t.light = (base_light << 4) | hi;
            }
            idx += 1;
            if idx >= self.tiles.len() {
                break;
            }
        }
    }

    pub fn set_origin(&mut self, xp: i16, yp: i16) {
        // Mirrors original sv_setorigin(): rewrites map[n].x/map[n].y based on xp/yp.
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
        // Mirrors original: memmove(map, map+1, sizeof(cmap) * (TILEX*TILEY-1))
        let len = self.tiles.len();
        if len < 2 {
            return;
        }
        self.tiles.copy_within(1..len, 0);
    }

    pub fn scroll_left(&mut self) {
        // Mirrors original: memmove(map+1, map, sizeof(cmap) * (TILEX*TILEY-1))
        let len = self.tiles.len();
        if len < 2 {
            return;
        }
        self.tiles.copy_within(0..len - 1, 1);
    }

    pub fn scroll_down(&mut self) {
        // Mirrors original: memmove(map, map+TILEX, sizeof(cmap) * (TILEX*TILEY-TILEX))
        let len = self.tiles.len();
        if TILEX == 0 || len <= TILEX {
            return;
        }
        self.tiles.copy_within(TILEX..len, 0);
    }

    pub fn scroll_up(&mut self) {
        // Mirrors original: memmove(map+TILEX, map, sizeof(cmap) * (TILEX*TILEY-TILEX))
        let len = self.tiles.len();
        if TILEX == 0 || len <= TILEX {
            return;
        }
        self.tiles.copy_within(0..len - TILEX, TILEX);
    }

    pub fn scroll_left_up(&mut self) {
        // Mirrors original: memmove(map+TILEX+1, map, sizeof(cmap) * (TILEX*TILEY-TILEX-1))
        let len = self.tiles.len();
        let shift = TILEX + 1;
        if shift == 0 || len <= shift {
            return;
        }
        self.tiles.copy_within(0..len - shift, shift);
    }

    pub fn scroll_left_down(&mut self) {
        // Mirrors original: memmove(map, map+TILEX-1, sizeof(cmap) * (TILEX*TILEY-TILEX+1))
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
        // Mirrors original: memmove(map+TILEX-1, map, sizeof(cmap) * (TILEX*TILEY-TILEX+1))
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
        // Mirrors original: memmove(map, map+TILEX+1, sizeof(cmap) * (TILEX*TILEY-TILEX-1))
        let len = self.tiles.len();
        let shift = TILEX + 1;
        if shift == 0 || len <= shift {
            return;
        }
        self.tiles.copy_within(shift..len, 0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tile_index_bounds() {
        assert_eq!(GameMap::tile_index(0, 0), Some(0));
        assert_eq!(
            GameMap::tile_index(TILEX.saturating_sub(1), 0),
            Some(TILEX - 1)
        );
        assert_eq!(
            GameMap::tile_index(0, TILEY.saturating_sub(1)),
            Some((TILEY - 1) * TILEX)
        );
        assert_eq!(GameMap::tile_index(TILEX, 0), None);
        assert_eq!(GameMap::tile_index(0, TILEY), None);
    }

    #[test]
    fn new_initializes_tile_xy() {
        let map = GameMap::new();
        assert_eq!(map.len(), TILEX * TILEY);

        let t00 = map.tile_at_xy(0, 0).unwrap();
        assert_eq!(t00.x, 0);
        assert_eq!(t00.y, 0);

        let tx = TILEX - 1;
        let ty = TILEY - 1;
        let tlast = map.tile_at_xy(tx, ty).unwrap();
        assert_eq!(tlast.x, tx as u16);
        assert_eq!(tlast.y, ty as u16);
    }

    #[test]
    fn iter_region_clamps_to_map() {
        let map = GameMap::new();

        let items: Vec<_> = map.iter_region(0, 0, 2, 2).collect();
        assert_eq!(items.len(), 4);
        assert_eq!(items[0].0, 0);
        assert_eq!(items[0].1, 0);
        assert_eq!(items[3].0, 1);
        assert_eq!(items[3].1, 1);

        let items: Vec<_> = map.iter_region(TILEX - 1, TILEY - 1, 10, 10).collect();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].0, TILEX - 1);
        assert_eq!(items[0].1, TILEY - 1);
    }

    #[test]
    fn draw_data_maps_zero_to_none() {
        let mut map = GameMap::new();
        let idx = GameMap::tile_index(0, 0).unwrap();
        let t = map.tile_at_index_mut(idx).unwrap();
        t.ba_sprite = 0;
        t.it_sprite = 0;
        t.ch_sprite = 0;
        t.light = 7;
        t.flags = 1;
        t.flags2 = 2;
        t.ch_status = 3;
        t.it_status = 4;

        let d = map.draw_data_at_index(idx).unwrap();
        assert_eq!(d.base_sprite, None);
        assert_eq!(d.item_sprite, None);
        assert_eq!(d.character_sprite, None);
        assert_eq!(d.light, 7);
        assert_eq!(d.flags, 1);
        assert_eq!(d.flags2, 2);
        assert_eq!(d.character_status, 3);
        assert_eq!(d.item_status, 4);

        let t = map.tile_at_index_mut(idx).unwrap();
        t.ba_sprite = 10;
        t.it_sprite = 11;
        t.ch_sprite = 12;

        let d = map.draw_data_at_index(idx).unwrap();
        assert_eq!(d.base_sprite, Some(10));
        assert_eq!(d.item_sprite, Some(11));
        assert_eq!(d.character_sprite, Some(12));
    }

    #[test]
    fn apply_set_map_tracks_relative_index() {
        let mut map = GameMap::new();

        map.reset_last_setmap_index();
        map.apply_set_map(
            1,
            None,
            Some(111),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        );
        assert_eq!(map.tile_at_index(0).unwrap().ba_sprite, 111);

        map.apply_set_map(
            1,
            None,
            Some(222),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        );
        assert_eq!(map.tile_at_index(1).unwrap().ba_sprite, 222);

        map.apply_set_map(
            0,
            Some(5),
            Some(333),
            Some(7),
            Some(9),
            Some(44),
            Some(1),
            Some(55),
            Some(2),
            Some(3),
            Some(4),
            Some(5),
            Some(6),
            Some(7),
        );

        let t = map.tile_at_index(5).unwrap();
        assert_eq!(t.ba_sprite, 333);
        assert_eq!(t.flags, 7);
        assert_eq!(t.flags2, 9);
        assert_eq!(t.it_sprite, 44);
        assert_eq!(t.it_status, 1);
        assert_eq!(t.ch_sprite, 55);
        assert_eq!(t.ch_status, 2);
        assert_eq!(t.ch_stat_off, 3);
        assert_eq!(t.ch_nr, 4);
        assert_eq!(t.ch_id, 5);
        assert_eq!(t.ch_speed, 6);
        assert_eq!(t.ch_proz, 7);
    }

    #[test]
    fn apply_set_map3_unpacks_two_nibbles() {
        let mut map = GameMap::new();
        map.apply_set_map3(0, 0xA, &[0x21]);
        assert_eq!(map.tile_at_index(0).unwrap().light, (0xA << 4) | 0x1);
        assert_eq!(map.tile_at_index(1).unwrap().light, (0xA << 4) | 0x2);
    }

    #[test]
    fn scrolling_moves_tiles_in_expected_direction() {
        let mut map = GameMap::new();
        for i in 0..3 {
            map.tile_at_index_mut(i).unwrap().ba_sprite = (i as i16) + 10;
        }
        map.scroll_right();
        assert_eq!(map.tile_at_index(0).unwrap().ba_sprite, 11);
        assert_eq!(map.tile_at_index(1).unwrap().ba_sprite, 12);

        map.scroll_left();
        // After right then left, index 1 should contain what index 0 had after right.
        assert_eq!(map.tile_at_index(1).unwrap().ba_sprite, 11);

        if TILEX >= 1 {
            map.tile_at_index_mut(0).unwrap().ba_sprite = 101;
            map.tile_at_index_mut(TILEX).unwrap().ba_sprite = 202;
            map.scroll_down();
            assert_eq!(map.tile_at_index(0).unwrap().ba_sprite, 202);

            map.scroll_up();
            assert_eq!(map.tile_at_index(TILEX).unwrap().ba_sprite, 202);
        }
    }

    #[test]
    fn set_origin_offsets_xy() {
        let mut map = GameMap::new();
        map.set_origin(100, 200);
        let t00 = map.tile_at_xy(0, 0).unwrap();
        assert_eq!(t00.x, 100);
        assert_eq!(t00.y, 200);

        let tlast = map.tile_at_xy(TILEX - 1, TILEY - 1).unwrap();
        assert_eq!(tlast.x, (100 + (TILEX - 1) as i16) as u16);
        assert_eq!(tlast.y, (200 + (TILEY - 1) as i16) as u16);
    }
}
