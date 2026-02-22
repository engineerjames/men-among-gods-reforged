use crate::{
    preferences::{self, CharacterIdentity, RuntimeProfile},
    state::AppState,
};

use super::GameScene;

impl GameScene {
    /// Loads a persisted profile for the given character identity and applies
    /// its settings to the game state (skill buttons, toggles, volume).
    ///
    /// # Arguments
    /// * `app_state` – mutable application state whose `player_state` fields are updated.
    /// * `identity` – identifies which character's profile to load from disk.
    pub(super) fn apply_loaded_profile(
        &mut self,
        app_state: &mut AppState,
        identity: &CharacterIdentity,
    ) {
        if let Some(profile) = preferences::load_profile(identity) {
            if let Some(ps) = app_state.player_state.as_mut() {
                ps.player_data_mut().skill_buttons = profile.skill_buttons;
                ps.player_data_mut().are_shadows_enabled =
                    if profile.shadows_enabled { 1 } else { 0 };
                ps.player_data_mut().hide = profile.hide;
                ps.player_data_mut().show_names = profile.show_names;
                ps.player_data_mut().show_proz = profile.show_proz;
            }
            self.are_spell_effects_enabled = profile.spell_effects_enabled;
            self.master_volume = profile.master_volume;
            app_state.master_volume = profile.master_volume;
            log::info!(
                "Loaded persisted SDL profile for character '{}' (id={})",
                identity.name,
                identity.id
            );
        }
    }

    /// Builds a `RuntimeProfile` snapshot from current in-game settings.
    ///
    /// # Returns
    /// `Some(RuntimeProfile)` if player state is available, `None` otherwise.
    pub(super) fn build_runtime_profile(&self, app_state: &AppState) -> Option<RuntimeProfile> {
        let ps = app_state.player_state.as_ref()?;
        let pdata = ps.player_data();

        Some(RuntimeProfile {
            skill_buttons: pdata.skill_buttons,
            shadows_enabled: pdata.are_shadows_enabled != 0,
            spell_effects_enabled: self.are_spell_effects_enabled,
            master_volume: self.master_volume,
            hide: pdata.hide,
            show_names: pdata.show_names,
            show_proz: pdata.show_proz,
        })
    }

    /// Saves the current profile to disk for the active character.
    pub(super) fn save_active_profile(&self, app_state: &AppState) {
        let Some(identity) = self.active_profile_character.as_ref() else {
            return;
        };
        let Some(runtime) = self.build_runtime_profile(app_state) else {
            return;
        };

        if let Err(err) = preferences::save_profile(identity, &runtime) {
            log::warn!(
                "Failed to persist SDL profile for '{}': {}",
                identity.name,
                err
            );
        }
    }
}
