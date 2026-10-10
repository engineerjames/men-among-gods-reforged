//! `template-sync` — push item/character templates from a `.wsnap` file into the live KeyDB.
//!
//! Compares every template slot in the file against what is stored in KeyDB, prints
//! which templates (and which fields) differ, writes only the changed slots, bumps the
//! template version counters, and asks the running game server to hot-reload them.
//!
//! ```text
//! cargo run -p server-utils --bin template-sync -- --dry-run
//! cargo run -p server-utils --bin template-sync -- --file my_templates.wsnap --chars
//! ```
//!
//! Only `game:titem:*` / `game:tchar:*` are touched; the rest of the world is left alone.

use std::fmt::Debug;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use bincode::Encode;
use clap::Parser;
use mag_core::string_operations::c_string_to_str;
use mag_core::template_store::{self, TemplateKind};
use mag_core::types::{Character, Item};
use redis::Commands;
use server::keydb::connection;
use server::keydb::snapshot::WorldSnapshot;
use server::keydb::store;

/// How long to wait for the server to confirm a reload.
const RELOAD_WAIT: Duration = Duration::from_secs(15);

/// Text fields compared as strings instead of raw byte arrays.
const TEXT_FIELDS: [&str; 3] = ["name", "reference", "description"];

/// Command-line arguments for `template-sync`.
#[derive(Debug, Parser)]
#[command(
    name = "template-sync",
    about = "Update live item/character templates in KeyDB from a world snapshot file"
)]
struct Cli {
    /// Snapshot file holding the desired templates.
    #[arg(long, default_value = "server/assets/world_seed.wsnap")]
    file: PathBuf,

    /// Only sync item templates.
    #[arg(long)]
    items: bool,

    /// Only sync character templates.
    #[arg(long)]
    chars: bool,

    /// Report differences without writing anything.
    #[arg(long)]
    dry_run: bool,

    /// Write to KeyDB but do not ask the running server to reload.
    #[arg(long)]
    no_reload: bool,
}

/// Field-level differences between two values, as `path: old -> new` lines.
///
/// Walks the pretty `Debug` output of both values in lockstep; this relies on both
/// having the same shape, which holds for the fixed-size `Item`/`Character` structs.
/// Fields named in [`TEXT_FIELDS`] are skipped (see [`text_changes`]).
///
/// # Arguments
///
/// * `old` - The value currently stored.
/// * `new` - The desired value.
///
/// # Returns
///
/// * One line per differing leaf value, empty when equal.
fn field_changes<T: Debug>(old: &T, new: &T) -> Vec<String> {
    let (a, b) = (format!("{old:#?}"), format!("{new:#?}"));
    // Open containers as (label, children seen so far); the root struct line is skipped.
    let mut stack: Vec<(String, usize)> = Vec::new();
    let mut out = Vec::new();
    for (la, lb) in a.lines().zip(b.lines()).skip(1) {
        let (ta, tb) = (la.trim(), lb.trim());
        if ta.starts_with([']', '}', ')']) {
            stack.pop();
            continue;
        }
        let idx = stack.last_mut().map(|p| {
            p.1 += 1;
            p.1 - 1
        });
        let field = ta.split_once(": ").map(|(f, _)| f).filter(|f| {
            f.chars().all(|c| c.is_alphanumeric() || c == '_')
                && !f.starts_with(|c: char| c.is_ascii_digit())
        });
        let label = field.map_or_else(|| format!("[{}]", idx.unwrap_or(0)), str::to_owned);
        let path = stack.iter().map(|p| p.0.as_str()).collect::<String>() + &label;

        if ta.ends_with(['[', '{', '(']) {
            stack.push((label, 0));
        } else if ta != tb && !TEXT_FIELDS.contains(&path.split('[').next().unwrap_or("")) {
            let value = |t: &str| {
                t.split_once(": ")
                    .filter(|_| field.is_some())
                    .map_or(t, |(_, v)| v)
                    .trim_end_matches(',')
                    .to_owned()
            };
            out.push(format!("{path}: {} -> {}", value(ta), value(tb)));
        }
    }
    out
}

/// Differences in the `name`/`reference`/`description` text fields.
///
/// # Arguments
///
/// * `old` - Stored `[name, reference, description]` bytes.
/// * `new` - Desired `[name, reference, description]` bytes.
///
/// # Returns
///
/// * One `field: "old" -> "new"` line per differing field.
fn text_changes(old: [&[u8]; 3], new: [&[u8]; 3]) -> Vec<String> {
    TEXT_FIELDS
        .iter()
        .zip(old.into_iter().zip(new))
        .filter(|(_, (o, n))| o != n)
        .map(|(f, (o, n))| format!("{f}: {:?} -> {:?}", c_string_to_str(o), c_string_to_str(n)))
        .collect()
}

/// Compare one template kind, print the differences, and write them unless `dry_run`.
///
/// # Arguments
///
/// * `kind`    - Which template kind is being synced.
/// * `file`    - Desired templates from the snapshot file.
/// * `live`    - Templates currently stored in KeyDB.
/// * `texts`   - Returns a template's `[name, reference, description]` bytes.
/// * `con`     - Open KeyDB connection.
/// * `dry_run` - When `true`, nothing is written.
///
/// # Returns
///
/// * Number of templates that differed.
fn sync_kind<T: Debug + PartialEq + Encode>(
    kind: TemplateKind,
    file: &[T],
    live: &[T],
    texts: fn(&T) -> [&[u8]; 3],
    con: &mut redis::Connection,
    dry_run: bool,
) -> Result<usize, String> {
    if file.len() != live.len() {
        return Err(format!(
            "{} template count mismatch: file has {}, KeyDB has {}",
            kind.label(),
            file.len(),
            live.len()
        ));
    }

    let mut changed = 0;
    for (idx, (new, old)) in file.iter().zip(live).enumerate() {
        if new == old {
            continue;
        }
        changed += 1;
        println!(
            "{} template {idx} {:?}",
            kind.label(),
            c_string_to_str(texts(new)[0])
        );
        for line in text_changes(texts(old), texts(new))
            .into_iter()
            .chain(field_changes(old, new))
        {
            println!("    {line}");
        }
        if !dry_run {
            store::save_indexed_entities_range(con, kind.key_prefix(), &file[idx..=idx], idx)?;
        }
    }

    if changed > 0 && !dry_run {
        con.incr::<_, _, i64>(kind.version_key(), 1)
            .map_err(|e| format!("bump {} version: {e}", kind.label()))?;
    }
    Ok(changed)
}

/// Ask the running server to hot-reload templates and wait for confirmation.
///
/// # Arguments
///
/// * `con`   - Open KeyDB connection.
/// * `kinds` - Reload kinds (`"items"` / `"characters"`) to request.
///
/// # Returns
///
/// * `Ok(true)` once the server reports `applied`, `Ok(false)` on timeout.
fn request_reload(con: &mut redis::Connection, kinds: &[&str]) -> Result<bool, String> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?;
    let request_id = format!("tsync-{}", now.as_nanos());
    let payload = format!(
        r#"{{"request_id":"{request_id}","kinds":[{}],"requested_at":{}}}"#,
        kinds
            .iter()
            .map(|k| format!("\"{k}\""))
            .collect::<Vec<_>>()
            .join(","),
        now.as_secs()
    );
    con.set_ex::<_, _, ()>(template_store::RELOAD_REQUEST_KEY, payload, 30)
        .map_err(|e| format!("enqueue reload: {e}"))?;

    let status_key = template_store::reload_status_key(&request_id);
    let start = Instant::now();
    while start.elapsed() < RELOAD_WAIT {
        std::thread::sleep(Duration::from_millis(500));
        let status: Option<String> = con.get(&status_key).map_err(|e| e.to_string())?;
        if status.is_some_and(|s| s.starts_with("applied")) {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Run the sync described by `cli`.
///
/// # Arguments
///
/// * `cli` - Parsed command-line arguments.
///
/// # Returns
///
/// * `Ok(())` on success, `Err` with a message on any failure.
fn run(cli: Cli) -> Result<(), String> {
    let both = !cli.items && !cli.chars;
    let snapshot = WorldSnapshot::from_file(&cli.file)
        .map_err(|e| format!("failed to load {}: {e}", cli.file.display()))?;
    let mut con = connection::connect()?;

    let mut kinds: Vec<&str> = Vec::new();
    let (mut n_items, mut n_chars) = (0, 0);
    if cli.items || both {
        let live = store::load_item_templates(&mut con)?;
        n_items = sync_kind(
            TemplateKind::Item,
            &snapshot.item_templates,
            &live,
            |t: &Item| [&t.name, &t.reference, &t.description],
            &mut con,
            cli.dry_run,
        )?;
        if n_items > 0 {
            kinds.push("items");
        }
    }
    if cli.chars || both {
        let live = store::load_character_templates(&mut con)?;
        n_chars = sync_kind(
            TemplateKind::Character,
            &snapshot.character_templates,
            &live,
            |t: &Character| [&t.name, &t.reference, &t.description],
            &mut con,
            cli.dry_run,
        )?;
        if n_chars > 0 {
            kinds.push("characters");
        }
    }

    let verb = if cli.dry_run { "would update" } else { "updated" };
    println!("{verb} {n_items} item and {n_chars} character templates");

    if !cli.dry_run && !cli.no_reload && !kinds.is_empty() {
        match request_reload(&mut con, &kinds)? {
            true => println!("server confirmed reload"),
            false => println!("no reload confirmation from server (is it running?)"),
        }
    }
    Ok(())
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("template-sync: {e}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_only_changed_fields() {
        let old = Item::default();
        let mut new = old;
        new.skill[2][1] = 7;
        new.value = 50;
        new.name[0] = b'X';

        let mut diff = field_changes(&old, &new);
        diff.sort();
        assert_eq!(
            diff,
            vec![
                "skill[2][1]: 0 -> 7".to_owned(),
                "value: 0 -> 50".to_owned()
            ]
        );
        let text = text_changes(
            [&old.name, &old.reference, &old.description],
            [&new.name, &new.reference, &new.description],
        );
        assert_eq!(text.len(), 1);
        assert!(text[0].starts_with("name: "));
    }

    #[test]
    fn equal_values_have_no_changes() {
        let ch = Character::default();
        assert!(field_changes(&ch, &ch).is_empty());
    }
}
