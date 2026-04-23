//! Shared KeyDB key schema and helpers for live map-tile editing.
//!
//! Mirrors [`crate::template_store`] for the `Map` slice. The `api` crate
//! exposes admin endpoints that read/write individual tiles and enqueue
//! [`MapPatch`] entries; the running server drains the queue between ticks
//! and merges the static fields into its in-memory map while preserving
//! dynamic fields (`ch`, `to_ch`, `it`, `light`, `dlight`).

use crate::constants::{SERVER_MAPX, SERVER_MAPY};
use bincode::{Decode, Encode};

// ---------------------------------------------------------------------------
//  Key schema
// ---------------------------------------------------------------------------

/// KeyDB key prefix for individual map tiles: `game:map:{x}:{y}`.
pub const MAP_KEY_PREFIX: &str = "game:map:";

/// KeyDB list key holding queued [`MapPatch`] entries (RPUSH/LPOP).
pub const MAP_PATCH_QUEUE_KEY: &str = "game:map:patch_queue";

/// KeyDB key the API writes a JSON reload payload into to flush the queue.
///
/// Carries `{ request_id, requested_at }`. The server's map-patch watcher
/// drains it via `GETDEL`, applies all queued patches synchronously on the
/// tick thread, and writes a status entry under [`map_patch_status_key`].
pub const MAP_PATCH_REQUEST_KEY: &str = "game:map:patch_request";

/// Pub/sub channel name reserved for future automatic patch signalling.
pub const MAP_PATCH_PUBSUB_CHANNEL: &str = "game:map:patch";

/// KeyDB counter incremented after every map-tile write through the admin API.
pub const MAP_VERSION_KEY: &str = "game:meta:map:version";

/// Build the KeyDB key for a single map tile.
///
/// # Arguments
///
/// * `x` - Tile X coordinate.
/// * `y` - Tile Y coordinate.
///
/// # Returns
///
/// * The fully-formatted key (e.g. `"game:map:12:34"`).
pub fn map_key(x: usize, y: usize) -> String {
    format!("{}{}:{}", MAP_KEY_PREFIX, x, y)
}

/// Build the KeyDB key for a map-patch reload-status response.
///
/// # Arguments
///
/// * `request_id` - Identifier returned by the admin API.
///
/// # Returns
///
/// * The fully-formatted status key.
pub fn map_patch_status_key(request_id: &str) -> String {
    format!("game:map:patch_status:{}", request_id)
}

// ---------------------------------------------------------------------------
//  Errors
// ---------------------------------------------------------------------------

/// Error returned by map-store helpers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MapStoreError {
    /// Either `x` or `y` falls outside the world bounds.
    OutOfRange {
        /// Offending X coordinate.
        x: usize,
        /// Offending Y coordinate.
        y: usize,
        /// Allowed exclusive upper bound on X (`SERVER_MAPX`).
        max_x: usize,
        /// Allowed exclusive upper bound on Y (`SERVER_MAPY`).
        max_y: usize,
    },
    /// Encoding the patch or tile to bytes failed.
    Encode(String),
    /// Decoding bytes into a patch or tile failed.
    Decode(String),
}

impl std::fmt::Display for MapStoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::OutOfRange { x, y, max_x, max_y } => write!(
                f,
                "map coordinates ({}, {}) out of range (max {}x{})",
                x, y, max_x, max_y
            ),
            Self::Encode(msg) => write!(f, "map encode failed: {}", msg),
            Self::Decode(msg) => write!(f, "map decode failed: {}", msg),
        }
    }
}

impl std::error::Error for MapStoreError {}

/// Validate that `(x, y)` falls within the world bounds.
///
/// # Arguments
///
/// * `x` - Tile X coordinate.
/// * `y` - Tile Y coordinate.
///
/// # Returns
///
/// * `Ok(())` when inside `[0, SERVER_MAPX) x [0, SERVER_MAPY)`.
/// * `Err(MapStoreError::OutOfRange { .. })` otherwise.
pub fn validate_map_coords(x: usize, y: usize) -> Result<(), MapStoreError> {
    let max_x = SERVER_MAPX as usize;
    let max_y = SERVER_MAPY as usize;
    if x < max_x && y < max_y {
        Ok(())
    } else {
        Err(MapStoreError::OutOfRange { x, y, max_x, max_y })
    }
}

// ---------------------------------------------------------------------------
//  MapPatch
// ---------------------------------------------------------------------------

/// Static-field overrides for a single map tile.
///
/// The admin API accepts these as the `PUT /admin/world/map/{x}/{y}` body.
/// Only fields managed by world-builders are present; dynamic runtime fields
/// (`ch`, `to_ch`, `it`, `light`, `dlight`) are owned by the tick loop and
/// preserved by the server when the patch is applied.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode, Decode)]
pub struct MapPatch {
    /// Tile X coordinate.
    pub x: u32,
    /// Tile Y coordinate.
    pub y: u32,
    /// Background sprite id.
    pub sprite: u16,
    /// Foreground sprite id.
    pub fsprite: u16,
    /// Tile flags bitset.
    pub flags: u64,
}

impl MapPatch {
    /// Encode this patch to the canonical bincode byte representation.
    ///
    /// # Returns
    ///
    /// * `Ok(bytes)` on success.
    /// * `Err(MapStoreError::Encode)` on bincode failure.
    pub fn to_bytes(&self) -> Result<Vec<u8>, MapStoreError> {
        bincode::encode_to_vec(self, bincode::config::standard())
            .map_err(|e| MapStoreError::Encode(e.to_string()))
    }

    /// Decode a patch from canonical bincode bytes.
    ///
    /// # Arguments
    ///
    /// * `bytes` - Encoded patch payload.
    ///
    /// # Returns
    ///
    /// * `Ok(patch)` on success.
    /// * `Err(MapStoreError::Decode)` when bincode decoding fails or trailing
    ///   bytes remain.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, MapStoreError> {
        let (value, consumed): (Self, usize) =
            bincode::decode_from_slice(bytes, bincode::config::standard())
                .map_err(|e| MapStoreError::Decode(e.to_string()))?;
        if consumed != bytes.len() {
            return Err(MapStoreError::Decode(format!(
                "trailing bytes after MapPatch (consumed {}, total {})",
                consumed,
                bytes.len()
            )));
        }
        Ok(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_key_formats_coords() {
        assert_eq!(map_key(0, 0), "game:map:0:0");
        assert_eq!(map_key(12, 34), "game:map:12:34");
    }

    #[test]
    fn map_patch_status_key_includes_id() {
        assert_eq!(map_patch_status_key("abc"), "game:map:patch_status:abc");
    }

    #[test]
    fn validate_coords_accepts_in_range() {
        assert!(validate_map_coords(0, 0).is_ok());
        assert!(
            validate_map_coords((SERVER_MAPX as usize) - 1, (SERVER_MAPY as usize) - 1).is_ok()
        );
    }

    #[test]
    fn validate_coords_rejects_out_of_range() {
        assert!(matches!(
            validate_map_coords(SERVER_MAPX as usize, 0),
            Err(MapStoreError::OutOfRange { .. })
        ));
        assert!(matches!(
            validate_map_coords(0, SERVER_MAPY as usize),
            Err(MapStoreError::OutOfRange { .. })
        ));
    }

    #[test]
    fn map_patch_roundtrip() {
        let patch = MapPatch {
            x: 10,
            y: 20,
            sprite: 42,
            fsprite: 99,
            flags: 0xDEADBEEFCAFEBABE,
        };
        let bytes = patch.to_bytes().expect("encode");
        let decoded = MapPatch::from_bytes(&bytes).expect("decode");
        assert_eq!(patch, decoded);
    }

    #[test]
    fn map_patch_decode_rejects_trailing_bytes() {
        let patch = MapPatch {
            x: 1,
            y: 2,
            sprite: 3,
            fsprite: 4,
            flags: 5,
        };
        let mut bytes = patch.to_bytes().expect("encode");
        bytes.push(0);
        assert!(MapPatch::from_bytes(&bytes).is_err());
    }
}
