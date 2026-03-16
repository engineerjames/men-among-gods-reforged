use std::{
    sync::mpsc::{self, TryRecvError},
    time::Duration,
};

use mag_core::{traits, types::CharacterSummary};
use sdl2::{event::Event, keyboard::Mod, rect::Rect, render::Canvas, video::Window};

use crate::{
    account_api,
    scenes::scene::{Scene, SceneType},
    state::{AppState, GameLoginTarget},
    ui::{
        self,
        character_selection_form::{CharacterSelectionForm, CharacterSelectionFormAction},
        delete_character_dialog::{DeleteCharacterDialog, DeleteCharacterDialogAction},
        scrollable_list::ListItem,
        widget::{KeyModifiers, Widget},
        RenderContext,
    },
};

/// Scene that lists the player's characters and lets them pick one to enter the game.
///
/// On enter, character summaries are loaded from the API on a background thread.
/// After the player selects a character and clicks "Continue", a game-login
/// ticket is created (also on a background thread) and the scene transitions
/// to `SceneType::Game`.
pub struct CharacterSelectionScene {
    last_error: Option<String>,
    is_loading_characters: bool,
    characters: Vec<CharacterSummary>,
    selected_character_id: Option<u64>,

    /// The selection form widget.
    form: CharacterSelectionForm,
    /// The delete confirmation dialog widget.
    delete_dialog: DeleteCharacterDialog,

    deleting_character: bool,

    characters_rx: Option<std::sync::mpsc::Receiver<Result<Vec<CharacterSummary>, String>>>,
    characters_thread: Option<std::thread::JoinHandle<()>>,

    login_rx: Option<std::sync::mpsc::Receiver<Result<u64, String>>>,
    login_thread: Option<std::thread::JoinHandle<()>>,

    delete_rx: Option<std::sync::mpsc::Receiver<Result<(), String>>>,
    delete_thread: Option<std::thread::JoinHandle<()>>,

    logging_in: bool,
    pending_race: Option<i32>,
    pending_delete_character_id: Option<u64>,
    pending_delete_character_name: Option<String>,
    pending_scene: Option<SceneType>,

    mouse_x: i32,
    mouse_y: i32,
}

impl CharacterSelectionScene {
    /// Creates a new `CharacterSelectionScene` with empty state.
    pub fn new() -> Self {
        Self {
            last_error: None,
            is_loading_characters: true,

            characters: Vec::new(),

            selected_character_id: None,
            deleting_character: false,

            form: CharacterSelectionForm::new(),
            delete_dialog: DeleteCharacterDialog::new(),

            characters_rx: None,
            characters_thread: None,
            login_rx: None,
            login_thread: None,
            delete_rx: None,
            delete_thread: None,
            logging_in: false,
            pending_race: None,
            pending_delete_character_id: None,
            pending_delete_character_name: None,
            pending_scene: None,
            mouse_x: 0,
            mouse_y: 0,
        }
    }

    /// Returns `true` if the given thread handle exists and has not yet finished.
    fn is_thread_running(handle: &Option<std::thread::JoinHandle<()>>) -> bool {
        match handle {
            Some(thread) => !thread.is_finished(),
            None => false,
        }
    }

    /// Joins a finished thread and clears its handle, logging on panic.
    /// If the thread is still running, the handle is left in place.
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

    /// Joins a finished thread or detaches a still-running one on scene exit.
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
    fn on_enter(&mut self, app_state: &mut AppState<'_>) {
        Self::cleanup_finished_thread(&mut self.characters_thread, "character loading");
        Self::cleanup_finished_thread(&mut self.login_thread, "game login");
        Self::cleanup_finished_thread(&mut self.delete_thread, "character delete");

        if Self::is_thread_running(&self.characters_thread) {
            self.last_error = Some("Character loading already in progress".to_string());
            return;
        }

        self.last_error = None;
        self.is_loading_characters = true;
        self.characters.clear();
        self.selected_character_id = None;
        self.deleting_character = false;
        self.characters_rx = None;
        self.delete_rx = None;
        self.pending_delete_character_id = None;
        self.pending_delete_character_name = None;
        self.delete_dialog.hide();
        self.form
            .set_status(Some("Loading characters...".to_string()));
        self.form.set_error(None);
        self.form.set_username(app_state.api.username.clone());

        let Some(token) = app_state.api.token.as_deref() else {
            self.is_loading_characters = false;
            self.last_error = Some("Missing account session token".to_string());
            self.form.set_error(self.last_error.clone());
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

    fn on_exit(&mut self, _app_state: &mut AppState<'_>) {
        self.characters_rx = None;
        self.login_rx = None;
        self.delete_rx = None;
        self.is_loading_characters = false;
        self.logging_in = false;
        self.deleting_character = false;
        self.pending_race = None;
        self.pending_delete_character_id = None;
        self.pending_delete_character_name = None;
        self.delete_dialog.hide();

        Self::drop_or_join_thread(&mut self.characters_thread, "character loading");
        Self::drop_or_join_thread(&mut self.login_thread, "game login");
        Self::drop_or_join_thread(&mut self.delete_thread, "character delete");
    }

    fn handle_event(&mut self, app_state: &mut AppState<'_>, event: &Event) -> Option<SceneType> {
        if let Event::MouseMotion { x, y, .. } = event {
            self.mouse_x = *x;
            self.mouse_y = *y;
        }

        let modifiers =
            KeyModifiers::from_sdl2(Mod::from_bits_truncate(sdl2::keyboard::Mod::empty().bits()));

        if let Some(ui_event) = ui::sdl_to_ui_event(event, self.mouse_x, self.mouse_y, modifiers) {
            // Delete dialog is modal: blocks form input.
            if self.delete_dialog.is_visible() {
                self.delete_dialog.handle_event(&ui_event);

                for action in self.delete_dialog.take_actions() {
                    match action {
                        DeleteCharacterDialogAction::Confirm { character_id } => {
                            if Self::is_thread_running(&self.delete_thread) {
                                self.form.set_error(Some(
                                    "Character delete already in progress".to_string(),
                                ));
                                continue;
                            }

                            let Some(token) = app_state.api.token.as_deref() else {
                                self.form
                                    .set_error(Some("Missing account session token".to_string()));
                                continue;
                            };

                            let base_url = app_state.api.base_url.clone();
                            let token = token.to_string();
                            let (tx, rx) = mpsc::channel();
                            self.delete_thread = Some(std::thread::spawn(move || {
                                let result =
                                    account_api::delete_character(&base_url, &token, character_id);
                                if let Err(err) = tx.send(result) {
                                    log::error!("Failed to send delete result: {}", err);
                                }
                            }));

                            self.delete_rx = Some(rx);
                            self.deleting_character = true;
                            self.delete_dialog.set_deleting(true);
                            self.form.set_error(None);
                        }
                        DeleteCharacterDialogAction::Cancel => {
                            self.delete_dialog.hide();
                            self.pending_delete_character_id = None;
                            self.pending_delete_character_name = None;
                        }
                    }
                }

                return self.pending_scene.take();
            }

            // Forward to form.
            self.form.handle_event(&ui_event);

            // Track selection changes.
            self.selected_character_id = self.form.selected_character_id();

            // Process form actions.
            for action in self.form.take_actions() {
                match action {
                    CharacterSelectionFormAction::CreateNew => {
                        self.last_error = None;
                        self.pending_scene = Some(SceneType::CharacterCreation);
                    }
                    CharacterSelectionFormAction::ContinueToGame { character_id } => {
                        if self.logging_in || Self::is_thread_running(&self.login_thread) {
                            self.form
                                .set_error(Some("Login already in progress".to_string()));
                            continue;
                        }

                        let Some(token) = app_state.api.token.as_deref() else {
                            self.form
                                .set_error(Some("Missing account session token".to_string()));
                            continue;
                        };

                        let Some(selected) = self.characters.iter().find(|c| c.id == character_id)
                        else {
                            self.form
                                .set_error(Some("Selected character not found".to_string()));
                            continue;
                        };

                        let race_int = traits::get_race_integer(
                            selected.sex == traits::Sex::Male,
                            selected.class,
                        );

                        self.pending_race = Some(race_int);
                        self.form.set_error(None);

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
                    CharacterSelectionFormAction::DeleteCharacter {
                        character_id,
                        character_name,
                    } => {
                        self.pending_delete_character_id = Some(character_id);
                        self.pending_delete_character_name = Some(character_name.clone());
                        self.delete_dialog.show(character_id, &character_name);
                        self.form.set_error(None);
                    }
                    CharacterSelectionFormAction::LogOut => {
                        app_state.api.token = None;
                        app_state.api.username = None;
                        self.last_error = None;
                        self.pending_scene = Some(SceneType::Login);
                    }
                }
            }
        }

        self.pending_scene.take()
    }

    fn update(&mut self, app_state: &mut AppState<'_>, dt: Duration) -> Option<SceneType> {
        app_state.panning_background.update(dt);
        self.form.update(dt);
        self.delete_dialog.update(dt);

        Self::cleanup_finished_thread(&mut self.characters_thread, "character loading");
        Self::cleanup_finished_thread(&mut self.login_thread, "game login");
        Self::cleanup_finished_thread(&mut self.delete_thread, "character delete");

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

                        let items: Vec<ListItem> = characters
                            .iter()
                            .map(|c| {
                                let sprite_id =
                                    Some(mag_core::traits::get_sprite_id_for_class_and_sex(
                                        c.class, c.sex,
                                    ));
                                ListItem {
                                    id: c.id,
                                    label: format!("{} ({})", c.name, c.class.to_string()),
                                    sprite_id,
                                }
                            })
                            .collect();

                        let names: Vec<(u64, String)> =
                            characters.iter().map(|c| (c.id, c.name.clone())).collect();

                        self.form.set_characters(items, names);
                        if let Some(id) = self.selected_character_id {
                            self.form.set_selected(Some(id));
                        }
                        self.form.set_status(None);

                        self.characters = characters;
                        self.last_error = None;
                    }
                    Err(error) => {
                        log::error!("Failed to load characters: {}", error);
                        self.characters.clear();
                        self.selected_character_id = None;
                        self.last_error = Some(error.clone());
                        self.form.set_error(Some(error));
                        self.form.set_status(None);
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
                        let Some(character_id) = self.selected_character_id else {
                            self.form
                                .set_error(Some("Select a character first".to_string()));
                            return None;
                        };

                        let Some(selected) = self.characters.iter().find(|c| c.id == character_id)
                        else {
                            self.form
                                .set_error(Some("Selected character not found".to_string()));
                            return None;
                        };

                        app_state.api.login_target = Some(GameLoginTarget {
                            ticket,
                            race: self.pending_race.unwrap_or(0),
                            character_id,
                            character_name: selected.name.clone(),
                        });
                        self.pending_race = None;
                        return Some(SceneType::Game);
                    }
                    Err(error) => {
                        log::error!("Failed to create game login ticket: {}", error);
                        self.last_error = Some(error.clone());
                        self.form.set_error(Some(error));
                    }
                }
            }
        }

        if self.deleting_character {
            let result = if let Some(receiver) = &self.delete_rx {
                match receiver.try_recv() {
                    Ok(result) => Some(result),
                    Err(TryRecvError::Empty) => None,
                    Err(TryRecvError::Disconnected) => {
                        Some(Err("Character delete task failed unexpectedly".to_string()))
                    }
                }
            } else {
                None
            };

            if let Some(result) = result {
                self.deleting_character = false;
                self.delete_rx = None;

                match result {
                    Ok(()) => {
                        let deleted_character_id = self.pending_delete_character_id;
                        let deleted_character_name = self
                            .pending_delete_character_name
                            .clone()
                            .unwrap_or_else(|| "<unknown>".to_string());

                        if let Some(character_id) = deleted_character_id {
                            self.characters.retain(|c| c.id != character_id);
                            self.form.remove_character(character_id);
                        }

                        self.selected_character_id = self.characters.first().map(|c| c.id);
                        self.form.set_selected(self.selected_character_id);
                        self.last_error = None;
                        self.delete_dialog.hide();
                        self.pending_delete_character_id = None;
                        self.pending_delete_character_name = None;
                        log::info!("Deleted character {}", deleted_character_name);
                    }
                    Err(error) => {
                        log::error!("Failed to delete character: {}", error);
                        self.last_error = Some(error.clone());
                        self.form.set_error(Some(error));
                        self.delete_dialog.hide();
                    }
                }
            }
        }

        None
    }

    fn render_world(
        &mut self,
        app_state: &mut AppState<'_>,
        canvas: &mut Canvas<Window>,
    ) -> Result<(), String> {
        let AppState {
            ref mut panning_background,
            ref mut gfx_cache,
            ..
        } = app_state;

        let mut render_ctx = RenderContext {
            canvas,
            gfx: gfx_cache,
        };

        panning_background.render(&mut render_ctx)?;

        // Render selected character portrait.
        if let Some(selected_character_id) = self.selected_character_id {
            if let Some(selected) = self
                .characters
                .iter()
                .find(|c| c.id == selected_character_id)
            {
                let sprite_id =
                    mag_core::traits::get_sprite_id_for_class_and_sex(selected.class, selected.sex);
                let texture = render_ctx.gfx.get_texture(sprite_id);
                let target_rect = Rect::new(400, 160, 160, 160);

                if let Err(error) = render_ctx.canvas.copy(texture, None, target_rect) {
                    log::error!(
                        "Failed to render selected portrait for class {:?}, sex {:?} (sprite ID {}): {}",
                        selected.class,
                        selected.sex,
                        sprite_id,
                        error
                    );
                }
            }
        }

        self.form.render(&mut render_ctx)?;
        self.delete_dialog.render(&mut render_ctx)?;

        Ok(())
    }
}
