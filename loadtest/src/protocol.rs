//! TLS connection, game-login handshake, and framed packet reader.
//!
//! The server uses two wire formats:
//!
//! * **Handshake** (before `SV_LOGIN_OK`): raw fixed-size packets written via
//!   `csend`.  Opcode 27 or 48 are 2 bytes total; everything else is 16 bytes.
//! * **Game loop** (after login): framed packets `[2-byte length|flags][payload]`
//!   written via `compress_ticks`.  Bit 15 of the header signals zlib
//!   compression over a continuous per-connection stream.

use std::sync::Arc;

use anyhow::{Context, anyhow};
use flate2::{Decompress, FlushDecompress, Status};
use mag_core::client_commands::ClientCommand;
use mag_core::server_commands::ServerCommandType;
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{ClientConfig, DigitallySignedStruct, Error as RustlsError, SignatureScheme};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio_rustls::TlsConnector;

// ---------------------------------------------------------------------------
// Accept-any TLS certificate verifier
// ---------------------------------------------------------------------------

/// A [`ServerCertVerifier`] that accepts any server certificate.
///
/// Used for load-test connections to game servers with self-signed certificates
/// on trusted loopback networks.  Security is irrelevant here; correctness of
/// TLS framing is preserved.
#[derive(Debug)]
struct AcceptAnyVerifier;

impl ServerCertVerifier for AcceptAnyVerifier {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, RustlsError> {
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, RustlsError> {
        Ok(HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, RustlsError> {
        Ok(HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        vec![
            SignatureScheme::RSA_PKCS1_SHA256,
            SignatureScheme::RSA_PKCS1_SHA384,
            SignatureScheme::RSA_PKCS1_SHA512,
            SignatureScheme::ECDSA_NISTP256_SHA256,
            SignatureScheme::ECDSA_NISTP384_SHA384,
            SignatureScheme::ECDSA_NISTP521_SHA512,
            SignatureScheme::RSA_PSS_SHA256,
            SignatureScheme::RSA_PSS_SHA384,
            SignatureScheme::RSA_PSS_SHA512,
            SignatureScheme::ED25519,
            SignatureScheme::ED448,
        ]
    }
}

/// Builds a [`ClientConfig`] that accepts any server certificate.
fn build_accept_any_tls_config() -> ClientConfig {
    ClientConfig::builder()
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(AcceptAnyVerifier))
        .with_no_client_auth()
}

// ---------------------------------------------------------------------------
// GameStream: TLS-wrapped TCP connection to the game server
// ---------------------------------------------------------------------------

/// Type alias for the TLS stream type used by the game server.
pub type TlsGameStream = tokio_rustls::client::TlsStream<TcpStream>;

/// Wraps a TLS connection to the game server.
pub struct GameStream {
    inner: TlsGameStream,
}

impl GameStream {
    /// Establishes a TLS TCP connection to the game server.
    ///
    /// # Arguments
    ///
    /// * `host` - Hostname or IP of the game server.
    /// * `port` - TCP port of the game server.
    ///
    /// # Returns
    ///
    /// * `Ok(GameStream)` on success.
    /// * `Err` if the TCP connect or TLS handshake fails.
    pub async fn connect(host: &str, port: u16) -> anyhow::Result<Self> {
        let addr = format!("{host}:{port}");
        let tcp = TcpStream::connect(&addr)
            .await
            .with_context(|| format!("TCP connect to {addr}"))?;

        let config = Arc::new(build_accept_any_tls_config());
        let connector = TlsConnector::from(config);
        let server_name = ServerName::try_from(host.to_owned())
            .map_err(|_| anyhow!("invalid server name '{host}'"))?;

        let tls = connector
            .connect(server_name, tcp)
            .await
            .context("TLS handshake")?;

        Ok(Self { inner: tls })
    }

    /// Performs the game-login handshake: sends `CL_API_LOGIN(ticket)` and waits
    /// for `SV_LOGIN_OK`, tolerating `Mod1..8` and `SV_TICK` packets in between.
    ///
    /// # Arguments
    ///
    /// * `ticket` - One-time login ticket obtained from the account API.
    ///
    /// # Returns
    ///
    /// * `Ok(())` on successful login.
    /// * `Err` if the server rejects the login or sends an unexpected packet.
    pub async fn handshake(&mut self, ticket: u64) -> anyhow::Result<()> {
        let cmd = ClientCommand::new_api_login(ticket);
        self.inner
            .write_all(&cmd.to_bytes())
            .await
            .context("send CL_API_LOGIN")?;

        loop {
            let packet = read_raw_server_packet(&mut self.inner)
                .await
                .context("read login packet")?;

            match ServerCommandType::from(packet[0]) {
                ServerCommandType::LoginOk => return Ok(()),
                ServerCommandType::Exit => {
                    let reason = packet.get(1).copied().unwrap_or(0);
                    return Err(anyhow!("server rejected login (SV_EXIT reason={reason})"));
                }
                _ => {
                    // SV_TICK, Mod1..8, and other login-phase packets — ignore.
                }
            }
        }
    }

    /// Consumes the `GameStream` and returns the underlying TLS stream, ready
    /// for the game loop (split into read/write halves by the caller).
    ///
    /// # Returns
    ///
    /// * The inner [`TlsGameStream`].
    pub fn into_inner(self) -> TlsGameStream {
        self.inner
    }
}

/// Reads one raw (un-framed) server packet during the login handshake phase.
///
/// Opcode 27 (`SV_TICK`) and 48 (`SV_EXIT`) are 2 bytes total; all others
/// are 16 bytes, matching the legacy C protocol.
///
/// # Arguments
///
/// * `stream` - Mutable reference to any `AsyncRead + Unpin` stream.
///
/// # Returns
///
/// * `Ok(Vec<u8>)` containing the complete packet bytes.
/// * `Err` on I/O failure or timeout.
async fn read_raw_server_packet<S: AsyncReadExt + Unpin>(
    stream: &mut S,
) -> anyhow::Result<Vec<u8>> {
    let mut opcode = [0u8; 1];
    stream
        .read_exact(&mut opcode)
        .await
        .context("read opcode")?;

    let remaining = match opcode[0] {
        27 | 48 => 1usize, // SV_TICK or SV_EXIT: 2 bytes total
        _ => 15usize,      // everything else: 16 bytes total
    };

    let mut buf = vec![0u8; 1 + remaining];
    buf[0] = opcode[0];
    if remaining > 0 {
        stream
            .read_exact(&mut buf[1..])
            .await
            .context("read packet body")?;
    }
    Ok(buf)
}

// ---------------------------------------------------------------------------
// FramedReader: post-login framed packet parser + decompressor
// ---------------------------------------------------------------------------

/// Parses the framed packet format used during the game loop.
///
/// Each frame is `[2-byte little-endian length|flags][payload]`.
/// Bit 15 of the length signals zlib compression; the zlib stream is
/// continuous per-connection (state is preserved in `zlib`).
pub struct FramedReader {
    buf: Vec<u8>,
    zlib: Decompress,
}

impl FramedReader {
    /// Creates a new [`FramedReader`] with an empty buffer and fresh zlib state.
    ///
    /// # Returns
    ///
    /// * A new [`FramedReader`].
    pub fn new() -> Self {
        Self {
            buf: Vec::with_capacity(32 * 1024),
            zlib: Decompress::new(true),
        }
    }

    /// Appends raw bytes received from the network into the internal buffer.
    ///
    /// # Arguments
    ///
    /// * `data` - Received bytes.
    pub fn feed(&mut self, data: &[u8]) {
        self.buf.extend_from_slice(data);
    }

    /// Tries to extract one complete, decompressed frame payload.
    ///
    /// Returns `Ok(None)` if the buffer holds an incomplete frame.
    /// Returns `Ok(Some(payload))` when a full frame is available.
    /// An empty `payload` is a valid "empty tick" (no server commands this tick).
    ///
    /// # Returns
    ///
    /// * `Ok(None)` — not enough data yet.
    /// * `Ok(Some(payload))` — complete frame payload (may be empty).
    /// * `Err` — malformed length header.
    pub fn next_frame_payload(&mut self) -> anyhow::Result<Option<Vec<u8>>> {
        if self.buf.len() < 2 {
            return Ok(None);
        }

        let len_flags = u16::from_ne_bytes([self.buf[0], self.buf[1]]);
        let is_compressed = (len_flags & 0x8000) != 0;
        let total_len = (len_flags & 0x7FFF) as usize;

        if total_len < 2 {
            return Err(anyhow!("invalid frame length header: 0x{len_flags:04X}"));
        }
        if self.buf.len() < total_len {
            return Ok(None); // incomplete
        }

        let payload = self.buf[2..total_len].to_vec();
        self.buf.drain(..total_len);

        if payload.is_empty() {
            return Ok(Some(Vec::new()));
        }

        if is_compressed {
            let inflated = self.inflate(&payload)?;
            Ok(Some(inflated))
        } else {
            Ok(Some(payload))
        }
    }

    /// Splits a decompressed frame payload into individual server command byte slices.
    ///
    /// Uses [`ServerCommandType::get_expected_length`] to determine command boundaries.
    ///
    /// # Arguments
    ///
    /// * `payload` - Decompressed frame payload from [`next_frame_payload`].
    ///
    /// # Returns
    ///
    /// * `Ok(commands)` — zero or more raw command byte slices.
    /// * `Err` — malformed command stream.
    pub fn split_commands(payload: &[u8]) -> anyhow::Result<Vec<Vec<u8>>> {
        let mut out = Vec::new();
        let mut idx = 0usize;
        let mut last_setmap_n: i32 = -1;

        while idx < payload.len() {
            let len = ServerCommandType::get_expected_length(&payload[idx..], &mut last_setmap_n)
                .map_err(|e| anyhow!("get_expected_length at idx={idx}: {e}"))?;

            if len == 0 {
                return Err(anyhow!("zero-length command at idx={idx}"));
            }
            let end = idx + len;
            if end > payload.len() {
                return Err(anyhow!(
                    "truncated command at idx={idx}: need {len} bytes but only {} remain",
                    payload.len() - idx
                ));
            }

            out.push(payload[idx..end].to_vec());
            idx = end;
        }

        Ok(out)
    }

    /// Inflates one chunk from the continuous per-connection zlib stream.
    fn inflate(&mut self, input: &[u8]) -> anyhow::Result<Vec<u8>> {
        if input.is_empty() {
            return Ok(Vec::new());
        }

        let mut out = Vec::new();
        let mut in_pos = 0usize;
        let mut scratch = [0u8; 8192];

        loop {
            let before_in = self.zlib.total_in() as usize;
            let before_out = self.zlib.total_out() as usize;

            let status = self
                .zlib
                .decompress(&input[in_pos..], &mut scratch, FlushDecompress::Sync)
                .map_err(|e| anyhow!("zlib inflate failed: {e}"))?;

            let consumed = self.zlib.total_in() as usize - before_in;
            let produced = self.zlib.total_out() as usize - before_out;

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
                return Err(anyhow!("zlib inflate made no progress (truncated input?)"));
            }

            break;
        }

        Ok(out)
    }
}

impl Default for FramedReader {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn framed_reader_incomplete_header() {
        let mut r = FramedReader::new();
        r.feed(&[0x04]); // only 1 byte — need 2 for header
        assert!(r.next_frame_payload().unwrap().is_none());
    }

    #[test]
    fn framed_reader_incomplete_payload() {
        let mut r = FramedReader::new();
        // length = 10 (total), not compressed, only 5 bytes provided
        let total: u16 = 10;
        r.feed(&total.to_ne_bytes());
        r.feed(&[0u8; 3]); // only 3 bytes of payload (need 8)
        assert!(r.next_frame_payload().unwrap().is_none());
    }

    #[test]
    fn framed_reader_empty_tick() {
        let mut r = FramedReader::new();
        // total_len = 2, payload = empty
        let total: u16 = 2;
        r.feed(&total.to_ne_bytes());
        let payload = r.next_frame_payload().unwrap().unwrap();
        assert!(payload.is_empty());
    }

    #[test]
    fn split_commands_empty_payload() {
        let cmds = FramedReader::split_commands(&[]).unwrap();
        assert!(cmds.is_empty());
    }
}
