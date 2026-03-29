use std::{
    sync::mpsc::{self, TryRecvError},
    time::Duration,
};

use mag_core::names;
use sdl2::{event::Event, keyboard::Mod, render::Canvas, video::Window};

use crate::{
    account_api,
    scenes::scene::{Scene, SceneType},
    state::AppState,
    ui::{
        self, RenderContext,
        controller_nav::ControllerNavState,
        forms::character_creation_form::{CharacterCreationForm, CharacterCreationFormAction},
        widget::{KeyModifiers, Widget},
    },
};

/// Scene for creating a new in-game character.
///
/// Lets the player choose a name, description, class (race) and sex.
/// The creation request runs on a background thread; on success the
/// scene transitions to `CharacterSelection`.
pub struct CharacterCreationScene {
    error: Option<String>,
    form: CharacterCreationForm,
    is_busy: bool,
    account_rx: Option<mpsc::Receiver<Result<account_api::CharacterSummary, String>>>,
    account_thread: Option<std::thread::JoinHandle<()>>,
    pending_scene: Option<SceneType>,
    mouse_x: i32,
    mouse_y: i32,

    /// Rising-edge tracker for controller → nav events.
    controller_nav: ControllerNavState,
}

impl CharacterCreationScene {
    /// Creates a new `CharacterCreationScene` with default selections.
    pub fn new() -> Self {
        Self {
            error: None,
            form: CharacterCreationForm::new(),
            is_busy: false,
            account_rx: None,
            account_thread: None,
            pending_scene: None,
            mouse_x: 0,
            mouse_y: 0,
            controller_nav: ControllerNavState::new(),
        }
    }
}

impl Scene for CharacterCreationScene {
    fn handle_event(&mut self, _app_state: &mut AppState<'_>, event: &Event) -> Option<SceneType> {
        if let Event::MouseMotion { x, y, .. } = event {
            self.mouse_x = *x;
            self.mouse_y = *y;
        }

        let modifiers =
            KeyModifiers::from_sdl2(Mod::from_bits_truncate(sdl2::keyboard::Mod::empty().bits()));

        // Controller → nav event (rising-edge gated for axes).
        if let Some(nav_event) = self.controller_nav.process_event(event) {
            self.form.handle_event(&nav_event);
        }

        if let Some(ui_event) = ui::sdl_to_ui_event(event, self.mouse_x, self.mouse_y, modifiers) {
            self.form.handle_event(&ui_event);
        }

        // Drain form actions unconditionally — controller nav events bypass
        // the sdl_to_ui_event block so actions must be processed regardless.
        for action in self.form.take_actions() {
            match action {
                CharacterCreationFormAction::Create {
                    name,
                    description: _,
                    class: _,
                    sex: _,
                } => {
                    let name = name.trim().to_string();

                    if name.is_empty() {
                        self.form
                            .set_error(Some("Character name is required".to_string()));
                        continue;
                    }

                    self.is_busy = true;
                    self.form.set_busy(true);
                    self.form.set_error(None);
                    self.error = None;
                    // Thread spawn deferred to update() which has app_state.
                }
                CharacterCreationFormAction::RandomName => {
                    let new_name = names::randomly_generate_name();
                    self.form.set_name(&new_name);
                }
                CharacterCreationFormAction::Back => {
                    self.error = None;
                    self.pending_scene = Some(SceneType::CharacterSelection);
                }
            }
        }

        self.pending_scene.take()
    }

    fn update(&mut self, app_state: &mut AppState<'_>, dt: Duration) -> Option<SceneType> {
        app_state.panning_background.update(dt);
        self.form.update(dt);

        // If the form submitted a create action and we haven't started the thread yet
        if self.is_busy && self.account_rx.is_none() && self.account_thread.is_none() {
            let Some(token) = app_state.api.token.as_deref() else {
                self.form
                    .set_error(Some("Missing account session token".to_string()));
                self.is_busy = false;
                self.form.set_busy(false);
                return None;
            };

            let base_url = app_state.api.base_url.clone();
            let token = token.to_string();
            let name = self.form.name_input_value().to_owned();
            let description = {
                let d = self.form.description_input_value().trim().to_string();
                if d.is_empty() { None } else { Some(d) }
            };
            let sex = self.form.selected_sex();
            let race = self.form.selected_class();

            let (tx, rx) = mpsc::channel();
            self.account_thread = Some(std::thread::spawn(move || {
                let result = account_api::create_character(
                    &base_url,
                    &token,
                    &name,
                    description.as_deref(),
                    sex,
                    race,
                );
                let _ = tx.send(result);
            }));
            self.account_rx = Some(rx);
        }

        if !self.is_busy {
            return None;
        }

        let result = if let Some(receiver) = &self.account_rx {
            match receiver.try_recv() {
                Ok(result) => Some(result),
                Err(TryRecvError::Empty) => None,
                Err(TryRecvError::Disconnected) => {
                    Some(Err("Character creation failed: channel closed".to_string()))
                }
            }
        } else {
            None
        };

        let Some(result) = result else {
            return None;
        };

        self.is_busy = false;
        self.form.set_busy(false);
        self.account_rx = None;

        if let Some(thread) = self.account_thread.take() {
            if thread.join().is_err() {
                log::error!("Character creation thread panicked");
            }
        }

        match result {
            Ok(summary) => {
                self.error = None;
                log::info!("Character creation successful: {}", summary.name);
                Some(SceneType::CharacterSelection)
            }
            Err(err) => {
                self.form.set_error(Some(err));
                None
            }
        }
    }

    fn on_enter(&mut self, _app_state: &mut AppState<'_>) {}

    fn render_world(
        &mut self,
        app_state: &mut AppState<'_>,
        canvas: &mut Canvas<Window>,
    ) -> Result<(), String> {
        // Render panning background.
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

        // Render the form.
        self.form.render(&mut ctx)?;

        Ok(())
    }
}
