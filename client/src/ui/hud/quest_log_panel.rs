//! Quest log overlay listing the player's outstanding NPC quests.
//!
//! GameScene composes a [`QuestLogPanelData`] each frame from
//! [`crate::player_state::PlayerState`] (combined with the static
//! [`mag_core::quest_defs::QUEST_DEFS`] catalogue) and feeds it to the
//! panel via [`QuestLogPanel::update_data`]. Clicking a row produces a
//! [`WidgetAction::SetActiveQuest`] which the scene forwards to the
//! server.

use sdl2::pixels::Color;
use sdl2::render::BlendMode;

use crate::font_cache;
use crate::ui::RenderContext;
use crate::ui::widget::{
    Bounds, EventResponse, HudPanel, MouseButton, UiEvent, Widget, WidgetAction,
};
use crate::ui::widgets::title_bar::{TITLE_BAR_H, TitleBar, clamp_to_viewport};

/// Font index used for panel text (yellow bitmap font, matches other HUD
/// panels).
const PANEL_FONT: usize = 1;

/// Vertical pixel height of a single quest row.
const ROW_H: i32 = 14;

/// Inner horizontal padding from the panel border to row content.
const H_INSET: i32 = 6;

/// Maximum number of quest rows visible at once before scrolling kicks in.
pub const VISIBLE_QUEST_ROWS: usize = 8;

/// Highlight color for the currently focused quest row.
const ACTIVE_HIGHLIGHT: Color = Color::RGBA(80, 80, 30, 200);

/// Color used to highlight the item name inside a fallback
/// `Bring <item> to <npc>` quest title.
const ITEM_NAME_COLOR: Color = Color::RGBA(255, 220, 0, 255);

/// Title text for a single quest row. `Plain` is used when a static quest
/// definition supplies a hand-authored title; `BringItemToNpc` is the
/// fallback for NPCs without an authored entry — it is rendered as
/// "Bring <item> to <npc>" with the item name highlighted in
/// [`ITEM_NAME_COLOR`].
#[derive(Clone, Debug)]
pub enum QuestTitle {
    /// Pre-formatted plain title (e.g. from a static quest definition).
    Plain(String),
    /// Render-time formatted title that colors the item name distinctly.
    BringItemToNpc {
        /// Display name of the wanted item.
        item_name: String,
        /// Display name of the NPC quest giver.
        npc_name: String,
    },
}

impl QuestTitle {
    /// Returns the matching text used for click-target detection and the
    /// non-colored highlight check (currently just the active row tint).
    /// Useful only for tests / debugging; the renderer does not call it.
    ///
    /// # Returns
    ///
    /// * Value returned by `as_display_string`.
    pub fn as_display_string(&self) -> String {
        match self {
            QuestTitle::Plain(s) => s.clone(),
            QuestTitle::BringItemToNpc {
                item_name,
                npc_name,
            } => {
                let item = if item_name.is_empty() { "?" } else { item_name };
                let npc = if npc_name.is_empty() { "NPC" } else { npc_name };
                format!("Bring {item} to {npc}")
            }
        }
    }
}

/// One quest entry as displayed in the panel.
///
/// Built by GameScene from a `QuestLogEntry` reported by the server,
/// optionally enriched with a static definition from
/// [`mag_core::quest_defs::QUEST_DEFS`].
#[derive(Clone, Debug)]
pub struct QuestEntryDisplay {
    /// NPC template ID — also the wire identifier used by `SetActiveQuest`.
    pub template_id: u16,
    /// Title shown on the quest row.
    pub title: QuestTitle,
    /// Multi-line description shown for the active quest.
    pub description: String,
    /// Walkthrough steps shown for the active quest.
    pub steps: Vec<String>,
    /// World tile X of the NPC quest giver.
    pub npc_x: u16,
    /// World tile Y of the NPC quest giver.
    pub npc_y: u16,
}

/// Per-frame panel data produced by GameScene.
#[derive(Clone, Default, Debug)]
pub struct QuestLogPanelData {
    /// Quest entries to display, in server order.
    pub entries: Vec<QuestEntryDisplay>,
    /// NPC template ID of the currently focused quest (`0` = none).
    pub active_template_id: u16,
}

/// The quest log HUD panel.
pub struct QuestLogPanel {
    bounds: Bounds,
    bg_color: Color,
    border_color: Color,
    visible: bool,
    data: QuestLogPanelData,
    pending_actions: Vec<WidgetAction>,
    scroll: usize,
    title_bar: TitleBar,
}

impl QuestLogPanel {
    /// Creates a new (hidden) quest log panel.
    ///
    /// # Arguments
    ///
    /// * `bounds`   - Screen-space bounds of the panel.
    /// * `bg_color` - Semi-transparent background color.
    ///
    /// # Returns
    ///
    /// * A new `QuestLogPanel`, initially hidden, with no data.
    pub fn new(bounds: Bounds, bg_color: Color) -> Self {
        let title_bar = TitleBar::new("Quest Log", bounds.x, bounds.y, bounds.width);
        Self {
            bounds,
            bg_color,
            border_color: Color::RGBA(120, 120, 140, 200),
            visible: false,
            data: QuestLogPanelData::default(),
            pending_actions: Vec::new(),
            scroll: 0,
            title_bar,
        }
    }

    /// Toggles the panel's visibility.
    pub fn toggle(&mut self) {
        self.visible = !self.visible;
    }

    /// Returns `true` when the panel is currently visible.
    ///
    /// # Returns
    ///
    /// * `true` when `is_visible` succeeds or the condition is met, otherwise `false`.
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Replaces the panel's per-frame data snapshot.
    ///
    /// # Arguments
    ///
    /// * `data` - New snapshot to display.
    pub fn update_data(&mut self, data: QuestLogPanelData) {
        // Clamp the scroll if the entry list shrank.
        let max_scroll = data.entries.len().saturating_sub(VISIBLE_QUEST_ROWS);
        if self.scroll > max_scroll {
            self.scroll = max_scroll;
        }
        self.data = data;
    }

    /// Y coordinate (top edge) of the row at visible-index `row_idx`.
    fn row_y(&self, row_idx: usize) -> i32 {
        self.bounds.y + TITLE_BAR_H + 4 + (row_idx as i32) * ROW_H
    }

    /// Y coordinate of the first description line below the rows.
    fn desc_y(&self) -> i32 {
        self.row_y(VISIBLE_QUEST_ROWS) + 6
    }

    /// Render a [`QuestTitle`] inline at `(x, y)`, optionally coloring the
    /// item name distinctly for fallback titles.
    ///
    /// # Arguments
    ///
    /// * `ctx`   - Active render context.
    /// * `title` - Title to render.
    /// * `x`     - Left edge of the first glyph.
    /// * `y`     - Baseline-aligned top edge.
    ///
    /// # Returns
    ///
    /// * `Ok(())` on success; SDL error string on failure.
    fn render_title(
        ctx: &mut RenderContext<'_, '_>,
        title: &QuestTitle,
        x: i32,
        y: i32,
    ) -> Result<(), String> {
        match title {
            QuestTitle::Plain(s) => {
                font_cache::draw_text(
                    ctx.canvas,
                    ctx.gfx,
                    PANEL_FONT,
                    s,
                    x,
                    y,
                    font_cache::TextStyle::PLAIN,
                )?;
            }
            QuestTitle::BringItemToNpc {
                item_name,
                npc_name,
            } => {
                let item = if item_name.is_empty() {
                    "?".to_owned()
                } else {
                    item_name.clone()
                };
                let npc = if npc_name.is_empty() {
                    "NPC".to_owned()
                } else {
                    npc_name.clone()
                };
                let prefix = "Bring ";
                let middle = " to ";
                let mut cursor = x;
                font_cache::draw_text(
                    ctx.canvas,
                    ctx.gfx,
                    PANEL_FONT,
                    prefix,
                    cursor,
                    y,
                    font_cache::TextStyle::PLAIN,
                )?;
                cursor += font_cache::text_width(prefix) as i32;
                font_cache::draw_text(
                    ctx.canvas,
                    ctx.gfx,
                    PANEL_FONT,
                    &item,
                    cursor,
                    y,
                    font_cache::TextStyle::tinted(ITEM_NAME_COLOR),
                )?;
                cursor += font_cache::text_width(&item) as i32;
                font_cache::draw_text(
                    ctx.canvas,
                    ctx.gfx,
                    PANEL_FONT,
                    middle,
                    cursor,
                    y,
                    font_cache::TextStyle::PLAIN,
                )?;
                cursor += font_cache::text_width(middle) as i32;
                font_cache::draw_text(
                    ctx.canvas,
                    ctx.gfx,
                    PANEL_FONT,
                    &npc,
                    cursor,
                    y,
                    font_cache::TextStyle::PLAIN,
                )?;
            }
        }
        Ok(())
    }
}

impl Widget for QuestLogPanel {
    fn bounds(&self) -> &Bounds {
        &self.bounds
    }

    fn set_position(&mut self, x: i32, y: i32) {
        self.bounds.x = x;
        self.bounds.y = y;
        self.title_bar.set_bar_position(x, y);
    }

    fn handle_event(&mut self, event: &UiEvent) -> EventResponse {
        if !self.visible {
            return EventResponse::Ignored;
        }

        let (tb_resp, drag_pos) = self.title_bar.handle_event(event);
        if let Some((new_x, new_y)) = drag_pos {
            let (cx, cy) = clamp_to_viewport(new_x, new_y, self.bounds.width, self.bounds.height);
            self.set_position(cx, cy);
        }
        if self.title_bar.was_close_requested() {
            self.visible = false;
            self.pending_actions
                .push(WidgetAction::TogglePanel(HudPanel::QuestLog));
            return EventResponse::Consumed;
        }
        if tb_resp == EventResponse::Consumed {
            return EventResponse::Consumed;
        }

        match event {
            UiEvent::MouseClick { x, y, button, .. } => {
                if !self.bounds.contains_point(*x, *y) {
                    return EventResponse::Ignored;
                }
                if *button != MouseButton::Left {
                    return EventResponse::Consumed;
                }
                for visible_idx in 0..VISIBLE_QUEST_ROWS {
                    let entry_idx = self.scroll + visible_idx;
                    let Some(entry) = self.data.entries.get(entry_idx) else {
                        break;
                    };
                    let row_top = self.row_y(visible_idx);
                    if *y >= row_top && *y < row_top + ROW_H {
                        self.pending_actions.push(WidgetAction::SetActiveQuest {
                            npc_template_id: entry.template_id,
                        });
                        break;
                    }
                }
                EventResponse::Consumed
            }
            UiEvent::MouseWheel { x, y, delta } => {
                if !self.bounds.contains_point(*x, *y) {
                    return EventResponse::Ignored;
                }
                let max_scroll = self.data.entries.len().saturating_sub(VISIBLE_QUEST_ROWS);
                if *delta > 0 {
                    self.scroll = self.scroll.saturating_sub(*delta as usize);
                } else if *delta < 0 {
                    self.scroll = (self.scroll + (-delta) as usize).min(max_scroll);
                }
                EventResponse::Consumed
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

        ctx.canvas.set_blend_mode(BlendMode::Blend);
        ctx.canvas.set_draw_color(self.bg_color);
        ctx.canvas.fill_rect(rect)?;

        ctx.canvas.set_draw_color(self.border_color);
        ctx.canvas.draw_rect(rect)?;

        self.title_bar.render(ctx)?;

        let text_x = self.bounds.x + H_INSET;

        if self.data.entries.is_empty() {
            font_cache::draw_text(
                ctx.canvas,
                ctx.gfx,
                PANEL_FONT,
                "No active quests",
                text_x,
                self.row_y(0),
                font_cache::TextStyle::PLAIN,
            )?;
            return Ok(());
        }

        // Quest rows.
        for visible_idx in 0..VISIBLE_QUEST_ROWS {
            let entry_idx = self.scroll + visible_idx;
            let Some(entry) = self.data.entries.get(entry_idx) else {
                break;
            };
            let row_top = self.row_y(visible_idx);

            if entry.template_id == self.data.active_template_id
                && self.data.active_template_id != 0
            {
                let hl = sdl2::rect::Rect::new(
                    self.bounds.x + 2,
                    row_top,
                    self.bounds.width.saturating_sub(4),
                    ROW_H as u32,
                );
                ctx.canvas.set_draw_color(ACTIVE_HIGHLIGHT);
                ctx.canvas.fill_rect(hl)?;
            }

            Self::render_title(ctx, &entry.title, text_x, row_top + 2)?;
        }

        // Active quest description block under the rows.
        if self.data.active_template_id != 0
            && let Some(active) = self
                .data
                .entries
                .iter()
                .find(|e| e.template_id == self.data.active_template_id)
        {
            let mut y = self.desc_y();
            if !active.description.is_empty() {
                font_cache::draw_text(
                    ctx.canvas,
                    ctx.gfx,
                    PANEL_FONT,
                    &active.description,
                    text_x,
                    y,
                    font_cache::TextStyle::PLAIN,
                )?;
                y += ROW_H;
            }
            for step in &active.steps {
                font_cache::draw_text(
                    ctx.canvas,
                    ctx.gfx,
                    PANEL_FONT,
                    step,
                    text_x,
                    y,
                    font_cache::TextStyle::PLAIN,
                )?;
                y += ROW_H;
            }
        }

        Ok(())
    }

    fn take_actions(&mut self) -> Vec<WidgetAction> {
        std::mem::take(&mut self.pending_actions)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_data() -> QuestLogPanelData {
        QuestLogPanelData {
            entries: vec![
                QuestEntryDisplay {
                    template_id: 11,
                    title: QuestTitle::Plain("Quest A".to_owned()),
                    description: String::new(),
                    steps: Vec::new(),
                    npc_x: 1,
                    npc_y: 2,
                },
                QuestEntryDisplay {
                    template_id: 22,
                    title: QuestTitle::Plain("Quest B".to_owned()),
                    description: String::new(),
                    steps: Vec::new(),
                    npc_x: 3,
                    npc_y: 4,
                },
            ],
            active_template_id: 22,
        }
    }

    #[test]
    fn toggle_flips_visibility() {
        let panel = QuestLogPanel::new(Bounds::new(0, 0, 200, 200), Color::RGBA(0, 0, 0, 200));
        let mut p = panel;
        assert!(!p.is_visible());
        p.toggle();
        assert!(p.is_visible());
        p.toggle();
        assert!(!p.is_visible());
    }

    #[test]
    fn update_data_clamps_scroll() {
        let mut p = QuestLogPanel::new(Bounds::new(0, 0, 200, 200), Color::RGBA(0, 0, 0, 200));
        p.scroll = 99;
        p.update_data(sample_data());
        assert_eq!(p.scroll, 0, "scroll should be clamped when list is small");
    }

    #[test]
    fn click_on_row_emits_set_active_quest() {
        let mut p = QuestLogPanel::new(Bounds::new(0, 0, 200, 200), Color::RGBA(0, 0, 0, 200));
        p.toggle();
        p.update_data(sample_data());

        let row_top = p.row_y(0);
        let event = UiEvent::MouseClick {
            x: 10,
            y: row_top + 1,
            button: MouseButton::Left,
            modifiers: crate::ui::widget::KeyModifiers::default(),
        };
        let resp = p.handle_event(&event);
        assert_eq!(resp, EventResponse::Consumed);
        let actions = p.take_actions();
        assert_eq!(actions.len(), 1);
        match &actions[0] {
            WidgetAction::SetActiveQuest { npc_template_id } => {
                assert_eq!(*npc_template_id, 11);
            }
            _ => panic!("expected SetActiveQuest action"),
        }
    }
}
