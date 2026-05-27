use super::graphics::GraphicsZipCache;
use eframe::egui;
use egui::Vec2;
use mag_core::skills;
use mag_core::string_operations::c_string_to_str;
use mag_core::{ranks, traits};
use server::keydb::snapshot::WorldSnapshot;
use server_utils::{AdminClient, DataSource, load_world_snapshot, save_world_snapshot};
use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ItemDetailsSource {
    ItemTemplates,
    Items,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CharacterDetailsSource {
    CharacterTemplates,
    Characters,
}

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
    item_popup_id: Option<u32>,
    view_mode: ViewMode,
    item_filter: String,
    character_filter: String,
    item_instance_filter: String,
    character_instance_filter: String,
    character_instances_player_only: bool,
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
    /// Item template slots whose full bincode payload has been fetched from
    /// the API. Slots absent from this set show a placeholder in the detail
    /// panel until they are lazily loaded on first selection.
    fully_loaded_item_slots: HashSet<usize>,
    /// Character template slots whose full bincode payload has been fetched.
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
    /// Used to auto-poll the status endpoint every ~2 s until applied or
    /// the 60-second TTL elapses.
    pending_reload_since: Option<std::time::Instant>,
    /// Wall-clock instant of the last automatic reload-status poll.
    last_reload_poll: Option<std::time::Instant>,
    save_status: Option<String>,
    initial_load_done: bool,
    frame_count: u32,
    data_source: DataSource,
}

#[derive(Clone, Copy, PartialEq)]
enum ViewMode {
    ItemTemplates,
    CharacterTemplates,
    Items,
    Characters,
}

impl Default for TemplateViewerApp {
    fn default() -> Self {
        Self {
            loaded_world: None,
            item_templates: Vec::new(),
            character_templates: Vec::new(),
            items: Vec::new(),
            characters: Vec::new(),
            map_tiles: Vec::new(),
            selected_item_index: None,
            selected_character_index: None,
            selected_item_instance_index: None,
            selected_character_instance_index: None,
            item_popup_id: None,
            view_mode: ViewMode::ItemTemplates,
            item_filter: String::new(),
            character_filter: String::new(),
            item_instance_filter: String::new(),
            character_instance_filter: String::new(),
            character_instances_player_only: false,
            show_unused_templates: false,
            show_all_data_fields: false,
            load_error: None,
            graphics_zip: None,
            graphics_zip_error: None,
            dirty: false,
            dirty_item_template_slots: HashSet::new(),
            dirty_character_template_slots: HashSet::new(),
            dirty_item_slots: HashSet::new(),
            dirty_character_slots: HashSet::new(),
            fully_loaded_item_slots: HashSet::new(),
            fully_loaded_char_slots: HashSet::new(),
            admin_client: None,
            pending_reload_request_id: None,
            connect_dialog_open: false,
            connect_form_base_url: String::new(),
            connect_form_token: String::new(),
            connect_form_show_token: false,
            connect_dialog_error: None,
            reload_confirm_open: false,
            pending_reload_since: None,
            last_reload_poll: None,
            save_status: None,
            initial_load_done: false,
            frame_count: 0,
            data_source: DataSource::default(),
        }
    }
}

impl TemplateViewerApp {
    pub(crate) fn new(data_source: DataSource) -> Self {
        let admin_client = match &data_source {
            DataSource::LiveApi { base_url, token } => {
                AdminClient::new(base_url.clone(), token.clone()).ok()
            }
            _ => None,
        };
        Self {
            data_source: data_source.clone(),
            admin_client,
            ..Self::default()
        }
    }

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
        self.dirty = false;
        self.dirty_item_template_slots.clear();
        self.dirty_character_template_slots.clear();
        self.dirty_item_slots.clear();
        self.dirty_character_slots.clear();
        self.fully_loaded_item_slots.clear();
        self.fully_loaded_char_slots.clear();
    }

    fn apply_loaded_world(&mut self, world: WorldSnapshot, status: String) {
        self.item_templates = world.item_templates.clone();
        self.character_templates = world.character_templates.clone();
        self.items = world.items.clone();
        self.characters = world.characters.clone();
        self.map_tiles = world.map.clone();
        self.loaded_world = Some(world);
        self.save_status = Some(status);
        self.dirty = false;
        self.dirty_item_template_slots.clear();
        self.dirty_character_template_slots.clear();
        self.dirty_item_slots.clear();
        self.dirty_character_slots.clear();
        self.fully_loaded_item_slots.clear();
        self.fully_loaded_char_slots.clear();

        // For sources that supply full template data up-front (KeyDB / snapshot),
        // mark every slot as loaded. For LiveApi the slot data arrives lazily on
        // first selection, so we leave the sets empty.
        if !self.data_source.is_live_api() {
            for i in 0..self.item_templates.len() {
                self.fully_loaded_item_slots.insert(i);
            }
            for i in 0..self.character_templates.len() {
                self.fully_loaded_char_slots.insert(i);
            }
        }

        if matches!(
            self.view_mode,
            ViewMode::ItemTemplates | ViewMode::CharacterTemplates
        ) {
            if !self.item_templates.is_empty() {
                self.view_mode = ViewMode::ItemTemplates;
            } else if !self.character_templates.is_empty() {
                self.view_mode = ViewMode::CharacterTemplates;
            }
        }
    }

    fn load_current_source(&mut self) {
        if matches!(self.data_source, DataSource::NotLoaded) {
            return;
        }

        self.load_error = None;
        self.save_status = None;

        match load_world_snapshot(&self.data_source) {
            Ok(world) => {
                let status = if let Some(path) = self.data_source.snapshot_path() {
                    format!("Loaded snapshot: {}", path.display())
                } else {
                    // LiveApi: status text is set by connect_to_api_from_form;
                    // return an empty string so the toolbar shows nothing here.
                    String::new()
                };

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
                self.dirty = false;
                self.dirty_item_template_slots.clear();
                self.dirty_character_template_slots.clear();
                self.dirty_item_slots.clear();
                self.dirty_character_slots.clear();
                self.save_status = Some(format!("Saved snapshot: {}", path.display()));
            }
            Err(e) => {
                self.save_status = Some(format!("Save failed: {e}"));
            }
        }
    }

    /// Push every dirty template slot to the admin API and clear the dirty
    /// sets. Called instead of snapshot save in LiveApi mode.
    fn save_to_api(&mut self) {
        self.save_status = None;
        if let Err(e) = self.sync_loaded_world_from_views() {
            self.save_status = Some(format!("Save failed: {e}"));
            return;
        }
        let Some(client) = self.admin_client.as_ref().cloned() else {
            self.save_status = Some("Admin client not initialized".to_owned());
            return;
        };

        let item_slots: Vec<usize> = self.dirty_item_template_slots.iter().copied().collect();
        let char_slots: Vec<usize> = self
            .dirty_character_template_slots
            .iter()
            .copied()
            .collect();
        let item_inst_slots: Vec<usize> = self.dirty_item_slots.iter().copied().collect();
        let char_inst_slots: Vec<usize> = self.dirty_character_slots.iter().copied().collect();

        let mut errors: Vec<String> = Vec::new();
        let mut item_pushed = 0usize;
        let mut char_pushed = 0usize;
        let mut item_inst_pushed = 0usize;
        let mut char_inst_pushed = 0usize;

        for idx in &item_slots {
            if let Some(item) = self.item_templates.get(*idx) {
                match client.put_item_template(*idx, item) {
                    Ok(()) => item_pushed += 1,
                    Err(e) => errors.push(format!("item[{idx}]: {e}")),
                }
            }
        }
        for idx in &char_slots {
            if let Some(ch) = self.character_templates.get(*idx) {
                match client.put_character_template(*idx, ch) {
                    Ok(()) => char_pushed += 1,
                    Err(e) => errors.push(format!("char[{idx}]: {e}")),
                }
            }
        }
        for idx in &item_inst_slots {
            if let Some(item) = self.items.get(*idx) {
                let patch = mag_core::item_store::ItemPatch::from_item(*idx, item);
                match client.put_item_patch(*idx, &patch) {
                    Ok(_) => item_inst_pushed += 1,
                    Err(e) => errors.push(format!("item_inst[{idx}]: {e}")),
                }
            }
        }
        for idx in &char_inst_slots {
            if let Some(ch) = self.characters.get(*idx) {
                let patch = mag_core::character_store::CharacterPatch::from_character(*idx, ch);
                match client.put_character_patch(*idx, &patch) {
                    Ok(_) => char_inst_pushed += 1,
                    Err(e) => errors.push(format!("char_inst[{idx}]: {e}")),
                }
            }
        }

        if errors.is_empty() {
            self.dirty = false;
            self.dirty_item_template_slots.clear();
            self.dirty_character_template_slots.clear();
            self.dirty_item_slots.clear();
            self.dirty_character_slots.clear();
            self.save_status = Some(format!(
                "Saved to API: {item_pushed} item template(s), {char_pushed} character template(s), {item_inst_pushed} item(s), {char_inst_pushed} character(s). Use 'Reload server templates' to apply."
            ));
        } else {
            // Drop the slots we pushed successfully so retry only sends failed ones.
            for idx in &item_slots {
                if !errors.iter().any(|e| e.contains(&format!("item[{idx}]"))) {
                    self.dirty_item_template_slots.remove(idx);
                }
            }
            for idx in &char_slots {
                if !errors.iter().any(|e| e.contains(&format!("char[{idx}]"))) {
                    self.dirty_character_template_slots.remove(idx);
                }
            }
            for idx in &item_inst_slots {
                if !errors
                    .iter()
                    .any(|e| e.contains(&format!("item_inst[{idx}]")))
                {
                    self.dirty_item_slots.remove(idx);
                }
            }
            for idx in &char_inst_slots {
                if !errors
                    .iter()
                    .any(|e| e.contains(&format!("char_inst[{idx}]")))
                {
                    self.dirty_character_slots.remove(idx);
                }
            }
            self.save_status = Some(format!(
                "Save partial: {item_pushed} item tpl, {char_pushed} char tpl, {item_inst_pushed} items, {char_inst_pushed} chars; {} error(s): {}",
                errors.len(),
                errors.join("; ")
            ));
        }
    }

    /// Open the modal dialog used to connect to the admin API.
    ///
    /// Pre-fills the form with the current LiveApi credentials when one is
    /// active, otherwise falls back to `MAG_API_BASE_URL` /
    /// `MAG_ADMIN_API_TOKEN` env vars, then to safe local-dev defaults.
    fn open_connect_dialog(&mut self) {
        match &self.data_source {
            DataSource::LiveApi { base_url, token } => {
                self.connect_form_base_url = base_url.clone();
                self.connect_form_token = token.clone();
            }
            _ => {
                if self.connect_form_base_url.is_empty() {
                    self.connect_form_base_url = std::env::var("MAG_API_BASE_URL")
                        .unwrap_or_else(|_| "https://127.0.0.1:5554".to_owned());
                }
                if self.connect_form_token.is_empty() {
                    self.connect_form_token =
                        std::env::var("MAG_ADMIN_API_TOKEN").unwrap_or_default();
                }
            }
        }
        self.connect_dialog_error = None;
        self.connect_dialog_open = true;
    }

    /// Switch the data source to LiveApi using the values currently in the
    /// connect-dialog form, build the admin client, and reload the world.
    ///
    /// Closes the dialog only on success; on failure leaves it open with an
    /// inline error message so the user can correct the URL/token.
    fn connect_to_api_from_form(&mut self) {
        let base_url = self.connect_form_base_url.trim().to_owned();
        let token = self.connect_form_token.trim().to_owned();

        if base_url.is_empty() {
            self.connect_dialog_error = Some("Base URL is required".to_owned());
            return;
        }
        if token.is_empty() {
            self.connect_dialog_error = Some("Admin token is required".to_owned());
            return;
        }

        let client = match AdminClient::new(base_url.clone(), token.clone()) {
            Ok(c) => c,
            Err(e) => {
                self.connect_dialog_error = Some(format!("Build client failed: {e}"));
                return;
            }
        };

        self.admin_client = Some(client);
        self.data_source = DataSource::LiveApi {
            base_url: base_url.clone(),
            token,
        };
        self.load_current_source();

        if let Some(err) = self.load_error.clone() {
            // Roll back so the user can re-edit the form without confusion.
            self.connect_dialog_error = Some(format!("Connection test failed: {err}"));
            self.admin_client = None;
            return;
        }

        self.connect_dialog_open = false;
        self.connect_dialog_error = None;
        // The "Source:" label in the toolbar already shows the URL, so we
        // only need a short confirmation without repeating it.
        self.save_status = Some("Connected to admin API".to_owned());
    }

    /// Trigger a reload on the running server for both template kinds, plus
    /// items and characters. Status display tracks the templates request id.
    fn request_server_reload(&mut self) {
        let Some(client) = self.admin_client.as_ref().cloned() else {
            self.save_status = Some("Admin client not initialized".to_owned());
            return;
        };

        let mut extra: Vec<String> = Vec::new();
        if let Err(e) = client.request_items_reload() {
            extra.push(format!("items reload failed: {e}"));
        } else {
            extra.push("items".to_owned());
        }
        if let Err(e) = client.request_characters_reload() {
            extra.push(format!("characters reload failed: {e}"));
        } else {
            extra.push("characters".to_owned());
        }

        match client.request_reload(true, true) {
            Ok(resp) => {
                self.pending_reload_request_id = Some(resp.request_id.clone());
                self.pending_reload_since = Some(std::time::Instant::now());
                self.save_status = Some(format!(
                    "Reload requested ({}): kinds=[{}] + [{}]",
                    resp.request_id,
                    resp.kinds.join(", "),
                    extra.join(", ")
                ));
            }
            Err(e) => {
                self.save_status = Some(format!("Reload failed: {e}"));
            }
        }
    }

    /// Poll the most recent reload request once (best effort).
    fn poll_reload_status(&mut self) {
        let Some(request_id) = self.pending_reload_request_id.clone() else {
            return;
        };
        let Some(client) = self.admin_client.as_ref().cloned() else {
            return;
        };
        match client.reload_status(&request_id) {
            Ok(status) => {
                if status.status == "applied" {
                    self.pending_reload_request_id = None;
                    self.pending_reload_since = None;
                    self.save_status = Some(format!("Reload applied ({})", status.request_id));
                } else {
                    self.save_status =
                        Some(format!("Reload status ({request_id}): {}", status.status));
                }
            }
            Err(e) => {
                self.save_status = Some(format!("Reload status error: {e}"));
            }
        }
    }

    fn load_from_snapshot(&mut self, path: PathBuf) {
        self.data_source = DataSource::SnapshotFile(path);
        self.load_current_source();
    }

    /// Render the modal dialog used to enter admin API connection details.
    ///
    /// # Arguments
    ///
    /// * `ctx` - egui context used to host the modal window.
    fn render_connect_dialog(&mut self, ctx: &egui::Context) {
        if !self.connect_dialog_open {
            return;
        }

        let mut still_open = true;
        let mut apply_clicked = false;
        let mut cancel_clicked = false;

        egui::Window::new("Connect to Admin API")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .open(&mut still_open)
            .show(ctx, |ui| {
                ui.set_min_width(420.0);
                ui.label(
                    "Point the template viewer at a running API service. \
                     Use a local URL when developing, or your production URL.",
                );
                ui.add_space(6.0);

                egui::Grid::new("connect_dialog_grid")
                    .num_columns(2)
                    .spacing([8.0, 6.0])
                    .show(ui, |ui| {
                        ui.label("Base URL:");
                        ui.add(
                            egui::TextEdit::singleline(&mut self.connect_form_base_url)
                                .hint_text("https://127.0.0.1:5554")
                                .desired_width(280.0),
                        );
                        ui.end_row();

                        ui.label("Admin token:");
                        ui.horizontal(|ui| {
                            ui.add(
                                egui::TextEdit::singleline(&mut self.connect_form_token)
                                    .password(!self.connect_form_show_token)
                                    .hint_text("MAG_ADMIN_API_TOKEN")
                                    .desired_width(220.0),
                            );
                            ui.checkbox(&mut self.connect_form_show_token, "Show");
                        });
                        ui.end_row();
                    });

                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new(
                        "Defaults are read from MAG_API_BASE_URL and MAG_ADMIN_API_TOKEN.",
                    )
                    .small()
                    .weak(),
                );

                if let Some(err) = &self.connect_dialog_error {
                    ui.add_space(6.0);
                    ui.colored_label(egui::Color32::RED, err);
                }

                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if ui.button("Connect").clicked() {
                        apply_clicked = true;
                    }
                    if ui.button("Cancel").clicked() {
                        cancel_clicked = true;
                    }
                });
            });

        if cancel_clicked || !still_open {
            self.connect_dialog_open = false;
            self.connect_dialog_error = None;
        } else if apply_clicked {
            self.connect_to_api_from_form();
        }
    }

    /// Render the confirmation modal for triggering a server-side template
    /// reload. The reload only happens when the user explicitly confirms.
    ///
    /// # Arguments
    ///
    /// * `ctx` - egui context used to host the modal window.
    fn render_reload_confirm_dialog(&mut self, ctx: &egui::Context) {
        if !self.reload_confirm_open {
            return;
        }

        let mut still_open = true;
        let mut confirm_clicked = false;
        let mut cancel_clicked = false;
        let has_unsaved = self.dirty;

        egui::Window::new("Reload Server Templates?")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .open(&mut still_open)
            .show(ctx, |ui| {
                ui.set_min_width(440.0);
                ui.colored_label(
                    egui::Color32::YELLOW,
                    "\u{26A0}  This will swap the running server's in-memory template tables.",
                );
                ui.add_space(6.0);
                ui.label(
                    "Existing entities and ongoing player actions may be affected. \
                     Run this only when you have just pushed template edits and \
                     are ready to apply them live.",
                );

                if has_unsaved {
                    ui.add_space(6.0);
                    ui.colored_label(
                        egui::Color32::RED,
                        "You have unsaved local edits. Save to API first or they will not be reloaded.",
                    );
                }

                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if ui
                        .add(egui::Button::new(
                            egui::RichText::new("Reload now").color(egui::Color32::WHITE),
                        ).fill(egui::Color32::from_rgb(160, 60, 60)))
                        .clicked()
                    {
                        confirm_clicked = true;
                    }
                    if ui.button("Cancel").clicked() {
                        cancel_clicked = true;
                    }
                });
            });

        if cancel_clicked || !still_open {
            self.reload_confirm_open = false;
        } else if confirm_clicked {
            self.reload_confirm_open = false;
            self.request_server_reload();
        }
    }

    fn revert_unsaved_changes(&mut self) {
        self.save_status = None;

        let prev_view_mode = self.view_mode;
        let prev_selected_item_index = self.selected_item_index;
        let prev_selected_character_index = self.selected_character_index;
        let prev_selected_item_instance_index = self.selected_item_instance_index;
        let prev_selected_character_instance_index = self.selected_character_instance_index;

        self.load_current_source();

        // Restore view and selections where possible.
        self.view_mode = prev_view_mode;
        self.selected_item_index =
            prev_selected_item_index.filter(|&i| i < self.item_templates.len());
        self.selected_character_index =
            prev_selected_character_index.filter(|&i| i < self.character_templates.len());
        self.selected_item_instance_index =
            prev_selected_item_instance_index.filter(|&i| i < self.items.len());
        self.selected_character_instance_index =
            prev_selected_character_instance_index.filter(|&i| i < self.characters.len());

        self.dirty = false;
        self.dirty_item_template_slots.clear();
        self.dirty_character_template_slots.clear();
        self.dirty_item_slots.clear();
        self.dirty_character_slots.clear();
        if self.load_error.is_some() {
            self.save_status = Some("Reverted changes (with load errors)".to_owned());
        } else {
            self.save_status = Some("Reverted unsaved changes".to_owned());
        }
    }

    fn mark_dirty_if(&mut self, changed: bool) {
        if changed {
            self.dirty = true;
            // Track per-slot dirty bits so LiveApi save can PUT only the
            // slots the user actually edited.
            match self.view_mode {
                ViewMode::ItemTemplates => {
                    if let Some(idx) = self.selected_item_index {
                        self.dirty_item_template_slots.insert(idx);
                    }
                }
                ViewMode::CharacterTemplates => {
                    if let Some(idx) = self.selected_character_index {
                        self.dirty_character_template_slots.insert(idx);
                    }
                }
                ViewMode::Items => {
                    if let Some(idx) = self.selected_item_instance_index {
                        self.dirty_item_slots.insert(idx);
                    }
                }
                ViewMode::Characters => {
                    if let Some(idx) = self.selected_character_instance_index {
                        self.dirty_character_slots.insert(idx);
                    }
                }
            }
        }
    }

    fn clamp_i8(v: i32) -> i8 {
        v.clamp(i32::from(i8::MIN), i32::from(i8::MAX)) as i8
    }

    fn clamp_u8(v: i32) -> u8 {
        v.clamp(i32::from(u8::MIN), i32::from(u8::MAX)) as u8
    }

    fn clamp_i16(v: i32) -> i16 {
        v.clamp(i32::from(i16::MIN), i32::from(i16::MAX)) as i16
    }

    fn clamp_u16(v: i32) -> u16 {
        v.clamp(i32::from(u16::MIN), i32::from(u16::MAX)) as u16
    }

    fn load_graphics_zip(&mut self, zip_path: PathBuf) {
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
            Ok(None) => {
                crate::centered_label(ui, format!("{}", sprite_id));
            }
            Err(e) => {
                self.graphics_zip_error = Some(e);
                crate::centered_label(ui, format!("{}", sprite_id));
            }
        }
    }

    fn find_item_template_index(&self, item_id: u32) -> Option<usize> {
        let index = item_id as usize;
        if index < self.item_templates.len() {
            return Some(index);
        }

        let temp_id = item_id as u16;
        self.item_templates
            .iter()
            .position(|item| item.temp == temp_id)
    }

    fn render_item_details_by_index(
        &mut self,
        ui: &mut egui::Ui,
        source: ItemDetailsSource,
        idx: usize,
    ) {
        // In LiveApi mode, item templates are populated with summary-only stubs
        // on connect. Fetch the full bincode payload the first time a slot is
        // selected (lazy load, 1 request per unique slot).
        if source == ItemDetailsSource::ItemTemplates
            && self.data_source.is_live_api()
            && !self.fully_loaded_item_slots.contains(&idx)
            && let Some(client) = self.admin_client.as_ref().cloned()
        {
            match client.fetch_single_item_template(idx) {
                Ok(item) => {
                    if idx < self.item_templates.len() {
                        self.item_templates[idx] = item;
                    }
                    self.fully_loaded_item_slots.insert(idx);
                }
                Err(e) => {
                    ui.colored_label(
                        egui::Color32::RED,
                        format!("Failed to load item template {idx}: {e}"),
                    );
                    return;
                }
            }
        }
        let item_ptr: *mut mag_core::types::Item = match source {
            ItemDetailsSource::ItemTemplates => {
                if idx >= self.item_templates.len() {
                    return;
                }
                unsafe { self.item_templates.as_mut_ptr().add(idx) }
            }
            ItemDetailsSource::Items => {
                if idx >= self.items.len() {
                    return;
                }
                unsafe { self.items.as_mut_ptr().add(idx) }
            }
        };

        // SAFETY: `item_ptr` points into `self` and is valid for the duration
        // of this call. We don't reallocate the backing Vec while rendering.
        unsafe {
            self.render_item_details(ui, &mut *item_ptr);
        }
    }

    fn render_character_details_by_index(
        &mut self,
        ui: &mut egui::Ui,
        source: CharacterDetailsSource,
        idx: usize,
    ) {
        // Same lazy-fetch as for item templates — only one request per slot.
        if source == CharacterDetailsSource::CharacterTemplates
            && self.data_source.is_live_api()
            && !self.fully_loaded_char_slots.contains(&idx)
            && let Some(client) = self.admin_client.as_ref().cloned()
        {
            match client.fetch_single_character_template(idx) {
                Ok(ch) => {
                    if idx < self.character_templates.len() {
                        self.character_templates[idx] = ch;
                    }
                    self.fully_loaded_char_slots.insert(idx);
                }
                Err(e) => {
                    ui.colored_label(
                        egui::Color32::RED,
                        format!("Failed to load character template {idx}: {e}"),
                    );
                    return;
                }
            }
        }
        let character_ptr: *mut mag_core::types::Character = match source {
            CharacterDetailsSource::CharacterTemplates => {
                if idx >= self.character_templates.len() {
                    return;
                }
                unsafe { self.character_templates.as_mut_ptr().add(idx) }
            }
            CharacterDetailsSource::Characters => {
                if idx >= self.characters.len() {
                    return;
                }
                unsafe { self.characters.as_mut_ptr().add(idx) }
            }
        };

        // SAFETY: `character_ptr` points into `self` and remains valid for the
        // duration of this call.
        unsafe {
            self.render_character_details(ui, &mut *character_ptr);
        }
    }

    fn render_item_popup(&mut self, ctx: &egui::Context) {
        let Some(item_id) = self.item_popup_id else {
            return;
        };

        let mut open = true;
        egui::Window::new(format!("Item {}", item_id))
            .open(&mut open)
            .show(ctx, |ui| {
                if let Some(idx) = self.find_item_template_index(item_id) {
                    self.render_item_details_by_index(ui, ItemDetailsSource::ItemTemplates, idx);
                } else {
                    ui.label(format!("No item template found for ID {}", item_id));
                }
            });

        if !open {
            self.item_popup_id = None;
        }
    }

    fn centered_clickable_item_instance_id(&mut self, ui: &mut egui::Ui, item_id: u32) {
        if item_id == 0 {
            crate::centered_label(ui, "0");
            return;
        }

        let response = ui
            .with_layout(
                egui::Layout::centered_and_justified(egui::Direction::LeftToRight),
                |ui| ui.add(egui::Label::new(format!("{}", item_id)).sense(egui::Sense::click())),
            )
            .inner;

        if response.clicked() {
            let idx = item_id as usize;
            if idx < self.items.len() && self.items[idx].used != mag_core::constants::USE_EMPTY {
                self.selected_item_instance_index = Some(idx);
                self.view_mode = ViewMode::Items;
            }
        }
    }

    fn render_item_list(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("Filter:");
            ui.text_edit_singleline(&mut self.item_filter);
        });

        ui.horizontal(|ui| {
            ui.checkbox(&mut self.show_unused_templates, "Show unused");
        });

        ui.separator();

        let list_width = ui.available_width();
        egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                ui.set_min_width(list_width);
                for (idx, item) in self.item_templates.iter().enumerate() {
                    if !self.show_unused_templates && item.used == mag_core::constants::USE_EMPTY {
                        continue;
                    }

                    let name = item.get_name();
                    if !self.item_filter.is_empty()
                        && !name
                            .to_lowercase()
                            .contains(&self.item_filter.to_lowercase())
                    {
                        continue;
                    }

                    if ui
                        .selectable_label(
                            self.selected_item_index == Some(idx),
                            format!("[{}] {}", idx, name),
                        )
                        .clicked()
                    {
                        self.selected_item_index = Some(idx);
                    }
                }
            });
    }

    fn render_item_instance_list(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("Filter:");
            ui.text_edit_singleline(&mut self.item_instance_filter);
        });

        ui.separator();

        let list_width = ui.available_width();
        egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                ui.set_min_width(list_width);
                for (idx, item) in self.items.iter().enumerate() {
                    if item.used == mag_core::constants::USE_EMPTY {
                        continue;
                    }

                    let name = item.get_name();
                    if !self.item_instance_filter.is_empty()
                        && !name
                            .to_lowercase()
                            .contains(&self.item_instance_filter.to_lowercase())
                    {
                        continue;
                    }

                    if ui
                        .selectable_label(
                            self.selected_item_instance_index == Some(idx),
                            format!("[{}] {}", idx, name),
                        )
                        .clicked()
                    {
                        self.selected_item_instance_index = Some(idx);
                    }
                }
            });
    }

    fn render_character_list(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("Filter:");
            ui.text_edit_singleline(&mut self.character_filter);
        });

        ui.horizontal(|ui| {
            ui.checkbox(&mut self.show_unused_templates, "Show unused");
        });

        ui.separator();

        let list_width = ui.available_width();
        egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                ui.set_min_width(list_width);
                for (idx, character) in self.character_templates.iter().enumerate() {
                    if !self.show_unused_templates
                        && character.used == mag_core::constants::USE_EMPTY
                    {
                        continue;
                    }

                    let name = character.get_name();
                    if !self.character_filter.is_empty()
                        && !name
                            .to_lowercase()
                            .contains(&self.character_filter.to_lowercase())
                    {
                        continue;
                    }

                    if ui
                        .selectable_label(
                            self.selected_character_index == Some(idx),
                            format!("[{}] {}", idx, name),
                        )
                        .clicked()
                    {
                        self.selected_character_index = Some(idx);
                    }
                }
            });
    }

    fn render_character_instance_list(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("Filter:");
            ui.text_edit_singleline(&mut self.character_instance_filter);
        });

        ui.horizontal(|ui| {
            ui.checkbox(&mut self.character_instances_player_only, "Player only");
        });

        ui.separator();

        let list_width = ui.available_width();
        egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                ui.set_min_width(list_width);
                for (idx, character) in self.characters.iter().enumerate() {
                    if character.used == mag_core::constants::USE_EMPTY {
                        continue;
                    }

                    if self.character_instances_player_only {
                        // "Player-held" character slots: those that are actual player characters.
                        // This is the most stable indicator across save states.
                        if (character.flags & mag_core::constants::CharacterFlags::Player.bits())
                            == 0
                        {
                            continue;
                        }
                    }

                    let name = character.get_name();
                    if !self.character_instance_filter.is_empty()
                        && !name
                            .to_lowercase()
                            .contains(&self.character_instance_filter.to_lowercase())
                    {
                        continue;
                    }

                    if ui
                        .selectable_label(
                            self.selected_character_instance_index == Some(idx),
                            format!("[{}] {}", idx, name),
                        )
                        .clicked()
                    {
                        self.selected_character_instance_index = Some(idx);
                    }
                }
            });
    }

    #[allow(clippy::needless_range_loop)]
    fn render_item_details(&mut self, ui: &mut egui::Ui, item: &mut mag_core::types::Item) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.heading(item.get_name());
            ui.separator();

            // Copy all fields to avoid packed struct issues
            let mut temp = i32::from(item.temp);
            let mut used = i32::from(item.used);
            let mut name_buf = item.name;
            let mut reference_buf = item.reference;
            let mut description_buf = item.description;
            let mut name = c_string_to_str(&name_buf).to_owned();
            let mut reference = c_string_to_str(&reference_buf).to_owned();
            let mut description = c_string_to_str(&description_buf).to_owned();

            let mut value = item.value;
            let mut placement = item.placement;
            let flags = item.flags;
            let mut sprite_0 = i32::from(item.sprite[0]);
            let mut sprite_1 = i32::from(item.sprite[1]);
            let mut status_0 = i32::from(item.status[0]);
            let mut status_1 = i32::from(item.status[1]);
            let mut armor_0 = i32::from(item.armor[0]);
            let mut armor_1 = i32::from(item.armor[1]);
            let mut weapon_0 = i32::from(item.weapon[0]);
            let mut weapon_1 = i32::from(item.weapon[1]);
            let mut light_0 = i32::from(item.light[0]);
            let mut light_1 = i32::from(item.light[1]);
            let mut duration = item.duration;
            let mut cost = item.cost;
            let mut power = item.power;
            let mut min_rank = i32::from(item.min_rank);
            let mut driver = i32::from(item.driver);

            let mut attrib = item.attrib;
            let mut hp = item.hp;
            let mut end = item.end;
            let mut mana = item.mana;
            let mut skill = item.skill;
            let mut data = item.data;

            let mut changed = false;

            egui::Grid::new("item_details")
                .num_columns(2)
                .spacing([40.0, 4.0])
                .striped(true)
                .show(ui, |ui| {
                    ui.label("Index:");
                    changed |= ui.add(egui::DragValue::new(&mut temp).speed(1)).changed();
                    ui.end_row();

                    ui.label("Used:");
                    changed |= ui.add(egui::DragValue::new(&mut used).speed(1)).changed();
                    ui.end_row();

                    ui.label("Name:");
                    changed |= ui
                        .add(egui::TextEdit::singleline(&mut name).desired_width(240.0))
                        .changed();
                    ui.end_row();

                    ui.label("Reference:");
                    changed |= ui
                        .add(egui::TextEdit::singleline(&mut reference).desired_width(240.0))
                        .changed();
                    ui.end_row();

                    ui.label("Description:");
                    changed |= ui
                        .add(
                            egui::TextEdit::multiline(&mut description)
                                .desired_width(240.0)
                                .desired_rows(3),
                        )
                        .changed();
                    ui.end_row();

                    ui.label("Value:");
                    ui.horizontal(|ui| {
                        changed |= ui.add(egui::DragValue::new(&mut value).speed(1)).changed();
                        ui.label(crate::format_gold_silver(value as i32));
                    });
                    ui.end_row();

                    ui.label("Placement:");
                    egui::ComboBox::from_id_salt(format!("placement_combo_{}", temp))
                        .selected_text(crate::placement_label(placement))
                        .show_ui(ui, |ui| {
                            for (value, name) in crate::placement_options() {
                                if ui.selectable_label(*value == placement, *name).clicked() {
                                    placement = *value;
                                    changed = true;
                                }
                            }
                        });
                    ui.end_row();

                    ui.label("Flags:");
                    ui.end_row();
                });

            let mut item_flags = mag_core::constants::ItemFlags::from_bits_truncate(flags);
            egui::Grid::new(format!("item_flags_grid_{}", temp))
                .num_columns(3)
                .spacing([10.0, 4.0])
                .striped(true)
                .show(ui, |ui| {
                    let mut col = 0;
                    for (flag, name) in crate::get_item_flag_info() {
                        let mut is_set = item_flags.contains(flag);
                        if ui.checkbox(&mut is_set, name).changed() {
                            if is_set {
                                item_flags.insert(flag);
                            } else {
                                item_flags.remove(flag);
                            }
                            changed = true;
                        }
                        col += 1;
                        if col == 3 {
                            ui.end_row();
                            col = 0;
                        }
                    }
                    if col != 0 {
                        ui.end_row();
                    }
                });

            egui::Grid::new(format!("item_details_grid2_{}", temp))
                .num_columns(2)
                .spacing([40.0, 4.0])
                .striped(true)
                .show(ui, |ui| {
                    ui.label("Sprite[0]:");
                    ui.vertical(|ui| {
                        changed |= ui
                            .add(egui::DragValue::new(&mut sprite_0).speed(1))
                            .changed();
                        self.sprite_cell(ui, sprite_0.max(0) as usize);
                    });
                    ui.end_row();

                    ui.label("Sprite[1]:");
                    ui.vertical(|ui| {
                        changed |= ui
                            .add(egui::DragValue::new(&mut sprite_1).speed(1))
                            .changed();
                        self.sprite_cell(ui, sprite_1.max(0) as usize);
                    });
                    ui.end_row();

                    ui.label("Status:");
                    ui.horizontal(|ui| {
                        changed |= ui
                            .add(egui::DragValue::new(&mut status_0).speed(1))
                            .changed();
                        changed |= ui
                            .add(egui::DragValue::new(&mut status_1).speed(1))
                            .changed();
                    });
                    ui.end_row();

                    ui.label("Armor:");
                    ui.horizontal(|ui| {
                        changed |= ui
                            .add(egui::DragValue::new(&mut armor_0).speed(1))
                            .changed();
                        changed |= ui
                            .add(egui::DragValue::new(&mut armor_1).speed(1))
                            .changed();
                    });
                    ui.end_row();

                    ui.label("Weapon:");
                    ui.horizontal(|ui| {
                        changed |= ui
                            .add(egui::DragValue::new(&mut weapon_0).speed(1))
                            .changed();
                        changed |= ui
                            .add(egui::DragValue::new(&mut weapon_1).speed(1))
                            .changed();
                    });
                    ui.end_row();

                    ui.label("Light:");
                    ui.horizontal(|ui| {
                        changed |= ui
                            .add(egui::DragValue::new(&mut light_0).speed(1))
                            .changed();
                        changed |= ui
                            .add(egui::DragValue::new(&mut light_1).speed(1))
                            .changed();
                    });
                    ui.end_row();

                    ui.label("Duration:");
                    changed |= ui
                        .add(egui::DragValue::new(&mut duration).speed(1))
                        .changed();
                    ui.end_row();

                    ui.label("Cost:");
                    changed |= ui.add(egui::DragValue::new(&mut cost).speed(1)).changed();
                    ui.end_row();

                    ui.label("Power:");
                    changed |= ui.add(egui::DragValue::new(&mut power).speed(1)).changed();
                    ui.end_row();

                    ui.label("Min Rank:");
                    egui::ComboBox::from_id_salt(format!("min_rank_combo_{}", temp))
                        .selected_text(crate::rank_label(min_rank as i8))
                        .show_ui(ui, |ui| {
                            if ui.selectable_label(min_rank < 0, "-1: None").clicked() {
                                min_rank = -1;
                                changed = true;
                            }
                            for (idx, name) in ranks::ranks().iter().enumerate() {
                                let label = format!("{}: {}", idx, name);
                                if ui.selectable_label(min_rank == idx as i32, label).clicked() {
                                    min_rank = idx as i32;
                                    changed = true;
                                }
                            }
                        });
                    ui.end_row();

                    ui.label("Driver:");
                    changed |= ui.add(egui::DragValue::new(&mut driver).speed(1)).changed();
                    ui.end_row();
                });

            ui.separator();
            crate::centered_heading(ui, "Attributes");
            egui::Grid::new("item_attributes")
                .num_columns(4)
                .spacing([20.0, 4.0])
                .striped(true)
                .show(ui, |ui| {
                    ui.label("Stat");
                    ui.label("Worn");
                    ui.label("Active");
                    ui.label("Min Required");
                    ui.end_row();

                    let attrib_names = ["Bravery", "Willpower", "Intuition", "Agility", "Strength"];
                    for (i, name) in attrib_names.iter().enumerate() {
                        ui.label(*name);
                        for j in 0..3 {
                            let mut v = i32::from(attrib[i][j]);
                            if ui.add(egui::DragValue::new(&mut v).speed(1)).changed() {
                                attrib[i][j] = Self::clamp_i8(v);
                                changed = true;
                            }
                        }
                        ui.end_row();
                    }

                    ui.label("HP");
                    for j in 0..3 {
                        let mut v = i32::from(hp[j]);
                        if ui.add(egui::DragValue::new(&mut v).speed(1)).changed() {
                            hp[j] = Self::clamp_i16(v);
                            changed = true;
                        }
                    }
                    ui.end_row();

                    ui.label("Endurance");
                    for j in 0..3 {
                        let mut v = i32::from(end[j]);
                        if ui.add(egui::DragValue::new(&mut v).speed(1)).changed() {
                            end[j] = Self::clamp_i16(v);
                            changed = true;
                        }
                    }
                    ui.end_row();

                    ui.label("Mana");
                    for j in 0..3 {
                        let mut v = i32::from(mana[j]);
                        if ui.add(egui::DragValue::new(&mut v).speed(1)).changed() {
                            mana[j] = Self::clamp_i16(v);
                            changed = true;
                        }
                    }
                    ui.end_row();
                });

            ui.separator();
            crate::centered_heading(ui, "Skills");
            egui::Grid::new("item_skills")
                .num_columns(5)
                .spacing([20.0, 4.0])
                .striped(true)
                .show(ui, |ui| {
                    ui.label("Skill #");
                    ui.label("Skill Name");
                    ui.label("Worn");
                    ui.label("Active");
                    ui.label("Min Required");
                    ui.end_row();

                    for i in 0..skill.len() {
                        crate::centered_label(ui, format!("{}", i));
                        ui.label(skills::get_skill_name(i));
                        for j in 0..3 {
                            let mut v = i32::from(skill[i][j]);
                            if ui.add(egui::DragValue::new(&mut v).speed(1)).changed() {
                                skill[i][j] = Self::clamp_i8(v);
                                changed = true;
                            }
                        }
                        ui.end_row();
                    }
                });

            ui.separator();
            crate::centered_heading(ui, "Driver Data");
            ui.horizontal(|ui| {
                ui.checkbox(
                    &mut self.show_all_data_fields,
                    "Show all possible data fields",
                );
            });
            egui::Grid::new("item_driver_data")
                .num_columns(2)
                .spacing([40.0, 4.0])
                .striped(true)
                .show(ui, |ui| {
                    let mut shown_any = false;
                    for i in 0..10 {
                        if !self.show_all_data_fields && data[i] == 0 {
                            continue;
                        }

                        shown_any = true;
                        ui.label(format!("data[{}]:", i));
                        changed |= ui
                            .add(egui::DragValue::new(&mut data[i]).speed(1))
                            .changed();
                        ui.end_row();
                    }

                    if !shown_any {
                        let i = 0;
                        ui.label(format!("data[{}]:", i));
                        changed |= ui
                            .add(egui::DragValue::new(&mut data[i]).speed(1))
                            .changed();
                        ui.end_row();
                    }
                });

            // Commit edits back into the packed struct
            item.temp = Self::clamp_u16(temp);
            item.used = Self::clamp_u8(used);

            crate::write_c_string(&mut name_buf, &name);
            crate::write_c_string(&mut reference_buf, &reference);
            crate::write_c_string(&mut description_buf, &description);
            item.name = name_buf;
            item.reference = reference_buf;
            item.description = description_buf;

            item.value = value;
            item.placement = placement;
            item.flags = item_flags.bits();
            item.sprite[0] = Self::clamp_i16(sprite_0);
            item.sprite[1] = Self::clamp_i16(sprite_1);
            item.status[0] = Self::clamp_u8(status_0);
            item.status[1] = Self::clamp_u8(status_1);
            item.armor[0] = Self::clamp_i8(armor_0);
            item.armor[1] = Self::clamp_i8(armor_1);
            item.weapon[0] = Self::clamp_i8(weapon_0);
            item.weapon[1] = Self::clamp_i8(weapon_1);
            item.light[0] = Self::clamp_i16(light_0);
            item.light[1] = Self::clamp_i16(light_1);
            item.duration = duration;
            item.cost = cost;
            item.power = power;
            item.min_rank = Self::clamp_i8(min_rank);
            item.driver = Self::clamp_u8(driver);
            item.attrib = attrib;
            item.hp = hp;
            item.end = end;
            item.mana = mana;
            item.skill = skill;
            item.data = data;

            self.mark_dirty_if(changed);

            if self.view_mode == ViewMode::ItemTemplates {
                ui.separator();

                // For templates, the *slot index* is the template id. The `temp` field inside
                // stored template entries is not reliable for this purpose.
                let temp_u16 = Self::clamp_u16(temp);
                let template_id = self
                    .selected_item_index
                    .map(|idx| idx as u16)
                    .unwrap_or(temp_u16);
                let tile_w = mag_core::constants::SERVER_MAPX as usize;
                let mut locations: Vec<(u32, u16, u16, String)> = Vec::new();

                for (tile_idx, tile) in self.map_tiles.iter().enumerate() {
                    let item_id = tile.it;
                    if item_id == 0 {
                        continue;
                    }

                    let item_idx = item_id as usize;
                    if item_idx >= self.items.len() {
                        continue;
                    }

                    let it = self.items[item_idx];
                    if it.used == mag_core::constants::USE_EMPTY {
                        continue;
                    }

                    if it.temp != template_id {
                        continue;
                    }

                    let x = (tile_idx % tile_w) as u16;
                    let y = (tile_idx / tile_w) as u16;
                    let area = mag_core::area::get_area_m(i32::from(x), i32::from(y))
                        .unwrap_or_else(|| "Unknown".to_owned());
                    locations.push((item_id, x, y, area));
                }

                locations.sort_by(|a, b| a.3.cmp(&b.3).then(a.2.cmp(&b.2)).then(a.1.cmp(&b.1)));

                let total = locations.len();
                let limit: usize = 500;

                egui::CollapsingHeader::new(format!("Where used ({} on map)", total))
                    .default_open(total > 0 && total <= 20)
                    .show(ui, |ui| {
                        if total == 0 {
                            ui.label("No instances of this template were found on the map.");
                            return;
                        }

                        if total > limit {
                            ui.colored_label(
                                egui::Color32::YELLOW,
                                format!("Showing first {} of {} results", limit, total),
                            );
                        }

                        egui::Grid::new(format!("item_where_used_{}", template_id))
                            .num_columns(4)
                            .spacing([20.0, 4.0])
                            .striped(true)
                            .show(ui, |ui| {
                                crate::centered_label(ui, "Item");
                                crate::centered_label(ui, "X");
                                crate::centered_label(ui, "Y");
                                ui.label("Area");
                                ui.end_row();

                                for (item_id, x, y, area) in locations.iter().take(limit) {
                                    self.centered_clickable_item_instance_id(ui, *item_id);
                                    crate::centered_label(ui, format!("{}", x));
                                    crate::centered_label(ui, format!("{}", y));
                                    ui.label(area);
                                    ui.end_row();
                                }
                            });
                    });
            }
        });
    }

    #[allow(clippy::needless_range_loop)]
    fn render_character_details(
        &mut self,
        ui: &mut egui::Ui,
        character: &mut mag_core::types::Character,
    ) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.heading(character.get_name());
            ui.separator();

            // Copy all packed fields to avoid alignment issues
            let mut temp = i32::from(character.temp);
            let mut used = i32::from(character.used);
            let mut name_buf = character.name;
            let mut reference_buf = character.reference;
            let mut description_buf = character.description;
            let mut name = c_string_to_str(&name_buf).to_owned();
            let mut reference = c_string_to_str(&reference_buf).to_owned();
            let mut description = c_string_to_str(&description_buf).to_owned();

            let mut kindred = character.kindred;
            let mut sprite = i32::from(character.sprite);
            let mut sound = i32::from(character.sound);
            let flags = character.flags;
            let mut alignment = i32::from(character.alignment);
            let mut temple_x = i32::from(character.temple_x);
            let mut temple_y = i32::from(character.temple_y);
            let mut tavern_x = i32::from(character.tavern_x);
            let mut tavern_y = i32::from(character.tavern_y);
            let mut x = i32::from(character.x);
            let mut y = i32::from(character.y);
            let mut gold = character.gold;
            let mut points = character.points;
            let mut points_tot = character.points_tot;
            let mut armor = i32::from(character.armor);
            let mut weapon = i32::from(character.weapon);
            let mut light = i32::from(character.light);
            let mut armor_bonus = i32::from(character.armor_bonus);
            let mut weapon_bonus = i32::from(character.weapon_bonus);
            let mut light_bonus = i32::from(character.light_bonus);
            let mut gethit_bonus = i32::from(character.gethit_bonus);
            let mut mode = i32::from(character.mode);
            let mut speed = i32::from(character.speed);
            let mut speed_mod_val = i32::from(character.speed_mod);
            let mut monster_class = character.monster_class;
            let mut a_hp = character.a_hp;
            let mut a_end = character.a_end;
            let mut a_mana = character.a_mana;

            let mut attrib = character.attrib;
            let mut hp = character.hp;
            let mut end = character.end;
            let mut mana = character.mana;
            let mut skills = character.skill;
            let mut inventory = character.item;
            let mut worn = character.worn;
            let mut data = character.data;

            let mut changed = false;

            egui::Grid::new("character_details")
                .num_columns(2)
                .spacing([40.0, 4.0])
                .striped(true)
                .show(ui, |ui| {
                    ui.label("Index:");
                    changed |= ui.add(egui::DragValue::new(&mut temp).speed(1)).changed();
                    ui.end_row();

                    ui.label("Used:");
                    changed |= ui.add(egui::DragValue::new(&mut used).speed(1)).changed();
                    ui.end_row();

                    ui.label("Name:");
                    changed |= ui
                        .add(egui::TextEdit::singleline(&mut name).desired_width(240.0))
                        .changed();
                    ui.end_row();

                    ui.label("Reference:");
                    changed |= ui
                        .add(egui::TextEdit::singleline(&mut reference).desired_width(240.0))
                        .changed();
                    ui.end_row();

                    ui.label("Description:");
                    changed |= ui
                        .add(
                            egui::TextEdit::multiline(&mut description)
                                .desired_width(240.0)
                                .desired_rows(3),
                        )
                        .changed();
                    ui.end_row();

                    ui.label("Kindred:");
                    ui.vertical(|ui| {
                        ui.label(format!("{} (0x{:08X})", kindred, kindred as u32));
                        changed |= ui
                            .add(egui::DragValue::new(&mut kindred).speed(1))
                            .changed();

                        if kindred < 0 {
                            kindred = 0;
                            changed = true;
                        }

                        ui.separator();

                        let mut kindred_bits = kindred as u32;
                        let kindred_flags: [(u32, &str); 12] = [
                            (traits::KIN_MERCENARY, "Mercenary"),
                            (traits::KIN_SEYAN_DU, "Seyan Du"),
                            (traits::KIN_PURPLE, "Purple"),
                            (traits::KIN_MONSTER, "Monster"),
                            (traits::KIN_TEMPLAR, "Templar"),
                            (traits::KIN_ARCHTEMPLAR, "ArchTemplar"),
                            (traits::KIN_HARAKIM, "Harakim"),
                            (traits::KIN_MALE, "Male"),
                            (traits::KIN_FEMALE, "Female"),
                            (traits::KIN_ARCHHARAKIM, "ArchHarakim"),
                            (traits::KIN_WARRIOR, "Warrior"),
                            (traits::KIN_SORCERER, "Sorcerer"),
                        ];

                        egui::Grid::new(format!("kindred_flags_grid_{}", temp))
                            .num_columns(2)
                            .spacing([10.0, 4.0])
                            .striped(false)
                            .show(ui, |ui| {
                                let mut col = 0;
                                for (bit, name) in kindred_flags {
                                    let mut is_set = (kindred_bits & bit) != 0;
                                    if ui.checkbox(&mut is_set, name).changed() {
                                        if is_set {
                                            kindred_bits |= bit;
                                        } else {
                                            kindred_bits &= !bit;
                                        }
                                        changed = true;
                                    }

                                    col += 1;
                                    if col == 2 {
                                        ui.end_row();
                                        col = 0;
                                    }
                                }
                                if col != 0 {
                                    ui.end_row();
                                }
                            });

                        if kindred_bits != kindred as u32 {
                            kindred = kindred_bits as i32;
                            changed = true;
                        }
                    });
                    ui.end_row();

                    ui.label("Sprite:");
                    ui.vertical(|ui| {
                        changed |= ui.add(egui::DragValue::new(&mut sprite).speed(1)).changed();
                        self.sprite_cell(ui, sprite.max(0) as usize);
                    });
                    ui.end_row();

                    ui.label("Sound:");
                    changed |= ui.add(egui::DragValue::new(&mut sound).speed(1)).changed();
                    ui.end_row();

                    ui.label("Flags:");
                    ui.end_row();
                });

            let mut character_flags =
                mag_core::constants::CharacterFlags::from_bits_truncate(flags);
            egui::Grid::new(format!("character_flags_grid_{}", temp))
                .num_columns(3)
                .spacing([10.0, 4.0])
                .striped(true)
                .show(ui, |ui| {
                    let mut col = 0;
                    for flag in crate::get_character_flag_info() {
                        let mut is_set = character_flags.contains(flag);
                        if ui
                            .checkbox(&mut is_set, mag_core::constants::character_flags_name(flag))
                            .changed()
                        {
                            if is_set {
                                character_flags.insert(flag);
                            } else {
                                character_flags.remove(flag);
                            }
                            changed = true;
                        }
                        col += 1;
                        if col == 3 {
                            ui.end_row();
                            col = 0;
                        }
                    }
                    if col != 0 {
                        ui.end_row();
                    }
                });

            egui::Grid::new(format!("character_details_grid2_{}", temp))
                .num_columns(2)
                .spacing([40.0, 4.0])
                .striped(true)
                .show(ui, |ui| {
                    ui.label("Alignment:");
                    changed |= ui
                        .add(egui::DragValue::new(&mut alignment).speed(1))
                        .changed();
                    ui.end_row();

                    ui.label("Temple:");
                    ui.horizontal(|ui| {
                        changed |= ui
                            .add(egui::DragValue::new(&mut temple_x).speed(1))
                            .changed();
                        changed |= ui
                            .add(egui::DragValue::new(&mut temple_y).speed(1))
                            .changed();
                    });
                    ui.end_row();

                    ui.label("Tavern:");
                    ui.horizontal(|ui| {
                        changed |= ui
                            .add(egui::DragValue::new(&mut tavern_x).speed(1))
                            .changed();
                        changed |= ui
                            .add(egui::DragValue::new(&mut tavern_y).speed(1))
                            .changed();
                    });
                    ui.end_row();

                    ui.label("Position:");
                    ui.horizontal(|ui| {
                        changed |= ui.add(egui::DragValue::new(&mut x).speed(1)).changed();
                        changed |= ui.add(egui::DragValue::new(&mut y).speed(1)).changed();
                    });
                    ui.end_row();

                    ui.label("Area:");
                    ui.label(
                        mag_core::area::get_area_m(x, y).unwrap_or_else(|| "Unknown".to_owned()),
                    );
                    ui.end_row();

                    ui.label("Gold:");
                    ui.horizontal(|ui| {
                        changed |= ui.add(egui::DragValue::new(&mut gold).speed(1)).changed();
                        ui.label(crate::format_gold_silver(gold));
                    });
                    ui.end_row();

                    ui.label("Points:");
                    let points_tot_u32 = i64::from(points_tot).max(0) as u32;
                    let rank_name = mag_core::ranks::rank_name(points_tot_u32);
                    ui.horizontal(|ui| {
                        changed |= ui.add(egui::DragValue::new(&mut points).speed(1)).changed();
                        changed |= ui
                            .add(egui::DragValue::new(&mut points_tot).speed(1))
                            .changed();
                        ui.label(format!("({})", rank_name));
                    });
                    ui.end_row();

                    ui.label("Armor:");
                    changed |= ui.add(egui::DragValue::new(&mut armor).speed(1)).changed();
                    ui.end_row();

                    ui.label("Weapon:");
                    changed |= ui.add(egui::DragValue::new(&mut weapon).speed(1)).changed();
                    ui.end_row();

                    ui.label("Light:");
                    changed |= ui.add(egui::DragValue::new(&mut light).speed(1)).changed();
                    ui.end_row();

                    ui.label("Armor Bonus:");
                    ui.vertical(|ui| {
                        changed |= ui.add(egui::DragValue::new(&mut armor_bonus).speed(1)).changed();
                        ui.label("Permanent armor added on top of worn items.");
                    });
                    ui.end_row();

                    ui.label("Weapon Bonus:");
                    ui.vertical(|ui| {
                        changed |= ui.add(egui::DragValue::new(&mut weapon_bonus).speed(1)).changed();
                        ui.label("Permanent weapon damage added on top of worn items.");
                    });
                    ui.end_row();

                    ui.label("Light Bonus:");
                    ui.vertical(|ui| {
                        changed |= ui.add(egui::DragValue::new(&mut light_bonus).speed(1)).changed();
                        ui.label("Permanent light radius added on top of worn items.");
                    });
                    ui.end_row();

                    ui.label("Gethit Bonus:");
                    ui.vertical(|ui| {
                        changed |= ui.add(egui::DragValue::new(&mut gethit_bonus).speed(1)).changed();
                        ui.label("Thorns damage dealt back to melee attackers (rand(value)+1, armor-bypassing). 0 = disabled.");
                    });
                    ui.end_row();

                    ui.label("Mode:");
                    changed |= ui.add(egui::DragValue::new(&mut mode).speed(1)).changed();
                    ui.end_row();

                    ui.label("Speed:");
                    changed |= ui.add(egui::DragValue::new(&mut speed).speed(1)).changed();
                    ui.end_row();

                    ui.label("Speed Mod:");
                    ui.vertical(|ui| {
                        changed |= ui.add(egui::DragValue::new(&mut speed_mod_val).speed(1)).changed();
                        ui.label("Race/template speed modifier applied on top of agility/strength.");
                    });
                    ui.end_row();

                    ui.label("Monster Class:");
                    changed |= ui
                        .add(egui::DragValue::new(&mut monster_class).speed(1))
                        .changed();
                    ui.end_row();
                });

            ui.separator();
            crate::centered_heading(ui, "Attributes");
            egui::Grid::new("character_attributes")
                .num_columns(7)
                .spacing([15.0, 4.0])
                .striped(true)
                .show(ui, |ui| {
                    ui.label("Stat");
                    ui.label("Base");
                    ui.label("Preset");
                    ui.label("Max");
                    ui.label("Difficulty");
                    ui.label("Dynamic");
                    ui.label("Total");
                    ui.end_row();

                    let attrib_names = ["Bravery", "Willpower", "Intuition", "Agility", "Strength"];
                    for (i, name) in attrib_names.iter().enumerate() {
                        ui.label(*name);
                        for j in 0..6 {
                            let mut v = i32::from(attrib[i][j]);
                            if ui.add(egui::DragValue::new(&mut v).speed(1)).changed() {
                                attrib[i][j] = Self::clamp_u8(v);
                                changed = true;
                            }
                        }
                        ui.end_row();
                    }
                });

            ui.separator();
            egui::Grid::new("character_vitals")
                .num_columns(7)
                .spacing([15.0, 4.0])
                .striped(true)
                .show(ui, |ui| {
                    ui.label("Vital");
                    ui.label("[0]");
                    ui.label("[1]");
                    ui.label("[2]");
                    ui.label("[3]");
                    ui.label("[4]");
                    ui.label("[5]");
                    ui.end_row();

                    ui.label("HP");
                    for i in 0..6 {
                        let mut v = i32::from(hp[i]);
                        if ui.add(egui::DragValue::new(&mut v).speed(1)).changed() {
                            hp[i] = Self::clamp_u16(v);
                            changed = true;
                        }
                    }
                    ui.end_row();

                    ui.label("Endurance");
                    for i in 0..6 {
                        let mut v = i32::from(end[i]);
                        if ui.add(egui::DragValue::new(&mut v).speed(1)).changed() {
                            end[i] = Self::clamp_u16(v);
                            changed = true;
                        }
                    }
                    ui.end_row();

                    ui.label("Mana");
                    for i in 0..6 {
                        let mut v = i32::from(mana[i]);
                        if ui.add(egui::DragValue::new(&mut v).speed(1)).changed() {
                            mana[i] = Self::clamp_u16(v);
                            changed = true;
                        }
                    }
                    ui.end_row();
                });

            ui.separator();
            crate::centered_heading(ui, "Active Values");
            egui::Grid::new("character_active")
                .num_columns(2)
                .spacing([40.0, 4.0])
                .striped(true)
                .show(ui, |ui| {
                    ui.label("Active HP:");
                    changed |= ui.add(egui::DragValue::new(&mut a_hp).speed(1)).changed();
                    ui.end_row();

                    ui.label("Active Endurance:");
                    changed |= ui.add(egui::DragValue::new(&mut a_end).speed(1)).changed();
                    ui.end_row();

                    ui.label("Active Mana:");
                    changed |= ui.add(egui::DragValue::new(&mut a_mana).speed(1)).changed();
                    ui.end_row();
                });

            ui.separator();
            crate::centered_heading(ui, "Skills");
            egui::Grid::new("character_skills")
                .num_columns(8)
                .spacing([15.0, 4.0])
                .striped(true)
                .show(ui, |ui| {
                    ui.label("Skill #");
                    ui.label("Skill Name");
                    ui.label("[0]");
                    ui.label("[1]");
                    ui.label("[2]");
                    ui.label("[3]");
                    ui.label("[4]");
                    ui.label("[5]");
                    ui.end_row();

                    for (i, _skill) in character.skill.iter().enumerate() {
                        crate::centered_label(ui, format!("{}", i));
                        ui.label(skills::get_skill_name(i));
                        for j in 0..6 {
                            let mut v = i32::from(skills[i][j]);
                            if ui.add(egui::DragValue::new(&mut v).speed(1)).changed() {
                                skills[i][j] = Self::clamp_u8(v);
                                changed = true;
                            }
                        }
                        ui.end_row();
                    }
                });

            ui.separator();
            crate::centered_heading(ui, "Inventory");
            egui::Grid::new("character_inventory")
                .num_columns(4)
                .spacing([20.0, 4.0])
                .striped(true)
                .show(ui, |ui| {
                    ui.label("Slot");
                    ui.label("Item ID");
                    ui.label("Slot");
                    ui.label("Item ID");
                    ui.end_row();

                    let item_count = 40; // character.item.len()
                    for i in (0..item_count).step_by(2) {
                        let mut item1 = i64::from(inventory[i]);
                        let mut item2 = if i + 1 < item_count {
                            i64::from(inventory[i + 1])
                        } else {
                            0
                        };

                        ui.label(format!("{}", i));
                        ui.horizontal(|ui| {
                            if ui.add(egui::DragValue::new(&mut item1).speed(1)).changed() {
                                inventory[i] = item1.max(0) as u32;
                                changed = true;
                            }

                            let current_id = item1.max(0) as u32;
                            if current_id != 0 && ui.small_button("View").clicked() {
                                self.item_popup_id = Some(current_id);
                            }
                        });
                        ui.label(format!("{}", i + 1));
                        if i + 1 < item_count {
                            ui.horizontal(|ui| {
                                if ui.add(egui::DragValue::new(&mut item2).speed(1)).changed() {
                                    inventory[i + 1] = item2.max(0) as u32;
                                    changed = true;
                                }

                                let current_id = item2.max(0) as u32;
                                if current_id != 0 && ui.small_button("View").clicked() {
                                    self.item_popup_id = Some(current_id);
                                }
                            });
                        } else {
                            ui.label("-");
                        }
                        ui.end_row();
                    }
                });

            ui.separator();
            crate::centered_heading(ui, "Worn Equipment");
            egui::Grid::new("character_worn")
                .num_columns(2)
                .spacing([20.0, 4.0])
                .striped(true)
                .show(ui, |ui| {
                    ui.label("Slot");
                    ui.label("Item ID");
                    ui.end_row();

                    let worn_count = 20;
                    for i in 0..worn_count {
                        let mut worn_item = i64::from(worn[i]);
                        ui.label(format!("{}", i));
                        ui.horizontal(|ui| {
                            if ui
                                .add(egui::DragValue::new(&mut worn_item).speed(1))
                                .changed()
                            {
                                worn[i] = worn_item.max(0) as u32;
                                changed = true;
                            }

                            let current_id = worn_item.max(0) as u32;
                            if current_id != 0 && ui.small_button("View").clicked() {
                                self.item_popup_id = Some(current_id);
                            }
                        });
                        ui.end_row();
                    }
                });

            ui.separator();
            crate::centered_heading(ui, "Driver Data");
            ui.horizontal(|ui| {
                ui.checkbox(
                    &mut self.show_all_data_fields,
                    "Show all possible data fields",
                );
            });
            egui::Grid::new("character_driver_data")
                .num_columns(2)
                .spacing([20.0, 4.0])
                .striped(true)
                .show(ui, |ui| {
                    let mut shown_any = false;
                    for i in 0..100 {
                        if !self.show_all_data_fields && data[i] == 0 {
                            continue;
                        }

                        shown_any = true;
                        ui.label(format!("data[{}]:", i));
                        changed |= ui
                            .add(egui::DragValue::new(&mut data[i]).speed(1))
                            .changed();
                        ui.end_row();
                    }

                    if !shown_any {
                        let i = 0;
                        ui.label(format!("data[{}]:", i));
                        changed |= ui
                            .add(egui::DragValue::new(&mut data[i]).speed(1))
                            .changed();
                        ui.end_row();
                    }
                });

            // Commit edits back into packed struct
            character.temp = Self::clamp_u16(temp);
            character.used = Self::clamp_u8(used);

            crate::write_c_string(&mut name_buf, &name);
            crate::write_c_string(&mut reference_buf, &reference);
            crate::write_c_string(&mut description_buf, &description);
            character.name = name_buf;
            character.reference = reference_buf;
            character.description = description_buf;

            character.kindred = kindred;
            character.sprite = Self::clamp_u16(sprite);
            character.sound = Self::clamp_u16(sound);
            character.flags = character_flags.bits();
            character.alignment = Self::clamp_i16(alignment);
            character.temple_x = Self::clamp_u16(temple_x);
            character.temple_y = Self::clamp_u16(temple_y);
            character.tavern_x = Self::clamp_u16(tavern_x);
            character.tavern_y = Self::clamp_u16(tavern_y);
            character.x = Self::clamp_i16(x);
            character.y = Self::clamp_i16(y);
            character.gold = gold;
            character.points = points;
            character.points_tot = points_tot;
            character.armor = Self::clamp_i16(armor);
            character.weapon = Self::clamp_i16(weapon);
            character.light = Self::clamp_u8(light);
            character.armor_bonus = Self::clamp_u8(armor_bonus);
            character.weapon_bonus = Self::clamp_u8(weapon_bonus);
            character.light_bonus = Self::clamp_u8(light_bonus);
            character.gethit_bonus = Self::clamp_i8(gethit_bonus);
            character.mode = Self::clamp_u8(mode);
            character.speed = Self::clamp_i16(speed);
            character.speed_mod = Self::clamp_i8(speed_mod_val);
            character.monster_class = monster_class;
            character.a_hp = a_hp;
            character.a_end = a_end;
            character.a_mana = a_mana;
            character.attrib = attrib;
            character.hp = hp;
            character.end = end;
            character.mana = mana;
            character.skill = skills;
            character.item = inventory;
            character.worn = worn;
            character.data = data;

            self.mark_dirty_if(changed);
        });
    }
}

impl eframe::App for TemplateViewerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.frame_count += 1;

        let save_shortcut = ctx.input(|i| {
            let mods = i.modifiers;
            (mods.command || mods.ctrl) && i.key_pressed(egui::Key::S)
        });
        if save_shortcut && self.loaded_world.is_some() {
            if self.data_source.is_live_api() {
                self.save_to_api();
            } else {
                self.save_snapshot_as_dialog();
            }
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

        // Auto-poll reload status every ~2 s while a request is pending.
        // We stop after 60 s to match the server-side STATUS_TTL and avoid
        // hammering the API if the server never responds.
        if self.pending_reload_request_id.is_some() {
            const POLL_INTERVAL: std::time::Duration = std::time::Duration::from_secs(2);
            const GIVE_UP: std::time::Duration = std::time::Duration::from_secs(60);

            let since_start = self
                .pending_reload_since
                .map(|t| t.elapsed())
                .unwrap_or(GIVE_UP);

            if since_start >= GIVE_UP {
                self.pending_reload_request_id = None;
                self.pending_reload_since = None;
                self.last_reload_poll = None;
                self.save_status = Some("Reload status: timed out waiting for server".to_owned());
            } else {
                let should_poll = self
                    .last_reload_poll
                    .map(|t| t.elapsed() >= POLL_INTERVAL)
                    .unwrap_or(true); // poll immediately on first frame after request

                if should_poll {
                    self.last_reload_poll = Some(std::time::Instant::now());
                    self.poll_reload_status();
                }

                // Schedule a repaint so egui wakes us up to poll again.
                ctx.request_repaint_after(POLL_INTERVAL);
            }
        }

        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    let is_live_api = self.data_source.is_live_api();
                    let save_label = if is_live_api {
                        "Save to API\tCtrl+S"
                    } else {
                        "Save Snapshot As...\tCtrl+S"
                    };
                    if ui
                        .add_enabled(self.loaded_world.is_some(), egui::Button::new(save_label))
                        .clicked()
                    {
                        if is_live_api {
                            self.save_to_api();
                        } else {
                            self.save_snapshot_as_dialog();
                        }
                        ui.close_menu();
                    }

                    if is_live_api {
                        if ui
                            .add_enabled(
                                self.admin_client.is_some(),
                                egui::Button::new("Reload server templates..."),
                            )
                            .clicked()
                        {
                            self.reload_confirm_open = true;
                            ui.close_menu();
                        }
                        if ui
                            .add_enabled(
                                self.pending_reload_request_id.is_some(),
                                egui::Button::new("Poll reload status"),
                            )
                            .clicked()
                        {
                            self.poll_reload_status();
                            ui.close_menu();
                        }
                    }

                    let can_revert = self.dirty;
                    if ui
                        .add_enabled(can_revert, egui::Button::new("Revert (discard changes)"))
                        .clicked()
                    {
                        self.revert_unsaved_changes();
                        ui.close_menu();
                    }

                    ui.separator();

                    let reload_label = "Reload snapshot";
                    if ui.button(reload_label).clicked() {
                        self.load_current_source();
                        ui.close_menu();
                    }

                    ui.separator();

                    ui.menu_button("Data Source", |ui| {
                        let is_snapshot = matches!(self.data_source, DataSource::SnapshotFile(_));
                        if ui
                            .selectable_label(is_snapshot, ".wsnap Snapshot")
                            .clicked()
                        {
                            if let Some(path) = rfd::FileDialog::new()
                                .add_filter("World Snapshot", &["wsnap"])
                                .pick_file()
                            {
                                self.load_from_snapshot(path);
                            }
                            ui.close_menu();
                        }
                        let is_api = self.data_source.is_live_api();
                        if ui.selectable_label(is_api, "Live Admin API...").clicked() {
                            self.open_connect_dialog();
                            ui.close_menu();
                        }
                    });

                    ui.separator();

                    if ui.button("Open snapshot...").clicked() {
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter("World Snapshot", &["wsnap"])
                            .pick_file()
                        {
                            self.load_from_snapshot(path);
                        }
                        ui.close_menu();
                    }

                    if ui.button("Select Graphics Zip...").clicked() {
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter("Zip", &["zip"])
                            .pick_file()
                        {
                            self.load_graphics_zip(path);
                        }
                        ui.close_menu();
                    }

                    if self.graphics_zip.is_some() && ui.button("Clear Graphics Zip").clicked() {
                        self.graphics_zip = None;
                        self.graphics_zip_error = None;
                        ui.close_menu();
                    }

                    ui.separator();

                    if ui.button("Exit").clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });

                ui.separator();

                if ui
                    .selectable_label(self.view_mode == ViewMode::ItemTemplates, "Item Templates")
                    .clicked()
                {
                    self.view_mode = ViewMode::ItemTemplates;
                }
                if ui
                    .selectable_label(
                        self.view_mode == ViewMode::CharacterTemplates,
                        "Character Templates",
                    )
                    .clicked()
                {
                    self.view_mode = ViewMode::CharacterTemplates;
                }
                if ui
                    .selectable_label(self.view_mode == ViewMode::Items, "Items")
                    .clicked()
                {
                    self.view_mode = ViewMode::Items;
                }
                if ui
                    .selectable_label(self.view_mode == ViewMode::Characters, "Characters")
                    .clicked()
                {
                    self.view_mode = ViewMode::Characters;
                }

                // Right-aligned action buttons for connection and reload.
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let is_live_api = self.data_source.is_live_api();
                    if is_live_api {
                        let reload_btn = egui::Button::new(
                            egui::RichText::new("Reload Server Templates")
                                .color(egui::Color32::WHITE),
                        )
                        .fill(egui::Color32::from_rgb(160, 60, 60));
                        if ui
                            .add_enabled(self.admin_client.is_some(), reload_btn)
                            .on_hover_text(
                                "Ask the running server to swap its in-memory template tables. \
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
                });
            });

            ui.separator();
            ui.label(format!("Source: {}", self.data_source.display_label()));

            if let Some(ref error) = self.load_error {
                ui.separator();
                ui.colored_label(egui::Color32::RED, format!("Error: {}", error));
            }

            if let Some(ref error) = self.graphics_zip_error {
                ui.separator();
                ui.colored_label(egui::Color32::YELLOW, format!("GFX: {}", error));
            }

            if self.dirty {
                ui.separator();
                ui.colored_label(egui::Color32::YELLOW, "Unsaved changes");
            }

            if let Some(ref status) = self.save_status {
                ui.separator();
                let color = if status.starts_with("Save failed") {
                    egui::Color32::RED
                } else {
                    egui::Color32::GREEN
                };
                ui.colored_label(color, status);
            }
        });

        egui::CentralPanel::default().show(ctx, |ui| match self.view_mode {
            ViewMode::ItemTemplates => {
                egui::SidePanel::left("item_template_list")
                    .resizable(true)
                    .default_width(300.0)
                    .show_inside(ui, |ui| {
                        let used_count = self
                            .item_templates
                            .iter()
                            .filter(|item| item.used == 1)
                            .count();
                        if self.show_unused_templates {
                            ui.heading(format!(
                                "Item Templates ({}/{})",
                                used_count,
                                self.item_templates.len()
                            ));
                        } else {
                            ui.heading(format!("Item Templates ({})", used_count));
                        }
                        ui.separator();
                        self.render_item_list(ui);
                    });

                egui::CentralPanel::default().show_inside(ui, |ui| {
                    if let Some(idx) = self.selected_item_index {
                        if idx < self.item_templates.len() {
                            self.render_item_details_by_index(
                                ui,
                                ItemDetailsSource::ItemTemplates,
                                idx,
                            );
                        }
                    } else {
                        ui.centered_and_justified(|ui| {
                            ui.label("Select an item template from the list");
                        });
                    }
                });
            }
            ViewMode::CharacterTemplates => {
                egui::SidePanel::left("character_template_list")
                    .resizable(true)
                    .default_width(300.0)
                    .show_inside(ui, |ui| {
                        let used_count = self
                            .character_templates
                            .iter()
                            .filter(|character| character.used == 1)
                            .count();
                        if self.show_unused_templates {
                            ui.heading(format!(
                                "Character Templates ({}/{})",
                                used_count,
                                self.character_templates.len()
                            ));
                        } else {
                            ui.heading(format!("Character Templates ({})", used_count));
                        }
                        ui.separator();
                        self.render_character_list(ui);
                    });

                egui::CentralPanel::default().show_inside(ui, |ui| {
                    if let Some(idx) = self.selected_character_index {
                        if idx < self.character_templates.len() {
                            self.render_character_details_by_index(
                                ui,
                                CharacterDetailsSource::CharacterTemplates,
                                idx,
                            );
                        }
                    } else {
                        ui.centered_and_justified(|ui| {
                            ui.label("Select a character template from the list");
                        });
                    }
                });
            }
            ViewMode::Items => {
                egui::SidePanel::left("item_list")
                    .resizable(true)
                    .default_width(300.0)
                    .show_inside(ui, |ui| {
                        let used_count = self
                            .items
                            .iter()
                            .filter(|item| item.used != mag_core::constants::USE_EMPTY)
                            .count();
                        ui.heading(format!("Items ({})", used_count));
                        ui.separator();
                        self.render_item_instance_list(ui);
                    });

                egui::CentralPanel::default().show_inside(ui, |ui| {
                    if let Some(idx) = self.selected_item_instance_index {
                        if idx < self.items.len() {
                            self.render_item_details_by_index(ui, ItemDetailsSource::Items, idx);
                        }
                    } else {
                        ui.centered_and_justified(|ui| {
                            ui.label("Select an item from the list");
                        });
                    }
                });
            }
            ViewMode::Characters => {
                egui::SidePanel::left("character_list")
                    .resizable(true)
                    .default_width(300.0)
                    .show_inside(ui, |ui| {
                        let used_count = self
                            .characters
                            .iter()
                            .filter(|character| character.used != mag_core::constants::USE_EMPTY)
                            .count();
                        ui.heading(format!("Characters ({})", used_count));
                        ui.separator();
                        self.render_character_instance_list(ui);
                    });

                egui::CentralPanel::default().show_inside(ui, |ui| {
                    if let Some(idx) = self.selected_character_instance_index {
                        if idx < self.characters.len() {
                            self.render_character_details_by_index(
                                ui,
                                CharacterDetailsSource::Characters,
                                idx,
                            );
                        }
                    } else {
                        ui.centered_and_justified(|ui| {
                            ui.label("Select a character from the list");
                        });
                    }
                });
            }
        });

        self.render_item_popup(ctx);
        self.render_connect_dialog(ctx);
        self.render_reload_confirm_dialog(ctx);
    }
}
