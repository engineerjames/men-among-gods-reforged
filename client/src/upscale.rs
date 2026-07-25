//! Hand-rolled pixel-art upscaling algorithms.
//!
//! The game's art is authored at 32x32 (64x32 for some characters). When the
//! internal render scale is raised above 1x, sprites would otherwise just be
//! point-replicated, gaining resolution but no detail. These filters
//! reconstruct plausible sub-pixel detail from the original art instead.
//!
//! All functions operate on tightly packed RGBA8 buffers (4 bytes per pixel,
//! row-major, no padding) and return a new buffer of the scaled size.
//!
//! # Alpha handling
//!
//! The classic formulations of these algorithms assume opaque images. Sprites
//! here are cut-outs surrounded by fully transparent pixels, so equality is
//! evaluated on all four channels and interpolation is performed in
//! premultiplied alpha. Without that, the (arbitrary) colour stored in
//! transparent pixels bleeds into the sprite's silhouette and produces dark
//! halos around every character.

/// A single RGBA8 pixel.
type Rgba = [u8; 4];

/// Reads the pixel at `(x, y)`, clamping coordinates to the image bounds.
///
/// Clamping (rather than wrapping or treating out-of-bounds as transparent)
/// matches the reference implementations of Scale2x/HQx and avoids eroding the
/// outermost row and column of a sprite.
///
/// # Arguments
///
/// * `src` - Tightly packed RGBA8 source buffer.
/// * `w` - Source width in pixels.
/// * `h` - Source height in pixels.
/// * `x` - X coordinate, may be out of range.
/// * `y` - Y coordinate, may be out of range.
///
/// # Returns
///
/// * The clamped pixel value.
fn px(src: &[u8], w: usize, h: usize, x: isize, y: isize) -> Rgba {
    let cx = x.clamp(0, w as isize - 1) as usize;
    let cy = y.clamp(0, h as isize - 1) as usize;
    let i = (cy * w + cx) * 4;
    [src[i], src[i + 1], src[i + 2], src[i + 3]]
}

/// Writes a pixel into a destination buffer.
///
/// # Arguments
///
/// * `dst` - Destination RGBA8 buffer.
/// * `w` - Destination width in pixels.
/// * `x` - X coordinate, must be in range.
/// * `y` - Y coordinate, must be in range.
/// * `value` - Pixel to write.
fn put(dst: &mut [u8], w: usize, x: usize, y: usize, value: Rgba) {
    let i = (y * w + x) * 4;
    dst[i..i + 4].copy_from_slice(&value);
}

/// Returns the luma of a pixel, weighted by its alpha.
///
/// # Arguments
///
/// * `p` - The pixel to measure.
///
/// # Returns
///
/// * Approximate luminance in `0..=255`.
fn luma(p: Rgba) -> i32 {
    (i32::from(p[0]) * 299 + i32::from(p[1]) * 587 + i32::from(p[2]) * 114) / 1000
}

/// Perceptual distance threshold below which two pixels count as "the same"
/// for the purposes of HQx edge detection.
const HQX_LUMA_THRESHOLD: i32 = 48;
/// Alpha difference above which two pixels are always considered different.
const HQX_ALPHA_THRESHOLD: i32 = 24;

/// Returns whether two pixels should be treated as equal.
///
/// Fully transparent pixels are equal to each other regardless of their stored
/// RGB, because that colour is meaningless and varies between authoring tools.
///
/// # Arguments
///
/// * `a` - First pixel.
/// * `b` - Second pixel.
///
/// # Returns
///
/// * `true` when the pixels are perceptually interchangeable.
fn similar(a: Rgba, b: Rgba) -> bool {
    if a[3] == 0 && b[3] == 0 {
        return true;
    }
    if (i32::from(a[3]) - i32::from(b[3])).abs() > HQX_ALPHA_THRESHOLD {
        return false;
    }
    (luma(a) - luma(b)).abs() <= HQX_LUMA_THRESHOLD
        && (i32::from(a[0]) - i32::from(b[0])).abs() <= 96
        && (i32::from(a[1]) - i32::from(b[1])).abs() <= 96
        && (i32::from(a[2]) - i32::from(b[2])).abs() <= 96
}

/// Returns whether two pixels are bit-for-bit identical, treating all fully
/// transparent pixels as identical.
///
/// Scale2x/Scale3x are exact-match algorithms; using the fuzzy [`similar`]
/// comparison there would round off intentional single-pixel detail.
///
/// # Arguments
///
/// * `a` - First pixel.
/// * `b` - Second pixel.
///
/// # Returns
///
/// * `true` when the pixels are equal.
fn exact(a: Rgba, b: Rgba) -> bool {
    if a[3] == 0 && b[3] == 0 {
        return true;
    }
    a == b
}

/// Returns whether two pixels sit on the same side of the alpha silhouette.
///
/// Every filter here may substitute or blend a neighbour into a sub-pixel of
/// the centre pixel's block. Doing that across the sprite's silhouette changes
/// its coverage: an opaque edge pixel loses part of its area to a transparent
/// neighbour (erosion), or the transparent surround gains a semi-opaque fringe
/// (dilation).
///
/// That is very visible on this game's isometric floor tiles. They are 32x32
/// images containing a 32x16 diamond, and adjacent diamonds interlock exactly.
/// Eroding each staircase step by a quarter-pixel opens a gap along every
/// shared edge, which reads in-game as a dark outline around every tile.
///
/// Requiring equal alpha before substituting keeps the silhouette bit-exact
/// while still letting the filters smooth colour steps inside the sprite and
/// inside the transparent surround.
///
/// # Arguments
///
/// * `a` - First pixel.
/// * `b` - Second pixel.
///
/// # Returns
///
/// * `true` when the two pixels have identical alpha.
fn same_coverage(a: Rgba, b: Rgba) -> bool {
    a[3] == b[3]
}

/// Fetches the eight neighbours of `(x, y)`, clamped to the alpha silhouette.
///
/// Any neighbour whose alpha differs from the centre's is replaced by the
/// centre pixel itself. The HQx filters blend the values they are given, so
/// this is what keeps them from softening a sprite's outline into a
/// semi-transparent fringe — see [`same_coverage`] for why that matters here.
///
/// # Arguments
///
/// * `src` - Tightly packed RGBA8 source buffer.
/// * `w` - Source width in pixels.
/// * `h` - Source height in pixels.
/// * `x` - Centre X coordinate.
/// * `y` - Centre Y coordinate.
/// * `e` - The centre pixel.
///
/// # Returns
///
/// * The neighbours in reading order, skipping the centre:
///   `[a, b, c, d, f, g, h, i]`.
fn neighbourhood(src: &[u8], w: usize, h: usize, x: isize, y: isize, e: Rgba) -> [Rgba; 8] {
    let mut out = [
        px(src, w, h, x - 1, y - 1),
        px(src, w, h, x, y - 1),
        px(src, w, h, x + 1, y - 1),
        px(src, w, h, x - 1, y),
        px(src, w, h, x + 1, y),
        px(src, w, h, x - 1, y + 1),
        px(src, w, h, x, y + 1),
        px(src, w, h, x + 1, y + 1),
    ];
    for n in &mut out {
        if !same_coverage(e, *n) {
            *n = e;
        }
    }
    out
}

/// Blends pixels in premultiplied alpha using integer weights.
///
/// Straight-alpha averaging would pull the RGB of transparent neighbours into
/// the result; premultiplying makes transparent contributors affect only the
/// output alpha, which is what produces clean anti-aliased sprite edges.
///
/// # Arguments
///
/// * `parts` - Slice of `(pixel, weight)` pairs. Weights must sum to a
///   non-zero value.
///
/// # Returns
///
/// * The blended pixel in straight alpha.
fn blend(parts: &[(Rgba, u32)]) -> Rgba {
    let mut total = 0u32;
    let mut acc = [0u32; 4];
    for (p, weight) in parts {
        let w = *weight;
        if w == 0 {
            continue;
        }
        let a = u32::from(p[3]);
        acc[0] += u32::from(p[0]) * a * w;
        acc[1] += u32::from(p[1]) * a * w;
        acc[2] += u32::from(p[2]) * a * w;
        acc[3] += a * w;
        total += w;
    }
    if total == 0 || acc[3] == 0 {
        return [0, 0, 0, 0];
    }
    [
        (acc[0] / acc[3]).min(255) as u8,
        (acc[1] / acc[3]).min(255) as u8,
        (acc[2] / acc[3]).min(255) as u8,
        (acc[3] / total).min(255) as u8,
    ]
}

/// Point-replicates an image by an integer factor.
///
/// # Arguments
///
/// * `src` - Tightly packed RGBA8 source buffer.
/// * `w` - Source width in pixels.
/// * `h` - Source height in pixels.
/// * `factor` - Integer scale factor; values below 2 return a copy.
///
/// # Returns
///
/// * A new RGBA8 buffer of `w * factor` by `h * factor` pixels.
pub fn nearest(src: &[u8], w: usize, h: usize, factor: usize) -> Vec<u8> {
    if factor <= 1 {
        return src.to_vec();
    }
    let dw = w * factor;
    let mut dst = vec![0u8; dw * h * factor * 4];
    for y in 0..h {
        for x in 0..w {
            let p = px(src, w, h, x as isize, y as isize);
            for dy in 0..factor {
                for dx in 0..factor {
                    put(&mut dst, dw, x * factor + dx, y * factor + dy, p);
                }
            }
        }
    }
    dst
}

/// Doubles an image using the Scale2x (EPX) algorithm.
///
/// Scale2x expands each pixel into a 2x2 block, replacing corners where two
/// perpendicular neighbours agree and the diagonal ones do not. It preserves
/// hard edges exactly and never introduces new colours.
///
/// # Arguments
///
/// * `src` - Tightly packed RGBA8 source buffer.
/// * `w` - Source width in pixels.
/// * `h` - Source height in pixels.
///
/// # Returns
///
/// * A new RGBA8 buffer of `w * 2` by `h * 2` pixels.
pub fn scale2x(src: &[u8], w: usize, h: usize) -> Vec<u8> {
    let dw = w * 2;
    let mut dst = vec![0u8; dw * h * 2 * 4];
    for y in 0..h as isize {
        for x in 0..w as isize {
            let e = px(src, w, h, x, y);
            let b = px(src, w, h, x, y - 1);
            let d = px(src, w, h, x - 1, y);
            let f = px(src, w, h, x + 1, y);
            let hh = px(src, w, h, x, y + 1);

            let (mut e0, mut e1, mut e2, mut e3) = (e, e, e, e);
            if !exact(b, hh) && !exact(d, f) {
                if exact(d, b) && same_coverage(e, d) {
                    e0 = d;
                }
                if exact(b, f) && same_coverage(e, f) {
                    e1 = f;
                }
                if exact(d, hh) && same_coverage(e, d) {
                    e2 = d;
                }
                if exact(hh, f) && same_coverage(e, f) {
                    e3 = f;
                }
            }

            let (dx, dy) = (x as usize * 2, y as usize * 2);
            put(&mut dst, dw, dx, dy, e0);
            put(&mut dst, dw, dx + 1, dy, e1);
            put(&mut dst, dw, dx, dy + 1, e2);
            put(&mut dst, dw, dx + 1, dy + 1, e3);
        }
    }
    dst
}

/// Triples an image using the Scale3x algorithm.
///
/// The 3x sibling of [`scale2x`], using the same exact-match rules over a 3x3
/// neighbourhood.
///
/// # Arguments
///
/// * `src` - Tightly packed RGBA8 source buffer.
/// * `w` - Source width in pixels.
/// * `h` - Source height in pixels.
///
/// # Returns
///
/// * A new RGBA8 buffer of `w * 3` by `h * 3` pixels.
pub fn scale3x(src: &[u8], w: usize, h: usize) -> Vec<u8> {
    let dw = w * 3;
    let mut dst = vec![0u8; dw * h * 3 * 4];
    for y in 0..h as isize {
        for x in 0..w as isize {
            let a = px(src, w, h, x - 1, y - 1);
            let b = px(src, w, h, x, y - 1);
            let c = px(src, w, h, x + 1, y - 1);
            let d = px(src, w, h, x - 1, y);
            let e = px(src, w, h, x, y);
            let f = px(src, w, h, x + 1, y);
            let g = px(src, w, h, x - 1, y + 1);
            let hh = px(src, w, h, x, y + 1);
            let i = px(src, w, h, x + 1, y + 1);

            let mut out = [e; 9];
            if !exact(b, hh) && !exact(d, f) {
                if exact(d, b) && same_coverage(e, d) {
                    out[0] = d;
                }
                if ((exact(d, b) && !exact(e, c)) || (exact(b, f) && !exact(e, a)))
                    && same_coverage(e, b)
                {
                    out[1] = b;
                }
                if exact(b, f) && same_coverage(e, f) {
                    out[2] = f;
                }
                if ((exact(d, b) && !exact(e, g)) || (exact(d, hh) && !exact(e, a)))
                    && same_coverage(e, d)
                {
                    out[3] = d;
                }
                if ((exact(b, f) && !exact(e, i)) || (exact(hh, f) && !exact(e, c)))
                    && same_coverage(e, f)
                {
                    out[5] = f;
                }
                if exact(d, hh) && same_coverage(e, d) {
                    out[6] = d;
                }
                if ((exact(d, hh) && !exact(e, i)) || (exact(hh, f) && !exact(e, g)))
                    && same_coverage(e, hh)
                {
                    out[7] = hh;
                }
                if exact(hh, f) && same_coverage(e, f) {
                    out[8] = f;
                }
            }

            let (dx, dy) = (x as usize * 3, y as usize * 3);
            for (idx, p) in out.iter().enumerate() {
                put(&mut dst, dw, dx + idx % 3, dy + idx / 3, *p);
            }
        }
    }
    dst
}

/// Doubles an image using an HQ2x-style interpolating filter.
///
/// Unlike Scale2x, HQx blends across detected edges, which smooths diagonals
/// and curves at the cost of introducing intermediate colours. This is a
/// weighted-neighbourhood formulation rather than the original 256-case lookup
/// table: it produces very similar output for sprite art while remaining
/// readable and alpha-correct.
///
/// # Arguments
///
/// * `src` - Tightly packed RGBA8 source buffer.
/// * `w` - Source width in pixels.
/// * `h` - Source height in pixels.
///
/// # Returns
///
/// * A new RGBA8 buffer of `w * 2` by `h * 2` pixels.
pub fn hq2x(src: &[u8], w: usize, h: usize) -> Vec<u8> {
    let dw = w * 2;
    let mut dst = vec![0u8; dw * h * 2 * 4];
    for y in 0..h as isize {
        for x in 0..w as isize {
            let e = px(src, w, h, x, y);
            let n = neighbourhood(src, w, h, x, y, e);
            let [a, b, c, d, f, g, hh, i] = n;

            let (dx, dy) = (x as usize * 2, y as usize * 2);
            put(&mut dst, dw, dx, dy, hqx_corner(e, d, b, a));
            put(&mut dst, dw, dx + 1, dy, hqx_corner(e, b, f, c));
            put(&mut dst, dw, dx, dy + 1, hqx_corner(e, hh, d, g));
            put(&mut dst, dw, dx + 1, dy + 1, hqx_corner(e, f, hh, i));
        }
    }
    dst
}

/// Triples an image using an HQ3x-style interpolating filter.
///
/// The centre output pixel is always the untouched source pixel, the four
/// edge-adjacent outputs are lightly pulled toward their neighbour, and the
/// four corners use the same corner rule as [`hq2x`].
///
/// # Arguments
///
/// * `src` - Tightly packed RGBA8 source buffer.
/// * `w` - Source width in pixels.
/// * `h` - Source height in pixels.
///
/// # Returns
///
/// * A new RGBA8 buffer of `w * 3` by `h * 3` pixels.
pub fn hq3x(src: &[u8], w: usize, h: usize) -> Vec<u8> {
    let dw = w * 3;
    let mut dst = vec![0u8; dw * h * 3 * 4];
    for y in 0..h as isize {
        for x in 0..w as isize {
            let e = px(src, w, h, x, y);
            let n = neighbourhood(src, w, h, x, y, e);
            let [a, b, c, d, f, g, hh, i] = n;

            let out = [
                hqx_corner(e, d, b, a),
                hqx_edge(e, b, d, f),
                hqx_corner(e, b, f, c),
                hqx_edge(e, d, b, hh),
                e,
                hqx_edge(e, f, b, hh),
                hqx_corner(e, hh, d, g),
                hqx_edge(e, hh, d, f),
                hqx_corner(e, f, hh, i),
            ];

            let (dx, dy) = (x as usize * 3, y as usize * 3);
            for (idx, p) in out.iter().enumerate() {
                put(&mut dst, dw, dx + idx % 3, dy + idx / 3, *p);
            }
        }
    }
    dst
}

/// Computes one corner sub-pixel of an HQx expansion.
///
/// When both perpendicular neighbours differ from the centre but match each
/// other, the corner sits on a diagonal edge and is pulled strongly toward
/// them. When only one differs, a gentle blend anti-aliases the step. When
/// neither differs, the centre is kept verbatim so flat areas stay flat.
///
/// # Arguments
///
/// * `e` - Centre pixel.
/// * `n1` - First perpendicular neighbour.
/// * `n2` - Second perpendicular neighbour.
/// * `diag` - The diagonal neighbour between `n1` and `n2`.
///
/// # Returns
///
/// * The interpolated corner pixel.
fn hqx_corner(e: Rgba, n1: Rgba, n2: Rgba, diag: Rgba) -> Rgba {
    let n1_differs = !similar(e, n1);
    let n2_differs = !similar(e, n2);

    if n1_differs && n2_differs {
        if similar(n1, n2) {
            // Interior of a diagonal edge: follow the edge, not the centre.
            return blend(&[(e, 1), (n1, 1), (n2, 1)]);
        }
        return blend(&[(e, 2), (n1, 1), (n2, 1)]);
    }
    if n1_differs || n2_differs {
        let other = if n1_differs { n1 } else { n2 };
        if similar(e, diag) {
            return blend(&[(e, 5), (other, 1)]);
        }
        return blend(&[(e, 3), (other, 1)]);
    }
    e
}

/// Computes one edge sub-pixel of an HQ3x expansion.
///
/// # Arguments
///
/// * `e` - Centre pixel.
/// * `toward` - The neighbour this sub-pixel faces.
/// * `side_a` - One neighbour perpendicular to `toward`.
/// * `side_b` - The other neighbour perpendicular to `toward`.
///
/// # Returns
///
/// * The interpolated edge pixel.
fn hqx_edge(e: Rgba, toward: Rgba, side_a: Rgba, side_b: Rgba) -> Rgba {
    if similar(e, toward) {
        return e;
    }
    // Only soften the step when the edge is genuinely diagonal; a straight
    // horizontal or vertical edge should stay crisp.
    if similar(side_a, toward) || similar(side_b, toward) {
        return blend(&[(e, 3), (toward, 1)]);
    }
    e
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Builds an RGBA8 buffer from a slice of pixels.
    fn buf(pixels: &[Rgba]) -> Vec<u8> {
        pixels.iter().flat_map(|p| p.iter().copied()).collect()
    }

    const RED: Rgba = [255, 0, 0, 255];
    const BLUE: Rgba = [0, 0, 255, 255];
    const CLEAR: Rgba = [0, 0, 0, 0];
    /// Transparent pixel carrying a stale opaque colour, as produced by some
    /// image editors. Must never bleed into neighbouring opaque pixels.
    const CLEAR_WHITE: Rgba = [255, 255, 255, 0];

    #[test]
    fn nearest_replicates_and_sizes_correctly() {
        let src = buf(&[RED, BLUE, BLUE, RED]);
        let out = nearest(&src, 2, 2, 2);
        assert_eq!(out.len(), 4 * 4 * 4);
        assert_eq!(px(&out, 4, 4, 0, 0), RED);
        assert_eq!(px(&out, 4, 4, 1, 1), RED);
        assert_eq!(px(&out, 4, 4, 2, 0), BLUE);
        assert_eq!(px(&out, 4, 4, 3, 3), RED);
    }

    #[test]
    fn nearest_with_factor_one_is_identity() {
        let src = buf(&[RED, BLUE]);
        assert_eq!(nearest(&src, 2, 1, 1), src);
    }

    #[test]
    fn scale2x_output_dimensions() {
        let src = buf(&[RED; 9]);
        assert_eq!(scale2x(&src, 3, 3).len(), 6 * 6 * 4);
    }

    #[test]
    fn scale2x_keeps_flat_regions_unchanged() {
        let src = buf(&[RED; 9]);
        let out = scale2x(&src, 3, 3);
        for y in 0..6 {
            for x in 0..6 {
                assert_eq!(px(&out, 6, 6, x, y), RED);
            }
        }
    }

    #[test]
    fn scale2x_rounds_a_diagonal_step() {
        // A 3x3 with a diagonal boundary: the corner facing the step should be
        // replaced by the neighbouring colour.
        let src = buf(&[
            BLUE, BLUE, RED, //
            BLUE, RED, RED, //
            RED, RED, RED,
        ]);
        let out = scale2x(&src, 3, 3);
        // Centre pixel (1,1) is RED, with BLUE above-left; its top-left
        // sub-pixel becomes BLUE.
        assert_eq!(px(&out, 6, 6, 2, 2), BLUE);
        assert_eq!(px(&out, 6, 6, 3, 3), RED);
    }

    #[test]
    fn scale2x_never_introduces_new_colours() {
        let src = buf(&[
            BLUE, BLUE, RED, //
            BLUE, RED, RED, //
            RED, RED, RED,
        ]);
        let out = scale2x(&src, 3, 3);
        for chunk in out.chunks_exact(4) {
            let p: Rgba = [chunk[0], chunk[1], chunk[2], chunk[3]];
            assert!(p == RED || p == BLUE, "unexpected colour {p:?}");
        }
    }

    #[test]
    fn scale3x_output_dimensions_and_centre_is_source() {
        let src = buf(&[
            BLUE, BLUE, RED, //
            BLUE, RED, RED, //
            RED, RED, RED,
        ]);
        let out = scale3x(&src, 3, 3);
        assert_eq!(out.len(), 9 * 9 * 4);
        // Centre sub-pixel of each 3x3 block is always the source pixel.
        for y in 0..3isize {
            for x in 0..3isize {
                assert_eq!(
                    px(&out, 9, 9, x * 3 + 1, y * 3 + 1),
                    px(&src, 3, 3, x, y),
                    "centre mismatch at ({x},{y})"
                );
            }
        }
    }

    #[test]
    fn hq2x_output_dimensions() {
        let src = buf(&[RED; 9]);
        assert_eq!(hq2x(&src, 3, 3).len(), 6 * 6 * 4);
    }

    #[test]
    fn hq3x_output_dimensions_and_centre_is_source() {
        let src = buf(&[
            BLUE, BLUE, RED, //
            BLUE, RED, RED, //
            RED, RED, RED,
        ]);
        let out = hq3x(&src, 3, 3);
        assert_eq!(out.len(), 9 * 9 * 4);
        for y in 0..3isize {
            for x in 0..3isize {
                assert_eq!(px(&out, 9, 9, x * 3 + 1, y * 3 + 1), px(&src, 3, 3, x, y));
            }
        }
    }

    #[test]
    fn hqx_keeps_flat_regions_unchanged() {
        let src = buf(&[RED; 9]);
        for out in [hq2x(&src, 3, 3), hq3x(&src, 3, 3)] {
            for chunk in out.chunks_exact(4) {
                assert_eq!([chunk[0], chunk[1], chunk[2], chunk[3]], RED);
            }
        }
    }

    #[test]
    fn transparent_neighbours_never_bleed_colour_into_opaque_pixels() {
        // A single opaque red pixel surrounded by transparent-but-white
        // pixels. Naive averaging would wash the red toward white.
        let src = buf(&[
            CLEAR_WHITE,
            CLEAR_WHITE,
            CLEAR_WHITE,
            CLEAR_WHITE,
            RED,
            CLEAR_WHITE,
            CLEAR_WHITE,
            CLEAR_WHITE,
            CLEAR_WHITE,
        ]);
        for out in [
            scale2x(&src, 3, 3),
            scale3x(&src, 3, 3),
            hq2x(&src, 3, 3),
            hq3x(&src, 3, 3),
        ] {
            for chunk in out.chunks_exact(4) {
                let p: Rgba = [chunk[0], chunk[1], chunk[2], chunk[3]];
                if p[3] == 0 {
                    continue;
                }
                assert!(
                    p[0] >= p[1] && p[0] >= p[2],
                    "red pixel contaminated by transparent white: {p:?}"
                );
                assert!(p[1] == 0 && p[2] == 0, "colour bleed detected: {p:?}");
            }
        }
    }

    #[test]
    fn fully_transparent_input_stays_fully_transparent() {
        let src = buf(&[CLEAR; 9]);
        for out in [
            scale2x(&src, 3, 3),
            scale3x(&src, 3, 3),
            hq2x(&src, 3, 3),
            hq3x(&src, 3, 3),
        ] {
            for chunk in out.chunks_exact(4) {
                assert_eq!(chunk[3], 0);
            }
        }
    }

    #[test]
    fn blend_of_transparent_and_opaque_keeps_opaque_colour() {
        let out = blend(&[(RED, 1), (CLEAR_WHITE, 1)]);
        assert_eq!([out[0], out[1], out[2]], [255, 0, 0]);
        // Alpha is the weighted average, so the edge softens without the
        // colour shifting.
        assert!(out[3] > 0 && out[3] < 255);
    }

    #[test]
    fn similar_treats_all_fully_transparent_pixels_as_equal() {
        assert!(similar(CLEAR, CLEAR_WHITE));
        assert!(exact(CLEAR, CLEAR_WHITE));
        assert!(!similar(RED, CLEAR_WHITE));
        assert!(!exact(RED, CLEAR_WHITE));
    }

    #[test]
    fn non_square_images_are_handled() {
        // Character sprites are 64x32, so non-square input must work.
        let src = buf(&[RED, BLUE, RED, BLUE, RED, BLUE, RED, BLUE]);
        assert_eq!(scale2x(&src, 4, 2).len(), 8 * 4 * 4);
        assert_eq!(scale3x(&src, 4, 2).len(), 12 * 6 * 4);
        assert_eq!(hq2x(&src, 4, 2).len(), 8 * 4 * 4);
        assert_eq!(hq3x(&src, 4, 2).len(), 12 * 6 * 4);
    }
}
