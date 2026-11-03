//! Supersampled off-screen frame.
//!
//! The whole frame is drawn at an integer multiple of the logical resolution
//! and then copied to the window exactly once. Scaling every sprite
//! individually (the `set_logical_size` approach) rounds neighbouring tiles to
//! different device pixels at non-integer window scales, which shows up as
//! grid lines between floor tiles; a single blit of a seamless image cannot do
//! that. Drawing at device resolution also lets the world move in device
//! pixels instead of logical pixels, so walking looks smooth on high-DPI and
//! high-refresh displays.

use sdl2::pixels::Color;
use sdl2::rect::Rect;
use sdl2::render::{BlendMode, Canvas, RendererInfo, ScaleMode, Texture, TextureCreator};
use sdl2::video::{Window, WindowContext};

/// Largest supported supersample factor; must divide
/// [`crate::types::map::SUBPIXEL_UNIT`].
pub const MAX_RENDER_SCALE: u32 = 8;

/// Off-screen frame texture together with the integer scale it was built for.
pub struct FrameTarget<'tc> {
    texture: Texture<'tc>,
    scale: u32,
}

/// Picks the integer supersample factor for a viewport `view_w` device pixels
/// wide: the smallest `k` whose `k * logical_w` covers the viewport, so the
/// final blit only ever shrinks (or is 1:1).
///
/// # Arguments
///
/// * `view_w` - Width of the letterboxed viewport in device pixels.
/// * `logical_w` - Logical frame width.
/// * `max_scale` - Upper bound, typically from the renderer's texture limits.
///
/// # Returns
///
/// * A scale in `1..=max_scale`.
pub fn render_scale(view_w: f32, logical_w: f32, max_scale: u32) -> u32 {
    if logical_w <= 0.0 || view_w <= 0.0 {
        return 1;
    }
    // Tolerate float noise so an exact integer ratio is not bumped up.
    let k = (view_w / logical_w - 1e-3).ceil().max(1.0);
    (k as u32).clamp(1, max_scale.max(1))
}

/// Largest scale whose target texture fits the renderer's texture limits.
///
/// # Arguments
///
/// * `info` - Renderer capabilities.
/// * `logical` - Logical frame size.
///
/// # Returns
///
/// * A scale in `1..=MAX_RENDER_SCALE`.
pub fn max_render_scale(info: &RendererInfo, logical: (u32, u32)) -> u32 {
    let by_w = if logical.0 > 0 {
        info.max_texture_width / logical.0
    } else {
        1
    };
    let by_h = if logical.1 > 0 {
        info.max_texture_height / logical.1
    } else {
        1
    };
    by_w.min(by_h).clamp(1, MAX_RENDER_SCALE)
}

/// Returns `true` when the blit from the supersampled frame to the viewport is
/// exactly 1:1 and can use nearest sampling without resampling artefacts.
///
/// # Arguments
///
/// * `scale` - Supersample factor of the frame.
/// * `view_w` - Viewport width in device pixels.
/// * `logical_w` - Logical frame width.
///
/// # Returns
///
/// * `true` when `scale * logical_w == view_w` (within half a pixel).
pub fn blit_is_exact(scale: u32, view_w: f32, logical_w: f32) -> bool {
    (scale as f32 * logical_w - view_w).abs() < 0.5
}

impl<'tc> FrameTarget<'tc> {
    /// Creates a frame target of `scale * logical` device pixels.
    ///
    /// # Arguments
    ///
    /// * `creator` - Texture creator of the window renderer.
    /// * `logical` - Logical frame size.
    /// * `scale` - Integer supersample factor.
    ///
    /// # Returns
    ///
    /// * The target, or the SDL error when render targets are unavailable.
    pub fn new(
        creator: &'tc TextureCreator<WindowContext>,
        logical: (u32, u32),
        scale: u32,
    ) -> Result<Self, String> {
        let scale = scale.max(1);
        let mut texture = creator
            .create_texture_target(None, logical.0 * scale, logical.1 * scale)
            .map_err(|e| e.to_string())?;
        // The frame is opaque; ignore whatever alpha the scenes left behind.
        texture.set_blend_mode(BlendMode::None);
        Ok(Self { texture, scale })
    }

    /// Integer supersample factor this target was built for.
    ///
    /// # Returns
    ///
    /// * The scale passed to [`FrameTarget::new`].
    pub fn scale(&self) -> u32 {
        self.scale
    }

    /// Renders one frame: `draw` runs against the target with the canvas scale
    /// set so logical coordinates map to `scale` device pixels, then the
    /// target is copied into `viewport` on the window.
    ///
    /// # Arguments
    ///
    /// * `canvas` - Window canvas.
    /// * `viewport` - Letterboxed destination rect in window device pixels.
    /// * `exact` - Whether the copy is 1:1 (nearest) or a resample (linear).
    /// * `draw` - Scene drawing callback.
    ///
    /// # Returns
    ///
    /// * The first SDL error encountered, if any.
    pub fn render_frame<F>(
        &mut self,
        canvas: &mut Canvas<Window>,
        viewport: Rect,
        exact: bool,
        draw: F,
    ) -> Result<(), String>
    where
        F: FnOnce(&mut Canvas<Window>) -> Result<(), String>,
    {
        let scale = self.scale as f32;
        let mut draw_result = Ok(());
        canvas
            .with_texture_canvas(&mut self.texture, |target| {
                let _ = target.set_scale(scale, scale);
                target.set_draw_color(Color::RGB(0, 0, 0));
                target.clear();
                draw_result = draw(target);
            })
            .map_err(|e| e.to_string())?;
        draw_result?;

        self.texture.set_scale_mode(if exact {
            ScaleMode::Nearest
        } else {
            ScaleMode::Linear
        });
        canvas.set_draw_color(Color::RGB(0, 0, 0));
        canvas.clear();
        canvas.copy(&self.texture, None, Some(viewport))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::map::SUBPIXEL_UNIT;

    #[test]
    fn render_scale_rounds_up_to_cover_viewport() {
        assert_eq!(render_scale(960.0, 960.0, 8), 1);
        assert_eq!(render_scale(1920.0, 960.0, 8), 2);
        assert_eq!(render_scale(2560.0, 960.0, 8), 3);
        assert_eq!(render_scale(3840.0, 960.0, 8), 4);
        assert_eq!(render_scale(1000.0, 960.0, 8), 2);
        assert_eq!(render_scale(500.0, 960.0, 8), 1);
    }

    #[test]
    fn render_scale_respects_cap_and_degenerate_input() {
        assert_eq!(render_scale(7680.0, 960.0, 4), 4);
        assert_eq!(render_scale(0.0, 960.0, 8), 1);
        assert_eq!(render_scale(1920.0, 0.0, 8), 1);
    }

    #[test]
    fn max_render_scale_divides_subpixel_unit() {
        for k in 1..=MAX_RENDER_SCALE {
            assert_eq!(SUBPIXEL_UNIT % k as i32, 0, "scale {k}");
        }
    }

    #[test]
    fn blit_exactness() {
        assert!(blit_is_exact(2, 1920.0, 960.0));
        assert!(blit_is_exact(4, 3840.0, 960.0));
        assert!(!blit_is_exact(3, 2560.0, 960.0));
    }
}
