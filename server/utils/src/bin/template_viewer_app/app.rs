//! Template viewer/editor application state and per-frame orchestration.
//!
//! Feature areas live in submodules that each extend [`TemplateViewerApp`]:
//! `panels` (menu bar and layout), `lists`, `item_details`, `character_details`,
//! `widgets` (shared editor widgets), `duplicate`, and `live_api`.

mod character_details;
mod duplicate;
mod item_details;
mod lists;
mod live_api;
mod panels;
mod widgets;

use super::graphics::GraphicsZipCache;
use eframe::egui;
use egui::Vec2;
use server::keydb::snapshot::WorldSnapshot;
use server_utils::{AdminClient, DataSource, load_world_snapshot, save_world_snapshot};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Active tab; also identifies which record table an edit belongs to.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum ViewMode {
    #[default]
    ItemTemplates,
    CharacterTemplates,
    Items,
    Characters,
}

impl ViewMode {
    /// All tabs, in menu-bar order.
    const ALL: [Self; 4] = [
        Self::ItemTemplates,
        Self::CharacterTemplates,
        Self::Items,
        Self::Characters,
    ];

    /// Tab / list heading title.
    fn title(self) -> &'static str {
        match self {
            Self::ItemTemplates => "Item Templates",
            Self::CharacterTemplates => "Character Templates",
            Self::Items => "Items",
            Self::Characters => "Characters",
        }
    }

    /// Placeholder shown when nothing is selected.
    fn empty_hint(self) -> &'static str {
        match self {
            Self::ItemTemplates => "Select an item template from the list",
            Self::CharacterTemplates => "Select a character template from the list",
            Self::Items => "Select an item from the list",
            Self::Characters => "Select a character from the list",
        }
    }

    /// Per-tab id for the list side panel, so each tab keeps its own width.
    fn list_panel_id(self) -> &'static str {
        match self {
            Self::ItemTemplates => "item_template_list",
            Self::CharacterTemplates => "character_template_list",
            Self::Items => "item_list",
            Self::Characters => "character_list",
        }
    }

    /// Whether this is a template table (vs. live instances).
    fn is_template(self) -> bool {
        matches!(self, Self::ItemTemplates | Self::CharacterTemplates)
    }

    /// Whether this table holds items (vs. characters).
    fn is_item(self) -> bool {
        matches!(self, Self::ItemTemplates | Self::Items)
    }
}

/// Template viewer/editor application state.
#[derive(Default)]
pub(crate) struct TemplateViewerApp {
    loaded_world: Option<WorldSnapshot>,
    item_templates: Vec<mag_core::types::Item>,
    character_templates: Vec<mag_core::types::Character>,
    items: Vec<mag_core::types::Item>,
    characters: Vec<mag_core::types::Character>,
    map_tiles: Vec<mag_core::types::Map>,
    selected_item_index: Option<usize>,
    selected_character_index: Option<usize>,
    selected_item_instance_index: Option<usize>,
    selected_character_instance_index: Option<usize>,
    /// Item id shown in the floating item-template window.
    item_popup_id: Option<u32>,
    view_mode: ViewMode,
    item_filter: String,
    character_filter: String,
    item_instance_filter: String,
    character_instance_filter: String,
    character_instances_player_only: bool,
    show_unused_instances: bool,
    show_unused_templates: bool,
    show_all_data_fields: bool,
    load_error: Option<String>,
    graphics_zip: Option<GraphicsZipCache>,
    graphics_zip_error: Option<String>,
    dirty: bool,
    /// Slots in `item_templates` that have unsaved edits (LiveApi mode).
    dirty_item_template_slots: HashSet<usize>,
    /// Slots in `character_templates` that have unsaved edits (LiveApi mode).
    dirty_character_template_slots: HashSet<usize>,
    /// Slots in `items` (live world state) that have unsaved edits.
    dirty_item_slots: HashSet<usize>,
    /// Slots in `characters` (live world state) that have unsaved edits.
    dirty_character_slots: HashSet<usize>,
    /// Item template slots whose full payload is present (LiveApi loads them lazily).
    fully_loaded_item_slots: HashSet<usize>,
    /// Character template slots whose full payload is present.
    fully_loaded_char_slots: HashSet<usize>,
    /// Cached admin API client for LiveApi mode.
    admin_client: Option<AdminClient>,
    /// Pending reload request id awaiting a status update.
    pending_reload_request_id: Option<String>,
    /// Whether the "Connect to admin API" modal dialog is open.
    connect_dialog_open: bool,
    /// Working draft of the API base URL inside the connect dialog.
    connect_form_base_url: String,
    /// Working draft of the admin token inside the connect dialog.
    connect_form_token: String,
    /// Whether the admin-token field is currently shown in plaintext.
    connect_form_show_token: bool,
    /// Last error reported by the connect dialog (e.g. failed fetch).
    connect_dialog_error: Option<String>,
    /// Whether the "confirm server reload" modal dialog is open.
    reload_confirm_open: bool,
    /// Wall-clock instant when the most recent reload request was fired.
    pending_reload_since: Option<std::time::Instant>,
    /// Wall-clock instant of the last automatic reload-status poll.
    last_reload_poll: Option<std::time::Instant>,
    save_status: Option<String>,
    /// Scroll the active list to its selected entry on the next frame.
    scroll_to_selection: bool,
    /// Cached "Where used" map scan for item templates.
    where_used: item_details::WhereUsedCache,
    /// Whether the deferred first load has run.
    initial_load_done: bool,
    /// Frames rendered so far; loading waits a couple so the window appears first.
    frame_count: u32,
    data_source: DataSource,
}

impl TemplateViewerApp {
    /// Create the app for a data source; loading is deferred to the first frames.
    ///
    /// # Arguments
    ///
    /// * `data_source` - Snapshot file or live admin API to load from.
    ///
    /// # Returns
    ///
    /// * A new, not-yet-loaded app.
    pub(crate) fn new(data_source: DataSource) -> Self {
        let admin_client = match &data_source {
            DataSource::LiveApi { base_url, token } => {
                AdminClient::new(base_url.clone(), token.clone()).ok()
            }
            _ => None,
        };
        Self {
            data_source,
            admin_client,
            ..Self::default()
        }
    }

    /// Clear the dirty flag and every per-slot dirty set.
    fn clear_dirty(&mut self) {
        self.dirty = false;
        self.dirty_item_template_slots.clear();
        self.dirty_character_template_slots.clear();
        self.dirty_item_slots.clear();
        self.dirty_character_slots.clear();
    }

    /// Record an unsaved edit to `table[idx]` so LiveApi save only PUTs edited slots.
    ///
    /// # Arguments
    ///
    /// * `table` - Table the edited record belongs to.
    /// * `idx` - Edited slot.
    fn mark_slot_dirty(&mut self, table: ViewMode, idx: usize) {
        self.dirty = true;
        if table == ViewMode::Items {
            self.invalidate_where_used();
        }
        let slots = match table {
            ViewMode::ItemTemplates => &mut self.dirty_item_template_slots,
            ViewMode::CharacterTemplates => &mut self.dirty_character_template_slots,
            ViewMode::Items => &mut self.dirty_item_slots,
            ViewMode::Characters => &mut self.dirty_character_slots,
        };
        slots.insert(idx);
    }

    /// Selected slot in `mode`'s table.
    fn selected_index(&self, mode: ViewMode) -> Option<usize> {
        match mode {
            ViewMode::ItemTemplates => self.selected_item_index,
            ViewMode::CharacterTemplates => self.selected_character_index,
            ViewMode::Items => self.selected_item_instance_index,
            ViewMode::Characters => self.selected_character_instance_index,
        }
    }

    /// Mutable selected slot in `mode`'s table.
    fn selected_index_mut(&mut self, mode: ViewMode) -> &mut Option<usize> {
        match mode {
            ViewMode::ItemTemplates => &mut self.selected_item_index,
            ViewMode::CharacterTemplates => &mut self.selected_character_index,
            ViewMode::Items => &mut self.selected_item_instance_index,
            ViewMode::Characters => &mut self.selected_character_instance_index,
        }
    }

    /// Drop all loaded world data (after a failed load).
    fn clear_loaded_world(&mut self) {
        self.loaded_world = None;
        self.item_templates.clear();
        self.character_templates.clear();
        self.items.clear();
        self.characters.clear();
        self.map_tiles.clear();
        self.selected_item_index = None;
        self.selected_character_index = None;
        self.selected_item_instance_index = None;
        self.selected_character_instance_index = None;
        self.item_popup_id = None;
        self.clear_dirty();
        self.invalidate_where_used();
        self.fully_loaded_item_slots.clear();
        self.fully_loaded_char_slots.clear();
    }

    /// Install a freshly loaded world and report `status`.
    fn apply_loaded_world(&mut self, mut world: WorldSnapshot, status: String) {
        // Views own the vectors; `sync_loaded_world_from_views` puts them back before saving.
        self.item_templates = std::mem::take(&mut world.item_templates);
        self.character_templates = std::mem::take(&mut world.character_templates);
        self.items = std::mem::take(&mut world.items);
        self.characters = std::mem::take(&mut world.characters);
        self.map_tiles = std::mem::take(&mut world.map);
        self.loaded_world = Some(world);
        self.save_status = Some(status);
        self.clear_dirty();
        self.invalidate_where_used();

        // Snapshot sources carry full template data; LiveApi fills slots lazily on first view.
        if self.data_source.is_live_api() {
            self.fully_loaded_item_slots.clear();
            self.fully_loaded_char_slots.clear();
        } else {
            self.fully_loaded_item_slots = (0..self.item_templates.len()).collect();
            self.fully_loaded_char_slots = (0..self.character_templates.len()).collect();
        }

        if self.view_mode.is_template() {
            if !self.item_templates.is_empty() {
                self.view_mode = ViewMode::ItemTemplates;
            } else if !self.character_templates.is_empty() {
                self.view_mode = ViewMode::CharacterTemplates;
            }
        }
    }

    /// (Re)load the world from the current data source.
    fn load_current_source(&mut self) {
        if matches!(self.data_source, DataSource::NotLoaded) {
            return;
        }

        self.load_error = None;
        self.save_status = None;

        match load_world_snapshot(&self.data_source) {
            Ok(world) => {
                // LiveApi status text is set by the connect flow instead.
                let status = self
                    .data_source
                    .snapshot_path()
                    .map(|path| format!("Loaded snapshot: {}", path.display()))
                    .unwrap_or_default();

                log::info!(
                    "Loaded world for template viewer: items={} chars={} item_templates={} char_templates={} map={} source={}",
                    world.items.len(),
                    world.characters.len(),
                    world.item_templates.len(),
                    world.character_templates.len(),
                    world.map.len(),
                    self.data_source.display_label()
                );

                self.apply_loaded_world(world, status);
            }
            Err(e) => {
                self.clear_loaded_world();
                self.load_error = Some(e);
            }
        }
    }

    /// Recompute derived fields and copy edited views back into the loaded snapshot.
    fn sync_loaded_world_from_views(&mut self) -> Result<(), String> {
        for tpl in &mut self.character_templates {
            tpl.points_tot = server::points::calculate_points_tot(tpl);
        }

        let Some(world) = self.loaded_world.as_mut() else {
            return Err("No world loaded".to_owned());
        };

        world.item_templates = self.item_templates.clone();
        world.character_templates = self.character_templates.clone();
        world.items = self.items.clone();
        world.characters = self.characters.clone();
        world.map = self.map_tiles.clone();
        Ok(())
    }

    /// Write the current world to `path` and switch the data source to it.
    fn save_snapshot_as(&mut self, path: &Path) -> Result<(), String> {
        self.sync_loaded_world_from_views()?;
        let world = self
            .loaded_world
            .as_ref()
            .ok_or_else(|| "No world loaded".to_owned())?;
        save_world_snapshot(world, path)?;
        self.data_source = DataSource::SnapshotFile(path.to_path_buf());
        Ok(())
    }

    /// Pick a destination and save the world snapshot there.
    fn save_snapshot_as_dialog(&mut self) {
        self.save_status = None;

        let Some(path) = rfd::FileDialog::new()
            .add_filter("World Snapshot", &["wsnap"])
            .set_file_name("world_snapshot.wsnap")
            .save_file()
        else {
            return;
        };

        match self.save_snapshot_as(&path) {
            Ok(()) => {
                self.clear_dirty();
                self.save_status = Some(format!("Saved snapshot: {}", path.display()));
            }
            Err(e) => self.save_status = Some(format!("Save failed: {e}")),
        }
    }

    /// Save to the API in LiveApi mode, otherwise to a snapshot file.
    fn save_current(&mut self) {
        if self.data_source.is_live_api() {
            self.save_to_api();
        } else {
            self.save_snapshot_as_dialog();
        }
    }

    /// Switch to a snapshot file and load it.
    fn load_from_snapshot(&mut self, path: PathBuf) {
        self.data_source = DataSource::SnapshotFile(path);
        self.load_current_source();
    }

    /// Pick a `.wsnap` file and load it.
    fn open_snapshot_dialog(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("World Snapshot", &["wsnap"])
            .pick_file()
        {
            self.load_from_snapshot(path);
        }
    }

    /// Reload from the data source, keeping the current tab and (still valid) selections.
    fn revert_unsaved_changes(&mut self) {
        self.save_status = None;

        let prev_view_mode = self.view_mode;
        let prev_selection = ViewMode::ALL.map(|mode| self.selected_index(mode));

        self.load_current_source();

        self.view_mode = prev_view_mode;
        for (mode, selected) in ViewMode::ALL.into_iter().zip(prev_selection) {
            let len = match mode {
                ViewMode::ItemTemplates => self.item_templates.len(),
                ViewMode::CharacterTemplates => self.character_templates.len(),
                ViewMode::Items => self.items.len(),
                ViewMode::Characters => self.characters.len(),
            };
            *self.selected_index_mut(mode) = selected.filter(|&i| i < len);
        }

        self.clear_dirty();
        self.save_status = Some(if self.load_error.is_some() {
            "Reverted changes (with load errors)".to_owned()
        } else {
            "Reverted unsaved changes".to_owned()
        });
    }

    /// Load the graphics zip used for sprite previews.
    fn load_graphics_zip(&mut self, zip_path: PathBuf) {
        match GraphicsZipCache::load(zip_path) {
            Ok(cache) => {
                self.graphics_zip = Some(cache);
                self.graphics_zip_error = None;
            }
            Err(e) => {
                self.graphics_zip = None;
                self.graphics_zip_error = Some(e);
            }
        }
    }

    /// Sprite preview, falling back to the numeric id while loading or when unavailable.
    fn sprite_cell(&mut self, ui: &mut egui::Ui, sprite_id: usize) {
        let Some(cache) = self.graphics_zip.as_mut() else {
            crate::centered_label(ui, format!("{}", sprite_id));
            return;
        };

        match cache.texture_for(ui.ctx(), sprite_id) {
            Ok(Some(texture)) => {
                ui.add(
                    egui::Image::new(texture)
                        .fit_to_exact_size(Vec2::new(125.0, 125.0))
                        .maintain_aspect_ratio(true),
                );
            }
            Ok(None) => crate::centered_label(ui, format!("{}", sprite_id)),
            Err(e) => {
                self.graphics_zip_error = Some(e);
                crate::centered_label(ui, format!("{}", sprite_id));
            }
        }
    }
}

impl eframe::App for TemplateViewerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.frame_count += 1;

        let save_shortcut =
            ctx.input(|i| (i.modifiers.command || i.modifiers.ctrl) && i.key_pressed(egui::Key::S));
        if save_shortcut && self.loaded_world.is_some() {
            self.save_current();
        }

        if !self.initial_load_done && self.frame_count > 2 {
            self.initial_load_done = true;
            self.load_current_source();
            if let Some(zip_path) = server_utils::graphics_zip_from_args()
                .or_else(server_utils::default_graphics_zip_path)
            {
                self.load_graphics_zip(zip_path);
            }
        }

        self.tick_reload_poll(ctx);
        self.ui_top_bar(ctx);
        self.ui_central_panel(ctx);
        self.render_item_popup(ctx);
        self.render_connect_dialog(ctx);
        self.render_reload_confirm_dialog(ctx);
    }
}

#[cfg(test)]
mod tests {
    use super::{TemplateViewerApp, ViewMode};

    #[test]
    fn mark_slot_dirty_and_clear_dirty_roundtrip() {
        let mut app = TemplateViewerApp::default();
        for (i, mode) in ViewMode::ALL.into_iter().enumerate() {
            app.mark_slot_dirty(mode, i);
        }
        assert!(app.dirty);
        assert!(app.dirty_item_template_slots.contains(&0));
        assert!(app.dirty_character_template_slots.contains(&1));
        assert!(app.dirty_item_slots.contains(&2));
        assert!(app.dirty_character_slots.contains(&3));

        app.clear_dirty();
        assert!(!app.dirty);
        assert!(app.dirty_item_template_slots.is_empty());
        assert!(app.dirty_character_slots.is_empty());
    }

    #[test]
    fn selected_index_mut_targets_each_table() {
        let mut app = TemplateViewerApp::default();
        for (i, mode) in ViewMode::ALL.into_iter().enumerate() {
            *app.selected_index_mut(mode) = Some(i);
        }
        assert_eq!(
            ViewMode::ALL.map(|mode| app.selected_index(mode)),
            [Some(0), Some(1), Some(2), Some(3)]
        );
    }

    #[test]
    fn view_mode_classification() {
        assert!(ViewMode::ItemTemplates.is_template() && ViewMode::ItemTemplates.is_item());
        assert!(ViewMode::CharacterTemplates.is_template());
        assert!(!ViewMode::CharacterTemplates.is_item());
        assert!(!ViewMode::Items.is_template() && ViewMode::Items.is_item());
        assert!(!ViewMode::Characters.is_template() && !ViewMode::Characters.is_item());
    }
}
