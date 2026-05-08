//! Admin route handlers for badwords text-data CRUD.
//!
//! Badwords are stored as a single bincode-encoded `Vec<String>` under
//! `game:badwords`. These endpoints expose JSON so operator scripts can list,
//! add, replace, remove, and refresh the running server's cached copy without
//! speaking bincode directly.

use crate::ApiState;
use crate::admin::types::{
    BadwordEntryResponse, BadwordsListResponse, BadwordsMutationRequest, BadwordsMutationResponse,
    ErrorResponse, TextReloadRequest, TextReloadResponse, TextReloadStatusResponse,
};
use axum::Json;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use log::{info, warn};
use mag_core::text_store::{
    BADWORDS_KEY, BADWORDS_VERSION_KEY, TEXT_RELOAD_PUBSUB_CHANNEL, TEXT_RELOAD_REQUEST_KEY,
    TextStoreError, decode_badwords, encode_badwords, normalize_badword, normalize_badwords,
    text_reload_status_key,
};
use rand::RngCore;
use rand::rngs::OsRng;
use redis::AsyncCommands;
use std::time::{SystemTime, UNIX_EPOCH};

/// TTL applied to text-reload request payloads written to KeyDB.
const RELOAD_REQUEST_TTL_SECS: u64 = 30;

/// Key used to serialize whole-list badwords mutations.
const BADWORDS_LOCK_KEY: &str = "admin:text:badwords:lock";

/// Milliseconds before a stale badwords mutation lock expires.
const BADWORDS_LOCK_TTL_MS: u64 = 5_000;

/// Query for `GET /admin/text/badwords/entry`.
#[derive(Debug, serde::Deserialize)]
pub(crate) struct BadwordEntryQuery {
    word: String,
}

/// Query for `GET /admin/text/reload/status`.
#[derive(Debug, serde::Deserialize)]
pub(crate) struct TextReloadStatusQuery {
    request_id: String,
}

#[derive(Debug, Clone, Copy)]
enum MutationKind {
    Add,
    Replace,
    Remove,
}

/// GET `/admin/text/badwords`.
pub(crate) async fn get_badwords(State(state): State<ApiState>) -> Response {
    let mut con = state.con.clone();
    let words = match load_badwords(&mut con).await {
        Ok(words) => words,
        Err(resp) => return resp,
    };
    let version = match load_badwords_version(&mut con).await {
        Ok(version) => version,
        Err(resp) => return resp,
    };

    Json(BadwordsListResponse {
        count: words.len(),
        words,
        version,
    })
    .into_response()
}

/// GET `/admin/text/badwords/entry?word=...`.
pub(crate) async fn get_badword_entry(
    State(state): State<ApiState>,
    Query(q): Query<BadwordEntryQuery>,
) -> Response {
    let word = match normalize_badword(&q.word) {
        Ok(word) => word,
        Err(err) => return text_error_response(err),
    };

    let mut con = state.con.clone();
    let words = match load_badwords(&mut con).await {
        Ok(words) => words,
        Err(resp) => return resp,
    };

    Json(BadwordEntryResponse {
        exists: words.iter().any(|entry| entry == &word),
        word,
    })
    .into_response()
}

/// POST `/admin/text/badwords`.
pub(crate) async fn add_badwords(
    State(state): State<ApiState>,
    Json(req): Json<BadwordsMutationRequest>,
) -> Response {
    if req.words.is_empty() {
        return bad_request("missing_words", "Provide at least one word to add");
    }
    mutate_badwords(state, MutationKind::Add, req.words).await
}

/// PUT `/admin/text/badwords`.
pub(crate) async fn replace_badwords(
    State(state): State<ApiState>,
    Json(req): Json<BadwordsMutationRequest>,
) -> Response {
    mutate_badwords(state, MutationKind::Replace, req.words).await
}

/// DELETE `/admin/text/badwords`.
pub(crate) async fn remove_badwords(
    State(state): State<ApiState>,
    Json(req): Json<BadwordsMutationRequest>,
) -> Response {
    if req.words.is_empty() {
        return bad_request("missing_words", "Provide at least one word to remove");
    }
    mutate_badwords(state, MutationKind::Remove, req.words).await
}

/// POST `/admin/text/reload`.
pub(crate) async fn request_text_reload(
    State(state): State<ApiState>,
    Json(req): Json<TextReloadRequest>,
) -> Response {
    if req.kinds.is_empty() {
        return bad_request(
            "missing_kinds",
            "Provide at least one kind in `kinds` (\"badwords\")",
        );
    }
    for kind in &req.kinds {
        if kind != "badwords" {
            return bad_request(
                "unknown_kind",
                format!("unknown text reload kind \"{}\"", kind),
            );
        }
    }

    let request_id = generate_request_id();
    let requested_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    let payload = format!(
        r#"{{"request_id":"{}","kinds":[{}],"requested_at":{}}}"#,
        request_id,
        req.kinds
            .iter()
            .map(|kind| format!("\"{}\"", kind))
            .collect::<Vec<_>>()
            .join(","),
        requested_at
    );

    let mut con = state.con.clone();
    if let Err(err) = con
        .set_ex::<_, _, ()>(TEXT_RELOAD_REQUEST_KEY, &payload, RELOAD_REQUEST_TTL_SECS)
        .await
    {
        warn!("admin request_text_reload SET failed: {}", err);
        return internal_error("keydb_error", "Failed to enqueue text reload request");
    }

    let _: Result<i64, _> = redis::cmd("PUBLISH")
        .arg(TEXT_RELOAD_PUBSUB_CHANNEL)
        .arg(&payload)
        .query_async(&mut con)
        .await;

    info!(
        "admin enqueued text reload request {} kinds={:?}",
        request_id, req.kinds
    );
    (
        StatusCode::ACCEPTED,
        Json(TextReloadResponse {
            request_id,
            kinds: req.kinds,
        }),
    )
        .into_response()
}

/// GET `/admin/text/reload/status?request_id=...`.
pub(crate) async fn get_text_reload_status(
    State(state): State<ApiState>,
    Query(q): Query<TextReloadStatusQuery>,
) -> Response {
    if q.request_id.is_empty() {
        return bad_request("missing_request_id", "Provide ?request_id=<id>");
    }

    let key = text_reload_status_key(&q.request_id);
    let mut con = state.con.clone();
    let stored: Option<String> = match con.get(&key).await {
        Ok(value) => value,
        Err(err) => {
            warn!("admin get_text_reload_status GET {} failed: {}", key, err);
            return internal_error("keydb_error", "Failed to read status");
        }
    };

    let status = match stored {
        Some(value) if value.starts_with("applied") => "applied",
        Some(_) | None => "pending",
    }.to_owned();

    Json(TextReloadStatusResponse {
        status,
        request_id: q.request_id,
    })
    .into_response()
}

async fn mutate_badwords(state: ApiState, kind: MutationKind, raw_words: Vec<String>) -> Response {
    let requested = match normalize_badwords(&raw_words) {
        Ok(words) => words,
        Err(err) => return text_error_response(err),
    };

    let mut con = state.con.clone();
    let lock_token = match acquire_badwords_lock(&mut con).await {
        Ok(Some(token)) => token,
        Ok(None) => {
            return (
                StatusCode::CONFLICT,
                Json(ErrorResponse::new(
                    "busy",
                    "Another badwords mutation is already in progress",
                )),
            )
                .into_response();
        }
        Err(resp) => return resp,
    };

    let result = apply_badwords_mutation(&mut con, kind, requested).await;
    release_badwords_lock(&mut con, &lock_token).await;

    match result {
        Ok(response) => Json(response).into_response(),
        Err(resp) => resp,
    }
}

async fn apply_badwords_mutation(
    con: &mut redis::aio::MultiplexedConnection,
    kind: MutationKind,
    requested: Vec<String>,
) -> Result<BadwordsMutationResponse, Response> {
    let current = load_badwords(con).await?;
    let mut next = current.clone();
    let mut added = Vec::new();
    let mut removed = Vec::new();
    let mut unchanged = Vec::new();

    match kind {
        MutationKind::Add => {
            for word in requested {
                if next.iter().any(|entry| entry == &word) {
                    unchanged.push(word);
                } else {
                    next.push(word.clone());
                    added.push(word);
                }
            }
        }
        MutationKind::Replace => {
            next = requested;
            for word in &next {
                if current.iter().any(|entry| entry == word) {
                    unchanged.push(word.clone());
                } else {
                    added.push(word.clone());
                }
            }
            for word in &current {
                if !next.iter().any(|entry| entry == word) {
                    removed.push(word.clone());
                }
            }
        }
        MutationKind::Remove => {
            for word in requested {
                if current.iter().any(|entry| entry == &word) {
                    removed.push(word);
                } else {
                    unchanged.push(word);
                }
            }
            next.retain(|entry| !removed.iter().any(|word| word == entry));
        }
    }

    let changed = next != current;
    let version = if changed {
        let bytes = match encode_badwords(&next) {
            Ok(bytes) => bytes,
            Err(err) => return Err(text_error_response(err)),
        };
        if let Err(err) = con.set::<_, _, ()>(BADWORDS_KEY, bytes).await {
            warn!("admin badwords SET {} failed: {}", BADWORDS_KEY, err);
            return Err(internal_error("keydb_error", "Failed to write badwords"));
        }
        match con.incr(BADWORDS_VERSION_KEY, 1_i64).await {
            Ok(version) => version,
            Err(err) => {
                warn!(
                    "admin badwords INCR {} failed: {}",
                    BADWORDS_VERSION_KEY, err
                );
                return Err(internal_error("keydb_error", "Failed to bump version"));
            }
        }
    } else {
        load_badwords_version(con).await?
    };

    info!(
        "admin mutated badwords kind={:?} changed={} version={}",
        kind, changed, version
    );

    Ok(BadwordsMutationResponse {
        count: next.len(),
        words: next,
        version,
        added,
        removed,
        unchanged,
    })
}

async fn load_badwords(
    con: &mut redis::aio::MultiplexedConnection,
) -> Result<Vec<String>, Response> {
    let bytes: Option<Vec<u8>> = match con.get(BADWORDS_KEY).await {
        Ok(value) => value,
        Err(err) => {
            warn!("admin badwords GET {} failed: {}", BADWORDS_KEY, err);
            return Err(internal_error("keydb_error", "Failed to read badwords"));
        }
    };

    let Some(bytes) = bytes else {
        return Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse::new(
                "not_seeded",
                "Missing game:badwords; seed the world snapshot into KeyDB first",
            )),
        )
            .into_response());
    };

    decode_badwords(&bytes).map_err(text_error_response)
}

async fn load_badwords_version(
    con: &mut redis::aio::MultiplexedConnection,
) -> Result<u64, Response> {
    match con.get::<_, Option<u64>>(BADWORDS_VERSION_KEY).await {
        Ok(value) => Ok(value.unwrap_or(0)),
        Err(err) => {
            warn!(
                "admin badwords version GET {} failed: {}",
                BADWORDS_VERSION_KEY, err
            );
            Err(internal_error(
                "keydb_error",
                "Failed to read badwords version",
            ))
        }
    }
}

async fn acquire_badwords_lock(
    con: &mut redis::aio::MultiplexedConnection,
) -> Result<Option<String>, Response> {
    let token = generate_request_id();
    let result: Option<String> = match redis::cmd("SET")
        .arg(BADWORDS_LOCK_KEY)
        .arg(&token)
        .arg("NX")
        .arg("PX")
        .arg(BADWORDS_LOCK_TTL_MS)
        .query_async(con)
        .await
    {
        Ok(value) => value,
        Err(err) => {
            warn!("admin badwords lock SET failed: {}", err);
            return Err(internal_error(
                "keydb_error",
                "Failed to acquire badwords lock",
            ));
        }
    };

    Ok(result.map(|_| token))
}

async fn release_badwords_lock(con: &mut redis::aio::MultiplexedConnection, token: &str) {
    let _: Result<i64, _> = redis::cmd("EVAL")
        .arg(
            "if redis.call('GET', KEYS[1]) == ARGV[1] then \
             return redis.call('DEL', KEYS[1]) else return 0 end",
        )
        .arg(1)
        .arg(BADWORDS_LOCK_KEY)
        .arg(token)
        .query_async(con)
        .await;
}

fn text_error_response(err: TextStoreError) -> Response {
    match err {
        TextStoreError::Encode(msg) => internal_error("encode_error", msg),
        TextStoreError::Decode(msg) => internal_error("decode_error", msg),
        other => bad_request("invalid_badword", other.to_string()),
    }
}

fn bad_request(code: &str, message: impl Into<String>) -> Response {
    (
        StatusCode::BAD_REQUEST,
        Json(ErrorResponse::new(code, message.into())),
    )
        .into_response()
}

fn internal_error(code: &str, message: impl Into<String>) -> Response {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ErrorResponse::new(code, message.into())),
    )
        .into_response()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_request_id_is_24_hex() {
        let id = generate_request_id();
        assert_eq!(id.len(), 24);
        assert!(id.chars().all(|character| character.is_ascii_hexdigit()));
    }

    #[test]
    fn text_error_validation_is_400() {
        let resp = text_error_response(TextStoreError::EmptyEntry);
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn text_error_decode_is_500() {
        let resp = text_error_response(TextStoreError::Decode("bad".into()));
        assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }
}
