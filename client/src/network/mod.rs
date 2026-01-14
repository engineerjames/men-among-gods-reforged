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
use flate2::{Decompress, FlushDecompress, Status};
use mag_core::constants::{
    SV_IGNORE, SV_LOAD, SV_PLAYSOUND, SV_SCROLL_DOWN, SV_SCROLL_LEFT, SV_SCROLL_LEFTDOWN,
    SV_SCROLL_LEFTUP, SV_SCROLL_RIGHT, SV_SCROLL_RIGHTDOWN, SV_SCROLL_RIGHTUP, SV_SCROLL_UP,
    SV_SETCHAR_AEND, SV_SETCHAR_AHP, SV_SETCHAR_AMANA, SV_SETCHAR_ATTRIB, SV_SETCHAR_DIR,
    SV_SETCHAR_ENDUR, SV_SETCHAR_GOLD, SV_SETCHAR_HP, SV_SETCHAR_ITEM, SV_SETCHAR_MANA,
    SV_SETCHAR_MODE, SV_SETCHAR_OBJ, SV_SETCHAR_PTS, SV_SETCHAR_SKILL, SV_SETCHAR_SPELL,
    SV_SETCHAR_WORN, SV_SETMAP, SV_SETMAP3, SV_SETMAP4, SV_SETMAP5, SV_SETMAP6, SV_SETORIGIN,
    SV_SETTARGET, SV_TICK, SV_UNIQUE,
};
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

/// Decode one zlib-compressed chunk from the server's continuous zlib stream.
///
/// The server uses a per-connection `ZlibEncoder` and sends only the newly
/// produced bytes each tick (i.e. it's a single streaming zlib payload split
/// into chunks). Therefore we must keep a persistent `Decompress` state.
fn zlib_inflate_chunk(z: &mut Decompress, input: &[u8]) -> Result<Vec<u8>, String> {
    if input.is_empty() {
        return Ok(Vec::new());
    }

    let mut out = Vec::<u8>::new();
    let mut in_pos = 0usize;
    let mut scratch = [0u8; 8192];

    while in_pos < input.len() {
        let before_in = z.total_in() as usize;
        let before_out = z.total_out() as usize;

        let status = z
            .decompress(&input[in_pos..], &mut scratch, FlushDecompress::Sync)
            .map_err(|e| format!("zlib inflate failed: {e}"))?;

        let after_in = z.total_in() as usize;
        let after_out = z.total_out() as usize;

        let consumed = after_in.saturating_sub(before_in);
        let produced = after_out.saturating_sub(before_out);

        if produced > 0 {
            out.extend_from_slice(&scratch[..produced]);
        }

        if consumed == 0 {
            // Avoid an infinite loop if the inflater can't make forward progress.
            // This can happen if the input is truncated mid-stream.
            if status == Status::Ok && produced == 0 {
                return Err("zlib inflate made no progress (truncated input?)".to_string());
            }
            break;
        }

        in_pos += consumed;
    }

    Ok(out)
}

fn sv_setmap_len(bytes: &[u8], off: u8, lastn: &mut i32) -> Result<usize, String> {
    if bytes.len() < 2 {
        return Err("SV_SETMAP truncated (need at least 2 bytes)".to_string());
    }

    // Mirrors `socket.c::sv_setmap`.
    let mut p: usize;
    let n: i32;
    if off != 0 {
        n = *lastn + off as i32;
        p = 2;
    } else {
        if bytes.len() < 4 {
            return Err("SV_SETMAP truncated (need 4 bytes for index)".to_string());
        }
        n = u16::from_le_bytes([bytes[2], bytes[3]]) as i32;
        p = 4;
    }

    *lastn = n;

    let flags = bytes[1];
    if flags == 0 {
        return Err("SV_SETMAP has zero flags".to_string());
    }

    // Size accounting only.
    if flags & 1 != 0 {
        p += 2;
    }
    if flags & 2 != 0 {
        p += 4;
    }
    if flags & 4 != 0 {
        p += 4;
    }
    if flags & 8 != 0 {
        p += 2;
    }
    if flags & 16 != 0 {
        p += 1;
    }
    if flags & 32 != 0 {
        p += 4;
    }
    if flags & 64 != 0 {
        p += 5;
    }
    if flags & 128 != 0 {
        p += 1;
    }

    Ok(p)
}

fn sv_setmap3_len(cnt: usize) -> usize {
    // Mirrors `socket.c::sv_setmap3`: returns p where p starts at 3 and increments once per two
    // tiles covered by `cnt`.
    3 + (cnt / 2)
}

fn sv_cmd_len(bytes: &[u8], last_setmap_n: &mut i32) -> Result<usize, String> {
    if bytes.is_empty() {
        return Err("sv_cmd_len called with empty buffer".to_string());
    }

    let op = bytes[0];

    // Special case: any opcode with the SV_SETMAP (0x80) bit set is a setmap packet.
    // The lower 7 bits carry the delta offset.
    if (op & SV_SETMAP) != 0 {
        let off = op & !SV_SETMAP;
        return sv_setmap_len(bytes, off, last_setmap_n);
    }

    let len = match op {
        SV_SETCHAR_MODE => 2,
        SV_SETCHAR_ATTRIB => 8,
        SV_SETCHAR_SKILL => 8,
        SV_SETCHAR_HP => 13,
        SV_SETCHAR_ENDUR => 13,
        SV_SETCHAR_MANA => 13,
        SV_SETCHAR_AHP => 3,
        SV_SETCHAR_AEND => 3,
        SV_SETCHAR_AMANA => 3,
        SV_SETCHAR_DIR => 2,

        SV_SETCHAR_PTS => 13,
        SV_SETCHAR_GOLD => 13,
        SV_SETCHAR_ITEM => 9,
        SV_SETCHAR_WORN => 9,
        SV_SETCHAR_SPELL => 9,
        SV_SETCHAR_OBJ => 5,

        SV_SETMAP3 => sv_setmap3_len(26),
        SV_SETMAP4 => sv_setmap3_len(0),
        SV_SETMAP5 => sv_setmap3_len(2),
        SV_SETMAP6 => sv_setmap3_len(6),
        SV_SETORIGIN => 5,

        SV_TICK => 2,

        SV_SCROLL_RIGHT => 1,
        SV_SCROLL_LEFT => 1,
        SV_SCROLL_DOWN => 1,
        SV_SCROLL_UP => 1,
        SV_SCROLL_RIGHTDOWN => 1,
        SV_SCROLL_RIGHTUP => 1,
        SV_SCROLL_LEFTDOWN => 1,
        SV_SCROLL_LEFTUP => 1,

        SV_SETTARGET => 13,
        SV_PLAYSOUND => 13,

        SV_LOAD => 5,
        SV_UNIQUE => 9,
        SV_IGNORE => {
            if bytes.len() < 5 {
                return Err("SV_IGNORE truncated (need 5 bytes for size)".to_string());
            }
            u32::from_le_bytes([bytes[1], bytes[2], bytes[3], bytes[4]]) as usize
        }

        // Most remaining commands are fixed 16 bytes in the original client.
        // Unknown opcodes should be treated as errors (the original client exits).
        _ => 16,
    };

    Ok(len)
}

fn split_tick_payload(payload: &[u8]) -> Result<Vec<Vec<u8>>, String> {
    let mut out = Vec::<Vec<u8>>::new();
    let mut idx = 0usize;

    // Mirrors `tick_do`: reset lastn before scanning each tick payload.
    let mut last_setmap_n: i32 = -1;

    while idx < payload.len() {
        let len = sv_cmd_len(&payload[idx..], &mut last_setmap_n)?;
        if len == 0 {
            return Err("sv_cmd_len returned 0".to_string());
        }
        if idx + len > payload.len() {
            return Err(format!(
                "Truncated server command: need {len} bytes, have {}",
                payload.len() - idx
            ));
        }
        out.push(payload[idx..idx + len].to_vec());
        idx += len;
    }

    Ok(out)
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
        // The server now sends framed tick packets:
        // - 2-byte header: (len_with_header | 0x8000 if compressed)
        // - payload: either raw tick bytes or a zlib chunk (streaming)
        //
        // Use non-blocking reads + a small accumulator buffer so we can
        // interleave outgoing writes with incoming packet assembly.
        if stream.set_nonblocking(true).is_err() {
            let _ = event_tx.send(NetworkEvent::Error(
                "Failed to set stream to nonblocking mode".to_string(),
            ));
            return;
        }

        let mut recv_buf: Vec<u8> = Vec::with_capacity(16 * 1024);
        let mut tmp = [0u8; 4096];
        let mut zlib = Decompress::new(true);

        loop {
            let mut did_work = false;

            while let Ok(cmd) = command_rx.try_recv() {
                did_work = true;
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

            // Read any available bytes from the socket into our accumulator.
            match stream.read(&mut tmp) {
                Ok(0) => {
                    let _ = event_tx.send(NetworkEvent::Error(
                        "Server closed connection".to_string(),
                    ));
                    return;
                }
                Ok(n) => {
                    did_work = true;
                    recv_buf.extend_from_slice(&tmp[..n]);
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    // nothing to read right now
                }
                Err(e) => {
                    let _ = event_tx.send(NetworkEvent::Error(format!("Read failed: {e}")));
                    return;
                }
            }

            // Parse as many complete framed packets as we can.
            loop {
                if recv_buf.len() < 2 {
                    break;
                }

                let len_flags = u16::from_ne_bytes([recv_buf[0], recv_buf[1]]);
                let is_compressed = (len_flags & 0x8000) != 0;
                let total_len = (len_flags & 0x7FFF) as usize;

                if total_len < 2 {
                    let _ = event_tx.send(NetworkEvent::Error(format!(
                        "Invalid packet length header: 0x{len_flags:04X}"
                    )));
                    return;
                }

                if recv_buf.len() < total_len {
                    break;
                }

                // Extract payload bytes (excluding the 2-byte header).
                let payload = recv_buf[2..total_len].to_vec();
                recv_buf.drain(..total_len);
                did_work = true;

                if payload.is_empty() {
                    continue;
                }

                if is_compressed {
                    match zlib_inflate_chunk(&mut zlib, &payload) {
                        Ok(inflated) => {
                            if inflated.is_empty() {
                                continue;
                            }

                            match split_tick_payload(&inflated) {
                                Ok(cmds) => {
                                    for cmd in cmds {
                                        let _ = event_tx.send(NetworkEvent::Bytes(cmd));
                                    }
                                }
                                Err(e) => {
                                    let _ = event_tx.send(NetworkEvent::Error(format!(
                                        "Tick parse failed (compressed): {e}"
                                    )));
                                    return;
                                }
                            }
                        }
                        Err(e) => {
                            let _ = event_tx.send(NetworkEvent::Error(e));
                            return;
                        }
                    }
                } else {
                    match split_tick_payload(&payload) {
                        Ok(cmds) => {
                            for cmd in cmds {
                                let _ = event_tx.send(NetworkEvent::Bytes(cmd));
                            }
                        }
                        Err(e) => {
                            let _ = event_tx.send(NetworkEvent::Error(format!(
                                "Tick parse failed (uncompressed): {e}"
                            )));
                            return;
                        }
                    }
                }
            }

            // Avoid pegging a core when idle.
            if !did_work {
                std::thread::sleep(Duration::from_millis(1));
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
                // After splitting, each event corresponds to exactly one server command.
                if bytes.is_empty() {
                    log::debug!("Received empty server command bytes");
                    continue;
                }

                if let Some(cmd) = ServerCommand::from_bytes(&bytes) {
                    log::info!("Received server command: {:?}", cmd);
                } else {
                    log::warn!(
                        "Received unknown/invalid server command opcode={} ({} bytes)",
                        bytes[0],
                        bytes.len()
                    );
                }
            }
        }
    }
}
