use super::graphics::GraphicsZipCache;
use eframe::egui;
use egui::{Pos2, Rect, Vec2};
use mag_core::constants::{ItemFlags, MAXITEM, SERVER_MAPX, SERVER_MAPY, TILEX, XPOS, YPOS};
use mag_core::types::{Item, Map};
use std::fs;
use std::path::PathBuf;

#[derive(Default)]
pub(crate) struct MapViewerApp {
    dat_dir: Option<PathBuf>,
    map_tiles: Vec<Map>,
    map_error: Option<String>,

    items: Vec<Item>,
    items_error: Option<String>,

    graphics_zip: Option<GraphicsZipCache>,
    graphics_zip_error: Option<String>,

    // Camera pan in screen pixels.
    pan: Vec2,

    // True once we auto-center after loading map/graphics.
    pan_initialized: bool,

    // Cached hover state for the right panel.
    hovered_tile: Option<(usize, usize)>,

    // Hide mode: clips non-background sprites to show only top half
    hide_enabled: bool,

    // Track if we've done initial load
    initial_load_done: bool,

    // Track frames to delay loading slightly so window appears first
    frame_count: u32,
}

impl MapViewerApp {
    pub(crate) fn new() -> Self {
        let app = Self::default();

        // Don't load map/graphics in constructor - it blocks window creation
        // We'll load on first update instead

        app
    }

    pub(crate) fn load_graphics_zip(&mut self, zip_path: PathBuf) {
        self.graphics_zip_error = None;
        match GraphicsZipCache::load(zip_path) {
            Ok(cache) => {
                self.graphics_zip = Some(cache);
            }
            Err(e) => {
                self.graphics_zip = None;
                self.graphics_zip_error = Some(e);
            }
        }
    }

    pub(crate) fn load_map_from_dir(&mut self, dir: PathBuf) {
        self.dat_dir = Some(dir.clone());
        self.map_error = None;
        self.items_error = None;
        self.pan_initialized = false;

        let map_path = dir.join("map.dat");
        match load_map_dat(&map_path) {
            Ok(tiles) => {
                self.map_tiles = tiles;
                log::info!("Loaded map tiles: {}", self.map_tiles.len());
            }
            Err(e) => {
                self.map_tiles.clear();
                self.map_error = Some(e);
            }
        }

        // Optional: load item instances so we can render `Map.it` overlays.
        let item_path = dir.join("item.dat");
        if item_path.is_file() {
            match load_item_dat(&item_path) {
                Ok(items) => {
                    self.items = items;
                    log::info!("Loaded items: {}", self.items.len());
                }
                Err(e) => {
                    self.items.clear();
                    self.items_error = Some(e);
                }
            }
        } else {
            self.items.clear();
        }
    }
}

fn load_item_dat(path: &PathBuf) -> Result<Vec<Item>, String> {
    let data = fs::read(path).map_err(|e| format!("Failed to read {:?}: {e}", path))?;

    let item_size = std::mem::size_of::<Item>();
    let expected_bytes = MAXITEM * item_size;

    if data.len() != expected_bytes {
        return Err(format!(
            "item.dat size mismatch: expected {} bytes ({} items), got {}",
            expected_bytes,
            MAXITEM,
            data.len()
        ));
    }

    let mut items = Vec::with_capacity(MAXITEM);
    for i in 0..MAXITEM {
        let offset = i * item_size;
        let end = offset + item_size;
        let Some(item) = Item::from_bytes(&data[offset..end]) else {
            return Err(format!("Failed to parse item at index {i}"));
        };
        items.push(item);
    }

    Ok(items)
}

#[inline]
fn item_map_sprite(item: Item) -> Option<i16> {
    // Mirror server logic used to populate client map tiles.
    let hidden = (item.flags & ItemFlags::IF_HIDDEN.bits()) != 0;
    if hidden {
        return None;
    }

    let sprite = if item.active != 0 {
        item.sprite[1]
    } else {
        item.sprite[0]
    };

    if sprite > 0 {
        Some(sprite)
    } else {
        None
    }
}

fn load_map_dat(path: &PathBuf) -> Result<Vec<Map>, String> {
    let data = fs::read(path).map_err(|e| format!("Failed to read {:?}: {e}", path))?;

    let expected_tiles = (SERVER_MAPX as usize) * (SERVER_MAPY as usize);
    let tile_size = std::mem::size_of::<Map>();
    let expected_bytes = expected_tiles * tile_size;

    if data.len() < expected_bytes {
        return Err(format!(
            "map.dat too small: expected {} bytes ({} tiles), got {}",
            expected_bytes,
            expected_tiles,
            data.len()
        ));
    }

    if data.len() != expected_bytes {
        log::warn!(
            "map.dat size mismatch: expected {} bytes, got {} (will parse first {} tiles)",
            expected_bytes,
            data.len(),
            expected_tiles
        );
    }

    let mut tiles = Vec::with_capacity(expected_tiles);
    for i in 0..expected_tiles {
        let offset = i * tile_size;
        let end = offset + tile_size;
        let Some(tile) = Map::from_bytes(&data[offset..end]) else {
            return Err(format!("Failed to parse map tile at index {i}"));
        };
        tiles.push(tile);
    }

    Ok(tiles)
}

#[inline]
fn tile_index(x: usize, y: usize) -> usize {
    y * (SERVER_MAPX as usize) + x
}

#[inline]
fn dd_tile_origin_screen_pos(xpos: i32, ypos: i32) -> (i32, i32) {
    // Ported from client gameplay `legacy_engine::copysprite_screen_pos` (dd.c copysprite).
    // Returns the tile origin BEFORE sprite-size offsets.
    // NOTE: we ignore the negative-coordinate odd-bit adjustments because xpos/ypos are >= 0.
    let rx = (xpos / 2) + (ypos / 2) + 32 + XPOS - (((TILEX as i32 - 34) / 2) * 32);
    let ry = (xpos / 4) - (ypos / 4) + YPOS;
    (rx, ry)
}

#[inline]
fn dd_copysprite_screen_pos(
    xpos: i32,
    ypos: i32,
    xoff: i32,
    yoff: i32,
    xs: i32,
    ys: i32,
) -> (i32, i32) {
    // Ported from client gameplay `legacy_engine::copysprite_screen_pos` (dd.c copysprite).
    let (mut rx, mut ry) = dd_tile_origin_screen_pos(xpos, ypos);
    rx -= xs * 16;
    ry -= ys * 32;
    rx += xoff;
    ry += yoff;
    (rx, ry)
}

fn clamp_range(min: i32, max: i32, lo: i32, hi: i32) -> (usize, usize) {
    let min = min.clamp(lo, hi);
    let max = max.clamp(lo, hi);
    (min as usize, max as usize)
}

impl eframe::App for MapViewerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.frame_count += 1;

        // Load map/graphics after a couple frames (window has appeared)
        if !self.initial_load_done && self.frame_count > 2 {
            self.initial_load_done = true;
            if let Some(dir) = crate::dat_dir_from_args().or_else(crate::default_dat_dir) {
                self.load_map_from_dir(dir);
            }
            if let Some(zip_path) =
                crate::graphics_zip_from_args().or_else(crate::default_graphics_zip_path)
            {
                self.load_graphics_zip(zip_path);
            }
        }

        // Preload graphics incrementally
        if let Some(cache) = self.graphics_zip.as_mut() {
            if !cache.loading_done {
                let _ = cache.preload_step(ctx);
                ctx.request_repaint();
            }
        }

        // Keyboard pan (WASD).
        let dt = ctx.input(|i| i.stable_dt).max(1.0 / 240.0);
        let speed = 750.0; // px/sec
        let mut delta = Vec2::ZERO;
        ctx.input(|i| {
            if i.key_down(egui::Key::W) {
                delta.y += 1.0;
            }
            if i.key_down(egui::Key::S) {
                delta.y -= 1.0;
            }
            if i.key_down(egui::Key::A) {
                delta.x += 1.0;
            }
            if i.key_down(egui::Key::D) {
                delta.x -= 1.0;
            }
        });
        if delta != Vec2::ZERO {
            self.pan += delta.normalized() * speed * dt;
            ctx.request_repaint();
        }

        egui::TopBottomPanel::top("top_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.button("Open dat dir...").clicked() {
                    if let Some(dir) = rfd::FileDialog::new().pick_folder() {
                        self.load_map_from_dir(dir);
                    }
                }

                if ui.button("Open graphics zip...").clicked() {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("zip", &["zip", "ZIP"])
                        .pick_file()
                    {
                        self.load_graphics_zip(path);
                    }
                }

                if ui.button("Reset view").clicked() {
                    self.pan = Vec2::ZERO;
                    self.pan_initialized = false;
                }

                ui.separator();

                if ui
                    .button(if self.hide_enabled {
                        "Hide: ON"
                    } else {
                        "Hide: OFF"
                    })
                    .clicked()
                {
                    self.hide_enabled = !self.hide_enabled;
                    ctx.request_repaint();
                }

                if let Some(dir) = &self.dat_dir {
                    ui.separator();
                    ui.label(format!("dat: {}", dir.display()));
                }
            });
        });

        egui::SidePanel::right("side_panel")
            .default_width(340.0)
            .show(ctx, |ui| {
                ui.heading("Map Viewer");

                if let Some(err) = &self.map_error {
                    ui.separator();
                    ui.colored_label(egui::Color32::LIGHT_RED, err);
                }
                if let Some(err) = &self.graphics_zip_error {
                    ui.separator();
                    ui.colored_label(egui::Color32::LIGHT_RED, err);
                }
                if let Some(err) = &self.items_error {
                    ui.separator();
                    ui.colored_label(egui::Color32::LIGHT_RED, err);
                }

                ui.separator();
                ui.label(format!("Map size: {} x {}", SERVER_MAPX, SERVER_MAPY));
                ui.label(format!("Loaded tiles: {}", self.map_tiles.len()));

                // Show loading progress
                if let Some(cache) = &self.graphics_zip {
                    if !cache.loading_done {
                        let (loaded, total) = cache.loading_progress();
                        ui.separator();
                        ui.label(format!("Loading sprites: {}/{}", loaded, total));
                        if total > 0 {
                            let progress = loaded as f32 / total as f32;
                            ui.add(
                                egui::ProgressBar::new(progress)
                                    .text(format!("{:.0}%", progress * 100.0)),
                            );
                        }
                    }
                }

                ui.separator();
                ui.label("Controls:");
                ui.label("- WASD: pan");
                ui.label("- Drag: pan");

                ui.separator();
                ui.label(format!("Pan: [{:.1}, {:.1}]", self.pan.x, self.pan.y));

                ui.separator();
                if let Some((x, y)) = self.hovered_tile {
                    ui.label(format!("Hover tile: ({}, {})", x, y));
                    if !self.map_tiles.is_empty() {
                        let idx = tile_index(x, y);
                        if idx < self.map_tiles.len() {
                            // `Map` is `#[repr(packed)]`, so don't pass field references into
                            // formatting macros (they may create unaligned references).
                            let tile = self.map_tiles[idx];
                            let sprite = tile.sprite;
                            let fsprite = tile.fsprite;
                            let flags = tile.flags;
                            let light = tile.light;
                            let dlight = tile.dlight;
                            let ch = tile.ch;
                            let to_ch = tile.to_ch;
                            let it = tile.it;

                            ui.label(format!("sprite: {}", sprite));
                            ui.label(format!("fsprite: {}", fsprite));
                            ui.label(format!("flags: 0x{:016X}", flags));
                            ui.label(format!("light: {} (dlight {})", light, dlight));
                            ui.label(format!("ch: {} to_ch: {} it: {}", ch, to_ch, it));

                            if it != 0 {
                                let it_idx = it as usize;
                                if it_idx < self.items.len() {
                                    let item = self.items[it_idx];
                                    let sprite = item_map_sprite(item).unwrap_or(0);
                                    ui.label(format!("item sprite: {}", sprite));
                                } else {
                                    ui.label("item sprite: (item.dat not loaded)");
                                }
                            }
                        }
                    }
                } else {
                    ui.label("Hover tile: (none)");
                }
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            let (rect, response) = ui.allocate_exact_size(ui.available_size(), egui::Sense::drag());

            if response.dragged() {
                self.pan += response.drag_delta();
                ctx.request_repaint();
            }

            // Auto-center on first paint after load.
            if !self.pan_initialized && !self.map_tiles.is_empty() {
                let mid_x = (SERVER_MAPX as usize) / 2;
                let mid_y = (SERVER_MAPY as usize) / 2;
                let xpos = (mid_x as i32) * 32;
                let ypos = (mid_y as i32) * 32;
                let (tx, ty) = dd_tile_origin_screen_pos(xpos, ypos);
                self.pan = (rect.center() - rect.min) - Vec2::new(tx as f32, ty as f32);
                self.pan_initialized = true;
            }

            // Compute hovered tile from mouse position (invert tile-origin mapping).
            self.hovered_tile = ctx.pointer_latest_pos().and_then(|pos| {
                if !rect.contains(pos) {
                    return None;
                }

                // Convert to map coordinate space
                let screen_pos = pos - rect.min - self.pan;

                // Invert dd_tile_origin_screen_pos:
                // rx = (xpos / 2) + (ypos / 2) + 32 + XPOS - (((TILEX as i32 - 34) / 2) * 32)
                // ry = (xpos / 4) - (ypos / 4) + YPOS
                //
                // Solving for xpos, ypos:
                // Let rx' = rx - offset_x, ry' = ry - offset_y
                // rx' = xpos/2 + ypos/2
                // ry' = xpos/4 - ypos/4
                // => xpos/2 = rx' - ypos/2
                // => xpos/4 = ry' + ypos/4
                // => 2*ry' + ypos/2 = rx' - ypos/2
                // => ypos = rx' - 2*ry'
                // => xpos = 2*rx' - ypos = 2*rx' - (rx' - 2*ry') = rx' + 2*ry'

                let offset_x = 32 + XPOS - (((TILEX as i32 - 34) / 2) * 32);
                let offset_y = YPOS;
                let rx_prime = screen_pos.x - offset_x as f32;
                let ry_prime = screen_pos.y - offset_y as f32;

                let xpos = rx_prime + 2.0 * ry_prime;
                let ypos = rx_prime - 2.0 * ry_prime;

                let x = (xpos / 32.0).floor() as i32;
                let y = (ypos / 32.0).floor() as i32;
                if x < 0 || y < 0 {
                    return None;
                }
                let (x, y) = (x as usize, y as usize);
                if x >= SERVER_MAPX as usize || y >= SERVER_MAPY as usize {
                    return None;
                }
                Some((x, y))
            });

            let painter = ui.painter_at(rect);
            painter.rect_filled(rect, 0.0, egui::Color32::from_rgb(20, 22, 26));

            if self.map_tiles.is_empty() {
                painter.text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "No map loaded (Open dat dir...) ",
                    egui::TextStyle::Heading.resolve(ui.style()),
                    egui::Color32::GRAY,
                );
                return;
            }

            // Visible range estimation: compute tile coord bounds for the canvas corners.
            let corners = [
                rect.left_top(),
                rect.right_top(),
                rect.left_bottom(),
                rect.right_bottom(),
            ];
            let mut min_x = f32::INFINITY;
            let mut max_x = f32::NEG_INFINITY;
            let mut min_y = f32::INFINITY;
            let mut max_y = f32::NEG_INFINITY;
            for c in corners {
                let local = c - rect.min - self.pan;
                let base_x = local.x - (32 + XPOS - (((TILEX as i32 - 34) / 2) * 32)) as f32;
                let base_y = local.y - (YPOS as f32);
                let xf = 0.5 * (base_x / 16.0 + base_y / 8.0);
                let yf = 0.5 * (base_x / 16.0 - base_y / 8.0);
                min_x = min_x.min(xf);
                max_x = max_x.max(xf);
                min_y = min_y.min(yf);
                max_y = max_y.max(yf);
            }

            // Expand to be safe (sprites extend beyond the anchor).
            let margin = 6;
            let (x0, x1) = clamp_range(
                min_x.floor() as i32 - margin,
                max_x.ceil() as i32 + margin,
                0,
                SERVER_MAPX - 1,
            );
            let (y0, y1) = clamp_range(
                min_y.floor() as i32 - margin,
                max_y.ceil() as i32 + margin,
                0,
                SERVER_MAPY - 1,
            );

            let Some(cache) = self.graphics_zip.as_mut() else {
                painter.text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "No graphics zip loaded (Open graphics zip...) ",
                    egui::TextStyle::Heading.resolve(ui.style()),
                    egui::Color32::GRAY,
                );
                return;
            };

            // Draw order: match the legacy scan order used by gameplay's pass-2 painter ordering.
            // Equivalent shape to `tile_draw_order = (TILEY-1-y)*TILEX + x` but for SERVER_MAP dims.
            // Larger `y` is higher on screen (ry ~= 8*x - 8*y), so it must be drawn first.
            let w = SERVER_MAPX as usize;
            let h = SERVER_MAPY as usize;

            for y in (y0..=y1).rev() {
                if y >= h {
                    continue;
                }
                for x in x0..=x1 {
                    if x >= w {
                        continue;
                    }

                    let idx = tile_index(x, y);
                    if idx >= self.map_tiles.len() {
                        continue;
                    }

                    let tile = self.map_tiles[idx];
                    let xpos = (x as i32) * 32;
                    let ypos = (y as i32) * 32;

                    // Background
                    if tile.sprite != 0 {
                        if let Err(e) = paint_sprite_dd(
                            &painter,
                            ctx,
                            cache,
                            tile.sprite as usize,
                            rect,
                            self.pan,
                            xpos,
                            ypos,
                            0,
                            0,
                            egui::Color32::WHITE,
                        ) {
                            self.graphics_zip_error = Some(e);
                        }
                    }

                    // Foreground
                    if tile.fsprite != 0 {
                        // Match client hide logic: substitute sprite_id + 1 when hide is enabled
                        let sprite_id = if self.hide_enabled {
                            tile.fsprite + 1
                        } else {
                            tile.fsprite
                        };
                        if let Err(e) = paint_sprite_dd(
                            &painter,
                            ctx,
                            cache,
                            sprite_id as usize,
                            rect,
                            self.pan,
                            xpos,
                            ypos,
                            0,
                            0,
                            egui::Color32::WHITE,
                        ) {
                            self.graphics_zip_error = Some(e);
                        }
                    } else if tile.it != 0 {
                        // Item overlay (Map.it is an item instance id).
                        let it_idx = tile.it as usize;
                        if it_idx < self.items.len() {
                            let item = self.items[it_idx];
                            if let Some(item_sprite) = item_map_sprite(item) {
                                // Highlight items red when hovering over them
                                let is_hovered = self.hovered_tile == Some((x, y));
                                let tint = if is_hovered {
                                    egui::Color32::from_rgb(255, 50, 50)
                                } else {
                                    egui::Color32::WHITE
                                };
                                if let Err(e) = paint_sprite_dd(
                                    &painter,
                                    ctx,
                                    cache,
                                    item_sprite as usize,
                                    rect,
                                    self.pan,
                                    xpos,
                                    ypos,
                                    0,
                                    0,
                                    tint,
                                ) {
                                    self.graphics_zip_error = Some(e);
                                }
                            }
                        }
                    }
                }
            }

            // Highlight hovered tile.
            if let Some((x, y)) = self.hovered_tile {
                let xpos = (x as i32) * 32;
                let ypos = (y as i32) * 32;
                let (tx, ty) = dd_tile_origin_screen_pos(xpos, ypos);
                let pos = rect.min + self.pan + Vec2::new(tx as f32, ty as f32);
                painter.circle_stroke(pos, 6.0, (2.0, egui::Color32::YELLOW));
            }
        });
    }
}

fn paint_sprite_dd(
    painter: &egui::Painter,
    ctx: &egui::Context,
    cache: &mut GraphicsZipCache,
    sprite_id: usize,
    rect: Rect,
    pan: Vec2,
    xpos: i32,
    ypos: i32,
    xoff: i32,
    yoff: i32,
    tint: egui::Color32,
) -> Result<(), String> {
    let Some((xs, ys)) = cache.sprite_tiles_xy(ctx, sprite_id)? else {
        return Ok(());
    };

    let Some(texture) = cache.texture_for(ctx, sprite_id)? else {
        return Ok(());
    };

    let (rx, ry) = dd_copysprite_screen_pos(xpos, ypos, xoff, yoff, xs, ys);
    let top_left = rect.min + pan + Vec2::new(rx as f32, ry as f32);
    let dst = Rect::from_min_size(top_left, texture.size_vec2());

    painter.image(
        texture.id(),
        dst,
        Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(1.0, 1.0)),
        tint,
    );

    Ok(())
}
