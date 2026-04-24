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

/// The full Harakim placeholder talent tree.
pub static HARAKIM_TREE: TalentTreeMeta = TalentTreeMeta {
    class: Class::Harakim,
    nodes: &[
        TalentNodeMeta {
            slot: DESERT_SENSE,
            name: "Desert Sense",
            description: "Root awareness talent for the Harakim path.",
            cost: 1,
            prereqs: &[],
            effect: TalentEffect::AttributePercent {
                attr: Attribute::Intuition,
                percent: 10,
            },
        },
        TalentNodeMeta {
            slot: MIND_SPIKE,
            name: "Mind Spike",
            description: "Root will-focused talent for the Harakim path.",
            cost: 1,
            prereqs: &[],
            effect: TalentEffect::AttributePercent {
                attr: Attribute::Willpower,
                percent: 10,
            },
        },
        TalentNodeMeta {
            slot: MIRAGE_STEP_1,
            name: "Mirage Step I",
            description: "Placeholder movement through misdirection.",
            cost: 1,
            prereqs: &[DESERT_SENSE],
            effect: TalentEffect::AttributePercent {
                attr: Attribute::Agility,
                percent: 10,
            },
        },
        TalentNodeMeta {
            slot: SAND_CHANNELING_1,
            name: "Sand Channeling I",
            description: "Placeholder spell channeling discipline.",
            cost: 1,
            prereqs: &[MIND_SPIKE],
            effect: TalentEffect::AttributePercent {
                attr: Attribute::Willpower,
                percent: 12,
            },
        },
        TalentNodeMeta {
            slot: MIRAGE_STEP_2,
            name: "Mirage Step II",
            description: "Further movement through misdirection.",
            cost: 1,
            prereqs: &[MIRAGE_STEP_1],
            effect: TalentEffect::AttributePercent {
                attr: Attribute::Agility,
                percent: 12,
            },
        },
        TalentNodeMeta {
            slot: SAND_CHANNELING_2,
            name: "Sand Channeling II",
            description: "Further spell channeling discipline.",
            cost: 1,
            prereqs: &[SAND_CHANNELING_1],
            effect: TalentEffect::AttributePercent {
                attr: Attribute::Willpower,
                percent: 16,
            },
        },
        TalentNodeMeta {
            slot: SWIFT_READING_1,
            name: "Swift Reading I",
            description: "Placeholder tactical reading improvement.",
            cost: 1,
            prereqs: &[MIRAGE_STEP_2],
            effect: TalentEffect::AttributePercent {
                attr: Attribute::Agility,
                percent: 8,
            },
        },
        TalentNodeMeta {
            slot: SPIRIT_CUT_1,
            name: "Spirit Cut I",
            description: "Placeholder focused strike improvement.",
            cost: 1,
            prereqs: &[SAND_CHANNELING_2],
            effect: TalentEffect::AttributePercent {
                attr: Attribute::Strength,
                percent: 6,
            },
        },
        TalentNodeMeta {
            slot: UNMASK,
            name: "Unmask",
            description: "Placeholder detection talent.",
            cost: 1,
            prereqs: &[SWIFT_READING_1],
            effect: TalentEffect::AttributePercent {
                attr: Attribute::Intuition,
                percent: 12,
            },
        },
        TalentNodeMeta {
            slot: SOUL_BURN,
            name: "Soul Burn",
            description: "Placeholder willpower attack talent.",
            cost: 1,
            prereqs: &[SPIRIT_CUT_1],
            effect: TalentEffect::AttributePercent {
                attr: Attribute::Willpower,
                percent: 14,
            },
        },
        TalentNodeMeta {
            slot: SWIFT_READING_2,
            name: "Swift Reading II",
            description: "Advanced tactical reading improvement.",
            cost: 1,
            prereqs: &[UNMASK],
            effect: TalentEffect::AttributePercent {
                attr: Attribute::Agility,
                percent: 12,
            },
        },
        TalentNodeMeta {
            slot: SPIRIT_CUT_2,
            name: "Spirit Cut II",
            description: "Advanced focused strike improvement.",
            cost: 1,
            prereqs: &[SOUL_BURN],
            effect: TalentEffect::AttributePercent {
                attr: Attribute::Strength,
                percent: 8,
            },
        },
        TalentNodeMeta {
            slot: VEIL_1,
            name: "Veil I",
            description: "Placeholder defensive illusion improvement.",
            cost: 1,
            prereqs: &[SWIFT_READING_2],
            effect: TalentEffect::AttributePercent {
                attr: Attribute::Willpower,
                percent: 12,
            },
        },
        TalentNodeMeta {
            slot: STILLNESS_1,
            name: "Stillness I",
            description: "Placeholder focus improvement.",
            cost: 1,
            prereqs: &[SPIRIT_CUT_2],
            effect: TalentEffect::AttributePercent {
                attr: Attribute::Braveness,
                percent: 8,
            },
        },
        TalentNodeMeta {
            slot: VEIL_2,
            name: "Veil II",
            description: "Further defensive illusion improvement.",
            cost: 1,
            prereqs: &[VEIL_1],
            effect: TalentEffect::AttributePercent {
                attr: Attribute::Willpower,
                percent: 16,
            },
        },
        TalentNodeMeta {
            slot: STILLNESS_2,
            name: "Stillness II",
            description: "Further focus improvement.",
            cost: 1,
            prereqs: &[STILLNESS_1],
            effect: TalentEffect::AttributePercent {
                attr: Attribute::Braveness,
                percent: 12,
            },
        },
        TalentNodeMeta {
            slot: DUST_DANCE,
            name: "Dust Dance",
            description: "Placeholder evasive capstone branch.",
            cost: 1,
            prereqs: &[VEIL_2],
            effect: TalentEffect::AttributePercent {
                attr: Attribute::Agility,
                percent: 16,
            },
        },
        TalentNodeMeta {
            slot: FEVER_DREAM,
            name: "Fever Dream",
            description: "Placeholder mental pressure branch.",
            cost: 1,
            prereqs: &[STILLNESS_2],
            effect: TalentEffect::AttributePercent {
                attr: Attribute::Willpower,
                percent: 22,
            },
        },
        TalentNodeMeta {
            slot: STRENGTH_OF_SAND_1,
            name: "Strength of Sand I",
            description: "Increase strength through desert discipline.",
            cost: 1,
            prereqs: &[DUST_DANCE],
            effect: TalentEffect::AttributePercent {
                attr: Attribute::Strength,
                percent: 6,
            },
        },
        TalentNodeMeta {
            slot: INSIGHT_OF_SAND_1,
            name: "Insight of Sand I",
            description: "Increase intuition through desert discipline.",
            cost: 1,
            prereqs: &[FEVER_DREAM],
            effect: TalentEffect::AttributePercent {
                attr: Attribute::Intuition,
                percent: 14,
            },
        },
        TalentNodeMeta {
            slot: STRENGTH_OF_SAND_2,
            name: "Strength of Sand II",
            description: "Further increase strength through desert discipline.",
            cost: 1,
            prereqs: &[STRENGTH_OF_SAND_1],
            effect: TalentEffect::AttributePercent {
                attr: Attribute::Strength,
                percent: 8,
            },
        },
        TalentNodeMeta {
            slot: INSIGHT_OF_SAND_2,
            name: "Insight of Sand II",
            description: "Further increase intuition through desert discipline.",
            cost: 1,
            prereqs: &[INSIGHT_OF_SAND_1],
            effect: TalentEffect::AttributePercent {
                attr: Attribute::Intuition,
                percent: 16,
            },
        },
        TalentNodeMeta {
            slot: EYE_OF_THE_STORM,
            name: "Eye of the Storm",
            description: "Capstone: unite Harakim perception and will.",
            cost: 1,
            prereqs: &[STRENGTH_OF_SAND_2, INSIGHT_OF_SAND_2],
            effect: TalentEffect::AttributePercent {
                attr: Attribute::Intuition,
                percent: 24,
            },
        },
    ],
};
