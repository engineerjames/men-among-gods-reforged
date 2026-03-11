//! Visual styling primitives for UI widgets (padding, backgrounds, borders).

use sdl2::pixels::Color;

/// Inset spacing applied inside a widget's bounding rectangle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Padding {
    /// Space above content, in pixels.
    pub top: u32,
    /// Space to the right of content, in pixels.
    pub right: u32,
    /// Space below content, in pixels.
    pub bottom: u32,
    /// Space to the left of content, in pixels.
    pub left: u32,
}

impl Padding {
    /// Creates padding with the same value on all four sides.
    ///
    /// # Arguments
    ///
    /// * `value` - Pixels of padding on each edge.
    ///
    /// # Returns
    ///
    /// A uniform `Padding`.
    pub fn uniform(value: u32) -> Self {
        Self {
            top: value,
            right: value,
            bottom: value,
            left: value,
        }
    }

    /// Creates padding with separate horizontal and vertical values.
    ///
    /// # Arguments
    ///
    /// * `horizontal` - Pixels of padding on left and right.
    /// * `vertical` - Pixels of padding on top and bottom.
    ///
    /// # Returns
    ///
    /// A symmetric `Padding`.
    pub fn symmetric(horizontal: u32, vertical: u32) -> Self {
        Self {
            top: vertical,
            right: horizontal,
            bottom: vertical,
            left: horizontal,
        }
    }

    /// Zero padding on all sides.
    pub const ZERO: Padding = Padding {
        top: 0,
        right: 0,
        bottom: 0,
        left: 0,
    };
}

/// How the background of a widget is filled.
#[derive(Clone, Copy, Debug)]
pub enum Background {
    /// No background is drawn (fully transparent).
    None,
    /// A solid color fill. Use `Color::RGBA` for semi-transparency.
    SolidColor(Color),
}

/// A rectangular border drawn around a widget.
#[derive(Clone, Copy, Debug)]
pub struct Border {
    /// Border color.
    pub color: Color,
    /// Border thickness in pixels.
    pub width: u32,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uniform_padding() {
        let p = Padding::uniform(8);
        assert_eq!(p.top, 8);
        assert_eq!(p.right, 8);
        assert_eq!(p.bottom, 8);
        assert_eq!(p.left, 8);
    }

    #[test]
    fn symmetric_padding() {
        let p = Padding::symmetric(12, 6);
        assert_eq!(p.left, 12);
        assert_eq!(p.right, 12);
        assert_eq!(p.top, 6);
        assert_eq!(p.bottom, 6);
    }

    #[test]
    fn zero_padding() {
        let p = Padding::ZERO;
        assert_eq!(p.top, 0);
        assert_eq!(p.right, 0);
        assert_eq!(p.bottom, 0);
        assert_eq!(p.left, 0);
    }
}
