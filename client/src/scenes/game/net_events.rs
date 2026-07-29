use mag_core::client_commands::ClientCommand;
use mag_core::constants::{IS_GRAVE, TILEX, TILEY};
use mag_core::server_commands::{ServerCommand, ServerCommandData};
use mag_core::skills;

use crate::{
    cert_trust,
    network::{NetworkEvent, ServerTickBatch},
    scenes::scene::SceneType,
    state::AppState,
    ui::{
        forms::cert_dialog::CertDialogAction,
        widget::UiEvent,
        widget::{HudPanel, Widget, WidgetAction},
    },
};

use super::{GameScene, QSIZE};

/// Result of routing a [`UiEvent`] through the widget stack.
///
/// Distinguishes "a widget consumed the event" from "no widget cared" so
/// callers can decide whether to fall through to world-level input handlers.
pub(super) enum UiHandleResult {
    /// A widget triggered a scene transition.
    SceneChange(SceneType),
    /// A widget consumed the event (no scene change).
    Consumed,
    /// No widget consumed the event.
    NotConsumed,
}

impl GameScene {
    /// Applies one decoded server command, preserving its position within the
    /// enclosing tick batch.
    fn apply_server_command(
        &mut self,
        app_state: &mut AppState<'_>,
        cmd: &ServerCommand,
        received_at: std::time::Instant,
    ) {
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
                    app_state.settings.master_volume,
                );
            }
            ServerCommandData::SetWeather {
                kind,
                intensity,
                duration_ticks,
                tint,
                flags,
            } => {
                log::info!(
                    "SetWeather: kind={} intensity={} dur={} flags={:08b}",
                    kind,
                    intensity,
                    duration_ticks,
                    flags
                );
                self.weather
                    .apply_packet(*kind, *intensity, *duration_ticks, *tint, *flags);
            }
            ServerCommandData::Exit { reason } => {
                log::info!("Received exit command from server: {}", reason);
                if let Some(ps) = app_state.player_state.as_mut() {
                    ps.update_from_server_command(cmd);
                }
            }
            _ => {
                if let Some(ps) = app_state.player_state.as_mut() {
                    ps.update_from_server_command(cmd);
                }
            }
        }
    }

    /// Applies and simulates one complete framed server tick packet.
    pub(super) fn apply_server_tick_batch(
        &mut self,
        app_state: &mut AppState<'_>,
        batch: ServerTickBatch,
    ) {
        if let Some(ps) = app_state.player_state.as_mut() {
            ps.map_mut().reset_last_setmap_index();
        }

        for bytes in batch.commands {
            if let Some(cmd) = ServerCommand::from_bytes(&bytes) {
                self.apply_server_command(app_state, &cmd, batch.received_at);
            }
        }

        if let Some(net) = app_state.network.as_mut() {
            net.client_ticker = net.client_ticker.wrapping_add(1);
            net.maybe_send_ping();
        }

        self.sim_ticker = self.sim_ticker.wrapping_add(1);
        if let Some(ps) = app_state.player_state.as_mut() {
            ps.advance_local_simulation_tick(self.sim_ticker);
        }
        if let Some(net) = app_state.network.as_mut() {
            net.maybe_send_ctick(self.sim_ticker);
        }
    }

    /// Drains pending network events, queuing complete tick batches for the
    /// gameplay scheduler and handling out-of-band status events immediately.
    ///
    /// # Returns
    /// `Some(SceneType)` if the scene should change (e.g. on disconnect),
    /// `None` to stay in-game.
    pub(super) fn process_network_events(
        &mut self,
        app_state: &mut AppState<'_>,
    ) -> Option<SceneType> {
        while let Some(net) = app_state.network.as_mut() {
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
                NetworkEvent::TickBatch(batch) => {
                    self.pending_tick_batches.push_back(batch);
                }
            }
        }

        if let Some(ps) = app_state.player_state.as_mut()
            && ps.take_exit_requested_reason().is_some()
        {
            return Some(SceneType::CharacterSelection);
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
            if let Some(lookat) = Self::find_unknown_look_target(
                ps,
                app_state.settings.show_names,
                app_state.settings.show_proz,
            ) {
                net.send(ClientCommand::new_autolook(lookat));
            }
            self.look_step = 0;
        }
    }

    /// Sends a `CmdAutoloot` for each unvisited grave tile adjacent to the
    /// player center, when the auto-loot feature is enabled.
    ///
    /// Called once per server tick. Scans tiles within Chebyshev distance 1
    /// of the player's position (the always-at-center tile `(TILEX/2, TILEY/2)`)
    /// for any tile with [`IS_GRAVE`] set.  At most one command is issued per
    /// tick; once a world coordinate is recorded in `autoloot_visited`, it is
    /// never retried until the scene is re-entered.
    ///
    /// # Arguments
    ///
    /// * `app_state` - Shared application state (settings, network, player state).
    pub(super) fn maybe_send_autoloot_graves(&mut self, app_state: &mut AppState<'_>) {
        if !app_state.settings.character.auto_loot_graves {
            return;
        }
        let (Some(net), Some(ps)) = (app_state.network.as_ref(), app_state.player_state.as_ref())
        else {
            return;
        };

        // TODO: Need some kind of cache invalidator for `autoloot_visited` in case the player moves or new graves spawn.
        const CX: usize = TILEX / 2;
        const CY: usize = TILEY / 2;

        // Scan the 3×3 grid of tiles adjacent to (and including) the player tile.
        'outer: for dy in -1i32..=1 {
            for dx in -1i32..=1 {
                let tx = (CX as i32 + dx) as usize;
                let ty = (CY as i32 + dy) as usize;
                let Some(tile) = ps.map().tile_at_xy(tx, ty) else {
                    continue;
                };
                if (tile.flags & IS_GRAVE) == 0 {
                    continue;
                }
                let key = (tile.x, tile.y);
                if self.autoloot_visited.contains(&key) {
                    continue;
                }
                net.send(ClientCommand::new_autoloot_graves(
                    tile.x as i16,
                    i32::from(tile.y),
                ));
                self.autoloot_visited.insert(key);
                // One command per tick to avoid flooding the server.
                break 'outer;
            }
        }
    }

    /// Drain pending `WidgetAction`s from the chat box and act on them.
    ///
    /// Intercepts the `/autoloot` command client-side: toggles per-character
    /// auto-loot and prints a confirmation to the chat log without sending
    /// anything to the server.  All other text is forwarded as say-packets.
    ///
    /// # Arguments
    ///
    /// * `app_state` - Shared application state (network + settings access).
    pub(crate) fn process_chat_box_actions(&mut self, app_state: &mut AppState) {
        for action in self.chat_box.take_actions() {
            if let WidgetAction::SendChat(text) = action {
                if text.trim().eq_ignore_ascii_case("/autoloot") {
                    app_state.settings.character.auto_loot_graves =
                        !app_state.settings.character.auto_loot_graves;
                    let status = if app_state.settings.character.auto_loot_graves {
                        "enabled"
                    } else {
                        "disabled"
                    };
                    if let Some(ps) = app_state.player_state.as_mut() {
                        ps.tlog(1, format!("Auto-loot graves: {status}."));
                    }
                    self.save_active_profile(app_state);
                    continue;
                }
                if let Some(net) = app_state.network.as_ref() {
                    for pkt in ClientCommand::new_say_packets(text.as_bytes()) {
                        net.send(pkt);
                    }
                }
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
            if let WidgetAction::ChangeMode(mode) = action
                && let Some(net) = app_state.network.as_ref()
            {
                net.send(ClientCommand::new_mode(mode as i16));
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
                        let target = Self::default_skill_target(ps, skill_nr as u32);
                        let a0 = u32::from(ps.character_info().attrib[0][5]);
                        net.send(ClientCommand::new_skill(skill_nr as u32, target, a0));
                    }
                }
                WidgetAction::BeginSkillAssign { skill_id } => {
                    self.pending_skill_assignment = Some(skill_id);
                }
                WidgetAction::BindSkillKey { skill_nr, key_slot } => {
                    use crate::ui::hud::skill_bar::TOP_CELLS;
                    let slot = key_slot as usize;
                    if slot >= TOP_CELLS {
                        // Secondary bar slot.
                        let sec_slot = slot - TOP_CELLS;
                        for s in app_state
                            .settings
                            .character
                            .skill_keybinds_secondary
                            .iter_mut()
                        {
                            if *s == Some(skill_nr) {
                                *s = None;
                            }
                        }
                        app_state.settings.character.skill_keybinds_secondary[sec_slot] =
                            Some(skill_nr);
                    } else {
                        // Primary bar slot — clear any previous slot with the same skill_nr.
                        for s in app_state.settings.character.skill_keybinds.iter_mut() {
                            if *s == Some(skill_nr) {
                                *s = None;
                            }
                        }
                        app_state.settings.character.skill_keybinds[slot] = Some(skill_nr);
                    }
                    if let Some(ps) = app_state.player_state.as_mut() {
                        let name = skills::get_skill_name(skill_nr);
                        ps.tlog(1, format!("Bound {} to key {}.", name, key_slot + 1));
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
                        let target = Self::default_skill_target(ps, skill_nr as u32);
                        let a0 = u32::from(ps.character_info().attrib[0][5]);
                        net.send(ClientCommand::new_skill(skill_nr as u32, target, a0));
                    }
                }
                WidgetAction::BeginSkillAssign { skill_id } => {
                    // Open the skill picker popup anchored above the clicked cell.
                    // When skill_id >= TOP_CELLS (secondary bar), still anchor visually
                    // to the corresponding primary position (same column).
                    use crate::ui::hud::skill_bar::TOP_CELLS;
                    let bar = self.skill_bar.bounds();
                    let visual_slot = skill_id.min(TOP_CELLS - 1);
                    let (cx, _cy) = crate::ui::hud::skill_bar::TOP_CELL_POSITIONS
                        .get(visual_slot)
                        .copied()
                        .unwrap_or((0, 0));
                    let anchor_x = bar.x + cx;
                    let anchor_y = bar.y + crate::ui::hud::skill_picker_popup::ANCHOR_Y_OFFSET; // above the skill bar
                    let player_skills = app_state
                        .player_state
                        .as_ref()
                        .map(|ps| ps.character_info().skill.as_slice())
                        .unwrap_or(&[]);
                    self.skill_picker
                        .show(skill_id as u8, anchor_x, anchor_y, player_skills);
                }
                WidgetAction::BindSkillKey {
                    skill_nr: 0,
                    key_slot,
                } => {
                    // Clear (unbind) the slot.
                    use crate::ui::hud::skill_bar::TOP_CELLS;
                    let slot = key_slot as usize;
                    if slot >= TOP_CELLS {
                        let sec_slot = slot - TOP_CELLS;
                        if sec_slot < app_state.settings.character.skill_keybinds_secondary.len() {
                            app_state.settings.character.skill_keybinds_secondary[sec_slot] = None;
                        }
                    } else if slot < app_state.settings.character.skill_keybinds.len() {
                        app_state.settings.character.skill_keybinds[slot] = None;
                    }
                    self.save_active_profile(app_state);
                }
                WidgetAction::BindSkillKey { skill_nr, key_slot } => {
                    use crate::ui::hud::skill_bar::TOP_CELLS;
                    let slot = key_slot as usize;
                    if slot >= TOP_CELLS {
                        let sec_slot = slot - TOP_CELLS;
                        for s in app_state
                            .settings
                            .character
                            .skill_keybinds_secondary
                            .iter_mut()
                        {
                            if *s == Some(skill_nr) {
                                *s = None;
                            }
                        }
                        app_state.settings.character.skill_keybinds_secondary[sec_slot] =
                            Some(skill_nr);
                    } else {
                        for s in app_state.settings.character.skill_keybinds.iter_mut() {
                            if *s == Some(skill_nr) {
                                *s = None;
                            }
                        }
                        app_state.settings.character.skill_keybinds[slot] = Some(skill_nr);
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
            if let WidgetAction::BindSkillKey { skill_nr, key_slot } = action {
                use crate::ui::hud::skill_bar::TOP_CELLS;
                let slot = key_slot as usize;
                if slot >= TOP_CELLS {
                    // Secondary bar slot.
                    let sec_slot = slot - TOP_CELLS;
                    for s in app_state
                        .settings
                        .character
                        .skill_keybinds_secondary
                        .iter_mut()
                    {
                        if *s == Some(skill_nr) {
                            *s = None;
                        }
                    }
                    app_state.settings.character.skill_keybinds_secondary[sec_slot] =
                        Some(skill_nr);
                } else {
                    // Primary bar slot — clear any previous slot with the same skill_nr.
                    for s in app_state.settings.character.skill_keybinds.iter_mut() {
                        if *s == Some(skill_nr) {
                            *s = None;
                        }
                    }
                    app_state.settings.character.skill_keybinds[slot] = Some(skill_nr);
                }
                if let Some(ps) = app_state.player_state.as_mut() {
                    let name = skills::get_skill_name(skill_nr);
                    ps.tlog(1, format!("Bound {} to key {}.", name, key_slot + 1));
                }
                self.save_active_profile(app_state);
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

    /// Drain pending `WidgetAction`s from the talent panel and forward
    /// `LearnTalent` / `ResetTalents` commands to the server.
    ///
    /// # Arguments
    ///
    /// * `app_state` - Shared application state (network access).
    pub(crate) fn process_talent_panel_actions(&mut self, app_state: &mut AppState<'_>) {
        for action in self.talent_panel.take_actions() {
            match action {
                WidgetAction::LearnTalent { slot } => {
                    if let Some(net) = app_state.network.as_ref() {
                        self.play_click_sound(app_state);
                        net.send(ClientCommand::new_learn_talent(slot));
                    }
                }
                WidgetAction::ResetTalents => {
                    if let Some(net) = app_state.network.as_ref() {
                        self.play_click_sound(app_state);
                        net.send(ClientCommand::new_reset_talents());
                    }
                }
                WidgetAction::TogglePanel(_) => {
                    // Panel was closed via its title bar X button.
                }
                _ => {}
            }
        }
    }

    /// Drain pending `WidgetAction`s from the quest log panel and apply
    /// `SetActiveQuest` selections locally (pure UI state).
    ///
    /// # Arguments
    ///
    /// * `app_state` - Shared application state.
    pub(crate) fn process_quest_log_panel_actions(&mut self, app_state: &mut AppState<'_>) {
        for action in self.quest_log_panel.take_actions() {
            match action {
                WidgetAction::SetActiveQuest { npc_template_id } => {
                    self.play_click_sound(app_state);
                    if let Some(ps) = app_state.player_state.as_mut() {
                        ps.set_active_quest(npc_template_id);
                    }
                }
                WidgetAction::TogglePanel(_) => {
                    // Panel was closed via its title bar X button.
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

                        // Depot actions do not always push an immediate full refresh,
                        // so request one explicitly to keep the open panel in sync.
                        if ((shop_nr as u16) & 0x8000) != 0 {
                            net.send(ClientCommand::new_look(u32::from(shop_nr as u16)));
                        }
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

    /// Dispatch a [`UiEvent`] through the movable HUD panels (skills,
    /// inventory, settings, talents, quest log).
    ///
    /// These panels are rendered on top of the chat box, so pointer events are
    /// offered to them before the chat box gets a chance to swallow clicks that
    /// land inside its bounds.
    ///
    /// # Arguments
    ///
    /// * `app_state` - Shared application state (network + player state).
    /// * `ui_event` - The already-converted [`UiEvent`] to dispatch.
    ///
    /// # Returns
    ///
    /// * `Some(result)` if a panel consumed the event or triggered a scene
    ///   change, otherwise `None`.
    fn dispatch_hud_panel_events(
        &mut self,
        app_state: &mut AppState<'_>,
        ui_event: &UiEvent,
    ) -> Option<UiHandleResult> {
        if self.skills_panel.handle_event(ui_event) == crate::ui::widget::EventResponse::Consumed {
            self.process_skills_panel_actions(app_state);
            return Some(UiHandleResult::Consumed);
        }
        if self.inventory_panel.handle_event(ui_event) == crate::ui::widget::EventResponse::Consumed
        {
            self.process_inventory_panel_actions(app_state);
            return Some(UiHandleResult::Consumed);
        }
        if self.settings_panel.handle_event(ui_event) == crate::ui::widget::EventResponse::Consumed
        {
            if let Some(sc) = self.process_settings_panel_actions(app_state) {
                return Some(UiHandleResult::SceneChange(sc));
            }
            return Some(UiHandleResult::Consumed);
        }
        if self.talent_panel.handle_event(ui_event) == crate::ui::widget::EventResponse::Consumed {
            self.process_talent_panel_actions(app_state);
            return Some(UiHandleResult::Consumed);
        }
        if self.quest_log_panel.handle_event(ui_event) == crate::ui::widget::EventResponse::Consumed
        {
            self.process_quest_log_panel_actions(app_state);
            return Some(UiHandleResult::Consumed);
        }

        None
    }

    /// Dispatch a pre-converted [`UiEvent`] through the full HUD widget stack.
    ///
    /// This method encapsulates _Block 3_ from `handle_event`: the priority-
    /// ordered chain of widget `handle_event` calls that runs after the SDL2
    /// event has been converted into a UI-level event.  It is called once per
    /// frame event through the thin wrapper in `handle_event`.
    ///
    /// # Arguments
    ///
    /// * `app_state` - Shared application state (network + player state).
    /// * `ui_event` - The already-converted [`UiEvent`] to dispatch.
    ///
    /// # Returns
    ///
    /// * [`UiHandleResult::SceneChange`] if a widget triggers a scene change.
    /// * [`UiHandleResult::Consumed`] if a widget consumed the event.
    /// * [`UiHandleResult::NotConsumed`] if no widget handled the event.
    pub(super) fn handle_ui_widget_events(
        &mut self,
        app_state: &mut AppState<'_>,
        ui_event: &UiEvent,
    ) -> UiHandleResult {
        // --- Certificate mismatch dialog (modal, blocks all other input) ---
        if let Some(ref mut dialog) = self.cert_dialog {
            dialog.handle_event(ui_event);
            for action in dialog.take_cert_actions() {
                match action {
                    CertDialogAction::Accept => {
                        if let Some(mismatch) = self.certificate_mismatch.take() {
                            match cert_trust::trust_fingerprint(
                                &mismatch.host,
                                &mismatch.received_fingerprint,
                            ) {
                                Ok(()) => {
                                    self.cert_dialog = None;
                                    if let Err(err) = self.start_game_network_session(app_state) {
                                        self.pending_exit = Some(err);
                                        return UiHandleResult::SceneChange(
                                            SceneType::CharacterSelection,
                                        );
                                    }
                                    return UiHandleResult::Consumed;
                                }
                                Err(err) => {
                                    self.cert_dialog = None;
                                    self.pending_exit =
                                        Some(format!("Failed to update known hosts: {err}"));
                                    return UiHandleResult::SceneChange(
                                        SceneType::CharacterSelection,
                                    );
                                }
                            }
                        }
                    }
                    CertDialogAction::Reject => {
                        self.certificate_mismatch = None;
                        self.cert_dialog = None;
                        return UiHandleResult::SceneChange(SceneType::CharacterSelection);
                    }
                }
            }
            return UiHandleResult::Consumed;
        }

        // --- Skill picker popup (modal — must come before skill bar) ---
        if self.skill_picker.handle_event(ui_event) == crate::ui::widget::EventResponse::Consumed {
            self.process_skill_picker_actions(app_state);
            return UiHandleResult::Consumed;
        }

        // --- Rank sigil (upper-left) ---
        if self.rank_sigil.handle_event(ui_event) == crate::ui::widget::EventResponse::Consumed {
            return UiHandleResult::Consumed;
        }

        self.rank_progress_line.handle_event(ui_event);

        self.vitality_bars.handle_event(ui_event);
        self.spell_effect_icons.handle_event(ui_event);

        // --- Dispatch to shop/depot/grave overlay (modal — eats outside clicks) ---
        if self.shop_panel.handle_event(ui_event) == crate::ui::widget::EventResponse::Consumed {
            self.process_shop_panel_actions(app_state);
            return UiHandleResult::Consumed;
        }

        // --- StatusPanel (WV/AV display, right of skill bar) ---
        if self.weapon_armor_panel.handle_event(ui_event)
            == crate::ui::widget::EventResponse::Consumed
        {
            return UiHandleResult::Consumed;
        }

        // Movable HUD panels render above the chat box, so pointer input has to
        // reach them first; otherwise the chat box eats clicks (including a
        // panel's close button) wherever the two overlap.
        let pointer_event = matches!(
            ui_event,
            UiEvent::MouseDown { .. }
                | UiEvent::MouseClick { .. }
                | UiEvent::MouseWheel { .. }
                | UiEvent::MouseMove { .. }
        );
        if pointer_event && let Some(result) = self.dispatch_hud_panel_events(app_state, ui_event) {
            return result;
        }

        if self.chat_box.handle_event(ui_event) == crate::ui::widget::EventResponse::Consumed {
            self.process_chat_box_actions(app_state);
            return UiHandleResult::Consumed;
        }

        // --- Dispatch to open HUD panels (eat clicks so they don't reach the world) ---
        if !pointer_event && let Some(result) = self.dispatch_hud_panel_events(app_state, ui_event)
        {
            return result;
        }

        // --- Dispatch to minimap toggle button / panel ---
        if self.minimap_widget.handle_event(ui_event) == crate::ui::widget::EventResponse::Consumed
        {
            return UiHandleResult::Consumed;
        }

        // --- Dispatch to mode button ---
        if self.mode_button.handle_event(ui_event) == crate::ui::widget::EventResponse::Consumed {
            self.process_mode_button_actions(app_state);
            return UiHandleResult::Consumed;
        }

        // --- Dispatch to look panel ---
        if self.look_panel.handle_event(ui_event) == crate::ui::widget::EventResponse::Consumed {
            return UiHandleResult::Consumed;
        }

        // --- Dispatch to skill bar ---
        if self.skill_bar.handle_event(ui_event) == crate::ui::widget::EventResponse::Consumed {
            self.process_skill_bar_actions(app_state);
            return UiHandleResult::Consumed;
        }

        // --- Dispatch to HUD button bar ---
        if self.hud_buttons.handle_event(ui_event) == crate::ui::widget::EventResponse::Consumed {
            for action in self.hud_buttons.take_actions() {
                if let WidgetAction::TogglePanel(panel) = action {
                    match panel {
                        HudPanel::Skills => self.skills_panel.toggle(),
                        HudPanel::Inventory => self.inventory_panel.toggle(),
                        HudPanel::Settings => {
                            self.settings_panel.toggle();
                            if self.settings_panel.is_visible() {
                                let data = self.build_settings_panel_data(app_state);
                                self.settings_panel.sync_state(&data);
                            }
                        }
                        HudPanel::Minimap => self.minimap_widget.toggle(),
                        HudPanel::KeyBindings => {}
                        HudPanel::Talents => self.talent_panel.toggle(),
                        HudPanel::QuestLog => self.quest_log_panel.toggle(),
                    }
                }
            }
            return UiHandleResult::Consumed;
        }

        UiHandleResult::NotConsumed
    }
}
