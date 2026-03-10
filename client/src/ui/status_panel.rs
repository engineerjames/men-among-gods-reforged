//! Compact HUD panel showing rank sigil, HP/Endurance/Mana bars, and
//! weapon/armor values in the upper-left corner of the viewport.
//!
//! The sigil is always visible. Clicking it toggles the stat bars and
//! weapon/armor text on or off. Bars are shown by default.

use sdl2::pixels::Color;
use sdl2::render::BlendMode;

use crate::font_cache;
use crate::player_state::PlayerState;

use super::widget::{Bounds, EventResponse, UiEvent, Widget};
use super::RenderContext;

// ---------------------------------------------------------------------------
// Layout constants
// ---------------------------------------------------------------------------

/// Width of each stat bar in pixels.
const BAR_WIDTH: i32 = 120;
/// Height of each stat bar in pixels.
const BAR_HEIGHT: i32 = 12;
/// Vertical gap between consecutive bars.
const BAR_GAP: i32 = 3;
/// Horizontal gap between the sigil and the first bar.
const SIGIL_BAR_GAP: i32 = 6;
/// Padding inside the panel background.
const PANEL_PADDING: i32 = 4;
/// Assumed sigil sprite dimensions (square).
const SIGIL_SIZE: i32 = 32;

/// Bitmap font index used for value text (yellow font).
const FONT: usize = 1;

// Bar fill colors (current value).
const HP_FILL: Color = Color::RGB(180, 30, 30);
const END_FILL: Color = Color::RGB(200, 180, 40);
const MANA_FILL: Color = Color::RGB(40, 80, 200);

// Bar background colors (max capacity).
const HP_BG: Color = Color::RGB(60, 10, 10);
const END_BG: Color = Color::RGB(65, 58, 12);
const MANA_BG: Color = Color::RGB(12, 25, 65);

// ---------------------------------------------------------------------------
// Stat snapshot
// ---------------------------------------------------------------------------

/// Snapshot of player stats pushed each frame via [`StatusPanel::sync`].
#[derive(Clone, Debug, Default)]
struct StatSnapshot {
    rank_index: usize,
    a_hp: i32,
    hp_max: i32,
    a_end: i32,
    end_max: i32,
    a_mana: i32,
    mana_max: i32,
    weapon: i32,
    armor: i32,
}

// ---------------------------------------------------------------------------
// Widget
// ---------------------------------------------------------------------------

/// A compact status panel drawn in the upper-left corner.
///
/// Shows the player's rank sigil on the left and, when expanded (default),
/// HP/Endurance/Mana bars with overlaid `current / max` text to the right,
/// plus weapon and armor values below the bars.
pub struct StatusPanel {
    /// Bounds of the *expanded* panel (recalculated if needed).
    bounds: Bounds,
    /// Whether the stat bars are visible.
    expanded: bool,
    /// Semi-transparent background color.
    bg_color: Color,
    /// Latest stats from the player state.
    stats: StatSnapshot,
}

impl StatusPanel {
    /// Create a new `StatusPanel` positioned at `(x, y)`.
    ///
    /// # Arguments
    ///
    /// * `x` - Left edge in pixels.
    /// * `y` - Top edge in pixels.
    /// * `bg_color` - Semi-transparent background fill.
    ///
    /// # Returns
    ///
    /// A new `StatusPanel`, expanded by default.
    pub fn new(x: i32, y: i32, bg_color: Color) -> Self {
        Self {
            bounds: Self::compute_bounds(x, y, true),
            expanded: true,
            bg_color,
            stats: StatSnapshot::default(),
        }
    }

    /// Push the latest player stats into the widget.
    ///
    /// Must be called once per frame before `render` so that bar values are
    /// up-to-date.
    ///
    /// # Arguments
    ///
    /// * `ps` - Current player state.
    /// * `rank_index` - Pre-computed rank index (0–23).
    pub fn sync(&mut self, ps: &PlayerState, rank_index: usize) {
        let ci = ps.character_info();
        self.stats = StatSnapshot {
            rank_index,
            a_hp: ci.a_hp,
            hp_max: ci.hp[5] as i32,
            a_end: ci.a_end,
            end_max: ci.end[5] as i32,
            a_mana: ci.a_mana,
            mana_max: ci.mana[5] as i32,
            weapon: ci.weapon,
            armor: ci.armor,
        };
    }

    /// Compute the bounding rectangle for the given expansion state.
    fn compute_bounds(x: i32, y: i32, expanded: bool) -> Bounds {
        if expanded {
            let w = PANEL_PADDING * 2 + SIGIL_SIZE + SIGIL_BAR_GAP + BAR_WIDTH;
            // 3 bars + 2 gaps + 1 text row for weapon/armor
            let bars_h = BAR_HEIGHT * 3 + BAR_GAP * 2;
            let wv_row_h = font_cache::BITMAP_GLYPH_H as i32 + BAR_GAP;
            let h = PANEL_PADDING * 2 + bars_h + wv_row_h;
            Bounds::new(x, y, w as u32, h as u32)
        } else {
            let w = PANEL_PADDING * 2 + SIGIL_SIZE;
            let h = PANEL_PADDING * 2 + SIGIL_SIZE;
            Bounds::new(x, y, w as u32, h as u32)
        }
    }

    /// Returns the bounding rectangle of just the sigil icon (for hit-testing
    /// the toggle click).
    fn sigil_bounds(&self) -> Bounds {
        Bounds::new(
            self.bounds.x + PANEL_PADDING,
            self.bounds.y + PANEL_PADDING,
            SIGIL_SIZE as u32,
            SIGIL_SIZE as u32,
        )
    }

    /// Draw a single stat bar with centered `"cur / max"` text.
    fn draw_bar(
        ctx: &mut RenderContext,
        x: i32,
        y: i32,
        current: i32,
        max: i32,
        fill_color: Color,
        bg_color: Color,
    ) -> Result<(), String> {
        // Background (full capacity).
        ctx.canvas.set_draw_color(bg_color);
        ctx.canvas.fill_rect(sdl2::rect::Rect::new(
            x,
            y,
            BAR_WIDTH as u32,
            BAR_HEIGHT as u32,
        ))?;

        // Foreground (current value).
        if max > 0 {
            let filled = ((current as i64 * BAR_WIDTH as i64) / max as i64)
                .clamp(0, BAR_WIDTH as i64) as u32;
            if filled > 0 {
                ctx.canvas.set_draw_color(fill_color);
                ctx.canvas
                    .fill_rect(sdl2::rect::Rect::new(x, y, filled, BAR_HEIGHT as u32))?;
            }
        }

        // Centered text: "cur / max".
        let text = format!("{} / {}", current, max);
        let center_x = x + BAR_WIDTH / 2;
        let text_y = y + (BAR_HEIGHT - font_cache::BITMAP_GLYPH_H as i32) / 2;
        font_cache::draw_text_centered(ctx.canvas, ctx.gfx, FONT, &text, center_x, text_y)?;

        Ok(())
    }
}

impl Widget for StatusPanel {
    /// Returns the bounding rectangle of the panel.
    fn bounds(&self) -> &Bounds {
        &self.bounds
    }

    /// Moves the panel's top-left corner.
    ///
    /// # Arguments
    ///
    /// * `x` - New left edge.
    /// * `y` - New top edge.
    fn set_position(&mut self, x: i32, y: i32) {
        self.bounds = Self::compute_bounds(x, y, self.expanded);
    }

    /// Handle input events.
    ///
    /// A click inside the sigil area toggles the expanded state. All other
    /// events are ignored.
    ///
    /// # Arguments
    ///
    /// * `event` - The translated UI event.
    ///
    /// # Returns
    ///
    /// `Consumed` if the click hit the sigil, `Ignored` otherwise.
    fn handle_event(&mut self, event: &UiEvent) -> EventResponse {
        if let UiEvent::MouseClick { x, y, .. } = event {
            if self.sigil_bounds().contains_point(*x, *y) {
                self.expanded = !self.expanded;
                self.bounds = Self::compute_bounds(self.bounds.x, self.bounds.y, self.expanded);
                return EventResponse::Consumed;
            }
        }
        EventResponse::Ignored
    }

    /// Draw the status panel.
    ///
    /// # Arguments
    ///
    /// * `ctx` - Mutable render context (canvas + graphics cache).
    ///
    /// # Returns
    ///
    /// `Ok(())` on success, or an SDL2 error string.
    fn render(&mut self, ctx: &mut RenderContext) -> Result<(), String> {
        // Semi-transparent background
        ctx.canvas.set_blend_mode(BlendMode::Blend);
        ctx.canvas.set_draw_color(self.bg_color);
        ctx.canvas.fill_rect(sdl2::rect::Rect::new(
            self.bounds.x,
            self.bounds.y,
            self.bounds.width,
            self.bounds.height,
        ))?;

        // Rank sigil
        let sigil_x = self.bounds.x + PANEL_PADDING;
        let sigil_y = self.bounds.y + PANEL_PADDING;
        let sprite_id = 10 + self.stats.rank_index.min(20);
        let tex = ctx.gfx.get_texture(sprite_id);
        let q = tex.query();
        ctx.canvas.copy(
            tex,
            None,
            Some(sdl2::rect::Rect::new(sigil_x, sigil_y, q.width, q.height)),
        )?;

        if !self.expanded {
            return Ok(());
        }

        // Stat bars (to the right of the sigil)
        let bar_x = sigil_x + SIGIL_SIZE + SIGIL_BAR_GAP;
        let bar_y_start = self.bounds.y + PANEL_PADDING;

        Self::draw_bar(
            ctx,
            bar_x,
            bar_y_start,
            self.stats.a_hp,
            self.stats.hp_max,
            HP_FILL,
            HP_BG,
        )?;
        Self::draw_bar(
            ctx,
            bar_x,
            bar_y_start + BAR_HEIGHT + BAR_GAP,
            self.stats.a_end,
            self.stats.end_max,
            END_FILL,
            END_BG,
        )?;
        Self::draw_bar(
            ctx,
            bar_x,
            bar_y_start + (BAR_HEIGHT + BAR_GAP) * 2,
            self.stats.a_mana,
            self.stats.mana_max,
            MANA_FILL,
            MANA_BG,
        )?;

        // Weapon / Armor text row below bars
        let wv_y = bar_y_start + (BAR_HEIGHT + BAR_GAP) * 3;
        let wv_text = format!("WV: {}  AV: {}", self.stats.weapon, self.stats.armor);
        font_cache::draw_text(ctx.canvas, ctx.gfx, FONT, &wv_text, bar_x, wv_y)?;

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expanded_by_default() {
        let panel = StatusPanel::new(4, 4, Color::RGBA(10, 10, 30, 180));
        assert!(panel.expanded);
    }

    #[test]
    fn click_inside_sigil_toggles() {
        let mut panel = StatusPanel::new(4, 4, Color::RGBA(10, 10, 30, 180));
        assert!(panel.expanded);

        // Click inside the sigil area (sigil starts at x=8, y=8, size 32×32).
        let click = UiEvent::MouseClick {
            x: 20,
            y: 20,
            button: super::super::widget::MouseButton::Left,
        };
        let resp = panel.handle_event(&click);
        assert_eq!(resp, EventResponse::Consumed);
        assert!(!panel.expanded);

        // Click again to re-expand.
        let resp2 = panel.handle_event(&click);
        assert_eq!(resp2, EventResponse::Consumed);
        assert!(panel.expanded);
    }

    #[test]
    fn click_outside_is_ignored() {
        let mut panel = StatusPanel::new(4, 4, Color::RGBA(10, 10, 30, 180));
        let click = UiEvent::MouseClick {
            x: 500,
            y: 500,
            button: super::super::widget::MouseButton::Left,
        };
        let resp = panel.handle_event(&click);
        assert_eq!(resp, EventResponse::Ignored);
        assert!(panel.expanded);
    }

    #[test]
    fn collapsed_bounds_smaller_than_expanded() {
        let expanded = StatusPanel::compute_bounds(4, 4, true);
        let collapsed = StatusPanel::compute_bounds(4, 4, false);
        assert!(collapsed.width < expanded.width);
        assert!(collapsed.height < expanded.height);
    }

    #[test]
    fn set_position_preserves_expansion_state() {
        let mut panel = StatusPanel::new(0, 0, Color::RGBA(10, 10, 30, 180));
        let orig_width = panel.bounds.width;

        // Collapse it.
        panel.expanded = false;
        panel.set_position(10, 10);
        assert_eq!(panel.bounds.x, 10);
        assert_eq!(panel.bounds.y, 10);
        assert!(panel.bounds.width < orig_width);

        // Expand it.
        panel.expanded = true;
        panel.set_position(20, 20);
        assert_eq!(panel.bounds.x, 20);
        assert_eq!(panel.bounds.y, 20);
        assert_eq!(panel.bounds.width, orig_width);
    }

    #[test]
    fn sigil_bounds_inside_panel() {
        let panel = StatusPanel::new(4, 4, Color::RGBA(10, 10, 30, 180));
        let sb = panel.sigil_bounds();
        assert!(sb.x >= panel.bounds.x);
        assert!(sb.y >= panel.bounds.y);
        assert!(sb.x + sb.width as i32 <= panel.bounds.x + panel.bounds.width as i32);
        assert!(sb.y + sb.height as i32 <= panel.bounds.y + panel.bounds.height as i32);
    }
}
