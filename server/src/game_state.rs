use crate::path_finding::PathFinder;
use crate::pathfinding_service::PathfindingService;
use crate::types::server_player::ServerPlayer;
use core::constants::{CharacterFlags, USE_EMPTY};
use core::talent_trees::total_points_spent;
use std::collections::HashMap;

/// Runtime state for the Harakim Element Switching passive.
///
/// This is intentionally separate from spell items. Spell items may be used to
/// show a client icon, but the server tracks the actual last-cast element here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ElementSwitchState {
    /// Last elemental spell type cast by the character.
    pub last_element: u32,
    /// Server tick at which this transient memory expires.
    pub expires_at_tick: i32,
}

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
///
/// All persistence is backed by KeyDB.  Use the `world-snapshot` binary to
/// export or import the complete world state as a portable `.wsnap` file.
use server::keydb::connection as keydb;
use server::keydb::store;

const AREA_NOTIFY_BUCKET_SIZE: usize = 16;
const AREA_NOTIFY_BUCKET_COLS: usize =
    core::constants::SERVER_MAPX as usize / AREA_NOTIFY_BUCKET_SIZE;
const AREA_NOTIFY_BUCKET_ROWS: usize =
    core::constants::SERVER_MAPY as usize / AREA_NOTIFY_BUCKET_SIZE;
const AREA_NOTIFY_BUCKET_COUNT: usize = AREA_NOTIFY_BUCKET_COLS * AREA_NOTIFY_BUCKET_ROWS;

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

    // -- Network player slots --
    /// Per-connection player data (sockets, buffers, client-side caches).
    pub players: Vec<ServerPlayer>,

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
    pub _visi: [i8; core::constants::VISI_BUFFER_LEN],
    /// Primary visibility buffer.
    pub visi: [i8; core::constants::VISI_BUFFER_LEN],
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

    /// Runtime-only landed primary-hit counters for talent passives.
    pub talent_primary_hit_counts: Vec<u8>,
    /// Runtime-only last-element state for the Harakim Element Switching passive.
    pub element_switch_states: HashMap<usize, ElementSwitchState>,
    /// Runtime-only next tick for non-urgent NPC self-spell evaluation.
    pub npc_next_self_spell_eval_tick: Vec<i32>,
    /// Runtime-only next tick for NPC combat spell evaluation.
    pub npc_next_combat_spell_eval_tick: Vec<i32>,

    // -- Labyrinth 9 --
    pub lab9: crate::lab9::Labyrinth9,

    // -- Pathfinding --
    /// A* pathfinder with pre-allocated node/visited buffers.
    pub pathfinder: PathFinder,
    /// Optional worker-thread pathfinding service.
    pub pathfinding_service: Option<PathfindingService>,

    /// Runtime-only spatial index for area notifications.
    area_notify_buckets: Vec<Vec<u32>>,

    // -- Persistence (private) --
    /// Set to `true` until loaded runtime data needs a final persistence pass.
    saved_cleanly: bool,

    // -- Runtime mode flags --
    /// When `true`, playtest-only commands such as `/equip` are available to all players.
    ///
    /// Enabled by passing `--playtest` on the command line.  Has no effect on
    /// normal gameplay behaviour outside of commands explicitly gated on this flag.
    pub playtest_mode: bool,

    /// God-mode activation password loaded from the `MAG_GOD_PASSWORD` environment variable.
    ///
    /// Any player who types this string in chat is immediately granted all god-level flags.
    /// The server refuses to start if this field is empty (i.e. the env var was not provided).
    pub god_password: String,
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

    /// Create a new `GameState` initialized with default values.
    ///
    /// Allocates and initializes all in-memory collections with sizes based on
    /// constants (for example `MAXITEM`, `MAXCHARS`, `SERVER_MAPX` × `SERVER_MAPY`).
    ///
    /// # Returns
    ///
    /// * A freshly allocated `GameState` with all fields at their defaults.
    pub(crate) fn new() -> Self {
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
            players: (0..core::constants::MAXPLAYER)
                .map(|_| ServerPlayer::new())
                .collect(),
            last_population_reset_tick: 0,
            ice_cloak_clock: 0,
            item_tick_gc_off: 0,
            item_tick_gc_count: 0,
            item_tick_expire_counter: 0,
            // Visibility state
            _visi: [0; core::constants::VISI_BUFFER_LEN],
            visi: [0; core::constants::VISI_BUFFER_LEN],
            vis_is_global: true,
            see_miss: 0,
            see_hit: 0,
            ox: 0,
            oy: 0,
            is_monster: false,
            penta_needed: 5,
            talent_primary_hit_counts: vec![0; core::constants::MAXCHARS],
            element_switch_states: HashMap::new(),
            npc_next_self_spell_eval_tick: vec![0; core::constants::MAXCHARS],
            npc_next_combat_spell_eval_tick: vec![0; core::constants::MAXCHARS],
            // Labyrinth 9
            lab9: crate::lab9::Labyrinth9::new(),
            // Pathfinding
            pathfinder: PathFinder::new(),
            pathfinding_service: None,
            area_notify_buckets: vec![Vec::new(); AREA_NOTIFY_BUCKET_COUNT],
            // Persistence is enabled only after KeyDB data loads successfully.
            saved_cleanly: true,
            // Runtime mode flags
            playtest_mode: false,
            god_password: String::new(),
        }
    }

    /// Removes expired Element Switching state entries.
    ///
    /// # Arguments
    ///
    /// * `current_tick` - Current server tick used as the expiry threshold.
    pub(crate) fn tick_element_switch_states(&mut self, current_tick: i32) {
        self.element_switch_states
            .retain(|_, state| state.expires_at_tick > current_tick);
    }

    fn area_notify_bucket_index_for_map_index(map_index: usize) -> Option<usize> {
        let width = core::constants::SERVER_MAPX as usize;
        let height = core::constants::SERVER_MAPY as usize;
        if map_index >= width * height {
            return None;
        }
        let x = map_index % width;
        let y = map_index / width;
        Some(
            (x / AREA_NOTIFY_BUCKET_SIZE) + (y / AREA_NOTIFY_BUCKET_SIZE) * AREA_NOTIFY_BUCKET_COLS,
        )
    }

    fn remove_area_notify_bucket_entry(&mut self, map_index: usize, character_id: u32) {
        if character_id == 0 {
            return;
        }
        let Some(bucket_index) = Self::area_notify_bucket_index_for_map_index(map_index) else {
            return;
        };
        if let Some(pos) = self.area_notify_buckets[bucket_index]
            .iter()
            .position(|&entry| entry == character_id)
        {
            self.area_notify_buckets[bucket_index].swap_remove(pos);
        }
    }

    fn add_area_notify_bucket_entry(&mut self, map_index: usize, character_id: u32) {
        if character_id == 0 {
            return;
        }
        let Some(bucket_index) = Self::area_notify_bucket_index_for_map_index(map_index) else {
            return;
        };
        if !self.area_notify_buckets[bucket_index].contains(&character_id) {
            self.area_notify_buckets[bucket_index].push(character_id);
        }
    }

    /// Set a map tile's active character and keep the area-notify bucket grid in sync.
    ///
    /// # Arguments
    ///
    /// * `map_index` - Linear map tile index.
    /// * `character_id` - Character id to store in `map[map_index].ch`, or `0` to clear it.
    pub(crate) fn set_map_ch(&mut self, map_index: usize, character_id: u32) {
        if map_index >= self.map.len() {
            return;
        }
        let old_character_id = self.map[map_index].ch;
        if old_character_id == character_id {
            return;
        }
        self.remove_area_notify_bucket_entry(map_index, old_character_id);
        self.map[map_index].ch = character_id;
        self.add_area_notify_bucket_entry(map_index, character_id);
    }

    /// Clear a map tile's active character if it matches the expected id.
    ///
    /// # Arguments
    ///
    /// * `map_index` - Linear map tile index.
    /// * `character_id` - Character id expected in `map[map_index].ch`.
    pub(crate) fn clear_map_ch_if(&mut self, map_index: usize, character_id: u32) {
        if map_index < self.map.len() && self.map[map_index].ch == character_id {
            self.set_map_ch(map_index, 0);
        }
    }

    /// Rebuild the area-notify bucket grid from current map occupancy.
    pub(crate) fn rebuild_area_notify_buckets(&mut self) {
        for bucket in &mut self.area_notify_buckets {
            bucket.clear();
        }
        for map_index in 0..self.map.len() {
            let character_id = self.map[map_index].ch;
            self.add_area_notify_bucket_entry(map_index, character_id);
        }
    }

    /// Return active character candidates inside an inclusive coordinate rectangle.
    ///
    /// # Arguments
    ///
    /// * `min_x` - Inclusive minimum x coordinate.
    /// * `max_x` - Inclusive maximum x coordinate.
    /// * `min_y` - Inclusive minimum y coordinate.
    /// * `max_y` - Inclusive maximum y coordinate.
    ///
    /// # Returns
    ///
    /// * Character ids currently indexed in buckets overlapping the rectangle.
    pub(crate) fn area_notify_candidates(
        &self,
        min_x: usize,
        max_x: usize,
        min_y: usize,
        max_y: usize,
    ) -> Vec<u32> {
        let min_bucket_x = min_x / AREA_NOTIFY_BUCKET_SIZE;
        let max_bucket_x = max_x / AREA_NOTIFY_BUCKET_SIZE;
        let min_bucket_y = min_y / AREA_NOTIFY_BUCKET_SIZE;
        let max_bucket_y = max_y / AREA_NOTIFY_BUCKET_SIZE;

        let mut candidates = Vec::new();
        for bucket_y in min_bucket_y..=max_bucket_y.min(AREA_NOTIFY_BUCKET_ROWS - 1) {
            for bucket_x in min_bucket_x..=max_bucket_x.min(AREA_NOTIFY_BUCKET_COLS - 1) {
                let bucket_index = bucket_x + bucket_y * AREA_NOTIFY_BUCKET_COLS;
                for &character_id in &self.area_notify_buckets[bucket_index] {
                    let character_index = character_id as usize;
                    if character_index >= self.characters.len() {
                        continue;
                    }
                    let character = &self.characters[character_index];
                    let x = character.x;
                    let y = character.y;
                    if x < 0 || y < 0 {
                        continue;
                    }
                    let x = x as usize;
                    let y = y as usize;
                    if x < min_x || x > max_x || y < min_y || y > max_y {
                        continue;
                    }
                    let map_index = x + y * core::constants::SERVER_MAPX as usize;
                    if map_index < self.map.len() && self.map[map_index].ch == character_id {
                        candidates.push(character_id);
                    }
                }
            }
        }
        candidates
    }

    /// Initialize a new `GameState` by loading all data from KeyDB.
    ///
    /// Allocates the struct, connects to KeyDB, and loads all world data.
    /// Returns the fully populated game state or an error if loading fails.
    ///
    /// Requires KeyDB to have been seeded with `world-snapshot import` before
    /// the server starts.
    ///
    /// # Returns
    ///
    /// * `Ok(GameState)` on success.
    /// * `Err(String)` if the KeyDB connection or data load fails.
    pub fn initialize() -> Result<GameState, String> {
        let mut gs = Self::new();
        gs.load_from_keydb()?;
        gs.pathfinding_service = PathfindingService::spawn_from_env();
        if gs.pathfinding_service.is_some() {
            log::info!("Async pathfinding enabled via MAG_ASYNC_PATHFINDING.");
        }
        gs.saved_cleanly = false;
        Ok(gs)
    }

    /// Fetch the latest MOTD from KeyDB for login-time display.
    ///
    /// Re-reads `game:motd` on each call so that operators can update the
    /// message without restarting the server.  Falls back to the
    /// boot-cached value if the KeyDB read fails.
    ///
    /// # Returns
    ///
    /// * The current message of the day string.
    pub fn latest_message_of_the_day(&self) -> String {
        match keydb::load_message_of_the_day() {
            Ok(motd) => Self::normalize_message_of_the_day(motd),
            Err(error) => {
                log::warn!(
                    "Falling back to cached MOTD after KeyDB read failure: {}",
                    error
                );
                self.message_of_the_day.clone()
            }
        }
    }

    /// Load all data from KeyDB.
    ///
    /// # Returns
    ///
    /// * `Ok(())` on success.
    /// * `Err(String)` if the KeyDB connection or load fails.
    fn load_from_keydb(&mut self) -> Result<(), String> {
        let mut con = keydb::connect()?;
        let data = store::load_all(&mut con)?;

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

        self.rebuild_area_notify_buckets();

        self.mark_talent_characters_for_stat_recompute();

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

    /// Mark loaded characters with learned talents for one stat recompute.
    ///
    /// Talent effects are derived from the persisted talent bitset. Setting the
    /// update flag after loading ensures a clean server restart recalculates
    /// those bonuses from current base stats even if the saved total fields are
    /// stale. This intentionally does not set `SaveMe`; the recompute itself
    /// will decide whether normal runtime state needs persistence later.
    fn mark_talent_characters_for_stat_recompute(&mut self) {
        for character in &mut self.characters {
            if character.used == USE_EMPTY {
                continue;
            }
            if total_points_spent(&character.future1) > 0 {
                character.flags |= CharacterFlags::Update.bits();
            }
        }
    }

    /// Save mutable runtime game data to KeyDB.
    ///
    /// Bad names, bad words, and MOTD are externally-managed content and are
    /// intentionally excluded from runtime saves.  Use `world-snapshot import`
    /// to update them.
    ///
    /// # Returns
    ///
    /// * `Ok(())` on success.
    /// * `Err(String)` if the KeyDB connection or save fails.
    pub fn save(&mut self) -> Result<(), String> {
        self.save_to_keydb()
    }

    /// Save all mutable runtime game data to KeyDB.
    ///
    /// # Returns
    ///
    /// * `Ok(())` on success.
    /// * `Err(String)` if the KeyDB connection or save fails.
    fn save_to_keydb(&self) -> Result<(), String> {
        let mut con = keydb::connect()?;
        store::save_runtime_data(
            &mut con,
            &self.map,
            &self.items,
            &self.characters,
            &self.effects,
            &self.globals,
        )
    }

    /// Perform a clean shutdown of the game state by clearing the dirty flag
    /// and saving all data to KeyDB.
    pub fn shutdown(&mut self) {
        if let Some(service) = &mut self.pathfinding_service {
            service.shutdown();
        }
        self.pathfinding_service = None;

        self.globals.set_dirty(false);
        if let Err(e) = self.save() {
            log::error!("Failed to save game state during shutdown: {}", e);
        } else {
            self.saved_cleanly = true;
            log::info!("GameState saved cleanly in shutdown()");
        }
    }
}

impl Drop for GameState {
    /// Safety-net save on drop if `shutdown()` was not called.
    ///
    /// If persistence has not been activated yet or `shutdown()` already
    /// performed a clean save, the drop is a no-op. Otherwise it attempts a
    /// last-ditch save to avoid data loss.
    fn drop(&mut self) {
        if self.saved_cleanly {
            log::info!("GameState drop: no pending persistence save, skipping.");
            return;
        }

        self.globals.set_dirty(false);
        if let Err(e) = self.save() {
            log::error!("Failed to save game state on drop: {}", e);
        } else {
            self.saved_cleanly = true;
            log::info!("GameState saved cleanly on drop.");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_motd_short_unchanged() {
        let input = "Hello world!".to_owned();
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
    fn area_notify_buckets_track_set_move_and_clear() {
        crate::test_helpers::with_test_gs(|gs| {
            let cn = 42_u32;
            let first = 10 + 10 * core::constants::SERVER_MAPX as usize;
            let second = 30 + 10 * core::constants::SERVER_MAPX as usize;

            gs.characters[cn as usize].x = 10;
            gs.characters[cn as usize].y = 10;
            gs.set_map_ch(first, cn);
            assert_eq!(gs.area_notify_candidates(1, 20, 1, 20), vec![cn]);

            gs.characters[cn as usize].x = 30;
            gs.characters[cn as usize].y = 10;
            gs.set_map_ch(first, 0);
            gs.set_map_ch(second, cn);
            assert!(gs.area_notify_candidates(1, 20, 1, 20).is_empty());
            assert_eq!(gs.area_notify_candidates(21, 40, 1, 20), vec![cn]);

            gs.clear_map_ch_if(second, cn);
            assert!(gs.area_notify_candidates(21, 40, 1, 20).is_empty());
        });
    }

    #[test]
    fn rebuild_area_notify_buckets_indexes_existing_map_occupancy() {
        crate::test_helpers::with_test_gs(|gs| {
            let cn = 43_u32;
            let map_index = 18 + 18 * core::constants::SERVER_MAPX as usize;
            gs.characters[cn as usize].x = 18;
            gs.characters[cn as usize].y = 18;
            gs.map[map_index].ch = cn;

            assert!(gs.area_notify_candidates(1, 30, 1, 30).is_empty());
            gs.rebuild_area_notify_buckets();
            assert_eq!(gs.area_notify_candidates(1, 30, 1, 30), vec![cn]);
        });
    }
}
