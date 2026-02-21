use mag_core::constants::{TILEX, TILEY};

use crate::types::map::CMapTile;

/// Fixed-size tile grid representing the player's visible map window.
///
/// The map is `TILEX × TILEY` tiles. Each tile stores background / item /
/// character sprite IDs, flags, lighting, and world coordinates. The server
/// streams incremental updates via `SV_SETMAP` commands; the client applies
/// them through [`apply_set_map`](Self::apply_set_map) and scrolls the grid
/// when the player moves.
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
    /// Creates a new map with all tiles zero-initialized and local
    /// coordinates set to their grid position.
    ///
    /// # Returns
    /// * A `GameMap` of `TILEX × TILEY` default tiles.
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

    /// Returns the total number of tiles in the grid (`TILEX * TILEY`).
    #[inline]
    pub fn len(&self) -> usize {
        self.tiles.len()
    }

    /// Resets the delta-index tracker used by [`apply_set_map`](Self::apply_set_map)
    /// so the next update must carry an absolute tile index.
    #[inline]
    pub fn reset_last_setmap_index(&mut self) {
        self.last_setmap_index = None;
    }

    /// Converts (x, y) grid coordinates to a flat index.
    ///
    /// # Arguments
    /// * `x` - Column (must be < `TILEX`).
    /// * `y` - Row (must be < `TILEY`).
    ///
    /// # Returns
    /// * `Some(index)` if in bounds, `None` otherwise.
    #[inline]
    pub fn tile_index(x: usize, y: usize) -> Option<usize> {
        if x < TILEX && y < TILEY {
            Some(x + y * TILEX)
        } else {
            None
        }
    }

    /// Returns a shared reference to the tile at the given flat index.
    ///
    /// # Arguments
    /// * `index` - Flat index into the tile array.
    ///
    /// # Returns
    /// * `Some(&CMapTile)` if in bounds, `None` otherwise.
    #[inline]
    pub fn tile_at_index(&self, index: usize) -> Option<&CMapTile> {
        self.tiles.get(index)
    }

    /// Returns a mutable reference to the tile at the given flat index.
    ///
    /// # Arguments
    /// * `index` - Flat index into the tile array.
    ///
    /// # Returns
    /// * `Some(&mut CMapTile)` if in bounds, `None` otherwise.
    #[inline]
    pub fn tile_at_index_mut(&mut self, index: usize) -> Option<&mut CMapTile> {
        self.tiles.get_mut(index)
    }

    /// Returns a shared reference to the tile at grid coordinates `(x, y)`.
    ///
    /// # Arguments
    /// * `x` - Column.
    /// * `y` - Row.
    ///
    /// # Returns
    /// * `Some(&CMapTile)` if in bounds, `None` otherwise.
    pub fn tile_at_xy(&self, x: usize, y: usize) -> Option<&CMapTile> {
        Self::tile_index(x, y).and_then(|idx| self.tiles.get(idx))
    }

    /// Applies an incremental `SV_SETMAP` update to a single tile.
    ///
    /// The target tile is identified either absolutely (when `off == 0`,
    /// using `absolute_tile_index`) or as a delta from the last update.
    /// Only fields wrapped in `Some(…)` are overwritten; `None` fields are
    /// left unchanged.
    ///
    /// # Arguments
    /// * `off` - Delta offset from the previous `SV_SETMAP` target (0 = absolute).
    /// * `absolute_tile_index` - Flat tile index used when `off` is 0.
    /// * `ba_sprite` .. `ch_proz` - Optional field updates for the target tile.
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

    /// Applies a `SV_SETMAP3` lighting update.
    ///
    /// Starting at `start_index`, the base light value is written directly,
    /// then subsequent tiles are updated two at a time from the nibble-packed
    /// `packed` slice (high nibble first).
    ///
    /// # Arguments
    /// * `start_index` - Flat tile index to begin writing light data.
    /// * `base_light` - Light value for the first tile (only low 4 bits used).
    /// * `packed` - Nibble-packed light data for subsequent tiles.
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

    /// Sets the world-coordinate origin of every tile in the grid.
    ///
    /// After this call, tile at grid position `(gx, gy)` will have
    /// `tile.x = gx + xp` and `tile.y = gy + yp`.
    ///
    /// # Arguments
    /// * `xp` - World X offset to add to each tile's grid column.
    /// * `yp` - World Y offset to add to each tile's grid row.
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

    /// Scrolls all tiles one position to the right (drops the leftmost column).
    pub fn scroll_right(&mut self) {
        let len = self.tiles.len();
        if len < 2 {
            return;
        }
        self.tiles.copy_within(1..len, 0);
    }

    /// Scrolls all tiles one position to the left (drops the rightmost column).
    pub fn scroll_left(&mut self) {
        let len = self.tiles.len();
        if len < 2 {
            return;
        }
        self.tiles.copy_within(0..len - 1, 1);
    }

    /// Scrolls all tiles one row downward (drops the top row).
    pub fn scroll_down(&mut self) {
        let len = self.tiles.len();
        if TILEX == 0 || len <= TILEX {
            return;
        }
        self.tiles.copy_within(TILEX..len, 0);
    }

    /// Scrolls all tiles one row upward (drops the bottom row).
    pub fn scroll_up(&mut self) {
        let len = self.tiles.len();
        if TILEX == 0 || len <= TILEX {
            return;
        }
        self.tiles.copy_within(0..len - TILEX, TILEX);
    }

    /// Scrolls tiles diagonally: one position left and one row up.
    pub fn scroll_left_up(&mut self) {
        let len = self.tiles.len();
        let shift = TILEX + 1;
        if shift == 0 || len <= shift {
            return;
        }
        self.tiles.copy_within(0..len - shift, shift);
    }

    /// Scrolls tiles diagonally: one position left and one row down.
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

    /// Scrolls tiles diagonally: one position right and one row up.
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

    /// Scrolls tiles diagonally: one position right and one row down.
    pub fn scroll_right_down(&mut self) {
        let len = self.tiles.len();
        let shift = TILEX + 1;
        if shift == 0 || len <= shift {
            return;
        }
        self.tiles.copy_within(shift..len, 0);
    }
}
