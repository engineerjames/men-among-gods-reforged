use std::process;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use egui_sdl2::egui;
use sdl2::gfx::framerate::FPSManager;
use sdl2::image::InitFlag;
use sdl2::mixer::{AUDIO_S16LSB, DEFAULT_CHANNELS};
use sdl2::video::FullscreenType;

use crate::gfx_cache::GraphicsCache;
use crate::preferences::DisplayMode;
use crate::scenes::scene::SceneType;
use crate::sfx_cache::SoundCache;
use crate::state::{ApiTokenState, AppState, DisplayCommand};

mod account_api;
mod cert_trust;
mod dpi_scaling;
mod filepaths;
mod font_cache;
mod game_map;
mod gfx_cache;
mod hosts;
mod legacy_engine;
mod network;
mod platform;
mod player_state;
mod preferences;
mod scenes;
mod sfx_cache;
mod state;
mod types;

/// Global flag ensuring the egui glyph warm-up runs exactly once.
static EGUI_GLYPH_WARMED: AtomicBool = AtomicBool::new(false);

/// Pre-renders glyphs and common text styles into egui's texture atlas.
///
/// This reduces the chance of mid-session atlas growth, which can trigger
/// text corruption with some SDL2-based egui backends when texture sizes
/// change at runtime.
///
/// # Arguments
/// * `ctx` – the egui context whose font atlas will be primed.
fn warm_egui_glyph_cache(ctx: &egui::Context) {
    if EGUI_GLYPH_WARMED.swap(true, Ordering::Relaxed) {
        return;
    }

    let ascii_glyphs = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789 \
!\"#$%&'()*+,-./:;<=>?@[\\]^_`{|}~";
    let cert_dialog_text = "Server Certificate Changed Host: 127.0.0.1 \
Previously trusted fingerprint New fingerprint presented by server \
Accept New Certificate Reject";
    let fingerprint_sample = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    let status_text = "UNENCRYPTED - Game traffic is not protected Profiling... 60s remaining";
    let combined =
        format!("{ascii_glyphs}\n{cert_dialog_text}\n{fingerprint_sample}\n{status_text}");

    egui::Area::new("glyph_warmup_area".into())
        .fixed_pos(egui::Pos2::new(-10_000.0, -10_000.0))
        .show(ctx, |ui| {
            ui.label(&combined);
            ui.heading(&combined);
            ui.monospace(&combined);
            ui.add(egui::Label::new(
                egui::RichText::new(&combined).text_style(egui::TextStyle::Button),
            ));
            ui.add(egui::Label::new(
                egui::RichText::new(&combined).text_style(egui::TextStyle::Small),
            ));
        });
}

/// Application entry point.
///
/// Initialises logging, SDL2 subsystems (video, audio, mixer), creates the
/// window and canvas, builds the scene manager, and enters the main loop.
/// The loop polls events, updates the active scene, renders world + UI layers,
/// and caps at 60 FPS via `FPSManager`.
fn main() -> Result<(), String> {
    // Build the log-file path relative to the executable so that the logger
    // resolves correctly inside a macOS .app bundle (where the OS sets CWD to
    // "/" rather than the MacOS/ directory).
    let log_path = preferences::log_file_path();
    let log_path_str = log_path.to_string_lossy();
    mag_core::initialize_logger(log::LevelFilter::Info, Some(log_path_str.as_ref()))
        .unwrap_or_else(|e| {
            eprintln!("Failed to initialize logger: {}. Exiting.", e);
            process::exit(1);
        });

    log::info!("Initializing SDL2 contexts...");
    let mut fps_manager = FPSManager::new();
    fps_manager.set_framerate(60)?;
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

    // Set a fixed 800x600 logical render size so that all SDL canvas.copy() /
    // fill_rect() calls in every scene use logical pixel coordinates regardless
    // of the physical drawable size (e.g. Retina 2x displays).
    // egui's painter pre-multiplies vertices by pixels_per_point itself, so we
    // temporarily disable the logical size before egui.paint() each frame.
    const LOGICAL_W: u32 = 800;
    const LOGICAL_H: u32 = 600;

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

    // --- Apply persisted display settings ---------------------------------
    let global_settings = preferences::load_global_settings();
    app_state.music_enabled = global_settings.music_enabled;
    app_state.display_mode = global_settings.display_mode;
    app_state.pixel_perfect_scaling = global_settings.pixel_perfect_scaling;
    app_state.vsync_enabled = global_settings.vsync_enabled;

    // Display mode
    let applied_startup_mode = apply_display_mode(&mut egui.painter.canvas, app_state.display_mode);
    app_state.display_mode = applied_startup_mode;
    if applied_startup_mode != global_settings.display_mode {
        save_global_display_settings(&app_state);
    }

    // VSync (runtime toggle via raw SDL2 FFI)
    apply_vsync(&egui.painter.canvas, app_state.vsync_enabled);
    // ----------------------------------------------------------------------

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
            let egui_event = dpi_scaling::adjust_mouse_event_for_egui_hidpi(
                &event,
                egui.painter.canvas.window(),
            );
            let _ = egui.on_event(&egui_event);

            let event = dpi_scaling::adjust_mouse_event_for_hidpi(
                event,
                egui.painter.canvas.window(),
                app_state.pixel_perfect_scaling,
            );

            scene_manager.handle_event(&mut app_state, &event);

            if scene_manager.get_scene() == SceneType::Exit {
                break 'running;
            }
        }

        scene_manager.update(&mut app_state, dt);

        // --- Apply any pending display commands from the UI ---------------
        if let Some(cmd) = app_state.display_command.take() {
            match cmd {
                DisplayCommand::SetDisplayMode(mode) => {
                    let applied_mode = apply_display_mode(&mut egui.painter.canvas, mode);
                    if applied_mode != mode {
                        log::warn!(
                            "Requested display mode {} adjusted to {}",
                            mode,
                            applied_mode
                        );
                    }
                    app_state.display_mode = applied_mode;
                    save_global_display_settings(&app_state);
                }
                DisplayCommand::SetPixelPerfectScaling(enabled) => {
                    app_state.pixel_perfect_scaling = enabled;
                    save_global_display_settings(&app_state);
                }
                DisplayCommand::SetVSync(enabled) => {
                    apply_vsync(&egui.painter.canvas, enabled);
                    app_state.vsync_enabled = enabled;
                    save_global_display_settings(&app_state);
                }
            }
        }
        // ------------------------------------------------------------------

        // Logical size on  → scene SDL drawing uses 800×600 coords.
        let _ = egui.painter.canvas.set_logical_size(LOGICAL_W, LOGICAL_H);
        // Integer scale → pixel-perfect (nearest integer multiplier) when on.
        let _ = egui
            .painter
            .canvas
            .set_integer_scale(app_state.pixel_perfect_scaling);
        scene_manager.render_world(&mut app_state, &mut egui.painter.canvas);
        // Logical size off → egui painter uses raw physical pixels.
        let _ = egui.painter.canvas.set_integer_scale(false);
        let _ = egui.painter.canvas.set_logical_size(0, 0);

        egui.run(|ctx: &egui::Context| {
            warm_egui_glyph_cache(ctx);
            scene_manager.render_ui(&mut app_state, ctx);
        });

        if scene_manager.get_scene() == SceneType::Exit {
            break 'running;
        }

        egui.paint();
        egui.present();

        fps_manager.delay();
    }

    Ok(())
}

/// Maps [`DisplayMode`] to the SDL2 fullscreen type and applies it.
fn apply_display_mode(
    canvas: &mut sdl2::render::Canvas<sdl2::video::Window>,
    mode: DisplayMode,
) -> DisplayMode {
    let mut applied_mode = mode;
    let ft = match mode {
        DisplayMode::Windowed => FullscreenType::Off,
        DisplayMode::Fullscreen => {
            #[cfg(target_os = "macos")]
            {
                log::warn!(
                    "Exclusive fullscreen is unstable on macOS; using borderless fullscreen instead"
                );
                applied_mode = DisplayMode::BorderlessFullscreen;
                FullscreenType::Desktop
            }

            #[cfg(not(target_os = "macos"))]
            {
                FullscreenType::True
            }
        }
        DisplayMode::BorderlessFullscreen => FullscreenType::Desktop,
    };

    if let Err(e) = canvas.window_mut().set_fullscreen(ft) {
        log::error!("Failed to set fullscreen mode to {mode}: {e}");
        if mode != DisplayMode::Windowed {
            if let Err(fallback_err) = canvas.window_mut().set_fullscreen(FullscreenType::Off) {
                log::error!(
                    "Failed to restore windowed mode after fullscreen failure: {fallback_err}"
                );
            }
            applied_mode = DisplayMode::Windowed;
        }
    }

    applied_mode
}

/// Toggles VSync on the renderer at runtime via raw SDL2 FFI.
fn apply_vsync(canvas: &sdl2::render::Canvas<sdl2::video::Window>, enabled: bool) {
    let raw = canvas.raw();
    let flag: std::os::raw::c_int = if enabled { 1 } else { 0 };
    let result = unsafe { sdl2::sys::SDL_RenderSetVSync(raw, flag) };
    if result != 0 {
        log::error!("SDL_RenderSetVSync failed: {}", sdl2::get_error());
    }
}

/// Persists current display-related settings from [`AppState`] into the
/// global profile.
fn save_global_display_settings(app_state: &AppState) {
    let settings = preferences::GlobalSettings {
        music_enabled: app_state.music_enabled,
        display_mode: app_state.display_mode,
        pixel_perfect_scaling: app_state.pixel_perfect_scaling,
        vsync_enabled: app_state.vsync_enabled,
    };
    if let Err(e) = preferences::save_global_settings(&settings) {
        log::error!("Failed to persist display settings: {e}");
    }
}
