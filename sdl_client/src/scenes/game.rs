use std::time::Duration;

use egui_sdl2::egui;
use sdl2::{
    event::Event, keyboard::Keycode, pixels::Color, rect::Rect, render::Canvas, video::Window,
};

use crate::{
    scenes::scene::{Scene, SceneType},
    state::AppState,
};

pub struct GameScene {
    x: f32,
    y: f32,
    velocity_px_per_sec: f32,
}

impl GameScene {
    pub fn new() -> Self {
        Self {
            x: 40.0,
            y: 260.0,
            velocity_px_per_sec: 220.0,
        }
    }
}

impl Scene for GameScene {
    fn handle_event(&mut self, _app_state: &mut AppState, event: &Event) -> Option<SceneType> {
        if let Event::KeyDown {
            keycode: Some(Keycode::Backspace),
            ..
        } = event
        {
            return Some(SceneType::Login);
        }
        None
    }

    fn update(&mut self, _app_state: &mut AppState, dt: Duration) -> Option<SceneType> {
        self.x += self.velocity_px_per_sec * dt.as_secs_f32();
        if self.x > 760.0 {
            self.x = -48.0;
        }
        None
    }

    fn render_world(
        &mut self,
        app_state: &mut AppState,
        canvas: &mut Canvas<Window>,
    ) -> Result<(), String> {
        canvas.set_draw_color(Color::RGB(14, 22, 34));
        canvas.clear();

        canvas.set_draw_color(Color::RGB(95, 160, 255));
        let player = Rect::new(self.x.round() as i32, self.y.round() as i32, 48, 48);
        canvas.fill_rect(player)?;

        canvas.copy(app_state.gfx_cache.get_texture(1), None, None)?;

        Ok(())
    }

    fn render_ui(&mut self, _app_state: &mut AppState, ctx: &egui::Context) -> Option<SceneType> {
        egui::TopBottomPanel::top("hud").show(ctx, |ui| {
            ui.label("Game Scene (SDL world + egui overlay)");
            ui.label("Press Backspace to return to LoginScene");
        });
        None
    }
}
