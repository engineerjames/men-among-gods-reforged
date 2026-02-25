//! TLS helper for the game server.
//!
//! Provides [`GameStream`], a wrapper that abstracts over plain TCP and
//! TLS-encrypted connections so that the rest of the server code can use
//! `Read` + `Write` without caring about the transport.

use rustls::ServerConnection;
use std::io::{self, Read, Write};
use std::net::{Shutdown, TcpStream};
use std::sync::Arc;

/// A game-server connection that may or may not be TLS-encrypted.
pub enum GameStream {
    /// Unencrypted TCP connection.
    Plain(TcpStream),
    /// TLS-encrypted connection wrapping a TCP stream.
    Tls(rustls::StreamOwned<ServerConnection, TcpStream>),
}

impl GameStream {
    /// Sets the underlying TCP stream to non-blocking mode.
    pub fn set_nonblocking(&self, nonblocking: bool) -> io::Result<()> {
        match self {
            GameStream::Plain(s) => s.set_nonblocking(nonblocking),
            GameStream::Tls(s) => s.sock.set_nonblocking(nonblocking),
        }
    }

    /// Shuts down the underlying TCP connection.
    pub fn shutdown(&self, how: Shutdown) -> io::Result<()> {
        match self {
            GameStream::Plain(s) => s.shutdown(how),
            GameStream::Tls(s) => s.sock.shutdown(how),
        }
    }
}

impl Read for GameStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self {
            GameStream::Plain(s) => s.read(buf),
            GameStream::Tls(s) => s.read(buf),
        }
    }
}

impl Write for GameStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self {
            GameStream::Plain(s) => s.write(buf),
            GameStream::Tls(s) => s.write(buf),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match self {
            GameStream::Plain(s) => s.flush(),
            GameStream::Tls(s) => s.flush(),
        }
    }
}

/// Loads a TLS `ServerConfig` from PEM cert-chain and private-key files.
///
/// Returns `None` if the env vars `SERVER_TLS_CERT` / `SERVER_TLS_KEY` are
/// not set.  Returns an error if the files exist but cannot be parsed.
pub fn load_tls_config() -> Result<Option<Arc<rustls::ServerConfig>>, String> {
    let cert_path = match std::env::var("SERVER_TLS_CERT") {
        Ok(p) if !p.trim().is_empty() => p,
        _ => return Ok(None),
    };
    let key_path = match std::env::var("SERVER_TLS_KEY") {
        Ok(p) if !p.trim().is_empty() => p,
        _ => return Ok(None),
    };

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
        .with_single_cert(certs, key.into())
        .map_err(|e| format!("Invalid TLS configuration: {e}"))?;

    Ok(Some(Arc::new(config)))
}

/// Perform a blocking TLS handshake on `stream` using `config`.
///
/// The stream is temporarily set to blocking mode for the handshake, then
/// switched back to non-blocking afterwards. Returns a `GameStream::Tls` on
/// success.
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
        return Err("TLS handshake did not complete".to_string());
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

    Ok(GameStream::Tls(tls_stream))
}
