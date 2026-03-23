use std::{
    sync::mpsc::{self, TryRecvError},
    time::Duration,
};

use crate::{
    account_api, cert_trust, preferences,
    scenes::scene::{Scene, SceneType},
    sfx_cache::MusicTrack,
    state::AppState,
    ui::{
        self,
        cert_dialog::{CertDialog, CertDialogAction},
        login_form::{LoginForm, LoginFormAction},
        widget::{KeyModifiers, Widget},
        RenderContext,
    },
};
use sdl2::{event::Event, keyboard::Mod, render::Canvas, video::Window};

/// Scene that presents the account login form.
///
/// Displays IP, username and password fields plus an optional music toggle.
/// Login is performed on a background thread; the result is polled in `update`.
/// On success the scene transitions to `CharacterSelection`.
pub struct LoginScene {
    /// Login form panel with text inputs and buttons.
    login_form: LoginForm,
    /// Certificate-mismatch dialog (shown when server cert changes).
    cert_dialog: Option<CertDialog>,
    /// Queued scene transition from widget actions.
    pending_scene: Option<SceneType>,

    // -- Async login state (unchanged from the egui version) --
    is_submitting: bool,
    api_result_rx: Option<std::sync::mpsc::Receiver<Result<String, String>>>,
    login_thread: Option<std::thread::JoinHandle<()>>,
    music_initialized: bool,

    // -- Mouse position for SDL-->UiEvent conversion --
    mouse_x: i32,
    mouse_y: i32,
}

impl LoginScene {
    /// Creates a new `LoginScene` with default field values and the configured server IP.
    ///
    /// The username field is pre-populated from the last successful login if one
    /// was previously saved.
    pub fn new() -> Self {
        let settings = preferences::load_global_settings();
        let login_form = LoginForm::new(
            &crate::hosts::get_server_ip(),
            &preferences::load_last_username().unwrap_or_default(),
            settings.music_enabled,
        );

        Self {
            login_form,
            cert_dialog: None,
            pending_scene: None,
            is_submitting: false,
            api_result_rx: None,
            login_thread: None,
            music_initialized: false,
            mouse_x: 0,
            mouse_y: 0,
        }
    }

    /// Lazily starts the login-screen music track if it hasn't been started yet.
    fn ensure_music_initialized(&mut self, app_state: &mut AppState<'_>) {
        if self.music_initialized {
            return;
        }

        let settings = preferences::load_global_settings();
        app_state.settings.music_enabled = settings.music_enabled;

        if app_state.settings.music_enabled {
            app_state.sfx_cache.play_music(MusicTrack::LoginTheme);
        } else {
            app_state.sfx_cache.stop_music();
        }

        self.music_initialized = true;
    }

    /// Persists the music-enabled preference to disk.
    fn save_music_setting(&self, enabled: bool) {
        let mut settings = preferences::load_global_settings();
        settings.music_enabled = enabled;

        if let Err(err) = preferences::save_global_settings(&settings) {
            log::warn!("Failed to save login music setting: {}", err);
        }
    }

    /// Starts an asynchronous login request using the current form values.
    ///
    /// # Arguments
    ///
    /// * `app_state` - Shared application state used to persist API session fields.
    /// * `api_base_url` - Resolved API base URL for the request.
    /// * `username` - Account username.
    /// * `password` - Account password (plain-text; hashed by `account_api::login`).
    fn begin_login_request(
        &mut self,
        app_state: &mut AppState<'_>,
        api_base_url: String,
        username: String,
        password: String,
    ) {
        let (sender, receiver) = mpsc::channel::<Result<String, String>>();

        self.login_form.set_error(None);
        self.is_submitting = true;
        self.login_form.set_submitting(true);
        self.api_result_rx = Some(receiver);

        let base_url = api_base_url;

        app_state.api.base_url = base_url.clone();
        app_state.api.username = Some(username.clone());

        self.login_thread = Some(std::thread::spawn(move || {
            let result = account_api::login(&base_url, &username, &password);
            if let Err(error) = sender.send(result) {
                log::error!("Failed to send login result: {}", error);
            }
        }));
    }

    /// Processes a [`LoginFormAction::Login`] — validates input, resolves the
    /// API base URL and fires the async request.
    ///
    /// # Arguments
    ///
    /// * `app_state` - Shared application state.
    /// * `ip` - Server IP / hostname from the form.
    /// * `username` - Username from the form.
    /// * `password` - Password from the form.
    fn handle_login_action(
        &mut self,
        app_state: &mut AppState<'_>,
        ip: String,
        username: String,
        password: String,
    ) {
        log::info!("Login clicked: ip={}, username={}", ip, username);

        let entered_host = ip.trim();
        if entered_host.is_empty() {
            self.login_form
                .set_error(Some("Please enter an IP address or hostname".to_string()));
            return;
        }

        let api_base_url =
            if entered_host.starts_with("http://") || entered_host.starts_with("https://") {
                entered_host.trim_end_matches('/').to_string()
            } else {
                format!("https://{}:5554", entered_host)
            };

        self.login_form
            .set_unencrypted_warning(api_base_url.to_ascii_lowercase().starts_with("http://"));

        self.begin_login_request(app_state, api_base_url, username, password);
    }
}

impl Scene for LoginScene {
    fn on_enter(&mut self, app_state: &mut AppState<'_>) {
        self.ensure_music_initialized(app_state);
    }

    fn on_exit(&mut self, app_state: &mut AppState<'_>) {
        app_state.sfx_cache.stop_music();
    }

    fn handle_event(&mut self, app_state: &mut AppState<'_>, event: &Event) -> Option<SceneType> {
        // Track mouse position for the SDL-->UiEvent conversion.
        if let Event::MouseMotion { x, y, .. } = event {
            self.mouse_x = *x;
            self.mouse_y = *y;
        }

        let modifiers =
            KeyModifiers::from_sdl2(Mod::from_bits_truncate(sdl2::keyboard::Mod::empty().bits()));

        // Build UiEvent from the raw SDL event.
        if let Some(ui_event) = ui::sdl_to_ui_event(event, self.mouse_x, self.mouse_y, modifiers) {
            // Certificate dialog blocks all input to the form behind it.
            if self.cert_dialog.is_some() {
                let dialog = self.cert_dialog.as_mut().unwrap();
                dialog.handle_event(&ui_event);
                let actions = dialog.take_cert_actions();

                // Collect data we need before dropping the borrow.
                let mut accept_data: Option<(String, String)> = None;
                let mut reject = false;

                for action in actions {
                    match action {
                        CertDialogAction::Accept => {
                            let d = self.cert_dialog.as_ref().unwrap();
                            accept_data = Some((d.host.clone(), d.received_fp.clone()));
                        }
                        CertDialogAction::Reject => {
                            reject = true;
                        }
                    }
                }

                if let Some((host, fp)) = accept_data {
                    self.cert_dialog = None;
                    match cert_trust::trust_fingerprint(&host, &fp) {
                        Ok(()) => {
                            let retry_url = app_state.api.base_url.clone();
                            if retry_url.trim().is_empty() {
                                self.login_form.set_error(Some(
                                    "Accepted new certificate. Click Login again.".to_string(),
                                ));
                            } else {
                                let username = self.login_form.username().to_owned();
                                let password = self.login_form.password().to_owned();
                                self.begin_login_request(app_state, retry_url, username, password);
                            }
                        }
                        Err(err) => {
                            self.login_form
                                .set_error(Some(format!("Failed to update known hosts: {err}")));
                        }
                    }
                } else if reject {
                    self.cert_dialog = None;
                }

                return self.pending_scene.take();
            }

            // ── Normal (no dialog) ──────────────────────────────────────
            self.login_form.handle_event(&ui_event);

            for action in self.login_form.take_login_actions() {
                match action {
                    LoginFormAction::Login {
                        ip,
                        username,
                        password,
                    } => {
                        self.handle_login_action(app_state, ip, username, password);
                    }
                    LoginFormAction::CreateAccount => {
                        log::info!("Create new account clicked");
                        return Some(SceneType::NewAccount);
                    }
                    LoginFormAction::ResetPassword => {
                        log::info!("Reset password clicked");
                        return Some(SceneType::RequestReset);
                    }
                    LoginFormAction::Quit => {
                        return Some(SceneType::Exit);
                    }
                    LoginFormAction::ToggleMusic(enabled) => {
                        app_state.settings.music_enabled = enabled;
                        if enabled {
                            app_state.sfx_cache.play_music(MusicTrack::LoginTheme);
                        } else {
                            app_state.sfx_cache.stop_music();
                        }
                        self.save_music_setting(enabled);
                    }
                }
            }
        }

        self.pending_scene.take()
    }

    fn update(&mut self, app_state: &mut AppState<'_>, dt: Duration) -> Option<SceneType> {
        self.ensure_music_initialized(app_state);

        // Animate background and form.
        app_state.panning_background.update(dt);
        self.login_form.update(dt);

        // Poll async login result.
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
                self.login_form.set_submitting(false);
                self.api_result_rx = None;

                match result {
                    Ok(token) => {
                        log::info!("Login successful!");
                        app_state.api.token = Some(token);
                        let username = self.login_form.username().to_owned();
                        if let Err(err) = preferences::save_last_username(&username) {
                            log::warn!("Failed to save last username: {}", err);
                        }
                        return Some(SceneType::CharacterSelection);
                    }
                    Err(error) => {
                        log::error!("Login failed: {}", error);
                        app_state.api.token = None;
                        if let Some(mismatch) = cert_trust::take_last_fingerprint_mismatch() {
                            self.cert_dialog = Some(CertDialog::new(
                                &mismatch.host,
                                &mismatch.expected_fingerprint,
                                &mismatch.received_fingerprint,
                            ));
                        }
                        self.login_form.set_error(Some(error));
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
        let mut ctx = RenderContext {
            canvas,
            gfx: gfx_cache,
        };

        panning_background.render(&mut ctx)?;
        self.login_form.render(&mut ctx)?;

        if let Some(ref mut dialog) = self.cert_dialog {
            dialog.render(&mut ctx)?;
        }

        Ok(())
    }
}
