//! Top menu bar and right-hand inspector panel.

use super::editing::item_map_sprite;
use super::flags::{
    GAMEPLAY_MAP_FLAG_MASK, MAX_FLAG_VIZ_LEGEND_ENTRIES, flag_checkbox, flag_combo_color,
    flag_names, map_flag_defs,
};
use super::geometry::tile_index;
use super::{LoadPurpose, MapViewerApp};
use eframe::egui;
use egui::Vec2;
use mag_core::constants::{SERVER_MAPX, SERVER_MAPY};
use mag_core::types::Map;
use server_utils::DataSource;

/// Size of each sprite preview in the inspector.
const TILE_PREVIEW_SIZE: Vec2 = Vec2::new(64.0, 64.0);

/// Show a tile's raw fields as labels.
///
/// # Arguments
///
/// * `ui` - Target UI.
/// * `tile` - Tile to describe, or `None` to show `N/A` placeholders.
fn ui_tile_fields(ui: &mut egui::Ui, tile: Option<&Map>) {
    let Some(tile) = tile else {
        for label in ["sprite", "fsprite", "flags", "light"] {
            ui.label(format!("{label}: N/A"));
        }
        ui.label("ch: N/A to_ch: N/A it: N/A");
        return;
    };
    ui.label(format!("sprite: {}", tile.sprite));
    ui.label(format!("fsprite: {}", tile.fsprite));
    ui.label(format!("flags: 0x{:016X}", tile.flags));
    ui.label(format!("light: {} (dlight {})", tile.light, tile.dlight));
    ui.label(format!(
        "ch: {} to_ch: {} it: {}",
        tile.ch, tile.to_ch, tile.it
    ));
}

impl MapViewerApp {
    /// Top menu bar: File/Settings menus, view toggles, status, and API actions.
    pub(super) fn ui_top_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("top_bar").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| self.ui_file_menu(ui));
                ui.menu_button("Settings", |ui| self.ui_settings_menu(ui, ctx));

                ui.separator();

                if ui.button("Reset view").clicked() {
                    self.pan = Vec2::ZERO;
                    self.pan_initialized = false;
                }

                let hide_label = if self.hide_enabled {
                    "Hide: ON"
                } else {
                    "Hide: OFF"
                };
                if ui.button(hide_label).clicked() {
                    self.hide_enabled = !self.hide_enabled;
                    ctx.request_repaint();
                }

                if ui
                    .add_enabled(
                        !self.undo_stack.is_empty(),
                        egui::Button::new(format!("Undo ({})", self.undo_stack.len())),
                    )
                    .on_hover_text("Ctrl+Z / Cmd+Z")
                    .clicked()
                {
                    self.undo();
                }

                if self.dirty {
                    ui.separator();
                    ui.colored_label(
                        egui::Color32::YELLOW,
                        format!(
                            "Unsaved: {} tile(s), {} item action(s)",
                            self.dirty_tiles.len(),
                            self.pending_item_actions.len()
                        ),
                    );
                }

                if let Some(status) = self.save_status.as_ref() {
                    ui.separator();
                    let is_error = ["Save failed", "Map reload failed", "Save partial"]
                        .iter()
                        .any(|prefix| status.starts_with(prefix));
                    let color = if is_error {
                        egui::Color32::LIGHT_RED
                    } else {
                        egui::Color32::LIGHT_GREEN
                    };
                    ui.colored_label(color, status);
                }

                let pending_sprites = self
                    .graphics_zip
                    .as_ref()
                    .map_or(0, |cache| cache.pending_count());
                if self.is_loading_world() || pending_sprites > 0 {
                    ui.separator();
                    ui.spinner();
                    if pending_sprites > 0 {
                        ui.label(format!("Decoding {pending_sprites} sprite(s)"));
                    }
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    self.ui_api_buttons(ui);
                });
            });
        });
    }

    /// Contents of the File menu.
    fn ui_file_menu(&mut self, ui: &mut egui::Ui) {
        if ui.button("Open snapshot...").clicked() {
            ui.close_menu();
            self.open_snapshot_dialog();
        }

        if ui.button("Reload snapshot").clicked() {
            self.load_current_source(LoadPurpose::Open);
            ui.close_menu();
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

        let is_live_api = self.data_source.is_live_api();
        let save_label = if is_live_api {
            "Save to API\tCtrl+S"
        } else {
            "Save Snapshot As..."
        };
        if ui
            .add_enabled(self.loaded_world.is_some(), egui::Button::new(save_label))
            .clicked()
        {
            ui.close_menu();
            self.save_current();
        }

        if is_live_api {
            if ui
                .add_enabled(
                    self.admin_client.is_some(),
                    egui::Button::new("Reload server map..."),
                )
                .clicked()
            {
                self.reload_confirm_open = true;
                ui.close_menu();
            }
            if ui
                .add_enabled(
                    self.pending_map_reload_request_id.is_some(),
                    egui::Button::new("Poll reload status"),
                )
                .clicked()
            {
                self.poll_map_reload_status();
                ui.close_menu();
            }
        }

        if ui
            .add_enabled(self.dirty, egui::Button::new("Revert (discard changes)"))
            .clicked()
        {
            ui.close_menu();
            self.revert_unsaved_changes();
        }

        ui.separator();

        ui.menu_button("Data Source", |ui| {
            let is_snap = matches!(self.data_source, DataSource::SnapshotFile(_));
            if ui.selectable_label(is_snap, ".wsnap Snapshot").clicked() {
                self.open_snapshot_dialog();
                ui.close_menu();
            }
            if ui
                .selectable_label(is_live_api, "Live Admin API...")
                .clicked()
            {
                self.open_connect_dialog();
                ui.close_menu();
            }
        });
    }

    /// Contents of the Settings menu (map flag visualization).
    fn ui_settings_menu(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        if ui
            .checkbox(&mut self.flag_viz_enabled, "Visualize map flags")
            .on_hover_text(
                "Tint each tile by its map flag combination. \
                 Tiles with identical flags share a color.",
            )
            .changed()
        {
            ctx.request_repaint();
        }

        ui.add_enabled_ui(self.flag_viz_enabled, |ui| {
            ui.add(egui::Slider::new(&mut self.flag_viz_opacity, 0.1..=0.9).text("Tint opacity"));

            ui.separator();
            ui.label("Flags to visualize:");
            ui.horizontal(|ui| {
                if ui.small_button("Gameplay").clicked() {
                    self.flag_viz_mask = GAMEPLAY_MAP_FLAG_MASK;
                }
                if ui.small_button("All").clicked() {
                    self.flag_viz_mask = u64::MAX;
                }
                if ui.small_button("None").clicked() {
                    self.flag_viz_mask = 0;
                }
            });

            egui::ScrollArea::vertical()
                .id_salt("flag_viz_mask_list")
                .max_height(300.0)
                .show(ui, |ui| {
                    for (mask, name) in map_flag_defs() {
                        flag_checkbox(ui, &mut self.flag_viz_mask, *mask, name);
                    }
                });
        });
    }

    /// Right-aligned connect / reload-server buttons.
    fn ui_api_buttons(&mut self, ui: &mut egui::Ui) {
        let is_live_api = self.data_source.is_live_api();
        if is_live_api {
            let reload_btn = egui::Button::new(
                egui::RichText::new("Reload Server Map").color(egui::Color32::WHITE),
            )
            .fill(egui::Color32::from_rgb(160, 60, 60));
            if ui
                .add_enabled(self.admin_client.is_some(), reload_btn)
                .on_hover_text(
                    "Ask the running server to drain pending map patches. \
                     You will be asked to confirm.",
                )
                .clicked()
            {
                self.reload_confirm_open = true;
            }
        }

        let connect_label = if is_live_api {
            "API: Connected"
        } else {
            "Connect to API..."
        };
        if ui
            .button(connect_label)
            .on_hover_text(
                "Point this viewer at a running API service \
                 (local dev or production).",
            )
            .clicked()
        {
            self.open_connect_dialog();
        }
    }

    /// Right-hand panel: source info, errors, controls, legend, hover and selected tile.
    pub(super) fn ui_side_panel(&mut self, ctx: &egui::Context) {
        egui::SidePanel::right("side_panel")
            .default_width(340.0)
            .show(ctx, |ui| {
                ui.heading("Map Viewer");

                ui.separator();
                ui.label(format!("Source: {}", self.data_source.display_label()));

                for err in [
                    &self.map_error,
                    &self.graphics_zip_error,
                    &self.items_error,
                    &self.item_templates_error,
                ]
                .into_iter()
                .flatten()
                {
                    ui.separator();
                    ui.colored_label(egui::Color32::LIGHT_RED, err);
                }

                ui.separator();
                ui.label(format!("Map size: {} x {}", SERVER_MAPX, SERVER_MAPY));
                ui.label(format!("Loaded tiles: {}", self.map_tiles.len()));

                ui.separator();
                ui.label("Controls:");
                ui.label("- WASD: pan");
                ui.label("- Drag: pan");
                ui.label("- Mouse wheel: zoom");
                ui.label("- Shift + left click: line mode");

                ui.separator();
                ui.label(format!("Pan: [{:.1}, {:.1}]", self.pan.x, self.pan.y));
                ui.label(format!("Zoom: {:.0}%", self.zoom * 100.0));
                if let Some((x, y)) = self.line_anchor {
                    ui.label(format!("Line anchor: ({}, {})", x, y));
                }

                if self.flag_viz_enabled {
                    ui.separator();
                    self.ui_flag_legend(ui);
                }

                ui.separator();
                self.ui_hovered_tile(ui, ctx);

                ui.separator();
                self.ui_selected_tile(ui, ctx);
            });
    }

    /// Legend of flag combinations visible on screen, most common first.
    fn ui_flag_legend(&self, ui: &mut egui::Ui) {
        ui.label("Map flag legend (in view):");
        if self.flag_viz_legend.is_empty() {
            ui.label("(no flagged tiles in view)");
            return;
        }
        egui::ScrollArea::vertical()
            .id_salt("flag_viz_legend")
            .max_height(180.0)
            .show(ui, |ui| {
                for (flags, count) in self
                    .flag_viz_legend
                    .iter()
                    .take(MAX_FLAG_VIZ_LEGEND_ENTRIES)
                {
                    ui.horizontal(|ui| {
                        let (swatch, _) =
                            ui.allocate_exact_size(Vec2::splat(14.0), egui::Sense::hover());
                        ui.painter()
                            .rect_filled(swatch, 2.0, flag_combo_color(*flags));
                        ui.label(format!("{} ({count})", flag_names(*flags).join(", ")));
                    });
                }
                let hidden = self
                    .flag_viz_legend
                    .len()
                    .saturating_sub(MAX_FLAG_VIZ_LEGEND_ENTRIES);
                if hidden > 0 {
                    ui.label(format!("... {hidden} more"));
                }
            });
    }

    /// Tile at `coords`, if the map is loaded and the coordinates are in range.
    fn tile_at(&self, coords: (usize, usize)) -> Option<Map> {
        self.map_tiles.get(tile_index(coords.0, coords.1)).copied()
    }

    /// Read-only details for the tile under the cursor.
    fn ui_hovered_tile(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        match self.hovered_tile {
            Some((x, y)) => ui.label(format!("Hover tile: ({}, {})", x, y)),
            None => ui.label("Hover tile: (N/A)"),
        };
        let tile = self.hovered_tile.and_then(|coords| self.tile_at(coords));
        ui_tile_fields(ui, tile.as_ref());
        self.ui_tile_preview_row(ui, ctx, tile.unwrap_or_default());
        self.ui_item_info(ui, tile.map_or(0, |t| t.it));
    }

    /// Editable details for the frozen selected tile.
    fn ui_selected_tile(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let Some((x, y)) = self.selected_tile else {
            ui.label("Selected tile: (none)");
            return;
        };
        ui.label(format!("Selected tile: ({}, {})", x, y));
        let Some(tile) = self.tile_at((x, y)) else {
            return;
        };

        ui_tile_fields(ui, Some(&tile));
        self.ui_tile_preview_row(ui, ctx, tile);

        let mut edited = false;
        ui.horizontal(|ui| {
            if tile.sprite != 0 && ui.button("Clear sprite").clicked() {
                edited |= self.edit_tile_with_undo(x, y, |t| t.sprite = 0);
            }
            if tile.fsprite != 0 && ui.button("Clear fsprite").clicked() {
                edited |= self.edit_tile_with_undo(x, y, |t| t.fsprite = 0);
            }
        });

        self.ui_item_info(ui, tile.it);
        if tile.it != 0 && ui.button("Clear item").clicked() {
            let undo_snapshot = self.snapshot_for_undo(&[(x, y)]);
            if self.clear_item_from_tile(x, y) {
                self.push_undo(undo_snapshot);
                edited = true;
            }
        }

        ui.separator();
        ui.label("Map flags:");
        let defs = map_flag_defs();
        let mut flags = tile.flags;
        egui::ScrollArea::vertical()
            .max_height(220.0)
            .show(ui, |ui| {
                egui::Grid::new("selected_tile_map_flags")
                    .num_columns(2)
                    .spacing([10.0, 4.0])
                    .show(ui, |ui| {
                        for (i, (mask, name)) in defs.iter().enumerate() {
                            flag_checkbox(ui, &mut flags, *mask, name);
                            if i % 2 == 1 {
                                ui.end_row();
                            }
                        }
                        if defs.len() % 2 == 1 {
                            ui.end_row();
                        }
                    });
            });
        if flags != tile.flags {
            edited |= self.edit_tile_with_undo(x, y, |t| t.flags = flags);
        }

        if ui
            .add_enabled(flags != 0, egui::Button::new("Flags → palette"))
            .on_hover_text(
                "Add a palette entry that sets this tile's flags, then \
                 click/Shift+click the map to paint them. Other flags on \
                 painted tiles are left untouched.",
            )
            .clicked()
        {
            self.push_flags_palette_entry(flags, false);
        }

        if edited {
            ctx.request_repaint();
        }
    }

    /// Item sprite/template labels for a tile's item instance id.
    fn ui_item_info(&self, ui: &mut egui::Ui, it: u32) {
        if it == 0 {
            ui.label("item sprite: N/A");
            ui.label("item template: N/A");
            return;
        }
        match self.items.get(it as usize) {
            Some(item) => {
                ui.label(format!(
                    "item sprite: {}",
                    item_map_sprite(*item).unwrap_or(0)
                ));
                ui.label(format!("item template: {}", item.temp));
            }
            None => {
                ui.label("item sprite: (item data not loaded)");
                ui.label("item template: (item data not loaded)");
            }
        }
    }

    /// Floor, object, and item sprite previews for one tile (blank slots when missing).
    fn ui_tile_preview_row(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, tile: Map) {
        let fsprite = if tile.fsprite != 0 && self.hide_enabled {
            tile.fsprite + 1
        } else {
            tile.fsprite
        };
        let item_sprite = self
            .items
            .get(tile.it as usize)
            .filter(|_| tile.it != 0)
            .and_then(|item| item_map_sprite(*item))
            .map_or(0, |s| s as usize);

        ui.horizontal(|ui| {
            for sprite in [tile.sprite as usize, fsprite as usize, item_sprite] {
                let sprite = (sprite != 0).then_some(sprite);
                self.ui_sprite_preview(ui, ctx, sprite, TILE_PREVIEW_SIZE);
            }
        });
    }
}
