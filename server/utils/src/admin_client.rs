//! Blocking HTTP client for the server admin API.
//!
//! Used by `template_viewer` (and any future tool) to read and write live
//! template data via the admin endpoints under `/admin/...`. The API answers
//! per-template GET/PUT requests with bincode bytes (`application/octet-stream`)
//! to avoid serializing fixed-size byte arrays through JSON.

use std::time::Duration;

use mag_core::map_store::MapPatch;
use mag_core::template_store::{
    self, TemplateKind, decode_character_template, decode_item_template, encode_character_template,
    encode_item_template,
};
use mag_core::types::{Character, Item, Map};
use reqwest::blocking::Client;
use reqwest::header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE};
use serde::Deserialize;

const OCTET_STREAM: &str = "application/octet-stream";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);

/// Per-slot summary returned by the listing endpoints.
///
/// Only `id`, `used`, `name`, and `reference` are present — not the full
/// binary template payload. Use this to populate list views cheaply, then
/// call [`AdminClient::fetch_single_item_template`] or
/// [`AdminClient::fetch_single_character_template`] to load an individual
/// slot's full data on demand.
#[derive(Debug, Clone, Deserialize)]
pub struct TemplateSummary {
    /// Slot index.
    pub id: usize,
    /// `true` when the slot's `used` field is non-zero.
    pub used: bool,
    /// Template name, NUL-trimmed.
    pub name: String,
    /// Template reference string (empty for character templates).
    #[serde(default)]
    pub reference: String,
}

/// Response envelope from `GET /admin/templates/{kind}`.
#[derive(Debug, Clone, Deserialize)]
pub struct TemplateListResponse {
    /// Total slot count for the kind.
    pub total: usize,
    /// Inclusive start index of this page.
    pub from: usize,
    /// Number of summaries in this response.
    pub count: usize,
    /// Per-slot summaries for slots that have stored data.
    pub items: Vec<TemplateSummary>,
}

/// Status returned by `POST /admin/templates/reload`.
#[derive(Debug, Clone, Deserialize)]
pub struct ReloadResponse {
    /// Opaque request id assigned by the API.
    pub request_id: String,
    /// Echoes the validated kinds list (e.g. `["items", "characters"]`).
    pub kinds: Vec<String>,
}

/// Status returned by `GET /admin/templates/reload/status?request_id=...`.
#[derive(Debug, Clone, Deserialize)]
pub struct ReloadStatusResponse {
    /// Lifecycle state: `pending`, `applied`, or `expired`.
    pub status: String,
    /// Opaque identifier echoed back by the API.
    pub request_id: String,
}

/// Response envelope from `PUT /admin/world/map/{x}/{y}`.
#[derive(Debug, Clone, Deserialize)]
pub struct PutMapTileResponse {
    /// New value of the map version counter.
    pub version: u64,
    /// Queue depth after this patch was enqueued.
    pub queued: u64,
}

/// Response envelope from `POST /admin/world/map/reload`.
#[derive(Debug, Clone, Deserialize)]
pub struct MapReloadResponse {
    /// Opaque identifier assigned by the API.
    pub request_id: String,
}

/// Response envelope from `GET /admin/world/map/reload/status?request_id=...`.
#[derive(Debug, Clone, Deserialize)]
pub struct MapReloadStatusResponse {
    /// Lifecycle state: `pending`, `applied`, or `expired`.
    pub status: String,
    /// Opaque identifier echoed back by the API.
    pub request_id: String,
}

/// Response envelope from `GET /admin/world/map/version`.
#[derive(Debug, Clone, Deserialize)]
pub struct MapVersionResponse {
    /// Current value of the map version counter.
    pub version: u64,
}

/// Blocking client for the admin API.
#[derive(Debug, Clone)]
pub struct AdminClient {
    base_url: String,
    token: String,
    client: Client,
}

impl AdminClient {
    /// Construct a new client pointed at `base_url` (no trailing slash) using
    /// `token` as the bearer credential.
    ///
    /// # Arguments
    ///
    /// * `base_url` - Base URL of the API service, e.g. `https://127.0.0.1:5554`.
    /// * `token` - Static admin bearer token.
    ///
    /// # Returns
    ///
    /// * `Ok(client)` on success.
    /// * `Err(message)` if the underlying HTTP client could not be built.
    pub fn new(base_url: impl Into<String>, token: impl Into<String>) -> Result<Self, String> {
        let client = Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .danger_accept_invalid_certs(true)
            .build()
            .map_err(|e| format!("admin client build failed: {e}"))?;
        Ok(Self {
            base_url: base_url.into().trim_end_matches('/').to_string(),
            token: token.into(),
            client,
        })
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base_url, path)
    }

    /// Fetch lightweight summaries for all item template slots.
    ///
    /// Issues **one** HTTP request regardless of how many slots exist.
    /// Use these to populate list views, then call
    /// [`fetch_single_item_template`](Self::fetch_single_item_template)
    /// on demand for the selected slot's full data.
    ///
    /// # Returns
    ///
    /// * `Ok(list)` — the server response including `total` and per-slot summaries.
    /// * `Err(message)` on HTTP or decode failure.
    pub fn fetch_item_template_summaries(&self) -> Result<TemplateListResponse, String> {
        self.fetch_template_summaries(TemplateKind::Item)
    }

    /// Fetch lightweight summaries for all character template slots.
    ///
    /// Issues **one** HTTP request regardless of how many slots exist.
    ///
    /// # Returns
    ///
    /// * `Ok(list)` — the server response including `total` and per-slot summaries.
    /// * `Err(message)` on HTTP or decode failure.
    pub fn fetch_character_template_summaries(&self) -> Result<TemplateListResponse, String> {
        self.fetch_template_summaries(TemplateKind::Character)
    }

    fn fetch_template_summaries(&self, kind: TemplateKind) -> Result<TemplateListResponse, String> {
        let path_root = match kind {
            TemplateKind::Item => "/admin/templates/items",
            TemplateKind::Character => "/admin/templates/characters",
        };
        // Request all slots in one shot (max allowed by the API is 4096).
        let url = self.url(path_root);
        let resp = self
            .client
            .get(&url)
            .query(&[("limit", kind.slot_count().to_string())])
            .header(AUTHORIZATION, format!("Bearer {}", self.token))
            .send()
            .map_err(|e| format!("GET {url}: {e}"))?;
        if !resp.status().is_success() {
            return Err(format!("GET {url}: HTTP {}", resp.status()));
        }
        resp.json::<TemplateListResponse>()
            .map_err(|e| format!("GET {url}: decode JSON: {e}"))
    }

    /// Fetch the full bincode payload for a single item template slot.
    ///
    /// # Arguments
    ///
    /// * `index` - Slot index in `[0, MAXTITEM)`.
    ///
    /// # Returns
    ///
    /// * `Ok(item)` on success.
    /// * `Err(message)` on HTTP or decode failure.
    pub fn fetch_single_item_template(&self, index: usize) -> Result<Item, String> {
        let url = self.url(&format!("/admin/templates/items/{index}"));
        let resp = self
            .client
            .get(&url)
            .header(AUTHORIZATION, format!("Bearer {}", self.token))
            .header(ACCEPT, OCTET_STREAM)
            .send()
            .map_err(|e| format!("GET {url}: {e}"))?;
        if !resp.status().is_success() {
            return Err(format!("GET {url}: HTTP {}", resp.status()));
        }
        let bytes = resp
            .bytes()
            .map_err(|e| format!("GET {url}: read body: {e}"))?;
        decode_item_template(&bytes).map_err(|e| format!("GET {url}: decode: {e}"))
    }

    /// Fetch the full bincode payload for a single character template slot.
    ///
    /// # Arguments
    ///
    /// * `index` - Slot index in `[0, MAXTCHARS)`.
    ///
    /// # Returns
    ///
    /// * `Ok(character)` on success.
    /// * `Err(message)` on HTTP or decode failure.
    pub fn fetch_single_character_template(&self, index: usize) -> Result<Character, String> {
        let url = self.url(&format!("/admin/templates/characters/{index}"));
        let resp = self
            .client
            .get(&url)
            .header(AUTHORIZATION, format!("Bearer {}", self.token))
            .header(ACCEPT, OCTET_STREAM)
            .send()
            .map_err(|e| format!("GET {url}: {e}"))?;
        if !resp.status().is_success() {
            return Err(format!("GET {url}: HTTP {}", resp.status()));
        }
        let bytes = resp
            .bytes()
            .map_err(|e| format!("GET {url}: read body: {e}"))?;
        decode_character_template(&bytes).map_err(|e| format!("GET {url}: decode: {e}"))
    }

    /// Fetch all item templates by performing per-slot GETs.
    ///
    /// # Returns
    ///
    /// * `Ok(items)` with `MAXTITEM` slots filled.
    /// * `Err(message)` on any request or decode failure.
    pub fn fetch_item_templates(&self) -> Result<Vec<Item>, String> {
        self.fetch_templates(TemplateKind::Item, decode_item_template)
    }

    /// Fetch all character templates by performing per-slot GETs.
    ///
    /// # Returns
    ///
    /// * `Ok(characters)` with `MAXTCHARS` slots filled.
    /// * `Err(message)` on any request or decode failure.
    pub fn fetch_character_templates(&self) -> Result<Vec<Character>, String> {
        self.fetch_templates(TemplateKind::Character, decode_character_template)
    }

    fn fetch_templates<T, F>(&self, kind: TemplateKind, decode: F) -> Result<Vec<T>, String>
    where
        F: Fn(&[u8]) -> Result<T, template_store::TemplateError>,
    {
        let count = kind.slot_count();
        let mut out = Vec::with_capacity(count);
        let path_root = match kind {
            TemplateKind::Item => "/admin/templates/items",
            TemplateKind::Character => "/admin/templates/characters",
        };
        for idx in 0..count {
            let url = self.url(&format!("{path_root}/{idx}"));
            let resp = self
                .client
                .get(&url)
                .header(AUTHORIZATION, format!("Bearer {}", self.token))
                .header(ACCEPT, OCTET_STREAM)
                .send()
                .map_err(|e| format!("GET {url}: {e}"))?;
            if !resp.status().is_success() {
                return Err(format!("GET {url}: HTTP {}", resp.status()));
            }
            let bytes = resp
                .bytes()
                .map_err(|e| format!("GET {url}: read body: {e}"))?;
            let value = decode(&bytes).map_err(|e| format!("GET {url}: decode: {e}"))?;
            out.push(value);
        }
        Ok(out)
    }

    /// Upload a single item template at slot `index`.
    ///
    /// # Arguments
    ///
    /// * `index` - Slot index in `[0, MAXTITEM)`.
    /// * `item` - Template payload to encode and PUT.
    ///
    /// # Returns
    ///
    /// * `Ok(())` on success.
    /// * `Err(message)` on encode or HTTP failure.
    pub fn put_item_template(&self, index: usize, item: &Item) -> Result<(), String> {
        let bytes = encode_item_template(item).map_err(|e| format!("encode: {e}"))?;
        self.put_template_bytes(TemplateKind::Item, index, bytes)
    }

    /// Upload a single character template at slot `index`.
    ///
    /// # Arguments
    ///
    /// * `index` - Slot index in `[0, MAXTCHARS)`.
    /// * `character` - Template payload to encode and PUT.
    ///
    /// # Returns
    ///
    /// * `Ok(())` on success.
    /// * `Err(message)` on encode or HTTP failure.
    pub fn put_character_template(
        &self,
        index: usize,
        character: &Character,
    ) -> Result<(), String> {
        let bytes = encode_character_template(character).map_err(|e| format!("encode: {e}"))?;
        self.put_template_bytes(TemplateKind::Character, index, bytes)
    }

    fn put_template_bytes(
        &self,
        kind: TemplateKind,
        index: usize,
        bytes: Vec<u8>,
    ) -> Result<(), String> {
        let path_root = match kind {
            TemplateKind::Item => "/admin/templates/items",
            TemplateKind::Character => "/admin/templates/characters",
        };
        let url = self.url(&format!("{path_root}/{index}"));
        let resp = self
            .client
            .put(&url)
            .header(AUTHORIZATION, format!("Bearer {}", self.token))
            .header(CONTENT_TYPE, OCTET_STREAM)
            .body(bytes)
            .send()
            .map_err(|e| format!("PUT {url}: {e}"))?;
        if !resp.status().is_success() {
            return Err(format!("PUT {url}: HTTP {}", resp.status()));
        }
        Ok(())
    }

    /// Trigger a templates reload on the running server.
    ///
    /// # Arguments
    ///
    /// * `reload_items` - Reload item templates.
    /// * `reload_characters` - Reload character templates.
    ///
    /// # Returns
    ///
    /// * `Ok(response)` containing the request id and pending status.
    /// * `Err(message)` on HTTP failure.
    pub fn request_reload(
        &self,
        reload_items: bool,
        reload_characters: bool,
    ) -> Result<ReloadResponse, String> {
        let url = self.url("/admin/templates/reload");
        let mut kinds: Vec<&str> = Vec::new();
        if reload_items {
            kinds.push("items");
        }
        if reload_characters {
            kinds.push("characters");
        }
        let body = serde_json::json!({ "kinds": kinds });
        let resp = self
            .client
            .post(&url)
            .header(AUTHORIZATION, format!("Bearer {}", self.token))
            .json(&body)
            .send()
            .map_err(|e| format!("POST {url}: {e}"))?;
        if !resp.status().is_success() {
            return Err(format!("POST {url}: HTTP {}", resp.status()));
        }
        resp.json::<ReloadResponse>()
            .map_err(|e| format!("POST {url}: decode: {e}"))
    }

    /// Poll the reload status endpoint for a previously enqueued request.
    ///
    /// # Arguments
    ///
    /// * `request_id` - Identifier returned by `request_reload`.
    ///
    /// # Returns
    ///
    /// * `Ok(status)` describing the current lifecycle state.
    /// * `Err(message)` on HTTP failure.
    pub fn reload_status(&self, request_id: &str) -> Result<ReloadStatusResponse, String> {
        let url = self.url("/admin/templates/reload/status");
        let resp = self
            .client
            .get(&url)
            .query(&[("request_id", request_id)])
            .header(AUTHORIZATION, format!("Bearer {}", self.token))
            .send()
            .map_err(|e| format!("GET {url}: {e}"))?;
        if !resp.status().is_success() {
            return Err(format!("GET {url}: HTTP {}", resp.status()));
        }
        resp.json::<ReloadStatusResponse>()
            .map_err(|e| format!("GET {url}: decode: {e}"))
    }

    // ------------------------------------------------------------------
    //  Map editing
    // ------------------------------------------------------------------

    /// Fetch every map tile in a single HTTP request.
    ///
    /// The API returns a bincode `Vec<Map>` in row-major order.
    ///
    /// # Returns
    ///
    /// * `Ok(tiles)` — length equals `SERVER_MAPX * SERVER_MAPY`.
    /// * `Err(message)` on HTTP or decode failure.
    pub fn fetch_map_tiles(&self) -> Result<Vec<Map>, String> {
        let url = self.url("/admin/world/map");
        let resp = self
            .client
            .get(&url)
            .header(AUTHORIZATION, format!("Bearer {}", self.token))
            .header(ACCEPT, OCTET_STREAM)
            .send()
            .map_err(|e| format!("GET {url}: {e}"))?;
        if !resp.status().is_success() {
            return Err(format!("GET {url}: HTTP {}", resp.status()));
        }
        let bytes = resp
            .bytes()
            .map_err(|e| format!("GET {url}: read body: {e}"))?;
        let (tiles, consumed): (Vec<Map>, usize) =
            bincode::decode_from_slice(&bytes, bincode::config::standard())
                .map_err(|e| format!("GET {url}: decode: {e}"))?;
        if consumed != bytes.len() {
            return Err(format!(
                "GET {url}: trailing bytes after Vec<Map> (consumed {} of {})",
                consumed,
                bytes.len()
            ));
        }
        Ok(tiles)
    }

    /// Fetch the raw bincode payload for a single map tile.
    ///
    /// # Arguments
    ///
    /// * `x` - Tile X coordinate.
    /// * `y` - Tile Y coordinate.
    ///
    /// # Returns
    ///
    /// * `Ok(tile)` on success.
    /// * `Err(message)` on HTTP or decode failure.
    pub fn fetch_single_map_tile(&self, x: usize, y: usize) -> Result<Map, String> {
        let url = self.url(&format!("/admin/world/map/{x}/{y}"));
        let resp = self
            .client
            .get(&url)
            .header(AUTHORIZATION, format!("Bearer {}", self.token))
            .header(ACCEPT, OCTET_STREAM)
            .send()
            .map_err(|e| format!("GET {url}: {e}"))?;
        if !resp.status().is_success() {
            return Err(format!("GET {url}: HTTP {}", resp.status()));
        }
        let bytes = resp
            .bytes()
            .map_err(|e| format!("GET {url}: read body: {e}"))?;
        Map::from_bytes(&bytes).ok_or_else(|| format!("GET {url}: decode Map failed"))
    }

    /// Enqueue a patch for a single map tile on the server.
    ///
    /// The server applies patches between ticks, preserving dynamic fields
    /// (`ch`, `to_ch`, `it`, `light`, `dlight`).
    ///
    /// # Arguments
    ///
    /// * `x`     - Tile X coordinate.
    /// * `y`     - Tile Y coordinate.
    /// * `patch` - Static-field overrides. `patch.x` / `patch.y` must match
    ///             the URL coordinates; the API rejects mismatches.
    ///
    /// # Returns
    ///
    /// * `Ok(response)` with new version counter and queue depth.
    /// * `Err(message)` on encode, HTTP, or server-side validation failure.
    pub fn put_map_tile_patch(
        &self,
        x: usize,
        y: usize,
        patch: &MapPatch,
    ) -> Result<PutMapTileResponse, String> {
        // Build the canonical byte form client-side so we get the same error
        // paths regardless of the HTTP layer.
        let bytes = patch.to_bytes().map_err(|e| format!("encode: {e}"))?;
        let url = self.url(&format!("/admin/world/map/{x}/{y}"));
        let resp = self
            .client
            .put(&url)
            .header(AUTHORIZATION, format!("Bearer {}", self.token))
            .header(CONTENT_TYPE, OCTET_STREAM)
            .body(bytes)
            .send()
            .map_err(|e| format!("PUT {url}: {e}"))?;
        if !resp.status().is_success() {
            return Err(format!("PUT {url}: HTTP {}", resp.status()));
        }
        resp.json::<PutMapTileResponse>()
            .map_err(|e| format!("PUT {url}: decode: {e}"))
    }

    /// Request a flush of the server-side map-patch queue.
    ///
    /// Returns the request id to poll via [`map_reload_status`].
    ///
    /// # Returns
    ///
    /// * `Ok(response)` on success.
    /// * `Err(message)` on HTTP failure.
    pub fn request_map_reload(&self) -> Result<MapReloadResponse, String> {
        let url = self.url("/admin/world/map/reload");
        let resp = self
            .client
            .post(&url)
            .header(AUTHORIZATION, format!("Bearer {}", self.token))
            .json(&serde_json::json!({}))
            .send()
            .map_err(|e| format!("POST {url}: {e}"))?;
        if !resp.status().is_success() {
            return Err(format!("POST {url}: HTTP {}", resp.status()));
        }
        resp.json::<MapReloadResponse>()
            .map_err(|e| format!("POST {url}: decode: {e}"))
    }

    /// Poll the map-reload status endpoint for a previously enqueued request.
    ///
    /// # Arguments
    ///
    /// * `request_id` - Identifier returned by [`request_map_reload`].
    ///
    /// # Returns
    ///
    /// * `Ok(status)` describing the current lifecycle state.
    /// * `Err(message)` on HTTP failure.
    pub fn map_reload_status(&self, request_id: &str) -> Result<MapReloadStatusResponse, String> {
        let url = self.url("/admin/world/map/reload/status");
        let resp = self
            .client
            .get(&url)
            .query(&[("request_id", request_id)])
            .header(AUTHORIZATION, format!("Bearer {}", self.token))
            .send()
            .map_err(|e| format!("GET {url}: {e}"))?;
        if !resp.status().is_success() {
            return Err(format!("GET {url}: HTTP {}", resp.status()));
        }
        resp.json::<MapReloadStatusResponse>()
            .map_err(|e| format!("GET {url}: decode: {e}"))
    }

    /// Fetch the current value of the admin map-version counter.
    ///
    /// # Returns
    ///
    /// * `Ok(version)` on success.
    /// * `Err(message)` on HTTP failure.
    pub fn fetch_map_version(&self) -> Result<u64, String> {
        let url = self.url("/admin/world/map/version");
        let resp = self
            .client
            .get(&url)
            .header(AUTHORIZATION, format!("Bearer {}", self.token))
            .send()
            .map_err(|e| format!("GET {url}: {e}"))?;
        if !resp.status().is_success() {
            return Err(format!("GET {url}: HTTP {}", resp.status()));
        }
        let body: MapVersionResponse =
            resp.json().map_err(|e| format!("GET {url}: decode: {e}"))?;
        Ok(body.version)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `AdminClient::new` strips trailing slashes from the base URL.
    #[test]
    fn base_url_trims_trailing_slash() {
        let client = AdminClient::new("https://example.test/", "x".repeat(32)).unwrap();
        assert_eq!(client.url("/admin/ping"), "https://example.test/admin/ping");
    }

    /// `AdminClient::new` accepts URLs without trailing slashes verbatim.
    #[test]
    fn base_url_preserved_when_no_trailing_slash() {
        let client = AdminClient::new("https://example.test", "x".repeat(32)).unwrap();
        assert_eq!(client.url("/admin/ping"), "https://example.test/admin/ping");
    }
}
