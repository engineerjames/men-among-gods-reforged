//! One-shot helper: re-encode an existing `.wsnap` file using the new
//! zstd-wrapped on-disk envelope.
//!
//! Reads any historical snapshot (raw `MGSN` v1 or v2, or already-wrapped
//! `MGSZ`) and writes it back through `WorldSnapshot::to_file`, which always
//! produces the compressed `MGSZ` envelope. Intended for a single use to
//! shrink `server/assets/world_seed.wsnap` below GitHub's 100 MB file limit.

use std::path::PathBuf;

use server::keydb::snapshot::WorldSnapshot;

fn main() {
    let mut args = std::env::args().skip(1);
    let input: PathBuf = args
        .next()
        .expect("usage: compress-world-snapshot <input.wsnap> <output.wsnap>")
        .into();
    let output: PathBuf = args
        .next()
        .expect("usage: compress-world-snapshot <input.wsnap> <output.wsnap>")
        .into();

    let snap = WorldSnapshot::from_file(&input).expect("decode input snapshot");
    snap.to_file(&output).expect("write compressed snapshot");
    println!("wrote {}", output.display());
}
