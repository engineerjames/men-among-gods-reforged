use std::sync::{mpsc, Arc, Mutex};

use bevy::prelude::*;
use bevy::tasks::{IoTaskPool, Task};
use bevy_egui::{
    egui::{self, Pos2},
    EguiContexts, EguiTextureHandle,
};

use crate::constants::{TARGET_HEIGHT, TARGET_WIDTH};
use crate::gfx_cache::GraphicsCache;
use crate::network::account_api::{self, ApiSession, CharacterRace, CharacterSex};
use crate::GameState;

#[derive(Resource, Debug)]
pub struct CharacterCreationUiState {
    name: String,
    description: String,
    selected_race: CharacterRace,
    selected_sex: CharacterSex,
    is_busy: bool,
    last_error: Option<String>,
    pending_task: Option<Task<()>>,
    pending_rx: Option<Arc<Mutex<mpsc::Receiver<Result<account_api::CharacterSummary, String>>>>>,
}

impl Default for CharacterCreationUiState {
    fn default() -> Self {
        Self {
            name: String::new(),
            description: String::new(),
            selected_race: CharacterRace::Mercenary,
            selected_sex: CharacterSex::Male,
            is_busy: false,
            last_error: None,
            pending_task: None,
            pending_rx: None,
        }
    }
}

pub fn setup_character_creation(mut commands: Commands, mut api_session: ResMut<ApiSession>) {
    api_session.ensure_defaults();
    commands.insert_resource(CharacterCreationUiState::default());
}

pub fn teardown_character_creation() {
    log::debug!("teardown_character_creation - end");
}

pub fn run_character_creation(
    mut contexts: EguiContexts,
    mut ui_state: ResMut<CharacterCreationUiState>,
    mut api_session: ResMut<ApiSession>,
    gfx: Res<GraphicsCache>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    let selected_sex = ui_state.selected_sex;
    let harakim_texture =
        texture_id_for_race(&mut contexts, &gfx, CharacterRace::Harakim, selected_sex);
    let templar_texture =
        texture_id_for_race(&mut contexts, &gfx, CharacterRace::Templar, selected_sex);
    let mercenary_texture =
        texture_id_for_race(&mut contexts, &gfx, CharacterRace::Mercenary, selected_sex);

    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };

    let rx_arc = ui_state.pending_rx.as_ref().map(Arc::clone);
    if let Some(rx_arc) = rx_arc {
        enum PendingStatus {
            Empty,
            Disconnected,
            Locked,
            Ready(Result<account_api::CharacterSummary, String>),
        }

        let status = match rx_arc.lock() {
            Ok(rx) => match rx.try_recv() {
                Ok(result) => PendingStatus::Ready(result),
                Err(mpsc::TryRecvError::Disconnected) => PendingStatus::Disconnected,
                Err(mpsc::TryRecvError::Empty) => PendingStatus::Empty,
            },
            Err(_) => PendingStatus::Locked,
        };

        match status {
            PendingStatus::Ready(result) => {
                ui_state.pending_rx = None;
                ui_state.pending_task = None;
                ui_state.is_busy = false;

                match result {
                    Ok(summary) => {
                        api_session.pending_notice = Some(format!(
                            "Character created: {} (id={})",
                            summary.name, summary.id
                        ));
                        ui_state.last_error = None;
                        next_state.set(GameState::CharacterSelection);
                    }
                    Err(err) => {
                        ui_state.last_error = Some(err);
                    }
                }
            }
            PendingStatus::Disconnected => {
                ui_state.pending_rx = None;
                ui_state.pending_task = None;
                ui_state.is_busy = false;
                ui_state.last_error = Some("Character creation failed: channel closed".to_string());
            }
            PendingStatus::Locked => {
                ui_state.pending_rx = None;
                ui_state.pending_task = None;
                ui_state.is_busy = false;
                ui_state.last_error = Some("Character creation failed: channel locked".to_string());
            }
            PendingStatus::Empty => {}
        }
    }

    egui::Window::new("Create Character")
        .default_height(TARGET_HEIGHT)
        .default_width(TARGET_WIDTH)
        .fixed_pos(Pos2::new(0.0, 0.0))
        .collapsible(false)
        .resizable(false)
        .show(ctx, |ui| {
            ui.heading("Create character (placeholder)");

            if let Some(username) = api_session.username.as_deref() {
                ui.label(format!("Logged in as: {username}"));
            } else {
                ui.label("No account session available");
            }

            if let Some(err) = ui_state.last_error.as_deref() {
                ui.colored_label(egui::Color32::LIGHT_RED, err);
            }

            ui.add_space(12.0);
            ui.label("Name");
            ui.add_enabled(
                !ui_state.is_busy,
                egui::TextEdit::singleline(&mut ui_state.name).desired_width(260.0),
            );

            ui.add_space(8.0);
            ui.label("Description");
            ui.add_enabled(
                !ui_state.is_busy,
                egui::TextEdit::multiline(&mut ui_state.description)
                    .desired_rows(3)
                    .desired_width(260.0),
            );

            ui.add_space(12.0);
            ui.label("Race");

            ui.group(|ui| {
                ui.vertical(|ui| {
                    race_option_ui(
                        ui,
                        &mut ui_state.selected_race,
                        CharacterRace::Harakim,
                        "Harakim",
                        harakim_texture,
                    );
                    race_option_ui(
                        ui,
                        &mut ui_state.selected_race,
                        CharacterRace::Templar,
                        "Templar",
                        templar_texture,
                    );
                    race_option_ui(
                        ui,
                        &mut ui_state.selected_race,
                        CharacterRace::Mercenary,
                        "Mercenary",
                        mercenary_texture,
                    );
                });
            });

            ui.add_space(12.0);
            ui.label("Sex");

            ui.group(|ui| {
                ui.horizontal(|ui| {
                    ui.radio_value(&mut ui_state.selected_sex, CharacterSex::Male, "Male");
                    ui.radio_value(&mut ui_state.selected_sex, CharacterSex::Female, "Female");
                });
            });

            ui.add_space(16.0);

            let create_clicked = ui
                .add_enabled(
                    !ui_state.is_busy,
                    egui::Button::new("Create character").min_size([180.0, 32.0].into()),
                )
                .clicked();

            let back_clicked = ui
                .add_enabled(
                    !ui_state.is_busy,
                    egui::Button::new("Back").min_size([120.0, 32.0].into()),
                )
                .clicked();

            if create_clicked {
                let name = ui_state.name.trim().to_string();
                let description = ui_state.description.trim().to_string();

                let Some(token) = api_session.token.as_deref() else {
                    ui_state.last_error = Some("Missing account session token".to_string());
                    return;
                };

                if name.is_empty() {
                    ui_state.last_error = Some("Character name is required".to_string());
                    return;
                }

                ui_state.is_busy = true;
                ui_state.last_error = None;

                let base_url = api_session.base_url.clone();
                let token = token.to_string();
                let race = ui_state.selected_race;
                let sex = ui_state.selected_sex;
                let description = if description.is_empty() {
                    None
                } else {
                    Some(description)
                };

                let (tx, rx) = mpsc::channel();
                let rx = Arc::new(Mutex::new(rx));
                let task = IoTaskPool::get().spawn(async move {
                    let result = account_api::create_character(
                        &base_url,
                        &token,
                        &name,
                        description.as_deref(),
                        sex,
                        race,
                    );
                    let _ = tx.send(result);
                });

                ui_state.pending_task = Some(task);
                ui_state.pending_rx = Some(rx);
            }

            if back_clicked {
                ui_state.last_error = None;
                next_state.set(GameState::CharacterSelection);
            }
        });
}

fn race_option_ui(
    ui: &mut egui::Ui,
    selected_race: &mut CharacterRace,
    race: CharacterRace,
    label: &str,
    texture_id: Option<egui::TextureId>,
) {
    ui.horizontal(|ui| {
        ui.radio_value(selected_race, race, label);

        if let Some(texture_id) = texture_id {
            let size = egui::vec2(64.0, 64.0);
            let textured = egui::load::SizedTexture::new(texture_id, size);
            ui.add(egui::Image::new(textured));
        } else {
            ui.label("Image missing");
        }
    });
}

fn texture_id_for_race(
    contexts: &mut EguiContexts,
    gfx: &GraphicsCache,
    race: CharacterRace,
    sex: CharacterSex,
) -> Option<egui::TextureId> {
    let sprite_id = sprite_id_for_selection(race, sex);
    let image = gfx
        .get_sprite(sprite_id)
        .map(|sprite| sprite.image.clone())?;
    let asset_id = image.id();
    let texture_id = contexts
        .image_id(asset_id)
        .unwrap_or_else(|| contexts.add_image(EguiTextureHandle::Weak(asset_id)));
    Some(texture_id)
}

fn sprite_id_for_selection(race: CharacterRace, sex: CharacterSex) -> usize {
    match (race, sex) {
        (CharacterRace::Harakim, CharacterSex::Male) => 4048,
        (CharacterRace::Templar, CharacterSex::Male) => 2000,
        (CharacterRace::Mercenary, CharacterSex::Male) => 5072,
        (CharacterRace::Harakim, CharacterSex::Female) => 6096,
        (CharacterRace::Templar, CharacterSex::Female) => 8144,
        (CharacterRace::Mercenary, CharacterSex::Female) => 7120,
        _ => 5072,
    }
}
