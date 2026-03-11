//! Purpose-built UI widget framework for the game client.
//!
//! Provides a lightweight [`Widget`] trait with nesting support and concrete
//! widgets: [`Panel`], [`Label`], [`RectButton`], [`CircleButton`], and
//! [`ChatBox`].  All rendering is done via SDL2 primitives and the existing
//! bitmap font system — no additional dependencies are required.

#[allow(dead_code)]
pub mod button;
#[allow(dead_code)]
pub mod chat_box;
#[allow(dead_code)]
pub mod hud_button_bar;
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
pub mod status_panel;
#[allow(dead_code)]
pub mod style;
#[allow(dead_code)]
pub mod widget;

use sdl2::{render::Canvas, video::Window};

use crate::gfx_cache::GraphicsCache;

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
