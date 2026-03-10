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

use super::style::Padding;
use super::widget::{Bounds, EventResponse, UiEvent, Widget, WidgetAction};
use super::RenderContext;

/// Maximum characters allowed in the chat input buffer.
const MAX_INPUT_LEN: usize = 120;

/// Maximum number of previously sent messages kept in history.
const MAX_HISTORY_LEN: usize = 100;

/// Height reserved for the input area (separator gap + one line of text).
const INPUT_AREA_H: u32 = font_cache::BITMAP_GLYPH_H + 4; // 2px gap above + 2px below

/// Default bitmap font index for the input line (yellow).
const INPUT_FONT: usize = 1;

/// Colour of the thin separator line between log and input.
const SEPARATOR_COLOR: Color = Color::RGBA(120, 120, 140, 200);

/// Seconds of inactivity before the fade-out animation begins.
const IDLE_FADE_DELAY_SECS: f32 = 5.0;

/// Duration in seconds of the fade-out transition (fully opaque → invisible).
const IDLE_FADE_DURATION_SECS: f32 = 1.0;

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
    sent_chat_history: Vec<String>,
    chat_history_index: Option<usize>,
    chat_history_draft: Option<String>,

    // -- Focus & actions --
    focused: bool,
    pending_actions: Vec<WidgetAction>,

    // -- Idle fade --
    /// Seconds elapsed since the last user interaction or incoming message.
    idle_elapsed: f32,
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
    /// A new `ChatBox` with focus enabled by default.
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
            sent_chat_history: Vec::new(),
            chat_history_index: None,
            chat_history_draft: None,
            focused: true,
            pending_actions: Vec::new(),
            idle_elapsed: 0.0,
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
    pub fn message_count(&self) -> usize {
        self.messages.len()
    }

    /// Returns a read-only view of the current input text.
    pub fn input_text(&self) -> &str {
        &self.input_buf
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

    // -- Input / history helpers --

    /// Handles the Enter key: sends the current input and records history.
    fn submit_input(&mut self) {
        if self.input_buf.is_empty() {
            return;
        }
        let text = self.input_buf.clone();
        self.input_buf.clear();

        self.sent_chat_history.push(text.clone());
        if self.sent_chat_history.len() > MAX_HISTORY_LEN {
            self.sent_chat_history.remove(0);
        }
        self.chat_history_index = None;
        self.chat_history_draft = None;

        self.pending_actions.push(WidgetAction::SendChat(text));
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
            self.input_buf = msg.clone();
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
                self.input_buf = msg.clone();
            }
        } else {
            self.chat_history_index = None;
            self.input_buf = self.chat_history_draft.take().unwrap_or_default();
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

            UiEvent::MouseClick { x, y, .. } => {
                if self.bounds.contains_point(*x, *y) {
                    self.idle_elapsed = 0.0;
                    self.focused = true;
                    EventResponse::Consumed
                } else {
                    self.focused = false;
                    EventResponse::Ignored
                }
            }

            UiEvent::TextInput { text } => {
                if !self.focused {
                    return EventResponse::Ignored;
                }
                self.idle_elapsed = 0.0;
                if self.input_buf.len() + text.len() <= MAX_INPUT_LEN {
                    self.input_buf.push_str(text);
                }
                EventResponse::Consumed
            }

            UiEvent::KeyDown { keycode, .. } => {
                if !self.focused {
                    return EventResponse::Ignored;
                }
                self.idle_elapsed = 0.0;
                match *keycode {
                    Keycode::Return | Keycode::KpEnter => {
                        self.submit_input();
                        EventResponse::Consumed
                    }
                    Keycode::Backspace => {
                        self.input_buf.pop();
                        EventResponse::Consumed
                    }
                    Keycode::Up => {
                        self.history_back();
                        EventResponse::Consumed
                    }
                    Keycode::Down => {
                        self.history_forward();
                        EventResponse::Consumed
                    }
                    _ => EventResponse::Ignored,
                }
            }

            UiEvent::MouseMove { .. } => EventResponse::Ignored,
        }
    }

    fn update(&mut self, dt: Duration) {
        self.idle_elapsed += dt.as_secs_f32();
        self.alpha = if self.idle_elapsed < IDLE_FADE_DELAY_SECS {
            255
        } else {
            let t = ((self.idle_elapsed - IDLE_FADE_DELAY_SECS) / IDLE_FADE_DURATION_SECS).min(1.0);
            ((1.0 - t) * 255.0) as u8
        };
    }

    fn render(&mut self, ctx: &mut RenderContext) -> Result<(), String> {
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
        let bg_a = ((self.bg_color.a as f32) * (self.alpha as f32 / 255.0)) as u8;
        let bg_color = Color::RGBA(self.bg_color.r, self.bg_color.g, self.bg_color.b, bg_a);
        ctx.canvas.set_blend_mode(BlendMode::Blend);
        ctx.canvas.set_draw_color(bg_color);
        ctx.canvas.fill_rect(bg_rect)?;

        let inner = self.bounds.inner(&self.padding);

        // 2. Render log lines (top→bottom, newest at bottom)
        for line in 0..self.visible_lines {
            let idx_from_most_recent = self
                .scroll_offset
                .saturating_add(self.visible_lines.saturating_sub(1).saturating_sub(line));

            if let Some(msg) = self.message_from_end(idx_from_most_recent) {
                let font = Self::font_for_color(msg.color);
                let y = inner.y + (line as i32) * self.line_height as i32;
                font_cache::draw_text_faded(
                    ctx.canvas,
                    ctx.gfx,
                    font,
                    &msg.message,
                    inner.x,
                    y,
                    self.alpha,
                )?;
            }
        }

        // 3. Separator line between log and input (alpha-scaled)
        let sep_a = ((SEPARATOR_COLOR.a as f32) * (self.alpha as f32 / 255.0)) as u8;
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
        let input_y = sep_y + 3; // 3px below separator
        font_cache::draw_text_faded(
            ctx.canvas,
            ctx.gfx,
            INPUT_FONT,
            &self.input_buf,
            inner.x,
            input_y,
            self.alpha,
        )?;

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
    fn click_inside_sets_focus() {
        let mut cb = test_chat_box();
        cb.focused = false;
        let event = UiEvent::MouseClick {
            x: 50,
            y: 400,
            button: super::super::widget::MouseButton::Left,
        };
        let resp = cb.handle_event(&event);
        assert_eq!(resp, EventResponse::Consumed);
        assert!(cb.focused);
    }

    #[test]
    fn click_outside_clears_focus() {
        let mut cb = test_chat_box();
        assert!(cb.focused);
        let event = UiEvent::MouseClick {
            x: 800,
            y: 100,
            button: super::super::widget::MouseButton::Left,
        };
        let resp = cb.handle_event(&event);
        assert_eq!(resp, EventResponse::Ignored);
        assert!(!cb.focused);
    }

    // -- text input --

    #[test]
    fn text_input_appends_when_focused() {
        let mut cb = test_chat_box();
        let event = UiEvent::TextInput {
            text: "hi".to_owned(),
        };
        let resp = cb.handle_event(&event);
        assert_eq!(resp, EventResponse::Consumed);
        assert_eq!(cb.input_text(), "hi");
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
        cb.input_buf = "hello world".to_owned();
        let event = UiEvent::KeyDown {
            keycode: Keycode::Return,
            modifiers: super::super::widget::KeyModifiers::default(),
        };
        let resp = cb.handle_event(&event);
        assert_eq!(resp, EventResponse::Consumed);
        assert_eq!(cb.input_text(), "");

        let actions = cb.take_actions();
        assert_eq!(actions.len(), 1);
        match &actions[0] {
            WidgetAction::SendChat(text) => assert_eq!(text, "hello world"),
            _ => panic!("Expected SendChat action"),
        }
    }

    #[test]
    fn enter_on_empty_input_does_nothing() {
        let mut cb = test_chat_box();
        let event = UiEvent::KeyDown {
            keycode: Keycode::Return,
            modifiers: super::super::widget::KeyModifiers::default(),
        };
        cb.handle_event(&event);
        assert!(cb.take_actions().is_empty());
    }

    // -- backspace --

    #[test]
    fn backspace_removes_last_char() {
        let mut cb = test_chat_box();
        cb.input_buf = "abc".to_owned();
        let event = UiEvent::KeyDown {
            keycode: Keycode::Backspace,
            modifiers: super::super::widget::KeyModifiers::default(),
        };
        cb.handle_event(&event);
        assert_eq!(cb.input_text(), "ab");
    }

    #[test]
    fn backspace_on_empty_is_noop() {
        let mut cb = test_chat_box();
        let event = UiEvent::KeyDown {
            keycode: Keycode::Backspace,
            modifiers: super::super::widget::KeyModifiers::default(),
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

        cb.input_buf = "draft".to_owned();

        // Press Up → should load "msg2"
        let up = UiEvent::KeyDown {
            keycode: Keycode::Up,
            modifiers: super::super::widget::KeyModifiers::default(),
        };
        cb.handle_event(&up);
        assert_eq!(cb.input_text(), "msg2");

        // Press Up again → should load "msg1"
        cb.handle_event(&up);
        assert_eq!(cb.input_text(), "msg1");
    }

    #[test]
    fn down_restores_draft() {
        let mut cb = test_chat_box();
        cb.input_buf = "sent".to_owned();
        cb.submit_input();
        cb.take_actions();

        cb.input_buf = "my draft".to_owned();

        let up = UiEvent::KeyDown {
            keycode: Keycode::Up,
            modifiers: super::super::widget::KeyModifiers::default(),
        };
        cb.handle_event(&up);
        assert_eq!(cb.input_text(), "sent");

        let down = UiEvent::KeyDown {
            keycode: Keycode::Down,
            modifiers: super::super::widget::KeyModifiers::default(),
        };
        cb.handle_event(&down);
        assert_eq!(cb.input_text(), "my draft");
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
    fn update_reset_on_click_inside() {
        use std::time::Duration;
        let mut cb = test_chat_box();
        cb.idle_elapsed = IDLE_FADE_DELAY_SECS + IDLE_FADE_DURATION_SECS + 1.0;
        cb.update(Duration::ZERO);
        assert_eq!(cb.alpha, 0);
        cb.handle_event(&UiEvent::MouseClick {
            x: 50,
            y: 400,
            button: super::super::widget::MouseButton::Left,
        });
        cb.update(Duration::ZERO);
        assert_eq!(cb.alpha, 255);
    }
}
