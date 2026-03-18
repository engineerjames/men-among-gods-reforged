//! Keyboard bindings editor panel.
//!
//! Provides a widget to view and edit the mapping from [`GameAction`]s to
//! [`KeyBinding`]s. Opened from the settings panel via a "Keyboard Bindings"
//! button.

use sdl2::keyboard::Keycode;
use sdl2::pixels::Color;
use sdl2::render::BlendMode;

use super::button::RectButton;
use super::style::{Background, Border};
use super::title_bar::{clamp_to_viewport, TitleBar, TITLE_BAR_H};
use super::widget::{
    Bounds, EventResponse, GameAction, HudPanel, KeyBinding, KeyBindings, UiEvent, Widget,
    WidgetAction,
};
use super::RenderContext;
use crate::font_cache;

// ---------------------------------------------------------------------------
// Layout constants
// ---------------------------------------------------------------------------

/// Row height for each binding entry.
const ROW_H: i32 = 20;
/// Horizontal inset from panel edges.
const H_INSET: i32 = 10;
/// Width of binding buttons on the right side.
const BTN_W: u32 = 120;
/// Height of binding buttons.
const BTN_H: u32 = 16;
/// Y offset of the first row below the title bar.
const Y_FIRST_ROW: i32 = 8 + TITLE_BAR_H;
/// Panel width.
pub const KEYBINDINGS_PANEL_W: u32 = 300;

/// Compute the total panel height for the given number of actions.
const fn panel_height(action_count: usize) -> u32 {
    (Y_FIRST_ROW + ROW_H * action_count as i32 + 10) as u32
}

/// Total panel height (based on current action count).
pub const KEYBINDINGS_PANEL_H: u32 = panel_height(GameAction::ALL.len());

// ---------------------------------------------------------------------------
// Data snapshot
// ---------------------------------------------------------------------------

/// Snapshot of current keybindings used to populate the panel when it opens.
pub struct KeybindingsPanelData {
    /// Current bindings.
    pub bindings: KeyBindings,
}

// ---------------------------------------------------------------------------
// KeybindingsPanel
// ---------------------------------------------------------------------------

/// The keyboard bindings editor panel.
///
/// Displays one row per [`GameAction`] with the action name on the left and
/// a clickable button showing the current binding on the right. Clicking the
/// button enters "listening" mode: the next key press (with modifiers)
/// replaces the binding. Escape cancels listening.
pub struct KeybindingsPanel {
    bounds: Bounds,
    bg_color: Color,
    border_color: Color,
    visible: bool,
    pending_actions: Vec<WidgetAction>,

    /// Draggable title bar.
    title_bar: TitleBar,

    /// One button per action, in the order of [`GameAction::ALL`].
    binding_buttons: Vec<RectButton>,

    /// Index into `binding_buttons` currently awaiting a key press, or `None`.
    listening_for: Option<usize>,

    /// Local copy of bindings, kept in sync with the panel's buttons.
    bindings: KeyBindings,
}

impl KeybindingsPanel {
    /// Creates a new keybindings panel.
    ///
    /// # Arguments
    ///
    /// * `bounds` - Position and size of the panel.
    /// * `bg_color` - Semi-transparent background colour.
    ///
    /// # Returns
    ///
    /// A new `KeybindingsPanel`, initially hidden.
    pub fn new(bounds: Bounds, bg_color: Color) -> Self {
        let btn_bg = Background::SolidColor(Color::RGBA(40, 40, 60, 200));
        let btn_border = Border {
            color: Color::RGBA(120, 120, 140, 200),
            width: 1,
        };

        let btn_x = bounds.x + bounds.width as i32 - H_INSET - BTN_W as i32;

        let bindings = KeyBindings::default();
        let binding_buttons = GameAction::ALL
            .iter()
            .enumerate()
            .map(|(i, action)| {
                let y = bounds.y + Y_FIRST_ROW + ROW_H * i as i32 + 2;
                let label = bindings
                    .binding_for(*action)
                    .map(|kb| kb.to_string())
                    .unwrap_or_else(|| "Unbound".to_string());
                RectButton::new(Bounds::new(btn_x, y, BTN_W, BTN_H), btn_bg)
                    .with_label(&label, 0)
                    .with_border(btn_border)
            })
            .collect();

        Self {
            bounds,
            bg_color,
            border_color: Color::RGBA(120, 120, 140, 200),
            visible: false,
            pending_actions: Vec::new(),
            title_bar: TitleBar::new("Keyboard Bindings", bounds.x, bounds.y, bounds.width),
            binding_buttons,
            listening_for: None,
            bindings,
        }
    }

    /// Toggles the panel's visibility.
    pub fn toggle(&mut self) {
        self.visible = !self.visible;
        if !self.visible {
            self.listening_for = None;
        }
    }

    /// Returns whether the panel is currently visible.
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Loads binding data from the given snapshot.
    ///
    /// # Arguments
    ///
    /// * `data` - Snapshot of current keybindings.
    pub fn sync_state(&mut self, data: &KeybindingsPanelData) {
        self.bindings = data.bindings.clone();
        self.refresh_button_labels();
        self.listening_for = None;
    }

    /// Updates every button label from the current `self.bindings`.
    fn refresh_button_labels(&mut self) {
        for (i, action) in GameAction::ALL.iter().enumerate() {
            let label = self
                .bindings
                .binding_for(*action)
                .map(|kb| kb.to_string())
                .unwrap_or_else(|| "Unbound".to_string());
            if let Some(btn) = self.binding_buttons.get_mut(i) {
                btn.set_label(&label);
            }
        }
    }

    /// Returns `true` if `kc` is a "real" key that should finalise a binding
    /// (letters, digits, F-keys, etc.) rather than a modifier-only key.
    fn is_bindable_key(kc: Keycode) -> bool {
        !matches!(
            kc,
            Keycode::LCtrl
                | Keycode::RCtrl
                | Keycode::LShift
                | Keycode::RShift
                | Keycode::LAlt
                | Keycode::RAlt
                | Keycode::LGui
                | Keycode::RGui
                | Keycode::CapsLock
                | Keycode::NumLockClear
                | Keycode::ScrollLock
        )
    }
}

impl Widget for KeybindingsPanel {
    fn bounds(&self) -> &Bounds {
        &self.bounds
    }

    fn set_position(&mut self, x: i32, y: i32) {
        let dx = x - self.bounds.x;
        let dy = y - self.bounds.y;
        self.bounds.x = x;
        self.bounds.y = y;
        self.title_bar.set_bar_position(x, y);
        for btn in &mut self.binding_buttons {
            let b = btn.bounds();
            let (nx, ny) = (b.x + dx, b.y + dy);
            btn.set_position(nx, ny);
        }
    }

    fn handle_event(&mut self, event: &UiEvent) -> EventResponse {
        if !self.visible {
            return EventResponse::Ignored;
        }

        // --- Listening mode: capture the next real key press ---
        if let Some(idx) = self.listening_for {
            if let UiEvent::KeyDown { keycode, modifiers } = event {
                if *keycode == Keycode::Escape {
                    // Cancel listening.
                    self.listening_for = None;
                    self.refresh_button_labels();
                    return EventResponse::Consumed;
                }
                if Self::is_bindable_key(*keycode) {
                    let binding = KeyBinding::new(*keycode, *modifiers);
                    let action = GameAction::ALL[idx];
                    self.bindings.set_binding(action, binding);
                    self.listening_for = None;
                    self.refresh_button_labels();
                    self.pending_actions
                        .push(WidgetAction::UpdateKeyBinding { action, binding });
                    return EventResponse::Consumed;
                }
                // Modifier-only key: keep listening, consume event.
                return EventResponse::Consumed;
            }
            // Consume all non-key events while listening to avoid click-through.
            match event {
                UiEvent::TextInput { .. } => return EventResponse::Consumed,
                UiEvent::MouseClick { x, y, .. } | UiEvent::MouseDown { x, y, .. } => {
                    if self.bounds.contains_point(*x, *y) {
                        return EventResponse::Consumed;
                    }
                }
                _ => {}
            }
        }

        // --- Title bar: drag / close ---
        let (tb_resp, drag_pos) = self.title_bar.handle_event(event);
        if let Some((nx, ny)) = drag_pos {
            let (cx, cy) = clamp_to_viewport(nx, ny, self.bounds.width, self.bounds.height);
            self.set_position(cx, cy);
            return EventResponse::Consumed;
        }
        if self.title_bar.was_close_requested() {
            self.visible = false;
            self.listening_for = None;
            self.pending_actions
                .push(WidgetAction::TogglePanel(HudPanel::KeyBindings));
            return EventResponse::Consumed;
        }
        if tb_resp == EventResponse::Consumed {
            return EventResponse::Consumed;
        }

        // --- Binding buttons ---
        for (i, btn) in self.binding_buttons.iter_mut().enumerate() {
            if btn.handle_event(event) == EventResponse::Consumed {
                // Enter listening mode for this row.
                self.listening_for = Some(i);
                if let Some(b) = self.binding_buttons.get_mut(i) {
                    b.set_label("Press a key...");
                }
                return EventResponse::Consumed;
            }
        }

        // Consume clicks inside panel bounds to prevent click-through.
        match event {
            UiEvent::MouseClick { x, y, .. } | UiEvent::MouseWheel { x, y, .. } => {
                if self.bounds.contains_point(*x, *y) {
                    EventResponse::Consumed
                } else {
                    EventResponse::Ignored
                }
            }
            _ => EventResponse::Ignored,
        }
    }

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

        // Semi-transparent background.
        ctx.canvas.set_blend_mode(BlendMode::Blend);
        ctx.canvas.set_draw_color(self.bg_color);
        ctx.canvas.fill_rect(rect)?;

        // Border.
        ctx.canvas.set_draw_color(self.border_color);
        ctx.canvas.draw_rect(rect)?;

        // Title bar.
        self.title_bar.render(ctx)?;

        // Rows: action label + binding button.
        let label_x = self.bounds.x + H_INSET;
        for (i, action) in GameAction::ALL.iter().enumerate() {
            let y = self.bounds.y + Y_FIRST_ROW + ROW_H * i as i32;
            // Draw action label.
            font_cache::draw_text(
                ctx.canvas,
                ctx.gfx,
                0,
                action.label(),
                label_x,
                y + 3,
                font_cache::TextStyle::default(),
            )?;

            // Draw binding button.
            if let Some(btn) = self.binding_buttons.get_mut(i) {
                btn.render(ctx)?;
            }
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
    use crate::ui::widget::KeyModifiers;

    fn make_panel() -> KeybindingsPanel {
        KeybindingsPanel::new(
            Bounds::new(0, 0, KEYBINDINGS_PANEL_W, KEYBINDINGS_PANEL_H),
            Color::RGBA(0, 0, 0, 180),
        )
    }

    #[test]
    fn starts_hidden() {
        let panel = make_panel();
        assert!(!panel.is_visible());
    }

    #[test]
    fn toggle_visibility() {
        let mut panel = make_panel();
        panel.toggle();
        assert!(panel.is_visible());
        panel.toggle();
        assert!(!panel.is_visible());
    }

    #[test]
    fn sync_state_updates_bindings() {
        let mut panel = make_panel();
        let mut bindings = KeyBindings::default();
        bindings.set_binding(
            GameAction::ToggleSkills,
            KeyBinding::new(
                Keycode::K,
                KeyModifiers {
                    ctrl: true,
                    shift: false,
                    alt: false,
                },
            ),
        );
        panel.sync_state(&KeybindingsPanelData {
            bindings: bindings.clone(),
        });
        assert_eq!(
            panel
                .bindings
                .binding_for(GameAction::ToggleSkills)
                .unwrap()
                .keycode,
            Keycode::K as i32,
        );
    }

    #[test]
    fn listening_mode_cancelled_on_toggle_off() {
        let mut panel = make_panel();
        panel.toggle(); // visible
        panel.listening_for = Some(0);
        panel.toggle(); // hidden — should clear listening
        assert!(panel.listening_for.is_none());
    }
}
