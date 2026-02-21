use serde::Deserialize;
use serde::Serialize;

use crate::constants::{
    KIN_ARCHHARAKIM, KIN_ARCHTEMPLAR, KIN_FEMALE, KIN_HARAKIM, KIN_MALE, KIN_MERCENARY,
    KIN_SEYAN_DU, KIN_SORCERER, KIN_TEMPLAR, KIN_WARRIOR,
};

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum Sex {
    Male = KIN_MALE,
    Female = KIN_FEMALE,
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Hash, Debug, Clone, Copy)]
#[repr(u32)]
pub enum Class {
    Mercenary = KIN_MERCENARY,
    Templar = KIN_TEMPLAR,
    Harakim = KIN_HARAKIM,
    Sorcerer = KIN_SORCERER,
    Warrior = KIN_WARRIOR,
    ArchTemplar = KIN_ARCHTEMPLAR,
    ArchHarakim = KIN_ARCHHARAKIM,
    SeyanDu = KIN_SEYAN_DU,
}

/// Maps a character class/sex pair to the sprite ID used in the character selection list.
///
/// This is a UI-only mapping (it does not affect server-side appearance). For any unsupported
/// combination, it falls back to the mercenary male sprite.
pub fn get_sprite_id_for_class_and_sex(class: Class, sex: Sex) -> usize {
    match (class, sex) {
        (Class::Harakim, Sex::Male) => 4048,
        (Class::Templar, Sex::Male) => 2000,
        (Class::Mercenary, Sex::Male) => 5072,
        (Class::Harakim, Sex::Female) => 6096,
        (Class::Templar, Sex::Female) => 8144,
        (Class::Mercenary, Sex::Female) => 7120,
        _ => 5072,
    }
}

/// Maps a `(sex, class)` pair to the client/server "race" integer used by the protocol.
///
/// # Arguments
/// * `is_male` - Whether the character is male (`true`) or female (`false`).
/// * `class` - The character class.
///
/// # Returns
/// * `i32` race identifier corresponding to the provided sex and class.
pub fn get_race_integer(is_male: bool, class: Class) -> i32 {
    if is_male {
        match class {
            Class::Templar => 3,
            Class::Mercenary => 2,
            Class::Harakim => 4,
            Class::SeyanDu => 13,
            Class::ArchTemplar => 544,
            Class::ArchHarakim => 545,
            Class::Sorcerer => 546,
            Class::Warrior => 547,
        }
    } else {
        match class {
            Class::Templar => 77,
            Class::Mercenary => 76,
            Class::Harakim => 78,
            Class::SeyanDu => 79,
            Class::ArchTemplar => 549,
            Class::ArchHarakim => 550,
            Class::Sorcerer => 551,
            Class::Warrior => 552,
        }
    }
}

/// Maps a protocol "race" integer back to `(sex, class)`.
///
/// # Arguments
/// * `race` - The protocol race identifier to decode.
///
/// # Returns
/// * `(bool, Class)` containing `(is_male, class)`. Unknown values fall back to `(true, Class::Mercenary)`.
pub fn get_sex_and_class(race: i32) -> (bool, Class) {
    match race {
        3 => (true, Class::Templar),
        2 => (true, Class::Mercenary),
        4 => (true, Class::Harakim),
        13 => (true, Class::SeyanDu),
        544 => (true, Class::ArchTemplar),
        545 => (true, Class::ArchHarakim),
        546 => (true, Class::Sorcerer),
        547 => (true, Class::Warrior),

        77 => (false, Class::Templar),
        76 => (false, Class::Mercenary),
        78 => (false, Class::Harakim),
        79 => (false, Class::SeyanDu),
        549 => (false, Class::ArchTemplar),
        550 => (false, Class::ArchHarakim),
        551 => (false, Class::Sorcerer),
        552 => (false, Class::Warrior),

        _ => (true, Class::Mercenary),
    }
}

impl Class {
    /// Converts a raw `u32` kin/class value into a [`Class`].
    ///
    /// # Arguments
    /// * `value` - The raw `u32` value to convert.
    ///
    /// # Returns
    /// * `Some(Class)` if the value matches a known class, `None` otherwise.
    pub fn from_u32(value: u32) -> Option<Self> {
        match value {
            value if value == Class::Mercenary as u32 => Some(Class::Mercenary),
            value if value == Class::Templar as u32 => Some(Class::Templar),
            value if value == Class::Harakim as u32 => Some(Class::Harakim),
            value if value == Class::Sorcerer as u32 => Some(Class::Sorcerer),
            value if value == Class::Warrior as u32 => Some(Class::Warrior),
            value if value == Class::ArchTemplar as u32 => Some(Class::ArchTemplar),
            value if value == Class::ArchHarakim as u32 => Some(Class::ArchHarakim),
            value if value == Class::SeyanDu as u32 => Some(Class::SeyanDu),
            _ => None,
        }
    }

    /// Returns the human-readable display name for this class.
    ///
    /// # Arguments
    /// * `self` - The class to format.
    ///
    /// # Returns
    /// * `&'static str` containing the display name.
    pub fn to_string(&self) -> &'static str {
        match self {
            Class::Mercenary => "Mercenary",
            Class::Templar => "Templar",
            Class::Harakim => "Harakim",
            Class::Sorcerer => "Sorcerer",
            Class::Warrior => "Warrior",
            Class::ArchTemplar => "Arch Templar",
            Class::ArchHarakim => "Arch Harakim",
            Class::SeyanDu => "Seyan Du",
        }
    }
}

impl Sex {
    /// Converts a raw `u32` kin/sex value into a [`Sex`].
    ///
    /// # Arguments
    /// * `value` - The raw `u32` value to convert.
    ///
    /// # Returns
    /// * `Some(Sex)` if the value matches a known sex, `None` otherwise.
    pub fn from_u32(value: u32) -> Option<Self> {
        match value {
            value if value == Sex::Male as u32 => Some(Sex::Male),
            value if value == Sex::Female as u32 => Some(Sex::Female),
            _ => None,
        }
    }

    /// Returns the human-readable display name for this sex.
    ///
    /// # Arguments
    /// * `self` - The sex to format.
    ///
    /// # Returns
    /// * `&'static str` containing the display name.
    pub fn to_string(&self) -> &'static str {
        match self {
            Sex::Female => "Female",
            Sex::Male => "Male",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{get_race_integer, get_sex_and_class, Class, Sex};

    #[test]
    fn race_mapping_roundtrips_for_all_classes_and_sexes() {
        let classes = [
            Class::Mercenary,
            Class::Templar,
            Class::Harakim,
            Class::SeyanDu,
            Class::ArchTemplar,
            Class::ArchHarakim,
            Class::Sorcerer,
            Class::Warrior,
        ];

        for is_male in [true, false] {
            for class in classes {
                let race = get_race_integer(is_male, class);
                let (decoded_is_male, decoded_class) = get_sex_and_class(race);
                assert_eq!(decoded_is_male, is_male, "unexpected sex for race={race}");
                assert_eq!(decoded_class, class, "unexpected class for race={race}");
            }
        }
    }

    #[test]
    fn race_integer_matches_known_values_for_spot_checks() {
        assert_eq!(get_race_integer(true, Class::Templar), 3);
        assert_eq!(get_race_integer(false, Class::Mercenary), 76);
        assert_eq!(get_race_integer(true, Class::ArchHarakim), 545);
        assert_eq!(get_race_integer(false, Class::Warrior), 552);
    }

    #[test]
    fn unknown_race_decodes_to_default() {
        assert_eq!(get_sex_and_class(-1), (true, Class::Mercenary));
        assert_eq!(get_sex_and_class(0), (true, Class::Mercenary));
        assert_eq!(get_sex_and_class(999_999), (true, Class::Mercenary));
    }

    #[test]
    fn class_from_u32_accepts_all_variants_and_rejects_unknown() {
        let classes = [
            Class::Mercenary,
            Class::Templar,
            Class::Harakim,
            Class::Sorcerer,
            Class::Warrior,
            Class::ArchTemplar,
            Class::ArchHarakim,
            Class::SeyanDu,
        ];

        for class in classes {
            let value = class as u32;
            assert_eq!(Class::from_u32(value), Some(class));
        }

        assert_eq!(Class::from_u32(0), None);
        assert_eq!(Class::from_u32(u32::MAX), None);
    }

    #[test]
    fn sex_from_u32_accepts_variants_and_rejects_unknown() {
        assert_eq!(Sex::from_u32(Sex::Male as u32), Some(Sex::Male));
        assert_eq!(Sex::from_u32(Sex::Female as u32), Some(Sex::Female));
        assert_eq!(Sex::from_u32(0), None);
        assert_eq!(Sex::from_u32(u32::MAX), None);
    }

    #[test]
    fn to_string_returns_expected_display_names() {
        assert_eq!(Class::ArchTemplar.to_string(), "Arch Templar");
        assert_eq!(Class::SeyanDu.to_string(), "Seyan Du");
        assert_eq!(Sex::Male.to_string(), "Male");
        assert_eq!(Sex::Female.to_string(), "Female");
    }
}
