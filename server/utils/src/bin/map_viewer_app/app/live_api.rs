//! Live admin-API integration: connect dialog, saving tile patches, and server map reloads.

use super::geometry::tile_index;
use super::{MapViewerApp, PendingItemAction};
use eframe::egui;
use mag_core::map_store::MapPatch;
use mag_core::world_action_store::WorldActionKind;
use server_utils::DataSource;
use server_utils::admin_client::AdminClient;
use std::collections::BTreeSet;
use std::time::{Duration, Instant};

/// How often a pending map-reload request is polled.
const RELOAD_POLL_INTERVAL: Duration = Duration::from_secs(2);
/// How long to keep polling a map-reload request before giving up.
const RELOAD_POLL_GIVE_UP: Duration = Duration::from_secs(300);

impl MapViewerApp {
    /// Push every dirty map tile and queued item action to the admin API.
    ///
    /// Each static tile edit produces one PUT request; item placements/removals
    /// enqueue server world actions so the running game owns item slot allocation.
    pub(super) fn save_to_api(&mut self) {
        self.save_status = None;
        if let Err(e) = self.sync_loaded_world_from_views() {
            self.save_status = Some(format!("Save failed: {e}"));
            return;
        }
        let Some(client) = self.admin_client.as_ref().cloned() else {
            self.save_status = Some("Admin client not initialized".to_owned());
            return;
        };

        let targets: Vec<(usize, usize)> = self.dirty_tiles.iter().copied().collect();
        let item_actions = self.pending_item_actions.clone();
        if targets.is_empty() && item_actions.is_empty() {
            self.save_status = Some("No changes to save".to_owned());
            self.mark_clean_if_no_pending_changes();
            return;
        }

        let mut pushed = 0usize;
        let mut errors: Vec<String> = Vec::new();

        for (x, y) in targets {
            let Some(tile) = self.map_tiles.get(tile_index(x, y)) else {
                errors.push(format!("({x},{y}): out of range"));
                continue;
            };
            let patch = MapPatch {
                x: x as u32,
                y: y as u32,
                sprite: tile.sprite,
                fsprite: tile.fsprite,
                flags: tile.flags,
            };
            match client.put_map_tile_patch(x, y, &patch) {
                Ok(_) => {
                    pushed += 1;
                    self.dirty_tiles.remove(&(x, y));
                }
                Err(e) => errors.push(format!("({x},{y}): {e}")),
            }
        }

        let mut succeeded = BTreeSet::new();
        for (idx, action) in item_actions.iter().enumerate() {
            let world_action = match *action {
                PendingItemAction::Place { x, y, template_id } => {
                    WorldActionKind::PlaceMapItemFromTemplate {
                        x,
                        y,
                        template_id: template_id as usize,
                    }
                }
                PendingItemAction::Clear { x, y } => WorldActionKind::ClearMapItem { x, y },
            };

            match client.request_world_action(&world_action) {
                Ok(_) => {
                    succeeded.insert(idx);
                }
                Err(e) => errors.push(format!("{}: {e}", world_action.name())),
            }
        }
        let queued_item_actions = succeeded.len();

        let mut action_idx = 0usize;
        self.pending_item_actions.retain(|_| {
            let keep = !succeeded.contains(&action_idx);
            action_idx += 1;
            keep
        });

        self.mark_clean_if_no_pending_changes();
        self.save_status = Some(if errors.is_empty() {
            format!(
                "Saved to API: {pushed} tile(s), queued {queued_item_actions} item action(s). Use 'Reload server map' to apply."
            )
        } else {
            format!(
                "Save partial: {pushed} tile(s), queued {queued_item_actions} item action(s); {} error(s): {}",
                errors.len(),
                errors.join("; ")
            )
        });
    }

    /// Open the modal dialog used to connect to the admin API.
    ///
    /// Pre-fills the form with the current LiveApi credentials when one is
    /// active, otherwise falls back to `MAG_API_BASE_URL` /
    /// `MAG_ADMIN_API_TOKEN` env vars, then to safe local-dev defaults.
    pub(super) fn open_connect_dialog(&mut self) {
        match &self.data_source {
            DataSource::LiveApi { base_url, token } => {
                self.connect_form_base_url = base_url.clone();
                self.connect_form_token = token.clone();
            }
            _ => {
                if self.connect_form_base_url.is_empty() {
                    self.connect_form_base_url = std::env::var("MAG_API_BASE_URL")
                        .unwrap_or_else(|_| "https://127.0.0.1:5554".to_owned());
                }
                if self.connect_form_token.is_empty() {
                    self.connect_form_token =
                        std::env::var("MAG_ADMIN_API_TOKEN").unwrap_or_default();
                }
            }
        }
        self.connect_dialog_error = None;
        self.connect_dialog_open = true;
    }

    /// Switch the data source to LiveApi using the connect-dialog form, then reload the world.
    fn connect_to_api_from_form(&mut self) {
        let base_url = self.connect_form_base_url.trim().to_owned();
        let token = self.connect_form_token.trim().to_owned();

        if base_url.is_empty() {
            self.connect_dialog_error = Some("Base URL is required".to_owned());
            return;
        }
        if token.is_empty() {
            self.connect_dialog_error = Some("Admin token is required".to_owned());
            return;
        }

        let client = match AdminClient::new(base_url.clone(), token.clone()) {
            Ok(c) => c,
            Err(e) => {
                self.connect_dialog_error = Some(format!("Build client failed: {e}"));
                return;
            }
        };

        self.admin_client = Some(client);
        self.data_source = DataSource::LiveApi { base_url, token };
        self.load_current_source();

        if let Some(err) = self.map_error.clone() {
            self.connect_dialog_error = Some(format!("Connection test failed: {err}"));
            self.admin_client = None;
            return;
        }

        self.connect_dialog_open = false;
        self.connect_dialog_error = None;
        self.save_status = Some("Connected to admin API".to_owned());
    }

    /// Fire a server-side map reload and remember the request id.
    fn request_server_map_reload(&mut self) {
        let Some(client) = self.admin_client.as_ref().cloned() else {
            self.save_status = Some("Admin client not initialized".to_owned());
            return;
        };
        match client.request_map_reload() {
            Ok(resp) => {
                self.save_status = Some(format!("Map reload requested ({})", resp.request_id));
                self.pending_map_reload_request_id = Some(resp.request_id);
                self.pending_reload_since = Some(Instant::now());
                self.last_reload_poll = None;
            }
            Err(e) => self.save_status = Some(format!("Map reload failed: {e}")),
        }
    }

    /// Forget the pending map-reload request.
    fn clear_pending_reload(&mut self) {
        self.pending_map_reload_request_id = None;
        self.pending_reload_since = None;
        self.last_reload_poll = None;
    }

    /// Poll the most recent map-reload request once (best effort).
    pub(super) fn poll_map_reload_status(&mut self) {
        let Some(request_id) = self.pending_map_reload_request_id.clone() else {
            return;
        };
        let Some(client) = self.admin_client.as_ref().cloned() else {
            return;
        };
        match client.map_reload_status(&request_id) {
            Ok(status) if status.status == "applied" => {
                self.clear_pending_reload();
                self.save_status = Some(format!("Map reload applied ({})", status.request_id));
            }
            Ok(status) => {
                self.save_status = Some(format!(
                    "Map reload status ({request_id}): {}",
                    status.status
                ));
            }
            Err(e) => self.save_status = Some(format!("Map reload status error: {e}")),
        }
    }

    /// Auto-poll a pending map reload every [`RELOAD_POLL_INTERVAL`], giving up after
    /// [`RELOAD_POLL_GIVE_UP`].
    ///
    /// # Arguments
    ///
    /// * `ctx` - egui context used to schedule the next poll repaint.
    pub(super) fn tick_map_reload_poll(&mut self, ctx: &egui::Context) {
        if self.pending_map_reload_request_id.is_none() {
            return;
        }

        let since_start = self
            .pending_reload_since
            .map_or(RELOAD_POLL_GIVE_UP, |t| t.elapsed());
        if since_start >= RELOAD_POLL_GIVE_UP {
            self.clear_pending_reload();
            self.save_status = Some("Map reload status: timed out waiting for server".to_owned());
            return;
        }

        let should_poll = self
            .last_reload_poll
            .is_none_or(|t| t.elapsed() >= RELOAD_POLL_INTERVAL);
        if should_poll {
            self.last_reload_poll = Some(Instant::now());
            self.poll_map_reload_status();
        }
        ctx.request_repaint_after(RELOAD_POLL_INTERVAL);
    }

    /// Render the modal dialog used to enter admin API connection details.
    ///
    /// # Arguments
    ///
    /// * `ctx` - egui context used to host the modal window.
    pub(super) fn render_connect_dialog(&mut self, ctx: &egui::Context) {
        if !self.connect_dialog_open {
            return;
        }

        let mut still_open = true;
        let mut apply_clicked = false;
        let mut cancel_clicked = false;

        egui::Window::new("Connect to Admin API")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .open(&mut still_open)
            .show(ctx, |ui| {
                ui.set_min_width(420.0);
                ui.label(
                    "Point the map viewer at a running API service. \
                     Use a local URL when developing, or your production URL.",
                );
                ui.add_space(6.0);

                egui::Grid::new("map_connect_dialog_grid")
                    .num_columns(2)
                    .spacing([8.0, 6.0])
                    .show(ui, |ui| {
                        ui.label("Base URL:");
                        ui.add(
                            egui::TextEdit::singleline(&mut self.connect_form_base_url)
                                .hint_text("https://127.0.0.1:5554")
                                .desired_width(280.0),
                        );
                        ui.end_row();

                        ui.label("Admin token:");
                        ui.horizontal(|ui| {
                            ui.add(
                                egui::TextEdit::singleline(&mut self.connect_form_token)
                                    .password(!self.connect_form_show_token)
                                    .hint_text("MAG_ADMIN_API_TOKEN")
                                    .desired_width(220.0),
                            );
                            ui.checkbox(&mut self.connect_form_show_token, "Show");
                        });
                        ui.end_row();
                    });

                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new(
                        "Defaults are read from MAG_API_BASE_URL and MAG_ADMIN_API_TOKEN.",
                    )
                    .small()
                    .weak(),
                );

                if let Some(err) = &self.connect_dialog_error {
                    ui.add_space(6.0);
                    ui.colored_label(egui::Color32::RED, err);
                }

                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    apply_clicked = ui.button("Connect").clicked();
                    cancel_clicked = ui.button("Cancel").clicked();
                });
            });

        if cancel_clicked || !still_open {
            self.connect_dialog_open = false;
            self.connect_dialog_error = None;
        } else if apply_clicked {
            self.connect_to_api_from_form();
        }
    }

    /// Render the confirmation modal for triggering a server-side map reload.
    ///
    /// # Arguments
    ///
    /// * `ctx` - egui context used to host the modal window.
    pub(super) fn render_reload_confirm_dialog(&mut self, ctx: &egui::Context) {
        if !self.reload_confirm_open {
            return;
        }

        let mut still_open = true;
        let mut confirm_clicked = false;
        let mut cancel_clicked = false;
        let has_unsaved = !self.dirty_tiles.is_empty() || !self.pending_item_actions.is_empty();

        egui::Window::new("Reload Server Map?")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .open(&mut still_open)
            .show(ctx, |ui| {
                ui.set_min_width(440.0);
                ui.colored_label(
                    egui::Color32::YELLOW,
                    "\u{26A0}  This will drain pending map patches on the running server.",
                );
                ui.add_space(6.0);
                ui.label(
                    "Existing players in the affected areas will see the new tiles on their \
                     next tick. Run this only after 'Save to API' has succeeded.",
                );

                if has_unsaved {
                    ui.add_space(6.0);
                    ui.colored_label(
                        egui::Color32::RED,
                        "You have unsaved local edits. Save to API first or they will not be reloaded.",
                    );
                }

                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    confirm_clicked = ui
                        .add(
                            egui::Button::new(
                                egui::RichText::new("Reload now").color(egui::Color32::WHITE),
                            )
                            .fill(egui::Color32::from_rgb(160, 60, 60)),
                        )
                        .clicked();
                    cancel_clicked = ui.button("Cancel").clicked();
                });
            });

        if cancel_clicked || !still_open {
            self.reload_confirm_open = false;
        } else if confirm_clicked {
            self.reload_confirm_open = false;
            self.request_server_map_reload();
        }
    }
}
