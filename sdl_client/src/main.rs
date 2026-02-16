use std::time::{Duration, Instant};

use egui_sdl2::egui::{self, Pos2};
use sdl2::event::Event;
use sdl2::image::InitFlag;
use sdl2::keyboard::Keycode;
use sdl2::pixels::Color;
use sdl2::rect::Rect;
use sdl2::render::Canvas;
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

enum SceneSwitch {
    ToLogin,
    ToGame,
}

trait Scene {
    fn handle_event(&mut self, event: &Event) -> Option<SceneSwitch>;
    fn update(&mut self, dt: Duration) -> Option<SceneSwitch>;
    fn render_world(&mut self, canvas: &mut Canvas<Window>) -> Result<(), String>;
    fn render_ui(&mut self, ctx: &egui::Context) -> Option<SceneSwitch>;
}

struct LoginScene {
    server_ip: String,
    username: String,
    password: String,
}

impl LoginScene {
    fn new() -> Self {
        Self {
            server_ip: "127.0.0.1".to_owned(),
            username: String::new(),
            password: String::new(),
        }
    }
}

impl Scene for LoginScene {
    fn handle_event(&mut self, _event: &Event) -> Option<SceneSwitch> {
        None
    }

    fn update(&mut self, _dt: Duration) -> Option<SceneSwitch> {
        None
    }

    fn render_world(&mut self, canvas: &mut Canvas<Window>) -> Result<(), String> {
        canvas.set_draw_color(Color::RGB(20, 20, 28));
        canvas.clear();
        Ok(())
    }

    fn render_ui(&mut self, ctx: &egui::Context) -> Option<SceneSwitch> {
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
                    next = Some(SceneSwitch::ToGame);
                }

                if create_clicked {
                    println!("Create new account clicked");
                }
            });

        next
    }
}

struct GameScene {
    x: f32,
    y: f32,
    velocity_px_per_sec: f32,
}

impl GameScene {
    fn new() -> Self {
        Self {
            x: 40.0,
            y: 260.0,
            velocity_px_per_sec: 220.0,
        }
    }
}

impl Scene for GameScene {
    fn handle_event(&mut self, event: &Event) -> Option<SceneSwitch> {
        if let Event::KeyDown {
            keycode: Some(Keycode::Backspace),
            ..
        } = event
        {
            return Some(SceneSwitch::ToLogin);
        }
        None
    }

    fn update(&mut self, dt: Duration) -> Option<SceneSwitch> {
        self.x += self.velocity_px_per_sec * dt.as_secs_f32();
        if self.x > 760.0 {
            self.x = -48.0;
        }
        None
    }

    fn render_world(&mut self, canvas: &mut Canvas<Window>) -> Result<(), String> {
        canvas.set_draw_color(Color::RGB(14, 22, 34));
        canvas.clear();

        canvas.set_draw_color(Color::RGB(95, 160, 255));
        let player = Rect::new(self.x.round() as i32, self.y.round() as i32, 48, 48);
        canvas.fill_rect(player)?;

        Ok(())
    }

    fn render_ui(&mut self, ctx: &egui::Context) -> Option<SceneSwitch> {
        egui::TopBottomPanel::top("hud").show(ctx, |ui| {
            ui.label("Game Scene (SDL world + egui overlay)");
            ui.label("Press Backspace to return to LoginScene");
        });
        None
    }
}

fn make_scene(switch: SceneSwitch) -> Box<dyn Scene> {
    match switch {
        SceneSwitch::ToLogin => Box::new(LoginScene::new()),
        SceneSwitch::ToGame => Box::new(GameScene::new()),
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
    let mut scene: Box<dyn Scene> = Box::new(LoginScene::new());
    let mut last_frame = Instant::now();

    'running: loop {
        let now = Instant::now();
        let dt = now.duration_since(last_frame);
        last_frame = now;

        let mut next_scene = None;

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
            if next_scene.is_none() {
                next_scene = scene.handle_event(&event);
            }
        }

        if next_scene.is_none() {
            next_scene = scene.update(dt);
        }

        scene.render_world(&mut egui.painter.canvas)?;

        egui.run(|ctx: &egui::Context| {
            if next_scene.is_none() {
                next_scene = scene.render_ui(ctx);
            }
        });

        egui.paint();
        egui.present();

        if let Some(change) = next_scene {
            scene = make_scene(change);
        }

        std::thread::sleep(Duration::from_millis(16));
    }

    Ok(())
}
