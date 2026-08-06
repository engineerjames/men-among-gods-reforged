//! `world_snapshot` — Export, import, verify, and sanitize world snapshots.
//!
//! This binary provides the supported snapshot import/export workflow for
//! seeding and backing up KeyDB world data, plus a one-way sanitizer that
//! removes all player characters from a snapshot without touching KeyDB:
//!
//! ```text
//! # Export the current KeyDB world state to a file
//! world_snapshot export --output world_seed.wsnap
//!
//! # Import a snapshot into KeyDB (seed a fresh instance)
//! world_snapshot import --input world_seed.wsnap [--skip-if-seeded] [--force]
//!
//! # Inspect a snapshot without touching KeyDB
//! world_snapshot verify --input world_seed.wsnap
//!
//! # Remove every player character from a snapshot (does not touch KeyDB)
//! world_snapshot clear-players --input world_seed.wsnap --output world_cleared.wsnap
//! ```
//!
//! The resulting `.wsnap` file is a single `bincode`-encoded
//! [`WorldSnapshot`](server::keydb::snapshot::WorldSnapshot) that can be committed
//! to version control, copied between environments, or edited by external
//! tooling.

use std::collections::HashSet;
use std::env;
use std::path::{Path, PathBuf};
use std::process;
use std::time::Instant;

use redis::Commands;

use server::keydb::connection as keydb;
use server::keydb::snapshot::{SNAPSHOT_SCHEMA_VERSION, WorldSnapshot};
use server::keydb::store;

// ---------------------------------------------------------------------------
//  CLI arg parsing
// ---------------------------------------------------------------------------

/// Parsed sub-command and options.
enum Command {
    Export {
        output: PathBuf,
    },
    Import {
        input: PathBuf,
        skip_if_seeded: bool,
        force: bool,
    },
    Verify {
        input: PathBuf,
    },
    ClearPlayers {
        input: PathBuf,
        output: PathBuf,
    },
}

/// Parse `std::env::args` into a [`Command`].
///
/// Prints usage and exits with code 1 on any error.
///
/// # Returns
///
/// * The parsed [`Command`].
fn parse_args() -> Command {
    let args: Vec<String> = env::args().collect();
    let prog = args.first().map(|s| s.as_str()).unwrap_or("world_snapshot");

    let usage = format!(
        "Usage:\n\
         \n  {prog} export        --output <file.wsnap>\
         \n  {prog} import        --input  <file.wsnap> [--skip-if-seeded] [--force]\
         \n  {prog} verify        --input  <file.wsnap>\
         \n  {prog} clear-players --input  <file.wsnap> --output <file.wsnap>\
         \n\nEnv vars:\
         \n  MAG_KEYDB_URL   — KeyDB connection URL (default: redis://127.0.0.1:5556/)\
         \n  KEYDB_PASSWORD  — password, if MAG_KEYDB_URL is not set\
         \n"
    );

    let sub = args.get(1).map(|s| s.as_str()).unwrap_or("");

    match sub {
        "export" => {
            let output = flag_value(&args, "--output").unwrap_or_else(|| {
                eprintln!("Error: --output <file> is required for 'export'.\n\n{usage}");
                process::exit(1);
            });
            Command::Export {
                output: PathBuf::from(output),
            }
        }
        "import" => {
            let input = flag_value(&args, "--input").unwrap_or_else(|| {
                eprintln!("Error: --input <file> is required for 'import'.\n\n{usage}");
                process::exit(1);
            });
            let skip_if_seeded = args.iter().any(|a| a == "--skip-if-seeded");
            let force = args.iter().any(|a| a == "--force");
            Command::Import {
                input: PathBuf::from(input),
                skip_if_seeded,
                force,
            }
        }
        "verify" => {
            let input = flag_value(&args, "--input").unwrap_or_else(|| {
                eprintln!("Error: --input <file> is required for 'verify'.\n\n{usage}");
                process::exit(1);
            });
            Command::Verify {
                input: PathBuf::from(input),
            }
        }
        "clear-players" => {
            let input = flag_value(&args, "--input").unwrap_or_else(|| {
                eprintln!("Error: --input <file> is required for 'clear-players'.\n\n{usage}");
                process::exit(1);
            });
            let output = flag_value(&args, "--output").unwrap_or_else(|| {
                eprintln!("Error: --output <file> is required for 'clear-players'.\n\n{usage}");
                process::exit(1);
            });
            Command::ClearPlayers {
                input: PathBuf::from(input),
                output: PathBuf::from(output),
            }
        }
        _ => {
            eprintln!("Error: unknown sub-command {:?}.\n\n{usage}", sub);
            process::exit(1);
        }
    }
}

/// Return the value of a `--flag value` pair from an args slice.
///
/// # Arguments
///
/// * `args`  - The full argument list.
/// * `flag`  - The flag name to search for (e.g. `"--output"`).
///
/// # Returns
///
/// * `Some(value)` if the flag was found and has a following argument.
/// * `None` otherwise.
fn flag_value<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
    args.windows(2)
        .find(|pair| pair[0] == flag)
        .map(|pair| pair[1].as_str())
}

// ---------------------------------------------------------------------------
//  Sub-command implementations
// ---------------------------------------------------------------------------

/// Remove every player character from a snapshot and write the result to a new file.
///
/// This subcommand does **not** connect to KeyDB. It reads the input `.wsnap`,
/// deletes every character with the `Player` flag, destroys the items they
/// carried/wore/held/depot-ed, zeros any map tiles that referenced them, and
/// writes the sanitized snapshot to `output`.
///
/// # Arguments
///
/// * `input`  - Source `.wsnap` file.
/// * `output` - Destination `.wsnap` file.
fn cmd_clear_players(input: &Path, output: &Path) {
    println!("Reading snapshot from {}...", input.display());
    let start = Instant::now();

    let mut snapshot = WorldSnapshot::from_file(input).unwrap_or_else(|e| {
        eprintln!("Failed to read snapshot: {e}");
        process::exit(1);
    });
    println!("{}", snapshot.summary());

    println!("Clearing player characters...");
    let stats = clear_player_characters(&mut snapshot);

    println!("Writing sanitized snapshot to {}...", output.display());
    snapshot.to_file(output).unwrap_or_else(|e| {
        eprintln!("Failed to write snapshot: {e}");
        process::exit(1);
    });

    let size_bytes = std::fs::metadata(output).map(|m| m.len()).unwrap_or(0);
    println!(
        "\nClear-players complete in {:.2?}.\n  Removed players: {}\n  Cleared map tiles: {}\n  Destroyed items: {}\n  Output size: {:.2} MiB.",
        start.elapsed(),
        stats.removed_players,
        stats.cleared_map_refs,
        stats.destroyed_items,
        size_bytes as f64 / (1024.0 * 1024.0),
    );
}

/// Statistics returned by [`clear_player_characters`].
struct ClearStats {
    /// Number of player character slots reset to `USE_EMPTY`.
    removed_players: usize,
    /// Number of map tiles whose `ch`/`to_ch` referenced a removed player.
    cleared_map_refs: usize,
    /// Number of item slots destroyed because they belonged to removed players.
    destroyed_items: usize,
}

/// Delete every player character from `snapshot` and clean up dangling references.
///
/// A character is considered a player when [`Character::is_player`] returns true.
/// For each such character:
///
/// * The name is logged.
/// * All items in `item[]`, `worn[]`, `spell[]`, `depot[]`, and `citem` are
///   marked as empty.
/// * The character slot is reset to `Character::default()` with `used = USE_EMPTY`.
///
/// After all player slots are cleared, the map is swept and any `ch`/`to_ch`
/// values that point to a removed player index are zeroed. A final item safety
/// sweep also marks any item whose `carried` field references a removed player
/// as empty.
///
/// # Arguments
///
/// * `snapshot` - The world snapshot to mutate in place.
///
/// # Returns
///
/// * A [`ClearStats`] summary of the work performed.
fn clear_player_characters(snapshot: &mut WorldSnapshot) -> ClearStats {
    let mut removed = HashSet::new();
    let mut destroyed_items = 0;

    for cn in 1..core::constants::MAXCHARS {
        if !snapshot.characters[cn].is_player() {
            continue;
        }

        let name = snapshot.characters[cn].get_name();
        if !name.is_empty() {
            println!("  Removing player character #{}: '{}'", cn, name);
        }

        destroyed_items += destroy_character_items(snapshot, cn);
        snapshot.characters[cn] = core::types::v2::Character::default();
        snapshot.characters[cn].used = core::constants::USE_EMPTY;
        removed.insert(cn);
    }

    let cleared_map_refs = sweep_map_references(snapshot, &removed);
    destroyed_items += sweep_orphaned_carried_items(snapshot, &removed);

    ClearStats {
        removed_players: removed.len(),
        cleared_map_refs,
        destroyed_items,
    }
}

/// Destroy all items belonging to a single character.
///
/// Mirrors the item-cleanup logic used at runtime, operating directly on the
/// snapshot vectors so that inventory/worn/spell/depot/held items do not
/// dangle after the character slot is deleted.
///
/// # Arguments
///
/// * `snapshot` - The snapshot whose item array will be mutated.
/// * `char_id`  - Index of the character whose items are to be destroyed.
fn destroy_character_items(snapshot: &mut WorldSnapshot, char_id: usize) -> usize {
    let ch = &snapshot.characters[char_id];
    let mut destroyed = 0;

    for slot in 0..40 {
        let item_id = ch.item[slot] as usize;
        if core::types::v2::Item::is_sane_item(item_id) {
            snapshot.items[item_id] = core::types::v2::Item::default();
            snapshot.items[item_id].used = core::constants::USE_EMPTY;
            destroyed += 1;
        }
    }

    for slot in 0..20 {
        let worn_id = ch.worn[slot] as usize;
        if core::types::v2::Item::is_sane_item(worn_id) {
            snapshot.items[worn_id] = core::types::v2::Item::default();
            snapshot.items[worn_id].used = core::constants::USE_EMPTY;
            destroyed += 1;
        }

        let spell_id = ch.spell[slot] as usize;
        if core::types::v2::Item::is_sane_item(spell_id) {
            snapshot.items[spell_id] = core::types::v2::Item::default();
            snapshot.items[spell_id].used = core::constants::USE_EMPTY;
            destroyed += 1;
        }
    }

    let citem_id = ch.citem as usize;
    if core::types::v2::Item::is_sane_item(citem_id) {
        snapshot.items[citem_id] = core::types::v2::Item::default();
        snapshot.items[citem_id].used = core::constants::USE_EMPTY;
        destroyed += 1;
    }

    if ch.is_player() {
        for slot in 0..62 {
            let depot_id = ch.depot[slot] as usize;
            if core::types::v2::Item::is_sane_item(depot_id) {
                snapshot.items[depot_id] = core::types::v2::Item::default();
                snapshot.items[depot_id].used = core::constants::USE_EMPTY;
                destroyed += 1;
            }
        }
    }

    destroyed
}

/// Zero `Map.ch`/`Map.to_ch` references that point to removed player indices.
///
/// # Arguments
///
/// * `snapshot`    - The snapshot whose map array will be mutated.
/// * `removed_cns` - Set of character indices that were deleted.
///
/// # Returns
///
/// * The number of tile references that were cleared.
fn sweep_map_references(snapshot: &mut WorldSnapshot, removed_cns: &HashSet<usize>) -> usize {
    let mut cleared = 0;

    for tile in &mut snapshot.map {
        let ch = tile.ch as usize;
        if ch != 0 && removed_cns.contains(&ch) {
            tile.ch = 0;
            cleared += 1;
        }

        let to_ch = tile.to_ch as usize;
        if to_ch != 0 && removed_cns.contains(&to_ch) {
            tile.to_ch = 0;
            cleared += 1;
        }
    }

    cleared
}

/// Mark any item whose `carried` field references a removed player as empty.
///
/// This is a safety net for items that may have been linked to the player
/// without appearing in the standard character arrays.
///
/// # Arguments
///
/// * `snapshot`    - The snapshot whose item array will be mutated.
/// * `removed_cns` - Set of character indices that were deleted.
///
/// # Returns
///
/// * The number of additional item slots that were destroyed.
fn sweep_orphaned_carried_items(
    snapshot: &mut WorldSnapshot,
    removed_cns: &HashSet<usize>,
) -> usize {
    let mut destroyed = 0;

    for item_id in 1..core::constants::MAXITEM {
        let carried = snapshot.items[item_id].carried as usize;
        if carried != 0 && removed_cns.contains(&carried) {
            snapshot.items[item_id] = core::types::v2::Item::default();
            snapshot.items[item_id].used = core::constants::USE_EMPTY;
            destroyed += 1;
        }
    }

    destroyed
}

/// Export all game data from KeyDB to a snapshot file.
///
/// Connects to KeyDB, calls [`store::load_all`] to read all entities,
/// wraps them in a [`WorldSnapshot`], and writes the result to `output`.
///
/// # Arguments
///
/// * `output` - Destination path for the `.wsnap` file.
fn cmd_export(output: &PathBuf) {
    println!("Connecting to KeyDB...");
    let mut con = keydb::connect().unwrap_or_else(|e| {
        eprintln!("KeyDB connection failed: {e}");
        process::exit(1);
    });

    println!("Loading game data from KeyDB...");
    let start = Instant::now();
    let data = store::load_all(&mut con).unwrap_or_else(|e| {
        eprintln!("Failed to load game data: {e}");
        process::exit(1);
    });

    println!("Building snapshot...");
    let snapshot = WorldSnapshot::new(
        data.map,
        data.items,
        data.item_templates,
        data.characters,
        data.character_templates,
        data.effects,
        data.globals,
        data.bad_names,
        data.bad_words,
        data.message_of_the_day,
    );

    println!("{}", snapshot.summary());
    println!("Writing snapshot to {}...", output.display());
    snapshot.to_file(output).unwrap_or_else(|e| {
        eprintln!("Failed to write snapshot: {e}");
        process::exit(1);
    });

    let size_bytes = std::fs::metadata(output).map(|m| m.len()).unwrap_or(0);

    println!(
        "\nExport complete in {:.2?}. File size: {:.2} MiB.",
        start.elapsed(),
        size_bytes as f64 / (1024.0 * 1024.0),
    );
}

/// Import a snapshot file into KeyDB.
///
/// Reads the `.wsnap` file, validates its magic and schema version, then
/// writes all entities to KeyDB using the `keydb_store` pipeline helpers.
/// Respects `--skip-if-seeded` and `--force` flags.
///
/// # Arguments
///
/// * `input`          - Path to the source `.wsnap` file.
/// * `skip_if_seeded` - Exit successfully without writing if data already exists.
/// * `force`          - Overwrite existing data without prompting.
fn cmd_import(input: &Path, skip_if_seeded: bool, force: bool) {
    println!("Reading snapshot from {}...", input.display());
    let start = Instant::now();

    let snapshot = WorldSnapshot::from_file(input).unwrap_or_else(|e| {
        eprintln!("Failed to read snapshot: {e}");
        process::exit(1);
    });
    println!("{}", snapshot.summary());

    println!("Connecting to KeyDB...");
    let mut con = keydb::connect().unwrap_or_else(|e| {
        eprintln!("KeyDB connection failed: {e}");
        process::exit(1);
    });

    // Seeded-data guard. This performs a semantic map check, not just a
    // schema-marker check, so corrupt all-default map volumes get reseeded.
    let exists = store::has_valid_game_data(&mut con).unwrap_or(false);
    if exists && !force {
        if skip_if_seeded {
            println!(
                "Valid game data already exists in KeyDB. Skipping import (--skip-if-seeded)."
            );
            return;
        }
        eprintln!(
            "Error: valid game data already exists in KeyDB.\n\
             Use --force to overwrite."
        );
        process::exit(1);
    }

    println!("\nWriting game data to KeyDB...");

    store::save_map(&mut con, &snapshot.map).unwrap_or_else(|e| {
        eprintln!("Failed to save map: {e}");
        process::exit(1);
    });

    store::save_items(&mut con, &snapshot.items).unwrap_or_else(|e| {
        eprintln!("Failed to save items: {e}");
        process::exit(1);
    });

    store::save_item_templates(&mut con, &snapshot.item_templates).unwrap_or_else(|e| {
        eprintln!("Failed to save item templates: {e}");
        process::exit(1);
    });

    store::save_characters(&mut con, &snapshot.characters).unwrap_or_else(|e| {
        eprintln!("Failed to save characters: {e}");
        process::exit(1);
    });

    store::save_character_templates(&mut con, &snapshot.character_templates).unwrap_or_else(|e| {
        eprintln!("Failed to save character templates: {e}");
        process::exit(1);
    });

    store::save_effects(&mut con, &snapshot.effects).unwrap_or_else(|e| {
        eprintln!("Failed to save effects: {e}");
        process::exit(1);
    });

    store::save_globals(&mut con, &snapshot.globals).unwrap_or_else(|e| {
        eprintln!("Failed to save globals: {e}");
        process::exit(1);
    });

    store::save_text_data(
        &mut con,
        &snapshot.bad_names,
        &snapshot.bad_words,
        &snapshot.motd,
    )
    .unwrap_or_else(|e| {
        eprintln!("Failed to save text data: {e}");
        process::exit(1);
    });

    // Schema version marker (must match store::SCHEMA_VERSION).
    // We write it last so the server startup check only succeeds after all
    // data is committed.
    con.set::<_, _, ()>("game:meta:version", SNAPSHOT_SCHEMA_VERSION)
        .unwrap_or_else(|e| {
            eprintln!("Failed to set game:meta:version: {e}");
            process::exit(1);
        });

    let total_keys = snapshot.map.len()
        + snapshot.items.len()
        + snapshot.item_templates.len()
        + snapshot.characters.len()
        + snapshot.character_templates.len()
        + snapshot.effects.len()
        + 4  // globals, badnames, badwords, motd
        + 1; // meta:version

    println!(
        "\nImport complete in {:.2?}. Total keys written: {}.",
        start.elapsed(),
        total_keys,
    );
}

/// Verify a snapshot file without touching KeyDB.
///
/// Decodes the file, validates magic and schema version, prints a summary,
/// and exits 0 on success.
///
/// # Arguments
///
/// * `input` - Path to the `.wsnap` file to verify.
fn cmd_verify(input: &Path) {
    println!("Verifying snapshot {}...", input.display());

    let snapshot = WorldSnapshot::from_file(input).unwrap_or_else(|e| {
        eprintln!("Verification failed: {e}");
        process::exit(1);
    });

    println!("{}", snapshot.summary());

    // Cross-check record counts against compiled constants.
    let expected_map =
        core::constants::SERVER_MAPX as usize * core::constants::SERVER_MAPY as usize;
    let warnings: Vec<String> = [
        (snapshot.map.len(), expected_map, "map tiles"),
        (snapshot.items.len(), core::constants::MAXITEM, "items"),
        (
            snapshot.item_templates.len(),
            core::constants::MAXTITEM,
            "item templates",
        ),
        (
            snapshot.characters.len(),
            core::constants::MAXCHARS,
            "characters",
        ),
        (
            snapshot.character_templates.len(),
            core::constants::MAXTCHARS,
            "character templates",
        ),
        (
            snapshot.effects.len(),
            core::constants::MAXEFFECT,
            "effects",
        ),
    ]
    .iter()
    .filter_map(|(got, expected, label)| {
        if got != expected {
            Some(format!(
                "  WARNING: {label} count {got} != expected {expected}"
            ))
        } else {
            None
        }
    })
    .collect();

    if warnings.is_empty() {
        println!("All record counts match compiled constants.");
        println!("Snapshot OK.");
    } else {
        for w in &warnings {
            eprintln!("{w}");
        }
        eprintln!("Snapshot has mismatched record counts (see above).");
        process::exit(1);
    }
}

// ---------------------------------------------------------------------------
//  Entry point
// ---------------------------------------------------------------------------

fn main() {
    let cmd = parse_args();
    match cmd {
        Command::Export { output } => cmd_export(&output),
        Command::Import {
            input,
            skip_if_seeded,
            force,
        } => cmd_import(&input, skip_if_seeded, force),
        Command::Verify { input } => cmd_verify(&input),
        Command::ClearPlayers { input, output } => cmd_clear_players(&input, &output),
    }
}

// ---------------------------------------------------------------------------
//  Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a minimal snapshot with one player character and one item they carry.
    ///
    /// The character sits at slot index `1` and has the `Player` flag set.  An
    /// item is placed at slot index `1`, marked as used, and referenced from the
    /// character's inventory.  A map tile also references the character via
    /// `ch`.
    fn minimal_snapshot_with_player() -> WorldSnapshot {
        let mut snapshot = WorldSnapshot::new(
            vec![
                core::types::v2::Map::default();
                core::constants::SERVER_MAPX as usize * core::constants::SERVER_MAPY as usize
            ],
            vec![core::types::v2::Item::default(); core::constants::MAXITEM],
            vec![core::types::v2::Item::default(); core::constants::MAXTITEM],
            vec![core::types::v2::Character::default(); core::constants::MAXCHARS],
            vec![core::types::v2::Character::default(); core::constants::MAXTCHARS],
            vec![core::types::v2::Effect::default(); core::constants::MAXEFFECT],
            core::types::v2::Global::default(),
            Vec::new(),
            Vec::new(),
            String::new(),
        );

        // Place a used item in slot 1, mark it as carried by the player, and
        // reference it from the character's inventory.
        snapshot.items[1].used = core::constants::USE_ACTIVE;
        snapshot.items[1].carried = 1;
        snapshot.characters[1].item[0] = 1;

        // Mark slot 1 as a player character.
        snapshot.characters[1].used = core::constants::USE_ACTIVE;
        snapshot.characters[1].flags = core::constants::CharacterFlags::Player.bits();
        core::string_operations::write_ascii_into_fixed(
            &mut snapshot.characters[1].name,
            "TestPlayer",
        );

        // Point a map tile at the character.
        snapshot.map[100].ch = 1;

        snapshot
    }

    /// `clear_player_characters` deletes the player slot, destroys their items,
    /// and clears map references.
    #[test]
    fn clear_players_removes_player_character() {
        let mut snapshot = minimal_snapshot_with_player();
        let stats = clear_player_characters(&mut snapshot);

        assert_eq!(stats.removed_players, 1);
        assert_eq!(snapshot.characters[1].used, core::constants::USE_EMPTY);
        assert_eq!(
            snapshot.characters[1].flags & core::constants::CharacterFlags::Player.bits(),
            0
        );
    }

    /// Items owned by removed player characters are destroyed.
    #[test]
    fn clear_players_destroys_carried_items() {
        let mut snapshot = minimal_snapshot_with_player();
        let stats = clear_player_characters(&mut snapshot);

        assert_eq!(stats.destroyed_items, 1);
        assert_eq!(snapshot.items[1].used, core::constants::USE_EMPTY);
    }

    /// Map tiles that referenced a removed player are zeroed.
    #[test]
    fn clear_players_clears_map_references() {
        let mut snapshot = minimal_snapshot_with_player();
        let stats = clear_player_characters(&mut snapshot);

        assert_eq!(stats.cleared_map_refs, 1);
        assert_eq!(snapshot.map[100].ch, 0);
    }

    /// A non-player character is left untouched by the sanitizer.
    #[test]
    fn clear_players_preserves_non_player_characters() {
        let mut snapshot = minimal_snapshot_with_player();
        snapshot.characters[2].used = core::constants::USE_ACTIVE;
        snapshot.characters[2].flags = 0; // not a player

        let stats = clear_player_characters(&mut snapshot);

        assert_eq!(stats.removed_players, 1);
        assert_eq!(snapshot.characters[2].used, core::constants::USE_ACTIVE);
    }
}
