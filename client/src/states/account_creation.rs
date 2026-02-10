use std::sync::{mpsc, Arc, Mutex};

use bevy::prelude::*;
use bevy::tasks::{IoTaskPool, Task};
use bevy_egui::{
    egui::{self, Pos2},
    EguiContexts,
};

use crate::constants::{TARGET_HEIGHT, TARGET_WIDTH};
use crate::network::account_api;
use crate::network::account_api::ApiSession;
use crate::GameState;

#[derive(Resource, Debug)]
pub struct AccountCreationUiState {
    username: String,
    email: String,
    password: String,
    is_busy: bool,
    last_error: Option<String>,
    pending_task: Option<Task<()>>,
    pending_rx: Option<Arc<Mutex<mpsc::Receiver<Result<String, String>>>>>,
}

impl Default for AccountCreationUiState {
    fn default() -> Self {
        Self {
            username: String::new(),
            email: String::new(),
            password: String::new(),
            is_busy: false,
            last_error: None,
            pending_task: None,
            pending_rx: None,
        }
    }
}

pub fn setup_account_creation(mut commands: Commands, mut api_session: ResMut<ApiSession>) {
    api_session.ensure_defaults();
    commands.insert_resource(AccountCreationUiState::default());
}

pub fn teardown_account_creation() {
    log::debug!("teardown_account_creation - end");
}

pub fn run_account_creation(
    mut contexts: EguiContexts,
    mut ui_state: ResMut<AccountCreationUiState>,
    mut api_session: ResMut<ApiSession>,
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
                    Ok(message) => {
                        api_session.pending_notice = Some(message);
                        next_state.set(GameState::AccountLogin);
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
                ui_state.last_error = Some("Account creation failed: channel closed".to_string());
            }
            PendingStatus::Locked => {
                ui_state.pending_rx = None;
                ui_state.pending_task = None;
                ui_state.is_busy = false;
                ui_state.last_error = Some("Account creation failed: channel locked".to_string());
            }
            PendingStatus::Empty => {}
        }
    }

    egui::Window::new("Create Account")
        .default_height(TARGET_HEIGHT)
        .default_width(TARGET_WIDTH)
        .fixed_pos(Pos2::new(0.0, 0.0))
        .collapsible(false)
        .resizable(false)
        .show(ctx, |ui| {
            ui.heading("Create a new account");
            ui.label(format!("API: {}", api_session.base_url));

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
            ui.label("Email");
            ui.add_enabled(
                !ui_state.is_busy,
                egui::TextEdit::singleline(&mut ui_state.email).desired_width(260.0),
            );

            ui.add_space(8.0);
            ui.label("Password");
            ui.add_enabled(
                !ui_state.is_busy,
                egui::TextEdit::singleline(&mut ui_state.password)
                    .password(true)
                    .desired_width(260.0),
            );

            ui.add_space(12.0);

            let submit_clicked = ui
                .add_enabled(
                    !ui_state.is_busy,
                    egui::Button::new("Create account").min_size([160.0, 32.0].into()),
                )
                .clicked();

            let cancel_clicked = ui
                .add_enabled(
                    !ui_state.is_busy,
                    egui::Button::new("Back").min_size([120.0, 32.0].into()),
                )
                .clicked();

            if submit_clicked {
                let username = ui_state.username.trim().to_string();
                let email = ui_state.email.trim().to_string();
                let password = ui_state.password.clone();

                if username.is_empty() || email.is_empty() || password.is_empty() {
                    ui_state.last_error = Some("All fields are required".to_string());
                } else {
                    ui_state.is_busy = true;
                    ui_state.last_error = None;

                    let base_url = api_session.base_url.clone();
                    let (tx, rx) = mpsc::channel();
                    let rx = Arc::new(Mutex::new(rx));
                    let task = IoTaskPool::get().spawn(async move {
                        let result =
                            account_api::create_account(&base_url, &email, &username, &password);
                        let _ = tx.send(result);
                    });

                    ui_state.pending_task = Some(task);
                    ui_state.pending_rx = Some(rx);
                }
            }

            if cancel_clicked {
                ui_state.last_error = None;
                next_state.set(GameState::AccountLogin);
            }
        });
}
