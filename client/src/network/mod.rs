mod command;

use std::{
    io::{Read, Write},
    net::TcpStream,
    sync::{mpsc, Arc, Mutex},
};

use bevy::prelude::*;
use bevy::tasks::{IoTaskPool, Task};

use crate::GameState;

#[derive(Message, Debug, Clone)]
pub struct LoginRequested {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
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

        let _ = event_tx.send(NetworkEvent::Status("Connected. Logging in...".to_string()));

        // TODO: Replace this with the actual MOA login handshake.
        // For now, we just demonstrate the wiring.
        let login_probe = format!("LOGIN {} {}\n", req.username, req.password);
        if let Err(e) = stream.write_all(login_probe.as_bytes()) {
            let _ = event_tx.send(NetworkEvent::Error(format!("Send failed: {e}")));
            return;
        }

        // Network loop: read and forward bytes; accept outgoing commands.
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
                        let _ = event_tx.send(NetworkEvent::Status("Disconnected".to_string()));
                        return;
                    }
                }
            }

            // NOTE: Placeholder framing. Replace with real packet framing.
            let mut buf = [0u8; 16];
            if let Err(e) = stream.read_exact(&mut buf) {
                let _ = event_tx.send(NetworkEvent::Error(format!("Read failed: {e}")));
                return;
            }

            let _ = event_tx.send(NetworkEvent::Bytes(buf.to_vec()));
        }
    }));
    log::debug!("start_login - end");
}

fn process_network_events(mut _net: ResMut<NetworkRuntime>, mut status: ResMut<LoginStatus>) {
    let Some(rx) = _net.event_rx.as_ref() else {
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
            NetworkEvent::Bytes(_bytes) => {
                // TODO: Decode bytes and emit higher-level events.
                log::debug!("Received {} bytes from server", _bytes.len());
                log::info!(
                    "Bytes received as utf-8: {}",
                    String::from_utf8_lossy(&_bytes)
                );
            }
        }
    }
}
