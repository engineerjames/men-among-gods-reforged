//! Templar class talent tree metadata and effects.

use super::{
    TalentEffect, TalentNode, TalentPrimaryHitProc, TalentPrimaryHitProcKind, TalentRef, TalentTree,
};
use crate::skills::{Attribute, Skill};
use crate::traits::Class;

const RENEWAL: TalentRef = TalentRef {
    layer: 1,
    mask: 0b0000_0001,
};
const GASH: TalentRef = TalentRef {
    layer: 1,
    mask: 0b0000_0010,
};
const CORPORAL_STRENGTH: TalentRef = TalentRef {
    layer: 2,
    mask: 0b0000_0001,
};
const STAFF_SERGEANT_STRENGTH: TalentRef = TalentRef {
    layer: 3,
    mask: 0b0000_0001,
};
const FIRST_SERGEANT_MEDITATE: TalentRef = TalentRef {
    layer: 4,
    mask: 0b0000_0001,
};
const DIVINE_BLESSING: TalentRef = TalentRef {
    layer: 5,
    mask: 0b0000_0001,
};
const SEEING_RED: TalentRef = TalentRef {
    layer: 5,
    mask: 0b0000_0010,
};
const CAPTAIN_VITALITY: TalentRef = TalentRef {
    layer: 6,
    mask: 0b0000_0001,
};
const RENEWING_STRIKES: TalentRef = TalentRef {
    layer: 7,
    mask: 0b0000_0001,
};
const JUDGMENT_STRIKES: TalentRef = TalentRef {
    layer: 7,
    mask: 0b0000_0010,
};
const BRIGADIER_GENERAL_VITALITY: TalentRef = TalentRef {
    layer: 8,
    mask: 0b0000_0001,
};
const HOLY_FURY: TalentRef = TalentRef {
    layer: 9,
    mask: 0b0000_0001,
};
const INNER_STRENGTH: TalentRef = TalentRef {
    layer: 9,
    mask: 0b0000_0010,
};
const FIELD_MARSHAL_AGILITY: TalentRef = TalentRef {
    layer: 10,
    mask: 0b0000_0001,
};
const BARON_AGILITY: TalentRef = TalentRef {
    layer: 11,
    mask: 0b0000_0001,
};
const WARLORD_ASCENDANCY: TalentRef = TalentRef {
    layer: 12,
    mask: 0b0000_0001,
};

const WARLORD_EFFECTS: &[TalentEffect] = &[
    TalentEffect::AttributesPercent {
        attrs: &[Attribute::Strength, Attribute::Agility],
        percents: &[10, 10],
    },
    TalentEffect::HpManaEndFlat {
        hp: 100,
        mana: 0,
        end: 100,
    },
];

/// The full Templar talent tree.
pub static TEMPLAR_TREE: TalentTree = TalentTree {
    class: Class::Templar,
    nodes: &[
        TalentNode {
            slot: RENEWAL,
            name: "Renewal",
            description: "Learn Rains of Renewal.",
            cost: 1,
            prereqs: &[],
            effect: TalentEffect::GrantSkill {
                skill: Skill::RainsOfRenewal,
            },
        },
        TalentNode {
            slot: GASH,
            name: "Gash",
            description: "Learn Gash.",
            cost: 1,
            prereqs: &[],
            effect: TalentEffect::GrantSkill { skill: Skill::Gash },
        },
        TalentNode {
            slot: CORPORAL_STRENGTH,
            name: "Strength Boost I",
            description: "Increase Strength by 5%.",
            cost: 1,
            prereqs: &[RENEWAL, GASH],
            effect: TalentEffect::AttributesPercent {
                attrs: &[Attribute::Strength],
                percents: &[5],
            },
        },
        TalentNode {
            slot: STAFF_SERGEANT_STRENGTH,
            name: "Strength Boost II",
            description: "Increase Strength by an additional 5%.",
            cost: 1,
            prereqs: &[CORPORAL_STRENGTH],
            effect: TalentEffect::AttributesPercent {
                attrs: &[Attribute::Strength],
                percents: &[5],
            },
        },
        TalentNode {
            slot: FIRST_SERGEANT_MEDITATE,
            name: "Meditative Discipline",
            description: "Unlock Meditate at base level 5.",
            cost: 1,
            prereqs: &[STAFF_SERGEANT_STRENGTH],
            effect: TalentEffect::GrantSkillAtBase {
                skill: Skill::Meditate,
                base: 5,
            },
        },
        TalentNode {
            slot: DIVINE_BLESSING,
            name: "Divine Blessing",
            description: "Learn Sun's Blessing.",
            cost: 1,
            prereqs: &[FIRST_SERGEANT_MEDITATE],
            effect: TalentEffect::GrantSkill {
                skill: Skill::SunsBlessing,
            },
        },
        TalentNode {
            slot: SEEING_RED,
            name: "Seeing Red",
            description: "Learn Seeing Red.",
            cost: 1,
            prereqs: &[FIRST_SERGEANT_MEDITATE],
            effect: TalentEffect::GrantSkill {
                skill: Skill::SeeingRed,
            },
        },
        TalentNode {
            slot: CAPTAIN_VITALITY,
            name: "Vitality Boost I",
            description: "Increase maximum HP by 100 and endurance by 50.",
            cost: 1,
            prereqs: &[DIVINE_BLESSING, SEEING_RED],
            effect: TalentEffect::HpManaEndFlat {
                hp: 100,
                mana: 0,
                end: 50,
            },
        },
        TalentNode {
            slot: RENEWING_STRIKES,
            name: "Renewing Strikes",
            description: "Every fifth landed primary attack heals you for 50 HP.",
            cost: 1,
            prereqs: &[CAPTAIN_VITALITY],
            effect: TalentEffect::PrimaryHitProc {
                proc: TalentPrimaryHitProc {
                    every_hits: 5,
                    kind: TalentPrimaryHitProcKind::HealSelfHp { hp: 50 },
                },
            },
        },
        TalentNode {
            slot: JUDGMENT_STRIKES,
            name: "Judgment Strikes",
            description: "Every fifth landed primary attack deals 25 extra damage.",
            cost: 1,
            prereqs: &[CAPTAIN_VITALITY],
            effect: TalentEffect::PrimaryHitProc {
                proc: TalentPrimaryHitProc {
                    every_hits: 5,
                    kind: TalentPrimaryHitProcKind::DamageTarget { damage: 25 },
                },
            },
        },
        TalentNode {
            slot: BRIGADIER_GENERAL_VITALITY,
            name: "Vitality Boost II",
            description: "Increase maximum HP by 100 and endurance by 50.",
            cost: 1,
            prereqs: &[RENEWING_STRIKES, JUDGMENT_STRIKES],
            effect: TalentEffect::HpManaEndFlat {
                hp: 100,
                mana: 0,
                end: 50,
            },
        },
        TalentNode {
            slot: HOLY_FURY,
            name: "Holy Fury",
            description: "Learn Thunderous Fury.",
            cost: 1,
            prereqs: &[BRIGADIER_GENERAL_VITALITY],
            effect: TalentEffect::GrantSkill {
                skill: Skill::ThunderousFury,
            },
        },
        TalentNode {
            slot: INNER_STRENGTH,
            name: "Inner Strength",
            description: "Learn Inner Strength.",
            cost: 1,
            prereqs: &[BRIGADIER_GENERAL_VITALITY],
            effect: TalentEffect::GrantSkill {
                skill: Skill::InnerStrength,
            },
        },
        TalentNode {
            slot: FIELD_MARSHAL_AGILITY,
            name: "Agility Boost I",
            description: "Increase Agility by 5%.",
            cost: 1,
            prereqs: &[HOLY_FURY, INNER_STRENGTH],
            effect: TalentEffect::AttributesPercent {
                attrs: &[Attribute::Agility],
                percents: &[5],
            },
        },
        TalentNode {
            slot: BARON_AGILITY,
            name: "Agility Boost II",
            description: "Increase Agility by an additional 5%.",
            cost: 1,
            prereqs: &[FIELD_MARSHAL_AGILITY],
            effect: TalentEffect::AttributesPercent {
                attrs: &[Attribute::Agility],
                percents: &[5],
            },
        },
        TalentNode {
            slot: WARLORD_ASCENDANCY,
            name: "Warlord Ascendancy",
            description: "Increase Strength and Agility by 10%, maximum HP by 100, and endurance by 100.",
            cost: 1,
            prereqs: &[BARON_AGILITY],
            effect: TalentEffect::Composite {
                effects: WARLORD_EFFECTS,
            },
        },
    ],
};
