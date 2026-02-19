use std::{
    sync::mpsc::{self, Receiver, TryRecvError},
    time::Duration,
};

use egui_sdl2::egui::{self, Align2, Vec2};
use sdl2::{event::Event, pixels::Color, render::Canvas, video::Window};

use crate::{
    account_api,
    scenes::scene::{Scene, SceneType},
    state::AppState,
};

pub struct NewAccountScene {
    email: String,
    username: String,
    password: String,
    is_submitting: bool,
    api_result_rx: Option<Receiver<Result<(), String>>>,
    error_message: Option<String>,
    account_thread: Option<std::thread::JoinHandle<()>>,
}

impl NewAccountScene {
    pub fn new() -> Self {
        NewAccountScene {
            email: String::new(),
            username: String::new(),
            password: String::new(),
            is_submitting: false,
            api_result_rx: None,
            error_message: None,
            account_thread: None,
        }
    }

    fn create_account(
        base_url: &str,
        email: &str,
        username: &str,
        password: &str,
    ) -> Result<(), String> {
        let email = email.trim();
        let username = username.trim();
        let password = password.trim();

        if email.is_empty() {
            return Err("Email is required".to_string());
        }

        if username.is_empty() {
            return Err("Username is required".to_string());
        }

        if password.is_empty() {
            return Err("Password is required".to_string());
        }

        account_api::create_account(base_url, email, username, password).map(|_| ())
    }
}

impl Scene for NewAccountScene {
    fn handle_event(&mut self, _app_state: &mut AppState, _event: &Event) -> Option<SceneType> {
        None
    }

    fn update(&mut self, _app_state: &mut AppState, _dt: Duration) -> Option<SceneType> {
        if self.is_submitting {
            let result = if let Some(receiver) = &self.api_result_rx {
                match receiver.try_recv() {
                    Ok(result) => Some(result),
                    Err(TryRecvError::Empty) => None,
                    Err(TryRecvError::Disconnected) => {
                        Some(Err("Account creation task failed unexpectedly".to_string()))
                    }
                }
            } else {
                None
            };

            if let Some(result) = result {
                self.is_submitting = false;
                self.api_result_rx = None;

                match result {
                    Ok(()) => return Some(SceneType::Login),
                    Err(error) => self.error_message = Some(error),
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
                let (create_clicked, cancel_clicked) = ui
                    .add_enabled_ui(!self.is_submitting, |ui| {
                        ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                            ui.heading("Create Account");
                        });
                        ui.add_space(10.0);

                        ui.label("E-mail");
                        ui.add(egui::TextEdit::singleline(&mut self.email).desired_width(260.0));
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
                            let create_clicked = ui
                                .add(egui::Button::new("Create").min_size([180.0, 32.0].into()))
                                .clicked();

                            let cancel_clicked = ui
                                .add(egui::Button::new("Cancel").min_size([180.0, 32.0].into()))
                                .clicked();

                            (create_clicked, cancel_clicked)
                        })
                        .inner
                    })
                    .inner;

                if self.is_submitting {
                    ui.add_space(8.0);
                    ui.label("Creating account...");

                    // Clear error message if the user re-submits while an error is displayed
                    if self.error_message.is_some() {
                        self.error_message = None;
                    }
                }

                if let Some(error_message) = &self.error_message {
                    ui.add_space(8.0);
                    ui.colored_label(egui::Color32::RED, error_message);
                }

                if cancel_clicked {
                    log::info!("Cancel clicked");
                    next = Some(SceneType::Login);
                }

                if create_clicked {
                    let (sender, receiver) = mpsc::channel::<Result<(), String>>();

                    self.error_message = None;
                    self.is_submitting = true;
                    self.api_result_rx = Some(receiver);

                    let base_url = app_state.api.base_url.clone();
                    let email = self.email.clone();
                    let username = self.username.clone();
                    let password = self.password.clone();

                    self.account_thread = Some(std::thread::spawn(move || {
                        let result = Self::create_account(&base_url, &email, &username, &password);
                        if let Err(error) = sender.send(result) {
                            log::error!("Failed to send account creation result: {}", error);
                        }
                    }));

                    log::info!(
                        "Create new account clicked with email={}, username={}",
                        self.email,
                        self.username
                    );
                }
            });

        next
    }
}
