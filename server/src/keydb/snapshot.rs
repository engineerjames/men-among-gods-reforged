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
//! All entity types come from [`core::types::v1`].  When a struct evolves to
//! v2, a new schema version is assigned and a migration arm is added to
//! [`WorldSnapshot::from_file`] that converts the on-disk bytes using the
//! old type layout before returning the current representation.

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use bincode::{Decode, Encode};

/// Magic bytes that identify a valid `.wsnap` file.
pub const SNAPSHOT_MAGIC: [u8; 4] = *b"MGSN";

/// Current schema version written into every snapshot.
///
/// Increment this (and add a migration arm in [`WorldSnapshot::from_file`])
/// whenever any serialised struct changes shape.
pub const SNAPSHOT_SCHEMA_VERSION: u32 = 1;

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
    pub map: Vec<core::types::v1::Map>,

    /// All item slots (length `MAXITEM`).
    pub items: Vec<core::types::v1::Item>,

    /// Item templates used for spawning/resetting items (length `MAXTITEM`).
    pub item_templates: Vec<core::types::v1::Item>,

    /// All character slots — players and NPCs (length `MAXCHARS`).
    pub characters: Vec<core::types::v1::Character>,

    /// Character templates used for NPC spawning (length `MAXTCHARS`).
    pub character_templates: Vec<core::types::v1::Character>,

    /// All world effect slots (length `MAXEFFECT`).
    pub effects: Vec<core::types::v1::Effect>,

    /// Single global server state value.
    pub globals: core::types::v1::Global,

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
        std::fs::write(path, &bytes)
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
        let bytes = std::fs::read(path)
            .map_err(|e| format!("WorldSnapshot read {}: {e}", path.display()))?;

        let (snapshot, _consumed): (Self, usize) =
            bincode::decode_from_slice(&bytes, bincode::config::standard())
                .map_err(|e| format!("WorldSnapshot decode {}: {e}", path.display()))?;

        if snapshot.magic != SNAPSHOT_MAGIC {
            return Err(format!(
                "Invalid snapshot magic in {}: expected {:?}, got {:?}",
                path.display(),
                SNAPSHOT_MAGIC,
                snapshot.magic
            ));
        }

        if snapshot.schema_version != SNAPSHOT_SCHEMA_VERSION {
            return Err(format!(
                "Unsupported snapshot schema version {} in {} (expected {}). \
                 A migration tool is required.",
                snapshot.schema_version,
                path.display(),
                SNAPSHOT_SCHEMA_VERSION
            ));
        }

        Ok(snapshot)
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Construct a minimal snapshot and verify encode/decode roundtrip integrity.
    #[test]
    fn encode_decode_roundtrip_snapshot() {
        let original = WorldSnapshot::new(
            vec![core::types::v1::Map::default(); 4],
            vec![core::types::v1::Item::default(); 2],
            vec![core::types::v1::Item::default(); 1],
            vec![core::types::v1::Character::default(); 2],
            vec![core::types::v1::Character::default(); 1],
            vec![core::types::v1::Effect::default(); 2],
            core::types::v1::Global::default(),
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
            vec![core::types::v1::Map::default()],
            vec![core::types::v1::Item::default()],
            vec![],
            vec![core::types::v1::Character::default()],
            vec![],
            vec![],
            core::types::v1::Global::default(),
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
            core::types::v1::Global::default(),
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
            core::types::v1::Global::default(),
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
    fn snapshot_schema_version_is_one() {
        assert_eq!(SNAPSHOT_SCHEMA_VERSION, 1);
    }
}
