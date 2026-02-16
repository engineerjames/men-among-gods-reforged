use std::time::Duration;

use egui_sdl2::egui::{self, Pos2};
use sdl2::event::Event;
use sdl2::image::InitFlag;
use sdl2::keyboard::Keycode;

fn main() -> Result<(), String> {
    let sdl_context = sdl2::init()?;
    let _image_context = sdl2::image::init(InitFlag::PNG)?;
    let video = sdl_context.video()?;

    let window = video
        .window("Rust SDL2 Starter", 800, 600)
        .position_centered()
        .resizable()
        .build()
        .map_err(|e| e.to_string())?;

    let mut event_pump = sdl_context.event_pump()?;

    let mut egui = egui_sdl2::EguiCanvas::new(window);

    'running: loop {
        // Poll events once, handle quit and forward to egui
        for event in event_pump.poll_iter() {
            if let Event::Quit { .. }
            | Event::KeyDown {
                keycode: Some(Keycode::Escape),
                ..
            } = event
            {
                break 'running;
            }
            egui.on_event(&event);
        }
        // Clear the egui-backed canvas
        egui.clear([20, 20, 28, 255]);
        // Call `run` + `paint` each frame:
        egui.run(|ctx: &egui::Context| {
            egui::Window::new("Account Login")
                .default_height(800.0)
                .default_width(600.0)
                .fixed_pos(Pos2::new(0.0, 0.0))
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.heading("Men Among Gods Reforged");
                    ui.add_space(10.0);

                    ui.label("Game server IP address");
                    let ip_resp = ui.add_enabled(
                        true, // TODO: FIX THIS
                        egui::TextEdit::singleline(&mut "127.0.0.1").desired_width(260.0),
                    );

                    ui.add_space(10.0);

                    ui.label("Username");
                    ui.add_enabled(
                        true, // TODO: FIX THIS
                        egui::TextEdit::singleline(&mut "").desired_width(260.0),
                    );

                    ui.add_space(8.0);
                    ui.label("Password");
                    ui.add_enabled(
                        true, // TODO: FIX THIS
                        egui::TextEdit::singleline(&mut "")
                            .password(true)
                            .desired_width(260.0),
                    );

                    ui.add_space(12.0);

                    let (login_clicked, create_clicked) = ui
                        .horizontal(|ui| {
                            let login_clicked = ui
                                .add_enabled(
                                    true, // TODO: FIX THIS
                                    egui::Button::new("Login").min_size([180.0, 32.0].into()),
                                )
                                .clicked();

                            let create_clicked = ui
                                .add_enabled(
                                    true, // TODO: FIX THIS
                                    egui::Button::new("Create new account")
                                        .min_size([180.0, 32.0].into()),
                                )
                                .clicked();

                            (login_clicked, create_clicked)
                        })
                        .inner;

                    if login_clicked {
                        println!("Login clicked");
                    }

                    if create_clicked {
                        println!("Create new account clicked");
                    }
                });
        });

        egui.paint();
        egui.present();

        std::thread::sleep(Duration::from_millis(16));
    }

    Ok(())
}
