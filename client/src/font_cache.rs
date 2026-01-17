use bevy::prelude::*;

/// Minimal font cache placeholder.
///
/// We don't currently ship any font assets in the repo; Bevy text rendering requires a font
/// handle, so this cache is intentionally conservative: it only loads a font if it exists
/// under `assets/fonts/`.
#[derive(Resource, Default)]
pub struct FontCache {
    ui_font: Option<Handle<Font>>,
    initialized: bool,
}

impl FontCache {
    pub fn ui_font(&self) -> Option<Handle<Font>> {
        self.ui_font.clone()
    }

    pub fn ensure_initialized(&mut self, asset_server: &AssetServer) {
        if self.initialized {
            return;
        }
        self.initialized = true;

        // If the user drops a font into `client/assets/fonts/ui.ttf`, we'll pick it up.
        let disk_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("assets")
            .join("fonts")
            .join("ui.ttf");

        if disk_path.exists() {
            self.ui_font = Some(asset_server.load("fonts/ui.ttf"));
        }
    }
}
