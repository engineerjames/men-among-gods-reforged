//! Mercenary class talent tree metadata and effects.

use super::{TalentEffect, TalentNodeMeta, TalentRef, TalentTreeMeta};
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

/// The full mercenary talent tree.
pub static MERCENARY_TREE: TalentTreeMeta = TalentTreeMeta {
    class: Class::Mercenary,
    nodes: &[
        node(
            DISTRACT,
            "Distract",
            "Distract the enemy, reducing their accuracy.",
            &[],
            attribute(Attribute::Strength, 10),
        ),
        node(
            PARASITE,
            "Parasite",
            "Infest the enemy with parasites, dealing damage over time.",
            &[],
            attribute(Attribute::Willpower, 10),
        ),
        node(
            DODGE_BOOST_1,
            "Dodge Boost I",
            "Increase your dodge chance by 5%.",
            &[DISTRACT, PARASITE],
            attribute(Attribute::Agility, 10),
        ),
        node(
            SPELL_BOOST_1,
            "Spell Boost I",
            "Increase the potency of your offensive spells by 5%.",
            &[DISTRACT, PARASITE],
            attribute(Attribute::Willpower, 10),
        ),
        node(
            DODGE_BOOST_2,
            "Dodge Boost II",
            "Increase your dodge chance by an additional 5%.",
            &[DODGE_BOOST_1],
            attribute(Attribute::Agility, 15),
        ),
        node(
            SPELL_BOOST_2,
            "Spell Boost II",
            "Further increase the potency of your offensive spells by 5%.",
            &[SPELL_BOOST_1],
            attribute(Attribute::Willpower, 15),
        ),
        node(
            ATTACK_SPEED_BOOST_1,
            "Attack Speed Boost I",
            "Increase your attack speed by 5%.",
            &[DODGE_BOOST_2],
            attribute(Attribute::Agility, 10),
        ),
        node(
            DAMAGE_BOOST_1,
            "Damage Boost I",
            "Increase your melee damage by 5%.",
            &[SPELL_BOOST_2],
            attribute(Attribute::Strength, 10),
        ),
        node(
            DISARM,
            "Disarm",
            "Chance on hit to disarm your opponent.",
            &[ATTACK_SPEED_BOOST_1],
            attribute(Attribute::Intuition, 10),
        ),
        node(
            DELIVER_DEATH,
            "Deliver Death",
            "A devastating finishing blow against low-health enemies.",
            &[DAMAGE_BOOST_1],
            attribute(Attribute::Strength, 15),
        ),
        node(
            ATTACK_SPEED_BOOST_2,
            "Attack Speed Boost II",
            "Further increase your attack speed by 5%.",
            &[DISARM],
            attribute(Attribute::Agility, 15),
        ),
        node(
            DAMAGE_BOOST_2,
            "Damage Boost II",
            "Further increase your melee damage by 5%.",
            &[DELIVER_DEATH],
            attribute(Attribute::Strength, 15),
        ),
        node(
            PROTECTIVE_SPELLS_BOOST_1,
            "Protective Spells Boost I",
            "Increase the potency of your protective spells by 5%.",
            &[ATTACK_SPEED_BOOST_2],
            attribute(Attribute::Willpower, 10),
        ),
        node(
            IMMUN_RESIST_BOOST_1,
            "Immunity & Resistance Boost I",
            "Increase your immunity and resistance by 5%.",
            &[DAMAGE_BOOST_2],
            attribute(Attribute::Braveness, 10),
        ),
        node(
            PROTECTIVE_SPELLS_BOOST_2,
            "Protective Spells Boost II",
            "Further increase the potency of your protective spells by 5%.",
            &[PROTECTIVE_SPELLS_BOOST_1],
            attribute(Attribute::Willpower, 15),
        ),
        node(
            IMMUN_RESIST_BOOST_2,
            "Immunity & Resistance Boost II",
            "Further increase your immunity and resistance by 5%.",
            &[IMMUN_RESIST_BOOST_1],
            attribute(Attribute::Braveness, 15),
        ),
        node(
            BLADE_DANCE,
            "Blade Dance",
            "A flurry of strikes against all adjacent enemies.",
            &[PROTECTIVE_SPELLS_BOOST_2],
            attribute(Attribute::Agility, 20),
        ),
        node(
            CONTAGION,
            "Contagion",
            "Spreads parasitic damage to nearby enemies.",
            &[IMMUN_RESIST_BOOST_2],
            attribute(Attribute::Willpower, 20),
        ),
        node(
            STRENGTH_BOOST_1,
            "Strength Boost I",
            "Increase your Strength attribute by 10%.",
            &[BLADE_DANCE],
            attribute(Attribute::Strength, 10),
        ),
        node(
            INTELLIGENCE_BOOST_1,
            "Intelligence Boost I",
            "Increase your Intuition attribute by 10%.",
            &[CONTAGION],
            attribute(Attribute::Intuition, 10),
        ),
        node(
            STRENGTH_BOOST_2,
            "Strength Boost II",
            "Further increase your Strength attribute by 10%.",
            &[STRENGTH_BOOST_1],
            attribute(Attribute::Strength, 10),
        ),
        node(
            INTELLIGENCE_BOOST_2,
            "Intelligence Boost II",
            "Further increase your Intuition attribute by 10%.",
            &[INTELLIGENCE_BOOST_1],
            attribute(Attribute::Intuition, 10),
        ),
        node(
            ALL_SKILLS_BOOST_1,
            "All Skills Boost I",
            "Capstone: increase all of your attributes by 5%.",
            &[STRENGTH_BOOST_2, INTELLIGENCE_BOOST_2],
            attribute(Attribute::Braveness, 25),
        ),
    ],
};
