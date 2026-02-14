use std::sync::{mpsc, Arc, Mutex};

use bevy::audio::{AudioPlayer, PlaybackSettings, Volume};
use bevy::prelude::*;
use bevy::tasks::{IoTaskPool, Task};
use bevy_egui::{
    egui::{self, Pos2},
    EguiContexts,
};

use crate::constants::{TARGET_HEIGHT, TARGET_WIDTH};
use crate::network::account_api;
use crate::network::account_api::ApiSession;
use crate::settings::{UserSettingsState, DEFAULT_SERVER_IP};
use crate::sfx_cache::SoundCache;
use crate::GameState;

#[derive(Component)]
pub(crate) struct AccountLoginMusic;

fn sync_login_music_entity(
    commands: &mut Commands,
    sfx: &SoundCache,
    enabled: bool,
    existing_music: Option<Entity>,
) {
    if enabled {
        if existing_music.is_none() {
            if let Some(handle) = sfx.login_music() {
                commands.spawn((
                    Name::new("AccountLoginMusic"),
                    AccountLoginMusic,
                    AudioPlayer::new(handle.clone()),
                    PlaybackSettings::LOOP.with_volume(Volume::Linear(0.5)),
                ));
            } else {
                log::warn!("Login music requested but login.mp3 was not found in SFX assets");
            }
        }
        return;
    }

    if let Some(entity) = existing_music {
        commands.entity(entity).despawn();
    }
}

#[derive(Resource, Debug)]
pub struct AccountLoginUiState {
    username: String,
    password: String,
    is_busy: bool,
    last_error: Option<String>,
    last_notice: Option<String>,
    pending_task: Option<Task<()>>,
    pending_rx: Option<Arc<Mutex<mpsc::Receiver<Result<String, String>>>>>,
}

impl Default for AccountLoginUiState {
    fn default() -> Self {
        Self {
            username: String::new(),
            password: String::new(),
            is_busy: false,
            last_error: None,
            last_notice: None,
            pending_task: None,
            pending_rx: None,
        }
    }
}

pub fn setup_account_login(
    mut commands: Commands,
    mut api_session: ResMut<ApiSession>,
    mut user_settings: ResMut<UserSettingsState>,
    sfx: Res<SoundCache>,
    music_q: Query<Entity, With<AccountLoginMusic>>,
) {
    api_session.ensure_defaults();

    if user_settings.settings.default_server_ip.trim().is_empty() {
        user_settings.settings.default_server_ip = DEFAULT_SERVER_IP.to_string();
        user_settings.request_save();
    }

    let mut ui = AccountLoginUiState::default();

    if let Some(notice) = api_session.pending_notice.take() {
        ui.last_notice = Some(notice);
    }

    sync_login_music_entity(
        &mut commands,
        &sfx,
        user_settings.settings.play_login_music,
        music_q.iter().next(),
    );

    commands.insert_resource(ui);
}

pub fn teardown_account_login() {
    log::debug!("teardown_account_login - end");
}

pub fn stop_account_login_music(
    mut commands: Commands,
    music_q: Query<Entity, With<AccountLoginMusic>>,
) {
    for entity in &music_q {
        commands.entity(entity).despawn();
    }
}

pub fn run_account_login(
    mut commands: Commands,
    mut contexts: EguiContexts,
    mut ui_state: ResMut<AccountLoginUiState>,
    mut api_session: ResMut<ApiSession>,
    mut user_settings: ResMut<UserSettingsState>,
    sfx: Res<SoundCache>,
    music_q: Query<Entity, With<AccountLoginMusic>>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };

    let rx_arc = ui_state.pending_rx.as_ref().map(Arc::clone);
    if let Some(rx_arc) = rx_arc {
        enum PendingStatus {
            Empty,
            Disconnected,
            Locked,
            Ready(Result<String, String>),
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
                ui_state.is_busy = false;

                match result {
                    Ok(token) => {
                        let username = ui_state.username.trim().to_string();
                        api_session.token = Some(token);
                        api_session.username = Some(username);
                        ui_state.password.clear();
                        ui_state.last_error = None;
                        next_state.set(GameState::CharacterSelection);
                    }
                    Err(err) => {
                        ui_state.last_error = Some(err);
                    }
                }
            }
            PendingStatus::Disconnected => {
                ui_state.pending_rx = None;
                ui_state.pending_task = None;
                ui_state.is_busy = false;
                ui_state.last_error = Some("Login request failed: channel closed".to_string());
            }
            PendingStatus::Locked => {
                ui_state.pending_rx = None;
                ui_state.pending_task = None;
                ui_state.is_busy = false;
                ui_state.last_error = Some("Login request failed: channel locked".to_string());
            }
            PendingStatus::Empty => {}
        }
    }

    let existing_music = music_q.iter().next();
    let mut music_pref_changed = false;

    egui::Window::new("Account Login")
        .default_height(TARGET_HEIGHT)
        .default_width(TARGET_WIDTH)
        .fixed_pos(Pos2::new(0.0, 0.0))
        .collapsible(false)
        .resizable(false)
        .show(ctx, |ui| {
            ui.heading("Men Among Gods Reforged");
            ui.add_space(10.0);

            ui.label("Game server IP address");
            let ip_resp = ui.add_enabled(
                !ui_state.is_busy,
                egui::TextEdit::singleline(&mut user_settings.settings.default_server_ip)
                    .desired_width(260.0),
            );
            if ip_resp.changed() {
                user_settings.request_save();
            }

            if let Some(msg) = ui_state.last_notice.as_deref() {
                ui.colored_label(egui::Color32::LIGHT_GREEN, msg);
            }
            if let Some(err) = ui_state.last_error.as_deref() {
                ui.colored_label(egui::Color32::LIGHT_RED, err);
            }

            ui.add_space(10.0);

            ui.label("Username");
            ui.add_enabled(
                !ui_state.is_busy,
                egui::TextEdit::singleline(&mut ui_state.username).desired_width(260.0),
            );

            ui.add_space(8.0);
            ui.label("Password");
            ui.add_enabled(
                !ui_state.is_busy,
                egui::TextEdit::singleline(&mut ui_state.password)
                    .password(true)
                    .desired_width(260.0),
            );

            ui.add_space(8.0);
            let play_login_music_resp = ui.add_enabled(
                !ui_state.is_busy,
                egui::Checkbox::new(
                    &mut user_settings.settings.play_login_music,
                    "Play login music",
                ),
            );
            if play_login_music_resp.changed() {
                user_settings.request_save();
                music_pref_changed = true;
            }

            ui.add_space(12.0);

            let (login_clicked, create_clicked) = ui
                .horizontal(|ui| {
                    let login_clicked = ui
                        .add_enabled(
                            !ui_state.is_busy,
                            egui::Button::new("Login").min_size([180.0, 32.0].into()),
                        )
                        .clicked();

                    let create_clicked = ui
                        .add_enabled(
                            !ui_state.is_busy,
                            egui::Button::new("Create new account").min_size([180.0, 32.0].into()),
                        )
                        .clicked();

                    (login_clicked, create_clicked)
                })
                .inner;

            if login_clicked {
                let username = ui_state.username.trim().to_string();
                let password = ui_state.password.clone();

                if username.is_empty() || password.is_empty() {
                    ui_state.last_error = Some("Username and password are required".to_string());
                } else {
                    ui_state.is_busy = true;
                    ui_state.last_error = None;
                    ui_state.last_notice = None;

                    let base_url = api_session.base_url.clone();
                    let (tx, rx) = mpsc::channel();
                    let rx = Arc::new(Mutex::new(rx));
                    let task = IoTaskPool::get().spawn(async move {
                        let result = account_api::login(&base_url, &username, &password);
                        let _ = tx.send(result);
                    });

                    ui_state.pending_task = Some(task);
                    ui_state.pending_rx = Some(rx);
                }
            }

            if create_clicked {
                ui_state.last_error = None;
                ui_state.last_notice = None;
                next_state.set(GameState::AccountCreation);
            }
        });

    if music_pref_changed {
        sync_login_music_entity(
            &mut commands,
            &sfx,
            user_settings.settings.play_login_music,
            existing_music,
        );
    }
}
