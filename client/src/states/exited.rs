use bevy::ecs::system::ResMut;
use bevy::{ecs::system::Commands, prelude::Resource};
use bevy_egui::{
    egui::{self, Align2, Vec2},
    EguiContexts,
};

use crate::helpers::open_dir_in_file_manager;
use crate::network::NetworkRuntime;

#[derive(Resource, Default)]
pub struct ExitedUiState {
    request_exit: bool,
    last_error: Option<String>,
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
/// We hard-exit the process from `Update` after signaling network shutdown. This avoids rare
/// cases where app-level shutdown can stall while background tasks are unwinding.
pub fn apply_exit_request(mut ui_state: ResMut<ExitedUiState>, mut net: ResMut<NetworkRuntime>) {
    if ui_state.request_exit {
        log::info!("Exit requested from Exited UI state.");
        ui_state.request_exit = false;

        // Ensure the background network task is asked to terminate before Bevy begins
        // shutting down systems/task pools.
        net.shutdown();
        net.stop();
        log::info!("Forcing immediate process exit.");
        std::process::exit(0);
    }
}

/// Simple "Exited" screen shown after the server sends `SV_EXIT`.
///
/// Presents a single centered Exit button which closes the application.
pub fn run_exited(mut contexts: EguiContexts, mut ui_state: ResMut<ExitedUiState>) {
    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };

    // Dim the game behind the modal without affecting egui widgets.
    {
        let screen_rect = ctx.input(|i| i.viewport_rect());
        let painter = ctx.layer_painter(egui::LayerId::new(
            egui::Order::Background,
            egui::Id::new("exited_dim_background"),
        ));
        painter.rect_filled(screen_rect, 0.0, egui::Color32::from_black_alpha(200));
    }

    egui::Window::new("Disconnected from Server")
        .title_bar(true)
        .collapsible(false)
        .resizable(false)
        .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
        .fixed_size(Vec2::new(320.0, 190.0))
        .show(ctx, |ui| {
            ui.allocate_ui_with_layout(
                ui.available_size(),
                egui::Layout::centered_and_justified(egui::Direction::TopDown),
                |ui| {
                    if let Some(err) = ui_state.last_error.as_deref() {
                        ui.colored_label(egui::Color32::LIGHT_RED, err);
                        ui.add_space(8.0);
                    }

                    if ui
                        .add_sized([180.0, 36.0], egui::Button::new("Open logs folder"))
                        .clicked()
                    {
                        let log_dir = crate::resolve_log_dir();
                        match open_dir_in_file_manager(&log_dir) {
                            Ok(()) => ui_state.last_error = None,
                            Err(err) => ui_state.last_error = Some(err),
                        }
                    }

                    ui.add_space(10.0);

                    if ui
                        .add_sized([140.0, 44.0], egui::Button::new("Exit"))
                        .clicked()
                    {
                        ui_state.request_exit = true;
                        log::info!("Exit button clicked in Exited UI.");
                    }
                },
            );
        });
}
