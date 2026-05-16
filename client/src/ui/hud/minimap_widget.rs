//! A toggleable minimap button + viewport widget.
//!
//! Displays a circular button near the top-right of the screen. When clicked
//! the button opens a framed minimap viewport anchored to the left of the
//! button. Clicking again hides the viewport.

use sdl2::pixels::Color;

use crate::ui::RenderContext;
use crate::ui::style::{Background, Border};
use crate::ui::widget::{Bounds, EventResponse, UiEvent, Widget};
use crate::ui::widgets::button::{CircleButton, RectButton};

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
const PANEL_PADDING: u32 = 0;

/// Border thickness around the panel (pixels).
const PANEL_BORDER: u32 = 1;

/// Horizontal gap between the right edge of the map panel and the left edge
/// of the circle button (pixels).
const BUTTON_MAP_GAP: i32 = 4;

/// Width of each zoom control button in pixels.
const ZOOM_BUTTON_W: u32 = 16;

/// Height of each zoom control button in pixels.
const ZOOM_BUTTON_H: u32 = 16;

/// Horizontal gap between the zoom buttons.
const ZOOM_BUTTON_GAP: i32 = 4;

/// Vertical gap between the zoom buttons and the minimap panel.
const ZOOM_BUTTON_PANEL_GAP: i32 = 4;

/// Horizontal pixel offset applied to the minimap zoom button labels.
const ZOOM_LABEL_OFFSET_X: i32 = 1;

/// Vertical pixel offset applied to the minimap zoom button labels.
const ZOOM_LABEL_OFFSET_Y: i32 = 1;

/// World-window sizes sampled into the fixed minimap viewport for each zoom level.
const ZOOM_SAMPLE_SIZES: [u32; 5] = [64, 96, 128, 160, 192];

/// Default zoom level preserving the current 128×128 sampling behavior.
const DEFAULT_ZOOM_LEVEL: usize = 2;

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
    /// Button that reduces the sampled world area, magnifying the minimap.
    zoom_in_button: RectButton,
    /// Button that increases the sampled world area, zooming the minimap out.
    zoom_out_button: RectButton,
    /// Whether the expanded map panel is currently shown.
    visible: bool,
    /// Index into [`ZOOM_SAMPLE_SIZES`] selecting the current zoom level.
    zoom_level: usize,
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
    /// World-tile positions of all NPC quest givers in the player's quest log.
    quest_giver_markers: Vec<(u16, u16)>,
    /// World-tile position of the destination of the focused quest, if any.
    active_quest_marker: Option<(u16, u16)>,
    /// World tile X of the most recent viewport center, captured by
    /// `update_viewport` and reused when projecting quest markers.
    last_center_x: u16,
    /// World tile Y of the most recent viewport center.
    last_center_y: u16,
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

        let bounds_collapsed = Bounds::new(
            button_cx - button_radius as i32,
            button_cy - button_radius as i32,
            button_radius * 2,
            button_radius * 2,
        );

        let mut widget = Self {
            button,
            zoom_in_button: Self::make_zoom_button("+"),
            zoom_out_button: Self::make_zoom_button("-"),
            visible: false,
            zoom_level: DEFAULT_ZOOM_LEVEL,
            viewport_pixels: vec![0u8; (view as usize) * (view as usize) * 4],
            viewport_dirty: false,
            panel_x: 0,
            panel_y: 0,
            panel_w,
            panel_h,
            bounds_expanded: bounds_collapsed,
            bounds_collapsed,
            quest_giver_markers: Vec::new(),
            active_quest_marker: None,
            last_center_x: 0,
            last_center_y: 0,
        };
        widget.recompute_layout();
        widget
    }

    /// Toggle the map viewport open or closed.
    pub fn toggle(&mut self) {
        self.visible = !self.visible;
        self.zoom_in_button.set_hovered(false);
        self.zoom_out_button.set_hovered(false);
    }

    /// Returns whether the map viewport is currently visible.
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Returns the world-window size currently sampled into the viewport.
    fn current_sample_size(&self) -> usize {
        ZOOM_SAMPLE_SIZES[self.zoom_level] as usize
    }

    /// Creates a small labeled button used for minimap zoom controls.
    fn make_zoom_button(label: &str) -> RectButton {
        // TODO: Clean this up...
        if label == "+" {
            RectButton::new(
                Bounds::new(0, 0, ZOOM_BUTTON_W, ZOOM_BUTTON_H),
                Background::SolidColor(BUTTON_FILL),
            )
            .with_border(Border {
                color: BUTTON_BORDER,
                width: 1,
            })
            .with_label(label, 1)
            .with_label_offset(0, ZOOM_LABEL_OFFSET_Y)
        } else {
            RectButton::new(
                Bounds::new(0, 0, ZOOM_BUTTON_W, ZOOM_BUTTON_H),
                Background::SolidColor(BUTTON_FILL),
            )
            .with_border(Border {
                color: BUTTON_BORDER,
                width: 1,
            })
            .with_label(label, 1)
            .with_label_offset(ZOOM_LABEL_OFFSET_X, ZOOM_LABEL_OFFSET_Y)
        }
    }
    /// Recomputes the minimap panel, zoom-button, and aggregate hit-test bounds.
    fn recompute_layout(&mut self) {
        let button_bounds = *self.button.bounds();
        let button_radius = button_bounds.width / 2;
        let button_cx = button_bounds.x + button_bounds.width as i32 / 2;
        let button_cy = button_bounds.y + button_bounds.height as i32 / 2;

        let screen_w = crate::constants::TARGET_WIDTH_INT as i32;
        let max_panel_x = (screen_w - self.panel_w as i32).max(0);
        // Open to the left: right edge of panel flush with left edge of button,
        // with a small gap. Top of panel aligned with the button center.
        let panel_x = (button_cx - button_radius as i32 - BUTTON_MAP_GAP - self.panel_w as i32)
            .clamp(0, max_panel_x);
        let panel_y = button_cy - button_radius as i32;

        let zoom_y = panel_y - ZOOM_BUTTON_H as i32 - ZOOM_BUTTON_PANEL_GAP;
        let zoom_in_x = panel_x;
        let zoom_out_x = zoom_in_x + ZOOM_BUTTON_W as i32 + ZOOM_BUTTON_GAP;

        self.zoom_in_button.set_position(zoom_in_x, zoom_y);
        self.zoom_out_button.set_position(zoom_out_x, zoom_y);

        self.panel_x = panel_x;
        self.panel_y = panel_y;
        self.bounds_collapsed = button_bounds;

        let zoom_in_bounds = *self.zoom_in_button.bounds();
        let zoom_out_bounds = *self.zoom_out_button.bounds();
        let min_x = button_bounds
            .x
            .min(panel_x)
            .min(zoom_in_bounds.x)
            .min(zoom_out_bounds.x);
        let min_y = button_bounds
            .y
            .min(panel_y)
            .min(zoom_in_bounds.y)
            .min(zoom_out_bounds.y);
        let max_x = (button_bounds.x + button_bounds.width as i32)
            .max(panel_x + self.panel_w as i32)
            .max(zoom_in_bounds.x + zoom_in_bounds.width as i32)
            .max(zoom_out_bounds.x + zoom_out_bounds.width as i32);
        let max_y = (button_bounds.y + button_bounds.height as i32)
            .max(panel_y + self.panel_h as i32)
            .max(zoom_in_bounds.y + zoom_in_bounds.height as i32)
            .max(zoom_out_bounds.y + zoom_out_bounds.height as i32);

        self.bounds_expanded =
            Bounds::new(min_x, min_y, (max_x - min_x) as u32, (max_y - min_y) as u32);
    }

    /// Advances to the next zoomed-in sampling window, clamped at the closest level.
    fn zoom_in(&mut self) {
        if self.zoom_level > 0 {
            self.zoom_level -= 1;
        }
    }

    /// Advances to the next zoomed-out sampling window, clamped at the widest level.
    fn zoom_out(&mut self) {
        if self.zoom_level + 1 < ZOOM_SAMPLE_SIZES.len() {
            self.zoom_level += 1;
        }
    }

    /// Extract the viewport from the full-world xmap buffer and store it for
    /// the next render pass.
    ///
    /// This samples a square world window centered on the player position,
    /// clamped to world bounds, and rescales it into the fixed
    /// [`MINIMAP_WIDGET_VIEW_SIZE`]² output buffer.
    ///
    /// # Arguments
    ///
    /// * `xmap` - The 1024×1024×4 RGBA world buffer.
    /// * `center_x` - Player X in world-map coordinates.
    /// * `center_y` - Player Y in world-map coordinates.
    pub fn update_viewport(&mut self, xmap: &[u8], center_x: u16, center_y: u16) {
        self.last_center_x = center_x;
        self.last_center_y = center_y;
        // Always update the pixel buffer even when hidden, so the minimap
        // displays current data immediately when opened.
        let view = MINIMAP_WIDGET_VIEW_SIZE as usize;
        let sample = self.current_sample_size();
        let half = sample as i32 / 2;
        let mapx = (i32::from(center_x) - half).clamp(0, WORLD_SIZE as i32 - sample as i32);
        let mapy = (i32::from(center_y) - half).clamp(0, WORLD_SIZE as i32 - sample as i32);

        // C call convention: dd_show_map(xmap, mapy, mapx).
        // xo = mapy (column offset in global Y), yo = mapx (row offset in global X).
        let xo = mapy as usize;
        let yo = mapx as usize;

        for row in 0..view {
            let src_row = yo + ((row * sample) / view).min(sample.saturating_sub(1));
            for col in 0..view {
                let src_col = xo + ((col * sample) / view).min(sample.saturating_sub(1));
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

    /// Updates the quest marker overlays drawn on top of the minimap.
    ///
    /// # Arguments
    ///
    /// * `givers` - World-tile positions of every NPC quest giver to mark.
    /// * `active` - World-tile position of the destination of the currently
    ///   focused quest, drawn in a distinct color. `None` clears the active
    ///   marker.
    pub fn set_quest_markers(&mut self, givers: Vec<(u16, u16)>, active: Option<(u16, u16)>) {
        self.quest_giver_markers = givers;
        self.active_quest_marker = active;
    }

    /// Projects a world tile position into screen-space coordinates inside the
    /// minimap viewport, mirroring the math performed by `update_viewport`.
    ///
    /// # Arguments
    ///
    /// * `world_x` - World tile X coordinate.
    /// * `world_y` - World tile Y coordinate.
    ///
    /// # Returns
    ///
    /// * `Some((sx, sy))` when the tile falls inside the visible window;
    ///   `None` when it lies outside the current zoom level's window.
    fn project_world_to_screen(&self, world_x: u16, world_y: u16) -> Option<(i32, i32)> {
        let view = MINIMAP_WIDGET_VIEW_SIZE as i32;
        let sample = self.current_sample_size() as i32;
        let half = sample / 2;
        let mapx = (i32::from(self.last_center_x) - half).clamp(0, WORLD_SIZE as i32 - sample);
        let mapy = (i32::from(self.last_center_y) - half).clamp(0, WORLD_SIZE as i32 - sample);

        // World -> sample-window relative coords.
        let rel_x = i32::from(world_x) - mapx;
        let rel_y = i32::from(world_y) - mapy;
        if rel_x < 0 || rel_y < 0 || rel_x >= sample || rel_y >= sample {
            return None;
        }

        // The xmap is column-major (`cell = (gy + gx * STRIDE)`), and
        // `update_viewport` reads it with `src_row = mapx + row_offset`,
        // `src_col = mapy + col_offset`. So the screen Y axis (rows) tracks
        // world X, and the screen X axis (columns) tracks world Y — the
        // minimap is rotated 90° clockwise relative to the world.
        let inset = (PANEL_PADDING + PANEL_BORDER) as i32;
        let map_x = self.panel_x + inset;
        let map_y = self.panel_y + inset;
        let sx = map_x + (rel_y * view) / sample;
        let sy = map_y + (rel_x * view) / sample;
        Some((sx, sy))
    }

    /// Renders all quest marker overlays on top of the minimap pixels.
    fn render_quest_markers(&self, ctx: &mut RenderContext<'_, '_>) -> Result<(), String> {
        ctx.canvas.set_blend_mode(sdl2::render::BlendMode::Blend);

        // Quest giver markers (yellow).
        ctx.canvas.set_draw_color(Color::RGBA(255, 220, 0, 255));
        for (wx, wy) in &self.quest_giver_markers {
            if let Some((sx, sy)) = self.project_world_to_screen(*wx, *wy) {
                ctx.canvas
                    .fill_rect(sdl2::rect::Rect::new(sx - 1, sy - 1, 2, 2))?;
            }
        }

        // Active quest marker (magenta) drawn on top so it is always visible.
        if let Some((wx, wy)) = self.active_quest_marker {
            if let Some((sx, sy)) = self.project_world_to_screen(wx, wy) {
                ctx.canvas.set_draw_color(Color::RGBA(255, 0, 255, 255));
                ctx.canvas
                    .fill_rect(sdl2::rect::Rect::new(sx - 1, sy - 1, 2, 2))?;
            }
        }

        Ok(())
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
        self.button.set_position(x, y);
        self.recompute_layout();
    }

    fn handle_event(&mut self, event: &UiEvent) -> EventResponse {
        // Always delegate mouse-move to the button for hover tracking.
        if let UiEvent::MouseMove { .. } = event {
            self.button.handle_event(event);
            if self.visible {
                self.zoom_in_button.handle_event(event);
                self.zoom_out_button.handle_event(event);
            }
        }

        // Button click toggles visibility.
        if let UiEvent::MouseClick { .. } = event {
            if self.button.handle_event(event) == EventResponse::Consumed {
                self.toggle();
                return EventResponse::Consumed;
            }

            if self.visible {
                if self.zoom_in_button.handle_event(event) == EventResponse::Consumed {
                    self.zoom_in();
                    return EventResponse::Consumed;
                }
                if self.zoom_out_button.handle_event(event) == EventResponse::Consumed {
                    self.zoom_out();
                    return EventResponse::Consumed;
                }
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

        // Draw quest markers on top of the minimap pixels.
        self.render_quest_markers(ctx)?;

        self.zoom_in_button.render(ctx)?;
        self.zoom_out_button.render(ctx)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::widget::MouseButton;

    fn click_in_bounds(bounds: Bounds) -> UiEvent {
        UiEvent::MouseClick {
            x: bounds.x + bounds.width as i32 / 2,
            y: bounds.y + bounds.height as i32 / 2,
            button: MouseButton::Left,
            modifiers: Default::default(),
        }
    }

    fn fill_gradient_region(
        xmap: &mut [u8],
        row_start: usize,
        row_end: usize,
        col_start: usize,
        col_end: usize,
    ) {
        for row in row_start..row_end {
            for col in col_start..col_end {
                let idx = (row * WORLD_SIZE + col) * 4;
                xmap[idx] = col as u8;
                xmap[idx + 1] = row as u8;
                xmap[idx + 2] = 0;
                xmap[idx + 3] = 255;
            }
        }
    }

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
    fn expanded_bounds_include_zoom_buttons() {
        let mut w = MinimapWidget::new(200, 30, 14);
        w.toggle();
        let expanded = *w.bounds();
        let zoom_in = *w.zoom_in_button.bounds();
        let zoom_out = *w.zoom_out_button.bounds();

        assert!(expanded.contains_point(
            zoom_in.x + zoom_in.width as i32 / 2,
            zoom_in.y + zoom_in.height as i32 / 2,
        ));
        assert!(expanded.contains_point(
            zoom_out.x + zoom_out.width as i32 / 2,
            zoom_out.y + zoom_out.height as i32 / 2,
        ));
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
    fn update_viewport_updates_even_when_hidden() {
        let mut w = MinimapWidget::new(100, 30, 14);
        let xmap = vec![0u8; WORLD_SIZE * WORLD_SIZE * 4];
        w.update_viewport(&xmap, 512, 512);
        assert!(w.viewport_dirty);
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
    fn zoom_buttons_are_ignored_when_hidden() {
        let mut w = MinimapWidget::new(200, 30, 14);
        let click = click_in_bounds(*w.zoom_in_button.bounds());

        assert_eq!(w.handle_event(&click), EventResponse::Ignored);
        assert_eq!(w.current_sample_size(), 128);
    }

    #[test]
    fn zoom_buttons_change_sample_size_when_visible() {
        let mut w = MinimapWidget::new(200, 30, 14);
        w.toggle();

        let zoom_in = click_in_bounds(*w.zoom_in_button.bounds());
        let zoom_out = click_in_bounds(*w.zoom_out_button.bounds());

        assert_eq!(w.current_sample_size(), 128);
        assert_eq!(w.handle_event(&zoom_in), EventResponse::Consumed);
        assert_eq!(w.current_sample_size(), 96);
        assert_eq!(w.handle_event(&zoom_out), EventResponse::Consumed);
        assert_eq!(w.current_sample_size(), 128);
    }

    #[test]
    fn zoom_level_clamps_at_limits() {
        let mut w = MinimapWidget::new(200, 30, 14);
        w.toggle();

        let zoom_in = click_in_bounds(*w.zoom_in_button.bounds());
        let zoom_out = click_in_bounds(*w.zoom_out_button.bounds());

        for _ in 0..10 {
            assert_eq!(w.handle_event(&zoom_in), EventResponse::Consumed);
        }
        assert_eq!(w.current_sample_size(), 64);

        for _ in 0..10 {
            assert_eq!(w.handle_event(&zoom_out), EventResponse::Consumed);
        }
        assert_eq!(w.current_sample_size(), 192);
    }

    #[test]
    fn zoom_level_persists_across_toggle() {
        let mut w = MinimapWidget::new(200, 30, 14);
        w.toggle();
        let zoom_in = click_in_bounds(*w.zoom_in_button.bounds());

        assert_eq!(w.handle_event(&zoom_in), EventResponse::Consumed);
        assert_eq!(w.current_sample_size(), 96);

        w.toggle();
        w.toggle();

        assert_eq!(w.current_sample_size(), 96);
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

    #[test]
    fn zoomed_in_viewport_duplicates_source_pixels() {
        let mut w = MinimapWidget::new(100, 30, 14);
        w.toggle();
        let zoom_in = click_in_bounds(*w.zoom_in_button.bounds());
        let zoom_in_again = click_in_bounds(*w.zoom_in_button.bounds());
        assert_eq!(w.handle_event(&zoom_in), EventResponse::Consumed);
        assert_eq!(w.handle_event(&zoom_in_again), EventResponse::Consumed);
        assert_eq!(w.current_sample_size(), 64);

        let mut xmap = vec![0u8; WORLD_SIZE * WORLD_SIZE * 4];
        fill_gradient_region(&mut xmap, 96, 160, 160, 224);

        w.update_viewport(&xmap, 128, 192);

        assert_eq!(w.viewport_pixels[0], 160);
        assert_eq!(w.viewport_pixels[1], 96);
        assert_eq!(w.viewport_pixels[4], 160);
        assert_eq!(w.viewport_pixels[5], 96);
    }

    #[test]
    fn zoomed_out_viewport_clamps_to_world_edges() {
        let mut w = MinimapWidget::new(100, 30, 14);
        w.toggle();
        let zoom_out = click_in_bounds(*w.zoom_out_button.bounds());
        let zoom_out_again = click_in_bounds(*w.zoom_out_button.bounds());
        assert_eq!(w.handle_event(&zoom_out), EventResponse::Consumed);
        assert_eq!(w.handle_event(&zoom_out_again), EventResponse::Consumed);
        assert_eq!(w.current_sample_size(), 192);

        let mut xmap = vec![0u8; WORLD_SIZE * WORLD_SIZE * 4];
        fill_gradient_region(&mut xmap, 0, 192, 0, 192);

        w.update_viewport(&xmap, 10, 20);

        assert_eq!(w.viewport_pixels[0], 0);
        assert_eq!(w.viewport_pixels[1], 0);

        let last =
            ((MINIMAP_WIDGET_VIEW_SIZE as usize * MINIMAP_WIDGET_VIEW_SIZE as usize) - 1) * 4;
        assert_eq!(w.viewport_pixels[last], 190);
        assert_eq!(w.viewport_pixels[last + 1], 190);
    }
}
