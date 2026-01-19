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
use crate::player_state::PlayerState;
use crate::types::mag_files;

fn write_ascii_into_fixed(dst: &mut [u8], s: &str) {
    // Match the original client's fixed-size C strings:
    // - NUL-terminated
    // - padded with zeros
    // - non-ASCII / control chars replaced with space
    dst.fill(0);
    if dst.is_empty() {
        return;
    }

    let mut i = 0usize;
    for &b in s.as_bytes() {
        if i >= dst.len().saturating_sub(1) {
            break;
        }

        // Keep visible ASCII; map others to space.
        dst[i] = if (32..=126).contains(&b) { b } else { b' ' };
        i += 1;
    }
}

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

    confirm: Option<ConfirmAction>,
    last_error: Option<String>,
    last_notice: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ConfirmAction {
    Clear,
    Load,
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
                    "MAG Files",
                    Arc::new(|path| path.extension().unwrap_or_default() == "mag"),
                )
                .default_file_filter("MAG Files")
                .initial_directory(std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))),
            save_character_dialog: FileDialog::new()
                .title("Save Character File")
                .add_file_filter(
                    "MAG Files",
                    Arc::new(|path| path.extension().unwrap_or_default() == "mag"),
                )
                .default_file_filter("MAG Files")
                .initial_directory(std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))),
            is_logging_in: false,
            server_ip: String::from("127.0.0.1"),
            server_port: String::from("5555"),

            confirm: None,
            last_error: None,
            last_notice: None,
        }
    }
}

pub fn setup_logging_in(mut commands: Commands, _asset_server: Res<AssetServer>, mut player_state: ResMut<PlayerState>) {
    log::debug!("setup_logging_in - start");

    // Load persisted mag.dat (if present) and pre-fill UI + runtime pdata/key.
    let mut login_info = LoginUIState::default();
    match mag_files::load_mag_dat() {
        Ok(mag_dat) => {
            let ip = mag_files::fixed_ascii_to_string(&mag_dat.server_ip);
            if !ip.is_empty() {
                login_info.server_ip = ip;
            }
            if mag_dat.server_port != 0 {
                login_info.server_port = mag_dat.server_port.to_string();
            }

            // Apply the stored character info to both UI and runtime state.
            let username = mag_files::fixed_ascii_to_string(&mag_dat.save_file.name);
            if !username.is_empty() {
                login_info.username = username;
            }
            login_info.description = mag_files::fixed_ascii_to_string(&mag_dat.player_data.desc);
            let (is_male, class) = class_from_race(mag_dat.save_file.race);
            login_info.is_male = is_male;
            login_info.class = class;

            player_state.set_character_from_file(mag_dat.save_file, mag_dat.player_data);
        }
        Err(e) => {
            // Non-fatal. UI will still work with defaults.
            login_info.last_error = Some(format!("Failed to load mag.dat: {e}"));
        }
    }

    commands.insert_resource(login_info);

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
    mut player_state: ResMut<PlayerState>,
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
            if let Some(msg) = login_info.last_notice.as_deref() {
                ui.colored_label(egui::Color32::LIGHT_GREEN, msg);
            }
            if let Some(err) = login_info.last_error.as_deref() {
                ui.colored_label(egui::Color32::LIGHT_RED, err);
            }

            if login_info.last_notice.is_some() || login_info.last_error.is_some() {
                ui.separator();
            }

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
                        login_info.confirm = Some(ConfirmAction::Clear);
                        login_info.last_notice = None;
                    }

                    let load_button = ui.add_sized([120., 40.], egui::Button::new("Load"));
                    if load_button.clicked() {
                        login_info.confirm = Some(ConfirmAction::Load);
                        login_info.last_notice = None;
                    }

                    let save_button = ui.add_sized([120., 40.], egui::Button::new("Save"));
                    if save_button.clicked() {
                        log::info!("Opening file dialog to save character file...");
                        login_info.save_character_dialog.save_file();
                        login_info.last_notice = None;
                    }

                    login_info.load_character_dialog.update(ctx);
                    login_info.save_character_dialog.update(ctx);

                    if let Some(path) = login_info.load_character_dialog.take_picked() {
                        let picked = path.to_path_buf();
                        let picked = ensure_mag_extension(picked);
                        match mag_files::load_character_file(&picked) {
                            Ok((save_file, player_data)) => {
                                player_state.set_character_from_file(save_file, player_data);

                                login_info.loaded_character_file = Some(picked);
                                login_info.username = mag_files::fixed_ascii_to_string(&save_file.name);
                                login_info.description = mag_files::fixed_ascii_to_string(&player_data.desc);
                                let (is_male, class) = class_from_race(save_file.race);
                                login_info.is_male = is_male;
                                login_info.class = class;

                                // Persist the latest state into mag.dat.
                                if let Ok(port) = login_info.server_port.parse::<u16>() {
                                    let mag_dat = mag_files::build_mag_dat(
                                        &login_info.server_ip,
                                        port,
                                        player_state.save_file(),
                                        player_state.player_data(),
                                    );
                                    if let Err(e) = mag_files::save_mag_dat(&mag_dat) {
                                        login_info.last_error = Some(format!("Failed to save mag.dat: {e}"));
                                        login_info.last_notice = None;
                                    } else {
                                        login_info.last_error = None;
                                    }
                                }
                            }
                            Err(e) => {
                                login_info.last_error = Some(format!("Failed to load .mag: {e}"));
                                login_info.last_notice = None;
                            }
                        }
                    }

                    if let Some(path) = login_info.save_character_dialog.take_picked() {
                        let picked = ensure_mag_extension(path.to_path_buf());
                        if let Err(e) = mag_files::save_character_file(
                            &picked,
                            player_state.save_file(),
                            player_state.player_data(),
                        ) {
                            login_info.last_error = Some(format!("Failed to save .mag: {e}"));
                            login_info.last_notice = None;
                        } else {
                            login_info.last_error = None;
                            login_info.last_notice = Some(format!("Saved as \"{}\".", picked.display()));
                            log::info!("Saved character to file: {:?}", picked);
                        }
                    }

                    let login_button = ui.add_sized([120., 40.], egui::Button::new("Login"));
                    if login_button.clicked() {
                        login_info.is_logging_in = true;

                        // Mirror login selections into the persisted key file layout.
                        {
                            let save_file = player_state.save_file_mut();
                            write_ascii_into_fixed(&mut save_file.name, &login_info.username);
                            save_file.race = get_race_integer(login_info.is_male, login_info.class);
                        }

                        // Ensure user-entered character name/description are pushed to pdata
                        // so gameplay's `send_opt()` will transmit them to the server.
                        {
                            let pdata = player_state.player_data_mut();
                            write_ascii_into_fixed(&mut pdata.cname, &login_info.username);
                            write_ascii_into_fixed(&mut pdata.desc, &login_info.description);
                            pdata.changed = 1;
                        }

                        // Persist current UI/runtime state into mag.dat.
                        if let Ok(port) = login_info.server_port.parse::<u16>() {
                            let mag_dat = mag_files::build_mag_dat(
                                &login_info.server_ip,
                                port,
                                player_state.save_file(),
                                player_state.player_data(),
                            );
                            if let Err(e) = mag_files::save_mag_dat(&mag_dat) {
                                login_info.last_error = Some(format!("Failed to save mag.dat: {e}"));
                            }
                        }

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

    // Confirmation modal.
    if let Some(action) = login_info.confirm {
        let msg = "Before loading or clearing, save your character via the Save button.\n\nUnsaved changes may be lost.\n\nContinue?";
        egui::Window::new("Warning")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.label(msg);
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if ui.button("Cancel").clicked() {
                        login_info.confirm = None;
                    }

                    if ui.button("Continue").clicked() {
                        match action {
                            ConfirmAction::Clear => {
                                *login_info = LoginUIState::default();
                                player_state.set_character_from_file(
                                    crate::types::save_file::SaveFile::default(),
                                    crate::types::player_data::PlayerData::default(),
                                );

                                // Persist cleared state.
                                if let Ok(port) = login_info.server_port.parse::<u16>() {
                                    let mag_dat = mag_files::build_mag_dat(
                                        &login_info.server_ip,
                                        port,
                                        player_state.save_file(),
                                        player_state.player_data(),
                                    );
                                    if let Err(e) = mag_files::save_mag_dat(&mag_dat) {
                                        login_info.last_error = Some(format!("Failed to save mag.dat: {e}"));
                                    }
                                }
                            }
                            ConfirmAction::Load => {
                                log::info!("Opening file dialog to load character file...");
                                login_info.load_character_dialog.pick_file();
                            }
                        }
                        login_info.confirm = None;
                    }
                });
            });
    }
    debug_once!("run_logging_in completed");
}

fn ensure_mag_extension(mut path: PathBuf) -> PathBuf {
    match path.extension().and_then(|e| e.to_str()) {
        Some("mag") => path,
        _ => {
            path.set_extension("mag");
            path
        }
    }
}

fn class_from_race(race: i32) -> (bool, Class) {
    match race {
        3 => (true, Class::Templar),
        2 => (true, Class::Mercenary),
        4 => (true, Class::Harakim),
        13 => (true, Class::SeyanDu),
        544 => (true, Class::ArchTemplar),
        545 => (true, Class::ArchHarakim),
        546 => (true, Class::Sorceror),
        547 => (true, Class::Warrior),

        77 => (false, Class::Templar),
        76 => (false, Class::Mercenary),
        78 => (false, Class::Harakim),
        79 => (false, Class::SeyanDu),
        549 => (false, Class::ArchTemplar),
        550 => (false, Class::ArchHarakim),
        551 => (false, Class::Sorceror),
        552 => (false, Class::Warrior),

        _ => (true, Class::Mercenary),
    }
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
