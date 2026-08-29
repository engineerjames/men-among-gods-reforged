//! Throwaway diagnostic: scans a `.wsnap` file for NPCs that accept a
//! quest-item turn-in (`data[49] != 0`) and prints their name, template
//! index, required item template, taught skill (`data[50]`), and any
//! return-gift template (`data[66]`).
//!
//! Usage: `cargo run -p server --example scan_quest_npcs -- server/assets/world_seed.wsnap`

use server::keydb::snapshot::WorldSnapshot;

fn main() {
    let path = std::env::args()
        .nth(1)
        .expect("usage: scan_quest_npcs <path.wsnap>");
    let snapshot =
        WorldSnapshot::from_file(std::path::Path::new(&path)).expect("failed to read snapshot");

    println!("== character_templates (data[49] != 0) ==");
    for (idx, ch) in snapshot.character_templates.iter().enumerate() {
        if ch.used != 0 && ch.data[49] != 0 {
            let item_name = snapshot
                .item_templates
                .get(ch.data[49] as usize)
                .map(|it| it.get_name())
                .unwrap_or("?");
            println!(
                "template_idx={idx} name={:?} item_temp={} item_name={:?} skill={} return_gift={} exp={} riddle_area={}",
                ch.get_name(),
                ch.data[49],
                item_name,
                ch.data[50],
                ch.data[66],
                ch.data[51],
                ch.data[72],
            );
        }
    }

    println!("== characters (data[49] != 0) ==");
    for (idx, ch) in snapshot.characters.iter().enumerate() {
        if ch.used != 0 && ch.data[49] != 0 {
            let item_name = snapshot
                .item_templates
                .get(ch.data[49] as usize)
                .map(|it| it.get_name())
                .unwrap_or("?");
            println!(
                "char_idx={idx} temp={} name={:?} item_temp={} item_name={:?} skill={} return_gift={} exp={} riddle_area={}",
                ch.temp,
                ch.get_name(),
                ch.data[49],
                item_name,
                ch.data[50],
                ch.data[66],
                ch.data[51],
                ch.data[72],
            );
        }
    }
}
