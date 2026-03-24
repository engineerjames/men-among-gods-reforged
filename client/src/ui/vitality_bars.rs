//! Player vitality chevron HUD overlay (HP, Endurance, Mana).
//!
//! Renders three nested upward-pointing chevron (^ caret) shapes directly onto
//! the canvas.  HP is the outermost (widest / tallest), Endurance is the
//! middle, and Mana is the innermost (narrowest / shortest).  Each layer
//! maintains the same arm slope angle, so inner chevrons are proportionally
//! shorter — their tips are staggered below the HP tip while all layers share
//! the same bottom (feet) y coordinate.
//!
//! Each chevron has a dark background track so missing values remain visible.
//! Fill direction is **left-to-right**: at 50 % the left arm is fully coloured
//! and the right arm is dark track.
//!
//! The position is stored in the public [`x`] and [`y`] fields:
//! - [`x`]: horizontal centre of the chevron group (tip x).
//! - [`y`]: bottom of the chevron group (feet y).
//!
//! [`x`]: VitalityBars::x
//! [`y`]: VitalityBars::y

use sdl2::pixels::Color;
use sdl2::render::BlendMode;

use super::RenderContext;

// ---------------------------------------------------------------------------
// Layout constants
// ---------------------------------------------------------------------------

/// Half-width of the outermost (HP) chevron at its bottom feet.
/// The total horizontal span is `2 * HP_HALF_W` (32 px).
const HP_HALF_W: i32 = 16;

/// Vertical height of the outermost (HP) chevron from its bottom feet to its
/// tip.  Inner layers scale this proportionally:
/// `h = CHEVRON_H * hw / HP_HALF_W`.
const CHEVRON_H: i32 = 10;

/// Horizontal inset per layer.  Each successive inner chevron is this many
/// pixels narrower per side at the bottom feet (END hw = 12, MANA hw = 8).
const LAYER_INSET: i32 = 4;

/// Thickness of each chevron arm in pixels.
const ARM_THICKNESS: i32 = 2;

/// Dark background track drawn behind each chevron to show the empty portion.
const TRACK_COLOR: Color = Color::RGB(30, 30, 30);

/// HP chevron fill colour.
const HP_COLOR: Color = Color::RGB(180, 30, 30);

/// Endurance chevron fill colour.
const END_COLOR: Color = Color::RGB(200, 180, 40);

/// Mana chevron fill colour.
const MANA_COLOR: Color = Color::RGB(40, 80, 200);

// ---------------------------------------------------------------------------
// VitalityBars
// ---------------------------------------------------------------------------

/// HUD overlay that renders the player's HP, Endurance, and Mana as three
/// nested upward-pointing chevron (^ caret) shapes.
///
/// Each chevron has a dark [`TRACK_COLOR`] background so missing values are
/// always visible.  Fill sweeps left-to-right.  Modify the public [`x`] and
/// [`y`] fields to reposition the widget without recreating it.
///
/// [`x`]: VitalityBars::x
/// [`y`]: VitalityBars::y
pub struct VitalityBars {
    /// Horizontal centre of the chevron group in logical pixels.
    pub x: i32,
    /// Bottom of the chevron group (feet y) in logical pixels.
    pub y: i32,
    /// HP fill fraction in `[0.0, 1.0]`.
    hp_fill: f32,
    /// Endurance fill fraction in `[0.0, 1.0]`.
    end_fill: f32,
    /// Mana fill fraction in `[0.0, 1.0]`.
    mana_fill: f32,
}

impl VitalityBars {
    /// Create a new `VitalityBars` widget centred at `x` with its bottom
    /// feet at `y`.
    ///
    /// All chevrons start fully filled (1.0) until [`sync`] is called with
    /// real character stats.
    ///
    /// # Arguments
    ///
    /// * `x` - Horizontal centre of the chevron group.
    /// * `y` - Bottom of the chevron group (feet y).
    ///
    /// # Returns
    ///
    /// * A new `VitalityBars` with all chevrons at 100 % fill.
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
    /// corresponding chevron fill is set to zero.
    ///
    /// # Arguments
    ///
    /// * `a_hp` - Current HP.
    /// * `max_hp` - Maximum HP; fill becomes 0 if this is zero.
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

    /// Draw a single upward-pointing chevron with left-to-right fill.
    ///
    /// The chevron tip is at `(cx, bot_y - height)` and the two feet are at
    /// `(cx ± half_w, bot_y)`.  Pixels whose x coordinate falls within the
    /// filled portion (measured left-to-right across the full chevron span)
    /// are drawn in `color`; the rest are drawn in [`TRACK_COLOR`].
    ///
    /// # Arguments
    ///
    /// * `canvas` - SDL2 canvas to draw on.
    /// * `cx` - Horizontal centre (tip and midpoint of feet).
    /// * `bot_y` - Bottom y coordinate (feet y).
    /// * `half_w` - Half-width of the chevron at the feet.
    /// * `height` - Vertical span from feet to tip.
    /// * `fill` - Fill fraction in `[0.0, 1.0]` (left-to-right).
    /// * `color` - Fill colour for the filled portion.
    fn draw_chevron(
        canvas: &mut sdl2::render::Canvas<sdl2::video::Window>,
        cx: i32,
        bot_y: i32,
        half_w: i32,
        height: i32,
        fill: f32,
        color: Color,
    ) -> Result<(), String> {
        if height <= 0 || half_w <= 0 {
            return Ok(());
        }

        // Full pixel span including arm thickness on each side.
        let leftmost = cx - half_w - (ARM_THICKNESS - 1);
        let rightmost = cx + half_w + (ARM_THICKNESS - 1);
        let pixel_span = (rightmost - leftmost + 1) as f32;
        let fill_x = leftmost as f32 + fill * pixel_span;

        // Walk scanlines from tip (row 0) down to feet (row height-1).
        for row in 0..height {
            let y = bot_y - height + row;
            // t = 0 at tip, 1 at feet.
            let t = row as f32 / (height - 1).max(1) as f32;
            // Horizontal offset from centre: 0 at tip, half_w at feet.
            let offset = (t * half_w as f32).round() as i32;

            // Left arm pixels (extend leftward from arm centre).
            for k in 0..ARM_THICKNESS {
                let px = cx - offset - k;
                let c = if (px as f32) < fill_x {
                    color
                } else {
                    TRACK_COLOR
                };
                canvas.set_draw_color(c);
                canvas.draw_point(sdl2::rect::Point::new(px, y))?;
            }
            // Right arm pixels (extend rightward from arm centre).
            for k in 0..ARM_THICKNESS {
                let px = cx + offset + k;
                let c = if (px as f32) < fill_x {
                    color
                } else {
                    TRACK_COLOR
                };
                canvas.set_draw_color(c);
                canvas.draw_point(sdl2::rect::Point::new(px, y))?;
            }
        }

        Ok(())
    }

    /// Draw the three nested chevrons onto the canvas.
    ///
    /// HP is outermost (widest / tallest), Endurance is middle, Mana is
    /// innermost (narrowest / shortest).  Each layer's height scales
    /// proportionally with its half-width so that the arm slope is consistent
    /// across all three.  All layers share the same feet y; tips are staggered.
    /// Uses `BlendMode::None` so chevrons are always fully opaque.
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

        let layers: [(f32, Color, i32); 3] = [
            (self.hp_fill, HP_COLOR, 0),
            (self.end_fill, END_COLOR, LAYER_INSET),
            (self.mana_fill, MANA_COLOR, LAYER_INSET * 2),
        ];

        for (fill, color, inset) in layers {
            let hw = HP_HALF_W - inset;
            if hw <= 0 {
                continue;
            }
            // Height scales proportionally so the arm slope stays the same.
            let h = CHEVRON_H * hw / HP_HALF_W;
            Self::draw_chevron(canvas, self.x, self.y, hw, h, fill, color)?;
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

    #[test]
    fn position_is_center_and_bottom() {
        let bars = VitalityBars::new(480, 270);
        // x is horizontal centre (tip x), y is bottom (feet y)
        assert_eq!(bars.x, 480);
        assert_eq!(bars.y, 270);
    }
}
