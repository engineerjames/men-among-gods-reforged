//! The main in-game scene — owns the gameplay HUD, world rendering, input
//! handling, and network event loop.
//!
//! The bulk of the logic is split across submodules for maintainability:
//!
//! | Module | Responsibility |
//! |---|---|
//! | [`profile`] | Load/save per-character preference profiles |
//! | [`world_render`] | Isometric tile/sprite/shadow/effect drawing |
//! | [`net_events`] | Per-frame network tick processing and auto-look |
//! | [`perf_profiler`] | Wall-clock profiler for rendering functions (activated from escape menu) |

mod controller_input;
mod game_math;
mod net_events;
mod perf_profiler;
mod profile;
mod weather;
mod world_input;
mod world_render;

use mag_core::traits::class_from_kindred;
use perf_profiler::{PerfLabel, PerfProfiler};

use std::collections::HashSet;
use std::io::Write;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use flate2::Compression;
use flate2::write::GzEncoder;
use sdl2::{event::Event, keyboard::Keycode, pixels::Color, render::Canvas, video::Window};

use mag_core::{
    client_commands::ClientCommand,
    constants::{TILEX, TILEY},
    ranks,
    skills::{SK_BLAST, SK_LAVA_BLAST, SkillIndex},
    types::api::NetworkTestSummary,
};

use crate::{
    account_api, cert_trust,
    constants::{TARGET_HEIGHT_INT, TARGET_WIDTH_INT},
    gfx_cache::GraphicsCache,
    network::NetworkRuntime,
    player_state::PlayerState,
    preferences::{self, CharacterIdentity},
    scenes::scene::{Scene, SceneType},
    state::{AppState, DisplayCommand},
    types::mouse::{ExtraMouseButton, MouseModifier},
    ui::{
        self, RenderContext,
        forms::cert_dialog::CertDialog,
        hud::button_bar::HudButtonBar,
        hud::chat_box::ChatBox,
        hud::inventory_panel::InventoryPanel,
        hud::look_panel::LookPanel,
        hud::minimap_widget::MinimapWidget,
        hud::mode_button::ModeButton,
        hud::settings_panel::{SETTINGS_PANEL_H, SettingsPanel, SettingsPanelData},
        hud::shop_panel::ShopPanel,
        hud::skill_bar::{SkillBar, TOP_CELL_POSITIONS},
        hud::skill_picker_popup::SkillPickerPopup,
        hud::skills_panel::SkillsPanel,
        hud::talent_panel::TalentPanel,
        hud::weapon_armor_panel::WeaponArmorPanel,
        style::Padding,
        visuals::rank_progress_line::RankProgressLine,
        visuals::rank_sigil::RankSigil,
        visuals::spell_effect_icons::SpellEffectIcons,
        visuals::vitality_bars::VitalityChevrons,
        widget::{Bounds, GameAction, KeyBindings, KeyModifiers, UiEvent, Widget, WidgetAction},
        widgets::on_screen_keyboard::OnScreenKeyboard,
    },
};

// ---------------------------------------------------------------------------
// Layout / tuning constants (all pub(super) so submodules can import them)
// ---------------------------------------------------------------------------

// ---- Right-side HUD button fade ---- //

/// Seconds to fade in from invisible to fully opaque.
const HUD_FADE_IN_SECS: f32 = 0.5;
/// Seconds of idle (mouse outside the right zone) before fade-out begins.
const HUD_FADE_OUT_DELAY_SECS: f32 = 3.0;
/// Seconds to fade out from fully opaque to invisible.
const HUD_FADE_OUT_SECS: f32 = 1.0;
/// Minimum mouse X position to keep the right-side HUD buttons visible.
const HUD_FADE_THRESHOLD_X: i32 = 810;

/// Maximum complete network tick groups processed per frame.
///
/// A tick group is all `NetworkEvent::Bytes` emitted for one server tick packet,
/// followed by its terminating `NetworkEvent::Tick`. We only stop processing at
/// tick boundaries so map state is never rendered from a partially applied group.
pub(super) const MAX_TICK_GROUPS_PER_FRAME: usize = 32;
pub(super) const QSIZE: u32 = 8;
/// Duration of a diagnostics network-test run.
const NETWORK_TEST_DURATION_SECS: u64 = 10;
/// Tick cadence used by the diagnostics network-test profile.
const NETWORK_TEST_SAMPLE_INTERVAL_MS: u64 = 50;
/// Per-request timeout for diagnostics network-test probes and summary upload.
const NETWORK_TEST_REQUEST_TIMEOUT_MS: u64 = 750;
/// Fixed client command packet size on the gameplay TCP protocol.
const NETWORK_TEST_CLIENT_PAYLOAD_BYTES: usize = 16;
/// Representative server tick payload sizes cycled during the test.
const NETWORK_TEST_SERVER_PAYLOAD_BYTES: [u16; 4] = [2, 18, 31, 50];
/// Maximum raw client-log bytes retained for one diagnostics upload.
const MAX_CLIENT_LOG_UPLOAD_BYTES: usize = 8 * 1024 * 1024;
/// Maximum base64-encoded compressed payload accepted by the diagnostics API.
const MAX_CLIENT_LOG_UPLOAD_B64_BYTES: usize = 12 * 1024 * 1024;
/// Amount removed from the oldest side of the log when shrinking an upload.
const CLIENT_LOG_UPLOAD_SHRINK_STEP_BYTES: usize = 256 * 1024;

/// Final computed diagnostics network-test metrics.
#[derive(Clone, Debug)]
struct NetworkTestMetrics {
    duration_ms: u32,
    total_samples: u32,
    failed_samples: u32,
    min_rtt_ms: Option<u32>,
    avg_rtt_ms: Option<u32>,
    max_rtt_ms: Option<u32>,
    jitter_ms: Option<u32>,
    quality_rating: String,
}

/// Completion message sent from network-test worker thread to `GameScene`.
#[derive(Debug)]
struct NetworkTestRunResult {
    run_id: String,
    metrics: NetworkTestMetrics,
    summary_submit_error: Option<String>,
    cancelled: bool,
}

/// Computes a jitter estimate from sequential RTT samples.
///
/// # Arguments
///
/// * `samples` - Successful probe RTT values in milliseconds.
///
/// # Returns
///
/// * `Some(jitter_ms)` when at least 2 samples are present.
/// * `None` when jitter cannot be computed.
fn estimate_jitter_ms(samples: &[u32]) -> Option<u32> {
    if samples.len() < 2 {
        return None;
    }

    let mut sum_abs_delta: u64 = 0;
    for pair in samples.windows(2) {
        let a = i64::from(pair[0]);
        let b = i64::from(pair[1]);
        sum_abs_delta += (b - a).unsigned_abs();
    }
    let steps = (samples.len() - 1) as u64;
    Some((sum_abs_delta / steps).min(u64::from(u32::MAX)) as u32)
}

/// Classifies network quality from latency and failure-rate metrics.
///
/// # Arguments
///
/// * `avg_rtt_ms` - Mean successful RTT value.
/// * `failed_samples` - Number of failed probes.
/// * `total_samples` - Number of attempted probes.
///
/// # Returns
///
/// * String quality rating (`Good`, `Fair`, `Poor`).
fn classify_network_quality(
    avg_rtt_ms: Option<u32>,
    failed_samples: u32,
    total_samples: u32,
) -> String {
    if total_samples == 0 {
        return "Poor".to_owned();
    }

    let failure_ratio = failed_samples as f32 / total_samples as f32;
    if failure_ratio > 0.20 {
        return "Poor".to_owned();
    }

    match avg_rtt_ms.unwrap_or(u32::MAX) {
        0..=120 => {
            if failure_ratio > 0.05 {
                "Fair".to_owned()
            } else {
                "Good".to_owned()
            }
        }
        121..=250 => "Fair".to_owned(),
        _ => "Poor".to_owned(),
    }
}

/// Formats an optional millisecond value for player-facing logs.
///
/// # Arguments
///
/// * `value` - Optional millisecond value.
///
/// # Returns
///
/// * Value formatted as `<n>ms` or `N/A`.
fn format_optional_ms(value: Option<u32>) -> String {
    value
        .map(|v| format!("{}ms", v))
        .unwrap_or_else(|| "N/A".to_owned())
}

/// Returns the base64 output length for `byte_len` input bytes.
fn base64_encoded_len(byte_len: usize) -> usize {
    byte_len.div_ceil(3) * 4
}

/// Returns a newest-log slice aligned to a line boundary when truncation occurs.
///
/// # Arguments
///
/// * `log_bytes` - Full log-file contents read from disk.
/// * `retained_bytes` - Target number of newest bytes to retain.
///
/// # Returns
///
/// * `(slice, slice_len)` where `slice` starts on a log-line boundary when possible.
fn newest_log_slice_for_upload(log_bytes: &[u8], retained_bytes: usize) -> (&[u8], usize) {
    let start = log_bytes.len().saturating_sub(retained_bytes);
    if start == 0 || log_bytes[start - 1] == b'\n' {
        return (&log_bytes[start..], log_bytes.len() - start);
    }

    let tail = &log_bytes[start..];
    if let Some(offset) = tail.iter().position(|byte| *byte == b'\n') {
        let aligned_start = (start + offset + 1).min(log_bytes.len());
        (&log_bytes[aligned_start..], log_bytes.len() - aligned_start)
    } else {
        (&log_bytes[start..], log_bytes.len() - start)
    }
}

/// Compresses the newest slice of the client log so it fits the diagnostics API.
///
/// # Arguments
///
/// * `log_bytes` - Full log-file contents read from disk.
///
/// # Returns
///
/// * `Ok((compressed_bytes, retained_plaintext_bytes))` when the upload payload fits.
/// * `Err(String)` when compression fails or no fitting slice can be produced.
fn compress_log_for_upload(log_bytes: &[u8]) -> Result<(Vec<u8>, usize), String> {
    if log_bytes.is_empty() {
        return Err("log file is empty".to_owned());
    }

    let mut retained_bytes = log_bytes.len().min(MAX_CLIENT_LOG_UPLOAD_BYTES);
    while retained_bytes > 0 {
        let (slice, actual_retained_bytes) = newest_log_slice_for_upload(log_bytes, retained_bytes);

        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder
            .write_all(slice)
            .map_err(|err| format!("compression error: {err}"))?;
        let compressed = encoder
            .finish()
            .map_err(|err| format!("compression error: {err}"))?;

        if base64_encoded_len(compressed.len()) <= MAX_CLIENT_LOG_UPLOAD_B64_BYTES {
            return Ok((compressed, actual_retained_bytes));
        }

        if retained_bytes <= CLIENT_LOG_UPLOAD_SHRINK_STEP_BYTES {
            break;
        }
        retained_bytes -= CLIENT_LOG_UPLOAD_SHRINK_STEP_BYTES;
    }

    Err("log file is too large to upload after compression".to_owned())
}

/// Builds the fixed-width probe payload used to approximate one gameplay client command.
///
/// # Arguments
///
/// * `sample_index` - Zero-based diagnostics sample index.
///
/// # Returns
///
/// * A deterministic 16-byte payload.
fn build_network_test_client_payload(sample_index: u32) -> [u8; NETWORK_TEST_CLIENT_PAYLOAD_BYTES] {
    let mut payload = [0_u8; NETWORK_TEST_CLIENT_PAYLOAD_BYTES];
    payload[0] = mag_core::client_commands::ClientCommandType::Ping as u8;
    payload[1..5].copy_from_slice(&sample_index.to_le_bytes());
    payload[5..9].copy_from_slice(&sample_index.wrapping_mul(50).to_le_bytes());
    payload[9..13].copy_from_slice(&(sample_index ^ 0x5a5a_1234).to_le_bytes());
    payload[13] = 0x11;
    payload[14] = 0x22;
    payload[15] = 0x33;
    payload
}

/// Returns the representative server payload size for one diagnostics sample.
///
/// # Arguments
///
/// * `sample_index` - Zero-based diagnostics sample index.
///
/// # Returns
///
/// * One of the configured representative tick payload sizes.
fn network_test_server_payload_bytes(sample_index: u32) -> u16 {
    NETWORK_TEST_SERVER_PAYLOAD_BYTES
        [sample_index as usize % NETWORK_TEST_SERVER_PAYLOAD_BYTES.len()]
}

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

const CHATBOX_X: i32 = crate::constants::TARGET_WIDTH_INT as i32 - CHATBOX_W as i32 - 4;
const CHATBOX_Y: i32 = 4;
const CHATBOX_W: u32 = 300;
const CHATBOX_H: u32 = 192;

// ---- HUD button bar layout ---- //

/// X center of the HUD layout (used for panel positioning and rank arc).
const HUD_ARC_CENTER_X: i32 = crate::constants::TARGET_WIDTH_INT as i32 / 2;
/// Y center of the HUD layout (bottom edge of the viewport).
const HUD_ARC_CENTER_Y: i32 = crate::constants::TARGET_HEIGHT_INT as i32;
/// Legacy arc radius, still used for panel vertical positioning.
const HUD_ARC_RADIUS: u32 = 60;
/// Radius of each individual HUD button.
const HUD_BUTTON_RADIUS: u32 = 16;
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
/// Y center of the minimap toggle button (one spacing above the top HUD button).
/// The HUD column has four buttons, so the minimap button sits one spacing
/// above the top button to avoid overlap.
const MINIMAP_BTN_CY: i32 = HUD_BTN_BOTTOM_CY - 5 * HUD_BTN_SPACING as i32;
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
const LOOK_PANEL_W: u32 = 130;
/// Height of the look panel.
const LOOK_PANEL_H: u32 = 260;
/// X position of the look panel (left side, 4 px margin).
const LOOK_PANEL_X: i32 = 4;
/// Y position of the look panel (vertically centered).
const LOOK_PANEL_Y: i32 = (crate::constants::TARGET_HEIGHT_INT as i32 - LOOK_PANEL_H as i32) / 4;

// ---- Shop panel (centered on screen) ---- //

/// Width of the shop panel.
const SHOP_PANEL_W: u32 = crate::ui::hud::shop_panel::SHOP_PANEL_W;
/// Height of the shop panel.
const SHOP_PANEL_H: u32 = crate::ui::hud::shop_panel::SHOP_PANEL_H;
/// X position of the shop panel (horizontally centered).
const SHOP_PANEL_X: i32 = (crate::constants::TARGET_WIDTH_INT as i32 - SHOP_PANEL_W as i32) / 2;
/// Y position of the shop panel (vertically centered).
const SHOP_PANEL_Y: i32 = (crate::constants::TARGET_HEIGHT_INT as i32 - SHOP_PANEL_H as i32) / 2;
/// Maximum character count for one helper-text line.
const HELPER_TEXT_MAX_CHARS: u32 = 50;
/// Minimum margin (in logical pixels) between helper text and the screen
/// edges. The tooltip is repositioned to honour this margin.
const HELPER_TEXT_SCREEN_MARGIN: i32 = 4;
/// Horizontal gap between the cursor and the helper-text block when drawn to
/// the right of (or flipped to the left of) the cursor.
const HELPER_TEXT_CURSOR_GAP_X: i32 = 12;
/// Vertical gap between the cursor and the helper-text block when drawn
/// below (or flipped above) the cursor.
const HELPER_TEXT_CURSOR_GAP_Y: i32 = 16;
/// Vertical gap used when the helper text is flipped to sit above the cursor.
const HELPER_TEXT_CURSOR_FLIP_GAP_Y: i32 = 4;

/// Rewrite saved Blast/Lava Blast bindings to match the currently learned skill.
///
/// # Arguments
///
/// * `primary` - Primary skill-bar keybinds to normalize.
/// * `secondary` - Secondary skill-bar keybinds to normalize.
/// * `skills` - Latest skill rows received from the server.
///
/// # Returns
///
/// * `true` if any saved keybind was changed.
fn normalize_lava_blast_keybind_arrays(
    primary: &mut [Option<usize>],
    secondary: &mut [Option<usize>],
    skills: &[[u8; SkillIndex::MaxIndex as usize]],
) -> bool {
    let blast_learned = skills[SK_BLAST][SkillIndex::BaseValue as usize] > 0;
    let lava_blast_learned = skills[SK_LAVA_BLAST][SkillIndex::BaseValue as usize] > 0;
    let replacement = match (blast_learned, lava_blast_learned) {
        (false, true) => Some((SK_BLAST, SK_LAVA_BLAST)),
        (true, false) => Some((SK_LAVA_BLAST, SK_BLAST)),
        _ => None,
    };

    let Some((from, to)) = replacement else {
        return false;
    };

    let mut changed = false;
    for slot in primary.iter_mut().chain(secondary.iter_mut()) {
        if *slot == Some(from) {
            *slot = Some(to);
            changed = true;
        }
    }
    changed
}

/// Computes the on-screen origin for a helper-text block of the given pixel
/// size, given the cursor position and the logical screen dimensions.
///
/// Defaults to placing the text below and to the right of the cursor. Flips
/// horizontally (text to the left of the cursor) when the default placement
/// would overflow the right edge, and vertically (above the cursor) when it
/// would overflow the bottom edge. Finally clamps both axes into the
/// `[margin, screen - margin - dim]` range as a safety net for tooltips
/// larger than the space on either side of the cursor.
///
/// # Arguments
///
/// * `cursor_x` - Cursor X position in logical pixels.
/// * `cursor_y` - Cursor Y position in logical pixels.
/// * `text_w`   - Width of the wrapped text block in pixels.
/// * `text_h`   - Height of the wrapped text block in pixels.
/// * `screen_w` - Logical screen width in pixels.
/// * `screen_h` - Logical screen height in pixels.
///
/// # Returns
///
/// * `(x, y)` origin in logical pixels for the top-left of the text block.
fn helper_text_origin(
    cursor_x: i32,
    cursor_y: i32,
    text_w: i32,
    text_h: i32,
    screen_w: i32,
    screen_h: i32,
) -> (i32, i32) {
    let margin = HELPER_TEXT_SCREEN_MARGIN;
    let mut x = cursor_x + HELPER_TEXT_CURSOR_GAP_X;
    let mut y = cursor_y + HELPER_TEXT_CURSOR_GAP_Y;

    if x + text_w > screen_w - margin {
        x = cursor_x - HELPER_TEXT_CURSOR_GAP_X - text_w;
    }
    if y + text_h > screen_h - margin {
        y = cursor_y - HELPER_TEXT_CURSOR_FLIP_GAP_Y - text_h;
    }

    let max_x = (screen_w - margin - text_w).max(margin);
    let max_y = (screen_h - margin - text_h).max(margin);
    x = x.clamp(margin, max_x);
    y = y.clamp(margin, max_y);

    (x, y)
}

// Minimap
pub(super) const MINIMAP_WORLD_SIZE: usize = 1024;

// ---- Rank sigil (upper-left) ---- //

/// X position of the rank sigil widget.
const RANK_SIGIL_X: i32 = 4;
/// Y position of the rank sigil widget.
const RANK_SIGIL_Y: i32 = 4;

// ---- Status panel (WV/AV, right of skill bar) ---- //

/// X position of the weapon/armor panel (8 px to the right of the skill bar's right edge).
const WEAPON_ARMOR_PANEL_X: i32 = TARGET_WIDTH_INT as i32 - 368;
/// Y position of the weapon/armor panel (same row as the rank progress line).
const WEAPON_ARMOR_PANEL_Y: i32 = TARGET_HEIGHT_INT as i32 - 33;

/// X position of the vitality chevrons (horizontal centre of the player sprite).
const VITALITY_BARS_X: i32 = TARGET_WIDTH_INT as i32 / 2;
/// Y position of the vitality chevron feet.
const VITALITY_BARS_Y: i32 = TARGET_HEIGHT_INT as i32 - 42;

// ---------------------------------------------------------------------------
// GameScene struct
// ---------------------------------------------------------------------------

/// Resolves the world-tile destination for the currently focused quest's
/// active step.
///
/// Falls back to the cached NPC quest-giver position for
/// `ReturnToQuestGiver` steps or whenever the static quest definition is
/// missing or the step index is out of range.
///
/// # Arguments
///
/// * `template_id`     - NPC template ID of the focused quest.
/// * `step_idx`        - Server-reported active step index within the quest's
///   walkthrough.
/// * `npc_pos_fallback` - World tile position of the NPC quest giver, used as
///   a fallback when no `FixedLocation` step applies.
///
/// # Returns
///
/// * `Some((x, y))` when a destination tile can be resolved, or `None` when
///   no fallback NPC position is known and no `FixedLocation` step applies.
fn active_quest_destination(
    template_id: u16,
    step_idx: usize,
    npc_pos_fallback: Option<(u16, u16)>,
) -> Option<(u16, u16)> {
    if let Some(def) = mag_core::quest_defs::find_quest_def(template_id)
        && let Some(step) = def.steps.get(step_idx)
    {
        return match step {
            mag_core::quest_defs::QuestStep::FixedLocation { x, y, .. } => Some((*x, *y)),
            mag_core::quest_defs::QuestStep::ReturnToQuestGiver { .. } => npc_pos_fallback,
        };
    }
    npc_pos_fallback
}

/// The primary in-game scene.
///
/// Holds all transient gameplay state: input buffer, modifier-key flags,
/// scroll positions, pending stat raises, minimap pixel buffer, and escape
/// menu state. Created fresh each time the player enters the game world.
pub struct GameScene {
    pub(super) weapon_armor_panel: WeaponArmorPanel,
    pub(super) rank_sigil: RankSigil,
    pub(super) chat_box: ChatBox,
    pub(super) hud_buttons: HudButtonBar,
    pub(super) rank_progress_line: RankProgressLine,
    pub(super) skills_panel: SkillsPanel,
    pub(super) talent_panel: TalentPanel,
    pub(super) quest_log_panel: crate::ui::hud::quest_log_panel::QuestLogPanel,
    pub(super) inventory_panel: InventoryPanel,
    pub(super) settings_panel: SettingsPanel,
    pub(super) minimap_widget: MinimapWidget,
    pub(super) mode_button: ModeButton,
    pub(super) look_panel: LookPanel,
    pub(super) shop_panel: ShopPanel,
    pub(super) vitality_bars: VitalityChevrons,
    pub(super) spell_effect_icons: SpellEffectIcons,
    pub(super) skill_bar: SkillBar,
    pub(super) skill_picker: SkillPickerPopup,
    pub(super) last_synced_log_len: usize,
    pub(super) pending_exit: Option<String>,
    pub(super) certificate_mismatch: Option<cert_trust::FingerprintMismatch>,
    /// SDL2 certificate-mismatch dialog (created on demand when a mismatch is detected).
    cert_dialog: Option<CertDialog>,
    pub(super) ctrl_held: bool,
    pub(super) shift_held: bool,
    pub(super) alt_held: bool,
    /// Whether a mouse side button currently contributes Ctrl behavior.
    pub(super) mouse_ctrl_held: bool,
    /// Whether a mouse side button currently contributes Shift behavior.
    pub(super) mouse_shift_held: bool,
    /// Whether the controller's left bumper (LB) is held.
    pub(super) lb_held: bool,
    /// Whether the controller's right bumper (RB) is held.
    pub(super) rb_held: bool,
    /// Whether the controller's left trigger (LT) is past the press threshold.
    pub(super) lt_held: bool,
    /// Whether the controller's right trigger (RT) is past the press threshold.
    pub(super) rt_held: bool,
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
    /// World-coordinate keys `(x, y)` of tombstone tiles for which a
    /// `CmdAutoloot` has already been sent this session.  Prevents
    /// re-sending the command every tick for the same grave.
    /// Cleared on scene enter (new game session / re-login).
    pub(super) autoloot_visited: HashSet<(u16, u16)>,
    /// When set, the player has right-clicked a skill and is choosing a spell-bar slot.
    /// Value is the skilltab index of the skill being assigned.
    pub(super) pending_skill_assignment: Option<usize>,
    pub(super) active_profile_character: Option<CharacterIdentity>,
    /// Wall-clock profiler for rendering functions (activated from escape menu).
    perf_profiler: PerfProfiler,
    /// Active client-side weather/ambient overlay state.
    pub(super) weather: weather::WeatherState,
    /// `true` when the player is using a game controller (mirrors
    /// `AppState::controller_active`). Stored locally so `handle_event` can
    /// read it without re-borrowing `AppState`.
    pub(super) controller_mode: bool,
    /// Virtual cursor X position (sub-pixel) for controller-driven cursor movement.
    pub(super) vcursor_x: f32,
    /// Virtual cursor Y position (sub-pixel) for controller-driven cursor movement.
    pub(super) vcursor_y: f32,
    /// Current raw left-stick X axis value (−32768..32767), updated each frame.
    pub(super) left_stick_x: i16,
    /// Current raw left-stick Y axis value (−32768..32767), updated each frame.
    pub(super) left_stick_y: i16,
    /// Current raw right-stick X axis value (−32768..32767), updated each frame.
    pub(super) right_stick_x: i16,
    /// Current raw right-stick Y axis value (−32768..32767), updated each frame.
    pub(super) right_stick_y: i16,
    /// Cooldown timer (seconds) to debounce right-stick skill bar navigation.
    pub(super) right_stick_cooldown: f32,
    /// Timestamp of the most recent left-stick press (L3) for
    /// short-press (select) vs hold (look) detection.
    pub(super) l3_pressed_at: Option<Instant>,
    /// Controller navigation tracker for HUD panels (settings menu, etc.).
    pub(super) hud_nav: crate::ui::controller_nav::ControllerNavState,
    /// Rising-edge flag: left-stick X was positive (right) last frame, for keyboard nav.
    pub(super) kb_stick_pos_x: bool,
    /// Rising-edge flag: left-stick X was negative (left) last frame, for keyboard nav.
    pub(super) kb_stick_neg_x: bool,
    /// Rising-edge flag: left-stick Y was positive (down) last frame, for keyboard nav.
    pub(super) kb_stick_pos_y: bool,
    /// Rising-edge flag: left-stick Y was negative (up) last frame, for keyboard nav.
    pub(super) kb_stick_neg_y: bool,
    /// On-screen keyboard for controller chat input.
    keyboard: OnScreenKeyboard,
    /// Seconds elapsed since the mouse last left the right-side HUD zone.
    hud_btn_idle_elapsed: f32,
    /// Current fade factor for right-side HUD buttons (0.0 = invisible, 1.0 = opaque).
    hud_btn_fade_t: f32,
    /// Result receiver for an active background diagnostics network-test run.
    network_test_result_rx: Option<Receiver<NetworkTestRunResult>>,
    /// `true` while a diagnostics network-test worker is active.
    network_test_running: bool,
    /// Cancellation flag for the active diagnostics network-test worker.
    network_test_cancel: Option<Arc<AtomicBool>>,
}

impl GameScene {
    /// Rewrite saved Blast/Lava Blast bindings to the currently learned replacement.
    ///
    /// # Arguments
    ///
    /// * `app_state` - Mutable application state containing active character settings.
    /// * `skills` - Latest skill rows received from the server.
    fn normalize_lava_blast_keybinds(
        &self,
        app_state: &mut AppState<'_>,
        skills: &[[u8; SkillIndex::MaxIndex as usize]],
    ) {
        if normalize_lava_blast_keybind_arrays(
            &mut app_state.settings.character.skill_keybinds,
            &mut app_state.settings.character.skill_keybinds_secondary,
            skills,
        ) {
            self.save_active_profile(app_state);
        }
    }

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
        let mut keyboard = OnScreenKeyboard::new();
        let keyboard_y = TARGET_HEIGHT_INT as i32
            - keyboard.bounds().height as i32
            - SkillBar::height() as i32
            - 8;
        keyboard.set_position(keyboard.bounds().x, keyboard_y);

        // Compute spell-effect icon positions:
        // - Positive icons start at the first skill-bar slot's x position.
        // - Negative icons right-align with the weapon/armor panel.
        let weapon_armor_panel =
            WeaponArmorPanel::new(WEAPON_ARMOR_PANEL_X, WEAPON_ARMOR_PANEL_Y, HUD_PANEL_BG);
        let positive_start_x =
            (TARGET_WIDTH_INT as i32 - SkillBar::width() as i32) / 2 + TOP_CELL_POSITIONS[0].0;
        let wap_bounds = weapon_armor_panel.bounds();
        let negative_right_x = wap_bounds.x + wap_bounds.width as i32;

        Self {
            weapon_armor_panel,
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
            ),
            rank_progress_line: RankProgressLine::new(
                (TARGET_WIDTH_INT as i32 - 370) / 2,
                TARGET_HEIGHT_INT as i32 - 40,
                370,
                4,
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
            talent_panel: TalentPanel::new(
                Bounds::new(panel_x, panel_y, HUD_PANEL_W, HUD_PANEL_H),
                HUD_PANEL_BG,
            ),
            quest_log_panel: crate::ui::hud::quest_log_panel::QuestLogPanel::new(
                Bounds::new(panel_x, panel_y, HUD_PANEL_W, HUD_PANEL_H),
                HUD_PANEL_BG,
            ),
            minimap_widget: MinimapWidget::new(MINIMAP_BTN_CX, MINIMAP_BTN_CY, MINIMAP_BTN_RADIUS),
            mode_button: ModeButton::new(MODE_BTN_CX, MODE_BTN_CY, MODE_BTN_RADIUS),
            vitality_bars: VitalityChevrons::new(VITALITY_BARS_X, VITALITY_BARS_Y),
            spell_effect_icons: SpellEffectIcons::new(
                positive_start_x,
                negative_right_x,
                VITALITY_BARS_Y,
            ),
            look_panel: LookPanel::new(
                Bounds::new(LOOK_PANEL_X, LOOK_PANEL_Y, LOOK_PANEL_W, LOOK_PANEL_H),
                HUD_PANEL_BG,
            ),
            shop_panel: ShopPanel::new(
                Bounds::new(SHOP_PANEL_X, SHOP_PANEL_Y, SHOP_PANEL_W, SHOP_PANEL_H),
                HUD_PANEL_BG,
            ),
            skill_bar: SkillBar::new(),
            skill_picker: SkillPickerPopup::new(),
            last_synced_log_len: 0,
            pending_exit: None,
            certificate_mismatch: None,
            cert_dialog: None,
            ctrl_held: false,
            shift_held: false,
            alt_held: false,
            mouse_ctrl_held: false,
            mouse_shift_held: false,
            lb_held: false,
            rb_held: false,
            lt_held: false,
            rt_held: false,
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
            autoloot_visited: HashSet::new(),
            pending_skill_assignment: None,
            active_profile_character: None,
            perf_profiler: PerfProfiler::new(),
            weather: weather::WeatherState::new(),
            controller_mode: false,
            vcursor_x: TARGET_WIDTH_INT as f32 / 2.0,
            vcursor_y: TARGET_HEIGHT_INT as f32 / 2.0,
            left_stick_x: 0,
            left_stick_y: 0,
            right_stick_x: 0,
            right_stick_y: 0,
            right_stick_cooldown: 0.0,
            l3_pressed_at: None,
            hud_nav: crate::ui::controller_nav::ControllerNavState::new(),
            kb_stick_pos_x: false,
            kb_stick_neg_x: false,
            kb_stick_pos_y: false,
            kb_stick_neg_y: false,
            keyboard,
            hud_btn_idle_elapsed: 0.0,
            hud_btn_fade_t: 1.0,
            network_test_result_rx: None,
            network_test_running: false,
            network_test_cancel: None,
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
            .map(|t| u32::from(t.ch_nr))
            .unwrap_or(0)
    }

    /// Resolve the default skill target.
    ///
    /// Priority matches expected gameplay behavior:
    /// 1) Explicitly selected character (Alt+click), unless that character is ourselves
    /// 2) Current attack target (`attack_cn`)
    /// 3) No target (0)
    pub(super) fn default_skill_target(ps: &PlayerState) -> u32 {
        let selected = u32::from(ps.selected_char());
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
            weather_enabled: app_state.settings.weather_enabled,
            show_names: app_state.settings.show_names,
            show_health_pct: app_state.settings.show_proz,
            hide_walls: app_state.settings.hide,
            show_helper_text: app_state.settings.show_helper_text,
            show_positions: app_state.settings.show_positions,
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
            key_bindings: app_state.settings.character.key_bindings.clone(),
            controller_bindings: app_state.settings.character.controller_bindings.clone(),
            mouse_modifier_bindings: app_state.settings.character.mouse_modifier_bindings.clone(),
        }
    }

    /// Returns whether Ctrl-like behavior is currently active.
    ///
    /// # Returns
    ///
    /// * `true` when physical/controller Ctrl state or a mouse binding is held.
    pub(super) fn effective_ctrl_held(&self) -> bool {
        self.ctrl_held || self.mouse_ctrl_held
    }

    /// Returns whether Shift-like behavior is currently active.
    ///
    /// # Returns
    ///
    /// * `true` when physical/controller Shift state or a mouse binding is held.
    pub(super) fn effective_shift_held(&self) -> bool {
        self.shift_held || self.mouse_shift_held
    }

    /// Builds the current effective modifier set for UI events.
    ///
    /// # Returns
    ///
    /// * Current Ctrl/Shift/Alt state, including mouse-derived modifiers.
    pub(super) fn effective_key_modifiers(&self) -> KeyModifiers {
        KeyModifiers {
            ctrl: self.effective_ctrl_held(),
            shift: self.effective_shift_held(),
            alt: self.alt_held,
        }
    }

    /// Applies a raw extra mouse-button event to mouse-derived modifier state.
    ///
    /// Returns `true` when the event was Mouse 4/Mouse 5 and should be
    /// consumed before normal widget/world click handling.
    ///
    /// # Arguments
    ///
    /// * `app_state` - Shared application state.
    /// * `button` - Extra mouse button from the SDL event.
    /// * `pressed` - `true` for button down, `false` for button up.
    ///
    /// # Returns
    ///
    /// * `(consumed, scene_change)` for the raw mouse-button event.
    fn handle_extra_mouse_button_event(
        &mut self,
        app_state: &mut AppState<'_>,
        button: ExtraMouseButton,
        pressed: bool,
    ) -> (bool, Option<SceneType>) {
        if pressed && self.settings_panel.is_mouse_modifier_listening() {
            self.settings_panel.capture_mouse_modifier_button(button);
            self.mouse_ctrl_held = false;
            self.mouse_shift_held = false;
            let scene_change = self.process_settings_panel_actions(app_state);
            return (true, scene_change);
        }

        match app_state
            .settings
            .character
            .mouse_modifier_bindings
            .modifier_for_button(button)
        {
            Some(MouseModifier::Ctrl) => self.mouse_ctrl_held = pressed,
            Some(MouseModifier::Shift) => self.mouse_shift_held = pressed,
            None => {}
        }

        (true, None)
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
                WidgetAction::SetWeather(v) => {
                    app_state.settings.weather_enabled = v;
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
                WidgetAction::SetShowPositions(v) => {
                    app_state.settings.show_positions = v;
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
                WidgetAction::SendClientLogs => {
                    if self.settings_panel.is_visible() {
                        self.settings_panel.toggle();
                    }
                    self.send_latest_client_log(app_state);
                }
                WidgetAction::RunNetworkTest => {
                    self.settings_panel.close();
                    self.start_network_test(app_state);
                }
                WidgetAction::UpdateKeyBinding { action, binding } => {
                    app_state
                        .settings
                        .character
                        .key_bindings
                        .set_binding(action, binding);
                    profile_changed = true;
                }
                WidgetAction::UpdateControllerBinding { slot, button } => {
                    app_state
                        .settings
                        .character
                        .controller_bindings
                        .set(slot as usize, button);
                    profile_changed = true;
                }
                WidgetAction::UpdateMouseModifierBinding { modifier, button } => {
                    app_state
                        .settings
                        .character
                        .mouse_modifier_bindings
                        .set(modifier, button);
                    self.mouse_ctrl_held = false;
                    self.mouse_shift_held = false;
                    profile_changed = true;
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

    /// Compresses and uploads the latest client log file to the diagnostics API.
    ///
    /// # Arguments
    ///
    /// * `app_state` - Shared application state carrying API/session data.
    fn send_latest_client_log(&self, app_state: &mut AppState<'_>) {
        let Some(login_target) = app_state.api.login_target.as_ref() else {
            log::warn!("Diagnostics upload skipped: no active login target");
            if let Some(ps) = app_state.player_state.as_mut() {
                ps.tlog(
                    1,
                    "Failed to send logs: no active character session.".to_owned(),
                );
            }
            return;
        };
        let Some(token) = app_state.api.token.as_deref() else {
            log::warn!("Diagnostics upload skipped: no auth token");
            if let Some(ps) = app_state.player_state.as_mut() {
                ps.tlog(1, "Failed to send logs: not authenticated.".to_owned());
            }
            return;
        };
        let character_id = login_target.character_id;

        let log_path = preferences::log_file_path();
        let log_bytes = match std::fs::read(&log_path) {
            Ok(value) => value,
            Err(err) => {
                log::warn!(
                    "Diagnostics upload failed reading log {}: {err}",
                    log_path.display()
                );
                if let Some(ps) = app_state.player_state.as_mut() {
                    ps.tlog(
                        1,
                        format!("Failed to read log file: {}", log_path.display()),
                    );
                }
                return;
            }
        };
        let (compressed, retained_log_bytes) = match compress_log_for_upload(&log_bytes) {
            Ok(value) => value,
            Err(err) => {
                log::warn!("Diagnostics upload failed preparing log payload: {err}");
                if let Some(ps) = app_state.player_state.as_mut() {
                    ps.tlog(1, format!("Failed to send logs: {err}"));
                }
                return;
            }
        };

        if retained_log_bytes < log_bytes.len() {
            log::info!(
                "Diagnostics upload trimming client log from {} to {} bytes",
                log_bytes.len(),
                retained_log_bytes
            );
        }

        match account_api::upload_client_log(
            &app_state.api.base_url,
            token,
            character_id,
            &compressed,
        ) {
            Ok(saved_file) => {
                if let Some(ps) = app_state.player_state.as_mut() {
                    if retained_log_bytes < log_bytes.len() {
                        ps.tlog(
                            1,
                            format!(
                                "Diagnostics uploaded: {saved_file} (latest {} bytes)",
                                retained_log_bytes
                            ),
                        );
                    } else {
                        ps.tlog(1, format!("Diagnostics uploaded: {saved_file}"));
                    }
                }
            }
            Err(err) => {
                log::warn!("Diagnostics upload request failed: {err}");
                if let Some(ps) = app_state.player_state.as_mut() {
                    ps.tlog(1, format!("Failed to send logs: {err}"));
                }
            }
        }
    }

    /// Starts a timed asynchronous diagnostics network-test run.
    ///
    /// # Arguments
    ///
    /// * `app_state` - Shared application state carrying API/session data.
    fn start_network_test(&mut self, app_state: &mut AppState<'_>) {
        if self.network_test_running {
            if let Some(ps) = app_state.player_state.as_mut() {
                ps.tlog(1, "Network test already running...".to_owned());
            }
            return;
        }

        let Some(login_target) = app_state.api.login_target.as_ref() else {
            log::warn!("Network test skipped: no active login target");
            if let Some(ps) = app_state.player_state.as_mut() {
                ps.tlog(
                    1,
                    "Failed to start network test: no active character session.".to_owned(),
                );
            }
            return;
        };
        let Some(token) = app_state.api.token.clone() else {
            log::warn!("Network test skipped: no auth token");
            if let Some(ps) = app_state.player_state.as_mut() {
                ps.tlog(
                    1,
                    "Failed to start network test: not authenticated.".to_owned(),
                );
            }
            return;
        };

        let character_id = login_target.character_id;
        let base_url = app_state.api.base_url.clone();
        let run_suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        let run_id = format!("nettest-{}-{}", character_id, run_suffix);

        if let Some(ps) = app_state.player_state.as_mut() {
            ps.tlog(
                1,
                format!("Network test started ({}s)...", NETWORK_TEST_DURATION_SECS),
            );
        }

        let (tx, rx) = mpsc::channel::<NetworkTestRunResult>();
        let cancel = Arc::new(AtomicBool::new(false));
        self.network_test_result_rx = Some(rx);
        self.network_test_running = true;
        self.network_test_cancel = Some(cancel.clone());

        std::thread::spawn(move || {
            let test_started = Instant::now();
            let client = match cert_trust::build_reqwest_client_with_timeout(Duration::from_millis(
                NETWORK_TEST_REQUEST_TIMEOUT_MS,
            )) {
                Ok(value) => value,
                Err(err) => {
                    let _ = tx.send(NetworkTestRunResult {
                        run_id,
                        metrics: NetworkTestMetrics {
                            duration_ms: 0,
                            total_samples: 0,
                            failed_samples: 0,
                            min_rtt_ms: None,
                            avg_rtt_ms: None,
                            max_rtt_ms: None,
                            jitter_ms: None,
                            quality_rating: "Poor".to_owned(),
                        },
                        summary_submit_error: Some(err),
                        cancelled: false,
                    });
                    return;
                }
            };
            let mut successful_rtts: Vec<u32> = Vec::new();
            let mut failed_samples: u32 = 0;
            let mut sample_index: u32 = 0;
            let mut next_sample_deadline = test_started;

            while test_started.elapsed() < Duration::from_secs(NETWORK_TEST_DURATION_SECS)
                && !cancel.load(Ordering::Relaxed)
            {
                let sample_started = Instant::now();
                let client_payload = build_network_test_client_payload(sample_index);
                let expected_server_payload_bytes = network_test_server_payload_bytes(sample_index);
                match account_api::run_network_test_probe(
                    &client,
                    &base_url,
                    &token,
                    character_id,
                    &run_id,
                    sample_index,
                    &client_payload,
                    expected_server_payload_bytes,
                ) {
                    Ok(_) => {
                        let rtt_ms = sample_started
                            .elapsed()
                            .as_millis()
                            .min(u128::from(u32::MAX)) as u32;
                        successful_rtts.push(rtt_ms);
                    }
                    Err(err) => {
                        failed_samples = failed_samples.saturating_add(1);
                        log::warn!(
                            "Network test probe failed (run_id={}, sample={}): {}",
                            run_id,
                            sample_index,
                            err
                        );
                    }
                }
                sample_index = sample_index.saturating_add(1);

                next_sample_deadline += Duration::from_millis(NETWORK_TEST_SAMPLE_INTERVAL_MS);
                let now = Instant::now();
                if next_sample_deadline > now {
                    std::thread::sleep(next_sample_deadline - now);
                }
            }

            let duration_ms = test_started.elapsed().as_millis().min(u128::from(u32::MAX)) as u32;
            let total_samples = sample_index;
            let min_rtt_ms = successful_rtts.iter().copied().min();
            let max_rtt_ms = successful_rtts.iter().copied().max();
            let avg_rtt_ms = if successful_rtts.is_empty() {
                None
            } else {
                let sum: u64 = successful_rtts.iter().map(|value| u64::from(*value)).sum();
                Some((sum / successful_rtts.len() as u64).min(u64::from(u32::MAX)) as u32)
            };
            let jitter_ms = estimate_jitter_ms(&successful_rtts);
            let quality_rating =
                classify_network_quality(avg_rtt_ms, failed_samples, total_samples);

            let metrics = NetworkTestMetrics {
                duration_ms,
                total_samples,
                failed_samples,
                min_rtt_ms,
                avg_rtt_ms,
                max_rtt_ms,
                jitter_ms,
                quality_rating,
            };

            let cancelled = cancel.load(Ordering::Relaxed);
            let summary_submit_error = if cancelled || metrics.total_samples == 0 {
                None
            } else {
                account_api::submit_network_test_summary(
                    &client,
                    &base_url,
                    &token,
                    character_id,
                    &run_id,
                    NetworkTestSummary {
                        duration_ms: metrics.duration_ms,
                        total_samples: metrics.total_samples,
                        failed_samples: metrics.failed_samples,
                        min_rtt_ms: metrics.min_rtt_ms,
                        avg_rtt_ms: metrics.avg_rtt_ms,
                        max_rtt_ms: metrics.max_rtt_ms,
                        jitter_ms: metrics.jitter_ms,
                        quality_rating: metrics.quality_rating.clone(),
                    },
                )
                .err()
            };

            let _ = tx.send(NetworkTestRunResult {
                run_id,
                metrics,
                summary_submit_error,
                cancelled,
            });
        });
    }

    /// Requests cancellation of any active diagnostics network-test worker.
    fn cancel_network_test(&mut self) {
        if let Some(cancel) = self.network_test_cancel.take() {
            cancel.store(true, Ordering::Relaxed);
        }
        self.network_test_result_rx = None;
        self.network_test_running = false;
    }

    /// Polls for completion of an active diagnostics network-test run.
    ///
    /// # Arguments
    ///
    /// * `app_state` - Shared application state carrying player log state.
    fn poll_network_test_result(&mut self, app_state: &mut AppState<'_>) {
        if !self.network_test_running {
            return;
        }

        let recv_result = match self.network_test_result_rx.as_ref() {
            Some(rx) => rx.try_recv(),
            None => {
                self.network_test_running = false;
                return;
            }
        };

        let result = match recv_result {
            Ok(value) => value,
            Err(TryRecvError::Empty) => return,
            Err(TryRecvError::Disconnected) => {
                self.network_test_running = false;
                self.network_test_result_rx = None;
                self.network_test_cancel = None;
                log::warn!("Network test worker disconnected before publishing a result");
                return;
            }
        };

        self.network_test_running = false;
        self.network_test_result_rx = None;
        self.network_test_cancel = None;

        if result.cancelled {
            log::info!("Network test cancelled: run_id={}", result.run_id);
            return;
        }

        let metrics = result.metrics;
        log::info!(
            "Network test completed: run_id={} duration_ms={} total_samples={} failed_samples={} min_rtt_ms={:?} avg_rtt_ms={:?} max_rtt_ms={:?} jitter_ms={:?} quality={}",
            result.run_id,
            metrics.duration_ms,
            metrics.total_samples,
            metrics.failed_samples,
            metrics.min_rtt_ms,
            metrics.avg_rtt_ms,
            metrics.max_rtt_ms,
            metrics.jitter_ms,
            metrics.quality_rating
        );

        if let Some(ps) = app_state.player_state.as_mut() {
            ps.tlog(
                1,
                format!(
                    "Network Test: {} | samples={} failed={} duration={}ms",
                    metrics.quality_rating,
                    metrics.total_samples,
                    metrics.failed_samples,
                    metrics.duration_ms
                ),
            );
            ps.tlog(
                1,
                format!(
                    "Latency: min={} avg={} max={} jitter={}",
                    format_optional_ms(metrics.min_rtt_ms),
                    format_optional_ms(metrics.avg_rtt_ms),
                    format_optional_ms(metrics.max_rtt_ms),
                    format_optional_ms(metrics.jitter_ms)
                ),
            );
        }

        if let Some(err) = result.summary_submit_error {
            log::warn!("Network test summary submission failed: {}", err);
            if let Some(ps) = app_state.player_state.as_mut() {
                ps.tlog(1, format!("Network test summary upload failed: {err}"));
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
                if let Some(tile) = ps.map().tile_at_xy(x, y)
                    && tile.ch_nr == selected
                {
                    return true;
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
        if self.rank_sigil.is_visible() && self.rank_sigil.bounds().contains_point(mx, my) {
            return true;
        }
        if self.vitality_bars.contains_point(mx, my) {
            return true;
        }
        if self.spell_effect_icons.contains_point(mx, my) {
            return true;
        }
        if self.weapon_armor_panel.bounds().contains_point(mx, my) {
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

        if self.quest_log_panel.is_visible() && self.quest_log_panel.bounds().contains_point(mx, my)
        {
            return true;
        }

        if self.settings_panel.is_visible() && self.settings_panel.bounds().contains_point(mx, my) {
            return true;
        }

        false
    }

    /// Returns `true` when a visible panel drawn above the skills panel is
    /// under the cursor.
    fn is_mouse_over_ui_above_skills_panel(&self) -> bool {
        let (mx, my) = (self.mouse_x, self.mouse_y);
        (self.inventory_panel.is_visible() && self.inventory_panel.bounds().contains_point(mx, my))
            || (self.settings_panel.is_visible()
                && self.settings_panel.bounds().contains_point(mx, my))
            || (self.talent_panel.is_visible() && self.talent_panel.bounds().contains_point(mx, my))
            || (self.quest_log_panel.is_visible()
                && self.quest_log_panel.bounds().contains_point(mx, my))
            || (self.shop_panel.is_visible() && self.shop_panel.bounds().contains_point(mx, my))
            || (self.skill_picker.is_visible() && self.skill_picker.bounds().contains_point(mx, my))
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
        show_positions: bool,
    ) -> Result<(), String> {
        if !show_helper_text {
            return Ok(());
        }
        if show_positions {
            let text = format!("({},{})", self.mouse_x, self.mouse_y);
            return self.draw_cursor_helper_text(canvas, gfx, &text);
        }
        // Show the rank name as a tooltip when hovering the rank sigil.
        if self.rank_sigil.is_hovered() {
            return self.draw_cursor_helper_text(canvas, gfx, self.rank_sigil.rank_name());
        }
        if let Some(text) = self.rank_progress_line.hover_text() {
            return self.draw_cursor_helper_text(canvas, gfx, &text);
        }
        if let Some(text) = self.vitality_bars.hover_text() {
            return self.draw_cursor_helper_text(canvas, gfx, &text);
        }
        if let Some(text) = self.spell_effect_icons.hover_text() {
            return self.draw_cursor_helper_text(canvas, gfx, &text);
        }
        if let Some(text) = self.skill_bar.hover_text() {
            return self.draw_cursor_helper_text(canvas, gfx, &text);
        }
        if let Some(text) = self.hud_buttons.hover_text() {
            return self.draw_cursor_helper_text(canvas, gfx, text);
        }
        if let Some(text) = self.minimap_widget.hover_text() {
            return self.draw_cursor_helper_text(canvas, gfx, text);
        }
        if let Some(text) = self.mode_button.hover_text() {
            return self.draw_cursor_helper_text(canvas, gfx, text);
        }
        if !self.is_mouse_over_ui_above_skills_panel()
            && let Some(text) = self.skills_panel.hover_text()
        {
            return self.draw_cursor_helper_text(canvas, gfx, text);
        }
        if self.is_mouse_over_ui() {
            return Ok(());
        }
        let Some(text) = self.resolve_helper_text(ps) else {
            return Ok(());
        };
        self.draw_cursor_helper_text(canvas, gfx, text)
    }

    /// Draws wrapped helper text near the cursor, repositioning the block to
    /// stay fully on screen.
    ///
    /// The block defaults to the bottom-right of the cursor, flips to the
    /// left/above when it would overflow the right/bottom edges, and is
    /// clamped to a `HELPER_TEXT_SCREEN_MARGIN`-pixel inset as a final safety
    /// net (see [`helper_text_origin`]).
    ///
    /// # Arguments
    ///
    /// * `canvas` - SDL2 canvas.
    /// * `gfx` - Graphics/texture cache.
    /// * `text` - Helper text to draw.
    ///
    /// # Returns
    ///
    /// * `Ok(())` on success, or an SDL2 error string.
    fn draw_cursor_helper_text(
        &self,
        canvas: &mut Canvas<Window>,
        gfx: &mut GraphicsCache<'_>,
        text: &str,
    ) -> Result<(), String> {
        let max_width = HELPER_TEXT_MAX_CHARS * crate::font_cache::BITMAP_GLYPH_ADVANCE;
        let (text_w, text_h) = crate::font_cache::measure_wrapped_bitmap(text, max_width);
        let (x, y) = helper_text_origin(
            self.mouse_x,
            self.mouse_y,
            text_w as i32,
            text_h as i32,
            TARGET_WIDTH_INT as i32,
            TARGET_HEIGHT_INT as i32,
        );
        crate::font_cache::draw_wrapped_text(
            canvas,
            gfx,
            1,
            text,
            x,
            y,
            max_width,
            crate::font_cache::TextStyle::drop_shadow(),
        )
        .map(|_| ())
    }

    /// Draw a crosshair cursor at the virtual cursor position when controller
    /// mode is active.
    ///
    /// # Arguments
    ///
    /// * `canvas` - SDL2 canvas.
    ///
    /// # Returns
    ///
    /// * `Ok(())` on success, or an SDL2 error string.
    fn draw_controller_cursor(&self, canvas: &mut Canvas<Window>) -> Result<(), String> {
        let cx = self.mouse_x;
        let cy = self.mouse_y;
        let size = 8i32; // arm length in pixels

        // White crosshair with slight transparency
        canvas.set_blend_mode(sdl2::render::BlendMode::Blend);
        canvas.set_draw_color(Color::RGBA(255, 255, 255, 220));

        // Horizontal line
        canvas.draw_line(
            sdl2::rect::Point::new(cx - size, cy),
            sdl2::rect::Point::new(cx + size, cy),
        )?;
        // Vertical line
        canvas.draw_line(
            sdl2::rect::Point::new(cx, cy - size),
            sdl2::rect::Point::new(cx, cy + size),
        )?;

        // Small center dot
        canvas.set_draw_color(Color::RGBA(255, 255, 100, 255));
        canvas.draw_point(sdl2::rect::Point::new(cx, cy))?;
        canvas.draw_point(sdl2::rect::Point::new(cx + 1, cy))?;
        canvas.draw_point(sdl2::rect::Point::new(cx, cy + 1))?;
        canvas.draw_point(sdl2::rect::Point::new(cx + 1, cy + 1))?;

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
            .ok_or_else(|| "No login target".to_owned())?;

        let host = crate::hosts::get_host_from_api_base_url(&app_state.api.base_url)
            .unwrap_or_else(crate::hosts::get_server_ip);

        log::info!(
            "GameScene: connecting to {}:5555 with ticket={} (api_base_url={})",
            host,
            login_target.ticket,
            app_state.api.base_url
        );

        if let Some(mut net) = app_state.network.take() {
            net.shutdown();
        }

        app_state.network = Some(NetworkRuntime::new(host, 5555, login_target.ticket));

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
        self.ctrl_held = false;
        self.shift_held = false;
        self.alt_held = false;
        self.mouse_ctrl_held = false;
        self.mouse_shift_held = false;
        self.lb_held = false;
        self.rb_held = false;
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
        self.autoloot_visited.clear();
        self.pending_skill_assignment = None;
        self.active_profile_character = None;
        self.vcursor_x = TARGET_WIDTH_INT as f32 / 2.0;
        self.vcursor_y = TARGET_HEIGHT_INT as f32 / 2.0;
        self.left_stick_x = 0;
        self.left_stick_y = 0;
        self.cancel_network_test();

        app_state.settings.spell_effects_enabled = true;
        app_state.settings.character.key_bindings = KeyBindings::default();
        app_state.settings.master_volume = 1.0;

        let login_target = match app_state.api.login_target.clone() {
            Some(t) => t,
            None => {
                log::error!("GameScene on_enter: no login_target set");
                self.pending_exit = Some("No login target".to_owned());
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
        self.cancel_network_test();

        if let Some(mut net) = app_state.network.take() {
            net.shutdown();
        }
        app_state.player_state = None;
        self.weather.reset();
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

            if self.inventory_panel.is_visible() {
                self.inventory_panel.toggle();
                self.inventory_panel.clear_controller_selection();
            }

            if self.skills_panel.is_visible() {
                self.skills_panel.toggle();
                self.skills_panel.clear_controller_focus();
            }

            if self.quest_log_panel.is_visible() {
                self.quest_log_panel.toggle();
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

        match event {
            Event::MouseButtonDown { mouse_btn, .. } => {
                if let Some(button) = ExtraMouseButton::from_sdl2(*mouse_btn) {
                    let (consumed, scene_change) =
                        self.handle_extra_mouse_button_event(app_state, button, true);
                    if consumed {
                        return scene_change;
                    }
                }
            }
            Event::MouseButtonUp { mouse_btn, .. } => {
                if let Some(button) = ExtraMouseButton::from_sdl2(*mouse_btn) {
                    let (consumed, scene_change) =
                        self.handle_extra_mouse_button_event(app_state, button, false);
                    if consumed {
                        return scene_change;
                    }
                }
            }
            _ => {}
        }

        // --- UI widget stack ---
        let mut ui_consumed = false;
        if let Some(ui_event) = ui::sdl_to_ui_event(
            event,
            self.mouse_x,
            self.mouse_y,
            self.effective_key_modifiers(),
        ) {
            match self.handle_ui_widget_events(app_state, &ui_event) {
                net_events::UiHandleResult::SceneChange(sc) => return Some(sc),
                net_events::UiHandleResult::Consumed => {
                    ui_consumed = true;
                }
                net_events::UiHandleResult::NotConsumed => {}
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
            if (has_modifier || !self.chat_box.is_focused())
                && let Some(action) = app_state
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

        // --- Controller events ---
        if matches!(
            event,
            Event::ControllerButtonDown { .. }
                | Event::ControllerButtonUp { .. }
                | Event::ControllerAxisMotion { .. }
        ) {
            return self.handle_controller_event(app_state, event);
        }

        // --- Num0-9 hotkeys ---
        if let Event::KeyDown {
            keycode: Some(kc), ..
        } = event
            && matches!(
                *kc,
                Keycode::Num0
                    | Keycode::Num1
                    | Keycode::Num2
                    | Keycode::Num3
                    | Keycode::Num4
                    | Keycode::Num5
                    | Keycode::Num6
                    | Keycode::Num7
                    | Keycode::Num8
                    | Keycode::Num9
            )
        {
            self.handle_num_hotkey(app_state, *kc);
            return None;
        }

        // --- Mouse world interactions ---
        if !ui_consumed
            && let Event::MouseButtonUp {
                mouse_btn, x, y, ..
            } = event
        {
            return self.handle_world_click(app_state, *mouse_btn, *x, *y);
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
        self.weapon_armor_panel.update(dt);
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
        self.poll_network_test_result(app_state);

        // --- Right-side HUD button fade ---
        {
            let dt_secs = dt.as_secs_f32();
            if self.mouse_x > HUD_FADE_THRESHOLD_X {
                self.hud_btn_idle_elapsed = 0.0;
                self.hud_btn_fade_t = (self.hud_btn_fade_t + dt_secs / HUD_FADE_IN_SECS).min(1.0);
            } else {
                self.hud_btn_idle_elapsed += dt_secs;
                if self.hud_btn_idle_elapsed >= HUD_FADE_OUT_DELAY_SECS {
                    self.hud_btn_fade_t =
                        (self.hud_btn_fade_t - dt_secs / HUD_FADE_OUT_SECS).max(0.0);
                }
            }
            let alpha = (self.hud_btn_fade_t * 255.0) as u8;
            self.hud_buttons.set_alpha(alpha);
            self.mode_button.set_alpha(alpha);
            if self.minimap_widget.is_visible() {
                self.minimap_widget.set_button_alpha(255);
            } else {
                self.minimap_widget.set_button_alpha(alpha);
            }
        }

        // Sync controller mode from the central AppState flag.
        self.controller_mode = app_state.controller_active;

        // --- Virtual cursor movement (controller mode) ---
        // Suppress cursor movement while the on-screen keyboard, settings
        // panel, shop overlay, or skill picker popup is visible so the left
        // stick doesn't drift the crosshair underneath modal overlays.
        let modal_ui_open = self.keyboard.is_visible()
            || self.settings_panel.is_visible()
            || self.shop_panel.is_visible()
            || self.skill_picker.is_visible();
        if self.controller_mode && !modal_ui_open {
            const DEADZONE: f32 = 8000.0;
            const MAX_AXIS: f32 = 32767.0;
            const CURSOR_SPEED: f32 = 300.0; // pixels per second

            let dt_secs = dt.as_secs_f32();

            let raw_x = f32::from(self.left_stick_x);
            let raw_y = f32::from(self.left_stick_y);

            let norm_x = if raw_x.abs() > DEADZONE {
                ((raw_x.abs() - DEADZONE) / (MAX_AXIS - DEADZONE))
                    .min(1.0)
                    .copysign(raw_x)
            } else {
                0.0
            };
            let norm_y = if raw_y.abs() > DEADZONE {
                ((raw_y.abs() - DEADZONE) / (MAX_AXIS - DEADZONE))
                    .min(1.0)
                    .copysign(raw_y)
            } else {
                0.0
            };

            self.vcursor_x += norm_x * CURSOR_SPEED * dt_secs;
            self.vcursor_y += norm_y * CURSOR_SPEED * dt_secs;

            // Clamp to viewport
            self.vcursor_x = self.vcursor_x.clamp(0.0, TARGET_WIDTH_INT as f32 - 1.0);
            self.vcursor_y = self.vcursor_y.clamp(0.0, TARGET_HEIGHT_INT as f32 - 1.0);

            // Override mouse_x/mouse_y so all existing consumers use the virtual cursor
            self.mouse_x = self.vcursor_x as i32;
            self.mouse_y = self.vcursor_y as i32;

            // Dispatch a synthetic MouseMove so widgets see hover state
            // changes from the virtual cursor (left stick) just like they
            // would from a real mouse.
            let synthetic_move = UiEvent::MouseMove {
                x: self.mouse_x,
                y: self.mouse_y,
            };
            // A MouseMove is not expected to trigger a scene change, but
            // handle it defensively without returning early — the rest of
            // update() (network processing, L3 hold, etc.) must still run.
            self.handle_ui_widget_events(app_state, &synthetic_move);

            // Sync modifier flags from bumpers so that helper text,
            // hover highlights, and tile snapping work with the controller
            // the same way they do with keyboard Shift/Ctrl.
            self.shift_held = self.lb_held;
            self.ctrl_held = self.rb_held;
        }

        // --- Right-stick navigation (controller mode) ---
        if self.controller_mode {
            self.right_stick_cooldown = (self.right_stick_cooldown - dt.as_secs_f32()).max(0.0);

            const RS_DEADZONE: f32 = 8000.0;
            if self.skill_picker.is_visible() {
                let rs_y = f32::from(self.right_stick_y);
                if self.right_stick_cooldown <= 0.0 && rs_y.abs() > RS_DEADZONE {
                    self.skill_picker
                        .controller_move_selection(if rs_y > 0.0 { 1 } else { -1 });
                    self.right_stick_cooldown = 0.2;
                }
            } else {
                use crate::ui::hud::skill_bar::TOP_CELLS;

                let rs_x = f32::from(self.right_stick_x);

                if self.right_stick_cooldown <= 0.0 && rs_x.abs() > RS_DEADZONE {
                    let current = self.skill_bar.controller_selected_slot();
                    let next = if rs_x > 0.0 {
                        // Right → advance (wrap 12 → 0)
                        Some(current.map_or(0, |s| (s + 1) % TOP_CELLS))
                    } else {
                        // Left → retreat (wrap 0 → 12)
                        Some(current.map_or(TOP_CELLS - 1, |s| {
                            if s == 0 { TOP_CELLS - 1 } else { s - 1 }
                        }))
                    };
                    self.skill_bar.set_controller_selected_slot(next);
                    self.right_stick_cooldown = 0.2;
                }
            }
        }

        // --- L3 hold detection: look at nearest character ---
        if self.controller_mode
            && let Some(pressed_at) = self.l3_pressed_at
        {
            const L3_HOLD_THRESHOLD: Duration = Duration::from_millis(500);
            if pressed_at.elapsed() >= L3_HOLD_THRESHOLD {
                self.l3_pressed_at = None; // consumed
                if let Some(ps) = app_state.player_state.as_ref() {
                    let (cam_xoff, cam_yoff) = Self::camera_offsets(ps);
                    if let Some((mx, my)) =
                        Self::screen_to_map_tile(self.mouse_x, self.mouse_y, cam_xoff, cam_yoff)
                    {
                        use mag_core::constants::ISCHAR;
                        if let Some((sx, sy)) = Self::nearest_tile_with_flag(ps, mx, my, ISCHAR) {
                            let tile = ps.map().tile_at_xy(sx, sy);
                            let target_cn = tile.map(|t| u32::from(t.ch_nr)).unwrap_or(0);
                            if target_cn != 0
                                && let Some(net) = app_state.network.as_ref()
                            {
                                self.play_click_sound(app_state);
                                net.send(ClientCommand::new_look(target_cn));
                            }
                        }
                    }
                }
            }
        }

        // Create the cert dialog widget when a mismatch is first detected.
        if let Some(m) = &self.certificate_mismatch
            && self.cert_dialog.is_none()
        {
            self.cert_dialog = Some(CertDialog::new(
                &m.host,
                &m.expected_fingerprint,
                &m.received_fingerprint,
            ));
        }

        let scene = self.process_network_events(app_state);
        if scene.is_none() {
            if let Some(ps) = app_state.player_state.as_mut()
                && !Self::is_selected_visible(ps)
            {
                ps.clear_selected_char();
            }

            let tick_now = app_state
                .network
                .as_ref()
                .map(|net| net.client_ticker)
                .unwrap_or(0);
            if tick_now != self.last_look_tick {
                self.last_look_tick = tick_now;
                self.maybe_send_autolook_and_shop_refresh(app_state);
                self.maybe_send_autoloot_graves(app_state);
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
        if let Some(skills) = app_state
            .player_state
            .as_ref()
            .map(|ps| ps.character_info().skill)
        {
            self.normalize_lava_blast_keybinds(app_state, &skills);
        }

        self.perf_profiler.begin_frame();

        // Split borrow: gfx_cache (mut) and player_state (ref) are separate fields.
        let AppState {
            ref mut gfx_cache,
            ref mut text_engine,
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

        // Advance weather state up-front so its shake offset is available to
        // the world camera below. Rendering the weather overlay still happens
        // *after* the world pass so particles/tints layer on top.
        if settings.weather_enabled {
            self.weather
                .update_auto(TARGET_WIDTH_INT as i32, TARGET_HEIGHT_INT as i32);
        } else {
            // Pause (not reset) so re-enabling resumes the same effect; the
            // server only re-sends when the resolved state changes.
            self.weather.pause();
        }
        let camera_shake = if settings.weather_enabled {
            self.weather.shake_offset()
        } else {
            (0, 0)
        };

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
            camera_shake,
        )?;
        self.perf_profiler.end_sample(PerfLabel::DrawWorld);

        // 1b. Weather / ambient overlay (rendered above world tiles, below HUD).
        self.perf_profiler.begin_sample(PerfLabel::DrawWeather);
        if settings.weather_enabled {
            self.weather.render_post_world(canvas)?;
        }
        self.perf_profiler.end_sample(PerfLabel::DrawWeather);

        // 5. Chat log + input line (via ChatBox widget)
        self.perf_profiler.begin_sample(PerfLabel::DrawChat);
        {
            let mut ctx = RenderContext {
                canvas,
                gfx: gfx_cache,
                text: text_engine,
            };
            self.chat_box.render(&mut ctx)?;
            self.keyboard.render(&mut ctx)?;
        }
        self.perf_profiler.end_sample(PerfLabel::DrawChat);

        // 5a. Rank sigil + status panel (WV/AV)
        self.perf_profiler
            .begin_sample(PerfLabel::SyncAndDrawStatus);
        {
            if let Some(ps) = app_state.player_state.as_ref() {
                let ci = ps.character_info();
                let rank_index = ranks::points2rank(ci.points_tot as u32);
                self.rank_sigil.sync(rank_index as usize);
                self.weapon_armor_panel.sync(ci.weapon, ci.armor);
                let wap = self.weapon_armor_panel.bounds();
                self.spell_effect_icons.negative_right_x = wap.x + wap.width as i32;
                self.rank_progress_line.sync(ci.points_tot as u32);
                self.mode_button.sync(ci.mode);
                self.vitality_bars.sync(
                    ci.a_hp,
                    i32::from(ci.hp[5]),
                    ci.a_end,
                    i32::from(ci.end[5]),
                    ci.a_mana,
                    i32::from(ci.mana[5]),
                );
                self.spell_effect_icons
                    .sync(&ci.spell, &ci.active, &ci.spell_type);
                use crate::ui::hud::skills_panel::{SkillsPanel as SP, SkillsPanelData};
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
                self.talent_panel
                    .sync_state(*ps.talents(), class_from_kindred(ci.kindred));
                self.hud_buttons.set_talent_points_badge(
                    mag_core::talent_trees::available_talent_points(ps.talents()),
                );
                use crate::ui::hud::inventory_panel::InventoryPanelData;
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

                // Skill bar: keybinds for the 11 assignable skill slots.
                {
                    use crate::preferences::NUMBER_OF_KEYBINDS;
                    use crate::ui::hud::skill_bar::SkillBarData;
                    let mut keybinds = [None; NUMBER_OF_KEYBINDS];
                    keybinds.copy_from_slice(
                        &app_state.settings.character.skill_keybinds[..NUMBER_OF_KEYBINDS],
                    );
                    let mut secondary_keybinds = [None; NUMBER_OF_KEYBINDS];
                    secondary_keybinds.copy_from_slice(
                        &app_state.settings.character.skill_keybinds_secondary
                            [..NUMBER_OF_KEYBINDS],
                    );
                    let show_secondary =
                        self.effective_shift_held() || (self.controller_mode && self.lt_held);
                    self.skill_bar.update_data(SkillBarData {
                        keybinds,
                        secondary_keybinds,
                        show_secondary,
                    });
                }

                // Update minimap xmap buffer, then push viewport pixels to the widget.
                if let Some((cx, cy)) = self.update_minimap_xmap(gfx_cache, ps) {
                    self.minimap_widget
                        .update_viewport(&self.minimap_xmap, cx, cy);
                }

                // --- Quest log panel data + minimap quest markers ---
                {
                    use crate::ui::hud::quest_log_panel::{
                        QuestEntryDisplay, QuestLogPanelData, QuestTitle,
                    };
                    let catalog = ps.quest_catalog();
                    let counts = ps.quest_completion_counts();
                    let active_template = ps.active_quest_template_id();
                    let active_step_idx = ps.active_quest_step_idx() as usize;
                    let active_npc_pos = ps.active_quest_npc_pos();

                    let mut display_entries: Vec<QuestEntryDisplay> = Vec::new();
                    for (idx, entry) in catalog.iter().enumerate() {
                        let count = counts.get(idx).copied().unwrap_or(-1);
                        // Skip quests the player has not yet discovered
                        // (server uses -1 sentinel until the player gets
                        // close enough for the NPC to sight them).
                        if count < 0 {
                            continue;
                        }
                        // Decide how many "open" stage rows to emit for this NPC.
                        let stage_rows: u8 = if entry.repeatable {
                            1
                        } else {
                            let stages = entry.stages.max(1);
                            (0..stages).filter(|s| count <= i16::from(*s)).count() as u8
                        };
                        if stage_rows == 0 {
                            continue;
                        }
                        let (title, description, steps) =
                            match mag_core::quest_defs::find_quest_def(entry.template_id) {
                                Some(def) => {
                                    let steps_str: Vec<String> = def
                                        .steps
                                        .iter()
                                        .map(|s| {
                                            match s {
                                            mag_core::quest_defs::QuestStep::FixedLocation {
                                                x,
                                                y,
                                                desc,
                                            } => format!("• {desc} ({x},{y})"),
                                            mag_core::quest_defs::QuestStep::ReturnToQuestGiver {
                                                desc,
                                            } => format!("• {desc}"),
                                        }
                                        })
                                        .collect();
                                    (
                                        QuestTitle::Plain(def.title.to_owned()),
                                        def.description.to_owned(),
                                        steps_str,
                                    )
                                }
                                None => (
                                    QuestTitle::BringItemToNpc {
                                        item_name: entry.item_name.clone(),
                                        npc_name: entry.npc_name.clone(),
                                    },
                                    String::new(),
                                    Vec::new(),
                                ),
                            };
                        for _ in 0..stage_rows {
                            display_entries.push(QuestEntryDisplay {
                                template_id: entry.template_id,
                                title: title.clone(),
                                description: description.clone(),
                                steps: steps.clone(),
                                npc_x: entry.npc_x,
                                npc_y: entry.npc_y,
                            });
                        }
                    }

                    self.quest_log_panel.update_data(QuestLogPanelData {
                        entries: display_entries,
                        active_template_id: active_template,
                    });

                    // Minimap markers: every quest giver in the catalog.
                    let givers: Vec<(u16, u16)> =
                        catalog.iter().map(|e| (e.npc_x, e.npc_y)).collect();
                    let active_marker = if active_template == 0 {
                        None
                    } else {
                        active_quest_destination(active_template, active_step_idx, active_npc_pos)
                    };
                    self.minimap_widget.set_quest_markers(givers, active_marker);
                }
            }
            let mut ctx = RenderContext {
                canvas,
                gfx: gfx_cache,
                text: text_engine,
            };
            if self.rank_sigil.is_visible() {
                self.rank_sigil.render(&mut ctx)?;
            }
            self.weapon_armor_panel.render(&mut ctx)?;
            self.vitality_bars.render(&mut ctx)?;
            self.spell_effect_icons.render(&mut ctx)?;
        }
        self.perf_profiler.end_sample(PerfLabel::SyncAndDrawStatus);

        // 5b. HUD panels + button bar (rendered after chat, before legacy HUD)
        self.perf_profiler.begin_sample(PerfLabel::DrawHudPanels);
        {
            let mut ctx = RenderContext {
                canvas,
                gfx: gfx_cache,
                text: text_engine,
            };
            self.skills_panel.render(&mut ctx)?;
            self.inventory_panel.render(&mut ctx)?;
            self.settings_panel.render(&mut ctx)?;
            self.talent_panel.render(&mut ctx)?;
            self.quest_log_panel.render(&mut ctx)?;
            self.hud_buttons.render(&mut ctx)?;
            self.minimap_widget.render(&mut ctx)?;
            self.mode_button.render(&mut ctx)?;
            self.skill_bar.render(&mut ctx)?;
            self.weapon_armor_panel.render(&mut ctx)?;
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
                text: text_engine,
            };
            self.look_panel.render(&mut ctx)?;
        }
        self.perf_profiler.end_sample(PerfLabel::DrawLookPanel);

        // 5d. Shop/depot/grave overlay (centered, when active)
        self.perf_profiler.begin_sample(PerfLabel::DrawShopPanel);
        {
            use crate::ui::hud::shop_panel::ShopPanelData;
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
                text: text_engine,
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

        // 5e-ii. Controller cursor (crosshair drawn when controller mode is active
        // and no modal panel is open)
        if self.controller_mode
            && !self.settings_panel.is_visible()
            && !self.shop_panel.is_visible()
            && !self.skill_picker.is_visible()
        {
            self.draw_controller_cursor(canvas)?;
        }

        // 5f. Context-sensitive helper text near the cursor
        self.perf_profiler.begin_sample(PerfLabel::DrawHelperText);
        if let Some(ps) = app_state.player_state.as_ref() {
            self.draw_helper_text(
                canvas,
                gfx_cache,
                ps,
                app_state.settings.show_helper_text,
                app_state.settings.show_positions,
            )?;
        }
        self.perf_profiler.end_sample(PerfLabel::DrawHelperText);

        self.perf_profiler.end_frame();

        // Render cert dialog as final overlay.
        {
            let mut ctx = RenderContext {
                canvas,
                gfx: gfx_cache,
                text: text_engine,
            };
            if let Some(ref mut dialog) = self.cert_dialog {
                dialog.render(&mut ctx)?;
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{
        GameScene, HELPER_TEXT_CURSOR_FLIP_GAP_Y, HELPER_TEXT_CURSOR_GAP_X,
        HELPER_TEXT_CURSOR_GAP_Y, HELPER_TEXT_SCREEN_MARGIN, MAX_CLIENT_LOG_UPLOAD_BYTES,
        NETWORK_TEST_CLIENT_PAYLOAD_BYTES, base64_encoded_len, build_network_test_client_payload,
        classify_network_quality, compress_log_for_upload, estimate_jitter_ms, helper_text_origin,
        network_test_server_payload_bytes, newest_log_slice_for_upload,
        normalize_lava_blast_keybind_arrays,
    };
    use flate2::read::GzDecoder;
    use mag_core::skills::{SK_BLAST, SK_LAVA_BLAST, SkillIndex};
    use std::io::Read;

    const SCREEN_W: i32 = 800;
    const SCREEN_H: i32 = 600;

    fn empty_skill_rows() -> [[u8; SkillIndex::MaxIndex as usize]; 100] {
        [[0; SkillIndex::MaxIndex as usize]; 100]
    }

    #[test]
    fn helper_text_default_anchor_in_center() {
        let (x, y) = helper_text_origin(400, 300, 60, 20, SCREEN_W, SCREEN_H);
        assert_eq!(x, 400 + HELPER_TEXT_CURSOR_GAP_X);
        assert_eq!(y, 300 + HELPER_TEXT_CURSOR_GAP_Y);
    }

    #[test]
    fn helper_text_flips_left_near_right_edge() {
        let cursor_x = SCREEN_W - 20;
        let text_w = 120;
        let (x, _) = helper_text_origin(cursor_x, 100, text_w, 20, SCREEN_W, SCREEN_H);
        assert_eq!(x, cursor_x - HELPER_TEXT_CURSOR_GAP_X - text_w);
    }

    #[test]
    fn helper_text_flips_above_near_bottom_edge() {
        let cursor_y = SCREEN_H - 10;
        let text_h = 30;
        let (_, y) = helper_text_origin(100, cursor_y, 60, text_h, SCREEN_W, SCREEN_H);
        assert_eq!(y, cursor_y - HELPER_TEXT_CURSOR_FLIP_GAP_Y - text_h);
    }

    #[test]
    fn helper_text_clamps_when_larger_than_either_side() {
        // Cursor near right edge with a tooltip wider than the space to its
        // left after flipping — the clamp keeps it within the margin.
        let text_w = SCREEN_W; // wider than the screen
        let (x, _) = helper_text_origin(SCREEN_W - 10, 100, text_w, 20, SCREEN_W, SCREEN_H);
        assert_eq!(x, HELPER_TEXT_SCREEN_MARGIN);
    }

    #[test]
    fn effective_modifiers_include_mouse_held_state() {
        let mut scene = GameScene::new();

        scene.mouse_ctrl_held = true;
        scene.mouse_shift_held = true;
        let modifiers = scene.effective_key_modifiers();

        assert!(scene.effective_ctrl_held());
        assert!(scene.effective_shift_held());
        assert!(modifiers.ctrl);
        assert!(modifiers.shift);
        assert!(!modifiers.alt);

        scene.mouse_ctrl_held = false;
        scene.ctrl_held = true;
        assert!(scene.effective_ctrl_held());
    }

    #[test]
    fn lava_blast_keybind_normalization_rewrites_blast_when_replacement_is_learned() {
        let mut primary = [None; 10];
        let mut secondary = [None; 10];
        primary[0] = Some(SK_BLAST);
        secondary[1] = Some(SK_BLAST);
        let mut skills = empty_skill_rows();
        skills[SK_LAVA_BLAST][SkillIndex::BaseValue as usize] = 4;

        let changed = normalize_lava_blast_keybind_arrays(&mut primary, &mut secondary, &skills);

        assert!(changed);
        assert_eq!(primary[0], Some(SK_LAVA_BLAST));
        assert_eq!(secondary[1], Some(SK_LAVA_BLAST));
    }

    #[test]
    fn lava_blast_keybind_normalization_rewrites_back_after_reset() {
        let mut primary = [None; 10];
        let mut secondary = [None; 10];
        primary[0] = Some(SK_LAVA_BLAST);
        secondary[1] = Some(SK_LAVA_BLAST);
        let mut skills = empty_skill_rows();
        skills[SK_BLAST][SkillIndex::BaseValue as usize] = 4;

        let changed = normalize_lava_blast_keybind_arrays(&mut primary, &mut secondary, &skills);

        assert!(changed);
        assert_eq!(primary[0], Some(SK_BLAST));
        assert_eq!(secondary[1], Some(SK_BLAST));
    }

    #[test]
    fn jitter_estimate_uses_average_delta() {
        assert_eq!(estimate_jitter_ms(&[100, 110, 90, 120]), Some(20));
        assert_eq!(estimate_jitter_ms(&[100]), None);
    }

    #[test]
    fn quality_classification_respects_latency_and_failures() {
        assert_eq!(classify_network_quality(Some(90), 0, 20), "Good");
        assert_eq!(classify_network_quality(Some(90), 2, 20), "Fair");
        assert_eq!(classify_network_quality(Some(260), 0, 20), "Poor");
        assert_eq!(classify_network_quality(Some(90), 5, 20), "Poor");
    }

    #[test]
    fn network_test_payload_profile_matches_protocol_shape() {
        let payload = build_network_test_client_payload(12);
        assert_eq!(payload.len(), NETWORK_TEST_CLIENT_PAYLOAD_BYTES);
        assert_eq!(
            payload[0],
            mag_core::client_commands::ClientCommandType::Ping as u8
        );
        assert_eq!(network_test_server_payload_bytes(0), 2);
        assert_eq!(network_test_server_payload_bytes(1), 18);
        assert_eq!(network_test_server_payload_bytes(2), 31);
        assert_eq!(network_test_server_payload_bytes(3), 50);
        assert_eq!(network_test_server_payload_bytes(4), 2);
    }

    #[test]
    fn compress_log_for_upload_keeps_small_logs_intact() {
        let log = b"hello\nworld\n".repeat(128);

        let (compressed, retained_bytes) = compress_log_for_upload(&log).unwrap();

        assert_eq!(retained_bytes, log.len());
        assert!(!compressed.is_empty());
        assert!(base64_encoded_len(compressed.len()) > 0);
    }

    #[test]
    fn compress_log_for_upload_trims_to_upload_window() {
        let log = vec![b'x'; MAX_CLIENT_LOG_UPLOAD_BYTES + 4096];

        let (_compressed, retained_bytes) = compress_log_for_upload(&log).unwrap();

        assert_eq!(retained_bytes, MAX_CLIENT_LOG_UPLOAD_BYTES);
    }

    #[test]
    fn newest_log_slice_for_upload_skips_partial_first_line() {
        let log = b"first line\nsecond line\nthird line\n";

        let (slice, retained_bytes) = newest_log_slice_for_upload(log, log.len() - 3);

        assert_eq!(slice, b"second line\nthird line\n");
        assert_eq!(retained_bytes, slice.len());
    }

    #[test]
    fn compress_log_for_upload_outputs_complete_first_line_after_trim() {
        let mut log = b"pan=0\n2026-05-02 full line\n2026-05-02 another line\n".to_vec();
        log.extend(vec![b'x'; MAX_CLIENT_LOG_UPLOAD_BYTES + 3 - log.len()]);

        let (compressed, _retained_bytes) = compress_log_for_upload(&log).unwrap();
        let mut decoded = String::new();
        GzDecoder::new(compressed.as_slice())
            .read_to_string(&mut decoded)
            .unwrap();

        assert!(decoded.starts_with("2026-05-02"));
        assert!(!decoded.starts_with("=0"));
    }
}
