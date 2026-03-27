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

use crate::ui::RenderContext;
use crate::ui::widget::{EventResponse, UiEvent};

// ---------------------------------------------------------------------------
// Layout constants
// ---------------------------------------------------------------------------

/// Half-width of the outermost (HP) chevron at its bottom feet.
/// The total horizontal span is `2 * HP_HALF_W` pixels.
/// Must be large enough for all three layers: `>= ARM_THICKNESS * 3 + LAYER_GAP * 2`
/// so the innermost chevron still has meaningful width.
const HP_HALF_W: i32 = 40;

/// Vertical height of the outermost (HP) chevron from its bottom feet to its
/// tip.  Inner layers scale this proportionally:
/// `h = round(CHEVRON_H * hw / HP_HALF_W)`.
const CHEVRON_H: i32 = 26;

/// Minimum gap in pixels between the outer edge of one chevron arm and the
/// outer edge of the next inner chevron arm.  The actual per-layer inset used
/// at render time is `ARM_THICKNESS + LAYER_GAP` so the layers never overlap
/// regardless of arm thickness.
const LAYER_GAP: i32 = 0;

/// Thickness of each chevron arm in pixels.
const ARM_THICKNESS: i32 = 8;

/// Dark background track drawn behind each chevron to show the empty portion.
const TRACK_COLOR: Color = Color::RGB(30, 30, 30);

/// HP chevron fill colour.
const HP_COLOR: Color = Color::RGB(180, 30, 30);

/// Endurance chevron fill colour.
const END_COLOR: Color = Color::RGB(200, 180, 40);

/// Mana chevron fill colour.
const MANA_COLOR: Color = Color::RGB(40, 80, 200);

/// Identifies which vitality chevron is currently hovered.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum HoveredChevron {
    Hp,
    Endurance,
    Mana,
}

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
    /// Current HP value displayed in the helper text.
    hp_current: i32,
    /// Maximum HP value displayed in the helper text.
    hp_max: i32,
    /// Current endurance value displayed in the helper text.
    end_current: i32,
    /// Maximum endurance value displayed in the helper text.
    end_max: i32,
    /// Current mana value displayed in the helper text.
    mana_current: i32,
    /// Maximum mana value displayed in the helper text.
    mana_max: i32,
    /// Chevron currently under the cursor, if any.
    hovered: Option<HoveredChevron>,
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
            hp_current: 0,
            hp_max: 0,
            end_current: 0,
            end_max: 0,
            mana_current: 0,
            mana_max: 0,
            hovered: None,
        }
    }

    /// Returns the horizontal inset between adjacent chevron layers.
    fn layer_inset() -> i32 {
        ARM_THICKNESS + LAYER_GAP
    }

    /// Computes the height for a chevron layer with the given half-width.
    fn chevron_height(half_w: i32) -> i32 {
        ((CHEVRON_H * half_w + HP_HALF_W / 2) / HP_HALF_W).max(1)
    }

    /// Returns `true` when the point lies inside the rasterized chevron band.
    fn point_in_chevron(px: i32, py: i32, cx: i32, bot_y: i32, half_w: i32, height: i32) -> bool {
        if height <= 0 || half_w <= 0 {
            return false;
        }

        let top_y = bot_y - height;
        if py < top_y || py >= bot_y {
            return false;
        }

        let row = py - top_y;
        let outer_offset = if height <= 1 {
            0
        } else {
            (row * half_w + (height - 1) / 2) / (height - 1)
        };
        let outer_left = cx - outer_offset;
        let outer_right = cx + outer_offset;
        if px < outer_left || px > outer_right {
            return false;
        }

        let inner_half_w = (half_w - ARM_THICKNESS).max(0);
        let inner_height = if inner_half_w > 0 {
            ((height * inner_half_w + half_w / 2) / half_w).max(1)
        } else {
            0
        };
        if inner_height == 0 {
            return true;
        }

        let inner_start_row = height - inner_height;
        if row < inner_start_row {
            return true;
        }

        let inner_row = row - inner_start_row;
        let inner_offset = if inner_height <= 1 {
            0
        } else {
            (inner_row * inner_half_w + (inner_height - 1) / 2) / (inner_height - 1)
        };
        let inner_left = cx - inner_offset;
        let inner_right = cx + inner_offset;
        px < inner_left || px > inner_right
    }

    /// Returns the hovered chevron at the given mouse position, prioritising
    /// inner layers first because they are drawn on top of outer layers.
    fn hovered_at(&self, x: i32, y: i32) -> Option<HoveredChevron> {
        let layer_inset = Self::layer_inset();
        let mana_half_w = HP_HALF_W - layer_inset * 2;
        if mana_half_w > 0
            && Self::point_in_chevron(
                x,
                y,
                self.x,
                self.y,
                mana_half_w,
                Self::chevron_height(mana_half_w),
            )
        {
            return Some(HoveredChevron::Mana);
        }

        let end_half_w = HP_HALF_W - layer_inset;
        if end_half_w > 0
            && Self::point_in_chevron(
                x,
                y,
                self.x,
                self.y,
                end_half_w,
                Self::chevron_height(end_half_w),
            )
        {
            return Some(HoveredChevron::Endurance);
        }

        if Self::point_in_chevron(
            x,
            y,
            self.x,
            self.y,
            HP_HALF_W,
            Self::chevron_height(HP_HALF_W),
        ) {
            return Some(HoveredChevron::Hp);
        }

        None
    }

    /// Returns `true` when the point lies within any chevron band.
    pub fn contains_point(&self, x: i32, y: i32) -> bool {
        self.hovered_at(x, y).is_some()
    }

    /// Updates hover state from a translated UI event.
    pub fn handle_event(&mut self, event: &UiEvent) -> EventResponse {
        if let UiEvent::MouseMove { x, y } = event {
            self.hovered = self.hovered_at(*x, *y);
        }
        EventResponse::Ignored
    }

    /// Returns helper text for the hovered chevron, if any.
    pub fn hover_text(&self) -> Option<String> {
        match self.hovered? {
            HoveredChevron::Hp => Some(format!("HP: {} / {}", self.hp_current, self.hp_max)),
            HoveredChevron::Endurance => Some(format!(
                "Endurance: {} / {}",
                self.end_current, self.end_max
            )),
            HoveredChevron::Mana => {
                Some(format!("Mana: {} / {}", self.mana_current, self.mana_max))
            }
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
        self.hp_current = a_hp.max(0);
        self.hp_max = max_hp.max(0);
        self.end_current = a_end.max(0);
        self.end_max = max_end.max(0);
        self.mana_current = a_mana.max(0);
        self.mana_max = max_mana.max(0);
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
    /// The chevron is rasterized as an outer triangle with an inner triangle
    /// removed from it. That produces a proper mitered apex and avoids the
    /// self-overdraw artifacts that happen when the two arms are painted as
    /// separate thick strokes.
    ///
    /// Outer geometry:
    /// * tip at `(cx, bot_y - height)`
    /// * feet at `(cx ± half_w, bot_y)`
    ///
    /// Inner geometry uses the same slope with a smaller half-width,
    /// `half_w - ARM_THICKNESS`, which creates the chevron band thickness.
    /// Pixels are filled left-to-right across the full outer span; pixels to
    /// the right of the fill boundary use [`TRACK_COLOR`].
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

        let inner_half_w = (half_w - ARM_THICKNESS).max(0);
        let inner_height = if inner_half_w > 0 {
            ((height * inner_half_w + half_w / 2) / half_w).max(1)
        } else {
            0
        };
        let inner_start_row = height - inner_height;

        let outermost_left = cx - half_w;
        let outermost_right = cx + half_w;
        let pixel_span = outermost_right - outermost_left + 1;
        let filled_pixels = ((fill * pixel_span as f32).round() as i32).clamp(0, pixel_span);
        let fill_end = outermost_left + filled_pixels - 1;

        // Walk scanlines from tip (row 0) down to feet (row height-1).
        for row in 0..height {
            let y = bot_y - height + row;
            let outer_offset = if height <= 1 {
                0
            } else {
                (row * half_w + (height - 1) / 2) / (height - 1)
            };
            let outer_left = cx - outer_offset;
            let outer_right = cx + outer_offset;

            if inner_height > 0 && row >= inner_start_row {
                let inner_row = row - inner_start_row;
                let inner_offset = if inner_height <= 1 {
                    0
                } else {
                    (inner_row * inner_half_w + (inner_height - 1) / 2) / (inner_height - 1)
                };
                let inner_left = cx - inner_offset;
                let inner_right = cx + inner_offset;

                for px in outer_left..inner_left {
                    let c = if px <= fill_end { color } else { TRACK_COLOR };
                    canvas.set_draw_color(c);
                    canvas.draw_point(sdl2::rect::Point::new(px, y))?;
                }
                for px in (inner_right + 1)..=outer_right {
                    let c = if px <= fill_end { color } else { TRACK_COLOR };
                    canvas.set_draw_color(c);
                    canvas.draw_point(sdl2::rect::Point::new(px, y))?;
                }
            } else {
                for px in outer_left..=outer_right {
                    let c = if px <= fill_end { color } else { TRACK_COLOR };
                    canvas.set_draw_color(c);
                    canvas.draw_point(sdl2::rect::Point::new(px, y))?;
                }
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
        let layer_inset = ARM_THICKNESS + LAYER_GAP;

        let layers: [(f32, Color, i32); 3] = [
            (self.hp_fill, HP_COLOR, 0),
            (self.end_fill, END_COLOR, layer_inset),
            (self.mana_fill, MANA_COLOR, layer_inset * 2),
        ];

        for (fill, color, inset) in layers {
            let hw = HP_HALF_W - inset;
            if hw <= 0 {
                continue;
            }
            // Height scales proportionally so the arm slope stays the same.
            // Use rounded division and clamp to at least 1 so even very narrow
            // inner chevrons still render a visible tip row.
            let h = Self::chevron_height(hw);
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
    use crate::ui::widget::UiEvent;

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

    #[test]
    fn hover_text_reports_hp_values() {
        let mut bars = VitalityBars::new(100, 200);
        bars.sync(34, 80, 21, 50, 12, 40);
        bars.handle_event(&UiEvent::MouseMove { x: 64, y: 199 });
        assert_eq!(bars.hover_text().as_deref(), Some("HP: 34 / 80"));
    }

    #[test]
    fn hover_text_reports_endurance_values() {
        let mut bars = VitalityBars::new(100, 200);
        bars.sync(34, 80, 21, 50, 12, 40);
        bars.handle_event(&UiEvent::MouseMove { x: 72, y: 199 });
        assert_eq!(bars.hover_text().as_deref(), Some("Endurance: 21 / 50"));
    }

    #[test]
    fn hover_text_reports_mana_values() {
        let mut bars = VitalityBars::new(100, 200);
        bars.sync(34, 80, 21, 50, 12, 40);
        bars.handle_event(&UiEvent::MouseMove { x: 80, y: 199 });
        assert_eq!(bars.hover_text().as_deref(), Some("Mana: 12 / 40"));
    }

    #[test]
    fn hover_text_clears_when_cursor_leaves_chevrons() {
        let mut bars = VitalityBars::new(100, 200);
        bars.sync(34, 80, 21, 50, 12, 40);
        bars.handle_event(&UiEvent::MouseMove { x: 10, y: 10 });
        assert_eq!(bars.hover_text(), None);
    }
}
