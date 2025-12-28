use core::{constants::CharacterFlags, types::FontColor};

use crate::{driver, god::God, populate, repository::Repository, state::State};

/// Port of C++ use_labtransfer(int cn, int nr, int exp)
/// Attempts to spawn a lab enemy and transfer the player, returning true on success, false on failure.
pub fn use_labtransfer(cn: usize, nr: i32, exp: i32) -> bool {
    use crate::repository::Repository;
    use core::constants::{CharacterFlags, SERVER_MAPX};
    // 1. Check if area is busy (any player or labkeeper in 164..184 x 159..178)
    let mut busy_name: Option<String> = None;
    'outer: for y in 159..179 {
        for x in 164..=184 {
            let co =
                Repository::with_map(|map| map[x + y * SERVER_MAPX as i32 as usize].ch as usize);
            if co != 0 {
                let flags = Repository::with_characters(|ch| ch[co].flags);
                if flags & (CharacterFlags::CF_PLAYER.bits() | CharacterFlags::CF_LABKEEPER.bits())
                    != 0
                {
                    let name = Repository::with_characters(|ch| ch[co].get_name().to_string());
                    busy_name = Some(name);
                    break 'outer;
                }
            }
        }
    }
    if let Some(name) = busy_name {
        State::with(|state| {
            state.do_character_log(
                cn,
                FontColor::Red,
                &format!("Sorry, the area is still busy. {} is there.\n", name),
            );
            log::info!(
                "Player {} attempted to enter lab {}, but area is busy with {}",
                Repository::with_characters(|ch| ch[cn].get_name().to_string()),
                nr,
                name
            );
        });
        return false;
    }

    // 2. Spawn the correct enemy type for the lab number
    let template = match nr {
        1 => 137, // grolms
        2 => 156, // lizard
        3 => 278, // spellcaster
        4 => 315, // knight
        5 => 328, // undead
        6 => 458, // light&dark
        7 => 462, // underwater
        8 => 845, // forest/golem
        9 => 919, // riddle
        _ => {
            // do_char_log(cn, 0, "Sorry, could not determine which enemy to send you.\n");
            // chlog(cn, "Sorry, could not determine which enemy to send you");
            return false;
        }
    };

    // pop_create_char(template, 0): create the enemy character (assume function exists)
    let co = populate::pop_create_char(template, false);
    if co == 0 {
        // do_char_log(cn, 0, "Sorry, could not create your enemy.\n");
        // chlog(cn, "Sorry, could not create your enemy");
        State::with(|state| {
            state.do_character_log(cn, FontColor::Red, "Sorry, could not create your enemy.\n");
            log::error!(
                "use_labtransfer: pop_create_char({}) failed for player {}",
                template,
                Repository::with_characters(|ch| ch[cn].get_name().to_string())
            );
        });
        return false;
    }

    // god_drop_char(co, 174, 172): place the enemy (assume function exists)
    if !God::drop_char(co, 174, 172) {
        State::with(|state| {
            state.do_character_log(cn, FontColor::Red, "Sorry, could not place your enemy.\n");
            log::error!(
                "use_labtransfer: god_drop_char({}, 174, 172) failed for player {}",
                co,
                Repository::with_characters(|ch| ch[cn].get_name().to_string())
            );
        });
        God::destroy_items(co);
        Repository::with_characters_mut(|ch| ch[co].used = core::constants::USE_EMPTY);
        return false;
    }

    // Set up enemy data fields and flags
    Repository::with_characters_mut(|ch| {
        ch[co].data[64] =
            Repository::with_globals(|globs| globs.ticker) + 5 * 60 * core::constants::TICKS; // die in 2 min
        ch[co].data[24] = 0; // do not interfere in fights
        ch[co].data[36] = 0; // no walking around
        ch[co].data[43] = 0; // don't attack anyone
        ch[co].data[80] = 0; // no enemies
        ch[co].data[0] = cn as i32; // person to make solve
        ch[co].data[1] = nr; // labnr
        ch[co].data[2] = exp; // exp plr is supposed to get
        ch[co].flags |= CharacterFlags::CF_LABKEEPER.bits() | CharacterFlags::CF_NOSLEEP.bits();
        ch[co].flags &= !CharacterFlags::CF_RESPAWN.bits();
    });

    // npc_add_enemy(co, cn, 1): make him attack the solver (assume function exists)
    driver::npc_add_enemy(co, cn, true);

    // god_transfer_char(cn, 174, 166): transfer player (assume function exists)
    if !God::transfer_char(cn, 174, 166) {
        State::with(|state| {
            state.do_character_log(
                cn,
                FontColor::Red,
                "Sorry, could not transfer you to your enemy.\n",
            );
            log::error!(
                "use_labtransfer: god_transfer_char({}, 174, 166) failed",
                Repository::with_characters(|ch| ch[cn].get_name().to_string())
            );
        });
        God::destroy_items(co);
        Repository::with_characters_mut(|ch| ch[co].used = core::constants::USE_EMPTY);
        return false;
    }
    // chlog(cn, "Entered Labkeeper room");
    true
}

/// Returns the monster class name for a given class number, or an error string if out of bounds.
pub fn get_class_name(nr: i32) -> &'static str {
    // List from C++ npc_class[]
    const NPC_CLASS: [&str; 77] = [
        "",
        "Weak Thief",
        "Thief",
        "Ghost",
        "Weak Skeleton",
        "Strong Skeleton",
        "Skeleton",
        "Outlaw",
        "Grolm Fighter",
        "Grolm Warrior",
        "Grolm Knight",
        "Lizard Youngster",
        "Lizard Youth",
        "Lizard Worker",
        "Lizard Fighter",
        "Lizard Warrior",
        "Lizard Mage",
        "Ratling",
        "Ratling Fighter",
        "Ratling Warrior",
        "Ratling Knight",
        "Ratling Baron",
        "Ratling Count",
        "Ratling Duke",
        "Ratling Prince",
        "Ratling King",
        "Spellcaster",
        "Knight",
        "Weak Golem",
        "Captain Gargoyle",
        "Undead",
        "Very Strong Ice Gargoyle",
        "Strong Outlaw",
        "Private Grolm",
        "PFC Grolm",
        "Lance Corp Grolm",
        "Corporal Grolm",
        "Sergeant Grolm",
        "Staff Sergeant Grolm",
        "Master Sergeant Grolm",
        "First Sergeant Grolm",
        "Sergeant Major Grolm",
        "2nd Lieutenant Grolm",
        "1st Lieutenant Grolm",
        "Major Gargoyle",
        "Lt. Colonel Gargoyle",
        "Colonel Gargoyle",
        "Brig. General Gargoyle",
        "Major General Gargoyle",
        "Lieutenant Gargoyle",
        "Weak Spider",
        "Spider",
        "Strong Spider",
        "Very Strong Outlaw",
        "Lizard Knight",
        "Lizard Archmage",
        "Undead Lord",
        "Undead King",
        "Very Weak Ice Gargoyle",
        "Strong Golem",
        "Strong Ghost",
        "Shiva",
        "Flame",
        "Weak Ice Gargoyle",
        "Ice Gargoyle",
        "Strong Ice Gargoyle",
        "Greenling",
        "Greenling Fighter",
        "Greenling Warrior",
        "Greenling Knight",
        "Greenling Baron",
        "Greenling Count",
        "Greenling Duke",
        "Greenling Prince",
        "Greenling King",
        "Strong Thief",
        "Major Grolm",
    ];
    if nr < 0 {
        return "err... nothing";
    }
    let nr = nr as usize;
    if nr >= NPC_CLASS.len() {
        return "umm... whatzit";
    }
    NPC_CLASS[nr]
}

/// Returns true if the class was already marked as killed, false if this is the first kill. Side effect: sets the bit for this class.
pub fn killed_class(cn: usize, val: i32) -> bool {
    Repository::with_characters_mut(|characters| {
        let (bit, data_idx) = if val < 32 {
            (1 << val, 60)
        } else if val < 64 {
            (1 << (val - 32), 61)
        } else if val < 96 {
            (1 << (val - 64), 62)
        } else {
            (1 << (val - 96), 63)
        };
        let tmp = characters[cn].data[data_idx] & bit;
        characters[cn].data[data_idx] |= bit;
        tmp != 0
    })
}

pub const WHO_RANK_NAME: [&str; core::constants::RANKS] = [
    " Pvt ", " PFC ", " LCp ", " Cpl ", " Sgt ", " SSg ", " MSg ", " 1Sg ", " SgM ", "2Lieu",
    "1Lieu", "Captn", "Major", "LtCol", "Colnl", "BrGen", "MaGen", "LtGen", "Genrl", "FDMAR",
    "KNIGT", "BARON", " EARL", "WARLD",
];

/// Return a human-friendly time-delta string from a tick-delta (ticks)
pub fn ago_string(dt: i32) -> String {
    let minutes = dt / (60 * core::constants::TICKS);
    if minutes <= 0 {
        return "just now".to_string();
    }
    if minutes < 60 {
        return format!("{} minutes ago", minutes);
    }
    let hours = minutes / 60;
    if hours <= 36 {
        return format!("{} hours ago", hours);
    }
    let days = hours / 24;
    if days <= 45 {
        return format!("{} days ago", days);
    }
    let months = days / 30;
    if months <= 24 {
        return format!("{} months ago", months);
    }
    let years = months / 12;
    format!("{} years ago", years)
}

pub const RANK_NAMES: [&str; core::constants::RANKS] = [
    "Private",
    "Private First Class",
    "Lance Corporal",
    "Corporal",
    "Sergeant",
    "Staff Sergeant",
    "Master Sergeant",
    "First Sergeant",
    "Sergeant Major",
    "Second Lieutenant",
    "First Lieutenant",
    "Captain",
    "Major",
    "Lieutenant Colonel",
    "Colonel",
    "Brigadier General",
    "Major General",
    "Lieutenant General",
    "General",
    "Field Marshal",
    "Knight",
    "Baron",
    "Earl",
    "Warlord",
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

pub fn invis_level(cn: usize) -> i32 {
    Repository::with_characters(|characters| {
        if characters[cn].flags & CharacterFlags::CF_GREATERINV.bits() != 0 {
            return 15;
        }
        if characters[cn].flags & CharacterFlags::CF_GOD.bits() != 0 {
            return 10;
        }
        if characters[cn].flags & (CharacterFlags::CF_IMP | CharacterFlags::CF_USURP).bits() != 0 {
            return 5;
        }
        if characters[cn].flags & CharacterFlags::CF_STAFF.bits() != 0 {
            return 2;
        }

        return 1;
    })
}

/// Helper function to calculate points needed to raise an attribute
/// Port of attrib_needed from svr_do.cpp
pub fn attrib_needed(v: i32, diff: i32) -> i32 {
    v * v * v * diff / 20
}

/// Helper function to calculate points needed to raise HP
/// Port of hp_needed from svr_do.cpp
pub fn hp_needed(v: i32, diff: i32) -> i32 {
    v * diff
}

/// Helper function to calculate points needed to raise endurance
/// Port of end_needed from svr_do.cpp
pub fn end_needed(v: i32, diff: i32) -> i32 {
    v * diff / 2
}

/// Helper function to calculate points needed to raise mana
/// Port of mana_needed from svr_do.cpp
pub fn mana_needed(v: i32, diff: i32) -> i32 {
    v * diff
}

/// Helper function to calculate points needed to raise a skill
/// Port of skill_needed from svr_do.cpp
pub fn skill_needed(v: i32, diff: i32) -> i32 {
    std::cmp::max(v, v * v * v * diff / 40)
}
