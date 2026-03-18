use std::{
    collections::BTreeMap,
    fmt, fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::ui::widget::KeyBindings;

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
/// Global fields: `music_enabled`, `display_mode`, `pixel_perfect_scaling`,
/// `vsync_enabled`.
///
/// Per-character fields: everything else.
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
    #[serde(default)]
    pub vsync_enabled: bool,
    /// Whether shadow rendering is enabled.
    #[serde(default)]
    pub shadows_enabled: bool,
    /// Whether spell visual effects are rendered.
    #[serde(default)]
    pub spell_effects_enabled: bool,
    /// Master volume (0.0–1.0).
    #[serde(default)]
    pub master_volume: f32,
    /// Wall-hiding toggle.
    #[serde(default)]
    pub hide: i32,
    /// Overhead player name display toggle.
    #[serde(default)]
    pub show_names: i32,
    /// Overhead health percentage display toggle.
    #[serde(default)]
    pub show_proz: i32,
    /// Whether context-sensitive helper text is shown near the cursor.
    #[serde(default = "default_true")]
    pub show_helper_text: bool,
    /// Custom CTRL+1-9 skill keybinds. Index 0 = key "1", index 8 = key "9".
    #[serde(default)]
    pub skill_keybinds: [Option<u32>; 9],
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

impl Default for Settings {
    fn default() -> Self {
        Self {
            music_enabled: true,
            display_mode: DisplayMode::default(),
            pixel_perfect_scaling: false,
            vsync_enabled: false,
            shadows_enabled: false,
            spell_effects_enabled: false,
            master_volume: 0.0,
            hide: 0,
            show_names: 0,
            show_proz: 0,
            show_helper_text: true,
            skill_keybinds: [None; 9],
            inventory_panel_pos: None,
            skills_panel_pos: None,
            settings_panel_pos: None,
            key_bindings: KeyBindings::default(),
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
    settings: Settings,
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
    let mut settings = entry.settings.clone();
    // Global fields always come from the global section.
    settings.music_enabled = storage.global.music_enabled;
    settings.display_mode = storage.global.display_mode;
    settings.pixel_perfect_scaling = storage.global.pixel_perfect_scaling;
    settings.vsync_enabled = storage.global.vsync_enabled;
    settings.master_volume = settings.master_volume.clamp(0.0, 1.0);
    Some(settings)
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
/// Only `music_enabled`, `display_mode`, `pixel_perfect_scaling`, and
/// `vsync_enabled` are written. All other fields and the `last_username`
/// value are preserved as-is.
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
    // Insert / update character entry.
    let key = profile_key(identity);
    storage.characters.insert(
        key,
        CharacterEntry {
            character_id: identity.id,
            character_name: identity.name.clone(),
            account_username: identity.account_username.clone(),
            settings: Settings {
                master_volume: settings.master_volume.clamp(0.0, 1.0),
                ..settings.clone()
            },
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
        s.skill_keybinds = [None, Some(42), None, None, Some(7), None, None, None, None];
        s.show_helper_text = false;

        let json = serde_json::to_string(&s).unwrap();
        let deserialized: Settings = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.music_enabled, s.music_enabled);
        assert_eq!(deserialized.display_mode, s.display_mode);
        assert_eq!(deserialized.shadows_enabled, s.shadows_enabled);
        assert!((deserialized.master_volume - s.master_volume).abs() < f32::EPSILON);
        assert_eq!(deserialized.skill_keybinds, s.skill_keybinds);
        assert_eq!(deserialized.show_helper_text, s.show_helper_text);
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
        assert_eq!(deserialized.skill_keybinds, defaults.skill_keybinds);
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
}
