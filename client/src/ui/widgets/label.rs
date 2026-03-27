//! Non-interactive text label widget.

use sdl2::pixels::Color;

use crate::font_cache;

use crate::ui::RenderContext;
use crate::ui::widget::{Bounds, EventResponse, UiEvent, Widget};

/// A simple single-line text label rendered with the bitmap font.
pub struct Label {
    bounds: Bounds,
    text: String,
    font: usize,
    tint: Option<Color>,
}

impl Label {
    /// Creates a new label at the given position.
    ///
    /// Width is computed from the text length; height is the bitmap glyph
    /// height.
    ///
    /// # Arguments
    ///
    /// * `text` - The text to display.
    /// * `font` - Bitmap font index (0–3).
    /// * `x` - Left edge in pixels.
    /// * `y` - Top edge in pixels.
    ///
    /// # Returns
    ///
    /// A new `Label`.
    pub fn new(text: &str, font: usize, x: i32, y: i32) -> Self {
        Self {
            bounds: Bounds::new(
                x,
                y,
                font_cache::text_width(text),
                font_cache::BITMAP_GLYPH_H,
            ),
            text: text.to_owned(),
            font,
            tint: None,
        }
    }

    /// Creates a new label with an explicit color tint.
    ///
    /// # Arguments
    ///
    /// * `text` - The text to display.
    /// * `font` - Bitmap font index (0–3).
    /// * `x` - Left edge in pixels.
    /// * `y` - Top edge in pixels.
    /// * `color` - RGB color modulation applied to the font texture.
    ///
    /// # Returns
    ///
    /// A tinted `Label`.
    pub fn with_color(text: &str, font: usize, x: i32, y: i32, color: Color) -> Self {
        let mut label = Self::new(text, font, x, y);
        label.tint = Some(color);
        label
    }

    /// Replaces the displayed text.
    ///
    /// The bounding width is updated to match.
    ///
    /// # Arguments
    ///
    /// * `text` - New text string.
    pub fn set_text(&mut self, text: &str) {
        self.text = text.to_owned();
        self.bounds.width = font_cache::text_width(text);
    }
}

impl Widget for Label {
    fn bounds(&self) -> &Bounds {
        &self.bounds
    }

    fn set_position(&mut self, x: i32, y: i32) {
        self.bounds.x = x;
        self.bounds.y = y;
    }

    fn handle_event(&mut self, _event: &UiEvent) -> EventResponse {
        EventResponse::Ignored
    }

    fn render(&mut self, ctx: &mut RenderContext<'_, '_>) -> Result<(), String> {
        match self.tint {
            Some(color) => font_cache::draw_text(
                ctx.canvas,
                ctx.gfx,
                self.font,
                &self.text,
                self.bounds.x,
                self.bounds.y,
                font_cache::TextStyle::tinted(color),
            ),
            None => font_cache::draw_text(
                ctx.canvas,
                ctx.gfx,
                self.font,
                &self.text,
                self.bounds.x,
                self.bounds.y,
                font_cache::TextStyle::PLAIN,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn label_bounds_match_text_width() {
        let label = Label::new("Hello", 1, 10, 20);
        assert_eq!(label.bounds.width, font_cache::text_width("Hello"));
        assert_eq!(label.bounds.height, font_cache::BITMAP_GLYPH_H);
    }

    #[test]
    fn set_text_updates_width() {
        let mut label = Label::new("Hi", 1, 0, 0);
        let old_w = label.bounds.width;
        label.set_text("Hello World");
        assert!(label.bounds.width > old_w);
        assert_eq!(label.bounds.width, font_cache::text_width("Hello World"));
    }

    #[test]
    fn label_always_ignores_events() {
        let mut label = Label::new("test", 0, 0, 0);
        let resp = label.handle_event(&UiEvent::MouseClick {
            x: 0,
            y: 0,
            button: crate::ui::widget::MouseButton::Left,
            modifiers: crate::ui::widget::KeyModifiers::default(),
        });
        assert_eq!(resp, EventResponse::Ignored);
    }
}
