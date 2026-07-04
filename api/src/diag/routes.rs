use std::io::Read;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use flate2::read::GzDecoder;
use log::{error, warn};
use mag_core::types::api::{UploadClientLogRequest, UploadClientLogResponse};

use crate::{ApiState, pipelines};

/// Hard-coded diagnostics upload directory inside the API container.
const DIAG_UPLOAD_DIR: &str = "/var/mag/diag-uploads";
/// Maximum accepted size for the base64 field to avoid oversized payload abuse.
const MAX_COMPRESSED_LOG_B64_LEN: usize = 2 * 1024 * 1024;
/// Maximum accepted decompressed log size in bytes.
const MAX_DECOMPRESSED_LOG_BYTES: usize = 8 * 1024 * 1024;

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

    let mut con = state.con.clone();
    let character_name = match pipelines::get_character_name(&mut con, payload.character_id).await {
        Ok(Some(name)) if !name.trim().is_empty() => name,
        Ok(_) => {
            return (
                StatusCode::NOT_FOUND,
                Json(UploadClientLogResponse {
                    saved_file: None,
                    error: Some("character not found".to_owned()),
                }),
            );
        }
        Err(err) => {
            error!("Diagnostics upload failed during character lookup: {err}");
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

#[cfg(test)]
mod tests {
    use super::sanitize_filename_component;

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
}
