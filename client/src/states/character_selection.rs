use std::sync::{mpsc, Arc, Mutex};

use bevy::prelude::*;
use bevy::tasks::{IoTaskPool, Task};
use bevy_egui::{
    egui::{self, Pos2},
    EguiContexts,
};
use mag_core::traits;

use crate::constants::{TARGET_HEIGHT, TARGET_WIDTH};
use crate::gfx_cache::GraphicsCache;
use crate::network::account_api::{self, ApiSession, CharacterSummary};
use crate::network::{LoginRequested, LoginStatus, NetworkRuntime};
use crate::settings::UserSettingsState;
use crate::settings::DEFAULT_SERVER_PORT;
use crate::states::helpers::texture_id_for_character;
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
    delete_dialog_open: bool,
    delete_confirm_input: String,
    delete_target_id: Option<u64>,
    delete_target_name: Option<String>,
    delete_error: Option<String>,
    delete_is_busy: bool,
    delete_task: Option<Task<()>>,
    delete_rx: Option<Arc<Mutex<mpsc::Receiver<Result<(), String>>>>>,

    login_is_busy: bool,
    login_task: Option<Task<()>>,
    login_rx: Option<Arc<Mutex<mpsc::Receiver<Result<u64, String>>>>>,
    login_target: Option<(u64, i32)>,
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
    net: Res<NetworkRuntime>,
    status: Res<LoginStatus>,
    user_settings: Res<UserSettingsState>,
    gfx: Res<GraphicsCache>,
    mut login_ev: MessageWriter<LoginRequested>,
) {
    if let Some(notice) = api_session.pending_notice.take() {
        ui_state.success_notice = Some(notice);
        ui_state.last_error = None;
        ui_state.has_loaded = false;
        ui_state.characters.clear();
        ui_state.selected_character_id = None;
    }

    let delete_rx_arc = ui_state.delete_rx.as_ref().map(Arc::clone);
    if let Some(delete_rx_arc) = delete_rx_arc {
        enum DeleteStatus {
            Empty,
            Disconnected,
            Locked,
            Ready(Result<(), String>),
        }

        let status = match delete_rx_arc.lock() {
            Ok(rx) => match rx.try_recv() {
                Ok(result) => DeleteStatus::Ready(result),
                Err(mpsc::TryRecvError::Disconnected) => DeleteStatus::Disconnected,
                Err(mpsc::TryRecvError::Empty) => DeleteStatus::Empty,
            },
            Err(_) => DeleteStatus::Locked,
        };

        match status {
            DeleteStatus::Ready(result) => {
                ui_state.delete_rx = None;
                ui_state.delete_task = None;
                ui_state.delete_is_busy = false;

                match result {
                    Ok(()) => {
                        ui_state.delete_dialog_open = false;
                        ui_state.delete_confirm_input.clear();
                        ui_state.delete_target_id = None;
                        ui_state.delete_target_name = None;
                        ui_state.delete_error = None;
                        ui_state.success_notice = Some("Character deleted".to_string());
                        ui_state.has_loaded = false;
                        ui_state.characters.clear();
                        ui_state.selected_character_id = None;
                    }
                    Err(err) => {
                        ui_state.delete_error = Some(err);
                    }
                }
            }
            DeleteStatus::Disconnected => {
                ui_state.delete_rx = None;
                ui_state.delete_task = None;
                ui_state.delete_is_busy = false;
                ui_state.delete_error = Some("Delete failed: channel closed".to_string());
            }
            DeleteStatus::Locked => {
                ui_state.delete_rx = None;
                ui_state.delete_task = None;
                ui_state.delete_is_busy = false;
                ui_state.delete_error = Some("Delete failed: channel locked".to_string());
            }
            DeleteStatus::Empty => {}
        }
    }

    let login_rx_arc = ui_state.login_rx.as_ref().map(Arc::clone);
    if let Some(login_rx_arc) = login_rx_arc {
        enum LoginTicketStatus {
            Empty,
            Disconnected,
            Locked,
            Ready(Result<u64, String>),
        }

        let status = match login_rx_arc.lock() {
            Ok(rx) => match rx.try_recv() {
                Ok(result) => LoginTicketStatus::Ready(result),
                Err(mpsc::TryRecvError::Disconnected) => LoginTicketStatus::Disconnected,
                Err(mpsc::TryRecvError::Empty) => LoginTicketStatus::Empty,
            },
            Err(_) => LoginTicketStatus::Locked,
        };

        match status {
            LoginTicketStatus::Ready(result) => {
                ui_state.login_rx = None;
                ui_state.login_task = None;
                // Keep the UI in a "busy" state while the TCP login handshake runs.
                // We'll clear it if the network task reports an error.
                ui_state.login_is_busy = true;

                match result {
                    Ok(ticket) => {
                        let Some((_character_id, race_int)) = ui_state.login_target.take() else {
                            ui_state.last_error = Some("Missing login target".to_string());
                            return;
                        };

                        login_ev.write(LoginRequested {
                            host: user_settings.settings.default_server_ip.trim().to_string(),
                            port: DEFAULT_SERVER_PORT,
                            username: String::new(),
                            password: String::new(),
                            race: race_int,

                            user_id: 0,
                            pass1: 0,
                            pass2: 0,

                            login_ticket: Some(ticket),
                        });
                    }
                    Err(err) => {
                        ui_state.login_is_busy = false;
                        ui_state.last_error = Some(err);
                    }
                }
            }
            LoginTicketStatus::Disconnected => {
                ui_state.login_rx = None;
                ui_state.login_task = None;
                ui_state.login_is_busy = false;
                ui_state.last_error = Some("Ticket request failed: channel closed".to_string());
            }
            LoginTicketStatus::Locked => {
                ui_state.login_rx = None;
                ui_state.login_task = None;
                ui_state.login_is_busy = false;
                ui_state.last_error = Some("Ticket request failed: channel locked".to_string());
            }
            LoginTicketStatus::Empty => {}
        }
    }

    // If we attempted to log in but the network task has stopped with an error, surface it here.
    if ui_state.login_is_busy && !net.is_started() {
        if let Some(msg) = status.message.strip_prefix("Error: ") {
            ui_state.login_is_busy = false;
            ui_state.last_error = Some(msg.to_string());
        }
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

    let selected_character_name = ui_state
        .selected_character_id
        .and_then(|id| ui_state.characters.iter().find(|c| c.id == id))
        .map(|c| c.name.clone());

    // Precompute egui texture IDs for each character before borrowing the egui context.
    // (Borrowing the egui context prevents us from mutably borrowing `contexts` to register images.)
    let character_textures: Vec<Option<egui::TextureId>> = ui_state
        .characters
        .iter()
        .map(|character| {
            texture_id_for_character(&mut contexts, &gfx, character.class, character.sex)
        })
        .collect();

    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };

    egui::Window::new("Character Selection")
        .default_height(TARGET_HEIGHT)
        .default_width(TARGET_WIDTH)
        .fixed_pos(Pos2::new(0.0, 0.0))
        .collapsible(false)
        .resizable(false)
        .show(ctx, |ui| {
            ui.heading("Character selection");

            if let Some(username) = api_session.username.as_deref() {
                ui.label(format!("Logged in as: {username}"));
            } else {
                ui.label("No account session available");
            }

            if let Some(err) = ui_state.last_error.as_deref() {
                ui.colored_label(egui::Color32::LIGHT_RED, err);
            }

            if ui_state.login_is_busy {
                ui.colored_label(egui::Color32::LIGHT_BLUE, status.message.as_str());
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
                        for (index, character) in ui_state.characters.iter().enumerate() {
                            let texture_id = character_textures.get(index).copied().flatten();
                            let label = format!(
                                "{} ({}, {})",
                                character.name,
                                character.class.to_string(),
                                character.sex.to_string(),
                            );
                            let selected = next_selection == Some(character.id);

                            ui.horizontal(|ui| {
                                if let Some(texture_id) = texture_id {
                                    let size = egui::vec2(48.0, 48.0);
                                    let textured = egui::load::SizedTexture::new(texture_id, size);
                                    let img_resp = ui.add(egui::Image::new(textured));
                                    if img_resp.clicked() {
                                        next_selection = Some(character.id);
                                    }
                                }

                                let resp = ui.selectable_label(selected, label);
                                if resp.clicked() {
                                    next_selection = Some(character.id);
                                }
                            });
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

            let delete_enabled =
                ui_state.selected_character_id.is_some() && !ui_state.delete_is_busy;
            if ui
                .add_enabled(
                    delete_enabled,
                    egui::Button::new("Delete selected character").min_size([200.0, 32.0].into()),
                )
                .clicked()
            {
                ui_state.delete_dialog_open = true;
                ui_state.delete_confirm_input.clear();
                ui_state.delete_error = None;
                ui_state.delete_target_id = ui_state.selected_character_id;
                ui_state.delete_target_name = selected_character_name.clone();
            }

            if ui
                .add(egui::Button::new("Continue to game login").min_size([200.0, 32.0].into()))
                .clicked()
            {
                if ui_state.login_is_busy {
                    ui_state.last_error = Some("Login already in progress".to_string());
                } else {
                    let Some(token) = api_session.token.as_deref() else {
                        ui_state.last_error = Some("Missing account session token".to_string());
                        return;
                    };

                    let Some(character_id) = ui_state.selected_character_id else {
                        ui_state.last_error = Some("Select a character first".to_string());
                        return;
                    };

                    let Some(selected) = ui_state.characters.iter().find(|c| c.id == character_id)
                    else {
                        ui_state.last_error = Some("Selected character not found".to_string());
                        return;
                    };

                    let race_int =
                        traits::get_race_integer(selected.sex == traits::Sex::Male, selected.class);
                    ui_state.login_target = Some((character_id, race_int));

                    ui_state.login_is_busy = true;
                    ui_state.last_error = None;

                    let base_url = api_session.base_url.clone();
                    let token = token.to_string();
                    let (tx, rx) = mpsc::channel();
                    let rx = Arc::new(Mutex::new(rx));
                    let task = IoTaskPool::get().spawn(async move {
                        let result =
                            account_api::create_game_login_ticket(&base_url, &token, character_id);
                        let _ = tx.send(result);
                    });

                    ui_state.login_task = Some(task);
                    ui_state.login_rx = Some(rx);
                }
            }

            if ui
                .add(egui::Button::new("Log out").min_size([200.0, 32.0].into()))
                .clicked()
            {
                api_session.token = None;
                api_session.username = None;
                ui_state.last_error = None;
                next_state.set(GameState::AccountLogin);
            }
        });

    if ui_state.delete_dialog_open {
        let target_name = ui_state
            .delete_target_name
            .clone()
            .unwrap_or_else(|| "<unknown>".to_string());
        let confirm_input = ui_state.delete_confirm_input.clone();
        let confirm_matches = confirm_input.trim() == target_name;

        egui::Window::new("Confirm Delete")
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.label("Type the full character name to confirm deletion:");
                ui.label(format!("Character: {target_name}"));

                if let Some(err) = ui_state.delete_error.as_deref() {
                    ui.colored_label(egui::Color32::LIGHT_RED, err);
                }

                ui.add_space(8.0);
                ui.add_enabled(
                    !ui_state.delete_is_busy,
                    egui::TextEdit::singleline(&mut ui_state.delete_confirm_input)
                        .desired_width(260.0),
                );

                ui.add_space(12.0);

                let confirm_clicked = ui
                    .add_enabled(
                        !ui_state.delete_is_busy && confirm_matches,
                        egui::Button::new("Delete").min_size([120.0, 32.0].into()),
                    )
                    .clicked();

                let cancel_clicked = ui
                    .add_enabled(
                        !ui_state.delete_is_busy,
                        egui::Button::new("Cancel").min_size([120.0, 32.0].into()),
                    )
                    .clicked();

                if confirm_clicked {
                    let Some(token) = api_session.token.as_deref() else {
                        ui_state.delete_error = Some("Missing account session token".to_string());
                        return;
                    };

                    let Some(character_id) = ui_state.delete_target_id else {
                        ui_state.delete_error = Some("Missing character selection".to_string());
                        return;
                    };

                    ui_state.delete_is_busy = true;
                    ui_state.delete_error = None;

                    let base_url = api_session.base_url.clone();
                    let token = token.to_string();
                    let (tx, rx) = mpsc::channel();
                    let rx = Arc::new(Mutex::new(rx));
                    let task = IoTaskPool::get().spawn(async move {
                        let result = account_api::delete_character(&base_url, &token, character_id);
                        let _ = tx.send(result);
                    });

                    ui_state.delete_task = Some(task);
                    ui_state.delete_rx = Some(rx);
                }

                if cancel_clicked {
                    ui_state.delete_dialog_open = false;
                    ui_state.delete_confirm_input.clear();
                    ui_state.delete_target_id = None;
                    ui_state.delete_target_name = None;
                    ui_state.delete_error = None;
                }
            });
    }
}
