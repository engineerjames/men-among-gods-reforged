use std::time::Duration;

use egui_sdl2::egui::{self, Align2, Vec2};
use sdl2::{event::Event, pixels::Color, render::Canvas, video::Window};

use crate::scenes::scene::{Scene, SceneType};

pub struct NewAccountScene {
    email: String,
    username: String,
    password: String,
}

impl NewAccountScene {
    pub fn new() -> Self {
        NewAccountScene {
            email: String::new(),
            username: String::new(),
            password: String::new(),
        }
    }
}

impl Scene for NewAccountScene {
    fn handle_event(&mut self, _event: &Event) -> Option<SceneType> {
        None
    }

    fn update(&mut self, _dt: Duration) -> Option<SceneType> {
        None
    }

    fn render_world(&mut self, canvas: &mut Canvas<Window>) -> Result<(), String> {
        canvas.set_draw_color(Color::RGB(20, 20, 28));
        canvas.clear();
        Ok(())
    }

    fn render_ui(&mut self, ctx: &egui::Context) -> Option<SceneType> {
        let mut next = None;

        egui::Window::new("Men Among Gods - Reforged")
            .default_height(430.0)
            .default_width(430.0)
            .anchor(Align2::CENTER_CENTER, Vec2::new(0.0, 0.0))
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                    ui.heading("Create Account");
                });
                ui.add_space(10.0);

                ui.label("E-mail");
                ui.add(egui::TextEdit::singleline(&mut self.email).desired_width(260.0));
                ui.add_space(10.0);

                ui.label("Username");
                ui.add(egui::TextEdit::singleline(&mut self.username).desired_width(260.0));
                ui.add_space(8.0);

                ui.label("Password");
                ui.add(
                    egui::TextEdit::singleline(&mut self.password)
                        .password(true)
                        .desired_width(260.0),
                );
                ui.add_space(12.0);

                let (create_clicked, cancel_clicked) = ui
                    .horizontal(|ui| {
                        let create_clicked = ui
                            .add(egui::Button::new("Create").min_size([180.0, 32.0].into()))
                            .clicked();

                        let cancel_clicked = ui
                            .add(egui::Button::new("Cancel").min_size([180.0, 32.0].into()))
                            .clicked();

                        (create_clicked, cancel_clicked)
                    })
                    .inner;

                if cancel_clicked {
                    log::info!("Cancel clicked");
                    next = Some(SceneType::Login);
                }

                if create_clicked {
                    log::info!(
                        "Create new account clicked with email={}, username={}",
                        self.email,
                        self.username
                    );
                }
            });

        next
    }
}
