//! Always-on helper window that guides newer players.
//!
//! Shows two sections: control hints (reflecting the player's current key
//! bindings) and an objectives list fed by a [`QuestLog`]. The window is
//! dragged by its title bar and can be hidden from Display Settings.

use sdl2::pixels::Color;
use sdl2::render::BlendMode;

use super::settings_panel::consume_mouse_events_in_bounds;
use crate::font_cache::{self, BITMAP_GLYPH_H, TextStyle};
use crate::quests::QuestLog;
use crate::ui::RenderContext;
use crate::ui::widget::{Bounds, EventResponse, GameAction, KeyBindings, UiEvent, Widget};
use crate::ui::widgets::title_bar::{TITLE_BAR_H, TitleBar, clamp_to_viewport};

/// Fixed window width in logical pixels.
pub const HELPER_WINDOW_W: u32 = 220;
/// Inner padding between the window edge and its text.
const PAD: i32 = 6;
/// Vertical distance between consecutive text lines.
const LINE_H: i32 = BITMAP_GLYPH_H as i32 + 2;
/// Extra vertical space between sections.
const SECTION_GAP: i32 = 6;
/// Bitmap font for section headers.
const HEADER_FONT: usize = 1;
/// Bitmap font for section body text.
const BODY_FONT: usize = 0;
/// Window background color (matches the other HUD panels).
const BG_COLOR: Color = Color::RGBA(10, 10, 30, 180);
/// Window border color.
const BORDER_COLOR: Color = Color::RGBA(120, 120, 140, 200);
/// Placeholder shown when the quest log has no entries.
const NO_OBJECTIVES_TEXT: &str = "No current objectives.";

/// Movable HUD window showing control hints and the current [`QuestLog`].
pub struct HelperWindow {
    bounds: Bounds,
    title_bar: TitleBar,
    /// Unwrapped control-hint lines.
    control_hints: Vec<String>,
    /// Unwrapped objective lines (or the empty-log placeholder).
    objectives: Vec<String>,
    /// Pre-wrapped `(text, font, y offset from window top)` lines to draw.
    lines: Vec<(String, usize, i32)>,
}

impl HelperWindow {
    /// Creates a helper window with default key-binding hints and no objectives.
    ///
    /// # Arguments
    ///
    /// * `x` - Left edge in logical pixels.
    /// * `y` - Top edge in logical pixels.
    ///
    /// # Returns
    ///
    /// * A new `HelperWindow`.
    pub fn new(x: i32, y: i32) -> Self {
        let mut window = Self {
            bounds: Bounds::new(x, y, HELPER_WINDOW_W, TITLE_BAR_H as u32),
            title_bar: TitleBar::new_drag_only("Helper", x, y, HELPER_WINDOW_W),
            control_hints: control_hints(&KeyBindings::default()),
            objectives: Vec::new(),
            lines: Vec::new(),
        };
        window.set_quest_log(&QuestLog::default());
        window
    }

    /// Returns whether the window is currently being dragged.
    ///
    /// # Returns
    ///
    /// * `true` while a title-bar drag is in progress.
    pub fn is_dragging(&self) -> bool {
        self.title_bar.is_dragging()
    }

    /// Rebuilds the control hints from the player's current key bindings.
    ///
    /// # Arguments
    ///
    /// * `bindings` - The active character's keyboard bindings.
    pub fn set_key_bindings(&mut self, bindings: &KeyBindings) {
        self.control_hints = control_hints(bindings);
        self.relayout();
    }

    /// Replaces the "What to do next" list.
    ///
    /// # Arguments
    ///
    /// * `quest_log` - Objectives to show, most relevant first.
    pub fn set_quest_log(&mut self, quest_log: &QuestLog) {
        self.objectives = if quest_log.entries.is_empty() {
            vec![NO_OBJECTIVES_TEXT.to_owned()]
        } else {
            quest_log
                .entries
                .iter()
                .map(|e| format!("- {} ({},{})", e.description, e.x, e.y))
                .collect()
        };
        self.relayout();
    }

    /// Re-wraps all text, resizes the window to fit, and re-clamps it on screen.
    fn relayout(&mut self) {
        let wrap_w = HELPER_WINDOW_W - 2 * PAD as u32;
        let mut lines = Vec::new();
        let mut y = TITLE_BAR_H + PAD;
        for (header, body) in [
            ("Controls", &self.control_hints),
            ("Objectives", &self.objectives),
        ] {
            lines.push((header.to_owned(), HEADER_FONT, y));
            y += LINE_H;
            for line in body
                .iter()
                .flat_map(|t| font_cache::wrap_lines_bitmap(t, wrap_w))
            {
                lines.push((line, BODY_FONT, y));
                y += LINE_H;
            }
            y += SECTION_GAP;
        }
        self.lines = lines;
        self.bounds.height = (y - SECTION_GAP + PAD) as u32;
        self.set_position(self.bounds.x, self.bounds.y);
    }
}

/// Builds the control-hint lines, using live bindings for rebindable actions.
///
/// # Arguments
///
/// * `bindings` - The active character's keyboard bindings.
///
/// # Returns
///
/// * One unwrapped hint string per control.
fn control_hints(bindings: &KeyBindings) -> Vec<String> {
    let mut hints: Vec<String> = [
        "Ctrl+L-click: Attack / give item",
        "Shift+L-click: Take / use / drop",
        "Ctrl+R-click: Look at character",
        "Shift+R-click: Look at item",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    hints.extend(GameAction::ALL.iter().filter_map(|&action| {
        bindings
            .binding_for(action)
            .map(|kb| format!("{kb}: {}", action.label()))
    }));
    hints
}

impl Widget for HelperWindow {
    fn bounds(&self) -> &Bounds {
        &self.bounds
    }

    fn set_position(&mut self, x: i32, y: i32) {
        let (x, y) = clamp_to_viewport(x, y, self.bounds.width, self.bounds.height);
        self.bounds.x = x;
        self.bounds.y = y;
        self.title_bar.set_bar_position(x, y);
    }

    fn handle_event(&mut self, event: &UiEvent) -> EventResponse {
        let (response, drag) = self.title_bar.handle_event(event);
        if let Some((x, y)) = drag {
            self.set_position(x, y);
        }
        if response == EventResponse::Consumed {
            return EventResponse::Consumed;
        }
        // Keep clicks on the window body from reaching the world map.
        consume_mouse_events_in_bounds(&self.bounds, event)
    }

    fn render(&mut self, ctx: &mut RenderContext<'_, '_>) -> Result<(), String> {
        let rect = sdl2::rect::Rect::new(
            self.bounds.x,
            self.bounds.y,
            self.bounds.width,
            self.bounds.height,
        );
        ctx.canvas.set_blend_mode(BlendMode::Blend);
        ctx.canvas.set_draw_color(BG_COLOR);
        ctx.canvas.fill_rect(rect)?;
        ctx.canvas.set_draw_color(BORDER_COLOR);
        ctx.canvas.draw_rect(rect)?;
        self.title_bar.render(ctx)?;

        for (text, font, dy) in &self.lines {
            font_cache::draw_text(
                ctx.canvas,
                ctx.gfx,
                *font,
                text,
                self.bounds.x + PAD,
                self.bounds.y + dy,
                TextStyle::PLAIN,
            )?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::quests::QuestLogEntry;
    use crate::ui::widget::{KeyBinding, KeyModifiers, MouseButton};
    use sdl2::keyboard::Keycode;

    fn has_line(window: &HelperWindow, needle: &str) -> bool {
        window
            .lines
            .iter()
            .any(|(text, _, _)| text.contains(needle))
    }

    #[test]
    fn quest_log_entries_resize_window_and_show_coordinates() {
        let mut window = HelperWindow::new(0, 0);
        assert!(has_line(&window, NO_OBJECTIVES_TEXT));
        let empty_h = window.bounds().height;

        window.set_quest_log(&QuestLog {
            entries: vec![QuestLogEntry {
                description: "Talk to the Cityguard captain about the Black Candle".into(),
                x: 512,
                y: 498,
                min_rank: mag_core::ranks::Rank::Private,
            }],
        });

        assert!(!has_line(&window, NO_OBJECTIVES_TEXT));
        assert!(has_line(&window, "(512,498)"));
        assert!(window.bounds().height > empty_h);
        let last_y = window.lines.last().unwrap().2;
        assert!(last_y + LINE_H <= window.bounds().height as i32);
    }

    #[test]
    fn control_hints_follow_rebound_keys() {
        let mut window = HelperWindow::new(0, 0);
        assert!(has_line(&window, "S: Toggle Skills"));

        let mut bindings = KeyBindings::default();
        bindings.set_binding(
            GameAction::ToggleSkills,
            KeyBinding::new(Keycode::K, KeyModifiers::default()),
        );
        window.set_key_bindings(&bindings);

        assert!(has_line(&window, "K: Toggle Skills"));
        assert!(!has_line(&window, "S: Toggle Skills"));
    }

    #[test]
    fn title_bar_drag_moves_window_and_body_clicks_are_consumed() {
        let mods = KeyModifiers::default();
        let mut window = HelperWindow::new(100, 100);

        let body_click = UiEvent::MouseClick {
            x: 110,
            y: 100 + TITLE_BAR_H + 20,
            button: MouseButton::Left,
            modifiers: mods,
        };
        assert_eq!(window.handle_event(&body_click), EventResponse::Consumed);
        assert!(!window.is_dragging());

        window.handle_event(&UiEvent::MouseDown {
            x: 150,
            y: 105,
            button: MouseButton::Left,
            modifiers: mods,
        });
        assert!(window.is_dragging());
        window.handle_event(&UiEvent::MouseMove { x: -900, y: -900 });
        assert_eq!((window.bounds().x, window.bounds().y), (0, 0));
        window.handle_event(&UiEvent::MouseClick {
            x: 0,
            y: 0,
            button: MouseButton::Left,
            modifiers: mods,
        });
        assert!(!window.is_dragging());
    }
}
