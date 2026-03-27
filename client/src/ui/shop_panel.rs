//! Shop / depot / grave overlay panel.
//!
//! Renders an 8-column × 8-row grid of up to 62 item slots (shops, depots,
//! and graves all use the same layout). Shows sell/buy price labels at the
//! bottom. Clicking outside the panel while it is visible closes it.

use sdl2::pixels::Color;
use sdl2::render::BlendMode;

use super::RenderContext;
use super::widget::{Bounds, EventResponse, MouseButton, UiEvent, Widget, WidgetAction};
use crate::font_cache;

// ---------------------------------------------------------------------------
// Layout constants
// ---------------------------------------------------------------------------

/// Size of each item cell (sprite slot) in pixels.
const CELL: i32 = 35;

/// Number of item columns in the grid.
const GRID_COLS: usize = 8;

/// Number of item rows in the grid.
const GRID_ROWS: usize = 8;

/// Maximum number of shop item slots.
const SHOP_SLOTS: usize = 62;

/// Inner padding from the panel edge to the item grid.
const PAD_X: i32 = 8;

/// Vertical offset from the panel top to the first grid row (leaves room for
/// a small title / header area).
const PAD_TOP: i32 = 20;

/// Extra height below the grid for the sell/buy price text.
const PRICE_AREA_H: i32 = 24;

/// Panel width: 8 cells + left/right padding.
pub const SHOP_PANEL_W: u32 = (GRID_COLS as i32 * CELL + PAD_X * 2) as u32;

/// Panel height: header + 8 rows + price area + bottom padding.
pub const SHOP_PANEL_H: u32 = (PAD_TOP + GRID_ROWS as i32 * CELL + PRICE_AREA_H + 4) as u32;

/// Border color matching the other HUD panels.
const BORDER_COLOR: Color = Color::RGBA(120, 120, 140, 200);

/// Bitmap font index (yellow, sprite 701).
const UI_FONT: usize = 1;

// ---------------------------------------------------------------------------
// Data snapshot
// ---------------------------------------------------------------------------

/// A lightweight snapshot of shop data, copied from `PlayerState` each frame.
///
/// Decouples the widget from the borrow of `PlayerState` so that rendering
/// and event handling can proceed without lifetime issues.
#[derive(Clone)]
pub struct ShopPanelData {
    /// Item sprite IDs for each of the 62 shop slots (0 = empty).
    pub items: [u16; SHOP_SLOTS],
    /// Sell prices for each slot (0 = not for sale).
    pub prices: [u32; SHOP_SLOTS],
    /// The price the server would charge for the player's currently carried
    /// item (buy price). 0 when no item is carried or the item cannot be sold.
    pub pl_price: u32,
    /// The NPC/shop number used in `CmdShop` packets.
    pub shop_nr: u16,
    /// The player's currently carried item sprite ID (0 = none).
    pub citem: i32,
    /// Whether the shop overlay should be visible.
    pub visible: bool,
    /// `true` when this overlay represents a corpse/grave rather than a merchant.
    /// Controls the title text displayed in the panel header.
    pub is_grave: bool,
}

// ---------------------------------------------------------------------------
// ShopPanel widget
// ---------------------------------------------------------------------------

/// Shop/depot/grave overlay widget.
///
/// Displays an 8×8 grid of item slots with hover highlighting and
/// sell/buy price labels. Any click outside the panel while it is
/// visible produces a [`WidgetAction::CloseShop`] action.
pub struct ShopPanel {
    bounds: Bounds,
    bg_color: Color,
    data: Option<ShopPanelData>,
    mouse_x: i32,
    mouse_y: i32,
    actions: Vec<WidgetAction>,
}

impl ShopPanel {
    /// Create a new shop panel at the given position.
    ///
    /// # Arguments
    ///
    /// * `bounds` - The panel's bounding rectangle (should be centered on screen).
    /// * `bg_color` - Semi-transparent background fill color.
    ///
    /// # Returns
    ///
    /// A new `ShopPanel` in the hidden state.
    pub fn new(bounds: Bounds, bg_color: Color) -> Self {
        Self {
            bounds,
            bg_color,
            data: None,
            mouse_x: 0,
            mouse_y: 0,
            actions: Vec::new(),
        }
    }

    /// Push a new data snapshot into the widget.
    ///
    /// Called each frame from `render_world` so the panel always has
    /// up-to-date shop contents and visibility.
    ///
    /// # Arguments
    ///
    /// * `data` - The latest shop state from `PlayerState`.
    pub fn update_data(&mut self, data: ShopPanelData) {
        self.data = Some(data);
    }

    /// Returns whether the panel is currently visible.
    pub fn is_visible(&self) -> bool {
        self.data.as_ref().map_or(false, |d| d.visible)
    }

    /// Toggle the panel's visibility.
    ///
    /// If no data snapshot has been set yet, a default (empty) snapshot is
    /// created with `visible = true`.
    pub fn toggle(&mut self) {
        match &mut self.data {
            Some(d) => d.visible = !d.visible,
            None => {
                self.data = Some(ShopPanelData {
                    items: [0; SHOP_SLOTS],
                    prices: [0; SHOP_SLOTS],
                    pl_price: 0,
                    shop_nr: 0,
                    citem: 0,
                    visible: true,
                    is_grave: false,
                });
            }
        }
    }

    // ── Hit-testing helpers ─────────────────────────────────────────────

    /// Returns the context-sensitive helper text label for the item slot
    /// currently under the cursor, or `None` if no filled slot is hovered.
    ///
    /// Returns `"TAKE"` for grave/corpse overlays and `"BUY"` for merchant
    /// shops. Used by the game scene's helper-text renderer so that the
    /// cursor label updates correctly while the panel is open.
    ///
    /// # Arguments
    ///
    /// * `is_grave` - `true` when this overlay represents a corpse/grave.
    ///
    /// # Returns
    ///
    /// * `Some("TAKE")` or `Some("BUY")` when a non-empty slot is hovered.
    /// * `None` when the cursor is over an empty slot or outside the grid.
    pub fn hovered_item_label(&self, is_grave: bool) -> Option<&'static str> {
        if !self.is_visible() {
            return None;
        }
        let data = self.data.as_ref()?;
        let idx = self.hovered_slot()?;
        if data.items[idx] == 0 {
            return None;
        }
        Some(if is_grave { "TAKE" } else { "BUY" })
    }

    /// Returns the grid slot index (0–61) under the current mouse position,
    /// or `None` if the mouse is outside the grid or beyond slot 61.
    fn hovered_slot(&self) -> Option<usize> {
        let grid_x = self.bounds.x + PAD_X;
        let grid_y = self.bounds.y + PAD_TOP;

        let mx = self.mouse_x - grid_x;
        let my = self.mouse_y - grid_y;

        if mx < 0 || my < 0 {
            return None;
        }

        let col = mx / CELL;
        let row = my / CELL;

        if col < 0 || col >= GRID_COLS as i32 || row < 0 || row >= GRID_ROWS as i32 {
            return None;
        }

        let idx = row as usize * GRID_COLS + col as usize;
        if idx < SHOP_SLOTS { Some(idx) } else { None }
    }
}

impl Widget for ShopPanel {
    /// Returns the panel's bounding rectangle.
    fn bounds(&self) -> &Bounds {
        &self.bounds
    }

    /// Moves the panel's top-left corner.
    ///
    /// # Arguments
    ///
    /// * `x` - New X position.
    /// * `y` - New Y position.
    fn set_position(&mut self, x: i32, y: i32) {
        self.bounds.x = x;
        self.bounds.y = y;
    }

    /// Process an input event.
    ///
    /// When the shop is visible:
    /// - Clicks inside the grid produce [`WidgetAction::ShopAction`].
    /// - Clicks anywhere outside the panel produce [`WidgetAction::CloseShop`].
    /// - Both return [`EventResponse::Consumed`].
    ///
    /// When the shop is hidden all events pass through as `Ignored`.
    ///
    /// # Arguments
    ///
    /// * `event` - The UI event to process.
    ///
    /// # Returns
    ///
    /// `EventResponse::Consumed` if the event was handled, `Ignored` otherwise.
    fn handle_event(&mut self, event: &UiEvent) -> EventResponse {
        if !self.is_visible() {
            return EventResponse::Ignored;
        }

        match event {
            UiEvent::MouseMove { x, y } => {
                self.mouse_x = *x;
                self.mouse_y = *y;
                // Don't consume moves — let other widgets track the cursor too.
                EventResponse::Ignored
            }
            UiEvent::MouseClick { x, y, button, .. } => {
                self.mouse_x = *x;
                self.mouse_y = *y;

                // Click outside the panel --> close shop.
                if !self.bounds.contains_point(*x, *y) {
                    self.actions.push(WidgetAction::CloseShop);
                    return EventResponse::Consumed;
                }

                // Hit-test the item grid.
                let data = match self.data.as_ref() {
                    Some(d) => d,
                    None => return EventResponse::Consumed,
                };

                if let Some(idx) = self.hovered_slot() {
                    let shop_nr = data.shop_nr as i16;
                    match button {
                        MouseButton::Left => {
                            self.actions.push(WidgetAction::ShopAction {
                                shop_nr,
                                action: idx as i32,
                            });
                        }
                        MouseButton::Right => {
                            self.actions.push(WidgetAction::ShopAction {
                                shop_nr,
                                action: (idx + SHOP_SLOTS) as i32,
                            });
                        }
                        _ => {}
                    }
                }

                EventResponse::Consumed
            }
            UiEvent::MouseWheel { .. } => {
                // Consume wheel events over the panel to prevent world scroll.
                if self.bounds.contains_point(self.mouse_x, self.mouse_y) {
                    EventResponse::Consumed
                } else {
                    EventResponse::Ignored
                }
            }
            _ => EventResponse::Ignored,
        }
    }

    /// Draw the shop overlay.
    ///
    /// Renders a semi-transparent background with a border, an 8×8 grid of
    /// item sprites with additive hover highlights, and sell/buy price
    /// labels at the bottom.
    ///
    /// # Arguments
    ///
    /// * `ctx` - Mutable render context (canvas + graphics cache).
    ///
    /// # Returns
    ///
    /// `Ok(())` on success, or an SDL2 error string.
    fn render(&mut self, ctx: &mut RenderContext<'_, '_>) -> Result<(), String> {
        let data = match self.data.as_ref() {
            Some(d) if d.visible => d,
            _ => return Ok(()),
        };

        let rect = sdl2::rect::Rect::new(
            self.bounds.x,
            self.bounds.y,
            self.bounds.width,
            self.bounds.height,
        );

        // Background fill.
        ctx.canvas.set_blend_mode(BlendMode::Blend);
        ctx.canvas.set_draw_color(self.bg_color);
        ctx.canvas.fill_rect(rect)?;

        // Border.
        ctx.canvas.set_draw_color(BORDER_COLOR);
        ctx.canvas.draw_rect(rect)?;

        // Title.
        let title = if data.is_grave {
            "Grave".to_string()
        } else {
            "Shop".to_string()
        };
        font_cache::draw_text(
            ctx.canvas,
            ctx.gfx,
            UI_FONT,
            &title,
            self.bounds.x + PAD_X,
            self.bounds.y + 4,
            font_cache::TextStyle::PLAIN,
        )?;

        let grid_x = self.bounds.x + PAD_X;
        let grid_y = self.bounds.y + PAD_TOP;
        let hovered = self.hovered_slot();

        // Draw item grid.
        for i in 0..SHOP_SLOTS {
            let item = data.items[i];
            if item == 0 {
                continue;
            }

            let col = (i % GRID_COLS) as i32;
            let row = (i / GRID_COLS) as i32;
            let x = grid_x + col * CELL + 2;
            let y = grid_y + row * CELL + 1;
            let is_hovered = hovered == Some(i);

            // Draw item sprite.
            let texture = ctx.gfx.get_texture(item as usize);
            let q = texture.query();
            ctx.canvas.copy(
                texture,
                None,
                Some(sdl2::rect::Rect::new(x, y, q.width, q.height)),
            )?;

            // Additive hover highlight.
            if is_hovered {
                texture.set_blend_mode(BlendMode::Add);
                texture.set_alpha_mod(96);
                let result = ctx.canvas.copy(
                    texture,
                    None,
                    Some(sdl2::rect::Rect::new(x, y, q.width, q.height)),
                );
                texture.set_alpha_mod(255);
                texture.set_blend_mode(BlendMode::Blend);
                result?;
            }
        }

        // Sell price label (shown when hovering a slot that has a price).
        let price_y = grid_y + GRID_ROWS as i32 * CELL + 2;
        if let Some(idx) = hovered {
            let price = data.prices[idx];
            if price != 0 {
                let sell_text = format!("Sell: {}G {}S", price / 100, price % 100);
                font_cache::draw_text(
                    ctx.canvas,
                    ctx.gfx,
                    UI_FONT,
                    &sell_text,
                    grid_x,
                    price_y,
                    font_cache::TextStyle::PLAIN,
                )?;
            }
        }

        // Buy price label (shown when carrying an item the shop will accept).
        if data.citem > 0 && data.pl_price > 0 {
            let buy_text = format!("Buy:  {}G {}S", data.pl_price / 100, data.pl_price % 100);
            font_cache::draw_text(
                ctx.canvas,
                ctx.gfx,
                UI_FONT,
                &buy_text,
                grid_x,
                price_y + 10,
                font_cache::TextStyle::PLAIN,
            )?;
        }

        Ok(())
    }

    /// Drain any pending shop actions.
    ///
    /// # Returns
    ///
    /// A `Vec` of [`WidgetAction`]s produced since the last drain.
    fn take_actions(&mut self) -> Vec<WidgetAction> {
        std::mem::take(&mut self.actions)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_panel() -> ShopPanel {
        ShopPanel::new(
            Bounds::new(100, 100, SHOP_PANEL_W, SHOP_PANEL_H),
            Color::RGBA(10, 10, 30, 180),
        )
    }

    fn make_visible_data() -> ShopPanelData {
        let mut data = ShopPanelData {
            items: [0; SHOP_SLOTS],
            prices: [0; SHOP_SLOTS],
            pl_price: 0,
            shop_nr: 42,
            citem: 0,
            visible: true,
            is_grave: false,
        };
        data.items[0] = 100; // put an item in slot 0
        data.prices[0] = 500;
        data
    }

    #[test]
    fn hidden_panel_ignores_events() {
        let mut panel = make_panel();
        let click = UiEvent::MouseClick {
            x: 150,
            y: 150,
            button: MouseButton::Left,
            modifiers: super::super::widget::KeyModifiers {
                ctrl: false,
                shift: false,
                alt: false,
            },
        };
        assert_eq!(panel.handle_event(&click), EventResponse::Ignored);
        assert!(panel.take_actions().is_empty());
    }

    #[test]
    fn click_outside_closes_shop() {
        let mut panel = make_panel();
        panel.update_data(make_visible_data());

        // Click well outside the panel bounds.
        let click = UiEvent::MouseClick {
            x: 0,
            y: 0,
            button: MouseButton::Left,
            modifiers: super::super::widget::KeyModifiers {
                ctrl: false,
                shift: false,
                alt: false,
            },
        };
        assert_eq!(panel.handle_event(&click), EventResponse::Consumed);
        let actions = panel.take_actions();
        assert_eq!(actions.len(), 1);
        assert!(matches!(actions[0], WidgetAction::CloseShop));
    }

    #[test]
    fn left_click_grid_produces_shop_action() {
        let mut panel = make_panel();
        panel.update_data(make_visible_data());

        // Click on slot 0 (top-left of grid).
        let grid_x = 100 + PAD_X + 5;
        let grid_y = 100 + PAD_TOP + 5;
        let click = UiEvent::MouseClick {
            x: grid_x,
            y: grid_y,
            button: MouseButton::Left,
            modifiers: super::super::widget::KeyModifiers {
                ctrl: false,
                shift: false,
                alt: false,
            },
        };
        assert_eq!(panel.handle_event(&click), EventResponse::Consumed);
        let actions = panel.take_actions();
        assert_eq!(actions.len(), 1);
        match &actions[0] {
            WidgetAction::ShopAction { shop_nr, action } => {
                assert_eq!(*shop_nr, 42);
                assert_eq!(*action, 0);
            }
            other => panic!("Expected ShopAction, got {:?}", other),
        }
    }

    #[test]
    fn right_click_grid_produces_offset_action() {
        let mut panel = make_panel();
        panel.update_data(make_visible_data());

        // Click on slot 0 with right button.
        let grid_x = 100 + PAD_X + 5;
        let grid_y = 100 + PAD_TOP + 5;
        let click = UiEvent::MouseClick {
            x: grid_x,
            y: grid_y,
            button: MouseButton::Right,
            modifiers: super::super::widget::KeyModifiers {
                ctrl: false,
                shift: false,
                alt: false,
            },
        };
        assert_eq!(panel.handle_event(&click), EventResponse::Consumed);
        let actions = panel.take_actions();
        assert_eq!(actions.len(), 1);
        match &actions[0] {
            WidgetAction::ShopAction { shop_nr, action } => {
                assert_eq!(*shop_nr, 42);
                assert_eq!(*action, SHOP_SLOTS as i32); // 0 + 62
            }
            other => panic!("Expected ShopAction, got {:?}", other),
        }
    }

    #[test]
    fn hovered_slot_clamps_to_max() {
        let mut panel = make_panel();
        // Place mouse at the very last cell row (row 7, col 7 = index 63 > 61).
        // This slot should be None since index 63 >= SHOP_SLOTS.
        panel.mouse_x = 100 + PAD_X + 7 * CELL + 5;
        panel.mouse_y = 100 + PAD_TOP + 7 * CELL + 5;
        assert_eq!(panel.hovered_slot(), None);

        // Slot 61 (row 7, col 5) should be valid.
        panel.mouse_x = 100 + PAD_X + 5 * CELL + 5;
        panel.mouse_y = 100 + PAD_TOP + 7 * CELL + 5;
        assert_eq!(panel.hovered_slot(), Some(61));
    }
}
