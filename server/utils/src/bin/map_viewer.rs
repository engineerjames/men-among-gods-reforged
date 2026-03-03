mod map_viewer_app;

use eframe::egui;
use std::path::PathBuf;

use map_viewer_app::MapViewerApp;

/// The data backend the viewer reads from and writes to.
#[derive(Clone, Debug, Default)]
enum DataSource {
    /// Read/write from `.dat` files in the given directory.
    DatFiles(PathBuf),
    /// Read/write via KeyDB at `redis://127.0.0.1:5556/`.
    #[default]
    KeyDb,
}

fn default_dat_dir() -> Option<PathBuf> {
    let crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let candidate = crate_dir.join("../assets/.dat");
    if candidate.is_dir() {
        return Some(candidate);
    }

    None
}

fn dat_dir_from_args() -> Option<PathBuf> {
    let mut args = std::env::args_os().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--dat-dir" || arg == "--data-dir" || arg == "--dat" {
            if let Some(dir) = args.next().map(PathBuf::from) {
                if dir.is_dir() {
                    return Some(dir);
                }
            }
            // --dat with no directory: fall back to default
            return default_dat_dir();
        }

        let dir = PathBuf::from(arg);
        if dir.is_dir() {
            return Some(dir);
        }
    }
    None
}

/// Determine the data source from CLI arguments.
///
/// Defaults to [`DataSource::KeyDb`]. Pass `--dat [path]` to use `.dat` files
/// instead.
///
/// # Returns
///
/// * The resolved [`DataSource`].
fn data_source_from_args() -> DataSource {
    if let Some(dir) = dat_dir_from_args() {
        DataSource::DatFiles(dir)
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
