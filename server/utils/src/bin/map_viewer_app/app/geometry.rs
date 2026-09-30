//! Isometric projection helpers ported from the legacy client's `dd.c` math.

use eframe::egui;
use egui::{Pos2, Rect, Vec2};
use mag_core::constants::{SERVER_MAPX, SERVER_MAPY, TILEX, XPOS, YPOS};

/// Smallest allowed camera zoom factor.
pub(super) const MIN_ZOOM: f32 = 0.25;
/// Largest allowed camera zoom factor.
pub(super) const MAX_ZOOM: f32 = 4.0;
/// Multiplicative zoom change per mouse-wheel notch.
pub(super) const ZOOM_STEP: f32 = 1.15;

/// Linear index of tile `(x, y)` in the flat map vector.
#[inline]
pub(super) fn tile_index(x: usize, y: usize) -> usize {
    y * (SERVER_MAPX as usize) + x
}

/// Camera state needed to convert between map-space pixels and screen space.
#[derive(Clone, Copy, Debug)]
pub(super) struct MapView {
    /// Canvas rect in screen space.
    pub(super) rect: Rect,
    /// Camera pan in screen pixels.
    pub(super) pan: Vec2,
    /// Camera zoom applied to map-space pixels.
    pub(super) zoom: f32,
}

impl MapView {
    /// Convert map-space pixels into screen-space coordinates.
    #[inline]
    pub(super) fn to_screen(self, map_pos: Vec2) -> Pos2 {
        self.rect.min + self.pan + map_pos * self.zoom
    }

    /// Convert screen-space coordinates into map-space pixels.
    #[inline]
    pub(super) fn to_map(self, screen_pos: Pos2) -> Vec2 {
        (screen_pos - self.rect.min - self.pan) / self.zoom
    }

    /// Screen position of the floor diamond center of `tile`.
    pub(super) fn tile_center(self, tile: (usize, usize)) -> Pos2 {
        let (cx, cy) = dd_tile_center_screen_pos(tile.0 as i32 * 32, tile.1 as i32 * 32);
        self.to_screen(Vec2::new(cx as f32, cy as f32))
    }

    /// Tile under a screen position, if it lies on the map.
    pub(super) fn tile_at(self, screen_pos: Pos2) -> Option<(usize, usize)> {
        map_point_to_tile(self.to_map(screen_pos))
    }

    /// Inclusive tile ranges `((x0, x1), (y0, y1))` covering the canvas plus `margin` tiles.
    pub(super) fn visible_tile_range(self, margin: i32) -> ((usize, usize), (usize, usize)) {
        let corners = [
            self.rect.left_top(),
            self.rect.right_top(),
            self.rect.left_bottom(),
            self.rect.right_bottom(),
        ];
        let (ox, oy) = dd_tile_origin_screen_pos(0, 0);
        let mut min = Vec2::splat(f32::INFINITY);
        let mut max = Vec2::splat(f32::NEG_INFINITY);
        for corner in corners {
            let local = self.to_map(corner);
            let base_x = local.x - ox as f32;
            let base_y = local.y - oy as f32;
            let tile = Vec2::new(
                0.5 * (base_x / 16.0 + base_y / 8.0),
                0.5 * (base_x / 16.0 - base_y / 8.0),
            );
            min = min.min(tile);
            max = max.max(tile);
        }

        let clamp = |lo: f32, hi: f32, limit: i32| {
            let lo = (lo.floor() as i32 - margin).clamp(0, limit - 1);
            let hi = (hi.ceil() as i32 + margin).clamp(0, limit - 1);
            (lo as usize, hi as usize)
        };
        (
            clamp(min.x, max.x, SERVER_MAPX),
            clamp(min.y, max.y, SERVER_MAPY),
        )
    }
}

/// Tile origin in map-space pixels before sprite-size offsets.
///
/// Ported from client gameplay `legacy_engine::copysprite_screen_pos` (dd.c copysprite).
/// The negative-coordinate odd-bit adjustments are ignored because `xpos`/`ypos` are `>= 0`.
#[inline]
pub(super) fn dd_tile_origin_screen_pos(xpos: i32, ypos: i32) -> (i32, i32) {
    let rx = (xpos / 2) + (ypos / 2) + 32 + XPOS - (((TILEX as i32 - 34) / 2) * 32);
    let ry = (xpos / 4) - (ypos / 4) + YPOS;
    (rx, ry)
}

/// Visual center of the isometric floor diamond in map-space pixels.
///
/// The 32x32 floor sprite is drawn top-left at `(rx - 16, ry - 32)` (see
/// [`dd_copysprite_screen_pos`]), and the floor diamond is its bottom half, so
/// the center is `ry - 8`. Picking and markers both anchor on this.
pub(super) fn dd_tile_center_screen_pos(xpos: i32, ypos: i32) -> (i32, i32) {
    let (rx, ry) = dd_tile_origin_screen_pos(xpos, ypos);
    (rx, ry - 8)
}

/// Tile containing a map-space point, by exactly inverting the isometric projection.
///
/// One tile step in `x` moves the diamond center `(+16, +8)` and one in `y`
/// moves it `(+16, -8)`; in that basis diamonds form a unit square grid, so
/// rounding is exact with no boundary ties.
pub(super) fn map_point_to_tile(map_pos: Vec2) -> Option<(usize, usize)> {
    let (ax, ay) = dd_tile_center_screen_pos(0, 0);
    let u = (map_pos.x - ax as f32) / 16.0;
    let v = (map_pos.y - ay as f32) / 8.0;

    let x = ((u + v) * 0.5).round() as i32;
    let y = ((u - v) * 0.5).round() as i32;

    if x < 0 || y < 0 || x >= SERVER_MAPX || y >= SERVER_MAPY {
        return None;
    }

    Some((x as usize, y as usize))
}

/// Top-left map-space position of a sprite spanning `xs` x `ys` tiles.
///
/// Ported from client gameplay `legacy_engine::copysprite_screen_pos` (dd.c copysprite).
#[inline]
pub(super) fn dd_copysprite_screen_pos(xpos: i32, ypos: i32, xs: i32, ys: i32) -> (i32, i32) {
    let (rx, ry) = dd_tile_origin_screen_pos(xpos, ypos);
    (rx - xs * 16, ry - ys * 32)
}

/// Every map tile touched by a straight Bresenham line, endpoints included.
pub(super) fn line_tiles(start: (usize, usize), end: (usize, usize)) -> Vec<(usize, usize)> {
    let (mut x0, mut y0) = (start.0 as i32, start.1 as i32);
    let (x1, y1) = (end.0 as i32, end.1 as i32);
    let dx = (x1 - x0).abs();
    let dy = -(y1 - y0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;
    let mut points = Vec::new();

    loop {
        if x0 >= 0 && y0 >= 0 && x0 < SERVER_MAPX && y0 < SERVER_MAPY {
            points.push((x0 as usize, y0 as usize));
        }
        if x0 == x1 && y0 == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x0 += sx;
        }
        if e2 <= dx {
            err += dx;
            y0 += sy;
        }
    }

    points
}

#[cfg(test)]
mod tests {
    use super::{MapView, dd_tile_center_screen_pos, line_tiles, map_point_to_tile};
    use eframe::egui::{Pos2, Rect, Vec2};

    #[test]
    fn line_tiles_single_point() {
        assert_eq!(line_tiles((7, 9), (7, 9)), vec![(7, 9)]);
    }

    #[test]
    fn line_tiles_horizontal_includes_endpoints() {
        assert_eq!(
            line_tiles((2, 4), (6, 4)),
            vec![(2, 4), (3, 4), (4, 4), (5, 4), (6, 4)]
        );
    }

    #[test]
    fn line_tiles_vertical_includes_endpoints() {
        assert_eq!(
            line_tiles((3, 2), (3, 6)),
            vec![(3, 2), (3, 3), (3, 4), (3, 5), (3, 6)]
        );
    }

    #[test]
    fn line_tiles_diagonal_includes_endpoints() {
        assert_eq!(
            line_tiles((1, 1), (4, 4)),
            vec![(1, 1), (2, 2), (3, 3), (4, 4)]
        );
    }

    #[test]
    fn line_tiles_shallow_slope() {
        assert_eq!(
            line_tiles((1, 1), (5, 3)),
            vec![(1, 1), (2, 2), (3, 2), (4, 3), (5, 3)]
        );
    }

    #[test]
    fn line_tiles_steep_slope() {
        assert_eq!(
            line_tiles((1, 1), (3, 5)),
            vec![(1, 1), (2, 2), (2, 3), (3, 4), (3, 5)]
        );
    }

    #[test]
    fn line_tiles_reversed_matches_reverse_order() {
        assert_eq!(
            line_tiles((5, 3), (1, 1)),
            vec![(5, 3), (4, 2), (3, 2), (2, 1), (1, 1)]
        );
    }

    #[test]
    fn map_point_to_tile_resolves_tile_center_to_same_tile() {
        let (cx, cy) = dd_tile_center_screen_pos(10 * 32, 20 * 32);
        assert_eq!(
            map_point_to_tile(Vec2::new(cx as f32, cy as f32)),
            Some((10, 20))
        );
    }

    #[test]
    fn map_point_to_tile_separates_vertical_neighbor_centers() {
        let (cx, cy) = dd_tile_center_screen_pos(10 * 32, 20 * 32);
        let (below_cx, below_cy) = dd_tile_center_screen_pos(10 * 32, 21 * 32);
        assert_eq!(
            map_point_to_tile(Vec2::new(cx as f32, cy as f32)),
            Some((10, 20))
        );
        assert_eq!(
            map_point_to_tile(Vec2::new(below_cx as f32, below_cy as f32)),
            Some((10, 21))
        );
    }

    #[test]
    fn map_point_to_tile_matches_screen_stacked_neighbors() {
        // Locks the floor-diamond center orientation so it cannot silently flip sign again.
        let (cx, cy) = dd_tile_center_screen_pos(10 * 32, 20 * 32);
        assert_eq!(
            map_point_to_tile(Vec2::new(cx as f32, cy as f32 + 16.0)),
            Some((11, 19))
        );
        assert_eq!(
            map_point_to_tile(Vec2::new(cx as f32, cy as f32 - 16.0)),
            Some((9, 21))
        );
    }

    #[test]
    fn map_point_to_tile_stays_on_tile_across_interior() {
        let (cx, cy) = dd_tile_center_screen_pos(40 * 32, 25 * 32);
        let interior = [
            (0.0, 0.0),
            (10.0, 0.0),
            (-10.0, 0.0),
            (0.0, 5.0),
            (0.0, -5.0),
            (6.0, 3.0),
            (-6.0, -3.0),
        ];
        for (dx, dy) in interior {
            assert_eq!(
                map_point_to_tile(Vec2::new(cx as f32 + dx, cy as f32 + dy)),
                Some((40, 25)),
                "interior offset ({dx}, {dy}) should stay on (40, 25)"
            );
        }
    }

    #[test]
    fn map_view_tile_center_roundtrips_through_tile_at() {
        let view = MapView {
            rect: Rect::from_min_size(Pos2::new(50.0, 30.0), Vec2::new(800.0, 600.0)),
            pan: Vec2::new(-1234.0, 567.0),
            zoom: 1.7,
        };
        for tile in [(0, 0), (10, 20), (40, 25), (511, 3)] {
            assert_eq!(view.tile_at(view.tile_center(tile)), Some(tile));
        }
    }

    #[test]
    fn visible_tile_range_contains_center_tile() {
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
        let (cx, cy) = dd_tile_center_screen_pos(100 * 32, 200 * 32);
        let zoom = 1.0;
        let view = MapView {
            rect,
            pan: (rect.center() - rect.min) - Vec2::new(cx as f32, cy as f32) * zoom,
            zoom,
        };
        assert_eq!(view.tile_at(rect.center()), Some((100, 200)));
        let ((x0, x1), (y0, y1)) = view.visible_tile_range(0);
        assert!((x0..=x1).contains(&100));
        assert!((y0..=y1).contains(&200));
    }
}
