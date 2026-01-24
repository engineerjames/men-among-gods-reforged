pub(crate) mod app;

// Reuse the existing graphics zip cache used by template_viewer.
#[path = "../template_viewer_app/graphics.rs"]
pub(crate) mod graphics;

pub(crate) use app::MapViewerApp;
