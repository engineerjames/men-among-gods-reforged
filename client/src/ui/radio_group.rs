//! Generic radio-button group widget.
//!
//! Renders a vertical (default) or horizontal list of radio options.
//! Each option is a filled/unfilled circle followed by a bitmap-font label.
//! Clicking an option selects it and deselects the others.

use std::fmt::Debug;
use std::time::Duration;

use sdl2::pixels::Color;
use sdl2::render::BlendMode;

use super::widget::{Bounds, EventResponse, MouseButton, UiEvent, Widget};
use super::RenderContext;
use crate::font_cache;

/// Radius of the radio circle in pixels.
const RADIO_RADIUS: i32 = 5;

/// Gap between the circle and the label text.
const LABEL_GAP: i32 = 6;

/// Bitmap font index used for option labels.
const FONT: usize = 1;

/// Vertical spacing between radio options in vertical layout.
const OPTION_GAP_V: i32 = 6;

/// Horizontal spacing between radio options in horizontal layout.
const OPTION_GAP_H: i32 = 16;

// ---------------------------------------------------------------------------
// Widget
// ---------------------------------------------------------------------------

/// A single radio option stored internally.
struct RadioOption<T> {
    /// The value this option represents.
    value: T,
    /// Display label.
    label: String,
}

/// Layout direction for the radio group.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Layout {
    /// Options are stacked top-to-bottom.
    Vertical,
    /// Options are arranged left-to-right.
    Horizontal,
}

/// A group of mutually-exclusive radio options.
///
/// Generic over `T` which must be `Copy + PartialEq` so that selections can
/// be compared and stored cheaply.
pub struct RadioGroup<T: Copy + PartialEq + Debug> {
    bounds: Bounds,
    options: Vec<RadioOption<T>>,
    selected: T,
    layout: Layout,
    /// One-shot flag: set when the selection changes, cleared by
    /// [`was_changed`](Self::was_changed).
    changed: bool,
}

impl<T: Copy + PartialEq + Debug> RadioGroup<T> {
    /// Creates a new vertical radio group.
    ///
    /// # Arguments
    ///
    /// * `bounds` - Position and size of the widget area.
    /// * `options` - List of `(value, label)` pairs.
    /// * `selected` - Initially selected value; must match one of the options.
    ///
    /// # Returns
    ///
    /// A new `RadioGroup` in vertical layout.
    pub fn new(bounds: Bounds, options: &[(T, &str)], selected: T) -> Self {
        let opts = options
            .iter()
            .map(|(val, lbl)| RadioOption {
                value: *val,
                label: lbl.to_string(),
            })
            .collect();
        Self {
            bounds,
            options: opts,
            selected,
            layout: Layout::Vertical,
            changed: false,
        }
    }

    /// Creates a new horizontal radio group.
    ///
    /// # Arguments
    ///
    /// * `bounds` - Position and size of the widget area.
    /// * `options` - List of `(value, label)` pairs.
    /// * `selected` - Initially selected value.
    ///
    /// # Returns
    ///
    /// A new `RadioGroup` in horizontal layout.
    pub fn horizontal(bounds: Bounds, options: &[(T, &str)], selected: T) -> Self {
        let opts = options
            .iter()
            .map(|(val, lbl)| RadioOption {
                value: *val,
                label: lbl.to_string(),
            })
            .collect();
        Self {
            bounds,
            options: opts,
            selected,
            layout: Layout::Horizontal,
            changed: false,
        }
    }

    /// Returns the currently selected value.
    pub fn selected(&self) -> T {
        self.selected
    }

    /// Programmatically sets the selected value without triggering the
    /// `changed` flag.
    ///
    /// # Arguments
    ///
    /// * `value` - The value to select.
    pub fn set_selected(&mut self, value: T) {
        self.selected = value;
    }

    /// Returns `true` once if the selection changed since the last call,
    /// then clears the flag.
    pub fn was_changed(&mut self) -> bool {
        let c = self.changed;
        self.changed = false;
        c
    }

    /// Returns the row height for a single radio option.
    fn option_height(&self) -> i32 {
        (RADIO_RADIUS * 2).max(font_cache::BITMAP_GLYPH_H as i32)
    }

    /// Returns the bounding rect for the n-th option.
    fn option_bounds(&self, index: usize) -> Bounds {
        let row_h = self.option_height();
        match self.layout {
            Layout::Vertical => {
                let y = self.bounds.y + index as i32 * (row_h + OPTION_GAP_V);
                Bounds::new(self.bounds.x, y, self.bounds.width, row_h as u32)
            }
            Layout::Horizontal => {
                let label_w = self
                    .options
                    .get(index)
                    .map(|o| font_cache::text_width(&o.label))
                    .unwrap_or(0);
                let item_w =
                    RADIO_RADIUS as u32 * 2 + LABEL_GAP as u32 + label_w + OPTION_GAP_H as u32;
                let mut x = self.bounds.x;
                for i in 0..index {
                    let lw = font_cache::text_width(&self.options[i].label);
                    x += RADIO_RADIUS as i32 * 2
                        + LABEL_GAP
                        + lw as i32
                        + OPTION_GAP_H;
                }
                Bounds::new(x, self.bounds.y, item_w, row_h as u32)
            }
        }
    }
}

impl<T: Copy + PartialEq + Debug> Widget for RadioGroup<T> {
    fn bounds(&self) -> &Bounds {
        &self.bounds
    }

    fn set_position(&mut self, x: i32, y: i32) {
        self.bounds.x = x;
        self.bounds.y = y;
    }

    fn handle_event(&mut self, event: &UiEvent) -> EventResponse {
        if let UiEvent::MouseClick {
            x,
            y,
            button: MouseButton::Left,
            ..
        } = event
        {
            for (i, opt) in self.options.iter().enumerate() {
                let ob = self.option_bounds(i);
                if ob.contains_point(*x, *y) {
                    if self.selected != opt.value {
                        self.selected = opt.value;
                        self.changed = true;
                    }
                    return EventResponse::Consumed;
                }
            }
        }
        EventResponse::Ignored
    }

    fn update(&mut self, _dt: Duration) {
        // No time-based state.
    }

    fn render(&mut self, ctx: &mut RenderContext) -> Result<(), String> {
        ctx.canvas.set_blend_mode(BlendMode::Blend);

        for (i, opt) in self.options.iter().enumerate() {
            let ob = self.option_bounds(i);
            let circle_cx = ob.x + RADIO_RADIUS;
            let circle_cy = ob.y + ob.height as i32 / 2;
            let is_selected = opt.value == self.selected;

            // Draw outer circle (border).
            draw_circle(
                ctx.canvas,
                circle_cx,
                circle_cy,
                RADIO_RADIUS,
                Color::RGBA(180, 180, 220, 220),
            )?;

            // Draw filled inner circle for selected option.
            if is_selected {
                draw_filled_circle(
                    ctx.canvas,
                    circle_cx,
                    circle_cy,
                    RADIO_RADIUS - 2,
                    Color::RGBA(200, 200, 255, 255),
                )?;
            }

            // Label text.
            let text_x = ob.x + RADIO_RADIUS * 2 + LABEL_GAP;
            let text_y = circle_cy - font_cache::BITMAP_GLYPH_H as i32 / 2;
            font_cache::draw_text(ctx.canvas, ctx.gfx, FONT, &opt.label, text_x, text_y)?;
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Circle drawing helpers (midpoint circle algorithm)
// ---------------------------------------------------------------------------

/// Draws a circle outline using the midpoint algorithm.
fn draw_circle(
    canvas: &mut sdl2::render::Canvas<sdl2::video::Window>,
    cx: i32,
    cy: i32,
    radius: i32,
    color: Color,
) -> Result<(), String> {
    canvas.set_draw_color(color);
    let mut x = radius;
    let mut y = 0i32;
    let mut err = 0i32;

    while x >= y {
        canvas.draw_point((cx + x, cy + y))?;
        canvas.draw_point((cx + y, cy + x))?;
        canvas.draw_point((cx - y, cy + x))?;
        canvas.draw_point((cx - x, cy + y))?;
        canvas.draw_point((cx - x, cy - y))?;
        canvas.draw_point((cx - y, cy - x))?;
        canvas.draw_point((cx + y, cy - x))?;
        canvas.draw_point((cx + x, cy - y))?;

        y += 1;
        err += 1 + 2 * y;
        if 2 * (err - x) + 1 > 0 {
            x -= 1;
            err += 1 - 2 * x;
        }
    }
    Ok(())
}

/// Draws a filled circle using horizontal scan lines.
fn draw_filled_circle(
    canvas: &mut sdl2::render::Canvas<sdl2::video::Window>,
    cx: i32,
    cy: i32,
    radius: i32,
    color: Color,
) -> Result<(), String> {
    canvas.set_draw_color(color);
    let mut x = radius;
    let mut y = 0i32;
    let mut err = 0i32;

    while x >= y {
        canvas.draw_line((cx - x, cy + y), (cx + x, cy + y))?;
        canvas.draw_line((cx - x, cy - y), (cx + x, cy - y))?;
        canvas.draw_line((cx - y, cy + x), (cx + y, cy + x))?;
        canvas.draw_line((cx - y, cy - x), (cx + y, cy - x))?;

        y += 1;
        err += 1 + 2 * y;
        if 2 * (err - x) + 1 > 0 {
            x -= 1;
            err += 1 - 2 * x;
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Fruit {
        Apple,
        Banana,
        Cherry,
    }

    fn make_vertical() -> RadioGroup<Fruit> {
        RadioGroup::new(
            Bounds::new(10, 10, 200, 60),
            &[
                (Fruit::Apple, "Apple"),
                (Fruit::Banana, "Banana"),
                (Fruit::Cherry, "Cherry"),
            ],
            Fruit::Apple,
        )
    }

    fn make_horizontal() -> RadioGroup<Fruit> {
        RadioGroup::horizontal(
            Bounds::new(10, 10, 300, 20),
            &[
                (Fruit::Apple, "Apple"),
                (Fruit::Banana, "Banana"),
            ],
            Fruit::Banana,
        )
    }

    #[test]
    fn initial_selection() {
        let rg = make_vertical();
        assert_eq!(rg.selected(), Fruit::Apple);
        assert!(!rg.changed);
    }

    #[test]
    fn set_selected_does_not_trigger_changed() {
        let mut rg = make_vertical();
        rg.set_selected(Fruit::Cherry);
        assert_eq!(rg.selected(), Fruit::Cherry);
        assert!(!rg.was_changed());
    }

    #[test]
    fn click_selects_option_vertical() {
        let mut rg = make_vertical();
        let ob1 = rg.option_bounds(1);
        let click = UiEvent::MouseClick {
            x: ob1.x + 5,
            y: ob1.y + 2,
            button: MouseButton::Left,
            modifiers: super::super::widget::KeyModifiers::default(),
        };
        let resp = rg.handle_event(&click);
        assert_eq!(resp, EventResponse::Consumed);
        assert_eq!(rg.selected(), Fruit::Banana);
        assert!(rg.was_changed());
        // One-shot: second call returns false.
        assert!(!rg.was_changed());
    }

    #[test]
    fn click_same_option_no_change() {
        let mut rg = make_vertical();
        let ob0 = rg.option_bounds(0);
        let click = UiEvent::MouseClick {
            x: ob0.x + 5,
            y: ob0.y + 2,
            button: MouseButton::Left,
            modifiers: super::super::widget::KeyModifiers::default(),
        };
        rg.handle_event(&click);
        assert!(!rg.was_changed());
    }

    #[test]
    fn horizontal_initial_selection() {
        let rg = make_horizontal();
        assert_eq!(rg.selected(), Fruit::Banana);
    }
}
