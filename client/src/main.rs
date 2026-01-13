mod constants;
mod gfx_cache;
mod helpers;
mod sfx_cache;
mod states;
mod systems;

use std::sync::OnceLock;
use tracing_appender::{non_blocking::WorkerGuard, rolling};

use bevy::log::{tracing_subscriber::Layer, BoxedLayer, LogPlugin};
use bevy::prelude::*;
use bevy::window::WindowResolution;

use crate::constants::{TARGET_HEIGHT, TARGET_WIDTH};
use crate::gfx_cache::GraphicsCache;
use crate::sfx_cache::SoundCache;
use crate::systems::debug::print_click_coords;
use crate::systems::display::enforce_aspect_and_pixel_coords;

static LOG_GUARD: OnceLock<WorkerGuard> = OnceLock::new();

#[allow(dead_code)]
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
        .insert_state(GameState::Loading)
        .insert_resource(ClearColor(Color::BLACK))
        .add_systems(Startup, setup_camera)
        .add_systems(StateTransition, run_on_any_transition)
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
        .add_systems(
            OnEnter(GameState::Gameplay),
            states::gameplay::setup_gameplay,
        )
        .add_systems(
            Update,
            states::gameplay::run_gameplay.run_if(in_state(GameState::Gameplay)),
        )
        .add_systems(OnEnter(GameState::Menu), states::menu::setup_menu)
        .add_systems(
            Update,
            states::menu::run_menu.run_if(in_state(GameState::Menu)),
        )
        .add_systems(OnExit(GameState::Menu), states::menu::teardown_menu)
        .add_systems(
            Update,
            print_click_coords.run_if(in_state(GameState::Gameplay)),
        )
        .add_systems(Update, enforce_aspect_and_pixel_coords)
        .run();
}

// System to run on any transition
fn run_on_any_transition(mut transitions: MessageReader<StateTransitionEvent<GameState>>) {
    for ev in transitions.read() {
        log::info!(
            "State Transition Detected! From {:?} to {:?}",
            ev.exited,
            ev.entered
        );
    }
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
