//! Class talent tree panel.
//!
//! Draws the player's class talent tree as a graph laid over the class
//! artwork in `assets/gfx/talents/`. Each artwork is a single wide image whose
//! left and right halves correspond to the two branches of the tree, so nodes
//! are placed by `TalentRef::layer` (row, 1..=12) and `TalentRef::mask`
//! (column: bit 0 = left half, bit 1 = right half). Prerequisite edges are
//! drawn as straight lines between node centers.
//!
//! Hovering a node fills the description box along the bottom of the panel.
//! Left-clicking an available node emits a [`WidgetAction::LearnTalent`] that
//! the scene forwards to the server. A "Reset" button refunds every spent
//! point back into the unspent pool.
//!
//! The panel is decoupled from `PlayerState`: GameScene calls
//! [`TalentPanel::sync_state`] each frame with the latest 25-byte talent
//! snapshot and the player's class.

use sdl2::pixels::Color;
use sdl2::render::BlendMode;

use std::time::Duration;

use mag_core::ranks::{rank_name_by_index, talent_rank_for_layer};
use mag_core::seyan_runes::ALL as ALL_RUNES;
use mag_core::talent_trees::{
    TalentNode, TalentRef, available_talent_points, is_talent_layer_spent, is_talent_spent,
    talent_prereqs_met, total_points_spent, tree_for,
};
use mag_core::traits::Class;

use crate::filepaths;
use crate::font_cache;
use crate::ui::RenderContext;
use crate::ui::style::{Background, Border};
use crate::ui::widget::{
    Bounds, EventResponse, HudPanel, MouseButton, UiEvent, Widget, WidgetAction,
};
use crate::ui::widgets::button::RectButton;
use crate::ui::widgets::radio_group::{draw_circle, draw_filled_circle};
use crate::ui::widgets::title_bar::{TITLE_BAR_H, TitleBar, clamp_to_viewport};

/// Bitmap font index used for button labels (yellow).
const FONT: usize = 1;

/// Bitmap font index used for tintable body text.
const TEXT_FONT: usize = 0;

/// Horizontal inset from panel edges.
const H_INSET: i32 = 8;

/// Height of the header strip between the title bar and the artwork.
const HEADER_H: i32 = 20;

/// Height of the reserved artwork viewport.
const ART_VIEWPORT_H: u32 = 400;

/// Width of the reserved artwork viewport.
const ART_VIEWPORT_W: u32 = 600;

/// Vertical gap between the artwork and the description box.
const ART_GAP: i32 = 6;

/// Height of the description box at the bottom of the panel.
const TOOLTIP_H: u32 = 70;

/// Padding inside the description box.
const TOOLTIP_PAD: i32 = 6;

/// Vertical padding below the description box.
const BOTTOM_PAD: i32 = 6;

/// Edge length of a talent node square.
const NODE_SIZE: u32 = 20;

/// Number of talent layers (rows) rendered in the tree.
const TALENT_ROWS: i32 = 12;

/// Radius in pixels of a Seyan'Du rune slot circle.
const RUNE_SLOT_RADIUS: i32 = 12;

/// Vertical gap in pixels between stacked rune slot circles.
const RUNE_SLOT_GAP: i32 = 32;

/// Placeholder fill color for each rune slot, in [`mag_core::seyan_runes::SeyanRune`] order.
const RUNE_SLOT_COLORS: [Color; 4] = [
    Color::RGBA(90, 140, 235, 235),
    Color::RGBA(190, 70, 200, 235),
    Color::RGBA(150, 170, 60, 235),
    Color::RGBA(220, 90, 90, 235),
];

/// Blends `color` toward grey to indicate a cooldown-disabled rune slot.
fn dim_color(color: Color) -> Color {
    let mix = |c: u8| ((u16::from(c) + 90) / 2) as u8;
    Color::RGBA(
        mix(color.r),
        mix(color.g),
        mix(color.b),
        color.a.saturating_sub(60),
    )
}

/// Converts a server tick count into a wall-clock [`Duration`], using
/// [`mag_core::constants::TICKS`] as ticks-per-second.
fn ticks_to_duration(ticks: u16) -> Duration {
    Duration::from_secs_f64(f64::from(ticks) / f64::from(mag_core::constants::TICKS))
}

/// Thickness in pixels of the bright core of a prerequisite connector.
const EDGE_CORE_W: u32 = 3;

/// Pixels of dark casing drawn on every side of a connector core so the line
/// stays readable over both bright and dark artwork.
const EDGE_CASING_PAD: i32 = 1;

/// Casing color drawn underneath every connector core.
const EDGE_CASING_COLOR: Color = Color::RGBA(6, 6, 12, 235);

/// Core color for a connector whose parent and child are both learned.
const EDGE_ACTIVE_COLOR: Color = Color::RGBA(150, 240, 120, 255);

/// Core color for a connector that is not yet fully unlocked.
const EDGE_IDLE_COLOR: Color = Color::RGBA(168, 168, 190, 225);

/// Width of the "Reset" button in the header strip.
const RESET_BTN_W: u32 = 80;

/// Height of the "Reset" button in the header strip.
const RESET_BTN_H: u32 = 16;

/// Panel width required by the artwork viewport plus horizontal insets.
pub const TALENT_PANEL_W: u32 = ART_VIEWPORT_W + (H_INSET as u32) * 2;

/// Panel height required by the title bar, header strip, artwork viewport,
/// description box and bottom padding.
pub const TALENT_PANEL_H: u32 = TITLE_BAR_H as u32
    + HEADER_H as u32
    + ART_VIEWPORT_H
    + ART_GAP as u32
    + TOOLTIP_H
    + BOTTOM_PAD as u32;

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

impl NodeStatus {
    /// Returns the interior fill and outline colors for a node square.
    ///
    /// # Returns
    ///
    /// * `(fill, border)` colors.
    fn colors(self) -> (Color, Color) {
        match self {
            NodeStatus::Learned => (
                Color::RGBA(46, 120, 60, 235),
                Color::RGBA(160, 240, 170, 255),
            ),
            NodeStatus::Available => (
                Color::RGBA(126, 104, 26, 235),
                Color::RGBA(255, 226, 90, 255),
            ),
            NodeStatus::NotEnoughPoints => (
                Color::RGBA(62, 62, 74, 235),
                Color::RGBA(170, 170, 182, 255),
            ),
            NodeStatus::Locked => (Color::RGBA(24, 24, 32, 225), Color::RGBA(88, 88, 100, 255)),
        }
    }

    /// Returns the text tint used for this status in the description box.
    ///
    /// # Returns
    ///
    /// * The tint color.
    fn text_color(self) -> Color {
        match self {
            NodeStatus::Learned => Color::RGBA(180, 230, 180, 255),
            NodeStatus::Available => Color::RGBA(230, 220, 110, 255),
            NodeStatus::NotEnoughPoints => Color::RGBA(190, 190, 195, 255),
            NodeStatus::Locked => Color::RGBA(140, 140, 150, 255),
        }
    }

    /// Returns a short human-readable label for this status.
    ///
    /// # Returns
    ///
    /// * A `'static` label such as `"Available"`.
    fn label(self) -> &'static str {
        match self {
            NodeStatus::Learned => "Learned",
            NodeStatus::Available => "Available",
            NodeStatus::NotEnoughPoints => "Not enough talent points",
            NodeStatus::Locked => "Locked",
        }
    }
}

/// One placed talent node: static metadata plus its on-screen square.
struct TalentNodeSlot {
    /// Static tree metadata for this node.
    meta: &'static TalentNode,
    /// Hit-test and render rectangle of the node square.
    rect: Bounds,
}

/// The class talent tree HUD panel.
pub struct TalentPanel {
    bounds: Bounds,
    bg_color: Color,
    border_color: Color,
    visible: bool,
    pending_actions: Vec<WidgetAction>,
    title_bar: TitleBar,

    /// "Reset" button in the header strip.
    reset_button: RectButton,

    /// Placed nodes for the active tree, in tree order.
    nodes: Vec<TalentNodeSlot>,
    /// Class the nodes were placed for, or `None` if none are placed.
    nodes_for_class: Option<Class>,
    /// Rectangle the artwork is actually drawn into (aspect-preserving fit
    /// inside the reserved viewport). Node positions are relative to this.
    art_rect: Bounds,

    /// Sprite ID of the loaded class artwork, if any.
    bg_texture_id: Option<usize>,
    /// Set once the artwork failed to load so we stop retrying every frame.
    bg_load_failed: bool,

    /// Last known cursor X in logical viewport coordinates.
    mouse_x: i32,
    /// Last known cursor Y in logical viewport coordinates.
    mouse_y: i32,

    /// Latest snapshot of the 25-byte talent state, or `None` until the
    /// first sync.
    talents: Option<[u8; 25]>,
    /// Player's class, or `None` if the kindred bits don't map to a class
    /// that has a tree defined.
    class: Option<Class>,

    /// Placed rune slot circles, only populated for [`Class::SeyanDu`].
    rune_slots: [Bounds; 4],
    /// Latest server snapshot of the active rune index (`0..=3`).
    active_rune: u8,
    /// Time remaining before the active rune can be swapped again.
    rune_swap_cooldown_remaining: Duration,
    /// Raw tick count from the last `sync_rune_state` call, so repeated
    /// syncs of the same stale server snapshot (sent every frame) don't
    /// clobber the locally ticking `rune_swap_cooldown_remaining`.
    last_synced_cooldown_ticks: Option<u16>,
}

impl TalentPanel {
    /// Creates a new talent panel.
    ///
    /// # Arguments
    ///
    /// * `bounds` - Position and size of the panel. Use [`TALENT_PANEL_W`] and
    ///   [`TALENT_PANEL_H`] so the artwork viewport is not clipped.
    /// * `bg_color` - Semi-transparent background color.
    ///
    /// # Returns
    ///
    /// A new `TalentPanel`, initially hidden with no nodes placed.
    pub fn new(bounds: Bounds, bg_color: Color) -> Self {
        let reset_button = RectButton::new(
            Bounds::new(
                bounds.x + bounds.width as i32 - H_INSET - RESET_BTN_W as i32,
                bounds.y + TITLE_BAR_H + 2,
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

        let art_rect = Self::art_viewport(&bounds);

        Self {
            bounds,
            bg_color,
            border_color: Color::RGBA(120, 120, 140, 200),
            visible: false,
            pending_actions: Vec::new(),
            title_bar: TitleBar::new("Talents", bounds.x, bounds.y, bounds.width),
            reset_button,
            nodes: Vec::new(),
            nodes_for_class: None,
            art_rect,
            bg_texture_id: None,
            bg_load_failed: false,
            mouse_x: -1,
            mouse_y: -1,
            talents: None,
            class: None,
            rune_slots: [Bounds::new(0, 0, 0, 0); 4],
            active_rune: 0,
            rune_swap_cooldown_remaining: Duration::ZERO,
            last_synced_cooldown_ticks: None,
        }
    }

    /// Toggles the panel's visibility.
    pub fn toggle(&mut self) {
        self.visible = !self.visible;
    }

    /// Returns whether the panel is currently visible.
    ///
    /// # Returns
    ///
    /// * `true` when the panel is shown, otherwise `false`.
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Updates the per-frame snapshot of talents and class.
    ///
    /// Changing class discards the cached artwork and re-places every node.
    ///
    /// # Arguments
    ///
    /// * `talents` - The latest 25-byte talent snapshot.
    /// * `class` - The player's class, or `None` if no tree exists.
    pub fn sync_state(&mut self, talents: [u8; 25], class: Option<Class>) {
        self.talents = Some(talents);
        if class != self.class || self.nodes_for_class != class {
            self.class = class;
            self.bg_texture_id = None;
            self.bg_load_failed = false;
            self.art_rect = Self::art_viewport(&self.bounds);
            self.rebuild_nodes();
            self.rebuild_rune_slots();
        }
    }

    /// Updates the per-frame snapshot of the active Seyan'Du rune and
    /// remaining swap cooldown. No-op for classes without a rune loadout.
    ///
    /// # Arguments
    ///
    /// * `active_rune` - The active rune slot index (`0..=3`).
    /// * `cooldown_remaining_ticks` - Remaining swap cooldown, in server ticks.
    pub fn sync_rune_state(&mut self, active_rune: u8, cooldown_remaining_ticks: u16) {
        self.active_rune = active_rune;
        // The caller resyncs every render frame from the last-received
        // packet, not just when a new one arrives; only reset the
        // locally-ticking countdown when the server value actually changed.
        if self.last_synced_cooldown_ticks != Some(cooldown_remaining_ticks) {
            self.rune_swap_cooldown_remaining = ticks_to_duration(cooldown_remaining_ticks);
            self.last_synced_cooldown_ticks = Some(cooldown_remaining_ticks);
        }
    }

    /// Returns the reserved artwork viewport for a panel rectangle.
    ///
    /// # Arguments
    ///
    /// * `bounds` - The panel rectangle.
    ///
    /// # Returns
    ///
    /// * The artwork viewport in absolute screen coordinates.
    fn art_viewport(bounds: &Bounds) -> Bounds {
        let chrome = (TITLE_BAR_H + HEADER_H + ART_GAP + BOTTOM_PAD) as u32 + TOOLTIP_H;
        Bounds::new(
            bounds.x + H_INSET,
            bounds.y + TITLE_BAR_H + HEADER_H,
            bounds.width.saturating_sub(H_INSET as u32 * 2),
            bounds.height.saturating_sub(chrome),
        )
    }

    /// Returns the description box rectangle for the current panel bounds.
    ///
    /// # Returns
    ///
    /// * The description box in absolute screen coordinates.
    fn tooltip_rect(&self) -> Bounds {
        let viewport = Self::art_viewport(&self.bounds);
        Bounds::new(
            self.bounds.x + H_INSET,
            viewport.y + viewport.height as i32 + ART_GAP,
            self.bounds.width.saturating_sub(H_INSET as u32 * 2),
            TOOLTIP_H,
        )
    }

    /// Scales `(tex_w, tex_h)` to fit inside `viewport` without distortion and
    /// centers the result.
    ///
    /// # Arguments
    ///
    /// * `viewport` - Rectangle to fit inside.
    /// * `tex_w` - Source texture width in pixels.
    /// * `tex_h` - Source texture height in pixels.
    ///
    /// # Returns
    ///
    /// * The centered, aspect-preserving destination rectangle, or `viewport`
    ///   unchanged when either dimension is zero.
    fn fit_preserving_aspect(viewport: Bounds, tex_w: u32, tex_h: u32) -> Bounds {
        if tex_w == 0 || tex_h == 0 || viewport.width == 0 || viewport.height == 0 {
            return viewport;
        }
        let scale = f64::min(
            f64::from(viewport.width) / f64::from(tex_w),
            f64::from(viewport.height) / f64::from(tex_h),
        );
        let w = ((f64::from(tex_w) * scale).round() as u32).max(1);
        let h = ((f64::from(tex_h) * scale).round() as u32).max(1);
        Bounds::new(
            viewport.x + (viewport.width as i32 - w as i32) / 2,
            viewport.y + (viewport.height as i32 - h as i32) / 2,
            w,
            h,
        )
    }

    /// Rebuilds the placed node squares for the current class and
    /// [`TalentPanel::art_rect`].
    ///
    /// Nodes are laid out on a grid whose row count matches the tree's
    /// highest talent layer (falling back to [`TALENT_ROWS`] for an empty
    /// tree), so a shorter tree like Seyan'Du's fills the artwork instead of
    /// clustering in the top rows. Column 0 (`mask` bit 0) is centered on the
    /// left half of the artwork and column 1 (`mask` bit 1) on the right
    /// half, matching how the class artwork is split. No-op when the class
    /// is `None` or has no tree defined.
    fn rebuild_nodes(&mut self) {
        self.nodes.clear();
        self.nodes_for_class = self.class;
        let Some(class) = self.class else {
            return;
        };
        let Some(tree) = tree_for(class) else {
            return;
        };

        let rows = tree
            .nodes
            .iter()
            .map(|node| i32::from(node.slot.layer))
            .max()
            .unwrap_or(TALENT_ROWS);

        let art = self.art_rect;
        let pitch = (art.height as i32 / rows).max(NODE_SIZE as i32);
        let half = NODE_SIZE as i32 / 2;

        for node in tree.nodes.iter() {
            let row = i32::from(node.slot.layer).saturating_sub(1);
            let col = node.slot.mask.trailing_zeros().min(1) as i32;
            let cy = art.y + pitch * row + pitch / 2;
            let cx = art.x + (art.width as i32 * (1 + 2 * col)) / 4;
            self.nodes.push(TalentNodeSlot {
                meta: node,
                rect: Bounds::new(cx - half, cy - half, NODE_SIZE, NODE_SIZE),
            });
        }
    }

    /// Rebuilds the 4 rune slot circles for Seyan'Du in whichever letterbox
    /// margin (left or right of the fitted artwork, within the reserved
    /// viewport) currently has more room. No-op — and clears any previously
    /// placed slots — for every other class.
    fn rebuild_rune_slots(&mut self) {
        self.rune_slots = [Bounds::new(0, 0, 0, 0); 4];
        if self.class != Some(Class::SeyanDu) {
            return;
        }

        let viewport = Self::art_viewport(&self.bounds);
        let art = self.art_rect;
        let left_margin = art.x - viewport.x;
        let right_margin = (viewport.x + viewport.width as i32) - (art.x + art.width as i32);

        let center_x = if right_margin >= left_margin {
            art.x + art.width as i32 + right_margin / 2
        } else {
            viewport.x + left_margin / 2
        };

        let total_span = RUNE_SLOT_GAP * (self.rune_slots.len() as i32 - 1);
        let start_y = viewport.y + (viewport.height as i32 - total_span) / 2;

        for (i, slot) in self.rune_slots.iter_mut().enumerate() {
            let cy = start_y + RUNE_SLOT_GAP * i as i32;
            *slot = Bounds::new(
                center_x - RUNE_SLOT_RADIUS,
                cy - RUNE_SLOT_RADIUS,
                (RUNE_SLOT_RADIUS * 2) as u32,
                (RUNE_SLOT_RADIUS * 2) as u32,
            );
        }
    }

    /// Returns the center point of the node occupying `slot`.
    ///
    /// # Arguments
    ///
    /// * `slot` - Tree coordinates of the node to locate.
    ///
    /// # Returns
    ///
    /// * `Some((x, y))` when the slot is placed, otherwise `None`.
    fn node_center(&self, slot: TalentRef) -> Option<(i32, i32)> {
        self.nodes
            .iter()
            .find(|n| n.meta.slot.layer == slot.layer && n.meta.slot.mask == slot.mask)
            .map(|n| {
                (
                    n.rect.x + n.rect.width as i32 / 2,
                    n.rect.y + n.rect.height as i32 / 2,
                )
            })
    }

    /// Returns the index of the node currently under the cursor.
    ///
    /// # Returns
    ///
    /// * `Some(index)` into `self.nodes`, or `None` when the panel is hidden
    ///   or no node is hovered.
    fn hovered_node(&self) -> Option<usize> {
        if !self.visible {
            return None;
        }
        self.nodes
            .iter()
            .position(|n| n.rect.contains_point(self.mouse_x, self.mouse_y))
    }

    /// Returns the index of the rune slot currently under the cursor.
    ///
    /// # Returns
    ///
    /// * `Some(index)` in `0..4`, or `None` when the panel is hidden, the
    ///   class has no rune loadout, or no slot is hovered.
    fn hovered_rune_slot(&self) -> Option<usize> {
        if !self.visible || self.class != Some(Class::SeyanDu) {
            return None;
        }
        self.rune_slots
            .iter()
            .position(|r| r.contains_point(self.mouse_x, self.mouse_y))
    }

    /// Returns the artwork file name for a class, if one exists.
    ///
    /// # Arguments
    ///
    /// * `class` - Player class to look up.
    ///
    /// # Returns
    ///
    /// * `Some(file_name)` relative to `assets/gfx/talents/`, or `None` for
    ///   classes without a talent tree.
    fn bg_asset_name(class: Class) -> Option<&'static str> {
        match class {
            Class::Harakim | Class::ArchHarakim => Some("harakim.png"),
            Class::Mercenary | Class::Warrior | Class::Sorcerer => Some("merc.png"),
            Class::Templar | Class::ArchTemplar => Some("templar.png"),
            Class::SeyanDu => Some("seyan_du.png"),
            Class::Monster => None,
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
    fn node_status(node: &TalentNode, talents: &[u8; 25]) -> NodeStatus {
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

    /// Formats the cost, required rank, and current status for a talent.
    ///
    /// # Arguments
    ///
    /// * `node` - Talent metadata containing the cost and tree layer.
    /// * `status` - Current client-side availability status.
    ///
    /// # Returns
    ///
    /// * Player-facing footer text for the talent description box.
    fn description_footer(node: &TalentNode, status: NodeStatus) -> String {
        let required_rank = talent_rank_for_layer(node.slot.layer)
            .map(|rank| rank_name_by_index(rank.index()))
            .unwrap_or("Unknown rank");

        format!(
            "Cost: {}  -  Requires: {}  -  {}",
            node.cost,
            required_rank,
            status.label()
        )
    }
}

impl Widget for TalentPanel {
    fn bounds(&self) -> &Bounds {
        &self.bounds
    }

    fn set_position(&mut self, x: i32, y: i32) {
        let dx = x - self.bounds.x;
        let dy = y - self.bounds.y;
        if dx == 0 && dy == 0 {
            return;
        }
        self.bounds.x = x;
        self.bounds.y = y;
        self.title_bar.set_bar_position(x, y);
        let rb = self.reset_button.bounds();
        self.reset_button.set_position(rb.x + dx, rb.y + dy);
        self.art_rect.x += dx;
        self.art_rect.y += dy;
        for node in &mut self.nodes {
            node.rect.x += dx;
            node.rect.y += dy;
        }
        for slot in &mut self.rune_slots {
            slot.x += dx;
            slot.y += dy;
        }
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

        if let UiEvent::MouseMove { x, y } = event {
            self.mouse_x = *x;
            self.mouse_y = *y;
        }

        // Left-click a node square to spend a point. Only emit when the node
        // is currently learnable; the server still validates.
        if let UiEvent::MouseClick {
            x,
            y,
            button: MouseButton::Left,
            ..
        } = event
            && let Some(talents) = self.talents
            && let Some(slot) = self
                .nodes
                .iter()
                .find(|n| n.rect.contains_point(*x, *y))
                .filter(|n| Self::node_status(n.meta, &talents) == NodeStatus::Available)
                .map(|n| n.meta.slot)
        {
            self.pending_actions
                .push(WidgetAction::LearnTalent { slot });
            return EventResponse::Consumed;
        }

        // Left-click a rune slot to activate it, unless a swap is on cooldown.
        if let UiEvent::MouseClick {
            x,
            y,
            button: MouseButton::Left,
            ..
        } = event
            && self.class == Some(Class::SeyanDu)
            && self.rune_swap_cooldown_remaining.is_zero()
            && let Some(rune_idx) = self
                .rune_slots
                .iter()
                .position(|r| r.contains_point(*x, *y))
                .filter(|&idx| idx as u8 != self.active_rune)
        {
            self.pending_actions.push(WidgetAction::SetActiveRune {
                rune_idx: rune_idx as u8,
            });
            return EventResponse::Consumed;
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

    fn update(&mut self, dt: Duration) {
        self.rune_swap_cooldown_remaining = self.rune_swap_cooldown_remaining.saturating_sub(dt);
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
        self.render_header(ctx)?;
        self.reset_button.render(ctx)?;
        self.render_artwork(ctx)?;

        if let Some(talents) = self.talents {
            self.render_edges(ctx, &talents)?;
            self.render_nodes(ctx, &talents)?;
        }
        self.render_runes(ctx)?;
        self.render_description(ctx)?;

        Ok(())
    }

    fn take_actions(&mut self) -> Vec<WidgetAction> {
        std::mem::take(&mut self.pending_actions)
    }
}

impl TalentPanel {
    /// Draws the header strip with the class name and point totals.
    ///
    /// # Arguments
    ///
    /// * `ctx` - Render context.
    ///
    /// # Returns
    ///
    /// * `Ok(())` on success, or an SDL2 error string.
    fn render_header(&self, ctx: &mut RenderContext<'_, '_>) -> Result<(), String> {
        let label = match self.class {
            Some(class) if tree_for(class).is_some() => class.to_string(),
            Some(_) => "(no tree)",
            None => "(no class)",
        };
        let (avail, spent) = match self.talents.as_ref() {
            Some(t) => (u32::from(available_talent_points(t)), total_points_spent(t)),
            None => (0, 0),
        };
        let header = format!("{}   Unspent: {}   Spent: {}", label, avail, spent);
        font_cache::draw_text(
            ctx.canvas,
            ctx.gfx,
            TEXT_FONT,
            &header,
            self.bounds.x + H_INSET,
            self.bounds.y + TITLE_BAR_H + 4,
            font_cache::TextStyle::default(),
        )
    }

    /// Lazily loads and draws the class artwork, updating
    /// [`TalentPanel::art_rect`] and the node layout when the fitted
    /// rectangle changes.
    ///
    /// Missing or unreadable artwork is tolerated: the tree is still drawn
    /// over the reserved viewport.
    ///
    /// # Arguments
    ///
    /// * `ctx` - Render context.
    ///
    /// # Returns
    ///
    /// * `Ok(())` on success, or an SDL2 error string.
    fn render_artwork(&mut self, ctx: &mut RenderContext<'_, '_>) -> Result<(), String> {
        let viewport = Self::art_viewport(&self.bounds);

        if self.bg_texture_id.is_none()
            && !self.bg_load_failed
            && let Some(class) = self.class
        {
            match Self::bg_asset_name(class) {
                Some(file) => {
                    let path = filepaths::get_asset_directory()
                        .join("gfx")
                        .join("talents")
                        .join(file);
                    match ctx.gfx.load_texture_from_path(&path) {
                        Ok(id) => self.bg_texture_id = Some(id),
                        Err(err) => {
                            log::warn!("Talent artwork unavailable ({}): {}", path.display(), err);
                            self.bg_load_failed = true;
                        }
                    }
                }
                None => self.bg_load_failed = true,
            }
        }

        let fitted = match self.bg_texture_id {
            Some(id) => {
                let (tex_w, tex_h) = ctx.gfx.query_texture_size(id);
                Self::fit_preserving_aspect(viewport, tex_w, tex_h)
            }
            None => viewport,
        };

        if fitted != self.art_rect {
            self.art_rect = fitted;
            self.rebuild_nodes();
            self.rebuild_rune_slots();
        }

        if let Some(id) = self.bg_texture_id {
            let dst = sdl2::rect::Rect::new(
                self.art_rect.x,
                self.art_rect.y,
                self.art_rect.width,
                self.art_rect.height,
            );
            let tex = ctx.gfx.get_texture(id);
            ctx.canvas.copy(tex, None, Some(dst))?;
        }

        Ok(())
    }

    /// Builds a vertical connector segment centered on `x`.
    ///
    /// # Arguments
    ///
    /// * `x` - Center column of the segment.
    /// * `y0` - One endpoint on the y axis.
    /// * `y1` - The other endpoint on the y axis.
    ///
    /// # Returns
    ///
    /// * A rectangle covering the segment, at least one pixel tall.
    fn v_seg(x: i32, y0: i32, y1: i32) -> sdl2::rect::Rect {
        let (top, bottom) = if y0 <= y1 { (y0, y1) } else { (y1, y0) };
        sdl2::rect::Rect::new(
            x - EDGE_CORE_W as i32 / 2,
            top,
            EDGE_CORE_W,
            (bottom - top).max(1) as u32,
        )
    }

    /// Builds a horizontal connector segment centered on `y`.
    ///
    /// The segment is extended by half the core width at each end so that
    /// corners join cleanly with the vertical segments it links.
    ///
    /// # Arguments
    ///
    /// * `y` - Center row of the segment.
    /// * `x0` - One endpoint on the x axis.
    /// * `x1` - The other endpoint on the x axis.
    ///
    /// # Returns
    ///
    /// * A rectangle covering the segment, at least one pixel wide.
    fn h_seg(y: i32, x0: i32, x1: i32) -> sdl2::rect::Rect {
        let pad = EDGE_CORE_W as i32 / 2;
        let (left, right) = if x0 <= x1 { (x0, x1) } else { (x1, x0) };
        sdl2::rect::Rect::new(
            left - pad,
            y - pad,
            ((right - left).max(1) as u32) + EDGE_CORE_W,
            EDGE_CORE_W,
        )
    }

    /// Routes a single prerequisite edge as axis-aligned segments.
    ///
    /// Edges between nodes in the same column are a single vertical run.
    /// Edges that change column leave the parent's bottom edge, cross on a
    /// shared horizontal lane halfway between the two rows, then drop into
    /// the child's top edge. Because the lane is shared, sibling edges into
    /// the same layer merge into one readable bus instead of a tangle of
    /// overlapping diagonals.
    ///
    /// # Arguments
    ///
    /// * `parent` - Center of the prerequisite node.
    /// * `child` - Center of the dependent node.
    ///
    /// # Returns
    ///
    /// * One or three rectangles describing the routed edge.
    fn edge_segments(parent: (i32, i32), child: (i32, i32)) -> Vec<sdl2::rect::Rect> {
        let half = NODE_SIZE as i32 / 2;
        let (px, py) = parent;
        let (cx, cy) = child;
        let start = if py <= cy { py + half } else { py - half };
        let end = if py <= cy { cy - half } else { cy + half };

        if px == cx {
            return vec![Self::v_seg(px, start, end)];
        }

        let lane = (start + end) / 2;
        vec![
            Self::v_seg(px, start, lane),
            Self::h_seg(lane, px, cx),
            Self::v_seg(cx, lane, end),
        ]
    }

    /// Draws the prerequisite edges between nodes.
    ///
    /// Edges are drawn before the node squares so the squares sit on top.
    /// Rendering happens in three passes — every casing, then every idle
    /// core, then every active core — so that overlapping segments never
    /// punch dark holes through one another and learned paths win at
    /// intersections.
    ///
    /// # Arguments
    ///
    /// * `ctx` - Render context.
    /// * `talents` - Current 25-byte talent state.
    ///
    /// # Returns
    ///
    /// * `Ok(())` on success, or an SDL2 error string.
    fn render_edges(
        &self,
        ctx: &mut RenderContext<'_, '_>,
        talents: &[u8; 25],
    ) -> Result<(), String> {
        let mut segments: Vec<(sdl2::rect::Rect, bool)> = Vec::new();

        for node in &self.nodes {
            let Some(child) = self.node_center(node.meta.slot) else {
                continue;
            };
            let child_learned =
                is_talent_spent(talents, node.meta.slot.mask, node.meta.slot.layer as usize);

            for prereq in node.meta.prereqs {
                let Some(parent) = self.node_center(*prereq) else {
                    continue;
                };
                let active =
                    child_learned && is_talent_spent(talents, prereq.mask, prereq.layer as usize);
                segments.extend(
                    Self::edge_segments(parent, child)
                        .into_iter()
                        .map(|rect| (rect, active)),
                );
            }
        }

        ctx.canvas.set_draw_color(EDGE_CASING_COLOR);
        for (rect, _) in &segments {
            ctx.canvas.fill_rect(sdl2::rect::Rect::new(
                rect.x() - EDGE_CASING_PAD,
                rect.y() - EDGE_CASING_PAD,
                rect.width() + (EDGE_CASING_PAD as u32) * 2,
                rect.height() + (EDGE_CASING_PAD as u32) * 2,
            ))?;
        }

        for (color, want_active) in [(EDGE_IDLE_COLOR, false), (EDGE_ACTIVE_COLOR, true)] {
            ctx.canvas.set_draw_color(color);
            for (rect, active) in &segments {
                if *active == want_active {
                    ctx.canvas.fill_rect(*rect)?;
                }
            }
        }

        Ok(())
    }

    /// Draws the node squares, tinted by status and outlined when hovered.
    ///
    /// # Arguments
    ///
    /// * `ctx` - Render context.
    /// * `talents` - Current 25-byte talent state.
    ///
    /// # Returns
    ///
    /// * `Ok(())` on success, or an SDL2 error string.
    fn render_nodes(
        &self,
        ctx: &mut RenderContext<'_, '_>,
        talents: &[u8; 25],
    ) -> Result<(), String> {
        let hovered = self.hovered_node();

        for (i, node) in self.nodes.iter().enumerate() {
            let (fill, border) = Self::node_status(node.meta, talents).colors();
            let rect =
                sdl2::rect::Rect::new(node.rect.x, node.rect.y, node.rect.width, node.rect.height);

            ctx.canvas.set_draw_color(fill);
            ctx.canvas.fill_rect(rect)?;
            ctx.canvas.set_draw_color(border);
            ctx.canvas.draw_rect(rect)?;

            if hovered == Some(i) {
                let halo = sdl2::rect::Rect::new(
                    node.rect.x - 2,
                    node.rect.y - 2,
                    node.rect.width + 4,
                    node.rect.height + 4,
                );
                ctx.canvas.set_draw_color(Color::RGBA(255, 255, 255, 235));
                ctx.canvas.draw_rect(halo)?;
            }
        }
        Ok(())
    }

    /// Draws the 4 Seyan'Du rune slot circles, highlighted when active and
    /// dimmed while a swap is on cooldown. No-op for every other class.
    ///
    /// # Arguments
    ///
    /// * `ctx` - Render context.
    ///
    /// # Returns
    ///
    /// * `Ok(())` on success, or an SDL2 error string.
    fn render_runes(&self, ctx: &mut RenderContext<'_, '_>) -> Result<(), String> {
        if self.class != Some(Class::SeyanDu) {
            return Ok(());
        }

        let on_cooldown = !self.rune_swap_cooldown_remaining.is_zero();
        let hovered = self.hovered_rune_slot();

        for (i, rune) in ALL_RUNES.iter().enumerate() {
            let slot = self.rune_slots[i];
            let cx = slot.x + slot.width as i32 / 2;
            let cy = slot.y + slot.height as i32 / 2;
            let is_active = self.active_rune == rune.index();

            let fill = if on_cooldown && !is_active {
                dim_color(RUNE_SLOT_COLORS[i])
            } else {
                RUNE_SLOT_COLORS[i]
            };
            draw_filled_circle(ctx.canvas, cx, cy, RUNE_SLOT_RADIUS - 2, fill)?;

            let border = if is_active {
                Color::RGBA(255, 255, 255, 255)
            } else {
                Color::RGBA(160, 160, 180, 220)
            };
            draw_circle(ctx.canvas, cx, cy, RUNE_SLOT_RADIUS, border)?;
            if is_active {
                draw_circle(ctx.canvas, cx, cy, RUNE_SLOT_RADIUS + 1, border)?;
            }

            if hovered == Some(i) {
                draw_circle(
                    ctx.canvas,
                    cx,
                    cy,
                    RUNE_SLOT_RADIUS + 3,
                    Color::RGBA(255, 255, 255, 200),
                )?;
            }
        }

        Ok(())
    }

    /// Draws the description box for the hovered node, or a hint when nothing
    /// is hovered.
    ///
    /// # Arguments
    ///
    /// * `ctx` - Render context.
    ///
    /// # Returns
    ///
    /// * `Ok(())` on success, or an SDL2 error string.
    fn render_description(&self, ctx: &mut RenderContext<'_, '_>) -> Result<(), String> {
        let box_rect = self.tooltip_rect();
        let rect = sdl2::rect::Rect::new(box_rect.x, box_rect.y, box_rect.width, box_rect.height);
        ctx.canvas.set_draw_color(Color::RGBA(8, 8, 18, 235));
        ctx.canvas.fill_rect(rect)?;
        ctx.canvas.set_draw_color(self.border_color);
        ctx.canvas.draw_rect(rect)?;

        let text_x = box_rect.x + TOOLTIP_PAD;
        let text_y = box_rect.y + TOOLTIP_PAD;
        let max_width = box_rect.width.saturating_sub(TOOLTIP_PAD as u32 * 2);
        let line_h = font_cache::BITMAP_GLYPH_H as i32;

        if let Some(rune_idx) = self.hovered_rune_slot() {
            let rune = ALL_RUNES[rune_idx];
            let tint = Color::RGBA(230, 220, 110, 255);
            font_cache::draw_text(
                ctx.canvas,
                ctx.gfx,
                TEXT_FONT,
                rune.name(),
                text_x,
                text_y,
                font_cache::TextStyle::default().with_tint(tint),
            )?;
            font_cache::draw_wrapped_text(
                ctx.canvas,
                ctx.gfx,
                TEXT_FONT,
                rune.description(),
                text_x,
                text_y + line_h + 2,
                max_width,
                font_cache::TextStyle::default().with_tint(Color::RGBA(210, 210, 220, 255)),
            )?;

            let footer = if self.active_rune == rune.index() {
                "Active".to_owned()
            } else if !self.rune_swap_cooldown_remaining.is_zero() {
                format!(
                    "On cooldown ({:.0}s)",
                    self.rune_swap_cooldown_remaining.as_secs_f64().ceil()
                )
            } else {
                "Click to activate".to_owned()
            };
            return font_cache::draw_text(
                ctx.canvas,
                ctx.gfx,
                TEXT_FONT,
                &footer,
                text_x,
                box_rect.y + box_rect.height as i32 - TOOLTIP_PAD - line_h,
                font_cache::TextStyle::default().with_tint(tint),
            );
        }

        let (Some(index), Some(talents)) = (self.hovered_node(), self.talents) else {
            return font_cache::draw_text(
                ctx.canvas,
                ctx.gfx,
                TEXT_FONT,
                "Hover a talent for details.",
                text_x,
                text_y,
                font_cache::TextStyle::default().with_tint(Color::RGBA(150, 150, 162, 255)),
            );
        };

        let node = self.nodes[index].meta;
        let status = Self::node_status(node, &talents);

        font_cache::draw_text(
            ctx.canvas,
            ctx.gfx,
            TEXT_FONT,
            node.name,
            text_x,
            text_y,
            font_cache::TextStyle::default().with_tint(status.text_color()),
        )?;

        font_cache::draw_wrapped_text(
            ctx.canvas,
            ctx.gfx,
            TEXT_FONT,
            node.description,
            text_x,
            text_y + line_h + 2,
            max_width,
            font_cache::TextStyle::default().with_tint(Color::RGBA(210, 210, 220, 255)),
        )?;

        let footer = Self::description_footer(node, status);
        font_cache::draw_text(
            ctx.canvas,
            ctx.gfx,
            TEXT_FONT,
            &footer,
            text_x,
            box_rect.y + box_rect.height as i32 - TOOLTIP_PAD - line_h,
            font_cache::TextStyle::default().with_tint(status.text_color()),
        )
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Builds a production-sized panel synced to `class`.
    fn panel_for(class: Class) -> TalentPanel {
        let mut p = TalentPanel::new(
            Bounds::new(0, 0, TALENT_PANEL_W, TALENT_PANEL_H),
            Color::RGBA(0, 0, 0, 200),
        );
        p.sync_state([0u8; 25], Some(class));
        p
    }

    /// Returns the center point of a placed node square.
    fn center_of(node: &TalentNodeSlot) -> (i32, i32) {
        (
            node.rect.x + node.rect.width as i32 / 2,
            node.rect.y + node.rect.height as i32 / 2,
        )
    }

    /// Make sure newly-constructed panels are hidden and have no nodes.
    #[test]
    fn new_panel_is_hidden_and_empty() {
        let p = TalentPanel::new(
            Bounds::new(0, 0, TALENT_PANEL_W, TALENT_PANEL_H),
            Color::RGBA(0, 0, 0, 200),
        );
        assert!(!p.is_visible());
        assert!(p.nodes.is_empty());
    }

    /// Toggling flips visibility.
    #[test]
    fn toggle_flips_visibility() {
        let mut p = TalentPanel::new(
            Bounds::new(0, 0, TALENT_PANEL_W, TALENT_PANEL_H),
            Color::RGBA(0, 0, 0, 200),
        );
        p.toggle();
        assert!(p.is_visible());
        p.toggle();
        assert!(!p.is_visible());
    }

    /// The reserved artwork viewport matches the advertised panel size.
    #[test]
    fn art_viewport_matches_reserved_size() {
        let bounds = Bounds::new(0, 0, TALENT_PANEL_W, TALENT_PANEL_H);
        let viewport = TalentPanel::art_viewport(&bounds);
        assert_eq!(viewport.width, ART_VIEWPORT_W);
        assert_eq!(viewport.height, ART_VIEWPORT_H);
    }

    /// The panel places exactly one square per tree node.
    #[test]
    fn sync_places_one_node_per_tree_entry() {
        for class in [
            Class::Mercenary,
            Class::Harakim,
            Class::Templar,
            Class::SeyanDu,
        ] {
            let p = panel_for(class);
            let tree = tree_for(class).unwrap();
            assert_eq!(p.nodes.len(), tree.nodes.len(), "class {:?}", class);
        }
    }

    /// A class without a talent tree places no nodes.
    #[test]
    fn class_without_tree_places_no_nodes() {
        assert!(tree_for(Class::Monster).is_none());
        let p = panel_for(Class::Monster);
        assert!(p.nodes.is_empty());
    }

    /// Nodes with mask bit 0 land entirely on the left half of the artwork
    /// and mask bit 1 entirely on the right half.
    #[test]
    fn nodes_are_placed_on_the_matching_half() {
        for class in [Class::Mercenary, Class::Harakim, Class::Templar] {
            let p = panel_for(class);
            let midline = p.art_rect.x + p.art_rect.width as i32 / 2;
            for node in &p.nodes {
                if node.meta.slot.mask == 0b0000_0001 {
                    assert!(
                        node.rect.x + node.rect.width as i32 <= midline,
                        "{} should sit left of the divider",
                        node.meta.name
                    );
                } else {
                    assert!(
                        node.rect.x >= midline,
                        "{} should sit right of the divider",
                        node.meta.name
                    );
                }
            }
        }
    }

    /// Node rows follow the talent layer and stay inside the artwork.
    #[test]
    fn node_rows_track_layer_and_fit_inside_artwork() {
        let p = panel_for(Class::Mercenary);
        for node in &p.nodes {
            assert!(node.rect.y >= p.art_rect.y, "{} above art", node.meta.name);
            assert!(
                node.rect.y + node.rect.height as i32 <= p.art_rect.y + p.art_rect.height as i32,
                "{} below art",
                node.meta.name
            );
        }

        let mut sorted: Vec<_> = p.nodes.iter().collect();
        sorted.sort_by_key(|n| n.meta.slot.layer);
        for pair in sorted.windows(2) {
            if pair[0].meta.slot.layer < pair[1].meta.slot.layer {
                assert!(
                    pair[0].rect.y < pair[1].rect.y,
                    "layer {} must render above layer {}",
                    pair[0].meta.slot.layer,
                    pair[1].meta.slot.layer
                );
            } else {
                assert_eq!(pair[0].rect.y, pair[1].rect.y);
            }
        }
    }

    /// Every placed slot is resolvable back to its center point.
    #[test]
    fn node_center_resolves_placed_slots() {
        let p = panel_for(Class::Templar);
        for node in &p.nodes {
            assert_eq!(p.node_center(node.meta.slot), Some(center_of(node)));
        }
        assert_eq!(p.node_center(TalentRef { layer: 20, mask: 4 }), None);
    }

    /// Same-column edges route as one vertical run that stops at both node
    /// borders rather than at their centers.
    #[test]
    fn same_column_edge_is_a_single_vertical_run() {
        let half = NODE_SIZE as i32 / 2;
        let segs = TalentPanel::edge_segments((100, 40), (100, 90));

        assert_eq!(segs.len(), 1);
        assert_eq!(segs[0].y(), 40 + half);
        assert_eq!(segs[0].bottom(), 90 - half);
        assert_eq!(segs[0].width(), EDGE_CORE_W);
    }

    /// Column-changing edges route through a shared horizontal lane so that
    /// sibling edges into the same layer overlap into one bus.
    #[test]
    fn cross_column_edges_share_one_horizontal_lane() {
        let left_to_right = TalentPanel::edge_segments((100, 40), (300, 90));
        let right_to_left = TalentPanel::edge_segments((300, 40), (100, 90));

        assert_eq!(left_to_right.len(), 3);
        assert_eq!(right_to_left.len(), 3);
        // Middle segment is horizontal, spans both columns, and both edges
        // put it on the same row.
        assert_eq!(left_to_right[1].height(), EDGE_CORE_W);
        assert_eq!(left_to_right[1].y(), right_to_left[1].y());
        assert!(left_to_right[1].width() >= 200);
        // Verticals leave the parent and enter the child on their own columns.
        assert_eq!(left_to_right[0].x() + EDGE_CORE_W as i32 / 2, 100);
        assert_eq!(left_to_right[2].x() + EDGE_CORE_W as i32 / 2, 300);
    }

    /// Routed segments never overlap the node squares they connect.
    #[test]
    fn edges_stop_outside_the_node_squares() {
        let p = panel_for(Class::Mercenary);
        for node in &p.nodes {
            let child = center_of(node);
            for prereq in node.meta.prereqs {
                let parent = p.node_center(*prereq).expect("prereq is placed");
                for seg in TalentPanel::edge_segments(parent, child) {
                    let node_rect = sdl2::rect::Rect::new(
                        node.rect.x,
                        node.rect.y,
                        node.rect.width,
                        node.rect.height,
                    );
                    assert!(
                        seg.intersection(node_rect).is_none(),
                        "{} edge overlaps its own square",
                        node.meta.name
                    );
                }
            }
        }
    }

    /// Hovering reports the node under the cursor, and only while visible.
    #[test]
    fn hovered_node_requires_visibility_and_a_hit() {
        let mut p = panel_for(Class::Mercenary);
        let (cx, cy) = center_of(&p.nodes[3]);

        p.handle_event(&UiEvent::MouseMove { x: cx, y: cy });
        assert_eq!(p.hovered_node(), None, "hidden panels never report hover");

        p.toggle();
        p.handle_event(&UiEvent::MouseMove { x: cx, y: cy });
        assert_eq!(p.hovered_node(), Some(3));

        p.handle_event(&UiEvent::MouseMove { x: -50, y: -50 });
        assert_eq!(p.hovered_node(), None);
    }

    /// Moving the panel translates the artwork and every node square by the
    /// same delta.
    #[test]
    fn set_position_translates_nodes_and_artwork() {
        let mut p = panel_for(Class::Mercenary);
        let before: Vec<_> = p.nodes.iter().map(|n| (n.rect.x, n.rect.y)).collect();
        let art_before = p.art_rect;

        p.set_position(40, 25);

        assert_eq!(p.art_rect.x, art_before.x + 40);
        assert_eq!(p.art_rect.y, art_before.y + 25);
        for (node, (x, y)) in p.nodes.iter().zip(before) {
            assert_eq!(node.rect.x, x + 40);
            assert_eq!(node.rect.y, y + 25);
        }
    }

    /// Clicking an available node emits a learn action for that exact slot.
    #[test]
    fn click_on_available_node_emits_learn_action() {
        let mut p = panel_for(Class::Mercenary);
        let mut talents = [0u8; 25];
        talents[0] = 1;
        p.sync_state(talents, Some(Class::Mercenary));
        p.toggle();

        let slot = p.nodes[0].meta.slot;
        let (cx, cy) = center_of(&p.nodes[0]);
        p.handle_event(&UiEvent::MouseClick {
            x: cx,
            y: cy,
            button: MouseButton::Left,
            modifiers: Default::default(),
        });

        let actions = p.take_actions();
        assert_eq!(actions.len(), 1);
        assert!(matches!(
            actions[0],
            WidgetAction::LearnTalent { slot: s } if s == slot
        ));
    }

    /// Clicking a locked node emits nothing.
    #[test]
    fn click_on_locked_node_emits_nothing() {
        let mut p = panel_for(Class::Mercenary);
        p.toggle();

        let index = p
            .nodes
            .iter()
            .position(|n| !n.meta.prereqs.is_empty())
            .unwrap();
        let (cx, cy) = center_of(&p.nodes[index]);
        p.handle_event(&UiEvent::MouseClick {
            x: cx,
            y: cy,
            button: MouseButton::Left,
            modifiers: Default::default(),
        });

        assert!(p.take_actions().is_empty());
    }

    /// Every class maps to an artwork file except `Monster`.
    #[test]
    fn bg_asset_name_covers_every_class() {
        assert_eq!(
            TalentPanel::bg_asset_name(Class::Harakim),
            Some("harakim.png")
        );
        assert_eq!(
            TalentPanel::bg_asset_name(Class::ArchHarakim),
            Some("harakim.png")
        );
        assert_eq!(
            TalentPanel::bg_asset_name(Class::Mercenary),
            Some("merc.png")
        );
        assert_eq!(TalentPanel::bg_asset_name(Class::Warrior), Some("merc.png"));
        assert_eq!(
            TalentPanel::bg_asset_name(Class::Sorcerer),
            Some("merc.png")
        );
        assert_eq!(
            TalentPanel::bg_asset_name(Class::Templar),
            Some("templar.png")
        );
        assert_eq!(
            TalentPanel::bg_asset_name(Class::ArchTemplar),
            Some("templar.png")
        );
        assert_eq!(
            TalentPanel::bg_asset_name(Class::SeyanDu),
            Some("seyan_du.png")
        );
        assert_eq!(TalentPanel::bg_asset_name(Class::Monster), None);
    }

    /// Aspect-preserving fit letterboxes and centers without distortion.
    #[test]
    fn fit_preserving_aspect_letterboxes_and_centers() {
        let viewport = Bounds::new(10, 20, 600, 400);

        // 3:2 source exactly fills a 3:2 viewport.
        let fitted = TalentPanel::fit_preserving_aspect(viewport, 1536, 1024);
        assert_eq!((fitted.x, fitted.y), (10, 20));
        assert_eq!((fitted.width, fitted.height), (600, 400));

        // Square source is limited by height and centered horizontally.
        let fitted = TalentPanel::fit_preserving_aspect(viewport, 1000, 1000);
        assert_eq!((fitted.width, fitted.height), (400, 400));
        assert_eq!((fitted.x, fitted.y), (110, 20));

        // Degenerate sources fall back to the viewport.
        assert_eq!(TalentPanel::fit_preserving_aspect(viewport, 0, 0), viewport);
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

    #[test]
    fn description_footer_shows_required_rank_for_representative_layers() {
        let tree = tree_for(Class::Mercenary).unwrap();

        for (layer, rank_name) in [(1, "Private First Class"), (6, "Captain"), (12, "Warlord")] {
            let node = tree
                .nodes
                .iter()
                .find(|node| node.slot.layer == layer)
                .unwrap();
            let footer = TalentPanel::description_footer(node, NodeStatus::Locked);

            assert_eq!(
                footer,
                format!("Cost: {}  -  Requires: {}  -  Locked", node.cost, rank_name)
            );
        }
    }

    /// Rune slots are only placed for Seyan'Du, and are cleared for every
    /// other class.
    #[test]
    fn rune_slots_only_placed_for_seyan_du() {
        let p = panel_for(Class::SeyanDu);
        for slot in &p.rune_slots {
            assert!(slot.width > 0 && slot.height > 0);
        }

        let p = panel_for(Class::Mercenary);
        for slot in &p.rune_slots {
            assert_eq!((slot.width, slot.height), (0, 0));
        }
    }

    /// Clicking an unlocked rune slot emits a `SetActiveRune` action for that
    /// exact index.
    #[test]
    fn click_on_rune_slot_emits_set_active_rune() {
        let mut p = panel_for(Class::SeyanDu);
        p.toggle();

        let slot = p.rune_slots[2];
        let cx = slot.x + slot.width as i32 / 2;
        let cy = slot.y + slot.height as i32 / 2;
        p.handle_event(&UiEvent::MouseClick {
            x: cx,
            y: cy,
            button: MouseButton::Left,
            modifiers: Default::default(),
        });

        let actions = p.take_actions();
        assert_eq!(actions.len(), 1);
        assert!(matches!(
            actions[0],
            WidgetAction::SetActiveRune { rune_idx: 2 }
        ));
    }

    /// Clicking the already-active rune slot must not re-emit `SetActiveRune`
    /// — doing so would needlessly reset its own swap cooldown.
    #[test]
    fn click_on_already_active_rune_slot_emits_nothing() {
        let mut p = panel_for(Class::SeyanDu);
        p.toggle();
        p.sync_rune_state(2, 0);

        let slot = p.rune_slots[2];
        let cx = slot.x + slot.width as i32 / 2;
        let cy = slot.y + slot.height as i32 / 2;
        p.handle_event(&UiEvent::MouseClick {
            x: cx,
            y: cy,
            button: MouseButton::Left,
            modifiers: Default::default(),
        });

        assert!(p.take_actions().is_empty());
    }

    /// Clicking a rune slot while a swap is on cooldown emits nothing.
    #[test]
    fn click_on_rune_slot_during_cooldown_emits_nothing() {
        let mut p = panel_for(Class::SeyanDu);
        p.sync_rune_state(0, 360);
        p.toggle();

        let slot = p.rune_slots[2];
        let cx = slot.x + slot.width as i32 / 2;
        let cy = slot.y + slot.height as i32 / 2;
        p.handle_event(&UiEvent::MouseClick {
            x: cx,
            y: cy,
            button: MouseButton::Left,
            modifiers: Default::default(),
        });

        assert!(p.take_actions().is_empty());
    }

    /// `update` counts the swap cooldown down to zero and no further.
    #[test]
    fn update_counts_down_rune_cooldown() {
        let mut p = panel_for(Class::SeyanDu);
        p.sync_rune_state(1, 36); // 1 second at 36 ticks/sec
        p.update(Duration::from_millis(500));
        assert!(!p.rune_swap_cooldown_remaining.is_zero());
        p.update(Duration::from_secs(10));
        assert!(p.rune_swap_cooldown_remaining.is_zero());
    }

    /// Regression test: the game scene calls `sync_rune_state` every render
    /// frame with the last-received (unchanging) server snapshot, not just
    /// when a new packet arrives. That repeated resync must not clobber the
    /// locally ticking countdown back to the full duration every frame.
    #[test]
    fn repeated_sync_with_same_ticks_does_not_reset_countdown() {
        let mut p = panel_for(Class::SeyanDu);
        p.sync_rune_state(0, 2160); // 60 seconds at 36 ticks/sec
        p.update(Duration::from_secs(30));
        // Simulate the per-frame render-time resync with the same stale value.
        p.sync_rune_state(0, 2160);
        assert!(
            p.rune_swap_cooldown_remaining <= Duration::from_secs(30),
            "resyncing the same ticks value must not reset the countdown"
        );
    }
}
