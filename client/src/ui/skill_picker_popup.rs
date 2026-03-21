//! Modal popup for selecting a skill to bind to a skill bar slot.
//!
//! When the player clicks the "+" sigil on an empty skill bar cell, this popup
//! opens and presents a scrollable list of bindable skills. Clicking a row
//! emits [`WidgetAction::BindSkillKey`]; pressing Escape or clicking outside
//! the popup hides it without binding anything.

use sdl2::keyboard::Keycode;
use sdl2::pixels::Color;
use sdl2::rect::Rect;
use sdl2::render::BlendMode;

use super::widget::{Bounds, EventResponse, MouseButton, UiEvent, Widget, WidgetAction};
use super::RenderContext;
use crate::font_cache::{self, BITMAP_GLYPH_ADVANCE, BITMAP_GLYPH_H};

// ---------------------------------------------------------------------------
// Layout constants
// ---------------------------------------------------------------------------

/// Popup width in pixels.
const POPUP_W: u32 = 160;

/// Height of a single skill row.
const ROW_H: u32 = BITMAP_GLYPH_H + 6;

/// Maximum visible rows before scrolling is required.
const MAX_VISIBLE_ROWS: u32 = 12;

/// Horizontal padding inside the popup.
const PAD_X: i32 = 4;

/// Vertical padding between the first row and the popup top edge.
const PAD_Y: i32 = 4;

/// Bitmap font index used for skill names.
const FONT: usize = 1;

/// Background color for the popup body.
const BG_COLOR: Color = Color::RGBA(15, 15, 35, 230);

/// Border color.
const BORDER_COLOR: Color = Color::RGBA(120, 120, 180, 200);

/// Color of the currently-hovered row.
const HOVER_COLOR: Color = Color::RGBA(60, 60, 120, 180);

/// Normal text tint.
const TEXT_COLOR: Color = Color::RGB(200, 200, 220);

// ---------------------------------------------------------------------------
// Bindable skills allow-list
// ---------------------------------------------------------------------------

/// Skills that may be bound to skill bar slots.
///
/// **Placeholder**: fill in with real skill indices once the desired set is
/// decided.  The order here determines the display order in the popup.
pub const BINDABLE_SKILLS: &[u32] = &[
    0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25,
    26, 27, 28, 29, 30, 31, 32, 33, 34, 35,
];

// ---------------------------------------------------------------------------
// Widget
// ---------------------------------------------------------------------------

/// A single row entry in the picker.
#[derive(Clone, Debug)]
struct SkillEntry {
    /// Protocol skill number.
    skill_nr: u32,
    /// Display name.
    name: &'static str,
}

/// Modal popup shown when the player clicks an empty skill bar slot.
///
/// The popup lists [`BINDABLE_SKILLS`] by name and lets the player select
/// one via left-click.  Mouse-wheel scrolls the list.  Escape or a click
/// outside the popup hides it.
pub struct SkillPickerPopup {
    /// Bounding box (recomputed each time the popup opens).
    bounds: Bounds,
    /// Whether the popup is visible.
    visible: bool,
    /// Skill bar slot index the selection will be bound to.
    target_slot: u8,
    /// Rows to display (built once from `BINDABLE_SKILLS`).
    entries: Vec<SkillEntry>,
    /// Index of the first visible row (scroll offset).
    scroll_offset: usize,
    /// Index of the row under the mouse (-1 if none).
    hover_index: Option<usize>,
    /// Mouse position (screen coords) for hover tracking.
    mouse_x: i32,
    mouse_y: i32,
    /// Pending actions for the scene to drain.
    actions: Vec<WidgetAction>,
}

impl SkillPickerPopup {
    /// Creates a new, initially hidden skill picker popup.
    ///
    /// The popup pre-computes its entry list from [`BINDABLE_SKILLS`].
    ///
    /// # Returns
    ///
    /// A `SkillPickerPopup` ready to be shown via [`show`](Self::show).
    pub fn new() -> Self {
        let entries: Vec<SkillEntry> = BINDABLE_SKILLS
            .iter()
            .map(|&nr| SkillEntry {
                skill_nr: nr,
                name: mag_core::skills::get_skill_name(nr as usize),
            })
            .filter(|e| !e.name.is_empty())
            .collect();

        Self {
            bounds: Bounds::new(0, 0, POPUP_W, 0),
            visible: false,
            target_slot: 0,
            entries,
            scroll_offset: 0,
            hover_index: None,
            mouse_x: 0,
            mouse_y: 0,
            actions: Vec::new(),
        }
    }

    /// Shows the popup anchored near the given screen coordinates.
    ///
    /// The popup is positioned so that it stays within the screen bounds
    /// (`TARGET_WIDTH_INT` × `TARGET_HEIGHT_INT`).
    ///
    /// # Arguments
    ///
    /// * `slot` - Skill bar slot index that the chosen skill will be bound to.
    /// * `anchor_x` - Desired left-edge X position (clamped to screen).
    /// * `anchor_y` - Desired top-edge Y position (clamped to screen).
    pub fn show(&mut self, slot: u8, anchor_x: i32, anchor_y: i32) {
        self.visible = true;
        self.target_slot = slot;
        self.scroll_offset = 0;
        self.hover_index = None;

        let visible_rows = (self.entries.len() as u32).min(MAX_VISIBLE_ROWS);
        let popup_h = visible_rows * ROW_H + PAD_Y as u32 * 2;

        let sw = crate::constants::TARGET_WIDTH_INT as i32;
        let sh = crate::constants::TARGET_HEIGHT_INT as i32;
        let x = anchor_x.clamp(0, (sw - POPUP_W as i32).max(0));
        let y = anchor_y.clamp(0, (sh - popup_h as i32).max(0));

        self.bounds = Bounds::new(x, y, POPUP_W, popup_h);
    }

    /// Hides the popup without producing an action.
    pub fn hide(&mut self) {
        self.visible = false;
    }

    /// Returns whether the popup is currently visible.
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Drains pending [`WidgetAction`]s.
    ///
    /// # Returns
    ///
    /// A vector of actions produced since the last call.
    pub fn take_actions(&mut self) -> Vec<WidgetAction> {
        std::mem::take(&mut self.actions)
    }

    // ---- helpers -------------------------------------------------------- //

    /// Number of fully visible rows.
    fn visible_rows(&self) -> usize {
        let avail = self.bounds.height.saturating_sub(PAD_Y as u32 * 2);
        (avail / ROW_H) as usize
    }

    /// Maximum valid scroll offset.
    fn max_scroll(&self) -> usize {
        self.entries.len().saturating_sub(self.visible_rows())
    }

    /// Returns the entry index for a screen coordinate, if any.
    fn hit_row(&self, _sx: i32, sy: i32) -> Option<usize> {
        let local_y = sy - self.bounds.y - PAD_Y;
        if local_y < 0 {
            return None;
        }
        let row = local_y as usize / ROW_H as usize;
        let idx = row + self.scroll_offset;
        if idx < self.entries.len() && row < self.visible_rows() {
            Some(idx)
        } else {
            None
        }
    }
}

impl Widget for SkillPickerPopup {
    /// Returns the bounding rectangle of the popup.
    fn bounds(&self) -> &Bounds {
        &self.bounds
    }

    /// Repositioning is not supported (position set via [`show`](Self::show)).
    fn set_position(&mut self, _x: i32, _y: i32) {}

    /// Handle user input.
    ///
    /// When visible, the popup consumes **all** events (modal behaviour).
    ///
    /// # Arguments
    ///
    /// * `event` - The incoming UI event.
    ///
    /// # Returns
    ///
    /// `Consumed` when visible, `Ignored` otherwise.
    fn handle_event(&mut self, event: &UiEvent) -> EventResponse {
        if !self.visible {
            return EventResponse::Ignored;
        }

        match event {
            UiEvent::KeyDown { keycode, .. } => {
                if *keycode == Keycode::Escape {
                    self.hide();
                }
                EventResponse::Consumed
            }
            UiEvent::MouseMove { x, y } => {
                self.mouse_x = *x;
                self.mouse_y = *y;
                self.hover_index = if self.bounds.contains_point(*x, *y) {
                    self.hit_row(*x, *y)
                } else {
                    None
                };
                EventResponse::Consumed
            }
            UiEvent::MouseClick { x, y, button, .. } => {
                self.mouse_x = *x;
                self.mouse_y = *y;

                if !self.bounds.contains_point(*x, *y) {
                    // Clicked outside — close.
                    self.hide();
                    return EventResponse::Consumed;
                }

                if *button == MouseButton::Left {
                    if let Some(idx) = self.hit_row(*x, *y) {
                        let entry = &self.entries[idx];
                        self.actions.push(WidgetAction::BindSkillKey {
                            skill_nr: entry.skill_nr,
                            key_slot: self.target_slot,
                        });
                        self.hide();
                    }
                }
                EventResponse::Consumed
            }
            UiEvent::MouseWheel { delta, .. } => {
                if *delta > 0 {
                    self.scroll_offset = self.scroll_offset.saturating_sub(*delta as usize);
                } else {
                    self.scroll_offset =
                        (self.scroll_offset + delta.unsigned_abs() as usize).min(self.max_scroll());
                }
                // Refresh hover after scroll.
                self.hover_index = if self.bounds.contains_point(self.mouse_x, self.mouse_y) {
                    self.hit_row(self.mouse_x, self.mouse_y)
                } else {
                    None
                };
                EventResponse::Consumed
            }
            _ => EventResponse::Consumed,
        }
    }

    /// Render the popup.
    ///
    /// # Arguments
    ///
    /// * `ctx` - Mutable render context.
    ///
    /// # Returns
    ///
    /// `Ok(())` on success, or an SDL2 error string.
    fn render(&mut self, ctx: &mut RenderContext<'_, '_>) -> Result<(), String> {
        if !self.visible {
            return Ok(());
        }

        ctx.canvas.set_blend_mode(BlendMode::Blend);

        // Background.
        ctx.canvas.set_draw_color(BG_COLOR);
        ctx.canvas.fill_rect(Rect::new(
            self.bounds.x,
            self.bounds.y,
            self.bounds.width,
            self.bounds.height,
        ))?;

        // Border.
        ctx.canvas.set_draw_color(BORDER_COLOR);
        ctx.canvas.draw_rect(Rect::new(
            self.bounds.x,
            self.bounds.y,
            self.bounds.width,
            self.bounds.height,
        ))?;

        // Rows.
        let vis = self.visible_rows();
        for i in 0..vis {
            let idx = self.scroll_offset + i;
            if idx >= self.entries.len() {
                break;
            }
            let row_y = self.bounds.y + PAD_Y + i as i32 * ROW_H as i32;

            // Highlight hovered row.
            if self.hover_index == Some(idx) {
                ctx.canvas.set_draw_color(HOVER_COLOR);
                ctx.canvas.fill_rect(Rect::new(
                    self.bounds.x + 1,
                    row_y,
                    self.bounds.width - 2,
                    ROW_H,
                ))?;
            }

            // Skill name.
            let name = self.entries[idx].name;
            let text_y = row_y + (ROW_H as i32 - BITMAP_GLYPH_H as i32) / 2;
            font_cache::draw_text(
                ctx.canvas,
                ctx.gfx,
                FONT,
                name,
                self.bounds.x + PAD_X,
                text_y,
                font_cache::TextStyle::PLAIN.with_tint(TEXT_COLOR),
            )?;
        }

        // Scroll indicator: small arrows when list is scrollable.
        let max = self.max_scroll();
        if max > 0 {
            let indicator_x =
                self.bounds.x + self.bounds.width as i32 - PAD_X - BITMAP_GLYPH_ADVANCE as i32;
            if self.scroll_offset > 0 {
                font_cache::draw_text(
                    ctx.canvas,
                    ctx.gfx,
                    FONT,
                    "^",
                    indicator_x,
                    self.bounds.y + 2,
                    font_cache::TextStyle::PLAIN.with_tint(TEXT_COLOR),
                )?;
            }
            if self.scroll_offset < max {
                font_cache::draw_text(
                    ctx.canvas,
                    ctx.gfx,
                    FONT,
                    "v",
                    indicator_x,
                    self.bounds.y + self.bounds.height as i32 - BITMAP_GLYPH_H as i32 - 2,
                    font_cache::TextStyle::PLAIN.with_tint(TEXT_COLOR),
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

    #[test]
    fn starts_hidden() {
        let popup = SkillPickerPopup::new();
        assert!(!popup.is_visible());
    }

    #[test]
    fn show_sets_visible_and_slot() {
        let mut popup = SkillPickerPopup::new();
        popup.show(5, 100, 200);
        assert!(popup.is_visible());
        assert_eq!(popup.target_slot, 5);
    }

    #[test]
    fn hide_clears_visible() {
        let mut popup = SkillPickerPopup::new();
        popup.show(0, 0, 0);
        popup.hide();
        assert!(!popup.is_visible());
    }

    #[test]
    fn escape_hides_popup() {
        let mut popup = SkillPickerPopup::new();
        popup.show(0, 0, 0);
        let resp = popup.handle_event(&UiEvent::KeyDown {
            keycode: Keycode::Escape,
            modifiers: super::super::widget::KeyModifiers::default(),
        });
        assert_eq!(resp, EventResponse::Consumed);
        assert!(!popup.is_visible());
    }

    #[test]
    fn click_outside_hides() {
        let mut popup = SkillPickerPopup::new();
        popup.show(0, 50, 50);
        // Click far outside the popup bounds.
        let resp = popup.handle_event(&UiEvent::MouseClick {
            x: 900,
            y: 500,
            button: MouseButton::Left,
            modifiers: super::super::widget::KeyModifiers::default(),
        });
        assert_eq!(resp, EventResponse::Consumed);
        assert!(!popup.is_visible());
    }

    #[test]
    fn click_row_emits_bind_action() {
        let mut popup = SkillPickerPopup::new();
        popup.show(3, 10, 10);
        // Click on the first row.
        let row_y = popup.bounds.y + PAD_Y + (ROW_H as i32 / 2);
        let resp = popup.handle_event(&UiEvent::MouseClick {
            x: popup.bounds.x + 20,
            y: row_y,
            button: MouseButton::Left,
            modifiers: super::super::widget::KeyModifiers::default(),
        });
        assert_eq!(resp, EventResponse::Consumed);
        assert!(!popup.is_visible());
        let actions = popup.take_actions();
        assert_eq!(actions.len(), 1);
        match &actions[0] {
            WidgetAction::BindSkillKey { skill_nr, key_slot } => {
                assert_eq!(*key_slot, 3);
                // First entry should be skill 0.
                assert_eq!(*skill_nr, popup.entries[0].skill_nr);
            }
            other => panic!("Expected BindSkillKey, got {:?}", other),
        }
    }

    #[test]
    fn ignored_when_hidden() {
        let mut popup = SkillPickerPopup::new();
        let resp = popup.handle_event(&UiEvent::MouseClick {
            x: 50,
            y: 50,
            button: MouseButton::Left,
            modifiers: super::super::widget::KeyModifiers::default(),
        });
        assert_eq!(resp, EventResponse::Ignored);
    }

    #[test]
    fn entries_populated_from_bindable_skills() {
        let popup = SkillPickerPopup::new();
        assert!(!popup.entries.is_empty());
        // All entries should have non-empty names.
        for e in &popup.entries {
            assert!(!e.name.is_empty(), "skill {} has empty name", e.skill_nr);
        }
    }

    #[test]
    fn scroll_clamps_to_max() {
        let mut popup = SkillPickerPopup::new();
        popup.show(0, 0, 0);
        // Scroll way past the end.
        popup.handle_event(&UiEvent::MouseWheel {
            x: 0,
            y: 0,
            delta: -1000,
        });
        assert_eq!(popup.scroll_offset, popup.max_scroll());
        // Scroll way past the beginning.
        popup.handle_event(&UiEvent::MouseWheel {
            x: 0,
            y: 0,
            delta: 1000,
        });
        assert_eq!(popup.scroll_offset, 0);
    }

    #[test]
    fn bounds_clamped_to_screen() {
        let mut popup = SkillPickerPopup::new();
        popup.show(0, 9999, 9999);
        let sw = crate::constants::TARGET_WIDTH_INT as i32;
        let sh = crate::constants::TARGET_HEIGHT_INT as i32;
        assert!(popup.bounds.x + popup.bounds.width as i32 <= sw);
        assert!(popup.bounds.y + popup.bounds.height as i32 <= sh);
    }
}
