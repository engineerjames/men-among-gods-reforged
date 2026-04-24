//! Seyan-Du class talent tree metadata and effects.

use super::{TalentEffect, TalentNodeMeta, TalentRef, TalentTreeMeta};
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
pub static SEYAN_DU_TREE: TalentTreeMeta = TalentTreeMeta {
    class: Class::SeyanDu,
    nodes: &[
        TalentNodeMeta {
            slot: VETERANS_POISE,
            name: "Veteran's Poise",
            description: "Root composure talent for the Seyan-Du path.",
            cost: 1,
            prereqs: &[],
            effect: TalentEffect::AttributePercent {
                attr: Attribute::Braveness,
                percent: 10,
            },
        },
        TalentNodeMeta {
            slot: DRAGON_PULSE,
            name: "Dragon Pulse",
            description: "Root inner-force talent for the Seyan-Du path.",
            cost: 1,
            prereqs: &[],
            effect: TalentEffect::AttributePercent {
                attr: Attribute::Intuition,
                percent: 10,
            },
        },
        TalentNodeMeta {
            slot: EVASION_DRILL_1,
            name: "Evasion Drill I",
            description: "Placeholder mobility drill.",
            cost: 1,
            prereqs: &[VETERANS_POISE],
            effect: TalentEffect::AttributePercent {
                attr: Attribute::Agility,
                percent: 10,
            },
        },
        TalentNodeMeta {
            slot: BATTLE_CHANNEL_1,
            name: "Battle Channel I",
            description: "Placeholder combat focus drill.",
            cost: 1,
            prereqs: &[DRAGON_PULSE],
            effect: TalentEffect::AttributePercent {
                attr: Attribute::Willpower,
                percent: 10,
            },
        },
        TalentNodeMeta {
            slot: EVASION_DRILL_2,
            name: "Evasion Drill II",
            description: "Further mobility drill.",
            cost: 1,
            prereqs: &[EVASION_DRILL_1],
            effect: TalentEffect::AttributePercent {
                attr: Attribute::Agility,
                percent: 12,
            },
        },
        TalentNodeMeta {
            slot: BATTLE_CHANNEL_2,
            name: "Battle Channel II",
            description: "Further combat focus drill.",
            cost: 1,
            prereqs: &[BATTLE_CHANNEL_1],
            effect: TalentEffect::AttributePercent {
                attr: Attribute::Willpower,
                percent: 12,
            },
        },
        TalentNodeMeta {
            slot: FLOWING_STRIKE_1,
            name: "Flowing Strike I",
            description: "Placeholder fast-strike technique.",
            cost: 1,
            prereqs: &[EVASION_DRILL_2],
            effect: TalentEffect::AttributePercent {
                attr: Attribute::Agility,
                percent: 12,
            },
        },
        TalentNodeMeta {
            slot: HEAVY_STRIKE_1,
            name: "Heavy Strike I",
            description: "Placeholder heavy-strike technique.",
            cost: 1,
            prereqs: &[BATTLE_CHANNEL_2],
            effect: TalentEffect::AttributePercent {
                attr: Attribute::Strength,
                percent: 12,
            },
        },
        TalentNodeMeta {
            slot: COUNTER,
            name: "Counter",
            description: "Placeholder counterattack talent.",
            cost: 1,
            prereqs: &[FLOWING_STRIKE_1],
            effect: TalentEffect::AttributePercent {
                attr: Attribute::Intuition,
                percent: 12,
            },
        },
        TalentNodeMeta {
            slot: FINAL_LESSON,
            name: "Final Lesson",
            description: "Placeholder finishing technique.",
            cost: 1,
            prereqs: &[HEAVY_STRIKE_1],
            effect: TalentEffect::AttributePercent {
                attr: Attribute::Strength,
                percent: 14,
            },
        },
        TalentNodeMeta {
            slot: FLOWING_STRIKE_2,
            name: "Flowing Strike II",
            description: "Advanced fast-strike technique.",
            cost: 1,
            prereqs: &[COUNTER],
            effect: TalentEffect::AttributePercent {
                attr: Attribute::Agility,
                percent: 14,
            },
        },
        TalentNodeMeta {
            slot: HEAVY_STRIKE_2,
            name: "Heavy Strike II",
            description: "Advanced heavy-strike technique.",
            cost: 1,
            prereqs: &[FINAL_LESSON],
            effect: TalentEffect::AttributePercent {
                attr: Attribute::Strength,
                percent: 14,
            },
        },
        TalentNodeMeta {
            slot: GUARDED_FOCUS_1,
            name: "Guarded Focus I",
            description: "Placeholder guarded stance improvement.",
            cost: 1,
            prereqs: &[FLOWING_STRIKE_2],
            effect: TalentEffect::AttributePercent {
                attr: Attribute::Braveness,
                percent: 10,
            },
        },
        TalentNodeMeta {
            slot: IRON_BREATH_1,
            name: "Iron Breath I",
            description: "Placeholder endurance discipline.",
            cost: 1,
            prereqs: &[HEAVY_STRIKE_2],
            effect: TalentEffect::AttributePercent {
                attr: Attribute::Willpower,
                percent: 10,
            },
        },
        TalentNodeMeta {
            slot: GUARDED_FOCUS_2,
            name: "Guarded Focus II",
            description: "Further guarded stance improvement.",
            cost: 1,
            prereqs: &[GUARDED_FOCUS_1],
            effect: TalentEffect::AttributePercent {
                attr: Attribute::Braveness,
                percent: 14,
            },
        },
        TalentNodeMeta {
            slot: IRON_BREATH_2,
            name: "Iron Breath II",
            description: "Further endurance discipline.",
            cost: 1,
            prereqs: &[IRON_BREATH_1],
            effect: TalentEffect::AttributePercent {
                attr: Attribute::Willpower,
                percent: 14,
            },
        },
        TalentNodeMeta {
            slot: STORM_FORM,
            name: "Storm Form",
            description: "Placeholder form capstone branch.",
            cost: 1,
            prereqs: &[GUARDED_FOCUS_2],
            effect: TalentEffect::AttributePercent {
                attr: Attribute::Agility,
                percent: 18,
            },
        },
        TalentNodeMeta {
            slot: BLOOD_ECHO,
            name: "Blood Echo",
            description: "Placeholder veteran pressure branch.",
            cost: 1,
            prereqs: &[IRON_BREATH_2],
            effect: TalentEffect::AttributePercent {
                attr: Attribute::Intuition,
                percent: 18,
            },
        },
        TalentNodeMeta {
            slot: STRENGTH_DISCIPLINE_1,
            name: "Strength Discipline I",
            description: "Increase strength through form practice.",
            cost: 1,
            prereqs: &[STORM_FORM],
            effect: TalentEffect::AttributePercent {
                attr: Attribute::Strength,
                percent: 10,
            },
        },
        TalentNodeMeta {
            slot: MIND_DISCIPLINE_1,
            name: "Mind Discipline I",
            description: "Increase intuition through form practice.",
            cost: 1,
            prereqs: &[BLOOD_ECHO],
            effect: TalentEffect::AttributePercent {
                attr: Attribute::Intuition,
                percent: 10,
            },
        },
        TalentNodeMeta {
            slot: STRENGTH_DISCIPLINE_2,
            name: "Strength Discipline II",
            description: "Further increase strength through form practice.",
            cost: 1,
            prereqs: &[STRENGTH_DISCIPLINE_1],
            effect: TalentEffect::AttributePercent {
                attr: Attribute::Strength,
                percent: 12,
            },
        },
        TalentNodeMeta {
            slot: MIND_DISCIPLINE_2,
            name: "Mind Discipline II",
            description: "Further increase intuition through form practice.",
            cost: 1,
            prereqs: &[MIND_DISCIPLINE_1],
            effect: TalentEffect::AttributePercent {
                attr: Attribute::Intuition,
                percent: 12,
            },
        },
        TalentNodeMeta {
            slot: MASTER_OF_FORMS,
            name: "Master of Forms",
            description: "Capstone: unite Seyan-Du speed and force.",
            cost: 1,
            prereqs: &[STRENGTH_DISCIPLINE_2, MIND_DISCIPLINE_2],
            effect: TalentEffect::AttributePercent {
                attr: Attribute::Braveness,
                percent: 22,
            },
        },
    ],
};
