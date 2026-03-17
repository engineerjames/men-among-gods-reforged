//! Non-interactive TLS warning banner widget.
//!
//! Displays a semi-transparent amber banner at the top of the screen when the
//! game connection is unencrypted.

use std::time::Duration;

use sdl2::pixels::Color;
use sdl2::rect::Rect;
use sdl2::render::BlendMode;

use super::widget::{Bounds, EventResponse, UiEvent, Widget};
use super::RenderContext;
use crate::font_cache;

/// Bitmap font index.
const FONT: usize = 1;

/// A non-interactive banner warning that the network connection is unencrypted.
pub struct TlsWarningBanner {
    bounds: Bounds,
    visible: bool,
}

impl TlsWarningBanner {
    /// Creates a new TLS warning banner, centred horizontally at the top of
    /// the screen.
    ///
    /// # Returns
    ///
    /// A new `TlsWarningBanner`, initially hidden.
    pub fn new() -> Self {
        let text = "UNENCRYPTED - Game traffic is not protected";
        let text_w = font_cache::text_width(text);
        let pad_h: u32 = 8;
        let pad_v: u32 = 4;
        let w = text_w + pad_h * 2;
        let h = font_cache::BITMAP_GLYPH_H + pad_v * 2;
        let x = (crate::constants::TARGET_WIDTH_INT - w) as i32 / 2;
        let y = 4;
        Self {
            bounds: Bounds::new(x, y, w, h),
            visible: false,
        }
    }

    /// Sets whether the banner is visible.
    ///
    /// # Arguments
    ///
    /// * `visible` - `true` to show, `false` to hide.
    pub fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }
}

impl Widget for TlsWarningBanner {
    fn bounds(&self) -> &Bounds {
        &self.bounds
    }

    fn set_position(&mut self, x: i32, y: i32) {
        self.bounds.x = x;
        self.bounds.y = y;
    }

    fn handle_event(&mut self, _event: &UiEvent) -> EventResponse {
        // Non-interactive — never consumes events.
        EventResponse::Ignored
    }

    fn update(&mut self, _dt: Duration) {
        // No time-based state.
    }

    fn render(&mut self, ctx: &mut RenderContext<'_, '_>) -> Result<(), String> {
        if !self.visible {
            return Ok(());
        }

        ctx.canvas.set_blend_mode(BlendMode::Blend);

        let rect = Rect::new(
            self.bounds.x,
            self.bounds.y,
            self.bounds.width,
            self.bounds.height,
        );
        ctx.canvas.set_draw_color(Color::RGBA(40, 30, 0, 200));
        ctx.canvas.fill_rect(rect)?;

        let text = "UNENCRYPTED - Game traffic is not protected";
        let text_x =
            self.bounds.x + (self.bounds.width as i32 - font_cache::text_width(text) as i32) / 2;
        let text_y =
            self.bounds.y + (self.bounds.height as i32 - font_cache::BITMAP_GLYPH_H as i32) / 2;
        font_cache::draw_text(
            ctx.canvas,
            ctx.gfx,
            FONT,
            text,
            text_x,
            text_y,
            font_cache::TextStyle::tinted(Color::RGB(255, 255, 80)),
        )?;

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initially_hidden() {
        let banner = TlsWarningBanner::new();
        assert!(!banner.visible);
    }

    #[test]
    fn set_visible_toggles() {
        let mut banner = TlsWarningBanner::new();
        banner.set_visible(true);
        assert!(banner.visible);
        banner.set_visible(false);
        assert!(!banner.visible);
    }

    #[test]
    fn non_interactive() {
        let mut banner = TlsWarningBanner::new();
        let click = UiEvent::MouseClick {
            x: banner.bounds.x + 5,
            y: banner.bounds.y + 2,
            button: super::super::widget::MouseButton::Left,
            modifiers: super::super::widget::KeyModifiers::default(),
        };
        assert_eq!(banner.handle_event(&click), EventResponse::Ignored);
    }
}
