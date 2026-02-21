use sdl2::{event::Event, video::Window};

const LOGICAL_W: f32 = 800.0;
const LOGICAL_H: f32 = 600.0;

fn logical_viewport(window: &Window) -> (f32, f32, f32, f32) {
    let (window_w, window_h) = window.size();
    let ww = window_w as f32;
    let wh = window_h as f32;

    if ww <= 0.0 || wh <= 0.0 {
        return (0.0, 0.0, LOGICAL_W, LOGICAL_H);
    }

    let target_aspect = LOGICAL_W / LOGICAL_H;
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

fn to_logical_coords(x: i32, y: i32, window: &Window) -> (i32, i32) {
    let (view_x, view_y, view_w, view_h) = logical_viewport(window);
    if view_w <= 0.0 || view_h <= 0.0 {
        return (x, y);
    }

    let lx = ((x as f32 - view_x) * LOGICAL_W / view_w).round() as i32;
    let ly = ((y as f32 - view_y) * LOGICAL_H / view_h).round() as i32;
    (lx, ly)
}

fn to_logical_rel(dx: i32, dy: i32, window: &Window) -> (i32, i32) {
    let (_, _, view_w, view_h) = logical_viewport(window);
    if view_w <= 0.0 || view_h <= 0.0 {
        return (dx, dy);
    }

    let ldx = ((dx as f32) * LOGICAL_W / view_w).round() as i32;
    let ldy = ((dy as f32) * LOGICAL_H / view_h).round() as i32;
    (ldx, ldy)
}

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

fn scale_coord(value: i32, scale: f32) -> i32 {
    ((value as f32) * scale).round() as i32
}

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

pub fn adjust_mouse_event_for_hidpi(event: Event, window: &Window) -> Event {
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
            let (x, y) = to_logical_coords(x, y, window);
            let (xrel, yrel) = to_logical_rel(xrel, yrel, window);
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
            let (x, y) = to_logical_coords(x, y, window);
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
            let (x, y) = to_logical_coords(x, y, window);
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
