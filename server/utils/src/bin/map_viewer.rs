mod map_viewer_app;

use eframe::egui;

use map_viewer_app::MapViewerApp;

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
            let data_source = server_utils::data_source_from_args();
            Ok(Box::new(MapViewerApp::new(data_source)))
        }),
    )
}
