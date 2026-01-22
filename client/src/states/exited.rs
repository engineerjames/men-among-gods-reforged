use bevy::ecs::message::MessageWriter;
use bevy::ecs::system::ResMut;
use bevy::{app::AppExit, ecs::system::Commands, prelude::Resource};
use bevy_egui::{
    egui::{self, Align2, Vec2},
    EguiContexts,
};

use crate::network::NetworkRuntime;

#[derive(Resource, Default)]
pub struct ExitedUiState {
    request_exit: bool,
}

/// Initializes resources for the in-game exited state.
pub fn setup_exited(mut commands: Commands, mut net: ResMut<NetworkRuntime>) {
    commands.insert_resource(ExitedUiState::default());

    // We are no longer in an active session once we enter Exited.
    // Request the network task to shut down so it can't block app exit.
    net.shutdown();
    net.stop();
}

/// Cleans up resources created by `setup_exited`.
pub fn teardown_exited() {
    // Currently no persistent entities to clean up.
}

/// Applies any "Exit" requests made by the UI.
///
/// We emit `AppExit` from the `Update` schedule (not the egui pass) because the default Bevy
/// runner checks for `AppExit` after running `Update`.
pub fn apply_exit_request(
    mut ui_state: ResMut<ExitedUiState>,
    mut exit_events: MessageWriter<AppExit>,
    mut net: ResMut<NetworkRuntime>,
) {
    if ui_state.request_exit {
        ui_state.request_exit = false;

        // Ensure the background network task is asked to terminate before Bevy begins
        // shutting down systems/task pools.
        net.shutdown();
        net.stop();
        exit_events.write(AppExit::Success);
    }
}

/// Simple "Exited" screen shown after the server sends `SV_EXIT`.
///
/// Presents a single centered Exit button which closes the application.
pub fn run_exited(mut contexts: EguiContexts, mut ui_state: ResMut<ExitedUiState>) {
    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };

    egui::Window::new("Exited")
        .title_bar(false)
        .collapsible(false)
        .resizable(false)
        .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
        .fixed_size(Vec2::new(280.0, 140.0))
        .show(ctx, |ui| {
            ui.allocate_ui_with_layout(
                ui.available_size(),
                egui::Layout::centered_and_justified(egui::Direction::TopDown),
                |ui| {
                    if ui
                        .add_sized([140.0, 44.0], egui::Button::new("Exit"))
                        .clicked()
                    {
                        ui_state.request_exit = true;
                    }
                },
            );
        });
}
