use std::time::{Duration, Instant};

use bevy::prelude::*;

use crate::gfx_cache::{CacheInitStatus, GraphicsCache};
use crate::helpers::despawn_tree;
use crate::sfx_cache::SoundCache;
use crate::GameState;

#[derive(Component)]
pub struct LoadingUi;

#[derive(Component)]
pub struct LoadingLabel;

#[derive(Component)]
pub struct LoadingBarFill;

/// Spawns the loading screen UI and resets asset cache initialization state.
pub fn setup_loading_ui(
    mut commands: Commands,
    mut gfx: ResMut<GraphicsCache>,
    mut sfx: ResMut<SoundCache>,
) {
    log::debug!("setup_loading_ui - start");
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
                Text::new("Loading graphical assets..."),
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
    log::debug!("setup_loading_ui - end");
}

/// Advances graphics/sound cache initialization while keeping the UI responsive.
///
/// This system processes initialization work for a small time budget per frame and updates the
/// progress bar. Once both caches are initialized, it transitions to `GameState::LoggingIn`.
pub fn run_loading(
    mut gfx: ResMut<GraphicsCache>,
    mut sfx: ResMut<SoundCache>,
    mut images: ResMut<Assets<Image>>,
    mut audio_sources: ResMut<Assets<AudioSource>>,
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

    // Doing only 1 "unit of work" per frame makes loading time proportional to
    // (number of assets) / (fps). `images.zip` currently has ~14k entries, so at 60fps
    // that's ~230 seconds (!) just to iterate them.
    //
    // Instead, process for a small time budget each frame to keep the UI responsive.
    const LOADING_BUDGET: Duration = Duration::from_millis(250);

    if !gfx.is_initialized() {
        **label = "Loading graphical assets...".to_string();
        let start = Instant::now();
        let mut last_progress;
        loop {
            match gfx.initialize(&mut images) {
                CacheInitStatus::InProgress { progress } => {
                    last_progress = progress;
                    if start.elapsed() >= LOADING_BUDGET {
                        break;
                    }
                }
                CacheInitStatus::Done => {
                    last_progress = 1.0;
                    break;
                }
                CacheInitStatus::Error(err) => {
                    **label = "Loading graphical assets (error)".to_string();
                    log::error!("GraphicsCache init failed: {err}");
                    // Advance anyway so we don't soft-lock on the loading screen.
                    last_progress = 1.0;
                    break;
                }
            }
        }

        fill.width = percent((last_progress.clamp(0.0, 1.0)) * 100.0);
        return;
    }

    if !sfx.is_initialized() {
        **label = "Loading sound assets...".to_string();
        let start = Instant::now();
        let mut last_progress;
        loop {
            match sfx.initialize(audio_sources.as_mut()) {
                CacheInitStatus::InProgress { progress } => {
                    last_progress = progress;
                    if start.elapsed() >= LOADING_BUDGET {
                        break;
                    }
                }
                CacheInitStatus::Done => {
                    last_progress = 1.0;
                    break;
                }
                CacheInitStatus::Error(err) => {
                    **label = "Loading sound assets (error)".to_string();
                    log::error!("SoundCache init failed: {err}");
                    last_progress = 1.0;
                    break;
                }
            }
        }

        fill.width = percent((last_progress.clamp(0.0, 1.0)) * 100.0);
        return;
    }

    next_state.set(GameState::LoggingIn);
}

/// Despawns the loading UI tree.
pub fn teardown_loading_ui(
    mut commands: Commands,
    roots: Query<Entity, With<LoadingUi>>,
    children_q: Query<&Children>,
) {
    for root in &roots {
        despawn_tree(root, &children_q, &mut commands);
    }
}
