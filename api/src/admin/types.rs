//! JSON payloads used by admin endpoints.
//!
//! Full template GET/PUT requests use raw bincode bytes (see
//! [`super::routes`]); only listing summaries and reload coordination travel
//! as JSON.

use serde::{Deserialize, Serialize};

/// JSON target selector used when creating a ban.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "scope", rename_all = "snake_case")]
pub enum BanTargetRequest {
    /// Account ban selected by account id or username.
    Account {
        /// Optional API account id.
        #[serde(default)]
        account_id: Option<u64>,
        /// Optional username to resolve to an account id.
        #[serde(default)]
        username: Option<String>,
    },
    /// Character ban selected by API character id.
    Character {
        /// API character id.
        character_id: u64,
    },
    /// IPv4 ban selected by dotted IPv4 address.
    Ipv4 {
        /// Dotted IPv4 address.
        address: String,
    },
}

/// Request body for `POST /admin/bans`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BanCreateRequest {
    /// Target to ban.
    pub target: BanTargetRequest,
    /// Optional operator reason.
    #[serde(default)]
    pub reason: Option<String>,
    /// Optional Unix timestamp when the ban expires.
    #[serde(default)]
    pub expires_at: Option<u64>,
    /// Optional relative duration in seconds. Mutually exclusive with `expires_at`.
    #[serde(default)]
    pub duration_seconds: Option<u64>,
    /// Optional issuer label. Defaults to `admin_api`.
    #[serde(default)]
    pub created_by: Option<String>,
    /// Whether the live server should kick matching online players.
    #[serde(default)]
    pub kick_online: Option<bool>,
}

/// JSON view of a ban target.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BanTargetResponse {
    /// Target scope: `account`, `character`, or `ipv4`.
    pub scope: String,
    /// Human-readable target value.
    pub value: String,
    /// Account id when the scope is `account`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account_id: Option<u64>,
    /// Character id when the scope is `character`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub character_id: Option<u64>,
    /// Dotted IPv4 address when the scope is `ipv4`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address: Option<String>,
}

/// JSON view of an active or expired ban record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BanRecordResponse {
    /// Opaque ban id.
    pub id: String,
    /// Target covered by this ban.
    pub target: BanTargetResponse,
    /// Operator reason.
    pub reason: String,
    /// Issuer label.
    pub created_by: String,
    /// Creation Unix timestamp.
    pub created_at: u64,
    /// Optional expiration Unix timestamp.
    pub expires_at: Option<u64>,
    /// Whether the ban is active at response time.
    pub active: bool,
    /// Mutation source label.
    pub source: String,
}

/// Response for `GET /admin/bans`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BanListResponse {
    /// Matching ban records.
    pub bans: Vec<BanRecordResponse>,
    /// Number of records returned.
    pub count: usize,
    /// Ban-store version counter.
    pub version: u64,
}

/// Response returned by ban mutation endpoints.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BanMutationResponse {
    /// Record after a create/update, or removed record after an unban.
    pub ban: Option<BanRecordResponse>,
    /// Whether persistent state changed.
    pub changed: bool,
    /// Ban-store version counter.
    pub version: u64,
    /// Optional live-action request id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub live_request_id: Option<String>,
}

/// Per-slot summary returned by listing endpoints.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateSummary {
    /// Slot index (`0..slot_count`).
    pub id: usize,
    /// `true` when the slot's `used` field is non-zero.
    pub used: bool,
    /// UTF-8 representation of the template's `name` field, NUL-trimmed.
    pub name: String,
    /// UTF-8 representation of the template's `reference` field, NUL-trimmed.
    /// Empty for character templates.
    #[serde(default)]
    pub reference: String,
}

/// Listing response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateListResponse {
    /// Total slot count for the addressed kind.
    pub total: usize,
    /// Inclusive start index of the returned page.
    pub from: usize,
    /// Number of summaries returned in this page.
    pub count: usize,
    /// Per-slot summaries.
    pub items: Vec<TemplateSummary>,
}

/// Optional pagination query for listing endpoints.
#[derive(Debug, Clone, Deserialize)]
pub struct TemplateListQuery {
    /// Inclusive start index. Defaults to `0`.
    pub from: Option<usize>,
    /// Maximum entries to return. Defaults to slot_count (no paging).
    pub limit: Option<usize>,
}

/// Body for `POST /admin/templates/reload`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReloadRequest {
    /// Which template kinds to reload.
    ///
    /// Recognised values: `"items"`, `"characters"`. Unknown values are
    /// rejected with `400`.
    pub kinds: Vec<String>,
}

/// Response for `POST /admin/templates/reload`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReloadResponse {
    /// Opaque identifier the caller polls via [`super::routes::get_reload_status`].
    pub request_id: String,
    /// Echoes the validated kinds list.
    pub kinds: Vec<String>,
}

/// Status snapshot for `GET /admin/templates/reload/status`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReloadStatusResponse {
    /// One of `"pending"`, `"applied"`, or `"expired"`.
    pub status: String,
    /// Opaque identifier the caller passed in.
    pub request_id: String,
}

/// Response shape for `PUT /admin/templates/{kind}/{id}`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PutTemplateResponse {
    /// New value of the kind's version counter (post-increment).
    pub version: u64,
}

/// Generic JSON error envelope.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorResponse {
    /// Short machine-readable code, e.g. `"out_of_range"`.
    pub error: String,
    /// Human-readable description.
    pub message: String,
}

impl ErrorResponse {
    /// Build an error envelope.
    ///
    /// # Arguments
    ///
    /// * `code`    - Short machine-readable identifier.
    /// * `message` - Human-readable description.
    ///
    /// # Returns
    ///
    /// * The constructed [`ErrorResponse`].
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            error: code.into(),
            message: message.into(),
        }
    }
}

/// Response for `GET /admin/text/badwords`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BadwordsListResponse {
    /// Canonical badword entries in stable storage order.
    pub words: Vec<String>,
    /// Number of entries in `words`.
    pub count: usize,
    /// Current badwords version counter.
    pub version: u64,
}

/// Response for `GET /admin/text/badwords/entry`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BadwordEntryResponse {
    /// Canonicalized query word.
    pub word: String,
    /// Whether `word` currently exists in the badwords list.
    pub exists: bool,
}

/// Request body used by badwords mutation endpoints.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BadwordsMutationRequest {
    /// Raw words to add, remove, or use as the replacement list.
    pub words: Vec<String>,
}

/// Response returned by badwords mutation endpoints.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BadwordsMutationResponse {
    /// Canonical badword entries after the mutation.
    pub words: Vec<String>,
    /// Number of entries after the mutation.
    pub count: usize,
    /// Current badwords version counter.
    pub version: u64,
    /// Canonical words newly added by this mutation.
    pub added: Vec<String>,
    /// Canonical words removed by this mutation.
    pub removed: Vec<String>,
    /// Canonical requested words that left storage unchanged.
    pub unchanged: Vec<String>,
}

/// Body for `POST /admin/text/reload`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextReloadRequest {
    /// Which text data kinds to reload.
    ///
    /// Recognised values: `"badwords"`. Unknown values are rejected with `400`.
    pub kinds: Vec<String>,
}

/// Response for `POST /admin/text/reload`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextReloadResponse {
    /// Opaque identifier the caller polls via the reload-status endpoint.
    pub request_id: String,
    /// Echoes the validated reload kinds.
    pub kinds: Vec<String>,
}

/// Status snapshot for `GET /admin/text/reload/status`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextReloadStatusResponse {
    /// One of `"pending"` or `"applied"`.
    pub status: String,
    /// Opaque identifier echoed back by the API.
    pub request_id: String,
}

/// Response shape for `PUT /admin/world/map/{x}/{y}`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PutMapTileResponse {
    /// New value of the map version counter (post-increment).
    pub version: u64,
    /// Number of patches now waiting in the server-side patch queue.
    pub queued: u64,
}

/// Body for `POST /admin/world/map/reload`.
///
/// Currently empty — the server flushes the entire patch queue on every
/// request. Reserved for future selective reload semantics.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MapReloadRequest {}

/// Response for `POST /admin/world/map/reload`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MapReloadResponse {
    /// Opaque identifier the caller polls via the status endpoint.
    pub request_id: String,
}

/// Status snapshot for `GET /admin/world/map/reload/status`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MapReloadStatusResponse {
    /// One of `"pending"`, `"applied"`, or `"expired"`.
    pub status: String,
    /// Opaque identifier the caller passed in.
    pub request_id: String,
}

/// Response for `GET /admin/world/map/version`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MapVersionResponse {
    /// Current value of the map version counter.
    pub version: u64,
}

/// JSON view of the persisted global game-state counters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalsResponse {
    /// Current in-game minute/time counter.
    pub mdtime: i32,
    /// Current in-game day.
    pub mdday: i32,
    /// Current in-game year.
    pub mdyear: i32,
    /// Current daylight value.
    pub dlight: i32,
    /// Total player characters created.
    pub players_created: i32,
    /// Total NPCs created.
    pub npcs_created: i32,
    /// Total player deaths.
    pub players_died: i32,
    /// Total NPC deaths.
    pub npcs_died: i32,
    /// Current character count.
    pub character_cnt: i32,
    /// Current item count.
    pub item_cnt: i32,
    /// Current effect count.
    pub effect_cnt: i32,
    /// Expiration pass counter.
    pub expire_cnt: i32,
    /// Expiration run marker.
    pub expire_run: i32,
    /// Garbage-collection pass counter.
    pub gc_cnt: i32,
    /// Garbage-collection run marker.
    pub gc_run: i32,
    /// Lost-object pass counter.
    pub lost_cnt: i32,
    /// Lost-object run marker.
    pub lost_run: i32,
    /// Character reset counter.
    pub reset_char: i32,
    /// Item reset counter.
    pub reset_item: i32,
    /// Server tick counter.
    pub ticker: i32,
    /// Total player online time.
    pub total_online_time: i64,
    /// Online time bucketed by hour.
    pub online_per_hour: [i64; 24],
    /// Global flag bitfield.
    pub flags: i32,
    /// Total server uptime.
    pub uptime: i64,
    /// Server uptime bucketed by hour.
    pub uptime_per_hour: [i64; 24],
    /// Awake-state counter.
    pub awake: i32,
    /// Body-state counter.
    pub body: i32,
    /// Current number of online players.
    pub players_online: i32,
    /// Current queue size.
    pub queuesize: i32,
    /// Total received bytes.
    pub recv: i64,
    /// Total sent bytes.
    pub send: i64,
    /// Transfer reset time marker.
    pub transfer_reset_time: i32,
    /// Current load average.
    pub load_avg: i32,
    /// Current raw load value.
    pub load: i64,
    /// Maximum online player count.
    pub max_online: i32,
    /// Maximum online count bucketed by hour.
    pub max_online_per_hour: [i32; 24],
    /// Full-moon marker.
    pub fullmoon: i8,
    /// New-moon marker.
    pub newmoon: i8,
    /// Unique id counter.
    pub unique: u64,
    /// Current cap value.
    pub cap: i32,
    /// Whether the global dirty flag is currently set.
    pub dirty: bool,
}

impl From<mag_core::types::Global> for GlobalsResponse {
    /// Build a JSON response DTO from the bincode-backed global state.
    ///
    /// # Arguments
    ///
    /// * `global` - Persisted global game-state value.
    ///
    /// # Returns
    ///
    /// * A [`GlobalsResponse`] with every operator-facing field copied out.
    fn from(global: mag_core::types::Global) -> Self {
        Self {
            mdtime: global.mdtime,
            mdday: global.mdday,
            mdyear: global.mdyear,
            dlight: global.dlight,
            players_created: global.players_created,
            npcs_created: global.npcs_created,
            players_died: global.players_died,
            npcs_died: global.npcs_died,
            character_cnt: global.character_cnt,
            item_cnt: global.item_cnt,
            effect_cnt: global.effect_cnt,
            expire_cnt: global.expire_cnt,
            expire_run: global.expire_run,
            gc_cnt: global.gc_cnt,
            gc_run: global.gc_run,
            lost_cnt: global.lost_cnt,
            lost_run: global.lost_run,
            reset_char: global.reset_char,
            reset_item: global.reset_item,
            ticker: global.ticker,
            total_online_time: global.total_online_time,
            online_per_hour: global.online_per_hour,
            flags: global.flags,
            uptime: global.uptime,
            uptime_per_hour: global.uptime_per_hour,
            awake: global.awake,
            body: global.body,
            players_online: global.players_online,
            queuesize: global.queuesize,
            recv: global.recv,
            send: global.send,
            transfer_reset_time: global.transfer_reset_time,
            load_avg: global.load_avg,
            load: global.load,
            max_online: global.max_online,
            max_online_per_hour: global.max_online_per_hour,
            fullmoon: global.fullmoon,
            newmoon: global.newmoon,
            unique: global.unique,
            cap: global.cap,
            dirty: global.is_dirty(),
        }
    }
}

// ---------------------------------------------------------------------------
//  Items / characters (live world state)
// ---------------------------------------------------------------------------

/// Per-slot summary for `GET /admin/world/{items|characters}/list`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldEntitySummary {
    /// Slot index.
    pub id: usize,
    /// `true` when the slot's `used` field is non-zero.
    pub used: bool,
    /// UTF-8 representation of the entity's `name` field, NUL-trimmed.
    pub name: String,
    /// UTF-8 representation of the entity's `reference` field, NUL-trimmed.
    pub reference: String,
}

/// Listing response for `GET /admin/world/{items|characters}/list`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldEntityListResponse {
    /// Total slot count for the addressed kind.
    pub total: usize,
    /// Inclusive start index of the returned page.
    pub from: usize,
    /// Number of summaries returned in this page.
    pub count: usize,
    /// Per-slot summaries.
    pub items: Vec<WorldEntitySummary>,
}

/// Optional pagination query for world listing endpoints.
#[derive(Debug, Clone, Deserialize)]
pub struct WorldEntityListQuery {
    /// Inclusive start index. Defaults to `0`.
    pub from: Option<usize>,
    /// Maximum entries to return. Defaults to `256`, capped at `4096`.
    pub limit: Option<usize>,
}

/// Response shape for `PUT /admin/world/items/{id}` and
/// `PUT /admin/world/characters/{id}`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PutWorldEntityResponse {
    /// New value of the kind's version counter (post-increment).
    pub version: u64,
    /// Number of patches now waiting in the server-side patch queue.
    pub queued: u64,
}

/// Body for `POST /admin/world/{items|characters}/reload`. Reserved for
/// future selective reload semantics; currently unused.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorldEntityReloadRequest {}

/// Response for `POST /admin/world/{items|characters}/reload`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldEntityReloadResponse {
    /// Opaque identifier the caller polls via the status endpoint.
    pub request_id: String,
}

/// Status snapshot for `GET /admin/world/{items|characters}/reload/status`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldEntityReloadStatusResponse {
    /// One of `"pending"`, `"applied"`, or `"expired"`.
    pub status: String,
    /// Opaque identifier the caller passed in.
    pub request_id: String,
}

/// Response for `GET /admin/world/{items|characters}/version`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldEntityVersionResponse {
    /// Current value of the kind's version counter.
    pub version: u64,
}
