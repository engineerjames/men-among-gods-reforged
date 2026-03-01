use std::{collections::HashMap, fs::File, io::Read, path::PathBuf};

use sdl2::{
    image::{ImageRWops, LoadTexture},
    pixels::PixelFormatEnum,
    render::{Texture, TextureCreator},
    rwops::RWops,
    video::WindowContext,
};
use zip::ZipArchive;

/// Pre-decoded RGBA pixel data for a single sprite image.
///
/// Used for CPU-side operations (e.g. average-color calculation) that do not
/// require a GPU texture.
pub struct CachedRgbaImage {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<u8>,
}

/// Lazy-loading sprite and texture cache backed by a ZIP archive.
///
/// Textures are loaded from `images.zip` on first access and kept in memory
/// for the lifetime of the cache. Average per-sprite colours and raw RGBA
/// pixel data are also cached for minimap and hit-test use.
pub struct GraphicsCache {
    sprite_cache: HashMap<usize, Texture>,
    avg_color_cache: HashMap<usize, (u8, u8, u8)>,
    rgba_image_cache: HashMap<usize, CachedRgbaImage>,
    creator: TextureCreator<WindowContext>,
    archive: ZipArchive<File>,
    index_to_filename: HashMap<usize, String>,
    /// Streaming texture used for minimap rendering (128x128 RGBA).
    pub minimap_texture: Option<Texture>,
}

impl GraphicsCache {
    /// Opens `images.zip` at the given path and builds a sprite-ID-to-filename
    /// index for lazy texture loading.
    ///
    /// # Arguments
    /// * `path_to_zip` - Filesystem path to the `images.zip` archive.
    /// * `creator` - SDL2 texture creator bound to the window.
    ///
    /// # Returns
    /// * A new `GraphicsCache`. Panics if the archive cannot be opened.
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
            minimap_texture: None,
        }
    }

    /// Returns the alpha-weighted average colour of a sprite.
    ///
    /// If the colour has not been calculated yet, the sprite is loaded from
    /// the ZIP archive as a side-effect.
    ///
    /// # Arguments
    /// * `id` - Numeric sprite ID.
    ///
    /// # Returns
    /// * `(r, g, b)` tuple. Returns `(0, 0, 0)` for fully-transparent or
    ///   missing sprites.
    pub fn get_avg_color(&mut self, id: usize) -> (u8, u8, u8) {
        if let Some(color) = self.avg_color_cache.get(&id) {
            return *color;
        }

        // If the average color isn't cached, load the texture to calculate it (this will cache it for next time)
        self.get_texture(id);
        *self.avg_color_cache.get(&id).unwrap_or_else(|| {
            log::warn!(
                "Average color not found for sprite ID {}. Returning (0, 0, 0).",
                id
            );
            &(0, 0, 0)
        })
    }

    /// Ensure the minimap streaming texture exists (128×128, ABGR8888).
    /// ABGR8888 stores bytes in memory as [R,G,B,A] on little-endian, which
    /// matches the xmap buffer layout directly.
    pub fn ensure_minimap_texture(&mut self) {
        if self.minimap_texture.is_none() {
            match self
                .creator
                .create_texture_streaming(Some(PixelFormatEnum::ABGR8888), 128, 128)
            {
                Ok(tex) => {
                    self.minimap_texture = Some(tex);
                }
                Err(e) => {
                    log::error!("Failed to create minimap texture: {}", e);
                }
            }
        }
    }

    /// Returns a mutable reference to the GPU texture for the given sprite ID.
    ///
    /// The texture is loaded from `images.zip` on first access and cached.
    /// If the sprite cannot be loaded, a fallback error texture (ID 128) is
    /// used instead.
    ///
    /// # Arguments
    /// * `id` - Numeric sprite ID.
    ///
    /// # Returns
    /// * `&mut Texture` — the caller may set blend/colour/alpha modulation
    ///   but must reset it before yielding control.
    pub fn get_texture(&mut self, id: usize) -> &mut Texture {
        const ERROR_SPRITE_ID: usize = 128;
        if !self.sprite_cache.contains_key(&id) {
            let texture = self.load_texture_from_zip(id);
            let final_texture = if let Some(tex) = texture {
                tex
            } else {
                log::warn!(
                    "Failed to load texture for sprite ID {}. Using error texture.",
                    id
                );
                self.load_texture_from_zip(ERROR_SPRITE_ID)
                    .unwrap_or_else(|| {
                        panic!(
                            "Failed to load error texture with ID {}. gfx.zip may be corrupted.",
                            ERROR_SPRITE_ID
                        );
                    })
            };
            self.sprite_cache.insert(id, final_texture);
        }

        self.sprite_cache.get_mut(&id).unwrap()
    }

    /// Loads and decodes a single sprite from the ZIP archive, caching its
    /// average colour and RGBA pixels as a side-effect.
    ///
    /// # Arguments
    /// * `id` - Numeric sprite ID.
    ///
    /// # Returns
    /// * `Some(Texture)` on success, `None` if the sprite is not in the archive
    ///   or decoding fails.
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

    /// Computes the alpha-weighted average RGB colour of raw PNG/image bytes.
    ///
    /// # Arguments
    /// * `image_bytes` - Raw image file bytes (e.g. PNG).
    ///
    /// # Returns
    /// * `(r, g, b)` average colour. Returns `(0, 0, 0)` on decode failure
    ///   or if all pixels are fully transparent.
    fn calculate_avg_color(image_bytes: &[u8]) -> (u8, u8, u8) {
        let rgba_image = match Self::decode_rgba_image(image_bytes) {
            Some(image) => image,
            None => {
                log::warn!(
                    "Failed to decode image for average color calculation. Returning (0, 0, 0)."
                );
                return (0, 0, 0);
            }
        };

        if rgba_image.width == 0 || rgba_image.height == 0 {
            log::warn!(
                "Image has zero width or height for average color calculation. Returning (0, 0, 0)."
            );
            return (0, 0, 0);
        }

        let pixels = &rgba_image.pixels;

        let mut total_r: u64 = 0;
        let mut total_g: u64 = 0;
        let mut total_b: u64 = 0;

        let mut pixels_counted: u64 = 0;
        for pixel in pixels.chunks_exact(4) {
            if pixel[3] == 0 {
                continue; // Skip fully transparent pixels
            }

            let r = pixel[0] as u64;
            let g = pixel[1] as u64;
            let b = pixel[2] as u64;

            total_r += r;
            total_g += g;
            total_b += b;
            pixels_counted += 1;
        }

        if pixels_counted == 0 {
            log::warn!(
                "All pixels are fully transparent for average color calculation. Returning (0, 0, 0)."
            );
            return (0, 0, 0); // Avoid division by zero if all pixels are transparent
        }

        (
            (total_r / pixels_counted) as u8,
            (total_g / pixels_counted) as u8,
            (total_b / pixels_counted) as u8,
        )
    }

    /// Decodes raw image bytes into a contiguous RGBA pixel buffer.
    ///
    /// # Arguments
    /// * `image_bytes` - Raw image file bytes (e.g. PNG).
    ///
    /// # Returns
    /// * `Some(CachedRgbaImage)` on success, `None` on decode failure.
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
}
