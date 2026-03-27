use std::{
    sync::mpsc::{self, Receiver, TryRecvError},
    time::Duration,
};

use sdl2::{event::Event, keyboard::Mod, render::Canvas, video::Window};

use crate::{
    account_api, cert_trust,
    scenes::scene::{Scene, SceneType},
    state::AppState,
    ui::{
        self, RenderContext,
        forms::cert_dialog::{CertDialog, CertDialogAction},
        forms::enter_reset_code_form::{EnterResetCodeForm, EnterResetCodeFormAction},
        widget::{KeyModifiers, Widget},
    },
};

/// Scene that collects the 6-digit reset code and new password, then
/// confirms the password reset via the API.  On success, transitions
/// back to `SceneType::Login`.
pub struct EnterResetCodeScene {
    /// The enter-reset-code form widget.
    form: EnterResetCodeForm,
    /// Certificate-mismatch dialog (shown when server cert changes).
    cert_dialog: Option<CertDialog>,
    /// Queued scene transition from widget actions.
    pending_scene: Option<SceneType>,

    is_submitting: bool,
    api_result_rx: Option<Receiver<Result<String, String>>>,
    confirm_thread: Option<std::thread::JoinHandle<()>>,

    mouse_x: i32,
    mouse_y: i32,
}

impl EnterResetCodeScene {
    /// Creates a new `EnterResetCodeScene` with empty form fields.
    pub fn new() -> Self {
        EnterResetCodeScene {
            form: EnterResetCodeForm::new(),
            cert_dialog: None,
            pending_scene: None,
            is_submitting: false,
            api_result_rx: None,
            confirm_thread: None,
            mouse_x: 0,
            mouse_y: 0,
        }
    }

    /// Validates inputs and calls the password-reset confirm API endpoint.
    ///
    /// # Arguments
    /// * `base_url` – API base URL.
    /// * `username` – account username (carried from previous scene).
    /// * `code` – 6-digit reset code.
    /// * `new_password` – desired new password.
    /// * `confirm_password` – confirmation of new password.
    ///
    /// # Returns
    /// `Ok(message)` on success, `Err(message)` on failure.
    fn confirm_reset(
        base_url: &str,
        username: &str,
        code: &str,
        new_password: &str,
        confirm_password: &str,
    ) -> Result<String, String> {
        let code = code.trim();
        let new_password = new_password.trim();
        let confirm_password = confirm_password.trim();

        if code.is_empty() {
            return Err("Reset code is required".to_string());
        }
        if new_password.is_empty() {
            return Err("New password is required".to_string());
        }
        if new_password != confirm_password {
            return Err("Passwords do not match".to_string());
        }

        account_api::confirm_password_reset(base_url, username, code, new_password)
    }

    /// Starts an asynchronous reset-confirm request using the current form values.
    ///
    /// # Arguments
    ///
    /// * `app_state` - Shared application state (provides API base URL and username).
    fn begin_confirm_request(&mut self, app_state: &AppState) {
        let username = match &app_state.reset_username {
            Some(u) => u.clone(),
            None => {
                self.form
                    .set_error(Some("Missing username — go back and try again".to_string()));
                return;
            }
        };

        let (sender, receiver) = mpsc::channel::<Result<String, String>>();

        self.form.set_error(None);
        self.form.set_info(None);
        self.is_submitting = true;
        self.form.set_submitting(true);
        self.api_result_rx = Some(receiver);

        let base_url = app_state.api.base_url.clone();
        let code = self.form.code().to_owned();
        let new_password = self.form.password().to_owned();
        let confirm_password = self.form.confirm_password().to_owned();

        self.confirm_thread = Some(std::thread::spawn(move || {
            let result = Self::confirm_reset(
                &base_url,
                &username,
                &code,
                &new_password,
                &confirm_password,
            );
            if let Err(error) = sender.send(result) {
                log::error!("Failed to send reset confirm result: {}", error);
            }
        }));
    }
}

impl Scene for EnterResetCodeScene {
    fn handle_event(&mut self, app_state: &mut AppState<'_>, event: &Event) -> Option<SceneType> {
        if let Event::MouseMotion { x, y, .. } = event {
            self.mouse_x = *x;
            self.mouse_y = *y;
        }

        let modifiers =
            KeyModifiers::from_sdl2(Mod::from_bits_truncate(sdl2::keyboard::Mod::empty().bits()));

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
                            self.begin_confirm_request(app_state);
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

            // Process form actions.
            for action in self.form.take_actions() {
                match action {
                    EnterResetCodeFormAction::Submit {
                        code,
                        new_password: _,
                    } => {
                        log::info!("Reset confirm submitted with code={}", code);
                        self.begin_confirm_request(app_state);
                    }
                    EnterResetCodeFormAction::Cancel => {
                        log::info!("Cancel clicked");
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

        if self.is_submitting {
            let result = if let Some(receiver) = &self.api_result_rx {
                match receiver.try_recv() {
                    Ok(result) => Some(result),
                    Err(TryRecvError::Empty) => None,
                    Err(TryRecvError::Disconnected) => {
                        Some(Err("Reset confirm task failed unexpectedly".to_string()))
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
                        app_state.reset_username = None;
                        return Some(SceneType::Login);
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

        if let Some(ref mut dialog) = self.cert_dialog {
            dialog.render(&mut ctx)?;
        }

        Ok(())
    }
}
