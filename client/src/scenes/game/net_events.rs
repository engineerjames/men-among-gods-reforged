use crate::{
    network::{client_commands::ClientCommand, NetworkEvent},
    scenes::scene::SceneType,
    state::AppState,
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
    pub(super) fn process_network_events(&mut self, app_state: &mut AppState) -> Option<SceneType> {
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
    pub(super) fn maybe_send_autolook_and_shop_refresh(&mut self, app_state: &mut AppState) {
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
}
