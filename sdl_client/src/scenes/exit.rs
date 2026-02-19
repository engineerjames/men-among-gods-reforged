use std::time::Duration;

use egui_sdl2::egui;
use sdl2::{event::Event, render::Canvas, video::Window};

use crate::{
    scenes::scene::{Scene, SceneType},
    state::AppState,
};

pub struct ExitScene {
    // No state needed for the exit scene, but we can add fields here if we want to display any information or perform any cleanup actions.
}

impl ExitScene {
    pub fn new() -> Self {
        Self {}
    }
}

impl Scene for ExitScene {
    fn handle_event(&mut self, _app_state: &mut AppState, _event: &Event) -> Option<SceneType> {
        None
    }

    fn update(&mut self, _app_state: &mut AppState, _dt: Duration) -> Option<SceneType> {
        None
    }

    fn render_world(
        &mut self,
        _app_state: &mut AppState,
        _canvas: &mut Canvas<Window>,
    ) -> Result<(), String> {
        Ok(())
    }

    fn render_ui(&mut self, _app_state: &mut AppState, _ctx: &egui::Context) -> Option<SceneType> {
        None
    }
}
