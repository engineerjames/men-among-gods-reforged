//! Rectangular and circular button widgets.

use std::{collections::HashMap, path::PathBuf};

use sdl2::pixels::Color;
use sdl2::render::BlendMode;

use crate::font_cache;
use crate::ui::RenderContext;
use crate::ui::style::{Background, Border};
use crate::ui::widget::{Bounds, EventResponse, MouseButton, UiEvent, Widget};

// ---------------------------------------------------------------------------
// RectButton
// ---------------------------------------------------------------------------

/// A clickable rectangular button with optional label and hover highlight.
pub struct RectButton {
    bounds: Bounds,
    background: Background,
    border: Option<Border>,
    label_text: Option<String>,
    label_font: usize,
    label_offset_x: i32,
    label_offset_y: i32,
    hovered: bool,
    /// Additive tint alpha applied on hover (0–255).
    hover_alpha: u8,
}

impl RectButton {
    /// Creates a new rectangular button.
    ///
    /// # Arguments
    ///
    /// * `bounds` - Position and size.
    /// * `background` - Fill style.
    ///
    /// # Returns
    ///
    /// A new `RectButton` with no label and no border.
    pub fn new(bounds: Bounds, background: Background) -> Self {
        Self {
            bounds,
            background,
            border: None,
            label_text: None,
            label_font: 1,
            label_offset_x: 0,
            label_offset_y: 0,
            hovered: false,
            hover_alpha: 96,
        }
    }

    /// Sets the border style.
    ///
    /// # Arguments
    ///
    /// * `border` - Border configuration.
    ///
    /// # Returns
    ///
    /// `self` for chaining.
    pub fn with_border(mut self, border: Border) -> Self {
        self.border = Some(border);
        self
    }

    /// Sets the button label text.
    ///
    /// # Arguments
    ///
    /// * `text` - Label string.
    /// * `font` - Bitmap font index (0–3).
    ///
    /// # Returns
    ///
    /// `self` for chaining.
    pub fn with_label(mut self, text: &str, font: usize) -> Self {
        self.label_text = Some(text.to_owned());
        self.label_font = font;
        self
    }

    /// Offsets the button label relative to its default centered position.
    ///
    /// # Arguments
    ///
    /// * `offset_x` - Horizontal pixel offset applied to the label center.
    /// * `offset_y` - Vertical pixel offset applied to the label baseline origin.
    ///
    /// # Returns
    ///
    /// `self` for chaining.
    pub fn with_label_offset(mut self, offset_x: i32, offset_y: i32) -> Self {
        self.label_offset_x = offset_x;
        self.label_offset_y = offset_y;
        self
    }

    /// Returns whether the button is currently hovered.
    ///
    /// # Returns
    ///
    /// * `true` when `is_hovered` succeeds or the condition is met, otherwise `false`.
    pub fn is_hovered(&self) -> bool {
        self.hovered
    }

    /// Sets the hover/focus highlight state directly.
    ///
    /// Used by controller focus navigation to highlight a button without a
    /// mouse event.
    ///
    /// # Arguments
    ///
    /// * `hovered` - `true` to show the highlight, `false` to clear it.
    pub fn set_hovered(&mut self, hovered: bool) {
        self.hovered = hovered;
    }

    /// Replaces the button label text.
    ///
    /// # Arguments
    ///
    /// * `text` - New label string.
    pub fn set_label(&mut self, text: &str) {
        self.label_text = Some(text.to_owned());
    }
}

impl Widget for RectButton {
    fn bounds(&self) -> &Bounds {
        &self.bounds
    }

    fn set_position(&mut self, x: i32, y: i32) {
        self.bounds.x = x;
        self.bounds.y = y;
    }

    fn handle_event(&mut self, event: &UiEvent) -> EventResponse {
        match event {
            UiEvent::MouseMove { x, y } => {
                self.hovered = self.bounds.contains_point(*x, *y);
                EventResponse::Ignored
            }
            UiEvent::MouseClick {
                x,
                y,
                button: MouseButton::Left,
                ..
            } => {
                if self.bounds.contains_point(*x, *y) {
                    EventResponse::Consumed
                } else {
                    EventResponse::Ignored
                }
            }
            _ => EventResponse::Ignored,
        }
    }

    fn render(&mut self, ctx: &mut RenderContext<'_, '_>) -> Result<(), String> {
        let rect = sdl2::rect::Rect::new(
            self.bounds.x,
            self.bounds.y,
            self.bounds.width,
            self.bounds.height,
        );

        // Background
        match self.background {
            Background::SolidColor(color) => {
                ctx.canvas.set_blend_mode(BlendMode::Blend);
                ctx.canvas.set_draw_color(color);
                ctx.canvas.fill_rect(rect)?;
            }
            Background::None => {}
        }

        // Border
        if let Some(ref border) = self.border {
            ctx.canvas.set_draw_color(border.color);
            ctx.canvas.draw_rect(rect)?;
        }

        // Label (centered)
        if let Some(ref text) = self.label_text {
            let center_x = self.bounds.x + self.bounds.width as i32 / 2 + self.label_offset_x;
            let center_y = self.bounds.y
                + (self.bounds.height as i32 - font_cache::BITMAP_GLYPH_H as i32) / 2
                + self.label_offset_y;
            font_cache::draw_text(
                ctx.canvas,
                ctx.gfx,
                self.label_font,
                text,
                center_x,
                center_y,
                font_cache::TextStyle::centered(),
            )?;
        }

        // Hover highlight (additive blend, matching draw_ui_item_with_hover)
        if self.hovered {
            ctx.canvas.set_blend_mode(BlendMode::Add);
            ctx.canvas
                .set_draw_color(Color::RGBA(255, 255, 255, self.hover_alpha));
            ctx.canvas.fill_rect(rect)?;
            ctx.canvas.set_blend_mode(BlendMode::Blend);
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// CircleButton
// ---------------------------------------------------------------------------

/// A clickable circular button.
///
/// Hit-testing uses the true Euclidean distance from the center.  Rendering
/// uses a midpoint-circle scanline fill so no external dependencies are
/// needed.
pub struct CircleButton {
    center_x: i32,
    center_y: i32,
    radius: u32,
    fill_color: Color,
    border_color: Option<Color>,
    hovered: bool,
    hover_alpha: u8,
    /// Optional sprite drawn centered inside the circle.
    sprite_id: Option<usize>,
    /// Cached bounding box, kept in sync with center/radius.
    cached_bounds: Bounds,
    /// Optional small text badge drawn in the upper-right corner.
    badge_text: Option<String>,
    /// Tint color used for the badge text.
    badge_color: Color,
}

/// Default badge text color (saturated red, easily visible over icons).
const DEFAULT_BADGE_COLOR: Color = Color::RGB(220, 40, 40);

impl CircleButton {
    /// Creates a new circle button.
    ///
    /// # Arguments
    ///
    /// * `center_x` - X center in logical pixels.
    /// * `center_y` - Y center in logical pixels.
    /// * `radius` - Radius in pixels.
    /// * `fill_color` - Fill color.
    ///
    /// # Returns
    ///
    /// A new `CircleButton`.
    pub fn new(center_x: i32, center_y: i32, radius: u32, fill_color: Color) -> Self {
        Self {
            center_x,
            center_y,
            radius,
            fill_color,
            border_color: None,
            hovered: false,
            hover_alpha: 96,
            sprite_id: None,
            cached_bounds: Self::compute_bounds(center_x, center_y, radius),
            badge_text: None,
            badge_color: DEFAULT_BADGE_COLOR,
        }
    }

    /// Computes the axis-aligned bounding box from center and radius.
    fn compute_bounds(cx: i32, cy: i32, r: u32) -> Bounds {
        Bounds::new(cx - r as i32, cy - r as i32, r * 2, r * 2)
    }

    /// Sets the border color for the circle outline.
    ///
    /// # Arguments
    ///
    /// * `color` - Outline color.
    ///
    /// # Returns
    ///
    /// `self` for chaining.
    pub fn with_border_color(mut self, color: Color) -> Self {
        self.border_color = Some(color);
        self
    }

    /// Sets a sprite to draw centered inside the circle.
    ///
    /// # Arguments
    ///
    /// * `id` - Sprite ID from the graphics cache.
    ///
    /// # Returns
    ///
    /// `self` for chaining.
    pub fn with_sprite(mut self, id: usize) -> Self {
        self.sprite_id = Some(id);
        self
    }

    /// Sets the badge text color.
    ///
    /// # Arguments
    ///
    /// * `color` - Tint color applied to badge glyphs.
    ///
    /// # Returns
    ///
    /// `self` for chaining.
    pub fn with_badge_color(mut self, color: Color) -> Self {
        self.badge_color = color;
        self
    }

    /// Sets or clears the small badge text drawn in the upper-right corner.
    ///
    /// Pass `None` to hide the badge. Pass `Some(text)` with a short string
    /// (typically 1–3 characters) to display it.
    ///
    /// # Arguments
    ///
    /// * `text` - Optional badge text. `None` hides the badge.
    pub fn set_badge(&mut self, text: Option<String>) {
        self.badge_text = text;
    }

    /// Returns the current badge text, if any.
    ///
    /// # Returns
    ///
    /// * `Some(&str)` when a badge is set, otherwise `None`.
    pub fn badge_text(&self) -> Option<&str> {
        self.badge_text.as_deref()
    }

    /// Returns `true` if the point `(px, py)` is inside the circle.
    ///
    /// Uses integer arithmetic to avoid floating-point: `dx² + dy² <= r²`.
    ///
    /// # Arguments
    ///
    /// * `px` - X coordinate.
    /// * `py` - Y coordinate.
    ///
    /// # Returns
    ///
    /// `true` if inside or on the boundary.
    fn contains_point(&self, px: i32, py: i32) -> bool {
        let dx = i64::from(px - self.center_x);
        let dy = i64::from(py - self.center_y);
        let r = i64::from(self.radius);
        dx * dx + dy * dy <= r * r
    }

    /// Returns whether the button is currently hovered.
    ///
    /// # Returns
    ///
    /// * `true` when `is_hovered` succeeds or the condition is met, otherwise `false`.
    pub fn is_hovered(&self) -> bool {
        self.hovered
    }

    /// Draws a filled circle using the midpoint algorithm + horizontal
    /// scanlines.
    ///
    /// Spans are collected into a `HashMap` keyed by row Y before any drawing
    /// occurs, so each row is painted exactly once regardless of how many
    /// octant passes would have covered it.  This prevents double-alpha
    /// artefacts under `BlendMode::Add` (e.g. bright bands at the center row
    /// or at the 45° diagonals).
    fn fill_circle(
        canvas: &mut sdl2::render::Canvas<sdl2::video::Window>,
        cx: i32,
        cy: i32,
        r: u32,
    ) -> Result<(), String> {
        // TODO: It's probably not ideal that we're calculating this from scratch every
        // frame; this is a huge potential for optimization in the future if it becomes a
        // bottleneck.
        if r == 0 {
            return canvas.draw_point(sdl2::rect::Point::new(cx, cy));
        }
        let r = r as i32;
        let mut x = r;
        let mut y: i32 = 0;
        let mut err = 1 - r;

        let mut rows: HashMap<i32, i32> = HashMap::new();

        while x >= y {
            // Each iteration contributes up to four unique rows.
            for (row_y, half_w) in [(cy + y, x), (cy - y, x), (cy + x, y), (cy - x, y)] {
                rows.entry(row_y)
                    .and_modify(|w| *w = (*w).max(half_w))
                    .or_insert(half_w);
            }

            y += 1;
            if err < 0 {
                err += 2 * y + 1;
            } else {
                x -= 1;
                err += 2 * (y - x) + 1;
            }
        }

        // Expand each row into its two endpoints and draw everything in one
        // pass so each pixel is touched exactly once.
        let points: Vec<sdl2::rect::Point> = rows
            .into_iter()
            .flat_map(|(row_y, half_w)| {
                [
                    sdl2::rect::Point::new(cx - half_w, row_y),
                    sdl2::rect::Point::new(cx + half_w, row_y),
                ]
            })
            .collect();

        for pair in points.chunks(2) {
            canvas.draw_line(pair[0], pair[1])?;
        }

        Ok(())
    }

    /// Draws a circle outline using the midpoint algorithm.
    /// TODO: We're already using SDL's gfx library; we should
    /// probably just use it instead of reimplementing algorithms
    /// to draw circles from scratch...
    fn draw_circle_outline(
        canvas: &mut sdl2::render::Canvas<sdl2::video::Window>,
        cx: i32,
        cy: i32,
        r: u32,
    ) -> Result<(), String> {
        if r == 0 {
            return canvas.draw_point(sdl2::rect::Point::new(cx, cy));
        }
        let r = r as i32;
        let mut x = r;
        let mut y: i32 = 0;
        let mut err = 1 - r;

        while x >= y {
            let points = [
                sdl2::rect::Point::new(cx + x, cy + y),
                sdl2::rect::Point::new(cx - x, cy + y),
                sdl2::rect::Point::new(cx + x, cy - y),
                sdl2::rect::Point::new(cx - x, cy - y),
                sdl2::rect::Point::new(cx + y, cy + x),
                sdl2::rect::Point::new(cx - y, cy + x),
                sdl2::rect::Point::new(cx + y, cy - x),
                sdl2::rect::Point::new(cx - y, cy - x),
            ];
            canvas.draw_points(points.as_ref())?;

            y += 1;
            if err < 0 {
                err += 2 * y + 1;
            } else {
                x -= 1;
                err += 2 * (y - x) + 1;
            }
        }
        Ok(())
    }
}

impl Widget for CircleButton {
    fn bounds(&self) -> &Bounds {
        &self.cached_bounds
    }

    fn set_position(&mut self, x: i32, y: i32) {
        self.center_x = x + self.radius as i32;
        self.center_y = y + self.radius as i32;
        self.cached_bounds = Self::compute_bounds(self.center_x, self.center_y, self.radius);
    }

    fn handle_event(&mut self, event: &UiEvent) -> EventResponse {
        match event {
            UiEvent::MouseMove { x, y } => {
                self.hovered = self.contains_point(*x, *y);
                EventResponse::Ignored
            }
            UiEvent::MouseClick {
                x,
                y,
                button: MouseButton::Left,
                ..
            } => {
                if self.contains_point(*x, *y) {
                    EventResponse::Consumed
                } else {
                    EventResponse::Ignored
                }
            }
            _ => EventResponse::Ignored,
        }
    }

    fn render(&mut self, ctx: &mut RenderContext<'_, '_>) -> Result<(), String> {
        // Fill
        ctx.canvas.set_blend_mode(BlendMode::Blend);
        ctx.canvas.set_draw_color(self.fill_color);
        Self::fill_circle(ctx.canvas, self.center_x, self.center_y, self.radius)?;

        // Outline
        if let Some(color) = self.border_color {
            ctx.canvas.set_draw_color(color);
            Self::draw_circle_outline(ctx.canvas, self.center_x, self.center_y, self.radius)?;
        }

        // Sprite icon (centered inside the circle)
        if let Some(id) = self.sprite_id {
            let texture = ctx.gfx.get_texture(id);
            let q = texture.query();
            let dst_x = self.center_x - q.width as i32 / 2;
            let dst_y = self.center_y - q.height as i32 / 2;
            ctx.canvas.copy(
                texture,
                None,
                Some(sdl2::rect::Rect::new(dst_x, dst_y, q.width, q.height)),
            )?;
        }

        // Hover highlight
        if self.hovered {
            ctx.canvas.set_blend_mode(BlendMode::Add);
            ctx.canvas
                .set_draw_color(Color::RGBA(255, 255, 255, self.hover_alpha));
            Self::fill_circle(ctx.canvas, self.center_x, self.center_y, self.radius)?;
            ctx.canvas.set_blend_mode(BlendMode::Blend);
        }

        // Badge text (upper-right corner). Drawn last so it sits above the
        // sprite icon and any hover tint.
        if let Some(text) = self.badge_text.as_deref()
            && !text.is_empty()
        {
            let text_w = (text.len() as i32) * font_cache::BITMAP_GLYPH_ADVANCE as i32;
            let r = self.radius as i32;
            let tx = self.center_x + r - text_w;
            let ty = self.center_y - r;
            let style = font_cache::TextStyle::default()
                .with_tint(self.badge_color)
                .with_drop_shadow();
            font_cache::draw_text(ctx.canvas, ctx.gfx, 0, text, tx, ty, style)?;
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// CircularImageButton
// ---------------------------------------------------------------------------

/// A clickable circular button rendered from a whole-button image.
///
/// The image is scaled to the button's bounding square. Hit-testing remains
/// circular, and hover/badge overlays are drawn above the image.
pub struct CircularImageButton {
    center_x: i32,
    center_y: i32,
    radius: u32,
    image_path: PathBuf,
    texture_id: Option<usize>,
    load_failed: bool,
    hovered: bool,
    hover_alpha: u8,
    /// Cached bounding box, kept in sync with center/radius.
    cached_bounds: Bounds,
    /// Optional small text badge drawn in the upper-right corner.
    badge_text: Option<String>,
    /// Tint color used for the badge text.
    badge_color: Color,
}

impl CircularImageButton {
    /// Creates a new circular image button.
    ///
    /// # Arguments
    ///
    /// * `center_x` - X center in logical pixels.
    /// * `center_y` - Y center in logical pixels.
    /// * `radius` - Radius in pixels.
    /// * `image_path` - Filesystem path to the whole-button PNG image.
    ///
    /// # Returns
    ///
    /// A new `CircularImageButton`.
    pub fn new(center_x: i32, center_y: i32, radius: u32, image_path: PathBuf) -> Self {
        Self {
            center_x,
            center_y,
            radius,
            image_path,
            texture_id: None,
            load_failed: false,
            hovered: false,
            hover_alpha: 96,
            cached_bounds: CircleButton::compute_bounds(center_x, center_y, radius),
            badge_text: None,
            badge_color: DEFAULT_BADGE_COLOR,
        }
    }

    /// Sets the badge text color.
    ///
    /// # Arguments
    ///
    /// * `color` - Tint color applied to badge glyphs.
    ///
    /// # Returns
    ///
    /// `self` for chaining.
    pub fn with_badge_color(mut self, color: Color) -> Self {
        self.badge_color = color;
        self
    }

    /// Sets or clears the small badge text drawn in the upper-right corner.
    ///
    /// Pass `None` to hide the badge. Pass `Some(text)` with a short string
    /// (typically 1-3 characters) to display it.
    ///
    /// # Arguments
    ///
    /// * `text` - Optional badge text. `None` hides the badge.
    pub fn set_badge(&mut self, text: Option<String>) {
        self.badge_text = text;
    }

    /// Returns the current badge text, if any.
    ///
    /// # Returns
    ///
    /// * `Some(&str)` when a badge is set, otherwise `None`.
    pub fn badge_text(&self) -> Option<&str> {
        self.badge_text.as_deref()
    }

    /// Returns `true` if the point `(px, py)` is inside the circular hit area.
    ///
    /// # Arguments
    ///
    /// * `px` - X coordinate.
    /// * `py` - Y coordinate.
    ///
    /// # Returns
    ///
    /// `true` if inside or on the boundary.
    fn contains_point(&self, px: i32, py: i32) -> bool {
        let dx = i64::from(px - self.center_x);
        let dy = i64::from(py - self.center_y);
        let r = i64::from(self.radius);
        dx * dx + dy * dy <= r * r
    }

    /// Returns whether the button is currently hovered.
    ///
    /// # Returns
    ///
    /// * `true` when the cursor is inside the circular hit area.
    pub fn is_hovered(&self) -> bool {
        self.hovered
    }

    /// Ensures the button image is loaded into the graphics cache.
    ///
    /// # Arguments
    ///
    /// * `ctx` - Render context containing the graphics cache.
    ///
    /// # Returns
    ///
    /// * `Some(texture_id)` when the image is available, otherwise `None`.
    fn ensure_texture_id(&mut self, ctx: &mut RenderContext<'_, '_>) -> Option<usize> {
        if self.texture_id.is_some() || self.load_failed {
            return self.texture_id;
        }

        match ctx.gfx.load_texture_from_path(&self.image_path) {
            Ok(id) => {
                self.texture_id = Some(id);
                Some(id)
            }
            Err(err) => {
                log::warn!(
                    "Failed to load circular image button {}: {}",
                    self.image_path.display(),
                    err
                );
                self.load_failed = true;
                None
            }
        }
    }

    /// Draws the optional badge text above the image and hover overlay.
    ///
    /// # Arguments
    ///
    /// * `ctx` - Render context used for drawing.
    ///
    /// # Returns
    ///
    /// `Ok(())` when rendering succeeds, or an SDL error string.
    fn render_badge(&self, ctx: &mut RenderContext<'_, '_>) -> Result<(), String> {
        if let Some(text) = self.badge_text.as_deref()
            && !text.is_empty()
        {
            let text_w = (text.len() as i32) * font_cache::BITMAP_GLYPH_ADVANCE as i32;
            let r = self.radius as i32;
            let tx = self.center_x + r - text_w;
            let ty = self.center_y - r;
            let style = font_cache::TextStyle::default()
                .with_tint(self.badge_color)
                .with_drop_shadow();
            font_cache::draw_text(ctx.canvas, ctx.gfx, 0, text, tx, ty, style)?;
        }

        Ok(())
    }
}

impl Widget for CircularImageButton {
    fn bounds(&self) -> &Bounds {
        &self.cached_bounds
    }

    fn set_position(&mut self, x: i32, y: i32) {
        self.center_x = x + self.radius as i32;
        self.center_y = y + self.radius as i32;
        self.cached_bounds =
            CircleButton::compute_bounds(self.center_x, self.center_y, self.radius);
    }

    fn handle_event(&mut self, event: &UiEvent) -> EventResponse {
        match event {
            UiEvent::MouseMove { x, y } => {
                self.hovered = self.contains_point(*x, *y);
                EventResponse::Ignored
            }
            UiEvent::MouseClick {
                x,
                y,
                button: MouseButton::Left,
                ..
            } => {
                if self.contains_point(*x, *y) {
                    EventResponse::Consumed
                } else {
                    EventResponse::Ignored
                }
            }
            _ => EventResponse::Ignored,
        }
    }

    fn render(&mut self, ctx: &mut RenderContext<'_, '_>) -> Result<(), String> {
        if let Some(id) = self.ensure_texture_id(ctx) {
            let texture = ctx.gfx.get_texture(id);
            ctx.canvas.set_blend_mode(BlendMode::Blend);
            ctx.canvas.copy(
                texture,
                None,
                Some(sdl2::rect::Rect::new(
                    self.cached_bounds.x,
                    self.cached_bounds.y,
                    self.cached_bounds.width,
                    self.cached_bounds.height,
                )),
            )?;
        }

        if self.hovered {
            ctx.canvas.set_blend_mode(BlendMode::Add);
            ctx.canvas
                .set_draw_color(Color::RGBA(255, 255, 255, self.hover_alpha));
            CircleButton::fill_circle(ctx.canvas, self.center_x, self.center_y, self.radius)?;
            ctx.canvas.set_blend_mode(BlendMode::Blend);
        }

        self.render_badge(ctx)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::widget::KeyModifiers;

    // -- RectButton --

    #[test]
    fn rect_button_hover_toggle() {
        let mut btn = RectButton::new(Bounds::new(10, 10, 50, 30), Background::None);
        assert!(!btn.is_hovered());

        // Move inside
        btn.handle_event(&UiEvent::MouseMove { x: 20, y: 20 });
        assert!(btn.is_hovered());

        // Move outside
        btn.handle_event(&UiEvent::MouseMove { x: 0, y: 0 });
        assert!(!btn.is_hovered());
    }

    #[test]
    fn rect_button_click_inside_consumed() {
        let mut btn = RectButton::new(Bounds::new(10, 10, 50, 30), Background::None);
        let resp = btn.handle_event(&UiEvent::MouseClick {
            x: 20,
            y: 20,
            button: MouseButton::Left,
            modifiers: KeyModifiers::default(),
        });
        assert_eq!(resp, EventResponse::Consumed);
    }

    #[test]
    fn rect_button_click_outside_ignored() {
        let mut btn = RectButton::new(Bounds::new(10, 10, 50, 30), Background::None);
        let resp = btn.handle_event(&UiEvent::MouseClick {
            x: 0,
            y: 0,
            button: MouseButton::Left,
            modifiers: KeyModifiers::default(),
        });
        assert_eq!(resp, EventResponse::Ignored);
    }

    // -- CircleButton --

    #[test]
    fn circle_contains_center() {
        let btn = CircleButton::new(100, 100, 20, Color::RGB(255, 0, 0));
        assert!(btn.contains_point(100, 100));
    }

    #[test]
    fn circle_contains_edge() {
        let btn = CircleButton::new(100, 100, 20, Color::RGB(255, 0, 0));
        // Point exactly on the radius (along X axis): (120, 100)
        assert!(btn.contains_point(120, 100));
    }

    #[test]
    fn circle_excludes_outside() {
        let btn = CircleButton::new(100, 100, 20, Color::RGB(255, 0, 0));
        assert!(!btn.contains_point(121, 100));
    }

    #[test]
    fn circle_diagonal_check() {
        let btn = CircleButton::new(0, 0, 10, Color::RGB(255, 0, 0));
        // (7, 7): 49 + 49 = 98, 10² = 100 --> inside
        assert!(btn.contains_point(7, 7));
        // (8, 8): 64 + 64 = 128 > 100 --> outside
        assert!(!btn.contains_point(8, 8));
    }

    #[test]
    fn circle_uses_i64_no_overflow() {
        // Large coordinates that would overflow i32 multiplication
        let btn = CircleButton::new(i32::MAX - 10, i32::MAX - 10, 5, Color::RGB(0, 0, 0));
        // Should not panic
        assert!(!btn.contains_point(0, 0));
    }

    #[test]
    fn circle_sprite_defaults_to_none() {
        let btn = CircleButton::new(50, 50, 10, Color::RGB(0, 0, 0));
        assert!(btn.sprite_id.is_none());
    }

    #[test]
    fn circle_with_sprite_sets_id() {
        let btn = CircleButton::new(50, 50, 10, Color::RGB(0, 0, 0)).with_sprite(267);
        assert_eq!(btn.sprite_id, Some(267));
    }

    #[test]
    fn circle_hover_toggle() {
        let mut btn = CircleButton::new(100, 100, 20, Color::RGB(255, 0, 0));
        assert!(!btn.is_hovered());

        btn.handle_event(&UiEvent::MouseMove { x: 100, y: 100 });
        assert!(btn.is_hovered());

        btn.handle_event(&UiEvent::MouseMove { x: 200, y: 200 });
        assert!(!btn.is_hovered());
    }

    #[test]
    fn circle_badge_defaults_to_none() {
        let btn = CircleButton::new(50, 50, 10, Color::RGB(0, 0, 0));
        assert!(btn.badge_text().is_none());
    }

    #[test]
    fn circle_set_badge_toggles_state() {
        let mut btn = CircleButton::new(50, 50, 10, Color::RGB(0, 0, 0));
        btn.set_badge(Some("3".into()));
        assert_eq!(btn.badge_text(), Some("3"));
        btn.set_badge(None);
        assert!(btn.badge_text().is_none());
    }

    #[test]
    fn circle_with_badge_color_overrides_default() {
        let btn = CircleButton::new(50, 50, 10, Color::RGB(0, 0, 0))
            .with_badge_color(Color::RGB(0, 255, 0));
        assert_eq!(btn.badge_color, Color::RGB(0, 255, 0));
    }

    // -- CircularImageButton --

    fn image_button() -> CircularImageButton {
        CircularImageButton::new(100, 100, 20, PathBuf::from("button.png"))
    }

    #[test]
    fn circular_image_contains_center() {
        let btn = image_button();
        assert!(btn.contains_point(100, 100));
    }

    #[test]
    fn circular_image_contains_edge() {
        let btn = image_button();
        assert!(btn.contains_point(120, 100));
    }

    #[test]
    fn circular_image_excludes_outside() {
        let btn = image_button();
        assert!(!btn.contains_point(121, 100));
    }

    #[test]
    fn circular_image_diagonal_check() {
        let btn = CircularImageButton::new(0, 0, 10, PathBuf::from("button.png"));
        assert!(btn.contains_point(7, 7));
        assert!(!btn.contains_point(8, 8));
    }

    #[test]
    fn circular_image_uses_i64_no_overflow() {
        let btn =
            CircularImageButton::new(i32::MAX - 10, i32::MAX - 10, 5, PathBuf::from("button.png"));
        assert!(!btn.contains_point(0, 0));
    }

    #[test]
    fn circular_image_hover_toggle() {
        let mut btn = image_button();
        assert!(!btn.is_hovered());

        btn.handle_event(&UiEvent::MouseMove { x: 100, y: 100 });
        assert!(btn.is_hovered());

        btn.handle_event(&UiEvent::MouseMove { x: 200, y: 200 });
        assert!(!btn.is_hovered());
    }

    #[test]
    fn circular_image_click_inside_consumed() {
        let mut btn = image_button();
        let resp = btn.handle_event(&UiEvent::MouseClick {
            x: 100,
            y: 100,
            button: MouseButton::Left,
            modifiers: KeyModifiers::default(),
        });
        assert_eq!(resp, EventResponse::Consumed);
    }

    #[test]
    fn circular_image_click_outside_ignored() {
        let mut btn = image_button();
        let resp = btn.handle_event(&UiEvent::MouseClick {
            x: 0,
            y: 0,
            button: MouseButton::Left,
            modifiers: KeyModifiers::default(),
        });
        assert_eq!(resp, EventResponse::Ignored);
    }

    #[test]
    fn circular_image_set_position_updates_bounds() {
        let mut btn = image_button();
        btn.set_position(10, 20);
        assert_eq!(*btn.bounds(), Bounds::new(10, 20, 40, 40));
        assert!(btn.contains_point(30, 40));
    }

    #[test]
    fn circular_image_badge_defaults_to_none() {
        let btn = image_button();
        assert!(btn.badge_text().is_none());
    }

    #[test]
    fn circular_image_set_badge_toggles_state() {
        let mut btn = image_button();
        btn.set_badge(Some("3".into()));
        assert_eq!(btn.badge_text(), Some("3"));
        btn.set_badge(None);
        assert!(btn.badge_text().is_none());
    }

    #[test]
    fn circular_image_with_badge_color_overrides_default() {
        let btn = image_button().with_badge_color(Color::RGB(0, 255, 0));
        assert_eq!(btn.badge_color, Color::RGB(0, 255, 0));
    }
}
