use bevy::ecs::message::MessageWriter;
use bevy::{app::AppExit, ecs::system::Commands, prelude::Resource};
use bevy_egui::{
    egui::{self, Align2, Vec2},
    EguiContexts,
};

#[derive(Resource, Default)]
pub struct ExitedUiState;

/// Initializes resources for the in-game exited state.
pub fn setup_exited(mut commands: Commands) {
    commands.insert_resource(ExitedUiState::default());
}

/// Cleans up resources created by `setup_exited`.
pub fn teardown_exited() {
    // Currently no persistent entities to clean up.
}

/// Simple "Exited" screen shown after the server sends `SV_EXIT`.
///
/// Presents a single centered Exit button which closes the application.
pub fn run_exited(mut contexts: EguiContexts, mut exit_events: MessageWriter<AppExit>) {
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
            ui.vertical_centered(|ui| {
                ui.add_space(24.0);
                if ui
                    .add_sized([140.0, 44.0], egui::Button::new("Exit"))
                    .clicked()
                {
                    exit_events.write(AppExit::Success);
                }
            });
        });
}
