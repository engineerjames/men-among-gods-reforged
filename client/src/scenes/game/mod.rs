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
//! | [`net_events`] | Per-frame network tick processing and auto-look |
//! | [`perf_profiler`] | Wall-clock profiler for rendering functions (activated from escape menu) |

mod game_math;
mod net_events;
mod perf_profiler;
mod profile;
mod world_render;

use perf_profiler::{PerfLabel, PerfProfiler};

use std::time::Duration;

use sdl2::{
    event::Event, keyboard::Keycode, mouse::MouseButton, pixels::Color, render::Canvas,
    video::Window,
};

use mag_core::constants::{ISCHAR, ISITEM, ISUSABLE, TILEX, TILEY};

use crate::{
    cert_trust,
    constants::{TARGET_HEIGHT_INT, TARGET_WIDTH_INT},
    gfx_cache::GraphicsCache,
    network::{client_commands::ClientCommand, NetworkRuntime},
    player_state::PlayerState,
    preferences::{self, CharacterIdentity},
    scenes::scene::{Scene, SceneType},
    state::{AppState, DisplayCommand},
    ui::{
        self,
        button_arc::HudButtonBar,
        cert_dialog::{CertDialog, CertDialogAction},
        chat_box::ChatBox,
        inventory_panel::InventoryPanel,
        keybindings_panel::{
            KeybindingsPanel, KeybindingsPanelData, KEYBINDINGS_PANEL_H, KEYBINDINGS_PANEL_W,
        },
        look_panel::LookPanel,
        minimap_widget::MinimapWidget,
        mode_button::ModeButton,
        rank_progress_line::RankProgressLine,
        rank_sigil::RankSigil,
        settings_panel::{SettingsPanel, SettingsPanelData, SETTINGS_PANEL_H},
        shop_panel::ShopPanel,
        skill_bar::SkillBar,
        skill_picker_popup::SkillPickerPopup,
        skills_panel::SkillsPanel,
        status_panel::StatusPanel,
        style::Padding,
        tls_warning_banner::TlsWarningBanner,
        vitality_bars::VitalityBars,
        widget::{
            Bounds, EventResponse, GameAction, HudPanel, KeyBindings, KeyModifiers, Widget,
            WidgetAction,
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

/// X center of the HUD layout (used for panel positioning and rank arc).
const HUD_ARC_CENTER_X: i32 = crate::constants::TARGET_WIDTH_INT as i32 / 2;
/// Y center of the HUD layout (bottom edge of the viewport).
const HUD_ARC_CENTER_Y: i32 = crate::constants::TARGET_HEIGHT_INT as i32;
/// Legacy arc radius, still used for panel vertical positioning.
const HUD_ARC_RADIUS: u32 = 60;
/// Radius of each individual HUD button.
const HUD_BUTTON_RADIUS: u32 = 16;
/// Sprite IDs for [Skills, Inventory, Settings] buttons.
const HUD_SPRITE_IDS: [usize; 3] = [267, 128, 35];
/// X center of the HUD button column (lower-right, aligned with minimap).
const HUD_BTN_CX: i32 = crate::constants::TARGET_WIDTH_INT as i32 - 30;
/// Center Y of the bottom-most HUD button (above the mode button).
const HUD_BTN_BOTTOM_CY: i32 = MODE_BTN_CY - MODE_BTN_RADIUS as i32 - HUD_BUTTON_RADIUS as i32 - 10;
/// Vertical spacing between adjacent HUD button centers.
const HUD_BTN_SPACING: u32 = 40;

// ---- Skill bar ---- //
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

// ---- Rank sigil (upper-left) ---- //

/// X position of the rank sigil widget.
const RANK_SIGIL_X: i32 = 4;
/// Y position of the rank sigil widget.
const RANK_SIGIL_Y: i32 = 4;

// ---- Status panel (WV/AV, right of skill bar) ---- //

/// X position of the status panel (8 px to the right of the skill bar's right edge).
const STATUS_PANEL_X: i32 = (TARGET_WIDTH_INT as i32 - 500) / 2 + 500 + 8;
/// Y position of the status panel (same row as the rank progress line).
const STATUS_PANEL_Y: i32 = TARGET_HEIGHT_INT as i32 - 38;

/// X position of the vitality chevrons (horizontal centre of the player sprite).
const VITALITY_BARS_X: i32 = TARGET_WIDTH_INT as i32 / 2;
/// Y position of the vitality chevron feet.
const VITALITY_BARS_Y: i32 = TARGET_HEIGHT_INT as i32 - 65;

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
    pub(super) rank_sigil: RankSigil,
    pub(super) chat_box: ChatBox,
    pub(super) hud_buttons: HudButtonBar,
    pub(super) rank_progress_line: RankProgressLine,
    pub(super) skills_panel: SkillsPanel,
    pub(super) inventory_panel: InventoryPanel,
    pub(super) settings_panel: SettingsPanel,
    pub(super) keybindings_panel: KeybindingsPanel,
    pub(super) minimap_widget: MinimapWidget,
    pub(super) mode_button: ModeButton,
    pub(super) look_panel: LookPanel,
    pub(super) shop_panel: ShopPanel,
    pub(super) vitality_bars: VitalityBars,
    pub(super) skill_bar: SkillBar,
    pub(super) skill_picker: SkillPickerPopup,
    pub(super) last_synced_log_len: usize,
    pub(super) pending_exit: Option<String>,
    pub(super) certificate_mismatch: Option<cert_trust::FingerprintMismatch>,
    /// SDL2 certificate-mismatch dialog (created on demand when a mismatch is detected).
    cert_dialog: Option<CertDialog>,
    /// Non-interactive TLS warning banner shown when the connection is unencrypted.
    tls_banner: TlsWarningBanner,
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
            status_panel: StatusPanel::new(STATUS_PANEL_X, STATUS_PANEL_Y, HUD_PANEL_BG),
            rank_sigil: RankSigil::new(RANK_SIGIL_X, RANK_SIGIL_Y, HUD_PANEL_BG),
            chat_box: ChatBox::new(
                Bounds::new(CHATBOX_X, CHATBOX_Y, CHATBOX_W, CHATBOX_H),
                Color::RGBA(10, 10, 30, 180),
                Padding::uniform(4),
            ),
            hud_buttons: HudButtonBar::new(
                HUD_BTN_CX,
                HUD_BTN_BOTTOM_CY,
                HUD_BTN_SPACING,
                HUD_BUTTON_RADIUS,
                HUD_SPRITE_IDS,
            ),
            rank_progress_line: RankProgressLine::new(
                (TARGET_WIDTH_INT as i32 - 380) / 2,
                TARGET_HEIGHT_INT as i32 - 38,
                400,
                2,
            ),
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
                Bounds::new(
                    HUD_ARC_CENTER_X - HUD_PANEL_W as i32 / 2,
                    panel_bottom - SETTINGS_PANEL_H as i32,
                    HUD_PANEL_W,
                    SETTINGS_PANEL_H,
                ),
                HUD_PANEL_BG,
            ),
            keybindings_panel: KeybindingsPanel::new(
                Bounds::new(
                    HUD_ARC_CENTER_X - KEYBINDINGS_PANEL_W as i32 / 2,
                    panel_bottom - KEYBINDINGS_PANEL_H as i32,
                    KEYBINDINGS_PANEL_W,
                    KEYBINDINGS_PANEL_H,
                ),
                HUD_PANEL_BG,
            ),
            minimap_widget: MinimapWidget::new(MINIMAP_BTN_CX, MINIMAP_BTN_CY, MINIMAP_BTN_RADIUS),
            mode_button: ModeButton::new(MODE_BTN_CX, MODE_BTN_CY, MODE_BTN_RADIUS),
            vitality_bars: VitalityBars::new(VITALITY_BARS_X, VITALITY_BARS_Y),
            look_panel: LookPanel::new(
                Bounds::new(LOOK_PANEL_X, LOOK_PANEL_Y, LOOK_PANEL_W, LOOK_PANEL_H),
                HUD_PANEL_BG,
            ),
            shop_panel: ShopPanel::new(
                Bounds::new(SHOP_PANEL_X, SHOP_PANEL_Y, SHOP_PANEL_W, SHOP_PANEL_H),
                HUD_PANEL_BG,
            ),
            skill_bar: SkillBar::new(crate::ui::skill_bar::SkillBarConfig {
                spell_x: 295,
                spell_y: TARGET_HEIGHT_INT as i32 - 57,
                spell_width: 24,
                spell_height: 24,
            }),
            skill_picker: SkillPickerPopup::new(),
            last_synced_log_len: 0,
            pending_exit: None,
            certificate_mismatch: None,
            cert_dialog: None,
            tls_banner: TlsWarningBanner::new(),
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
            pending_skill_assignment: None,
            active_profile_character: None,
            perf_profiler: PerfProfiler::new(),
        }
    }

    /// Returns the player's own `ch_nr` from the canonical center map tile.
    ///
    /// The center tile `(TILEX/2, TILEY/2)` is always the local player's
    /// character. Returns `0` when the tile is not yet available.
    /// TODO: Should we just have the server do this?
    pub(super) fn own_ch_nr(ps: &PlayerState) -> u32 {
        ps.map()
            .tile_at_xy(TILEX / 2, TILEY / 2)
            .map(|t| t.ch_nr as u32)
            .unwrap_or(0)
    }

    /// Resolve the default skill target.
    ///
    /// Priority matches expected gameplay behavior:
    /// 1) Explicitly selected character (Alt+click), unless that character is ourselves
    /// 2) Current attack target (`attack_cn`)
    /// 3) No target (0)
    pub(super) fn default_skill_target(ps: &PlayerState) -> u32 {
        let selected = ps.selected_char() as u32;
        if selected != 0 && selected != Self::own_ch_nr(ps) {
            return selected;
        }

        ps.character_info().attack_cn.max(0) as u32
    }

    pub(super) fn play_click_sound(&self, app_state: &AppState) {
        app_state
            .sfx_cache
            .play_click(app_state.settings.master_volume);
    }

    /// Build a [`SettingsPanelData`] snapshot from current game state.
    ///
    /// # Arguments
    ///
    /// * `app_state` - Shared application state.
    ///
    /// # Returns
    ///
    /// A snapshot suitable for [`SettingsPanel::sync_state`].
    fn build_settings_panel_data(&self, app_state: &AppState) -> SettingsPanelData {
        let last_rtt = app_state.network.as_ref().and_then(|net| net.last_rtt_ms);

        SettingsPanelData {
            shadows_enabled: app_state.settings.shadows_enabled,
            spell_effects_enabled: app_state.settings.spell_effects_enabled,
            show_names: app_state.settings.show_names,
            show_health_pct: app_state.settings.show_proz,
            hide_walls: app_state.settings.hide,
            show_helper_text: app_state.settings.show_helper_text,
            master_volume: app_state.settings.master_volume,
            display_mode: app_state.settings.display_mode,
            pixel_perfect_scaling: app_state.settings.pixel_perfect_scaling,
            vsync_enabled: app_state.settings.vsync_enabled,
            last_rtt_ms: last_rtt,
            profiler_active: self.perf_profiler.is_active(),
            profiler_remaining_secs: if self.perf_profiler.is_active() {
                Some(self.perf_profiler.remaining_secs())
            } else {
                None
            },
        }
    }

    /// Drain pending `WidgetAction`s from the settings panel and apply
    /// the corresponding state mutations.
    ///
    /// # Arguments
    ///
    /// * `app_state` - Shared application state (network + player state).
    ///
    /// # Returns
    ///
    /// `Some(SceneType)` if the user chose to disconnect or quit.
    fn process_settings_panel_actions(
        &mut self,
        app_state: &mut AppState<'_>,
    ) -> Option<SceneType> {
        let mut scene_change: Option<SceneType> = None;
        let mut profile_changed = false;

        for action in self.settings_panel.take_actions() {
            match action {
                WidgetAction::SetShadows(v) => {
                    app_state.settings.shadows_enabled = v;
                    profile_changed = true;
                }
                WidgetAction::SetSpellEffects(v) => {
                    app_state.settings.spell_effects_enabled = v;
                    profile_changed = true;
                }
                WidgetAction::SetShowNames(v) => {
                    app_state.settings.show_names = v;
                    profile_changed = true;
                }
                WidgetAction::SetShowHealthPct(v) => {
                    app_state.settings.show_proz = v;
                    profile_changed = true;
                }
                WidgetAction::SetHideWalls(v) => {
                    app_state.settings.hide = v;
                    profile_changed = true;
                }
                WidgetAction::SetShowHelperText(v) => {
                    app_state.settings.show_helper_text = v;
                    profile_changed = true;
                }
                WidgetAction::SetMasterVolume(v) => {
                    app_state.settings.master_volume = v;
                    profile_changed = true;
                }
                WidgetAction::SetDisplayMode(m) => {
                    app_state.display_command = Some(DisplayCommand::SetDisplayMode(m));
                }
                WidgetAction::SetPixelPerfectScaling(v) => {
                    app_state.display_command = Some(DisplayCommand::SetPixelPerfectScaling(v));
                }
                WidgetAction::SetVSync(v) => {
                    app_state.display_command = Some(DisplayCommand::SetVSync(v));
                }
                WidgetAction::Disconnect => {
                    scene_change = Some(SceneType::CharacterSelection);
                }
                WidgetAction::Quit => {
                    scene_change = Some(SceneType::Exit);
                }
                WidgetAction::OpenLogDir => {
                    let log_dir = preferences::log_file_path()
                        .parent()
                        .map(|p| p.to_path_buf())
                        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
                    crate::platform::open_directory_in_file_manager(&log_dir);
                }
                WidgetAction::StartProfiler => {
                    self.perf_profiler.start();
                }
                WidgetAction::TogglePanel(HudPanel::KeyBindings) => {
                    self.keybindings_panel.toggle();
                    if self.keybindings_panel.is_visible() {
                        self.keybindings_panel.sync_state(&KeybindingsPanelData {
                            bindings: app_state.settings.character.key_bindings.clone(),
                        });
                    }
                }
                WidgetAction::TogglePanel(_) => {
                    profile_changed = true;
                }
                _ => {}
            }
        }

        if profile_changed {
            self.save_active_profile(app_state);
        }

        scene_change
    }

    /// Drain pending `WidgetAction`s from the keybindings panel and apply
    /// binding updates.
    ///
    /// # Arguments
    ///
    /// * `app_state` - Shared application state.
    fn process_keybindings_panel_actions(&mut self, app_state: &mut AppState) {
        for action in self.keybindings_panel.take_actions() {
            match action {
                WidgetAction::UpdateKeyBinding { action, binding } => {
                    app_state
                        .settings
                        .character
                        .key_bindings
                        .set_binding(action, binding);
                    self.save_active_profile(app_state);
                }
                WidgetAction::TogglePanel(HudPanel::KeyBindings) => {
                    // Close button pressed — panel already toggled itself.
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
        let total_pushed = ps.log_total_pushed();
        if total_pushed <= self.last_synced_log_len {
            return;
        }
        let new_count = total_pushed - self.last_synced_log_len;
        let available = ps.log_len();
        // If more messages arrived than the buffer can hold, we can only
        // retrieve what's still in the buffer.
        let fetchable = new_count.min(available);
        let start = available - fetchable;
        let new_messages = (start..available).filter_map(|i| ps.log_message(i).cloned());
        self.chat_box.push_messages(new_messages);
        self.last_synced_log_len = total_pushed;
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
    fn draw_carried_item(
        &self,
        canvas: &mut Canvas<Window>,
        gfx: &mut GraphicsCache<'_>,
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

    /// Returns `true` when the mouse cursor is hovering over any visible UI
    /// widget, in which case helper text should be suppressed.
    fn is_mouse_over_ui(&self) -> bool {
        let (mx, my) = (self.mouse_x, self.mouse_y);
        if self.chat_box.is_focused() && self.chat_box.bounds().contains_point(mx, my) {
            return true;
        }
        if self.rank_sigil.bounds().contains_point(mx, my) {
            return true;
        }
        if self.status_panel.bounds().contains_point(mx, my) {
            return true;
        }
        if self.hud_buttons.bounds().contains_point(mx, my) {
            return true;
        }
        if self.skill_bar.bounds().contains_point(mx, my) {
            return true;
        }
        if self.minimap_widget.is_visible() && self.minimap_widget.bounds().contains_point(mx, my) {
            return true;
        }
        if self.mode_button.bounds().contains_point(mx, my) {
            return true;
        }
        if self.rank_progress_line.bounds().contains_point(mx, my) {
            return true;
        }
        if self.skills_panel.is_visible() && self.skills_panel.bounds().contains_point(mx, my) {
            return true;
        }

        if self.settings_panel.is_visible() && self.settings_panel.bounds().contains_point(mx, my) {
            return true;
        }

        false
    }

    /// Draws context-sensitive helper text below and to the right of the
    /// mouse cursor with a drop shadow, matching the nameplate style.
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
    fn draw_helper_text(
        &self,
        canvas: &mut Canvas<Window>,
        gfx: &mut GraphicsCache<'_>,
        ps: &PlayerState,
        show_helper_text: bool,
    ) -> Result<(), String> {
        if !show_helper_text {
            return Ok(());
        }
        // Show the rank name as a tooltip when hovering the rank sigil.
        if self.rank_sigil.is_hovered() {
            let x = self.mouse_x + 12;
            let y = self.mouse_y + 16;
            return crate::font_cache::draw_text(
                canvas,
                gfx,
                1,
                self.rank_sigil.rank_name(),
                x,
                y,
                crate::font_cache::TextStyle::drop_shadow(),
            );
        }
        if self.is_mouse_over_ui() {
            return Ok(());
        }
        let Some(text) = self.resolve_helper_text(ps) else {
            return Ok(());
        };
        let x = self.mouse_x + 12;
        let y = self.mouse_y + 16;
        crate::font_cache::draw_text(
            canvas,
            gfx,
            1,
            text,
            x,
            y,
            crate::font_cache::TextStyle::drop_shadow(),
        )
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
    fn update_minimap_xmap(
        &mut self,
        gfx: &mut GraphicsCache<'_>,
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

                // Use the network-authoritative ba_sprite rather than the
                // engine_tick-computed `tile.back` — the latter is briefly
                // zeroed during engine_tick phase 1 and introduces an ordering
                // dependency we don't need.
                let back_id = tile.ba_sprite.max(0) as usize;
                if back_id != 0 {
                    let (r, g, b) = gfx.get_avg_color(back_id);
                    // Guard against all-transparent sprites whose average color
                    // is (0,0,0) — writing that would produce an opaque black
                    // pixel indistinguishable from an unvisited cell.
                    if (r | g | b) != 0 {
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
    fn start_game_network_session(&mut self, app_state: &mut AppState<'_>) -> Result<(), String> {
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
    fn on_enter(&mut self, app_state: &mut AppState<'_>) {
        self.chat_box = ChatBox::new(
            Bounds::new(CHATBOX_X, CHATBOX_Y, CHATBOX_W, CHATBOX_H),
            Color::RGBA(10, 10, 30, 180),
            Padding::uniform(4),
        );
        self.last_synced_log_len = 0;
        self.pending_exit = None;
        self.certificate_mismatch = None;
        self.cert_dialog = None;
        self.tls_banner.set_visible(false);
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
        self.pending_skill_assignment = None;
        self.active_profile_character = None;

        app_state.settings.spell_effects_enabled = true;
        app_state.settings.character.key_bindings = KeyBindings::default();
        app_state.settings.master_volume = 1.0;

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
    fn on_exit(&mut self, app_state: &mut AppState<'_>) {
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
    fn handle_event(&mut self, app_state: &mut AppState<'_>, event: &Event) -> Option<SceneType> {
        // --- Escape key: always processed regardless of menu state ---
        if let Event::KeyDown {
            keycode: Some(Keycode::Escape),
            ..
        } = event
        {
            // Always send CmdReset (preserving legacy behavior for now...).
            if let Some(net) = app_state.network.as_ref() {
                self.play_click_sound(app_state);
                net.send(ClientCommand::new_reset());
            }

            // If any windows are open, close them.
            if self.shop_panel.is_visible() {
                // Closing the shop requires resetting the PlayerState flag as well;
                // the ShopPanelData snapshot is rebuilt from it every frame.
                if let Some(ps) = app_state.player_state.as_mut() {
                    ps.close_shop();
                }
                self.shop_panel.toggle();
            }

            if self.settings_panel.is_visible() {
                self.settings_panel.toggle();
            }

            if self.keybindings_panel.is_visible() {
                self.keybindings_panel.toggle();
            }

            if self.inventory_panel.is_visible() {
                self.inventory_panel.toggle();
            }

            if self.skills_panel.is_visible() {
                self.skills_panel.toggle();
            }

            if self.minimap_widget.is_visible() {
                self.minimap_widget.toggle();
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

        // --- Dispatch to ChatBox first; if consumed, act on pending actions ---
        if let Some(ui_event) = ui::sdl_to_ui_event(
            event,
            self.mouse_x,
            self.mouse_y,
            KeyModifiers {
                ctrl: self.ctrl_held,
                shift: self.shift_held,
                alt: self.alt_held,
            },
        ) {
            // --- Certificate mismatch dialog (modal, blocks all other input) ---
            if let Some(ref mut dialog) = self.cert_dialog {
                dialog.handle_event(&ui_event);
                for action in dialog.take_cert_actions() {
                    match action {
                        CertDialogAction::Accept => {
                            if let Some(mismatch) = self.certificate_mismatch.take() {
                                match cert_trust::trust_fingerprint(
                                    &mismatch.host,
                                    &mismatch.received_fingerprint,
                                ) {
                                    Ok(()) => {
                                        self.cert_dialog = None;
                                        if let Err(err) = self.start_game_network_session(app_state)
                                        {
                                            self.pending_exit = Some(err);
                                            return Some(SceneType::CharacterSelection);
                                        }
                                        return None;
                                    }
                                    Err(err) => {
                                        self.cert_dialog = None;
                                        self.pending_exit =
                                            Some(format!("Failed to update known hosts: {err}"));
                                        return Some(SceneType::CharacterSelection);
                                    }
                                }
                            }
                        }
                        CertDialogAction::Reject => {
                            self.certificate_mismatch = None;
                            self.cert_dialog = None;
                            return Some(SceneType::CharacterSelection);
                        }
                    }
                }
                return None;
            }

            // --- Skill picker popup (modal — must come before skill bar) ---
            if self.skill_picker.handle_event(&ui_event) == EventResponse::Consumed {
                self.process_skill_picker_actions(app_state);
                return None;
            }

            // --- Rank sigil (upper-left) ---
            if self.rank_sigil.handle_event(&ui_event) == EventResponse::Consumed {
                return None;
            }

            // --- StatusPanel (WV/AV display, right of skill bar) ---
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
                if let Some(sc) = self.process_settings_panel_actions(app_state) {
                    return Some(sc);
                }
                return None;
            }

            // --- Dispatch to keybindings editor panel ---
            if self.keybindings_panel.handle_event(&ui_event) == EventResponse::Consumed {
                self.process_keybindings_panel_actions(app_state);
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

            // --- Dispatch to skill bar ---
            if self.skill_bar.handle_event(&ui_event) == EventResponse::Consumed {
                self.process_skill_bar_actions(app_state);
                return None;
            }

            // --- Dispatch to HUD button bar ---
            if self.hud_buttons.handle_event(&ui_event) == EventResponse::Consumed {
                for action in self.hud_buttons.take_actions() {
                    if let WidgetAction::TogglePanel(panel) = action {
                        match panel {
                            HudPanel::Skills => self.skills_panel.toggle(),
                            HudPanel::Inventory => self.inventory_panel.toggle(),
                            HudPanel::Settings => {
                                self.settings_panel.toggle();
                                if self.settings_panel.is_visible() {
                                    let data = self.build_settings_panel_data(app_state);
                                    self.settings_panel.sync_state(&data);
                                }
                            }
                            HudPanel::Minimap => self.minimap_widget.toggle(),
                            HudPanel::KeyBindings => {
                                self.keybindings_panel.toggle();
                                if self.keybindings_panel.is_visible() {
                                    self.keybindings_panel.sync_state(&KeybindingsPanelData {
                                        bindings: app_state.settings.character.key_bindings.clone(),
                                    });
                                }
                            }
                        }
                    }
                }
                return None;
            }
        }

        // --- Keyboard bindings (suppressed when chat is focused, unless modifiers are held) ---
        if let Event::KeyDown {
            keycode: Some(kc),
            keymod,
            ..
        } = event
        {
            let mods = KeyModifiers::from_sdl2(*keymod);
            let has_modifier = mods.ctrl || mods.alt;
            if has_modifier || !self.chat_box.is_focused() {
                if let Some(action) = app_state
                    .settings
                    .character
                    .key_bindings
                    .action_for_key(*kc, mods)
                {
                    match action {
                        GameAction::ToggleSkills => self.skills_panel.toggle(),
                        GameAction::ToggleInventory => self.inventory_panel.toggle(),
                    }
                    return None;
                }
            }
        }

        match event {
            Event::KeyDown {
                keycode: Some(kc), ..
            } => match *kc {
                Keycode::Num1
                | Keycode::Num2
                | Keycode::Num3
                | Keycode::Num4
                | Keycode::Num5
                | Keycode::Num6
                | Keycode::Num7
                | Keycode::Num8
                | Keycode::Num9 => {
                    if !self.chat_box.is_focused() {
                        let key_slot = (i32::from(*kc) - i32::from(Keycode::Num1)) as usize;
                        if let (Some(net), Some(ps)) =
                            (app_state.network.as_ref(), app_state.player_state.as_ref())
                        {
                            if let Some(skill_nr) =
                                app_state.settings.character.skill_keybinds[key_slot]
                            {
                                self.play_click_sound(app_state);
                                net.send(ClientCommand::new_skill(
                                    skill_nr as u32,
                                    Self::default_skill_target(ps),
                                    ps.character_info().attrib[0][0] as u32,
                                ));
                            }
                        }
                    }
                }
                _ => {}
            },
            Event::MouseButtonUp {
                mouse_btn, x, y, ..
            } => {
                let Some(ps) = app_state.player_state.as_ref() else {
                    log::warn!("Mouse click with no player state");
                    return None;
                };

                let (cam_xoff, cam_yoff) = Self::camera_offsets(ps);

                let Some((mx, my)) = Self::screen_to_map_tile(*x, *y, cam_xoff, cam_yoff) else {
                    log::warn!("Click outside of map area: screen=({}, {})", x, y);
                    return None;
                };

                let has_ctrl = self.ctrl_held;
                let has_shift = self.shift_held;
                let has_alt = self.alt_held;

                // Read citem early so we can suppress ISITEM snapping when the
                // player is carrying an item and wants to drop, not pick up.
                let citem = ps.character_info().citem;

                let snapped = if has_ctrl || has_alt {
                    Self::nearest_tile_with_flag(ps, mx, my, ISCHAR).unwrap_or((mx, my))
                } else if has_shift && citem == 0 {
                    // Only snap to the nearest item tile when the hand is empty.
                    // With a citem held, use the raw tile so the drop lands where
                    // the player clicked rather than locking onto a nearby item.
                    Self::nearest_tile_with_flag(ps, mx, my, ISITEM).unwrap_or((mx, my))
                } else {
                    (mx, my)
                };

                let (sx, sy) = snapped;
                let tile = ps.map().tile_at_xy(sx, sy);
                let target_cn = tile.map(|t| t.ch_nr as u32).unwrap_or(0);
                let target_id = tile.map(|t| t.ch_id).unwrap_or(0);
                let (world_x, world_y) = tile.map(|t| (t.x as i16, t.y as i32)).unwrap_or((0, 0));
                // citem already read above.
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
                            // Holding item, clicked non-item tile --> drop
                            self.play_click_sound(app_state);
                            net.send(ClientCommand::new_drop(world_x, world_y));
                        } else if is_item && is_usable {
                            // Item is usable --> use
                            self.play_click_sound(app_state);
                            net.send(ClientCommand::new_use(world_x, world_y));
                        } else if is_item {
                            // Item not usable --> pickup
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
            _ => {}
        }
        None
    }

    /// Process pending network events and advance the auto-look timer.
    ///
    /// # Returns
    ///
    /// `Some(SceneType)` if a disconnect or exit was signalled, otherwise `None`.
    fn update(&mut self, app_state: &mut AppState<'_>, dt: Duration) -> Option<SceneType> {
        self.chat_box.update(dt);
        self.status_panel.update(dt);
        self.skills_panel.update(dt);
        self.inventory_panel.update(dt);
        self.settings_panel.update(dt);
        // Keep read-only settings panel values current each frame.
        if self.settings_panel.is_visible() {
            let rtt = app_state.network.as_ref().and_then(|net| net.last_rtt_ms);
            self.settings_panel.update_ping(rtt);
            self.settings_panel.update_profiler_label(
                self.perf_profiler.is_active(),
                if self.perf_profiler.is_active() {
                    Some(self.perf_profiler.remaining_secs())
                } else {
                    None
                },
            );
        }
        self.mode_button.update(dt);
        self.shop_panel.update(dt);
        self.perf_profiler.check_expired();
        // Create the cert dialog widget when a mismatch is first detected.
        if self.certificate_mismatch.is_some() && self.cert_dialog.is_none() {
            let m = self.certificate_mismatch.as_ref().unwrap();
            self.cert_dialog = Some(CertDialog::new(
                &m.host,
                &m.expected_fingerprint,
                &m.received_fingerprint,
            ));
        }

        // Update TLS warning banner visibility.
        let is_unencrypted = app_state
            .network
            .as_ref()
            .map_or(false, |n| n.logged_in && !n.tls_active);
        self.tls_banner.set_visible(is_unencrypted);

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
        app_state: &mut AppState<'_>,
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
            ref settings,
            ..
        } = *app_state;

        let Some(ps) = player_state.as_ref() else {
            self.perf_profiler.end_frame();
            return Ok(());
        };

        // 1. World tiles (two-pass painter order)
        let shadows_on = settings.shadows_enabled;
        let effects_on = settings.spell_effects_enabled;
        self.perf_profiler.begin_sample(PerfLabel::DrawWorld);
        self.draw_world(
            canvas,
            gfx_cache,
            ps,
            shadows_on,
            effects_on,
            settings.show_names,
            settings.show_proz,
            settings.hide,
        )?;
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

        // 5a. Rank sigil + status panel (WV/AV)
        self.perf_profiler
            .begin_sample(PerfLabel::SyncAndDrawStatus);
        {
            if let Some(ps) = app_state.player_state.as_ref() {
                let ci = ps.character_info();
                let rank_index = Self::points_to_rank_index(ci.points_tot as u32);
                self.rank_sigil.sync(rank_index);
                self.status_panel.sync(ci.weapon, ci.armor);
                self.rank_progress_line
                    .set_progress(mag_core::ranks::rank_progress(ci.points_tot as u32));
                self.mode_button.sync(ci.mode);
                self.vitality_bars.sync(
                    ci.a_hp,
                    ci.hp[5] as i32,
                    ci.a_end,
                    ci.end[5] as i32,
                    ci.a_mana,
                    ci.mana[5] as i32,
                );
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

                // Skill bar: 13 keybinds plus all 20 active spell/activity slots.
                {
                    use crate::preferences::NUMBER_OF_KEYBINDS;
                    use crate::ui::skill_bar::SkillBarData;
                    let mut keybinds = [None; NUMBER_OF_KEYBINDS];
                    keybinds.copy_from_slice(
                        &app_state.settings.character.skill_keybinds[..NUMBER_OF_KEYBINDS],
                    );
                    self.skill_bar.update_data(SkillBarData {
                        keybinds,
                        spell: ci.spell,
                        active: ci.active,
                    });
                }

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
            self.rank_sigil.render(&mut ctx)?;
            self.status_panel.render(&mut ctx)?;
            self.vitality_bars.render(&mut ctx)?;
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
            self.keybindings_panel.render(&mut ctx)?;
            self.hud_buttons.render(&mut ctx)?;
            self.minimap_widget.render(&mut ctx)?;
            self.mode_button.render(&mut ctx)?;
            self.skill_bar.render(&mut ctx)?;
            self.rank_progress_line.render(&mut ctx)?;
            self.skill_picker.render(&mut ctx)?;
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
                    is_grave: ps.shop_is_grave(),
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

        // 5f. Context-sensitive helper text near the cursor
        self.perf_profiler.begin_sample(PerfLabel::DrawHelperText);
        if let Some(ps) = app_state.player_state.as_ref() {
            self.draw_helper_text(canvas, gfx_cache, ps, app_state.settings.show_helper_text)?;
        }
        self.perf_profiler.end_sample(PerfLabel::DrawHelperText);

        self.perf_profiler.end_frame();

        // Render TLS warning banner and cert dialog as final overlays.
        {
            let mut ctx = RenderContext {
                canvas,
                gfx: gfx_cache,
            };
            self.tls_banner.render(&mut ctx)?;
            if let Some(ref mut dialog) = self.cert_dialog {
                dialog.render(&mut ctx)?;
            }
        }

        Ok(())
    }
}
