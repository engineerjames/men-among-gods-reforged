//! Placeholder effect table for the Harakim talent tree.
//!
//! Structural metadata lives in `core::talent_trees::harakim`; this table
//! gives those shared node ids a distinct caster/awareness-leaning effect
//! profile on the server.

use core::skills::Attribute;
use core::talent_trees::{TalentId, harakim::ids};

use super::TalentEffect;

/// `TalentId` -> `TalentEffect` lookup table for the Harakim placeholder tree.
pub static HARAKIM_TALENT_EFFECTS: &[(TalentId, TalentEffect)] = &[
    (ids::DESERT_SENSE, attribute(Attribute::Intuition, 10)),
    (ids::MIND_SPIKE, attribute(Attribute::Willpower, 10)),
    (ids::MIRAGE_STEP_1, attribute(Attribute::Agility, 10)),
    (ids::SAND_CHANNELING_1, attribute(Attribute::Willpower, 12)),
    (ids::MIRAGE_STEP_2, attribute(Attribute::Agility, 12)),
    (ids::SAND_CHANNELING_2, attribute(Attribute::Willpower, 16)),
    (ids::SWIFT_READING_1, attribute(Attribute::Agility, 8)),
    (ids::SPIRIT_CUT_1, attribute(Attribute::Strength, 6)),
    (ids::UNMASK, attribute(Attribute::Intuition, 12)),
    (ids::SOUL_BURN, attribute(Attribute::Willpower, 14)),
    (ids::SWIFT_READING_2, attribute(Attribute::Agility, 12)),
    (ids::SPIRIT_CUT_2, attribute(Attribute::Strength, 8)),
    (ids::VEIL_1, attribute(Attribute::Willpower, 12)),
    (ids::STILLNESS_1, attribute(Attribute::Braveness, 8)),
    (ids::VEIL_2, attribute(Attribute::Willpower, 16)),
    (ids::STILLNESS_2, attribute(Attribute::Braveness, 12)),
    (ids::DUST_DANCE, attribute(Attribute::Agility, 16)),
    (ids::FEVER_DREAM, attribute(Attribute::Willpower, 22)),
    (ids::STRENGTH_OF_SAND_1, attribute(Attribute::Strength, 6)),
    (ids::INSIGHT_OF_SAND_1, attribute(Attribute::Intuition, 14)),
    (ids::STRENGTH_OF_SAND_2, attribute(Attribute::Strength, 8)),
    (ids::INSIGHT_OF_SAND_2, attribute(Attribute::Intuition, 16)),
    (ids::EYE_OF_THE_STORM, attribute(Attribute::Intuition, 24)),
];

/// Build an attribute-percent placeholder effect.
const fn attribute(attr: Attribute, percent: i32) -> TalentEffect {
    TalentEffect::AttributePercent { attr, percent }
}
