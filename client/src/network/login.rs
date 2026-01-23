use std::{
    io::{Read, Write},
    net::TcpStream,
    sync::mpsc,
    time::Duration,
};

use bevy::prelude::*;
use bevy::tasks::IoTaskPool;
use flate2::Decompress;
use mag_core::encrypt::xcrypt;

use super::{
    client_commands, server_commands, tick_stream, LoginRequested, LoginStatus, NetworkCommand,
    NetworkEvent, NetworkRuntime,
};

/// Starts the async network task for a login attempt.
///
/// Reads the most recent `LoginRequested` message, allocates the command/event channels, and
/// spawns a background task that performs the TCP connect + login handshake + main loop.
pub(super) fn start_login(
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
    net.event_rx = Some(std::sync::Arc::new(std::sync::Mutex::new(event_rx)));
    net.started = true;

    // Keep the task stored in the resource so it isn't dropped/canceled.
    net.task = Some(IoTaskPool::get().spawn(async move {
        log::debug!("Network task started");
        run_network_task(req, command_rx, event_tx);
    }));

    log::debug!("start_login - end");
}

/// Runs the connection and login sequence, then enters the main network loop.
///
/// All user-facing state updates are emitted back to the main thread via `NetworkEvent`s.
fn run_network_task(
    req: LoginRequested,
    command_rx: mpsc::Receiver<NetworkCommand>,
    event_tx: mpsc::Sender<NetworkEvent>,
) {
    let _ = event_tx.send(NetworkEvent::Status(format!(
        "Connecting to {}:{}...",
        req.host, req.port
    )));

    let mut stream = match connect_stream(&req) {
        Ok(s) => s,
        Err(e) => {
            log::error!("connect_stream failed: {e}");
            let _ = event_tx.send(NetworkEvent::Error(e));
            return;
        }
    };

    if let Err(e) = stream.set_read_timeout(Some(Duration::from_millis(5000))) {
        log::warn!("Failed to set read timeout: {e}");
    }

    let _ = event_tx.send(NetworkEvent::Status("Connected. Logging in...".to_string()));

    if let Err(e) = login_handshake(&mut stream, &req, &event_tx) {
        log::error!("login_handshake failed: {e}");
        let _ = event_tx.send(NetworkEvent::Error(e));
        return;
    }

    // `Sender` is cheap to clone; keep one copy so we can report failures.
    let event_tx_loop = event_tx.clone();
    if let Err(e) = run_network_loop(stream, command_rx, event_tx_loop) {
        log::error!("network loop exited with error: {e}");
        let _ = event_tx.send(NetworkEvent::Error(e));
    }
}

/// Connects a TCP stream to the requested host/port.
///
/// Returns a user-displayable error string on failure.
fn connect_stream(req: &LoginRequested) -> Result<TcpStream, String> {
    let addr = format!("{}:{}", req.host, req.port);
    TcpStream::connect(&addr).map_err(|e| {
        log::error!("Connect failed to {addr}: {e}");
        format!("Connect failed: {e}")
    })
}

/// Reads one 16-byte login-phase server command and decodes it.
///
/// The original protocol uses fixed 16-byte commands during login.
fn get_server_response(stream: &mut TcpStream) -> Option<server_commands::ServerCommand> {
    // Read exactly 16 bytes (login-phase command size in original client).
    let mut buf = [0u8; 16];
    stream.read_exact(&mut buf).ok()?;
    server_commands::ServerCommand::from_bytes(&buf)
}

/// Performs the login handshake, mirroring the original client's `socket.c` flow.
///
/// This sends either a new-player login or an existing login depending on whether stored
/// credentials are present (`user_id != 0`). On success, emits `NetworkEvent::LoggedIn`.
fn login_handshake(
    stream: &mut TcpStream,
    req: &LoginRequested,
    event_tx: &mpsc::Sender<NetworkEvent>,
) -> Result<(), String> {
    // Mirror `socket.c`:
    // 1) if a password was provided, send CL_PASSWD
    // 2) if we have stored credentials (user_id != 0), send CL_LOGIN; else send CL_NEWLOGIN

    if !req.password.is_empty() {
        log::info!("Sending password command");
        let pw = client_commands::ClientCommand::new_password(req.password.as_bytes());
        stream
            .write_all(&pw.to_bytes())
            .map_err(|e| format!("Send failed: {e}"))?;
    }

    let (login_pass1, login_pass2) = if req.password.is_empty() {
        (req.pass1, req.pass2)
    } else {
        pass_hash_from_password(&req.password)
    };

    let login_command = if req.user_id != 0 {
        log::info!("Sending existing login command (CL_LOGIN)");
        client_commands::ClientCommand::new_existing_login(req.user_id, login_pass1, login_pass2)
    } else {
        log::info!("Sending newplayer login command (CL_NEWLOGIN)");
        client_commands::ClientCommand::new_newplayer_login()
    };
    stream
        .write_all(&login_command.to_bytes())
        .map_err(|e| format!("Send failed: {e}"))?;

    log::info!("Waiting for server response to login command");
    let login_response = get_server_response(stream).ok_or_else(|| {
        log::error!("Failed to read login response command");
        "Read failed".to_string()
    })?;
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
            stream
                .write_all(&challenge_response.to_bytes())
                .map_err(|e| format!("Send failed: {e}"))?;
        }
        _ => {
            log::error!(
                "Unexpected server response during login (expected Challenge): {:?}",
                login_response
            );
            return Err("Unexpected server response during login".to_string());
        }
    }

    log::info!("Sending unique command");
    let unique_command = client_commands::ClientCommand::new_unique(12345, 67890);
    stream
        .write_all(&unique_command.to_bytes())
        .map_err(|e| format!("Send failed: {e}"))?;

    loop {
        let Some(is_logged_in) = get_server_response(stream) else {
            log::error!("Failed to read login completion response");
            return Err("Read failed".to_string());
        };

        match is_logged_in.structured_data {
            // For an existing player
            server_commands::ServerCommandData::LoginOk { server_version } => {
                let _ = event_tx.send(NetworkEvent::Status("Login successful.".to_string()));
                log::info!("Logged in with server version: {}", server_version);
                let _ = event_tx.send(NetworkEvent::LoggedIn);
                return Ok(());
            }
            // For a new player
            server_commands::ServerCommandData::NewPlayer {
                player_id,
                pass1,
                pass2,
                server_version,
            } => {
                let _ = event_tx.send(NetworkEvent::NewPlayerCredentials {
                    user_id: player_id,
                    pass1,
                    pass2,
                });
                let _ = event_tx.send(NetworkEvent::Status("Login successful.".to_string()));
                log::info!(
                    "New player created with ID: {}, server version: {}, pass1: {}, pass2: {}",
                    player_id,
                    server_version,
                    pass1,
                    pass2
                );
                let _ = event_tx.send(NetworkEvent::LoggedIn);
                return Ok(());
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
                log::warn!("Server demanded exit during login, reason={reason}");
                return Err(format!(
                    "Server closed connection during login, reason code: {}",
                    reason
                ));
            }
            _ => {
                log::error!(
                    "Unexpected server response during login completion: {:?}",
                    is_logged_in
                );
                return Err(format!(
                    "Unexpected server response during login {:?}",
                    is_logged_in
                ));
            }
        }
    }
}

/// Match the server's password hashing for `pass1`/`pass2` derived from user input.
fn pass_hash_from_password(password: &str) -> (u32, u32) {
    let bytes = password.as_bytes();
    let pass1 = bytes
        .iter()
        .take(4)
        .fold(0u32, |acc, &b| (acc << 8) | (b as u32));
    let pass2 = bytes
        .iter()
        .skip(4)
        .take(4)
        .fold(0u32, |acc, &b| (acc << 8) | (b as u32));
    (pass1, pass2)
}

/// Main network loop: interleaves outgoing writes with incoming tick packet parsing.
///
/// The server sends framed tick packets with a 2-byte length/flags header. Payloads may be
/// uncompressed (raw tick bytes) or a chunk of a streaming zlib stream.
fn run_network_loop(
    mut stream: TcpStream,
    command_rx: mpsc::Receiver<NetworkCommand>,
    event_tx: mpsc::Sender<NetworkEvent>,
) -> Result<(), String> {
    // Network loop: read and forward bytes; accept outgoing commands.
    log::info!("Entering network loop");

    // The server sends framed tick packets:
    // - 2-byte header: (len_with_header | 0x8000 if compressed)
    // - payload: either raw tick bytes or a zlib chunk (streaming)
    //
    // Use non-blocking reads + a small accumulator buffer so we can interleave outgoing writes
    // with incoming packet assembly.
    stream.set_nonblocking(true).map_err(|e| {
        log::error!("Failed to set stream to nonblocking mode: {e}");
        "Failed to set stream to nonblocking mode".to_string()
    })?;

    let mut recv_buf: Vec<u8> = Vec::with_capacity(16 * 1024);
    let mut tick_buffer = [0u8; 4096];
    let mut zlib = Decompress::new(true);

    loop {
        let mut did_work = false;

        // Process outgoing commands.
        loop {
            match command_rx.try_recv() {
                Ok(cmd) => {
                    did_work = true;
                    match cmd {
                        NetworkCommand::Send(bytes) => {
                            stream
                                .write_all(&bytes)
                                .map_err(|e| format!("Send failed: {e}"))?;
                            log::debug!("Sent {} bytes to server: {:?}", bytes.len(), bytes);
                        }
                        NetworkCommand::Shutdown => {
                            if event_tx
                                .send(NetworkEvent::Status("Disconnected".to_string()))
                                .is_err()
                            {
                                log::warn!(
                                    "Network task: event receiver dropped, should shut down"
                                );
                            }
                            return Ok(());
                        }
                    }
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    // The main thread dropped the sender (e.g. app shutdown). Exit so the
                    // task can't keep the app alive / block shutdown.
                    return Ok(());
                }
            }
        }

        // Read any available bytes from the socket into our accumulator.
        match stream.read(&mut tick_buffer) {
            Ok(0) => {
                log::warn!("Server closed connection");
                return Err("Server closed connection".to_string());
            }
            Ok(n) => {
                did_work = true;
                recv_buf.extend_from_slice(&tick_buffer[..n]);
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                // nothing to read right now
            }
            Err(e) => {
                log::error!("Read failed in network loop: {e}");
                return Err(format!("Read failed: {e}"));
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
                log::error!("Invalid packet length header: 0x{len_flags:04X}");
                return Err(format!("Invalid packet length header: 0x{len_flags:04X}"));
            }

            if recv_buf.len() < total_len {
                break;
            }

            // Extract payload bytes (excluding the 2-byte header).
            let payload = recv_buf[2..total_len].to_vec();
            recv_buf.drain(..total_len);
            did_work = true;

            // A tick packet may legitimately contain no payload (len==2). The original client
            // still counts this as a tick.
            if payload.is_empty() {
                let _ = event_tx.send(NetworkEvent::Tick);
                continue;
            }

            if is_compressed {
                let inflated = tick_stream::inflate_chunk(&mut zlib, &payload).map_err(|e| {
                    log::error!("Tick inflate failed: {e}");
                    e
                })?;
                if inflated.is_empty() {
                    let _ = event_tx.send(NetworkEvent::Tick);
                    continue;
                }

                let cmds = tick_stream::split_tick_payload(&inflated).map_err(|e| {
                    log::error!("Tick parse failed (compressed): {e}");
                    format!("Tick parse failed (compressed): {e}")
                })?;
                for cmd in cmds {
                    let _ = event_tx.send(NetworkEvent::Bytes(cmd));
                }
                let _ = event_tx.send(NetworkEvent::Tick);
            } else {
                let cmds = tick_stream::split_tick_payload(&payload).map_err(|e| {
                    log::error!("Tick parse failed (uncompressed): {e}");
                    format!("Tick parse failed (uncompressed): {e}")
                })?;
                for cmd in cmds {
                    let _ = event_tx.send(NetworkEvent::Bytes(cmd));
                }
                let _ = event_tx.send(NetworkEvent::Tick);
            }
        }

        // Avoid pegging a core when idle.
        if !did_work {
            std::thread::sleep(Duration::from_millis(1));
        }
    }
}
