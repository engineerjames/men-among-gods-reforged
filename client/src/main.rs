use std::process;
use std::time::Instant;

use sdl2::gfx::framerate::FPSManager;
use sdl2::image::InitFlag;
use sdl2::mixer::{AUDIO_S16LSB, DEFAULT_CHANNELS};
use sdl2::pixels::{Color, PixelFormatEnum};
use sdl2::rect::Rect;
use sdl2::render::{BlendMode, Canvas, ScaleMode, Texture};
use sdl2::video::{FullscreenType, Window};

use client::gfx_cache::GraphicsCache;
use client::platform::PlatformProfile;
use client::preferences::{
    ColorGradeMode, DisplayMode, PixelScalerMode, ScanlineMode, Settings, SharpenMode, UpscaleMode,
};
use client::scenes::scene::{SceneManager, SceneType};
use client::sfx_cache::SoundCache;
use client::state::{ApiTokenState, AppState, DisplayCommand};
use client::ui::visuals::panning_background::PanningBackground;
use client::ui::widget::Bounds;
use client::{constants, dpi_scaling, filepaths, hosts, preferences};

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

    let platform = PlatformProfile::detect();
    let is_first_run = !preferences::profile_exists();

    log::info!("Initializing SDL2 contexts...");
    let mut fps_manager = FPSManager::new();
    fps_manager.set_framerate(60)?;
    let sdl_context = sdl2::init()?;
    let _image_context = sdl2::image::init(InitFlag::PNG)?;
    let _audio_subsystem = sdl_context
        .audio()
        .map_err(|e| {
            log::warn!("Failed to initialise audio subsystem (audio will be disabled): {e}");
            e
        })
        .ok();

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
    let audio_available = _audio_subsystem.is_some()
        && sdl2::mixer::open_audio(frequency, format, channels, chunk_size)
            .map_err(|e| log::warn!("Failed to open audio device (audio will be disabled): {e}"))
            .is_ok()
        && sdl2::mixer::init(sdl2::mixer::InitFlag::MP3)
            .map_err(|e| {
                log::warn!("Failed to initialise SDL2_mixer (audio will be disabled): {e}")
            })
            .is_ok();

    log::info!("Creating window and event pump...");
    let video = sdl_context.video()?;
    let mut canvas = match create_window_canvas(&video, true) {
        Ok(canvas) => canvas,
        Err(err) => {
            log::warn!(
                "Failed to create render-target canvas ({err}); falling back to direct rendering"
            );
            create_window_canvas(&video, false)?
        }
    };

    let mut event_pump = sdl_context.event_pump()?;

    log::info!("Initializing graphics and sound caches (audio_available={audio_available})...");
    let texture_creator = canvas.texture_creator();
    let mut scene_texture = match texture_creator.create_texture_target(
        Some(PixelFormatEnum::RGBA8888),
        constants::TARGET_WIDTH_INT,
        constants::TARGET_HEIGHT_INT,
    ) {
        Ok(texture) => Some(texture),
        Err(err) => {
            log::warn!(
                "Failed to create render-target scene texture ({err}); falling back to direct rendering"
            );
            None
        }
    };
    let gfx_cache = GraphicsCache::new(filepaths::get_gfx_zipfile(), &texture_creator);
    let sfx_cache = if audio_available {
        SoundCache::new(
            filepaths::get_sfx_directory(),
            filepaths::get_music_directory(),
        )
    } else {
        SoundCache::new_disabled()
    };
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
        0.9,
        Some(sdl2::pixels::Color::RGBA(10, 10, 30, 100)),
    );

    let mut app_state = AppState::new(
        gfx_cache,
        sfx_cache,
        api_state,
        panning_background,
        platform,
    );

    // Track the previous controller_active state so we can detect transitions
    // and toggle the system cursor accordingly.
    let mut prev_controller_active = false;

    // --- Apply persisted display settings ---------------------------------
    app_state.settings = preferences::load_global_settings();

    // On the very first run apply platform-specific defaults, then persist
    // them immediately so subsequent runs treat them as the user's baseline.
    if is_first_run {
        platform.apply_first_run_defaults(&mut app_state.settings);
        if let Err(e) = preferences::save_global_settings(&app_state.settings) {
            log::warn!("Failed to persist first-run platform defaults: {e}");
        }
    }

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

    let mut scene_manager = SceneManager::new();
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
                    // Use saturating_abs to avoid overflow on i16::MIN (-32768).
                    const DEADZONE: i16 = 8000;
                    if value.saturating_abs() > DEADZONE && !app_state.controller_active {
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
                app_state.settings.upscale_mode,
            );

            scene_manager.handle_event(&mut app_state, &event);

            if scene_manager.get_scene() == SceneType::Exit {
                break 'running;
            }
        }

        // --- Toggle system cursor visibility on controller mode changes ---
        if app_state.controller_active != prev_controller_active {
            sdl_context
                .mouse()
                .show_cursor(!app_state.controller_active);
            prev_controller_active = app_state.controller_active;
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
                DisplayCommand::SetUpscaleMode(mode) => {
                    app_state.settings.upscale_mode = mode;
                    app_state.settings.pixel_perfect_scaling = mode.uses_integer_scale();
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
        let mut rendered_with_scene_texture = false;
        if let Some(texture) = scene_texture.as_mut() {
            match render_scene_texture_to_window(
                &mut canvas,
                texture,
                &mut scene_manager,
                &mut app_state,
            ) {
                Ok(()) => rendered_with_scene_texture = true,
                Err(err) => {
                    log::warn!(
                        "Render-target scene path failed ({err}); falling back to direct rendering"
                    );
                    scene_texture = None;
                }
            }
        }

        if !rendered_with_scene_texture {
            render_direct_to_window(&mut canvas, &mut scene_manager, &mut app_state);
        }

        if scene_manager.get_scene() == SceneType::Exit {
            break 'running;
        }

        canvas.present();

        fps_manager.delay();
    }

    Ok(())
}

/// Creates a client window with the expected size and platform flags.
fn create_window(video: &sdl2::VideoSubsystem) -> Result<Window, String> {
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
    Ok(window)
}

/// Creates the SDL window canvas, optionally requesting render-target support.
fn create_window_canvas(
    video: &sdl2::VideoSubsystem,
    render_target: bool,
) -> Result<Canvas<Window>, String> {
    let builder = create_window(video)?.into_canvas();
    let builder = if render_target {
        builder.target_texture()
    } else {
        builder
    };

    builder.build().map_err(|e| e.to_string())
}

/// Renders one frame through the persistent scene texture and copies it to the window.
fn render_scene_texture_to_window(
    canvas: &mut Canvas<Window>,
    scene_texture: &mut Texture<'_>,
    scene_manager: &mut SceneManager,
    app_state: &mut AppState<'_>,
) -> Result<(), String> {
    canvas
        .with_texture_canvas(scene_texture, |target_canvas| {
            target_canvas.set_draw_color(Color::RGB(0, 0, 0));
            target_canvas.clear();

            let _ = target_canvas.set_integer_scale(false);
            let _ = target_canvas.set_logical_size(0, 0);

            scene_manager.render_world(app_state, target_canvas);
        })
        .map_err(|err| err.to_string())?;

    canvas.set_draw_color(Color::RGB(0, 0, 0));
    canvas.clear();

    let _ = canvas.set_integer_scale(false);
    let _ = canvas.set_logical_size(0, 0);
    scene_texture.set_scale_mode(scene_texture_scale_mode(
        app_state.settings.upscale_mode,
        app_state.settings.pixel_scaler_mode,
    ));
    let dst = scene_destination_rect(canvas.window(), app_state.settings.upscale_mode);
    let copy_result = canvas
        .copy(scene_texture, None, Some(dst))
        .and_then(|_| apply_post_processes(canvas, scene_texture, dst, &app_state.settings));
    reset_window_scaling(canvas);

    copy_result
}

/// Renders one frame directly to the window using the legacy path.
fn render_direct_to_window(
    canvas: &mut Canvas<Window>,
    scene_manager: &mut SceneManager,
    app_state: &mut AppState<'_>,
) {
    let _ = canvas.set_logical_size(constants::TARGET_WIDTH_INT, constants::TARGET_HEIGHT_INT);
    let _ = canvas.set_integer_scale(app_state.settings.upscale_mode.uses_integer_scale());
    scene_manager.render_world(app_state, canvas);
    reset_window_scaling(canvas);
}

/// Returns the destination rectangle for copying the final scene texture.
fn scene_destination_rect(window: &Window, upscale_mode: UpscaleMode) -> Rect {
    let (x, y, width, height) = dpi_scaling::logical_viewport(
        window,
        constants::TARGET_WIDTH,
        constants::TARGET_HEIGHT,
        upscale_mode,
    );
    Rect::new(
        x.round() as i32,
        y.round() as i32,
        width.round().max(1.0) as u32,
        height.round().max(1.0) as u32,
    )
}

/// Returns the SDL sampling mode for the composited scene texture.
fn scene_texture_scale_mode(
    upscale_mode: UpscaleMode,
    pixel_scaler_mode: PixelScalerMode,
) -> ScaleMode {
    if pixel_scaler_mode == PixelScalerMode::Scale2x {
        return ScaleMode::Nearest;
    }

    match upscale_mode {
        UpscaleMode::PixelPerfect | UpscaleMode::Crisp => ScaleMode::Nearest,
        UpscaleMode::Smooth => ScaleMode::Linear,
    }
}

/// Applies final-scene post-process passes after the scene texture is copied.
fn apply_post_processes(
    canvas: &mut Canvas<Window>,
    scene_texture: &mut Texture<'_>,
    dst: Rect,
    settings: &Settings,
) -> Result<(), String> {
    apply_sharpen_pass(canvas, scene_texture, dst, settings.sharpen_mode)?;
    apply_color_grade_pass(canvas, dst, settings.color_grade_mode)?;
    apply_scanline_pass(canvas, dst, settings.scanline_mode)
}

/// Applies a lightweight sharpness/contrast boost using the final scene texture.
fn apply_sharpen_pass(
    canvas: &mut Canvas<Window>,
    scene_texture: &mut Texture<'_>,
    dst: Rect,
    sharpen_mode: SharpenMode,
) -> Result<(), String> {
    let alpha = match sharpen_mode {
        SharpenMode::Off => return Ok(()),
        SharpenMode::Subtle => 18,
        SharpenMode::Strong => 34,
    };

    let old_blend = scene_texture.blend_mode();
    let old_alpha = scene_texture.alpha_mod();
    let old_color = scene_texture.color_mod();

    scene_texture.set_blend_mode(BlendMode::Add);
    scene_texture.set_alpha_mod(alpha);
    scene_texture.set_color_mod(255, 255, 255);
    let result = canvas.copy(scene_texture, None, Some(dst));

    scene_texture.set_blend_mode(old_blend);
    scene_texture.set_alpha_mod(old_alpha);
    scene_texture.set_color_mod(old_color.0, old_color.1, old_color.2);

    result
}

/// Applies a color-grade overlay inside the final scene rectangle.
fn apply_color_grade_pass(
    canvas: &mut Canvas<Window>,
    dst: Rect,
    color_grade_mode: ColorGradeMode,
) -> Result<(), String> {
    let Some((blend_mode, color)) = color_grade_overlay(color_grade_mode) else {
        return Ok(());
    };

    canvas.set_blend_mode(blend_mode);
    canvas.set_draw_color(color);
    canvas.fill_rect(dst)?;
    canvas.set_blend_mode(BlendMode::Blend);
    Ok(())
}

/// Returns the overlay used for the selected color-grade mode.
fn color_grade_overlay(color_grade_mode: ColorGradeMode) -> Option<(BlendMode, Color)> {
    match color_grade_mode {
        ColorGradeMode::Off => None,
        ColorGradeMode::Warm => Some((BlendMode::Blend, Color::RGBA(255, 136, 48, 24))),
        ColorGradeMode::Cool => Some((BlendMode::Blend, Color::RGBA(64, 148, 255, 24))),
        ColorGradeMode::HighContrast => Some((BlendMode::Mod, Color::RGB(232, 232, 232))),
    }
}

/// Applies a scanline overlay inside the final scene rectangle.
fn apply_scanline_pass(
    canvas: &mut Canvas<Window>,
    dst: Rect,
    scanline_mode: ScanlineMode,
) -> Result<(), String> {
    let (step, alpha) = match scanline_mode {
        ScanlineMode::Off => return Ok(()),
        ScanlineMode::Subtle => (3, 36),
        ScanlineMode::Strong => (2, 54),
    };

    let left = dst.x();
    let right = dst.x() + dst.width() as i32 - 1;
    let top = dst.y();
    let bottom = dst.y() + dst.height() as i32;

    canvas.set_blend_mode(BlendMode::Blend);
    canvas.set_draw_color(Color::RGBA(0, 0, 0, alpha));
    for y in (top..bottom).step_by(step) {
        canvas.draw_line(
            sdl2::rect::Point::new(left, y),
            sdl2::rect::Point::new(right, y),
        )?;
    }
    Ok(())
}

/// Restores the renderer to raw physical-pixel coordinates after presentation setup.
fn reset_window_scaling(canvas: &mut Canvas<Window>) {
    let _ = canvas.set_integer_scale(false);
    let _ = canvas.set_logical_size(0, 0);
}

/// Maps [`DisplayMode`] to the SDL2 fullscreen type and applies it.
fn apply_display_mode(canvas: &mut Canvas<Window>, mode: DisplayMode) -> DisplayMode {
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
fn apply_vsync(canvas: &Canvas<Window>, enabled: bool) {
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
