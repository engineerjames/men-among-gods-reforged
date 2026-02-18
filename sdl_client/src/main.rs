use std::process;
use std::time::{Duration, Instant};

use egui_sdl2::egui;
use sdl2::image::InitFlag;
use sdl2::mixer::{AUDIO_S16LSB, DEFAULT_CHANNELS};

use crate::gfx_cache::GraphicsCache;
use crate::scenes::scene::SceneType;
use crate::sfx_cache::SoundCache;

mod dpi_scaling;
mod filepaths;
mod gfx_cache;
mod scenes;
mod sfx_cache;

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
    let _audio_subsystem = sdl_context.audio()?;

    let frequency = 44_100;
    let format = AUDIO_S16LSB;
    let channels = DEFAULT_CHANNELS; // Stereo
    let chunk_size = 1_024;
    sdl2::mixer::open_audio(frequency, format, channels, chunk_size)?;

    // Initialize the mixer with desired frequency, format, channels, and chunk size
    sdl2::mixer::init(sdl2::mixer::InitFlag::MP3)?;

    log::info!("Creating window and event pump...");
    let video = sdl_context.video()?;
    let mut window = video
        .window("Men Among Gods - Reforged v1.3.0", 800, 600)
        .position_centered()
        .allow_highdpi()
        .resizable()
        .build()
        .map_err(|e| e.to_string())?;

    let _ = window.set_minimum_size(800, 600);

    let mut event_pump = sdl_context.event_pump()?;

    log::info!("Initializing canvas...");
    let mut egui = egui_sdl2::EguiCanvas::new(window);

    log::info!("Initializing graphics and sound caches...");
    let gfx_cache = GraphicsCache::new(
        filepaths::get_gfx_zipfile(),
        egui.painter.canvas.texture_creator(),
    );
    let _sfx_cache = SoundCache::new(
        filepaths::get_sfx_directory(),
        filepaths::get_music_directory(),
    );

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
