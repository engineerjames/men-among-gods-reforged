//! Seyan-Du class talent tree metadata and effects.
//!
//! Unlike the other three class trees, Seyan'Du currently only defines the
//! first 7 of the usual 12 talent layers; rank milestones still award 12
//! points, so the last 5 are intentionally unspendable until the tree grows.

use super::{TalentEffect, TalentNode, TalentRef, TalentSkillProfile, TalentTree};
use crate::skills::{Attribute, Skill};
use crate::traits::Class;

// Layer 1 — root (no prerequisites)
const WELLSPRING: TalentRef = TalentRef {
    layer: 1,
    mask: 0b0000_0001,
};
const SECOND_WIND: TalentRef = TalentRef {
    layer: 1,
    mask: 0b0000_0010,
};

// Layer 2
const PIERCING_WILL: TalentRef = TalentRef {
    layer: 2,
    mask: 0b0000_0001,
};
const FLEET_HANDS: TalentRef = TalentRef {
    layer: 2,
    mask: 0b0000_0010,
};

// Layer 3
const AURA_OF_DESPAIR: TalentRef = TalentRef {
    layer: 3,
    mask: 0b0000_0001,
};
const WAR_BANNER: TalentRef = TalentRef {
    layer: 3,
    mask: 0b0000_0010,
};

// Layer 4 — single node
const WINDSTEP: TalentRef = TalentRef {
    layer: 4,
    mask: 0b0000_0001,
};

// Layer 5
const SOUL_REFLECTION: TalentRef = TalentRef {
    layer: 5,
    mask: 0b0000_0001,
};
const BLADE_DANCE: TalentRef = TalentRef {
    layer: 5,
    mask: 0b0000_0010,
};

// Layer 6
const WARRIORS_DISCIPLINE: TalentRef = TalentRef {
    layer: 6,
    mask: 0b0000_0001,
};
const SCHOLARS_DISCIPLINE: TalentRef = TalentRef {
    layer: 6,
    mask: 0b0000_0010,
};

// Layer 7 — single node, capstone (for now)
const UNBROKEN_RESOLVE: TalentRef = TalentRef {
    layer: 7,
    mask: 0b0000_0001,
};

/// The Seyan-Du talent tree.
///
/// Only layers 1..=7 are populated; layers 8..=12 remain reserved for future
/// expansion, so talent points awarded past layer 7 currently go unspent.
pub static SEYAN_DU_TREE: TalentTree = TalentTree {
    class: Class::SeyanDu,
    nodes: &[
        TalentNode {
            slot: WELLSPRING,
            name: "Wellspring",
            description: "Increase mana regeneration by 100%.",
            cost: 1,
            prereqs: &[],
            effect: TalentEffect::RegenPercent {
                hp: 0,
                end: 0,
                mana: 100,
            },
        },
        TalentNode {
            slot: SECOND_WIND,
            name: "Second Wind",
            description: "Increase endurance regeneration by 100%.",
            cost: 1,
            prereqs: &[],
            effect: TalentEffect::RegenPercent {
                hp: 0,
                end: 100,
                mana: 0,
            },
        },
        TalentNode {
            slot: PIERCING_WILL,
            name: "Piercing Will",
            description: "Increase spell penetration by 15%.",
            cost: 1,
            prereqs: &[WELLSPRING, SECOND_WIND],
            effect: TalentEffect::SpellPenetrationPercent { percent: 15 },
        },
        TalentNode {
            slot: FLEET_HANDS,
            name: "Fleet Hands",
            description: "Increase attack speed by 10%.",
            cost: 1,
            prereqs: &[WELLSPRING, SECOND_WIND],
            effect: TalentEffect::AttributesPercent {
                attrs: &[Attribute::Agility],
                percents: &[10],
            },
        },
        TalentNode {
            slot: AURA_OF_DESPAIR,
            name: "Aura of Despair",
            description: "Surround yourself with a minor curse aura that weakens nearby enemies.",
            cost: 1,
            prereqs: &[PIERCING_WILL, FLEET_HANDS],
            effect: TalentEffect::GrantSkill {
                skill: Skill::AuraCurse,
                profile: TalentSkillProfile::DEFAULT_NON_MERC
                    .with_max(0)
                    .with_difficulty(0),
            },
        },
        TalentNode {
            slot: WAR_BANNER,
            name: "War Banner",
            description: "Raise a banner that improves the armor and weapon of nearby allies.",
            cost: 1,
            prereqs: &[PIERCING_WILL, FLEET_HANDS],
            effect: TalentEffect::GrantSkill {
                skill: Skill::AuraWarBanner,
                profile: TalentSkillProfile::DEFAULT_NON_MERC
                    .with_max(0)
                    .with_difficulty(0),
            },
        },
        TalentNode {
            slot: WINDSTEP,
            name: "Windstep",
            description: "Increase movement speed by 15%.",
            cost: 1,
            prereqs: &[AURA_OF_DESPAIR, WAR_BANNER],
            effect: TalentEffect::AttributesPercent {
                attrs: &[Attribute::Agility],
                percents: &[15],
            },
        },
        TalentNode {
            slot: SOUL_REFLECTION,
            name: "Soul Reflection",
            description: "Terrify nearby enemies, causing them to flee from you for a short time.",
            cost: 1,
            prereqs: &[WINDSTEP],
            effect: TalentEffect::GrantSkill {
                skill: Skill::SoulReflection,
                profile: TalentSkillProfile::DEFAULT_NON_MERC,
            },
        },
        TalentNode {
            slot: BLADE_DANCE,
            name: "Blade Dance",
            description: "Surround Hit secondary strikes deal amplified damage.",
            cost: 1,
            prereqs: &[WINDSTEP],
            effect: TalentEffect::GrantSkill {
                skill: Skill::BladeDance,
                profile: TalentSkillProfile::DEFAULT_NON_MERC,
            },
        },
        TalentNode {
            slot: WARRIORS_DISCIPLINE,
            name: "Warrior's Discipline",
            description: "Increase Strength and Agility by 10%.",
            cost: 1,
            prereqs: &[SOUL_REFLECTION, BLADE_DANCE],
            effect: TalentEffect::AttributesPercent {
                attrs: &[Attribute::Strength, Attribute::Agility],
                percents: &[10, 10],
            },
        },
        TalentNode {
            slot: SCHOLARS_DISCIPLINE,
            name: "Scholar's Discipline",
            description: "Increase Willpower and Intuition by 10%.",
            cost: 1,
            prereqs: &[SOUL_REFLECTION, BLADE_DANCE],
            effect: TalentEffect::AttributesPercent {
                attrs: &[Attribute::Willpower, Attribute::Intuition],
                percents: &[10, 10],
            },
        },
        TalentNode {
            slot: UNBROKEN_RESOLVE,
            name: "Unbroken Resolve",
            description: "Increase Braveness by 20%.",
            cost: 1,
            prereqs: &[WARRIORS_DISCIPLINE, SCHOLARS_DISCIPLINE],
            effect: TalentEffect::AttributesPercent {
                attrs: &[Attribute::Braveness],
                percents: &[20],
            },
        },
    ],
};
