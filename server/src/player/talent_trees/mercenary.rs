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
        TalentNode {
            id: TalentId(0x0202),
            layer: SPELL_BOOST_1.layer,
            mask: SPELL_BOOST_1.mask,
            name: "Spell Boost I",
            description: "Increase the potency of your offensive spells by 5%.",
            prereqs: &[DISTRACT, PARASITE],
            // TODO: Temporarily this will just enhance Willpower by 10%
            effect: TalentEffect::AttributePercent {
                attr: core::skills::Attribute::Willpower,
                percent: 10,
            },
        },
        TalentNode {
            id: TalentId(0x0301),
            layer: DODGE_BOOST_2.layer,
            mask: DODGE_BOOST_2.mask,
            name: "Dodge Boost II",
            description: "Increase your dodge chance by an additional 5%.",
            prereqs: &[DODGE_BOOST_1],
            // TODO: Temporarily this will just enhance Agility by 15%
            effect: TalentEffect::AttributePercent {
                attr: core::skills::Attribute::Agility,
                percent: 15,
            },
        },
        TalentNode {
            id: TalentId(0x0302),
            layer: SPELL_BOOST_2.layer,
            mask: SPELL_BOOST_2.mask,
            name: "Spell Boost II",
            description: "Further increase the potency of your offensive spells by 5%.",
            prereqs: &[SPELL_BOOST_1],
            // TODO: Temporarily this will just enhance Willpower by 15%
            effect: TalentEffect::AttributePercent {
                attr: core::skills::Attribute::Willpower,
                percent: 15,
            },
        },
        TalentNode {
            id: TalentId(0x0401),
            layer: ATTACK_SPEED_BOOST_1.layer,
            mask: ATTACK_SPEED_BOOST_1.mask,
            name: "Attack Speed Boost I",
            description: "Increase your attack speed by 5%.",
            prereqs: &[DODGE_BOOST_2],
            // TODO: Temporarily this will just enhance Agility by 10%
            effect: TalentEffect::AttributePercent {
                attr: core::skills::Attribute::Agility,
                percent: 10,
            },
        },
        TalentNode {
            id: TalentId(0x0402),
            layer: DAMAGE_BOOST_1.layer,
            mask: DAMAGE_BOOST_1.mask,
            name: "Damage Boost I",
            description: "Increase your melee damage by 5%.",
            prereqs: &[SPELL_BOOST_2],
            // TODO: Temporarily this will just enhance Strength by 10%
            effect: TalentEffect::AttributePercent {
                attr: core::skills::Attribute::Strength,
                percent: 10,
            },
        },
        TalentNode {
            id: TalentId(0x0501),
            layer: DISARM.layer,
            mask: DISARM.mask,
            name: "Disarm",
            description: "Chance on hit to disarm your opponent.",
            prereqs: &[ATTACK_SPEED_BOOST_1],
            // TODO: Temporarily this will just enhance Intuition by 10%
            effect: TalentEffect::AttributePercent {
                attr: core::skills::Attribute::Intuition,
                percent: 10,
            },
        },
        TalentNode {
            id: TalentId(0x0502),
            layer: DELIVER_DEATH.layer,
            mask: DELIVER_DEATH.mask,
            name: "Deliver Death",
            description: "A devastating finishing blow against low-health enemies.",
            prereqs: &[DAMAGE_BOOST_1],
            // TODO: Temporarily this will just enhance Strength by 15%
            effect: TalentEffect::AttributePercent {
                attr: core::skills::Attribute::Strength,
                percent: 15,
            },
        },
        TalentNode {
            id: TalentId(0x0601),
            layer: ATTACK_SPEED_BOOST_2.layer,
            mask: ATTACK_SPEED_BOOST_2.mask,
            name: "Attack Speed Boost II",
            description: "Further increase your attack speed by 5%.",
            prereqs: &[DISARM],
            // TODO: Temporarily this will just enhance Agility by 15%
            effect: TalentEffect::AttributePercent {
                attr: core::skills::Attribute::Agility,
                percent: 15,
            },
        },
        TalentNode {
            id: TalentId(0x0602),
            layer: DAMAGE_BOOST_2.layer,
            mask: DAMAGE_BOOST_2.mask,
            name: "Damage Boost II",
            description: "Further increase your melee damage by 5%.",
            prereqs: &[DELIVER_DEATH],
            // TODO: Temporarily this will just enhance Strength by 15%
            effect: TalentEffect::AttributePercent {
                attr: core::skills::Attribute::Strength,
                percent: 15,
            },
        },
        TalentNode {
            id: TalentId(0x0701),
            layer: PROTECTIVE_SPELLS_BOOST_1.layer,
            mask: PROTECTIVE_SPELLS_BOOST_1.mask,
            name: "Protective Spells Boost I",
            description: "Increase the potency of your protective spells by 5%.",
            prereqs: &[ATTACK_SPEED_BOOST_2],
            // TODO: Temporarily this will just enhance Willpower by 10%
            effect: TalentEffect::AttributePercent {
                attr: core::skills::Attribute::Willpower,
                percent: 10,
            },
        },
        TalentNode {
            id: TalentId(0x0702),
            layer: IMMUN_RESIST_BOOST_1.layer,
            mask: IMMUN_RESIST_BOOST_1.mask,
            name: "Immunity & Resistance Boost I",
            description: "Increase your immunity and resistance by 5%.",
            prereqs: &[DAMAGE_BOOST_2],
            // TODO: Temporarily this will just enhance Braveness by 10%
            effect: TalentEffect::AttributePercent {
                attr: core::skills::Attribute::Braveness,
                percent: 10,
            },
        },
        TalentNode {
            id: TalentId(0x0801),
            layer: PROTECTIVE_SPELLS_BOOST_2.layer,
            mask: PROTECTIVE_SPELLS_BOOST_2.mask,
            name: "Protective Spells Boost II",
            description: "Further increase the potency of your protective spells by 5%.",
            prereqs: &[PROTECTIVE_SPELLS_BOOST_1],
            // TODO: Temporarily this will just enhance Willpower by 15%
            effect: TalentEffect::AttributePercent {
                attr: core::skills::Attribute::Willpower,
                percent: 15,
            },
        },
        TalentNode {
            id: TalentId(0x0802),
            layer: IMMUN_RESIST_BOOST_2.layer,
            mask: IMMUN_RESIST_BOOST_2.mask,
            name: "Immunity & Resistance Boost II",
            description: "Further increase your immunity and resistance by 5%.",
            prereqs: &[IMMUN_RESIST_BOOST_1],
            // TODO: Temporarily this will just enhance Braveness by 15%
            effect: TalentEffect::AttributePercent {
                attr: core::skills::Attribute::Braveness,
                percent: 15,
            },
        },
        TalentNode {
            id: TalentId(0x0901),
            layer: BLADE_DANCE.layer,
            mask: BLADE_DANCE.mask,
            name: "Blade Dance",
            description: "A flurry of strikes against all adjacent enemies.",
            prereqs: &[PROTECTIVE_SPELLS_BOOST_2],
            // TODO: Temporarily this will just enhance Agility by 20%
            effect: TalentEffect::AttributePercent {
                attr: core::skills::Attribute::Agility,
                percent: 20,
            },
        },
        TalentNode {
            id: TalentId(0x0902),
            layer: CONTAGION.layer,
            mask: CONTAGION.mask,
            name: "Contagion",
            description: "Spreads parasitic damage to nearby enemies.",
            prereqs: &[IMMUN_RESIST_BOOST_2],
            // TODO: Temporarily this will just enhance Willpower by 20%
            effect: TalentEffect::AttributePercent {
                attr: core::skills::Attribute::Willpower,
                percent: 20,
            },
        },
        TalentNode {
            id: TalentId(0x0A01),
            layer: STRENGTH_BOOST_1.layer,
            mask: STRENGTH_BOOST_1.mask,
            name: "Strength Boost I",
            description: "Increase your Strength attribute by 10%.",
            prereqs: &[BLADE_DANCE],
            effect: TalentEffect::AttributePercent {
                attr: core::skills::Attribute::Strength,
                percent: 10,
            },
        },
        TalentNode {
            id: TalentId(0x0A02),
            layer: INTELLIGENCE_BOOST_1.layer,
            mask: INTELLIGENCE_BOOST_1.mask,
            name: "Intelligence Boost I",
            description: "Increase your Intuition attribute by 10%.",
            prereqs: &[CONTAGION],
            effect: TalentEffect::AttributePercent {
                attr: core::skills::Attribute::Intuition,
                percent: 10,
            },
        },
        TalentNode {
            id: TalentId(0x0B01),
            layer: STRENGTH_BOOST_2.layer,
            mask: STRENGTH_BOOST_2.mask,
            name: "Strength Boost II",
            description: "Further increase your Strength attribute by 10%.",
            prereqs: &[STRENGTH_BOOST_1],
            effect: TalentEffect::AttributePercent {
                attr: core::skills::Attribute::Strength,
                percent: 10,
            },
        },
        TalentNode {
            id: TalentId(0x0B02),
            layer: INTELLIGENCE_BOOST_2.layer,
            mask: INTELLIGENCE_BOOST_2.mask,
            name: "Intelligence Boost II",
            description: "Further increase your Intuition attribute by 10%.",
            prereqs: &[INTELLIGENCE_BOOST_1],
            effect: TalentEffect::AttributePercent {
                attr: core::skills::Attribute::Intuition,
                percent: 10,
            },
        },
        TalentNode {
            id: TalentId(0x0C01),
            layer: ALL_SKILLS_BOOST_1.layer,
            mask: ALL_SKILLS_BOOST_1.mask,
            name: "All Skills Boost I",
            description: "Capstone: increase all of your attributes by 5%.",
            prereqs: &[STRENGTH_BOOST_2, INTELLIGENCE_BOOST_2],
            // TODO: Temporarily this will just enhance Braveness by 25%.
            // The intended capstone applies a smaller bonus to every
            // attribute / skill once the dispatcher supports multi-effects.
            effect: TalentEffect::AttributePercent {
                attr: core::skills::Attribute::Braveness,
                percent: 25,
            },
        },
    ],
};
