#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod constants;
mod font_cache;
mod gfx_cache;
mod helpers;
mod map;
mod network;
mod player_state;
mod settings;
mod sfx_cache;
mod states;
mod systems;
mod types;

use bevy_egui::{EguiPlugin, EguiPrimaryContextPass};
use std::path::PathBuf;
use tracing_appender::{non_blocking::WorkerGuard, rolling};

use bevy::log::{tracing_subscriber::Layer, BoxedLayer, LogPlugin};
use bevy::prelude::*;
#[cfg(not(target_os = "macos"))]
use bevy::window::PrimaryWindow;
use bevy::window::WindowResolution;
#[cfg(not(target_os = "macos"))]
use bevy::winit::WinitWindows;
use bevy::winit::{UpdateMode, WinitSettings};
#[cfg(not(target_os = "macos"))]
use winit::window::Icon;

use crate::gfx_cache::GraphicsCache;
use crate::sfx_cache::SoundCache;
use crate::systems::debug::{self, GameplayDebugSettings};
use crate::systems::display;
use crate::systems::magic_postprocess::MagicPostProcessPlugin;
use crate::systems::map_hover;
use crate::systems::nameplates;
use crate::systems::sound;

fn install_crash_handler() {
    std::panic::set_hook(Box::new(|info| {
        // Best-effort: write a crash report to the same directory we use for logs.
        let log_dir = resolve_log_dir();
        let _ = std::fs::create_dir_all(&log_dir);

        let crash_path = log_dir.join("client.crash.log");
        let backtrace = std::backtrace::Backtrace::force_capture();

        let payload =
            format!("Men Among Gods client panic\n\n{info}\n\nBacktrace:\n{backtrace:?}\n");

        // Avoid panicking inside the panic hook.
        let _ = std::fs::write(&crash_path, payload);
    }));
}

#[derive(Resource)]
struct LogGuard(#[allow(dead_code)] WorkerGuard);

#[derive(Resource, Clone, Debug)]
struct ClientAssetsDir(#[allow(dead_code)] PathBuf);

#[derive(States, Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum GameState {
    Loading,
    LoggingIn,
    Gameplay,
    Menu,
    Exited,
}

pub(crate) fn resolve_log_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("MAG_LOG_DIR") {
        if !dir.is_empty() {
            return PathBuf::from(dir);
        }
    }

    if let Some(base) = helpers::get_mag_base_dir() {
        return base;
    }

    std::env::temp_dir().join(".men-among-gods")
}

fn custom_layer(app: &mut App) -> Option<BoxedLayer> {
    let log_dir = resolve_log_dir();
    // Avoid panicking on startup if the log directory cannot be created.
    if std::fs::create_dir_all(&log_dir).is_err() {
        return None;
    }

    let file_appender = rolling::daily(log_dir, "client.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);
    app.insert_resource(LogGuard(guard));
    Some(
        bevy::log::tracing_subscriber::fmt::layer()
            .with_ansi(false)
            .with_writer(non_blocking)
            .with_file(true)
            .with_line_number(true)
            .boxed(),
    )
}

fn resolve_log_filter() -> String {
    // Respect an explicit user override.
    if let Ok(filter) = std::env::var("RUST_LOG") {
        if !filter.trim().is_empty() {
            return filter;
        }
    }

    // The Vulkan loader can emit very loud "ERROR" messages on Windows when there are stale
    // (uninstalled/moved) implicit layer registrations. These are often non-fatal and can be
    // confusing to users.
    //
    // If you need to debug Vulkan instance creation, set `RUST_LOG` explicitly.
    if cfg!(target_os = "windows") {
        "info,wgpu_hal::vulkan::instance=off".to_string()
    } else {
        "info".to_string()
    }
}

fn resolve_assets_base_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("MAG_ASSETS_DIR") {
        if !dir.is_empty() {
            return PathBuf::from(dir);
        }
    }

    // Prefer workspace-relative assets when running from source.
    let dev_assets = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets");
    if dev_assets.exists() {
        return dev_assets;
    }

    // Fall back to assets next to the built executable for packaged releases.
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.join("assets")))
        .unwrap_or_else(|| dev_assets)
}

fn main() {
    install_crash_handler();

    let assets_dir = resolve_assets_base_dir();
    let gfx_zip = assets_dir.join("gfx").join("images.zip");
    let sfx_dir = assets_dir.join("sfx");

    log::info!("Using assets directory: {:?}", assets_dir);
    log::info!("Using graphics zip: {:?}", gfx_zip);
    log::info!("Using sound effects directory: {:?}", sfx_dir);
    log::info!("Base directory: {:?}", helpers::get_mag_base_dir());
    log::info!("Log directory: {:?}", resolve_log_dir());

    let mut app = App::new();
    let log_filter = resolve_log_filter();
    app
        // Setup resources
        .insert_resource(GraphicsCache::new(gfx_zip.to_string_lossy().as_ref()))
        .insert_resource(SoundCache::new(sfx_dir.to_string_lossy().as_ref()))
        .insert_resource(ClientAssetsDir(assets_dir))
        // Keep the game simulation running even when the window is unfocused/minimized.
        .insert_resource(WinitSettings {
            focused_mode: UpdateMode::Continuous,
            unfocused_mode: UpdateMode::Continuous,
            ..default()
        })
        .init_resource::<font_cache::FontCache>()
        .init_resource::<sound::SoundEventQueue>()
        .init_resource::<sound::SoundSettings>()
        .init_resource::<states::gameplay::CursorActionTextSettings>()
        .init_resource::<GameplayDebugSettings>()
        .init_resource::<states::gameplay::MiniMapState>()
        .init_resource::<player_state::PlayerState>()
        .add_plugins(
            DefaultPlugins
                .build()
                .set(ImagePlugin::default_nearest())
                .set(LogPlugin {
                    custom_layer,
                    filter: log_filter,
                    ..default()
                })
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Men Among Gods (Client)".to_string(),
                        resolution: WindowResolution::new(800, 600),
                        resizable: true,
                        ..default()
                    }),
                    ..default()
                }),
        )
        .add_plugins(EguiPlugin::default())
        .add_plugins(MagicPostProcessPlugin)
        .add_plugins(network::NetworkPlugin)
        // Initialize the state to loading
        .insert_state(GameState::Loading)
        .insert_resource(ClearColor(Color::BLACK))
        //
        // Setup systems for each state
        //
        // Initial setup
        //
        // Cameras are set up by MagicPostProcessPlugin (world -> texture -> postprocess -> UI).
        //
        // Loading state
        //
        .add_systems(
            OnEnter(GameState::Loading),
            states::loading::setup_loading_ui,
        )
        .add_systems(
            Update,
            states::loading::run_loading.run_if(in_state(GameState::Loading)),
        )
        .add_systems(
            OnExit(GameState::Loading),
            states::loading::teardown_loading_ui,
        )
        //
        // LoggingIn state
        //
        .add_systems(
            OnEnter(GameState::LoggingIn),
            states::logging_in::setup_logging_in,
        )
        .add_systems(
            EguiPrimaryContextPass,
            states::logging_in::run_logging_in.run_if(in_state(GameState::LoggingIn)),
        )
        .add_systems(
            OnExit(GameState::LoggingIn),
            states::logging_in::teardown_logging_in,
        )
        //
        // Gameplay state
        //
        .add_systems(
            OnEnter(GameState::Gameplay),
            states::gameplay::setup_gameplay,
        )
        .add_systems(
            Update,
            states::gameplay::run_gameplay
                .run_if(in_state(GameState::Gameplay).or(in_state(GameState::Menu)))
                .after(network::NetworkSet::Receive),
        )
        .add_systems(
            Update,
            sound::play_queued_sounds.run_if(in_state(GameState::Gameplay)),
        )
        .add_systems(
            Update,
            sound::play_queued_sounds.run_if(in_state(GameState::Menu)),
        )
        .add_systems(
            Update,
            states::gameplay::ui::hud::run_gameplay_buttonbox_toggles
                .run_if(in_state(GameState::Gameplay)),
        )
        .add_systems(
            Update,
            states::gameplay::ui::statbox::run_gameplay_statbox_input
                .run_if(in_state(GameState::Gameplay)),
        )
        .add_systems(
            Update,
            states::gameplay::ui::inventory::run_gameplay_inventory_input
                .run_if(in_state(GameState::Gameplay)),
        )
        .add_systems(
            Update,
            states::gameplay::ui::shop::run_gameplay_shop_input
                .run_if(in_state(GameState::Gameplay)),
        )
        .add_systems(
            Update,
            states::gameplay::ui::cursor::run_gameplay_update_cursor_and_carried_item
                .run_if(in_state(GameState::Gameplay)),
        )
        .add_systems(
            Update,
            states::gameplay::ui::inventory::run_gameplay_update_equipment_blocks
                .run_if(in_state(GameState::Gameplay)),
        )
        .add_systems(
            Update,
            map_hover::run_gameplay_map_hover_and_click.run_if(in_state(GameState::Gameplay)),
        )
        .add_systems(
            Update,
            states::gameplay::ui::cursor::run_gameplay_update_cursor_action_text
                .run_if(in_state(GameState::Gameplay))
                .after(map_hover::run_gameplay_map_hover_and_click)
                .before(states::gameplay::run_gameplay_bitmap_text_renderer),
        )
        .add_systems(
            Update,
            map_hover::run_gameplay_move_target_marker
                .run_if(in_state(GameState::Gameplay).or(in_state(GameState::Menu)))
                .after(states::gameplay::run_gameplay),
        )
        .add_systems(
            Update,
            map_hover::run_gameplay_attack_target_marker
                .run_if(in_state(GameState::Gameplay).or(in_state(GameState::Menu)))
                .after(states::gameplay::run_gameplay),
        )
        .add_systems(
            Update,
            map_hover::run_gameplay_misc_action_marker
                .run_if(in_state(GameState::Gameplay).or(in_state(GameState::Menu)))
                .after(states::gameplay::run_gameplay),
        )
        .add_systems(
            Update,
            map_hover::run_gameplay_sprite_highlight
                .run_if(in_state(GameState::Gameplay).or(in_state(GameState::Menu)))
                .after(states::gameplay::run_gameplay),
        )
        .add_systems(
            Update,
            nameplates::run_gameplay_nameplates
                .run_if(in_state(GameState::Gameplay).or(in_state(GameState::Menu)))
                .after(states::gameplay::run_gameplay),
        )
        .add_systems(
            Update,
            states::gameplay::run_gameplay_text_ui
                .run_if(in_state(GameState::Gameplay).or(in_state(GameState::Menu))),
        )
        .add_systems(
            Update,
            states::gameplay::ui::hud::run_gameplay_update_hud_labels
                .run_if(in_state(GameState::Gameplay).or(in_state(GameState::Menu))),
        )
        .add_systems(
            Update,
            states::gameplay::ui::extra::run_gameplay_update_extra_ui
                .run_if(in_state(GameState::Gameplay).or(in_state(GameState::Menu))),
        )
        .add_systems(
            Update,
            states::gameplay::ui::hud::run_gameplay_update_stat_bars
                .run_if(in_state(GameState::Gameplay).or(in_state(GameState::Menu))),
        )
        .add_systems(
            Update,
            states::gameplay::ui::statbox::run_gameplay_update_scroll_knobs
                .run_if(in_state(GameState::Gameplay).or(in_state(GameState::Menu))),
        )
        .add_systems(
            Update,
            states::gameplay::ui::portrait::run_gameplay_update_top_selected_name
                .run_if(in_state(GameState::Gameplay).or(in_state(GameState::Menu))),
        )
        .add_systems(
            Update,
            states::gameplay::ui::portrait::run_gameplay_update_portrait_name_and_rank
                .run_if(in_state(GameState::Gameplay).or(in_state(GameState::Menu))),
        )
        .add_systems(
            Update,
            states::gameplay::ui::shop::run_gameplay_update_shop_price_labels
                .run_if(in_state(GameState::Gameplay).or(in_state(GameState::Menu)))
                .after(states::gameplay::ui::shop::run_gameplay_shop_input),
        )
        .add_systems(
            Update,
            states::gameplay::run_gameplay_bitmap_text_renderer
                .run_if(in_state(GameState::Gameplay).or(in_state(GameState::Menu)))
                .after(states::gameplay::ui::extra::run_gameplay_update_extra_ui)
                .after(states::gameplay::ui::hud::run_gameplay_update_stat_bars)
                .after(states::gameplay::ui::portrait::run_gameplay_update_top_selected_name)
                .after(states::gameplay::ui::portrait::run_gameplay_update_portrait_name_and_rank)
                .after(states::gameplay::ui::shop::run_gameplay_update_shop_price_labels)
                .after(nameplates::run_gameplay_nameplates),
        )
        //
        // Menu state
        //
        .add_systems(OnEnter(GameState::Menu), states::menu::setup_menu)
        .add_systems(
            EguiPrimaryContextPass,
            states::menu::run_menu.run_if(in_state(GameState::Menu)),
        )
        .add_systems(OnExit(GameState::Menu), states::menu::teardown_menu)
        //
        // Exited state
        //
        .add_systems(OnEnter(GameState::Exited), states::exited::setup_exited)
        .add_systems(
            Update,
            states::exited::apply_exit_request.run_if(in_state(GameState::Exited)),
        )
        .add_systems(
            EguiPrimaryContextPass,
            states::exited::run_exited.run_if(in_state(GameState::Exited)),
        )
        .add_systems(OnExit(GameState::Exited), states::exited::teardown_exited)
        //
        // Global (utility) systems
        //
        .add_systems(StateTransition, debug::run_on_any_transition)
        .add_systems(Update, display::enforce_aspect_and_pixel_coords)
        .add_systems(Startup, settings::load_user_settings_startup)
        .add_systems(Update, settings::save_user_settings_if_pending);

    app.run();
}
