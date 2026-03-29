use std::{
    sync::mpsc::{self, Receiver, TryRecvError},
    time::Duration,
};

use sdl2::{controller::Button as Btn, event::Event, keyboard::Mod, render::Canvas, video::Window};

use crate::{
    account_api, cert_trust,
    scenes::scene::{Scene, SceneType},
    state::AppState,
    ui::{
        self, RenderContext,
        controller_nav::ControllerNavState,
        forms::cert_dialog::{CertDialog, CertDialogAction},
        forms::request_reset_form::{RequestResetForm, RequestResetFormAction},
        widget::{KeyModifiers, UiEvent, Widget},
        widgets::on_screen_keyboard::{OnScreenKeyboard, OnScreenKeyboardAction},
    },
};

/// Scene that collects the username and e-mail, then requests a password-
/// reset code from the API.  On success, transitions to
/// `SceneType::EnterResetCode`.
pub struct RequestResetScene {
    /// The reset-request form widget.
    form: RequestResetForm,
    /// Certificate-mismatch dialog (shown when server cert changes).
    cert_dialog: Option<CertDialog>,
    /// Queued scene transition from widget actions.
    pending_scene: Option<SceneType>,

    is_submitting: bool,
    api_result_rx: Option<Receiver<Result<String, String>>>,
    request_thread: Option<std::thread::JoinHandle<()>>,

    mouse_x: i32,
    mouse_y: i32,

    /// Rising-edge tracker for controller → nav events.
    controller_nav: ControllerNavState,
    /// On-screen keyboard for controller text input.
    keyboard: OnScreenKeyboard,
}

impl RequestResetScene {
    /// Creates a new `RequestResetScene` with empty form fields.
    pub fn new() -> Self {
        RequestResetScene {
            form: RequestResetForm::new(),
            cert_dialog: None,
            pending_scene: None,
            is_submitting: false,
            api_result_rx: None,
            request_thread: None,
            mouse_x: 0,
            mouse_y: 0,
            controller_nav: ControllerNavState::new(),
            keyboard: OnScreenKeyboard::new(),
        }
    }

    /// Validates inputs and calls the password-reset request API endpoint.
    ///
    /// # Arguments
    /// * `base_url` – API base URL.
    /// * `username` – account username.
    /// * `email` – e-mail address on the account.
    ///
    /// # Returns
    /// `Ok(message)` on success, `Err(message)` on failure.
    fn request_reset(base_url: &str, username: &str, email: &str) -> Result<String, String> {
        let username = username.trim();
        let email = email.trim();

        if username.is_empty() {
            return Err("Username is required".to_string());
        }
        if email.is_empty() {
            return Err("E-mail is required".to_string());
        }

        account_api::request_password_reset(base_url, username, email)
    }

    /// Starts an asynchronous reset-request using the current form values.
    ///
    /// # Arguments
    ///
    /// * `app_state` - Shared application state that provides the API base URL.
    fn begin_reset_request(&mut self, app_state: &AppState) {
        let (sender, receiver) = mpsc::channel::<Result<String, String>>();

        self.form.set_error(None);
        self.form.set_info(None);
        self.is_submitting = true;
        self.form.set_submitting(true);
        self.api_result_rx = Some(receiver);

        let base_url = app_state.api.base_url.clone();
        let username = self.form.username().to_owned();
        let email = self.form.email().to_owned();

        self.request_thread = Some(std::thread::spawn(move || {
            let result = Self::request_reset(&base_url, &username, &email);
            if let Err(error) = sender.send(result) {
                log::error!("Failed to send reset request result: {}", error);
            }
        }));
    }
}

impl Scene for RequestResetScene {
    fn handle_event(&mut self, app_state: &mut AppState<'_>, event: &Event) -> Option<SceneType> {
        if let Event::MouseMotion { x, y, .. } = event {
            self.mouse_x = *x;
            self.mouse_y = *y;
        }

        let modifiers =
            KeyModifiers::from_sdl2(Mod::from_bits_truncate(sdl2::keyboard::Mod::empty().bits()));

        // When the on-screen keyboard is visible, intercept raw SDL
        // controller buttons for keyboard-specific actions.
        if self.keyboard.is_visible() {
            if let Event::ControllerButtonDown { button, .. } = event {
                match button {
                    Btn::X => {
                        self.keyboard.handle_event(&UiEvent::KeyboardToggleShift);
                        return self.pending_scene.take();
                    }
                    Btn::Start => {
                        self.keyboard.handle_event(&UiEvent::KeyboardDismiss);
                        for kb_action in self.keyboard.take_actions() {
                            if let OnScreenKeyboardAction::Dismiss = kb_action {
                                self.keyboard.hide();
                            }
                        }
                        return self.pending_scene.take();
                    }
                    Btn::DPadUp => {
                        self.keyboard.handle_event(&UiEvent::KeyboardRowUp);
                        return self.pending_scene.take();
                    }
                    Btn::DPadDown => {
                        self.keyboard.handle_event(&UiEvent::KeyboardRowDown);
                        return self.pending_scene.take();
                    }
                    _ => {}
                }
            }
        }

        // Controller → nav event (rising-edge gated for axes).
        if let Some(nav_event) = self.controller_nav.process_event(event) {
            if self.keyboard.is_visible() {
                self.keyboard.handle_event(&nav_event);
                for kb_action in self.keyboard.take_actions() {
                    match kb_action {
                        OnScreenKeyboardAction::TypeChar(ch) => {
                            self.form.inject_char(ch);
                        }
                        OnScreenKeyboardAction::Backspace => {
                            self.form.inject_backspace();
                        }
                        OnScreenKeyboardAction::Dismiss => {
                            self.keyboard.hide();
                        }
                    }
                }
            } else if self.cert_dialog.is_some() {
                let dialog = self.cert_dialog.as_mut().unwrap();
                dialog.handle_event(&nav_event);
            } else {
                self.form.handle_event(&nav_event);
            }
        }

        if let Some(ui_event) = ui::sdl_to_ui_event(event, self.mouse_x, self.mouse_y, modifiers) {
            // Certificate dialog blocks all input to the form behind it.
            if self.cert_dialog.is_some() {
                let dialog = self.cert_dialog.as_mut().unwrap();
                dialog.handle_event(&ui_event);
                let actions = dialog.take_cert_actions();

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
                            self.begin_reset_request(app_state);
                        }
                        Err(err) => {
                            self.form
                                .set_error(Some(format!("Failed to update known hosts: {err}")));
                        }
                    }
                }

                if reject {
                    self.cert_dialog = None;
                }

                return self.pending_scene.take();
            }

            // Forward to form.
            self.form.handle_event(&ui_event);
        }

        // Drain form actions unconditionally — controller nav events bypass
        // the sdl_to_ui_event block so actions must be processed regardless.
        for action in self.form.take_actions() {
            match action {
                RequestResetFormAction::Submit { username, email } => {
                    log::info!(
                        "Reset request submitted for username={}, email={}",
                        username,
                        email
                    );
                    self.begin_reset_request(app_state);
                }
                RequestResetFormAction::Cancel => {
                    log::info!("Cancel clicked");
                    self.pending_scene = Some(SceneType::Login);
                }
                RequestResetFormAction::OpenKeyboard(field_idx) => {
                    self.form.set_text_focus(field_idx);
                    self.keyboard.show();
                }
            }
        }

        self.pending_scene.take()
    }

    fn update(&mut self, app_state: &mut AppState<'_>, dt: Duration) -> Option<SceneType> {
        app_state.panning_background.update(dt);
        self.form.update(dt);

        if self.is_submitting {
            let result = if let Some(receiver) = &self.api_result_rx {
                match receiver.try_recv() {
                    Ok(result) => Some(result),
                    Err(TryRecvError::Empty) => None,
                    Err(TryRecvError::Disconnected) => {
                        Some(Err("Reset request task failed unexpectedly".to_string()))
                    }
                }
            } else {
                None
            };

            if let Some(result) = result {
                self.is_submitting = false;
                self.form.set_submitting(false);
                self.api_result_rx = None;

                match result {
                    Ok(_message) => {
                        // Stash username for the next scene.
                        app_state.reset_username = Some(self.form.username().trim().to_owned());
                        return Some(SceneType::EnterResetCode);
                    }
                    Err(error) => {
                        if let Some(mismatch) = cert_trust::take_last_fingerprint_mismatch() {
                            self.cert_dialog = Some(CertDialog::new(
                                &mismatch.host,
                                &mismatch.expected_fingerprint,
                                &mismatch.received_fingerprint,
                            ));
                        }
                        self.form.set_error(Some(error));
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
            panning_background,
            gfx_cache,
            ..
        } = app_state;
        let mut ctx = RenderContext {
            canvas,
            gfx: gfx_cache,
        };

        panning_background.render(&mut ctx)?;
        self.form.render(&mut ctx)?;
        self.keyboard.render(&mut ctx)?;

        if let Some(ref mut dialog) = self.cert_dialog {
            dialog.render(&mut ctx)?;
        }

        Ok(())
    }
}
