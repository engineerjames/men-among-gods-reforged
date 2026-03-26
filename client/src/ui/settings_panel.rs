//! Settings / options panel.
//!
//! Presents a compact main menu with category buttons (Display Settings,
//! Diagnostics, Controls), an inline volume slider, and session controls.
//! Each category button opens a sub-panel that overlaps the main panel
//! content. Only one sub-panel is visible at a time.

use sdl2::keyboard::Keycode;
use sdl2::pixels::Color;
use sdl2::render::BlendMode;

use super::RenderContext;
use super::button::RectButton;
use super::checkbox::Checkbox;
use super::dropdown::Dropdown;
use super::label::Label;
use super::quit_confirm_dialog::{QuitConfirmDialog, QuitConfirmDialogAction};
use super::slider::Slider;
use super::style::{Background, Border};
use super::title_bar::{TITLE_BAR_H, TitleBar, clamp_to_viewport};
use super::widget::{
    Bounds, EventResponse, GameAction, HudPanel, KeyBinding, KeyBindings, UiEvent, Widget,
    WidgetAction,
};
use crate::font_cache;
use crate::preferences::DisplayMode;

// ---------------------------------------------------------------------------
// Layout constants — main panel
// ---------------------------------------------------------------------------

/// Row height for controls.
const ROW_H: i32 = 14;
/// Horizontal inset from panel edges for controls.
const H_INSET: i32 = 10;
/// Width available for controls inside the panel.
const CONTROL_W: u32 = 280;
/// Height of a button row.
const BTN_H: u32 = 16;

// Y offsets from main panel top (relative, added to bounds.y).
const Y_DISPLAY_BTN: i32 = TITLE_BAR_H + 8;
const Y_DIAG_BTN: i32 = Y_DISPLAY_BTN + BTN_H as i32 + 6;
const Y_CONTROLS_BTN: i32 = Y_DIAG_BTN + BTN_H as i32 + 6;
const Y_VOLUME_LABEL: i32 = Y_CONTROLS_BTN + BTN_H as i32 + 10;
const Y_VOLUME: i32 = Y_VOLUME_LABEL + ROW_H + 2;
const Y_SEPARATOR: i32 = Y_VOLUME + ROW_H + 8;
const Y_SESSION_BTNS: i32 = Y_SEPARATOR + 10;
const Y_RETURN_BTN: i32 = Y_SESSION_BTNS + BTN_H as i32 + 6;

/// Total panel height needed to fit all main-panel controls.
pub const SETTINGS_PANEL_H: u32 = (Y_RETURN_BTN + BTN_H as i32 + 8) as u32;

// ---------------------------------------------------------------------------
// Layout constants — Display sub-panel
// ---------------------------------------------------------------------------

const DS_ROW_H: i32 = 14;
const DS_Y_SHADOWS: i32 = TITLE_BAR_H + 8;
const DS_Y_SPELL_FX: i32 = DS_Y_SHADOWS + DS_ROW_H;
const DS_Y_NAMES: i32 = DS_Y_SPELL_FX + DS_ROW_H;
const DS_Y_HEALTH: i32 = DS_Y_NAMES + DS_ROW_H;
const DS_Y_HELPER_TEXT: i32 = DS_Y_HEALTH + DS_ROW_H;
const DS_Y_WALLS: i32 = DS_Y_HELPER_TEXT + DS_ROW_H;
const DS_Y_SEP: i32 = DS_Y_WALLS + DS_ROW_H + 4;
const DS_Y_DISPLAY_MODE: i32 = DS_Y_SEP + 8;
const DS_Y_PIXEL_PERFECT: i32 = DS_Y_DISPLAY_MODE + 20;
const DS_Y_VSYNC: i32 = DS_Y_PIXEL_PERFECT + DS_ROW_H;
const DS_PANEL_H: u32 = (DS_Y_VSYNC + DS_ROW_H + 10 + BTN_H as i32 + 8) as u32;

// ---------------------------------------------------------------------------
// Layout constants — Diagnostics sub-panel
// ---------------------------------------------------------------------------

const DG_Y_SHOW_POS: i32 = TITLE_BAR_H + 8;
const DG_Y_PING: i32 = DG_Y_SHOW_POS + ROW_H + 4;
const DG_Y_PROFILER_BTN: i32 = DG_Y_PING + ROW_H + 6;
const DG_Y_LOGDIR_BTN: i32 = DG_Y_PROFILER_BTN + BTN_H as i32 + 6;
const DG_PANEL_H: u32 = (DG_Y_LOGDIR_BTN + BTN_H as i32 + 10 + BTN_H as i32 + 8) as u32;

// ---------------------------------------------------------------------------
// Layout constants — Controls sub-panel
// ---------------------------------------------------------------------------

const CT_ROW_H: i32 = 20;
const CT_BTN_W: u32 = 120;
const CT_Y_FIRST_ROW: i32 = TITLE_BAR_H + 8;

const fn controls_panel_height(action_count: usize) -> u32 {
    (CT_Y_FIRST_ROW + CT_ROW_H * action_count as i32 + 10 + BTN_H as i32 + 8) as u32
}

const CT_PANEL_H: u32 = controls_panel_height(GameAction::ALL.len());

// ---------------------------------------------------------------------------
// Which sub-panel is active
// ---------------------------------------------------------------------------

/// Identifies which settings sub-panel is currently showing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SettingsSubPanel {
    /// Display (merged Visual + Display).
    Display,
    /// Diagnostics (ping, profiler, log dir, pixel positions).
    Diagnostics,
    /// Controls (keyboard bindings).
    Controls,
}

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

/// Standard button background used in the settings UI.
fn btn_bg() -> Background {
    Background::SolidColor(Color::RGBA(40, 40, 60, 200))
}

/// Standard button border used in the settings UI.
fn btn_border() -> Border {
    Border {
        color: Color::RGBA(120, 120, 140, 200),
        width: 1,
    }
}

/// Semi-transparent dark background used by sub-panels.
const SUB_PANEL_BG: Color = Color::RGBA(10, 10, 30, 220);
/// Border color shared by all panels.
const BORDER_COLOR: Color = Color::RGBA(120, 120, 140, 200);
/// Grey-out overlay applied to the main settings panel while a sub-panel is open.
const SUB_PANEL_DIM_OVERLAY: Color = Color::RGBA(90, 90, 100, 120);

/// Shorthand to shift a widget by a pixel delta.
fn shift(w: &mut impl Widget, dx: i32, dy: i32) {
    let b = w.bounds();
    let (nx, ny) = (b.x + dx, b.y + dy);
    w.set_position(nx, ny);
}

/// Returns `Consumed` for mouse events inside `bounds` to block pass-through.
fn consume_mouse_events_in_bounds(bounds: &Bounds, event: &UiEvent) -> EventResponse {
    match event {
        UiEvent::MouseClick { x, y, .. }
        | UiEvent::MouseDown { x, y, .. }
        | UiEvent::MouseMove { x, y }
        | UiEvent::MouseWheel { x, y, .. } => {
            if bounds.contains_point(*x, *y) {
                EventResponse::Consumed
            } else {
                EventResponse::Ignored
            }
        }
        _ => EventResponse::Ignored,
    }
}

/// Returns the top-left origin for a sub-panel centered on `parent_bounds`.
fn centered_sub_panel_origin(
    parent_bounds: &Bounds,
    panel_width: u32,
    panel_height: u32,
) -> (i32, i32) {
    let x = parent_bounds.x + (parent_bounds.width as i32 - panel_width as i32) / 2;
    let y = parent_bounds.y + (parent_bounds.height as i32 - panel_height as i32) / 2;
    (x, y)
}

/// Returns the display label for a keyboard binding button.
fn keybinding_button_label(bindings: &KeyBindings, action: GameAction) -> String {
    bindings
        .binding_for(action)
        .map(|binding| binding.to_string())
        .unwrap_or_else(|| "Unbound".to_string())
}

/// Draws a centered section header string.
fn draw_section_header(
    ctx: &mut RenderContext,
    text: &str,
    center_x: i32,
    y: i32,
) -> Result<(), String> {
    font_cache::draw_text(
        ctx.canvas,
        ctx.gfx,
        1,
        text,
        center_x,
        y,
        font_cache::TextStyle::centered(),
    )
}

/// Draw a sub-panel background and border rectangle.
fn draw_sub_panel_frame(
    ctx: &mut RenderContext,
    bounds: &Bounds,
    bg_color: Color,
    border_color: Color,
) -> Result<(), String> {
    let rect = sdl2::rect::Rect::new(bounds.x, bounds.y, bounds.width, bounds.height);
    ctx.canvas.set_blend_mode(BlendMode::Blend);
    ctx.canvas.set_draw_color(bg_color);
    ctx.canvas.fill_rect(rect)?;
    ctx.canvas.set_draw_color(border_color);
    ctx.canvas.draw_rect(rect)?;
    Ok(())
}

// ===========================================================================
// DisplaySettingsSubPanel
// ===========================================================================

/// Sub-panel for display/visual settings.
///
/// Contains visual toggles (shadows, spell effects, names, health, helper
/// text, hide walls) and display controls (mode, pixel-perfect scaling,
/// VSync).
struct DisplaySettingsSubPanel {
    bounds: Bounds,
    visible: bool,
    title_bar: TitleBar,
    chk_shadows: Checkbox,
    chk_spell_effects: Checkbox,
    chk_show_names: Checkbox,
    chk_show_health: Checkbox,
    chk_helper_text: Checkbox,
    chk_hide_walls: Checkbox,
    drp_display_mode: Dropdown,
    chk_pixel_perfect: Checkbox,
    chk_vsync: Checkbox,
    btn_close: RectButton,
    pending_actions: Vec<WidgetAction>,
}

impl DisplaySettingsSubPanel {
    /// Creates a new display settings sub-panel positioned relative to `origin`.
    ///
    /// # Arguments
    ///
    /// * `origin_x` - Left edge of the sub-panel.
    /// * `origin_y` - Top edge of the sub-panel.
    /// * `width` - Panel width.
    ///
    /// # Returns
    ///
    /// A new `DisplaySettingsSubPanel`, initially hidden.
    fn new(origin_x: i32, origin_y: i32, width: u32) -> Self {
        let x = origin_x + H_INSET;
        let w = CONTROL_W.min(width.saturating_sub(H_INSET as u32 * 2));
        let close_y = origin_y + DS_PANEL_H as i32 - BTN_H as i32 - 8;
        Self {
            bounds: Bounds::new(origin_x, origin_y, width, DS_PANEL_H),
            visible: false,
            title_bar: TitleBar::new_static("Display Settings", origin_x, origin_y, width),
            chk_shadows: Checkbox::new(
                Bounds::new(x, origin_y + DS_Y_SHADOWS, w, DS_ROW_H as u32),
                "Enable Shadows",
                0,
            ),
            chk_spell_effects: Checkbox::new(
                Bounds::new(x, origin_y + DS_Y_SPELL_FX, w, DS_ROW_H as u32),
                "Enable Spell Effects",
                0,
            ),
            chk_show_names: Checkbox::new(
                Bounds::new(x, origin_y + DS_Y_NAMES, w, DS_ROW_H as u32),
                "Show Names",
                0,
            ),
            chk_show_health: Checkbox::new(
                Bounds::new(x, origin_y + DS_Y_HEALTH, w, DS_ROW_H as u32),
                "Show % Health",
                0,
            ),
            chk_helper_text: Checkbox::new(
                Bounds::new(x, origin_y + DS_Y_HELPER_TEXT, w, DS_ROW_H as u32),
                "Show Helper Text",
                0,
            ),
            chk_hide_walls: Checkbox::new(
                Bounds::new(x, origin_y + DS_Y_WALLS, w, DS_ROW_H as u32),
                "Hide Walls",
                0,
            ),
            drp_display_mode: Dropdown::new(
                Bounds::new(x, origin_y + DS_Y_DISPLAY_MODE, w, 16),
                DisplayMode::ALL.iter().map(|m| m.to_string()).collect(),
                0,
                0,
            ),
            chk_pixel_perfect: Checkbox::new(
                Bounds::new(x, origin_y + DS_Y_PIXEL_PERFECT, w, DS_ROW_H as u32),
                "Pixel-Perfect Scaling",
                0,
            ),
            chk_vsync: Checkbox::new(
                Bounds::new(x, origin_y + DS_Y_VSYNC, w, DS_ROW_H as u32),
                "VSync",
                0,
            ),
            btn_close: RectButton::new(Bounds::new(x, close_y, w, BTN_H), btn_bg())
                .with_label("Close", 0)
                .with_border(btn_border()),
            pending_actions: Vec::new(),
        }
    }

    /// Loads widget values from the data snapshot.
    ///
    /// # Arguments
    ///
    /// * `data` - Snapshot of current settings values.
    fn sync_state(&mut self, data: &SettingsPanelData) {
        self.chk_shadows.set_checked(data.shadows_enabled);
        self.chk_spell_effects
            .set_checked(data.spell_effects_enabled);
        self.chk_show_names.set_checked(data.show_names);
        self.chk_show_health.set_checked(data.show_health_pct);
        self.chk_helper_text.set_checked(data.show_helper_text);
        self.chk_hide_walls.set_checked(data.hide_walls);
        self.chk_pixel_perfect
            .set_checked(data.pixel_perfect_scaling);
        self.chk_vsync.set_checked(data.vsync_enabled);

        let mode_idx = DisplayMode::ALL
            .iter()
            .position(|m| *m == data.display_mode)
            .unwrap_or(0);
        self.drp_display_mode.set_selected(mode_idx);
    }

    /// Collects `WidgetAction`s from toggled/changed children.
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

    /// Shifts all widgets by a pixel delta.
    fn shift_all(&mut self, dx: i32, dy: i32) {
        self.bounds.x += dx;
        self.bounds.y += dy;
        self.title_bar
            .set_bar_position(self.bounds.x, self.bounds.y);
        shift(&mut self.chk_shadows, dx, dy);
        shift(&mut self.chk_spell_effects, dx, dy);
        shift(&mut self.chk_show_names, dx, dy);
        shift(&mut self.chk_show_health, dx, dy);
        shift(&mut self.chk_helper_text, dx, dy);
        shift(&mut self.chk_hide_walls, dx, dy);
        shift(&mut self.drp_display_mode, dx, dy);
        shift(&mut self.chk_pixel_perfect, dx, dy);
        shift(&mut self.chk_vsync, dx, dy);
        shift(&mut self.btn_close, dx, dy);
    }

    /// Returns whether the title bar close button was pressed.
    fn was_close_requested(&mut self) -> bool {
        self.title_bar.was_close_requested()
    }

    /// Handles a UI event. Returns `Consumed` if the sub-panel ate it.
    fn handle_event(&mut self, event: &UiEvent) -> EventResponse {
        if !self.visible {
            return EventResponse::Ignored;
        }

        // Title bar (close button only — no independent drag).
        let (tb_resp, _drag) = self.title_bar.handle_event(event);
        if self.was_close_requested() {
            self.visible = false;
            return EventResponse::Consumed;
        }
        if tb_resp == EventResponse::Consumed {
            return EventResponse::Consumed;
        }

        if self.btn_close.handle_event(event) == EventResponse::Consumed {
            self.visible = false;
            return EventResponse::Consumed;
        }

        // Expanded dropdown gets priority.
        if self.drp_display_mode.is_expanded() {
            let resp = self.drp_display_mode.handle_event(event);
            self.collect_child_actions();
            if resp == EventResponse::Consumed {
                return EventResponse::Consumed;
            }
        }

        let children_responses = [
            self.chk_shadows.handle_event(event),
            self.chk_spell_effects.handle_event(event),
            self.chk_show_names.handle_event(event),
            self.chk_show_health.handle_event(event),
            self.chk_helper_text.handle_event(event),
            self.chk_hide_walls.handle_event(event),
            if !self.drp_display_mode.is_expanded() {
                self.drp_display_mode.handle_event(event)
            } else {
                EventResponse::Ignored
            },
            self.chk_pixel_perfect.handle_event(event),
            self.chk_vsync.handle_event(event),
        ];

        self.collect_child_actions();

        if children_responses
            .iter()
            .any(|r| *r == EventResponse::Consumed)
        {
            return EventResponse::Consumed;
        }

        consume_mouse_events_in_bounds(&self.bounds, event)
    }

    /// Renders the sub-panel and its children.
    fn render(&mut self, ctx: &mut RenderContext<'_, '_>) -> Result<(), String> {
        if !self.visible {
            return Ok(());
        }

        draw_sub_panel_frame(ctx, &self.bounds, SUB_PANEL_BG, BORDER_COLOR)?;
        self.title_bar.render(ctx)?;

        // Visual/display separator line.
        let sep_y = self.bounds.y + DS_Y_SEP;
        ctx.canvas.set_draw_color(Color::RGBA(120, 120, 140, 150));
        ctx.canvas.draw_line(
            sdl2::rect::Point::new(self.bounds.x + H_INSET, sep_y),
            sdl2::rect::Point::new(self.bounds.x + self.bounds.width as i32 - H_INSET, sep_y),
        )?;

        self.chk_shadows.render(ctx)?;
        self.chk_spell_effects.render(ctx)?;
        self.chk_show_names.render(ctx)?;
        self.chk_show_health.render(ctx)?;
        self.chk_helper_text.render(ctx)?;
        self.chk_hide_walls.render(ctx)?;
        self.chk_pixel_perfect.render(ctx)?;
        self.chk_vsync.render(ctx)?;
        self.btn_close.render(ctx)?;
        // Dropdown last so expanded list overlays.
        self.drp_display_mode.render(ctx)?;

        Ok(())
    }

    /// Drains pending actions.
    fn take_actions(&mut self) -> Vec<WidgetAction> {
        std::mem::take(&mut self.pending_actions)
    }
}

// ===========================================================================
// DiagnosticsSubPanel
// ===========================================================================

/// Sub-panel for diagnostic tools.
///
/// Contains the "Show Pixel Positions" toggle (moved from Visual), ping
/// readout, profiler button, and log directory button.
struct DiagnosticsSubPanel {
    bounds: Bounds,
    visible: bool,
    title_bar: TitleBar,
    chk_show_positions: Checkbox,
    lbl_ping: Label,
    btn_profiler: RectButton,
    btn_log_dir: RectButton,
    btn_close: RectButton,
    pending_actions: Vec<WidgetAction>,
}

impl DiagnosticsSubPanel {
    /// Creates a new diagnostics sub-panel positioned at the given origin.
    ///
    /// # Arguments
    ///
    /// * `origin_x` - Left edge of the sub-panel.
    /// * `origin_y` - Top edge of the sub-panel.
    /// * `width` - Panel width.
    ///
    /// # Returns
    ///
    /// A new `DiagnosticsSubPanel`, initially hidden.
    fn new(origin_x: i32, origin_y: i32, width: u32) -> Self {
        let x = origin_x + H_INSET;
        let w = CONTROL_W.min(width.saturating_sub(H_INSET as u32 * 2));
        let close_y = origin_y + DG_PANEL_H as i32 - BTN_H as i32 - 8;
        Self {
            bounds: Bounds::new(origin_x, origin_y, width, DG_PANEL_H),
            visible: false,
            title_bar: TitleBar::new_static("Diagnostics", origin_x, origin_y, width),
            chk_show_positions: Checkbox::new(
                Bounds::new(x, origin_y + DG_Y_SHOW_POS, w, ROW_H as u32),
                "Show Pixel Positions",
                0,
            ),
            lbl_ping: Label::new("Ping: N/A", 0, x, origin_y + DG_Y_PING),
            btn_profiler: RectButton::new(
                Bounds::new(x, origin_y + DG_Y_PROFILER_BTN, w, BTN_H),
                btn_bg(),
            )
            .with_label("Profile Performance", 0)
            .with_border(btn_border()),
            btn_log_dir: RectButton::new(
                Bounds::new(x, origin_y + DG_Y_LOGDIR_BTN, w, BTN_H),
                btn_bg(),
            )
            .with_label("Open Log Directory", 0)
            .with_border(btn_border()),
            btn_close: RectButton::new(Bounds::new(x, close_y, w, BTN_H), btn_bg())
                .with_label("Close", 0)
                .with_border(btn_border()),
            pending_actions: Vec::new(),
        }
    }

    /// Loads widget values from the data snapshot.
    ///
    /// # Arguments
    ///
    /// * `data` - Snapshot of current settings values.
    fn sync_state(&mut self, data: &SettingsPanelData) {
        self.chk_show_positions.set_checked(data.show_positions);
        self.update_ping(data.last_rtt_ms);
    }

    /// Updates the ping readout label.
    ///
    /// # Arguments
    ///
    /// * `rtt_ms` - Latest round-trip time in milliseconds, or `None`.
    fn update_ping(&mut self, rtt_ms: Option<u32>) {
        let text = match rtt_ms {
            Some(ms) => format!("Ping: {} ms", ms),
            None => "Ping: N/A".to_string(),
        };
        self.lbl_ping.set_text(&text);
    }

    /// Shifts all widgets by a pixel delta.
    fn shift_all(&mut self, dx: i32, dy: i32) {
        self.bounds.x += dx;
        self.bounds.y += dy;
        self.title_bar
            .set_bar_position(self.bounds.x, self.bounds.y);
        shift(&mut self.chk_show_positions, dx, dy);
        shift(&mut self.lbl_ping, dx, dy);
        shift(&mut self.btn_profiler, dx, dy);
        shift(&mut self.btn_log_dir, dx, dy);
        shift(&mut self.btn_close, dx, dy);
    }

    /// Returns whether the title bar close button was pressed.
    fn was_close_requested(&mut self) -> bool {
        self.title_bar.was_close_requested()
    }

    /// Handles a UI event. Returns `Consumed` if the sub-panel ate it.
    fn handle_event(&mut self, event: &UiEvent) -> EventResponse {
        if !self.visible {
            return EventResponse::Ignored;
        }

        let (tb_resp, _drag) = self.title_bar.handle_event(event);
        if self.was_close_requested() {
            self.visible = false;
            return EventResponse::Consumed;
        }
        if tb_resp == EventResponse::Consumed {
            return EventResponse::Consumed;
        }

        if self.btn_close.handle_event(event) == EventResponse::Consumed {
            self.visible = false;
            return EventResponse::Consumed;
        }

        if self.chk_show_positions.handle_event(event) == EventResponse::Consumed {
            if self.chk_show_positions.was_toggled() {
                self.pending_actions.push(WidgetAction::SetShowPositions(
                    self.chk_show_positions.is_checked(),
                ));
            }
            return EventResponse::Consumed;
        }

        if self.btn_profiler.handle_event(event) == EventResponse::Consumed {
            self.pending_actions.push(WidgetAction::StartProfiler);
            return EventResponse::Consumed;
        }
        if self.btn_log_dir.handle_event(event) == EventResponse::Consumed {
            self.pending_actions.push(WidgetAction::OpenLogDir);
            return EventResponse::Consumed;
        }

        consume_mouse_events_in_bounds(&self.bounds, event)
    }

    /// Renders the sub-panel and its children.
    fn render(&mut self, ctx: &mut RenderContext<'_, '_>) -> Result<(), String> {
        if !self.visible {
            return Ok(());
        }

        draw_sub_panel_frame(ctx, &self.bounds, SUB_PANEL_BG, BORDER_COLOR)?;
        self.title_bar.render(ctx)?;
        self.chk_show_positions.render(ctx)?;
        self.lbl_ping.render(ctx)?;
        self.btn_profiler.render(ctx)?;
        self.btn_log_dir.render(ctx)?;
        self.btn_close.render(ctx)?;

        Ok(())
    }

    /// Drains pending actions.
    fn take_actions(&mut self) -> Vec<WidgetAction> {
        std::mem::take(&mut self.pending_actions)
    }
}

// ===========================================================================
// ControlsSubPanel
// ===========================================================================

/// Sub-panel for control/input settings.
///
/// Contains the inline keyboard bindings editor.
struct ControlsSubPanel {
    bounds: Bounds,
    visible: bool,
    title_bar: TitleBar,
    binding_buttons: Vec<RectButton>,
    listening_for: Option<usize>,
    bindings: KeyBindings,
    btn_close: RectButton,
    pending_actions: Vec<WidgetAction>,
}

impl ControlsSubPanel {
    /// Creates a new controls sub-panel positioned at the given origin.
    ///
    /// # Arguments
    ///
    /// * `origin_x` - Left edge of the sub-panel.
    /// * `origin_y` - Top edge of the sub-panel.
    /// * `width` - Panel width.
    ///
    /// # Returns
    ///
    /// A new `ControlsSubPanel`, initially hidden.
    fn new(origin_x: i32, origin_y: i32, width: u32) -> Self {
        let btn_x = origin_x + width as i32 - H_INSET - CT_BTN_W as i32;
        let close_x = origin_x + H_INSET;
        let close_w = CONTROL_W.min(width.saturating_sub(H_INSET as u32 * 2));
        let close_y = origin_y + CT_PANEL_H as i32 - BTN_H as i32 - 8;
        let bindings = KeyBindings::default();
        let binding_buttons = GameAction::ALL
            .iter()
            .enumerate()
            .map(|(index, action)| {
                let y = origin_y + CT_Y_FIRST_ROW + CT_ROW_H * index as i32 + 2;
                RectButton::new(Bounds::new(btn_x, y, CT_BTN_W, BTN_H), btn_bg())
                    .with_label(&keybinding_button_label(&bindings, *action), 0)
                    .with_border(btn_border())
            })
            .collect();
        Self {
            bounds: Bounds::new(origin_x, origin_y, width, CT_PANEL_H),
            visible: false,
            title_bar: TitleBar::new_static("Controls", origin_x, origin_y, width),
            binding_buttons,
            listening_for: None,
            bindings,
            btn_close: RectButton::new(Bounds::new(close_x, close_y, close_w, BTN_H), btn_bg())
                .with_label("Close", 0)
                .with_border(btn_border()),
            pending_actions: Vec::new(),
        }
    }

    /// Loads widget values from the data snapshot.
    fn sync_state(&mut self, data: &SettingsPanelData) {
        self.bindings = data.key_bindings.clone();
        self.cancel_listening();
    }

    /// Rebuilds the button labels from the current bindings.
    fn refresh_button_labels(&mut self) {
        for (index, action) in GameAction::ALL.iter().enumerate() {
            if let Some(button) = self.binding_buttons.get_mut(index) {
                button.set_label(&keybinding_button_label(&self.bindings, *action));
            }
        }
    }

    /// Cancels any in-progress key listening state and restores labels.
    fn cancel_listening(&mut self) {
        self.listening_for = None;
        self.refresh_button_labels();
    }

    /// Marks the panel visible and resets transient input state.
    fn show(&mut self) {
        self.visible = true;
        self.cancel_listening();
    }

    /// Hides the panel and clears transient input state.
    fn hide(&mut self) {
        self.visible = false;
        self.cancel_listening();
    }

    /// Returns whether a key is suitable for binding.
    fn is_bindable_key(keycode: Keycode) -> bool {
        !matches!(
            keycode,
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

    /// Shifts all widgets by a pixel delta.
    fn shift_all(&mut self, dx: i32, dy: i32) {
        self.bounds.x += dx;
        self.bounds.y += dy;
        self.title_bar
            .set_bar_position(self.bounds.x, self.bounds.y);
        for button in &mut self.binding_buttons {
            shift(button, dx, dy);
        }
        shift(&mut self.btn_close, dx, dy);
    }

    /// Returns whether the title bar close button was pressed.
    fn was_close_requested(&mut self) -> bool {
        self.title_bar.was_close_requested()
    }

    /// Handles a UI event. Returns `Consumed` if the sub-panel ate it.
    fn handle_event(&mut self, event: &UiEvent) -> EventResponse {
        if !self.visible {
            return EventResponse::Ignored;
        }

        if let Some(index) = self.listening_for {
            if let UiEvent::KeyDown { keycode, modifiers } = event {
                if *keycode == Keycode::Escape {
                    self.cancel_listening();
                    return EventResponse::Consumed;
                }

                if Self::is_bindable_key(*keycode) {
                    let binding = KeyBinding::new(*keycode, *modifiers);
                    let action = GameAction::ALL[index];
                    self.bindings.set_binding(action, binding);
                    self.cancel_listening();
                    self.pending_actions
                        .push(WidgetAction::UpdateKeyBinding { action, binding });
                    return EventResponse::Consumed;
                }

                return EventResponse::Consumed;
            }

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

        let (tb_resp, _drag) = self.title_bar.handle_event(event);
        if self.was_close_requested() {
            self.hide();
            return EventResponse::Consumed;
        }
        if tb_resp == EventResponse::Consumed {
            return EventResponse::Consumed;
        }

        if self.btn_close.handle_event(event) == EventResponse::Consumed {
            self.hide();
            return EventResponse::Consumed;
        }

        for (index, button) in self.binding_buttons.iter_mut().enumerate() {
            if button.handle_event(event) == EventResponse::Consumed {
                self.listening_for = Some(index);
                if let Some(active_button) = self.binding_buttons.get_mut(index) {
                    active_button.set_label("Press a key...");
                }
                return EventResponse::Consumed;
            }
        }

        consume_mouse_events_in_bounds(&self.bounds, event)
    }

    /// Renders the sub-panel and its children.
    fn render(&mut self, ctx: &mut RenderContext<'_, '_>) -> Result<(), String> {
        if !self.visible {
            return Ok(());
        }

        draw_sub_panel_frame(ctx, &self.bounds, SUB_PANEL_BG, BORDER_COLOR)?;
        self.title_bar.render(ctx)?;

        let label_x = self.bounds.x + H_INSET;
        for (index, action) in GameAction::ALL.iter().enumerate() {
            let y = self.bounds.y + CT_Y_FIRST_ROW + CT_ROW_H * index as i32;
            font_cache::draw_text(
                ctx.canvas,
                ctx.gfx,
                0,
                action.label(),
                label_x,
                y + 3,
                font_cache::TextStyle::default(),
            )?;

            if let Some(button) = self.binding_buttons.get_mut(index) {
                button.render(ctx)?;
            }
        }

        self.btn_close.render(ctx)?;

        Ok(())
    }

    /// Drains pending actions.
    fn take_actions(&mut self) -> Vec<WidgetAction> {
        std::mem::take(&mut self.pending_actions)
    }
}

// ===========================================================================
// Data snapshot
// ===========================================================================

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
    /// Whether helper text is replaced with the cursor's logical screen position.
    pub show_positions: bool,
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
    /// Current keyboard bindings for control actions.
    pub key_bindings: KeyBindings,
}

// ===========================================================================
// SettingsPanel (main panel)
// ===========================================================================

/// The settings / options HUD panel.
///
/// Presents a compact menu of category buttons (Display,
/// Diagnostics, Controls), an inline volume slider, and session controls
/// (Disconnect, Quit, Return to Game). Each category button opens a
/// sub-panel that overlaps the main panel content.
pub struct SettingsPanel {
    bounds: Bounds,
    bg_color: Color,
    border_color: Color,
    visible: bool,
    pending_actions: Vec<WidgetAction>,

    /// Draggable title bar.
    title_bar: TitleBar,

    // --- Category buttons ---
    btn_display: RectButton,
    btn_diagnostics: RectButton,
    btn_controls: RectButton,

    // --- Inline volume ---
    sld_volume: Slider,

    // --- Session buttons ---
    btn_disconnect: RectButton,
    btn_quit: RectButton,
    btn_return: RectButton,
    /// Confirmation dialog shown before quitting.
    quit_dialog: QuitConfirmDialog,

    // --- Sub-panels ---
    active_sub_panel: Option<SettingsSubPanel>,
    sub_display: DisplaySettingsSubPanel,
    sub_diagnostics: DiagnosticsSubPanel,
    sub_controls: ControlsSubPanel,
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
        let half_w = (w.saturating_sub(10)) / 2;

        let (display_x, display_y) = centered_sub_panel_origin(&bounds, bounds.width, DS_PANEL_H);
        let (diagnostics_x, diagnostics_y) =
            centered_sub_panel_origin(&bounds, bounds.width, DG_PANEL_H);
        let (controls_x, controls_y) = centered_sub_panel_origin(&bounds, bounds.width, CT_PANEL_H);

        Self {
            bounds,
            bg_color,
            border_color: BORDER_COLOR,
            visible: false,
            pending_actions: Vec::new(),

            title_bar: TitleBar::new("Settings", bounds.x, bounds.y, bounds.width),

            btn_display: RectButton::new(
                Bounds::new(x, bounds.y + Y_DISPLAY_BTN, w, BTN_H),
                btn_bg(),
            )
            .with_label("Display", 0)
            .with_border(btn_border()),

            btn_diagnostics: RectButton::new(
                Bounds::new(x, bounds.y + Y_DIAG_BTN, w, BTN_H),
                btn_bg(),
            )
            .with_label("Diagnostics", 0)
            .with_border(btn_border()),

            btn_controls: RectButton::new(
                Bounds::new(x, bounds.y + Y_CONTROLS_BTN, w, BTN_H),
                btn_bg(),
            )
            .with_label("Controls", 0)
            .with_border(btn_border()),

            sld_volume: Slider::new(
                Bounds::new(x, bounds.y + Y_VOLUME, w, ROW_H as u32),
                "Volume",
                0.0,
                1.0,
                1.0,
                0,
            ),

            btn_disconnect: RectButton::new(
                Bounds::new(x, bounds.y + Y_SESSION_BTNS, half_w, BTN_H),
                btn_bg(),
            )
            .with_label("Disconnect", 0)
            .with_border(btn_border()),
            btn_quit: RectButton::new(
                Bounds::new(
                    x + half_w as i32 + 10,
                    bounds.y + Y_SESSION_BTNS,
                    half_w,
                    BTN_H,
                ),
                btn_bg(),
            )
            .with_label("Quit", 0)
            .with_border(btn_border()),
            btn_return: RectButton::new(
                Bounds::new(x, bounds.y + Y_RETURN_BTN, w, BTN_H),
                btn_bg(),
            )
            .with_label("Return to Game", 0)
            .with_border(btn_border()),
            quit_dialog: QuitConfirmDialog::new(),

            active_sub_panel: None,
            sub_display: DisplaySettingsSubPanel::new(display_x, display_y, bounds.width),
            sub_diagnostics: DiagnosticsSubPanel::new(diagnostics_x, diagnostics_y, bounds.width),
            sub_controls: ControlsSubPanel::new(controls_x, controls_y, bounds.width),
        }
    }

    /// Toggles the panel's visibility.
    pub fn toggle(&mut self) {
        self.visible = !self.visible;
        if !self.visible {
            self.close_active_sub_panel();
        }
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
        self.sld_volume.set_value(data.master_volume);
        self.sub_display.sync_state(data);
        self.sub_diagnostics.sync_state(data);
        self.sub_controls.sync_state(data);
    }

    /// Updates the ping readout label.
    ///
    /// Called each frame when visible so the value stays current.
    ///
    /// # Arguments
    ///
    /// * `rtt_ms` - Latest round-trip time in milliseconds, or `None`.
    pub fn update_ping(&mut self, rtt_ms: Option<u32>) {
        self.sub_diagnostics.update_ping(rtt_ms);
    }

    /// Updates the profiler button label.
    ///
    /// # Arguments
    ///
    /// * `active` - Whether the profiler is currently running.
    /// * `remaining_secs` - Seconds remaining, if active.
    pub fn update_profiler_label(&mut self, active: bool, remaining_secs: Option<u64>) {
        let _ = (active, remaining_secs);
    }

    /// Opens the given sub-panel, closing any currently open one.
    fn open_sub_panel(&mut self, panel: SettingsSubPanel) {
        self.close_active_sub_panel();
        self.active_sub_panel = Some(panel);
        match panel {
            SettingsSubPanel::Display => self.sub_display.visible = true,
            SettingsSubPanel::Diagnostics => self.sub_diagnostics.visible = true,
            SettingsSubPanel::Controls => self.sub_controls.show(),
        }
    }

    /// Closes whichever sub-panel is currently open.
    fn close_active_sub_panel(&mut self) {
        if let Some(panel) = self.active_sub_panel.take() {
            match panel {
                SettingsSubPanel::Display => self.sub_display.visible = false,
                SettingsSubPanel::Diagnostics => self.sub_diagnostics.visible = false,
                SettingsSubPanel::Controls => self.sub_controls.hide(),
            }
        }
    }

    /// Collects `WidgetAction`s from the volume slider.
    fn collect_main_actions(&mut self) {
        if self.sld_volume.was_changed() {
            self.pending_actions
                .push(WidgetAction::SetMasterVolume(self.sld_volume.value()));
        }
    }

    /// Drains actions from all sub-panels into the main pending list.
    fn collect_sub_panel_actions(&mut self) {
        self.pending_actions.extend(self.sub_display.take_actions());
        self.pending_actions
            .extend(self.sub_diagnostics.take_actions());
        self.pending_actions
            .extend(self.sub_controls.take_actions());
    }
}

impl Widget for SettingsPanel {
    fn bounds(&self) -> &Bounds {
        &self.bounds
    }

    fn set_position(&mut self, x: i32, y: i32) {
        let dx = x - self.bounds.x;
        let dy = y - self.bounds.y;
        self.bounds.x = x;
        self.bounds.y = y;
        self.title_bar.set_bar_position(x, y);

        shift(&mut self.btn_display, dx, dy);
        shift(&mut self.btn_diagnostics, dx, dy);
        shift(&mut self.btn_controls, dx, dy);
        shift(&mut self.sld_volume, dx, dy);
        shift(&mut self.btn_disconnect, dx, dy);
        shift(&mut self.btn_quit, dx, dy);
        shift(&mut self.btn_return, dx, dy);

        // Sub-panels move with the main panel.
        self.sub_display.shift_all(dx, dy);
        self.sub_diagnostics.shift_all(dx, dy);
        self.sub_controls.shift_all(dx, dy);
    }

    fn handle_event(&mut self, event: &UiEvent) -> EventResponse {
        if !self.visible {
            return EventResponse::Ignored;
        }

        // 1. Quit confirmation dialog is modal — blocks everything.
        if self.quit_dialog.is_visible() {
            self.quit_dialog.handle_event(event);
            for action in self.quit_dialog.take_actions() {
                match action {
                    QuitConfirmDialogAction::Confirm => {
                        self.pending_actions.push(WidgetAction::Quit);
                    }
                    QuitConfirmDialogAction::Cancel => {
                        self.quit_dialog.hide();
                    }
                }
            }
            return EventResponse::Consumed;
        }

        // 2. Active sub-panel gets priority over the main panel, including
        //    its own title bar close button.
        if self.active_sub_panel.is_some() {
            let resp = match self.active_sub_panel.unwrap() {
                SettingsSubPanel::Display => self.sub_display.handle_event(event),
                SettingsSubPanel::Diagnostics => self.sub_diagnostics.handle_event(event),
                SettingsSubPanel::Controls => self.sub_controls.handle_event(event),
            };
            self.collect_sub_panel_actions();

            let closed = match self.active_sub_panel.unwrap() {
                SettingsSubPanel::Display => !self.sub_display.visible,
                SettingsSubPanel::Diagnostics => !self.sub_diagnostics.visible,
                SettingsSubPanel::Controls => !self.sub_controls.visible,
            };
            if closed {
                self.active_sub_panel = None;
            }

            if resp == EventResponse::Consumed {
                return EventResponse::Consumed;
            }
        }

        // 3. Title bar: drag / close.
        let (tb_resp, drag_pos) = self.title_bar.handle_event(event);
        if let Some((nx, ny)) = drag_pos {
            let (cx, cy) = clamp_to_viewport(nx, ny, self.bounds.width, self.bounds.height);
            self.set_position(cx, cy);
            return EventResponse::Consumed;
        }
        if self.title_bar.was_close_requested() {
            self.visible = false;
            self.close_active_sub_panel();
            self.pending_actions
                .push(WidgetAction::TogglePanel(HudPanel::Settings));
            return EventResponse::Consumed;
        }
        if tb_resp == EventResponse::Consumed {
            return EventResponse::Consumed;
        }

        // 4. Category buttons.
        if self.btn_display.handle_event(event) == EventResponse::Consumed {
            if self.active_sub_panel == Some(SettingsSubPanel::Display) {
                self.close_active_sub_panel();
            } else {
                self.open_sub_panel(SettingsSubPanel::Display);
            }
            return EventResponse::Consumed;
        }
        if self.btn_diagnostics.handle_event(event) == EventResponse::Consumed {
            if self.active_sub_panel == Some(SettingsSubPanel::Diagnostics) {
                self.close_active_sub_panel();
            } else {
                self.open_sub_panel(SettingsSubPanel::Diagnostics);
            }
            return EventResponse::Consumed;
        }
        if self.btn_controls.handle_event(event) == EventResponse::Consumed {
            if self.active_sub_panel == Some(SettingsSubPanel::Controls) {
                self.close_active_sub_panel();
            } else {
                self.open_sub_panel(SettingsSubPanel::Controls);
            }
            return EventResponse::Consumed;
        }

        // 5. Volume slider.
        if self.sld_volume.handle_event(event) == EventResponse::Consumed {
            self.collect_main_actions();
            return EventResponse::Consumed;
        }

        // 6. Session buttons (beneath sub-panel overlay).
        if self.btn_disconnect.handle_event(event) == EventResponse::Consumed {
            self.pending_actions.push(WidgetAction::Disconnect);
            return EventResponse::Consumed;
        }
        if self.btn_quit.handle_event(event) == EventResponse::Consumed {
            self.quit_dialog.center_on(&self.bounds);
            self.quit_dialog.show();
            return EventResponse::Consumed;
        }
        if self.btn_return.handle_event(event) == EventResponse::Consumed {
            self.visible = false;
            self.close_active_sub_panel();
            self.pending_actions
                .push(WidgetAction::TogglePanel(HudPanel::Settings));
            return EventResponse::Consumed;
        }

        // 7. Consume any click inside panel bounds to prevent click-through.
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

        // --- Main panel background ---
        let rect = sdl2::rect::Rect::new(
            self.bounds.x,
            self.bounds.y,
            self.bounds.width,
            self.bounds.height,
        );
        ctx.canvas.set_blend_mode(BlendMode::Blend);
        ctx.canvas.set_draw_color(self.bg_color);
        ctx.canvas.fill_rect(rect)?;
        ctx.canvas.set_draw_color(self.border_color);
        ctx.canvas.draw_rect(rect)?;

        // Title bar
        self.title_bar.render(ctx)?;

        // "-- Volume --" header label
        let center_x = self.bounds.x + self.bounds.width as i32 / 2;
        draw_section_header(
            ctx,
            "-- Volume --",
            center_x,
            self.bounds.y + Y_VOLUME_LABEL,
        )?;

        // Category buttons
        self.btn_display.render(ctx)?;
        self.btn_diagnostics.render(ctx)?;
        self.btn_controls.render(ctx)?;

        // Volume slider
        self.sld_volume.render(ctx)?;

        // Separator line above session buttons
        let sep_y = self.bounds.y + Y_SEPARATOR;
        ctx.canvas.set_draw_color(Color::RGBA(120, 120, 140, 150));
        ctx.canvas.draw_line(
            sdl2::rect::Point::new(self.bounds.x + H_INSET, sep_y),
            sdl2::rect::Point::new(self.bounds.x + self.bounds.width as i32 - H_INSET, sep_y),
        )?;

        // Session buttons
        self.btn_disconnect.render(ctx)?;
        self.btn_quit.render(ctx)?;
        self.btn_return.render(ctx)?;

        if self.active_sub_panel.is_some() {
            ctx.canvas.set_blend_mode(BlendMode::Blend);
            ctx.canvas.set_draw_color(SUB_PANEL_DIM_OVERLAY);
            ctx.canvas.fill_rect(rect)?;
        }

        // --- Active sub-panel drawn ON TOP ---
        self.sub_display.render(ctx)?;
        self.sub_diagnostics.render(ctx)?;
        self.sub_controls.render(ctx)?;

        // Quit confirmation rendered topmost.
        self.quit_dialog.render(ctx)?;

        Ok(())
    }

    fn take_actions(&mut self) -> Vec<WidgetAction> {
        // Collect any remaining sub-panel actions not yet picked up.
        self.collect_sub_panel_actions();
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

    fn make_data() -> SettingsPanelData {
        SettingsPanelData {
            shadows_enabled: true,
            spell_effects_enabled: false,
            show_names: true,
            show_health_pct: true,
            hide_walls: false,
            show_helper_text: true,
            show_positions: true,
            master_volume: 0.75,
            display_mode: DisplayMode::Fullscreen,
            pixel_perfect_scaling: true,
            vsync_enabled: false,
            last_rtt_ms: Some(42),
            profiler_active: false,
            profiler_remaining_secs: None,
            key_bindings: KeyBindings::default(),
        }
    }

    fn left_click(x: i32, y: i32) -> UiEvent {
        UiEvent::MouseClick {
            x,
            y,
            button: MouseButton::Left,
            modifiers: KeyModifiers::default(),
        }
    }

    fn left_mouse_down(x: i32, y: i32) -> UiEvent {
        UiEvent::MouseDown {
            x,
            y,
            button: MouseButton::Left,
            modifiers: KeyModifiers::default(),
        }
    }

    fn mouse_move(x: i32, y: i32) -> UiEvent {
        UiEvent::MouseMove { x, y }
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
        let resp = panel.handle_event(&left_click(50, 50));
        assert_eq!(resp, EventResponse::Ignored);
    }

    #[test]
    fn visible_panel_consumes_clicks_inside() {
        let mut panel = make_panel();
        panel.toggle();
        let resp = panel.handle_event(&left_click(50, 50));
        assert_eq!(resp, EventResponse::Consumed);
    }

    #[test]
    fn visible_panel_ignores_clicks_outside() {
        let mut panel = make_panel();
        panel.toggle();
        let resp = panel.handle_event(&left_click(500, 500));
        assert_eq!(resp, EventResponse::Ignored);
    }

    #[test]
    fn sync_state_populates_sub_panels() {
        let mut panel = make_panel();
        panel.sync_state(&make_data());
        // Display sub-panel checkboxes.
        assert!(panel.sub_display.chk_shadows.is_checked());
        assert!(!panel.sub_display.chk_spell_effects.is_checked());
        assert!(panel.sub_display.chk_show_names.is_checked());
        assert!(panel.sub_display.chk_show_health.is_checked());
        assert!(!panel.sub_display.chk_hide_walls.is_checked());
        assert!(panel.sub_display.chk_helper_text.is_checked());
        assert_eq!(panel.sub_display.drp_display_mode.selected_index(), 1);
        assert!(panel.sub_display.chk_pixel_perfect.is_checked());
        assert!(!panel.sub_display.chk_vsync.is_checked());
        // Diagnostics sub-panel.
        assert!(panel.sub_diagnostics.chk_show_positions.is_checked());
        // Volume on main panel.
        assert!((panel.sld_volume.value() - 0.75).abs() < 0.01);
    }

    #[test]
    fn display_button_opens_sub_panel() {
        let mut panel = make_panel();
        panel.toggle();
        // Click the "Display" button.
        let btn_y = Y_DISPLAY_BTN + 5;
        let resp = panel.handle_event(&left_click(15, btn_y));
        assert_eq!(resp, EventResponse::Consumed);
        assert_eq!(panel.active_sub_panel, Some(SettingsSubPanel::Display));
        assert!(panel.sub_display.visible);
    }

    #[test]
    fn controls_sub_panel_is_centered_on_main_panel() {
        let mut panel = make_panel();
        panel.toggle();
        panel.handle_event(&left_click(15, Y_CONTROLS_BTN + 5));

        let panel_center_x = panel.bounds.x + panel.bounds.width as i32 / 2;
        let panel_center_y = panel.bounds.y + panel.bounds.height as i32 / 2;
        let sub_bounds = panel.sub_controls.bounds;
        let sub_center_x = sub_bounds.x + sub_bounds.width as i32 / 2;
        let sub_center_y = sub_bounds.y + sub_bounds.height as i32 / 2;

        assert_eq!(sub_center_x, panel_center_x);
        assert_eq!(sub_center_y, panel_center_y);
    }

    #[test]
    fn only_one_sub_panel_at_a_time() {
        let mut panel = make_panel();
        panel.open_sub_panel(SettingsSubPanel::Display);
        assert!(panel.sub_display.visible);
        panel.open_sub_panel(SettingsSubPanel::Diagnostics);
        assert!(!panel.sub_display.visible);
        assert!(panel.sub_diagnostics.visible);
        assert_eq!(panel.active_sub_panel, Some(SettingsSubPanel::Diagnostics));
    }

    #[test]
    fn disconnect_button_emits_action() {
        let mut panel = make_panel();
        panel.toggle();
        let btn_y = Y_SESSION_BTNS + 5;
        panel.handle_event(&left_click(15, btn_y));
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
    fn toggle_off_closes_sub_panel() {
        let mut panel = make_panel();
        panel.toggle();
        panel.handle_event(&left_click(15, Y_CONTROLS_BTN + 5));
        assert!(panel.sub_controls.visible);
        // Toggle the whole settings panel off.
        panel.toggle();
        assert!(!panel.sub_controls.visible);
        assert_eq!(panel.active_sub_panel, None);
    }

    #[test]
    fn take_actions_drains() {
        let mut panel = make_panel();
        panel.toggle();
        panel.handle_event(&left_click(15, Y_SESSION_BTNS + 5));
        let first = panel.take_actions();
        assert!(!first.is_empty());
        let second = panel.take_actions();
        assert!(second.is_empty());
    }

    #[test]
    fn display_sub_panel_checkbox_emits_action() {
        let mut panel = make_panel();
        panel.toggle();
        // Open display sub-panel.
        panel.handle_event(&left_click(15, Y_DISPLAY_BTN + 5));
        assert!(panel.sub_display.visible);
        // Drain any actions from the first click.
        let _ = panel.take_actions();
        // Click the shadows checkbox using its actual bounds.
        let chk_b = *panel.sub_display.chk_shadows.bounds();
        let resp = panel.handle_event(&left_click(chk_b.x + 5, chk_b.y + 2));
        assert_eq!(resp, EventResponse::Consumed);
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
    fn diagnostics_pixel_positions_emits_action() {
        let mut panel = make_panel();
        panel.toggle();
        // Open diagnostics sub-panel.
        panel.handle_event(&left_click(15, Y_DIAG_BTN + 5));
        assert!(panel.sub_diagnostics.visible);
        let _ = panel.take_actions();
        // Click the "Show Pixel Positions" checkbox using actual bounds.
        let chk_b = *panel.sub_diagnostics.chk_show_positions.bounds();
        panel.handle_event(&left_click(chk_b.x + 5, chk_b.y + 2));
        let actions = panel.take_actions();
        assert!(
            actions
                .iter()
                .any(|a| matches!(a, WidgetAction::SetShowPositions(true))),
            "Expected SetShowPositions action, got {:?}",
            actions
        );
    }

    #[test]
    fn controls_keybindings_emits_update_action() {
        let mut panel = make_panel();
        panel.toggle();
        panel.sync_state(&make_data());
        // Open controls sub-panel.
        panel.handle_event(&left_click(15, Y_CONTROLS_BTN + 5));
        assert!(panel.sub_controls.visible);
        let _ = panel.take_actions();
        let btn_b = *panel.sub_controls.binding_buttons[0].bounds();
        panel.handle_event(&left_click(btn_b.x + 5, btn_b.y + 2));
        let resp = panel.handle_event(&UiEvent::KeyDown {
            keycode: Keycode::K,
            modifiers: KeyModifiers {
                ctrl: true,
                shift: false,
                alt: false,
            },
        });

        assert_eq!(resp, EventResponse::Consumed);
        let actions = panel.take_actions();
        assert!(
            actions.iter().any(|action| matches!(
                action,
                WidgetAction::UpdateKeyBinding {
                    action: GameAction::ToggleSkills,
                    binding
                } if binding.keycode == i32::from(Keycode::K) && binding.modifiers.ctrl
            )),
            "Expected UpdateKeyBinding action, got {:?}",
            actions
        );
    }

    #[test]
    fn sub_panel_title_bar_is_not_draggable() {
        let mut panel = make_panel();
        panel.toggle();
        panel.handle_event(&left_click(15, Y_DISPLAY_BTN + 5));

        let start_bounds = panel.sub_display.bounds;
        let title_bar_x = start_bounds.x + 24;
        let title_bar_y = start_bounds.y + 6;

        let down_resp = panel.handle_event(&left_mouse_down(title_bar_x, title_bar_y));
        let move_resp = panel.handle_event(&mouse_move(title_bar_x + 40, title_bar_y + 20));

        assert_eq!(down_resp, EventResponse::Consumed);
        assert_eq!(move_resp, EventResponse::Consumed);
        assert_eq!(panel.sub_display.bounds, start_bounds);
        assert!(!panel.sub_display.title_bar.is_dragging());
    }

    #[test]
    fn sub_panel_bottom_close_button_closes_panel() {
        let mut panel = make_panel();
        panel.toggle();
        panel.handle_event(&left_click(15, Y_DISPLAY_BTN + 5));
        assert_eq!(panel.active_sub_panel, Some(SettingsSubPanel::Display));

        let close_bounds = *panel.sub_display.btn_close.bounds();
        let resp = panel.handle_event(&left_click(close_bounds.x + 5, close_bounds.y + 5));

        assert_eq!(resp, EventResponse::Consumed);
        assert!(!panel.sub_display.visible);
        assert_eq!(panel.active_sub_panel, None);
    }

    #[test]
    fn sub_panel_title_bar_close_does_not_close_main_panel() {
        let mut panel = make_panel();
        panel.toggle();
        panel.handle_event(&left_click(15, Y_DISPLAY_BTN + 5));

        let sub_bounds = panel.sub_display.bounds;
        let close_x = sub_bounds.x + sub_bounds.width as i32 - 6;
        let close_y = sub_bounds.y + 5;
        let resp = panel.handle_event(&left_click(close_x, close_y));

        assert_eq!(resp, EventResponse::Consumed);
        assert!(panel.is_visible());
        assert!(!panel.sub_display.visible);
        assert_eq!(panel.active_sub_panel, None);
    }

    #[test]
    fn sub_panel_mouse_down_does_not_reach_volume_slider() {
        let mut panel = make_panel();
        panel.toggle();
        panel.sync_state(&make_data());
        panel.handle_event(&left_click(15, Y_DISPLAY_BTN + 5));

        let slider_bounds = *panel.sld_volume.bounds();
        let original_value = panel.sld_volume.value();
        let resp = panel.handle_event(&left_mouse_down(
            slider_bounds.x + slider_bounds.width as i32 - 4,
            slider_bounds.y + slider_bounds.height as i32 / 2,
        ));

        assert_eq!(resp, EventResponse::Consumed);
        assert_eq!(panel.sld_volume.value(), original_value);
        assert!(!panel.sld_volume.was_changed());
    }

    #[test]
    fn sub_panel_mouse_move_does_not_hover_underlying_buttons() {
        let mut panel = make_panel();
        panel.toggle();
        panel.handle_event(&left_click(15, Y_DISPLAY_BTN + 5));

        let btn_bounds = *panel.btn_disconnect.bounds();
        let resp = panel.handle_event(&mouse_move(btn_bounds.x + 5, btn_bounds.y + 5));

        assert_eq!(resp, EventResponse::Consumed);
        assert!(!panel.btn_disconnect.is_hovered());
        assert!(!panel.btn_quit.is_hovered());
        assert!(!panel.btn_return.is_hovered());
    }

    #[test]
    fn quit_dialog_centers_on_settings_panel() {
        let mut panel = make_panel();
        panel.toggle();

        let quit_bounds = *panel.btn_quit.bounds();
        let resp = panel.handle_event(&left_click(quit_bounds.x + 5, quit_bounds.y + 5));

        assert_eq!(resp, EventResponse::Consumed);
        assert!(panel.quit_dialog.is_visible());

        let panel_center_x = panel.bounds.x + panel.bounds.width as i32 / 2;
        let panel_center_y = panel.bounds.y + panel.bounds.height as i32 / 2;
        let dialog_bounds = *panel.quit_dialog.bounds();
        let dialog_center_x = dialog_bounds.x + dialog_bounds.width as i32 / 2;
        let dialog_center_y = dialog_bounds.y + dialog_bounds.height as i32 / 2;

        assert_eq!(dialog_center_x, panel_center_x);
        assert_eq!(dialog_center_y, panel_center_y);
    }
}
