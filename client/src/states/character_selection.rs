use bevy::prelude::*;
use bevy_egui::{
    egui::{self, Pos2},
    EguiContexts,
};

use crate::constants::{TARGET_HEIGHT, TARGET_WIDTH};
use crate::network::account_api::ApiSession;
use crate::GameState;

#[derive(Resource, Debug, Default)]
pub struct CharacterSelectionUiState {
    last_error: Option<String>,
}

pub fn setup_character_selection(mut commands: Commands) {
    commands.insert_resource(CharacterSelectionUiState::default());
}

pub fn teardown_character_selection() {
    log::debug!("teardown_character_selection - end");
}

pub fn run_character_selection(
    mut contexts: EguiContexts,
    mut ui_state: ResMut<CharacterSelectionUiState>,
    mut api_session: ResMut<ApiSession>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };

    egui::Window::new("Character Selection")
        .default_height(TARGET_HEIGHT)
        .default_width(TARGET_WIDTH)
        .fixed_pos(Pos2::new(0.0, 0.0))
        .collapsible(false)
        .resizable(false)
        .show(ctx, |ui| {
            ui.heading("Character selection (placeholder)");

            if let Some(username) = api_session.username.as_deref() {
                ui.label(format!("Logged in as: {username}"));
            } else {
                ui.label("No account session available");
            }

            if let Some(err) = ui_state.last_error.as_deref() {
                ui.colored_label(egui::Color32::LIGHT_RED, err);
            }

            ui.add_space(12.0);

            if ui
                .add(egui::Button::new("Continue to game login").min_size([200.0, 32.0].into()))
                .clicked()
            {
                next_state.set(GameState::LoggingIn);
            }

            if ui
                .add(egui::Button::new("Log out").min_size([120.0, 32.0].into()))
                .clicked()
            {
                api_session.token = None;
                api_session.username = None;
                ui_state.last_error = None;
                next_state.set(GameState::AccountLogin);
            }
        });
}
