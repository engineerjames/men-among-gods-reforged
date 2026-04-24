//! Placeholder effect table for the Seyan-Du talent tree.
//!
//! Structural metadata lives in `core::talent_trees::seyan_du`; this table
//! gives those shared node ids a distinct balanced veteran effect profile on
//! the server.

use core::skills::Attribute;
use core::talent_trees::{TalentId, seyan_du::ids};

use super::TalentEffect;

/// `TalentId` -> `TalentEffect` lookup table for the Seyan-Du placeholder tree.
pub static SEYAN_DU_TALENT_EFFECTS: &[(TalentId, TalentEffect)] = &[
    (ids::VETERANS_POISE, attribute(Attribute::Braveness, 10)),
    (ids::DRAGON_PULSE, attribute(Attribute::Intuition, 10)),
    (ids::EVASION_DRILL_1, attribute(Attribute::Agility, 10)),
    (ids::BATTLE_CHANNEL_1, attribute(Attribute::Willpower, 10)),
    (ids::EVASION_DRILL_2, attribute(Attribute::Agility, 12)),
    (ids::BATTLE_CHANNEL_2, attribute(Attribute::Willpower, 12)),
    (ids::FLOWING_STRIKE_1, attribute(Attribute::Agility, 12)),
    (ids::HEAVY_STRIKE_1, attribute(Attribute::Strength, 12)),
    (ids::COUNTER, attribute(Attribute::Intuition, 12)),
    (ids::FINAL_LESSON, attribute(Attribute::Strength, 14)),
    (ids::FLOWING_STRIKE_2, attribute(Attribute::Agility, 14)),
    (ids::HEAVY_STRIKE_2, attribute(Attribute::Strength, 14)),
    (ids::GUARDED_FOCUS_1, attribute(Attribute::Braveness, 10)),
    (ids::IRON_BREATH_1, attribute(Attribute::Willpower, 10)),
    (ids::GUARDED_FOCUS_2, attribute(Attribute::Braveness, 14)),
    (ids::IRON_BREATH_2, attribute(Attribute::Willpower, 14)),
    (ids::STORM_FORM, attribute(Attribute::Agility, 18)),
    (ids::BLOOD_ECHO, attribute(Attribute::Intuition, 18)),
    (
        ids::STRENGTH_DISCIPLINE_1,
        attribute(Attribute::Strength, 10),
    ),
    (ids::MIND_DISCIPLINE_1, attribute(Attribute::Intuition, 10)),
    (
        ids::STRENGTH_DISCIPLINE_2,
        attribute(Attribute::Strength, 12),
    ),
    (ids::MIND_DISCIPLINE_2, attribute(Attribute::Intuition, 12)),
    (ids::MASTER_OF_FORMS, attribute(Attribute::Braveness, 22)),
];

/// Build an attribute-percent placeholder effect.
const fn attribute(attr: Attribute, percent: i32) -> TalentEffect {
    TalentEffect::AttributePercent { attr, percent }
}
