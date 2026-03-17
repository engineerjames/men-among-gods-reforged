//! Purpose-built UI widget framework for the game client.
//!
//! Provides a lightweight [`Widget`] trait with nesting support and concrete
//! widgets: [`Panel`], [`Label`], [`RectButton`], [`CircleButton`], and
//! [`ChatBox`].  All rendering is done via SDL2 primitives and the existing
//! bitmap font system — no additional dependencies are required.

pub mod button;
pub mod button_arc;
pub mod cert_dialog;
pub mod character_creation_form;
pub mod character_selection_form;
pub mod chat_box;
pub mod checkbox;
pub mod delete_character_dialog;
pub mod dropdown;
pub mod inventory_panel;
pub mod label;
pub mod login_form;
pub mod look_panel;
pub mod minimap_widget;
pub mod mode_button;
pub mod new_account_form;
pub mod panel;
pub mod panning_background;
pub mod quit_confirm_dialog;
pub mod radio_group;
pub mod rank_arc;
pub mod scrollable_list;
pub mod settings_panel;
pub mod shop_panel;
pub mod skills_panel;
pub mod slider;
pub mod status_panel;
pub mod style;
pub mod text_input;
pub mod tls_warning_banner;
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
///
/// Two lifetimes avoid the `&'a mut T<'a>` invariance footgun:
/// * `'f` — frame/function borrow (how long this context reference lives).
/// * `'tc` — texture-creator lifetime (how long the GPU textures are valid).
pub struct RenderContext<'f, 'tc> {
    /// The SDL2 canvas for the current frame.
    pub canvas: &'f mut Canvas<Window>,
    /// The lazy-loading sprite cache (fonts, sprites, minimap texture).
    pub gfx: &'f mut GraphicsCache<'tc>,
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
