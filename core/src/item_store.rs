//! Shared KeyDB key schema and helpers for live item editing.
//!
//! Mirrors [`crate::map_store`] for the runtime `Item` slice. The `api` crate
//! exposes admin endpoints that read individual item slots and enqueue
//! [`ItemPatch`] entries; the running server drains the queue between ticks
//! and merges only the static authoring fields into its in-memory item table
//! while preserving dynamic runtime fields.
//!
//! ## Static vs dynamic fields
//!
//! [`ItemPatch`] carries the **static authoring fields** that world builders
//! own — name, reference, description, flags, value, placement, template
//! origin, modifier tables, base sprite, driver routine and driver data,
//! and the merchant timestamps. The following dynamic runtime fields are
//! intentionally **not** part of the patch and are owned by the tick loop:
//!
//! * Position: `x`, `y`, `carried`
//! * Damage state: `damage_state`, `current_age`, `current_damage`
//! * Sprite override applied at runtime: `sprite_override`
//!
//! The watcher overwrites only the patch fields when applying, so the tick
//! thread keeps full ownership of placement, decay, and any per-instance
//! state changes.

use crate::constants::MAXITEM;
use bincode::{Decode, Encode};

// ---------------------------------------------------------------------------
//  Key schema
// ---------------------------------------------------------------------------

/// KeyDB key prefix for individual item slots: `game:item:{idx}`.
pub const ITEM_KEY_PREFIX: &str = "game:item:";

/// KeyDB list key holding queued [`ItemPatch`] entries (RPUSH/LPOP).
pub const ITEM_PATCH_QUEUE_KEY: &str = "game:item:patch_queue";

/// KeyDB key the API writes a JSON reload payload into to flush the queue.
///
/// Carries `{ request_id, requested_at }`. The server's item-patch watcher
/// drains it via `GETDEL`, applies all queued patches synchronously on the
/// tick thread, and writes a status entry under [`item_patch_status_key`].
pub const ITEM_PATCH_REQUEST_KEY: &str = "game:item:patch_request";

/// Pub/sub channel name reserved for future automatic patch signalling.
pub const ITEM_PATCH_PUBSUB_CHANNEL: &str = "game:item:patch";

/// KeyDB counter incremented after every item write through the admin API.
pub const ITEM_VERSION_KEY: &str = "game:meta:item:version";

/// Maximum number of item slots.
///
/// Re-exported from [`crate::constants::MAXITEM`] so callers do not have to
/// reach into both modules.
pub const ITEM_SLOT_COUNT: usize = MAXITEM;

/// Build the KeyDB key for a single item slot.
///
/// # Arguments
///
/// * `index` - Zero-based slot index.
///
/// # Returns
///
/// * The fully-formatted key (e.g. `"game:item:42"`).
pub fn item_key(index: usize) -> String {
    format!("{}{}", ITEM_KEY_PREFIX, index)
}

/// Build the KeyDB key for an item-patch reload-status response.
///
/// # Arguments
///
/// * `request_id` - Identifier returned by the admin API.
///
/// # Returns
///
/// * The fully-formatted status key.
pub fn item_patch_status_key(request_id: &str) -> String {
    format!("game:item:patch_status:{}", request_id)
}

// ---------------------------------------------------------------------------
//  Errors
// ---------------------------------------------------------------------------

/// Error returned by item-store helpers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ItemStoreError {
    /// The supplied slot index is outside the valid range.
    OutOfRange {
        /// The offending index.
        index: usize,
        /// The allowed slot count (exclusive upper bound).
        slot_count: usize,
    },
    /// The patch's embedded `id` did not match the slot it was applied to.
    Mismatch {
        /// The slot index addressed by the URL or call site.
        expected: usize,
        /// The `id` carried inside the patch payload.
        actual: usize,
    },
    /// Encoding the patch to bytes failed.
    Encode(String),
    /// Decoding bytes into a patch failed.
    Decode(String),
}

impl std::fmt::Display for ItemStoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::OutOfRange { index, slot_count } => write!(
                f,
                "item index {} out of range (max {})",
                index,
                slot_count.saturating_sub(1)
            ),
            Self::Mismatch { expected, actual } => write!(
                f,
                "item patch id {} does not match slot {}",
                actual, expected
            ),
            Self::Encode(msg) => write!(f, "item encode failed: {}", msg),
            Self::Decode(msg) => write!(f, "item decode failed: {}", msg),
        }
    }
}

impl std::error::Error for ItemStoreError {}

/// Validate that `index` falls within the item slot range.
///
/// # Arguments
///
/// * `index` - The slot index to validate.
///
/// # Returns
///
/// * `Ok(())` when `index < MAXITEM`.
/// * `Err(ItemStoreError::OutOfRange { .. })` otherwise.
pub fn validate_item_index(index: usize) -> Result<(), ItemStoreError> {
    if index < ITEM_SLOT_COUNT {
        Ok(())
    } else {
        Err(ItemStoreError::OutOfRange {
            index,
            slot_count: ITEM_SLOT_COUNT,
        })
    }
}

// ---------------------------------------------------------------------------
//  ItemPatch
// ---------------------------------------------------------------------------

/// Static authoring fields for a single item slot.
///
/// The admin API accepts these as the `PUT /admin/world/items/{id}` body.
/// Only fields managed by world-builders are present; dynamic runtime
/// fields (position, damage state, current age/damage, runtime sprite
/// override) are owned by the tick loop and preserved by the server when
/// the patch is applied.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode, Decode)]
pub struct ItemPatch {
    /// Slot index this patch targets. Must match the URL path component.
    pub id: u32,
    /// `used` flag (e.g. `USE_EMPTY`/`USE_ITEM`).
    pub used: u8,
    /// Item name.
    pub name: [u8; 40],
    /// Item reference string ("a pair of boots").
    pub reference: [u8; 40],
    /// Item description.
    pub description: [u8; 200],
    /// Bitset of item flags.
    pub flags: u64,
    /// Merchant value.
    pub value: u32,
    /// Placement slot bitfield.
    pub placement: u16,
    /// Template index this slot was created from.
    pub temp: u16,
    /// Maximum age in inactive/active state.
    pub max_age: [u32; 2],
    /// Maximum damage in inactive/active state.
    pub max_damage: u32,
    /// Per-attribute modifiers `[wear, active, min]`.
    pub attrib: [[i8; 3]; 5],
    /// HP modifiers `[wear, active, min]`.
    pub hp: [i16; 3],
    /// Endurance modifiers `[wear, active, min]`.
    pub end: [i16; 3],
    /// Mana modifiers `[wear, active, min]`.
    pub mana: [i16; 3],
    /// Per-skill modifiers `[wear, active, min]`.
    pub skill: [[i8; 3]; 50],
    /// Armor bonus (inactive/active).
    pub armor: [i8; 2],
    /// Weapon bonus (inactive/active).
    pub weapon: [i8; 2],
    /// Light radius (inactive/active).
    pub light: [i16; 2],
    /// Spell duration.
    pub duration: u32,
    /// Spell cost.
    pub cost: u32,
    /// Spell power.
    pub power: u32,
    /// Active state cooldown.
    pub active: u32,
    /// Base sprite (inactive/active).
    pub sprite: [i16; 2],
    /// Status flags (inactive/active).
    pub status: [u8; 2],
    /// Damage dealt to attackers when this item is hit.
    pub gethit_dam: [i8; 2],
    /// Minimum rank required to wear/use this item.
    pub min_rank: i8,
    /// Last time merchant bought this item.
    pub t_bought: i32,
    /// Last time merchant sold this item.
    pub t_sold: i32,
    /// `LOOKSPECIAL` / `USESPECIAL` driver routine selector.
    pub driver: u8,
    /// Driver-specific data.
    pub data: [u32; 10],
}

impl ItemPatch {
    /// Build an [`ItemPatch`] from a full [`crate::types::Item`] for slot
    /// `index`.
    ///
    /// Dynamic runtime fields on the source `Item` are dropped; only the
    /// authoring fields end up on the patch.
    ///
    /// # Arguments
    ///
    /// * `index` - Slot index this patch will be applied to.
    /// * `item`  - Source item.
    ///
    /// # Returns
    ///
    /// * A patch carrying `index` as `id` plus the static fields of `item`.
    pub fn from_item(index: usize, item: &crate::types::Item) -> Self {
        Self {
            id: index as u32,
            used: item.used,
            name: item.name,
            reference: item.reference,
            description: item.description,
            flags: item.flags,
            value: item.value,
            placement: item.placement,
            temp: item.temp,
            max_age: item.max_age,
            max_damage: item.max_damage,
            attrib: item.attrib,
            hp: item.hp,
            end: item.end,
            mana: item.mana,
            skill: item.skill,
            armor: item.armor,
            weapon: item.weapon,
            light: item.light,
            duration: item.duration,
            cost: item.cost,
            power: item.power,
            active: item.active,
            sprite: item.sprite,
            status: item.status,
            gethit_dam: item.gethit_dam,
            min_rank: item.min_rank,
            t_bought: item.t_bought,
            t_sold: item.t_sold,
            driver: item.driver,
            data: item.data,
        }
    }

    /// Apply this patch's static fields onto `target`, leaving dynamic
    /// runtime fields (position, damage state, current age/damage,
    /// sprite override) untouched.
    ///
    /// # Arguments
    ///
    /// * `target` - Mutable reference to the in-memory item slot.
    pub fn apply_to(&self, target: &mut crate::types::Item) {
        target.used = self.used;
        target.name = self.name;
        target.reference = self.reference;
        target.description = self.description;
        target.flags = self.flags;
        target.value = self.value;
        target.placement = self.placement;
        target.temp = self.temp;
        target.max_age = self.max_age;
        target.max_damage = self.max_damage;
        target.attrib = self.attrib;
        target.hp = self.hp;
        target.end = self.end;
        target.mana = self.mana;
        target.skill = self.skill;
        target.armor = self.armor;
        target.weapon = self.weapon;
        target.light = self.light;
        target.duration = self.duration;
        target.cost = self.cost;
        target.power = self.power;
        target.active = self.active;
        target.sprite = self.sprite;
        target.status = self.status;
        target.gethit_dam = self.gethit_dam;
        target.min_rank = self.min_rank;
        target.t_bought = self.t_bought;
        target.t_sold = self.t_sold;
        target.driver = self.driver;
        target.data = self.data;
    }

    /// Encode this patch to the canonical bincode byte representation.
    ///
    /// # Returns
    ///
    /// * `Ok(bytes)` on success.
    /// * `Err(ItemStoreError::Encode)` on bincode failure.
    pub fn to_bytes(&self) -> Result<Vec<u8>, ItemStoreError> {
        bincode::encode_to_vec(self, bincode::config::standard())
            .map_err(|e| ItemStoreError::Encode(e.to_string()))
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
    /// * `Err(ItemStoreError::Decode)` when bincode decoding fails or
    ///   trailing bytes remain.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, ItemStoreError> {
        let (value, consumed): (Self, usize) =
            bincode::decode_from_slice(bytes, bincode::config::standard())
                .map_err(|e| ItemStoreError::Decode(e.to_string()))?;
        if consumed != bytes.len() {
            return Err(ItemStoreError::Decode(format!(
                "trailing bytes after ItemPatch (consumed {}, total {})",
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
    use crate::types::Item;

    #[test]
    fn item_key_formats_index() {
        assert_eq!(item_key(0), "game:item:0");
        assert_eq!(item_key(98_303), "game:item:98303");
    }

    #[test]
    fn item_patch_status_key_includes_id() {
        assert_eq!(item_patch_status_key("abc"), "game:item:patch_status:abc");
    }

    #[test]
    fn validate_index_accepts_in_range() {
        assert!(validate_item_index(0).is_ok());
        assert!(validate_item_index(ITEM_SLOT_COUNT - 1).is_ok());
    }

    #[test]
    fn validate_index_rejects_out_of_range() {
        assert!(matches!(
            validate_item_index(ITEM_SLOT_COUNT),
            Err(ItemStoreError::OutOfRange { .. })
        ));
    }

    #[test]
    fn patch_roundtrip_preserves_fields() {
        let mut item = Item {
            used: 1,
            value: 12_345,
            placement: 0x0040,
            flags: 0xDEAD_BEEF,
            ..Item::default()
        };
        item.skill[3] = [1, 2, 3];
        item.name[..4].copy_from_slice(b"boot");

        let patch = ItemPatch::from_item(7, &item);
        let bytes = patch.to_bytes().expect("encode");
        let decoded = ItemPatch::from_bytes(&bytes).expect("decode");
        assert_eq!(patch, decoded);
        assert_eq!(decoded.id, 7);
    }

    #[test]
    fn apply_preserves_dynamic_fields() {
        let mut existing = Item {
            x: 12,
            y: 34,
            carried: 99,
            damage_state: 2,
            current_age: [10, 20],
            current_damage: 5,
            sprite_override: 444,
            value: 1,
            ..Item::default()
        };

        let new_item = Item {
            value: 9_999,
            flags: 0xAA,
            ..Item::default()
        };
        let patch = ItemPatch::from_item(5, &new_item);

        patch.apply_to(&mut existing);

        // Static fields overwritten.
        assert_eq!(existing.value, 9_999);
        assert_eq!(existing.flags, 0xAA);
        // Dynamic fields preserved.
        assert_eq!(existing.x, 12);
        assert_eq!(existing.y, 34);
        assert_eq!(existing.carried, 99);
        assert_eq!(existing.damage_state, 2);
        assert_eq!(existing.current_age, [10, 20]);
        assert_eq!(existing.current_damage, 5);
        assert_eq!(existing.sprite_override, 444);
    }

    #[test]
    fn from_bytes_rejects_trailing_bytes() {
        let patch = ItemPatch::from_item(0, &Item::default());
        let mut bytes = patch.to_bytes().expect("encode");
        bytes.push(0xFF);
        assert!(matches!(
            ItemPatch::from_bytes(&bytes),
            Err(ItemStoreError::Decode(_))
        ));
    }
}
