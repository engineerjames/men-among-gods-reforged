//! Admin route handlers for read-only global game-state inspection.

use crate::ApiState;
use crate::admin::types::{ErrorResponse, GlobalsResponse};
use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use log::warn;
use mag_core::types::Global;
use redis::AsyncCommands;

/// KeyDB key containing the bincode-encoded [`Global`] value.
const GLOBALS_KEY: &str = "game:global";

/// GET `/admin/world/globals` - returns persisted global counters as JSON.
pub(crate) async fn get_globals(State(state): State<ApiState>) -> Response {
    let mut con = state.con.clone();
    let bytes: Option<Vec<u8>> = match con.get(GLOBALS_KEY).await {
        Ok(value) => value,
        Err(error) => {
            warn!("admin get_globals GET {} failed: {}", GLOBALS_KEY, error);
            return internal_error("keydb_error", "Failed to read globals");
        }
    };

    let bytes = match bytes {
        Some(bytes) => bytes,
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse::new(
                    "not_seeded",
                    "Missing globals; the world snapshot has not been seeded into KeyDB",
                )),
            )
                .into_response();
        }
    };

    let global = match Global::from_bytes(&bytes) {
        Some(global) => global,
        None => {
            warn!("admin get_globals decode failed for {}", GLOBALS_KEY);
            return internal_error("decode_error", "Failed to decode globals");
        }
    };

    Json(GlobalsResponse::from(global)).into_response()
}

fn internal_error(code: &str, message: &str) -> Response {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ErrorResponse::new(code, message)),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn globals_response_copies_values() {
        let mut global = Global {
            mdtime: 12,
            mdday: 3,
            mdyear: 2026,
            players_online: 7,
            unique: 1234,
            ..Global::default()
        };
        global.online_per_hour[5] = 42;
        global.max_online_per_hour[5] = 9;
        global.set_dirty(true);

        let response = GlobalsResponse::from(global);

        assert_eq!(response.mdtime, 12);
        assert_eq!(response.mdday, 3);
        assert_eq!(response.mdyear, 2026);
        assert_eq!(response.players_online, 7);
        assert_eq!(response.online_per_hour[5], 42);
        assert_eq!(response.max_online_per_hour[5], 9);
        assert_eq!(response.unique, 1234);
        assert!(response.dirty);
    }
}
