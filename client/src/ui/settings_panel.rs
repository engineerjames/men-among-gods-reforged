//! Settings / options panel.
//!
//! Duplicates all capabilities of the egui-based escape menu (visual toggles,
//! audio volume, display mode, diagnostics, disconnect/quit) using the native
//! Widget-based UI framework.

use sdl2::pixels::Color;
use sdl2::render::BlendMode;

use super::button::RectButton;
use super::checkbox::Checkbox;
use super::dropdown::Dropdown;
use super::label::Label;
use super::slider::Slider;
use super::style::{Background, Border};
use super::widget::{Bounds, EventResponse, UiEvent, Widget, WidgetAction};
use super::RenderContext;
use crate::font_cache;
use crate::preferences::DisplayMode;

// ---------------------------------------------------------------------------
// Layout constants
// ---------------------------------------------------------------------------

/// Row height for controls.
const ROW_H: i32 = 14;
/// Horizontal inset from panel edges for controls.
const H_INSET: i32 = 10;
/// Width available for controls inside the panel.
const CONTROL_W: u32 = 280;
/// Height of a button row.
const BTN_H: u32 = 16;

// Y offsets from panel top for each element.
const Y_TITLE: i32 = 6;
const Y_VISUAL_HEADER: i32 = 22;
const Y_SHADOWS: i32 = 36;
const Y_SPELL_FX: i32 = 50;
const Y_NAMES: i32 = 64;
const Y_HEALTH: i32 = 78;
const Y_HELPER_TEXT: i32 = 92;
const Y_WALLS: i32 = 106;
const Y_AUDIO_HEADER: i32 = 124;
const Y_VOLUME: i32 = 138;
const Y_DISPLAY_HEADER: i32 = 156;
const Y_DISPLAY_MODE: i32 = 170;
const Y_PIXEL_PERFECT: i32 = 190;
const Y_VSYNC: i32 = 204;
const Y_DIAG_HEADER: i32 = 222;
const Y_PING: i32 = 236;
const Y_PROFILER_BTN: i32 = 252;
const Y_LOGDIR_BTN: i32 = 272;
const Y_SEPARATOR: i32 = 294;
const Y_SESSION_BTNS: i32 = 306;
const Y_RETURN_BTN: i32 = 328;

/// Total panel height needed to fit all controls.
pub const SETTINGS_PANEL_H: u32 = 352;

// ---------------------------------------------------------------------------
// Data snapshot
// ---------------------------------------------------------------------------

/// Snapshot of current settings values used to populate the panel when it
/// opens.
///
/// Built by the owning scene and passed to [`SettingsPanel::sync_state`].
pub struct SettingsPanelData {
    /// Whether shadow rendering is enabled.
    pub shadows_enabled: bool,
    /// Whether spell visual effects are rendered.
    pub spell_effects_enabled: bool,
    /// Whether overhead player names are shown.
    pub show_names: bool,
    /// Whether overhead health percentages are shown.
    pub show_health_pct: bool,
    /// Whether walls are hidden.
    pub hide_walls: bool,
    /// Whether context-sensitive helper text is shown near the cursor.
    pub show_helper_text: bool,
    /// Master volume (0.0–1.0).
    pub master_volume: f32,
    /// Current display mode.
    pub display_mode: DisplayMode,
    /// Whether pixel-perfect (integer) scaling is active.
    pub pixel_perfect_scaling: bool,
    /// Whether VSync is enabled.
    pub vsync_enabled: bool,
    /// Latest network round-trip time, if available.
    pub last_rtt_ms: Option<u32>,
    /// Whether the performance profiler is currently running.
    pub profiler_active: bool,
    /// Seconds remaining on the profiler, if active.
    pub profiler_remaining_secs: Option<u64>,
}

// ---------------------------------------------------------------------------
// SettingsPanel
// ---------------------------------------------------------------------------

/// The settings / options HUD panel.
///
/// Toggleable via the HUD button bar. When visible, draws all settings
/// controls and emits [`WidgetAction`]s when the user changes a value.
pub struct SettingsPanel {
    bounds: Bounds,
    bg_color: Color,
    border_color: Color,
    visible: bool,
    pending_actions: Vec<WidgetAction>,

    // --- Child widgets ---
    chk_shadows: Checkbox,
    chk_spell_effects: Checkbox,
    chk_show_names: Checkbox,
    chk_show_health: Checkbox,
    chk_helper_text: Checkbox,
    chk_hide_walls: Checkbox,
    sld_volume: Slider,
    drp_display_mode: Dropdown,
    chk_pixel_perfect: Checkbox,
    chk_vsync: Checkbox,
    lbl_ping: Label,
    btn_profiler: RectButton,
    btn_log_dir: RectButton,
    btn_disconnect: RectButton,
    btn_quit: RectButton,
    btn_return: RectButton,
}

impl SettingsPanel {
    /// Creates a new settings panel with all child controls laid out
    /// relative to the given bounds.
    ///
    /// # Arguments
    ///
    /// * `bounds` - Position and size of the panel.
    /// * `bg_color` - Semi-transparent background color.
    ///
    /// # Returns
    ///
    /// A new `SettingsPanel`, initially hidden.
    pub fn new(bounds: Bounds, bg_color: Color) -> Self {
        let x = bounds.x + H_INSET;
        let w = CONTROL_W.min(bounds.width.saturating_sub(H_INSET as u32 * 2));

        let btn_bg = Background::SolidColor(Color::RGBA(40, 40, 60, 200));
        let btn_border = Border {
            color: Color::RGBA(120, 120, 140, 200),
            width: 1,
        };

        let half_w = (w.saturating_sub(10)) / 2;

        Self {
            bounds,
            bg_color,
            border_color: Color::RGBA(120, 120, 140, 200),
            visible: false,
            pending_actions: Vec::new(),

            chk_shadows: Checkbox::new(
                Bounds::new(x, bounds.y + Y_SHADOWS, w, ROW_H as u32),
                "Enable Shadows",
                0,
            ),
            chk_spell_effects: Checkbox::new(
                Bounds::new(x, bounds.y + Y_SPELL_FX, w, ROW_H as u32),
                "Enable Spell Effects",
                0,
            ),
            chk_show_names: Checkbox::new(
                Bounds::new(x, bounds.y + Y_NAMES, w, ROW_H as u32),
                "Show Names",
                0,
            ),
            chk_show_health: Checkbox::new(
                Bounds::new(x, bounds.y + Y_HEALTH, w, ROW_H as u32),
                "Show % Health",
                0,
            ),
            chk_helper_text: Checkbox::new(
                Bounds::new(x, bounds.y + Y_HELPER_TEXT, w, ROW_H as u32),
                "Show Helper Text",
                0,
            ),
            chk_hide_walls: Checkbox::new(
                Bounds::new(x, bounds.y + Y_WALLS, w, ROW_H as u32),
                "Hide Walls",
                0,
            ),
            sld_volume: Slider::new(
                Bounds::new(x, bounds.y + Y_VOLUME, w, ROW_H as u32),
                "Volume",
                0.0,
                1.0,
                1.0,
                0,
            ),
            drp_display_mode: Dropdown::new(
                Bounds::new(x, bounds.y + Y_DISPLAY_MODE, w, 16),
                DisplayMode::ALL.iter().map(|m| m.to_string()).collect(),
                0,
                0,
            ),
            chk_pixel_perfect: Checkbox::new(
                Bounds::new(x, bounds.y + Y_PIXEL_PERFECT, w, ROW_H as u32),
                "Pixel-Perfect Scaling",
                0,
            ),
            chk_vsync: Checkbox::new(
                Bounds::new(x, bounds.y + Y_VSYNC, w, ROW_H as u32),
                "VSync",
                0,
            ),
            lbl_ping: Label::new("Ping: N/A", 0, x, bounds.y + Y_PING),
            btn_profiler: RectButton::new(
                Bounds::new(x, bounds.y + Y_PROFILER_BTN, w, BTN_H),
                btn_bg,
            )
            .with_label("Profile Performance", 0)
            .with_border(btn_border),
            btn_log_dir: RectButton::new(Bounds::new(x, bounds.y + Y_LOGDIR_BTN, w, BTN_H), btn_bg)
                .with_label("Open Log Directory", 0)
                .with_border(btn_border),
            btn_disconnect: RectButton::new(
                Bounds::new(x, bounds.y + Y_SESSION_BTNS, half_w, BTN_H),
                btn_bg,
            )
            .with_label("Disconnect", 0)
            .with_border(btn_border),
            btn_quit: RectButton::new(
                Bounds::new(
                    x + half_w as i32 + 10,
                    bounds.y + Y_SESSION_BTNS,
                    half_w,
                    BTN_H,
                ),
                btn_bg,
            )
            .with_label("Quit", 0)
            .with_border(btn_border),
            btn_return: RectButton::new(Bounds::new(x, bounds.y + Y_RETURN_BTN, w, BTN_H), btn_bg)
                .with_label("Return to Game", 0)
                .with_border(btn_border),
        }
    }

    /// Toggles the panel's visibility.
    pub fn toggle(&mut self) {
        self.visible = !self.visible;
    }

    /// Returns whether the panel is currently visible.
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Loads all control values from the given data snapshot.
    ///
    /// Call this when the panel becomes visible so controls reflect the
    /// current game state.
    ///
    /// # Arguments
    ///
    /// * `data` - Snapshot of current settings values.
    pub fn sync_state(&mut self, data: &SettingsPanelData) {
        self.chk_shadows.set_checked(data.shadows_enabled);
        self.chk_spell_effects
            .set_checked(data.spell_effects_enabled);
        self.chk_show_names.set_checked(data.show_names);
        self.chk_show_health.set_checked(data.show_health_pct);
        self.chk_helper_text.set_checked(data.show_helper_text);
        self.chk_hide_walls.set_checked(data.hide_walls);
        self.sld_volume.set_value(data.master_volume);
        self.chk_pixel_perfect
            .set_checked(data.pixel_perfect_scaling);
        self.chk_vsync.set_checked(data.vsync_enabled);

        // Map DisplayMode to dropdown index.
        let mode_idx = DisplayMode::ALL
            .iter()
            .position(|m| *m == data.display_mode)
            .unwrap_or(0);
        self.drp_display_mode.set_selected(mode_idx);

        self.update_ping(data.last_rtt_ms);
        self.update_profiler_label(data.profiler_active, data.profiler_remaining_secs);
    }

    /// Updates the ping readout label.
    ///
    /// Called each frame when visible so the value stays current.
    ///
    /// # Arguments
    ///
    /// * `rtt_ms` - Latest round-trip time in milliseconds, or `None`.
    pub fn update_ping(&mut self, rtt_ms: Option<u32>) {
        let text = match rtt_ms {
            Some(ms) => format!("Ping: {} ms", ms),
            None => "Ping: N/A".to_string(),
        };
        self.lbl_ping.set_text(&text);
    }

    /// Updates the profiler button label.
    ///
    /// # Arguments
    ///
    /// * `active` - Whether the profiler is currently running.
    /// * `remaining_secs` - Seconds remaining, if active.
    pub fn update_profiler_label(&mut self, active: bool, remaining_secs: Option<u64>) {
        // RectButton doesn't have set_label, so we rebuild it.
        // For now we just track the state; the label is static.
        // A future improvement could add a set_label method to RectButton.
        let _ = (active, remaining_secs);
    }

    /// Draws a centered section header string.
    fn draw_section_header(
        ctx: &mut RenderContext,
        text: &str,
        center_x: i32,
        y: i32,
    ) -> Result<(), String> {
        font_cache::draw_text_centered(ctx.canvas, ctx.gfx, 1, text, center_x, y)
    }

    /// Collects `WidgetAction`s from all child widgets that report changes.
    fn collect_child_actions(&mut self) {
        if self.chk_shadows.was_toggled() {
            self.pending_actions
                .push(WidgetAction::SetShadows(self.chk_shadows.is_checked()));
        }
        if self.chk_spell_effects.was_toggled() {
            self.pending_actions.push(WidgetAction::SetSpellEffects(
                self.chk_spell_effects.is_checked(),
            ));
        }
        if self.chk_show_names.was_toggled() {
            self.pending_actions
                .push(WidgetAction::SetShowNames(self.chk_show_names.is_checked()));
        }
        if self.chk_show_health.was_toggled() {
            self.pending_actions.push(WidgetAction::SetShowHealthPct(
                self.chk_show_health.is_checked(),
            ));
        }
        if self.chk_hide_walls.was_toggled() {
            self.pending_actions
                .push(WidgetAction::SetHideWalls(self.chk_hide_walls.is_checked()));
        }
        if self.chk_helper_text.was_toggled() {
            self.pending_actions.push(WidgetAction::SetShowHelperText(
                self.chk_helper_text.is_checked(),
            ));
        }
        if self.sld_volume.was_changed() {
            self.pending_actions
                .push(WidgetAction::SetMasterVolume(self.sld_volume.value()));
        }
        if self.drp_display_mode.was_changed() {
            let mode = DisplayMode::ALL[self.drp_display_mode.selected_index()];
            self.pending_actions
                .push(WidgetAction::SetDisplayMode(mode));
        }
        if self.chk_pixel_perfect.was_toggled() {
            self.pending_actions
                .push(WidgetAction::SetPixelPerfectScaling(
                    self.chk_pixel_perfect.is_checked(),
                ));
        }
        if self.chk_vsync.was_toggled() {
            self.pending_actions
                .push(WidgetAction::SetVSync(self.chk_vsync.is_checked()));
        }
    }
}

impl Widget for SettingsPanel {
    fn bounds(&self) -> &Bounds {
        &self.bounds
    }

    fn set_position(&mut self, x: i32, y: i32) {
        self.bounds.x = x;
        self.bounds.y = y;
    }

    fn handle_event(&mut self, event: &UiEvent) -> EventResponse {
        if !self.visible {
            return EventResponse::Ignored;
        }

        // When dropdown is expanded, it gets first priority so it can
        // capture clicks on its overlay area.
        if self.drp_display_mode.is_expanded() {
            let resp = self.drp_display_mode.handle_event(event);
            self.collect_child_actions();
            if resp == EventResponse::Consumed {
                return EventResponse::Consumed;
            }
        }

        // Delegate to interactive children (order doesn't matter much here
        // since they don't overlap, but we check buttons last).
        let children_responses = [
            self.chk_shadows.handle_event(event),
            self.chk_spell_effects.handle_event(event),
            self.chk_show_names.handle_event(event),
            self.chk_show_health.handle_event(event),
            self.chk_helper_text.handle_event(event),
            self.chk_hide_walls.handle_event(event),
            self.sld_volume.handle_event(event),
            if !self.drp_display_mode.is_expanded() {
                self.drp_display_mode.handle_event(event)
            } else {
                EventResponse::Ignored
            },
            self.chk_pixel_perfect.handle_event(event),
            self.chk_vsync.handle_event(event),
        ];

        // Check checkbox/slider/dropdown changes.
        self.collect_child_actions();

        if children_responses
            .iter()
            .any(|r| *r == EventResponse::Consumed)
        {
            return EventResponse::Consumed;
        }

        // Check buttons.
        if self.btn_profiler.handle_event(event) == EventResponse::Consumed {
            self.pending_actions.push(WidgetAction::StartProfiler);
            return EventResponse::Consumed;
        }
        if self.btn_log_dir.handle_event(event) == EventResponse::Consumed {
            self.pending_actions.push(WidgetAction::OpenLogDir);
            return EventResponse::Consumed;
        }
        if self.btn_disconnect.handle_event(event) == EventResponse::Consumed {
            self.pending_actions.push(WidgetAction::Disconnect);
            return EventResponse::Consumed;
        }
        if self.btn_quit.handle_event(event) == EventResponse::Consumed {
            self.pending_actions.push(WidgetAction::Quit);
            return EventResponse::Consumed;
        }
        if self.btn_return.handle_event(event) == EventResponse::Consumed {
            self.visible = false;
            return EventResponse::Consumed;
        }

        // Consume any click inside panel bounds to prevent click-through.
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

        // Semi-transparent background
        ctx.canvas.set_blend_mode(BlendMode::Blend);
        ctx.canvas.set_draw_color(self.bg_color);
        ctx.canvas.fill_rect(rect)?;

        // Border
        ctx.canvas.set_draw_color(self.border_color);
        ctx.canvas.draw_rect(rect)?;

        let center_x = self.bounds.x + self.bounds.width as i32 / 2;

        // Title
        font_cache::draw_text_centered(
            ctx.canvas,
            ctx.gfx,
            1,
            "Settings",
            center_x,
            self.bounds.y + Y_TITLE,
        )?;

        // Section headers
        Self::draw_section_header(
            ctx,
            "-- Visual --",
            center_x,
            self.bounds.y + Y_VISUAL_HEADER,
        )?;
        Self::draw_section_header(ctx, "-- Audio --", center_x, self.bounds.y + Y_AUDIO_HEADER)?;
        Self::draw_section_header(
            ctx,
            "-- Display --",
            center_x,
            self.bounds.y + Y_DISPLAY_HEADER,
        )?;
        Self::draw_section_header(
            ctx,
            "-- Diagnostics --",
            center_x,
            self.bounds.y + Y_DIAG_HEADER,
        )?;

        // Separator line above disconnect/quit
        let sep_y = self.bounds.y + Y_SEPARATOR;
        ctx.canvas.set_draw_color(Color::RGBA(120, 120, 140, 150));
        ctx.canvas.draw_line(
            sdl2::rect::Point::new(self.bounds.x + H_INSET, sep_y),
            sdl2::rect::Point::new(self.bounds.x + self.bounds.width as i32 - H_INSET, sep_y),
        )?;

        // Render child widgets (order: back-to-front, dropdown last if expanded)
        self.chk_shadows.render(ctx)?;
        self.chk_spell_effects.render(ctx)?;
        self.chk_show_names.render(ctx)?;
        self.chk_show_health.render(ctx)?;
        self.chk_helper_text.render(ctx)?;
        self.chk_hide_walls.render(ctx)?;
        self.sld_volume.render(ctx)?;
        self.chk_pixel_perfect.render(ctx)?;
        self.chk_vsync.render(ctx)?;
        self.lbl_ping.render(ctx)?;
        self.btn_profiler.render(ctx)?;
        self.btn_log_dir.render(ctx)?;
        self.btn_disconnect.render(ctx)?;
        self.btn_quit.render(ctx)?;
        self.btn_return.render(ctx)?;

        // Dropdown rendered last so its expanded option list overlays everything.
        self.drp_display_mode.render(ctx)?;

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
    use crate::ui::widget::{KeyModifiers, MouseButton};

    fn make_panel() -> SettingsPanel {
        SettingsPanel::new(
            Bounds::new(0, 0, 300, SETTINGS_PANEL_H),
            Color::RGBA(0, 0, 0, 180),
        )
    }

    #[test]
    fn starts_hidden() {
        let panel = make_panel();
        assert!(!panel.is_visible());
    }

    #[test]
    fn toggle_flips_visibility() {
        let mut panel = make_panel();
        panel.toggle();
        assert!(panel.is_visible());
        panel.toggle();
        assert!(!panel.is_visible());
    }

    #[test]
    fn hidden_panel_ignores_clicks() {
        let mut panel = make_panel();
        let resp = panel.handle_event(&UiEvent::MouseClick {
            x: 50,
            y: 50,
            button: MouseButton::Left,
            modifiers: KeyModifiers::default(),
        });
        assert_eq!(resp, EventResponse::Ignored);
    }

    #[test]
    fn visible_panel_consumes_clicks_inside() {
        let mut panel = make_panel();
        panel.toggle();
        let resp = panel.handle_event(&UiEvent::MouseClick {
            x: 50,
            y: 50,
            button: MouseButton::Left,
            modifiers: KeyModifiers::default(),
        });
        assert_eq!(resp, EventResponse::Consumed);
    }

    #[test]
    fn visible_panel_ignores_clicks_outside() {
        let mut panel = make_panel();
        panel.toggle();
        let resp = panel.handle_event(&UiEvent::MouseClick {
            x: 500,
            y: 500,
            button: MouseButton::Left,
            modifiers: KeyModifiers::default(),
        });
        assert_eq!(resp, EventResponse::Ignored);
    }

    #[test]
    fn sync_state_sets_checkboxes() {
        let mut panel = make_panel();
        panel.sync_state(&SettingsPanelData {
            shadows_enabled: true,
            spell_effects_enabled: false,
            show_names: true,
            show_health_pct: true,
            hide_walls: false,
            show_helper_text: true,
            master_volume: 0.75,
            display_mode: DisplayMode::Fullscreen,
            pixel_perfect_scaling: true,
            vsync_enabled: false,
            last_rtt_ms: Some(42),
            profiler_active: false,
            profiler_remaining_secs: None,
        });
        assert!(panel.chk_shadows.is_checked());
        assert!(!panel.chk_spell_effects.is_checked());
        assert!(panel.chk_show_names.is_checked());
        assert!(panel.chk_show_health.is_checked());
        assert!(!panel.chk_hide_walls.is_checked());
        assert!(panel.chk_helper_text.is_checked());
        assert!((panel.sld_volume.value() - 0.75).abs() < 0.01);
        assert_eq!(panel.drp_display_mode.selected_index(), 1); // Fullscreen
        assert!(panel.chk_pixel_perfect.is_checked());
        assert!(!panel.chk_vsync.is_checked());
    }

    #[test]
    fn checkbox_click_emits_action() {
        let mut panel = make_panel();
        panel.toggle();
        // Click the shadows checkbox area.
        let shadows_y = Y_SHADOWS + 5;
        panel.handle_event(&UiEvent::MouseClick {
            x: 15,
            y: shadows_y,
            button: MouseButton::Left,
            modifiers: KeyModifiers::default(),
        });
        let actions = panel.take_actions();
        assert!(
            actions
                .iter()
                .any(|a| matches!(a, WidgetAction::SetShadows(true))),
            "Expected SetShadows action, got {:?}",
            actions
        );
    }

    #[test]
    fn disconnect_button_emits_action() {
        let mut panel = make_panel();
        panel.toggle();
        let btn_y = Y_SESSION_BTNS + 5;
        panel.handle_event(&UiEvent::MouseClick {
            x: 15,
            y: btn_y,
            button: MouseButton::Left,
            modifiers: KeyModifiers::default(),
        });
        let actions = panel.take_actions();
        assert!(
            actions
                .iter()
                .any(|a| matches!(a, WidgetAction::Disconnect)),
            "Expected Disconnect action, got {:?}",
            actions
        );
    }

    #[test]
    fn take_actions_drains() {
        let mut panel = make_panel();
        panel.toggle();
        panel.handle_event(&UiEvent::MouseClick {
            x: 15,
            y: Y_SHADOWS + 5,
            button: MouseButton::Left,
            modifiers: KeyModifiers::default(),
        });
        let first = panel.take_actions();
        assert!(!first.is_empty());
        let second = panel.take_actions();
        assert!(second.is_empty());
    }
}
