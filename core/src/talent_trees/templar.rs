//! Templar class talent tree (metadata only - no effects).
//!
//! Effects are dispatched by the server via a parallel id->effect table
//! in `server/src/player/talent_trees/templar.rs`.

use super::{TalentId, TalentNodeMeta, TalentRef, TalentTreeMeta};
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

/// Stable ids for every Templar node.
pub mod ids {
    use super::TalentId;

    /// Root defensive oath.
    pub const SHIELD_OATH: TalentId = TalentId(0x1101);
    /// Root caster focus.
    pub const SACRED_FOCUS: TalentId = TalentId(0x1102);
    /// First bulwark node.
    pub const BULWARK_1: TalentId = TalentId(0x1201);
    /// First radiant strike node.
    pub const RADIANT_STRIKE_1: TalentId = TalentId(0x1202);
    /// Second bulwark node.
    pub const BULWARK_2: TalentId = TalentId(0x1301);
    /// Second radiant strike node.
    pub const RADIANT_STRIKE_2: TalentId = TalentId(0x1302);
    /// First guarding step node.
    pub const GUARDING_STEP_1: TalentId = TalentId(0x1401);
    /// First wrath node.
    pub const WRATH_1: TalentId = TalentId(0x1402);
    /// Aegis active placeholder node.
    pub const AEGIS: TalentId = TalentId(0x1501);
    /// Judgment active placeholder node.
    pub const JUDGMENT: TalentId = TalentId(0x1502);
    /// Second guarding step node.
    pub const GUARDING_STEP_2: TalentId = TalentId(0x1601);
    /// Second wrath node.
    pub const WRATH_2: TalentId = TalentId(0x1602);
    /// First sanctuary node.
    pub const SANCTUARY_1: TalentId = TalentId(0x1701);
    /// First resolve node.
    pub const RESOLVE_1: TalentId = TalentId(0x1702);
    /// Second sanctuary node.
    pub const SANCTUARY_2: TalentId = TalentId(0x1801);
    /// Second resolve node.
    pub const RESOLVE_2: TalentId = TalentId(0x1802);
    /// Bastion placeholder node.
    pub const BASTION: TalentId = TalentId(0x1901);
    /// Consecration placeholder node.
    pub const CONSECRATION: TalentId = TalentId(0x1902);
    /// First strength of faith node.
    pub const STRENGTH_OF_FAITH_1: TalentId = TalentId(0x1A01);
    /// First wisdom of faith node.
    pub const WISDOM_OF_FAITH_1: TalentId = TalentId(0x1A02);
    /// Second strength of faith node.
    pub const STRENGTH_OF_FAITH_2: TalentId = TalentId(0x1B01);
    /// Second wisdom of faith node.
    pub const WISDOM_OF_FAITH_2: TalentId = TalentId(0x1B02);
    /// Templar capstone node.
    pub const OATHBOUND_PARAGON: TalentId = TalentId(0x1C01);
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

/// The full Templar placeholder talent tree.
pub static TEMPLAR_TREE: TalentTreeMeta = TalentTreeMeta {
    class: Class::Templar,
    nodes: &[
        node(
            ids::SHIELD_OATH,
            SHIELD_OATH,
            "Shield Oath",
            "Root defensive vow for the Templar path.",
            &[],
        ),
        node(
            ids::SACRED_FOCUS,
            SACRED_FOCUS,
            "Sacred Focus",
            "Root spell discipline for the Templar path.",
            &[],
        ),
        node(
            ids::BULWARK_1,
            BULWARK_1,
            "Bulwark I",
            "Placeholder defensive training.",
            &[SHIELD_OATH],
        ),
        node(
            ids::RADIANT_STRIKE_1,
            RADIANT_STRIKE_1,
            "Radiant Strike I",
            "Placeholder offensive zeal training.",
            &[SACRED_FOCUS],
        ),
        node(
            ids::BULWARK_2,
            BULWARK_2,
            "Bulwark II",
            "Further defensive training.",
            &[BULWARK_1],
        ),
        node(
            ids::RADIANT_STRIKE_2,
            RADIANT_STRIKE_2,
            "Radiant Strike II",
            "Further offensive zeal training.",
            &[RADIANT_STRIKE_1],
        ),
        node(
            ids::GUARDING_STEP_1,
            GUARDING_STEP_1,
            "Guarding Step I",
            "Placeholder control of battlefield positioning.",
            &[BULWARK_2],
        ),
        node(
            ids::WRATH_1,
            WRATH_1,
            "Wrath I",
            "Placeholder righteous damage improvement.",
            &[RADIANT_STRIKE_2],
        ),
        node(
            ids::AEGIS,
            AEGIS,
            "Aegis",
            "Placeholder protective active talent.",
            &[GUARDING_STEP_1],
        ),
        node(
            ids::JUDGMENT,
            JUDGMENT,
            "Judgment",
            "Placeholder finishing talent.",
            &[WRATH_1],
        ),
        node(
            ids::GUARDING_STEP_2,
            GUARDING_STEP_2,
            "Guarding Step II",
            "Advanced positioning discipline.",
            &[AEGIS],
        ),
        node(
            ids::WRATH_2,
            WRATH_2,
            "Wrath II",
            "Advanced righteous damage improvement.",
            &[JUDGMENT],
        ),
        node(
            ids::SANCTUARY_1,
            SANCTUARY_1,
            "Sanctuary I",
            "Placeholder party protection improvement.",
            &[GUARDING_STEP_2],
        ),
        node(
            ids::RESOLVE_1,
            RESOLVE_1,
            "Resolve I",
            "Placeholder resistance improvement.",
            &[WRATH_2],
        ),
        node(
            ids::SANCTUARY_2,
            SANCTUARY_2,
            "Sanctuary II",
            "Further party protection improvement.",
            &[SANCTUARY_1],
        ),
        node(
            ids::RESOLVE_2,
            RESOLVE_2,
            "Resolve II",
            "Further resistance improvement.",
            &[RESOLVE_1],
        ),
        node(
            ids::BASTION,
            BASTION,
            "Bastion",
            "Placeholder defensive capstone branch.",
            &[SANCTUARY_2],
        ),
        node(
            ids::CONSECRATION,
            CONSECRATION,
            "Consecration",
            "Placeholder sacred ground branch.",
            &[RESOLVE_2],
        ),
        node(
            ids::STRENGTH_OF_FAITH_1,
            STRENGTH_OF_FAITH_1,
            "Strength of Faith I",
            "Increase strength through discipline.",
            &[BASTION],
        ),
        node(
            ids::WISDOM_OF_FAITH_1,
            WISDOM_OF_FAITH_1,
            "Wisdom of Faith I",
            "Increase intuition through discipline.",
            &[CONSECRATION],
        ),
        node(
            ids::STRENGTH_OF_FAITH_2,
            STRENGTH_OF_FAITH_2,
            "Strength of Faith II",
            "Further increase strength through discipline.",
            &[STRENGTH_OF_FAITH_1],
        ),
        node(
            ids::WISDOM_OF_FAITH_2,
            WISDOM_OF_FAITH_2,
            "Wisdom of Faith II",
            "Further increase intuition through discipline.",
            &[WISDOM_OF_FAITH_1],
        ),
        node(
            ids::OATHBOUND_PARAGON,
            OATHBOUND_PARAGON,
            "Oathbound Paragon",
            "Capstone: unite Templar defense and zeal.",
            &[STRENGTH_OF_FAITH_2, WISDOM_OF_FAITH_2],
        ),
    ],
};
