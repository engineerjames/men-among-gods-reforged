//! Single-line text input widget with optional password masking.

use std::time::Duration;

use sdl2::keyboard::Keycode;
use sdl2::pixels::Color;
use sdl2::render::BlendMode;

use crate::font_cache;
use crate::ui::RenderContext;
use crate::ui::widget::{Bounds, EventResponse, MouseButton, UiEvent, Widget};

/// Horizontal padding (pixels) between the border and the text area on each side.
const INNER_PAD: i32 = 3;

/// Cursor blink interval in seconds — each half-period the cursor toggles.
const BLINK_INTERVAL: f32 = 0.5;

/// Character used to mask password input.
const PASSWORD_CHAR: char = '*';

/// A single-line text input field.
///
/// Renders a filled background box with a colored border.  The border changes
/// color to `border_color_focused` while the widget has keyboard focus.
/// Typing appends characters up to `max_len`; `Backspace` removes the last
/// character.  In password mode every character is rendered as `*` so the
/// actual value is never shown.
///
/// The caller reads the current value via [`TextInput::value`] — no
/// [`WidgetAction`](crate::ui::widget::WidgetAction) is emitted for Enter or
/// any other key.
pub struct TextInput {
    bounds: Bounds,
    /// Hint text shown at reduced opacity when the field is empty.
    placeholder: String,
    /// Bitmap font index (0–3).
    font: usize,
    /// Current text content.
    value: String,
    /// Maximum number of characters that can be entered.
    max_len: usize,
    /// Whether the widget currently has keyboard focus.
    focused: bool,
    /// Whether the cursor is inside the widget bounds.
    hovered: bool,
    /// When `true`, every character is rendered as [`PASSWORD_CHAR`].
    password_mode: bool,
    /// Border color used when the widget is not focused.
    border_color_normal: Color,
    /// Border color used when the widget is focused.
    border_color_focused: Color,
    /// Border thickness in pixels (≥ 1).
    border_width: u32,
    /// How many characters have scrolled off the left edge of the visible area.
    view_offset: usize,
    /// Accumulated time since the last cursor blink toggle (seconds).
    blink_elapsed: f32,
    /// Whether the cursor bar is currently in the visible (drawn) phase.
    cursor_visible: bool,
    /// One-shot flag: set when the value changes; cleared by [`was_changed`](Self::was_changed).
    changed: bool,
    /// Additive tint alpha applied over the widget on hover (0–255).
    hover_alpha: u8,
}

impl TextInput {
    /// Creates a new text input field.
    ///
    /// # Arguments
    ///
    /// * `bounds` - Position and size of the widget.
    /// * `placeholder` - Hint text displayed at reduced opacity when the field is empty.
    /// * `font` - Bitmap font index (0–3).
    /// * `max_len` - Maximum number of characters the user may enter.
    /// * `password_mode` - When `true`, all rendered characters are replaced by `*`.
    /// * `border_color_normal` - Border color in the unfocused state.
    /// * `border_color_focused` - Border color when the widget has keyboard focus.
    ///
    /// # Returns
    ///
    /// A new `TextInput` with an empty value and no focus.
    pub fn new(
        bounds: Bounds,
        placeholder: &str,
        font: usize,
        max_len: usize,
        password_mode: bool,
        border_color_normal: Color,
        border_color_focused: Color,
    ) -> Self {
        Self {
            bounds,
            placeholder: placeholder.to_owned(),
            font,
            value: String::new(),
            max_len,
            focused: false,
            hovered: false,
            password_mode,
            border_color_normal,
            border_color_focused,
            border_width: 1,
            view_offset: 0,
            blink_elapsed: 0.0,
            cursor_visible: true,
            changed: false,
            hover_alpha: 32,
        }
    }

    /// Returns a reference to the current text content.
    pub fn value(&self) -> &str {
        &self.value
    }

    /// Overwrites the current value without triggering the `changed` flag.
    ///
    /// Use this for external sync (e.g. loading saved preferences).  The value
    /// is truncated to `max_len` if necessary.
    ///
    /// # Arguments
    ///
    /// * `v` - The new string value.
    pub fn set_value(&mut self, v: &str) {
        self.value = v.chars().take(self.max_len).collect();
        self.recalculate_view_offset();
    }

    /// Returns `true` once if the value changed since the last call, then
    /// clears the flag.
    pub fn was_changed(&mut self) -> bool {
        let c = self.changed;
        self.changed = false;
        c
    }

    /// Programmatically sets keyboard focus.
    ///
    /// # Arguments
    ///
    /// * `focused` - `true` to give focus, `false` to remove it.
    pub fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
        if focused {
            // Reset blink so cursor is immediately visible on focus.
            self.blink_elapsed = 0.0;
            self.cursor_visible = true;
        }
    }

    /// Clears the current value and resets the scroll offset.
    ///
    /// Does not trigger the `changed` flag.
    pub fn clear(&mut self) {
        self.value.clear();
        self.view_offset = 0;
        self.blink_elapsed = 0.0;
        self.cursor_visible = true;
    }

    /// Sets the hover highlight state (used by controller focus navigation).
    ///
    /// # Arguments
    ///
    /// * `hovered` - `true` to activate the highlight, `false` to clear it.
    pub fn set_hovered(&mut self, hovered: bool) {
        self.hovered = hovered;
    }

    /// Inserts a character as if the user typed it.
    ///
    /// Respects `max_len`. Does nothing if the field is already at capacity.
    ///
    /// # Arguments
    ///
    /// * `ch` - The character to insert.
    pub fn inject_char(&mut self, ch: char) {
        if self.value.len() < self.max_len {
            self.value.push(ch);
            self.recalculate_view_offset();
            self.changed = true;
        }
    }

    /// Removes the last character as if the user pressed Backspace.
    pub fn inject_backspace(&mut self) {
        if self.value.pop().is_some() {
            self.recalculate_view_offset();
            self.changed = true;
        }
    }

    /// Returns whether this field currently has keyboard focus.
    pub fn is_focused(&self) -> bool {
        self.focused
    }

    // -----------------------------------------------------------------------
    // Internal helpers
    // -----------------------------------------------------------------------

    /// Returns the number of characters that fit inside the text area.
    fn visible_char_count(&self) -> usize {
        let inner_w = self.bounds.width as i32 - 2 * (self.border_width as i32 + INNER_PAD);
        (inner_w.max(0) as u32 / font_cache::BITMAP_GLYPH_ADVANCE) as usize
    }

    /// Adjusts `view_offset` so that the cursor (always at the end of the
    /// value) remains inside the visible text area.
    fn recalculate_view_offset(&mut self) {
        let visible = self.visible_char_count();
        let len = self.value.len();
        self.view_offset = if len <= visible { 0 } else { len - visible };
    }

    /// Returns the display string for the current view window.
    ///
    /// In password mode every character is replaced by [`PASSWORD_CHAR`].
    fn display_slice(&self) -> String {
        let visible = self.visible_char_count();
        let chars: Vec<char> = self
            .value
            .chars()
            .skip(self.view_offset)
            .take(visible)
            .collect();
        if self.password_mode {
            PASSWORD_CHAR.to_string().repeat(chars.len())
        } else {
            chars.into_iter().collect()
        }
    }
}

impl Widget for TextInput {
    fn bounds(&self) -> &Bounds {
        &self.bounds
    }

    fn set_position(&mut self, x: i32, y: i32) {
        self.bounds.x = x;
        self.bounds.y = y;
    }

    fn handle_event(&mut self, event: &UiEvent) -> EventResponse {
        match event {
            // ── Hover tracking ─────────────────────────────────────────────
            UiEvent::MouseMove { x, y } => {
                self.hovered = self.bounds.contains_point(*x, *y);
                EventResponse::Ignored
            }

            // ── Click: gain/lose focus ──────────────────────────────────────
            UiEvent::MouseClick {
                x,
                y,
                button: MouseButton::Left,
                ..
            } => {
                if self.bounds.contains_point(*x, *y) {
                    if !self.focused {
                        self.focused = true;
                        self.blink_elapsed = 0.0;
                        self.cursor_visible = true;
                    }
                    EventResponse::Consumed
                } else {
                    self.focused = false;
                    // Return Ignored so the click can propagate to widgets behind.
                    EventResponse::Ignored
                }
            }

            // ── Character input ─────────────────────────────────────────────
            UiEvent::TextInput { text } => {
                if !self.focused {
                    return EventResponse::Ignored;
                }
                let remaining = self.max_len.saturating_sub(self.value.len());
                if remaining > 0 {
                    let added: String = text.chars().take(remaining).collect();
                    self.value.push_str(&added);
                    self.recalculate_view_offset();
                    self.changed = true;
                }
                EventResponse::Consumed
            }

            // ── Key presses ─────────────────────────────────────────────────
            UiEvent::KeyDown { keycode, .. } => {
                if !self.focused {
                    return EventResponse::Ignored;
                }
                match keycode {
                    &Keycode::Backspace => {
                        if self.value.pop().is_some() {
                            self.recalculate_view_offset();
                            self.changed = true;
                        }
                        EventResponse::Consumed
                    }
                    _ => EventResponse::Ignored,
                }
            }

            _ => EventResponse::Ignored,
        }
    }

    fn update(&mut self, dt: Duration) {
        if !self.focused {
            // Keep cursor in visible state while unfocused so it appears
            // immediately when focus is gained.
            self.cursor_visible = true;
            self.blink_elapsed = 0.0;
            return;
        }
        self.blink_elapsed += dt.as_secs_f32();
        if self.blink_elapsed >= BLINK_INTERVAL {
            self.blink_elapsed -= BLINK_INTERVAL;
            self.cursor_visible = !self.cursor_visible;
        }
    }

    fn render(&mut self, ctx: &mut RenderContext<'_, '_>) -> Result<(), String> {
        let bw = self.border_width as i32;

        // ── 1. Background fill ─────────────────────────────────────────────
        let bg_rect = sdl2::rect::Rect::new(
            self.bounds.x,
            self.bounds.y,
            self.bounds.width,
            self.bounds.height,
        );
        ctx.canvas.set_blend_mode(BlendMode::Blend);
        ctx.canvas.set_draw_color(Color::RGBA(30, 30, 40, 220));
        ctx.canvas.fill_rect(bg_rect)?;

        // ── 2. Border ──────────────────────────────────────────────────────
        let border_color = if self.focused {
            self.border_color_focused
        } else {
            self.border_color_normal
        };
        ctx.canvas.set_draw_color(border_color);
        for i in 0..bw {
            let border_rect = sdl2::rect::Rect::new(
                self.bounds.x + i,
                self.bounds.y + i,
                (self.bounds.width as i32 - 2 * i).max(0) as u32,
                (self.bounds.height as i32 - 2 * i).max(0) as u32,
            );
            ctx.canvas.draw_rect(border_rect)?;
        }

        // ── 3. Text / placeholder ──────────────────────────────────────────
        let text_x = self.bounds.x + bw + INNER_PAD;
        let text_y =
            self.bounds.y + (self.bounds.height as i32 - font_cache::BITMAP_GLYPH_H as i32) / 2;

        if self.value.is_empty() {
            font_cache::draw_text(
                ctx.canvas,
                ctx.gfx,
                self.font,
                &self.placeholder,
                text_x,
                text_y,
                font_cache::TextStyle::faded(90),
            )?;
        } else {
            let display = self.display_slice();
            font_cache::draw_text(
                ctx.canvas,
                ctx.gfx,
                self.font,
                &display,
                text_x,
                text_y,
                font_cache::TextStyle::PLAIN,
            )?;
        }

        // ── 4. Cursor bar ──────────────────────────────────────────────────
        if self.focused && self.cursor_visible {
            let visible_count = self.value.len() - self.view_offset;
            let cursor_x =
                text_x + (visible_count as i32) * font_cache::BITMAP_GLYPH_ADVANCE as i32;
            let cursor_h = font_cache::BITMAP_GLYPH_H as i32 + 2;
            let cursor_y = self.bounds.y + (self.bounds.height as i32 - cursor_h) / 2;
            let cursor_rect = sdl2::rect::Rect::new(cursor_x, cursor_y, 1, cursor_h as u32);
            ctx.canvas.set_draw_color(Color::RGBA(220, 220, 240, 220));
            ctx.canvas.fill_rect(cursor_rect)?;
        }

        // ── 5. Hover highlight ─────────────────────────────────────────────
        if self.hovered {
            ctx.canvas.set_blend_mode(BlendMode::Add);
            ctx.canvas
                .set_draw_color(Color::RGBA(255, 255, 255, self.hover_alpha));
            ctx.canvas.fill_rect(bg_rect)?;
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

    fn make_input(max_len: usize, password_mode: bool) -> TextInput {
        TextInput::new(
            Bounds::new(0, 0, 200, 20),
            "hint",
            0,
            max_len,
            password_mode,
            Color::RGBA(100, 100, 120, 200),
            Color::RGBA(100, 180, 255, 255),
        )
    }

    fn no_mods() -> KeyModifiers {
        KeyModifiers {
            ctrl: false,
            shift: false,
            alt: false,
        }
    }

    fn text_event(s: &str) -> UiEvent {
        UiEvent::TextInput { text: s.to_owned() }
    }

    fn key_event(kc: Keycode) -> UiEvent {
        UiEvent::KeyDown {
            keycode: kc,
            modifiers: no_mods(),
        }
    }

    fn click_inside() -> UiEvent {
        UiEvent::MouseClick {
            x: 10,
            y: 10,
            button: MouseButton::Left,
            modifiers: no_mods(),
        }
    }

    fn click_outside() -> UiEvent {
        UiEvent::MouseClick {
            x: 500,
            y: 500,
            button: MouseButton::Left,
            modifiers: no_mods(),
        }
    }

    #[test]
    fn new_creates_empty_value() {
        let t = make_input(32, false);
        assert_eq!(t.value(), "");
    }

    #[test]
    fn focus_gained_on_click_inside() {
        let mut t = make_input(32, false);
        assert!(!t.focused);
        t.handle_event(&click_inside());
        assert!(t.focused);
    }

    #[test]
    fn focus_lost_on_click_outside() {
        let mut t = make_input(32, false);
        t.handle_event(&click_inside());
        assert!(t.focused);
        t.handle_event(&click_outside());
        assert!(!t.focused);
    }

    #[test]
    fn text_input_ignored_when_not_focused() {
        let mut t = make_input(32, false);
        assert_eq!(t.handle_event(&text_event("hello")), EventResponse::Ignored);
        assert_eq!(t.value(), "");
    }

    #[test]
    fn text_input_appends_when_focused() {
        let mut t = make_input(32, false);
        t.handle_event(&click_inside());
        t.handle_event(&text_event("hi"));
        assert_eq!(t.value(), "hi");
    }

    #[test]
    fn max_len_enforced() {
        let mut t = make_input(3, false);
        t.handle_event(&click_inside());
        t.handle_event(&text_event("abcde"));
        assert_eq!(t.value(), "abc"); // truncated to max_len
    }

    #[test]
    fn max_len_blocks_further_input() {
        let mut t = make_input(2, false);
        t.handle_event(&click_inside());
        t.handle_event(&text_event("ab"));
        let resp = t.handle_event(&text_event("c"));
        // Consumed because widget is focused, but value doesn't grow.
        assert_eq!(t.value(), "ab");
        assert_eq!(resp, EventResponse::Consumed);
    }

    #[test]
    fn was_changed_clears_flag() {
        let mut t = make_input(32, false);
        t.handle_event(&click_inside());
        t.handle_event(&text_event("x"));
        assert!(t.was_changed());
        assert!(!t.was_changed()); // cleared after first read
    }

    #[test]
    fn backspace_removes_last_char() {
        let mut t = make_input(32, false);
        t.handle_event(&click_inside());
        t.handle_event(&text_event("ab"));
        t.handle_event(&key_event(Keycode::Backspace));
        assert_eq!(t.value(), "a");
    }

    #[test]
    fn backspace_on_empty_does_not_panic() {
        let mut t = make_input(32, false);
        t.handle_event(&click_inside());
        t.handle_event(&key_event(Keycode::Backspace));
        assert_eq!(t.value(), "");
    }

    #[test]
    fn password_mask_length_matches_value() {
        let mut t = make_input(32, true);
        t.handle_event(&click_inside());
        t.handle_event(&text_event("secret"));
        // display_slice should contain 6 asterisks (value fits without scrolling)
        assert_eq!(t.display_slice(), "******");
        assert_eq!(t.display_slice().len(), t.value().len());
    }

    #[test]
    fn set_value_silent() {
        let mut t = make_input(32, false);
        t.set_value("preset");
        assert_eq!(t.value(), "preset");
        assert!(!t.was_changed());
    }

    #[test]
    fn set_value_truncates_to_max_len() {
        let mut t = make_input(3, false);
        t.set_value("toolong");
        assert_eq!(t.value(), "too");
    }

    #[test]
    fn clear_empties_value() {
        let mut t = make_input(32, false);
        t.set_value("hello");
        t.clear();
        assert_eq!(t.value(), "");
        assert_eq!(t.view_offset, 0);
    }

    #[test]
    fn view_offset_advances_when_text_overflows() {
        // Bounds width 200, border 1, inner pad 3 each side --> inner = 192px.
        // visible_chars = 192 / 6 = 32.
        // Typing 33 chars should push view_offset to 1.
        let mut t = make_input(100, false);
        t.handle_event(&click_inside());
        let long: String = "a".repeat(33);
        t.handle_event(&text_event(&long));
        assert_eq!(t.view_offset, 1);
    }

    #[test]
    fn view_offset_resets_after_backspace() {
        let mut t = make_input(100, false);
        t.handle_event(&click_inside());
        let long: String = "a".repeat(33);
        t.handle_event(&text_event(&long));
        assert_eq!(t.view_offset, 1);
        t.handle_event(&key_event(Keycode::Backspace));
        assert_eq!(t.view_offset, 0); // back to 32 chars, fits in view
    }
}
