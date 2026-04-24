//! Templar class talent tree metadata and effects.

use super::{TalentEffect, TalentNodeMeta, TalentRef, TalentTreeMeta};
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

/// The full Templar placeholder talent tree.
pub static TEMPLAR_TREE: TalentTreeMeta = TalentTreeMeta {
    class: Class::Templar,
    nodes: &[
        node(
            SHIELD_OATH,
            "Shield Oath",
            "Root defensive vow for the Templar path.",
            &[],
            attribute(Attribute::Braveness, 10),
        ),
        node(
            SACRED_FOCUS,
            "Sacred Focus",
            "Root spell discipline for the Templar path.",
            &[],
            attribute(Attribute::Willpower, 8),
        ),
        node(
            BULWARK_1,
            "Bulwark I",
            "Placeholder defensive training.",
            &[SHIELD_OATH],
            attribute(Attribute::Agility, 6),
        ),
        node(
            RADIANT_STRIKE_1,
            "Radiant Strike I",
            "Placeholder offensive zeal training.",
            &[SACRED_FOCUS],
            attribute(Attribute::Willpower, 10),
        ),
        node(
            BULWARK_2,
            "Bulwark II",
            "Further defensive training.",
            &[BULWARK_1],
            attribute(Attribute::Agility, 8),
        ),
        node(
            RADIANT_STRIKE_2,
            "Radiant Strike II",
            "Further offensive zeal training.",
            &[RADIANT_STRIKE_1],
            attribute(Attribute::Willpower, 12),
        ),
        node(
            GUARDING_STEP_1,
            "Guarding Step I",
            "Placeholder control of battlefield positioning.",
            &[BULWARK_2],
            attribute(Attribute::Strength, 8),
        ),
        node(
            WRATH_1,
            "Wrath I",
            "Placeholder righteous damage improvement.",
            &[RADIANT_STRIKE_2],
            attribute(Attribute::Strength, 12),
        ),
        node(
            AEGIS,
            "Aegis",
            "Placeholder protective active talent.",
            &[GUARDING_STEP_1],
            attribute(Attribute::Braveness, 12),
        ),
        node(
            JUDGMENT,
            "Judgment",
            "Placeholder finishing talent.",
            &[WRATH_1],
            attribute(Attribute::Strength, 16),
        ),
        node(
            GUARDING_STEP_2,
            "Guarding Step II",
            "Advanced positioning discipline.",
            &[AEGIS],
            attribute(Attribute::Strength, 10),
        ),
        node(
            WRATH_2,
            "Wrath II",
            "Advanced righteous damage improvement.",
            &[JUDGMENT],
            attribute(Attribute::Strength, 16),
        ),
        node(
            SANCTUARY_1,
            "Sanctuary I",
            "Placeholder party protection improvement.",
            &[GUARDING_STEP_2],
            attribute(Attribute::Willpower, 12),
        ),
        node(
            RESOLVE_1,
            "Resolve I",
            "Placeholder resistance improvement.",
            &[WRATH_2],
            attribute(Attribute::Braveness, 12),
        ),
        node(
            SANCTUARY_2,
            "Sanctuary II",
            "Further party protection improvement.",
            &[SANCTUARY_1],
            attribute(Attribute::Willpower, 16),
        ),
        node(
            RESOLVE_2,
            "Resolve II",
            "Further resistance improvement.",
            &[RESOLVE_1],
            attribute(Attribute::Braveness, 16),
        ),
        node(
            BASTION,
            "Bastion",
            "Placeholder defensive capstone branch.",
            &[SANCTUARY_2],
            attribute(Attribute::Agility, 10),
        ),
        node(
            CONSECRATION,
            "Consecration",
            "Placeholder sacred ground branch.",
            &[RESOLVE_2],
            attribute(Attribute::Willpower, 14),
        ),
        node(
            STRENGTH_OF_FAITH_1,
            "Strength of Faith I",
            "Increase strength through discipline.",
            &[BASTION],
            attribute(Attribute::Strength, 12),
        ),
        node(
            WISDOM_OF_FAITH_1,
            "Wisdom of Faith I",
            "Increase intuition through discipline.",
            &[CONSECRATION],
            attribute(Attribute::Intuition, 8),
        ),
        node(
            STRENGTH_OF_FAITH_2,
            "Strength of Faith II",
            "Further increase strength through discipline.",
            &[STRENGTH_OF_FAITH_1],
            attribute(Attribute::Strength, 14),
        ),
        node(
            WISDOM_OF_FAITH_2,
            "Wisdom of Faith II",
            "Further increase intuition through discipline.",
            &[WISDOM_OF_FAITH_1],
            attribute(Attribute::Intuition, 10),
        ),
        node(
            OATHBOUND_PARAGON,
            "Oathbound Paragon",
            "Capstone: unite Templar defense and zeal.",
            &[STRENGTH_OF_FAITH_2, WISDOM_OF_FAITH_2],
            attribute(Attribute::Braveness, 25),
        ),
    ],
};
