use std::time::Duration;

use egui_sdl2::egui;
use sdl2::{event::Event, keyboard::Keycode, pixels::Color, render::Canvas, video::Window};

use mag_core::constants::{SPR_EMPTY, TILEX, TILEY, XPOS, YPOS};

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

/// Maximum network events processed per frame.
const MAX_EVENTS_PER_FRAME: usize = 128;

// ---- Layout constants (ported from engine.c / layout.rs) ---- //

/// Camera X shift to account for the left-hand UI panel.
const MAP_X_SHIFT: i32 = -176;

/// Sprite ID of the static 800x600 UI background frame.
const UI_FRAME_SPRITE: usize = 1;

/// Default bitmap font index (yellow, sprite 701).
const UI_FONT: usize = 1;

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

pub struct GameScene {
    input_buf: String,
    pending_exit: Option<String>,
    log_scroll: usize,
}

impl GameScene {
    pub fn new() -> Self {
        Self {
            input_buf: String::new(),
            pending_exit: None,
            log_scroll: 0,
        }
    }

    fn process_network_events(&mut self, app_state: &mut AppState) -> Option<SceneType> {
        for _ in 0..MAX_EVENTS_PER_FRAME {
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
                                log::debug!("PlaySound: nr={} vol={} pan={}", nr, vol, pan);
                                // TODO: trigger sfx playback via SoundCache
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

        // Pass 1: Background / terrain sprites.
        for y in 0..TILEY {
            for x in 0..TILEX {
                let Some(tile) = map.tile_at_xy(x, y) else {
                    continue;
                };

                let ba = tile.ba_sprite;
                if ba <= 0 || ba as u16 == SPR_EMPTY {
                    continue;
                }

                let texture = gfx.get_texture(ba as usize);
                let q = texture.query();
                let xs = q.width as i32 / 32;
                let ys = q.height as i32 / 32;

                let xpos = (x as i32) * 32;
                let ypos = (y as i32) * 32;
                let rx = xpos / 2 + ypos / 2 - xs * 16 + 32 + XPOS + MAP_X_SHIFT;
                let ry = xpos / 4 - ypos / 4 + YPOS - ys * 32;

                canvas.copy(
                    texture,
                    None,
                    Some(sdl2::rect::Rect::new(rx, ry, q.width, q.height)),
                )?;
            }
        }

        // Pass 2: Items and characters (same scan order for correct z-layering).
        for y in 0..TILEY {
            for x in 0..TILEX {
                let Some(tile) = map.tile_at_xy(x, y) else {
                    continue;
                };

                let xpos = (x as i32) * 32;
                let ypos = (y as i32) * 32;
                let xoff = tile.obj_xoff;
                let yoff = tile.obj_yoff;

                let it = tile.it_sprite;
                if it > 0 && it != SPR_EMPTY {
                    let texture = gfx.get_texture(it as usize);
                    let q = texture.query();
                    let xs = q.width as i32 / 32;
                    let ys = q.height as i32 / 32;

                    let rx = xpos / 2 + ypos / 2 - xs * 16 + 32 + XPOS + MAP_X_SHIFT + xoff;
                    let ry = xpos / 4 - ypos / 4 + YPOS - ys * 32 + yoff;

                    canvas.copy(
                        texture,
                        None,
                        Some(sdl2::rect::Rect::new(rx, ry, q.width, q.height)),
                    )?;
                }

                let ch = tile.ch_sprite;
                if ch > 0 && ch != SPR_EMPTY {
                    let texture = gfx.get_texture(ch as usize);
                    let q = texture.query();
                    let xs = q.width as i32 / 32;
                    let ys = q.height as i32 / 32;

                    let rx = xpos / 2 + ypos / 2 - xs * 16 + 32 + XPOS + MAP_X_SHIFT + xoff;
                    let ry = xpos / 4 - ypos / 4 + YPOS - ys * 32 + yoff;

                    canvas.copy(
                        texture,
                        None,
                        Some(sdl2::rect::Rect::new(rx, ry, q.width, q.height)),
                    )?;
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

        let hp = ci.hp[0] as i32;
        let a_hp = ci.a_hp;
        let end_val = ci.end[0] as i32;
        let a_end = ci.a_end;
        let mana = ci.mana[0] as i32;
        let a_mana = ci.a_mana;

        // dd_showbar(373, y, n, 6, color) — bar_width = (cur * 62) / max, clamped [0,124]
        let draw_bar =
            |canvas: &mut Canvas<Window>, y: i32, cur: i32, max: i32| -> Result<(), String> {
                if max <= 0 {
                    return Ok(());
                }
                let filled = ((cur * BAR_SCALE_NUM) / max).clamp(0, BAR_W_MAX);
                // Background (full capacity)
                canvas.set_draw_color(BAR_BG_COLOR);
                canvas.fill_rect(sdl2::rect::Rect::new(BAR_X, y, BAR_W_MAX as u32, BAR_H))?;
                // Foreground (current)
                if filled > 0 {
                    canvas.set_draw_color(BAR_FILL_COLOR);
                    canvas.fill_rect(sdl2::rect::Rect::new(BAR_X, y, filled as u32, BAR_H))?;
                }
                Ok(())
            };

        draw_bar(canvas, BAR_HP_Y, hp, a_hp)?;
        draw_bar(canvas, BAR_END_Y, end_val, a_end)?;
        draw_bar(canvas, BAR_MANA_Y, mana, a_mana)?;

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
        let hp_text = format!("Hitpoints: {}/{}", ci.hp[0], ci.a_hp);
        font_cache::draw_text(canvas, gfx, UI_FONT, &hp_text, STAT_HP_X, STAT_HP_Y)?;

        let end_text = format!("Endurance: {}/{}", ci.end[0], ci.a_end);
        font_cache::draw_text(canvas, gfx, UI_FONT, &end_text, STAT_END_X, STAT_END_Y)?;

        let mana_text = format!("Mana: {}/{}", ci.mana[0], ci.a_mana);
        font_cache::draw_text(canvas, gfx, UI_FONT, &mana_text, STAT_MANA_X, STAT_MANA_Y)?;

        // Gold (money display: G and S)
        if ci.gold > 0 {
            let gold = ci.gold / 100;
            let silver = ci.gold % 100;
            let money_text = format!("Money  {}G {}S", gold, silver);
            font_cache::draw_text(
                canvas,
                gfx,
                UI_FONT,
                &money_text,
                STAT_MONEY_X,
                STAT_MONEY_Y,
            )?;
        }

        // Weapon / Armor / Experience in bottom-right area
        let wv_text = format!("WV: {}", ci.weapon);
        font_cache::draw_text(canvas, gfx, UI_FONT, &wv_text, STAT_WEAPON_X, STAT_WEAPON_Y)?;

        let av_text = format!("AV: {}", ci.armor);
        font_cache::draw_text(canvas, gfx, UI_FONT, &av_text, STAT_ARMOR_X, STAT_ARMOR_Y)?;

        let exp_text = format!("Exp: {}", ci.points_tot);
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

        for (i, log_idx) in (start..end).enumerate() {
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
                keycode: Some(kc), ..
            } => match *kc {
                Keycode::Return | Keycode::KpEnter => {
                    if !self.input_buf.is_empty() {
                        let text = self.input_buf.clone();
                        self.input_buf.clear();
                        if let Some(net) = app_state.network.as_ref() {
                            for pkt in ClientCommand::new_say_packets(text.as_bytes()) {
                                net.send(pkt.to_bytes());
                            }
                        }
                    }
                }
                Keycode::Backspace => {
                    self.input_buf.pop();
                }
                Keycode::Escape => {
                    if let Some(net) = app_state.network.as_ref() {
                        net.send(ClientCommand::new_reset().to_bytes());
                    }
                }
                Keycode::PageUp => {
                    self.log_scroll = self.log_scroll.saturating_add(3);
                }
                Keycode::PageDown => {
                    // silences unused_variables
                    self.log_scroll = self.log_scroll.saturating_sub(3);
                }
                _ => {}
            },
            Event::TextInput { text, .. } => {
                if self.input_buf.len() + text.len() <= MAX_INPUT_LEN {
                    self.input_buf.push_str(text);
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

        Ok(())
    }

    fn render_ui(&mut self, _app_state: &mut AppState, _ctx: &egui::Context) -> Option<SceneType> {
        // All UI is drawn via sprites and bitmap fonts in render_world.
        // egui is not used for the gameplay scene.
        None
    }
}
