//! Top menu bar and the list/details central panel.

use super::{TemplateViewerApp, ViewMode};
use eframe::egui;
use server_utils::DataSource;

impl TemplateViewerApp {
    /// Menu bar, tab selector, API buttons, and the status line.
    pub(super) fn ui_top_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| self.ui_file_menu(ui, ctx));
                ui.separator();

                for mode in ViewMode::ALL {
                    if ui
                        .selectable_label(self.view_mode == mode, mode.title())
                        .clicked()
                    {
                        self.view_mode = mode;
                    }
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    self.ui_api_buttons(ui);
                });
            });

            ui.separator();
            ui.label(format!("Source: {}", self.data_source.display_label()));

            if let Some(error) = &self.load_error {
                ui.separator();
                ui.colored_label(egui::Color32::RED, format!("Error: {}", error));
            }
            if let Some(error) = &self.graphics_zip_error {
                ui.separator();
                ui.colored_label(egui::Color32::YELLOW, format!("GFX: {}", error));
            }
            if self.dirty {
                ui.separator();
                ui.colored_label(egui::Color32::YELLOW, "Unsaved changes");
            }
            if let Some(status) = &self.save_status {
                ui.separator();
                let is_error = ["Save failed", "Duplicate failed"]
                    .iter()
                    .any(|prefix| status.starts_with(prefix));
                let color = if is_error {
                    egui::Color32::RED
                } else {
                    egui::Color32::GREEN
                };
                ui.colored_label(color, status);
            }
        });
    }

    /// Contents of the File menu.
    fn ui_file_menu(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let is_live_api = self.data_source.is_live_api();
        let save_label = if is_live_api {
            "Save to API\tCtrl+S"
        } else {
            "Save Snapshot As...\tCtrl+S"
        };
        if ui
            .add_enabled(self.loaded_world.is_some(), egui::Button::new(save_label))
            .clicked()
        {
            self.save_current();
            ui.close_menu();
        }

        if is_live_api {
            if ui
                .add_enabled(
                    self.admin_client.is_some(),
                    egui::Button::new("Reload server templates..."),
                )
                .clicked()
            {
                self.reload_confirm_open = true;
                ui.close_menu();
            }
            if ui
                .add_enabled(
                    self.pending_reload_request_id.is_some(),
                    egui::Button::new("Poll reload status"),
                )
                .clicked()
            {
                self.poll_reload_status();
                ui.close_menu();
            }
        }

        if ui
            .add_enabled(self.dirty, egui::Button::new("Revert (discard changes)"))
            .clicked()
        {
            self.revert_unsaved_changes();
            ui.close_menu();
        }

        ui.separator();

        if ui.button("Reload snapshot").clicked() {
            self.load_current_source();
            ui.close_menu();
        }

        ui.separator();

        ui.menu_button("Data Source", |ui| {
            let is_snapshot = matches!(self.data_source, DataSource::SnapshotFile(_));
            if ui
                .selectable_label(is_snapshot, ".wsnap Snapshot")
                .clicked()
            {
                self.open_snapshot_dialog();
                ui.close_menu();
            }
            if ui
                .selectable_label(is_live_api, "Live Admin API...")
                .clicked()
            {
                self.open_connect_dialog();
                ui.close_menu();
            }
        });

        ui.separator();

        if ui.button("Open snapshot...").clicked() {
            self.open_snapshot_dialog();
            ui.close_menu();
        }

        if ui.button("Select Graphics Zip...").clicked() {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("Zip", &["zip"])
                .pick_file()
            {
                self.load_graphics_zip(path);
            }
            ui.close_menu();
        }

        if self.graphics_zip.is_some() && ui.button("Clear Graphics Zip").clicked() {
            self.graphics_zip = None;
            self.graphics_zip_error = None;
            ui.close_menu();
        }

        ui.separator();

        if ui.button("Exit").clicked() {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    /// Right-aligned connect / reload-server buttons.
    fn ui_api_buttons(&mut self, ui: &mut egui::Ui) {
        let is_live_api = self.data_source.is_live_api();
        if is_live_api {
            let reload_btn = egui::Button::new(
                egui::RichText::new("Reload Server Templates").color(egui::Color32::WHITE),
            )
            .fill(egui::Color32::from_rgb(160, 60, 60));
            if ui
                .add_enabled(self.admin_client.is_some(), reload_btn)
                .on_hover_text(
                    "Ask the running server to swap its in-memory template tables. \
                     You will be asked to confirm.",
                )
                .clicked()
            {
                self.reload_confirm_open = true;
            }
        }

        let connect_label = if is_live_api {
            "API: Connected"
        } else {
            "Connect to API..."
        };
        if ui
            .button(connect_label)
            .on_hover_text(
                "Point this viewer at a running API service \
                 (local dev or production).",
            )
            .clicked()
        {
            self.open_connect_dialog();
        }
    }

    /// Record list on the left, details of the selected record on the right.
    pub(super) fn ui_central_panel(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            let mode = self.view_mode;
            egui::SidePanel::left(mode.list_panel_id())
                .resizable(true)
                .default_width(300.0)
                .show_inside(ui, |ui| {
                    ui.heading(self.list_heading(mode));
                    ui.separator();
                    self.render_list(ui, mode);
                });

            egui::CentralPanel::default().show_inside(ui, |ui| match self.selected_index(mode) {
                Some(idx) if mode.is_item() => {
                    self.render_item_details_by_index(ui, mode, idx);
                }
                Some(idx) => self.render_character_details_by_index(ui, mode, idx),
                None => {
                    ui.centered_and_justified(|ui| ui.label(mode.empty_hint()));
                }
            });
        });
    }
}
