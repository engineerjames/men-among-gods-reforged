//! Modal quit-confirmation dialog.
//!
//! Presents a simple "Are you sure you want to quit?" prompt with
//! **Quit** and **Cancel** buttons.  The owning widget or scene reads
//! pending [`QuitConfirmDialogAction`]s via
//! [`QuitConfirmDialog::take_actions`].

use sdl2::keyboard::Keycode;
use sdl2::pixels::Color;
use sdl2::render::BlendMode;

use super::button::RectButton;
use super::style::{Background, Border};
use super::title_bar::{TitleBar, TITLE_BAR_H};
use super::widget::{Bounds, EventResponse, UiEvent, Widget};
use super::RenderContext;
use crate::font_cache;

// ---------------------------------------------------------------------------
// Layout constants
// ---------------------------------------------------------------------------

/// Dialog width in pixels.
const DIALOG_W: u32 = 300;

/// Dialog height in pixels (includes title bar).
const DIALOG_H: u32 = 100 + TITLE_BAR_H as u32;

/// Horizontal padding inside the dialog.
const PAD_X: i32 = 20;

/// Button height in pixels.
const BTN_H: u32 = 22;

/// Gap between the two buttons in pixels.
const BTN_GAP: i32 = 8;

/// Bitmap font index used throughout the dialog.
const FONT: usize = 1;

// ---------------------------------------------------------------------------
// Actions
// ---------------------------------------------------------------------------

/// A side-effect produced by the quit-confirmation dialog.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum QuitConfirmDialogAction {
    /// User confirmed they want to quit.
    Confirm,
    /// User cancelled and does not want to quit.
    Cancel,
}

// ---------------------------------------------------------------------------
// Widget
// ---------------------------------------------------------------------------

/// Modal dialog that asks the user to confirm quitting the application.
pub struct QuitConfirmDialog {
    bounds: Bounds,
    /// Whether the dialog is currently visible.
    visible: bool,
    /// Non-movable title bar (close button only).
    title_bar: TitleBar,
    /// Confirm-quit button (red styling).
    confirm_button: RectButton,
    /// Cancel button (neutral styling).
    cancel_button: RectButton,
    /// Pending actions for the owner to drain.
    actions: Vec<QuitConfirmDialogAction>,
}

impl QuitConfirmDialog {
    /// Creates a new, initially hidden quit-confirmation dialog centerd on
    /// screen.
    ///
    /// # Returns
    ///
    /// A fully-initialised `QuitConfirmDialog`.
    pub fn new() -> Self {
        let panel_x = (crate::constants::TARGET_WIDTH_INT - DIALOG_W) as i32 / 2;
        let panel_y = (crate::constants::TARGET_HEIGHT_INT - DIALOG_H) as i32 / 2;
        let bounds = Bounds::new(panel_x, panel_y, DIALOG_W, DIALOG_H);

        let confirm_bg = Background::SolidColor(Color::RGBA(80, 30, 30, 220));
        let confirm_border = Border {
            color: Color::RGBA(200, 80, 80, 220),
            width: 1,
        };

        let cancel_bg = Background::SolidColor(Color::RGBA(50, 50, 80, 200));
        let cancel_border = Border {
            color: Color::RGBA(120, 120, 180, 200),
            width: 1,
        };

        let btn_w = 120u32;
        let total_btn_w = btn_w * 2 + BTN_GAP as u32;
        let btn_x = panel_x + (DIALOG_W as i32 - total_btn_w as i32) / 2;
        let btn_y = panel_y + DIALOG_H as i32 - BTN_H as i32 - 14;

        let confirm_button = RectButton::new(Bounds::new(btn_x, btn_y, btn_w, BTN_H), confirm_bg)
            .with_border(confirm_border)
            .with_label("Quit", FONT);

        let cancel_button = RectButton::new(
            Bounds::new(btn_x + btn_w as i32 + BTN_GAP, btn_y, btn_w, BTN_H),
            cancel_bg,
        )
        .with_border(cancel_border)
        .with_label("Cancel", FONT);

        Self {
            bounds,
            visible: false,
            title_bar: TitleBar::new_static("Quit?", panel_x, panel_y, DIALOG_W),
            confirm_button,
            cancel_button,
            actions: Vec::new(),
        }
    }

    /// Shows the dialog.
    pub fn show(&mut self) {
        self.visible = true;
    }

    /// Hides the dialog and clears any pending actions.
    pub fn hide(&mut self) {
        self.visible = false;
        self.actions.clear();
    }

    /// Returns `true` if the dialog is currently visible.
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Drains pending [`QuitConfirmDialogAction`]s.
    ///
    /// # Returns
    ///
    /// A vector of actions produced since the last call.
    pub fn take_actions(&mut self) -> Vec<QuitConfirmDialogAction> {
        std::mem::take(&mut self.actions)
    }
}

impl Widget for QuitConfirmDialog {
    fn bounds(&self) -> &Bounds {
        &self.bounds
    }

    fn set_position(&mut self, _x: i32, _y: i32) {
        // Fixed center position — repositioning not supported.
    }

    fn handle_event(&mut self, event: &UiEvent) -> EventResponse {
        if !self.visible {
            return EventResponse::Ignored;
        }

        // Title bar close button acts as Cancel.
        let (tb_resp, _) = self.title_bar.handle_event(event);
        if self.title_bar.was_close_requested() {
            self.actions.push(QuitConfirmDialogAction::Cancel);
            return EventResponse::Consumed;
        }
        if tb_resp == EventResponse::Consumed {
            return EventResponse::Consumed;
        }

        // Escape cancels the dialog.
        if let UiEvent::KeyDown { keycode, .. } = event {
            match *keycode {
                Keycode::Escape => {
                    self.actions.push(QuitConfirmDialogAction::Cancel);
                    return EventResponse::Consumed;
                }
                Keycode::Return | Keycode::KpEnter => {
                    self.actions.push(QuitConfirmDialogAction::Confirm);
                    return EventResponse::Consumed;
                }
                _ => {}
            }
        }

        if self.confirm_button.handle_event(event) == EventResponse::Consumed {
            self.actions.push(QuitConfirmDialogAction::Confirm);
            return EventResponse::Consumed;
        }

        if self.cancel_button.handle_event(event) == EventResponse::Consumed {
            self.actions.push(QuitConfirmDialogAction::Cancel);
            return EventResponse::Consumed;
        }

        // Consume all events while visible (modal behaviour).
        EventResponse::Consumed
    }

    fn render(&mut self, ctx: &mut RenderContext<'_, '_>) -> Result<(), String> {
        if !self.visible {
            return Ok(());
        }

        // Dim overlay covering the whole viewport.
        let (w, h) = ctx.canvas.output_size()?;
        ctx.canvas.set_blend_mode(BlendMode::Blend);
        ctx.canvas.set_draw_color(Color::RGBA(0, 0, 0, 160));
        ctx.canvas.fill_rect(sdl2::rect::Rect::new(0, 0, w, h))?;

        // Dialog background.
        let dialog_rect = sdl2::rect::Rect::new(
            self.bounds.x,
            self.bounds.y,
            self.bounds.width,
            self.bounds.height,
        );
        ctx.canvas.set_draw_color(Color::RGBA(20, 15, 25, 245));
        ctx.canvas.fill_rect(dialog_rect)?;
        ctx.canvas.set_draw_color(Color::RGBA(180, 80, 80, 220));
        ctx.canvas.draw_rect(dialog_rect)?;

        // Title bar.
        self.title_bar.render(ctx)?;

        // Confirmation message.
        font_cache::draw_text(
            ctx.canvas,
            ctx.gfx,
            FONT,
            "Are you sure you want to quit?",
            self.bounds.x + PAD_X,
            self.bounds.y + TITLE_BAR_H + 12,
            font_cache::TextStyle::PLAIN,
        )?;

        // Buttons.
        self.confirm_button.render(ctx)?;
        self.cancel_button.render(ctx)?;

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_dialog() -> QuitConfirmDialog {
        QuitConfirmDialog::new()
    }

    #[test]
    fn initially_hidden() {
        let dialog = make_dialog();
        assert!(!dialog.is_visible());
    }

    #[test]
    fn show_makes_visible() {
        let mut dialog = make_dialog();
        dialog.show();
        assert!(dialog.is_visible());
    }

    #[test]
    fn hide_makes_invisible() {
        let mut dialog = make_dialog();
        dialog.show();
        dialog.hide();
        assert!(!dialog.is_visible());
    }

    #[test]
    fn escape_key_pushes_cancel() {
        use crate::ui::widget::KeyModifiers;
        let mut dialog = make_dialog();
        dialog.show();
        let resp = dialog.handle_event(&UiEvent::KeyDown {
            keycode: Keycode::Escape,
            modifiers: KeyModifiers::default(),
        });
        assert_eq!(resp, EventResponse::Consumed);
        let actions = dialog.take_actions();
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0], QuitConfirmDialogAction::Cancel);
    }

    #[test]
    fn enter_key_pushes_confirm() {
        use crate::ui::widget::KeyModifiers;
        let mut dialog = make_dialog();
        dialog.show();
        let resp = dialog.handle_event(&UiEvent::KeyDown {
            keycode: Keycode::Return,
            modifiers: KeyModifiers::default(),
        });
        assert_eq!(resp, EventResponse::Consumed);
        let actions = dialog.take_actions();
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0], QuitConfirmDialogAction::Confirm);
    }

    #[test]
    fn hidden_dialog_ignores_key() {
        use crate::ui::widget::KeyModifiers;
        let mut dialog = make_dialog();
        let resp = dialog.handle_event(&UiEvent::KeyDown {
            keycode: Keycode::Return,
            modifiers: KeyModifiers::default(),
        });
        assert_eq!(resp, EventResponse::Ignored);
    }

    #[test]
    fn take_actions_drains_buffer() {
        use crate::ui::widget::KeyModifiers;
        let mut dialog = make_dialog();
        dialog.show();
        dialog.handle_event(&UiEvent::KeyDown {
            keycode: Keycode::Escape,
            modifiers: KeyModifiers::default(),
        });
        assert_eq!(dialog.take_actions().len(), 1);
        // Second drain must be empty.
        assert!(dialog.take_actions().is_empty());
    }
}
