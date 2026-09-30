//! Interpolates world positions between the two most recent server ticks so the
//! isometric view can be rendered at display rate while the simulation still
//! advances at the 36 Hz server tick.
//!
//! Positions are tracked in an absolute world-space screen projection so that
//! the sliding tile grid (`SV_SCROLL*` + `SV_SETORIGIN`) does not disturb
//! them: a character that finishes a step and is re-slotted into the next grid
//! tile keeps a continuous projected position.

use std::collections::HashMap;

use mag_core::constants::{TILEX, TILEY};

use crate::game_map::GameMap;
use crate::types::map::{CMapTile, SUBPIXEL_UNIT};

use super::FLOOR_TILE_WIDTH;

/// Projected distance beyond which a position change is treated as a teleport
/// and shown without interpolation.
const SNAP_DISTANCE_SUB: i32 = FLOOR_TILE_WIDTH * SUBPIXEL_UNIT;

/// Projected world-space position in [`SUBPIXEL_UNIT`] units.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct WorldPos {
    x: i32,
    y: i32,
}

impl WorldPos {
    /// Returns the linear part of the ground-diamond projection for a tile.
    ///
    /// # Arguments
    ///
    /// * `tile` - Tile whose absolute `x`/`y` world coordinates are projected.
    ///
    /// # Returns
    ///
    /// * The projected anchor, without screen-origin constants.
    fn anchor(tile: &CMapTile) -> Self {
        // `set_origin` stores world coordinates as wrapped `u16`; reinterpret
        // as `i16` so tiles left of the map origin project consistently.
        let xpos = i32::from(tile.x as i16) * FLOOR_TILE_WIDTH;
        let ypos = i32::from(tile.y as i16) * FLOOR_TILE_WIDTH;
        Self {
            x: (xpos / 2 + ypos / 2) * SUBPIXEL_UNIT,
            y: (xpos / 4 - ypos / 4) * SUBPIXEL_UNIT,
        }
    }

    /// Returns the projected position of the object occupying a tile.
    ///
    /// # Arguments
    ///
    /// * `tile` - Tile carrying the object's sub-tile movement offsets.
    ///
    /// # Returns
    ///
    /// * The tile anchor displaced by `obj_xoff_sub` / `obj_yoff_sub`.
    fn of_object(tile: &CMapTile) -> Self {
        let anchor = Self::anchor(tile);
        Self {
            x: anchor.x + tile.obj_xoff_sub,
            y: anchor.y + tile.obj_yoff_sub,
        }
    }

    /// Returns whether two positions are close enough to interpolate between.
    ///
    /// # Arguments
    ///
    /// * `other` - Position to compare against.
    ///
    /// # Returns
    ///
    /// * `false` when the move looks like a teleport.
    fn is_near(self, other: Self) -> bool {
        (self.x - other.x).abs() < SNAP_DISTANCE_SUB && (self.y - other.y).abs() < SNAP_DISTANCE_SUB
    }

    /// Linearly interpolates from `self` toward `target`.
    ///
    /// # Arguments
    ///
    /// * `target` - Position reached at `alpha == 1`.
    /// * `alpha` - Interpolation factor in `[0, 1]`.
    ///
    /// # Returns
    ///
    /// * The interpolated position.
    fn lerp(self, target: Self, alpha: f32) -> Self {
        Self {
            x: lerp_i32(self.x, target.x, alpha),
            y: lerp_i32(self.y, target.y, alpha),
        }
    }
}

/// Interpolates between two integers with a floating-point factor.
///
/// # Arguments
///
/// * `from` - Value at `alpha == 0`.
/// * `to` - Value at `alpha == 1`.
/// * `alpha` - Interpolation factor in `[0, 1]`.
///
/// # Returns
///
/// * The interpolated value, rounded toward negative infinity.
fn lerp_i32(from: i32, to: i32, alpha: f32) -> i32 {
    from + ((to - from) as f32 * alpha).floor() as i32
}

/// Positions captured after one server tick was applied.
#[derive(Debug, Default)]
struct TickSnapshot {
    /// Projected camera position (the centre tile plus its movement offset).
    camera: Option<WorldPos>,
    /// Projected position of every visible character keyed by `(ch_nr, ch_id)`.
    characters: HashMap<(u16, u16), WorldPos>,
}

impl TickSnapshot {
    /// Refills the snapshot from the current map state.
    ///
    /// # Arguments
    ///
    /// * `map` - Map whose tiles have just been advanced by one tick.
    fn capture(&mut self, map: &GameMap) {
        self.characters.clear();
        self.camera = map
            .tile_at_xy(TILEX / 2, TILEY / 2)
            .map(WorldPos::of_object);
        for index in 0..map.len() {
            let Some(tile) = map.tile_at_index(index) else {
                continue;
            };
            if tile.ch_nr == 0 || tile.ch_sprite == 0 {
                continue;
            }
            self.characters
                .insert((tile.ch_nr, tile.ch_id), WorldPos::of_object(tile));
        }
    }
}

/// Render-side interpolator between the previous and current server tick.
#[derive(Debug, Default)]
pub(super) struct WorldInterpolator {
    /// Snapshot from the tick before the current one.
    previous: TickSnapshot,
    /// Snapshot from the most recently applied tick.
    current: TickSnapshot,
    /// Progress from `previous` toward `current` for the frame being rendered.
    alpha: f32,
}

impl WorldInterpolator {
    /// Creates an interpolator with no history; offsets pass through unchanged.
    ///
    /// # Returns
    ///
    /// * An empty interpolator.
    pub(super) fn new() -> Self {
        Self {
            alpha: 1.0,
            ..Self::default()
        }
    }

    /// Drops all history, e.g. on login or map reload.
    pub(super) fn reset(&mut self) {
        self.previous.camera = None;
        self.previous.characters.clear();
        self.current.camera = None;
        self.current.characters.clear();
        self.alpha = 1.0;
    }

    /// Records the map state after a server tick has been applied.
    ///
    /// # Arguments
    ///
    /// * `map` - Map advanced by the tick that was just applied.
    pub(super) fn record_tick(&mut self, map: &GameMap) {
        std::mem::swap(&mut self.previous, &mut self.current);
        self.current.capture(map);
    }

    /// Sets the interpolation factor for the frame about to be rendered.
    ///
    /// # Arguments
    ///
    /// * `alpha` - Progress from the previous tick toward the current one.
    pub(super) fn set_alpha(&mut self, alpha: f32) {
        self.alpha = alpha.clamp(0.0, 1.0);
    }

    /// Returns the interpolated camera offset for the current grid.
    ///
    /// # Arguments
    ///
    /// * `map` - Live map whose centre tile anchors the camera.
    ///
    /// # Returns
    ///
    /// * `(x, y)` in [`SUBPIXEL_UNIT`] units, equal to `-centre.obj_*off_sub`
    ///   when no usable history exists.
    pub(super) fn camera_offset(&self, map: &GameMap) -> (i32, i32) {
        let Some(center) = map.tile_at_xy(TILEX / 2, TILEY / 2) else {
            return (0, 0);
        };
        let anchor = WorldPos::anchor(center);
        let target = WorldPos::of_object(center);
        let camera = match self.previous.camera {
            Some(prev) if prev.is_near(target) => prev.lerp(target, self.alpha),
            _ => target,
        };
        (anchor.x - camera.x, anchor.y - camera.y)
    }

    /// Returns the interpolated sub-tile offset for the character on a tile.
    ///
    /// # Arguments
    ///
    /// * `tile` - Live tile holding the character.
    ///
    /// # Returns
    ///
    /// * `(x, y)` in [`SUBPIXEL_UNIT`] units relative to the tile's ground
    ///   diamond; falls back to the tile's own offsets without history.
    pub(super) fn character_offset(&self, tile: &CMapTile) -> (i32, i32) {
        let target = WorldPos::of_object(tile);
        let prev = if tile.ch_nr != 0 {
            self.previous.characters.get(&(tile.ch_nr, tile.ch_id))
        } else {
            None
        };
        match prev {
            Some(&prev) if prev.is_near(target) => {
                let anchor = WorldPos::anchor(tile);
                let pos = prev.lerp(target, self.alpha);
                (pos.x - anchor.x, pos.y - anchor.y)
            }
            _ => (tile.obj_xoff_sub, tile.obj_yoff_sub),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map_with_origin(xp: i16, yp: i16) -> GameMap {
        let mut map = GameMap::new();
        map.set_origin(xp, yp);
        map
    }

    fn set_center_character(map: &mut GameMap, ch_nr: u16, xoff: i32, yoff: i32) {
        let idx = GameMap::tile_index(TILEX / 2, TILEY / 2).unwrap();
        let tile = map.tile_at_index_mut(idx).unwrap();
        tile.ch_nr = ch_nr;
        tile.ch_id = 1;
        tile.ch_sprite = 1000;
        tile.obj_xoff_sub = xoff;
        tile.obj_yoff_sub = yoff;
    }

    #[test]
    fn without_history_offsets_pass_through() {
        let mut map = map_with_origin(100, 100);
        set_center_character(&mut map, 7, 5 * SUBPIXEL_UNIT, -3 * SUBPIXEL_UNIT);
        let interp = WorldInterpolator::new();

        assert_eq!(
            interp.camera_offset(&map),
            (-5 * SUBPIXEL_UNIT, 3 * SUBPIXEL_UNIT)
        );
        let tile = map.tile_at_xy(TILEX / 2, TILEY / 2).unwrap();
        assert_eq!(
            interp.character_offset(tile),
            (5 * SUBPIXEL_UNIT, -3 * SUBPIXEL_UNIT)
        );
    }

    #[test]
    fn alpha_one_matches_current_tick_exactly() {
        let mut map = map_with_origin(100, 100);
        let mut interp = WorldInterpolator::new();
        set_center_character(&mut map, 7, 0, 0);
        interp.record_tick(&map);
        set_center_character(&mut map, 7, -8 * SUBPIXEL_UNIT, 4 * SUBPIXEL_UNIT);
        interp.record_tick(&map);
        interp.set_alpha(1.0);

        assert_eq!(
            interp.camera_offset(&map),
            (8 * SUBPIXEL_UNIT, -4 * SUBPIXEL_UNIT)
        );
    }

    #[test]
    fn midpoint_alpha_halves_the_movement() {
        let mut map = map_with_origin(100, 100);
        let mut interp = WorldInterpolator::new();
        set_center_character(&mut map, 7, 0, 0);
        interp.record_tick(&map);
        set_center_character(&mut map, 7, -8 * SUBPIXEL_UNIT, 4 * SUBPIXEL_UNIT);
        interp.record_tick(&map);
        interp.set_alpha(0.5);

        assert_eq!(
            interp.camera_offset(&map),
            (4 * SUBPIXEL_UNIT, -2 * SUBPIXEL_UNIT)
        );
        let tile = map.tile_at_xy(TILEX / 2, TILEY / 2).unwrap();
        assert_eq!(
            interp.character_offset(tile),
            (-4 * SUBPIXEL_UNIT, 2 * SUBPIXEL_UNIT)
        );
    }

    #[test]
    fn camera_stays_continuous_across_a_grid_scroll() {
        // Walking one tile east: the offset approaches a full tile, then the
        // grid scrolls, the origin advances and the offset wraps to zero.
        let mut map = map_with_origin(100, 100);
        let mut interp = WorldInterpolator::new();
        set_center_character(&mut map, 7, 14 * SUBPIXEL_UNIT, 7 * SUBPIXEL_UNIT);
        interp.record_tick(&map);

        map.scroll_right();
        map.set_origin(101, 100);
        set_center_character(&mut map, 7, 0, 0);
        interp.record_tick(&map);

        // Projected step for +1 in x is (+16, +8) px, so the previous position
        // sits (-2, -1) px behind the new anchor.
        interp.set_alpha(0.0);
        assert_eq!(
            interp.camera_offset(&map),
            (2 * SUBPIXEL_UNIT, SUBPIXEL_UNIT)
        );
        interp.set_alpha(1.0);
        assert_eq!(interp.camera_offset(&map), (0, 0));
    }

    #[test]
    fn teleports_snap_instead_of_sliding() {
        let mut map = map_with_origin(100, 100);
        let mut interp = WorldInterpolator::new();
        set_center_character(&mut map, 7, 0, 0);
        interp.record_tick(&map);

        map.set_origin(300, 300);
        set_center_character(&mut map, 7, 0, 0);
        interp.record_tick(&map);
        interp.set_alpha(0.0);

        assert_eq!(interp.camera_offset(&map), (0, 0));
        let tile = map.tile_at_xy(TILEX / 2, TILEY / 2).unwrap();
        assert_eq!(interp.character_offset(tile), (0, 0));
    }

    #[test]
    fn unknown_characters_use_their_tile_offsets() {
        let mut map = map_with_origin(100, 100);
        let mut interp = WorldInterpolator::new();
        set_center_character(&mut map, 7, 0, 0);
        interp.record_tick(&map);
        interp.record_tick(&map);

        let idx = GameMap::tile_index(3, 3).unwrap();
        {
            let tile = map.tile_at_index_mut(idx).unwrap();
            tile.ch_nr = 9;
            tile.ch_id = 2;
            tile.ch_sprite = 1000;
            tile.obj_xoff_sub = 6 * SUBPIXEL_UNIT;
        }
        interp.set_alpha(0.0);
        let tile = map.tile_at_index(idx).unwrap();
        assert_eq!(interp.character_offset(tile), (6 * SUBPIXEL_UNIT, 0));
    }

    #[test]
    fn reset_clears_history() {
        let mut map = map_with_origin(100, 100);
        let mut interp = WorldInterpolator::new();
        set_center_character(&mut map, 7, 0, 0);
        interp.record_tick(&map);
        set_center_character(&mut map, 7, -8 * SUBPIXEL_UNIT, 0);
        interp.record_tick(&map);
        interp.set_alpha(0.0);
        interp.reset();

        assert_eq!(interp.camera_offset(&map), (8 * SUBPIXEL_UNIT, 0));
    }
}
