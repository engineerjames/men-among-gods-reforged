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
