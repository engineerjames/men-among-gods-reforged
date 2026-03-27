//! A horizontal progress line widget that displays rank progress.
//!
//! The line fills from left to right as the player approaches the next rank.
//! This is a flat alternative to [`super::rank_progress_arc::RankArc`].

use sdl2::pixels::Color;
use sdl2::render::BlendMode;

use mag_core::ranks::{RANK_THRESHOLDS, TOTAL_RANKS, points2rank, rank_progress};

use super::RenderContext;
use super::widget::{Bounds, EventResponse, UiEvent, Widget};

/// Default color for the unfilled (background) portion of the line.
const UNFILLED_COLOR: Color = Color::RGBA(60, 60, 80, 120);

/// Default color for the filled (progress) portion of the line.
const FILLED_COLOR: Color = Color::RGBA(220, 180, 50, 220);

/// A horizontal progress bar that visualises rank progress.
///
/// Positioned at `(x, y)` with a given `width` and `stroke_height`. The
/// filled portion grows from the left edge proportional to `progress`
/// ∈ [0.0, 1.0].
///
/// This widget is purely decorative: it has no click handling, but it can
/// expose helper text while hovered.
pub struct RankProgressLine {
    bounds: Bounds,
    stroke_height: u32,
    progress: f64,
    experience_until_rank: u32,
    hovered: bool,
    unfilled_color: Color,
    filled_color: Color,
}

impl RankProgressLine {
    /// Creates a new rank progress line.
    ///
    /// # Arguments
    ///
    /// * `x` - Left edge (screen pixels).
    /// * `y` - Top edge (screen pixels).
    /// * `width` - Total line width in pixels.
    /// * `stroke_height` - Line thickness in pixels.
    ///
    /// # Returns
    ///
    /// A new `RankProgressLine` with progress at 0.0.
    pub fn new(x: i32, y: i32, width: u32, stroke_height: u32) -> Self {
        Self {
            bounds: Bounds::new(x, y, width, stroke_height),
            stroke_height,
            progress: 0.0,
            experience_until_rank: 0,
            hovered: false,
            unfilled_color: UNFILLED_COLOR,
            filled_color: FILLED_COLOR,
        }
    }

    /// Syncs the line from the player's total experience points.
    ///
    /// # Arguments
    ///
    /// * `points` - Total experience points for the active character.
    pub fn sync(&mut self, points: u32) {
        self.progress = rank_progress(points);

        let rank_index = points2rank(points) as usize;
        self.experience_until_rank = if rank_index + 1 >= TOTAL_RANKS {
            0
        } else {
            RANK_THRESHOLDS[rank_index + 1].saturating_sub(points)
        };
    }

    /// Sets the current rank progress (clamped to [0.0, 1.0]).
    ///
    /// # Arguments
    ///
    /// * `progress` - Fractional progress toward the next rank.
    pub fn set_progress(&mut self, progress: f64) {
        self.progress = progress.clamp(0.0, 1.0);
    }

    /// Returns the helper text to display while the line is hovered.
    ///
    /// # Returns
    ///
    /// * `Some(String)` with the remaining experience to the next rank when
    ///   hovered, otherwise `None`.
    pub fn hover_text(&self) -> Option<String> {
        self.hovered
            .then(|| format!("{} experience until rank", self.experience_until_rank))
    }
}

impl Widget for RankProgressLine {
    /// Returns the bounding rectangle of the line.
    fn bounds(&self) -> &Bounds {
        &self.bounds
    }

    /// Moves the line's top-left corner.
    ///
    /// # Arguments
    ///
    /// * `x` - New left edge.
    /// * `y` - New top edge.
    fn set_position(&mut self, x: i32, y: i32) {
        self.bounds.x = x;
        self.bounds.y = y;
    }

    /// Updates hover state while keeping the line non-interactive.
    fn handle_event(&mut self, event: &UiEvent) -> EventResponse {
        if let UiEvent::MouseMove { x, y } = event {
            self.hovered = self.bounds.contains_point(*x, *y);
        }
        EventResponse::Ignored
    }

    /// Draw the progress line.
    ///
    /// # Arguments
    ///
    /// * `ctx` - Mutable render context (canvas + graphics cache).
    ///
    /// # Returns
    ///
    /// `Ok(())` on success, or an SDL2 error string.
    fn render(&mut self, ctx: &mut RenderContext<'_, '_>) -> Result<(), String> {
        let total_w = self.bounds.width;
        let filled_w = ((self.progress * total_w as f64).round() as u32).min(total_w);
        let unfilled_w = total_w.saturating_sub(filled_w);

        ctx.canvas.set_blend_mode(BlendMode::Blend);

        // Filled portion (left).
        if filled_w > 0 {
            ctx.canvas.set_draw_color(self.filled_color);
            ctx.canvas.fill_rect(sdl2::rect::Rect::new(
                self.bounds.x,
                self.bounds.y,
                filled_w,
                self.stroke_height,
            ))?;
        }

        // Unfilled portion (right).
        if unfilled_w > 0 {
            ctx.canvas.set_draw_color(self.unfilled_color);
            ctx.canvas.fill_rect(sdl2::rect::Rect::new(
                self.bounds.x + filled_w as i32,
                self.bounds.y,
                unfilled_w,
                self.stroke_height,
            ))?;
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
    fn progress_clamps_to_valid_range() {
        let mut line = RankProgressLine::new(0, 0, 200, 4);
        line.set_progress(-0.5);
        assert!((line.progress - 0.0).abs() < 1e-9);
        line.set_progress(1.5);
        assert!((line.progress - 1.0).abs() < 1e-9);
        line.set_progress(0.75);
        assert!((line.progress - 0.75).abs() < 1e-9);
    }

    #[test]
    fn bounds_matches_constructor() {
        let line = RankProgressLine::new(10, 20, 300, 6);
        let b = line.bounds();
        assert_eq!(b.x, 10);
        assert_eq!(b.y, 20);
        assert_eq!(b.width, 300);
        assert_eq!(b.height, 6);
    }

    #[test]
    fn set_position_updates_bounds() {
        let mut line = RankProgressLine::new(0, 0, 100, 4);
        line.set_position(50, 60);
        assert_eq!(line.bounds.x, 50);
        assert_eq!(line.bounds.y, 60);
        // Width/height unchanged.
        assert_eq!(line.bounds.width, 100);
        assert_eq!(line.bounds.height, 4);
    }

    #[test]
    fn ignores_all_events() {
        let mut line = RankProgressLine::new(0, 0, 100, 4);
        let resp = line.handle_event(&UiEvent::MouseClick {
            x: 50,
            y: 2,
            button: super::super::widget::MouseButton::Left,
            modifiers: super::super::widget::KeyModifiers::default(),
        });
        assert_eq!(resp, EventResponse::Ignored);
    }

    #[test]
    fn default_progress_is_zero() {
        let line = RankProgressLine::new(0, 0, 100, 4);
        assert!((line.progress - 0.0).abs() < 1e-9);
    }

    #[test]
    fn sync_sets_experience_until_next_rank() {
        let mut line = RankProgressLine::new(0, 0, 100, 4);
        line.sync(49);
        assert_eq!(line.experience_until_rank, 1);

        line.sync(80_977_100);
        assert_eq!(line.experience_until_rank, 0);
    }

    #[test]
    fn hover_text_reports_remaining_experience_only_while_hovered() {
        let mut line = RankProgressLine::new(0, 0, 100, 4);
        line.sync(849);

        assert_eq!(line.hover_text(), None);

        line.handle_event(&UiEvent::MouseMove { x: 20, y: 2 });
        assert_eq!(
            line.hover_text().as_deref(),
            Some("1 experience until rank")
        );

        line.handle_event(&UiEvent::MouseMove { x: 200, y: 50 });
        assert_eq!(line.hover_text(), None);
    }
}
