//! Paint palette: entry types, persistence, and the floating palette window.

use super::MapViewerApp;
use super::editing::template_preview_sprite;
use super::flags::{flag_checkbox, flag_names, map_flag_defs};
use eframe::egui;
use egui::{Pos2, Rect, Vec2};
use mag_core::constants::USE_EMPTY;
use serde::{Deserialize, Serialize};

/// Which `Map` sprite field a palette sprite entry paints.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(super) enum SpriteLayer {
    /// Background/floor sprite (`Map::sprite`).
    #[default]
    Floor,
    /// Foreground/wall/object sprite (`Map::fsprite`).
    Object,
}

/// What a palette entry paints onto a tile.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(super) enum PaletteEntryKind {
    /// Write `sprite` into the given layer.
    Sprite { sprite: u16, layer: SpriteLayer },
    /// Place an item instance created from this template id.
    ItemTemplate(u16),
    /// Set (`clear == false`) or clear (`clear == true`) a mask of map flags.
    Flags { mask: u64, clear: bool },
}

/// One paintable palette entry (serialized to palette JSON files).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct PaletteEntry {
    pub(super) kind: PaletteEntryKind,
}

/// Size of one palette grid cell.
const PALETTE_ICON_SIZE: Vec2 = Vec2::new(48.0, 48.0);
/// Size of the sprite/template draft previews.
const PALETTE_PREVIEW_SIZE: Vec2 = Vec2::new(96.0, 96.0);
/// Number of columns in the palette grid.
const PALETTE_COLUMNS: usize = 4;

/// Fill used for text-only palette cells.
///
/// # Arguments
///
/// * `selected` - Whether the cell is the active palette entry.
///
/// # Returns
///
/// * Green when selected, dark gray otherwise.
fn palette_button_fill(selected: bool) -> egui::Color32 {
    if selected {
        egui::Color32::from_rgb(70, 110, 70)
    } else {
        egui::Color32::from_rgb(55, 55, 55)
    }
}

/// Fallback label for a palette grid cell when no texture preview is available.
///
/// # Arguments
///
/// * `entry` - The palette entry being labelled.
/// * `sprite_id` - Resolved preview sprite, if any.
///
/// # Returns
///
/// * A short multi-line label.
fn palette_entry_label(entry: PaletteEntry, sprite_id: Option<usize>) -> String {
    match entry.kind {
        PaletteEntryKind::Sprite { sprite, layer } => {
            let prefix = match layer {
                SpriteLayer::Floor => "F",
                SpriteLayer::Object => "O",
            };
            format!("{prefix}{sprite}")
        }
        PaletteEntryKind::ItemTemplate(template_id) => match sprite_id {
            Some(sprite_id) => format!("T{template_id}\nS{sprite_id}"),
            None => format!("T{template_id}"),
        },
        PaletteEntryKind::Flags { mask, clear } => {
            let verb = if clear { "Clear" } else { "Set" };
            format!("{verb}\n{} flag(s)", mask.count_ones())
        }
    }
}

impl MapViewerApp {
    /// Return the currently selected palette entry, clearing stale selection.
    pub(super) fn selected_palette_entry(&mut self) -> Option<PaletteEntry> {
        let index = self.selected_palette_index?;
        let entry = self.palette.get(index).copied();
        if entry.is_none() {
            self.selected_palette_index = None;
        }
        entry
    }

    /// Append a palette entry, select it, and report `status`.
    fn push_palette_entry(&mut self, kind: PaletteEntryKind, status: String) {
        self.palette.push(PaletteEntry { kind });
        self.selected_palette_index = Some(self.palette.len() - 1);
        self.save_status = Some(status);
    }

    /// Append a flags palette entry and select it so the next map click paints it.
    ///
    /// # Arguments
    ///
    /// * `mask` - Map flag bits the entry sets or clears.
    /// * `clear` - `true` to clear `mask` on painted tiles, `false` to set it.
    pub(super) fn push_flags_palette_entry(&mut self, mask: u64, clear: bool) {
        let status = format!(
            "Added {} flags entry to palette: {}",
            if clear { "clear" } else { "set" },
            flag_names(mask).join(", ")
        );
        self.push_palette_entry(PaletteEntryKind::Flags { mask, clear }, status);
    }

    /// Validate that a sprite id can be added to the palette.
    ///
    /// # Returns
    ///
    /// * `Err` with a user-facing reason when the sprite is unusable.
    fn can_add_palette_sprite(&mut self, ctx: &egui::Context, sprite: u16) -> Result<(), String> {
        if sprite == 0 {
            return Err("Sprite 0 cannot be painted".to_owned());
        }
        let Some(cache) = self.graphics_zip.as_mut() else {
            return Err("No graphics zip loaded".to_owned());
        };
        if !cache.contains(sprite as usize) {
            return Err(format!("Sprite {} is not present in graphics zip", sprite));
        }
        cache
            .texture_for(ctx, sprite as usize)
            .map(|_| ())
            .map_err(|e| format!("Sprite {} could not be loaded: {}", sprite, e))
    }

    /// Whether a texture exists for `sprite + 1`, the frame the real client
    /// substitutes for object sprites in Hide Walls mode.
    fn has_object_hide_companion(&self, sprite: u16) -> bool {
        let Some(companion) = sprite.checked_add(1) else {
            return false;
        };
        // Can't validate without a loaded graphics zip; don't warn.
        self.graphics_zip
            .as_ref()
            .is_none_or(|cache| cache.contains(companion as usize))
    }

    /// Resolve (and lazily load) the preview sprite for a palette entry.
    fn palette_entry_sprite(&mut self, entry: PaletteEntry) -> Option<usize> {
        match entry.kind {
            PaletteEntryKind::Sprite { sprite, .. } => (sprite != 0).then_some(sprite as usize),
            PaletteEntryKind::ItemTemplate(template_id) => {
                let it_idx = template_id as usize;
                let template = self.item_templates.get(it_idx).copied()?;
                if template_id == 0 || template.used == USE_EMPTY {
                    return None;
                }
                if let Err(e) = self.ensure_item_template_loaded(template_id) {
                    self.item_templates_error = Some(e);
                }
                template_preview_sprite(self.item_templates[it_idx])
            }
            PaletteEntryKind::Flags { .. } => None,
        }
    }

    /// Save the current palette to a JSON file chosen via a file dialog.
    fn save_palette_dialog(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Palette JSON", &["json"])
            .set_file_name("palette.json")
            .save_file()
        else {
            return;
        };

        let result = serde_json::to_string_pretty(&self.palette)
            .map_err(|e| e.to_string())
            .and_then(|json| std::fs::write(&path, json).map_err(|e| e.to_string()));

        self.save_status = Some(match result {
            Ok(()) => format!("Saved palette: {}", path.display()),
            Err(e) => format!("Save palette failed: {e}"),
        });
    }

    /// Load a palette from a JSON file chosen via a file dialog, replacing the current one.
    fn load_palette_dialog(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Palette JSON", &["json"])
            .pick_file()
        else {
            return;
        };

        let result = std::fs::read_to_string(&path)
            .map_err(|e| e.to_string())
            .and_then(|contents| {
                serde_json::from_str::<Vec<PaletteEntry>>(&contents).map_err(|e| e.to_string())
            });

        match result {
            Ok(palette) => {
                self.palette = palette;
                self.selected_palette_index = None;
                self.save_status = Some(format!("Loaded palette: {}", path.display()));
            }
            Err(e) => self.save_status = Some(format!("Load palette failed: {e}")),
        }
    }

    /// Draw the floating palette window.
    ///
    /// # Arguments
    ///
    /// * `ctx` - egui context hosting the window.
    /// * `anchor` - Default top-left position of the window.
    ///
    /// # Returns
    ///
    /// * The window's screen rect, used to ignore map clicks that land on it.
    pub(super) fn render_palette_overlay(&mut self, ctx: &egui::Context, anchor: Pos2) -> Rect {
        let response = egui::Window::new("Palette")
            .id(egui::Id::new("map_palette_overlay_window"))
            .default_pos(anchor)
            .default_size(Vec2::new(360.0, 420.0))
            .resizable(true)
            .movable(true)
            .collapsible(false)
            .show(ctx, |ui| {
                ui.set_min_width(260.0);
                self.ui_palette_toolbar(ui);
                ui.separator();
                self.ui_palette_sprite_row(ui, ctx);
                self.ui_palette_template_row(ui, ctx);
                ui.separator();
                self.ui_palette_flag_rows(ui);
                ui.separator();
                self.ui_palette_grid(ui, ctx);
            });

        response
            .map(|inner| inner.response.rect)
            .unwrap_or_else(|| {
                self.palette_rect
                    .unwrap_or(Rect::from_min_size(anchor, Vec2::new(260.0, 260.0)))
            })
    }

    /// Remove / save / load buttons.
    fn ui_palette_toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if ui
                .add_enabled(
                    self.selected_palette_index.is_some(),
                    egui::Button::new("Remove selected"),
                )
                .clicked()
                && let Some(index) = self.selected_palette_index.take()
                && index < self.palette.len()
            {
                self.palette.remove(index);
            }
            if ui.small_button("Save palette...").clicked() {
                self.save_palette_dialog();
            }
            if ui.small_button("Load palette...").clicked() {
                self.load_palette_dialog();
            }
        });
    }

    /// Draw a sprite preview, or reserve the same space when it's unavailable.
    pub(super) fn ui_sprite_preview(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &egui::Context,
        sprite: Option<usize>,
        size: Vec2,
    ) {
        let texture = sprite.and_then(|sprite| {
            self.graphics_zip
                .as_mut()
                .and_then(|cache| cache.texture_for(ctx, sprite).ok().flatten())
        });
        match texture {
            Some(texture) => {
                ui.add(
                    egui::Image::new(texture)
                        .fit_to_exact_size(size)
                        .maintain_aspect_ratio(true),
                );
            }
            None => {
                ui.allocate_exact_size(size, egui::Sense::hover());
            }
        }
    }

    /// Draft row for adding floor/object sprite entries.
    fn ui_palette_sprite_row(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.horizontal(|ui| {
            ui.label("sprite:");
            ui.add(egui::DragValue::new(&mut self.draft_sprite));
            ui.selectable_value(&mut self.draft_sprite_layer, SpriteLayer::Floor, "Floor");
            ui.selectable_value(&mut self.draft_sprite_layer, SpriteLayer::Object, "Object");

            let sprite = self.draft_sprite;
            self.ui_sprite_preview(ui, ctx, Some(sprite as usize), PALETTE_PREVIEW_SIZE);

            if !ui.small_button("Add").clicked() || sprite == 0 {
                return;
            }
            if let Err(e) = self.can_add_palette_sprite(ctx, sprite) {
                self.save_status = Some(e);
                return;
            }
            let layer = self.draft_sprite_layer;
            let status = if layer == SpriteLayer::Object
                && !self.has_object_hide_companion(sprite)
            {
                format!(
                    "Added sprite {sprite} (Object); no companion texture at {} — Hide Walls will show an error texture near this tile.",
                    u32::from(sprite) + 1
                )
            } else {
                format!("Added sprite {sprite} ({layer:?}) to palette")
            };
            self.push_palette_entry(PaletteEntryKind::Sprite { sprite, layer }, status);
        });
    }

    /// Draft row for adding item template entries.
    fn ui_palette_template_row(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.horizontal(|ui| {
            ui.label("template:");
            ui.add(egui::DragValue::new(&mut self.draft_item_template_id));

            let template_id = self.draft_item_template_id;
            let it_idx = template_id as usize;
            let in_range = it_idx < self.item_templates.len();
            let is_used = in_range && self.item_templates[it_idx].used != USE_EMPTY;

            if template_id != 0
                && is_used
                && let Err(e) = self.ensure_item_template_loaded(template_id)
            {
                self.item_templates_error = Some(e);
            }

            let preview_sprite = is_used
                .then(|| template_preview_sprite(self.item_templates[it_idx]))
                .flatten();
            self.ui_sprite_preview(ui, ctx, preview_sprite, PALETTE_PREVIEW_SIZE);
            if let Some(sprite) = preview_sprite {
                ui.label(format!("sprite: {}", sprite));
            }

            if ui.small_button("Add").clicked() && template_id != 0 {
                if !in_range {
                    self.save_status = Some("Template id is out of range".to_owned());
                } else if let Err(e) = self.ensure_item_template_loaded(template_id) {
                    self.save_status = Some(e);
                } else if !is_used {
                    self.save_status = Some("Template slot is unused".to_owned());
                } else {
                    self.push_palette_entry(
                        PaletteEntryKind::ItemTemplate(template_id),
                        format!("Added template {} to palette", template_id),
                    );
                }
            }

            if template_id != 0 && !is_used {
                let message = if in_range {
                    "Template slot is unused"
                } else {
                    "Template id is out of range"
                };
                ui.colored_label(egui::Color32::LIGHT_RED, message);
            }
        });
    }

    /// Draft rows for building set/clear flag entries.
    fn ui_palette_flag_rows(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            ui.label("flags:");
            for (mask, name) in map_flag_defs() {
                flag_checkbox(ui, &mut self.draft_flag_mask, *mask, name);
            }
        });

        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.draft_flag_clear, false, "Set");
            ui.selectable_value(&mut self.draft_flag_clear, true, "Clear");

            if ui.small_button("Add").clicked() && self.draft_flag_mask != 0 {
                self.push_flags_palette_entry(self.draft_flag_mask, self.draft_flag_clear);
                self.draft_flag_mask = 0;
            }
        });
    }

    /// Grid of palette entries; clicking toggles selection.
    fn ui_palette_grid(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            egui::Grid::new("palette_image_grid")
                .num_columns(PALETTE_COLUMNS)
                .spacing([6.0, 6.0])
                .show(ui, |ui| {
                    for idx in 0..self.palette.len() {
                        let entry = self.palette[idx];
                        let selected = self.selected_palette_index == Some(idx);
                        if self.ui_palette_cell(ui, ctx, entry, selected).clicked() {
                            self.selected_palette_index = (!selected).then_some(idx);
                        }
                        if idx % PALETTE_COLUMNS == PALETTE_COLUMNS - 1 {
                            ui.end_row();
                        }
                    }
                    if !self.palette.len().is_multiple_of(PALETTE_COLUMNS) {
                        ui.end_row();
                    }
                });
        });
    }

    /// One palette grid cell: a sprite image when available, otherwise a labelled button.
    fn ui_palette_cell(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &egui::Context,
        entry: PaletteEntry,
        selected: bool,
    ) -> egui::Response {
        let sprite_id = self.palette_entry_sprite(entry);

        if let Some(sprite_id) = sprite_id
            && let Some(cache) = self.graphics_zip.as_mut()
            && let Ok(Some(texture)) = cache.texture_for(ctx, sprite_id)
        {
            let tint = if selected {
                egui::Color32::from_rgb(180, 255, 180)
            } else {
                egui::Color32::WHITE
            };
            return ui.add(
                egui::Image::new(texture)
                    .fit_to_exact_size(PALETTE_ICON_SIZE)
                    .maintain_aspect_ratio(true)
                    .tint(tint)
                    .sense(egui::Sense::click()),
            );
        }

        let response = ui.add_sized(
            PALETTE_ICON_SIZE,
            egui::Button::new(palette_entry_label(entry, sprite_id))
                .fill(palette_button_fill(selected)),
        );
        match entry.kind {
            PaletteEntryKind::Flags { mask, clear } => response.on_hover_text(format!(
                "{}:\n{}\n\nClick map to paint, Shift+click for lines.",
                if clear { "Clear" } else { "Set" },
                flag_names(mask).join("\n")
            )),
            _ => response,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::MapViewerApp;
    use super::{PaletteEntry, PaletteEntryKind, SpriteLayer, palette_entry_label};

    #[test]
    fn push_flags_palette_entry_appends_and_selects() {
        let mut app = MapViewerApp::for_tests(4, 4);
        app.push_flags_palette_entry(0b11, false);
        app.push_flags_palette_entry(0b100, true);
        assert_eq!(app.palette.len(), 2);
        assert_eq!(app.selected_palette_index, Some(1));
        assert_eq!(
            app.palette[1].kind,
            PaletteEntryKind::Flags {
                mask: 0b100,
                clear: true
            }
        );
    }

    #[test]
    fn selected_palette_entry_clears_stale_index() {
        let mut app = MapViewerApp::for_tests(4, 4);
        app.selected_palette_index = Some(3);
        assert_eq!(app.selected_palette_entry(), None);
        assert_eq!(app.selected_palette_index, None);
    }

    #[test]
    fn palette_entry_label_formats_each_kind() {
        let sprite = PaletteEntry {
            kind: PaletteEntryKind::Sprite {
                sprite: 12,
                layer: SpriteLayer::Object,
            },
        };
        assert_eq!(palette_entry_label(sprite, Some(12)), "O12");

        let template = PaletteEntry {
            kind: PaletteEntryKind::ItemTemplate(7),
        };
        assert_eq!(palette_entry_label(template, None), "T7");
        assert_eq!(palette_entry_label(template, Some(40)), "T7\nS40");

        let flags = PaletteEntry {
            kind: PaletteEntryKind::Flags {
                mask: 0b1011,
                clear: true,
            },
        };
        assert_eq!(palette_entry_label(flags, None), "Clear\n3 flag(s)");
    }

    #[test]
    fn palette_entry_json_roundtrip() {
        let entries = vec![
            PaletteEntry {
                kind: PaletteEntryKind::Sprite {
                    sprite: 5,
                    layer: SpriteLayer::Floor,
                },
            },
            PaletteEntry {
                kind: PaletteEntryKind::Flags {
                    mask: 1 << 40,
                    clear: false,
                },
            },
        ];
        let json = serde_json::to_string(&entries).expect("serialize palette");
        let decoded: Vec<PaletteEntry> = serde_json::from_str(&json).expect("deserialize palette");
        assert_eq!(decoded, entries);
    }
}
