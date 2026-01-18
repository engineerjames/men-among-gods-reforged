use std::path::PathBuf;

use bevy::{
    asset::{Assets, Handle},
    audio::AudioSource,
    ecs::resource::Resource,
};

use crate::gfx_cache::CacheInitStatus;

#[derive(Debug, Default)]
struct InitState {
    entries: Vec<PathBuf>,
    index: usize,
}

#[derive(Resource, Default)]
#[allow(dead_code)]
pub struct SoundCache {
    assets_directory: PathBuf,
    sfx_by_number: Vec<Option<Handle<AudioSource>>>,
    click: Option<Handle<AudioSource>>,
    initialized: bool,
    init_state: Option<InitState>,
    init_error: Option<String>,
}

impl SoundCache {
    pub fn new(assets_directory: &str) -> Self {
        Self {
            assets_directory: PathBuf::from(assets_directory),
            sfx_by_number: Vec::new(),
            click: None,
            initialized: false,
            init_state: None,
            init_error: None,
        }
    }

    pub fn reset_loading(&mut self) {
        self.sfx_by_number.clear();
        self.click = None;
        self.initialized = false;
        self.init_state = None;
        self.init_error = None;
    }

    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    #[allow(dead_code)]
    pub fn get_numbered(&self, nr: u32) -> Option<&Handle<AudioSource>> {
        self.sfx_by_number.get(nr as usize).and_then(|h| h.as_ref())
    }

    pub fn click(&self) -> Option<&Handle<AudioSource>> {
        self.click.as_ref()
    }

    /// Incrementally initializes the cache by walking a directory.
    ///
    /// This currently treats `assets_zip` as a folder path (e.g. `.../assets/SFX`).
    pub fn initialize(&mut self, audio_sources: &mut Assets<AudioSource>) -> CacheInitStatus {
        if self.initialized {
            log::info!("SoundCache already initialized");
            return CacheInitStatus::Done;
        }

        if let Some(err) = self.init_error.clone() {
            log::error!("SoundCache initialization error: {}", err);
            return CacheInitStatus::Error(err);
        }

        if self.init_state.is_none() {
            let mut entries = Vec::new();
            match std::fs::read_dir(&self.assets_directory) {
                Ok(dir) => {
                    for entry in dir.flatten() {
                        let path = entry.path();
                        if path.is_file() {
                            // Convert to absolute path if possible
                            let abs_path = std::fs::canonicalize(&path).unwrap_or(path);
                            entries.push(abs_path);
                        }
                    }
                }
                Err(e) => {
                    let err = format!(
                        "Failed to read sounds directory {:?}: {e}",
                        self.assets_directory
                    );
                    log::error!("{}", err);
                    self.init_error = Some(err.clone());
                    return CacheInitStatus::Error(err);
                }
            }

            log::info!(
                "SoundCache found {} audio files in {:?}",
                entries.len(),
                self.assets_directory
            );
            self.init_state = Some(InitState { entries, index: 0 });
        }

        let state = self.init_state.as_mut().unwrap();
        if state.entries.is_empty() {
            self.initialized = true;
            self.init_state = None;

            log::error!("SoundCache::initialize completed with no entries");
            return CacheInitStatus::Done;
        }

        if state.index >= state.entries.len() {
            self.initialized = true;
            self.init_state = None;

            log::info!("SoundCache::initialize completed successfully");
            return CacheInitStatus::Done;
        }

        let audio_file = &state.entries[state.index];

        let stem = audio_file
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();

        let audio_bytes = match std::fs::read(audio_file) {
            Ok(bytes) => bytes,
            Err(e) => {
                let err = format!("Failed to read sound file {:?}: {e}", audio_file);
                self.init_error = Some(err.clone());

                log::error!("SoundCache::initialize failed to read audio file: {}", err);
                return CacheInitStatus::Error(err);
            }
        };

        // AudioSource is an Asset that stores the encoded bytes; Rodio decodes
        // based on file contents + enabled Bevy audio features (wav/vorbis/etc).
        let source = AudioSource {
            bytes: audio_bytes.into(),
        };
        let handle = audio_sources.add(source);

        if stem == "click" {
            self.click = Some(handle);
        } else if let Ok(nr) = stem.parse::<u32>() {
            let idx = nr as usize;
            if self.sfx_by_number.len() <= idx {
                self.sfx_by_number.resize_with(idx + 1, || None);
            }
            self.sfx_by_number[idx] = Some(handle);
        } else {
            // Unrecognized naming scheme; ignore for now.
        }

        state.index += 1;

        let progress = state.index as f32 / state.entries.len() as f32;
        CacheInitStatus::InProgress { progress }
    }
}
