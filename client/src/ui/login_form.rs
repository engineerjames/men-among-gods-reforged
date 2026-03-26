//! Composite login form widget.
//!
//! Contains text inputs for server IP, username and password, a music
//! checkbox, Login / Create-Account / Quit buttons, and optional status and
//! error labels.  The owning scene reads pending [`LoginFormAction`]s via
//! [`LoginForm::take_login_actions`].

use std::time::Duration;

use sdl2::keyboard::Keycode;
use sdl2::pixels::Color;
use sdl2::render::BlendMode;

use super::RenderContext;
use super::button::RectButton;
use super::checkbox::Checkbox;
use super::style::{Background, Border};
use super::text_input::TextInput;
use super::widget::{Bounds, EventResponse, MouseButton, UiEvent, Widget};
use crate::font_cache;

// ---------------------------------------------------------------------------
// Layout constants
// ---------------------------------------------------------------------------

/// Panel dimensions.
const PANEL_W: u32 = 350;
const PANEL_H: u32 = 310;

/// Horizontal padding inside the panel.
const PAD_X: i32 = 20;

/// Width of the three text input fields.
const INPUT_W: u32 = PANEL_W - (PAD_X as u32) * 2;

/// Height of each text input field.
const INPUT_H: u32 = 16;

/// Vertical gap between a label and the text input beneath it.
const LABEL_INPUT_GAP: i32 = 2;

/// Vertical gap between one field group (label+input) and the next.
const FIELD_GAP: i32 = 10;

/// Button height.
const BTN_H: u32 = 22;

/// Gap between buttons in the horizontal row.
const BTN_GAP: i32 = 6;

/// Bitmap font index used throughout the form.
const FONT: usize = 1;

// ---------------------------------------------------------------------------
// Actions
// ---------------------------------------------------------------------------

/// A side-effect produced by the login form for the owning scene to handle.
#[derive(Clone, Debug)]
pub enum LoginFormAction {
    /// User pressed Login (or hit Enter in one of the text fields).
    Login {
        /// Server IP / hostname entered by the user.
        ip: String,
        /// Account username.
        username: String,
        /// Account password (plain-text; the scene hashes before sending).
        password: String,
    },
    /// User pressed the Create Account button.
    CreateAccount,
    /// User pressed the Reset Password button.
    ResetPassword,
    /// User pressed the Quit button.
    Quit,
    /// Music checkbox was toggled.
    ToggleMusic(bool),
}

// ---------------------------------------------------------------------------
// Widget
// ---------------------------------------------------------------------------

/// The login form panel containing all interactive elements.
pub struct LoginForm {
    bounds: Bounds,
    /// IP address / hostname input.
    ip_input: TextInput,
    /// Username input.
    username_input: TextInput,
    /// Password input (masked).
    password_input: TextInput,
    /// Music toggle checkbox.
    music_checkbox: Checkbox,
    /// Login button.
    login_button: RectButton,
    /// Create-account button.
    create_button: RectButton,
    /// Reset-password button.
    reset_button: RectButton,
    /// Quit button.
    quit_button: RectButton,
    /// Index of the currently focused text field (0–2).
    focused_field: usize,
    /// Pending actions for the scene to drain.
    actions: Vec<LoginFormAction>,
    /// Whether to show the "Logging in..." status.
    show_submitting: bool,
    /// Optional error message text.
    error_text: Option<String>,
    /// Whether to show the unencrypted-connection warning banner.
    show_unencrypted_warning: bool,
}

impl LoginForm {
    /// Creates a new login form, centerd on screen.
    ///
    /// # Arguments
    ///
    /// * `server_ip` - Initial server IP / hostname value.
    /// * `username` - Initial username value (e.g. from saved preferences).
    /// * `music_enabled` - Initial state of the music checkbox.
    ///
    /// # Returns
    ///
    /// A fully-initialised `LoginForm`.
    pub fn new(server_ip: &str, username: &str, music_enabled: bool) -> Self {
        let panel_x = (crate::constants::TARGET_WIDTH_INT - PANEL_W) as i32 / 2;
        let panel_y = (crate::constants::TARGET_HEIGHT_INT - PANEL_H) as i32 / 2;

        let bounds = Bounds::new(panel_x, panel_y, PANEL_W, PANEL_H);

        let border_normal = Color::RGBA(100, 100, 140, 200);
        let border_focused = Color::RGBA(180, 180, 255, 255);

        // -- Text inputs --
        let mut cursor_y = panel_y + 30; // room for title

        // IP
        let ip_input_y = cursor_y + font_cache::BITMAP_GLYPH_H as i32 + LABEL_INPUT_GAP;
        let mut ip_input = TextInput::new(
            Bounds::new(panel_x + PAD_X, ip_input_y, INPUT_W, INPUT_H),
            "e.g. 127.0.0.1",
            FONT,
            128,
            false,
            border_normal,
            border_focused,
        );
        ip_input.set_value(server_ip);
        ip_input.set_focused(true);
        cursor_y = ip_input_y + INPUT_H as i32 + FIELD_GAP;

        // Username
        let user_input_y = cursor_y + font_cache::BITMAP_GLYPH_H as i32 + LABEL_INPUT_GAP;
        let mut username_input = TextInput::new(
            Bounds::new(panel_x + PAD_X, user_input_y, INPUT_W, INPUT_H),
            "username",
            FONT,
            64,
            false,
            border_normal,
            border_focused,
        );
        username_input.set_value(username);
        cursor_y = user_input_y + INPUT_H as i32 + FIELD_GAP;

        // Password
        let pw_input_y = cursor_y + font_cache::BITMAP_GLYPH_H as i32 + LABEL_INPUT_GAP;
        let password_input = TextInput::new(
            Bounds::new(panel_x + PAD_X, pw_input_y, INPUT_W, INPUT_H),
            "password",
            FONT,
            64,
            true,
            border_normal,
            border_focused,
        );
        cursor_y = pw_input_y + INPUT_H as i32 + FIELD_GAP;

        // Music checkbox
        let cb_w = font_cache::text_width("Enable Login Music") + 16;
        let music_checkbox_bounds = Bounds::new(panel_x + PAD_X, cursor_y, cb_w, 14);
        let mut music_checkbox = Checkbox::new(music_checkbox_bounds, "Enable Login Music", FONT);
        music_checkbox.set_checked(music_enabled);
        cursor_y += 14 + FIELD_GAP + 4;

        // Buttons - laid out horizontally, evenly
        let total_btn_w = 4 * 75 + 3 * BTN_GAP as u32;
        let btn_start_x = panel_x + (PANEL_W as i32 - total_btn_w as i32) / 2;

        let btn_bg = Background::SolidColor(Color::RGBA(50, 50, 80, 200));
        let btn_border = Border {
            color: Color::RGBA(120, 120, 180, 200),
            width: 1,
        };

        let login_button = RectButton::new(Bounds::new(btn_start_x, cursor_y, 75, BTN_H), btn_bg)
            .with_border(btn_border)
            .with_label("Login", FONT);

        let create_button = RectButton::new(
            Bounds::new(btn_start_x + 75 + BTN_GAP, cursor_y, 75, BTN_H),
            btn_bg,
        )
        .with_border(btn_border)
        .with_label("New Acct", FONT);

        let reset_button = RectButton::new(
            Bounds::new(btn_start_x + 150 + BTN_GAP * 2, cursor_y, 75, BTN_H),
            btn_bg,
        )
        .with_border(btn_border)
        .with_label("Reset Pass", FONT);

        let quit_button = RectButton::new(
            Bounds::new(btn_start_x + 225 + BTN_GAP * 3, cursor_y, 75, BTN_H),
            btn_bg,
        )
        .with_border(btn_border)
        .with_label("Quit", FONT);

        Self {
            bounds,
            ip_input,
            username_input,
            password_input,
            music_checkbox,
            login_button,
            create_button,
            reset_button,
            quit_button,
            focused_field: 0,
            actions: Vec::new(),
            show_submitting: false,
            error_text: None,
            show_unencrypted_warning: false,
        }
    }

    /// Returns a reference to the current server-IP value.
    pub fn server_ip(&self) -> &str {
        self.ip_input.value()
    }

    /// Returns a reference to the current username value.
    pub fn username(&self) -> &str {
        self.username_input.value()
    }

    /// Returns a reference to the current password value.
    pub fn password(&self) -> &str {
        self.password_input.value()
    }

    /// Sets the "Logging in..." status indicator.
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

    /// Sets or clears the unencrypted-connection warning banner.
    ///
    /// # Arguments
    ///
    /// * `show` - Whether to show the warning.
    pub fn set_unencrypted_warning(&mut self, show: bool) {
        self.show_unencrypted_warning = show;
    }

    /// Drains pending [`LoginFormAction`]s.
    ///
    /// # Returns
    ///
    /// A vector of actions produced since the last call.
    pub fn take_login_actions(&mut self) -> Vec<LoginFormAction> {
        std::mem::take(&mut self.actions)
    }

    /// Pushes a Login action with the current field values.
    fn push_login_action(&mut self) {
        self.actions.push(LoginFormAction::Login {
            ip: self.ip_input.value().to_owned(),
            username: self.username_input.value().to_owned(),
            password: self.password_input.value().to_owned(),
        });
    }

    /// Advances keyboard focus to the next text field (Tab cycling).
    fn cycle_focus_forward(&mut self) {
        self.focused_field = (self.focused_field + 1) % 3;
        self.apply_focus();
    }

    /// Moves keyboard focus to the previous text field (Shift+Tab).
    fn cycle_focus_backward(&mut self) {
        self.focused_field = if self.focused_field == 0 {
            2
        } else {
            self.focused_field - 1
        };
        self.apply_focus();
    }

    /// Synchronises `set_focused` on all three text inputs based on
    /// `self.focused_field`.
    fn apply_focus(&mut self) {
        self.ip_input.set_focused(self.focused_field == 0);
        self.username_input.set_focused(self.focused_field == 1);
        self.password_input.set_focused(self.focused_field == 2);
    }

    /// Returns the field index (0-2) that contains the given point, if any.
    fn field_index_at(&self, x: i32, y: i32) -> Option<usize> {
        if self.ip_input.bounds().contains_point(x, y) {
            Some(0)
        } else if self.username_input.bounds().contains_point(x, y) {
            Some(1)
        } else if self.password_input.bounds().contains_point(x, y) {
            Some(2)
        } else {
            None
        }
    }
}

impl Widget for LoginForm {
    fn bounds(&self) -> &Bounds {
        &self.bounds
    }

    fn set_position(&mut self, _x: i32, _y: i32) {
        // Fixed layout — repositioning not supported.
    }

    fn handle_event(&mut self, event: &UiEvent) -> EventResponse {
        // ── Tab / Enter key handling ──────────────────────────────────────
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
                    self.push_login_action();
                    return EventResponse::Consumed;
                }
                _ => {}
            }
        }

        // ── Mouse click: detect which field was clicked for focus ─────────
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

        // ── Forward to interactive children ──────────────────────────────
        // Buttons first (highest priority for clicks).
        let login_resp = self.login_button.handle_event(event);
        if login_resp == EventResponse::Consumed {
            self.push_login_action();
            return EventResponse::Consumed;
        }

        let create_resp = self.create_button.handle_event(event);
        if create_resp == EventResponse::Consumed {
            self.actions.push(LoginFormAction::CreateAccount);
            return EventResponse::Consumed;
        }

        let reset_resp = self.reset_button.handle_event(event);
        if reset_resp == EventResponse::Consumed {
            self.actions.push(LoginFormAction::ResetPassword);
            return EventResponse::Consumed;
        }

        let quit_resp = self.quit_button.handle_event(event);
        if quit_resp == EventResponse::Consumed {
            self.actions.push(LoginFormAction::Quit);
            return EventResponse::Consumed;
        }

        // Checkbox
        let _cb_resp = self.music_checkbox.handle_event(event);
        if self.music_checkbox.was_toggled() {
            self.actions.push(LoginFormAction::ToggleMusic(
                self.music_checkbox.is_checked(),
            ));
        }

        // Text inputs
        self.ip_input.handle_event(event);
        self.username_input.handle_event(event);
        self.password_input.handle_event(event);

        // Consume the event if it landed inside the panel so it does not
        // propagate to the background.
        if let UiEvent::MouseClick { x, y, .. } | UiEvent::MouseDown { x, y, .. } = event {
            if self.bounds.contains_point(*x, *y) {
                return EventResponse::Consumed;
            }
        }

        // Text and key events are consumed when any field is focused.
        match event {
            UiEvent::TextInput { .. } | UiEvent::KeyDown { .. } => EventResponse::Consumed,
            _ => EventResponse::Ignored,
        }
    }

    fn update(&mut self, dt: Duration) {
        self.ip_input.update(dt);
        self.username_input.update(dt);
        self.password_input.update(dt);
    }

    fn render(&mut self, ctx: &mut RenderContext<'_, '_>) -> Result<(), String> {
        // ── Panel background ─────────────────────────────────────────────
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

        // ── Title ───────────────────────────────────────────────────────
        let title = "Men Among Gods - Reforged";
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

        // ── Unencrypted warning ──────────────────────────────────────────
        if self.show_unencrypted_warning {
            let warn_rect = sdl2::rect::Rect::new(
                self.bounds.x + PAD_X,
                cursor_y,
                INPUT_W,
                font_cache::BITMAP_GLYPH_H + 6,
            );
            ctx.canvas.set_draw_color(Color::RGBA(60, 50, 0, 220));
            ctx.canvas.fill_rect(warn_rect)?;
            font_cache::draw_text(
                ctx.canvas,
                ctx.gfx,
                FONT,
                "Warning: connection is not encrypted!",
                self.bounds.x + PAD_X + 3,
                cursor_y + 3,
                font_cache::TextStyle::tinted(Color::RGB(255, 255, 80)),
            )?;
            cursor_y += font_cache::BITMAP_GLYPH_H as i32 + 10;
        }

        // ── IP field ─────────────────────────────────────────────────────
        font_cache::draw_text(
            ctx.canvas,
            ctx.gfx,
            FONT,
            "Server Address",
            self.bounds.x + PAD_X,
            cursor_y,
            font_cache::TextStyle::PLAIN,
        )?;
        cursor_y += font_cache::BITMAP_GLYPH_H as i32 + LABEL_INPUT_GAP;
        // Reposition input to follow dynamic layout (warning may shift things).
        self.ip_input.set_position(self.bounds.x + PAD_X, cursor_y);
        self.ip_input.render(ctx)?;
        cursor_y += INPUT_H as i32 + FIELD_GAP;

        // ── Username field ───────────────────────────────────────────────
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

        // ── Password field ───────────────────────────────────────────────
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

        // ── Music checkbox ───────────────────────────────────────────────
        self.music_checkbox
            .set_position(self.bounds.x + PAD_X, cursor_y);
        self.music_checkbox.render(ctx)?;
        cursor_y += 14 + FIELD_GAP + 2;

        // ── Buttons ──────────────────────────────────────────────────────
        let total_btn_w: i32 = 4 * 75 + 3 * BTN_GAP;
        let btn_x = self.bounds.x + (self.bounds.width as i32 - total_btn_w) / 2;
        self.login_button.set_position(btn_x, cursor_y);
        self.create_button
            .set_position(btn_x + 75 + BTN_GAP, cursor_y);
        self.reset_button
            .set_position(btn_x + 150 + BTN_GAP * 2, cursor_y);
        self.quit_button
            .set_position(btn_x + 225 + BTN_GAP * 3, cursor_y);

        self.login_button.render(ctx)?;
        self.create_button.render(ctx)?;
        self.reset_button.render(ctx)?;
        self.quit_button.render(ctx)?;
        cursor_y += BTN_H as i32 + 8;

        // ── Status / error labels ────────────────────────────────────────
        if self.show_submitting {
            font_cache::draw_text(
                ctx.canvas,
                ctx.gfx,
                FONT,
                "Logging in...",
                self.bounds.x + PAD_X,
                cursor_y,
                font_cache::TextStyle::tinted(Color::RGB(180, 180, 255)),
            )?;
        }

        if let Some(ref err) = self.error_text {
            font_cache::draw_wrapped_text(
                ctx.canvas,
                ctx.gfx,
                FONT,
                err,
                self.bounds.x + PAD_X,
                cursor_y,
                INPUT_W,
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

    fn make_form() -> LoginForm {
        LoginForm::new("127.0.0.1", "testuser", false)
    }

    #[test]
    fn initial_values() {
        let form = make_form();
        assert_eq!(form.server_ip(), "127.0.0.1");
        assert_eq!(form.username(), "testuser");
        assert_eq!(form.password(), "");
        assert!(!form.music_checkbox.is_checked());
    }

    #[test]
    fn tab_cycles_focus_forward() {
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
        assert_eq!(form.focused_field, 0);
    }

    #[test]
    fn shift_tab_cycles_focus_backward() {
        let mut form = make_form();
        form.handle_event(&UiEvent::KeyDown {
            keycode: Keycode::Tab,
            modifiers: KeyModifiers {
                shift: true,
                ..Default::default()
            },
        });
        assert_eq!(form.focused_field, 2);
    }

    #[test]
    fn enter_pushes_login_action() {
        let mut form = make_form();
        form.handle_event(&UiEvent::KeyDown {
            keycode: Keycode::Return,
            modifiers: KeyModifiers::default(),
        });
        let actions = form.take_login_actions();
        assert_eq!(actions.len(), 1);
        match &actions[0] {
            LoginFormAction::Login { ip, username, .. } => {
                assert_eq!(ip, "127.0.0.1");
                assert_eq!(username, "testuser");
            }
            other => panic!("Expected Login, got {:?}", other),
        }
    }

    #[test]
    fn set_error_shown_and_cleared() {
        let mut form = make_form();
        form.set_error(Some("bad password".to_string()));
        assert!(form.error_text.is_some());
        form.set_error(None);
        assert!(form.error_text.is_none());
    }

    #[test]
    fn set_submitting_clears_error() {
        let mut form = make_form();
        form.set_error(Some("oops".to_string()));
        form.set_submitting(true);
        assert!(form.show_submitting);
        assert!(form.error_text.is_none());
    }
}
