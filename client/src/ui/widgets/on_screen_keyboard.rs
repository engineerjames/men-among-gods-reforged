//! On-screen QWERTY keyboard for controller text input.
//!
//! Displayed when controller mode is active and a text field has focus.
//! Navigation: D-pad/left-stick to move between keys, A to type, B to
//! backspace, X to toggle shift/caps, Start to dismiss.

use std::time::Duration;

use sdl2::pixels::Color;
use sdl2::render::BlendMode;

use crate::font_cache;
use crate::ui::RenderContext;
use crate::ui::widget::{Bounds, EventResponse, UiEvent, Widget};

// ---------------------------------------------------------------------------
// Layout
// ---------------------------------------------------------------------------

/// Key cell width in pixels.
const KEY_W: u32 = 18;
/// Key cell height in pixels.
const KEY_H: u32 = 18;
/// Gap between key cells in pixels.
const KEY_GAP: u32 = 2;
/// Horizontal padding inside the keyboard panel.
const PAD_X: i32 = 6;
/// Vertical padding inside the keyboard panel.
const PAD_Y: i32 = 6;
/// Height of the hint bar below the keys ("A=type  B=bksp  X=shift").
const HINT_H: i32 = 14;
/// Bitmap font index.
const FONT: usize = 0;

// ---------------------------------------------------------------------------
// Key rows (lower-case layer)
// ---------------------------------------------------------------------------

/// Row definitions — each row is a static slice of `&str` labels.
const ROWS: &[&[&str]] = &[
    &["1", "2", "3", "4", "5", "6", "7", "8", "9", "0", "-", "="],
    &["q", "w", "e", "r", "t", "y", "u", "i", "o", "p"],
    &["a", "s", "d", "f", "g", "h", "j", "k", "l", ";", "'"],
    &["z", "x", "c", "v", "b", "n", "m", ",", ".", "/"],
    &["SPACE", "ENTER"],
];

/// Shifted equivalents for each row.
const ROWS_SHIFT: &[&[&str]] = &[
    &["!", "@", "#", "$", "%", "^", "&", "*", "(", ")", "_", "+"],
    &["Q", "W", "E", "R", "T", "Y", "U", "I", "O", "P"],
    &["A", "S", "D", "F", "G", "H", "J", "K", "L", ":", "\""],
    &["Z", "X", "C", "V", "B", "N", "M", "<", ">", "?"],
    &["SPACE", "ENTER"],
];

/// Returns the number of columns in the widest row.
const fn max_cols() -> usize {
    let mut max = 0;
    let mut i = 0;
    while i < ROWS.len() {
        if ROWS[i].len() > max {
            max = ROWS[i].len();
        }
        i += 1;
    }
    max
}

/// Maximum number of columns across all rows.
const MAX_COLS: usize = max_cols();

/// Computed keyboard panel width.
const KB_W: u32 = PAD_X as u32 * 2 + MAX_COLS as u32 * KEY_W + (MAX_COLS as u32 - 1) * KEY_GAP;
/// Computed keyboard panel height.
const KB_H: u32 = PAD_Y as u32 * 2
    + ROWS.len() as u32 * KEY_H
    + (ROWS.len() as u32 - 1) * KEY_GAP
    + HINT_H as u32;

// ---------------------------------------------------------------------------
// Actions produced by the keyboard
// ---------------------------------------------------------------------------

/// Actions emitted by the on-screen keyboard for the parent to process.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OnScreenKeyboardAction {
    /// A character was typed.
    TypeChar(char),
    /// Backspace was pressed.
    Backspace,
    /// The keyboard was dismissed (Start pressed).
    Dismiss,
    /// The Enter/Send key was pressed on the on-screen keyboard.
    Submit,
}

// ---------------------------------------------------------------------------
// Widget
// ---------------------------------------------------------------------------

/// On-screen QWERTY keyboard navigated with controller inputs.
pub struct OnScreenKeyboard {
    /// Bounding rectangle of the entire keyboard panel.
    bounds: Bounds,
    /// Whether the keyboard is currently visible.
    visible: bool,
    /// Currently focused row index.
    focused_row: usize,
    /// Currently focused column index within the row.
    focused_col: usize,
    /// Whether shift is active (one-shot or toggled).
    shift_active: bool,
    /// Pending output actions.
    actions: Vec<OnScreenKeyboardAction>,
}

impl OnScreenKeyboard {
    /// Creates a new on-screen keyboard, initially hidden, centered
    /// horizontally at the bottom of the target viewport.
    ///
    /// # Returns
    ///
    /// A new `OnScreenKeyboard`.
    pub fn new() -> Self {
        let x = (crate::constants::TARGET_WIDTH_INT as i32 - KB_W as i32) / 2;
        let y = crate::constants::TARGET_HEIGHT_INT as i32 - KB_H as i32 - 8;
        Self {
            bounds: Bounds::new(x, y, KB_W, KB_H),
            visible: false,
            focused_row: 1,
            focused_col: 0,
            shift_active: false,
            actions: Vec::new(),
        }
    }

    /// Shows the keyboard and resets focus to the first key of row 1.
    pub fn show(&mut self) {
        self.visible = true;
        self.focused_row = 1;
        self.focused_col = 0;
        self.shift_active = false;
    }

    /// Hides the keyboard.
    pub fn hide(&mut self) {
        self.visible = false;
    }

    /// Returns whether the keyboard is currently visible.
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Drains the pending action queue.
    ///
    /// # Returns
    ///
    /// A vector of actions produced since the last call.
    pub fn take_actions(&mut self) -> Vec<OnScreenKeyboardAction> {
        std::mem::take(&mut self.actions)
    }

    /// Returns the key rows for the current shift state.
    fn rows(&self) -> &'static [&'static [&'static str]] {
        if self.shift_active { ROWS_SHIFT } else { ROWS }
    }

    /// Returns the label of the currently focused key.
    fn focused_label(&self) -> &'static str {
        let rows = self.rows();
        let row = rows[self.focused_row];
        row[self.focused_col.min(row.len() - 1)]
    }

    /// Clamps `focused_col` to fit the current row.
    fn clamp_col(&mut self) {
        let row_len = self.rows()[self.focused_row].len();
        if self.focused_col >= row_len {
            self.focused_col = row_len - 1;
        }
    }

    /// Returns the pixel origin (x, y) for a key at (row, col).
    fn key_origin(&self, row: usize, col: usize) -> (i32, i32) {
        let row_keys = self.rows()[row];
        // Center each row within the panel width.
        let row_w = row_keys.len() as i32 * (KEY_W as i32 + KEY_GAP as i32) - KEY_GAP as i32;
        let row_x_offset = (self.bounds.width as i32 - row_w) / 2;
        let kx = self.bounds.x + row_x_offset + col as i32 * (KEY_W as i32 + KEY_GAP as i32);
        let ky = self.bounds.y + PAD_Y + row as i32 * (KEY_H as i32 + KEY_GAP as i32);
        (kx, ky)
    }
}

impl Widget for OnScreenKeyboard {
    fn bounds(&self) -> &Bounds {
        &self.bounds
    }

    fn set_position(&mut self, x: i32, y: i32) {
        self.bounds.x = x;
        self.bounds.y = y;
    }

    fn handle_event(&mut self, event: &UiEvent) -> EventResponse {
        if !self.visible {
            return EventResponse::Ignored;
        }

        match event {
            // D-pad / stick navigation.
            UiEvent::NavNext => {
                // Move right.
                let row_len = self.rows()[self.focused_row].len();
                self.focused_col = (self.focused_col + 1) % row_len;
                EventResponse::Consumed
            }
            UiEvent::NavPrev => {
                // Move left.
                let row_len = self.rows()[self.focused_row].len();
                self.focused_col = if self.focused_col == 0 {
                    row_len - 1
                } else {
                    self.focused_col - 1
                };
                EventResponse::Consumed
            }
            // Up/down via raw controller D-pad is not handled by NavNext/NavPrev
            // (those are horizontal). We handle vertical movement via
            // ControllerDPad events passed from the scene.
            UiEvent::KeyboardRowUp => {
                if self.focused_row > 0 {
                    self.focused_row -= 1;
                    self.clamp_col();
                }
                EventResponse::Consumed
            }
            UiEvent::KeyboardRowDown => {
                if self.focused_row + 1 < self.rows().len() {
                    self.focused_row += 1;
                    self.clamp_col();
                }
                EventResponse::Consumed
            }
            // A button → type the focused key.
            UiEvent::NavConfirm => {
                let label = self.focused_label();
                if label == "SPACE" {
                    self.actions.push(OnScreenKeyboardAction::TypeChar(' '));
                } else if label == "ENTER" {
                    self.actions.push(OnScreenKeyboardAction::Submit);
                } else {
                    for ch in label.chars() {
                        self.actions.push(OnScreenKeyboardAction::TypeChar(ch));
                    }
                }
                // Auto-release shift after typing one character.
                self.shift_active = false;
                EventResponse::Consumed
            }
            // B button → backspace.
            UiEvent::NavBack => {
                self.actions.push(OnScreenKeyboardAction::Backspace);
                EventResponse::Consumed
            }
            // X button → toggle shift.
            UiEvent::KeyboardToggleShift => {
                self.shift_active = !self.shift_active;
                self.clamp_col();
                EventResponse::Consumed
            }
            // Start button → dismiss.
            UiEvent::KeyboardDismiss => {
                self.actions.push(OnScreenKeyboardAction::Dismiss);
                self.visible = false;
                EventResponse::Consumed
            }
            // Consume mouse events over the keyboard to block pass-through.
            UiEvent::MouseClick { x, y, .. } | UiEvent::MouseMove { x, y } => {
                if self.bounds.contains_point(*x, *y) {
                    EventResponse::Consumed
                } else {
                    EventResponse::Ignored
                }
            }
            _ => EventResponse::Ignored,
        }
    }

    fn update(&mut self, _dt: Duration) {}

    fn render(&mut self, ctx: &mut RenderContext<'_, '_>) -> Result<(), String> {
        if !self.visible {
            return Ok(());
        }

        let rect = sdl2::rect::Rect::new(
            self.bounds.x,
            self.bounds.y,
            self.bounds.width,
            self.bounds.height,
        );

        // Background.
        ctx.canvas.set_blend_mode(BlendMode::Blend);
        ctx.canvas.set_draw_color(Color::RGBA(10, 10, 30, 220));
        ctx.canvas.fill_rect(rect)?;
        ctx.canvas.set_draw_color(Color::RGBA(120, 120, 140, 200));
        ctx.canvas.draw_rect(rect)?;

        let rows = self.rows();

        for (ri, row) in rows.iter().enumerate() {
            for (ci, label) in row.iter().enumerate() {
                let (kx, ky) = self.key_origin(ri, ci);
                let key_w = if *label == "SPACE" {
                    // Space bar spans the remaining width.
                    let row_w = row.len() as u32 * (KEY_W + KEY_GAP) - KEY_GAP;
                    row_w.max(KEY_W * 5)
                } else {
                    KEY_W
                };
                let key_rect = sdl2::rect::Rect::new(kx, ky, key_w, KEY_H);

                let is_focused = ri == self.focused_row && ci == self.focused_col;

                // Key background.
                if is_focused {
                    ctx.canvas.set_draw_color(Color::RGBA(80, 80, 120, 220));
                } else {
                    ctx.canvas.set_draw_color(Color::RGBA(30, 30, 50, 200));
                }
                ctx.canvas.fill_rect(key_rect)?;

                // Key border.
                if is_focused {
                    ctx.canvas.set_draw_color(Color::RGBA(255, 220, 100, 255));
                } else {
                    ctx.canvas.set_draw_color(Color::RGBA(80, 80, 100, 180));
                }
                ctx.canvas.draw_rect(key_rect)?;

                // Key label — centered in the cell.
                let display = if *label == "SPACE" { "___" } else { label };
                let tw = font_cache::text_width(display) as i32;
                let tx = kx + (key_w as i32 - tw) / 2;
                let ty = ky + (KEY_H as i32 - font_cache::BITMAP_GLYPH_H as i32) / 2;
                font_cache::draw_text(
                    ctx.canvas,
                    ctx.gfx,
                    FONT,
                    display,
                    tx,
                    ty,
                    font_cache::TextStyle::PLAIN,
                )?;
            }
        }

        // Hint bar.
        let hint_y = self.bounds.y + self.bounds.height as i32 - HINT_H - 2;
        let shift_label = if self.shift_active { "SHIFT" } else { "shift" };
        let hint = format!("A=type  B=bksp  X={}  Start=done", shift_label);
        let hint_x =
            self.bounds.x + (self.bounds.width as i32 - font_cache::text_width(&hint) as i32) / 2;
        font_cache::draw_text(
            ctx.canvas,
            ctx.gfx,
            FONT,
            &hint,
            hint_x,
            hint_y,
            font_cache::TextStyle::PLAIN,
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

    #[test]
    fn starts_hidden() {
        let kb = OnScreenKeyboard::new();
        assert!(!kb.is_visible());
    }

    #[test]
    fn show_makes_visible() {
        let mut kb = OnScreenKeyboard::new();
        kb.show();
        assert!(kb.is_visible());
    }

    #[test]
    fn hide_makes_invisible() {
        let mut kb = OnScreenKeyboard::new();
        kb.show();
        kb.hide();
        assert!(!kb.is_visible());
    }

    #[test]
    fn nav_confirm_types_focused_key() {
        let mut kb = OnScreenKeyboard::new();
        kb.show();
        // Default focus is row 1, col 0 → "q".
        kb.handle_event(&UiEvent::NavConfirm);
        let actions = kb.take_actions();
        assert_eq!(actions, vec![OnScreenKeyboardAction::TypeChar('q')]);
    }

    #[test]
    fn nav_back_emits_backspace() {
        let mut kb = OnScreenKeyboard::new();
        kb.show();
        kb.handle_event(&UiEvent::NavBack);
        let actions = kb.take_actions();
        assert_eq!(actions, vec![OnScreenKeyboardAction::Backspace]);
    }

    #[test]
    fn nav_next_wraps_around_row() {
        let mut kb = OnScreenKeyboard::new();
        kb.show();
        // Row 1 has 10 keys (q..p). Press NavNext 10 times to wrap.
        for _ in 0..10 {
            kb.handle_event(&UiEvent::NavNext);
        }
        // Should be back at col 0.
        kb.handle_event(&UiEvent::NavConfirm);
        let actions = kb.take_actions();
        assert_eq!(actions, vec![OnScreenKeyboardAction::TypeChar('q')]);
    }

    #[test]
    fn space_key_emits_space_char() {
        let mut kb = OnScreenKeyboard::new();
        kb.show();
        // Navigate to the space bar row (row 4).
        kb.handle_event(&UiEvent::KeyboardRowDown);
        kb.handle_event(&UiEvent::KeyboardRowDown);
        kb.handle_event(&UiEvent::KeyboardRowDown);
        kb.handle_event(&UiEvent::NavConfirm);
        let actions = kb.take_actions();
        assert_eq!(actions, vec![OnScreenKeyboardAction::TypeChar(' ')]);
    }

    #[test]
    fn shift_produces_uppercase() {
        let mut kb = OnScreenKeyboard::new();
        kb.show();
        kb.shift_active = true;
        // Row 1, col 0 → "Q" in shift mode.
        kb.handle_event(&UiEvent::NavConfirm);
        let actions = kb.take_actions();
        assert_eq!(actions, vec![OnScreenKeyboardAction::TypeChar('Q')]);
        // Shift auto-released.
        assert!(!kb.shift_active);
    }

    #[test]
    fn take_actions_drains() {
        let mut kb = OnScreenKeyboard::new();
        kb.show();
        kb.handle_event(&UiEvent::NavConfirm);
        let first = kb.take_actions();
        assert!(!first.is_empty());
        let second = kb.take_actions();
        assert!(second.is_empty());
    }

    #[test]
    fn hidden_ignores_events() {
        let mut kb = OnScreenKeyboard::new();
        let resp = kb.handle_event(&UiEvent::NavConfirm);
        assert_eq!(resp, EventResponse::Ignored);
    }
}
