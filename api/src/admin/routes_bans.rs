//! Admin route handlers for durable ban management.

use crate::ApiState;
use crate::admin::types::{
    BanCreateRequest, BanListResponse, BanMutationResponse, BanRecordResponse, BanTargetRequest,
    BanTargetResponse, CharacterSearchResponse, CharacterSearchResult, ErrorResponse,
};
use crate::pipelines;
use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use log::{info, warn};
use mag_core::ban_action_store::{
    BAN_ACTION_PUBSUB_CHANNEL, BAN_ACTION_QUEUE_KEY, BAN_ACTION_STATUS_TTL_SECS, BanActionKind,
    BanActionRequest, BanActionStatusResponse, STATUS_PENDING, ban_action_status_key,
};
use mag_core::ban_store::{
    BAN_ACTIVE_INDEX_KEY, BAN_MUTATION_LOCK_KEY, BAN_MUTATION_LOCK_TTL_MS, BAN_VERSION_KEY,
    BanRecord, BanTarget, parse_ipv4,
};
use rand::RngCore;
use rand::rngs::OsRng;
use redis::AsyncCommands;
use serde::Deserialize;
use std::time::{SystemTime, UNIX_EPOCH};

/// Query parameters for listing bans.
#[derive(Debug, Deserialize)]
pub(crate) struct BanListQuery {
    /// Optional scope filter: `account`, `character`, or `ipv4`.
    scope: Option<String>,
    /// Include expired active-key records in the response.
    include_expired: Option<bool>,
}

/// Query parameters for live ban-action status.
#[derive(Debug, Deserialize)]
pub(crate) struct BanActionStatusQuery {
    /// Live action request id.
    request_id: String,
}

/// Query parameters for character-name search.
#[derive(Debug, Deserialize)]
pub(crate) struct CharacterSearchQuery {
    /// Name or partial name to search for.
    name: String,
    /// Maximum number of matches to return.
    limit: Option<usize>,
}

const DEFAULT_CHARACTER_SEARCH_LIMIT: usize = 20;
const MAX_CHARACTER_SEARCH_LIMIT: usize = 50;

/// GET `/admin/bans`.
pub(crate) async fn list_bans(
    State(state): State<ApiState>,
    Query(query): Query<BanListQuery>,
) -> Response {
    let mut con = state.con.clone();
    let records = match load_bans(&mut con).await {
        Ok(records) => records,
        Err(response) => return response,
    };
    let version = match load_ban_version(&mut con).await {
        Ok(version) => version,
        Err(response) => return response,
    };
    let now = now_secs();
    let scope = query
        .scope
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());

    let bans: Vec<BanRecordResponse> = records
        .into_iter()
        .filter(|record| query.include_expired.unwrap_or(false) || record.is_active_at(now))
        .filter(|record| {
            scope
                .map(|scope| scope == record.target.scope())
                .unwrap_or(true)
        })
        .map(|record| record_response(record, now))
        .collect();

    Json(BanListResponse {
        count: bans.len(),
        bans,
        version,
    })
    .into_response()
}

/// GET `/admin/bans/account/{account_id}`.
pub(crate) async fn get_account_ban(
    State(state): State<ApiState>,
    Path(account_id): Path<u64>,
) -> Response {
    get_ban(state, BanTarget::Account { account_id }).await
}

/// GET `/admin/bans/character/{character_id}`.
pub(crate) async fn get_character_ban(
    State(state): State<ApiState>,
    Path(character_id): Path<u64>,
) -> Response {
    get_ban(state, BanTarget::Character { character_id }).await
}

/// GET `/admin/bans/ip/{address}`.
pub(crate) async fn get_ipv4_ban(
    State(state): State<ApiState>,
    Path(address): Path<String>,
) -> Response {
    let address = match parse_ipv4(&address) {
        Ok(address) => address,
        Err(error) => return bad_request("invalid_ipv4", error.to_string()),
    };
    get_ban(state, BanTarget::Ipv4 { address }).await
}

/// POST `/admin/bans`.
pub(crate) async fn create_ban(
    State(state): State<ApiState>,
    Json(request): Json<BanCreateRequest>,
) -> Response {
    let mut con = state.con.clone();
    let target = match resolve_target(&mut con, request.target).await {
        Ok(target) => target,
        Err(response) => return response,
    };
    let now = now_secs();
    let expires_at = match resolve_expires_at(now, request.expires_at, request.duration_seconds) {
        Ok(expires_at) => expires_at,
        Err(response) => return response,
    };
    let record = BanRecord {
        id: generate_request_id(),
        target: target.clone(),
        reason: request.reason.unwrap_or_default(),
        created_by: request
            .created_by
            .unwrap_or_else(|| "admin_api".to_owned()),
        created_at: now,
        expires_at,
        source: "admin_api".to_owned(),
    };

    let lock_token = match acquire_ban_lock(&mut con).await {
        Ok(Some(token)) => token,
        Ok(None) => return conflict("busy", "Another ban mutation is already in progress"),
        Err(response) => return response,
    };

    let result = upsert_ban(&mut con, &record).await;
    release_ban_lock(&mut con, &lock_token).await;
    let version = match result {
        Ok(version) => version,
        Err(response) => return response,
    };
    let live_request_id = enqueue_live_action(
        &mut con,
        BanActionKind::ApplyBan {
            target,
            kick_online: request.kick_online.unwrap_or(true),
        },
    )
    .await;

    info!("admin created ban {}", record.id);
    Json(BanMutationResponse {
        ban: Some(record_response(record, now)),
        changed: true,
        version,
        live_request_id,
    })
    .into_response()
}

/// DELETE `/admin/bans/account/{account_id}`.
pub(crate) async fn delete_account_ban(
    State(state): State<ApiState>,
    Path(account_id): Path<u64>,
) -> Response {
    delete_ban(state, BanTarget::Account { account_id }).await
}

/// DELETE `/admin/bans/character/{character_id}`.
pub(crate) async fn delete_character_ban(
    State(state): State<ApiState>,
    Path(character_id): Path<u64>,
) -> Response {
    delete_ban(state, BanTarget::Character { character_id }).await
}

/// DELETE `/admin/bans/ip/{address}`.
pub(crate) async fn delete_ipv4_ban(
    State(state): State<ApiState>,
    Path(address): Path<String>,
) -> Response {
    let address = match parse_ipv4(&address) {
        Ok(address) => address,
        Err(error) => return bad_request("invalid_ipv4", error.to_string()),
    };
    delete_ban(state, BanTarget::Ipv4 { address }).await
}

/// GET `/admin/bans/actions/status?request_id=...`.
pub(crate) async fn get_ban_action_status(
    State(state): State<ApiState>,
    Query(query): Query<BanActionStatusQuery>,
) -> Response {
    if query.request_id.is_empty() {
        return bad_request("missing_request_id", "Provide ?request_id=<id>");
    }

    let key = ban_action_status_key(&query.request_id);
    let mut con = state.con.clone();
    let stored: Option<String> = match con.get(&key).await {
        Ok(value) => value,
        Err(error) => {
            warn!("admin get_ban_action_status GET {} failed: {}", key, error);
            return internal_error("keydb_error", "Failed to read status");
        }
    };

    Json(parse_status(&query.request_id, stored)).into_response()
}

/// GET `/admin/bans/characters/search?name=...`.
pub(crate) async fn search_characters(
    State(state): State<ApiState>,
    Query(query): Query<CharacterSearchQuery>,
) -> Response {
    let name = query.name.trim();
    if name.is_empty() {
        return bad_request("missing_name", "Provide ?name=<character name>");
    }

    let limit = query
        .limit
        .unwrap_or(DEFAULT_CHARACTER_SEARCH_LIMIT)
        .min(MAX_CHARACTER_SEARCH_LIMIT);
    let mut con = state.con.clone();
    let matches = match pipelines::search_characters_by_name_scan(&mut con, name, limit).await {
        Ok(matches) => matches,
        Err(error) => {
            warn!("admin character search failed: {}", error);
            return internal_error("keydb_error", "Failed to search characters");
        }
    };

    let characters: Vec<CharacterSearchResult> = matches
        .into_iter()
        .map(|value| CharacterSearchResult {
            id: value.id,
            name: value.name,
            account_id: value.account_id,
            account_username: value.account_username,
            server_id: value.server_id,
        })
        .collect();

    Json(CharacterSearchResponse {
        query: name.to_owned(),
        count: characters.len(),
        characters,
    })
    .into_response()
}

/// Return whether an account has an active ban.
pub(crate) async fn account_is_banned(
    con: &mut redis::aio::MultiplexedConnection,
    account_id: u64,
) -> Result<bool, String> {
    target_is_banned(con, &BanTarget::Account { account_id }).await
}

/// Return whether a character has an active ban.
pub(crate) async fn character_is_banned(
    con: &mut redis::aio::MultiplexedConnection,
    character_id: u64,
) -> Result<bool, String> {
    target_is_banned(con, &BanTarget::Character { character_id }).await
}

async fn get_ban(state: ApiState, target: BanTarget) -> Response {
    let mut con = state.con.clone();
    match load_ban(&mut con, &target).await {
        Ok(Some(record)) => Json(record_response(record, now_secs())).into_response(),
        Ok(None) => not_found("not_found", "No active ban for target"),
        Err(response) => response,
    }
}

async fn delete_ban(state: ApiState, target: BanTarget) -> Response {
    let mut con = state.con.clone();
    let lock_token = match acquire_ban_lock(&mut con).await {
        Ok(Some(token)) => token,
        Ok(None) => return conflict("busy", "Another ban mutation is already in progress"),
        Err(response) => return response,
    };

    let result = remove_ban(&mut con, &target).await;
    release_ban_lock(&mut con, &lock_token).await;
    let (removed, version) = match result {
        Ok(value) => value,
        Err(response) => return response,
    };
    let live_request_id = enqueue_live_action(
        &mut con,
        BanActionKind::RemoveBan {
            target: target.clone(),
        },
    )
    .await;
    let ban = removed.map(|record| record_response(record, now_secs()));

    Json(BanMutationResponse {
        changed: ban.is_some(),
        ban,
        version,
        live_request_id,
    })
    .into_response()
}

async fn resolve_target(
    con: &mut redis::aio::MultiplexedConnection,
    request: BanTargetRequest,
) -> Result<BanTarget, Response> {
    match request {
        BanTargetRequest::Account {
            account_id,
            username,
        } => {
            if let Some(account_id) = account_id {
                return Ok(BanTarget::Account { account_id });
            }
            let Some(username) = username.map(|value| value.trim().to_ascii_lowercase()) else {
                return Err(bad_request(
                    "missing_account_target",
                    "Provide account_id or username",
                ));
            };
            match pipelines::get_account_id_by_username(con, &username).await {
                Ok(Some(account_id)) => Ok(BanTarget::Account { account_id }),
                Ok(None) => Err(not_found("account_not_found", "Account username not found")),
                Err(error) => {
                    warn!("admin ban account lookup failed: {}", error);
                    Err(internal_error("keydb_error", "Failed to resolve account"))
                }
            }
        }
        BanTargetRequest::Character { character_id } => Ok(BanTarget::Character { character_id }),
        BanTargetRequest::Ipv4 { address } => parse_ipv4(&address)
            .map(|address| BanTarget::Ipv4 { address })
            .map_err(|error| bad_request("invalid_ipv4", error.to_string())),
    }
}

fn resolve_expires_at(
    now: u64,
    expires_at: Option<u64>,
    duration_seconds: Option<u64>,
) -> Result<Option<u64>, Response> {
    match (expires_at, duration_seconds) {
        (Some(_), Some(_)) => Err(bad_request(
            "conflicting_expiration",
            "Provide expires_at or duration_seconds, not both",
        )),
        (Some(value), None) if value <= now => Err(bad_request(
            "expired_ban",
            "expires_at must be in the future",
        )),
        (Some(value), None) => Ok(Some(value)),
        (None, Some(value)) if value == 0 => Err(bad_request(
            "invalid_duration",
            "duration_seconds must be greater than zero",
        )),
        (None, Some(value)) => Ok(Some(now.saturating_add(value))),
        (None, None) => Ok(None),
    }
}

async fn load_bans(
    con: &mut redis::aio::MultiplexedConnection,
) -> Result<Vec<BanRecord>, Response> {
    let keys: Vec<String> = con.smembers(BAN_ACTIVE_INDEX_KEY).await.map_err(|error| {
        warn!("admin bans SMEMBERS failed: {}", error);
        internal_error("keydb_error", "Failed to read bans")
    })?;
    let mut records = Vec::with_capacity(keys.len());
    for key in keys {
        let bytes: Option<Vec<u8>> = con.get(&key).await.map_err(|error| {
            warn!("admin bans GET {} failed: {}", key, error);
            internal_error("keydb_error", "Failed to read ban")
        })?;
        let Some(bytes) = bytes else {
            let _: Result<i64, _> = con.srem(BAN_ACTIVE_INDEX_KEY, &key).await;
            continue;
        };
        match BanRecord::from_bytes(&bytes) {
            Ok(record) => records.push(record),
            Err(error) => warn!("admin bans decode {} failed: {}", key, error),
        }
    }
    records.sort_by(|left, right| {
        left.target
            .scope()
            .cmp(right.target.scope())
            .then(left.target.value().cmp(&right.target.value()))
    });
    Ok(records)
}

async fn load_ban(
    con: &mut redis::aio::MultiplexedConnection,
    target: &BanTarget,
) -> Result<Option<BanRecord>, Response> {
    let key = target.active_key();
    let bytes: Option<Vec<u8>> = con.get(&key).await.map_err(|error| {
        warn!("admin bans GET {} failed: {}", key, error);
        internal_error("keydb_error", "Failed to read ban")
    })?;
    let Some(bytes) = bytes else {
        return Ok(None);
    };
    let record = BanRecord::from_bytes(&bytes)
        .map_err(|error| internal_error("decode_error", error.to_string()))?;
    if record.is_active_at(now_secs()) {
        Ok(Some(record))
    } else {
        Ok(None)
    }
}

async fn target_is_banned(
    con: &mut redis::aio::MultiplexedConnection,
    target: &BanTarget,
) -> Result<bool, String> {
    let key = target.active_key();
    let bytes: Option<Vec<u8>> = con
        .get(&key)
        .await
        .map_err(|error| format!("failed to read ban {}: {}", key, error))?;
    let Some(bytes) = bytes else {
        return Ok(false);
    };
    let record = BanRecord::from_bytes(&bytes).map_err(|error| error.to_string())?;
    Ok(record.is_active_at(now_secs()))
}

async fn upsert_ban(
    con: &mut redis::aio::MultiplexedConnection,
    record: &BanRecord,
) -> Result<u64, Response> {
    let key = record.target.active_key();
    let bytes = record
        .to_bytes()
        .map_err(|error| internal_error("encode_error", error.to_string()))?;
    con.set::<_, _, ()>(&key, bytes).await.map_err(|error| {
        warn!("admin bans SET {} failed: {}", key, error);
        internal_error("keydb_error", "Failed to write ban")
    })?;
    con.sadd::<_, _, ()>(BAN_ACTIVE_INDEX_KEY, &key)
        .await
        .map_err(|error| {
            warn!("admin bans SADD failed: {}", error);
            internal_error("keydb_error", "Failed to index ban")
        })?;
    bump_version(con).await
}

async fn remove_ban(
    con: &mut redis::aio::MultiplexedConnection,
    target: &BanTarget,
) -> Result<(Option<BanRecord>, u64), Response> {
    let key = target.active_key();
    let existing = load_ban(con, target).await?;
    con.del::<_, ()>(&key).await.map_err(|error| {
        warn!("admin bans DEL {} failed: {}", key, error);
        internal_error("keydb_error", "Failed to delete ban")
    })?;
    con.srem::<_, _, ()>(BAN_ACTIVE_INDEX_KEY, &key)
        .await
        .map_err(|error| {
            warn!("admin bans SREM failed: {}", error);
            internal_error("keydb_error", "Failed to deindex ban")
        })?;
    let version = if existing.is_some() {
        bump_version(con).await?
    } else {
        load_ban_version(con).await?
    };
    Ok((existing, version))
}

async fn load_ban_version(con: &mut redis::aio::MultiplexedConnection) -> Result<u64, Response> {
    con.get::<_, Option<u64>>(BAN_VERSION_KEY)
        .await
        .map(|value| value.unwrap_or(0))
        .map_err(|error| {
            warn!("admin bans version GET failed: {}", error);
            internal_error("keydb_error", "Failed to read ban version")
        })
}

async fn bump_version(con: &mut redis::aio::MultiplexedConnection) -> Result<u64, Response> {
    con.incr(BAN_VERSION_KEY, 1_u64).await.map_err(|error| {
        warn!("admin bans version INCR failed: {}", error);
        internal_error("keydb_error", "Failed to bump ban version")
    })
}

async fn enqueue_live_action(
    con: &mut redis::aio::MultiplexedConnection,
    action: BanActionKind,
) -> Option<String> {
    let request_id = generate_request_id();
    let requested_at = now_secs();
    let action_name = action.name().to_owned();
    let request = BanActionRequest {
        request_id: request_id.clone(),
        action,
        requested_at,
    };
    let bytes = match request.to_bytes() {
        Ok(bytes) => bytes,
        Err(error) => {
            warn!("admin ban action encode failed: {}", error);
            return None;
        }
    };
    let status_key = ban_action_status_key(&request_id);
    let status_value = format_status_value(STATUS_PENDING, &action_name, "queued", requested_at);
    if let Err(error) = con
        .set_ex::<_, _, ()>(&status_key, status_value, BAN_ACTION_STATUS_TTL_SECS)
        .await
    {
        warn!("admin ban action status SET failed: {}", error);
        return None;
    }
    if let Err(error) = redis::cmd("RPUSH")
        .arg(BAN_ACTION_QUEUE_KEY)
        .arg(bytes)
        .query_async::<i64>(con)
        .await
    {
        warn!("admin ban action RPUSH failed: {}", error);
        return None;
    }
    let _: Result<i64, _> = redis::cmd("PUBLISH")
        .arg(BAN_ACTION_PUBSUB_CHANNEL)
        .arg(&request_id)
        .query_async(con)
        .await;
    Some(request_id)
}

async fn acquire_ban_lock(
    con: &mut redis::aio::MultiplexedConnection,
) -> Result<Option<String>, Response> {
    let token = generate_request_id();
    let result: Option<String> = redis::cmd("SET")
        .arg(BAN_MUTATION_LOCK_KEY)
        .arg(&token)
        .arg("NX")
        .arg("PX")
        .arg(BAN_MUTATION_LOCK_TTL_MS)
        .query_async(con)
        .await
        .map_err(|error| {
            warn!("admin bans lock SET failed: {}", error);
            internal_error("keydb_error", "Failed to acquire ban lock")
        })?;
    Ok(result.map(|_| token))
}

async fn release_ban_lock(con: &mut redis::aio::MultiplexedConnection, token: &str) {
    let _: Result<i64, _> = redis::cmd("EVAL")
        .arg(
            "if redis.call('GET', KEYS[1]) == ARGV[1] then \
             return redis.call('DEL', KEYS[1]) else return 0 end",
        )
        .arg(1)
        .arg(BAN_MUTATION_LOCK_KEY)
        .arg(token)
        .query_async(con)
        .await;
}

fn record_response(record: BanRecord, now: u64) -> BanRecordResponse {
    BanRecordResponse {
        active: record.is_active_at(now),
        target: target_response(&record.target),
        id: record.id,
        reason: record.reason,
        created_by: record.created_by,
        created_at: record.created_at,
        expires_at: record.expires_at,
        source: record.source,
    }
}

fn target_response(target: &BanTarget) -> BanTargetResponse {
    match target {
        BanTarget::Account { account_id } => BanTargetResponse {
            scope: target.scope().to_owned(),
            value: target.value(),
            account_id: Some(*account_id),
            character_id: None,
            address: None,
        },
        BanTarget::Character { character_id } => BanTargetResponse {
            scope: target.scope().to_owned(),
            value: target.value(),
            account_id: None,
            character_id: Some(*character_id),
            address: None,
        },
        BanTarget::Ipv4 { .. } => BanTargetResponse {
            scope: target.scope().to_owned(),
            value: target.value(),
            account_id: None,
            character_id: None,
            address: Some(target.value()),
        },
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

fn parse_status(request_id: &str, stored: Option<String>) -> BanActionStatusResponse {
    let Some(raw) = stored else {
        return BanActionStatusResponse {
            request_id: request_id.to_owned(),
            action: String::new(),
            status: STATUS_PENDING.to_owned(),
            message: String::new(),
            updated_at: 0,
        };
    };

    let mut parts = raw.splitn(4, '|');
    let status = parts.next().unwrap_or(STATUS_PENDING).to_owned();
    let action = parts.next().unwrap_or_default().to_owned();
    let updated_at = parts
        .next()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(0);
    let message = parts.next().unwrap_or_default().to_owned();

    BanActionStatusResponse {
        request_id: request_id.to_owned(),
        action,
        status,
        message,
        updated_at,
    }
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
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

fn bad_request(code: &str, message: impl Into<String>) -> Response {
    (
        StatusCode::BAD_REQUEST,
        Json(ErrorResponse::new(code, message.into())),
    )
        .into_response()
}

fn conflict(code: &str, message: impl Into<String>) -> Response {
    (
        StatusCode::CONFLICT,
        Json(ErrorResponse::new(code, message.into())),
    )
        .into_response()
}

fn not_found(code: &str, message: impl Into<String>) -> Response {
    (
        StatusCode::NOT_FOUND,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_expires_at_rejects_conflicting_inputs() {
        assert!(resolve_expires_at(10, Some(20), Some(5)).is_err());
    }

    #[test]
    fn resolve_expires_at_accepts_relative_duration() {
        assert_eq!(resolve_expires_at(10, None, Some(5)).unwrap(), Some(15));
    }

    #[test]
    fn target_response_formats_ipv4() {
        let response = target_response(&BanTarget::Ipv4 {
            address: 0xcb00_7107,
        });
        assert_eq!(response.scope, "ipv4");
        assert_eq!(response.address.as_deref(), Some("203.0.113.7"));
    }
}
