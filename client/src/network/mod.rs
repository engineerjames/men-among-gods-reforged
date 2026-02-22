pub mod account_api;
pub mod client_commands;
pub mod login;
pub mod server_commands;
pub mod tick_stream;

use std::collections::HashMap;
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};

use bevy::prelude::*;
use bevy::tasks::Task;

use crate::player_state::PlayerState;
use crate::settings::UserSettingsState;
use crate::systems::sound::SoundEventQueue;
use crate::GameState;
use server_commands::ServerCommand;
use server_commands::ServerCommandData;

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NetworkSet {
    Receive,
    Send,
}

#[allow(dead_code)]
#[derive(Message, Debug, Clone)]
pub struct LoginRequested {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub race: i32,

    /// Mirrors `okey.usnr` from the original client. If non-zero, we should send `CL_LOGIN`.
    pub user_id: u32,
    /// Mirrors `okey.pass1` from the original client.
    pub pass1: u32,
    /// Mirrors `okey.pass2` from the original client.
    pub pass2: u32,

    /// When set, the client will use the custom API ticket login flow instead of
    /// legacy per-character credentials.
    pub login_ticket: Option<u64>,
}

#[derive(Resource, Debug, Clone)]
pub struct LoginStatus {
    pub message: String,
}

impl Default for LoginStatus {
    /// Creates a default status message for a disconnected client.
    fn default() -> Self {
        Self {
            message: "Disconnected".to_string(),
        }
    }
}

enum NetworkCommand {
    Send(Vec<u8>),
    Shutdown,
}

enum NetworkEvent {
    Status(String),
    Bytes {
        bytes: Vec<u8>,
        received_at: Instant,
    },
    /// One complete framed server tick packet was processed.
    Tick,
    Error(String),
    /// New player credentials received during the login handshake.
    NewPlayerCredentials {
        user_id: u32,
        pass1: u32,
        pass2: u32,
    },
    LoggedIn,
}

#[derive(Resource)]
pub struct NetworkRuntime {
    command_tx: Option<mpsc::Sender<NetworkCommand>>,
    event_rx: Option<Arc<Mutex<mpsc::Receiver<NetworkEvent>>>>,
    task: Option<Task<()>>,
    started: bool,
    logged_in: bool,

    /// Client-side tick counter used for `CL_CMD_CTICK` (aka `CmdCTick`).
    ///
    /// In the original C client this is `ticker`, incremented once per processed server tick.
    client_ticker: u32,
    last_ctick_sent: u32,

    // ---------------------------------------------------------------------
    // Ping / Pong RTT tracking (custom extension)
    // ---------------------------------------------------------------------
    start_instant: Instant,
    ping_seq: u32,
    last_ping_sent_at: Option<Instant>,
    pings_in_flight: HashMap<u32, Instant>,
    last_rtt_ms: Option<u32>,
    rtt_ewma_ms: Option<f32>,
}

impl Default for NetworkRuntime {
    /// Creates an unstarted network runtime (no task, no channels, not logged in).
    fn default() -> Self {
        Self {
            command_tx: None,
            event_rx: None,
            task: None,
            started: false,
            logged_in: false,
            client_ticker: 0,
            last_ctick_sent: 0,
            start_instant: Instant::now(),
            ping_seq: 0,
            last_ping_sent_at: None,
            pings_in_flight: HashMap::new(),
            last_rtt_ms: None,
            rtt_ewma_ms: None,
        }
    }
}

impl NetworkRuntime {
    /// Returns whether the background network task is running.
    pub fn is_started(&self) -> bool {
        self.started
    }

    /// Returns the client-side tick counter (used for `CL_CMD_CTICK`).
    pub fn client_ticker(&self) -> u32 {
        self.client_ticker
    }

    /// Most recent measured RTT in milliseconds (from `CL_PING`/`SV_PONG`).
    pub fn last_rtt_ms(&self) -> Option<u32> {
        self.last_rtt_ms
    }

    /// Smoothed RTT (EWMA) in milliseconds.
    pub fn rtt_ewma_ms(&self) -> Option<f32> {
        self.rtt_ewma_ms
    }

    /// Queues raw bytes to be written to the server by the network task.
    ///
    /// No-op if the network task hasn't started.
    pub fn send(&self, bytes: Vec<u8>) {
        let Some(tx) = &self.command_tx else {
            return;
        };
        let _ = tx.send(NetworkCommand::Send(bytes));
    }

    #[allow(dead_code)]
    /// Requests a graceful shutdown of the network task.
    ///
    /// No-op if the network task hasn't started.
    pub fn shutdown(&self) {
        let Some(tx) = &self.command_tx else {
            return;
        };
        let _ = tx.send(NetworkCommand::Shutdown);
    }

    /// Stops the network runtime immediately by dropping channels and the task handle.
    ///
    /// This is useful during state transitions or app shutdown to ensure we can't get
    /// stuck processing an unbounded backlog of network events.
    pub fn stop(&mut self) {
        self.command_tx = None;
        self.event_rx = None;
        self.task = None;
        self.started = false;
        self.logged_in = false;
        self.client_ticker = 0;
        self.last_ctick_sent = 0;
        self.start_instant = Instant::now();
        self.ping_seq = 0;
        self.last_ping_sent_at = None;
        self.pings_in_flight.clear();
        self.last_rtt_ms = None;
        self.rtt_ewma_ms = None;
    }
}

pub struct NetworkPlugin;

impl Plugin for NetworkPlugin {
    /// Registers networking resources, messages, and the login/network processing systems.
    fn build(&self, app: &mut App) {
        app.init_resource::<LoginStatus>()
            .init_resource::<NetworkRuntime>()
            .init_resource::<account_api::ApiSession>()
            .add_message::<LoginRequested>()
            .configure_sets(Update, (NetworkSet::Receive, NetworkSet::Send).chain())
            .add_systems(
                Update,
                login::start_login.run_if(
                    in_state(GameState::LoggingIn).or(in_state(GameState::CharacterSelection)),
                ),
            )
            // Only process network events while we're in active network-driven states.
            // When the "Exited" UI is showing, we intentionally stop draining the queue so
            // the main thread can't stall on a large backlog while the user tries to quit.
            .add_systems(
                Update,
                process_network_events
                    .run_if(
                        in_state(GameState::LoggingIn)
                            .or(in_state(GameState::CharacterSelection))
                            .or(in_state(GameState::Gameplay))
                            .or(in_state(GameState::Menu)),
                    )
                    .in_set(NetworkSet::Receive),
            )
            .add_systems(
                Update,
                send_client_tick
                    .run_if(in_state(GameState::Gameplay).or(in_state(GameState::Menu)))
                    .in_set(NetworkSet::Send)
                    .after(NetworkSet::Receive),
            )
            .add_systems(
                Update,
                send_ping
                    .run_if(in_state(GameState::Gameplay).or(in_state(GameState::Menu)))
                    .in_set(NetworkSet::Send)
                    .after(NetworkSet::Receive),
            );
    }
}

/// Sends a periodic ping (`CL_PING`) used to compute effective RTT on the client.
fn send_ping(mut net: ResMut<NetworkRuntime>) {
    if !net.logged_in {
        return;
    }

    // Don't start until we have processed at least one tick.
    if net.client_ticker == 0 {
        return;
    }

    // One ping every 5 seconds is plenty; keep it stable across framerates.
    const PING_INTERVAL: Duration = Duration::from_secs(5);
    const PING_TIMEOUT: Duration = Duration::from_secs(30);
    const MAX_IN_FLIGHT: usize = 3;
    let now = Instant::now();

    // Expire stale ping entries (e.g. if the server stopped responding).
    net.pings_in_flight
        .retain(|_, sent_at| now.duration_since(*sent_at) <= PING_TIMEOUT);
    if net.pings_in_flight.len() >= MAX_IN_FLIGHT {
        return;
    }

    if let Some(last) = net.last_ping_sent_at {
        if now.duration_since(last) < PING_INTERVAL {
            return;
        }
    }

    let client_time_ms: u32 = now
        .duration_since(net.start_instant)
        .as_millis()
        .min(u128::from(u32::MAX)) as u32;

    net.ping_seq = net.ping_seq.wrapping_add(1);
    let seq = net.ping_seq;

    net.last_ping_sent_at = Some(now);
    net.pings_in_flight.insert(seq, now);

    let cmd = client_commands::ClientCommand::new_ping(seq, client_time_ms);
    net.send(cmd.to_bytes());
}

/// Sends the periodic client tick (`CL_CMD_CTICK`) while in gameplay.
///
/// This mirrors the original client behavior (one tick command every 16 processed server ticks).
fn send_client_tick(mut net: ResMut<NetworkRuntime>) {
    if !net.logged_in {
        return;
    }

    let Some(tx) = &net.command_tx else {
        return;
    };

    // Match original C client behavior:
    // - `ticker` increments once per processed server tick packet.
    // - send `CL_CMD_CTICK` when `(ticker & 15) == 0` (i.e. every 16 ticks).
    // With `TICKS=20`, that's 0.8s.
    let t = net.client_ticker;
    if t == 0 {
        return;
    }
    if (t & 15) != 0 {
        return;
    }
    if net.last_ctick_sent == t {
        return;
    }

    let tick_command = client_commands::ClientCommand::new_tick(t);
    if tx
        .send(NetworkCommand::Send(tick_command.to_bytes()))
        .is_ok()
    {
        net.last_ctick_sent = t;
    }
}

/// Drains events produced by the network task and applies them to game state.
///
/// This updates the on-screen login status, forwards server commands to `PlayerState`, and
/// advances the game state once login completes.
fn process_network_events(
    mut net: ResMut<NetworkRuntime>,
    mut status: ResMut<LoginStatus>,
    mut next_state: ResMut<NextState<GameState>>,
    mut player_state: ResMut<PlayerState>,
    mut sound_queue: ResMut<SoundEventQueue>,
    mut user_settings: ResMut<UserSettingsState>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
) {
    let window_focused = windows
        .single()
        .map(|window| window.focused)
        .unwrap_or(true);

    let max_ticks_per_frame = if window_focused { 1 } else { 64 };
    let mut ticks_processed = 0u32;

    let Some(rx_arc) = net.event_rx.clone() else {
        return;
    };

    let Ok(rx) = rx_arc.lock() else {
        status.message = "Error: network receiver mutex poisoned".to_string();
        log::error!("process_network_events: network receiver mutex poisoned");
        return;
    };

    while let Ok(evt) = rx.try_recv() {
        match evt {
            NetworkEvent::Status(s) => status.message = s,
            NetworkEvent::Error(e) => {
                log::error!("Network error: {e}");

                // During the login screen, errors should put us back into a usable state.
                // Otherwise, the UI stays disabled (is_logging_in=true) and `net.started=true`
                // prevents new attempts.
                if !net.logged_in {
                    net.stop();
                }

                status.message = format!("Error: {e}");
            }
            NetworkEvent::Bytes { bytes, received_at } => {
                // After splitting, each event corresponds to exactly one server command.
                if bytes.is_empty() {
                    log::debug!("Received empty server command bytes");
                    continue;
                }

                if let Some(cmd) = ServerCommand::from_bytes(&bytes) {
                    match &cmd.structured_data {
                        ServerCommandData::Pong {
                            seq,
                            client_time_ms,
                        } => {
                            if let Some(sent_at) = net.pings_in_flight.remove(seq) {
                                let rtt_ms = if received_at >= sent_at {
                                    received_at.duration_since(sent_at).as_millis() as u32
                                } else {
                                    sent_at.elapsed().as_millis() as u32
                                };
                                net.last_rtt_ms = Some(rtt_ms);
                                net.rtt_ewma_ms = Some(match net.rtt_ewma_ms {
                                    Some(prev) => prev * 0.8 + (rtt_ms as f32) * 0.2,
                                    None => rtt_ms as f32,
                                });
                                log::info!(
                                    "Ping RTT: {} ms (seq={}, client_time_ms={})",
                                    rtt_ms,
                                    seq,
                                    client_time_ms
                                );
                            }
                            continue;
                        }
                        server_commands::ServerCommandData::PlaySound { nr, vol, pan } => {
                            sound_queue.push_server_play_sound(*nr, *vol, *pan);
                        }
                        _ => {
                            player_state.update_from_server_command(&cmd);
                            log::debug!("Received server command: {:?}", cmd);

                            // Persist updated character name/race once the full name arrives.
                            // The server sends it in 3 chunks; chunk 3 completes the name.
                            if matches!(cmd.structured_data, ServerCommandData::SetCharName3 { .. })
                            {
                                user_settings.sync_character_from_player_state(&player_state);
                                user_settings.request_save();
                            }

                            if player_state.take_exit_requested_reason().is_some() {
                                next_state.set(GameState::Exited);
                            }
                        }
                    }
                } else {
                    log::warn!(
                        "Received unknown/invalid server command opcode={} ({} bytes)",
                        bytes[0],
                        bytes.len()
                    );
                }
            }
            NetworkEvent::Tick => {
                net.client_ticker = net.client_ticker.wrapping_add(1);
                player_state.on_tick_packet(net.client_ticker);
                player_state.map_mut().reset_last_setmap_index();
                // Match original client: process one tick packet per frame while focused.
                // When unfocused, allow draining more to avoid a growing backlog.
                ticks_processed += 1;
                if ticks_processed >= max_ticks_per_frame {
                    break;
                }
            }
            NetworkEvent::LoggedIn => {
                log::info!("Login process complete, switching to Gameplay state");
                net.logged_in = true;
                next_state.set(GameState::Gameplay);
            }
            NetworkEvent::NewPlayerCredentials {
                user_id,
                pass1,
                pass2,
            } => {
                log::info!(
                    "Persisting new player credentials (id={}, pass1={}, pass2={})",
                    user_id,
                    pass1,
                    pass2
                );

                {
                    // let save = player_state.save_file_mut();
                    // save.usnr = user_id;
                    // save.pass1 = pass1;
                    // save.pass2 = pass2;
                }

                user_settings.sync_character_from_player_state(&player_state);
                user_settings.request_save();
            }
        }
    }
}
