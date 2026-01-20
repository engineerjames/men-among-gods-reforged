use bevy::camera::Viewport;
use bevy::prelude::*;

use bevy::window::{PrimaryWindow, WindowResized};

use crate::constants::{TARGET_HEIGHT, TARGET_WIDTH};

/// Enforce 4:3 viewport letterboxing and fixed 800x600 pixel coordinates.
pub fn enforce_aspect_and_pixel_coords(
    mut window_resized: MessageReader<WindowResized>,
    mut initialized: Local<bool>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut cameras: Query<(&mut Camera, &mut Projection), With<Camera2d>>,
) {
    let mut should_update = !*initialized;
    for _ in window_resized.read() {
        should_update = true;
    }

    if !should_update {
        return;
    }
    *initialized = true;

    let Ok(window) = windows.single() else {
        return;
    };

    let window_w = window.resolution.physical_width();
    let window_h = window.resolution.physical_height();
    if window_w == 0 || window_h == 0 {
        return;
    }

    let target_aspect = TARGET_WIDTH / TARGET_HEIGHT;
    let window_aspect = window_w as f32 / window_h as f32;

    // Fit a 4:3 viewport inside the window (letterbox or pillarbox).
    let (viewport_w, viewport_h) = if window_aspect > target_aspect {
        // Window is wider than target: clamp by height.
        let vh = window_h;
        let vw = (window_h as f32 * target_aspect).round() as u32;
        (vw.min(window_w), vh)
    } else {
        // Window is taller than target: clamp by width.
        let vw = window_w;
        let vh = (window_w as f32 / target_aspect).round() as u32;
        (vw, vh.min(window_h))
    };

    let viewport_x = (window_w - viewport_w) / 2;
    let viewport_y = (window_h - viewport_h) / 2;

    for (mut camera, mut projection) in &mut cameras {
        camera.viewport = Some(Viewport {
            physical_position: UVec2::new(viewport_x, viewport_y),
            physical_size: UVec2::new(viewport_w, viewport_h),
            depth: 0.0..1.0,
        });

        // Keep the world view fixed at 800x600 world units (pixel coordinates).
        if let Projection::Orthographic(ortho) = &mut *projection {
            ortho.scaling_mode = bevy::camera::ScalingMode::Fixed {
                width: TARGET_WIDTH,
                height: TARGET_HEIGHT,
            };
            ortho.scale = 1.0;
        }
    }
}
