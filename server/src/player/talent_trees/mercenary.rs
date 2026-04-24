use crate::player::talent_trees::{TalentEffect, TalentId, TalentNode, TalentRef, TalentTree};

// Layer 0 - root (no-prerequisites)
const DISTRACT: TalentRef = TalentRef {
    layer: 0,
    mask: 0b0000_0001,
};

const PARASITE: TalentRef = TalentRef {
    layer: 0,
    mask: 0b0000_0010,
};

// Layer 1
const DODGE_BOOST_1: TalentRef = TalentRef {
    layer: 1,
    mask: 0b0000_0001,
};

const SPELL_BOOST_1: TalentRef = TalentRef {
    layer: 1,
    mask: 0b0000_0010,
};

// Layer 2
const DODGE_BOOST_2: TalentRef = TalentRef {
    layer: 2,
    mask: 0b0000_0001,
};

const SPELL_BOOST_2: TalentRef = TalentRef {
    layer: 2,
    mask: 0b0000_0010,
};

// Layer 3
const ATTACK_SPEED_BOOST_1: TalentRef = TalentRef {
    layer: 3,
    mask: 0b0000_0001,
};

const DAMAGE_BOOST_1: TalentRef = TalentRef {
    layer: 3,
    mask: 0b0000_0010,
};

// Layer 4
const DISARM: TalentRef = TalentRef {
    layer: 4,
    mask: 0b0000_0001,
};

const DELIVER_DEATH: TalentRef = TalentRef {
    layer: 4,
    mask: 0b0000_0010,
};

// Layer 5
const ATTACK_SPEED_BOOST_2: TalentRef = TalentRef {
    layer: 5,
    mask: 0b0000_0001,
};

const DAMAGE_BOOST_2: TalentRef = TalentRef {
    layer: 5,
    mask: 0b0000_0010,
};

// Layer 6
const PROTECTIVE_SPELLS_BOOST_1: TalentRef = TalentRef {
    layer: 6,
    mask: 0b0000_0001,
};

const IMMUN_RESIST_BOOST_1: TalentRef = TalentRef {
    layer: 6,
    mask: 0b0000_0010,
};

// Layer 7
const PROTECTIVE_SPELLS_BOOST_2: TalentRef = TalentRef {
    layer: 7,
    mask: 0b0000_0001,
};

const IMMUN_RESIST_BOOST_2: TalentRef = TalentRef {
    layer: 7,
    mask: 0b0000_0010,
};

// Layer 8
const BLADE_DANCE: TalentRef = TalentRef {
    layer: 8,
    mask: 0b0000_0001,
};

const CONTAGION: TalentRef = TalentRef {
    layer: 8,
    mask: 0b0000_0010,
};

// Layer 9
const STRENGTH_BOOST_1: TalentRef = TalentRef {
    layer: 9,
    mask: 0b0000_0001,
};

const INTELLIGENCE_BOOST_1: TalentRef = TalentRef {
    layer: 9,
    mask: 0b0000_0010,
};

// Layer 10
const STRENGTH_BOOST_2: TalentRef = TalentRef {
    layer: 10,
    mask: 0b0000_0001,
};

const INTELLIGENCE_BOOST_2: TalentRef = TalentRef {
    layer: 10,
    mask: 0b0000_0010,
};

// Layer 11
const ALL_SKILLS_BOOST_1: TalentRef = TalentRef {
    layer: 11,
    mask: 0b0000_0001,
};

pub static MERCENARY_TREE: TalentTree = TalentTree {
    class: core::types::Class::Mercenary,
    nodes: &[
        TalentNode {
            // TODO: This ID doesn't seem like we really need it...
            // And it feels like we're duplicating a lot of info between
            // the TalentRef and the TalentNode. Maybe we can combine them?
            id: TalentId(0x0101),
            layer: DISTRACT.layer,
            mask: DISTRACT.mask,
            name: "Distract",
            description: "Distract the enemy, reducing their accuracy.",
            prereqs: &[],
            // TODO: Temporarily this will just enhance strength by 10%
            effect: TalentEffect::AttributePercent {
                attr: core::skills::Attribute::Strength,
                percent: 10,
            },
        },
        TalentNode {
            id: TalentId(0x0102),
            layer: PARASITE.layer,
            mask: PARASITE.mask,
            name: "Parasite",
            description: "Infest the enemy with parasites, dealing damage over time.",
            prereqs: &[],
            // TODO: Temporarily this will just enhance Willpower by 10%
            effect: TalentEffect::AttributePercent {
                attr: core::skills::Attribute::Willpower,
                percent: 10,
            },
        },
        TalentNode {
            id: TalentId(0x0201),
            layer: DODGE_BOOST_1.layer,
            mask: DODGE_BOOST_1.mask,
            name: "Dodge Boost I",
            description: "Increase your dodge chance by 5%.",
            prereqs: &[DISTRACT, PARASITE],
            // TODO: Temporarily this will just enhance Agility by 10%
            effect: TalentEffect::AttributePercent {
                attr: core::skills::Attribute::Agility,
                percent: 10,
            },
        },
    ],
};
