use std::cmp::Ordering;
use std::time::Duration;

use egui_sdl2::egui;
use sdl2::{
    event::Event,
    keyboard::{Keycode, Mod},
    mouse::MouseButton,
    pixels::Color,
    render::Canvas,
    video::Window,
};

use mag_core::constants::{
    DEATH, DR_DROP, DR_GIVE, DR_PICKUP, DR_USE, INJURED, INJURED1, INJURED2, INVIS, ISCHAR, ISITEM,
    ISUSABLE, MF_ARENA, MF_BANK, MF_DEATHTRAP, MF_INDOORS, MF_MOVEBLOCK, MF_NOEXPIRE, MF_NOLAG,
    MF_NOMAGIC, MF_NOMONST, MF_SIGHTBLOCK, MF_TAVERN, MF_UWATER, SPR_EMPTY, TILEX, TILEY, TOMB,
    XPOS, YPOS,
};
use mag_core::types::skilltab::{get_skill_name, get_skill_nr, get_skill_sortkey, MAX_SKILLS};

use crate::{
    font_cache,
    gfx_cache::GraphicsCache,
    network::{client_commands::ClientCommand, NetworkEvent, NetworkRuntime},
    player_state::PlayerState,
    scenes::scene::{Scene, SceneType},
    state::AppState,
    types::log_message::LogMessageColor,
};

/// Maximum characters allowed in the chat input buffer.
const MAX_INPUT_LEN: usize = 120;

/// Maximum complete network tick groups processed per frame.
///
/// A tick group is all `NetworkEvent::Bytes` emitted for one server tick packet,
/// followed by its terminating `NetworkEvent::Tick`. We only stop processing at
/// tick boundaries so map state is never rendered from a partially applied group.
const MAX_TICK_GROUPS_PER_FRAME: usize = 32;

// ---- Layout constants (ported from engine.c / layout.rs) ---- //

/// Camera X shift to account for the left-hand UI panel.
const MAP_X_SHIFT: i32 = -176;

/// Sprite ID of the static 800x600 UI background frame.
const UI_FRAME_SPRITE: usize = 1;

/// Default bitmap font index (yellow, sprite 701).
const UI_FONT: usize = 1;

// Matches original engine.c worn-slot draw order (wntab[]), and the Bevy client's
// EQUIP_WNTAB mapping.
const EQUIP_WNTAB: [usize; 12] = [0, 9, 2, 3, 1, 4, 8, 7, 10, 11, 5, 6];

// HP / Endurance / Mana bars
const BAR_X: i32 = 373;
const BAR_HP_Y: i32 = 127;
const BAR_END_Y: i32 = 134;
const BAR_MANA_Y: i32 = 141;
const BAR_H: u32 = 6;
const BAR_SCALE_NUM: i32 = 62;
const BAR_W_MAX: i32 = 124;

/// Bar background (capacity).
const BAR_BG_COLOR: Color = Color::RGB(9, 4, 58);
/// Bar fill (own character).
const BAR_FILL_COLOR: Color = Color::RGB(8, 77, 23);
const BAR_FILL_LOOK_COLOR: Color = Color::RGB(140, 20, 20);
const MODE_INDICATOR_COLOR: Color = Color::RGB(200, 96, 24);

// Stat text positions
const STAT_HP_X: i32 = 5;
const STAT_HP_Y: i32 = 270;
const STAT_END_X: i32 = 5;
const STAT_END_Y: i32 = 284;
const STAT_MANA_X: i32 = 5;
const STAT_MANA_Y: i32 = 298;
const STAT_MONEY_X: i32 = 375;
const STAT_MONEY_Y: i32 = 190;
const STAT_WEAPON_X: i32 = 646;
const STAT_WEAPON_Y: i32 = 243;
const STAT_ARMOR_X: i32 = 646;
const STAT_ARMOR_Y: i32 = 257;
const STAT_EXP_X: i32 = 646;
const STAT_EXP_Y: i32 = 271;

// Name text (centered in 125px wide area)
const NAME_AREA_X: i32 = 374;
const NAME_AREA_W: i32 = 125;
const NAME_Y: i32 = 28;
const PORTRAIT_NAME_Y: i32 = 152;
const PORTRAIT_RANK_Y: i32 = 172;

// Chat log area
const LOG_X: i32 = 500;
const LOG_Y: i32 = 4;
const LOG_LINE_H: i32 = 10;
const LOG_LINES: usize = 22;
const INPUT_X: i32 = 500;
const INPUT_Y: i32 = 9 + LOG_LINE_H * (LOG_LINES as i32);

// Minimap
const MINIMAP_X: i32 = 3;
const MINIMAP_Y: i32 = 471;
const MINIMAP_VIEW_SIZE: u32 = 128;
const MINIMAP_WORLD_SIZE: usize = 1024;

pub struct GameScene {
    input_buf: String,
    pending_exit: Option<String>,
    log_scroll: usize,
    ctrl_held: bool,
    shift_held: bool,
    alt_held: bool,
    skill_scroll: usize,
    inv_scroll: usize,
    mouse_x: i32,
    mouse_y: i32,
    /// Pending stat raises not yet committed to the server (indices 0-7 = attrib/HP/End/Mana,
    /// 8-107 = sorted skill positions). Mirrors statbox.stat_raised in the Bevy client.
    stat_raised: [i32; 108],
    /// Points already spent on pending raises (sum of costs for each stat_raised[n]).
    stat_points_used: i32,
    /// Persistent 1024×1024 world map for minimap rendering.
    /// Layout: 4 bytes per cell [R,G,B,A], cell index = (gy + gx * 1024) * 4.
    /// This matches the C xmap column-major storage: `xmap[map[m].y + map[m].x*1024]`.
    minimap_xmap: Vec<u8>,
    minimap_last_xy: Option<(u16, u16)>,
}

impl GameScene {
    pub fn new() -> Self {
        Self {
            input_buf: String::new(),
            pending_exit: None,
            log_scroll: 0,
            ctrl_held: false,
            shift_held: false,
            alt_held: false,
            skill_scroll: 0,
            inv_scroll: 0,
            mouse_x: 0,
            mouse_y: 0,
            stat_raised: [0; 108],
            stat_points_used: 0,
            minimap_xmap: vec![0u8; MINIMAP_WORLD_SIZE * MINIMAP_WORLD_SIZE * 4],
            minimap_last_xy: None,
        }
    }

    fn autohide(x: usize, y: usize) -> bool {
        !(x >= (TILEX / 2) || y <= (TILEX / 2))
    }

    fn facing(x: usize, y: usize, dir: i32) -> bool {
        (dir == 1 && x == TILEX / 2 + 1 && y == TILEY / 2)
            || (dir == 2 && x == TILEX / 2 - 1 && y == TILEY / 2)
            || (dir == 4 && x == TILEX / 2 && y == TILEY / 2 + 1)
            || (dir == 3 && x == TILEX / 2 && y == TILEY / 2 - 1)
    }

    /// Default gamma-based LEFFECT value matching C client: gamma=5000, LEFFECT=gamma-4880=120.
    const LEFFECT: i32 = 120;

    fn draw_world_sprite(
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

    fn tile_ground_diamond_origin(
        tile_x: usize,
        tile_y: usize,
        cam_xoff: i32,
        cam_yoff: i32,
    ) -> (i32, i32) {
        let xpos = (tile_x as i32) * 32;
        let ypos = (tile_y as i32) * 32;
        let cx = xpos / 2 + ypos / 2 + 32 + XPOS + MAP_X_SHIFT + cam_xoff;
        let cy = xpos / 4 - ypos / 4 + YPOS - 16 + cam_yoff;
        (cx, cy)
    }

    fn camera_offsets(ps: &PlayerState) -> (i32, i32) {
        let map = ps.map();
        if let Some(center) = map.tile_at_xy(TILEX / 2, TILEY / 2) {
            (-center.obj_xoff, -center.obj_yoff)
        } else {
            (0, 0)
        }
    }

    fn draw_hover_tile_diamond(
        canvas: &mut Canvas<Window>,
        cx: i32,
        cy: i32,
        color: Color,
    ) -> Result<(), String> {
        canvas.set_blend_mode(sdl2::render::BlendMode::Blend);
        canvas.set_draw_color(color);

        // Fill diamond with horizontal scanlines:
        // top=(cx,cy), right=(cx+16,cy+8), bottom=(cx,cy+16), left=(cx-16,cy+8)
        for y in cy..=cy + 16 {
            let (x1, x2) = if y <= cy + 8 {
                let t = (y - cy) as f32 / 8.0;
                let left = (cx as f32 - 16.0 * t).round() as i32;
                let right = (cx as f32 + 16.0 * t).round() as i32;
                (left, right)
            } else {
                let t = (y - (cy + 8)) as f32 / 8.0;
                let left = ((cx - 16) as f32 + 16.0 * t).round() as i32;
                let right = ((cx + 16) as f32 - 16.0 * t).round() as i32;
                (left, right)
            };

            canvas.draw_line(sdl2::rect::Point::new(x1, y), sdl2::rect::Point::new(x2, y))?;
        }

        canvas.set_blend_mode(sdl2::render::BlendMode::None);
        Ok(())
    }

    fn process_network_events(&mut self, app_state: &mut AppState) -> Option<SceneType> {
        let mut tick_groups_processed = 0usize;

        loop {
            if tick_groups_processed >= MAX_TICK_GROUPS_PER_FRAME {
                break;
            }

            let Some(net) = app_state.network.as_mut() else {
                break;
            };
            let Some(evt) = net.try_recv() else {
                break;
            };

            match evt {
                NetworkEvent::Status(msg) => {
                    log::info!("Network status: {}", msg);
                }
                NetworkEvent::Error(e) => {
                    log::error!("Network error: {}", e);
                    self.pending_exit = Some(e);
                }
                NetworkEvent::LoggedIn => {
                    if let Some(net) = app_state.network.as_mut() {
                        net.logged_in = true;
                    }
                    log::info!("Logged in to game server");
                }
                NetworkEvent::NewPlayerCredentials {
                    user_id,
                    pass1,
                    pass2,
                } => {
                    if let Some(ps) = app_state.player_state.as_mut() {
                        let save = ps.save_file_mut();
                        save.usnr = user_id;
                        save.pass1 = pass1;
                        save.pass2 = pass2;
                    }
                }
                NetworkEvent::Bytes { bytes, received_at } => {
                    if bytes.is_empty() {
                        continue;
                    }

                    use crate::network::server_commands::{ServerCommand, ServerCommandData};

                    if let Some(cmd) = ServerCommand::from_bytes(&bytes) {
                        match &cmd.structured_data {
                            ServerCommandData::Pong { seq, .. } => {
                                if let Some(net) = app_state.network.as_mut() {
                                    net.handle_pong(*seq, received_at);
                                }
                            }
                            ServerCommandData::PlaySound { nr, vol, pan } => {
                                log::info!("PlaySound: nr={} vol={} pan={}", nr, vol, pan);
                                app_state.sfx_cache.play_sfx(*nr as usize, *vol, *pan);
                            }
                            _ => {
                                if let Some(ps) = app_state.player_state.as_mut() {
                                    ps.update_from_server_command(&cmd);
                                }
                            }
                        }
                    }
                }
                NetworkEvent::Tick => {
                    if let Some(net) = app_state.network.as_mut() {
                        net.client_ticker = net.client_ticker.wrapping_add(1);
                        let ticker = net.client_ticker;
                        if let Some(ps) = app_state.player_state.as_mut() {
                            ps.on_tick_packet(ticker);
                            ps.map_mut().reset_last_setmap_index();
                        }
                        net.maybe_send_ctick();
                        net.maybe_send_ping();
                    }
                    tick_groups_processed += 1;
                }
            }
        }

        if let Some(ps) = app_state.player_state.as_mut() {
            if ps.take_exit_requested_reason().is_some() {
                return Some(SceneType::CharacterSelection);
            }
        }

        if self.pending_exit.take().is_some() {
            return Some(SceneType::CharacterSelection);
        }

        None
    }

    fn draw_world(
        canvas: &mut Canvas<Window>,
        gfx: &mut GraphicsCache,
        ps: &PlayerState,
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
                if ba <= 0 || ba as u16 == SPR_EMPTY {
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
            }
        }

        Ok(())
    }

    /// Draw the static UI frame (sprite 1) at (0,0), covering the full 800x600 window.
    fn draw_ui_frame(canvas: &mut Canvas<Window>, gfx: &mut GraphicsCache) -> Result<(), String> {
        let texture = gfx.get_texture(UI_FRAME_SPRITE);
        let q = texture.query();
        canvas.copy(
            texture,
            None,
            Some(sdl2::rect::Rect::new(0, 0, q.width, q.height)),
        )
    }

    /// Draw HP/End/Mana bars at the classic engine.c positions.
    fn draw_bars(canvas: &mut Canvas<Window>, ps: &PlayerState) -> Result<(), String> {
        let ci = ps.character_info();

        let hp = ci.a_hp;
        let max_hp = ci.hp[5] as i32;
        let end_val = ci.a_end;
        let max_end = ci.end[5] as i32;
        let mana = ci.a_mana;
        let max_mana = ci.mana[5] as i32;

        // dd_showbar(373, y, n, 6, color) — bar_width = (cur * 62) / max, clamped [0,124]
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
    fn draw_stat_text(
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

        // Character name (centered in 125px area at top)
        let name = mag_core::string_operations::c_string_to_str(&ci.name);
        if !name.is_empty() {
            let name_w = font_cache::text_width(name) as i32;
            let name_x = NAME_AREA_X + (NAME_AREA_W - name_w) / 2;
            font_cache::draw_text(canvas, gfx, UI_FONT, name, name_x, NAME_Y)?;

            // Portrait name + rank (below portrait area)
            let center_x = NAME_AREA_X + NAME_AREA_W / 2;
            font_cache::draw_text_centered(canvas, gfx, UI_FONT, name, center_x, PORTRAIT_NAME_Y)?;

            let rank_name = mag_core::ranks::rank_name(ci.points_tot as u32);
            font_cache::draw_text_centered(
                canvas,
                gfx,
                UI_FONT,
                rank_name,
                center_x,
                PORTRAIT_RANK_Y,
            )?;
        }

        Ok(())
    }

    /// Draw the chat log and input line using bitmap fonts.
    fn draw_chat(
        &self,
        canvas: &mut Canvas<Window>,
        gfx: &mut GraphicsCache,
        ps: &PlayerState,
    ) -> Result<(), String> {
        let total = ps.log_len();

        // Determine visible window: newest messages at the bottom, scroll moves the window up.
        let end = total.saturating_sub(self.log_scroll);
        let start = end.saturating_sub(LOG_LINES);

        for (i, log_idx) in (start..end).rev().enumerate() {
            if let Some(msg) = ps.log_message(log_idx) {
                let font = match msg.color {
                    LogMessageColor::Red => 0,
                    LogMessageColor::Yellow => 1,
                    LogMessageColor::Green => 2,
                    LogMessageColor::Blue => 3,
                };
                let y = LOG_Y + (i as i32) * LOG_LINE_H;
                font_cache::draw_text(canvas, gfx, font, &msg.message, LOG_X, y)?;
            }
        }

        // Input line: draw "> " prefix then the current input buffer.
        let input_display = format!("> {}", self.input_buf);
        font_cache::draw_text(canvas, gfx, UI_FONT, &input_display, INPUT_X, INPUT_Y)?;

        Ok(())
    }

    fn draw_mode_indicators(canvas: &mut Canvas<Window>, ps: &PlayerState) -> Result<(), String> {
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

    fn draw_inventory_equipment_spells(
        &self,
        canvas: &mut Canvas<Window>,
        gfx: &mut GraphicsCache,
        ps: &PlayerState,
    ) -> Result<(), String> {
        let ci = ps.character_info();

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
            let tex = gfx.get_texture(sprite as usize);
            let q = tex.query();
            canvas.copy(
                tex,
                None,
                Some(sdl2::rect::Rect::new(x, y, q.width, q.height)),
            )?;
        }

        for n in 0..12usize {
            let worn_index = EQUIP_WNTAB[n];
            let sprite = ci.worn[worn_index];
            if sprite <= 0 {
                continue;
            }
            let x = 303 + ((n % 2) as i32) * 35;
            let y = 2 + ((n / 2) as i32) * 35;
            let tex = gfx.get_texture(sprite as usize);
            let q = tex.query();
            canvas.copy(
                tex,
                None,
                Some(sdl2::rect::Rect::new(x, y, q.width, q.height)),
            )?;
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

        Ok(())
    }

    fn draw_look_overlay(
        canvas: &mut Canvas<Window>,
        gfx: &mut GraphicsCache,
        ps: &PlayerState,
    ) -> Result<(), String> {
        if !ps.should_show_look() {
            return Ok(());
        }
        let look = ps.look_target();
        let name = look.name().unwrap_or("Unknown");

        font_cache::draw_text(canvas, gfx, UI_FONT, "Look:", 500, 236)?;
        font_cache::draw_text(canvas, gfx, UI_FONT, name, 542, 236)?;

        let hp_line = format!("HP {:>3}/{:>3}", look.a_hp(), look.hp());
        let end_line = format!("End {:>3}/{:>3}", look.a_end(), look.end());
        let mana_line = format!("Mana {:>3}/{:>3}", look.a_mana(), look.mana());
        font_cache::draw_text(canvas, gfx, UI_FONT, &hp_line, 500, 250)?;
        font_cache::draw_text(canvas, gfx, UI_FONT, &end_line, 500, 264)?;
        font_cache::draw_text(canvas, gfx, UI_FONT, &mana_line, 500, 278)?;

        Ok(())
    }

    fn draw_shop_overlay(
        canvas: &mut Canvas<Window>,
        gfx: &mut GraphicsCache,
        ps: &PlayerState,
    ) -> Result<(), String> {
        if !ps.should_show_shop() {
            return Ok(());
        }
        let shop = ps.shop_target();

        font_cache::draw_text(canvas, gfx, UI_FONT, "Shop", 500, 296)?;
        for i in 0..16usize {
            let item = shop.item(i);
            let price = shop.price(i);
            if item == 0 {
                continue;
            }
            let y = 310 + (i as i32) * 12;
            let tex = gfx.get_texture(item as usize);
            let q = tex.query();
            canvas.copy(
                tex,
                None,
                Some(sdl2::rect::Rect::new(500, y, q.width, q.height)),
            )?;
            let price_text = format!("{:>8}G", price / 100);
            font_cache::draw_text(canvas, gfx, UI_FONT, &price_text, 524, y)?;
        }

        Ok(())
    }

    fn draw_hover_effects(
        &self,
        canvas: &mut Canvas<Window>,
        ps: &PlayerState,
    ) -> Result<(), String> {
        canvas.set_draw_color(Color::RGB(255, 170, 80));

        // Hover highlight: skill labels grid
        if (610..=798).contains(&self.mouse_x) && (504..=548).contains(&self.mouse_y) {
            let col = (self.mouse_x - 610) / 49;
            let row = (self.mouse_y - 504) / 15;
            if (0..4).contains(&col) && (0..3).contains(&row) {
                let rx = 604 + col * 49;
                let ry = 504 + row * 15;
                canvas.draw_rect(sdl2::rect::Rect::new(rx, ry, 41, 14))?;
                return Ok(());
            }
        }

        // Hover highlight: mode/toggle buttons
        if (604..=798).contains(&self.mouse_x) && (552..=582).contains(&self.mouse_y) {
            let col = (self.mouse_x - 604) / 49;
            let row = (self.mouse_y - 552) / 16;
            if (0..4).contains(&col) && (0..2).contains(&row) {
                let rx = 604 + col * 49;
                let ry = 552 + row * 16;
                canvas.draw_rect(sdl2::rect::Rect::new(rx, ry, 41, 14))?;
                return Ok(());
            }
        }

        // Hover highlight: map tile marker — semi-transparent white tint over the
        // isometric floor diamond.  The ground plane of tile (mx,my) is the 32×16 region
        // starting at (cx-16, cy) where cx/cy are derived from the tile's view-space coords.
        //
        // When shift is held (C inter.c behavior):
        //  - Snap hover to the nearest ISITEM tile via spiral search
        //  - Only draw the diamond if an item tile was found
        //  - Color: green if ISUSABLE, yellow otherwise
        let (cam_xoff, cam_yoff) = Self::camera_offsets(ps);
        if let Some((mx, my)) =
            Self::screen_to_map_tile(self.mouse_x, self.mouse_y, cam_xoff, cam_yoff)
        {
            if self.shift_held {
                // Shift held: only highlight nearby ISITEM tiles.
                if let Some((sx, sy)) = Self::nearest_tile_with_flag(ps, mx, my, ISITEM) {
                    let (cx, cy) = Self::tile_ground_diamond_origin(sx, sy, cam_xoff, cam_yoff);
                    let tile = ps.map().tile_at_xy(sx, sy);
                    let is_usable = tile.map(|t| (t.flags & ISUSABLE) != 0).unwrap_or(false);
                    let color = if is_usable {
                        Color::RGBA(0, 255, 0, 100) // green = usable
                    } else {
                        Color::RGBA(255, 255, 0, 100) // yellow = pickup
                    };
                    Self::draw_hover_tile_diamond(canvas, cx, cy, color)?;
                }
            } else {
                let (cx, cy) = Self::tile_ground_diamond_origin(mx, my, cam_xoff, cam_yoff);
                Self::draw_hover_tile_diamond(canvas, cx, cy, Color::RGBA(255, 255, 255, 80))?;
            }
            canvas.set_draw_color(Color::RGB(255, 170, 80));
        }

        Ok(())
    }

    /// Update the persistent world minimap buffer from the current map state, then
    /// blit the 128×128 viewport centred on the player to the minimap area (x=3, y=471).
    fn draw_minimap(
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
                if (tile.flags & INVIS) != 0 {
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

        // Build the 128×128 viewport.
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

    fn attrib_needed(ci: &mag_core::types::ClientPlayer, n: usize, v: i32) -> i32 {
        const HIGH_VAL: i32 = i32::MAX;
        let max_v = ci.attrib[n][2] as i32;
        if v >= max_v {
            return HIGH_VAL;
        }
        let diff = ci.attrib[n][3] as i32;
        let v64 = v as i64;
        ((v64 * v64 * v64) * (diff as i64) / 20).clamp(0, i32::MAX as i64) as i32
    }

    fn skill_needed(ci: &mag_core::types::ClientPlayer, n: usize, v: i32) -> i32 {
        const HIGH_VAL: i32 = i32::MAX;
        let max_v = ci.skill[n][2] as i32;
        if v >= max_v {
            return HIGH_VAL;
        }
        let diff = ci.skill[n][3] as i32;
        let v64 = v as i64;
        let cubic = ((v64 * v64 * v64) * (diff as i64) / 40).clamp(0, i32::MAX as i64) as i32;
        v.max(cubic)
    }

    fn hp_needed(ci: &mag_core::types::ClientPlayer, v: i32) -> i32 {
        const HIGH_VAL: i32 = i32::MAX;
        if v >= ci.hp[2] as i32 {
            return HIGH_VAL;
        }
        (v as i64 * ci.hp[3] as i64).clamp(0, i32::MAX as i64) as i32
    }

    fn end_needed(ci: &mag_core::types::ClientPlayer, v: i32) -> i32 {
        const HIGH_VAL: i32 = i32::MAX;
        if v >= ci.end[2] as i32 {
            return HIGH_VAL;
        }
        (v as i64 * ci.end[3] as i64 / 2).clamp(0, i32::MAX as i64) as i32
    }

    fn mana_needed(ci: &mag_core::types::ClientPlayer, v: i32) -> i32 {
        const HIGH_VAL: i32 = i32::MAX;
        if v >= ci.mana[2] as i32 {
            return HIGH_VAL;
        }
        (v as i64 * ci.mana[3] as i64).clamp(0, i32::MAX as i64) as i32
    }

    fn sorted_skills(ci: &mag_core::types::ClientPlayer) -> Vec<usize> {
        let mut out: Vec<usize> = (0..MAX_SKILLS).collect();
        out.sort_by(|&a, &b| {
            let a_unused = get_skill_sortkey(a) == 'Z' || get_skill_name(a).is_empty();
            let b_unused = get_skill_sortkey(b) == 'Z' || get_skill_name(b).is_empty();
            if a_unused != b_unused {
                return if a_unused {
                    Ordering::Greater
                } else {
                    Ordering::Less
                };
            }

            let a_learned = ci.skill[a][0] != 0;
            let b_learned = ci.skill[b][0] != 0;
            if a_learned != b_learned {
                return if a_learned {
                    Ordering::Less
                } else {
                    Ordering::Greater
                };
            }

            let a_key = get_skill_sortkey(a);
            let b_key = get_skill_sortkey(b);
            if a_key != b_key {
                return a_key.cmp(&b_key);
            }

            get_skill_name(a).cmp(get_skill_name(b))
        });
        out
    }

    fn draw_attributes_skills(
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

        Ok(())
    }

    fn nearest_tile_with_flag(
        ps: &PlayerState,
        mx: usize,
        my: usize,
        flag: u32,
    ) -> Option<(usize, usize)> {
        let mut best: Option<(usize, usize, i32)> = None;
        for dy in -2..=2 {
            for dx in -2..=2 {
                let nx = mx as i32 + dx;
                let ny = my as i32 + dy;
                if nx < 0 || ny < 0 || nx >= TILEX as i32 || ny >= TILEY as i32 {
                    continue;
                }
                let ux = nx as usize;
                let uy = ny as usize;
                let Some(tile) = ps.map().tile_at_xy(ux, uy) else {
                    continue;
                };
                if (tile.flags & flag) == 0 {
                    continue;
                }
                let dist = dx * dx + dy * dy;
                match best {
                    Some((_, _, cur_dist)) if dist >= cur_dist => {}
                    _ => best = Some((ux, uy, dist)),
                }
            }
        }
        best.map(|(x, y, _)| (x, y))
    }

    fn click_mode_or_skill_button(
        &self,
        app_state: &mut AppState,
        mouse_btn: MouseButton,
        x: i32,
        y: i32,
    ) -> bool {
        if mouse_btn != MouseButton::Left {
            return false;
        }

        // Skill button labels area: 4x3
        if (610..=798).contains(&x) && (504..=548).contains(&y) {
            let col = ((x - 610) / 49) as usize;
            let row = ((y - 504) / 15) as usize;
            if col < 4 && row < 3 {
                let idx = row * 4 + col;
                if let (Some(net), Some(ps)) =
                    (app_state.network.as_ref(), app_state.player_state.as_ref())
                {
                    let btn = ps.player_data().skill_buttons[idx];
                    if !btn.is_unassigned() {
                        net.send(ClientCommand::new_skill(
                            btn.skill_nr(),
                            ps.selected_char() as u32,
                            ps.character_info().attrib[0][0] as u32,
                        ));
                    }
                }
                return true;
            }
        }

        // Mode/toggle buttons area: two rows, 4 cols, trans_button geometry.
        if (604..=798).contains(&x) && (552..=582).contains(&y) {
            let col = (x - 604) / 49;
            let row = (y - 552) / 16;
            if let Some(net) = app_state.network.as_ref() {
                if row == 0 {
                    match col {
                        0 => net.send(ClientCommand::new_mode(2)),
                        1 => net.send(ClientCommand::new_mode(1)),
                        2 => net.send(ClientCommand::new_mode(0)),
                        3 => {
                            if let Some(ps) = app_state.player_state.as_mut() {
                                let cur = ps.player_data().show_proz;
                                ps.player_data_mut().show_proz = 1 - cur;
                            }
                        }
                        _ => {}
                    }
                } else if row == 1 {
                    match col {
                        1 => {
                            if let Some(ps) = app_state.player_state.as_mut() {
                                let cur = ps.player_data().hide;
                                ps.player_data_mut().hide = 1 - cur;
                            }
                        }
                        2 => {
                            if let Some(ps) = app_state.player_state.as_mut() {
                                let cur = ps.player_data().show_names;
                                ps.player_data_mut().show_names = 1 - cur;
                            }
                        }
                        _ => {}
                    }
                }
            }
            return true;
        }

        false
    }

    /// Handle clicks on the stat/inventory/worn/shop panels (left-hand side + shop overlay).
    /// Returns true if the click was consumed and should not fall through to map interaction.
    fn click_stat_or_inv(
        &mut self,
        app_state: &mut AppState,
        mouse_btn: MouseButton,
        x: i32,
        y: i32,
    ) -> bool {
        // Extract all data we need from player_state up front to avoid borrow conflicts.
        let (ci, selected_char) = {
            let Some(ps) = app_state.player_state.as_ref() else {
                return false;
            };
            (ps.character_info().clone(), ps.selected_char() as u32)
        };

        // --- Stat/skill commit ("Update") button: x=109..158, y=254..266 (LMB only) ---
        if mouse_btn == MouseButton::Left && (109..=158).contains(&x) && (254..=266).contains(&y) {
            let sorted = Self::sorted_skills(&ci);
            for n in 0usize..108 {
                let v = self.stat_raised[n];
                if v == 0 {
                    continue;
                }
                let which: i16 = if n >= 8 {
                    let Some(&skill_id) = sorted.get(n - 8) else {
                        continue;
                    };
                    (get_skill_nr(skill_id) + 8) as i16
                } else {
                    n as i16
                };
                if let Some(net) = app_state.network.as_ref() {
                    net.send(ClientCommand::new_stat(which, v));
                }
            }
            self.stat_raised = [0; 108];
            self.stat_points_used = 0;
            return true;
        }

        // --- Stat +/- buttons: x=133..157, y=2..251 ---
        // + button: x < 145  |  - button: x >= 145
        // Row n = (y-2)/14.  Rows 0-4 = attrib, 5=HP, 6=End, 7=Mana, 8+ = skills
        if (133..=157).contains(&x)
            && (2..=251).contains(&y)
            && matches!(mouse_btn, MouseButton::Left)
        {
            let n = ((y - 2) / 14) as usize;
            let raising = x < 145;
            let repeat = if self.ctrl_held {
                90
            } else if self.shift_held {
                10
            } else {
                1
            };
            let sorted = Self::sorted_skills(&ci);

            let avail_now = ci.points - self.stat_points_used;
            let button_visible = if raising {
                match n {
                    0..=4 => {
                        let cur = ci.attrib[n][0] as i32 + self.stat_raised[n];
                        let need = Self::attrib_needed(&ci, n, cur);
                        need != i32::MAX && need <= avail_now
                    }
                    5 => {
                        let cur = ci.hp[0] as i32 + self.stat_raised[5];
                        let need = Self::hp_needed(&ci, cur);
                        need != i32::MAX && need <= avail_now
                    }
                    6 => {
                        let cur = ci.end[0] as i32 + self.stat_raised[6];
                        let need = Self::end_needed(&ci, cur);
                        need != i32::MAX && need <= avail_now
                    }
                    7 => {
                        let cur = ci.mana[0] as i32 + self.stat_raised[7];
                        let need = Self::mana_needed(&ci, cur);
                        need != i32::MAX && need <= avail_now
                    }
                    _ => {
                        let skilltab_index = (self.skill_scroll + n.saturating_sub(8)).min(99);
                        let raised_idx = 8 + skilltab_index;
                        if raised_idx >= 108 {
                            false
                        } else if let Some(&skill_id) = sorted.get(skilltab_index) {
                            if ci.skill[skill_id][0] == 0 {
                                false
                            } else {
                                let cur =
                                    ci.skill[skill_id][0] as i32 + self.stat_raised[raised_idx];
                                let need = Self::skill_needed(&ci, skill_id, cur);
                                need != i32::MAX && need <= avail_now
                            }
                        } else {
                            false
                        }
                    }
                }
            } else {
                match n {
                    0..=7 => self.stat_raised[n] > 0,
                    _ => {
                        let skilltab_index = (self.skill_scroll + n.saturating_sub(8)).min(99);
                        let raised_idx = 8 + skilltab_index;
                        raised_idx < 108 && self.stat_raised[raised_idx] > 0
                    }
                }
            };

            if !button_visible {
                return true;
            }

            for _ in 0..repeat {
                let avail = ci.points - self.stat_points_used;
                if raising {
                    match n {
                        0..=4 => {
                            let cur = ci.attrib[n][0] as i32 + self.stat_raised[n];
                            let need = Self::attrib_needed(&ci, n, cur);
                            if need != i32::MAX && need <= avail {
                                self.stat_points_used += need;
                                self.stat_raised[n] += 1;
                            }
                        }
                        5 => {
                            let cur = ci.hp[0] as i32 + self.stat_raised[5];
                            let need = Self::hp_needed(&ci, cur);
                            if need != i32::MAX && need <= avail {
                                self.stat_points_used += need;
                                self.stat_raised[5] += 1;
                            }
                        }
                        6 => {
                            let cur = ci.end[0] as i32 + self.stat_raised[6];
                            let need = Self::end_needed(&ci, cur);
                            if need != i32::MAX && need <= avail {
                                self.stat_points_used += need;
                                self.stat_raised[6] += 1;
                            }
                        }
                        7 => {
                            let cur = ci.mana[0] as i32 + self.stat_raised[7];
                            let need = Self::mana_needed(&ci, cur);
                            if need != i32::MAX && need <= avail {
                                self.stat_points_used += need;
                                self.stat_raised[7] += 1;
                            }
                        }
                        _ => {
                            let skilltab_index = (self.skill_scroll + n.saturating_sub(8)).min(99);
                            let raised_idx = 8 + skilltab_index;
                            if raised_idx < 108 {
                                if let Some(&skill_id) = sorted.get(skilltab_index) {
                                    if ci.skill[skill_id][0] != 0 {
                                        let cur = ci.skill[skill_id][0] as i32
                                            + self.stat_raised[raised_idx];
                                        let need = Self::skill_needed(&ci, skill_id, cur);
                                        if need != i32::MAX && need <= avail {
                                            self.stat_points_used += need;
                                            self.stat_raised[raised_idx] += 1;
                                        }
                                    }
                                }
                            }
                        }
                    }
                } else {
                    // Lowering
                    match n {
                        0..=4 => {
                            if self.stat_raised[n] > 0 {
                                self.stat_raised[n] -= 1;
                                let cur = ci.attrib[n][0] as i32 + self.stat_raised[n];
                                let refund = Self::attrib_needed(&ci, n, cur);
                                if refund != i32::MAX {
                                    self.stat_points_used -= refund;
                                }
                            }
                        }
                        5 => {
                            if self.stat_raised[5] > 0 {
                                self.stat_raised[5] -= 1;
                                let cur = ci.hp[0] as i32 + self.stat_raised[5];
                                let refund = Self::hp_needed(&ci, cur);
                                if refund != i32::MAX {
                                    self.stat_points_used -= refund;
                                }
                            }
                        }
                        6 => {
                            if self.stat_raised[6] > 0 {
                                self.stat_raised[6] -= 1;
                                let cur = ci.end[0] as i32 + self.stat_raised[6];
                                let refund = Self::end_needed(&ci, cur);
                                if refund != i32::MAX {
                                    self.stat_points_used -= refund;
                                }
                            }
                        }
                        7 => {
                            if self.stat_raised[7] > 0 {
                                self.stat_raised[7] -= 1;
                                let cur = ci.mana[0] as i32 + self.stat_raised[7];
                                let refund = Self::mana_needed(&ci, cur);
                                if refund != i32::MAX {
                                    self.stat_points_used -= refund;
                                }
                            }
                        }
                        _ => {
                            let skilltab_index = (self.skill_scroll + n.saturating_sub(8)).min(99);
                            let raised_idx = 8 + skilltab_index;
                            if raised_idx < 108 && self.stat_raised[raised_idx] > 0 {
                                self.stat_raised[raised_idx] -= 1;
                                if let Some(&skill_id) = sorted.get(skilltab_index) {
                                    let cur =
                                        ci.skill[skill_id][0] as i32 + self.stat_raised[raised_idx];
                                    let refund = Self::skill_needed(&ci, skill_id, cur);
                                    if refund != i32::MAX {
                                        self.stat_points_used -= refund;
                                    }
                                }
                            }
                        }
                    }
                }
            }
            return true;
        }

        // --- Inventory backpack click: x=220..290, y=2..177 (2 cols × 35px, 5 rows × 35px) ---
        if (220..=290).contains(&x) && (2..=177).contains(&y) {
            let col = (x - 220) / 35;
            let row = (y - 2) / 35;
            if col < 2 && row < 5 {
                let idx = (self.inv_scroll + (row * 2 + col) as usize) as u32;
                if let Some(net) = app_state.network.as_ref() {
                    if mouse_btn == MouseButton::Right {
                        net.send(ClientCommand::new_inv_look(idx, 0, selected_char));
                    } else {
                        let a = if self.shift_held { 0u32 } else { 6u32 };
                        net.send(ClientCommand::new_inv(a, idx, selected_char));
                    }
                }
                return true;
            }
        }

        // --- Worn equipment click: x=303..373, y=2..212 (2 cols × 35px, 6 rows × 35px) ---
        // Slot remapping from orig/inter.c::mouse_inventory (matches Bevy inventory.rs).
        if (303..=373).contains(&x) && (2..=212).contains(&y) {
            let tx = (x - 303) / 35;
            let ty = (y - 2) / 35;
            let slot_nr: Option<u32> = match (tx, ty) {
                (0, 0) => Some(0),  // head
                (1, 0) => Some(9),  // cloak
                (0, 1) => Some(2),  // body
                (1, 1) => Some(3),  // arms
                (0, 2) => Some(1),  // neck
                (1, 2) => Some(4),  // belt
                (0, 3) => Some(8),  // right hand
                (1, 3) => Some(7),  // left hand
                (0, 4) => Some(10), // left ring
                (1, 4) => Some(11), // right ring
                (0, 5) => Some(5),  // legs
                (1, 5) => Some(6),  // feet
                _ => None,
            };
            if let Some(slot_nr) = slot_nr {
                if let Some(net) = app_state.network.as_ref() {
                    // RMB=7 (right-click worn), LMB+Shift=1 (shift-equip), LMB=5 (normal equip)
                    let a = match mouse_btn {
                        MouseButton::Right => 7u32,
                        MouseButton::Left if self.shift_held => 1u32,
                        _ => 5u32,
                    };
                    net.send(ClientCommand::new_inv(a, slot_nr, selected_char));
                }
                return true;
            }
        }

        // --- Shop overlay click: item rows at y=310..(310+16*12) when shop is visible ---
        let shop_cmd: Option<(i16, i32)> = {
            let Some(ps) = app_state.player_state.as_ref() else {
                return false;
            };
            if mouse_btn == MouseButton::Left && ps.should_show_shop() {
                let shop = ps.shop_target();
                if (500..=620).contains(&x) && (310..=502).contains(&y) {
                    let i = ((y - 310) / 12) as usize;
                    if i < 16 && shop.item(i) != 0 {
                        Some(((i as i16) + 1, shop.nr() as i32))
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            }
        };
        if let Some((shop_idx, shop_nr_val)) = shop_cmd {
            if let Some(net) = app_state.network.as_ref() {
                net.send(ClientCommand::new_shop(shop_idx, shop_nr_val));
            }
            return true;
        }

        false
    }

    fn screen_to_map_tile(
        screen_x: i32,
        screen_y: i32,
        cam_xoff: i32,
        cam_yoff: i32,
    ) -> Option<(usize, usize)> {
        // Use the same projected ground diamond geometry as rendering so hover/click
        // always align with visible tiles.
        let mut best: Option<(usize, usize, i32)> = None;

        for my in 0..TILEY {
            for mx in 0..TILEX {
                let (cx, cy_top) = Self::tile_ground_diamond_origin(mx, my, cam_xoff, cam_yoff);
                let dx = (screen_x - cx).abs();
                let dy = (screen_y - (cy_top + 8)).abs();

                // Inside 32x16 isometric floor diamond:
                // |dx|/16 + |dy|/8 <= 1  =>  dx*8 + dy*16 <= 128
                let metric = dx * 8 + dy * 16;
                if metric <= 128 {
                    match best {
                        Some((_, _, cur_metric)) if metric >= cur_metric => {}
                        _ => best = Some((mx, my, metric)),
                    }
                }
            }
        }

        best.map(|(mx, my, _)| (mx, my))
    }
}

impl Default for GameScene {
    fn default() -> Self {
        Self::new()
    }
}

impl Scene for GameScene {
    fn on_enter(&mut self, app_state: &mut AppState) {
        self.input_buf.clear();
        self.pending_exit = None;
        self.log_scroll = 0;
        self.ctrl_held = false;
        self.shift_held = false;
        self.alt_held = false;
        self.skill_scroll = 0;
        self.inv_scroll = 0;
        self.mouse_x = 0;
        self.mouse_y = 0;
        self.stat_raised = [0; 108];
        self.stat_points_used = 0;
        self.minimap_xmap.fill(0);
        self.minimap_last_xy = None;

        let (ticket, race) = match app_state.api.login_target {
            Some(t) => t,
            None => {
                log::error!("GameScene on_enter: no login_target set");
                self.pending_exit = Some("No login target".to_string());
                return;
            }
        };

        let host = crate::hosts::get_server_ip();
        log::info!(
            "GameScene: connecting to {}:5555 with ticket={}",
            host,
            ticket
        );

        app_state.network = Some(NetworkRuntime::new(host, 5555, ticket, race));
        app_state.player_state = Some(PlayerState::default());
    }

    fn on_exit(&mut self, app_state: &mut AppState) {
        if let Some(mut net) = app_state.network.take() {
            net.shutdown();
        }
        app_state.player_state = None;
    }

    fn handle_event(&mut self, app_state: &mut AppState, event: &Event) -> Option<SceneType> {
        match event {
            Event::KeyDown {
                keycode: Some(kc),
                keymod,
                ..
            } => match *kc {
                Keycode::Return | Keycode::KpEnter => {
                    if !self.input_buf.is_empty() {
                        let text = self.input_buf.clone();
                        self.input_buf.clear();
                        if let Some(net) = app_state.network.as_ref() {
                            for pkt in ClientCommand::new_say_packets(text.as_bytes()) {
                                net.send(pkt);
                            }
                        }
                    }
                }
                Keycode::Backspace => {
                    self.input_buf.pop();
                }
                Keycode::LCtrl | Keycode::RCtrl => {
                    self.ctrl_held = true;
                }
                Keycode::LShift | Keycode::RShift => {
                    self.shift_held = true;
                }
                Keycode::LAlt | Keycode::RAlt => {
                    self.alt_held = true;
                }
                Keycode::Escape => {
                    if let Some(net) = app_state.network.as_ref() {
                        net.send(ClientCommand::new_reset());
                    }
                }
                Keycode::F1 => {
                    if keymod.intersects(Mod::LSHIFTMOD | Mod::RSHIFTMOD) {
                        if let (Some(net), Some(ps)) =
                            (app_state.network.as_ref(), app_state.player_state.as_ref())
                        {
                            let btn = ps.player_data().skill_buttons[0];
                            if !btn.is_unassigned() {
                                net.send(ClientCommand::new_skill(
                                    btn.skill_nr(),
                                    ps.selected_char() as u32,
                                    ps.character_info().attrib[0][0] as u32,
                                ));
                            }
                        }
                    } else if let Some(net) = app_state.network.as_ref() {
                        net.send(ClientCommand::new_mode(2));
                    }
                }
                Keycode::F2 => {
                    if keymod.intersects(Mod::LSHIFTMOD | Mod::RSHIFTMOD) {
                        if let (Some(net), Some(ps)) =
                            (app_state.network.as_ref(), app_state.player_state.as_ref())
                        {
                            let btn = ps.player_data().skill_buttons[1];
                            if !btn.is_unassigned() {
                                net.send(ClientCommand::new_skill(
                                    btn.skill_nr(),
                                    ps.selected_char() as u32,
                                    ps.character_info().attrib[0][0] as u32,
                                ));
                            }
                        }
                    } else if let Some(net) = app_state.network.as_ref() {
                        net.send(ClientCommand::new_mode(1));
                    }
                }
                Keycode::F3 => {
                    if keymod.intersects(Mod::LSHIFTMOD | Mod::RSHIFTMOD) {
                        if let (Some(net), Some(ps)) =
                            (app_state.network.as_ref(), app_state.player_state.as_ref())
                        {
                            let btn = ps.player_data().skill_buttons[2];
                            if !btn.is_unassigned() {
                                net.send(ClientCommand::new_skill(
                                    btn.skill_nr(),
                                    ps.selected_char() as u32,
                                    ps.character_info().attrib[0][0] as u32,
                                ));
                            }
                        }
                    } else if let Some(net) = app_state.network.as_ref() {
                        net.send(ClientCommand::new_mode(0));
                    }
                }
                Keycode::F4 => {
                    if let Some(ps) = app_state.player_state.as_mut() {
                        let current = ps.player_data().show_proz;
                        ps.player_data_mut().show_proz = 1 - current;
                    }
                }
                Keycode::F6 => {
                    if let Some(ps) = app_state.player_state.as_mut() {
                        let current = ps.player_data().hide;
                        ps.player_data_mut().hide = 1 - current;
                    }
                }
                Keycode::F7 => {
                    if let Some(ps) = app_state.player_state.as_mut() {
                        let current = ps.player_data().show_names;
                        ps.player_data_mut().show_names = 1 - current;
                    }
                }
                Keycode::F12 => {
                    if keymod.intersects(Mod::LSHIFTMOD | Mod::RSHIFTMOD) {
                        if let (Some(net), Some(ps)) =
                            (app_state.network.as_ref(), app_state.player_state.as_ref())
                        {
                            let btn = ps.player_data().skill_buttons[11];
                            if !btn.is_unassigned() {
                                net.send(ClientCommand::new_skill(
                                    btn.skill_nr(),
                                    ps.selected_char() as u32,
                                    ps.character_info().attrib[0][0] as u32,
                                ));
                            }
                        }
                    } else if let Some(net) = app_state.network.as_ref() {
                        net.send(ClientCommand::new_exit());
                    }
                }
                Keycode::PageUp => {
                    self.log_scroll = self.log_scroll.saturating_add(3);
                }
                Keycode::PageDown => {
                    self.log_scroll = self.log_scroll.saturating_sub(3);
                }
                Keycode::Up => {
                    self.skill_scroll = self.skill_scroll.saturating_sub(1);
                }
                Keycode::Down => {
                    self.skill_scroll = (self.skill_scroll + 1).min(90);
                }
                _ => {}
            },
            Event::KeyUp {
                keycode: Some(kc), ..
            } => match *kc {
                Keycode::LCtrl | Keycode::RCtrl => {
                    self.ctrl_held = false;
                }
                Keycode::LShift | Keycode::RShift => {
                    self.shift_held = false;
                }
                Keycode::LAlt | Keycode::RAlt => {
                    self.alt_held = false;
                }
                _ => {}
            },
            Event::TextInput { text, .. } => {
                if self.input_buf.len() + text.len() <= MAX_INPUT_LEN {
                    self.input_buf.push_str(text);
                }
            }
            Event::MouseMotion { x, y, .. } => {
                self.mouse_x = *x;
                self.mouse_y = *y;
            }
            Event::MouseButtonUp {
                mouse_btn, x, y, ..
            } => {
                if self.click_stat_or_inv(app_state, *mouse_btn, *x, *y) {
                    return None;
                }
                if self.click_mode_or_skill_button(app_state, *mouse_btn, *x, *y) {
                    return None;
                }

                let Some(ps) = app_state.player_state.as_ref() else {
                    return None;
                };

                let (cam_xoff, cam_yoff) = Self::camera_offsets(ps);

                let Some((mx, my)) = Self::screen_to_map_tile(*x, *y, cam_xoff, cam_yoff) else {
                    return None;
                };

                // C client edge-tile clipping (inter.c:872):
                // Reject clicks on the outer edge tiles where the map data is unreliable.
                if !(3..=TILEX - 7).contains(&mx) || !(7..=TILEY - 3).contains(&my) {
                    return None;
                }

                let has_ctrl = self.ctrl_held;
                let has_shift = self.shift_held;
                let has_alt = self.alt_held;

                let snapped = if has_ctrl || has_alt {
                    Self::nearest_tile_with_flag(ps, mx, my, ISCHAR).unwrap_or((mx, my))
                } else if has_shift {
                    Self::nearest_tile_with_flag(ps, mx, my, ISITEM).unwrap_or((mx, my))
                } else {
                    (mx, my)
                };

                let (sx, sy) = snapped;
                let tile = ps.map().tile_at_xy(sx, sy);
                let target_cn = tile.map(|t| t.ch_nr as u32).unwrap_or(0);
                let target_id = tile.map(|t| t.ch_id).unwrap_or(0);
                let (world_x, world_y) = tile.map(|t| (t.x as i16, t.y as i32)).unwrap_or((0, 0));
                let citem = ps.character_info().citem;
                let selected_char = ps.selected_char();

                let Some(net) = app_state.network.as_ref() else {
                    return None;
                };

                match *mouse_btn {
                    MouseButton::Left if has_alt => {
                        if let Some(ps_mut) = app_state.player_state.as_mut() {
                            if target_cn != 0 {
                                if selected_char == target_cn as u16 {
                                    ps_mut.clear_selected_char();
                                } else {
                                    ps_mut.set_selected_char_with_id(target_cn as u16, target_id);
                                }
                            }
                        }
                    }
                    MouseButton::Right if has_alt => {
                        if target_cn != 0 {
                            net.send(ClientCommand::new_look(target_cn));
                        }
                    }
                    MouseButton::Left if has_ctrl => {
                        if target_cn != 0 {
                            if citem != 0 {
                                net.send(ClientCommand::new_give(target_cn));
                            } else {
                                net.send(ClientCommand::new_attack(target_cn));
                            }
                        }
                    }
                    MouseButton::Right if has_ctrl => {
                        if target_cn != 0 {
                            net.send(ClientCommand::new_look(target_cn));
                        }
                    }
                    MouseButton::Left if has_shift => {
                        let tile_flags = tile.map(|t| t.flags).unwrap_or(0);
                        let is_item = (tile_flags & ISITEM) != 0;
                        let is_usable = (tile_flags & ISUSABLE) != 0;
                        if citem != 0 && !is_item {
                            // Holding item, clicked non-item tile → drop
                            net.send(ClientCommand::new_drop(world_x, world_y));
                        } else if is_item && is_usable {
                            // Item is usable → use
                            net.send(ClientCommand::new_use(world_x, world_y));
                        } else if is_item {
                            // Item not usable → pickup
                            net.send(ClientCommand::new_pickup(world_x, world_y));
                        }
                    }
                    MouseButton::Right if has_shift => {
                        net.send(ClientCommand::new_look_item(world_x, world_y));
                    }
                    MouseButton::Left => {
                        net.send(ClientCommand::new_move(world_x, world_y));
                    }
                    MouseButton::Right => {
                        net.send(ClientCommand::new_turn(world_x, world_y));
                    }
                    _ => {}
                }
            }
            Event::MouseWheel { y, .. } => {
                let dy = *y;
                if self.mouse_x < 220 {
                    // Skill / stat panel
                    if dy > 0 {
                        self.skill_scroll = self.skill_scroll.saturating_sub(dy as usize);
                    } else if dy < 0 {
                        self.skill_scroll = (self.skill_scroll + (-dy) as usize).min(90);
                    }
                } else if self.mouse_x < 300 {
                    // Inventory panel
                    if dy > 0 {
                        self.inv_scroll = self.inv_scroll.saturating_sub(dy as usize);
                    } else if dy < 0 {
                        self.inv_scroll = (self.inv_scroll + (-dy) as usize).min(30);
                    }
                } else {
                    // Chat / default: scroll log
                    if dy > 0 {
                        self.log_scroll = self.log_scroll.saturating_add(dy as usize);
                    } else if dy < 0 {
                        self.log_scroll = self.log_scroll.saturating_sub((-dy) as usize);
                    }
                }
            }
            _ => {}
        }
        None
    }

    fn update(&mut self, app_state: &mut AppState, _dt: Duration) -> Option<SceneType> {
        self.process_network_events(app_state)
    }

    fn render_world(
        &mut self,
        app_state: &mut AppState,
        canvas: &mut Canvas<Window>,
    ) -> Result<(), String> {
        canvas.set_draw_color(Color::RGB(0, 0, 0));
        canvas.clear();

        // Split borrow: gfx_cache (mut) and player_state (ref) are separate fields.
        let AppState {
            ref mut gfx_cache,
            ref player_state,
            ..
        } = *app_state;

        let Some(ps) = player_state.as_ref() else {
            return Ok(());
        };

        // 1. World tiles (two-pass painter order)
        Self::draw_world(canvas, gfx_cache, ps)?;

        // 2. Static UI frame (sprite 1) overlays the world
        Self::draw_ui_frame(canvas, gfx_cache)?;

        // 3. HP / End / Mana bars
        Self::draw_bars(canvas, ps)?;

        // 4. Stat text labels
        Self::draw_stat_text(canvas, gfx_cache, ps)?;

        // 5. Chat log + input line
        self.draw_chat(canvas, gfx_cache, ps)?;

        // 6. Lower-right mode/status indicators
        Self::draw_mode_indicators(canvas, ps)?;

        // 7. Left panel attributes and skills
        self.draw_attributes_skills(canvas, gfx_cache, ps)?;

        // 8. Inventory, worn items, spells, carried item
        self.draw_inventory_equipment_spells(canvas, gfx_cache, ps)?;

        // 9. Look/shop overlays
        Self::draw_look_overlay(canvas, gfx_cache, ps)?;
        Self::draw_shop_overlay(canvas, gfx_cache, ps)?;

        // 10. Hover highlights
        self.draw_hover_effects(canvas, ps)?;

        // 11. Minimap (bottom-left, 128×128, persistent world buffer)
        self.draw_minimap(canvas, gfx_cache, ps)?;

        Ok(())
    }

    fn render_ui(&mut self, _app_state: &mut AppState, _ctx: &egui::Context) -> Option<SceneType> {
        // All UI is drawn via sprites and bitmap fonts in render_world.
        // egui is not used for the gameplay scene.
        None
    }
}
