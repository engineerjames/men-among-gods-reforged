use bevy::prelude::*;
use bevy_egui::{
    egui::{self, Pos2},
    EguiContexts,
};

use crate::constants::{TARGET_HEIGHT, TARGET_WIDTH};
use crate::player_state::PlayerState;
use crate::states::gameplay::CursorActionTextSettings;
use crate::systems::sound::SoundSettings;
use crate::GameState;

#[derive(Resource, Debug, Default)]
pub struct MenuUiState {
    last_error: Option<String>,
}

/// Initializes resources for the in-game menu state.
pub fn setup_menu(mut commands: Commands) {
    commands.insert_resource(MenuUiState::default());
}

/// Cleans up resources created by `setup_menu`.
pub fn teardown_menu() {
    // Currently no persistent entities to clean up.
}

/// Runs the in-game menu UI (egui).
///
/// Provides:
/// - Return to game
/// - Shadows toggle
/// - Sound enable toggle
/// - Master volume slider
pub fn run_menu(
    mut contexts: EguiContexts,
    keys: Res<ButtonInput<KeyCode>>,
    mut next_state: ResMut<NextState<GameState>>,
    menu_ui: ResMut<MenuUiState>,
    mut player_state: ResMut<PlayerState>,
    mut sound_settings: ResMut<SoundSettings>,
    mut cursor_action_text: ResMut<CursorActionTextSettings>,
) {
    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };

    // Quick keyboard escape: close menu.
    if keys.just_pressed(KeyCode::Escape) {
        next_state.set(GameState::Gameplay);
        return;
    }

    egui::Window::new("Menu")
        .default_height(TARGET_HEIGHT)
        .default_width(TARGET_WIDTH)
        .fixed_pos(Pos2::new(0.0, 0.0))
        .collapsible(false)
        .resizable(false)
        .show(ctx, |ui| {
            ui.heading("Men Among Gods Reforged");
            ui.label("Paused");

            if let Some(err) = menu_ui.last_error.as_deref() {
                ui.colored_label(egui::Color32::LIGHT_RED, err);
            }

            ui.separator();

            if ui
                .add_sized([180., 40.], egui::Button::new("Return to game"))
                .clicked()
            {
                next_state.set(GameState::Gameplay);
            }

            ui.add_space(10.0);

            let mut shadows = player_state.player_data().are_shadows_enabled != 0;
            if ui.checkbox(&mut shadows, "Render shadows").changed() {
                player_state.player_data_mut().are_shadows_enabled = if shadows { 1 } else { 0 };
            }

            if ui
                .checkbox(&mut cursor_action_text.enabled, "Show cursor action text")
                .changed()
            {
                // no-op; applied by the cursor UI system
            }

            if ui
                .checkbox(&mut sound_settings.enabled, "Play sounds")
                .changed()
            {
                // no-op; behavior is applied in the sound system
            }

            ui.horizontal(|ui| {
                ui.label("Volume");
                let slider = egui::Slider::new(&mut sound_settings.master_volume, 0.0..=1.0)
                    .show_value(false);
                ui.add_sized([220.0, 18.0], slider);
                ui.label(format!("{:3.0}%", sound_settings.master_volume * 100.0));
            });
        });
}
