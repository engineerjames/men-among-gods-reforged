//! Skills / character / attributes panel.
//!
//! Displays attributes, HP/End/Mana pools, and learned skills with +/-
//! raising controls. Left-clicking a skill row casts it; right-clicking
//! begins a spell-bar assignment. The "Update" button commits pending
//! raises to the server.

use std::cmp::Ordering;

use sdl2::pixels::Color;
use sdl2::render::BlendMode;

use mag_core::skills::{MAX_SKILLS, get_skill_name, get_skill_nr, get_skill_sortkey};

use super::RenderContext;
use super::title_bar::{TitleBar, clamp_to_viewport};
use super::widget::{Bounds, EventResponse, HudPanel, MouseButton, UiEvent, Widget, WidgetAction};
use crate::font_cache;

/// Font index used for panel text (yellow bitmap font).
const PANEL_FONT: usize = 1;

/// Row height in pixels for attribute/skill rows.
const ROW_H: i32 = 14;

/// Number of visible skill rows.
const VISIBLE_SKILL_ROWS: usize = 6;

/// Maximum skill scroll offset.
const SKILL_SCROLL_MAX: usize = 90;

/// Attribute names matching the 5-element attrib array.
const ATTR_NAMES: [&str; 5] = ["Bravery", "Willpower", "Intuition", "Agility", "Strength"];

/// Per-frame data snapshot fed by GameScene.
///
/// The panel is decoupled from `PlayerState` — GameScene builds this each
/// frame and passes it via [`SkillsPanel::update_data`].
#[derive(Clone)]
pub struct SkillsPanelData {
    /// The 5 attributes, each with 6 sub-values.
    pub attrib: [[u8; 6]; 5],
    /// HP pool sub-values.
    pub hp: [u16; 6],
    /// Endurance pool sub-values.
    pub end: [u16; 6],
    /// Mana pool sub-values.
    pub mana: [u16; 6],
    /// All 100 skills, each with 6 sub-values.
    pub skill: [[u8; 6]; 100],
    /// Available experience points for raising.
    pub points: i32,
    /// Pre-sorted skill indices (learned first, then by sort key).
    pub sorted_skills: Vec<usize>,
}

/// The skills / character / attributes HUD panel.
///
/// Toggled via the HUD button bar. Provides full legacy parity with the
/// original left-panel stat/skill display, including +/- raising, the
/// Update button, skill casting, and spell-bar assignment.
pub struct SkillsPanel {
    bounds: Bounds,
    bg_color: Color,
    border_color: Color,
    visible: bool,
    /// Per-frame data snapshot from GameScene.
    data: Option<SkillsPanelData>,
    /// Pending stat raises (indices 0-4 = attribs, 5=HP, 6=End, 7=Mana,
    /// 8-107 = sorted skill positions).
    stat_raised: [i32; 108],
    /// Points already spent on pending raises.
    stat_points_used: i32,
    /// Scroll offset for the skill list.
    skill_scroll: usize,
    /// Actions to be drained by the owning scene.
    pending_actions: Vec<WidgetAction>,
    /// Draggable title bar with pin and close buttons.
    title_bar: TitleBar,
}

impl SkillsPanel {
    /// Creates a new skills panel.
    ///
    /// # Arguments
    ///
    /// * `bounds` - Position and size.
    /// * `bg_color` - Semi-transparent background color.
    ///
    /// # Returns
    ///
    /// A new `SkillsPanel`, initially hidden.
    pub fn new(bounds: Bounds, bg_color: Color) -> Self {
        let title_bar = TitleBar::new("Skills & Attributes", bounds.x, bounds.y, bounds.width);
        Self {
            bounds,
            bg_color,
            border_color: Color::RGBA(120, 120, 140, 200),
            visible: false,
            data: None,
            stat_raised: [0; 108],
            stat_points_used: 0,
            skill_scroll: 0,
            pending_actions: Vec::new(),
            title_bar,
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

    /// Feeds a new per-frame data snapshot into the panel.
    ///
    /// # Arguments
    ///
    /// * `data` - Snapshot of attribute/skill data from `PlayerState`.
    pub fn update_data(&mut self, data: SkillsPanelData) {
        self.data = Some(data);
    }

    /// Resets all pending stat raises (e.g. on leaving the game scene).
    pub fn reset_raises(&mut self) {
        self.stat_raised = [0; 108];
        self.stat_points_used = 0;
    }

    // ---- Layout helpers ---------------------------------------------------

    /// Inner content area (inset from panel border).
    fn content_bounds(&self) -> Bounds {
        Bounds::new(
            self.bounds.x + 6,
            self.bounds.y + 6,
            self.bounds.width.saturating_sub(12),
            self.bounds.height.saturating_sub(12),
        )
    }

    /// Y offset within the panel for attribute row `n` (0-4).
    fn attr_row_y(&self, n: usize) -> i32 {
        let cb = self.content_bounds();
        cb.y + 18 + (n as i32) * ROW_H
    }

    /// Y offset for pool row (0=HP, 1=End, 2=Mana).
    fn pool_row_y(&self, n: usize) -> i32 {
        let cb = self.content_bounds();
        cb.y + 18 + 5 * ROW_H + 4 + (n as i32) * ROW_H
    }

    /// Y offset for skill row `n` (0-based visible row).
    fn skill_row_y(&self, n: usize) -> i32 {
        let cb = self.content_bounds();
        cb.y + 18 + 5 * ROW_H + 4 + 3 * ROW_H + 6 + (n as i32) * ROW_H
    }

    /// Y offset for the Update button row.
    fn update_row_y(&self) -> i32 {
        let cb = self.content_bounds();
        cb.y + (cb.height as i32) - 16
    }

    /// X positions for bind button, name, value, +, -, cost columns.
    fn col_x(&self) -> (i32, i32, i32, i32, i32) {
        let cb = self.content_bounds();
        let name_x = cb.x + 18;
        let value_x = cb.x + 146;
        let plus_x = cb.x + 165;
        let minus_x = cb.x + 180;
        let cost_x = cb.x + 195;
        (name_x, value_x, plus_x, minus_x, cost_x)
    }

    // ---- Cost calculation (mirrors legacy formulas) ----------------------

    // TODO: Deduplicate these formulas into one place.
    /// Cost to raise attribute `n` by one from base value `v`.
    fn attrib_cost(data: &SkillsPanelData, n: usize, v: i32) -> i32 {
        let max_v = data.attrib[n][2] as i32;
        if v >= max_v {
            return i32::MAX;
        }
        let diff = data.attrib[n][3] as i32;
        let v64 = v as i64;
        ((v64 * v64 * v64) * (diff as i64) / 20).clamp(0, i32::MAX as i64) as i32
    }

    /// Cost to raise skill `n` by one from base value `v`.
    fn skill_cost(data: &SkillsPanelData, n: usize, v: i32) -> i32 {
        let max_v = data.skill[n][2] as i32;
        if v >= max_v {
            return i32::MAX;
        }
        let diff = data.skill[n][3] as i32;
        let v64 = v as i64;
        let cubic = ((v64 * v64 * v64) * (diff as i64) / 40).clamp(0, i32::MAX as i64) as i32;
        v.max(cubic)
    }

    /// Cost to raise HP by one from base value `v`.
    fn hp_cost(data: &SkillsPanelData, v: i32) -> i32 {
        if v >= data.hp[2] as i32 {
            return i32::MAX;
        }
        (v as i64 * data.hp[3] as i64).clamp(0, i32::MAX as i64) as i32
    }

    /// Cost to raise Endurance by one from base value `v`.
    fn end_cost(data: &SkillsPanelData, v: i32) -> i32 {
        if v >= data.end[2] as i32 {
            return i32::MAX;
        }
        (v as i64 * data.end[3] as i64 / 2).clamp(0, i32::MAX as i64) as i32
    }

    /// Cost to raise Mana by one from base value `v`.
    fn mana_cost(data: &SkillsPanelData, v: i32) -> i32 {
        if v >= data.mana[2] as i32 {
            return i32::MAX;
        }
        (v as i64 * data.mana[3] as i64).clamp(0, i32::MAX as i64) as i32
    }

    // ---- Click handling ---------------------------------------------------

    /// Handle a stat +/- click at panel-local coordinates.
    ///
    /// When `shift` is `true` (Shift held), raises by up to 10 levels,
    /// stopping early if points run out before all 10 are spent.
    ///
    /// # Arguments
    ///
    /// * `x` - Panel-local X coordinate.
    /// * `y` - Panel-local Y coordinate.
    /// * `data` - Current character data snapshot.
    /// * `shift` - `true` when the Shift modifier was held.
    fn handle_stat_click(&mut self, x: i32, y: i32, data: &SkillsPanelData, shift: bool) {
        let (_, _, plus_x, minus_x, _) = self.col_x();

        let is_plus = (plus_x..plus_x + 12).contains(&x);
        let is_minus = (minus_x..minus_x + 12).contains(&x);
        if !is_plus && !is_minus {
            return;
        }

        let repeats = if shift { 10 } else { 1 };

        // Attributes (0-4).
        for n in 0..5 {
            let ry = self.attr_row_y(n);
            if y >= ry && y < ry + ROW_H {
                if is_plus {
                    for _ in 0..repeats {
                        self.raise_attrib(data, n);
                    }
                } else {
                    for _ in 0..repeats {
                        self.lower_attrib(data, n);
                    }
                }
                return;
            }
        }

        // Pools (HP=5, End=6, Mana=7).
        for p in 0..3 {
            let ry = self.pool_row_y(p);
            if y >= ry && y < ry + ROW_H {
                let idx = 5 + p;
                if is_plus {
                    for _ in 0..repeats {
                        self.raise_pool(data, idx, p);
                    }
                } else {
                    for _ in 0..repeats {
                        self.lower_pool(data, idx, p);
                    }
                }
                return;
            }
        }

        // Skills.
        for row in 0..VISIBLE_SKILL_ROWS {
            let ry = self.skill_row_y(row);
            if y >= ry && y < ry + ROW_H {
                let sorted_idx = self.skill_scroll + row;
                let raised_idx = 8 + sorted_idx;
                if raised_idx >= 108 {
                    return;
                }
                if let Some(&skill_id) = data.sorted_skills.get(sorted_idx) {
                    if data.skill[skill_id][0] == 0 || get_skill_name(skill_id).is_empty() {
                        return;
                    }
                    if is_plus {
                        for _ in 0..repeats {
                            self.raise_skill(data, skill_id, raised_idx);
                        }
                    } else {
                        for _ in 0..repeats {
                            self.lower_skill(data, skill_id, raised_idx);
                        }
                    }
                }
                return;
            }
        }
    }

    /// Spend points to raise attribute `n` by one.
    fn raise_attrib(&mut self, data: &SkillsPanelData, n: usize) {
        let avail = data.points - self.stat_points_used;
        let cur = data.attrib[n][0] as i32 + self.stat_raised[n];
        let need = Self::attrib_cost(data, n, cur);
        if need != i32::MAX && need <= avail {
            self.stat_points_used += need;
            self.stat_raised[n] += 1;
        }
    }

    /// Refund one pending attribute raise for attribute `n`.
    fn lower_attrib(&mut self, data: &SkillsPanelData, n: usize) {
        if self.stat_raised[n] > 0 {
            self.stat_raised[n] -= 1;
            let cur = data.attrib[n][0] as i32 + self.stat_raised[n];
            let refund = Self::attrib_cost(data, n, cur);
            if refund != i32::MAX {
                self.stat_points_used -= refund;
            }
        }
    }

    /// Spend points to raise pool `pool` (0=HP, 1=End, 2=Mana) by one.
    fn raise_pool(&mut self, data: &SkillsPanelData, stat_idx: usize, pool: usize) {
        let avail = data.points - self.stat_points_used;
        let (base, cost_fn): (i32, fn(&SkillsPanelData, i32) -> i32) = match pool {
            0 => (data.hp[0] as i32, Self::hp_cost),
            1 => (data.end[0] as i32, Self::end_cost),
            _ => (data.mana[0] as i32, Self::mana_cost),
        };
        let cur = base + self.stat_raised[stat_idx];
        let need = cost_fn(data, cur);
        if need != i32::MAX && need <= avail {
            self.stat_points_used += need;
            self.stat_raised[stat_idx] += 1;
        }
    }

    /// Refund one pending pool raise for pool `pool`.
    fn lower_pool(&mut self, data: &SkillsPanelData, stat_idx: usize, pool: usize) {
        if self.stat_raised[stat_idx] > 0 {
            self.stat_raised[stat_idx] -= 1;
            let (base, cost_fn): (i32, fn(&SkillsPanelData, i32) -> i32) = match pool {
                0 => (data.hp[0] as i32, Self::hp_cost),
                1 => (data.end[0] as i32, Self::end_cost),
                _ => (data.mana[0] as i32, Self::mana_cost),
            };
            let cur = base + self.stat_raised[stat_idx];
            let refund = cost_fn(data, cur);
            if refund != i32::MAX {
                self.stat_points_used -= refund;
            }
        }
    }

    /// Spend points to raise skill `skill_id` by one.
    fn raise_skill(&mut self, data: &SkillsPanelData, skill_id: usize, raised_idx: usize) {
        let avail = data.points - self.stat_points_used;
        let cur = data.skill[skill_id][0] as i32 + self.stat_raised[raised_idx];
        let need = Self::skill_cost(data, skill_id, cur);
        if need != i32::MAX && need <= avail {
            self.stat_points_used += need;
            self.stat_raised[raised_idx] += 1;
        }
    }

    /// Refund one pending skill raise at `raised_idx`.
    fn lower_skill(&mut self, data: &SkillsPanelData, skill_id: usize, raised_idx: usize) {
        if self.stat_raised[raised_idx] > 0 {
            self.stat_raised[raised_idx] -= 1;
            let cur = data.skill[skill_id][0] as i32 + self.stat_raised[raised_idx];
            let refund = Self::skill_cost(data, skill_id, cur);
            if refund != i32::MAX {
                self.stat_points_used -= refund;
            }
        }
    }

    /// Handle the "Update" button click — commit all pending raises.
    fn handle_update_click(&mut self, data: &SkillsPanelData) {
        let mut raises: Vec<(i16, i32)> = Vec::new();

        for n in 0usize..108 {
            let v = self.stat_raised[n];
            if v == 0 {
                continue;
            }
            let which: i16 = if n >= 8 {
                let Some(&skill_id) = data.sorted_skills.get(n - 8) else {
                    continue;
                };
                (get_skill_nr(skill_id) + 8) as i16
            } else {
                n as i16
            };
            raises.push((which, v));
        }

        if !raises.is_empty() {
            self.pending_actions
                .push(WidgetAction::CommitStats { raises });
        }

        self.stat_raised = [0; 108];
        self.stat_points_used = 0;
    }

    /// Handle a left-click on a skill row (cast skill).
    fn handle_skill_cast_click(&mut self, y: i32, data: &SkillsPanelData) {
        for row in 0..VISIBLE_SKILL_ROWS {
            let ry = self.skill_row_y(row);
            if y >= ry && y < ry + ROW_H {
                let sorted_idx = self.skill_scroll + row;
                if let Some(&skill_id) = data.sorted_skills.get(sorted_idx) {
                    if !get_skill_name(skill_id).is_empty() && data.skill[skill_id][0] != 0 {
                        self.pending_actions.push(WidgetAction::CastSkill {
                            skill_nr: get_skill_nr(skill_id),
                        });
                    }
                }
                return;
            }
        }
    }

    /// Handle a right-click on a skill row (begin spell-bar assignment).
    fn handle_skill_assign_click(&mut self, y: i32, data: &SkillsPanelData) {
        for row in 0..VISIBLE_SKILL_ROWS {
            let ry = self.skill_row_y(row);
            if y >= ry && y < ry + ROW_H {
                let sorted_idx = self.skill_scroll + row;
                if let Some(&skill_id) = data.sorted_skills.get(sorted_idx) {
                    if !get_skill_name(skill_id).is_empty() && data.skill[skill_id][0] != 0 {
                        self.pending_actions
                            .push(WidgetAction::BeginSkillAssign { skill_id });
                    }
                }
                return;
            }
        }
    }

    /// Returns true if the x coordinate is in the skill-name column area.
    fn is_in_name_column(&self, x: i32) -> bool {
        let (name_x, _, plus_x, _, _) = self.col_x();
        x >= name_x && x < plus_x
    }

    /// Build a sorted skills list from raw skill data.
    ///
    /// Learned skills sort first, then by sort-key char, then by name.
    /// Unused/unnamed skills sort to the end.
    ///
    /// # Arguments
    ///
    /// * `skill` - The full 100-element skill array from `ClientPlayer`.
    ///
    /// # Returns
    ///
    /// A vector of `MAX_SKILLS` indices sorted for display.
    pub fn build_sorted_skills(skill: &[[u8; 6]; 100]) -> Vec<usize> {
        let mut out: Vec<usize> = (0..MAX_SKILLS).collect();
        out.sort_by(|&a, &b| {
            let a_unused = get_skill_sortkey(a) == 'Z' || get_skill_name(a).is_empty();
            let b_unused = get_skill_sortkey(b) == 'Z' || get_skill_name(b).is_empty();
            if a_unused != b_unused {
                return if a_unused {
                    Ordering::Greater
                } else {
                    Ordering::Less
                };
            }

            let a_learned = skill[a][0] != 0;
            let b_learned = skill[b][0] != 0;
            if a_learned != b_learned {
                return if a_learned {
                    Ordering::Less
                } else {
                    Ordering::Greater
                };
            }

            let a_key = get_skill_sortkey(a);
            let b_key = get_skill_sortkey(b);
            if a_key != b_key {
                return a_key.cmp(&b_key);
            }

            get_skill_name(a).cmp(get_skill_name(b))
        });
        out
    }
}

impl Widget for SkillsPanel {
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

        // --- Title bar gets first crack at all events ---
        let (tb_resp, drag_pos) = self.title_bar.handle_event(event);
        if let Some((new_x, new_y)) = drag_pos {
            let (cx, cy) = clamp_to_viewport(new_x, new_y, self.bounds.width, self.bounds.height);
            self.set_position(cx, cy);
        }
        if self.title_bar.was_close_requested() {
            self.visible = false;
            self.pending_actions
                .push(WidgetAction::TogglePanel(HudPanel::Skills));
            return EventResponse::Consumed;
        }
        if tb_resp == EventResponse::Consumed {
            return EventResponse::Consumed;
        }

        match event {
            UiEvent::MouseClick {
                x,
                y,
                button,
                modifiers,
                ..
            } => {
                if !self.bounds.contains_point(*x, *y) {
                    return EventResponse::Ignored;
                }

                let data = match self.data.as_ref() {
                    Some(d) => d.clone(),
                    None => return EventResponse::Consumed,
                };

                match button {
                    MouseButton::Left => {
                        // Check Update button.
                        let update_y = self.update_row_y();
                        let cb = self.content_bounds();
                        if *y >= update_y
                            && *y < update_y + ROW_H
                            && *x >= cb.x + 80
                            && *x < cb.x + 160
                        {
                            self.handle_update_click(&data);
                            return EventResponse::Consumed;
                        }

                        // Check +/- columns.
                        let (_, _, plus_x, minus_x, _) = self.col_x();
                        if *x >= plus_x && *x < minus_x + 12 {
                            self.handle_stat_click(*x, *y, &data, modifiers.shift);
                            return EventResponse::Consumed;
                        }

                        // Check skill row name area for casting.
                        if self.is_in_name_column(*x) {
                            let first_skill_y = self.skill_row_y(0);
                            let last_skill_y = self.skill_row_y(VISIBLE_SKILL_ROWS - 1) + ROW_H;
                            if *y >= first_skill_y && *y < last_skill_y {
                                self.handle_skill_cast_click(*y, &data);
                                return EventResponse::Consumed;
                            }
                        }
                    }
                    MouseButton::Right => {
                        // Right-click on skill row for spell-bar assignment.
                        let first_skill_y = self.skill_row_y(0);
                        let last_skill_y = self.skill_row_y(VISIBLE_SKILL_ROWS - 1) + ROW_H;
                        if *y >= first_skill_y && *y < last_skill_y {
                            self.handle_skill_assign_click(*y, &data);
                            return EventResponse::Consumed;
                        }
                    }
                    _ => {}
                }

                EventResponse::Consumed
            }
            UiEvent::MouseWheel { x, y, delta } => {
                if !self.bounds.contains_point(*x, *y) {
                    return EventResponse::Ignored;
                }
                if *delta > 0 {
                    self.skill_scroll = self.skill_scroll.saturating_sub(*delta as usize);
                } else if *delta < 0 {
                    self.skill_scroll =
                        (self.skill_scroll + (-delta) as usize).min(SKILL_SCROLL_MAX);
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

        // Semi-transparent background.
        ctx.canvas.set_blend_mode(BlendMode::Blend);
        ctx.canvas.set_draw_color(self.bg_color);
        ctx.canvas.fill_rect(rect)?;

        // Border.
        ctx.canvas.set_draw_color(self.border_color);
        ctx.canvas.draw_rect(rect)?;

        // Title bar (draggable, with pin/close).
        self.title_bar.render(ctx)?;

        let cb = self.content_bounds();

        let data = match self.data.as_ref() {
            Some(d) => d,
            None => return Ok(()),
        };

        let available_points = (data.points - self.stat_points_used).max(0);
        let (name_x, _value_x, plus_x, minus_x, cost_x) = self.col_x();

        // --- Attributes ---
        for n in 0..5 {
            let y = self.attr_row_y(n);
            let raised = self.stat_raised[n];
            let value_total = data.attrib[n][5] as i32 + raised;
            let value_bare = data.attrib[n][0] as i32 + raised;
            let cost = Self::attrib_cost(data, n, value_bare);

            let line = format!("{:<14} {:3}", ATTR_NAMES[n], value_total);
            font_cache::draw_text(
                ctx.canvas,
                ctx.gfx,
                PANEL_FONT,
                &line,
                name_x,
                y,
                font_cache::TextStyle::PLAIN,
            )?;

            let plus = if cost != i32::MAX && cost <= available_points {
                "+"
            } else {
                ""
            };
            let minus = if raised > 0 { "-" } else { "" };
            font_cache::draw_text(
                ctx.canvas,
                ctx.gfx,
                PANEL_FONT,
                plus,
                plus_x,
                y,
                font_cache::TextStyle::PLAIN,
            )?;
            font_cache::draw_text(
                ctx.canvas,
                ctx.gfx,
                PANEL_FONT,
                minus,
                minus_x,
                y,
                font_cache::TextStyle::PLAIN,
            )?;
            if cost != i32::MAX {
                let cost_text = format!("{:>7}", cost);
                font_cache::draw_text(
                    ctx.canvas,
                    ctx.gfx,
                    PANEL_FONT,
                    &cost_text,
                    cost_x,
                    y,
                    font_cache::TextStyle::PLAIN,
                )?;
            }
        }

        // --- Pools (HP, End, Mana) ---
        let pool_info: [(&str, i32, i32, usize); 3] = [
            (
                "Hitpoints",
                data.hp[5] as i32 + self.stat_raised[5],
                Self::hp_cost(data, data.hp[0] as i32 + self.stat_raised[5]),
                5,
            ),
            (
                "Endurance",
                data.end[5] as i32 + self.stat_raised[6],
                Self::end_cost(data, data.end[0] as i32 + self.stat_raised[6]),
                6,
            ),
            (
                "Mana",
                data.mana[5] as i32 + self.stat_raised[7],
                Self::mana_cost(data, data.mana[0] as i32 + self.stat_raised[7]),
                7,
            ),
        ];

        for (p, (name, value, cost, stat_idx)) in pool_info.iter().enumerate() {
            let y = self.pool_row_y(p);
            let raised = self.stat_raised[*stat_idx];

            let line = format!("{:<14} {:3}", name, value);
            font_cache::draw_text(
                ctx.canvas,
                ctx.gfx,
                PANEL_FONT,
                &line,
                name_x,
                y,
                font_cache::TextStyle::PLAIN,
            )?;

            let plus = if *cost != i32::MAX && *cost <= available_points {
                "+"
            } else {
                ""
            };
            let minus = if raised > 0 { "-" } else { "" };
            font_cache::draw_text(
                ctx.canvas,
                ctx.gfx,
                PANEL_FONT,
                plus,
                plus_x,
                y,
                font_cache::TextStyle::PLAIN,
            )?;
            font_cache::draw_text(
                ctx.canvas,
                ctx.gfx,
                PANEL_FONT,
                minus,
                minus_x,
                y,
                font_cache::TextStyle::PLAIN,
            )?;
            if *cost != i32::MAX {
                let cost_text = format!("{:>7}", cost);
                font_cache::draw_text(
                    ctx.canvas,
                    ctx.gfx,
                    PANEL_FONT,
                    &cost_text,
                    cost_x,
                    y,
                    font_cache::TextStyle::PLAIN,
                )?;
            }
        }

        // --- Separator line ---
        let sep_y = self.skill_row_y(0) - 3;
        ctx.canvas.set_draw_color(self.border_color);
        ctx.canvas.draw_line(
            sdl2::rect::Point::new(cb.x, sep_y),
            sdl2::rect::Point::new(cb.x + cb.width as i32 - 1, sep_y),
        )?;

        // --- Skills ---
        for row in 0..VISIBLE_SKILL_ROWS {
            let y = self.skill_row_y(row);
            let sorted_idx = self.skill_scroll + row;
            let raised_idx = 8 + sorted_idx;

            if let Some(&skill_id) = data.sorted_skills.get(sorted_idx) {
                let name = get_skill_name(skill_id);
                if name.is_empty() || data.skill[skill_id][0] == 0 {
                    continue;
                }

                if raised_idx >= self.stat_raised.len() {
                    continue;
                }

                let raised = self.stat_raised[raised_idx];
                let value_total = data.skill[skill_id][5] as i32 + raised;
                let value_bare = data.skill[skill_id][0] as i32 + raised;
                let cost = Self::skill_cost(data, skill_id, value_bare);

                let line = format!("{:<14} {:3}", name, value_total);
                font_cache::draw_text(
                    ctx.canvas,
                    ctx.gfx,
                    PANEL_FONT,
                    &line,
                    name_x,
                    y,
                    font_cache::TextStyle::PLAIN,
                )?;

                let plus = if cost != i32::MAX && cost <= available_points {
                    "+"
                } else {
                    ""
                };
                let minus = if raised > 0 { "-" } else { "" };
                font_cache::draw_text(
                    ctx.canvas,
                    ctx.gfx,
                    PANEL_FONT,
                    plus,
                    plus_x,
                    y,
                    font_cache::TextStyle::PLAIN,
                )?;
                font_cache::draw_text(
                    ctx.canvas,
                    ctx.gfx,
                    PANEL_FONT,
                    minus,
                    minus_x,
                    y,
                    font_cache::TextStyle::PLAIN,
                )?;
                if cost != i32::MAX {
                    let cost_text = format!("{:>7}", cost);
                    font_cache::draw_text(
                        ctx.canvas,
                        ctx.gfx,
                        PANEL_FONT,
                        &cost_text,
                        cost_x,
                        y,
                        font_cache::TextStyle::PLAIN,
                    )?;
                }
            }
        }

        // --- Scroll indicator ---
        if data.sorted_skills.len() > VISIBLE_SKILL_ROWS {
            let scroll_x = cb.x + cb.width as i32 - 12;
            let scroll_y_top = self.skill_row_y(0);
            let scroll_y_bot = self.skill_row_y(VISIBLE_SKILL_ROWS - 1) + ROW_H;
            let track_h = scroll_y_bot - scroll_y_top;

            // Draw track.
            ctx.canvas.set_draw_color(Color::RGBA(60, 60, 80, 120));
            ctx.canvas.fill_rect(sdl2::rect::Rect::new(
                scroll_x,
                scroll_y_top,
                8,
                track_h as u32,
            ))?;

            // Draw knob.
            let max_scroll = SKILL_SCROLL_MAX.max(1);
            let knob_h = 11i32;
            let knob_range = (track_h - knob_h).max(0);
            let knob_y = scroll_y_top + (self.skill_scroll as i32 * knob_range) / max_scroll as i32;
            ctx.canvas.set_draw_color(Color::RGB(8, 77, 23));
            ctx.canvas
                .fill_rect(sdl2::rect::Rect::new(scroll_x, knob_y, 8, knob_h as u32))?;
        }

        // --- Update button + remaining points ---
        let update_y = self.update_row_y();
        font_cache::draw_text(
            ctx.canvas,
            ctx.gfx,
            PANEL_FONT,
            "Update",
            cb.x + 80,
            update_y,
            font_cache::TextStyle::PLAIN,
        )?;
        let pts_text = format!("{:>7}", available_points);
        font_cache::draw_text(
            ctx.canvas,
            ctx.gfx,
            PANEL_FONT,
            &pts_text,
            cb.x + 140,
            update_y,
            font_cache::TextStyle::PLAIN,
        )?;

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

    fn make_data() -> SkillsPanelData {
        SkillsPanelData {
            attrib: [[10, 0, 100, 5, 0, 10]; 5],
            hp: [50, 0, 200, 10, 0, 50],
            end: [50, 0, 200, 10, 0, 50],
            mana: [50, 0, 200, 10, 0, 50],
            skill: [[0; 6]; 100],
            points: 10000,
            sorted_skills: (0..MAX_SKILLS).collect(),
        }
    }

    #[test]
    fn starts_hidden() {
        let panel = SkillsPanel::new(Bounds::new(0, 0, 300, 250), Color::RGBA(0, 0, 0, 180));
        assert!(!panel.is_visible());
    }

    #[test]
    fn toggle_flips_visibility() {
        let mut panel = SkillsPanel::new(Bounds::new(0, 0, 300, 250), Color::RGBA(0, 0, 0, 180));
        panel.toggle();
        assert!(panel.is_visible());
        panel.toggle();
        assert!(!panel.is_visible());
    }

    #[test]
    fn hidden_panel_ignores_clicks() {
        let mut panel = SkillsPanel::new(Bounds::new(10, 10, 300, 250), Color::RGBA(0, 0, 0, 180));
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
        let mut panel = SkillsPanel::new(Bounds::new(10, 10, 300, 250), Color::RGBA(0, 0, 0, 180));
        panel.toggle();
        panel.update_data(make_data());
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
        let mut panel = SkillsPanel::new(Bounds::new(10, 10, 300, 250), Color::RGBA(0, 0, 0, 180));
        panel.toggle();
        panel.update_data(make_data());
        let resp = panel.handle_event(&UiEvent::MouseClick {
            x: 0,
            y: 0,
            button: MouseButton::Left,
            modifiers: KeyModifiers::default(),
        });
        assert_eq!(resp, EventResponse::Ignored);
    }

    #[test]
    fn mouse_wheel_scrolls_skill_list() {
        let mut panel = SkillsPanel::new(Bounds::new(0, 0, 300, 250), Color::RGBA(0, 0, 0, 180));
        panel.toggle();
        panel.update_data(make_data());

        // Scroll down.
        panel.handle_event(&UiEvent::MouseWheel {
            x: 50,
            y: 50,
            delta: -3,
        });
        assert_eq!(panel.skill_scroll, 3);

        // Scroll up.
        panel.handle_event(&UiEvent::MouseWheel {
            x: 50,
            y: 50,
            delta: 1,
        });
        assert_eq!(panel.skill_scroll, 2);

        // Scroll up past zero.
        panel.handle_event(&UiEvent::MouseWheel {
            x: 50,
            y: 50,
            delta: 10,
        });
        assert_eq!(panel.skill_scroll, 0);
    }

    #[test]
    fn reset_raises_clears_state() {
        let mut panel = SkillsPanel::new(Bounds::new(0, 0, 300, 250), Color::RGBA(0, 0, 0, 180));
        panel.stat_raised[0] = 5;
        panel.stat_points_used = 100;
        panel.reset_raises();
        assert_eq!(panel.stat_raised[0], 0);
        assert_eq!(panel.stat_points_used, 0);
    }

    #[test]
    fn attrib_cost_matches_formula() {
        let data = make_data();
        // v=10, diff=5: (10^3 * 5) / 20 = 5000/20 = 250
        let cost = SkillsPanel::attrib_cost(&data, 0, 10);
        assert_eq!(cost, 250);
    }

    #[test]
    fn update_emits_commit_stats_action() {
        let mut panel = SkillsPanel::new(Bounds::new(0, 0, 300, 250), Color::RGBA(0, 0, 0, 180));
        let data = make_data();
        panel.stat_raised[0] = 3;
        panel.stat_points_used = 100;

        panel.handle_update_click(&data);

        let actions = panel.take_actions();
        assert_eq!(actions.len(), 1);
        match &actions[0] {
            WidgetAction::CommitStats { raises } => {
                assert_eq!(raises.len(), 1);
                assert_eq!(raises[0], (0, 3));
            }
            _ => panic!("Expected CommitStats action"),
        }
        // Raises should be reset after commit.
        assert_eq!(panel.stat_raised[0], 0);
        assert_eq!(panel.stat_points_used, 0);
    }

    #[test]
    fn build_sorted_skills_handles_empty() {
        let skill = [[0u8; 6]; 100];
        let sorted = SkillsPanel::build_sorted_skills(&skill);
        assert_eq!(sorted.len(), MAX_SKILLS);
    }
}
