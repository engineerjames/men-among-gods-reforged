use std::{
    sync::mpsc::{self, TryRecvError},
    time::Duration,
};

use egui_sdl2::egui::{self, Pos2};
use mag_core::{
    names,
    types::{Class, Sex},
};
use sdl2::{event::Event, pixels::Color, rect::Rect, render::Canvas, video::Window};

use crate::{
    account_api,
    scenes::{
        helpers,
        scene::{Scene, SceneType},
    },
    state::AppState,
};

pub struct CharacterCreationScene {
    error: Option<String>,
    name: String,
    description: String,
    selected_class: Class,
    selected_sex: Sex,
    is_busy: bool,
    account_rx: Option<mpsc::Receiver<Result<account_api::CharacterSummary, String>>>,
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
            account_rx: None,
            account_thread: None,
        }
    }
}

impl Scene for CharacterCreationScene {
    fn handle_event(&mut self, _app_state: &mut AppState, _event: &Event) -> Option<SceneType> {
        // Handle input events for character creation
        None
    }

    fn update(&mut self, _app_state: &mut AppState, _dt: Duration) -> Option<SceneType> {
        if !self.is_busy {
            return None;
        }

        let result = if let Some(receiver) = &self.account_rx {
            match receiver.try_recv() {
                Ok(result) => Some(result),
                Err(TryRecvError::Empty) => None,
                Err(TryRecvError::Disconnected) => {
                    Some(Err("Character creation failed: channel closed".to_string()))
                }
            }
        } else {
            None
        };

        let Some(result) = result else {
            return None;
        };

        self.is_busy = false;
        self.account_rx = None;

        if let Some(thread) = self.account_thread.take() {
            if thread.join().is_err() {
                log::error!("Character creation thread panicked");
            }
        }

        match result {
            Ok(summary) => {
                self.error = None;
                log::info!("Character creation successful: {}", summary.name);
                Some(SceneType::CharacterSelection)
            }
            Err(err) => {
                self.error = Some(err);
                None
            }
        }
    }

    fn on_enter(&mut self, _app_state: &mut AppState) {}

    fn render_world(
        &mut self,
        app_state: &mut AppState,
        canvas: &mut Canvas<Window>,
    ) -> Result<(), String> {
        canvas.set_draw_color(Color::RGB(20, 20, 28));
        canvas.clear();

        let portrait_slots = [
            (Class::Harakim, Rect::new(600, 150, 160, 160)),
            (Class::Templar, Rect::new(600, 320, 160, 160)),
            (Class::Mercenary, Rect::new(600, 490, 160, 160)),
        ];

        for (class, target_rect) in portrait_slots {
            let sprite_id = helpers::get_sprite_id_for_class_and_sex(class, self.selected_sex);
            let texture = app_state.gfx_cache.get_texture(sprite_id);
            if let Err(error) = canvas.copy(texture, None, target_rect) {
                log::error!(
                    "Failed to render portrait for class {:?}, sex {:?} (sprite ID {}): {}",
                    class,
                    self.selected_sex,
                    sprite_id,
                    error
                );
            }

            if class == self.selected_class {
                canvas.set_draw_color(Color::RGB(200, 200, 220));
                if let Err(error) = canvas.draw_rect(target_rect) {
                    log::error!("Failed to draw selected portrait outline: {}", error);
                }
            }
        }

        Ok(())
    }

    fn render_ui(&mut self, app_state: &mut AppState, ctx: &egui::Context) -> Option<SceneType> {
        let mut next = None;

        let username = app_state.api.username.clone();
        let token = app_state.api.token.clone();
        let base_url = app_state.api.base_url.clone();

        egui::Window::new("Create Character")
            .default_height(800.0)
            .default_width(500.0)
            .fixed_pos(Pos2::new(0.0, 0.0))
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.heading("Create character");

                if let Some(username) = username.as_deref() {
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
                        race_option_ui(ui, &mut self.selected_class, Class::Harakim, "Harakim");
                        race_option_ui(ui, &mut self.selected_class, Class::Templar, "Templar");
                        race_option_ui(ui, &mut self.selected_class, Class::Mercenary, "Mercenary");
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

                    let Some(token) = token.as_deref() else {
                        self.error = Some("Missing account session token".to_string());
                        return;
                    };

                    if name.is_empty() {
                        self.error = Some("Character name is required".to_string());
                        return;
                    }

                    self.is_busy = true;
                    self.error = None;

                    let base_url = base_url.clone();
                    let token = token.to_string();
                    let race = self.selected_class;
                    let sex = self.selected_sex;
                    let description = if description.is_empty() {
                        None
                    } else {
                        Some(description)
                    };

                    let (tx, rx) = mpsc::channel();
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
                    self.account_rx = Some(rx);
                }

                if back_clicked {
                    self.error = None;
                    next = Some(SceneType::CharacterSelection);
                }
            });

        next
    }
}

fn race_option_ui(ui: &mut egui::Ui, selected_class: &mut Class, class: Class, label: &str) {
    ui.radio_value(selected_class, class, label);
}
