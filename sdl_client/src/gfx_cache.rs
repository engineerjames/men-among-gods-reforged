use std::{collections::HashMap, fs::File, io::Read, path::PathBuf};

use sdl2::{
    image::{ImageRWops, LoadTexture},
    pixels::PixelFormatEnum,
    render::{Texture, TextureCreator},
    rwops::RWops,
    video::WindowContext,
};
use zip::ZipArchive;

pub struct CachedRgbaImage {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<u8>,
}

pub struct GraphicsCache {
    sprite_cache: HashMap<usize, Texture>,
    avg_color_cache: HashMap<usize, (u8, u8, u8)>,
    rgba_image_cache: HashMap<usize, CachedRgbaImage>,
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
            rgba_image_cache: HashMap::new(),
            creator,
            archive,
            index_to_filename,
        }
    }

    // TODO: The minimap will use this but we haven't implemented that yet
    #[allow(dead_code)]
    pub fn get_avg_color(&mut self, id: usize) -> (u8, u8, u8) {
        if let Some(color) = self.avg_color_cache.get(&id) {
            return *color;
        }

        // If the average color isn't cached, load the texture to calculate it (this will cache it for next time)
        self.get_texture(id);
        *self.avg_color_cache.get(&id).unwrap_or(&(0, 0, 0))
    }

    pub fn get_texture(&mut self, id: usize) -> &Texture {
        if self.sprite_cache.contains_key(&id) {
            return &self.sprite_cache[&id];
        }

        let texture = self.load_texture_from_zip(id);
        self.sprite_cache.insert(id, texture.unwrap());

        &self.sprite_cache[&id]
    }

    pub fn get_rgba_image(&mut self, id: usize) -> Option<&CachedRgbaImage> {
        if self.rgba_image_cache.contains_key(&id) {
            return self.rgba_image_cache.get(&id);
        }

        let filename = self.index_to_filename.get(&id)?.to_string();
        let mut file = self.archive.by_name(&filename).ok()?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer).ok()?;

        let rgba_image = Self::decode_rgba_image(&buffer)?;
        self.rgba_image_cache.insert(id, rgba_image);
        self.rgba_image_cache.get(&id)
    }

    fn load_texture_from_zip(&mut self, id: usize) -> Option<Texture> {
        if let Some(filename) = self.index_to_filename.get(&id) {
            if let Ok(mut file) = self.archive.by_name(filename) {
                let mut buffer = Vec::new();
                file.read_to_end(&mut buffer).ok()?;
                if let Ok(texture) = self.creator.load_texture_bytes(&buffer) {
                    self.avg_color_cache
                        .insert(id, Self::calculate_avg_color(&buffer));
                    if let Some(rgba_image) = Self::decode_rgba_image(&buffer) {
                        self.rgba_image_cache.insert(id, rgba_image);
                    }
                    return Some(texture);
                }
            }
        }

        None
    }

    fn calculate_avg_color(image_bytes: &[u8]) -> (u8, u8, u8) {
        let rgba_image = match Self::decode_rgba_image(image_bytes) {
            Some(image) => image,
            None => return (0, 0, 0),
        };

        if rgba_image.width == 0 || rgba_image.height == 0 {
            return (0, 0, 0);
        }

        let pixels = &rgba_image.pixels;

        let mut total_r: u64 = 0;
        let mut total_g: u64 = 0;
        let mut total_b: u64 = 0;
        let mut alpha_sum: u64 = 0;

        for pixel in pixels.chunks_exact(4) {
            let r = pixel[0] as u64;
            let g = pixel[1] as u64;
            let b = pixel[2] as u64;
            let a = pixel[3] as u64;

            total_r += r * a;
            total_g += g * a;
            total_b += b * a;
            alpha_sum += a;
        }

        if alpha_sum == 0 {
            return (0, 0, 0);
        }

        (
            (total_r / alpha_sum) as u8,
            (total_g / alpha_sum) as u8,
            (total_b / alpha_sum) as u8,
        )
    }

    fn decode_rgba_image(image_bytes: &[u8]) -> Option<CachedRgbaImage> {
        let rwops = match RWops::from_bytes(image_bytes) {
            Ok(rwops) => rwops,
            Err(error) => {
                log::warn!("Failed to create RWops for image decode: {}", error);
                return None;
            }
        };

        let surface = match rwops.load() {
            Ok(surface) => surface,
            Err(error) => {
                log::warn!("Failed to decode image: {}", error);
                return None;
            }
        };

        let surface = match surface.convert_format(PixelFormatEnum::RGBA32) {
            Ok(surface) => surface,
            Err(error) => {
                log::warn!("Failed to convert image format to RGBA32: {}", error);
                return None;
            }
        };

        let width = surface.width() as usize;
        let height = surface.height() as usize;
        if width == 0 || height == 0 {
            return None;
        }

        let pixels = match surface.without_lock() {
            Some(pixels) => pixels,
            None => {
                log::warn!("Failed to access pixel buffer for image decode");
                return None;
            }
        };

        let pitch = surface.pitch() as usize;
        let row_size = width * 4;
        let mut contiguous = Vec::with_capacity(height * row_size);

        for y in 0..height {
            let row_start = y * pitch;
            let row_end = row_start + row_size;
            contiguous.extend_from_slice(&pixels[row_start..row_end]);
        }

        Some(CachedRgbaImage {
            width,
            height,
            pixels: contiguous,
        })
    }

    pub fn get_bytes(&mut self, id: usize) -> Option<Vec<u8>> {
        let filename = self.index_to_filename.get(&id)?.to_string();
        let mut file = self.archive.by_name(&filename).ok()?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer).ok()?;
        Some(buffer)
    }
}
