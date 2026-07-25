use sdl2::{event::Event, rect::Rect, video::Window};

/// Computes the letterboxed viewport for a logical coordinate space inside a
/// drawable area of the given size.
///
/// This is the single source of truth for how the logical 960x540 frame maps
/// onto physical pixels. Both the frame presentation blit (`present_rect`) and
/// the mouse-input mapping (`to_logical_coords` / `to_logical_rel`) are derived
/// from it, so the two can never drift apart.
///
/// When `pixel_perfect_scaling` is enabled an integer zoom factor is used and
/// the result is centered; otherwise aspect-preserving continuous letterboxing
/// is applied.
///
/// # Arguments
/// * `drawable_w` - Width of the drawable area in physical pixels.
/// * `drawable_h` - Height of the drawable area in physical pixels.
/// * `logical_width` - The width of the logical coordinate space (e.g. 960).
/// * `logical_height` - The height of the logical coordinate space (e.g. 540).
/// * `pixel_perfect_scaling` - Whether integer scaling is active.
///
/// # Returns
/// * `(view_x, view_y, view_w, view_h)` in drawable pixels.
pub fn present_viewport(
    drawable_w: u32,
    drawable_h: u32,
    logical_width: f32,
    logical_height: f32,
    pixel_perfect_scaling: bool,
) -> (f32, f32, f32, f32) {
    let ww = drawable_w as f32;
    let wh = drawable_h as f32;

    if ww <= 0.0 || wh <= 0.0 || logical_width <= 0.0 || logical_height <= 0.0 {
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

/// Returns the integer destination rectangle used to blit the composed frame
/// buffer onto the window backbuffer.
///
/// Rounds the floating-point viewport from [`present_viewport`] to whole
/// pixels, clamping the size to at least 1x1 so SDL never receives a degenerate
/// rectangle.
///
/// # Arguments
/// * `drawable_w` - Width of the drawable area in physical pixels.
/// * `drawable_h` - Height of the drawable area in physical pixels.
/// * `logical_width` - The width of the logical coordinate space (e.g. 960).
/// * `logical_height` - The height of the logical coordinate space (e.g. 540).
/// * `pixel_perfect_scaling` - Whether integer scaling is active.
///
/// # Returns
/// * The destination [`Rect`] in drawable pixels.
pub fn present_rect(
    drawable_w: u32,
    drawable_h: u32,
    logical_width: f32,
    logical_height: f32,
    pixel_perfect_scaling: bool,
) -> Rect {
    let (x, y, w, h) = present_viewport(
        drawable_w,
        drawable_h,
        logical_width,
        logical_height,
        pixel_perfect_scaling,
    );
    Rect::new(
        x.round() as i32,
        y.round() as i32,
        (w.round() as u32).max(1),
        (h.round() as u32).max(1),
    )
}

/// Computes the viewport rectangle used to map logical coordinates
/// into the current drawable area of `window`.
///
/// Thin wrapper over [`present_viewport`] that measures the window.
///
/// # Arguments
/// * `window` - The SDL2 window to measure.
/// * `logical_width` - The width of the logical coordinate space (e.g. 960).
/// * `logical_height` - The height of the logical coordinate space (e.g. 540).
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
    present_viewport(
        drawable_w,
        drawable_h,
        logical_width,
        logical_height,
        pixel_perfect_scaling,
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
