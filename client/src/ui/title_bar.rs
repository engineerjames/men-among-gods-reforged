//! Draggable title bar component for HUD panels.
//!
//! Provides drag-to-move, pin-to-lock, and close (X) functionality.
//! This is a sub-component — it does not implement [`Widget`] directly but
//! is embedded inside panel widgets that delegate events and rendering to it.

use sdl2::pixels::Color;
use sdl2::render::BlendMode;

use super::widget::{Bounds, EventResponse, MouseButton, UiEvent};
use super::RenderContext;
use crate::font_cache;

/// Height of the title bar in pixels.
pub const TITLE_BAR_H: i32 = 18;

/// Size of the pin / close icon squares in pixels.
const ICON_SIZE: i32 = 12;

/// Horizontal padding inside the bar for the first icon.
const ICON_PAD_X: i32 = 3;

/// Vertical centering offset for icons within the bar.
const ICON_PAD_Y: i32 = (TITLE_BAR_H - ICON_SIZE) / 2;

/// Outline color for pin / close icon boxes.
const ICON_OUTLINE: Color = Color::RGBA(180, 180, 200, 220);

/// Fill color for the pin indicator when pinned.
const PIN_ACTIVE_COLOR: Color = Color::RGBA(200, 220, 255, 240);

/// Fill color for the close X lines.
const CLOSE_X_COLOR: Color = Color::RGBA(200, 220, 255, 240);

/// Title bar background color (slightly darker than panel background).
const BAR_BG: Color = Color::RGBA(6, 6, 20, 200);

/// Border at the bottom of the title bar.
const BAR_BORDER: Color = Color::RGBA(120, 120, 140, 200);

/// Hover highlight alpha.
const HOVER_ALPHA: u8 = 64;

/// Bitmap font index (yellow, sprite 701).
const UI_FONT: usize = 1;

// ---------------------------------------------------------------------------
// Viewport clamping
// ---------------------------------------------------------------------------

/// Clamps a panel position so the panel stays entirely within the logical
/// viewport (960×540).
///
/// # Arguments
///
/// * `x` - Desired left edge.
/// * `y` - Desired top edge.
/// * `width` - Panel width.
/// * `height` - Panel height.
///
/// # Returns
///
/// Clamped `(x, y)`.
pub fn clamp_to_viewport(x: i32, y: i32, width: u32, height: u32) -> (i32, i32) {
    let max_x = crate::constants::TARGET_WIDTH_INT as i32 - width as i32;
    let max_y = crate::constants::TARGET_HEIGHT_INT as i32 - height as i32;
    (x.clamp(0, max_x.max(0)), y.clamp(0, max_y.max(0)))
}

// ---------------------------------------------------------------------------
// TitleBar struct
// ---------------------------------------------------------------------------

/// A draggable title bar with pin and close buttons.
///
/// Designed to be embedded as a child component inside panel widgets.
/// The parent panel delegates event handling and rendering to the title bar,
/// and responds to drag position updates and close requests.
///
/// When `movable` is `false` the pin icon is hidden and drag is disabled;
/// only the close (X) button remains functional.
pub struct TitleBar {
    /// Bounding rectangle of the full title bar.
    bounds: Bounds,
    /// Label text displayed centered in the bar.
    title: String,
    /// Whether drag and pin functionality is enabled.
    movable: bool,
    /// Whether the panel is pinned (drag-locked).
    pinned: bool,
    /// Whether a drag operation is in progress.
    dragging: bool,
    /// Mouse offset from the parent panel's top-left corner at drag start.
    drag_offset: (i32, i32),
    /// One-shot flag: the user clicked the close button.
    close_requested: bool,
    /// Hover state for the pin button.
    hovered_pin: bool,
    /// Hover state for the close button.
    hovered_close: bool,
}

impl TitleBar {
    /// Creates a new title bar.
    ///
    /// # Arguments
    ///
    /// * `title` - Label text (e.g. "Inventory").
    /// * `x` - Left edge of the parent panel.
    /// * `y` - Top edge of the parent panel.
    /// * `bar_width` - Width of the bar (should match the parent panel width).
    ///
    /// # Returns
    ///
    /// A new `TitleBar`.
    pub fn new(title: &str, x: i32, y: i32, bar_width: u32) -> Self {
        Self {
            bounds: Bounds::new(x, y, bar_width, TITLE_BAR_H as u32),
            title: title.to_owned(),
            movable: true,
            pinned: false,
            dragging: false,
            drag_offset: (0, 0),
            close_requested: false,
            hovered_pin: false,
            hovered_close: false,
        }
    }

    /// Creates a non-movable title bar (close button only, no pin or drag).
    ///
    /// # Arguments
    ///
    /// * `title` - Label text (e.g. "Quit?").
    /// * `x` - Left edge of the parent panel.
    /// * `y` - Top edge of the parent panel.
    /// * `bar_width` - Width of the bar (should match the parent panel width).
    ///
    /// # Returns
    ///
    /// A new `TitleBar` with drag and pin disabled.
    pub fn new_static(title: &str, x: i32, y: i32, bar_width: u32) -> Self {
        Self {
            bounds: Bounds::new(x, y, bar_width, TITLE_BAR_H as u32),
            title: title.to_owned(),
            movable: false,
            pinned: true, // effectively locked in place
            dragging: false,
            drag_offset: (0, 0),
            close_requested: false,
            hovered_pin: false,
            hovered_close: false,
        }
    }

    /// Repositions the title bar to match a new parent panel position.
    ///
    /// # Arguments
    ///
    /// * `x` - New left edge.
    /// * `y` - New top edge.
    pub fn set_bar_position(&mut self, x: i32, y: i32) {
        self.bounds.x = x;
        self.bounds.y = y;
    }

    /// Returns whether the panel is pinned (drag-locked).
    pub fn is_pinned(&self) -> bool {
        self.pinned
    }

    /// Returns whether a drag is currently in progress.
    pub fn is_dragging(&self) -> bool {
        self.dragging
    }

    /// Returns `true` once if the close button was clicked since the last
    /// call, then clears the flag.
    pub fn was_close_requested(&mut self) -> bool {
        let v = self.close_requested;
        self.close_requested = false;
        v
    }

    // -----------------------------------------------------------------------
    // Layout helpers
    // -----------------------------------------------------------------------

    /// Pixel rect for the pin icon button.
    fn pin_rect(&self) -> Bounds {
        Bounds::new(
            self.bounds.x + ICON_PAD_X,
            self.bounds.y + ICON_PAD_Y,
            ICON_SIZE as u32,
            ICON_SIZE as u32,
        )
    }

    /// Pixel rect for the close icon button.
    fn close_rect(&self) -> Bounds {
        Bounds::new(
            self.bounds.x + self.bounds.width as i32 - ICON_PAD_X - ICON_SIZE,
            self.bounds.y + ICON_PAD_Y,
            ICON_SIZE as u32,
            ICON_SIZE as u32,
        )
    }

    // -----------------------------------------------------------------------
    // Event handling
    // -----------------------------------------------------------------------

    /// Process an input event.
    ///
    /// Returns `(response, drag_position)` where `drag_position` is
    /// `Some((new_x, new_y))` when the parent panel should move (already
    /// un-offset but **not** clamped — the parent must clamp).
    ///
    /// # Arguments
    ///
    /// * `event` - The UI event to process.
    ///
    /// # Returns
    ///
    /// A tuple of `EventResponse` and an optional new panel position.
    pub fn handle_event(&mut self, event: &UiEvent) -> (EventResponse, Option<(i32, i32)>) {
        match event {
            // -- Hover tracking for pin / close icons ----------------------
            UiEvent::MouseMove { x, y } => {
                self.hovered_pin = self.pin_rect().contains_point(*x, *y);
                self.hovered_close = self.close_rect().contains_point(*x, *y);

                if self.dragging {
                    let new_x = *x - self.drag_offset.0;
                    let new_y = *y - self.drag_offset.1;
                    return (EventResponse::Consumed, Some((new_x, new_y)));
                }
                // Don't consume — let other widgets also track mouse.
                (EventResponse::Ignored, None)
            }

            // -- Begin drag on mouse-down in the bar area ------------------
            UiEvent::MouseDown {
                x,
                y,
                button: MouseButton::Left,
                ..
            } => {
                if !self.bounds.contains_point(*x, *y) {
                    return (EventResponse::Ignored, None);
                }
                // Don't start drag when clicking on pin or close icons.
                if (self.movable && self.pin_rect().contains_point(*x, *y))
                    || self.close_rect().contains_point(*x, *y)
                {
                    return (EventResponse::Consumed, None);
                }
                if self.movable && !self.pinned {
                    self.dragging = true;
                    self.drag_offset = (*x - self.bounds.x, *y - self.bounds.y);
                }
                (EventResponse::Consumed, None)
            }

            // -- End drag / pin toggle / close on mouse-up -----------------
            UiEvent::MouseClick {
                x,
                y,
                button: MouseButton::Left,
                ..
            } => {
                if self.dragging {
                    self.dragging = false;
                    return (EventResponse::Consumed, None);
                }
                if self.movable && self.pin_rect().contains_point(*x, *y) {
                    self.pinned = !self.pinned;
                    return (EventResponse::Consumed, None);
                }
                if self.close_rect().contains_point(*x, *y) {
                    self.close_requested = true;
                    return (EventResponse::Consumed, None);
                }
                if self.bounds.contains_point(*x, *y) {
                    // Click on bar area (e.g. pinned panel) — consume but no action.
                    return (EventResponse::Consumed, None);
                }
                (EventResponse::Ignored, None)
            }

            _ => (EventResponse::Ignored, None),
        }
    }

    // -----------------------------------------------------------------------
    // Rendering
    // -----------------------------------------------------------------------

    /// Draws the title bar.
    ///
    /// # Arguments
    ///
    /// * `ctx` - Mutable render context (canvas + graphics cache).
    ///
    /// # Returns
    ///
    /// `Ok(())` on success, or an SDL2 error string.
    pub fn render(&self, ctx: &mut RenderContext<'_, '_>) -> Result<(), String> {
        let bar_rect = sdl2::rect::Rect::new(
            self.bounds.x,
            self.bounds.y,
            self.bounds.width,
            TITLE_BAR_H as u32,
        );

        // Background.
        ctx.canvas.set_blend_mode(BlendMode::Blend);
        ctx.canvas.set_draw_color(BAR_BG);
        ctx.canvas.fill_rect(bar_rect)?;

        // Bottom border line.
        ctx.canvas.set_draw_color(BAR_BORDER);
        ctx.canvas.draw_line(
            sdl2::rect::Point::new(self.bounds.x, self.bounds.y + TITLE_BAR_H - 1),
            sdl2::rect::Point::new(
                self.bounds.x + self.bounds.width as i32 - 1,
                self.bounds.y + TITLE_BAR_H - 1,
            ),
        )?;

        // --- Pin icon (left) — only for movable title bars ---
        if self.movable {
            let pr = self.pin_rect();
            let pin_sdl = sdl2::rect::Rect::new(pr.x, pr.y, pr.width, pr.height);
            ctx.canvas.set_draw_color(ICON_OUTLINE);
            ctx.canvas.draw_rect(pin_sdl)?;

            if self.pinned {
                // Filled inner square when pinned.
                let inset = 3_i32;
                let inner = sdl2::rect::Rect::new(
                    pr.x + inset,
                    pr.y + inset,
                    (ICON_SIZE - 2 * inset) as u32,
                    (ICON_SIZE - 2 * inset) as u32,
                );
                ctx.canvas.set_draw_color(PIN_ACTIVE_COLOR);
                ctx.canvas.fill_rect(inner)?;
            } else {
                // Small vertical line (pin "needle") when unpinned.
                let cx = pr.x + ICON_SIZE / 2;
                ctx.canvas.set_draw_color(ICON_OUTLINE);
                ctx.canvas.draw_line(
                    sdl2::rect::Point::new(cx, pr.y + 3),
                    sdl2::rect::Point::new(cx, pr.y + ICON_SIZE - 4),
                )?;
            }

            // Pin hover highlight.
            if self.hovered_pin {
                ctx.canvas.set_blend_mode(BlendMode::Add);
                ctx.canvas
                    .set_draw_color(Color::RGBA(255, 255, 255, HOVER_ALPHA));
                ctx.canvas.fill_rect(pin_sdl)?;
                ctx.canvas.set_blend_mode(BlendMode::Blend);
            }
        }

        // --- Close icon (right) ---
        let cr = self.close_rect();
        let close_sdl = sdl2::rect::Rect::new(cr.x, cr.y, cr.width, cr.height);
        ctx.canvas.set_draw_color(ICON_OUTLINE);
        ctx.canvas.draw_rect(close_sdl)?;

        // X diagonal lines inside the close box.
        let inset = 3_i32;
        let x0 = cr.x + inset;
        let y0 = cr.y + inset;
        let x1 = cr.x + ICON_SIZE - inset - 1;
        let y1 = cr.y + ICON_SIZE - inset - 1;
        ctx.canvas.set_draw_color(CLOSE_X_COLOR);
        ctx.canvas.draw_line(
            sdl2::rect::Point::new(x0, y0),
            sdl2::rect::Point::new(x1, y1),
        )?;
        ctx.canvas.draw_line(
            sdl2::rect::Point::new(x1, y0),
            sdl2::rect::Point::new(x0, y1),
        )?;

        // Close hover highlight.
        if self.hovered_close {
            ctx.canvas.set_blend_mode(BlendMode::Add);
            ctx.canvas
                .set_draw_color(Color::RGBA(255, 255, 255, HOVER_ALPHA));
            ctx.canvas.fill_rect(close_sdl)?;
            ctx.canvas.set_blend_mode(BlendMode::Blend);
        }

        // --- Title text (centered in the area between pin and close) ---
        let text_area_left = if self.movable {
            let pr = self.pin_rect();
            pr.x + ICON_SIZE + 4
        } else {
            self.bounds.x + ICON_PAD_X
        };
        let text_area_right = cr.x - 4;
        let text_cx = (text_area_left + text_area_right) / 2;
        let text_cy = self.bounds.y + (TITLE_BAR_H - font_cache::BITMAP_GLYPH_H as i32) / 2;
        font_cache::draw_text(
            ctx.canvas,
            ctx.gfx,
            UI_FONT,
            &self.title,
            text_cx,
            text_cy,
            font_cache::TextStyle::centered(),
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
    use crate::ui::widget::KeyModifiers;

    #[test]
    fn new_title_bar_defaults() {
        let mut tb = TitleBar::new("Test", 10, 20, 200);
        assert!(!tb.is_pinned());
        assert!(!tb.is_dragging());
        assert!(!tb.was_close_requested());
        assert_eq!(tb.bounds.x, 10);
        assert_eq!(tb.bounds.y, 20);
        assert_eq!(tb.bounds.width, 200);
        assert_eq!(tb.bounds.height, TITLE_BAR_H as u32);
    }

    #[test]
    fn set_bar_position_updates_bounds() {
        let mut tb = TitleBar::new("Test", 0, 0, 100);
        tb.set_bar_position(50, 60);
        assert_eq!(tb.bounds.x, 50);
        assert_eq!(tb.bounds.y, 60);
    }

    #[test]
    fn pin_toggle_via_click() {
        let mut tb = TitleBar::new("Test", 0, 0, 200);
        assert!(!tb.is_pinned());

        // Click on the pin icon area.
        let pin = tb.pin_rect();
        let cx = pin.x + pin.width as i32 / 2;
        let cy = pin.y + pin.height as i32 / 2;

        // Mouse-down on pin (consumed but no action yet).
        let (resp, pos) = tb.handle_event(&UiEvent::MouseDown {
            x: cx,
            y: cy,
            button: MouseButton::Left,
            modifiers: KeyModifiers::default(),
        });
        assert_eq!(resp, EventResponse::Consumed);
        assert!(pos.is_none());
        assert!(!tb.is_dragging()); // pin click should NOT start drag

        // Mouse-up (click) on pin toggles pinned.
        let (resp, _) = tb.handle_event(&UiEvent::MouseClick {
            x: cx,
            y: cy,
            button: MouseButton::Left,
            modifiers: KeyModifiers::default(),
        });
        assert_eq!(resp, EventResponse::Consumed);
        assert!(tb.is_pinned());

        // Click again to unpin.
        let _ = tb.handle_event(&UiEvent::MouseDown {
            x: cx,
            y: cy,
            button: MouseButton::Left,
            modifiers: KeyModifiers::default(),
        });
        let (_, _) = tb.handle_event(&UiEvent::MouseClick {
            x: cx,
            y: cy,
            button: MouseButton::Left,
            modifiers: KeyModifiers::default(),
        });
        assert!(!tb.is_pinned());
    }

    #[test]
    fn close_requested_one_shot() {
        let mut tb = TitleBar::new("Test", 0, 0, 200);
        let cr = tb.close_rect();
        let cx = cr.x + cr.width as i32 / 2;
        let cy = cr.y + cr.height as i32 / 2;

        // Mouse-down then click on close.
        tb.handle_event(&UiEvent::MouseDown {
            x: cx,
            y: cy,
            button: MouseButton::Left,
            modifiers: KeyModifiers::default(),
        });
        tb.handle_event(&UiEvent::MouseClick {
            x: cx,
            y: cy,
            button: MouseButton::Left,
            modifiers: KeyModifiers::default(),
        });

        assert!(tb.was_close_requested());
        // Second read should return false.
        assert!(!tb.was_close_requested());
    }

    #[test]
    fn drag_start_and_move() {
        let mut tb = TitleBar::new("Test", 0, 0, 200);
        // Click in the middle of the bar (between pin and close).
        let mid_x = 100;
        let mid_y = TITLE_BAR_H / 2;

        // Mouse-down starts drag.
        let (resp, pos) = tb.handle_event(&UiEvent::MouseDown {
            x: mid_x,
            y: mid_y,
            button: MouseButton::Left,
            modifiers: KeyModifiers::default(),
        });
        assert_eq!(resp, EventResponse::Consumed);
        assert!(pos.is_none());
        assert!(tb.is_dragging());

        // Mouse-move while dragging returns new position.
        let (resp, pos) = tb.handle_event(&UiEvent::MouseMove {
            x: mid_x + 30,
            y: mid_y + 20,
        });
        assert_eq!(resp, EventResponse::Consumed);
        assert!(pos.is_some());
        let (nx, ny) = pos.unwrap();
        assert_eq!(nx, 30); // (100+30) - offset(100) = 30
        assert_eq!(ny, 20); // ( 9+20) - offset(9)   = 20

        // Mouse-up ends drag.
        let (resp, _) = tb.handle_event(&UiEvent::MouseClick {
            x: mid_x + 30,
            y: mid_y + 20,
            button: MouseButton::Left,
            modifiers: KeyModifiers::default(),
        });
        assert_eq!(resp, EventResponse::Consumed);
        assert!(!tb.is_dragging());
    }

    #[test]
    fn pinned_prevents_drag() {
        let mut tb = TitleBar::new("Test", 0, 0, 200);
        // Pin it first.
        let pin = tb.pin_rect();
        let pcx = pin.x + pin.width as i32 / 2;
        let pcy = pin.y + pin.height as i32 / 2;
        tb.handle_event(&UiEvent::MouseDown {
            x: pcx,
            y: pcy,
            button: MouseButton::Left,
            modifiers: KeyModifiers::default(),
        });
        tb.handle_event(&UiEvent::MouseClick {
            x: pcx,
            y: pcy,
            button: MouseButton::Left,
            modifiers: KeyModifiers::default(),
        });
        assert!(tb.is_pinned());

        // Try to start a drag in the middle.
        let mid_x = 100;
        let mid_y = TITLE_BAR_H / 2;
        tb.handle_event(&UiEvent::MouseDown {
            x: mid_x,
            y: mid_y,
            button: MouseButton::Left,
            modifiers: KeyModifiers::default(),
        });
        assert!(!tb.is_dragging());
    }

    #[test]
    fn events_outside_bar_are_ignored() {
        let mut tb = TitleBar::new("Test", 10, 10, 100);
        let (resp, pos) = tb.handle_event(&UiEvent::MouseDown {
            x: 0,
            y: 0,
            button: MouseButton::Left,
            modifiers: KeyModifiers::default(),
        });
        assert_eq!(resp, EventResponse::Ignored);
        assert!(pos.is_none());
    }

    #[test]
    fn clamp_to_viewport_basic() {
        // Panel fits, no change.
        assert_eq!(clamp_to_viewport(100, 100, 200, 100), (100, 100));
        // Panel too far right.
        assert_eq!(clamp_to_viewport(900, 100, 200, 100), (760, 100));
        // Panel too far down.
        assert_eq!(clamp_to_viewport(100, 500, 200, 100), (100, 440));
        // Negative values clamped to 0.
        assert_eq!(clamp_to_viewport(-50, -10, 200, 100), (0, 0));
    }

    #[test]
    fn clamp_to_viewport_oversized() {
        // Panel larger than viewport — clamps to (0, 0).
        assert_eq!(clamp_to_viewport(100, 100, 1000, 600), (0, 0));
    }

    #[test]
    fn static_bar_ignores_drag_and_pin() {
        let mut tb = TitleBar::new_static("Static", 0, 0, 200);
        assert!(!tb.is_dragging());
        // Pinned is true internally but pin toggle should be ignored.
        assert!(tb.is_pinned());

        // Try to drag in the middle — should not start drag.
        let mid_x = 100;
        let mid_y = TITLE_BAR_H / 2;
        let (resp, _) = tb.handle_event(&UiEvent::MouseDown {
            x: mid_x,
            y: mid_y,
            button: MouseButton::Left,
            modifiers: KeyModifiers::default(),
        });
        assert_eq!(resp, EventResponse::Consumed);
        assert!(!tb.is_dragging());

        // Click the close button — should still work.
        let cr = tb.close_rect();
        let cx = cr.x + cr.width as i32 / 2;
        let cy = cr.y + cr.height as i32 / 2;
        tb.handle_event(&UiEvent::MouseDown {
            x: cx,
            y: cy,
            button: MouseButton::Left,
            modifiers: KeyModifiers::default(),
        });
        tb.handle_event(&UiEvent::MouseClick {
            x: cx,
            y: cy,
            button: MouseButton::Left,
            modifiers: KeyModifiers::default(),
        });
        assert!(tb.was_close_requested());
    }
}
