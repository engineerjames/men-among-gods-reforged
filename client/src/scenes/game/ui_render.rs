use sdl2::{pixels::Color, render::Canvas, video::Window};

use mag_core::constants::{
    PL_ARMS, PL_BELT, PL_BODY, PL_CLOAK, PL_FEET, PL_HEAD, PL_LEGS, PL_NECK, PL_RING, PL_SHIELD,
    PL_TWOHAND, PL_WEAPON, TILEX, TILEY, WN_LHAND, WN_RHAND,
};
use mag_core::types::skilltab::get_skill_name;

use crate::{font_cache, gfx_cache::GraphicsCache, player_state::PlayerState};

use super::{
    GameScene, BAR_BG_COLOR, BAR_END_Y, BAR_FILL_COLOR, BAR_FILL_LOOK_COLOR, BAR_H, BAR_HP_Y,
    BAR_MANA_Y, BAR_SCALE_NUM, BAR_W_MAX, BAR_X, EQUIP_WNTAB, INV_SCROLL_MAX, INV_SCROLL_RANGE,
    INV_SCROLL_X, INV_SCROLL_Y_BASE, MINIMAP_VIEW_SIZE, MINIMAP_WORLD_SIZE, MINIMAP_X, MINIMAP_Y,
    MODE_INDICATOR_COLOR, NAME_AREA_W, NAME_AREA_X, NAME_Y, PORTRAIT_NAME_Y, PORTRAIT_RANK_Y,
    SCROLL_KNOB_COLOR, SCROLL_KNOB_H, SCROLL_KNOB_W, SKILL_SCROLL_MAX, SKILL_SCROLL_RANGE,
    SKILL_SCROLL_X, SKILL_SCROLL_Y_BASE, STAT_ARMOR_X, STAT_ARMOR_Y, STAT_END_X, STAT_END_Y,
    STAT_EXP_X, STAT_EXP_Y, STAT_HP_X, STAT_HP_Y, STAT_MANA_X, STAT_MANA_Y, STAT_MONEY_X,
    STAT_MONEY_Y, STAT_WEAPON_X, STAT_WEAPON_Y, UI_FONT, UI_FRAME_SPRITE,
};

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

    /// Draw mode indicator rectangles (fight/protect/magic, proz, names, hide).
    pub(super) fn draw_mode_indicators(
        canvas: &mut Canvas<Window>,
        ps: &PlayerState,
    ) -> Result<(), String> {
        let ci = ps.character_info();
        let pdata = ps.player_data();

        canvas.set_draw_color(MODE_INDICATOR_COLOR);

        match ci.mode {
            2 => canvas.draw_rect(sdl2::rect::Rect::new(608, 554, 45, 12))?,
            1 => canvas.draw_rect(sdl2::rect::Rect::new(656, 554, 45, 12))?,
            _ => canvas.draw_rect(sdl2::rect::Rect::new(704, 554, 45, 12))?,
        }

        if pdata.show_proz != 0 {
            canvas.draw_rect(sdl2::rect::Rect::new(753, 554, 45, 12))?;
        }
        if pdata.show_names != 0 {
            canvas.draw_rect(sdl2::rect::Rect::new(704, 569, 45, 12))?;
        }
        if pdata.hide != 0 {
            canvas.draw_rect(sdl2::rect::Rect::new(656, 569, 45, 12))?;
        }

        Ok(())
    }

    /// Draw inventory backpack, worn equipment slots, spell icons, carried item,
    /// and the inventory scrollbar knob.
    pub(super) fn draw_inventory_equipment_spells(
        &self,
        canvas: &mut Canvas<Window>,
        gfx: &mut GraphicsCache,
        ps: &PlayerState,
    ) -> Result<(), String> {
        let ci = ps.character_info();
        let show_shop = ps.should_show_shop();
        let show_look = ps.should_show_look() && !show_shop;

        if show_look {
            let look = ps.look_target();
            for n in 0..12usize {
                let worn_index = EQUIP_WNTAB[n];
                let sprite = look.worn(worn_index) as i32;
                if sprite <= 0 {
                    continue;
                }
                let x = 303 + ((n % 2) as i32) * 35;
                let y = 2 + ((n / 2) as i32) * 35;
                Self::draw_ui_item_with_hover(canvas, gfx, sprite, x, y, false)?;
            }

            if ci.citem > 0 {
                let tex = gfx.get_texture(ci.citem as usize);
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
                )?;
            }

            return Ok(());
        }

        let hovered_inv_slot =
            if (220..=290).contains(&self.mouse_x) && (2..=177).contains(&self.mouse_y) {
                let col = ((self.mouse_x - 220) / 35) as usize;
                let row = ((self.mouse_y - 2) / 35) as usize;
                if col < 2 && row < 5 {
                    Some(self.inv_scroll + row * 2 + col)
                } else {
                    None
                }
            } else {
                None
            };

        let hovered_equip_index =
            if (303..=373).contains(&self.mouse_x) && (2..=212).contains(&self.mouse_y) {
                let col = ((self.mouse_x - 303) / 35) as usize;
                let row = ((self.mouse_y - 2) / 35) as usize;
                if col < 2 && row < 6 {
                    Some(row * 2 + col)
                } else {
                    None
                }
            } else {
                None
            };

        for n in 0..10usize {
            let idx = self.inv_scroll + n;
            if idx >= ci.item.len() {
                continue;
            }
            let sprite = ci.item[idx];
            if sprite <= 0 {
                continue;
            }
            let x = 220 + ((n % 2) as i32) * 35;
            let y = 2 + ((n / 2) as i32) * 35;
            let hovered = hovered_inv_slot == Some(idx);
            Self::draw_ui_item_with_hover(canvas, gfx, sprite, x, y, hovered)?;
        }

        for n in 0..12usize {
            let worn_index = EQUIP_WNTAB[n];
            let sprite = ci.worn[worn_index];
            if sprite <= 0 {
                continue;
            }
            let x = 303 + ((n % 2) as i32) * 35;
            let y = 2 + ((n / 2) as i32) * 35;
            let hovered = hovered_equip_index == Some(n);
            Self::draw_ui_item_with_hover(canvas, gfx, sprite, x, y, hovered)?;
        }

        // C-equivalent of inter.c::reset_block + engine.c overlay draw:
        // when carrying an item, draw sprite 4 over worn slots where placement is invalid.
        if !show_shop && ci.citem > 0 {
            let citem_p = ci.citem_p as u16;
            let mut blocked = [false; 20];

            let slot_accepts = |slot: usize| -> bool {
                match slot {
                    mag_core::constants::WN_HEAD => (citem_p & PL_HEAD) != 0,
                    mag_core::constants::WN_NECK => (citem_p & PL_NECK) != 0,
                    mag_core::constants::WN_BODY => (citem_p & PL_BODY) != 0,
                    mag_core::constants::WN_ARMS => (citem_p & PL_ARMS) != 0,
                    mag_core::constants::WN_BELT => (citem_p & PL_BELT) != 0,
                    mag_core::constants::WN_LEGS => (citem_p & PL_LEGS) != 0,
                    mag_core::constants::WN_FEET => (citem_p & PL_FEET) != 0,
                    WN_RHAND => (citem_p & PL_WEAPON) != 0,
                    WN_LHAND => (citem_p & PL_SHIELD) != 0,
                    mag_core::constants::WN_CLOAK => (citem_p & PL_CLOAK) != 0,
                    mag_core::constants::WN_LRING | mag_core::constants::WN_RRING => {
                        (citem_p & PL_RING) != 0
                    }
                    _ => true,
                }
            };

            for slot in 0..20usize {
                blocked[slot] = !slot_accepts(slot);
            }

            if (ci.worn_p[WN_RHAND] as u16 & PL_TWOHAND) != 0 {
                blocked[WN_LHAND] = true;
            }

            for n in 0..12usize {
                let worn_index = EQUIP_WNTAB[n];
                if blocked[worn_index] {
                    let x = 303 + ((n % 2) as i32) * 35;
                    let y = 2 + ((n / 2) as i32) * 35;
                    let tex = gfx.get_texture(4);
                    let q = tex.query();
                    canvas.copy(
                        tex,
                        None,
                        Some(sdl2::rect::Rect::new(x, y, q.width, q.height)),
                    )?;
                }
            }
        }

        for n in 0..20usize {
            let sprite = ci.spell[n];
            if sprite <= 0 {
                continue;
            }
            let x = 374 + ((n % 5) as i32) * 24;
            let y = 4 + ((n / 5) as i32) * 24;
            let tex = gfx.get_texture(sprite as usize);
            let q = tex.query();

            // Match original engine.c spell-slot rendering:
            // copyspritex(pl.spell[n], ..., 15-min(15,pl.active[n]))
            // The effect parameter darkens RGB only (do_rgb8_effect / LEFFECT curve);
            // the sprite's own alpha channel is left untouched — no alpha-mod.
            let active = (ci.active[n] as i32).clamp(0, 15);
            let effect = 15 - active;
            let atten = (255 * 120 / (effect * effect + 120)) as u8;

            tex.set_color_mod(atten, atten, atten);
            canvas.copy(
                tex,
                None,
                Some(sdl2::rect::Rect::new(x, y, q.width, q.height)),
            )?;
            tex.set_color_mod(255, 255, 255);
        }

        if ci.citem > 0 {
            let tex = gfx.get_texture(ci.citem as usize);
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
            )?;
        }

        // C-style inventory scrollbar knob (engine.c: dd_showbar(290,36+(inv_pos*94)/30,11,11,GREEN)).
        let inv_pos = (self.inv_scroll as i32).clamp(0, INV_SCROLL_MAX);
        let inv_y = INV_SCROLL_Y_BASE + (inv_pos * INV_SCROLL_RANGE) / INV_SCROLL_MAX;
        canvas.set_draw_color(SCROLL_KNOB_COLOR);
        canvas.fill_rect(sdl2::rect::Rect::new(
            INV_SCROLL_X,
            inv_y,
            SCROLL_KNOB_W,
            SCROLL_KNOB_H,
        ))?;

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

    /// Update the persistent world minimap buffer from the current map state, then
    /// blit the 128x128 viewport centred on the player to the minimap area (x=3, y=471).
    pub(super) fn draw_minimap(
        &mut self,
        canvas: &mut Canvas<Window>,
        gfx: &mut GraphicsCache,
        ps: &PlayerState,
    ) -> Result<(), String> {
        let map = ps.map();

        let Some(center) = map.tile_at_xy(TILEX / 2, TILEY / 2) else {
            return Ok(());
        };

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

        // Build the 128x128 viewport.
        let half = (MINIMAP_VIEW_SIZE as i32) / 2;
        let mapx = ((center.x as i32) - half)
            .clamp(0, MINIMAP_WORLD_SIZE as i32 - MINIMAP_VIEW_SIZE as i32);
        let mapy = ((center.y as i32) - half)
            .clamp(0, MINIMAP_WORLD_SIZE as i32 - MINIMAP_VIEW_SIZE as i32);

        // C call: dd_show_map(xmap, mapy, mapx)  →  xo=mapy, yo=mapx
        // C blit index: s = (y + yo)*1024 + xo  =  (row + mapx)*1024 + (col + mapy)
        let xo = mapy as usize; // column offset (global Y)
        let yo = mapx as usize; // row offset (global X)

        let view_size = MINIMAP_VIEW_SIZE as usize;
        let mut pixels: Vec<u8> = vec![0u8; view_size * view_size * 4];
        for row in 0..view_size {
            for col in 0..view_size {
                let src_row = yo + row;
                let src_col = xo + col;
                if src_row >= MINIMAP_WORLD_SIZE || src_col >= MINIMAP_WORLD_SIZE {
                    continue;
                }
                // Match C: s = (y+yo)*1024 + xo;  s++ per column
                let src = (src_row * MINIMAP_WORLD_SIZE + src_col) * 4;
                let dst = (row * view_size + col) * 4;
                // ABGR8888 on little-endian: memory bytes are [R,G,B,A] — matches xmap layout directly.
                pixels[dst] = self.minimap_xmap[src];
                pixels[dst + 1] = self.minimap_xmap[src + 1];
                pixels[dst + 2] = self.minimap_xmap[src + 2];
                pixels[dst + 3] = self.minimap_xmap[src + 3];
            }
        }

        gfx.ensure_minimap_texture();
        if let Some(tex) = gfx.minimap_texture.as_mut() {
            let pitch = view_size * 4;
            tex.update(None, &pixels, pitch)
                .map_err(|e| e.to_string())?;
            canvas.copy(
                tex,
                None,
                Some(sdl2::rect::Rect::new(
                    MINIMAP_X,
                    MINIMAP_Y,
                    MINIMAP_VIEW_SIZE,
                    MINIMAP_VIEW_SIZE,
                )),
            )?;
        }

        Ok(())
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
