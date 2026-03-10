//! Character inspection panel (right side of viewport).
//!
//! Visible when `should_show_look()` is true and `should_show_shop()` is
//! false. Displays the looked-at character's name, animated sprite (from
//! the map tile's `obj2` field at 2× zoom), equipment grid, and
//! HP/Endurance/Mana bars.

use mag_core::constants::{
    TILEX, TILEY, WN_ARMS, WN_BELT, WN_BODY, WN_CLOAK, WN_FEET, WN_HEAD, WN_LEGS, WN_LHAND,
    WN_LRING, WN_NECK, WN_RHAND, WN_RRING,
};
use sdl2::pixels::Color;
use sdl2::render::BlendMode;

use crate::font_cache;
use crate::player_state::PlayerState;

use super::widget::{Bounds, EventResponse, UiEvent, Widget};
use super::RenderContext;

// ---------------------------------------------------------------------------
// Layout constants
// ---------------------------------------------------------------------------

/// Padding inside the panel.
const PAD: i32 = 6;
/// Vertical gap between sections.
const GAP: i32 = 4;
/// Equipment grid cell size.
const EQUIP_CELL: i32 = 24;
/// Equipment grid columns.
const EQUIP_COLS: i32 = 2;
/// Equipment grid rows.
const EQUIP_ROWS: i32 = 6;
/// Sprite zoom factor.
const SPRITE_ZOOM: u32 = 2;
/// Stat bar width.
const BAR_W: i32 = 100;
/// Stat bar height.
const BAR_H: i32 = 10;
/// Gap between bars.
const BAR_GAP: i32 = 3;
/// Font index (yellow bitmap).
const FONT: usize = 1;

// Bar colors
const HP_FILL: Color = Color::RGB(180, 30, 30);
const END_FILL: Color = Color::RGB(200, 180, 40);
const MANA_FILL: Color = Color::RGB(40, 80, 200);
const HP_BG: Color = Color::RGB(60, 10, 10);
const END_BG: Color = Color::RGB(65, 58, 12);
const MANA_BG: Color = Color::RGB(12, 25, 65);

/// Equipment grid slot order (row-major, 2 cols × 6 rows → WN_* indices).
const EQUIP_WNTAB: [usize; 12] = [
    WN_HEAD, WN_CLOAK, WN_BODY, WN_ARMS, WN_NECK, WN_BELT, WN_RHAND, WN_LHAND, WN_LRING,
    WN_RRING, WN_LEGS, WN_FEET,
];

// ---------------------------------------------------------------------------
// Snapshot
// ---------------------------------------------------------------------------

/// Cached data from the look target, copied once per frame.
#[derive(Clone, Debug, Default)]
struct LookSnapshot {
    visible: bool,
    name: String,
    /// Sprite ID from tile obj2 (pre-computed by engine_tick).
    sprite_id: i32,
    worn: [u16; 12],
    a_hp: u32,
    hp: u32,
    a_end: u32,
    end: u32,
    a_mana: u32,
    mana: u32,
}

// ---------------------------------------------------------------------------
// Widget
// ---------------------------------------------------------------------------

/// A panel that shows detailed information about the currently looked-at
/// character: name, animated sprite, equipment, and stat bars.
pub struct LookPanel {
    bounds: Bounds,
    bg_color: Color,
    snap: LookSnapshot,
}

impl LookPanel {
    /// Create a new `LookPanel`.
    ///
    /// # Arguments
    ///
    /// * `bounds` - Position and size of the panel.
    /// * `bg_color` - Semi-transparent background color.
    ///
    /// # Returns
    ///
    /// A new `LookPanel`.
    pub fn new(bounds: Bounds, bg_color: Color) -> Self {
        Self {
            bounds,
            bg_color,
            snap: LookSnapshot::default(),
        }
    }

    /// Push the latest look target state into the widget.
    ///
    /// Reads the look target from `PlayerState` and finds the matching map
    /// tile to extract the animated sprite (`obj2`).
    ///
    /// # Arguments
    ///
    /// * `ps` - Current player state.
    pub fn sync(&mut self, ps: &PlayerState) {
        let show_look = ps.should_show_look() && !ps.should_show_shop();
        if !show_look {
            self.snap.visible = false;
            return;
        }

        let look = ps.look_target();
        let ch_nr = look.nr();

        // Find the tile with a matching ch_nr to get obj2 (animated sprite).
        let mut sprite_id: i32 = 0;
        if ch_nr != 0 {
            let map = ps.map();
            let total = TILEX * TILEY;
            for i in 0..total {
                if let Some(tile) = map.tile_at_index(i) {
                    if tile.ch_nr == ch_nr {
                        sprite_id = tile.obj2;
                        break;
                    }
                }
            }
        }

        // Fall back to the look struct's static sprite if tile search failed.
        if sprite_id <= 0 {
            sprite_id = look.sprite() as i32;
        }

        let mut worn = [0u16; 12];
        for (n, slot) in EQUIP_WNTAB.iter().enumerate() {
            worn[n] = look.worn(*slot);
        }

        self.snap = LookSnapshot {
            visible: true,
            name: look.name().unwrap_or("").to_string(),
            sprite_id,
            worn,
            a_hp: look.a_hp(),
            hp: look.hp(),
            a_end: look.a_end(),
            end: look.end(),
            a_mana: look.a_mana(),
            mana: look.mana(),
        };
    }

    /// Draw a stat bar with centered fraction text.
    fn draw_bar(
        ctx: &mut RenderContext,
        x: i32,
        y: i32,
        current: u32,
        max: u32,
        fill: Color,
        bg: Color,
    ) -> Result<(), String> {
        ctx.canvas.set_draw_color(bg);
        ctx.canvas
            .fill_rect(sdl2::rect::Rect::new(x, y, BAR_W as u32, BAR_H as u32))?;

        if max > 0 {
            let filled =
                ((current as i64 * BAR_W as i64) / max as i64).clamp(0, BAR_W as i64) as u32;
            if filled > 0 {
                ctx.canvas.set_draw_color(fill);
                ctx.canvas
                    .fill_rect(sdl2::rect::Rect::new(x, y, filled, BAR_H as u32))?;
            }
        }

        let text = format!("{}/{}", current, max);
        let cx = x + BAR_W / 2;
        let ty = y + (BAR_H - font_cache::BITMAP_GLYPH_H as i32) / 2;
        font_cache::draw_text_centered(ctx.canvas, ctx.gfx, FONT, &text, cx, ty)?;
        Ok(())
    }
}

impl Widget for LookPanel {
    /// Returns the bounding rectangle.
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
        self.bounds.x = x;
        self.bounds.y = y;
    }

    /// Input events are ignored — the panel is display-only.
    ///
    /// # Arguments
    ///
    /// * `_event` - The UI event (unused).
    ///
    /// # Returns
    ///
    /// Always `EventResponse::Ignored`.
    fn handle_event(&mut self, _event: &UiEvent) -> EventResponse {
        EventResponse::Ignored
    }

    /// Draw the look panel.
    ///
    /// # Arguments
    ///
    /// * `ctx` - Mutable render context (canvas + graphics cache).
    ///
    /// # Returns
    ///
    /// `Ok(())` on success, or an SDL2 error string.
    fn render(&mut self, ctx: &mut RenderContext) -> Result<(), String> {
        if !self.snap.visible {
            return Ok(());
        }

        // Background
        ctx.canvas.set_blend_mode(BlendMode::Blend);
        ctx.canvas.set_draw_color(self.bg_color);
        ctx.canvas.fill_rect(sdl2::rect::Rect::new(
            self.bounds.x,
            self.bounds.y,
            self.bounds.width,
            self.bounds.height,
        ))?;

        let mut y = self.bounds.y + PAD;
        let cx = self.bounds.x + self.bounds.width as i32 / 2;

        // Name (centered)
        font_cache::draw_text_centered(ctx.canvas, ctx.gfx, FONT, &self.snap.name, cx, y)?;
        y += font_cache::BITMAP_GLYPH_H as i32 + GAP;

        // Sprite (2× zoom, centered)
        if self.snap.sprite_id > 0 {
            let tex = ctx.gfx.get_texture(self.snap.sprite_id as usize);
            let q = tex.query();
            let draw_w = q.width * SPRITE_ZOOM;
            let draw_h = q.height * SPRITE_ZOOM;
            let sx = cx - draw_w as i32 / 2;
            ctx.canvas.copy(
                tex,
                None,
                Some(sdl2::rect::Rect::new(sx, y, draw_w, draw_h)),
            )?;
            y += draw_h as i32 + GAP;
        }

        // Equipment grid (2 cols × 6 rows)
        let equip_x = cx - (EQUIP_COLS * EQUIP_CELL) / 2;
        for n in 0..12usize {
            let sprite = self.snap.worn[n];
            if sprite == 0 {
                continue;
            }
            let ex = equip_x + ((n % EQUIP_COLS as usize) as i32) * EQUIP_CELL;
            let ey = y + ((n / EQUIP_COLS as usize) as i32) * EQUIP_CELL;
            let tex = ctx.gfx.get_texture(sprite as usize);
            let q = tex.query();
            ctx.canvas.copy(
                tex,
                None,
                Some(sdl2::rect::Rect::new(ex, ey, q.width, q.height)),
            )?;
        }
        y += EQUIP_ROWS * EQUIP_CELL + GAP;

        // Stat bars
        let bar_x = cx - BAR_W / 2;
        Self::draw_bar(ctx, bar_x, y, self.snap.a_hp, self.snap.hp, HP_FILL, HP_BG)?;
        y += BAR_H + BAR_GAP;
        Self::draw_bar(
            ctx,
            bar_x,
            y,
            self.snap.a_end,
            self.snap.end,
            END_FILL,
            END_BG,
        )?;
        y += BAR_H + BAR_GAP;
        Self::draw_bar(
            ctx,
            bar_x,
            y,
            self.snap.a_mana,
            self.snap.mana,
            MANA_FILL,
            MANA_BG,
        )?;

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
    fn not_visible_by_default() {
        let panel = LookPanel::new(
            Bounds::new(0, 0, 140, 260),
            Color::RGBA(10, 10, 30, 180),
        );
        assert!(!panel.snap.visible);
    }

    #[test]
    fn events_are_ignored() {
        let mut panel = LookPanel::new(
            Bounds::new(0, 0, 140, 260),
            Color::RGBA(10, 10, 30, 180),
        );
        let click = UiEvent::MouseClick {
            x: 70,
            y: 130,
            button: super::super::widget::MouseButton::Left,
            modifiers: super::super::widget::KeyModifiers::default(),
        };
        assert_eq!(panel.handle_event(&click), EventResponse::Ignored);
    }
}
