//! Purpose-built UI widget framework for the game client.
//!
//! Provides a lightweight [`Widget`] trait with nesting support and concrete
//! widgets: [`Panel`], [`Label`], [`RectButton`], [`CircleButton`], and
//! [`ChatBox`].  All rendering is done via SDL2 primitives and the existing
//! bitmap font system — no additional dependencies are required.

#[allow(dead_code)]
pub mod button;
#[allow(dead_code)]
pub mod button_arc;
#[allow(dead_code)]
pub mod chat_box;
#[allow(dead_code)]
pub mod checkbox;
#[allow(dead_code)]
pub mod dropdown;
#[allow(dead_code)]
pub mod inventory_panel;
#[allow(dead_code)]
pub mod label;
#[allow(dead_code)]
pub mod look_panel;
#[allow(dead_code)]
pub mod minimap_widget;
#[allow(dead_code)]
pub mod mode_button;
#[allow(dead_code)]
pub mod panel;
#[allow(dead_code)]
pub mod rank_arc;
#[allow(dead_code)]
pub mod settings_panel;
#[allow(dead_code)]
pub mod shop_panel;
#[allow(dead_code)]
pub mod skills_panel;
#[allow(dead_code)]
pub mod slider;
#[allow(dead_code)]
pub mod status_panel;
#[allow(dead_code)]
pub mod style;
#[allow(dead_code)]
pub mod widget;

use sdl2::{event::Event, mouse::MouseButton, render::Canvas, video::Window};

use crate::{
    gfx_cache::GraphicsCache,
    ui::widget::{KeyModifiers, MouseButton as UiMouseButton, UiEvent},
};

/// Mutable rendering context passed to [`Widget::render`].
///
/// Bundles the SDL2 canvas and the sprite/texture cache so that widgets can
/// draw without needing direct access to `AppState`.
pub struct RenderContext<'a> {
    /// The SDL2 canvas for the current frame.
    pub canvas: &'a mut Canvas<Window>,
    /// The lazy-loading sprite cache (fonts, sprites, minimap texture).
    pub gfx: &'a mut GraphicsCache,
}

/// Translate an SDL2 event into a UI-framework `UiEvent`, if applicable.
///
/// # Arguments
///
/// * `event` - The SDL2 event.
/// * `mouse_x` - Current logical mouse X position.
/// * `mouse_y` - Current logical mouse Y position.
/// * `modifiers` - Current modifier key state.
///
/// # Returns
///
/// `Some(UiEvent)` for events the widget system cares about, `None` otherwise.
pub fn sdl_to_ui_event(
    event: &Event,
    mouse_x: i32,
    mouse_y: i32,
    modifiers: KeyModifiers,
) -> Option<UiEvent> {
    match event {
        Event::MouseWheel { y, .. } => Some(UiEvent::MouseWheel {
            x: mouse_x,
            y: mouse_y,
            delta: *y,
        }),
        Event::MouseButtonDown {
            mouse_btn, x, y, ..
        } => {
            let button = match mouse_btn {
                MouseButton::Left => UiMouseButton::Left,
                MouseButton::Right => UiMouseButton::Right,
                MouseButton::Middle => UiMouseButton::Middle,
                _ => return None,
            };
            Some(UiEvent::MouseDown {
                x: *x,
                y: *y,
                button,
                modifiers,
            })
        }
        Event::MouseButtonUp {
            mouse_btn, x, y, ..
        } => {
            let button = match mouse_btn {
                MouseButton::Left => UiMouseButton::Left,
                MouseButton::Right => UiMouseButton::Right,
                MouseButton::Middle => UiMouseButton::Middle,
                _ => return None,
            };
            Some(UiEvent::MouseClick {
                x: *x,
                y: *y,
                button,
                modifiers,
            })
        }
        Event::TextInput { text, .. } => Some(UiEvent::TextInput { text: text.clone() }),
        Event::KeyDown {
            keycode: Some(kc),
            keymod,
            ..
        } => Some(UiEvent::KeyDown {
            keycode: *kc,
            modifiers: KeyModifiers::from_sdl2(*keymod),
        }),
        Event::MouseMotion { x, y, .. } => Some(UiEvent::MouseMove { x: *x, y: *y }),
        _ => None,
    }
}
