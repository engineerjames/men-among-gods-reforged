use std::ffi::OsString;
use std::path::{Path, PathBuf};

use server::keydb::snapshot::WorldSnapshot;

/// The world-data source backing a viewer session.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum DataSource {
    /// No source is configured yet.  The viewers will not attempt to load
    /// any world data until the user opens a snapshot or connects to the API.
    #[default]
    NotLoaded,

    /// Read and edit a portable `.wsnap` snapshot file.
    SnapshotFile(PathBuf),

    /// Read and edit live templates via the admin API of a running server.
    ///
    /// Phase 1: only item and character templates are exposed in this mode.
    /// Other slices (items, characters, map) are returned empty.
    LiveApi {
        /// Base URL of the API service (e.g. `https://127.0.0.1:5554`).
        base_url: String,
        /// Static admin bearer token sent in `Authorization`.
        token: String,
    },
}

impl DataSource {
    /// Return whether this source points at a running admin API.
    ///
    /// # Returns
    ///
    /// * `true` when the viewer is connected to an admin API.
    /// * `false` otherwise.
    pub fn is_live_api(&self) -> bool {
        matches!(self, Self::LiveApi { .. })
    }

    /// Return the snapshot path when this source is file-backed.
    ///
    /// # Returns
    ///
    /// * `Some(&Path)` for snapshot-backed sessions.
    /// * `None` otherwise.
    pub fn snapshot_path(&self) -> Option<&Path> {
        match self {
            Self::SnapshotFile(path) => Some(path.as_path()),
            Self::NotLoaded | Self::LiveApi { .. } => None,
        }
    }

    /// Return a short human-readable description of the source.
    ///
    /// # Returns
    ///
    /// * A label suitable for toolbars and status text.
    pub fn display_label(&self) -> String {
        match self {
            Self::NotLoaded => "None".to_owned(),
            Self::SnapshotFile(path) => format!("Snapshot: {}", path.display()),
            Self::LiveApi { base_url, .. } => format!("Live API: {}", base_url),
        }
    }
}

/// Determine the data source from the current process arguments.
///
/// Defaults to [`DataSource::NotLoaded`] when no flags are supplied.
/// Pass `--snapshot <path>` to load a `.wsnap` file, or `--api <url>`
/// (with optional `--admin-token <tok>`) to connect to a running admin API.
///
/// # Returns
///
/// * The resolved [`DataSource`].
pub fn data_source_from_args() -> DataSource {
    data_source_from_iter(std::env::args_os().skip(1))
}

/// Return the default graphics archive path when one exists in the workspace.
///
/// # Returns
///
/// * `Some(PathBuf)` when `client/assets/gfx/images.zip` exists.
/// * `None` otherwise.
pub fn default_graphics_zip_path() -> Option<PathBuf> {
    let crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let candidates = [
        crate_dir.join("../../client/assets/gfx/images.zip"),
        crate_dir.join("../../client/assets/gfx/images.ZIP"),
    ];

    candidates.into_iter().find(|candidate| candidate.is_file())
}

/// Return the graphics archive path supplied on the command line, if any.
///
/// Supports `--graphics-zip <path>` and `--gfx-zip <path>`.
///
/// # Returns
///
/// * `Some(PathBuf)` when a valid graphics archive path is present.
/// * `None` otherwise.
pub fn graphics_zip_from_args() -> Option<PathBuf> {
    path_arg_from_iter(
        std::env::args_os().skip(1),
        &["--graphics-zip", "--gfx-zip"],
    )
}

/// Load a complete world snapshot from the supplied data source.
///
/// For live KeyDB sources, this reads the latest persisted data from KeyDB and
/// shapes it into a [`WorldSnapshot`]. For snapshot sources, it decodes the
/// file directly.
///
/// # Arguments
///
/// * `source` - The world-data source to load.
///
/// # Returns
///
/// * `Ok(WorldSnapshot)` on success.
/// * `Err(String)` when the source cannot be read or decoded.
pub fn load_world_snapshot(source: &DataSource) -> Result<WorldSnapshot, String> {
    match source {
        DataSource::NotLoaded => Err("No source configured".to_owned()),
        DataSource::SnapshotFile(path) => WorldSnapshot::from_file(path),
        DataSource::LiveApi { base_url, token } => load_world_snapshot_from_api(base_url, token),
    }
}

/// Write a complete world snapshot to disk.
///
/// # Arguments
///
/// * `snapshot` - The snapshot to write.
/// * `path` - Destination `.wsnap` file path.
///
/// # Returns
///
/// * `Ok(())` on success.
/// * `Err(String)` on I/O or encode failure.
pub fn save_world_snapshot(snapshot: &WorldSnapshot, path: &Path) -> Result<(), String> {
    snapshot.to_file(path)
}

fn data_source_from_iter<I>(args: I) -> DataSource
where
    I: IntoIterator<Item = OsString>,
{
    let args: Vec<OsString> = args.into_iter().collect();

    // --api <url> [--admin-token <tok>] takes precedence over --snapshot.
    if let Some(base_url) = string_arg_from_slice(&args, &["--api", "--admin-api"]) {
        let token = string_arg_from_slice(&args, &["--admin-token", "--api-token"])
            .or_else(|| std::env::var("MAG_ADMIN_API_TOKEN").ok())
            .unwrap_or_default();
        return DataSource::LiveApi { base_url, token };
    }

    path_arg_from_iter(args, &["--snapshot"])
        .map(DataSource::SnapshotFile)
        .unwrap_or_default()
}

fn string_arg_from_slice(args: &[OsString], flag_names: &[&str]) -> Option<String> {
    for window in args.windows(2) {
        let flag = window[0].to_string_lossy();
        if flag_names.iter().any(|candidate| *candidate == flag) {
            return Some(window[1].to_string_lossy().into_owned());
        }
    }
    None
}

fn path_arg_from_iter<I>(args: I, flag_names: &[&str]) -> Option<PathBuf>
where
    I: IntoIterator<Item = OsString>,
{
    let args: Vec<OsString> = args.into_iter().collect();
    for window in args.windows(2) {
        let flag = window[0].to_string_lossy();
        if flag_names.iter().any(|candidate| *candidate == flag) {
            let path = PathBuf::from(&window[1]);
            if path.is_file() {
                return Some(path);
            }
        }
    }

    None
}

/// Phase 1 LiveApi snapshot loader: fetches only lightweight summaries for
/// item and character templates (2 HTTP requests total). Full template data
/// is **not** loaded here — call
/// [`AdminClient::fetch_single_item_template`] /
/// [`AdminClient::fetch_single_character_template`] on demand when a slot
/// is selected to avoid hammering the 1 req/sec rate-limiter.
///
/// Each slot in the returned vecs has `used` and `name` populated from the
/// summary. All other fields are zeroed/default and should be replaced with
/// the result of a single-slot fetch before the slot is displayed in detail.
///
/// # Arguments
///
/// * `base_url` - Base URL of the admin API.
/// * `token` - Static admin bearer token.
///
/// # Returns
///
/// * `Ok(WorldSnapshot)` with stub templates and all other slices empty.
/// * `Err(message)` on HTTP or decode failure.
fn load_world_snapshot_from_api(base_url: &str, token: &str) -> Result<WorldSnapshot, String> {
    use mag_core::constants::{MAXTCHARS, MAXTITEM};
    use mag_core::string_operations::write_ascii_into_fixed;
    use mag_core::types::{Character, Item};

    if token.is_empty() {
        return Err(
            "Admin token is empty. Pass --admin-token <tok> or set MAG_ADMIN_API_TOKEN.".to_owned(),
        );
    }
    let client = crate::admin_client::AdminClient::new(base_url, token)?;

    // Two requests for the template summaries, plus three bulk requests for
    // the live world state (map, items, characters).
    let item_list = client.fetch_item_template_summaries()?;
    let char_list = client.fetch_character_template_summaries()?;
    let map_tiles = client.fetch_map_tiles()?;
    let items = client.fetch_items()?;
    let characters = client.fetch_characters()?;

    // Build placeholder vecs the same length as the full template arrays so
    // the sidebar list renders correctly. Only name/used are populated; the
    // rest stays zeroed until the slot is actually opened.
    let mut item_templates: Vec<Item> = vec![Item::default(); MAXTITEM];
    for summary in &item_list.items {
        if summary.id < MAXTITEM {
            let slot = &mut item_templates[summary.id];
            slot.used = u8::from(summary.used);
            write_ascii_into_fixed(&mut slot.name, &summary.name);
        }
    }

    let mut character_templates: Vec<Character> = vec![Character::default(); MAXTCHARS];
    for summary in &char_list.items {
        if summary.id < MAXTCHARS {
            let slot = &mut character_templates[summary.id];
            slot.used = u8::from(summary.used);
            write_ascii_into_fixed(&mut slot.name, &summary.name);
        }
    }

    Ok(WorldSnapshot::new(
        map_tiles,
        items,
        item_templates,
        characters,
        character_templates,
        Vec::new(),
        mag_core::types::Global::default(),
        Vec::new(),
        Vec::new(),
        String::new(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Convert arbitrary test names into a filename-safe path component.
    ///
    /// # Arguments
    ///
    /// * `value` - Raw string that may contain characters invalid in filenames.
    ///
    /// # Returns
    ///
    /// * A string containing only ASCII letters, digits, `_`, `-`, and `.`.
    fn sanitize_filename_component(value: &str) -> String {
        value
            .chars()
            .map(|ch| match ch {
                'a'..='z' | 'A'..='Z' | '0'..='9' | '_' | '-' | '.' => ch,
                _ => '_',
            })
            .collect()
    }

    fn unique_test_path(name: &str) -> PathBuf {
        let thread_name =
            sanitize_filename_component(std::thread::current().name().unwrap_or("test"));
        std::env::temp_dir().join(format!(
            "men_among_gods_reforged_{name}_{}_{}.wsnap",
            std::process::id(),
            thread_name
        ))
    }

    /// Prefer snapshot mode when a valid `--snapshot` argument is present.
    #[test]
    fn args_resolve_snapshot_source() {
        let path = unique_test_path("snapshot_source");
        std::fs::write(&path, b"test").expect("create test snapshot path");

        let source =
            data_source_from_iter([OsString::from("--snapshot"), path.clone().into_os_string()]);

        assert_eq!(source, DataSource::SnapshotFile(path.clone()));

        std::fs::remove_file(&path).expect("remove test snapshot path");
    }

    /// Default to `NotLoaded` when no valid snapshot path is supplied.
    #[test]
    fn args_default_to_not_loaded() {
        let source = data_source_from_iter([
            OsString::from("--snapshot"),
            OsString::from("/missing/file.wsnap"),
        ]);

        assert_eq!(source, DataSource::NotLoaded);
    }

    /// Replace path-hostile characters so generated temp files work on Windows.
    #[test]
    fn sanitize_filename_component_replaces_colons() {
        assert_eq!(
            sanitize_filename_component("viewer_support::tests::args_resolve_snapshot_source"),
            "viewer_support__tests__args_resolve_snapshot_source"
        );
    }
}
