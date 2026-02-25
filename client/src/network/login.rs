use std::{
    io::{Read, Write},
    net::TcpStream,
    sync::mpsc,
    time::{Duration, Instant},
};

use flate2::Decompress;
use mag_core::constants::LO_PASSWORD;
use mag_core::encrypt::xcrypt;

use super::{client_commands, server_commands, tick_stream, NetworkCommand, NetworkEvent};

/// A combined `Read + Write` trait for use as a trait object.
trait ReadWrite: Read + Write + Send {}
impl<T: Read + Write + Send> ReadWrite for T {}

/// A boxed stream that is `Read + Write` and optionally backed by TLS.
/// We also keep a handle to the raw `TcpStream` so we can call
/// `set_nonblocking` on the underlying socket.
struct GameConnection {
    stream: Box<dyn ReadWrite>,
    raw_tcp: std::net::TcpStream,
}

impl GameConnection {
    fn set_nonblocking(&self, nonblocking: bool) -> std::io::Result<()> {
        self.raw_tcp.set_nonblocking(nonblocking)
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

fn login_exit_reason_message(reason: u32) -> String {
    if (reason as u8) == LO_PASSWORD {
        "Invalid password".to_string()
    } else {
        mag_core::constants::get_exit_reason(reason).to_string()
    }
}

/// Runs the network task: connect, optionally wrap in TLS, handshake, then main loop.
///
/// Intended to be called from `std::thread::spawn`.
pub(crate) fn run_network_task(
    host: String,
    port: u16,
    ticket: u64,
    race: i32,
    use_tls: bool,
    command_rx: mpsc::Receiver<NetworkCommand>,
    event_tx: mpsc::Sender<NetworkEvent>,
) {
    let _ = event_tx.send(NetworkEvent::Status(format!(
        "Connecting to {host}:{port}{}...",
        if use_tls { " (TLS)" } else { "" }
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

    let mut conn = if use_tls {
        let _ = event_tx.send(NetworkEvent::Status("TLS handshake...".to_string()));
        match crate::cert_trust::build_game_tls_connector(&host) {
            Ok(tls_conn) => {
                let tls_stream =
                    rustls::StreamOwned::new(tls_conn, tcp_stream.try_clone().unwrap());
                GameConnection {
                    stream: Box::new(tls_stream),
                    raw_tcp: tcp_stream,
                }
            }
            Err(e) => {
                let _ = event_tx.send(NetworkEvent::Error(format!("TLS setup failed: {e}")));
                return;
            }
        }
    } else {
        let raw_clone = tcp_stream.try_clone().unwrap();
        GameConnection {
            stream: Box::new(tcp_stream),
            raw_tcp: raw_clone,
        }
    };

    let _ = event_tx.send(NetworkEvent::Status("Connected. Logging in...".to_string()));

    if let Err(e) = login_handshake(&mut conn, ticket, race, &event_tx) {
        log::error!("login_handshake failed: {e}");
        let _ = event_tx.send(NetworkEvent::Error(e));
        return;
    }

    if let Err(e) = run_network_loop(conn, command_rx, event_tx.clone()) {
        log::error!("network loop exited with error: {e}");
        let _ = event_tx.send(NetworkEvent::Error(e));
    }
}

/// Reads one login-phase server command (16 bytes, or 2 bytes for tick/exit).
fn get_server_response(
    stream: &mut GameConnection,
) -> Result<server_commands::ServerCommand, String> {
    let mut header = [0u8; 1];
    stream.read_exact(&mut header).map_err(|e| {
        if e.kind() == std::io::ErrorKind::WouldBlock {
            "Timed out waiting for server response (check game server IP/port)".to_string()
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
                "Timed out waiting for server response (check game server IP/port)".to_string()
            } else {
                format!("Read failed: {e}")
            }
        })?;
        buf.extend_from_slice(&rest);
    }

    server_commands::ServerCommand::from_bytes(&buf)
        .ok_or_else(|| "Failed to parse server response".to_string())
}

/// Performs the API-ticket login handshake.
///
/// Flow: `CL_API_LOGIN(ticket)` → `SV_CHALLENGE` → `xcrypt()` → `CL_CHALLENGE` + `CL_UNIQUE`
/// → loop until `SV_LOGIN_OK` (or `SV_NEW_PLAYER` for new accounts).
fn login_handshake(
    stream: &mut GameConnection,
    ticket: u64,
    race: i32,
    event_tx: &mpsc::Sender<NetworkEvent>,
) -> Result<(), String> {
    log::info!("Sending api login command (CL_API_LOGIN)");
    let cmd = client_commands::ClientCommand::new_api_login(ticket);
    stream
        .write_all(&cmd.to_bytes())
        .map_err(|e| format!("Send failed: {e}"))?;

    log::info!("Waiting for server response to login command");
    let login_response = get_server_response(stream)?;
    log::info!("Received login response: {:?}", login_response);
    let _ = event_tx.send(NetworkEvent::Status(
        "Initial command successful.".to_string(),
    ));

    match login_response.structured_data {
        server_commands::ServerCommandData::Challenge { server_challenge } => {
            let encrypted_challenge = xcrypt(server_challenge);
            let challenge_response = client_commands::ClientCommand::new_challenge(
                encrypted_challenge,
                0xFFFFFF, // client version 3.0.0
                race,
            );
            stream
                .write_all(&challenge_response.to_bytes())
                .map_err(|e| format!("Send failed: {e}"))?;
        }
        server_commands::ServerCommandData::Exit { reason } => {
            return Err(login_exit_reason_message(reason));
        }
        _ => {
            log::error!(
                "Unexpected server response during login (expected Challenge): {:?}",
                login_response
            );
            return Err("Server did not respond with a login challenge".to_string());
        }
    }

    log::info!("Sending unique command");
    let unique_command = client_commands::ClientCommand::new_unique(12345, 67890);
    stream
        .write_all(&unique_command.to_bytes())
        .map_err(|e| format!("Send failed: {e}"))?;

    loop {
        let response = get_server_response(stream)?;

        match response.structured_data {
            server_commands::ServerCommandData::LoginOk { server_version } => {
                let _ = event_tx.send(NetworkEvent::Status("Login successful.".to_string()));
                log::info!("Logged in with server version: {}", server_version);
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
                log::info!("Received mod data during login, ignoring");
            }
            server_commands::ServerCommandData::Exit { reason } => {
                log::warn!("Server demanded exit during login, reason={reason}");
                return Err(login_exit_reason_message(reason));
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
                            let _ = event_tx.send(NetworkEvent::Status("Disconnected".to_string()));
                            return Ok(());
                        }
                    }
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => return Ok(()),
            }
        }

        // Read available bytes.
        match stream.read(&mut tick_buffer) {
            Ok(0) => {
                log::warn!("Server closed connection");
                return Err("Server closed connection".to_string());
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
                    let _ = event_tx.send(NetworkEvent::Bytes {
                        bytes: cmd,
                        received_at: Instant::now(),
                    });
                }
                let _ = event_tx.send(NetworkEvent::Tick);
            } else {
                let cmds = tick_stream::split_tick_payload(&payload).map_err(|e| {
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
