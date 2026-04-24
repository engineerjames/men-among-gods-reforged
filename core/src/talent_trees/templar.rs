//! Templar class talent tree metadata and effects.

use super::{TalentEffect, TalentNode, TalentRef, TalentTree};
use crate::skills::Attribute;
use crate::traits::Class;

const SHIELD_OATH: TalentRef = TalentRef {
    layer: 1,
    mask: 0b0000_0001,
};
const SACRED_FOCUS: TalentRef = TalentRef {
    layer: 1,
    mask: 0b0000_0010,
};
const BULWARK_1: TalentRef = TalentRef {
    layer: 2,
    mask: 0b0000_0001,
};
const RADIANT_STRIKE_1: TalentRef = TalentRef {
    layer: 2,
    mask: 0b0000_0010,
};
const BULWARK_2: TalentRef = TalentRef {
    layer: 3,
    mask: 0b0000_0001,
};
const RADIANT_STRIKE_2: TalentRef = TalentRef {
    layer: 3,
    mask: 0b0000_0010,
};
const GUARDING_STEP_1: TalentRef = TalentRef {
    layer: 4,
    mask: 0b0000_0001,
};
const WRATH_1: TalentRef = TalentRef {
    layer: 4,
    mask: 0b0000_0010,
};
const AEGIS: TalentRef = TalentRef {
    layer: 5,
    mask: 0b0000_0001,
};
const JUDGMENT: TalentRef = TalentRef {
    layer: 5,
    mask: 0b0000_0010,
};
const GUARDING_STEP_2: TalentRef = TalentRef {
    layer: 6,
    mask: 0b0000_0001,
};
const WRATH_2: TalentRef = TalentRef {
    layer: 6,
    mask: 0b0000_0010,
};
const SANCTUARY_1: TalentRef = TalentRef {
    layer: 7,
    mask: 0b0000_0001,
};
const RESOLVE_1: TalentRef = TalentRef {
    layer: 7,
    mask: 0b0000_0010,
};
const SANCTUARY_2: TalentRef = TalentRef {
    layer: 8,
    mask: 0b0000_0001,
};
const RESOLVE_2: TalentRef = TalentRef {
    layer: 8,
    mask: 0b0000_0010,
};
const BASTION: TalentRef = TalentRef {
    layer: 9,
    mask: 0b0000_0001,
};
const CONSECRATION: TalentRef = TalentRef {
    layer: 9,
    mask: 0b0000_0010,
};
const STRENGTH_OF_FAITH_1: TalentRef = TalentRef {
    layer: 10,
    mask: 0b0000_0001,
};
const WISDOM_OF_FAITH_1: TalentRef = TalentRef {
    layer: 10,
    mask: 0b0000_0010,
};
const STRENGTH_OF_FAITH_2: TalentRef = TalentRef {
    layer: 11,
    mask: 0b0000_0001,
};
const WISDOM_OF_FAITH_2: TalentRef = TalentRef {
    layer: 11,
    mask: 0b0000_0010,
};
const OATHBOUND_PARAGON: TalentRef = TalentRef {
    layer: 12,
    mask: 0b0000_0001,
};

/// The full Templar placeholder talent tree.
pub static TEMPLAR_TREE: TalentTree = TalentTree {
    class: Class::Templar,
    nodes: &[
        TalentNode {
            slot: SHIELD_OATH,
            name: "Shield Oath",
            description: "Root defensive vow for the Templar path.",
            cost: 1,
            prereqs: &[],
            effect: TalentEffect::AttributePercent {
                attr: Attribute::Braveness,
                percent: 10,
            },
        },
        TalentNode {
            slot: SACRED_FOCUS,
            name: "Sacred Focus",
            description: "Root spell discipline for the Templar path.",
            cost: 1,
            prereqs: &[],
            effect: TalentEffect::AttributePercent {
                attr: Attribute::Willpower,
                percent: 8,
            },
        },
        TalentNode {
            slot: BULWARK_1,
            name: "Bulwark I",
            description: "Placeholder defensive training.",
            cost: 1,
            prereqs: &[SHIELD_OATH],
            effect: TalentEffect::AttributePercent {
                attr: Attribute::Agility,
                percent: 6,
            },
        },
        TalentNode {
            slot: RADIANT_STRIKE_1,
            name: "Radiant Strike I",
            description: "Placeholder offensive zeal training.",
            cost: 1,
            prereqs: &[SACRED_FOCUS],
            effect: TalentEffect::AttributePercent {
                attr: Attribute::Willpower,
                percent: 10,
            },
        },
        TalentNode {
            slot: BULWARK_2,
            name: "Bulwark II",
            description: "Further defensive training.",
            cost: 1,
            prereqs: &[BULWARK_1],
            effect: TalentEffect::AttributePercent {
                attr: Attribute::Agility,
                percent: 8,
            },
        },
        TalentNode {
            slot: RADIANT_STRIKE_2,
            name: "Radiant Strike II",
            description: "Further offensive zeal training.",
            cost: 1,
            prereqs: &[RADIANT_STRIKE_1],
            effect: TalentEffect::AttributePercent {
                attr: Attribute::Willpower,
                percent: 12,
            },
        },
        TalentNode {
            slot: GUARDING_STEP_1,
            name: "Guarding Step I",
            description: "Placeholder control of battlefield positioning.",
            cost: 1,
            prereqs: &[BULWARK_2],
            effect: TalentEffect::AttributePercent {
                attr: Attribute::Strength,
                percent: 8,
            },
        },
        TalentNode {
            slot: WRATH_1,
            name: "Wrath I",
            description: "Placeholder righteous damage improvement.",
            cost: 1,
            prereqs: &[RADIANT_STRIKE_2],
            effect: TalentEffect::AttributePercent {
                attr: Attribute::Strength,
                percent: 12,
            },
        },
        TalentNode {
            slot: AEGIS,
            name: "Aegis",
            description: "Placeholder protective active talent.",
            cost: 1,
            prereqs: &[GUARDING_STEP_1],
            effect: TalentEffect::AttributePercent {
                attr: Attribute::Braveness,
                percent: 12,
            },
        },
        TalentNode {
            slot: JUDGMENT,
            name: "Judgment",
            description: "Placeholder finishing talent.",
            cost: 1,
            prereqs: &[WRATH_1],
            effect: TalentEffect::AttributePercent {
                attr: Attribute::Strength,
                percent: 16,
            },
        },
        TalentNode {
            slot: GUARDING_STEP_2,
            name: "Guarding Step II",
            description: "Advanced positioning discipline.",
            cost: 1,
            prereqs: &[AEGIS],
            effect: TalentEffect::AttributePercent {
                attr: Attribute::Strength,
                percent: 10,
            },
        },
        TalentNode {
            slot: WRATH_2,
            name: "Wrath II",
            description: "Advanced righteous damage improvement.",
            cost: 1,
            prereqs: &[JUDGMENT],
            effect: TalentEffect::AttributePercent {
                attr: Attribute::Strength,
                percent: 16,
            },
        },
        TalentNode {
            slot: SANCTUARY_1,
            name: "Sanctuary I",
            description: "Placeholder party protection improvement.",
            cost: 1,
            prereqs: &[GUARDING_STEP_2],
            effect: TalentEffect::AttributePercent {
                attr: Attribute::Willpower,
                percent: 12,
            },
        },
        TalentNode {
            slot: RESOLVE_1,
            name: "Resolve I",
            description: "Placeholder resistance improvement.",
            cost: 1,
            prereqs: &[WRATH_2],
            effect: TalentEffect::AttributePercent {
                attr: Attribute::Braveness,
                percent: 12,
            },
        },
        TalentNode {
            slot: SANCTUARY_2,
            name: "Sanctuary II",
            description: "Further party protection improvement.",
            cost: 1,
            prereqs: &[SANCTUARY_1],
            effect: TalentEffect::AttributePercent {
                attr: Attribute::Willpower,
                percent: 16,
            },
        },
        TalentNode {
            slot: RESOLVE_2,
            name: "Resolve II",
            description: "Further resistance improvement.",
            cost: 1,
            prereqs: &[RESOLVE_1],
            effect: TalentEffect::AttributePercent {
                attr: Attribute::Braveness,
                percent: 16,
            },
        },
        TalentNode {
            slot: BASTION,
            name: "Bastion",
            description: "Placeholder defensive capstone branch.",
            cost: 1,
            prereqs: &[SANCTUARY_2],
            effect: TalentEffect::AttributePercent {
                attr: Attribute::Agility,
                percent: 10,
            },
        },
        TalentNode {
            slot: CONSECRATION,
            name: "Consecration",
            description: "Placeholder sacred ground branch.",
            cost: 1,
            prereqs: &[RESOLVE_2],
            effect: TalentEffect::AttributePercent {
                attr: Attribute::Willpower,
                percent: 14,
            },
        },
        TalentNode {
            slot: STRENGTH_OF_FAITH_1,
            name: "Strength of Faith I",
            description: "Increase strength through discipline.",
            cost: 1,
            prereqs: &[BASTION],
            effect: TalentEffect::AttributePercent {
                attr: Attribute::Strength,
                percent: 12,
            },
        },
        TalentNode {
            slot: WISDOM_OF_FAITH_1,
            name: "Wisdom of Faith I",
            description: "Increase intuition through discipline.",
            cost: 1,
            prereqs: &[CONSECRATION],
            effect: TalentEffect::AttributePercent {
                attr: Attribute::Intuition,
                percent: 8,
            },
        },
        TalentNode {
            slot: STRENGTH_OF_FAITH_2,
            name: "Strength of Faith II",
            description: "Further increase strength through discipline.",
            cost: 1,
            prereqs: &[STRENGTH_OF_FAITH_1],
            effect: TalentEffect::AttributePercent {
                attr: Attribute::Strength,
                percent: 14,
            },
        },
        TalentNode {
            slot: WISDOM_OF_FAITH_2,
            name: "Wisdom of Faith II",
            description: "Further increase intuition through discipline.",
            cost: 1,
            prereqs: &[WISDOM_OF_FAITH_1],
            effect: TalentEffect::AttributePercent {
                attr: Attribute::Intuition,
                percent: 10,
            },
        },
        TalentNode {
            slot: OATHBOUND_PARAGON,
            name: "Oathbound Paragon",
            description: "Capstone: unite Templar defense and zeal.",
            cost: 1,
            prereqs: &[STRENGTH_OF_FAITH_2, WISDOM_OF_FAITH_2],
            effect: TalentEffect::AttributePercent {
                attr: Attribute::Braveness,
                percent: 25,
            },
        },
    ],
};
