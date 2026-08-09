use sdl2::{event::Event, video::Window};

/// Computes the window size that exactly contains the largest integer-scaled
/// logical viewport fitting inside the current drawable area.
///
/// # Arguments
///
/// * `window_size` - Current window size in OS window coordinates.
/// * `drawable_size` - Current renderer drawable size in physical pixels.
/// * `logical_size` - Game rendering size in logical pixels.
///
/// # Returns
///
/// * The smaller window size that removes unused letterbox space, or `None`
///   when any supplied dimension is zero.
pub fn fitted_pixel_perfect_window_size(
    window_size: (u32, u32),
    drawable_size: (u32, u32),
    logical_size: (u32, u32),
) -> Option<(u32, u32)> {
    let (window_width, window_height) = window_size;
    let (drawable_width, drawable_height) = drawable_size;
    let (logical_width, logical_height) = logical_size;
    if window_width == 0
        || window_height == 0
        || drawable_width == 0
        || drawable_height == 0
        || logical_width == 0
        || logical_height == 0
    {
        return None;
    }

    let integer_scale = (drawable_width / logical_width)
        .min(drawable_height / logical_height)
        .max(1);
    let viewport_width = logical_width.saturating_mul(integer_scale);
    let viewport_height = logical_height.saturating_mul(integer_scale);
    let fitted_width = (u64::from(viewport_width) * u64::from(window_width)
        + u64::from(drawable_width) / 2)
        / u64::from(drawable_width);
    let fitted_height = (u64::from(viewport_height) * u64::from(window_height)
        + u64::from(drawable_height) / 2)
        / u64::from(drawable_height);

    Some((fitted_width as u32, fitted_height as u32))
}

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
    use super::fitted_pixel_perfect_window_size;

    #[test]
    fn fitted_size_removes_standard_dpi_letterboxing() {
        assert_eq!(
            fitted_pixel_perfect_window_size((2_500, 1_440), (2_500, 1_440), (960, 540)),
            Some((1_920, 1_080))
        );
    }

    #[test]
    fn fitted_size_accounts_for_retina_drawable_pixels() {
        assert_eq!(
            fitted_pixel_perfect_window_size((1_500, 900), (3_000, 1_800), (960, 540)),
            Some((1_440, 810))
        );
    }

    #[test]
    fn fitted_size_accounts_for_fractional_dpi_scaling() {
        assert_eq!(
            fitted_pixel_perfect_window_size((1_600, 900), (2_400, 1_350), (960, 540)),
            Some((1_280, 720))
        );
    }

    #[test]
    fn fitted_size_rejects_zero_dimensions() {
        assert_eq!(
            fitted_pixel_perfect_window_size((0, 540), (0, 540), (960, 540)),
            None
        );
    }
}
