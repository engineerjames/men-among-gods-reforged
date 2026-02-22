use std::{collections::HashMap, time::Duration};

use egui_sdl2::egui;
use sdl2::{event::Event, render::Canvas, video::Window};

use crate::state::AppState;

/// Trait implemented by each game scene (login, character selection, gameplay, etc.).
///
/// The scene manager drives the lifecycle: `on_enter` → frame loop (`handle_event`,
/// `update`, `render_world`, `render_ui`) → `on_exit`. Returning `Some(SceneType)`
/// from any frame method triggers a scene transition.
pub trait Scene {
    /// Called once when the scene becomes active.
    fn on_enter(&mut self, _app_state: &mut AppState) {}

    /// Called once when the scene is about to be replaced by another.
    fn on_exit(&mut self, _app_state: &mut AppState) {}

    /// Processes a single SDL event. Returns `Some(SceneType)` to request a scene change.
    fn handle_event(&mut self, app_state: &mut AppState, event: &Event) -> Option<SceneType>;

    /// Per-frame logic update. `dt` is the time elapsed since the last frame.
    fn update(&mut self, app_state: &mut AppState, dt: Duration) -> Option<SceneType>;

    /// Renders non-UI world elements (tiles, sprites) onto the SDL canvas.
    fn render_world(
        &mut self,
        app_state: &mut AppState,
        canvas: &mut Canvas<Window>,
    ) -> Result<(), String>;

    /// Renders the egui immediate-mode UI overlay. Returns `Some(SceneType)` to request a scene change.
    fn render_ui(&mut self, app_state: &mut AppState, ctx: &egui::Context) -> Option<SceneType>;
}

/// Identifies which scene is active. Used as `HashMap` keys and for scene transition requests.
#[derive(Hash, Eq, PartialEq, Debug, Copy, Clone)]
pub enum SceneType {
    Login,
    CharacterCreation,
    CharacterSelection,
    Game,
    NewAccount,
    Exit,
}

/// Owns all scene instances and drives the scene lifecycle (enter, update, render, exit).
///
/// Exactly one scene is active at a time. Scene transitions are requested by returning
/// `Some(SceneType)` from any `Scene` method; `SceneManager` calls `on_exit` / `on_enter`
/// automatically.
pub struct SceneManager {
    active_scene: SceneType,
    scenes: HashMap<SceneType, Box<dyn Scene>>,
}

impl SceneManager {
    /// Creates a new `SceneManager` pre-populated with all known scene implementations.
    /// The initial active scene is `SceneType::Login`.
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
        }
    }

    /// Returns the currently active scene type.
    pub fn get_scene(&self) -> SceneType {
        self.active_scene
    }

    /// Forwards an SDL event to the active scene and applies any resulting scene change.
    pub fn handle_event(&mut self, app_state: &mut AppState, event: &Event) {
        if self.active_scene == SceneType::Exit {
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
    pub fn update(&mut self, app_state: &mut AppState, dt: Duration) {
        if self.active_scene == SceneType::Exit {
            return;
        }

        let possible_next_scene = self
            .scenes
            .get_mut(&self.active_scene)
            .unwrap()
            .update(app_state, dt);

        self.apply_scene_change(possible_next_scene, app_state);
    }

    /// Delegates world rendering to the active scene.
    pub fn render_world(&mut self, app_state: &mut AppState, canvas: &mut Canvas<Window>) {
        if self.active_scene == SceneType::Exit {
            return;
        }

        self.scenes
            .get_mut(&self.active_scene)
            .unwrap()
            .render_world(app_state, canvas)
            .unwrap_or_else(|err| log::error!("Error rendering world: {}", err));
    }

    /// Delegates UI rendering to the active scene and applies any resulting scene change.
    pub fn render_ui(&mut self, app_state: &mut AppState, ctx: &egui::Context) {
        if self.active_scene == SceneType::Exit {
            return;
        }

        let possible_next_scene = self
            .scenes
            .get_mut(&self.active_scene)
            .unwrap()
            .render_ui(app_state, ctx);

        self.apply_scene_change(possible_next_scene, app_state);
    }

    /// Externally requests a scene transition (e.g. from the main loop on quit).
    pub fn request_scene_change(&mut self, scene_type: SceneType, app_state: &mut AppState) {
        self.apply_scene_change(Some(scene_type), app_state);
    }

    /// Performs the actual scene switch: calls `on_exit` on the current scene, swaps the
    /// active scene type, and calls `on_enter` on the new scene.
    pub fn set_scene(&mut self, scene_type: SceneType, app_state: &mut AppState) {
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

    /// If `next_scene` is `Some`, delegates to `set_scene` to perform the transition.
    fn apply_scene_change(&mut self, next_scene: Option<SceneType>, app_state: &mut AppState) {
        let Some(scene) = next_scene else {
            return;
        };

        log::info!("Scene change requested: {:?}", scene);
        self.set_scene(scene, app_state);
    }
}
