use std::path::PathBuf;

use egui_sdl2::egui::ahash::{HashMap, HashMapExt};
use sdl2::mixer::{Channel, Chunk};

const LOGIN_MUSIC_CHANNEL: i32 = 0;

pub struct SoundCache {
    sfx_cache: HashMap<usize, Chunk>,
    music_cache: HashMap<MusicTrack, Chunk>,
}

#[derive(Hash, Eq, PartialEq, Clone, Copy)]
pub enum MusicTrack {
    LoginTheme,
}

impl SoundCache {
    pub fn new(sfx_directory: PathBuf, music_directory: PathBuf) -> Self {
        let mut sfx_cache: HashMap<usize, Chunk> = HashMap::new();

        for file in std::fs::read_dir(&sfx_directory).unwrap_or_else(|e| {
            log::error!("Failed to read sound directory: {}", e);
            panic!("Failed to read sound directory: {}", e);
        }) {
            if let Ok(entry) = file {
                let path = entry.path();
                if path.is_file() {
                    // Our SFX IDs are numeric filenames (e.g. 00031.wav). Some zip builds
                    // include a directory prefix (e.g. sounds/00031.wav), so parse only the
                    // final path component.
                    let file_name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
                    let stem = file_name.split('.').next().unwrap_or("");
                    if let Ok(id) = stem.parse::<usize>() {
                        match Chunk::from_file(&path) {
                            Ok(chunk) => {
                                sfx_cache.insert(id, chunk);
                            }
                            Err(e) => {
                                log::warn!(
                                    "Failed to load sfx {} from {}: {}",
                                    id,
                                    path.display(),
                                    e
                                );
                            }
                        }
                    }
                }
            }
        }

        let mut music_cache: HashMap<MusicTrack, Chunk> = HashMap::new();

        let music_path = music_directory.join("login.mp3");
        match Chunk::from_file(&music_path) {
            Ok(chunk) => {
                music_cache.insert(MusicTrack::LoginTheme, chunk);
            }
            Err(e) => {
                log::warn!(
                    "Failed to load login music from {}: {}",
                    music_path.display(),
                    e
                );
            }
        }

        SoundCache {
            sfx_cache,
            music_cache,
        }
    }

    /// Play a sound effect by numeric ID. `vol` is 0-127, `pan` is 0 (left) – 255 (right)
    /// with 128 as centre. `master_volume` is a 0.0–1.0 multiplier applied on top.
    /// Mismatched or missing IDs are silently ignored.
    pub fn play_sfx(&self, nr: usize, vol: i32, pan: i32, master_volume: f32) {
        let Some(chunk) = self.sfx_cache.get(&nr) else {
            return;
        };
        // Find a free channel and play
        match Channel::all().play(chunk, 0) {
            Ok(ch) => {
                // SDL_mixer volume is 0-128.
                let scaled = (vol.clamp(0, 127) as f32 * master_volume.clamp(0.0, 1.0)) as i32;
                let sdl_vol = scaled * 128 / 127;
                ch.set_volume(sdl_vol);
                // Panning: left + right must sum to ~255.
                let pan = pan.clamp(0, 255) as u8;
                let left = 255 - pan;
                let right = pan;
                let _ = ch.set_panning(left, right);
            }
            Err(e) => {
                log::warn!("Failed to play sfx {}: {}", nr, e);
            }
        }
    }

    #[allow(dead_code)]
    pub fn play_music(&self, track: MusicTrack) {
        if let Some(chunk) = self.music_cache.get(&track) {
            if let Err(e) = Channel(LOGIN_MUSIC_CHANNEL).play(chunk, -1) {
                log::warn!("Failed to play music: {}", e);
            }
        }
    }

    pub fn stop_music(&self) {
        Channel(LOGIN_MUSIC_CHANNEL).halt();
    }
}
