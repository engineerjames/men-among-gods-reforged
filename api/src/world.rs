//! Public, read-only world information routes mounted at `/world/*`.
//!
//! Two data sources back these endpoints:
//!
//! * `/world/time` and `/world/online` read the compact
//!   [`LiveStatus`] snapshot the game server republishes every ~10 s under
//!   [`LIVE_STATUS_KEY`]. If the key is absent the server is down (the key
//!   carries a TTL) and the endpoints answer `503`.
//! * `/world/top` and `/world/richest` scan the persisted `game:char:*`
//!   slots (flushed by the server every ~12 min) and keep a process-local
//!   cache for [`LEADERBOARD_CACHE_TTL`], so repeated polling from bots never
//!   costs more than one 8k-key pipeline per minute.

use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use axum::Json;
use axum::Router;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use log::warn;
use mag_core::character_store::{self, CHARACTER_SLOT_COUNT};
use mag_core::constants::{CharacterFlags, USE_EMPTY};
use mag_core::live_status_store::{
    CharacterRoles, LIVE_STATUS_KEY, LiveStatus, OnlinePlayer, WorldTime,
};
use mag_core::types::Character;
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};

use crate::ApiState;
use crate::admin::types::ErrorResponse;

/// How long a scanned leaderboard snapshot is reused before re-reading KeyDB.
pub const LEADERBOARD_CACHE_TTL: Duration = Duration::from_secs(60);

/// Default number of leaderboard entries when `?limit=` is omitted.
pub const DEFAULT_LEADERBOARD_LIMIT: usize = 10;

/// Hard cap on `?limit=`.
pub const MAX_LEADERBOARD_LIMIT: usize = 100;

/// Number of `GET`s pipelined per KeyDB round trip while scanning characters.
const PIPELINE_BATCH_SIZE: usize = 4096;

/// Compact per-player fields needed by both leaderboards.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerSummary {
    /// Character name.
    pub name: String,
    /// Total experience points.
    pub points: i32,
    /// Carried gold in hundredths (`/100` = gold, `%100` = silver).
    pub gold: i32,
    /// Banked gold in hundredths.
    pub bank: i32,
    /// Privilege/affiliation markers.
    pub roles: CharacterRoles,
}

impl PlayerSummary {
    /// Build a summary for a persisted character, or `None` if the slot is
    /// not a named player character.
    ///
    /// # Arguments
    ///
    /// * `ch` - Decoded character slot.
    ///
    /// # Returns
    ///
    /// * `Some` for player characters; `None` for empty slots and NPCs.
    pub fn from_character(ch: &Character) -> Option<Self> {
        let name = ch.get_name();
        if ch.used == USE_EMPTY || ch.flags & CharacterFlags::Player.bits() == 0 || name.is_empty()
        {
            return None;
        }
        Some(Self {
            name: name.to_owned(),
            points: ch.points_tot,
            gold: ch.gold,
            // Bank balance lives in `data[13]` (see `do_pay_depot`).
            bank: ch.data[13],
            roles: CharacterRoles::from_flags(ch.flags, ch.kindred),
        })
    }

    /// Carried plus banked gold, in hundredths.
    ///
    /// # Returns
    ///
    /// * Saturating sum of `gold` and `bank`.
    pub fn wealth(&self) -> i64 {
        i64::from(self.gold) + i64::from(self.bank)
    }
}

/// Cached leaderboard source data plus when it was scanned.
#[derive(Debug, Clone)]
pub struct LeaderboardSnapshot {
    /// Unix seconds when the scan completed.
    pub generated_at: u64,
    /// When the scan completed, for TTL checks.
    pub scanned: Instant,
    /// All player characters found in KeyDB.
    pub players: Arc<Vec<PlayerSummary>>,
}

/// Shared, lazily refreshed leaderboard cache.
///
/// The mutex is held across the KeyDB scan on purpose: concurrent callers
/// wait for one refresh instead of each issuing their own 8k-key pipeline.
pub type LeaderboardCache = Arc<tokio::sync::Mutex<Option<LeaderboardSnapshot>>>;

/// Create an empty [`LeaderboardCache`].
///
/// # Returns
///
/// * A cache that will populate on first use.
pub fn new_leaderboard_cache() -> LeaderboardCache {
    Arc::new(tokio::sync::Mutex::new(None))
}

/// Build the `/world` sub-router.
///
/// # Returns
///
/// * Router with the public world-information endpoints.
pub fn router() -> Router<ApiState> {
    Router::new()
        .route("/time", get(get_time))
        .route("/online", get(get_online))
        .route("/top", get(get_top))
        .route("/richest", get(get_richest))
}

/// `?limit=` query for the leaderboard endpoints.
#[derive(Debug, Deserialize)]
pub struct LimitQuery {
    /// Requested number of entries; clamped to `1..=MAX_LEADERBOARD_LIMIT`.
    pub limit: Option<usize>,
}

impl LimitQuery {
    /// Resolve the effective limit.
    ///
    /// # Returns
    ///
    /// * The clamped limit.
    pub fn effective(&self) -> usize {
        self.limit
            .unwrap_or(DEFAULT_LEADERBOARD_LIMIT)
            .clamp(1, MAX_LEADERBOARD_LIMIT)
    }
}

/// Response for `GET /world/time`.
#[derive(Debug, Serialize)]
pub struct TimeResponse {
    /// Unix seconds when the server published this snapshot.
    pub published_at: u64,
    /// Calendar/clock fields.
    #[serde(flatten)]
    pub time: WorldTime,
    /// `"It's 0:40 on the 8th of the 11th month of the year 50."`
    pub sentence: String,
    /// `"The Moon is growing."` etc.
    pub moon_description: &'static str,
}

/// Response for `GET /world/online`.
#[derive(Debug, Serialize)]
pub struct OnlineResponse {
    /// Unix seconds when the server published this snapshot.
    pub published_at: u64,
    /// Number of visible players.
    pub count: usize,
    /// Visible players in slot order.
    pub players: Vec<OnlinePlayer>,
}

/// One entry of `GET /world/top`.
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct RankedPlayer {
    /// Leaderboard position, 1-based.
    pub position: usize,
    /// Character name.
    pub name: String,
    /// Total experience points.
    pub points: i32,
    /// Full rank title.
    pub rank: &'static str,
    /// Privilege/affiliation markers.
    pub roles: CharacterRoles,
}

/// One entry of `GET /world/richest`.
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct RichPlayer {
    /// Leaderboard position, 1-based.
    pub position: usize,
    /// Character name.
    pub name: String,
    /// Carried gold in hundredths.
    pub gold: i32,
    /// Banked gold in hundredths.
    pub bank: i32,
    /// `gold + bank`, in hundredths.
    pub total: i64,
    /// Privilege/affiliation markers.
    pub roles: CharacterRoles,
}

/// Envelope for both leaderboards.
#[derive(Debug, Serialize)]
pub struct LeaderboardResponse<T: Serialize> {
    /// Unix seconds when the underlying character scan ran.
    pub generated_at: u64,
    /// Effective limit after clamping.
    pub limit: usize,
    /// Ordered entries, best first.
    pub players: Vec<T>,
}

/// GET `/world/time` — in-game clock, calendar and moon phase.
pub(crate) async fn get_time(State(state): State<ApiState>) -> Response {
    match load_live_status(&state).await {
        Ok(status) => Json(TimeResponse {
            published_at: status.published_at,
            sentence: status.time.sentence(),
            moon_description: status.time.moon.description(),
            time: status.time,
        })
        .into_response(),
        Err(response) => response,
    }
}

/// GET `/world/online` — players currently visible on the who list.
pub(crate) async fn get_online(State(state): State<ApiState>) -> Response {
    match load_live_status(&state).await {
        Ok(status) => Json(OnlineResponse {
            published_at: status.published_at,
            count: status.online.len(),
            players: status.online,
        })
        .into_response(),
        Err(response) => response,
    }
}

/// GET `/world/top?limit=` — highest experience totals.
pub(crate) async fn get_top(
    State(state): State<ApiState>,
    Query(query): Query<LimitQuery>,
) -> Response {
    let limit = query.effective();
    match load_leaderboard(&state).await {
        Ok(snapshot) => Json(LeaderboardResponse {
            generated_at: snapshot.generated_at,
            limit,
            players: top_ranked(&snapshot.players, limit),
        })
        .into_response(),
        Err(response) => response,
    }
}

/// GET `/world/richest?limit=` — highest carried + banked gold.
pub(crate) async fn get_richest(
    State(state): State<ApiState>,
    Query(query): Query<LimitQuery>,
) -> Response {
    let limit = query.effective();
    match load_leaderboard(&state).await {
        Ok(snapshot) => Json(LeaderboardResponse {
            generated_at: snapshot.generated_at,
            limit,
            players: richest(&snapshot.players, limit),
        })
        .into_response(),
        Err(response) => response,
    }
}

/// Rank players by `points` descending (name ascending on ties).
///
/// # Arguments
///
/// * `players` - Source summaries.
/// * `limit` - Maximum entries to return.
///
/// # Returns
///
/// * Up to `limit` entries with 1-based positions.
pub fn top_ranked(players: &[PlayerSummary], limit: usize) -> Vec<RankedPlayer> {
    let mut sorted: Vec<&PlayerSummary> = players.iter().collect();
    sorted.sort_by(|a, b| b.points.cmp(&a.points).then_with(|| a.name.cmp(&b.name)));
    sorted
        .into_iter()
        .take(limit)
        .enumerate()
        .map(|(i, p)| RankedPlayer {
            position: i + 1,
            name: p.name.clone(),
            points: p.points,
            rank: mag_core::ranks::rank_name(p.points.unsigned_abs()),
            roles: p.roles,
        })
        .collect()
}

/// Rank players by `gold + bank` descending (name ascending on ties).
///
/// # Arguments
///
/// * `players` - Source summaries.
/// * `limit` - Maximum entries to return.
///
/// # Returns
///
/// * Up to `limit` entries with 1-based positions.
pub fn richest(players: &[PlayerSummary], limit: usize) -> Vec<RichPlayer> {
    let mut sorted: Vec<&PlayerSummary> = players.iter().collect();
    sorted.sort_by(|a, b| {
        b.wealth()
            .cmp(&a.wealth())
            .then_with(|| a.name.cmp(&b.name))
    });
    sorted
        .into_iter()
        .take(limit)
        .enumerate()
        .map(|(i, p)| RichPlayer {
            position: i + 1,
            name: p.name.clone(),
            gold: p.gold,
            bank: p.bank,
            total: p.wealth(),
            roles: p.roles,
        })
        .collect()
}

/// Read and decode the server's live status snapshot.
async fn load_live_status(state: &ApiState) -> Result<LiveStatus, Response> {
    let mut con = state.con.clone();
    let bytes: Option<Vec<u8>> = con.get(LIVE_STATUS_KEY).await.map_err(|e| {
        warn!("world: GET {LIVE_STATUS_KEY} failed: {e}");
        error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "keydb_error",
            "Failed to read live status",
        )
    })?;
    let bytes = bytes.ok_or_else(|| {
        error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "server_offline",
            "The game server is not currently publishing live status",
        )
    })?;
    LiveStatus::from_bytes(&bytes).ok_or_else(|| {
        warn!("world: decode of {LIVE_STATUS_KEY} failed");
        error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "decode_error",
            "Failed to decode live status",
        )
    })
}

/// Return the cached leaderboard snapshot, rescanning KeyDB when stale.
async fn load_leaderboard(state: &ApiState) -> Result<LeaderboardSnapshot, Response> {
    let mut guard = state.leaderboard_cache.lock().await;
    if let Some(snapshot) = guard.as_ref()
        && snapshot.scanned.elapsed() < LEADERBOARD_CACHE_TTL
    {
        return Ok(snapshot.clone());
    }

    let players = scan_players(state).await.map_err(|e| {
        warn!("world: character scan failed: {e}");
        error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "keydb_error",
            "Failed to read characters",
        )
    })?;
    let snapshot = LeaderboardSnapshot {
        generated_at: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0),
        scanned: Instant::now(),
        players: Arc::new(players),
    };
    *guard = Some(snapshot.clone());
    Ok(snapshot)
}

/// Pipeline-read every persisted character slot and keep the player ones.
async fn scan_players(state: &ApiState) -> Result<Vec<PlayerSummary>, redis::RedisError> {
    let mut con = state.con.clone();
    let mut players = Vec::new();
    for batch_start in (0..CHARACTER_SLOT_COUNT).step_by(PIPELINE_BATCH_SIZE) {
        let batch_end = (batch_start + PIPELINE_BATCH_SIZE).min(CHARACTER_SLOT_COUNT);
        let mut pipeline = redis::pipe();
        for idx in batch_start..batch_end {
            pipeline.cmd("GET").arg(character_store::character_key(idx));
        }
        let batch: Vec<Option<Vec<u8>>> = pipeline.query_async(&mut con).await?;
        for (rel, bytes) in batch.into_iter().enumerate() {
            let Some(bytes) = bytes else { continue };
            match Character::from_bytes(&bytes) {
                Some(ch) => players.extend(PlayerSummary::from_character(&ch)),
                None => warn!(
                    "world: decode failed for character slot {}",
                    batch_start + rel
                ),
            }
        }
    }
    Ok(players)
}

fn error_response(status: StatusCode, code: &str, message: &str) -> Response {
    (status, Json(ErrorResponse::new(code, message))).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn summary(name: &str, points: i32, gold: i32, bank: i32) -> PlayerSummary {
        PlayerSummary {
            name: name.to_owned(),
            points,
            gold,
            bank,
            roles: CharacterRoles::default(),
        }
    }

    #[test]
    fn limit_query_clamps() {
        assert_eq!(
            LimitQuery { limit: None }.effective(),
            DEFAULT_LEADERBOARD_LIMIT
        );
        assert_eq!(LimitQuery { limit: Some(0) }.effective(), 1);
        assert_eq!(LimitQuery { limit: Some(5) }.effective(), 5);
        assert_eq!(
            LimitQuery {
                limit: Some(10_000)
            }
            .effective(),
            MAX_LEADERBOARD_LIMIT
        );
    }

    #[test]
    fn from_character_filters_non_players() {
        let mut ch = Character::default();
        assert!(PlayerSummary::from_character(&ch).is_none());

        ch.used = mag_core::constants::USE_NONACTIVE;
        ch.set_name("Piltur");
        assert!(
            PlayerSummary::from_character(&ch).is_none(),
            "NPCs are skipped"
        );

        ch.flags = (CharacterFlags::Player | CharacterFlags::Imp).bits();
        ch.points_tot = 42;
        ch.gold = 150;
        ch.data[13] = 1_000;
        let summary = PlayerSummary::from_character(&ch).unwrap();
        assert_eq!(summary.name, "Piltur");
        assert_eq!(
            (summary.points, summary.gold, summary.bank),
            (42, 150, 1_000)
        );
        assert_eq!(summary.wealth(), 1_150);
        assert!(summary.roles.imp);
    }

    #[test]
    fn leaderboards_sort_and_limit() {
        let players = vec![
            summary("Bob", 100, 5, 5),
            summary("Alice", 100, 1, 0),
            summary("Carol", 900, 0, i32::MAX),
            summary("Dave", 50, i32::MAX, 1),
        ];

        let top = top_ranked(&players, 3);
        let names: Vec<&str> = top.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["Carol", "Alice", "Bob"]);
        assert_eq!(top[0].position, 1);
        assert_eq!(top[0].rank, mag_core::ranks::rank_name(900));

        let rich = richest(&players, 2);
        let names: Vec<&str> = rich.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["Dave", "Carol"]);
        assert_eq!(
            rich[0].total,
            i64::from(i32::MAX) + 1,
            "sum must not overflow i32"
        );
    }
}
