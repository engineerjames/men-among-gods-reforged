pub const SK_HAND: usize = 0;
pub const SK_KARATE: usize = 1;
pub const SK_SWORD: usize = 3;
pub const SK_AXE: usize = 4;
pub const SK_DAGGER: usize = 2;
pub const SK_STAFF: usize = 5;
pub const SK_TWOHAND: usize = 6; // two handed weapon
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

pub fn attribute_name(n: usize) -> &'static str {
    if n < AT_NAME.len() {
        AT_NAME[n]
    } else {
        ""
    }
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
    "",
];

/// Returns the skill name for a given index, or an empty string if out of bounds.
///
/// # Arguments
///
/// * `n` - Index of the skill
///
/// # Returns
///
/// The skill name as a string slice, or an empty string if out of bounds.
pub fn skill_name(n: usize) -> &'static str {
    if n < SKILL_NAMES.len() {
        SKILL_NAMES[n]
    } else {
        ""
    }
}

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

        loop {
            match (name_iter.next(), skill_iter.next()) {
                (Some(pc), Some(sc)) => {
                    if sc == ' ' {
                        break; // skill name reached a space -> accept match
                    }
                    if pc != sc {
                        matched = false;
                        break;
                    }
                }
                (Some(_), None) | (None, Some(_)) | (None, None) => {
                    // either string ended -> accept if no mismatch so far
                    break;
                }
            }
        }

        if matched {
            return j as i32;
        }
    }

    -1
}
