//! JSON payloads used by admin endpoints.
//!
//! Full template GET/PUT requests use raw bincode bytes (see
//! [`super::routes`]); only listing summaries and reload coordination travel
//! as JSON.

use serde::{Deserialize, Serialize};

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
