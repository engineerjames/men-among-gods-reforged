use std::{fs::File, path::PathBuf};

use bevy::{ecs::resource::Resource, sprite::Sprite};
use zip::ZipArchive;

#[derive(Debug, Clone)]
pub enum CacheInitStatus {
    InProgress { progress: f32 },
    Done,
    Error(String),
}

#[derive(Debug, Default)]
struct InitState {
    entries: Vec<String>,
    index: usize,
}

/// A cache for graphical assets loaded from a zip file. Currently
/// we do the very slow operation of extracting the zip file contents
/// every time the game starts. This is a placeholder implementation.
#[derive(Resource, Default)]
#[allow(dead_code)]
pub struct GraphicsCache {
    assets_zip: PathBuf,
    gfx: Vec<Sprite>,
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
            initialized: false,
            init_state: None,
            init_error: None,
            archive: None,
        }
    }

    pub fn reset_loading(&mut self) {
        self.gfx.clear();
        self.initialized = false;
        self.init_state = None;
        self.init_error = None;
        self.archive = None;
    }

    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    pub fn initialize(&mut self) -> CacheInitStatus {
        if self.initialized {
            log::warn!("GraphicsCache::initialize called but already initialized");
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
                        entries.push(name);
                    }
                }
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

        // Placeholder: in the future this is where we'd decode the image and cache a handle.
        self.gfx.push(Sprite::default());
        state.index += 1;

        let progress = state.index as f32 / state.entries.len() as f32;
        CacheInitStatus::InProgress { progress }
    }
}
