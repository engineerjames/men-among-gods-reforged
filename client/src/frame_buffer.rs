//! Off-screen frame composition for the enhanced graphics pipeline.
//!
//! Historically the client rendered straight into the window backbuffer using
//! SDL's logical-size scaling, which meant every sprite was scaled
//! independently by the renderer. That prevents any kind of whole-screen
//! filtering: bilinear filtering applied per sprite samples past each sprite's
//! edges and produces visible seams between adjacent floor tiles.
//!
//! Instead, the whole scene is composed into a single off-screen texture at
//! `factor * 960 x factor * 540` and that one texture is then blitted onto the
//! window. Scaling and filtering therefore happen exactly once, on a single
//! contiguous image, which is what makes smooth output filtering and
//! supersampled anti-aliasing possible.

use sdl2::pixels::PixelFormatEnum;
use sdl2::rect::Rect;
use sdl2::render::{Canvas, ScaleMode, Texture, TextureCreator};
use sdl2::video::{Window, WindowContext};

use crate::preferences::OutputFilter;

/// Pixel format used for all frame buffer targets.
///
/// `ARGB8888` is the format SDL's accelerated backends use natively for render
/// targets, so this avoids a conversion on present.
const FRAME_BUFFER_FORMAT: PixelFormatEnum = PixelFormatEnum::ARGB8888;

/// An off-screen render target that the scene is composed into.
///
/// The buffer is sized `factor` times the logical frame. Scene code continues
/// to draw in logical coordinates; the extra resolution is obtained by applying
/// a renderer scale of `factor` while the target is bound (see
/// [`FrameBuffer::compose`]).
pub struct FrameBuffer<'tc> {
    /// Texture creator used to allocate targets, kept so the optional
    /// sharp-bilinear intermediate can be created lazily with the same lifetime.
    creator: &'tc TextureCreator<WindowContext>,
    /// The render-target texture holding the composed frame.
    texture: Texture<'tc>,
    /// Intermediate target used by [`OutputFilter::SharpBilinear`].
    prescale: Option<Texture<'tc>>,
    /// Integer prescale factor currently baked into `prescale`.
    prescale_factor: u32,
    /// Set once a prescale allocation has failed, to avoid retrying every frame.
    prescale_unavailable: bool,
    /// Internal resolution multiplier.
    factor: u32,
    /// Logical frame width in logical pixels.
    logical_width: u32,
    /// Logical frame height in logical pixels.
    logical_height: u32,
    /// Filter currently applied when presenting.
    filter: OutputFilter,
}

impl<'tc> FrameBuffer<'tc> {
    /// Creates a frame buffer for the given logical frame size and multiplier.
    ///
    /// # Arguments
    ///
    /// * `creator` - Texture creator bound to the window renderer.
    /// * `logical_width` - Logical frame width (e.g. 960).
    /// * `logical_height` - Logical frame height (e.g. 540).
    /// * `factor` - Internal resolution multiplier; clamped to at least 1.
    /// * `filter` - Output filter to apply when presenting.
    ///
    /// # Returns
    ///
    /// * `Ok(FrameBuffer)`, or `Err` if the renderer cannot allocate a render
    ///   target of the requested size.
    pub fn new(
        creator: &'tc TextureCreator<WindowContext>,
        logical_width: u32,
        logical_height: u32,
        factor: u32,
        filter: OutputFilter,
    ) -> Result<Self, String> {
        let factor = factor.max(1);
        let logical_width = logical_width.max(1);
        let logical_height = logical_height.max(1);
        let width = logical_width * factor;
        let height = logical_height * factor;

        let texture = creator
            .create_texture_target(FRAME_BUFFER_FORMAT, width, height)
            .map_err(|e| {
                format!("Failed to create {width}x{height} frame buffer render target: {e}")
            })?;

        let mut buffer = Self {
            creator,
            texture,
            prescale: None,
            prescale_factor: 0,
            prescale_unavailable: false,
            factor,
            logical_width,
            logical_height,
            filter,
        };
        buffer.apply_scale_mode();
        Ok(buffer)
    }

    /// Returns the internal resolution multiplier.
    ///
    /// # Returns
    ///
    /// * The multiplier, always at least 1.
    pub fn factor(&self) -> u32 {
        self.factor
    }

    /// Returns the logical frame size this buffer was built for.
    ///
    /// # Returns
    ///
    /// * `(logical_width, logical_height)`.
    pub fn logical_size(&self) -> (u32, u32) {
        (self.logical_width, self.logical_height)
    }

    /// Applies an output filter to the composed frame.
    ///
    /// Only the single composed texture is affected, so this can never
    /// introduce seams between individual sprites.
    ///
    /// # Arguments
    ///
    /// * `filter` - The filter to apply.
    pub fn set_filter(&mut self, filter: OutputFilter) {
        if self.filter == filter {
            return;
        }
        self.filter = filter;
        if filter != OutputFilter::SharpBilinear {
            self.prescale = None;
            self.prescale_factor = 0;
        }
        self.apply_scale_mode();
    }

    /// Syncs the composed texture's scale mode with the active filter.
    ///
    /// Under `SharpBilinear` the composed frame is point-sampled into an
    /// integer prescale target, so it must stay `Nearest`; the bilinear step
    /// happens on the prescale target instead.
    fn apply_scale_mode(&mut self) {
        let mode = match self.filter {
            OutputFilter::Nearest | OutputFilter::SharpBilinear => ScaleMode::Nearest,
            OutputFilter::Linear => ScaleMode::Linear,
        };
        self.texture.set_scale_mode(mode);
    }

    /// Composes a frame by running `draw` with this buffer bound as the render
    /// target.
    ///
    /// The renderer scale is set to the buffer's multiplier for the duration of
    /// the closure, so `draw` can keep working purely in logical coordinates.
    ///
    /// # Arguments
    ///
    /// * `canvas` - The window canvas whose render target is temporarily
    ///   redirected.
    /// * `draw` - Scene drawing callback, invoked with the redirected canvas.
    ///
    /// # Returns
    ///
    /// * `Ok(())` on success, or `Err` if the render target could not be bound.
    pub fn compose<F>(&mut self, canvas: &mut Canvas<Window>, draw: F) -> Result<(), String>
    where
        F: FnOnce(&mut Canvas<Window>),
    {
        let factor = self.factor as f32;
        canvas
            .with_texture_canvas(&mut self.texture, |target| {
                // Scene code draws in logical units; the renderer widens them
                // to the buffer's internal resolution.
                let _ = target.set_scale(factor, factor);
                draw(target);
                let _ = target.set_scale(1.0, 1.0);
            })
            .map_err(|e| format!("Failed to bind frame buffer render target: {e}"))
    }

    /// Blits the composed frame onto the window backbuffer.
    ///
    /// The caller is responsible for clearing the backbuffer first (the frame
    /// is letterboxed, so the margins would otherwise retain stale pixels) and
    /// for calling `present` afterwards.
    ///
    /// # Arguments
    ///
    /// * `canvas` - The window canvas to draw onto.
    /// * `dst` - Destination rectangle in physical pixels.
    ///
    /// # Returns
    ///
    /// * `Ok(())` on success, or `Err` with the SDL error message.
    pub fn present(&mut self, canvas: &mut Canvas<Window>, dst: Rect) -> Result<(), String> {
        if self.filter == OutputFilter::SharpBilinear {
            return self.present_sharp_bilinear(canvas, dst);
        }
        canvas.copy(&self.texture, None, Some(dst))
    }

    /// Presents using an integer prescale followed by a bilinear stretch.
    ///
    /// Pure bilinear scaling of pixel art softens every edge; pure nearest
    /// scaling at a non-integer factor makes some pixels wider than others.
    /// Prescaling by the largest integer factor that still fits and letting
    /// bilinear cover only the fractional remainder keeps pixels square while
    /// removing the uneven stair-stepping.
    ///
    /// Falls back to a direct bilinear blit if the intermediate target cannot
    /// be allocated.
    ///
    /// # Arguments
    ///
    /// * `canvas` - The window canvas to draw onto.
    /// * `dst` - Destination rectangle in physical pixels.
    ///
    /// # Returns
    ///
    /// * `Ok(())` on success, or `Err` with the SDL error message.
    fn present_sharp_bilinear(
        &mut self,
        canvas: &mut Canvas<Window>,
        dst: Rect,
    ) -> Result<(), String> {
        let src_w = self.logical_width * self.factor;
        let src_h = self.logical_height * self.factor;
        let wanted = Self::prescale_factor_for(src_w, src_h, dst.width(), dst.height());

        if wanted <= 1 || self.prescale_unavailable {
            // Either the composed frame is already at/above the destination
            // size, or no intermediate is available: a plain bilinear
            // (down)scale is the best remaining option.
            return self.present_linear_fallback(canvas, dst);
        }

        if self.prescale_factor != wanted {
            match self.creator.create_texture_target(
                FRAME_BUFFER_FORMAT,
                src_w * wanted,
                src_h * wanted,
            ) {
                Ok(mut texture) => {
                    texture.set_scale_mode(ScaleMode::Linear);
                    self.prescale = Some(texture);
                    self.prescale_factor = wanted;
                }
                Err(e) => {
                    log::warn!(
                        "Sharp bilinear prescale target unavailable, falling back to linear: {e}"
                    );
                    self.prescale = None;
                    self.prescale_factor = 0;
                    self.prescale_unavailable = true;
                    return self.present_linear_fallback(canvas, dst);
                }
            }
        }

        // Split the borrow so the composed frame can be read while the
        // intermediate is bound as the render target.
        let Self {
            texture, prescale, ..
        } = self;
        let Some(prescale) = prescale.as_mut() else {
            return canvas.copy(texture, None, Some(dst));
        };

        canvas
            .with_texture_canvas(prescale, |target| {
                let _ = target.copy(texture, None, None);
            })
            .map_err(|e| format!("Failed to bind sharp bilinear prescale target: {e}"))?;

        canvas.copy(prescale, None, Some(dst))
    }

    /// Blits the composed frame with bilinear filtering, restoring the nearest
    /// scale mode afterwards.
    ///
    /// # Arguments
    ///
    /// * `canvas` - The window canvas to draw onto.
    /// * `dst` - Destination rectangle in physical pixels.
    ///
    /// # Returns
    ///
    /// * `Ok(())` on success, or `Err` with the SDL error message.
    fn present_linear_fallback(
        &mut self,
        canvas: &mut Canvas<Window>,
        dst: Rect,
    ) -> Result<(), String> {
        self.texture.set_scale_mode(ScaleMode::Linear);
        let result = canvas.copy(&self.texture, None, Some(dst));
        self.texture.set_scale_mode(ScaleMode::Nearest);
        result
    }

    /// Chooses the integer prescale factor for sharp-bilinear presentation.
    ///
    /// # Arguments
    ///
    /// * `src_w` - Composed frame width in pixels.
    /// * `src_h` - Composed frame height in pixels.
    /// * `dst_w` - Destination width in pixels.
    /// * `dst_h` - Destination height in pixels.
    ///
    /// # Returns
    ///
    /// * The largest integer factor that does not overshoot the destination,
    ///   at least 1.
    fn prescale_factor_for(src_w: u32, src_h: u32, dst_w: u32, dst_h: u32) -> u32 {
        if src_w == 0 || src_h == 0 {
            return 1;
        }
        let by_w = dst_w / src_w;
        let by_h = dst_h / src_h;
        by_w.min(by_h).max(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prescale_factor_is_largest_non_overshooting_integer() {
        assert_eq!(FrameBuffer::prescale_factor_for(960, 540, 2560, 1440), 2);
        assert_eq!(FrameBuffer::prescale_factor_for(960, 540, 1920, 1080), 2);
        assert_eq!(FrameBuffer::prescale_factor_for(960, 540, 1900, 1070), 1);
        assert_eq!(FrameBuffer::prescale_factor_for(960, 540, 3840, 2160), 4);
    }

    #[test]
    fn prescale_factor_never_returns_zero() {
        assert_eq!(FrameBuffer::prescale_factor_for(960, 540, 640, 360), 1);
        assert_eq!(FrameBuffer::prescale_factor_for(0, 0, 1920, 1080), 1);
    }
}
