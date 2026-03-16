//! A composite widget that arranges three circular buttons on an arc at the
//! bottom-center of the screen. Each button toggles a corresponding HUD panel.

use sdl2::pixels::Color;

use super::button::CircleButton;
use super::widget::{Bounds, EventResponse, HudPanel, UiEvent, Widget, WidgetAction};
use super::RenderContext;

/// Default fill color for the HUD buttons (semi-transparent dark slate).
const BUTTON_FILL: Color = Color::RGBA(20, 20, 40, 200);

/// Default border color for the HUD buttons.
const BUTTON_BORDER: Color = Color::RGBA(140, 140, 160, 220);

/// The three angular positions (in degrees) where buttons are placed on the
/// arc. 210°, 270°, 330° give an even spread across the bottom half.
const BUTTON_ANGLES_DEG: [f64; 3] = [210.0, 270.0, 330.0];

/// Three circular buttons arranged on an invisible arc.
///
/// Clicking a button produces a [`WidgetAction::TogglePanel`] action that the
/// owning scene can drain to toggle the corresponding panel's visibility.
pub struct HudButtonBar {
    buttons: [CircleButton; 3],
    panel_kinds: [HudPanel; 3],
    pending_actions: Vec<WidgetAction>,
    /// Cached bounding box that encloses all three buttons.
    bounds: Bounds,
}

impl HudButtonBar {
    /// Creates a new button bar.
    ///
    /// # Arguments
    ///
    /// * `arc_center_x` - X center of the invisible layout circle.
    /// * `arc_center_y` - Y center of the invisible layout circle.
    /// * `arc_radius` - Radius of the layout circle (distance from center to
    ///   each button center).
    /// * `button_radius` - Radius of each individual circular button.
    /// * `sprite_ids` - Sprite IDs for [Skills, Inventory, Settings] buttons.
    ///
    /// # Returns
    ///
    /// A new `HudButtonBar` ready for rendering.
    pub fn new(
        arc_center_x: i32,
        arc_center_y: i32,
        arc_radius: u32,
        button_radius: u32,
        sprite_ids: [usize; 3],
    ) -> Self {
        let panel_kinds = [HudPanel::Skills, HudPanel::Inventory, HudPanel::Settings];

        let positions = Self::compute_positions(arc_center_x, arc_center_y, arc_radius);

        let buttons = [
            CircleButton::new(positions[0].0, positions[0].1, button_radius, BUTTON_FILL)
                .with_border_color(BUTTON_BORDER)
                .with_sprite(sprite_ids[0]),
            CircleButton::new(positions[1].0, positions[1].1, button_radius, BUTTON_FILL)
                .with_border_color(BUTTON_BORDER)
                .with_sprite(sprite_ids[1]),
            CircleButton::new(positions[2].0, positions[2].1, button_radius, BUTTON_FILL)
                .with_border_color(BUTTON_BORDER)
                .with_sprite(sprite_ids[2]),
        ];

        let bounds = Self::enclosing_bounds(&positions, button_radius);

        Self {
            buttons,
            panel_kinds,
            pending_actions: Vec::new(),
            bounds,
        }
    }

    /// Computes the (cx, cy) center positions for each button on the arc.
    ///
    /// # Arguments
    ///
    /// * `arc_cx` - X center of the arc.
    /// * `arc_cy` - Y center of the arc.
    /// * `arc_r` - Arc radius.
    ///
    /// # Returns
    ///
    /// An array of three `(i32, i32)` center positions.
    fn compute_positions(arc_cx: i32, arc_cy: i32, arc_r: u32) -> [(i32, i32); 3] {
        let r = arc_r as f64;
        let mut out = [(0i32, 0i32); 3];
        for (i, deg) in BUTTON_ANGLES_DEG.iter().enumerate() {
            let rad = deg.to_radians();
            let cx = arc_cx as f64 + r * rad.cos();
            let cy = arc_cy as f64 + r * rad.sin();
            out[i] = (cx.round() as i32, cy.round() as i32);
        }
        out
    }

    /// Computes the smallest axis-aligned bounding box that encloses all three
    /// button circles.
    fn enclosing_bounds(positions: &[(i32, i32); 3], button_r: u32) -> Bounds {
        let r = button_r as i32;
        let min_x = positions.iter().map(|(x, _)| x - r).min().unwrap();
        let min_y = positions.iter().map(|(_, y)| y - r).min().unwrap();
        let max_x = positions.iter().map(|(x, _)| x + r).max().unwrap();
        let max_y = positions.iter().map(|(_, y)| y + r).max().unwrap();
        Bounds::new(min_x, min_y, (max_x - min_x) as u32, (max_y - min_y) as u32)
    }
}

impl Widget for HudButtonBar {
    fn bounds(&self) -> &Bounds {
        &self.bounds
    }

    fn set_position(&mut self, x: i32, y: i32) {
        self.bounds.x = x;
        self.bounds.y = y;
    }

    fn handle_event(&mut self, event: &UiEvent) -> EventResponse {
        for (i, btn) in self.buttons.iter_mut().enumerate() {
            if btn.handle_event(event) == EventResponse::Consumed {
                self.pending_actions
                    .push(WidgetAction::TogglePanel(self.panel_kinds[i]));
                return EventResponse::Consumed;
            }
        }
        EventResponse::Ignored
    }

    fn render(&mut self, ctx: &mut RenderContext<'_, '_>) -> Result<(), String> {
        for btn in &mut self.buttons {
            btn.render(ctx)?;
        }
        Ok(())
    }

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
    fn compute_positions_120_degree_spread() {
        // Arc at (100, 100) with radius 50.
        let positions = HudButtonBar::compute_positions(100, 100, 50);

        // In screen coordinates (Y-down), sin(270°) = -1 moves the button
        // upward.  Arc center (100,100) + radius 50 → (100, 50).
        assert_eq!(positions[1].0, 100);
        assert_eq!(positions[1].1, 50);

        // 210° and 330° should be symmetric around x = 100
        assert_eq!(positions[0].1, positions[2].1);
        assert!(positions[0].0 < 100);
        assert!(positions[2].0 > 100);
    }

    #[test]
    fn enclosing_bounds_covers_all_buttons() {
        let positions = HudButtonBar::compute_positions(200, 200, 60);
        let bounds = HudButtonBar::enclosing_bounds(&positions, 16);

        for (cx, cy) in &positions {
            assert!(bounds.contains_point(*cx, *cy));
        }
    }

    #[test]
    fn click_produces_toggle_action() {
        let bar = HudButtonBar::new(200, 200, 60, 16, [267, 128, 35]);
        // The center button (270°) is at (200, 260).
        let positions = HudButtonBar::compute_positions(200, 200, 60);
        let (cx, cy) = positions[1]; // Inventory button

        let mut bar = bar;
        let resp = bar.handle_event(&UiEvent::MouseClick {
            x: cx,
            y: cy,
            button: MouseButton::Left,
            modifiers: KeyModifiers::default(),
        });
        assert_eq!(resp, EventResponse::Consumed);

        let actions = bar.take_actions();
        assert_eq!(actions.len(), 1);
        match &actions[0] {
            WidgetAction::TogglePanel(HudPanel::Inventory) => {}
            other => panic!("Expected TogglePanel(Inventory), got {:?}", other),
        }
    }

    #[test]
    fn click_outside_all_buttons_ignored() {
        let mut bar = HudButtonBar::new(200, 200, 60, 16, [267, 128, 35]);
        let resp = bar.handle_event(&UiEvent::MouseClick {
            x: 0,
            y: 0,
            button: MouseButton::Left,
            modifiers: KeyModifiers::default(),
        });
        assert_eq!(resp, EventResponse::Ignored);
        assert!(bar.take_actions().is_empty());
    }

    #[test]
    fn take_actions_drains() {
        let mut bar = HudButtonBar::new(200, 200, 60, 16, [267, 128, 35]);
        let positions = HudButtonBar::compute_positions(200, 200, 60);
        let (cx, cy) = positions[0]; // Skills button

        bar.handle_event(&UiEvent::MouseClick {
            x: cx,
            y: cy,
            button: MouseButton::Left,
            modifiers: KeyModifiers::default(),
        });
        assert_eq!(bar.take_actions().len(), 1);
        // Second drain should be empty
        assert!(bar.take_actions().is_empty());
    }
}
