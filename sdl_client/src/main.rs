use std::path::PathBuf;
use std::process;
use std::time::{Duration, Instant};

use egui_sdl2::egui::{self, Pos2};
use sdl2::event::Event;
use sdl2::image::InitFlag;
use sdl2::keyboard::Keycode;
use sdl2::pixels::Color;
use sdl2::rect::Rect;
use sdl2::render::Canvas;
use sdl2::video::Window;

use crate::gfx_cache::GraphicsCache;
use crate::sfx_cache::SoundCache;

mod gfx_cache;
mod sfx_cache;

/// Attempts to determine the base directory for Men Among Gods data files.
/// This is where we place the settings.json file, and logs.
pub fn get_mag_base_dir() -> Option<PathBuf> {
    let suffix = PathBuf::from(".men-among-gods");

    let debug_or_release = if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    };

    let is_windows = cfg!(target_os = "windows");

    // First, check if we are running in a development environment
    // This should give us a directory in target/{debug|release}
    let cargo_directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    if cargo_directory.exists() {
        return Some(
            cargo_directory
                .join("..")
                .join("target")
                .join(debug_or_release),
        );
    }

    // Next, check standard user directories for Unix/Mac OS/Linux
    if !is_windows {
        let environment_vars = ["HOME", "XDG_CONFIG_HOME", "XDG_DATA_HOME"];
        for var in environment_vars.iter() {
            if let Ok(home) = std::env::var(var) {
                return Some(PathBuf::from(home).join(suffix));
            }
        }
    } else {
        // Finally, check APPDATA on Windows
        if let Ok(appdata) = std::env::var("APPDATA") {
            return Some(PathBuf::from(appdata).join(suffix));
        }
    }

    None
}

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

enum Scenes {
    Login,
    Game,
}

trait Scene {
    fn handle_event(&mut self, event: &Event) -> Option<Scenes>;
    fn update(&mut self, dt: Duration) -> Option<Scenes>;
    fn render_world(&mut self, canvas: &mut Canvas<Window>) -> Result<(), String>;
    fn render_ui(&mut self, ctx: &egui::Context) -> Option<Scenes>;
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
    fn handle_event(&mut self, _event: &Event) -> Option<Scenes> {
        None
    }

    fn update(&mut self, _dt: Duration) -> Option<Scenes> {
        None
    }

    fn render_world(&mut self, canvas: &mut Canvas<Window>) -> Result<(), String> {
        canvas.set_draw_color(Color::RGB(20, 20, 28));
        canvas.clear();
        Ok(())
    }

    fn render_ui(&mut self, ctx: &egui::Context) -> Option<Scenes> {
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
                    next = Some(Scenes::Game);
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
    fn handle_event(&mut self, event: &Event) -> Option<Scenes> {
        if let Event::KeyDown {
            keycode: Some(Keycode::Backspace),
            ..
        } = event
        {
            return Some(Scenes::Login);
        }
        None
    }

    fn update(&mut self, dt: Duration) -> Option<Scenes> {
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

    fn render_ui(&mut self, ctx: &egui::Context) -> Option<Scenes> {
        egui::TopBottomPanel::top("hud").show(ctx, |ui| {
            ui.label("Game Scene (SDL world + egui overlay)");
            ui.label("Press Backspace to return to LoginScene");
        });
        None
    }
}

fn make_scene(switch: Scenes) -> Box<dyn Scene> {
    match switch {
        Scenes::Login => Box::new(LoginScene::new()),
        Scenes::Game => Box::new(GameScene::new()),
    }
}

fn main() -> Result<(), String> {
    mag_core::initialize_logger(log::LevelFilter::Info, Some("sdl_client.log")).unwrap_or_else(
        |e| {
            eprintln!("Failed to initialize logger: {}. Exiting.", e);
            process::exit(1);
        },
    );

    log::info!("Initializing SDL2 contexts...");
    let sdl_context = sdl2::init()?;
    let _image_context = sdl2::image::init(InitFlag::PNG)?;

    log::info!("Creating window and event pump...");
    let video = sdl_context.video()?;
    let window = video
        .window("Rust SDL2 Starter", 800, 600)
        .position_centered()
        .allow_highdpi()
        .resizable()
        .build()
        .map_err(|e| e.to_string())?;

    let mut event_pump = sdl_context.event_pump()?;

    log::info!("Initializing canvas...");
    let mut egui = egui_sdl2::EguiCanvas::new(window);
    let mut scene: Box<dyn Scene> = Box::new(LoginScene::new());
    let mut last_frame = Instant::now();

    log::info!("Initializing graphics and sound caches...");
    let _gfx_cache = GraphicsCache::new(egui.painter.canvas.texture_creator());
    let _sfx_cache = SoundCache::new();

    // Log info about the monitor, graphics card, etc.
    if let Ok(video_subsystem) = sdl_context.video() {
        for i in 0..video_subsystem.num_video_displays().unwrap_or(0) {
            if let Ok(display_mode) = video_subsystem.desktop_display_mode(i) {
                log::info!(
                    "Display mode: {}x{} @ {}Hz",
                    display_mode.w,
                    display_mode.h,
                    display_mode.refresh_rate
                );

                let dpi = video_subsystem.display_dpi(i).unwrap_or((0.0, 0.0, 0.0));
                log::info!(
                    "Display DPI: {:.2} (horizontal), {:.2} (vertical), {:.2} (diagonal)",
                    dpi.0,
                    dpi.1,
                    dpi.2
                );
            } else {
                log::warn!("Failed to get display mode information for display {}", i);
            }
        }

        log::info!(
            "Current video driver: {}",
            video_subsystem.current_video_driver()
        );
    } else {
        log::error!("Failed to get video subsystem");
        process::exit(1);
    }

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
