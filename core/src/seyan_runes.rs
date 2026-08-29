//! Seyan'Du rune loadout: shared metadata for the server and client.
//!
//! Runes are a Seyan'Du-only mechanic separate from the point-based talent
//! tree. All four runes are always unlocked; only one is active at a time,
//! and switching the active rune has a fixed cooldown
//! ([`RUNE_SWAP_COOLDOWN_TICKS`]). See `server/src/state/combat.rs` for the
//! on-hit proc dispatch and `client/src/ui/hud/talent_panel.rs` for the UI.

use crate::constants::TICKS;

/// Per-hit chance, out of 100, that an active spellcaster rune procs.
pub const RUNE_PROC_CHANCE_PERCENT: i32 = 15;

/// Ticks a player must wait after switching runes before switching again.
pub const RUNE_SWAP_COOLDOWN_TICKS: i32 = TICKS * 60;

/// WV/AV reduction applied per Corrosion rune proc, before capping.
pub const RUNE_CORROSION_STEP: i32 = 1;

/// Maximum total WV/AV reduction the Corrosion rune can stack to.
pub const RUNE_CORROSION_MAX: i32 = 15;

/// Flat HP restored to the attacker per Lifesteal rune proc, on top of rank.
pub const RUNE_LIFESTEAL_FLAT: i32 = 10;

/// One of the four mutually-exclusive Seyan'Du rune slots.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum SeyanRune {
    /// Chance on hit to cast a free Blast at the defender (no mana/cooldown).
    FreeBlast = 0,
    /// Chance on hit to cast a free Curse at the defender (no mana/cooldown).
    FreeCurse = 1,
    /// Every hit slightly lowers the defender's WV/AV, up to a hard cap.
    Corrosion = 2,
    /// Every hit restores HP to the attacker based on rank.
    Lifesteal = 3,
}

/// All four runes, in slot-index order.
pub const ALL: [SeyanRune; 4] = [
    SeyanRune::FreeBlast,
    SeyanRune::FreeCurse,
    SeyanRune::Corrosion,
    SeyanRune::Lifesteal,
];

impl SeyanRune {
    /// Resolves a wire/slot index into a rune.
    ///
    /// # Arguments
    ///
    /// * `index` - Rune slot index, expected in `0..=3`.
    ///
    /// # Returns
    ///
    /// * `Some(rune)` for a valid index, otherwise `None`.
    pub fn from_index(index: u8) -> Option<Self> {
        match index {
            0 => Some(SeyanRune::FreeBlast),
            1 => Some(SeyanRune::FreeCurse),
            2 => Some(SeyanRune::Corrosion),
            3 => Some(SeyanRune::Lifesteal),
            _ => None,
        }
    }

    /// Returns the wire/slot index for this rune.
    ///
    /// # Returns
    ///
    /// * Value returned by `index`.
    pub fn index(self) -> u8 {
        self as u8
    }

    /// Returns the display name shown on the talent panel.
    ///
    /// # Returns
    ///
    /// * A short `'static` label.
    pub fn name(self) -> &'static str {
        match self {
            SeyanRune::FreeBlast => "Rune of Blasting",
            SeyanRune::FreeCurse => "Rune of Cursing",
            SeyanRune::Corrosion => "Rune of Corrosion",
            SeyanRune::Lifesteal => "Rune of Renewal",
        }
    }

    /// Returns the tooltip description shown on the talent panel.
    ///
    /// # Returns
    ///
    /// * A `'static` description string.
    pub fn description(self) -> &'static str {
        match self {
            SeyanRune::FreeBlast => {
                "Chance on hit to cast a free Blast at your target, using your current Blast skill, with no mana cost or cooldown."
            }
            SeyanRune::FreeCurse => {
                "Chance on hit to cast a free Curse at your target, using your current Curse skill, with no mana cost or cooldown."
            }
            SeyanRune::Corrosion => {
                "Every hit slightly lowers your target's weapon and armor value, up to a maximum."
            }
            SeyanRune::Lifesteal => "Every hit restores HP to you based on your rank.",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_index_round_trips_through_all_slots() {
        for rune in ALL {
            assert_eq!(SeyanRune::from_index(rune.index()), Some(rune));
        }
    }

    #[test]
    fn from_index_rejects_out_of_range() {
        assert_eq!(SeyanRune::from_index(4), None);
        assert_eq!(SeyanRune::from_index(255), None);
    }
}
