//! Admin route handlers for live item-slot editing.
//!
//! `GET /admin/world/items` and per-slot GET respond with raw bincode
//! (`application/octet-stream`); per-slot PUT accepts a bincode
//! [`ItemPatch`] body. Listing and coordination endpoints use JSON.

use crate::ApiState;
use crate::admin::types::{
    ErrorResponse, PutWorldEntityResponse, WorldEntityListQuery, WorldEntityListResponse,
    WorldEntityReloadRequest, WorldEntityReloadResponse, WorldEntityReloadStatusResponse,
    WorldEntitySummary, WorldEntityVersionResponse,
};
use axum::Json;
use axum::body::Bytes;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use log::{info, warn};
use mag_core::item_store::{
    self, ITEM_PATCH_QUEUE_KEY, ITEM_PATCH_REQUEST_KEY, ITEM_SLOT_COUNT, ITEM_VERSION_KEY,
    ItemPatch, ItemStoreError,
};
use mag_core::string_operations;
use mag_core::types::Item;
use rand::RngCore;
use rand::rngs::OsRng;
use redis::AsyncCommands;
use redis::pipe;
use std::time::{SystemTime, UNIX_EPOCH};

/// Maximum number of `GET` commands to pipeline in a single round-trip.
const PIPELINE_BATCH_SIZE: usize = 4096;

/// TTL applied to reload-request payloads written to KeyDB.
const RELOAD_REQUEST_TTL_SECS: u64 = 30;

/// Default `limit` for the listing endpoint when the caller does not supply one.
const DEFAULT_LIST_LIMIT: usize = 256;

/// Hard cap on `limit` for the listing endpoint.
const MAX_LIST_LIMIT: usize = 4096;

// ---------------------------------------------------------------------------
//  Bulk item fetch
// ---------------------------------------------------------------------------

/// GET `/admin/world/items` — returns bincode `Vec<Item>` in slot order.
pub(crate) async fn get_items_bulk(State(state): State<ApiState>) -> Response {
    let total = ITEM_SLOT_COUNT;
    let mut con = state.con.clone();
    let mut items: Vec<Item> = Vec::with_capacity(total);

    for batch_start in (0..total).step_by(PIPELINE_BATCH_SIZE) {
        let batch_end = (batch_start + PIPELINE_BATCH_SIZE).min(total);
        let mut pipeline = pipe();
        for idx in batch_start..batch_end {
            pipeline.cmd("GET").arg(item_store::item_key(idx));
        }

        let bytes_batch: Vec<Option<Vec<u8>>> =
            match pipeline.query_async::<Vec<Option<Vec<u8>>>>(&mut con).await {
                Ok(v) => v,
                Err(e) => {
                    warn!("admin get_items_bulk pipeline failed: {}", e);
                    return internal_error("keydb_error", "Failed to read items");
                }
            };

        for (rel, bytes_opt) in bytes_batch.into_iter().enumerate() {
            let abs = batch_start + rel;
            match bytes_opt {
                Some(bytes) => match Item::from_bytes(&bytes) {
                    Some(item) => items.push(item),
                    None => {
                        warn!("admin get_items_bulk decode failed for slot {}", abs);
                        return internal_error("decode_error", "Failed to decode item");
                    }
                },
                None => {
                    return (
                        StatusCode::NOT_FOUND,
                        Json(ErrorResponse::new(
                            "not_seeded",
                            format!(
                                "Missing item slot {}; the world snapshot has not been seeded into KeyDB",
                                abs
                            ),
                        )),
                    )
                        .into_response();
                }
            }
        }
    }

    let body = match bincode::encode_to_vec(&items, bincode::config::standard()) {
        Ok(b) => b,
        Err(e) => {
            warn!("admin get_items_bulk encode Vec<Item> failed: {}", e);
            return internal_error("encode_error", "Failed to encode items");
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
//  Listing
// ---------------------------------------------------------------------------

/// GET `/admin/world/items/list?from=&limit=` — paginated JSON summaries.
pub(crate) async fn list_items(
    State(state): State<ApiState>,
    Query(q): Query<WorldEntityListQuery>,
) -> Response {
    let total = ITEM_SLOT_COUNT;
    let from = q.from.unwrap_or(0);
    if from >= total {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse::new(
                "out_of_range",
                format!("from={} exceeds total={}", from, total),
            )),
        )
            .into_response();
    }
    let limit = q.limit.unwrap_or(DEFAULT_LIST_LIMIT).min(MAX_LIST_LIMIT);
    let end = (from + limit).min(total);
    let count = end - from;

    let mut con = state.con.clone();
    let mut summaries: Vec<WorldEntitySummary> = Vec::with_capacity(count);

    for batch_start in (from..end).step_by(PIPELINE_BATCH_SIZE) {
        let batch_end = (batch_start + PIPELINE_BATCH_SIZE).min(end);
        let mut pipeline = pipe();
        for idx in batch_start..batch_end {
            pipeline.cmd("GET").arg(item_store::item_key(idx));
        }

        let bytes_batch: Vec<Option<Vec<u8>>> =
            match pipeline.query_async::<Vec<Option<Vec<u8>>>>(&mut con).await {
                Ok(v) => v,
                Err(e) => {
                    warn!("admin list_items pipeline failed: {}", e);
                    return internal_error("keydb_error", "Failed to read items");
                }
            };

        for (rel, bytes_opt) in bytes_batch.into_iter().enumerate() {
            let abs = batch_start + rel;
            match bytes_opt {
                Some(bytes) => match Item::from_bytes(&bytes) {
                    Some(item) => summaries.push(item_summary(abs, &item)),
                    None => {
                        warn!("admin list_items decode failed for slot {}", abs);
                        return internal_error("decode_error", "Failed to decode item");
                    }
                },
                None => {
                    return (
                        StatusCode::NOT_FOUND,
                        Json(ErrorResponse::new(
                            "not_seeded",
                            format!("Missing item slot {}", abs),
                        )),
                    )
                        .into_response();
                }
            }
        }
    }

    Json(WorldEntityListResponse {
        total,
        from,
        count,
        items: summaries,
    })
    .into_response()
}

fn item_summary(id: usize, item: &Item) -> WorldEntitySummary {
    WorldEntitySummary {
        id,
        used: item.used != 0,
        name: string_operations::c_string_to_str(&item.name).to_owned(),
        reference: string_operations::c_string_to_str(&item.reference).to_owned(),
    }
}

// ---------------------------------------------------------------------------
//  Single-slot GET
// ---------------------------------------------------------------------------

/// GET `/admin/world/items/{id}` — returns raw bincode `Item` bytes.
pub(crate) async fn get_item(State(state): State<ApiState>, Path(id): Path<usize>) -> Response {
    if let Err(e) = item_store::validate_item_index(id) {
        return item_error_response(e);
    }
    let key = item_store::item_key(id);
    let mut con = state.con.clone();
    let bytes: Option<Vec<u8>> = match con.get(&key).await {
        Ok(v) => v,
        Err(e) => {
            warn!("admin get_item GET {} failed: {}", key, e);
            return internal_error("keydb_error", "Failed to read item");
        }
    };

    let bytes = match bytes {
        Some(b) => b,
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse::new(
                    "not_found",
                    format!("Item slot {} has no stored bytes", id),
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
//  Single-slot PUT
// ---------------------------------------------------------------------------

/// PUT `/admin/world/items/{id}` — body is bincode [`ItemPatch`].
pub(crate) async fn put_item(
    State(state): State<ApiState>,
    Path(id): Path<usize>,
    body: Bytes,
) -> Response {
    if let Err(e) = item_store::validate_item_index(id) {
        return item_error_response(e);
    }

    let patch = match ItemPatch::from_bytes(&body) {
        Ok(p) => p,
        Err(e) => return item_error_response(e),
    };

    if patch.id as usize != id {
        return item_error_response(ItemStoreError::Mismatch {
            expected: id,
            actual: patch.id as usize,
        });
    }

    let canonical = match patch.to_bytes() {
        Ok(b) => b,
        Err(e) => return item_error_response(e),
    };

    let mut con = state.con.clone();

    let queued: u64 = match con
        .rpush::<_, _, u64>(ITEM_PATCH_QUEUE_KEY, canonical)
        .await
    {
        Ok(v) => v,
        Err(e) => {
            warn!("admin put_item RPUSH failed: {}", e);
            return internal_error("keydb_error", "Failed to enqueue item patch");
        }
    };

    let version: u64 = match con.incr(ITEM_VERSION_KEY, 1_i64).await {
        Ok(v) => v,
        Err(e) => {
            warn!("admin put_item INCR {} failed: {}", ITEM_VERSION_KEY, e);
            return internal_error("keydb_error", "Failed to bump version");
        }
    };

    info!(
        "admin queued item patch {} (version {}, queue depth {})",
        id, version, queued
    );
    Json(PutWorldEntityResponse { version, queued }).into_response()
}

// ---------------------------------------------------------------------------
//  Version
// ---------------------------------------------------------------------------

/// GET `/admin/world/items/version`.
pub(crate) async fn get_items_version(State(state): State<ApiState>) -> Response {
    let mut con = state.con.clone();
    let version: u64 = match con.get::<_, Option<u64>>(ITEM_VERSION_KEY).await {
        Ok(v) => v.unwrap_or(0),
        Err(e) => {
            warn!("admin get_items_version GET failed: {}", e);
            return internal_error("keydb_error", "Failed to read version");
        }
    };
    Json(WorldEntityVersionResponse { version }).into_response()
}

// ---------------------------------------------------------------------------
//  Reload coordination
// ---------------------------------------------------------------------------

/// POST `/admin/world/items/reload`.
pub(crate) async fn request_items_reload(
    State(state): State<ApiState>,
    body: Option<Json<WorldEntityReloadRequest>>,
) -> Response {
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
        .set_ex::<_, _, ()>(ITEM_PATCH_REQUEST_KEY, &payload, RELOAD_REQUEST_TTL_SECS)
        .await
    {
        warn!("admin request_items_reload SET failed: {}", e);
        return internal_error("keydb_error", "Failed to enqueue reload request");
    }

    let _: Result<i64, _> = redis::cmd("PUBLISH")
        .arg(item_store::ITEM_PATCH_PUBSUB_CHANNEL)
        .arg(&payload)
        .query_async(&mut con)
        .await;

    info!("admin enqueued items reload request {}", request_id);
    (
        StatusCode::ACCEPTED,
        Json(WorldEntityReloadResponse { request_id }),
    )
        .into_response()
}

/// Query for [`get_items_reload_status`].
#[derive(Debug, serde::Deserialize)]
pub(crate) struct ReloadStatusQuery {
    request_id: String,
}

/// GET `/admin/world/items/reload/status?request_id=…`.
pub(crate) async fn get_items_reload_status(
    State(state): State<ApiState>,
    Query(q): Query<ReloadStatusQuery>,
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

    let key = item_store::item_patch_status_key(&q.request_id);
    let mut con = state.con.clone();
    let stored: Option<String> = match con.get(&key).await {
        Ok(v) => v,
        Err(e) => {
            warn!("admin get_items_reload_status GET {} failed: {}", key, e);
            return internal_error("keydb_error", "Failed to read status");
        }
    };

    let status = match stored {
        Some(s) if s.starts_with("applied") => "applied",
        Some(_) => "pending",
        None => "pending",
    }
    .to_owned();

    Json(WorldEntityReloadStatusResponse {
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

fn item_error_response(err: ItemStoreError) -> Response {
    match err {
        ItemStoreError::OutOfRange { index, slot_count } => (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse::new(
                "out_of_range",
                format!(
                    "Item slot {} out of range (slot_count {})",
                    index, slot_count
                ),
            )),
        )
            .into_response(),
        ItemStoreError::Mismatch { expected, actual } => (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse::new(
                "id_mismatch",
                format!("Patch id {} does not match URL slot {}", actual, expected),
            )),
        )
            .into_response(),
        ItemStoreError::Decode(msg) => (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse::new("decode_error", msg)),
        )
            .into_response(),
        ItemStoreError::Encode(msg) => internal_error("encode_error", msg),
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
    fn item_error_out_of_range_is_400() {
        let resp = item_error_response(ItemStoreError::OutOfRange {
            index: 999_999,
            slot_count: ITEM_SLOT_COUNT,
        });
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn item_error_mismatch_is_400() {
        let resp = item_error_response(ItemStoreError::Mismatch {
            expected: 1usize,
            actual: 2usize,
        });
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn item_error_decode_is_400() {
        let resp = item_error_response(ItemStoreError::Decode("bad".into()));
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn item_error_encode_is_500() {
        let resp = item_error_response(ItemStoreError::Encode("oops".into()));
        assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }
}
