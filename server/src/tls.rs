//! TLS helper for the game server.
//!
//! Provides [`GameStream`], a wrapper that abstracts over plain TCP and
//! TLS-encrypted connections so that the rest of the server code can use
//! `Read` + `Write` without caring about the transport.

use rustls::ServerConnection;
use socket2::SockRef;
use std::io::{self, Read, Write};
use std::net::{Shutdown, SocketAddr, TcpStream};
use std::sync::Arc;
use std::time::{Duration, Instant};

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
        stream.conn.send_close_notify();

        // Best-effort: a peer that stops reading must not block the tick thread.
        while stream.conn.wants_write() {
            match stream.conn.write_tls(&mut stream.sock) {
                Ok(0) => break,
                Ok(_) => {}
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

/// A server-side TLS handshake driven incrementally from the tick loop.
///
/// The socket is non-blocking for its whole lifetime, so a peer that stalls
/// mid-handshake can never block the game thread; it simply stays pending
/// until [`PendingTls::is_expired`] and is dropped by the caller.
pub struct PendingTls {
    conn: ServerConnection,
    sock: TcpStream,
    /// Remote peer address.
    pub addr: SocketAddr,
    deadline: Instant,
}

impl PendingTls {
    /// Starts a non-blocking server handshake on an accepted stream.
    ///
    /// # Arguments
    ///
    /// * `sock` - Accepted TCP stream.
    /// * `addr` - Remote peer address.
    /// * `config` - Server TLS configuration.
    /// * `timeout` - Total time the peer has to finish the handshake.
    ///
    /// # Returns
    ///
    /// * `Ok` with the pending handshake, or `Err` with failure details.
    pub fn new(
        sock: TcpStream,
        addr: SocketAddr,
        config: Arc<rustls::ServerConfig>,
        timeout: Duration,
    ) -> Result<Self, String> {
        sock.set_nonblocking(true)
            .map_err(|e| format!("set_nonblocking(true): {e}"))?;
        let conn =
            ServerConnection::new(config).map_err(|e| format!("ServerConnection::new: {e}"))?;
        Ok(Self {
            conn,
            sock,
            addr,
            deadline: Instant::now() + timeout,
        })
    }

    /// Performs whatever handshake I/O is possible without blocking.
    ///
    /// # Returns
    ///
    /// * `Ok(true)` once the handshake has completed.
    /// * `Ok(false)` while still waiting on the peer.
    /// * `Err` when the peer sent invalid data or closed the connection.
    pub fn poll(&mut self) -> io::Result<bool> {
        match self.conn.complete_io(&mut self.sock) {
            Ok(_) => {}
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {}
            Err(e) => return Err(e),
        }
        Ok(!self.conn.is_handshaking())
    }

    /// Returns whether the handshake deadline has passed.
    ///
    /// # Arguments
    ///
    /// * `now` - Current time.
    ///
    /// # Returns
    ///
    /// * `true` when `now` is at or past the deadline.
    pub fn is_expired(&self, now: Instant) -> bool {
        now >= self.deadline
    }

    /// Converts a completed handshake into a game stream.
    ///
    /// # Returns
    ///
    /// * A non-blocking `GameStream::Tls`.
    pub fn into_stream(self) -> GameStream {
        GameStream::Tls(TlsGameStream::new(rustls::StreamOwned::new(
            self.conn, self.sock,
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    #[derive(Debug)]
    struct NoCert;

    impl rustls::server::ResolvesServerCert for NoCert {
        fn resolve(
            &self,
            _: rustls::server::ClientHello<'_>,
        ) -> Option<Arc<rustls::sign::CertifiedKey>> {
            None
        }
    }

    fn pending_with_peer() -> (PendingTls, TcpStream) {
        let config = rustls::ServerConfig::builder_with_provider(Arc::new(
            rustls::crypto::aws_lc_rs::default_provider(),
        ))
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_no_client_auth()
        .with_cert_resolver(Arc::new(NoCert));
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let peer = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let (sock, addr) = listener.accept().unwrap();
        let pending =
            PendingTls::new(sock, addr, Arc::new(config), Duration::from_secs(5)).unwrap();
        (pending, peer)
    }

    #[test]
    fn silent_peer_does_not_block_poll() {
        let (mut pending, _peer) = pending_with_peer();
        let start = Instant::now();
        assert!(!pending.poll().unwrap());
        assert!(start.elapsed() < Duration::from_millis(500));
        assert!(!pending.is_expired(Instant::now()));
        assert!(pending.is_expired(Instant::now() + Duration::from_secs(6)));
    }

    #[test]
    fn non_tls_peer_fails_poll() {
        let (mut pending, mut peer) = pending_with_peer();
        peer.write_all(b"GET / HTTP/1.1\r\nHost: x\r\n\r\n")
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            match pending.poll() {
                Err(_) => break,
                Ok(false) if Instant::now() < deadline => std::thread::yield_now(),
                other => panic!("expected handshake failure, got {other:?}"),
            }
        }
    }
}
