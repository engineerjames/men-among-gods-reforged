//! Horizontal slider widget for numeric values.

use sdl2::pixels::Color;
use sdl2::render::BlendMode;

use crate::ui::RenderContext;
use crate::ui::widget::{Bounds, EventResponse, MouseButton, UiEvent, Widget};
use crate::font_cache;

/// Height of the slider track bar in pixels.
const TRACK_H: u32 = 4;
/// Width of the slider thumb handle in pixels.
const THUMB_W: u32 = 8;
/// Height of the slider thumb handle in pixels.
const THUMB_H: u32 = 14;
/// Horizontal offset from the left edge where the track begins (label area).
const TRACK_LEFT_INSET: i32 = 0;

/// A horizontal slider that maps a mouse click position to a floating-point
/// value between `min` and `max`.
///
/// The widget renders a label above the track, a horizontal bar, a
/// draggable thumb, and a percentage readout on the right.
pub struct Slider {
    bounds: Bounds,
    label: String,
    font: usize,
    value: f32,
    min: f32,
    max: f32,
    hovered: bool,
    /// Whether the left mouse button is currently held down over the track.
    dragging: bool,
    /// One-shot flag indicating the value changed since last read.
    changed: bool,
    /// Additive tint alpha applied on hover (0–255).
    hover_alpha: u8,
}

impl Slider {
    /// Creates a new horizontal slider.
    ///
    /// # Arguments
    ///
    /// * `bounds` - Position and size of the entire widget area.
    /// * `label` - Text drawn to the left of the track.
    /// * `min` - Minimum value (left end of track).
    /// * `max` - Maximum value (right end of track).
    /// * `initial` - Starting value, clamped to `[min, max]`.
    /// * `font` - Bitmap font index (0–3).
    ///
    /// # Returns
    ///
    /// A new `Slider`.
    pub fn new(bounds: Bounds, label: &str, min: f32, max: f32, initial: f32, font: usize) -> Self {
        Self {
            bounds,
            label: label.to_owned(),
            font,
            value: initial.clamp(min, max),
            min,
            max,
            hovered: false,
            dragging: false,
            changed: false,
            hover_alpha: 48,
        }
    }

    /// Returns the current slider value.
    pub fn value(&self) -> f32 {
        self.value
    }

    /// Sets the slider value without triggering the changed flag.
    ///
    /// Use this when syncing from external state (e.g. loading a profile).
    ///
    /// # Arguments
    ///
    /// * `value` - New value, clamped to `[min, max]`.
    pub fn set_value(&mut self, value: f32) {
        self.value = value.clamp(self.min, self.max);
    }

    /// Returns `true` once if the value changed since the last call.
    ///
    /// Clears the flag on read.
    pub fn was_changed(&mut self) -> bool {
        let c = self.changed;
        self.changed = false;
        c
    }

    /// Returns the track region (the clickable bar area), expressed as
    /// `(x_start, x_end, y_center)` in absolute coordinates.
    fn track_geometry(&self) -> (i32, i32, i32) {
        // Label area is the left portion; track starts after it.
        let label_width = font_cache::text_width(&self.label) as i32 + 6;
        let value_text_w = font_cache::text_width("100%") as i32 + 4;
        let track_x_start = self.bounds.x + TRACK_LEFT_INSET + label_width;
        let track_x_end = self.bounds.x + self.bounds.width as i32 - value_text_w;
        let track_y_center = self.bounds.y + self.bounds.height as i32 / 2;
        (track_x_start, track_x_end, track_y_center)
    }

    /// Converts an absolute X coordinate to a slider value.
    fn x_to_value(&self, x: i32) -> f32 {
        let (x_start, x_end, _) = self.track_geometry();
        let track_w = (x_end - x_start).max(1) as f32;
        let t = ((x - x_start) as f32 / track_w).clamp(0.0, 1.0);
        self.min + t * (self.max - self.min)
    }
}

impl Widget for Slider {
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
                if self.dragging {
                    let new_val = self.x_to_value(*x);
                    if (new_val - self.value).abs() > f32::EPSILON {
                        self.value = new_val;
                        self.changed = true;
                    }
                    return EventResponse::Consumed;
                }
                EventResponse::Ignored
            }
            UiEvent::MouseDown {
                x,
                y,
                button: MouseButton::Left,
                ..
            } => {
                if self.bounds.contains_point(*x, *y) {
                    self.dragging = true;
                    let new_val = self.x_to_value(*x);
                    if (new_val - self.value).abs() > f32::EPSILON {
                        self.value = new_val;
                        self.changed = true;
                    }
                    EventResponse::Consumed
                } else {
                    EventResponse::Ignored
                }
            }
            UiEvent::MouseClick {
                x,
                y,
                button: MouseButton::Left,
                ..
            } => {
                self.dragging = false;
                if self.bounds.contains_point(*x, *y) {
                    let new_val = self.x_to_value(*x);
                    if (new_val - self.value).abs() > f32::EPSILON {
                        self.value = new_val;
                        self.changed = true;
                    }
                    EventResponse::Consumed
                } else {
                    EventResponse::Ignored
                }
            }
            _ => EventResponse::Ignored,
        }
    }

    fn render(&mut self, ctx: &mut RenderContext<'_, '_>) -> Result<(), String> {
        let (track_x_start, track_x_end, track_y_center) = self.track_geometry();
        let track_w = (track_x_end - track_x_start).max(1) as u32;

        // Label text (left side)
        let label_y =
            self.bounds.y + (self.bounds.height as i32 - font_cache::BITMAP_GLYPH_H as i32) / 2;
        font_cache::draw_text(
            ctx.canvas,
            ctx.gfx,
            self.font,
            &self.label,
            self.bounds.x,
            label_y,
            font_cache::TextStyle::PLAIN,
        )?;

        // Track bar (centered vertically)
        let track_rect = sdl2::rect::Rect::new(
            track_x_start,
            track_y_center - (TRACK_H as i32 / 2),
            track_w,
            TRACK_H,
        );
        ctx.canvas.set_blend_mode(BlendMode::Blend);
        ctx.canvas.set_draw_color(Color::RGBA(100, 100, 120, 200));
        ctx.canvas.fill_rect(track_rect)?;

        // Thumb handle
        let t = if (self.max - self.min).abs() > f32::EPSILON {
            (self.value - self.min) / (self.max - self.min)
        } else {
            0.0
        };
        let thumb_x = track_x_start + (t * (track_w.saturating_sub(THUMB_W)) as f32).round() as i32;
        let thumb_y = track_y_center - (THUMB_H as i32 / 2);
        let thumb_rect = sdl2::rect::Rect::new(thumb_x, thumb_y, THUMB_W, THUMB_H);

        ctx.canvas.set_draw_color(Color::RGBA(200, 210, 230, 240));
        ctx.canvas.fill_rect(thumb_rect)?;
        ctx.canvas.set_draw_color(Color::RGBA(140, 140, 160, 240));
        ctx.canvas.draw_rect(thumb_rect)?;

        // Value text (right side, as percentage)
        let pct = ((self.value - self.min) / (self.max - self.min) * 100.0).round() as i32;
        let value_text = format!("{}%", pct);
        let value_x = track_x_end + 4;
        let value_y = label_y;
        font_cache::draw_text(
            ctx.canvas,
            ctx.gfx,
            self.font,
            &value_text,
            value_x,
            value_y,
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
    fn initial_value_clamped() {
        let s = Slider::new(Bounds::new(0, 0, 200, 16), "Vol", 0.0, 1.0, 1.5, 0);
        assert!((s.value() - 1.0).abs() < f32::EPSILON);

        let s2 = Slider::new(Bounds::new(0, 0, 200, 16), "Vol", 0.0, 1.0, -0.5, 0);
        assert!((s2.value() - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn set_value_clamps() {
        let mut s = Slider::new(Bounds::new(0, 0, 200, 16), "Vol", 0.0, 1.0, 0.5, 0);
        s.set_value(2.0);
        assert!((s.value() - 1.0).abs() < f32::EPSILON);
        s.set_value(-1.0);
        assert!((s.value() - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn set_value_does_not_trigger_changed() {
        let mut s = Slider::new(Bounds::new(0, 0, 200, 16), "Vol", 0.0, 1.0, 0.5, 0);
        s.set_value(0.8);
        assert!(!s.was_changed());
    }

    #[test]
    fn click_outside_ignored() {
        let mut s = Slider::new(Bounds::new(10, 10, 200, 16), "Vol", 0.0, 1.0, 0.5, 0);
        let resp = s.handle_event(&UiEvent::MouseClick {
            x: 0,
            y: 0,
            button: MouseButton::Left,
            modifiers: KeyModifiers::default(),
        });
        assert_eq!(resp, EventResponse::Ignored);
        assert!(!s.was_changed());
    }

    #[test]
    fn click_inside_changes_value() {
        let mut s = Slider::new(Bounds::new(0, 0, 200, 16), "Vol", 0.0, 1.0, 0.0, 0);
        let resp = s.handle_event(&UiEvent::MouseClick {
            x: 100,
            y: 8,
            button: MouseButton::Left,
            modifiers: KeyModifiers::default(),
        });
        assert_eq!(resp, EventResponse::Consumed);
        assert!(s.was_changed());
        // Value should be somewhere between 0 and 1 based on click position.
        assert!(s.value() > 0.0);
        assert!(s.value() <= 1.0);
    }

    #[test]
    fn was_changed_clears_on_read() {
        let mut s = Slider::new(Bounds::new(0, 0, 200, 16), "Vol", 0.0, 1.0, 0.0, 0);
        s.handle_event(&UiEvent::MouseClick {
            x: 100,
            y: 8,
            button: MouseButton::Left,
            modifiers: KeyModifiers::default(),
        });
        assert!(s.was_changed());
        assert!(!s.was_changed());
    }
}
