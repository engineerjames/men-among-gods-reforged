use std::{collections::HashMap, fs::File, io::Read, path::PathBuf};

use sdl2::{
    image::{ImageRWops, LoadTexture},
    pixels::PixelFormatEnum,
    render::{BlendMode, Texture, TextureCreator, TextureQuery},
    rwops::RWops,
    video::WindowContext,
};
use zip::ZipArchive;

use crate::preferences::SpriteUpscaler;
use crate::upscale;

/// First synthetic sprite ID handed out by [`GraphicsCache::load_texture_from_path`].
///
/// IDs at or above this value refer to textures loaded from the filesystem
/// rather than from `images.zip`. They are never upscaled (their source art is
/// not 32x32 pixel art) and never evicted, because the caller holds the ID
/// indefinitely and there is no way to reload it from the archive.
const CUSTOM_ID_BASE: usize = 100_000;

/// Default VRAM budget for cached sprite textures, in bytes.
///
/// The cache used to be unbounded, which was tolerable at 1x. At 3x with an
/// upscaler active each sprite costs nine times as much, so a bound is needed
/// to keep long sessions on low-memory hardware from growing without limit.
const DEFAULT_CACHE_BUDGET_BYTES: usize = 192 * 1024 * 1024;

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
/// subject to a VRAM budget. Average per-sprite colors and raw RGBA pixel data
/// are also cached for minimap and hit-test use; those are always stored at
/// native 1x resolution regardless of the active sprite scale.
///
/// The lifetime `'tc` ties all GPU textures to the [`TextureCreator`] that
/// produced them, ensuring they cannot outlive the renderer.
pub struct GraphicsCache<'tc> {
    sprite_cache: HashMap<usize, Texture<'tc>>,
    avg_color_cache: HashMap<usize, (u8, u8, u8)>,
    rgba_image_cache: HashMap<usize, CachedRgbaImage>,
    creator: &'tc TextureCreator<WindowContext>,
    archive: ZipArchive<File>,
    index_to_filename: HashMap<usize, String>,
    /// Streaming texture used for minimap rendering (128x128 RGBA).
    pub minimap_texture: Option<Texture<'tc>>,
    /// Next synthetic sprite ID for textures loaded from the filesystem.
    next_custom_id: usize,
    /// Integer factor archive sprites are upscaled by when decoded.
    sprite_scale: u32,
    /// Algorithm used to perform that upscale.
    upscaler: SpriteUpscaler,
    /// Monotonic counter used to order cache entries by recency of use.
    access_tick: u64,
    /// Last access tick per cached sprite ID.
    last_used: HashMap<usize, u64>,
    /// Approximate GPU bytes currently held by evictable sprite textures.
    cached_bytes: usize,
    /// Upper bound on `cached_bytes` before eviction kicks in.
    budget_bytes: usize,
}

impl<'tc> GraphicsCache<'tc> {
    /// Opens `images.zip` at the given path and builds a sprite-ID-to-filename
    /// index for lazy texture loading.
    ///
    /// # Arguments
    /// * `path_to_zip` - Filesystem path to the `images.zip` archive.
    /// * `creator` - SDL2 texture creator bound to the window.
    ///
    /// # Returns
    /// * A new `GraphicsCache`. Panics if the archive cannot be opened.
    pub fn new(path_to_zip: PathBuf, creator: &'tc TextureCreator<WindowContext>) -> Self {
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
                let name = file.name().to_owned();
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
            next_custom_id: CUSTOM_ID_BASE,
            sprite_scale: 1,
            upscaler: SpriteUpscaler::None,
            access_tick: 0,
            last_used: HashMap::new(),
            cached_bytes: 0,
            budget_bytes: DEFAULT_CACHE_BUDGET_BYTES,
        }
    }

    /// Sets the upscale factor and algorithm applied to archive sprites.
    ///
    /// The factor must match the active internal render scale so that upscaled
    /// sprites still blit 1:1 into the frame buffer. Changing either value
    /// invalidates every cached GPU texture, but the decoded RGBA pixels and
    /// average colours are retained so nothing has to be re-read from the ZIP.
    ///
    /// # Arguments
    ///
    /// * `scale` - Integer upscale factor; clamped to at least 1. Forced to 1
    ///   when `upscaler` is [`SpriteUpscaler::None`], since without an
    ///   algorithm a CPU upscale would only duplicate what the renderer already
    ///   does for free.
    /// * `upscaler` - Algorithm to use. Ignored when `scale` is 1.
    pub fn set_sprite_scaling(&mut self, scale: u32, upscaler: SpriteUpscaler) {
        let scale = if upscaler == SpriteUpscaler::None {
            1
        } else {
            scale.max(1)
        };
        if self.sprite_scale == scale && self.upscaler == upscaler {
            return;
        }
        log::info!("Sprite scaling changed to {scale}x using {upscaler}; flushing texture cache");
        self.sprite_scale = scale;
        self.upscaler = upscaler;
        self.sprite_cache.clear();
        self.last_used.clear();
        self.cached_bytes = 0;
    }

    /// Sets the VRAM budget for evictable sprite textures.
    ///
    /// # Arguments
    ///
    /// * `bytes` - Budget in bytes; clamped to a floor of 16 MiB so the cache
    ///   can never thrash on the working set of a single frame.
    pub fn set_cache_budget_bytes(&mut self, bytes: usize) {
        self.budget_bytes = bytes.max(16 * 1024 * 1024);
        self.evict_until_within_budget(None);
    }

    /// Returns the upscale factor applied to a given sprite.
    ///
    /// Callers that pass an explicit source rectangle to `copy` must multiply
    /// that rectangle by this factor, since source rects are in texture space
    /// while destination rects are in logical space.
    ///
    /// # Arguments
    ///
    /// * `id` - Numeric sprite ID.
    ///
    /// # Returns
    ///
    /// * The sprite scale for archive sprites, or 1 for filesystem textures.
    pub fn sprite_scale_of(&self, id: usize) -> u32 {
        if id >= CUSTOM_ID_BASE {
            1
        } else {
            self.sprite_scale.max(1)
        }
    }

    /// Scales a source rectangle from logical pixels into texture space.
    ///
    /// Source rectangles index the texture directly, so unlike destination
    /// rectangles they must follow the sprite's upscale factor. Call sites that
    /// crop a sprite (bitmap font glyphs, rank sigils) must route their source
    /// rect through this helper.
    ///
    /// # Arguments
    ///
    /// * `id` - Numeric sprite ID.
    /// * `src` - Source rectangle expressed in native 1x pixels.
    ///
    /// # Returns
    ///
    /// * The rectangle scaled into the cached texture's coordinate space.
    pub fn scale_src_rect(&self, id: usize, src: sdl2::rect::Rect) -> sdl2::rect::Rect {
        let scale = self.sprite_scale_of(id);
        if scale == 1 {
            return src;
        }
        let s = scale as i32;
        sdl2::rect::Rect::new(
            src.x() * s,
            src.y() * s,
            src.width() * scale,
            src.height() * scale,
        )
    }

    /// Returns the upscale factor applied to a given sprite.
    ///
    /// # Arguments
    ///
    /// * `id` - Numeric sprite ID.
    ///
    /// # Returns
    ///
    /// * The sprite scale for archive sprites, or 1 for filesystem textures.
    fn scale_for(&self, id: usize) -> u32 {
        self.sprite_scale_of(id)
    }

    /// Returns a sprite's size in logical (unscaled) pixels.
    ///
    /// Destination rectangles must always be expressed in logical units: the
    /// renderer scale applied while the frame buffer is bound converts them to
    /// the internal resolution, at which point an upscaled texture lands 1:1.
    /// Using the raw texture size instead would draw sprites `sprite_scale`
    /// times too large.
    ///
    /// # Arguments
    ///
    /// * `id` - Numeric sprite ID. Loaded on demand if not yet cached.
    ///
    /// # Returns
    ///
    /// * `(width, height)` in logical pixels.
    pub fn logical_texture_size(&mut self, id: usize) -> (u32, u32) {
        let scale = self.scale_for(id);
        let TextureQuery { width, height, .. } = self.get_texture(id).query();
        (width / scale.max(1), height / scale.max(1))
    }

    /// Converts a raw texture size to logical pixels for a given sprite.
    ///
    /// # Arguments
    ///
    /// * `id` - Numeric sprite ID.
    /// * `size` - Raw texture size in physical pixels.
    ///
    /// # Returns
    ///
    /// * `(width, height)` in logical pixels.
    pub fn to_logical_size(&self, id: usize, size: (u32, u32)) -> (u32, u32) {
        let scale = self.scale_for(id).max(1);
        (size.0 / scale, size.1 / scale)
    }

    /// Evicts least-recently-used sprites until the cache fits its budget.
    ///
    /// Eviction only ever happens at insertion time. `get_texture` hands out
    /// `&mut Texture` references that callers hold across draw calls, so
    /// dropping textures at any other point would be unsound.
    ///
    /// # Arguments
    ///
    /// * `keep` - Sprite ID that must not be evicted (the one just inserted).
    fn evict_until_within_budget(&mut self, keep: Option<usize>) {
        while self.cached_bytes > self.budget_bytes {
            let victim = self
                .sprite_cache
                .keys()
                .copied()
                .filter(|id| *id < CUSTOM_ID_BASE && Some(*id) != keep)
                .min_by_key(|id| self.last_used.get(id).copied().unwrap_or(0));

            let Some(victim) = victim else {
                // Nothing evictable left; the budget is smaller than the set of
                // pinned textures. Stop rather than spin.
                break;
            };

            if let Some(texture) = self.sprite_cache.remove(&victim) {
                self.cached_bytes = self
                    .cached_bytes
                    .saturating_sub(Self::texture_bytes(&texture));
            }
            self.last_used.remove(&victim);
        }
    }

    /// Estimates the GPU memory footprint of a texture.
    ///
    /// # Arguments
    ///
    /// * `texture` - The texture to measure.
    ///
    /// # Returns
    ///
    /// * Size in bytes, assuming 4 bytes per pixel.
    fn texture_bytes(texture: &Texture<'_>) -> usize {
        let TextureQuery { width, height, .. } = texture.query();
        width as usize * height as usize * 4
    }

    /// Returns the alpha-weighted average color of a sprite.
    ///
    /// If the color has not been calculated yet, the sprite is loaded from
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
                Ok(mut tex) => {
                    tex.set_blend_mode(sdl2::render::BlendMode::Blend);
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
    /// * `&mut Texture<'tc>` — the caller may set blend/color/alpha modulation
    ///   but must reset it before yielding control.
    pub fn get_texture(&mut self, id: usize) -> &mut Texture<'tc> {
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
            self.cached_bytes += Self::texture_bytes(&final_texture);
            self.sprite_cache.insert(id, final_texture);
            self.evict_until_within_budget(Some(id));
        }

        self.access_tick += 1;
        self.last_used.insert(id, self.access_tick);

        self.sprite_cache.get_mut(&id).unwrap()
    }

    /// Loads a texture from a filesystem path (not from the ZIP archive).
    ///
    /// The texture is assigned a synthetic sprite ID (starting at 100 000)
    /// and cached like any other sprite.  Subsequent calls with the same ID
    /// use the cached texture.
    ///
    /// # Arguments
    ///
    /// * `path` - Filesystem path to a PNG (or other SDL2_image-supported) file.
    ///
    /// # Returns
    ///
    /// The assigned sprite ID on success, or an error message.
    pub fn load_texture_from_path(&mut self, path: &std::path::Path) -> Result<usize, String> {
        let mut file =
            File::open(path).map_err(|e| format!("Failed to open {}: {}", path.display(), e))?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)
            .map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;
        let texture = self
            .creator
            .load_texture_bytes(&buffer)
            .map_err(|e| format!("Failed to decode texture from {}: {}", path.display(), e))?;
        let id = self.next_custom_id;
        self.next_custom_id += 1;
        self.sprite_cache.insert(id, texture);
        Ok(id)
    }

    /// Returns the pixel dimensions of a cached texture.
    ///
    /// The result is in **logical** pixels, i.e. already divided by the active
    /// sprite upscale factor, so callers can use it directly as a destination
    /// size.
    ///
    /// # Arguments
    ///
    /// * `id` - Sprite ID (must already be loaded).
    ///
    /// # Returns
    ///
    /// `(width, height)` in logical pixels, or `(0, 0)` if the ID is not cached.
    pub fn query_texture_size(&self, id: usize) -> (u32, u32) {
        if let Some(tex) = self.sprite_cache.get(&id) {
            let TextureQuery { width, height, .. } = tex.query();
            self.to_logical_size(id, (width, height))
        } else {
            (0, 0)
        }
    }

    /// Loads and decodes a single sprite from the ZIP archive, caching its
    /// average color and RGBA pixels as a side-effect.
    ///
    /// # Arguments
    /// * `id` - Numeric sprite ID.
    ///
    /// # Returns
    /// * `Some(Texture)` on success, `None` if the sprite is not in the archive
    ///   or decoding fails.
    fn load_texture_from_zip(&mut self, id: usize) -> Option<Texture<'tc>> {
        let filename = self.index_to_filename.get(&id)?.clone();

        // Read the archive entry into memory in its own scope so the mutable
        // borrow of `self.archive` ends before the cache is mutated below.
        let buffer = {
            let mut file = self.archive.by_name(&filename).ok()?;
            let mut buffer = Vec::new();
            file.read_to_end(&mut buffer).ok()?;
            buffer
        };

        self.avg_color_cache
            .insert(id, Self::calculate_avg_color(&buffer));
        let rgba_image = Self::decode_rgba_image(&buffer);

        // Upscaling needs CPU pixels, so it can only run when the decode
        // succeeded. Otherwise fall back to SDL2_image's direct-to-GPU path.
        if self.sprite_scale > 1
            && let Some(image) = rgba_image.as_ref()
            && let Some(texture) = self.build_upscaled_texture(image)
        {
            if let Some(image) = rgba_image {
                self.rgba_image_cache.insert(id, image);
            }
            return Some(texture);
        }

        let texture = self.creator.load_texture_bytes(&buffer).ok();
        if let Some(image) = rgba_image {
            self.rgba_image_cache.insert(id, image);
        }
        texture
    }

    /// Builds a GPU texture from CPU pixels, applying the configured pixel-art
    /// upscaler.
    ///
    /// # Arguments
    ///
    /// * `image` - Native-resolution decoded sprite pixels.
    ///
    /// # Returns
    ///
    /// * `Some(Texture)` on success, `None` if the texture could not be
    ///   allocated or uploaded.
    fn build_upscaled_texture(&self, image: &CachedRgbaImage) -> Option<Texture<'tc>> {
        let scale = self.sprite_scale.max(1) as usize;
        let (w, h) = (image.width, image.height);
        let pixels = match (self.upscaler, scale) {
            (_, 1) => return None,
            (SpriteUpscaler::Scale2x, 2) => upscale::scale2x(&image.pixels, w, h),
            (SpriteUpscaler::Scale2x, 3) => upscale::scale3x(&image.pixels, w, h),
            (SpriteUpscaler::Hqx, 2) => upscale::hq2x(&image.pixels, w, h),
            (SpriteUpscaler::Hqx, 3) => upscale::hq3x(&image.pixels, w, h),
            (_, n) => upscale::nearest(&image.pixels, w, h, n),
        };

        let (dw, dh) = ((w * scale) as u32, (h * scale) as u32);
        let mut texture = self
            .creator
            .create_texture_static(Some(PixelFormatEnum::RGBA32), dw, dh)
            .map_err(|e| log::warn!("Failed to allocate {dw}x{dh} upscaled sprite texture: {e}"))
            .ok()?;
        texture
            .update(None, &pixels, w * scale * 4)
            .map_err(|e| log::warn!("Failed to upload upscaled sprite pixels: {e}"))
            .ok()?;
        // Textures created directly (rather than via SDL2_image) default to no
        // blending, which would render every sprite's transparent border black.
        texture.set_blend_mode(BlendMode::Blend);
        Some(texture)
    }

    /// Computes the alpha-weighted average RGB color of raw PNG/image bytes.
    ///
    /// # Arguments
    /// * `image_bytes` - Raw image file bytes (e.g. PNG).
    ///
    /// # Returns
    /// * `(r, g, b)` average color. Returns `(0, 0, 0)` on decode failure
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

            let r = u64::from(pixel[0]);
            let g = u64::from(pixel[1]);
            let b = u64::from(pixel[2]);

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
