//! Static skill table ported from original `SkillTab.cpp`.
//! Provides name and attribute mapping for each skill.

pub const MAX_SKILLS: usize = 50;

#[allow(dead_code)]
pub struct SkillTab {
    nr: usize,
    cat: char,
    name: &'static str,
    desc: &'static str,
    attrib: [usize; 3],
}

impl SkillTab {
    pub const fn new(
        nr: usize,
        cat: char,
        name: &'static str,
        desc: &'static str,
        a0: usize,
        a1: usize,
        a2: usize,
    ) -> Self {
        SkillTab {
            nr,
            cat,
            name,
            desc,
            attrib: [a0, a1, a2],
        }
    }
}

// NOTE: attribute indices use core::constants::AT_* constants; values are inlined here
// to avoid having to reference those constants at compile time in const context.
// The order and values mirror the original C++ `static_skilltab`.

pub static SKILLTAB: [SkillTab; MAX_SKILLS] = [
    SkillTab::new(0, 'C', "Hand to Hand", "Fighting without weapons.", 0, 3, 4),
    SkillTab::new(
        1,
        'C',
        "Karate",
        "Fighting without weapons and doing damage.",
        0,
        3,
        4,
    ),
    SkillTab::new(
        2,
        'C',
        "Dagger",
        "Fighting with daggers or similiar weapons.",
        0,
        3,
        2,
    ),
    SkillTab::new(
        3,
        'C',
        "Sword",
        "Fighting with swords or similiar weapons.",
        0,
        3,
        4,
    ),
    SkillTab::new(
        4,
        'C',
        "Axe",
        "Fighting with axes or similiar weapons.",
        0,
        4,
        4,
    ),
    SkillTab::new(
        5,
        'C',
        "Staff",
        "Fighting with staffs or similiar weapons.",
        3,
        4,
        4,
    ),
    SkillTab::new(
        6,
        'C',
        "Two-Handed",
        "Fighting with two-handed weapons.",
        3,
        4,
        4,
    ),
    SkillTab::new(
        7,
        'G',
        "Lock-Picking",
        "Opening doors without keys.",
        2,
        1,
        3,
    ),
    SkillTab::new(
        8,
        'G',
        "Stealth",
        "Moving without being seen or heard.",
        2,
        1,
        3,
    ),
    SkillTab::new(9, 'G', "Perception", "Seeing and hearing.", 2, 1, 3),
    SkillTab::new(
        10,
        'M',
        "Swimming",
        "Moving through water without drowning.",
        2,
        1,
        3,
    ),
    SkillTab::new(
        11,
        'R',
        "Magic Shield",
        "Spell: Create a magic shield (Cost: 25 Mana).",
        0,
        2,
        1,
    ),
    SkillTab::new(
        12,
        'G',
        "Bartering",
        "Getting good prices from merchants.",
        0,
        2,
        1,
    ),
    SkillTab::new(13, 'G', "Repair", "Repairing items.", 2, 1, 3),
    SkillTab::new(
        14,
        'R',
        "Light",
        "Spell: Create light (Cost: 5 Mana).",
        0,
        2,
        1,
    ),
    SkillTab::new(
        15,
        'R',
        "Recall",
        "Spell: Teleport to temple (Cost: 15 Mana).",
        0,
        2,
        1,
    ),
    SkillTab::new(
        16,
        'R',
        "Guardian Angel",
        "Spell: Avoid loss of HPs and items on death.",
        0,
        2,
        1,
    ),
    SkillTab::new(
        17,
        'R',
        "Protection",
        "Spell: Enhance Armor of target (Cost: 15 Mana).",
        0,
        2,
        1,
    ),
    SkillTab::new(
        18,
        'R',
        "Enhance Weapon",
        "Spell: Enhance Weapon of target (Cost: 15 Mana).",
        0,
        2,
        1,
    ),
    SkillTab::new(
        19,
        'R',
        "Stun",
        "Spell: Make target motionless (Cost: 20 Mana).",
        0,
        2,
        1,
    ),
    SkillTab::new(
        20,
        'R',
        "Curse",
        "Spell: Decrease attributes of target (Cost: 35 Mana).",
        0,
        2,
        1,
    ),
    SkillTab::new(
        21,
        'R',
        "Bless",
        "Spell: Increase attributes of target (Cost: 35 Mana).",
        0,
        2,
        1,
    ),
    SkillTab::new(
        22,
        'R',
        "Identify",
        "Spell: Read stats of item/character. (Cost: 25 Mana)",
        0,
        2,
        1,
    ),
    SkillTab::new(23, 'G', "Resistance", "Resist against magic.", 2, 1, 4),
    SkillTab::new(
        24,
        'R',
        "Blast",
        "Spell: Inflict injuries to target (Cost: varies).",
        2,
        1,
        4,
    ),
    SkillTab::new(
        25,
        'R',
        "Dispel Magic",
        "Spell: Removes curse magic from target (Cost: 25 Mana).",
        0,
        2,
        1,
    ),
    SkillTab::new(
        26,
        'R',
        "Heal",
        "Spell: Heal injuries (Cost: 25 Mana).",
        0,
        2,
        1,
    ),
    SkillTab::new(
        27,
        'R',
        "Ghost Companion",
        "Spell: Create a ghost to attack an enemy.",
        0,
        2,
        1,
    ),
    SkillTab::new(
        28,
        'B',
        "Regenerate",
        "Regenerate Hitpoints faster.",
        4,
        4,
        4,
    ),
    SkillTab::new(29, 'B', "Rest", "Regenerate Endurance faster.", 3, 3, 3),
    SkillTab::new(30, 'B', "Meditate", "Regenerate Mana faster.", 2, 1, 1),
    SkillTab::new(
        31,
        'G',
        "Sense Magic",
        "Find out who casts what at you.",
        0,
        2,
        1,
    ),
    SkillTab::new(
        32,
        'G',
        "Immunity",
        "Partial immunity against negative magic.",
        0,
        3,
        4,
    ),
    SkillTab::new(
        33,
        'G',
        "Surround Hit",
        "Hit all your enemies at once.",
        0,
        3,
        4,
    ),
    SkillTab::new(
        34,
        'G',
        "Concentrate",
        "Reduces mana cost for all spells.",
        1,
        1,
        1,
    ),
    SkillTab::new(
        35,
        'G',
        "Warcry",
        "Frighten all enemies in hearing distance.",
        0,
        0,
        4,
    ),
    // 36..49 reserved empty
    SkillTab::new(36, 'Z', "", "", 0, 0, 0),
    SkillTab::new(37, 'Z', "", "", 0, 0, 0),
    SkillTab::new(38, 'Z', "", "", 0, 0, 0),
    SkillTab::new(39, 'Z', "", "", 0, 0, 0),
    SkillTab::new(40, 'Z', "", "", 0, 0, 0),
    SkillTab::new(41, 'Z', "", "", 0, 0, 0),
    SkillTab::new(42, 'Z', "", "", 0, 0, 0),
    SkillTab::new(43, 'Z', "", "", 0, 0, 0),
    SkillTab::new(44, 'Z', "", "", 0, 0, 0),
    SkillTab::new(45, 'Z', "", "", 0, 0, 0),
    SkillTab::new(46, 'Z', "", "", 0, 0, 0),
    SkillTab::new(47, 'Z', "", "", 0, 0, 0),
    SkillTab::new(48, 'Z', "", "", 0, 0, 0),
    SkillTab::new(49, 'Z', "", "", 0, 0, 0),
];

/// Safely get the attribute indices for a skill. Returns (0,0,0) on invalid index.
pub fn get_skill_attribs(skill: usize) -> [usize; 3] {
    if skill < MAX_SKILLS {
        SKILLTAB[skill].attrib
    } else {
        [0, 0, 0]
    }
}

/// Safely get the skill name (empty string on invalid index)
#[allow(dead_code)]
pub fn get_skill_name(skill: usize) -> &'static str {
    if skill < MAX_SKILLS {
        SKILLTAB[skill].name
    } else {
        ""
    }
}
