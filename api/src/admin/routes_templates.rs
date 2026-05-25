//! Admin route handlers for live template editing.
//!
//! Full templates (`GET`/`PUT /admin/templates/{kind}/{id}`) use raw bincode
//! bytes (`Content-Type: application/octet-stream`) so the API never has to
//! mirror the on-disk schema in JSON. Listing and reload coordination
//! endpoints use JSON.

use crate::ApiState;
use crate::admin::types::{
    ErrorResponse, PutTemplateResponse, ReloadRequest, ReloadResponse, ReloadStatusResponse,
    TemplateListQuery, TemplateListResponse, TemplateSummary,
};
use axum::Json;
use axum::body::Bytes;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use log::{info, warn};
use mag_core::template_store::{
    RELOAD_REQUEST_KEY, TemplateError, TemplateKind, reload_status_key,
};
use mag_core::types::{Character, Item};
use mag_core::{string_operations, template_store};
use rand::RngCore;
use rand::rngs::OsRng;
use redis::AsyncCommands;
use std::time::{SystemTime, UNIX_EPOCH};

/// Default page size when no `limit` is supplied to listing endpoints.
const DEFAULT_LIST_LIMIT: usize = 256;

/// Maximum allowed `limit` to prevent oversized responses.
const MAX_LIST_LIMIT: usize = 4096;

/// TTL applied to reload-request payloads written to KeyDB.
const RELOAD_REQUEST_TTL_SECS: u64 = 30;

// ---------------------------------------------------------------------------
//  Listing
// ---------------------------------------------------------------------------

/// GET `/admin/templates/items`.
pub(crate) async fn list_item_templates(
    State(state): State<ApiState>,
    Query(q): Query<TemplateListQuery>,
) -> Response {
    list_templates(state, q, TemplateKind::Item).await
}

/// GET `/admin/templates/characters`.
pub(crate) async fn list_character_templates(
    State(state): State<ApiState>,
    Query(q): Query<TemplateListQuery>,
) -> Response {
    list_templates(state, q, TemplateKind::Character).await
}

async fn list_templates(state: ApiState, q: TemplateListQuery, kind: TemplateKind) -> Response {
    let total = kind.slot_count();
    let from = q.from.unwrap_or(0).min(total);
    let limit = q
        .limit
        .unwrap_or(DEFAULT_LIST_LIMIT)
        .min(MAX_LIST_LIMIT)
        .min(total.saturating_sub(from));

    let mut con = state.con.clone();
    let mut summaries: Vec<TemplateSummary> = Vec::with_capacity(limit);

    // Build the key list and MGET in one round-trip.
    let keys: Vec<String> = (from..from + limit)
        .map(|idx| match kind {
            TemplateKind::Item => template_store::item_template_key(idx),
            TemplateKind::Character => template_store::character_template_key(idx),
        })
        .collect();

    if !keys.is_empty() {
        let raw: Vec<Option<Vec<u8>>> = match con.mget::<_, Vec<Option<Vec<u8>>>>(keys).await {
            Ok(v) => v,
            Err(e) => {
                warn!("admin list_templates MGET failed: {}", e);
                return internal_error("keydb_error", "Failed to read templates");
            }
        };

        for (offset, bytes_opt) in raw.into_iter().enumerate() {
            let id = from + offset;
            let summary = match bytes_opt {
                Some(bytes) => match kind {
                    TemplateKind::Item => match template_store::decode_item_template(&bytes) {
                        Ok(item) => item_summary(id, &item),
                        Err(e) => {
                            warn!("admin list_templates decode item {}: {}", id, e);
                            continue;
                        }
                    },
                    TemplateKind::Character => {
                        match template_store::decode_character_template(&bytes) {
                            Ok(ch) => character_summary(id, &ch),
                            Err(e) => {
                                warn!("admin list_templates decode char {}: {}", id, e);
                                continue;
                            }
                        }
                    }
                },
                None => continue,
            };
            summaries.push(summary);
        }
    }

    let count = summaries.len();
    Json(TemplateListResponse {
        total,
        from,
        count,
        items: summaries,
    })
    .into_response()
}

fn item_summary(id: usize, item: &Item) -> TemplateSummary {
    TemplateSummary {
        id,
        used: item.used != 0,
        name: c_str_owned(&item.name),
        reference: c_str_owned(&item.reference),
    }
}

fn character_summary(id: usize, ch: &Character) -> TemplateSummary {
    TemplateSummary {
        id,
        used: ch.used != 0,
        name: c_str_owned(&ch.name),
        reference: String::new(),
    }
}

fn c_str_owned(bytes: &[u8]) -> String {
    string_operations::c_string_to_str(bytes).to_owned()
}

// ---------------------------------------------------------------------------
//  Single-template GET
// ---------------------------------------------------------------------------

/// GET `/admin/templates/items/{id}` — returns raw bincode bytes.
pub(crate) async fn get_item_template(
    State(state): State<ApiState>,
    Path(id): Path<usize>,
) -> Response {
    get_template_bytes(state, id, TemplateKind::Item).await
}

/// GET `/admin/templates/characters/{id}` — returns raw bincode bytes.
pub(crate) async fn get_character_template(
    State(state): State<ApiState>,
    Path(id): Path<usize>,
) -> Response {
    get_template_bytes(state, id, TemplateKind::Character).await
}

async fn get_template_bytes(state: ApiState, id: usize, kind: TemplateKind) -> Response {
    if let Err(e) = template_store::validate_index(kind, id) {
        return template_error_response(e);
    }
    let key = match kind {
        TemplateKind::Item => template_store::item_template_key(id),
        TemplateKind::Character => template_store::character_template_key(id),
    };

    let mut con = state.con.clone();
    let bytes: Option<Vec<u8>> = match con.get(&key).await {
        Ok(v) => v,
        Err(e) => {
            warn!("admin get_template GET {} failed: {}", key, e);
            return internal_error("keydb_error", "Failed to read template");
        }
    };

    let bytes = match bytes {
        Some(b) => b,
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse::new(
                    "not_found",
                    format!("{} template {} has no stored bytes", kind.label(), id),
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
//  Single-template PUT
// ---------------------------------------------------------------------------

/// PUT `/admin/templates/items/{id}` — body is raw bincode bytes.
pub(crate) async fn put_item_template(
    State(state): State<ApiState>,
    Path(id): Path<usize>,
    body: Bytes,
) -> Response {
    put_template_bytes(state, id, TemplateKind::Item, body).await
}

/// PUT `/admin/templates/characters/{id}` — body is raw bincode bytes.
pub(crate) async fn put_character_template(
    State(state): State<ApiState>,
    Path(id): Path<usize>,
    body: Bytes,
) -> Response {
    put_template_bytes(state, id, TemplateKind::Character, body).await
}

async fn put_template_bytes(
    state: ApiState,
    id: usize,
    kind: TemplateKind,
    body: Bytes,
) -> Response {
    if let Err(e) = template_store::validate_index(kind, id) {
        return template_error_response(e);
    }

    // Decode-then-re-encode to validate the payload before persisting and
    // to normalise to the canonical bincode form.
    let canonical_bytes = match kind {
        TemplateKind::Item => match template_store::decode_item_template(&body) {
            Ok(item) => match template_store::encode_item_template(&item) {
                Ok(b) => b,
                Err(e) => return template_error_response(e),
            },
            Err(e) => return template_error_response(e),
        },
        TemplateKind::Character => match template_store::decode_character_template(&body) {
            Ok(ch) => match template_store::encode_character_template(&ch) {
                Ok(b) => b,
                Err(e) => return template_error_response(e),
            },
            Err(e) => return template_error_response(e),
        },
    };

    let key = match kind {
        TemplateKind::Item => template_store::item_template_key(id),
        TemplateKind::Character => template_store::character_template_key(id),
    };

    let mut con = state.con.clone();
    if let Err(e) = con.set::<_, _, ()>(&key, canonical_bytes).await {
        warn!("admin put_template SET {} failed: {}", key, e);
        return internal_error("keydb_error", "Failed to write template");
    }

    let version: u64 = match con.incr(kind.version_key(), 1_i64).await {
        Ok(v) => v,
        Err(e) => {
            warn!(
                "admin put_template INCR {} failed: {}",
                kind.version_key(),
                e
            );
            return internal_error("keydb_error", "Failed to bump version");
        }
    };

    info!(
        "admin updated {} template {} (version {})",
        kind.label(),
        id,
        version
    );
    Json(PutTemplateResponse { version }).into_response()
}

/// Stub PUT for `/admin/templates/items` (bulk PUT not supported in phase 1).
pub(crate) async fn put_item_templates_bulk_unsupported() -> Response {
    (
        StatusCode::METHOD_NOT_ALLOWED,
        Json(ErrorResponse::new(
            "not_supported",
            "Bulk PUT is not supported; PUT individual /admin/templates/items/{id}",
        )),
    )
        .into_response()
}

// ---------------------------------------------------------------------------
//  Reload coordination
// ---------------------------------------------------------------------------

/// POST `/admin/templates/reload`.
pub(crate) async fn request_reload(
    State(state): State<ApiState>,
    Json(req): Json<ReloadRequest>,
) -> Response {
    if req.kinds.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse::new(
                "missing_kinds",
                "Provide at least one kind in `kinds` (\"items\" or \"characters\")",
            )),
        )
            .into_response();
    }
    for kind in &req.kinds {
        match kind.as_str() {
            "items" | "characters" => {}
            other => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(ErrorResponse::new(
                        "unknown_kind",
                        format!("unknown reload kind \"{}\"", other),
                    )),
                )
                    .into_response();
            }
        }
    }

    let request_id = generate_request_id();
    let requested_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let payload = format!(
        r#"{{"request_id":"{}","kinds":[{}],"requested_at":{}}}"#,
        request_id,
        req.kinds
            .iter()
            .map(|k| format!("\"{}\"", k))
            .collect::<Vec<_>>()
            .join(","),
        requested_at
    );

    let mut con = state.con.clone();
    if let Err(e) = con
        .set_ex::<_, _, ()>(RELOAD_REQUEST_KEY, &payload, RELOAD_REQUEST_TTL_SECS)
        .await
    {
        warn!("admin request_reload SET failed: {}", e);
        return internal_error("keydb_error", "Failed to enqueue reload request");
    }

    // Best-effort pub/sub publish so future server versions can pick up
    // changes without polling.
    let _: Result<i64, _> = redis::cmd("PUBLISH")
        .arg(template_store::RELOAD_PUBSUB_CHANNEL)
        .arg(&payload)
        .query_async(&mut con)
        .await;

    info!(
        "admin enqueued reload request {} kinds={:?}",
        request_id, req.kinds
    );
    (
        StatusCode::ACCEPTED,
        Json(ReloadResponse {
            request_id,
            kinds: req.kinds,
        }),
    )
        .into_response()
}

/// Query for [`get_reload_status`].
#[derive(Debug, serde::Deserialize)]
pub(crate) struct ReloadStatusQuery {
    request_id: String,
}

/// GET `/admin/templates/reload/status?request_id=…`.
pub(crate) async fn get_reload_status(
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

    let key = reload_status_key(&q.request_id);
    let mut con = state.con.clone();
    let stored: Option<String> = match con.get(&key).await {
        Ok(v) => v,
        Err(e) => {
            warn!("admin get_reload_status GET {} failed: {}", key, e);
            return internal_error("keydb_error", "Failed to read status");
        }
    };

    let status = match stored {
        Some(s) if s.starts_with("applied") => "applied",
        Some(_) => "pending",
        None => {
            // Treat as either still pending (request just enqueued) or
            // expired. Caller distinguishes by elapsed time.
            "pending"
        }
    }
    .to_owned();

    Json(ReloadStatusResponse {
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

fn template_error_response(err: TemplateError) -> Response {
    match err {
        TemplateError::OutOfRange {
            kind,
            index,
            slot_count,
        } => (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse::new(
                "out_of_range",
                format!(
                    "{} template index {} out of range (0..{})",
                    kind.label(),
                    index,
                    slot_count
                ),
            )),
        )
            .into_response(),
        TemplateError::Decode(msg) => (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse::new("decode_error", msg)),
        )
            .into_response(),
        TemplateError::Encode(msg) => internal_error("encode_error", msg),
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
    use mag_core::template_store::{CHARACTER_TEMPLATE_SLOT_COUNT, ITEM_TEMPLATE_SLOT_COUNT};

    #[test]
    fn generate_request_id_is_24_hex() {
        let id = generate_request_id();
        assert_eq!(id.len(), 24);
        assert!(id.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn item_summary_extracts_fields() {
        let mut item = Item { used: 1, ..Item::default() };
        item.name[..5].copy_from_slice(b"sword");
        item.reference[..3].copy_from_slice(b"ref");
        let s = item_summary(7, &item);
        assert_eq!(s.id, 7);
        assert!(s.used);
        assert_eq!(s.name, "sword");
        assert_eq!(s.reference, "ref");
    }

    #[test]
    fn template_error_response_out_of_range_is_400() {
        let resp = template_error_response(TemplateError::OutOfRange {
            kind: TemplateKind::Item,
            index: 99999,
            slot_count: ITEM_TEMPLATE_SLOT_COUNT,
        });
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn slot_counts_match_constants() {
        assert_eq!(TemplateKind::Item.slot_count(), ITEM_TEMPLATE_SLOT_COUNT);
        assert_eq!(
            TemplateKind::Character.slot_count(),
            CHARACTER_TEMPLATE_SLOT_COUNT
        );
    }
}
