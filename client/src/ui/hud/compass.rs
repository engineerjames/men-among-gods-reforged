//! Static compass rose matching the legacy client's isometric HUD compass.
//!
//! Two crossed lines form an "X" with a bitmap-font letter at each end:
//! N (top-left), E (top-right), W (bottom-left) and S (bottom-right), which
//! matches how the isometric map's cardinal directions appear on screen.
//! The player can reposition it by left-click dragging anywhere on it.
//!
//! When a target world tile is set, a needle points from the compass center
//! in the on-screen direction the player must walk to reach it. A debug
//! override target (set from the Diagnostics settings) takes precedence over
//! the normal target without replacing it, so clearing the override restores
//! whatever the normal target is at that point.

use sdl2::pixels::Color;
use sdl2::rect::Point;

use crate::font_cache::{self, BITMAP_GLYPH_H, TextStyle};
use crate::ui::RenderContext;
use crate::ui::widget::{Bounds, EventResponse, MouseButton, UiEvent, Widget};
use crate::ui::widgets::title_bar::clamp_to_viewport;

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
/// Horizontal radius of the ellipse the target needle tip travels on.
const MARKER_RX: f32 = 14.0;
/// Vertical radius of the needle ellipse. Must be `MARKER_RX / 2` to match the
/// 2:1 isometric projection so the needle points along the true screen ray.
const MARKER_RY: f32 = MARKER_RX / 2.0;
/// Color of the target needle and its tip.
const MARKER_COLOR: Color = Color::RGB(255, 80, 60);

/// Compass rose drawn in the HUD that the player can drag to reposition.
pub struct Compass {
    bounds: Bounds,
    /// Cursor offset from the top-left corner while a drag is in progress.
    drag_offset: Option<(i32, i32)>,
    /// World tile the needle points toward, if any.
    target: Option<(u16, u16)>,
    /// Debug override that takes precedence over `target` while set.
    debug_target: Option<(u16, u16)>,
    /// Local player's current world tile, if known.
    player_pos: Option<(u16, u16)>,
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
            drag_offset: None,
            target: None,
            debug_target: None,
            player_pos: None,
        }
    }

    /// Sets or clears the world tile the compass needle points toward.
    ///
    /// # Arguments
    ///
    /// * `target` - World tile `(x, y)` to point at, or `None` to hide the needle.
    pub fn set_target(&mut self, target: Option<(u16, u16)>) {
        self.target = target;
    }

    /// Sets or clears the debug override target.
    ///
    /// While set it takes precedence over [`Compass::set_target`]; clearing it
    /// makes the needle point at the normal target again.
    ///
    /// # Arguments
    ///
    /// * `target` - Override world tile `(x, y)`, or `None` to clear it.
    pub fn set_debug_target(&mut self, target: Option<(u16, u16)>) {
        self.debug_target = target;
    }

    /// Returns the active debug override target, if any.
    ///
    /// # Returns
    ///
    /// * The override world tile, or `None` when no override is set.
    pub fn debug_target(&self) -> Option<(u16, u16)> {
        self.debug_target
    }

    /// Updates the local player's world tile used to aim the needle.
    ///
    /// # Arguments
    ///
    /// * `pos` - The player's world tile `(x, y)`, or `None` when unknown.
    pub fn set_player_position(&mut self, pos: Option<(u16, u16)>) {
        self.player_pos = pos;
    }

    /// Returns the needle tip offset from the compass center.
    ///
    /// World `+x` projects to screen `(+16, +8)` and world `+y` to `(+16, -8)`
    /// (see `GameScene::tile_ground_diamond_origin`), so the screen direction
    /// of a world delta `(dx, dy)` is `(dx + dy, (dx - dy) / 2)`. Scaling the
    /// unit vector of `(dx + dy, dx - dy)` by `(MARKER_RX, MARKER_RY)` lands the
    /// tip on that ray.
    ///
    /// # Returns
    ///
    /// * `None` when no target (or debug override) is set or the player
    ///   position is unknown.
    /// * `Some((0, 0))` when the player stands on the target tile.
    /// * Otherwise the `(dx, dy)` pixel offset of the needle tip.
    fn marker_offset(&self) -> Option<(i32, i32)> {
        let (tx, ty) = self.debug_target.or(self.target)?;
        let (px, py) = self.player_pos?;
        let dx = i32::from(tx) - i32::from(px);
        let dy = i32::from(ty) - i32::from(py);
        let (u, v) = ((dx + dy) as f32, (dx - dy) as f32);
        let len = u.hypot(v);
        if len == 0.0 {
            return Some((0, 0));
        }
        Some((
            (MARKER_RX * u / len).round() as i32,
            (MARKER_RY * v / len).round() as i32,
        ))
    }

    /// Returns whether the compass is currently being dragged.
    ///
    /// # Returns
    ///
    /// * `true` between a left mouse-down on the compass and the release.
    pub fn is_dragging(&self) -> bool {
        self.drag_offset.is_some()
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

    fn handle_event(&mut self, event: &UiEvent) -> EventResponse {
        match (event, self.drag_offset) {
            (
                UiEvent::MouseDown {
                    x,
                    y,
                    button: MouseButton::Left,
                    ..
                },
                _,
            ) if self.bounds.contains_point(*x, *y) => {
                self.drag_offset = Some((*x - self.bounds.x, *y - self.bounds.y));
                EventResponse::Consumed
            }
            (UiEvent::MouseMove { x, y }, Some((ox, oy))) => {
                let (nx, ny) = clamp_to_viewport(*x - ox, *y - oy, WIDTH, HEIGHT);
                self.set_position(nx, ny);
                EventResponse::Consumed
            }
            (
                UiEvent::MouseClick {
                    button: MouseButton::Left,
                    ..
                },
                Some(_),
            ) => {
                self.drag_offset = None;
                EventResponse::Consumed
            }
            _ => EventResponse::Ignored,
        }
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

        if let Some((mx, my)) = self.marker_offset() {
            let tip = Point::new(cx + mx, cy + my);
            ctx.canvas.set_draw_color(MARKER_COLOR);
            ctx.canvas.draw_line(Point::new(cx, cy), tip)?;
            ctx.canvas
                .fill_rect(sdl2::rect::Rect::new(tip.x - 1, tip.y - 1, 3, 3))?;
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

    #[test]
    fn marker_points_along_isometric_screen_direction() {
        let mut compass = Compass::new(100, 50);
        assert_eq!(compass.marker_offset(), None);

        compass.set_target(Some((110, 100)));
        assert_eq!(compass.marker_offset(), None, "needs player position");

        compass.set_player_position(Some((100, 100)));
        // World +x is screen down-right (the "S" letter quadrant).
        let (mx, my) = compass.marker_offset().unwrap();
        assert!(mx > 0 && my > 0);
        // Tip stays on the 2:1 diagonal, i.e. along the drawn "X" arm.
        assert_eq!((mx, my), (10, 5));

        // World -y is screen down-left ("W" quadrant).
        compass.set_target(Some((100, 90)));
        assert_eq!(compass.marker_offset(), Some((-10, 5)));

        // dx == dy points straight right on screen.
        compass.set_target(Some((105, 105)));
        assert_eq!(compass.marker_offset(), Some((MARKER_RX as i32, 0)));

        // Standing on the target collapses the needle to the center.
        compass.set_target(Some((100, 100)));
        assert_eq!(compass.marker_offset(), Some((0, 0)));

        compass.set_target(None);
        assert_eq!(compass.marker_offset(), None);

        // Tip always stays inside the widget bounds.
        compass.set_target(Some((u16::MAX, 0)));
        let (mx, my) = compass.marker_offset().unwrap();
        let (cx, cy) = compass.center();
        assert!(compass.bounds().contains_point(cx + mx, cy + my));
    }

    #[test]
    fn debug_target_overrides_and_then_restores_normal_target() {
        let mut compass = Compass::new(100, 50);
        compass.set_player_position(Some((100, 100)));
        compass.set_target(Some((110, 100)));
        let normal = compass.marker_offset();

        compass.set_debug_target(Some((100, 90)));
        assert_eq!(compass.marker_offset(), Some((-10, 5)));

        compass.set_debug_target(None);
        assert_eq!(compass.marker_offset(), normal);
    }

    #[test]
    fn drag_moves_compass_and_clamps_to_viewport() {
        use crate::ui::widget::KeyModifiers;

        let down = |x, y| UiEvent::MouseDown {
            x,
            y,
            button: MouseButton::Left,
            modifiers: KeyModifiers::default(),
        };
        let mut compass = Compass::new(100, 50);

        // Clicks outside the compass are ignored.
        assert_eq!(compass.handle_event(&down(0, 0)), EventResponse::Ignored);

        assert_eq!(
            compass.handle_event(&down(100, 50)),
            EventResponse::Consumed
        );
        assert!(compass.is_dragging());

        compass.handle_event(&UiEvent::MouseMove { x: 130, y: 70 });
        assert_eq!(compass.center(), (130, 70));

        compass.handle_event(&UiEvent::MouseMove { x: -500, y: -500 });
        assert_eq!((compass.bounds().x, compass.bounds().y), (0, 0));

        let up = UiEvent::MouseClick {
            x: 0,
            y: 0,
            button: MouseButton::Left,
            modifiers: KeyModifiers::default(),
        };
        assert_eq!(compass.handle_event(&up), EventResponse::Consumed);
        assert!(!compass.is_dragging());

        // Moves after release no longer reposition it.
        compass.handle_event(&UiEvent::MouseMove { x: 300, y: 300 });
        assert_eq!((compass.bounds().x, compass.bounds().y), (0, 0));
    }
}
