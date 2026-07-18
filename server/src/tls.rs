//! TLS helper for the game server.
//!
//! Provides [`GameStream`], a wrapper that abstracts over plain TCP and
//! TLS-encrypted connections so that the rest of the server code can use
//! `Read` + `Write` without caring about the transport.

use rustls::ServerConnection;
use socket2::SockRef;
use std::io::{self, Read, Write};
use std::net::{Shutdown, TcpStream};
use std::sync::Arc;

const GAME_SOCKET_BUFFER_BYTES: usize = 65_536;

/// Applies game TCP options to an accepted client stream.
///
/// These options mirror the meaningful legacy server socket flags while keeping
/// failures non-fatal, matching the old unchecked `setsockopt` behavior.
///
/// # Arguments
///
/// * `stream` - Accepted TCP stream before TLS wrapping.
pub(crate) fn configure_accepted_tcp_stream(stream: &TcpStream) {
    if let Err(err) = stream.set_nodelay(true) {
        log::warn!("Failed to set TCP_NODELAY on game socket: {err}");
    }

    let socket = SockRef::from(stream);
    if let Err(err) = socket.set_send_buffer_size(GAME_SOCKET_BUFFER_BYTES) {
        log::warn!("Failed to set SO_SNDBUF on game socket: {err}");
    }
    if let Err(err) = socket.set_recv_buffer_size(GAME_SOCKET_BUFFER_BYTES) {
        log::warn!("Failed to set SO_RCVBUF on game socket: {err}");
    }
    if let Err(err) = socket.set_keepalive(true) {
        log::warn!("Failed to set SO_KEEPALIVE on game socket: {err}");
    }
}

/// TLS-encrypted game stream with idempotent shutdown tracking.
pub struct TlsGameStream {
    stream: rustls::StreamOwned<ServerConnection, TcpStream>,
    is_shutdown: bool,
}

impl TlsGameStream {
    fn new(stream: rustls::StreamOwned<ServerConnection, TcpStream>) -> Self {
        Self {
            stream,
            is_shutdown: false,
        }
    }

    fn set_nonblocking(&self, nonblocking: bool) -> io::Result<()> {
        self.stream.sock.set_nonblocking(nonblocking)
    }

    fn shutdown(&mut self, how: Shutdown) -> io::Result<()> {
        if self.is_shutdown {
            return Ok(());
        }
        self.is_shutdown = true;

        let stream = &mut self.stream;
        let _ = stream.sock.set_nonblocking(false);

        stream.conn.send_close_notify();

        while stream.conn.wants_write() {
            match stream.conn.write_tls(&mut stream.sock) {
                Ok(0) => break,
                Ok(_) => {}
                Err(err) if err.kind() == io::ErrorKind::WouldBlock => continue,
                Err(err) => {
                    log::debug!("Failed to write TLS close_notify: {err}");
                    break;
                }
            }
        }

        stream.sock.shutdown(how)
    }
}

impl Drop for TlsGameStream {
    fn drop(&mut self) {
        let _ = self.shutdown(Shutdown::Both);
    }
}

/// A game-server connection that may or may not be TLS-encrypted.
#[allow(clippy::large_enum_variant)]
pub enum GameStream {
    /// Unencrypted TCP connection.
    Plain(TcpStream),
    /// TLS-encrypted connection wrapping a TCP stream.
    Tls(TlsGameStream),
}

impl GameStream {
    /// Sets the underlying TCP stream to non-blocking mode.
    ///
    /// # Arguments
    ///
    /// * `nonblocking` - Value passed to `set_nonblocking`.
    pub fn set_nonblocking(&self, nonblocking: bool) -> io::Result<()> {
        match self {
            GameStream::Plain(s) => s.set_nonblocking(nonblocking),
            GameStream::Tls(s) => s.set_nonblocking(nonblocking),
        }
    }

    /// Shuts down the underlying game connection.
    ///
    /// # Arguments
    ///
    /// * `how` - Value passed to `shutdown`.
    pub fn shutdown(&mut self, how: Shutdown) -> io::Result<()> {
        match self {
            GameStream::Plain(s) => s.shutdown(how),
            GameStream::Tls(s) => s.shutdown(how),
        }
    }
}

impl Read for GameStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self {
            GameStream::Plain(s) => s.read(buf),
            GameStream::Tls(s) => s.stream.read(buf),
        }
    }
}

impl Write for GameStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self {
            GameStream::Plain(s) => s.write(buf),
            GameStream::Tls(s) => s.stream.write(buf),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match self {
            GameStream::Plain(s) => s.flush(),
            GameStream::Tls(s) => s.stream.flush(),
        }
    }
}

/// Loads a TLS `ServerConfig` from PEM cert-chain and private-key files.
///
/// Both `SERVER_TLS_CERT` and `SERVER_TLS_KEY` environment variables are
/// required; the server refuses to start without them.
///
/// # Returns
///
/// * `Ok` when `load_tls_config` succeeds, or `Err` with failure details.
pub fn load_tls_config() -> Result<Arc<rustls::ServerConfig>, String> {
    let cert_path = std::env::var("SERVER_TLS_CERT")
        .ok()
        .filter(|p| !p.trim().is_empty())
        .ok_or_else(|| {
            "SERVER_TLS_CERT environment variable is required (TLS is mandatory)".to_owned()
        })?;
    let key_path = std::env::var("SERVER_TLS_KEY")
        .ok()
        .filter(|p| !p.trim().is_empty())
        .ok_or_else(|| {
            "SERVER_TLS_KEY environment variable is required (TLS is mandatory)".to_owned()
        })?;

    let cert_file = std::fs::File::open(&cert_path)
        .map_err(|e| format!("Cannot open TLS cert file '{cert_path}': {e}"))?;
    let key_file = std::fs::File::open(&key_path)
        .map_err(|e| format!("Cannot open TLS key file '{key_path}': {e}"))?;

    let certs: Vec<rustls::pki_types::CertificateDer<'static>> =
        rustls_pemfile::certs(&mut std::io::BufReader::new(cert_file))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("Failed to parse TLS cert '{cert_path}': {e}"))?;

    let key = rustls_pemfile::private_key(&mut std::io::BufReader::new(key_file))
        .map_err(|e| format!("Failed to parse TLS key '{key_path}': {e}"))?
        .ok_or_else(|| format!("No private key found in '{key_path}'"))?;

    let config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .map_err(|e| format!("Invalid TLS configuration: {e}"))?;

    Ok(Arc::new(config))
}

/// Perform a blocking TLS handshake on `stream` using `config`.
///
/// The stream is temporarily set to blocking mode for the handshake, then
/// switched back to non-blocking afterwards. Returns a `GameStream::Tls` on
/// success.
///
/// # Arguments
///
/// * `stream` - Value passed to `accept_tls`.
/// * `config` - Value passed to `accept_tls`.
///
/// # Returns
///
/// * `Ok` when `accept_tls` succeeds, or `Err` with failure details.
pub fn accept_tls(
    stream: TcpStream,
    config: Arc<rustls::ServerConfig>,
) -> Result<GameStream, String> {
    // TLS handshake requires blocking I/O
    stream
        .set_nonblocking(false)
        .map_err(|e| format!("set_nonblocking(false): {e}"))?;

    // Set a generous timeout for the handshake
    stream
        .set_read_timeout(Some(std::time::Duration::from_secs(10)))
        .map_err(|e| format!("set_read_timeout: {e}"))?;

    let conn = ServerConnection::new(config).map_err(|e| format!("ServerConnection::new: {e}"))?;
    let mut tls_stream = rustls::StreamOwned::new(conn, stream);

    // Drive the handshake to completion by doing a zero-byte read.
    // rustls will perform the handshake I/O internally.
    let _ = tls_stream.read(&mut []);

    // Check that the handshake actually completed
    if tls_stream.conn.is_handshaking() {
        return Err("TLS handshake did not complete".to_owned());
    }

    // Switch back to non-blocking for the game loop
    tls_stream
        .sock
        .set_nonblocking(true)
        .map_err(|e| format!("set_nonblocking(true): {e}"))?;
    tls_stream
        .sock
        .set_read_timeout(None)
        .map_err(|e| format!("clear read_timeout: {e}"))?;

    Ok(GameStream::Tls(TlsGameStream::new(tls_stream)))
}
