//! Scrollable chat log with input line — the first concrete widget built on
//! the UI framework.
//!
//! `ChatBox` owns the message log, input buffer, and sent-message history.  It
//! renders a semi-transparent background over the world view and handles
//! scroll, typing, and history-navigation events internally.

use std::time::Duration;

use sdl2::keyboard::Keycode;
use sdl2::pixels::Color;
use sdl2::render::BlendMode;

use crate::font_cache;
use crate::types::log_message::{LogMessage, LogMessageColor};

use crate::ui::RenderContext;
use crate::ui::style::Padding;
use crate::ui::widget::{Bounds, EventResponse, UiEvent, Widget, WidgetAction};

/// Maximum characters allowed in the chat input buffer.
const MAX_INPUT_LEN: usize = 120;

/// Maximum number of previously sent messages kept in history.
const MAX_HISTORY_LEN: usize = 100;

/// Command names that Tab history should treat as reusable chat prefixes.
const CHAT_PREFIX_COMMANDS: &[&str] = &["shout", "gtell", "itell", "stell", "tell", "me", "emote"];

/// Height reserved for the input area (separator gap + one line of text).
const INPUT_AREA_H: u32 = font_cache::BITMAP_GLYPH_H + 4; // 2px gap above + 2px below

/// Default bitmap font index for the input line (yellow).
const INPUT_FONT: usize = 1;

/// Color of the thin separator line between log and input.
const SEPARATOR_COLOR: Color = Color::RGBA(120, 120, 140, 200);

/// Seconds of inactivity before the fade-out animation begins.
const IDLE_FADE_DELAY_SECS: f32 = 5.0;

/// Duration in seconds of the fade-out transition (fully opaque --> invisible).
const IDLE_FADE_DURATION_SECS: f32 = 1.0;

/// Duration in seconds for one complete caret blink cycle.
const CARET_BLINK_PERIOD_SECS: f32 = 1.0;

/// A self-contained scrollable chat log with an input line.
///
/// Draws a semi-transparent background, then the most recent messages
/// (bottom-aligned, newest at the bottom) and the current input buffer below a
/// thin separator.
pub struct ChatBox {
    bounds: Bounds,
    bg_color: Color,
    padding: Padding,

    // -- Scroll state --
    scroll_offset: usize,
    last_message_count: usize,

    // -- Computed layout (derived from bounds + padding) --
    visible_lines: usize,
    line_height: u32,

    // -- Data owned by the widget --
    messages: Vec<LogMessage>,
    input_buf: String,
    input_cursor: usize,
    sent_chat_history: Vec<String>,
    chat_history_index: Option<usize>,
    chat_history_draft: Option<String>,
    chat_prefix_history_index: Option<usize>,

    // -- Focus & actions --
    focused: bool,
    pending_actions: Vec<WidgetAction>,

    // -- Idle fade --
    /// Seconds elapsed since the last user interaction or incoming message.
    idle_elapsed: f32,
    /// Seconds elapsed in the current caret blink cycle.
    caret_elapsed: f32,
    /// Current draw opacity in the range 0 (invisible) – 255 (fully opaque).
    alpha: u8,
}

impl ChatBox {
    /// Creates a new chat box.
    ///
    /// # Arguments
    ///
    /// * `bounds` - Position and size of the widget.
    /// * `bg_color` - Background fill color (use `Color::RGBA` for
    ///   semi-transparency).
    /// * `padding` - Inner spacing between the box edge and text content.
    ///
    /// # Returns
    ///
    /// A new `ChatBox` with focus disabled. Focus is gained when the user
    /// presses `/`, Enter, or Numpad-Enter.
    pub fn new(bounds: Bounds, bg_color: Color, padding: Padding) -> Self {
        let line_height = font_cache::BITMAP_GLYPH_H;
        let visible_lines = Self::compute_visible_lines(&bounds, &padding, line_height);
        Self {
            bounds,
            bg_color,
            padding,
            scroll_offset: 0,
            last_message_count: 0,
            visible_lines,
            line_height,
            messages: Vec::new(),
            input_buf: String::new(),
            input_cursor: 0,
            sent_chat_history: Vec::new(),
            chat_history_index: None,
            chat_history_draft: None,
            chat_prefix_history_index: None,
            focused: false,
            pending_actions: Vec::new(),
            idle_elapsed: 0.0,
            caret_elapsed: 0.0,
            alpha: 255,
        }
    }

    /// Appends a single message to the log.
    ///
    /// # Arguments
    ///
    /// * `message` - The log message to add.
    pub fn push_message(&mut self, message: LogMessage) {
        self.idle_elapsed = 0.0;
        self.messages.push(message);
    }

    /// Appends multiple messages to the log.
    ///
    /// # Arguments
    ///
    /// * `messages` - An iterator of log messages to add.
    pub fn push_messages(&mut self, messages: impl Iterator<Item = LogMessage>) {
        self.idle_elapsed = 0.0;
        self.messages.extend(messages);
    }

    /// Returns the total number of stored messages.
    ///
    /// # Returns
    ///
    /// * Value returned by `message_count`.
    pub fn message_count(&self) -> usize {
        self.messages.len()
    }

    /// Returns a read-only view of the current input text.
    ///
    /// # Returns
    ///
    /// * Value returned by `input_text`.
    pub fn input_text(&self) -> &str {
        &self.input_buf
    }

    /// Returns `true` if the chat input has keyboard focus.
    ///
    /// # Returns
    ///
    /// * `true` when `is_focused` succeeds or the condition is met, otherwise `false`.
    pub fn is_focused(&self) -> bool {
        self.focused
    }

    /// Programmatically sets the focus state of the chat input.
    ///
    /// # Arguments
    ///
    /// * `focused` - Whether the chat input should have focus.
    pub fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
        if focused {
            self.idle_elapsed = 0.0;
            self.reset_caret_blink();
        }
    }

    /// Injects a single character into the input buffer (for the on-screen
    /// keyboard).
    ///
    /// # Arguments
    ///
    /// * `ch` - The character to inject.
    pub fn inject_char(&mut self, ch: char) {
        self.idle_elapsed = 0.0;
        if self.input_buf.len() + ch.len_utf8() <= MAX_INPUT_LEN {
            self.input_buf.insert(self.input_cursor, ch);
            self.input_cursor += ch.len_utf8();
            self.reset_caret_blink();
        }
    }

    /// Removes the last character from the input buffer (for the on-screen
    /// keyboard backspace).
    pub fn inject_backspace(&mut self) {
        self.idle_elapsed = 0.0;
        self.delete_before_cursor();
    }

    /// Scrolls the message log by one page.
    ///
    /// `scroll_offset` is measured from the newest message (0 = newest at the
    /// bottom), so scrolling up toward older history increases it. Clamping to
    /// the valid range happens in [`Self::sync_scroll`] before rendering.
    ///
    /// # Arguments
    ///
    /// * `up` - When `true`, scroll toward older messages; otherwise toward newer.
    fn page_scroll(&mut self, up: bool) {
        let step = self.visible_lines.saturating_sub(1).max(1);
        if up {
            self.scroll_offset = self.scroll_offset.saturating_add(step);
        } else {
            self.scroll_offset = self.scroll_offset.saturating_sub(step);
        }
        self.idle_elapsed = 0.0;
    }

    /// Runs the follow-tail and clamping logic for the scroll offset.
    ///
    /// Call this once per frame before rendering so that new messages push the
    /// viewport correctly.
    fn sync_scroll(&mut self) {
        let total = self.messages.len();

        // Follow-tail: if new messages arrived while manually scrolled up,
        // shift the offset so the viewport stays on the same messages.
        if total > self.last_message_count && self.scroll_offset > 0 {
            let delta = total - self.last_message_count;
            self.scroll_offset = self.scroll_offset.saturating_add(delta);
        }
        self.last_message_count = total;

        // Clamp to valid range. 0 = newest-at-bottom.
        let max_scroll = total.saturating_sub(self.visible_lines);
        self.scroll_offset = self.scroll_offset.min(max_scroll);
    }

    /// Computes how many full text lines fit in the log area.
    ///
    /// # Arguments
    ///
    /// * `bounds` - Overall widget bounds.
    /// * `padding` - Inner padding.
    /// * `line_height` - Height of one text line in pixels.
    ///
    /// # Returns
    ///
    /// Number of visible lines.
    fn compute_visible_lines(bounds: &Bounds, padding: &Padding, line_height: u32) -> usize {
        let inner = bounds.inner(padding);
        let log_area_h = inner.height.saturating_sub(INPUT_AREA_H);
        (log_area_h / line_height) as usize
    }

    /// Maps a [`LogMessageColor`] to a bitmap font index.
    fn font_for_color(color: LogMessageColor) -> usize {
        match color {
            LogMessageColor::Red => 0,
            LogMessageColor::Yellow => 1,
            LogMessageColor::Green => 2,
            LogMessageColor::Blue => 3,
        }
    }

    /// Returns a message by index-from-most-recent (0 = newest).
    fn message_from_end(&self, index: usize) -> Option<&LogMessage> {
        if index < self.messages.len() {
            Some(&self.messages[self.messages.len() - 1 - index])
        } else {
            None
        }
    }

    /// Resets the caret blink cycle so the caret is immediately visible.
    fn reset_caret_blink(&mut self) {
        self.caret_elapsed = 0.0;
    }

    /// Clamps cursor state to a valid UTF-8 boundary inside the input buffer.
    fn normalize_cursor(&mut self) {
        if self.input_cursor > self.input_buf.len() {
            self.input_cursor = self.input_buf.len();
        }
        while !self.input_buf.is_char_boundary(self.input_cursor) {
            self.input_cursor -= 1;
        }
    }

    /// Moves the input cursor one Unicode scalar value to the left.
    fn move_cursor_left(&mut self) {
        self.normalize_cursor();
        if self.input_cursor == 0 {
            return;
        }
        if let Some((idx, _)) = self.input_buf[..self.input_cursor]
            .char_indices()
            .next_back()
        {
            self.input_cursor = idx;
            self.reset_caret_blink();
        }
    }

    /// Moves the input cursor one Unicode scalar value to the right.
    fn move_cursor_right(&mut self) {
        self.normalize_cursor();
        if self.input_cursor >= self.input_buf.len() {
            return;
        }
        if let Some(ch) = self.input_buf[self.input_cursor..].chars().next() {
            self.input_cursor += ch.len_utf8();
            self.reset_caret_blink();
        }
    }

    /// Deletes the character immediately before the input cursor.
    fn delete_before_cursor(&mut self) {
        self.normalize_cursor();
        if self.input_cursor == 0 {
            return;
        }
        if let Some((start, _)) = self.input_buf[..self.input_cursor]
            .char_indices()
            .next_back()
        {
            self.input_buf.drain(start..self.input_cursor);
            self.input_cursor = start;
            self.reset_caret_blink();
        }
    }

    /// Inserts text at the cursor while respecting the maximum input length.
    fn insert_text_at_cursor(&mut self, text: &str) {
        self.normalize_cursor();
        if self.input_buf.len() + text.len() <= MAX_INPUT_LEN {
            self.input_buf.insert_str(self.input_cursor, text);
            self.input_cursor += text.len();
            self.reset_caret_blink();
        }
    }

    /// Replaces the input buffer and moves the cursor to the end.
    fn replace_input(&mut self, text: String) {
        self.input_buf = text;
        self.input_cursor = self.input_buf.len();
        self.reset_caret_blink();
    }

    /// Returns the visible input suffix and cursor position within it.
    fn visible_input_window(&self, max_visible_chars: usize) -> (String, usize) {
        let cursor_char = self.input_buf[..self.input_cursor].chars().count();
        let char_count = self.input_buf.chars().count();
        if char_count <= max_visible_chars {
            return (self.input_buf.clone(), cursor_char);
        }

        let desired_start = cursor_char
            .saturating_add(1)
            .saturating_sub(max_visible_chars);
        let max_start = char_count.saturating_sub(max_visible_chars);
        let start_char = desired_start.min(max_start);
        let visible: String = self
            .input_buf
            .chars()
            .skip(start_char)
            .take(max_visible_chars)
            .collect();

        (visible, cursor_char.saturating_sub(start_char))
    }

    // -- Input / history helpers --

    /// Handles the Enter key: sends the current input and records history.
    pub fn submit_input(&mut self) {
        self.focused = false;
        if self.input_buf.is_empty() {
            self.input_cursor = 0;
            return;
        }
        let text = self.input_buf.clone();
        self.input_buf.clear();
        self.input_cursor = 0;

        self.sent_chat_history.push(text.clone());
        if self.sent_chat_history.len() > MAX_HISTORY_LEN {
            self.sent_chat_history.remove(0);
        }
        self.chat_history_index = None;
        self.chat_history_draft = None;
        self.chat_prefix_history_index = None;

        self.pending_actions.push(WidgetAction::SendChat(text));
    }

    /// Extracts a reusable chat command prefix from a sent message.
    ///
    /// # Arguments
    ///
    /// * `message` - The previously sent chat input to inspect.
    ///
    /// # Returns
    ///
    /// A command prefix ending in a space when the message is a supported chat
    /// command, otherwise `None`.
    fn chat_command_prefix(message: &str) -> Option<String> {
        let trimmed = message.trim_start();
        let sigil = trimmed
            .chars()
            .next()
            .filter(|ch| *ch == '/' || *ch == '#')?;
        let without_sigil = &trimmed[sigil.len_utf8()..];
        let mut parts = without_sigil.split_whitespace();
        let command = parts.next()?;
        let command_lower = command.to_ascii_lowercase();

        if !CHAT_PREFIX_COMMANDS.contains(&command_lower.as_str()) {
            return None;
        }

        if command_lower == "tell"
            && let Some(target) = parts.next()
        {
            return Some(format!("{sigil}{command} {target} "));
        }

        Some(format!("{sigil}{command} "))
    }

    /// Navigates backward through recently used reusable chat command prefixes.
    fn chat_prefix_back(&mut self) {
        let prefixes: Vec<String> = self
            .sent_chat_history
            .iter()
            .filter_map(|message| Self::chat_command_prefix(message))
            .collect();

        if prefixes.is_empty() {
            return;
        }

        let next_index = match self.chat_prefix_history_index {
            Some(0) => prefixes.len() - 1,
            Some(idx) => idx - 1,
            None => prefixes.len() - 1,
        };
        self.chat_prefix_history_index = Some(next_index);
        self.chat_history_index = None;
        self.chat_history_draft = None;
        self.replace_input(prefixes[next_index].clone());
    }

    /// Navigates backward (older) in the sent history.
    fn history_back(&mut self) {
        if self.sent_chat_history.is_empty() {
            return;
        }
        let next_index = match self.chat_history_index {
            Some(idx) => idx.saturating_sub(1),
            None => {
                self.chat_history_draft = Some(self.input_buf.clone());
                self.sent_chat_history.len() - 1
            }
        };
        self.chat_history_index = Some(next_index);
        if let Some(msg) = self.sent_chat_history.get(next_index) {
            self.replace_input(msg.clone());
        }
    }

    /// Navigates forward (newer) in the sent history.
    fn history_forward(&mut self) {
        let Some(idx) = self.chat_history_index else {
            return;
        };
        if idx + 1 < self.sent_chat_history.len() {
            let next = idx + 1;
            self.chat_history_index = Some(next);
            if let Some(msg) = self.sent_chat_history.get(next) {
                self.replace_input(msg.clone());
            }
        } else {
            self.chat_history_index = None;
            let draft = self.chat_history_draft.take().unwrap_or_default();
            self.replace_input(draft);
        }
    }
}

impl Widget for ChatBox {
    fn bounds(&self) -> &Bounds {
        &self.bounds
    }

    fn set_position(&mut self, x: i32, y: i32) {
        self.bounds.x = x;
        self.bounds.y = y;
    }

    fn handle_event(&mut self, event: &UiEvent) -> EventResponse {
        match event {
            UiEvent::MouseWheel { x, y, delta } => {
                if !self.bounds.contains_point(*x, *y) {
                    return EventResponse::Ignored;
                }
                self.idle_elapsed = 0.0;
                if *delta > 0 {
                    self.scroll_offset = self.scroll_offset.saturating_add(*delta as usize);
                } else if *delta < 0 {
                    self.scroll_offset = self.scroll_offset.saturating_sub((-*delta) as usize);
                }
                EventResponse::Consumed
            }

            UiEvent::MouseClick { x: _, y: _, .. } => {
                // Do nothing intentionally
                EventResponse::Ignored
            }

            UiEvent::TextInput { text } => {
                if !self.focused {
                    return EventResponse::Ignored;
                }
                self.idle_elapsed = 0.0;
                self.insert_text_at_cursor(text);
                EventResponse::Consumed
            }

            UiEvent::KeyDown { keycode, .. } => {
                if !self.focused {
                    match *keycode {
                        Keycode::Return | Keycode::KpEnter | Keycode::Slash => {
                            self.idle_elapsed = 0.0;
                            self.focused = true;
                            self.input_cursor = self.input_buf.len();
                            self.reset_caret_blink();
                            return EventResponse::Consumed;
                        }
                        Keycode::Up => {
                            self.idle_elapsed = 0.0;
                            self.focused = true;
                            self.reset_caret_blink();
                            self.history_back();
                            return EventResponse::Consumed;
                        }
                        Keycode::PageUp => {
                            self.page_scroll(true);
                            return EventResponse::Consumed;
                        }
                        Keycode::PageDown => {
                            self.page_scroll(false);
                            return EventResponse::Consumed;
                        }
                        _ => {}
                    }
                    return EventResponse::Ignored;
                }
                self.idle_elapsed = 0.0;
                match *keycode {
                    Keycode::Return | Keycode::KpEnter => {
                        self.submit_input();
                        EventResponse::Consumed
                    }
                    Keycode::Escape => {
                        self.focused = false;
                        EventResponse::Consumed
                    }
                    Keycode::Backspace => {
                        self.delete_before_cursor();
                        EventResponse::Consumed
                    }
                    Keycode::Left => {
                        self.move_cursor_left();
                        EventResponse::Consumed
                    }
                    Keycode::Right => {
                        self.move_cursor_right();
                        EventResponse::Consumed
                    }
                    Keycode::Home => {
                        self.input_cursor = 0;
                        self.reset_caret_blink();
                        EventResponse::Consumed
                    }
                    Keycode::End => {
                        self.input_cursor = self.input_buf.len();
                        self.reset_caret_blink();
                        EventResponse::Consumed
                    }
                    Keycode::PageUp => {
                        self.page_scroll(true);
                        EventResponse::Consumed
                    }
                    Keycode::PageDown => {
                        self.page_scroll(false);
                        EventResponse::Consumed
                    }
                    Keycode::Up => {
                        self.history_back();
                        EventResponse::Consumed
                    }
                    Keycode::Tab => {
                        self.chat_prefix_back();
                        EventResponse::Consumed
                    }
                    Keycode::Down => {
                        self.history_forward();
                        EventResponse::Consumed
                    }
                    _ => EventResponse::Ignored,
                }
            }

            UiEvent::MouseMove { .. }
            | UiEvent::MouseDown { .. }
            | UiEvent::NavNext
            | UiEvent::NavPrev
            | UiEvent::NavConfirm
            | UiEvent::NavBack
            | UiEvent::KeyboardRowUp
            | UiEvent::KeyboardRowDown
            | UiEvent::KeyboardToggleShift
            | UiEvent::KeyboardDismiss => EventResponse::Ignored,
        }
    }

    fn update(&mut self, dt: Duration) {
        self.caret_elapsed = (self.caret_elapsed + dt.as_secs_f32()) % CARET_BLINK_PERIOD_SECS;
        if self.focused {
            self.idle_elapsed = 0.0;
            self.alpha = 255;
            return;
        }

        self.idle_elapsed += dt.as_secs_f32();
        self.alpha = if self.idle_elapsed < IDLE_FADE_DELAY_SECS {
            255
        } else {
            let t = ((self.idle_elapsed - IDLE_FADE_DELAY_SECS) / IDLE_FADE_DURATION_SECS).min(1.0);
            ((1.0 - t) * 255.0) as u8
        };
        // Safety net: once fully faded out, always drop focus so the
        // invisible widget cannot silently capture keyboard input.
        if self.alpha == 0 {
            self.focused = false;
        }
    }

    fn render(&mut self, ctx: &mut RenderContext<'_, '_>) -> Result<(), String> {
        if self.alpha == 0 {
            return Ok(());
        }

        self.sync_scroll();

        let bg_rect = sdl2::rect::Rect::new(
            self.bounds.x,
            self.bounds.y,
            self.bounds.width,
            self.bounds.height,
        );

        // 1. Semi-transparent background (alpha-scaled by idle fade)
        let bg_a = (f32::from(self.bg_color.a) * (f32::from(self.alpha) / 255.0)) as u8;
        let bg_color = Color::RGBA(self.bg_color.r, self.bg_color.g, self.bg_color.b, bg_a);
        ctx.canvas.set_blend_mode(BlendMode::Blend);
        ctx.canvas.set_draw_color(bg_color);
        ctx.canvas.fill_rect(bg_rect)?;

        let inner = self.bounds.inner(&self.padding);

        // 2. Render log lines (top-->bottom, newest at bottom)
        for line in 0..self.visible_lines {
            let idx_from_most_recent = self
                .scroll_offset
                .saturating_add(self.visible_lines.saturating_sub(1).saturating_sub(line));

            if let Some(msg) = self.message_from_end(idx_from_most_recent) {
                let font = Self::font_for_color(msg.color);
                let y = inner.y + (line as i32) * self.line_height as i32;
                font_cache::draw_text(
                    ctx.canvas,
                    ctx.gfx,
                    font,
                    &msg.message,
                    inner.x,
                    y,
                    font_cache::TextStyle::faded(self.alpha),
                )?;
            }
        }

        // 3. Separator line between log and input (alpha-scaled)
        let sep_a = (f32::from(SEPARATOR_COLOR.a) * (f32::from(self.alpha) / 255.0)) as u8;
        let sep_color = Color::RGBA(
            SEPARATOR_COLOR.r,
            SEPARATOR_COLOR.g,
            SEPARATOR_COLOR.b,
            sep_a,
        );
        let sep_y = inner.y + (self.visible_lines as i32) * self.line_height as i32 + 1;
        ctx.canvas.set_draw_color(sep_color);
        ctx.canvas.draw_line(
            sdl2::rect::Point::new(inner.x, sep_y),
            sdl2::rect::Point::new(inner.x + inner.width as i32 - 1, sep_y),
        )?;

        // 4. Input line below separator
        // TODO: This is really inefficient to do this every frame;
        // just cache a "visible input substring" that gets updated on input events.
        const MAX_INPUT_TO_SHOW: usize = 43;
        let input_y = sep_y + 3; // 3px below separator
        let (input_text_to_render, visible_cursor_chars) =
            self.visible_input_window(MAX_INPUT_TO_SHOW);

        font_cache::draw_text(
            ctx.canvas,
            ctx.gfx,
            INPUT_FONT,
            &input_text_to_render,
            inner.x,
            input_y,
            font_cache::TextStyle::faded(self.alpha),
        )?;

        if self.focused && self.caret_elapsed < CARET_BLINK_PERIOD_SECS / 2.0 {
            let caret_x =
                inner.x + (visible_cursor_chars as i32 * font_cache::BITMAP_GLYPH_ADVANCE as i32);
            font_cache::draw_text(
                ctx.canvas,
                ctx.gfx,
                INPUT_FONT,
                "|",
                caret_x,
                input_y,
                font_cache::TextStyle::faded(self.alpha),
            )?;
        }

        Ok(())
    }

    fn take_actions(&mut self) -> Vec<WidgetAction> {
        std::mem::take(&mut self.pending_actions)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper to build a default test ChatBox.
    fn test_chat_box() -> ChatBox {
        ChatBox::new(
            Bounds::new(4, 356, 400, 180),
            Color::RGBA(10, 10, 30, 180),
            Padding::uniform(4),
        )
    }

    fn make_msg(text: &str, color: LogMessageColor) -> LogMessage {
        LogMessage {
            message: text.to_owned(),
            color,
        }
    }

    // -- visible_lines --

    #[test]
    fn visible_lines_computed_correctly() {
        let cb = test_chat_box();
        // Inner height = 180 - 4 - 4 = 172
        // Log area = 172 - INPUT_AREA_H(14) = 158
        // Lines = 158 / 10 = 15
        assert_eq!(cb.visible_lines, 15);
    }

    // -- sync_scroll --

    #[test]
    fn sync_scroll_follow_tail_at_zero() {
        let mut cb = test_chat_box();
        for i in 0..5 {
            cb.push_message(make_msg(&format!("msg {}", i), LogMessageColor::Yellow));
        }
        cb.sync_scroll();
        // Not scrolled, so offset stays 0.
        assert_eq!(cb.scroll_offset, 0);
    }

    #[test]
    fn page_up_and_down_scroll_the_log() {
        let mut cb = test_chat_box();
        // visible_lines == 15, so the page step is 14.
        let step = cb.visible_lines.saturating_sub(1).max(1);
        for i in 0..50 {
            cb.push_message(make_msg(&format!("msg {}", i), LogMessageColor::Yellow));
        }

        // PageUp scrolls toward older messages even when unfocused.
        let resp = cb.handle_event(&UiEvent::KeyDown {
            keycode: Keycode::PageUp,
            modifiers: crate::ui::widget::KeyModifiers::default(),
        });
        assert_eq!(resp, EventResponse::Consumed);
        assert_eq!(cb.scroll_offset, step);

        // PageDown scrolls back toward the newest messages.
        let resp = cb.handle_event(&UiEvent::KeyDown {
            keycode: Keycode::PageDown,
            modifiers: crate::ui::widget::KeyModifiers::default(),
        });
        assert_eq!(resp, EventResponse::Consumed);
        assert_eq!(cb.scroll_offset, 0);
    }

    #[test]
    fn page_up_works_while_focused() {
        let mut cb = test_chat_box();
        cb.focused = true;
        let step = cb.visible_lines.saturating_sub(1).max(1);
        for i in 0..50 {
            cb.push_message(make_msg(&format!("msg {}", i), LogMessageColor::Yellow));
        }
        let resp = cb.handle_event(&UiEvent::KeyDown {
            keycode: Keycode::PageUp,
            modifiers: crate::ui::widget::KeyModifiers::default(),
        });
        assert_eq!(resp, EventResponse::Consumed);
        assert_eq!(cb.scroll_offset, step);
    }

    #[test]
    fn sync_scroll_clamp_to_max() {
        let mut cb = test_chat_box();
        // Push fewer messages than visible_lines
        for i in 0..3 {
            cb.push_message(make_msg(&format!("msg {}", i), LogMessageColor::Yellow));
        }
        cb.scroll_offset = 100;
        cb.sync_scroll();
        // max_scroll = 3 - 15 = 0
        assert_eq!(cb.scroll_offset, 0);
    }

    #[test]
    fn sync_scroll_preserves_viewport_when_scrolled_up() {
        let mut cb = test_chat_box();
        // Fill with many messages
        for i in 0..30 {
            cb.push_message(make_msg(&format!("msg {}", i), LogMessageColor::Yellow));
        }
        cb.last_message_count = 30;
        cb.scroll_offset = 5;

        // Two new messages arrive
        cb.push_message(make_msg("new1", LogMessageColor::Green));
        cb.push_message(make_msg("new2", LogMessageColor::Green));
        cb.sync_scroll();

        // Offset should have grown by 2 to keep the same messages in view
        assert_eq!(cb.scroll_offset, 7);
    }

    // -- push_message --

    #[test]
    fn push_message_increments_count() {
        let mut cb = test_chat_box();
        assert_eq!(cb.message_count(), 0);
        cb.push_message(make_msg("hello", LogMessageColor::Yellow));
        assert_eq!(cb.message_count(), 1);
    }

    // -- message_from_end --

    #[test]
    fn message_from_end_zero_is_newest() {
        let mut cb = test_chat_box();
        cb.push_message(make_msg("first", LogMessageColor::Yellow));
        cb.push_message(make_msg("second", LogMessageColor::Green));
        let msg = cb.message_from_end(0).unwrap();
        assert_eq!(msg.message, "second");
    }

    #[test]
    fn message_from_end_out_of_range() {
        let cb = test_chat_box();
        assert!(cb.message_from_end(0).is_none());
    }

    // -- scroll_via_event --

    #[test]
    fn mouse_wheel_inside_scrolls() {
        let mut cb = test_chat_box();
        for i in 0..30 {
            cb.push_message(make_msg(&format!("msg {}", i), LogMessageColor::Yellow));
        }
        cb.last_message_count = 30;

        let event = UiEvent::MouseWheel {
            x: 50,
            y: 400,
            delta: 3,
        };
        let resp = cb.handle_event(&event);
        assert_eq!(resp, EventResponse::Consumed);
        assert_eq!(cb.scroll_offset, 3);
    }

    #[test]
    fn mouse_wheel_outside_ignored() {
        let mut cb = test_chat_box();
        let event = UiEvent::MouseWheel {
            x: 800,
            y: 100,
            delta: 3,
        };
        let resp = cb.handle_event(&event);
        assert_eq!(resp, EventResponse::Ignored);
        assert_eq!(cb.scroll_offset, 0);
    }

    // -- focus --

    #[test]
    fn click_inside_is_ignored() {
        let mut cb = test_chat_box();
        cb.focused = false;
        let event = UiEvent::MouseClick {
            x: 50,
            y: 400,
            button: crate::ui::widget::MouseButton::Left,
            modifiers: crate::ui::widget::KeyModifiers::default(),
        };
        let resp = cb.handle_event(&event);
        assert_eq!(resp, EventResponse::Ignored);
        assert!(!cb.focused);
    }

    #[test]
    fn click_outside_is_ignored() {
        let mut cb = test_chat_box();
        assert!(!cb.focused);
        let event = UiEvent::MouseClick {
            x: 800,
            y: 100,
            button: crate::ui::widget::MouseButton::Left,
            modifiers: crate::ui::widget::KeyModifiers::default(),
        };
        let resp = cb.handle_event(&event);
        assert_eq!(resp, EventResponse::Ignored);
        assert!(!cb.focused);
    }

    // -- text input --

    #[test]
    fn text_input_appends_when_focused() {
        let mut cb = test_chat_box();
        cb.focused = true;
        let event = UiEvent::TextInput {
            text: "hi".to_owned(),
        };
        let resp = cb.handle_event(&event);
        assert_eq!(resp, EventResponse::Consumed);
        assert_eq!(cb.input_text(), "hi");
        assert_eq!(cb.input_cursor, 2);
    }

    #[test]
    fn text_input_inserts_at_cursor() {
        let mut cb = test_chat_box();
        cb.focused = true;
        cb.input_buf = "helo".to_owned();
        cb.input_cursor = 2;
        let event = UiEvent::TextInput {
            text: "l".to_owned(),
        };
        let resp = cb.handle_event(&event);
        assert_eq!(resp, EventResponse::Consumed);
        assert_eq!(cb.input_text(), "hello");
        assert_eq!(cb.input_cursor, 3);
    }

    #[test]
    fn text_input_ignored_when_unfocused() {
        let mut cb = test_chat_box();
        cb.focused = false;
        let event = UiEvent::TextInput {
            text: "hi".to_owned(),
        };
        let resp = cb.handle_event(&event);
        assert_eq!(resp, EventResponse::Ignored);
        assert_eq!(cb.input_text(), "");
    }

    #[test]
    fn text_input_respects_max_len() {
        let mut cb = test_chat_box();
        cb.focused = true;
        cb.input_buf = "x".repeat(MAX_INPUT_LEN);
        let event = UiEvent::TextInput {
            text: "a".to_owned(),
        };
        cb.handle_event(&event);
        assert_eq!(cb.input_buf.len(), MAX_INPUT_LEN);
    }

    // -- submit / enter --

    #[test]
    fn enter_submits_and_clears_input() {
        let mut cb = test_chat_box();
        cb.focused = true;
        cb.input_buf = "hello world".to_owned();
        cb.input_cursor = cb.input_buf.len();
        let event = UiEvent::KeyDown {
            keycode: Keycode::Return,
            modifiers: crate::ui::widget::KeyModifiers::default(),
        };
        let resp = cb.handle_event(&event);
        assert_eq!(resp, EventResponse::Consumed);
        assert_eq!(cb.input_text(), "");
        assert_eq!(cb.input_cursor, 0);
        assert!(!cb.focused);

        let actions = cb.take_actions();
        assert_eq!(actions.len(), 1);
        match &actions[0] {
            WidgetAction::SendChat(text) => assert_eq!(text, "hello world"),
            _ => panic!("Expected SendChat action"),
        }
    }

    #[test]
    fn enter_on_empty_input_drops_focus_without_action() {
        let mut cb = test_chat_box();
        cb.focused = true;
        let event = UiEvent::KeyDown {
            keycode: Keycode::Return,
            modifiers: crate::ui::widget::KeyModifiers::default(),
        };
        cb.handle_event(&event);
        assert!(cb.take_actions().is_empty());
        // Focus must be dropped so the transparent box cannot silently eat
        // subsequent keystrokes.
        assert!(!cb.focused);
    }

    #[test]
    fn arrow_keys_move_cursor_and_backspace_deletes_before_it() {
        let mut cb = test_chat_box();
        cb.focused = true;
        cb.input_buf = "abc".to_owned();
        cb.input_cursor = cb.input_buf.len();

        cb.handle_event(&UiEvent::KeyDown {
            keycode: Keycode::Left,
            modifiers: crate::ui::widget::KeyModifiers::default(),
        });
        assert_eq!(cb.input_cursor, 2);

        cb.handle_event(&UiEvent::KeyDown {
            keycode: Keycode::Backspace,
            modifiers: crate::ui::widget::KeyModifiers::default(),
        });
        assert_eq!(cb.input_text(), "ac");
        assert_eq!(cb.input_cursor, 1);
    }

    #[test]
    fn focused_update_keeps_chat_visible_and_blinks_caret() {
        let mut cb = test_chat_box();
        cb.focused = true;

        cb.update(Duration::from_secs(10));

        assert!(cb.focused);
        assert_eq!(cb.alpha, 255);
        assert!(cb.caret_elapsed < CARET_BLINK_PERIOD_SECS);
    }

    #[test]
    fn visible_input_window_keeps_cursor_visible() {
        let mut cb = test_chat_box();
        cb.input_buf = "abcdefghijklmnopqrstuvwxyz".to_owned();
        cb.input_cursor = cb.input_buf.len();

        let (visible, cursor) = cb.visible_input_window(10);

        assert_eq!(visible, "qrstuvwxyz");
        assert_eq!(cursor, 10);
    }

    #[test]
    fn escape_drops_focus() {
        let mut cb = test_chat_box();
        cb.focused = true;
        cb.input_buf = "half typed".to_owned();
        let event = UiEvent::KeyDown {
            keycode: Keycode::Escape,
            modifiers: crate::ui::widget::KeyModifiers::default(),
        };
        let resp = cb.handle_event(&event);
        assert_eq!(resp, EventResponse::Consumed);
        assert!(!cb.focused);
        // Input buffer preserved so player can re-open and continue
        assert_eq!(cb.input_text(), "half typed");
    }

    #[test]
    fn up_when_unfocused_focuses_and_loads_last_message() {
        let mut cb = test_chat_box();
        cb.input_buf = "last sent".to_owned();
        cb.submit_input();
        cb.take_actions();

        assert!(!cb.focused);

        let event = UiEvent::KeyDown {
            keycode: Keycode::Up,
            modifiers: crate::ui::widget::KeyModifiers::default(),
        };
        let resp = cb.handle_event(&event);

        assert_eq!(resp, EventResponse::Consumed);
        assert!(cb.focused);
        assert_eq!(cb.input_text(), "last sent");
    }

    #[test]
    fn up_when_unfocused_with_empty_history_only_focuses() {
        let mut cb = test_chat_box();
        assert!(!cb.focused);

        let event = UiEvent::KeyDown {
            keycode: Keycode::Up,
            modifiers: crate::ui::widget::KeyModifiers::default(),
        };
        let resp = cb.handle_event(&event);

        assert_eq!(resp, EventResponse::Consumed);
        assert!(cb.focused);
        assert_eq!(cb.input_text(), "");
    }

    #[test]
    fn enter_when_unfocused_sets_focus_without_submitting() {
        let mut cb = test_chat_box();
        cb.focused = false;
        cb.input_buf = "pending".to_owned();

        let event = UiEvent::KeyDown {
            keycode: Keycode::Return,
            modifiers: crate::ui::widget::KeyModifiers::default(),
        };

        let resp = cb.handle_event(&event);

        assert_eq!(resp, EventResponse::Consumed);
        assert!(cb.focused);
        assert_eq!(cb.input_text(), "pending");
        assert!(cb.take_actions().is_empty());
    }

    // -- backspace --

    #[test]
    fn backspace_removes_last_char() {
        let mut cb = test_chat_box();
        cb.focused = true;
        cb.input_buf = "abc".to_owned();
        cb.input_cursor = cb.input_buf.len();
        let event = UiEvent::KeyDown {
            keycode: Keycode::Backspace,
            modifiers: crate::ui::widget::KeyModifiers::default(),
        };
        cb.handle_event(&event);
        assert_eq!(cb.input_text(), "ab");
    }

    #[test]
    fn backspace_on_empty_is_noop() {
        let mut cb = test_chat_box();
        cb.focused = true;
        let event = UiEvent::KeyDown {
            keycode: Keycode::Backspace,
            modifiers: crate::ui::widget::KeyModifiers::default(),
        };
        cb.handle_event(&event);
        assert_eq!(cb.input_text(), "");
    }

    // -- history navigation --

    #[test]
    fn up_navigates_back_in_history() {
        let mut cb = test_chat_box();
        // Simulate two sent messages
        cb.input_buf = "msg1".to_owned();
        cb.submit_input();
        cb.input_buf = "msg2".to_owned();
        cb.submit_input();
        cb.take_actions(); // drain

        cb.focused = true;
        cb.input_buf = "draft".to_owned();

        // Press Up --> should load "msg2"
        let up = UiEvent::KeyDown {
            keycode: Keycode::Up,
            modifiers: crate::ui::widget::KeyModifiers::default(),
        };
        cb.handle_event(&up);
        assert_eq!(cb.input_text(), "msg2");

        // Press Up again --> should load "msg1"
        cb.handle_event(&up);
        assert_eq!(cb.input_text(), "msg1");
    }

    #[test]
    fn down_restores_draft() {
        let mut cb = test_chat_box();
        cb.input_buf = "sent".to_owned();
        cb.submit_input();
        cb.take_actions();

        cb.focused = true;
        cb.input_buf = "my draft".to_owned();

        let up = UiEvent::KeyDown {
            keycode: Keycode::Up,
            modifiers: crate::ui::widget::KeyModifiers::default(),
        };
        cb.handle_event(&up);
        assert_eq!(cb.input_text(), "sent");

        let down = UiEvent::KeyDown {
            keycode: Keycode::Down,
            modifiers: crate::ui::widget::KeyModifiers::default(),
        };
        cb.handle_event(&down);
        assert_eq!(cb.input_text(), "my draft");
    }

    #[test]
    fn tab_cycles_chat_command_prefixes_only() {
        let mut cb = test_chat_box();
        for text in [
            "/autoloot",
            "/tell bob hello!",
            "/rank",
            "/gtell meet at temple",
            "plain speech",
            "/shout incoming",
        ] {
            cb.input_buf = text.to_owned();
            cb.submit_input();
        }
        cb.take_actions();
        cb.focused = true;

        let tab = UiEvent::KeyDown {
            keycode: Keycode::Tab,
            modifiers: crate::ui::widget::KeyModifiers::default(),
        };

        assert_eq!(cb.handle_event(&tab), EventResponse::Consumed);
        assert_eq!(cb.input_text(), "/shout ");
        assert_eq!(cb.input_cursor, cb.input_text().len());

        cb.handle_event(&tab);
        assert_eq!(cb.input_text(), "/gtell ");

        cb.handle_event(&tab);
        assert_eq!(cb.input_text(), "/tell bob ");

        cb.handle_event(&tab);
        assert_eq!(cb.input_text(), "/shout ");
    }

    #[test]
    fn tab_accepts_hash_commands_and_preserves_sigil() {
        let mut cb = test_chat_box();
        cb.input_buf = "#tell Alice thanks".to_owned();
        cb.submit_input();
        cb.take_actions();
        cb.focused = true;

        let resp = cb.handle_event(&UiEvent::KeyDown {
            keycode: Keycode::Tab,
            modifiers: crate::ui::widget::KeyModifiers::default(),
        });

        assert_eq!(resp, EventResponse::Consumed);
        assert_eq!(cb.input_text(), "#tell Alice ");
    }

    #[test]
    fn submit_adds_to_history() {
        let mut cb = test_chat_box();
        cb.input_buf = "test msg".to_owned();
        cb.submit_input();
        assert_eq!(cb.sent_chat_history.len(), 1);
        assert_eq!(cb.sent_chat_history[0], "test msg");
    }

    #[test]
    fn history_capped_at_max() {
        let mut cb = test_chat_box();
        for i in 0..MAX_HISTORY_LEN + 10 {
            cb.input_buf = format!("msg {}", i);
            cb.submit_input();
        }
        assert_eq!(cb.sent_chat_history.len(), MAX_HISTORY_LEN);
    }

    // -- font_for_color --

    #[test]
    fn font_color_mapping() {
        assert_eq!(ChatBox::font_for_color(LogMessageColor::Red), 0);
        assert_eq!(ChatBox::font_for_color(LogMessageColor::Yellow), 1);
        assert_eq!(ChatBox::font_for_color(LogMessageColor::Green), 2);
        assert_eq!(ChatBox::font_for_color(LogMessageColor::Blue), 3);
    }

    // -- idle fade --

    #[test]
    fn update_no_fade_before_delay() {
        use std::time::Duration;
        let mut cb = test_chat_box();
        cb.update(Duration::from_secs_f32(IDLE_FADE_DELAY_SECS - 0.1));
        assert_eq!(cb.alpha, 255);
    }

    #[test]
    fn update_mid_fade() {
        use std::time::Duration;
        let mut cb = test_chat_box();
        cb.update(Duration::from_secs_f32(
            IDLE_FADE_DELAY_SECS + IDLE_FADE_DURATION_SECS * 0.5,
        ));
        assert!(cb.alpha > 0 && cb.alpha < 255, "alpha={}", cb.alpha);
    }

    #[test]
    fn update_fully_faded() {
        use std::time::Duration;
        let mut cb = test_chat_box();
        cb.update(Duration::from_secs_f32(
            IDLE_FADE_DELAY_SECS + IDLE_FADE_DURATION_SECS + 1.0,
        ));
        assert_eq!(cb.alpha, 0);
    }

    #[test]
    fn update_reset_on_text_input() {
        use std::time::Duration;
        let mut cb = test_chat_box();
        // Age the widget past the full fade threshold.
        cb.idle_elapsed = IDLE_FADE_DELAY_SECS + IDLE_FADE_DURATION_SECS + 1.0;
        cb.update(Duration::ZERO);
        assert_eq!(cb.alpha, 0);
        // Any text input should reset the timer even if nothing is appended.
        cb.focused = true;
        cb.handle_event(&UiEvent::TextInput {
            text: "a".to_owned(),
        });
        cb.update(Duration::ZERO);
        assert_eq!(cb.alpha, 255);
    }

    #[test]
    fn update_reset_on_push_message() {
        use std::time::Duration;
        let mut cb = test_chat_box();
        cb.idle_elapsed = IDLE_FADE_DELAY_SECS + IDLE_FADE_DURATION_SECS + 1.0;
        cb.update(Duration::ZERO);
        assert_eq!(cb.alpha, 0);
        cb.push_message(make_msg("incoming", LogMessageColor::Green));
        cb.update(Duration::ZERO);
        assert_eq!(cb.alpha, 255);
    }

    #[test]
    fn update_not_reset_on_click_inside() {
        use std::time::Duration;
        let mut cb = test_chat_box();
        cb.idle_elapsed = IDLE_FADE_DELAY_SECS + IDLE_FADE_DURATION_SECS + 1.0;
        cb.update(Duration::ZERO);
        assert_eq!(cb.alpha, 0);
        cb.handle_event(&UiEvent::MouseClick {
            x: 50,
            y: 400,
            button: crate::ui::widget::MouseButton::Left,
            modifiers: crate::ui::widget::KeyModifiers::default(),
        });
        cb.update(Duration::ZERO);
        assert_eq!(cb.alpha, 0);
    }
}
