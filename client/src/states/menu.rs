use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy_egui::{
    egui::{self, Pos2},
    EguiContexts,
};

use crate::constants::{TARGET_HEIGHT, TARGET_WIDTH};
use crate::helpers::open_dir_in_file_manager;
use crate::player_state::PlayerState;
use crate::settings::{UserSettingsState, VideoModeSetting};
use crate::states::gameplay::CursorActionTextSettings;
use crate::systems::magic_postprocess::MagicPostProcessSettings;
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
    mut menu_ui: ResMut<MenuUiState>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
    mut player_state: ResMut<PlayerState>,
    mut sound_settings: ResMut<SoundSettings>,
    mut cursor_action_text: ResMut<CursorActionTextSettings>,
    mut user_settings: ResMut<UserSettingsState>,
    mut magic_settings: ResMut<MagicPostProcessSettings>,
) {
    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };

    // Quick keyboard escape: close menu.
    if keys.just_pressed(KeyCode::Escape) {
        next_state.set(GameState::Gameplay);
        return;
    }

    // Dim the game behind the menu UI without affecting egui widgets.
    {
        let screen_rect = ctx.input(|i| i.viewport_rect());
        let painter = ctx.layer_painter(egui::LayerId::new(
            egui::Order::Background,
            egui::Id::new("menu_dim_background"),
        ));
        painter.rect_filled(screen_rect, 0.0, egui::Color32::from_black_alpha(200));
    }

    egui::Window::new("Menu")
        .default_height(TARGET_HEIGHT)
        .default_width(TARGET_WIDTH)
        .fixed_pos(Pos2::new(0.0, 0.0))
        .collapsible(false)
        .resizable(false)
        .show(ctx, |ui| {
            ui.heading("Men Among Gods Reforged");

            if let Some(err) = menu_ui.last_error.as_deref() {
                ui.colored_label(egui::Color32::LIGHT_RED, err);
            }

            ui.separator();

            if ui
                .add_sized([180., 36.], egui::Button::new("Open logs folder"))
                .clicked()
            {
                let log_dir = crate::resolve_log_dir();
                match open_dir_in_file_manager(&log_dir) {
                    Ok(()) => menu_ui.last_error = None,
                    Err(err) => menu_ui.last_error = Some(err),
                }
            }

            ui.add_space(10.0);

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
                user_settings.sync_from_resources(
                    &player_state,
                    &sound_settings,
                    &cursor_action_text,
                    &magic_settings,
                );
                user_settings.request_save();
            }

            if ui
                .checkbox(&mut cursor_action_text.enabled, "Show cursor action text")
                .changed()
            {
                user_settings.sync_from_resources(
                    &player_state,
                    &sound_settings,
                    &cursor_action_text,
                    &magic_settings,
                );
                user_settings.request_save();
            }

            if ui
                .checkbox(&mut sound_settings.enabled, "Play sounds")
                .changed()
            {
                user_settings.sync_from_resources(
                    &player_state,
                    &sound_settings,
                    &cursor_action_text,
                    &magic_settings,
                );
                user_settings.request_save();
            }

            ui.horizontal(|ui| {
                ui.label("Volume");
                let slider = egui::Slider::new(&mut sound_settings.master_volume, 0.0..=1.0)
                    .show_value(false);
                let changed = ui.add_sized([220.0, 18.0], slider).changed();
                ui.label(format!("{:3.0}%", sound_settings.master_volume * 100.0));

                if changed {
                    user_settings.sync_from_resources(
                        &player_state,
                        &sound_settings,
                        &cursor_action_text,
                        &magic_settings,
                    );
                    user_settings.request_save();
                }
            });

            ui.separator();
            ui.heading("Graphics");

            if let Some(mut window) = windows.iter_mut().next() {
                ui.horizontal(|ui| {
                    ui.label("Video mode");

                    let current = VideoModeSetting::from_window_mode(&window.mode);
                    let mut selected = current;
                    let mut changed = false;

                    egui::ComboBox::from_id_salt("video_mode_combo")
                        .selected_text(selected.label())
                        .show_ui(ui, |ui| {
                            changed |= ui
                                .selectable_value(
                                    &mut selected,
                                    VideoModeSetting::Windowed,
                                    VideoModeSetting::Windowed.label(),
                                )
                                .changed();
                            changed |= ui
                                .selectable_value(
                                    &mut selected,
                                    VideoModeSetting::BorderlessFullscreen,
                                    VideoModeSetting::BorderlessFullscreen.label(),
                                )
                                .changed();
                            changed |= ui
                                .selectable_value(
                                    &mut selected,
                                    VideoModeSetting::Fullscreen,
                                    VideoModeSetting::Fullscreen.label(),
                                )
                                .changed();
                        });

                    if changed && selected != current {
                        window.mode = selected.to_window_mode();
                        user_settings.settings.video_mode = selected;
                        user_settings.request_save();
                    }
                });
            }

            if ui
                .checkbox(&mut magic_settings.enabled, "Enable magic screen effects")
                .changed()
            {
                user_settings.sync_from_resources(
                    &player_state,
                    &sound_settings,
                    &cursor_action_text,
                    &magic_settings,
                );
                user_settings.request_save();
            }

            ui.horizontal(|ui| {
                ui.label("Gamma");
                let slider =
                    egui::Slider::new(&mut magic_settings.gamma, 0.8..=1.2).show_value(false);
                let changed = ui.add_sized([220.0, 18.0], slider).changed();
                ui.label(format!("{:.2}", magic_settings.gamma));

                if changed {
                    user_settings.sync_from_resources(
                        &player_state,
                        &sound_settings,
                        &cursor_action_text,
                        &magic_settings,
                    );
                    user_settings.request_save();
                }
            });
        });
}
