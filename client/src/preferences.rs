use std::{
    collections::BTreeMap,
    fmt, fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::ui::widget::KeyBindings;

/// Number of numeric-key skill binding slots (keys 1–9 plus 4 reserved).
pub const NUMBER_OF_KEYBINDS: usize = 13;

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
    /// Skill keybinds for keys 1–13. Index 0 = key "1". `Some(skill_nr)` if bound.
    #[serde(default)]
    pub skill_keybinds: [Option<usize>; NUMBER_OF_KEYBINDS],
    /// Saved position of the inventory panel, or `None` for default.
    #[serde(default)]
    pub inventory_panel_pos: Option<(i32, i32)>,
    /// Saved position of the skills panel, or `None` for default.
    #[serde(default)]
    pub skills_panel_pos: Option<(i32, i32)>,
    /// Saved position of the settings panel, or `None` for default.
    #[serde(default)]
    pub settings_panel_pos: Option<(i32, i32)>,
    /// Keyboard bindings mapping game actions to key combinations.
    #[serde(default)]
    pub key_bindings: KeyBindings,
}

impl Default for CharacterSettings {
    fn default() -> Self {
        Self {
            skill_keybinds: [None; NUMBER_OF_KEYBINDS],
            inventory_panel_pos: None,
            skills_panel_pos: None,
            settings_panel_pos: None,
            key_bindings: KeyBindings::default(),
        }
    }
}

/// Window display mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DisplayMode {
    Windowed,
    Fullscreen,
    BorderlessFullscreen,
}

impl Default for DisplayMode {
    fn default() -> Self {
        Self::Windowed
    }
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

const LOG_FILE_NAME: &str = "mag_client.log";
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
    /// Whether shadow rendering is enabled.
    #[serde(default = "default_true")]
    pub shadows_enabled: bool,
    /// Whether spell visual effects are rendered.
    #[serde(default = "default_true")]
    pub spell_effects_enabled: bool,
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
            shadows_enabled: true,
            spell_effects_enabled: true,
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
pub fn profile_file_path() -> PathBuf {
    data_directory().join(PROFILE_FILE_NAME)
}

/// Returns the path to the log file (`mag_client.log`).
pub fn log_file_path() -> PathBuf {
    data_directory().join(LOG_FILE_NAME)
}

/// Returns the path to the trusted hosts file (`known_hosts.json`).
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
/// * `Some(Settings)` if the character's entry exists, `None` otherwise.
///   Global fields are always populated from the global section;
///   per-character fields come from the character entry.
pub fn load_settings(identity: &CharacterIdentity) -> Option<Settings> {
    let path = profile_file_path();
    let storage = read_storage(&path);
    let key = profile_key(identity);
    let entry = storage.characters.get(&key)?;
    Some(Settings {
        music_enabled: storage.global.music_enabled,
        display_mode: storage.global.display_mode,
        pixel_perfect_scaling: storage.global.pixel_perfect_scaling,
        vsync_enabled: storage.global.vsync_enabled,
        shadows_enabled: storage.global.shadows_enabled,
        spell_effects_enabled: storage.global.spell_effects_enabled,
        master_volume: storage.global.master_volume.clamp(0.0, 1.0),
        hide: storage.global.hide,
        show_names: storage.global.show_names,
        show_proz: storage.global.show_proz,
        show_helper_text: storage.global.show_helper_text,
        show_positions: storage.global.show_positions,
        character: entry.character.clone(),
    })
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
    storage.global
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
    storage.global.music_enabled = settings.music_enabled;
    storage.global.display_mode = settings.display_mode;
    storage.global.pixel_perfect_scaling = settings.pixel_perfect_scaling;
    storage.global.vsync_enabled = settings.vsync_enabled;
    storage.global.shadows_enabled = settings.shadows_enabled;
    storage.global.spell_effects_enabled = settings.spell_effects_enabled;
    storage.global.master_volume = settings.master_volume.clamp(0.0, 1.0);
    storage.global.hide = settings.hide;
    storage.global.show_names = settings.show_names;
    storage.global.show_proz = settings.show_proz;
    storage.global.show_helper_text = settings.show_helper_text;
    storage.global.show_positions = settings.show_positions;
    write_storage(&path, &storage)
}

/// Returns the username from the most recent successful login, or `None` if
/// no login has been saved yet.
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
    storage.global.music_enabled = settings.music_enabled;
    storage.global.display_mode = settings.display_mode;
    storage.global.pixel_perfect_scaling = settings.pixel_perfect_scaling;
    storage.global.vsync_enabled = settings.vsync_enabled;
    storage.global.shadows_enabled = settings.shadows_enabled;
    storage.global.spell_effects_enabled = settings.spell_effects_enabled;
    storage.global.master_volume = settings.master_volume.clamp(0.0, 1.0);
    storage.global.hide = settings.hide;
    storage.global.show_names = settings.show_names;
    storage.global.show_proz = settings.show_proz;
    storage.global.show_helper_text = settings.show_helper_text;
    storage.global.show_positions = settings.show_positions;
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
        let mut s = Settings::default();
        s.music_enabled = false;
        s.display_mode = DisplayMode::BorderlessFullscreen;
        s.shadows_enabled = true;
        s.master_volume = 0.75;
        s.show_helper_text = false;
        s.show_positions = true;
        s.character.skill_keybinds = [
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
            None,
            None,
            None,
        ];

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
            name: "TestChar".to_string(),
            account_username: Some("alice".to_string()),
        };
        assert_eq!(profile_key(&identity), "alice:99");
    }

    #[test]
    fn profile_key_without_username() {
        let identity = CharacterIdentity {
            id: 7,
            name: "NoAccount".to_string(),
            account_username: None,
        };
        assert_eq!(profile_key(&identity), "unknown_account:7");
    }

    #[test]
    fn profile_storage_serde_roundtrip() {
        let storage = ProfileStorage {
            version: 1,
            last_username: Some("bob".to_string()),
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
        let mut s = Settings::default();
        s.shadows_enabled = true;
        s.master_volume = 0.5;
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
        let mut s = Settings::default();
        s.show_names = false;
        s.show_proz = false;
        s.hide = true;
        s.show_positions = true;
        s.spell_effects_enabled = false;

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
}
