use std::{
    io::{Read, Write},
    net::TcpStream,
    sync::{mpsc, Arc, Mutex},
};

use bevy::prelude::*;
use bevy::tasks::{IoTaskPool, Task};

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
enum NetCmd {
    Send(Vec<u8>),
    Shutdown,
}

enum NetEvt {
    Status(String),
    Bytes(Vec<u8>),
    Error(String),
}

#[derive(Resource)]
pub struct NetworkRuntime {
    cmd_tx: Option<mpsc::Sender<NetCmd>>,
    evt_rx: Option<Arc<Mutex<mpsc::Receiver<NetEvt>>>>,
    task: Option<Task<()>>,
    started: bool,
}

impl Default for NetworkRuntime {
    fn default() -> Self {
        Self {
            cmd_tx: None,
            evt_rx: None,
            task: None,
            started: false,
        }
    }
}

impl NetworkRuntime {
    #[allow(dead_code)]
    pub fn send(&self, bytes: Vec<u8>) {
        let Some(tx) = &self.cmd_tx else {
            return;
        };
        let _ = tx.send(NetCmd::Send(bytes));
    }

    #[allow(dead_code)]
    pub fn shutdown(&self) {
        let Some(tx) = &self.cmd_tx else {
            return;
        };
        let _ = tx.send(NetCmd::Shutdown);
    }
}

pub struct NetworkPlugin;

impl Plugin for NetworkPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LoginStatus>()
            .init_resource::<NetworkRuntime>()
            .add_message::<LoginRequested>()
            .add_systems(Update, start_login)
            .add_systems(Update, pump_network_events);
    }
}

fn start_login(
    mut ev: MessageReader<LoginRequested>,
    mut net: ResMut<NetworkRuntime>,
    mut status: ResMut<LoginStatus>,
) {
    let Some(req) = ev.read().last().cloned() else {
        return;
    };

    if net.started {
        status.message = "Already connected/connecting".to_string();
        return;
    }

    status.message = "Connecting...".to_string();

    let (cmd_tx, cmd_rx) = mpsc::channel::<NetCmd>();
    let (evt_tx, evt_rx) = mpsc::channel::<NetEvt>();

    net.cmd_tx = Some(cmd_tx);
    net.evt_rx = Some(Arc::new(Mutex::new(evt_rx)));
    net.started = true;

    // Keep the task stored in the resource so it isn't dropped/canceled.
    net.task = Some(IoTaskPool::get().spawn(async move {
        let _ = evt_tx.send(NetEvt::Status(format!(
            "Connecting to {}:{}...",
            req.host, req.port
        )));

        let addr = format!("{}:{}", req.host, req.port);
        let mut stream = match TcpStream::connect(addr) {
            Ok(s) => s,
            Err(e) => {
                let _ = evt_tx.send(NetEvt::Error(format!("Connect failed: {e}")));
                return;
            }
        };

        let _ = evt_tx.send(NetEvt::Status("Connected. Logging in...".to_string()));

        // TODO: Replace this with the actual MOA login handshake.
        // For now, we just demonstrate the wiring.
        let login_probe = format!("LOGIN {} {}\n", req.username, req.password);
        if let Err(e) = stream.write_all(login_probe.as_bytes()) {
            let _ = evt_tx.send(NetEvt::Error(format!("Send failed: {e}")));
            return;
        }

        // Network loop: read and forward bytes; accept outgoing commands.
        loop {
            while let Ok(cmd) = cmd_rx.try_recv() {
                match cmd {
                    NetCmd::Send(bytes) => {
                        if let Err(e) = stream.write_all(&bytes) {
                            let _ = evt_tx.send(NetEvt::Error(format!("Send failed: {e}")));
                            return;
                        }
                    }
                    NetCmd::Shutdown => {
                        let _ = evt_tx.send(NetEvt::Status("Disconnected".to_string()));
                        return;
                    }
                }
            }

            // NOTE: Placeholder framing. Replace with real packet framing.
            let mut buf = [0u8; 16];
            if let Err(e) = stream.read_exact(&mut buf) {
                let _ = evt_tx.send(NetEvt::Error(format!("Read failed: {e}")));
                return;
            }

            let _ = evt_tx.send(NetEvt::Bytes(buf.to_vec()));
        }
    }));
}

fn pump_network_events(mut _net: ResMut<NetworkRuntime>, mut status: ResMut<LoginStatus>) {
    let Some(rx) = _net.evt_rx.as_ref() else {
        return;
    };

    let Ok(rx) = rx.lock() else {
        status.message = "Error: network receiver mutex poisoned".to_string();
        return;
    };

    while let Ok(evt) = rx.try_recv() {
        match evt {
            NetEvt::Status(s) => status.message = s,
            NetEvt::Error(e) => status.message = format!("Error: {e}"),
            NetEvt::Bytes(_bytes) => {
                // TODO: Decode bytes and emit higher-level events.
            }
        }
    }
}
