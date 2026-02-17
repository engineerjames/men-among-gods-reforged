use std::{collections::HashMap, fs::File, io::Read, path::PathBuf};

use image::GenericImageView;
use sdl2::{
    image::LoadTexture,
    render::{Texture, TextureCreator},
    video::WindowContext,
};
use zip::ZipArchive;

pub struct GraphicsCache {
    sprite_cache: HashMap<usize, Texture>,
    avg_color_cache: HashMap<usize, (u8, u8, u8)>,
    creator: TextureCreator<WindowContext>,
    archive: ZipArchive<File>,
    index_to_filename: HashMap<usize, String>,
}

impl GraphicsCache {
    pub fn new(path_to_zip: PathBuf, creator: TextureCreator<WindowContext>) -> Self {
        let file = match File::open(path_to_zip) {
            Ok(f) => f,
            Err(e) => {
                log::error!("Failed to open gfx.zip: {}", e);
                panic!("Failed to open gfx.zip: {}", e);
            }
        };

        let mut archive = match ZipArchive::new(file) {
            Ok(archive) => archive,
            Err(e) => {
                log::error!("Failed to read gfx.zip: {}", e);
                panic!("Failed to read gfx.zip: {}", e);
            }
        };

        log::info!("Building index of gfx.zip contents...");
        let mut index_to_filename = HashMap::new();
        for i in 0..archive.len() {
            if let Ok(file) = archive.by_index(i) {
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
                        index_to_filename.insert(id, name);
                    }
                }
            }
        }

        log::info!("Successfully loaded gfx.zip with {} files", archive.len());

        GraphicsCache {
            sprite_cache: HashMap::new(),
            avg_color_cache: HashMap::new(),
            creator,
            archive,
            index_to_filename,
        }
    }

    pub fn get_texture(&mut self, id: usize) -> &Texture {
        if self.sprite_cache.contains_key(&id) {
            return &self.sprite_cache[&id];
        }

        let texture = self.load_texture_from_zip(id);
        self.sprite_cache.insert(id, texture.unwrap());

        &self.sprite_cache[&id]
    }

    fn load_texture_from_zip(&mut self, id: usize) -> Option<Texture> {
        if let Some(filename) = self.index_to_filename.get(&id) {
            if let Ok(mut file) = self.archive.by_name(filename) {
                let mut buffer = Vec::new();
                file.read_to_end(&mut buffer).ok()?;
                if let Ok(texture) = self.creator.load_texture_bytes(&buffer) {
                    self.avg_color_cache
                        .insert(id, Self::calculate_avg_color(&buffer));
                    return Some(texture);
                }
            }
        }

        None
    }

    fn calculate_avg_color(image_bytes: &[u8]) -> (u8, u8, u8) {
        let decoded = match image::load_from_memory(image_bytes) {
            Ok(image) => image.to_rgba8(),
            Err(error) => {
                log::warn!(
                    "Failed to decode image for average color calculation: {}",
                    error
                );
                return (0, 0, 0);
            }
        };

        let (width, height) = decoded.dimensions();
        let pixel_count = (width as u64) * (height as u64);
        if pixel_count == 0 {
            return (0, 0, 0);
        }

        let mut total_r: u64 = 0;
        let mut total_g: u64 = 0;
        let mut total_b: u64 = 0;

        for pixel in decoded.pixels() {
            let [r, g, b, a] = pixel.0;
            total_r += (r as u64) * (a as u64);
            total_g += (g as u64) * (a as u64);
            total_b += (b as u64) * (a as u64);
        }

        let alpha_sum: u64 = decoded.pixels().map(|pixel| pixel.0[3] as u64).sum();
        if alpha_sum == 0 {
            return (0, 0, 0);
        }

        (
            (total_r / alpha_sum) as u8,
            (total_g / alpha_sum) as u8,
            (total_b / alpha_sum) as u8,
        )
    }
}
