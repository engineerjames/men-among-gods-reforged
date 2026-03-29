//! Expandable dropdown selection widget.

use sdl2::pixels::Color;
use sdl2::render::BlendMode;

use crate::font_cache;
use crate::ui::RenderContext;
use crate::ui::widget::{Bounds, EventResponse, MouseButton, UiEvent, Widget};

/// Height of each option row in the expanded list in pixels.
const OPTION_ROW_H: u32 = 14;
/// Horizontal padding inside the collapsed header and option rows.
const H_PAD: i32 = 4;

/// A dropdown selector that expands to show a list of options.
///
/// When collapsed, renders a bordered rectangle with the selected option
/// text and a "▼" indicator. When expanded, renders the option list below
/// the header. The parent widget should render the dropdown **last** when
/// expanded (for correct z-order) and delegate events to it **first** so
/// it can capture clicks on the overlay.
pub struct Dropdown {
    /// Bounds of the collapsed header.
    bounds: Bounds,
    options: Vec<String>,
    selected: usize,
    expanded: bool,
    hovered_option: Option<usize>,
    font: usize,
    /// One-shot flag indicating the selection changed since last read.
    changed: bool,
    /// Additive tint alpha for hovered items (0–255).
    hover_alpha: u8,
}

impl Dropdown {
    /// Creates a new dropdown.
    ///
    /// # Arguments
    ///
    /// * `bounds` - Position and size of the collapsed header.
    /// * `options` - Display strings for each selectable option.
    /// * `selected` - Index of the initially-selected option.
    /// * `font` - Bitmap font index (0–3).
    ///
    /// # Returns
    ///
    /// A new `Dropdown`, initially collapsed.
    ///
    /// # Panics
    ///
    /// Panics if `options` is empty or `selected >= options.len()`.
    pub fn new(bounds: Bounds, options: Vec<String>, selected: usize, font: usize) -> Self {
        assert!(!options.is_empty(), "Dropdown needs at least one option");
        assert!(
            selected < options.len(),
            "selected index {} out of range (len {})",
            selected,
            options.len()
        );
        Self {
            bounds,
            options,
            selected,
            expanded: false,
            hovered_option: None,
            font,
            changed: false,
            hover_alpha: 80,
        }
    }

    /// Returns the index of the currently selected option.
    pub fn selected_index(&self) -> usize {
        self.selected
    }

    /// Sets the selected index without triggering the changed flag.
    ///
    /// # Arguments
    ///
    /// * `index` - Option index. Must be `< options.len()`.
    ///
    /// # Panics
    ///
    /// Panics if `index >= options.len()`.
    pub fn set_selected(&mut self, index: usize) {
        assert!(
            index < self.options.len(),
            "set_selected index {} out of range (len {})",
            index,
            self.options.len()
        );
        self.selected = index;
    }

    /// Returns `true` once if the selection changed since the last call.
    ///
    /// Clears the flag on read.
    pub fn was_changed(&mut self) -> bool {
        let c = self.changed;
        self.changed = false;
        c
    }

    /// Returns whether the option list is currently expanded.
    pub fn is_expanded(&self) -> bool {
        self.expanded
    }

    /// Returns the bounding rectangle of the full expanded area
    /// (header + option list).
    fn expanded_bounds(&self) -> Bounds {
        let list_h = self.options.len() as u32 * OPTION_ROW_H;
        Bounds::new(
            self.bounds.x,
            self.bounds.y,
            self.bounds.width,
            self.bounds.height + list_h,
        )
    }

    /// Returns the option index at the given absolute `(x, y)` position
    /// within the expanded list area, or `None` if outside.
    fn option_at(&self, x: i32, y: i32) -> Option<usize> {
        let list_top = self.bounds.y + self.bounds.height as i32;
        let list_bottom = list_top + (self.options.len() as u32 * OPTION_ROW_H) as i32;
        if x >= self.bounds.x
            && x < self.bounds.x + self.bounds.width as i32
            && y >= list_top
            && y < list_bottom
        {
            let idx = ((y - list_top) / OPTION_ROW_H as i32) as usize;
            if idx < self.options.len() {
                return Some(idx);
            }
        }
        None
    }
}

impl Widget for Dropdown {
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
                if self.expanded {
                    self.hovered_option = self.option_at(*x, *y);
                }
                EventResponse::Ignored
            }
            UiEvent::MouseClick {
                x,
                y,
                button: MouseButton::Left,
                ..
            } => {
                if self.expanded {
                    // Check if clicked on an option row.
                    if let Some(idx) = self.option_at(*x, *y) {
                        if idx != self.selected {
                            self.selected = idx;
                            self.changed = true;
                        }
                        self.expanded = false;
                        self.hovered_option = None;
                        return EventResponse::Consumed;
                    }
                    // Click on header or outside — just collapse.
                    let eb = self.expanded_bounds();
                    self.expanded = false;
                    self.hovered_option = None;
                    // Consume if click was inside the expanded area (header or list)
                    // to prevent click-through.
                    if eb.contains_point(*x, *y) {
                        return EventResponse::Consumed;
                    }
                    // Click was clearly outside the entire dropdown area.
                    return EventResponse::Consumed;
                }

                // Collapsed: click on header opens.
                if self.bounds.contains_point(*x, *y) {
                    self.expanded = true;
                    return EventResponse::Consumed;
                }

                EventResponse::Ignored
            }
            _ => EventResponse::Ignored,
        }
    }

    fn render(&mut self, ctx: &mut RenderContext<'_, '_>) -> Result<(), String> {
        ctx.canvas.set_blend_mode(BlendMode::Blend);

        // --- Header ---
        let header_rect = sdl2::rect::Rect::new(
            self.bounds.x,
            self.bounds.y,
            self.bounds.width,
            self.bounds.height,
        );

        // Background
        ctx.canvas.set_draw_color(Color::RGBA(30, 30, 50, 220));
        ctx.canvas.fill_rect(header_rect)?;

        // Border
        ctx.canvas.set_draw_color(Color::RGBA(120, 120, 140, 200));
        ctx.canvas.draw_rect(header_rect)?;

        // Selected text
        let text_y =
            self.bounds.y + (self.bounds.height as i32 - font_cache::BITMAP_GLYPH_H as i32) / 2;
        if let Some(text) = self.options.get(self.selected) {
            font_cache::draw_text(
                ctx.canvas,
                ctx.gfx,
                self.font,
                text,
                self.bounds.x + H_PAD,
                text_y,
                font_cache::TextStyle::PLAIN,
            )?;
        }

        // Arrow indicator "▼" on the right
        let arrow_text = if self.expanded { "^" } else { "v" };
        let arrow_x = self.bounds.x + self.bounds.width as i32
            - font_cache::text_width(arrow_text) as i32
            - H_PAD;
        font_cache::draw_text(
            ctx.canvas,
            ctx.gfx,
            self.font,
            arrow_text,
            arrow_x,
            text_y,
            font_cache::TextStyle::PLAIN,
        )?;

        // --- Expanded option list ---
        if self.expanded {
            let list_top = self.bounds.y + self.bounds.height as i32;
            let list_h = self.options.len() as u32 * OPTION_ROW_H;
            let list_rect =
                sdl2::rect::Rect::new(self.bounds.x, list_top, self.bounds.width, list_h);

            // List background
            ctx.canvas.set_draw_color(Color::RGBA(20, 20, 40, 240));
            ctx.canvas.fill_rect(list_rect)?;

            // List border
            ctx.canvas.set_draw_color(Color::RGBA(120, 120, 140, 200));
            ctx.canvas.draw_rect(list_rect)?;

            // Option rows
            for (i, option) in self.options.iter().enumerate() {
                let row_y = list_top + (i as u32 * OPTION_ROW_H) as i32;
                let row_rect = sdl2::rect::Rect::new(
                    self.bounds.x + 1,
                    row_y,
                    self.bounds.width.saturating_sub(2),
                    OPTION_ROW_H,
                );

                // Highlight hovered or selected
                if Some(i) == self.hovered_option {
                    ctx.canvas.set_blend_mode(BlendMode::Add);
                    ctx.canvas
                        .set_draw_color(Color::RGBA(255, 255, 255, self.hover_alpha));
                    ctx.canvas.fill_rect(row_rect)?;
                    ctx.canvas.set_blend_mode(BlendMode::Blend);
                } else if i == self.selected {
                    ctx.canvas.set_draw_color(Color::RGBA(60, 60, 90, 200));
                    ctx.canvas.fill_rect(row_rect)?;
                }

                let opt_text_y =
                    row_y + (OPTION_ROW_H as i32 - font_cache::BITMAP_GLYPH_H as i32) / 2;
                font_cache::draw_text(
                    ctx.canvas,
                    ctx.gfx,
                    self.font,
                    option,
                    self.bounds.x + H_PAD,
                    opt_text_y,
                    font_cache::TextStyle::PLAIN,
                )?;
            }
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

    fn make_dropdown() -> Dropdown {
        Dropdown::new(
            Bounds::new(10, 10, 150, 16),
            vec!["Windowed".into(), "Fullscreen".into(), "Borderless".into()],
            0,
            0,
        )
    }

    #[test]
    fn starts_collapsed() {
        let dd = make_dropdown();
        assert!(!dd.is_expanded());
        assert_eq!(dd.selected_index(), 0);
    }

    #[test]
    fn click_header_expands() {
        let mut dd = make_dropdown();
        let resp = dd.handle_event(&UiEvent::MouseClick {
            x: 50,
            y: 15,
            button: MouseButton::Left,
            modifiers: KeyModifiers::default(),
        });
        assert_eq!(resp, EventResponse::Consumed);
        assert!(dd.is_expanded());
    }

    #[test]
    fn select_option_collapses_and_sets_changed() {
        let mut dd = make_dropdown();
        // Open
        dd.handle_event(&UiEvent::MouseClick {
            x: 50,
            y: 15,
            button: MouseButton::Left,
            modifiers: KeyModifiers::default(),
        });
        assert!(dd.is_expanded());

        // Click second option (y = header_bottom + row_height/2)
        let option_y = 10 + 16 + (OPTION_ROW_H as i32) + 2; // middle of second row
        let resp = dd.handle_event(&UiEvent::MouseClick {
            x: 50,
            y: option_y,
            button: MouseButton::Left,
            modifiers: KeyModifiers::default(),
        });
        assert_eq!(resp, EventResponse::Consumed);
        assert!(!dd.is_expanded());
        assert_eq!(dd.selected_index(), 1);
        assert!(dd.was_changed());
        assert!(!dd.was_changed()); // clears on read
    }

    #[test]
    fn click_outside_collapses_without_change() {
        let mut dd = make_dropdown();
        // Open
        dd.handle_event(&UiEvent::MouseClick {
            x: 50,
            y: 15,
            button: MouseButton::Left,
            modifiers: KeyModifiers::default(),
        });
        assert!(dd.is_expanded());

        // Click way outside
        let resp = dd.handle_event(&UiEvent::MouseClick {
            x: 500,
            y: 500,
            button: MouseButton::Left,
            modifiers: KeyModifiers::default(),
        });
        assert_eq!(resp, EventResponse::Consumed); // consumes to prevent click-through
        assert!(!dd.is_expanded());
        assert!(!dd.was_changed());
    }

    #[test]
    fn set_selected_does_not_trigger_changed() {
        let mut dd = make_dropdown();
        dd.set_selected(2);
        assert_eq!(dd.selected_index(), 2);
        assert!(!dd.was_changed());
    }

    #[test]
    #[should_panic(expected = "out of range")]
    fn set_selected_out_of_range_panics() {
        let mut dd = make_dropdown();
        dd.set_selected(5);
    }

    #[test]
    fn click_collapsed_outside_ignored() {
        let mut dd = make_dropdown();
        let resp = dd.handle_event(&UiEvent::MouseClick {
            x: 0,
            y: 0,
            button: MouseButton::Left,
            modifiers: KeyModifiers::default(),
        });
        assert_eq!(resp, EventResponse::Ignored);
        assert!(!dd.is_expanded());
    }

    #[test]
    fn reselecting_same_option_no_change() {
        let mut dd = make_dropdown();
        // Open
        dd.handle_event(&UiEvent::MouseClick {
            x: 50,
            y: 15,
            button: MouseButton::Left,
            modifiers: KeyModifiers::default(),
        });
        // Click first option (already selected)
        let option_y = 10 + 16 + 2; // middle of first row
        dd.handle_event(&UiEvent::MouseClick {
            x: 50,
            y: option_y,
            button: MouseButton::Left,
            modifiers: KeyModifiers::default(),
        });
        assert!(!dd.is_expanded());
        assert!(!dd.was_changed());
    }
}
