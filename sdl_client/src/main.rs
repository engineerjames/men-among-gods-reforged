use std::time::Duration;

use egui_sdl2::egui::{self, Pos2};
use sdl2::event::Event;
use sdl2::image::InitFlag;
use sdl2::keyboard::Keycode;
use sdl2::video::Window;

fn hidpi_scale(window: &Window) -> (f32, f32) {
    let (window_w, window_h) = window.size();
    let (drawable_w, drawable_h) = window.drawable_size();
    let scale_x = if window_w > 0 {
        drawable_w as f32 / window_w as f32
    } else {
        1.0
    };
    let scale_y = if window_h > 0 {
        drawable_h as f32 / window_h as f32
    } else {
        1.0
    };
    (scale_x, scale_y)
}

fn scale_coord(value: i32, scale: f32) -> i32 {
    ((value as f32) * scale).round() as i32
}

fn adjust_mouse_event_for_hidpi(event: Event, window: &Window) -> Event {
    let (scale_x, scale_y) = hidpi_scale(window);
    if (scale_x - 1.0).abs() < f32::EPSILON && (scale_y - 1.0).abs() < f32::EPSILON {
        return event;
    }

    match event {
        Event::MouseMotion {
            timestamp,
            window_id,
            which,
            mousestate,
            x,
            y,
            xrel,
            yrel,
        } => Event::MouseMotion {
            timestamp,
            window_id,
            which,
            mousestate,
            x: scale_coord(x, scale_x),
            y: scale_coord(y, scale_y),
            xrel: scale_coord(xrel, scale_x),
            yrel: scale_coord(yrel, scale_y),
        },
        Event::MouseButtonDown {
            timestamp,
            window_id,
            which,
            mouse_btn,
            clicks,
            x,
            y,
        } => Event::MouseButtonDown {
            timestamp,
            window_id,
            which,
            mouse_btn,
            clicks,
            x: scale_coord(x, scale_x),
            y: scale_coord(y, scale_y),
        },
        Event::MouseButtonUp {
            timestamp,
            window_id,
            which,
            mouse_btn,
            clicks,
            x,
            y,
        } => Event::MouseButtonUp {
            timestamp,
            window_id,
            which,
            mouse_btn,
            clicks,
            x: scale_coord(x, scale_x),
            y: scale_coord(y, scale_y),
        },
        other => other,
    }
}

fn main() -> Result<(), String> {
    let sdl_context = sdl2::init()?;
    let _image_context = sdl2::image::init(InitFlag::PNG)?;
    let video = sdl_context.video()?;

    let window = video
        .window("Rust SDL2 Starter", 800, 600)
        .position_centered()
        .allow_highdpi()
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
            let event = adjust_mouse_event_for_hidpi(event, egui.painter.canvas.window());
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
