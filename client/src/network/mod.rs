pub mod client_commands;
pub mod login;
pub mod server_commands;
pub mod tick_stream;

use std::sync::{mpsc, Arc, Mutex};

use bevy::prelude::*;
use bevy::tasks::Task;

use crate::player_state::PlayerState;
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
}

#[derive(Resource, Debug, Clone)]
pub struct LoginStatus {
    pub message: String,
}

impl Default for LoginStatus {
    fn default() -> Self {
        Self {
            message: "Disconnected".to_string(),
        }
    }
}

#[allow(dead_code)]
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
    pub fn client_ticker(&self) -> u32 {
        self.client_ticker
    }

    #[allow(dead_code)]
    pub fn send(&self, bytes: Vec<u8>) {
        let Some(tx) = &self.command_tx else {
            return;
        };
        let _ = tx.send(NetworkCommand::Send(bytes));
    }

    #[allow(dead_code)]
    pub fn shutdown(&self) {
        let Some(tx) = &self.command_tx else {
            return;
        };
        let _ = tx.send(NetworkCommand::Shutdown);
    }
}

pub struct NetworkPlugin;

impl Plugin for NetworkPlugin {
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
                    .run_if(in_state(GameState::Gameplay))
                    .in_set(NetworkSet::Send)
                    .after(NetworkSet::Receive),
            );
    }
}

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

fn process_network_events(
    mut net: ResMut<NetworkRuntime>,
    mut status: ResMut<LoginStatus>,
    mut next_state: ResMut<NextState<GameState>>,
    mut player_state: ResMut<PlayerState>,
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
            NetworkEvent::Error(e) => status.message = format!("Error: {e}"),
            NetworkEvent::Bytes(bytes) => {
                // After splitting, each event corresponds to exactly one server command.
                if bytes.is_empty() {
                    log::debug!("Received empty server command bytes");
                    continue;
                }

                if let Some(cmd) = ServerCommand::from_bytes(&bytes) {
                    player_state.update_from_server_command(&cmd);
                    log::info!("Received server command: {:?}", cmd);
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
        }
    }
}
