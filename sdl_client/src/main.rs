use std::path::PathBuf;
use std::process;
use std::time::{Duration, Instant};

use egui_sdl2::egui;
use sdl2::image::InitFlag;

use crate::gfx_cache::GraphicsCache;
use crate::scenes::scene::SceneType;
use crate::sfx_cache::SoundCache;

mod dpi_scaling;
mod filepaths;
mod gfx_cache;
mod scenes;
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

    log::info!("Initializing graphics and sound caches...");
    let gfx_cache = GraphicsCache::new(
        filepaths::get_gfx_zipfile(),
        egui.painter.canvas.texture_creator(),
    );
    let _sfx_cache = SoundCache::new();
    let mut scene_manager = scenes::scene::SceneManager::new(gfx_cache);
    let mut last_frame = Instant::now();

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
            if let sdl2::event::Event::Quit { .. } = event {
                next_scene = Some(SceneType::Exit);
            }

            let event =
                dpi_scaling::adjust_mouse_event_for_hidpi(event, egui.painter.canvas.window());

            let _ = egui.on_event(&event);

            if next_scene.is_none() {
                next_scene = scene_manager.active_scene().handle_event(&event);
            }

            // After each event, check if we need to switch scenes (e.g. quit event)
            if next_scene.is_some_and(|s| s == SceneType::Exit) {
                break 'running;
            }
        }

        if next_scene.is_none() {
            next_scene = scene_manager.active_scene().update(dt);
        }

        scene_manager
            .active_scene()
            .render_world(&mut egui.painter.canvas)?;

        egui.run(|ctx: &egui::Context| {
            if next_scene.is_none() {
                next_scene = scene_manager.active_scene().render_ui(ctx);
            }
        });

        egui.paint();
        egui.present();

        if let Some(scene) = next_scene {
            log::info!("Scene change requested: {:?}", scene);
            scene_manager.set_scene(scene);
        }

        std::thread::sleep(Duration::from_millis(16));
    }

    Ok(())
}
