//! Standalone benchmark runner that prints the formatted report.
//!
//! Automatically runs both synthetic and real `.dat` data benchmarks (when
//! `.dat` files are found). The search order for `.dat` files is:
//!
//!   1. `MAG_DAT_PATH` environment variable (if set)
//!   2. `../assets/.dat`  (when run from `server/sqlite-bench/`)
//!   3. `../../server/assets/.dat`  (when run from repo root)
//!
//! Usage:
//!   cargo run --release
//!   MAG_DAT_PATH=/path/to/.dat cargo run --release   # explicit override

use rusqlite::Connection;
use sqlite_bench::populate::{generate_synthetic, try_load_dat_files, GameData};
use sqlite_bench::report::{print_report, SchemaResult};
use sqlite_bench::schema::blob::BlobSchema;
use sqlite_bench::schema::normalized::NormalizedSchema;
use sqlite_bench::schema::{configure_connection, BenchSchema, PopulationParams};
use sqlite_bench::tick_sim::simulate_tick;
use std::time::Instant;

const WARMUP_TICKS: u32 = 10;
const SAMPLE_TICKS: u32 = 100;

fn bench_schema_with_data(
    schema: &dyn BenchSchema,
    schema_name: &str,
    params: &PopulationParams,
    pop_label: &str,
    data: &GameData,
) -> SchemaResult {
    let conn = Connection::open_in_memory().expect("open in-memory SQLite");
    configure_connection(&conn).expect("configure connection");
    schema.create_tables(&conn).expect("create tables");

    schema
        .populate(
            &conn,
            &data.characters,
            &data.items,
            &data.map,
            &data.effects,
            &data.globals,
        )
        .expect("populate");

    // Warmup
    for t in 0..WARMUP_TICKS {
        simulate_tick(&conn, schema, params, t).expect("warmup tick");
    }

    // Collect samples
    let mut result = SchemaResult::new(schema_name, pop_label);
    for t in WARMUP_TICKS..(WARMUP_TICKS + SAMPLE_TICKS) {
        let start = Instant::now();
        let tick_stats = simulate_tick(&conn, schema, params, t).expect("sample tick");
        result.add_sample_with_stats(start.elapsed(), &tick_stats);
    }

    result
}

/// Try to locate real `.dat` files on disk.
fn find_dat_files() -> Option<GameData> {
    // 1. Explicit env var
    if let Ok(path) = std::env::var("MAG_DAT_PATH") {
        if let Some(data) = try_load_dat_files(&path) {
            eprintln!("  Loaded real .dat files from MAG_DAT_PATH={path}");
            return Some(data);
        }
        eprintln!("  Warning: MAG_DAT_PATH={path} set but load failed, trying fallback paths...");
    }

    // 2. Common relative paths (from server/sqlite-bench/ and repo root)
    let candidates = [
        "../assets/.dat",
        "../../server/assets/.dat",
        "server/assets/.dat",
    ];
    for candidate in &candidates {
        if let Some(data) = try_load_dat_files(candidate) {
            eprintln!("  Loaded real .dat files from {candidate}");
            return Some(data);
        }
    }

    None
}

/// Count active characters in the data to derive realistic population params.
fn params_from_dat(data: &GameData) -> PopulationParams {
    let active = data.characters.iter().filter(|c| c.used != 0).count();
    let players = data
        .characters
        .iter()
        .filter(|c| c.used != 0 && c.flags & 1 != 0)
        .count();
    let npcs = active.saturating_sub(players);
    let active_items = data.items.iter().filter(|i| i.used != 0).count();
    let active_effects = data.effects.iter().filter(|e| e.used != 0).count();

    PopulationParams {
        active_players: players.max(1),
        active_npcs: npcs,
        active_items,
        active_effects: active_effects.max(1),
        max_characters: data.characters.len(),
        max_items: data.items.len(),
        max_effects: data.effects.len(),
    }
}

fn run_suite(
    label_prefix: &str,
    schemas: &[(&str, &dyn BenchSchema)],
    populations: &[(&str, PopulationParams, GameData)],
) -> Vec<SchemaResult> {
    let mut results = Vec::new();
    for (schema_name, schema) in schemas {
        for (pop_label, params, data) in populations {
            let tag = format!("{label_prefix}{pop_label}");
            eprint!("  Benchmarking {schema_name}/{tag}...");
            let r = bench_schema_with_data(*schema, schema_name, params, &tag, data);
            eprintln!(" done ({:.2}ms mean)", r.mean_us() / 1000.0);
            results.push(r);
        }
    }
    results
}

fn main() {
    println!("Running SQLite in-memory tick benchmark...");
    println!("  Warmup ticks:  {WARMUP_TICKS}");
    println!("  Sample ticks:  {SAMPLE_TICKS}");

    let blob = BlobSchema::new();
    let norm = NormalizedSchema::new();
    let schemas: Vec<(&str, &dyn BenchSchema)> = vec![("blob", &blob), ("normalized", &norm)];

    let mut results = Vec::new();

    // ── Synthetic data ──────────────────────────────────────────────
    println!("\n── Synthetic data ─────────────────────────────────────");
    let std_params = PopulationParams::standard();
    let stressed_params = PopulationParams::stressed();

    let synthetic_pops: Vec<(&str, PopulationParams, GameData)> = vec![
        ("std", std_params, generate_synthetic(&std_params)),
        (
            "stressed",
            stressed_params,
            generate_synthetic(&stressed_params),
        ),
    ];

    results.extend(run_suite("", &schemas, &synthetic_pops));

    // ── Real .dat data ──────────────────────────────────────────────
    println!("\n── Real .dat data ─────────────────────────────────────");
    match find_dat_files() {
        Some(dat_data) => {
            let dat_params = params_from_dat(&dat_data);
            eprintln!(
                "  Population: {} players, {} NPCs, {} items, {} effects",
                dat_params.active_players,
                dat_params.active_npcs,
                dat_params.active_items,
                dat_params.active_effects
            );

            let dat_pops: Vec<(&str, PopulationParams, GameData)> =
                vec![("dat", dat_params, dat_data)];

            results.extend(run_suite("", &schemas, &dat_pops));
        }
        None => {
            eprintln!("  No .dat files found — skipping real data benchmarks.");
            eprintln!("  Set MAG_DAT_PATH or run from a directory with ../assets/.dat");
        }
    }

    print_report(&results);
}
