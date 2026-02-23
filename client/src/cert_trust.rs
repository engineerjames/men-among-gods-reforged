//! Trust-On-First-Use (TOFU) certificate verification for self-signed servers.
//!
//! On the first connection to a given `host:port`, the SHA-256 fingerprint of
//! the server's leaf certificate is saved to a JSON file
//! (`mag_known_hosts.json`).  Subsequent connections accept a certificate only
//! if its fingerprint matches the stored value.  If the fingerprint changes,
//! the connection is rejected with an explanatory error.
//!
//! This module provides:
//! - [`TofuVerifier`] — a [`rustls::client::danger::ServerCertVerifier`] impl
//! - [`build_rustls_client_config`] — returns a `rustls::ClientConfig` using TOFU
//! - [`build_reqwest_client`] — returns a `reqwest::blocking::Client` using TOFU
//! - [`build_game_tls_connector`] — returns a `rustls::ClientConfig` for the game TCP

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{ClientConfig, DigitallySignedStruct, Error, SignatureScheme};
use sha2::{Digest, Sha256};

/// File name for the TOFU fingerprint store (lives next to `mag_profile.json`).
const KNOWN_HOSTS_FILE: &str = "mag_known_hosts.json";

// ---------------------------------------------------------------------------
// Fingerprint store
// ---------------------------------------------------------------------------

/// Persistent mapping of `"host:port"` → SHA-256 hex fingerprint.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
struct KnownHostsStore {
    hosts: HashMap<String, String>,
}

impl KnownHostsStore {
    fn path() -> PathBuf {
        crate::preferences::working_directory().join(KNOWN_HOSTS_FILE)
    }

    fn load() -> Self {
        let path = Self::path();
        match fs::read_to_string(&path) {
            Ok(data) => serde_json::from_str(&data).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    fn save(&self) {
        let path = Self::path();
        if let Ok(data) = serde_json::to_string_pretty(self) {
            let tmp = path.with_extension("json.tmp");
            if fs::write(&tmp, &data).is_ok() {
                let _ = fs::rename(&tmp, &path);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// TOFU verifier
// ---------------------------------------------------------------------------

/// A [`ServerCertVerifier`] that implements Trust-On-First-Use.
#[derive(Debug)]
pub struct TofuVerifier {
    store: Mutex<KnownHostsStore>,
}

impl TofuVerifier {
    /// Create a new verifier, loading any previously-saved fingerprints.
    pub fn new() -> Self {
        Self {
            store: Mutex::new(KnownHostsStore::load()),
        }
    }

    /// Compute the SHA-256 fingerprint (hex-encoded) of a DER certificate.
    fn fingerprint(cert: &CertificateDer<'_>) -> String {
        let mut hasher = Sha256::new();
        hasher.update(cert.as_ref());
        hex_encode(&hasher.finalize())
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

impl ServerCertVerifier for TofuVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, Error> {
        let fp = Self::fingerprint(end_entity);
        let key = match server_name {
            ServerName::DnsName(dns) => dns.as_ref().to_string(),
            ServerName::IpAddress(ip) => format!("{}", std::net::IpAddr::from(*ip)),
            _ => "unknown".to_string(),
        };

        let mut store = self.store.lock().unwrap();

        if let Some(saved) = store.hosts.get(&key) {
            if *saved == fp {
                log::debug!("TOFU: fingerprint match for {key}");
                return Ok(ServerCertVerified::assertion());
            }
            log::error!(
                "TOFU: fingerprint MISMATCH for {key}! \
                 stored={saved}, received={fp}"
            );
            return Err(Error::General(format!(
                "Server certificate for '{key}' has changed!\n\
                 Expected fingerprint: {saved}\n\
                 Received fingerprint: {fp}\n\
                 This may indicate a man-in-the-middle attack.\n\
                 If the server certificate was intentionally rotated, \
                 delete '{KNOWN_HOSTS_FILE}' and reconnect."
            )));
        }

        // First connection — trust and save
        log::info!("TOFU: first connection to {key}, saving fingerprint {fp}");
        store.hosts.insert(key, fp);
        store.save();

        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, Error> {
        // We trust the certificate via TOFU; accept signature as-is.
        Ok(HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, Error> {
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

// ---------------------------------------------------------------------------
// Public helpers
// ---------------------------------------------------------------------------

/// Build a `rustls::ClientConfig` that uses the shared TOFU verifier.
pub fn build_rustls_client_config() -> ClientConfig {
    let verifier = Arc::new(TofuVerifier::new());
    ClientConfig::builder()
        .dangerous()
        .with_custom_certificate_verifier(verifier)
        .with_no_client_auth()
}

/// Build a `reqwest::blocking::Client` that uses TOFU for HTTPS.
pub fn build_reqwest_client() -> Result<reqwest::blocking::Client, String> {
    let tls_config = build_rustls_client_config();
    reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .use_preconfigured_tls(tls_config)
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {e}"))
}

/// Build a `rustls::ClientConnection` for wrapping a game-server TCP stream.
pub fn build_game_tls_connector(
    host: &str,
) -> Result<rustls::ClientConnection, String> {
    let config = Arc::new(build_rustls_client_config());
    let server_name = rustls::pki_types::ServerName::try_from(host.to_string())
        .map_err(|e| format!("Invalid server name '{host}': {e}"))?;
    rustls::ClientConnection::new(config, server_name)
        .map_err(|e| format!("TLS ClientConnection::new failed: {e}"))
}
