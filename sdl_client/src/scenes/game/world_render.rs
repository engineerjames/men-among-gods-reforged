use sdl2::{pixels::Color, render::Canvas, video::Window};

use mag_core::constants::{
    CMAGIC, DEATH, DR_DROP, DR_GIVE, DR_PICKUP, DR_USE, EMAGIC, GMAGIC, INJURED, INJURED1,
    INJURED2, INVIS, ISCHAR, ISITEM, ISUSABLE, MF_ARENA, MF_BANK, MF_DEATHTRAP, MF_INDOORS,
    MF_MOVEBLOCK, MF_NOEXPIRE, MF_NOLAG, MF_NOMAGIC, MF_NOMONST, MF_SIGHTBLOCK, MF_TAVERN,
    MF_UWATER, SPR_EMPTY, TILEX, TILEY, TOMB, XPOS, YPOS,
};

use crate::{font_cache, gfx_cache::GraphicsCache, player_state::PlayerState};

use super::{GameScene, MAP_X_SHIFT};

impl GameScene {
    /// Default gamma-based LEFFECT value matching C client: gamma=5000, LEFFECT=gamma-4880=120.
    const LEFFECT: i32 = 120;

    /// Draw a single world sprite at `(tile_x, tile_y)` with camera and sub-tile offsets.
    /// Applies darkness modulation from the tile `light` value.
    pub(super) fn draw_world_sprite(
        canvas: &mut Canvas<Window>,
        gfx: &mut GraphicsCache,
        sprite_id: i32,
        tile_x: usize,
        tile_y: usize,
        cam_xoff: i32,
        cam_yoff: i32,
        xoff: i32,
        yoff: i32,
        light: u8,
    ) -> Result<(), String> {
        if sprite_id <= 0 {
            return Ok(());
        }

        let texture = gfx.get_texture(sprite_id as usize);
        let q = texture.query();
        let xs = q.width as i32 / 32;
        let ys = q.height as i32 / 32;

        let xpos = (tile_x as i32) * 32;
        let ypos = (tile_y as i32) * 32;

        let rx = xpos / 2 + ypos / 2 - xs * 16 + 32 + XPOS + MAP_X_SHIFT + cam_xoff + xoff;
        let ry = xpos / 4 - ypos / 4 + YPOS - ys * 32 + cam_yoff + yoff;

        // Apply darkness modulation from tile light value.
        // C formula: channel = channel * LEFFECT / (darkness² + LEFFECT)
        // Bits 0-3 of light = darkness level, higher bits are special effects (TODO).
        let darkness = (light & 0x0F) as i32;
        if darkness > 0 {
            let factor = (255 * Self::LEFFECT / (darkness * darkness + Self::LEFFECT)) as u8;
            texture.set_color_mod(factor, factor, factor);
        }

        let result = canvas.copy(
            texture,
            None,
            Some(sdl2::rect::Rect::new(rx, ry, q.width, q.height)),
        );

        // Reset color modulation so next draw of this sprite is unaffected.
        if darkness > 0 {
            let texture = gfx.get_texture(sprite_id as usize);
            texture.set_color_mod(255, 255, 255);
        }

        result
    }

    /// Draw a sprite with an additive highlight (used for hover effects).
    pub(super) fn draw_world_sprite_highlight(
        canvas: &mut Canvas<Window>,
        gfx: &mut GraphicsCache,
        sprite_id: i32,
        tile_x: usize,
        tile_y: usize,
        cam_xoff: i32,
        cam_yoff: i32,
        xoff: i32,
        yoff: i32,
        alpha: u8,
    ) -> Result<(), String> {
        if sprite_id <= 0 || sprite_id as u16 == SPR_EMPTY {
            return Ok(());
        }

        let texture = gfx.get_texture(sprite_id as usize);
        let q = texture.query();
        let xs = q.width as i32 / 32;
        let ys = q.height as i32 / 32;

        let xpos = (tile_x as i32) * 32;
        let ypos = (tile_y as i32) * 32;

        let rx = xpos / 2 + ypos / 2 - xs * 16 + 32 + XPOS + MAP_X_SHIFT + cam_xoff + xoff;
        let ry = xpos / 4 - ypos / 4 + YPOS - ys * 32 + cam_yoff + yoff;

        texture.set_blend_mode(sdl2::render::BlendMode::Add);
        texture.set_alpha_mod(alpha);
        let result = canvas.copy(
            texture,
            None,
            Some(sdl2::rect::Rect::new(rx, ry, q.width, q.height)),
        );
        texture.set_alpha_mod(255);
        texture.set_blend_mode(sdl2::render::BlendMode::Blend);
        result
    }

    /// Draw a darkened, vertically-flattened shadow beneath a character sprite.
    /// Ported from dd_shadow() in the original dd.c.
    pub(super) fn draw_shadow(
        canvas: &mut Canvas<Window>,
        gfx: &mut GraphicsCache,
        sprite_id: i32,
        tile_x: usize,
        tile_y: usize,
        cam_xoff: i32,
        cam_yoff: i32,
        xoff: i32,
        yoff: i32,
    ) -> Result<(), String> {
        if sprite_id <= 0 {
            return Ok(());
        }

        // Original dd_shadow only renders shadows for character sprites in these ranges.
        let nr = sprite_id as u32;
        if !((2000..16336).contains(&nr) || nr > 17360) {
            return Ok(());
        }

        let texture = gfx.get_texture(sprite_id as usize);
        let q = texture.query();
        let xs = q.width as i32 / 32;
        let ys = q.height as i32 / 32;

        let xpos = (tile_x as i32) * 32;
        let ypos = (tile_y as i32) * 32;

        let rx = xpos / 2 + ypos / 2 - xs * 16 + 32 + XPOS + MAP_X_SHIFT + cam_xoff + xoff;
        let ry = xpos / 4 - ypos / 4 + YPOS - ys * 32 + cam_yoff + yoff;

        // Shadow is placed at character's feet, flattened to 1/4 height.
        // disp=14 matches the original C code.
        let shadow_ry = ry + ys * 32 - 14;
        let shadow_h = (q.height / 4).max(1);

        // Darken: black tint with partial alpha to simulate the v >>= 1 pixel halving.
        texture.set_color_mod(0, 0, 0);
        texture.set_alpha_mod(80);
        texture.set_blend_mode(sdl2::render::BlendMode::Blend);

        let result = canvas.copy_ex(
            texture,
            None,
            Some(sdl2::rect::Rect::new(rx, shadow_ry, q.width, shadow_h)),
            0.0,
            None,
            false,
            true,
        );

        // Reset texture state.
        texture.set_color_mod(255, 255, 255);
        texture.set_alpha_mod(255);

        result
    }

    /// Draw a diamond-shaped magic glow effect over a tile.
    /// Ported from dd_alphaeffect_magic_0() in the original dd.c.
    ///
    /// `alpha_mask`: bitmask of active channels (bit0=R/electric, bit1=G/green, bit2=B/cold).
    /// `strength`: intensity divider (higher = weaker glow), extracted from flag bits.
    pub(super) fn draw_magic_effect(
        canvas: &mut Canvas<Window>,
        alpha_mask: u32,
        strength: u32,
        tile_x: usize,
        tile_y: usize,
        cam_xoff: i32,
        cam_yoff: i32,
        xoff: i32,
        yoff: i32,
    ) -> Result<(), String> {
        // Isometric projection for a 2×2 tile area (64×64 pixels), matching dd_alphaeffect_magic.
        let xpos = (tile_x as i32) * 32;
        let ypos = (tile_y as i32) * 32;
        let rx = xpos / 2 + ypos / 2 - 2 * 16 + 32 + XPOS + MAP_X_SHIFT + cam_xoff + xoff;
        let ry = xpos / 4 - ypos / 4 + YPOS - 2 * 32 + cam_yoff + yoff;

        let str_div = strength.max(1) as i32;

        // Draw the diamond glow as a series of horizontal lines using additive blending.
        // This avoids needing a streaming texture while closely matching the original effect.
        canvas.set_blend_mode(sdl2::render::BlendMode::Add);

        for y in 0..64i32 {
            let py = ry + y;
            if !(0..600).contains(&py) {
                continue;
            }

            for x in 0..64i32 {
                let px = rx + x;
                if !(0..800).contains(&px) {
                    continue;
                }

                // Diamond envelope — same formula as the original C code.
                let mut e: i32 = 32;
                if x < 32 {
                    e -= 32 - x;
                }
                if x > 31 {
                    e -= x - 31;
                }
                if y < 16 {
                    e -= 16 - y;
                }
                if y > 55 {
                    e -= (y - 55) * 2;
                }
                if e <= 0 {
                    continue;
                }
                e /= str_div;
                if e <= 0 {
                    continue;
                }

                // Scale to 0–255 range (original works with 0–31 in RGB565, ×8 ≈ 0–255).
                let r = if (alpha_mask & 1) != 0 {
                    (e * 8).min(255) as u8
                } else {
                    0u8
                };
                let g = if (alpha_mask & 2) != 0 {
                    (e * 8).min(255) as u8
                } else {
                    0u8
                };
                let b = if (alpha_mask & 4) != 0 {
                    (e * 8).min(255) as u8
                } else {
                    0u8
                };

                canvas.set_draw_color(Color::RGBA(r, g, b, 255));
                let _ = canvas.draw_point(sdl2::rect::Point::new(px, py));
            }
        }

        canvas.set_blend_mode(sdl2::render::BlendMode::None);
        Ok(())
    }

    /// Render all world tiles in two painter-order passes (backgrounds, then
    /// objects/characters/effects). This is the main world-drawing entry point.
    pub(super) fn draw_world(
        canvas: &mut Canvas<Window>,
        gfx: &mut GraphicsCache,
        ps: &PlayerState,
        shadows_enabled: bool,
        spell_effects_enabled: bool,
    ) -> Result<(), String> {
        let map = ps.map();
        let ci = ps.character_info();
        let pdata = ps.player_data();
        let show_names = pdata.show_names != 0;
        let show_proz = pdata.show_proz != 0;
        let (cam_xoff, cam_yoff) = Self::camera_offsets(ps);

        // Pass 1: Background / terrain sprites (legacy eng_display order: y descending).
        for y in (0..TILEY).rev() {
            for x in 0..TILEX {
                let Some(tile) = map.tile_at_xy(x, y) else {
                    continue;
                };

                let ba = if tile.back > 0 {
                    tile.back as i16
                } else {
                    tile.ba_sprite
                };

                let ba = if (tile.flags & INVIS) != 0 {
                    SPR_EMPTY as i16
                } else {
                    ba
                };

                if ba <= 0 {
                    continue;
                }

                Self::draw_world_sprite(
                    canvas, gfx, ba as i32, x, y, cam_xoff, cam_yoff, 0, 0, tile.light,
                )?;

                if ci.goto_x == tile.x as i32 && ci.goto_y == tile.y as i32 {
                    Self::draw_world_sprite(
                        canvas, gfx, 31, x, y, cam_xoff, cam_yoff, 0, 0, tile.light,
                    )?;
                }
            }
        }

        // Pass 2: Objects/characters/markers/effects (legacy eng_display order: y descending).
        for y in (0..TILEY).rev() {
            for x in 0..TILEX {
                let Some(tile) = map.tile_at_xy(x, y) else {
                    continue;
                };

                if (tile.flags & INVIS) != 0 {
                    continue;
                }

                let xpos = (x as i32) * 32;
                let ypos = (y as i32) * 32;
                let ch_xoff = tile.obj_xoff;
                let ch_yoff = tile.obj_yoff;

                let mut obj = tile.obj1;
                if obj > 0 {
                    let hide_enabled = pdata.hide != 0;
                    let is_item = (tile.flags & ISITEM) != 0;

                    if hide_enabled && !is_item && !Self::autohide(x, y) {
                        let is_mine_wall = obj > 16335
                            && obj < 16422
                            && !matches!(
                                obj,
                                16357
                                    | 16365
                                    | 16373
                                    | 16381
                                    | 16389
                                    | 16397
                                    | 16405
                                    | 16413
                                    | 16421
                            )
                            && !Self::facing(x, y, ci.dir);

                        if is_mine_wall {
                            obj = if obj < 16358 {
                                457
                            } else if obj < 16366 {
                                456
                            } else if obj < 16374 {
                                455
                            } else if obj < 16382 {
                                466
                            } else if obj < 16390 {
                                459
                            } else if obj < 16398 {
                                458
                            } else if obj < 16406 {
                                468
                            } else {
                                467
                            };
                        } else {
                            obj += 1;
                        }
                    }

                    Self::draw_world_sprite(
                        canvas, gfx, obj, x, y, cam_xoff, cam_yoff, 0, 0, tile.light,
                    )?;
                }

                // Shadow (before character sprite, matching engine.c line 789).
                if shadows_enabled {
                    Self::draw_shadow(
                        canvas,
                        gfx,
                        tile.obj2,
                        x,
                        y,
                        cam_xoff,
                        cam_yoff,
                        ch_xoff,
                        ch_yoff + 4,
                    )?;
                }

                let ch = if tile.obj2 > 0 {
                    tile.obj2
                } else {
                    tile.ch_sprite as i32
                };
                Self::draw_world_sprite(
                    canvas, gfx, ch, x, y, cam_xoff, cam_yoff, ch_xoff, ch_yoff, tile.light,
                )?;

                if ci.attack_cn != 0 && ci.attack_cn == tile.ch_nr as i32 {
                    Self::draw_world_sprite(
                        canvas, gfx, 34, x, y, cam_xoff, cam_yoff, ch_xoff, ch_yoff, tile.light,
                    )?;
                }

                if ci.misc_action == DR_GIVE as i32 && ci.misc_target1 == tile.ch_id as i32 {
                    Self::draw_world_sprite(
                        canvas, gfx, 45, x, y, cam_xoff, cam_yoff, ch_xoff, ch_yoff, tile.light,
                    )?;
                }

                // Nameplate (same pass as character sprites in engine.c).
                if tile.ch_nr != 0 && (show_names || show_proz) {
                    let is_center = x == TILEX / 2 && y == TILEY / 2;

                    let name: Option<String> = if show_names {
                        if is_center {
                            let own = mag_core::string_operations::c_string_to_str(
                                &ps.character_info().name,
                            );
                            if !own.is_empty() {
                                Some(own.to_string())
                            } else {
                                None
                            }
                        } else {
                            ps.lookup_name(tile.ch_nr, tile.ch_id)
                                .map(|s| s.to_string())
                        }
                    } else {
                        None
                    };

                    let proz: Option<u8> = if show_proz && tile.ch_proz != 0 {
                        Some(tile.ch_proz)
                    } else {
                        None
                    };

                    let text = match (show_names, show_proz, name.as_deref(), proz) {
                        (true, true, Some(n), Some(p)) if !n.is_empty() => {
                            format!("{} {}%", n, p)
                        }
                        (true, true, _, Some(p)) => format!("{}%", p),
                        (true, _, Some(n), _) if !n.is_empty() => n.to_string(),
                        (false, true, _, Some(p)) => format!("{}%", p),
                        _ => String::new(),
                    };

                    if !text.is_empty() {
                        // dd_gputtext formula (ported from engine.c + nameplates.rs):
                        // horizontally centered, shifted 64px up relative to sprite origin.
                        let text_len = text.len() as i32;
                        let np_rx = xpos / 2 + ypos / 2 + 32 - (text_len * 5 / 2)
                            + XPOS
                            + MAP_X_SHIFT
                            + cam_xoff
                            + ch_xoff;
                        let np_ry = xpos / 4 - ypos / 4 + YPOS - 64 + cam_yoff + ch_yoff;
                        font_cache::draw_text_tinted(
                            canvas,
                            gfx,
                            1,
                            &text,
                            np_rx + 1,
                            np_ry + 1,
                            Color::RGB(0, 0, 0),
                        )?;
                        font_cache::draw_text(canvas, gfx, 1, &text, np_rx, np_ry)?;
                    }
                }

                if ci.misc_action == DR_DROP as i32
                    && ci.misc_target1 == tile.x as i32
                    && ci.misc_target2 == tile.y as i32
                {
                    Self::draw_world_sprite(
                        canvas, gfx, 32, x, y, cam_xoff, cam_yoff, 0, 0, tile.light,
                    )?;
                }
                if ci.misc_action == DR_PICKUP as i32
                    && ci.misc_target1 == tile.x as i32
                    && ci.misc_target2 == tile.y as i32
                {
                    Self::draw_world_sprite(
                        canvas, gfx, 33, x, y, cam_xoff, cam_yoff, 0, 0, tile.light,
                    )?;
                }
                if ci.misc_action == DR_USE as i32
                    && ci.misc_target1 == tile.x as i32
                    && ci.misc_target2 == tile.y as i32
                {
                    Self::draw_world_sprite(
                        canvas, gfx, 45, x, y, cam_xoff, cam_yoff, 0, 0, tile.light,
                    )?;
                }

                if (tile.flags2 & MF_MOVEBLOCK) != 0 {
                    Self::draw_world_sprite(
                        canvas, gfx, 55, x, y, cam_xoff, cam_yoff, 0, 0, tile.light,
                    )?;
                }
                if (tile.flags2 & MF_SIGHTBLOCK) != 0 {
                    Self::draw_world_sprite(
                        canvas, gfx, 84, x, y, cam_xoff, cam_yoff, 0, 0, tile.light,
                    )?;
                }
                if (tile.flags2 & MF_INDOORS) != 0 {
                    Self::draw_world_sprite(
                        canvas, gfx, 56, x, y, cam_xoff, cam_yoff, 0, 0, tile.light,
                    )?;
                }
                if (tile.flags2 & MF_UWATER) != 0 {
                    Self::draw_world_sprite(
                        canvas, gfx, 75, x, y, cam_xoff, cam_yoff, 0, 0, tile.light,
                    )?;
                }
                if (tile.flags2 & MF_NOMONST) != 0 {
                    Self::draw_world_sprite(
                        canvas, gfx, 59, x, y, cam_xoff, cam_yoff, 0, 0, tile.light,
                    )?;
                }
                if (tile.flags2 & MF_BANK) != 0 {
                    Self::draw_world_sprite(
                        canvas, gfx, 60, x, y, cam_xoff, cam_yoff, 0, 0, tile.light,
                    )?;
                }
                if (tile.flags2 & MF_TAVERN) != 0 {
                    Self::draw_world_sprite(
                        canvas, gfx, 61, x, y, cam_xoff, cam_yoff, 0, 0, tile.light,
                    )?;
                }
                if (tile.flags2 & MF_NOMAGIC) != 0 {
                    Self::draw_world_sprite(
                        canvas, gfx, 62, x, y, cam_xoff, cam_yoff, 0, 0, tile.light,
                    )?;
                }
                if (tile.flags2 & MF_DEATHTRAP) != 0 {
                    Self::draw_world_sprite(
                        canvas, gfx, 73, x, y, cam_xoff, cam_yoff, 0, 0, tile.light,
                    )?;
                }
                if (tile.flags2 & MF_NOLAG) != 0 {
                    Self::draw_world_sprite(
                        canvas, gfx, 57, x, y, cam_xoff, cam_yoff, 0, 0, tile.light,
                    )?;
                }
                if (tile.flags2 & MF_ARENA) != 0 {
                    Self::draw_world_sprite(
                        canvas, gfx, 76, x, y, cam_xoff, cam_yoff, 0, 0, tile.light,
                    )?;
                }
                if (tile.flags2 & MF_NOEXPIRE) != 0 {
                    Self::draw_world_sprite(
                        canvas, gfx, 82, x, y, cam_xoff, cam_yoff, 0, 0, tile.light,
                    )?;
                }
                if (tile.flags2 & 0x8000_0000) != 0 {
                    Self::draw_world_sprite(
                        canvas, gfx, 72, x, y, cam_xoff, cam_yoff, 0, 0, tile.light,
                    )?;
                }

                let injured_mask = tile.flags & (INJURED | INJURED1 | INJURED2);
                if injured_mask == INJURED {
                    Self::draw_world_sprite(
                        canvas, gfx, 1079, x, y, cam_xoff, cam_yoff, ch_xoff, ch_yoff, tile.light,
                    )?;
                }
                if injured_mask == (INJURED | INJURED1) {
                    Self::draw_world_sprite(
                        canvas, gfx, 1080, x, y, cam_xoff, cam_yoff, ch_xoff, ch_yoff, tile.light,
                    )?;
                }
                if injured_mask == (INJURED | INJURED2) {
                    Self::draw_world_sprite(
                        canvas, gfx, 1081, x, y, cam_xoff, cam_yoff, ch_xoff, ch_yoff, tile.light,
                    )?;
                }
                if injured_mask == (INJURED | INJURED1 | INJURED2) {
                    Self::draw_world_sprite(
                        canvas, gfx, 1082, x, y, cam_xoff, cam_yoff, ch_xoff, ch_yoff, tile.light,
                    )?;
                }

                if (tile.flags & DEATH) != 0 {
                    let death_variant = ((tile.flags & DEATH) >> 17) as i32;
                    if death_variant > 0 {
                        let sprite = 280 + death_variant - 1;
                        if tile.obj2 != 0 {
                            Self::draw_world_sprite(
                                canvas, gfx, sprite, x, y, cam_xoff, cam_yoff, ch_xoff, ch_yoff,
                                tile.light,
                            )?;
                        } else {
                            Self::draw_world_sprite(
                                canvas, gfx, sprite, x, y, cam_xoff, cam_yoff, 0, 0, tile.light,
                            )?;
                        }
                    }
                }

                if (tile.flags & TOMB) != 0 {
                    let tomb_variant = ((tile.flags & TOMB) >> 12) as i32;
                    if tomb_variant > 0 {
                        let sprite = 240 + tomb_variant - 1;
                        Self::draw_world_sprite(
                            canvas, gfx, sprite, x, y, cam_xoff, cam_yoff, 0, 0, tile.light,
                        )?;
                    }
                }

                // Magic spell effects (EMAGIC/GMAGIC/CMAGIC diamond glows).
                // Matches engine.c lines 846–860.
                if spell_effects_enabled {
                    let mut alpha_mask = 0u32;
                    let mut alphastr = 0u32;
                    if (tile.flags & EMAGIC) != 0 {
                        alpha_mask |= 1;
                        alphastr = alphastr.max((tile.flags & EMAGIC) >> 22);
                    }
                    if (tile.flags & GMAGIC) != 0 {
                        alpha_mask |= 2;
                        alphastr = alphastr.max((tile.flags & GMAGIC) >> 25);
                    }
                    if (tile.flags & CMAGIC) != 0 {
                        alpha_mask |= 4;
                        alphastr = alphastr.max((tile.flags & CMAGIC) >> 28);
                    }
                    if alpha_mask != 0 {
                        Self::draw_magic_effect(
                            canvas, alpha_mask, alphastr, x, y, cam_xoff, cam_yoff, ch_xoff,
                            ch_yoff,
                        )?;
                    }
                }
            }
        }

        Ok(())
    }

    /// Draw hover highlights on world tiles when the cursor is over the map area.
    /// Ctrl/Alt = character targeting, Shift = item targeting, default = floor tile.
    pub(super) fn draw_hover_effects(
        &self,
        canvas: &mut Canvas<Window>,
        gfx: &mut GraphicsCache,
        ps: &PlayerState,
    ) -> Result<(), String> {
        if ps.should_show_shop() {
            return Ok(());
        }

        if !Self::cursor_in_map_interaction_area(self.mouse_x, self.mouse_y) {
            return Ok(());
        }

        canvas.set_draw_color(Color::RGB(255, 170, 80));

        // Hover highlight on world tiles: brighten the underlying sprite(s)
        // instead of drawing an overlay shape.
        let (cam_xoff, cam_yoff) = Self::camera_offsets(ps);
        if let Some((mx, my)) =
            Self::screen_to_map_tile(self.mouse_x, self.mouse_y, cam_xoff, cam_yoff)
        {
            if !(3..=TILEX - 7).contains(&mx) || !(7..=TILEY - 3).contains(&my) {
                return Ok(());
            }

            if self.ctrl_held || self.alt_held {
                // Ctrl/Alt targeting: highlight nearest character sprite (C mouse_mapbox behavior).
                if let Some((sx, sy)) = Self::nearest_tile_with_flag(ps, mx, my, ISCHAR) {
                    if !(3..=TILEX - 7).contains(&sx) || !(7..=TILEY - 3).contains(&sy) {
                        return Ok(());
                    }

                    if let Some(tile) = ps.map().tile_at_xy(sx, sy) {
                        if (tile.flags & INVIS) != 0 {
                            return Ok(());
                        }

                        if tile.obj2 > 0 {
                            Self::draw_world_sprite_highlight(
                                canvas,
                                gfx,
                                tile.obj2,
                                sx,
                                sy,
                                cam_xoff,
                                cam_yoff,
                                tile.obj_xoff,
                                tile.obj_yoff,
                                140,
                            )?;
                        }
                    }
                }
            } else if self.shift_held {
                // Shift held: only highlight nearby ISITEM tiles (use/pickup targeting).
                if let Some((sx, sy)) = Self::nearest_tile_with_flag(ps, mx, my, ISITEM) {
                    if !(3..=TILEX - 7).contains(&sx) || !(7..=TILEY - 3).contains(&sy) {
                        return Ok(());
                    }

                    if let Some(tile) = ps.map().tile_at_xy(sx, sy) {
                        if (tile.flags & INVIS) != 0 {
                            return Ok(());
                        }

                        let highlight_obj = if tile.obj1 > 0 {
                            tile.obj1
                        } else if tile.it_sprite > 0 {
                            tile.it_sprite as i32
                        } else if tile.back > 0 {
                            tile.back
                        } else {
                            tile.ba_sprite as i32
                        };
                        let strength = if (tile.flags & ISUSABLE) != 0 {
                            150
                        } else {
                            120
                        };
                        Self::draw_world_sprite_highlight(
                            canvas,
                            gfx,
                            highlight_obj,
                            sx,
                            sy,
                            cam_xoff,
                            cam_yoff,
                            0,
                            0,
                            strength,
                        )?;
                    }
                }
            } else {
                // Normal movement hover: brighten the floor tile being targeted.
                if let Some(tile) = ps.map().tile_at_xy(mx, my) {
                    if (tile.flags & INVIS) != 0 {
                        return Ok(());
                    }

                    let floor_sprite = if tile.back > 0 {
                        tile.back
                    } else {
                        tile.ba_sprite as i32
                    };
                    Self::draw_world_sprite_highlight(
                        canvas,
                        gfx,
                        floor_sprite,
                        mx,
                        my,
                        cam_xoff,
                        cam_yoff,
                        0,
                        0,
                        96,
                    )?;
                }
            }
            canvas.set_draw_color(Color::RGB(255, 170, 80));
        }

        Ok(())
    }
}
