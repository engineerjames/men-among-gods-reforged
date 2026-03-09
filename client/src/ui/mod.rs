//! Purpose-built UI widget framework for the game client.
//!
//! Provides a lightweight [`Widget`] trait with nesting support and concrete
//! widgets: [`Panel`], [`Label`], [`RectButton`], [`CircleButton`], and
//! [`ChatBox`].  All rendering is done via SDL2 primitives and the existing
//! bitmap font system — no additional dependencies are required.

pub mod button;
pub mod chat_box;
pub mod label;
pub mod panel;
pub mod style;
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
