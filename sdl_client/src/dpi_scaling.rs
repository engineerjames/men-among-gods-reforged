use sdl2::{event::Event, video::Window};

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

pub fn adjust_mouse_event_for_hidpi(event: Event, window: &Window) -> Event {
    let (scale_x, scale_y) = hidpi_scale(window);
    if (scale_x - 1.0).abs() < f32::EPSILON && (scale_y - 1.0).abs() < f32::EPSILON {
        return event;
    }

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
