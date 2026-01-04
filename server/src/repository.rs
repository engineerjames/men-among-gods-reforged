use parking_lot::ReentrantMutex;
use std::cell::UnsafeCell;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::{env, fs};

// TODO: Currently this only reads data files into memory.
// So if you close down the server and restart, any changes made during runtime will be lost.
// In the future, we will want to implement saving changes back to the data files.

static REPOSITORY: OnceLock<ReentrantMutex<UnsafeCell<Repository>>> = OnceLock::new();

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
}

impl Repository {
    /// Create a new `Repository` initialized with default values.
    ///
    /// Allocates and initializes all in-memory collections with sizes based on
    /// constants (for example `MAXITEM`, `MAXCHARS`, `SERVER_MAPX` × `SERVER_MAPY`)
    /// and attempts to discover the current executable path to resolve the
    /// `.dat` directory via `get_dat_file_path`.
    fn new() -> Self {
        Self {
            // TODO: Evaluate how we can prevent accidental copying of any of these types...
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
        }
    }
    /// Load all game data from disk into memory.
    ///
    /// This calls each of the `load_*` helper methods in sequence and returns an
    /// error if any step fails. After a successful `load`, the repository
    /// contains populated `map`, `items`, `characters`, `globals`, etc.
    pub fn load(&mut self) -> Result<(), String> {
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

    /// Save all game data from memory back to disk.
    /// The bad names, words, and message of the day are not saved back as
    /// they are managed separately via text files, and are treated currently
    /// as read-only.
    pub fn save(&mut self) -> Result<(), String> {
        self.save_map()?;
        self.save_items()?;
        self.save_item_templates()?;
        self.save_characters()?;
        self.save_character_templates()?;
        self.save_effects()?;
        self.save_globals()?;
        Ok(())
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

    /// Load `map.dat` and populate the `map` vector.
    ///
    /// Validates the file size against the expected tile count and parses each
    /// `Map` entry via `core::types::Map::from_bytes`. Returns an error if the
    /// file cannot be read or its size doesn't match expectations.
    fn load_map(&mut self) -> Result<(), String> {
        let map_path = self.get_dat_file_path("map.dat");
        log::info!("Loading map data from {:?}", map_path);
        let map_data = fs::read(&map_path).map_err(|e| e.to_string())?;

        let expected_map_size = core::constants::SERVER_MAPX as usize
            * core::constants::SERVER_MAPY as usize
            * std::mem::size_of::<core::types::Map>();

        let actual_map_size = map_data.len();
        if actual_map_size != expected_map_size {
            return Err(format!(
                "Map data size mismatch: expected {}, got {}",
                expected_map_size, actual_map_size
            ));
        }

        let num_map_tiles = actual_map_size / std::mem::size_of::<core::types::Map>();

        for i in 0..num_map_tiles {
            let offset = i * std::mem::size_of::<core::types::Map>();
            let map_tile = core::types::Map::from_bytes(
                &map_data[offset..offset + std::mem::size_of::<core::types::Map>()],
            )
            .ok_or_else(|| format!("Failed to parse map tile at index {}", i))?;
            self.map[i] = map_tile;
        }

        log::info!(
            "Map data loaded successfully. Loaded {} tiles.",
            num_map_tiles
        );

        Ok(())
    }

    /// Save `map.dat` from the in-memory `map` vector back to disk.
    fn save_map(&self) -> Result<(), String> {
        let map_path = self.get_dat_file_path("map.dat");
        log::info!("Saving map data to {:?}", map_path);

        // We could definitely be more efficient here by writing directly to the file
        // from the map tiles without allocating a large buffer first.
        let mut map_data: Vec<u8> = Vec::with_capacity(
            core::constants::SERVER_MAPX as usize
                * core::constants::SERVER_MAPY as usize
                * std::mem::size_of::<core::types::Map>(),
        );

        for map_tile in &self.map {
            let tile_bytes = map_tile.to_bytes();
            map_data.extend_from_slice(&tile_bytes);
        }

        fs::write(&map_path, &map_data).map_err(|e| e.to_string())?;
        log::info!("Map data saved successfully.");
        Ok(())
    }

    /// Load `item.dat` and populate the `items` array.
    ///
    /// Verifies the file size equals `MAXITEM * size_of::<Item>()` and parses
    /// each `Item` via `core::types::Item::from_bytes`. Returns an error on
    /// read or parse failures.
    fn load_items(&mut self) -> Result<(), String> {
        let items_path = self.get_dat_file_path("item.dat");
        log::info!("Loading items data from {:?}", items_path);
        let items_data = fs::read(&items_path).map_err(|e| e.to_string())?;

        let expected_items_size =
            core::constants::MAXITEM * std::mem::size_of::<core::types::Item>();

        let actual_items_size = items_data.len();
        if actual_items_size != expected_items_size {
            return Err(format!(
                "Items data size mismatch: expected {}, got {}",
                expected_items_size, actual_items_size
            ));
        }

        let num_items = actual_items_size / std::mem::size_of::<core::types::Item>();

        for i in 0..num_items {
            let offset = i * std::mem::size_of::<core::types::Item>();
            let item = core::types::Item::from_bytes(
                &items_data[offset..offset + std::mem::size_of::<core::types::Item>()],
            )
            .ok_or_else(|| format!("Failed to parse item at index {}", i))?;
            self.items[i] = item;
        }

        log::info!(
            "Items data loaded successfully. Loaded {} items.",
            num_items
        );

        Ok(())
    }

    fn save_items(&self) -> Result<(), String> {
        let items_path = self.get_dat_file_path("item.dat");

        log::info!("Saving items data to {:?}", items_path);

        let mut items_data: Vec<u8> =
            Vec::with_capacity(core::constants::MAXITEM * std::mem::size_of::<core::types::Item>());

        for item in &self.items {
            let item_bytes = item.to_bytes();
            items_data.extend_from_slice(&item_bytes);
        }

        fs::write(&items_path, &items_data).map_err(|e| e.to_string())?;

        log::info!("Items data saved successfully.");
        Ok(())
    }

    /// Load `titem.dat` and populate the `item_templates` array.
    ///
    /// Validates length and parses each template entry. This is used when
    /// resetting or creating items from templates at runtime.
    fn load_item_templates(&mut self) -> Result<(), String> {
        let item_templates_path = self.get_dat_file_path("titem.dat");
        log::info!("Loading item templates data from {:?}", item_templates_path);
        let item_templates_data = fs::read(&item_templates_path).map_err(|e| e.to_string())?;

        let expected_item_templates_size =
            core::constants::MAXTITEM * std::mem::size_of::<core::types::Item>();

        let actual_item_templates_size = item_templates_data.len();

        if actual_item_templates_size != expected_item_templates_size {
            return Err(format!(
                "Item templates data size mismatch: expected {}, got {}",
                expected_item_templates_size, actual_item_templates_size
            ));
        }
        let num_item_templates =
            actual_item_templates_size / std::mem::size_of::<core::types::Item>();

        for i in 0..num_item_templates {
            let offset = i * std::mem::size_of::<core::types::Item>();
            let item_template = core::types::Item::from_bytes(
                &item_templates_data[offset..offset + std::mem::size_of::<core::types::Item>()],
            )
            .ok_or_else(|| format!("Failed to parse item template at index {}", i))?;
            self.item_templates[i] = item_template;
        }

        log::info!(
            "Item templates data loaded successfully. Loaded {} templates.",
            num_item_templates
        );

        Ok(())
    }

    fn save_item_templates(&self) -> Result<(), String> {
        let item_templates_path = self.get_dat_file_path("titem.dat");

        log::info!("Saving item templates data to {:?}", item_templates_path);
        let mut item_templates_data: Vec<u8> = Vec::with_capacity(
            core::constants::MAXTITEM * std::mem::size_of::<core::types::Item>(),
        );
        for item_template in &self.item_templates {
            let item_template_bytes = item_template.to_bytes();
            item_templates_data.extend_from_slice(&item_template_bytes);
        }
        fs::write(&item_templates_path, &item_templates_data).map_err(|e| e.to_string())?;
        log::info!("Item templates data saved successfully.");
        Ok(())
    }

    /// Load `char.dat` and populate the `characters` array.
    ///
    /// Validates the file size equals `MAXCHARS * size_of::<Character>()` and
    /// parses each `Character` via `core::types::Character::from_bytes`.
    fn load_characters(&mut self) -> Result<(), String> {
        let characters_path = self.get_dat_file_path("char.dat");
        log::info!("Loading characters data from {:?}", characters_path);
        let characters_data = fs::read(&characters_path).map_err(|e| e.to_string())?;

        let expected_characters_size =
            core::constants::MAXCHARS * std::mem::size_of::<core::types::Character>();
        let actual_characters_size = characters_data.len();

        if actual_characters_size != expected_characters_size {
            return Err(format!(
                "Characters data size mismatch: expected {}, got {}",
                expected_characters_size, actual_characters_size
            ));
        }

        let num_characters = actual_characters_size / std::mem::size_of::<core::types::Character>();

        for i in 0..num_characters {
            let offset = i * std::mem::size_of::<core::types::Character>();
            let character = core::types::Character::from_bytes(
                &characters_data[offset..offset + std::mem::size_of::<core::types::Character>()],
            )
            .ok_or_else(|| format!("Failed to parse character at index {}", i))?;
            self.characters[i] = character;
        }

        Ok(())
    }

    fn save_characters(&self) -> Result<(), String> {
        let characters_path = self.get_dat_file_path("char.dat");

        log::info!("Saving characters data to {:?}", characters_path);

        let mut characters_data: Vec<u8> = Vec::with_capacity(
            core::constants::MAXCHARS * std::mem::size_of::<core::types::Character>(),
        );

        for character in &self.characters {
            let character_bytes = character.to_bytes();
            characters_data.extend_from_slice(&character_bytes);
        }

        fs::write(&characters_path, &characters_data).map_err(|e| e.to_string())?;

        log::info!("Characters data saved successfully.");
        Ok(())
    }

    /// Load `tchar.dat` and populate the `character_templates` array.
    ///
    /// Validates file size and parses each template entry used for NPC spawning
    /// and template-based resets.
    fn load_character_templates(&mut self) -> Result<(), String> {
        let character_templates_path = self.get_dat_file_path("tchar.dat");
        log::info!(
            "Loading character templates data from {:?}",
            character_templates_path
        );
        let character_templates_data =
            fs::read(&character_templates_path).map_err(|e| e.to_string())?;
        let expected_character_templates_size =
            core::constants::MAXTCHARS * std::mem::size_of::<core::types::Character>();
        let actual_character_templates_size = character_templates_data.len();
        if actual_character_templates_size != expected_character_templates_size {
            return Err(format!(
                "Character templates data size mismatch: expected {}, got {}",
                expected_character_templates_size, actual_character_templates_size
            ));
        }

        let num_character_templates =
            actual_character_templates_size / std::mem::size_of::<core::types::Character>();

        for i in 0..num_character_templates {
            let offset = i * std::mem::size_of::<core::types::Character>();
            let character_template = core::types::Character::from_bytes(
                &character_templates_data
                    [offset..offset + std::mem::size_of::<core::types::Character>()],
            )
            .ok_or_else(|| format!("Failed to parse character template at index {}", i))?;
            self.character_templates[i] = character_template;
        }

        Ok(())
    }

    fn save_character_templates(&self) -> Result<(), String> {
        let character_templates_path = self.get_dat_file_path("tchar.dat");

        log::info!(
            "Saving character templates data to {:?}",
            character_templates_path
        );

        let mut character_templates_data: Vec<u8> = Vec::with_capacity(
            core::constants::MAXTCHARS * std::mem::size_of::<core::types::Character>(),
        );

        for character_template in &self.character_templates {
            let character_template_bytes = character_template.to_bytes();
            character_templates_data.extend_from_slice(&character_template_bytes);
        }

        fs::write(&character_templates_path, &character_templates_data)
            .map_err(|e| e.to_string())?;

        log::info!("Character templates data saved successfully.");
        Ok(())
    }

    /// Load `effect.dat` and populate the `effects` array.
    ///
    /// Validates file size and parses each `Effect` entry. Effects represent
    /// transient or persistent world effects used by the server.
    fn load_effects(&mut self) -> Result<(), String> {
        let effects_path = self.get_dat_file_path("effect.dat");
        log::info!("Loading effects data from {:?}", effects_path);
        let effects_data = fs::read(&effects_path).map_err(|e| e.to_string())?;

        let expected_effects_size =
            core::constants::MAXEFFECT * std::mem::size_of::<core::types::Effect>();
        let actual_effects_size = effects_data.len();

        if actual_effects_size != expected_effects_size {
            return Err(format!(
                "Effects data size mismatch: expected {}, got {}",
                expected_effects_size, actual_effects_size
            ));
        }

        let num_effects = actual_effects_size / std::mem::size_of::<core::types::Effect>();

        for i in 0..num_effects {
            let offset = i * std::mem::size_of::<core::types::Effect>();
            let effect = core::types::Effect::from_bytes(
                &effects_data[offset..offset + std::mem::size_of::<core::types::Effect>()],
            )
            .ok_or_else(|| format!("Failed to parse effect at index {}", i))?;
            self.effects[i] = effect;
        }

        log::info!(
            "Effects data loaded successfully. Loaded {} effects.",
            num_effects
        );

        Ok(())
    }

    fn save_effects(&self) -> Result<(), String> {
        let effects_path = self.get_dat_file_path("effect.dat");

        log::info!("Saving effects data to {:?}", effects_path);

        let mut effects_data: Vec<u8> = Vec::with_capacity(
            core::constants::MAXEFFECT * std::mem::size_of::<core::types::Effect>(),
        );

        for effect in &self.effects {
            let effect_bytes = effect.to_bytes();
            effects_data.extend_from_slice(&effect_bytes);
        }

        fs::write(&effects_path, &effects_data).map_err(|e| e.to_string())?;

        log::info!("Effects data saved successfully.");
        Ok(())
    }

    /// Load `global.dat` and parse into the `globals` structure.
    ///
    /// The file is expected to contain at least `size_of::<Global>()` bytes.
    /// The first bytes are parsed into `core::types::Global` using
    /// `from_bytes` and stored in `self.globals`.
    fn load_globals(&mut self) -> Result<(), String> {
        let globals_path = self.get_dat_file_path("global.dat");
        log::info!("Loading globals data from {:?}", globals_path);
        let globals_data = fs::read(&globals_path).map_err(|e| e.to_string())?;

        let expected_size = std::mem::size_of::<core::types::Global>();
        if globals_data.len() < expected_size {
            return Err(format!(
                "Globals data size mismatch: expected at least {}, got {}",
                expected_size,
                globals_data.len()
            ));
        }

        let slice = &globals_data[..expected_size];
        self.globals = core::types::Global::from_bytes(slice)
            .ok_or_else(|| "Failed to parse globals data".to_string())?;

        log::info!("Globals data loaded successfully.");

        Ok(())
    }

    fn save_globals(&self) -> Result<(), String> {
        let globals_path = self.get_dat_file_path("global.dat");

        log::info!("Saving globals data to {:?}", globals_path);

        let globals_data = self.globals.to_bytes();

        fs::write(&globals_path, &globals_data).map_err(|e| e.to_string())?;

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
        let mut repo = Repository::new();
        repo.load()?;
        REPOSITORY
            .set(ReentrantMutex::new(UnsafeCell::new(repo)))
            .map_err(|_| "Repository already initialized".to_string())?;
        Ok(())
    }

    // Access helpers for the global repository protected by a reentrant mutex
    /// Internal helper: acquire the global repository for read-only access.
    ///
    /// Locks the global `REPOSITORY` reentrant mutex and passes a shared
    /// reference to the provided closure `f`. This guarantees safe concurrent
    /// read access through the closure while the lock is held.
    fn with_repo<F, R>(f: F) -> R
    where
        F: FnOnce(&Repository) -> R,
    {
        let lock = REPOSITORY.get().expect("Repository not initialized");
        let guard = lock.lock();
        let inner: &UnsafeCell<Repository> = &guard;
        // SAFETY: We only create a shared reference here while holding the mutex; there are no
        // concurrent &mut aliases when the mutex is locked.
        let repo_ref: &Repository = unsafe { &*inner.get() };
        f(repo_ref)
    }

    /// Internal helper: acquire the global repository for mutable access.
    ///
    /// Locks the global `REPOSITORY` reentrant mutex and passes a unique
    /// mutable reference to the provided closure `f`. Reentrancy allows nested
    /// calls from the same thread.
    fn with_repo_mut<F, R>(f: F) -> R
    where
        F: FnOnce(&mut Repository) -> R,
    {
        let lock = REPOSITORY.get().expect("Repository not initialized");
        let guard = lock.lock();
        let inner: &UnsafeCell<Repository> = &guard;
        // SAFETY: We create a unique mutable reference from the raw pointer held inside UnsafeCell.
        // This is safe because the ReentrantMutex provides mutual exclusion across threads and we
        // only call this function while holding the mutex. Reentrancy ensures nested calls succeed
        // on the same thread.
        let repo_mut: &mut Repository = unsafe { &mut *inner.get() };
        f(repo_mut)
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

    // TODO: Not sure if we need this yet...
    #[allow(dead_code)]
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

    /// Execute `f` with a mutable slice of item templates.
    ///
    /// Allows modifying or resetting item templates in a synchronized way.
    pub fn with_item_templates_mut<F, R>(f: F) -> R
    where
        F: FnOnce(&mut [core::types::Item]) -> R,
    {
        Self::with_repo_mut(|repo| f(&mut repo.item_templates[..]))
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

    /// Execute `f` with a mutable slice of character templates.
    ///
    /// Use this to change templates and mark respawns in a synchronized way.
    pub fn with_character_templates_mut<F, R>(f: F) -> R
    where
        F: FnOnce(&mut [core::types::Character]) -> R,
    {
        Self::with_repo_mut(|repo| f(&mut repo.character_templates[..]))
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
}

impl Drop for Repository {
    /// Called when the `Repository` is dropped (on server shutdown).
    ///
    /// Marks repository data as clean and performs any final logging; further
    /// graceful shutdown steps may be added here in the future.
    fn drop(&mut self) {
        self.globals.set_dirty(false);
        self.save().unwrap_or_else(|e| {
            log::error!("Failed to save repository cleanly on shutdown: {}", e);
        });
    }
}
