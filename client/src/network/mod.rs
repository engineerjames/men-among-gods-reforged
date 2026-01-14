mod client_commands;
mod server_commands;

use std::{
    io::{Read, Write},
    net::TcpStream,
    sync::{mpsc, Arc, Mutex},
    time::Duration,
};

use bevy::prelude::*;
use bevy::tasks::{IoTaskPool, Task};
use mag_core::encrypt::xcrypt;

use crate::{network::server_commands::ServerCommand, GameState};

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
    Error(String),
}

#[derive(Resource)]
pub struct NetworkRuntime {
    command_tx: Option<mpsc::Sender<NetworkCommand>>,
    event_rx: Option<Arc<Mutex<mpsc::Receiver<NetworkEvent>>>>,
    task: Option<Task<()>>,
    started: bool,
}

impl Default for NetworkRuntime {
    fn default() -> Self {
        Self {
            command_tx: None,
            event_rx: None,
            task: None,
            started: false,
        }
    }
}

impl NetworkRuntime {
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
            .add_systems(Update, start_login.run_if(in_state(GameState::LoggingIn)))
            .add_systems(Update, process_network_events);
    }
}

fn get_server_response(stream: &mut TcpStream) -> Option<ServerCommand> {
    // read exactly 16 bytes (login-phase command size in original client)
    let mut buf = [0u8; 16];
    // This will block until 16 bytes are read or an error occurs.
    // If you keep a read timeout set, read_exact will return Err(kind = TimedOut);
    // handle retries/higher-level logic where appropriate.
    stream.read_exact(&mut buf).ok()?;

    ServerCommand::from_bytes(&buf)
}

fn start_login(
    mut ev: MessageReader<LoginRequested>,
    mut net: ResMut<NetworkRuntime>,
    mut status: ResMut<LoginStatus>,
) {
    log::debug!("start_login - start");
    let Some(req) = ev.read().last().cloned() else {
        return;
    };

    if net.started {
        status.message = "Already connected/connecting".to_string();
        log::warn!("start_login called but login already started");
        return;
    }

    status.message = "Connecting...".to_string();

    let (command_tx, command_rx) = mpsc::channel::<NetworkCommand>();
    let (event_tx, event_rx) = mpsc::channel::<NetworkEvent>();

    net.command_tx = Some(command_tx);
    net.event_rx = Some(Arc::new(Mutex::new(event_rx)));
    net.started = true;

    // Keep the task stored in the resource so it isn't dropped/canceled.
    net.task = Some(IoTaskPool::get().spawn(async move {
        log::debug!("Network task started");
        let _ = event_tx.send(NetworkEvent::Status(format!(
            "Connecting to {}:{}...",
            req.host, req.port
        )));

        let addr = format!("{}:{}", req.host, req.port);
        let mut stream = match TcpStream::connect(addr) {
            Ok(s) => s,
            Err(e) => {
                let _ = event_tx.send(NetworkEvent::Error(format!("Connect failed: {e}")));
                return;
            }
        };

        stream
            .set_read_timeout(Some(Duration::from_millis(5000)))
            .ok();

        let _ = event_tx.send(NetworkEvent::Status("Connected. Logging in...".to_string()));

        // TODO: For now, always just send the newplayer login command.
        log::info!("Sending newplayer login command");
        let login_command = client_commands::ClientCommand::new_newplayer_login();
        if let Err(e) = stream.write_all(&login_command.to_bytes()) {
            let _ = event_tx.send(NetworkEvent::Error(format!("Send failed: {e}")));
            return;
        }

        log::info!("Waiting for server response to login command");
        if let Some(login_response) = get_server_response(&mut stream) {
            log::info!("Received login response command: {:?}", login_response);
            let _ = event_tx.send(NetworkEvent::Status(
                "Initial command successful.".to_string(),
            ));

            match login_response.structured_data {
                server_commands::ServerCommandData::Challenge { server_challenge } => {
                    let encrypted_challenge = xcrypt(server_challenge);
                    let challenge_response = client_commands::ClientCommand::new_challenge(
                        encrypted_challenge,
                        0xFFFFFF, // client version 3.0.0
                        req.race,
                    );
                    if let Err(e) = stream.write_all(&challenge_response.to_bytes()) {
                        let _ = event_tx.send(NetworkEvent::Error(format!("Send failed: {e}")));
                        return;
                    }
                }
                _ => {
                    let _ = event_tx.send(NetworkEvent::Error(
                        "Unexpected server response during login".to_string(),
                    ));
                    return;
                }
            }

            log::info!("Sending unique command");
            let unique_command = client_commands::ClientCommand::new_unique(12345, 67890);
            if let Err(e) = stream.write_all(&unique_command.to_bytes()) {
                let _ = event_tx.send(NetworkEvent::Error(format!("Send failed: {e}")));
                return;
            }
        } else {
            let _ = event_tx.send(NetworkEvent::Error("Read failed".to_string()));
            return;
        }

        loop {
            if let Some(is_logged_in) = get_server_response(&mut stream) {
                match is_logged_in.structured_data {
                    // For an existing player
                    server_commands::ServerCommandData::LoginOk { server_version } => {
                        let _ =
                            event_tx.send(NetworkEvent::Status("Login successful.".to_string()));
                        log::info!("Logged in with server version: {}", server_version);
                        // TODO: Transition to the next state
                        break;
                    }
                    // For a new player
                    server_commands::ServerCommandData::NewPlayer {
                        player_id,
                        pass1,
                        pass2,
                        server_version,
                    } => {
                        let _ =
                            event_tx.send(NetworkEvent::Status("Login successful.".to_string()));
                        log::info!(
                            "New player created with ID: {}, server version: {}, pass1: {}, pass2: {}",
                            player_id,
                            server_version,
                            pass1,
                            pass2
                        );
                        break;
                    }
                    server_commands::ServerCommandData::Mod1 { .. }
                    | server_commands::ServerCommandData::Mod2 { .. }
                    | server_commands::ServerCommandData::Mod3 { .. }
                    | server_commands::ServerCommandData::Mod4 { .. }
                    | server_commands::ServerCommandData::Mod5 { .. }
                    | server_commands::ServerCommandData::Mod6 { .. }
                    | server_commands::ServerCommandData::Mod7 { .. }
                    | server_commands::ServerCommandData::Mod8 { .. } => {
                        log::info!("Received mod data during login, ignoring for now");
                    }
                    server_commands::ServerCommandData::Exit { reason } => {
                        let _ = event_tx.send(NetworkEvent::Error(format!(
                            "Server closed connection during login, reason code: {}",
                            reason
                        )));
                        return;
                    }
                    _ => {
                        let _ = event_tx.send(NetworkEvent::Error(format!(
                            "Unexpected server response during login {:?}",
                            is_logged_in
                        )));
                        return;
                    }
                }
            } else {
                let _ = event_tx.send(NetworkEvent::Error("Read failed".to_string()));
                return;
            }
        }

        // Network loop: read and forward bytes; accept outgoing commands.
        log::info!("Entering network loop");
        if stream.set_nonblocking(false).is_err() {
            let _ = event_tx.send(NetworkEvent::Error(
                "Failed to set stream to blocking mode".to_string(),
            ));
            return;
        }

        loop {
            while let Ok(cmd) = command_rx.try_recv() {
                match cmd {
                    NetworkCommand::Send(bytes) => {
                        if let Err(e) = stream.write_all(&bytes) {
                            let _ = event_tx.send(NetworkEvent::Error(format!("Send failed: {e}")));
                            return;
                        }
                    }
                    NetworkCommand::Shutdown => {
                        if event_tx
                            .send(NetworkEvent::Status("Disconnected".to_string()))
                            .is_err()
                        {
                            log::warn!("Network task: event receiver dropped, should shut down");
                        }
                        return;
                    }
                }
            }

            // Drain socket of any incoming data and forward it as events.
            if let Some(cmd) = get_server_response(&mut stream) {
                log::debug!("Network task: received command: {:?}", cmd);
                // TODO: Update to commands with structured data.
                let _ = event_tx.send(NetworkEvent::Bytes(cmd.payload));
            }
        }
    }));
    log::debug!("start_login - end");
}

fn process_network_events(net: ResMut<NetworkRuntime>, mut status: ResMut<LoginStatus>) {
    let Some(rx) = net.event_rx.as_ref() else {
        return;
    };

    let Ok(rx) = rx.lock() else {
        status.message = "Error: network receiver mutex poisoned".to_string();
        log::error!("process_network_events: network receiver mutex poisoned");
        return;
    };

    while let Ok(evt) = rx.try_recv() {
        match evt {
            NetworkEvent::Status(s) => status.message = s,
            NetworkEvent::Error(e) => status.message = format!("Error: {e}"),
            NetworkEvent::Bytes(bytes) => {
                let cmd = ServerCommand::from_bytes(&bytes);

                if let Some(cmd) = cmd {
                    log::info!("Received server command: {:?}", cmd);
                } else {
                    log::warn!("Received invalid server command bytes: {:?}", bytes);
                }
            }
        }
    }
}
