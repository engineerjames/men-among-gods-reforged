use super::graphics::GraphicsZipCache;
use eframe::egui;
use egui::{Pos2, Rect, Vec2};
use mag_core::constants::{
    ItemFlags, MAXITEM, MAXTITEM, SERVER_MAPX, SERVER_MAPY, TILEX, USE_EMPTY, XPOS, YPOS,
};
use mag_core::types::{Item, Map};
use std::fs;
use std::path::{Path, PathBuf};

use bincode::{Decode, Encode};

const NORMALIZED_MAGIC: [u8; 4] = *b"MAG2";
const NORMALIZED_VERSION: u32 = 1;

#[derive(Debug, Encode, Decode)]
struct NormalizedDataSet<T> {
    magic: [u8; 4],
    version: u32,
    source_file: String,
    source_record_size: usize,
    records: Vec<T>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PaletteEntryKind {
    Sprite(u16),
    Item(u32),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PaletteEntry {
    kind: PaletteEntryKind,
}

#[derive(Default)]
pub(crate) struct MapViewerApp {
    dat_dir: Option<PathBuf>,
    map_path: Option<PathBuf>,
    map_tiles: Vec<Map>,
    map_error: Option<String>,

    dirty: bool,
    save_status: Option<String>,

    items: Vec<Item>,
    items_error: Option<String>,
    item_templates: Vec<Item>,
    item_templates_error: Option<String>,

    graphics_zip: Option<GraphicsZipCache>,
    graphics_zip_error: Option<String>,

    // Camera pan in screen pixels.
    pan: Vec2,

    // True once we auto-center after loading map/graphics.
    pan_initialized: bool,

    // Cached hover state for the right panel.
    hovered_tile: Option<(usize, usize)>,

    // Frozen selection (click on map when no palette entry is selected).
    selected_tile: Option<(usize, usize)>,

    // Hide mode: clips non-background sprites to show only top half
    hide_enabled: bool,

    // Track if we've done initial load
    initial_load_done: bool,

    // Track frames to delay loading slightly so window appears first
    frame_count: u32,

    // Palette / painting
    palette: Vec<PaletteEntry>,
    selected_palette_index: Option<usize>,
    draft_sprite: u16,
    draft_item_instance_id: u32,
    palette_rect: Option<Rect>,
}

impl MapViewerApp {
    pub(crate) fn new() -> Self {
        let app = Self::default();

        // Don't load map/graphics in constructor - it blocks window creation
        // We'll load on first update instead

        app
    }

    fn ui_tile_preview_row(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &egui::Context,
        sprite: u16,
        fsprite: u16,
        it: u32,
        preview_size: Vec2,
    ) {
        ui.horizontal(|ui| {
            if let Some(cache) = self.graphics_zip.as_mut() {
                let mut try_draw = |ui: &mut egui::Ui, sprite_id: usize| -> bool {
                    if let Ok(Some(texture)) = cache.texture_for(ctx, sprite_id) {
                        ui.add(
                            egui::Image::new(texture)
                                .fit_to_exact_size(preview_size)
                                .maintain_aspect_ratio(true),
                        );
                        true
                    } else {
                        false
                    }
                };

                // Background
                if sprite != 0 {
                    if !try_draw(ui, sprite as usize) {
                        ui.allocate_exact_size(preview_size, egui::Sense::hover());
                    }
                } else {
                    ui.allocate_exact_size(preview_size, egui::Sense::hover());
                }

                // Foreground
                if fsprite != 0 {
                    let sprite_id = if self.hide_enabled {
                        fsprite + 1
                    } else {
                        fsprite
                    };
                    if !try_draw(ui, sprite_id as usize) {
                        ui.allocate_exact_size(preview_size, egui::Sense::hover());
                    }
                } else {
                    ui.allocate_exact_size(preview_size, egui::Sense::hover());
                }

                // Item (instance)
                if it != 0 {
                    let it_idx = it as usize;
                    let item_sprite = if it_idx < self.items.len() {
                        item_map_sprite(self.items[it_idx])
                    } else {
                        None
                    };
                    if let Some(item_sprite) = item_sprite {
                        if !try_draw(ui, item_sprite as usize) {
                            ui.allocate_exact_size(preview_size, egui::Sense::hover());
                        }
                    } else {
                        ui.allocate_exact_size(preview_size, egui::Sense::hover());
                    }
                } else {
                    ui.allocate_exact_size(preview_size, egui::Sense::hover());
                }
            } else {
                ui.allocate_exact_size(preview_size, egui::Sense::hover());
                ui.allocate_exact_size(preview_size, egui::Sense::hover());
                ui.allocate_exact_size(preview_size, egui::Sense::hover());
            }
        });
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
        self.load_map_from_path(dir.join("map.dat"));
    }

    pub(crate) fn load_map_from_path(&mut self, map_path: PathBuf) {
        self.map_error = None;
        self.items_error = None;
        self.item_templates_error = None;
        self.pan_initialized = false;
        self.dirty = false;

        self.dat_dir = map_path.parent().map(|p| p.to_path_buf());
        self.map_path = Some(map_path.clone());

        let display_name = map_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("(unknown)");
        self.save_status = Some(format!("Loaded {}", display_name));

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

        let Some(dir) = self.dat_dir.clone() else {
            self.items.clear();
            self.item_templates.clear();
            return;
        };

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

        // Optional: load item templates for palette item previews.
        let item_templates_path = dir.join("titem.dat");
        if item_templates_path.is_file() {
            match load_item_templates_dat(&item_templates_path) {
                Ok(templates) => {
                    self.item_templates = templates;
                    log::info!("Loaded item templates: {}", self.item_templates.len());
                }
                Err(e) => {
                    self.item_templates.clear();
                    self.item_templates_error = Some(e);
                }
            }
        } else {
            self.item_templates.clear();
        }
    }

    fn default_save_filename(&self) -> &'static str {
        "map_new.dat"
    }

    fn save_map_dialog(&mut self) {
        self.save_status = None;

        let mut dialog = rfd::FileDialog::new().add_filter("DAT", &["dat"]);
        if let Some(dir) = self.dat_dir.as_ref() {
            dialog = dialog.set_directory(dir);
        }
        dialog = dialog.set_file_name(self.default_save_filename());

        let Some(path) = dialog.save_file() else {
            return;
        };

        match self.save_map_to_path(&path) {
            Ok(()) => {
                self.dirty = false;
                self.dat_dir = path.parent().map(|p| p.to_path_buf());
                self.map_path = Some(path.clone());
                self.save_status = Some(format!("Saved: {}", path.display()));
            }
            Err(e) => {
                self.save_status = Some(format!("Save failed: {e}"));
            }
        }
    }

    fn save_map_to_path(&self, path: &PathBuf) -> Result<(), String> {
        if self.map_tiles.is_empty() {
            return Err("No map loaded".to_string());
        }

        save_normalized_records(
            path,
            "map.dat",
            std::mem::size_of::<Map>(),
            self.map_tiles.clone(),
        )
    }

    fn revert_unsaved_changes(&mut self) {
        if let Some(path) = self.map_path.clone() {
            self.load_map_from_path(path);
        } else {
            let Some(dir) = self.dat_dir.clone() else {
                self.save_status = Some("Revert failed: no map loaded".to_string());
                return;
            };
            self.load_map_from_dir(dir);
        }
        self.dirty = false;
        self.save_status = Some("Reverted (discarded unsaved changes)".to_string());
    }

    fn render_palette_overlay(&mut self, ctx: &egui::Context, anchor: Pos2) -> Rect {
        let response = egui::Area::new("map_palette_overlay".into())
            .order(egui::Order::Foreground)
            .fixed_pos(anchor)
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.set_min_width(260.0);
                    ui.vertical(|ui| {
                        ui.strong("Palette");
                        ui.separator();

                        ui.horizontal(|ui| {
                            ui.label("sprite:");
                            ui.add(egui::DragValue::new(&mut self.draft_sprite));

                            let preview_size = Vec2::new(96.0, 96.0);
                            let mut preview_drawn = false;

                            if let Some(cache) = self.graphics_zip.as_mut() {
                                if let Ok(Some(texture)) =
                                    cache.texture_for(ctx, self.draft_sprite as usize)
                                {
                                    ui.add(
                                        egui::Image::new(texture)
                                            .fit_to_exact_size(preview_size)
                                            .maintain_aspect_ratio(true),
                                    );
                                    preview_drawn = true;
                                }
                            }

                            if !preview_drawn {
                                ui.allocate_exact_size(preview_size, egui::Sense::hover());
                            }

                            if ui.small_button("Add").clicked() {
                                if self.draft_sprite != 0 {
                                    self.palette.push(PaletteEntry {
                                        kind: PaletteEntryKind::Sprite(self.draft_sprite),
                                    });
                                }
                            }
                        });

                        ui.horizontal(|ui| {
                            ui.label("it:");
                            ui.add(egui::DragValue::new(&mut self.draft_item_instance_id));

                            let preview_size = Vec2::new(96.0, 96.0);
                            let mut preview_drawn = false;
                            let it_idx = self.draft_item_instance_id as usize;

                            if it_idx < self.item_templates.len()
                                && self.item_templates[it_idx].used != USE_EMPTY
                            {
                                if let Some(sprite) = item_map_sprite(self.item_templates[it_idx]) {
                                    if let Some(cache) = self.graphics_zip.as_mut() {
                                        if let Ok(Some(texture)) =
                                            cache.texture_for(ctx, sprite as usize)
                                        {
                                            ui.add(
                                                egui::Image::new(texture)
                                                    .fit_to_exact_size(preview_size)
                                                    .maintain_aspect_ratio(true),
                                            );
                                            preview_drawn = true;
                                        }
                                    }
                                }
                            }

                            if !preview_drawn {
                                ui.allocate_exact_size(preview_size, egui::Sense::hover());
                            }

                            if ui.small_button("Add").clicked() {
                                if self.draft_item_instance_id != 0 {
                                    self.palette.push(PaletteEntry {
                                        kind: PaletteEntryKind::Item(self.draft_item_instance_id),
                                    });
                                }
                            }
                        });

                        ui.separator();

                        egui::ScrollArea::vertical()
                            .max_height(260.0)
                            .show(ui, |ui| {
                                let icon_size = Vec2::new(48.0, 48.0);
                                egui::Grid::new("palette_image_grid")
                                    .num_columns(4)
                                    .spacing([6.0, 6.0])
                                    .show(ui, |ui| {
                                        let mut col = 0;
                                        for (idx, entry) in self.palette.iter().enumerate() {
                                            let sprite_id: Option<usize> = match entry.kind {
                                                PaletteEntryKind::Sprite(sprite) => {
                                                    if sprite == 0 {
                                                        None
                                                    } else {
                                                        Some(sprite as usize)
                                                    }
                                                }
                                                PaletteEntryKind::Item(it) => {
                                                    if it == 0 {
                                                        None
                                                    } else {
                                                        let it_idx = it as usize;
                                                        if it_idx < self.item_templates.len()
                                                            && self.item_templates[it_idx].used
                                                                != USE_EMPTY
                                                        {
                                                            let item = self.item_templates[it_idx];
                                                            item_map_sprite(item)
                                                                .map(|s| s as usize)
                                                        } else {
                                                            None
                                                        }
                                                    }
                                                }
                                            };

                                            let Some(sprite_id) = sprite_id else {
                                                continue;
                                            };

                                            let Some(cache) = self.graphics_zip.as_mut() else {
                                                break;
                                            };

                                            let Ok(Some(texture)) =
                                                cache.texture_for(ctx, sprite_id)
                                            else {
                                                continue;
                                            };

                                            let selected = self.selected_palette_index == Some(idx);
                                            let tint = if selected {
                                                egui::Color32::from_rgb(180, 255, 180)
                                            } else {
                                                egui::Color32::WHITE
                                            };

                                            let clicked = ui
                                                .add(
                                                    egui::Image::new(texture)
                                                        .fit_to_exact_size(icon_size)
                                                        .maintain_aspect_ratio(true)
                                                        .tint(tint)
                                                        .sense(egui::Sense::click()),
                                                )
                                                .clicked();

                                            if clicked {
                                                if selected {
                                                    self.selected_palette_index = None;
                                                } else {
                                                    self.selected_palette_index = Some(idx);
                                                }
                                            }

                                            col += 1;
                                            if col == 4 {
                                                ui.end_row();
                                                col = 0;
                                            }
                                        }
                                        if col != 0 {
                                            ui.end_row();
                                        }
                                    });
                            });
                    });
                });
            });

        response.response.rect
    }
}

fn load_normalized_records<T: Decode<()>>(
    path: &Path,
    expected_record_count: usize,
) -> Result<Vec<T>, String> {
    let data = fs::read(path).map_err(|e| format!("Failed to read {:?}: {e}", path))?;
    let (payload, consumed): (NormalizedDataSet<T>, usize) =
        bincode::decode_from_slice(&data, bincode::config::standard())
            .map_err(|e| format!("Failed to decode {:?}: {e}", path))?;

    if payload.magic != NORMALIZED_MAGIC {
        return Err(format!(
            "Invalid normalized magic in {:?}: {:?}",
            path, payload.magic
        ));
    }

    if payload.version != NORMALIZED_VERSION {
        return Err(format!(
            "Unsupported normalized version in {:?}: {}",
            path, payload.version
        ));
    }

    if payload.records.len() != expected_record_count {
        return Err(format!(
            "Record count mismatch in {:?}: expected {}, got {}",
            path,
            expected_record_count,
            payload.records.len()
        ));
    }

    if consumed != data.len() {
        log::warn!(
            "Trailing bytes in normalized dataset {:?}: {}",
            path,
            data.len() - consumed
        );
    }

    Ok(payload.records)
}

fn save_normalized_records<T: Encode>(
    path: &Path,
    source_file: &str,
    source_record_size: usize,
    records: Vec<T>,
) -> Result<(), String> {
    let payload = NormalizedDataSet {
        magic: NORMALIZED_MAGIC,
        version: NORMALIZED_VERSION,
        source_file: source_file.to_string(),
        source_record_size,
        records,
    };

    let bytes = bincode::encode_to_vec(payload, bincode::config::standard())
        .map_err(|e| format!("Failed to encode {:?}: {e}", path))?;

    fs::write(path, bytes).map_err(|e| format!("Failed to write {:?}: {e}", path))
}

fn load_item_dat(path: &PathBuf) -> Result<Vec<Item>, String> {
    load_normalized_records(path, MAXITEM)
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
    let expected_tiles = (SERVER_MAPX as usize) * (SERVER_MAPY as usize);
    load_normalized_records(path, expected_tiles)
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

        // Save shortcut (Cmd+S on macOS, Ctrl+S elsewhere).
        let save_shortcut = ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::S));
        if save_shortcut {
            self.save_map_dialog();
        }

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
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Open dat dir...").clicked() {
                        ui.close_menu();
                        if let Some(dir) = rfd::FileDialog::new().pick_folder() {
                            self.load_map_from_dir(dir);
                        }
                    }

                    if ui.button("Open map file...").clicked() {
                        ui.close_menu();
                        let mut dialog = rfd::FileDialog::new().add_filter("DAT", &["dat"]);
                        if let Some(dir) = self.dat_dir.as_ref() {
                            dialog = dialog.set_directory(dir);
                        }
                        if let Some(path) = dialog.pick_file() {
                            self.load_map_from_path(path);
                        }
                    }

                    if ui.button("Open graphics zip...").clicked() {
                        ui.close_menu();
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter("zip", &["zip", "ZIP"])
                            .pick_file()
                        {
                            self.load_graphics_zip(path);
                        }
                    }

                    ui.separator();

                    let save_enabled = !self.map_tiles.is_empty() && self.dirty;
                    if ui
                        .add_enabled(save_enabled, egui::Button::new("Save..."))
                        .clicked()
                    {
                        ui.close_menu();
                        self.save_map_dialog();
                    }

                    let revert_enabled = self.dirty && self.dat_dir.is_some();
                    if ui
                        .add_enabled(
                            revert_enabled,
                            egui::Button::new("Revert (discard changes)"),
                        )
                        .clicked()
                    {
                        ui.close_menu();
                        self.revert_unsaved_changes();
                    }
                });

                ui.separator();

                if ui.button("Reset view").clicked() {
                    self.pan = Vec2::ZERO;
                    self.pan_initialized = false;
                }

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

                if self.dirty {
                    ui.separator();
                    ui.colored_label(egui::Color32::YELLOW, "Unsaved changes");
                }

                if let Some(status) = self.save_status.as_ref() {
                    ui.separator();
                    let color = if status.starts_with("Save failed") {
                        egui::Color32::LIGHT_RED
                    } else {
                        egui::Color32::LIGHT_GREEN
                    };
                    ui.colored_label(color, status);
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
                if let Some(err) = &self.item_templates_error {
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
                {
                    let (hx, hy, hover_tile) = if let Some((x, y)) = self.hovered_tile {
                        if !self.map_tiles.is_empty() {
                            let idx = tile_index(x, y);
                            if idx < self.map_tiles.len() {
                                (Some(x), Some(y), Some(self.map_tiles[idx]))
                            } else {
                                (Some(x), Some(y), None)
                            }
                        } else {
                            (Some(x), Some(y), None)
                        }
                    } else {
                        (None, None, None)
                    };

                    if let (Some(x), Some(y)) = (hx, hy) {
                        ui.label(format!("Hover tile: ({}, {})", x, y));
                    } else {
                        ui.label("Hover tile: (N/A)");
                    }

                    let preview_size = Vec2::new(64.0, 64.0);
                    if let Some(tile) = hover_tile {
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
                        self.ui_tile_preview_row(ui, ctx, sprite, fsprite, it, preview_size);

                        if it != 0 {
                            let it_idx = it as usize;
                            if it_idx < self.items.len() {
                                let item = self.items[it_idx];
                                let sprite = item_map_sprite(item).unwrap_or(0);
                                ui.label(format!("item sprite: {}", sprite));
                            } else {
                                ui.label("item sprite: (item.dat not loaded)");
                            }
                        } else {
                            ui.label("item sprite: N/A");
                        }
                    } else {
                        ui.label("sprite: N/A");
                        ui.label("fsprite: N/A");
                        ui.label("flags: N/A");
                        ui.label("light: N/A");
                        ui.label("ch: N/A to_ch: N/A it: N/A");
                        self.ui_tile_preview_row(ui, ctx, 0, 0, 0, preview_size);
                        ui.label("item sprite: N/A");
                    }
                }

                ui.separator();
                if let Some((x, y)) = self.selected_tile {
                    ui.label(format!("Selected tile: ({}, {})", x, y));
                    if !self.map_tiles.is_empty() {
                        let idx = tile_index(x, y);
                        if idx < self.map_tiles.len() {
                            let tile = self.map_tiles[idx];
                            let sprite = tile.sprite;
                            let fsprite = tile.fsprite;
                            let mut flags = tile.flags;
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

                            // Visual preview of the selected tile's sprites.
                            let preview_size = Vec2::new(64.0, 64.0);
                            self.ui_tile_preview_row(ui, ctx, sprite, fsprite, it, preview_size);

                            if sprite != 0 && fsprite != 0 {
                                if ui.button("Clear fsprite").clicked() {
                                    let mut updated = self.map_tiles[idx];
                                    updated.fsprite = 0;
                                    if updated != self.map_tiles[idx] {
                                        self.map_tiles[idx] = updated;
                                        self.dirty = true;
                                        ctx.request_repaint();
                                    }
                                }
                            }

                            if it != 0 {
                                let it_idx = it as usize;
                                if it_idx < self.items.len() {
                                    let item = self.items[it_idx];
                                    let sprite = item_map_sprite(item).unwrap_or(0);
                                    ui.label(format!("item sprite: {}", sprite));
                                } else {
                                    ui.label("item sprite: (item.dat not loaded)");
                                }
                            } else {
                                ui.label("item sprite: N/A");
                            }

                            ui.separator();
                            ui.label("Map flags:");
                            let original_flags = flags;

                            // Keep this list aligned with `core/src/constants.rs` map flags.
                            let defs: &[(u64, &str)] = &[
                                (mag_core::constants::MF_MOVEBLOCK as u64, "MF_MOVEBLOCK"),
                                (mag_core::constants::MF_SIGHTBLOCK as u64, "MF_SIGHTBLOCK"),
                                (mag_core::constants::MF_INDOORS as u64, "MF_INDOORS"),
                                (mag_core::constants::MF_UWATER as u64, "MF_UWATER"),
                                (mag_core::constants::MF_NOLAG as u64, "MF_NOLAG"),
                                (mag_core::constants::MF_NOMONST as u64, "MF_NOMONST"),
                                (mag_core::constants::MF_BANK as u64, "MF_BANK"),
                                (mag_core::constants::MF_TAVERN as u64, "MF_TAVERN"),
                                (mag_core::constants::MF_NOMAGIC as u64, "MF_NOMAGIC"),
                                (mag_core::constants::MF_DEATHTRAP as u64, "MF_DEATHTRAP"),
                                (mag_core::constants::MF_ARENA as u64, "MF_ARENA"),
                                (mag_core::constants::MF_NOEXPIRE as u64, "MF_NOEXPIRE"),
                                (mag_core::constants::MF_NOFIGHT, "MF_NOFIGHT"),
                                (mag_core::constants::MF_GFX_INJURED, "MF_GFX_INJURED"),
                                (mag_core::constants::MF_GFX_INJURED1, "MF_GFX_INJURED1"),
                                (mag_core::constants::MF_GFX_INJURED2, "MF_GFX_INJURED2"),
                                (mag_core::constants::MF_GFX_TOMB, "MF_GFX_TOMB"),
                                (mag_core::constants::MF_GFX_TOMB1, "MF_GFX_TOMB1"),
                                (mag_core::constants::MF_GFX_DEATH, "MF_GFX_DEATH"),
                                (mag_core::constants::MF_GFX_DEATH1, "MF_GFX_DEATH1"),
                                (mag_core::constants::MF_GFX_EMAGIC, "MF_GFX_EMAGIC"),
                                (mag_core::constants::MF_GFX_EMAGIC1, "MF_GFX_EMAGIC1"),
                                (mag_core::constants::MF_GFX_GMAGIC, "MF_GFX_GMAGIC"),
                                (mag_core::constants::MF_GFX_GMAGIC1, "MF_GFX_GMAGIC1"),
                                (mag_core::constants::MF_GFX_CMAGIC, "MF_GFX_CMAGIC"),
                                (mag_core::constants::MF_GFX_CMAGIC1, "MF_GFX_CMAGIC1"),
                            ];

                            egui::ScrollArea::vertical()
                                .max_height(220.0)
                                .show(ui, |ui| {
                                    egui::Grid::new("selected_tile_map_flags")
                                        .num_columns(2)
                                        .spacing([10.0, 4.0])
                                        .show(ui, |ui| {
                                            for (i, (mask, name)) in defs.iter().enumerate() {
                                                let mut on = (flags & *mask) != 0;
                                                if ui.checkbox(&mut on, *name).changed() {
                                                    if on {
                                                        flags |= *mask;
                                                    } else {
                                                        flags &= !*mask;
                                                    }
                                                }
                                                if i % 2 == 1 {
                                                    ui.end_row();
                                                }
                                            }
                                            if defs.len() % 2 == 1 {
                                                ui.end_row();
                                            }
                                        });
                                });

                            if flags != original_flags {
                                let mut updated = self.map_tiles[idx];
                                updated.flags = flags;
                                if updated != self.map_tiles[idx] {
                                    self.map_tiles[idx] = updated;
                                    self.dirty = true;
                                    ctx.request_repaint();
                                }
                            }
                        }
                    }
                } else {
                    ui.label("Selected tile: (none)");
                }
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            let (rect, response) =
                ui.allocate_exact_size(ui.available_size(), egui::Sense::click_and_drag());

            // Overlay palette anchored in the map canvas.
            let palette_rect =
                self.render_palette_overlay(ctx, rect.left_top() + Vec2::new(12.0, 12.0));
            self.palette_rect = Some(palette_rect);

            if response.dragged() {
                self.pan += response.drag_delta();
                ctx.request_repaint();
            }

            if response.clicked_by(egui::PointerButton::Primary) {
                let pointer_pos = ctx.pointer_latest_pos();
                let clicked_palette = pointer_pos.is_some_and(|p| palette_rect.contains(p));

                if !clicked_palette {
                    let Some(sel_idx) = self.selected_palette_index else {
                        // No palette selection => select the tile (freeze details).
                        if let Some((x, y)) = self.hovered_tile {
                            self.selected_tile = Some((x, y));
                            ctx.request_repaint();
                        }
                        return;
                    };
                    if sel_idx >= self.palette.len() {
                        self.selected_palette_index = None;
                        return;
                    }
                    let Some((x, y)) = self.hovered_tile else {
                        return;
                    };
                    let idx = tile_index(x, y);
                    if idx >= self.map_tiles.len() {
                        return;
                    }

                    let mut tile = self.map_tiles[idx];
                    match self.palette[sel_idx].kind {
                        PaletteEntryKind::Sprite(sprite) => {
                            if sprite != 0 {
                                tile.fsprite = sprite;
                            }
                        }
                        PaletteEntryKind::Item(it) => {
                            if it != 0 {
                                tile.it = it;
                                tile.fsprite = 0;
                            }
                        }
                    }

                    if tile != self.map_tiles[idx] {
                        self.map_tiles[idx] = tile;
                        self.dirty = true;
                        ctx.request_repaint();
                    }
                }
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
                            let item_sprite = item_map_sprite(item);
                            if let Some(item_sprite) = item_sprite {
                                // Highlight items red when hovering over them
                                let is_hovered = self.hovered_tile == Some((x, y));
                                let is_selected = self.selected_tile == Some((x, y));
                                let tint = if is_hovered || is_selected {
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

            // Highlight selected tile (persistent).
            if let Some((x, y)) = self.selected_tile {
                let xpos = (x as i32) * 32;
                let ypos = (y as i32) * 32;
                let (tx, ty) = dd_tile_origin_screen_pos(xpos, ypos);
                let pos = rect.min + self.pan + Vec2::new(tx as f32, ty as f32);
                painter.circle_stroke(pos, 7.0, (3.0, egui::Color32::from_rgb(255, 50, 50)));
            }
        });
    }
}

fn load_item_templates_dat(path: &PathBuf) -> Result<Vec<Item>, String> {
    load_normalized_records(path, MAXTITEM)
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
