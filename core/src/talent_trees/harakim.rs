//! Harakim class talent tree (metadata only - no effects).
//!
//! Effects are dispatched by the server via a parallel id->effect table
//! in `server/src/player/talent_trees/harakim.rs`.

use super::{TalentId, TalentNodeMeta, TalentRef, TalentTreeMeta};
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

/// Stable ids for every Harakim node.
pub mod ids {
    use super::TalentId;

    /// Root awareness node.
    pub const DESERT_SENSE: TalentId = TalentId(0x2101);
    /// Root mind magic node.
    pub const MIND_SPIKE: TalentId = TalentId(0x2102);
    /// First mirage step node.
    pub const MIRAGE_STEP_1: TalentId = TalentId(0x2201);
    /// First sand channeling node.
    pub const SAND_CHANNELING_1: TalentId = TalentId(0x2202);
    /// Second mirage step node.
    pub const MIRAGE_STEP_2: TalentId = TalentId(0x2301);
    /// Second sand channeling node.
    pub const SAND_CHANNELING_2: TalentId = TalentId(0x2302);
    /// First swift reading node.
    pub const SWIFT_READING_1: TalentId = TalentId(0x2401);
    /// First spirit cut node.
    pub const SPIRIT_CUT_1: TalentId = TalentId(0x2402);
    /// Unmask placeholder node.
    pub const UNMASK: TalentId = TalentId(0x2501);
    /// Soul burn placeholder node.
    pub const SOUL_BURN: TalentId = TalentId(0x2502);
    /// Second swift reading node.
    pub const SWIFT_READING_2: TalentId = TalentId(0x2601);
    /// Second spirit cut node.
    pub const SPIRIT_CUT_2: TalentId = TalentId(0x2602);
    /// First veil node.
    pub const VEIL_1: TalentId = TalentId(0x2701);
    /// First stillness node.
    pub const STILLNESS_1: TalentId = TalentId(0x2702);
    /// Second veil node.
    pub const VEIL_2: TalentId = TalentId(0x2801);
    /// Second stillness node.
    pub const STILLNESS_2: TalentId = TalentId(0x2802);
    /// Dust dance placeholder node.
    pub const DUST_DANCE: TalentId = TalentId(0x2901);
    /// Fever dream placeholder node.
    pub const FEVER_DREAM: TalentId = TalentId(0x2902);
    /// First strength of sand node.
    pub const STRENGTH_OF_SAND_1: TalentId = TalentId(0x2A01);
    /// First insight of sand node.
    pub const INSIGHT_OF_SAND_1: TalentId = TalentId(0x2A02);
    /// Second strength of sand node.
    pub const STRENGTH_OF_SAND_2: TalentId = TalentId(0x2B01);
    /// Second insight of sand node.
    pub const INSIGHT_OF_SAND_2: TalentId = TalentId(0x2B02);
    /// Harakim capstone node.
    pub const EYE_OF_THE_STORM: TalentId = TalentId(0x2C01);
}

const fn node(
    id: TalentId,
    slot: TalentRef,
    name: &'static str,
    description: &'static str,
    prereqs: &'static [TalentRef],
) -> TalentNodeMeta {
    TalentNodeMeta {
        id,
        layer: slot.layer,
        mask: slot.mask,
        name,
        description,
        cost: 1,
        prereqs,
    }
}

/// The full Harakim placeholder talent tree.
pub static HARAKIM_TREE: TalentTreeMeta = TalentTreeMeta {
    class: Class::Harakim,
    nodes: &[
        node(
            ids::DESERT_SENSE,
            DESERT_SENSE,
            "Desert Sense",
            "Root awareness talent for the Harakim path.",
            &[],
        ),
        node(
            ids::MIND_SPIKE,
            MIND_SPIKE,
            "Mind Spike",
            "Root will-focused talent for the Harakim path.",
            &[],
        ),
        node(
            ids::MIRAGE_STEP_1,
            MIRAGE_STEP_1,
            "Mirage Step I",
            "Placeholder movement through misdirection.",
            &[DESERT_SENSE],
        ),
        node(
            ids::SAND_CHANNELING_1,
            SAND_CHANNELING_1,
            "Sand Channeling I",
            "Placeholder spell channeling discipline.",
            &[MIND_SPIKE],
        ),
        node(
            ids::MIRAGE_STEP_2,
            MIRAGE_STEP_2,
            "Mirage Step II",
            "Further movement through misdirection.",
            &[MIRAGE_STEP_1],
        ),
        node(
            ids::SAND_CHANNELING_2,
            SAND_CHANNELING_2,
            "Sand Channeling II",
            "Further spell channeling discipline.",
            &[SAND_CHANNELING_1],
        ),
        node(
            ids::SWIFT_READING_1,
            SWIFT_READING_1,
            "Swift Reading I",
            "Placeholder tactical reading improvement.",
            &[MIRAGE_STEP_2],
        ),
        node(
            ids::SPIRIT_CUT_1,
            SPIRIT_CUT_1,
            "Spirit Cut I",
            "Placeholder focused strike improvement.",
            &[SAND_CHANNELING_2],
        ),
        node(
            ids::UNMASK,
            UNMASK,
            "Unmask",
            "Placeholder detection talent.",
            &[SWIFT_READING_1],
        ),
        node(
            ids::SOUL_BURN,
            SOUL_BURN,
            "Soul Burn",
            "Placeholder willpower attack talent.",
            &[SPIRIT_CUT_1],
        ),
        node(
            ids::SWIFT_READING_2,
            SWIFT_READING_2,
            "Swift Reading II",
            "Advanced tactical reading improvement.",
            &[UNMASK],
        ),
        node(
            ids::SPIRIT_CUT_2,
            SPIRIT_CUT_2,
            "Spirit Cut II",
            "Advanced focused strike improvement.",
            &[SOUL_BURN],
        ),
        node(
            ids::VEIL_1,
            VEIL_1,
            "Veil I",
            "Placeholder defensive illusion improvement.",
            &[SWIFT_READING_2],
        ),
        node(
            ids::STILLNESS_1,
            STILLNESS_1,
            "Stillness I",
            "Placeholder focus improvement.",
            &[SPIRIT_CUT_2],
        ),
        node(
            ids::VEIL_2,
            VEIL_2,
            "Veil II",
            "Further defensive illusion improvement.",
            &[VEIL_1],
        ),
        node(
            ids::STILLNESS_2,
            STILLNESS_2,
            "Stillness II",
            "Further focus improvement.",
            &[STILLNESS_1],
        ),
        node(
            ids::DUST_DANCE,
            DUST_DANCE,
            "Dust Dance",
            "Placeholder evasive capstone branch.",
            &[VEIL_2],
        ),
        node(
            ids::FEVER_DREAM,
            FEVER_DREAM,
            "Fever Dream",
            "Placeholder mental pressure branch.",
            &[STILLNESS_2],
        ),
        node(
            ids::STRENGTH_OF_SAND_1,
            STRENGTH_OF_SAND_1,
            "Strength of Sand I",
            "Increase strength through desert discipline.",
            &[DUST_DANCE],
        ),
        node(
            ids::INSIGHT_OF_SAND_1,
            INSIGHT_OF_SAND_1,
            "Insight of Sand I",
            "Increase intuition through desert discipline.",
            &[FEVER_DREAM],
        ),
        node(
            ids::STRENGTH_OF_SAND_2,
            STRENGTH_OF_SAND_2,
            "Strength of Sand II",
            "Further increase strength through desert discipline.",
            &[STRENGTH_OF_SAND_1],
        ),
        node(
            ids::INSIGHT_OF_SAND_2,
            INSIGHT_OF_SAND_2,
            "Insight of Sand II",
            "Further increase intuition through desert discipline.",
            &[INSIGHT_OF_SAND_1],
        ),
        node(
            ids::EYE_OF_THE_STORM,
            EYE_OF_THE_STORM,
            "Eye of the Storm",
            "Capstone: unite Harakim perception and will.",
            &[STRENGTH_OF_SAND_2, INSIGHT_OF_SAND_2],
        ),
    ],
};
