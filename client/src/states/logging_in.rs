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
use crate::helpers::open_dir_in_file_manager;
use crate::network::{LoginRequested, LoginStatus};
use crate::player_state::PlayerState;
use crate::settings::{UserSettings, UserSettingsState, DEFAULT_SERVER_IP, DEFAULT_SERVER_PORT};
use crate::types::mag_files;

/// Writes a Rust string into a fixed-size, NUL-terminated ASCII buffer.
///
/// This mirrors the original client's C string behavior (NUL-terminated, zero padded, and
/// non-ASCII/control characters replaced with spaces).
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

    /// When set, the UI will re-apply initial values from `settings.json`.
    /// Used after failed login attempts to unlock the screen and restore defaults.
    reset_to_settings: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ConfirmAction {
    Clear,
    Load,
}

impl Default for LoginUIState {
    /// Creates a default login UI model and initializes file dialogs.
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
            server_ip: DEFAULT_SERVER_IP.to_string(),
            server_port: DEFAULT_SERVER_PORT.to_string(),

            confirm: None,
            last_error: None,
            last_notice: None,

            reset_to_settings: false,
        }
    }
}

impl LoginUIState {
    pub fn on_login_failed(&mut self, err: String) {
        self.is_logging_in = false;
        self.reset_to_settings = true;
        self.last_error = Some(err);
        self.last_notice = None;
    }
}

fn apply_settings_to_login_ui(
    login_info: &mut LoginUIState,
    player_state: &mut PlayerState,
    settings: &UserSettings,
) {
    login_info.server_ip = settings.default_server_ip.clone();
    login_info.server_port = settings.default_server_port.to_string();

    player_state.set_character_from_file(settings.save_file, settings.player_data);

    // The authoritative character name is `pdata.cname`.
    let username = mag_files::fixed_ascii_to_string(&settings.player_data.cname);
    let username = if username.is_empty() {
        mag_files::fixed_ascii_to_string(&settings.save_file.name)
    } else {
        username
    };

    login_info.username = username;
    login_info.password.clear();
    login_info.description = mag_files::fixed_ascii_to_string(&settings.player_data.desc);

    let (is_male, class) = class_from_race(settings.save_file.race);
    login_info.is_male = is_male;
    login_info.class = class;

    login_info.loaded_character_file = None;
    login_info.is_logging_in = false;
    login_info.confirm = None;
}

/// Sets up the login screen state.
pub fn setup_logging_in(
    mut commands: Commands,
    _asset_server: Res<AssetServer>,
    mut player_state: ResMut<PlayerState>,
    user_settings: Res<UserSettingsState>,
) {
    log::debug!("setup_logging_in - start");

    // Prefill UI and runtime state from settings.json.
    let settings = &user_settings.settings;
    let mut login_info = LoginUIState::default();
    apply_settings_to_login_ui(&mut login_info, &mut player_state, settings);

    // Re-apply global user settings on top of persisted character state.
    // (Only shadows currently overlaps PlayerData.)
    player_state.player_data_mut().are_shadows_enabled =
        if settings.render_shadows { 1 } else { 0 };

    commands.insert_resource(login_info);

    // Here you would set up your logging in UI elements, e.g., spawn entities
    log::debug!("setup_logging_in - end");
}

/// Tears down the login screen state.
pub fn teardown_logging_in() {
    log::debug!("teardown_logging_in - start");
    log::debug!("teardown_logging_in - end");
}

/// Runs the login screen UI (egui) and emits `LoginRequested` when the user logs in.
///
/// Also handles persistence: saving/loading character `.mag` files.
pub fn run_logging_in(
    mut contexts: EguiContexts,
    mut login_info: ResMut<LoginUIState>,
    status: Res<LoginStatus>,
    mut login_ev: MessageWriter<LoginRequested>,
    mut player_state: ResMut<PlayerState>,
    mut user_settings: ResMut<UserSettingsState>,
) {
    debug_once!("run_logging_in called");

    let Ok(ctx) = contexts.ctx_mut() else {
        debug_once!("run_logging_in: no egui context available");
        // TODO: Transition to critical error state?
        return;
    };

    if login_info.reset_to_settings {
        login_info.reset_to_settings = false;
        let last_error = login_info.last_error.clone();
        apply_settings_to_login_ui(&mut login_info, &mut player_state, &user_settings.settings);
        // Keep the error visible after reset.
        login_info.last_error = last_error;
        login_info.last_notice = None;
    }

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

                    let open_logs_button =
                        ui.add_sized([120., 40.], egui::Button::new("Open logs"));
                    if open_logs_button.clicked() {
                        let log_dir = crate::resolve_log_dir();
                        match open_dir_in_file_manager(&log_dir) {
                            Ok(()) => {
                                login_info.last_error = None;
                                login_info.last_notice =
                                    Some(format!("Opened logs folder: {}", log_dir.display()));
                            }
                            Err(err) => {
                                login_info.last_error = Some(err);
                                login_info.last_notice = None;
                            }
                        }
                    }

                    login_info.load_character_dialog.update(ctx);
                    login_info.save_character_dialog.update(ctx);

                    if let Some(path) = login_info.load_character_dialog.take_picked() {
                        let picked = path.to_path_buf();
                        let picked = ensure_mag_extension(picked);
                        match mag_files::load_character_file(&picked) {
                            Ok((save_file, player_data)) => {
                                player_state.set_character_from_file(save_file, player_data);

                                // Re-apply global user settings (character load overwrites pdata).
                                player_state.player_data_mut().are_shadows_enabled =
                                    if user_settings.settings.render_shadows {
                                        1
                                    } else {
                                        0
                                    };

                                login_info.loaded_character_file = Some(picked);

                                // The authoritative character name is `pdata.cname`.
                                // (Empty cname is rejected by `load_character_file`.)
                                let username = mag_files::fixed_ascii_to_string(&player_data.cname);
                                login_info.username = username.clone();

                                // Keep key name in sync with cname for persistence.
                                let save_file = player_state.save_file_mut();
                                write_ascii_into_fixed(&mut save_file.name, &username);
                                login_info.description =
                                    mag_files::fixed_ascii_to_string(&player_data.desc);
                                let (is_male, class) = class_from_race(save_file.race);
                                login_info.is_male = is_male;
                                login_info.class = class;

                                // Persist the latest state into settings.json.
                                user_settings.sync_character_from_player_state(&player_state);
                                user_settings.request_save();
                                login_info.last_error = None;
                            }
                            Err(e) => {
                                log::error!("Failed to load .mag file {:?}: {e}", picked);
                                login_info.last_error = Some(format!("Failed to load .mag: {e}"));
                                login_info.last_notice = None;
                            }
                        }
                    }

                    if let Some(path) = login_info.save_character_dialog.take_picked() {
                        let picked = ensure_mag_extension(path.to_path_buf());

                        // Ensure the `.mag` file reflects the current login UI fields.
                        // (xbuttons are already stored in `player_state.player_data()`.)
                        {
                            let save_file = player_state.save_file_mut();
                            write_ascii_into_fixed(&mut save_file.name, &login_info.username);
                            save_file.race = get_race_integer(login_info.is_male, login_info.class);
                        }
                        {
                            let pdata = player_state.player_data_mut();
                            write_ascii_into_fixed(&mut pdata.cname, &login_info.username);
                            write_ascii_into_fixed(&mut pdata.desc, &login_info.description);
                        }

                        if let Err(e) = mag_files::save_character_file(
                            &picked,
                            player_state.save_file(),
                            player_state.player_data(),
                        ) {
                            log::error!("Failed to save .mag file {:?}: {e}", picked);
                            login_info.last_error = Some(format!("Failed to save .mag: {e}"));
                            login_info.last_notice = None;
                        } else {
                            login_info.last_error = None;
                            login_info.last_notice =
                                Some(format!("Saved as \"{}\".", picked.display()));
                            log::info!("Saved character to file: {:?}", picked);
                        }
                    }

                    let login_button = ui.add_sized([120., 40.], egui::Button::new("Login"));
                    if login_button.clicked() {
                        login_info.is_logging_in = true;

                        // Persist the login screen server fields into settings.json only when the
                        // user commits the action (presses Login), not while typing.
                        let committed_ip = login_info.server_ip.trim().to_string();
                        let committed_port = login_info.server_port.trim().parse::<u16>().ok();
                        if !committed_ip.is_empty() {
                            let mut changed = false;
                            if user_settings.settings.default_server_ip != committed_ip {
                                user_settings.settings.default_server_ip = committed_ip;
                                changed = true;
                            }
                            if let Some(port) = committed_port {
                                if user_settings.settings.default_server_port != port {
                                    user_settings.settings.default_server_port = port;
                                    changed = true;
                                }
                            }
                            if changed {
                                user_settings.request_save();
                            }
                        }

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

                        // Persist current UI/runtime state into settings.json.
                        user_settings.sync_character_from_player_state(&player_state);
                        user_settings.request_save();

                        log::info!(
                            "Attempting login for user '{}' to {}:{}",
                            login_info.username,
                            login_info.server_ip,
                            login_info.server_port
                        );

                        let save_file = *player_state.save_file();
                        login_ev.write(LoginRequested {
                            host: login_info.server_ip.clone(),
                            port: login_info
                                .server_port
                                .parse()
                                .unwrap_or(DEFAULT_SERVER_PORT),
                            username: login_info.username.clone(),
                            password: login_info.password.clone(),
                            race: get_race_integer(login_info.is_male, login_info.class),

                            user_id: save_file.usnr,
                            pass1: save_file.pass1,
                            pass2: save_file.pass2,
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
                                login_info.server_ip =
                                    user_settings.settings.default_server_ip.clone();
                                login_info.server_port =
                                    user_settings.settings.default_server_port.to_string();
                                player_state.set_character_from_file(
                                    crate::types::save_file::SaveFile::default(),
                                    crate::types::player_data::PlayerData::default(),
                                );

                                // Persist cleared state.
                                user_settings.sync_character_from_player_state(&player_state);
                                user_settings.request_save();
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

/// Ensures a selected path ends with the `.mag` extension.
fn ensure_mag_extension(mut path: PathBuf) -> PathBuf {
    match path.extension().and_then(|e| e.to_str()) {
        Some("mag") => path,
        _ => {
            path.set_extension("mag");
            path
        }
    }
}

/// Decodes the legacy race integer into `(is_male, class)`.
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

/// Encodes `(is_male, class)` into the legacy race integer.
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
