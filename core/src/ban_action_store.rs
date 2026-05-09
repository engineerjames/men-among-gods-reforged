//! Shared KeyDB queue contract for live ban enforcement actions.
//!
//! Admin API mutations persist ban records first, then enqueue a live action
//! for the running game server so matching online sessions can be disconnected
//! without waiting for the next login attempt.

use crate::ban_store::BanTarget;
use bincode::{Decode, Encode};
use serde::{Deserialize, Serialize};

/// KeyDB list key holding queued live ban actions.
pub const BAN_ACTION_QUEUE_KEY: &str = "game:admin:ban_action_queue";

/// Pub/sub channel reserved for future live ban-action notifications.
pub const BAN_ACTION_PUBSUB_CHANNEL: &str = "game:admin:ban_action";

/// TTL in seconds for ban-action status keys.
pub const BAN_ACTION_STATUS_TTL_SECS: u64 = 300;

/// Status string written when the API accepts a live action request.
pub const STATUS_PENDING: &str = "pending";

/// Status string written when the server starts applying a live action.
pub const STATUS_RUNNING: &str = "running";

/// Status string written when the server applies a live action.
pub const STATUS_APPLIED: &str = "applied";

/// Status string written when the server fails a live action.
pub const STATUS_FAILED: &str = "failed";

/// Build the KeyDB status key for a live ban action.
///
/// # Arguments
///
/// * `request_id` - Identifier returned by the admin API.
///
/// # Returns
///
/// * Fully formatted KeyDB key.
pub fn ban_action_status_key(request_id: &str) -> String {
    format!("game:admin:ban_action_status:{}", request_id)
}

/// Live action for the running game server after a ban mutation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Encode, Decode)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum BanActionKind {
    /// Kick online sessions matching an active ban target.
    ApplyBan {
        /// Target that was banned.
        target: BanTarget,
        /// Whether matching online sessions should be disconnected.
        kick_online: bool,
    },
    /// Remove any in-memory state for a target after an unban.
    RemoveBan {
        /// Target that was unbanned.
        target: BanTarget,
    },
    /// Reload all active ban records from KeyDB.
    ReloadBans,
}

impl BanActionKind {
    /// Return a stable action name for logging and status responses.
    ///
    /// # Returns
    ///
    /// * Snake-case action name.
    pub fn name(&self) -> &'static str {
        match self {
            Self::ApplyBan { .. } => "apply_ban",
            Self::RemoveBan { .. } => "remove_ban",
            Self::ReloadBans => "reload_bans",
        }
    }
}

/// Request queued by the admin API for live ban enforcement.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Encode, Decode)]
pub struct BanActionRequest {
    /// Opaque identifier returned to the admin caller.
    pub request_id: String,
    /// Live action to execute on the server tick thread.
    pub action: BanActionKind,
    /// Unix timestamp when the API accepted the request.
    pub requested_at: u64,
}

impl BanActionRequest {
    /// Encode this request to canonical bincode bytes.
    ///
    /// # Returns
    ///
    /// * `Ok(bytes)` on success.
    /// * `Err(BanActionStoreError::Encode)` on bincode failure.
    pub fn to_bytes(&self) -> Result<Vec<u8>, BanActionStoreError> {
        bincode::encode_to_vec(self, bincode::config::standard())
            .map_err(|error| BanActionStoreError::Encode(error.to_string()))
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
    /// * `Err(BanActionStoreError::Decode)` on decode failure.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, BanActionStoreError> {
        let (value, consumed): (Self, usize) =
            bincode::decode_from_slice(bytes, bincode::config::standard())
                .map_err(|error| BanActionStoreError::Decode(error.to_string()))?;
        if consumed != bytes.len() {
            return Err(BanActionStoreError::Decode(format!(
                "trailing bytes after BanActionRequest (consumed {}, total {})",
                consumed,
                bytes.len()
            )));
        }
        Ok(value)
    }
}

/// Response returned when the API queues a live ban action.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BanActionResponse {
    /// Opaque identifier the caller polls through the status endpoint.
    pub request_id: String,
    /// Stable action name.
    pub action: String,
}

/// Status returned by the live ban-action status endpoint.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BanActionStatusResponse {
    /// Opaque identifier the caller passed in.
    pub request_id: String,
    /// Stable action name when known.
    #[serde(default)]
    pub action: String,
    /// Lifecycle state.
    pub status: String,
    /// Optional human-readable detail written by the server.
    #[serde(default)]
    pub message: String,
    /// Unix timestamp when this status was written.
    #[serde(default)]
    pub updated_at: u64,
}

/// Error returned by live ban-action helpers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BanActionStoreError {
    /// Encoding failed.
    Encode(String),
    /// Decoding failed.
    Decode(String),
}

impl std::fmt::Display for BanActionStoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Encode(message) => write!(f, "ban action encode failed: {}", message),
            Self::Decode(message) => write!(f, "ban action decode failed: {}", message),
        }
    }
}

impl std::error::Error for BanActionStoreError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_key_formats_request_id() {
        assert_eq!(
            ban_action_status_key("abc"),
            "game:admin:ban_action_status:abc"
        );
    }

    #[test]
    fn encode_decode_request_roundtrip() {
        let request = BanActionRequest {
            request_id: "ban-action-1".to_owned(),
            action: BanActionKind::ApplyBan {
                target: BanTarget::Account { account_id: 5 },
                kick_online: true,
            },
            requested_at: 123,
        };

        let bytes = request.to_bytes().unwrap();
        assert_eq!(BanActionRequest::from_bytes(&bytes).unwrap(), request);
    }
}
