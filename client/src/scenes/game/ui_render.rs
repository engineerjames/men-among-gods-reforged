use sdl2::{pixels::Color, render::Canvas, video::Window};

use mag_core::constants::{TILEX, TILEY};
use mag_core::types::skilltab::get_skill_name;

use crate::{font_cache, gfx_cache::GraphicsCache, player_state::PlayerState};

use super::{
    GameScene, BAR_BG_COLOR, BAR_END_Y, BAR_FILL_COLOR, BAR_FILL_LOOK_COLOR, BAR_H, BAR_HP_Y,
    BAR_MANA_Y, BAR_SCALE_NUM, BAR_W_MAX, BAR_X, EQUIP_WNTAB, INPUT_X, INPUT_Y, INV_SCROLL_MAX,
    INV_SCROLL_RANGE, INV_SCROLL_X, INV_SCROLL_Y_BASE, LOG_LINES, LOG_LINE_H, LOG_X, LOG_Y,
    MINIMAP_VIEW_SIZE, MINIMAP_WORLD_SIZE, MINIMAP_X, MINIMAP_Y, MODE_INDICATOR_COLOR, NAME_AREA_W,
    NAME_AREA_X, NAME_Y, PORTRAIT_NAME_Y, PORTRAIT_RANK_Y, SCROLL_KNOB_COLOR, SCROLL_KNOB_H,
    SCROLL_KNOB_W, SKILL_SCROLL_MAX, SKILL_SCROLL_RANGE, SKILL_SCROLL_X, SKILL_SCROLL_Y_BASE,
    STAT_ARMOR_X, STAT_ARMOR_Y, STAT_END_X, STAT_END_Y, STAT_EXP_X, STAT_EXP_Y, STAT_HP_X,
    STAT_HP_Y, STAT_MANA_X, STAT_MANA_Y, STAT_MONEY_X, STAT_MONEY_Y, STAT_WEAPON_X, STAT_WEAPON_Y,
    UI_FONT, UI_FRAME_SPRITE,
};

use crate::types::log_message::LogMessageColor;

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

    /// Draw the static UI frame (sprite 1) at (0,0), covering the full 800x600 window.
    pub(super) fn draw_ui_frame(
        canvas: &mut Canvas<Window>,
        gfx: &mut GraphicsCache,
    ) -> Result<(), String> {
        let texture = gfx.get_texture(UI_FRAME_SPRITE);
        let q = texture.query();
        canvas.copy(
            texture,
            None,
            Some(sdl2::rect::Rect::new(0, 0, q.width, q.height)),
        )
    }

    /// Draw HP/End/Mana bars at the classic engine.c positions.
    pub(super) fn draw_bars(canvas: &mut Canvas<Window>, ps: &PlayerState) -> Result<(), String> {
        let ci = ps.character_info();

        let hp = ci.a_hp;
        let max_hp = ci.hp[5] as i32;
        let end_val = ci.a_end;
        let max_end = ci.end[5] as i32;
        let mana = ci.a_mana;
        let max_mana = ci.mana[5] as i32;

        // dd_showbar(373, y, n, 6, color) — bar_width = (cur * 62) / max, clamped [0,62]
        let draw_bar = |canvas: &mut Canvas<Window>,
                        y: i32,
                        cur: i32,
                        max: i32,
                        fill: Color|
         -> Result<(), String> {
            if max <= 0 {
                return Ok(());
            }
            let filled = ((cur * BAR_SCALE_NUM) / max).clamp(0, BAR_W_MAX);
            // Background (full capacity)
            canvas.set_draw_color(BAR_BG_COLOR);
            canvas.fill_rect(sdl2::rect::Rect::new(BAR_X, y, BAR_W_MAX as u32, BAR_H))?;
            // Foreground (current)
            if filled > 0 {
                canvas.set_draw_color(fill);
                canvas.fill_rect(sdl2::rect::Rect::new(BAR_X, y, filled as u32, BAR_H))?;
            }
            Ok(())
        };

        draw_bar(canvas, BAR_HP_Y, hp, max_hp, BAR_FILL_COLOR)?;
        draw_bar(canvas, BAR_END_Y, end_val, max_end, BAR_FILL_COLOR)?;
        draw_bar(canvas, BAR_MANA_Y, mana, max_mana, BAR_FILL_COLOR)?;

        if ps.should_show_look() {
            let look = ps.look_target();
            let base_hp = max_hp.max(1);
            let base_end = max_end.max(1);
            let base_mana = max_mana.max(1);

            draw_bar(
                canvas,
                BAR_HP_Y,
                look.a_hp() as i32,
                base_hp,
                BAR_FILL_LOOK_COLOR,
            )?;
            draw_bar(
                canvas,
                BAR_END_Y,
                look.a_end() as i32,
                base_end,
                BAR_FILL_LOOK_COLOR,
            )?;
            draw_bar(
                canvas,
                BAR_MANA_Y,
                look.a_mana() as i32,
                base_mana,
                BAR_FILL_LOOK_COLOR,
            )?;
        }

        Ok(())
    }

    /// Draw stat text labels (HP, End, Mana, Gold, Weapon, Armor, Exp, name).
    pub(super) fn draw_stat_text(
        canvas: &mut Canvas<Window>,
        gfx: &mut GraphicsCache,
        ps: &PlayerState,
    ) -> Result<(), String> {
        let ci = ps.character_info();

        // Hitpoints / Endurance / Mana text
        let hp_text = format!("Hitpoints         {:>3} {:>3}", ci.a_hp, ci.hp[5]);
        font_cache::draw_text(canvas, gfx, UI_FONT, &hp_text, STAT_HP_X, STAT_HP_Y)?;

        let end_text = format!("Endurance         {:>3} {:>3}", ci.a_end, ci.end[5]);
        font_cache::draw_text(canvas, gfx, UI_FONT, &end_text, STAT_END_X, STAT_END_Y)?;

        let mana_text = format!("Mana              {:>3} {:>3}", ci.a_mana, ci.mana[5]);
        font_cache::draw_text(canvas, gfx, UI_FONT, &mana_text, STAT_MANA_X, STAT_MANA_Y)?;

        // Gold (money display: G and S)
        let gold = ci.gold / 100;
        let silver = ci.gold % 100;
        let money_text = format!("Money  {:>8}G {:>2}S", gold, silver);
        font_cache::draw_text(
            canvas,
            gfx,
            UI_FONT,
            &money_text,
            STAT_MONEY_X,
            STAT_MONEY_Y,
        )?;

        // Weapon / Armor / Experience in bottom-right area
        let wv_text = format!("Weapon value   {:>10}", ci.weapon);
        font_cache::draw_text(canvas, gfx, UI_FONT, &wv_text, STAT_WEAPON_X, STAT_WEAPON_Y)?;

        let av_text = format!("Armor value    {:>10}", ci.armor);
        font_cache::draw_text(canvas, gfx, UI_FONT, &av_text, STAT_ARMOR_X, STAT_ARMOR_Y)?;

        let exp_text = format!("Experience     {:>10}", ci.points_tot);
        font_cache::draw_text(canvas, gfx, UI_FONT, &exp_text, STAT_EXP_X, STAT_EXP_Y)?;

        // Character/selection name (centered in 125px area at top).
        // Matches original behavior: show selected target name when available.
        let own_name = mag_core::string_operations::c_string_to_str(&ci.name);
        let top_name = if ps.selected_char() != 0 {
            ps.lookup_name(ps.selected_char(), 0).unwrap_or(own_name)
        } else {
            own_name
        };
        if !top_name.is_empty() {
            let name_w = font_cache::text_width(top_name) as i32;
            let name_x = NAME_AREA_X + (NAME_AREA_W - name_w) / 2;
            font_cache::draw_text(canvas, gfx, UI_FONT, top_name, name_x, NAME_Y)?;
        }

        Ok(())
    }

    /// Draw the portrait panel: character/look/shop target sprite, rank badge, name, rank text.
    pub(super) fn draw_portrait_panel(
        canvas: &mut Canvas<Window>,
        gfx: &mut GraphicsCache,
        ps: &PlayerState,
    ) -> Result<(), String> {
        let show_shop = ps.should_show_shop();
        let show_look = ps.should_show_look() && !show_shop;

        let (sprite, points, name) = if show_shop {
            let shop = ps.shop_target();
            (
                shop.sprite() as i32,
                shop.points(),
                shop.name().unwrap_or("Unknown").to_string(),
            )
        } else if show_look {
            let look = ps.look_target();
            (
                look.sprite() as i32,
                look.points(),
                look.name().unwrap_or("Unknown").to_string(),
            )
        } else {
            let ci = ps.character_info();
            let center_sprite = ps
                .map()
                .tile_at_xy(TILEX / 2, TILEY / 2)
                .map(|t| t.obj2)
                .unwrap_or(0);
            (
                center_sprite,
                ci.points_tot as u32,
                mag_core::string_operations::c_string_to_str(&ci.name).to_string(),
            )
        };

        let rank_index = Self::points_to_rank_index(points);
        let rank_sprite = 10 + rank_index.min(20) as i32;

        let rank_tex = gfx.get_texture(rank_sprite as usize);
        let rq = rank_tex.query();
        canvas.copy(
            rank_tex,
            None,
            Some(sdl2::rect::Rect::new(463, 38, rq.width, rq.height)),
        )?;

        if sprite > 0 {
            let tex = gfx.get_texture(sprite as usize);
            let q = tex.query();
            canvas.copy(
                tex,
                None,
                Some(sdl2::rect::Rect::new(402, 32, q.width, q.height)),
            )?;
        }

        let center_x = NAME_AREA_X + NAME_AREA_W / 2;
        font_cache::draw_text_centered(canvas, gfx, UI_FONT, &name, center_x, PORTRAIT_NAME_Y)?;
        let rank_name = mag_core::ranks::rank_name(points);
        font_cache::draw_text_centered(canvas, gfx, UI_FONT, rank_name, center_x, PORTRAIT_RANK_Y)?;

        Ok(())
    }

    /// Draw the chat log and input line using bitmap fonts.
    pub(super) fn draw_chat(
        &mut self,
        canvas: &mut Canvas<Window>,
        gfx: &mut GraphicsCache,
        ps: &PlayerState,
    ) -> Result<(), String> {
        let total = ps.log_len();

        // Follow-tail behavior unless manually scrolled: when new messages arrive while scrolled
        // up, keep the current viewport stable by moving the offset with the growth.
        if total > self.last_log_len && self.log_scroll > 0 {
            let delta = total - self.last_log_len;
            self.log_scroll = self.log_scroll.saturating_add(delta);
        }
        self.last_log_len = total;

        // Clamp to valid history range. 0 means newest-at-bottom.
        let max_scroll = total.saturating_sub(LOG_LINES);
        self.log_scroll = self.log_scroll.min(max_scroll);

        // Render fixed lines top->bottom, where bottom is always the newest message at current
        // scroll offset (matches original C client behavior).
        for line in 0..LOG_LINES {
            let idx_from_most_recent = self
                .log_scroll
                .saturating_add(LOG_LINES.saturating_sub(1).saturating_sub(line));

            if let Some(msg) = ps.log_message(idx_from_most_recent) {
                let font = match msg.color {
                    LogMessageColor::Red => 0,
                    LogMessageColor::Yellow => 1,
                    LogMessageColor::Green => 2,
                    LogMessageColor::Blue => 3,
                };
                let y = LOG_Y + (line as i32) * LOG_LINE_H;
                font_cache::draw_text(canvas, gfx, font, &msg.message, LOG_X, y)?;
            }
        }

        // Input line: draw "> " prefix then the current input buffer.
        let input_display = format!("> {}", self.input_buf);
        font_cache::draw_text(canvas, gfx, UI_FONT, &input_display, INPUT_X, INPUT_Y)?;

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

        for n in 0..20usize {
            let sprite = ci.spell[n];
            if sprite <= 0 {
                continue;
            }
            let x = 374 + ((n % 5) as i32) * 24;
            let y = 4 + ((n / 5) as i32) * 24;
            let tex = gfx.get_texture(sprite as usize);
            let q = tex.query();
            canvas.copy(
                tex,
                None,
                Some(sdl2::rect::Rect::new(x, y, q.width, q.height)),
            )?;
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

    /// Render the 4x3 spell-button label grid (lower-right, 610..798 x 504..548).
    pub(super) fn draw_skill_button_labels(
        &self,
        canvas: &mut Canvas<Window>,
        gfx: &mut GraphicsCache,
        ps: &PlayerState,
    ) -> Result<(), String> {
        let pdata = ps.player_data();
        let pending = self.pending_skill_assignment.is_some();

        for row in 0..3usize {
            for col in 0..4usize {
                let idx = row * 4 + col;
                let btn = &pdata.skill_buttons[idx];
                let bx = 612 + (col as i32) * 49;
                let by = 506 + (row as i32) * 15;

                if pending {
                    // Draw subtle highlight to indicate "pick a slot" mode.
                    canvas.set_draw_color(Color::RGBA(255, 200, 80, 60));
                    canvas.set_blend_mode(sdl2::render::BlendMode::Blend);
                    let _ = canvas.fill_rect(sdl2::rect::Rect::new(bx - 2, by - 1, 47, 13));
                    canvas.set_blend_mode(sdl2::render::BlendMode::None);
                }

                if !btn.is_unassigned() {
                    let name = btn.name_str();
                    font_cache::draw_text(canvas, gfx, UI_FONT, &name, bx, by)?;
                }
            }
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
                    let is_blank = self.minimap_xmap[cell] == 0
                        && self.minimap_xmap[cell + 1] == 0
                        && self.minimap_xmap[cell + 2] == 0;
                    // 0xFF marks the player position — always overwrite it.
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

                // Objects override background.
                if tile.obj1 > 0 {
                    let (r, g, b) = gfx.get_avg_color(tile.obj1 as usize);
                    self.minimap_xmap[cell] = r;
                    self.minimap_xmap[cell + 1] = g;
                    self.minimap_xmap[cell + 2] = b;
                    self.minimap_xmap[cell + 3] = 255;
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

    /// Draw the attributes panel, HP/End/Mana raise rows, learned skills list,
    /// stat +/- controls, and the Update button + skill scrollbar.
    pub(super) fn draw_attributes_skills(
        &self,
        canvas: &mut Canvas<Window>,
        gfx: &mut GraphicsCache,
        ps: &PlayerState,
    ) -> Result<(), String> {
        const ATTR_NAMES: [&str; 5] = ["Bravery", "Willpower", "Intuition", "Agility", "Strength"];

        let ci = ps.character_info();
        let available_points = (ci.points - self.stat_points_used).max(0);

        for (n, name) in ATTR_NAMES.iter().enumerate() {
            let y = 4 + (n as i32) * 14;
            let raised = self.stat_raised[n];
            let value_total = ci.attrib[n][5] as i32 + raised;
            let value_bare = ci.attrib[n][0] as i32 + raised;
            let cost = Self::attrib_needed(ci, n, value_bare);
            let line = format!("{:<16}  {:3}", name, value_total);
            font_cache::draw_text(canvas, gfx, UI_FONT, &line, 5, y)?;
            let plus = if cost != i32::MAX && cost <= available_points {
                "+"
            } else {
                ""
            };
            let minus = if raised > 0 { "-" } else { "" };
            font_cache::draw_text(canvas, gfx, UI_FONT, plus, 136, y)?;
            font_cache::draw_text(canvas, gfx, UI_FONT, minus, 150, y)?;
            let cost_text = if cost == i32::MAX {
                String::new()
            } else {
                format!("{:7}", cost)
            };
            font_cache::draw_text(canvas, gfx, UI_FONT, &cost_text, 162, y)?;
        }

        let hp_raised = self.stat_raised[5];
        let end_raised = self.stat_raised[6];
        let mana_raised = self.stat_raised[7];

        let hp_cost = Self::hp_needed(ci, ci.hp[0] as i32 + hp_raised);
        let end_cost = Self::end_needed(ci, ci.end[0] as i32 + end_raised);
        let mana_cost = Self::mana_needed(ci, ci.mana[0] as i32 + mana_raised);

        for (name, value, y, cost, raised) in [
            (
                "Hitpoints",
                ci.hp[5] as i32 + hp_raised,
                74,
                hp_cost,
                hp_raised,
            ),
            (
                "Endurance",
                ci.end[5] as i32 + end_raised,
                88,
                end_cost,
                end_raised,
            ),
            (
                "Mana",
                ci.mana[5] as i32 + mana_raised,
                102,
                mana_cost,
                mana_raised,
            ),
        ] {
            let line = format!("{:<16}  {:3}", name, value);
            font_cache::draw_text(canvas, gfx, UI_FONT, &line, 5, y)?;
            let plus = if cost != i32::MAX && cost <= available_points {
                "+"
            } else {
                ""
            };
            let minus = if raised > 0 { "-" } else { "" };
            font_cache::draw_text(canvas, gfx, UI_FONT, plus, 136, y)?;
            font_cache::draw_text(canvas, gfx, UI_FONT, minus, 150, y)?;
            let cost_text = if cost == i32::MAX {
                String::new()
            } else {
                format!("{:7}", cost)
            };
            font_cache::draw_text(canvas, gfx, UI_FONT, &cost_text, 162, y)?;
        }

        let sorted = Self::sorted_skills(ci);
        for row in 0..10 {
            let y = 116 + (row as i32) * 14;
            let idx = self.skill_scroll + row;
            if let Some(skill_id) = sorted.get(idx).copied() {
                let name = get_skill_name(skill_id);
                if name.is_empty() || ci.skill[skill_id][0] == 0 {
                    continue;
                }

                let raised_idx = 8 + idx;
                if raised_idx >= self.stat_raised.len() {
                    continue;
                }
                let raised = self.stat_raised[raised_idx];
                let value_total = ci.skill[skill_id][5] as i32 + raised;
                let value_bare = ci.skill[skill_id][0] as i32 + raised;
                let cost = Self::skill_needed(ci, skill_id, value_bare);
                let line = format!("{:<16}  {:3}", name, value_total);
                font_cache::draw_text(canvas, gfx, UI_FONT, &line, 5, y)?;
                let plus = if cost != i32::MAX && cost <= available_points {
                    "+"
                } else {
                    ""
                };
                let minus = if raised > 0 { "-" } else { "" };
                font_cache::draw_text(canvas, gfx, UI_FONT, plus, 136, y)?;
                font_cache::draw_text(canvas, gfx, UI_FONT, minus, 150, y)?;
                let cost_text = if cost == i32::MAX {
                    String::new()
                } else {
                    format!("{:7}", cost)
                };
                font_cache::draw_text(canvas, gfx, UI_FONT, &cost_text, 162, y)?;
            }
        }

        let pts = format!("{:7}", (ci.points - self.stat_points_used).max(0));
        font_cache::draw_text(canvas, gfx, UI_FONT, "Update", 117, 256)?;
        font_cache::draw_text(canvas, gfx, UI_FONT, &pts, 162, 256)?;

        // C-style skill scrollbar knob (engine.c: dd_showbar(207,149+(skill_pos*58)/40,11,11,GREEN)).
        let skill_pos = (self.skill_scroll as i32).clamp(0, SKILL_SCROLL_MAX);
        let skill_y = SKILL_SCROLL_Y_BASE + (skill_pos * SKILL_SCROLL_RANGE) / SKILL_SCROLL_MAX;
        canvas.set_draw_color(SCROLL_KNOB_COLOR);
        canvas.fill_rect(sdl2::rect::Rect::new(
            SKILL_SCROLL_X,
            skill_y,
            SCROLL_KNOB_W,
            SCROLL_KNOB_H,
        ))?;

        Ok(())
    }
}
