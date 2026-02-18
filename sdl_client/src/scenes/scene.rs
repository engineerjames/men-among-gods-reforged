use std::{collections::HashMap, time::Duration};

use egui_sdl2::egui;
use sdl2::{event::Event, render::Canvas, video::Window};

use crate::gfx_cache::GraphicsCache;

pub trait Scene {
    fn handle_event(&mut self, event: &Event) -> Option<SceneType>;
    fn update(&mut self, dt: Duration) -> Option<SceneType>;
    fn render_world(&mut self, canvas: &mut Canvas<Window>) -> Result<(), String>;
    fn render_ui(&mut self, ctx: &egui::Context) -> Option<SceneType>;
}

#[derive(Hash, Eq, PartialEq, Debug, Copy, Clone)]
pub enum SceneType {
    Login,
    Game,
    NewAccount,
    Exit,
}

pub struct SceneManager {
    active_scene: SceneType,
    scenes: HashMap<SceneType, Box<dyn Scene>>,
}

impl SceneManager {
    pub fn new(graphics_cache: GraphicsCache) -> Self {
        let mut scene_map: HashMap<SceneType, Box<dyn Scene>> = HashMap::new();

        scene_map.insert(
            SceneType::Login,
            Box::new(crate::scenes::login::LoginScene::new()),
        );

        scene_map.insert(
            SceneType::Game,
            Box::new(crate::scenes::game::GameScene::new(graphics_cache)),
        );

        scene_map.insert(
            SceneType::NewAccount,
            Box::new(crate::scenes::new_account::NewAccountScene::new()),
        );

        SceneManager {
            active_scene: SceneType::Login,
            scenes: scene_map,
        }
    }

    pub fn active_scene(&mut self) -> &mut Box<dyn Scene> {
        self.scenes.get_mut(&self.active_scene).unwrap()
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
