use sdl2::{event::Event, video::Window};

use crate::preferences::UpscaleMode;

/// Computes an aspect-preserving viewport for a drawable area.
///
/// Pixel-perfect mode uses an integer scale factor and centers the result.
/// Crisp and Smooth both use continuous aspect-preserving letterboxing.
///
/// # Arguments
///
/// * `drawable_width` - Width of the drawable output in physical pixels.
/// * `drawable_height` - Height of the drawable output in physical pixels.
/// * `logical_width` - Width of the logical coordinate space.
/// * `logical_height` - Height of the logical coordinate space.
/// * `upscale_mode` - Final-scene scaling mode.
///
/// # Returns
///
/// * `(view_x, view_y, view_w, view_h)` in drawable pixels.
pub fn calculate_logical_viewport(
    drawable_width: u32,
    drawable_height: u32,
    logical_width: f32,
    logical_height: f32,
    upscale_mode: UpscaleMode,
) -> (f32, f32, f32, f32) {
    let ww = drawable_width as f32;
    let wh = drawable_height as f32;

    if ww <= 0.0 || wh <= 0.0 {
        return (0.0, 0.0, logical_width, logical_height);
    }

    if upscale_mode.uses_integer_scale() {
        let scale = (ww / logical_width)
            .floor()
            .min((wh / logical_height).floor())
            .max(1.0);
        let view_w = logical_width * scale;
        let view_h = logical_height * scale;
        let view_x = (ww - view_w) * 0.5;
        let view_y = (wh - view_h) * 0.5;
        return (view_x, view_y, view_w, view_h);
    }

    let target_aspect = logical_width / logical_height;
    let window_aspect = ww / wh;

    if window_aspect > target_aspect {
        let view_h = wh;
        let view_w = view_h * target_aspect;
        let view_x = (ww - view_w) * 0.5;
        (view_x, 0.0, view_w, view_h)
    } else {
        let view_w = ww;
        let view_h = view_w / target_aspect;
        let view_y = (wh - view_h) * 0.5;
        (0.0, view_y, view_w, view_h)
    }
}

/// Computes the viewport rectangle used to map logical coordinates
/// into the current drawable area.
///
/// Pixel-perfect mode emulates SDL integer scaling by using an integer zoom
/// factor and centering the result. Other modes use aspect-preserving
/// continuous letterboxing.
///
/// # Arguments
/// * `window` - The SDL2 window to measure.
/// * `logical_width` - The width of the logical coordinate space (e.g. 1920).
/// * `logical_height` - The height of the logical coordinate space (e.g. 1080).
/// * `upscale_mode` - Final-scene scaling mode.
///
/// # Returns
/// * `(view_x, view_y, view_w, view_h)` in drawable pixels.
pub fn logical_viewport(
    window: &Window,
    logical_width: f32,
    logical_height: f32,
    upscale_mode: UpscaleMode,
) -> (f32, f32, f32, f32) {
    let (drawable_w, drawable_h) = window.drawable_size();
    calculate_logical_viewport(
        drawable_w,
        drawable_h,
        logical_width,
        logical_height,
        upscale_mode,
    )
}

/// Converts a physical screen coordinate pair to logical (1920×1080) coordinates,
/// accounting for letterboxing.
///
/// # Arguments
/// * `x` - Physical X coordinate.
/// * `y` - Physical Y coordinate.
/// * `window` - The SDL2 window for viewport calculation.
/// * `logical_width` - The width of the logical coordinate space (e.g. 1920).
/// * `logical_height` - The height of the logical coordinate space (e.g. 1080).
/// * `upscale_mode` - Final-scene scaling mode.
///
/// # Returns
/// * `(logical_x, logical_y)` in the 1920×1080 coordinate space.
fn to_logical_coords(
    x: i32,
    y: i32,
    window: &Window,
    logical_width: f32,
    logical_height: f32,
    upscale_mode: UpscaleMode,
) -> (i32, i32) {
    let (scale_x, scale_y) = hidpi_scale(window);
    let x_draw = x as f32 * scale_x;
    let y_draw = y as f32 * scale_y;

    let (view_x, view_y, view_w, view_h) =
        logical_viewport(window, logical_width, logical_height, upscale_mode);
    if view_w <= 0.0 || view_h <= 0.0 {
        return (x, y);
    }

    let lx = ((x_draw - view_x) * logical_width / view_w).round() as i32;
    let ly = ((y_draw - view_y) * logical_height / view_h).round() as i32;
    (lx, ly)
}

/// Converts a relative (delta) motion from physical to logical coordinates.
///
/// # Arguments
/// * `dx` - Physical X delta.
/// * `dy` - Physical Y delta.
/// * `window` - The SDL2 window for viewport calculation.
/// * `logical_width` - The width of the logical coordinate space (e.g. 1920).
/// * `logical_height` - The height of the logical coordinate space (e.g. 1080).
/// * `upscale_mode` - Final-scene scaling mode.
///
/// # Returns
/// * `(logical_dx, logical_dy)` in the logical coordinate space.
fn to_logical_rel(
    dx: i32,
    dy: i32,
    window: &Window,
    logical_width: f32,
    logical_height: f32,
    upscale_mode: UpscaleMode,
) -> (i32, i32) {
    let (scale_x, scale_y) = hidpi_scale(window);
    let dx_draw = dx as f32 * scale_x;
    let dy_draw = dy as f32 * scale_y;

    let (_, _, view_w, view_h) =
        logical_viewport(window, logical_width, logical_height, upscale_mode);
    if view_w <= 0.0 || view_h <= 0.0 {
        return (dx, dy);
    }

    let ldx = (dx_draw * logical_width / view_w).round() as i32;
    let ldy = (dy_draw * logical_height / view_h).round() as i32;
    (ldx, ldy)
}

/// Returns the ratio of drawable size to window size on each axis.
///
/// On Retina / HiDPI displays this is typically `(2.0, 2.0)`; on standard
/// displays it is `(1.0, 1.0)`.
///
/// # Arguments
/// * `window` - The SDL2 window to query.
///
/// # Returns
/// * `(scale_x, scale_y)`.
fn hidpi_scale(window: &Window) -> (f32, f32) {
    let (window_w, window_h) = window.size();
    let (drawable_w, drawable_h) = window.drawable_size();
    let scale_x = if window_w > 0 {
        drawable_w as f32 / window_w as f32
    } else {
        1.0
    };
    let scale_y = if window_h > 0 {
        drawable_h as f32 / window_h as f32
    } else {
        1.0
    };
    (scale_x, scale_y)
}

/// Re-maps mouse event coordinates from physical window space to the 1920×1080
/// logical coordinate space used by the game renderer.
///
/// # Arguments
/// * `event` - The original SDL2 mouse event (consumed).
/// * `window` - The SDL2 window for viewport calculation.
/// * `logical_width` - The width of the logical coordinate space (e.g. 1920).
/// * `logical_height` - The height of the logical coordinate space (e.g. 1080).
/// * `upscale_mode` - Final-scene scaling mode.
///
/// # Returns
/// * A new `Event` with coordinates in logical space.
pub fn adjust_mouse_event_for_hidpi(
    event: Event,
    window: &Window,
    logical_width: f32,
    logical_height: f32,
    upscale_mode: UpscaleMode,
) -> Event {
    match event {
        Event::MouseMotion {
            timestamp,
            window_id,
            which,
            mousestate,
            x,
            y,
            xrel,
            yrel,
        } => {
            let (x, y) =
                to_logical_coords(x, y, window, logical_width, logical_height, upscale_mode);
            let (xrel, yrel) = to_logical_rel(
                xrel,
                yrel,
                window,
                logical_width,
                logical_height,
                upscale_mode,
            );
            Event::MouseMotion {
                timestamp,
                window_id,
                which,
                mousestate,
                x,
                y,
                xrel,
                yrel,
            }
        }
        Event::MouseButtonDown {
            timestamp,
            window_id,
            which,
            mouse_btn,
            clicks,
            x,
            y,
        } => {
            let (x, y) =
                to_logical_coords(x, y, window, logical_width, logical_height, upscale_mode);
            Event::MouseButtonDown {
                timestamp,
                window_id,
                which,
                mouse_btn,
                clicks,
                x,
                y,
            }
        }
        Event::MouseButtonUp {
            timestamp,
            window_id,
            which,
            mouse_btn,
            clicks,
            x,
            y,
        } => {
            let (x, y) =
                to_logical_coords(x, y, window, logical_width, logical_height, upscale_mode);
            Event::MouseButtonUp {
                timestamp,
                window_id,
                which,
                mouse_btn,
                clicks,
                x,
                y,
            }
        }
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_viewport_close(actual: (f32, f32, f32, f32), expected: (f32, f32, f32, f32)) {
        let epsilon = 0.01;
        assert!((actual.0 - expected.0).abs() < epsilon, "x: {actual:?}");
        assert!((actual.1 - expected.1).abs() < epsilon, "y: {actual:?}");
        assert!((actual.2 - expected.2).abs() < epsilon, "w: {actual:?}");
        assert!((actual.3 - expected.3).abs() < epsilon, "h: {actual:?}");
    }

    #[test]
    fn pixel_perfect_uses_integer_scale_and_centers() {
        let viewport =
            calculate_logical_viewport(1280, 800, 960.0, 540.0, UpscaleMode::PixelPerfect);

        assert_viewport_close(viewport, (160.0, 130.0, 960.0, 540.0));
    }

    #[test]
    fn crisp_uses_continuous_letterbox_for_steam_deck_shape() {
        let viewport = calculate_logical_viewport(1280, 800, 960.0, 540.0, UpscaleMode::Crisp);

        assert_viewport_close(viewport, (0.0, 40.0, 1280.0, 720.0));
    }

    #[test]
    fn smooth_uses_same_viewport_as_crisp() {
        let crisp = calculate_logical_viewport(1280, 800, 960.0, 540.0, UpscaleMode::Crisp);
        let smooth = calculate_logical_viewport(1280, 800, 960.0, 540.0, UpscaleMode::Smooth);

        assert_viewport_close(smooth, crisp);
    }

    #[test]
    fn ultrawide_drawable_letterboxes_horizontally() {
        let viewport = calculate_logical_viewport(2560, 1080, 960.0, 540.0, UpscaleMode::Crisp);

        assert_viewport_close(viewport, (320.0, 0.0, 1920.0, 1080.0));
    }
}
