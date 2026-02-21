use std::{
    sync::mpsc::{self, TryRecvError},
    time::Duration,
};

use egui_sdl2::egui;
use mag_core::{traits, types::CharacterSummary};
use sdl2::pixels::Color;
use sdl2::{event::Event, rect::Rect, render::Canvas, video::Window};

use crate::{
    account_api,
    scenes::{
        helpers,
        scene::{Scene, SceneType},
    },
    state::AppState,
};

pub struct CharacterSelectionScene {
    last_error: Option<String>,
    is_loading_characters: bool,
    characters: Vec<CharacterSummary>,
    character_textures: Vec<Option<egui::TextureId>>,
    selected_character_id: Option<u64>,

    characters_rx: Option<std::sync::mpsc::Receiver<Result<Vec<CharacterSummary>, String>>>,
    characters_thread: Option<std::thread::JoinHandle<()>>,

    login_rx: Option<std::sync::mpsc::Receiver<Result<u64, String>>>,
    login_thread: Option<std::thread::JoinHandle<()>>,
    logging_in: bool,
    pending_race: Option<i32>,
}

impl CharacterSelectionScene {
    pub fn new() -> Self {
        Self {
            last_error: None,
            is_loading_characters: true,

            characters: Vec::new(),
            character_textures: Vec::new(),

            selected_character_id: None,
            characters_rx: None,
            characters_thread: None,
            login_rx: None,
            login_thread: None,
            logging_in: false,
            pending_race: None,
        }
    }

    fn is_thread_running(handle: &Option<std::thread::JoinHandle<()>>) -> bool {
        match handle {
            Some(thread) => !thread.is_finished(),
            None => false,
        }
    }

    fn cleanup_finished_thread(handle: &mut Option<std::thread::JoinHandle<()>>, name: &str) {
        let Some(thread) = handle.take() else {
            return;
        };

        if thread.is_finished() {
            if thread.join().is_err() {
                log::error!("{} thread panicked", name);
            }
            return;
        }

        *handle = Some(thread);
    }

    fn drop_or_join_thread(handle: &mut Option<std::thread::JoinHandle<()>>, name: &str) {
        let Some(thread) = handle.take() else {
            return;
        };

        if thread.is_finished() {
            if thread.join().is_err() {
                log::error!("{} thread panicked", name);
            }
        } else {
            log::warn!(
                "{} thread still running on scene exit; detaching handle",
                name
            );
        }
    }
}

impl Scene for CharacterSelectionScene {
    fn on_enter(&mut self, app_state: &mut AppState) {
        Self::cleanup_finished_thread(&mut self.characters_thread, "character loading");
        Self::cleanup_finished_thread(&mut self.login_thread, "game login");

        if Self::is_thread_running(&self.characters_thread) {
            self.last_error = Some("Character loading already in progress".to_string());
            return;
        }

        self.last_error = None;
        self.is_loading_characters = true;
        self.characters.clear();
        self.character_textures.clear();
        self.selected_character_id = None;
        self.characters_rx = None;

        let Some(token) = app_state.api.token.as_deref() else {
            self.is_loading_characters = false;
            self.last_error = Some("Missing account session token".to_string());
            return;
        };

        let base_url = app_state.api.base_url.clone();
        let token = token.to_string();
        let (tx, rx) = mpsc::channel();
        self.characters_thread = Some(std::thread::spawn(move || {
            let result = account_api::get_characters(&base_url, &token);
            if let Err(err) = tx.send(result) {
                log::error!("Failed to send characters result: {}", err);
            }
        }));
        self.characters_rx = Some(rx);
    }

    fn on_exit(&mut self, _app_state: &mut AppState) {
        self.characters_rx = None;
        self.login_rx = None;
        self.is_loading_characters = false;
        self.logging_in = false;
        self.pending_race = None;

        Self::drop_or_join_thread(&mut self.characters_thread, "character loading");
        Self::drop_or_join_thread(&mut self.login_thread, "game login");
    }

    fn handle_event(&mut self, _app_state: &mut AppState, _event: &Event) -> Option<SceneType> {
        // Handle input events for character selection
        None
    }

    fn update(&mut self, app_state: &mut AppState, _dt: Duration) -> Option<SceneType> {
        Self::cleanup_finished_thread(&mut self.characters_thread, "character loading");
        Self::cleanup_finished_thread(&mut self.login_thread, "game login");

        if self.is_loading_characters {
            let result = if let Some(receiver) = &self.characters_rx {
                match receiver.try_recv() {
                    Ok(result) => Some(result),
                    Err(TryRecvError::Empty) => None,
                    Err(TryRecvError::Disconnected) => {
                        Some(Err("Character load task failed unexpectedly".to_string()))
                    }
                }
            } else {
                None
            };

            if let Some(result) = result {
                self.is_loading_characters = false;
                self.characters_rx = None;

                match result {
                    Ok(characters) => {
                        log::info!("Loaded {} characters", characters.len());
                        self.selected_character_id = characters.first().map(|c| c.id);
                        self.character_textures = vec![None; characters.len()];
                        self.characters = characters;
                        self.last_error = None;
                    }
                    Err(error) => {
                        log::error!("Failed to load characters: {}", error);
                        self.characters.clear();
                        self.character_textures.clear();
                        self.selected_character_id = None;
                        self.last_error = Some(error);
                    }
                }
            }
        }

        if self.logging_in {
            let result = if let Some(receiver) = &self.login_rx {
                match receiver.try_recv() {
                    Ok(result) => Some(result),
                    Err(TryRecvError::Empty) => None,
                    Err(TryRecvError::Disconnected) => {
                        Some(Err("Game login task failed unexpectedly".to_string()))
                    }
                }
            } else {
                None
            };

            if let Some(result) = result {
                self.logging_in = false;
                self.login_rx = None;

                match result {
                    Ok(ticket) => {
                        log::info!("Created game login ticket {}", ticket);
                        app_state.api.login_target =
                            Some((ticket, self.pending_race.unwrap_or(0)));
                        self.pending_race = None;
                        return Some(SceneType::Game);
                    }
                    Err(error) => {
                        log::error!("Failed to create game login ticket: {}", error);
                        self.last_error = Some(error);
                    }
                }
            }
        }

        None
    }

    fn render_world(
        &mut self,
        app_state: &mut AppState,
        canvas: &mut Canvas<Window>,
    ) -> Result<(), String> {
        canvas.set_draw_color(Color::RGB(20, 20, 28));
        canvas.clear();

        let Some(selected_character_id) = self.selected_character_id else {
            return Ok(());
        };

        let Some(selected_character) = self
            .characters
            .iter()
            .find(|character| character.id == selected_character_id)
        else {
            return Ok(());
        };

        let sprite_id = helpers::get_sprite_id_for_class_and_sex(
            selected_character.class,
            selected_character.sex,
        );
        let texture = app_state.gfx_cache.get_texture(sprite_id);
        let target_rect = Rect::new(600, 260, 160, 160);

        if let Err(error) = canvas.copy(texture, None, target_rect) {
            log::error!(
                "Failed to render selected portrait for class {:?}, sex {:?} (sprite ID {}): {}",
                selected_character.class,
                selected_character.sex,
                sprite_id,
                error
            );
        }

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
                            let mut character_selected_this_frame = self.selected_character_id;

                            for (index, character) in self.characters.iter().enumerate() {
                                let texture_id =
                                    self.character_textures.get(index).copied().flatten();

                                let label =
                                    format!("{} ({})", character.name, character.class.to_string());
                                let selected = character_selected_this_frame == Some(character.id);

                                ui.horizontal(|ui| {
                                    if let Some(texture_id) = texture_id {
                                        let size = egui::vec2(48.0, 48.0);
                                        let textured =
                                            egui::load::SizedTexture::new(texture_id, size);
                                        let img_resp = ui.add(egui::Image::new(textured));
                                        if img_resp.clicked() {
                                            log::info!("Selected character: {}", character.name);
                                            character_selected_this_frame = Some(character.id);
                                        }
                                    }

                                    let resp = ui.selectable_label(selected, &label);
                                    if resp.clicked() {
                                        log::info!("Selected character: {}", character.name);
                                        character_selected_this_frame = Some(character.id);
                                    }
                                });
                            }

                            self.selected_character_id = character_selected_this_frame;
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
                    if self.logging_in || Self::is_thread_running(&self.login_thread) {
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

                        self.pending_race = Some(race_int);

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
                        self.logging_in = true;
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
