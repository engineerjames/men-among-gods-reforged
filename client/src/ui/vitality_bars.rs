//! Player vitality bar HUD overlay (HP, Endurance, Mana).
//!
//! Renders three horizontally-stacked 32×2 px bars directly onto the canvas.
//! Each bar has a dark background track so missing values remain visible even
//! when the player is at zero HP/End/Mana.
//!
//! The position is stored in the public [`x`] and [`y`] fields so it can be
//! repositioned at any time without recreating the widget.
//!
//! [`x`]: VitalityBars::x
//! [`y`]: VitalityBars::y

use sdl2::pixels::Color;
use sdl2::render::BlendMode;

use super::RenderContext;

// ---------------------------------------------------------------------------
// Layout constants
// ---------------------------------------------------------------------------

/// Width of each bar in pixels.
const BAR_W: i32 = 32;

/// Height of each bar in pixels.
const BAR_H: u32 = 2;

/// Vertical distance between the top of adjacent bar rows (bar height + 1 px gap).
const BAR_SPACING: i32 = 3;

/// Dark background track drawn behind each bar to show the empty portion.
const TRACK_COLOR: Color = Color::RGB(30, 30, 30);

/// HP bar fill colour.
const HP_COLOR: Color = Color::RGB(180, 30, 30);

/// Endurance bar fill colour.
const END_COLOR: Color = Color::RGB(200, 180, 40);

/// Mana bar fill colour.
const MANA_COLOR: Color = Color::RGB(40, 80, 200);

// ---------------------------------------------------------------------------
// VitalityBars
// ---------------------------------------------------------------------------

/// HUD overlay that renders the player's HP, Endurance, and Mana bars.
///
/// Each bar is [`BAR_W`]×[`BAR_H`] pixels with a dark [`TRACK_COLOR`]
/// background so missing values are always visible.  Modify the public
/// [`x`] and [`y`] fields to reposition the widget without recreating it.
///
/// [`x`]: VitalityBars::x
/// [`y`]: VitalityBars::y
pub struct VitalityBars {
    /// Left edge of the bar group in logical pixels.
    pub x: i32,
    /// Top edge of the HP bar in logical pixels.
    pub y: i32,
    /// HP fill fraction in `[0.0, 1.0]`.
    hp_fill: f32,
    /// Endurance fill fraction in `[0.0, 1.0]`.
    end_fill: f32,
    /// Mana fill fraction in `[0.0, 1.0]`.
    mana_fill: f32,
}

impl VitalityBars {
    /// Create a new `VitalityBars` widget with its top-left at `(x, y)`.
    ///
    /// All bars start fully filled (1.0) until [`sync`] is called with real
    /// character stats.
    ///
    /// # Arguments
    ///
    /// * `x` - Left edge of the bar group in logical pixels.
    /// * `y` - Top edge of the HP bar in logical pixels.
    ///
    /// # Returns
    ///
    /// * A new `VitalityBars` with all bars at 100 % fill.
    ///
    /// [`sync`]: Self::sync
    pub fn new(x: i32, y: i32) -> Self {
        Self {
            x,
            y,
            hp_fill: 1.0,
            end_fill: 1.0,
            mana_fill: 1.0,
        }
    }

    /// Update the displayed fill fractions from raw current and maximum stat values.
    ///
    /// Negative current values are clamped to zero.  When `max_*` is zero the
    /// corresponding bar fill is set to zero (bar is not drawn).
    ///
    /// # Arguments
    ///
    /// * `a_hp` - Current HP.
    /// * `max_hp` - Maximum HP; bar fill becomes 0 if this is zero.
    /// * `a_end` - Current Endurance.
    /// * `max_end` - Maximum Endurance.
    /// * `a_mana` - Current Mana.
    /// * `max_mana` - Maximum Mana.
    pub fn sync(
        &mut self,
        a_hp: i32,
        max_hp: i32,
        a_end: i32,
        max_end: i32,
        a_mana: i32,
        max_mana: i32,
    ) {
        self.hp_fill = if max_hp > 0 {
            (a_hp.max(0) as f32 / max_hp as f32).clamp(0.0, 1.0)
        } else {
            0.0
        };
        self.end_fill = if max_end > 0 {
            (a_end.max(0) as f32 / max_end as f32).clamp(0.0, 1.0)
        } else {
            0.0
        };
        self.mana_fill = if max_mana > 0 {
            (a_mana.max(0) as f32 / max_mana as f32).clamp(0.0, 1.0)
        } else {
            0.0
        };
    }

    /// Draw the three vital bars onto the canvas.
    ///
    /// Renders a full-width dark background track for each bar first, then
    /// overlays the coloured fill proportional to the stored fraction.
    /// Uses `BlendMode::None` so bars are always fully opaque.
    ///
    /// # Arguments
    ///
    /// * `ctx` - Mutable render context (canvas + graphics cache).
    ///
    /// # Returns
    ///
    /// * `Ok(())` on success, or an SDL2 error string.
    pub fn render(&mut self, ctx: &mut RenderContext<'_, '_>) -> Result<(), String> {
        let canvas = &mut ctx.canvas;
        canvas.set_blend_mode(BlendMode::None);

        let bars = [
            (self.hp_fill, HP_COLOR),
            (self.end_fill, END_COLOR),
            (self.mana_fill, MANA_COLOR),
        ];

        for (i, (fill, color)) in bars.iter().enumerate() {
            let bar_y = self.y + (i as i32) * BAR_SPACING;
            let filled = (fill * BAR_W as f32) as u32;

            canvas.set_draw_color(TRACK_COLOR);
            canvas.fill_rect(sdl2::rect::Rect::new(self.x, bar_y, BAR_W as u32, BAR_H))?;

            if filled > 0 {
                canvas.set_draw_color(*color);
                canvas.fill_rect(sdl2::rect::Rect::new(self.x, bar_y, filled, BAR_H))?;
            }
        }

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
    fn new_defaults_to_full_fill() {
        let bars = VitalityBars::new(100, 200);
        assert_eq!(bars.x, 100);
        assert_eq!(bars.y, 200);
        assert_eq!(bars.hp_fill, 1.0);
        assert_eq!(bars.end_fill, 1.0);
        assert_eq!(bars.mana_fill, 1.0);
    }

    #[test]
    fn sync_clamps_fill_to_zero_when_max_is_zero() {
        let mut bars = VitalityBars::new(0, 0);
        bars.sync(100, 0, 100, 0, 100, 0);
        assert_eq!(bars.hp_fill, 0.0);
        assert_eq!(bars.end_fill, 0.0);
        assert_eq!(bars.mana_fill, 0.0);
    }

    #[test]
    fn sync_clamps_negative_current_to_zero() {
        let mut bars = VitalityBars::new(0, 0);
        bars.sync(-10, 100, -5, 100, -1, 100);
        assert_eq!(bars.hp_fill, 0.0);
        assert_eq!(bars.end_fill, 0.0);
        assert_eq!(bars.mana_fill, 0.0);
    }

    #[test]
    fn sync_sets_half_fill() {
        let mut bars = VitalityBars::new(0, 0);
        bars.sync(50, 100, 50, 100, 50, 100);
        assert!((bars.hp_fill - 0.5).abs() < f32::EPSILON);
        assert!((bars.end_fill - 0.5).abs() < f32::EPSILON);
        assert!((bars.mana_fill - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn sync_does_not_exceed_one() {
        let mut bars = VitalityBars::new(0, 0);
        bars.sync(200, 100, 200, 100, 200, 100);
        assert_eq!(bars.hp_fill, 1.0);
        assert_eq!(bars.end_fill, 1.0);
        assert_eq!(bars.mana_fill, 1.0);
    }
}
