//! Throwaway diagnostic: prints the key driver fields for one character
//! template (and any live instances of it) from a `.wsnap` file.
//!
//! Usage: `cargo run -p server --example inspect_zoetje -- server/assets/world_seed.wsnap 1172`

use server::keydb::snapshot::WorldSnapshot;

fn main() {
    let path = std::env::args()
        .nth(1)
        .expect("usage: inspect_zoetje <path.wsnap> [template_idx]");
    let target: usize = std::env::args()
        .nth(2)
        .and_then(|v| v.parse().ok())
        .unwrap_or(1172);

    let snapshot =
        WorldSnapshot::from_file(std::path::Path::new(&path)).expect("failed to read snapshot");

    let ch = &snapshot.character_templates[target];
    println!(
        "template {target}: name={:?} used={} flags=0x{:x} x={} y={} dir={}",
        ch.get_name(),
        ch.used,
        ch.flags,
        ch.x,
        ch.y,
        ch.dir
    );
    println!(
        "  data[25] special_driver={} data[26] sub={} data[29] rest_pos={} data[30] rest_dir={}",
        ch.data[25], ch.data[26], ch.data[29], ch.data[30]
    );
    println!("  sprite={} temp={}", ch.sprite, ch.temp);

    for (idx, live) in snapshot.characters.iter().enumerate() {
        if live.used != 0 && live.temp as usize == target {
            println!(
                "  live char {idx}: x={} y={} dir={} data[25]={} data[29]={} data[30]={} stunned={}",
                live.x, live.y, live.dir, live.data[25], live.data[29], live.data[30], live.stunned
            );
        }
    }
}
