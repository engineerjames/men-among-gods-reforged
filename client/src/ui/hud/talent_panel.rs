//! Class talent tree panel.
//!
//! Shows every node of the player's class talent tree as a row with a
//! "Learn" button. Clicking a button emits a [`WidgetAction::LearnTalent`]
//! that the scene forwards to the server. A "Reset" button at the bottom
//! refunds every spent point back into the unspent pool.
//!
//! The panel is decoupled from `PlayerState`: GameScene calls
//! [`TalentPanel::sync_state`] each frame with the latest 25-byte talent
//! snapshot and the player's class.

use sdl2::pixels::Color;
use sdl2::render::BlendMode;

use mag_core::talent_trees::{
    TalentNodeMeta, TalentTreeMeta, available_talent_points, is_talent_layer_spent,
    is_talent_spent, talent_prereqs_met, total_points_spent, tree_for,
};
use mag_core::traits::Class;

use crate::font_cache;
use crate::ui::RenderContext;
use crate::ui::style::{Background, Border};
use crate::ui::widget::{Bounds, EventResponse, HudPanel, UiEvent, Widget, WidgetAction};
use crate::ui::widgets::button::RectButton;
use crate::ui::widgets::title_bar::{TITLE_BAR_H, TitleBar, clamp_to_viewport};

/// Bitmap font index used for panel text.
const FONT: usize = 1;

/// Y offset for the first row below the title bar.
const Y_FIRST_ROW: i32 = 8 + TITLE_BAR_H;

/// Per-row vertical spacing.
const ROW_H: i32 = 18;

/// Horizontal inset from panel edges.
const H_INSET: i32 = 8;

/// Width of the "Learn" button on each row.
const LEARN_BTN_W: u32 = 60;

/// Height of the "Learn" button on each row.
const LEARN_BTN_H: u32 = 14;

/// Width of the "Reset" button at the bottom of the panel.
const RESET_BTN_W: u32 = 80;

/// Height of the "Reset" button at the bottom of the panel.
const RESET_BTN_H: u32 = 16;

/// Vertical gap above the bottom reset-button row.
const RESET_AREA_H: i32 = 28;

/// Width of the vertical scrollbar track.
const SCROLLBAR_W: u32 = 6;

/// Minimum scrollbar thumb height.
const SCROLLBAR_THUMB_MIN_H: u32 = 10;

/// Status of a single talent node from the player's perspective.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum NodeStatus {
    /// Node is already learned.
    Learned,
    /// Prereqs satisfied and the player has enough unspent points.
    Available,
    /// Prereqs satisfied but the player lacks the points to learn it.
    NotEnoughPoints,
    /// One or more prereq nodes are not yet learned.
    Locked,
}

/// One row in the talent panel: button + cached metadata.
struct TalentRow {
    meta: &'static TalentNodeMeta,
    button: RectButton,
}

/// The class talent tree HUD panel.
pub struct TalentPanel {
    bounds: Bounds,
    bg_color: Color,
    border_color: Color,
    visible: bool,
    pending_actions: Vec<WidgetAction>,
    title_bar: TitleBar,

    /// Class the rows were built for, or `None` if no rows are built.
    rows_for_class: Option<Class>,
    /// One row per node in the active tree, in tree order.
    rows: Vec<TalentRow>,
    /// First visible row index in `rows`.
    scroll_offset: usize,
    /// "Reset" button at the bottom of the panel.
    reset_button: RectButton,

    /// Latest snapshot of the 25-byte talent state, or `None` until the
    /// first sync.
    talents: Option<[u8; 25]>,
    /// Player's class, or `None` if the kindred bits don't map to a class
    /// that has a tree defined.
    class: Option<Class>,
}

impl TalentPanel {
    /// Creates a new talent panel.
    ///
    /// # Arguments
    ///
    /// * `bounds` - Position and size of the panel.
    /// * `bg_color` - Semi-transparent background color.
    ///
    /// # Returns
    ///
    /// A new `TalentPanel`, initially hidden with no rows built.
    pub fn new(bounds: Bounds, bg_color: Color) -> Self {
        let reset_button = RectButton::new(
            Bounds::new(
                bounds.x + bounds.width as i32 - H_INSET - RESET_BTN_W as i32,
                bounds.y + bounds.height as i32 - 6 - RESET_BTN_H as i32,
                RESET_BTN_W,
                RESET_BTN_H,
            ),
            Background::SolidColor(Color::RGBA(60, 30, 30, 220)),
        )
        .with_label("Reset", FONT)
        .with_border(Border {
            color: Color::RGBA(160, 100, 100, 220),
            width: 1,
        });

        Self {
            bounds,
            bg_color,
            border_color: Color::RGBA(120, 120, 140, 200),
            visible: false,
            pending_actions: Vec::new(),
            title_bar: TitleBar::new("Talents", bounds.x, bounds.y, bounds.width),
            rows_for_class: None,
            rows: Vec::new(),
            scroll_offset: 0,
            reset_button,
            talents: None,
            class: None,
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

    /// Updates the per-frame snapshot of talents and class.
    ///
    /// # Arguments
    ///
    /// * `talents` - The latest 25-byte talent snapshot.
    /// * `class` - The player's class, or `None` if no tree exists.
    pub fn sync_state(&mut self, talents: [u8; 25], class: Option<Class>) {
        self.talents = Some(talents);
        if class != self.class || self.rows_for_class != class {
            self.class = class;
            self.rebuild_rows();
        }
        self.clamp_scroll();
    }

    /// Rebuilds the per-node row buttons for the current `self.class`.
    ///
    /// No-op when the class is `None` or has no tree defined.
    fn rebuild_rows(&mut self) {
        self.rows.clear();
        self.rows_for_class = self.class;
        let Some(class) = self.class else {
            return;
        };
        let Some(tree) = tree_for(class) else {
            return;
        };

        let btn_x = self.bounds.x + self.bounds.width as i32 - H_INSET - LEARN_BTN_W as i32;
        let btn_bg = Background::SolidColor(Color::RGBA(40, 60, 40, 220));
        let btn_border = Border {
            color: Color::RGBA(120, 160, 120, 220),
            width: 1,
        };

        for (i, node) in tree.nodes.iter().enumerate() {
            let y = self.bounds.y + Y_FIRST_ROW + ROW_H * i as i32 + 2;
            let button = RectButton::new(Bounds::new(btn_x, y, LEARN_BTN_W, LEARN_BTN_H), btn_bg)
                .with_label("Learn", FONT)
                .with_border(btn_border);
            self.rows.push(TalentRow { meta: node, button });
        }
        self.scroll_offset = 0;
        self.update_visible_button_positions();
    }

    /// Returns the first row's render Y coordinate.
    fn row_start_y(&self) -> i32 {
        self.bounds.y + Y_FIRST_ROW + 2
    }

    /// Returns the Y coordinate at which row rendering must stop.
    fn row_end_y(&self) -> i32 {
        self.bounds.y + self.bounds.height as i32 - RESET_AREA_H
    }

    /// Returns how many complete rows fit in the visible content area.
    fn visible_row_count(&self) -> usize {
        ((self.row_end_y() - self.row_start_y()) / ROW_H).max(1) as usize
    }

    /// Returns the maximum valid scroll offset for the current row count.
    fn max_scroll_offset(&self) -> usize {
        self.rows.len().saturating_sub(self.visible_row_count())
    }

    /// Clamps `scroll_offset` and keeps visible button bounds in sync.
    fn clamp_scroll(&mut self) {
        self.scroll_offset = self.scroll_offset.min(self.max_scroll_offset());
        self.update_visible_button_positions();
    }

    /// Scrolls by `delta_rows` rows and updates row button positions.
    ///
    /// Positive values scroll down; negative values scroll up.
    fn scroll_by(&mut self, delta_rows: i32) {
        let next = if delta_rows >= 0 {
            self.scroll_offset.saturating_add(delta_rows as usize)
        } else {
            self.scroll_offset
                .saturating_sub(delta_rows.unsigned_abs() as usize)
        };
        self.scroll_offset = next.min(self.max_scroll_offset());
        self.update_visible_button_positions();
    }

    /// Returns the currently visible row range as absolute row indices.
    fn visible_range(&self) -> std::ops::Range<usize> {
        let start = self.scroll_offset.min(self.rows.len());
        let end = (start + self.visible_row_count()).min(self.rows.len());
        start..end
    }

    /// Repositions each visible row's Learn button to its on-panel slot.
    fn update_visible_button_positions(&mut self) {
        let btn_x = self.bounds.x + self.bounds.width as i32 - H_INSET - LEARN_BTN_W as i32;
        let row_start_y = self.row_start_y();
        let scroll_offset = self.scroll_offset;
        for (i, row) in self.rows.iter_mut().enumerate() {
            let visible_index = i as i32 - scroll_offset as i32;
            row.button
                .set_position(btn_x, row_start_y + visible_index * ROW_H);
        }
    }

    /// Returns the status of a single node given the current `talents`
    /// snapshot.
    ///
    /// # Arguments
    ///
    /// * `node` - The node to evaluate.
    /// * `talents` - The 25-byte talent state.
    ///
    /// # Returns
    ///
    /// The node's [`NodeStatus`].
    fn node_status(node: &TalentNodeMeta, talents: &[u8; 25]) -> NodeStatus {
        if is_talent_spent(talents, node.slot.mask, node.slot.layer as usize) {
            return NodeStatus::Learned;
        }
        if is_talent_layer_spent(talents, node.slot.layer as usize) {
            return NodeStatus::Locked;
        }
        if !talent_prereqs_met(talents, node) {
            return NodeStatus::Locked;
        }
        if available_talent_points(talents) < node.cost {
            return NodeStatus::NotEnoughPoints;
        }
        NodeStatus::Available
    }

    /// Truncates ASCII UI text to fit within `max_width` pixels.
    ///
    /// # Arguments
    ///
    /// * `text` - Text to fit.
    /// * `max_width` - Maximum pixel width available.
    ///
    /// # Returns
    ///
    /// The original text when it fits, or a `...`-terminated copy.
    fn fit_text(text: &str, max_width: i32) -> String {
        let max_chars = (max_width.max(0) as u32 / font_cache::BITMAP_GLYPH_ADVANCE) as usize;
        if text.len() <= max_chars {
            return text.to_string();
        }
        if max_chars == 0 {
            return String::new();
        }
        if max_chars <= 3 {
            return ".".repeat(max_chars);
        }
        format!("{}...", &text[..max_chars - 3])
    }
}

impl Widget for TalentPanel {
    fn bounds(&self) -> &Bounds {
        &self.bounds
    }

    fn set_position(&mut self, x: i32, y: i32) {
        let dx = x - self.bounds.x;
        let dy = y - self.bounds.y;
        self.bounds.x = x;
        self.bounds.y = y;
        self.title_bar.set_bar_position(x, y);
        for row in &mut self.rows {
            let b = row.button.bounds();
            row.button.set_position(b.x + dx, b.y + dy);
        }
        let rb = self.reset_button.bounds();
        self.reset_button.set_position(rb.x + dx, rb.y + dy);
        self.update_visible_button_positions();
    }

    fn handle_event(&mut self, event: &UiEvent) -> EventResponse {
        if !self.visible {
            return EventResponse::Ignored;
        }

        // Title bar drag/close.
        let (tb_resp, drag_pos) = self.title_bar.handle_event(event);
        if let Some((nx, ny)) = drag_pos {
            let (cx, cy) = clamp_to_viewport(nx, ny, self.bounds.width, self.bounds.height);
            self.set_position(cx, cy);
            return EventResponse::Consumed;
        }
        if self.title_bar.was_close_requested() {
            self.visible = false;
            self.pending_actions
                .push(WidgetAction::TogglePanel(HudPanel::Talents));
            return EventResponse::Consumed;
        }
        if tb_resp == EventResponse::Consumed {
            return EventResponse::Consumed;
        }

        if let UiEvent::MouseWheel { x, y, delta } = event {
            if self.bounds.contains_point(*x, *y) {
                self.scroll_by(-*delta);
                return EventResponse::Consumed;
            }
        }

        // Per-row Learn buttons. Only emit if the visible row is currently
        // Available (the server still validates, but this avoids spam).
        let snapshot = self.talents;
        let visible = self.visible_range();
        for row in self.rows[visible].iter_mut() {
            if row.button.handle_event(event) == EventResponse::Consumed {
                if let Some(t) = snapshot.as_ref() {
                    if Self::node_status(row.meta, t) == NodeStatus::Available {
                        self.pending_actions.push(WidgetAction::LearnTalent {
                            slot: row.meta.slot,
                        });
                    }
                }
                return EventResponse::Consumed;
            }
        }

        // Reset button.
        if self.reset_button.handle_event(event) == EventResponse::Consumed {
            self.pending_actions.push(WidgetAction::ResetTalents);
            return EventResponse::Consumed;
        }

        // Eat clicks/wheel inside our bounds to prevent click-through.
        match event {
            UiEvent::MouseClick { x, y, .. }
            | UiEvent::MouseDown { x, y, .. }
            | UiEvent::MouseWheel { x, y, .. } => {
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

        ctx.canvas.set_blend_mode(BlendMode::Blend);
        ctx.canvas.set_draw_color(self.bg_color);
        ctx.canvas.fill_rect(rect)?;
        ctx.canvas.set_draw_color(self.border_color);
        ctx.canvas.draw_rect(rect)?;

        self.title_bar.render(ctx)?;

        // Header line: unspent points / total spent / class label.
        let header_x = self.bounds.x + H_INSET;
        let header_y = self.bounds.y + TITLE_BAR_H + 2;
        let (avail, spent, class_label) = match (self.talents.as_ref(), self.class) {
            (Some(t), Some(_)) => (
                available_talent_points(t) as u32,
                total_points_spent(t),
                self.class
                    .and_then(|c| tree_for(c))
                    .map(class_label)
                    .unwrap_or("(no tree)"),
            ),
            _ => (0, 0, "(no class)"),
        };
        let header = format!("{}   Unspent: {}   Spent: {}", class_label, avail, spent);
        let header = Self::fit_text(&header, self.bounds.width as i32 - H_INSET * 2);
        font_cache::draw_text(
            ctx.canvas,
            ctx.gfx,
            0,
            &header,
            header_x,
            header_y,
            font_cache::TextStyle::default(),
        )?;

        // Rows.
        if self.talents.is_none() {
            return Ok(());
        }
        let talents = self.talents.unwrap();
        self.update_visible_button_positions();
        let visible = self.visible_range();
        for row in self.rows[visible].iter_mut() {
            let status = Self::node_status(row.meta, &talents);
            let row_y = row.button.bounds().y;
            let label_x = self.bounds.x + H_INSET;

            let (status_tag, name_color) = match status {
                NodeStatus::Learned => ("[X]", Color::RGBA(180, 220, 180, 255)),
                NodeStatus::Available => ("[ ]", Color::RGBA(220, 220, 100, 255)),
                NodeStatus::NotEnoughPoints => ("[ ]", Color::RGBA(180, 180, 180, 255)),
                NodeStatus::Locked => ("[-]", Color::RGBA(120, 120, 120, 255)),
            };
            let line = format!(
                "{} L{} {} (cost {})",
                status_tag, row.meta.slot.layer, row.meta.name, row.meta.cost
            );
            let label_width = row.button.bounds().x - label_x - 4;
            let line = Self::fit_text(&line, label_width);
            font_cache::draw_text(
                ctx.canvas,
                ctx.gfx,
                0,
                &line,
                label_x,
                row_y + 2,
                font_cache::TextStyle::default().with_tint(name_color),
            )?;

            // Only draw the Learn button when the node is actually learnable.
            if status == NodeStatus::Available {
                row.button.render(ctx)?;
            }
        }

        // Reset button.
        self.reset_button.render(ctx)?;
        self.render_scrollbar(ctx)?;

        Ok(())
    }

    fn take_actions(&mut self) -> Vec<WidgetAction> {
        std::mem::take(&mut self.pending_actions)
    }
}

impl TalentPanel {
    /// Renders the vertical scrollbar when the tree has more rows than fit.
    fn render_scrollbar(&self, ctx: &mut RenderContext<'_, '_>) -> Result<(), String> {
        let visible_rows = self.visible_row_count();
        if self.rows.len() <= visible_rows {
            return Ok(());
        }

        let track_h = (self.row_end_y() - self.row_start_y()).max(1) as u32;
        let track_x = self.bounds.x + self.bounds.width as i32 - SCROLLBAR_W as i32 - 2;
        let track_y = self.row_start_y();
        let track = sdl2::rect::Rect::new(track_x, track_y, SCROLLBAR_W, track_h);
        ctx.canvas.set_draw_color(Color::RGBA(40, 40, 55, 180));
        ctx.canvas.fill_rect(track)?;

        let thumb_h = ((track_h as usize * visible_rows) / self.rows.len())
            .max(SCROLLBAR_THUMB_MIN_H as usize)
            .min(track_h as usize) as u32;
        let max_offset = self.max_scroll_offset().max(1);
        let free_h = track_h.saturating_sub(thumb_h) as usize;
        let thumb_y = track_y + ((free_h * self.scroll_offset) / max_offset) as i32;
        let thumb = sdl2::rect::Rect::new(track_x, thumb_y, SCROLLBAR_W, thumb_h);
        ctx.canvas.set_draw_color(Color::RGBA(150, 150, 170, 220));
        ctx.canvas.fill_rect(thumb)?;

        Ok(())
    }
}

/// Returns a short human-readable label for the talent tree's class.
///
/// # Arguments
///
/// * `tree` - The talent tree metadata.
///
/// # Returns
///
/// A `'static` string label such as `"Mercenary"`.
fn class_label(tree: &'static TalentTreeMeta) -> &'static str {
    match tree.class {
        Class::Mercenary => "Mercenary",
        Class::Templar => "Templar",
        Class::Harakim => "Harakim",
        Class::Sorcerer => "Sorcerer",
        Class::Warrior => "Warrior",
        Class::ArchTemplar => "ArchTemplar",
        Class::ArchHarakim => "ArchHarakim",
        Class::SeyanDu => "Seyan Du",
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Make sure newly-constructed panels are hidden and have no rows.
    #[test]
    fn new_panel_is_hidden_and_empty() {
        let p = TalentPanel::new(Bounds::new(0, 0, 200, 200), Color::RGBA(0, 0, 0, 200));
        assert!(!p.is_visible());
        assert!(p.rows.is_empty());
    }

    /// Toggling flips visibility.
    #[test]
    fn toggle_flips_visibility() {
        let mut p = TalentPanel::new(Bounds::new(0, 0, 200, 200), Color::RGBA(0, 0, 0, 200));
        p.toggle();
        assert!(p.is_visible());
        p.toggle();
        assert!(!p.is_visible());
    }

    /// Syncing with a Mercenary class builds one row per tree node.
    #[test]
    fn sync_with_mercenary_builds_rows() {
        let mut p = TalentPanel::new(Bounds::new(0, 0, 300, 600), Color::RGBA(0, 0, 0, 200));
        p.sync_state([0u8; 25], Some(Class::Mercenary));
        let tree = tree_for(Class::Mercenary).unwrap();
        assert_eq!(p.rows.len(), tree.nodes.len());
    }

    /// The row list scrolls when more nodes exist than fit in the panel.
    #[test]
    fn scroll_by_clamps_to_valid_range() {
        let mut p = TalentPanel::new(Bounds::new(0, 0, 300, 120), Color::RGBA(0, 0, 0, 200));
        p.sync_state([0u8; 25], Some(Class::Mercenary));
        assert!(p.max_scroll_offset() > 0);

        p.scroll_by(999);
        assert_eq!(p.scroll_offset, p.max_scroll_offset());

        p.scroll_by(-999);
        assert_eq!(p.scroll_offset, 0);
    }

    /// Visible range excludes rows above and below the scrolled viewport.
    #[test]
    fn visible_range_tracks_scroll_offset() {
        let mut p = TalentPanel::new(Bounds::new(0, 0, 300, 120), Color::RGBA(0, 0, 0, 200));
        p.sync_state([0u8; 25], Some(Class::Mercenary));
        p.scroll_by(2);

        let range = p.visible_range();
        assert_eq!(range.start, 2);
        assert!(range.end <= p.rows.len());
    }

    /// Long row labels are shortened to fit the available pixel width.
    #[test]
    fn fit_text_truncates_long_text() {
        let max_width = font_cache::BITMAP_GLYPH_ADVANCE as i32 * 10;
        let fitted = TalentPanel::fit_text("Protective Spells Boost 1", max_width);
        assert_eq!(fitted, "Protect...");
    }

    /// Short labels are kept unchanged.
    #[test]
    fn fit_text_preserves_short_text() {
        let max_width = font_cache::BITMAP_GLYPH_ADVANCE as i32 * 20;
        let fitted = TalentPanel::fit_text("Distract", max_width);
        assert_eq!(fitted, "Distract");
    }

    /// Status: a learned node reports `Learned`.
    #[test]
    fn node_status_learned() {
        let mut talents = [0u8; 25];
        talents[1] = 0b01; // mark layer 1 mask 1 (DISTRACT) as spent
        let tree = tree_for(Class::Mercenary).unwrap();
        let distract = tree.nodes.first().unwrap();
        assert_eq!(distract.slot.layer, 1);
        assert_eq!(distract.slot.mask, 0x01);
        assert_eq!(
            TalentPanel::node_status(distract, &talents),
            NodeStatus::Learned
        );
    }

    /// Status: a node with no prereqs and zero unspent points reports
    /// `NotEnoughPoints`.
    #[test]
    fn node_status_not_enough_points() {
        let talents = [0u8; 25];
        let tree = tree_for(Class::Mercenary).unwrap();
        let distract = tree.nodes.first().unwrap();
        assert_eq!(
            TalentPanel::node_status(distract, &talents),
            NodeStatus::NotEnoughPoints
        );
    }

    /// Status: a no-prereq node with at least one unspent point is
    /// `Available`.
    #[test]
    fn node_status_available() {
        let mut talents = [0u8; 25];
        talents[0] = 5;
        let tree = tree_for(Class::Mercenary).unwrap();
        let distract = tree.nodes.first().unwrap();
        assert_eq!(
            TalentPanel::node_status(distract, &talents),
            NodeStatus::Available
        );
    }

    /// Status: a node with unmet prereqs is `Locked` even when there are
    /// plenty of unspent points.
    #[test]
    fn node_status_locked() {
        let mut talents = [0u8; 25];
        talents[0] = 99;
        let tree = tree_for(Class::Mercenary).unwrap();
        let prereq_node = tree.nodes.iter().find(|n| !n.prereqs.is_empty()).unwrap();
        assert_eq!(
            TalentPanel::node_status(prereq_node, &talents),
            NodeStatus::Locked
        );
    }

    /// Status: learning one root talent unlocks the next layer without
    /// requiring both root options.
    #[test]
    fn node_status_next_layer_available_after_one_prior_pick() {
        let mut talents = [0u8; 25];
        talents[0] = 1;
        talents[1] = 0b01;
        let tree = tree_for(Class::Mercenary).unwrap();
        let dodge = tree
            .nodes
            .iter()
            .find(|n| n.name == "Dodge Boost I")
            .unwrap();
        assert_eq!(
            TalentPanel::node_status(dodge, &talents),
            NodeStatus::Available
        );
    }

    /// Status: once a layer has a learned talent, its sibling choices are
    /// locked.
    #[test]
    fn node_status_sibling_locked_after_layer_pick() {
        let mut talents = [0u8; 25];
        talents[0] = 1;
        talents[1] = 0b01;
        let tree = tree_for(Class::Mercenary).unwrap();
        let parasite = tree.nodes.iter().find(|n| n.name == "Parasite").unwrap();
        assert_eq!(
            TalentPanel::node_status(parasite, &talents),
            NodeStatus::Locked
        );
    }
}
