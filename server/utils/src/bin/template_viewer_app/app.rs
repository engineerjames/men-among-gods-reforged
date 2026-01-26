use super::graphics::GraphicsZipCache;
use eframe::egui;
use egui::Vec2;
use mag_core::string_operations::c_string_to_str;
use mag_core::types::skilltab::get_skill_name;
use std::fs;
use std::path::PathBuf;

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
    load_error: Option<String>,
    graphics_zip: Option<GraphicsZipCache>,
    graphics_zip_error: Option<String>,
}

#[derive(PartialEq)]
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
            load_error: None,
            graphics_zip: None,
            graphics_zip_error: None,
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

    fn find_item_template(&self, item_id: u32) -> Option<&mag_core::types::Item> {
        let index = item_id as usize;
        if index < self.item_templates.len() {
            return Some(&self.item_templates[index]);
        }

        let temp_id = item_id as u16;
        self.item_templates.iter().find(|item| item.temp == temp_id)
    }

    fn render_item_popup(&mut self, ctx: &egui::Context) {
        let Some(item_id) = self.item_popup_id else {
            return;
        };

        let mut open = true;
        egui::Window::new(format!("Item {}", item_id))
            .open(&mut open)
            .show(ctx, |ui| {
                if let Some(item) = self.find_item_template(item_id).copied() {
                    self.render_item_details(ui, &item);
                } else {
                    ui.label(format!("No item template found for ID {}", item_id));
                }
            });

        if !open {
            self.item_popup_id = None;
        }
    }

    fn centered_clickable_item_id(&mut self, ui: &mut egui::Ui, item_id: u32) {
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
            self.item_popup_id = Some(item_id);
        }
    }

    fn load_templates_from_dir(&mut self, dir: PathBuf) {
        self.load_error = None;
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

        ui.separator();

        let list_width = ui.available_width();
        egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                ui.set_min_width(list_width);
                for (idx, item) in self.item_templates.iter().enumerate() {
                    if item.used == mag_core::constants::USE_EMPTY {
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

        ui.separator();

        let list_width = ui.available_width();
        egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                ui.set_min_width(list_width);
                for (idx, character) in self.character_templates.iter().enumerate() {
                    if character.used == mag_core::constants::USE_EMPTY {
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

    fn render_item_details(&mut self, ui: &mut egui::Ui, item: &mag_core::types::Item) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.heading(item.get_name());
            ui.separator();

            // Copy all fields to avoid packed struct issues
            let temp = item.temp;
            let used = item.used;
            let value = item.value;
            let placement = item.placement;
            let flags = item.flags;
            let sprite_0 = item.sprite[0];
            let sprite_1 = item.sprite[1];
            let status_0 = item.status[0];
            let status_1 = item.status[1];
            let armor_0 = item.armor[0];
            let armor_1 = item.armor[1];
            let weapon_0 = item.weapon[0];
            let weapon_1 = item.weapon[1];
            let light_0 = item.light[0];
            let light_1 = item.light[1];
            let duration = item.duration;
            let cost = item.cost;
            let power = item.power;
            let min_rank = item.min_rank;
            let driver = item.driver;

            egui::Grid::new("item_details")
                .num_columns(2)
                .spacing([40.0, 4.0])
                .striped(true)
                .show(ui, |ui| {
                    ui.label("Index:");
                    crate::centered_label(ui, format!("{}", temp));
                    ui.end_row();

                    ui.label("Used:");
                    crate::centered_label(ui, format!("{}", used));
                    ui.end_row();

                    ui.label("Reference:");
                    ui.label(c_string_to_str(&item.reference));
                    ui.end_row();

                    ui.label("Description:");
                    ui.add(egui::Label::new(c_string_to_str(&item.description)).wrap());
                    ui.end_row();

                    ui.label("Value:");
                    crate::centered_label(ui, crate::format_gold_silver(value as i32));
                    ui.end_row();

                    ui.label("Placement:");
                    ui.add_enabled_ui(false, |ui| {
                        egui::ComboBox::from_id_salt(format!("placement_combo_{}", temp))
                            .selected_text(crate::placement_label(placement))
                            .show_ui(ui, |ui| {
                                for (value, name) in crate::placement_options() {
                                    let _ = ui.selectable_label(*value == placement, *name);
                                }
                            });
                    });
                    ui.end_row();

                    ui.label("Flags:");
                    ui.end_row();
                });

            let item_flags = mag_core::constants::ItemFlags::from_bits_truncate(flags);
            egui::Grid::new(format!("item_flags_grid_{}", temp))
                .num_columns(3)
                .spacing([10.0, 4.0])
                .striped(true)
                .show(ui, |ui| {
                    let mut col = 0;
                    for (flag, name) in crate::get_item_flag_info() {
                        let mut is_set = item_flags.contains(flag);
                        ui.add_enabled(false, egui::Checkbox::new(&mut is_set, name));
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
                    self.sprite_cell(ui, sprite_0 as usize);
                    ui.end_row();

                    ui.label("Sprite[1]:");
                    self.sprite_cell(ui, sprite_1 as usize);
                    ui.end_row();

                    ui.label("Status:");
                    crate::centered_label(ui, format!("[{}, {}]", status_0, status_1));
                    ui.end_row();

                    ui.label("Armor:");
                    crate::centered_label(ui, format!("[{}, {}]", armor_0, armor_1));
                    ui.end_row();

                    ui.label("Weapon:");
                    crate::centered_label(ui, format!("[{}, {}]", weapon_0, weapon_1));
                    ui.end_row();

                    ui.label("Light:");
                    crate::centered_label(ui, format!("[{}, {}]", light_0, light_1));
                    ui.end_row();

                    ui.label("Duration:");
                    crate::centered_label(ui, format!("{}", duration));
                    ui.end_row();

                    ui.label("Cost:");
                    crate::centered_label(ui, format!("{}", cost));
                    ui.end_row();

                    ui.label("Power:");
                    crate::centered_label(ui, format!("{}", power));
                    ui.end_row();

                    ui.label("Min Rank:");
                    ui.add_enabled_ui(false, |ui| {
                        egui::ComboBox::from_id_salt(format!("min_rank_combo_{}", temp))
                            .selected_text(crate::rank_label(min_rank))
                            .show_ui(ui, |ui| {
                                let none_label = "-1: None";
                                let _ = ui.selectable_label(min_rank < 0, none_label);
                                for (idx, name) in mag_core::ranks::RANK_NAMES.iter().enumerate() {
                                    let label = format!("{}: {}", idx, name);
                                    let _ = ui.selectable_label(min_rank == idx as i8, label);
                                }
                            });
                    });
                    ui.end_row();

                    ui.label("Driver:");
                    crate::centered_label(ui, format!("{}", driver));
                    ui.end_row();
                });

            ui.separator();
            crate::centered_heading(ui, "Attributes");
            egui::Grid::new("item_attributes")
                .num_columns(4)
                .spacing([20.0, 4.0])
                .striped(true)
                .show(ui, |ui| {
                    crate::centered_label(ui, "Stat");
                    crate::centered_label(ui, "Worn");
                    crate::centered_label(ui, "Active");
                    crate::centered_label(ui, "Min Required");
                    ui.end_row();

                    let attrib_names = ["Bravery", "Willpower", "Intuition", "Agility", "Strength"];
                    for (i, name) in attrib_names.iter().enumerate() {
                        let val_0 = item.attrib[i][0];
                        let val_1 = item.attrib[i][1];
                        let val_2 = item.attrib[i][2];
                        ui.label(*name);
                        crate::centered_label(ui, format!("{:+}", val_0));
                        crate::centered_label(ui, format!("{:+}", val_1));
                        crate::centered_label(ui, format!("{}", val_2));
                        ui.end_row();
                    }

                    let hp_0 = item.hp[0];
                    let hp_1 = item.hp[1];
                    let hp_2 = item.hp[2];
                    ui.label("HP");
                    crate::centered_label(ui, format!("{:+}", hp_0));
                    crate::centered_label(ui, format!("{:+}", hp_1));
                    crate::centered_label(ui, format!("{}", hp_2));
                    ui.end_row();

                    let end_0 = item.end[0];
                    let end_1 = item.end[1];
                    let end_2 = item.end[2];
                    ui.label("Endurance");
                    crate::centered_label(ui, format!("{:+}", end_0));
                    crate::centered_label(ui, format!("{:+}", end_1));
                    crate::centered_label(ui, format!("{}", end_2));
                    ui.end_row();

                    let mana_0 = item.mana[0];
                    let mana_1 = item.mana[1];
                    let mana_2 = item.mana[2];
                    ui.label("Mana");
                    crate::centered_label(ui, format!("{:+}", mana_0));
                    crate::centered_label(ui, format!("{:+}", mana_1));
                    crate::centered_label(ui, format!("{}", mana_2));
                    ui.end_row();
                });

            ui.separator();
            crate::centered_heading(ui, "Skills");
            egui::Grid::new("item_skills")
                .num_columns(5)
                .spacing([20.0, 4.0])
                .striped(true)
                .show(ui, |ui| {
                    crate::centered_label(ui, "Skill #");
                    crate::centered_label(ui, "Skill Name");
                    crate::centered_label(ui, "Worn");
                    crate::centered_label(ui, "Active");
                    crate::centered_label(ui, "Min Required");
                    ui.end_row();

                    for (i, skill) in item.skill.iter().enumerate() {
                        let s0 = skill[0];
                        let s1 = skill[1];
                        let s2 = skill[2];
                        crate::centered_label(ui, format!("{}", i));
                        ui.label(get_skill_name(i));
                        crate::centered_label(ui, format!("{:+}", s0));
                        crate::centered_label(ui, format!("{:+}", s1));
                        crate::centered_label(ui, format!("{}", s2));
                        ui.end_row();
                    }
                });

            ui.separator();
            crate::centered_heading(ui, "Driver Data");
            egui::Grid::new("item_driver_data")
                .num_columns(2)
                .spacing([40.0, 4.0])
                .striped(true)
                .show(ui, |ui| {
                    for i in 0..10 {
                        let data = item.data[i];
                        ui.label(format!("data[{}]:", i));
                        crate::centered_label(ui, format!("{}", data));
                        ui.end_row();
                    }
                });

            if self.view_mode == ViewMode::ItemTemplates {
                ui.separator();

                // For templates, the *slot index* is the template id. The `temp` field inside
                // `titem.dat` entries is not reliable for this purpose.
                let template_id = self
                    .selected_item_index
                    .map(|idx| idx as u16)
                    .unwrap_or(temp);
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
                                    crate::centered_label(ui, format!("{}", item_id));
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
        character: &mag_core::types::Character,
    ) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.heading(character.get_name());
            ui.separator();

            // Copy all packed fields to avoid alignment issues
            let temp = character.temp;
            let used = character.used;
            let kindred = character.kindred;
            let sprite = character.sprite;
            let sound = character.sound;
            let flags = character.flags;
            let alignment = character.alignment;
            let temple_x = character.temple_x;
            let temple_y = character.temple_y;
            let tavern_x = character.tavern_x;
            let tavern_y = character.tavern_y;
            let x = character.x;
            let y = character.y;
            let gold = character.gold;
            let points = character.points;
            let points_tot = character.points_tot;
            let armor = character.armor;
            let weapon = character.weapon;
            let light = character.light;
            let mode = character.mode;
            let speed = character.speed;
            let monster_class = character.monster_class;
            let a_hp = character.a_hp;
            let a_end = character.a_end;
            let a_mana = character.a_mana;

            egui::Grid::new("character_details")
                .num_columns(2)
                .spacing([40.0, 4.0])
                .striped(true)
                .show(ui, |ui| {
                    ui.label("Index:");
                    crate::centered_label(ui, format!("{}", temp));
                    ui.end_row();

                    ui.label("Used:");
                    crate::centered_label(ui, format!("{}", used));
                    ui.end_row();

                    ui.label("Reference:");
                    ui.label(character.get_reference());
                    ui.end_row();

                    ui.label("Description:");
                    ui.add(egui::Label::new(c_string_to_str(&character.description)).wrap());
                    ui.end_row();

                    ui.label("Kindred:");
                    crate::centered_label(ui, format!("{}", kindred));
                    ui.end_row();

                    ui.label("Sprite:");
                    self.sprite_cell(ui, sprite as usize);
                    ui.end_row();

                    ui.label("Sound:");
                    crate::centered_label(ui, format!("{}", sound));
                    ui.end_row();

                    ui.label("Flags:");
                    ui.end_row();
                });

            let character_flags = mag_core::constants::CharacterFlags::from_bits_truncate(flags);
            egui::Grid::new(format!("character_flags_grid_{}", temp))
                .num_columns(3)
                .spacing([10.0, 4.0])
                .striped(true)
                .show(ui, |ui| {
                    let mut col = 0;
                    for flag in crate::get_character_flag_info() {
                        let mut is_set = character_flags.contains(flag);
                        ui.add_enabled(
                            false,
                            egui::Checkbox::new(
                                &mut is_set,
                                mag_core::constants::character_flags_name(flag),
                            ),
                        );
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
                    crate::centered_label(ui, format!("{}", alignment));
                    ui.end_row();

                    ui.label("Temple:");
                    crate::centered_label(ui, format!("({}, {})", temple_x, temple_y));
                    ui.end_row();

                    ui.label("Tavern:");
                    crate::centered_label(ui, format!("({}, {})", tavern_x, tavern_y));
                    ui.end_row();

                    ui.label("Position:");
                    crate::centered_label(ui, format!("({}, {})", x, y));
                    ui.end_row();

                    ui.label("Area:");
                    ui.label(
                        mag_core::area::get_area_m(x as i32, y as i32)
                            .unwrap_or_else(|| "Unknown".to_string()),
                    );
                    ui.end_row();

                    ui.label("Gold:");
                    crate::centered_label(ui, crate::format_gold_silver(gold));
                    ui.end_row();

                    ui.label("Points:");
                    let points_tot_u32 = (points_tot as i64).max(0) as u32;
                    let rank_name = mag_core::ranks::rank_name(points_tot_u32);
                    crate::centered_label(
                        ui,
                        format!("{} / {} ({})", points, points_tot, rank_name),
                    );
                    ui.end_row();

                    ui.label("Armor:");
                    crate::centered_label(ui, format!("{}", armor));
                    ui.end_row();

                    ui.label("Weapon:");
                    crate::centered_label(ui, format!("{}", weapon));
                    ui.end_row();

                    ui.label("Light:");
                    crate::centered_label(ui, format!("{}", light));
                    ui.end_row();

                    ui.label("Mode:");
                    crate::centered_label(ui, format!("{}", mode));
                    ui.end_row();

                    ui.label("Speed:");
                    crate::centered_label(ui, format!("{}", speed));
                    ui.end_row();

                    ui.label("Monster Class:");
                    crate::centered_label(ui, format!("{}", monster_class));
                    ui.end_row();
                });

            ui.separator();
            crate::centered_heading(ui, "Attributes");
            egui::Grid::new("character_attributes")
                .num_columns(7)
                .spacing([15.0, 4.0])
                .striped(true)
                .show(ui, |ui| {
                    crate::centered_label(ui, "Stat");
                    crate::centered_label(ui, "Base");
                    crate::centered_label(ui, "Preset");
                    crate::centered_label(ui, "Max");
                    crate::centered_label(ui, "Difficulty");
                    crate::centered_label(ui, "Dynamic");
                    crate::centered_label(ui, "Total");
                    ui.end_row();

                    let attrib_names = ["Bravery", "Willpower", "Intuition", "Agility", "Strength"];
                    for (i, name) in attrib_names.iter().enumerate() {
                        ui.label(*name);
                        for j in 0..6 {
                            let val = character.attrib[i][j];
                            crate::centered_label(ui, format!("{}", val));
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
                    crate::centered_label(ui, "Vital");
                    crate::centered_label(ui, "[0]");
                    crate::centered_label(ui, "[1]");
                    crate::centered_label(ui, "[2]");
                    crate::centered_label(ui, "[3]");
                    crate::centered_label(ui, "[4]");
                    crate::centered_label(ui, "[5]");
                    ui.end_row();

                    ui.label("HP");
                    for i in 0..6 {
                        let val = character.hp[i];
                        crate::centered_label(ui, format!("{}", val));
                    }
                    ui.end_row();

                    ui.label("Endurance");
                    for i in 0..6 {
                        let val = character.end[i];
                        crate::centered_label(ui, format!("{}", val));
                    }
                    ui.end_row();

                    ui.label("Mana");
                    for i in 0..6 {
                        let val = character.mana[i];
                        crate::centered_label(ui, format!("{}", val));
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
                    crate::centered_label(ui, format!("{}", a_hp));
                    ui.end_row();

                    ui.label("Active Endurance:");
                    crate::centered_label(ui, format!("{}", a_end));
                    ui.end_row();

                    ui.label("Active Mana:");
                    crate::centered_label(ui, format!("{}", a_mana));
                    ui.end_row();
                });

            ui.separator();
            crate::centered_heading(ui, "Skills");
            egui::Grid::new("character_skills")
                .num_columns(8)
                .spacing([15.0, 4.0])
                .striped(true)
                .show(ui, |ui| {
                    crate::centered_label(ui, "Skill #");
                    crate::centered_label(ui, "Skill Name");
                    crate::centered_label(ui, "[0]");
                    crate::centered_label(ui, "[1]");
                    crate::centered_label(ui, "[2]");
                    crate::centered_label(ui, "[3]");
                    crate::centered_label(ui, "[4]");
                    crate::centered_label(ui, "[5]");
                    ui.end_row();

                    for (i, skill) in character.skill.iter().enumerate() {
                        crate::centered_label(ui, format!("{}", i));
                        ui.label(get_skill_name(i));
                        for j in 0..6 {
                            let val = skill[j];
                            crate::centered_label(ui, format!("{}", val));
                        }
                        ui.end_row();
                    }
                });

            ui.separator();
            crate::centered_heading(ui, "Inventory");
            egui::Grid::new("character_inventory")
                .num_columns(2)
                .spacing([20.0, 4.0])
                .striped(true)
                .show(ui, |ui| {
                    crate::centered_label(ui, "Slot");
                    crate::centered_label(ui, "Item ID");
                    crate::centered_label(ui, "Slot");
                    crate::centered_label(ui, "Item ID");
                    ui.end_row();

                    let item_count = 40; // character.item.len()
                    for i in (0..item_count).step_by(2) {
                        let item1 = character.item[i];
                        let item2 = if i + 1 < item_count {
                            character.item[i + 1]
                        } else {
                            0
                        };

                        crate::centered_label(ui, format!("{}", i));
                        self.centered_clickable_item_id(ui, item1);
                        crate::centered_label(ui, format!("{}", i + 1));
                        self.centered_clickable_item_id(ui, item2);
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
                    crate::centered_label(ui, "Slot");
                    crate::centered_label(ui, "Item ID");
                    ui.end_row();

                    let worn_count = 20;
                    for i in 0..worn_count {
                        let worn_item = character.worn[i];
                        crate::centered_label(ui, format!("{}", i));
                        self.centered_clickable_item_id(ui, worn_item);
                        ui.end_row();
                    }
                });

            ui.separator();
            crate::centered_heading(ui, "Driver Data");
            egui::Grid::new("character_driver_data")
                .num_columns(2)
                .spacing([20.0, 4.0])
                .striped(true)
                .show(ui, |ui| {
                    for i in 0..100 {
                        let data = character.data[i];
                        ui.label(format!("data[{}]:", i));
                        crate::centered_label(ui, format!("{}", data));
                        ui.end_row();
                    }
                });
        });
    }
}

impl eframe::App for TemplateViewerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
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
                        ui.heading(format!("Item Templates ({})", used_count));
                        ui.separator();
                        self.render_item_list(ui);
                    });

                egui::CentralPanel::default().show_inside(ui, |ui| {
                    if let Some(idx) = self.selected_item_index {
                        if idx < self.item_templates.len() {
                            let item = self.item_templates[idx];
                            self.render_item_details(ui, &item);
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
                        ui.heading(format!("Character Templates ({})", used_count));
                        ui.separator();
                        self.render_character_list(ui);
                    });

                egui::CentralPanel::default().show_inside(ui, |ui| {
                    if let Some(idx) = self.selected_character_index {
                        if idx < self.character_templates.len() {
                            let character = self.character_templates[idx];
                            self.render_character_details(ui, &character);
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
                            let item = self.items[idx];
                            self.render_item_details(ui, &item);
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
                            let character = self.characters[idx];
                            self.render_character_details(ui, &character);
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
