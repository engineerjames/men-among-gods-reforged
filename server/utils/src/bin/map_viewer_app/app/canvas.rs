//! Central map canvas: input handling, tile rendering, markers, and the flag overlay.

use super::MapViewerApp;
use super::editing::item_map_sprite;
use super::flags::flag_combo_color;
use super::geometry::{
    MAX_ZOOM, MIN_ZOOM, MapView, ZOOM_STEP, dd_copysprite_screen_pos, dd_tile_center_screen_pos,
    dd_tile_origin_screen_pos, line_tiles, tile_index,
};
use crate::map_viewer_app::graphics::GraphicsZipCache;
use eframe::egui;
use egui::{Pos2, Rect, Vec2};
use mag_core::constants::{SERVER_MAPX, SERVER_MAPY};
use std::collections::BTreeMap;

/// Extra tiles drawn past the canvas edges, since sprites extend beyond their anchor.
const VISIBLE_TILE_MARGIN: i32 = 6;

/// Tint applied to items under the cursor or selection.
const ITEM_HIGHLIGHT: egui::Color32 = egui::Color32::from_rgb(255, 50, 50);

impl MapViewerApp {
    /// Central panel hosting the palette overlay and the map canvas.
    pub(super) fn ui_map_canvas(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            let (rect, response) =
                ui.allocate_exact_size(ui.available_size(), egui::Sense::click_and_drag());

            let palette_rect =
                self.render_palette_overlay(ctx, rect.left_top() + Vec2::new(12.0, 12.0));
            self.palette_rect = Some(palette_rect);

            if response.dragged() {
                self.pan += response.drag_delta();
                ctx.request_repaint();
            }
            if response.clicked_by(egui::PointerButton::Primary) {
                self.handle_canvas_click(ctx, palette_rect);
            }
            if !ctx.input(|i| i.modifiers.shift) {
                self.line_anchor = None;
            }

            if !self.pan_initialized && !self.map_tiles.is_empty() {
                self.center_view_on_map(rect);
            }
            self.handle_zoom(ctx, rect, palette_rect);

            let view = self.view(rect);
            self.hovered_tile = ctx
                .pointer_latest_pos()
                .filter(|pos| rect.contains(*pos))
                .and_then(|pos| view.tile_at(pos));

            let painter = ui.painter_at(rect);
            painter.rect_filled(rect, 0.0, egui::Color32::from_rgb(20, 22, 26));

            let message = if self.map_tiles.is_empty() {
                Some("No map loaded (Open dat dir...) ")
            } else if self.graphics_zip.is_none() {
                Some("No graphics zip loaded (Open graphics zip...) ")
            } else {
                None
            };
            if let Some(message) = message {
                painter.text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    message,
                    egui::TextStyle::Heading.resolve(ui.style()),
                    egui::Color32::GRAY,
                );
                return;
            }

            let (x_range, y_range) = view.visible_tile_range(VISIBLE_TILE_MARGIN);
            self.paint_tiles(&painter, ctx, view, x_range, y_range);
            if self.flag_viz_enabled {
                self.paint_flag_overlay(&painter, view, x_range, y_range);
            }
            self.paint_markers(&painter, view);
        });
    }

    /// Current camera for a canvas rect.
    fn view(&self, rect: Rect) -> MapView {
        MapView {
            rect,
            pan: self.pan,
            zoom: self.zoom,
        }
    }

    /// Center the camera on the middle of the map (first paint after load).
    fn center_view_on_map(&mut self, rect: Rect) {
        let mid_x = SERVER_MAPX / 2;
        let mid_y = SERVER_MAPY / 2;
        let (tx, ty) = dd_tile_origin_screen_pos(mid_x * 32, mid_y * 32);
        self.pan = (rect.center() - rect.min) - Vec2::new(tx as f32, ty as f32) * self.zoom;
        self.pan_initialized = true;
    }

    /// Paint the selected palette entry (Shift = line from the previous click),
    /// or select the hovered tile when no palette entry is active.
    fn handle_canvas_click(&mut self, ctx: &egui::Context, palette_rect: Rect) {
        let clicked_palette = ctx
            .pointer_latest_pos()
            .is_some_and(|p| palette_rect.contains(p));
        if clicked_palette {
            self.line_anchor = None;
            return;
        }

        let Some(entry) = self.selected_palette_entry() else {
            self.line_anchor = None;
            if let Some(tile) = self.hovered_tile {
                self.selected_tile = Some(tile);
                ctx.request_repaint();
            }
            return;
        };
        let Some((x, y)) = self.hovered_tile else {
            self.line_anchor = None;
            return;
        };

        let shift_held = ctx.input(|i| i.modifiers.shift);
        let coords = if shift_held {
            line_tiles(self.line_anchor.unwrap_or((x, y)), (x, y))
        } else {
            vec![(x, y)]
        };

        let undo_snapshot = self.snapshot_for_undo(&coords);
        let mut changed = false;
        for (line_x, line_y) in coords {
            changed |= self.apply_palette_to_tile(line_x, line_y, entry);
        }
        self.line_anchor = shift_held.then_some((x, y));

        if changed {
            self.push_undo(undo_snapshot);
            ctx.request_repaint();
        }
    }

    /// Mouse-wheel zoom anchored on the map point under the cursor.
    fn handle_zoom(&mut self, ctx: &egui::Context, rect: Rect, palette_rect: Rect) {
        let zoom_delta = ctx.input(|i| i.raw_scroll_delta.y);
        let Some(pointer_pos) = ctx.pointer_latest_pos() else {
            return;
        };
        if zoom_delta == 0.0 || !rect.contains(pointer_pos) || palette_rect.contains(pointer_pos) {
            return;
        }

        let factor = if zoom_delta > 0.0 {
            ZOOM_STEP
        } else {
            1.0 / ZOOM_STEP
        };
        let new_zoom = (self.zoom * factor).clamp(MIN_ZOOM, MAX_ZOOM);
        if (new_zoom - self.zoom).abs() <= f32::EPSILON {
            return;
        }
        let map_under_pointer = self.view(rect).to_map(pointer_pos);
        self.zoom = new_zoom;
        self.pan = pointer_pos - rect.min - map_under_pointer * new_zoom;
        ctx.request_repaint();
    }

    /// Draw floor, object, and item sprites in legacy back-to-front order.
    fn paint_tiles(
        &mut self,
        painter: &egui::Painter,
        ctx: &egui::Context,
        view: MapView,
        x_range: (usize, usize),
        y_range: (usize, usize),
    ) {
        let Some(cache) = self.graphics_zip.as_mut() else {
            return;
        };
        let mut last_error = None;

        // Larger `y` is higher on screen (ry ~= 8*x - 8*y), so it must be drawn first.
        for y in (y_range.0..=y_range.1).rev() {
            for x in x_range.0..=x_range.1 {
                let Some(tile) = self.map_tiles.get(tile_index(x, y)).copied() else {
                    continue;
                };
                let mut draw = |sprite: usize, tint| {
                    if let Err(e) = paint_sprite_dd(painter, ctx, cache, view, sprite, (x, y), tint)
                    {
                        last_error = Some(e);
                    }
                };

                if tile.sprite != 0 {
                    draw(tile.sprite as usize, egui::Color32::WHITE);
                }

                if tile.fsprite != 0 {
                    // Mirror the client's Hide Walls: substitute `sprite + 1`.
                    let sprite = if self.hide_enabled {
                        tile.fsprite + 1
                    } else {
                        tile.fsprite
                    };
                    draw(sprite as usize, egui::Color32::WHITE);
                } else if tile.it != 0
                    && let Some(item) = self.items.get(tile.it as usize)
                    && let Some(sprite) = item_map_sprite(*item)
                {
                    let highlighted =
                        self.hovered_tile == Some((x, y)) || self.selected_tile == Some((x, y));
                    let tint = if highlighted {
                        ITEM_HIGHLIGHT
                    } else {
                        egui::Color32::WHITE
                    };
                    draw(sprite as usize, tint);
                }
            }
        }

        if let Some(e) = last_error {
            self.graphics_zip_error = Some(e);
        }
    }

    /// Hover, line-anchor, and selection markers.
    fn paint_markers(&self, painter: &egui::Painter, view: MapView) {
        if let Some(tile) = self.hovered_tile {
            let radius = (6.0 * self.zoom).clamp(4.0, 14.0);
            painter.circle_stroke(view.tile_center(tile), radius, (2.0, egui::Color32::YELLOW));
        }

        if let (Some(anchor), Some(hovered)) = (self.line_anchor, self.hovered_tile) {
            let start = view.tile_center(anchor);
            let stroke = (2.0, egui::Color32::LIGHT_GREEN);
            painter.line_segment([start, view.tile_center(hovered)], stroke);
            painter.circle_stroke(start, (5.0 * self.zoom).clamp(4.0, 12.0), stroke);
        }

        if let Some(tile) = self.selected_tile {
            let radius = (7.0 * self.zoom).clamp(5.0, 16.0);
            painter.circle_stroke(view.tile_center(tile), radius, (3.0, ITEM_HIGHLIGHT));
        }
    }

    /// Tint visible tiles by their filtered flag combination and refresh the legend.
    ///
    /// # Arguments
    ///
    /// * `painter` - Painter clipped to the map canvas.
    /// * `view` - Current camera.
    /// * `x_range` - Inclusive tile x range to scan.
    /// * `y_range` - Inclusive tile y range to scan.
    fn paint_flag_overlay(
        &mut self,
        painter: &egui::Painter,
        view: MapView,
        x_range: (usize, usize),
        y_range: (usize, usize),
    ) {
        const DIAMOND: [(i32, i32); 4] = [(0, -8), (16, 0), (0, 8), (-16, 0)];

        let mut combos: BTreeMap<u64, usize> = BTreeMap::new();
        for y in y_range.0..=y_range.1 {
            for x in x_range.0..=x_range.1 {
                let Some(tile) = self.map_tiles.get(tile_index(x, y)) else {
                    continue;
                };
                let flags = tile.flags & self.flag_viz_mask;
                if flags == 0 {
                    continue;
                }

                let (cx, cy) = dd_tile_center_screen_pos(x as i32 * 32, y as i32 * 32);
                let points: Vec<Pos2> = DIAMOND
                    .iter()
                    .map(|(dx, dy)| view.to_screen(Vec2::new((cx + dx) as f32, (cy + dy) as f32)))
                    .collect();
                if !points.iter().any(|p| view.rect.contains(*p)) {
                    continue;
                }

                painter.add(egui::Shape::convex_polygon(
                    points,
                    flag_combo_color(flags).gamma_multiply(self.flag_viz_opacity),
                    egui::Stroke::NONE,
                ));
                *combos.entry(flags).or_default() += 1;
            }
        }

        let mut legend: Vec<(u64, usize)> = combos.into_iter().collect();
        legend.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        self.flag_viz_legend = legend;
    }
}

/// Draw one sprite anchored on a tile, matching the legacy `copysprite` placement.
///
/// # Arguments
///
/// * `painter` - Painter clipped to the map canvas.
/// * `ctx` - egui context used to upload textures.
/// * `cache` - Graphics zip texture cache.
/// * `view` - Current camera.
/// * `sprite_id` - Sprite to draw.
/// * `tile` - Anchor tile `(x, y)`.
/// * `tint` - Multiplicative tint.
///
/// # Returns
///
/// * `Err` when the sprite exists but couldn't be decoded.
fn paint_sprite_dd(
    painter: &egui::Painter,
    ctx: &egui::Context,
    cache: &mut GraphicsZipCache,
    view: MapView,
    sprite_id: usize,
    tile: (usize, usize),
    tint: egui::Color32,
) -> Result<(), String> {
    let Some(texture) = cache.texture_for(ctx, sprite_id)? else {
        return Ok(());
    };

    let [w, h] = texture.size();
    let (rx, ry) = dd_copysprite_screen_pos(
        tile.0 as i32 * 32,
        tile.1 as i32 * 32,
        w as i32 / 32,
        h as i32 / 32,
    );
    let top_left = view.to_screen(Vec2::new(rx as f32, ry as f32));
    let dst = Rect::from_min_size(top_left, texture.size_vec2() * view.zoom);

    painter.image(
        texture.id(),
        dst,
        Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
        tint,
    );
    Ok(())
}
