//! "Autoloot Settings" sub-panel of the settings menu.
//!
//! Presents a master enable toggle followed by one row per
//! [`AutolootCategories`] entry (checkbox on the left).  The ratling and
//! greenling rows additionally carry a dropdown selecting the minimum
//! creature rank whose eye is taken.  Every change emits a
//! [`WidgetAction::SetAutolootConfig`] (or
//! [`WidgetAction::SetAutolootEnabled`] for the master toggle) that the game
//! scene persists to the character profile and uploads to the server.

use mag_core::autoloot::{AutolootCategories, AutolootConfig, EyeRank};
use sdl2::pixels::Color;

use crate::ui::RenderContext;
use crate::ui::widget::{Bounds, EventResponse, UiEvent, Widget, WidgetAction};
use crate::ui::widgets::button::RectButton;
use crate::ui::widgets::checkbox::Checkbox;
use crate::ui::widgets::dropdown::Dropdown;
use crate::ui::widgets::label::Label;
use crate::ui::widgets::title_bar::{TITLE_BAR_H, TitleBar};

use super::settings_panel::{
    BORDER_COLOR, BTN_H, CONTROL_W, H_INSET, ROW_H, SUB_PANEL_BG, btn_bg, btn_border,
    consume_mouse_events_in_bounds, draw_sub_panel_frame, shift,
};

/// Height of one category row (tall enough for a dropdown header).
const AL_ROW_H: i32 = 18;
/// Width of the minimum-rank dropdowns on the eye rows.
const AL_RANK_W: u32 = 124;
/// Gap between a row's checkbox and its dropdown.
const AL_GAP: i32 = 6;
/// Y of the master enable checkbox.
const AL_Y_ENABLE: i32 = TITLE_BAR_H + 8;
/// Y of the separator under the master toggle.
const AL_Y_SEP: i32 = AL_Y_ENABLE + ROW_H + 4;
/// Y of the column header row.
const AL_Y_HEADER: i32 = AL_Y_SEP + 6;
/// Y of the first category row.
const AL_Y_FIRST_ROW: i32 = AL_Y_HEADER + ROW_H + 2;

/// Total sub-panel height.
pub(super) const AL_PANEL_H: u32 =
    (AL_Y_FIRST_ROW + AL_ROW_H * AutolootCategories::ALL.len() as i32 + 10 + BTN_H as i32 + 8)
        as u32;

/// Index of the ratling row within [`AutolootCategories::ALL`].
const RATLING_ROW: usize = 7;
/// Index of the greenling row within [`AutolootCategories::ALL`].
const GREENLING_ROW: usize = 8;

/// Builds the option labels for a minimum-rank dropdown.
///
/// # Arguments
///
/// * `creature` - Creature name used as the label prefix.
///
/// # Returns
///
/// * One label per [`EyeRank`], lowest first.
fn rank_options(creature: &str) -> Vec<String> {
    EyeRank::ALL
        .iter()
        .map(|rank| rank.label(creature))
        .collect()
}

/// Sub-panel editing the per-character grave auto-loot configuration.
pub(super) struct AutolootSettingsSubPanel {
    bounds: Bounds,
    pub(super) visible: bool,
    title_bar: TitleBar,
    chk_enabled: Checkbox,
    lbl_header_category: Label,
    lbl_header_rank: Label,
    /// One checkbox per entry of [`AutolootCategories::ALL`].
    chk_categories: Vec<Checkbox>,
    drp_ratling_rank: Dropdown,
    drp_greenling_rank: Dropdown,
    btn_close: RectButton,
    /// Current configuration mirrored from the widgets.
    config: AutolootConfig,
    pending_actions: Vec<WidgetAction>,
    /// Controller focus index: `0` = master toggle, `1..=10` = category rows,
    /// `11` = ratling rank, `12` = greenling rank, `13` = close.
    controller_focused: Option<usize>,
}

impl AutolootSettingsSubPanel {
    /// Focus index of the ratling rank dropdown.
    const FOCUS_RATLING_RANK: usize = 1 + AutolootCategories::ALL.len();
    /// Focus index of the greenling rank dropdown.
    const FOCUS_GREENLING_RANK: usize = Self::FOCUS_RATLING_RANK + 1;
    /// Focus index of the close button.
    const FOCUS_CLOSE: usize = Self::FOCUS_GREENLING_RANK + 1;
    /// Number of focusable elements.
    const FOCUSABLE_COUNT: usize = Self::FOCUS_CLOSE + 1;

    /// Creates a new auto-loot settings sub-panel at the given origin.
    ///
    /// # Arguments
    ///
    /// * `origin_x` - Left edge of the sub-panel.
    /// * `origin_y` - Top edge of the sub-panel.
    /// * `width` - Panel width.
    ///
    /// # Returns
    ///
    /// * A new `AutolootSettingsSubPanel`, initially hidden.
    pub(super) fn new(origin_x: i32, origin_y: i32, width: u32) -> Self {
        let x = origin_x + H_INSET;
        let w = CONTROL_W.min(width.saturating_sub(H_INSET as u32 * 2));
        let close_y = origin_y + AL_PANEL_H as i32 - BTN_H as i32 - 8;
        let rank_x = x + w as i32 - AL_RANK_W as i32;
        let chk_w = (w as i32 - AL_RANK_W as i32 - AL_GAP).max(40) as u32;

        let chk_categories = AutolootCategories::ALL
            .iter()
            .enumerate()
            .map(|(row, category)| {
                let y = origin_y + AL_Y_FIRST_ROW + AL_ROW_H * row as i32;
                let width = if row == RATLING_ROW || row == GREENLING_ROW {
                    chk_w
                } else {
                    w
                };
                Checkbox::new(
                    Bounds::new(x, y + 2, width, ROW_H as u32),
                    category.label(),
                    0,
                )
            })
            .collect();

        let ratling_y = origin_y + AL_Y_FIRST_ROW + AL_ROW_H * RATLING_ROW as i32;
        let greenling_y = origin_y + AL_Y_FIRST_ROW + AL_ROW_H * GREENLING_ROW as i32;

        Self {
            bounds: Bounds::new(origin_x, origin_y, width, AL_PANEL_H),
            visible: false,
            title_bar: TitleBar::new_static("Autoloot Settings", origin_x, origin_y, width),
            chk_enabled: Checkbox::new(
                Bounds::new(x, origin_y + AL_Y_ENABLE, w, ROW_H as u32),
                "Enable Auto-Loot (adjacent graves)",
                0,
            ),
            lbl_header_category: Label::with_color(
                "Category",
                0,
                x,
                origin_y + AL_Y_HEADER,
                Color::RGB(180, 180, 200),
            ),
            lbl_header_rank: Label::with_color(
                "Minimum Rank",
                0,
                rank_x,
                origin_y + AL_Y_HEADER,
                Color::RGB(180, 180, 200),
            ),
            chk_categories,
            // The eye rows sit near the bottom of the panel, so their lists
            // open upward to stay on screen.
            drp_ratling_rank: Dropdown::new(
                Bounds::new(rank_x, ratling_y, AL_RANK_W, BTN_H),
                rank_options("Ratling"),
                0,
                0,
            )
            .with_opens_upward(true),
            drp_greenling_rank: Dropdown::new(
                Bounds::new(rank_x, greenling_y, AL_RANK_W, BTN_H),
                rank_options("Greenling"),
                0,
                0,
            )
            .with_opens_upward(true),
            btn_close: RectButton::new(Bounds::new(x, close_y, w, BTN_H), btn_bg())
                .with_label("Close", 0)
                .with_border(btn_border()),
            config: AutolootConfig::default(),
            pending_actions: Vec::new(),
            controller_focused: None,
        }
    }

    /// Marks the panel visible.
    pub(super) fn show(&mut self) {
        self.visible = true;
    }

    /// Hides the panel and clears controller focus.
    pub(super) fn hide(&mut self) {
        self.visible = false;
        self.controller_focused = None;
        self.apply_controller_focus();
    }

    /// Loads widget values from the current settings.
    ///
    /// # Arguments
    ///
    /// * `enabled` - Whether auto-loot is active for the character.
    /// * `config` - Current category configuration.
    pub(super) fn sync_state(&mut self, enabled: bool, config: &AutolootConfig) {
        self.config = *config;
        self.chk_enabled.set_checked(enabled);
        for (chk, category) in self
            .chk_categories
            .iter_mut()
            .zip(AutolootCategories::ALL.iter())
        {
            chk.set_checked(config.has(*category));
        }
        self.drp_ratling_rank
            .set_selected(config.ratling_min_rank as usize);
        self.drp_greenling_rank
            .set_selected(config.greenling_min_rank as usize);
    }

    /// Returns the configuration currently shown by the widgets.
    ///
    /// # Returns
    ///
    /// * The mirrored [`AutolootConfig`].
    #[cfg(test)]
    pub(super) fn config(&self) -> AutolootConfig {
        self.config
    }

    /// Returns the sub-panel bounds.
    #[cfg(test)]
    pub(super) fn bounds(&self) -> &Bounds {
        &self.bounds
    }

    /// Applies controller focus highlighting.
    fn apply_controller_focus(&mut self) {
        let f = self.controller_focused;
        self.chk_enabled.set_hovered(f == Some(0));
        for (i, chk) in self.chk_categories.iter_mut().enumerate() {
            chk.set_hovered(f == Some(1 + i));
        }
        self.drp_ratling_rank
            .set_hovered(f == Some(Self::FOCUS_RATLING_RANK));
        self.drp_greenling_rank
            .set_hovered(f == Some(Self::FOCUS_GREENLING_RANK));
        self.btn_close.set_hovered(f == Some(Self::FOCUS_CLOSE));
    }

    /// Queues a `SetAutolootConfig` action carrying the current config.
    fn push_config_action(&mut self) {
        self.pending_actions
            .push(WidgetAction::SetAutolootConfig(self.config));
    }

    /// Collects `WidgetAction`s from toggled/changed children.
    fn collect_child_actions(&mut self) {
        if self.chk_enabled.was_toggled() {
            self.pending_actions.push(WidgetAction::SetAutolootEnabled(
                self.chk_enabled.is_checked(),
            ));
        }

        let mut changed = false;
        for (chk, category) in self
            .chk_categories
            .iter_mut()
            .zip(AutolootCategories::ALL.iter())
        {
            if chk.was_toggled() {
                self.config.set(*category, chk.is_checked());
                changed = true;
            }
        }
        if self.drp_ratling_rank.was_changed() {
            self.config.ratling_min_rank =
                EyeRank::from_u8(self.drp_ratling_rank.selected_index() as u8);
            changed = true;
        }
        if self.drp_greenling_rank.was_changed() {
            self.config.greenling_min_rank =
                EyeRank::from_u8(self.drp_greenling_rank.selected_index() as u8);
            changed = true;
        }
        if changed {
            self.push_config_action();
        }
    }

    /// Shifts all widgets by a pixel delta.
    pub(super) fn shift_all(&mut self, dx: i32, dy: i32) {
        self.bounds.x += dx;
        self.bounds.y += dy;
        self.title_bar
            .set_bar_position(self.bounds.x, self.bounds.y);
        shift(&mut self.chk_enabled, dx, dy);
        shift(&mut self.lbl_header_category, dx, dy);
        shift(&mut self.lbl_header_rank, dx, dy);
        for chk in &mut self.chk_categories {
            shift(chk, dx, dy);
        }
        shift(&mut self.drp_ratling_rank, dx, dy);
        shift(&mut self.drp_greenling_rank, dx, dy);
        shift(&mut self.btn_close, dx, dy);
    }

    /// Cycles a rank dropdown to its next option via controller confirm.
    fn cycle_rank(dropdown: &mut Dropdown) -> EyeRank {
        let next = (dropdown.selected_index() + 1) % EyeRank::ALL.len();
        dropdown.set_selected(next);
        EyeRank::from_u8(next as u8)
    }

    /// Handles controller navigation events.
    ///
    /// # Returns
    ///
    /// * `Some(response)` when the event was a navigation event.
    fn handle_nav_event(&mut self, event: &UiEvent) -> Option<EventResponse> {
        match event {
            UiEvent::NavNext => {
                self.controller_focused = Some(match self.controller_focused {
                    None => 0,
                    Some(i) => (i + 1) % Self::FOCUSABLE_COUNT,
                });
                self.apply_controller_focus();
                Some(EventResponse::Consumed)
            }
            UiEvent::NavPrev => {
                self.controller_focused = Some(match self.controller_focused {
                    None | Some(0) => Self::FOCUSABLE_COUNT - 1,
                    Some(i) => i - 1,
                });
                self.apply_controller_focus();
                Some(EventResponse::Consumed)
            }
            UiEvent::NavConfirm => {
                match self.controller_focused {
                    Some(0) => {
                        let v = !self.chk_enabled.is_checked();
                        self.chk_enabled.set_checked(v);
                        self.pending_actions
                            .push(WidgetAction::SetAutolootEnabled(v));
                    }
                    Some(i) if (1..Self::FOCUS_RATLING_RANK).contains(&i) => {
                        let category = AutolootCategories::ALL[i - 1];
                        let v = !self.chk_categories[i - 1].is_checked();
                        self.chk_categories[i - 1].set_checked(v);
                        self.config.set(category, v);
                        self.push_config_action();
                    }
                    Some(Self::FOCUS_RATLING_RANK) => {
                        self.config.ratling_min_rank = Self::cycle_rank(&mut self.drp_ratling_rank);
                        self.push_config_action();
                    }
                    Some(Self::FOCUS_GREENLING_RANK) => {
                        self.config.greenling_min_rank =
                            Self::cycle_rank(&mut self.drp_greenling_rank);
                        self.push_config_action();
                    }
                    Some(Self::FOCUS_CLOSE) => self.hide(),
                    _ => {}
                }
                self.apply_controller_focus();
                Some(EventResponse::Consumed)
            }
            UiEvent::NavBack => {
                self.hide();
                Some(EventResponse::Consumed)
            }
            UiEvent::MouseMove { .. } if self.controller_focused.is_some() => {
                self.controller_focused = None;
                self.apply_controller_focus();
                None
            }
            _ => None,
        }
    }

    /// Handles a UI event. Returns `Consumed` if the sub-panel ate it.
    ///
    /// # Arguments
    ///
    /// * `event` - The event to route.
    ///
    /// # Returns
    ///
    /// * `EventResponse::Consumed` when handled, `Ignored` otherwise.
    pub(super) fn handle_event(&mut self, event: &UiEvent) -> EventResponse {
        if !self.visible {
            return EventResponse::Ignored;
        }

        let (tb_resp, _drag) = self.title_bar.handle_event(event);
        if self.title_bar.was_close_requested() {
            self.hide();
            return EventResponse::Consumed;
        }
        if tb_resp == EventResponse::Consumed {
            return EventResponse::Consumed;
        }

        if let Some(resp) = self.handle_nav_event(event) {
            return resp;
        }

        // Expanded dropdowns overlay everything below them, so they must see
        // the event before the close button and the other rows.
        if self.drp_ratling_rank.is_expanded() {
            let resp = self.drp_ratling_rank.handle_event(event);
            self.collect_child_actions();
            if resp == EventResponse::Consumed {
                return EventResponse::Consumed;
            }
        }
        if self.drp_greenling_rank.is_expanded() {
            let resp = self.drp_greenling_rank.handle_event(event);
            self.collect_child_actions();
            if resp == EventResponse::Consumed {
                return EventResponse::Consumed;
            }
        }

        if self.btn_close.handle_event(event) == EventResponse::Consumed {
            self.hide();
            return EventResponse::Consumed;
        }

        let mut consumed = self.chk_enabled.handle_event(event) == EventResponse::Consumed;
        for chk in &mut self.chk_categories {
            consumed |= chk.handle_event(event) == EventResponse::Consumed;
        }
        if !self.drp_ratling_rank.is_expanded() {
            consumed |= self.drp_ratling_rank.handle_event(event) == EventResponse::Consumed;
        }
        if !self.drp_greenling_rank.is_expanded() {
            consumed |= self.drp_greenling_rank.handle_event(event) == EventResponse::Consumed;
        }

        self.collect_child_actions();

        if consumed {
            return EventResponse::Consumed;
        }

        consume_mouse_events_in_bounds(&self.bounds, event)
    }

    /// Renders the sub-panel and its children.
    ///
    /// # Arguments
    ///
    /// * `ctx` - Render context.
    ///
    /// # Returns
    ///
    /// * `Ok(())` on success, or an SDL error string.
    pub(super) fn render(&mut self, ctx: &mut RenderContext<'_, '_>) -> Result<(), String> {
        if !self.visible {
            return Ok(());
        }

        draw_sub_panel_frame(ctx, &self.bounds, SUB_PANEL_BG, BORDER_COLOR)?;
        self.title_bar.render(ctx)?;
        self.chk_enabled.render(ctx)?;

        let sep_color = Color::RGBA(120, 120, 140, 150);
        let left = self.bounds.x + H_INSET;
        let right = self.bounds.x + self.bounds.width as i32 - H_INSET;
        let sep_y = self.bounds.y + AL_Y_SEP;
        ctx.canvas.set_draw_color(sep_color);
        ctx.canvas.draw_line(
            sdl2::rect::Point::new(left, sep_y),
            sdl2::rect::Point::new(right, sep_y),
        )?;

        self.lbl_header_category.render(ctx)?;
        self.lbl_header_rank.render(ctx)?;
        let header_line_y = self.bounds.y + AL_Y_FIRST_ROW - 2;
        ctx.canvas.set_draw_color(sep_color);
        ctx.canvas.draw_line(
            sdl2::rect::Point::new(left, header_line_y),
            sdl2::rect::Point::new(right, header_line_y),
        )?;

        for chk in &mut self.chk_categories {
            chk.render(ctx)?;
        }
        self.btn_close.render(ctx)?;

        // Dropdowns last so an expanded list overlays the rows above it;
        // the expanded one goes very last.
        if self.drp_greenling_rank.is_expanded() {
            self.drp_ratling_rank.render(ctx)?;
            self.drp_greenling_rank.render(ctx)?;
        } else {
            self.drp_greenling_rank.render(ctx)?;
            self.drp_ratling_rank.render(ctx)?;
        }

        Ok(())
    }

    /// Drains pending actions.
    ///
    /// # Returns
    ///
    /// * All actions queued since the previous call.
    pub(super) fn take_actions(&mut self) -> Vec<WidgetAction> {
        std::mem::take(&mut self.pending_actions)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::widget::{KeyModifiers, MouseButton};

    fn make_panel() -> AutolootSettingsSubPanel {
        let mut panel = AutolootSettingsSubPanel::new(0, 0, 300);
        panel.sync_state(true, &AutolootConfig::default());
        panel.show();
        panel
    }

    fn left_click(x: i32, y: i32) -> UiEvent {
        UiEvent::MouseClick {
            x,
            y,
            button: MouseButton::Left,
            modifiers: KeyModifiers::default(),
        }
    }

    fn row_center(row: usize) -> (i32, i32) {
        (
            H_INSET + 5,
            AL_Y_FIRST_ROW + AL_ROW_H * row as i32 + 2 + ROW_H / 2,
        )
    }

    #[test]
    fn panel_height_fits_all_rows_and_close_button() {
        let panel = make_panel();
        let last_row_bottom = AL_Y_FIRST_ROW + AL_ROW_H * AutolootCategories::ALL.len() as i32;
        assert!(panel.btn_close.bounds().y >= last_row_bottom);
        assert_eq!(panel.chk_categories.len(), AutolootCategories::ALL.len());
    }

    #[test]
    fn sync_state_mirrors_config_into_widgets() {
        let mut panel = make_panel();
        let cfg = AutolootConfig {
            categories: AutolootCategories::JEWELRY | AutolootCategories::RATLING_EYES,
            ratling_min_rank: EyeRank::Duke,
            greenling_min_rank: EyeRank::Prince,
        };
        panel.sync_state(false, &cfg);
        assert!(!panel.chk_enabled.is_checked());
        assert!(panel.chk_categories[6].is_checked());
        assert!(panel.chk_categories[RATLING_ROW].is_checked());
        assert!(!panel.chk_categories[0].is_checked());
        assert_eq!(
            panel.drp_ratling_rank.selected_index(),
            EyeRank::Duke as usize
        );
        assert_eq!(
            panel.drp_greenling_rank.selected_index(),
            EyeRank::Prince as usize
        );
        assert_eq!(panel.config(), cfg);
    }

    #[test]
    fn clicking_category_row_emits_config_action() {
        let mut panel = make_panel();
        let (x, y) = row_center(RATLING_ROW);
        assert_eq!(
            panel.handle_event(&left_click(x, y)),
            EventResponse::Consumed
        );
        let actions = panel.take_actions();
        assert_eq!(actions.len(), 1);
        match &actions[0] {
            WidgetAction::SetAutolootConfig(cfg) => {
                assert!(cfg.has(AutolootCategories::RATLING_EYES));
                assert!(cfg.has(AutolootCategories::GOLD), "other bits preserved");
            }
            other => panic!("unexpected action {other:?}"),
        }
    }

    #[test]
    fn master_toggle_emits_enabled_action_only() {
        let mut panel = make_panel();
        let y = AL_Y_ENABLE + ROW_H / 2;
        assert_eq!(
            panel.handle_event(&left_click(H_INSET + 5, y)),
            EventResponse::Consumed
        );
        let actions = panel.take_actions();
        assert!(matches!(
            actions.as_slice(),
            [WidgetAction::SetAutolootEnabled(false)]
        ));
    }

    #[test]
    fn controller_confirm_cycles_rank_dropdown() {
        let mut panel = make_panel();
        panel.controller_focused = Some(AutolootSettingsSubPanel::FOCUS_GREENLING_RANK);
        panel.handle_event(&UiEvent::NavConfirm);
        let actions = panel.take_actions();
        match actions.as_slice() {
            [WidgetAction::SetAutolootConfig(cfg)] => {
                assert_eq!(cfg.greenling_min_rank, EyeRank::Fighter);
                assert_eq!(cfg.ratling_min_rank, EyeRank::Base);
            }
            other => panic!("unexpected actions {other:?}"),
        }
    }

    #[test]
    fn close_button_hides_panel() {
        let mut panel = make_panel();
        let b = *panel.btn_close.bounds();
        panel.handle_event(&left_click(
            b.x + b.width as i32 / 2,
            b.y + b.height as i32 / 2,
        ));
        assert!(!panel.visible);
    }
}
