//! Static skill table ported from original `SkillTab.cpp`.
//! Provides name and attribute mapping for each skill.

pub const MAX_SKILLS: usize = 50;

#[derive(Copy, Clone)]
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

impl Default for SkillTab {
    fn default() -> Self {
        Self {
            nr: 0,
            cat: '\0',
            name: "",
            desc: "",
            attrib: [0; 3],
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

pub fn get_skill_nr(skill_id: usize) -> usize {
    SKILLTAB.get(skill_id).map(|s| s.nr).unwrap_or(skill_id)
}

/// Safely get the attribute indices for a skill. Returns (0,0,0) on invalid index.
pub fn get_skill_attribs(skill: usize) -> [usize; 3] {
    if skill < MAX_SKILLS {
        SKILLTAB[skill].attrib
    } else {
        [0, 0, 0]
    }
}

/// Safely get the skill name (empty string on invalid index)
pub fn get_skill_name(skill: usize) -> &'static str {
    if skill < MAX_SKILLS {
        SKILLTAB[skill].name
    } else {
        ""
    }
}

/// Safely get the skill description (empty string on invalid index)
pub fn get_skill_desc(skill: usize) -> &'static str {
    if skill < MAX_SKILLS {
        SKILLTAB[skill].desc
    } else {
        ""
    }
}

/// Safely get the skill sort key / category (defaults to 'Z' on invalid index)
pub fn get_skill_sortkey(skill: usize) -> char {
    if skill < MAX_SKILLS {
        SKILLTAB[skill].cat
    } else {
        'Z'
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_skilltab_new() {
        let skill = SkillTab::new(1, 'C', "Test Skill", "Test Description", 0, 1, 2);

        assert_eq!(skill.nr, 1);
        assert_eq!(skill.cat, 'C');
        assert_eq!(skill.name, "Test Skill");
        assert_eq!(skill.desc, "Test Description");
        assert_eq!(skill.attrib, [0, 1, 2]);
    }

    #[test]
    fn test_get_skill_attribs_valid_indices() {
        // Test first skill (Hand to Hand)
        assert_eq!(get_skill_attribs(0), [0, 3, 4]);

        // Test Karate
        assert_eq!(get_skill_attribs(1), [0, 3, 4]);

        // Test Dagger
        assert_eq!(get_skill_attribs(2), [0, 3, 2]);

        // Test Sword
        assert_eq!(get_skill_attribs(3), [0, 3, 4]);

        // Test Lock-Picking
        assert_eq!(get_skill_attribs(7), [2, 1, 3]);

        // Test Magic Shield
        assert_eq!(get_skill_attribs(11), [0, 2, 1]);

        // Test last valid skill
        assert_eq!(get_skill_attribs(MAX_SKILLS - 1), [0, 0, 0]); // Empty skill
    }

    #[test]
    fn test_get_skill_attribs_invalid_indices() {
        // Test out of bounds indices
        assert_eq!(get_skill_attribs(MAX_SKILLS), [0, 0, 0]);
        assert_eq!(get_skill_attribs(MAX_SKILLS + 1), [0, 0, 0]);
        assert_eq!(get_skill_attribs(1000), [0, 0, 0]);
        assert_eq!(get_skill_attribs(usize::MAX), [0, 0, 0]);
    }

    #[test]
    fn test_get_skill_name_valid_indices() {
        // Test first few skills
        assert_eq!(get_skill_name(0), "Hand to Hand");
        assert_eq!(get_skill_name(1), "Karate");
        assert_eq!(get_skill_name(2), "Dagger");
        assert_eq!(get_skill_name(3), "Sword");
        assert_eq!(get_skill_name(4), "Axe");
        assert_eq!(get_skill_name(5), "Staff");
        assert_eq!(get_skill_name(6), "Two-Handed");

        // Test some magic skills
        assert_eq!(get_skill_name(11), "Magic Shield");
        assert_eq!(get_skill_name(14), "Light");
        assert_eq!(get_skill_name(15), "Recall");
        assert_eq!(get_skill_name(26), "Heal");

        // Test general skills
        assert_eq!(get_skill_name(7), "Lock-Picking");
        assert_eq!(get_skill_name(8), "Stealth");
        assert_eq!(get_skill_name(9), "Perception");
        assert_eq!(get_skill_name(12), "Bartering");

        // Test empty skills (reserved slots)
        assert_eq!(get_skill_name(36), "");
        assert_eq!(get_skill_name(49), "");
    }

    #[test]
    fn test_get_skill_name_invalid_indices() {
        // Test out of bounds indices
        assert_eq!(get_skill_name(MAX_SKILLS), "");
        assert_eq!(get_skill_name(MAX_SKILLS + 1), "");
        assert_eq!(get_skill_name(1000), "");
        assert_eq!(get_skill_name(usize::MAX), "");
    }

    #[test]
    fn test_skilltab_structure() {
        // Test that SKILLTAB has the expected number of skills
        assert_eq!(SKILLTAB.len(), MAX_SKILLS);

        // Test that skill numbers match their indices
        for (i, skill) in SKILLTAB.iter().enumerate() {
            assert_eq!(skill.nr, i);
        }

        // Test that all skills have valid categories
        let valid_categories = ['C', 'G', 'M', 'R', 'B', 'Z'];
        for skill in SKILLTAB.iter() {
            assert!(
                valid_categories.contains(&skill.cat),
                "Invalid category '{}' for skill '{}'",
                skill.cat,
                skill.name
            );
        }
    }

    #[test]
    fn test_skill_categories() {
        // Test Combat skills (category 'C')
        assert_eq!(SKILLTAB[0].cat, 'C'); // Hand to Hand
        assert_eq!(SKILLTAB[1].cat, 'C'); // Karate
        assert_eq!(SKILLTAB[2].cat, 'C'); // Dagger
        assert_eq!(SKILLTAB[3].cat, 'C'); // Sword
        assert_eq!(SKILLTAB[4].cat, 'C'); // Axe
        assert_eq!(SKILLTAB[5].cat, 'C'); // Staff
        assert_eq!(SKILLTAB[6].cat, 'C'); // Two-Handed

        // Test General skills (category 'G')
        assert_eq!(SKILLTAB[7].cat, 'G'); // Lock-Picking
        assert_eq!(SKILLTAB[8].cat, 'G'); // Stealth
        assert_eq!(SKILLTAB[9].cat, 'G'); // Perception
        assert_eq!(SKILLTAB[12].cat, 'G'); // Bartering
        assert_eq!(SKILLTAB[13].cat, 'G'); // Repair

        // Test Magic skills (category 'R')
        assert_eq!(SKILLTAB[11].cat, 'R'); // Magic Shield
        assert_eq!(SKILLTAB[14].cat, 'R'); // Light
        assert_eq!(SKILLTAB[15].cat, 'R'); // Recall
        assert_eq!(SKILLTAB[16].cat, 'R'); // Guardian Angel

        // Test Body skills (category 'B')
        assert_eq!(SKILLTAB[28].cat, 'B'); // Regenerate
        assert_eq!(SKILLTAB[29].cat, 'B'); // Rest
        assert_eq!(SKILLTAB[30].cat, 'B'); // Meditate

        // Test Misc skills (category 'M')
        assert_eq!(SKILLTAB[10].cat, 'M'); // Swimming

        // Test empty skills (category 'Z')
        assert_eq!(SKILLTAB[36].cat, 'Z');
        assert_eq!(SKILLTAB[49].cat, 'Z');
    }

    #[test]
    fn test_skill_descriptions() {
        // Test that all active skills have non-empty descriptions
        for i in 0..36 {
            // First 36 are active skills
            assert!(
                !SKILLTAB[i].desc.is_empty(),
                "Skill {} '{}' should have a description",
                i,
                SKILLTAB[i].name
            );
        }

        // Test some specific descriptions
        assert!(SKILLTAB[0].desc.contains("Fighting without weapons"));
        assert!(SKILLTAB[7].desc.contains("Opening doors without keys"));
        assert!(SKILLTAB[11].desc.contains("Create a magic shield"));
        assert!(SKILLTAB[26].desc.contains("Heal injuries"));
    }

    #[test]
    fn test_skill_attribute_ranges() {
        // Test that all attribute indices are within reasonable bounds
        // Assuming attributes are indexed 0-4 (common in RPGs)
        for skill in SKILLTAB.iter() {
            for &attr in skill.attrib.iter() {
                assert!(
                    attr <= 4,
                    "Attribute index {} is out of expected range for skill '{}'",
                    attr,
                    skill.name
                );
            }
        }
    }

    #[test]
    fn test_specific_skill_attributes() {
        // Test some known skill attribute combinations

        // Combat skills typically use Strength (0), Agility (3), Stamina (4)
        let hand_to_hand = get_skill_attribs(0);
        assert_eq!(hand_to_hand, [0, 3, 4]);

        let sword = get_skill_attribs(3);
        assert_eq!(sword, [0, 3, 4]);

        // Magic skills typically use Strength (0), Intuition (2), Willpower (1)
        let magic_shield = get_skill_attribs(11);
        assert_eq!(magic_shield, [0, 2, 1]);

        let light = get_skill_attribs(14);
        assert_eq!(light, [0, 2, 1]);

        // General skills often use Intuition (2), Willpower (1), Agility (3)
        let lock_picking = get_skill_attribs(7);
        assert_eq!(lock_picking, [2, 1, 3]);

        let stealth = get_skill_attribs(8);
        assert_eq!(stealth, [2, 1, 3]);
    }

    #[test]
    fn test_max_skills_constant() {
        // Verify MAX_SKILLS matches the actual array size
        assert_eq!(MAX_SKILLS, 50);
        assert_eq!(SKILLTAB.len(), MAX_SKILLS);
    }

    #[test]
    fn test_skill_names_uniqueness() {
        // Test that non-empty skill names are unique
        let mut names = std::collections::HashSet::new();
        for skill in SKILLTAB.iter() {
            if !skill.name.is_empty() {
                assert!(
                    names.insert(skill.name),
                    "Duplicate skill name found: '{}'",
                    skill.name
                );
            }
        }
    }
}
