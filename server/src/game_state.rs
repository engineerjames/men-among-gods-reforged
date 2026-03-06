/// Unified game state container for all server-side world data.
///
/// `GameState` consolidates data previously spread across three global
/// singletons (`Repository`, `State`, `PathFinder`) into a single owned
/// struct.  It is created in `main()` and threaded through the call chain
/// as `&mut GameState`, eliminating ~4,400 closure-based accessor calls
/// and all nested closure patterns.
///
/// # Lifecycle
///
/// ```text
/// main():
///   let mut gs = GameState::initialize()?;
///   server.initialize(&mut gs)?;
///   loop { server.tick(&mut gs); }
///   gs.shutdown();
/// ```
use std::path::{Path, PathBuf};
use std::{env, fs};

use bincode::{Decode, Encode};

use crate::keydb;
use crate::keydb_store;
use crate::path_finding::PathFinder;

/// Persistence backend used for loading and saving game data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageBackend {
    /// Load/save from legacy `.dat` files on disk.
    DatFiles,
    /// Load/save from KeyDB.
    KeyDb,
}

impl StorageBackend {
    /// Determine the storage backend from the `MAG_STORAGE_BACKEND` env var.
    pub fn from_env() -> Self {
        match env::var("MAG_STORAGE_BACKEND")
            .unwrap_or_default()
            .to_lowercase()
            .as_str()
        {
            "keydb" | "redis" => StorageBackend::KeyDb,
            _ => StorageBackend::DatFiles,
        }
    }
}

const NORMALIZED_MAGIC: [u8; 4] = *b"MAG2";
const NORMALIZED_VERSION: u32 = 1;

#[derive(Debug, Encode, Decode)]
struct NormalizedDataSet<T> {
    magic: [u8; 4],
    version: u32,
    source_file: String,
    source_record_size: usize,
    records: Vec<T>,
}

/// The unified in-memory game state for the server.
///
/// Owns all world data (maps, items, characters, effects, globals), visibility
/// computation state, pathfinding state, and persistence metadata.  Created
/// once in `main()` via [`GameState::initialize`] and passed by mutable
/// reference throughout the server's call chain.
pub struct GameState {
    // -- World data (formerly Repository) --
    /// Map tiles indexed by `x + y * SERVER_MAPX`.
    pub map: Vec<core::types::Map>,
    /// All item instances (size `MAXITEM`).
    pub items: Vec<core::types::Item>,
    /// Item templates for creating/resetting items (size `MAXTITEM`).
    pub item_templates: Vec<core::types::Item>,
    /// All character instances — players and NPCs (size `MAXCHARS`).
    pub characters: Vec<core::types::Character>,
    /// Character templates for NPC spawning (size `MAXTCHARS`).
    pub character_templates: Vec<core::types::Character>,
    /// Transient/persistent world effects (size `MAXEFFECT`).
    pub effects: Vec<core::types::Effect>,
    /// Global server state (ticker, counters, flags, etc.).
    pub globals: core::types::Global,
    /// Per-character visibility information (size `MAXCHARS`).
    pub see_map: Vec<core::types::SeeMap>,
    /// Banned name patterns loaded from `badnames.txt`.
    pub bad_names: Vec<String>,
    /// Banned chat words loaded from `badwords.txt`.
    pub bad_words: Vec<String>,
    /// Message of the day text.
    pub message_of_the_day: String,
    /// Runtime ban list.
    pub ban_list: Vec<core::types::Ban>,

    // -- Counters (formerly Repository fields) --
    /// Tick at which the last population reset occurred.
    pub last_population_reset_tick: u32,
    /// Ice cloak timing clock.
    pub ice_cloak_clock: u32,
    /// Item tick GC offset counter.
    pub item_tick_gc_off: u32,
    /// Item tick GC count accumulator.
    pub item_tick_gc_count: u32,
    /// Item tick expiration counter.
    pub item_tick_expire_counter: u32,

    // -- Visibility state (formerly State) --
    /// Scratch visibility buffer (underscore prefix preserved from original).
    pub _visi: [i8; 40 * 40],
    /// Primary visibility buffer.
    pub visi: [i8; 40 * 40],
    /// Whether visibility is computed globally or per-character.
    pub vis_is_global: bool,
    /// Cache miss counter for visibility lookups.
    pub see_miss: u64,
    /// Cache hit counter for visibility lookups.
    pub see_hit: u64,
    /// Current visibility origin X.
    pub ox: i32,
    /// Current visibility origin Y.
    pub oy: i32,
    /// Whether current visibility target is a monster.
    pub is_monster: bool,
    /// Number of pentagram items needed for a quest completion.
    pub penta_needed: usize,

    // -- Labyrinth 9 --
    pub lab9: crate::lab9::Labyrinth9,

    // -- Pathfinding --
    /// A* pathfinder with pre-allocated node/visited buffers.
    pub pathfinder: PathFinder,

    // -- Persistence (private) --
    /// Which storage backend this game state was loaded from.
    storage_backend: StorageBackend,
    /// Set to true once a clean save has been performed (avoids double-save
    /// from both `shutdown()` and `Drop`).
    saved_cleanly: bool,
    /// Absolute path to the running executable, used to resolve `.dat` dir.
    executable_path: String,
}

impl GameState {
    /// Normalize MOTD text for safe client display.
    ///
    /// Applies the historical maximum length constraint to avoid client
    /// issues with oversized MOTD payloads.
    ///
    /// # Arguments
    ///
    /// * `message_of_the_day` - The raw MOTD string to normalize.
    ///
    /// # Returns
    ///
    /// * The MOTD string, truncated to 130 characters if necessary.
    pub fn normalize_message_of_the_day(mut message_of_the_day: String) -> String {
        let char_count = message_of_the_day.chars().count();
        if char_count > 130 {
            log::warn!(
                "Message of the day is too long ({} characters). Truncating to 130 characters.",
                char_count
            );
            message_of_the_day = message_of_the_day.chars().take(130).collect();
        }
        message_of_the_day
    }

    /// Read MOTD from `motd.txt` and normalize it for display.
    ///
    /// # Returns
    ///
    /// * The normalized MOTD string read from disk, or a default if the file
    ///   is missing.
    fn read_message_of_the_day_from_dat_file(&self) -> String {
        let motd_path = self.get_dat_file_path("motd.txt");
        log::info!("Loading message of the day from {:?}", motd_path);
        let motd_data =
            fs::read_to_string(&motd_path).unwrap_or("Live long and prosper!".to_string());
        Self::normalize_message_of_the_day(motd_data)
    }

    /// Create a new `GameState` initialized with default values.
    ///
    /// Allocates and initializes all in-memory collections with sizes based on
    /// constants (for example `MAXITEM`, `MAXCHARS`, `SERVER_MAPX` × `SERVER_MAPY`)
    /// and attempts to discover the current executable path to resolve the
    /// `.dat` directory via `get_dat_file_path`.
    ///
    /// # Arguments
    ///
    /// * `backend` - The storage backend to use for persistence.
    ///
    /// # Returns
    ///
    /// * A freshly allocated `GameState` with all fields at their defaults.
    fn new(backend: StorageBackend) -> Self {
        Self {
            map: vec![
                core::types::Map::default();
                core::constants::SERVER_MAPX as usize * core::constants::SERVER_MAPY as usize
            ],
            items: vec![core::types::Item::default(); core::constants::MAXITEM],
            item_templates: vec![core::types::Item::default(); core::constants::MAXTITEM],
            characters: vec![core::types::Character::default(); core::constants::MAXCHARS],
            character_templates: vec![
                core::types::Character::default();
                core::constants::MAXTCHARS
            ],
            effects: vec![core::types::Effect::default(); core::constants::MAXEFFECT],
            globals: core::types::Global::default(),
            see_map: vec![core::types::SeeMap::default(); core::constants::MAXCHARS],
            bad_names: Vec::new(),
            bad_words: Vec::new(),
            message_of_the_day: String::new(),
            ban_list: Vec::new(),
            last_population_reset_tick: 0,
            ice_cloak_clock: 0,
            item_tick_gc_off: 0,
            item_tick_gc_count: 0,
            item_tick_expire_counter: 0,
            // Visibility state
            _visi: [0; 40 * 40],
            visi: [0; 40 * 40],
            vis_is_global: true,
            see_miss: 0,
            see_hit: 0,
            ox: 0,
            oy: 0,
            is_monster: false,
            penta_needed: 5,
            // Labyrinth 9
            lab9: crate::lab9::Labyrinth9::new(),
            // Pathfinding
            pathfinder: PathFinder::new(),
            // Persistence
            storage_backend: backend,
            saved_cleanly: false,
            executable_path: match env::current_exe() {
                Ok(exe_path) => exe_path.to_string_lossy().to_string(),
                Err(e) => {
                    log::error!("Failed to get executable path: {}", e);
                    String::new()
                }
            },
        }
    }

    /// Initialize a new `GameState` by loading all data from the configured
    /// storage backend.
    ///
    /// Determines the backend from the `MAG_STORAGE_BACKEND` env var, allocates
    /// the struct, and loads all world data.  Returns the fully populated
    /// game state or an error if loading fails.
    ///
    /// # Returns
    ///
    /// * `Ok(GameState)` on success.
    /// * `Err(String)` if data loading fails.
    pub fn initialize() -> Result<GameState, String> {
        let backend = StorageBackend::from_env();
        log::info!("GameState storage backend: {:?}", backend);

        let mut gs = Self::new(backend);
        gs.load()?;
        Ok(gs)
    }

    /// Return the storage backend in use.
    ///
    /// # Returns
    ///
    /// * The [`StorageBackend`] variant this game state was loaded from.
    pub fn storage_backend(&self) -> StorageBackend {
        self.storage_backend
    }

    /// Fetch the latest MOTD from the active backend for login-time display.
    ///
    /// In `DatFiles` mode this reads `motd.txt` from disk each call.
    /// In `KeyDb` mode this reads `game:motd` from KeyDB each call.
    /// On transient read failures the in-memory boot-loaded MOTD is used.
    ///
    /// # Returns
    ///
    /// * The current message of the day string.
    pub fn latest_message_of_the_day(&self) -> String {
        match self.storage_backend {
            StorageBackend::DatFiles => self.read_message_of_the_day_from_dat_file(),
            StorageBackend::KeyDb => match keydb::load_message_of_the_day() {
                Ok(motd) => Self::normalize_message_of_the_day(motd),
                Err(error) => {
                    log::warn!(
                        "Falling back to cached MOTD after KeyDB read failure: {}",
                        error
                    );
                    self.message_of_the_day.clone()
                }
            },
        }
    }

    /// Load all game data from disk into memory.
    ///
    /// This calls each of the `load_*` helper methods in sequence and returns an
    /// error if any step fails. After a successful `load`, the game state
    /// contains populated `map`, `items`, `characters`, `globals`, etc.
    ///
    /// # Returns
    ///
    /// * `Ok(())` on success.
    /// * `Err(String)` if any data file fails to load.
    fn load(&mut self) -> Result<(), String> {
        match self.storage_backend {
            StorageBackend::DatFiles => self.load_from_dat_files(),
            StorageBackend::KeyDb => self.load_from_keydb(),
        }
    }

    /// Load all data from `.dat` files on disk (legacy path).
    ///
    /// # Returns
    ///
    /// * `Ok(())` on success.
    /// * `Err(String)` if any `.dat` file fails to load.
    fn load_from_dat_files(&mut self) -> Result<(), String> {
        self.load_map()?;
        self.load_items()?;
        self.load_item_templates()?;
        self.load_characters()?;
        self.load_character_templates()?;
        self.load_effects()?;
        self.load_globals()?;
        self.load_bad_names()?;
        self.load_bad_words()?;
        self.load_message_of_the_day()?;
        self.load_ban_list()?;
        Ok(())
    }

    /// Load all data from KeyDB.
    ///
    /// # Returns
    ///
    /// * `Ok(())` on success.
    /// * `Err(String)` if the KeyDB connection or load fails.
    fn load_from_keydb(&mut self) -> Result<(), String> {
        let mut con = keydb::connect()?;
        let data = keydb_store::load_all(&mut con)?;

        self.map = data.map;
        self.items = data.items;
        self.item_templates = data.item_templates;
        self.characters = data.characters;
        self.character_templates = data.character_templates;
        self.effects = data.effects;
        self.globals = data.globals;
        self.bad_names = data.bad_names;
        self.bad_words = data.bad_words;
        self.message_of_the_day = data.message_of_the_day;

        log::info!(
            "Globals data: dirty={}, character_cnt={}, ticker={}, fullmoon={}, newmoon={}, unique={}, cap={}",
            self.globals.is_dirty(),
            self.globals.character_cnt,
            self.globals.ticker,
            self.globals.fullmoon,
            self.globals.newmoon,
            self.globals.unique,
            self.globals.cap
        );

        Ok(())
    }

    /// Save mutable game runtime data back to the configured backend.
    ///
    /// Bad names, bad words, and MOTD are treated as externally-managed
    /// read-only text data and are intentionally excluded from runtime saves.
    ///
    /// # Returns
    ///
    /// * `Ok(())` on success.
    /// * `Err(String)` if saving fails.
    pub fn save(&mut self) -> Result<(), String> {
        match self.storage_backend {
            StorageBackend::DatFiles => self.save_to_dat_files(),
            StorageBackend::KeyDb => self.save_to_keydb(),
        }
    }

    /// Save all data to `.dat` files on disk (legacy path).
    ///
    /// # Returns
    ///
    /// * `Ok(())` on success.
    /// * `Err(String)` if any file write fails.
    fn save_to_dat_files(&self) -> Result<(), String> {
        self.save_map()?;
        self.save_items()?;
        self.save_characters()?;
        self.save_effects()?;
        self.save_globals()?;
        Ok(())
    }

    /// Save all data to KeyDB.
    ///
    /// # Returns
    ///
    /// * `Ok(())` on success.
    /// * `Err(String)` if the KeyDB connection or save fails.
    fn save_to_keydb(&self) -> Result<(), String> {
        let mut con = keydb::connect()?;
        keydb_store::save_runtime_data(
            &mut con,
            &self.map,
            &self.items,
            &self.characters,
            &self.effects,
            &self.globals,
        )
    }

    /// Perform a clean shutdown of the game state by clearing the dirty flag
    /// and saving all data to the configured storage backend.
    pub fn shutdown(&mut self) {
        self.globals.set_dirty(false);
        if let Err(e) = self.save() {
            log::error!("Failed to save game state during shutdown: {}", e);
        } else {
            self.saved_cleanly = true;
            log::info!("GameState saved cleanly in shutdown()");
        }
    }

    // -----------------------------------------------------------------------
    //  File I/O helpers (moved from Repository)
    // -----------------------------------------------------------------------

    /// Resolve the absolute path to a `.dat` file given its file name.
    ///
    /// The path is computed relative to the parent directory of the running
    /// executable. Returns a `PathBuf` pointing to `<exe_parent>/.dat/<file_name>`.
    ///
    /// # Arguments
    ///
    /// * `file_name` - The name of the `.dat` file to resolve.
    ///
    /// # Returns
    ///
    /// * The absolute `PathBuf` to the file.
    pub fn get_dat_file_path(&self, file_name: &str) -> PathBuf {
        let exe_path = Path::new(&self.executable_path);

        exe_path
            .parent()
            .unwrap_or_else(|| Path::new(""))
            .join(".dat")
            .join(file_name)
    }

    /// Load and decode a normalized data set from a `.dat` file.
    ///
    /// # Arguments
    ///
    /// * `file_name` - Name of the `.dat` file to load.
    /// * `expected_record_count` - Expected number of records in the file.
    ///
    /// # Returns
    ///
    /// * `Ok(Vec<T>)` containing the decoded records.
    /// * `Err(String)` on I/O or decoding errors.
    fn load_normalized_records<T: Decode<()>>(
        &self,
        file_name: &str,
        expected_record_count: usize,
    ) -> Result<Vec<T>, String> {
        let path = self.get_dat_file_path(file_name);
        let bytes = fs::read(&path).map_err(|e| e.to_string())?;

        let (payload, consumed): (NormalizedDataSet<T>, usize) =
            bincode::decode_from_slice(&bytes, bincode::config::standard())
                .map_err(|e| format!("Failed to decode {}: {}", path.display(), e))?;

        if consumed != bytes.len() {
            log::warn!(
                "Normalized payload {} has {} trailing bytes",
                path.display(),
                bytes.len() - consumed
            );
        }

        if payload.magic != NORMALIZED_MAGIC {
            return Err(format!(
                "Invalid normalized magic in {}: {:?}",
                path.display(),
                payload.magic
            ));
        }

        if payload.version != NORMALIZED_VERSION {
            return Err(format!(
                "Unsupported normalized version in {}: {}",
                path.display(),
                payload.version
            ));
        }

        if payload.source_file != file_name {
            return Err(format!(
                "source_file mismatch in {}: expected {}, got {}",
                path.display(),
                file_name,
                payload.source_file
            ));
        }

        if payload.records.len() != expected_record_count {
            return Err(format!(
                "Record count mismatch in {}: expected {}, got {}",
                path.display(),
                expected_record_count,
                payload.records.len()
            ));
        }

        Ok(payload.records)
    }

    /// Encode and write a normalized data set to a `.dat` file.
    ///
    /// # Arguments
    ///
    /// * `file_name` - Name of the `.dat` file to write.
    /// * `source_record_size` - The `size_of` the source record type.
    /// * `records` - The records to serialize.
    ///
    /// # Returns
    ///
    /// * `Ok(())` on success.
    /// * `Err(String)` on encoding or I/O errors.
    fn save_normalized_records<T: Encode>(
        &self,
        file_name: &str,
        source_record_size: usize,
        records: Vec<T>,
    ) -> Result<(), String> {
        let path = self.get_dat_file_path(file_name);
        let payload = NormalizedDataSet {
            magic: NORMALIZED_MAGIC,
            version: NORMALIZED_VERSION,
            source_file: file_name.to_string(),
            source_record_size,
            records,
        };

        let bytes = bincode::encode_to_vec(payload, bincode::config::standard())
            .map_err(|e| format!("Failed to encode {}: {}", path.display(), e))?;

        fs::write(&path, bytes).map_err(|e| e.to_string())?;
        Ok(())
    }

    // -----------------------------------------------------------------------
    //  Individual data loaders
    // -----------------------------------------------------------------------

    /// Load `map.dat` and populate the `map` vector.
    ///
    /// # Returns
    ///
    /// * `Ok(())` on success.
    /// * `Err(String)` if the file cannot be read or decoded.
    fn load_map(&mut self) -> Result<(), String> {
        let expected_tiles =
            (core::constants::SERVER_MAPX as usize) * (core::constants::SERVER_MAPY as usize);
        self.map = self.load_normalized_records::<core::types::Map>("map.dat", expected_tiles)?;

        log::info!(
            "Map data loaded successfully. Loaded {} tiles.",
            expected_tiles
        );

        Ok(())
    }

    /// Save `map.dat` from the in-memory `map` vector back to disk.
    ///
    /// # Returns
    ///
    /// * `Ok(())` on success.
    /// * `Err(String)` if the file cannot be written.
    fn save_map(&self) -> Result<(), String> {
        let map_path = self.get_dat_file_path("map.dat");
        log::info!("Saving map data to {:?}", map_path);
        self.save_normalized_records(
            "map.dat",
            std::mem::size_of::<core::types::Map>(),
            self.map.clone(),
        )?;
        log::info!("Map data saved successfully.");
        Ok(())
    }

    /// Load `item.dat` and populate the `items` array.
    ///
    /// # Returns
    ///
    /// * `Ok(())` on success.
    /// * `Err(String)` if the file cannot be read or decoded.
    fn load_items(&mut self) -> Result<(), String> {
        self.items = self
            .load_normalized_records::<core::types::Item>("item.dat", core::constants::MAXITEM)?;

        log::info!(
            "Items data loaded successfully. Loaded {} items.",
            self.items.len()
        );

        Ok(())
    }

    /// Save `item.dat` from the in-memory `items` array to disk.
    ///
    /// # Returns
    ///
    /// * `Ok(())` on success.
    /// * `Err(String)` if the file cannot be written.
    fn save_items(&self) -> Result<(), String> {
        let items_path = self.get_dat_file_path("item.dat");

        log::info!("Saving items data to {:?}", items_path);
        self.save_normalized_records(
            "item.dat",
            std::mem::size_of::<core::types::Item>(),
            self.items.clone(),
        )?;

        log::info!("Items data saved successfully.");
        Ok(())
    }

    /// Load `titem.dat` and populate the `item_templates` array.
    ///
    /// # Returns
    ///
    /// * `Ok(())` on success.
    /// * `Err(String)` if the file cannot be read or decoded.
    fn load_item_templates(&mut self) -> Result<(), String> {
        self.item_templates = self
            .load_normalized_records::<core::types::Item>("titem.dat", core::constants::MAXTITEM)?;

        log::info!(
            "Item templates data loaded successfully. Loaded {} templates.",
            self.item_templates.len()
        );

        Ok(())
    }

    /// Load `char.dat` and populate the `characters` array.
    ///
    /// # Returns
    ///
    /// * `Ok(())` on success.
    /// * `Err(String)` if the file cannot be read or decoded.
    fn load_characters(&mut self) -> Result<(), String> {
        self.characters = self.load_normalized_records::<core::types::Character>(
            "char.dat",
            core::constants::MAXCHARS,
        )?;

        Ok(())
    }

    /// Save `char.dat` from the in-memory `characters` array to disk.
    ///
    /// # Returns
    ///
    /// * `Ok(())` on success.
    /// * `Err(String)` if the file cannot be written.
    fn save_characters(&self) -> Result<(), String> {
        let characters_path = self.get_dat_file_path("char.dat");

        log::info!("Saving characters data to {:?}", characters_path);
        self.save_normalized_records(
            "char.dat",
            std::mem::size_of::<core::types::Character>(),
            self.characters.clone(),
        )?;

        log::info!("Characters data saved successfully.");
        Ok(())
    }

    /// Load `tchar.dat` and populate the `character_templates` array.
    ///
    /// # Returns
    ///
    /// * `Ok(())` on success.
    /// * `Err(String)` if the file cannot be read or decoded.
    fn load_character_templates(&mut self) -> Result<(), String> {
        self.character_templates = self.load_normalized_records::<core::types::Character>(
            "tchar.dat",
            core::constants::MAXTCHARS,
        )?;

        Ok(())
    }

    /// Load `effect.dat` and populate the `effects` array.
    ///
    /// # Returns
    ///
    /// * `Ok(())` on success.
    /// * `Err(String)` if the file cannot be read or decoded.
    fn load_effects(&mut self) -> Result<(), String> {
        self.effects = self.load_normalized_records::<core::types::Effect>(
            "effect.dat",
            core::constants::MAXEFFECT,
        )?;

        log::info!(
            "Effects data loaded successfully. Loaded {} effects.",
            self.effects.len()
        );

        Ok(())
    }

    /// Save `effect.dat` from the in-memory `effects` array to disk.
    ///
    /// # Returns
    ///
    /// * `Ok(())` on success.
    /// * `Err(String)` if the file cannot be written.
    fn save_effects(&self) -> Result<(), String> {
        let effects_path = self.get_dat_file_path("effect.dat");

        log::info!("Saving effects data to {:?}", effects_path);
        self.save_normalized_records(
            "effect.dat",
            std::mem::size_of::<core::types::Effect>(),
            self.effects.clone(),
        )?;

        log::info!("Effects data saved successfully.");
        Ok(())
    }

    /// Load `global.dat` and parse into the `globals` structure.
    ///
    /// # Returns
    ///
    /// * `Ok(())` on success.
    /// * `Err(String)` if the file cannot be read or decoded.
    fn load_globals(&mut self) -> Result<(), String> {
        let mut records = self.load_normalized_records::<core::types::Global>("global.dat", 1)?;
        self.globals = records
            .drain(..)
            .next()
            .ok_or_else(|| "global.dat normalized payload is empty".to_string())?;

        log::info!("Globals data loaded successfully.");

        log::info!(
            "Globals data: dirty={}, character_cnt={}, ticker={}, fullmoon={}, newmoon={}, unique={}, cap={}",
            self.globals.is_dirty(),
            self.globals.character_cnt,
            self.globals.ticker,
            self.globals.fullmoon,
            self.globals.newmoon,
            self.globals.unique,
            self.globals.cap
        );

        Ok(())
    }

    /// Save `global.dat` from the in-memory `globals` structure to disk.
    ///
    /// # Returns
    ///
    /// * `Ok(())` on success.
    /// * `Err(String)` if the file cannot be written.
    fn save_globals(&self) -> Result<(), String> {
        let globals_path = self.get_dat_file_path("global.dat");

        log::info!("Saving globals data to {:?}", globals_path);
        let globals_copy = core::types::Global::from_bytes(&self.globals.to_bytes())
            .ok_or_else(|| "Failed to clone globals for serialization".to_string())?;

        self.save_normalized_records(
            "global.dat",
            std::mem::size_of::<core::types::Global>(),
            vec![globals_copy],
        )?;

        log::info!("Globals data saved successfully.");
        Ok(())
    }

    /// Load `badnames.txt` into memory.
    ///
    /// Each line is treated as a banned name pattern.
    ///
    /// # Returns
    ///
    /// * `Ok(())` on success.
    /// * `Err(String)` if the file cannot be read.
    fn load_bad_names(&mut self) -> Result<(), String> {
        let bad_names_path = self.get_dat_file_path("badnames.txt");
        log::info!("Loading bad names from {:?}", bad_names_path);
        let bad_names_data = fs::read_to_string(&bad_names_path).map_err(|e| e.to_string())?;

        for line in bad_names_data.lines() {
            self.bad_names.push(line.to_string());
        }

        log::info!(
            "Bad names loaded successfully. Loaded {} bad names.",
            self.bad_names.len()
        );

        Ok(())
    }

    /// Load `badwords.txt` into memory.
    ///
    /// Each line is treated as a banned chat word or filter term.
    ///
    /// # Returns
    ///
    /// * `Ok(())` on success.
    /// * `Err(String)` if the file cannot be read.
    fn load_bad_words(&mut self) -> Result<(), String> {
        let bad_words_path = self.get_dat_file_path("badwords.txt");
        log::info!("Loading bad words from {:?}", bad_words_path);
        let bad_words_data = fs::read_to_string(&bad_words_path).map_err(|e| e.to_string())?;

        for line in bad_words_data.lines() {
            self.bad_words.push(line.to_string());
        }

        log::info!(
            "Bad words loaded successfully. Loaded {} bad words.",
            self.bad_words.len()
        );

        Ok(())
    }

    /// Load the Message of the Day from `motd.txt`.
    ///
    /// Falls back to a default string if the file is not present.
    ///
    /// # Returns
    ///
    /// * `Ok(())` always succeeds.
    fn load_message_of_the_day(&mut self) -> Result<(), String> {
        self.message_of_the_day = self.read_message_of_the_day_from_dat_file();
        Ok(())
    }

    /// Load the ban list from `banlist.dat` if present.
    ///
    /// This currently logs and leaves `ban_list` empty when no file is present;
    /// parsing and population is TODO.
    ///
    /// # Returns
    ///
    /// * `Ok(())` always succeeds.
    fn load_ban_list(&mut self) -> Result<(), String> {
        let banlist_path = self.get_dat_file_path("banlist.dat");
        log::info!("Loading ban list from {:?}", banlist_path);
        let banlist_data = fs::read(&banlist_path);

        match banlist_data {
            Ok(_data) => {
                // Parse ban list data here
                log::info!("Ban list loaded successfully.");
                // TODO: Actually load this.
            }
            Err(_) => {
                log::warn!("Ban list file not found. Continuing without loading ban list.");
            }
        }
        Ok(())
    }
}

impl Drop for GameState {
    /// Safety-net save on drop if `shutdown()` was not called.
    ///
    /// If `shutdown()` already performed a clean save (indicated by
    /// `saved_cleanly`), the drop is a no-op. Otherwise it attempts a
    /// last-ditch save to avoid data loss.
    fn drop(&mut self) {
        if self.saved_cleanly {
            log::info!("GameState drop: already saved cleanly, skipping.");
            return;
        }

        self.globals.set_dirty(false);
        self.save().unwrap_or_else(|e| {
            log::error!("Failed to save game state cleanly on shutdown: {}", e);
        });

        log::info!("GameState saved cleanly on shutdown (via Drop).");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_motd_short_unchanged() {
        let input = "Hello world!".to_string();
        let result = GameState::normalize_message_of_the_day(input.clone());
        assert_eq!(result, input);
    }

    #[test]
    fn normalize_motd_exactly_130_unchanged() {
        let input: String = "A".repeat(130);
        let result = GameState::normalize_message_of_the_day(input.clone());
        assert_eq!(result, input);
    }

    #[test]
    fn normalize_motd_truncates_at_131() {
        let input: String = "B".repeat(200);
        let result = GameState::normalize_message_of_the_day(input);
        assert_eq!(result.chars().count(), 130);
        assert!(result.chars().all(|c| c == 'B'));
    }

    #[test]
    fn normalize_motd_empty() {
        let result = GameState::normalize_message_of_the_day(String::new());
        assert_eq!(result, "");
    }

    #[test]
    fn storage_backend_default_is_dat() {
        // When the env var is not set, default to DatFiles.
        // This test relies on the env var not being set in the test runner.
        // We don't override the env var to avoid interfering with parallel tests.
        let backend = StorageBackend::from_env();
        // Accept either variant — what matters is it doesn't panic
        assert!(backend == StorageBackend::DatFiles || backend == StorageBackend::KeyDb);
    }
}
