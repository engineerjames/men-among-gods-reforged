//! Harakim class talent tree metadata and effects.

use super::{TalentEffect, TalentNode, TalentRef, TalentTree};
use crate::skills::{Attribute, Skill};
use crate::traits::Class;

const LAVA_BLAST: TalentRef = TalentRef {
    layer: 1,
    mask: 0b0000_0001,
};
const REVENANT_CONDUIT: TalentRef = TalentRef {
    layer: 1,
    mask: 0b0000_0010,
};
const CORPORAL_INTUITION: TalentRef = TalentRef {
    layer: 2,
    mask: 0b0000_0001,
};
const STAFF_SERGEANT_INTUITION: TalentRef = TalentRef {
    layer: 3,
    mask: 0b0000_0001,
};
const FIRST_SERGEANT_WILLPOWER: TalentRef = TalentRef {
    layer: 4,
    mask: 0b0000_0001,
};
const ICE_STUN: TalentRef = TalentRef {
    layer: 5,
    mask: 0b0000_0001,
};
const KINDRED_SPIRIT: TalentRef = TalentRef {
    layer: 5,
    mask: 0b0000_0010,
};
const CAPTAIN_RESERVES: TalentRef = TalentRef {
    layer: 6,
    mask: 0b0000_0001,
};
const ELEMENT_SWITCHING: TalentRef = TalentRef {
    layer: 7,
    mask: 0b0000_0001,
};
const SPELLCASTER_KINDRED_SPIRIT: TalentRef = TalentRef {
    layer: 7,
    mask: 0b0000_0010,
};
const BRIGADIER_GENERAL_INTUITION: TalentRef = TalentRef {
    layer: 8,
    mask: 0b0000_0001,
};
const ELEMENTAL_ANGUISH: TalentRef = TalentRef {
    layer: 9,
    mask: 0b0000_0001,
};
const SPECTRAL_PACT: TalentRef = TalentRef {
    layer: 9,
    mask: 0b0000_0010,
};
const FIELD_MARSHAL_INTUITION: TalentRef = TalentRef {
    layer: 10,
    mask: 0b0000_0001,
};
const BARON_WILLPOWER: TalentRef = TalentRef {
    layer: 11,
    mask: 0b0000_0001,
};
const WARLORD_ASCENDANCY: TalentRef = TalentRef {
    layer: 12,
    mask: 0b0000_0001,
};

const CAPTAIN_RESERVE_EFFECTS: &[TalentEffect] = &[TalentEffect::HpManaEndFlat {
    hp: 25,
    mana: 100,
    end: 25,
}];

const WARLORD_EFFECTS: &[TalentEffect] = &[
    TalentEffect::AttributesPercent {
        attrs: &[Attribute::Intuition, Attribute::Willpower],
        percents: &[10, 10],
    },
    TalentEffect::HpManaEndFlat {
        hp: 50,
        mana: 100,
        end: 50,
    },
];

/// The full Harakim talent tree.
pub static HARAKIM_TREE: TalentTree = TalentTree {
    class: Class::Harakim,
    nodes: &[
        TalentNode {
            slot: LAVA_BLAST,
            name: "Lava Blast",
            description: "Learn Lava Blast.",
            cost: 1,
            prereqs: &[],
            effect: TalentEffect::GrantSkill {
                skill: Skill::LavaBlast,
            },
        },
        TalentNode {
            slot: REVENANT_CONDUIT,
            name: "Revenant Conduit",
            description: "Learn Revenant Conduit.",
            cost: 1,
            prereqs: &[],
            effect: TalentEffect::GrantSkill {
                skill: Skill::RevenantConduit,
            },
        },
        TalentNode {
            slot: CORPORAL_INTUITION,
            name: "Corporal Intuition",
            description: "Increase Intuition by 5%.",
            cost: 1,
            prereqs: &[LAVA_BLAST, REVENANT_CONDUIT],
            effect: TalentEffect::AttributesPercent {
                attrs: &[Attribute::Intuition],
                percents: &[5],
            },
        },
        TalentNode {
            slot: STAFF_SERGEANT_INTUITION,
            name: "Staff Sergeant Intuition",
            description: "Increase Intuition by an additional 5%.",
            cost: 1,
            prereqs: &[CORPORAL_INTUITION],
            effect: TalentEffect::AttributesPercent {
                attrs: &[Attribute::Intuition],
                percents: &[5],
            },
        },
        TalentNode {
            slot: FIRST_SERGEANT_WILLPOWER,
            name: "First Sergeant Willpower",
            description: "Increase Willpower by 5%.",
            cost: 1,
            prereqs: &[STAFF_SERGEANT_INTUITION],
            effect: TalentEffect::AttributesPercent {
                attrs: &[Attribute::Willpower],
                percents: &[5],
            },
        },
        TalentNode {
            slot: ICE_STUN,
            name: "Ice Stun",
            description: "Learn Ice Stun.",
            cost: 1,
            prereqs: &[FIRST_SERGEANT_WILLPOWER],
            effect: TalentEffect::GrantSkill {
                skill: Skill::IceStun,
            },
        },
        TalentNode {
            slot: KINDRED_SPIRIT,
            name: "Kindred Spirit",
            description: "Learn Kindred Spirit.",
            cost: 1,
            prereqs: &[FIRST_SERGEANT_WILLPOWER],
            effect: TalentEffect::GrantSkill {
                skill: Skill::KindredSpirit,
            },
        },
        TalentNode {
            slot: CAPTAIN_RESERVES,
            name: "Captain Reserves",
            description: "Increase maximum mana by 100, HP by 25, and endurance by 25.",
            cost: 1,
            prereqs: &[ICE_STUN, KINDRED_SPIRIT],
            effect: TalentEffect::Composite {
                effects: CAPTAIN_RESERVE_EFFECTS,
            },
        },
        TalentNode {
            slot: ELEMENT_SWITCHING,
            name: "Element Switching",
            description: "Learn Element Switching.",
            cost: 1,
            prereqs: &[CAPTAIN_RESERVES],
            effect: TalentEffect::GrantSkill {
                skill: Skill::ElementSwitching,
            },
        },
        TalentNode {
            slot: SPELLCASTER_KINDRED_SPIRIT,
            name: "Spellcaster Kindred Spirit",
            description: "Learn Spellcaster Kindred Spirit.",
            cost: 1,
            prereqs: &[CAPTAIN_RESERVES],
            effect: TalentEffect::GrantSkill {
                skill: Skill::SpellcasterKindredSpirit,
            },
        },
        TalentNode {
            slot: BRIGADIER_GENERAL_INTUITION,
            name: "Brigadier General Intuition",
            description: "Increase Intuition by 5%.",
            cost: 1,
            prereqs: &[ELEMENT_SWITCHING, SPELLCASTER_KINDRED_SPIRIT],
            effect: TalentEffect::AttributesPercent {
                attrs: &[Attribute::Intuition],
                percents: &[5],
            },
        },
        TalentNode {
            slot: ELEMENTAL_ANGUISH,
            name: "Elemental Anguish",
            description: "Learn Anguish (Earth).",
            cost: 1,
            prereqs: &[BRIGADIER_GENERAL_INTUITION],
            effect: TalentEffect::GrantSkill {
                skill: Skill::AnguishEarth,
            },
        },
        TalentNode {
            slot: SPECTRAL_PACT,
            name: "Spectral Pact",
            description: "Learn Spectral Pact.",
            cost: 1,
            prereqs: &[BRIGADIER_GENERAL_INTUITION],
            effect: TalentEffect::GrantSkill {
                skill: Skill::SpectralPact,
            },
        },
        TalentNode {
            slot: FIELD_MARSHAL_INTUITION,
            name: "Field Marshal Intuition",
            description: "Increase Intuition by 5%.",
            cost: 1,
            prereqs: &[ELEMENTAL_ANGUISH, SPECTRAL_PACT],
            effect: TalentEffect::AttributesPercent {
                attrs: &[Attribute::Intuition],
                percents: &[5],
            },
        },
        TalentNode {
            slot: BARON_WILLPOWER,
            name: "Baron Willpower",
            description: "Increase Willpower by 5%.",
            cost: 1,
            prereqs: &[FIELD_MARSHAL_INTUITION],
            effect: TalentEffect::AttributesPercent {
                attrs: &[Attribute::Willpower],
                percents: &[5],
            },
        },
        TalentNode {
            slot: WARLORD_ASCENDANCY,
            name: "Warlord Ascendancy",
            description: "Increase Intuition and Willpower by 10%, maximum mana by 100, HP by 50, and endurance by 50.",
            cost: 1,
            prereqs: &[BARON_WILLPOWER],
            effect: TalentEffect::Composite {
                effects: WARLORD_EFFECTS,
            },
        },
    ],
};
