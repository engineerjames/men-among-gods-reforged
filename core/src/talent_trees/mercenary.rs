//! Mercenary class talent tree (metadata only — no effects).
//!
//! Effects are dispatched by the server via a parallel id→effect table
//! in `server/src/player/talent_trees/mercenary.rs`.

use super::{TalentId, TalentNodeMeta, TalentRef, TalentTreeMeta};
use crate::traits::Class;

// ---- TalentRefs (used both as "this node's slot" and as prereqs) ----

// Layer 1 — root (no-prerequisites)
const DISTRACT: TalentRef = TalentRef {
    layer: 1,
    mask: 0b0000_0001,
};

const PARASITE: TalentRef = TalentRef {
    layer: 1,
    mask: 0b0000_0010,
};

// Layer 2
const DODGE_BOOST_1: TalentRef = TalentRef {
    layer: 2,
    mask: 0b0000_0001,
};

const SPELL_BOOST_1: TalentRef = TalentRef {
    layer: 2,
    mask: 0b0000_0010,
};

// Layer 3
const DODGE_BOOST_2: TalentRef = TalentRef {
    layer: 3,
    mask: 0b0000_0001,
};

const SPELL_BOOST_2: TalentRef = TalentRef {
    layer: 3,
    mask: 0b0000_0010,
};

// Layer 4
const ATTACK_SPEED_BOOST_1: TalentRef = TalentRef {
    layer: 4,
    mask: 0b0000_0001,
};

const DAMAGE_BOOST_1: TalentRef = TalentRef {
    layer: 4,
    mask: 0b0000_0010,
};

// Layer 5
const DISARM: TalentRef = TalentRef {
    layer: 5,
    mask: 0b0000_0001,
};

const DELIVER_DEATH: TalentRef = TalentRef {
    layer: 5,
    mask: 0b0000_0010,
};

// Layer 6
const ATTACK_SPEED_BOOST_2: TalentRef = TalentRef {
    layer: 6,
    mask: 0b0000_0001,
};

const DAMAGE_BOOST_2: TalentRef = TalentRef {
    layer: 6,
    mask: 0b0000_0010,
};

// Layer 7
const PROTECTIVE_SPELLS_BOOST_1: TalentRef = TalentRef {
    layer: 7,
    mask: 0b0000_0001,
};

const IMMUN_RESIST_BOOST_1: TalentRef = TalentRef {
    layer: 7,
    mask: 0b0000_0010,
};

// Layer 8
const PROTECTIVE_SPELLS_BOOST_2: TalentRef = TalentRef {
    layer: 8,
    mask: 0b0000_0001,
};

const IMMUN_RESIST_BOOST_2: TalentRef = TalentRef {
    layer: 8,
    mask: 0b0000_0010,
};

// Layer 9
const BLADE_DANCE: TalentRef = TalentRef {
    layer: 9,
    mask: 0b0000_0001,
};

const CONTAGION: TalentRef = TalentRef {
    layer: 9,
    mask: 0b0000_0010,
};

// Layer 10
const STRENGTH_BOOST_1: TalentRef = TalentRef {
    layer: 10,
    mask: 0b0000_0001,
};

const INTELLIGENCE_BOOST_1: TalentRef = TalentRef {
    layer: 10,
    mask: 0b0000_0010,
};

// Layer 11
const STRENGTH_BOOST_2: TalentRef = TalentRef {
    layer: 11,
    mask: 0b0000_0001,
};

const INTELLIGENCE_BOOST_2: TalentRef = TalentRef {
    layer: 11,
    mask: 0b0000_0010,
};

// Layer 12 — capstone
const ALL_SKILLS_BOOST_1: TalentRef = TalentRef {
    layer: 12,
    mask: 0b0000_0001,
};

/// Stable ids for every mercenary node.  Re-exported so the server's
/// effect table and (later) the client UI can refer to nodes without
/// hard-coding the `(layer, mask)` pair.
pub mod ids {
    use super::TalentId;

    pub const DISTRACT: TalentId = TalentId(0x0101);
    pub const PARASITE: TalentId = TalentId(0x0102);
    pub const DODGE_BOOST_1: TalentId = TalentId(0x0201);
    pub const SPELL_BOOST_1: TalentId = TalentId(0x0202);
    pub const DODGE_BOOST_2: TalentId = TalentId(0x0301);
    pub const SPELL_BOOST_2: TalentId = TalentId(0x0302);
    pub const ATTACK_SPEED_BOOST_1: TalentId = TalentId(0x0401);
    pub const DAMAGE_BOOST_1: TalentId = TalentId(0x0402);
    pub const DISARM: TalentId = TalentId(0x0501);
    pub const DELIVER_DEATH: TalentId = TalentId(0x0502);
    pub const ATTACK_SPEED_BOOST_2: TalentId = TalentId(0x0601);
    pub const DAMAGE_BOOST_2: TalentId = TalentId(0x0602);
    pub const PROTECTIVE_SPELLS_BOOST_1: TalentId = TalentId(0x0701);
    pub const IMMUN_RESIST_BOOST_1: TalentId = TalentId(0x0702);
    pub const PROTECTIVE_SPELLS_BOOST_2: TalentId = TalentId(0x0801);
    pub const IMMUN_RESIST_BOOST_2: TalentId = TalentId(0x0802);
    pub const BLADE_DANCE: TalentId = TalentId(0x0901);
    pub const CONTAGION: TalentId = TalentId(0x0902);
    pub const STRENGTH_BOOST_1: TalentId = TalentId(0x0A01);
    pub const INTELLIGENCE_BOOST_1: TalentId = TalentId(0x0A02);
    pub const STRENGTH_BOOST_2: TalentId = TalentId(0x0B01);
    pub const INTELLIGENCE_BOOST_2: TalentId = TalentId(0x0B02);
    pub const ALL_SKILLS_BOOST_1: TalentId = TalentId(0x0C01);
}

/// The full mercenary talent tree.
pub static MERCENARY_TREE: TalentTreeMeta = TalentTreeMeta {
    class: Class::Mercenary,
    nodes: &[
        TalentNodeMeta {
            id: ids::DISTRACT,
            layer: DISTRACT.layer,
            mask: DISTRACT.mask,
            name: "Distract",
            description: "Distract the enemy, reducing their accuracy.",
            cost: 1,
            prereqs: &[],
        },
        TalentNodeMeta {
            id: ids::PARASITE,
            layer: PARASITE.layer,
            mask: PARASITE.mask,
            name: "Parasite",
            description: "Infest the enemy with parasites, dealing damage over time.",
            cost: 1,
            prereqs: &[],
        },
        TalentNodeMeta {
            id: ids::DODGE_BOOST_1,
            layer: DODGE_BOOST_1.layer,
            mask: DODGE_BOOST_1.mask,
            name: "Dodge Boost I",
            description: "Increase your dodge chance by 5%.",
            cost: 1,
            prereqs: &[DISTRACT, PARASITE],
        },
        TalentNodeMeta {
            id: ids::SPELL_BOOST_1,
            layer: SPELL_BOOST_1.layer,
            mask: SPELL_BOOST_1.mask,
            name: "Spell Boost I",
            description: "Increase the potency of your offensive spells by 5%.",
            cost: 1,
            prereqs: &[DISTRACT, PARASITE],
        },
        TalentNodeMeta {
            id: ids::DODGE_BOOST_2,
            layer: DODGE_BOOST_2.layer,
            mask: DODGE_BOOST_2.mask,
            name: "Dodge Boost II",
            description: "Increase your dodge chance by an additional 5%.",
            cost: 1,
            prereqs: &[DODGE_BOOST_1],
        },
        TalentNodeMeta {
            id: ids::SPELL_BOOST_2,
            layer: SPELL_BOOST_2.layer,
            mask: SPELL_BOOST_2.mask,
            name: "Spell Boost II",
            description: "Further increase the potency of your offensive spells by 5%.",
            cost: 1,
            prereqs: &[SPELL_BOOST_1],
        },
        TalentNodeMeta {
            id: ids::ATTACK_SPEED_BOOST_1,
            layer: ATTACK_SPEED_BOOST_1.layer,
            mask: ATTACK_SPEED_BOOST_1.mask,
            name: "Attack Speed Boost I",
            description: "Increase your attack speed by 5%.",
            cost: 1,
            prereqs: &[DODGE_BOOST_2],
        },
        TalentNodeMeta {
            id: ids::DAMAGE_BOOST_1,
            layer: DAMAGE_BOOST_1.layer,
            mask: DAMAGE_BOOST_1.mask,
            name: "Damage Boost I",
            description: "Increase your melee damage by 5%.",
            cost: 1,
            prereqs: &[SPELL_BOOST_2],
        },
        TalentNodeMeta {
            id: ids::DISARM,
            layer: DISARM.layer,
            mask: DISARM.mask,
            name: "Disarm",
            description: "Chance on hit to disarm your opponent.",
            cost: 1,
            prereqs: &[ATTACK_SPEED_BOOST_1],
        },
        TalentNodeMeta {
            id: ids::DELIVER_DEATH,
            layer: DELIVER_DEATH.layer,
            mask: DELIVER_DEATH.mask,
            name: "Deliver Death",
            description: "A devastating finishing blow against low-health enemies.",
            cost: 1,
            prereqs: &[DAMAGE_BOOST_1],
        },
        TalentNodeMeta {
            id: ids::ATTACK_SPEED_BOOST_2,
            layer: ATTACK_SPEED_BOOST_2.layer,
            mask: ATTACK_SPEED_BOOST_2.mask,
            name: "Attack Speed Boost II",
            description: "Further increase your attack speed by 5%.",
            cost: 1,
            prereqs: &[DISARM],
        },
        TalentNodeMeta {
            id: ids::DAMAGE_BOOST_2,
            layer: DAMAGE_BOOST_2.layer,
            mask: DAMAGE_BOOST_2.mask,
            name: "Damage Boost II",
            description: "Further increase your melee damage by 5%.",
            cost: 1,
            prereqs: &[DELIVER_DEATH],
        },
        TalentNodeMeta {
            id: ids::PROTECTIVE_SPELLS_BOOST_1,
            layer: PROTECTIVE_SPELLS_BOOST_1.layer,
            mask: PROTECTIVE_SPELLS_BOOST_1.mask,
            name: "Protective Spells Boost I",
            description: "Increase the potency of your protective spells by 5%.",
            cost: 1,
            prereqs: &[ATTACK_SPEED_BOOST_2],
        },
        TalentNodeMeta {
            id: ids::IMMUN_RESIST_BOOST_1,
            layer: IMMUN_RESIST_BOOST_1.layer,
            mask: IMMUN_RESIST_BOOST_1.mask,
            name: "Immunity & Resistance Boost I",
            description: "Increase your immunity and resistance by 5%.",
            cost: 1,
            prereqs: &[DAMAGE_BOOST_2],
        },
        TalentNodeMeta {
            id: ids::PROTECTIVE_SPELLS_BOOST_2,
            layer: PROTECTIVE_SPELLS_BOOST_2.layer,
            mask: PROTECTIVE_SPELLS_BOOST_2.mask,
            name: "Protective Spells Boost II",
            description: "Further increase the potency of your protective spells by 5%.",
            cost: 1,
            prereqs: &[PROTECTIVE_SPELLS_BOOST_1],
        },
        TalentNodeMeta {
            id: ids::IMMUN_RESIST_BOOST_2,
            layer: IMMUN_RESIST_BOOST_2.layer,
            mask: IMMUN_RESIST_BOOST_2.mask,
            name: "Immunity & Resistance Boost II",
            description: "Further increase your immunity and resistance by 5%.",
            cost: 1,
            prereqs: &[IMMUN_RESIST_BOOST_1],
        },
        TalentNodeMeta {
            id: ids::BLADE_DANCE,
            layer: BLADE_DANCE.layer,
            mask: BLADE_DANCE.mask,
            name: "Blade Dance",
            description: "A flurry of strikes against all adjacent enemies.",
            cost: 1,
            prereqs: &[PROTECTIVE_SPELLS_BOOST_2],
        },
        TalentNodeMeta {
            id: ids::CONTAGION,
            layer: CONTAGION.layer,
            mask: CONTAGION.mask,
            name: "Contagion",
            description: "Spreads parasitic damage to nearby enemies.",
            cost: 1,
            prereqs: &[IMMUN_RESIST_BOOST_2],
        },
        TalentNodeMeta {
            id: ids::STRENGTH_BOOST_1,
            layer: STRENGTH_BOOST_1.layer,
            mask: STRENGTH_BOOST_1.mask,
            name: "Strength Boost I",
            description: "Increase your Strength attribute by 10%.",
            cost: 1,
            prereqs: &[BLADE_DANCE],
        },
        TalentNodeMeta {
            id: ids::INTELLIGENCE_BOOST_1,
            layer: INTELLIGENCE_BOOST_1.layer,
            mask: INTELLIGENCE_BOOST_1.mask,
            name: "Intelligence Boost I",
            description: "Increase your Intuition attribute by 10%.",
            cost: 1,
            prereqs: &[CONTAGION],
        },
        TalentNodeMeta {
            id: ids::STRENGTH_BOOST_2,
            layer: STRENGTH_BOOST_2.layer,
            mask: STRENGTH_BOOST_2.mask,
            name: "Strength Boost II",
            description: "Further increase your Strength attribute by 10%.",
            cost: 1,
            prereqs: &[STRENGTH_BOOST_1],
        },
        TalentNodeMeta {
            id: ids::INTELLIGENCE_BOOST_2,
            layer: INTELLIGENCE_BOOST_2.layer,
            mask: INTELLIGENCE_BOOST_2.mask,
            name: "Intelligence Boost II",
            description: "Further increase your Intuition attribute by 10%.",
            cost: 1,
            prereqs: &[INTELLIGENCE_BOOST_1],
        },
        TalentNodeMeta {
            id: ids::ALL_SKILLS_BOOST_1,
            layer: ALL_SKILLS_BOOST_1.layer,
            mask: ALL_SKILLS_BOOST_1.mask,
            name: "All Skills Boost I",
            description: "Capstone: increase all of your attributes by 5%.",
            cost: 1,
            prereqs: &[STRENGTH_BOOST_2, INTELLIGENCE_BOOST_2],
        },
    ],
};
