//! Toggleable checkbox widget with a text label.

use sdl2::pixels::Color;
use sdl2::render::BlendMode;

use super::widget::{Bounds, EventResponse, MouseButton, UiEvent, Widget};
use super::RenderContext;
use crate::font_cache;

/// Size of the checkbox square in pixels.
const BOX_SIZE: u32 = 10;
/// Horizontal gap between the checkbox square and its label text.
const LABEL_GAP: i32 = 4;

/// A toggleable checkbox with a text label.
///
/// Renders a small outlined square on the left with a filled inner square
/// when checked, followed by the label text. Left-clicking anywhere inside
/// the widget bounds toggles the checked state.
pub struct Checkbox {
    bounds: Bounds,
    label: String,
    font: usize,
    checked: bool,
    hovered: bool,
    /// One-shot flag indicating the value changed since last read.
    toggled: bool,
    /// Additive tint alpha applied on hover (0–255).
    hover_alpha: u8,
}

impl Checkbox {
    /// Creates a new checkbox.
    ///
    /// # Arguments
    ///
    /// * `bounds` - Position and size of the entire widget (box + label area).
    /// * `label` - Text drawn to the right of the checkbox square.
    /// * `font` - Bitmap font index (0–3).
    ///
    /// # Returns
    ///
    /// A new `Checkbox`, initially unchecked.
    pub fn new(bounds: Bounds, label: &str, font: usize) -> Self {
        Self {
            bounds,
            label: label.to_owned(),
            font,
            checked: false,
            hovered: false,
            toggled: false,
            hover_alpha: 64,
        }
    }

    /// Returns whether the checkbox is currently checked.
    pub fn is_checked(&self) -> bool {
        self.checked
    }

    /// Sets the checked state without triggering the toggled flag.
    ///
    /// Use this when syncing from external state (e.g. loading a profile).
    ///
    /// # Arguments
    ///
    /// * `checked` - New checked state.
    pub fn set_checked(&mut self, checked: bool) {
        self.checked = checked;
    }

    /// Returns `true` once if the checkbox was toggled since the last call.
    ///
    /// Clears the flag on read.
    pub fn was_toggled(&mut self) -> bool {
        let t = self.toggled;
        self.toggled = false;
        t
    }
}

impl Widget for Checkbox {
    fn bounds(&self) -> &Bounds {
        &self.bounds
    }

    fn set_position(&mut self, x: i32, y: i32) {
        self.bounds.x = x;
        self.bounds.y = y;
    }

    fn handle_event(&mut self, event: &UiEvent) -> EventResponse {
        match event {
            UiEvent::MouseMove { x, y } => {
                self.hovered = self.bounds.contains_point(*x, *y);
                EventResponse::Ignored
            }
            UiEvent::MouseClick {
                x,
                y,
                button: MouseButton::Left,
                ..
            } => {
                if self.bounds.contains_point(*x, *y) {
                    self.checked = !self.checked;
                    self.toggled = true;
                    EventResponse::Consumed
                } else {
                    EventResponse::Ignored
                }
            }
            _ => EventResponse::Ignored,
        }
    }

    fn render(&mut self, ctx: &mut RenderContext<'_, '_>) -> Result<(), String> {
        let box_x = self.bounds.x;
        let box_y = self.bounds.y + (self.bounds.height as i32 - BOX_SIZE as i32) / 2;

        let box_rect = sdl2::rect::Rect::new(box_x, box_y, BOX_SIZE, BOX_SIZE);

        // Outline
        ctx.canvas.set_blend_mode(BlendMode::Blend);
        ctx.canvas.set_draw_color(Color::RGBA(180, 180, 200, 220));
        ctx.canvas.draw_rect(box_rect)?;

        // Checkmark (X drawn from corner to corner of the inset area)
        if self.checked {
            let inset = 2_i32;
            let x0 = box_x + inset;
            let y0 = box_y + inset;
            let x1 = box_x + BOX_SIZE as i32 - inset - 1;
            let y1 = box_y + BOX_SIZE as i32 - inset - 1;
            ctx.canvas.set_draw_color(Color::RGBA(200, 220, 255, 240));
            ctx.canvas.draw_line(
                sdl2::rect::Point::new(x0, y0),
                sdl2::rect::Point::new(x1, y1),
            )?;
            ctx.canvas.draw_line(
                sdl2::rect::Point::new(x1, y0),
                sdl2::rect::Point::new(x0, y1),
            )?;
        }

        // Label text
        let text_x = box_x + BOX_SIZE as i32 + LABEL_GAP;
        let text_y =
            self.bounds.y + (self.bounds.height as i32 - font_cache::BITMAP_GLYPH_H as i32) / 2;
        font_cache::draw_text(
            ctx.canvas,
            ctx.gfx,
            self.font,
            &self.label,
            text_x,
            text_y,
            font_cache::TextStyle::PLAIN,
        )?;

        // Hover highlight
        if self.hovered {
            let full_rect = sdl2::rect::Rect::new(
                self.bounds.x,
                self.bounds.y,
                self.bounds.width,
                self.bounds.height,
            );
            ctx.canvas.set_blend_mode(BlendMode::Add);
            ctx.canvas
                .set_draw_color(Color::RGBA(255, 255, 255, self.hover_alpha));
            ctx.canvas.fill_rect(full_rect)?;
            ctx.canvas.set_blend_mode(BlendMode::Blend);
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
    use crate::ui::widget::KeyModifiers;

    #[test]
    fn starts_unchecked() {
        let cb = Checkbox::new(Bounds::new(0, 0, 120, 14), "Test", 0);
        assert!(!cb.is_checked());
    }

    #[test]
    fn toggle_roundtrip() {
        let mut cb = Checkbox::new(Bounds::new(0, 0, 120, 14), "Test", 0);
        cb.set_checked(true);
        assert!(cb.is_checked());
        cb.set_checked(false);
        assert!(!cb.is_checked());
    }

    #[test]
    fn click_inside_toggles() {
        let mut cb = Checkbox::new(Bounds::new(10, 10, 120, 14), "Test", 0);
        let resp = cb.handle_event(&UiEvent::MouseClick {
            x: 50,
            y: 15,
            button: MouseButton::Left,
            modifiers: KeyModifiers::default(),
        });
        assert_eq!(resp, EventResponse::Consumed);
        assert!(cb.is_checked());
        assert!(cb.was_toggled());
        // Second read clears the flag.
        assert!(!cb.was_toggled());
    }

    #[test]
    fn click_outside_ignored() {
        let mut cb = Checkbox::new(Bounds::new(10, 10, 120, 14), "Test", 0);
        let resp = cb.handle_event(&UiEvent::MouseClick {
            x: 0,
            y: 0,
            button: MouseButton::Left,
            modifiers: KeyModifiers::default(),
        });
        assert_eq!(resp, EventResponse::Ignored);
        assert!(!cb.is_checked());
        assert!(!cb.was_toggled());
    }

    #[test]
    fn set_checked_does_not_trigger_toggled() {
        let mut cb = Checkbox::new(Bounds::new(0, 0, 120, 14), "Test", 0);
        cb.set_checked(true);
        assert!(!cb.was_toggled());
    }

    #[test]
    fn double_click_returns_to_unchecked() {
        let mut cb = Checkbox::new(Bounds::new(0, 0, 120, 14), "Test", 0);
        let click = UiEvent::MouseClick {
            x: 5,
            y: 5,
            button: MouseButton::Left,
            modifiers: KeyModifiers::default(),
        };
        cb.handle_event(&click);
        assert!(cb.is_checked());
        cb.was_toggled(); // clear flag
        cb.handle_event(&click);
        assert!(!cb.is_checked());
        assert!(cb.was_toggled());
    }
}
