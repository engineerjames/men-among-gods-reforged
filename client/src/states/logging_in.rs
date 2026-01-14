// Placeholders

use std::{path::PathBuf, sync::Arc};

use bevy::ecs::system::Commands;
use bevy::prelude::*;
use bevy_egui::{
    egui::{self, Pos2},
    EguiContexts,
};
use egui_file_dialog::FileDialog;

use crate::constants::{TARGET_HEIGHT, TARGET_WIDTH};
use crate::network::{LoginRequested, LoginStatus};

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]

pub enum Class {
    Mercenary,
    Templar,
    Harakim,

    // Achieved through gameplay:
    Sorceror,
    Warrior,
    ArchHarakim,
    ArchTemplar,
    SeyanDu,
}

#[derive(Resource, Debug)]

pub struct LoginUIState {
    username: String,
    password: String,
    description: String,
    is_male: bool,
    class: Class,
    loaded_character_file: Option<PathBuf>,
    load_character_dialog: FileDialog,
    save_character_dialog: FileDialog,
    is_logging_in: bool,
    server_ip: String,
    server_port: String,
}

impl Default for LoginUIState {
    fn default() -> Self {
        Self {
            username: String::new(),
            password: String::new(),
            description: String::new(),
            is_male: true,
            class: Class::Mercenary,
            loaded_character_file: None,
            load_character_dialog: FileDialog::new()
                .title("Load Character File")
                .add_file_filter(
                    "MOA Files",
                    Arc::new(|path| path.extension().unwrap_or_default() == "moa"),
                )
                .default_file_filter("MOA Files")
                .initial_directory(std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))),
            save_character_dialog: FileDialog::new()
                .title("Save Character File")
                .add_file_filter(
                    "MOA Files",
                    Arc::new(|path| path.extension().unwrap_or_default() == "moa"),
                )
                .default_file_filter("MOA Files")
                .initial_directory(std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))),
            is_logging_in: false,
            server_ip: String::from("127.0.0.1"),
            server_port: String::from("5555"),
        }
    }
}

pub fn setup_logging_in(mut commands: Commands, _asset_server: Res<AssetServer>) {
    log::debug!("setup_logging_in - start");

    // Store login UI state as a resource so egui can mutate it.
    commands.init_resource::<LoginUIState>();

    // Here you would set up your logging in UI elements, e.g., spawn entities
    log::debug!("setup_logging_in - end");
}

pub fn teardown_logging_in() {
    log::debug!("teardown_logging_in - start");
    log::debug!("teardown_logging_in - end");
}

pub fn run_logging_in(
    mut contexts: EguiContexts,
    mut login_info: ResMut<LoginUIState>,
    status: Res<LoginStatus>,
    mut login_ev: MessageWriter<LoginRequested>,
) {
    debug_once!("run_logging_in called");

    let Ok(ctx) = contexts.ctx_mut() else {
        debug_once!("run_logging_in: no egui context available");
        // TODO: Transition to critical error state?
        return;
    };

    egui::Window::new("Men Among Gods Reforged - Login")
        .default_height(TARGET_HEIGHT)
        .default_width(TARGET_WIDTH)
        .fixed_pos(Pos2::new(0.0, 0.0))
        .collapsible(false)
        .resizable(false)
        .show(ctx, |ui| {
            ui.add_enabled_ui(!login_info.is_logging_in, |ui| {
                ui.label("Server IP");
                ui.text_edit_singleline(&mut login_info.server_ip);

                ui.label("Server Port");
                ui.text_edit_singleline(&mut login_info.server_port);

                ui.separator();

                ui.label("Username");
                ui.text_edit_singleline(&mut login_info.username);

                ui.label("Password");
                ui.add(egui::TextEdit::singleline(&mut login_info.password).password(true));

                ui.separator();

                ui.label("Description");
                ui.text_edit_multiline(&mut login_info.description);

                ui.separator();
                ui.horizontal(|ui| {
                    ui.radio_value(&mut login_info.is_male, true, "Male");
                    ui.radio_value(&mut login_info.is_male, false, "Female");
                });
                ui.separator();
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label("Class: ");
                        ui.radio_value(&mut login_info.class, Class::Mercenary, "Mercenary");
                        ui.radio_value(&mut login_info.class, Class::Templar, "Templar");
                        ui.radio_value(&mut login_info.class, Class::Harakim, "Harakim");
                    });

                    ui.add_space(30.0);

                    ui.vertical(|ui| {
                        ui.add_enabled_ui(false, |ui| {
                            ui.label("Achieved through gameplay:");
                            ui.radio_value(&mut login_info.class, Class::Sorceror, "Sorceror");
                            ui.radio_value(&mut login_info.class, Class::Warrior, "Warrior");
                            ui.radio_value(
                                &mut login_info.class,
                                Class::ArchHarakim,
                                "ArchHarakim",
                            );
                            ui.radio_value(
                                &mut login_info.class,
                                Class::ArchTemplar,
                                "ArchTemplar",
                            );
                            ui.radio_value(&mut login_info.class, Class::SeyanDu, "SeyanDu");
                        });
                    });
                });

                ui.add_space(30.0);

                ui.horizontal(|ui| {
                    let clear_button = ui.add_sized([120., 40.], egui::Button::new("Clear"));
                    if clear_button.clicked() {
                        *login_info = LoginUIState::default();
                    }

                    let load_button = ui.add_sized([120., 40.], egui::Button::new("Load"));
                    if load_button.clicked() {
                        log::info!("Opening file dialog to load character file...");
                        login_info.load_character_dialog.pick_file();
                    }

                    let save_button = ui.add_sized([120., 40.], egui::Button::new("Save"));
                    if save_button.clicked() {
                        log::info!("Opening file dialog to save character file...");
                        login_info.save_character_dialog.save_file();
                    }

                    login_info.load_character_dialog.update(ctx);
                    login_info.save_character_dialog.update(ctx);

                    if let Some(path) = login_info.load_character_dialog.take_picked() {
                        login_info.loaded_character_file = Some(path.to_path_buf());
                        // TODO: Actually load the character data from the file here.
                        log::info!(
                            "Selected character file: {:?}",
                            login_info.loaded_character_file
                        );
                    }

                    if let Some(path) = login_info.save_character_dialog.take_picked() {
                        // TODO: Actually save the character data to the file here.
                        log::info!("Saving character to file: {:?}", path);
                    }

                    let login_button = ui.add_sized([120., 40.], egui::Button::new("Login"));
                    if login_button.clicked() {
                        login_info.is_logging_in = true;
                        log::info!(
                            "Attempting login for user '{}' to {}:{}",
                            login_info.username,
                            login_info.server_ip,
                            login_info.server_port
                        );
                        login_ev.write(LoginRequested {
                            host: login_info.server_ip.clone(),
                            port: login_info.server_port.parse().unwrap_or(5555),
                            username: login_info.username.clone(),
                            password: login_info.password.clone(),
                            race: get_race_integer(login_info.is_male, login_info.class),
                        });
                    }
                });
            });

            ui.add_enabled_ui(login_info.is_logging_in, |ui| {
                ui.label(format!("Login status: {}", &status.message));
            });
        });
    debug_once!("run_logging_in completed");
}

fn get_race_integer(is_male: bool, class: Class) -> i32 {
    if is_male {
        match class {
            Class::Templar => 3,
            Class::Mercenary => 2,
            Class::Harakim => 4,
            Class::SeyanDu => 13,
            Class::ArchTemplar => 544,
            Class::ArchHarakim => 545,
            Class::Sorceror => 546,
            Class::Warrior => 547,
        }
    } else {
        match class {
            Class::Templar => 77,
            Class::Mercenary => 76,
            Class::Harakim => 78,
            Class::SeyanDu => 79,
            Class::ArchTemplar => 549,
            Class::ArchHarakim => 550,
            Class::Sorceror => 551,
            Class::Warrior => 552,
        }
    }
}
