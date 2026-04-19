//! `world_snapshot` — Export, import, and verify world snapshots.
//!
//! This binary provides the supported snapshot import/export workflow for
//! seeding and backing up KeyDB world data:
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
//! ```
//!
//! The resulting `.wsnap` file is a single `bincode`-encoded
//! [`WorldSnapshot`](server::snapshot::WorldSnapshot) that can be committed
//! to version control, copied between environments, or edited by external
//! tooling.

use std::env;
use std::path::PathBuf;
use std::process;
use std::time::Instant;

use redis::Commands;

use server::keydb;
use server::keydb_store;
use server::snapshot::{SNAPSHOT_SCHEMA_VERSION, WorldSnapshot};

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
         \n  {prog} export  --output <file.wsnap>\
         \n  {prog} import  --input  <file.wsnap> [--skip-if-seeded] [--force]\
         \n  {prog} verify  --input  <file.wsnap>\
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

/// Export all game data from KeyDB to a snapshot file.
///
/// Connects to KeyDB, calls [`keydb_store::load_all`] to read all entities,
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
    let data = keydb_store::load_all(&mut con).unwrap_or_else(|e| {
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
fn cmd_import(input: &PathBuf, skip_if_seeded: bool, force: bool) {
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

    // Seeded-data guard.
    let exists = keydb_store::has_game_data(&mut con).unwrap_or(false);
    if exists && !force {
        if skip_if_seeded {
            println!("Game data already exists in KeyDB. Skipping import (--skip-if-seeded).");
            return;
        }
        eprintln!(
            "Error: game data already exists in KeyDB (game:meta:version found).\n\
             Use --force to overwrite."
        );
        process::exit(1);
    }

    println!("\nWriting game data to KeyDB...");

    keydb_store::save_map(&mut con, &snapshot.map).unwrap_or_else(|e| {
        eprintln!("Failed to save map: {e}");
        process::exit(1);
    });

    keydb_store::save_items(&mut con, &snapshot.items).unwrap_or_else(|e| {
        eprintln!("Failed to save items: {e}");
        process::exit(1);
    });

    keydb_store::save_item_templates(&mut con, &snapshot.item_templates).unwrap_or_else(|e| {
        eprintln!("Failed to save item templates: {e}");
        process::exit(1);
    });

    keydb_store::save_characters(&mut con, &snapshot.characters).unwrap_or_else(|e| {
        eprintln!("Failed to save characters: {e}");
        process::exit(1);
    });

    keydb_store::save_character_templates(&mut con, &snapshot.character_templates).unwrap_or_else(
        |e| {
            eprintln!("Failed to save character templates: {e}");
            process::exit(1);
        },
    );

    keydb_store::save_effects(&mut con, &snapshot.effects).unwrap_or_else(|e| {
        eprintln!("Failed to save effects: {e}");
        process::exit(1);
    });

    keydb_store::save_globals(&mut con, &snapshot.globals).unwrap_or_else(|e| {
        eprintln!("Failed to save globals: {e}");
        process::exit(1);
    });

    keydb_store::save_text_data(
        &mut con,
        &snapshot.bad_names,
        &snapshot.bad_words,
        &snapshot.motd,
    )
    .unwrap_or_else(|e| {
        eprintln!("Failed to save text data: {e}");
        process::exit(1);
    });

    // Schema version marker (must match keydb_store::SCHEMA_VERSION).
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
fn cmd_verify(input: &PathBuf) {
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
    }
}
