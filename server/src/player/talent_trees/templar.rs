//! Placeholder effect table for the Templar talent tree.
//!
//! Structural metadata lives in `core::talent_trees::templar`; this table
//! gives those shared node ids a distinct defensive/front-line effect profile
//! on the server.

use core::skills::Attribute;
use core::talent_trees::{TalentId, templar::ids};

use super::TalentEffect;

/// `TalentId` -> `TalentEffect` lookup table for the Templar placeholder tree.
pub static TEMPLAR_TALENT_EFFECTS: &[(TalentId, TalentEffect)] = &[
    (ids::SHIELD_OATH, attribute(Attribute::Braveness, 10)),
    (ids::SACRED_FOCUS, attribute(Attribute::Willpower, 8)),
    (ids::BULWARK_1, attribute(Attribute::Agility, 6)),
    (ids::RADIANT_STRIKE_1, attribute(Attribute::Willpower, 10)),
    (ids::BULWARK_2, attribute(Attribute::Agility, 8)),
    (ids::RADIANT_STRIKE_2, attribute(Attribute::Willpower, 12)),
    (ids::GUARDING_STEP_1, attribute(Attribute::Strength, 8)),
    (ids::WRATH_1, attribute(Attribute::Strength, 12)),
    (ids::AEGIS, attribute(Attribute::Braveness, 12)),
    (ids::JUDGMENT, attribute(Attribute::Strength, 16)),
    (ids::GUARDING_STEP_2, attribute(Attribute::Strength, 10)),
    (ids::WRATH_2, attribute(Attribute::Strength, 16)),
    (ids::SANCTUARY_1, attribute(Attribute::Willpower, 12)),
    (ids::RESOLVE_1, attribute(Attribute::Braveness, 12)),
    (ids::SANCTUARY_2, attribute(Attribute::Willpower, 16)),
    (ids::RESOLVE_2, attribute(Attribute::Braveness, 16)),
    (ids::BASTION, attribute(Attribute::Agility, 10)),
    (ids::CONSECRATION, attribute(Attribute::Willpower, 14)),
    (ids::STRENGTH_OF_FAITH_1, attribute(Attribute::Strength, 12)),
    (ids::WISDOM_OF_FAITH_1, attribute(Attribute::Intuition, 8)),
    (ids::STRENGTH_OF_FAITH_2, attribute(Attribute::Strength, 14)),
    (ids::WISDOM_OF_FAITH_2, attribute(Attribute::Intuition, 10)),
    (ids::OATHBOUND_PARAGON, attribute(Attribute::Braveness, 25)),
];

/// Build an attribute-percent placeholder effect.
const fn attribute(attr: Attribute, percent: i32) -> TalentEffect {
    TalentEffect::AttributePercent { attr, percent }
}
