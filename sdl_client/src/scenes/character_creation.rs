use std::{
    sync::{mpsc, Arc, Mutex},
    time::Duration,
};

use egui_sdl2::egui::{self, Pos2};
use mag_core::{
    names,
    types::{Class, Sex},
};
use sdl2::{event::Event, render::Canvas, video::Window};

use crate::{
    account_api,
    scenes::scene::{Scene, SceneType},
};

pub struct CharacterCreationScene {
    error: Option<String>,
    name: String,
    description: String,
    selected_class: Class,
    selected_sex: Sex,
    is_busy: bool,
    account_thread: Option<std::thread::JoinHandle<()>>,
}

impl CharacterCreationScene {
    pub fn new() -> Self {
        Self {
            error: None,
            name: String::new(),
            description: String::new(),
            selected_class: Class::Mercenary,
            selected_sex: Sex::Male,
            is_busy: false,
            account_thread: None,
        }
    }
}

impl Scene for CharacterCreationScene {
    fn handle_event(&mut self, _event: &Event) -> Option<SceneType> {
        // Handle input events for character creation
        None
    }

    fn update(&mut self, _dt: Duration) -> Option<SceneType> {
        // Update any character creation logic
        None
    }

    fn render_world(&mut self, _canvas: &mut Canvas<Window>) -> Result<(), String> {
        // Render any character creation background or world elements
        Ok(())
    }

    fn render_ui(&mut self, ctx: &egui::Context) -> Option<SceneType> {
        let mut next = None;

        egui::Window::new("Create Character")
            .default_height(800.0)
            .default_width(600.0)
            .fixed_pos(Pos2::new(0.0, 0.0))
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.heading("Create character");

                if let Some(username) = api_session.username.as_deref() {
                    ui.label(format!("Logged in as: {username}"));
                } else {
                    ui.label("No account session available");
                }

                if let Some(err) = self.error.as_deref() {
                    ui.colored_label(egui::Color32::LIGHT_RED, err);
                }

                ui.add_space(12.0);
                ui.label("Name");
                ui.add_enabled(
                    !self.is_busy,
                    egui::TextEdit::singleline(&mut self.name).desired_width(260.0),
                );

                if ui
                    .add_enabled(!self.is_busy, egui::Button::new("Random name"))
                    .clicked()
                {
                    self.name = names::randomly_generate_name();
                }

                ui.add_space(8.0);
                ui.label("Description");
                ui.add_enabled(
                    !self.is_busy,
                    egui::TextEdit::multiline(&mut self.description)
                        .desired_rows(3)
                        .desired_width(260.0),
                );

                ui.add_space(12.0);
                ui.label("Race");

                ui.group(|ui| {
                    ui.vertical(|ui| {
                        race_option_ui(
                            ui,
                            &mut self.selected_class,
                            Class::Harakim,
                            "Harakim",
                            harakim_texture,
                        );
                        race_option_ui(
                            ui,
                            &mut self.selected_class,
                            Class::Templar,
                            "Templar",
                            templar_texture,
                        );
                        race_option_ui(
                            ui,
                            &mut self.selected_class,
                            Class::Mercenary,
                            "Mercenary",
                            mercenary_texture,
                        );
                    });
                });

                ui.add_space(12.0);
                ui.label("Sex");

                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        ui.radio_value(&mut self.selected_sex, Sex::Male, "Male");
                        ui.radio_value(&mut self.selected_sex, Sex::Female, "Female");
                    });
                });

                ui.add_space(16.0);

                let create_clicked = ui
                    .add_enabled(
                        !self.is_busy,
                        egui::Button::new("Create character").min_size([180.0, 32.0].into()),
                    )
                    .clicked();

                let back_clicked = ui
                    .add_enabled(
                        !self.is_busy,
                        egui::Button::new("Back").min_size([180.0, 32.0].into()),
                    )
                    .clicked();

                if create_clicked {
                    let name = self.name.trim().to_string();
                    let description = self.description.trim().to_string();

                    let Some(token) = api_session.token.as_deref() else {
                        self.error = Some("Missing account session token".to_string());
                        return;
                    };

                    if name.is_empty() {
                        self.error = Some("Character name is required".to_string());
                        return;
                    }

                    self.is_busy = true;
                    self.error = None;

                    let base_url = api_session.base_url.clone();
                    let token = token.to_string();
                    let race = self.selected_class;
                    let sex = self.selected_sex;
                    let description = if description.is_empty() {
                        None
                    } else {
                        Some(description)
                    };

                    let (tx, rx) = mpsc::channel();
                    let rx = Arc::new(Mutex::new(rx));
                    self.account_thread = Some(std::thread::spawn(move || {
                        let result = account_api::create_character(
                            &base_url,
                            &token,
                            &name,
                            description.as_deref(),
                            sex,
                            race,
                        );
                        let _ = tx.send(result);
                    }));
                }

                if back_clicked {
                    self.error = None;
                    next = Some(SceneType::CharacterSelection);
                }
            });

        next
    }
}

fn race_option_ui(
    ui: &mut egui::Ui,
    selected_class: &mut Class,
    class: Class,
    label: &str,
    texture_id: Option<egui::TextureId>,
) {
    ui.horizontal(|ui| {
        ui.radio_value(selected_class, class, label);

        if let Some(texture_id) = texture_id {
            let size = egui::vec2(64.0, 64.0);
            let textured = egui::load::SizedTexture::new(texture_id, size);
            ui.add(egui::Image::new(textured));
        } else {
            ui.label("Image missing");
        }
    });
}
