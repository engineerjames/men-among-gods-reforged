//! Circular speed-mode button with idle fade-out.
//!
//! Composes [`CircleButton`] from `button.rs` and adds:
//! - Mode cycling (Slow --> Normal --> Fast --> Slow) on click
//! - ChatBox-style idle fade-out (invisible after a few seconds of inactivity)
//! - Per-mode fill color and label
//! - `WidgetAction::ChangeMode` emission

use std::time::Duration;

use sdl2::pixels::Color;

use crate::font_cache;

use super::button::CircleButton;
use super::widget::{Bounds, EventResponse, UiEvent, Widget, WidgetAction};
use super::RenderContext;

// ---------------------------------------------------------------------------
// Fade constants
// ---------------------------------------------------------------------------

/// Seconds of inactivity before the fade-out animation begins.
const IDLE_FADE_DELAY_SECS: f32 = 3.0;

/// Duration in seconds of the fade-out transition (opaque --> invisible).
const IDLE_FADE_DURATION_SECS: f32 = 1.0;

// ---------------------------------------------------------------------------
// Per-mode visuals
// ---------------------------------------------------------------------------

/// Fill colors for each mode index (0 = Slow, 1 = Normal, 2 = Fast).
const MODE_COLORS: [Color; 3] = [
    Color::RGBA(50, 140, 50, 200),  // Slow  – green
    Color::RGBA(180, 160, 40, 200), // Normal – yellow
    Color::RGBA(180, 50, 50, 200),  // Fast   – red
];

/// Single-character labels for each mode.
const MODE_LABELS: [&str; 3] = ["S", "N", "F"];

/// Border color for the circle outline.
const BORDER_COLOR: Color = Color::RGBA(180, 180, 200, 220);

/// Bitmap font index for the mode label (yellow).
const LABEL_FONT: usize = 1;

// ---------------------------------------------------------------------------
// Widget
// ---------------------------------------------------------------------------

/// A circular button in the lower-right corner that cycles through speed modes.
///
/// When idle for [`IDLE_FADE_DELAY_SECS`] the button fades to invisible over
/// [`IDLE_FADE_DURATION_SECS`].  Any mouse movement within the circle or a
/// click resets the timer.
pub struct ModeButton {
    /// Inner circle button (handles rendering, hit-testing, hover state).
    inner: CircleButton,
    /// Current mode: 0 = Slow, 1 = Normal, 2 = Fast.
    mode: i32,
    /// Seconds since the last interaction.
    idle_elapsed: f32,
    /// Current draw opacity (0 = invisible, 255 = fully opaque).
    alpha: u8,
    /// Queued actions to be drained by the scene.
    pending_actions: Vec<WidgetAction>,
}

impl ModeButton {
    /// Create a new mode button.
    ///
    /// # Arguments
    ///
    /// * `center_x` - X center in logical pixels.
    /// * `center_y` - Y center in logical pixels.
    /// * `radius` - Circle radius in pixels.
    ///
    /// # Returns
    ///
    /// A new `ModeButton` defaulting to mode 1 (Normal).
    pub fn new(center_x: i32, center_y: i32, radius: u32) -> Self {
        let mode = 1i32;
        let inner = CircleButton::new(center_x, center_y, radius, MODE_COLORS[mode as usize])
            .with_border_color(BORDER_COLOR);
        Self {
            inner,
            mode,
            idle_elapsed: 0.0,
            alpha: 255,
            pending_actions: Vec::new(),
        }
    }

    /// Synchronise the widget with the server-confirmed mode value.
    ///
    /// This does **not** emit a `ChangeMode` action — it only updates the
    /// visual state.
    ///
    /// # Arguments
    ///
    /// * `mode` - Current mode from `character_info().mode`.
    pub fn sync(&mut self, mode: i32) {
        self.mode = mode.clamp(0, 2);
    }

    /// Apply the current alpha to the inner button's fill and border colors.
    fn apply_alpha(&mut self) {
        let base = MODE_COLORS[self.mode as usize];
        self.inner = CircleButton::new(
            self.inner.bounds().x + self.inner.bounds().width as i32 / 2,
            self.inner.bounds().y + self.inner.bounds().height as i32 / 2,
            self.inner.bounds().width / 2,
            Color::RGBA(base.r, base.g, base.b, scale_alpha(base.a, self.alpha)),
        )
        .with_border_color(Color::RGBA(
            BORDER_COLOR.r,
            BORDER_COLOR.g,
            BORDER_COLOR.b,
            scale_alpha(BORDER_COLOR.a, self.alpha),
        ));
    }
}

/// Scale a base alpha value by a global fade alpha (0–255).
fn scale_alpha(base: u8, fade: u8) -> u8 {
    ((base as u16 * fade as u16) / 255) as u8
}

impl Widget for ModeButton {
    /// Returns the bounding rectangle of the circle.
    fn bounds(&self) -> &Bounds {
        self.inner.bounds()
    }

    /// Moves the button's top-left corner.
    ///
    /// # Arguments
    ///
    /// * `x` - New left edge.
    /// * `y` - New top edge.
    fn set_position(&mut self, x: i32, y: i32) {
        self.inner.set_position(x, y);
    }

    /// Advance the idle timer and compute the current alpha.
    ///
    /// # Arguments
    ///
    /// * `dt` - Elapsed time since the last frame.
    fn update(&mut self, dt: Duration) {
        self.idle_elapsed += dt.as_secs_f32();
        self.alpha = if self.idle_elapsed < IDLE_FADE_DELAY_SECS {
            255
        } else {
            let t = ((self.idle_elapsed - IDLE_FADE_DELAY_SECS) / IDLE_FADE_DURATION_SECS).min(1.0);
            ((1.0 - t) * 255.0) as u8
        };
        self.apply_alpha();
    }

    /// Handle input events.
    ///
    /// Clicks inside the circle cycle the mode and emit a `ChangeMode` action.
    /// Mouse movement inside the circle resets the idle timer.
    ///
    /// # Arguments
    ///
    /// * `event` - The translated UI event.
    ///
    /// # Returns
    ///
    /// `Consumed` if the click hit the circle, `Ignored` otherwise.
    fn handle_event(&mut self, event: &UiEvent) -> EventResponse {
        match event {
            UiEvent::MouseMove { .. } => {
                let resp = self.inner.handle_event(event);
                if self.inner.is_hovered() {
                    self.idle_elapsed = 0.0;
                }
                resp
            }
            UiEvent::MouseClick { .. } => {
                let resp = self.inner.handle_event(event);
                if resp == EventResponse::Consumed {
                    self.idle_elapsed = 0.0;
                    self.mode = (self.mode + 1) % 3;
                    self.pending_actions
                        .push(WidgetAction::ChangeMode(self.mode));
                }
                resp
            }
            _ => EventResponse::Ignored,
        }
    }

    /// Draw the mode button.
    ///
    /// When fully faded (`alpha == 0`) the method returns immediately.
    ///
    /// # Arguments
    ///
    /// * `ctx` - Mutable render context (canvas + graphics cache).
    ///
    /// # Returns
    ///
    /// `Ok(())` on success, or an SDL2 error string.
    fn render(&mut self, ctx: &mut RenderContext<'_, '_>) -> Result<(), String> {
        if self.alpha == 0 {
            return Ok(());
        }

        self.inner.render(ctx)?;

        // Draw centered label on top of the circle.
        let label = MODE_LABELS[self.mode as usize];
        let bounds = self.inner.bounds();
        let center_x = bounds.x + bounds.width as i32 / 2;
        let text_y = bounds.y + (bounds.height as i32 - font_cache::BITMAP_GLYPH_H as i32) / 2;
        font_cache::draw_text(
            ctx.canvas,
            ctx.gfx,
            LABEL_FONT,
            label,
            center_x - font_cache::text_width(label) as i32 / 2,
            text_y,
            font_cache::TextStyle::faded(self.alpha),
        )?;

        Ok(())
    }

    /// Drain any pending actions.
    fn take_actions(&mut self) -> Vec<WidgetAction> {
        std::mem::take(&mut self.pending_actions)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::widget::{KeyModifiers, MouseButton};

    #[test]
    fn defaults_to_normal_mode() {
        let btn = ModeButton::new(100, 100, 18);
        assert_eq!(btn.mode, 1);
    }

    #[test]
    fn click_cycles_mode() {
        let mut btn = ModeButton::new(100, 100, 18);
        assert_eq!(btn.mode, 1); // Normal

        let click = UiEvent::MouseClick {
            x: 100,
            y: 100,
            button: MouseButton::Left,
            modifiers: KeyModifiers::default(),
        };
        let resp = btn.handle_event(&click);
        assert_eq!(resp, EventResponse::Consumed);
        assert_eq!(btn.mode, 2); // Fast

        btn.handle_event(&click);
        assert_eq!(btn.mode, 0); // Slow

        btn.handle_event(&click);
        assert_eq!(btn.mode, 1); // Normal (wrap)
    }

    #[test]
    fn click_emits_change_mode_action() {
        let mut btn = ModeButton::new(100, 100, 18);
        let click = UiEvent::MouseClick {
            x: 100,
            y: 100,
            button: MouseButton::Left,
            modifiers: KeyModifiers::default(),
        };
        btn.handle_event(&click);
        let actions = btn.take_actions();
        assert_eq!(actions.len(), 1);
        match &actions[0] {
            WidgetAction::ChangeMode(m) => assert_eq!(*m, 2),
            other => panic!("unexpected action: {:?}", other),
        }
    }

    #[test]
    fn click_outside_ignored() {
        let mut btn = ModeButton::new(100, 100, 18);
        let click = UiEvent::MouseClick {
            x: 0,
            y: 0,
            button: MouseButton::Left,
            modifiers: KeyModifiers::default(),
        };
        let resp = btn.handle_event(&click);
        assert_eq!(resp, EventResponse::Ignored);
        assert_eq!(btn.mode, 1);
        assert!(btn.take_actions().is_empty());
    }

    #[test]
    fn hover_resets_idle_timer() {
        let mut btn = ModeButton::new(100, 100, 18);
        btn.update(Duration::from_secs(10));
        assert_eq!(btn.alpha, 0);

        // Move inside the circle.
        btn.handle_event(&UiEvent::MouseMove { x: 100, y: 100 });
        // Alpha recalculated on next update.
        btn.update(Duration::from_millis(0));
        assert_eq!(btn.alpha, 255);
    }

    #[test]
    fn fades_to_zero_after_delay() {
        let mut btn = ModeButton::new(100, 100, 18);
        assert_eq!(btn.alpha, 255);

        // Still visible during delay.
        btn.update(Duration::from_secs_f32(IDLE_FADE_DELAY_SECS - 0.1));
        assert_eq!(btn.alpha, 255);

        // Fully faded after delay + duration.
        btn.update(Duration::from_secs_f32(IDLE_FADE_DURATION_SECS + 0.2));
        assert_eq!(btn.alpha, 0);
    }

    #[test]
    fn sync_updates_mode_without_action() {
        let mut btn = ModeButton::new(100, 100, 18);
        btn.sync(0);
        assert_eq!(btn.mode, 0);
        assert!(btn.take_actions().is_empty());
    }

    #[test]
    fn sync_clamps_mode() {
        let mut btn = ModeButton::new(100, 100, 18);
        btn.sync(5);
        assert_eq!(btn.mode, 2);
        btn.sync(-1);
        assert_eq!(btn.mode, 0);
    }
}
