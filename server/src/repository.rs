use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::{env, fs};

use bincode::{Decode, Encode};

use crate::keydb;
use crate::keydb_store;
use crate::single_thread_cell::SingleThreadCell;

static REPOSITORY: OnceLock<SingleThreadCell<Repository>> = OnceLock::new();

const NORMALIZED_MAGIC: [u8; 4] = *b"MAG2";
const NORMALIZED_VERSION: u32 = 1;

/// Persistence backend used by the repository for loading and saving data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageBackend {
    /// Load/save from `.dat` files on disk (legacy).
    DatFiles,
    /// Load/save from KeyDB.
    KeyDb,
}

impl StorageBackend {
    /// Determine the storage backend from the `MAG_STORAGE_BACKEND` env var.
    /// Values: `keydb` → [`KeyDb`], anything else (or unset) → [`DatFiles`].
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

#[derive(Debug, Encode, Decode)]
struct NormalizedDataSet<T> {
    magic: [u8; 4],
    version: u32,
    source_file: String,
    source_record_size: usize,
    records: Vec<T>,
}

/// The in-memory data repository used by the server.
///
/// Holds maps, items, characters, globals, and other game data loaded from the
/// `.dat` files. Accessed via thread-safe accessors via the public `with_*`
/// helper methods which acquire a reentrant mutex and provide closures access
/// to the internal storage.
pub struct Repository {
    map: Vec<core::types::Map>,
    items: Vec<core::types::Item>,
    item_templates: Vec<core::types::Item>,
    characters: Vec<core::types::Character>,
    character_templates: Vec<core::types::Character>,
    effects: Vec<core::types::Effect>,
    globals: core::types::Global,
    see_map: Vec<core::types::SeeMap>,
    bad_names: Vec<String>,
    bad_words: Vec<String>,
    message_of_the_day: String,
    ban_list: Vec<core::types::Ban>,
    executable_path: String,
    last_population_reset_tick: u32,
    ice_cloak_clock: u32,
    item_tick_gc_off: u32,
    item_tick_gc_count: u32,
    item_tick_expire_counter: u32,
    /// Which storage backend this repository was loaded from.
    storage_backend: StorageBackend,
    /// Set to true once a clean save has been performed (avoids double-save
    /// from both `shutdown()` and `Drop`).
    saved_cleanly: bool,
}

impl Repository {
    /// Create a new `Repository` initialized with default values.
    ///
    /// Allocates and initializes all in-memory collections with sizes based on
    /// constants (for example `MAXITEM`, `MAXCHARS`, `SERVER_MAPX` × `SERVER_MAPY`)
    /// and attempts to discover the current executable path to resolve the
    /// `.dat` directory via `get_dat_file_path`.
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
            executable_path: match env::current_exe() {
                Ok(exe_path) => exe_path.to_string_lossy().to_string(),
                Err(e) => {
                    log::error!("Failed to get executable path: {}", e);
                    String::new()
                }
            },
            last_population_reset_tick: 0,
            ice_cloak_clock: 0,
            item_tick_gc_off: 0,
            item_tick_gc_count: 0,
            item_tick_expire_counter: 0,
            storage_backend: backend,
            saved_cleanly: false,
        }
    }
    /// Load all game data from disk into memory.
    ///
    /// This calls each of the `load_*` helper methods in sequence and returns an
    /// error if any step fails. After a successful `load`, the repository
    /// contains populated `map`, `items`, `characters`, `globals`, etc.
    fn load(&mut self) -> Result<(), String> {
        match self.storage_backend {
            StorageBackend::DatFiles => self.load_from_dat_files(),
            StorageBackend::KeyDb => self.load_from_keydb(),
        }
    }

    /// Load all data from `.dat` files on disk (legacy path).
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

    /// Save all game data from memory back to disk.
    /// The bad names, words, and message of the day are not saved back as
    /// they are managed separately via text files, and are treated currently
    /// as read-only.
    fn save(&mut self) -> Result<(), String> {
        match self.storage_backend {
            StorageBackend::DatFiles => self.save_to_dat_files(),
            StorageBackend::KeyDb => self.save_to_keydb(),
        }
    }

    /// Save all data to `.dat` files on disk (legacy path).
    fn save_to_dat_files(&self) -> Result<(), String> {
        self.save_map()?;
        self.save_items()?;
        self.save_item_templates()?;
        self.save_characters()?;
        self.save_character_templates()?;
        self.save_effects()?;
        self.save_globals()?;
        Ok(())
    }

    /// Save all data to KeyDB.
    fn save_to_keydb(&self) -> Result<(), String> {
        let mut con = keydb::connect()?;
        keydb_store::save_all(
            &mut con,
            &self.map,
            &self.items,
            &self.item_templates,
            &self.characters,
            &self.character_templates,
            &self.effects,
            &self.globals,
            &self.bad_names,
            &self.bad_words,
            &self.message_of_the_day,
        )
    }

    /// Perform a clean shutdown of the repository by saving all data to the
    /// configured storage backend.
    pub fn shutdown() {
        if REPOSITORY.get().is_none() {
            log::warn!("Repository.shutdown called but repository not initialized.");
            return;
        }

        Self::with_repo_mut(|repo| {
            repo.globals.set_dirty(false);
            if let Err(e) = repo.save() {
                log::error!("Failed to save repository during shutdown: {}", e);
            } else {
                repo.saved_cleanly = true;
                log::info!("Repository saved cleanly in shutdown()");
            }
        });
    }

    /// Return the storage backend in use.
    pub fn storage_backend() -> StorageBackend {
        Self::with_repo(|repo| repo.storage_backend)
    }

    /// Resolve the absolute path to a `.dat` file given its file name.
    ///
    /// The path is computed relative to the parent directory of the running
    /// executable (the `executable_path` stored on construction). Returns a
    /// `PathBuf` pointing to `<exe_parent>/.dat/<file_name>`.
    fn get_dat_file_path(&self, file_name: &str) -> PathBuf {
        let exe_path = Path::new(&self.executable_path);

        let full_path = exe_path
            .parent()
            .unwrap_or_else(|| Path::new(""))
            .join(".dat")
            .join(file_name);

        full_path
    }

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

    /// Load `map.dat` and populate the `map` vector.
    ///
    /// Validates the file size against the expected tile count and parses each
    /// `Map` entry via `core::types::Map::from_bytes`. Returns an error if the
    /// file cannot be read or its size doesn't match expectations.
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
    /// Verifies the file size equals `MAXITEM * size_of::<Item>()` and parses
    /// each `Item` via `core::types::Item::from_bytes`. Returns an error on
    /// read or parse failures.
    fn load_items(&mut self) -> Result<(), String> {
        self.items = self
            .load_normalized_records::<core::types::Item>("item.dat", core::constants::MAXITEM)?;

        log::info!(
            "Items data loaded successfully. Loaded {} items.",
            self.items.len()
        );

        Ok(())
    }

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
    /// Validates length and parses each template entry. This is used when
    /// resetting or creating items from templates at runtime.
    fn load_item_templates(&mut self) -> Result<(), String> {
        self.item_templates = self
            .load_normalized_records::<core::types::Item>("titem.dat", core::constants::MAXTITEM)?;

        log::info!(
            "Item templates data loaded successfully. Loaded {} templates.",
            self.item_templates.len()
        );

        Ok(())
    }

    fn save_item_templates(&self) -> Result<(), String> {
        let item_templates_path = self.get_dat_file_path("titem.dat");

        log::info!("Saving item templates data to {:?}", item_templates_path);
        self.save_normalized_records(
            "titem.dat",
            std::mem::size_of::<core::types::Item>(),
            self.item_templates.clone(),
        )?;
        log::info!("Item templates data saved successfully.");
        Ok(())
    }

    /// Load `char.dat` and populate the `characters` array.
    ///
    /// Validates the file size equals `MAXCHARS * size_of::<Character>()` and
    /// parses each `Character` via `core::types::Character::from_bytes`.
    fn load_characters(&mut self) -> Result<(), String> {
        self.characters = self.load_normalized_records::<core::types::Character>(
            "char.dat",
            core::constants::MAXCHARS,
        )?;

        Ok(())
    }

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
    /// Validates file size and parses each template entry used for NPC spawning
    /// and template-based resets.
    fn load_character_templates(&mut self) -> Result<(), String> {
        self.character_templates = self.load_normalized_records::<core::types::Character>(
            "tchar.dat",
            core::constants::MAXTCHARS,
        )?;

        Ok(())
    }

    fn save_character_templates(&self) -> Result<(), String> {
        let character_templates_path = self.get_dat_file_path("tchar.dat");

        log::info!(
            "Saving character templates data to {:?}",
            character_templates_path
        );

        self.save_normalized_records(
            "tchar.dat",
            std::mem::size_of::<core::types::Character>(),
            self.character_templates.clone(),
        )?;

        log::info!("Character templates data saved successfully.");
        Ok(())
    }

    /// Load `effect.dat` and populate the `effects` array.
    ///
    /// Validates file size and parses each `Effect` entry. Effects represent
    /// transient or persistent world effects used by the server.
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
    /// The file is expected to contain at least `size_of::<Global>()` bytes.
    /// The first bytes are parsed into `core::types::Global` using
    /// `from_bytes` and stored in `self.globals`.
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
    /// Each line is treated as a banned name pattern and stored in
    /// `self.bad_names` for name validation checks.
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
    /// Each line is treated as a banned chat word or filter term and stored
    /// in `self.bad_words` for chat filtering.
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
    /// Falls back to a default string if the file is not present. The MOTD is
    /// truncated to 130 characters if it is too long to avoid client issues.
    fn load_message_of_the_day(&mut self) -> Result<(), String> {
        let motd_path = self.get_dat_file_path("motd.txt");
        log::info!("Loading message of the day from {:?}", motd_path);
        let motd_data =
            fs::read_to_string(&motd_path).unwrap_or("Live long and prosper!".to_string());
        self.message_of_the_day = motd_data;

        if self.message_of_the_day.len() > 130 {
            log::warn!(
                "Message of the day is too long ({} characters). Truncating to 130 characters.",
                self.message_of_the_day.len()
            );
            self.message_of_the_day = self.message_of_the_day[..130].to_string();
        }

        Ok(())
    }

    /// Load the ban list from `banlist.dat` if present.
    ///
    /// This currently logs and leaves `ban_list` empty when no file is present;
    /// parsing and population of `ban_list` is TODO.
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

    // Initialize the global repository
    /// Initialize the global `Repository` singleton.
    ///
    /// Loads data from disk and stores the `Repository` inside the global
    /// `REPOSITORY` OnceLock guarded by a `ReentrantMutex`. Returns an error if
    /// initialization or loading fails, or if the repository was already set.
    pub fn initialize() -> Result<(), String> {
        let backend = StorageBackend::from_env();
        log::info!("Repository storage backend: {:?}", backend);

        let mut repo = Repository::new(backend);
        repo.load()?;
        REPOSITORY
            .set(SingleThreadCell::new(repo))
            .map_err(|_| "Repository already initialized".to_string())?;
        Ok(())
    }

    // Access helpers for the global repository protected by a reentrant mutex
    /// Internal helper: acquire the global repository for read-only access.
    ///
    /// Locks the global `REPOSITORY` reentrant mutex and passes a shared
    /// reference to the provided closure `f`. This guarantees safe concurrent
    /// read access through the closure while the lock is held.
    pub fn with_repo<F, R>(f: F) -> R
    where
        F: FnOnce(&Repository) -> R,
    {
        let repo = REPOSITORY.get().expect("Repository not initialized");
        repo.with(f)
    }

    /// Internal helper: acquire the global repository for mutable access.
    ///
    /// Locks the global `REPOSITORY` reentrant mutex and passes a unique
    /// mutable reference to the provided closure `f`. Reentrancy allows nested
    /// calls from the same thread.
    pub fn with_repo_mut<F, R>(f: F) -> R
    where
        F: FnOnce(&mut Repository) -> R,
    {
        let repo = REPOSITORY.get().expect("Repository not initialized");
        repo.with_mut(f)
    }

    // Static accessor methods for read-only access
    /// Execute `f` with a read-only slice of the map tiles.
    ///
    /// The closure `f` is called while holding the repository lock, guaranteeing
    /// safe concurrent access to the map data.
    pub fn with_map<F, R>(f: F) -> R
    where
        F: FnOnce(&[core::types::Map]) -> R,
    {
        Self::with_repo(|repo| f(&repo.map[..]))
    }

    /// Execute `f` with a read-only slice of all item instances.
    ///
    /// Use this to safely read item fields while the repository lock is held.
    pub fn with_items<F, R>(f: F) -> R
    where
        F: FnOnce(&[core::types::Item]) -> R,
    {
        Self::with_repo(|repo| f(&repo.items[..]))
    }

    /// Execute `f` with a read-only slice of item templates.
    ///
    /// Item templates are static data used to create or reset item instances.
    pub fn with_item_templates<F, R>(f: F) -> R
    where
        F: FnOnce(&[core::types::Item]) -> R,
    {
        Self::with_repo(|repo| f(&repo.item_templates[..]))
    }

    /// Execute `f` with a read-only slice of characters.
    ///
    /// Characters include both player and NPC instances. Read access is
    /// synchronized via the repository mutex.
    pub fn with_characters<F, R>(f: F) -> R
    where
        F: FnOnce(&[core::types::Character]) -> R,
    {
        Self::with_repo(|repo| f(&repo.characters[..]))
    }

    /// Execute `f` with a read-only slice of effects.
    ///
    /// Effects are transient world-state objects processed during ticks.
    pub fn with_effects<F, R>(f: F) -> R
    where
        F: FnOnce(&[core::types::Effect]) -> R,
    {
        Self::with_repo(|repo| f(&repo.effects[..]))
    }

    /// Execute `f` with a read-only reference to the global server state.
    ///
    /// Use this to query global counters and configuration loaded from
    /// `global.dat`.
    pub fn with_globals<F, R>(f: F) -> R
    where
        F: FnOnce(&core::types::Global) -> R,
    {
        Self::with_repo(|repo| f(&repo.globals))
    }

    /// Execute `f` with a read-only slice of `SeeMap` data used for visibility
    /// calculations. This function is currently unused but kept for completeness.
    pub fn with_see_map<F, R>(f: F) -> R
    where
        F: FnOnce(&[core::types::SeeMap]) -> R,
    {
        Self::with_repo(|repo| f(&repo.see_map[..]))
    }

    /// Execute `f` with a mutable slice of the map tiles.
    ///
    /// Provides exclusive mutable access to the map while the repository lock
    /// is held; use this to perform map updates safely.
    pub fn with_map_mut<F, R>(f: F) -> R
    where
        F: FnOnce(&mut [core::types::Map]) -> R,
    {
        Self::with_repo_mut(|repo| f(&mut repo.map[..]))
    }

    /// Execute `f` with a mutable slice of item instances.
    ///
    /// Use this to modify items (create, reset, update) while holding the
    /// repository mutex to ensure consistency.
    pub fn with_items_mut<F, R>(f: F) -> R
    where
        F: FnOnce(&mut [core::types::Item]) -> R,
    {
        Self::with_repo_mut(|repo| f(&mut repo.items[..]))
    }

    /// Execute `f` with a read-only slice of character templates.
    ///
    /// Character templates are used to spawn NPCs and to reset template
    /// instances.
    pub fn with_character_templates<F, R>(f: F) -> R
    where
        F: FnOnce(&[core::types::Character]) -> R,
    {
        Self::with_repo(|repo| f(&repo.character_templates[..]))
    }

    /// Execute `f` with a mutable slice of character instances.
    ///
    /// This allows adding/removing/modifying characters while holding the
    /// repository lock to maintain consistency.
    pub fn with_characters_mut<F, R>(f: F) -> R
    where
        F: FnOnce(&mut [core::types::Character]) -> R,
    {
        Self::with_repo_mut(|repo| f(&mut repo.characters[..]))
    }

    /// Execute `f` with a mutable slice of effects.
    ///
    /// Use this to create or clear effects during world ticks.
    pub fn with_effects_mut<F, R>(f: F) -> R
    where
        F: FnOnce(&mut [core::types::Effect]) -> R,
    {
        Self::with_repo_mut(|repo| f(&mut repo.effects[..]))
    }

    /// Execute `f` with a mutable reference to the global server state.
    ///
    /// Use this to increment counters, set flags, or update global settings
    /// in a thread-safe manner.
    pub fn with_globals_mut<F, R>(f: F) -> R
    where
        F: FnOnce(&mut core::types::Global) -> R,
    {
        Self::with_repo_mut(|repo| f(&mut repo.globals))
    }

    /// Execute `f` with a mutable slice of `SeeMap` data.
    ///
    /// This allows updates to visibility state; kept for completeness.
    pub fn with_see_map_mut<F, R>(f: F) -> R
    where
        F: FnOnce(&mut [core::types::SeeMap]) -> R,
    {
        Self::with_repo_mut(|repo| f(&mut repo.see_map[..]))
    }

    /// Execute `f` with a read-only reference to the ban list.
    ///
    /// The ban list is optionally loaded from `banlist.dat`; parsing is still
    /// TODO. Use this to check bans in a thread-safe manner.
    pub fn with_ban_list<F, R>(f: F) -> R
    where
        F: FnOnce(&Vec<core::types::Ban>) -> R,
    {
        Self::with_repo(|repo| f(&repo.ban_list))
    }

    /// Execute `f` with a mutable reference to the ban list.
    ///
    /// Allows adding or removing ban entries in a synchronized manner.
    pub fn with_ban_list_mut<F, R>(f: F) -> R
    where
        F: FnOnce(&mut Vec<core::types::Ban>) -> R,
    {
        Self::with_repo_mut(|repo| f(&mut repo.ban_list))
    }

    /// Get the last population reset tick.
    ///
    /// Used to track when the last population reset occurred.
    pub fn get_last_population_reset_tick() -> u32 {
        Self::with_repo(|repo| repo.last_population_reset_tick)
    }

    /// Set the last population reset tick.
    ///
    /// Used to track when the last population reset occurred.
    pub fn set_last_population_reset_tick(tick: u32) {
        Self::with_repo_mut(|repo| {
            repo.last_population_reset_tick = tick;
        });
    }

    /// Get the current ice cloak clock value.
    ///
    /// Used for timing ice cloak effects (aging more slowly in inventory vs. worn).
    pub fn get_ice_cloak_clock() -> u32 {
        Self::with_repo(|repo| repo.ice_cloak_clock)
    }

    /// Set the ice cloak clock value.
    ///
    /// Used for timing ice cloak effects (aging more slowly in inventory vs. worn).
    pub fn set_ice_cloak_clock(clock: u32) {
        Self::with_repo_mut(|repo| {
            repo.ice_cloak_clock = clock;
        });
    }

    // Getters and setters for various counters that are used (statically) during
    // the execution of the C implementation of item garbage collection and expiration.
    pub fn get_item_tick_gc_off() -> u32 {
        Self::with_repo(|repo| repo.item_tick_gc_off)
    }

    pub fn set_item_tick_gc_off(tick: u32) {
        Self::with_repo_mut(|repo| {
            repo.item_tick_gc_off = tick;
        });
    }

    pub fn get_item_tick_gc_count() -> u32 {
        Self::with_repo(|repo| repo.item_tick_gc_count)
    }

    pub fn set_item_tick_gc_count(count: u32) {
        Self::with_repo_mut(|repo| {
            repo.item_tick_gc_count = count;
        });
    }

    pub fn get_item_tick_expire_counter() -> u32 {
        Self::with_repo(|repo| repo.item_tick_expire_counter)
    }

    pub fn set_item_tick_expire_counter(counter: u32) {
        Self::with_repo_mut(|repo| {
            repo.item_tick_expire_counter = counter;
        });
    }
}

impl Drop for Repository {
    /// Called when the `Repository` is dropped (on server shutdown).
    ///
    /// Acts as a safety net: if `shutdown()` already performed a clean save
    /// (indicated by `saved_cleanly`), the drop is a no-op. Otherwise it
    /// attempts a last-ditch save to avoid data loss.
    fn drop(&mut self) {
        if self.saved_cleanly {
            log::info!("Repository drop: already saved cleanly, skipping.");
            return;
        }

        self.globals.set_dirty(false);
        self.save().unwrap_or_else(|e| {
            log::error!("Failed to save repository cleanly on shutdown: {}", e);
        });

        log::info!("Repository saved cleanly on shutdown (via Drop).");
    }
}
