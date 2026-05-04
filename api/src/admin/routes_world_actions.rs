//! Admin endpoints for executing live world actions on the running server.

use crate::ApiState;
use crate::admin::types::ErrorResponse;
use axum::Json;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use log::{info, warn};
use mag_core::world_action_store::{
    STATUS_PENDING, WORLD_ACTION_PUBSUB_CHANNEL, WORLD_ACTION_QUEUE_KEY,
    WORLD_ACTION_STATUS_TTL_SECS, WorldActionKind, WorldActionRequest, WorldActionResponse,
    WorldActionStatusResponse, world_action_status_key,
};
use rand::RngCore;
use rand::rngs::OsRng;
use redis::AsyncCommands;
use std::time::{SystemTime, UNIX_EPOCH};

/// POST `/admin/world/actions`.
pub(crate) async fn request_world_action(
    State(state): State<ApiState>,
    Json(action): Json<WorldActionKind>,
) -> Response {
    let request_id = generate_request_id();
    let requested_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    let action_name = action.name().to_string();
    let request = WorldActionRequest {
        request_id: request_id.clone(),
        action,
        requested_at,
    };
    let bytes = match request.to_bytes() {
        Ok(bytes) => bytes,
        Err(error) => return internal_error("encode_error", error.to_string()),
    };

    let status_key = world_action_status_key(&request_id);
    let status_value = format_status_value(STATUS_PENDING, &action_name, "queued", requested_at);

    let mut con = state.con.clone();
    if let Err(error) = con
        .set_ex::<_, _, ()>(&status_key, status_value, WORLD_ACTION_STATUS_TTL_SECS)
        .await
    {
        warn!(
            "admin request_world_action SET {} failed: {}",
            status_key, error
        );
        return internal_error("keydb_error", "Failed to write action status");
    }

    let queued: Result<i64, _> = redis::cmd("RPUSH")
        .arg(WORLD_ACTION_QUEUE_KEY)
        .arg(bytes)
        .query_async(&mut con)
        .await;
    if let Err(error) = queued {
        warn!(
            "admin request_world_action RPUSH {} failed: {}",
            WORLD_ACTION_QUEUE_KEY, error
        );
        return internal_error("keydb_error", "Failed to enqueue world action");
    }

    let _: Result<i64, _> = redis::cmd("PUBLISH")
        .arg(WORLD_ACTION_PUBSUB_CHANNEL)
        .arg(&request_id)
        .query_async(&mut con)
        .await;

    info!(
        "admin enqueued world action {} ({})",
        request_id, action_name
    );
    (
        StatusCode::ACCEPTED,
        Json(WorldActionResponse {
            request_id,
            action: action_name,
        }),
    )
        .into_response()
}

/// Query for [`get_world_action_status`].
#[derive(Debug, serde::Deserialize)]
pub(crate) struct WorldActionStatusQuery {
    request_id: String,
}

/// GET `/admin/world/actions/status?request_id=…`.
pub(crate) async fn get_world_action_status(
    State(state): State<ApiState>,
    Query(q): Query<WorldActionStatusQuery>,
) -> Response {
    if q.request_id.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse::new(
                "missing_request_id",
                "Provide ?request_id=<id>",
            )),
        )
            .into_response();
    }

    let key = world_action_status_key(&q.request_id);
    let mut con = state.con.clone();
    let stored: Option<String> = match con.get(&key).await {
        Ok(value) => value,
        Err(error) => {
            warn!(
                "admin get_world_action_status GET {} failed: {}",
                key, error
            );
            return internal_error("keydb_error", "Failed to read status");
        }
    };

    Json(parse_status(&q.request_id, stored)).into_response()
}

fn parse_status(request_id: &str, stored: Option<String>) -> WorldActionStatusResponse {
    let Some(raw) = stored else {
        return WorldActionStatusResponse {
            request_id: request_id.to_string(),
            action: String::new(),
            status: STATUS_PENDING.to_string(),
            message: String::new(),
            updated_at: 0,
        };
    };

    let mut parts = raw.splitn(4, '|');
    let status = parts.next().unwrap_or(STATUS_PENDING).to_string();
    let action = parts.next().unwrap_or_default().to_string();
    let updated_at = parts
        .next()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(0);
    let message = parts.next().unwrap_or_default().to_string();

    WorldActionStatusResponse {
        request_id: request_id.to_string(),
        action,
        status,
        message,
        updated_at,
    }
}

fn format_status_value(status: &str, action: &str, message: &str, updated_at: u64) -> String {
    format!(
        "{}|{}|{}|{}",
        status,
        action,
        updated_at,
        message.replace(['|', '\n', '\r'], " ")
    )
}

fn generate_request_id() -> String {
    let mut bytes = [0u8; 12];
    OsRng.fill_bytes(&mut bytes);
    let mut out = String::with_capacity(24);
    for byte in bytes {
        out.push_str(&format!("{:02x}", byte));
    }
    out
}

fn internal_error(code: &str, message: impl Into<String>) -> Response {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ErrorResponse::new(code, message.into())),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use mag_core::world_action_store::{STATUS_APPLIED, STATUS_FAILED};

    #[test]
    fn generate_request_id_is_24_hex() {
        let id = generate_request_id();
        assert_eq!(id.len(), 24);
        assert!(id.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn parse_status_reads_delimited_value() {
        let response = parse_status(
            "abc",
            Some("applied|rebuild_lights|42|map lighting rebuilt".to_string()),
        );
        assert_eq!(response.request_id, "abc");
        assert_eq!(response.status, STATUS_APPLIED);
        assert_eq!(response.action, "rebuild_lights");
        assert_eq!(response.updated_at, 42);
        assert_eq!(response.message, "map lighting rebuilt");
    }

    #[test]
    fn parse_status_missing_defaults_to_pending() {
        let response = parse_status("abc", None);
        assert_eq!(response.status, STATUS_PENDING);
        assert_eq!(response.request_id, "abc");
    }

    #[test]
    fn format_status_value_sanitizes_message() {
        assert_eq!(
            format_status_value(STATUS_FAILED, "wipe_runtime", "bad|thing\nhere", 7),
            "failed|wipe_runtime|7|bad thing here"
        );
    }
}
