//! Certificate-mismatch dialog widget.
//!
//! Displayed when the server presents a TLS certificate whose fingerprint
//! does not match the one previously trusted by the client.  The user can
//! accept the new fingerprint or reject the connection.

use sdl2::pixels::Color;
use sdl2::render::BlendMode;

use super::button::RectButton;
use super::style::{Background, Border};
use super::widget::{Bounds, EventResponse, UiEvent, Widget};
use super::RenderContext;
use crate::font_cache;

// ---------------------------------------------------------------------------
// Layout constants
// ---------------------------------------------------------------------------

const DIALOG_W: u32 = 440;
const DIALOG_H: u32 = 220;
const PAD: i32 = 12;
const FONT: usize = 1;
const BTN_W: u32 = 140;
const BTN_H: u32 = 22;
const BTN_GAP: i32 = 10;

// ---------------------------------------------------------------------------
// Actions
// ---------------------------------------------------------------------------

/// Side-effect produced by the certificate dialog.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CertDialogAction {
    /// User accepted the new certificate fingerprint.
    Accept,
    /// User rejected the new certificate.
    Reject,
}

// ---------------------------------------------------------------------------
// Widget
// ---------------------------------------------------------------------------

/// Modal dialog shown when a server certificate fingerprint changes.
pub struct CertDialog {
    bounds: Bounds,
    /// The hostname that presented the mismatched certificate.
    pub host: String,
    /// Previously trusted fingerprint.
    pub expected_fp: String,
    /// Fingerprint received from the server.
    pub received_fp: String,
    accept_button: RectButton,
    reject_button: RectButton,
    actions: Vec<CertDialogAction>,
}

impl CertDialog {
    /// Creates a new certificate dialog, centred on screen.
    ///
    /// # Arguments
    ///
    /// * `host` - The hostname that presented the mismatched certificate.
    /// * `expected_fp` - Previously trusted fingerprint.
    /// * `received_fp` - Fingerprint received from the server.
    ///
    /// # Returns
    ///
    /// A new `CertDialog`.
    pub fn new(host: &str, expected_fp: &str, received_fp: &str) -> Self {
        let x = (crate::constants::TARGET_WIDTH_INT - DIALOG_W) as i32 / 2;
        let y = (crate::constants::TARGET_HEIGHT_INT - DIALOG_H) as i32 / 2;
        let bounds = Bounds::new(x, y, DIALOG_W, DIALOG_H);

        let btn_bg = Background::SolidColor(Color::RGBA(50, 50, 80, 220));
        let btn_border = Border {
            color: Color::RGBA(140, 140, 200, 220),
            width: 1,
        };

        let btn_y = y + DIALOG_H as i32 - PAD - BTN_H as i32;
        let total_btn = BTN_W as i32 * 2 + BTN_GAP;
        let btn_x = x + (DIALOG_W as i32 - total_btn) / 2;

        let accept_button = RectButton::new(Bounds::new(btn_x, btn_y, BTN_W, BTN_H), btn_bg)
            .with_border(btn_border)
            .with_label("Accept New Cert", FONT);
        let reject_button = RectButton::new(
            Bounds::new(btn_x + BTN_W as i32 + BTN_GAP, btn_y, BTN_W, BTN_H),
            btn_bg,
        )
        .with_border(btn_border)
        .with_label("Reject", FONT);

        Self {
            bounds,
            host: host.to_owned(),
            expected_fp: expected_fp.to_owned(),
            received_fp: received_fp.to_owned(),
            accept_button,
            reject_button,
            actions: Vec::new(),
        }
    }

    /// Drains pending [`CertDialogAction`]s.
    ///
    /// # Returns
    ///
    /// A vector of actions produced since the last call.
    pub fn take_cert_actions(&mut self) -> Vec<CertDialogAction> {
        std::mem::take(&mut self.actions)
    }
}

impl Widget for CertDialog {
    fn bounds(&self) -> &Bounds {
        &self.bounds
    }

    fn set_position(&mut self, _x: i32, _y: i32) {
        // Fixed position — not supported.
    }

    fn handle_event(&mut self, event: &UiEvent) -> EventResponse {
        if self.accept_button.handle_event(event) == EventResponse::Consumed {
            self.actions.push(CertDialogAction::Accept);
            return EventResponse::Consumed;
        }
        if self.reject_button.handle_event(event) == EventResponse::Consumed {
            self.actions.push(CertDialogAction::Reject);
            return EventResponse::Consumed;
        }

        // Consume all mouse events so the login form behind is not interactive.
        match event {
            UiEvent::MouseClick { .. }
            | UiEvent::MouseDown { .. }
            | UiEvent::TextInput { .. }
            | UiEvent::KeyDown { .. } => EventResponse::Consumed,
            _ => EventResponse::Ignored,
        }
    }

    fn render(&mut self, ctx: &mut RenderContext<'_, '_>) -> Result<(), String> {
        // ── Dim background ──────────────────────────────────────────────
        let screen = sdl2::rect::Rect::new(
            0,
            0,
            crate::constants::TARGET_WIDTH_INT,
            crate::constants::TARGET_HEIGHT_INT,
        );
        ctx.canvas.set_blend_mode(BlendMode::Blend);
        ctx.canvas.set_draw_color(Color::RGBA(0, 0, 0, 140));
        ctx.canvas.fill_rect(screen)?;

        // ── Dialog panel ─────────────────────────────────────────────────
        let rect = sdl2::rect::Rect::new(
            self.bounds.x,
            self.bounds.y,
            self.bounds.width,
            self.bounds.height,
        );
        ctx.canvas.set_draw_color(Color::RGBA(20, 20, 40, 240));
        ctx.canvas.fill_rect(rect)?;
        ctx.canvas.set_draw_color(Color::RGBA(140, 100, 100, 220));
        ctx.canvas.draw_rect(rect)?;

        let mut y = self.bounds.y + PAD;
        let x = self.bounds.x + PAD;

        // Title
        font_cache::draw_text_tinted(
            ctx.canvas,
            ctx.gfx,
            FONT,
            "Server Certificate Changed",
            x,
            y,
            Color::RGB(255, 200, 200),
        )?;
        y += font_cache::BITMAP_GLYPH_H as i32 + 6;

        // Warning
        font_cache::draw_text_tinted(
            ctx.canvas,
            ctx.gfx,
            FONT,
            "This may indicate a man-in-the-middle attack",
            x,
            y,
            Color::RGB(255, 255, 80),
        )?;
        y += font_cache::BITMAP_GLYPH_H as i32 + 2;
        font_cache::draw_text_tinted(
            ctx.canvas,
            ctx.gfx,
            FONT,
            "unless you rotated your certificates.",
            x,
            y,
            Color::RGB(255, 255, 80),
        )?;
        y += font_cache::BITMAP_GLYPH_H as i32 + 8;

        // Host
        let host_line = format!("Host: {}", self.host);
        font_cache::draw_text(ctx.canvas, ctx.gfx, FONT, &host_line, x, y)?;
        y += font_cache::BITMAP_GLYPH_H as i32 + 4;

        // Expected fingerprint
        font_cache::draw_text(
            ctx.canvas,
            ctx.gfx,
            FONT,
            "Previously trusted fingerprint:",
            x,
            y,
        )?;
        y += font_cache::BITMAP_GLYPH_H as i32 + 2;

        // Use font 0 (smallest) for the fingerprint hashes
        let fp_display_len =
            (DIALOG_W as usize - 2 * PAD as usize) / font_cache::BITMAP_GLYPH_ADVANCE as usize;
        let expected_display: String = self.expected_fp.chars().take(fp_display_len).collect();
        font_cache::draw_text(ctx.canvas, ctx.gfx, 0, &expected_display, x, y)?;
        y += font_cache::BITMAP_GLYPH_H as i32 + 4;

        // Received fingerprint
        font_cache::draw_text(
            ctx.canvas,
            ctx.gfx,
            FONT,
            "New fingerprint from server:",
            x,
            y,
        )?;
        y += font_cache::BITMAP_GLYPH_H as i32 + 2;
        let received_display: String = self.received_fp.chars().take(fp_display_len).collect();
        font_cache::draw_text(ctx.canvas, ctx.gfx, 0, &received_display, x, y)?;

        // ── Buttons ──────────────────────────────────────────────────────
        self.accept_button.render(ctx)?;
        self.reject_button.render(ctx)?;

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::widget::{KeyModifiers, MouseButton};

    fn make_dialog() -> CertDialog {
        CertDialog::new("example.com", "AA:BB:CC:DD:EE:FF", "11:22:33:44:55:66")
    }

    #[test]
    fn initial_state() {
        let dialog = make_dialog();
        assert_eq!(dialog.host, "example.com");
        assert_eq!(dialog.expected_fp, "AA:BB:CC:DD:EE:FF");
        assert_eq!(dialog.received_fp, "11:22:33:44:55:66");
    }

    #[test]
    fn click_accept_produces_action() {
        let mut dialog = make_dialog();
        let btn_bounds = *dialog.accept_button.bounds();
        let click = UiEvent::MouseClick {
            x: btn_bounds.x + 5,
            y: btn_bounds.y + 5,
            button: MouseButton::Left,
            modifiers: KeyModifiers::default(),
        };
        let resp = dialog.handle_event(&click);
        assert_eq!(resp, EventResponse::Consumed);
        let actions = dialog.take_cert_actions();
        assert_eq!(actions, vec![CertDialogAction::Accept]);
    }

    #[test]
    fn click_reject_produces_action() {
        let mut dialog = make_dialog();
        let btn_bounds = *dialog.reject_button.bounds();
        let click = UiEvent::MouseClick {
            x: btn_bounds.x + 5,
            y: btn_bounds.y + 5,
            button: MouseButton::Left,
            modifiers: KeyModifiers::default(),
        };
        let resp = dialog.handle_event(&click);
        assert_eq!(resp, EventResponse::Consumed);
        let actions = dialog.take_cert_actions();
        assert_eq!(actions, vec![CertDialogAction::Reject]);
    }

    #[test]
    fn blocks_mouse_events() {
        let mut dialog = make_dialog();
        // Click outside the buttons but the dialog still consumes.
        let resp = dialog.handle_event(&UiEvent::MouseClick {
            x: dialog.bounds.x + 1,
            y: dialog.bounds.y + 1,
            button: MouseButton::Left,
            modifiers: KeyModifiers::default(),
        });
        assert_eq!(resp, EventResponse::Consumed);
    }

    #[test]
    fn blocks_keyboard_events() {
        let mut dialog = make_dialog();
        let resp = dialog.handle_event(&UiEvent::KeyDown {
            keycode: sdl2::keyboard::Keycode::A,
            modifiers: KeyModifiers::default(),
        });
        assert_eq!(resp, EventResponse::Consumed);
    }
}
