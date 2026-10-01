//! `template-search` — quick CLI lookup of item and character template IDs.
//!
//! Loads a `.wsnap` world snapshot (defaults to `server/assets/world_seed.wsnap`)
//! and prints every item template matching the supplied filters, one per line:
//!
//! ```text
//! id   name                          reference                driver flags
//! ```
//!
//! The tool exists so that template IDs referenced from code (e.g. the
//! auto-loot category tables in `core::constants`) can be located and verified
//! against the live world data instead of being guessed.
//!
//! Examples:
//!
//! ```text
//! cargo run -p server-utils --bin template-search -- --name potion
//! cargo run -p server-utils --bin template-search -- --flag IF_MAGIC --flag IF_WEAPON
//! cargo run -p server-utils --bin template-search -- --chars --name ratling
//! cargo run -p server-utils --bin template-search -- --ids 101,102,127
//! ```

use std::path::PathBuf;

use clap::Parser;
use mag_core::constants::ItemFlags;
use mag_core::string_operations::c_string_to_str;
use server::keydb::snapshot::WorldSnapshot;

/// Command-line arguments for `template-search`.
#[derive(Debug, Parser)]
#[command(
    name = "template-search",
    about = "Search item/character templates in a world snapshot"
)]
struct Cli {
    /// Path to the `.wsnap` snapshot file.
    #[arg(long, default_value = "server/assets/world_seed.wsnap")]
    snapshot: PathBuf,

    /// Case-insensitive substring matched against name, reference and description.
    #[arg(long)]
    name: Option<String>,

    /// Case-insensitive substring matched against the description only.
    #[arg(long)]
    desc: Option<String>,

    /// Required item flag (repeatable, ANDed). Accepts `IF_MAGIC`, `MAGIC`, or a raw bit index.
    /// Composite names such as `WEAPON` match when any of their bits is set.
    #[arg(long = "flag")]
    flags: Vec<String>,

    /// Only items with a non-zero attribute/skill/hp/end/mana modifier.
    #[arg(long)]
    bonus: bool,

    /// Item flag that must be absent (repeatable).
    #[arg(long = "not-flag")]
    not_flags: Vec<String>,

    /// Required item driver number.
    #[arg(long)]
    driver: Option<u8>,

    /// Required placement bitmask (any bit overlapping).
    #[arg(long)]
    placement: Option<u16>,

    /// Explicit comma-separated list of template IDs to print.
    #[arg(long, value_delimiter = ',')]
    ids: Vec<usize>,

    /// Search character templates instead of item templates.
    #[arg(long)]
    chars: bool,

    /// List item templates that some NPC wants as a quest hand-in (`data[49]`).
    #[arg(long)]
    quest_items: bool,

    /// Print the full description for each match.
    #[arg(long)]
    verbose: bool,
}

/// Resolve a user-supplied flag name or bit index to an [`ItemFlags`] value.
///
/// # Arguments
///
/// * `raw` - Flag name with or without the `IF_` prefix, or a decimal bit index.
///
/// # Returns
///
/// * `Some(flags)` when recognised, `None` otherwise.
fn parse_item_flag(raw: &str) -> Option<ItemFlags> {
    let upper = raw.trim().to_ascii_uppercase();
    if let Ok(bit) = upper.parse::<u32>() {
        return ItemFlags::from_bits(1u64 << bit);
    }
    let name = if upper.starts_with("IF_") {
        upper
    } else {
        format!("IF_{upper}")
    };
    ItemFlags::from_name(&name)
}

/// Render the set flag names of an item as a compact `|`-joined string.
///
/// # Arguments
///
/// * `bits` - Raw item flag bits.
///
/// # Returns
///
/// * A string such as `TAKE|ARMOR|MAGIC`, or `-` when no flags are set.
fn flag_names(bits: u64) -> String {
    let flags = ItemFlags::from_bits_truncate(bits);
    let names: Vec<&str> = flags
        .iter_names()
        .filter(|(name, _)| *name != "IF_WEAPON" && *name != "IF_SELLABLE")
        .map(|(name, _)| name.trim_start_matches("IF_"))
        .collect();
    if names.is_empty() {
        "-".to_owned()
    } else {
        names.join("|")
    }
}

/// Whether an item template carries any stat-modifying bonus.
///
/// # Arguments
///
/// * `item` - Item template to inspect.
///
/// # Returns
///
/// * `true` when any attribute, skill, hp, endurance or mana modifier is non-zero.
fn has_bonus(item: &mag_core::types::Item) -> bool {
    item.attrib.iter().any(|a| a[0] != 0)
        || item.skill.iter().any(|s| s[0] != 0)
        || item.hp[0] != 0
        || item.end[0] != 0
        || item.mana[0] != 0
}

fn main() {
    let cli = Cli::parse();

    let snapshot = match WorldSnapshot::from_file(&cli.snapshot) {
        Ok(snapshot) => snapshot,
        Err(err) => {
            eprintln!("failed to load {}: {err}", cli.snapshot.display());
            std::process::exit(1);
        }
    };

    let needle = cli.name.as_deref().map(str::to_ascii_lowercase);
    let desc_needle = cli.desc.as_deref().map(str::to_ascii_lowercase);

    if cli.quest_items {
        let mut wanted: Vec<(usize, String)> = Vec::new();
        for ch in snapshot
            .character_templates
            .iter()
            .chain(snapshot.characters.iter())
        {
            if ch.used == 0 {
                continue;
            }
            let item_temp = ch.data[49] as usize;
            if item_temp == 0 || item_temp >= snapshot.item_templates.len() {
                continue;
            }
            if !wanted.iter().any(|(id, _)| *id == item_temp) {
                wanted.push((item_temp, ch.get_name().to_owned()));
            }
        }
        wanted.sort_by_key(|(id, _)| *id);
        println!("{:<5} {:<30} {:<26} wanted_by", "id", "name", "reference");
        for (id, npc) in wanted {
            let item = &snapshot.item_templates[id];
            println!(
                "{:<5} {:<30} {:<26} {}",
                id,
                item.get_name(),
                c_string_to_str(&item.reference),
                npc
            );
        }
        return;
    }

    if cli.chars {
        println!(
            "{:<5} {:<30} {:<30} {:<6} flags",
            "id", "name", "reference", "sprite"
        );
        for (id, ch) in snapshot.character_templates.iter().enumerate() {
            if ch.used == 0 {
                continue;
            }
            if !cli.ids.is_empty() && !cli.ids.contains(&id) {
                continue;
            }
            let name = ch.get_name();
            let reference = ch.get_reference();
            let description = c_string_to_str(&ch.description);
            if let Some(n) = &needle {
                let hay = format!("{name} {reference} {description}").to_ascii_lowercase();
                if !hay.contains(n) {
                    continue;
                }
            }
            println!(
                "{:<5} {:<30} {:<30} {:<6} 0x{:x}",
                id, name, reference, ch.sprite, ch.flags
            );
            if cli.verbose {
                println!("      {description}");
            }
        }
        return;
    }

    let mut required: Vec<ItemFlags> = Vec::new();
    for raw in &cli.flags {
        match parse_item_flag(raw) {
            Some(flag) => required.push(flag),
            None => {
                eprintln!("unknown item flag: {raw}");
                std::process::exit(2);
            }
        }
    }
    let mut forbidden = ItemFlags::empty();
    for raw in &cli.not_flags {
        match parse_item_flag(raw) {
            Some(flag) => forbidden |= flag,
            None => {
                eprintln!("unknown item flag: {raw}");
                std::process::exit(2);
            }
        }
    }

    println!(
        "{:<5} {:<28} {:<26} {:<6} {:<5} {:<9} flags",
        "id", "name", "reference", "driver", "place", "value"
    );
    for (id, item) in snapshot.item_templates.iter().enumerate() {
        if item.used == 0 {
            continue;
        }
        if !cli.ids.is_empty() && !cli.ids.contains(&id) {
            continue;
        }
        let bits = ItemFlags::from_bits_truncate(item.flags);
        if !required.iter().all(|flag| bits.intersects(*flag)) || bits.intersects(forbidden) {
            continue;
        }
        if cli.bonus && !has_bonus(item) {
            continue;
        }
        if let Some(driver) = cli.driver
            && item.driver != driver
        {
            continue;
        }
        if let Some(placement) = cli.placement
            && item.placement & placement == 0
        {
            continue;
        }
        let name = item.get_name();
        let reference = c_string_to_str(&item.reference);
        let description = c_string_to_str(&item.description);
        if let Some(n) = &needle {
            let hay = format!("{name} {reference} {description}").to_ascii_lowercase();
            if !hay.contains(n) {
                continue;
            }
        }
        if let Some(n) = &desc_needle
            && !description.to_ascii_lowercase().contains(n)
        {
            continue;
        }
        println!(
            "{:<5} {:<28} {:<26} {:<6} {:<5} {:<9} {}",
            id,
            name,
            reference,
            item.driver,
            item.placement,
            item.value,
            flag_names(item.flags)
        );
        if cli.verbose {
            println!("      {description}");
        }
    }
}
