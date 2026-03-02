/// KeyDB-backed persistence layer for game data.
///
/// Stores all game entities (map tiles, items, characters, effects, globals,
/// templates, and text data) as bincode-encoded blobs in KeyDB, keyed per-entity.
///
/// Key schema:
/// - `game:map:{x}:{y}`     — 1,048,576 map tiles (bincode `Map`)
/// - `game:item:{idx}`       — 98,304 item slots (bincode `Item`)
/// - `game:titem:{idx}`      — 4,548 item templates (bincode `Item`)
/// - `game:char:{idx}`       — 8,192 character slots (bincode `Character`)
/// - `game:tchar:{idx}`      — 4,548 character templates (bincode `Character`)
/// - `game:effect:{idx}`     — 4,096 effects (bincode `Effect`)
/// - `game:global`           — 1 global state (bincode `Global`)
/// - `game:badnames`         — 1 key (bincode `Vec<String>`)
/// - `game:badwords`         — 1 key (bincode `Vec<String>`)
/// - `game:motd`             — 1 key (UTF-8 string)
/// - `game:meta:version`     — schema version integer
use bincode::{Decode, Encode};
use redis::{pipe, Commands, Connection};

/// Current schema version written to `game:meta:version`.
const SCHEMA_VERSION: u32 = 1;

/// Number of keys to batch in a single Redis pipeline round-trip.
const PIPELINE_BATCH_SIZE: usize = 4096;

// ---------------------------------------------------------------------------
//  Load helpers
// ---------------------------------------------------------------------------

/// Check whether game data has been seeded into KeyDB.
pub fn has_game_data(con: &mut Connection) -> Result<bool, String> {
    let exists: bool = con
        .exists("game:meta:version")
        .map_err(|e| format!("KeyDB exists check failed: {e}"))?;
    Ok(exists)
}

/// Load a single bincode-encoded entity from a key.
fn load_entity<T: Decode<()>>(con: &mut Connection, key: &str) -> Result<T, String> {
    let bytes: Vec<u8> = con
        .get(key)
        .map_err(|e| format!("KeyDB GET {key} failed: {e}"))?;

    let (val, _consumed) = bincode::decode_from_slice(&bytes, bincode::config::standard())
        .map_err(|e| format!("Decode {key}: {e}"))?;
    Ok(val)
}

/// Load a contiguous range of bincode-encoded entities from keys formatted
/// with a single integer index: `{prefix}{0..count}`.
fn load_indexed_entities<T: Decode<()>>(
    con: &mut Connection,
    prefix: &str,
    count: usize,
) -> Result<Vec<T>, String> {
    let mut results: Vec<T> = Vec::with_capacity(count);

    for batch_start in (0..count).step_by(PIPELINE_BATCH_SIZE) {
        let batch_end = (batch_start + PIPELINE_BATCH_SIZE).min(count);
        let mut pipeline = pipe();
        for idx in batch_start..batch_end {
            pipeline.cmd("GET").arg(format!("{prefix}{idx}"));
        }
        let batch_bytes: Vec<Vec<u8>> = pipeline
            .query(con)
            .map_err(|e| format!("KeyDB pipeline GET {prefix}*: {e}"))?;

        for (rel_idx, bytes) in batch_bytes.into_iter().enumerate() {
            let abs_idx = batch_start + rel_idx;
            if bytes.is_empty() {
                return Err(format!("Missing key {prefix}{abs_idx}"));
            }
            let (val, _) = bincode::decode_from_slice(&bytes, bincode::config::standard())
                .map_err(|e| format!("Decode {prefix}{abs_idx}: {e}"))?;
            results.push(val);
        }
    }

    Ok(results)
}

/// Load all map tiles from `game:map:{x}:{y}` keys.
fn load_map(con: &mut Connection) -> Result<Vec<core::types::Map>, String> {
    let map_x = core::constants::SERVER_MAPX as usize;
    let map_y = core::constants::SERVER_MAPY as usize;
    let total = map_x * map_y;
    let mut results: Vec<core::types::Map> = Vec::with_capacity(total);

    for batch_start in (0..total).step_by(PIPELINE_BATCH_SIZE) {
        let batch_end = (batch_start + PIPELINE_BATCH_SIZE).min(total);
        let mut pipeline = pipe();
        for linear in batch_start..batch_end {
            let x = linear % map_x;
            let y = linear / map_x;
            pipeline.cmd("GET").arg(format!("game:map:{x}:{y}"));
        }
        let batch_bytes: Vec<Vec<u8>> = pipeline
            .query(con)
            .map_err(|e| format!("KeyDB pipeline GET game:map: {e}"))?;

        for (rel_idx, bytes) in batch_bytes.into_iter().enumerate() {
            let abs = batch_start + rel_idx;
            if bytes.is_empty() {
                return Err(format!(
                    "Missing key game:map:{}:{}",
                    abs % map_x,
                    abs / map_x
                ));
            }
            let (val, _) = bincode::decode_from_slice(&bytes, bincode::config::standard())
                .map_err(|e| {
                    format!(
                        "Decode game:map:{}:{}: {e}",
                        abs % map_x,
                        abs / map_x
                    )
                })?;
            results.push(val);
        }
    }

    Ok(results)
}

// ---------------------------------------------------------------------------
//  Save helpers
// ---------------------------------------------------------------------------

/// Encode a value via bincode and return the byte vec.
fn encode<T: Encode>(val: &T) -> Result<Vec<u8>, String> {
    bincode::encode_to_vec(val, bincode::config::standard()).map_err(|e| format!("Encode: {e}"))
}

/// Save a contiguous slice of entities under `{prefix}{index}` keys.
fn save_indexed_entities<T: Encode>(
    con: &mut Connection,
    prefix: &str,
    entities: &[T],
) -> Result<(), String> {
    for batch_start in (0..entities.len()).step_by(PIPELINE_BATCH_SIZE) {
        let batch_end = (batch_start + PIPELINE_BATCH_SIZE).min(entities.len());
        let mut pipeline = pipe();
        for idx in batch_start..batch_end {
            let bytes = encode(&entities[idx])?;
            pipeline.cmd("SET").arg(format!("{prefix}{idx}")).arg(bytes);
        }
        pipeline
            .query::<()>(con)
            .map_err(|e| format!("KeyDB pipeline SET {prefix}*: {e}"))?;
    }
    Ok(())
}

/// Save a sub-range of entities under `{prefix}{start_index + offset}` keys.
pub fn save_indexed_entities_range<T: Encode>(
    con: &mut Connection,
    prefix: &str,
    entities: &[T],
    start_index: usize,
) -> Result<(), String> {
    for batch_start in (0..entities.len()).step_by(PIPELINE_BATCH_SIZE) {
        let batch_end = (batch_start + PIPELINE_BATCH_SIZE).min(entities.len());
        let mut pipeline = pipe();
        for rel in batch_start..batch_end {
            let abs = start_index + rel;
            let bytes = encode(&entities[rel])?;
            pipeline.cmd("SET").arg(format!("{prefix}{abs}")).arg(bytes);
        }
        pipeline
            .query::<()>(con)
            .map_err(|e| format!("KeyDB pipeline SET {prefix}*: {e}"))?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
//  Public load/save API
// ---------------------------------------------------------------------------

/// All game data loaded from KeyDB, ready to populate a `Repository`.
pub struct GameData {
    pub map: Vec<core::types::Map>,
    pub items: Vec<core::types::Item>,
    pub item_templates: Vec<core::types::Item>,
    pub characters: Vec<core::types::Character>,
    pub character_templates: Vec<core::types::Character>,
    pub effects: Vec<core::types::Effect>,
    pub globals: core::types::Global,
    pub bad_names: Vec<String>,
    pub bad_words: Vec<String>,
    pub message_of_the_day: String,
}

/// Load ALL game data from KeyDB.
///
/// Returns an error if `game:meta:version` is absent (no data has been
/// migrated yet) or if any entity cannot be loaded/decoded.
pub fn load_all(con: &mut Connection) -> Result<GameData, String> {
    if !has_game_data(con)? {
        return Err(
            "No game data found in KeyDB (game:meta:version missing). \
             Run the dat-to-keydb migration tool first."
                .to_string(),
        );
    }

    let version: u32 = con
        .get("game:meta:version")
        .map_err(|e| format!("KeyDB GET game:meta:version: {e}"))?;
    if version != SCHEMA_VERSION {
        return Err(format!(
            "Unsupported KeyDB schema version {version} (expected {SCHEMA_VERSION})"
        ));
    }

    log::info!("Loading game data from KeyDB (schema v{version})...");

    log::info!("  Loading map tiles...");
    let map = load_map(con)?;
    log::info!("  Loaded {} map tiles.", map.len());

    log::info!("  Loading items...");
    let items =
        load_indexed_entities::<core::types::Item>(con, "game:item:", core::constants::MAXITEM)?;
    log::info!("  Loaded {} items.", items.len());

    log::info!("  Loading item templates...");
    let item_templates = load_indexed_entities::<core::types::Item>(
        con,
        "game:titem:",
        core::constants::MAXTITEM,
    )?;
    log::info!("  Loaded {} item templates.", item_templates.len());

    log::info!("  Loading characters...");
    let characters = load_indexed_entities::<core::types::Character>(
        con,
        "game:char:",
        core::constants::MAXCHARS,
    )?;
    log::info!("  Loaded {} characters.", characters.len());

    log::info!("  Loading character templates...");
    let character_templates = load_indexed_entities::<core::types::Character>(
        con,
        "game:tchar:",
        core::constants::MAXTCHARS,
    )?;
    log::info!(
        "  Loaded {} character templates.",
        character_templates.len()
    );

    log::info!("  Loading effects...");
    let effects = load_indexed_entities::<core::types::Effect>(
        con,
        "game:effect:",
        core::constants::MAXEFFECT,
    )?;
    log::info!("  Loaded {} effects.", effects.len());

    log::info!("  Loading globals...");
    let globals: core::types::Global = load_entity(con, "game:global")?;
    log::info!("  Globals loaded.");

    log::info!("  Loading text data...");
    let bad_names: Vec<String> = load_entity(con, "game:badnames")?;
    let bad_words: Vec<String> = load_entity(con, "game:badwords")?;
    let message_of_the_day: String = con
        .get("game:motd")
        .map_err(|e| format!("KeyDB GET game:motd: {e}"))?;
    log::info!(
        "  Loaded {} bad names, {} bad words, motd ({} chars).",
        bad_names.len(),
        bad_words.len(),
        message_of_the_day.len()
    );

    log::info!("Game data loaded from KeyDB successfully.");

    Ok(GameData {
        map,
        items,
        item_templates,
        characters,
        character_templates,
        effects,
        globals,
        bad_names,
        bad_words,
        message_of_the_day,
    })
}

/// Save ALL game data to KeyDB (used by migration tool and clean shutdown).
pub fn save_all(
    con: &mut Connection,
    map: &[core::types::Map],
    items: &[core::types::Item],
    item_templates: &[core::types::Item],
    characters: &[core::types::Character],
    character_templates: &[core::types::Character],
    effects: &[core::types::Effect],
    globals: &core::types::Global,
    bad_names: &[String],
    bad_words: &[String],
    message_of_the_day: &str,
) -> Result<(), String> {
    log::info!("Saving all game data to KeyDB...");

    save_map(con, map)?;
    save_items(con, items)?;
    save_item_templates(con, item_templates)?;
    save_characters(con, characters)?;
    save_character_templates(con, character_templates)?;
    save_effects(con, effects)?;
    save_globals(con, globals)?;
    save_text_data(con, bad_names, bad_words, message_of_the_day)?;

    // Write schema version marker
    con.set::<_, _, ()>("game:meta:version", SCHEMA_VERSION)
        .map_err(|e| format!("KeyDB SET game:meta:version: {e}"))?;

    log::info!("All game data saved to KeyDB successfully.");
    Ok(())
}

/// Save map tiles to KeyDB.
pub fn save_map(con: &mut Connection, map: &[core::types::Map]) -> Result<(), String> {
    let map_x = core::constants::SERVER_MAPX as usize;
    let total = map.len();

    log::info!("  Saving {} map tiles...", total);
    for batch_start in (0..total).step_by(PIPELINE_BATCH_SIZE) {
        let batch_end = (batch_start + PIPELINE_BATCH_SIZE).min(total);
        let mut pipeline = pipe();
        for linear in batch_start..batch_end {
            let x = linear % map_x;
            let y = linear / map_x;
            let bytes = encode(&map[linear])?;
            pipeline.cmd("SET").arg(format!("game:map:{x}:{y}")).arg(bytes);
        }
        pipeline
            .query::<()>(con)
            .map_err(|e| format!("KeyDB pipeline SET game:map: {e}"))?;
    }
    log::info!("  Map tiles saved.");
    Ok(())
}

/// Save a range of map tiles (by linear index) to KeyDB.
pub fn save_map_range(
    con: &mut Connection,
    map: &[core::types::Map],
    start_linear: usize,
) -> Result<(), String> {
    let map_x = core::constants::SERVER_MAPX as usize;
    let total = map.len();

    for batch_start in (0..total).step_by(PIPELINE_BATCH_SIZE) {
        let batch_end = (batch_start + PIPELINE_BATCH_SIZE).min(total);
        let mut pipeline = pipe();
        for rel in batch_start..batch_end {
            let abs = start_linear + rel;
            let x = abs % map_x;
            let y = abs / map_x;
            let bytes = encode(&map[rel])?;
            pipeline.cmd("SET").arg(format!("game:map:{x}:{y}")).arg(bytes);
        }
        pipeline
            .query::<()>(con)
            .map_err(|e| format!("KeyDB pipeline SET game:map range: {e}"))?;
    }
    Ok(())
}

/// Save items to KeyDB.
pub fn save_items(con: &mut Connection, items: &[core::types::Item]) -> Result<(), String> {
    log::info!("  Saving {} items...", items.len());
    save_indexed_entities(con, "game:item:", items)?;
    log::info!("  Items saved.");
    Ok(())
}

/// Save item templates to KeyDB.
pub fn save_item_templates(
    con: &mut Connection,
    item_templates: &[core::types::Item],
) -> Result<(), String> {
    log::info!("  Saving {} item templates...", item_templates.len());
    save_indexed_entities(con, "game:titem:", item_templates)?;
    log::info!("  Item templates saved.");
    Ok(())
}

/// Save characters to KeyDB.
pub fn save_characters(
    con: &mut Connection,
    characters: &[core::types::Character],
) -> Result<(), String> {
    log::info!("  Saving {} characters...", characters.len());
    save_indexed_entities(con, "game:char:", characters)?;
    log::info!("  Characters saved.");
    Ok(())
}

/// Save character templates to KeyDB.
pub fn save_character_templates(
    con: &mut Connection,
    character_templates: &[core::types::Character],
) -> Result<(), String> {
    log::info!(
        "  Saving {} character templates...",
        character_templates.len()
    );
    save_indexed_entities(con, "game:tchar:", character_templates)?;
    log::info!("  Character templates saved.");
    Ok(())
}

/// Save effects to KeyDB.
pub fn save_effects(
    con: &mut Connection,
    effects: &[core::types::Effect],
) -> Result<(), String> {
    log::info!("  Saving {} effects...", effects.len());
    save_indexed_entities(con, "game:effect:", effects)?;
    log::info!("  Effects saved.");
    Ok(())
}

/// Save the global state to KeyDB.
pub fn save_globals(con: &mut Connection, globals: &core::types::Global) -> Result<(), String> {
    log::info!("  Saving globals...");
    let bytes = encode(globals)?;
    con.set::<_, _, ()>("game:global", bytes)
        .map_err(|e| format!("KeyDB SET game:global: {e}"))?;
    log::info!("  Globals saved.");
    Ok(())
}

/// Save text data (bad names, bad words, MOTD) to KeyDB.
pub fn save_text_data(
    con: &mut Connection,
    bad_names: &[String],
    bad_words: &[String],
    message_of_the_day: &str,
) -> Result<(), String> {
    log::info!("  Saving text data...");

    let bad_names_bytes = encode(&bad_names.to_vec())?;
    con.set::<_, _, ()>("game:badnames", bad_names_bytes)
        .map_err(|e| format!("KeyDB SET game:badnames: {e}"))?;

    let bad_words_bytes = encode(&bad_words.to_vec())?;
    con.set::<_, _, ()>("game:badwords", bad_words_bytes)
        .map_err(|e| format!("KeyDB SET game:badwords: {e}"))?;

    con.set::<_, _, ()>("game:motd", message_of_the_day)
        .map_err(|e| format!("KeyDB SET game:motd: {e}"))?;

    log::info!("  Text data saved.");
    Ok(())
}
