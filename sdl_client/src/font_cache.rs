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
/// Each character advances `BITMAP_GLYPH_ADVANCE` pixels horizontally.
pub fn draw_text(
    canvas: &mut sdl2::render::Canvas<sdl2::video::Window>,
    gfx_cache: &mut crate::gfx_cache::GraphicsCache,
    font: usize,
    text: &str,
    x: i32,
    y: i32,
) -> Result<(), String> {
    let sprite_id = BITMAP_FONT_FIRST_SPRITE_ID + (font % BITMAP_FONT_COUNT);

    let mut cx = x;
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
        canvas.copy(texture, Some(src), Some(dst))?;

        cx += BITMAP_GLYPH_ADVANCE as i32;
    }

    Ok(())
}

/// Draws `text` centered horizontally around `center_x`.
pub fn draw_text_centered(
    canvas: &mut sdl2::render::Canvas<sdl2::video::Window>,
    gfx_cache: &mut crate::gfx_cache::GraphicsCache,
    font: usize,
    text: &str,
    center_x: i32,
    y: i32,
) -> Result<(), String> {
    let width = text.len() as i32 * BITMAP_GLYPH_ADVANCE as i32;
    draw_text(canvas, gfx_cache, font, text, center_x - width / 2, y)
}

/// Returns the pixel width of the given text string when rendered with the bitmap font.
#[inline]
pub fn text_width(text: &str) -> u32 {
    (text.len() as u32) * BITMAP_GLYPH_ADVANCE
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
