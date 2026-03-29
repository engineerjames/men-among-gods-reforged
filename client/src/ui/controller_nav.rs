//! Stateful rising-edge tracker for controller → UI navigation events.
//!
//! Pre-game scenes own a [`ControllerNavState`] and call
//! [`ControllerNavState::process_event`] for every raw SDL event.  When a
//! navigation-relevant input is detected (D-pad button, A/Start, or left-stick
//! beyond a threshold), the method returns the appropriate [`UiEvent`]
//! (`NavNext`, `NavPrev`, or `NavConfirm`) which the scene can then forward to
//! its form widget.
//!
//! Axis inputs use *rising-edge* detection: once the stick crosses the
//! threshold the event fires exactly once; the stick must return inside the
//! dead-zone before the same direction fires again.

use sdl2::event::Event;

use super::widget::UiEvent;

/// Dead-zone threshold for left-stick navigation.
const NAV_DEADZONE: i16 = 16_000;

/// Converts raw SDL controller events into [`UiEvent::NavNext`],
/// [`UiEvent::NavPrev`], or [`UiEvent::NavConfirm`].
///
/// Keeps internal state for rising-edge axis detection so that holding
/// the stick in one direction produces only a single event until the
/// stick returns to centre.
pub struct ControllerNavState {
    /// Left-stick X was beyond `+NAV_DEADZONE` last frame.
    axis_pos_x: bool,
    /// Left-stick X was beyond `-NAV_DEADZONE` last frame.
    axis_neg_x: bool,
    /// Left-stick Y was beyond `+NAV_DEADZONE` last frame (stick down).
    axis_pos_y: bool,
    /// Left-stick Y was beyond `-NAV_DEADZONE` last frame (stick up).
    axis_neg_y: bool,
}

impl ControllerNavState {
    /// Creates a new tracker with all axes in the dead-zone.
    pub fn new() -> Self {
        Self {
            axis_pos_x: false,
            axis_neg_x: false,
            axis_pos_y: false,
            axis_neg_y: false,
        }
    }

    /// Inspects a raw SDL event and returns a nav event if appropriate.
    ///
    /// # Arguments
    ///
    /// * `event` - The raw SDL2 event from the event pump.
    ///
    /// # Returns
    ///
    /// `Some(UiEvent::NavNext)`, `Some(UiEvent::NavPrev)`, or
    /// `Some(UiEvent::NavConfirm)` when the event maps to a navigation
    /// action, `None` otherwise.
    pub fn process_event(&mut self, event: &Event) -> Option<UiEvent> {
        match event {
            // ── D-pad buttons (instant, no debounce needed) ──────────
            Event::ControllerButtonDown { button, .. } => {
                use sdl2::controller::Button as Btn;
                match button {
                    Btn::DPadDown | Btn::DPadRight => Some(UiEvent::NavNext),
                    Btn::DPadUp | Btn::DPadLeft => Some(UiEvent::NavPrev),
                    Btn::A | Btn::Start => Some(UiEvent::NavConfirm),
                    _ => None,
                }
            }

            // ── Left-stick axis (rising-edge gated) ──────────────────
            Event::ControllerAxisMotion { axis, value, .. } => {
                use sdl2::controller::Axis;
                match axis {
                    Axis::LeftX => {
                        let v = *value;
                        if v > NAV_DEADZONE && !self.axis_pos_x {
                            self.axis_pos_x = true;
                            return Some(UiEvent::NavNext);
                        } else if v <= NAV_DEADZONE {
                            self.axis_pos_x = false;
                        }
                        if v < -NAV_DEADZONE && !self.axis_neg_x {
                            self.axis_neg_x = true;
                            return Some(UiEvent::NavPrev);
                        } else if v >= -NAV_DEADZONE {
                            self.axis_neg_x = false;
                        }
                        None
                    }
                    Axis::LeftY => {
                        let v = *value;
                        // SDL Y axis: positive = stick pushed down.
                        if v > NAV_DEADZONE && !self.axis_pos_y {
                            self.axis_pos_y = true;
                            return Some(UiEvent::NavNext);
                        } else if v <= NAV_DEADZONE {
                            self.axis_pos_y = false;
                        }
                        if v < -NAV_DEADZONE && !self.axis_neg_y {
                            self.axis_neg_y = true;
                            return Some(UiEvent::NavPrev);
                        } else if v >= -NAV_DEADZONE {
                            self.axis_neg_y = false;
                        }
                        None
                    }
                    _ => None,
                }
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Simulates a controller button down event for testing.
    fn btn_event(button: sdl2::controller::Button) -> Event {
        Event::ControllerButtonDown {
            timestamp: 0,
            which: 0,
            button,
        }
    }

    /// Simulates a controller axis motion event for testing.
    fn axis_event(axis: sdl2::controller::Axis, value: i16) -> Event {
        Event::ControllerAxisMotion {
            timestamp: 0,
            which: 0,
            axis,
            value,
        }
    }

    #[test]
    fn dpad_down_maps_to_nav_next() {
        let mut state = ControllerNavState::new();
        assert_eq!(
            state.process_event(&btn_event(sdl2::controller::Button::DPadDown)),
            Some(UiEvent::NavNext)
        );
    }

    #[test]
    fn dpad_up_maps_to_nav_prev() {
        let mut state = ControllerNavState::new();
        assert_eq!(
            state.process_event(&btn_event(sdl2::controller::Button::DPadUp)),
            Some(UiEvent::NavPrev)
        );
    }

    #[test]
    fn a_button_maps_to_nav_confirm() {
        let mut state = ControllerNavState::new();
        assert_eq!(
            state.process_event(&btn_event(sdl2::controller::Button::A)),
            Some(UiEvent::NavConfirm)
        );
    }

    #[test]
    fn start_button_maps_to_nav_confirm() {
        let mut state = ControllerNavState::new();
        assert_eq!(
            state.process_event(&btn_event(sdl2::controller::Button::Start)),
            Some(UiEvent::NavConfirm)
        );
    }

    #[test]
    fn axis_rising_edge_fires_once() {
        let mut state = ControllerNavState::new();
        // First crossing fires.
        assert_eq!(
            state.process_event(&axis_event(sdl2::controller::Axis::LeftY, 20_000)),
            Some(UiEvent::NavNext)
        );
        // Held past threshold — no repeat.
        assert_eq!(
            state.process_event(&axis_event(sdl2::controller::Axis::LeftY, 25_000)),
            None
        );
        // Return to dead-zone.
        assert_eq!(
            state.process_event(&axis_event(sdl2::controller::Axis::LeftY, 0)),
            None
        );
        // Cross again — fires.
        assert_eq!(
            state.process_event(&axis_event(sdl2::controller::Axis::LeftY, 20_000)),
            Some(UiEvent::NavNext)
        );
    }

    #[test]
    fn axis_negative_fires_nav_prev() {
        let mut state = ControllerNavState::new();
        assert_eq!(
            state.process_event(&axis_event(sdl2::controller::Axis::LeftY, -20_000)),
            Some(UiEvent::NavPrev)
        );
        // Held — no repeat.
        assert_eq!(
            state.process_event(&axis_event(sdl2::controller::Axis::LeftY, -25_000)),
            None
        );
    }

    #[test]
    fn unrelated_event_returns_none() {
        let mut state = ControllerNavState::new();
        let event = Event::Quit { timestamp: 0 };
        assert_eq!(state.process_event(&event), None);
    }
}
