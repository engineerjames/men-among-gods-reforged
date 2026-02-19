use std::{sync::mpsc, time::Duration};

use egui_sdl2::egui;
use mag_core::{traits, types::CharacterSummary};
use sdl2::{event::Event, render::Canvas, video::Window};

use crate::{
    account_api,
    scenes::scene::{Scene, SceneType},
    state::AppState,
};

pub struct CharacterSelectionScene {
    last_error: Option<String>,
    is_loading_characters: bool,
    characters: Vec<CharacterSummary>,
    character_textures: Vec<Option<egui::TextureId>>,
    selected_character_id: Option<u64>,

    login_rx: Option<std::sync::mpsc::Receiver<Result<u64, String>>>,
    login_thread: Option<std::thread::JoinHandle<()>>,
    logging_in: bool,
}

impl CharacterSelectionScene {
    pub fn new() -> Self {
        Self {
            last_error: None,
            is_loading_characters: true,

            characters: Vec::new(),
            character_textures: Vec::new(),

            selected_character_id: None,
            login_rx: None,
            login_thread: None,
            logging_in: false,
        }
    }
}

impl Scene for CharacterSelectionScene {
    fn handle_event(&mut self, _app_state: &mut AppState, _event: &Event) -> Option<SceneType> {
        // Handle input events for character selection
        None
    }

    fn update(&mut self, _app_state: &mut AppState, _dt: Duration) -> Option<SceneType> {
        // Update any character selection logic
        None
    }

    fn render_world(
        &mut self,
        _app_state: &mut AppState,
        _canvas: &mut Canvas<Window>,
    ) -> Result<(), String> {
        // Render any character selection background or world elements
        Ok(())
    }

    fn render_ui(&mut self, app_state: &mut AppState, ctx: &egui::Context) -> Option<SceneType> {
        let mut next_scene: Option<SceneType> = None;

        egui::Window::new("Character Selection")
            .default_height(800.0)
            .default_width(600.0)
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.heading("Character selection");

                if let Some(username) = app_state.api.username.as_deref() {
                    ui.label(format!("Logged in as: {username}"));
                } else {
                    ui.label("No account session available");
                }

                if let Some(err) = self.last_error.as_deref() {
                    ui.colored_label(egui::Color32::LIGHT_RED, err);
                }

                ui.add_space(12.0);
                ui.label("Characters");

                if self.is_loading_characters {
                    ui.label("Loading characters...");
                } else if self.characters.is_empty() {
                    ui.label("No characters found");
                } else {
                    egui::ScrollArea::vertical()
                        .max_height(180.0)
                        .show(ui, |ui| {
                            for (index, character) in self.characters.iter().enumerate() {
                                let texture_id =
                                    self.character_textures.get(index).copied().flatten();

                                let _label = format!(
                                    "{} ({}, {})",
                                    character.name,
                                    character.class.to_string(),
                                    character.sex.to_string(),
                                );

                                ui.horizontal(|ui| {
                                    if let Some(texture_id) = texture_id {
                                        let size = egui::vec2(48.0, 48.0);
                                        let textured =
                                            egui::load::SizedTexture::new(texture_id, size);
                                        let img_resp = ui.add(egui::Image::new(textured));
                                        if img_resp.clicked() {
                                            log::info!("Selected character: {}", character.name);
                                            self.selected_character_id = Some(character.id);
                                        }
                                    }
                                });
                            }
                        });
                }

                ui.add_space(12.0);

                if ui
                    .add(egui::Button::new("Create new character").min_size([200.0, 32.0].into()))
                    .clicked()
                {
                    self.last_error = None;
                    next_scene = Some(SceneType::CharacterCreation);
                    return;
                }

                if ui
                    .add(egui::Button::new("Continue to game login").min_size([200.0, 32.0].into()))
                    .clicked()
                {
                    if self.logging_in {
                        self.last_error = Some("Login already in progress".to_string());
                    } else {
                        let Some(token) = app_state.api.token.as_deref() else {
                            self.last_error = Some("Missing account session token".to_string());
                            return;
                        };

                        let Some(character_id) = self.selected_character_id else {
                            self.last_error = Some("Select a character first".to_string());
                            return;
                        };

                        let Some(selected) = self.characters.iter().find(|c| c.id == character_id)
                        else {
                            self.last_error = Some("Selected character not found".to_string());
                            return;
                        };

                        let race_int = traits::get_race_integer(
                            selected.sex == traits::Sex::Male,
                            selected.class,
                        );

                        app_state.api.login_target = Some((character_id, race_int));

                        self.last_error = None;

                        let base_url = app_state.api.base_url.clone();
                        let token = token.to_string();
                        let (tx, rx) = mpsc::channel();
                        self.login_thread = Some(std::thread::spawn(move || {
                            let result = account_api::create_game_login_ticket(
                                &base_url,
                                &token,
                                character_id,
                            );
                            if let Err(err) = tx.send(result) {
                                log::error!("Failed to send login result: {}", err);
                            }
                        }));

                        self.login_rx = Some(rx);
                    }
                }

                if ui
                    .add(egui::Button::new("Log out").min_size([200.0, 32.0].into()))
                    .clicked()
                {
                    app_state.api.token = None;
                    app_state.api.username = None;
                    self.last_error = None;
                    next_scene = Some(SceneType::Login);
                }
            });

        next_scene
    }
}
