//! Seyan-Du class talent tree (metadata only - no effects).
//!
//! Effects are dispatched by the server via a parallel id->effect table
//! in `server/src/player/talent_trees/seyan_du.rs`.

use super::{TalentId, TalentNodeMeta, TalentRef, TalentTreeMeta};
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

/// Stable ids for every Seyan-Du node.
pub mod ids {
    use super::TalentId;

    /// Root veteran stance node.
    pub const VETERANS_POISE: TalentId = TalentId(0x3101);
    /// Root inner power node.
    pub const DRAGON_PULSE: TalentId = TalentId(0x3102);
    /// First evasion drill node.
    pub const EVASION_DRILL_1: TalentId = TalentId(0x3201);
    /// First battle channel node.
    pub const BATTLE_CHANNEL_1: TalentId = TalentId(0x3202);
    /// Second evasion drill node.
    pub const EVASION_DRILL_2: TalentId = TalentId(0x3301);
    /// Second battle channel node.
    pub const BATTLE_CHANNEL_2: TalentId = TalentId(0x3302);
    /// First flowing strike node.
    pub const FLOWING_STRIKE_1: TalentId = TalentId(0x3401);
    /// First heavy strike node.
    pub const HEAVY_STRIKE_1: TalentId = TalentId(0x3402);
    /// Counter placeholder node.
    pub const COUNTER: TalentId = TalentId(0x3501);
    /// Final lesson placeholder node.
    pub const FINAL_LESSON: TalentId = TalentId(0x3502);
    /// Second flowing strike node.
    pub const FLOWING_STRIKE_2: TalentId = TalentId(0x3601);
    /// Second heavy strike node.
    pub const HEAVY_STRIKE_2: TalentId = TalentId(0x3602);
    /// First guarded focus node.
    pub const GUARDED_FOCUS_1: TalentId = TalentId(0x3701);
    /// First iron breath node.
    pub const IRON_BREATH_1: TalentId = TalentId(0x3702);
    /// Second guarded focus node.
    pub const GUARDED_FOCUS_2: TalentId = TalentId(0x3801);
    /// Second iron breath node.
    pub const IRON_BREATH_2: TalentId = TalentId(0x3802);
    /// Storm form placeholder node.
    pub const STORM_FORM: TalentId = TalentId(0x3901);
    /// Blood echo placeholder node.
    pub const BLOOD_ECHO: TalentId = TalentId(0x3902);
    /// First strength discipline node.
    pub const STRENGTH_DISCIPLINE_1: TalentId = TalentId(0x3A01);
    /// First mind discipline node.
    pub const MIND_DISCIPLINE_1: TalentId = TalentId(0x3A02);
    /// Second strength discipline node.
    pub const STRENGTH_DISCIPLINE_2: TalentId = TalentId(0x3B01);
    /// Second mind discipline node.
    pub const MIND_DISCIPLINE_2: TalentId = TalentId(0x3B02);
    /// Seyan-Du capstone node.
    pub const MASTER_OF_FORMS: TalentId = TalentId(0x3C01);
}

const fn node(
    id: TalentId,
    slot: TalentRef,
    name: &'static str,
    description: &'static str,
    prereqs: &'static [TalentRef],
) -> TalentNodeMeta {
    TalentNodeMeta {
        id,
        layer: slot.layer,
        mask: slot.mask,
        name,
        description,
        cost: 1,
        prereqs,
    }
}

/// The full Seyan-Du placeholder talent tree.
pub static SEYAN_DU_TREE: TalentTreeMeta = TalentTreeMeta {
    class: Class::SeyanDu,
    nodes: &[
        node(
            ids::VETERANS_POISE,
            VETERANS_POISE,
            "Veteran's Poise",
            "Root composure talent for the Seyan-Du path.",
            &[],
        ),
        node(
            ids::DRAGON_PULSE,
            DRAGON_PULSE,
            "Dragon Pulse",
            "Root inner-force talent for the Seyan-Du path.",
            &[],
        ),
        node(
            ids::EVASION_DRILL_1,
            EVASION_DRILL_1,
            "Evasion Drill I",
            "Placeholder mobility drill.",
            &[VETERANS_POISE],
        ),
        node(
            ids::BATTLE_CHANNEL_1,
            BATTLE_CHANNEL_1,
            "Battle Channel I",
            "Placeholder combat focus drill.",
            &[DRAGON_PULSE],
        ),
        node(
            ids::EVASION_DRILL_2,
            EVASION_DRILL_2,
            "Evasion Drill II",
            "Further mobility drill.",
            &[EVASION_DRILL_1],
        ),
        node(
            ids::BATTLE_CHANNEL_2,
            BATTLE_CHANNEL_2,
            "Battle Channel II",
            "Further combat focus drill.",
            &[BATTLE_CHANNEL_1],
        ),
        node(
            ids::FLOWING_STRIKE_1,
            FLOWING_STRIKE_1,
            "Flowing Strike I",
            "Placeholder fast-strike technique.",
            &[EVASION_DRILL_2],
        ),
        node(
            ids::HEAVY_STRIKE_1,
            HEAVY_STRIKE_1,
            "Heavy Strike I",
            "Placeholder heavy-strike technique.",
            &[BATTLE_CHANNEL_2],
        ),
        node(
            ids::COUNTER,
            COUNTER,
            "Counter",
            "Placeholder counterattack talent.",
            &[FLOWING_STRIKE_1],
        ),
        node(
            ids::FINAL_LESSON,
            FINAL_LESSON,
            "Final Lesson",
            "Placeholder finishing technique.",
            &[HEAVY_STRIKE_1],
        ),
        node(
            ids::FLOWING_STRIKE_2,
            FLOWING_STRIKE_2,
            "Flowing Strike II",
            "Advanced fast-strike technique.",
            &[COUNTER],
        ),
        node(
            ids::HEAVY_STRIKE_2,
            HEAVY_STRIKE_2,
            "Heavy Strike II",
            "Advanced heavy-strike technique.",
            &[FINAL_LESSON],
        ),
        node(
            ids::GUARDED_FOCUS_1,
            GUARDED_FOCUS_1,
            "Guarded Focus I",
            "Placeholder guarded stance improvement.",
            &[FLOWING_STRIKE_2],
        ),
        node(
            ids::IRON_BREATH_1,
            IRON_BREATH_1,
            "Iron Breath I",
            "Placeholder endurance discipline.",
            &[HEAVY_STRIKE_2],
        ),
        node(
            ids::GUARDED_FOCUS_2,
            GUARDED_FOCUS_2,
            "Guarded Focus II",
            "Further guarded stance improvement.",
            &[GUARDED_FOCUS_1],
        ),
        node(
            ids::IRON_BREATH_2,
            IRON_BREATH_2,
            "Iron Breath II",
            "Further endurance discipline.",
            &[IRON_BREATH_1],
        ),
        node(
            ids::STORM_FORM,
            STORM_FORM,
            "Storm Form",
            "Placeholder form capstone branch.",
            &[GUARDED_FOCUS_2],
        ),
        node(
            ids::BLOOD_ECHO,
            BLOOD_ECHO,
            "Blood Echo",
            "Placeholder veteran pressure branch.",
            &[IRON_BREATH_2],
        ),
        node(
            ids::STRENGTH_DISCIPLINE_1,
            STRENGTH_DISCIPLINE_1,
            "Strength Discipline I",
            "Increase strength through form practice.",
            &[STORM_FORM],
        ),
        node(
            ids::MIND_DISCIPLINE_1,
            MIND_DISCIPLINE_1,
            "Mind Discipline I",
            "Increase intuition through form practice.",
            &[BLOOD_ECHO],
        ),
        node(
            ids::STRENGTH_DISCIPLINE_2,
            STRENGTH_DISCIPLINE_2,
            "Strength Discipline II",
            "Further increase strength through form practice.",
            &[STRENGTH_DISCIPLINE_1],
        ),
        node(
            ids::MIND_DISCIPLINE_2,
            MIND_DISCIPLINE_2,
            "Mind Discipline II",
            "Further increase intuition through form practice.",
            &[MIND_DISCIPLINE_1],
        ),
        node(
            ids::MASTER_OF_FORMS,
            MASTER_OF_FORMS,
            "Master of Forms",
            "Capstone: unite Seyan-Du speed and force.",
            &[STRENGTH_DISCIPLINE_2, MIND_DISCIPLINE_2],
        ),
    ],
};
