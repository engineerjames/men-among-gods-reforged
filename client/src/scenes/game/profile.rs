use crate::{
    preferences::{self, CharacterIdentity, CharacterSettings, Settings},
    state::AppState,
    ui::widget::Widget,
    ui::widgets::title_bar::clamp_to_viewport,
};

use super::GameScene;

impl GameScene {
    /// Returns the default top-left position for the skills and settings panels.
    fn default_hud_panel_position() -> (i32, i32) {
        let x = super::HUD_ARC_CENTER_X - super::HUD_PANEL_W as i32 / 2;
        let panel_bottom = super::HUD_ARC_CENTER_Y
            - super::HUD_ARC_RADIUS as i32
            - super::HUD_BUTTON_RADIUS as i32
            - 20;
        let y = panel_bottom - super::HUD_PANEL_H as i32;
        (x, y)
    }

    /// Returns the default top-left position for the settings panel.
    fn default_settings_panel_position() -> (i32, i32) {
        let x = super::HUD_ARC_CENTER_X - super::HUD_PANEL_W as i32 / 2;
        let panel_bottom = super::HUD_ARC_CENTER_Y
            - super::HUD_ARC_RADIUS as i32
            - super::HUD_BUTTON_RADIUS as i32
            - 20;
        let y = panel_bottom - super::SETTINGS_PANEL_H as i32;
        (x, y)
    }

    /// Returns the default top-left position for the inventory panel.
    fn default_inventory_panel_position() -> (i32, i32) {
        let x = super::HUD_ARC_CENTER_X - super::INV_PANEL_W as i32 / 2;
        let panel_bottom = super::HUD_ARC_CENTER_Y
            - super::HUD_ARC_RADIUS as i32
            - super::HUD_BUTTON_RADIUS as i32
            - 20;
        let y = panel_bottom - super::INV_PANEL_H as i32;
        (x, y)
    }

    /// Restores all profile-scoped HUD panels to their default positions.
    fn reset_character_panel_positions(&mut self) {
        let (skills_x, skills_y) = Self::default_hud_panel_position();
        self.skills_panel.set_position(skills_x, skills_y);
        let (settings_x, settings_y) = Self::default_settings_panel_position();
        self.settings_panel.set_position(settings_x, settings_y);

        let (inventory_x, inventory_y) = Self::default_inventory_panel_position();
        self.inventory_panel.set_position(inventory_x, inventory_y);
    }

    /// Applies any saved per-character panel positions on top of the defaults.
    ///
    /// Missing saved positions intentionally leave the corresponding widget at
    /// its default location, which prevents another character's panel layout
    /// from leaking into the current session.
    fn apply_character_panel_positions(&mut self, settings: &CharacterSettings) {
        self.reset_character_panel_positions();

        if let Some((x, y)) = settings.inventory_panel_pos {
            let b = self.inventory_panel.bounds();
            let (cx, cy) = clamp_to_viewport(x, y, b.width, b.height);
            self.inventory_panel.set_position(cx, cy);
        }
        if let Some((x, y)) = settings.skills_panel_pos {
            let b = self.skills_panel.bounds();
            let (cx, cy) = clamp_to_viewport(x, y, b.width, b.height);
            self.skills_panel.set_position(cx, cy);
        }
        if let Some((x, y)) = settings.settings_panel_pos {
            let b = self.settings_panel.bounds();
            let (cx, cy) = clamp_to_viewport(x, y, b.width, b.height);
            self.settings_panel.set_position(cx, cy);
        }
    }

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
        // Always start from a clean per-character state so unconfigured
        // characters do not inherit another character's bindings or HUD layout.
        app_state.settings = preferences::load_settings(identity);
        self.apply_character_panel_positions(&app_state.settings.character);

        log::info!(
            "Applied SDL profile state for character '{}' (id={})",
            identity.name,
            identity.id
        );
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::preferences::Settings;

    #[test]
    fn apply_character_panel_positions_resets_missing_saved_positions_to_defaults() {
        let mut scene = GameScene::new();
        scene.inventory_panel.set_position(900, 800);
        scene.skills_panel.set_position(700, 600);
        scene.settings_panel.set_position(500, 400);

        let mut settings = CharacterSettings::default();
        settings.inventory_panel_pos = Some((12, 34));

        scene.apply_character_panel_positions(&settings);

        assert_eq!(
            (
                scene.inventory_panel.bounds().x,
                scene.inventory_panel.bounds().y
            ),
            (12, 34)
        );
        assert_eq!(
            (scene.skills_panel.bounds().x, scene.skills_panel.bounds().y),
            GameScene::default_hud_panel_position()
        );
        assert_eq!(
            (
                scene.settings_panel.bounds().x,
                scene.settings_panel.bounds().y
            ),
            GameScene::default_settings_panel_position()
        );
    }

    #[test]
    fn default_panel_positions_match_new_scene_layout() {
        let scene = GameScene::new();

        assert_eq!(
            (scene.skills_panel.bounds().x, scene.skills_panel.bounds().y),
            GameScene::default_hud_panel_position()
        );
        assert_eq!(
            (
                scene.settings_panel.bounds().x,
                scene.settings_panel.bounds().y
            ),
            GameScene::default_settings_panel_position()
        );
        assert_eq!(
            (
                scene.inventory_panel.bounds().x,
                scene.inventory_panel.bounds().y
            ),
            GameScene::default_inventory_panel_position()
        );
    }

    #[test]
    fn default_character_settings_start_with_empty_skill_bindings() {
        let settings = Settings::default();
        assert!(
            settings
                .character
                .skill_keybinds
                .iter()
                .all(|slot| slot.is_none())
        );
    }
}
