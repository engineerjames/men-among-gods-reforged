//! Scrollable, selectable list widget.
//!
//! Displays a vertical list of labeled rows, each optionally preceded by a
//! sprite thumbnail.  One row at a time can be selected.  The list scrolls
//! via the mouse wheel when the cursor is inside the widget bounds.

use std::time::Duration;

use sdl2::pixels::Color;
use sdl2::rect::Rect;
use sdl2::render::BlendMode;

use super::RenderContext;
use super::widget::{Bounds, EventResponse, MouseButton, UiEvent, Widget};
use crate::font_cache;

// ---------------------------------------------------------------------------
// Layout constants
// ---------------------------------------------------------------------------

/// Row height in pixels (sprite thumbnail + label).
const ROW_H: i32 = 68;

/// Size of the sprite thumbnail square.
const THUMB_SIZE: u32 = 64;

/// Horizontal gap between thumbnail and label text.
const THUMB_TEXT_GAP: i32 = 6;

/// Inner horizontal padding from the list edge.
const PAD_X: i32 = 4;

/// Bitmap font index for labels.
const FONT: usize = 1;

/// Width of the scrollbar track.
const SCROLLBAR_W: u32 = 6;

/// Minimum scrollbar knob height.
const SCROLLBAR_KNOB_MIN_H: u32 = 10;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// A single item in the list.
#[derive(Clone, Debug)]
pub struct ListItem {
    /// Unique identifier (e.g. character ID).
    pub id: u64,
    /// Display label.
    pub label: String,
    /// Optional sprite ID to render as a thumbnail.
    pub sprite_id: Option<usize>,
}

// ---------------------------------------------------------------------------
// Widget
// ---------------------------------------------------------------------------

/// A scrollable list of selectable items.
pub struct ScrollableList {
    bounds: Bounds,
    items: Vec<ListItem>,
    selected_id: Option<u64>,
    scroll_offset: usize,
    /// One-shot flag indicating the selection changed.
    selection_changed: bool,
}

impl ScrollableList {
    /// Creates a new empty scrollable list.
    ///
    /// # Arguments
    ///
    /// * `bounds` - Position and size of the widget.
    ///
    /// # Returns
    ///
    /// A new `ScrollableList` with no items.
    pub fn new(bounds: Bounds) -> Self {
        Self {
            bounds,
            items: Vec::new(),
            selected_id: None,
            scroll_offset: 0,
            selection_changed: false,
        }
    }

    /// Replaces all items in the list and resets scroll and selection.
    ///
    /// # Arguments
    ///
    /// * `items` - The new list items.
    pub fn set_items(&mut self, items: Vec<ListItem>) {
        self.items = items;
        self.scroll_offset = 0;
        self.selected_id = self.items.first().map(|i| i.id);
        self.selection_changed = false;
    }

    /// Returns the currently selected item ID, if any.
    pub fn selected_id(&self) -> Option<u64> {
        self.selected_id
    }

    /// Programmatically sets the selected item by ID.
    ///
    /// # Arguments
    ///
    /// * `id` - The item ID to select, or `None` to clear selection.
    pub fn set_selected(&mut self, id: Option<u64>) {
        self.selected_id = id;
    }

    /// Returns `true` once if the selection changed since the last call,
    /// then clears the flag.
    pub fn was_selection_changed(&mut self) -> bool {
        let c = self.selection_changed;
        self.selection_changed = false;
        c
    }

    /// Returns the number of fully visible rows.
    fn visible_rows(&self) -> usize {
        (self.bounds.height as i32 / ROW_H).max(1) as usize
    }

    /// Returns the maximum scroll offset.
    fn max_scroll(&self) -> usize {
        self.items.len().saturating_sub(self.visible_rows())
    }

    /// Clamps scroll offset to valid range.
    fn clamp_scroll(&mut self) {
        let max = self.max_scroll();
        if self.scroll_offset > max {
            self.scroll_offset = max;
        }
    }

    /// Returns `true` if the item list is empty.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Removes an item by ID and adjusts selection + scroll.
    ///
    /// # Arguments
    ///
    /// * `id` - The ID of the item to remove.
    pub fn remove_item(&mut self, id: u64) {
        if let Some(pos) = self.items.iter().position(|i| i.id == id) {
            self.items.remove(pos);
            if self.selected_id == Some(id) {
                self.selected_id = self.items.first().map(|i| i.id);
            }
            self.clamp_scroll();
        }
    }
}

impl Widget for ScrollableList {
    fn bounds(&self) -> &Bounds {
        &self.bounds
    }

    fn set_position(&mut self, x: i32, y: i32) {
        self.bounds.x = x;
        self.bounds.y = y;
    }

    fn handle_event(&mut self, event: &UiEvent) -> EventResponse {
        match event {
            // ── Scroll ──────────────────────────────────────────────────
            UiEvent::MouseWheel { x, y, delta } => {
                if !self.bounds.contains_point(*x, *y) {
                    return EventResponse::Ignored;
                }
                if *delta < 0 {
                    // Scroll down.
                    self.scroll_offset = (self.scroll_offset + 1).min(self.max_scroll());
                } else if *delta > 0 {
                    // Scroll up.
                    self.scroll_offset = self.scroll_offset.saturating_sub(1);
                }
                EventResponse::Consumed
            }

            // ── Click to select ─────────────────────────────────────────
            UiEvent::MouseClick {
                x,
                y,
                button: MouseButton::Left,
                ..
            } => {
                if !self.bounds.contains_point(*x, *y) {
                    return EventResponse::Ignored;
                }
                let row = (*y - self.bounds.y) / ROW_H;
                let index = self.scroll_offset + row as usize;
                if let Some(item) = self.items.get(index) {
                    if self.selected_id != Some(item.id) {
                        self.selected_id = Some(item.id);
                        self.selection_changed = true;
                    }
                    return EventResponse::Consumed;
                }
                EventResponse::Consumed
            }

            _ => EventResponse::Ignored,
        }
    }

    fn update(&mut self, _dt: Duration) {
        // No time-based state.
    }

    fn render(&mut self, ctx: &mut RenderContext<'_, '_>) -> Result<(), String> {
        ctx.canvas.set_blend_mode(BlendMode::Blend);

        // ── Background ──────────────────────────────────────────────────
        let bg_rect = Rect::new(
            self.bounds.x,
            self.bounds.y,
            self.bounds.width,
            self.bounds.height,
        );
        ctx.canvas.set_draw_color(Color::RGBA(15, 15, 30, 180));
        ctx.canvas.fill_rect(bg_rect)?;
        ctx.canvas.set_draw_color(Color::RGBA(80, 80, 120, 200));
        ctx.canvas.draw_rect(bg_rect)?;

        let visible = self.visible_rows();
        let needs_scrollbar = self.items.len() > visible;
        let content_w = if needs_scrollbar {
            self.bounds.width.saturating_sub(SCROLLBAR_W + 2)
        } else {
            self.bounds.width
        };

        for i in 0..visible {
            let index = self.scroll_offset + i;
            let Some(item) = self.items.get(index) else {
                break;
            };

            let row_y = self.bounds.y + i as i32 * ROW_H;
            let is_selected = self.selected_id == Some(item.id);

            // Highlight selected row.
            if is_selected {
                let highlight = Rect::new(
                    self.bounds.x + 1,
                    row_y,
                    content_w.saturating_sub(1),
                    ROW_H as u32,
                );
                ctx.canvas.set_draw_color(Color::RGBA(60, 60, 120, 180));
                ctx.canvas.fill_rect(highlight)?;
            }

            let mut text_x = self.bounds.x + PAD_X;

            // Sprite thumbnail.
            if let Some(sprite_id) = item.sprite_id {
                let thumb_y = row_y + (ROW_H - THUMB_SIZE as i32) / 2;
                let texture = ctx.gfx.get_texture(sprite_id);
                let dest = Rect::new(text_x, thumb_y, THUMB_SIZE, THUMB_SIZE);
                let _ = ctx.canvas.copy(texture, None, dest);
                text_x += THUMB_SIZE as i32 + THUMB_TEXT_GAP;
            }

            // Label text (vertically centered in the row).
            let text_y = row_y + (ROW_H - font_cache::BITMAP_GLYPH_H as i32) / 2;
            font_cache::draw_text(
                ctx.canvas,
                ctx.gfx,
                FONT,
                &item.label,
                text_x,
                text_y,
                font_cache::TextStyle::PLAIN,
            )?;
        }

        // ── Scrollbar ───────────────────────────────────────────────────
        if needs_scrollbar {
            let track_x = self.bounds.x + self.bounds.width as i32 - SCROLLBAR_W as i32 - 1;
            let track_h = self.bounds.height;
            let track_rect = Rect::new(track_x, self.bounds.y, SCROLLBAR_W, track_h);
            ctx.canvas.set_draw_color(Color::RGBA(30, 30, 50, 180));
            ctx.canvas.fill_rect(track_rect)?;

            let total = self.items.len() as f32;
            let knob_h =
                ((visible as f32 / total) * track_h as f32).max(SCROLLBAR_KNOB_MIN_H as f32) as u32;
            let max_scroll = self.max_scroll().max(1) as f32;
            let knob_y = self.bounds.y
                + ((self.scroll_offset as f32 / max_scroll) * (track_h - knob_h) as f32) as i32;
            let knob_rect = Rect::new(track_x, knob_y, SCROLLBAR_W, knob_h);
            ctx.canvas.set_draw_color(Color::RGBA(100, 100, 160, 200));
            ctx.canvas.fill_rect(knob_rect)?;
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

    fn make_list(n: usize) -> ScrollableList {
        let mut list = ScrollableList::new(Bounds::new(0, 0, 200, 204)); // 3 rows visible (3 × 68)
        let items: Vec<ListItem> = (0..n)
            .map(|i| ListItem {
                id: i as u64,
                label: format!("Item {}", i),
                sprite_id: None,
            })
            .collect();
        list.set_items(items);
        list
    }

    #[test]
    fn initial_selection_is_first_item() {
        let list = make_list(5);
        assert_eq!(list.selected_id(), Some(0));
    }

    #[test]
    fn empty_list_has_no_selection() {
        let list = ScrollableList::new(Bounds::new(0, 0, 200, 204));
        assert_eq!(list.selected_id(), None);
    }

    #[test]
    fn click_selects_item() {
        let mut list = make_list(5);
        // Click the second row (y = ROW_H * 1 + some offset within).
        let click = UiEvent::MouseClick {
            x: 50,
            y: ROW_H + 2,
            button: MouseButton::Left,
            modifiers: super::super::widget::KeyModifiers::default(),
        };
        let resp = list.handle_event(&click);
        assert_eq!(resp, EventResponse::Consumed);
        assert_eq!(list.selected_id(), Some(1));
        assert!(list.was_selection_changed());
        assert!(!list.was_selection_changed()); // One-shot.
    }

    #[test]
    fn scroll_down_shifts_view() {
        let mut list = make_list(10);
        assert_eq!(list.scroll_offset, 0);
        let scroll = UiEvent::MouseWheel {
            x: 50,
            y: 10,
            delta: -1,
        };
        list.handle_event(&scroll);
        assert_eq!(list.scroll_offset, 1);
    }

    #[test]
    fn scroll_does_not_exceed_max() {
        let mut list = make_list(4); // 3 visible, max_scroll = 1
        let scroll = UiEvent::MouseWheel {
            x: 50,
            y: 10,
            delta: -1,
        };
        list.handle_event(&scroll);
        list.handle_event(&scroll);
        list.handle_event(&scroll);
        assert_eq!(list.scroll_offset, 1);
    }

    #[test]
    fn remove_item_adjusts_selection() {
        let mut list = make_list(3);
        list.set_selected(Some(1));
        list.remove_item(1);
        assert_eq!(list.selected_id(), Some(0)); // Falls back to first.
        assert_eq!(list.items.len(), 2);
    }
}
