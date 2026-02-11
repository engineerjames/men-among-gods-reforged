#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]

pub enum Class {
    Mercenary,
    Templar,
    Harakim,

    // Achieved through gameplay:
    Sorceror,
    Warrior,
    ArchHarakim,
    ArchTemplar,
    SeyanDu,
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
            Class::Sorceror => 546,
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
            Class::Sorceror => 551,
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
        546 => (true, Class::Sorceror),
        547 => (true, Class::Warrior),

        77 => (false, Class::Templar),
        76 => (false, Class::Mercenary),
        78 => (false, Class::Harakim),
        79 => (false, Class::SeyanDu),
        549 => (false, Class::ArchTemplar),
        550 => (false, Class::ArchHarakim),
        551 => (false, Class::Sorceror),
        552 => (false, Class::Warrior),

        _ => (true, Class::Mercenary),
    }
}
