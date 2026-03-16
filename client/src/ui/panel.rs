//! Container widget that groups children, draws a background, and propagates
//! events.

use sdl2::render::BlendMode;

use super::style::{Background, Border, Padding};
use super::widget::{Bounds, EventResponse, UiEvent, Widget, WidgetAction};
use super::RenderContext;

/// A rectangular container that can hold child widgets.
///
/// Children are rendered in order (first added  drawn first / bottom-most).
/// Events propagate back-to-front so the top-most child gets first chance to
/// consume.
pub struct Panel {
    bounds: Bounds,
    background: Background,
    border: Option<Border>,
    padding: Padding,
    children: Vec<Box<dyn Widget>>,
}

impl Panel {
    /// Creates an empty panel with the given bounds.
    ///
    /// # Arguments
    ///
    /// * `bounds` - Position and size of the panel.
    ///
    /// # Returns
    ///
    /// A new `Panel` with no background, no border, zero padding, and no
    /// children.
    pub fn new(bounds: Bounds) -> Self {
        Self {
            bounds,
            background: Background::None,
            border: None,
            padding: Padding::ZERO,
            children: Vec::new(),
        }
    }

    /// Sets the background fill style.
    ///
    /// # Arguments
    ///
    /// * `bg` - The background to draw behind children.
    ///
    /// # Returns
    ///
    /// `self` for chaining.
    pub fn with_background(mut self, bg: Background) -> Self {
        self.background = bg;
        self
    }

    /// Sets the border drawn around the panel.
    ///
    /// # Arguments
    ///
    /// * `border` - Border style.
    ///
    /// # Returns
    ///
    /// `self` for chaining.
    pub fn with_border(mut self, border: Border) -> Self {
        self.border = Some(border);
        self
    }

    /// Sets the inner padding between the panel edge and its children.
    ///
    /// # Arguments
    ///
    /// * `padding` - Padding to apply.
    ///
    /// # Returns
    ///
    /// `self` for chaining.
    pub fn with_padding(mut self, padding: Padding) -> Self {
        self.padding = padding;
        self
    }

    /// Appends a child widget.
    ///
    /// # Arguments
    ///
    /// * `child` - The widget to add.
    pub fn add_child(&mut self, child: Box<dyn Widget>) {
        self.children.push(child);
    }
}

impl Widget for Panel {
    fn bounds(&self) -> &Bounds {
        &self.bounds
    }

    fn set_position(&mut self, x: i32, y: i32) {
        self.bounds.x = x;
        self.bounds.y = y;
    }

    fn handle_event(&mut self, event: &UiEvent) -> EventResponse {
        // Propagate back-to-front so topmost child gets first chance.
        for child in self.children.iter_mut().rev() {
            if child.handle_event(event) == EventResponse::Consumed {
                return EventResponse::Consumed;
            }
        }
        EventResponse::Ignored
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
            for i in 0..border.width {
                let offset = i as i32;
                let border_rect = sdl2::rect::Rect::new(
                    self.bounds.x + offset,
                    self.bounds.y + offset,
                    self.bounds.width.saturating_sub(i * 2),
                    self.bounds.height.saturating_sub(i * 2),
                );
                ctx.canvas.draw_rect(border_rect)?;
            }
        }

        // Children
        for child in &mut self.children {
            child.render(ctx)?;
        }

        Ok(())
    }

    fn take_actions(&mut self) -> Vec<WidgetAction> {
        let mut actions = Vec::new();
        for child in &mut self.children {
            actions.append(&mut child.take_actions());
        }
        actions
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Dummy widget that tracks whether it received an event.
    struct StubWidget {
        bounds: Bounds,
        consume: bool,
        event_count: usize,
    }

    impl StubWidget {
        fn new(consume: bool) -> Self {
            Self {
                bounds: Bounds::new(0, 0, 10, 10),
                consume,
                event_count: 0,
            }
        }
    }

    impl Widget for StubWidget {
        fn bounds(&self) -> &Bounds {
            &self.bounds
        }
        fn set_position(&mut self, x: i32, y: i32) {
            self.bounds.x = x;
            self.bounds.y = y;
        }
        fn handle_event(&mut self, _event: &UiEvent) -> EventResponse {
            self.event_count += 1;
            if self.consume {
                EventResponse::Consumed
            } else {
                EventResponse::Ignored
            }
        }
        fn render(&mut self, _ctx: &mut RenderContext<'_, '_>) -> Result<(), String> {
            Ok(())
        }
    }

    #[test]
    fn event_stops_at_consuming_child() {
        let mut panel = Panel::new(Bounds::new(0, 0, 200, 200));
        // First child ignores, second consumes.
        panel.add_child(Box::new(StubWidget::new(false)));
        panel.add_child(Box::new(StubWidget::new(true)));

        let event = UiEvent::MouseClick {
            x: 5,
            y: 5,
            button: super::super::widget::MouseButton::Left,
            modifiers: super::super::widget::KeyModifiers::default(),
        };
        let resp = panel.handle_event(&event);
        assert_eq!(resp, EventResponse::Consumed);
    }

    #[test]
    fn event_propagates_through_all_when_none_consume() {
        let mut panel = Panel::new(Bounds::new(0, 0, 200, 200));
        panel.add_child(Box::new(StubWidget::new(false)));
        panel.add_child(Box::new(StubWidget::new(false)));

        let event = UiEvent::MouseMove { x: 5, y: 5 };
        let resp = panel.handle_event(&event);
        assert_eq!(resp, EventResponse::Ignored);
    }
}
