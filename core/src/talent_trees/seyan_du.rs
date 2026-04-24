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

/// The full Seyan-Du placeholder talent tree.
pub static SEYAN_DU_TREE: TalentTreeMeta = TalentTreeMeta {
    class: Class::SeyanDu,
    nodes: &[
        node(
            VETERANS_POISE,
            "Veteran's Poise",
            "Root composure talent for the Seyan-Du path.",
            &[],
            attribute(Attribute::Braveness, 10),
        ),
        node(
            DRAGON_PULSE,
            "Dragon Pulse",
            "Root inner-force talent for the Seyan-Du path.",
            &[],
            attribute(Attribute::Intuition, 10),
        ),
        node(
            EVASION_DRILL_1,
            "Evasion Drill I",
            "Placeholder mobility drill.",
            &[VETERANS_POISE],
            attribute(Attribute::Agility, 10),
        ),
        node(
            BATTLE_CHANNEL_1,
            "Battle Channel I",
            "Placeholder combat focus drill.",
            &[DRAGON_PULSE],
            attribute(Attribute::Willpower, 10),
        ),
        node(
            EVASION_DRILL_2,
            "Evasion Drill II",
            "Further mobility drill.",
            &[EVASION_DRILL_1],
            attribute(Attribute::Agility, 12),
        ),
        node(
            BATTLE_CHANNEL_2,
            "Battle Channel II",
            "Further combat focus drill.",
            &[BATTLE_CHANNEL_1],
            attribute(Attribute::Willpower, 12),
        ),
        node(
            FLOWING_STRIKE_1,
            "Flowing Strike I",
            "Placeholder fast-strike technique.",
            &[EVASION_DRILL_2],
            attribute(Attribute::Agility, 12),
        ),
        node(
            HEAVY_STRIKE_1,
            "Heavy Strike I",
            "Placeholder heavy-strike technique.",
            &[BATTLE_CHANNEL_2],
            attribute(Attribute::Strength, 12),
        ),
        node(
            COUNTER,
            "Counter",
            "Placeholder counterattack talent.",
            &[FLOWING_STRIKE_1],
            attribute(Attribute::Intuition, 12),
        ),
        node(
            FINAL_LESSON,
            "Final Lesson",
            "Placeholder finishing technique.",
            &[HEAVY_STRIKE_1],
            attribute(Attribute::Strength, 14),
        ),
        node(
            FLOWING_STRIKE_2,
            "Flowing Strike II",
            "Advanced fast-strike technique.",
            &[COUNTER],
            attribute(Attribute::Agility, 14),
        ),
        node(
            HEAVY_STRIKE_2,
            "Heavy Strike II",
            "Advanced heavy-strike technique.",
            &[FINAL_LESSON],
            attribute(Attribute::Strength, 14),
        ),
        node(
            GUARDED_FOCUS_1,
            "Guarded Focus I",
            "Placeholder guarded stance improvement.",
            &[FLOWING_STRIKE_2],
            attribute(Attribute::Braveness, 10),
        ),
        node(
            IRON_BREATH_1,
            "Iron Breath I",
            "Placeholder endurance discipline.",
            &[HEAVY_STRIKE_2],
            attribute(Attribute::Willpower, 10),
        ),
        node(
            GUARDED_FOCUS_2,
            "Guarded Focus II",
            "Further guarded stance improvement.",
            &[GUARDED_FOCUS_1],
            attribute(Attribute::Braveness, 14),
        ),
        node(
            IRON_BREATH_2,
            "Iron Breath II",
            "Further endurance discipline.",
            &[IRON_BREATH_1],
            attribute(Attribute::Willpower, 14),
        ),
        node(
            STORM_FORM,
            "Storm Form",
            "Placeholder form capstone branch.",
            &[GUARDED_FOCUS_2],
            attribute(Attribute::Agility, 18),
        ),
        node(
            BLOOD_ECHO,
            "Blood Echo",
            "Placeholder veteran pressure branch.",
            &[IRON_BREATH_2],
            attribute(Attribute::Intuition, 18),
        ),
        node(
            STRENGTH_DISCIPLINE_1,
            "Strength Discipline I",
            "Increase strength through form practice.",
            &[STORM_FORM],
            attribute(Attribute::Strength, 10),
        ),
        node(
            MIND_DISCIPLINE_1,
            "Mind Discipline I",
            "Increase intuition through form practice.",
            &[BLOOD_ECHO],
            attribute(Attribute::Intuition, 10),
        ),
        node(
            STRENGTH_DISCIPLINE_2,
            "Strength Discipline II",
            "Further increase strength through form practice.",
            &[STRENGTH_DISCIPLINE_1],
            attribute(Attribute::Strength, 12),
        ),
        node(
            MIND_DISCIPLINE_2,
            "Mind Discipline II",
            "Further increase intuition through form practice.",
            &[MIND_DISCIPLINE_1],
            attribute(Attribute::Intuition, 12),
        ),
        node(
            MASTER_OF_FORMS,
            "Master of Forms",
            "Capstone: unite Seyan-Du speed and force.",
            &[STRENGTH_DISCIPLINE_2, MIND_DISCIPLINE_2],
            attribute(Attribute::Braveness, 22),
        ),
    ],
};
