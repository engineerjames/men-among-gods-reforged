//! Compact HUD panel showing weapon and armor values.
//!
//! Positioned to the right of the skill bar near the bottom of the viewport.
//! The rank sigil and HP/End/Mana bars have been moved to [`super::rank_sigil`]
//! and the in-world vital bars respectively.

use sdl2::pixels::Color;
use sdl2::render::BlendMode;

use crate::font_cache;

use crate::ui::RenderContext;
use crate::ui::widget::{Bounds, EventResponse, UiEvent, Widget};

// ---------------------------------------------------------------------------
// Layout constants
// ---------------------------------------------------------------------------

/// Padding inside the panel background on each side.
const PANEL_PADDING: i32 = 4;
/// Shared label-column width for status text.
const STATUS_LABEL_WIDTH: usize = "Weapon:".len();
/// Minimum right-aligned number-column width for status text.
const MIN_STATUS_VALUE_WIDTH: usize = 3;
/// Bitmap font index used for value text (yellow font).
const FONT: usize = 1;

/// Formats one status row with a fixed label column and right-aligned value.
///
/// # Arguments
///
/// * `label` - Label shown before the numeric value.
/// * `value` - Numeric value to display.
/// * `value_width` - Width of the numeric column.
///
/// # Returns
///
/// * A formatted status row with deterministic alignment.
fn format_status_line(label: &str, value: i32, value_width: usize) -> String {
    format!(
        "{:<label_width$} {:>value_width$}",
        label,
        value,
        label_width = STATUS_LABEL_WIDTH,
        value_width = value_width
    )
}

/// Returns the value-column width needed for the current status values.
///
/// # Arguments
///
/// * `weapon` - Current weapon value.
/// * `armor` - Current armor value.
///
/// # Returns
///
/// * The shared right-aligned number-column width.
fn status_value_width(weapon: i32, armor: i32) -> usize {
    MIN_STATUS_VALUE_WIDTH
        .max(weapon.to_string().len())
        .max(armor.to_string().len())
}

/// Returns the panel width needed for a status row with `value_width` digits.
///
/// # Arguments
///
/// * `value_width` - Width of the numeric column.
///
/// # Returns
///
/// * Width in logical pixels, including panel padding.
fn panel_width_for_value_width(value_width: usize) -> u32 {
    let chars = STATUS_LABEL_WIDTH + 1 + value_width;
    (PANEL_PADDING * 2) as u32 + chars as u32 * font_cache::BITMAP_GLYPH_ADVANCE
}

// ---------------------------------------------------------------------------
// Widget
// ---------------------------------------------------------------------------

/// A compact panel displaying the player's weapon and armor values.
///
/// Renders two aligned text rows on a semi-transparent background. The panel is
/// read-only; clicks within its bounds are consumed to prevent world
/// interaction from passing through.
pub struct WeaponArmorPanel {
    /// Bounding rectangle of the panel.
    bounds: Bounds,
    /// Semi-transparent background fill color.
    bg_color: Color,
    /// Current weapon value.
    weapon: i32,
    /// Current armor value.
    armor: i32,
}

impl WeaponArmorPanel {
    /// Create a new `WeaponArmorPanel` positioned at `(x, y)`.
    ///
    /// # Arguments
    ///
    /// * `x` - Left edge in logical pixels.
    /// * `y` - Top edge in logical pixels.
    /// * `bg_color` - Semi-transparent background fill.
    ///
    /// # Returns
    ///
    /// * A new `WeaponArmorPanel` with zeroed weapon and armor values.
    pub fn new(x: i32, y: i32, bg_color: Color) -> Self {
        let w = panel_width_for_value_width(MIN_STATUS_VALUE_WIDTH);
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
        self.bounds.width = panel_width_for_value_width(status_value_width(weapon, armor));
    }

    /// Returns the formatted weapon and armor lines for rendering.
    ///
    /// # Returns
    ///
    /// * A pair of equal-length status strings.
    fn formatted_lines(&self) -> (String, String) {
        let value_width = status_value_width(self.weapon, self.armor);
        (
            format_status_line("Weapon:", self.weapon, value_width),
            format_status_line("Armor:", self.armor, value_width),
        )
    }
}

impl Widget for WeaponArmorPanel {
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
        if let UiEvent::MouseClick { x, y, .. } = event
            && self.bounds.contains_point(*x, *y)
        {
            return EventResponse::Consumed;
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

        let (wv_text, av_text) = self.formatted_lines();
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
            text_x,
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
        let panel = WeaponArmorPanel::new(4, 4, Color::RGBA(10, 10, 30, 180));
        assert_eq!(panel.weapon, 0);
        assert_eq!(panel.armor, 0);
    }

    #[test]
    fn sync_updates_weapon_and_armor() {
        let mut panel = WeaponArmorPanel::new(4, 4, Color::RGBA(10, 10, 30, 180));
        panel.sync(42, 17);
        assert_eq!(panel.weapon, 42);
        assert_eq!(panel.armor, 17);
    }

    #[test]
    fn click_inside_is_consumed() {
        let mut panel = WeaponArmorPanel::new(4, 4, Color::RGBA(10, 10, 30, 180));
        let click = UiEvent::MouseClick {
            x: 20,
            y: 8,
            button: crate::ui::widget::MouseButton::Left,
            modifiers: crate::ui::widget::KeyModifiers::default(),
        };
        assert_eq!(panel.handle_event(&click), EventResponse::Consumed);
    }

    #[test]
    fn click_outside_is_ignored() {
        let mut panel = WeaponArmorPanel::new(4, 4, Color::RGBA(10, 10, 30, 180));
        let click = UiEvent::MouseClick {
            x: 500,
            y: 500,
            button: crate::ui::widget::MouseButton::Left,
            modifiers: crate::ui::widget::KeyModifiers::default(),
        };
        assert_eq!(panel.handle_event(&click), EventResponse::Ignored);
    }

    #[test]
    fn bounds_have_expected_dimensions() {
        let panel = WeaponArmorPanel::new(0, 0, Color::RGBA(10, 10, 30, 180));
        let w = panel_width_for_value_width(MIN_STATUS_VALUE_WIDTH);
        let h = (PANEL_PADDING * 2 + font_cache::BITMAP_GLYPH_H as i32 * 2) as u32;
        assert_eq!(panel.bounds().width, w);
        assert_eq!(panel.bounds().height, h);
    }

    #[test]
    fn formatted_lines_align_values_and_lengths() {
        let mut panel = WeaponArmorPanel::new(0, 0, Color::RGBA(10, 10, 30, 180));
        panel.sync(42, 1700);

        let (weapon, armor) = panel.formatted_lines();

        assert_eq!(weapon, "Weapon:   42");
        assert_eq!(armor, "Armor:  1700");
        assert_eq!(weapon.len(), armor.len());
    }

    #[test]
    fn sync_expands_bounds_for_wider_values() {
        let mut panel = WeaponArmorPanel::new(0, 0, Color::RGBA(10, 10, 30, 180));
        let original_width = panel.bounds().width;

        panel.sync(123_456, 7);

        assert!(panel.bounds().width > original_width);
        assert_eq!(panel.bounds().width, panel_width_for_value_width(6));
    }
}
