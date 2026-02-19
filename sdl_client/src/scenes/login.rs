use std::{
    sync::mpsc::{self, TryRecvError},
    time::Duration,
};

use crate::{
    account_api,
    scenes::scene::{Scene, SceneType},
    state::AppState,
};
use egui_sdl2::egui::{self, Align2, Vec2};
use sdl2::{event::Event, pixels::Color, render::Canvas, video::Window};

pub struct LoginScene {
    server_ip: String,
    username: String,
    password: String,
    is_submitting: bool,
    api_result_rx: Option<std::sync::mpsc::Receiver<Result<String, String>>>,
    error_message: Option<String>,
    login_thread: Option<std::thread::JoinHandle<()>>,
}

impl LoginScene {
    pub fn new() -> Self {
        Self {
            server_ip: "127.0.0.1".to_owned(),
            username: String::new(),
            password: String::new(),
            is_submitting: false,
            api_result_rx: None,
            error_message: None,
            login_thread: None,
        }
    }

    fn login(username: &str, password: &str) -> Result<String, String> {
        let base_url = if cfg!(debug_assertions) {
            "http://127.0.0.1:5554"
        } else {
            "http://menamonggods.ddns.net:5554"
        };

        account_api::login(base_url, username, password)
    }
}

impl Scene for LoginScene {
    fn handle_event(&mut self, _app_state: &mut AppState, _event: &Event) -> Option<SceneType> {
        None
    }

    fn update(&mut self, app_state: &mut AppState, _dt: Duration) -> Option<SceneType> {
        if self.is_submitting {
            let result = if let Some(receiver) = &self.api_result_rx {
                match receiver.try_recv() {
                    Ok(result) => Some(result),
                    Err(TryRecvError::Empty) => None,
                    Err(TryRecvError::Disconnected) => {
                        app_state.api.token = None;
                        Some(Err("Login task failed unexpectedly".to_string()))
                    }
                }
            } else {
                None
            };

            if let Some(result) = result {
                self.is_submitting = false;
                self.api_result_rx = None;

                match result {
                    Ok(token) => {
                        log::info!("Login successful!");
                        app_state.api.token = Some(token);
                        return Some(SceneType::CharacterSelection);
                    }
                    Err(error) => {
                        log::error!("Login failed: {}", error);
                        app_state.api.token = None;
                        self.error_message = Some(error);
                    }
                }
            }
        }
        None
    }

    fn render_world(
        &mut self,
        _app_state: &mut AppState,
        canvas: &mut Canvas<Window>,
    ) -> Result<(), String> {
        canvas.set_draw_color(Color::RGB(20, 20, 28));
        canvas.clear();
        Ok(())
    }

    fn render_ui(&mut self, app_state: &mut AppState, ctx: &egui::Context) -> Option<SceneType> {
        let mut next = None;

        egui::Window::new("Men Among Gods - Reforged")
            .default_height(430.0)
            .default_width(430.0)
            .anchor(Align2::CENTER_CENTER, Vec2::new(0.0, 0.0))
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                let (login_clicked, create_clicked) = ui
                    .add_enabled_ui(!self.is_submitting, |ui| {
                        ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                            ui.heading("Account Login");
                        });
                        ui.add_space(10.0);

                        ui.label("IP Address (IPv4)");
                        ui.add(
                            egui::TextEdit::singleline(&mut self.server_ip).desired_width(260.0),
                        );
                        ui.add_space(10.0);

                        ui.label("Username");
                        ui.add(egui::TextEdit::singleline(&mut self.username).desired_width(260.0));
                        ui.add_space(8.0);

                        ui.label("Password");
                        ui.add(
                            egui::TextEdit::singleline(&mut self.password)
                                .password(true)
                                .desired_width(260.0),
                        );
                        ui.add_space(12.0);

                        ui.horizontal(|ui| {
                            let login_clicked = ui
                                .add(egui::Button::new("Login").min_size([180.0, 32.0].into()))
                                .clicked();

                            let create_clicked = ui
                                .add(
                                    egui::Button::new("Create new account")
                                        .min_size([180.0, 32.0].into()),
                                )
                                .clicked();

                            (login_clicked, create_clicked)
                        })
                        .inner
                    })
                    .inner;

                if self.is_submitting {
                    ui.add_space(8.0);
                    ui.label("Logging in...");

                    if self.error_message.is_some() {
                        self.error_message = None;
                    }
                }

                if let Some(error_message) = &self.error_message {
                    ui.add_space(8.0);
                    ui.colored_label(egui::Color32::RED, error_message);
                }

                if login_clicked {
                    log::info!(
                        "Login clicked: ip={}, username={}",
                        self.server_ip,
                        self.username
                    );

                    let (sender, receiver) = mpsc::channel::<Result<String, String>>();

                    self.error_message = None;
                    self.is_submitting = true;
                    self.api_result_rx = Some(receiver);

                    let username = self.username.clone();
                    let password = self.password.clone();

                    app_state.api.username = Some(username.clone());

                    self.login_thread = Some(std::thread::spawn(move || {
                        let result = Self::login(&username, &password);
                        if let Err(error) = sender.send(result) {
                            log::error!("Failed to send login result: {}", error);
                        }
                    }));
                }

                if create_clicked {
                    log::info!("Create new account clicked");
                    next = Some(SceneType::NewAccount);
                }
            });

        next
    }
}
