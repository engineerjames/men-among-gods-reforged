//! One-shot migration: read a v1 `world_seed.wsnap`, convert to v2 in
//! memory, and write it back. Safe to delete after the migration has run.
//!
//! Usage:
//!
//! ```text
//! cargo run -p server --bin migrate_world_snapshot_v1_to_v2 -- \
//!   --input server/assets/world_seed.wsnap \
//!   --output server/assets/world_seed.wsnap
//! ```

use std::env;
use std::path::PathBuf;
use std::process;

use server::keydb::snapshot::WorldSnapshot;

fn flag_value(args: &[String], name: &str) -> Option<String> {
    let mut iter = args.iter();
    while let Some(a) = iter.next() {
        if a == name {
            return iter.next().cloned();
        }
    }
    None
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let input = flag_value(&args, "--input").unwrap_or_else(|| {
        eprintln!("Usage: migrate_world_snapshot_v1_to_v2 --input <file> --output <file>");
        process::exit(1);
    });
    let output = flag_value(&args, "--output").unwrap_or_else(|| input.clone());

    let in_path = PathBuf::from(&input);
    let out_path = PathBuf::from(&output);

    println!("Reading {}", in_path.display());
    let snapshot = match WorldSnapshot::from_file(&in_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Migration read failed: {e}");
            process::exit(1);
        }
    };

    println!("Migrated -> {}", snapshot.summary());
    println!("Writing  {}", out_path.display());
    if let Err(e) = snapshot.to_file(&out_path) {
        eprintln!("Migration write failed: {e}");
        process::exit(1);
    }
    println!("Done.");
}
