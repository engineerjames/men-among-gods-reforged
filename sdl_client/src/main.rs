use std::process;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use egui_sdl2::egui;
use sdl2::image::InitFlag;
use sdl2::mixer::{AUDIO_S16LSB, DEFAULT_CHANNELS};

use crate::gfx_cache::GraphicsCache;
use crate::scenes::scene::SceneType;
use crate::sfx_cache::SoundCache;
use crate::state::{ApiTokenState, AppState};

mod account_api;
mod dpi_scaling;
mod filepaths;
mod font_cache;
mod game_map;
mod gfx_cache;
mod helpers;
mod hosts;
mod network;
mod player_state;
mod scenes;
mod sfx_cache;
mod state;
mod types;

static EGUI_GLYPH_WARMED: AtomicBool = AtomicBool::new(false);

// This is really stupid, but it seems to prevent the odd text warping observed
fn warm_egui_glyph_cache(ctx: &egui::Context) {
    if EGUI_GLYPH_WARMED.swap(true, Ordering::Relaxed) {
        return;
    }

    let warmup_text = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789 \
!\"#$%&'()*+,-./:;<=>?@[\\]^_`{|}~";

    egui::Area::new("glyph_warmup_area".into())
        .fixed_pos(egui::Pos2::new(-10_000.0, -10_000.0))
        .show(ctx, |ui| {
            ui.label(warmup_text);
            ui.heading(warmup_text);
            ui.monospace(warmup_text);
        });
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
    let sfx_cache = SoundCache::new(
        filepaths::get_sfx_directory(),
        filepaths::get_music_directory(),
    );
    let api_state = ApiTokenState::new(hosts::get_api_base_url());
    let mut app_state = AppState::new(gfx_cache, sfx_cache, api_state);

    let mut scene_manager = scenes::scene::SceneManager::new();
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

        // Poll events once, handle quit and forward to egui
        for event in event_pump.poll_iter() {
            if let sdl2::event::Event::Quit { .. } = event {
                scene_manager.request_scene_change(SceneType::Exit, &mut app_state);
            }

            let event =
                dpi_scaling::adjust_mouse_event_for_hidpi(event, egui.painter.canvas.window());

            let _ = egui.on_event(&event);

            scene_manager.handle_event(&mut app_state, &event);

            if scene_manager.get_scene() == SceneType::Exit {
                break 'running;
            }
        }

        scene_manager.update(&mut app_state, dt);
        scene_manager.render_world(&mut app_state, &mut egui.painter.canvas);

        egui.run(|ctx: &egui::Context| {
            warm_egui_glyph_cache(ctx);
            scene_manager.render_ui(&mut app_state, ctx);
        });

        if scene_manager.get_scene() == SceneType::Exit {
            break 'running;
        }

        egui.paint();
        egui.present();

        std::thread::sleep(Duration::from_millis(16));
    }

    Ok(())
}
