//! Migration tool: load `.dat` files and write them to KeyDB.
//!
//! Usage:
//!   cargo run -p server --bin dat-to-keydb [-- [--dat-dir <path>] [--force]]
//!
//! If `--dat-dir` is omitted, defaults to `<exe_parent>/.dat/`.
//! If game data already exists in KeyDB, `--force` is required to overwrite.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

use bincode::{Decode, Encode};
use redis::{pipe, Commands, Connection};

const NORMALIZED_MAGIC: [u8; 4] = *b"MAG2";
const NORMALIZED_VERSION: u32 = 1;

/// KeyDB schema version written to `game:meta:version`.
const SCHEMA_VERSION: u32 = 1;

/// Pipeline batch size for Redis writes.
const PIPELINE_BATCH_SIZE: usize = 4096;

#[allow(dead_code)]
#[derive(Debug, bincode::Decode)]
struct NormalizedDataSet<T> {
    magic: [u8; 4],
    version: u32,
    source_file: String,
    source_record_size: usize,
    records: Vec<T>,
}

// ---------------------------------------------------------------------------
//  Inline KeyDB write helpers (mirrors keydb_store.rs)
// ---------------------------------------------------------------------------

fn encode_bincode<T: Encode>(val: &T) -> Result<Vec<u8>, String> {
    bincode::encode_to_vec(val, bincode::config::standard()).map_err(|e| format!("Encode: {e}"))
}

fn save_indexed<T: Encode>(
    con: &mut Connection,
    prefix: &str,
    entities: &[T],
    label: &str,
) -> Result<(), String> {
    println!("  Writing {label} ({} keys)...", entities.len());
    let t = Instant::now();

    for batch_start in (0..entities.len()).step_by(PIPELINE_BATCH_SIZE) {
        let batch_end = (batch_start + PIPELINE_BATCH_SIZE).min(entities.len());
        let mut pipeline = pipe();
        for idx in batch_start..batch_end {
            let bytes = encode_bincode(&entities[idx])?;
            pipeline.cmd("SET").arg(format!("{prefix}{idx}")).arg(bytes);
        }
        pipeline
            .query::<()>(con)
            .map_err(|e| format!("Pipeline SET {prefix}*: {e}"))?;
    }

    println!("    done ({:.2?})", t.elapsed());
    Ok(())
}

fn save_map_tiles(con: &mut Connection, map: &[core::types::Map]) -> Result<(), String> {
    let map_x = core::constants::SERVER_MAPX as usize;
    let total = map.len();
    println!("  Writing map tiles ({total} keys)...");
    let t = Instant::now();

    for batch_start in (0..total).step_by(PIPELINE_BATCH_SIZE) {
        let batch_end = (batch_start + PIPELINE_BATCH_SIZE).min(total);
        let mut pipeline = pipe();
        for linear in batch_start..batch_end {
            let x = linear % map_x;
            let y = linear / map_x;
            let bytes = encode_bincode(&map[linear])?;
            pipeline
                .cmd("SET")
                .arg(format!("game:map:{x}:{y}"))
                .arg(bytes);
        }
        pipeline
            .query::<()>(con)
            .map_err(|e| format!("Pipeline SET game:map: {e}"))?;
    }

    println!("    done ({:.2?})", t.elapsed());
    Ok(())
}

fn load_normalized_records<T: Decode<()>>(
    dat_dir: &Path,
    file_name: &str,
    expected_count: usize,
) -> Result<Vec<T>, String> {
    let path = dat_dir.join(file_name);
    let bytes = fs::read(&path).map_err(|e| format!("Read {}: {e}", path.display()))?;

    let (payload, consumed): (NormalizedDataSet<T>, usize) =
        bincode::decode_from_slice(&bytes, bincode::config::standard())
            .map_err(|e| format!("Decode {}: {e}", path.display()))?;

    if consumed != bytes.len() {
        eprintln!(
            "Warning: {} trailing bytes in {}",
            bytes.len() - consumed,
            path.display()
        );
    }

    if payload.magic != NORMALIZED_MAGIC {
        return Err(format!("Invalid magic in {}", path.display()));
    }
    if payload.version != NORMALIZED_VERSION {
        return Err(format!(
            "Unsupported version {} in {}",
            payload.version,
            path.display()
        ));
    }
    if payload.source_file != file_name {
        return Err(format!(
            "Source file mismatch in {}: expected {file_name}, got {}",
            path.display(),
            payload.source_file
        ));
    }
    if payload.records.len() != expected_count {
        return Err(format!(
            "Record count mismatch in {}: expected {expected_count}, got {}",
            path.display(),
            payload.records.len()
        ));
    }
    Ok(payload.records)
}

fn main() {
    let args: Vec<String> = env::args().collect();

    // Parse simple CLI flags
    let mut dat_dir: Option<PathBuf> = None;
    let mut force = false;
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--dat-dir" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("Error: --dat-dir requires a path argument");
                    std::process::exit(1);
                }
                dat_dir = Some(PathBuf::from(&args[i]));
            }
            "--force" => {
                force = true;
            }
            other => {
                eprintln!("Unknown argument: {other}");
                eprintln!("Usage: dat-to-keydb [--dat-dir <path>] [--force]");
                std::process::exit(1);
            }
        }
        i += 1;
    }

    // Resolve .dat directory
    let dat_dir = dat_dir.unwrap_or_else(|| {
        let exe = env::current_exe().expect("Failed to determine executable path");
        exe.parent()
            .unwrap_or_else(|| Path::new("."))
            .join(".dat")
    });

    if !dat_dir.is_dir() {
        eprintln!("Error: .dat directory not found at {}", dat_dir.display());
        std::process::exit(1);
    }

    println!("=== Men Among Gods: .dat → KeyDB Migration Tool ===");
    println!("Loading from: {}", dat_dir.display());
    println!();

    // -----------------------------------------------------------------------
    //  Load .dat files
    // -----------------------------------------------------------------------
    let total_start = Instant::now();

    let t = Instant::now();
    let map_x = core::constants::SERVER_MAPX as usize;
    let map_y = core::constants::SERVER_MAPY as usize;
    let map = load_normalized_records::<core::types::Map>(&dat_dir, "map.dat", map_x * map_y)
        .unwrap_or_else(|e| {
            eprintln!("Failed to load map.dat: {e}");
            std::process::exit(1);
        });
    println!("  map.dat        — {} tiles    ({:.2?})", map.len(), t.elapsed());

    let t = Instant::now();
    let items = load_normalized_records::<core::types::Item>(
        &dat_dir,
        "item.dat",
        core::constants::MAXITEM,
    )
    .unwrap_or_else(|e| {
        eprintln!("Failed to load item.dat: {e}");
        std::process::exit(1);
    });
    println!("  item.dat       — {} items   ({:.2?})", items.len(), t.elapsed());

    let t = Instant::now();
    let item_templates = load_normalized_records::<core::types::Item>(
        &dat_dir,
        "titem.dat",
        core::constants::MAXTITEM,
    )
    .unwrap_or_else(|e| {
        eprintln!("Failed to load titem.dat: {e}");
        std::process::exit(1);
    });
    println!(
        "  titem.dat      — {} templates ({:.2?})",
        item_templates.len(),
        t.elapsed()
    );

    let t = Instant::now();
    let characters = load_normalized_records::<core::types::Character>(
        &dat_dir,
        "char.dat",
        core::constants::MAXCHARS,
    )
    .unwrap_or_else(|e| {
        eprintln!("Failed to load char.dat: {e}");
        std::process::exit(1);
    });
    println!(
        "  char.dat       — {} chars    ({:.2?})",
        characters.len(),
        t.elapsed()
    );

    let t = Instant::now();
    let character_templates = load_normalized_records::<core::types::Character>(
        &dat_dir,
        "tchar.dat",
        core::constants::MAXTCHARS,
    )
    .unwrap_or_else(|e| {
        eprintln!("Failed to load tchar.dat: {e}");
        std::process::exit(1);
    });
    println!(
        "  tchar.dat      — {} templates ({:.2?})",
        character_templates.len(),
        t.elapsed()
    );

    let t = Instant::now();
    let effects = load_normalized_records::<core::types::Effect>(
        &dat_dir,
        "effect.dat",
        core::constants::MAXEFFECT,
    )
    .unwrap_or_else(|e| {
        eprintln!("Failed to load effect.dat: {e}");
        std::process::exit(1);
    });
    println!(
        "  effect.dat     — {} effects  ({:.2?})",
        effects.len(),
        t.elapsed()
    );

    let t = Instant::now();
    let mut globals_vec =
        load_normalized_records::<core::types::Global>(&dat_dir, "global.dat", 1)
            .unwrap_or_else(|e| {
                eprintln!("Failed to load global.dat: {e}");
                std::process::exit(1);
            });
    let globals = globals_vec.drain(..).next().unwrap();
    println!("  global.dat     — loaded      ({:.2?})", t.elapsed());

    // Text files
    let bad_names: Vec<String> = fs::read_to_string(dat_dir.join("badnames.txt"))
        .unwrap_or_default()
        .lines()
        .map(|l| l.to_string())
        .collect();
    let bad_words: Vec<String> = fs::read_to_string(dat_dir.join("badwords.txt"))
        .unwrap_or_default()
        .lines()
        .map(|l| l.to_string())
        .collect();
    let motd = fs::read_to_string(dat_dir.join("motd.txt"))
        .unwrap_or_else(|_| "Live long and prosper!".to_string());
    println!(
        "  text files     — {} bad names, {} bad words, motd {} chars",
        bad_names.len(),
        bad_words.len(),
        motd.len()
    );

    println!(
        "\nAll .dat files loaded in {:.2?}",
        total_start.elapsed()
    );

    // -----------------------------------------------------------------------
    //  Connect to KeyDB
    // -----------------------------------------------------------------------
    let keydb_url =
        env::var("MAG_KEYDB_URL").unwrap_or_else(|_| "redis://127.0.0.1:5556/".to_string());
    println!("\nConnecting to KeyDB at {keydb_url}...");

    let client = redis::Client::open(keydb_url.as_str()).unwrap_or_else(|e| {
        eprintln!("Failed to create KeyDB client: {e}");
        std::process::exit(1);
    });
    let mut con = client.get_connection().unwrap_or_else(|e| {
        eprintln!("Failed to connect to KeyDB: {e}");
        std::process::exit(1);
    });

    // Check for existing data
    let exists: bool = redis::Commands::exists(&mut con, "game:meta:version").unwrap_or(false);
    if exists && !force {
        eprintln!(
            "Error: Game data already exists in KeyDB (game:meta:version found).\n\
             Use --force to overwrite."
        );
        std::process::exit(1);
    }

    // -----------------------------------------------------------------------
    //  Write to KeyDB
    // -----------------------------------------------------------------------
    println!("\nWriting game data to KeyDB...");
    let write_start = Instant::now();

    save_map_tiles(&mut con, &map).unwrap_or_else(|e| {
        eprintln!("Failed to save map: {e}");
        std::process::exit(1);
    });

    save_indexed(&mut con, "game:item:", &items, "items").unwrap_or_else(|e| {
        eprintln!("Failed to save items: {e}");
        std::process::exit(1);
    });

    save_indexed(&mut con, "game:titem:", &item_templates, "item templates").unwrap_or_else(|e| {
        eprintln!("Failed to save item templates: {e}");
        std::process::exit(1);
    });

    save_indexed(&mut con, "game:char:", &characters, "characters").unwrap_or_else(|e| {
        eprintln!("Failed to save characters: {e}");
        std::process::exit(1);
    });

    save_indexed(&mut con, "game:tchar:", &character_templates, "character templates")
        .unwrap_or_else(|e| {
            eprintln!("Failed to save character templates: {e}");
            std::process::exit(1);
        });

    save_indexed(&mut con, "game:effect:", &effects, "effects").unwrap_or_else(|e| {
        eprintln!("Failed to save effects: {e}");
        std::process::exit(1);
    });

    // Globals
    {
        println!("  Writing globals...");
        let bytes = encode_bincode(&globals).unwrap();
        con.set::<_, _, ()>("game:global", bytes).unwrap_or_else(|e| {
            eprintln!("Failed to save globals: {e}");
            std::process::exit(1);
        });
    }

    // Text data
    {
        println!("  Writing text data...");
        let bn_bytes = encode_bincode(&bad_names).unwrap();
        con.set::<_, _, ()>("game:badnames", bn_bytes).unwrap_or_else(|e| {
            eprintln!("Failed to save badnames: {e}");
            std::process::exit(1);
        });

        let bw_bytes = encode_bincode(&bad_words).unwrap();
        con.set::<_, _, ()>("game:badwords", bw_bytes).unwrap_or_else(|e| {
            eprintln!("Failed to save badwords: {e}");
            std::process::exit(1);
        });

        con.set::<_, _, ()>("game:motd", &motd).unwrap_or_else(|e| {
            eprintln!("Failed to save motd: {e}");
            std::process::exit(1);
        });
    }

    // Schema version marker
    con.set::<_, _, ()>("game:meta:version", SCHEMA_VERSION)
        .unwrap_or_else(|e| {
            eprintln!("Failed to set schema version: {e}");
            std::process::exit(1);
        });

    let _load_duration = write_start.elapsed() - write_start.elapsed(); // approx
    println!(
        "\nMigration complete! Total time: {:.2?} (write: {:.2?})",
        total_start.elapsed(),
        write_start.elapsed(),
    );
    let total_keys = map.len()
        + items.len()
        + item_templates.len()
        + characters.len()
        + character_templates.len()
        + effects.len()
        + 1  // globals
        + 3  // badnames, badwords, motd
        + 1; // meta:version
    println!("Total keys written: {total_keys}");
    println!("\nDone! Server can now start with MAG_STORAGE_BACKEND=keydb");
}
