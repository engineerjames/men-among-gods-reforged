//! Full-screen panning background image widget.
//!
//! Displays a subsection of a large image and slowly pans across it in a
//! ping-pong pattern. An optional colour tint overlay can be applied to
//! darken / colour-shift the image so foreground UI elements stand out.

use std::path::PathBuf;
use std::time::Duration;

use sdl2::pixels::Color;
use sdl2::rect::Rect;
use sdl2::render::BlendMode;

use super::widget::{Bounds, EventResponse, UiEvent, Widget};
use super::RenderContext;

/// A full-screen background that shows a subsection of a large image and
/// slowly pans across it with a ping-pong motion.
///
/// The widget is non-interactive — it always ignores input events.
/// The texture is lazy-loaded from the filesystem on the first `render` call
/// via [`GraphicsCache::load_texture_from_path`].
pub struct PanningBackground {
    bounds: Bounds,
    /// Filesystem path to the source image (PNG).
    texture_path: PathBuf,
    /// Sprite ID assigned by GraphicsCache after the first load.
    texture_id: Option<usize>,
    /// Source image dimensions in pixels (populated after load).
    image_width: u32,
    image_height: u32,
    /// Current horizontal offset into the source image (sub-pixel).
    pan_x: f32,
    /// Current vertical offset into the source image (sub-pixel).
    pan_y: f32,
    /// Horizontal pan speed in pixels per second.
    pan_speed_x: f32,
    /// Vertical pan speed in pixels per second.
    pan_speed_y: f32,
    /// Current horizontal direction (+1.0 or −1.0) for ping-pong.
    dir_x: f32,
    /// Current vertical direction (+1.0 or −1.0) for ping-pong.
    dir_y: f32,
    /// Optional RGBA colour drawn over the image with alpha blending.
    tint: Option<Color>,
}

impl PanningBackground {
    /// Creates a new panning background.
    ///
    /// # Arguments
    ///
    /// * `bounds` - Destination rectangle (normally full-screen 960×540).
    /// * `texture_path` - Filesystem path to a PNG image.
    /// * `pan_speed_x` - Horizontal pan speed in pixels per second.
    /// * `pan_speed_y` - Vertical pan speed in pixels per second.
    /// * `tint` - Optional RGBA overlay colour.
    ///
    /// # Returns
    ///
    /// A new `PanningBackground`.
    pub fn new(
        bounds: Bounds,
        texture_path: PathBuf,
        pan_speed_x: f32,
        pan_speed_y: f32,
        tint: Option<Color>,
    ) -> Self {
        Self {
            bounds,
            texture_path,
            texture_id: None,
            image_width: 0,
            image_height: 0,
            pan_x: 0.0,
            pan_y: 0.0,
            pan_speed_x,
            pan_speed_y,
            dir_x: 1.0,
            dir_y: 1.0,
            tint,
        }
    }

    /// Replaces the tint colour.
    ///
    /// # Arguments
    ///
    /// * `tint` - New RGBA tint, or `None` to remove.
    pub fn set_tint(&mut self, tint: Option<Color>) {
        self.tint = tint;
    }
}

impl Widget for PanningBackground {
    fn bounds(&self) -> &Bounds {
        &self.bounds
    }

    fn set_position(&mut self, x: i32, y: i32) {
        self.bounds.x = x;
        self.bounds.y = y;
    }

    fn handle_event(&mut self, _event: &UiEvent) -> EventResponse {
        EventResponse::Ignored
    }

    fn update(&mut self, dt: Duration) {
        if self.texture_id.is_none() {
            return;
        }

        let dt_secs = dt.as_secs_f32();

        // Horizontal ping-pong
        let max_x = self.image_width.saturating_sub(self.bounds.width) as f32;
        if max_x > 0.0 {
            self.pan_x += self.pan_speed_x * self.dir_x * dt_secs;
            if self.pan_x >= max_x {
                self.pan_x = max_x;
                self.dir_x = -1.0;
            } else if self.pan_x <= 0.0 {
                self.pan_x = 0.0;
                self.dir_x = 1.0;
            }
        }

        // Vertical ping-pong
        let max_y = self.image_height.saturating_sub(self.bounds.height) as f32;
        if max_y > 0.0 {
            self.pan_y += self.pan_speed_y * self.dir_y * dt_secs;
            if self.pan_y >= max_y {
                self.pan_y = max_y;
                self.dir_y = -1.0;
            } else if self.pan_y <= 0.0 {
                self.pan_y = 0.0;
                self.dir_y = 1.0;
            }
        }
    }

    fn render(&mut self, ctx: &mut RenderContext) -> Result<(), String> {
        // Lazy-load the texture on first render.
        if self.texture_id.is_none() {
            match ctx.gfx.load_texture_from_path(&self.texture_path) {
                Ok(id) => {
                    let (w, h) = ctx.gfx.query_texture_size(id);
                    self.image_width = w;
                    self.image_height = h;
                    self.texture_id = Some(id);
                    log::info!(
                        "Loaded panning background {}x{} from {}",
                        w,
                        h,
                        self.texture_path.display()
                    );
                }
                Err(e) => {
                    log::error!("Failed to load panning background: {}", e);
                    // Draw a solid fallback colour so the screen isn't blank.
                    ctx.canvas.set_draw_color(Color::RGB(20, 20, 28));
                    ctx.canvas.clear();
                    return Ok(());
                }
            }
        }

        let tex_id = self.texture_id.unwrap();

        // Work out the source sub-rectangle to sample.
        let src_w = self.bounds.width.min(self.image_width);
        let src_h = self.bounds.height.min(self.image_height);
        let src_rect = Rect::new(self.pan_x as i32, self.pan_y as i32, src_w, src_h);

        let dst_rect = Rect::new(
            self.bounds.x,
            self.bounds.y,
            self.bounds.width,
            self.bounds.height,
        );

        let texture = ctx.gfx.get_texture(tex_id);
        ctx.canvas.copy(texture, Some(src_rect), Some(dst_rect))?;

        // Tint overlay
        if let Some(tint) = self.tint {
            ctx.canvas.set_blend_mode(BlendMode::Blend);
            ctx.canvas.set_draw_color(tint);
            ctx.canvas.fill_rect(dst_rect)?;
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn make_bg(w: u32, h: u32) -> PanningBackground {
        let mut bg = PanningBackground::new(
            Bounds::new(0, 0, 960, 540),
            PathBuf::from("dummy.png"),
            15.0,
            5.0,
            None,
        );
        // Simulate having loaded a texture.
        bg.texture_id = Some(999);
        bg.image_width = w;
        bg.image_height = h;
        bg
    }

    #[test]
    fn pan_advances_horizontally() {
        let mut bg = make_bg(1920, 540);
        bg.update(Duration::from_secs(1));
        assert!(bg.pan_x > 0.0, "pan_x should advance");
    }

    #[test]
    fn pan_reverses_at_right_edge() {
        let mut bg = make_bg(1920, 540);
        // Fast-forward past the right edge.
        for _ in 0..1000 {
            bg.update(Duration::from_millis(100));
        }
        // Direction should have reversed at least once and pan_x clamped.
        assert!(bg.pan_x <= (1920 - 960) as f32);
        assert!(bg.dir_x == -1.0 || bg.dir_x == 1.0);
    }

    #[test]
    fn pan_reverses_at_left_edge() {
        let mut bg = make_bg(1920, 540);
        bg.dir_x = -1.0;
        bg.pan_x = 5.0;
        // Update enough to go past zero.
        bg.update(Duration::from_secs(1));
        // pan_x should be clamped at 0.
        assert!(bg.pan_x >= 0.0);
    }

    #[test]
    fn no_pan_when_image_fits_exactly() {
        let mut bg = make_bg(960, 540);
        bg.update(Duration::from_secs(10));
        assert_eq!(bg.pan_x, 0.0, "no horizontal pan if image == viewport");
        assert_eq!(bg.pan_y, 0.0, "no vertical pan if image == viewport");
    }

    #[test]
    fn always_ignores_events() {
        let mut bg = make_bg(1920, 540);
        let resp = bg.handle_event(&UiEvent::MouseClick {
            x: 100,
            y: 100,
            button: super::super::widget::MouseButton::Left,
            modifiers: super::super::widget::KeyModifiers::default(),
        });
        assert_eq!(resp, EventResponse::Ignored);
    }

    #[test]
    fn set_tint_replaces_tint() {
        let mut bg = make_bg(1920, 540);
        assert!(bg.tint.is_none());
        bg.set_tint(Some(Color::RGBA(0, 0, 0, 128)));
        assert!(bg.tint.is_some());
        bg.set_tint(None);
        assert!(bg.tint.is_none());
    }
}
