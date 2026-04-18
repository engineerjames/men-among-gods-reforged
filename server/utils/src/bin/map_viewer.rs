mod map_viewer_app;

use eframe::egui;
use std::path::PathBuf;

use map_viewer_app::MapViewerApp;

/// The data backend the viewer reads from.
#[derive(Clone, Debug, Default)]
enum DataSource {
    /// Load world data from a `.wsnap` snapshot file (read-only).
    Snapshot(PathBuf),
    /// Read/write via KeyDB at `redis://127.0.0.1:5556/`.
    #[default]
    KeyDb,
}

/// Return the path supplied after `--snapshot` on the command line, if any.
///
/// # Returns
///
/// * `Some(PathBuf)` when `--snapshot <path>` is present and the file exists.
/// * `None` otherwise.
fn snapshot_from_args() -> Option<PathBuf> {
    let mut args = std::env::args_os().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--snapshot" {
            if let Some(path) = args.next().map(PathBuf::from) {
                if path.is_file() {
                    return Some(path);
                }
            }
        }
    }
    None
}

/// Determine the data source from CLI arguments.
///
/// Defaults to [`DataSource::KeyDb`]. Pass `--snapshot <path>` to load a
/// `.wsnap` file for offline, read-only inspection instead.
///
/// # Returns
///
/// * The resolved [`DataSource`].
fn data_source_from_args() -> DataSource {
    if let Some(path) = snapshot_from_args() {
        DataSource::Snapshot(path)
    } else {
        DataSource::KeyDb
    }
}

fn default_graphics_zip_path() -> Option<PathBuf> {
    let crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let candidates = [
        crate_dir.join("../../client/assets/gfx/images.zip"),
        crate_dir.join("../../client/assets/gfx/images.ZIP"),
    ];

    candidates.into_iter().find(|candidate| candidate.is_file())
}

fn graphics_zip_from_args() -> Option<PathBuf> {
    let mut args = std::env::args_os().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--graphics-zip" || arg == "--gfx-zip" {
            if let Some(path) = args.next().map(PathBuf::from) {
                if path.is_file() {
                    return Some(path);
                }
            }
            continue;
        }
    }

    None
}

fn main() -> Result<(), eframe::Error> {
    env_logger::init();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1400.0, 900.0])
            .with_title("Map Viewer"),
        ..Default::default()
    };

    eframe::run_native(
        "Map Viewer",
        options,
        Box::new(|_cc| {
            let data_source = data_source_from_args();
            Ok(Box::new(MapViewerApp::new(data_source)))
        }),
    )
}
