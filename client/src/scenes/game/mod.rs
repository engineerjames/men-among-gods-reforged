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
mod perf_profiler;
mod profile;
mod ui_render;
mod world_render;

use perf_profiler::{PerfLabel, PerfProfiler};

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
    cert_trust,
    constants::TARGET_HEIGHT_INT,
    network::{client_commands::ClientCommand, NetworkRuntime},
    player_state::PlayerState,
    preferences::{self, CharacterIdentity, DisplayMode},
    scenes::scene::{Scene, SceneType},
    state::{AppState, DisplayCommand},
    ui::{
        chat_box::ChatBox,
        hud_button_bar::HudButtonBar,
        inventory_panel::InventoryPanel,
        look_panel::LookPanel,
        minimap_widget::MinimapWidget,
        mode_button::ModeButton,
        rank_arc::RankArc,
        settings_panel::SettingsPanel,
        shop_panel::ShopPanel,
        skills_panel::SkillsPanel,
        status_panel::StatusPanel,
        style::Padding,
        widget::{
            Bounds, EventResponse, HudPanel, KeyModifiers, MouseButton as UiMouseButton, UiEvent,
            Widget, WidgetAction,
        },
        RenderContext,
    },
};

// ---------------------------------------------------------------------------
// Layout / tuning constants (all pub(super) so submodules can import them)
// ---------------------------------------------------------------------------

/// Maximum complete network tick groups processed per frame.
///
/// A tick group is all `NetworkEvent::Bytes` emitted for one server tick packet,
/// followed by its terminating `NetworkEvent::Tick`. We only stop processing at
/// tick boundaries so map state is never rendered from a partially applied group.
pub(super) const MAX_TICK_GROUPS_PER_FRAME: usize = 32;
pub(super) const QSIZE: u32 = 8;

// ---- Layout constants (ported from engine.c / layout.rs) ---- //

/// Width in pixels of one ground diamond.
pub(super) const FLOOR_TILE_WIDTH: i32 = 32;

/// Height in pixels of one ground diamond.
pub(super) const FLOOR_TILE_HEIGHT: i32 = 16;

/// Optional X nudge applied after centering (positive = right).
pub(super) const MAP_X_TWEAK: i32 = 0;

/// Optional Y nudge applied after centering (positive = down).
pub(super) const MAP_Y_TWEAK: i32 = 0;

/// X origin offset that places tile (TILEX/2, TILEY/2) at the horizontal
/// center of the logical viewport.
pub(super) const MAP_ORIGIN_X: i32 = (crate::constants::TARGET_WIDTH_INT as i32) / 2
    - ((TILEX / 2) as i32 * (FLOOR_TILE_WIDTH / 2)
        + (TILEY / 2) as i32 * (FLOOR_TILE_WIDTH / 2)
        + FLOOR_TILE_WIDTH)
    + MAP_X_TWEAK;

/// Y origin offset that places tile (TILEX/2, TILEY/2) at the vertical
/// center of the logical viewport.
pub(super) const MAP_ORIGIN_Y: i32 = (crate::constants::TARGET_HEIGHT_INT as i32) / 2
    - (FLOOR_TILE_HEIGHT / 2)
    - ((TILEX / 2) as i32 * (FLOOR_TILE_WIDTH / 4) - (TILEY / 2) as i32 * (FLOOR_TILE_WIDTH / 4))
    + MAP_Y_TWEAK;

const CHATBOX_X: i32 = 0;
const CHATBOX_Y: i32 = TARGET_HEIGHT_INT as i32 - CHATBOX_H as i32;
const CHATBOX_W: u32 = 300;
const CHATBOX_H: u32 = 200;

// ---- HUD button bar layout ---- //

/// X center of the invisible arc that the HUD buttons sit on.
const HUD_ARC_CENTER_X: i32 = crate::constants::TARGET_WIDTH_INT as i32 / 2;
/// Y center of the invisible arc (at the bottom edge of the viewport).
const HUD_ARC_CENTER_Y: i32 = crate::constants::TARGET_HEIGHT_INT as i32;
/// Radius of the invisible layout arc.
const HUD_ARC_RADIUS: u32 = 60;
/// Radius of each individual HUD button.
const HUD_BUTTON_RADIUS: u32 = 16;
/// Sprite IDs for [Skills, Inventory, Settings] buttons.
const HUD_SPRITE_IDS: [usize; 3] = [267, 128, 35];
/// Width of each togglable HUD panel.
const HUD_PANEL_W: u32 = 300;
/// Height of each togglable HUD panel.
const HUD_PANEL_H: u32 = 250;
/// Wider width for the inventory panel (two grids + scrollbar + gap).
const INV_PANEL_W: u32 = 190;
/// Taller height for the inventory panel.
const INV_PANEL_H: u32 = 280;
/// Semi-transparent background color shared by all HUD panels.
const HUD_PANEL_BG: Color = Color::RGBA(10, 10, 30, 180);

// ---- Minimap toggle button ---- //

/// X center of the minimap toggle button (near top-right of screen).
const MINIMAP_BTN_CX: i32 = crate::constants::TARGET_WIDTH_INT as i32 - 30;
/// Y center of the minimap toggle button.
const MINIMAP_BTN_CY: i32 = 30;
/// Radius of the minimap toggle button.
const MINIMAP_BTN_RADIUS: u32 = 14;

// ---- Mode button (lower-right) ---- //

/// X center of the circular speed-mode button.
const MODE_BTN_CX: i32 = crate::constants::TARGET_WIDTH_INT as i32 - 30;
/// Y center of the circular speed-mode button.
const MODE_BTN_CY: i32 = crate::constants::TARGET_HEIGHT_INT as i32 - 30;
/// Radius of the circular speed-mode button.
const MODE_BTN_RADIUS: u32 = 18;

// ---- Look panel (center-right) ---- //

/// Width of the look panel.
const LOOK_PANEL_W: u32 = 180;
/// Height of the look panel.
const LOOK_PANEL_H: u32 = 260;
/// X position of the look panel (right side, 4 px margin).
const LOOK_PANEL_X: i32 = crate::constants::TARGET_WIDTH_INT as i32 - LOOK_PANEL_W as i32 - 4;
/// Y position of the look panel (vertically centered).
const LOOK_PANEL_Y: i32 = (crate::constants::TARGET_HEIGHT_INT as i32 - LOOK_PANEL_H as i32) / 4;

// ---- Shop panel (centered on screen) ---- //

/// Width of the shop panel.
const SHOP_PANEL_W: u32 = crate::ui::shop_panel::SHOP_PANEL_W;
/// Height of the shop panel.
const SHOP_PANEL_H: u32 = crate::ui::shop_panel::SHOP_PANEL_H;
/// X position of the shop panel (horizontally centered).
const SHOP_PANEL_X: i32 = (crate::constants::TARGET_WIDTH_INT as i32 - SHOP_PANEL_W as i32) / 2;
/// Y position of the shop panel (vertically centered).
const SHOP_PANEL_Y: i32 = (crate::constants::TARGET_HEIGHT_INT as i32 - SHOP_PANEL_H as i32) / 2;

// Minimap
pub(super) const MINIMAP_WORLD_SIZE: usize = 1024;

// ---------------------------------------------------------------------------
// GameScene struct
// ---------------------------------------------------------------------------

/// The primary in-game scene.
///
/// Holds all transient gameplay state: input buffer, modifier-key flags,
/// scroll positions, pending stat raises, minimap pixel buffer, and escape
/// menu state. Created fresh each time the player enters the game world.
pub struct GameScene {
    pub(super) status_panel: StatusPanel,
    pub(super) chat_box: ChatBox,
    pub(super) hud_buttons: HudButtonBar,
    pub(super) rank_arc: RankArc,
    pub(super) skills_panel: SkillsPanel,
    pub(super) inventory_panel: InventoryPanel,
    pub(super) settings_panel: SettingsPanel,
    pub(super) minimap_widget: MinimapWidget,
    pub(super) mode_button: ModeButton,
    pub(super) look_panel: LookPanel,
    pub(super) shop_panel: ShopPanel,
    pub(super) last_synced_log_len: usize,
    pub(super) pending_exit: Option<String>,
    pub(super) certificate_mismatch: Option<cert_trust::FingerprintMismatch>,
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
    /// Wall-clock profiler for rendering functions (activated from escape menu).
    perf_profiler: PerfProfiler,
}

impl GameScene {
    /// Create a new `GameScene` with default (zeroed) state.
    ///
    /// # Returns
    ///
    /// A fresh `GameScene` ready to be entered via [`Scene::on_enter`].
    pub fn new() -> Self {
        // HUD panels are centered horizontally, positioned so their bottom
        // edge sits 20 px above the top of the button arc.
        let panel_x = HUD_ARC_CENTER_X - HUD_PANEL_W as i32 / 2;
        let panel_bottom = HUD_ARC_CENTER_Y - HUD_ARC_RADIUS as i32 - HUD_BUTTON_RADIUS as i32 - 20;
        let panel_y = panel_bottom - HUD_PANEL_H as i32;

        Self {
            status_panel: StatusPanel::new(4, 4, HUD_PANEL_BG),
            chat_box: ChatBox::new(
                Bounds::new(CHATBOX_X, CHATBOX_Y, CHATBOX_W, CHATBOX_H),
                Color::RGBA(10, 10, 30, 180),
                Padding::uniform(4),
            ),
            hud_buttons: HudButtonBar::new(
                HUD_ARC_CENTER_X,
                HUD_ARC_CENTER_Y,
                HUD_ARC_RADIUS,
                HUD_BUTTON_RADIUS,
                HUD_SPRITE_IDS,
            ),
            rank_arc: RankArc::new(HUD_ARC_CENTER_X, HUD_ARC_CENTER_Y, 30, 2),
            skills_panel: SkillsPanel::new(
                Bounds::new(panel_x, panel_y, HUD_PANEL_W, HUD_PANEL_H),
                HUD_PANEL_BG,
            ),
            inventory_panel: InventoryPanel::new(
                Bounds::new(
                    HUD_ARC_CENTER_X - INV_PANEL_W as i32 / 2,
                    panel_bottom - INV_PANEL_H as i32,
                    INV_PANEL_W,
                    INV_PANEL_H,
                ),
                HUD_PANEL_BG,
            ),
            settings_panel: SettingsPanel::new(
                Bounds::new(panel_x, panel_y, HUD_PANEL_W, HUD_PANEL_H),
                HUD_PANEL_BG,
            ),
            minimap_widget: MinimapWidget::new(MINIMAP_BTN_CX, MINIMAP_BTN_CY, MINIMAP_BTN_RADIUS),
            mode_button: ModeButton::new(MODE_BTN_CX, MODE_BTN_CY, MODE_BTN_RADIUS),
            look_panel: LookPanel::new(
                Bounds::new(LOOK_PANEL_X, LOOK_PANEL_Y, LOOK_PANEL_W, LOOK_PANEL_H),
                HUD_PANEL_BG,
            ),
            shop_panel: ShopPanel::new(
                Bounds::new(SHOP_PANEL_X, SHOP_PANEL_Y, SHOP_PANEL_W, SHOP_PANEL_H),
                HUD_PANEL_BG,
            ),
            last_synced_log_len: 0,
            pending_exit: None,
            certificate_mismatch: None,
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
            perf_profiler: PerfProfiler::new(),
        }
    }

    /// Resolve the default skill target.
    ///
    /// Priority matches expected gameplay behavior:
    /// 1) Explicitly selected character (Alt+click)
    /// 2) Current attack target (`attack_cn`)
    /// 3) No target (0)
    pub(super) fn default_skill_target(ps: &PlayerState) -> u32 {
        let selected = ps.selected_char() as u32;
        if selected != 0 {
            return selected;
        }

        ps.character_info().attack_cn.max(0) as u32
    }

    pub(super) fn play_click_sound(&self, app_state: &AppState) {
        app_state.sfx_cache.play_click(self.master_volume);
    }

    /// Translate an SDL2 event into a UI-framework `UiEvent`, if applicable.
    ///
    /// # Arguments
    ///
    /// * `event` - The SDL2 event.
    /// * `mouse_x` - Current logical mouse X position.
    /// * `mouse_y` - Current logical mouse Y position.
    /// * `modifiers` - Current modifier key state.
    ///
    /// # Returns
    ///
    /// `Some(UiEvent)` for events the widget system cares about, `None` otherwise.
    fn sdl_to_ui_event(
        event: &Event,
        mouse_x: i32,
        mouse_y: i32,
        modifiers: KeyModifiers,
    ) -> Option<UiEvent> {
        match event {
            Event::MouseWheel { y, .. } => Some(UiEvent::MouseWheel {
                x: mouse_x,
                y: mouse_y,
                delta: *y,
            }),
            Event::MouseButtonUp {
                mouse_btn, x, y, ..
            } => {
                let button = match mouse_btn {
                    MouseButton::Left => UiMouseButton::Left,
                    MouseButton::Right => UiMouseButton::Right,
                    MouseButton::Middle => UiMouseButton::Middle,
                    _ => return None,
                };
                Some(UiEvent::MouseClick {
                    x: *x,
                    y: *y,
                    button,
                    modifiers,
                })
            }
            Event::TextInput { text, .. } => Some(UiEvent::TextInput { text: text.clone() }),
            Event::KeyDown {
                keycode: Some(kc),
                keymod,
                ..
            } => Some(UiEvent::KeyDown {
                keycode: *kc,
                modifiers: KeyModifiers::from_sdl2(*keymod),
            }),
            Event::MouseMotion { x, y, .. } => Some(UiEvent::MouseMove { x: *x, y: *y }),
            _ => None,
        }
    }

    /// Drain pending `WidgetAction`s from the chat box and act on them.
    ///
    /// Currently the only action is `SendChat`, which sends say-packets
    /// through the network runtime.
    ///
    /// # Arguments
    ///
    /// * `app_state` - Shared application state (network access).
    fn process_chat_box_actions(&mut self, app_state: &AppState) {
        for action in self.chat_box.take_actions() {
            match action {
                WidgetAction::SendChat(text) => {
                    if let Some(net) = app_state.network.as_ref() {
                        for pkt in ClientCommand::new_say_packets(text.as_bytes()) {
                            net.send(pkt);
                        }
                    }
                }
                WidgetAction::TogglePanel(_)
                | WidgetAction::CommitStats { .. }
                | WidgetAction::CastSkill { .. }
                | WidgetAction::BeginSkillAssign { .. }
                | WidgetAction::BindSkillKey { .. }
                | WidgetAction::InvAction { .. }
                | WidgetAction::InvLookAction { .. }
                | WidgetAction::ChangeMode(_)
                | WidgetAction::ShopAction { .. }
                | WidgetAction::CloseShop => {}
            }
        }
    }

    /// Drain pending `WidgetAction`s from the mode button and send mode
    /// commands to the server.
    ///
    /// # Arguments
    ///
    /// * `app_state` - Shared application state (network access).
    fn process_mode_button_actions(&mut self, app_state: &AppState) {
        for action in self.mode_button.take_actions() {
            if let WidgetAction::ChangeMode(mode) = action {
                if let Some(net) = app_state.network.as_ref() {
                    net.send(ClientCommand::new_mode(mode as i16));
                }
            }
        }
    }

    /// Drain and process actions produced by the skills panel.
    ///
    /// # Arguments
    ///
    /// * `app_state` - Shared application state (network access).
    fn process_skills_panel_actions(&mut self, app_state: &mut AppState) {
        for action in self.skills_panel.take_actions() {
            match action {
                WidgetAction::CommitStats { raises } => {
                    if let Some(net) = app_state.network.as_ref() {
                        for (which, value) in raises {
                            net.send(ClientCommand::new_stat(which, value));
                        }
                    }
                }
                WidgetAction::CastSkill { skill_nr } => {
                    if let (Some(net), Some(ps)) =
                        (app_state.network.as_ref(), app_state.player_state.as_ref())
                    {
                        let target = Self::default_skill_target(ps);
                        let a0 = ps.character_info().attrib[0][5] as u32;
                        net.send(ClientCommand::new_skill(skill_nr, target, a0));
                    }
                }
                WidgetAction::BeginSkillAssign { skill_id } => {
                    self.pending_skill_assignment = Some(skill_id);
                }
                WidgetAction::BindSkillKey { skill_nr, key_slot } => {
                    if let Some(ps) = app_state.player_state.as_mut() {
                        // Clear any previous slot that had the same skill_nr.
                        for slot in ps.player_data_mut().skill_keybinds.iter_mut() {
                            if *slot == Some(skill_nr) {
                                *slot = None;
                            }
                        }
                        ps.player_data_mut().skill_keybinds[key_slot as usize] = Some(skill_nr);
                        let name = mag_core::types::skilltab::get_skill_name(skill_nr as usize);
                        ps.tlog(1, &format!("Bound {} to Ctrl+{}.", name, key_slot + 1));
                    }
                    self.save_active_profile(app_state);
                }
                WidgetAction::SendChat(_)
                | WidgetAction::TogglePanel(_)
                | WidgetAction::InvAction { .. }
                | WidgetAction::InvLookAction { .. }
                | WidgetAction::ChangeMode(_)
                | WidgetAction::ShopAction { .. }
                | WidgetAction::CloseShop => {}
            }
        }
    }

    /// Drain pending `WidgetAction`s from the inventory panel and send the
    /// corresponding network commands.
    ///
    /// # Arguments
    ///
    /// * `app_state` - Shared application state (network access).
    fn process_inventory_panel_actions(&mut self, app_state: &AppState) {
        for action in self.inventory_panel.take_actions() {
            match action {
                WidgetAction::InvAction {
                    a,
                    b,
                    selected_char,
                } => {
                    if let Some(net) = app_state.network.as_ref() {
                        self.play_click_sound(app_state);
                        net.send(ClientCommand::new_inv(a, b, selected_char));
                    }
                }
                WidgetAction::InvLookAction { a, b, c } => {
                    if let Some(net) = app_state.network.as_ref() {
                        self.play_click_sound(app_state);
                        net.send(ClientCommand::new_inv_look(a, b, c));
                    }
                }
                _ => {}
            }
        }
    }

    /// Drain pending `WidgetAction`s from the shop panel and send the
    /// corresponding network commands, or close the shop.
    ///
    /// # Arguments
    ///
    /// * `app_state` - Shared application state (network + player state).
    fn process_shop_panel_actions(&mut self, app_state: &mut AppState) {
        for action in self.shop_panel.take_actions() {
            match action {
                WidgetAction::ShopAction { shop_nr, action } => {
                    if let Some(net) = app_state.network.as_ref() {
                        self.play_click_sound(app_state);
                        net.send(ClientCommand::new_shop(shop_nr, action));
                    }
                }
                WidgetAction::CloseShop => {
                    if let Some(ps) = app_state.player_state.as_mut() {
                        ps.close_shop();
                    }
                }
                _ => {}
            }
        }
    }

    /// Forward any new log messages from `PlayerState` into the `ChatBox`.
    ///
    /// Messages are fetched in insertion order (oldest-first) starting from
    /// `last_synced_log_len` so the ChatBox receives them chronologically.
    ///
    /// # Arguments
    ///
    /// * `ps` - The current player state with the authoritative message log.
    fn sync_chat_messages(&mut self, ps: &PlayerState) {
        let total = ps.log_len();
        if total <= self.last_synced_log_len {
            return;
        }
        let new_messages =
            (self.last_synced_log_len..total).filter_map(|i| ps.log_message(i).cloned());
        self.chat_box.push_messages(new_messages);
        self.last_synced_log_len = total;
    }

    fn is_selected_visible(ps: &PlayerState) -> bool {
        let selected = ps.selected_char();
        if selected == 0 {
            return true;
        }

        for y in 0..TILEY {
            for x in 0..TILEX {
                if let Some(tile) = ps.map().tile_at_xy(x, y) {
                    if tile.ch_nr == selected {
                        return true;
                    }
                }
            }
        }

        false
    }

    /// Starts (or restarts) the game network session from the current login target.
    ///
    /// # Arguments
    ///
    /// * `app_state` - Shared application state with API login target and session.
    ///
    /// # Returns
    ///
    /// * `Ok(())` if the network runtime is started.
    /// * `Err(String)` when required login target data is missing.
    fn start_game_network_session(&mut self, app_state: &mut AppState) -> Result<(), String> {
        let login_target = app_state
            .api
            .login_target
            .clone()
            .ok_or_else(|| "No login target".to_string())?;

        let host = crate::hosts::get_host_from_api_base_url(&app_state.api.base_url)
            .unwrap_or_else(crate::hosts::get_server_ip);
        let use_tls = app_state.api.base_url.starts_with("https://");

        log::info!(
            "GameScene: connecting to {}:5555 with ticket={} tls={} (api_base_url={})",
            host,
            login_target.ticket,
            use_tls,
            app_state.api.base_url
        );

        if let Some(mut net) = app_state.network.take() {
            net.shutdown();
        }

        app_state.network = Some(NetworkRuntime::new(
            host,
            5555,
            login_target.ticket,
            login_target.race,
            use_tls,
        ));

        app_state.player_state = Some(PlayerState::default());
        self.pending_exit = None;
        self.certificate_mismatch = None;
        Ok(())
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
        self.chat_box = ChatBox::new(
            Bounds::new(CHATBOX_X, CHATBOX_Y, CHATBOX_W, CHATBOX_H),
            Color::RGBA(10, 10, 30, 180),
            Padding::uniform(4),
        );
        self.last_synced_log_len = 0;
        self.pending_exit = None;
        self.certificate_mismatch = None;
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

        if let Err(err) = self.start_game_network_session(app_state) {
            log::error!(
                "GameScene on_enter: failed to start network session: {}",
                err
            );
            self.pending_exit = Some(err);
            return;
        }

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
                self.play_click_sound(app_state);
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
            }
            _ => {}
        }

        // --- When escape menu is open, block all other game input ---
        if self.escape_menu_open {
            return None;
        }

        // --- Dispatch to ChatBox first; if consumed, act on pending actions ---
        if let Some(ui_event) = Self::sdl_to_ui_event(
            event,
            self.mouse_x,
            self.mouse_y,
            KeyModifiers {
                ctrl: self.ctrl_held,
                shift: self.shift_held,
                alt: self.alt_held,
            },
        ) {
            // --- StatusPanel toggle (upper-left sigil) ---
            if self.status_panel.handle_event(&ui_event) == EventResponse::Consumed {
                return None;
            }

            if self.chat_box.handle_event(&ui_event) == EventResponse::Consumed {
                self.process_chat_box_actions(app_state);
                return None;
            }

            // --- Dispatch to open HUD panels (eat clicks so they don't reach the world) ---
            if self.skills_panel.handle_event(&ui_event) == EventResponse::Consumed {
                self.process_skills_panel_actions(app_state);
                return None;
            }
            if self.inventory_panel.handle_event(&ui_event) == EventResponse::Consumed {
                self.process_inventory_panel_actions(app_state);
                return None;
            }
            if self.settings_panel.handle_event(&ui_event) == EventResponse::Consumed {
                return None;
            }

            // --- Dispatch to shop/depot/grave overlay (modal — eats outside clicks) ---
            if self.shop_panel.handle_event(&ui_event) == EventResponse::Consumed {
                self.process_shop_panel_actions(app_state);
                return None;
            }

            // --- Dispatch to minimap toggle button / panel ---
            if self.minimap_widget.handle_event(&ui_event) == EventResponse::Consumed {
                return None;
            }

            // --- Dispatch to mode button ---
            if self.mode_button.handle_event(&ui_event) == EventResponse::Consumed {
                self.process_mode_button_actions(app_state);
                return None;
            }

            // --- Dispatch to look panel ---
            if self.look_panel.handle_event(&ui_event) == EventResponse::Consumed {
                return None;
            }

            // --- Dispatch to HUD button bar ---
            if self.hud_buttons.handle_event(&ui_event) == EventResponse::Consumed {
                for action in self.hud_buttons.take_actions() {
                    if let WidgetAction::TogglePanel(panel) = action {
                        match panel {
                            HudPanel::Skills => self.skills_panel.toggle(),
                            HudPanel::Inventory => self.inventory_panel.toggle(),
                            HudPanel::Settings => self.settings_panel.toggle(),
                            HudPanel::Minimap => self.minimap_widget.toggle(),
                        }
                    }
                }
                return None;
            }
        }

        match event {
            Event::KeyDown {
                keycode: Some(kc),
                keymod,
                ..
            } => match *kc {
                Keycode::F1 => {
                    if keymod.intersects(Mod::LSHIFTMOD | Mod::RSHIFTMOD) {
                        if let (Some(net), Some(ps)) =
                            (app_state.network.as_ref(), app_state.player_state.as_ref())
                        {
                            let btn = ps.player_data().skill_buttons[0];
                            if !btn.is_unassigned() {
                                self.play_click_sound(app_state);
                                net.send(ClientCommand::new_skill(
                                    btn.skill_nr(),
                                    Self::default_skill_target(ps),
                                    ps.character_info().attrib[0][0] as u32,
                                ));
                            }
                        }
                    }
                }
                Keycode::F2 => {
                    if keymod.intersects(Mod::LSHIFTMOD | Mod::RSHIFTMOD) {
                        if let (Some(net), Some(ps)) =
                            (app_state.network.as_ref(), app_state.player_state.as_ref())
                        {
                            let btn = ps.player_data().skill_buttons[1];
                            if !btn.is_unassigned() {
                                self.play_click_sound(app_state);
                                net.send(ClientCommand::new_skill(
                                    btn.skill_nr(),
                                    Self::default_skill_target(ps),
                                    ps.character_info().attrib[0][0] as u32,
                                ));
                            }
                        }
                    }
                }
                Keycode::F3 => {
                    if keymod.intersects(Mod::LSHIFTMOD | Mod::RSHIFTMOD) {
                        if let (Some(net), Some(ps)) =
                            (app_state.network.as_ref(), app_state.player_state.as_ref())
                        {
                            let btn = ps.player_data().skill_buttons[2];
                            if !btn.is_unassigned() {
                                self.play_click_sound(app_state);
                                net.send(ClientCommand::new_skill(
                                    btn.skill_nr(),
                                    Self::default_skill_target(ps),
                                    ps.character_info().attrib[0][0] as u32,
                                ));
                            }
                        }
                    }
                }
                Keycode::F12 => {
                    if keymod.intersects(Mod::LSHIFTMOD | Mod::RSHIFTMOD) {
                        if let (Some(net), Some(ps)) =
                            (app_state.network.as_ref(), app_state.player_state.as_ref())
                        {
                            let btn = ps.player_data().skill_buttons[11];
                            if !btn.is_unassigned() {
                                self.play_click_sound(app_state);
                                net.send(ClientCommand::new_skill(
                                    btn.skill_nr(),
                                    Self::default_skill_target(ps),
                                    ps.character_info().attrib[0][0] as u32,
                                ));
                            }
                        }
                    } else if let Some(net) = app_state.network.as_ref() {
                        self.play_click_sound(app_state);
                        net.send(ClientCommand::new_exit());
                    }
                }
                Keycode::Num1
                | Keycode::Num2
                | Keycode::Num3
                | Keycode::Num4
                | Keycode::Num5
                | Keycode::Num6
                | Keycode::Num7
                | Keycode::Num8
                | Keycode::Num9 => {
                    if self.ctrl_held {
                        let key_slot = (i32::from(*kc) - i32::from(Keycode::Num1)) as usize;
                        if let (Some(net), Some(ps)) =
                            (app_state.network.as_ref(), app_state.player_state.as_ref())
                        {
                            if let Some(skill_nr) = ps.player_data().skill_keybinds[key_slot] {
                                self.play_click_sound(app_state);
                                net.send(ClientCommand::new_skill(
                                    skill_nr,
                                    Self::default_skill_target(ps),
                                    ps.character_info().attrib[0][0] as u32,
                                ));
                            }
                        }
                    }
                }
                _ => {}
            },
            Event::KeyUp { .. } => {
                // Modifier keys handled above the menu gate; nothing else needed.
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
                            } else {
                                ps_mut.clear_selected_char();
                            }
                        }
                    }
                    MouseButton::Right if has_alt => {
                        if target_cn != 0 {
                            self.play_click_sound(app_state);
                            net.send(ClientCommand::new_look(target_cn));
                        }
                    }
                    MouseButton::Left if has_ctrl => {
                        if target_cn != 0 {
                            self.play_click_sound(app_state);
                            if citem != 0 {
                                net.send(ClientCommand::new_give(target_cn));
                            } else {
                                net.send(ClientCommand::new_attack(target_cn));
                            }
                        }
                    }
                    MouseButton::Right if has_ctrl => {
                        if target_cn != 0 {
                            self.play_click_sound(app_state);
                            net.send(ClientCommand::new_look(target_cn));
                        }
                    }
                    MouseButton::Left if has_shift => {
                        let tile_flags = tile.map(|t| t.flags).unwrap_or(0);
                        let is_item = (tile_flags & ISITEM) != 0;
                        let is_usable = (tile_flags & ISUSABLE) != 0;
                        if citem != 0 && !is_item {
                            // Holding item, clicked non-item tile → drop
                            self.play_click_sound(app_state);
                            net.send(ClientCommand::new_drop(world_x, world_y));
                        } else if is_item && is_usable {
                            // Item is usable → use
                            self.play_click_sound(app_state);
                            net.send(ClientCommand::new_use(world_x, world_y));
                        } else if is_item {
                            // Item not usable → pickup
                            self.play_click_sound(app_state);
                            net.send(ClientCommand::new_pickup(world_x, world_y));
                        }
                    }
                    MouseButton::Right if has_shift => {
                        self.play_click_sound(app_state);
                        net.send(ClientCommand::new_look_item(world_x, world_y));
                    }
                    MouseButton::Left => {
                        self.play_click_sound(app_state);
                        net.send(ClientCommand::new_move(world_x, world_y));
                    }
                    MouseButton::Right => {
                        self.play_click_sound(app_state);
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
                    let step = (dy.unsigned_abs() as usize) * 2;
                    if dy > 0 {
                        self.inv_scroll = self.inv_scroll.saturating_sub(step);
                    } else if dy < 0 {
                        self.inv_scroll = (self.inv_scroll + step).min(30);
                    }
                    // Keep inventory index aligned to the left column (C client uses inv_pos +=/-= 2).
                    self.inv_scroll &= !1usize;
                }
                // Chat area scroll is handled by ChatBox above.
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
    fn update(&mut self, app_state: &mut AppState, dt: Duration) -> Option<SceneType> {
        self.chat_box.update(dt);
        self.status_panel.update(dt);
        self.skills_panel.update(dt);
        self.inventory_panel.update(dt);
        self.settings_panel.update(dt);
        self.mode_button.update(dt);
        self.shop_panel.update(dt);
        self.perf_profiler.check_expired();
        let scene = self.process_network_events(app_state);
        if scene.is_none() {
            if let Some(ps) = app_state.player_state.as_mut() {
                if !Self::is_selected_visible(ps) {
                    ps.clear_selected_char();
                }
            }

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

        // Sync new log messages from PlayerState into the ChatBox before rendering.
        if let Some(ps) = app_state.player_state.as_ref() {
            self.sync_chat_messages(ps);
        }

        self.perf_profiler.begin_frame();

        // Split borrow: gfx_cache (mut) and player_state (ref) are separate fields.
        let AppState {
            ref mut gfx_cache,
            ref player_state,
            ..
        } = *app_state;

        let Some(ps) = player_state.as_ref() else {
            self.perf_profiler.end_frame();
            return Ok(());
        };

        // 1. World tiles (two-pass painter order)
        let shadows_on = ps.player_data().are_shadows_enabled != 0;
        let effects_on = self.are_spell_effects_enabled;
        self.perf_profiler.begin_sample(PerfLabel::DrawWorld);
        self.draw_world(canvas, gfx_cache, ps, shadows_on, effects_on)?;
        self.perf_profiler.end_sample(PerfLabel::DrawWorld);

        // 5. Chat log + input line (via ChatBox widget)
        self.perf_profiler.begin_sample(PerfLabel::DrawChat);
        {
            let mut ctx = RenderContext {
                canvas,
                gfx: gfx_cache,
            };
            self.chat_box.render(&mut ctx)?;
        }
        self.perf_profiler.end_sample(PerfLabel::DrawChat);

        // 5a. Status panel (upper-left sigil + stat bars)
        self.perf_profiler
            .begin_sample(PerfLabel::SyncAndDrawStatus);
        {
            if let Some(ps) = app_state.player_state.as_ref() {
                let ci = ps.character_info();
                let rank_index = Self::points_to_rank_index(ci.points_tot as u32);
                self.status_panel.sync(ps, rank_index);
                self.rank_arc
                    .set_progress(mag_core::ranks::rank_progress(ci.points_tot as u32));
                self.mode_button.sync(ci.mode);
                use crate::ui::skills_panel::{SkillsPanel as SP, SkillsPanelData};
                let sorted = SP::build_sorted_skills(&ci.skill);
                self.skills_panel.update_data(SkillsPanelData {
                    attrib: ci.attrib,
                    hp: ci.hp,
                    end: ci.end,
                    mana: ci.mana,
                    skill: ci.skill,
                    points: ci.points,
                    sorted_skills: sorted,
                    keybinds: ps.player_data().skill_keybinds,
                });
                use crate::ui::inventory_panel::InventoryPanelData;
                self.inventory_panel.update_data(InventoryPanelData {
                    items: ci.item,
                    items_p: ci.item_p,
                    worn: ci.worn,
                    worn_p: ci.worn_p,
                    citem: ci.citem,
                    citem_p: ci.citem_p,
                    gold: ci.gold,
                    selected_char: ps.selected_char(),
                });

                // Update minimap xmap buffer, then push viewport pixels to the widget.
                if let Some((cx, cy)) = self.update_minimap_xmap(gfx_cache, ps) {
                    self.minimap_widget
                        .update_viewport(&self.minimap_xmap, cx, cy);
                }
            }
            let mut ctx = RenderContext {
                canvas,
                gfx: gfx_cache,
            };
            self.status_panel.render(&mut ctx)?;
        }
        self.perf_profiler.end_sample(PerfLabel::SyncAndDrawStatus);

        // 5b. HUD panels + button bar (rendered after chat, before legacy HUD)
        self.perf_profiler.begin_sample(PerfLabel::DrawHudPanels);
        {
            let mut ctx = RenderContext {
                canvas,
                gfx: gfx_cache,
            };
            self.skills_panel.render(&mut ctx)?;
            self.inventory_panel.render(&mut ctx)?;
            self.settings_panel.render(&mut ctx)?;
            self.rank_arc.render(&mut ctx)?;
            self.hud_buttons.render(&mut ctx)?;
            self.minimap_widget.render(&mut ctx)?;
            self.mode_button.render(&mut ctx)?;
        }
        self.perf_profiler.end_sample(PerfLabel::DrawHudPanels);

        // 5c-ii. Look panel (center-right, when look target is visible)
        self.perf_profiler.begin_sample(PerfLabel::DrawLookPanel);
        if let Some(ps) = app_state.player_state.as_ref() {
            self.look_panel.sync(ps);
            let mut ctx = RenderContext {
                canvas,
                gfx: gfx_cache,
            };
            self.look_panel.render(&mut ctx)?;
        }
        self.perf_profiler.end_sample(PerfLabel::DrawLookPanel);

        // 5d. Shop/depot/grave overlay (centered, when active)
        self.perf_profiler.begin_sample(PerfLabel::DrawShopPanel);
        {
            use crate::ui::shop_panel::ShopPanelData;
            if let Some(ps) = app_state.player_state.as_ref() {
                let shop = ps.shop_target();
                let mut items = [0u16; 62];
                let mut prices = [0u32; 62];
                for i in 0..62 {
                    items[i] = shop.item(i);
                    prices[i] = shop.price(i);
                }
                self.shop_panel.update_data(ShopPanelData {
                    items,
                    prices,
                    pl_price: shop.pl_price(),
                    shop_nr: shop.nr(),
                    citem: ps.character_info().citem,
                    visible: ps.should_show_shop(),
                });
            }
            let mut ctx = RenderContext {
                canvas,
                gfx: gfx_cache,
            };
            self.shop_panel.render(&mut ctx)?;
        }
        self.perf_profiler.end_sample(PerfLabel::DrawShopPanel);

        // 5e. Carried item (always drawn, even when inventory panel is hidden)
        self.perf_profiler.begin_sample(PerfLabel::DrawCarriedItem);
        if let Some(ps) = app_state.player_state.as_ref() {
            self.draw_carried_item(canvas, gfx_cache, ps)?;
        }
        self.perf_profiler.end_sample(PerfLabel::DrawCarriedItem);

        self.perf_profiler.end_frame();
        Ok(())
    }

    /// Render the egui escape/options overlay (shadows, effects, volume, disconnect/quit).
    ///
    /// # Returns
    ///
    /// `Some(SceneType)` if the player chose to disconnect or quit, otherwise `None`.
    fn render_ui(&mut self, app_state: &mut AppState, ctx: &egui::Context) -> Option<SceneType> {
        let mut cert_accept_clicked = false;
        let mut cert_reject_clicked = false;

        if let Some(mismatch) = self.certificate_mismatch.as_ref() {
            egui::Window::new("Game Server Certificate Changed")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label("The game server certificate fingerprint changed.");
                    ui.colored_label(
                        egui::Color32::YELLOW,
                        "This may indicate a man-in-the-middle attack unless you intentionally rotated certificates.",
                    );
                    ui.add_space(8.0);
                    ui.label(format!("Host: {}", mismatch.host));
                    ui.label("Previously trusted fingerprint:");
                    ui.monospace(&mismatch.expected_fingerprint);
                    ui.add_space(4.0);
                    ui.label("New fingerprint presented by server:");
                    ui.monospace(&mismatch.received_fingerprint);
                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        if ui.button("Accept New Certificate").clicked() {
                            cert_accept_clicked = true;
                        }
                        if ui.button("Reject").clicked() {
                            cert_reject_clicked = true;
                        }
                    });
                });
        }

        if cert_accept_clicked {
            if let Some(mismatch) = self.certificate_mismatch.take() {
                match cert_trust::trust_fingerprint(&mismatch.host, &mismatch.received_fingerprint)
                {
                    Ok(()) => {
                        if let Err(err) = self.start_game_network_session(app_state) {
                            self.pending_exit = Some(err);
                            return Some(SceneType::CharacterSelection);
                        }
                        return None;
                    }
                    Err(err) => {
                        self.pending_exit = Some(format!("Failed to update known hosts: {err}"));
                        return Some(SceneType::CharacterSelection);
                    }
                }
            }
        } else if cert_reject_clicked {
            self.certificate_mismatch = None;
            return Some(SceneType::CharacterSelection);
        }

        // Show an unencrypted-connection warning banner only after the game
        // session is actually logged in.
        let is_unencrypted = app_state
            .network
            .as_ref()
            .map_or(false, |n| n.logged_in && !n.tls_active);
        if is_unencrypted {
            egui::Area::new(egui::Id::new("tls_warning_banner"))
                .anchor(egui::Align2::CENTER_TOP, [0.0, 4.0])
                .interactable(false)
                .show(ctx, |ui| {
                    egui::Frame::new()
                        .fill(egui::Color32::from_rgba_premultiplied(40, 30, 0, 200))
                        .inner_margin(egui::Margin::symmetric(12, 4))
                        .corner_radius(4.0)
                        .show(ui, |ui| {
                            ui.colored_label(
                                egui::Color32::YELLOW,
                                "UNENCRYPTED - Game traffic is not protected",
                            );
                        });
                });
        }
        self.perf_profiler.begin_sample(PerfLabel::RenderUi);

        if !self.escape_menu_open {
            self.perf_profiler.end_sample(PerfLabel::RenderUi);
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

                // Show Names toggle
                let mut show_names = if let Some(ps) = app_state.player_state.as_ref() {
                    ps.player_data().show_names != 0
                } else {
                    false
                };
                if ui.checkbox(&mut show_names, "Show Names").changed() {
                    if let Some(ps) = app_state.player_state.as_mut() {
                        ps.player_data_mut().show_names = if show_names { 1 } else { 0 };
                        profile_changed = true;
                    }
                }

                // Show % Health toggle
                let mut show_proz = if let Some(ps) = app_state.player_state.as_ref() {
                    ps.player_data().show_proz != 0
                } else {
                    false
                };
                if ui.checkbox(&mut show_proz, "Show % Health").changed() {
                    if let Some(ps) = app_state.player_state.as_mut() {
                        ps.player_data_mut().show_proz = if show_proz { 1 } else { 0 };
                        profile_changed = true;
                    }
                }

                // Hide Walls toggle
                let mut hide_walls = if let Some(ps) = app_state.player_state.as_ref() {
                    ps.player_data().hide != 0
                } else {
                    false
                };
                if ui.checkbox(&mut hide_walls, "Hide Walls").changed() {
                    if let Some(ps) = app_state.player_state.as_mut() {
                        ps.player_data_mut().hide = if hide_walls { 1 } else { 0 };
                        profile_changed = true;
                    }
                }

                ui.separator();

                let latest_rtt = app_state
                    .network
                    .as_ref()
                    .and_then(|net| net.last_rtt_ms)
                    .map(|value| format!("{} ms", value))
                    .unwrap_or_else(|| "N/A".to_string());
                ui.label(format!("Latest Ping (Round-Trip Time): {}", latest_rtt));

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

                // --- Display settings ------------------------------------
                ui.heading("Display");

                // Display mode combo box
                let mut selected_mode = app_state.display_mode;
                egui::ComboBox::from_label("Display Mode")
                    .selected_text(selected_mode.to_string())
                    .show_ui(ui, |ui| {
                        for mode in DisplayMode::ALL {
                            ui.selectable_value(&mut selected_mode, mode, mode.to_string());
                        }
                    });
                if selected_mode != app_state.display_mode {
                    app_state.display_command = Some(DisplayCommand::SetDisplayMode(selected_mode));
                }

                // Pixel-perfect scaling checkbox
                let mut pixel_perfect = app_state.pixel_perfect_scaling;
                if ui
                    .checkbox(&mut pixel_perfect, "Pixel-Perfect Scaling")
                    .changed()
                {
                    app_state.display_command =
                        Some(DisplayCommand::SetPixelPerfectScaling(pixel_perfect));
                }

                // VSync checkbox
                let mut vsync = app_state.vsync_enabled;
                if ui.checkbox(&mut vsync, "VSync").changed() {
                    app_state.display_command = Some(DisplayCommand::SetVSync(vsync));
                }
                // ---------------------------------------------------------

                ui.separator();

                // --- Performance profiling ---------------------------------
                let profiler_label = if self.perf_profiler.is_active() {
                    format!(
                        "Profiling... {}s remaining",
                        self.perf_profiler.remaining_secs()
                    )
                } else {
                    "Profile Performance".to_string()
                };
                if ui.button(&profiler_label).clicked() {
                    self.perf_profiler.start();
                }
                // ---------------------------------------------------------

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
                if ui.button("Return to game").clicked() {
                    self.escape_menu_open = false;
                }
            });

        if profile_changed {
            self.save_active_profile(app_state);
        }

        self.perf_profiler.end_sample(PerfLabel::RenderUi);

        scene_change
    }
}
