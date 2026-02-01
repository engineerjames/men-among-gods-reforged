use eframe::egui;
use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use zip::ZipArchive;

pub(crate) struct GraphicsZipCache {
    zip_path: PathBuf,
    entries: HashMap<usize, String>,
    textures: HashMap<usize, egui::TextureHandle>,
    sprite_tiles: HashMap<usize, (i32, i32)>,
    // Cache decoded images to avoid re-decoding on every frame
    decoded_cache: HashMap<usize, Vec<u8>>,
    decoded_dims: HashMap<usize, (u32, u32)>,
}

impl GraphicsZipCache {
    pub(crate) fn load(zip_path: PathBuf) -> Result<Self, String> {
        let file = File::open(&zip_path)
            .map_err(|e| format!("Failed to open graphics zip {:?}: {e}", zip_path))?;
        let mut archive =
            ZipArchive::new(file).map_err(|e| format!("Failed to read zip {:?}: {e}", zip_path))?;

        let mut entries: HashMap<usize, String> = HashMap::new();
        for i in 0..archive.len() {
            let Ok(file) = archive.by_index(i) else {
                continue;
            };

            let name = file.name().to_string();
            if name.ends_with('/') {
                continue;
            }

            let stem = Path::new(&name)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("");
            if let Ok(id) = stem.parse::<usize>() {
                entries.insert(id, name);
            }
        }

        log::info!(
            "GraphicsZipCache loaded {:?} ({} indexed sprites)",
            zip_path,
            entries.len()
        );

        Ok(Self {
            zip_path,
            entries,
            textures: HashMap::new(),
            sprite_tiles: HashMap::new(),
            decoded_cache: HashMap::new(),
            decoded_dims: HashMap::new(),
        })
    }

    #[allow(dead_code)]
    pub(crate) fn sprite_tiles_xy(
        &mut self,
        ctx: &egui::Context,
        sprite_id: usize,
    ) -> Result<Option<(i32, i32)>, String> {
        let _ = self.texture_for(ctx, sprite_id)?;
        Ok(self.sprite_tiles.get(&sprite_id).copied())
    }

    pub(crate) fn texture_for(
        &mut self,
        ctx: &egui::Context,
        sprite_id: usize,
    ) -> Result<Option<&egui::TextureHandle>, String> {
        if self.textures.contains_key(&sprite_id) {
            return Ok(self.textures.get(&sprite_id));
        }

        let Some(entry_name) = self.entries.get(&sprite_id).cloned() else {
            return Ok(None);
        };

        // Check if we have decoded bytes cached
        let (pixels, w, h) = if let Some(cached_bytes) = self.decoded_cache.get(&sprite_id) {
            let (w, h) = self.decoded_dims.get(&sprite_id).copied().unwrap_or((1, 1));
            (cached_bytes.clone(), w, h)
        } else {
            // Load and decode from ZIP
            let file = File::open(&self.zip_path)
                .map_err(|e| format!("Failed to open graphics zip {:?}: {e}", self.zip_path))?;
            let mut archive = ZipArchive::new(file)
                .map_err(|e| format!("Failed to read graphics zip {:?}: {e}", self.zip_path))?;

            let mut entry = archive
                .by_name(&entry_name)
                .map_err(|e| format!("Failed to read zip entry {:?}: {e}", entry_name))?;

            let mut bytes = Vec::new();
            entry
                .read_to_end(&mut bytes)
                .map_err(|e| format!("Failed to read zip entry {:?} bytes: {e}", entry_name))?;

            let decoded = image::load_from_memory(&bytes)
                .map_err(|e| format!("Failed to decode {:?}: {e}", entry_name))?;
            let rgba = decoded.to_rgba8();
            let (w, h) = rgba.dimensions();

            let pixels = rgba.into_raw();

            // Cache the decoded pixels and dimensions for future frames
            self.decoded_cache.insert(sprite_id, pixels.clone());
            self.decoded_dims.insert(sprite_id, (w, h));

            (pixels, w, h)
        };

        // dd.c tile dimensions in 32x32 blocks.
        let w_i = (w.max(1) as i32).max(1);
        let h_i = (h.max(1) as i32).max(1);
        let xs = ((w_i + 31) / 32).max(1);
        let ys = ((h_i + 31) / 32).max(1);

        let color =
            egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], pixels.as_slice());

        let texture = ctx.load_texture(
            format!("sprite:{}:{}", self.zip_path.display(), sprite_id),
            color,
            egui::TextureOptions::NEAREST,
        );
        self.textures.insert(sprite_id, texture);
        self.sprite_tiles.insert(sprite_id, (xs, ys));

        Ok(self.textures.get(&sprite_id))
    }
}
