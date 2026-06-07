//! Mouse binding types for extra-button modifier shortcuts.
//!
//! This module contains the small persisted model used by the settings panel
//! and game scene to let mouse side buttons temporarily act like keyboard
//! modifiers.

use std::fmt;

use sdl2::mouse::MouseButton as SdlMouseButton;
use serde::{Deserialize, Serialize};

/// Number of extra mouse buttons supported by the Mouse Settings panel.
pub const EXTRA_MOUSE_BUTTON_COUNT: usize = 2;

/// Extra mouse buttons that can be bound to modifier behavior.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ExtraMouseButton {
    /// First side/auxiliary mouse button, reported by SDL as X1.
    Mouse4,
    /// Second side/auxiliary mouse button, reported by SDL as X2.
    Mouse5,
}

impl ExtraMouseButton {
    /// All supported extra mouse buttons in UI display order.
    pub const ALL: [ExtraMouseButton; EXTRA_MOUSE_BUTTON_COUNT] = [Self::Mouse4, Self::Mouse5];

    /// Returns a short display label for this button.
    ///
    /// # Returns
    ///
    /// * Human-readable button label.
    pub fn label(self) -> &'static str {
        match self {
            Self::Mouse4 => "Mouse 4",
            Self::Mouse5 => "Mouse 5",
        }
    }

    /// Converts an SDL mouse button into an extra mouse button binding value.
    ///
    /// # Arguments
    ///
    /// * `button` - SDL mouse button from a raw mouse-button event.
    ///
    /// # Returns
    ///
    /// * `Some` for Mouse 4/Mouse 5, otherwise `None`.
    pub fn from_sdl2(button: SdlMouseButton) -> Option<Self> {
        match button {
            SdlMouseButton::X1 => Some(Self::Mouse4),
            SdlMouseButton::X2 => Some(Self::Mouse5),
            _ => None,
        }
    }
}

impl fmt::Display for ExtraMouseButton {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Modifier behavior that can be triggered by an extra mouse button.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MouseModifier {
    /// Keyboard Ctrl modifier behavior.
    Ctrl,
    /// Keyboard Shift modifier behavior.
    Shift,
}

impl MouseModifier {
    /// All supported modifier targets in UI display order.
    pub const ALL: [MouseModifier; 2] = [Self::Ctrl, Self::Shift];

    /// Returns a short display label for this modifier.
    ///
    /// # Returns
    ///
    /// * Human-readable modifier label.
    pub fn label(self) -> &'static str {
        match self {
            Self::Ctrl => "Ctrl",
            Self::Shift => "Shift",
        }
    }
}

impl fmt::Display for MouseModifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Per-character mouse side-button bindings for modifier behavior.
///
/// Each modifier may be assigned at most one extra mouse button. Assigning a
/// button to one modifier automatically clears it from the other modifier.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct MouseModifierBindings {
    /// Extra mouse button assigned to Ctrl behavior.
    #[serde(default)]
    ctrl: Option<ExtraMouseButton>,
    /// Extra mouse button assigned to Shift behavior.
    #[serde(default)]
    shift: Option<ExtraMouseButton>,
}

impl MouseModifierBindings {
    /// Returns the button assigned to a modifier.
    ///
    /// # Arguments
    ///
    /// * `modifier` - Modifier target to query.
    ///
    /// # Returns
    ///
    /// * Bound extra mouse button, or `None` if unbound.
    pub fn get(&self, modifier: MouseModifier) -> Option<ExtraMouseButton> {
        match modifier {
            MouseModifier::Ctrl => self.ctrl,
            MouseModifier::Shift => self.shift,
        }
    }

    /// Sets or clears the button assigned to a modifier.
    ///
    /// If `button` is already assigned to the other modifier, that other
    /// assignment is cleared so one button cannot trigger both modifiers.
    ///
    /// # Arguments
    ///
    /// * `modifier` - Modifier target to change.
    /// * `button` - Extra mouse button to bind, or `None` to clear.
    pub fn set(&mut self, modifier: MouseModifier, button: Option<ExtraMouseButton>) {
        if let Some(button) = button {
            match modifier {
                MouseModifier::Ctrl if self.shift == Some(button) => self.shift = None,
                MouseModifier::Shift if self.ctrl == Some(button) => self.ctrl = None,
                _ => {}
            }
        }

        match modifier {
            MouseModifier::Ctrl => self.ctrl = button,
            MouseModifier::Shift => self.shift = button,
        }
    }

    /// Finds the modifier assigned to an extra mouse button.
    ///
    /// # Arguments
    ///
    /// * `button` - Extra mouse button to look up.
    ///
    /// # Returns
    ///
    /// * Bound modifier target, or `None` if the button is unbound.
    pub fn modifier_for_button(&self, button: ExtraMouseButton) -> Option<MouseModifier> {
        if self.ctrl == Some(button) {
            Some(MouseModifier::Ctrl)
        } else if self.shift == Some(button) {
            Some(MouseModifier::Shift)
        } else {
            None
        }
    }

    /// Returns a display label for a modifier binding button.
    ///
    /// # Arguments
    ///
    /// * `modifier` - Modifier target whose binding label should be shown.
    ///
    /// # Returns
    ///
    /// * Button label for the binding.
    pub fn button_label(&self, modifier: MouseModifier) -> &'static str {
        self.get(modifier)
            .map(ExtraMouseButton::label)
            .unwrap_or("Unbound")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_unbound() {
        let bindings = MouseModifierBindings::default();
        assert_eq!(bindings.get(MouseModifier::Ctrl), None);
        assert_eq!(bindings.get(MouseModifier::Shift), None);
    }

    #[test]
    fn labels_are_stable() {
        assert_eq!(ExtraMouseButton::Mouse4.label(), "Mouse 4");
        assert_eq!(ExtraMouseButton::Mouse5.label(), "Mouse 5");
        assert_eq!(MouseModifier::Ctrl.label(), "Ctrl");
        assert_eq!(MouseModifier::Shift.label(), "Shift");
    }

    #[test]
    fn set_get_and_clear_binding() {
        let mut bindings = MouseModifierBindings::default();
        bindings.set(MouseModifier::Ctrl, Some(ExtraMouseButton::Mouse4));
        assert_eq!(
            bindings.get(MouseModifier::Ctrl),
            Some(ExtraMouseButton::Mouse4)
        );
        bindings.set(MouseModifier::Ctrl, None);
        assert_eq!(bindings.get(MouseModifier::Ctrl), None);
    }

    #[test]
    fn assigning_button_clears_duplicate_modifier() {
        let mut bindings = MouseModifierBindings::default();
        bindings.set(MouseModifier::Ctrl, Some(ExtraMouseButton::Mouse4));
        bindings.set(MouseModifier::Shift, Some(ExtraMouseButton::Mouse4));

        assert_eq!(bindings.get(MouseModifier::Ctrl), None);
        assert_eq!(
            bindings.get(MouseModifier::Shift),
            Some(ExtraMouseButton::Mouse4)
        );
        assert_eq!(
            bindings.modifier_for_button(ExtraMouseButton::Mouse4),
            Some(MouseModifier::Shift)
        );
    }

    #[test]
    fn serde_roundtrip() {
        let mut bindings = MouseModifierBindings::default();
        bindings.set(MouseModifier::Ctrl, Some(ExtraMouseButton::Mouse4));
        bindings.set(MouseModifier::Shift, Some(ExtraMouseButton::Mouse5));

        let json = serde_json::to_string(&bindings).unwrap();
        let deserialized: MouseModifierBindings = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized, bindings);
    }

    #[test]
    fn missing_fields_default_unbound() {
        let deserialized: MouseModifierBindings = serde_json::from_str("{}").unwrap();
        assert_eq!(deserialized, MouseModifierBindings::default());
    }

    #[test]
    fn converts_only_extra_sdl_buttons() {
        assert_eq!(
            ExtraMouseButton::from_sdl2(SdlMouseButton::X1),
            Some(ExtraMouseButton::Mouse4)
        );
        assert_eq!(
            ExtraMouseButton::from_sdl2(SdlMouseButton::X2),
            Some(ExtraMouseButton::Mouse5)
        );
        assert_eq!(ExtraMouseButton::from_sdl2(SdlMouseButton::Left), None);
    }
}
