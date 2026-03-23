use crate::{
    preferences::{self, CharacterIdentity, CharacterSettings, Settings},
    state::AppState,
    ui::title_bar::clamp_to_viewport,
    ui::widget::Widget,
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
            if let Some(ps) = app_state.player_state.as_mut() {
                ps.player_data_mut().skill_keybinds = settings.character.skill_keybinds;
                ps.player_data_mut().are_shadows_enabled = settings.shadows_enabled;
                ps.player_data_mut().hide = settings.hide;
                ps.player_data_mut().show_names = settings.show_names;
                ps.player_data_mut().show_proz = settings.show_proz;
                ps.player_data_mut().show_helper_text = settings.show_helper_text;
            }
            self.are_spell_effects_enabled = settings.spell_effects_enabled;
            self.master_volume = settings.master_volume;
            app_state.master_volume = settings.master_volume;
            self.key_bindings = settings.character.key_bindings.clone();

            // Restore saved panel positions.
            if let Some((x, y)) = settings.character.inventory_panel_pos {
                let b = self.inventory_panel.bounds();
                let (cx, cy) = clamp_to_viewport(x, y, b.width, b.height);
                self.inventory_panel.set_position(cx, cy);
            }
            if let Some((x, y)) = settings.character.skills_panel_pos {
                let b = self.skills_panel.bounds();
                let (cx, cy) = clamp_to_viewport(x, y, b.width, b.height);
                self.skills_panel.set_position(cx, cy);
            }
            if let Some((x, y)) = settings.character.settings_panel_pos {
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
    /// # Returns
    /// `Some(Settings)` if player state is available, `None` otherwise.
    pub(super) fn build_settings_snapshot(&self, app_state: &AppState) -> Option<Settings> {
        let ps = app_state.player_state.as_ref()?;
        let pdata = ps.player_data();

        Some(Settings {
            music_enabled: app_state.music_enabled,
            display_mode: app_state.display_mode,
            pixel_perfect_scaling: app_state.pixel_perfect_scaling,
            vsync_enabled: app_state.vsync_enabled,
            shadows_enabled: pdata.are_shadows_enabled,
            spell_effects_enabled: self.are_spell_effects_enabled,
            master_volume: self.master_volume,
            hide: pdata.hide,
            show_names: pdata.show_names,
            show_proz: pdata.show_proz,
            show_helper_text: pdata.show_helper_text,
            character: CharacterSettings {
                skill_keybinds: pdata.skill_keybinds,
                inventory_panel_pos: Some((
                    self.inventory_panel.bounds().x,
                    self.inventory_panel.bounds().y,
                )),
                skills_panel_pos: Some((
                    self.skills_panel.bounds().x,
                    self.skills_panel.bounds().y,
                )),
                settings_panel_pos: Some((
                    self.settings_panel.bounds().x,
                    self.settings_panel.bounds().y,
                )),
                key_bindings: self.key_bindings.clone(),
            },
        })
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
