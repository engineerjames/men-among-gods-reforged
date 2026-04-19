use std::ffi::OsString;
use std::path::{Path, PathBuf};

use server::snapshot::WorldSnapshot;

/// The world-data source backing a viewer session.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum DataSource {
    /// Read the latest persisted world state from KeyDB.
    ///
    /// Viewers must treat this source as read-only because the running server
    /// owns the authoritative in-memory state and periodically overwrites
    /// KeyDB from its background saver.
    #[default]
    LiveKeyDbReadOnly,

    /// Read and edit a portable `.wsnap` snapshot file.
    SnapshotFile(PathBuf),
}

impl DataSource {
    /// Return whether this source should allow world-data edits.
    ///
    /// # Returns
    ///
    /// * `true` for snapshot-backed sessions.
    /// * `false` for live KeyDB sessions.
    pub fn can_edit(&self) -> bool {
        matches!(self, Self::SnapshotFile(_))
    }

    /// Return whether this source points at live KeyDB state.
    ///
    /// # Returns
    ///
    /// * `true` when the viewer is reading persisted KeyDB state.
    /// * `false` otherwise.
    pub fn is_live_keydb(&self) -> bool {
        matches!(self, Self::LiveKeyDbReadOnly)
    }

    /// Return the snapshot path when this source is file-backed.
    ///
    /// # Returns
    ///
    /// * `Some(&Path)` for snapshot-backed sessions.
    /// * `None` for live KeyDB sessions.
    pub fn snapshot_path(&self) -> Option<&Path> {
        match self {
            Self::SnapshotFile(path) => Some(path.as_path()),
            Self::LiveKeyDbReadOnly => None,
        }
    }

    /// Return a short human-readable description of the source.
    ///
    /// # Returns
    ///
    /// * A label suitable for toolbars and status text.
    pub fn display_label(&self) -> String {
        match self {
            Self::LiveKeyDbReadOnly => "Live KeyDB (read-only)".to_string(),
            Self::SnapshotFile(path) => format!("Snapshot: {}", path.display()),
        }
    }
}

/// Determine the data source from the current process arguments.
///
/// Defaults to [`DataSource::LiveKeyDbReadOnly`]. Pass `--snapshot <path>` to
/// load a `.wsnap` file for editable offline work instead.
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
        DataSource::LiveKeyDbReadOnly => load_world_snapshot_from_keydb(),
        DataSource::SnapshotFile(path) => WorldSnapshot::from_file(path),
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
    path_arg_from_iter(args, &["--snapshot"])
        .map(DataSource::SnapshotFile)
        .unwrap_or_default()
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

fn load_world_snapshot_from_keydb() -> Result<WorldSnapshot, String> {
    let mut con = server::keydb::connect()?;
    let data = server::keydb_store::load_all(&mut con)?;
    Ok(WorldSnapshot::new(
        data.map,
        data.items,
        data.item_templates,
        data.characters,
        data.character_templates,
        data.effects,
        data.globals,
        data.bad_names,
        data.bad_words,
        data.message_of_the_day,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unique_test_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "men_among_gods_reforged_{name}_{}_{}.wsnap",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
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

    /// Default to live KeyDB mode when no valid snapshot path is supplied.
    #[test]
    fn args_default_to_live_keydb() {
        let source = data_source_from_iter([
            OsString::from("--snapshot"),
            OsString::from("/missing/file.wsnap"),
        ]);

        assert_eq!(source, DataSource::LiveKeyDbReadOnly);
    }

    /// Expose editing only for snapshot-backed sessions.
    #[test]
    fn snapshot_sources_are_editable() {
        assert!(DataSource::SnapshotFile(PathBuf::from("world.wsnap")).can_edit());
        assert!(!DataSource::LiveKeyDbReadOnly.can_edit());
    }
}
