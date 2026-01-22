pub mod client_commands;
pub mod login;
pub mod server_commands;
pub mod tick_stream;

use std::sync::{mpsc, Arc, Mutex};

use bevy::prelude::*;
use bevy::tasks::Task;

use crate::player_state::PlayerState;
use crate::systems::sound::SoundEventQueue;
use crate::GameState;
use server_commands::ServerCommand;

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
    Bytes(Vec<u8>),
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
        }
    }
}

impl NetworkRuntime {
    /// Returns the client-side tick counter (used for `CL_CMD_CTICK`).
    pub fn client_ticker(&self) -> u32 {
        self.client_ticker
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
}

pub struct NetworkPlugin;

impl Plugin for NetworkPlugin {
    /// Registers networking resources, messages, and the login/network processing systems.
    fn build(&self, app: &mut App) {
        app.init_resource::<LoginStatus>()
            .init_resource::<NetworkRuntime>()
            .add_message::<LoginRequested>()
            .configure_sets(Update, (NetworkSet::Receive, NetworkSet::Send).chain())
            .add_systems(
                Update,
                login::start_login.run_if(in_state(GameState::LoggingIn)),
            )
            .add_systems(Update, process_network_events.in_set(NetworkSet::Receive))
            .add_systems(
                Update,
                send_client_tick
                    .run_if(in_state(GameState::Gameplay).or(in_state(GameState::Menu)))
                    .in_set(NetworkSet::Send)
                    .after(NetworkSet::Receive),
            );
    }
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
) {
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
                status.message = format!("Error: {e}");
            }
            NetworkEvent::Bytes(bytes) => {
                // After splitting, each event corresponds to exactly one server command.
                if bytes.is_empty() {
                    log::debug!("Received empty server command bytes");
                    continue;
                }

                if let Some(cmd) = ServerCommand::from_bytes(&bytes) {
                    if let server_commands::ServerCommandData::PlaySound { nr, vol, pan } =
                        &cmd.structured_data
                    {
                        sound_queue.push_server_play_sound(*nr, *vol, *pan);
                    } else {
                        player_state.update_from_server_command(&cmd);
                        log::debug!("Received server command: {:?}", cmd);

                        if player_state.take_exit_requested_reason().is_some() {
                            next_state.set(GameState::Exited);
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
                // Match original client: process one tick packet per frame.
                // Stop draining further events so animation steps aren't skipped.
                break;
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
                    let save = player_state.save_file_mut();
                    save.usnr = user_id;
                    save.pass1 = pass1;
                    save.pass2 = pass2;
                }

                match crate::types::mag_files::load_mag_dat() {
                    Ok(mut mag_dat) => {
                        mag_dat.save_file = *player_state.save_file();
                        mag_dat.player_data = *player_state.player_data();
                        if let Err(e) = crate::types::mag_files::save_mag_dat(&mag_dat) {
                            log::error!("Failed to persist mag.dat with new credentials: {e}");
                        }
                    }
                    Err(e) => {
                        log::error!("Failed to load mag.dat to persist new credentials: {e}");
                    }
                }
            }
        }
    }
}
