//! Controller button types and binding storage for gamepad skill-bar mappings.
//!
//! [`ControllerButton`] represents a single button (or modifier+button combo)
//! on a standard gamepad. [`ControllerBindings`] maps skill-bar slots (0–8,
//! corresponding to keys 1–9) to controller buttons.

use std::fmt;

use serde::{Deserialize, Serialize};

/// Number of skill-bar slots that can be bound to controller buttons (1–9).
pub const CONTROLLER_BIND_SLOTS: usize = 9;

/// A button (or modifier + button combo) on a standard game controller.
///
/// Variants without a modifier prefix represent a single button press.
/// `Lb*` variants represent a button press while the left bumper is held.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ControllerButton {
    /// A / Cross button.
    A,
    /// B / Circle button.
    B,
    /// X / Square button.
    X,
    /// Y / Triangle button.
    Y,
    /// Left bumper (shoulder).
    Lb,
    /// Right bumper (shoulder).
    Rb,
    /// Left trigger (full press, treated as digital).
    Lt,
    /// Right trigger (full press, treated as digital).
    Rt,
    /// Left stick click.
    LeftStick,
    /// Right stick click.
    RightStick,
    /// Start / Menu button.
    Start,
    /// Back / View / Select button.
    Back,
    /// LB + A combo.
    LbA,
    /// LB + B combo.
    LbB,
    /// LB + X combo.
    LbX,
    /// LB + Y combo.
    LbY,
    /// RB + A combo.
    RbA,
    /// RB + B combo.
    RbB,
    /// RB + X combo.
    RbX,
    /// RB + Y combo.
    RbY,
}

impl ControllerButton {
    /// All single-button variants in display order (no combos).
    pub const SINGLE: &'static [ControllerButton] = &[
        ControllerButton::A,
        ControllerButton::B,
        ControllerButton::X,
        ControllerButton::Y,
        ControllerButton::Lb,
        ControllerButton::Rb,
        ControllerButton::Lt,
        ControllerButton::Rt,
        ControllerButton::LeftStick,
        ControllerButton::RightStick,
        ControllerButton::Start,
        ControllerButton::Back,
    ];

    /// All variants including combos, in display order.
    pub const ALL: &'static [ControllerButton] = &[
        ControllerButton::A,
        ControllerButton::B,
        ControllerButton::X,
        ControllerButton::Y,
        ControllerButton::Lb,
        ControllerButton::Rb,
        ControllerButton::Lt,
        ControllerButton::Rt,
        ControllerButton::LeftStick,
        ControllerButton::RightStick,
        ControllerButton::Start,
        ControllerButton::Back,
        ControllerButton::LbA,
        ControllerButton::LbB,
        ControllerButton::LbX,
        ControllerButton::LbY,
        ControllerButton::RbA,
        ControllerButton::RbB,
        ControllerButton::RbX,
        ControllerButton::RbY,
    ];

    /// Short display label suitable for UI buttons.
    pub fn label(self) -> &'static str {
        match self {
            Self::A => "A",
            Self::B => "B",
            Self::X => "X",
            Self::Y => "Y",
            Self::Lb => "LB",
            Self::Rb => "RB",
            Self::Lt => "LT",
            Self::Rt => "RT",
            Self::LeftStick => "LS",
            Self::RightStick => "RS",
            Self::Start => "Start",
            Self::Back => "Back",
            Self::LbA => "LB+A",
            Self::LbB => "LB+B",
            Self::LbX => "LB+X",
            Self::LbY => "LB+Y",
            Self::RbA => "RB+A",
            Self::RbB => "RB+B",
            Self::RbX => "RB+X",
            Self::RbY => "RB+Y",
        }
    }

    /// Attempt to match an SDL2 `GameController` button to a
    /// [`ControllerButton`], optionally considering a held modifier (LB or RB).
    ///
    /// # Arguments
    ///
    /// * `sdl_button` - The SDL2 controller button that was pressed.
    /// * `lb_held` - Whether the left bumper is currently held.
    /// * `rb_held` - Whether the right bumper is currently held.
    ///
    /// # Returns
    ///
    /// The matching `ControllerButton`, or `None` if the SDL button is not
    /// mapped (e.g. D-pad, guide).
    pub fn from_sdl2(
        sdl_button: sdl2::controller::Button,
        lb_held: bool,
        rb_held: bool,
    ) -> Option<Self> {
        use sdl2::controller::Button as Btn;
        match sdl_button {
            Btn::A if lb_held => Some(Self::LbA),
            Btn::B if lb_held => Some(Self::LbB),
            Btn::X if lb_held => Some(Self::LbX),
            Btn::Y if lb_held => Some(Self::LbY),
            Btn::A if rb_held => Some(Self::RbA),
            Btn::B if rb_held => Some(Self::RbB),
            Btn::X if rb_held => Some(Self::RbX),
            Btn::Y if rb_held => Some(Self::RbY),
            Btn::A => Some(Self::A),
            Btn::B => Some(Self::B),
            Btn::X => Some(Self::X),
            Btn::Y => Some(Self::Y),
            Btn::LeftShoulder => Some(Self::Lb),
            Btn::RightShoulder => Some(Self::Rb),
            Btn::LeftStick => Some(Self::LeftStick),
            Btn::RightStick => Some(Self::RightStick),
            Btn::Start => Some(Self::Start),
            Btn::Back => Some(Self::Back),
            _ => None,
        }
    }

    /// Converts an SDL2 trigger axis value to a digital `ControllerButton`.
    ///
    /// # Arguments
    ///
    /// * `axis` - The SDL2 axis (`LeftTrigger` or `RightTrigger`).
    /// * `value` - Axis value (0–32767 for triggers).
    ///
    /// # Returns
    ///
    /// `Some(Lt)` or `Some(Rt)` if the trigger is past the threshold, `None`
    /// otherwise.
    pub fn from_trigger_axis(axis: sdl2::controller::Axis, value: i16) -> Option<Self> {
        const TRIGGER_THRESHOLD: i16 = 16000;
        if value < TRIGGER_THRESHOLD {
            return None;
        }
        match axis {
            sdl2::controller::Axis::TriggerLeft => Some(Self::Lt),
            sdl2::controller::Axis::TriggerRight => Some(Self::Rt),
            _ => None,
        }
    }
}

impl fmt::Display for ControllerButton {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Maps skill-bar slots (0–8, corresponding to keys 1–9) to controller
/// buttons.
///
/// Persisted inside [`CharacterSettings`](crate::preferences::CharacterSettings)
/// so each character can have independent controller bindings.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ControllerBindings {
    /// One entry per skill-bar slot. `Some(button)` if bound, `None` if unbound.
    bindings: [Option<ControllerButton>; CONTROLLER_BIND_SLOTS],
}

impl Default for ControllerBindings {
    fn default() -> Self {
        Self {
            bindings: [None; CONTROLLER_BIND_SLOTS],
        }
    }
}

impl ControllerBindings {
    /// Returns the binding for the given skill-bar slot (0–8).
    ///
    /// # Arguments
    ///
    /// * `slot` - Skill-bar slot index (0 = key "1", 8 = key "9").
    ///
    /// # Returns
    ///
    /// The bound `ControllerButton`, or `None` if the slot is unbound.
    pub fn get(&self, slot: usize) -> Option<ControllerButton> {
        self.bindings.get(slot).copied().flatten()
    }

    /// Sets (or clears) the binding for the given skill-bar slot.
    ///
    /// # Arguments
    ///
    /// * `slot` - Skill-bar slot index (0–8).
    /// * `button` - The button to bind, or `None` to clear.
    pub fn set(&mut self, slot: usize, button: Option<ControllerButton>) {
        if let Some(entry) = self.bindings.get_mut(slot) {
            *entry = button;
        }
    }

    /// Find which skill-bar slot (if any) is bound to `button`.
    ///
    /// # Arguments
    ///
    /// * `button` - The controller button to look up.
    ///
    /// # Returns
    ///
    /// The slot index (0–8), or `None` if no slot is bound to that button.
    pub fn slot_for_button(&self, button: ControllerButton) -> Option<usize> {
        self.bindings.iter().position(|b| *b == Some(button))
    }

    /// Returns a slice of all 9 binding slots.
    pub fn slots(&self) -> &[Option<ControllerButton>; CONTROLLER_BIND_SLOTS] {
        &self.bindings
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_bindings_are_all_none() {
        let bindings = ControllerBindings::default();
        for slot in 0..CONTROLLER_BIND_SLOTS {
            assert_eq!(bindings.get(slot), None);
        }
    }

    #[test]
    fn set_and_get_binding() {
        let mut bindings = ControllerBindings::default();
        bindings.set(0, Some(ControllerButton::A));
        bindings.set(4, Some(ControllerButton::LbA));
        assert_eq!(bindings.get(0), Some(ControllerButton::A));
        assert_eq!(bindings.get(4), Some(ControllerButton::LbA));
        assert_eq!(bindings.get(1), None);
    }

    #[test]
    fn clear_binding() {
        let mut bindings = ControllerBindings::default();
        bindings.set(2, Some(ControllerButton::X));
        assert_eq!(bindings.get(2), Some(ControllerButton::X));
        bindings.set(2, None);
        assert_eq!(bindings.get(2), None);
    }

    #[test]
    fn slot_for_button_found() {
        let mut bindings = ControllerBindings::default();
        bindings.set(3, Some(ControllerButton::Y));
        assert_eq!(bindings.slot_for_button(ControllerButton::Y), Some(3));
    }

    #[test]
    fn slot_for_button_not_found() {
        let bindings = ControllerBindings::default();
        assert_eq!(bindings.slot_for_button(ControllerButton::B), None);
    }

    #[test]
    fn out_of_bounds_slot_is_safe() {
        let mut bindings = ControllerBindings::default();
        bindings.set(100, Some(ControllerButton::A)); // no panic
        assert_eq!(bindings.get(100), None);
    }

    #[test]
    fn serde_roundtrip() {
        let mut bindings = ControllerBindings::default();
        bindings.set(0, Some(ControllerButton::A));
        bindings.set(5, Some(ControllerButton::LbB));
        let json = serde_json::to_string(&bindings).unwrap();
        let deserialized: ControllerBindings = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.get(0), Some(ControllerButton::A));
        assert_eq!(deserialized.get(5), Some(ControllerButton::LbB));
        assert_eq!(deserialized.get(1), None);
    }

    #[test]
    fn controller_button_display() {
        assert_eq!(ControllerButton::A.to_string(), "A");
        assert_eq!(ControllerButton::LbX.to_string(), "LB+X");
        assert_eq!(ControllerButton::RightStick.to_string(), "RS");
    }

    #[test]
    fn from_sdl2_single_button() {
        use sdl2::controller::Button as Btn;
        assert_eq!(
            ControllerButton::from_sdl2(Btn::A, false, false),
            Some(ControllerButton::A)
        );
        assert_eq!(
            ControllerButton::from_sdl2(Btn::Y, false, false),
            Some(ControllerButton::Y)
        );
    }

    #[test]
    fn from_sdl2_combo_with_lb() {
        use sdl2::controller::Button as Btn;
        assert_eq!(
            ControllerButton::from_sdl2(Btn::A, true, false),
            Some(ControllerButton::LbA)
        );
        assert_eq!(
            ControllerButton::from_sdl2(Btn::X, true, false),
            Some(ControllerButton::LbX)
        );
    }

    #[test]
    fn from_sdl2_combo_with_rb() {
        use sdl2::controller::Button as Btn;
        assert_eq!(
            ControllerButton::from_sdl2(Btn::A, false, true),
            Some(ControllerButton::RbA)
        );
    }

    #[test]
    fn from_sdl2_dpad_returns_none() {
        use sdl2::controller::Button as Btn;
        assert_eq!(ControllerButton::from_sdl2(Btn::DPadUp, false, false), None);
    }
}
