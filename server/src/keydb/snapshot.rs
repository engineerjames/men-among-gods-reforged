//! World snapshot format for portable, versioned game-world backups.
//!
//! A [`WorldSnapshot`] captures all serialisable game state into a single
//! binary file (conventionally `*.wsnap`) using `bincode`'s standard
//! configuration — the same codec used for KeyDB blobs.
//!
//! ## File layout
//!
//! The file is a single `bincode`-encoded [`WorldSnapshot`] value.  The
//! first fields (`magic`, `schema_version`) are always written and checked
//! first so that incompatible files can be rejected cheaply before decoding
//! the rest of the (potentially large) map data.
//!
//! ## Versioning
//!
//! Current entity types come from [`core::types::v2`]. When a struct evolves
//! again, freeze the current shape under `core::types::v{N}`, bump
//! [`SNAPSHOT_SCHEMA_VERSION`], and add a migration arm in
//! [`WorldSnapshot::from_file`] that decodes legacy bytes via the frozen
//! struct, converts to the live shape (`impl From<vN::Foo> for crate::types::Foo`),
//! and returns the current representation.

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use bincode::{Decode, Encode};

/// Magic bytes that identify a valid `.wsnap` file.
pub const SNAPSHOT_MAGIC: [u8; 4] = *b"MGSN";

/// Magic bytes that identify a zstd-wrapped `.wsnap` file. The payload
/// after these four bytes is a raw zstd stream whose decompressed contents
/// are themselves a full `MGSN`-prefixed snapshot.
pub const SNAPSHOT_MAGIC_ZSTD: [u8; 4] = *b"MGSZ";

/// zstd compression level used when writing snapshots. Level 19 keeps
/// world_seed.wsnap well under GitHub's 100 MB single-file limit while
/// staying fast enough for the background saver / `world-snapshot export`.
const SNAPSHOT_ZSTD_LEVEL: i32 = 19;

/// Upper bound on the decompressed size of a `MGSZ` payload. The current
/// uncompressed world_seed.wsnap is ~104 MB; 1 GiB leaves ample headroom
/// for future growth while still rejecting obviously bogus inputs.
const MAX_DECOMPRESSED_BYTES: usize = 1024 * 1024 * 1024;

/// Current schema version written into every snapshot.
///
/// Bump this (and add a migration arm in [`WorldSnapshot::from_file`])
/// whenever any serialised entity struct changes shape.
///
/// History:
/// - **1** — initial layout (50-slot `Character.skill` / `Item.skill`).
/// - **2** — skill matrix grew to [`core::skills::MAX_SKILLS`] (75) for
///   Harakim ability slots and future-class headroom.
pub const SNAPSHOT_SCHEMA_VERSION: u32 = 2;

/// Complete portable snapshot of all server game-world data.
///
/// Encodes the entire world — map tiles, items, characters, effects, globals,
/// and text data — into a single `bincode` binary for backup, transfer, or
/// manual editing.  The [`magic`](WorldSnapshot::magic) and
/// [`schema_version`](WorldSnapshot::schema_version) fields are validated on
/// decode before the rest of the payload is returned to the caller.
#[derive(Debug, Encode, Decode)]
pub struct WorldSnapshot {
    /// Magic bytes; must equal [`SNAPSHOT_MAGIC`] (`b"MGSN"`).
    pub magic: [u8; 4],

    /// Snapshot schema version; must equal [`SNAPSHOT_SCHEMA_VERSION`].
    pub schema_version: u32,

    /// Wall-clock time at export (seconds since Unix epoch).
    pub created_unix_secs: i64,

    /// All map tiles in row-major order (`x + y * SERVER_MAPX`).
    pub map: Vec<core::types::v2::Map>,

    /// All item slots (length `MAXITEM`).
    pub items: Vec<core::types::v2::Item>,

    /// Item templates used for spawning/resetting items (length `MAXTITEM`).
    pub item_templates: Vec<core::types::v2::Item>,

    /// All character slots — players and NPCs (length `MAXCHARS`).
    pub characters: Vec<core::types::v2::Character>,

    /// Character templates used for NPC spawning (length `MAXTCHARS`).
    pub character_templates: Vec<core::types::v2::Character>,

    /// All world effect slots (length `MAXEFFECT`).
    pub effects: Vec<core::types::v2::Effect>,

    /// Single global server state value.
    pub globals: core::types::v2::Global,

    /// Banned player name patterns, one per entry.
    pub bad_names: Vec<String>,

    /// Banned chat words, one per entry.
    pub bad_words: Vec<String>,

    /// Message of the day shown to players at login.
    pub motd: String,
}

impl WorldSnapshot {
    /// Create a new snapshot with the current timestamp and the supplied data.
    ///
    /// The `magic` and `schema_version` fields are set automatically.
    ///
    /// # Arguments
    ///
    /// * `map`                  - All map tiles in row-major order.
    /// * `items`                - All item slots.
    /// * `item_templates`       - All item templates.
    /// * `characters`           - All character slots.
    /// * `character_templates`  - All character templates.
    /// * `effects`              - All effect slots.
    /// * `globals`              - The single global state value.
    /// * `bad_names`            - Banned player name patterns.
    /// * `bad_words`            - Banned chat words.
    /// * `motd`                 - Message of the day.
    ///
    /// # Returns
    ///
    /// * A fully populated [`WorldSnapshot`] ready to be written to disk.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        map: Vec<core::types::v2::Map>,
        items: Vec<core::types::v2::Item>,
        item_templates: Vec<core::types::v2::Item>,
        characters: Vec<core::types::v2::Character>,
        character_templates: Vec<core::types::v2::Character>,
        effects: Vec<core::types::v2::Effect>,
        globals: core::types::v2::Global,
        bad_names: Vec<String>,
        bad_words: Vec<String>,
        motd: String,
    ) -> Self {
        let created_unix_secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        Self {
            magic: SNAPSHOT_MAGIC,
            schema_version: SNAPSHOT_SCHEMA_VERSION,
            created_unix_secs,
            map,
            items,
            item_templates,
            characters,
            character_templates,
            effects,
            globals,
            bad_names,
            bad_words,
            motd,
        }
    }

    /// Encode and write this snapshot to a file.
    ///
    /// # Arguments
    ///
    /// * `path` - Destination path for the `.wsnap` file.
    ///
    /// # Returns
    ///
    /// * `Ok(())` on success.
    /// * `Err(String)` on encode or I/O failure.
    pub fn to_file(&self, path: &Path) -> Result<(), String> {
        let bytes = bincode::encode_to_vec(self, bincode::config::standard())
            .map_err(|e| format!("WorldSnapshot encode: {e}"))?;
        let compressed = zstd::bulk::compress(&bytes, SNAPSHOT_ZSTD_LEVEL)
            .map_err(|e| format!("WorldSnapshot zstd compress: {e}"))?;
        let mut out = Vec::with_capacity(SNAPSHOT_MAGIC_ZSTD.len() + compressed.len());
        out.extend_from_slice(&SNAPSHOT_MAGIC_ZSTD);
        out.extend_from_slice(&compressed);
        std::fs::write(path, &out)
            .map_err(|e| format!("WorldSnapshot write {}: {e}", path.display()))?;
        Ok(())
    }

    /// Read and decode a snapshot from a file, validating magic and version.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the `.wsnap` file to read.
    ///
    /// # Returns
    ///
    /// * `Ok(WorldSnapshot)` on success.
    /// * `Err(String)` if the file cannot be read, the magic is wrong, the
    ///   schema version is unsupported, or the data cannot be decoded.
    pub fn from_file(path: &Path) -> Result<Self, String> {
        let raw = std::fs::read(path)
            .map_err(|e| format!("WorldSnapshot read {}: {e}", path.display()))?;

        // Transparently decompress zstd-wrapped snapshots. The first four
        // bytes pick the envelope: `MGSZ` = zstd-wrapped `MGSN` payload,
        // otherwise the file is assumed to be a raw `MGSN` snapshot.
        let bytes: std::borrow::Cow<'_, [u8]> = if raw.len() >= SNAPSHOT_MAGIC_ZSTD.len()
            && raw[..4] == SNAPSHOT_MAGIC_ZSTD
        {
            let decoded = zstd::bulk::decompress(&raw[4..], MAX_DECOMPRESSED_BYTES)
                .map_err(|e| format!("WorldSnapshot zstd decompress {}: {e}", path.display()))?;
            std::borrow::Cow::Owned(decoded)
        } else {
            std::borrow::Cow::Owned(raw)
        };

        // Peek at magic + schema version without decoding the (potentially
        // huge) body so we can dispatch to a migration arm if the on-disk
        // layout doesn't match the current `Self`.
        let header: SnapshotHeader =
            bincode::decode_from_slice(&bytes, bincode::config::standard())
                .map(|(h, _)| h)
                .map_err(|e| format!("WorldSnapshot header decode {}: {e}", path.display()))?;

        if header.magic != SNAPSHOT_MAGIC {
            return Err(format!(
                "Invalid snapshot magic in {}: expected {:?}, got {:?}",
                path.display(),
                SNAPSHOT_MAGIC,
                header.magic
            ));
        }

        match header.schema_version {
            SNAPSHOT_SCHEMA_VERSION => {
                let (snapshot, _consumed): (Self, usize) =
                    bincode::decode_from_slice(&bytes, bincode::config::standard())
                        .map_err(|e| format!("WorldSnapshot decode {}: {e}", path.display()))?;
                Ok(snapshot)
            }
            1 => migrate_v1_to_current(&bytes, path),
            other => Err(format!(
                "Unsupported snapshot schema version {} in {} (expected {}). \
                 A migration tool is required.",
                other,
                path.display(),
                SNAPSHOT_SCHEMA_VERSION
            )),
        }
    }

    /// Return a human-readable summary of this snapshot's contents.
    ///
    /// # Returns
    ///
    /// * A multi-line string listing record counts and metadata.
    pub fn summary(&self) -> String {
        format!(
            "WorldSnapshot v{} created={} map={} items={} item_templates={} \
             characters={} character_templates={} effects={} bad_names={} \
             bad_words={} motd_len={}",
            self.schema_version,
            self.created_unix_secs,
            self.map.len(),
            self.items.len(),
            self.item_templates.len(),
            self.characters.len(),
            self.character_templates.len(),
            self.effects.len(),
            self.bad_names.len(),
            self.bad_words.len(),
            self.motd.len(),
        )
    }
}

/// Just the leading fields of [`WorldSnapshot`] used to peek at version
/// information before committing to a full decode.
#[derive(Decode)]
struct SnapshotHeader {
    magic: [u8; 4],
    schema_version: u32,
}

/// Frozen on-disk shape of [`WorldSnapshot`] at schema version 1.
///
/// Differs from the current shape only in using the v1 entity types
/// (50-slot skill matrix on `Character` / `Item`).
#[derive(Decode)]
struct WorldSnapshotV1 {
    #[allow(dead_code)]
    magic: [u8; 4],
    #[allow(dead_code)]
    schema_version: u32,
    created_unix_secs: i64,
    map: Vec<core::types::v1::Map>,
    items: Vec<core::types::v1::Item>,
    item_templates: Vec<core::types::v1::Item>,
    characters: Vec<core::types::v1::Character>,
    character_templates: Vec<core::types::v1::Character>,
    effects: Vec<core::types::v1::Effect>,
    globals: core::types::v1::Global,
    bad_names: Vec<String>,
    bad_words: Vec<String>,
    motd: String,
}

/// Decode the legacy v1 snapshot bytes and convert to the live (v2) shape.
///
/// Each `v1::Character` / `v1::Item` is promoted via its `From` impl, which
/// zero-pads the skill matrix from 50 rows to [`core::skills::MAX_SKILLS`]
/// rows. All other entity types (`Map`, `Effect`, `Global`) are unchanged
/// between v1 and v2 and pass through verbatim.
///
/// # Arguments
///
/// * `bytes` - Raw bytes of the legacy v1 `.wsnap` file.
/// * `path`  - Path the bytes were read from (used in error messages).
///
/// # Returns
///
/// * `Ok(WorldSnapshot)` populated with the migrated content, tagged with
///   the current schema version.
/// * `Err(String)` if the v1 payload cannot be decoded.
fn migrate_v1_to_current(bytes: &[u8], path: &Path) -> Result<WorldSnapshot, String> {
    let (v1, _consumed): (WorldSnapshotV1, usize) =
        bincode::decode_from_slice(bytes, bincode::config::standard())
            .map_err(|e| format!("WorldSnapshot v1 decode {}: {e}", path.display()))?;

    Ok(WorldSnapshot {
        magic: SNAPSHOT_MAGIC,
        schema_version: SNAPSHOT_SCHEMA_VERSION,
        created_unix_secs: v1.created_unix_secs,
        map: v1.map,
        items: v1.items.into_iter().map(Into::into).collect(),
        item_templates: v1.item_templates.into_iter().map(Into::into).collect(),
        characters: v1.characters.into_iter().map(Into::into).collect(),
        character_templates: v1.character_templates.into_iter().map(Into::into).collect(),
        effects: v1.effects,
        globals: v1.globals,
        bad_names: v1.bad_names,
        bad_words: v1.bad_words,
        motd: v1.motd,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Construct a minimal snapshot and verify encode/decode roundtrip integrity.
    #[test]
    fn encode_decode_roundtrip_snapshot() {
        let original = WorldSnapshot::new(
            vec![core::types::v2::Map::default(); 4],
            vec![core::types::v2::Item::default(); 2],
            vec![core::types::v2::Item::default(); 1],
            vec![core::types::v2::Character::default(); 2],
            vec![core::types::v2::Character::default(); 1],
            vec![core::types::v2::Effect::default(); 2],
            core::types::v2::Global::default(),
            vec!["badname".to_owned()],
            vec!["badword".to_owned()],
            "Hello world!".to_owned(),
        );

        let bytes = bincode::encode_to_vec(&original, bincode::config::standard())
            .expect("encode WorldSnapshot");
        let (decoded, _): (WorldSnapshot, _) =
            bincode::decode_from_slice(&bytes, bincode::config::standard())
                .expect("decode WorldSnapshot");

        assert_eq!(decoded.magic, SNAPSHOT_MAGIC);
        assert_eq!(decoded.schema_version, SNAPSHOT_SCHEMA_VERSION);
        assert_eq!(decoded.map.len(), 4);
        assert_eq!(decoded.items.len(), 2);
        assert_eq!(decoded.item_templates.len(), 1);
        assert_eq!(decoded.characters.len(), 2);
        assert_eq!(decoded.character_templates.len(), 1);
        assert_eq!(decoded.effects.len(), 2);
        assert_eq!(decoded.bad_names, vec!["badname".to_owned()]);
        assert_eq!(decoded.bad_words, vec!["badword".to_owned()]);
        assert_eq!(decoded.motd, "Hello world!");
    }

    /// Verify [`WorldSnapshot::to_file`] and [`WorldSnapshot::from_file`] roundtrip.
    #[test]
    fn file_roundtrip() {
        let tmp = std::env::temp_dir().join("test_world_snapshot_roundtrip.wsnap");

        let original = WorldSnapshot::new(
            vec![core::types::v2::Map::default()],
            vec![core::types::v2::Item::default()],
            vec![],
            vec![core::types::v2::Character::default()],
            vec![],
            vec![],
            core::types::v2::Global::default(),
            vec![],
            vec![],
            "Test MOTD".to_owned(),
        );

        original.to_file(&tmp).expect("to_file should succeed");
        let loaded = WorldSnapshot::from_file(&tmp).expect("from_file should succeed");

        assert_eq!(loaded.schema_version, SNAPSHOT_SCHEMA_VERSION);
        assert_eq!(loaded.map.len(), 1);
        assert_eq!(loaded.motd, "Test MOTD");

        let _ = std::fs::remove_file(&tmp);
    }

    /// A file with a wrong magic header is rejected with a clear error message.
    #[test]
    fn from_file_rejects_bad_magic() {
        let tmp = std::env::temp_dir().join("test_world_snapshot_bad_magic.wsnap");

        let mut bad = WorldSnapshot::new(
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
            core::types::v2::Global::default(),
            vec![],
            vec![],
            String::new(),
        );
        bad.magic = *b"XXXX";
        bad.to_file(&tmp).expect("to_file should succeed");

        let result = WorldSnapshot::from_file(&tmp);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid snapshot magic"));

        let _ = std::fs::remove_file(&tmp);
    }

    /// A file with an unsupported schema version is rejected with a clear error.
    #[test]
    fn from_file_rejects_wrong_version() {
        let tmp = std::env::temp_dir().join("test_world_snapshot_wrong_version.wsnap");

        let mut bad = WorldSnapshot::new(
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
            core::types::v2::Global::default(),
            vec![],
            vec![],
            String::new(),
        );
        bad.schema_version = 99;
        bad.to_file(&tmp).expect("to_file should succeed");

        let result = WorldSnapshot::from_file(&tmp);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .contains("Unsupported snapshot schema version")
        );

        let _ = std::fs::remove_file(&tmp);
    }

    /// Snapshot schema version constant guard — mirrors keydb_store's schema test.
    #[test]
    fn snapshot_schema_version_is_two() {
        assert_eq!(SNAPSHOT_SCHEMA_VERSION, 2);
    }
}
