use eframe::egui;
use mag_core::string_operations::c_string_to_str;
use std::fs;
use std::path::PathBuf;

fn main() -> Result<(), eframe::Error> {
    env_logger::init();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 800.0])
            .with_title("Template Viewer"),
        ..Default::default()
    };

    eframe::run_native(
        "Template Viewer",
        options,
        Box::new(|_cc| Ok(Box::new(TemplateViewerApp::default()))),
    )
}

struct TemplateViewerApp {
    item_templates: Vec<mag_core::types::Item>,
    character_templates: Vec<mag_core::types::Character>,
    selected_item_index: Option<usize>,
    selected_character_index: Option<usize>,
    view_mode: ViewMode,
    item_filter: String,
    character_filter: String,
    load_error: Option<String>,
}

#[derive(PartialEq)]
enum ViewMode {
    Items,
    Characters,
}

impl Default for TemplateViewerApp {
    fn default() -> Self {
        Self {
            item_templates: Vec::new(),
            character_templates: Vec::new(),
            selected_item_index: None,
            selected_character_index: None,
            view_mode: ViewMode::Items,
            item_filter: String::new(),
            character_filter: String::new(),
            load_error: None,
        }
    }
}

impl TemplateViewerApp {
    fn load_item_templates_from_file(&mut self, path: PathBuf) {
        self.load_error = None;
        log::info!("Loading item templates from {:?}", path);

        match self.load_item_templates(&path) {
            Ok(items) => {
                self.item_templates = items;
                self.view_mode = ViewMode::Items;
                log::info!("Loaded {} item templates", self.item_templates.len());
            }
            Err(e) => {
                self.load_error = Some(format!("Failed to load item templates: {}", e));
                log::error!("Failed to load item templates: {}", e);
            }
        }
    }

    fn load_character_templates_from_file(&mut self, path: PathBuf) {
        self.load_error = None;
        log::info!("Loading character templates from {:?}", path);

        match self.load_character_templates(&path) {
            Ok(chars) => {
                self.character_templates = chars;
                self.view_mode = ViewMode::Characters;
                log::info!(
                    "Loaded {} character templates",
                    self.character_templates.len()
                );
            }
            Err(e) => {
                self.load_error = Some(format!("Failed to load character templates: {}", e));
                log::error!("Failed to load character templates: {}", e);
            }
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

    fn render_item_list(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("Filter:");
            ui.text_edit_singleline(&mut self.item_filter);
        });

        ui.separator();

        egui::ScrollArea::vertical().show(ui, |ui| {
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

    fn render_character_list(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("Filter:");
            ui.text_edit_singleline(&mut self.character_filter);
        });

        ui.separator();

        egui::ScrollArea::vertical().show(ui, |ui| {
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

    fn render_item_details(&self, ui: &mut egui::Ui, item: &mag_core::types::Item) {
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
                    ui.label(format!("{}", temp));
                    ui.end_row();

                    ui.label("Used:");
                    ui.label(format!("{}", used));
                    ui.end_row();

                    ui.label("Reference:");
                    ui.label(c_string_to_str(&item.reference));
                    ui.end_row();

                    ui.label("Description:");
                    ui.add(egui::Label::new(c_string_to_str(&item.description)).wrap());
                    ui.end_row();

                    ui.label("Value:");
                    ui.label(format_gold_silver(value as i32));
                    ui.end_row();

                    ui.label("Placement:");
                    ui.label(format!("{}", placement));
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
                    for (flag, name) in get_item_flag_info() {
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
                    ui.label("Sprite:");
                    ui.label(format!("[{}, {}]", sprite_0, sprite_1));
                    ui.end_row();

                    ui.label("Status:");
                    ui.label(format!("[{}, {}]", status_0, status_1));
                    ui.end_row();

                    ui.label("Armor:");
                    ui.label(format!("[{}, {}]", armor_0, armor_1));
                    ui.end_row();

                    ui.label("Weapon:");
                    ui.label(format!("[{}, {}]", weapon_0, weapon_1));
                    ui.end_row();

                    ui.label("Light:");
                    ui.label(format!("[{}, {}]", light_0, light_1));
                    ui.end_row();

                    ui.label("Duration:");
                    ui.label(format!("{}", duration));
                    ui.end_row();

                    ui.label("Cost:");
                    ui.label(format!("{}", cost));
                    ui.end_row();

                    ui.label("Power:");
                    ui.label(format!("{}", power));
                    ui.end_row();

                    ui.label("Min Rank:");
                    ui.label(format!("{}", min_rank));
                    ui.end_row();

                    ui.label("Driver:");
                    ui.label(format!("{}", driver));
                    ui.end_row();
                });

            ui.separator();
            ui.heading("Attributes");
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

                    let attrib_names = ["Strength", "Intuition", "Agility", "Wisdom", "Hitpoints"];
                    for (i, name) in attrib_names.iter().enumerate() {
                        let val_0 = item.attrib[i][0];
                        let val_1 = item.attrib[i][1];
                        let val_2 = item.attrib[i][2];
                        ui.label(*name);
                        ui.label(format!("{:+}", val_0));
                        ui.label(format!("{:+}", val_1));
                        ui.label(format!("{}", val_2));
                        ui.end_row();
                    }

                    let hp_0 = item.hp[0];
                    let hp_1 = item.hp[1];
                    let hp_2 = item.hp[2];
                    ui.label("HP");
                    ui.label(format!("{:+}", hp_0));
                    ui.label(format!("{:+}", hp_1));
                    ui.label(format!("{}", hp_2));
                    ui.end_row();

                    let end_0 = item.end[0];
                    let end_1 = item.end[1];
                    let end_2 = item.end[2];
                    ui.label("Endurance");
                    ui.label(format!("{:+}", end_0));
                    ui.label(format!("{:+}", end_1));
                    ui.label(format!("{}", end_2));
                    ui.end_row();

                    let mana_0 = item.mana[0];
                    let mana_1 = item.mana[1];
                    let mana_2 = item.mana[2];
                    ui.label("Mana");
                    ui.label(format!("{:+}", mana_0));
                    ui.label(format!("{:+}", mana_1));
                    ui.label(format!("{}", mana_2));
                    ui.end_row();
                });

            ui.separator();
            ui.heading("Skills");
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

                    for (i, skill) in item.skill.iter().enumerate() {
                        let s0 = skill[0];
                        let s1 = skill[1];
                        let s2 = skill[2];
                        if s0 != 0 || s1 != 0 || s2 != 0 {
                            ui.label(format!("{}", i));
                            ui.label(get_skill_name(i));
                            ui.label(format!("{:+}", s0));
                            ui.label(format!("{:+}", s1));
                            ui.label(format!("{}", s2));
                            ui.end_row();
                        }
                    }
                });

            ui.separator();
            ui.heading("Driver Data");
            egui::Grid::new("item_driver_data")
                .num_columns(2)
                .spacing([40.0, 4.0])
                .striped(true)
                .show(ui, |ui| {
                    for i in 0..10 {
                        let data = item.data[i];
                        if data != 0 {
                            ui.label(format!("data[{}]:", i));
                            ui.label(format!("{}", data));
                            ui.end_row();
                        }
                    }
                });
        });
    }

    fn render_character_details(&self, ui: &mut egui::Ui, character: &mag_core::types::Character) {
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
                    ui.label(format!("{}", temp));
                    ui.end_row();

                    ui.label("Used:");
                    ui.label(format!("{}", used));
                    ui.end_row();

                    ui.label("Reference:");
                    ui.label(character.get_reference());
                    ui.end_row();

                    ui.label("Description:");
                    ui.add(egui::Label::new(c_string_to_str(&character.description)).wrap());
                    ui.end_row();

                    ui.label("Kindred:");
                    ui.label(format!("{}", kindred));
                    ui.end_row();

                    ui.label("Sprite:");
                    ui.label(format!("{}", sprite));
                    ui.end_row();

                    ui.label("Sound:");
                    ui.label(format!("{}", sound));
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
                    for flag in get_character_flag_info() {
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
                    ui.label(format!("{}", alignment));
                    ui.end_row();

                    ui.label("Temple:");
                    ui.label(format!("({}, {})", temple_x, temple_y));
                    ui.end_row();

                    ui.label("Tavern:");
                    ui.label(format!("({}, {})", tavern_x, tavern_y));
                    ui.end_row();

                    ui.label("Position:");
                    ui.label(format!("({}, {})", x, y));
                    ui.end_row();

                    ui.label("Gold:");
                    ui.label(format_gold_silver(gold));
                    ui.end_row();

                    ui.label("Points:");
                    ui.label(format!("{} / {}", points, points_tot));
                    ui.end_row();

                    ui.label("Armor:");
                    ui.label(format!("{}", armor));
                    ui.end_row();

                    ui.label("Weapon:");
                    ui.label(format!("{}", weapon));
                    ui.end_row();

                    ui.label("Light:");
                    ui.label(format!("{}", light));
                    ui.end_row();

                    ui.label("Mode:");
                    ui.label(format!("{}", mode));
                    ui.end_row();

                    ui.label("Speed:");
                    ui.label(format!("{}", speed));
                    ui.end_row();

                    ui.label("Monster Class:");
                    ui.label(format!("{}", monster_class));
                    ui.end_row();
                });

            ui.separator();
            ui.heading("Attributes");
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

                    let attrib_names = ["Strength", "Intuition", "Agility", "Wisdom", "Hitpoints"];
                    for (i, name) in attrib_names.iter().enumerate() {
                        ui.label(*name);
                        for j in 0..6 {
                            let val = character.attrib[i][j];
                            ui.label(format!("{}", val));
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
                        let val = character.hp[i];
                        ui.label(format!("{}", val));
                    }
                    ui.end_row();

                    ui.label("Endurance");
                    for i in 0..6 {
                        let val = character.end[i];
                        ui.label(format!("{}", val));
                    }
                    ui.end_row();

                    ui.label("Mana");
                    for i in 0..6 {
                        let val = character.mana[i];
                        ui.label(format!("{}", val));
                    }
                    ui.end_row();
                });

            ui.separator();
            ui.heading("Active Values");
            egui::Grid::new("character_active")
                .num_columns(2)
                .spacing([40.0, 4.0])
                .striped(true)
                .show(ui, |ui| {
                    ui.label("Active HP:");
                    ui.label(format!("{}", a_hp));
                    ui.end_row();

                    ui.label("Active Endurance:");
                    ui.label(format!("{}", a_end));
                    ui.end_row();

                    ui.label("Active Mana:");
                    ui.label(format!("{}", a_mana));
                    ui.end_row();
                });

            ui.separator();
            ui.heading("Skills (Non-Zero Only)");
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

                    for (i, skill) in character.skill.iter().enumerate() {
                        if skill.iter().any(|&s| s != 0) {
                            ui.label(format!("{}", i));
                            ui.label(get_skill_name(i));
                            for j in 0..6 {
                                let val = skill[j];
                                ui.label(format!("{}", val));
                            }
                            ui.end_row();
                        }
                    }
                });

            ui.separator();
            ui.heading("Inventory (Non-Zero Only)");
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
                        let item1 = character.item[i];
                        let item2 = if i + 1 < item_count {
                            character.item[i + 1]
                        } else {
                            0
                        };

                        if item1 != 0 || item2 != 0 {
                            if item1 != 0 {
                                ui.label(format!("{}", i));
                                ui.label(format!("{}", item1));
                            } else {
                                ui.label("");
                                ui.label("");
                            }
                            if item2 != 0 {
                                ui.label(format!("{}", i + 1));
                                ui.label(format!("{}", item2));
                            } else {
                                ui.label("");
                                ui.label("");
                            }
                            ui.end_row();
                        }
                    }
                });

            ui.separator();
            ui.heading("Worn Equipment (Non-Zero Only)");
            egui::Grid::new("character_worn")
                .num_columns(4)
                .spacing([20.0, 4.0])
                .striped(true)
                .show(ui, |ui| {
                    ui.label("Slot");
                    ui.label("Item ID");
                    ui.label("Slot");
                    ui.label("Item ID");
                    ui.end_row();

                    let worn_count = 20;
                    for i in (0..worn_count).step_by(2) {
                        let worn1 = character.worn[i];
                        let worn2 = if i + 1 < worn_count {
                            character.worn[i + 1]
                        } else {
                            0
                        };

                        if worn1 != 0 || worn2 != 0 {
                            if worn1 != 0 {
                                ui.label(format!("{}", i));
                                ui.label(format!("{}", worn1));
                            } else {
                                ui.label("");
                                ui.label("");
                            }
                            if worn2 != 0 {
                                ui.label(format!("{}", i + 1));
                                ui.label(format!("{}", worn2));
                            } else {
                                ui.label("");
                                ui.label("");
                            }
                            ui.end_row();
                        }
                    }
                });

            ui.separator();
            ui.heading("Driver Data (Non-Zero Only)");
            egui::Grid::new("character_driver_data")
                .num_columns(4)
                .spacing([20.0, 4.0])
                .striped(true)
                .show(ui, |ui| {
                    for i in (0..100).step_by(2) {
                        let data1 = character.data[i];
                        let data2 = if i + 1 < 100 {
                            character.data[i + 1]
                        } else {
                            0
                        };

                        if data1 != 0 || data2 != 0 {
                            if data1 != 0 {
                                ui.label(format!("data[{}]:", i));
                                ui.label(format!("{}", data1));
                            } else {
                                ui.label("");
                                ui.label("");
                            }

                            if data2 != 0 {
                                ui.label(format!("data[{}]:", i + 1));
                                ui.label(format!("{}", data2));
                            } else {
                                ui.label("");
                                ui.label("");
                            }
                            ui.end_row();
                        }
                    }
                });
        });
    }
}

// Helper function to format currency as gold and silver
fn format_gold_silver(value: i32) -> String {
    let gold = value / 1000;
    let silver = value % 1000;
    if gold > 0 && silver > 0 {
        format!("{} gold, {} silver", gold, silver)
    } else if gold > 0 {
        format!("{} gold", gold)
    } else {
        format!("{} silver", silver)
    }
}

// Helper function to get skill name from skill index
fn get_skill_name(skill_num: usize) -> &'static str {
    match skill_num {
        0 => "Hand",
        1 => "Karate",
        2 => "Sword",
        3 => "Dagger",
        4 => "Staff",
        5 => "Two-Handed",
        6 => "Axe",
        7 => "Whip",
        8 => "Shield",
        9 => "Attack",
        10 => "Parry",
        11 => "Warcry",
        12 => "Tactics",
        13 => "Stealth",
        14 => "Perception",
        15 => "Bravery",
        16 => "Willpower",
        17 => "Endurance",
        18 => "Hitpoints",
        19 => "Mana",
        20 => "Fire Magic",
        21 => "Freeze Magic",
        22 => "Magic Shield",
        23 => "Healing",
        24 => "Ghost",
        25 => "Blast",
        26 => "Lightning",
        27 => "Hail",
        28 => "Regenerate",
        29 => "Meditate",
        30 => "Light",
        31 => "Magicshield",
        32 => "Bless",
        33 => "Warcry Alt",
        34 => "Rest",
        35 => "Pray",
        _ => "Unknown",
    }
}

fn get_item_flag_info() -> Vec<(mag_core::constants::ItemFlags, &'static str)> {
    use mag_core::constants::ItemFlags;
    vec![
        (ItemFlags::IF_MOVEBLOCK, "Move Block"),
        (ItemFlags::IF_SIGHTBLOCK, "Sight Block"),
        (ItemFlags::IF_TAKE, "Take"),
        (ItemFlags::IF_MONEY, "Money"),
        (ItemFlags::IF_LOOK, "Look"),
        (ItemFlags::IF_LOOKSPECIAL, "Look Special"),
        (ItemFlags::IF_SPELL, "Spell"),
        (ItemFlags::IF_NOREPAIR, "No Repair"),
        (ItemFlags::IF_ARMOR, "Armor"),
        (ItemFlags::IF_USE, "Use"),
        (ItemFlags::IF_USESPECIAL, "Use Special"),
        (ItemFlags::IF_SINGLEAGE, "Single Age"),
        (ItemFlags::IF_SHOPDESTROY, "Shop Destroy"),
        (ItemFlags::IF_UPDATE, "Update"),
        (ItemFlags::IF_ALWAYSEXP1, "Always Expire 1"),
        (ItemFlags::IF_ALWAYSEXP2, "Always Expire 2"),
        (ItemFlags::IF_WP_SWORD, "Weapon Sword"),
        (ItemFlags::IF_WP_DAGGER, "Weapon Dagger"),
        (ItemFlags::IF_WP_AXE, "Weapon Axe"),
        (ItemFlags::IF_WP_STAFF, "Weapon Staff"),
        (ItemFlags::IF_WP_TWOHAND, "Weapon Two-Hand"),
        (ItemFlags::IF_USEDESTROY, "Use Destroy"),
        (ItemFlags::IF_USEACTIVATE, "Use Activate"),
        (ItemFlags::IF_USEDEACTIVATE, "Use Deactivate"),
        (ItemFlags::IF_MAGIC, "Magic"),
        (ItemFlags::IF_MISC, "Misc"),
        (ItemFlags::IF_REACTIVATE, "Reactivate"),
        (ItemFlags::IF_PERMSPELL, "Perm Spell"),
        (ItemFlags::IF_UNIQUE, "Unique"),
        (ItemFlags::IF_DONATE, "Donate"),
        (ItemFlags::IF_LABYDESTROY, "Laby Destroy"),
        (ItemFlags::IF_NOMARKET, "No Market"),
        (ItemFlags::IF_HIDDEN, "Hidden"),
        (ItemFlags::IF_STEPACTION, "Step Action"),
        (ItemFlags::IF_NODEPOT, "No Depot"),
        (ItemFlags::IF_LIGHTAGE, "Light Age"),
        (ItemFlags::IF_EXPIREPROC, "Expire Proc"),
        (ItemFlags::IF_IDENTIFIED, "Identified"),
        (ItemFlags::IF_NOEXPIRE, "No Expire"),
        (ItemFlags::IF_SOULSTONE, "Soulstone"),
    ]
}

fn get_character_flag_info() -> Vec<mag_core::constants::CharacterFlags> {
    use mag_core::constants::CharacterFlags;
    vec![
        CharacterFlags::Immortal,
        CharacterFlags::God,
        CharacterFlags::Creator,
        CharacterFlags::BuildMode,
        CharacterFlags::Respawn,
        CharacterFlags::Player,
        CharacterFlags::NewUser,
        CharacterFlags::NoTell,
        CharacterFlags::NoShout,
        CharacterFlags::Merchant,
        CharacterFlags::Staff,
        CharacterFlags::NoHpReg,
        CharacterFlags::NoEndReg,
        CharacterFlags::NoManaReg,
        CharacterFlags::Invisible,
        CharacterFlags::Infrared,
        CharacterFlags::Body,
        CharacterFlags::NoSleep,
        CharacterFlags::Undead,
        CharacterFlags::NoMagic,
        CharacterFlags::Stoned,
        CharacterFlags::Usurp,
        CharacterFlags::Imp,
        CharacterFlags::ShutUp,
        CharacterFlags::NoDesc,
        CharacterFlags::Profile,
        CharacterFlags::Simple,
        CharacterFlags::Kicked,
        CharacterFlags::NoList,
        CharacterFlags::NoWho,
        CharacterFlags::SpellIgnore,
        CharacterFlags::ComputerControlledPlayer,
        CharacterFlags::Safe,
        CharacterFlags::NoStaff,
        CharacterFlags::Poh,
        CharacterFlags::PohLeader,
        CharacterFlags::Thrall,
        CharacterFlags::LabKeeper,
        CharacterFlags::IsLooting,
        CharacterFlags::Golden,
        CharacterFlags::Black,
        CharacterFlags::Passwd,
        CharacterFlags::Update,
        CharacterFlags::SaveMe,
        CharacterFlags::GreaterGod,
        CharacterFlags::GreaterInv,
    ]
}

impl eframe::App for TemplateViewerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Open Item Templates...").clicked() {
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter("DAT files", &["dat"])
                            .set_can_create_directories(true)
                            .pick_file()
                        {
                            self.load_item_templates_from_file(path);
                        }
                        ui.close_menu();
                    }
                    if ui.button("Open Character Templates...").clicked() {
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter("DAT files", &["dat"])
                            .set_can_create_directories(true)
                            .pick_file()
                        {
                            self.load_character_templates_from_file(path);
                        }
                        ui.close_menu();
                    }

                    ui.separator();

                    if ui.button("Exit").clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });

                ui.separator();

                if ui
                    .selectable_label(self.view_mode == ViewMode::Items, "Item Templates")
                    .clicked()
                {
                    self.view_mode = ViewMode::Items;
                }
                if ui
                    .selectable_label(
                        self.view_mode == ViewMode::Characters,
                        "Character Templates",
                    )
                    .clicked()
                {
                    self.view_mode = ViewMode::Characters;
                }
            });

            if let Some(ref error) = self.load_error {
                ui.separator();
                ui.colored_label(egui::Color32::RED, format!("Error: {}", error));
            }
        });

        egui::CentralPanel::default().show(ctx, |ui| match self.view_mode {
            ViewMode::Items => {
                egui::SidePanel::left("item_list")
                    .resizable(true)
                    .default_width(300.0)
                    .show_inside(ui, |ui| {
                        ui.heading(format!("Item Templates ({})", self.item_templates.len()));
                        ui.separator();
                        self.render_item_list(ui);
                    });

                egui::CentralPanel::default().show_inside(ui, |ui| {
                    if let Some(idx) = self.selected_item_index {
                        if idx < self.item_templates.len() {
                            self.render_item_details(ui, &self.item_templates[idx]);
                        }
                    } else {
                        ui.centered_and_justified(|ui| {
                            ui.label("Select an item template from the list");
                        });
                    }
                });
            }
            ViewMode::Characters => {
                egui::SidePanel::left("character_list")
                    .resizable(true)
                    .default_width(300.0)
                    .show_inside(ui, |ui| {
                        ui.heading(format!(
                            "Character Templates ({})",
                            self.character_templates.len()
                        ));
                        ui.separator();
                        self.render_character_list(ui);
                    });

                egui::CentralPanel::default().show_inside(ui, |ui| {
                    if let Some(idx) = self.selected_character_index {
                        if idx < self.character_templates.len() {
                            self.render_character_details(ui, &self.character_templates[idx]);
                        }
                    } else {
                        ui.centered_and_justified(|ui| {
                            ui.label("Select a character template from the list");
                        });
                    }
                });
            }
        });
    }
}
