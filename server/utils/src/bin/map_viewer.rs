mod map_viewer_app;

use eframe::egui;
use std::path::PathBuf;

use map_viewer_app::MapViewerApp;

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
        if arg == "--dat-dir" || arg == "--data-dir" {
            if let Some(dir) = args.next().map(PathBuf::from) {
                if dir.is_dir() {
                    return Some(dir);
                }
            }
            continue;
        }

        let dir = PathBuf::from(arg);
        if dir.is_dir() {
            return Some(dir);
        }
    }
    None
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
        Box::new(|_cc| Ok(Box::new(MapViewerApp::new()))),
    )
}
