//! Interpolates world positions between the two most recent server ticks so the
//! isometric view can be rendered at display rate while the simulation still
//! advances at the 36 Hz server tick.
//!
//! Positions are tracked in an absolute world-space screen projection so that
//! the sliding tile grid (`SV_SCROLL*` + `SV_SETORIGIN`) does not disturb
//! them: a character that finishes a step and is re-slotted into the next grid
//! tile keeps a continuous projected position.

use std::collections::HashMap;
use std::ops::{Add, Neg, Sub};

use mag_core::constants::{TILEX, TILEY};

use crate::game_map::GameMap;
use crate::types::map::{CMapTile, SUBPIXEL_UNIT};

use super::FLOOR_TILE_WIDTH;

/// Projected distance beyond which a position change is treated as a teleport
/// and shown without interpolation.
const SNAP_DISTANCE_SUB: i32 = FLOOR_TILE_WIDTH * SUBPIXEL_UNIT;

/// One `(2, ±1)` px step of the isometric pixel lattice, in [`SUBPIXEL_UNIT`] units.
const LATTICE_UNIT: i32 = 4 * SUBPIXEL_UNIT;

/// Projected world-space position in [`SUBPIXEL_UNIT`] units.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct WorldPos {
    x: i32,
    y: i32,
}

impl Add for WorldPos {
    type Output = Self;
    fn add(self, o: Self) -> Self {
        Self {
            x: self.x + o.x,
            y: self.y + o.y,
        }
    }
}

impl Sub for WorldPos {
    type Output = Self;
    fn sub(self, o: Self) -> Self {
        Self {
            x: self.x - o.x,
            y: self.y - o.y,
        }
    }
}

impl Neg for WorldPos {
    type Output = Self;
    fn neg(self) -> Self {
        Self {
            x: -self.x,
            y: -self.y,
        }
    }
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

    /// Floors each axis to a whole pixel.
    ///
    /// # Returns
    ///
    /// * The position with both components multiples of [`SUBPIXEL_UNIT`].
    fn floor_px(self) -> Self {
        Self {
            x: self.x.div_euclid(SUBPIXEL_UNIT) * SUBPIXEL_UNIT,
            y: self.y.div_euclid(SUBPIXEL_UNIT) * SUBPIXEL_UNIT,
        }
    }

    /// Rounds to the nearest point of the isometric pixel lattice spanned by
    /// `(2, 1)` and `(2, -1)` px.
    ///
    /// Floor-diamond edges have slope ±1/2, so only shifts on this lattice
    /// keep their rasterised staircase in phase; flooring x and y separately
    /// on a diagonal walk makes edges flicker between two phases.
    ///
    /// # Returns
    ///
    /// * The nearest lattice point, in [`SUBPIXEL_UNIT`] units.
    fn snap_lattice(self) -> Self {
        let m = (self.x + 2 * self.y + LATTICE_UNIT / 2).div_euclid(LATTICE_UNIT);
        let n = (self.x - 2 * self.y + LATTICE_UNIT / 2).div_euclid(LATTICE_UNIT);
        Self {
            x: 2 * (m + n) * SUBPIXEL_UNIT,
            y: (m - n) * SUBPIXEL_UNIT,
        }
    }
}

/// An interpolated screen-space offset together with its change over the
/// current tick, both in [`SUBPIXEL_UNIT`] units.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Motion {
    /// Offset for the frame being rendered.
    off: WorldPos,
    /// Offset change from the previous tick to the current one.
    vel: WorldPos,
}

impl Add for Motion {
    type Output = Self;
    fn add(self, o: Self) -> Self {
        Self {
            off: self.off + o.off,
            vel: self.vel + o.vel,
        }
    }
}

impl Neg for Motion {
    type Output = Self;
    fn neg(self) -> Self {
        Self {
            off: -self.off,
            vel: -self.vel,
        }
    }
}

impl Motion {
    /// Interpolates an object's offset from its tile anchor.
    ///
    /// # Arguments
    ///
    /// * `prev` - Projected position at the previous tick, if known.
    /// * `target` - Projected position at the current tick.
    /// * `anchor` - Tile anchor; also the motion origin when there is no history.
    /// * `alpha` - Interpolation factor in `[0, 1]`.
    ///
    /// # Returns
    ///
    /// * The raw (unrounded) offset and its per-tick velocity.
    fn of(prev: Option<WorldPos>, target: WorldPos, anchor: WorldPos, alpha: f32) -> Self {
        let (pos, from) = match prev {
            Some(prev) if prev.is_near(target) => (prev.lerp(target, alpha), prev),
            _ => (target, anchor),
        };
        Self {
            off: pos - anchor,
            vel: target - from,
        }
    }

    /// Rounds the offset to whole pixels: onto the isometric lattice when the
    /// motion is diagonal in screen space, per axis otherwise.
    ///
    /// # Returns
    ///
    /// * The pixel-aligned offset.
    fn quantise(self) -> WorldPos {
        if self.vel.x != 0 && self.vel.y != 0 {
            self.off.snap_lattice()
        } else {
            self.off.floor_px()
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
    /// * `(x, y)` in [`SUBPIXEL_UNIT`] units, rounded to whole pixels; equal
    ///   to `-centre.obj_*off_sub` when no usable history exists.
    pub(super) fn camera_offset(&self, map: &GameMap) -> (i32, i32) {
        let cam = self.camera_motion(map).quantise();
        (cam.x, cam.y)
    }

    /// Returns the raw camera offset motion (the negated centre-object motion).
    ///
    /// # Arguments
    ///
    /// * `map` - Live map whose centre tile anchors the camera.
    ///
    /// # Returns
    ///
    /// * The unrounded camera offset and its per-tick velocity; zero without a centre tile.
    fn camera_motion(&self, map: &GameMap) -> Motion {
        let Some(center) = map.tile_at_xy(TILEX / 2, TILEY / 2) else {
            return Motion::default();
        };
        let anchor = WorldPos::anchor(center);
        let target = WorldPos::of_object(center);
        -Motion::of(self.previous.camera, target, anchor, self.alpha)
    }

    /// Returns the interpolated sub-tile offset for the character on a tile.
    ///
    /// The camera and character offsets are rounded together so that two
    /// sprites a constant sub-pixel distance apart never alternate pixels;
    /// the result is `round(camera + character) - round(camera)`.
    ///
    /// # Arguments
    ///
    /// * `map` - Live map, used to derive the camera motion.
    /// * `tile` - Live tile holding the character.
    ///
    /// # Returns
    ///
    /// * `(x, y)` in [`SUBPIXEL_UNIT`] units relative to the tile's ground
    ///   diamond, rounded to whole pixels; falls back to the tile's own
    ///   offsets without history.
    pub(super) fn character_offset(&self, map: &GameMap, tile: &CMapTile) -> (i32, i32) {
        let anchor = WorldPos::anchor(tile);
        let target = WorldPos::of_object(tile);
        let prev = if tile.ch_nr != 0 {
            self.previous
                .characters
                .get(&(tile.ch_nr, tile.ch_id))
                .copied()
        } else {
            None
        };
        let ch = Motion::of(prev, target, anchor, self.alpha);
        let cam = self.camera_motion(map);
        let off = (cam + ch).quantise() - cam.quantise();
        (off.x, off.y)
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
        set_center_character(&mut map, 7, 6 * SUBPIXEL_UNIT, -3 * SUBPIXEL_UNIT);
        let interp = WorldInterpolator::new();

        assert_eq!(
            interp.camera_offset(&map),
            (-6 * SUBPIXEL_UNIT, 3 * SUBPIXEL_UNIT)
        );
        let tile = map.tile_at_xy(TILEX / 2, TILEY / 2).unwrap();
        assert_eq!(
            interp.character_offset(&map, tile),
            (6 * SUBPIXEL_UNIT, -3 * SUBPIXEL_UNIT)
        );
    }

    fn alpha_sweep() -> impl Iterator<Item = f32> {
        (0..=64).map(|i| i as f32 / 64.0)
    }

    #[test]
    fn diagonal_walk_stays_on_isometric_lattice() {
        let mut map = map_with_origin(100, 100);
        let mut interp = WorldInterpolator::new();
        set_center_character(&mut map, 7, 0, 0);
        interp.record_tick(&map);
        set_center_character(&mut map, 7, -16 * SUBPIXEL_UNIT, 8 * SUBPIXEL_UNIT);
        interp.record_tick(&map);

        for alpha in alpha_sweep() {
            interp.set_alpha(alpha);
            let (x, y) = interp.camera_offset(&map);
            assert_eq!(x % SUBPIXEL_UNIT, 0, "alpha {alpha}");
            assert_eq!(y % SUBPIXEL_UNIT, 0, "alpha {alpha}");
            let (px, py) = (x / SUBPIXEL_UNIT, y / SUBPIXEL_UNIT);
            assert_eq!(px % 2, 0, "alpha {alpha}: x {px} not even");
            assert_eq!(px, -2 * py, "alpha {alpha}: ({px}, {py}) off the 2:1 line");
        }
        interp.set_alpha(1.0);
        assert_eq!(
            interp.camera_offset(&map),
            (16 * SUBPIXEL_UNIT, -8 * SUBPIXEL_UNIT)
        );
    }

    #[test]
    fn horizontal_walk_keeps_single_pixel_steps() {
        let mut map = map_with_origin(100, 100);
        let mut interp = WorldInterpolator::new();
        set_center_character(&mut map, 7, 0, 0);
        interp.record_tick(&map);
        set_center_character(&mut map, 7, -8 * SUBPIXEL_UNIT, 0);
        interp.record_tick(&map);

        let mut seen = std::collections::HashSet::new();
        for alpha in alpha_sweep() {
            interp.set_alpha(alpha);
            let (x, y) = interp.camera_offset(&map);
            assert_eq!(y, 0);
            assert_eq!(x % SUBPIXEL_UNIT, 0);
            seen.insert(x / SUBPIXEL_UNIT);
        }
        assert!((0..=8).all(|px| seen.contains(&px)), "{seen:?}");
    }

    #[test]
    fn own_character_cancels_camera_during_diagonal_walk() {
        let mut map = map_with_origin(100, 100);
        let mut interp = WorldInterpolator::new();
        set_center_character(&mut map, 7, 3 * SUBPIXEL_UNIT, -SUBPIXEL_UNIT - 100);
        interp.record_tick(&map);
        set_center_character(
            &mut map,
            7,
            13 * SUBPIXEL_UNIT + 77,
            -6 * SUBPIXEL_UNIT - 200,
        );
        interp.record_tick(&map);

        let tile = map.tile_at_xy(TILEX / 2, TILEY / 2).unwrap();
        for alpha in alpha_sweep() {
            interp.set_alpha(alpha);
            let (cx, cy) = interp.camera_offset(&map);
            let (hx, hy) = interp.character_offset(&map, tile);
            assert_eq!((cx + hx, cy + hy), (0, 0), "alpha {alpha}");
        }
    }

    fn set_npc(map: &mut GameMap, idx: usize, xoff: i32, yoff: i32) {
        let tile = map.tile_at_index_mut(idx).unwrap();
        tile.ch_nr = 9;
        tile.ch_id = 2;
        tile.ch_sprite = 1000;
        tile.obj_xoff_sub = xoff;
        tile.obj_yoff_sub = yoff;
    }

    #[test]
    fn other_characters_walking_diagonally_stay_on_lattice() {
        let mut map = map_with_origin(100, 100);
        let mut interp = WorldInterpolator::new();
        set_center_character(&mut map, 7, 0, 0);
        let idx = GameMap::tile_index(3, 3).unwrap();
        set_npc(&mut map, idx, 0, 0);
        interp.record_tick(&map);
        set_npc(&mut map, idx, 16 * SUBPIXEL_UNIT, 8 * SUBPIXEL_UNIT);
        interp.record_tick(&map);

        let tile = map.tile_at_index(idx).unwrap();
        for alpha in alpha_sweep() {
            interp.set_alpha(alpha);
            let (x, y) = interp.character_offset(&map, tile);
            assert_eq!(x % SUBPIXEL_UNIT, 0);
            assert_eq!(x / SUBPIXEL_UNIT, 2 * (y / SUBPIXEL_UNIT), "alpha {alpha}");
        }
    }

    #[test]
    fn character_walking_alongside_player_holds_its_screen_position() {
        // NPC walks the same diagonal as the player, a constant off-pixel distance away.
        let (dx, dy) = (3 * SUBPIXEL_UNIT + 100, SUBPIXEL_UNIT + 37);
        let mut map = map_with_origin(100, 100);
        let mut interp = WorldInterpolator::new();
        let idx = GameMap::tile_index(3, 3).unwrap();
        set_center_character(&mut map, 7, 0, 0);
        set_npc(&mut map, idx, dx, dy);
        interp.record_tick(&map);
        set_center_character(&mut map, 7, -16 * SUBPIXEL_UNIT, 8 * SUBPIXEL_UNIT);
        set_npc(
            &mut map,
            idx,
            -16 * SUBPIXEL_UNIT + dx,
            8 * SUBPIXEL_UNIT + dy,
        );
        interp.record_tick(&map);

        let tile = map.tile_at_index(idx).unwrap();
        let mut screen = std::collections::HashSet::new();
        for alpha in alpha_sweep() {
            interp.set_alpha(alpha);
            let (cx, cy) = interp.camera_offset(&map);
            let (hx, hy) = interp.character_offset(&map, tile);
            screen.insert((cx + hx, cy + hy));
        }
        assert_eq!(screen.len(), 1, "{screen:?}");
    }

    #[test]
    fn stationary_character_moves_with_the_ground() {
        let mut map = map_with_origin(100, 100);
        let mut interp = WorldInterpolator::new();
        let idx = GameMap::tile_index(3, 3).unwrap();
        set_center_character(&mut map, 7, 0, 0);
        set_npc(&mut map, idx, 0, 0);
        interp.record_tick(&map);
        set_center_character(&mut map, 7, -8 * SUBPIXEL_UNIT, 0);
        interp.record_tick(&map);

        let tile = map.tile_at_index(idx).unwrap();
        for alpha in alpha_sweep() {
            interp.set_alpha(alpha);
            assert_eq!(interp.character_offset(&map, tile), (0, 0), "alpha {alpha}");
        }
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
            interp.character_offset(&map, tile),
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
        assert_eq!(interp.character_offset(&map, tile), (0, 0));
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
        assert_eq!(interp.character_offset(&map, tile), (6 * SUBPIXEL_UNIT, 0));
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
