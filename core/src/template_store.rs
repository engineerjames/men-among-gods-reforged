//! Shared KeyDB key schema and helpers for item and character templates.
//!
//! This module is consumed by the running server (`server` crate), the API
//! service (`api` crate), and viewer utilities (`server-utils` crate) so that
//! all callers agree on the on-disk key layout, slot counts, and basic
//! validation rules without depending on each other.
//!
//! The actual bincode encoders for `Item` and `Character` live on those types
//! (`Item::to_bytes` / `Item::from_bytes`, `Character::to_bytes` /
//! `Character::from_bytes`) and are reused here.

use crate::constants::{MAXTCHARS, MAXTITEM};
use crate::types::{Character, Item};

// ---------------------------------------------------------------------------
//  Key prefixes
// ---------------------------------------------------------------------------

/// KeyDB key prefix for item templates: `game:titem:{idx}`.
pub const ITEM_TEMPLATE_KEY_PREFIX: &str = "game:titem:";

/// KeyDB key prefix for character templates: `game:tchar:{idx}`.
pub const CHARACTER_TEMPLATE_KEY_PREFIX: &str = "game:tchar:";

/// KeyDB key holding the schema version integer.
pub const META_VERSION_KEY: &str = "game:meta:version";

/// KeyDB counter incremented after any item-template write.
pub const ITEM_TEMPLATE_VERSION_KEY: &str = "game:meta:templates:item_version";

/// KeyDB counter incremented after any character-template write.
pub const CHARACTER_TEMPLATE_VERSION_KEY: &str = "game:meta:templates:character_version";

/// KeyDB key the API writes a JSON reload payload into.
///
/// Carries `{ kinds, requested_at, request_id }`. The server's reload watcher
/// polls this key, drains it, and writes a status entry under
/// [`reload_status_key`].
pub const RELOAD_REQUEST_KEY: &str = "game:templates:reload_request";

/// Pub/sub channel name reserved for future automatic reload signalling.
///
/// The API publishes here in addition to writing [`RELOAD_REQUEST_KEY`]; the
/// server does not yet subscribe to this channel.
pub const RELOAD_PUBSUB_CHANNEL: &str = "game:templates:reload";

/// Maximum number of item-template slots.
///
/// Re-exported from [`crate::constants::MAXTITEM`] so callers do not have to
/// reach into both modules.
pub const ITEM_TEMPLATE_SLOT_COUNT: usize = MAXTITEM;

/// Maximum number of character-template slots.
///
/// Re-exported from [`crate::constants::MAXTCHARS`] so callers do not have to
/// reach into both modules.
pub const CHARACTER_TEMPLATE_SLOT_COUNT: usize = MAXTCHARS;

// ---------------------------------------------------------------------------
//  Errors
// ---------------------------------------------------------------------------

/// Error returned by template-store helpers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TemplateError {
    /// The supplied slot index is outside the valid range for its kind.
    OutOfRange {
        /// Which template kind was being addressed.
        kind: TemplateKind,
        /// The offending index.
        index: usize,
        /// The allowed slot count (exclusive upper bound).
        slot_count: usize,
    },
    /// Encoding the template to bytes failed.
    Encode(String),
    /// Decoding bytes into a template failed.
    Decode(String),
}

impl std::fmt::Display for TemplateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::OutOfRange {
                kind,
                index,
                slot_count,
            } => write!(
                f,
                "{} template index {} out of range (max {})",
                kind.label(),
                index,
                slot_count.saturating_sub(1)
            ),
            Self::Encode(msg) => write!(f, "template encode failed: {}", msg),
            Self::Decode(msg) => write!(f, "template decode failed: {}", msg),
        }
    }
}

impl std::error::Error for TemplateError {}

/// The two template kinds managed through this module.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TemplateKind {
    /// Item templates stored under [`ITEM_TEMPLATE_KEY_PREFIX`].
    Item,
    /// Character templates stored under [`CHARACTER_TEMPLATE_KEY_PREFIX`].
    Character,
}

impl TemplateKind {
    /// Return a short human-readable label suitable for log/error messages.
    ///
    /// # Returns
    ///
    /// * `"item"` or `"character"`.
    pub fn label(self) -> &'static str {
        match self {
            Self::Item => "item",
            Self::Character => "character",
        }
    }

    /// Return the key prefix used for this template kind.
    ///
    /// # Returns
    ///
    /// * The KeyDB key prefix string (with trailing colon).
    pub fn key_prefix(self) -> &'static str {
        match self {
            Self::Item => ITEM_TEMPLATE_KEY_PREFIX,
            Self::Character => CHARACTER_TEMPLATE_KEY_PREFIX,
        }
    }

    /// Return the version-counter key bumped after writes for this kind.
    ///
    /// # Returns
    ///
    /// * The KeyDB key name as a `&'static str`.
    pub fn version_key(self) -> &'static str {
        match self {
            Self::Item => ITEM_TEMPLATE_VERSION_KEY,
            Self::Character => CHARACTER_TEMPLATE_VERSION_KEY,
        }
    }

    /// Return the slot-count upper bound for this kind.
    ///
    /// # Returns
    ///
    /// * The number of valid slot indices (`0..slot_count`).
    pub fn slot_count(self) -> usize {
        match self {
            Self::Item => ITEM_TEMPLATE_SLOT_COUNT,
            Self::Character => CHARACTER_TEMPLATE_SLOT_COUNT,
        }
    }
}

// ---------------------------------------------------------------------------
//  Key helpers
// ---------------------------------------------------------------------------

/// Build the KeyDB key for an item-template slot.
///
/// # Arguments
///
/// * `index` - Zero-based slot index.
///
/// # Returns
///
/// * The fully-formatted key (e.g. `"game:titem:42"`).
pub fn item_template_key(index: usize) -> String {
    format!("{}{}", ITEM_TEMPLATE_KEY_PREFIX, index)
}

/// Build the KeyDB key for a character-template slot.
///
/// # Arguments
///
/// * `index` - Zero-based slot index.
///
/// # Returns
///
/// * The fully-formatted key (e.g. `"game:tchar:42"`).
pub fn character_template_key(index: usize) -> String {
    format!("{}{}", CHARACTER_TEMPLATE_KEY_PREFIX, index)
}

/// Build the KeyDB key for a reload-status response.
///
/// # Arguments
///
/// * `request_id` - The request identifier returned by the API.
///
/// # Returns
///
/// * The fully-formatted key (e.g. `"game:templates:reload_status:abc"`).
pub fn reload_status_key(request_id: &str) -> String {
    format!("game:templates:reload_status:{}", request_id)
}

/// Validate that an index falls within the slot range for the given kind.
///
/// # Arguments
///
/// * `kind`  - Which template kind is being addressed.
/// * `index` - Slot index to validate.
///
/// # Returns
///
/// * `Ok(())` when in range.
/// * `Err(TemplateError::OutOfRange { .. })` otherwise.
pub fn validate_index(kind: TemplateKind, index: usize) -> Result<(), TemplateError> {
    let slot_count = kind.slot_count();
    if index < slot_count {
        Ok(())
    } else {
        Err(TemplateError::OutOfRange {
            kind,
            index,
            slot_count,
        })
    }
}

// ---------------------------------------------------------------------------
//  Encode / decode wrappers
// ---------------------------------------------------------------------------

/// Encode an item template to its on-disk byte representation.
///
/// # Arguments
///
/// * `item` - Template to encode.
///
/// # Returns
///
/// * `Ok(Vec<u8>)` on success.
/// * `Err(TemplateError::Encode)` on bincode failure (currently infallible
///   for `Item`, but wrapped for symmetry with future panicking changes).
pub fn encode_item_template(item: &Item) -> Result<Vec<u8>, TemplateError> {
    Ok(item.to_bytes())
}

/// Encode a character template to its on-disk byte representation.
///
/// # Arguments
///
/// * `character` - Template to encode.
///
/// # Returns
///
/// * `Ok(Vec<u8>)` on success.
/// * `Err(TemplateError::Encode)` on bincode failure.
pub fn encode_character_template(character: &Character) -> Result<Vec<u8>, TemplateError> {
    Ok(character.to_bytes())
}

/// Decode an item template from raw KeyDB bytes.
///
/// # Arguments
///
/// * `bytes` - Encoded payload.
///
/// # Returns
///
/// * `Ok(Item)` on success.
/// * `Err(TemplateError::Decode)` when bincode decoding fails.
pub fn decode_item_template(bytes: &[u8]) -> Result<Item, TemplateError> {
    Item::from_bytes(bytes).ok_or_else(|| TemplateError::Decode("invalid item bytes".to_owned()))
}

/// Decode a character template from raw KeyDB bytes.
///
/// # Arguments
///
/// * `bytes` - Encoded payload.
///
/// # Returns
///
/// * `Ok(Character)` on success.
/// * `Err(TemplateError::Decode)` when bincode decoding fails.
pub fn decode_character_template(bytes: &[u8]) -> Result<Character, TemplateError> {
    Character::from_bytes(bytes)
        .ok_or_else(|| TemplateError::Decode("invalid character bytes".to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn item_template_key_format() {
        assert_eq!(item_template_key(0), "game:titem:0");
        assert_eq!(item_template_key(42), "game:titem:42");
    }

    #[test]
    fn character_template_key_format() {
        assert_eq!(character_template_key(0), "game:tchar:0");
        assert_eq!(character_template_key(99), "game:tchar:99");
    }

    #[test]
    fn reload_status_key_format() {
        assert_eq!(
            reload_status_key("req-1"),
            "game:templates:reload_status:req-1"
        );
    }

    #[test]
    fn validate_index_accepts_first_and_last() {
        validate_index(TemplateKind::Item, 0).expect("0 valid");
        validate_index(TemplateKind::Item, ITEM_TEMPLATE_SLOT_COUNT - 1).expect("last valid");
        validate_index(TemplateKind::Character, 0).expect("0 valid");
        validate_index(TemplateKind::Character, CHARACTER_TEMPLATE_SLOT_COUNT - 1)
            .expect("last valid");
    }

    #[test]
    fn validate_index_rejects_out_of_range() {
        let err = validate_index(TemplateKind::Item, ITEM_TEMPLATE_SLOT_COUNT).unwrap_err();
        match err {
            TemplateError::OutOfRange {
                kind,
                index,
                slot_count,
            } => {
                assert_eq!(kind, TemplateKind::Item);
                assert_eq!(index, ITEM_TEMPLATE_SLOT_COUNT);
                assert_eq!(slot_count, ITEM_TEMPLATE_SLOT_COUNT);
            }
            other => panic!("unexpected error: {:?}", other),
        }
    }

    #[test]
    fn encode_decode_roundtrip_item() {
        let original = Item::default();
        let bytes = encode_item_template(&original).expect("encode");
        let decoded = decode_item_template(&bytes).expect("decode");
        assert_eq!(original, decoded);
    }

    #[test]
    fn encode_decode_roundtrip_character() {
        let original = Character::default();
        let bytes = encode_character_template(&original).expect("encode");
        let decoded = decode_character_template(&bytes).expect("decode");
        assert_eq!(original, decoded);
    }

    #[test]
    fn template_kind_metadata_consistent() {
        assert_eq!(TemplateKind::Item.key_prefix(), ITEM_TEMPLATE_KEY_PREFIX);
        assert_eq!(
            TemplateKind::Character.key_prefix(),
            CHARACTER_TEMPLATE_KEY_PREFIX
        );
        assert_eq!(TemplateKind::Item.version_key(), ITEM_TEMPLATE_VERSION_KEY);
        assert_eq!(
            TemplateKind::Character.version_key(),
            CHARACTER_TEMPLATE_VERSION_KEY
        );
        assert_eq!(TemplateKind::Item.slot_count(), ITEM_TEMPLATE_SLOT_COUNT);
        assert_eq!(
            TemplateKind::Character.slot_count(),
            CHARACTER_TEMPLATE_SLOT_COUNT
        );
        assert_eq!(TemplateKind::Item.label(), "item");
        assert_eq!(TemplateKind::Character.label(), "character");
    }

    #[test]
    fn template_error_display_contains_index() {
        let err = TemplateError::OutOfRange {
            kind: TemplateKind::Item,
            index: 9999,
            slot_count: ITEM_TEMPLATE_SLOT_COUNT,
        };
        let msg = format!("{}", err);
        assert!(msg.contains("9999"));
        assert!(msg.contains("item"));
    }
}
