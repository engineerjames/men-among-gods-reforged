use std::{
    collections::BTreeMap,
    fmt, fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::types::controller::ControllerBindings;
use crate::types::mouse::MouseModifierBindings;
use crate::ui::widget::KeyBindings;

/// Number of skill-bar binding slots.
pub const NUMBER_OF_KEYBINDS: usize = 10;

/// Persisted position and size of a movable UI window.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WindowLayout {
    /// Left edge in logical viewport pixels.
    pub x: i32,
    /// Top edge in logical viewport pixels.
    pub y: i32,
    /// Width in logical viewport pixels.
    pub width: u32,
    /// Height in logical viewport pixels.
    pub height: u32,
}

// ---------------------------------------------------------------------------
// Per-character settings
// ---------------------------------------------------------------------------

/// Settings that are scoped to a specific character.
///
/// These are persisted inside each character's entry in the profile file and
/// are never shared across characters or stored in the global section.
/// Only data that is truly character-specific lives here: skill keybinds,
/// keyboard action bindings, and remembered panel positions.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CharacterSettings {
    /// Skill keybinds for the primary bar slots 1-10. `Some(skill_nr)` if bound.
    #[serde(default)]
    pub skill_keybinds: [Option<usize>; NUMBER_OF_KEYBINDS],
    /// Skill keybinds for the secondary bar slots 1-10, accessed by holding
    /// Shift (keyboard/mouse) or LT (controller). `Some(skill_nr)` if bound.
    #[serde(default)]
    pub skill_keybinds_secondary: [Option<usize>; NUMBER_OF_KEYBINDS],
    /// Saved position of the inventory panel, or `None` for default.
    #[serde(default)]
    pub inventory_panel_pos: Option<(i32, i32)>,
    /// Saved position of the skills panel, or `None` for default.
    #[serde(default)]
    pub skills_panel_pos: Option<(i32, i32)>,
    /// Saved position of the settings panel, or `None` for default.
    #[serde(default)]
    pub settings_panel_pos: Option<(i32, i32)>,
    /// Saved position and size of the chat window, or `None` for default.
    #[serde(default)]
    pub chat_window: Option<WindowLayout>,
    /// Saved position of the expanded minimap window, or `None` for default.
    #[serde(default)]
    pub minimap_window_pos: Option<(i32, i32)>,
    /// Keyboard bindings mapping game actions to key combinations.
    #[serde(default)]
    pub key_bindings: KeyBindings,
    /// Controller button bindings for skill-bar slots 1–9.
    #[serde(default)]
    pub controller_bindings: ControllerBindings,
    /// Mouse side-button bindings that temporarily mimic Ctrl/Shift.
    #[serde(default)]
    pub mouse_modifier_bindings: MouseModifierBindings,
    /// Whether graves adjacent to the player are automatically looted on
    /// each server tick. Defaults to `true`. Toggle with `/autoloot`.
    #[serde(default = "default_auto_loot_graves")]
    pub auto_loot_graves: bool,
}

/// Returns the default value of `true` for
/// [`CharacterSettings::auto_loot_graves`].
fn default_auto_loot_graves() -> bool {
    true
}

impl Default for CharacterSettings {
    fn default() -> Self {
        Self {
            skill_keybinds: [None; NUMBER_OF_KEYBINDS],
            skill_keybinds_secondary: [None; NUMBER_OF_KEYBINDS],
            inventory_panel_pos: None,
            skills_panel_pos: None,
            settings_panel_pos: None,
            chat_window: None,
            minimap_window_pos: None,
            key_bindings: KeyBindings::default(),
            controller_bindings: ControllerBindings::default(),
            mouse_modifier_bindings: MouseModifierBindings::default(),
            auto_loot_graves: true,
        }
    }
}

/// Window display mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum DisplayMode {
    #[default]
    Windowed,
    Fullscreen,
    BorderlessFullscreen,
}

impl fmt::Display for DisplayMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Windowed => write!(f, "Windowed"),
            Self::Fullscreen => write!(f, "Fullscreen"),
            Self::BorderlessFullscreen => write!(f, "Borderless Fullscreen"),
        }
    }
}

impl DisplayMode {
    /// All variants in UI display order.
    pub const ALL: [DisplayMode; 3] = [
        DisplayMode::Windowed,
        DisplayMode::Fullscreen,
        DisplayMode::BorderlessFullscreen,
    ];
}

/// Internal rendering resolution, expressed as a multiplier of the logical
/// 960x540 frame.
///
/// The whole scene is composed into an off-screen buffer of
/// `scale * 960 x scale * 540` pixels and then blitted to the window. Higher
/// scales sharpen TrueType text and vector primitives, and are a prerequisite
/// for the sprite upscalers (see [`SpriteUpscaler`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum RenderScale {
    /// Native logical resolution (960x540). Lowest cost, classic look.
    #[default]
    X1,
    /// 2x internal resolution (1920x1080).
    X2,
    /// 3x internal resolution (2880x1620).
    X3,
    /// Pick the largest scale that fits the current window, capped at 3x.
    Auto,
}

impl fmt::Display for RenderScale {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::X1 => write!(f, "1x (Native)"),
            Self::X2 => write!(f, "2x"),
            Self::X3 => write!(f, "3x"),
            Self::Auto => write!(f, "Auto"),
        }
    }
}

impl RenderScale {
    /// All variants in UI display order.
    pub const ALL: [RenderScale; 4] = [
        RenderScale::X1,
        RenderScale::X2,
        RenderScale::X3,
        RenderScale::Auto,
    ];

    /// Highest internal render multiplier the pipeline will ever use.
    pub const MAX_FACTOR: u32 = 3;

    /// Resolves this setting to a concrete integer multiplier.
    ///
    /// # Arguments
    /// * `drawable_w` - Width of the window drawable area in physical pixels.
    /// * `drawable_h` - Height of the window drawable area in physical pixels.
    /// * `logical_w` - Logical frame width (e.g. 960).
    /// * `logical_h` - Logical frame height (e.g. 540).
    ///
    /// # Returns
    /// * A multiplier in `1..=MAX_FACTOR`.
    pub fn factor(self, drawable_w: u32, drawable_h: u32, logical_w: u32, logical_h: u32) -> u32 {
        match self {
            Self::X1 => 1,
            Self::X2 => 2,
            Self::X3 => 3,
            Self::Auto => {
                if logical_w == 0 || logical_h == 0 {
                    return 1;
                }
                let by_w = drawable_w / logical_w.max(1);
                let by_h = drawable_h / logical_h.max(1);
                by_w.min(by_h).clamp(1, Self::MAX_FACTOR)
            }
        }
    }

    /// Maps a concrete integer multiplier back onto an explicit variant.
    ///
    /// Used when the renderer could not allocate a target at the requested
    /// scale and the downgraded value has to be written back to settings.
    /// Never returns [`RenderScale::Auto`], since a resolved factor carries no
    /// information about whether it was chosen automatically.
    ///
    /// # Arguments
    /// * `factor` - Multiplier actually in effect; values outside
    ///   `1..=MAX_FACTOR` are clamped.
    ///
    /// # Returns
    /// * The matching explicit variant.
    pub fn from_factor(factor: u32) -> Self {
        match factor.clamp(1, Self::MAX_FACTOR) {
            3 => Self::X3,
            2 => Self::X2,
            _ => Self::X1,
        }
    }
}

/// Filtering applied when the composed frame buffer is scaled onto the window.
///
/// Because the whole scene is composed into a single texture first, filtering
/// here never bleeds neighbouring sprites into each other — unlike per-sprite
/// filtering, which would produce seams between adjacent floor tiles.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum OutputFilter {
    /// Nearest-neighbour. Crisp, classic, but uneven pixel sizes at
    /// non-integer scale factors.
    #[default]
    Nearest,
    /// Bilinear filtering across the whole screen. Smooth, slightly soft.
    Linear,
    /// Integer prescale followed by bilinear for the remainder. Keeps pixels
    /// square while removing the uneven "pixel shimmer" of pure nearest.
    SharpBilinear,
}

impl fmt::Display for OutputFilter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Nearest => write!(f, "Nearest (Sharp)"),
            Self::Linear => write!(f, "Linear (Smooth)"),
            Self::SharpBilinear => write!(f, "Sharp Bilinear"),
        }
    }
}

impl OutputFilter {
    /// All variants in UI display order.
    pub const ALL: [OutputFilter; 3] = [
        OutputFilter::Nearest,
        OutputFilter::Linear,
        OutputFilter::SharpBilinear,
    ];
}

/// Pixel-art upscaling algorithm applied to sprites when they are decoded.
///
/// The upscale factor is not chosen independently: it always matches the
/// active [`RenderScale`] factor, so an upscaled sprite still blits 1:1 into
/// the internal frame buffer. At `RenderScale::X1` this setting has no effect.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum SpriteUpscaler {
    /// No filtering; sprites are point-replicated to the render scale.
    #[default]
    None,
    /// Scale2x / Scale3x (EPX family). Preserves hard edges, minimal blurring.
    Scale2x,
    /// HQ2x / HQ3x. Smoother diagonals and anti-aliased curves.
    Hqx,
}

impl fmt::Display for SpriteUpscaler {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::None => write!(f, "None (Blocky)"),
            Self::Scale2x => write!(f, "Scale2x / EPX"),
            Self::Hqx => write!(f, "HQx"),
        }
    }
}

impl SpriteUpscaler {
    /// All variants in UI display order.
    pub const ALL: [SpriteUpscaler; 3] = [
        SpriteUpscaler::None,
        SpriteUpscaler::Scale2x,
        SpriteUpscaler::Hqx,
    ];
}

const LOG_FILE_NAME: &str = "mag_client.log";
const PERF_LOG_FILE_NAME: &str = "mag_client_perf.log";
const PROFILE_FILE_NAME: &str = "mag_profile.json";
const KNOWN_HOSTS_FILE: &str = "mag_known_hosts.json";

/// Identifies a specific character for profile look-up.
#[derive(Clone, Debug)]
pub struct CharacterIdentity {
    pub id: u64,
    pub name: String,
    pub account_username: Option<String>,
}

/// Unified settings for both global (all-character) and per-character
/// preferences. Loaded from / saved to the JSON profile file.
///
/// Global fields apply to all characters on the machine: audio, display,
/// and gameplay toggles that a player typically wants consistent regardless
/// of which character they log in as.
///
/// Per-character fields are nested in [`CharacterSettings`] and are keyed by
/// character identity, ensuring each character has its own independent
/// skill keybinds and UI layout.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Settings {
    /// Whether background music is enabled.
    #[serde(default = "default_true")]
    pub music_enabled: bool,
    /// Window display mode.
    #[serde(default)]
    pub display_mode: DisplayMode,
    /// Whether pixel-perfect (integer) scaling is active.
    #[serde(default)]
    pub pixel_perfect_scaling: bool,
    /// Whether VSync is enabled.
    #[serde(default = "default_true")]
    pub vsync_enabled: bool,
    /// Internal rendering resolution multiplier.
    #[serde(default)]
    pub render_scale: RenderScale,
    /// Filtering applied when the composed frame is scaled onto the window.
    #[serde(default)]
    pub output_filter: OutputFilter,
    /// Pixel-art upscaling algorithm applied to sprites at decode time.
    #[serde(default)]
    pub sprite_upscaler: SpriteUpscaler,
    /// Whether shadow rendering is enabled.
    #[serde(default = "default_true")]
    pub shadows_enabled: bool,
    /// Whether spell visual effects are rendered.
    #[serde(default = "default_true")]
    pub spell_effects_enabled: bool,
    /// Whether weather / ambient particle effects are rendered.
    #[serde(default = "default_true")]
    pub weather_enabled: bool,
    /// Master volume (0.0–1.0).
    #[serde(default)]
    pub master_volume: f32,
    /// Wall-hiding toggle.
    #[serde(default)]
    pub hide: bool,
    /// Overhead player name display toggle.
    #[serde(default = "default_true")]
    pub show_names: bool,
    /// Overhead health percentage display toggle.
    #[serde(default = "default_true")]
    pub show_proz: bool,
    /// Whether context-sensitive helper text is shown near the cursor.
    #[serde(default = "default_true")]
    pub show_helper_text: bool,
    /// Whether helper text is replaced with the cursor's logical screen position.
    #[serde(default)]
    pub show_positions: bool,
    /// Per-character settings (skill keybinds and UI panel positions).
    #[serde(default)]
    pub character: CharacterSettings,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            music_enabled: true,
            display_mode: DisplayMode::default(),
            pixel_perfect_scaling: false,
            vsync_enabled: true,
            render_scale: RenderScale::default(),
            output_filter: OutputFilter::default(),
            sprite_upscaler: SpriteUpscaler::default(),
            shadows_enabled: true,
            spell_effects_enabled: true,
            weather_enabled: true,
            master_volume: 0.0,
            hide: false,
            show_names: true,
            show_proz: true,
            show_helper_text: true,
            show_positions: false,
            character: CharacterSettings::default(),
        }
    }
}

/// Internal JSON container for a character's saved settings.
#[derive(Clone, Debug, Serialize, Deserialize)]
struct CharacterEntry {
    character_id: u64,
    character_name: String,
    account_username: Option<String>,
    #[serde(flatten)]
    character: CharacterSettings,
}

/// Top-level JSON structure persisted to `mag_profile.json`.
#[derive(Clone, Debug, Serialize, Deserialize)]
struct ProfileStorage {
    version: u32,
    #[serde(default)]
    last_username: Option<String>,
    #[serde(default)]
    global: Settings,
    #[serde(default)]
    characters: BTreeMap<String, CharacterEntry>,
}

impl Default for ProfileStorage {
    fn default() -> Self {
        Self {
            version: 1,
            last_username: None,
            global: Settings::default(),
            characters: BTreeMap::new(),
        }
    }
}

/// Serde helper: returns `true` for default values of new boolean fields.
fn default_true() -> bool {
    true
}

/// Returns a `Settings` snapshot containing only global fields.
///
/// Character-scoped fields are always reset to defaults so account-level
/// profile state cannot leak skill bindings or panel layout between
/// characters.
fn global_settings_only(settings: &Settings) -> Settings {
    Settings {
        music_enabled: settings.music_enabled,
        display_mode: settings.display_mode,
        pixel_perfect_scaling: settings.pixel_perfect_scaling,
        vsync_enabled: settings.vsync_enabled,
        render_scale: settings.render_scale,
        output_filter: settings.output_filter,
        sprite_upscaler: settings.sprite_upscaler,
        shadows_enabled: settings.shadows_enabled,
        spell_effects_enabled: settings.spell_effects_enabled,
        weather_enabled: settings.weather_enabled,
        master_volume: settings.master_volume.clamp(0.0, 1.0),
        hide: settings.hide,
        show_names: settings.show_names,
        show_proz: settings.show_proz,
        show_helper_text: settings.show_helper_text,
        show_positions: settings.show_positions,
        character: CharacterSettings::default(),
    }
}

/// Builds the BTreeMap key used to store a character's profile.
///
/// # Arguments
/// * `identity` - The character to key.
///
/// # Returns
/// * A string in the form `"<username>:<character_id>"`.
fn profile_key(identity: &CharacterIdentity) -> String {
    let username = identity
        .account_username
        .as_deref()
        .unwrap_or("unknown_account");
    format!("{username}:{}", identity.id)
}

/// Returns the directory used for all writable runtime files
/// (profile JSON, log file, etc.) and ensures it exists.
///
/// **macOS / Linux** — files are stored in `~/.men-among-gods/` so that:
///   * macOS `.app` bundles are not broken (Apple prohibits writing inside the
///     bundle, and the OS sets CWD to `/` on launch, making relative paths
///     fail with "permission denied").
///   * Linux follows the convention of a dotfolder in `$HOME`.
///
/// **Windows** — files are stored next to the executable, matching the
/// existing behaviour and expectations for a portable Windows install.
fn data_directory() -> PathBuf {
    #[cfg(unix)]
    {
        // Prefer $HOME; fall back to the exe directory on the rare chance
        // $HOME is unset (e.g. stripped environments / CI containers).
        let dir = std::env::var("HOME")
            .map(|home| PathBuf::from(home).join(".men-among-gods"))
            .unwrap_or_else(|_| {
                std::env::current_exe()
                    .ok()
                    .and_then(|p| p.parent().map(|p| p.to_path_buf()))
                    .unwrap_or_else(|| PathBuf::from("."))
            });

        if let Err(e) = fs::create_dir_all(&dir) {
            eprintln!(
                "Warning: could not create data directory '{}': {}",
                dir.display(),
                e
            );
        }

        dir
    }

    #[cfg(not(unix))]
    {
        // Windows: keep files next to the executable (portable install).
        std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.to_path_buf()))
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
    }
}

/// Returns the path to the JSON profile file (`mag_profile.json`).
///
/// # Returns
///
/// * Value returned by `profile_file_path`.
pub fn profile_file_path() -> PathBuf {
    data_directory().join(PROFILE_FILE_NAME)
}

/// Returns `true` if a saved profile file already exists on disk.
///
/// Used at startup to determine whether this is the first run so that
/// platform-specific defaults can be applied exactly once.
///
/// # Returns
///
/// * `true` if `mag_profile.json` is present, `false` otherwise.
pub fn profile_exists() -> bool {
    profile_file_path().exists()
}

/// Returns the path to the log file (`mag_client.log`).
pub fn log_file_path() -> PathBuf {
    data_directory().join(LOG_FILE_NAME)
}

/// Returns the path to the perf log file (`mag_client_perf.log`).
pub fn perf_log_file_path() -> PathBuf {
    data_directory().join(PERF_LOG_FILE_NAME)
}

/// Returns the path to the trusted hosts file (`known_hosts.json`).
///
/// # Returns
///
/// * Value returned by `known_hosts_file_path`.
pub fn known_hosts_file_path() -> PathBuf {
    data_directory().join(KNOWN_HOSTS_FILE)
}

fn read_storage(path: &Path) -> ProfileStorage {
    let Ok(raw) = fs::read_to_string(path) else {
        return ProfileStorage::default();
    };

    match serde_json::from_str::<ProfileStorage>(&raw) {
        Ok(storage) => storage,
        Err(err) => {
            log::warn!(
                "Failed to parse persisted SDL client profile at {}: {}",
                path.display(),
                err
            );
            ProfileStorage::default()
        }
    }
}

fn write_storage(path: &Path, storage: &ProfileStorage) -> Result<(), String> {
    let tmp_path = path.with_extension("json.tmp");
    let json = serde_json::to_string_pretty(storage)
        .map_err(|err| format!("Failed to serialize profile JSON: {err}"))?;

    fs::write(&tmp_path, json).map_err(|err| {
        format!(
            "Failed to write profile temp file {}: {err}",
            tmp_path.display()
        )
    })?;
    fs::rename(&tmp_path, path)
        .map_err(|err| format!("Failed to replace profile file {}: {err}", path.display()))?;
    Ok(())
}

/// Loads a character's saved settings from disk, merging global and
/// per-character fields into a single [`Settings`].
///
/// # Arguments
/// * `identity` - The character to look up.
///
/// # Returns
/// * The merged character settings. Global fields always come from the
///   global section. Per-character fields come from the character entry
///   when present, otherwise they fall back to defaults.
pub fn load_settings(identity: &CharacterIdentity) -> Settings {
    let path = profile_file_path();
    let storage = read_storage(&path);
    let mut settings = global_settings_only(&storage.global);
    let key = profile_key(identity);
    if let Some(entry) = storage.characters.get(&key) {
        settings.character = entry.character.clone();
    }
    settings
}

/// Loads the global (non-character) settings from disk, returning
/// defaults if the file is missing or corrupt.
///
/// Per-character fields will be at their defaults; callers that only
/// need global fields (e.g. the login scene) should use this.
///
/// # Returns
/// * A [`Settings`] whose global fields are populated from disk.
pub fn load_global_settings() -> Settings {
    let path = profile_file_path();
    let storage = read_storage(&path);
    global_settings_only(&storage.global)
}

/// Persists the global fields of `settings` to the profile file.
///
/// Only global settings fields are written. Per-character fields and the
/// `last_username` value are preserved as-is.
///
/// # Arguments
/// * `settings` - The settings whose global fields to save.
///
/// # Returns
/// * `Ok(())` on success, `Err(String)` with a description on I/O failure.
pub fn save_global_settings(settings: &Settings) -> Result<(), String> {
    let path = profile_file_path();
    let mut storage = read_storage(&path);
    storage.global = global_settings_only(settings);
    write_storage(&path, &storage)
}

/// Returns the username from the most recent successful login, or `None` if
/// no login has been saved yet.
///
/// # Returns
///
/// * `Some` value when `load_last_username` produces one, otherwise `None`.
pub fn load_last_username() -> Option<String> {
    let path = profile_file_path();
    read_storage(&path).last_username
}

/// Persists `username` as the most recently used login name.
///
/// # Arguments
/// * `username` - The account name to remember.
///
/// # Returns
/// * `Ok(())` on success, `Err(String)` with a description on I/O failure.
pub fn save_last_username(username: &str) -> Result<(), String> {
    let path = profile_file_path();
    let mut storage = read_storage(&path);
    storage.last_username = Some(username.to_owned());
    write_storage(&path, &storage)
}

/// Persists a character's settings to the profile file.
///
/// Both the global fields (in the global section) and per-character
/// fields (in the character entry) are written.
///
/// # Arguments
/// * `identity` - The character whose settings to save.
/// * `settings` - The full settings to persist.
///
/// # Returns
/// * `Ok(())` on success, `Err(String)` with a description on I/O failure.
pub fn save_settings(identity: &CharacterIdentity, settings: &Settings) -> Result<(), String> {
    let path = profile_file_path();
    let mut storage = read_storage(&path);
    // Update global fields.
    storage.global = global_settings_only(settings);
    // Insert / update character entry.
    let key = profile_key(identity);
    storage.characters.insert(
        key,
        CharacterEntry {
            character_id: identity.id,
            character_name: identity.name.clone(),
            account_username: identity.account_username.clone(),
            character: settings.character.clone(),
        },
    );
    write_storage(&path, &storage)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_serde_roundtrip() {
        let s = Settings {
            music_enabled: false,
            display_mode: DisplayMode::BorderlessFullscreen,
            shadows_enabled: true,
            master_volume: 0.75,
            show_helper_text: false,
            show_positions: true,
            character: CharacterSettings {
                skill_keybinds: [
                    None,
                    Some(42),
                    None,
                    None,
                    Some(7),
                    None,
                    None,
                    None,
                    None,
                    None,
                ],
                ..CharacterSettings::default()
            },
            ..Settings::default()
        };

        let json = serde_json::to_string(&s).unwrap();
        let deserialized: Settings = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.music_enabled, s.music_enabled);
        assert_eq!(deserialized.display_mode, s.display_mode);
        assert_eq!(deserialized.shadows_enabled, s.shadows_enabled);
        assert!((deserialized.master_volume - s.master_volume).abs() < f32::EPSILON);
        assert_eq!(
            deserialized.character.skill_keybinds,
            s.character.skill_keybinds
        );
        assert_eq!(deserialized.show_helper_text, s.show_helper_text);
        assert_eq!(deserialized.show_positions, s.show_positions);
    }

    #[test]
    fn settings_default_from_empty_json() {
        let deserialized: Settings = serde_json::from_str("{}").unwrap();
        let defaults = Settings::default();

        assert_eq!(deserialized.music_enabled, defaults.music_enabled);
        assert_eq!(deserialized.display_mode, defaults.display_mode);
        assert_eq!(deserialized.shadows_enabled, defaults.shadows_enabled);
        assert!((deserialized.master_volume - defaults.master_volume).abs() < f32::EPSILON);
        assert_eq!(deserialized.show_helper_text, defaults.show_helper_text);
        assert_eq!(deserialized.show_positions, defaults.show_positions);
        assert_eq!(
            deserialized.character.skill_keybinds,
            defaults.character.skill_keybinds
        );
        assert_eq!(
            deserialized.character.mouse_modifier_bindings,
            defaults.character.mouse_modifier_bindings
        );
    }

    #[test]
    fn character_settings_missing_mouse_modifier_bindings_default_unbound() {
        let deserialized: CharacterSettings = serde_json::from_str("{}").unwrap();
        assert_eq!(
            deserialized.mouse_modifier_bindings,
            MouseModifierBindings::default()
        );
    }

    #[test]
    fn settings_missing_show_positions_defaults_false() {
        let deserialized: Settings = serde_json::from_str(r#"{"show_helper_text":true}"#).unwrap();

        assert!(deserialized.show_helper_text);
        assert!(!deserialized.show_positions);
    }

    #[test]
    fn profile_key_with_username() {
        let identity = CharacterIdentity {
            id: 99,
            name: "TestChar".to_owned(),
            account_username: Some("alice".to_owned()),
        };
        assert_eq!(profile_key(&identity), "alice:99");
    }

    #[test]
    fn profile_key_without_username() {
        let identity = CharacterIdentity {
            id: 7,
            name: "NoAccount".to_owned(),
            account_username: None,
        };
        assert_eq!(profile_key(&identity), "unknown_account:7");
    }

    #[test]
    fn profile_storage_serde_roundtrip() {
        let storage = ProfileStorage {
            version: 1,
            last_username: Some("bob".to_owned()),
            global: Settings {
                music_enabled: false,
                vsync_enabled: true,
                ..Settings::default()
            },
            characters: BTreeMap::new(),
        };

        let json = serde_json::to_string_pretty(&storage).unwrap();
        let deserialized: ProfileStorage = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.version, 1);
        assert_eq!(deserialized.last_username.as_deref(), Some("bob"));
        assert!(!deserialized.global.music_enabled);
        assert!(deserialized.global.vsync_enabled);
        assert!(deserialized.characters.is_empty());
    }

    #[test]
    fn character_settings_skill_keybinds_default_all_none() {
        let cs = CharacterSettings::default();
        assert!(cs.skill_keybinds.iter().all(|s| s.is_none()));
    }

    #[test]
    fn character_settings_independent_across_characters() {
        // Two characters with different skill_keybinds must not share data.
        let mut cs1 = CharacterSettings::default();
        let cs2 = CharacterSettings::default();
        cs1.skill_keybinds[0] = Some(5);
        assert_ne!(cs1.skill_keybinds[0], cs2.skill_keybinds[0]);
    }

    #[test]
    fn global_settings_shadows_and_volume_are_not_per_character() {
        // Verify that shadows_enabled and master_volume live on Settings, not CharacterSettings.
        let s = Settings {
            shadows_enabled: true,
            master_volume: 0.5,
            ..Settings::default()
        };
        // CharacterSettings must not have these fields — confirmed by the struct definition.
        assert!(s.shadows_enabled);
        assert!((s.master_volume - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn settings_hud_toggle_defaults() {
        let s = Settings::default();
        assert!(s.show_names);
        assert!(s.show_proz);
        assert!(!s.hide);
        assert!(s.show_helper_text);
        assert!(!s.show_positions);
        assert!(s.spell_effects_enabled);
    }

    #[test]
    fn settings_hud_toggles_serde_roundtrip() {
        let s = Settings {
            show_names: false,
            show_proz: false,
            hide: true,
            show_positions: true,
            spell_effects_enabled: false,
            ..Settings::default()
        };

        let json = serde_json::to_string(&s).unwrap();
        let d: Settings = serde_json::from_str(&json).unwrap();

        assert!(!d.show_names);
        assert!(!d.show_proz);
        assert!(d.hide);
        assert!(d.show_positions);
        assert!(!d.spell_effects_enabled);
    }

    #[test]
    fn character_settings_key_bindings_serde_roundtrip() {
        use crate::ui::widget::KeyBindings;
        let mut cs = CharacterSettings::default();
        let bindings = KeyBindings::default();
        cs.key_bindings = bindings.clone();

        let json = serde_json::to_string(&cs).unwrap();
        let d: CharacterSettings = serde_json::from_str(&json).unwrap();

        assert_eq!(
            serde_json::to_string(&d.key_bindings).unwrap(),
            serde_json::to_string(&cs.key_bindings).unwrap()
        );
    }

    #[test]
    fn character_window_layouts_serde_roundtrip_and_default() {
        let legacy: CharacterSettings = serde_json::from_str("{}").unwrap();
        assert_eq!(legacy.chat_window, None);
        assert_eq!(legacy.minimap_window_pos, None);

        let settings = CharacterSettings {
            chat_window: Some(WindowLayout {
                x: 11,
                y: 22,
                width: 333,
                height: 144,
            }),
            minimap_window_pos: Some((55, 66)),
            ..CharacterSettings::default()
        };
        let json = serde_json::to_string(&settings).unwrap();
        let decoded: CharacterSettings = serde_json::from_str(&json).unwrap();

        assert_eq!(decoded.chat_window, settings.chat_window);
        assert_eq!(decoded.minimap_window_pos, settings.minimap_window_pos);
    }

    #[test]
    fn global_settings_only_clears_character_scoped_fields() {
        let mut settings = Settings::default();
        settings.character.skill_keybinds[0] = Some(42);
        settings.character.inventory_panel_pos = Some((99, 88));
        settings.character.settings_panel_pos = Some((77, 66));
        settings.character.chat_window = Some(WindowLayout {
            x: 1,
            y: 2,
            width: 300,
            height: 200,
        });
        settings.character.minimap_window_pos = Some((33, 44));

        let global = global_settings_only(&settings);

        assert_eq!(global.music_enabled, settings.music_enabled);
        assert_eq!(global.display_mode, settings.display_mode);
        assert!(
            global
                .character
                .skill_keybinds
                .iter()
                .all(|slot| slot.is_none())
        );
        assert_eq!(global.character.inventory_panel_pos, None);
        assert_eq!(global.character.settings_panel_pos, None);
        assert_eq!(global.character.chat_window, None);
        assert_eq!(global.character.minimap_window_pos, None);
    }

    #[test]
    fn global_settings_only_preserves_graphics_pipeline_fields() {
        // `global_settings_only` copies fields by hand, so a newly added global
        // setting that is forgotten there would be silently wiped on save.
        let settings = Settings {
            render_scale: RenderScale::X3,
            output_filter: OutputFilter::SharpBilinear,
            sprite_upscaler: SpriteUpscaler::Hqx,
            ..Settings::default()
        };

        let global = global_settings_only(&settings);

        assert_eq!(global.render_scale, RenderScale::X3);
        assert_eq!(global.output_filter, OutputFilter::SharpBilinear);
        assert_eq!(global.sprite_upscaler, SpriteUpscaler::Hqx);
    }

    #[test]
    fn graphics_pipeline_fields_survive_a_serde_roundtrip() {
        let settings = Settings {
            render_scale: RenderScale::Auto,
            output_filter: OutputFilter::Linear,
            sprite_upscaler: SpriteUpscaler::Scale2x,
            ..Settings::default()
        };

        let json = serde_json::to_string(&settings).unwrap();
        let decoded: Settings = serde_json::from_str(&json).unwrap();

        assert_eq!(decoded.render_scale, RenderScale::Auto);
        assert_eq!(decoded.output_filter, OutputFilter::Linear);
        assert_eq!(decoded.sprite_upscaler, SpriteUpscaler::Scale2x);
    }

    #[test]
    fn graphics_pipeline_fields_default_when_absent_from_stored_json() {
        // Profiles written by older builds have none of these keys.
        let decoded: Settings = serde_json::from_str("{}").unwrap();

        assert_eq!(decoded.render_scale, RenderScale::default());
        assert_eq!(decoded.output_filter, OutputFilter::default());
        assert_eq!(decoded.sprite_upscaler, SpriteUpscaler::default());
    }

    #[test]
    fn auto_render_scale_never_overshoots_the_drawable_size() {
        // 960x540 logical: a 1080p window fits 2x exactly, 4K fits 3x (capped).
        assert_eq!(RenderScale::Auto.factor(1920, 1080, 960, 540), 2);
        assert_eq!(RenderScale::Auto.factor(3840, 2160, 960, 540), 3);
        assert_eq!(RenderScale::Auto.factor(1280, 800, 960, 540), 1);
        // Degenerate sizes must still produce a usable factor.
        assert_eq!(RenderScale::Auto.factor(0, 0, 960, 540), 1);
    }

    #[test]
    fn explicit_render_scales_ignore_the_drawable_size() {
        assert_eq!(RenderScale::X1.factor(3840, 2160, 960, 540), 1);
        assert_eq!(RenderScale::X2.factor(640, 360, 960, 540), 2);
        assert_eq!(RenderScale::X3.factor(640, 360, 960, 540), 3);
    }

    #[test]
    fn from_factor_round_trips_explicit_render_scales() {
        for scale in [RenderScale::X1, RenderScale::X2, RenderScale::X3] {
            let factor = scale.factor(0, 0, 960, 540);
            assert_eq!(RenderScale::from_factor(factor), scale);
        }
    }

    #[test]
    fn from_factor_clamps_out_of_range_values() {
        assert_eq!(RenderScale::from_factor(0), RenderScale::X1);
        assert_eq!(RenderScale::from_factor(99), RenderScale::X3);
    }
}
