use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::types::skill_buttons::SkillButtons;

const LOG_FILE_NAME: &str = "sdl_client.log";
const PROFILE_FILE_NAME: &str = "sdl_client_profile.json";

#[derive(Clone, Debug)]
pub struct CharacterIdentity {
    pub id: u64,
    pub name: String,
    pub account_username: Option<String>,
}

#[derive(Clone, Debug)]
pub struct RuntimeProfile {
    pub skill_buttons: [SkillButtons; 12],
    pub shadows_enabled: bool,
    pub spell_effects_enabled: bool,
    pub master_volume: f32,
    pub hide: i32,
    pub show_names: i32,
    pub show_proz: i32,
}

#[derive(Clone, Debug)]
pub struct GlobalSettings {
    pub music_enabled: bool,
}

impl Default for GlobalSettings {
    fn default() -> Self {
        Self {
            music_enabled: true,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct SpellButtonEntry {
    name: String,
    skill_nr: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct CharacterProfile {
    character_id: u64,
    character_name: String,
    account_username: Option<String>,
    skill_buttons: Vec<SpellButtonEntry>,
    shadows_enabled: bool,
    spell_effects_enabled: bool,
    master_volume: f32,
    hide: i32,
    show_names: i32,
    show_proz: i32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct AppProfileStorage {
    version: u32,
    global: GlobalSettingsStorage,
    characters: BTreeMap<String, CharacterProfile>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct GlobalSettingsStorage {
    music_enabled: bool,
}

impl Default for GlobalSettingsStorage {
    fn default() -> Self {
        Self {
            music_enabled: true,
        }
    }
}

impl Default for AppProfileStorage {
    fn default() -> Self {
        Self {
            version: 1,
            global: GlobalSettingsStorage::default(),
            characters: BTreeMap::new(),
        }
    }
}

fn to_global_settings(storage: &GlobalSettingsStorage) -> GlobalSettings {
    GlobalSettings {
        music_enabled: storage.music_enabled,
    }
}

fn from_global_settings(settings: &GlobalSettings) -> GlobalSettingsStorage {
    GlobalSettingsStorage {
        music_enabled: settings.music_enabled,
    }
}

fn profile_key(identity: &CharacterIdentity) -> String {
    let username = identity
        .account_username
        .as_deref()
        .unwrap_or("unknown_account");
    format!("{username}:{}", identity.id)
}

fn working_directory() -> PathBuf {
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

pub fn profile_file_path() -> PathBuf {
    working_directory().join(PROFILE_FILE_NAME)
}

pub fn log_file_path() -> PathBuf {
    working_directory().join(LOG_FILE_NAME)
}

fn read_storage(path: &Path) -> AppProfileStorage {
    let Ok(raw) = fs::read_to_string(path) else {
        return AppProfileStorage::default();
    };

    match serde_json::from_str::<AppProfileStorage>(&raw) {
        Ok(storage) => storage,
        Err(err) => {
            log::warn!(
                "Failed to parse persisted SDL client profile at {}: {}",
                path.display(),
                err
            );
            AppProfileStorage::default()
        }
    }
}

fn write_storage(path: &Path, storage: &AppProfileStorage) -> Result<(), String> {
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

fn to_runtime_profile(profile: &CharacterProfile) -> RuntimeProfile {
    let mut buttons = [SkillButtons::default(); 12];

    for (idx, button) in profile.skill_buttons.iter().take(12).enumerate() {
        if button.name.is_empty()
            || button.name == "-"
            || button.skill_nr == SkillButtons::UNASSIGNED_SKILL_NR
        {
            buttons[idx].set_unassigned();
        } else {
            buttons[idx].set_name(&button.name);
            buttons[idx].set_skill_nr(button.skill_nr);
        }
    }

    RuntimeProfile {
        skill_buttons: buttons,
        shadows_enabled: profile.shadows_enabled,
        spell_effects_enabled: profile.spell_effects_enabled,
        master_volume: profile.master_volume.clamp(0.0, 1.0),
        hide: profile.hide,
        show_names: profile.show_names,
        show_proz: profile.show_proz,
    }
}

fn from_runtime_profile(
    identity: &CharacterIdentity,
    runtime: &RuntimeProfile,
) -> CharacterProfile {
    let skill_buttons = runtime
        .skill_buttons
        .iter()
        .map(|button| {
            let name = button.name_str();
            let skill_nr = if button.is_unassigned() {
                SkillButtons::UNASSIGNED_SKILL_NR
            } else {
                button.skill_nr()
            };

            SpellButtonEntry { name, skill_nr }
        })
        .collect::<Vec<_>>();

    CharacterProfile {
        character_id: identity.id,
        character_name: identity.name.clone(),
        account_username: identity.account_username.clone(),
        skill_buttons,
        shadows_enabled: runtime.shadows_enabled,
        spell_effects_enabled: runtime.spell_effects_enabled,
        master_volume: runtime.master_volume.clamp(0.0, 1.0),
        hide: runtime.hide,
        show_names: runtime.show_names,
        show_proz: runtime.show_proz,
    }
}

pub fn load_profile(identity: &CharacterIdentity) -> Option<RuntimeProfile> {
    let path = profile_file_path();
    let storage = read_storage(&path);
    let key = profile_key(identity);
    storage.characters.get(&key).map(to_runtime_profile)
}

pub fn load_global_settings() -> GlobalSettings {
    let path = profile_file_path();
    let storage = read_storage(&path);
    to_global_settings(&storage.global)
}

pub fn save_global_settings(settings: &GlobalSettings) -> Result<(), String> {
    let path = profile_file_path();
    let mut storage = read_storage(&path);
    storage.global = from_global_settings(settings);
    write_storage(&path, &storage)
}

pub fn save_profile(identity: &CharacterIdentity, runtime: &RuntimeProfile) -> Result<(), String> {
    let path = profile_file_path();
    let mut storage = read_storage(&path);
    let key = profile_key(identity);
    storage
        .characters
        .insert(key, from_runtime_profile(identity, runtime));
    write_storage(&path, &storage)
}
