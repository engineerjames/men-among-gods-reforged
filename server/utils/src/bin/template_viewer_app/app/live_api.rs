//! Live admin-API integration: connect dialog, saving dirty slots, and server reloads.

use super::TemplateViewerApp;
use eframe::egui;
use server_utils::{AdminClient, DataSource};
use std::collections::HashSet;
use std::time::{Duration, Instant};

/// How often a pending reload request is polled.
const RELOAD_POLL_INTERVAL: Duration = Duration::from_secs(2);
/// Stop polling after this long, matching the server-side status TTL.
const RELOAD_POLL_GIVE_UP: Duration = Duration::from_secs(60);

/// PUT every slot in `dirty`, removing slots that were pushed (or no longer exist).
///
/// # Arguments
///
/// * `dirty` - Dirty slot set for one table.
/// * `label` - Prefix for error messages, e.g. `item`.
/// * `errors` - Collected `label[idx]: error` messages.
/// * `push` - Sends one slot; `None` when the slot is out of range.
///
/// # Returns
///
/// * Number of slots pushed successfully.
fn push_dirty_slots(
    dirty: &mut HashSet<usize>,
    label: &str,
    errors: &mut Vec<String>,
    mut push: impl FnMut(usize) -> Option<Result<(), String>>,
) -> usize {
    let mut slots: Vec<usize> = dirty.iter().copied().collect();
    slots.sort_unstable();
    let mut pushed = 0;
    for idx in slots {
        match push(idx) {
            Some(Err(e)) => errors.push(format!("{label}[{idx}]: {e}")),
            Some(Ok(())) => {
                pushed += 1;
                dirty.remove(&idx);
            }
            None => {
                dirty.remove(&idx);
            }
        }
    }
    pushed
}

impl TemplateViewerApp {
    /// Push every dirty slot to the admin API. Called instead of snapshot save in LiveApi mode.
    ///
    /// Successfully pushed slots are cleared, so a retry only resends failures.
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

        let mut errors: Vec<String> = Vec::new();
        let item_pushed = push_dirty_slots(
            &mut self.dirty_item_template_slots,
            "item",
            &mut errors,
            |idx| {
                self.item_templates
                    .get(idx)
                    .map(|t| client.put_item_template(idx, t))
            },
        );
        let char_pushed = push_dirty_slots(
            &mut self.dirty_character_template_slots,
            "char",
            &mut errors,
            |idx| {
                self.character_templates
                    .get(idx)
                    .map(|t| client.put_character_template(idx, t))
            },
        );
        let item_inst_pushed = push_dirty_slots(
            &mut self.dirty_item_slots,
            "item_inst",
            &mut errors,
            |idx| {
                self.items.get(idx).map(|item| {
                    let patch = mag_core::item_store::ItemPatch::from_item(idx, item);
                    client.put_item_patch(idx, &patch).map(|_| ())
                })
            },
        );
        let char_inst_pushed = push_dirty_slots(
            &mut self.dirty_character_slots,
            "char_inst",
            &mut errors,
            |idx| {
                self.characters.get(idx).map(|ch| {
                    let patch = mag_core::character_store::CharacterPatch::from_character(idx, ch);
                    client.put_character_patch(idx, &patch).map(|_| ())
                })
            },
        );

        if errors.is_empty() {
            self.clear_dirty();
            self.save_status = Some(format!(
                "Saved to API: {item_pushed} item template(s), {char_pushed} character template(s), {item_inst_pushed} item(s), {char_inst_pushed} character(s). Use 'Reload server templates' to apply."
            ));
        } else {
            self.save_status = Some(format!(
                "Save partial: {item_pushed} item tpl, {char_pushed} char tpl, {item_inst_pushed} items, {char_inst_pushed} chars; {} error(s): {}",
                errors.len(),
                errors.join("; ")
            ));
        }
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

    /// Switch to LiveApi using the connect-dialog form and reload the world.
    ///
    /// Closes the dialog only on success; on failure leaves it open with an
    /// inline error so the user can correct the URL/token.
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

        if let Some(err) = self.load_error.clone() {
            self.connect_dialog_error = Some(format!("Connection test failed: {err}"));
            self.admin_client = None;
            return;
        }

        self.connect_dialog_open = false;
        self.connect_dialog_error = None;
        self.save_status = Some("Connected to admin API".to_owned());
    }

    /// Ask the running server to reload templates, items, and characters.
    ///
    /// Status display tracks the templates request id.
    fn request_server_reload(&mut self) {
        let Some(client) = self.admin_client.as_ref().cloned() else {
            self.save_status = Some("Admin client not initialized".to_owned());
            return;
        };

        let extra = [
            ("items", client.request_items_reload().err()),
            ("characters", client.request_characters_reload().err()),
        ]
        .map(|(kind, err)| match err {
            Some(e) => format!("{kind} reload failed: {e}"),
            None => kind.to_owned(),
        });

        match client.request_reload(true, true) {
            Ok(resp) => {
                self.save_status = Some(format!(
                    "Reload requested ({}): kinds=[{}] + [{}]",
                    resp.request_id,
                    resp.kinds.join(", "),
                    extra.join(", ")
                ));
                self.pending_reload_request_id = Some(resp.request_id);
                self.pending_reload_since = Some(Instant::now());
            }
            Err(e) => self.save_status = Some(format!("Reload failed: {e}")),
        }
    }

    /// Forget the pending reload request.
    fn clear_pending_reload(&mut self) {
        self.pending_reload_request_id = None;
        self.pending_reload_since = None;
        self.last_reload_poll = None;
    }

    /// Poll the most recent reload request once (best effort).
    pub(super) fn poll_reload_status(&mut self) {
        let Some(request_id) = self.pending_reload_request_id.clone() else {
            return;
        };
        let Some(client) = self.admin_client.as_ref().cloned() else {
            return;
        };
        match client.reload_status(&request_id) {
            Ok(status) if status.status == "applied" => {
                self.pending_reload_request_id = None;
                self.pending_reload_since = None;
                self.save_status = Some(format!("Reload applied ({})", status.request_id));
            }
            Ok(status) => {
                self.save_status = Some(format!("Reload status ({request_id}): {}", status.status));
            }
            Err(e) => self.save_status = Some(format!("Reload status error: {e}")),
        }
    }

    /// Auto-poll a pending reload every [`RELOAD_POLL_INTERVAL`] until applied or
    /// [`RELOAD_POLL_GIVE_UP`] elapses.
    ///
    /// # Arguments
    ///
    /// * `ctx` - egui context used to schedule the next poll repaint.
    pub(super) fn tick_reload_poll(&mut self, ctx: &egui::Context) {
        if self.pending_reload_request_id.is_none() {
            return;
        }

        let since_start = self
            .pending_reload_since
            .map_or(RELOAD_POLL_GIVE_UP, |t| t.elapsed());
        if since_start >= RELOAD_POLL_GIVE_UP {
            self.clear_pending_reload();
            self.save_status = Some("Reload status: timed out waiting for server".to_owned());
            return;
        }

        if self
            .last_reload_poll
            .is_none_or(|t| t.elapsed() >= RELOAD_POLL_INTERVAL)
        {
            self.last_reload_poll = Some(Instant::now());
            self.poll_reload_status();
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
                    "Point the template viewer at a running API service. \
                     Use a local URL when developing, or your production URL.",
                );
                ui.add_space(6.0);

                egui::Grid::new("connect_dialog_grid")
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

    /// Render the confirmation modal for a server-side template reload.
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
        let has_unsaved = self.dirty;

        egui::Window::new("Reload Server Templates?")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .open(&mut still_open)
            .show(ctx, |ui| {
                ui.set_min_width(440.0);
                ui.colored_label(
                    egui::Color32::YELLOW,
                    "\u{26A0}  This will swap the running server's in-memory template tables.",
                );
                ui.add_space(6.0);
                ui.label(
                    "Existing entities and ongoing player actions may be affected. \
                     Run this only when you have just pushed template edits and \
                     are ready to apply them live.",
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
            self.request_server_reload();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::push_dirty_slots;
    use std::collections::HashSet;

    #[test]
    fn push_dirty_slots_keeps_only_failed_slots() {
        let mut dirty: HashSet<usize> = [1, 2, 3, 99].into_iter().collect();
        let mut errors = Vec::new();
        let pushed = push_dirty_slots(&mut dirty, "item", &mut errors, |idx| match idx {
            99 => None,
            2 => Some(Err("boom".to_owned())),
            _ => Some(Ok(())),
        });

        assert_eq!(pushed, 2);
        assert_eq!(dirty, [2].into_iter().collect());
        assert_eq!(errors, vec!["item[2]: boom".to_owned()]);
    }
}
