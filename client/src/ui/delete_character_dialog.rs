//! Modal confirmation dialog for deleting a character.
//!
//! Asks the user to type the character's name to confirm deletion.
//! The owning scene reads pending [`DeleteCharacterDialogAction`]s via
//! [`DeleteCharacterDialog::take_actions`].

use std::time::Duration;

use sdl2::keyboard::Keycode;
use sdl2::pixels::Color;
use sdl2::render::BlendMode;

use super::button::RectButton;
use super::style::{Background, Border};
use super::text_input::TextInput;
use super::widget::{Bounds, EventResponse, MouseButton, UiEvent, Widget};
use super::RenderContext;
use crate::font_cache;

// ---------------------------------------------------------------------------
// Layout constants
// ---------------------------------------------------------------------------

/// Dialog dimensions.
const DIALOG_W: u32 = 400;
const DIALOG_H: u32 = 200;

/// Horizontal padding inside the dialog.
const PAD_X: i32 = 20;

/// Width of the confirmation text input.
const INPUT_W: u32 = DIALOG_W - (PAD_X as u32) * 2;

/// Height of the text input.
const INPUT_H: u32 = 16;

/// Button height.
const BTN_H: u32 = 22;

/// Gap between buttons.
const BTN_GAP: i32 = 6;

/// Bitmap font index.
const FONT: usize = 1;

// ---------------------------------------------------------------------------
// Actions
// ---------------------------------------------------------------------------

/// A side-effect produced by the delete character dialog.
#[derive(Clone, Debug)]
pub enum DeleteCharacterDialogAction {
    /// User confirmed deletion.
    Confirm {
        /// Character ID to delete.
        character_id: u64,
    },
    /// User cancelled.
    Cancel,
}

// ---------------------------------------------------------------------------
// Widget
// ---------------------------------------------------------------------------

/// Modal dialog that asks the user to type a character's name to confirm deletion.
pub struct DeleteCharacterDialog {
    bounds: Bounds,
    /// Whether the dialog is visible.
    visible: bool,
    /// Character ID pending deletion.
    character_id: u64,
    /// Expected character name to match.
    expected_name: String,
    /// Confirmation text input.
    name_input: TextInput,
    /// Confirm button.
    confirm_button: RectButton,
    /// Cancel button.
    cancel_button: RectButton,
    /// Whether a delete operation is in progress.
    is_deleting: bool,
    /// Pending actions for the scene to drain.
    actions: Vec<DeleteCharacterDialogAction>,
}

impl DeleteCharacterDialog {
    /// Creates a new, initially hidden delete character dialog.
    ///
    /// # Returns
    ///
    /// A fully-initialised `DeleteCharacterDialog`.
    pub fn new() -> Self {
        let panel_x =
            (crate::constants::TARGET_WIDTH_INT - DIALOG_W) as i32 / 2;
        let panel_y =
            (crate::constants::TARGET_HEIGHT_INT - DIALOG_H) as i32 / 2;

        let bounds = Bounds::new(panel_x, panel_y, DIALOG_W, DIALOG_H);

        let border_normal = Color::RGBA(100, 100, 140, 200);
        let border_focused = Color::RGBA(180, 180, 255, 255);

        let name_input = TextInput::new(
            Bounds::new(panel_x + PAD_X, panel_y + 80, INPUT_W, INPUT_H),
            "type character name",
            FONT,
            64,
            false,
            border_normal,
            border_focused,
        );

        let btn_bg = Background::SolidColor(Color::RGBA(80, 30, 30, 200));
        let btn_border = Border {
            color: Color::RGBA(200, 80, 80, 200),
            width: 1,
        };
        let cancel_bg = Background::SolidColor(Color::RGBA(50, 50, 80, 200));
        let cancel_border = Border {
            color: Color::RGBA(120, 120, 180, 200),
            width: 1,
        };

        let total_btn_w = 2 * 140 + BTN_GAP as u32;
        let btn_start_x = panel_x + (DIALOG_W as i32 - total_btn_w as i32) / 2;
        let btn_y = panel_y + DIALOG_H as i32 - BTN_H as i32 - 30;

        let confirm_button =
            RectButton::new(Bounds::new(btn_start_x, btn_y, 140, BTN_H), btn_bg)
                .with_border(btn_border)
                .with_label("Confirm delete", FONT);

        let cancel_button = RectButton::new(
            Bounds::new(btn_start_x + 140 + BTN_GAP, btn_y, 140, BTN_H),
            cancel_bg,
        )
        .with_border(cancel_border)
        .with_label("Cancel", FONT);

        let mut dialog = Self {
            bounds,
            visible: false,
            character_id: 0,
            expected_name: String::new(),
            name_input,
            confirm_button,
            cancel_button,
            is_deleting: false,
            actions: Vec::new(),
        };
        dialog.name_input.set_focused(true);
        dialog
    }

    /// Shows the dialog for the given character.
    ///
    /// # Arguments
    ///
    /// * `character_id` - ID of the character to delete.
    /// * `character_name` - Name the user must type to confirm.
    pub fn show(&mut self, character_id: u64, character_name: &str) {
        self.visible = true;
        self.character_id = character_id;
        self.expected_name = character_name.to_owned();
        self.name_input.set_value("");
        self.name_input.set_focused(true);
        self.is_deleting = false;
    }

    /// Hides the dialog and resets state.
    pub fn hide(&mut self) {
        self.visible = false;
        self.character_id = 0;
        self.expected_name.clear();
        self.name_input.set_value("");
        self.is_deleting = false;
    }

    /// Returns `true` if the dialog is currently visible.
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Sets the deleting-in-progress state.
    ///
    /// # Arguments
    ///
    /// * `deleting` - `true` while the delete request is in flight.
    pub fn set_deleting(&mut self, deleting: bool) {
        self.is_deleting = deleting;
    }

    /// Drains pending [`DeleteCharacterDialogAction`]s.
    ///
    /// # Returns
    ///
    /// A vector of actions produced since the last call.
    pub fn take_actions(&mut self) -> Vec<DeleteCharacterDialogAction> {
        std::mem::take(&mut self.actions)
    }

    /// Returns `true` when the typed name matches the expected name.
    fn name_matches(&self) -> bool {
        self.name_input.value() == self.expected_name
    }
}

impl Widget for DeleteCharacterDialog {
    fn bounds(&self) -> &Bounds {
        &self.bounds
    }

    fn set_position(&mut self, _x: i32, _y: i32) {
        // Fixed layout — repositioning not supported.
    }

    fn handle_event(&mut self, event: &UiEvent) -> EventResponse {
        if !self.visible {
            return EventResponse::Ignored;
        }

        // Enter key: confirm if name matches.
        if let UiEvent::KeyDown { keycode, .. } = event {
            if *keycode == Keycode::Return || *keycode == Keycode::KpEnter {
                if self.name_matches() && !self.is_deleting {
                    self.actions.push(DeleteCharacterDialogAction::Confirm {
                        character_id: self.character_id,
                    });
                }
                return EventResponse::Consumed;
            }
            if *keycode == Keycode::Escape {
                self.actions.push(DeleteCharacterDialogAction::Cancel);
                return EventResponse::Consumed;
            }
        }

        // Forward to buttons.
        if self.confirm_button.handle_event(event) == EventResponse::Consumed {
            if self.name_matches() && !self.is_deleting {
                self.actions.push(DeleteCharacterDialogAction::Confirm {
                    character_id: self.character_id,
                });
            }
            return EventResponse::Consumed;
        }
        if self.cancel_button.handle_event(event) == EventResponse::Consumed {
            self.actions.push(DeleteCharacterDialogAction::Cancel);
            return EventResponse::Consumed;
        }

        // Forward to text input.
        self.name_input.handle_event(event);

        // Consume all events when visible (modal).
        EventResponse::Consumed
    }

    fn update(&mut self, dt: Duration) {
        if self.visible {
            self.name_input.update(dt);
        }
    }

    fn render(&mut self, ctx: &mut RenderContext) -> Result<(), String> {
        if !self.visible {
            return Ok(());
        }

        // Dim overlay.
        let (w, h) = ctx.canvas.output_size()?;
        ctx.canvas.set_blend_mode(BlendMode::Blend);
        ctx.canvas.set_draw_color(Color::RGBA(0, 0, 0, 140));
        ctx.canvas
            .fill_rect(sdl2::rect::Rect::new(0, 0, w, h))?;

        // Dialog background.
        let dialog_rect = sdl2::rect::Rect::new(
            self.bounds.x,
            self.bounds.y,
            self.bounds.width,
            self.bounds.height,
        );
        ctx.canvas.set_draw_color(Color::RGBA(25, 15, 15, 240));
        ctx.canvas.fill_rect(dialog_rect)?;
        ctx.canvas.set_draw_color(Color::RGBA(200, 80, 80, 200));
        ctx.canvas.draw_rect(dialog_rect)?;

        // Title.
        let title = "Delete Character";
        let title_cx = self.bounds.x + self.bounds.width as i32 / 2;
        let title_y = self.bounds.y + 12;
        font_cache::draw_text_centered(ctx.canvas, ctx.gfx, FONT, title, title_cx, title_y)?;

        // Instruction label.
        let instr_y = title_y + font_cache::BITMAP_GLYPH_H as i32 + 10;
        font_cache::draw_text(
            ctx.canvas,
            ctx.gfx,
            FONT,
            "Type the character name to confirm:",
            self.bounds.x + PAD_X,
            instr_y,
        )?;

        // Expected name in red.
        let name_y = instr_y + font_cache::BITMAP_GLYPH_H as i32 + 4;
        font_cache::draw_text_tinted(
            ctx.canvas,
            ctx.gfx,
            FONT,
            &self.expected_name,
            self.bounds.x + PAD_X,
            name_y,
            Color::RGB(255, 100, 100),
        )?;

        // Text input.
        let input_y = name_y + font_cache::BITMAP_GLYPH_H as i32 + 8;
        self.name_input
            .set_position(self.bounds.x + PAD_X, input_y);
        self.name_input.render(ctx)?;

        // Buttons.
        let total_btn_w: i32 = 2 * 140 + BTN_GAP;
        let btn_x = self.bounds.x + (self.bounds.width as i32 - total_btn_w) / 2;
        let btn_y = self.bounds.y + self.bounds.height as i32 - BTN_H as i32 - 30;
        self.confirm_button.set_position(btn_x, btn_y);
        self.cancel_button
            .set_position(btn_x + 140 + BTN_GAP, btn_y);
        self.confirm_button.render(ctx)?;
        self.cancel_button.render(ctx)?;

        // Hint text below buttons.
        let hint_y = btn_y + BTN_H as i32 + 4;
        if !self.name_matches() {
            font_cache::draw_text_tinted(
                ctx.canvas,
                ctx.gfx,
                FONT,
                "Name must match exactly to enable deletion.",
                self.bounds.x + PAD_X,
                hint_y,
                Color::RGB(160, 160, 160),
            )?;
        }

        if self.is_deleting {
            font_cache::draw_text_tinted(
                ctx.canvas,
                ctx.gfx,
                FONT,
                "Deleting character...",
                self.bounds.x + PAD_X,
                hint_y,
                Color::RGB(255, 180, 100),
            )?;
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

    fn make_dialog() -> DeleteCharacterDialog {
        DeleteCharacterDialog::new()
    }

    #[test]
    fn initially_hidden() {
        let dialog = make_dialog();
        assert!(!dialog.is_visible());
    }

    #[test]
    fn show_makes_visible() {
        let mut dialog = make_dialog();
        dialog.show(42, "TestChar");
        assert!(dialog.is_visible());
        assert_eq!(dialog.character_id, 42);
        assert_eq!(dialog.expected_name, "TestChar");
    }

    #[test]
    fn hide_clears_state() {
        let mut dialog = make_dialog();
        dialog.show(42, "TestChar");
        dialog.hide();
        assert!(!dialog.is_visible());
        assert_eq!(dialog.character_id, 0);
    }

    #[test]
    fn enter_with_matching_name_produces_confirm() {
        let mut dialog = make_dialog();
        dialog.show(99, "Hero");
        // Simulate typing "Hero"
        for ch in "Hero".chars() {
            dialog.handle_event(&UiEvent::TextInput {
                text: ch.to_string(),
            });
        }
        dialog.handle_event(&UiEvent::KeyDown {
            keycode: Keycode::Return,
            modifiers: KeyModifiers::default(),
        });
        let actions = dialog.take_actions();
        assert_eq!(actions.len(), 1);
        match &actions[0] {
            DeleteCharacterDialogAction::Confirm { character_id } => {
                assert_eq!(*character_id, 99);
            }
            _ => panic!("Expected Confirm action"),
        }
    }

    #[test]
    fn enter_without_matching_name_does_not_confirm() {
        let mut dialog = make_dialog();
        dialog.show(99, "Hero");
        // Type something wrong
        dialog.handle_event(&UiEvent::TextInput {
            text: "Wrong".to_string(),
        });
        dialog.handle_event(&UiEvent::KeyDown {
            keycode: Keycode::Return,
            modifiers: KeyModifiers::default(),
        });
        let actions = dialog.take_actions();
        assert!(actions.is_empty());
    }

    #[test]
    fn escape_produces_cancel() {
        let mut dialog = make_dialog();
        dialog.show(99, "Hero");
        dialog.handle_event(&UiEvent::KeyDown {
            keycode: Keycode::Escape,
            modifiers: KeyModifiers::default(),
        });
        let actions = dialog.take_actions();
        assert_eq!(actions.len(), 1);
        assert!(matches!(
            &actions[0],
            DeleteCharacterDialogAction::Cancel
        ));
    }

    #[test]
    fn ignored_when_hidden() {
        let mut dialog = make_dialog();
        let resp = dialog.handle_event(&UiEvent::KeyDown {
            keycode: Keycode::Return,
            modifiers: KeyModifiers::default(),
        });
        assert_eq!(resp, EventResponse::Ignored);
        assert!(dialog.take_actions().is_empty());
    }
}
