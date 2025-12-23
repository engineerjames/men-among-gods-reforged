use crate::repository::Repository;

const WHO_RANK_NAME: [&str; core::constants::RANKS] = [
    " Pvt ", " PFC ", " LCp ", " Cpl ", " Sgt ", " SSg ", " MSg ", " 1Sg ", " SgM ", "2Lieu",
    "1Lieu", "Captn", "Major", "LtCol", "Colnl", "BrGen", "MaGen", "LtGen", "Genrl", "FDMAR",
    "KNIGT", "BARON", " EARL", "WARLD",
];

// WTF is this some kind of weird hash function?
pub fn char_id(cn: usize) -> i32 {
    Repository::with_characters(|characters| {
        let mut id = 0;

        for n in (0..40).step_by(std::mem::size_of::<i32>()) {
            id ^= characters[cn].name[n] as u32;
        }

        id ^= characters[cn].pass1;
        id ^= characters[cn].pass2;

        id as i32
    })
}

pub fn points2rank(value: u32) -> u32 {
    match value {
        0..50 => 0,
        50..850 => 1,
        850..4900 => 2,
        4900..17700 => 3,
        17700..48950 => 4,
        48950..113750 => 5,
        113750..233800 => 6,
        233800..438600 => 7,
        438600..766650 => 8,
        766650..1266650 => 9,
        1266650..1998700 => 10,
        1998700..3035500 => 11,
        3035500..4463550 => 12,
        4463550..6384350 => 13,
        6384350..8915600 => 14,
        8915600..12192400 => 15,
        12192400..16368450 => 16,
        16368450..21617250 => 17,
        21617250..28133300 => 18,
        28133300..36133300 => 19,
        36133300..49014500 => 20,
        49014500..63000600 => 21,
        63000600..80977100 => 22,
        _ => 23,
    }
}

/* Calculates experience to next level from current experience and the
points2rank() function. As no inverse function is supplied we use a
binary search to determine the experience for the next level.
If the given number of points corresponds to the highest level,
return 0. */
// TODO: This seems far overcomplicated... write tests
pub fn points_tolevel(current_experience: u32) -> u32 {
    let curr_level = points2rank(current_experience);
    if curr_level == 23 {
        return 0;
    }
    let next_level = curr_level + 1;

    let mut p0 = 1;
    let mut p5;
    let mut p9 = 20 * current_experience;

    for _ in 0..100 {
        if p0 >= p9 {
            break;
        }

        p5 = (p0 + p9) / 2;
        let r = points2rank(current_experience + p5);

        if r < next_level {
            p0 = p5 + 1;
        } else {
            p9 = p5 - 1;
        }
    }

    if p0 > (20 * current_experience) {
        return 0; // Can't do it
    }

    p0 + 1
}

pub fn rankdiff(cn: i32, co: i32) -> i32 {
    let cn_experience =
        Repository::with_characters(|characters| characters[cn as usize].points_tot as u32);
    let co_experience =
        Repository::with_characters(|characters| characters[co as usize].points_tot as u32);

    points2rank(co_experience) as i32 - points2rank(cn_experience) as i32
}

pub fn absrankdiff(cn: i32, co: i32) -> u32 {
    rankdiff(cn, co).abs() as u32
}

pub fn in_attackrange(cn: i32, co: i32) -> bool {
    absrankdiff(cn, co) <= core::constants::ATTACK_RANGE as u32
}

pub fn in_grouprange(cn: i32, co: i32) -> bool {
    absrankdiff(cn, co) <= core::constants::GROUP_RANGE as u32
}

pub fn scale_exps2(cn: i32, co_rank: i32, exp: i32) -> i32 {
    const SCALE_TAB: [f32; 49] = [
        0.01, 0.01, 0.01, 0.01, 0.01, 0.01, 0.01, 0.01, 0.01, 0.02, 0.03, 0.04, 0.05, 0.06, 0.07,
        0.10, 0.15, 0.20, 0.25, 0.33, 0.50, 0.70, 0.80, 0.90, 1.00, 1.02, 1.04, 1.08, 1.16, 1.32,
        1.50, 1.75, 2.00, 2.25, 2.50, 2.75, 3.00, 3.25, 3.50, 3.75, 4.00, 4.00, 4.00, 4.00, 4.00,
        4.00, 4.00, 4.00, 4.00,
    ];

    let player_experience =
        Repository::with_characters(|characters| characters[cn as usize].points_tot as u32);

    let mut diff = co_rank - points2rank(player_experience) as i32;

    diff += 24;
    if diff < 0 {
        diff = 0;
    }
    if diff > 48 {
        diff = 48;
    }

    (exp as f32 * SCALE_TAB[diff as usize]) as i32
}

pub fn scale_exps(cn: i32, co: i32, exp: i32) -> i32 {
    let co_experience =
        Repository::with_characters(|characters| characters[co as usize].points_tot as u32);
    scale_exps2(cn, points2rank(co_experience) as i32, exp)
}

pub fn drv_dcoor2dir(dx: i32, dy: i32) -> i32 {
    match (dx.cmp(&0), dy.cmp(&0)) {
        (std::cmp::Ordering::Greater, std::cmp::Ordering::Greater) => {
            core::constants::DX_RIGHTDOWN as i32
        }
        (std::cmp::Ordering::Greater, std::cmp::Ordering::Equal) => {
            core::constants::DX_RIGHT as i32
        }
        (std::cmp::Ordering::Greater, std::cmp::Ordering::Less) => {
            core::constants::DX_RIGHTUP as i32
        }
        (std::cmp::Ordering::Equal, std::cmp::Ordering::Greater) => core::constants::DX_DOWN as i32,
        (std::cmp::Ordering::Equal, std::cmp::Ordering::Less) => core::constants::DX_UP as i32,
        (std::cmp::Ordering::Less, std::cmp::Ordering::Greater) => {
            core::constants::DX_LEFTDOWN as i32
        }
        (std::cmp::Ordering::Less, std::cmp::Ordering::Equal) => core::constants::DX_LEFT as i32,
        (std::cmp::Ordering::Less, std::cmp::Ordering::Less) => core::constants::DX_LEFTUP as i32,
        _ => -1,
    }
}

pub fn turncount(dir1: u8, dir2: u8) -> i32 {
    if dir1 == dir2 {
        return 0;
    }

    match dir1 {
        core::constants::DX_UP => match dir2 {
            core::constants::DX_DOWN => 4,
            core::constants::DX_RIGHTUP | core::constants::DX_LEFTUP => 1,
            core::constants::DX_RIGHT | core::constants::DX_LEFT => 2,
            _ => 3,
        },
        core::constants::DX_DOWN => match dir2 {
            core::constants::DX_UP => 4,
            core::constants::DX_RIGHTDOWN | core::constants::DX_LEFTDOWN => 1,
            core::constants::DX_RIGHT | core::constants::DX_LEFT => 2,
            _ => 3,
        },
        core::constants::DX_LEFT => match dir2 {
            core::constants::DX_RIGHT => 4,
            core::constants::DX_LEFTUP | core::constants::DX_LEFTDOWN => 1,
            core::constants::DX_UP | core::constants::DX_DOWN => 2,
            _ => 3,
        },
        core::constants::DX_RIGHT => match dir2 {
            core::constants::DX_LEFT => 4,
            core::constants::DX_RIGHTUP | core::constants::DX_RIGHTDOWN => 1,
            core::constants::DX_UP | core::constants::DX_DOWN => 2,
            _ => 3,
        },
        core::constants::DX_LEFTUP => match dir2 {
            core::constants::DX_RIGHTDOWN => 4,
            core::constants::DX_UP | core::constants::DX_LEFT => 1,
            core::constants::DX_RIGHTUP | core::constants::DX_LEFTDOWN => 2,
            _ => 3,
        },
        core::constants::DX_LEFTDOWN => match dir2 {
            core::constants::DX_RIGHTUP => 4,
            core::constants::DX_DOWN | core::constants::DX_LEFT => 1,
            core::constants::DX_RIGHTDOWN | core::constants::DX_LEFTUP => 2,
            _ => 3,
        },
        core::constants::DX_RIGHTUP => match dir2 {
            core::constants::DX_LEFTDOWN => 4,
            core::constants::DX_UP | core::constants::DX_RIGHT => 1,
            core::constants::DX_RIGHTDOWN | core::constants::DX_LEFTUP => 2,
            _ => 3,
        },
        core::constants::DX_RIGHTDOWN => match dir2 {
            core::constants::DX_LEFTUP => 4,
            core::constants::DX_DOWN | core::constants::DX_RIGHT => 1,
            core::constants::DX_RIGHTUP | core::constants::DX_LEFTDOWN => 2,
            _ => 3,
        },
        _ => 99,
    }
}

pub fn invis_level(cn: usize) -> i32 {}

pub fn attrib_needed(value: i32, diff: i32) -> i32 {}

pub fn hp_needed(value: i32, diff: i32) -> i32 {}

pub fn end_needed(value: i32, diff: i32) -> i32 {}

pub fn mana_needed(value: i32, diff: i32) -> i32 {}

pub fn skill_needed(value: i32, diff: i32) -> i32 {}
