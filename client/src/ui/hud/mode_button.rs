//! Circular speed-mode button.
//!
//! Composes [`CircleButton`] from `button.rs` and adds:
//! - Mode cycling (Slow --> Normal --> Fast --> Slow) on click
//! - Per-mode fill color and label
//! - `WidgetAction::ChangeMode` emission

use sdl2::pixels::Color;

use crate::font_cache;

use crate::ui::RenderContext;
use crate::ui::widgets::button::CircleButton;
use crate::ui::widget::{Bounds, EventResponse, UiEvent, Widget, WidgetAction};

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
pub struct ModeButton {
    /// Inner circle button (handles rendering, hit-testing, hover state).
    inner: CircleButton,
    /// Current mode: 0 = Slow, 1 = Normal, 2 = Fast.
    mode: i32,
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
        self.apply_mode_visuals();
    }

    /// Rebuild the inner button so its fill reflects the current mode.
    fn apply_mode_visuals(&mut self) {
        let fill = MODE_COLORS[self.mode as usize];
        self.inner = CircleButton::new(
            self.inner.bounds().x + self.inner.bounds().width as i32 / 2,
            self.inner.bounds().y + self.inner.bounds().height as i32 / 2,
            self.inner.bounds().width / 2,
            fill,
        )
        .with_border_color(BORDER_COLOR);
    }
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

    /// Handle input events.
    ///
    /// Clicks inside the circle cycle the mode and emit a `ChangeMode` action.
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
            UiEvent::MouseMove { .. } => self.inner.handle_event(event),
            UiEvent::MouseClick { .. } => {
                let resp = self.inner.handle_event(event);
                if resp == EventResponse::Consumed {
                    self.mode = (self.mode + 1) % 3;
                    self.apply_mode_visuals();
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
    /// # Arguments
    ///
    /// * `ctx` - Mutable render context (canvas + graphics cache).
    ///
    /// # Returns
    ///
    /// `Ok(())` on success, or an SDL2 error string.
    fn render(&mut self, ctx: &mut RenderContext<'_, '_>) -> Result<(), String> {
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
            font_cache::TextStyle::PLAIN,
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
    fn hover_is_still_forwarded_to_inner_button() {
        let mut btn = ModeButton::new(100, 100, 18);
        let resp = btn.handle_event(&UiEvent::MouseMove { x: 100, y: 100 });
        assert_eq!(resp, EventResponse::Ignored);
        assert!(btn.inner.is_hovered());
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
