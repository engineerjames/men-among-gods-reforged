use crate::{
    preferences::{self, CharacterIdentity, Settings},
    state::AppState,
    ui::widget::Widget,
    ui::widgets::title_bar::clamp_to_viewport,
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
        app_state: &mut AppState<'_>,
        identity: &CharacterIdentity,
    ) {
        if let Some(settings) = preferences::load_settings(identity) {
            // Overwrite the live settings with everything from the persisted
            // profile.  Global fields (display_mode, vsync, etc.) are already
            // present in the loaded Settings because `load_settings` merges
            // global + per-character data.
            app_state.settings = settings;

            // Restore saved panel positions.
            if let Some((x, y)) = app_state.settings.character.inventory_panel_pos {
                let b = self.inventory_panel.bounds();
                let (cx, cy) = clamp_to_viewport(x, y, b.width, b.height);
                self.inventory_panel.set_position(cx, cy);
            }
            if let Some((x, y)) = app_state.settings.character.skills_panel_pos {
                let b = self.skills_panel.bounds();
                let (cx, cy) = clamp_to_viewport(x, y, b.width, b.height);
                self.skills_panel.set_position(cx, cy);
            }
            if let Some((x, y)) = app_state.settings.character.settings_panel_pos {
                let b = self.settings_panel.bounds();
                let (cx, cy) = clamp_to_viewport(x, y, b.width, b.height);
                self.settings_panel.set_position(cx, cy);
            }

            log::info!(
                "Loaded persisted SDL profile for character '{}' (id={})",
                identity.name,
                identity.id
            );
        }
    }

    /// Builds a [`Settings`] snapshot from current in-game settings.
    ///
    /// Clones the live `app_state.settings` and patches in the current panel
    /// positions and GameScene-owned fields before returning.
    ///
    /// # Returns
    /// `Some(Settings)` if player state is available, `None` otherwise.
    pub(super) fn build_settings_snapshot(&self, app_state: &AppState) -> Option<Settings> {
        // We require player state to exist (i.e. we're in-game) before saving.
        let _ps = app_state.player_state.as_ref()?;

        let mut snapshot = app_state.settings.clone();
        snapshot.character.inventory_panel_pos = Some((
            self.inventory_panel.bounds().x,
            self.inventory_panel.bounds().y,
        ));
        snapshot.character.skills_panel_pos =
            Some((self.skills_panel.bounds().x, self.skills_panel.bounds().y));
        snapshot.character.settings_panel_pos = Some((
            self.settings_panel.bounds().x,
            self.settings_panel.bounds().y,
        ));

        Some(snapshot)
    }

    /// Saves the current settings to disk for the active character.
    pub(super) fn save_active_profile(&self, app_state: &AppState) {
        let Some(identity) = self.active_profile_character.as_ref() else {
            return;
        };
        let Some(settings) = self.build_settings_snapshot(app_state) else {
            return;
        };

        if let Err(err) = preferences::save_settings(identity, &settings) {
            log::warn!(
                "Failed to persist SDL profile for '{}': {}",
                identity.name,
                err
            );
        }
    }
}
