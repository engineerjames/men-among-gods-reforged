//! Compact rank sigil widget drawn at the top-left of the HUD.
//!
//! Renders the player's current rank as a trimmed sprite icon. When the mouse
//! is hovering, [`RankSigil::is_hovered`] returns `true` so the owning scene
//! can display the rank name as a context-sensitive helper-text tooltip.

use sdl2::pixels::Color;
use sdl2::render::BlendMode;

use mag_core::ranks::{TOTAL_RANKS, rank_name_by_index};

use super::RenderContext;
use super::widget::{Bounds, EventResponse, UiEvent, Widget};

// ---------------------------------------------------------------------------
// Layout constants
// ---------------------------------------------------------------------------

/// Width of the rank sigil sprite in pixels.
const SIGIL_WIDTH: i32 = 32;
/// Full height of the rank sigil sprite in pixels (before per-rank trimming).
const SIGIL_HEIGHT: i32 = 96;
/// Padding between the panel edge and the sigil sprite.
const PANEL_PADDING: i32 = 4;

/// Per-rank transparent row counts to trim from the top and bottom of the
/// sigil sprite before drawing, matching the sprite sheet layout.
///
/// Each tuple is `(trim_top, trim_bottom)`.
const SIGIL_TRIM_ROWS: [(u32, u32); TOTAL_RANKS] = [
    (0, 0),   // Private        (sprite 10: fully transparent)
    (46, 38), // Private First Class  (sprite 11: rows 46-57)
    (26, 38), // Lance Corporal (sprite 12: rows 26-57)
    (46, 30), // Corporal       (sprite 13: rows 46-65)
    (46, 22), // Sergeant       (sprite 14: rows 46-73)
    (26, 22), // Staff Sergeant (sprite 15: rows 26-73)
    (18, 22), // Master Sergeant (sprite 16: rows 18-73)
    (10, 22), // First Sergeant (sprite 17: rows 10-73)
    (10, 22), // Sergeant Major (sprite 18: rows 10-73)
    (34, 48), // Second Lieutenant (sprite 19: rows 34-47)
    (34, 32), // First Lieutenant  (sprite 20: rows 34-63)
    (34, 16), // Captain        (sprite 21: rows 34-79)
    (0, 64),  // Major          (sprite 22: rows 0-31)
    (0, 48),  // Lieutenant Colonel (sprite 23: rows 0-47)
    (0, 32),  // Colonel        (sprite 24: rows 0-63)
    (0, 16),  // Brigadier General (sprite 25: rows 0-79)
    (0, 31),  // Major General  (sprite 26: rows 0-64)
    (0, 16),  // Lieutenant General (sprite 27: rows 0-79)
    (0, 16),  // General        (sprite 28: rows 0-79)
    (1, 16),  // Field Marshal  (sprite 29: rows 1-79)
    (1, 16),  // Knight         (sprite 30: rows 1-79)
    (1, 16),  // Baron          (sprite 30: same as Knight)
    (1, 16),  // Earl           (sprite 30: same as Knight)
    (1, 16),  // Warlord        (sprite 30: same as Knight)
];

// ---------------------------------------------------------------------------
// Widget
// ---------------------------------------------------------------------------

/// The rank sigil widget, displayed in the top-left corner of the HUD.
///
/// Renders the current rank as a trimmed sprite. Exposes [`is_hovered`] so
/// the owning scene can show the rank name as a tooltip near the cursor.
///
/// [`is_hovered`]: RankSigil::is_hovered
pub struct RankSigil {
    /// Bounding rectangle of the panel (sigil + padding).
    bounds: Bounds,
    /// Semi-transparent background fill colour.
    bg_color: Color,
    /// Current rank index (0–23).
    rank_index: usize,
    /// `true` when the mouse cursor is inside the widget bounds.
    hovered: bool,
}

impl RankSigil {
    /// Create a new `RankSigil` positioned at `(x, y)`.
    ///
    /// # Arguments
    ///
    /// * `x` - Left edge in logical pixels.
    /// * `y` - Top edge in logical pixels.
    /// * `bg_color` - Semi-transparent panel background colour.
    ///
    /// # Returns
    ///
    /// * A new `RankSigil` initialised at rank 0.
    pub fn new(x: i32, y: i32, bg_color: Color) -> Self {
        Self {
            bounds: Self::compute_bounds(x, y, 0),
            bg_color,
            rank_index: 0,
            hovered: false,
        }
    }

    /// Compute the bounding rectangle for a given position and rank index.
    ///
    /// # Arguments
    ///
    /// * `x` - Left edge in logical pixels.
    /// * `y` - Top edge in logical pixels.
    /// * `rank_index` - Rank index whose trim values determine the panel height.
    ///
    /// # Returns
    ///
    /// * A `Bounds` sized to contain the padded sigil sprite.
    fn compute_bounds(x: i32, y: i32, rank_index: usize) -> Bounds {
        let (_, draw_height) = Self::draw_metrics(rank_index);
        let w = (PANEL_PADDING * 2 + SIGIL_WIDTH) as u32;
        let h = (PANEL_PADDING * 2) as u32 + draw_height;
        Bounds::new(x, y, w, h)
    }

    /// Returns `(trim_top, draw_height)` for the given rank index.
    ///
    /// # Arguments
    ///
    /// * `rank_index` - Rank index to look up; clamped to the table length.
    ///
    /// # Returns
    ///
    /// * `trim_top` — pixel rows to skip from the top of the sprite.
    /// * `draw_height` — pixel rows to actually render, always at least 1.
    fn draw_metrics(rank_index: usize) -> (u32, u32) {
        let idx = rank_index.min(SIGIL_TRIM_ROWS.len() - 1);
        let (trim_top, trim_bottom) = SIGIL_TRIM_ROWS[idx];
        let max = SIGIL_HEIGHT as u32;
        let trim_top = trim_top.min(max);
        let trim_bottom = trim_bottom.min(max.saturating_sub(trim_top));
        let draw_height = max.saturating_sub(trim_top + trim_bottom).max(1);
        (trim_top, draw_height)
    }

    /// Update the displayed rank index, rebuilding bounds as needed.
    ///
    /// Should be called once per frame before [`render`].
    ///
    /// # Arguments
    ///
    /// * `rank_index` - New rank index (0–23).
    ///
    /// [`render`]: Self::render
    pub fn sync(&mut self, rank_index: usize) {
        if self.rank_index != rank_index {
            self.rank_index = rank_index;
            self.bounds = Self::compute_bounds(self.bounds.x, self.bounds.y, rank_index);
        }
    }

    /// Returns `true` when the mouse cursor is over this widget.
    pub fn is_hovered(&self) -> bool {
        self.hovered
    }

    /// Returns the display name of the currently shown rank.
    ///
    /// # Returns
    ///
    /// * A static rank name string (e.g. `"Sergeant"`, `"Colonel"`).
    pub fn rank_name(&self) -> &'static str {
        rank_name_by_index(self.rank_index)
    }
}

impl Widget for RankSigil {
    /// Returns the bounding rectangle of the sigil panel.
    fn bounds(&self) -> &Bounds {
        &self.bounds
    }

    /// Moves the top-left corner of the sigil to `(x, y)`.
    ///
    /// # Arguments
    ///
    /// * `x` - New left edge in logical pixels.
    /// * `y` - New top edge in logical pixels.
    fn set_position(&mut self, x: i32, y: i32) {
        self.bounds.x = x;
        self.bounds.y = y;
    }

    /// Handle mouse events.
    ///
    /// Hover state is updated on every [`UiEvent::MouseMove`]. Clicks that
    /// land inside the bounds are consumed to prevent world interaction.
    ///
    /// # Arguments
    ///
    /// * `event` - The translated UI event.
    ///
    /// # Returns
    ///
    /// * `Consumed` if a click landed inside the sigil bounds, `Ignored` otherwise.
    fn handle_event(&mut self, event: &UiEvent) -> EventResponse {
        match event {
            UiEvent::MouseMove { x, y } => {
                self.hovered = self.bounds.contains_point(*x, *y);
                EventResponse::Ignored
            }
            UiEvent::MouseClick { x, y, .. } => {
                if self.bounds.contains_point(*x, *y) {
                    EventResponse::Consumed
                } else {
                    EventResponse::Ignored
                }
            }
            _ => EventResponse::Ignored,
        }
    }

    /// Draw the sigil panel background and sprite.
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

        let sigil_x = self.bounds.x + PANEL_PADDING;
        let sigil_y = self.bounds.y + PANEL_PADDING;
        let sprite_id = 10 + self.rank_index.min(20);
        let tex = ctx.gfx.get_texture(sprite_id);
        let (trim_top, draw_height) = Self::draw_metrics(self.rank_index);
        ctx.canvas.copy(
            tex,
            Some(sdl2::rect::Rect::new(
                0,
                trim_top as i32,
                SIGIL_WIDTH as u32,
                draw_height,
            )),
            Some(sdl2::rect::Rect::new(
                sigil_x,
                sigil_y,
                SIGIL_WIDTH as u32,
                draw_height,
            )),
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
    fn new_initialises_rank_zero_and_not_hovered() {
        let sigil = RankSigil::new(4, 4, Color::RGBA(10, 10, 30, 180));
        assert_eq!(sigil.rank_index, 0);
        assert!(!sigil.is_hovered());
    }

    #[test]
    fn sync_updates_rank_index_and_bounds() {
        let mut sigil = RankSigil::new(4, 4, Color::RGBA(10, 10, 30, 180));
        let h_before = sigil.bounds().height;
        // Rank 1 (Private First Class): trim (46,38) → draw_height = 96-84 = 12
        sigil.sync(1);
        assert_eq!(sigil.rank_index, 1);
        assert_ne!(
            sigil.bounds().height,
            h_before,
            "bounds must shrink for rank 1"
        );
    }

    #[test]
    fn sync_no_op_when_rank_unchanged() {
        let mut sigil = RankSigil::new(4, 4, Color::RGBA(10, 10, 30, 180));
        sigil.sync(5);
        let h = sigil.bounds().height;
        sigil.sync(5);
        assert_eq!(sigil.bounds().height, h);
    }

    #[test]
    fn rank_name_matches_index() {
        let mut sigil = RankSigil::new(4, 4, Color::RGBA(10, 10, 30, 180));
        sigil.sync(0);
        assert_eq!(sigil.rank_name(), "Private");
        sigil.sync(11);
        assert_eq!(sigil.rank_name(), "Captain");
    }

    #[test]
    fn click_inside_is_consumed() {
        let mut sigil = RankSigil::new(4, 4, Color::RGBA(10, 10, 30, 180));
        // Bounds: x=4, y=4, w=40, h varies; point (20, 8) is inside.
        let click = UiEvent::MouseClick {
            x: 20,
            y: 8,
            button: super::super::widget::MouseButton::Left,
            modifiers: super::super::widget::KeyModifiers::default(),
        };
        assert_eq!(sigil.handle_event(&click), EventResponse::Consumed);
    }

    #[test]
    fn click_outside_is_ignored() {
        let mut sigil = RankSigil::new(4, 4, Color::RGBA(10, 10, 30, 180));
        let click = UiEvent::MouseClick {
            x: 500,
            y: 500,
            button: super::super::widget::MouseButton::Left,
            modifiers: super::super::widget::KeyModifiers::default(),
        };
        assert_eq!(sigil.handle_event(&click), EventResponse::Ignored);
    }

    #[test]
    fn mouse_move_tracks_hover() {
        let mut sigil = RankSigil::new(4, 4, Color::RGBA(10, 10, 30, 180));
        assert!(!sigil.is_hovered());
        sigil.handle_event(&UiEvent::MouseMove { x: 20, y: 8 });
        assert!(sigil.is_hovered());
        sigil.handle_event(&UiEvent::MouseMove { x: 500, y: 500 });
        assert!(!sigil.is_hovered());
    }

    #[test]
    fn draw_metrics_clamps_out_of_range_index() {
        // Should not panic for very large index
        let (trim_top, draw_height) = RankSigil::draw_metrics(999);
        assert!(draw_height >= 1);
        assert!(trim_top < SIGIL_HEIGHT as u32);
    }
}
