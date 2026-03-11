use sdl2::{event::Event, video::Window};

/// Computes the viewport rectangle used to map logical coordinates
/// into the current drawable area.
///
/// When `pixel_perfect_scaling` is enabled, this emulates SDL integer scaling
/// by using an integer zoom factor and centering the result. Otherwise it uses
/// aspect-preserving continuous letterboxing.
///
/// # Arguments
/// * `window` - The SDL2 window to measure.
/// * `logical_width` - The width of the logical coordinate space (e.g. 1920).
/// * `logical_height` - The height of the logical coordinate space (e.g. 1080).
/// * `pixel_perfect_scaling` - Whether integer scaling is active.
///
/// # Returns
/// * `(view_x, view_y, view_w, view_h)` in drawable pixels.
fn logical_viewport(
    window: &Window,
    logical_width: f32,
    logical_height: f32,
    pixel_perfect_scaling: bool,
) -> (f32, f32, f32, f32) {
    let (drawable_w, drawable_h) = window.drawable_size();
    let ww = drawable_w as f32;
    let wh = drawable_h as f32;

    if ww <= 0.0 || wh <= 0.0 {
        return (0.0, 0.0, logical_width, logical_height);
    }

    if pixel_perfect_scaling {
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

/// Converts a physical screen coordinate pair to logical (1920×1080) coordinates,
/// accounting for letterboxing.
///
/// # Arguments
/// * `x` - Physical X coordinate.
/// * `y` - Physical Y coordinate.
/// * `window` - The SDL2 window for viewport calculation.
/// * `logical_width` - The width of the logical coordinate space (e.g. 1920).
/// * `logical_height` - The height of the logical coordinate space (e.g. 1080).
/// * `pixel_perfect_scaling` - Whether integer scaling is active.
///
/// # Returns
/// * `(logical_x, logical_y)` in the 1920×1080 coordinate space.
fn to_logical_coords(
    x: i32,
    y: i32,
    window: &Window,
    logical_width: f32,
    logical_height: f32,
    pixel_perfect_scaling: bool,
) -> (i32, i32) {
    let (scale_x, scale_y) = hidpi_scale(window);
    let x_draw = x as f32 * scale_x;
    let y_draw = y as f32 * scale_y;

    let (view_x, view_y, view_w, view_h) =
        logical_viewport(window, logical_width, logical_height, pixel_perfect_scaling);
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
/// * `pixel_perfect_scaling` - Whether integer scaling is active.
///
/// # Returns
/// * `(logical_dx, logical_dy)` in the logical coordinate space.
fn to_logical_rel(
    dx: i32,
    dy: i32,
    window: &Window,
    logical_width: f32,
    logical_height: f32,
    pixel_perfect_scaling: bool,
) -> (i32, i32) {
    let (scale_x, scale_y) = hidpi_scale(window);
    let dx_draw = dx as f32 * scale_x;
    let dy_draw = dy as f32 * scale_y;

    let (_, _, view_w, view_h) =
        logical_viewport(window, logical_width, logical_height, pixel_perfect_scaling);
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

/// Scales an integer coordinate by a floating-point factor, rounding to the
/// nearest integer.
///
/// # Arguments
/// * `value` - The coordinate value to scale.
/// * `scale` - The multiplier.
///
/// # Returns
/// * The scaled value as `i32`.
fn scale_coord(value: i32, scale: f32) -> i32 {
    ((value as f32) * scale).round() as i32
}

/// Re-maps mouse event coordinates for egui on HiDPI displays.
///
/// egui expects coordinates in physical (drawable) pixels, so this multiplies
/// the SDL2 window-space coordinates by the HiDPI scale factor.
///
/// # Arguments
/// * `event` - The original SDL2 mouse event.
/// * `window` - The SDL2 window for scale calculation.
///
/// # Returns
/// * A new `Event` with scaled coordinates.
pub fn adjust_mouse_event_for_egui_hidpi(event: &Event, window: &Window) -> Event {
    let (scale_x, scale_y) = hidpi_scale(window);

    match event.clone() {
        Event::MouseMotion {
            timestamp,
            window_id,
            which,
            mousestate,
            x,
            y,
            xrel,
            yrel,
        } => Event::MouseMotion {
            timestamp,
            window_id,
            which,
            mousestate,
            x: scale_coord(x, scale_x),
            y: scale_coord(y, scale_y),
            xrel: scale_coord(xrel, scale_x),
            yrel: scale_coord(yrel, scale_y),
        },
        Event::MouseButtonDown {
            timestamp,
            window_id,
            which,
            mouse_btn,
            clicks,
            x,
            y,
        } => Event::MouseButtonDown {
            timestamp,
            window_id,
            which,
            mouse_btn,
            clicks,
            x: scale_coord(x, scale_x),
            y: scale_coord(y, scale_y),
        },
        Event::MouseButtonUp {
            timestamp,
            window_id,
            which,
            mouse_btn,
            clicks,
            x,
            y,
        } => Event::MouseButtonUp {
            timestamp,
            window_id,
            which,
            mouse_btn,
            clicks,
            x: scale_coord(x, scale_x),
            y: scale_coord(y, scale_y),
        },
        other => other,
    }
}

/// Re-maps mouse event coordinates from physical window space to the 1920×1080
/// logical coordinate space used by the game renderer.
///
/// # Arguments
/// * `event` - The original SDL2 mouse event (consumed).
/// * `window` - The SDL2 window for viewport calculation.
/// * `logical_width` - The width of the logical coordinate space (e.g. 1920).
/// * `logical_height` - The height of the logical coordinate space (e.g. 1080).
/// * `pixel_perfect_scaling` - Whether integer scaling is active.
///
/// # Returns
/// * A new `Event` with coordinates in logical space.
pub fn adjust_mouse_event_for_hidpi(
    event: Event,
    window: &Window,
    logical_width: f32,
    logical_height: f32,
    pixel_perfect_scaling: bool,
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
            let (x, y) = to_logical_coords(
                x,
                y,
                window,
                logical_width,
                logical_height,
                pixel_perfect_scaling,
            );
            let (xrel, yrel) = to_logical_rel(
                xrel,
                yrel,
                window,
                logical_width,
                logical_height,
                pixel_perfect_scaling,
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
            let (x, y) = to_logical_coords(
                x,
                y,
                window,
                logical_width,
                logical_height,
                pixel_perfect_scaling,
            );
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
            let (x, y) = to_logical_coords(
                x,
                y,
                window,
                logical_width,
                logical_height,
                pixel_perfect_scaling,
            );
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

    #[test]
    fn scale_coord_identity() {
        assert_eq!(scale_coord(100, 1.0), 100);
    }

    #[test]
    fn scale_coord_double() {
        assert_eq!(scale_coord(100, 2.0), 200);
    }

    #[test]
    fn scale_coord_half_rounds() {
        // 3 * 0.5 = 1.5, rounds to 2
        assert_eq!(scale_coord(3, 0.5), 2);
    }

    #[test]
    fn scale_coord_zero() {
        assert_eq!(scale_coord(0, 5.0), 0);
    }

    #[test]
    fn scale_coord_negative() {
        assert_eq!(scale_coord(-10, 2.0), -20);
    }
}
