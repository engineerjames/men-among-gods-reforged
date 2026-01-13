mod constants;
mod gfx_cache;
mod sfx_cache;
mod systems;

use std::sync::OnceLock;
use tracing_appender::{non_blocking::WorkerGuard, rolling};

use bevy::log::{tracing_subscriber::Layer, BoxedLayer, LogPlugin};
use bevy::prelude::*;
use bevy::window::WindowResolution;

use crate::constants::{TARGET_HEIGHT, TARGET_WIDTH};
use crate::gfx_cache::CacheInitStatus;
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

#[derive(Component)]
struct LoadingUi;

#[derive(Component)]
struct LoadingLabel;

#[derive(Component)]
struct LoadingBarFill;

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
        .add_systems(OnEnter(GameState::Loading), setup_loading_ui)
        .add_systems(Update, run_loading.run_if(in_state(GameState::Loading)))
        .add_systems(OnExit(GameState::Loading), teardown_loading_ui)
        .add_systems(OnEnter(GameState::Gameplay), setup_gameplay)
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

fn setup_loading_ui(
    mut commands: Commands,
    mut gfx: ResMut<GraphicsCache>,
    mut sfx: ResMut<SoundCache>,
) {
    gfx.reset_loading();
    sfx.reset_loading();

    commands.spawn((
        LoadingUi,
        Node {
            width: percent(100),
            height: percent(100),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            flex_direction: FlexDirection::Column,
            row_gap: px(16),
            ..default()
        },
        BackgroundColor(Color::BLACK),
        children![
            (
                LoadingLabel,
                Text::new("LOADING GFX"),
                TextFont::from_font_size(42.0),
                TextColor(Color::srgb(0.95, 0.95, 0.95)),
            ),
            (
                Node {
                    width: px(420),
                    height: px(22),
                    padding: UiRect::all(px(3)),
                    ..default()
                },
                BorderColor::all(Color::srgb(0.9, 0.9, 0.9)),
                BackgroundColor(Color::srgb(0.15, 0.15, 0.15)),
                children![(
                    LoadingBarFill,
                    Node {
                        width: percent(0),
                        height: percent(100),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.2, 0.8, 0.2)),
                )],
            ),
        ],
    ));
}

fn run_loading(
    mut gfx: ResMut<GraphicsCache>,
    mut sfx: ResMut<SoundCache>,
    mut label_q: Query<&mut Text, With<LoadingLabel>>,
    mut fill_q: Query<&mut Node, With<LoadingBarFill>>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    let Ok(mut label) = label_q.single_mut() else {
        return;
    };
    let Ok(mut fill) = fill_q.single_mut() else {
        return;
    };

    if !gfx.is_initialized() {
        **label = "LOADING GFX".to_string();
        match gfx.initialize() {
            CacheInitStatus::InProgress { progress } => {
                fill.width = percent((progress.clamp(0.0, 1.0)) * 100.0);
            }
            CacheInitStatus::Done => {
                fill.width = percent(100);
            }
            CacheInitStatus::Error(err) => {
                **label = "LOADING GFX (ERROR)".to_string();
                log::error!("GraphicsCache init failed: {err}");
                // Advance anyway so we don't soft-lock on the loading screen.
                fill.width = percent(100);
            }
        }
        return;
    }

    if !sfx.is_initialized() {
        **label = "LOADING SFX".to_string();
        match sfx.initialize() {
            CacheInitStatus::InProgress { progress } => {
                fill.width = percent((progress.clamp(0.0, 1.0)) * 100.0);
            }
            CacheInitStatus::Done => {
                fill.width = percent(100);
            }
            CacheInitStatus::Error(err) => {
                **label = "LOADING SFX (ERROR)".to_string();
                log::error!("SoundCache init failed: {err}");
                fill.width = percent(100);
            }
        }
        return;
    }

    next_state.set(GameState::Gameplay);
}

fn teardown_loading_ui(
    mut commands: Commands,
    q: Query<Entity, Or<(With<LoadingUi>, With<LoadingLabel>, With<LoadingBarFill>)>>,
) {
    for e in &q {
        commands.entity(e).despawn();
    }
}

fn setup_gameplay(mut commands: Commands, asset_server: Res<AssetServer>) {
    log::info!("Loading complete; entering gameplay");
    commands.spawn(Sprite::from_image(asset_server.load("gfx/00001.png")));
}
