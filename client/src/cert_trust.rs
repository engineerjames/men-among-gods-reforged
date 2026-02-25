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

use crate::preferences;

fn ensure_crypto_provider_installed() -> Result<(), String> {
    if rustls::crypto::CryptoProvider::get_default().is_some() {
        return Ok(());
    }

    if rustls::crypto::ring::default_provider()
        .install_default()
        .is_ok()
    {
        return Ok(());
    }

    if rustls::crypto::CryptoProvider::get_default().is_some() {
        return Ok(());
    }

    Err("Failed to initialize rustls crypto provider".to_string())
}

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
        crate::preferences::known_hosts_file_path()
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
                 delete '{}' and reconnect.",
                preferences::known_hosts_file_path().display()
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
    if let Err(err) = ensure_crypto_provider_installed() {
        panic!("{err}");
    }

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
pub fn build_game_tls_connector(host: &str) -> Result<rustls::ClientConnection, String> {
    let config = Arc::new(build_rustls_client_config());
    let server_name = rustls::pki_types::ServerName::try_from(host.to_string())
        .map_err(|e| format!("Invalid server name '{host}': {e}"))?;
    rustls::ClientConnection::new(config, server_name)
        .map_err(|e| format!("TLS ClientConnection::new failed: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};
    use std::sync::{Mutex, OnceLock};
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    fn cwd_test_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn unique_test_dir(prefix: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::from_secs(0))
            .as_nanos();
        std::env::temp_dir().join(format!(
            "mag-reforged-{prefix}-{}-{nanos}",
            std::process::id()
        ))
    }

    fn unique_test_host(prefix: &str) -> String {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::from_secs(0))
            .as_nanos();
        format!("{prefix}-{}-{nanos}.local", std::process::id())
    }

    fn with_temp_cwd<F>(prefix: &str, f: F)
    where
        F: FnOnce(&Path),
    {
        let _guard = cwd_test_lock().lock().unwrap();
        let original_cwd = std::env::current_dir().unwrap();
        let test_dir = unique_test_dir(prefix);
        std::fs::create_dir_all(&test_dir).unwrap();
        std::env::set_current_dir(&test_dir).unwrap();

        f(&test_dir);

        std::env::set_current_dir(original_cwd).unwrap();
        let _ = std::fs::remove_dir_all(&test_dir);
    }

    #[test]
    fn installs_crypto_provider_idempotently() {
        ensure_crypto_provider_installed().unwrap();
        ensure_crypto_provider_installed().unwrap();
        assert!(rustls::crypto::CryptoProvider::get_default().is_some());
    }

    #[test]
    fn hex_encode_outputs_lowercase_hex() {
        assert_eq!(hex_encode(&[0x00, 0x0f, 0xab, 0xff]), "000fabff");
    }

    #[test]
    fn tofu_verifier_accepts_first_seen_and_matching_cert() {
        with_temp_cwd("tofu-match", |_| {
            let verifier = TofuVerifier::new();
            let cert = CertificateDer::from(vec![1, 2, 3, 4, 5]);
            let server_name = ServerName::try_from(unique_test_host("tofu-accept")).unwrap();

            let first = verifier.verify_server_cert(
                &cert,
                &[],
                &server_name,
                &[],
                UnixTime::since_unix_epoch(Duration::from_secs(0)),
            );
            assert!(first.is_ok());

            let second = verifier.verify_server_cert(
                &cert,
                &[],
                &server_name,
                &[],
                UnixTime::since_unix_epoch(Duration::from_secs(1)),
            );
            assert!(second.is_ok());
        });
    }

    #[test]
    fn tofu_verifier_rejects_changed_cert_for_same_host() {
        with_temp_cwd("tofu-mismatch", |_| {
            let verifier = TofuVerifier::new();
            let cert_a = CertificateDer::from(vec![10, 20, 30]);
            let cert_b = CertificateDer::from(vec![40, 50, 60]);
            let server_name = ServerName::try_from(unique_test_host("tofu-mismatch")).unwrap();

            verifier
                .verify_server_cert(
                    &cert_a,
                    &[],
                    &server_name,
                    &[],
                    UnixTime::since_unix_epoch(Duration::from_secs(0)),
                )
                .unwrap();

            let changed = verifier.verify_server_cert(
                &cert_b,
                &[],
                &server_name,
                &[],
                UnixTime::since_unix_epoch(Duration::from_secs(1)),
            );
            assert!(changed.is_err());
        });
    }
}
