//! Tick simulation: replays one full game tick's worth of database operations.
//!
//! Based on measured access patterns from `server/src/server.rs` game_tick:
//!
//! | Phase                     | Reads        | Writes     |
//! |---------------------------|-------------|------------|
//! | Character triage          | ~600 rows   | 0          |
//! | Viewport reads (50 plr)   | ~57,800 map | ~1,000 ch  |
//! | Player delta sync         | ~5,500 ch   | 0          |
//! | Active char updates       | 0           | ~550       |
//! | Movement (70 chars)       | ~140 map    | ~140 map + ~30,000 light |
//! | NPC AI reads              | ~3,000 ch   | ~1,000 items |
//! | really_update_char        | ~75 ch      | ~75 ch + ~3,000 items |
//! | do_regenerate             | ~550 ch     | ~550 ch + ~2,750 items |
//! | Effect triage + update    | ~4,100      | ~50        |
//! | Item expire (4 rows)      | ~4,096 map  | ~200 items |
//! | Item GC                   | ~256 items  | 0          |
//! | Global writes             | 0           | 1          |

use crate::schema::{BenchSchema, PopulationParams};
use anyhow::Result;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use rusqlite::Connection;

/// Run a single simulated tick against the database using the given schema.
///
/// Returns the number of operations performed (for verification).
pub fn simulate_tick(
    conn: &Connection,
    schema: &dyn BenchSchema,
    params: &PopulationParams,
    tick_number: u32,
) -> Result<TickStats> {
    let mut rng = StdRng::seed_from_u64(tick_number as u64 ^ 0xBEEF);
    let mut stats = TickStats::default();

    let mapx = mag_core::constants::SERVER_MAPX as u16;

    // ── Phase 1: Global counter update ──────────────────────────────
    schema.update_globals_tick(conn, tick_number as i32, tick_number as i64 * 27778)?;
    stats.writes += 1;

    // ── Phase 2: Character triage ───────────────────────────────────
    let active_chars = schema.character_triage(conn)?;
    stats.reads += active_chars.len();

    // ── Phase 3: Viewport reads (one per player) ───────────────────
    let num_players = params.active_players.min(active_chars.len());
    for i in 0..num_players {
        let (char_id, _, _) = active_chars[i];

        // Read the character to get position
        let ch = schema.read_character(conn, char_id)?;
        stats.reads += 1;

        // Read viewport (34×34 tiles)
        let base_x = (ch.x as u16).saturating_sub(17).min(mapx - 34);
        let base_y = (ch.y as u16).saturating_sub(17).min(mapx - 34);
        let tiles = schema.read_viewport(conn, base_x, base_y)?;
        stats.reads += tiles.len();

        // Read characters and items visible in viewport
        let visible_chars: Vec<u32> = tiles
            .iter()
            .filter(|t| t.ch != 0)
            .map(|t| t.ch)
            .take(20)
            .collect();
        for &cid in &visible_chars {
            let _ = schema.read_character(conn, cid);
            stats.reads += 1;
        }

        let visible_items: Vec<u32> = tiles
            .iter()
            .filter(|t| t.it != 0)
            .map(|t| t.it)
            .take(30)
            .collect();
        if !visible_items.is_empty() {
            let _ = schema.read_items_batch(conn, &visible_items);
            stats.reads += visible_items.len();
        }
    }

    // ── Phase 4: Player delta sync ─────────────────────────────────
    for i in 0..num_players {
        let (char_id, _, _) = active_chars[i];

        // Read character stats for comparison
        let _ = schema.read_character(conn, char_id)?;
        stats.reads += 1;

        // Read equipment slots
        let slots = schema.read_character_slots(conn, char_id)?;
        stats.reads += 1;

        // Read items in worn slots for stat display
        let worn_ids: Vec<u32> = extract_u32_ids(&slots.worn)
            .into_iter()
            .filter(|&id| id != 0)
            .take(20)
            .collect();
        if !worn_ids.is_empty() {
            let _ = schema.read_items_batch(conn, &worn_ids);
            stats.reads += worn_ids.len();
        }
    }

    // ── Phase 5: Active character stat updates (do_regenerate) ─────
    let total_active = active_chars.len();
    for i in 0..total_active {
        let (char_id, _, _) = active_chars[i];

        // Read character for regeneration
        let ch = schema.read_character(conn, char_id)?;
        stats.reads += 1;

        // Read map tile for underwater check
        if ch.x > 0 && ch.y > 0 {
            let idx = ch.x as u32 + ch.y as u32 * mapx as u32;
            let _ = schema.read_map_tile(conn, idx);
            stats.reads += 1;
        }

        // Read some spell items for duration checks (~5 per char average)
        let spell_count = rng.gen_range(0..8).min(5);
        for _ in 0..spell_count {
            let item_id = rng.gen_range(1..params.active_items as u32);
            let _ = schema.read_item(conn, item_id);
            stats.reads += 1;
        }

        // Write updated stats
        schema.update_character_stats(
            conn,
            char_id,
            ch.a_hp + rng.gen_range(0..3),
            ch.a_end + rng.gen_range(0..2),
            ch.a_mana + rng.gen_range(0..1),
            ch.status,
        )?;
        stats.writes += 1;
    }

    // ── Phase 6: Movement (subset of active chars move each tick) ──
    let movers = total_active / 8; // ~1/8 of chars move per tick (speed-gated)
    for i in 0..movers {
        let idx = (i * 8) % total_active;
        let (char_id, _, _) = active_chars[idx];

        // Read current position map tile
        let ch = schema.read_character(conn, char_id)?;
        stats.reads += 1;

        if ch.x <= 0 || ch.y <= 0 {
            continue;
        }

        let old_idx = ch.x as u32 + ch.y as u32 * mapx as u32;
        let dx: i16 = rng.gen_range(-1..=1);
        let dy: i16 = rng.gen_range(-1..=1);
        let new_x = (ch.x + dx).max(1).min(mapx as i16 - 2);
        let new_y = (ch.y + dy).max(1).min(mapx as i16 - 2);
        let new_idx = new_x as u32 + new_y as u32 * mapx as u32;

        // Read target tile (moveblock check)
        let _ = schema.read_map_tile(conn, new_idx);
        stats.reads += 1;

        // Update map: clear old, set new
        schema.update_map_ch(conn, old_idx, 0)?;
        schema.update_map_ch(conn, new_idx, char_id)?;
        stats.writes += 2;

        // Light update: 21×21 = 441 tiles (do_add_light)
        // Only do this for ~half of movers to match realistic light carriers
        if rng.gen_bool(0.5) {
            schema.update_map_light_area(conn, new_x as u16, new_y as u16, 10, 1)?;
            stats.writes += 441; // approximate tiles hit
        }
    }

    // ── Phase 7: NPC AI reads ──────────────────────────────────────
    let npc_start = num_players;
    let npc_count = params
        .active_npcs
        .min(total_active.saturating_sub(npc_start));
    for i in 0..npc_count {
        let idx = npc_start + i;
        if idx >= active_chars.len() {
            break;
        }
        let (char_id, _, _) = active_chars[idx];

        // Read character data fields (data[25], attack_cn, etc.)
        let _ = schema.read_character(conn, char_id);
        stats.reads += 1;

        // Random item reads (NPC interaction with equipment/world items)
        let item_reads = rng.gen_range(1..4);
        for _ in 0..item_reads {
            let item_id = rng.gen_range(1..params.active_items as u32);
            let _ = schema.read_item(conn, item_id);
            stats.reads += 1;
        }

        // Pathfinding map reads (for NPCs with goto_x set, ~1/3 of NPCs)
        if rng.gen_bool(0.33) {
            let path_reads = rng.gen_range(10..50);
            for _ in 0..path_reads {
                let tile_idx = rng.gen_range(0..(mapx as u32 * mapx as u32));
                let _ = schema.read_map_tile(conn, tile_idx);
                stats.reads += 1;
            }
        }
    }

    // ── Phase 8: really_update_char (~75 per tick) ─────────────────
    let update_count = (total_active / 8).min(100);
    for _ in 0..update_count {
        let idx = rng.gen_range(0..total_active);
        let (char_id, _, _) = active_chars[idx];

        // Full character read
        let _ = schema.read_character(conn, char_id)?;
        stats.reads += 1;

        // Read worn items (up to 20)
        let slots = schema.read_character_slots(conn, char_id)?;
        stats.reads += 1;

        let worn_ids: Vec<u32> = extract_u32_ids(&slots.worn)
            .into_iter()
            .filter(|&id| id != 0)
            .take(20)
            .collect();
        if !worn_ids.is_empty() {
            let _ = schema.read_items_batch(conn, &worn_ids);
            stats.reads += worn_ids.len();
        }

        // Read spell items (up to 20)
        let spell_ids: Vec<u32> = extract_u32_ids(&slots.spell)
            .into_iter()
            .filter(|&id| id != 0)
            .take(20)
            .collect();
        if !spell_ids.is_empty() {
            let _ = schema.read_items_batch(conn, &spell_ids);
            stats.reads += spell_ids.len();
        }

        // Write back updated stats
        schema.update_character_stats(
            conn,
            char_id,
            rng.gen_range(10..500),
            rng.gen_range(10..300),
            rng.gen_range(0..200),
            0,
        )?;
        stats.writes += 1;
    }

    // ── Phase 9: Effect triage + update ────────────────────────────
    let active_effects = schema.effect_triage(conn)?;
    stats.reads += active_effects.len();

    for (eff_id, _, duration) in &active_effects {
        if *duration > 0 {
            schema.update_effect(conn, *eff_id, duration - 1)?;
            stats.writes += 1;
        }
    }

    // ── Phase 10: Item tick expire (4 map rows) ────────────────────
    let expire_row = (tick_number % mag_core::constants::SERVER_MAPY as u32) as u16;
    for row_offset in 0..4 {
        let row_y = (expire_row + row_offset) % mag_core::constants::SERVER_MAPY as u16;
        let row_tiles = schema.read_map_row(conn, row_y)?;
        stats.reads += row_tiles.len();

        // For tiles with items, read the item to check expiration
        let item_tiles: Vec<u32> = row_tiles
            .iter()
            .filter(|t| t.it != 0)
            .map(|t| t.it)
            .collect();
        if !item_tiles.is_empty() {
            let _ = schema.read_items_batch(conn, &item_tiles);
            stats.reads += item_tiles.len();
        }
    }

    // ── Phase 11: Item tick GC (256 items in a round-robin window) ─
    let gc_start = (tick_number * 256) % params.max_items as u32;
    let _ = schema.read_items_range(conn, gc_start, 256)?;
    stats.reads += 256;

    Ok(stats)
}

/// Extract u32 ids from a byte slice (little-endian u32 encoding).
fn extract_u32_ids(blob: &[u8]) -> Vec<u32> {
    blob.chunks_exact(4)
        .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}

/// Statistics from a single tick simulation.
#[derive(Debug, Default, Clone)]
pub struct TickStats {
    pub reads: usize,
    pub writes: usize,
}

impl TickStats {
    pub fn total_ops(&self) -> usize {
        self.reads + self.writes
    }
}
