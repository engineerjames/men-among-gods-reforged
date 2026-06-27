//! Circular speed-mode button.
//!
//! Holds three [`CircularImageButton`] structs (one per speed mode) and swaps
//! which one is active based on the current mode. Clicking the active button:
//! - Cycles the mode (Slow --> Normal --> Fast --> Slow)
//! - Switches the rendered image to match the new mode
//! - Emits a `WidgetAction::ChangeMode`

use crate::filepaths;

use crate::ui::RenderContext;
use crate::ui::widget::{Bounds, EventResponse, UiEvent, Widget, WidgetAction};
use crate::ui::widgets::button::CircularImageButton;

// ---------------------------------------------------------------------------
// Per-mode visuals
// ---------------------------------------------------------------------------

/// Number of selectable speed modes.
const MODE_COUNT: usize = 3;

/// Whole-button image filenames indexed by mode (0 = Slow, 1 = Normal, 2 = Fast).
const MODE_IMAGE_FILES: [&str; MODE_COUNT] = ["slow.png", "normal.png", "fast.png"];

/// Full mode names used for the hover tooltip.
const MODE_HOVER_TEXTS: [&str; MODE_COUNT] = ["Slow mode", "Normal mode", "Fast mode"];

// ---------------------------------------------------------------------------
// Widget
// ---------------------------------------------------------------------------

/// A circular button in the lower-right corner that cycles through speed modes.
pub struct ModeButton {
    /// One image button per mode; only the active mode's button is rendered.
    buttons: [CircularImageButton; MODE_COUNT],
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
        let button_dir = filepaths::get_asset_directory().join("gfx").join("buttons");
        let buttons = std::array::from_fn(|i| {
            CircularImageButton::new(
                center_x,
                center_y,
                radius,
                button_dir.join(MODE_IMAGE_FILES[i]),
            )
        });
        Self {
            buttons,
            mode: 1,
            pending_actions: Vec::new(),
        }
    }

    /// Returns the index of the currently active mode, clamped to a valid range.
    fn active_index(&self) -> usize {
        self.mode.clamp(0, MODE_COUNT as i32 - 1) as usize
    }

    /// Returns a shared reference to the currently active button.
    fn active(&self) -> &CircularImageButton {
        &self.buttons[self.active_index()]
    }

    /// Returns a mutable reference to the currently active button.
    fn active_mut(&mut self) -> &mut CircularImageButton {
        let idx = self.active_index();
        &mut self.buttons[idx]
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
        self.mode = mode.clamp(0, MODE_COUNT as i32 - 1);
    }

    /// Returns the hover tooltip describing the current mode, or `None` when
    /// the button is not under the cursor.
    ///
    /// # Returns
    ///
    /// * `Some("Slow mode" | "Normal mode" | "Fast mode")` while hovered,
    ///   or `None`.
    pub fn hover_text(&self) -> Option<&'static str> {
        if self.active().is_hovered() {
            Some(MODE_HOVER_TEXTS[self.active_index()])
        } else {
            None
        }
    }

    /// Sets the draw opacity for all mode buttons.
    ///
    /// # Arguments
    ///
    /// * `alpha` - Opacity value (0 = invisible, 255 = fully opaque).
    pub fn set_alpha(&mut self, alpha: u8) {
        for btn in &mut self.buttons {
            btn.set_draw_alpha(alpha);
        }
    }
}

impl Widget for ModeButton {
    /// Returns the bounding rectangle of the active circle.
    fn bounds(&self) -> &Bounds {
        self.active().bounds()
    }

    /// Moves every button's top-left corner so they stay co-located.
    ///
    /// # Arguments
    ///
    /// * `x` - New left edge.
    /// * `y` - New top edge.
    fn set_position(&mut self, x: i32, y: i32) {
        for button in &mut self.buttons {
            button.set_position(x, y);
        }
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
            UiEvent::MouseMove { .. } => self.active_mut().handle_event(event),
            UiEvent::MouseClick { .. } => {
                let resp = self.active_mut().handle_event(event);
                if resp == EventResponse::Consumed {
                    self.mode = (self.mode + 1) % MODE_COUNT as i32;
                    self.pending_actions
                        .push(WidgetAction::ChangeMode(self.mode));
                }
                resp
            }
            _ => EventResponse::Ignored,
        }
    }

    /// Draw the active mode button.
    ///
    /// # Arguments
    ///
    /// * `ctx` - Mutable render context (canvas + graphics cache).
    ///
    /// # Returns
    ///
    /// `Ok(())` on success, or an SDL2 error string.
    fn render(&mut self, ctx: &mut RenderContext<'_, '_>) -> Result<(), String> {
        self.active_mut().render(ctx)
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
    fn hover_is_still_forwarded_to_active_button() {
        let mut btn = ModeButton::new(100, 100, 18);
        let resp = btn.handle_event(&UiEvent::MouseMove { x: 100, y: 100 });
        assert_eq!(resp, EventResponse::Ignored);
        assert!(btn.active().is_hovered());
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

    #[test]
    fn hover_text_none_when_not_hovered() {
        let btn = ModeButton::new(100, 100, 18);
        assert_eq!(btn.hover_text(), None);
    }

    #[test]
    fn hover_text_reports_current_mode() {
        let hover = UiEvent::MouseMove { x: 100, y: 100 };
        let mut btn = ModeButton::new(100, 100, 18);
        btn.handle_event(&hover);
        assert_eq!(btn.hover_text(), Some("Normal mode"));
        btn.sync(0);
        btn.handle_event(&hover);
        assert_eq!(btn.hover_text(), Some("Slow mode"));
        btn.sync(2);
        btn.handle_event(&hover);
        assert_eq!(btn.hover_text(), Some("Fast mode"));
    }
}
