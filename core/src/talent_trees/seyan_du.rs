//! Seyan-Du class talent tree metadata and effects.

use super::{TalentEffect, TalentNode, TalentRef, TalentTree};
use crate::skills::Attribute;
use crate::traits::Class;

const VETERANS_POISE: TalentRef = TalentRef {
    layer: 1,
    mask: 0b0000_0001,
};
const DRAGON_PULSE: TalentRef = TalentRef {
    layer: 1,
    mask: 0b0000_0010,
};
const EVASION_DRILL_1: TalentRef = TalentRef {
    layer: 2,
    mask: 0b0000_0001,
};
const BATTLE_CHANNEL_1: TalentRef = TalentRef {
    layer: 2,
    mask: 0b0000_0010,
};
const EVASION_DRILL_2: TalentRef = TalentRef {
    layer: 3,
    mask: 0b0000_0001,
};
const BATTLE_CHANNEL_2: TalentRef = TalentRef {
    layer: 3,
    mask: 0b0000_0010,
};
const FLOWING_STRIKE_1: TalentRef = TalentRef {
    layer: 4,
    mask: 0b0000_0001,
};
const HEAVY_STRIKE_1: TalentRef = TalentRef {
    layer: 4,
    mask: 0b0000_0010,
};
const COUNTER: TalentRef = TalentRef {
    layer: 5,
    mask: 0b0000_0001,
};
const FINAL_LESSON: TalentRef = TalentRef {
    layer: 5,
    mask: 0b0000_0010,
};
const FLOWING_STRIKE_2: TalentRef = TalentRef {
    layer: 6,
    mask: 0b0000_0001,
};
const HEAVY_STRIKE_2: TalentRef = TalentRef {
    layer: 6,
    mask: 0b0000_0010,
};
const GUARDED_FOCUS_1: TalentRef = TalentRef {
    layer: 7,
    mask: 0b0000_0001,
};
const IRON_BREATH_1: TalentRef = TalentRef {
    layer: 7,
    mask: 0b0000_0010,
};
const GUARDED_FOCUS_2: TalentRef = TalentRef {
    layer: 8,
    mask: 0b0000_0001,
};
const IRON_BREATH_2: TalentRef = TalentRef {
    layer: 8,
    mask: 0b0000_0010,
};
const STORM_FORM: TalentRef = TalentRef {
    layer: 9,
    mask: 0b0000_0001,
};
const BLOOD_ECHO: TalentRef = TalentRef {
    layer: 9,
    mask: 0b0000_0010,
};
const STRENGTH_DISCIPLINE_1: TalentRef = TalentRef {
    layer: 10,
    mask: 0b0000_0001,
};
const MIND_DISCIPLINE_1: TalentRef = TalentRef {
    layer: 10,
    mask: 0b0000_0010,
};
const STRENGTH_DISCIPLINE_2: TalentRef = TalentRef {
    layer: 11,
    mask: 0b0000_0001,
};
const MIND_DISCIPLINE_2: TalentRef = TalentRef {
    layer: 11,
    mask: 0b0000_0010,
};
const MASTER_OF_FORMS: TalentRef = TalentRef {
    layer: 12,
    mask: 0b0000_0001,
};

/// The full Seyan-Du placeholder talent tree.
pub static SEYAN_DU_TREE: TalentTree = TalentTree {
    class: Class::SeyanDu,
    nodes: &[
        TalentNode {
            slot: VETERANS_POISE,
            name: "Veteran's Poise",
            description: "Increase Braveness by 10%.",
            cost: 1,
            prereqs: &[],
            effect: TalentEffect::AttributesPercent {
                attrs: &[Attribute::Braveness],
                percents: &[10],
            },
        },
        TalentNode {
            slot: DRAGON_PULSE,
            name: "Dragon Pulse",
            description: "Increase Intuition by 10%.",
            cost: 1,
            prereqs: &[],
            effect: TalentEffect::AttributesPercent {
                attrs: &[Attribute::Intuition],
                percents: &[10],
            },
        },
        TalentNode {
            slot: EVASION_DRILL_1,
            name: "Evasion Drill I",
            description: "Increase Agility by 10%.",
            cost: 1,
            prereqs: &[VETERANS_POISE],
            effect: TalentEffect::AttributesPercent {
                attrs: &[Attribute::Agility],
                percents: &[10],
            },
        },
        TalentNode {
            slot: BATTLE_CHANNEL_1,
            name: "Battle Channel I",
            description: "Increase Willpower by 10%.",
            cost: 1,
            prereqs: &[DRAGON_PULSE],
            effect: TalentEffect::AttributesPercent {
                attrs: &[Attribute::Willpower],
                percents: &[10],
            },
        },
        TalentNode {
            slot: EVASION_DRILL_2,
            name: "Evasion Drill II",
            description: "Increase Agility by an additional 12%.",
            cost: 1,
            prereqs: &[EVASION_DRILL_1],
            effect: TalentEffect::AttributesPercent {
                attrs: &[Attribute::Agility],
                percents: &[12],
            },
        },
        TalentNode {
            slot: BATTLE_CHANNEL_2,
            name: "Battle Channel II",
            description: "Increase Willpower by an additional 12%.",
            cost: 1,
            prereqs: &[BATTLE_CHANNEL_1],
            effect: TalentEffect::AttributesPercent {
                attrs: &[Attribute::Willpower],
                percents: &[12],
            },
        },
        TalentNode {
            slot: FLOWING_STRIKE_1,
            name: "Flowing Strike I",
            description: "Increase Agility by a further 12%.",
            cost: 1,
            prereqs: &[EVASION_DRILL_2],
            effect: TalentEffect::AttributesPercent {
                attrs: &[Attribute::Agility],
                percents: &[12],
            },
        },
        TalentNode {
            slot: HEAVY_STRIKE_1,
            name: "Heavy Strike I",
            description: "Increase Strength by 12%.",
            cost: 1,
            prereqs: &[BATTLE_CHANNEL_2],
            effect: TalentEffect::AttributesPercent {
                attrs: &[Attribute::Strength],
                percents: &[12],
            },
        },
        TalentNode {
            slot: COUNTER,
            name: "Counter",
            description: "Increase Intuition by 12%.",
            cost: 1,
            prereqs: &[FLOWING_STRIKE_1],
            effect: TalentEffect::AttributesPercent {
                attrs: &[Attribute::Intuition],
                percents: &[12],
            },
        },
        TalentNode {
            slot: FINAL_LESSON,
            name: "Final Lesson",
            description: "Increase Strength by 14%.",
            cost: 1,
            prereqs: &[HEAVY_STRIKE_1],
            effect: TalentEffect::AttributesPercent {
                attrs: &[Attribute::Strength],
                percents: &[14],
            },
        },
        TalentNode {
            slot: FLOWING_STRIKE_2,
            name: "Flowing Strike II",
            description: "Increase Agility by a further 14%.",
            cost: 1,
            prereqs: &[COUNTER],
            effect: TalentEffect::AttributesPercent {
                attrs: &[Attribute::Agility],
                percents: &[14],
            },
        },
        TalentNode {
            slot: HEAVY_STRIKE_2,
            name: "Heavy Strike II",
            description: "Increase Strength by a further 14%.",
            cost: 1,
            prereqs: &[FINAL_LESSON],
            effect: TalentEffect::AttributesPercent {
                attrs: &[Attribute::Strength],
                percents: &[14],
            },
        },
        TalentNode {
            slot: GUARDED_FOCUS_1,
            name: "Guarded Focus I",
            description: "Increase Braveness by an additional 10%.",
            cost: 1,
            prereqs: &[FLOWING_STRIKE_2],
            effect: TalentEffect::AttributesPercent {
                attrs: &[Attribute::Braveness],
                percents: &[10],
            },
        },
        TalentNode {
            slot: IRON_BREATH_1,
            name: "Iron Breath I",
            description: "Increase Willpower by a further 10%.",
            cost: 1,
            prereqs: &[HEAVY_STRIKE_2],
            effect: TalentEffect::AttributesPercent {
                attrs: &[Attribute::Willpower],
                percents: &[10],
            },
        },
        TalentNode {
            slot: GUARDED_FOCUS_2,
            name: "Guarded Focus II",
            description: "Increase Braveness by a further 14%.",
            cost: 1,
            prereqs: &[GUARDED_FOCUS_1],
            effect: TalentEffect::AttributesPercent {
                attrs: &[Attribute::Braveness],
                percents: &[14],
            },
        },
        TalentNode {
            slot: IRON_BREATH_2,
            name: "Iron Breath II",
            description: "Increase Willpower by a further 14%.",
            cost: 1,
            prereqs: &[IRON_BREATH_1],
            effect: TalentEffect::AttributesPercent {
                attrs: &[Attribute::Willpower],
                percents: &[14],
            },
        },
        TalentNode {
            slot: STORM_FORM,
            name: "Storm Form",
            description: "Increase Agility by a further 18%.",
            cost: 1,
            prereqs: &[GUARDED_FOCUS_2],
            effect: TalentEffect::AttributesPercent {
                attrs: &[Attribute::Agility],
                percents: &[18],
            },
        },
        TalentNode {
            slot: BLOOD_ECHO,
            name: "Blood Echo",
            description: "Increase Intuition by a further 18%.",
            cost: 1,
            prereqs: &[IRON_BREATH_2],
            effect: TalentEffect::AttributesPercent {
                attrs: &[Attribute::Intuition],
                percents: &[18],
            },
        },
        TalentNode {
            slot: STRENGTH_DISCIPLINE_1,
            name: "Strength Discipline I",
            description: "Increase Strength by a further 10%.",
            cost: 1,
            prereqs: &[STORM_FORM],
            effect: TalentEffect::AttributesPercent {
                attrs: &[Attribute::Strength],
                percents: &[10],
            },
        },
        TalentNode {
            slot: MIND_DISCIPLINE_1,
            name: "Mind Discipline I",
            description: "Increase Intuition by a further 10%.",
            cost: 1,
            prereqs: &[BLOOD_ECHO],
            effect: TalentEffect::AttributesPercent {
                attrs: &[Attribute::Intuition],
                percents: &[10],
            },
        },
        TalentNode {
            slot: STRENGTH_DISCIPLINE_2,
            name: "Strength Discipline II",
            description: "Increase Strength by a further 12%.",
            cost: 1,
            prereqs: &[STRENGTH_DISCIPLINE_1],
            effect: TalentEffect::AttributesPercent {
                attrs: &[Attribute::Strength],
                percents: &[12],
            },
        },
        TalentNode {
            slot: MIND_DISCIPLINE_2,
            name: "Mind Discipline II",
            description: "Increase Intuition by a further 12%.",
            cost: 1,
            prereqs: &[MIND_DISCIPLINE_1],
            effect: TalentEffect::AttributesPercent {
                attrs: &[Attribute::Intuition],
                percents: &[12],
            },
        },
        TalentNode {
            slot: MASTER_OF_FORMS,
            name: "Master of Forms",
            description: "Increase Braveness by a further 22%.",
            cost: 1,
            prereqs: &[STRENGTH_DISCIPLINE_2, MIND_DISCIPLINE_2],
            effect: TalentEffect::AttributesPercent {
                attrs: &[Attribute::Braveness],
                percents: &[22],
            },
        },
    ],
};
