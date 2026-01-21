mod constants;
mod font_cache;
mod gfx_cache;
mod helpers;
mod map;
mod network;
mod player_state;
mod sfx_cache;
mod states;
mod systems;
mod types;

use bevy_egui::{EguiPlugin, EguiPrimaryContextPass};
use std::path::PathBuf;
use std::sync::OnceLock;
use tracing_appender::{non_blocking::WorkerGuard, rolling};

use bevy::log::{tracing_subscriber::Layer, BoxedLayer, LogPlugin};
use bevy::prelude::*;
use bevy::window::WindowResolution;
use bevy::winit::{UpdateMode, WinitSettings};

use crate::constants::{TARGET_HEIGHT, TARGET_WIDTH};
use crate::gfx_cache::GraphicsCache;
use crate::sfx_cache::SoundCache;
use crate::systems::debug::{self, GameplayDebugSettings};
use crate::systems::display;
use crate::systems::map_hover;
use crate::systems::nameplates;
use crate::systems::sound;

static LOG_GUARD: OnceLock<WorkerGuard> = OnceLock::new();

#[derive(States, Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum GameState {
    Loading,
    LoggingIn,
    Gameplay,
    Menu,
}

fn custom_layer(_app: &mut App) -> Option<BoxedLayer> {
    let file_appender = rolling::daily("logs", "client.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);
    let _ = LOG_GUARD.set(guard);
    Some(
        bevy::log::tracing_subscriber::fmt::layer()
            .with_ansi(false)
            .with_writer(non_blocking)
            .with_file(true)
            .with_line_number(true)
            .boxed(),
    )
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
    let assets_dir = resolve_assets_base_dir();
    let gfx_zip = assets_dir.join("GFX").join("images.zip");
    let sfx_dir = assets_dir.join("SFX");

    App::new()
        // Setup resources
        .insert_resource(GraphicsCache::new(gfx_zip.to_string_lossy().as_ref()))
        .insert_resource(SoundCache::new(sfx_dir.to_string_lossy().as_ref()))
        // Keep the game simulation running even when the window is unfocused/minimized.
        .insert_resource(WinitSettings {
            focused_mode: UpdateMode::Continuous,
            unfocused_mode: UpdateMode::Continuous,
            ..default()
        })
        .init_resource::<font_cache::FontCache>()
        .init_resource::<sound::SoundEventQueue>()
        .init_resource::<GameplayDebugSettings>()
        .init_resource::<states::gameplay::MiniMapState>()
        .init_resource::<player_state::PlayerState>()
        .add_plugins(
            DefaultPlugins
                .build()
                .set(ImagePlugin::default_nearest())
                .set(LogPlugin {
                    custom_layer,
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
        .add_plugins(network::NetworkPlugin)
        // Initialize the state to loading
        .insert_state(GameState::Loading)
        .insert_resource(ClearColor(Color::BLACK))
        //
        // Setup systems for each state
        //
        // Initial setup
        //
        .add_systems(Startup, setup_camera)
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
                .run_if(in_state(GameState::Gameplay))
                .after(network::NetworkSet::Receive),
        )
        .add_systems(
            Update,
            sound::play_queued_sounds.run_if(in_state(GameState::Gameplay)),
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
            map_hover::run_gameplay_move_target_marker
                .run_if(in_state(GameState::Gameplay))
                .after(states::gameplay::run_gameplay),
        )
        .add_systems(
            Update,
            map_hover::run_gameplay_attack_target_marker
                .run_if(in_state(GameState::Gameplay))
                .after(states::gameplay::run_gameplay),
        )
        .add_systems(
            Update,
            map_hover::run_gameplay_misc_action_marker
                .run_if(in_state(GameState::Gameplay))
                .after(states::gameplay::run_gameplay),
        )
        .add_systems(
            Update,
            map_hover::run_gameplay_sprite_highlight
                .run_if(in_state(GameState::Gameplay))
                .after(states::gameplay::run_gameplay),
        )
        .add_systems(
            Update,
            nameplates::run_gameplay_nameplates.run_if(in_state(GameState::Gameplay)),
        )
        .add_systems(
            Update,
            states::gameplay::run_gameplay_text_ui.run_if(in_state(GameState::Gameplay)),
        )
        .add_systems(
            Update,
            states::gameplay::ui::hud::run_gameplay_update_hud_labels
                .run_if(in_state(GameState::Gameplay)),
        )
        .add_systems(
            Update,
            states::gameplay::ui::extra::run_gameplay_update_extra_ui
                .run_if(in_state(GameState::Gameplay)),
        )
        .add_systems(
            Update,
            states::gameplay::ui::hud::run_gameplay_update_stat_bars
                .run_if(in_state(GameState::Gameplay)),
        )
        .add_systems(
            Update,
            states::gameplay::ui::statbox::run_gameplay_update_scroll_knobs
                .run_if(in_state(GameState::Gameplay)),
        )
        .add_systems(
            Update,
            states::gameplay::ui::portrait::run_gameplay_update_top_selected_name
                .run_if(in_state(GameState::Gameplay)),
        )
        .add_systems(
            Update,
            states::gameplay::ui::portrait::run_gameplay_update_portrait_name_and_rank
                .run_if(in_state(GameState::Gameplay)),
        )
        .add_systems(
            Update,
            states::gameplay::ui::shop::run_gameplay_update_shop_price_labels
                .run_if(in_state(GameState::Gameplay))
                .after(states::gameplay::ui::shop::run_gameplay_shop_input),
        )
        .add_systems(
            Update,
            states::gameplay::run_gameplay_bitmap_text_renderer
                .run_if(in_state(GameState::Gameplay))
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
            Update,
            states::menu::run_menu.run_if(in_state(GameState::Menu)),
        )
        .add_systems(OnExit(GameState::Menu), states::menu::teardown_menu)
        //
        // Global (utility) systems
        //
        .add_systems(StateTransition, debug::run_on_any_transition)
        .add_systems(Update, display::enforce_aspect_and_pixel_coords)
        .run();
}

fn setup_camera(mut commands: Commands) {
    commands.spawn((
        Name::new("Camera"),
        Camera2d::default(),
        SpatialListener::default(),
        Transform::default(),
        GlobalTransform::default(),
        Projection::from(OrthographicProjection {
            scaling_mode: bevy::camera::ScalingMode::AutoMin {
                min_width: TARGET_WIDTH,
                min_height: TARGET_HEIGHT,
            },
            ..OrthographicProjection::default_2d()
        }),
    ));
}
