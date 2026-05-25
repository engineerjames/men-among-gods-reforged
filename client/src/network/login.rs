use std::{
    io::{Read, Write},
    net::{Shutdown, TcpStream},
    sync::mpsc,
    time::{Duration, Instant},
};

use flate2::{Decompress, FlushDecompress, Status};
use mag_core::server_commands::ServerCommandData;
use mag_core::{client_commands, server_commands::ServerCommand};
use mag_core::{
    logout_reasons::{LogoutReason, get_exit_reason},
    server_commands::ServerCommandType,
};

use super::{NetworkCommand, NetworkEvent};

/// A game connection backed by a TLS session over TCP.
struct GameConnection {
    stream: rustls::StreamOwned<rustls::ClientConnection, TcpStream>,
}

impl GameConnection {
    fn set_nonblocking(&self, nonblocking: bool) -> std::io::Result<()> {
        self.stream.sock.set_nonblocking(nonblocking)
    }

    fn shutdown(&mut self) {
        let stream = &mut self.stream;
        let _ = stream.sock.set_nonblocking(false);

        stream.conn.send_close_notify();

        let (conn, sock) = (&mut stream.conn, &mut stream.sock);
        while conn.wants_write() {
            match conn.write_tls(sock) {
                Ok(0) => break,
                Ok(_) => {}
                Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => continue,
                Err(err) => {
                    log::debug!("Failed to write TLS close_notify: {err}");
                    break;
                }
            }
        }

        let _ = sock.shutdown(Shutdown::Both);
    }
}

impl Read for GameConnection {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.stream.read(buf)
    }
}

impl Write for GameConnection {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.stream.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.stream.flush()
    }
}

/// Runs the network task: connect, wrap in TLS, handshake, then main loop.
///
/// Intended to be called from `std::thread::spawn`.
pub(crate) fn run_network_task(
    host: String,
    port: u16,
    ticket: u64,
    command_rx: mpsc::Receiver<NetworkCommand>,
    event_tx: mpsc::Sender<NetworkEvent>,
) {
    let _ = event_tx.send(NetworkEvent::Status(format!(
        "Connecting to {host}:{port} (TLS)..."
    )));

    let addr = format!("{host}:{port}");
    let tcp_stream = match TcpStream::connect(&addr) {
        Ok(s) => s,
        Err(e) => {
            let _ = event_tx.send(NetworkEvent::Error(format!("Connect failed: {e}")));
            return;
        }
    };

    if let Err(e) = tcp_stream.set_read_timeout(Some(Duration::from_millis(5000))) {
        log::warn!("Failed to set read timeout: {e}");
    }

    let _ = event_tx.send(NetworkEvent::Status("TLS handshake...".to_owned()));
    let mut conn = match crate::cert_trust::build_game_tls_connector(&host) {
        Ok(tls_conn) => {
            let tls_stream = rustls::StreamOwned::new(tls_conn, tcp_stream);
            GameConnection { stream: tls_stream }
        }
        Err(e) => {
            let _ = event_tx.send(NetworkEvent::Error(format!("TLS setup failed: {e}")));
            return;
        }
    };

    let _ = event_tx.send(NetworkEvent::Status("Connected. Logging in...".to_owned()));

    if let Err(e) = login_handshake(&mut conn, ticket, &event_tx) {
        log::error!("login_handshake failed: {e}");
        conn.shutdown();
        let _ = event_tx.send(NetworkEvent::Error(e));
        return;
    }

    if let Err(e) = run_network_loop(conn, command_rx, event_tx.clone()) {
        log::error!("network loop exited with error: {e}");
        let _ = event_tx.send(NetworkEvent::Error(e));
    }
}

/// Reads one login-phase server command (16 bytes, or 2 bytes for tick/exit).
fn get_server_response(stream: &mut GameConnection) -> Result<ServerCommand, String> {
    let mut header = [0u8; 1];
    stream.read_exact(&mut header).map_err(|e| {
        if e.kind() == std::io::ErrorKind::WouldBlock {
            "Timed out waiting for server response (check game server IP/port)".to_owned()
        } else {
            format!("Read failed: {e}")
        }
    })?;

    let opcode = header[0];
    let remaining = match opcode {
        27 | 48 => 1usize,
        _ => 15usize,
    };

    let mut buf = Vec::with_capacity(1 + remaining);
    buf.push(opcode);

    if remaining > 0 {
        let mut rest = vec![0u8; remaining];
        stream.read_exact(&mut rest).map_err(|e| {
            if e.kind() == std::io::ErrorKind::WouldBlock {
                "Timed out waiting for server response (check game server IP/port)".to_owned()
            } else {
                format!("Read failed: {e}")
            }
        })?;
        buf.extend_from_slice(&rest);
    }

    ServerCommand::from_bytes(&buf).ok_or_else(|| "Failed to parse server response".to_owned())
}

/// Performs the API-ticket login handshake.
///
/// Flow: `CL_API_LOGIN(ticket)` --> loop until `SV_LOGIN_OK`, while accepting
/// login-time mod data and server exits.
fn login_handshake(
    stream: &mut GameConnection,
    ticket: u64,
    event_tx: &mpsc::Sender<NetworkEvent>,
) -> Result<(), String> {
    log::info!("Sending api login command (CL_API_LOGIN)");
    let cmd = client_commands::ClientCommand::new_api_login(ticket);
    stream
        .write_all(&cmd.to_bytes())
        .map_err(|e| format!("Send failed: {e}"))?;

    let _ = event_tx.send(NetworkEvent::Status("Login command sent.".to_owned()));

    loop {
        let response = get_server_response(stream)?;

        match response.structured_data {
            ServerCommandData::LoginOk { server_version } => {
                let _ = event_tx.send(NetworkEvent::Status("Login successful.".to_owned()));
                log::info!("Logged in with server version: {}", server_version);
                let _ = event_tx.send(NetworkEvent::LoggedIn);
                return Ok(());
            }
            ServerCommandData::Mod1 { .. }
            | ServerCommandData::Mod2 { .. }
            | ServerCommandData::Mod3 { .. }
            | ServerCommandData::Mod4 { .. }
            | ServerCommandData::Mod5 { .. }
            | ServerCommandData::Mod6 { .. }
            | ServerCommandData::Mod7 { .. }
            | ServerCommandData::Mod8 { .. } => {
                log::info!("Received mod data during login, ignoring");
            }
            ServerCommandData::Exit { reason } => {
                log::warn!("Server demanded exit during login, reason={reason}");
                return Err(get_exit_reason(LogoutReason::from(reason as u8)).to_owned());
            }
            _ => {
                log::error!(
                    "Unexpected server response during login completion: {:?}",
                    response
                );
                return Err(format!(
                    "Unexpected server response during login {:?}",
                    response
                ));
            }
        }
    }
}

/// Main network loop: reads framed tick packets from the server, sends outgoing commands.
fn run_network_loop(
    mut stream: GameConnection,
    command_rx: mpsc::Receiver<NetworkCommand>,
    event_tx: mpsc::Sender<NetworkEvent>,
) -> Result<(), String> {
    log::info!("Entering network loop");

    stream
        .set_nonblocking(true)
        .map_err(|e| format!("Failed to set stream to nonblocking mode: {e}"))?;

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
                        }
                        NetworkCommand::Shutdown => {
                            stream.shutdown();
                            let _ = event_tx.send(NetworkEvent::Status("Disconnected".to_owned()));
                            return Ok(());
                        }
                    }
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    stream.shutdown();
                    return Ok(());
                }
            }
        }

        // Read available bytes.
        match stream.read(&mut tick_buffer) {
            Ok(0) => {
                log::warn!("Server closed connection");
                return Err("Server closed connection".to_owned());
            }
            Ok(n) => {
                did_work = true;
                recv_buf.extend_from_slice(&tick_buffer[..n]);
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(e) => return Err(format!("Read failed: {e}")),
        }

        // Parse complete framed packets.
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

            let payload = recv_buf[2..total_len].to_vec();
            recv_buf.drain(..total_len);
            did_work = true;

            if payload.is_empty() {
                let _ = event_tx.send(NetworkEvent::Tick);
                continue;
            }

            if is_compressed {
                let inflated = inflate_chunk(&mut zlib, &payload).map_err(|e| {
                    log::error!("Tick inflate failed: {e}");
                    e
                })?;
                if inflated.is_empty() {
                    let _ = event_tx.send(NetworkEvent::Tick);
                    continue;
                }

                let cmds = split_tick_payload(&inflated).map_err(|e| {
                    log::error!("Tick parse failed (compressed): {e}");
                    format!("Tick parse failed (compressed): {e}")
                })?;
                for cmd in cmds {
                    let _ = event_tx.send(NetworkEvent::Bytes {
                        bytes: cmd,
                        received_at: Instant::now(),
                    });
                }
                let _ = event_tx.send(NetworkEvent::Tick);
            } else {
                let cmds = split_tick_payload(&payload).map_err(|e| {
                    log::error!("Tick parse failed (uncompressed): {e}");
                    format!("Tick parse failed (uncompressed): {e}")
                })?;
                for cmd in cmds {
                    let _ = event_tx.send(NetworkEvent::Bytes {
                        bytes: cmd,
                        received_at: Instant::now(),
                    });
                }
                let _ = event_tx.send(NetworkEvent::Tick);
            }
        }

        if !did_work {
            std::thread::sleep(Duration::from_millis(1));
        }
    }
}

/// Decode one zlib-compressed chunk from a continuous zlib stream.
fn inflate_chunk(z: &mut Decompress, input: &[u8]) -> Result<Vec<u8>, String> {
    if input.is_empty() {
        return Ok(Vec::new());
    }

    let mut out = Vec::<u8>::new();
    let mut in_pos = 0usize;
    let mut scratch = [0u8; 8192];

    while in_pos <= input.len() {
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

        if consumed > 0 {
            in_pos += consumed;
            continue;
        }

        if produced > 0 {
            continue;
        }

        if in_pos < input.len() && status == Status::Ok {
            return Err("zlib inflate made no progress (truncated input?)".to_owned());
        }
        break;
    }

    Ok(out)
}

/// Splits a server tick payload into individual raw server command byte slices.
fn split_tick_payload(payload: &[u8]) -> Result<Vec<Vec<u8>>, String> {
    let mut out = Vec::<Vec<u8>>::new();
    let mut idx = 0usize;
    let mut last_setmap_n: i32 = -1;

    while idx < payload.len() {
        let len = ServerCommandType::get_expected_length(&payload[idx..], &mut last_setmap_n)?;
        if len == 0 {
            return Err("sv_cmd_len returned 0".to_owned());
        }
        if idx + len > payload.len() {
            let opcode = ServerCommandType::from(payload[idx]);
            let remaining = payload.len() - idx;

            if opcode == ServerCommandType::Exit && remaining < 5 {
                let mut cmd = vec![0u8; 5];
                cmd[0] = ServerCommandType::Exit as u8;
                cmd[1..1 + remaining.saturating_sub(1)]
                    .copy_from_slice(&payload[idx + 1..payload.len()]);
                out.push(cmd);
                break;
            }

            return Err(format!(
                "Truncated server command opcode={opcode:?} at offset={idx}: need {len} bytes, have {remaining}"
            ));
        }
        out.push(payload[idx..idx + len].to_vec());
        idx += len;
    }

    Ok(out)
}

#[cfg(test)]
mod tests {
    use mag_core::server_commands::ServerCommandType;

    use super::*;

    /// `split_tick_payload` correctly splits a payload that mixes a SV_TICK (2
    /// bytes) with one of each light command, all using the new 4-byte header.
    #[test]
    #[allow(clippy::vec_init_then_push)]
    fn split_tick_payload_light_packets_new_format() {
        let mut payload: Vec<u8> = Vec::new();

        // SV_TICK (2 bytes)
        payload.push(ServerCommandType::Tick as u8);
        payload.push(0x05);

        // SV_SETMAP4 / cl_light_one (4 bytes): [op, idx_lo, idx_hi, light]
        payload.push(ServerCommandType::SetMap4 as u8);
        payload.push(0x01); // idx = 1
        payload.push(0x00);
        payload.push(0x07); // light = 7

        // SV_SETMAP5 / cl_light_three (5 bytes): [op, idx_lo, idx_hi, light, nibble]
        payload.push(ServerCommandType::SetMap5 as u8);
        payload.push(0x04);
        payload.push(0x00);
        payload.push(0x05);
        payload.push(0x23); // nibble pair for tiles 5,6

        // SV_SETMAP6 / cl_light_seven (7 bytes): [op, idx_lo, idx_hi, light, 3 nibbles]
        payload.push(ServerCommandType::SetMap6 as u8);
        payload.push(0x0A);
        payload.push(0x00);
        payload.push(0x03);
        payload.push(0x45);
        payload.push(0x67);
        payload.push(0x89);

        // SV_SETMAP3 / cl_light_26 (17 bytes): [op, idx_lo, idx_hi, light, 13 nibbles]
        payload.push(ServerCommandType::SetMap3 as u8);
        payload.push(0x10);
        payload.push(0x00);
        payload.push(0x0F);
        payload.extend(std::iter::repeat_n(0xABu8, 13));

        let cmds = split_tick_payload(&payload).expect("should parse without error");
        assert_eq!(cmds.len(), 5);
        assert_eq!(cmds[0].len(), 2); // SV_TICK
        assert_eq!(cmds[1].len(), 4); // SV_SETMAP4
        assert_eq!(cmds[2].len(), 5); // SV_SETMAP5
        assert_eq!(cmds[3].len(), 7); // SV_SETMAP6
        assert_eq!(cmds[4].len(), 17); // SV_SETMAP3
    }

    /// Ensure a payload containing ONLY an old-format 3-byte SV_SETMAP4 produces
    /// a truncation error (guards against regression to the old length).
    #[test]
    fn split_tick_payload_rejects_old_3byte_light_packet() {
        let payload = vec![ServerCommandType::SetMap4 as u8, 0x01, 0x00]; // only 3 bytes — old format
        let result = split_tick_payload(&payload);
        assert!(result.is_err(), "3-byte SV_SETMAP4 should be rejected");
    }
}
