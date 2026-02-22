//! The main in-game scene — owns the gameplay HUD, world rendering, input
//! handling, and network event loop.
//!
//! The bulk of the logic is split across submodules for maintainability:
//!
//! | Module | Responsibility |
//! |---|---|
//! | [`profile`] | Load/save per-character preference profiles |
//! | [`game_math`] | Pure geometry, stat-cost formulas, coordinate transforms |
//! | [`world_render`] | Isometric tile/sprite/shadow/effect drawing |
//! | [`ui_render`] | HUD panels: bars, chat, inventory, minimap, shop overlay |
//! | [`input`] | Click-handling for stat/inv/skill/shop panels |
//! | [`net_events`] | Per-frame network tick processing and auto-look |

mod game_math;
mod input;
mod net_events;
mod profile;
mod ui_render;
mod world_render;

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

use mag_core::constants::{ISCHAR, ISITEM, ISUSABLE, TILEX, TILEY};

use crate::{
    network::{client_commands::ClientCommand, NetworkRuntime},
    player_state::PlayerState,
    preferences::{self, CharacterIdentity},
    scenes::scene::{Scene, SceneType},
    state::AppState,
};

// ---------------------------------------------------------------------------
// Layout / tuning constants (all pub(super) so submodules can import them)
// ---------------------------------------------------------------------------

/// Maximum characters allowed in the chat input buffer.
pub(super) const MAX_INPUT_LEN: usize = 120;

/// Maximum complete network tick groups processed per frame.
///
/// A tick group is all `NetworkEvent::Bytes` emitted for one server tick packet,
/// followed by its terminating `NetworkEvent::Tick`. We only stop processing at
/// tick boundaries so map state is never rendered from a partially applied group.
pub(super) const MAX_TICK_GROUPS_PER_FRAME: usize = 32;
pub(super) const QSIZE: u32 = 8;

// ---- Layout constants (ported from engine.c / layout.rs) ---- //

/// Camera X shift to account for the left-hand UI panel.
pub(super) const MAP_X_SHIFT: i32 = -176;

/// Sprite ID of the static 800×600 UI background frame.
pub(super) const UI_FRAME_SPRITE: usize = 1;

/// Default bitmap font index (yellow, sprite 701).
pub(super) const UI_FONT: usize = 1;

// Matches original engine.c worn-slot draw order (wntab[]).
pub(super) const EQUIP_WNTAB: [usize; 12] = [0, 9, 2, 3, 1, 4, 8, 7, 10, 11, 5, 6];

// HP / Endurance / Mana bars
pub(super) const BAR_X: i32 = 373;
pub(super) const BAR_HP_Y: i32 = 127;
pub(super) const BAR_END_Y: i32 = 134;
pub(super) const BAR_MANA_Y: i32 = 141;
pub(super) const BAR_H: u32 = 6;
pub(super) const BAR_SCALE_NUM: i32 = 62;
pub(super) const BAR_W_MAX: i32 = 62;

/// Bar background (capacity).
pub(super) const BAR_BG_COLOR: Color = Color::RGB(9, 4, 58);
/// Bar fill (own character).
pub(super) const BAR_FILL_COLOR: Color = Color::RGB(8, 77, 23);
pub(super) const BAR_FILL_LOOK_COLOR: Color = Color::RGB(140, 20, 20);
pub(super) const MODE_INDICATOR_COLOR: Color = Color::RGB(200, 96, 24);
pub(super) const SCROLL_KNOB_COLOR: Color = Color::RGB(8, 77, 23);

// Stat text positions
pub(super) const STAT_HP_X: i32 = 5;
pub(super) const STAT_HP_Y: i32 = 270;
pub(super) const STAT_END_X: i32 = 5;
pub(super) const STAT_END_Y: i32 = 284;
pub(super) const STAT_MANA_X: i32 = 5;
pub(super) const STAT_MANA_Y: i32 = 298;
pub(super) const STAT_MONEY_X: i32 = 375;
pub(super) const STAT_MONEY_Y: i32 = 190;
pub(super) const STAT_WEAPON_X: i32 = 646;
pub(super) const STAT_WEAPON_Y: i32 = 243;
pub(super) const STAT_ARMOR_X: i32 = 646;
pub(super) const STAT_ARMOR_Y: i32 = 257;
pub(super) const STAT_EXP_X: i32 = 646;
pub(super) const STAT_EXP_Y: i32 = 271;

// Name text (centered in 125px wide area)
pub(super) const NAME_AREA_X: i32 = 374;
pub(super) const NAME_AREA_W: i32 = 125;
pub(super) const NAME_Y: i32 = 28;
pub(super) const PORTRAIT_NAME_Y: i32 = 152;
pub(super) const PORTRAIT_RANK_Y: i32 = 172;

// Chat log area
pub(super) const LOG_X: i32 = 500;
pub(super) const LOG_Y: i32 = 4;
pub(super) const LOG_LINE_H: i32 = 10;
pub(super) const LOG_LINES: usize = 22;
pub(super) const INPUT_X: i32 = 500;
pub(super) const INPUT_Y: i32 = 9 + LOG_LINE_H * (LOG_LINES as i32);

// Minimap
pub(super) const MINIMAP_X: i32 = 3;
pub(super) const MINIMAP_Y: i32 = 471;
pub(super) const MINIMAP_VIEW_SIZE: u32 = 128;
pub(super) const MINIMAP_WORLD_SIZE: usize = 1024;

pub(super) const SCROLL_KNOB_W: u32 = 11;
pub(super) const SCROLL_KNOB_H: u32 = 11;
pub(super) const SKILL_SCROLL_X: i32 = 207;
pub(super) const SKILL_SCROLL_Y_BASE: i32 = 149;
pub(super) const SKILL_SCROLL_RANGE: i32 = 58;
pub(super) const SKILL_SCROLL_MAX: i32 = 40;
pub(super) const INV_SCROLL_X: i32 = 290;
pub(super) const INV_SCROLL_Y_BASE: i32 = 36;
pub(super) const INV_SCROLL_RANGE: i32 = 94;
pub(super) const INV_SCROLL_MAX: i32 = 30;

// ---------------------------------------------------------------------------
// GameScene struct
// ---------------------------------------------------------------------------

/// The primary in-game scene.
///
/// Holds all transient gameplay state: input buffer, modifier-key flags,
/// scroll positions, pending stat raises, minimap pixel buffer, and escape
/// menu state. Created fresh each time the player enters the game world.
pub struct GameScene {
    pub(super) input_buf: String,
    pub(super) pending_exit: Option<String>,
    pub(super) log_scroll: usize,
    pub(super) last_log_len: usize,
    pub(super) ctrl_held: bool,
    pub(super) shift_held: bool,
    pub(super) alt_held: bool,
    pub(super) skill_scroll: usize,
    pub(super) inv_scroll: usize,
    pub(super) mouse_x: i32,
    pub(super) mouse_y: i32,
    /// Pending stat raises not yet committed to the server (indices 0-7 = attrib/HP/End/Mana,
    /// 8-107 = sorted skill positions).
    pub(super) stat_raised: [i32; 108],
    /// Points already spent on pending raises (sum of costs for each `stat_raised[n]`).
    pub(super) stat_points_used: i32,
    /// Persistent 1024×1024 world map for minimap rendering.
    /// Layout: 4 bytes per cell [R,G,B,A], cell index = `(gy + gx * 1024) * 4`.
    /// This matches the C xmap column-major storage: `xmap[map[m].y + map[m].x*1024]`.
    pub(super) minimap_xmap: Vec<u8>,
    pub(super) minimap_last_xy: Option<(u16, u16)>,
    pub(super) look_step: u32,
    pub(super) last_look_tick: u32,
    /// Whether the escape/options menu overlay is currently visible.
    pub(super) escape_menu_open: bool,
    /// Whether spell visual effects (EMAGIC/GMAGIC/CMAGIC glows) are rendered.
    pub(super) are_spell_effects_enabled: bool,
    /// Master volume multiplier (0.0 = muted, 1.0 = full).
    pub(super) master_volume: f32,
    /// When set, the player has right-clicked a skill and is choosing a spell-bar slot.
    /// Value is the skilltab index of the skill being assigned.
    pub(super) pending_skill_assignment: Option<usize>,
    pub(super) active_profile_character: Option<CharacterIdentity>,
}

impl GameScene {
    /// Create a new `GameScene` with default (zeroed) state.
    ///
    /// # Returns
    ///
    /// A fresh `GameScene` ready to be entered via [`Scene::on_enter`].
    pub fn new() -> Self {
        Self {
            input_buf: String::new(),
            pending_exit: None,
            log_scroll: 0,
            last_log_len: 0,
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
            look_step: 0,
            last_look_tick: 0,
            escape_menu_open: false,
            are_spell_effects_enabled: true,
            master_volume: 1.0,
            pending_skill_assignment: None,
            active_profile_character: None,
        }
    }
}

impl Default for GameScene {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Scene trait implementation
// ---------------------------------------------------------------------------

impl Scene for GameScene {
    /// Initialise the game scene: reset all transient state, establish a TCP
    /// connection to the game server via the login ticket, and load the
    /// player's saved profile (skill-button assignments, volume, etc.).
    fn on_enter(&mut self, app_state: &mut AppState) {
        self.input_buf.clear();
        self.pending_exit = None;
        self.log_scroll = 0;
        self.last_log_len = 0;
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
        self.look_step = 0;
        self.last_look_tick = 0;
        self.escape_menu_open = false;
        self.pending_skill_assignment = None;
        self.active_profile_character = None;

        self.are_spell_effects_enabled = true;
        self.master_volume = 1.0;
        app_state.master_volume = self.master_volume;

        let login_target = match app_state.api.login_target.clone() {
            Some(t) => t,
            None => {
                log::error!("GameScene on_enter: no login_target set");
                self.pending_exit = Some("No login target".to_string());
                return;
            }
        };

        log::info!(
            "Using profile JSON at {} (next to log file: {})",
            preferences::profile_file_path().display(),
            preferences::log_file_path().display()
        );

        let host = crate::hosts::get_host_from_api_base_url(&app_state.api.base_url)
            .unwrap_or_else(crate::hosts::get_server_ip);
        log::info!(
            "GameScene: connecting to {}:5555 with ticket={} (api_base_url={})",
            host,
            login_target.ticket,
            app_state.api.base_url
        );

        app_state.network = Some(NetworkRuntime::new(
            host,
            5555,
            login_target.ticket,
            login_target.race,
        ));
        app_state.player_state = Some(PlayerState::default());

        let identity = CharacterIdentity {
            id: login_target.character_id,
            name: login_target.character_name,
            account_username: app_state.api.username.clone(),
        };
        self.apply_loaded_profile(app_state, &identity);
        self.active_profile_character = Some(identity);
    }

    /// Clean up: persist the active profile and shut down the network connection.
    fn on_exit(&mut self, app_state: &mut AppState) {
        self.save_active_profile(app_state);

        if let Some(mut net) = app_state.network.take() {
            net.shutdown();
        }
        app_state.player_state = None;
    }

    /// Dispatch SDL2 events to the appropriate handler.
    ///
    /// Escape toggles the options overlay. Modifier keys are tracked for
    /// shift/ctrl/alt click behaviour. When the escape menu is open all
    /// gameplay input is suppressed.
    ///
    /// # Arguments
    ///
    /// * `app_state` - Shared application state.
    /// * `event` - The SDL2 event to handle.
    ///
    /// # Returns
    ///
    /// `Some(SceneType)` to trigger a scene transition, or `None` to stay.
    fn handle_event(&mut self, app_state: &mut AppState, event: &Event) -> Option<SceneType> {
        // --- Escape key: always processed regardless of menu state ---
        if let Event::KeyDown {
            keycode: Some(Keycode::Escape),
            ..
        } = event
        {
            // Always send CmdReset (preserving legacy behavior).
            if let Some(net) = app_state.network.as_ref() {
                net.send(ClientCommand::new_reset());
            }
            self.escape_menu_open = !self.escape_menu_open;
            // Clear pending skill assignment when toggling menu.
            if self.escape_menu_open {
                self.pending_skill_assignment = None;
            }
            return None;
        }

        // --- Modifier key tracking: always processed so state stays correct ---
        match event {
            Event::KeyDown {
                keycode: Some(kc), ..
            } => match *kc {
                Keycode::LCtrl | Keycode::RCtrl => {
                    self.ctrl_held = true;
                    return None;
                }
                Keycode::LShift | Keycode::RShift => {
                    self.shift_held = true;
                    return None;
                }
                Keycode::LAlt | Keycode::RAlt => {
                    self.alt_held = true;
                    return None;
                }
                _ => {}
            },
            Event::KeyUp {
                keycode: Some(kc), ..
            } => match *kc {
                Keycode::LCtrl | Keycode::RCtrl => {
                    self.ctrl_held = false;
                    return None;
                }
                Keycode::LShift | Keycode::RShift => {
                    self.shift_held = false;
                    return None;
                }
                Keycode::LAlt | Keycode::RAlt => {
                    self.alt_held = false;
                    return None;
                }
                _ => {}
            },
            Event::MouseMotion { x, y, .. } => {
                self.mouse_x = *x;
                self.mouse_y = *y;
                return None;
            }
            _ => {}
        }

        // --- When escape menu is open, block all other game input ---
        if self.escape_menu_open {
            return None;
        }

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
                        self.save_active_profile(app_state);
                    }
                }
                Keycode::F6 => {
                    if let Some(ps) = app_state.player_state.as_mut() {
                        let current = ps.player_data().hide;
                        ps.player_data_mut().hide = 1 - current;
                        self.save_active_profile(app_state);
                    }
                }
                Keycode::F7 => {
                    if let Some(ps) = app_state.player_state.as_mut() {
                        let current = ps.player_data().show_names;
                        ps.player_data_mut().show_names = 1 - current;
                        self.save_active_profile(app_state);
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
            Event::KeyUp { .. } => {
                // Modifier keys handled above the menu gate; nothing else needed.
            }
            Event::TextInput { text, .. } => {
                if self.input_buf.len() + text.len() <= MAX_INPUT_LEN {
                    self.input_buf.push_str(text);
                }
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

    /// Process pending network events and advance the auto-look timer.
    ///
    /// # Returns
    ///
    /// `Some(SceneType)` if a disconnect or exit was signalled, otherwise `None`.
    fn update(&mut self, app_state: &mut AppState, _dt: Duration) -> Option<SceneType> {
        let scene = self.process_network_events(app_state);
        if scene.is_none() {
            let tick_now = app_state
                .network
                .as_ref()
                .map(|net| net.client_ticker)
                .unwrap_or(0);
            if tick_now != self.last_look_tick {
                self.last_look_tick = tick_now;
                self.maybe_send_autolook_and_shop_refresh(app_state);
            }
        }
        scene
    }

    /// Render the isometric world, all HUD panels, and overlay effects.
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
        let shadows_on = ps.player_data().are_shadows_enabled != 0;
        let effects_on = self.are_spell_effects_enabled;
        Self::draw_world(canvas, gfx_cache, ps, shadows_on, effects_on)?;

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

        // 9. Portrait/shop overlays
        Self::draw_portrait_panel(canvas, gfx_cache, ps)?;
        self.draw_shop_overlay(canvas, gfx_cache, ps)?;

        // 10. Hover highlights (suppressed while escape menu is open)
        if !self.escape_menu_open {
            self.draw_hover_effects(canvas, gfx_cache, ps)?;
        }

        // 11. Minimap (bottom-left, 128×128, persistent world buffer)
        self.draw_minimap(canvas, gfx_cache, ps)?;

        // 12. Skill button labels (4×3 grid in lower-right)
        self.draw_skill_button_labels(canvas, gfx_cache, ps)?;

        Ok(())
    }

    /// Render the egui escape/options overlay (shadows, effects, volume, disconnect/quit).
    ///
    /// # Returns
    ///
    /// `Some(SceneType)` if the player chose to disconnect or quit, otherwise `None`.
    fn render_ui(&mut self, app_state: &mut AppState, ctx: &egui::Context) -> Option<SceneType> {
        if !self.escape_menu_open {
            return None;
        }

        let mut scene_change: Option<SceneType> = None;
        let mut profile_changed = false;

        egui::Window::new("Options")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.heading("Settings");
                ui.separator();

                // Shadows toggle
                let mut shadows = if let Some(ps) = app_state.player_state.as_ref() {
                    ps.player_data().are_shadows_enabled != 0
                } else {
                    false
                };
                if ui.checkbox(&mut shadows, "Enable Shadows").changed() {
                    if let Some(ps) = app_state.player_state.as_mut() {
                        ps.player_data_mut().are_shadows_enabled = if shadows { 1 } else { 0 };
                        profile_changed = true;
                    }
                }

                // Spell effects toggle
                if ui
                    .checkbox(&mut self.are_spell_effects_enabled, "Enable Spell Effects")
                    .changed()
                {
                    profile_changed = true;
                }

                ui.separator();

                let latest_rtt = app_state
                    .network
                    .as_ref()
                    .and_then(|net| net.last_rtt_ms)
                    .map(|value| format!("{} ms", value))
                    .unwrap_or_else(|| "N/A".to_string());
                ui.label(format!("Latest RTT: {}", latest_rtt));

                // Volume slider
                if ui
                    .add(
                        egui::Slider::new(&mut self.master_volume, 0.0..=1.0)
                            .text("Volume")
                            .show_value(true),
                    )
                    .changed()
                {
                    profile_changed = true;
                }
                // Sync to AppState so SFX playback uses it.
                app_state.master_volume = self.master_volume;

                ui.separator();

                if ui.button("Open Log Directory").clicked() {
                    let log_dir = preferences::log_file_path()
                        .parent()
                        .map(|p| p.to_path_buf())
                        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
                    crate::platform::open_directory_in_file_manager(&log_dir);
                }

                ui.separator();

                if ui.button("Disconnect").clicked() {
                    scene_change = Some(SceneType::CharacterSelection);
                }
                if ui.button("Quit").clicked() {
                    scene_change = Some(SceneType::Exit);
                }
            });

        if profile_changed {
            self.save_active_profile(app_state);
        }

        scene_change
    }
}
