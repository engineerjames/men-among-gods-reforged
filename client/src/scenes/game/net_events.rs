use mag_core::skills;

use crate::{
    network::{client_commands::ClientCommand, NetworkEvent},
    scenes::scene::SceneType,
    state::AppState,
    ui::widget::{Widget, WidgetAction},
};

use super::{GameScene, MAX_TICK_GROUPS_PER_FRAME, QSIZE};

impl GameScene {
    /// Drains pending network events (bytes, ticks, status, errors) and applies
    /// them to the game state.
    ///
    /// Processes up to `MAX_TICK_GROUPS_PER_FRAME` complete tick groups per call
    /// to avoid starving the render loop.
    ///
    /// # Returns
    /// `Some(SceneType)` if the scene should change (e.g. on disconnect),
    /// `None` to stay in-game.
    pub(super) fn process_network_events(
        &mut self,
        app_state: &mut AppState<'_>,
    ) -> Option<SceneType> {
        let mut tick_groups_processed = 0usize;

        loop {
            if tick_groups_processed >= MAX_TICK_GROUPS_PER_FRAME {
                break;
            }

            let Some(net) = app_state.network.as_mut() else {
                break;
            };
            let Some(evt) = net.try_recv() else {
                break;
            };

            match evt {
                NetworkEvent::Status(msg) => {
                    log::info!("Network status: {}", msg);
                }
                NetworkEvent::Error(e) => {
                    log::error!("Network error: {}", e);
                    if let Some(mismatch) = crate::cert_trust::take_last_fingerprint_mismatch() {
                        self.certificate_mismatch = Some(mismatch);
                        self.pending_exit = None;
                        continue;
                    }
                    self.pending_exit = Some(e);
                }
                NetworkEvent::LoggedIn => {
                    if let Some(net) = app_state.network.as_mut() {
                        net.logged_in = true;
                    }
                    log::info!("Logged in to game server");
                }
                NetworkEvent::NewPlayerCredentials {
                    _user_id,
                    _pass1,
                    _pass2,
                } => {}
                NetworkEvent::Bytes { bytes, received_at } => {
                    if bytes.is_empty() {
                        continue;
                    }

                    use crate::network::server_commands::{ServerCommand, ServerCommandData};

                    if let Some(cmd) = ServerCommand::from_bytes(&bytes) {
                        match &cmd.structured_data {
                            ServerCommandData::Pong { seq, .. } => {
                                if let Some(net) = app_state.network.as_mut() {
                                    net.handle_pong(*seq, received_at);
                                }
                            }
                            ServerCommandData::PlaySound { nr, vol, pan } => {
                                log::info!("PlaySound: nr={} vol={} pan={}", nr, vol, pan);
                                app_state.sfx_cache.play_sfx(
                                    *nr as usize,
                                    *vol,
                                    *pan,
                                    app_state.master_volume,
                                );
                            }
                            ServerCommandData::Exit { reason } => {
                                log::info!("Received exit command from server: {}", reason);
                                if let Some(ps) = app_state.player_state.as_mut() {
                                    ps.update_from_server_command(&cmd);
                                }
                            }
                            _ => {
                                if let Some(ps) = app_state.player_state.as_mut() {
                                    ps.update_from_server_command(&cmd);
                                }
                            }
                        }
                    }
                }
                NetworkEvent::Tick => {
                    if let Some(net) = app_state.network.as_mut() {
                        net.client_ticker = net.client_ticker.wrapping_add(1);
                        let ticker = net.client_ticker;
                        if let Some(ps) = app_state.player_state.as_mut() {
                            ps.on_tick_packet(ticker);
                            ps.map_mut().reset_last_setmap_index();
                        }
                        net.maybe_send_ctick();
                        net.maybe_send_ping();
                    }
                    tick_groups_processed += 1;
                }
            }
        }

        if let Some(ps) = app_state.player_state.as_mut() {
            if ps.take_exit_requested_reason().is_some() {
                return Some(SceneType::CharacterSelection);
            }
        }

        if self.pending_exit.take().is_some() {
            return Some(SceneType::CharacterSelection);
        }

        None
    }

    /// Periodically sends auto-look commands (for nameplates) and shop refresh.
    ///
    /// Called once per server tick. Increments an internal step counter and fires
    /// `CL_CMD_AUTOLOOK` every `QSIZE * 3` steps for the first character whose
    /// name is not yet known.
    pub(super) fn maybe_send_autolook_and_shop_refresh(&mut self, app_state: &mut AppState<'_>) {
        let (Some(net), Some(ps)) = (app_state.network.as_ref(), app_state.player_state.as_ref())
        else {
            return;
        };

        self.look_step = self.look_step.saturating_add(1);

        // C engine.c: if (lookat && lookstep>QSIZE*3) cmd1s(CL_CMD_AUTOLOOK,lookat);
        if self.look_step > QSIZE * 3 {
            if let Some(lookat) = Self::find_unknown_look_target(ps) {
                net.send(ClientCommand::new_autolook(lookat));
            }
            self.look_step = 0;
        }
    }

    /// Drain pending `WidgetAction`s from the chat box and act on them.
    ///
    /// Currently the only action is `SendChat`, which sends say-packets
    /// through the network runtime.
    ///
    /// # Arguments
    ///
    /// * `app_state` - Shared application state (network access).
    pub(crate) fn process_chat_box_actions(&mut self, app_state: &AppState) {
        for action in self.chat_box.take_actions() {
            match action {
                WidgetAction::SendChat(text) => {
                    if let Some(net) = app_state.network.as_ref() {
                        for pkt in ClientCommand::new_say_packets(text.as_bytes()) {
                            net.send(pkt);
                        }
                    }
                }
                _ => {}
            }
        }
    }

    /// Drain pending `WidgetAction`s from the mode button and send mode
    /// commands to the server.
    ///
    /// # Arguments
    ///
    /// * `app_state` - Shared application state (network access).
    pub(crate) fn process_mode_button_actions(&mut self, app_state: &AppState) {
        for action in self.mode_button.take_actions() {
            if let WidgetAction::ChangeMode(mode) = action {
                if let Some(net) = app_state.network.as_ref() {
                    net.send(ClientCommand::new_mode(mode as i16));
                }
            }
        }
    }

    /// Drain and process actions produced by the skills panel.
    ///
    /// # Arguments
    ///
    /// * `app_state` - Shared application state (network access).
    pub(crate) fn process_skills_panel_actions(&mut self, app_state: &mut AppState<'_>) {
        for action in self.skills_panel.take_actions() {
            match action {
                WidgetAction::CommitStats { raises } => {
                    if let Some(net) = app_state.network.as_ref() {
                        for (which, value) in raises {
                            net.send(ClientCommand::new_stat(which, value));
                        }
                    }
                }
                WidgetAction::CastSkill { skill_nr } => {
                    if let (Some(net), Some(ps)) =
                        (app_state.network.as_ref(), app_state.player_state.as_ref())
                    {
                        let target = Self::default_skill_target(ps);
                        let a0 = ps.character_info().attrib[0][5] as u32;
                        net.send(ClientCommand::new_skill(skill_nr as u32, target, a0));
                    }
                }
                WidgetAction::BeginSkillAssign { skill_id } => {
                    self.pending_skill_assignment = Some(skill_id);
                }
                WidgetAction::BindSkillKey { skill_nr, key_slot } => {
                    if let Some(ps) = app_state.player_state.as_mut() {
                        // Clear any previous slot that had the same skill_nr.
                        for slot in ps.player_data_mut().skill_keybinds.iter_mut() {
                            if *slot == Some(skill_nr) {
                                *slot = None;
                            }
                        }
                        ps.player_data_mut().skill_keybinds[key_slot as usize] = Some(skill_nr);
                        let name = skills::get_skill_name(skill_nr);
                        ps.tlog(1, &format!("Bound {} to Ctrl+{}.", name, key_slot + 1));
                    }
                    self.save_active_profile(app_state);
                }
                WidgetAction::TogglePanel(_) => {
                    // Panel was closed via its title bar X button.
                    self.save_active_profile(app_state);
                }
                _ => {}
            }
        }
    }

    /// Drain pending `WidgetAction`s from the skill bar and send the
    /// corresponding network commands.
    ///
    /// Handles `CastSkill` (click bound slot), `BeginSkillAssign` (click
    /// empty slot — future popup), and `BindSkillKey` with `skill_nr == 0`
    /// (right-click to clear a slot).
    ///
    /// # Arguments
    ///
    /// * `app_state` - Shared application state.
    pub(crate) fn process_skill_bar_actions(&mut self, app_state: &mut AppState<'_>) {
        for action in self.skill_bar.take_actions() {
            match action {
                WidgetAction::CastSkill { skill_nr } => {
                    if let (Some(net), Some(ps)) =
                        (app_state.network.as_ref(), app_state.player_state.as_ref())
                    {
                        self.play_click_sound(app_state);
                        let target = Self::default_skill_target(ps);
                        let a0 = ps.character_info().attrib[0][5] as u32;
                        net.send(ClientCommand::new_skill(skill_nr as u32, target, a0));
                    }
                }
                WidgetAction::BeginSkillAssign { skill_id } => {
                    // Open the skill picker popup anchored above the clicked cell.
                    let bar = self.skill_bar.bounds();
                    let (cx, _cy) = crate::ui::skill_bar::TOP_CELL_POSITIONS
                        .get(skill_id)
                        .copied()
                        .unwrap_or((0, 0));
                    let anchor_x = bar.x + cx;
                    let anchor_y = bar.y - 200; // above the skill bar
                    self.skill_picker.show(skill_id as u8, anchor_x, anchor_y);
                }
                WidgetAction::BindSkillKey {
                    skill_nr: 0,
                    key_slot,
                } => {
                    // Clear (unbind) the slot.
                    if let Some(ps) = app_state.player_state.as_mut() {
                        let slot = key_slot as usize;
                        if slot < ps.player_data().skill_keybinds.len() {
                            ps.player_data_mut().skill_keybinds[slot] = None;
                        }
                    }
                    self.save_active_profile(app_state);
                }
                WidgetAction::BindSkillKey { skill_nr, key_slot } => {
                    if let Some(ps) = app_state.player_state.as_mut() {
                        for slot in ps.player_data_mut().skill_keybinds.iter_mut() {
                            if *slot == Some(skill_nr) {
                                *slot = None;
                            }
                        }
                        ps.player_data_mut().skill_keybinds[key_slot as usize] = Some(skill_nr);
                    }
                    self.save_active_profile(app_state);
                }
                _ => {}
            }
        }
    }

    /// Drain pending [`WidgetAction`]s from the skill picker popup.
    ///
    /// A `BindSkillKey` action produced by the popup binds the chosen skill
    /// to the target slot and saves the profile.
    ///
    /// # Arguments
    ///
    /// * `app_state` - Shared application state.
    pub(crate) fn process_skill_picker_actions(&mut self, app_state: &mut AppState<'_>) {
        for action in self.skill_picker.take_actions() {
            match action {
                WidgetAction::BindSkillKey { skill_nr, key_slot } => {
                    if let Some(ps) = app_state.player_state.as_mut() {
                        // Clear any previous slot that had the same skill_nr.
                        for slot in ps.player_data_mut().skill_keybinds.iter_mut() {
                            if *slot == Some(skill_nr) {
                                *slot = None;
                            }
                        }
                        ps.player_data_mut().skill_keybinds[key_slot as usize] = Some(skill_nr);
                        let name = skills::get_skill_name(skill_nr);
                        ps.tlog(1, &format!("Bound {} to Ctrl+{}.", name, key_slot + 1));
                    }
                    self.save_active_profile(app_state);
                }
                _ => {}
            }
        }
    }

    /// Drain pending `WidgetAction`s from the inventory panel and send the
    /// corresponding network commands.
    ///
    /// # Arguments
    ///
    /// * `app_state` - Shared application state (network access).
    pub(crate) fn process_inventory_panel_actions(&mut self, app_state: &AppState) {
        for action in self.inventory_panel.take_actions() {
            match action {
                WidgetAction::InvAction {
                    a,
                    b,
                    selected_char,
                } => {
                    if let Some(net) = app_state.network.as_ref() {
                        self.play_click_sound(app_state);
                        // Sanitize: if the selected character is the player
                        // themselves, send 0 so server-side item spells use the
                        // correct self-cast path.
                        let target = app_state
                            .player_state
                            .as_ref()
                            .map(|ps| {
                                let self_cn = GameScene::own_ch_nr(ps);
                                if selected_char != 0 && selected_char == self_cn {
                                    0
                                } else {
                                    selected_char
                                }
                            })
                            .unwrap_or(selected_char);
                        net.send(ClientCommand::new_inv(a, b, target));
                    }
                }
                WidgetAction::InvLookAction { a, b, c } => {
                    if let Some(net) = app_state.network.as_ref() {
                        self.play_click_sound(app_state);
                        net.send(ClientCommand::new_inv_look(a, b, c));
                    }
                }
                WidgetAction::TogglePanel(_) => {
                    // Panel was closed via its title bar X button.
                    self.save_active_profile(app_state);
                }
                _ => {}
            }
        }
    }

    /// Drain pending `WidgetAction`s from the shop panel and send the
    /// corresponding network commands, or close the shop.
    ///
    /// # Arguments
    ///
    /// * `app_state` - Shared application state (network + player state).
    pub(crate) fn process_shop_panel_actions(&mut self, app_state: &mut AppState<'_>) {
        for action in self.shop_panel.take_actions() {
            match action {
                WidgetAction::ShopAction { shop_nr, action } => {
                    if let Some(net) = app_state.network.as_ref() {
                        self.play_click_sound(app_state);
                        net.send(ClientCommand::new_shop(shop_nr, action));
                    }
                }
                WidgetAction::CloseShop => {
                    if let Some(ps) = app_state.player_state.as_mut() {
                        ps.close_shop();
                    }
                }
                _ => {}
            }
        }
    }
}
