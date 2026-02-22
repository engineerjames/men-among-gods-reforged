use std::path::PathBuf;

use egui_sdl2::egui::ahash::{HashMap, HashMapExt};
use sdl2::mixer::{Channel, Chunk};

const LOGIN_MUSIC_CHANNEL: i32 = 0;

/// Manages pre-loaded sound effects and background music tracks.
///
/// Sound effects are identified by numeric sprite IDs; music tracks by the
/// [`MusicTrack`] enum. All audio data is loaded eagerly at construction
/// and played through SDL2_mixer channels.
pub struct SoundCache {
    sfx_cache: HashMap<usize, Chunk>,
    music_cache: HashMap<MusicTrack, Chunk>,
    click_sfx: Option<Chunk>,
}

/// Named background-music tracks that can be played or stopped.
#[derive(Hash, Eq, PartialEq, Clone, Copy)]
pub enum MusicTrack {
    LoginTheme,
}

impl SoundCache {
    fn convert_server_volume(vol: i32, master_volume: f32) -> i32 {
        let master = master_volume.clamp(0.0, 1.0);

        let base = if vol <= 0 {
            // Server commonly sends attenuation values in the range [-5000, 0].
            // 0 means full volume; -5000 is effectively silent.
            let attenuated = (5000 + vol.clamp(-5000, 0)) as f32 / 5000.0;
            (attenuated * 127.0).round() as i32
        } else {
            // Preserve compatibility with callers that already pass SDL-like 0..127 values.
            vol.clamp(0, 127)
        };

        (base as f32 * master).round() as i32
    }

    fn convert_server_pan(pan: i32) -> u8 {
        if (-500..=500).contains(&pan) {
            // Server pan convention: -500 = hard left, 0 = center, 500 = hard right.
            (((pan + 500) as f32 / 1000.0) * 255.0)
                .round()
                .clamp(0.0, 255.0) as u8
        } else {
            // Preserve compatibility with callers already using SDL's 0..255 convention.
            pan.clamp(0, 255) as u8
        }
    }

    /// Loads all `.wav` files from `sfx_directory` and music files from
    /// `music_directory` into memory.
    ///
    /// # Arguments
    /// * `sfx_directory` - Path to the directory containing numbered `.wav` files.
    /// * `music_directory` - Path to the directory containing music tracks.
    ///
    /// # Returns
    /// * A new `SoundCache`. Panics if the sfx directory cannot be read.
    pub fn new(sfx_directory: PathBuf, music_directory: PathBuf) -> Self {
        let mut sfx_cache: HashMap<usize, Chunk> = HashMap::new();
        let mut click_sfx: Option<Chunk> = None;

        for file in std::fs::read_dir(&sfx_directory).unwrap_or_else(|e| {
            log::error!("Failed to read sound directory: {}", e);
            panic!("Failed to read sound directory: {}", e);
        }) {
            if let Ok(entry) = file {
                let path = entry.path();
                if path.is_file() {
                    let file_name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
                    if file_name.eq_ignore_ascii_case("click.wav") {
                        match Chunk::from_file(&path) {
                            Ok(chunk) => {
                                click_sfx = Some(chunk);
                            }
                            Err(e) => {
                                log::warn!(
                                    "Failed to load click sfx from {}: {}",
                                    path.display(),
                                    e
                                );
                            }
                        }
                    }

                    // Our SFX IDs are numeric filenames (e.g. 00031.wav). Some zip builds
                    // include a directory prefix (e.g. sounds/00031.wav), so parse only the
                    // final path component.
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
            click_sfx,
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
                let scaled = Self::convert_server_volume(vol, master_volume);
                let sdl_vol = scaled * 128 / 127;
                ch.set_volume(sdl_vol.clamp(0, 128));
                // Panning: left + right must sum to ~255.
                let right = Self::convert_server_pan(pan);
                let left = 255 - right;
                let _ = ch.set_panning(left, right);
            }
            Err(e) => {
                log::warn!("Failed to play sfx {}: {}", nr, e);
            }
        }
    }

    /// Plays the classic UI click sound (`click.wav`) if present in the asset pack.
    pub fn play_click(&self, master_volume: f32) {
        let Some(chunk) = self.click_sfx.as_ref() else {
            return;
        };

        match Channel::all().play(chunk, 0) {
            Ok(ch) => {
                let scaled = Self::convert_server_volume(-1000, master_volume);
                let sdl_vol = scaled * 128 / 127;
                ch.set_volume(sdl_vol.clamp(0, 128));
                let right = Self::convert_server_pan(0);
                let left = 255 - right;
                let _ = ch.set_panning(left, right);
            }
            Err(e) => {
                log::warn!("Failed to play click sfx: {}", e);
            }
        }
    }

    /// Plays a music track on a dedicated channel, looping indefinitely.
    ///
    /// # Arguments
    /// * `track` - The [`MusicTrack`] to play.
    pub fn play_music(&self, track: MusicTrack) {
        if let Some(chunk) = self.music_cache.get(&track) {
            if let Err(e) = Channel(LOGIN_MUSIC_CHANNEL).play(chunk, -1) {
                log::warn!("Failed to play music: {}", e);
            }
        }
    }

    /// Stops any currently playing music on the dedicated music channel.
    pub fn stop_music(&self) {
        Channel(LOGIN_MUSIC_CHANNEL).halt();
    }
}
