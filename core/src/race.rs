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
    pub fn from_u32(value: u32) -> Option<Self> {
        match value {
            value if value == Sex::Male as u32 => Some(Sex::Male),
            value if value == Sex::Female as u32 => Some(Sex::Female),
            _ => None,
        }
    }

    pub fn to_string(&self) -> &'static str {
        match self {
            Sex::Female => "Female",
            Sex::Male => "Male",
        }
    }
}
