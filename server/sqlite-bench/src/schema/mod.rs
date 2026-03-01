//! Schema definitions and the common `BenchSchema` trait.
//!
//! Two implementations are provided:
//! - [`blob::BlobSchema`] — array fields stored as BLOBs
//! - [`normalized::NormalizedSchema`] — array fields in relational sub-tables

pub mod blob;
pub mod normalized;

use anyhow::Result;
use mag_core::types::{Character, Effect, Global, Item, Map};
use rusqlite::Connection;

/// Simulation parameters describing the population level to benchmark.
#[derive(Debug, Clone, Copy)]
pub struct PopulationParams {
    pub active_players: usize,
    pub active_npcs: usize,
    /// Total character slots (should be MAXCHARS = 8192)
    pub max_characters: usize,
    /// Total item slots (should be MAXITEM = 98304)
    pub max_items: usize,
    /// Total effect slots (should be MAXEFFECT = 4096)
    pub max_effects: usize,
    /// Active items (equipped + on ground)
    pub active_items: usize,
    /// Active effects
    pub active_effects: usize,
}

impl PopulationParams {
    /// Standard test population: 50 players, 500 NPCs.
    pub fn standard() -> Self {
        Self {
            active_players: 50,
            active_npcs: 500,
            max_characters: mag_core::constants::MAXCHARS,
            max_items: mag_core::constants::MAXITEM,
            max_effects: mag_core::constants::MAXEFFECT,
            active_items: 5_000,
            active_effects: 50,
        }
    }

    /// Stressed test population: 250 players, 2000 NPCs.
    pub fn stressed() -> Self {
        Self {
            active_players: 250,
            active_npcs: 2_000,
            max_characters: mag_core::constants::MAXCHARS,
            max_items: mag_core::constants::MAXITEM,
            max_effects: mag_core::constants::MAXEFFECT,
            active_items: 20_000,
            active_effects: 200,
        }
    }

    pub fn total_active_characters(&self) -> usize {
        self.active_players + self.active_npcs
    }
}

/// Trait implemented by each schema variant (blob vs normalized).
///
/// Each method corresponds to a class of database operation performed during
/// a game tick. Implementations should use prepared statements internally.
pub trait BenchSchema {
    /// Human-readable name for reports.
    fn name(&self) -> &'static str;

    /// Create all tables and indexes.
    fn create_tables(&self, conn: &Connection) -> Result<()>;

    /// Bulk-insert population data.
    fn populate(
        &self,
        conn: &Connection,
        characters: &[Character],
        items: &[Item],
        map: &[Map],
        effects: &[Effect],
        globals: &Global,
    ) -> Result<()>;

    // ── Tick operations ─────────────────────────────────────────────

    /// Character triage: SELECT all non-empty character ids + flags.
    /// Returns (id, used, flags) for characters where used != 0.
    fn character_triage(&self, conn: &Connection) -> Result<Vec<(u32, u8, u64)>>;

    /// Read a rectangular viewport of map tiles.
    /// `base_x`, `base_y` define the top-left corner; reads 34×34 tiles.
    fn read_viewport(
        &self,
        conn: &Connection,
        base_x: u16,
        base_y: u16,
    ) -> Result<Vec<ViewportTile>>;

    /// Read a single character by id (full row).
    fn read_character(&self, conn: &Connection, id: u32) -> Result<CharacterRow>;

    /// Read a single item by id (full row).
    fn read_item(&self, conn: &Connection, id: u32) -> Result<ItemRow>;

    /// Batch-read items by ids (e.g., worn slots lookup).
    fn read_items_batch(&self, conn: &Connection, ids: &[u32]) -> Result<Vec<ItemRow>>;

    /// Read a single map tile by linear index.
    fn read_map_tile(&self, conn: &Connection, idx: u32) -> Result<ViewportTile>;

    /// Update character combat stats (a_hp, a_end, a_mana, status).
    fn update_character_stats(
        &self,
        conn: &Connection,
        id: u32,
        a_hp: i32,
        a_end: i32,
        a_mana: i32,
        status: i16,
    ) -> Result<()>;

    /// Update map tile ownership (ch field) — used during movement.
    fn update_map_ch(&self, conn: &Connection, idx: u32, ch: u32) -> Result<()>;

    /// Update map tile light value.
    fn update_map_light(&self, conn: &Connection, idx: u32, light: i16) -> Result<()>;

    /// Batch update map light for a rectangular area (e.g., do_add_light 21×21).
    fn update_map_light_area(
        &self,
        conn: &Connection,
        center_x: u16,
        center_y: u16,
        radius: u16,
        amount: i16,
    ) -> Result<()>;

    /// Effect triage: SELECT all non-empty effect ids.
    fn effect_triage(&self, conn: &Connection) -> Result<Vec<(u32, u8, u32)>>;

    /// Update effect duration.
    fn update_effect(&self, conn: &Connection, id: u32, duration: u32) -> Result<()>;

    /// Read a row of map tiles for item_tick_expire (1024 tiles in a row).
    fn read_map_row(&self, conn: &Connection, row_y: u16) -> Result<Vec<ViewportTile>>;

    /// Read items by sequential id range (for item_tick_gc).
    fn read_items_range(
        &self,
        conn: &Connection,
        start_id: u32,
        count: u32,
    ) -> Result<Vec<ItemRow>>;

    /// Update global counters.
    fn update_globals_tick(&self, conn: &Connection, ticker: i32, uptime: i64) -> Result<()>;

    /// Read character item/worn/spell slot arrays (for delta sync).
    fn read_character_slots(&self, conn: &Connection, id: u32) -> Result<CharacterSlots>;
}

// ── Lightweight row types for benchmark results ──────────────────────

/// Subset of map tile data returned by viewport reads.
#[derive(Debug, Clone, Default)]
pub struct ViewportTile {
    pub id: u32,
    pub sprite: u16,
    pub fsprite: u16,
    pub ch: u32,
    pub to_ch: u32,
    pub it: u32,
    pub dlight: u16,
    pub light: i16,
    pub flags: u64,
}

/// Subset of character data returned by reads.
#[derive(Debug, Clone, Default)]
pub struct CharacterRow {
    pub id: u32,
    pub used: u8,
    pub flags: u64,
    pub x: i16,
    pub y: i16,
    pub sprite: u16,
    pub a_hp: i32,
    pub a_end: i32,
    pub a_mana: i32,
    pub status: i16,
    pub speed: i16,
    pub light: u8,
    pub attack_cn: u16,
    pub name: Vec<u8>,
    pub attrib: Vec<u8>,
    pub hp: Vec<u8>,
    pub mana: Vec<u8>,
    pub end: Vec<u8>,
    pub skill: Vec<u8>,
}

/// Item data returned by reads.
#[derive(Debug, Clone, Default)]
pub struct ItemRow {
    pub id: u32,
    pub used: u8,
    pub flags: u64,
    pub sprite_0: i16,
    pub status_0: u8,
    pub value: u32,
    pub name: Vec<u8>,
    pub attrib: Vec<u8>,
    pub hp: Vec<u8>,
    pub mana: Vec<u8>,
    pub end: Vec<u8>,
    pub skill: Vec<u8>,
    pub armor: Vec<u8>,
    pub weapon: Vec<u8>,
    pub light: Vec<u8>,
}

/// Character equipment slot arrays.
#[derive(Debug, Clone, Default)]
pub struct CharacterSlots {
    pub item: Vec<u8>,
    pub worn: Vec<u8>,
    pub spell: Vec<u8>,
    pub depot: Vec<u8>,
}

/// Configure a connection for maximum in-memory performance.
pub fn configure_connection(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "PRAGMA journal_mode = WAL;
         PRAGMA synchronous = OFF;
         PRAGMA cache_size = -131072;
         PRAGMA mmap_size = 268435456;
         PRAGMA temp_store = MEMORY;
         PRAGMA page_size = 4096;",
    )?;
    Ok(())
}
