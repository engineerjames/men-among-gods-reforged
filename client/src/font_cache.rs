//! Unified bitmap + TrueType text rendering.
//!
//! This module is the single home for all text rendering in the client.
//! It exposes two complementary APIs:
//!
//! - Legacy **bitmap** glyphs (sprites 700–703 from the legacy GFX zip),
//!   accessed via the standalone functions [`draw_text`], [`text_width`],
//!   and [`draw_wrapped_text`]. Used by virtually every existing widget.
//! - **TrueType** rasterized text via [`TextEngine`] together with
//!   [`FontHandle`], [`draw_text_handle`], and [`draw_wrapped_text_handle`].
//!   Used by newer UI/menu code that wants higher-quality / variably-sized
//!   text.
//!
//! Both branches share [`TextStyle`] so tinting (`set_color_mod`),
//! alpha-fading, centering, and drop-shadow flags behave identically.
//!
//! TrueType fonts are auto-discovered from
//! [`crate::filepaths::get_fonts_directory`] when the [`TextEngine`] is
//! constructed; callers look them up by filename stem with
//! [`TextEngine::handle`].

use std::collections::HashMap;
use std::path::PathBuf;

use sdl2::pixels::Color;
use sdl2::render::{Canvas, Texture, TextureCreator};
use sdl2::ttf::{Font, Hinting, Sdl2TtfContext};
use sdl2::video::{Window, WindowContext};

/// Sprite ID of the first bitmap font sheet (yellow/default).
pub const BITMAP_FONT_FIRST_SPRITE_ID: usize = 700;

/// Number of bitmap font sprite sheets (fonts 0–3 maps to sprites 700–703).
pub const BITMAP_FONT_COUNT: usize = 4;

/// Width in pixels of each glyph cell in the font sprite sheet.
pub const BITMAP_GLYPH_W: u32 = 6;

/// Height in pixels of the rendered portion of each glyph.
pub const BITMAP_GLYPH_H: u32 = 10;

/// Y-offset within the font sprite sheet where glyphs start.
pub const BITMAP_GLYPH_Y_OFFSET: i32 = 1;

/// Returns the advance width of a single glyph (rendered width is 5px, advance is 6px).
pub const BITMAP_GLYPH_ADVANCE: u32 = BITMAP_GLYPH_W;

/// Styling options for bitmap text rendering.
///
/// Use the associated constants and builder methods to construct styles:
/// - `TextStyle::PLAIN` — no tint, no alpha, left-aligned.
/// - `TextStyle::tinted(color)` — color-modulated text.
/// - `TextStyle::faded(alpha)` — semi-transparent text.
/// - `TextStyle::centered()` — horizontally centered around `x`.
/// - Chain with `.with_tint()` for combined styles.
#[derive(Clone, Copy, Debug)]
pub struct TextStyle {
    /// Optional tint color applied via SDL texture color modulation.
    pub tint: Option<sdl2::pixels::Color>,
    /// Optional alpha for transparency (255 = opaque, 0 = invisible).
    pub alpha: Option<u8>,
    /// If true, `x` is treated as `center_x` and the text is centered horizontally.
    pub centered: bool,
    /// If true, a 1-pixel black drop shadow is drawn at (+1, +1) behind the text.
    pub drop_shadow: bool,
}

impl TextStyle {
    /// Plain text: no tint, no alpha, left-aligned.
    pub const PLAIN: Self = Self {
        tint: None,
        alpha: None,
        centered: false,
        drop_shadow: false,
    };

    /// Creates a style with the given tint color.
    ///
    /// # Arguments
    ///
    /// * `color` - Tint color applied via SDL texture color modulation.
    ///
    /// # Returns
    ///
    /// * A new instance configured by `tinted`.
    pub fn tinted(color: sdl2::pixels::Color) -> Self {
        Self {
            tint: Some(color),
            ..Self::PLAIN
        }
    }

    /// Creates a style with the given alpha value.
    ///
    /// # Arguments
    ///
    /// * `alpha` - Opacity: 255 = fully opaque, 0 = invisible.
    ///
    /// # Returns
    ///
    /// * A new instance configured by `faded`.
    pub fn faded(alpha: u8) -> Self {
        Self {
            alpha: Some(alpha),
            ..Self::PLAIN
        }
    }

    /// Creates a centered style with no tint or alpha.
    ///
    /// # Returns
    ///
    /// * A new instance configured by `centered`.
    pub fn centered() -> Self {
        Self {
            centered: true,
            ..Self::PLAIN
        }
    }

    /// Returns a copy of this style with the given tint color applied.
    ///
    /// # Arguments
    ///
    /// * `color` - Tint color applied via SDL texture color modulation.
    ///
    /// # Returns
    ///
    /// * A new instance configured by `with_tint`.
    pub fn with_tint(mut self, color: sdl2::pixels::Color) -> Self {
        self.tint = Some(color);
        self
    }

    /// Returns a copy of this style with a 1-pixel black drop shadow enabled.
    ///
    /// # Arguments
    ///
    /// * `self` - Value passed to `with_drop_shadow`.
    ///
    /// # Returns
    ///
    /// * A new instance configured by `with_drop_shadow`.
    pub fn with_drop_shadow(mut self) -> Self {
        self.drop_shadow = true;
        self
    }

    /// Creates a plain style with a 1-pixel black drop shadow.
    ///
    /// # Returns
    ///
    /// * A new instance configured by `drop_shadow`.
    pub fn drop_shadow() -> Self {
        Self {
            drop_shadow: true,
            ..Self::PLAIN
        }
    }
}

impl Default for TextStyle {
    fn default() -> Self {
        Self::PLAIN
    }
}

/// Returns the 0-based glyph index for the given ASCII character.
///
/// Returns -1 for characters outside the printable range.
///
/// # Arguments
///
/// * `ch` - Value passed to `glyph_index`.
///
/// # Returns
///
/// * Value returned by `glyph_index`.
#[inline]
pub fn glyph_index(ch: char) -> i32 {
    let code = ch as i32;
    if !(32..=127).contains(&code) {
        return -1;
    }
    code - 32
}

/// Draws a text string onto `canvas` using the bitmap font.
///
/// When `style.centered` is true, `x` is treated as the horizontal center
/// and the text is drawn centered around it. Otherwise `x` is the left edge.
///
/// # Arguments
///
/// * `canvas` - SDL2 canvas to draw onto.
/// * `gfx_cache` - Graphics cache holding font textures.
/// * `font` - Bitmap font index (0–3).
/// * `text` - Text string to render.
/// * `x` - Left edge, or horizontal center when `style.centered` is true.
/// * `y` - Top edge of the glyph row in pixels.
/// * `style` - Rendering style (tint, alpha, centering).
///
/// # Returns
///
/// `Ok(())` on success, or an SDL2 error string.
pub fn draw_text(
    canvas: &mut sdl2::render::Canvas<sdl2::video::Window>,
    gfx_cache: &mut crate::gfx_cache::GraphicsCache<'_>,
    font: usize,
    text: &str,
    x: i32,
    y: i32,
    style: TextStyle,
) -> Result<(), String> {
    let draw_x = if style.centered {
        let width = text.len() as i32 * BITMAP_GLYPH_ADVANCE as i32;
        x - width / 2
    } else {
        x
    };

    if style.drop_shadow {
        draw_text_impl(
            canvas,
            gfx_cache,
            font,
            text,
            draw_x + 1,
            y + 1,
            Some(sdl2::pixels::Color::RGB(0, 0, 0)),
            style.alpha,
        )?;
    }

    draw_text_impl(
        canvas,
        gfx_cache,
        font,
        text,
        draw_x,
        y,
        style.tint,
        style.alpha,
    )
}

#[allow(clippy::too_many_arguments)]
fn draw_text_impl(
    canvas: &mut sdl2::render::Canvas<sdl2::video::Window>,
    gfx_cache: &mut crate::gfx_cache::GraphicsCache<'_>,
    font: usize,
    text: &str,
    x: i32,
    y: i32,
    tint: Option<sdl2::pixels::Color>,
    alpha: Option<u8>,
) -> Result<(), String> {
    let sprite_id = BITMAP_FONT_FIRST_SPRITE_ID + (font % BITMAP_FONT_COUNT);

    if let Some(color) = tint {
        let texture = gfx_cache.get_texture(sprite_id);
        texture.set_color_mod(color.r, color.g, color.b);
    }
    if let Some(a) = alpha {
        let texture = gfx_cache.get_texture(sprite_id);
        texture.set_alpha_mod(a);
    }

    let mut cx = x;
    let mut first_error: Option<String> = None;
    for ch in text.chars() {
        let glyph = glyph_index(ch);
        if glyph < 0 {
            cx += BITMAP_GLYPH_ADVANCE as i32;
            continue;
        }

        // Re-fetch each iteration to avoid holding a reference across the `copy` call.
        let texture = gfx_cache.get_texture(sprite_id);
        let src = sdl2::rect::Rect::new(
            glyph * BITMAP_GLYPH_W as i32,
            BITMAP_GLYPH_Y_OFFSET,
            BITMAP_GLYPH_W - 1,
            BITMAP_GLYPH_H,
        );
        let dst = sdl2::rect::Rect::new(cx, y, BITMAP_GLYPH_W - 1, BITMAP_GLYPH_H);
        if let Err(err) = canvas.copy(texture, Some(src), Some(dst)) {
            first_error = Some(err);
            break;
        }

        cx += BITMAP_GLYPH_ADVANCE as i32;
    }

    if tint.is_some() {
        let texture = gfx_cache.get_texture(sprite_id);
        texture.set_color_mod(255, 255, 255);
    }
    if alpha.is_some() {
        let texture = gfx_cache.get_texture(sprite_id);
        texture.set_alpha_mod(255);
    }

    if let Some(err) = first_error {
        return Err(err);
    }

    Ok(())
}

/// Returns the pixel width of the given text string when rendered with the bitmap font.
///
/// # Arguments
///
/// * `text` - Text used by this function.
///
/// # Returns
///
/// * Value returned by `text_width`.
#[inline]
pub fn text_width(text: &str) -> u32 {
    (text.len() as u32) * BITMAP_GLYPH_ADVANCE
}

/// Draws word-wrapped text within the given pixel width, left-aligned.
///
/// Splits `text` at word boundaries so that each rendered line fits within
/// `max_width` pixels. Lines are separated by `BITMAP_GLYPH_H` pixels
/// vertically. Words wider than `max_width` are hard-broken at the character
/// boundary instead of overflowing.
///
/// # Arguments
///
/// * `canvas` - SDL2 canvas to draw onto.
/// * `gfx_cache` - Graphics cache holding font textures.
/// * `font` - Bitmap font index (0–3).
/// * `text` - Text to render (may contain spaces; newlines are not handled).
/// * `x` - Left edge of the text block in pixels.
/// * `y` - Top edge of the first line in pixels.
/// * `max_width` - Maximum pixel width of a single line.
/// * `style` - Rendering style (tint, alpha, centering is ignored — always left-aligned).
///
/// # Returns
///
/// `Ok(lines_drawn)` on success, or an SDL2 error string.
#[allow(clippy::too_many_arguments)]
pub fn draw_wrapped_text(
    canvas: &mut sdl2::render::Canvas<sdl2::video::Window>,
    gfx_cache: &mut crate::gfx_cache::GraphicsCache<'_>,
    font: usize,
    text: &str,
    x: i32,
    y: i32,
    max_width: u32,
    style: TextStyle,
) -> Result<u32, String> {
    let chars_per_line = (max_width / BITMAP_GLYPH_ADVANCE).max(1) as usize;
    let line_h = BITMAP_GLYPH_H as i32;
    let mut lines_drawn = 0u32;
    let mut cur_y = y;

    // Build lines respecting word boundaries.
    let words: Vec<&str> = text.split(' ').collect();
    let mut current_line = String::new();

    for word in words {
        // If a single word exceeds the available width, hard-break it.
        if word.len() >= chars_per_line {
            // Flush any pending line first.
            if !current_line.is_empty() {
                let flush = std::mem::take(&mut current_line);
                draw_text(canvas, gfx_cache, font, &flush, x, cur_y, style)?;
                cur_y += line_h;
                lines_drawn += 1;
            }
            // Hard-break the long word across multiple lines.
            let mut remaining = word;
            while !remaining.is_empty() {
                let take = remaining.len().min(chars_per_line);
                draw_text(canvas, gfx_cache, font, &remaining[..take], x, cur_y, style)?;
                cur_y += line_h;
                lines_drawn += 1;
                remaining = &remaining[take..];
            }
            continue;
        }

        // Try appending the word to the current line.
        let candidate = if current_line.is_empty() {
            word.to_owned()
        } else {
            format!("{} {}", current_line, word)
        };

        if candidate.len() <= chars_per_line {
            current_line = candidate;
        } else {
            // Flush the current line and start a new one with this word.
            if !current_line.is_empty() {
                draw_text(canvas, gfx_cache, font, &current_line, x, cur_y, style)?;
                cur_y += line_h;
                lines_drawn += 1;
            }
            current_line = word.to_owned();
        }
    }

    // Flush the final line.
    if !current_line.is_empty() {
        draw_text(canvas, gfx_cache, font, &current_line, x, cur_y, style)?;
        lines_drawn += 1;
    }

    Ok(lines_drawn)
}

// ---------------------------------------------------------------------------
// TrueType section
// ---------------------------------------------------------------------------

/// Numeric identifier for a registered TrueType font face.
///
/// TtfIds are assigned by [`TextEngine::new`] as it auto-discovers font
/// files; callers should look fonts up by name via [`TextEngine::font_id`]
/// or build handles directly with [`TextEngine::handle`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TtfId(pub u16);

/// A handle identifying which font and (for TTF) which size to render with.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FontHandle {
    /// Legacy bitmap font sheet. `id` is the 0–3 sprite-sheet index.
    Bitmap {
        /// Bitmap font sheet index (0–3).
        id: u8,
    },
    /// TrueType font registered with the [`TextEngine`].
    Truetype {
        /// Identifier returned by [`TextEngine::font_id`].
        id: TtfId,
        /// Logical point size; DPI-scaled internally during rasterization.
        size_pt: u16,
    },
}

impl FontHandle {
    /// Convenience constructor for a bitmap font handle.
    ///
    /// # Arguments
    ///
    /// * `id` - Bitmap font sheet index (0–3).
    pub const fn bitmap(id: u8) -> Self {
        FontHandle::Bitmap { id }
    }

    /// Convenience constructor for a TrueType font handle.
    ///
    /// # Arguments
    ///
    /// * `id` - Identifier of a discovered TTF font.
    /// * `size_pt` - Logical point size.
    pub const fn ttf(id: TtfId, size_pt: u16) -> Self {
        FontHandle::Truetype { id, size_pt }
    }
}

/// One cached glyph texture together with its rasterized dimensions.
struct GlyphTexture<'tc> {
    texture: Texture<'tc>,
    /// Pixel width of the rasterized glyph surface.
    width_px: u32,
    /// Pixel height of the rasterized glyph surface.
    height_px: u32,
}

/// Composite cache key for a TrueType glyph.
///
/// Glyphs are rasterized white and recolored via texture color modulation,
/// so color is intentionally not part of the key.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct GlyphKey {
    font: TtfId,
    /// Pixel size after DPI scaling, used to select the loaded `Font`.
    size_px: u16,
    ch: char,
}

/// TrueType rendering engine and cache.
///
/// Owns lazily-loaded fonts and rasterized glyph textures. Construct one
/// per application instance during startup and pass a mutable reference
/// into draw paths alongside the bitmap [`crate::gfx_cache::GraphicsCache`].
///
/// All font files under the directory passed to [`TextEngine::new`] are
/// discovered immediately; the `.ttf` / `.otf` faces themselves are opened
/// lazily on first use.
pub struct TextEngine<'ttf, 'tc> {
    ttf_ctx: &'ttf Sdl2TtfContext,
    creator: &'tc TextureCreator<WindowContext>,
    /// Filesystem paths for each `TtfId`, used on lazy load.
    font_paths: HashMap<TtfId, PathBuf>,
    /// `stem -> TtfId` lookup populated during auto-discovery.
    font_ids_by_stem: HashMap<String, TtfId>,
    /// Alphabetically-sorted list of all discovered font stems.
    sorted_stems: Vec<String>,
    /// Loaded font faces keyed by `(TtfId, size_px)`.
    loaded_fonts: HashMap<(TtfId, u16), Font<'ttf, 'static>>,
    /// White-rasterized glyph textures keyed by `(TtfId, size_px, char)`.
    glyph_cache: HashMap<GlyphKey, GlyphTexture<'tc>>,
    /// DPI scale factor applied to logical point sizes during rasterization.
    dpi_scale: f32,
}

impl<'ttf, 'tc> TextEngine<'ttf, 'tc> {
    /// Builds a new engine and auto-discovers every `.ttf` / `.otf` file
    /// under `fonts_dir`.
    ///
    /// The font files are not opened until the first time a `FontHandle`
    /// using them is rendered or measured.
    ///
    /// # Arguments
    ///
    /// * `ttf_ctx` - Shared SDL TTF context (one per process).
    /// * `creator` - Texture creator bound to the rendering canvas.
    /// * `fonts_dir` - Directory to scan for TTF font files. A missing or
    ///   unreadable directory is logged and treated as empty.
    /// * `dpi_scale` - Multiplier applied to logical point sizes when
    ///   loading fonts. `1.0` disables scaling; values above `1.0`
    ///   rasterize at higher resolution for crisper text on high-DPI
    ///   displays.
    ///
    /// # Returns
    ///
    /// A new `TextEngine` with all discovered font stems registered.
    pub fn new(
        ttf_ctx: &'ttf Sdl2TtfContext,
        creator: &'tc TextureCreator<WindowContext>,
        fonts_dir: PathBuf,
        dpi_scale: f32,
    ) -> Self {
        let mut engine = Self {
            ttf_ctx,
            creator,
            font_paths: HashMap::new(),
            font_ids_by_stem: HashMap::new(),
            sorted_stems: Vec::new(),
            loaded_fonts: HashMap::new(),
            glyph_cache: HashMap::new(),
            dpi_scale: dpi_scale.max(0.1),
        };
        engine.discover_fonts(&fonts_dir);
        engine
    }

    /// Walks `fonts_dir` (non-recursively) and registers each `.ttf` /
    /// `.otf` file under its filename stem.
    fn discover_fonts(&mut self, fonts_dir: &std::path::Path) {
        let entries = match std::fs::read_dir(fonts_dir) {
            Ok(e) => e,
            Err(err) => {
                log::warn!(
                    "font_cache: cannot read fonts directory {}: {}",
                    fonts_dir.display(),
                    err
                );
                return;
            }
        };
        let mut discovered: Vec<(String, PathBuf)> = Vec::new();
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let ext_ok = path
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e.eq_ignore_ascii_case("ttf") || e.eq_ignore_ascii_case("otf"))
                .unwrap_or(false);
            if !ext_ok {
                continue;
            }
            if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                discovered.push((stem.to_owned(), path));
            }
        }
        discovered.sort_by(|a, b| a.0.cmp(&b.0));
        for (stem, path) in discovered {
            let id = TtfId(self.font_paths.len() as u16);
            self.font_paths.insert(id, path);
            self.font_ids_by_stem.insert(stem.clone(), id);
            self.sorted_stems.push(stem);
        }
        log::info!(
            "font_cache: discovered {} TTF font(s) in {}",
            self.sorted_stems.len(),
            fonts_dir.display()
        );
    }

    /// Returns the `TtfId` for a discovered font by filename stem (no
    /// extension), or `None` if no such font was found.
    ///
    /// # Arguments
    ///
    /// * `stem` - Filename stem, e.g. `"NotoSans-Bold"`.
    ///
    /// # Returns
    ///
    /// * `Some` value when `font_id` produces one, otherwise `None`.
    pub fn font_id(&self, stem: &str) -> Option<TtfId> {
        self.font_ids_by_stem.get(stem).copied()
    }

    /// Returns a [`FontHandle`] for the named TTF font at `size_pt`.
    ///
    /// If no font with that stem was discovered, logs a warning and falls
    /// back to bitmap font 0 so callers never panic. This mirrors the
    /// soft-failure ergonomics of [`crate::gfx_cache::GraphicsCache`].
    ///
    /// # Arguments
    ///
    /// * `stem` - Filename stem, e.g. `"MatrixSans-Regular"`.
    /// * `size_pt` - Logical point size.
    ///
    /// # Returns
    ///
    /// * Value returned by `handle`.
    pub fn handle(&self, stem: &str, size_pt: u16) -> FontHandle {
        match self.font_id(stem) {
            Some(id) => FontHandle::ttf(id, size_pt),
            None => {
                log::warn!(
                    "font_cache: unknown TTF font stem '{}' — falling back to bitmap font 0",
                    stem
                );
                FontHandle::bitmap(0)
            }
        }
    }

    /// Returns the alphabetically-sorted list of discovered TTF font stems.
    ///
    /// # Returns
    ///
    /// * Value returned by `ttf_stems`.
    pub fn ttf_stems(&self) -> &[String] {
        &self.sorted_stems
    }

    /// Updates the DPI scale used when (re-)loading fonts.
    ///
    /// Existing loaded fonts and glyph textures are dropped so subsequent
    /// draws re-rasterize at the new scale.
    ///
    /// # Arguments
    ///
    /// * `dpi_scale` - New DPI scale; clamped to a minimum of `0.1`.
    pub fn set_dpi_scale(&mut self, dpi_scale: f32) {
        let new_scale = dpi_scale.max(0.1);
        if (new_scale - self.dpi_scale).abs() < f32::EPSILON {
            return;
        }
        self.dpi_scale = new_scale;
        self.loaded_fonts.clear();
        self.glyph_cache.clear();
    }

    /// Returns the current DPI scale factor.
    ///
    /// # Returns
    ///
    /// * Value returned by `dpi_scale`.
    pub fn dpi_scale(&self) -> f32 {
        self.dpi_scale
    }

    /// Auto-detects the canvas's logical-to-physical pixel ratio and uses
    /// it as the DPI scale.
    ///
    /// On a HiDPI display with `allow_highdpi` and a logical size set,
    /// the renderer scales destination rects up from logical to physical
    /// pixels. Rasterizing TTF glyphs at only the logical size and letting
    /// SDL upscale them produces blurry text; calling this method picks
    /// the scale factor that lets [`draw_text_handle`] rasterize at full
    /// physical resolution and blit 1:1 to the screen.
    ///
    /// Call this once after creating the canvas (and again whenever the
    /// canvas's logical size changes).
    ///
    /// # Arguments
    ///
    /// * `canvas` - Canvas whose output / logical sizes are sampled.
    ///
    /// # Returns
    ///
    /// * The resulting DPI scale (clamped to >= 0.1), or an SDL error.
    pub fn sync_dpi_scale_from_canvas(&mut self, canvas: &Canvas<Window>) -> Result<f32, String> {
        let (out_w, out_h) = canvas.output_size()?;
        let (logical_w, logical_h) = canvas.logical_size();
        let scale = if logical_w > 0 && logical_h > 0 {
            (out_w as f32 / logical_w as f32).max(out_h as f32 / logical_h as f32)
        } else {
            // Logical size disabled: caller draws in raw physical pixels,
            // so no extra rasterization scaling is required.
            1.0
        };
        self.set_dpi_scale(scale);
        Ok(self.dpi_scale)
    }

    /// Converts a logical point size to its DPI-scaled pixel size.
    fn size_pt_to_px(&self, size_pt: u16) -> u16 {
        (f32::from(size_pt) * self.dpi_scale).round().max(1.0) as u16
    }

    /// Returns the loaded `Font` for `(id, size_pt)`, loading it on demand.
    fn font_for(&mut self, id: TtfId, size_pt: u16) -> Result<&Font<'ttf, 'static>, String> {
        let size_px = self.size_pt_to_px(size_pt);
        if !self.loaded_fonts.contains_key(&(id, size_px)) {
            let path = self
                .font_paths
                .get(&id)
                .ok_or_else(|| format!("font_cache: TtfId({}) is not registered", id.0))?
                .clone();
            let mut font = self.ttf_ctx.load_font(&path, size_px).map_err(|e| {
                format!(
                    "font_cache: failed to load {} @ {}px: {}",
                    path.display(),
                    size_px,
                    e
                )
            })?;
            // Light hinting keeps small sizes crisp without the heavy
            // pixel-snapping of full hinting (which distorts glyph
            // proportions at large sizes).
            font.set_hinting(Hinting::None);
            self.loaded_fonts.insert((id, size_px), font);
        }
        Ok(&self.loaded_fonts[&(id, size_px)])
    }

    /// Rasterizes (and caches) the given glyph as a white blended texture.
    ///
    /// Returns `Ok(None)` for characters the font cannot render.
    fn ensure_glyph(
        &mut self,
        id: TtfId,
        size_pt: u16,
        ch: char,
    ) -> Result<Option<&GlyphTexture<'tc>>, String> {
        let size_px = self.size_pt_to_px(size_pt);
        let key = GlyphKey {
            font: id,
            size_px,
            ch,
        };
        if self.glyph_cache.contains_key(&key) {
            return Ok(self.glyph_cache.get(&key));
        }

        let font = self.font_for(id, size_pt)?;
        let surface = match font
            .render_char(ch)
            .blended(Color::RGBA(255, 255, 255, 255))
        {
            Ok(surf) => surf,
            Err(_) => return Ok(None),
        };
        let width_px = surface.width();
        let height_px = surface.height();
        if width_px == 0 || height_px == 0 {
            return Ok(None);
        }
        let texture = self
            .creator
            .create_texture_from_surface(&surface)
            .map_err(|e| format!("font_cache: create_texture_from_surface: {}", e))?;
        self.glyph_cache.insert(
            key,
            GlyphTexture {
                texture,
                width_px,
                height_px,
            },
        );
        Ok(self.glyph_cache.get(&key))
    }
}

/// Returns the rendered `(width_logical, height_logical)` of `text` for the
/// given font handle.
///
/// Bitmap widths are exact (monospace `BITMAP_GLYPH_ADVANCE` per char).
/// TTF widths come from `Font::size_of`, divided by the DPI scale so
/// layout math stays in logical coordinates.
///
/// # Arguments
///
/// * `engine` - Text engine (used only for the TTF branch).
/// * `handle` - Font handle.
/// * `text` - Text to measure.
///
/// # Returns
///
/// * `(width, height)` in logical pixels.
pub fn text_size(engine: &mut TextEngine<'_, '_>, handle: &FontHandle, text: &str) -> (u32, u32) {
    match *handle {
        FontHandle::Bitmap { .. } => (text_width(text), BITMAP_GLYPH_H),
        FontHandle::Truetype { id, size_pt } => match engine.font_for(id, size_pt) {
            Ok(font) => match font.size_of(text) {
                Ok((w_px, h_px)) => {
                    let scale = engine.dpi_scale.max(0.1);
                    (
                        ((w_px as f32) / scale).round() as u32,
                        ((h_px as f32) / scale).round() as u32,
                    )
                }
                Err(_) => (0, line_height(engine, handle)),
            },
            Err(err) => {
                log::warn!("{}", err);
                (0, BITMAP_GLYPH_H)
            }
        },
    }
}

/// Returns the line height (in logical pixels) used by the given font.
///
/// For bitmap fonts this is `BITMAP_GLYPH_H` (10). For TTF this is
/// `Font::recommended_line_spacing` divided by the DPI scale.
///
/// # Arguments
///
/// * `engine` - Text engine (used only for the TTF branch).
/// * `handle` - Font handle.
///
/// # Returns
///
/// * Value returned by `line_height`.
pub fn line_height(engine: &mut TextEngine<'_, '_>, handle: &FontHandle) -> u32 {
    match *handle {
        FontHandle::Bitmap { .. } => BITMAP_GLYPH_H,
        FontHandle::Truetype { id, size_pt } => match engine.font_for(id, size_pt) {
            Ok(font) => {
                let h_px = font.recommended_line_spacing().max(1) as f32;
                (h_px / engine.dpi_scale.max(0.1)).round().max(1.0) as u32
            }
            Err(err) => {
                log::warn!("{}", err);
                BITMAP_GLYPH_H
            }
        },
    }
}

/// Draws `text` onto `canvas` using the given font handle.
///
/// Bitmap handles delegate to the standalone [`draw_text`] above. TTF
/// handles iterate the string character-by-character, using cached white
/// glyph textures retinted via `set_color_mod` / `set_alpha_mod`.
///
/// When `style.centered` is true `x` is treated as the horizontal center.
/// When `style.drop_shadow` is true a 1-pixel black shadow is drawn behind
/// the text.
///
/// # Arguments
///
/// * `canvas` - SDL2 canvas to draw onto.
/// * `engine` - Text engine; required for the TTF branch.
/// * `gfx_cache` - Graphics cache; required for the bitmap branch.
/// * `handle` - Font handle to render with.
/// * `text` - Text to draw.
/// * `x` - Left edge (or horizontal center when `style.centered`).
/// * `y` - Top edge in logical pixels.
/// * `style` - Style flags.
///
/// # Returns
///
/// `Ok(())` on success or an SDL error message string.
#[allow(clippy::too_many_arguments)]
pub fn draw_text_handle(
    canvas: &mut Canvas<Window>,
    engine: &mut TextEngine<'_, '_>,
    gfx_cache: &mut crate::gfx_cache::GraphicsCache<'_>,
    handle: &FontHandle,
    text: &str,
    x: i32,
    y: i32,
    style: TextStyle,
) -> Result<(), String> {
    match *handle {
        FontHandle::Bitmap { id } => draw_text(canvas, gfx_cache, id as usize, text, x, y, style),
        FontHandle::Truetype { id, size_pt } => {
            draw_ttf_text(canvas, engine, id, size_pt, text, x, y, style)
        }
    }
}

/// Renders TTF text by iterating cached glyphs.
#[allow(clippy::too_many_arguments)]
fn draw_ttf_text(
    canvas: &mut Canvas<Window>,
    engine: &mut TextEngine<'_, '_>,
    id: TtfId,
    size_pt: u16,
    text: &str,
    x: i32,
    y: i32,
    style: TextStyle,
) -> Result<(), String> {
    let scale = engine.dpi_scale.max(0.1);

    let draw_x = if style.centered {
        let (w_logical, _) = text_size(engine, &FontHandle::Truetype { id, size_pt }, text);
        x - (w_logical as i32) / 2
    } else {
        x
    };

    if style.drop_shadow {
        draw_ttf_text_impl(
            canvas,
            engine,
            id,
            size_pt,
            text,
            draw_x + 1,
            y + 1,
            Color::RGB(0, 0, 0),
            style.alpha,
            scale,
        )?;
    }

    let color = style.tint.unwrap_or(Color::RGBA(255, 255, 255, 255));
    draw_ttf_text_impl(
        canvas,
        engine,
        id,
        size_pt,
        text,
        draw_x,
        y,
        color,
        style.alpha,
        scale,
    )
}

/// Inner TTF glyph blit loop.
#[allow(clippy::too_many_arguments)]
fn draw_ttf_text_impl(
    canvas: &mut Canvas<Window>,
    engine: &mut TextEngine<'_, '_>,
    id: TtfId,
    size_pt: u16,
    text: &str,
    x: i32,
    y: i32,
    color: Color,
    alpha: Option<u8>,
    scale: f32,
) -> Result<(), String> {
    // Glyph textures are rasterized at `size_px = size_pt * scale` physical
    // pixels, but the caller passes `x`/`y` in their canvas coordinate space
    // (logical units when `set_logical_size` is active, or raw pixels when
    // it isn't). To blit the high-resolution texture 1:1 against the screen,
    // we keep the destination rect in caller coordinates and shrink its
    // width/height by `scale` so SDL's logical-to-physical scaling restores
    // them to the full `size_px` on the backbuffer.
    let inv_scale = 1.0_f32 / scale.max(0.1);
    let size_px = engine.size_pt_to_px(size_pt);
    let mut cursor_x: f32 = x as f32;
    let mut first_error: Option<String> = None;

    for ch in text.chars() {
        if engine.ensure_glyph(id, size_pt, ch)?.is_none() {
            cursor_x += BITMAP_GLYPH_ADVANCE as f32;
            continue;
        }

        let key = GlyphKey {
            font: id,
            size_px,
            ch,
        };
        let glyph = engine
            .glyph_cache
            .get_mut(&key)
            .expect("glyph just ensured to be cached");
        let w_caller = ((glyph.width_px as f32) * inv_scale).round().max(1.0) as u32;
        let h_caller = ((glyph.height_px as f32) * inv_scale).round().max(1.0) as u32;
        let dst_x = cursor_x.round() as i32;
        cursor_x += (glyph.width_px as f32) * inv_scale;

        glyph.texture.set_color_mod(color.r, color.g, color.b);
        glyph.texture.set_alpha_mod(alpha.unwrap_or(255));
        let dst = sdl2::rect::Rect::new(dst_x, y, w_caller, h_caller);
        let res = canvas.copy(&glyph.texture, None, Some(dst));
        glyph.texture.set_color_mod(255, 255, 255);
        glyph.texture.set_alpha_mod(255);
        if let Err(err) = res {
            first_error = Some(err);
            break;
        }
    }

    if let Some(err) = first_error {
        return Err(err);
    }
    Ok(())
}

/// Draws word-wrapped `text` within `max_width` logical pixels using the
/// given font handle.
///
/// Splits `text` at ASCII whitespace and emits as many lines as required.
/// Words wider than `max_width` are hard-broken at the character boundary.
/// Lines are stacked using [`line_height`] for the chosen font.
///
/// # Arguments
///
/// * `canvas` - SDL2 canvas to draw onto.
/// * `engine` - Text engine (TTF branch only).
/// * `gfx_cache` - Graphics cache (bitmap branch only).
/// * `handle` - Font handle.
/// * `text` - Text to render.
/// * `x` - Left edge in logical pixels.
/// * `y` - Top edge of the first line in logical pixels.
/// * `max_width` - Maximum line width in logical pixels.
/// * `style` - Style flags (centering is ignored — always left-aligned).
///
/// # Returns
///
/// * Number of lines drawn, or an SDL error string.
#[allow(clippy::too_many_arguments)]
pub fn draw_wrapped_text_handle(
    canvas: &mut Canvas<Window>,
    engine: &mut TextEngine<'_, '_>,
    gfx_cache: &mut crate::gfx_cache::GraphicsCache<'_>,
    handle: &FontHandle,
    text: &str,
    x: i32,
    y: i32,
    max_width: u32,
    style: TextStyle,
) -> Result<u32, String> {
    let line_h = line_height(engine, handle) as i32;
    let mut cur_y = y;
    let mut lines_drawn = 0u32;

    let lines = wrap_lines_handle(engine, handle, text, max_width);
    for line in &lines {
        draw_text_handle(canvas, engine, gfx_cache, handle, line, x, cur_y, style)?;
        cur_y += line_h;
        lines_drawn += 1;
    }
    Ok(lines_drawn)
}

/// Word-wraps `text` to `max_width` logical pixels using [`text_size`] as
/// the measurer. Pulled out of [`draw_wrapped_text_handle`] so it can be
/// unit-tested with a bitmap font (no SDL context required).
fn wrap_lines_handle(
    engine: &mut TextEngine<'_, '_>,
    handle: &FontHandle,
    text: &str,
    max_width: u32,
) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut current = String::new();

    for word in text.split(' ') {
        let (word_w, _) = text_size(engine, handle, word);
        if word_w > max_width {
            if !current.is_empty() {
                out.push(std::mem::take(&mut current));
            }
            let mut buf = String::new();
            for ch in word.chars() {
                let candidate = {
                    let mut s = buf.clone();
                    s.push(ch);
                    s
                };
                let (cw, _) = text_size(engine, handle, &candidate);
                if cw > max_width && !buf.is_empty() {
                    out.push(std::mem::take(&mut buf));
                }
                buf.push(ch);
            }
            if !buf.is_empty() {
                out.push(buf);
            }
            continue;
        }

        let candidate = if current.is_empty() {
            word.to_owned()
        } else {
            format!("{} {}", current, word)
        };
        let (cw, _) = text_size(engine, handle, &candidate);
        if cw <= max_width {
            current = candidate;
        } else {
            if !current.is_empty() {
                out.push(std::mem::take(&mut current));
            }
            current = word.to_owned();
        }
    }

    if !current.is_empty() {
        out.push(current);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glyph_index_space() {
        assert_eq!(glyph_index(' '), 0);
    }

    #[test]
    fn glyph_index_uppercase_a() {
        // 'A' = 65, 65 - 32 = 33
        assert_eq!(glyph_index('A'), 33);
    }

    #[test]
    fn glyph_index_tilde() {
        // '~' = 126, 126 - 32 = 94
        assert_eq!(glyph_index('~'), 94);
    }

    #[test]
    fn glyph_index_del_char() {
        // DEL = 127, 127 - 32 = 95
        assert_eq!(glyph_index('\x7F'), 95);
    }

    #[test]
    fn glyph_index_non_printable() {
        assert_eq!(glyph_index('\t'), -1);
        assert_eq!(glyph_index('\n'), -1);
    }

    #[test]
    fn glyph_index_high_unicode() {
        assert_eq!(glyph_index('€'), -1);
    }

    #[test]
    fn text_width_empty() {
        assert_eq!(text_width(""), 0);
    }

    #[test]
    fn text_width_hello() {
        assert_eq!(text_width("Hello"), 30); // 5 * 6
    }

    #[test]
    fn text_width_single_char() {
        assert_eq!(text_width("X"), 6);
    }

    #[test]
    fn font_handle_constructors() {
        assert_eq!(FontHandle::bitmap(2), FontHandle::Bitmap { id: 2 });
        let id = TtfId(7);
        assert_eq!(
            FontHandle::ttf(id, 14),
            FontHandle::Truetype { id, size_pt: 14 }
        );
    }

    /// Word-wrap with bitmap-font measurement: the algorithm should keep
    /// short words on one line and break only when the cumulative width
    /// exceeds `max_width`.
    #[test]
    fn wrap_lines_bitmap_word_break() {
        // Each char is 6px advance. Words: "abc def ghi" = lengths 3,3,3.
        // Line widths: "abc def" = 7*6 = 42, "abc def ghi" = 11*6 = 66.
        // With max_width = 50 we expect ["abc def", "ghi"].
        let lines = bitmap_wrap_for_test("abc def ghi", 50);
        assert_eq!(lines, vec!["abc def".to_owned(), "ghi".to_owned()]);
    }

    #[test]
    fn wrap_lines_bitmap_hard_break_long_word() {
        // "abcdefgh" = 8*6 = 48. With max_width = 30 (=5 chars), expect
        // ["abcde", "fgh"].
        let lines = bitmap_wrap_for_test("abcdefgh", 30);
        assert_eq!(lines, vec!["abcde".to_owned(), "fgh".to_owned()]);
    }

    #[test]
    fn wrap_lines_bitmap_empty_input() {
        let lines = bitmap_wrap_for_test("", 100);
        assert!(lines.is_empty());
    }

    /// Pure-Rust mirror of [`wrap_lines_handle`] using the bitmap measurer
    /// directly, so we can unit-test the wrap algorithm without an SDL
    /// context. Must mirror `wrap_lines_handle` exactly when `handle` is
    /// bitmap.
    fn bitmap_wrap_for_test(text: &str, max_width: u32) -> Vec<String> {
        let measure = text_width;
        let mut out: Vec<String> = Vec::new();
        let mut current = String::new();
        for word in text.split(' ') {
            if word.is_empty() && text.is_empty() {
                continue;
            }
            if measure(word) > max_width {
                if !current.is_empty() {
                    out.push(std::mem::take(&mut current));
                }
                let mut buf = String::new();
                for ch in word.chars() {
                    let mut candidate = buf.clone();
                    candidate.push(ch);
                    if measure(&candidate) > max_width && !buf.is_empty() {
                        out.push(std::mem::take(&mut buf));
                    }
                    buf.push(ch);
                }
                if !buf.is_empty() {
                    out.push(buf);
                }
                continue;
            }
            let candidate = if current.is_empty() {
                word.to_owned()
            } else {
                format!("{} {}", current, word)
            };
            if measure(&candidate) <= max_width {
                current = candidate;
            } else {
                if !current.is_empty() {
                    out.push(std::mem::take(&mut current));
                }
                current = word.to_owned();
            }
        }
        if !current.is_empty() {
            out.push(current);
        }
        out
    }
}
