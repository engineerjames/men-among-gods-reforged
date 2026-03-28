//! Money / inventory / equipment panel.
//!
//! Renders a scrollable 2-column inventory backpack grid (left) and a
//! 2×6 labeled equipment grid (right). When the player carries an item
//! (`citem > 0`), invalid equipment slots are overlaid with a blocking
//! sprite and the carried item follows the mouse cursor.

use sdl2::pixels::Color;
use sdl2::render::BlendMode;

use mag_core::constants::{
    PL_ARMS, PL_BELT, PL_BODY, PL_CLOAK, PL_FEET, PL_HEAD, PL_LEGS, PL_NECK, PL_RING, PL_SHIELD,
    PL_TWOHAND, PL_WEAPON, WN_ARMS, WN_BELT, WN_BODY, WN_CLOAK, WN_FEET, WN_HEAD, WN_LEGS,
    WN_LHAND, WN_LRING, WN_NECK, WN_RHAND, WN_RRING,
};

use crate::font_cache;
use crate::ui::RenderContext;
use crate::ui::widget::{
    Bounds, EventResponse, HudPanel, MouseButton, UiEvent, Widget, WidgetAction,
};
use crate::ui::widgets::title_bar::{TITLE_BAR_H, TitleBar, clamp_to_viewport};

// ---------------------------------------------------------------------------
// Layout constants
// ---------------------------------------------------------------------------

/// Size of each item cell (sprite slot) in pixels.
const CELL: i32 = 35;

/// Visible inventory rows (2 columns × 5 rows = 10 visible slots).
const INV_VISIBLE_ROWS: usize = 5;
/// Total inventory capacity.
const INV_TOTAL_SLOTS: usize = 40;
/// Maximum scroll value (in item-index units; must be even).
const INV_SCROLL_MAX: usize = INV_TOTAL_SLOTS - INV_VISIBLE_ROWS * 2;

/// Padding from the panel's left edge to the inventory grid.
const INV_GRID_PAD_X: i32 = 10;
/// Vertical offset from panel top to the first inventory row.
const INV_GRID_PAD_Y: i32 = 36;

/// Horizontal gap between the inventory grid and the equipment grid.
const GRID_GAP: i32 = 20;

/// Horizontal gap between the two equipment columns.
const EQUIP_COL_GAP: i32 = 8;

/// Equipment grid rows (2 columns × 6 rows = 12 slots).
const EQUIP_ROWS: usize = 6;

/// Scrollbar track dimensions and colors.
const SCROLL_TRACK_W: u32 = 8;
const SCROLL_KNOB_H: u32 = 14;
const SCROLL_TRACK_COLOR: Color = Color::RGBA(40, 40, 60, 160);
const SCROLL_KNOB_COLOR: Color = Color::RGB(8, 77, 23);

/// Dimmed label color for empty equipment slots.
const SLOT_LABEL_COLOR: Color = Color::RGBA(110, 110, 130, 200);

/// Color for the grid lines drawn around each inventory/equipment cell.
const CELL_GRID_COLOR: Color = Color::RGBA(80, 80, 100, 180);

/// Bitmap font index (yellow, sprite 701).
const UI_FONT: usize = 1;

/// Maps the 12 equipment grid positions (row-major, 2 cols × 6 rows) to
/// `WN_*` wear-slot indices.  Matches the original C `wntab[]` order.
/// TODO: Refactor this to put this logic all in one place.
const EQUIP_WNTAB: [usize; 12] = [
    WN_HEAD, WN_CLOAK, WN_BODY, WN_ARMS, WN_NECK, WN_BELT, WN_RHAND, WN_LHAND, WN_LRING, WN_RRING,
    WN_LEGS, WN_FEET,
];

/// Human-readable labels drawn inside empty equipment cells, indexed the same
/// as `EQUIP_WNTAB`.
///
const EQUIP_LABELS: [&str; 12] = [
    "Head", "Cloak", "Body", "Arms", "Neck", "Belt", "Weapon", "Shield", "Ring", "Ring", "Legs",
    "Feet",
];

// ---------------------------------------------------------------------------
// Data snapshot
// ---------------------------------------------------------------------------

/// Immutable per-frame snapshot of inventory/equipment state, pushed into
/// the panel by `GameScene` each render cycle.
pub struct InventoryPanelData {
    /// Inventory item sprite IDs (40 slots).
    pub items: [i32; 40],
    /// Inventory item placement flags.
    pub items_p: [i32; 40],
    /// Equipped item sprite IDs (indices 0-11 used, 20 reserved).
    pub worn: [i32; 20],
    /// Equipped item placement flags.
    pub worn_p: [i32; 20],
    /// Currently carried/held item sprite ID (0 = none).
    pub citem: i32,
    /// Placement bitmask of the carried item.
    pub citem_p: i32,
    /// Total gold (gold = val/100, silver = val%100).
    pub gold: i32,
    /// Currently selected/targeted character (0 = self).
    pub selected_char: u16,
}

// ---------------------------------------------------------------------------
// Panel struct
// ---------------------------------------------------------------------------

/// The money / inventory / equipment HUD panel.
///
/// Toggleable via the HUD button bar. When visible, draws two side-by-side
/// grids: a scrollable inventory backpack (left) and a labeled equipment
/// grid (right). Consumes clicks and scroll-wheel events inside its bounds
/// to prevent them from passing through to the game world.
pub struct InventoryPanel {
    bounds: Bounds,
    bg_color: Color,
    border_color: Color,
    visible: bool,
    data: Option<InventoryPanelData>,
    inv_scroll: usize,
    mouse_x: i32,
    mouse_y: i32,
    actions: Vec<WidgetAction>,
    /// Draggable title bar with pin and close buttons.
    title_bar: TitleBar,
}

impl InventoryPanel {
    /// Creates a new inventory panel.
    ///
    /// # Arguments
    ///
    /// * `bounds` - Position and size.
    /// * `bg_color` - Semi-transparent background color.
    ///
    /// # Returns
    ///
    /// A new `InventoryPanel`, initially hidden.
    pub fn new(bounds: Bounds, bg_color: Color) -> Self {
        let title_bar = TitleBar::new("Inventory", bounds.x, bounds.y, bounds.width);
        Self {
            bounds,
            bg_color,
            border_color: Color::RGBA(120, 120, 140, 200),
            visible: false,
            data: None,
            inv_scroll: 0,
            mouse_x: 0,
            mouse_y: 0,
            actions: Vec::new(),
            title_bar,
        }
    }

    /// Push a fresh data snapshot for this frame.
    ///
    /// # Arguments
    ///
    /// * `data` - Current inventory/equipment state from `PlayerState`.
    pub fn update_data(&mut self, data: InventoryPanelData) {
        self.data = Some(data);
    }

    /// Toggles the panel's visibility.
    pub fn toggle(&mut self) {
        self.visible = !self.visible;
    }

    /// Returns whether the panel is currently visible.
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    // -----------------------------------------------------------------------
    // Layout helpers
    // -----------------------------------------------------------------------

    /// Top-left corner of the inventory grid (absolute pixel coords).
    fn inv_origin(&self) -> (i32, i32) {
        (
            self.bounds.x + INV_GRID_PAD_X,
            self.bounds.y + INV_GRID_PAD_Y,
        )
    }

    /// Top-left corner of the equipment grid (absolute pixel coords).
    fn equip_origin(&self) -> (i32, i32) {
        let (ix, iy) = self.inv_origin();
        (ix + 2 * CELL + GRID_GAP, iy)
    }

    /// Pixel rect of the inventory scroll track.
    fn scroll_track_rect(&self) -> sdl2::rect::Rect {
        let (ix, iy) = self.inv_origin();
        let x = ix + 2 * CELL + 4;
        let h = (INV_VISIBLE_ROWS as i32) * CELL;
        sdl2::rect::Rect::new(x, iy, SCROLL_TRACK_W, h as u32)
    }

    /// Returns which inventory slot index the mouse is hovering, if any.
    fn hovered_inv_slot(&self) -> Option<usize> {
        let (ox, oy) = self.inv_origin();
        let mx = self.mouse_x - ox;
        let my = self.mouse_y - oy;
        if mx < 0 || my < 0 {
            return None;
        }
        let col = (mx / CELL) as usize;
        let row = (my / CELL) as usize;
        if col < 2
            && row < INV_VISIBLE_ROWS
            && mx < 2 * CELL
            && my < (INV_VISIBLE_ROWS as i32) * CELL
        {
            let idx = self.inv_scroll + row * 2 + col;
            if idx < INV_TOTAL_SLOTS {
                Some(idx)
            } else {
                None
            }
        } else {
            None
        }
    }

    /// Returns which equipment grid position (0..11) the mouse is hovering.
    ///
    /// Accounts for `EQUIP_COL_GAP` between the two columns: clicks in the
    /// gap region return `None`.
    fn hovered_equip_pos(&self) -> Option<usize> {
        let (ox, oy) = self.equip_origin();
        let mx = self.mouse_x - ox;
        let my = self.mouse_y - oy;
        if mx < 0 || my < 0 {
            return None;
        }
        let col = if mx < CELL {
            0usize
        } else if (CELL + EQUIP_COL_GAP..2 * CELL + EQUIP_COL_GAP).contains(&mx) {
            1usize
        } else {
            return None;
        };
        let row = (my / CELL) as usize;
        if row < EQUIP_ROWS && my < (EQUIP_ROWS as i32) * CELL {
            Some(row * 2 + col)
        } else {
            None
        }
    }

    /// Returns `true` when the tracked mouse position is within the panel's bounds.
    ///
    /// Used by the helper-text renderer to suppress world-label logic while
    /// the cursor is anywhere over the panel.
    pub fn is_cursor_over_panel(&self) -> bool {
        self.bounds.contains_point(self.mouse_x, self.mouse_y)
    }

    /// Returns the context-sensitive helper text label for whatever slot the
    /// cursor is currently over, or `None` when no label should be shown.
    ///
    /// **Backpack slots:**
    /// - Shift not held, non-empty slot --> `"USE"`
    /// - Shift held, non-empty slot, hand empty --> `"PICK UP"`
    /// - Shift held, non-empty slot, hand has item --> `"SWAP"`
    /// - Empty slot, hand has item --> `"DROP"`
    /// - Empty slot, hand empty --> `None`
    ///
    /// **Equipment slots:**
    /// - Shift not held, slot occupied --> `"USE"`
    /// - Shift held, hand has item, slot occupied --> `"SWAP"`
    /// - Shift held, hand has item, slot empty --> `"EQUIP"`
    /// - Otherwise --> `None`
    ///
    /// # Arguments
    ///
    /// * `shift` - `true` when the Shift modifier key is currently held.
    ///
    /// # Returns
    ///
    /// * `Some(&'static str)` with the action label, or `None`.
    pub fn hovered_label(&self, shift: bool) -> Option<&'static str> {
        let data = self.data.as_ref()?;

        // ── Backpack grid ──────────────────────────────────────────────────
        if let Some(idx) = self.hovered_inv_slot() {
            if data.items[idx] <= 0 {
                return if data.citem > 0 { Some("DROP") } else { None };
            }
            return Some(if shift {
                if data.citem > 0 { "SWAP" } else { "PICK UP" }
            } else {
                "USE"
            });
        }

        // ── Equipment grid ─────────────────────────────────────────────────
        if let Some(pos) = self.hovered_equip_pos() {
            let wn_slot = EQUIP_WNTAB[pos];
            let slot_has_item = data.worn[wn_slot] > 0;
            if shift && data.citem > 0 {
                return Some(if slot_has_item { "SWAP" } else { "EQUIP" });
            }
            if !shift && slot_has_item {
                return Some("USE");
            }
        }

        None
    }

    // -----------------------------------------------------------------------
    // Slot-acceptance / blocking helpers
    // -----------------------------------------------------------------------

    /// Returns `true` if a carried item with placement flags `citem_p` can
    /// be placed into the given `WN_*` wear slot.
    fn slot_accepts(slot: usize, citem_p: u16) -> bool {
        match slot {
            WN_HEAD => (citem_p & PL_HEAD) != 0,
            WN_NECK => (citem_p & PL_NECK) != 0,
            WN_BODY => (citem_p & PL_BODY) != 0,
            WN_ARMS => (citem_p & PL_ARMS) != 0,
            WN_BELT => (citem_p & PL_BELT) != 0,
            WN_LEGS => (citem_p & PL_LEGS) != 0,
            WN_FEET => (citem_p & PL_FEET) != 0,
            WN_RHAND => (citem_p & PL_WEAPON) != 0,
            WN_LHAND => (citem_p & PL_SHIELD) != 0,
            WN_CLOAK => (citem_p & PL_CLOAK) != 0,
            WN_LRING | WN_RRING => (citem_p & PL_RING) != 0,
            _ => true,
        }
    }

    /// Compute which of the 20 wear slots are blocked given carried-item
    /// placement flags and the currently-equipped right-hand placement.
    fn compute_blocked(citem_p: u16, worn_p: &[i32; 20]) -> [bool; 20] {
        let mut blocked = [false; 20];
        for slot in 0..20 {
            blocked[slot] = !Self::slot_accepts(slot, citem_p);
        }
        if (worn_p[WN_RHAND] as u16 & PL_TWOHAND) != 0 {
            blocked[WN_LHAND] = true;
        }
        blocked
    }

    // -----------------------------------------------------------------------
    // Rendering helpers
    // -----------------------------------------------------------------------

    /// Draw a single item sprite with an optional additive hover highlight.
    fn draw_item(
        ctx: &mut RenderContext,
        sprite_id: i32,
        x: i32,
        y: i32,
        hovered: bool,
    ) -> Result<(), String> {
        if sprite_id <= 0 {
            return Ok(());
        }
        let tex = ctx.gfx.get_texture(sprite_id as usize);
        let q = tex.query();
        ctx.canvas.copy(
            tex,
            None,
            Some(sdl2::rect::Rect::new(x, y, q.width, q.height)),
        )?;

        if hovered {
            tex.set_blend_mode(sdl2::render::BlendMode::Add);
            tex.set_alpha_mod(96);
            let result = ctx.canvas.copy(
                tex,
                None,
                Some(sdl2::rect::Rect::new(x, y, q.width, q.height)),
            );
            tex.set_alpha_mod(255);
            tex.set_blend_mode(sdl2::render::BlendMode::Blend);
            result?;
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Widget trait impl
// ---------------------------------------------------------------------------

impl Widget for InventoryPanel {
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
            self.actions
                .push(WidgetAction::TogglePanel(HudPanel::Inventory));
            return EventResponse::Consumed;
        }
        if tb_resp == EventResponse::Consumed {
            return EventResponse::Consumed;
        }

        match event {
            UiEvent::MouseMove { x, y } => {
                self.mouse_x = *x;
                self.mouse_y = *y;
                // Don't consume — other widgets and GameScene also track mouse.
                EventResponse::Ignored
            }
            UiEvent::MouseWheel { x, y, delta } => {
                if !self.bounds.contains_point(*x, *y) {
                    return EventResponse::Ignored;
                }
                // Scroll inventory by one row (2 slots) per wheel tick.
                if *delta < 0 {
                    self.inv_scroll = (self.inv_scroll + 2).min(INV_SCROLL_MAX);
                } else if *delta > 0 {
                    self.inv_scroll = self.inv_scroll.saturating_sub(2);
                }
                // Keep scroll aligned to full rows.
                self.inv_scroll &= !1usize;
                EventResponse::Consumed
            }
            UiEvent::MouseClick {
                x,
                y,
                button,
                modifiers,
            } => {
                if !self.bounds.contains_point(*x, *y) {
                    return EventResponse::Ignored;
                }
                // Sync stored coords so hovered_inv_slot / hovered_equip_pos
                // use the click position even if no MouseMove preceded it.
                self.mouse_x = *x;
                self.mouse_y = *y;

                let data = match self.data.as_ref() {
                    Some(d) => d,
                    None => return EventResponse::Consumed,
                };
                let selected_char = data.selected_char as u32;

                // Check inventory grid hit.
                if let Some(idx) = self.hovered_inv_slot() {
                    match button {
                        MouseButton::Right => {
                            self.actions.push(WidgetAction::InvLookAction {
                                a: idx as u32,
                                b: 0,
                                c: selected_char,
                            });
                        }
                        MouseButton::Left => {
                            let a = if modifiers.shift { 0u32 } else { 6u32 };
                            self.actions.push(WidgetAction::InvAction {
                                a,
                                b: idx as u32,
                                selected_char,
                            });
                        }
                        _ => {}
                    }
                    return EventResponse::Consumed;
                }

                // Check equipment grid hit.
                if let Some(pos) = self.hovered_equip_pos() {
                    let wn_slot = EQUIP_WNTAB[pos];
                    match button {
                        MouseButton::Right => {
                            self.actions.push(WidgetAction::InvAction {
                                a: 7,
                                b: wn_slot as u32,
                                selected_char,
                            });
                        }
                        MouseButton::Left => {
                            let a = if modifiers.shift { 1u32 } else { 5u32 };
                            self.actions.push(WidgetAction::InvAction {
                                a,
                                b: wn_slot as u32,
                                selected_char,
                            });
                        }
                        _ => {}
                    }
                    return EventResponse::Consumed;
                }

                EventResponse::Consumed
            }
            _ => EventResponse::Ignored,
        }
    }

    fn take_actions(&mut self) -> Vec<WidgetAction> {
        std::mem::take(&mut self.actions)
    }

    fn render(&mut self, ctx: &mut RenderContext<'_, '_>) -> Result<(), String> {
        if !self.visible {
            return Ok(());
        }
        let data = match self.data.as_ref() {
            Some(d) => d,
            None => return Ok(()),
        };

        // --- Background + border ---
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

        // --- Title bar (draggable, with pin/close) ---
        self.title_bar.render(ctx)?;

        // --- Money line ---
        let money_cx = self.bounds.x + self.bounds.width as i32 / 2;
        let money_y = self.bounds.y + TITLE_BAR_H + 2;
        let gold = data.gold / 100;
        let silver = data.gold % 100;
        let money_text = format!("{}G {}S", gold, silver);
        font_cache::draw_text(
            ctx.canvas,
            ctx.gfx,
            UI_FONT,
            &money_text,
            money_cx,
            money_y,
            font_cache::TextStyle::centered(),
        )?;

        // --- Inventory grid (left, scrollable) ---
        let (inv_x, inv_y) = self.inv_origin();
        let hovered_inv = self.hovered_inv_slot();

        // Draw inventory cell grid.
        ctx.canvas.set_draw_color(CELL_GRID_COLOR);
        for n in 0..(INV_VISIBLE_ROWS * 2) {
            let col = (n % 2) as i32;
            let row = (n / 2) as i32;
            ctx.canvas.draw_rect(sdl2::rect::Rect::new(
                inv_x + col * CELL,
                inv_y + row * CELL,
                CELL as u32,
                CELL as u32,
            ))?;
        }

        for n in 0..(INV_VISIBLE_ROWS * 2) {
            let idx = self.inv_scroll + n;
            if idx >= INV_TOTAL_SLOTS {
                break;
            }
            let sprite = data.items[idx];
            let col = (n % 2) as i32;
            let row = (n / 2) as i32;
            let x = inv_x + col * CELL;
            let y = inv_y + row * CELL;
            let hovered = hovered_inv == Some(idx);
            Self::draw_item(ctx, sprite, x, y, hovered)?;
        }

        // --- Inventory scrollbar ---
        let track = self.scroll_track_rect();
        ctx.canvas.set_draw_color(SCROLL_TRACK_COLOR);
        ctx.canvas.fill_rect(track)?;

        if INV_SCROLL_MAX > 0 {
            let track_h = track.height() as i32 - SCROLL_KNOB_H as i32;
            let knob_y = track.y() + (self.inv_scroll as i32 * track_h) / (INV_SCROLL_MAX as i32);
            ctx.canvas.set_draw_color(SCROLL_KNOB_COLOR);
            ctx.canvas.fill_rect(sdl2::rect::Rect::new(
                track.x(),
                knob_y,
                SCROLL_TRACK_W,
                SCROLL_KNOB_H,
            ))?;
        }

        // --- Equipment grid (right, with slot labels) ---
        let (eq_x, eq_y) = self.equip_origin();
        let hovered_eq = self.hovered_equip_pos();

        // Draw equipment cell grid.
        ctx.canvas.set_draw_color(CELL_GRID_COLOR);
        for n in 0..12usize {
            let col = (n % 2) as i32;
            let row = (n / 2) as i32;
            ctx.canvas.draw_rect(sdl2::rect::Rect::new(
                eq_x + col * (CELL + EQUIP_COL_GAP),
                eq_y + row * CELL,
                CELL as u32,
                CELL as u32,
            ))?;
        }

        // Pre-compute blocked slots if carrying an item.
        let blocked = if data.citem > 0 {
            Some(Self::compute_blocked(data.citem_p as u16, &data.worn_p))
        } else {
            None
        };

        for n in 0..12usize {
            let worn_index = EQUIP_WNTAB[n];
            let sprite = data.worn[worn_index];
            let col = (n % 2) as i32;
            let row = (n / 2) as i32;
            let x = eq_x + col * (CELL + EQUIP_COL_GAP);
            let y = eq_y + row * CELL;

            if sprite > 0 {
                let hovered = hovered_eq == Some(n);
                Self::draw_item(ctx, sprite, x, y, hovered)?;
            } else {
                // Draw a label inside the empty slot.
                let cx = x + CELL / 2;
                let cy = y + CELL / 2 - 5;
                ctx.canvas.set_draw_color(SLOT_LABEL_COLOR);
                font_cache::draw_text(
                    ctx.canvas,
                    ctx.gfx,
                    UI_FONT,
                    EQUIP_LABELS[n],
                    cx,
                    cy,
                    font_cache::TextStyle::centered(),
                )?;
            }

            // Blocked-slot overlay (sprite 4) when carrying an incompatible item.
            if let Some(ref bl) = blocked {
                if bl[worn_index] {
                    let tex = ctx.gfx.get_texture(4);
                    let q = tex.query();
                    ctx.canvas.copy(
                        tex,
                        None,
                        Some(sdl2::rect::Rect::new(x, y, q.width, q.height)),
                    )?;
                }
            }
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
    use crate::ui::widget::{KeyModifiers, MouseButton};

    /// Helper to create a default data snapshot for testing.
    fn test_data() -> InventoryPanelData {
        InventoryPanelData {
            items: [0; 40],
            items_p: [0; 40],
            worn: [0; 20],
            worn_p: [0; 20],
            citem: 0,
            citem_p: 0,
            gold: 0,
            selected_char: 0,
        }
    }

    #[test]
    fn starts_hidden() {
        let panel = InventoryPanel::new(Bounds::new(0, 0, 100, 100), Color::RGBA(0, 0, 0, 180));
        assert!(!panel.is_visible());
    }

    #[test]
    fn toggle_flips_visibility() {
        let mut panel = InventoryPanel::new(Bounds::new(0, 0, 100, 100), Color::RGBA(0, 0, 0, 180));
        panel.toggle();
        assert!(panel.is_visible());
        panel.toggle();
        assert!(!panel.is_visible());
    }

    #[test]
    fn hidden_panel_ignores_clicks() {
        let mut panel =
            InventoryPanel::new(Bounds::new(10, 10, 100, 100), Color::RGBA(0, 0, 0, 180));
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
        let mut panel =
            InventoryPanel::new(Bounds::new(10, 10, 100, 100), Color::RGBA(0, 0, 0, 180));
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
        let mut panel =
            InventoryPanel::new(Bounds::new(10, 10, 100, 100), Color::RGBA(0, 0, 0, 180));
        panel.toggle();
        let resp = panel.handle_event(&UiEvent::MouseClick {
            x: 0,
            y: 0,
            button: MouseButton::Left,
            modifiers: KeyModifiers::default(),
        });
        assert_eq!(resp, EventResponse::Ignored);
    }

    #[test]
    fn scroll_clamps_to_max() {
        let mut panel = InventoryPanel::new(Bounds::new(0, 0, 400, 300), Color::RGBA(0, 0, 0, 180));
        panel.toggle();
        // Scroll down many times — must not exceed INV_SCROLL_MAX.
        for _ in 0..100 {
            panel.handle_event(&UiEvent::MouseWheel {
                x: 50,
                y: 50,
                delta: -1,
            });
        }
        assert!(panel.inv_scroll <= INV_SCROLL_MAX);
        assert_eq!(panel.inv_scroll % 2, 0);
    }

    #[test]
    fn scroll_clamps_to_zero() {
        let mut panel = InventoryPanel::new(Bounds::new(0, 0, 400, 300), Color::RGBA(0, 0, 0, 180));
        panel.toggle();
        // Scroll up from zero — must stay at 0.
        panel.handle_event(&UiEvent::MouseWheel {
            x: 50,
            y: 50,
            delta: 1,
        });
        assert_eq!(panel.inv_scroll, 0);
    }

    #[test]
    fn hidden_panel_ignores_wheel() {
        let mut panel = InventoryPanel::new(Bounds::new(0, 0, 400, 300), Color::RGBA(0, 0, 0, 180));
        let resp = panel.handle_event(&UiEvent::MouseWheel {
            x: 50,
            y: 50,
            delta: -1,
        });
        assert_eq!(resp, EventResponse::Ignored);
        assert_eq!(panel.inv_scroll, 0);
    }

    #[test]
    fn slot_accepts_head() {
        assert!(InventoryPanel::slot_accepts(WN_HEAD, PL_HEAD));
        assert!(!InventoryPanel::slot_accepts(WN_HEAD, PL_NECK));
    }

    #[test]
    fn slot_accepts_weapon_shield() {
        assert!(InventoryPanel::slot_accepts(WN_RHAND, PL_WEAPON));
        assert!(!InventoryPanel::slot_accepts(WN_RHAND, PL_SHIELD));
        assert!(InventoryPanel::slot_accepts(WN_LHAND, PL_SHIELD));
        assert!(!InventoryPanel::slot_accepts(WN_LHAND, PL_WEAPON));
    }

    #[test]
    fn slot_accepts_ring_both_hands() {
        assert!(InventoryPanel::slot_accepts(WN_LRING, PL_RING));
        assert!(InventoryPanel::slot_accepts(WN_RRING, PL_RING));
        assert!(!InventoryPanel::slot_accepts(WN_LRING, PL_HEAD));
    }

    #[test]
    fn twohand_blocks_lhand() {
        let mut worn_p = [0i32; 20];
        worn_p[WN_RHAND] = PL_TWOHAND as i32;
        let blocked = InventoryPanel::compute_blocked(PL_WEAPON, &worn_p);
        assert!(blocked[WN_LHAND]);
    }

    #[test]
    fn blocked_array_size() {
        let worn_p = [0i32; 20];
        let blocked = InventoryPanel::compute_blocked(PL_HEAD, &worn_p);
        assert_eq!(blocked.len(), 20);
        // Head item should only be accepted in HEAD slot.
        assert!(!blocked[WN_HEAD]);
        assert!(blocked[WN_NECK]);
        assert!(blocked[WN_BODY]);
    }

    #[test]
    fn update_data_stores_snapshot() {
        let mut panel = InventoryPanel::new(Bounds::new(0, 0, 400, 300), Color::RGBA(0, 0, 0, 180));
        assert!(panel.data.is_none());
        panel.update_data(test_data());
        assert!(panel.data.is_some());
    }

    #[test]
    fn hovered_label_drop_on_empty_slot_with_item_in_hand() {
        // Panel bounds chosen so that the first inventory slot falls under
        // the simulated cursor position used below.
        let mut panel = InventoryPanel::new(Bounds::new(0, 0, 400, 300), Color::RGBA(0, 0, 0, 180));
        panel.toggle();
        let mut data = test_data();
        // Slot 0 is empty; player is carrying an item.
        data.citem = 42;
        panel.update_data(data);
        // Move cursor over the first inventory slot (INV_GRID_PAD_X + 1, INV_GRID_PAD_Y + 1).
        let mx = INV_GRID_PAD_X + 1;
        let my = INV_GRID_PAD_Y + 1;
        panel.handle_event(&UiEvent::MouseMove { x: mx, y: my });
        assert_eq!(panel.hovered_label(false), Some("DROP"));
        assert_eq!(panel.hovered_label(true), Some("DROP"));
    }

    #[test]
    fn hovered_label_none_on_empty_slot_without_item_in_hand() {
        let mut panel = InventoryPanel::new(Bounds::new(0, 0, 400, 300), Color::RGBA(0, 0, 0, 180));
        panel.toggle();
        panel.update_data(test_data()); // citem == 0, slot 0 empty
        let mx = INV_GRID_PAD_X + 1;
        let my = INV_GRID_PAD_Y + 1;
        panel.handle_event(&UiEvent::MouseMove { x: mx, y: my });
        assert_eq!(panel.hovered_label(false), None);
        assert_eq!(panel.hovered_label(true), None);
    }
}
