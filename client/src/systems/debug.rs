use bevy::prelude::*;

use bevy::window::PrimaryWindow;

use crate::{
    constants::{TARGET_HEIGHT, TARGET_WIDTH},
    GameState,
};

pub fn print_click_coords(
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cameras: Query<&Camera, With<Camera2d>>,
) {
    if !mouse.just_pressed(MouseButton::Left) {
        return;
    }

    let Ok(window) = windows.single() else {
        error!("click: no primary window found");
        return;
    };

    let Some(cursor_logical) = window.cursor_position() else {
        info!("click: cursor not in window");
        return;
    };

    let scale_factor = window.resolution.scale_factor();
    let cursor_physical = cursor_logical * scale_factor;

    let mut extra = String::new();
    if let Ok(camera) = cameras.single() {
        let (vp_pos, vp_size) = if let Some(viewport) = camera.viewport.as_ref() {
            (
                Vec2::new(
                    viewport.physical_position.x as f32,
                    viewport.physical_position.y as f32,
                ),
                Vec2::new(
                    viewport.physical_size.x as f32,
                    viewport.physical_size.y as f32,
                ),
            )
        } else {
            (
                Vec2::ZERO,
                Vec2::new(
                    window.resolution.physical_width() as f32,
                    window.resolution.physical_height() as f32,
                ),
            )
        };

        if vp_size.x > 0.0 && vp_size.y > 0.0 {
            let vp_max = vp_pos + vp_size;
            let in_viewport = cursor_physical - vp_pos;

            // Only compute game coords if the click is inside the render viewport.
            if cursor_physical.x >= vp_pos.x
                && cursor_physical.x < vp_max.x
                && cursor_physical.y >= vp_pos.y
                && cursor_physical.y < vp_max.y
            {
                let game = Vec2::new(
                    in_viewport.x / vp_size.x * TARGET_WIDTH,
                    in_viewport.y / vp_size.y * TARGET_HEIGHT,
                );

                extra = format!(
                    ", viewport_px=({:.1},{:.1}), game_800x600=({:.1},{:.1})",
                    in_viewport.x, in_viewport.y, game.x, game.y
                );
            } else {
                extra = ", click in letterbox/pillarbox".to_string();
            }
        }
    }

    info!(
        "click: logical=({:.1},{:.1}), physical=({:.1},{:.1}){}",
        cursor_logical.x, cursor_logical.y, cursor_physical.x, cursor_physical.y, extra
    );
}

pub fn run_on_any_transition(mut transitions: MessageReader<StateTransitionEvent<GameState>>) {
    for ev in transitions.read() {
        log::info!(
            "State Transition Detected! From {:?} to {:?}",
            ev.exited,
            ev.entered
        );
    }
}
