use std::sync::{mpsc, Arc, Mutex};

use bevy::prelude::*;
use bevy::tasks::{IoTaskPool, Task};
use bevy_egui::{
    egui::{self, Pos2},
    EguiContexts,
};

use crate::constants::{TARGET_HEIGHT, TARGET_WIDTH};
use crate::network::account_api::{
    self, ApiSession, CharacterRace, CharacterSex, CharacterSummary,
};
use crate::GameState;

#[derive(Resource, Debug, Default)]
pub struct CharacterSelectionUiState {
    characters: Vec<CharacterSummary>,
    selected_character_id: Option<u64>,
    is_loading: bool,
    has_loaded: bool,
    last_error: Option<String>,
    success_notice: Option<String>,
    pending_task: Option<Task<()>>,
    pending_rx: Option<Arc<Mutex<mpsc::Receiver<Result<Vec<CharacterSummary>, String>>>>>,
}

pub fn setup_character_selection(mut commands: Commands) {
    commands.insert_resource(CharacterSelectionUiState::default());
}

pub fn teardown_character_selection() {
    log::debug!("teardown_character_selection - end");
}

pub fn run_character_selection(
    mut contexts: EguiContexts,
    mut ui_state: ResMut<CharacterSelectionUiState>,
    mut api_session: ResMut<ApiSession>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };

    if let Some(notice) = api_session.pending_notice.take() {
        ui_state.success_notice = Some(notice);
        ui_state.last_error = None;
        ui_state.has_loaded = false;
        ui_state.characters.clear();
        ui_state.selected_character_id = None;
    }

    let rx_arc = ui_state.pending_rx.as_ref().map(Arc::clone);
    if let Some(rx_arc) = rx_arc {
        enum PendingStatus {
            Empty,
            Disconnected,
            Locked,
            Ready(Result<Vec<CharacterSummary>, String>),
        }

        let status = match rx_arc.lock() {
            Ok(rx) => match rx.try_recv() {
                Ok(result) => PendingStatus::Ready(result),
                Err(mpsc::TryRecvError::Disconnected) => PendingStatus::Disconnected,
                Err(mpsc::TryRecvError::Empty) => PendingStatus::Empty,
            },
            Err(_) => PendingStatus::Locked,
        };

        match status {
            PendingStatus::Ready(result) => {
                ui_state.pending_rx = None;
                ui_state.pending_task = None;
                ui_state.is_loading = false;
                ui_state.has_loaded = true;

                match result {
                    Ok(characters) => {
                        ui_state.characters = characters;
                        ui_state.last_error = None;
                    }
                    Err(err) => {
                        ui_state.last_error = Some(err);
                    }
                }
            }
            PendingStatus::Disconnected => {
                ui_state.pending_rx = None;
                ui_state.pending_task = None;
                ui_state.is_loading = false;
                ui_state.last_error = Some("Character list failed: channel closed".to_string());
            }
            PendingStatus::Locked => {
                ui_state.pending_rx = None;
                ui_state.pending_task = None;
                ui_state.is_loading = false;
                ui_state.last_error = Some("Character list failed: channel locked".to_string());
            }
            PendingStatus::Empty => {}
        }
    }

    if !ui_state.has_loaded && !ui_state.is_loading {
        let Some(token) = api_session.token.as_deref() else {
            ui_state.last_error = Some("Missing account session token".to_string());
            ui_state.has_loaded = true;
            return;
        };

        ui_state.is_loading = true;
        ui_state.last_error = None;

        let base_url = api_session.base_url.clone();
        let token = token.to_string();
        let (tx, rx) = mpsc::channel();
        let rx = Arc::new(Mutex::new(rx));
        let task = IoTaskPool::get().spawn(async move {
            let result = account_api::get_characters(&base_url, &token);
            let _ = tx.send(result);
        });

        ui_state.pending_task = Some(task);
        ui_state.pending_rx = Some(rx);
    }

    egui::Window::new("Character Selection")
        .default_height(TARGET_HEIGHT)
        .default_width(TARGET_WIDTH)
        .fixed_pos(Pos2::new(0.0, 0.0))
        .collapsible(false)
        .resizable(false)
        .show(ctx, |ui| {
            ui.heading("Character selection (placeholder)");

            if let Some(username) = api_session.username.as_deref() {
                ui.label(format!("Logged in as: {username}"));
            } else {
                ui.label("No account session available");
            }

            if let Some(err) = ui_state.last_error.as_deref() {
                ui.colored_label(egui::Color32::LIGHT_RED, err);
            }

            if let Some(notice) = ui_state.success_notice.as_deref() {
                ui.colored_label(egui::Color32::LIGHT_GREEN, notice);
            }

            ui.add_space(12.0);
            ui.label("Characters");

            if ui_state.is_loading {
                ui.label("Loading characters...");
            } else if ui_state.characters.is_empty() {
                ui.label("No characters found");
            } else {
                egui::ScrollArea::vertical()
                    .max_height(180.0)
                    .show(ui, |ui| {
                        let mut next_selection = ui_state.selected_character_id;
                        for character in &ui_state.characters {
                            let label = format!(
                                "{} ({}, {}) (id={})",
                                character.name,
                                format_race(character.race),
                                format_sex(character.sex),
                                character.id
                            );
                            let selected = next_selection == Some(character.id);
                            if ui.selectable_label(selected, label).clicked() {
                                next_selection = Some(character.id);
                            }
                        }
                        ui_state.selected_character_id = next_selection;
                    });
            }

            ui.add_space(12.0);

            if ui
                .add(egui::Button::new("Create new character").min_size([200.0, 32.0].into()))
                .clicked()
            {
                ui_state.last_error = None;
                next_state.set(GameState::CharacterCreation);
            }

            if ui
                .add(egui::Button::new("Continue to game login").min_size([200.0, 32.0].into()))
                .clicked()
            {
                next_state.set(GameState::LoggingIn);
            }

            if ui
                .add(egui::Button::new("Log out").min_size([120.0, 32.0].into()))
                .clicked()
            {
                api_session.token = None;
                api_session.username = None;
                ui_state.last_error = None;
                next_state.set(GameState::AccountLogin);
            }
        });
}

fn format_race(race: CharacterRace) -> &'static str {
    match race {
        CharacterRace::Mercenary => "Mercenary",
        CharacterRace::Templar => "Templar",
        CharacterRace::Harakim => "Harakim",
        CharacterRace::Sorcerer => "Sorcerer",
        CharacterRace::Warrior => "Warrior",
        CharacterRace::ArchTemplar => "Arch Templar",
        CharacterRace::ArchHarakim => "Arch Harakim",
        CharacterRace::SeyanDu => "Seyan Du",
    }
}

fn format_sex(sex: CharacterSex) -> &'static str {
    match sex {
        CharacterSex::Male => "Male",
        CharacterSex::Female => "Female",
    }
}
