use std::{collections::HashMap, time::Duration};

use egui_sdl2::egui;
use sdl2::{event::Event, render::Canvas, video::Window};

use crate::state::AppState;

pub trait Scene {
    fn handle_event(&mut self, app_state: &mut AppState, event: &Event) -> Option<SceneType>;

    fn update(&mut self, app_state: &mut AppState, dt: Duration) -> Option<SceneType>;

    fn render_world(
        &mut self,
        app_state: &mut AppState,
        canvas: &mut Canvas<Window>,
    ) -> Result<(), String>;

    fn render_ui(&mut self, app_state: &mut AppState, ctx: &egui::Context) -> Option<SceneType>;
}

#[derive(Hash, Eq, PartialEq, Debug, Copy, Clone)]
pub enum SceneType {
    Login,
    CharacterCreation,
    CharacterSelection,
    Game,
    NewAccount,
    Exit,
}

pub struct SceneManager {
    active_scene: SceneType,
    scenes: HashMap<SceneType, Box<dyn Scene>>,
}

impl SceneManager {
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

    pub fn get_scene(&self) -> SceneType {
        self.active_scene
    }

    pub fn handle_event(&mut self, app_state: &mut AppState, event: &Event) {
        if self.active_scene == SceneType::Exit {
            return;
        }

        let possible_next_scene = self
            .scenes
            .get_mut(&self.active_scene)
            .unwrap()
            .handle_event(app_state, event);

        self.apply_scene_change(possible_next_scene);
    }

    pub fn update(&mut self, app_state: &mut AppState, dt: Duration) {
        if self.active_scene == SceneType::Exit {
            return;
        }

        let possible_next_scene = self
            .scenes
            .get_mut(&self.active_scene)
            .unwrap()
            .update(app_state, dt);

        self.apply_scene_change(possible_next_scene);
    }

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

    pub fn render_ui(&mut self, app_state: &mut AppState, ctx: &egui::Context) {
        if self.active_scene == SceneType::Exit {
            return;
        }

        let possible_next_scene = self
            .scenes
            .get_mut(&self.active_scene)
            .unwrap()
            .render_ui(app_state, ctx);

        self.apply_scene_change(possible_next_scene);
    }

    pub fn request_scene_change(&mut self, scene_type: SceneType) {
        self.apply_scene_change(Some(scene_type));
    }

    pub fn set_scene(&mut self, scene_type: SceneType) {
        if self.scenes.contains_key(&scene_type) {
            log::info!("Switching to scene: {:?}", scene_type);
        } else {
            log::error!("Attempted to switch to unknown scene: {:?}", scene_type);
        }
        self.active_scene = scene_type;
    }

    fn apply_scene_change(&mut self, next_scene: Option<SceneType>) {
        let Some(scene) = next_scene else {
            return;
        };

        log::info!("Scene change requested: {:?}", scene);
        self.set_scene(scene);
    }
}
