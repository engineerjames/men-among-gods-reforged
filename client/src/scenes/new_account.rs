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
        self,
        cert_dialog::{CertDialog, CertDialogAction},
        new_account_form::{NewAccountForm, NewAccountFormAction},
        widget::{KeyModifiers, Widget},
        RenderContext,
    },
};

/// Scene that presents the new-account registration form.
///
/// Collects e-mail, username and password, then creates the account
/// on a background thread via the REST API. On success, transitions
/// back to `SceneType::Login`.
pub struct NewAccountScene {
    /// The account registration form widget.
    form: NewAccountForm,
    /// Certificate-mismatch dialog (shown when server cert changes).
    cert_dialog: Option<CertDialog>,
    /// Queued scene transition from widget actions.
    pending_scene: Option<SceneType>,

    is_submitting: bool,
    api_result_rx: Option<Receiver<Result<(), String>>>,
    account_thread: Option<std::thread::JoinHandle<()>>,

    mouse_x: i32,
    mouse_y: i32,
}

impl NewAccountScene {
    /// Creates a new `NewAccountScene` with empty form fields.
    pub fn new() -> Self {
        NewAccountScene {
            form: NewAccountForm::new(),
            cert_dialog: None,
            pending_scene: None,
            is_submitting: false,
            api_result_rx: None,
            account_thread: None,
            mouse_x: 0,
            mouse_y: 0,
        }
    }

    /// Validates inputs and calls the account-creation API endpoint.
    ///
    /// # Arguments
    /// * `base_url` – API base URL.
    /// * `email` – user-supplied e-mail address.
    /// * `username` – desired account name.
    /// * `password` – desired password.
    ///
    /// # Returns
    /// `Ok(())` on success, `Err(message)` on validation or API failure.
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

    /// Starts an asynchronous account-creation request using current form values.
    ///
    /// # Arguments
    ///
    /// * `app_state` - Shared application state that provides the API base URL.
    fn begin_account_creation_request(&mut self, app_state: &AppState) {
        let (sender, receiver) = mpsc::channel::<Result<(), String>>();

        self.form.set_error(None);
        self.is_submitting = true;
        self.form.set_submitting(true);
        self.api_result_rx = Some(receiver);

        let base_url = app_state.api.base_url.clone();
        let email = self.form.email().to_owned();
        let username = self.form.username().to_owned();
        let password = self.form.password().to_owned();

        self.account_thread = Some(std::thread::spawn(move || {
            let result = Self::create_account(&base_url, &email, &username, &password);
            if let Err(error) = sender.send(result) {
                log::error!("Failed to send account creation result: {}", error);
            }
        }));
    }
}

impl Scene for NewAccountScene {
    fn handle_event(&mut self, app_state: &mut AppState, event: &Event) -> Option<SceneType> {
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
                            self.begin_account_creation_request(app_state);
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
                    NewAccountFormAction::Create {
                        email,
                        username,
                        password: _,
                    } => {
                        log::info!(
                            "Create new account clicked with email={}, username={}",
                            email,
                            username
                        );
                        self.begin_account_creation_request(app_state);
                    }
                    NewAccountFormAction::Cancel => {
                        log::info!("Cancel clicked");
                        self.pending_scene = Some(SceneType::Login);
                    }
                }
            }
        }

        self.pending_scene.take()
    }

    fn update(&mut self, app_state: &mut AppState, dt: Duration) -> Option<SceneType> {
        app_state.panning_background.update(dt);
        self.form.update(dt);

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
                self.form.set_submitting(false);
                self.api_result_rx = None;

                match result {
                    Ok(()) => return Some(SceneType::Login),
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
        app_state: &mut AppState,
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
        self.form.render(&mut ctx)?;

        if let Some(ref mut dialog) = self.cert_dialog {
            dialog.render(&mut ctx)?;
        }

        Ok(())
    }
}
