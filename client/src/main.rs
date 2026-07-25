use std::process;
use std::time::{Duration, Instant};

use sdl2::gfx::framerate::FPSManager;
use sdl2::image::InitFlag;
use sdl2::mixer::{AUDIO_S16LSB, DEFAULT_CHANNELS};
use sdl2::video::FullscreenType;

use client::font_cache::TextEngine;
use client::frame_buffer::FrameBuffer;
use client::gfx_cache::GraphicsCache;
use client::platform::PlatformProfile;
use client::preferences::{DisplayMode, RenderScale, Settings};
use client::scenes::scene::{FramePresentation, SceneType};
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
    let perf_log_path = preferences::perf_log_file_path();

    let perf_log_path_str = perf_log_path.to_string_lossy();
    let log_path_str = log_path.to_string_lossy();
    mag_core::initialize_logger(
        log::LevelFilter::Info,
        Some(log_path_str.as_ref()),
        Some(perf_log_path_str.as_ref()),
    )
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
                log::warn!("Failed to initialise SDL2_mixer (audio will be disabled): {e}");
            })
            .is_ok();

    log::info!("Creating window and event pump...");
    let video = sdl_context.video()?;
    let window_title = format!("Men Among Gods - Reforged v{}", env!("CARGO_PKG_VERSION"));
    let mut window = video
        .window(
            &window_title,
            constants::TARGET_WIDTH_INT,
            constants::TARGET_HEIGHT_INT,
        )
        .position_centered()
        .allow_highdpi()
        .resizable()
        .build()
        .map_err(|e| e.to_string())?;

    let _ = window.set_minimum_size(constants::TARGET_WIDTH_INT, constants::TARGET_HEIGHT_INT);

    // `target_texture()` is required for the off-screen frame buffer used by the
    // enhanced graphics pipeline; `accelerated()` makes the GPU path explicit
    // rather than relying on SDL's driver ordering.
    let mut canvas = window
        .into_canvas()
        .accelerated()
        .target_texture()
        .build()
        .map_err(|e| e.to_string())?;

    let mut event_pump = sdl_context.event_pump()?;

    log::info!("Initializing graphics and sound caches (audio_available={audio_available})...");
    let texture_creator = canvas.texture_creator();
    let gfx_cache = GraphicsCache::new(filepaths::get_gfx_zipfile(), &texture_creator);

    // SDL TTF context: leak to 'static so AppState only needs the existing
    // 'tc lifetime (matching gfx_cache). Single-process resource that lives
    // for the entire run, so leaking is acceptable.
    let ttf_ctx_static: &'static sdl2::ttf::Sdl2TtfContext =
        Box::leak(Box::new(sdl2::ttf::init().map_err(|e| e.to_string())?));
    let text_engine = TextEngine::new(
        ttf_ctx_static,
        &texture_creator,
        filepaths::get_fonts_directory(),
        1.0,
    );
    // NOTE: the TTF DPI scale is driven by the internal render scale, not by the
    // canvas logical size (which is zero at this point). It is applied below,
    // once settings have been loaded, and again whenever the render scale changes.

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
        text_engine,
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

    // Sprite textures grow quadratically with the upscale factor, so cap the
    // cache more tightly on memory-constrained handhelds.
    if platform.is_steam_deck() {
        app_state.gfx_cache.set_cache_budget_bytes(96 * 1024 * 1024);
    }

    // Display mode
    let requested_mode = app_state.settings.display_mode;
    let applied_startup_mode = apply_display_mode(&mut canvas, requested_mode);
    app_state.settings.display_mode = applied_startup_mode;
    if applied_startup_mode != requested_mode {
        save_global_display_settings(&app_state);
    }

    // VSync (runtime toggle via raw SDL2 FFI)
    let mut effective_vsync_enabled = apply_vsync(&canvas, app_state.settings.vsync_enabled, false);

    // --- Enhanced graphics pipeline ---------------------------------------
    // The whole scene is composed into one off-screen texture at
    // `factor x 960 x 540` and blitted to the window once, so scaling and
    // filtering happen exactly once on a single contiguous image.
    //
    // `render_factor` starts at 0 so the first loop iteration always builds the
    // buffer, keeping the setup and the resize/settings paths on one code path.
    let mut frame_buffer: Option<FrameBuffer> = None;
    let mut render_factor: u32 = 0;
    // ----------------------------------------------------------------------
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
                sdl2::event::Event::ControllerButtonDown { .. } if !app_state.controller_active => {
                    log::info!("Controller input detected — switching to controller mode");
                    app_state.controller_active = true;
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
                | sdl2::event::Event::TextInput { .. }
                    if app_state.controller_active =>
                {
                    log::info!("Keyboard/mouse input detected — leaving controller mode");
                    app_state.controller_active = false;
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

        // --- Toggle system cursor visibility on controller mode changes ---
        if app_state.controller_active != prev_controller_active {
            sdl_context
                .mouse()
                .show_cursor(!app_state.controller_active);
            prev_controller_active = app_state.controller_active;
        }

        scene_manager.update(&mut app_state, dt);

        // --- Apply any pending display commands from the UI ---------------
        while let Some(cmd) = app_state.display_commands.pop_front() {
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
                    effective_vsync_enabled =
                        apply_vsync(&canvas, enabled, effective_vsync_enabled);
                    app_state.settings.vsync_enabled = enabled;
                    save_global_display_settings(&app_state);
                }
                DisplayCommand::SetRenderScale(scale) => {
                    app_state.settings.render_scale = scale;
                    // Force the render section below to rebuild the buffer.
                    render_factor = 0;
                    save_global_display_settings(&app_state);
                }
                DisplayCommand::SetOutputFilter(filter) => {
                    app_state.settings.output_filter = filter;
                    if let Some(fb) = frame_buffer.as_mut() {
                        fb.set_filter(filter);
                    }
                    save_global_display_settings(&app_state);
                }
                DisplayCommand::SetSpriteUpscaler(upscaler) => {
                    app_state.settings.sprite_upscaler = upscaler;
                    app_state
                        .gfx_cache
                        .set_sprite_scaling(render_factor, upscaler);
                    save_global_display_settings(&app_state);
                }
            }
        }
        // ------------------------------------------------------------------
        let frame_presentation = scene_manager.take_frame_presentation();
        if frame_presentation == FramePresentation::Skip {
            continue;
        }

        // --- (Re)build the frame buffer when the effective scale changes ----
        // The desired factor depends on both the settings and the current
        // drawable size, so this has to be re-evaluated every frame; the
        // expensive rebuild only runs when the factor actually changes.
        let (drawable_w, drawable_h) = canvas.window().drawable_size();
        let desired_factor = app_state.settings.render_scale.factor(
            drawable_w,
            drawable_h,
            constants::TARGET_WIDTH_INT,
            constants::TARGET_HEIGHT_INT,
        );
        if desired_factor != render_factor {
            let (buffer, applied) =
                build_frame_buffer(&texture_creator, desired_factor, &app_state.settings);
            render_factor = applied;
            frame_buffer = buffer;
            apply_render_scale(&mut app_state, applied);

            // If an explicitly requested scale could not be allocated, persist
            // the downgraded value so the UI reflects reality and the next
            // launch does not retry a scale this machine cannot support.
            // `Auto` is left alone: it is resolution-dependent by definition.
            if applied != desired_factor && app_state.settings.render_scale != RenderScale::Auto {
                app_state.settings.render_scale = RenderScale::from_factor(applied);
                save_global_display_settings(&app_state);
            }
        }
        // -------------------------------------------------------------------

        if let Some(fb) = frame_buffer.as_mut() {
            let compose_result = fb.compose(&mut canvas, |target| {
                scene_manager.render_world(&mut app_state, target);
            });

            if let Err(e) = compose_result {
                log::error!("Frame composition failed, falling back to direct rendering: {e}");
                frame_buffer = None;
                render_factor = 0;
                continue;
            }

            // The composed frame is letterboxed, so the margins must be cleared
            // or they retain the previous frame's pixels.
            canvas.set_draw_color(sdl2::pixels::Color::RGB(0, 0, 0));
            canvas.clear();
            let dst = dpi_scaling::present_rect(
                drawable_w,
                drawable_h,
                constants::TARGET_WIDTH,
                constants::TARGET_HEIGHT,
                app_state.settings.pixel_perfect_scaling,
            );
            if let Some(fb) = frame_buffer.as_mut()
                && let Err(e) = fb.present(&mut canvas, dst)
            {
                log::error!("Frame present failed: {e}");
            }
        } else {
            // Fallback path used when no render target could be allocated:
            // render straight into the backbuffer using SDL's logical scaling.
            let _ =
                canvas.set_logical_size(constants::TARGET_WIDTH_INT, constants::TARGET_HEIGHT_INT);
            // Integer scale --> pixel-perfect (nearest integer multiplier) when on.
            let _ = canvas.set_integer_scale(app_state.settings.pixel_perfect_scaling);
            scene_manager.render_world(&mut app_state, &mut canvas);
            // Logical size off --> raw physical pixels.
            let _ = canvas.set_integer_scale(false);
            let _ = canvas.set_logical_size(0, 0);
        }

        if scene_manager.get_scene() == SceneType::Exit {
            break 'running;
        }

        if let FramePresentation::PresentAt(deadline) = frame_presentation {
            wait_until(deadline, &mut event_pump);
        }
        canvas.present();

        if frame_presentation == FramePresentation::Immediate && !effective_vsync_enabled {
            fps_manager.delay();
        }
    }

    Ok(())
}

/// Builds the off-screen frame buffer, downgrading the factor on failure.
///
/// Large render targets can fail to allocate on low-VRAM devices, so rather
/// than giving up the requested factor is halved step by step until either a
/// target is created or every factor has been tried.
///
/// # Arguments
///
/// * `creator` - Texture creator bound to the window renderer.
/// * `desired_factor` - Internal resolution multiplier requested by settings.
/// * `settings` - Current display settings, used for the output filter.
///
/// # Returns
///
/// * `(Some(buffer), applied_factor)` on success, or `(None, 1)` when no
///   render target could be allocated at any factor.
fn build_frame_buffer<'tc>(
    creator: &'tc sdl2::render::TextureCreator<sdl2::video::WindowContext>,
    desired_factor: u32,
    settings: &Settings,
) -> (Option<FrameBuffer<'tc>>, u32) {
    for factor in (1..=desired_factor.max(1)).rev() {
        match FrameBuffer::new(
            creator,
            constants::TARGET_WIDTH_INT,
            constants::TARGET_HEIGHT_INT,
            factor,
            settings.output_filter,
        ) {
            Ok(buffer) => {
                if factor != desired_factor {
                    log::warn!(
                        "Internal render scale downgraded from {desired_factor}x to {factor}x"
                    );
                } else {
                    log::info!("Internal render scale set to {factor}x");
                }
                return (Some(buffer), factor);
            }
            Err(e) => log::warn!("Frame buffer at {factor}x unavailable: {e}"),
        }
    }
    log::error!("No off-screen render target available; using direct rendering");
    (None, 1)
}

/// Propagates a new internal render scale to the text and sprite caches.
///
/// TrueType glyphs are rasterised at the internal resolution so they stay crisp
/// instead of being upscaled as bitmaps, and archive sprites are upscaled by
/// the same factor so they blit 1:1 into the frame buffer.
///
/// # Arguments
///
/// * `app_state` - Application state owning the caches.
/// * `factor` - The internal resolution multiplier now in effect.
fn apply_render_scale(app_state: &mut AppState<'_>, factor: u32) {
    app_state.text_engine.set_dpi_scale(factor as f32);
    app_state
        .gfx_cache
        .set_sprite_scaling(factor, app_state.settings.sprite_upscaler);
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

/// Waits until an absolute presentation deadline without accumulating drift.
///
/// # Arguments
///
/// * `deadline` - Absolute time at which the completed frame should be presented.
/// * `event_pump` - SDL event pump used to keep the operating-system window responsive.
fn wait_until(deadline: Instant, event_pump: &mut sdl2::EventPump) {
    const COARSE_MARGIN: Duration = Duration::from_millis(1);

    loop {
        event_pump.pump_events();
        let now = Instant::now();
        let Some(remaining) = deadline.checked_duration_since(now) else {
            return;
        };
        if remaining > COARSE_MARGIN {
            std::thread::sleep(remaining - COARSE_MARGIN);
        } else {
            std::thread::yield_now();
        }
    }
}

/// Toggles VSync on the renderer at runtime via raw SDL2 FFI.
///
/// # Arguments
///
/// * `canvas` - SDL renderer whose swap interval should change.
/// * `enabled` - Requested VSync state.
/// * `previous_effective` - Last VSync state successfully applied to SDL.
///
/// # Returns
///
/// * The effective VSync state after the request.
fn apply_vsync(
    canvas: &sdl2::render::Canvas<sdl2::video::Window>,
    enabled: bool,
    previous_effective: bool,
) -> bool {
    let raw = canvas.raw();
    let flag: std::os::raw::c_int = if enabled { 1 } else { 0 };
    let result = unsafe { sdl2::sys::SDL_RenderSetVSync(raw, flag) };
    if result != 0 {
        log::error!(
            "SDL_RenderSetVSync requested={} failed; retaining effective={}: {}",
            enabled,
            previous_effective,
            sdl2::get_error()
        );
        previous_effective
    } else {
        enabled
    }
}

/// Persists current display-related settings from [`AppState`] into the
/// global profile.
fn save_global_display_settings(app_state: &AppState<'_>) {
    if let Err(e) = preferences::save_global_settings(&app_state.settings) {
        log::error!("Failed to persist display settings: {e}");
    }
}
