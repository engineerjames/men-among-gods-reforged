//! Static compass rose matching the legacy client's isometric HUD compass.
//!
//! Two crossed lines form an "X" with a bitmap-font letter at each end:
//! N (top-left), E (top-right), W (bottom-left) and S (bottom-right), which
//! matches how the isometric map's cardinal directions appear on screen.

use sdl2::pixels::Color;
use sdl2::rect::Point;

use crate::font_cache::{self, BITMAP_GLYPH_H, TextStyle};
use crate::ui::RenderContext;
use crate::ui::widget::{Bounds, EventResponse, UiEvent, Widget};

/// Bitmap font used for the direction letters (yellow UI font).
const FONT: usize = 1;
/// Horizontal distance from the compass center to each letter's center.
const LETTER_DX: i32 = 16;
/// Vertical distance from the compass center to each letter row's center.
const LETTER_DY: i32 = 7;
/// Horizontal half-extent of each line of the "X".
const LINE_DX: i32 = 11;
/// Vertical half-extent of each line of the "X".
const LINE_DY: i32 = 5;
/// Total widget width (letters plus a small margin).
const WIDTH: u32 = 40;
/// Total widget height (two letter rows).
const HEIGHT: u32 = 26;
/// Color of the crossed lines, matched to the yellow letter font.
const LINE_COLOR: Color = Color::RGB(230, 215, 150);

/// Display-only compass rose drawn in the HUD.
pub struct Compass {
    bounds: Bounds,
}

impl Compass {
    /// Creates a compass centered on the given point.
    ///
    /// # Arguments
    ///
    /// * `center_x` - X center in logical pixels.
    /// * `center_y` - Y center in logical pixels.
    ///
    /// # Returns
    ///
    /// * A new `Compass`.
    pub fn new(center_x: i32, center_y: i32) -> Self {
        Self {
            bounds: Bounds::new(
                center_x - WIDTH as i32 / 2,
                center_y - HEIGHT as i32 / 2,
                WIDTH,
                HEIGHT,
            ),
        }
    }

    /// Returns the compass center point.
    fn center(&self) -> (i32, i32) {
        (
            self.bounds.x + WIDTH as i32 / 2,
            self.bounds.y + HEIGHT as i32 / 2,
        )
    }
}

impl Widget for Compass {
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
        let (cx, cy) = self.center();

        ctx.canvas.set_draw_color(LINE_COLOR);
        ctx.canvas.draw_line(
            Point::new(cx - LINE_DX, cy - LINE_DY),
            Point::new(cx + LINE_DX, cy + LINE_DY),
        )?;
        ctx.canvas.draw_line(
            Point::new(cx + LINE_DX, cy - LINE_DY),
            Point::new(cx - LINE_DX, cy + LINE_DY),
        )?;

        let half_h = BITMAP_GLYPH_H as i32 / 2;
        for (letter, dx, dy) in [
            ("N", -LETTER_DX, -LETTER_DY),
            ("E", LETTER_DX, -LETTER_DY),
            ("W", -LETTER_DX, LETTER_DY),
            ("S", LETTER_DX, LETTER_DY),
        ] {
            font_cache::draw_text(
                ctx.canvas,
                ctx.gfx,
                FONT,
                letter,
                cx + dx,
                cy + dy - half_h,
                TextStyle::centered(),
            )?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn letters_and_lines_fit_inside_bounds() {
        let compass = Compass::new(100, 50);
        let b = *compass.bounds();
        let (cx, cy) = compass.center();
        assert_eq!((cx, cy), (100, 50));

        let half_glyph_w = font_cache::BITMAP_GLYPH_W as i32 / 2;
        assert!(b.contains_point(cx - LETTER_DX - half_glyph_w, cy - LETTER_DY - 5));
        assert!(b.contains_point(cx + LETTER_DX + half_glyph_w, cy + LETTER_DY + 4));
        assert!(LINE_DX < LETTER_DX && LINE_DY < LETTER_DY);
    }
}
