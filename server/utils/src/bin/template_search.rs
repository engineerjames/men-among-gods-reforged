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
//! cargo run -p server-utils --bin template-search -- --chars --ids 364-374 --clear-worn RHAND
//! ```
//!
//! Character templates can also be edited (`--chars` plus `--clear-worn`,
//! `--set-armor-bonus`, `--set-weapon-bonus`). Edits apply to every matched
//! template and are only reported unless `--write` is given.

use std::path::PathBuf;

use clap::Parser;
use mag_core::constants::{
    ItemFlags, WN_ARMS, WN_BELT, WN_BODY, WN_CLOAK, WN_FEET, WN_HEAD, WN_LEGS, WN_LHAND, WN_LRING,
    WN_NECK, WN_RHAND, WN_RRING,
};
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

    /// Comma-separated template IDs or inclusive ranges (`364-374`) to match.
    #[arg(long, value_delimiter = ',', value_parser = parse_id_range)]
    ids: Vec<(usize, usize)>,

    /// Search character templates instead of item templates.
    #[arg(long)]
    chars: bool,

    /// Only character templates whose spawn point lies in `x1,y1,x2,y2` (inclusive). Requires `--chars`.
    #[arg(long, value_parser = parse_area)]
    area: Option<[i32; 4]>,

    /// Edit: empty this worn slot on matched character templates (repeatable).
    /// Accepts `HEAD NECK BODY ARMS BELT LEGS FEET LHAND RHAND CLOAK LRING RRING` or an index.
    #[arg(long = "clear-worn")]
    clear_worn: Vec<String>,

    /// Edit: set `armor_bonus` on matched character templates.
    #[arg(long)]
    set_armor_bonus: Option<u8>,

    /// Edit: set `weapon_bonus` on matched character templates.
    #[arg(long)]
    set_weapon_bonus: Option<u8>,

    /// Edit: set `points_tot` on matched character templates to the minimum for this rank index
    /// (e.g. 11 = Captain, 12 = Major). Overwritten if the template is later reset from its stats.
    #[arg(long, value_parser = clap::value_parser!(u8).range(0..24))]
    set_rank: Option<u8>,

    /// Edit: lower a base attribute to at most `NAME=VALUE` (e.g. `STREN=65`; repeatable).
    /// Names: `BRAVE WILL INT AGIL STREN`. Values already at or below the cap are untouched.
    #[arg(long = "cap-attrib")]
    cap_attrib: Vec<String>,

    /// Edit: lower a base skill to at most `NAME=VALUE` (e.g. `weapon=90`; repeatable).
    /// Name is a skill name/prefix or index. Values already at or below the cap are untouched.
    #[arg(long = "cap-skill")]
    cap_skill: Vec<String>,

    /// Save edits to disk; without it edits are only reported (dry run).
    #[arg(long)]
    write: bool,

    /// Where to save edits with `--write`; defaults to overwriting `--snapshot`.
    #[arg(long)]
    output: Option<PathBuf>,

    /// List item templates that some NPC wants as a quest hand-in (`data[49]`).
    #[arg(long)]
    quest_items: bool,

    /// Print the full description for each match.
    #[arg(long)]
    verbose: bool,
}

/// Parse a single ID (`12`) or inclusive range (`12-20`) into `(lo, hi)`.
///
/// # Arguments
///
/// * `raw` - The user-supplied token.
///
/// # Returns
///
/// * `Ok((lo, hi))` on success, `Err` with a message when malformed or reversed.
fn parse_id_range(raw: &str) -> Result<(usize, usize), String> {
    let num = |s: &str| s.trim().parse::<usize>().map_err(|e| format!("{raw}: {e}"));
    let (lo, hi) = match raw.split_once('-') {
        Some((lo, hi)) => (num(lo)?, num(hi)?),
        None => (num(raw)?, num(raw)?),
    };
    if lo > hi {
        return Err(format!("{raw}: range start exceeds end"));
    }
    Ok((lo, hi))
}

/// Parse `x1,y1,x2,y2` into a rectangle.
///
/// # Arguments
///
/// * `raw` - The user-supplied token.
///
/// # Returns
///
/// * `Ok([x1, y1, x2, y2])` on success, `Err` with a message when malformed or reversed.
fn parse_area(raw: &str) -> Result<[i32; 4], String> {
    let nums: Vec<i32> = raw
        .split(',')
        .map(|s| s.trim().parse::<i32>().map_err(|e| format!("{raw}: {e}")))
        .collect::<Result<_, _>>()?;
    match nums[..] {
        [x1, y1, x2, y2] if x1 <= x2 && y1 <= y2 => Ok([x1, y1, x2, y2]),
        _ => Err(format!(
            "{raw}: expected x1,y1,x2,y2 with x1<=x2 and y1<=y2"
        )),
    }
}

/// Whether `id` passes the `--ids` filter (an empty filter matches everything).
///
/// # Arguments
///
/// * `ranges` - Inclusive `(lo, hi)` ranges from `--ids`.
/// * `id` - Template ID to test.
///
/// # Returns
///
/// * `true` when no ranges were given or `id` falls in one of them.
fn id_selected(ranges: &[(usize, usize)], id: usize) -> bool {
    ranges.is_empty() || ranges.iter().any(|&(lo, hi)| (lo..=hi).contains(&id))
}

/// Resolve a worn-slot name (`RHAND`, `WN_RHAND`) or index to a `worn[]` index.
///
/// # Arguments
///
/// * `raw` - Slot name or decimal index.
///
/// # Returns
///
/// * `Some(index)` when recognised and below 20, `None` otherwise.
fn parse_worn_slot(raw: &str) -> Option<usize> {
    let upper = raw.trim().to_ascii_uppercase();
    if let Ok(idx) = upper.parse::<usize>() {
        return (idx < 20).then_some(idx);
    }
    Some(match upper.trim_start_matches("WN_") {
        "HEAD" => WN_HEAD,
        "NECK" => WN_NECK,
        "BODY" => WN_BODY,
        "ARMS" => WN_ARMS,
        "BELT" => WN_BELT,
        "LEGS" => WN_LEGS,
        "FEET" => WN_FEET,
        "LHAND" => WN_LHAND,
        "RHAND" => WN_RHAND,
        "CLOAK" => WN_CLOAK,
        "LRING" => WN_LRING,
        "RRING" => WN_RRING,
        _ => return None,
    })
}

/// Parse a `NAME=VALUE` cap into its resolved slot index and value.
///
/// # Arguments
///
/// * `raw`      - The user-supplied `NAME=VALUE` token.
/// * `is_skill` - Resolve `NAME` as a skill (otherwise as an attribute).
///
/// # Returns
///
/// * `Some((index, cap))` when the name resolves and the value is a `u16`.
fn parse_cap(raw: &str, is_skill: bool) -> Option<(usize, u16)> {
    let (name, value) = raw.split_once('=')?;
    let cap = value.trim().parse::<u16>().ok()?;
    let idx = if is_skill {
        usize::try_from(mag_core::skills::skill_lookup(name))
            .ok()
            .filter(|&i| i < mag_core::skills::MAX_SKILLS)?
    } else {
        ["BRAVE", "WILL", "INT", "AGIL", "STREN"]
            .iter()
            .position(|a| a.eq_ignore_ascii_case(name.trim()))?
    };
    Some((idx, cap))
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

    let mut snapshot = match WorldSnapshot::from_file(&cli.snapshot) {
        Ok(snapshot) => snapshot,
        Err(err) => {
            eprintln!("failed to load {}: {err}", cli.snapshot.display());
            std::process::exit(1);
        }
    };

    let needle = cli.name.as_deref().map(str::to_ascii_lowercase);
    let desc_needle = cli.desc.as_deref().map(str::to_ascii_lowercase);

    let editing = !cli.clear_worn.is_empty()
        || cli.set_armor_bonus.is_some()
        || cli.set_weapon_bonus.is_some()
        || cli.set_rank.is_some()
        || !cli.cap_attrib.is_empty()
        || !cli.cap_skill.is_empty();
    // Refuse to edit every template by accident.
    if editing && (!cli.chars || (cli.ids.is_empty() && needle.is_none())) {
        eprintln!("edits require --chars and a selector (--ids or --name)");
        std::process::exit(2);
    }
    if cli.area.is_some() && !cli.chars {
        eprintln!("--area requires --chars");
        std::process::exit(2);
    }
    let mut clear_slots: Vec<usize> = Vec::new();
    for raw in &cli.clear_worn {
        match parse_worn_slot(raw) {
            Some(slot) => clear_slots.push(slot),
            None => {
                eprintln!("unknown worn slot: {raw}");
                std::process::exit(2);
            }
        }
    }

    let parse_caps = |raws: &[String], is_skill: bool| -> Vec<(usize, u16)> {
        raws.iter()
            .map(|raw| {
                parse_cap(raw, is_skill).unwrap_or_else(|| {
                    eprintln!("bad cap (expected NAME=VALUE): {raw}");
                    std::process::exit(2);
                })
            })
            .collect()
    };
    let attrib_caps = parse_caps(&cli.cap_attrib, false);
    let skill_caps = parse_caps(&cli.cap_skill, true);

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
        let mut edited = 0usize;
        for (id, ch) in snapshot.character_templates.iter_mut().enumerate() {
            if ch.used == 0 {
                continue;
            }
            if !id_selected(&cli.ids, id) {
                continue;
            }
            if let Some([x1, y1, x2, y2]) = cli.area {
                let (x, y) = (i32::from(ch.x), i32::from(ch.y));
                if x < x1 || x > x2 || y < y1 || y > y2 {
                    continue;
                }
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
                println!(
                    "      points_tot {} ({}) armor_bonus {} weapon_bonus {}",
                    ch.points_tot,
                    mag_core::ranks::rank_name(ch.points_tot.max(0) as u32),
                    ch.armor_bonus,
                    ch.weapon_bonus
                );
            }
            if !editing {
                continue;
            }
            let mut changes: Vec<String> = Vec::new();
            for &slot in &clear_slots {
                let tmpl = ch.worn[slot];
                if tmpl != 0 {
                    let item_name = snapshot
                        .item_templates
                        .get(tmpl as usize)
                        .map_or("?", |i| i.get_name());
                    changes.push(format!("worn[{slot}] {tmpl} ({item_name}) -> 0"));
                    ch.worn[slot] = 0;
                }
            }
            if let Some(v) = cli.set_armor_bonus
                && ch.armor_bonus != v
            {
                changes.push(format!("armor_bonus {} -> {v}", ch.armor_bonus));
                ch.armor_bonus = v;
            }
            if let Some(v) = cli.set_weapon_bonus
                && ch.weapon_bonus != v
            {
                changes.push(format!("weapon_bonus {} -> {v}", ch.weapon_bonus));
                ch.weapon_bonus = v;
            }
            if let Some(rank) = cli.set_rank {
                let v = mag_core::ranks::RANK_THRESHOLDS[rank as usize] as i32;
                if ch.points_tot != v {
                    changes.push(format!("points_tot {} -> {v}", ch.points_tot));
                    ch.points_tot = v;
                }
            }
            for &(idx, cap) in &attrib_caps {
                if ch.attrib[idx][0] > cap {
                    changes.push(format!("attrib[{idx}] {} -> {cap}", ch.attrib[idx][0]));
                    ch.attrib[idx][0] = cap;
                }
            }
            for &(idx, cap) in &skill_caps {
                if ch.skill[idx][0] > cap {
                    changes.push(format!("skill[{idx}] {} -> {cap}", ch.skill[idx][0]));
                    ch.skill[idx][0] = cap;
                }
            }
            if !changes.is_empty() {
                edited += 1;
                println!("      edit: {}", changes.join(", "));
            }
        }
        if editing {
            if !cli.write {
                println!("{edited} template(s) would change (dry run; pass --write to save)");
            } else if edited == 0 {
                println!("nothing to change; snapshot not written");
            } else {
                let out = cli.output.as_ref().unwrap_or(&cli.snapshot);
                match snapshot.to_file(out) {
                    Ok(()) => println!("{edited} template(s) changed; wrote {}", out.display()),
                    Err(err) => {
                        eprintln!("{err}");
                        std::process::exit(1);
                    }
                }
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
        if !id_selected(&cli.ids, id) {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn id_ranges_parse_and_select() {
        let r = [
            parse_id_range("5").unwrap(),
            parse_id_range("10-12").unwrap(),
        ];
        assert!(id_selected(&r, 5) && id_selected(&r, 11) && !id_selected(&r, 9));
        assert!(id_selected(&[], 99));
        assert!(parse_id_range("12-10").is_err());
    }

    #[test]
    fn worn_slot_names_and_indices() {
        assert_eq!(parse_worn_slot("rhand"), Some(WN_RHAND));
        assert_eq!(parse_worn_slot("WN_HEAD"), Some(WN_HEAD));
        assert_eq!(parse_worn_slot("8"), Some(8));
        assert_eq!(parse_worn_slot("20"), None);
        assert_eq!(parse_worn_slot("bogus"), None);
    }

    #[test]
    fn caps_resolve_attribs_and_skills() {
        assert_eq!(parse_cap("stren=65", false), Some((4, 65)));
        assert_eq!(parse_cap("AGIL=65", false), Some((3, 65)));
        assert_eq!(parse_cap("hand=90", true), Some((0, 90)));
        assert_eq!(
            parse_cap("weapon=90", true),
            Some((mag_core::skills::SK_WEAPON, 90))
        );
        assert_eq!(parse_cap("bogus=1", false), None);
        assert_eq!(parse_cap("stren", false), None);
    }
}
