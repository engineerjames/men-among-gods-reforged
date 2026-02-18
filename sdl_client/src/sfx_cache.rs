use std::path::PathBuf;

use egui_sdl2::egui::ahash::{HashMap, HashMapExt};
use sdl2::{audio::AudioSpecWAV, mixer::Chunk};

#[allow(dead_code)]
pub struct SoundCache {
    // Placeholder for sound caching logic. In a real implementation, this would manage loaded sound effects and music tracks.
    sfx_cache: HashMap<usize, AudioSpecWAV>,
    music_cache: HashMap<MusicTrack, Chunk>,
}

#[derive(Hash, Eq, PartialEq, Clone, Copy)]
pub enum MusicTrack {
    LoginTheme,
}

impl SoundCache {
    pub fn new(sfx_directory: PathBuf, music_directory: PathBuf) -> Self {
        let mut sfx_cache: HashMap<usize, AudioSpecWAV> = HashMap::new();

        for file in std::fs::read_dir(sfx_directory).unwrap_or_else(|e| {
            log::error!("Failed to read sound directory: {}", e);
            panic!("Failed to read sound directory: {}", e);
        }) {
            if let Ok(entry) = file {
                let path = entry.path();
                if path.is_file() {
                    log::info!("Found sound file: {}", path.display());

                    // Our SFX IDs are numeric filenames (e.g. 00031.wav). Some zip builds
                    // include a directory prefix (e.g. sounds/00031.wav), so parse only the
                    // final path component.
                    let file_name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
                    let stem = file_name.split('.').next().unwrap_or("");
                    if let Ok(id) = stem.parse::<usize>() {
                        sfx_cache.insert(
                            id,
                            AudioSpecWAV::load_wav(&path).unwrap_or_else(|e| {
                                log::error!("Failed to load sound file {}: {}", path.display(), e);
                                panic!("Failed to load sound file {}: {}", path.display(), e);
                            }),
                        );
                    }
                }
            }
        }

        let mut music_cache: HashMap<MusicTrack, Chunk> = HashMap::new();

        music_cache.insert(
            MusicTrack::LoginTheme,
            Chunk::from_file(music_directory.join("login.mp3")).unwrap_or_else(|e| {
                log::error!("Failed to load music file: {}", e);
                panic!("Failed to load music file: {}", e);
            }),
        );

        SoundCache {
            sfx_cache,
            music_cache,
        }
    }
}
