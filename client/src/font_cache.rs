use bevy::prelude::*;

use crate::gfx_cache::GraphicsCache;

pub const BITMAP_GLYPH_W: f32 = 6.0;
pub const BITMAP_GLYPH_H: f32 = 9.0;
pub const BITMAP_GLYPH_COUNT: usize = 96; // ASCII 32..=127 inclusive
pub const BITMAP_GLYPH_Y_OFFSET: f32 = 1.0; // dd.c uses +576 (one row) before glyph pixels
pub const BITMAP_FONT_FIRST_SPRITE_ID: usize = 700;
pub const BITMAP_FONT_COUNT: usize = 4;

/// Minimal font cache placeholder.
///
/// We don't currently ship any font assets in the repo; Bevy text rendering requires a font
/// handle, so this cache is intentionally conservative: it only loads a font if it exists
/// under `assets/fonts/`.
#[derive(Resource, Default)]
pub struct FontCache {
    ui_font: Option<Handle<Font>>,
    initialized: bool,

    bitmap_layout: Option<Handle<TextureAtlasLayout>>,
    bitmap_fonts: Vec<Option<Handle<Image>>>,
    bitmap_initialized: bool,
}

impl FontCache {
    pub fn ui_font(&self) -> Option<Handle<Font>> {
        self.ui_font.clone()
    }

    pub fn bitmap_layout(&self) -> Option<Handle<TextureAtlasLayout>> {
        self.bitmap_layout.clone()
    }

    pub fn bitmap_font_image(&self, font: usize) -> Option<Handle<Image>> {
        self.bitmap_fonts
            .get(font)
            .and_then(|h| h.as_ref())
            .cloned()
    }

    pub fn bitmap_glyph_index(ch: char) -> usize {
        let code = ch as u32;
        if !(32..=127).contains(&code) {
            return 0;
        }
        let idx = (code - 32) as usize;
        if idx >= BITMAP_GLYPH_COUNT {
            0
        } else {
            idx
        }
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

    pub fn ensure_bitmap_initialized(
        &mut self,
        gfx: &GraphicsCache,
        atlas_layouts: &mut Assets<TextureAtlasLayout>,
    ) {
        if self.bitmap_initialized {
            return;
        }

        // Create the shared 96-column atlas layout for all 4 fonts.
        if self.bitmap_layout.is_none() {
            let layout = TextureAtlasLayout::from_grid(
                UVec2::new(BITMAP_GLYPH_W as u32, BITMAP_GLYPH_H as u32),
                BITMAP_GLYPH_COUNT as u32,
                1,
                None,
                Some(UVec2::new(0, BITMAP_GLYPH_Y_OFFSET as u32)),
            );
            self.bitmap_layout = Some(atlas_layouts.add(layout));
        }

        if self.bitmap_fonts.is_empty() {
            self.bitmap_fonts.resize(BITMAP_FONT_COUNT, None);
        }

        // Pull the atlas images from the already-loaded GraphicsCache.
        let mut any_loaded = false;
        for font in 0..BITMAP_FONT_COUNT {
            let sprite_id = BITMAP_FONT_FIRST_SPRITE_ID + font;
            if let Some(sprite) = gfx.get_sprite(sprite_id) {
                self.bitmap_fonts[font] = Some(sprite.image.clone());
                any_loaded = true;
            }
        }

        // Mark initialized even if some are missing; callers can probe per-font handle.
        // We only require at least one to exist to avoid spamming repeated attempts.
        self.bitmap_initialized = any_loaded;
    }
}
