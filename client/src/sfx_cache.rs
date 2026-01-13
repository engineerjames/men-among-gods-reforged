use std::path::PathBuf;

use bevy::{ecs::resource::Resource, sprite::Sprite};

use crate::gfx_cache::CacheInitStatus;

#[derive(Debug, Default)]
struct InitState {
    entries: Vec<PathBuf>,
    index: usize,
}

#[derive(Resource, Default)]
#[allow(dead_code)]
pub struct SoundCache {
    assets_zip: PathBuf,
    sfx: Vec<Sprite>,
    initialized: bool,
    init_state: Option<InitState>,
    init_error: Option<String>,
}

impl SoundCache {
    pub fn new(assets_zip: &str) -> Self {
        Self {
            assets_zip: PathBuf::from(assets_zip),
            sfx: Vec::new(),
            initialized: false,
            init_state: None,
            init_error: None,
        }
    }

    pub fn reset_loading(&mut self) {
        self.sfx.clear();
        self.initialized = false;
        self.init_state = None;
        self.init_error = None;
    }

    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    /// Incrementally initializes the cache by walking a directory.
    ///
    /// This currently treats `assets_zip` as a folder path (e.g. `.../assets/SFX`).
    pub fn initialize(&mut self) -> CacheInitStatus {
        if self.initialized {
            return CacheInitStatus::Done;
        }

        if let Some(err) = self.init_error.clone() {
            return CacheInitStatus::Error(err);
        }

        if self.init_state.is_none() {
            let mut entries = Vec::new();
            match std::fs::read_dir(&self.assets_zip) {
                Ok(dir) => {
                    for entry in dir.flatten() {
                        let path = entry.path();
                        if path.is_file() {
                            entries.push(path);
                        }
                    }
                }
                Err(e) => {
                    let err = format!("Failed to read sounds directory {:?}: {e}", self.assets_zip);
                    self.init_error = Some(err.clone());
                    return CacheInitStatus::Error(err);
                }
            }

            self.init_state = Some(InitState { entries, index: 0 });
        }

        let state = self.init_state.as_mut().unwrap();
        if state.entries.is_empty() {
            self.initialized = true;
            self.init_state = None;
            return CacheInitStatus::Done;
        }

        if state.index >= state.entries.len() {
            self.initialized = true;
            self.init_state = None;
            return CacheInitStatus::Done;
        }

        // Placeholder: in the future this is where we'd decode audio & cache handles.
        self.sfx.push(Sprite::default());
        state.index += 1;

        let progress = state.index as f32 / state.entries.len() as f32;
        CacheInitStatus::InProgress { progress }
    }
}
