use sdl2::{render::Canvas, video::Window};

use mag_core::constants::{TILEX, TILEY};

use crate::{font_cache, gfx_cache::GraphicsCache, player_state::PlayerState};

use super::{GameScene, MINIMAP_WORLD_SIZE, UI_FONT};

impl GameScene {
    /// Draw a UI item sprite with an optional additive hover highlight.
    pub(super) fn draw_ui_item_with_hover(
        canvas: &mut Canvas<Window>,
        gfx: &mut GraphicsCache,
        sprite_id: i32,
        x: i32,
        y: i32,
        hovered: bool,
    ) -> Result<(), String> {
        if sprite_id <= 0 {
            return Ok(());
        }

        let texture = gfx.get_texture(sprite_id as usize);
        let q = texture.query();
        canvas.copy(
            texture,
            None,
            Some(sdl2::rect::Rect::new(x, y, q.width, q.height)),
        )?;

        if hovered {
            texture.set_blend_mode(sdl2::render::BlendMode::Add);
            texture.set_alpha_mod(96);
            let result = canvas.copy(
                texture,
                None,
                Some(sdl2::rect::Rect::new(x, y, q.width, q.height)),
            );
            texture.set_alpha_mod(255);
            texture.set_blend_mode(sdl2::render::BlendMode::Blend);
            result?;
        }

        Ok(())
    }

    /// Draw the shop/depot/grave overlay grid and buy/sell price labels.
    pub(super) fn draw_shop_overlay(
        &self,
        canvas: &mut Canvas<Window>,
        gfx: &mut GraphicsCache,
        ps: &PlayerState,
    ) -> Result<(), String> {
        if !ps.should_show_shop() {
            return Ok(());
        }
        let shop = ps.shop_target();

        let bg = gfx.get_texture(92);
        let bq = bg.query();
        canvas.copy(
            bg,
            None,
            Some(sdl2::rect::Rect::new(220, 260, bq.width, bq.height)),
        )?;

        let hovered_shop_slot =
            if (220..=500).contains(&self.mouse_x) && (261..=552).contains(&self.mouse_y) {
                let tx = ((self.mouse_x - 220) / 35) as usize;
                let ty = ((self.mouse_y - 261) / 35) as usize;
                let nr = tx + ty * 8;
                if tx < 8 && nr < 62 {
                    Some(nr)
                } else {
                    None
                }
            } else {
                None
            };

        for i in 0..62usize {
            let item = shop.item(i);
            if item == 0 {
                continue;
            }
            let x = 222 + ((i % 8) as i32) * 35;
            let y = 262 + ((i / 8) as i32) * 35;
            let hovered = hovered_shop_slot == Some(i);
            Self::draw_ui_item_with_hover(canvas, gfx, item as i32, x, y, hovered)?;
        }

        if (222..=501).contains(&self.mouse_x) && (262..=541).contains(&self.mouse_y) {
            let col = ((self.mouse_x - 222) / 35) as usize;
            let row = ((self.mouse_y - 262) / 35) as usize;
            let idx = row * 8 + col;
            if idx < 62 {
                let price = shop.price(idx);
                if price != 0 {
                    let sell_text = format!("Sell: {}G {}S", price / 100, price % 100);
                    font_cache::draw_text(canvas, gfx, UI_FONT, &sell_text, 225, 549)?;
                }
            }
        }

        if ps.character_info().citem > 0 && shop.pl_price() > 0 {
            let buy_text = format!(
                "Buy:  {}G {}S",
                shop.pl_price() / 100,
                shop.pl_price() % 100
            );
            font_cache::draw_text(canvas, gfx, UI_FONT, &buy_text, 225, 559)?;
        }

        Ok(())
    }

    /// Repaint the persistent 1024×1024 world minimap buffer from the current
    /// map state.
    ///
    /// Only performs work when the player has moved since the last call.
    /// The viewport extraction + rendering is handled by [`MinimapWidget`].
    ///
    /// # Arguments
    ///
    /// * `gfx` - Graphics cache (used for average-color lookups).
    /// * `ps` - Current player state (map tiles + player position).
    ///
    /// # Returns
    ///
    /// The player's center `(x, y)` in world-map coordinates, or `None` if
    /// the center tile is unavailable.
    pub(super) fn update_minimap_xmap(
        &mut self,
        gfx: &mut GraphicsCache,
        ps: &PlayerState,
    ) -> Option<(u16, u16)> {
        let map = ps.map();

        let center = map.tile_at_xy(TILEX / 2, TILEY / 2)?;

        let center_xy = (center.x, center.y);

        // Only repaint xmap when the player moved.
        if self.minimap_last_xy != Some(center_xy) {
            self.minimap_last_xy = Some(center_xy);

            for idx in 0..map.len() {
                let Some(tile) = map.tile_at_index(idx) else {
                    continue;
                };
                let gx = tile.x as usize;
                let gy = tile.y as usize;
                if gx >= MINIMAP_WORLD_SIZE || gy >= MINIMAP_WORLD_SIZE {
                    continue;
                }
                if (tile.flags & mag_core::constants::INVIS) != 0 {
                    continue;
                }
                let cell = (gy + gx * MINIMAP_WORLD_SIZE) * 4;

                let back_id = tile.back.max(0) as usize;
                if back_id != 0 {
                    // Use the alpha byte as the "never visited" sentinel: the buffer is
                    // zero-initialised, so alpha==0 means this cell has never been painted.
                    // RGB-only checks incorrectly treated legitimately-black backgrounds as
                    // blank, causing them to be re-queried on every step.
                    let is_blank = self.minimap_xmap[cell + 3] == 0;
                    // 0xFF marks the player position — always overwrite it so the old
                    // white dot is replaced with the real tile color when the player moves.
                    let is_player_marker = self.minimap_xmap[cell] == 0xFF
                        && self.minimap_xmap[cell + 1] == 0xFF
                        && self.minimap_xmap[cell + 2] == 0xFF;
                    if is_blank || is_player_marker {
                        let (r, g, b) = gfx.get_avg_color(back_id);
                        self.minimap_xmap[cell] = r;
                        self.minimap_xmap[cell + 1] = g;
                        self.minimap_xmap[cell + 2] = b;
                        self.minimap_xmap[cell + 3] = 255;
                    }
                }

                // Objects override background — but only when the sprite has a
                // non-zero average color.  Transparent / invisible obj sprites
                // return (0,0,0) from get_avg_color; writing that value would paint
                // an opaque black pixel over the valid background color.  In the
                // original C engine, setting xmap[..]=0 implicitly marked the cell
                // as "unvisited" so the background reclaimed it next pass; our RGBA
                // buffer has no such equivalence, so we guard the write instead.
                if tile.obj1 > 0 {
                    let (r, g, b) = gfx.get_avg_color(tile.obj1 as usize);
                    if (r | g | b) != 0 {
                        self.minimap_xmap[cell] = r;
                        self.minimap_xmap[cell + 1] = g;
                        self.minimap_xmap[cell + 2] = b;
                        self.minimap_xmap[cell + 3] = 255;
                    }
                }
            }

            // Mark player position (white pixel).
            let cx = center.x as usize;
            let cy = center.y as usize;
            if cx < MINIMAP_WORLD_SIZE && cy < MINIMAP_WORLD_SIZE {
                let cell = (cy + cx * MINIMAP_WORLD_SIZE) * 4;
                self.minimap_xmap[cell] = 0xFF;
                self.minimap_xmap[cell + 1] = 0xFF;
                self.minimap_xmap[cell + 2] = 0xFF;
                self.minimap_xmap[cell + 3] = 0xFF;
            }
        }

        Some(center_xy)
    }

    /// Draw the currently carried item (citem) sprite under the mouse cursor.
    ///
    /// This is drawn unconditionally (regardless of inventory panel visibility)
    /// so the player always sees the item they are holding.
    ///
    /// # Arguments
    ///
    /// * `canvas` - SDL2 canvas.
    /// * `gfx` - Graphics/texture cache.
    /// * `ps` - Current player state.
    ///
    /// # Returns
    ///
    /// * `Ok(())` on success, or an SDL2 error string.
    pub(super) fn draw_carried_item(
        &self,
        canvas: &mut Canvas<Window>,
        gfx: &mut GraphicsCache,
        ps: &PlayerState,
    ) -> Result<(), String> {
        let citem = ps.character_info().citem;
        if citem <= 0 {
            return Ok(());
        }
        let tex = gfx.get_texture(citem as usize);
        let q = tex.query();
        canvas.copy(
            tex,
            None,
            Some(sdl2::rect::Rect::new(
                self.mouse_x - 8,
                self.mouse_y - 8,
                q.width,
                q.height,
            )),
        )
    }
}
