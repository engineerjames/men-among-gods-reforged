use std::fs;
use std::path::{Path, PathBuf};

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::helpers::get_mag_base_dir;
use crate::player_state::PlayerState;
use crate::states::gameplay::CursorActionTextSettings;
use crate::systems::magic_postprocess::MagicPostProcessSettings;
use crate::systems::sound::SoundSettings;
use crate::types::{player_data::PlayerData, save_file::SaveFile};

pub const DEFAULT_SERVER_IP: &str = "menamonggods.ddns.net";
pub const DEFAULT_SERVER_PORT: u16 = 5555;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UserSettings {
    pub render_shadows: bool,
    pub play_sounds: bool,
    pub master_volume: f32,
    pub show_cursor_action_text: bool,
    pub magic_effects_enabled: bool,
    pub gamma: f32,

    /// Default server address shown on the login screen.
    pub default_server_ip: String,
    /// Default server port shown on the login screen.
    pub default_server_port: u16,

    /// Persisted character/key data (replaces legacy mag.dat).
    pub save_file: SaveFile,
    pub player_data: PlayerData,
}

impl Default for UserSettings {
    fn default() -> Self {
        Self {
            render_shadows: true,
            play_sounds: true,
            master_volume: 1.0,
            show_cursor_action_text: true,
            magic_effects_enabled: true,
            gamma: 1.0,

            default_server_ip: DEFAULT_SERVER_IP.to_string(),
            default_server_port: DEFAULT_SERVER_PORT,

            save_file: SaveFile::default(),
            player_data: PlayerData::default(),
        }
    }
}

#[derive(Resource, Debug)]
pub struct UserSettingsState {
    pub path: PathBuf,
    pub settings: UserSettings,
    pending_save: bool,
    save_debounce: Timer,
}

impl UserSettingsState {
    fn new(path: PathBuf, settings: UserSettings) -> Self {
        Self {
            path,
            settings,
            pending_save: false,
            save_debounce: Timer::from_seconds(0.5, TimerMode::Once),
        }
    }

    pub fn sync_from_resources(
        &mut self,
        player_state: &PlayerState,
        sound_settings: &SoundSettings,
        cursor_action_text: &CursorActionTextSettings,
        magic_settings: &MagicPostProcessSettings,
    ) {
        self.settings.render_shadows = player_state.player_data().are_shadows_enabled != 0;
        self.settings.play_sounds = sound_settings.enabled;
        self.settings.master_volume = sound_settings.master_volume.clamp(0.0, 1.0);
        self.settings.show_cursor_action_text = cursor_action_text.enabled;
        self.settings.magic_effects_enabled = magic_settings.enabled;
        self.settings.gamma = magic_settings.gamma.clamp(0.1, 5.0);
    }

    pub fn apply_to_resources(
        &self,
        player_state: &mut PlayerState,
        sound_settings: &mut SoundSettings,
        cursor_action_text: &mut CursorActionTextSettings,
        magic_settings: &mut MagicPostProcessSettings,
    ) {
        player_state.player_data_mut().are_shadows_enabled =
            if self.settings.render_shadows { 1 } else { 0 };
        sound_settings.enabled = self.settings.play_sounds;
        sound_settings.master_volume = self.settings.master_volume.clamp(0.0, 1.0);
        cursor_action_text.enabled = self.settings.show_cursor_action_text;
        magic_settings.enabled = self.settings.magic_effects_enabled;
        magic_settings.gamma = self.settings.gamma.clamp(0.1, 5.0);
    }

    pub fn request_save(&mut self) {
        self.pending_save = true;
        self.save_debounce.reset();
    }

    pub fn sync_character_from_player_state(&mut self, player_state: &PlayerState) {
        self.settings.save_file = *player_state.save_file();
        self.settings.player_data = *player_state.player_data();
    }

    fn try_save_now(&mut self) {
        let Some(parent) = self.path.parent() else {
            return;
        };

        if let Err(e) = fs::create_dir_all(parent) {
            log::error!("Failed to create settings dir {:?}: {e}", parent);
            return;
        }

        let json = match serde_json::to_string_pretty(&self.settings) {
            Ok(s) => s,
            Err(e) => {
                log::error!("Failed to serialize settings to JSON: {e}");
                return;
            }
        };

        if let Err(e) = fs::write(&self.path, format!("{json}\n")) {
            log::error!("Failed to write settings file {:?}: {e}", self.path);
            return;
        }

        self.pending_save = false;
    }
}

fn default_settings_path() -> PathBuf {
    if let Some(base) = get_mag_base_dir() {
        let path = base.join("settings.json");
        log::info!("Using settings path: {}", path.display());
        return path;
    }

    let fallback = PathBuf::from("settings.json");
    log::info!("Using fallback settings path: {}", fallback.display());
    fallback
}

fn load_settings_from_disk(path: &Path) -> UserSettings {
    let bytes = match fs::read(path) {
        Ok(b) => b,
        Err(_) => return UserSettings::default(),
    };

    match serde_json::from_slice::<UserSettings>(&bytes) {
        Ok(s) => s,
        Err(e) => {
            log::error!("Failed to parse settings file {:?}: {e}", path);
            UserSettings::default()
        }
    }
}

/// Loads `settings.json` (or defaults) and applies it to runtime resources.
///
/// This intentionally stays separate from `mag.dat` / character `.mag` formats.
pub fn load_user_settings_startup(
    mut commands: Commands,
    mut player_state: ResMut<PlayerState>,
    mut sound_settings: ResMut<SoundSettings>,
    mut cursor_action_text: ResMut<CursorActionTextSettings>,
    mut magic_settings: ResMut<MagicPostProcessSettings>,
) {
    let path = default_settings_path();
    let settings = load_settings_from_disk(&path);

    // Restore persisted character/key data (replaces mag.dat).
    player_state.set_character_from_file(settings.save_file, settings.player_data);

    let state = UserSettingsState::new(path, settings);
    state.apply_to_resources(
        &mut player_state,
        &mut sound_settings,
        &mut cursor_action_text,
        &mut magic_settings,
    );

    commands.insert_resource(state);
}

/// Saves the user settings JSON when changes are requested (debounced).
pub fn save_user_settings_if_pending(
    time: Res<Time>,
    mut user_settings: ResMut<UserSettingsState>,
) {
    if !user_settings.pending_save {
        return;
    }

    user_settings.save_debounce.tick(time.delta());
    if user_settings.save_debounce.just_finished() || user_settings.save_debounce.is_finished() {
        user_settings.try_save_now();
    }
}
