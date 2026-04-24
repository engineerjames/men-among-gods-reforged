//! Harakim class talent tree metadata and effects.

use super::{TalentEffect, TalentNodeMeta, TalentRef, TalentTreeMeta};
use crate::skills::Attribute;
use crate::traits::Class;

const DESERT_SENSE: TalentRef = TalentRef {
    layer: 1,
    mask: 0b0000_0001,
};
const MIND_SPIKE: TalentRef = TalentRef {
    layer: 1,
    mask: 0b0000_0010,
};
const MIRAGE_STEP_1: TalentRef = TalentRef {
    layer: 2,
    mask: 0b0000_0001,
};
const SAND_CHANNELING_1: TalentRef = TalentRef {
    layer: 2,
    mask: 0b0000_0010,
};
const MIRAGE_STEP_2: TalentRef = TalentRef {
    layer: 3,
    mask: 0b0000_0001,
};
const SAND_CHANNELING_2: TalentRef = TalentRef {
    layer: 3,
    mask: 0b0000_0010,
};
const SWIFT_READING_1: TalentRef = TalentRef {
    layer: 4,
    mask: 0b0000_0001,
};
const SPIRIT_CUT_1: TalentRef = TalentRef {
    layer: 4,
    mask: 0b0000_0010,
};
const UNMASK: TalentRef = TalentRef {
    layer: 5,
    mask: 0b0000_0001,
};
const SOUL_BURN: TalentRef = TalentRef {
    layer: 5,
    mask: 0b0000_0010,
};
const SWIFT_READING_2: TalentRef = TalentRef {
    layer: 6,
    mask: 0b0000_0001,
};
const SPIRIT_CUT_2: TalentRef = TalentRef {
    layer: 6,
    mask: 0b0000_0010,
};
const VEIL_1: TalentRef = TalentRef {
    layer: 7,
    mask: 0b0000_0001,
};
const STILLNESS_1: TalentRef = TalentRef {
    layer: 7,
    mask: 0b0000_0010,
};
const VEIL_2: TalentRef = TalentRef {
    layer: 8,
    mask: 0b0000_0001,
};
const STILLNESS_2: TalentRef = TalentRef {
    layer: 8,
    mask: 0b0000_0010,
};
const DUST_DANCE: TalentRef = TalentRef {
    layer: 9,
    mask: 0b0000_0001,
};
const FEVER_DREAM: TalentRef = TalentRef {
    layer: 9,
    mask: 0b0000_0010,
};
const STRENGTH_OF_SAND_1: TalentRef = TalentRef {
    layer: 10,
    mask: 0b0000_0001,
};
const INSIGHT_OF_SAND_1: TalentRef = TalentRef {
    layer: 10,
    mask: 0b0000_0010,
};
const STRENGTH_OF_SAND_2: TalentRef = TalentRef {
    layer: 11,
    mask: 0b0000_0001,
};
const INSIGHT_OF_SAND_2: TalentRef = TalentRef {
    layer: 11,
    mask: 0b0000_0010,
};
const EYE_OF_THE_STORM: TalentRef = TalentRef {
    layer: 12,
    mask: 0b0000_0001,
};

const fn node(
    slot: TalentRef,
    name: &'static str,
    description: &'static str,
    prereqs: &'static [TalentRef],
    effect: TalentEffect,
) -> TalentNodeMeta {
    TalentNodeMeta {
        slot,
        name,
        description,
        cost: 1,
        prereqs,
        effect,
    }
}

const fn attribute(attr: Attribute, percent: i32) -> TalentEffect {
    TalentEffect::AttributePercent { attr, percent }
}

/// The full Harakim placeholder talent tree.
pub static HARAKIM_TREE: TalentTreeMeta = TalentTreeMeta {
    class: Class::Harakim,
    nodes: &[
        node(
            DESERT_SENSE,
            "Desert Sense",
            "Root awareness talent for the Harakim path.",
            &[],
            attribute(Attribute::Intuition, 10),
        ),
        node(
            MIND_SPIKE,
            "Mind Spike",
            "Root will-focused talent for the Harakim path.",
            &[],
            attribute(Attribute::Willpower, 10),
        ),
        node(
            MIRAGE_STEP_1,
            "Mirage Step I",
            "Placeholder movement through misdirection.",
            &[DESERT_SENSE],
            attribute(Attribute::Agility, 10),
        ),
        node(
            SAND_CHANNELING_1,
            "Sand Channeling I",
            "Placeholder spell channeling discipline.",
            &[MIND_SPIKE],
            attribute(Attribute::Willpower, 12),
        ),
        node(
            MIRAGE_STEP_2,
            "Mirage Step II",
            "Further movement through misdirection.",
            &[MIRAGE_STEP_1],
            attribute(Attribute::Agility, 12),
        ),
        node(
            SAND_CHANNELING_2,
            "Sand Channeling II",
            "Further spell channeling discipline.",
            &[SAND_CHANNELING_1],
            attribute(Attribute::Willpower, 16),
        ),
        node(
            SWIFT_READING_1,
            "Swift Reading I",
            "Placeholder tactical reading improvement.",
            &[MIRAGE_STEP_2],
            attribute(Attribute::Agility, 8),
        ),
        node(
            SPIRIT_CUT_1,
            "Spirit Cut I",
            "Placeholder focused strike improvement.",
            &[SAND_CHANNELING_2],
            attribute(Attribute::Strength, 6),
        ),
        node(
            UNMASK,
            "Unmask",
            "Placeholder detection talent.",
            &[SWIFT_READING_1],
            attribute(Attribute::Intuition, 12),
        ),
        node(
            SOUL_BURN,
            "Soul Burn",
            "Placeholder willpower attack talent.",
            &[SPIRIT_CUT_1],
            attribute(Attribute::Willpower, 14),
        ),
        node(
            SWIFT_READING_2,
            "Swift Reading II",
            "Advanced tactical reading improvement.",
            &[UNMASK],
            attribute(Attribute::Agility, 12),
        ),
        node(
            SPIRIT_CUT_2,
            "Spirit Cut II",
            "Advanced focused strike improvement.",
            &[SOUL_BURN],
            attribute(Attribute::Strength, 8),
        ),
        node(
            VEIL_1,
            "Veil I",
            "Placeholder defensive illusion improvement.",
            &[SWIFT_READING_2],
            attribute(Attribute::Willpower, 12),
        ),
        node(
            STILLNESS_1,
            "Stillness I",
            "Placeholder focus improvement.",
            &[SPIRIT_CUT_2],
            attribute(Attribute::Braveness, 8),
        ),
        node(
            VEIL_2,
            "Veil II",
            "Further defensive illusion improvement.",
            &[VEIL_1],
            attribute(Attribute::Willpower, 16),
        ),
        node(
            STILLNESS_2,
            "Stillness II",
            "Further focus improvement.",
            &[STILLNESS_1],
            attribute(Attribute::Braveness, 12),
        ),
        node(
            DUST_DANCE,
            "Dust Dance",
            "Placeholder evasive capstone branch.",
            &[VEIL_2],
            attribute(Attribute::Agility, 16),
        ),
        node(
            FEVER_DREAM,
            "Fever Dream",
            "Placeholder mental pressure branch.",
            &[STILLNESS_2],
            attribute(Attribute::Willpower, 22),
        ),
        node(
            STRENGTH_OF_SAND_1,
            "Strength of Sand I",
            "Increase strength through desert discipline.",
            &[DUST_DANCE],
            attribute(Attribute::Strength, 6),
        ),
        node(
            INSIGHT_OF_SAND_1,
            "Insight of Sand I",
            "Increase intuition through desert discipline.",
            &[FEVER_DREAM],
            attribute(Attribute::Intuition, 14),
        ),
        node(
            STRENGTH_OF_SAND_2,
            "Strength of Sand II",
            "Further increase strength through desert discipline.",
            &[STRENGTH_OF_SAND_1],
            attribute(Attribute::Strength, 8),
        ),
        node(
            INSIGHT_OF_SAND_2,
            "Insight of Sand II",
            "Further increase intuition through desert discipline.",
            &[INSIGHT_OF_SAND_1],
            attribute(Attribute::Intuition, 16),
        ),
        node(
            EYE_OF_THE_STORM,
            "Eye of the Storm",
            "Capstone: unite Harakim perception and will.",
            &[STRENGTH_OF_SAND_2, INSIGHT_OF_SAND_2],
            attribute(Attribute::Intuition, 24),
        ),
    ],
};
