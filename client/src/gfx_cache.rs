use std::{fs::File, io::Read, path::PathBuf};

use bevy::{
    asset::RenderAssetUsages,
    ecs::resource::Resource,
    image::{CompressedImageFormats, Image, ImageSampler, ImageType},
    prelude::Assets,
    sprite::Sprite,
};
use zip::ZipArchive;

#[derive(Debug, Clone)]
pub enum CacheInitStatus {
    InProgress { progress: f32 },
    Done,
    Error(String),
}

#[derive(Debug, Default)]
struct InitState {
    entries: Vec<(usize, String)>,
    index: usize,
}

/// A cache for graphical assets loaded from a zip file. Currently
/// we do the very slow operation of extracting the zip file contents
/// every time the game starts. This is a placeholder implementation.
#[derive(Resource, Default)]
pub struct GraphicsCache {
    assets_zip: PathBuf,
    gfx: Vec<Option<Sprite>>,
    sprite_tiles: Vec<Option<(i32, i32)>>,
    initialized: bool,
    init_state: Option<InitState>,
    init_error: Option<String>,
    archive: Option<ZipArchive<File>>,
}

impl GraphicsCache {
    pub fn new(assets_zip: &str) -> Self {
        Self {
            assets_zip: PathBuf::from(assets_zip),
            gfx: Vec::new(),
            sprite_tiles: Vec::new(),
            initialized: false,
            init_state: None,
            init_error: None,
            archive: None,
        }
    }

    pub fn reset_loading(&mut self) {
        self.gfx.clear();
        self.sprite_tiles.clear();
        self.initialized = false;
        self.init_state = None;
        self.init_error = None;
        self.archive = None;
    }

    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    pub fn get_sprite(&self, index: usize) -> Option<&Sprite> {
        self.gfx.get(index).and_then(|s| s.as_ref())
    }

    pub fn get_sprite_tiles_xy(&self, index: usize) -> Option<(i32, i32)> {
        self.sprite_tiles
            .get(index)
            .and_then(|v| v.as_ref())
            .copied()
    }

    pub fn initialize(&mut self, images_assets: &mut Assets<Image>) -> CacheInitStatus {
        if self.initialized {
            return CacheInitStatus::Done;
        }

        if let Some(err) = self.init_error.clone() {
            log::error!(
                "GraphicsCache::initialize encountered previous error: {}",
                err
            );
            return CacheInitStatus::Error(err);
        }

        if self.init_state.is_none() {
            let file = match File::open(&self.assets_zip) {
                Ok(f) => f,
                Err(e) => {
                    let err = format!("Failed to open graphics zip {:?}: {e}", self.assets_zip);
                    self.init_error = Some(err.clone());

                    log::error!("GraphicsCache::initialize failed to open zip file: {}", err);
                    return CacheInitStatus::Error(err);
                }
            };

            log::info!(
                "GraphicsCache::initialize opened graphics zip {:?}",
                self.assets_zip
            );

            self.archive = match ZipArchive::new(file) {
                Ok(a) => Some(a),
                Err(e) => {
                    let err = format!("Failed to read graphics zip {:?}: {e}", self.assets_zip);
                    self.init_error = Some(err.clone());

                    log::error!(
                        "GraphicsCache::initialize failed to read zip archive: {}",
                        err
                    );
                    return CacheInitStatus::Error(err);
                }
            };

            let mut entries = Vec::new();
            for i in 0..self.archive.as_ref().unwrap().len() {
                if let Ok(file) = self.archive.as_mut().unwrap().by_index(i) {
                    let name = file.name().to_string();
                    // Skip directory entries
                    if !name.ends_with('/') {
                        // Our sprite IDs are numeric filenames (e.g. 00031.png). Some zip builds
                        // include a directory prefix (e.g. images/00031.png), so parse only the
                        // final path component.
                        let file_name = std::path::Path::new(&name)
                            .file_name()
                            .and_then(|s| s.to_str())
                            .unwrap_or("");
                        let stem = file_name.split('.').next().unwrap_or("");
                        if let Ok(id) = stem.parse::<usize>() {
                            entries.push((id, name));
                        }
                    }
                }
            }

            entries.sort_by_key(|(id, _)| *id);

            // Allocate a sparse vector where `gfx[id]` exists.
            if let Some((max_id, _)) = entries.last() {
                self.gfx.resize(max_id.saturating_add(1), None);
                self.sprite_tiles.resize(max_id.saturating_add(1), None);
            }

            log::info!(
                "GraphicsCache::initialize found {} entries in zip file",
                entries.len()
            );
            self.init_state = Some(InitState { entries, index: 0 });
        }

        let state = self.init_state.as_mut().unwrap();

        if state.entries.is_empty() {
            self.initialized = true;
            self.init_state = None;

            log::error!("GraphicsCache::initialize completed with no entries");
            return CacheInitStatus::Done;
        }

        if state.index >= state.entries.len() {
            self.initialized = true;
            self.init_state = None;

            log::info!("GraphicsCache::initialize completed successfully");
            return CacheInitStatus::Done;
        }

        let (sprite_id, entry_name) = &state.entries[state.index];
        log::debug!(
            "GraphicsCache::initialize loading entry {}/{}: {}",
            state.index + 1,
            state.entries.len(),
            entry_name
        );

        let mut file = match self.archive.as_mut().unwrap().by_name(entry_name) {
            Ok(f) => f,
            Err(e) => {
                let err = format!(
                    "Failed to read graphics entry {:?} from zip: {e}",
                    entry_name
                );
                self.init_error = Some(err.clone());

                log::error!(
                    "GraphicsCache::initialize failed to read entry from zip: {}",
                    err
                );
                return CacheInitStatus::Error(err);
            }
        };

        let file_bytes = {
            let mut buf = Vec::new();
            if let Err(e) = file.read_to_end(&mut buf) {
                let err = format!("Failed to read graphics entry {:?} data: {e}", entry_name);
                self.init_error = Some(err.clone());

                log::error!(
                    "GraphicsCache::initialize failed to read entry data: {}",
                    err
                );
                return CacheInitStatus::Error(err);
            }
            buf
        };

        let ext = entry_name
            .rsplit('.')
            .next()
            .unwrap_or("png")
            .to_ascii_lowercase();

        let image = match Image::from_buffer(
            &file_bytes,
            ImageType::Extension(ext.as_str()),
            CompressedImageFormats::default(),
            true,
            ImageSampler::nearest(),
            RenderAssetUsages::default(),
        ) {
            Ok(img) => img,
            Err(e) => {
                let err = format!(
                    "Failed to decode image entry {:?} (ext={}) from zip: {e}",
                    entry_name, ext
                );
                self.init_error = Some(err.clone());
                log::error!("GraphicsCache::initialize decode failed: {}", err);
                return CacheInitStatus::Error(err);
            }
        };

        // Cache dd.c tile dimensions (in 32x32 blocks). This avoids per-frame image-size queries.
        let size = image.size();
        let w = (size.x.max(1) as i32).max(1);
        let h = (size.y.max(1) as i32).max(1);
        let xs = ((w + 31) / 32).max(1);
        let ys = ((h + 31) / 32).max(1);

        let image_handle = images_assets.add(image);
        if *sprite_id >= self.gfx.len() {
            self.gfx.resize(sprite_id.saturating_add(1), None);
            self.sprite_tiles.resize(sprite_id.saturating_add(1), None);
        }
        self.gfx[*sprite_id] = Some(Sprite::from_image(image_handle));
        self.sprite_tiles[*sprite_id] = Some((xs, ys));
        state.index += 1;

        let progress = state.index as f32 / state.entries.len() as f32;
        CacheInitStatus::InProgress { progress }
    }
}
