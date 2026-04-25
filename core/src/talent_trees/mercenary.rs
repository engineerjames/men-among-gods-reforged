//! Mercenary class talent tree metadata and effects.

use super::{TalentEffect, TalentNode, TalentRef, TalentTree};
use crate::skills::Attribute;
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
/// Packed talent slot for the first mercenary dodge chance bonus.
pub const DODGE_BOOST_1: TalentRef = TalentRef {
    layer: 2,
    mask: 0b0000_0001,
};

const SPELL_BOOST_1: TalentRef = TalentRef {
    layer: 2,
    mask: 0b0000_0010,
};

// Layer 3
/// Packed talent slot for the second mercenary dodge chance bonus.
pub const DODGE_BOOST_2: TalentRef = TalentRef {
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

/// The full mercenary talent tree.
pub static MERCENARY_TREE: TalentTree = TalentTree {
    class: Class::Mercenary,
    nodes: &[
        TalentNode {
            slot: DISTRACT,
            name: "Distract",
            description: "Distract the enemy, reducing their accuracy.",
            cost: 1,
            prereqs: &[],
            effect: TalentEffect::AttributePercent {
                attr: Attribute::Strength,
                percent: 10,
            },
        },
        TalentNode {
            slot: PARASITE,
            name: "Parasite",
            description: "Infest the enemy with parasites, dealing damage over time.",
            cost: 1,
            prereqs: &[],
            effect: TalentEffect::AttributePercent {
                attr: Attribute::Willpower,
                percent: 10,
            },
        },
        TalentNode {
            slot: DODGE_BOOST_1,
            name: "Dodge Boost I",
            description: "Increase your dodge chance by 5%.",
            cost: 1,
            prereqs: &[DISTRACT, PARASITE],
            effect: TalentEffect::AttributePercent {
                attr: Attribute::Agility,
                percent: 10,
            },
        },
        TalentNode {
            slot: SPELL_BOOST_1,
            name: "Spell Boost I",
            description: "Increase the potency of your offensive spells by 5%.",
            cost: 1,
            prereqs: &[DISTRACT, PARASITE],
            effect: TalentEffect::AttributePercent {
                attr: Attribute::Willpower,
                percent: 10,
            },
        },
        TalentNode {
            slot: DODGE_BOOST_2,
            name: "Dodge Boost II",
            description: "Increase your dodge chance by an additional 5%.",
            cost: 1,
            prereqs: &[DODGE_BOOST_1],
            effect: TalentEffect::AttributePercent {
                attr: Attribute::Agility,
                percent: 15,
            },
        },
        TalentNode {
            slot: SPELL_BOOST_2,
            name: "Spell Boost II",
            description: "Further increase the potency of your offensive spells by 5%.",
            cost: 1,
            prereqs: &[SPELL_BOOST_1],
            effect: TalentEffect::AttributePercent {
                attr: Attribute::Willpower,
                percent: 15,
            },
        },
        TalentNode {
            slot: ATTACK_SPEED_BOOST_1,
            name: "Attack Speed Boost I",
            description: "Increase your attack speed by 5%.",
            cost: 1,
            prereqs: &[DODGE_BOOST_2],
            effect: TalentEffect::AttributePercent {
                attr: Attribute::Agility,
                percent: 10,
            },
        },
        TalentNode {
            slot: DAMAGE_BOOST_1,
            name: "Damage Boost I",
            description: "Increase your melee damage by 5%.",
            cost: 1,
            prereqs: &[SPELL_BOOST_2],
            effect: TalentEffect::AttributePercent {
                attr: Attribute::Strength,
                percent: 10,
            },
        },
        TalentNode {
            slot: DISARM,
            name: "Disarm",
            description: "Chance on hit to disarm your opponent.",
            cost: 1,
            prereqs: &[ATTACK_SPEED_BOOST_1],
            effect: TalentEffect::AttributePercent {
                attr: Attribute::Intuition,
                percent: 10,
            },
        },
        TalentNode {
            slot: DELIVER_DEATH,
            name: "Deliver Death",
            description: "A devastating finishing blow against low-health enemies.",
            cost: 1,
            prereqs: &[DAMAGE_BOOST_1],
            effect: TalentEffect::AttributePercent {
                attr: Attribute::Strength,
                percent: 15,
            },
        },
        TalentNode {
            slot: ATTACK_SPEED_BOOST_2,
            name: "Attack Speed Boost II",
            description: "Further increase your attack speed by 5%.",
            cost: 1,
            prereqs: &[DISARM],
            effect: TalentEffect::AttributePercent {
                attr: Attribute::Agility,
                percent: 15,
            },
        },
        TalentNode {
            slot: DAMAGE_BOOST_2,
            name: "Damage Boost II",
            description: "Further increase your melee damage by 5%.",
            cost: 1,
            prereqs: &[DELIVER_DEATH],
            effect: TalentEffect::AttributePercent {
                attr: Attribute::Strength,
                percent: 15,
            },
        },
        TalentNode {
            slot: PROTECTIVE_SPELLS_BOOST_1,
            name: "Protective Spells Boost I",
            description: "Increase the potency of your protective spells by 5%.",
            cost: 1,
            prereqs: &[ATTACK_SPEED_BOOST_2],
            effect: TalentEffect::AttributePercent {
                attr: Attribute::Willpower,
                percent: 10,
            },
        },
        TalentNode {
            slot: IMMUN_RESIST_BOOST_1,
            name: "Immunity & Resistance Boost I",
            description: "Increase your immunity and resistance by 5%.",
            cost: 1,
            prereqs: &[DAMAGE_BOOST_2],
            effect: TalentEffect::AttributePercent {
                attr: Attribute::Braveness,
                percent: 10,
            },
        },
        TalentNode {
            slot: PROTECTIVE_SPELLS_BOOST_2,
            name: "Protective Spells Boost II",
            description: "Further increase the potency of your protective spells by 5%.",
            cost: 1,
            prereqs: &[PROTECTIVE_SPELLS_BOOST_1],
            effect: TalentEffect::AttributePercent {
                attr: Attribute::Willpower,
                percent: 15,
            },
        },
        TalentNode {
            slot: IMMUN_RESIST_BOOST_2,
            name: "Immunity & Resistance Boost II",
            description: "Further increase your immunity and resistance by 5%.",
            cost: 1,
            prereqs: &[IMMUN_RESIST_BOOST_1],
            effect: TalentEffect::AttributePercent {
                attr: Attribute::Braveness,
                percent: 15,
            },
        },
        TalentNode {
            slot: BLADE_DANCE,
            name: "Blade Dance",
            description: "A flurry of strikes against all adjacent enemies.",
            cost: 1,
            prereqs: &[PROTECTIVE_SPELLS_BOOST_2],
            effect: TalentEffect::AttributePercent {
                attr: Attribute::Agility,
                percent: 20,
            },
        },
        TalentNode {
            slot: CONTAGION,
            name: "Contagion",
            description: "Spreads parasitic damage to nearby enemies.",
            cost: 1,
            prereqs: &[IMMUN_RESIST_BOOST_2],
            effect: TalentEffect::AttributePercent {
                attr: Attribute::Willpower,
                percent: 20,
            },
        },
        TalentNode {
            slot: STRENGTH_BOOST_1,
            name: "Strength Boost I",
            description: "Increase your Strength attribute by 10%.",
            cost: 1,
            prereqs: &[BLADE_DANCE],
            effect: TalentEffect::AttributePercent {
                attr: Attribute::Strength,
                percent: 10,
            },
        },
        TalentNode {
            slot: INTELLIGENCE_BOOST_1,
            name: "Intelligence Boost I",
            description: "Increase your Intuition attribute by 10%.",
            cost: 1,
            prereqs: &[CONTAGION],
            effect: TalentEffect::AttributePercent {
                attr: Attribute::Intuition,
                percent: 10,
            },
        },
        TalentNode {
            slot: STRENGTH_BOOST_2,
            name: "Strength Boost II",
            description: "Further increase your Strength attribute by 10%.",
            cost: 1,
            prereqs: &[STRENGTH_BOOST_1],
            effect: TalentEffect::AttributePercent {
                attr: Attribute::Strength,
                percent: 10,
            },
        },
        TalentNode {
            slot: INTELLIGENCE_BOOST_2,
            name: "Intelligence Boost II",
            description: "Further increase your Intuition attribute by 10%.",
            cost: 1,
            prereqs: &[INTELLIGENCE_BOOST_1],
            effect: TalentEffect::AttributePercent {
                attr: Attribute::Intuition,
                percent: 10,
            },
        },
        TalentNode {
            slot: ALL_SKILLS_BOOST_1,
            name: "All Skills Boost I",
            description: "Capstone: increase all of your attributes by 5%.",
            cost: 1,
            prereqs: &[STRENGTH_BOOST_2, INTELLIGENCE_BOOST_2],
            effect: TalentEffect::AttributePercent {
                attr: Attribute::Braveness,
                percent: 25,
            },
        },
    ],
};
