//! Composite new-account registration form widget.
//!
//! Contains text inputs for e-mail, username and password, plus Create and
//! Cancel buttons.  The owning scene reads pending [`NewAccountFormAction`]s
//! via [`NewAccountForm::take_actions`].

use std::time::Duration;

use sdl2::keyboard::Keycode;
use sdl2::pixels::Color;
use sdl2::render::BlendMode;

use super::RenderContext;
use super::button::RectButton;
use super::style::{Background, Border};
use super::text_input::TextInput;
use super::widget::{Bounds, EventResponse, MouseButton, UiEvent, Widget};
use crate::font_cache;

// ---------------------------------------------------------------------------
// Layout constants
// ---------------------------------------------------------------------------

/// Panel dimensions.
const PANEL_W: u32 = 350;
const PANEL_H: u32 = 346;

/// Horizontal padding inside the panel.
const PAD_X: i32 = 20;

/// Width of the three text input fields.
const INPUT_W: u32 = PANEL_W - (PAD_X as u32) * 2;

/// Height of each text input field.
const INPUT_H: u32 = 16;

/// Vertical gap between a label and the text input beneath it.
const LABEL_INPUT_GAP: i32 = 2;

/// Vertical gap between one field group and the next.
const FIELD_GAP: i32 = 10;

/// Button height.
const BTN_H: u32 = 22;

/// Gap between buttons.
const BTN_GAP: i32 = 6;

/// Bitmap font index.
const FONT: usize = 1;

// ---------------------------------------------------------------------------
// Actions
// ---------------------------------------------------------------------------

/// A side-effect produced by the new-account form for the owning scene.
#[derive(Clone, Debug)]
pub enum NewAccountFormAction {
    /// User pressed Create (or hit Enter).
    Create {
        /// E-mail address.
        email: String,
        /// Account username.
        username: String,
        /// Account password (plain-text; the scene hashes before sending).
        password: String,
    },
    /// User pressed Cancel.
    Cancel,
}

// ---------------------------------------------------------------------------
// Widget
// ---------------------------------------------------------------------------

/// The new-account registration form panel.
pub struct NewAccountForm {
    bounds: Bounds,
    /// E-mail input.
    email_input: TextInput,
    /// Username input.
    username_input: TextInput,
    /// Password input (masked).
    password_input: TextInput,
    /// Confirm-password input (masked).
    confirm_password_input: TextInput,
    /// Create button.
    create_button: RectButton,
    /// Cancel button.
    cancel_button: RectButton,
    /// Index of the currently focused text field (0–3).
    focused_field: usize,
    /// Pending actions for the scene to drain.
    actions: Vec<NewAccountFormAction>,
    /// Whether to show the "Creating account..." status.
    show_submitting: bool,
    /// Optional error message text.
    error_text: Option<String>,
}

impl NewAccountForm {
    /// Creates a new account form, centerd on screen.
    ///
    /// # Returns
    ///
    /// A fully-initialised `NewAccountForm`.
    pub fn new() -> Self {
        let panel_x = (crate::constants::TARGET_WIDTH_INT - PANEL_W) as i32 / 2;
        let panel_y = (crate::constants::TARGET_HEIGHT_INT - PANEL_H) as i32 / 2;

        let bounds = Bounds::new(panel_x, panel_y, PANEL_W, PANEL_H);

        let border_normal = Color::RGBA(100, 100, 140, 200);
        let border_focused = Color::RGBA(180, 180, 255, 255);

        let mut cursor_y = panel_y + 30; // room for title

        // E-mail
        let email_y = cursor_y + font_cache::BITMAP_GLYPH_H as i32 + LABEL_INPUT_GAP;
        let email_input = TextInput::new(
            Bounds::new(panel_x + PAD_X, email_y, INPUT_W, INPUT_H),
            "e-mail address",
            FONT,
            128,
            false,
            border_normal,
            border_focused,
        );
        cursor_y = email_y + INPUT_H as i32 + FIELD_GAP;

        // Username
        let user_y = cursor_y + font_cache::BITMAP_GLYPH_H as i32 + LABEL_INPUT_GAP;
        let username_input = TextInput::new(
            Bounds::new(panel_x + PAD_X, user_y, INPUT_W, INPUT_H),
            "username",
            FONT,
            64,
            false,
            border_normal,
            border_focused,
        );
        cursor_y = user_y + INPUT_H as i32 + FIELD_GAP;

        // Password
        let pw_y = cursor_y + font_cache::BITMAP_GLYPH_H as i32 + LABEL_INPUT_GAP;
        let password_input = TextInput::new(
            Bounds::new(panel_x + PAD_X, pw_y, INPUT_W, INPUT_H),
            "password",
            FONT,
            64,
            true,
            border_normal,
            border_focused,
        );
        cursor_y = pw_y + INPUT_H as i32 + FIELD_GAP;

        // Confirm password
        let confirm_pw_y = cursor_y + font_cache::BITMAP_GLYPH_H as i32 + LABEL_INPUT_GAP;
        let confirm_password_input = TextInput::new(
            Bounds::new(panel_x + PAD_X, confirm_pw_y, INPUT_W, INPUT_H),
            "confirm password",
            FONT,
            64,
            true,
            border_normal,
            border_focused,
        );
        cursor_y = confirm_pw_y + INPUT_H as i32 + FIELD_GAP + 4;

        // Buttons
        let total_btn_w = 2 * 150 + BTN_GAP as u32;
        let btn_start_x = panel_x + (PANEL_W as i32 - total_btn_w as i32) / 2;

        let btn_bg = Background::SolidColor(Color::RGBA(50, 50, 80, 200));
        let btn_border = Border {
            color: Color::RGBA(120, 120, 180, 200),
            width: 1,
        };

        let create_button = RectButton::new(Bounds::new(btn_start_x, cursor_y, 150, BTN_H), btn_bg)
            .with_border(btn_border)
            .with_label("Create", FONT);

        let cancel_button = RectButton::new(
            Bounds::new(btn_start_x + 150 + BTN_GAP, cursor_y, 150, BTN_H),
            btn_bg,
        )
        .with_border(btn_border)
        .with_label("Cancel", FONT);

        let mut form = Self {
            bounds,
            email_input,
            username_input,
            password_input,
            confirm_password_input,
            create_button,
            cancel_button,
            focused_field: 0,
            actions: Vec::new(),
            show_submitting: false,
            error_text: None,
        };
        form.apply_focus();
        form
    }

    /// Returns a reference to the current e-mail value.
    pub fn email(&self) -> &str {
        self.email_input.value()
    }

    /// Returns a reference to the current username value.
    pub fn username(&self) -> &str {
        self.username_input.value()
    }

    /// Returns a reference to the current password value.
    pub fn password(&self) -> &str {
        self.password_input.value()
    }

    /// Returns a reference to the current confirm-password value.
    pub fn confirm_password(&self) -> &str {
        self.confirm_password_input.value()
    }

    /// Sets the "Creating account..." status indicator.
    ///
    /// # Arguments
    ///
    /// * `submitting` - `true` to show, `false` to hide.
    pub fn set_submitting(&mut self, submitting: bool) {
        self.show_submitting = submitting;
        if submitting {
            self.error_text = None;
        }
    }

    /// Sets or clears the error message.
    ///
    /// # Arguments
    ///
    /// * `msg` - Error text, or `None` to clear.
    pub fn set_error(&mut self, msg: Option<String>) {
        self.error_text = msg;
    }

    /// Drains pending [`NewAccountFormAction`]s.
    ///
    /// # Returns
    ///
    /// A vector of actions produced since the last call.
    pub fn take_actions(&mut self) -> Vec<NewAccountFormAction> {
        std::mem::take(&mut self.actions)
    }

    /// Pushes a Create action with the current field values, after validating
    /// that the password and confirm-password fields match.
    fn push_create_action(&mut self) {
        if self.password_input.value() != self.confirm_password_input.value() {
            self.error_text = Some("Passwords do not match".to_string());
            return;
        }
        self.error_text = None;
        self.actions.push(NewAccountFormAction::Create {
            email: self.email_input.value().to_owned(),
            username: self.username_input.value().to_owned(),
            password: self.password_input.value().to_owned(),
        });
    }

    /// Advances keyboard focus to the next text field.
    fn cycle_focus_forward(&mut self) {
        self.focused_field = (self.focused_field + 1) % 4;
        self.apply_focus();
    }

    /// Moves keyboard focus to the previous text field.
    fn cycle_focus_backward(&mut self) {
        self.focused_field = if self.focused_field == 0 {
            3
        } else {
            self.focused_field - 1
        };
        self.apply_focus();
    }

    /// Synchronises `set_focused` on all text inputs.
    fn apply_focus(&mut self) {
        self.email_input.set_focused(self.focused_field == 0);
        self.username_input.set_focused(self.focused_field == 1);
        self.password_input.set_focused(self.focused_field == 2);
        self.confirm_password_input
            .set_focused(self.focused_field == 3);
    }

    /// Returns the field index (0–3) that contains the given point, if any.
    fn field_index_at(&self, x: i32, y: i32) -> Option<usize> {
        if self.email_input.bounds().contains_point(x, y) {
            Some(0)
        } else if self.username_input.bounds().contains_point(x, y) {
            Some(1)
        } else if self.password_input.bounds().contains_point(x, y) {
            Some(2)
        } else if self.confirm_password_input.bounds().contains_point(x, y) {
            Some(3)
        } else {
            None
        }
    }
}

impl Widget for NewAccountForm {
    fn bounds(&self) -> &Bounds {
        &self.bounds
    }

    fn set_position(&mut self, _x: i32, _y: i32) {
        // Fixed layout — repositioning not supported.
    }

    fn handle_event(&mut self, event: &UiEvent) -> EventResponse {
        // Tab / Enter key handling.
        if let UiEvent::KeyDown {
            keycode, modifiers, ..
        } = event
        {
            match *keycode {
                Keycode::Tab => {
                    if modifiers.shift {
                        self.cycle_focus_backward();
                    } else {
                        self.cycle_focus_forward();
                    }
                    return EventResponse::Consumed;
                }
                Keycode::Return | Keycode::KpEnter => {
                    self.push_create_action();
                    return EventResponse::Consumed;
                }
                _ => {}
            }
        }

        // Mouse click: detect field focus.
        if let UiEvent::MouseClick {
            x,
            y,
            button: MouseButton::Left,
            ..
        } = event
        {
            if let Some(idx) = self.field_index_at(*x, *y) {
                self.focused_field = idx;
                self.apply_focus();
            }
        }

        // Forward to buttons.
        if self.create_button.handle_event(event) == EventResponse::Consumed {
            self.push_create_action();
            return EventResponse::Consumed;
        }
        if self.cancel_button.handle_event(event) == EventResponse::Consumed {
            self.actions.push(NewAccountFormAction::Cancel);
            return EventResponse::Consumed;
        }

        // Forward to text inputs.
        self.email_input.handle_event(event);
        self.username_input.handle_event(event);
        self.password_input.handle_event(event);
        self.confirm_password_input.handle_event(event);

        // Consume if inside panel.
        if let UiEvent::MouseClick { x, y, .. } | UiEvent::MouseDown { x, y, .. } = event {
            if self.bounds.contains_point(*x, *y) {
                return EventResponse::Consumed;
            }
        }

        match event {
            UiEvent::TextInput { .. } | UiEvent::KeyDown { .. } => EventResponse::Consumed,
            _ => EventResponse::Ignored,
        }
    }

    fn update(&mut self, dt: Duration) {
        self.email_input.update(dt);
        self.username_input.update(dt);
        self.password_input.update(dt);
        self.confirm_password_input.update(dt);
    }

    fn render(&mut self, ctx: &mut RenderContext<'_, '_>) -> Result<(), String> {
        // Panel background.
        let panel_rect = sdl2::rect::Rect::new(
            self.bounds.x,
            self.bounds.y,
            self.bounds.width,
            self.bounds.height,
        );
        ctx.canvas.set_blend_mode(BlendMode::Blend);
        ctx.canvas.set_draw_color(Color::RGBA(15, 15, 30, 210));
        ctx.canvas.fill_rect(panel_rect)?;
        ctx.canvas.set_draw_color(Color::RGBA(100, 100, 160, 200));
        ctx.canvas.draw_rect(panel_rect)?;

        // Title.
        let title = "Create Account";
        let title_cx = self.bounds.x + self.bounds.width as i32 / 2;
        let title_y = self.bounds.y + 10;
        font_cache::draw_text(
            ctx.canvas,
            ctx.gfx,
            FONT,
            title,
            title_cx,
            title_y,
            font_cache::TextStyle::centered(),
        )?;

        let mut cursor_y = title_y + font_cache::BITMAP_GLYPH_H as i32 + 8;

        // E-mail field.
        font_cache::draw_text(
            ctx.canvas,
            ctx.gfx,
            FONT,
            "E-mail",
            self.bounds.x + PAD_X,
            cursor_y,
            font_cache::TextStyle::PLAIN,
        )?;
        cursor_y += font_cache::BITMAP_GLYPH_H as i32 + LABEL_INPUT_GAP;
        self.email_input
            .set_position(self.bounds.x + PAD_X, cursor_y);
        self.email_input.render(ctx)?;
        cursor_y += INPUT_H as i32 + FIELD_GAP;

        // Username field.
        font_cache::draw_text(
            ctx.canvas,
            ctx.gfx,
            FONT,
            "Username",
            self.bounds.x + PAD_X,
            cursor_y,
            font_cache::TextStyle::PLAIN,
        )?;
        cursor_y += font_cache::BITMAP_GLYPH_H as i32 + LABEL_INPUT_GAP;
        self.username_input
            .set_position(self.bounds.x + PAD_X, cursor_y);
        self.username_input.render(ctx)?;
        cursor_y += INPUT_H as i32 + FIELD_GAP;

        // Password field.
        font_cache::draw_text(
            ctx.canvas,
            ctx.gfx,
            FONT,
            "Password",
            self.bounds.x + PAD_X,
            cursor_y,
            font_cache::TextStyle::PLAIN,
        )?;
        cursor_y += font_cache::BITMAP_GLYPH_H as i32 + LABEL_INPUT_GAP;
        self.password_input
            .set_position(self.bounds.x + PAD_X, cursor_y);
        self.password_input.render(ctx)?;
        cursor_y += INPUT_H as i32 + FIELD_GAP;

        // Confirm password field.
        font_cache::draw_text(
            ctx.canvas,
            ctx.gfx,
            FONT,
            "Confirm Password",
            self.bounds.x + PAD_X,
            cursor_y,
            font_cache::TextStyle::PLAIN,
        )?;
        cursor_y += font_cache::BITMAP_GLYPH_H as i32 + LABEL_INPUT_GAP;
        self.confirm_password_input
            .set_position(self.bounds.x + PAD_X, cursor_y);
        self.confirm_password_input.render(ctx)?;
        cursor_y += INPUT_H as i32 + FIELD_GAP + 4;

        // Buttons.
        let total_btn_w: i32 = 2 * 150 + BTN_GAP;
        let btn_x = self.bounds.x + (self.bounds.width as i32 - total_btn_w) / 2;
        self.create_button.set_position(btn_x, cursor_y);
        self.cancel_button
            .set_position(btn_x + 150 + BTN_GAP, cursor_y);
        self.create_button.render(ctx)?;
        self.cancel_button.render(ctx)?;
        cursor_y += BTN_H as i32 + 8;

        // Status / error labels.
        if self.show_submitting {
            font_cache::draw_text(
                ctx.canvas,
                ctx.gfx,
                FONT,
                "Creating account...",
                self.bounds.x + PAD_X,
                cursor_y,
                font_cache::TextStyle::tinted(Color::RGB(180, 180, 255)),
            )?;
        }

        if let Some(ref err) = self.error_text {
            font_cache::draw_text(
                ctx.canvas,
                ctx.gfx,
                FONT,
                err,
                self.bounds.x + PAD_X,
                cursor_y,
                font_cache::TextStyle::tinted(Color::RGB(255, 80, 80)),
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

    fn make_form() -> NewAccountForm {
        NewAccountForm::new()
    }

    #[test]
    fn initial_values_empty() {
        let form = make_form();
        assert_eq!(form.email(), "");
        assert_eq!(form.username(), "");
        assert_eq!(form.password(), "");
        assert_eq!(form.confirm_password(), "");
    }

    #[test]
    fn tab_cycles_focus() {
        let mut form = make_form();
        assert_eq!(form.focused_field, 0);
        form.handle_event(&UiEvent::KeyDown {
            keycode: Keycode::Tab,
            modifiers: KeyModifiers::default(),
        });
        assert_eq!(form.focused_field, 1);
        form.handle_event(&UiEvent::KeyDown {
            keycode: Keycode::Tab,
            modifiers: KeyModifiers::default(),
        });
        assert_eq!(form.focused_field, 2);
        form.handle_event(&UiEvent::KeyDown {
            keycode: Keycode::Tab,
            modifiers: KeyModifiers::default(),
        });
        assert_eq!(form.focused_field, 3);
        form.handle_event(&UiEvent::KeyDown {
            keycode: Keycode::Tab,
            modifiers: KeyModifiers::default(),
        });
        assert_eq!(form.focused_field, 0);
    }

    #[test]
    fn enter_pushes_create_action() {
        let mut form = make_form();
        form.handle_event(&UiEvent::KeyDown {
            keycode: Keycode::Return,
            modifiers: KeyModifiers::default(),
        });
        let actions = form.take_actions();
        assert_eq!(actions.len(), 1);
        assert!(matches!(actions[0], NewAccountFormAction::Create { .. }));
    }

    #[test]
    fn set_error_shown_and_cleared() {
        let mut form = make_form();
        form.set_error(Some("bad input".to_string()));
        assert!(form.error_text.is_some());
        form.set_error(None);
        assert!(form.error_text.is_none());
    }
}
