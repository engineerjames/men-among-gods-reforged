//! A toggleable minimap button + viewport widget.
//!
//! Displays a circular button near the top-right of the screen. When clicked
//! the button opens a framed minimap viewport anchored just below. Clicking
//! again hides the viewport.

use sdl2::pixels::Color;

use super::button::CircleButton;
use super::widget::{Bounds, EventResponse, UiEvent, Widget};
use super::RenderContext;

// ---------------------------------------------------------------------------
// Layout constants
// ---------------------------------------------------------------------------

/// Size (width and height) of the minimap viewport in pixels.
///
/// Change this single value to resize the expanded map display — all layout
/// math derives from it.
pub const MINIMAP_WIDGET_VIEW_SIZE: u32 = 128;

/// Full world-map dimension used to index the xmap buffer (1024×1024).
const WORLD_SIZE: usize = 1024;

/// Padding between the map pixel area and the outer panel border (pixels).
const PANEL_PADDING: u32 = 3;

/// Border thickness around the panel (pixels).
const PANEL_BORDER: u32 = 1;

/// Vertical gap between the bottom of the circle button and the top of the
/// map panel (pixels).
const BUTTON_MAP_GAP: i32 = 4;

/// Background color for the panel area behind the minimap pixels.
const PANEL_BG: Color = Color::RGBA(10, 10, 30, 200);

/// Border color for the panel frame.
const PANEL_BORDER_COLOR: Color = Color::RGBA(120, 120, 140, 220);

/// Fill color for the toggle circle button.
const BUTTON_FILL: Color = Color::RGBA(20, 20, 40, 200);

/// Border color for the toggle circle button.
const BUTTON_BORDER: Color = Color::RGBA(120, 120, 140, 220);

/// Sprite ID drawn centered inside the toggle button.
const BUTTON_SPRITE: usize = 1231;

// ---------------------------------------------------------------------------
// MinimapWidget
// ---------------------------------------------------------------------------

/// A circular toggle button that reveals a framed minimap viewport when open.
///
/// The heavy 1024×1024 xmap buffer lives on `GameScene`. Each frame the scene
/// calls [`MinimapWidget::update_viewport`] to push the 128×128 (or whatever
/// [`MINIMAP_WIDGET_VIEW_SIZE`] is set to) viewport pixels extracted from that
/// buffer. The widget then blits those pixels to the screen during
/// [`Widget::render`].
pub struct MinimapWidget {
    /// The circle toggle button.
    button: CircleButton,
    /// Whether the expanded map panel is currently shown.
    visible: bool,
    /// Pre-allocated pixel buffer for the viewport (VIEW_SIZE² × 4 bytes RGBA).
    viewport_pixels: Vec<u8>,
    /// True when `viewport_pixels` contains valid data ready to blit.
    viewport_dirty: bool,
    /// Top-left X of the map panel (centered below the button).
    panel_x: i32,
    /// Top-left Y of the map panel (anchored below button + gap).
    panel_y: i32,
    /// Total panel width including padding + border.
    panel_w: u32,
    /// Total panel height including padding + border.
    panel_h: u32,
    /// Bounds enclosing button + panel when expanded (for hit-testing).
    bounds_expanded: Bounds,
    /// Bounds enclosing only the button when collapsed.
    bounds_collapsed: Bounds,
}

impl MinimapWidget {
    /// Create a new minimap widget.
    ///
    /// # Arguments
    ///
    /// * `button_cx` - X center of the toggle button in logical pixels.
    /// * `button_cy` - Y center of the toggle button in logical pixels.
    /// * `button_radius` - Radius of the toggle button in pixels.
    ///
    /// # Returns
    ///
    /// A new `MinimapWidget` with the viewport hidden.
    pub fn new(button_cx: i32, button_cy: i32, button_radius: u32) -> Self {
        let button = CircleButton::new(button_cx, button_cy, button_radius, BUTTON_FILL)
            .with_border_color(BUTTON_BORDER)
            .with_sprite(BUTTON_SPRITE);

        let view = MINIMAP_WIDGET_VIEW_SIZE;
        let panel_w = view + 2 * (PANEL_PADDING + PANEL_BORDER);
        let panel_h = panel_w; // square

        // Center the panel horizontally below the button, clamped to screen.
        let screen_w = crate::constants::TARGET_WIDTH_INT as i32;
        let panel_x = (button_cx - panel_w as i32 / 2).min(screen_w - panel_w as i32);
        let panel_y = button_cy + button_radius as i32 + BUTTON_MAP_GAP;

        let bounds_collapsed = Bounds::new(
            button_cx - button_radius as i32,
            button_cy - button_radius as i32,
            button_radius * 2,
            button_radius * 2,
        );

        let total_h = (panel_y + panel_h as i32) - bounds_collapsed.y;
        let min_x = bounds_collapsed.x.min(panel_x);
        let max_x =
            (bounds_collapsed.x + bounds_collapsed.width as i32).max(panel_x + panel_w as i32);
        let bounds_expanded = Bounds::new(
            min_x,
            bounds_collapsed.y,
            (max_x - min_x) as u32,
            total_h as u32,
        );

        Self {
            button,
            visible: false,
            viewport_pixels: vec![0u8; (view as usize) * (view as usize) * 4],
            viewport_dirty: false,
            panel_x,
            panel_y,
            panel_w,
            panel_h,
            bounds_expanded,
            bounds_collapsed,
        }
    }

    /// Toggle the map viewport open or closed.
    pub fn toggle(&mut self) {
        self.visible = !self.visible;
    }

    /// Returns whether the map viewport is currently visible.
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Extract the viewport from the full-world xmap buffer and store it for
    /// the next render pass.
    ///
    /// This performs the same viewport extraction that the old `draw_minimap()`
    /// did: a [`MINIMAP_WIDGET_VIEW_SIZE`]² window centered on the player
    /// position, clamped to world bounds.
    ///
    /// # Arguments
    ///
    /// * `xmap` - The 1024×1024×4 RGBA world buffer.
    /// * `center_x` - Player X in world-map coordinates.
    /// * `center_y` - Player Y in world-map coordinates.
    pub fn update_viewport(&mut self, xmap: &[u8], center_x: u16, center_y: u16) {
        if !self.visible {
            return;
        }

        let view = MINIMAP_WIDGET_VIEW_SIZE as usize;
        let half = (MINIMAP_WIDGET_VIEW_SIZE as i32) / 2;
        let mapx = ((center_x as i32) - half)
            .clamp(0, WORLD_SIZE as i32 - MINIMAP_WIDGET_VIEW_SIZE as i32);
        let mapy = ((center_y as i32) - half)
            .clamp(0, WORLD_SIZE as i32 - MINIMAP_WIDGET_VIEW_SIZE as i32);

        // C call convention: dd_show_map(xmap, mapy, mapx).
        // xo = mapy (column offset in global Y), yo = mapx (row offset in global X).
        let xo = mapy as usize;
        let yo = mapx as usize;

        for row in 0..view {
            for col in 0..view {
                let src_row = yo + row;
                let src_col = xo + col;
                if src_row >= WORLD_SIZE || src_col >= WORLD_SIZE {
                    continue;
                }
                let src = (src_row * WORLD_SIZE + src_col) * 4;
                let dst = (row * view + col) * 4;
                self.viewport_pixels[dst] = xmap[src];
                self.viewport_pixels[dst + 1] = xmap[src + 1];
                self.viewport_pixels[dst + 2] = xmap[src + 2];
                self.viewport_pixels[dst + 3] = xmap[src + 3];
            }
        }

        self.viewport_dirty = true;
    }

    /// Returns `true` if `(px, py)` lands inside the map panel rectangle.
    #[allow(dead_code)]
    fn panel_contains(&self, px: i32, py: i32) -> bool {
        px >= self.panel_x
            && py >= self.panel_y
            && px < self.panel_x + self.panel_w as i32
            && py < self.panel_y + self.panel_h as i32
    }
}

impl Widget for MinimapWidget {
    /// Returns the bounding rectangle — expanded when the map is visible,
    /// collapsed (button only) when hidden.
    fn bounds(&self) -> &Bounds {
        if self.visible {
            &self.bounds_expanded
        } else {
            &self.bounds_collapsed
        }
    }

    fn set_position(&mut self, x: i32, y: i32) {
        let dx = x - self.bounds_collapsed.x;
        let dy = y - self.bounds_collapsed.y;
        self.bounds_collapsed.x = x;
        self.bounds_collapsed.y = y;
        self.bounds_expanded.x += dx;
        self.bounds_expanded.y += dy;
        self.panel_x += dx;
        self.panel_y += dy;
        self.button.set_position(x, y);
    }

    fn handle_event(&mut self, event: &UiEvent) -> EventResponse {
        // Always delegate mouse-move to the button for hover tracking.
        if let UiEvent::MouseMove { .. } = event {
            self.button.handle_event(event);
        }

        // Button click toggles visibility.
        if let UiEvent::MouseClick { .. } = event {
            if self.button.handle_event(event) == EventResponse::Consumed {
                self.toggle();
                return EventResponse::Consumed;
            }
        }

        EventResponse::Ignored
    }

    fn render(&mut self, ctx: &mut RenderContext<'_, '_>) -> Result<(), String> {
        // Always draw the toggle button.
        self.button.render(ctx)?;

        if !self.visible {
            return Ok(());
        }

        let view = MINIMAP_WIDGET_VIEW_SIZE;
        let inset = PANEL_PADDING + PANEL_BORDER;

        // Panel background.
        ctx.canvas.set_draw_color(PANEL_BG);
        ctx.canvas.fill_rect(sdl2::rect::Rect::new(
            self.panel_x,
            self.panel_y,
            self.panel_w,
            self.panel_h,
        ))?;

        // Panel border.
        ctx.canvas.set_draw_color(PANEL_BORDER_COLOR);
        ctx.canvas.draw_rect(sdl2::rect::Rect::new(
            self.panel_x,
            self.panel_y,
            self.panel_w,
            self.panel_h,
        ))?;

        // Blit the minimap viewport pixels.
        if self.viewport_dirty {
            let map_x = self.panel_x + inset as i32;
            let map_y = self.panel_y + inset as i32;

            // Ensure we are in normal alpha-blend mode; prior render passes
            // (e.g. spell effects) may have left the canvas in Add or None.
            ctx.canvas.set_blend_mode(sdl2::render::BlendMode::Blend);

            ctx.gfx.ensure_minimap_texture();
            if let Some(tex) = ctx.gfx.minimap_texture.as_mut() {
                let pitch = view as usize * 4;
                tex.update(None, &self.viewport_pixels, pitch)
                    .map_err(|e| e.to_string())?;
                ctx.canvas.copy(
                    tex,
                    None,
                    Some(sdl2::rect::Rect::new(map_x, map_y, view, view)),
                )?;
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::widget::MouseButton;

    #[test]
    fn toggle_changes_visibility() {
        let mut w = MinimapWidget::new(100, 30, 14);
        assert!(!w.is_visible());
        w.toggle();
        assert!(w.is_visible());
        w.toggle();
        assert!(!w.is_visible());
    }

    #[test]
    fn bounds_differ_when_expanded() {
        let w_collapsed = MinimapWidget::new(100, 30, 14);
        let collapsed = *w_collapsed.bounds();

        let mut w_expanded = MinimapWidget::new(100, 30, 14);
        w_expanded.toggle();
        let expanded = *w_expanded.bounds();

        // Expanded bounds must be taller (includes the map panel).
        assert!(expanded.height > collapsed.height);
    }

    #[test]
    fn panel_hit_test() {
        let mut w = MinimapWidget::new(200, 30, 14);
        w.toggle();

        // Point inside the panel area should be contained.
        assert!(w.panel_contains(w.panel_x + 5, w.panel_y + 5));
        // Point outside should not.
        assert!(!w.panel_contains(0, 0));
    }

    #[test]
    fn update_viewport_no_op_when_hidden() {
        let mut w = MinimapWidget::new(100, 30, 14);
        let xmap = vec![0u8; WORLD_SIZE * WORLD_SIZE * 4];
        w.update_viewport(&xmap, 512, 512);
        // viewport_dirty should remain false when hidden.
        assert!(!w.viewport_dirty);
    }

    #[test]
    fn update_viewport_sets_dirty_when_visible() {
        let mut w = MinimapWidget::new(100, 30, 14);
        w.toggle();
        let xmap = vec![42u8; WORLD_SIZE * WORLD_SIZE * 4];
        w.update_viewport(&xmap, 512, 512);
        assert!(w.viewport_dirty);
        // Spot-check that pixels were copied.
        assert_eq!(w.viewport_pixels[0], 42);
    }

    #[test]
    fn click_outside_is_ignored() {
        let mut w = MinimapWidget::new(200, 30, 14);
        let click = UiEvent::MouseClick {
            x: 0,
            y: 0,
            button: MouseButton::Left,
            modifiers: Default::default(),
        };
        assert_eq!(w.handle_event(&click), EventResponse::Ignored);
    }

    #[test]
    fn click_on_button_toggles() {
        let mut w = MinimapWidget::new(200, 30, 14);
        assert!(!w.is_visible());
        let click = UiEvent::MouseClick {
            x: 200,
            y: 30,
            button: MouseButton::Left,
            modifiers: Default::default(),
        };
        assert_eq!(w.handle_event(&click), EventResponse::Consumed);
        assert!(w.is_visible());
        assert_eq!(w.handle_event(&click), EventResponse::Consumed);
        assert!(!w.is_visible());
    }

    #[test]
    fn click_on_panel_ignored_when_visible() {
        let mut w = MinimapWidget::new(200, 30, 14);
        w.toggle();
        let click = UiEvent::MouseClick {
            x: w.panel_x + 10,
            y: w.panel_y + 10,
            button: MouseButton::Left,
            modifiers: Default::default(),
        };
        assert_eq!(w.handle_event(&click), EventResponse::Ignored);
    }
}
