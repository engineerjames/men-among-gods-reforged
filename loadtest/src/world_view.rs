//! Minimal client-side model of the bot's visible map window.
//!
//! The real client keeps a full `TILEX x TILEY` tile grid (`client/src/game_map.rs`)
//! with sprites, lighting and animation state. A load-test bot only needs
//! enough of that to make decisions: which tiles hold a usable item and which
//! hold another character. This module mirrors just the parts of the wire
//! protocol needed to keep those two facts current:
//!
//! * `SV_SETMAP` (absolute or delta-indexed) updates a tile's `flags1` and
//!   `ch_nr`.
//! * The eight `Scroll*` opcodes shift the grid when the player moves, using
//!   the exact same `copy_within` semantics as the client so tile indices stay
//!   aligned with subsequent delta updates.
//! * `SV_SETORIGIN` gives the world coordinate of grid tile `(0, 0)`; the
//!   player always sits at the grid centre.

use mag_core::constants::{ISCHAR, ISUSABLE, TILEX, TILEY};
use mag_core::server_commands::{ServerCommand, ServerCommandData, ServerCommandType};

/// Per-tile state the bot cares about.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ViewTile {
    /// Display flags (`ISUSABLE`, `ISITEM`, `ISCHAR`, ...) from `flags1`.
    pub flags: u32,
    /// Server character number standing on this tile (0 = none).
    pub ch_nr: u16,
}

/// A visible character on the map window, in world coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VisibleChar {
    /// Server character number.
    pub ch_nr: u16,
    /// World X coordinate.
    pub x: i32,
    /// World Y coordinate.
    pub y: i32,
}

/// Bot-side view of the `TILEX x TILEY` map window around the player.
#[derive(Debug, Clone)]
pub struct WorldView {
    tiles: Vec<ViewTile>,
    last_setmap_index: Option<u16>,
    origin: Option<(i16, i16)>,
}

impl Default for WorldView {
    fn default() -> Self {
        Self::new()
    }
}

impl WorldView {
    /// Creates an empty view with no known origin.
    ///
    /// # Returns
    ///
    /// * A `WorldView` of `TILEX * TILEY` zeroed tiles.
    pub fn new() -> Self {
        Self {
            tiles: vec![ViewTile::default(); TILEX * TILEY],
            last_setmap_index: None,
            origin: None,
        }
    }

    /// Returns the player's world position, if an origin has been received.
    ///
    /// The player is always at the centre of the map window, so this is
    /// `origin + (TILEX / 2, TILEY / 2)`.
    ///
    /// # Returns
    ///
    /// * `Some((x, y))` once `SV_SETORIGIN` has been seen, `None` before.
    pub fn self_pos(&self) -> Option<(i16, i16)> {
        self.origin.map(|(ox, oy)| {
            (
                ox.wrapping_add(TILEX as i16 / 2),
                oy.wrapping_add(TILEY as i16 / 2),
            )
        })
    }

    /// Applies one parsed server command to the view.
    ///
    /// Handles the scroll opcodes, `SetMap` and `SetOrigin`; every other
    /// command is ignored.
    ///
    /// # Arguments
    ///
    /// * `cmd` - Parsed server command.
    pub fn apply(&mut self, cmd: &ServerCommand) {
        match cmd.header {
            ServerCommandType::ScrollRight => self.scroll_right(),
            ServerCommandType::ScrollLeft => self.scroll_left(),
            ServerCommandType::ScrollUp => self.scroll_up(),
            ServerCommandType::ScrollDown => self.scroll_down(),
            ServerCommandType::ScrollRightUp => self.scroll_right_up(),
            ServerCommandType::ScrollRightDown => self.scroll_right_down(),
            ServerCommandType::ScrollLeftUp => self.scroll_left_up(),
            ServerCommandType::ScrollLeftDown => self.scroll_left_down(),
            _ => {}
        }

        match &cmd.structured_data {
            ServerCommandData::SetMap {
                off,
                absolute_tile_index,
                flags1,
                ch_nr,
                ..
            } => self.apply_set_map(*off, *absolute_tile_index, *flags1, *ch_nr),
            ServerCommandData::SetOrigin { x, y } => self.set_origin(*x, *y),
            _ => {}
        }
    }

    /// Records the world coordinate of grid tile `(0, 0)` from `SV_SETORIGIN`.
    ///
    /// # Arguments
    ///
    /// * `x` - World X of the top-left visible tile.
    /// * `y` - World Y of the top-left visible tile.
    pub fn set_origin(&mut self, x: i16, y: i16) {
        self.origin = Some((x, y));
    }

    /// Applies an `SV_SETMAP` update to a single tile.
    ///
    /// # Arguments
    ///
    /// * `off` - Delta offset from the previous target tile (0 = absolute).
    /// * `absolute_tile_index` - Flat tile index used when `off == 0`.
    /// * `flags1` - New display flags, if present in the packet.
    /// * `ch_nr` - New character number, if present in the packet.
    pub fn apply_set_map(
        &mut self,
        off: u8,
        absolute_tile_index: Option<u16>,
        flags1: Option<u32>,
        ch_nr: Option<u16>,
    ) {
        let next_index = if off == 0 {
            absolute_tile_index
        } else {
            let base = self.last_setmap_index.map(i32::from).unwrap_or(-1);
            let next = base + i32::from(off);
            if next < 0 { None } else { Some(next as u16) }
        };

        let Some(tile_index) = next_index else {
            return;
        };
        let idx = tile_index as usize;
        if idx >= self.tiles.len() {
            return;
        }
        self.last_setmap_index = Some(tile_index);

        let tile = &mut self.tiles[idx];
        if let Some(v) = flags1 {
            tile.flags = v;
        }
        if let Some(v) = ch_nr {
            tile.ch_nr = v;
        }
    }

    /// Lists world coordinates of tiles flagged `ISUSABLE` within `radius`
    /// (Chebyshev distance) of the player.
    ///
    /// # Arguments
    ///
    /// * `radius` - Maximum tile distance from the player.
    ///
    /// # Returns
    ///
    /// * World `(x, y)` pairs of usable items, empty if the origin is unknown.
    pub fn usable_tiles_within(&self, radius: i32) -> Vec<(i32, i32)> {
        self.tiles_within(radius, |t| t.flags & ISUSABLE != 0)
            .into_iter()
            .map(|(x, y, _)| (x, y))
            .collect()
    }

    /// Lists characters visible within `radius` of the player, excluding the
    /// player's own tile.
    ///
    /// # Arguments
    ///
    /// * `radius` - Maximum tile distance from the player.
    ///
    /// # Returns
    ///
    /// * Visible characters, empty if the origin is unknown.
    pub fn chars_within(&self, radius: i32) -> Vec<VisibleChar> {
        let centre = (TILEX / 2, TILEY / 2);
        self.tiles_within(radius, |t| t.ch_nr != 0 && t.flags & ISCHAR != 0)
            .into_iter()
            .filter(|&(_, _, idx)| idx != centre.0 + centre.1 * TILEX)
            .map(|(x, y, idx)| VisibleChar {
                ch_nr: self.tiles[idx].ch_nr,
                x,
                y,
            })
            .collect()
    }

    /// Scans the square window of `radius` around the centre for tiles
    /// matching `pred`, returning world coords and the flat tile index.
    fn tiles_within(
        &self,
        radius: i32,
        pred: impl Fn(&ViewTile) -> bool,
    ) -> Vec<(i32, i32, usize)> {
        let Some((ox, oy)) = self.origin else {
            return Vec::new();
        };
        let (cx, cy) = (TILEX as i32 / 2, TILEY as i32 / 2);
        let radius = radius.max(0);
        let mut out = Vec::new();
        for gy in (cy - radius).max(0)..=(cy + radius).min(TILEY as i32 - 1) {
            for gx in (cx - radius).max(0)..=(cx + radius).min(TILEX as i32 - 1) {
                let idx = gx as usize + gy as usize * TILEX;
                if pred(&self.tiles[idx]) {
                    out.push((i32::from(ox) + gx, i32::from(oy) + gy, idx));
                }
            }
        }
        out
    }

    fn scroll_right(&mut self) {
        let len = self.tiles.len();
        if len >= 2 {
            self.tiles.copy_within(1..len, 0);
        }
    }

    fn scroll_left(&mut self) {
        let len = self.tiles.len();
        if len >= 2 {
            self.tiles.copy_within(0..len - 1, 1);
        }
    }

    fn scroll_down(&mut self) {
        let len = self.tiles.len();
        if len > TILEX {
            self.tiles.copy_within(TILEX..len, 0);
        }
    }

    fn scroll_up(&mut self) {
        let len = self.tiles.len();
        if len > TILEX {
            self.tiles.copy_within(0..len - TILEX, TILEX);
        }
    }

    fn scroll_left_up(&mut self) {
        let len = self.tiles.len();
        let shift = TILEX + 1;
        if len > shift {
            self.tiles.copy_within(0..len - shift, shift);
        }
    }

    fn scroll_left_down(&mut self) {
        let len = self.tiles.len();
        let shift = TILEX - 1;
        if len > shift {
            self.tiles.copy_within(shift..len, 0);
        }
    }

    fn scroll_right_up(&mut self) {
        let len = self.tiles.len();
        let shift = TILEX - 1;
        let count = len - TILEX + 1;
        if shift < len && count <= len {
            self.tiles.copy_within(0..count, shift);
        }
    }

    fn scroll_right_down(&mut self) {
        let len = self.tiles.len();
        let shift = TILEX + 1;
        if len > shift {
            self.tiles.copy_within(shift..len, 0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mag_core::constants::ISITEM;

    fn centre_index() -> u16 {
        (TILEX / 2 + (TILEY / 2) * TILEX) as u16
    }

    fn cmd(header: ServerCommandType, structured_data: ServerCommandData) -> ServerCommand {
        ServerCommand {
            header,
            structured_data,
            _payload: Vec::new(),
        }
    }

    #[test]
    fn self_pos_unknown_until_origin() {
        let mut view = WorldView::new();
        assert!(view.self_pos().is_none());
        view.apply(&cmd(
            ServerCommandType::SetOrigin,
            ServerCommandData::SetOrigin { x: 100, y: 200 },
        ));
        assert_eq!(
            view.self_pos(),
            Some((100 + TILEX as i16 / 2, 200 + TILEY as i16 / 2))
        );
    }

    #[test]
    fn set_map_absolute_then_delta() {
        let mut view = WorldView::new();
        view.apply_set_map(0, Some(10), Some(ISITEM | ISUSABLE), None);
        view.apply_set_map(5, None, Some(ISCHAR), Some(42));
        assert_eq!(view.tiles[10].flags, ISITEM | ISUSABLE);
        assert_eq!(view.tiles[15].flags, ISCHAR);
        assert_eq!(view.tiles[15].ch_nr, 42);
    }

    #[test]
    fn set_map_out_of_range_ignored() {
        let mut view = WorldView::new();
        view.apply_set_map(0, Some(u16::MAX), Some(ISUSABLE), None);
        assert!(view.tiles.iter().all(|t| t.flags == 0));
    }

    #[test]
    fn usable_tiles_reported_in_world_coords_within_radius() {
        let mut view = WorldView::new();
        view.origin = Some((1000, 2000));
        let c = centre_index() as usize;
        // Two tiles to the right of the player and three rows down.
        let near = c + 2 + 3 * TILEX;
        // Far outside a radius of 5.
        let far = c + 20;
        view.tiles[near].flags = ISITEM | ISUSABLE;
        view.tiles[far].flags = ISITEM | ISUSABLE;

        let found = view.usable_tiles_within(5);
        assert_eq!(
            found,
            vec![(1000 + TILEX as i32 / 2 + 2, 2000 + TILEY as i32 / 2 + 3)]
        );
        assert_eq!(view.usable_tiles_within(25).len(), 2);
    }

    #[test]
    fn usable_tiles_empty_without_origin() {
        let mut view = WorldView::new();
        view.tiles[centre_index() as usize + 1].flags = ISUSABLE;
        assert!(view.usable_tiles_within(10).is_empty());
    }

    #[test]
    fn chars_within_excludes_self_tile() {
        let mut view = WorldView::new();
        view.origin = Some((0, 0));
        let c = centre_index() as usize;
        view.tiles[c] = ViewTile {
            flags: ISCHAR,
            ch_nr: 7,
        };
        view.tiles[c + 1] = ViewTile {
            flags: ISCHAR,
            ch_nr: 8,
        };
        let chars = view.chars_within(3);
        assert_eq!(chars.len(), 1);
        assert_eq!(chars[0].ch_nr, 8);
        assert_eq!(chars[0].x, TILEX as i32 / 2 + 1);
    }

    #[test]
    fn scroll_right_drops_first_tile() {
        let mut view = WorldView::new();
        view.tiles[1].ch_nr = 9;
        view.apply(&cmd(
            ServerCommandType::ScrollRight,
            ServerCommandData::Empty,
        ));
        assert_eq!(view.tiles[0].ch_nr, 9);
    }

    #[test]
    fn scroll_down_shifts_one_row() {
        let mut view = WorldView::new();
        view.tiles[TILEX + 3].ch_nr = 5;
        view.apply(&cmd(
            ServerCommandType::ScrollDown,
            ServerCommandData::Empty,
        ));
        assert_eq!(view.tiles[3].ch_nr, 5);
    }

    #[test]
    fn scroll_left_up_shifts_by_row_plus_one() {
        let mut view = WorldView::new();
        view.tiles[0].ch_nr = 11;
        view.apply(&cmd(
            ServerCommandType::ScrollLeftUp,
            ServerCommandData::Empty,
        ));
        assert_eq!(view.tiles[TILEX + 1].ch_nr, 11);
    }
}
