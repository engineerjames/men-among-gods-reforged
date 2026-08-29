//! Mouse binding types for extra-button modifier shortcuts.
//!
//! This module contains the small persisted model used by the settings panel
//! and game scene to let extra mouse buttons act like keyboard modifiers while
//! they are held.

use std::fmt;

use sdl2::mouse::MouseButton as SdlMouseButton;
use serde::{Deserialize, Serialize};

/// Number of extra mouse buttons supported by the Mouse Settings panel.
pub const EXTRA_MOUSE_BUTTON_COUNT: usize = 6;

/// Lowest raw SDL button index that `sdl2::mouse::MouseButton` cannot express.
///
/// SDL numbers buttons 1..=5 as left, middle, right, X1 and X2. Anything above
/// that is reported as `MouseButton::Unknown` by the Rust bindings, so those
/// buttons have to be detected from the raw button-state bitmask instead.
pub const FIRST_HIGH_RAW_BUTTON_INDEX: u8 = 6;

/// Highest raw SDL button index polled from the raw button-state bitmask.
pub const LAST_HIGH_RAW_BUTTON_INDEX: u8 = 8;

/// Extra mouse buttons that can be bound to modifier behavior.
///
/// Left and right buttons are deliberately excluded because the game world and
/// UI already rely on them. Every other button SDL can report is bindable so
/// that players are not limited to a specific mouse model.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ExtraMouseButton {
    /// Middle button / wheel click, reported by SDL as raw button 2.
    Mouse3,
    /// First side/auxiliary mouse button, reported by SDL as X1.
    Mouse4,
    /// Second side/auxiliary mouse button, reported by SDL as X2.
    Mouse5,
    /// Additional button, reported by SDL as raw button 6.
    Mouse6,
    /// Additional button, reported by SDL as raw button 7.
    Mouse7,
    /// Additional button, reported by SDL as raw button 8.
    Mouse8,
}

impl ExtraMouseButton {
    /// All supported extra mouse buttons in UI display order.
    pub const ALL: [ExtraMouseButton; EXTRA_MOUSE_BUTTON_COUNT] = [
        Self::Mouse3,
        Self::Mouse4,
        Self::Mouse5,
        Self::Mouse6,
        Self::Mouse7,
        Self::Mouse8,
    ];

    /// Returns a short display label for this button.
    ///
    /// # Returns
    ///
    /// * Human-readable button label.
    pub fn label(self) -> &'static str {
        match self {
            Self::Mouse3 => "Mouse 3",
            Self::Mouse4 => "Mouse 4",
            Self::Mouse5 => "Mouse 5",
            Self::Mouse6 => "Mouse 6",
            Self::Mouse7 => "Mouse 7",
            Self::Mouse8 => "Mouse 8",
        }
    }

    /// Returns the raw SDL button index for this button.
    ///
    /// # Returns
    ///
    /// * SDL button index, where 1 is the left button.
    pub fn raw_index(self) -> u8 {
        match self {
            Self::Mouse3 => 2,
            Self::Mouse4 => 4,
            Self::Mouse5 => 5,
            Self::Mouse6 => 6,
            Self::Mouse7 => 7,
            Self::Mouse8 => 8,
        }
    }

    /// Returns the raw SDL button-state bitmask bit for this button.
    ///
    /// # Returns
    ///
    /// * Single-bit mask matching `SDL_BUTTON(raw_index)`.
    pub fn state_mask(self) -> u32 {
        1u32 << (self.raw_index() - 1)
    }

    /// Converts an SDL mouse button into an extra mouse button binding value.
    ///
    /// # Arguments
    ///
    /// * `button` - SDL mouse button from a raw mouse-button event.
    ///
    /// # Returns
    ///
    /// * `Some` for middle/X1/X2, otherwise `None`.
    pub fn from_sdl2(button: SdlMouseButton) -> Option<Self> {
        match button {
            SdlMouseButton::Middle => Some(Self::Mouse3),
            SdlMouseButton::X1 => Some(Self::Mouse4),
            SdlMouseButton::X2 => Some(Self::Mouse5),
            _ => None,
        }
    }

    /// Converts a raw SDL button index into an extra mouse button value.
    ///
    /// # Arguments
    ///
    /// * `index` - Raw SDL button index, where 1 is the left button.
    ///
    /// # Returns
    ///
    /// * `Some` for bindable buttons, otherwise `None`.
    pub fn from_raw_index(index: u8) -> Option<Self> {
        Self::ALL.into_iter().find(|b| b.raw_index() == index)
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
    /// Keyboard Alt modifier behavior.
    Alt,
}

impl MouseModifier {
    /// All supported modifier targets in UI display order.
    pub const ALL: [MouseModifier; 3] = [Self::Ctrl, Self::Shift, Self::Alt];

    /// Returns a short display label for this modifier.
    ///
    /// # Returns
    ///
    /// * Human-readable modifier label.
    pub fn label(self) -> &'static str {
        match self {
            Self::Ctrl => "Ctrl",
            Self::Shift => "Shift",
            Self::Alt => "Alt",
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
    /// Extra mouse button assigned to Alt behavior.
    #[serde(default)]
    alt: Option<ExtraMouseButton>,
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
            MouseModifier::Alt => self.alt,
        }
    }

    /// Sets or clears the button assigned to a modifier.
    ///
    /// If `button` is already assigned to another modifier, that other
    /// assignment is cleared so one button cannot trigger two modifiers.
    ///
    /// # Arguments
    ///
    /// * `modifier` - Modifier target to change.
    /// * `button` - Extra mouse button to bind, or `None` to clear.
    pub fn set(&mut self, modifier: MouseModifier, button: Option<ExtraMouseButton>) {
        if button.is_some() {
            for other in MouseModifier::ALL {
                if other != modifier && self.get(other) == button {
                    self.clear(other);
                }
            }
        }

        match modifier {
            MouseModifier::Ctrl => self.ctrl = button,
            MouseModifier::Shift => self.shift = button,
            MouseModifier::Alt => self.alt = button,
        }
    }

    /// Clears the button assigned to a modifier.
    ///
    /// # Arguments
    ///
    /// * `modifier` - Modifier target to clear.
    fn clear(&mut self, modifier: MouseModifier) {
        match modifier {
            MouseModifier::Ctrl => self.ctrl = None,
            MouseModifier::Shift => self.shift = None,
            MouseModifier::Alt => self.alt = None,
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
        MouseModifier::ALL
            .into_iter()
            .find(|&modifier| self.get(modifier) == Some(button))
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
        assert_eq!(bindings.get(MouseModifier::Alt), None);
    }

    #[test]
    fn labels_are_stable() {
        assert_eq!(ExtraMouseButton::Mouse3.label(), "Mouse 3");
        assert_eq!(ExtraMouseButton::Mouse4.label(), "Mouse 4");
        assert_eq!(ExtraMouseButton::Mouse5.label(), "Mouse 5");
        assert_eq!(ExtraMouseButton::Mouse8.label(), "Mouse 8");
        assert_eq!(MouseModifier::Ctrl.label(), "Ctrl");
        assert_eq!(MouseModifier::Shift.label(), "Shift");
        assert_eq!(MouseModifier::Alt.label(), "Alt");
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

        bindings.set(MouseModifier::Alt, Some(ExtraMouseButton::Mouse4));
        assert_eq!(bindings.get(MouseModifier::Shift), None);
        assert_eq!(
            bindings.modifier_for_button(ExtraMouseButton::Mouse4),
            Some(MouseModifier::Alt)
        );
    }

    #[test]
    fn serde_roundtrip() {
        let mut bindings = MouseModifierBindings::default();
        bindings.set(MouseModifier::Ctrl, Some(ExtraMouseButton::Mouse4));
        bindings.set(MouseModifier::Alt, Some(ExtraMouseButton::Mouse5));

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
            ExtraMouseButton::from_sdl2(SdlMouseButton::Middle),
            Some(ExtraMouseButton::Mouse3)
        );
        assert_eq!(
            ExtraMouseButton::from_sdl2(SdlMouseButton::X1),
            Some(ExtraMouseButton::Mouse4)
        );
        assert_eq!(
            ExtraMouseButton::from_sdl2(SdlMouseButton::X2),
            Some(ExtraMouseButton::Mouse5)
        );
        assert_eq!(ExtraMouseButton::from_sdl2(SdlMouseButton::Left), None);
        assert_eq!(ExtraMouseButton::from_sdl2(SdlMouseButton::Right), None);
    }

    #[test]
    fn converts_raw_button_indices() {
        assert_eq!(ExtraMouseButton::from_raw_index(1), None);
        assert_eq!(ExtraMouseButton::from_raw_index(3), None);
        assert_eq!(
            ExtraMouseButton::from_raw_index(2),
            Some(ExtraMouseButton::Mouse3)
        );
        assert_eq!(
            ExtraMouseButton::from_raw_index(6),
            Some(ExtraMouseButton::Mouse6)
        );
        assert_eq!(
            ExtraMouseButton::from_raw_index(8),
            Some(ExtraMouseButton::Mouse8)
        );
        assert_eq!(ExtraMouseButton::from_raw_index(9), None);
    }

    #[test]
    fn state_masks_match_sdl_button_bits() {
        assert_eq!(ExtraMouseButton::Mouse3.state_mask(), 1 << 1);
        assert_eq!(ExtraMouseButton::Mouse4.state_mask(), 1 << 3);
        assert_eq!(ExtraMouseButton::Mouse5.state_mask(), 1 << 4);
        assert_eq!(ExtraMouseButton::Mouse8.state_mask(), 1 << 7);
    }

    #[test]
    fn high_raw_button_range_maps_to_buttons() {
        for index in FIRST_HIGH_RAW_BUTTON_INDEX..=LAST_HIGH_RAW_BUTTON_INDEX {
            assert!(
                ExtraMouseButton::from_raw_index(index).is_some(),
                "raw index {index} should be bindable"
            );
        }
    }
}
