//! A decorative semicircle arc widget that displays rank progress.
//!
//! The arc is centered at the bottom-center of the screen, curves upward,
//! and fills from left to right as the player approaches the next rank.

use std::f64::consts::PI;

use sdl2::pixels::Color;
use sdl2::render::BlendMode;

use super::widget::{Bounds, EventResponse, UiEvent, Widget};
use super::RenderContext;

/// Default color for the unfilled (background) portion of the arc.
const UNFILLED_COLOR: Color = Color::RGBA(60, 60, 80, 120);

/// Default color for the filled (progress) portion of the arc.
const FILLED_COLOR: Color = Color::RGBA(220, 180, 50, 220);

/// A thin semicircle arc that visualises rank progress.
///
/// Centered at `(center_x, center_y)` — typically the bottom-center of the
/// viewport. The arc spans 180° (left --> up --> right) and the filled portion
/// sweeps from left to right proportional to `progress` ∈ [0.0, 1.0].
///
/// This widget is purely decorative: it has no click handling.
pub struct RankArc {
    center_x: i32,
    center_y: i32,
    radius: u32,
    stroke_width: u32,
    progress: f64,
    unfilled_color: Color,
    filled_color: Color,
    bounds: Bounds,
}

impl RankArc {
    /// Creates a new rank progress arc.
    ///
    /// # Arguments
    ///
    /// * `center_x` - X center (screen pixels).
    /// * `center_y` - Y center (screen pixels).
    /// * `radius` - Arc radius in pixels.
    /// * `stroke_width` - Line thickness of the arc in pixels.
    ///
    /// # Returns
    ///
    /// A new `RankArc` with progress at 0.0.
    pub fn new(center_x: i32, center_y: i32, radius: u32, stroke_width: u32) -> Self {
        let bounds = Self::compute_bounds(center_x, center_y, radius);
        Self {
            center_x,
            center_y,
            radius,
            stroke_width,
            progress: 0.0,
            unfilled_color: UNFILLED_COLOR,
            filled_color: FILLED_COLOR,
            bounds,
        }
    }

    /// Sets the current rank progress (clamped to [0.0, 1.0]).
    ///
    /// # Arguments
    ///
    /// * `progress` - Fractional progress toward the next rank.
    pub fn set_progress(&mut self, progress: f64) {
        self.progress = progress.clamp(0.0, 1.0);
    }

    /// Computes the axis-aligned bounding box.
    fn compute_bounds(cx: i32, cy: i32, r: u32) -> Bounds {
        let r = r as i32;
        // The semicircle only extends upward (and left/right), not below center.
        Bounds::new(cx - r, cy - r, (r * 2) as u32, r as u32)
    }

    /// Draw an arc (portion of a circle outline) from `angle_start` to `angle_end`
    /// (radians, counter-clockwise in math convention, which is clockwise on
    /// screen since Y is flipped).
    ///
    /// The arc is drawn by iterating fine angular steps and plotting pixels
    /// for each of the `stroke_width` concentric radii.
    ///
    /// # Arguments
    ///
    /// * `canvas` - The SDL2 canvas.
    /// * `cx` - Center X.
    /// * `cy` - Center Y.
    /// * `r` - Outer radius.
    /// * `stroke` - Stroke width (drawn inward from `r`).
    /// * `angle_start` - Start angle in radians.
    /// * `angle_end` - End angle in radians (must be > `angle_start`).
    /// * `color` - Draw color.
    fn draw_arc(
        canvas: &mut sdl2::render::Canvas<sdl2::video::Window>,
        cx: i32,
        cy: i32,
        r: u32,
        stroke: u32,
        angle_start: f64,
        angle_end: f64,
        color: Color,
    ) -> Result<(), String> {
        if angle_end <= angle_start {
            return Ok(());
        }

        canvas.set_blend_mode(BlendMode::Blend);
        canvas.set_draw_color(color);

        // Number of angular steps: use circumference-based count for smooth rendering.
        let outer_r = r as f64;
        let steps = ((outer_r * (angle_end - angle_start)) * 2.0)
            .ceil()
            .max(1.0) as usize;
        let dt = (angle_end - angle_start) / steps as f64;

        let mut points = Vec::with_capacity(steps * stroke as usize);

        for s in 0..=steps {
            let theta = angle_start + dt * s as f64;
            let cos_t = theta.cos();
            let sin_t = theta.sin();

            for dr in 0..stroke {
                let ri = outer_r - dr as f64;
                if ri < 0.0 {
                    break;
                }
                let px = (cx as f64 + ri * cos_t).round() as i32;
                let py = (cy as f64 - ri * sin_t).round() as i32;
                points.push(sdl2::rect::Point::new(px, py));
            }
        }

        if !points.is_empty() {
            canvas.draw_points(points.as_slice())?;
        }

        Ok(())
    }
}

impl Widget for RankArc {
    fn bounds(&self) -> &Bounds {
        &self.bounds
    }

    fn set_position(&mut self, x: i32, y: i32) {
        self.center_x = x + self.radius as i32;
        self.center_y = y + self.radius as i32;
        self.bounds = Self::compute_bounds(self.center_x, self.center_y, self.radius);
    }

    fn handle_event(&mut self, _event: &UiEvent) -> EventResponse {
        EventResponse::Ignored
    }

    fn render(&mut self, ctx: &mut RenderContext<'_, '_>) -> Result<(), String> {
        // Full semicircle spans from π (left) to 0 (right) upward.
        // In our convention: angle_start = 0, angle_end = π  (we draw counter-clockwise).
        // Progress fills left-to-right: filled = [0, progress * π],
        // unfilled = [progress * π, π].
        //
        // Since our draw_arc uses math-convention angles (CCW from right),
        // π = left, 0 = right. To fill left-to-right:
        //   filled:   [π - progress * π, π]  --> but that fills right-to-left visually.
        //
        // Actually: angle 0 = right, π = left. Going counter-clockwise 0-->π sweeps
        // right-->up-->left. We want to fill left-->right, so filled is [π*(1-progress), π]?
        // No — let's think carefully:
        //
        // math angle 0   --> screen right
        // math angle π/2 --> screen up
        // math angle π   --> screen left
        //
        // "Left to right" on screen means angle decreases from π toward 0.
        // So filled portion: angle from (1 - progress) * π  to  π
        // That means we draw the filled arc from angle π down to π*(1-progress).
        // Since draw_arc wants start < end: filled = [(1-progress)*π, π]
        // And unfilled = [0, (1-progress)*π]

        let filled_boundary = (1.0 - self.progress) * PI;

        // Draw unfilled (background) portion: [0, filled_boundary]
        if filled_boundary > 0.001 {
            Self::draw_arc(
                ctx.canvas,
                self.center_x,
                self.center_y,
                self.radius,
                self.stroke_width,
                0.0,
                filled_boundary,
                self.unfilled_color,
            )?;
        }

        // Draw filled (progress) portion: [filled_boundary, π]
        if self.progress > 0.001 {
            Self::draw_arc(
                ctx.canvas,
                self.center_x,
                self.center_y,
                self.radius,
                self.stroke_width,
                filled_boundary,
                PI,
                self.filled_color,
            )?;
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
        let mut arc = RankArc::new(100, 100, 30, 2);
        arc.set_progress(-0.5);
        assert!((arc.progress - 0.0).abs() < 1e-9);
        arc.set_progress(1.5);
        assert!((arc.progress - 1.0).abs() < 1e-9);
        arc.set_progress(0.5);
        assert!((arc.progress - 0.5).abs() < 1e-9);
    }

    #[test]
    fn bounds_covers_upper_semicircle() {
        let arc = RankArc::new(200, 300, 30, 2);
        let b = arc.bounds();
        assert_eq!(b.x, 170); // cx - r
        assert_eq!(b.y, 270); // cy - r
        assert_eq!(b.width, 60); // 2r
        assert_eq!(b.height, 30); // r (only upper half)
    }

    #[test]
    fn ignores_all_events() {
        let mut arc = RankArc::new(100, 100, 30, 2);
        let resp = arc.handle_event(&UiEvent::MouseClick {
            x: 100,
            y: 100,
            button: super::super::widget::MouseButton::Left,
            modifiers: super::super::widget::KeyModifiers::default(),
        });
        assert_eq!(resp, EventResponse::Ignored);
    }
}
