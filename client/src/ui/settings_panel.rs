//! Settings / options panel.
//!
//! Currently renders as a blank semi-transparent window. Will eventually
//! replace the egui-based escape menu with native Widget-based controls.

use sdl2::pixels::Color;
use sdl2::render::BlendMode;

use super::widget::{Bounds, EventResponse, UiEvent, Widget};
use super::RenderContext;
use crate::font_cache;

/// The settings / options HUD panel.
///
/// Toggleable via the HUD button bar. When visible, draws a semi-transparent
/// background and title text. Consumes clicks inside its bounds to prevent
/// them from passing through to the game world.
pub struct SettingsPanel {
    bounds: Bounds,
    bg_color: Color,
    border_color: Color,
    visible: bool,
}

impl SettingsPanel {
    /// Creates a new settings panel.
    ///
    /// # Arguments
    ///
    /// * `bounds` - Position and size.
    /// * `bg_color` - Semi-transparent background color.
    ///
    /// # Returns
    ///
    /// A new `SettingsPanel`, initially hidden.
    pub fn new(bounds: Bounds, bg_color: Color) -> Self {
        Self {
            bounds,
            bg_color,
            border_color: Color::RGBA(120, 120, 140, 200),
            visible: false,
        }
    }

    /// Toggles the panel's visibility.
    pub fn toggle(&mut self) {
        self.visible = !self.visible;
    }

    /// Returns whether the panel is currently visible.
    pub fn is_visible(&self) -> bool {
        self.visible
    }
}

impl Widget for SettingsPanel {
    fn bounds(&self) -> &Bounds {
        &self.bounds
    }

    fn set_position(&mut self, x: i32, y: i32) {
        self.bounds.x = x;
        self.bounds.y = y;
    }

    fn handle_event(&mut self, event: &UiEvent) -> EventResponse {
        if !self.visible {
            return EventResponse::Ignored;
        }
        match event {
            UiEvent::MouseClick { x, y, .. } | UiEvent::MouseWheel { x, y, .. } => {
                if self.bounds.contains_point(*x, *y) {
                    EventResponse::Consumed
                } else {
                    EventResponse::Ignored
                }
            }
            _ => EventResponse::Ignored,
        }
    }

    fn render(&mut self, ctx: &mut RenderContext) -> Result<(), String> {
        if !self.visible {
            return Ok(());
        }

        let rect = sdl2::rect::Rect::new(
            self.bounds.x,
            self.bounds.y,
            self.bounds.width,
            self.bounds.height,
        );

        // Semi-transparent background
        ctx.canvas.set_blend_mode(BlendMode::Blend);
        ctx.canvas.set_draw_color(self.bg_color);
        ctx.canvas.fill_rect(rect)?;

        // Border
        ctx.canvas.set_draw_color(self.border_color);
        ctx.canvas.draw_rect(rect)?;

        // Title
        let title_x = self.bounds.x + self.bounds.width as i32 / 2;
        let title_y = self.bounds.y + 6;
        font_cache::draw_text_centered(ctx.canvas, ctx.gfx, 1, "Settings", title_x, title_y)?;

        Ok(())
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
    fn starts_hidden() {
        let panel = SettingsPanel::new(Bounds::new(0, 0, 100, 100), Color::RGBA(0, 0, 0, 180));
        assert!(!panel.is_visible());
    }

    #[test]
    fn toggle_flips_visibility() {
        let mut panel = SettingsPanel::new(Bounds::new(0, 0, 100, 100), Color::RGBA(0, 0, 0, 180));
        panel.toggle();
        assert!(panel.is_visible());
        panel.toggle();
        assert!(!panel.is_visible());
    }

    #[test]
    fn hidden_panel_ignores_clicks() {
        let mut panel =
            SettingsPanel::new(Bounds::new(10, 10, 100, 100), Color::RGBA(0, 0, 0, 180));
        let resp = panel.handle_event(&UiEvent::MouseClick {
            x: 50,
            y: 50,
            button: MouseButton::Left,
            modifiers: KeyModifiers::default(),
        });
        assert_eq!(resp, EventResponse::Ignored);
    }

    #[test]
    fn visible_panel_consumes_clicks_inside() {
        let mut panel =
            SettingsPanel::new(Bounds::new(10, 10, 100, 100), Color::RGBA(0, 0, 0, 180));
        panel.toggle();
        let resp = panel.handle_event(&UiEvent::MouseClick {
            x: 50,
            y: 50,
            button: MouseButton::Left,
            modifiers: KeyModifiers::default(),
        });
        assert_eq!(resp, EventResponse::Consumed);
    }

    #[test]
    fn visible_panel_ignores_clicks_outside() {
        let mut panel =
            SettingsPanel::new(Bounds::new(10, 10, 100, 100), Color::RGBA(0, 0, 0, 180));
        panel.toggle();
        let resp = panel.handle_event(&UiEvent::MouseClick {
            x: 0,
            y: 0,
            button: MouseButton::Left,
            modifiers: KeyModifiers::default(),
        });
        assert_eq!(resp, EventResponse::Ignored);
    }
}
