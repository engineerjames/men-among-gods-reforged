use std::io::Read;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use flate2::read::GzDecoder;
use log::{error, info, warn};
use mag_core::types::api::{
    NetworkTestProbeRequest, NetworkTestProbeResponse, NetworkTestSummaryRequest,
    NetworkTestSummaryResponse, UploadClientLogRequest, UploadClientLogResponse,
};

use crate::{ApiState, auth_extractor::AuthUser, pipelines};

/// Hard-coded diagnostics upload directory inside the API container.
const DIAG_UPLOAD_DIR: &str = "/var/mag/diag-uploads";
/// Maximum accepted size for the base64 field.
///
/// This is sized to safely carry the worst-case gzip+base64 expansion of an
/// 8 MiB plain-text log file plus a small JSON wrapper.
pub(crate) const MAX_COMPRESSED_LOG_B64_LEN: usize = 12 * 1024 * 1024;
/// Maximum accepted decompressed log size in bytes.
pub(crate) const MAX_DECOMPRESSED_LOG_BYTES: usize = 8 * 1024 * 1024;
/// Maximum supported diagnostics run ID length.
const MAX_RUN_ID_LEN: usize = 64;
/// Maximum accepted decoded network-test payload size in bytes.
const MAX_NETWORK_TEST_PAYLOAD_BYTES: usize = 256;

/// Character ownership lookup outcome for diagnostics endpoints.
enum CharacterAccess {
    /// Character exists and belongs to the authenticated account.
    Owned(String),
    /// Character is missing or belongs to a different account.
    Unauthorized,
    /// The ownership check failed unexpectedly.
    Internal,
}

/// Handles one diagnostics network-test probe request.
///
/// # Arguments
///
/// * `payload` - Character id, run id, and sample index for this probe.
///
/// # Returns
///
/// * `(200, server_unix_ms)` on success.
/// * `(400, error)` on invalid request data.
pub(crate) async fn network_test_probe(
    State(state): State<ApiState>,
    auth_user: AuthUser,
    Json(payload): Json<NetworkTestProbeRequest>,
) -> (StatusCode, Json<NetworkTestProbeResponse>) {
    if !is_valid_run_id(&payload.run_id) {
        return (
            StatusCode::BAD_REQUEST,
            Json(NetworkTestProbeResponse {
                server_unix_ms: None,
                server_payload_b64: None,
                error: Some("run_id is required and must be <= 64 [A-Za-z0-9_-] chars".to_owned()),
            }),
        );
    }

    let client_payload = match decode_probe_payload(&payload.client_payload_b64) {
        Ok(bytes) => bytes,
        Err(err) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(NetworkTestProbeResponse {
                    server_unix_ms: None,
                    server_payload_b64: None,
                    error: Some(err),
                }),
            );
        }
    };

    if payload.requested_server_payload_bytes == 0
        || usize::from(payload.requested_server_payload_bytes) > MAX_NETWORK_TEST_PAYLOAD_BYTES
    {
        return (
            StatusCode::BAD_REQUEST,
            Json(NetworkTestProbeResponse {
                server_unix_ms: None,
                server_payload_b64: None,
                error: Some("requested_server_payload_bytes must be 1..=256".to_owned()),
            }),
        );
    }

    let character_name = match resolve_owned_character_name(
        &state,
        auth_user.account_id,
        payload.character_id,
    )
    .await
    {
        CharacterAccess::Owned(name) => name,
        CharacterAccess::Unauthorized => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(NetworkTestProbeResponse {
                    server_unix_ms: None,
                    server_payload_b64: None,
                    error: Some("Unauthorized".to_owned()),
                }),
            );
        }
        CharacterAccess::Internal => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(NetworkTestProbeResponse {
                    server_unix_ms: None,
                    server_payload_b64: None,
                    error: Some("server error".to_owned()),
                }),
            );
        }
    };

    let server_payload = build_probe_payload(
        payload.sample_index,
        usize::from(payload.requested_server_payload_bytes),
    );

    info!(
        "diag network test probe: account_id={} character_id={} character_name={} run_id={} sample_index={} client_payload_bytes={} server_payload_bytes={}",
        auth_user.account_id,
        payload.character_id,
        character_name,
        payload.run_id,
        payload.sample_index,
        client_payload.len(),
        server_payload.len()
    );

    (
        StatusCode::OK,
        Json(NetworkTestProbeResponse {
            server_unix_ms: Some(current_unix_ms()),
            server_payload_b64: Some(STANDARD.encode(server_payload)),
            error: None,
        }),
    )
}

/// Receives and logs final diagnostics network-test summary metrics.
///
/// # Arguments
///
/// * `state` - Shared API state.
/// * `payload` - Completed run summary metrics.
///
/// # Returns
///
/// * `(200, accepted=true)` on success.
/// * `(400, error)` on invalid request data.
/// * `(404, error)` when the character id does not exist.
/// * `(500, error)` on unexpected internal failures.
pub(crate) async fn network_test_summary(
    auth_user: AuthUser,
    State(state): State<ApiState>,
    Json(payload): Json<NetworkTestSummaryRequest>,
) -> (StatusCode, Json<NetworkTestSummaryResponse>) {
    if !is_valid_run_id(&payload.run_id) {
        return (
            StatusCode::BAD_REQUEST,
            Json(NetworkTestSummaryResponse {
                accepted: false,
                error: Some("run_id is required and must be <= 64 [A-Za-z0-9_-] chars".to_owned()),
            }),
        );
    }

    if payload.summary.total_samples == 0 {
        return (
            StatusCode::BAD_REQUEST,
            Json(NetworkTestSummaryResponse {
                accepted: false,
                error: Some("total_samples must be > 0".to_owned()),
            }),
        );
    }

    if payload.summary.failed_samples > payload.summary.total_samples {
        return (
            StatusCode::BAD_REQUEST,
            Json(NetworkTestSummaryResponse {
                accepted: false,
                error: Some("failed_samples cannot exceed total_samples".to_owned()),
            }),
        );
    }

    let character_name = match resolve_owned_character_name(
        &state,
        auth_user.account_id,
        payload.character_id,
    )
    .await
    {
        CharacterAccess::Owned(name) => name,
        CharacterAccess::Unauthorized => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(NetworkTestSummaryResponse {
                    accepted: false,
                    error: Some("Unauthorized".to_owned()),
                }),
            );
        }
        CharacterAccess::Internal => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(NetworkTestSummaryResponse {
                    accepted: false,
                    error: Some("server error".to_owned()),
                }),
            );
        }
    };

    info!(
        "diag network test summary: account_id={} character_id={} character_name={} run_id={} duration_ms={} total_samples={} failed_samples={} min_rtt_ms={:?} avg_rtt_ms={:?} max_rtt_ms={:?} jitter_ms={:?} quality={}",
        auth_user.account_id,
        payload.character_id,
        character_name,
        payload.run_id,
        payload.summary.duration_ms,
        payload.summary.total_samples,
        payload.summary.failed_samples,
        payload.summary.min_rtt_ms,
        payload.summary.avg_rtt_ms,
        payload.summary.max_rtt_ms,
        payload.summary.jitter_ms,
        payload.summary.quality_rating
    );

    (
        StatusCode::OK,
        Json(NetworkTestSummaryResponse {
            accepted: true,
            error: None,
        }),
    )
}

/// Receives a compressed client log upload and stores it on disk.
///
/// # Arguments
///
/// * `state` - Shared API state.
/// * `payload` - Character id and base64 encoded gzipped log bytes.
///
/// # Returns
///
/// * `(200, saved_file)` when the log is accepted and written.
/// * `(400, error)` when the payload is invalid.
/// * `(404, error)` when the character id does not exist.
/// * `(500, error)` on unexpected internal failures.
pub(crate) async fn upload_client_log(
    auth_user: AuthUser,
    State(state): State<ApiState>,
    Json(payload): Json<UploadClientLogRequest>,
) -> (StatusCode, Json<UploadClientLogResponse>) {
    if payload.compressed_log_b64.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(UploadClientLogResponse {
                saved_file: None,
                error: Some("compressed_log_b64 is required".to_owned()),
            }),
        );
    }

    if payload.compressed_log_b64.len() > MAX_COMPRESSED_LOG_B64_LEN {
        return (
            StatusCode::BAD_REQUEST,
            Json(UploadClientLogResponse {
                saved_file: None,
                error: Some("compressed log payload is too large".to_owned()),
            }),
        );
    }

    let compressed_bytes = match STANDARD.decode(payload.compressed_log_b64.as_bytes()) {
        Ok(value) => value,
        Err(err) => {
            warn!("Diagnostics upload rejected: invalid base64: {err}");
            return (
                StatusCode::BAD_REQUEST,
                Json(UploadClientLogResponse {
                    saved_file: None,
                    error: Some("compressed_log_b64 must be valid base64".to_owned()),
                }),
            );
        }
    };

    let mut decoder = GzDecoder::new(compressed_bytes.as_slice());
    let mut decompressed = Vec::new();
    if let Err(err) = decoder.read_to_end(&mut decompressed) {
        warn!("Diagnostics upload rejected: invalid gzip payload: {err}");
        return (
            StatusCode::BAD_REQUEST,
            Json(UploadClientLogResponse {
                saved_file: None,
                error: Some("compressed_log_b64 must contain valid gzip bytes".to_owned()),
            }),
        );
    }

    if decompressed.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(UploadClientLogResponse {
                saved_file: None,
                error: Some("decompressed log payload is empty".to_owned()),
            }),
        );
    }

    if decompressed.len() > MAX_DECOMPRESSED_LOG_BYTES {
        return (
            StatusCode::BAD_REQUEST,
            Json(UploadClientLogResponse {
                saved_file: None,
                error: Some("decompressed log payload is too large".to_owned()),
            }),
        );
    }

    let character_name = match resolve_owned_character_name(
        &state,
        auth_user.account_id,
        payload.character_id,
    )
    .await
    {
        CharacterAccess::Owned(name) => name,
        CharacterAccess::Unauthorized => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(UploadClientLogResponse {
                    saved_file: None,
                    error: Some("Unauthorized".to_owned()),
                }),
            );
        }
        CharacterAccess::Internal => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(UploadClientLogResponse {
                    saved_file: None,
                    error: Some("server error".to_owned()),
                }),
            );
        }
    };

    let safe_character_name = sanitize_filename_component(&character_name);
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let output_file_name = format!("{}_{}.log", safe_character_name, timestamp);
    let output_path = Path::new(DIAG_UPLOAD_DIR).join(&output_file_name);

    if let Err(err) = std::fs::create_dir_all(DIAG_UPLOAD_DIR) {
        error!("Diagnostics upload failed creating output dir: {err}");
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(UploadClientLogResponse {
                saved_file: None,
                error: Some("server error".to_owned()),
            }),
        );
    }

    if let Err(err) = std::fs::write(&output_path, &decompressed) {
        error!(
            "Diagnostics upload failed writing {}: {err}",
            output_path.display()
        );
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(UploadClientLogResponse {
                saved_file: None,
                error: Some("server error".to_owned()),
            }),
        );
    }

    (
        StatusCode::OK,
        Json(UploadClientLogResponse {
            saved_file: Some(output_file_name),
            error: None,
        }),
    )
}

/// Resolves a character name only when the character belongs to the authenticated account.
async fn resolve_owned_character_name(
    state: &ApiState,
    account_id: u64,
    character_id: u64,
) -> CharacterAccess {
    let mut con = state.con.clone();
    let owner_id = match pipelines::get_character_account_id(&mut con, character_id).await {
        Ok(value) => value,
        Err(err) => {
            error!("Diagnostics ownership check failed during owner lookup: {err}");
            return CharacterAccess::Internal;
        }
    };

    if owner_id != Some(account_id) {
        warn!(
            "Diagnostics request rejected: account {} does not own character {}",
            account_id, character_id
        );
        return CharacterAccess::Unauthorized;
    }

    match pipelines::get_character_name(&mut con, character_id).await {
        Ok(Some(name)) if !name.trim().is_empty() => CharacterAccess::Owned(name),
        Ok(_) => {
            warn!(
                "Diagnostics request rejected: owned character {} had no stored name",
                character_id
            );
            CharacterAccess::Unauthorized
        }
        Err(err) => {
            error!("Diagnostics ownership check failed during character lookup: {err}");
            CharacterAccess::Internal
        }
    }
}

/// Decodes and validates a base64-encoded probe payload.
fn decode_probe_payload(value: &str) -> Result<Vec<u8>, String> {
    if value.trim().is_empty() {
        return Err("client_payload_b64 is required".to_owned());
    }

    let bytes = STANDARD
        .decode(value.as_bytes())
        .map_err(|_| "client_payload_b64 must be valid base64".to_owned())?;
    if bytes.is_empty() {
        return Err("decoded client payload must not be empty".to_owned());
    }
    if bytes.len() > MAX_NETWORK_TEST_PAYLOAD_BYTES {
        return Err("decoded client payload is too large".to_owned());
    }
    Ok(bytes)
}

/// Builds a deterministic response payload for a network-test probe.
fn build_probe_payload(sample_index: u32, size: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(size);
    for offset in 0..size {
        let value = sample_index.wrapping_mul(31).wrapping_add(offset as u32) as u8;
        out.push(value ^ 0x5a);
    }
    out
}

/// Sanitizes a string for safe use in generated file names.
///
/// # Arguments
///
/// * `value` - Raw value to sanitize.
///
/// # Returns
///
/// * Safe file-name component containing only `[A-Za-z0-9_-]`.
fn sanitize_filename_component(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for c in value.chars() {
        if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
            out.push(c);
        } else {
            out.push('_');
        }
    }

    let trimmed = out.trim_matches('_');
    if trimmed.is_empty() {
        "character".to_owned()
    } else {
        trimmed.to_owned()
    }
}

/// Returns current unix timestamp in milliseconds.
fn current_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis().min(u128::from(u64::MAX)) as u64)
        .unwrap_or(0)
}

/// Validates diagnostics run IDs used to correlate probe/summary events.
fn is_valid_run_id(value: &str) -> bool {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.len() > MAX_RUN_ID_LEN {
        return false;
    }
    trimmed
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

#[cfg(test)]
mod tests {
    use super::{
        build_probe_payload, decode_probe_payload, is_valid_run_id, sanitize_filename_component,
    };
    use base64::Engine;
    use base64::engine::general_purpose::STANDARD;

    #[test]
    fn sanitize_filename_component_replaces_unsafe_chars() {
        assert_eq!(
            sanitize_filename_component("Name With Spaces"),
            "Name_With_Spaces"
        );
        assert_eq!(sanitize_filename_component("A/B\\C"), "A_B_C");
    }

    #[test]
    fn sanitize_filename_component_fallback_when_empty() {
        assert_eq!(sanitize_filename_component("***"), "character");
    }

    #[test]
    fn run_id_validation_accepts_simple_ids() {
        assert!(is_valid_run_id("network-test-001"));
        assert!(is_valid_run_id("ABC_123"));
    }

    #[test]
    fn run_id_validation_rejects_invalid_values() {
        assert!(!is_valid_run_id(""));
        assert!(!is_valid_run_id("contains space"));
        assert!(!is_valid_run_id("symbols!"));
    }

    #[test]
    fn decode_probe_payload_rejects_invalid_values() {
        assert!(decode_probe_payload("").is_err());
        assert!(decode_probe_payload("***").is_err());
    }

    #[test]
    fn build_probe_payload_is_deterministic() {
        let a = build_probe_payload(7, 16);
        let b = build_probe_payload(7, 16);
        let c = build_probe_payload(8, 16);

        assert_eq!(a, b);
        assert_ne!(a, c);
        assert_eq!(a.len(), 16);
    }

    #[test]
    fn decode_probe_payload_accepts_valid_base64() {
        let encoded = STANDARD.encode([1_u8, 2, 3, 4]);
        assert_eq!(decode_probe_payload(&encoded).unwrap(), vec![1, 2, 3, 4]);
    }
}
