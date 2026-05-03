//! Shared ban record model and KeyDB key helpers.
//!
//! Ban records are durable operator controls stored in KeyDB and consumed by
//! the API, game server, and admin tooling. Each active ban is keyed by its
//! canonical target so there is at most one active ban for an account,
//! character, or IPv4 address at a time.

use bincode::{Decode, Encode};
use serde::{Deserialize, Serialize};
use std::net::Ipv4Addr;

/// KeyDB set containing all active ban keys.
pub const BAN_ACTIVE_INDEX_KEY: &str = "game:ban:active:index";

/// KeyDB integer incremented after every ban mutation.
pub const BAN_VERSION_KEY: &str = "game:ban:version";

/// KeyDB lock key used to serialize active-ban mutations.
pub const BAN_MUTATION_LOCK_KEY: &str = "admin:bans:lock";

/// Maximum time in milliseconds for a stale ban mutation lock.
pub const BAN_MUTATION_LOCK_TTL_MS: u64 = 5_000;

/// Durable ban target.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Encode, Decode)]
#[serde(tag = "scope", rename_all = "snake_case")]
pub enum BanTarget {
    /// Ban an entire account by API account id.
    Account {
        /// API account id.
        account_id: u64,
    },
    /// Ban a single character by API character id.
    Character {
        /// API character id.
        character_id: u64,
    },
    /// Ban a single IPv4 address.
    Ipv4 {
        /// IPv4 address stored as a big-endian `u32`.
        address: u32,
    },
}

impl BanTarget {
    /// Return the stable scope name for this target.
    ///
    /// # Returns
    ///
    /// * Scope name used by API, CLI, and logs.
    pub fn scope(&self) -> &'static str {
        match self {
            Self::Account { .. } => "account",
            Self::Character { .. } => "character",
            Self::Ipv4 { .. } => "ipv4",
        }
    }

    /// Return a stable human-readable target value.
    ///
    /// # Returns
    ///
    /// * Account id, character id, or dotted IPv4 string.
    pub fn value(&self) -> String {
        match self {
            Self::Account { account_id } => account_id.to_string(),
            Self::Character { character_id } => character_id.to_string(),
            Self::Ipv4 { address } => ipv4_to_string(*address),
        }
    }

    /// Build the active KeyDB record key for this target.
    ///
    /// # Returns
    ///
    /// * Fully formatted KeyDB key.
    pub fn active_key(&self) -> String {
        match self {
            Self::Account { account_id } => ban_account_key(*account_id),
            Self::Character { character_id } => ban_character_key(*character_id),
            Self::Ipv4 { address } => ban_ipv4_key(*address),
        }
    }
}

/// Durable active ban record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Encode, Decode)]
pub struct BanRecord {
    /// Opaque ban identifier used in responses and logs.
    pub id: String,
    /// Target covered by this ban.
    pub target: BanTarget,
    /// Operator-supplied reason. Empty when not provided.
    pub reason: String,
    /// Operator or in-game issuer label.
    pub created_by: String,
    /// Unix timestamp when this ban was created or last replaced.
    pub created_at: u64,
    /// Unix timestamp when this ban expires. `None` means permanent.
    #[serde(default)]
    pub expires_at: Option<u64>,
    /// Origin of the mutation, e.g. `admin_api`, `admin_cli`, or `game_command`.
    pub source: String,
}

impl BanRecord {
    /// Encode this record to canonical bincode bytes.
    ///
    /// # Returns
    ///
    /// * `Ok(bytes)` on success.
    /// * `Err(BanStoreError::Encode)` when bincode encoding fails.
    pub fn to_bytes(&self) -> Result<Vec<u8>, BanStoreError> {
        bincode::encode_to_vec(self, bincode::config::standard())
            .map_err(|error| BanStoreError::Encode(error.to_string()))
    }

    /// Decode a record from canonical bincode bytes.
    ///
    /// # Arguments
    ///
    /// * `bytes` - Encoded record payload.
    ///
    /// # Returns
    ///
    /// * `Ok(record)` when decoding consumes the whole input.
    /// * `Err(BanStoreError::Decode)` when decoding fails.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, BanStoreError> {
        let (record, consumed): (Self, usize) =
            bincode::decode_from_slice(bytes, bincode::config::standard())
                .map_err(|error| BanStoreError::Decode(error.to_string()))?;
        if consumed != bytes.len() {
            return Err(BanStoreError::Decode(format!(
                "trailing bytes after BanRecord (consumed {}, total {})",
                consumed,
                bytes.len()
            )));
        }
        Ok(record)
    }

    /// Return whether this ban is active at `now_secs`.
    ///
    /// # Arguments
    ///
    /// * `now_secs` - Current Unix timestamp.
    ///
    /// # Returns
    ///
    /// * `true` when permanent or expiring in the future.
    pub fn is_active_at(&self, now_secs: u64) -> bool {
        self.expires_at
            .map(|expires_at| expires_at > now_secs)
            .unwrap_or(true)
    }
}

/// Error returned by ban store helpers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BanStoreError {
    /// Encoding failed.
    Encode(String),
    /// Decoding failed.
    Decode(String),
    /// IPv4 parsing failed.
    InvalidIpv4(String),
}

impl std::fmt::Display for BanStoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Encode(message) => write!(f, "ban encode failed: {}", message),
            Self::Decode(message) => write!(f, "ban decode failed: {}", message),
            Self::InvalidIpv4(message) => write!(f, "invalid IPv4 address: {}", message),
        }
    }
}

impl std::error::Error for BanStoreError {}

/// Build the active-ban key for an account.
///
/// # Arguments
///
/// * `account_id` - API account id.
///
/// # Returns
///
/// * Fully formatted KeyDB key.
pub fn ban_account_key(account_id: u64) -> String {
    format!("game:ban:active:account:{}", account_id)
}

/// Build the active-ban key for a character.
///
/// # Arguments
///
/// * `character_id` - API character id.
///
/// # Returns
///
/// * Fully formatted KeyDB key.
pub fn ban_character_key(character_id: u64) -> String {
    format!("game:ban:active:character:{}", character_id)
}

/// Build the active-ban key for an IPv4 address.
///
/// # Arguments
///
/// * `address` - IPv4 address encoded as a big-endian `u32`.
///
/// # Returns
///
/// * Fully formatted KeyDB key.
pub fn ban_ipv4_key(address: u32) -> String {
    format!("game:ban:active:ipv4:{}", address)
}

/// Parse a dotted IPv4 address into the server's canonical integer form.
///
/// # Arguments
///
/// * `value` - Dotted IPv4 string.
///
/// # Returns
///
/// * `Ok(address)` as a big-endian `u32`.
/// * `Err(BanStoreError::InvalidIpv4)` when parsing fails.
pub fn parse_ipv4(value: &str) -> Result<u32, BanStoreError> {
    value
        .trim()
        .parse::<Ipv4Addr>()
        .map(u32::from)
        .map_err(|_| BanStoreError::InvalidIpv4(value.trim().to_string()))
}

/// Convert a canonical IPv4 integer to dotted notation.
///
/// # Arguments
///
/// * `address` - IPv4 address encoded as a big-endian `u32`.
///
/// # Returns
///
/// * Dotted IPv4 string.
pub fn ipv4_to_string(address: u32) -> String {
    Ipv4Addr::from(address).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ipv4_roundtrip_uses_big_endian_integer() {
        let address = parse_ipv4("203.0.113.7").unwrap();
        assert_eq!(address, 0xcb00_7107);
        assert_eq!(ipv4_to_string(address), "203.0.113.7");
    }

    #[test]
    fn ipv6_is_rejected() {
        assert!(parse_ipv4("2001:db8::1").is_err());
    }

    #[test]
    fn target_keys_are_canonical() {
        assert_eq!(
            BanTarget::Account { account_id: 42 }.active_key(),
            "game:ban:active:account:42"
        );
        assert_eq!(
            BanTarget::Character { character_id: 77 }.active_key(),
            "game:ban:active:character:77"
        );
        assert_eq!(
            BanTarget::Ipv4 {
                address: 0xcb00_7107
            }
            .active_key(),
            "game:ban:active:ipv4:3405803783"
        );
    }

    #[test]
    fn record_expiration_controls_activity() {
        let mut record = BanRecord {
            id: "ban-1".to_string(),
            target: BanTarget::Account { account_id: 1 },
            reason: String::new(),
            created_by: "test".to_string(),
            created_at: 10,
            expires_at: None,
            source: "test".to_string(),
        };
        assert!(record.is_active_at(100));

        record.expires_at = Some(101);
        assert!(record.is_active_at(100));
        assert!(!record.is_active_at(101));
    }

    #[test]
    fn encode_decode_record_roundtrip() {
        let record = BanRecord {
            id: "ban-1".to_string(),
            target: BanTarget::Ipv4 {
                address: 0xcb00_7107,
            },
            reason: "test".to_string(),
            created_by: "admin".to_string(),
            created_at: 123,
            expires_at: Some(456),
            source: "admin_api".to_string(),
        };

        let bytes = record.to_bytes().unwrap();
        assert_eq!(BanRecord::from_bytes(&bytes).unwrap(), record);
    }
}
