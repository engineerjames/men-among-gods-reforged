//! Shared KeyDB key schema and payloads for live world-admin actions.
//!
//! The admin API enqueues [`WorldActionRequest`] values into KeyDB. The
//! running server drains that queue between ticks and executes the requested
//! action on the tick thread, then writes a status entry under
//! [`world_action_status_key`].

use bincode::{Decode, Encode};
use serde::{Deserialize, Serialize};

/// KeyDB list key holding queued [`WorldActionRequest`] entries.
pub const WORLD_ACTION_QUEUE_KEY: &str = "game:admin:world_action_queue";

/// Pub/sub channel reserved for notifying the server about queued actions.
pub const WORLD_ACTION_PUBSUB_CHANNEL: &str = "game:admin:world_action";

/// TTL in seconds for request status keys.
pub const WORLD_ACTION_STATUS_TTL_SECS: u64 = 300;

/// Status string written when the API accepts an action request.
pub const STATUS_PENDING: &str = "pending";

/// Status string written when the server begins executing an action.
pub const STATUS_RUNNING: &str = "running";

/// Status string written after the server applies and persists an action.
pub const STATUS_APPLIED: &str = "applied";

/// Status string written when the server rejects or fails an action.
pub const STATUS_FAILED: &str = "failed";

/// Build the KeyDB key for a world-action status response.
///
/// # Arguments
///
/// * `request_id` - Identifier returned by the admin API.
///
/// # Returns
///
/// * The fully-formatted status key.
pub fn world_action_status_key(request_id: &str) -> String {
    format!("game:admin:world_action_status:{}", request_id)
}

/// Admin action that can be executed by the running game server.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Encode, Decode)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum WorldActionKind {
    /// Spawn any missing respawnable NPC templates.
    PopulateMissing,
    /// Wipe dynamic runtime world state.
    WipeRuntime,
    /// Recompute map lighting from current map and item state.
    RebuildLights,
    /// Synchronize player skill metadata from character templates.
    SyncPlayerSkills,
    /// Reset one character template and its live instances.
    ResetChar {
        /// Character template id to reset.
        template_id: usize,
    },
    /// Reset one item template and its live instances.
    ResetItem {
        /// Item template id to reset.
        template_id: usize,
    },
    /// Reset all character and item templates.
    ResetAll,
}

impl WorldActionKind {
    /// Return a short stable name for logging and CLI display.
    ///
    /// # Returns
    ///
    /// * Snake-case action name.
    pub fn name(&self) -> &'static str {
        match self {
            Self::PopulateMissing => "populate_missing",
            Self::WipeRuntime => "wipe_runtime",
            Self::RebuildLights => "rebuild_lights",
            Self::SyncPlayerSkills => "sync_player_skills",
            Self::ResetChar { .. } => "reset_char",
            Self::ResetItem { .. } => "reset_item",
            Self::ResetAll => "reset_all",
        }
    }
}

/// Request queued by the admin API for the server to execute.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Encode, Decode)]
pub struct WorldActionRequest {
    /// Opaque identifier returned to the admin caller.
    pub request_id: String,
    /// Action to execute on the running server.
    pub action: WorldActionKind,
    /// Unix timestamp when the API accepted the request.
    pub requested_at: u64,
}

impl WorldActionRequest {
    /// Encode this request to the canonical bincode byte representation.
    ///
    /// # Returns
    ///
    /// * `Ok(bytes)` on success.
    /// * `Err(WorldActionStoreError::Encode)` on bincode failure.
    pub fn to_bytes(&self) -> Result<Vec<u8>, WorldActionStoreError> {
        bincode::encode_to_vec(self, bincode::config::standard())
            .map_err(|error| WorldActionStoreError::Encode(error.to_string()))
    }

    /// Decode a request from canonical bincode bytes.
    ///
    /// # Arguments
    ///
    /// * `bytes` - Encoded request payload.
    ///
    /// # Returns
    ///
    /// * `Ok(request)` on success.
    /// * `Err(WorldActionStoreError::Decode)` when decoding fails or trailing
    ///   bytes remain.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, WorldActionStoreError> {
        let (value, consumed): (Self, usize) =
            bincode::decode_from_slice(bytes, bincode::config::standard())
                .map_err(|error| WorldActionStoreError::Decode(error.to_string()))?;
        if consumed != bytes.len() {
            return Err(WorldActionStoreError::Decode(format!(
                "trailing bytes after WorldActionRequest (consumed {}, total {})",
                consumed,
                bytes.len()
            )));
        }
        Ok(value)
    }
}

/// Response returned when the API accepts a world action.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorldActionResponse {
    /// Opaque identifier the caller polls through the status endpoint.
    pub request_id: String,
    /// Stable action name.
    pub action: String,
}

/// Status returned by the world-action status endpoint.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorldActionStatusResponse {
    /// Opaque identifier the caller passed in.
    pub request_id: String,
    /// Stable action name when known.
    #[serde(default)]
    pub action: String,
    /// Lifecycle state: `pending`, `running`, `applied`, or `failed`.
    pub status: String,
    /// Optional human-readable detail written by the server.
    #[serde(default)]
    pub message: String,
    /// Unix timestamp when this status was written.
    #[serde(default)]
    pub updated_at: u64,
}

/// Error returned by world-action store helpers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorldActionStoreError {
    /// Encoding the request to bytes failed.
    Encode(String),
    /// Decoding bytes into a request failed.
    Decode(String),
}

impl std::fmt::Display for WorldActionStoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Encode(message) => write!(f, "world action encode failed: {}", message),
            Self::Decode(message) => write!(f, "world action decode failed: {}", message),
        }
    }
}

impl std::error::Error for WorldActionStoreError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn world_action_status_key_includes_id() {
        assert_eq!(
            world_action_status_key("abc"),
            "game:admin:world_action_status:abc"
        );
    }

    #[test]
    fn world_action_kind_names_are_stable() {
        assert_eq!(WorldActionKind::PopulateMissing.name(), "populate_missing");
        assert_eq!(WorldActionKind::WipeRuntime.name(), "wipe_runtime");
        assert_eq!(WorldActionKind::RebuildLights.name(), "rebuild_lights");
        assert_eq!(
            WorldActionKind::SyncPlayerSkills.name(),
            "sync_player_skills"
        );
        assert_eq!(
            WorldActionKind::ResetChar { template_id: 42 }.name(),
            "reset_char"
        );
        assert_eq!(
            WorldActionKind::ResetItem { template_id: 12 }.name(),
            "reset_item"
        );
        assert_eq!(WorldActionKind::ResetAll.name(), "reset_all");
    }

    #[test]
    fn world_action_request_roundtrip() {
        let request = WorldActionRequest {
            request_id: "req123".to_string(),
            action: WorldActionKind::ResetChar { template_id: 17 },
            requested_at: 12345,
        };
        let bytes = request.to_bytes().expect("encode");
        let decoded = WorldActionRequest::from_bytes(&bytes).expect("decode");
        assert_eq!(decoded, request);
    }

    #[test]
    fn world_action_request_decode_rejects_trailing_bytes() {
        let request = WorldActionRequest {
            request_id: "req123".to_string(),
            action: WorldActionKind::RebuildLights,
            requested_at: 12345,
        };
        let mut bytes = request.to_bytes().expect("encode");
        bytes.push(0);
        assert!(WorldActionRequest::from_bytes(&bytes).is_err());
    }
}
