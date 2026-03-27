//! Compact HUD panel showing weapon and armor values.
//!
//! Positioned to the right of the skill bar near the bottom of the viewport.
//! The rank sigil and HP/End/Mana bars have been moved to [`super::rank_sigil`]
//! and the in-world vital bars respectively.

use sdl2::pixels::Color;
use sdl2::render::BlendMode;

use crate::font_cache;

use super::RenderContext;
use super::widget::{Bounds, EventResponse, UiEvent, Widget};

// ---------------------------------------------------------------------------
// Layout constants
// ---------------------------------------------------------------------------

/// Padding inside the panel background on each side.
const PANEL_PADDING: i32 = 4;
/// Width of the text content area (fits "WV: 9999  AV: 9999").
const PANEL_WIDTH: i32 = 34;
/// Bitmap font index used for value text (yellow font).
const FONT: usize = 1;

// ---------------------------------------------------------------------------
// Widget
// ---------------------------------------------------------------------------

/// A compact panel displaying the player's weapon and armor values.
///
/// Renders a single `"WV: xx  AV: xx"` text line on a semi-transparent
/// background. The panel is read-only; clicks within its bounds are consumed
/// to prevent world interaction from passing through.
pub struct StatusPanel {
    /// Bounding rectangle of the panel.
    bounds: Bounds,
    /// Semi-transparent background fill colour.
    bg_color: Color,
    /// Current weapon value.
    weapon: i32,
    /// Current armor value.
    armor: i32,
}

impl StatusPanel {
    /// Create a new `StatusPanel` positioned at `(x, y)`.
    ///
    /// # Arguments
    ///
    /// * `x` - Left edge in logical pixels.
    /// * `y` - Top edge in logical pixels.
    /// * `bg_color` - Semi-transparent background fill.
    ///
    /// # Returns
    ///
    /// * A new `StatusPanel` with zeroed weapon and armor values.
    pub fn new(x: i32, y: i32, bg_color: Color) -> Self {
        let w = (PANEL_PADDING * 2 + PANEL_WIDTH) as u32;
        let h = (PANEL_PADDING * 2 + font_cache::BITMAP_GLYPH_H as i32 * 2) as u32;
        Self {
            bounds: Bounds::new(x, y, w, h),
            bg_color,
            weapon: 0,
            armor: 0,
        }
    }

    /// Push the latest weapon and armor values into the widget.
    ///
    /// Must be called once per frame before [`render`] so that the displayed
    /// values are up-to-date.
    ///
    /// # Arguments
    ///
    /// * `weapon` - Current weapon value.
    /// * `armor`  - Current armor value.
    ///
    /// [`render`]: Self::render
    pub fn sync(&mut self, weapon: i32, armor: i32) {
        self.weapon = weapon;
        self.armor = armor;
    }
}

impl Widget for StatusPanel {
    /// Returns the bounding rectangle of the panel.
    fn bounds(&self) -> &Bounds {
        &self.bounds
    }

    /// Moves the panel's top-left corner.
    ///
    /// # Arguments
    ///
    /// * `x` - New left edge.
    /// * `y` - New top edge.
    fn set_position(&mut self, x: i32, y: i32) {
        self.bounds.x = x;
        self.bounds.y = y;
    }

    /// Handle input events.
    ///
    /// Clicks within the panel bounds are consumed to prevent world
    /// interaction. All other events are ignored.
    ///
    /// # Arguments
    ///
    /// * `event` - The translated UI event.
    ///
    /// # Returns
    ///
    /// * `Consumed` if a click landed inside the panel, `Ignored` otherwise.
    fn handle_event(&mut self, event: &UiEvent) -> EventResponse {
        if let UiEvent::MouseClick { x, y, .. } = event {
            if self.bounds.contains_point(*x, *y) {
                return EventResponse::Consumed;
            }
        }
        EventResponse::Ignored
    }

    /// Draw the status panel.
    ///
    /// # Arguments
    ///
    /// * `ctx` - Mutable render context (canvas + graphics cache).
    ///
    /// # Returns
    ///
    /// * `Ok(())` on success, or an SDL2 error string.
    fn render(&mut self, ctx: &mut RenderContext<'_, '_>) -> Result<(), String> {
        ctx.canvas.set_blend_mode(BlendMode::Blend);
        ctx.canvas.set_draw_color(self.bg_color);
        ctx.canvas.fill_rect(sdl2::rect::Rect::new(
            self.bounds.x,
            self.bounds.y,
            self.bounds.width,
            self.bounds.height,
        ))?;

        let wv_text = format!("WV: {}", self.weapon);
        let av_text = format!("AV: {}", self.armor);
        let text_x = self.bounds.x + PANEL_PADDING;
        let text_y = self.bounds.y + PANEL_PADDING;
        font_cache::draw_text(
            ctx.canvas,
            ctx.gfx,
            FONT,
            &wv_text,
            text_x,
            text_y,
            font_cache::TextStyle::PLAIN,
        )?;

        font_cache::draw_text(
            ctx.canvas,
            ctx.gfx,
            FONT,
            &av_text,
            text_x, // Adjust spacing as needed
            text_y + font_cache::BITMAP_GLYPH_H as i32 + 2,
            font_cache::TextStyle::PLAIN,
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
    fn new_initialises_with_zeroed_values() {
        let panel = StatusPanel::new(4, 4, Color::RGBA(10, 10, 30, 180));
        assert_eq!(panel.weapon, 0);
        assert_eq!(panel.armor, 0);
    }

    #[test]
    fn sync_updates_weapon_and_armor() {
        let mut panel = StatusPanel::new(4, 4, Color::RGBA(10, 10, 30, 180));
        panel.sync(42, 17);
        assert_eq!(panel.weapon, 42);
        assert_eq!(panel.armor, 17);
    }

    #[test]
    fn click_inside_is_consumed() {
        let mut panel = StatusPanel::new(4, 4, Color::RGBA(10, 10, 30, 180));
        let click = UiEvent::MouseClick {
            x: 20,
            y: 8,
            button: super::super::widget::MouseButton::Left,
            modifiers: super::super::widget::KeyModifiers::default(),
        };
        assert_eq!(panel.handle_event(&click), EventResponse::Consumed);
    }

    #[test]
    fn click_outside_is_ignored() {
        let mut panel = StatusPanel::new(4, 4, Color::RGBA(10, 10, 30, 180));
        let click = UiEvent::MouseClick {
            x: 500,
            y: 500,
            button: super::super::widget::MouseButton::Left,
            modifiers: super::super::widget::KeyModifiers::default(),
        };
        assert_eq!(panel.handle_event(&click), EventResponse::Ignored);
    }

    #[test]
    fn bounds_have_expected_dimensions() {
        let panel = StatusPanel::new(0, 0, Color::RGBA(10, 10, 30, 180));
        let w = (PANEL_PADDING * 2 + PANEL_WIDTH) as u32;
        let h = (PANEL_PADDING * 2 + font_cache::BITMAP_GLYPH_H as i32 * 2) as u32;
        assert_eq!(panel.bounds().width, w);
        assert_eq!(panel.bounds().height, h);
    }
}
