use std::{collections::HashMap, time::Duration};

use sdl2::{
    event::Event,
    pixels::Color,
    rect::Rect,
    render::{BlendMode, Canvas},
    video::Window,
};

use crate::{
    constants::{TARGET_HEIGHT_INT, TARGET_WIDTH_INT},
    state::AppState,
    ui::widget::Bounds,
};

/// Duration of the fade-to-black before a new scene is swapped in.
const SCENE_FADE_OUT: Duration = Duration::from_millis(120);
/// Duration of the fade-from-black once the new scene becomes active.
const SCENE_FADE_IN: Duration = Duration::from_millis(120);

/// Tracks an in-progress fade transition between two scenes.
enum SceneTransition {
    /// No transition is in progress.
    None,
    /// Fading the outgoing scene to black before swapping to `target`.
    FadingOut {
        target: SceneType,
        elapsed: Duration,
    },
    /// Fading the (already swapped-in) new scene back in from black.
    FadingIn { elapsed: Duration },
}

/// Trait implemented by each game scene (login, character selection, gameplay, etc.).
///
/// The scene manager drives the lifecycle: `on_enter` --> frame loop (`handle_event`,
/// `update`, `render_world`) --> `on_exit`. Returning `Some(SceneType)`
/// from any frame method triggers a scene transition.
pub trait Scene {
    /// Called once when the scene becomes active.
    fn on_enter(&mut self, _app_state: &mut AppState<'_>) {}

    /// Called once when the scene is about to be replaced by another.
    fn on_exit(&mut self, _app_state: &mut AppState<'_>) {}

    /// Processes a single SDL event. Returns `Some(SceneType)` to request a scene change.
    fn handle_event(&mut self, app_state: &mut AppState<'_>, event: &Event) -> Option<SceneType>;

    /// Per-frame logic update. `dt` is the time elapsed since the last frame.
    fn update(&mut self, app_state: &mut AppState<'_>, dt: Duration) -> Option<SceneType>;

    /// Renders non-UI world elements (tiles, sprites) onto the SDL canvas.
    fn render_world(
        &mut self,
        app_state: &mut AppState<'_>,
        canvas: &mut Canvas<Window>,
    ) -> Result<(), String>;

    /// Lists named, clickable regions for the UI automation driver.
    ///
    /// Names are scene-local (e.g. `username`, `login`) and are resolved by
    /// [`crate::automation`] as `@name` or `@<scene>.name`. Bounds are in
    /// logical 960x540 coordinates.
    ///
    /// # Returns
    ///
    /// * `(name, bounds)` pairs; empty when the scene exposes no targets.
    fn automation_targets(&self) -> Vec<(&'static str, Bounds)> {
        Vec::new()
    }

    /// Performs a scene-specific automation action that has no natural input
    /// event (e.g. starting the render profiler).
    ///
    /// # Arguments
    ///
    /// * `_app_state` - Shared application state.
    /// * `action` - Action name from the automation script.
    /// * `_args` - Remaining script arguments.
    ///
    /// # Returns
    ///
    /// * `Err` describing the problem when the action is unsupported or fails.
    fn automation_action(
        &mut self,
        _app_state: &mut AppState<'_>,
        action: &str,
        _args: &[String],
    ) -> Result<(), String> {
        Err(format!("action `{action}` is not supported by this scene"))
    }
}

/// Identifies which scene is active. Used as `HashMap` keys and for scene transition requests.
#[derive(Hash, Eq, PartialEq, Debug, Copy, Clone)]
pub enum SceneType {
    Login,
    CharacterCreation,
    CharacterSelection,
    Game,
    NewAccount,
    RequestReset,
    EnterResetCode,
    Exit,
}

impl SceneType {
    /// Stable lowercase name used by automation scripts (`wait_scene`, `@scene.target`).
    ///
    /// # Returns
    ///
    /// * The script-facing name of this scene.
    pub fn automation_name(self) -> &'static str {
        match self {
            Self::Login => "login",
            Self::CharacterCreation => "char_create",
            Self::CharacterSelection => "char_select",
            Self::Game => "game",
            Self::NewAccount => "new_account",
            Self::RequestReset => "request_reset",
            Self::EnterResetCode => "enter_reset_code",
            Self::Exit => "exit",
        }
    }

    /// Parses a script-facing scene name (case-insensitive).
    ///
    /// # Arguments
    ///
    /// * `name` - Name as written in an automation script.
    ///
    /// # Returns
    ///
    /// * The matching scene, or `None` for unknown names.
    pub fn from_automation_name(name: &str) -> Option<Self> {
        const ALL: [SceneType; 8] = [
            SceneType::Login,
            SceneType::CharacterCreation,
            SceneType::CharacterSelection,
            SceneType::Game,
            SceneType::NewAccount,
            SceneType::RequestReset,
            SceneType::EnterResetCode,
            SceneType::Exit,
        ];
        let lower = name.to_ascii_lowercase();
        ALL.into_iter().find(|s| s.automation_name() == lower)
    }
}

/// Owns all scene instances and drives the scene lifecycle (enter, update, render, exit).
///
/// Exactly one scene is active at a time. Scene transitions are requested by returning
/// `Some(SceneType)` from any `Scene` method; `SceneManager` calls `on_exit` / `on_enter`
/// automatically.
pub struct SceneManager {
    active_scene: SceneType,
    scenes: HashMap<SceneType, Box<dyn Scene>>,
    transition: SceneTransition,
}

impl Default for SceneManager {
    fn default() -> Self {
        Self::new()
    }
}

impl SceneManager {
    /// Creates a new `SceneManager` pre-populated with all known scene implementations.
    /// The initial active scene is `SceneType::Login`.
    ///
    /// # Returns
    ///
    /// * A new instance configured by `new`.
    pub fn new() -> Self {
        let mut scene_map: HashMap<SceneType, Box<dyn Scene>> = HashMap::new();

        scene_map.insert(
            SceneType::Login,
            Box::new(crate::scenes::login::LoginScene::new()),
        );

        scene_map.insert(
            SceneType::Game,
            Box::new(crate::scenes::game::GameScene::new()),
        );

        scene_map.insert(
            SceneType::NewAccount,
            Box::new(crate::scenes::new_account::NewAccountScene::new()),
        );

        scene_map.insert(
            SceneType::RequestReset,
            Box::new(crate::scenes::request_reset::RequestResetScene::new()),
        );

        scene_map.insert(
            SceneType::EnterResetCode,
            Box::new(crate::scenes::enter_reset_code::EnterResetCodeScene::new()),
        );

        scene_map.insert(
            SceneType::CharacterCreation,
            Box::new(crate::scenes::character_creation::CharacterCreationScene::new()),
        );

        scene_map.insert(
            SceneType::CharacterSelection,
            Box::new(crate::scenes::character_selection::CharacterSelectionScene::new()),
        );

        scene_map.insert(
            SceneType::Exit,
            Box::new(crate::scenes::exit::ExitScene::new()),
        );

        SceneManager {
            active_scene: SceneType::Login,
            scenes: scene_map,
            transition: SceneTransition::None,
        }
    }

    /// Returns the currently active scene type.
    ///
    /// # Returns
    ///
    /// * Value returned by `get_scene`.
    pub fn get_scene(&self) -> SceneType {
        self.active_scene
    }

    /// Forwards an SDL event to the active scene and applies any resulting scene change.
    ///
    /// # Arguments
    ///
    /// * `app_state` - Value passed to `handle_event`.
    /// * `event` - Input event handled by this function.
    pub fn handle_event(&mut self, app_state: &mut AppState<'_>, event: &Event) {
        if self.active_scene == SceneType::Exit {
            return;
        }

        // Ignore input while a fade transition is in progress so it can't
        // trigger another scene change mid-fade.
        if !matches!(self.transition, SceneTransition::None) {
            return;
        }

        let possible_next_scene = self
            .scenes
            .get_mut(&self.active_scene)
            .unwrap()
            .handle_event(app_state, event);

        self.apply_scene_change(possible_next_scene, app_state);
    }

    /// Runs the active scene's per-frame update and applies any resulting scene change.
    ///
    /// # Arguments
    ///
    /// * `app_state` - Value passed to `update`.
    /// * `dt` - Value passed to `update`.
    pub fn update(&mut self, app_state: &mut AppState<'_>, dt: Duration) {
        if self.active_scene == SceneType::Exit {
            return;
        }

        // Advance any in-progress fade transition. While fading out, the
        // outgoing scene is still updated (frozen visuals aside) so timers
        // and animations don't jump once the fade completes.
        match &mut self.transition {
            SceneTransition::None => {}
            SceneTransition::FadingOut { target, elapsed } => {
                *elapsed += dt;
                if *elapsed >= SCENE_FADE_OUT {
                    let target = *target;
                    self.set_scene(target, app_state);
                    self.transition = SceneTransition::FadingIn {
                        elapsed: Duration::ZERO,
                    };
                }
                return;
            }
            SceneTransition::FadingIn { elapsed } => {
                *elapsed += dt;
                if *elapsed >= SCENE_FADE_IN {
                    self.transition = SceneTransition::None;
                }
            }
        }

        let possible_next_scene = self
            .scenes
            .get_mut(&self.active_scene)
            .unwrap()
            .update(app_state, dt);

        self.apply_scene_change(possible_next_scene, app_state);
    }

    /// Delegates world rendering to the active scene.
    ///
    /// # Arguments
    ///
    /// * `app_state` - Value passed to `render_world`.
    /// * `canvas` - SDL2 canvas used by this function.
    pub fn render_world(&mut self, app_state: &mut AppState<'_>, canvas: &mut Canvas<Window>) {
        if self.active_scene == SceneType::Exit {
            return;
        }

        self.scenes
            .get_mut(&self.active_scene)
            .unwrap()
            .render_world(app_state, canvas)
            .unwrap_or_else(|err| log::error!("Error rendering world: {}", err));

        let alpha = match &self.transition {
            SceneTransition::None => 0.0,
            SceneTransition::FadingOut { elapsed, .. } => {
                (elapsed.as_secs_f32() / SCENE_FADE_OUT.as_secs_f32()).clamp(0.0, 1.0)
            }
            SceneTransition::FadingIn { elapsed } => {
                1.0 - (elapsed.as_secs_f32() / SCENE_FADE_IN.as_secs_f32()).clamp(0.0, 1.0)
            }
        };

        if alpha > 0.0 {
            let prev_blend_mode = canvas.blend_mode();
            canvas.set_blend_mode(BlendMode::Blend);
            canvas.set_draw_color(Color::RGBA(0, 0, 0, (alpha * 255.0) as u8));
            let _ = canvas.fill_rect(Rect::new(0, 0, TARGET_WIDTH_INT, TARGET_HEIGHT_INT));
            canvas.set_blend_mode(prev_blend_mode);
        }
    }

    /// Externally requests a scene transition (e.g. from the main loop on quit).
    ///
    /// # Arguments
    ///
    /// * `scene_type` - Value passed to `request_scene_change`.
    /// * `app_state` - Value passed to `request_scene_change`.
    pub fn request_scene_change(&mut self, scene_type: SceneType, app_state: &mut AppState<'_>) {
        self.apply_scene_change(Some(scene_type), app_state);
    }

    /// Whether a fade transition is in progress (input is ignored meanwhile).
    ///
    /// # Returns
    ///
    /// * `true` while fading out of or into a scene.
    pub fn is_transitioning(&self) -> bool {
        !matches!(self.transition, SceneTransition::None)
    }

    /// Named automation targets exposed by the active scene.
    ///
    /// # Returns
    ///
    /// * `(name, bounds)` pairs from [`Scene::automation_targets`].
    pub fn automation_targets(&self) -> Vec<(&'static str, Bounds)> {
        self.scenes
            .get(&self.active_scene)
            .map(|scene| scene.automation_targets())
            .unwrap_or_default()
    }

    /// Forwards an automation action to the active scene.
    ///
    /// # Arguments
    ///
    /// * `app_state` - Shared application state.
    /// * `action` - Action name from the automation script.
    /// * `args` - Remaining script arguments.
    ///
    /// # Returns
    ///
    /// * Result of [`Scene::automation_action`].
    pub fn automation_action(
        &mut self,
        app_state: &mut AppState<'_>,
        action: &str,
        args: &[String],
    ) -> Result<(), String> {
        match self.scenes.get_mut(&self.active_scene) {
            Some(scene) => scene.automation_action(app_state, action, args),
            None => Err("no active scene".to_owned()),
        }
    }

    /// Performs the actual scene switch: calls `on_exit` on the current scene, swaps the
    /// active scene type, and calls `on_enter` on the new scene.
    ///
    /// # Arguments
    ///
    /// * `scene_type` - Value passed to `set_scene`.
    /// * `app_state` - Value passed to `set_scene`.
    pub fn set_scene(&mut self, scene_type: SceneType, app_state: &mut AppState<'_>) {
        if scene_type == self.active_scene {
            return;
        }

        if self.scenes.contains_key(&scene_type) {
            log::info!("Switching to scene: {:?}", scene_type);
        } else {
            log::error!("Attempted to switch to unknown scene: {:?}", scene_type);
            return;
        }

        if let Some(current_scene) = self.scenes.get_mut(&self.active_scene) {
            log::info!("Calling on_exit for scene: {:?}", self.active_scene);
            current_scene.on_exit(app_state);
        }

        self.active_scene = scene_type;

        if let Some(next_scene) = self.scenes.get_mut(&self.active_scene) {
            log::info!("Calling on_enter for scene: {:?}", self.active_scene);
            next_scene.on_enter(app_state);
        }
    }

    /// If `next_scene` is `Some`, starts a fade-out transition that will swap to the
    /// requested scene once the fade completes (except for `SceneType::Exit`, which is
    /// applied immediately so quitting isn't delayed).
    fn apply_scene_change(&mut self, next_scene: Option<SceneType>, app_state: &mut AppState<'_>) {
        let Some(scene) = next_scene else {
            return;
        };

        if scene == self.active_scene {
            return;
        }

        log::info!("Scene change requested: {:?}", scene);

        if scene == SceneType::Exit {
            self.transition = SceneTransition::None;
            self.set_scene(scene, app_state);
            return;
        }

        self.transition = SceneTransition::FadingOut {
            target: scene,
            elapsed: Duration::ZERO,
        };
    }
}
