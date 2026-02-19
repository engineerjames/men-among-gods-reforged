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
    app_state: AppState,
    scenes: HashMap<SceneType, Box<dyn Scene>>,
}

impl SceneManager {
    pub fn new(app_state: AppState) -> Self {
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

        SceneManager {
            active_scene: SceneType::Login,
            app_state,
            scenes: scene_map,
        }
    }

    pub fn handle_event(&mut self, event: &Event) -> Option<SceneType> {
        self.scenes
            .get_mut(&self.active_scene)
            .unwrap()
            .handle_event(&mut self.app_state, event)
    }

    pub fn update(&mut self, dt: Duration) -> Option<SceneType> {
        self.scenes
            .get_mut(&self.active_scene)
            .unwrap()
            .update(&mut self.app_state, dt)
    }

    pub fn render_world(&mut self, canvas: &mut Canvas<Window>) -> Result<(), String> {
        self.scenes
            .get_mut(&self.active_scene)
            .unwrap()
            .render_world(&mut self.app_state, canvas)
    }

    pub fn render_ui(&mut self, ctx: &egui::Context) -> Option<SceneType> {
        self.scenes
            .get_mut(&self.active_scene)
            .unwrap()
            .render_ui(&mut self.app_state, ctx)
    }

    pub fn set_scene(&mut self, scene_type: SceneType) {
        if self.scenes.contains_key(&scene_type) {
            log::info!("Switching to scene: {:?}", scene_type);
        } else {
            log::error!("Attempted to switch to unknown scene: {:?}", scene_type);
        }
        self.active_scene = scene_type;
    }
}
