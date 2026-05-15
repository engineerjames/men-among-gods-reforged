//! Unified text rendering API.
//!
//! Wraps both the legacy bitmap glyph fonts (see [`crate::font_cache`]) and
//! TrueType fonts rasterized through `sdl2::ttf` behind a single
//! [`FontHandle`] enum. Use this module for new UI/menu code; existing call
//! sites that still call `font_cache::draw_text` continue to work unchanged
//! and route through the bitmap branch here automatically when migrated.
//!
//! ## Threading and lifetimes
//!
//! [`TextEngine`] borrows the SDL `Sdl2TtfContext` (`'ttf`) and the canvas's
//! `TextureCreator` (`'tc`). Construct a single instance during application
//! startup and thread `&mut TextEngine` through scene/widget draw signatures
//! alongside `GraphicsCache`.
//!
//! ## Sizing model
//!
//! TrueType sizes are specified in *logical points*. The engine multiplies
//! the size by the configured DPI scale when calling `load_font`, so the
//! rasterized glyphs match physical pixel density even though widths and
//! line heights returned by [`text_size`] / [`line_height`] are reported in
//! the logical (1920×1080) coordinate space used by `canvas.set_logical_size`.
//!
//! ## Caching
//!
//! Fonts are loaded lazily on first use and keyed by `(TtfId, size_px)`.
//! Glyphs are rasterized white (anti-aliased via `Font::blended`) and
//! re-tinted at draw time via SDL `set_color_mod` / `set_alpha_mod`, which
//! mirrors the bitmap branch and keeps the cache memory bounded by glyph
//! count rather than (glyph × color) combinations.

use std::collections::HashMap;
use std::path::PathBuf;

use sdl2::pixels::Color;
use sdl2::render::{Canvas, Texture, TextureCreator};
use sdl2::ttf::{Font, Sdl2TtfContext};
use sdl2::video::{Window, WindowContext};

use crate::font_cache::{self, BITMAP_GLYPH_ADVANCE, BITMAP_GLYPH_H, TextStyle};
use crate::gfx_cache::GraphicsCache;

pub use crate::font_cache::TextStyle as Style;

/// Numeric identifier for a registered TrueType font face.
///
/// Use the [`UI_REGULAR`] / [`UI_BOLD`] constants when referring to the
/// fonts bundled with the client; new faces should be registered via
/// [`TextEngine::register_font`] with their own `TtfId`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TtfId(pub u16);

/// The bundled regular-weight UI font (Noto Sans Regular).
pub const UI_REGULAR: TtfId = TtfId(0);

/// The bundled bold-weight UI font (Noto Sans Bold).
pub const UI_BOLD: TtfId = TtfId(1);

/// A handle identifying which font and (for TTF) which size to render with.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FontHandle {
    /// Legacy bitmap font (`font_cache`). `id` is the 0–3 sprite-sheet index.
    Bitmap {
        /// Bitmap font sheet index (0–3).
        id: u8,
    },
    /// TrueType font registered with the [`TextEngine`].
    Truetype {
        /// Identifier returned/used when registering the face.
        id: TtfId,
        /// Logical point size; DPI-scaled internally during rasterization.
        size_pt: u16,
    },
}

impl FontHandle {
    /// Convenience constructor for a bitmap font handle.
    ///
    /// # Arguments
    /// * `id` - Bitmap font sheet index (0–3).
    pub const fn bitmap(id: u8) -> Self {
        FontHandle::Bitmap { id }
    }

    /// Convenience constructor for a TrueType font handle.
    ///
    /// # Arguments
    /// * `id` - Identifier of a previously-registered face.
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
/// Owns lazily-loaded fonts and rasterized glyph textures. Construct one per
/// application instance and pass a mutable reference into draw paths.
pub struct TextEngine<'ttf, 'tc> {
    ttf_ctx: &'ttf Sdl2TtfContext,
    creator: &'tc TextureCreator<WindowContext>,
    /// Filesystem paths registered for each `TtfId`, used on lazy load.
    font_paths: HashMap<TtfId, PathBuf>,
    /// Loaded font faces keyed by `(TtfId, size_px)`.
    loaded_fonts: HashMap<(TtfId, u16), Font<'ttf, 'static>>,
    /// White-rasterized glyph textures keyed by `(TtfId, size_px, char)`.
    glyph_cache: HashMap<GlyphKey, GlyphTexture<'tc>>,
    /// DPI scale factor applied to logical point sizes during rasterization.
    dpi_scale: f32,
}

impl<'ttf, 'tc> TextEngine<'ttf, 'tc> {
    /// Builds a new engine borrowing the given SDL TTF context and texture
    /// creator. No fonts are loaded yet — call [`register_font`] for each
    /// face you want to render.
    ///
    /// # Arguments
    /// * `ttf_ctx` - Shared SDL TTF context (one per process).
    /// * `creator` - Texture creator bound to the rendering canvas.
    /// * `dpi_scale` - Multiplier applied to logical point sizes when loading
    ///   fonts. `1.0` disables scaling; values above `1.0` rasterize at
    ///   higher resolution for crisper text on high-DPI displays.
    ///
    /// [`register_font`]: TextEngine::register_font
    pub fn new(
        ttf_ctx: &'ttf Sdl2TtfContext,
        creator: &'tc TextureCreator<WindowContext>,
        dpi_scale: f32,
    ) -> Self {
        Self {
            ttf_ctx,
            creator,
            font_paths: HashMap::new(),
            loaded_fonts: HashMap::new(),
            glyph_cache: HashMap::new(),
            dpi_scale: dpi_scale.max(0.1),
        }
    }

    /// Registers a TrueType font file under the given `id`.
    ///
    /// The file is not opened until the first time a [`FontHandle`] using
    /// this `id` is rendered or measured. Re-registering the same `id`
    /// replaces the previous mapping and evicts any cached fonts/glyphs for
    /// it.
    ///
    /// # Arguments
    /// * `id` - Identifier callers will reference via [`FontHandle::ttf`].
    /// * `path` - Filesystem path to the `.ttf` / `.otf` file.
    pub fn register_font(&mut self, id: TtfId, path: PathBuf) {
        self.font_paths.insert(id, path);
        self.loaded_fonts.retain(|(fid, _), _| *fid != id);
        self.glyph_cache.retain(|key, _| key.font != id);
    }

    /// Updates the DPI scale used when (re-)loading fonts.
    ///
    /// Existing loaded fonts and glyph textures are dropped so subsequent
    /// draws re-rasterize at the new scale.
    ///
    /// # Arguments
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
    pub fn dpi_scale(&self) -> f32 {
        self.dpi_scale
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
                .ok_or_else(|| format!("text: TtfId({}) is not registered", id.0))?
                .clone();
            let font = self.ttf_ctx.load_font(&path, size_px).map_err(|e| {
                format!(
                    "text: failed to load {} @ {}px: {}",
                    path.display(),
                    size_px,
                    e
                )
            })?;
            self.loaded_fonts.insert((id, size_px), font);
        }
        Ok(&self.loaded_fonts[&(id, size_px)])
    }

    /// Rasterizes (and caches) the given glyph as a white blended texture.
    ///
    /// Returns `Ok(None)` for characters the font cannot render (e.g. a
    /// missing glyph that produces a zero-sized surface); callers should
    /// simply advance by a fallback width in that case.
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
            .map_err(|e| format!("text: create_texture_from_surface: {}", e))?;
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
/// TTF widths are derived from `Font::size_of`, then divided by the DPI
/// scale so layout math stays in logical coordinates.
///
/// # Arguments
/// * `engine` - Text engine (used only for the TTF branch).
/// * `handle` - Font handle.
/// * `text` - Text to measure.
///
/// # Returns
/// * `(width, height)` in logical pixels.
///
/// # Panics
/// Never; falls back to bitmap metrics on TTF errors.
pub fn text_size(engine: &mut TextEngine<'_, '_>, handle: &FontHandle, text: &str) -> (u32, u32) {
    match *handle {
        FontHandle::Bitmap { .. } => (font_cache::text_width(text), BITMAP_GLYPH_H),
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
/// * `engine` - Text engine (used only for the TTF branch).
/// * `handle` - Font handle.
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
/// Bitmap handles delegate to [`font_cache::draw_text`]. TTF handles iterate
/// the string character-by-character, using cached white glyph textures
/// retinted via `set_color_mod` / `set_alpha_mod`.
///
/// When `style.centered` is true `x` is treated as the horizontal center.
/// When `style.drop_shadow` is true a 1-logical-pixel black shadow is drawn
/// behind the text.
///
/// # Arguments
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
/// * `Ok(())` on success or an SDL error message string.
pub fn draw_text(
    canvas: &mut Canvas<Window>,
    engine: &mut TextEngine<'_, '_>,
    gfx_cache: &mut GraphicsCache<'_>,
    handle: &FontHandle,
    text: &str,
    x: i32,
    y: i32,
    style: TextStyle,
) -> Result<(), String> {
    match *handle {
        FontHandle::Bitmap { id } => {
            font_cache::draw_text(canvas, gfx_cache, id as usize, text, x, y, style)
        }
        FontHandle::Truetype { id, size_pt } => {
            draw_ttf_text(canvas, engine, id, size_pt, text, x, y, style)
        }
    }
}

/// Renders TTF text by iterating cached glyphs.
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

/// Inner glyph blit loop.
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
    let size_px = engine.size_pt_to_px(size_pt);
    let mut cursor_x_px: f32 = (x as f32) * scale;
    let y_px = ((y as f32) * scale).round() as i32;
    let mut first_error: Option<String> = None;

    for ch in text.chars() {
        // Ensure the glyph is cached; skip whitespace-like chars that the
        // font cannot render by advancing one bitmap-glyph width.
        if engine.ensure_glyph(id, size_pt, ch)?.is_none() {
            cursor_x_px += scale * (BITMAP_GLYPH_ADVANCE as f32);
            continue;
        }

        let key = GlyphKey {
            font: id,
            size_px,
            ch,
        };
        // Unwrap is safe: ensure_glyph above returned Some, meaning the
        // entry was just inserted (or already present).
        let glyph = engine
            .glyph_cache
            .get_mut(&key)
            .expect("glyph just ensured to be cached");
        let w_px = glyph.width_px;
        let h_px = glyph.height_px;
        let dst_x = cursor_x_px.round() as i32;
        cursor_x_px += w_px as f32;

        glyph.texture.set_color_mod(color.r, color.g, color.b);
        glyph.texture.set_alpha_mod(alpha.unwrap_or(255));
        let dst = sdl2::rect::Rect::new(dst_x, y_px, w_px, h_px);
        let res = canvas.copy(&glyph.texture, None, Some(dst));
        // Always reset modulation so other consumers see neutral state.
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

/// Draws word-wrapped `text` within `max_width` logical pixels.
///
/// Splits `text` at ASCII whitespace and emits as many lines as required.
/// Words wider than `max_width` are hard-broken at the character boundary.
/// Lines are stacked using [`line_height`] for the chosen font.
///
/// # Arguments
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
/// * Number of lines drawn, or an SDL error string.
pub fn draw_wrapped_text(
    canvas: &mut Canvas<Window>,
    engine: &mut TextEngine<'_, '_>,
    gfx_cache: &mut GraphicsCache<'_>,
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

    let lines = wrap_lines(engine, handle, text, max_width);
    for line in &lines {
        draw_text(canvas, engine, gfx_cache, handle, line, x, cur_y, style)?;
        cur_y += line_h;
        lines_drawn += 1;
    }
    Ok(lines_drawn)
}

/// Word-wraps `text` to `max_width` logical pixels using [`text_size`] as
/// the measurer. Pulled out of `draw_wrapped_text` so it can be unit-tested
/// with a bitmap font (no SDL context required).
///
/// # Arguments
/// * `engine` - Text engine (only consulted for TTF handles).
/// * `handle` - Font handle.
/// * `text` - Source text; ASCII spaces are treated as word separators.
/// * `max_width` - Maximum line width in logical pixels.
fn wrap_lines(
    engine: &mut TextEngine<'_, '_>,
    handle: &FontHandle,
    text: &str,
    max_width: u32,
) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut current = String::new();

    for word in text.split(' ') {
        // Hard-break overlong words.
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
    fn font_handle_constructors() {
        assert_eq!(FontHandle::bitmap(2), FontHandle::Bitmap { id: 2 });
        assert_eq!(
            FontHandle::ttf(UI_REGULAR, 14),
            FontHandle::Truetype {
                id: UI_REGULAR,
                size_pt: 14
            }
        );
    }

    #[test]
    fn ttf_id_constants_distinct() {
        assert_ne!(UI_REGULAR, UI_BOLD);
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

    /// Pure-Rust mirror of [`wrap_lines`] using the bitmap measurer
    /// directly, so we can unit-test the wrap algorithm without an SDL
    /// context. Must mirror `wrap_lines` exactly when `handle` is bitmap.
    fn bitmap_wrap_for_test(text: &str, max_width: u32) -> Vec<String> {
        let measure = |s: &str| font_cache::text_width(s);
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
