//! Public, read-only world information routes mounted at `/world/*`.
//!
//! Two data sources back these endpoints:
//!
//! * `/world/time` and `/world/online` read the compact
//!   [`LiveStatus`] snapshot the game server republishes every ~10 s under
//!   [`LIVE_STATUS_KEY`]. If the key is absent the server is down (the key
//!   carries a TTL) and the endpoints answer `503`.
//! * `/world/leaderboard/{board}` and `/world/stats` scan the persisted
//!   `game:char:*` slots (flushed by the server every ~12 min) and keep a
//!   process-local cache for [`LEADERBOARD_CACHE_TTL`], so repeated polling
//!   from bots never costs more than one 8k-key pipeline per minute.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use axum::Json;
use axum::Router;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use log::warn;
use mag_core::character_store::{self, CHARACTER_SLOT_COUNT};
use mag_core::constants::{CharacterFlags, TICKS, USE_EMPTY};
use mag_core::live_status_store::{
    CharacterRoles, LIVE_STATUS_KEY, LiveStatus, OnlinePlayer, WorldTime,
};
use mag_core::ranks::{self, TOTAL_RANKS};
use mag_core::traits;
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

/// A "veteran" must have logged in within this many seconds.
pub const VETERAN_RECENT_LOGIN_SECS: u64 = 30 * 24 * 3600;

/// Number of `GET`s pipelined per KeyDB round trip while scanning characters.
const PIPELINE_BATCH_SIZE: usize = 4096;

/// Compact per-player fields needed by the leaderboards and stats.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerSummary {
    /// Character name.
    pub name: String,
    /// Total experience points.
    pub points: i32,
    /// Carried gold in hundredths (`/100` = gold, `%100` = silver).
    pub gold: i32,
    /// Banked gold in hundredths (`data[13]`).
    pub bank: i32,
    /// Player kills (`data[29]`).
    pub pvp_kills: i32,
    /// Times died (`data[14]`).
    pub deaths: i32,
    /// Kills of NPCs below own rank (`data[23]`).
    pub kills_below: i32,
    /// Kills of NPCs at own rank (`data[24]`).
    pub kills_at: i32,
    /// Kills of NPCs above own rank (`data[25]`).
    pub kills_above: i32,
    /// Black Stronghold kills below own rank (`data[26]`).
    pub stronghold_below: i32,
    /// Black Stronghold kills at own rank (`data[27]`).
    pub stronghold_at: i32,
    /// Black Stronghold kills above own rank (`data[28]`).
    pub stronghold_above: i32,
    /// Stronghold candles (`data[43]`).
    pub candles: i32,
    /// Unspent stronghold points ([`Character::stronghold_points`]).
    pub stronghold_points: i32,
    /// Shopkeeper / quest-giver murders (`data[40]`).
    pub infamy: i32,
    /// Lifetime online time in ticks.
    pub online_ticks: u32,
    /// Unix seconds the character was created (0 when unknown).
    pub created: u32,
    /// Unix seconds of the last login (0 when unknown).
    pub last_login: u32,
    /// Raw kindred bit set.
    pub kindred: i32,
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
            bank: ch.data[13],
            pvp_kills: ch.data[29],
            deaths: ch.data[14],
            kills_below: ch.data[23],
            kills_at: ch.data[24],
            kills_above: ch.data[25],
            stronghold_below: ch.data[26],
            stronghold_at: ch.data[27],
            stronghold_above: ch.data[28],
            candles: ch.data[43],
            stronghold_points: ch.stronghold_points(),
            infamy: ch.data[40],
            online_ticks: ch.total_online_time,
            created: ch.creation_date,
            last_login: ch.login_date,
            kindred: ch.kindred,
            roles: CharacterRoles::from_flags(ch.flags, ch.kindred),
        })
    }

    /// Carried plus banked gold, in hundredths.
    ///
    /// # Returns
    ///
    /// * Overflow-safe sum of `gold` and `bank`.
    pub fn wealth(&self) -> i64 {
        i64::from(self.gold) + i64::from(self.bank)
    }

    /// Lifetime online time in whole seconds.
    ///
    /// # Returns
    ///
    /// * `online_ticks / TICKS`.
    pub fn online_seconds(&self) -> i64 {
        i64::from(self.online_ticks) / i64::from(TICKS)
    }
}

/// Which leaderboard to compute. Path segment of `/world/leaderboard/{board}`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Board {
    /// Highest `points_tot`.
    Points,
    /// Highest carried + banked gold.
    Wealth,
    /// Most player kills.
    PvpKills,
    /// Most deaths.
    Deaths,
    /// Most kills of NPCs above own rank.
    GiantSlayers,
    /// Most lifetime online time.
    Playtime,
    /// Earliest creation date.
    Oldest,
    /// Earliest creation date among players seen in the last
    /// [`VETERAN_RECENT_LOGIN_SECS`].
    Veterans,
    /// Most unspent Black Stronghold points.
    Stronghold,
    /// Most shopkeeper / quest-giver murders.
    Infamous,
}

impl Board {
    /// Human-readable description of `value` for this board.
    ///
    /// # Returns
    ///
    /// * A short unit/metric label.
    pub fn metric(self) -> &'static str {
        match self {
            Self::Points => "experience points",
            Self::Wealth => "gold + bank, in hundredths",
            Self::PvpKills => "player kills",
            Self::Deaths => "deaths",
            Self::GiantSlayers => "kills above own rank",
            Self::Playtime => "online time, seconds",
            Self::Oldest | Self::Veterans => "creation date, unix seconds",
            Self::Stronghold => "unspent stronghold points",
            Self::Infamous => "shopkeeper and quest-giver murders",
        }
    }

    /// The metric shown as `value` for a player.
    fn value(self, p: &PlayerSummary) -> i64 {
        match self {
            Self::Points => p.points.into(),
            Self::Wealth => p.wealth(),
            Self::PvpKills => p.pvp_kills.into(),
            Self::Deaths => p.deaths.into(),
            Self::GiantSlayers => p.kills_above.into(),
            Self::Playtime => p.online_seconds(),
            Self::Oldest | Self::Veterans => p.created.into(),
            Self::Stronghold => p.stronghold_points.into(),
            Self::Infamous => p.infamy.into(),
        }
    }

    /// Sort key: higher is better for every board.
    fn sort_key(self, p: &PlayerSummary) -> i64 {
        match self {
            Self::Oldest | Self::Veterans => -self.value(p),
            _ => self.value(p),
        }
    }

    /// Whether a player qualifies for this board at all.
    fn includes(self, p: &PlayerSummary, now: u64) -> bool {
        match self {
            Self::Points | Self::Wealth | Self::Stronghold => true,
            Self::Oldest => p.created > 0,
            Self::Veterans => {
                p.created > 0 && u64::from(p.last_login) + VETERAN_RECENT_LOGIN_SECS >= now
            }
            _ => self.value(p) > 0,
        }
    }

    /// Supplementary per-entry fields.
    fn extra(self, p: &PlayerSummary) -> BTreeMap<&'static str, i64> {
        let mut m = BTreeMap::new();
        match self {
            Self::Wealth => {
                m.insert("gold", p.gold.into());
                m.insert("bank", p.bank.into());
            }
            Self::PvpKills | Self::Deaths => {
                m.insert("pvp_kills", p.pvp_kills.into());
                m.insert("deaths", p.deaths.into());
            }
            Self::GiantSlayers => {
                m.insert("kills_below", p.kills_below.into());
                m.insert("kills_at", p.kills_at.into());
                m.insert("kills_above", p.kills_above.into());
            }
            Self::Playtime => {
                m.insert("hours", p.online_seconds() / 3600);
            }
            Self::Oldest | Self::Veterans => {
                m.insert("last_login", p.last_login.into());
            }
            Self::Stronghold => {
                m.insert("kills_below", p.stronghold_below.into());
                m.insert("kills_at", p.stronghold_at.into());
                m.insert("kills_above", p.stronghold_above.into());
                m.insert("candles", p.candles.into());
            }
            Self::Points | Self::Infamous => {}
        }
        m
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
        .route("/stats", get(get_stats))
        .route("/leaderboard/{board}", get(get_leaderboard))
        // Shortcuts for the two most common boards.
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
    /// Server uptime in whole seconds.
    pub uptime_seconds: i64,
    /// Visible players online right now.
    pub players_online: i32,
    /// All-time peak concurrent players.
    pub max_online: i32,
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

/// One leaderboard row.
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct LeaderboardEntry {
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
    /// The board's metric for this player (see `metric` on the envelope).
    pub value: i64,
    /// Board-specific supplementary numbers (e.g. `gold`/`bank`).
    #[serde(flatten)]
    pub extra: BTreeMap<&'static str, i64>,
}

/// Envelope for `GET /world/leaderboard/{board}`.
#[derive(Debug, Serialize)]
pub struct LeaderboardResponse {
    /// Unix seconds when the underlying character scan ran.
    pub generated_at: u64,
    /// Which board this is.
    pub board: Board,
    /// What `value` means for this board.
    pub metric: &'static str,
    /// Effective limit after clamping.
    pub limit: usize,
    /// Ordered entries, best first.
    pub players: Vec<LeaderboardEntry>,
}

/// One bucket of the rank histogram.
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct RankBucket {
    /// Rank index, 0-based.
    pub index: usize,
    /// Rank title.
    pub rank: &'static str,
    /// Players currently holding this rank.
    pub count: usize,
}

/// Response for `GET /world/stats`.
#[derive(Debug, Serialize)]
pub struct StatsResponse {
    /// Unix seconds when the underlying character scan ran.
    pub generated_at: u64,
    /// Player characters found.
    pub total_players: usize,
    /// Players per rank, lowest rank first.
    pub ranks: Vec<RankBucket>,
    /// Players per class (`Mercenary`, `Templar`, `Harakim`, `SeyanDu`,
    /// `Warrior`, `Sorcerer`, `ArchTemplar`, `ArchHarakim`, `Unknown`).
    pub classes: BTreeMap<String, usize>,
    /// Players with the purple (PvP) kindred bit.
    pub purple: usize,
}

/// GET `/world/time` — in-game clock, calendar, moon phase and server stats.
pub(crate) async fn get_time(State(state): State<ApiState>) -> Response {
    match load_live_status(&state).await {
        Ok(status) => Json(TimeResponse {
            published_at: status.published_at,
            uptime_seconds: status.uptime_ticks / i64::from(TICKS),
            players_online: status.players_online,
            max_online: status.max_online,
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

/// GET `/world/leaderboard/{board}?limit=`.
pub(crate) async fn get_leaderboard(
    State(state): State<ApiState>,
    Path(board): Path<Board>,
    Query(query): Query<LimitQuery>,
) -> Response {
    leaderboard_response(&state, board, query.effective()).await
}

/// GET `/world/top?limit=` — alias for `/world/leaderboard/points`.
pub(crate) async fn get_top(
    State(state): State<ApiState>,
    Query(query): Query<LimitQuery>,
) -> Response {
    leaderboard_response(&state, Board::Points, query.effective()).await
}

/// GET `/world/richest?limit=` — alias for `/world/leaderboard/wealth`.
pub(crate) async fn get_richest(
    State(state): State<ApiState>,
    Query(query): Query<LimitQuery>,
) -> Response {
    leaderboard_response(&state, Board::Wealth, query.effective()).await
}

/// GET `/world/stats` — rank histogram and class breakdown.
pub(crate) async fn get_stats(State(state): State<ApiState>) -> Response {
    match load_leaderboard(&state).await {
        Ok(snapshot) => Json(stats(snapshot.generated_at, &snapshot.players)).into_response(),
        Err(response) => response,
    }
}

async fn leaderboard_response(state: &ApiState, board: Board, limit: usize) -> Response {
    match load_leaderboard(state).await {
        Ok(snapshot) => Json(LeaderboardResponse {
            generated_at: snapshot.generated_at,
            board,
            metric: board.metric(),
            limit,
            players: leaderboard(board, &snapshot.players, limit, snapshot.generated_at),
        })
        .into_response(),
        Err(response) => response,
    }
}

/// Compute a board: filter, sort best-first (name ascending on ties), take `limit`.
///
/// # Arguments
///
/// * `board` - Which metric to rank by.
/// * `players` - Source summaries.
/// * `limit` - Maximum entries to return.
/// * `now` - Current unix seconds (for recency-based boards).
///
/// # Returns
///
/// * Up to `limit` entries with 1-based positions.
pub fn leaderboard(
    board: Board,
    players: &[PlayerSummary],
    limit: usize,
    now: u64,
) -> Vec<LeaderboardEntry> {
    let mut sorted: Vec<&PlayerSummary> =
        players.iter().filter(|p| board.includes(p, now)).collect();
    sorted.sort_by(|a, b| {
        board
            .sort_key(b)
            .cmp(&board.sort_key(a))
            .then_with(|| a.name.cmp(&b.name))
    });
    sorted
        .into_iter()
        .take(limit)
        .enumerate()
        .map(|(i, p)| LeaderboardEntry {
            position: i + 1,
            name: p.name.clone(),
            points: p.points,
            rank: ranks::rank_name(p.points.unsigned_abs()),
            roles: p.roles,
            value: board.value(p),
            extra: board.extra(p),
        })
        .collect()
}

/// Build the rank histogram and class breakdown.
///
/// # Arguments
///
/// * `generated_at` - Unix seconds of the source scan.
/// * `players` - Source summaries.
///
/// # Returns
///
/// * Populated [`StatsResponse`].
pub fn stats(generated_at: u64, players: &[PlayerSummary]) -> StatsResponse {
    let mut rank_counts = [0usize; TOTAL_RANKS];
    let mut classes: BTreeMap<String, usize> = BTreeMap::new();
    let mut purple = 0;
    for p in players {
        let idx = (ranks::points2rank(p.points.unsigned_abs()) as usize).min(TOTAL_RANKS - 1);
        rank_counts[idx] += 1;
        let class = traits::class_from_kindred(p.kindred)
            .map_or_else(|| "Unknown".to_owned(), |c| format!("{c:?}"));
        *classes.entry(class).or_default() += 1;
        purple += usize::from(p.roles.purple);
    }
    StatsResponse {
        generated_at,
        total_players: players.len(),
        ranks: rank_counts
            .iter()
            .enumerate()
            .map(|(index, &count)| RankBucket {
                index,
                rank: ranks::rank_name_by_index(index),
                count,
            })
            .collect(),
        classes,
        purple,
    }
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

    const NOW: u64 = 1_800_000_000;

    fn summary(name: &str, points: i32) -> PlayerSummary {
        PlayerSummary {
            name: name.to_owned(),
            points,
            gold: 0,
            bank: 0,
            pvp_kills: 0,
            deaths: 0,
            kills_below: 0,
            kills_at: 0,
            kills_above: 0,
            stronghold_below: 0,
            stronghold_at: 0,
            stronghold_above: 0,
            candles: 0,
            stronghold_points: 0,
            infamy: 0,
            online_ticks: 0,
            created: 0,
            last_login: 0,
            kindred: traits::KIN_MERCENARY as i32,
            roles: CharacterRoles::default(),
        }
    }

    fn names(entries: &[LeaderboardEntry]) -> Vec<&str> {
        entries.iter().map(|e| e.name.as_str()).collect()
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
    fn from_character_filters_non_players_and_maps_data_slots() {
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
        ch.data[29] = 7;
        ch.data[14] = 3;
        ch.data[25] = 11;
        ch.data[40] = 2;
        ch.data[27] = 4;
        ch.data[43] = 1;
        ch.total_online_time = 36 * 7200;
        let s = PlayerSummary::from_character(&ch).unwrap();
        assert_eq!(s.name, "Piltur");
        assert_eq!((s.gold, s.bank, s.wealth()), (150, 1_000, 1_150));
        assert_eq!(
            (s.pvp_kills, s.deaths, s.kills_above, s.infamy),
            (7, 3, 11, 2)
        );
        assert_eq!(s.stronghold_points, 4 + 25);
        assert_eq!(s.online_seconds(), 7200);
        assert!(s.roles.imp);
    }

    #[test]
    fn points_and_wealth_boards_sort_and_limit() {
        let mut bob = summary("Bob", 100);
        bob.gold = 5;
        bob.bank = 5;
        let mut alice = summary("Alice", 100);
        alice.gold = 1;
        let mut carol = summary("Carol", 900);
        carol.bank = i32::MAX;
        let mut dave = summary("Dave", 50);
        dave.gold = i32::MAX;
        dave.bank = 1;
        let players = vec![bob, alice, carol, dave];

        let top = leaderboard(Board::Points, &players, 3, NOW);
        assert_eq!(names(&top), ["Carol", "Alice", "Bob"]);
        assert_eq!(top[0].position, 1);
        assert_eq!(top[0].rank, ranks::rank_name(900));

        let rich = leaderboard(Board::Wealth, &players, 2, NOW);
        assert_eq!(names(&rich), ["Dave", "Carol"]);
        assert_eq!(rich[0].value, i64::from(i32::MAX) + 1);
        assert_eq!(rich[0].extra["gold"], i64::from(i32::MAX));
        assert_eq!(rich[0].extra["bank"], 1);
    }

    #[test]
    fn count_boards_drop_zero_entries() {
        let mut killer = summary("Killer", 1);
        killer.pvp_kills = 3;
        let players = vec![summary("Pacifist", 1), killer];
        let board = leaderboard(Board::PvpKills, &players, 10, NOW);
        assert_eq!(names(&board), ["Killer"]);
        assert!(leaderboard(Board::Deaths, &players, 10, NOW).is_empty());
    }

    #[test]
    fn oldest_and_veterans_sort_ascending_by_creation() {
        let mut old = summary("Old", 1);
        old.created = 1_000;
        old.last_login = 1_000; // long gone
        let mut mid = summary("Mid", 1);
        mid.created = 5_000;
        mid.last_login = (NOW - 60) as u32;
        let mut new = summary("New", 1);
        new.created = 9_000;
        new.last_login = NOW as u32;
        let unknown = summary("Unknown", 1); // created == 0 excluded
        let players = vec![new, unknown, mid, old];

        assert_eq!(
            names(&leaderboard(Board::Oldest, &players, 10, NOW)),
            ["Old", "Mid", "New"]
        );
        assert_eq!(
            names(&leaderboard(Board::Veterans, &players, 10, NOW)),
            ["Mid", "New"]
        );
    }

    #[test]
    fn stats_bucket_ranks_and_classes() {
        let mut purple = summary("P", 0);
        purple.kindred = (traits::KIN_TEMPLAR | traits::KIN_PURPLE) as i32;
        purple.roles.purple = true;
        let mut high = summary("H", i32::MAX);
        high.kindred = 0;
        let players = vec![summary("A", 0), purple, high];

        let s = stats(NOW, &players);
        assert_eq!(s.total_players, 3);
        assert_eq!(s.ranks.len(), TOTAL_RANKS);
        assert_eq!(s.ranks[0].count, 2);
        assert_eq!(s.ranks[TOTAL_RANKS - 1].count, 1);
        assert_eq!(s.ranks.iter().map(|b| b.count).sum::<usize>(), 3);
        assert_eq!(s.classes["Mercenary"], 1);
        assert_eq!(s.classes["Templar"], 1);
        assert_eq!(s.classes["Unknown"], 1);
        assert_eq!(s.purple, 1);
    }
}
