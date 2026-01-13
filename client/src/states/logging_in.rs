// Placeholders

use bevy::ecs::system::Commands;
use bevy::prelude::*;
use bevy_egui::{
    egui::{self, Pos2},
    EguiContexts,
};

use crate::constants::{TARGET_HEIGHT, TARGET_WIDTH};

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

#[derive(Resource, Clone, PartialEq, Eq, Hash, Debug)]

pub struct LoginInformation {
    pub username: String,
    pub password: String,
    pub description: String,
    pub is_male: bool,
    pub class: Class,
}

impl Default for LoginInformation {
    fn default() -> Self {
        Self {
            username: String::new(),
            password: String::new(),
            description: String::new(),
            is_male: true,
            class: Class::Mercenary,
        }
    }
}

pub fn setup_logging_in(mut commands: Commands, _asset_server: Res<AssetServer>) {
    log::debug!("setup_logging_in - start");

    // Store login UI state as a resource so egui can mutate it.
    commands.init_resource::<LoginInformation>();

    // Here you would set up your logging in UI elements, e.g., spawn entities
    log::debug!("setup_logging_in - end");
}

pub fn teardown_logging_in() {
    log::debug!("teardown_logging_in - start");
    log::debug!("teardown_logging_in - end");
}

pub fn run_logging_in(mut contexts: EguiContexts, mut login_info: ResMut<LoginInformation>) {
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
                        ui.radio_value(&mut login_info.class, Class::ArchHarakim, "ArchHarakim");
                        ui.radio_value(&mut login_info.class, Class::ArchTemplar, "ArchTemplar");
                        ui.radio_value(&mut login_info.class, Class::SeyanDu, "SeyanDu");
                    });
                });
            });

            ui.add_space(30.0);

            ui.horizontal(|ui| {
                let clear_button = ui.add_sized([120., 40.], egui::Button::new("Clear"));
                if clear_button.clicked() {
                    *login_info = LoginInformation::default();
                }

                let login_button = ui.add_sized([120., 40.], egui::Button::new("Login"));
                if login_button.clicked() {
                    // TODO: Eventually we will handle the login action here.
                    println!("Button clicked: {:?}", *login_info);
                }
            });
        });
    debug_once!("run_logging_in completed");
}
