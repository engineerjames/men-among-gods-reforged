//! Shared KeyDB key schema and helpers for live character editing.
//!
//! Mirrors [`crate::map_store`] for the runtime `Character` slice. The `api`
//! crate exposes admin endpoints that read individual character slots and
//! enqueue [`CharacterPatch`] entries; the running server drains the queue
//! between ticks and merges only the static authoring fields into its
//! in-memory character table while preserving dynamic runtime fields.
//!
//! ## Static vs dynamic fields
//!
//! [`CharacterPatch`] carries the **static authoring fields** that world
//! builders own — name, reference, description, race/kindred, account
//! linkage (`player`, `pass1`/`pass2`), base sprite/sound, flags,
//! alignment, temple/tavern coordinates, template origin, base
//! attribute/skill/hp/end/mana arrays, weapon/armor bonuses, base movement
//! mode/speed, monster class, light bonus, gethit damage, NPC dialogue
//! `text`, password, and driver `data`. Defensive copies of `passwd` are
//! preserved so the API can rotate credentials.
//!
//! The following dynamic runtime fields are intentionally **not** part of
//! the patch and are owned by the tick loop:
//!
//! * Position: `x`, `y`, `tox`, `toy`, `frx`, `fry`, `dir`
//! * Animation/state: `status`, `status2`, `lastattack`, `sprite_override`,
//!   `last_action`
//! * Combat AI: `attack_cn`, `skill_nr`, `skill_target1`, `skill_target2`,
//!   `goto_x`, `goto_y`, `use_nr`, `misc_action`, `misc_target1`,
//!   `misc_target2`, `cerrno`, `escape_timer`, `enemy`, `current_enemy`,
//!   `retry`, `stunned`, `unreach`, `unreachx`, `unreachy`
//! * Live resource pools: `a_hp`, `a_end`, `a_mana`, `light`
//! * Networking: `addr`, `current_online_time`, `total_online_time`,
//!   `comp_volume`, `raw_volume`, `idle`, `login_date`, `logout_date`
//! * Inventory and economy: `gold`, `item`, `worn`, `spell`, `citem`,
//!   `depot`, `depot_cost`, `depot_sold`, `luck`
//! * Identity timestamps managed by the server: `creation_date`
//! * Talent progression: `future1`
//! * Reserved padding: `unused`, `future2`, `future3`
//!
//! The watcher overwrites only the patch fields when applying, so the
//! tick thread keeps full ownership of placement, combat, and per-character
//! progression.

use crate::constants::MAXCHARS;
use crate::skills::{MAX_SKILLS, SkillIndex};
use bincode::{Decode, Encode};

/// Width of the per-character attribute/skill arrays.
const SKILL_AXIS: usize = SkillIndex::MaxIndex as usize;

// ---------------------------------------------------------------------------
//  Key schema
// ---------------------------------------------------------------------------

/// KeyDB key prefix for individual character slots: `game:char:{idx}`.
pub const CHARACTER_KEY_PREFIX: &str = "game:char:";

/// KeyDB list key holding queued [`CharacterPatch`] entries (RPUSH/LPOP).
pub const CHARACTER_PATCH_QUEUE_KEY: &str = "game:char:patch_queue";

/// KeyDB key the API writes a JSON reload payload into to flush the queue.
///
/// Carries `{ request_id, requested_at }`. The server's character-patch
/// watcher drains it via `GETDEL`, applies all queued patches synchronously
/// on the tick thread, and writes a status entry under
/// [`character_patch_status_key`].
pub const CHARACTER_PATCH_REQUEST_KEY: &str = "game:char:patch_request";

/// Pub/sub channel name reserved for future automatic patch signalling.
pub const CHARACTER_PATCH_PUBSUB_CHANNEL: &str = "game:char:patch";

/// KeyDB counter incremented after every character write through the admin API.
pub const CHARACTER_VERSION_KEY: &str = "game:meta:char:version";

/// Maximum number of character slots.
///
/// Re-exported from [`crate::constants::MAXCHARS`] so callers do not have to
/// reach into both modules.
pub const CHARACTER_SLOT_COUNT: usize = MAXCHARS;

/// Build the KeyDB key for a single character slot.
///
/// # Arguments
///
/// * `index` - Zero-based slot index.
///
/// # Returns
///
/// * The fully-formatted key (e.g. `"game:char:42"`).
pub fn character_key(index: usize) -> String {
    format!("{}{}", CHARACTER_KEY_PREFIX, index)
}

/// Build the KeyDB key for a character-patch reload-status response.
///
/// # Arguments
///
/// * `request_id` - Identifier returned by the admin API.
///
/// # Returns
///
/// * The fully-formatted status key.
pub fn character_patch_status_key(request_id: &str) -> String {
    format!("game:char:patch_status:{}", request_id)
}

// ---------------------------------------------------------------------------
//  Errors
// ---------------------------------------------------------------------------

/// Error returned by character-store helpers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CharacterStoreError {
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

impl std::fmt::Display for CharacterStoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::OutOfRange { index, slot_count } => write!(
                f,
                "character index {} out of range (max {})",
                index,
                slot_count.saturating_sub(1)
            ),
            Self::Mismatch { expected, actual } => write!(
                f,
                "character patch id {} does not match slot {}",
                actual, expected
            ),
            Self::Encode(msg) => write!(f, "character encode failed: {}", msg),
            Self::Decode(msg) => write!(f, "character decode failed: {}", msg),
        }
    }
}

impl std::error::Error for CharacterStoreError {}

/// Validate that `index` falls within the character slot range.
///
/// # Arguments
///
/// * `index` - The slot index to validate.
///
/// # Returns
///
/// * `Ok(())` when `index < MAXCHARS`.
/// * `Err(CharacterStoreError::OutOfRange { .. })` otherwise.
pub fn validate_character_index(index: usize) -> Result<(), CharacterStoreError> {
    if index < CHARACTER_SLOT_COUNT {
        Ok(())
    } else {
        Err(CharacterStoreError::OutOfRange {
            index,
            slot_count: CHARACTER_SLOT_COUNT,
        })
    }
}

// ---------------------------------------------------------------------------
//  CharacterPatch
// ---------------------------------------------------------------------------

/// Static authoring fields for a single character slot.
///
/// The admin API accepts these as the `PUT /admin/world/characters/{id}`
/// body. Only fields managed by world-builders are present; dynamic
/// runtime fields (position, combat AI, current resource pools,
/// inventory, networking) are owned by the tick loop and preserved by
/// the server when the patch is applied.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode, Decode)]
pub struct CharacterPatch {
    /// Slot index this patch targets. Must match the URL path component.
    pub id: u32,
    /// `used` flag (e.g. `USE_EMPTY`, `USE_PLAYER`, `USE_CHAR`).
    pub used: u8,
    /// Character name.
    pub name: [u8; 40],
    /// Character reference string.
    pub reference: [u8; 40],
    /// Character description.
    pub description: [u8; 200],
    /// Race / kindred.
    pub kindred: i32,
    /// Owning account id (-1 for NPCs).
    pub player: i32,
    /// Password lo half.
    pub pass1: u32,
    /// Password hi half.
    pub pass2: u32,
    /// Sprite base value.
    pub sprite: u16,
    /// Sound base value.
    pub sound: u16,
    /// Bitset of character flags.
    pub flags: u64,
    /// Alignment value.
    pub alignment: i16,
    /// Temple X coordinate (recall + death respawn).
    pub temple_x: u16,
    /// Temple Y coordinate.
    pub temple_y: u16,
    /// Tavern X coordinate (re-login spawn).
    pub tavern_x: u16,
    /// Tavern Y coordinate.
    pub tavern_y: u16,
    /// Template index this slot was created from.
    pub temp: u16,
    /// Per-attribute base values (`[base, mod, max?]` per skill axis).
    pub attrib: [[u8; SKILL_AXIS]; 5],
    /// Base HP per skill axis.
    pub hp: [u16; SKILL_AXIS],
    /// Base endurance per skill axis.
    pub end: [u16; SKILL_AXIS],
    /// Base mana per skill axis.
    pub mana: [u16; SKILL_AXIS],
    /// Base skill values, `[base, mod, max?]` per skill axis.
    pub skill: [[u8; SKILL_AXIS]; MAX_SKILLS],
    /// Weapon proficiency bonus.
    pub weapon_bonus: u8,
    /// Armor proficiency bonus.
    pub armor_bonus: u8,
    /// Base movement mode (`0` slow, `1` medium, `2` fast).
    pub mode: u8,
    /// Base movement speed.
    pub speed: i16,
    /// Race specific speed modifier.
    pub speed_mod: i8,
    /// Damage dealt to attackers when this character is hit.
    pub gethit_dam: i8,
    /// Race-specific bonus to `gethit_dam`.
    pub gethit_bonus: i8,
    /// Permanent light radius.
    pub light_bonus: u8,
    /// Monster classification.
    pub monster_class: i32,
    /// Persisted password hash bytes.
    pub passwd: [u8; 16],
    /// Custom NPC text lines (greetings, death cries, etc.).
    pub text: [[u8; 160]; 10],
    /// Driver-specific data.
    pub data: [i32; 100],
}

impl CharacterPatch {
    /// Build a [`CharacterPatch`] from a full [`crate::types::Character`]
    /// for slot `index`.
    ///
    /// Dynamic runtime fields on the source character are dropped; only
    /// the authoring fields end up on the patch.
    ///
    /// # Arguments
    ///
    /// * `index`     - Slot index this patch will be applied to.
    /// * `character` - Source character.
    ///
    /// # Returns
    ///
    /// * A patch carrying `index` as `id` plus the static fields of `character`.
    pub fn from_character(index: usize, character: &crate::types::Character) -> Self {
        Self {
            id: index as u32,
            used: character.used,
            name: character.name,
            reference: character.reference,
            description: character.description,
            kindred: character.kindred,
            player: character.player,
            pass1: character.pass1,
            pass2: character.pass2,
            sprite: character.sprite,
            sound: character.sound,
            flags: character.flags,
            alignment: character.alignment,
            temple_x: character.temple_x,
            temple_y: character.temple_y,
            tavern_x: character.tavern_x,
            tavern_y: character.tavern_y,
            temp: character.temp,
            attrib: character.attrib,
            hp: character.hp,
            end: character.end,
            mana: character.mana,
            skill: character.skill,
            weapon_bonus: character.weapon_bonus,
            armor_bonus: character.armor_bonus,
            mode: character.mode,
            speed: character.speed,
            speed_mod: character.speed_mod,
            gethit_dam: character.gethit_dam,
            gethit_bonus: character.gethit_bonus,
            light_bonus: character.light_bonus,
            monster_class: character.monster_class,
            passwd: character.passwd,
            text: character.text,
            data: character.data,
        }
    }

    /// Apply this patch's static fields onto `target`, leaving dynamic
    /// runtime fields (position, combat AI, current resources, inventory,
    /// networking, padding) untouched.
    ///
    /// # Arguments
    ///
    /// * `target` - Mutable reference to the in-memory character slot.
    pub fn apply_to(&self, target: &mut crate::types::Character) {
        target.used = self.used;
        target.name = self.name;
        target.reference = self.reference;
        target.description = self.description;
        target.kindred = self.kindred;
        target.player = self.player;
        target.pass1 = self.pass1;
        target.pass2 = self.pass2;
        target.sprite = self.sprite;
        target.sound = self.sound;
        target.flags = self.flags;
        target.alignment = self.alignment;
        target.temple_x = self.temple_x;
        target.temple_y = self.temple_y;
        target.tavern_x = self.tavern_x;
        target.tavern_y = self.tavern_y;
        target.temp = self.temp;
        target.attrib = self.attrib;
        target.hp = self.hp;
        target.end = self.end;
        target.mana = self.mana;
        target.skill = self.skill;
        target.weapon_bonus = self.weapon_bonus;
        target.armor_bonus = self.armor_bonus;
        target.mode = self.mode;
        target.speed = self.speed;
        target.speed_mod = self.speed_mod;
        target.gethit_dam = self.gethit_dam;
        target.gethit_bonus = self.gethit_bonus;
        target.light_bonus = self.light_bonus;
        target.monster_class = self.monster_class;
        target.passwd = self.passwd;
        target.text = self.text;
        target.data = self.data;
    }

    /// Encode this patch to the canonical bincode byte representation.
    ///
    /// # Returns
    ///
    /// * `Ok(bytes)` on success.
    /// * `Err(CharacterStoreError::Encode)` on bincode failure.
    pub fn to_bytes(&self) -> Result<Vec<u8>, CharacterStoreError> {
        bincode::encode_to_vec(self, bincode::config::standard())
            .map_err(|e| CharacterStoreError::Encode(e.to_string()))
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
    /// * `Err(CharacterStoreError::Decode)` when bincode decoding fails or
    ///   trailing bytes remain.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, CharacterStoreError> {
        let (value, consumed): (Self, usize) =
            bincode::decode_from_slice(bytes, bincode::config::standard())
                .map_err(|e| CharacterStoreError::Decode(e.to_string()))?;
        if consumed != bytes.len() {
            return Err(CharacterStoreError::Decode(format!(
                "trailing bytes after CharacterPatch (consumed {}, total {})",
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
    use crate::types::Character;

    #[test]
    fn character_key_formats_index() {
        assert_eq!(character_key(0), "game:char:0");
        assert_eq!(character_key(8_191), "game:char:8191");
    }

    #[test]
    fn character_patch_status_key_includes_id() {
        assert_eq!(
            character_patch_status_key("abc"),
            "game:char:patch_status:abc"
        );
    }

    #[test]
    fn validate_index_accepts_in_range() {
        assert!(validate_character_index(0).is_ok());
        assert!(validate_character_index(CHARACTER_SLOT_COUNT - 1).is_ok());
    }

    #[test]
    fn validate_index_rejects_out_of_range() {
        assert!(matches!(
            validate_character_index(CHARACTER_SLOT_COUNT),
            Err(CharacterStoreError::OutOfRange { .. })
        ));
    }

    #[test]
    fn patch_roundtrip_preserves_fields() {
        let mut character = Character {
            used: 1,
            kindred: 7,
            flags: 0xCAFEBABE,
            alignment: -42,
            temple_x: 320,
            temple_y: 240,
            ..Character::default()
        };
        character.skill[12][3] = 5;
        character.text[1][..3].copy_from_slice(b"Hi!");

        let patch = CharacterPatch::from_character(13, &character);
        let bytes = patch.to_bytes().expect("encode");
        let decoded = CharacterPatch::from_bytes(&bytes).expect("decode");
        assert_eq!(patch, decoded);
        assert_eq!(decoded.id, 13);
    }

    #[test]
    fn apply_preserves_dynamic_fields() {
        let mut existing = Character {
            x: 11,
            y: 22,
            tox: 33,
            toy: 44,
            dir: 5,
            status: 7,
            a_hp: 999,
            a_end: 888,
            a_mana: 777,
            gold: 1_234_567,
            ..Character::default()
        };
        existing.item[0] = 42;
        existing.worn[1] = 24;
        existing.spell[2] = 13;
        existing.citem = 99;
        existing.attack_cn = 7;
        existing.skill_nr = 8;
        existing.goto_x = 12;
        existing.goto_y = 13;
        existing.idle = 555;
        existing.addr = 0xDEAD_BEEF;
        existing.current_online_time = 1_111;
        existing.depot[0] = 99;
        existing.depot_cost = 5;
        existing.luck = 100;
        existing.kindred = 1; // static field; should be overwritten

        let new_char = Character {
            kindred: 9,
            flags: 0xAAAA,
            ..Character::default()
        };
        let patch = CharacterPatch::from_character(5, &new_char);
        patch.apply_to(&mut existing);

        // Static fields overwritten.
        assert_eq!(existing.kindred, 9);
        assert_eq!(existing.flags, 0xAAAA);
        // Dynamic fields preserved.
        assert_eq!(existing.x, 11);
        assert_eq!(existing.y, 22);
        assert_eq!(existing.tox, 33);
        assert_eq!(existing.toy, 44);
        assert_eq!(existing.dir, 5);
        assert_eq!(existing.status, 7);
        assert_eq!(existing.a_hp, 999);
        assert_eq!(existing.a_end, 888);
        assert_eq!(existing.a_mana, 777);
        assert_eq!(existing.gold, 1_234_567);
        assert_eq!(existing.item[0], 42);
        assert_eq!(existing.worn[1], 24);
        assert_eq!(existing.spell[2], 13);
        assert_eq!(existing.citem, 99);
        assert_eq!(existing.attack_cn, 7);
        assert_eq!(existing.skill_nr, 8);
        assert_eq!(existing.goto_x, 12);
        assert_eq!(existing.goto_y, 13);
        assert_eq!(existing.idle, 555);
        assert_eq!(existing.addr, 0xDEAD_BEEF);
        assert_eq!(existing.current_online_time, 1_111);
        assert_eq!(existing.depot[0], 99);
        assert_eq!(existing.depot_cost, 5);
        assert_eq!(existing.luck, 100);
    }

    #[test]
    fn from_bytes_rejects_trailing_bytes() {
        let patch = CharacterPatch::from_character(0, &Character::default());
        let mut bytes = patch.to_bytes().expect("encode");
        bytes.push(0xFF);
        assert!(matches!(
            CharacterPatch::from_bytes(&bytes),
            Err(CharacterStoreError::Decode(_))
        ));
    }
}
