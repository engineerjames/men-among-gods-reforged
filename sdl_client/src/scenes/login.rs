use std::time::Duration;

use crate::scenes::scene::{Scene, SceneType};
use egui_sdl2::egui::{self, Pos2};
use sdl2::{event::Event, pixels::Color, render::Canvas, video::Window};

pub struct LoginScene {
    server_ip: String,
    username: String,
    password: String,
}

impl LoginScene {
    pub fn new() -> Self {
        Self {
            server_ip: "127.0.0.1".to_owned(),
            username: String::new(),
            password: String::new(),
        }
    }
}

impl Scene for LoginScene {
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

        egui::Window::new("Account Login")
            .default_height(430.0)
            .default_width(430.0)
            .fixed_pos(Pos2::new(20.0, 20.0))
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.heading("Men Among Gods Reforged");
                ui.add_space(10.0);

                ui.label("Game server IP address");
                ui.add(egui::TextEdit::singleline(&mut self.server_ip).desired_width(260.0));
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

                let (login_clicked, create_clicked) = ui
                    .horizontal(|ui| {
                        let login_clicked = ui
                            .add(egui::Button::new("Login").min_size([180.0, 32.0].into()))
                            .clicked();

                        let create_clicked = ui
                            .add(
                                egui::Button::new("Create new account")
                                    .min_size([180.0, 32.0].into()),
                            )
                            .clicked();

                        (login_clicked, create_clicked)
                    })
                    .inner;

                if login_clicked {
                    println!(
                        "Login clicked: ip={}, username={}",
                        self.server_ip, self.username
                    );
                    next = Some(SceneType::Game);
                }

                if create_clicked {
                    println!("Create new account clicked");
                }
            });

        next
    }
}
