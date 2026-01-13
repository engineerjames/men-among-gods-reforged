mod constants;
mod gfx_cache;
mod helpers;
mod sfx_cache;
mod states;
mod systems;

use bevy_egui::{EguiPlugin, EguiPrimaryContextPass};
use std::sync::OnceLock;
use tracing_appender::{non_blocking::WorkerGuard, rolling};

use bevy::log::{tracing_subscriber::Layer, BoxedLayer, LogPlugin};
use bevy::prelude::*;
use bevy::window::WindowResolution;

use crate::constants::{TARGET_HEIGHT, TARGET_WIDTH};
use crate::gfx_cache::GraphicsCache;
use crate::sfx_cache::SoundCache;
use crate::systems::debug;
use crate::systems::display;

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

fn main() {
    App::new()
        // Setup resources
        // Use stable absolute paths so running from workspace root works.
        .insert_resource(GraphicsCache::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/assets/GFX/images.zip"
        )))
        .insert_resource(SoundCache::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/assets/SFX"
        )))
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
            states::gameplay::run_gameplay.run_if(in_state(GameState::Gameplay)),
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
        .add_systems(
            Update,
            debug::print_click_coords.run_if(in_state(GameState::Gameplay)),
        )
        .add_systems(StateTransition, debug::run_on_any_transition)
        .add_systems(Update, display::enforce_aspect_and_pixel_coords)
        .run();
}

fn setup_camera(mut commands: Commands) {
    commands.spawn((
        Name::new("Camera"),
        Camera2d::default(),
        Projection::from(OrthographicProjection {
            scaling_mode: bevy::camera::ScalingMode::AutoMin {
                min_width: TARGET_WIDTH,
                min_height: TARGET_HEIGHT,
            },
            ..OrthographicProjection::default_2d()
        }),
    ));
}
