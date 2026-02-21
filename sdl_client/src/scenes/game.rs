use std::time::Duration;

use egui_sdl2::egui;
use sdl2::{event::Event, keyboard::Keycode, pixels::Color, render::Canvas, video::Window};

use mag_core::constants::{SPR_EMPTY, TILEX, TILEY, XPOS, YPOS};

use crate::{
    font_cache,
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
        gfx: &mut crate::gfx_cache::GraphicsCache,
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
                let rx = xpos / 2 + ypos / 2 - xs * 16 + 32 + XPOS;
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

                    let rx = xpos / 2 + ypos / 2 - xs * 16 + 32 + XPOS + xoff;
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

                    let rx = xpos / 2 + ypos / 2 - xs * 16 + 32 + XPOS + xoff;
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

        Self::draw_world(canvas, gfx_cache, ps)?;

        // Draw HUD bars directly on the SDL canvas.
        let ci = ps.character_info();
        let hp = ci.hp.first().copied().unwrap_or(0) as i32;
        let a_hp = ci.a_hp;
        let end_val = ci.end.first().copied().unwrap_or(0) as i32;
        let a_end = ci.a_end;
        let mana = ci.mana.first().copied().unwrap_or(0) as i32;
        let a_mana = ci.a_mana;

        const BAR_X: i32 = 8;
        const BAR_Y: i32 = 562;
        const BAR_W: u32 = 100;

        let draw_bar = |canvas: &mut Canvas<Window>, y: i32, cur: i32, max: i32, color: Color| {
            let filled = if max > 0 {
                (BAR_W as i32 * cur / max).clamp(0, BAR_W as i32) as u32
            } else {
                0
            };
            canvas.set_draw_color(Color::RGB(40, 40, 40));
            let _ = canvas.fill_rect(sdl2::rect::Rect::new(BAR_X, y, BAR_W, 8));
            if filled > 0 {
                canvas.set_draw_color(color);
                let _ = canvas.fill_rect(sdl2::rect::Rect::new(BAR_X, y, filled, 8));
            }
        };

        if a_hp > 0 {
            draw_bar(canvas, BAR_Y, hp, a_hp, Color::RGB(200, 50, 50));
        }
        if a_end > 0 {
            draw_bar(canvas, BAR_Y + 10, end_val, a_end, Color::RGB(50, 200, 50));
        }
        if a_mana > 0 {
            draw_bar(canvas, BAR_Y + 20, mana, a_mana, Color::RGB(50, 100, 220));
        }

        // Re-borrow for font rendering.
        let AppState {
            ref mut gfx_cache, ..
        } = *app_state;
        font_cache::draw_text(canvas, gfx_cache, 1, "HP", BAR_X + BAR_W as i32 + 4, BAR_Y)?;
        font_cache::draw_text(
            canvas,
            gfx_cache,
            2,
            "End",
            BAR_X + BAR_W as i32 + 4,
            BAR_Y + 10,
        )?;
        font_cache::draw_text(
            canvas,
            gfx_cache,
            3,
            "Mana",
            BAR_X + BAR_W as i32 + 4,
            BAR_Y + 20,
        )?;

        Ok(())
    }

    fn render_ui(&mut self, app_state: &mut AppState, ctx: &egui::Context) -> Option<SceneType> {
        // ---- Character stats panel (right) ---- //
        egui::SidePanel::right("stats_panel")
            .resizable(false)
            .min_width(150.0)
            .max_width(150.0)
            .show(ctx, |ui| {
                ui.heading("Character");
                ui.separator();

                if let Some(ps) = app_state.player_state.as_ref() {
                    let ci = ps.character_info();

                    let name = mag_core::string_operations::c_string_to_str(&ci.name);
                    if !name.is_empty() {
                        ui.label(name);
                        ui.add_space(2.0);
                    }

                    let hp = ci.hp.first().copied().unwrap_or(0) as i32;
                    let a_hp = ci.a_hp;
                    let end_val = ci.end.first().copied().unwrap_or(0) as i32;
                    let a_end = ci.a_end;
                    let mana = ci.mana.first().copied().unwrap_or(0) as i32;
                    let a_mana = ci.a_mana;

                    if a_hp > 0 {
                        ui.label(format!("HP: {}/{}", hp, a_hp));
                        ui.add(
                            egui::ProgressBar::new((hp as f32 / a_hp as f32).clamp(0.0, 1.0))
                                .fill(egui::Color32::from_rgb(200, 50, 50))
                                .desired_width(130.0),
                        );
                    }
                    if a_end > 0 {
                        ui.label(format!("End: {}/{}", end_val, a_end));
                        ui.add(
                            egui::ProgressBar::new((end_val as f32 / a_end as f32).clamp(0.0, 1.0))
                                .fill(egui::Color32::from_rgb(50, 200, 50))
                                .desired_width(130.0),
                        );
                    }
                    if a_mana > 0 {
                        ui.label(format!("Mana: {}/{}", mana, a_mana));
                        ui.add(
                            egui::ProgressBar::new((mana as f32 / a_mana as f32).clamp(0.0, 1.0))
                                .fill(egui::Color32::from_rgb(50, 100, 220))
                                .desired_width(130.0),
                        );
                    }

                    ui.add_space(4.0);
                    ui.separator();

                    if ci.gold > 0 {
                        ui.label(format!("Gold: {}", ci.gold));
                    }
                    if ci.points > 0 {
                        ui.label(format!("Points: {}", ci.points));
                    }
                    if ci.kindred > 0 {
                        ui.label(format!("Kindred: {}", ci.kindred));
                    }
                } else {
                    ui.label("Connecting...");
                }

                if let Some(net) = app_state.network.as_ref() {
                    ui.add_space(4.0);
                    ui.separator();
                    ui.label(format!("Tick: {}", net.client_ticker));
                    if let Some(rtt) = net.last_rtt_ms {
                        ui.label(format!("RTT: {}ms", rtt));
                    }
                }
            });

        // ---- Chat log panel (bottom) ---- //
        egui::TopBottomPanel::bottom("chat_panel")
            .resizable(false)
            .min_height(130.0)
            .max_height(130.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(">");
                    let resp = ui.add(
                        egui::TextEdit::singleline(&mut self.input_buf)
                            .desired_width(f32::INFINITY)
                            .hint_text("Type to chat, Enter to send"),
                    );

                    if resp.lost_focus() && ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
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
                });

                ui.separator();

                egui::ScrollArea::vertical()
                    .auto_shrink([false; 2])
                    .stick_to_bottom(true)
                    .show(ui, |ui| {
                        if let Some(ps) = app_state.player_state.as_ref() {
                            for i in 0..ps.log_len() {
                                if let Some(msg) = ps.log_message(i) {
                                    let color = match msg.color {
                                        LogMessageColor::Yellow => egui::Color32::YELLOW,
                                        LogMessageColor::Green => egui::Color32::GREEN,
                                        LogMessageColor::Blue => {
                                            egui::Color32::from_rgb(100, 150, 255)
                                        }
                                        LogMessageColor::Red => egui::Color32::RED,
                                    };
                                    ui.colored_label(color, &msg.message);
                                }
                            }
                        }
                    });
            });

        None
    }
}
