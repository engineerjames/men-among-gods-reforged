//! Admin route handlers for live map-tile editing.
//!
//! The bulk fetch (`GET /admin/world/map`) and per-tile GET respond with raw
//! bincode (`application/octet-stream`); per-tile PUT accepts a bincode
//! [`MapPatch`] body. Coordination endpoints use JSON.

use crate::ApiState;
use crate::admin::types::{
    ErrorResponse, MapReloadRequest, MapReloadResponse, MapReloadStatusResponse,
    MapVersionResponse, PutMapTileResponse,
};
use axum::Json;
use axum::body::Bytes;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use log::{info, warn};
use mag_core::constants::{SERVER_MAPX, SERVER_MAPY};
use mag_core::map_store::{
    self, MAP_PATCH_QUEUE_KEY, MAP_PATCH_REQUEST_KEY, MAP_VERSION_KEY, MapPatch, MapStoreError,
};
use mag_core::types::Map;
use rand::RngCore;
use rand::rngs::OsRng;
use redis::AsyncCommands;
use redis::pipe;
use std::time::{SystemTime, UNIX_EPOCH};

/// Maximum number of `MGET` commands to pipeline in a single round-trip.
const PIPELINE_BATCH_SIZE: usize = 4096;

/// TTL applied to reload-request payloads written to KeyDB.
const MAP_RELOAD_REQUEST_TTL_SECS: u64 = 30;

// ---------------------------------------------------------------------------
//  Bulk map fetch
// ---------------------------------------------------------------------------

/// GET `/admin/world/map` — returns bincode `Vec<Map>` in row-major order.
pub(crate) async fn get_map_bulk(State(state): State<ApiState>) -> Response {
    let map_x = SERVER_MAPX as usize;
    let map_y = SERVER_MAPY as usize;
    let total = map_x * map_y;

    let mut con = state.con.clone();
    let mut tiles: Vec<Map> = Vec::with_capacity(total);

    for batch_start in (0..total).step_by(PIPELINE_BATCH_SIZE) {
        let batch_end = (batch_start + PIPELINE_BATCH_SIZE).min(total);
        let mut pipeline = pipe();
        for linear in batch_start..batch_end {
            let x = linear % map_x;
            let y = linear / map_x;
            pipeline.cmd("GET").arg(map_store::map_key(x, y));
        }

        let bytes_batch: Vec<Option<Vec<u8>>> =
            match pipeline.query_async::<Vec<Option<Vec<u8>>>>(&mut con).await {
                Ok(v) => v,
                Err(e) => {
                    warn!("admin get_map_bulk pipeline failed: {}", e);
                    return internal_error("keydb_error", "Failed to read map tiles");
                }
            };

        for (rel, bytes_opt) in bytes_batch.into_iter().enumerate() {
            let abs = batch_start + rel;
            let x = abs % map_x;
            let y = abs / map_x;
            match bytes_opt {
                Some(bytes) => match Map::from_bytes(&bytes) {
                    Some(tile) => tiles.push(tile),
                    None => {
                        warn!("admin get_map_bulk decode failed for {}:{}", x, y);
                        return internal_error("decode_error", "Failed to decode map tile");
                    }
                },
                None => {
                    // Missing tile means the world has not been seeded; return
                    // 404 with a clear message rather than partial data.
                    return (
                        StatusCode::NOT_FOUND,
                        Json(ErrorResponse::new(
                            "not_seeded",
                            format!(
                                "Missing map tile {}:{}; the world snapshot has not been seeded into KeyDB",
                                x, y
                            ),
                        )),
                    )
                        .into_response();
                }
            }
        }
    }

    let body = match bincode::encode_to_vec(&tiles, bincode::config::standard()) {
        Ok(b) => b,
        Err(e) => {
            warn!("admin get_map_bulk encode Vec<Map> failed: {}", e);
            return internal_error("encode_error", "Failed to encode map");
        }
    };

    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/octet-stream"),
    );
    (StatusCode::OK, headers, body).into_response()
}

// ---------------------------------------------------------------------------
//  Single-tile GET
// ---------------------------------------------------------------------------

/// GET `/admin/world/map/{x}/{y}` — returns raw bincode `Map` bytes.
pub(crate) async fn get_map_tile(
    State(state): State<ApiState>,
    Path((x, y)): Path<(usize, usize)>,
) -> Response {
    if let Err(e) = map_store::validate_map_coords(x, y) {
        return map_error_response(e);
    }
    let key = map_store::map_key(x, y);
    let mut con = state.con.clone();
    let bytes: Option<Vec<u8>> = match con.get(&key).await {
        Ok(v) => v,
        Err(e) => {
            warn!("admin get_map_tile GET {} failed: {}", key, e);
            return internal_error("keydb_error", "Failed to read tile");
        }
    };

    let bytes = match bytes {
        Some(b) => b,
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse::new(
                    "not_found",
                    format!("Map tile {}:{} has no stored bytes", x, y),
                )),
            )
                .into_response();
        }
    };

    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/octet-stream"),
    );
    (StatusCode::OK, headers, bytes).into_response()
}

// ---------------------------------------------------------------------------
//  Single-tile PUT
// ---------------------------------------------------------------------------

/// PUT `/admin/world/map/{x}/{y}` — body is bincode [`MapPatch`].
///
/// The patch is appended to the server-side queue (`RPUSH game:map:patch_queue`)
/// and the version counter is incremented. The running server applies queued
/// patches between ticks, preserving dynamic fields (`ch`, `it`, `light`, …).
pub(crate) async fn put_map_tile(
    State(state): State<ApiState>,
    Path((x, y)): Path<(usize, usize)>,
    body: Bytes,
) -> Response {
    if let Err(e) = map_store::validate_map_coords(x, y) {
        return map_error_response(e);
    }

    let patch = match MapPatch::from_bytes(&body) {
        Ok(p) => p,
        Err(e) => return map_error_response(e),
    };

    if (patch.x as usize) != x || (patch.y as usize) != y {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse::new(
                "coord_mismatch",
                format!(
                    "Patch coordinates ({}, {}) do not match URL ({}, {})",
                    patch.x, patch.y, x, y
                ),
            )),
        )
            .into_response();
    }

    // Re-encode to canonical form before queuing so the server consumer sees
    // a fully-validated payload.
    let canonical = match patch.to_bytes() {
        Ok(b) => b,
        Err(e) => return map_error_response(e),
    };

    let mut con = state.con.clone();

    let queued: u64 = match con.rpush::<_, _, u64>(MAP_PATCH_QUEUE_KEY, canonical).await {
        Ok(v) => v,
        Err(e) => {
            warn!("admin put_map_tile RPUSH failed: {}", e);
            return internal_error("keydb_error", "Failed to enqueue map patch");
        }
    };

    let version: u64 = match con.incr(MAP_VERSION_KEY, 1_i64).await {
        Ok(v) => v,
        Err(e) => {
            warn!("admin put_map_tile INCR {} failed: {}", MAP_VERSION_KEY, e);
            return internal_error("keydb_error", "Failed to bump version");
        }
    };

    info!(
        "admin queued map patch ({},{}) (version {}, queue depth {})",
        x, y, version, queued
    );
    Json(PutMapTileResponse { version, queued }).into_response()
}

// ---------------------------------------------------------------------------
//  Version
// ---------------------------------------------------------------------------

/// GET `/admin/world/map/version`.
pub(crate) async fn get_map_version(State(state): State<ApiState>) -> Response {
    let mut con = state.con.clone();
    let version: u64 = match con.get::<_, Option<u64>>(MAP_VERSION_KEY).await {
        Ok(v) => v.unwrap_or(0),
        Err(e) => {
            warn!("admin get_map_version GET failed: {}", e);
            return internal_error("keydb_error", "Failed to read version");
        }
    };
    Json(MapVersionResponse { version }).into_response()
}

// ---------------------------------------------------------------------------
//  Reload coordination
// ---------------------------------------------------------------------------

/// POST `/admin/world/map/reload`.
///
/// Writes a request payload to [`MAP_PATCH_REQUEST_KEY`]; the server's
/// map-patch watcher drains that key, flushes every queued [`MapPatch`] into
/// the in-memory map, and writes an `applied:{ts}` status entry under
/// [`map_store::map_patch_status_key`].
pub(crate) async fn request_map_reload(
    State(state): State<ApiState>,
    body: Option<Json<MapReloadRequest>>,
) -> Response {
    // Body is optional and currently unused; accept either an empty body or
    // an empty JSON object.
    let _ = body;

    let request_id = generate_request_id();
    let requested_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let payload = format!(
        r#"{{"request_id":"{}","requested_at":{}}}"#,
        request_id, requested_at
    );

    let mut con = state.con.clone();
    if let Err(e) = con
        .set_ex::<_, _, ()>(MAP_PATCH_REQUEST_KEY, &payload, MAP_RELOAD_REQUEST_TTL_SECS)
        .await
    {
        warn!("admin request_map_reload SET failed: {}", e);
        return internal_error("keydb_error", "Failed to enqueue reload request");
    }

    let _: Result<i64, _> = redis::cmd("PUBLISH")
        .arg(map_store::MAP_PATCH_PUBSUB_CHANNEL)
        .arg(&payload)
        .query_async(&mut con)
        .await;

    info!("admin enqueued map reload request {}", request_id);
    (StatusCode::ACCEPTED, Json(MapReloadResponse { request_id })).into_response()
}

/// Query for [`get_map_reload_status`].
#[derive(Debug, serde::Deserialize)]
pub(crate) struct MapReloadStatusQuery {
    request_id: String,
}

/// GET `/admin/world/map/reload/status?request_id=…`.
pub(crate) async fn get_map_reload_status(
    State(state): State<ApiState>,
    Query(q): Query<MapReloadStatusQuery>,
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

    let key = map_store::map_patch_status_key(&q.request_id);
    let mut con = state.con.clone();
    let stored: Option<String> = match con.get(&key).await {
        Ok(v) => v,
        Err(e) => {
            warn!("admin get_map_reload_status GET {} failed: {}", key, e);
            return internal_error("keydb_error", "Failed to read status");
        }
    };

    let status = match stored {
        Some(s) if s.starts_with("applied") => "applied",
        Some(_) => "pending",
        None => "pending",
    }
    .to_string();

    Json(MapReloadStatusResponse {
        status,
        request_id: q.request_id,
    })
    .into_response()
}

// ---------------------------------------------------------------------------
//  Helpers
// ---------------------------------------------------------------------------

fn generate_request_id() -> String {
    let mut bytes = [0u8; 12];
    OsRng.fill_bytes(&mut bytes);
    let mut out = String::with_capacity(24);
    for b in bytes {
        out.push_str(&format!("{:02x}", b));
    }
    out
}

fn map_error_response(err: MapStoreError) -> Response {
    match err {
        MapStoreError::OutOfRange { x, y, max_x, max_y } => (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse::new(
                "out_of_range",
                format!(
                    "Map coordinates ({}, {}) out of range (max {}x{})",
                    x, y, max_x, max_y
                ),
            )),
        )
            .into_response(),
        MapStoreError::Decode(msg) => (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse::new("decode_error", msg)),
        )
            .into_response(),
        MapStoreError::Encode(msg) => internal_error("encode_error", msg),
    }
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

    #[test]
    fn generate_request_id_is_24_hex() {
        let id = generate_request_id();
        assert_eq!(id.len(), 24);
        assert!(id.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn map_error_out_of_range_is_400() {
        let resp = map_error_response(MapStoreError::OutOfRange {
            x: 9999,
            y: 0,
            max_x: SERVER_MAPX as usize,
            max_y: SERVER_MAPY as usize,
        });
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn map_error_decode_is_400() {
        let resp = map_error_response(MapStoreError::Decode("bad".into()));
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn map_error_encode_is_500() {
        let resp = map_error_response(MapStoreError::Encode("bad".into()));
        assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }
}
