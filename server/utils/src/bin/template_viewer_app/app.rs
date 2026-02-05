use super::graphics::GraphicsZipCache;
use eframe::egui;
use egui::Vec2;
use mag_core::string_operations::c_string_to_str;
use mag_core::types::skilltab::get_skill_name;
use std::fs;
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
    dat_dir: Option<PathBuf>,
    dirty: bool,
    save_status: Option<String>,
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
            dat_dir: None,
            dirty: false,
            save_status: None,
        }
    }
}

impl TemplateViewerApp {
    pub(crate) fn new() -> Self {
        let mut app = Self::default();
        if let Some(dir) = crate::dat_dir_from_args().or_else(crate::default_dat_dir) {
            app.load_templates_from_dir(dir);
        }

        if let Some(zip_path) =
            crate::graphics_zip_from_args().or_else(crate::default_graphics_zip_path)
        {
            app.load_graphics_zip(zip_path);
        }
        app
    }

    fn default_save_filename(&self) -> &'static str {
        match self.view_mode {
            ViewMode::ItemTemplates => "titem_new.dat",
            ViewMode::CharacterTemplates => "tchar_new.dat",
            ViewMode::Items => "items_new.dat",
            ViewMode::Characters => "chars_new.dat",
        }
    }

    fn save_current_view_dialog(&mut self) {
        self.save_status = None;

        let mut dialog = rfd::FileDialog::new().add_filter("DAT", &["dat"]);
        if let Some(dir) = self.dat_dir.as_ref() {
            dialog = dialog.set_directory(dir);
        }
        dialog = dialog.set_file_name(self.default_save_filename());

        let Some(path) = dialog.save_file() else {
            return;
        };

        match self.save_current_view_to_path(&path) {
            Ok(()) => {
                self.dirty = false;
                self.save_status = Some(format!("Saved to {}", path.display()));
            }
            Err(e) => {
                self.save_status = Some(format!("Save failed: {e}"));
            }
        }
    }

    fn save_current_view_to_path(&self, path: &PathBuf) -> Result<(), String> {
        let mut bytes: Vec<u8> = Vec::new();

        match self.view_mode {
            ViewMode::ItemTemplates => {
                for item in &self.item_templates {
                    bytes.extend_from_slice(&item.to_bytes());
                }
            }
            ViewMode::CharacterTemplates => {
                for character in &self.character_templates {
                    bytes.extend_from_slice(&character.to_bytes());
                }
            }
            ViewMode::Items => {
                for item in &self.items {
                    bytes.extend_from_slice(&item.to_bytes());
                }
            }
            ViewMode::Characters => {
                for character in &self.characters {
                    bytes.extend_from_slice(&character.to_bytes());
                }
            }
        }

        fs::write(path, bytes).map_err(|e| format!("Failed to write {}: {e}", path.display()))
    }

    fn revert_unsaved_changes(&mut self) {
        self.save_status = None;

        let Some(dir) = self.dat_dir.clone() else {
            self.save_status = Some("Revert failed: no data directory selected".to_string());
            return;
        };

        let prev_view_mode = self.view_mode;
        let prev_selected_item_index = self.selected_item_index;
        let prev_selected_character_index = self.selected_character_index;
        let prev_selected_item_instance_index = self.selected_item_instance_index;
        let prev_selected_character_instance_index = self.selected_character_instance_index;

        self.load_templates_from_dir(dir);

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
        if self.load_error.is_some() {
            self.save_status = Some("Reverted changes (with load errors)".to_string());
        } else {
            self.save_status = Some("Reverted unsaved changes".to_string());
        }
    }

    fn mark_dirty_if(&mut self, changed: bool) {
        if changed {
            self.dirty = true;
        }
    }

    fn clamp_i8(v: i32) -> i8 {
        v.clamp(i8::MIN as i32, i8::MAX as i32) as i8
    }

    fn clamp_u8(v: i32) -> u8 {
        v.clamp(u8::MIN as i32, u8::MAX as i32) as u8
    }

    fn clamp_i16(v: i32) -> i16 {
        v.clamp(i16::MIN as i32, i16::MAX as i32) as i16
    }

    fn clamp_u16(v: i32) -> u16 {
        v.clamp(u16::MIN as i32, u16::MAX as i32) as u16
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

    fn load_items_from_file(&mut self, path: PathBuf) {
        self.load_error = None;
        self.save_status = None;
        self.dirty = false;
        self.selected_item_instance_index = None;

        if let Some(parent) = path.parent() {
            self.dat_dir = Some(parent.to_path_buf());
        }

        match self.load_items(&path) {
            Ok(items) => {
                self.items = items;
                self.view_mode = ViewMode::Items;
                self.save_status = Some(format!("Loaded items from {}", path.display()));
                log::info!("Loaded {} items from {:?}", self.items.len(), path);
            }
            Err(e) => {
                self.load_error = Some(format!("Failed to load items: {}", e));
                log::error!("Failed to load items from {:?}: {}", path, e);
            }
        }
    }

    fn load_characters_from_file(&mut self, path: PathBuf) {
        self.load_error = None;
        self.save_status = None;
        self.dirty = false;
        self.selected_character_instance_index = None;

        if let Some(parent) = path.parent() {
            self.dat_dir = Some(parent.to_path_buf());
        }

        match self.load_characters(&path) {
            Ok(chars) => {
                self.characters = chars;
                self.view_mode = ViewMode::Characters;
                self.save_status = Some(format!("Loaded characters from {}", path.display()));
                log::info!(
                    "Loaded {} characters from {:?}",
                    self.characters.len(),
                    path
                );
            }
            Err(e) => {
                self.load_error = Some(format!("Failed to load characters: {}", e));
                log::error!("Failed to load characters from {:?}: {}", path, e);
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

    fn load_templates_from_dir(&mut self, dir: PathBuf) {
        self.load_error = None;
        self.save_status = None;
        self.dat_dir = Some(dir.clone());
        self.dirty = false;
        log::info!("Loading templates from {:?}", dir);

        let item_path = dir.join("titem.dat");
        match self.load_item_templates(&item_path) {
            Ok(items) => {
                self.item_templates = items;
                self.view_mode = ViewMode::ItemTemplates;
                log::info!("Loaded {} item templates", self.item_templates.len());
            }
            Err(e) => {
                self.load_error = Some(format!("Failed to load item templates: {}", e));
                log::error!("Failed to load item templates: {}", e);
            }
        }

        let char_path = dir.join("tchar.dat");
        match self.load_character_templates(&char_path) {
            Ok(chars) => {
                self.character_templates = chars;
                if self.item_templates.is_empty() {
                    self.view_mode = ViewMode::CharacterTemplates;
                }
                log::info!(
                    "Loaded {} character templates",
                    self.character_templates.len()
                );
            }
            Err(e) => {
                let message = format!("Failed to load character templates: {}", e);
                if let Some(ref mut error) = self.load_error {
                    error.push_str("\n");
                    error.push_str(&message);
                } else {
                    self.load_error = Some(message);
                }
                log::error!("Failed to load character templates: {}", e);
            }
        }

        let item_path = dir.join("item.dat");
        match self.load_items(&item_path) {
            Ok(items) => {
                self.items = items;
                log::info!("Loaded {} items", self.items.len());
            }
            Err(e) => {
                let message = format!("Failed to load items: {}", e);
                if let Some(ref mut error) = self.load_error {
                    error.push_str("\n");
                    error.push_str(&message);
                } else {
                    self.load_error = Some(message);
                }
                log::error!("Failed to load items: {}", e);
            }
        }

        let char_path = dir.join("char.dat");
        match self.load_characters(&char_path) {
            Ok(chars) => {
                self.characters = chars;
                log::info!("Loaded {} characters", self.characters.len());
            }
            Err(e) => {
                let message = format!("Failed to load characters: {}", e);
                if let Some(ref mut error) = self.load_error {
                    error.push_str("\n");
                    error.push_str(&message);
                } else {
                    self.load_error = Some(message);
                }
                log::error!("Failed to load characters: {}", e);
            }
        }

        let map_path = dir.join("map.dat");
        match self.load_map(&map_path) {
            Ok(map_tiles) => {
                self.map_tiles = map_tiles;
                log::info!("Loaded {} map tiles", self.map_tiles.len());
            }
            Err(e) => {
                let message = format!("Failed to load map: {}", e);
                if let Some(ref mut error) = self.load_error {
                    error.push_str("\n");
                    error.push_str(&message);
                } else {
                    self.load_error = Some(message);
                }
                log::error!("Failed to load map: {}", e);
            }
        }

        // Pick a sensible default view.
        if !self.item_templates.is_empty() {
            self.view_mode = ViewMode::ItemTemplates;
        } else if !self.character_templates.is_empty() {
            self.view_mode = ViewMode::CharacterTemplates;
        } else if !self.items.is_empty() {
            self.view_mode = ViewMode::Items;
        } else if !self.characters.is_empty() {
            self.view_mode = ViewMode::Characters;
        }
    }

    fn load_item_templates(&self, path: &PathBuf) -> Result<Vec<mag_core::types::Item>, String> {
        let data = fs::read(&path).map_err(|e| e.to_string())?;
        let expected_size =
            mag_core::constants::MAXTITEM * std::mem::size_of::<mag_core::types::Item>();

        if data.len() != expected_size {
            return Err(format!(
                "Item templates size mismatch: expected {}, got {}",
                expected_size,
                data.len()
            ));
        }

        let mut templates = Vec::new();
        let item_size = std::mem::size_of::<mag_core::types::Item>();

        for i in 0..mag_core::constants::MAXTITEM {
            let offset = i * item_size;
            if let Some(item) = mag_core::types::Item::from_bytes(&data[offset..offset + item_size])
            {
                templates.push(item);
            } else {
                return Err(format!("Failed to parse item template at index {}", i));
            }
        }

        Ok(templates)
    }

    fn load_items(&self, path: &PathBuf) -> Result<Vec<mag_core::types::Item>, String> {
        let data = fs::read(&path).map_err(|e| e.to_string())?;
        let expected_size =
            mag_core::constants::MAXITEM * std::mem::size_of::<mag_core::types::Item>();

        if data.len() != expected_size {
            return Err(format!(
                "Items size mismatch: expected {}, got {}",
                expected_size,
                data.len()
            ));
        }

        let mut items = Vec::new();
        let item_size = std::mem::size_of::<mag_core::types::Item>();

        for i in 0..mag_core::constants::MAXITEM {
            let offset = i * item_size;
            if let Some(item) = mag_core::types::Item::from_bytes(&data[offset..offset + item_size])
            {
                items.push(item);
            } else {
                return Err(format!("Failed to parse item at index {}", i));
            }
        }

        Ok(items)
    }

    fn load_character_templates(
        &self,
        path: &PathBuf,
    ) -> Result<Vec<mag_core::types::Character>, String> {
        let data = fs::read(&path).map_err(|e| e.to_string())?;
        let expected_size =
            mag_core::constants::MAXTCHARS * std::mem::size_of::<mag_core::types::Character>();

        if data.len() != expected_size {
            return Err(format!(
                "Character templates size mismatch: expected {}, got {}",
                expected_size,
                data.len()
            ));
        }

        let mut templates = Vec::new();
        let char_size = std::mem::size_of::<mag_core::types::Character>();

        for i in 0..mag_core::constants::MAXTCHARS {
            let offset = i * char_size;
            if let Some(character) =
                mag_core::types::Character::from_bytes(&data[offset..offset + char_size])
            {
                templates.push(character);
            } else {
                return Err(format!("Failed to parse character template at index {}", i));
            }
        }

        Ok(templates)
    }

    fn load_characters(&self, path: &PathBuf) -> Result<Vec<mag_core::types::Character>, String> {
        let data = fs::read(&path).map_err(|e| e.to_string())?;
        let expected_size =
            mag_core::constants::MAXCHARS * std::mem::size_of::<mag_core::types::Character>();

        if data.len() != expected_size {
            return Err(format!(
                "Characters size mismatch: expected {}, got {}",
                expected_size,
                data.len()
            ));
        }

        let mut chars = Vec::new();
        let char_size = std::mem::size_of::<mag_core::types::Character>();

        for i in 0..mag_core::constants::MAXCHARS {
            let offset = i * char_size;
            if let Some(character) =
                mag_core::types::Character::from_bytes(&data[offset..offset + char_size])
            {
                chars.push(character);
            } else {
                return Err(format!("Failed to parse character at index {}", i));
            }
        }

        Ok(chars)
    }

    fn load_map(&self, path: &PathBuf) -> Result<Vec<mag_core::types::Map>, String> {
        let data = fs::read(&path).map_err(|e| e.to_string())?;
        let tile_count = (mag_core::constants::SERVER_MAPX as usize)
            * (mag_core::constants::SERVER_MAPY as usize);
        let expected_size = tile_count * std::mem::size_of::<mag_core::types::Map>();

        if data.len() != expected_size {
            return Err(format!(
                "Map size mismatch: expected {}, got {}",
                expected_size,
                data.len()
            ));
        }

        let mut map_tiles = Vec::with_capacity(tile_count);
        let tile_size = std::mem::size_of::<mag_core::types::Map>();

        for i in 0..tile_count {
            let offset = i * tile_size;
            if let Some(tile) = mag_core::types::Map::from_bytes(&data[offset..offset + tile_size])
            {
                map_tiles.push(tile);
            } else {
                return Err(format!("Failed to parse map tile at index {}", i));
            }
        }

        Ok(map_tiles)
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

    fn render_item_details(&mut self, ui: &mut egui::Ui, item: &mut mag_core::types::Item) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.heading(item.get_name());
            ui.separator();

            // Copy all fields to avoid packed struct issues
            let mut temp = item.temp as i32;
            let mut used = item.used as i32;
            let mut name_buf = item.name;
            let mut reference_buf = item.reference;
            let mut description_buf = item.description;
            let mut name = c_string_to_str(&name_buf).to_string();
            let mut reference = c_string_to_str(&reference_buf).to_string();
            let mut description = c_string_to_str(&description_buf).to_string();

            let mut value = item.value;
            let mut placement = item.placement;
            let flags = item.flags;
            let mut sprite_0 = item.sprite[0] as i32;
            let mut sprite_1 = item.sprite[1] as i32;
            let mut status_0 = item.status[0] as i32;
            let mut status_1 = item.status[1] as i32;
            let mut armor_0 = item.armor[0] as i32;
            let mut armor_1 = item.armor[1] as i32;
            let mut weapon_0 = item.weapon[0] as i32;
            let mut weapon_1 = item.weapon[1] as i32;
            let mut light_0 = item.light[0] as i32;
            let mut light_1 = item.light[1] as i32;
            let mut duration = item.duration;
            let mut cost = item.cost;
            let mut power = item.power;
            let mut min_rank = item.min_rank as i32;
            let mut driver = item.driver as i32;

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
                            for (idx, name) in mag_core::ranks::RANK_NAMES.iter().enumerate() {
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
                            let mut v = attrib[i][j] as i32;
                            if ui.add(egui::DragValue::new(&mut v).speed(1)).changed() {
                                attrib[i][j] = Self::clamp_i8(v);
                                changed = true;
                            }
                        }
                        ui.end_row();
                    }

                    ui.label("HP");
                    for j in 0..3 {
                        let mut v = hp[j] as i32;
                        if ui.add(egui::DragValue::new(&mut v).speed(1)).changed() {
                            hp[j] = Self::clamp_i16(v);
                            changed = true;
                        }
                    }
                    ui.end_row();

                    ui.label("Endurance");
                    for j in 0..3 {
                        let mut v = end[j] as i32;
                        if ui.add(egui::DragValue::new(&mut v).speed(1)).changed() {
                            end[j] = Self::clamp_i16(v);
                            changed = true;
                        }
                    }
                    ui.end_row();

                    ui.label("Mana");
                    for j in 0..3 {
                        let mut v = mana[j] as i32;
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
                        ui.label(get_skill_name(i));
                        for j in 0..3 {
                            let mut v = skill[i][j] as i32;
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
                // `titem.dat` entries is not reliable for this purpose.
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
                    let area = mag_core::area::get_area_m(x as i32, y as i32)
                        .unwrap_or_else(|| "Unknown".to_string());
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

    fn render_character_details(
        &mut self,
        ui: &mut egui::Ui,
        character: &mut mag_core::types::Character,
    ) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.heading(character.get_name());
            ui.separator();

            // Copy all packed fields to avoid alignment issues
            let mut temp = character.temp as i32;
            let mut used = character.used as i32;
            let mut name_buf = character.name;
            let mut reference_buf = character.reference;
            let mut description_buf = character.description;
            let mut name = c_string_to_str(&name_buf).to_string();
            let mut reference = c_string_to_str(&reference_buf).to_string();
            let mut description = c_string_to_str(&description_buf).to_string();

            let mut kindred = character.kindred;
            let mut sprite = character.sprite as i32;
            let mut sound = character.sound as i32;
            let flags = character.flags;
            let mut alignment = character.alignment as i32;
            let mut temple_x = character.temple_x as i32;
            let mut temple_y = character.temple_y as i32;
            let mut tavern_x = character.tavern_x as i32;
            let mut tavern_y = character.tavern_y as i32;
            let mut x = character.x as i32;
            let mut y = character.y as i32;
            let mut gold = character.gold;
            let mut points = character.points;
            let mut points_tot = character.points_tot;
            let mut armor = character.armor as i32;
            let mut weapon = character.weapon as i32;
            let mut light = character.light as i32;
            let mut mode = character.mode as i32;
            let mut speed = character.speed as i32;
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
                    changed |= ui
                        .add(egui::DragValue::new(&mut kindred).speed(1))
                        .changed();
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
                        mag_core::area::get_area_m(x, y).unwrap_or_else(|| "Unknown".to_string()),
                    );
                    ui.end_row();

                    ui.label("Gold:");
                    ui.horizontal(|ui| {
                        changed |= ui.add(egui::DragValue::new(&mut gold).speed(1)).changed();
                        ui.label(crate::format_gold_silver(gold));
                    });
                    ui.end_row();

                    ui.label("Points:");
                    let points_tot_u32 = (points_tot as i64).max(0) as u32;
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

                    ui.label("Mode:");
                    changed |= ui.add(egui::DragValue::new(&mut mode).speed(1)).changed();
                    ui.end_row();

                    ui.label("Speed:");
                    changed |= ui.add(egui::DragValue::new(&mut speed).speed(1)).changed();
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
                            let mut v = attrib[i][j] as i32;
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
                        let mut v = hp[i] as i32;
                        if ui.add(egui::DragValue::new(&mut v).speed(1)).changed() {
                            hp[i] = Self::clamp_u16(v);
                            changed = true;
                        }
                    }
                    ui.end_row();

                    ui.label("Endurance");
                    for i in 0..6 {
                        let mut v = end[i] as i32;
                        if ui.add(egui::DragValue::new(&mut v).speed(1)).changed() {
                            end[i] = Self::clamp_u16(v);
                            changed = true;
                        }
                    }
                    ui.end_row();

                    ui.label("Mana");
                    for i in 0..6 {
                        let mut v = mana[i] as i32;
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
                        ui.label(get_skill_name(i));
                        for j in 0..6 {
                            let mut v = skills[i][j] as i32;
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
                        let mut item1 = inventory[i] as i64;
                        let mut item2 = if i + 1 < item_count {
                            inventory[i + 1] as i64
                        } else {
                            0
                        };

                        ui.label(format!("{}", i));
                        if ui.add(egui::DragValue::new(&mut item1).speed(1)).changed() {
                            inventory[i] = item1.max(0) as u32;
                            changed = true;
                        }
                        ui.label(format!("{}", i + 1));
                        if i + 1 < item_count {
                            if ui.add(egui::DragValue::new(&mut item2).speed(1)).changed() {
                                inventory[i + 1] = item2.max(0) as u32;
                                changed = true;
                            }
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
                        let mut worn_item = worn[i] as i64;
                        ui.label(format!("{}", i));
                        if ui
                            .add(egui::DragValue::new(&mut worn_item).speed(1))
                            .changed()
                        {
                            worn[i] = worn_item.max(0) as u32;
                            changed = true;
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
            character.mode = Self::clamp_u8(mode);
            character.speed = Self::clamp_i16(speed);
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
        let save_shortcut = ctx.input(|i| {
            let mods = i.modifiers;
            (mods.command || mods.ctrl) && i.key_pressed(egui::Key::S)
        });
        if save_shortcut {
            self.save_current_view_dialog();
        }

        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui
                        .add_enabled(self.dirty, egui::Button::new("Save...\tCtrl+S"))
                        .clicked()
                    {
                        self.save_current_view_dialog();
                        ui.close_menu();
                    }

                    if ui
                        .add_enabled(
                            self.dirty && self.dat_dir.is_some(),
                            egui::Button::new("Revert (discard changes)"),
                        )
                        .clicked()
                    {
                        self.revert_unsaved_changes();
                        ui.close_menu();
                    }

                    ui.separator();

                    if ui.button("Open Items File... (item.dat)").clicked() {
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter("DAT", &["dat"])
                            .pick_file()
                        {
                            self.load_items_from_file(path);
                        }
                        ui.close_menu();
                    }

                    if ui.button("Open Characters File... (char.dat)").clicked() {
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter("DAT", &["dat"])
                            .pick_file()
                        {
                            self.load_characters_from_file(path);
                        }
                        ui.close_menu();
                    }

                    ui.separator();

                    if ui.button("Select Data Directory...").clicked() {
                        if let Some(path) = rfd::FileDialog::new()
                            .set_can_create_directories(true)
                            .pick_folder()
                        {
                            self.load_templates_from_dir(path);
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

                    if self.graphics_zip.is_some() {
                        if ui.button("Clear Graphics Zip").clicked() {
                            self.graphics_zip = None;
                            self.graphics_zip_error = None;
                            ui.close_menu();
                        }
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
            });

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
    }
}
