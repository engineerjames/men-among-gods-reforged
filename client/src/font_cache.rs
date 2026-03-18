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
    pub fn faded(alpha: u8) -> Self {
        Self {
            alpha: Some(alpha),
            ..Self::PLAIN
        }
    }

    /// Creates a centered style with no tint or alpha.
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
    pub fn with_tint(mut self, color: sdl2::pixels::Color) -> Self {
        self.tint = Some(color);
        self
    }

    /// Returns a copy of this style with a 1-pixel black drop shadow enabled.
    pub fn with_drop_shadow(mut self) -> Self {
        self.drop_shadow = true;
        self
    }

    /// Creates a plain style with a 1-pixel black drop shadow.
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
            word.to_string()
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
            current_line = word.to_string();
        }
    }

    // Flush the final line.
    if !current_line.is_empty() {
        draw_text(canvas, gfx_cache, font, &current_line, x, cur_y, style)?;
        lines_drawn += 1;
    }

    Ok(lines_drawn)
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
}
