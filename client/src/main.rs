use std::process;
use std::time::Instant;

use sdl2::gfx::framerate::FPSManager;
use sdl2::image::InitFlag;
use sdl2::mixer::{AUDIO_S16LSB, DEFAULT_CHANNELS};
use sdl2::video::FullscreenType;

use client::gfx_cache::GraphicsCache;
use client::preferences::DisplayMode;
use client::scenes::scene::SceneType;
use client::sfx_cache::SoundCache;
use client::state::{ApiTokenState, AppState, DisplayCommand};
use client::ui::visuals::panning_background::PanningBackground;
use client::ui::widget::Bounds;
use client::{constants, dpi_scaling, filepaths, hosts, preferences, scenes};

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

    // --- Game controller subsystem ----------------------------------------
    let game_controller_subsystem = sdl_context.game_controller().map_err(|e| {
        log::warn!("Failed to initialize game controller subsystem: {e}");
        e
    });
    let mut _open_controllers: Vec<sdl2::controller::GameController> = Vec::new();
    if let Ok(ref gc_subsystem) = game_controller_subsystem {
        let num_joysticks = gc_subsystem.num_joysticks().map_err(|e| e.to_string())?;
        log::info!("Detected {num_joysticks} joystick(s) at startup");
        for i in 0..num_joysticks {
            if gc_subsystem.is_game_controller(i) {
                match gc_subsystem.open(i) {
                    Ok(controller) => {
                        log::info!("Opened game controller {i}: \"{}\"", controller.name());
                        _open_controllers.push(controller);
                    }
                    Err(e) => {
                        log::warn!("Failed to open game controller {i}: {e}");
                    }
                }
            }
        }
    }
    // ----------------------------------------------------------------------

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
        .window(
            "Men Among Gods - Reforged v1.3.0",
            constants::TARGET_WIDTH_INT,
            constants::TARGET_HEIGHT_INT,
        )
        .position_centered()
        .allow_highdpi()
        .resizable()
        .build()
        .map_err(|e| e.to_string())?;

    let _ = window.set_minimum_size(constants::TARGET_WIDTH_INT, constants::TARGET_HEIGHT_INT);

    let mut canvas = window.into_canvas().build().map_err(|e| e.to_string())?;

    let mut event_pump = sdl_context.event_pump()?;

    log::info!("Initializing graphics and sound caches...");
    let texture_creator = canvas.texture_creator();
    let gfx_cache = GraphicsCache::new(filepaths::get_gfx_zipfile(), &texture_creator);
    let sfx_cache = SoundCache::new(
        filepaths::get_sfx_directory(),
        filepaths::get_music_directory(),
    );
    let api_state = ApiTokenState::new(hosts::get_api_base_url());

    let asset_gfx = filepaths::get_asset_directory().join("gfx");
    let bg_paths = vec![
        asset_gfx.join("login_pents.png"),
        asset_gfx.join("login_black_stronghold.png"),
        asset_gfx.join("login_last_gate.png"),
        asset_gfx.join("login_skua_temple.png"),
        asset_gfx.join("login_tower.png"),
    ];
    let panning_background = PanningBackground::new(
        Bounds::new(
            0,
            0,
            constants::TARGET_WIDTH_INT,
            constants::TARGET_HEIGHT_INT,
        ),
        bg_paths,
        6.0,
        2.0,
        Some(sdl2::pixels::Color::RGBA(10, 10, 30, 100)),
    );

    let mut app_state = AppState::new(gfx_cache, sfx_cache, api_state, panning_background);

    // --- Apply persisted display settings ---------------------------------
    app_state.settings = preferences::load_global_settings();

    // Display mode
    let requested_mode = app_state.settings.display_mode;
    let applied_startup_mode = apply_display_mode(&mut canvas, requested_mode);
    app_state.settings.display_mode = applied_startup_mode;
    if applied_startup_mode != requested_mode {
        save_global_display_settings(&app_state);
    }

    // VSync (runtime toggle via raw SDL2 FFI)
    apply_vsync(&canvas, app_state.settings.vsync_enabled);
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

        // Poll events
        for event in event_pump.poll_iter() {
            if let sdl2::event::Event::Quit { .. } = event {
                scene_manager.request_scene_change(SceneType::Exit, &mut app_state);
            }

            // --- Controller input mode detection --------------------------
            // Any gamepad input switches to controller mode; any
            // keyboard/mouse input switches back.
            match &event {
                sdl2::event::Event::ControllerButtonDown { .. } => {
                    if !app_state.controller_active {
                        log::info!("Controller input detected — switching to controller mode");
                        app_state.controller_active = true;
                    }
                }
                sdl2::event::Event::ControllerAxisMotion { value, .. } => {
                    // Ignore small axis values inside the deadzone.
                    const DEADZONE: i16 = 8000;
                    if value.abs() > DEADZONE && !app_state.controller_active {
                        log::info!("Controller input detected — switching to controller mode");
                        app_state.controller_active = true;
                    }
                }
                sdl2::event::Event::ControllerDeviceAdded { which, .. } => {
                    if let Ok(ref gc_subsystem) = game_controller_subsystem {
                        match gc_subsystem.open(*which) {
                            Ok(controller) => {
                                log::info!(
                                    "Game controller connected: \"{}\" (index {which})",
                                    controller.name()
                                );
                                _open_controllers.push(controller);
                            }
                            Err(e) => {
                                log::warn!(
                                    "Failed to open newly connected controller {which}: {e}"
                                );
                            }
                        }
                    }
                }
                sdl2::event::Event::ControllerDeviceRemoved { which, .. } => {
                    log::info!("Game controller disconnected (instance id {which})");
                    _open_controllers.retain(|c| c.instance_id() != *which);
                }
                sdl2::event::Event::KeyDown { .. }
                | sdl2::event::Event::KeyUp { .. }
                | sdl2::event::Event::MouseButtonDown { .. }
                | sdl2::event::Event::MouseButtonUp { .. }
                | sdl2::event::Event::MouseMotion { .. }
                | sdl2::event::Event::MouseWheel { .. }
                | sdl2::event::Event::TextInput { .. } => {
                    if app_state.controller_active {
                        log::info!("Keyboard/mouse input detected — leaving controller mode");
                        app_state.controller_active = false;
                    }
                }
                _ => {}
            }
            // --------------------------------------------------------------

            let event = dpi_scaling::adjust_mouse_event_for_hidpi(
                event,
                canvas.window(),
                constants::TARGET_WIDTH,
                constants::TARGET_HEIGHT,
                app_state.settings.pixel_perfect_scaling,
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
                    let applied_mode = apply_display_mode(&mut canvas, mode);
                    if applied_mode != mode {
                        log::warn!(
                            "Requested display mode {} adjusted to {}",
                            mode,
                            applied_mode
                        );
                    }
                    app_state.settings.display_mode = applied_mode;
                    save_global_display_settings(&app_state);
                }
                DisplayCommand::SetPixelPerfectScaling(enabled) => {
                    app_state.settings.pixel_perfect_scaling = enabled;
                    save_global_display_settings(&app_state);
                }
                DisplayCommand::SetVSync(enabled) => {
                    apply_vsync(&canvas, enabled);
                    app_state.settings.vsync_enabled = enabled;
                    save_global_display_settings(&app_state);
                }
            }
        }
        // ------------------------------------------------------------------
        let _ = canvas.set_logical_size(constants::TARGET_WIDTH_INT, constants::TARGET_HEIGHT_INT);
        // Integer scale --> pixel-perfect (nearest integer multiplier) when on.
        let _ = canvas.set_integer_scale(app_state.settings.pixel_perfect_scaling);
        scene_manager.render_world(&mut app_state, &mut canvas);
        // Logical size off --> raw physical pixels.
        let _ = canvas.set_integer_scale(false);
        let _ = canvas.set_logical_size(0, 0);

        if scene_manager.get_scene() == SceneType::Exit {
            break 'running;
        }

        canvas.present();

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
fn save_global_display_settings(app_state: &AppState<'_>) {
    if let Err(e) = preferences::save_global_settings(&app_state.settings) {
        log::error!("Failed to persist display settings: {e}");
    }
}
