pub const SK_HAND: usize = 0;
pub const SK_KARATE: usize = 1;
pub const SK_SWORD: usize = 3;
pub const SK_AXE: usize = 4;
pub const SK_DAGGER: usize = 2;
pub const SK_STAFF: usize = 5;
pub const SK_TWOHAND: usize = 6; // two handed weapon
/// Canonical combat skill used for all weapon and unarmed fighting.
pub const SK_WEAPON: usize = 36;

/// Legacy combat skill slots now unified under [`SK_WEAPON`].
pub const LEGACY_WEAPON_SKILLS: [usize; 7] = [
    SK_HAND, SK_KARATE, SK_DAGGER, SK_SWORD, SK_AXE, SK_STAFF, SK_TWOHAND,
];
pub const SK_LOCK: usize = 7;
pub const SK_STEALTH: usize = 8;
pub const SK_PERCEPT: usize = 9;
pub const SK_SWIM: usize = 10;
pub const SK_MSHIELD: usize = 11;
pub const SK_BARTER: usize = 12;
pub const SK_REPAIR: usize = 13;
pub const SK_LIGHT: usize = 14;
pub const SK_RECALL: usize = 15;
pub const SK_WIMPY: usize = 16;
pub const SK_PROTECT: usize = 17;
pub const SK_ENHANCE: usize = 18;
pub const SK_STUN: usize = 19;
pub const SK_CURSE: usize = 20;
pub const SK_BLESS: usize = 21;
pub const SK_IDENT: usize = 22;
pub const SK_RESIST: usize = 23;
pub const SK_BLAST: usize = 24;
pub const SK_DISPEL: usize = 25;
pub const SK_HEAL: usize = 26;
pub const SK_GHOST: usize = 27;
pub const SK_REGEN: usize = 28;
pub const SK_REST: usize = 29;
pub const SK_MEDIT: usize = 30;
pub const SK_SENSE: usize = 31;
pub const SK_IMMUN: usize = 32;
pub const SK_SURROUND: usize = 33;
pub const SK_CONCEN: usize = 34;
pub const SK_WARCRY: usize = 35;
pub const SK_WARCRY2: usize = SK_WARCRY + 100;

const AT_NAME: [&str; 5] = ["Braveness", "Willpower", "Intuition", "Agility", "Strength"];

/// Maximum number of skill slots.
pub const MAX_SKILLS: usize = 50;

/// Returns whether `skill` is one of the retired per-weapon combat slots.
///
/// # Arguments
///
/// * `skill` - Skill index to inspect.
///
/// # Returns
///
/// * `true` if the skill feeds the unified [`SK_WEAPON`] slot.
pub const fn is_legacy_weapon_skill(skill: usize) -> bool {
    matches!(
        skill,
        SK_HAND | SK_KARATE | SK_DAGGER | SK_SWORD | SK_AXE | SK_STAFF | SK_TWOHAND
    )
}

/// Maps retired combat-weapon skill slots onto the canonical [`SK_WEAPON`] slot.
///
/// # Arguments
///
/// * `skill` - Skill index to canonicalize.
///
/// # Returns
///
/// * [`SK_WEAPON`] for retired combat weapon skills, otherwise `skill` unchanged.
pub const fn canonicalize_weapon_skill(skill: usize) -> usize {
    if is_legacy_weapon_skill(skill) {
        SK_WEAPON
    } else {
        skill
    }
}

#[repr(usize)]
pub enum SkillIndex {
    /// The base value of the skill, before any modifiers.
    BaseValue = 0,
    /// The preset modifier for the skill
    PresetModifier = 1,
    /// The maximum value the skill can reach
    MaxValue = 2,
    /// The difficulty to raise the skill (0=not raisable, 1=easy ... 10=hard)
    RaiseDifficulty = 3,
    /// The dynamic modifier for the skill, depends on equipment and spells
    DynamicModifier = 4,
    /// The total value of the skill, including all modifiers
    TotalValue = 5,
    /// Maximum index for iterating over skill values
    MaxIndex = 6,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(usize)]
pub enum Attribute {
    Braveness = 0,
    Willpower = 1,
    Intuition = 2,
    Agility = 3,
    Strength = 4,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum SkillCategory {
    Combat = b'C',
    General = b'G',
    Magic = b'R',
    Body = b'B',
    Misc = b'M',
    Unknown = b'Z',
}

impl From<SkillCategory> for char {
    fn from(cat: SkillCategory) -> char {
        match cat {
            SkillCategory::Combat => 'C',
            SkillCategory::General => 'G',
            SkillCategory::Magic => 'R',
            SkillCategory::Body => 'B',
            SkillCategory::Misc => 'M',
            SkillCategory::Unknown => 'Z',
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(usize)]
pub enum Skill {
    Hand = SK_HAND,
    Karate = SK_KARATE,
    Dagger = SK_DAGGER, // TODO: Clean this up before merging.
    Sword = SK_SWORD,
    Axe = SK_AXE,
    Staff = SK_STAFF,
    TwoHanded = SK_TWOHAND,
    LockPicking = SK_LOCK,
    Stealth = SK_STEALTH,
    Perception = SK_PERCEPT,
    Swimming = SK_SWIM,
    MagicShield = SK_MSHIELD,
    Bartering = SK_BARTER,
    Repair = SK_REPAIR,
    Light = SK_LIGHT,
    Recall = SK_RECALL,
    GuardianAngel = SK_WIMPY,
    Protection = SK_PROTECT,
    EnhanceWeapon = SK_ENHANCE,
    Stun = SK_STUN,
    Curse = SK_CURSE,
    Bless = SK_BLESS,
    Identify = SK_IDENT,
    Resistance = SK_RESIST,
    Blast = SK_BLAST,
    DispelMagic = SK_DISPEL,
    Heal = SK_HEAL,
    GhostCompanion = SK_GHOST,
    Regenerate = SK_REGEN,
    Rest = SK_REST,
    Meditate = SK_MEDIT,
    SenseMagic = SK_SENSE,
    Immunity = SK_IMMUN,
    SurroundHit = SK_SURROUND,
    Concentrate = SK_CONCEN,
    Warcry = SK_WARCRY,
}

/// A skill definition entry describing one learnable ability.
///
/// Each entry records the skill's index, category code, name,
/// description, and the three attribute indices that govern it.
#[derive(Copy, Clone)]
pub struct SkillTab {
    nr: usize,
    cat: SkillCategory,
    #[allow(dead_code)]
    name: &'static str,
    desc: &'static str,
    attrib: [usize; 3],
}

impl SkillTab {
    /// Creates a new `SkillTab` entry.
    ///
    /// # Arguments
    ///
    /// * `nr` - Skill index.
    /// * `cat` - Category of the skill.
    /// * `_name` - Display name.
    /// * `desc` - In-game description text.
    /// * `a0` - First governing attribute index.
    /// * `a1` - Second governing attribute index.
    /// * `a2` - Third governing attribute index.
    ///
    /// # Returns
    ///
    /// * A new `SkillTab` entry.
    pub const fn new(
        nr: usize,
        cat: SkillCategory,
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
            cat: SkillCategory::Unknown,
            name: "",
            desc: "",
            attrib: [0; 3],
        }
    }
}

/// Static lookup table of all 50 skill definitions.
pub static SKILLTAB: [SkillTab; MAX_SKILLS] = [
    SkillTab::new(
        0,
        SkillCategory::Combat,
        "Hand to Hand",
        "Fighting without weapons.",
        0,
        3,
        4,
    ),
    SkillTab::new(
        1,
        SkillCategory::Combat,
        "Karate",
        "Fighting without weapons and doing damage.",
        0,
        3,
        4,
    ),
    SkillTab::new(
        2,
        SkillCategory::Combat,
        "Dagger",
        "Fighting with daggers or similiar weapons.",
        0,
        3,
        2,
    ),
    SkillTab::new(
        3,
        SkillCategory::Combat,
        "Sword",
        "Fighting with swords or similiar weapons.",
        0,
        3,
        4,
    ),
    SkillTab::new(
        4,
        SkillCategory::Combat,
        "Axe",
        "Fighting with axes or similiar weapons.",
        0,
        4,
        4,
    ),
    SkillTab::new(
        5,
        SkillCategory::Combat,
        "Staff",
        "Fighting with staffs or similiar weapons.",
        3,
        4,
        4,
    ),
    SkillTab::new(
        6,
        SkillCategory::Combat,
        "Two-Handed",
        "Fighting with two-handed weapons.",
        3,
        4,
        4,
    ),
    SkillTab::new(
        7,
        SkillCategory::General,
        "Lock-Picking",
        "Opening doors without keys.",
        2,
        1,
        3,
    ),
    SkillTab::new(
        8,
        SkillCategory::General,
        "Stealth",
        "Moving without being seen or heard.",
        2,
        1,
        3,
    ),
    SkillTab::new(
        9,
        SkillCategory::General,
        "Perception",
        "Seeing and hearing.",
        2,
        1,
        3,
    ),
    SkillTab::new(
        10,
        SkillCategory::Misc,
        "Swimming",
        "Moving through water without drowning.",
        2,
        1,
        3,
    ),
    SkillTab::new(
        11,
        SkillCategory::Magic,
        "Magic Shield",
        "Spell: Create a magic shield (Cost: 25 Mana).",
        0,
        2,
        1,
    ),
    SkillTab::new(
        12,
        SkillCategory::General,
        "Bartering",
        "Getting good prices from merchants.",
        0,
        2,
        1,
    ),
    SkillTab::new(
        13,
        SkillCategory::General,
        "Repair",
        "Repairing items.",
        2,
        1,
        3,
    ),
    SkillTab::new(
        14,
        SkillCategory::Magic,
        "Light",
        "Spell: Create light (Cost: 5 Mana).",
        0,
        2,
        1,
    ),
    SkillTab::new(
        15,
        SkillCategory::Magic,
        "Recall",
        "Spell: Teleport to temple (Cost: 15 Mana).",
        0,
        2,
        1,
    ),
    SkillTab::new(
        16,
        SkillCategory::Magic,
        "Guardian Angel",
        "Spell: Avoid loss of HPs and items on death.",
        0,
        2,
        1,
    ),
    SkillTab::new(
        17,
        SkillCategory::Magic,
        "Protection",
        "Spell: Enhance Armor of target (Cost: 15 Mana).",
        0,
        2,
        1,
    ),
    SkillTab::new(
        18,
        SkillCategory::Magic,
        "Enhance Weapon",
        "Spell: Enhance Weapon of target (Cost: 15 Mana).",
        0,
        2,
        1,
    ),
    SkillTab::new(
        19,
        SkillCategory::Magic,
        "Stun",
        "Spell: Make target motionless (Cost: 20 Mana).",
        0,
        2,
        1,
    ),
    SkillTab::new(
        20,
        SkillCategory::Magic,
        "Curse",
        "Spell: Decrease attributes of target (Cost: 35 Mana).",
        0,
        2,
        1,
    ),
    SkillTab::new(
        21,
        SkillCategory::Magic,
        "Bless",
        "Spell: Increase attributes of target (Cost: 35 Mana).",
        0,
        2,
        1,
    ),
    SkillTab::new(
        22,
        SkillCategory::Magic,
        "Identify",
        "Spell: Read stats of item/character. (Cost: 25 Mana)",
        0,
        2,
        1,
    ),
    SkillTab::new(
        23,
        SkillCategory::General,
        "Resistance",
        "Resist against magic.",
        2,
        1,
        4,
    ),
    SkillTab::new(
        24,
        SkillCategory::Magic,
        "Blast",
        "Spell: Inflict injuries to target (Cost: varies).",
        2,
        1,
        4,
    ),
    SkillTab::new(
        25,
        SkillCategory::Magic,
        "Dispel Magic",
        "Spell: Removes curse magic from target (Cost: 25 Mana).",
        0,
        2,
        1,
    ),
    SkillTab::new(
        26,
        SkillCategory::Magic,
        "Heal",
        "Spell: Heal injuries (Cost: 25 Mana).",
        0,
        2,
        1,
    ),
    SkillTab::new(
        27,
        SkillCategory::Magic,
        "Ghost Companion",
        "Spell: Create a ghost to attack an enemy.",
        0,
        2,
        1,
    ),
    SkillTab::new(
        28,
        SkillCategory::Body,
        "Regenerate",
        "Regenerate Hitpoints faster.",
        4,
        4,
        4,
    ),
    SkillTab::new(
        29,
        SkillCategory::Body,
        "Rest",
        "Regenerate Endurance faster.",
        3,
        3,
        3,
    ),
    SkillTab::new(
        30,
        SkillCategory::Body,
        "Meditate",
        "Regenerate Mana faster.",
        2,
        1,
        1,
    ),
    SkillTab::new(
        31,
        SkillCategory::General,
        "Sense Magic",
        "Find out who casts what at you.",
        0,
        2,
        1,
    ),
    SkillTab::new(
        32,
        SkillCategory::General,
        "Immunity",
        "Partial immunity against negative magic.",
        0,
        3,
        4,
    ),
    SkillTab::new(
        33,
        SkillCategory::General,
        "Surround Hit",
        "Hit all your enemies at once.",
        0,
        3,
        4,
    ),
    SkillTab::new(
        34,
        SkillCategory::General,
        "Concentrate",
        "Reduces mana cost for all spells.",
        1,
        1,
        1,
    ),
    SkillTab::new(
        35,
        SkillCategory::General,
        "Warcry",
        "Frighten all enemies in hearing distance.",
        0,
        0,
        4,
    ),
    SkillTab::new(
        36,
        SkillCategory::Combat,
        "Weapon Skill",
        "Fighting with weapons or in close combat.",
        0,
        3,
        4,
    ),
    // 37..49 reserved empty
    SkillTab::new(37, SkillCategory::Unknown, "", "", 0, 0, 0),
    SkillTab::new(38, SkillCategory::Unknown, "", "", 0, 0, 0),
    SkillTab::new(39, SkillCategory::Unknown, "", "", 0, 0, 0),
    SkillTab::new(40, SkillCategory::Unknown, "", "", 0, 0, 0),
    SkillTab::new(41, SkillCategory::Unknown, "", "", 0, 0, 0),
    SkillTab::new(42, SkillCategory::Unknown, "", "", 0, 0, 0),
    SkillTab::new(43, SkillCategory::Unknown, "", "", 0, 0, 0),
    SkillTab::new(44, SkillCategory::Unknown, "", "", 0, 0, 0),
    SkillTab::new(45, SkillCategory::Unknown, "", "", 0, 0, 0),
    SkillTab::new(46, SkillCategory::Unknown, "", "", 0, 0, 0),
    SkillTab::new(47, SkillCategory::Unknown, "", "", 0, 0, 0),
    SkillTab::new(48, SkillCategory::Unknown, "", "", 0, 0, 0),
    SkillTab::new(49, SkillCategory::Unknown, "", "", 0, 0, 0),
];

/// Returns the canonical skill number for a given slot index.
///
/// # Arguments
///
/// * `skill_id` - Slot index into `SKILLTAB`.
///
/// # Returns
///
/// * The skill's `nr` field, or `skill_id` unchanged if out of bounds.
pub fn get_skill_nr(skill_id: usize) -> usize {
    SKILLTAB.get(skill_id).map(|s| s.nr).unwrap_or(skill_id)
}

/// Safely get the attribute indices for a skill.
///
/// # Arguments
///
/// * `skill` - Skill index.
///
/// # Returns
///
/// * `[a0, a1, a2]` attribute indices, or `[0, 0, 0]` on invalid index.
pub fn get_skill_attribs(skill: usize) -> [usize; 3] {
    if skill < MAX_SKILLS {
        SKILLTAB[skill].attrib
    } else {
        [0, 0, 0]
    }
}

/// Returns the skill name for a given index, or an empty string if out of bounds.
///
/// # Arguments
///
/// * `n` - Index of the skill
///
/// # Returns
///
/// The skill name as a string slice, or an empty string if out of bounds.
pub fn get_skill_name(n: usize) -> &'static str {
    if n < SKILL_NAMES.len() {
        SKILL_NAMES[n]
    } else {
        ""
    }
}

/// Returns the in-game description for a skill.
///
/// # Arguments
///
/// * `skill` - Skill index.
///
/// # Returns
///
/// * The description string, or an empty string on invalid index.
pub fn get_skill_desc(skill: usize) -> &'static str {
    if skill < MAX_SKILLS {
        SKILLTAB[skill].desc
    } else {
        ""
    }
}

/// Returns the category/sort-key character for a skill.
///
/// # Arguments
///
/// * `skill` - Skill index.
///
/// # Returns
///
/// * The category char, or `'Z'` on invalid index.
pub fn get_skill_sortkey(skill: usize) -> char {
    if skill < MAX_SKILLS {
        char::from(SKILLTAB[skill].cat)
    } else {
        'Z'
    }
}

/// Returns the display name for an attribute index.
///
/// # Arguments
///
/// * `n` - Attribute index (0..4).
///
/// # Returns
///
/// * The attribute name (e.g. `"Strength"`), or an empty string if out of bounds.
pub fn attribute_name(n: usize) -> &'static str {
    if n < AT_NAME.len() { AT_NAME[n] } else { "" }
}

// Static skill table (taken from server/original_source/SkillTab.cpp)
const SKILL_NAMES: [&str; 50] = [
    "Hand to Hand",
    "Karate",
    "Dagger",
    "Sword",
    "Axe",
    "Staff",
    "Two-Handed",
    "Lock-Picking",
    "Stealth",
    "Perception",
    "Swimming",
    "Magic Shield",
    "Bartering",
    "Repair",
    "Light",
    "Recall",
    "Guardian Angel",
    "Protection",
    "Enhance Weapon",
    "Stun",
    "Curse",
    "Bless",
    "Identify",
    "Resistance",
    "Blast",
    "Dispel Magic",
    "Heal",
    "Ghost Companion",
    "Regenerate",
    "Rest",
    "Meditate",
    "Sense Magic",
    "Immunity",
    "Surround Hit",
    "Concentrate",
    "Warcry",
    "Weapon Skill",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
];

/// Looks up a skill by name or numeric string.
///
/// Matching is case-insensitive and prefix-based: `"sw"` matches `"Sword"`.
/// A numeric string is interpreted as a direct skill index.
///
/// # Arguments
///
/// * `name` - Skill name, prefix, or numeric index string.
///
/// # Returns
///
/// * The matching skill index (≥ 0), or `-1` if not found.
pub fn skill_lookup(name: &str) -> i32 {
    // Full implementation ported from original C++ skill_lookup
    let name = name.trim();
    if name.is_empty() {
        return -1;
    }
    if name == "0" {
        return 0;
    }

    // Try numeric
    if let Ok(n) = name.parse::<i32>() {
        if n >= 0 && (n as usize) < SKILL_NAMES.len() {
            if n > 0 {
                return n;
            }
        } else {
            return -1;
        }
    }

    // Determine the number of meaningful skills (stop at first empty name)
    let max = SKILL_NAMES
        .iter()
        .position(|s| s.is_empty())
        .unwrap_or(SKILL_NAMES.len());

    // Try tolerant alpha matching: succeed when input matches prefix of skill name
    for (j, &skill) in SKILL_NAMES.iter().enumerate().take(max) {
        let mut name_iter = name.chars().map(|c| c.to_ascii_lowercase());
        let mut skill_iter = skill.chars().map(|c| c.to_ascii_lowercase());
        let mut matched = true;

        while let (Some(pc), Some(sc)) = (name_iter.next(), skill_iter.next()) {
            if sc == ' ' {
                break; // skill name reached a space -> accept match
            }
            if pc != sc {
                matched = false;
                break;
            }
        }

        if matched {
            return j as i32;
        }
    }

    -1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_skilltab_new() {
        let skill = SkillTab::new(
            1,
            SkillCategory::Combat,
            "Test Skill",
            "Test Description",
            0,
            1,
            2,
        );

        assert_eq!(skill.nr, 1);
        assert_eq!(skill.cat, SkillCategory::Combat);
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

        assert_eq!(get_skill_name(SK_WEAPON), "Weapon Skill");

        // Test empty skills (reserved slots)
        assert_eq!(get_skill_name(37), "");
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
                valid_categories.contains(&char::from(skill.cat)),
                "Invalid category '{}' for skill '{}'",
                char::from(skill.cat),
                skill.name
            );
        }
    }

    #[test]
    fn test_skill_categories() {
        // Test Combat skills (category 'C')
        assert_eq!(SKILLTAB[0].cat, SkillCategory::Combat); // Hand to Hand
        assert_eq!(SKILLTAB[1].cat, SkillCategory::Combat); // Karate
        assert_eq!(SKILLTAB[2].cat, SkillCategory::Combat); // Dagger
        assert_eq!(SKILLTAB[3].cat, SkillCategory::Combat); // Sword
        assert_eq!(SKILLTAB[4].cat, SkillCategory::Combat); // Axe
        assert_eq!(SKILLTAB[5].cat, SkillCategory::Combat); // Staff
        assert_eq!(SKILLTAB[6].cat, SkillCategory::Combat); // Two-Handed
        assert_eq!(SKILLTAB[SK_WEAPON].cat, SkillCategory::Combat); // Weapon Skill

        // Test General skills (category 'G')
        assert_eq!(SKILLTAB[7].cat, SkillCategory::General); // Lock-Picking
        assert_eq!(SKILLTAB[8].cat, SkillCategory::General); // Stealth
        assert_eq!(SKILLTAB[9].cat, SkillCategory::General); // Perception
        assert_eq!(SKILLTAB[12].cat, SkillCategory::General); // Bartering
        assert_eq!(SKILLTAB[13].cat, SkillCategory::General); // Repair

        // Test Magic skills (category 'R')
        assert_eq!(SKILLTAB[11].cat, SkillCategory::Magic); // Magic Shield
        assert_eq!(SKILLTAB[14].cat, SkillCategory::Magic); // Light
        assert_eq!(SKILLTAB[15].cat, SkillCategory::Magic); // Recall
        assert_eq!(SKILLTAB[16].cat, SkillCategory::Magic); // Guardian Angel

        // Test Body skills (category 'B')
        assert_eq!(SKILLTAB[28].cat, SkillCategory::Body); // Regenerate
        assert_eq!(SKILLTAB[29].cat, SkillCategory::Body); // Rest
        assert_eq!(SKILLTAB[30].cat, SkillCategory::Body); // Meditate

        // Test Misc skills (category 'M')
        assert_eq!(SKILLTAB[10].cat, SkillCategory::Misc); // Swimming

        // Test empty skills (category 'Z')
        assert_eq!(SKILLTAB[37].cat, SkillCategory::Unknown);
        assert_eq!(SKILLTAB[49].cat, SkillCategory::Unknown);
    }

    #[test]
    fn test_skill_descriptions() {
        // Test that all active skills have non-empty descriptions
        for (i, skill) in SKILLTAB.iter().take(SK_WEAPON + 1).enumerate() {
            // Active skills including the unified Weapon Skill slot.
            assert!(
                !skill.desc.is_empty(),
                "Skill {} '{}' should have a description",
                i,
                skill.name
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

    #[test]
    fn test_skill_lookup_by_name() {
        assert_eq!(skill_lookup("Sword"), 3);
        assert_eq!(skill_lookup("sw"), 3);
        assert_eq!(skill_lookup("Hand"), 0);
        assert_eq!(skill_lookup("Heal"), 26);
        assert_eq!(skill_lookup("Weapon"), SK_WEAPON as i32);
    }

    #[test]
    fn test_skill_lookup_by_number() {
        assert_eq!(skill_lookup("0"), 0);
        assert_eq!(skill_lookup("3"), 3);
        assert_eq!(skill_lookup("26"), 26);
    }

    #[test]
    fn test_skill_lookup_invalid() {
        assert_eq!(skill_lookup(""), -1);
        assert_eq!(skill_lookup("nonexistent"), -1);
        assert_eq!(skill_lookup("999"), -1);
    }

    #[test]
    fn test_attribute_name_valid() {
        assert_eq!(attribute_name(0), "Braveness");
        assert_eq!(attribute_name(1), "Willpower");
        assert_eq!(attribute_name(2), "Intuition");
        assert_eq!(attribute_name(3), "Agility");
        assert_eq!(attribute_name(4), "Strength");
    }

    #[test]
    fn test_attribute_name_out_of_bounds() {
        assert_eq!(attribute_name(5), "");
        assert_eq!(attribute_name(usize::MAX), "");
    }

    #[test]
    fn test_get_skill_desc_valid() {
        assert!(get_skill_desc(0).contains("Fighting without weapons"));
        assert!(get_skill_desc(26).contains("Heal"));
    }

    #[test]
    fn test_get_skill_desc_invalid() {
        assert_eq!(get_skill_desc(MAX_SKILLS), "");
        assert_eq!(get_skill_desc(usize::MAX), "");
    }

    #[test]
    fn test_get_skill_sortkey_valid() {
        assert_eq!(get_skill_sortkey(0), 'C');
        assert_eq!(get_skill_sortkey(7), 'G');
        assert_eq!(get_skill_sortkey(10), 'M');
        assert_eq!(get_skill_sortkey(11), 'R');
        assert_eq!(get_skill_sortkey(28), 'B');
        assert_eq!(get_skill_sortkey(SK_WEAPON), 'C');
        assert_eq!(get_skill_sortkey(37), 'Z');
    }

    #[test]
    fn legacy_weapon_skills_map_to_weapon_skill() {
        for legacy in LEGACY_WEAPON_SKILLS {
            assert!(is_legacy_weapon_skill(legacy));
            assert_eq!(canonicalize_weapon_skill(legacy), SK_WEAPON);
        }

        assert!(!is_legacy_weapon_skill(SK_WEAPON));
        assert_eq!(canonicalize_weapon_skill(SK_WEAPON), SK_WEAPON);
        assert_eq!(canonicalize_weapon_skill(SK_BLAST), SK_BLAST);
    }

    #[test]
    fn test_get_skill_sortkey_invalid() {
        assert_eq!(get_skill_sortkey(MAX_SKILLS), 'Z');
        assert_eq!(get_skill_sortkey(usize::MAX), 'Z');
    }

    #[test]
    fn test_get_skill_nr_matches_index() {
        for i in 0..MAX_SKILLS {
            assert_eq!(get_skill_nr(i), i);
        }
    }

    #[test]
    fn test_get_skill_nr_out_of_bounds() {
        assert_eq!(get_skill_nr(MAX_SKILLS), MAX_SKILLS);
        assert_eq!(get_skill_nr(100), 100);
    }
}
