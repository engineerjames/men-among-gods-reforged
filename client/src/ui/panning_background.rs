//! Full-screen panning background image widget.
//!
//! Displays a subsection of a large image and slowly pans across it in a
//! ping-pong pattern.  Every 30 seconds the widget crossfades to a randomly
//! chosen image from the supplied list (excluding the one currently shown).
//! An optional colour tint overlay can be applied to darken / colour-shift
//! the image so foreground UI elements stand out.

use std::path::PathBuf;
use std::time::Duration;

use rand::Rng as _;
use sdl2::pixels::Color;
use sdl2::rect::{FRect, Rect};
use sdl2::render::BlendMode;

use super::widget::{Bounds, EventResponse, UiEvent, Widget};
use super::RenderContext;

/// How long each image is displayed before a crossfade begins (seconds).
const DISPLAY_DURATION: f32 = 30.0;

/// Duration of the crossfade transition between images (seconds).
const TRANSITION_DURATION: f32 = 2.0;

// ---------------------------------------------------------------------------
// Per-image state
// ---------------------------------------------------------------------------

/// Pan state and texture metadata for a single background image.
struct ImageState {
    /// Filesystem path to the source image (PNG).
    path: PathBuf,
    /// Sprite ID assigned by `GraphicsCache` after the first load.
    texture_id: Option<usize>,
    /// Source image width in pixels (populated after load).
    width: u32,
    /// Source image height in pixels (populated after load).
    height: u32,
    /// Current horizontal pan offset in pixels (sub-pixel precision).
    pan_x: f32,
    /// Current vertical pan offset in pixels (sub-pixel precision).
    pan_y: f32,
    /// Current horizontal pan direction: +1.0 or −1.0.
    dir_x: f32,
    /// Current vertical pan direction: +1.0 or −1.0.
    dir_y: f32,
}

impl ImageState {
    /// Creates a new `ImageState` for the given filesystem path.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to a PNG (or SDL2_image-supported) image file.
    ///
    /// # Returns
    ///
    /// A new `ImageState` with pan at (0, 0) and both directions set to +1.
    fn new(path: PathBuf) -> Self {
        Self {
            path,
            texture_id: None,
            width: 0,
            height: 0,
            pan_x: 0.0,
            pan_y: 0.0,
            dir_x: 1.0,
            dir_y: 1.0,
        }
    }

    /// Resets the pan position to the top-left corner and both directions to +1.
    fn reset_pan(&mut self) {
        self.pan_x = 0.0;
        self.pan_y = 0.0;
        self.dir_x = 1.0;
        self.dir_y = 1.0;
    }
}

// ---------------------------------------------------------------------------
// Widget
// ---------------------------------------------------------------------------

/// A full-screen background that shows a subsection of a large image, pans
/// across it with ping-pong motion, and every [`DISPLAY_DURATION`] seconds
/// crossfades to a randomly chosen image from the supplied list.
///
/// The widget is non-interactive — it always ignores input events.
/// Textures are lazy-loaded from the filesystem on the first `render` call
/// via [`GraphicsCache::load_texture_from_path`].
pub struct PanningBackground {
    bounds: Bounds,
    /// All candidate background images (at least one required).
    images: Vec<ImageState>,
    /// Index into `images` for the currently visible image.
    current_idx: usize,
    /// Index into `images` for the incoming image while crossfading.
    /// `None` when no transition is in progress.
    next_idx: Option<usize>,
    /// Seconds the current image has been fully visible (resets after each
    /// completed transition).
    display_elapsed: f32,
    /// Fractional progress of the active crossfade (`0.0` → `1.0`).
    /// Zero when no transition is in progress.
    transition_progress: f32,
    /// Horizontal pan speed in pixels per second (shared by all images).
    pan_speed_x: f32,
    /// Vertical pan speed in pixels per second (shared by all images).
    pan_speed_y: f32,
    /// Optional RGBA colour drawn over the image with alpha blending.
    tint: Option<Color>,
}

impl PanningBackground {
    /// Creates a new panning background that cycles through the supplied images.
    ///
    /// The first image in `image_paths` is shown immediately; subsequent images
    /// are selected at random every [`DISPLAY_DURATION`] seconds.
    ///
    /// # Arguments
    ///
    /// * `bounds` - Destination rectangle (normally full-screen 960×540).
    /// * `image_paths` - One or more filesystem paths to PNG images.  At least
    ///   one path is required; the function panics if the slice is empty.
    /// * `pan_speed_x` - Horizontal pan speed in pixels per second.
    /// * `pan_speed_y` - Vertical pan speed in pixels per second.
    /// * `tint` - Optional RGBA overlay colour.
    ///
    /// # Returns
    ///
    /// A new `PanningBackground`.
    ///
    /// # Panics
    ///
    /// * Panics if `image_paths` is empty.
    pub fn new(
        bounds: Bounds,
        image_paths: Vec<PathBuf>,
        pan_speed_x: f32,
        pan_speed_y: f32,
        tint: Option<Color>,
    ) -> Self {
        assert!(
            !image_paths.is_empty(),
            "PanningBackground requires at least one image path"
        );
        let images = image_paths.into_iter().map(ImageState::new).collect();
        Self {
            bounds,
            images,
            current_idx: 0,
            next_idx: None,
            display_elapsed: 0.0,
            transition_progress: 0.0,
            pan_speed_x,
            pan_speed_y,
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

    /// Advances the ping-pong pan state for the image at `idx`.
    ///
    /// Does nothing if the image has not yet been loaded (i.e. its dimensions
    /// are still zero).
    ///
    /// # Arguments
    ///
    /// * `idx` - Index into `self.images` to update.
    /// * `dt_secs` - Elapsed time in seconds since the last update.
    fn advance_pan(&mut self, idx: usize, dt_secs: f32) {
        let img = &mut self.images[idx];
        if img.width == 0 {
            return;
        }

        // Horizontal ping-pong
        let max_x = img.width.saturating_sub(self.bounds.width) as f32;
        if max_x > 0.0 {
            img.pan_x += self.pan_speed_x * img.dir_x * dt_secs;
            if img.pan_x >= max_x {
                img.pan_x = max_x;
                img.dir_x = -1.0;
            } else if img.pan_x <= 0.0 {
                img.pan_x = 0.0;
                img.dir_x = 1.0;
            }
        }

        // Vertical ping-pong
        let max_y = img.height.saturating_sub(self.bounds.height) as f32;
        if max_y > 0.0 {
            img.pan_y += self.pan_speed_y * img.dir_y * dt_secs;
            if img.pan_y >= max_y {
                img.pan_y = max_y;
                img.dir_y = -1.0;
            } else if img.pan_y <= 0.0 {
                img.pan_y = 0.0;
                img.dir_y = 1.0;
            }
        }
    }

    /// Picks a random index from `self.images` that is different from `current_idx`.
    ///
    /// Returns `current_idx` unchanged when `self.images` has fewer than 2 entries
    /// (caller must check before starting a transition).
    ///
    /// # Returns
    ///
    /// * A random image index not equal to `self.current_idx`.
    fn pick_next_idx(&self) -> usize {
        let len = self.images.len();
        if len < 2 {
            return self.current_idx;
        }
        // Generate an offset in [1, len-1] to guarantee we skip current_idx.
        let offset = rand::thread_rng().gen_range(1..len);
        (self.current_idx + offset) % len
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
        let dt_secs = dt.as_secs_f32();

        // Always advance the pan for the visible image.
        self.advance_pan(self.current_idx, dt_secs);

        if let Some(next) = self.next_idx {
            // Transition in progress — advance the incoming image's pan too.
            self.advance_pan(next, dt_secs);

            self.transition_progress += dt_secs / TRANSITION_DURATION;
            if self.transition_progress >= 1.0 {
                // Transition complete: swap images.
                self.current_idx = next;
                self.next_idx = None;
                self.transition_progress = 0.0;
                self.display_elapsed = 0.0;
            }
        } else {
            // Idle: count how long the current image has been shown.
            // Only start counting once the texture has been loaded.
            if self.images[self.current_idx].texture_id.is_some() {
                self.display_elapsed += dt_secs;
            }

            if self.display_elapsed >= DISPLAY_DURATION && self.images.len() > 1 {
                let next = self.pick_next_idx();
                self.images[next].reset_pan();
                self.next_idx = Some(next);
                self.transition_progress = 0.0;
                self.display_elapsed = 0.0;
            }
        }
    }

    fn render(&mut self, ctx: &mut RenderContext<'_, '_>) -> Result<(), String> {
        // --- Lazy-load current image ----------------------------------------
        if self.images[self.current_idx].texture_id.is_none() {
            let path = self.images[self.current_idx].path.clone();
            match ctx.gfx.load_texture_from_path(&path) {
                Ok(id) => {
                    let (w, h) = ctx.gfx.query_texture_size(id);
                    let img = &mut self.images[self.current_idx];
                    img.width = w;
                    img.height = h;
                    img.texture_id = Some(id);
                    log::info!(
                        "Loaded panning background {}×{} from {}",
                        w,
                        h,
                        path.display()
                    );
                }
                Err(e) => {
                    log::error!("Failed to load panning background: {}", e);
                    ctx.canvas.set_draw_color(Color::RGB(20, 20, 28));
                    ctx.canvas.clear();
                    return Ok(());
                }
            }
        }

        // --- Draw current image (full opacity) --------------------------------
        let cur = &self.images[self.current_idx];
        let cur_dst = FRect::new(
            self.bounds.x as f32 - cur.pan_x,
            self.bounds.y as f32 - cur.pan_y,
            cur.width as f32,
            cur.height as f32,
        );
        let cur_id = cur.texture_id.unwrap();
        let cur_tex = ctx.gfx.get_texture(cur_id);
        cur_tex.set_blend_mode(BlendMode::None);
        ctx.canvas.copy_f(cur_tex, None::<Rect>, Some(cur_dst))?;

        // --- Draw incoming image during crossfade ----------------------------
        if let Some(next) = self.next_idx {
            // Lazy-load the incoming image if needed.
            if self.images[next].texture_id.is_none() {
                let path = self.images[next].path.clone();
                if let Ok(id) = ctx.gfx.load_texture_from_path(&path) {
                    let (w, h) = ctx.gfx.query_texture_size(id);
                    let img = &mut self.images[next];
                    img.width = w;
                    img.height = h;
                    img.texture_id = Some(id);
                    log::info!(
                        "Loaded panning background {}×{} from {}",
                        w,
                        h,
                        path.display()
                    );
                }
                // If load failed this frame, skip drawing the incoming image.
            }

            if let Some(next_id) = self.images[next].texture_id {
                let nxt = &self.images[next];
                let nxt_dst = FRect::new(
                    self.bounds.x as f32 - nxt.pan_x,
                    self.bounds.y as f32 - nxt.pan_y,
                    nxt.width as f32,
                    nxt.height as f32,
                );
                let alpha = (self.transition_progress.clamp(0.0, 1.0) * 255.0) as u8;
                let nxt_tex = ctx.gfx.get_texture(next_id);
                nxt_tex.set_blend_mode(BlendMode::Blend);
                nxt_tex.set_alpha_mod(alpha);
                ctx.canvas.copy_f(nxt_tex, None::<Rect>, Some(nxt_dst))?;
                // Reset alpha so cached texture isn't left in a modified state.
                ctx.gfx.get_texture(next_id).set_alpha_mod(255);
            }
        }

        // --- Tint overlay (viewport area only) --------------------------------
        let viewport = Rect::new(
            self.bounds.x,
            self.bounds.y,
            self.bounds.width,
            self.bounds.height,
        );
        if let Some(tint) = self.tint {
            ctx.canvas.set_blend_mode(BlendMode::Blend);
            ctx.canvas.set_draw_color(tint);
            ctx.canvas.fill_rect(viewport)?;
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

    /// Creates a `PanningBackground` with one or more pre-loaded dummy images
    /// of the given pixel dimensions.  The image list has `count` entries and
    /// all are pre-populated with fake texture IDs so `update` can run.
    fn make_bg_many(w: u32, h: u32, count: usize) -> PanningBackground {
        assert!(count >= 1);
        let paths: Vec<PathBuf> = (0..count)
            .map(|i| PathBuf::from(format!("dummy{}.png", i)))
            .collect();
        let mut bg = PanningBackground::new(Bounds::new(0, 0, 960, 540), paths, 15.0, 5.0, None);
        for (i, img) in bg.images.iter_mut().enumerate() {
            img.texture_id = Some(1000 + i);
            img.width = w;
            img.height = h;
        }
        bg
    }

    /// Convenience wrapper for the common single-image case.
    fn make_bg(w: u32, h: u32) -> PanningBackground {
        make_bg_many(w, h, 1)
    }

    #[test]
    fn pan_advances_horizontally() {
        let mut bg = make_bg(1920, 540);
        bg.update(Duration::from_secs(1));
        assert!(
            bg.images[0].pan_x > 0.0,
            "pan_x should advance after 1 second"
        );
    }

    #[test]
    fn pan_reverses_at_right_edge() {
        let mut bg = make_bg(1920, 540);
        for _ in 0..1000 {
            bg.update(Duration::from_millis(100));
        }
        assert!(bg.images[0].pan_x <= (1920 - 960) as f32);
        let dir = bg.images[0].dir_x;
        assert!(dir == -1.0 || dir == 1.0);
    }

    #[test]
    fn pan_reverses_at_left_edge() {
        let mut bg = make_bg(1920, 540);
        bg.images[0].dir_x = -1.0;
        bg.images[0].pan_x = 5.0;
        bg.update(Duration::from_secs(1));
        assert!(bg.images[0].pan_x >= 0.0);
    }

    #[test]
    fn no_pan_when_image_fits_exactly() {
        let mut bg = make_bg(960, 540);
        bg.update(Duration::from_secs(10));
        assert_eq!(
            bg.images[0].pan_x, 0.0,
            "no horizontal pan if image == viewport"
        );
        assert_eq!(
            bg.images[0].pan_y, 0.0,
            "no vertical pan if image == viewport"
        );
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

    #[test]
    fn no_transition_with_single_image() {
        let mut bg = make_bg(1920, 540);
        // Simulate 60 seconds passing — should never start a transition with
        // only one image available.
        for _ in 0..600 {
            bg.update(Duration::from_millis(100));
        }
        assert!(bg.next_idx.is_none(), "no transition with a single image");
        assert_eq!(bg.current_idx, 0);
    }

    #[test]
    fn transition_triggers_after_display_duration() {
        let mut bg = make_bg_many(1920, 540, 3);
        // Advance just past DISPLAY_DURATION.
        let ticks = ((DISPLAY_DURATION / 0.1) as usize) + 1;
        for _ in 0..ticks {
            bg.update(Duration::from_millis(100));
        }
        assert!(
            bg.next_idx.is_some(),
            "a transition should start after DISPLAY_DURATION seconds"
        );
    }

    #[test]
    fn transition_completes_and_swaps_image() {
        let mut bg = make_bg_many(1920, 540, 2);
        // Manually start a transition to image 1.
        bg.next_idx = Some(1);
        bg.transition_progress = 0.95;

        // One more tick should push progress over 1.0 and complete the swap.
        bg.update(Duration::from_millis(100));
        assert!(
            bg.next_idx.is_none(),
            "next_idx should be cleared after transition completes"
        );
        assert_eq!(
            bg.current_idx, 1,
            "current_idx should be the former next_idx"
        );
        assert_eq!(bg.display_elapsed, 0.0, "display_elapsed should reset");
    }

    #[test]
    fn pick_next_never_returns_current() {
        let bg = make_bg_many(1920, 540, 5);
        for _ in 0..100 {
            let next = bg.pick_next_idx();
            assert_ne!(
                next, bg.current_idx,
                "pick_next_idx must not return current_idx"
            );
        }
    }

    #[test]
    fn reset_pan_zeroes_position_and_direction() {
        let mut img = ImageState::new(PathBuf::from("test.png"));
        img.pan_x = 100.0;
        img.pan_y = 50.0;
        img.dir_x = -1.0;
        img.dir_y = -1.0;
        img.reset_pan();
        assert_eq!(img.pan_x, 0.0);
        assert_eq!(img.pan_y, 0.0);
        assert_eq!(img.dir_x, 1.0);
        assert_eq!(img.dir_y, 1.0);
    }
}
