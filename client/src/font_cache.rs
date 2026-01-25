use bevy::prelude::*;

use crate::gfx_cache::GraphicsCache;

pub const BITMAP_GLYPH_W: f32 = 6.0;
pub const BITMAP_GLYPH_H: f32 = 9.0;
pub const BITMAP_GLYPH_COUNT: usize = 96; // ASCII 32..=127 inclusive
pub const BITMAP_GLYPH_Y_OFFSET: f32 = 1.0; // dd.c uses +576 (one row) before glyph pixels
pub const BITMAP_FONT_FIRST_SPRITE_ID: usize = 700;
pub const BITMAP_FONT_COUNT: usize = 4;

#[derive(Resource, Default)]
pub struct FontCache {
    bitmap_layout: Option<Handle<TextureAtlasLayout>>,
    bitmap_fonts: Vec<Option<Handle<Image>>>,
    bitmap_initialized: bool,
}

impl FontCache {
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

    pub fn ensure_bitmap_initialized(
        &mut self,
        gfx: &GraphicsCache,
        atlas_layouts: &mut Assets<TextureAtlasLayout>,
    ) {
        if self.bitmap_initialized {
            return;
        }

        // Create the shared 96-column atlas layout for all 4 fonts.
        //
        // Important: `TextureAtlasLayout::from_grid` does NOT include `offset` in `layout.size`.
        // Because we use a Y offset (the glyph row starts at y=1), that would produce UVs with
        // `v > 1.0` and cause subtle edge artifacts.
        //
        // Additionally, the original font sheet uses 6px cells but effectively 5px glyphs with a
        // 1px spacer column; sampling the full 6px width can show "next glyph" bleed on the
        // right edge depending on exact screen placement. We therefore crop the atlas rect width
        // to 5px while keeping the 6px advance.
        if self.bitmap_layout.is_none() {
            let cell_w = BITMAP_GLYPH_W as u32;
            let cell_h = BITMAP_GLYPH_H as u32;
            let offset_y = BITMAP_GLYPH_Y_OFFSET as u32;

            let mut layout = TextureAtlasLayout::new_empty(UVec2::new(
                cell_w * BITMAP_GLYPH_COUNT as u32,
                offset_y + cell_h,
            ));

            for i in 0..BITMAP_GLYPH_COUNT as u32 {
                let x = i * cell_w;
                let min = UVec2::new(x, offset_y);
                let max = UVec2::new(
                    x + cell_w.saturating_sub(1),
                    offset_y + cell_h.saturating_sub(1),
                );
                layout.add_texture(URect { min, max });
            }

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
