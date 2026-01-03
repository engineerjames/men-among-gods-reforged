use crate::enums::CharacterFlags;
use crate::path_finding::PathFinder;
use crate::player;
use crate::state::State;
use crate::Repository;
use crate::{core, driver};
use rand::Rng;

/// Notifies the area of the character's presence if the ticker matches.
///
/// # Arguments
///
/// * `cn` - Character number (index)
pub fn act_idle(cn: usize) {
    let should_notify = Repository::with_globals(|g| (g.ticker & 15) == (cn as i32 & 15));
    if should_notify {
        let (x, y) = Repository::with_characters(|ch| (ch[cn].x as i32, ch[cn].y as i32));
        State::with(|state| {
            state.do_area_notify(
                cn as i32,
                0,
                x,
                y,
                core::constants::NT_SEE as i32,
                cn as i32,
                0,
                0,
                0,
            );
        });
    }
}

/// Attempts to make the character drop (flee) in the direction they are facing.
///
/// # Arguments
///
/// * `cn` - Character number (index)
pub fn act_drop(cn: usize) {
    Repository::with_characters_mut(|ch| ch[cn].cerrno = core::constants::ERR_SUCCESS as u16);

    let cannot_flee = State::with(|s| s.do_char_can_flee(cn) == 0);
    let simple = Repository::with_characters(|ch| {
        (ch[cn].flags & CharacterFlags::Simple.bits() as u64) != 0
    });

    if cannot_flee || simple {
        Repository::with_characters_mut(|ch| ch[cn].cerrno = core::constants::ERR_FAILED as u16);
        return;
    }

    Repository::with_characters_mut(|ch| match ch[cn].dir {
        0 => {
            if ch[cn].y > 0 {
                ch[cn].status = 160;
                ch[cn].status2 = 2;
            } else {
                ch[cn].cerrno = core::constants::ERR_FAILED as u16;
            }
        }
        1 => {
            if ch[cn].y < (core::constants::SERVER_MAPY as i16 - 1) {
                ch[cn].status = 168;
                ch[cn].status2 = 2;
            } else {
                ch[cn].cerrno = core::constants::ERR_FAILED as u16;
            }
        }
        2 => {
            if ch[cn].x > 0 {
                ch[cn].status = 176;
                ch[cn].status2 = 2;
            } else {
                ch[cn].cerrno = core::constants::ERR_FAILED as u16;
            }
        }
        3 => {
            if ch[cn].x < (core::constants::SERVER_MAPX as i16 - 1) {
                ch[cn].status = 184;
                ch[cn].status2 = 2;
            } else {
                ch[cn].cerrno = core::constants::ERR_FAILED as u16;
            }
        }
        _ => ch[cn].cerrno = core::constants::ERR_FAILED as u16,
    });
}

/// Attempts to make the character use an item or interact in the direction they are facing.
///
/// # Arguments
///
/// * `cn` - Character number (index)
pub fn act_use(cn: usize) {
    Repository::with_characters_mut(|ch| ch[cn].cerrno = core::constants::ERR_SUCCESS as u16);

    let cannot_flee = State::with(|s| s.do_char_can_flee(cn) == 0);
    let simple = Repository::with_characters(|ch| {
        (ch[cn].flags & CharacterFlags::Simple.bits() as u64) != 0
    });

    if cannot_flee || simple {
        Repository::with_characters_mut(|ch| ch[cn].cerrno = core::constants::ERR_FAILED as u16);
        return;
    }

    Repository::with_characters_mut(|ch| match ch[cn].dir {
        0 => {
            if ch[cn].y > 0 {
                ch[cn].status = 160;
                ch[cn].status2 = 4;
            } else {
                ch[cn].cerrno = core::constants::ERR_FAILED as u16;
            }
        }
        1 => {
            if ch[cn].y < (core::constants::SERVER_MAPY as i16 - 1) {
                ch[cn].status = 168;
                ch[cn].status2 = 4;
            } else {
                ch[cn].cerrno = core::constants::ERR_FAILED as u16;
            }
        }
        2 => {
            if ch[cn].x > 0 {
                ch[cn].status = 176;
                ch[cn].status2 = 4;
            } else {
                ch[cn].cerrno = core::constants::ERR_FAILED as u16;
            }
        }
        3 => {
            if ch[cn].x < (core::constants::SERVER_MAPX as i16 - 1) {
                ch[cn].status = 184;
                ch[cn].status2 = 4;
            } else {
                ch[cn].cerrno = core::constants::ERR_FAILED as u16;
            }
        }
        _ => ch[cn].cerrno = core::constants::ERR_FAILED as u16,
    });
}

/// Attempts to make the character pick up an item in the direction they are facing.
///
/// # Arguments
///
/// * `cn` - Character number (index)
pub fn act_pickup(cn: usize) {
    let simple_initial = Repository::with_characters(|ch| {
        (ch[cn].flags & CharacterFlags::Simple.bits() as u64) != 0
    });
    if simple_initial {
        Repository::with_characters_mut(|ch| ch[cn].cerrno = core::constants::ERR_FAILED as u16);
        return;
    }

    Repository::with_characters_mut(|ch| ch[cn].cerrno = core::constants::ERR_SUCCESS as u16);

    let cannot_flee = State::with(|s| s.do_char_can_flee(cn) == 0);
    let simple = Repository::with_characters(|ch| {
        (ch[cn].flags & CharacterFlags::Simple.bits() as u64) != 0
    });

    if cannot_flee || simple {
        Repository::with_characters_mut(|ch| ch[cn].cerrno = core::constants::ERR_FAILED as u16);
        return;
    }

    Repository::with_characters_mut(|ch| match ch[cn].dir {
        0 => {
            if ch[cn].y > 0 {
                ch[cn].status = 160;
                ch[cn].status2 = 1;
            } else {
                ch[cn].cerrno = core::constants::ERR_FAILED as u16;
            }
        }
        1 => {
            if ch[cn].y < (core::constants::SERVER_MAPY as i16 - 1) {
                ch[cn].status = 168;
                ch[cn].status2 = 1;
            } else {
                ch[cn].cerrno = core::constants::ERR_FAILED as u16;
            }
        }
        2 => {
            if ch[cn].x > 0 {
                ch[cn].status = 176;
                ch[cn].status2 = 1;
            } else {
                ch[cn].cerrno = core::constants::ERR_FAILED as u16;
            }
        }
        3 => {
            if ch[cn].x < (core::constants::SERVER_MAPX as i16 - 1) {
                ch[cn].status = 184;
                ch[cn].status2 = 1;
            } else {
                ch[cn].cerrno = core::constants::ERR_FAILED as u16;
            }
        }
        _ => ch[cn].cerrno = core::constants::ERR_FAILED as u16,
    });
}

/// Attempts to make the character use a skill in the direction they are facing.
///
/// # Arguments
///
/// * `cn` - Character number (index)
pub fn act_skill(cn: usize) {
    let simple = Repository::with_characters(|ch| {
        (ch[cn].flags & CharacterFlags::Simple.bits() as u64) != 0
    });
    if simple {
        Repository::with_characters_mut(|ch| ch[cn].cerrno = core::constants::ERR_FAILED as u16);
        return;
    }

    Repository::with_characters_mut(|ch| ch[cn].cerrno = core::constants::ERR_SUCCESS as u16);

    Repository::with_characters_mut(|ch| match ch[cn].dir {
        0 => {
            ch[cn].status = 160;
            ch[cn].status2 = 9;
        }
        1 => {
            ch[cn].status = 168;
            ch[cn].status2 = 9;
        }
        2 => {
            ch[cn].status = 176;
            ch[cn].status2 = 9;
        }
        3 => {
            ch[cn].status = 184;
            ch[cn].status2 = 9;
        }
        _ => ch[cn].cerrno = core::constants::ERR_FAILED as u16,
    });
}

/// Attempts to make the character perform a wave action in the direction they are facing.
///
/// # Arguments
///
/// * `cn` - Character number (index)
pub fn act_wave(cn: usize) {
    let simple = Repository::with_characters(|ch| {
        (ch[cn].flags & CharacterFlags::Simple.bits() as u64) != 0
    });
    if simple {
        Repository::with_characters_mut(|ch| ch[cn].cerrno = core::constants::ERR_FAILED as u16);
        return;
    }

    Repository::with_characters_mut(|ch| ch[cn].cerrno = core::constants::ERR_SUCCESS as u16);

    Repository::with_characters_mut(|ch| match ch[cn].dir {
        0 => {
            if ch[cn].y > 0 {
                ch[cn].status = 160;
                ch[cn].status2 = 8;
            } else {
                ch[cn].cerrno = core::constants::ERR_FAILED as u16;
            }
        }
        1 => {
            if ch[cn].y < (core::constants::SERVER_MAPY as i16 - 1) {
                ch[cn].status = 168;
                ch[cn].status2 = 8;
            } else {
                ch[cn].cerrno = core::constants::ERR_FAILED as u16;
            }
        }
        2 => {
            if ch[cn].x > 0 {
                ch[cn].status = 176;
                ch[cn].status2 = 8;
            } else {
                ch[cn].cerrno = core::constants::ERR_FAILED as u16;
            }
        }
        3 => {
            if ch[cn].x < (core::constants::SERVER_MAPX as i16 - 1) {
                ch[cn].status = 184;
                ch[cn].status2 = 8;
            } else {
                ch[cn].cerrno = core::constants::ERR_FAILED as u16;
            }
        }
        _ => ch[cn].cerrno = core::constants::ERR_FAILED as u16,
    });
}

/// Attempts to make the character perform a bow action in the direction they are facing.
///
/// # Arguments
///
/// * `cn` - Character number (index)
pub fn act_bow(cn: usize) {
    let simple = Repository::with_characters(|ch| {
        (ch[cn].flags & CharacterFlags::Simple.bits() as u64) != 0
    });
    if simple {
        Repository::with_characters_mut(|ch| ch[cn].cerrno = core::constants::ERR_FAILED as u16);
        return;
    }

    Repository::with_characters_mut(|ch| ch[cn].cerrno = core::constants::ERR_SUCCESS as u16);

    Repository::with_characters_mut(|ch| match ch[cn].dir {
        0 => {
            if ch[cn].y > 0 {
                ch[cn].status = 160;
                ch[cn].status2 = 7;
            } else {
                ch[cn].cerrno = core::constants::ERR_FAILED as u16;
            }
        }
        1 => {
            if ch[cn].y < (core::constants::SERVER_MAPY as i16 - 1) {
                ch[cn].status = 168;
                ch[cn].status2 = 7;
            } else {
                ch[cn].cerrno = core::constants::ERR_FAILED as u16;
            }
        }
        2 => {
            if ch[cn].x > 0 {
                ch[cn].status = 176;
                ch[cn].status2 = 7;
            } else {
                ch[cn].cerrno = core::constants::ERR_FAILED as u16;
            }
        }
        3 => {
            if ch[cn].x < (core::constants::SERVER_MAPX as i16 - 1) {
                ch[cn].status = 184;
                ch[cn].status2 = 7;
            } else {
                ch[cn].cerrno = core::constants::ERR_FAILED as u16;
            }
        }
        _ => ch[cn].cerrno = core::constants::ERR_FAILED as u16,
    });
}

/// Attempts to make the character give an item in the direction they are facing.
///
/// # Arguments
///
/// * `cn` - Character number (index)
pub fn act_give(cn: usize) {
    let simple = Repository::with_characters(|ch| {
        (ch[cn].flags & CharacterFlags::Simple.bits() as u64) != 0
    });
    if simple {
        Repository::with_characters_mut(|ch| ch[cn].cerrno = core::constants::ERR_FAILED as u16);
        return;
    }

    Repository::with_characters_mut(|ch| ch[cn].cerrno = core::constants::ERR_SUCCESS as u16);

    Repository::with_characters_mut(|ch| match ch[cn].dir {
        0 => {
            if ch[cn].y > 0 {
                ch[cn].status = 160;
                ch[cn].status2 = 3;
            } else {
                ch[cn].cerrno = core::constants::ERR_FAILED as u16;
            }
        }
        1 => {
            if ch[cn].y < (core::constants::SERVER_MAPY as i16 - 1) {
                ch[cn].status = 168;
                ch[cn].status2 = 3;
            } else {
                ch[cn].cerrno = core::constants::ERR_FAILED as u16;
            }
        }
        2 => {
            if ch[cn].x > 0 {
                ch[cn].status = 176;
                ch[cn].status2 = 3;
            } else {
                ch[cn].cerrno = core::constants::ERR_FAILED as u16;
            }
        }
        3 => {
            if ch[cn].x < (core::constants::SERVER_MAPX as i16 - 1) {
                ch[cn].status = 184;
                ch[cn].status2 = 3;
            } else {
                ch[cn].cerrno = core::constants::ERR_FAILED as u16;
            }
        }
        _ => ch[cn].cerrno = core::constants::ERR_FAILED as u16,
    });
}

/// Attempts to make the character perform an attack in the direction they are facing.
///
/// # Arguments
///
/// * `cn` - Character number (index)
pub fn act_attack(cn: usize) {
    Repository::with_characters_mut(|ch| ch[cn].cerrno = core::constants::ERR_NONE as u16);

    let is_simple = Repository::with_characters(|ch| {
        (ch[cn].flags & CharacterFlags::Simple.bits() as u64) != 0
    });

    let mut v: i32;
    if !is_simple {
        let mut vv: i32;
        loop {
            vv = rand::thread_rng().gen_range(0..3);
            let last = Repository::with_characters(|ch| ch[cn].lastattack);
            if vv != last as i32 {
                break;
            }
        }
        Repository::with_characters_mut(|ch| ch[cn].lastattack = vv as i8);

        v = vv;
        if v != 0 {
            v += 4;
        }
    } else {
        v = 0;
    }

    Repository::with_characters_mut(|ch| match ch[cn].dir {
        d if d == core::constants::DX_UP => {
            if ch[cn].y > 0 {
                ch[cn].status = 160;
                ch[cn].status2 = v as i16;
            } else {
                ch[cn].cerrno = core::constants::ERR_FAILED as u16;
            }
        }
        d if d == core::constants::DX_DOWN => {
            if ch[cn].y < (core::constants::SERVER_MAPY as i16 - 1) {
                ch[cn].status = 168;
                ch[cn].status2 = v as i16;
            } else {
                ch[cn].cerrno = core::constants::ERR_FAILED as u16;
            }
        }
        d if d == core::constants::DX_LEFT => {
            if ch[cn].x > 0 {
                ch[cn].status = 176;
                ch[cn].status2 = v as i16;
            } else {
                ch[cn].cerrno = core::constants::ERR_FAILED as u16;
            }
        }
        d if d == core::constants::DX_RIGHT => {
            if ch[cn].x < (core::constants::SERVER_MAPX as i16 - 1) {
                ch[cn].status = 184;
                ch[cn].status2 = v as i16;
            } else {
                ch[cn].cerrno = core::constants::ERR_FAILED as u16;
            }
        }
        _ => ch[cn].cerrno = core::constants::ERR_FAILED as u16,
    });
}

/// Turns the character to the specified direction.
///
/// # Arguments
///
/// * `cn` - Character number (index)
/// * `dir` - Direction to turn to
pub fn act_turn(cn: usize, dir: i32) {
    Repository::with_characters_mut(|ch| ch[cn].cerrno = core::constants::ERR_NONE as u16);

    let same = Repository::with_characters(|ch| ch[cn].dir == dir as u8);
    if same {
        Repository::with_characters_mut(|ch| ch[cn].cerrno = core::constants::ERR_SUCCESS as u16);
        return;
    }

    match dir as u8 {
        d if d == core::constants::DX_UP => act_turn_up(cn),
        d if d == core::constants::DX_DOWN => act_turn_down(cn),
        d if d == core::constants::DX_RIGHT => act_turn_right(cn),
        d if d == core::constants::DX_LEFT => act_turn_left(cn),
        d if d == core::constants::DX_LEFTUP => act_turn_leftup(cn),
        d if d == core::constants::DX_LEFTDOWN => act_turn_leftdown(cn),
        d if d == core::constants::DX_RIGHTUP => act_turn_rightup(cn),
        d if d == core::constants::DX_RIGHTDOWN => act_turn_rightdown(cn),
        _ => {
            Repository::with_characters_mut(|ch| ch[cn].cerrno = core::constants::ERR_FAILED as u16)
        }
    }
}

pub fn act_turn_rightdown(cn: usize) {
    Repository::with_characters_mut(|ch| ch[cn].cerrno = core::constants::ERR_NONE as u16);

    let dir = Repository::with_characters(|ch| ch[cn].dir);
    if dir == core::constants::DX_LEFTUP {
        act_turn_up(cn);
    } else if dir == core::constants::DX_UP {
        act_turn_rightup(cn);
    } else if dir == core::constants::DX_LEFT {
        act_turn_leftdown(cn);
    } else if dir == core::constants::DX_LEFTDOWN {
        act_turn_down(cn);
    } else if dir == core::constants::DX_RIGHTUP {
        act_turn_right(cn);
    } else if dir == core::constants::DX_DOWN {
        Repository::with_characters_mut(|ch| ch[cn].status = 120);
    } else {
        Repository::with_characters_mut(|ch| ch[cn].status = 152);
    }
}

pub fn act_turn_rightup(cn: usize) {
    Repository::with_characters_mut(|ch| ch[cn].cerrno = core::constants::ERR_NONE as u16);

    let dir = Repository::with_characters(|ch| ch[cn].dir);
    if dir == core::constants::DX_LEFTDOWN {
        act_turn_down(cn);
    } else if dir == core::constants::DX_DOWN {
        act_turn_rightdown(cn);
    } else if dir == core::constants::DX_LEFT {
        act_turn_leftup(cn);
    } else if dir == core::constants::DX_LEFTUP {
        act_turn_up(cn);
    } else if dir == core::constants::DX_RIGHTDOWN {
        act_turn_right(cn);
    } else if dir == core::constants::DX_UP {
        Repository::with_characters_mut(|ch| ch[cn].status = 104);
    } else {
        Repository::with_characters_mut(|ch| ch[cn].status = 144);
    }
}

pub fn act_turn_leftdown(cn: usize) {
    Repository::with_characters_mut(|ch| ch[cn].cerrno = core::constants::ERR_NONE as u16);

    let dir = Repository::with_characters(|ch| ch[cn].dir);
    if dir == core::constants::DX_RIGHTUP {
        act_turn_up(cn);
    } else if dir == core::constants::DX_UP {
        act_turn_leftup(cn);
    } else if dir == core::constants::DX_RIGHT {
        act_turn_rightdown(cn);
    } else if dir == core::constants::DX_RIGHTDOWN {
        act_turn_down(cn);
    } else if dir == core::constants::DX_LEFTUP {
        act_turn_left(cn);
    } else if dir == core::constants::DX_DOWN {
        Repository::with_characters_mut(|ch| ch[cn].status = 112);
    } else {
        Repository::with_characters_mut(|ch| ch[cn].status = 136);
    }
}

pub fn act_turn_leftup(cn: usize) {
    Repository::with_characters_mut(|ch| ch[cn].cerrno = core::constants::ERR_NONE as u16);

    let dir = Repository::with_characters(|ch| ch[cn].dir);
    if dir == core::constants::DX_RIGHTDOWN {
        act_turn_down(cn);
    } else if dir == core::constants::DX_DOWN {
        act_turn_leftdown(cn);
    } else if dir == core::constants::DX_RIGHT {
        act_turn_rightup(cn);
    } else if dir == core::constants::DX_RIGHTUP {
        act_turn_up(cn);
    } else if dir == core::constants::DX_LEFTDOWN {
        act_turn_left(cn);
    } else if dir == core::constants::DX_UP {
        Repository::with_characters_mut(|ch| ch[cn].status = 96);
    } else {
        Repository::with_characters_mut(|ch| ch[cn].status = 128);
    }
}

pub fn act_turn_right(cn: usize) {
    Repository::with_characters_mut(|ch| ch[cn].cerrno = core::constants::ERR_NONE as u16);

    let dir = Repository::with_characters(|ch| ch[cn].dir);
    if dir == core::constants::DX_LEFT {
        act_turn_leftdown(cn);
    } else if dir == core::constants::DX_LEFTUP {
        act_turn_up(cn);
    } else if dir == core::constants::DX_LEFTDOWN {
        act_turn_down(cn);
    } else if dir == core::constants::DX_UP {
        act_turn_rightup(cn);
    } else if dir == core::constants::DX_DOWN {
        act_turn_rightdown(cn);
    } else if dir == core::constants::DX_RIGHTUP {
        Repository::with_characters_mut(|ch| ch[cn].status = 108);
    } else {
        Repository::with_characters_mut(|ch| ch[cn].status = 124);
    }
}

pub fn act_turn_left(cn: usize) {
    Repository::with_characters_mut(|ch| ch[cn].cerrno = core::constants::ERR_NONE as u16);

    let dir = Repository::with_characters(|ch| ch[cn].dir);
    if dir == core::constants::DX_RIGHT {
        act_turn_rightup(cn);
    } else if dir == core::constants::DX_RIGHTUP {
        act_turn_up(cn);
    } else if dir == core::constants::DX_RIGHTDOWN {
        act_turn_down(cn);
    } else if dir == core::constants::DX_UP {
        act_turn_leftup(cn);
    } else if dir == core::constants::DX_DOWN {
        act_turn_leftdown(cn);
    } else if dir == core::constants::DX_LEFTUP {
        Repository::with_characters_mut(|ch| ch[cn].status = 100);
    } else {
        Repository::with_characters_mut(|ch| ch[cn].status = 116);
    }
}

pub fn act_turn_down(cn: usize) {
    Repository::with_characters_mut(|ch| ch[cn].cerrno = core::constants::ERR_NONE as u16);

    let dir = Repository::with_characters(|ch| ch[cn].dir);
    if dir == core::constants::DX_UP {
        act_turn_leftup(cn);
    } else if dir == core::constants::DX_LEFTUP {
        act_turn_left(cn);
    } else if dir == core::constants::DX_RIGHTUP {
        act_turn_right(cn);
    } else if dir == core::constants::DX_LEFT {
        act_turn_leftdown(cn);
    } else if dir == core::constants::DX_RIGHT {
        act_turn_rightdown(cn);
    } else if dir == core::constants::DX_LEFTDOWN {
        Repository::with_characters_mut(|ch| ch[cn].status = 140);
    } else {
        Repository::with_characters_mut(|ch| ch[cn].status = 156);
    }
}

pub fn act_turn_up(cn: usize) {
    Repository::with_characters_mut(|ch| ch[cn].cerrno = core::constants::ERR_NONE as u16);

    let dir = Repository::with_characters(|ch| ch[cn].dir);

    if dir == core::constants::DX_DOWN {
        act_turn_rightdown(cn);
    } else if dir == core::constants::DX_LEFTDOWN {
        act_turn_left(cn);
    } else if dir == core::constants::DX_RIGHTDOWN {
        act_turn_right(cn);
    } else if dir == core::constants::DX_LEFT {
        act_turn_leftup(cn);
    } else if dir == core::constants::DX_RIGHT {
        act_turn_rightup(cn);
    } else if dir == core::constants::DX_LEFTUP {
        Repository::with_characters_mut(|ch| ch[cn].status = 132);
    } else {
        Repository::with_characters_mut(|ch| ch[cn].status = 148);
    }
}

pub fn act_move_rightdown(cn: usize) {
    Repository::with_characters_mut(|ch| ch[cn].cerrno = core::constants::ERR_NONE as u16);

    let (x, y, dir) =
        Repository::with_characters(|ch| (ch[cn].x as i32, ch[cn].y as i32, ch[cn].dir));

    if x >= core::constants::SERVER_MAPX - 2 {
        Repository::with_characters_mut(|ch| ch[cn].cerrno = core::constants::ERR_FAILED as u16);
        return;
    }
    if y >= core::constants::SERVER_MAPY - 2 {
        Repository::with_characters_mut(|ch| ch[cn].cerrno = core::constants::ERR_FAILED as u16);
        return;
    }
    if dir != core::constants::DX_RIGHTDOWN {
        Repository::with_characters_mut(|ch| ch[cn].cerrno = core::constants::ERR_FAILED as u16);
        return;
    }

    if State::with(|s| s.do_char_can_flee(cn) == 0) {
        Repository::with_characters_mut(|ch| ch[cn].cerrno = core::constants::ERR_FAILED as u16);
        return;
    }

    let base = (x + y * core::constants::SERVER_MAPX) as i32;
    let m1 = (base + core::constants::SERVER_MAPX) as usize;
    let m2 = (base + 1) as usize;
    let target = (base + core::constants::SERVER_MAPX + 1) as usize;

    if !player::plr_check_target(m1) {
        Repository::with_characters_mut(|ch| ch[cn].cerrno = core::constants::ERR_FAILED as u16);
        return;
    }
    if !player::plr_check_target(m2) {
        Repository::with_characters_mut(|ch| ch[cn].cerrno = core::constants::ERR_FAILED as u16);
        return;
    }
    if !player::plr_set_target(target, cn) {
        Repository::with_characters_mut(|ch| ch[cn].cerrno = core::constants::ERR_FAILED as u16);
        return;
    }

    Repository::with_characters_mut(|ch| {
        ch[cn].status = 84;
        ch[cn].tox = (x + 1) as i16;
        ch[cn].toy = (y + 1) as i16;
    });
}

pub fn act_move_rightup(cn: usize) {
    Repository::with_characters_mut(|ch| ch[cn].cerrno = core::constants::ERR_NONE as u16);

    let (x, y, dir) =
        Repository::with_characters(|ch| (ch[cn].x as i32, ch[cn].y as i32, ch[cn].dir));

    if x >= core::constants::SERVER_MAPX - 2 {
        Repository::with_characters_mut(|ch| ch[cn].cerrno = core::constants::ERR_FAILED as u16);
        return;
    }
    if y < 1 {
        Repository::with_characters_mut(|ch| ch[cn].cerrno = core::constants::ERR_FAILED as u16);
        return;
    }
    if dir != core::constants::DX_RIGHTUP {
        Repository::with_characters_mut(|ch| ch[cn].cerrno = core::constants::ERR_FAILED as u16);
        return;
    }

    if State::with(|s| s.do_char_can_flee(cn) == 0) {
        Repository::with_characters_mut(|ch| ch[cn].cerrno = core::constants::ERR_FAILED as u16);
        return;
    }

    let base = (x + y * core::constants::SERVER_MAPX) as i32;
    let m1 = (base - core::constants::SERVER_MAPX) as usize;
    let m2 = (base + 1) as usize;
    let target = (base - core::constants::SERVER_MAPX + 1) as usize;

    if !player::plr_check_target(m1) {
        Repository::with_characters_mut(|ch| ch[cn].cerrno = core::constants::ERR_FAILED as u16);
        return;
    }
    if !player::plr_check_target(m2) {
        Repository::with_characters_mut(|ch| ch[cn].cerrno = core::constants::ERR_FAILED as u16);
        return;
    }
    if !player::plr_set_target(target, cn) {
        Repository::with_characters_mut(|ch| ch[cn].cerrno = core::constants::ERR_FAILED as u16);
        return;
    }

    Repository::with_characters_mut(|ch| {
        ch[cn].status = 72;
        ch[cn].tox = (x + 1) as i16;
        ch[cn].toy = (y - 1) as i16;
    });
}

pub fn act_move_leftdown(cn: usize) {
    Repository::with_characters_mut(|ch| ch[cn].cerrno = core::constants::ERR_NONE as u16);

    let (x, y, dir) =
        Repository::with_characters(|ch| (ch[cn].x as i32, ch[cn].y as i32, ch[cn].dir));

    if x < 1 {
        Repository::with_characters_mut(|ch| ch[cn].cerrno = core::constants::ERR_FAILED as u16);
        return;
    }
    if y >= core::constants::SERVER_MAPY - 2 {
        Repository::with_characters_mut(|ch| ch[cn].cerrno = core::constants::ERR_FAILED as u16);
        return;
    }
    if dir != core::constants::DX_LEFTDOWN {
        Repository::with_characters_mut(|ch| ch[cn].cerrno = core::constants::ERR_FAILED as u16);
        return;
    }

    if State::with(|s| s.do_char_can_flee(cn) == 0) {
        Repository::with_characters_mut(|ch| ch[cn].cerrno = core::constants::ERR_FAILED as u16);
        return;
    }

    let base = (x + y * core::constants::SERVER_MAPX) as i32;
    let m1 = (base + core::constants::SERVER_MAPX) as usize;
    let m2 = (base - 1) as usize;
    let target = (base + core::constants::SERVER_MAPX - 1) as usize;

    if !player::plr_check_target(m1) {
        Repository::with_characters_mut(|ch| ch[cn].cerrno = core::constants::ERR_FAILED as u16);
        return;
    }
    if !player::plr_check_target(m2) {
        Repository::with_characters_mut(|ch| ch[cn].cerrno = core::constants::ERR_FAILED as u16);
        return;
    }
    if !player::plr_set_target(target, cn) {
        Repository::with_characters_mut(|ch| ch[cn].cerrno = core::constants::ERR_FAILED as u16);
        return;
    }

    Repository::with_characters_mut(|ch| {
        ch[cn].status = 60;
        ch[cn].tox = (x - 1) as i16;
        ch[cn].toy = (y + 1) as i16;
    });
}

pub fn act_move_leftup(cn: usize) {
    Repository::with_characters_mut(|ch| ch[cn].cerrno = core::constants::ERR_NONE as u16);

    let (x, y, dir) =
        Repository::with_characters(|ch| (ch[cn].x as i32, ch[cn].y as i32, ch[cn].dir));

    if x < 1 {
        Repository::with_characters_mut(|ch| ch[cn].cerrno = core::constants::ERR_FAILED as u16);
        return;
    }
    if y < 1 {
        Repository::with_characters_mut(|ch| ch[cn].cerrno = core::constants::ERR_FAILED as u16);
        return;
    }
    if dir != core::constants::DX_LEFTUP {
        Repository::with_characters_mut(|ch| ch[cn].cerrno = core::constants::ERR_FAILED as u16);
        return;
    }

    if State::with(|s| s.do_char_can_flee(cn) == 0) {
        Repository::with_characters_mut(|ch| ch[cn].cerrno = core::constants::ERR_FAILED as u16);
        return;
    }

    let base = (x + y * core::constants::SERVER_MAPX) as i32;
    let m1 = (base - core::constants::SERVER_MAPX) as usize;
    let m2 = (base - 1) as usize;
    let target = (base - core::constants::SERVER_MAPX - 1) as usize;

    if !player::plr_check_target(m1) {
        Repository::with_characters_mut(|ch| ch[cn].cerrno = core::constants::ERR_FAILED as u16);
        return;
    }
    if !player::plr_check_target(m2) {
        Repository::with_characters_mut(|ch| ch[cn].cerrno = core::constants::ERR_FAILED as u16);
        return;
    }
    if !player::plr_set_target(target, cn) {
        Repository::with_characters_mut(|ch| ch[cn].cerrno = core::constants::ERR_FAILED as u16);
        return;
    }

    Repository::with_characters_mut(|ch| {
        ch[cn].status = 48;
        ch[cn].tox = (x - 1) as i16;
        ch[cn].toy = (y - 1) as i16;
    });
}

pub fn act_move_right(cn: usize) {
    Repository::with_characters_mut(|ch| ch[cn].cerrno = core::constants::ERR_NONE as u16);

    let (x, y, dir) =
        Repository::with_characters(|ch| (ch[cn].x as i32, ch[cn].y as i32, ch[cn].dir));

    if x >= core::constants::SERVER_MAPX - 2 {
        Repository::with_characters_mut(|ch| ch[cn].cerrno = core::constants::ERR_FAILED as u16);
        return;
    }
    if dir != core::constants::DX_RIGHT {
        Repository::with_characters_mut(|ch| ch[cn].cerrno = core::constants::ERR_FAILED as u16);
        return;
    }

    if State::with(|s| s.do_char_can_flee(cn) == 0) {
        Repository::with_characters_mut(|ch| ch[cn].cerrno = core::constants::ERR_FAILED as u16);
        return;
    }

    let base = (x + y * core::constants::SERVER_MAPX) as i32;
    let target = (base + 1) as usize;

    if !player::plr_set_target(target, cn) {
        Repository::with_characters_mut(|ch| ch[cn].cerrno = core::constants::ERR_FAILED as u16);
        return;
    }

    Repository::with_characters_mut(|ch| {
        ch[cn].status = 40;
        ch[cn].tox = (x + 1) as i16;
        ch[cn].toy = y as i16;
    });
}

pub fn act_move_left(cn: usize) {
    Repository::with_characters_mut(|ch| ch[cn].cerrno = core::constants::ERR_NONE as u16);

    let (x, y, dir) =
        Repository::with_characters(|ch| (ch[cn].x as i32, ch[cn].y as i32, ch[cn].dir));

    if x < 1 {
        Repository::with_characters_mut(|ch| ch[cn].cerrno = core::constants::ERR_FAILED as u16);
        return;
    }
    if dir != core::constants::DX_LEFT {
        Repository::with_characters_mut(|ch| ch[cn].cerrno = core::constants::ERR_FAILED as u16);
        return;
    }

    if State::with(|s| s.do_char_can_flee(cn) == 0) {
        Repository::with_characters_mut(|ch| ch[cn].cerrno = core::constants::ERR_FAILED as u16);
        return;
    }

    let base = (x + y * core::constants::SERVER_MAPX) as i32;
    let target = (base - 1) as usize;

    if !player::plr_set_target(target, cn) {
        Repository::with_characters_mut(|ch| ch[cn].cerrno = core::constants::ERR_FAILED as u16);
        return;
    }

    Repository::with_characters_mut(|ch| {
        ch[cn].status = 32;
        ch[cn].tox = (x - 1) as i16;
        ch[cn].toy = y as i16;
    });
}

pub fn act_move_down(cn: usize) {
    Repository::with_characters_mut(|ch| ch[cn].cerrno = core::constants::ERR_NONE as u16);

    let (x, y, dir) =
        Repository::with_characters(|ch| (ch[cn].x as i32, ch[cn].y as i32, ch[cn].dir));

    if y >= core::constants::SERVER_MAPY - 2 {
        Repository::with_characters_mut(|ch| ch[cn].cerrno = core::constants::ERR_FAILED as u16);
        return;
    }
    if dir != core::constants::DX_DOWN {
        Repository::with_characters_mut(|ch| ch[cn].cerrno = core::constants::ERR_FAILED as u16);
        return;
    }

    if State::with(|s| s.do_char_can_flee(cn) == 0) {
        Repository::with_characters_mut(|ch| ch[cn].cerrno = core::constants::ERR_FAILED as u16);
        return;
    }

    let base = (x + y * core::constants::SERVER_MAPX) as i32;
    let target = (base + core::constants::SERVER_MAPX) as usize;

    if !player::plr_set_target(target, cn) {
        Repository::with_characters_mut(|ch| ch[cn].cerrno = core::constants::ERR_FAILED as u16);
        return;
    }

    Repository::with_characters_mut(|ch| {
        ch[cn].status = 24;
        ch[cn].tox = x as i16;
        ch[cn].toy = (y + 1) as i16;
    });
}

pub fn act_move_up(cn: usize) {
    Repository::with_characters_mut(|ch| ch[cn].cerrno = core::constants::ERR_NONE as u16);

    let (x, y, dir) =
        Repository::with_characters(|ch| (ch[cn].x as i32, ch[cn].y as i32, ch[cn].dir));

    if y < 1 {
        Repository::with_characters_mut(|ch| ch[cn].cerrno = core::constants::ERR_FAILED as u16);
        return;
    }
    if dir != core::constants::DX_UP {
        Repository::with_characters_mut(|ch| ch[cn].cerrno = core::constants::ERR_FAILED as u16);
        return;
    }

    if State::with(|s| s.do_char_can_flee(cn) == 0) {
        Repository::with_characters_mut(|ch| ch[cn].cerrno = core::constants::ERR_FAILED as u16);
        return;
    }

    let base = (x + y * core::constants::SERVER_MAPX) as i32;
    let target = (base - core::constants::SERVER_MAPX) as usize;

    if !player::plr_set_target(target, cn) {
        Repository::with_characters_mut(|ch| ch[cn].cerrno = core::constants::ERR_FAILED as u16);
        return;
    }

    Repository::with_characters_mut(|ch| {
        ch[cn].status = 16;
        ch[cn].tox = x as i16;
        ch[cn].toy = (y - 1) as i16;
    });
}

pub fn char_give_char(cn: usize, co: usize) -> i32 {
    // Port of C++ char_give_char
    // quick error checks
    let cerrno = Repository::with_characters(|ch| ch[cn].cerrno);
    if cerrno == core::constants::ERR_FAILED as u16 {
        Repository::with_characters_mut(|ch| ch[cn].cerrno = core::constants::ERR_NONE as u16);
        return -1;
    }

    let co_used = Repository::with_characters(|ch| ch[co].used);
    let can_see = State::with_mut(|state| state.do_char_can_see(cn, co));
    if co_used != core::constants::USE_ACTIVE as u8 || can_see == 0 || cn == co {
        return -1;
    }

    let citem = Repository::with_characters(|ch| ch[cn].citem != 0);
    if !citem {
        return 1;
    }

    let (x, tox, y, toy, ax, ay) = Repository::with_characters(|ch| {
        (
            ch[co].x as i32,
            ch[co].tox as i32,
            ch[co].y as i32,
            ch[co].toy as i32,
            ch[cn].x as i32,
            ch[cn].y as i32,
        )
    });

    if (x == ax + 1 && (y == ay + 1 || y == ay - 1))
        || (x == ax - 1 && (y == ay + 1 || y == ay - 1))
    {
        let err = char_moveto(cn, x, y, 2, tox, toy);
        if err == -1 {
            return -1;
        } else {
            return 0;
        }
    }

    // give if possible
    if (ax == x - 1 && ay == y) || (ax == tox - 1 && ay == toy) {
        if Repository::with_characters(|ch| ch[cn].dir as i32) != core::constants::DX_RIGHT as i32 {
            act_turn_right(cn);
            return 0;
        }
        act_give(cn);
        return 0;
    }
    if (ax == x + 1 && ay == y) || (ax == tox + 1 && ay == toy) {
        if Repository::with_characters(|ch| ch[cn].dir as i32) != core::constants::DX_LEFT as i32 {
            act_turn_left(cn);
            return 0;
        }
        act_give(cn);
        return 0;
    }
    if (ax == x && ay == y - 1) || (ax == tox && ay == toy - 1) {
        if Repository::with_characters(|ch| ch[cn].dir as i32) != core::constants::DX_DOWN as i32 {
            act_turn_down(cn);
            return 0;
        }
        act_give(cn);
        return 0;
    }
    if (ax == x && ay == y + 1) || (ax == tox && ay == toy + 1) {
        if Repository::with_characters(|ch| ch[cn].dir as i32) != core::constants::DX_UP as i32 {
            act_turn_up(cn);
            return 0;
        }
        act_give(cn);
        return 0;
    }

    let err = char_moveto(cn, x, y, 2, tox, toy);
    if err == -1 {
        return -1;
    } else {
        return 0;
    }
}

pub fn char_attack_char(cn: usize, co: usize) -> i32 {
    // Port of C++ char_attack_char
    let cerrno = Repository::with_characters(|ch| ch[cn].cerrno);
    if cerrno == core::constants::ERR_FAILED as u16 {
        Repository::with_characters_mut(|ch| ch[cn].cerrno = core::constants::ERR_NONE as u16);
        return -1;
    }

    let co_used = Repository::with_characters(|ch| ch[co].used);
    let can_see = State::with_mut(|state| state.do_char_can_see(cn, co));
    let co_flags = Repository::with_characters(|ch| ch[co].flags);
    if co_used != core::constants::USE_ACTIVE as u8
        || can_see == 0
        || cn == co
        || (co_flags & CharacterFlags::Body.bits() as u64) != 0
        || (co_flags & CharacterFlags::Stoned.bits() as u64) != 0
    {
        return -1;
    }

    let (x, tox, y, toy, ax, ay) = Repository::with_characters(|ch| {
        (
            ch[co].x as i32,
            ch[co].tox as i32,
            ch[co].y as i32,
            ch[co].toy as i32,
            ch[cn].x as i32,
            ch[cn].y as i32,
        )
    });

    // diagonal adjacency
    if (x == ax + 1 && (y == ay + 1 || y == ay - 1))
        || (x == ax - 1 && (y == ay + 1 || y == ay - 1))
    {
        let err = char_moveto(cn, x, y, 2, tox, toy);
        if err == -1 {
            return -1;
        } else {
            return 0;
        }
    }

    // attack if possible
    if (ax == x - 1 && ay == y) || (ax == tox - 1 && ay == toy) {
        if Repository::with_characters(|ch| ch[cn].dir as i32) != core::constants::DX_RIGHT as i32 {
            act_turn_right(cn);
            return 0;
        }
        act_attack(cn);
        return 1;
    }
    if (ax == x + 1 && ay == y) || (ax == tox + 1 && ay == toy) {
        if Repository::with_characters(|ch| ch[cn].dir as i32) != core::constants::DX_LEFT as i32 {
            act_turn_left(cn);
            return 0;
        }
        act_attack(cn);
        return 1;
    }
    if (ax == x && ay == y - 1) || (ax == tox && ay == toy - 1) {
        if Repository::with_characters(|ch| ch[cn].dir as i32) != core::constants::DX_DOWN as i32 {
            act_turn_down(cn);
            return 0;
        }
        act_attack(cn);
        return 1;
    }
    if (ax == x && ay == y + 1) || (ax == tox && ay == toy + 1) {
        if Repository::with_characters(|ch| ch[cn].dir as i32) != core::constants::DX_UP as i32 {
            act_turn_up(cn);
            return 0;
        }
        act_attack(cn);
        return 1;
    }

    let dist1 = (ax - x).abs() + (ay - y).abs();
    let dist2 = (ax - tox).abs() + (ay - toy).abs();
    let diff = dist1 - dist2;

    let mut nx = x;
    let mut ntx = tox;
    let mut ny = y;
    let mut nty = toy;

    if dist1 > 20 && diff < 5 {
        nx = ntx + (ntx - x) * 8;
        ny = nty + (nty - y) * 8;
        ntx = nx;
        nty = ny;
    } else if dist1 > 10 && diff < 4 {
        nx = ntx + (ntx - x) * 5;
        ny = nty + (nty - y) * 5;
        ntx = nx;
        nty = ny;
    } else if dist1 > 5 && diff < 3 {
        nx = ntx + (ntx - x) * 3;
        ny = nty + (nty - y) * 3;
        ntx = nx;
        nty = ny;
    } else if dist1 > 3 && diff < 2 {
        nx = ntx + (ntx - x) * 2;
        ny = nty + (nty - y) * 2;
        ntx = nx;
        nty = ny;
    } else if dist1 > 2 && diff < 1 {
        nx = ntx + (ntx - x);
        ny = nty + (nty - y);
        ntx = nx;
        nty = ny;
    }

    let err = char_moveto(cn, nx, ny, 2, ntx, nty);
    if err == -1 {
        return -1;
    } else {
        return 0;
    }
}

pub fn char_dropto(cn: usize, x: i32, y: i32) -> i32 {
    // Port of C++ char_dropto
    let cerrno = Repository::with_characters(|ch| ch[cn].cerrno);
    if cerrno == core::constants::ERR_FAILED as u16 {
        Repository::with_characters_mut(|ch| ch[cn].cerrno = core::constants::ERR_NONE as u16);
        return -1;
    }

    // nothing to drop?
    let has_citem = Repository::with_characters(|ch| ch[cn].citem != 0);
    if !has_citem {
        return -1;
    }

    let cx = Repository::with_characters(|ch| ch[cn].x as i32);
    let cy = Repository::with_characters(|ch| ch[cn].y as i32);
    if cx == x - 1 && cy == y {
        if Repository::with_characters(|ch| ch[cn].dir as i32) != core::constants::DX_RIGHT as i32 {
            act_turn_right(cn);
            return 0;
        }
        act_drop(cn);
        return 1;
    }
    if cx == x + 1 && cy == y {
        if Repository::with_characters(|ch| ch[cn].dir as i32) != core::constants::DX_LEFT as i32 {
            act_turn_left(cn);
            return 0;
        }
        act_drop(cn);
        return 1;
    }
    if cx == x && cy == y - 1 {
        if Repository::with_characters(|ch| ch[cn].dir as i32) != core::constants::DX_DOWN as i32 {
            act_turn_down(cn);
            return 0;
        }
        act_drop(cn);
        return 1;
    }
    if cx == x && cy == y + 1 {
        if Repository::with_characters(|ch| ch[cn].dir as i32) != core::constants::DX_UP as i32 {
            act_turn_up(cn);
            return 0;
        }
        act_drop(cn);
        return 1;
    }

    // we're too far away... go there:
    if char_moveto(cn, x, y, 1, 0, 0) == -1 {
        return -1;
    }
    0
}

pub fn char_pickup(cn: usize, x: i32, y: i32) -> i32 {
    let cerrno = Repository::with_characters(|ch| ch[cn].cerrno);
    if cerrno == core::constants::ERR_FAILED as u16 {
        Repository::with_characters_mut(|ch| ch[cn].cerrno = core::constants::ERR_NONE as u16);
        return -1;
    }

    let cx = Repository::with_characters(|ch| ch[cn].x as i32);
    let cy = Repository::with_characters(|ch| ch[cn].y as i32);

    if cx == x - 1 && cy == y {
        if Repository::with_characters(|ch| ch[cn].dir as i32) != core::constants::DX_RIGHT as i32 {
            act_turn_right(cn);
            return 0;
        }
        act_pickup(cn);
        return 1;
    }
    if cx == x + 1 && cy == y {
        if Repository::with_characters(|ch| ch[cn].dir as i32) != core::constants::DX_LEFT as i32 {
            act_turn_left(cn);
            return 0;
        }
        act_pickup(cn);
        return 1;
    }
    if cx == x && cy == y - 1 {
        if Repository::with_characters(|ch| ch[cn].dir as i32) != core::constants::DX_DOWN as i32 {
            act_turn_down(cn);
            return 0;
        }
        act_pickup(cn);
        return 1;
    }
    if cx == x && cy == y + 1 {
        if Repository::with_characters(|ch| ch[cn].dir as i32) != core::constants::DX_UP as i32 {
            act_turn_up(cn);
            return 0;
        }
        act_pickup(cn);
        return 1;
    }

    -1
}

pub fn char_pickupto(cn: usize, x: i32, y: i32) -> i32 {
    // Port of C++ char_pickupto
    let cerrno = Repository::with_characters(|ch| ch[cn].cerrno);
    if cerrno == core::constants::ERR_FAILED as u16 {
        Repository::with_characters_mut(|ch| ch[cn].cerrno = core::constants::ERR_NONE as u16);
        return -1;
    }

    // already an item in hand?
    let has_citem = Repository::with_characters(|ch| ch[cn].citem != 0);
    if has_citem {
        return -1;
    }

    let ret = char_pickup(cn, x, y);
    if ret == -1 {
        if char_moveto(cn, x, y, 1, 0, 0) == -1 {
            return -1;
        } else {
            return 0;
        }
    }
    if ret == 1 {
        return 1;
    }
    0
}

pub fn char_use(cn: usize, x: i32, y: i32) -> i32 {
    let cerrno = Repository::with_characters(|ch| ch[cn].cerrno);
    if cerrno == core::constants::ERR_FAILED as u16 {
        Repository::with_characters_mut(|ch| ch[cn].cerrno = core::constants::ERR_NONE as u16);
        return -1;
    }

    let cx = Repository::with_characters(|ch| ch[cn].x as i32);
    let cy = Repository::with_characters(|ch| ch[cn].y as i32);

    if cx == x - 1 && cy == y {
        if Repository::with_characters(|ch| ch[cn].dir as i32) != core::constants::DX_RIGHT as i32 {
            act_turn_right(cn);
            return 0;
        }
        act_use(cn);
        return 1;
    }
    if cx == x + 1 && cy == y {
        if Repository::with_characters(|ch| ch[cn].dir as i32) != core::constants::DX_LEFT as i32 {
            act_turn_left(cn);
            return 0;
        }
        act_use(cn);
        return 1;
    }
    if cx == x && cy == y - 1 {
        if Repository::with_characters(|ch| ch[cn].dir as i32) != core::constants::DX_DOWN as i32 {
            act_turn_down(cn);
            return 0;
        }
        act_use(cn);
        return 1;
    }
    if cx == x && cy == y + 1 {
        if Repository::with_characters(|ch| ch[cn].dir as i32) != core::constants::DX_UP as i32 {
            act_turn_up(cn);
            return 0;
        }
        act_use(cn);
        return 1;
    }

    -1
}

pub fn char_useto(cn: usize, x: i32, y: i32) -> i32 {
    // Port of C++ char_useto
    let cerrno = Repository::with_characters(|ch| ch[cn].cerrno);
    if cerrno == core::constants::ERR_FAILED as u16 {
        Repository::with_characters_mut(|ch| ch[cn].cerrno = core::constants::ERR_NONE as u16);
        return -1;
    }

    let ret = char_use(cn, x, y);
    if ret == -1 {
        if char_moveto(cn, x, y, 1, 0, 0) == -1 {
            return -1;
        } else {
            return 0;
        }
    }
    if ret == 1 {
        return 1;
    }
    0
}

pub fn char_moveto(cn: usize, x: i32, y: i32, flag: i32, x2: i32, y2: i32) -> i32 {
    // Port of C++ char_moveto
    let (cx, cy) = Repository::with_characters(|ch| (ch[cn].x as i32, ch[cn].y as i32));
    if cx == x && cy == y && flag != 1 && flag != 3 {
        return 1;
    }

    let cerrno = Repository::with_characters(|ch| ch[cn].cerrno);
    if cerrno == core::constants::ERR_FAILED as u16 {
        Repository::with_characters_mut(|ch| ch[cn].cerrno = core::constants::ERR_NONE as u16);
        return -1;
    }

    let unreach = Repository::with_characters(|ch| ch[cn].unreach);
    let unreachx = Repository::with_characters(|ch| ch[cn].unreachx as i32);
    let unreachy = Repository::with_characters(|ch| ch[cn].unreachy as i32);
    let ticker = Repository::with_globals(|g| g.ticker as i64);
    if unreach as i64 > ticker && unreachx == x && unreachy == y {
        return -1;
    }

    let dir = PathFinder::with_mut(|pf| {
        pf.find_path(cn, x as i16, y as i16, flag as u8, x2 as i16, y2 as i16)
    });

    if dir.is_none() {
        Repository::with_characters_mut(|ch| {
            ch[cn].unreach = Repository::with_globals(|g| g.ticker) + core::constants::TICKS as i32;
            ch[cn].unreachx = x;
            ch[cn].unreachy = y;
        });
        return -1;
    }

    if dir == Some(0) {
        return 0;
    }

    match dir {
        d if d == Some(core::constants::DX_RIGHTDOWN) => {
            if Repository::with_characters(|ch| ch[cn].dir as i32)
                != core::constants::DX_RIGHTDOWN as i32
            {
                act_turn_rightdown(cn);
                return 0;
            }
            act_move_rightdown(cn);
            return 0;
        }
        d if d == Some(core::constants::DX_RIGHTUP) => {
            if Repository::with_characters(|ch| ch[cn].dir as i32)
                != core::constants::DX_RIGHTUP as i32
            {
                act_turn_rightup(cn);
                return 0;
            }
            act_move_rightup(cn);
            return 0;
        }
        d if d == Some(core::constants::DX_LEFTDOWN) => {
            if Repository::with_characters(|ch| ch[cn].dir as i32)
                != core::constants::DX_LEFTDOWN as i32
            {
                act_turn_leftdown(cn);
                return 0;
            }
            act_move_leftdown(cn);
            return 0;
        }
        d if d == Some(core::constants::DX_LEFTUP) => {
            if Repository::with_characters(|ch| ch[cn].dir as i32)
                != core::constants::DX_LEFTUP as i32
            {
                act_turn_leftup(cn);
                return 0;
            }
            act_move_leftup(cn);
            return 0;
        }
        d if d == Some(core::constants::DX_RIGHT) => {
            if Repository::with_characters(|ch| ch[cn].dir as i32)
                != core::constants::DX_RIGHT as i32
            {
                act_turn_right(cn);
                return 0;
            }
            let base_x = Repository::with_characters(|ch| ch[cn].x as usize);
            let base_y = Repository::with_characters(|ch| ch[cn].y as usize);
            let in_id = Repository::with_map(|map| {
                map[(base_x + base_y * core::constants::SERVER_MAPX as usize) + 1].it
            });
            if in_id != 0
                && Repository::with_items(|items| items[in_id as usize].active) == 0
                && Repository::with_items(|items| items[in_id as usize].driver) == 2
            {
                act_use(cn);
                return 0;
            }
            act_move_right(cn);
            return 0;
        }
        d if d == Some(core::constants::DX_LEFT) => {
            if Repository::with_characters(|ch| ch[cn].dir as i32)
                != core::constants::DX_LEFT as i32
            {
                act_turn_left(cn);
                return 0;
            }
            let base_x = Repository::with_characters(|ch| ch[cn].x as usize);
            let base_y = Repository::with_characters(|ch| ch[cn].y as usize);
            let in_id = Repository::with_map(|map| {
                map[(base_x + base_y * core::constants::SERVER_MAPX as usize) - 1].it
            });
            if in_id != 0
                && Repository::with_items(|items| items[in_id as usize].active) == 0
                && Repository::with_items(|items| items[in_id as usize].driver) == 2
            {
                act_use(cn);
                return 0;
            }
            act_move_left(cn);
            return 0;
        }
        d if d == Some(core::constants::DX_DOWN) => {
            if Repository::with_characters(|ch| ch[cn].dir as i32)
                != core::constants::DX_DOWN as i32
            {
                act_turn_down(cn);
                return 0;
            }
            let base_x = Repository::with_characters(|ch| ch[cn].x as usize);
            let base_y = Repository::with_characters(|ch| ch[cn].y as usize);
            let in_id = Repository::with_map(|map| {
                map[base_x + (base_y + 1) * core::constants::SERVER_MAPX as usize].it
            });
            if in_id != 0
                && Repository::with_items(|items| items[in_id as usize].active) == 0
                && Repository::with_items(|items| items[in_id as usize].driver) == 2
            {
                act_use(cn);
                return 0;
            }
            act_move_down(cn);
            return 0;
        }
        d if d == Some(core::constants::DX_UP) => {
            if Repository::with_characters(|ch| ch[cn].dir as i32) != core::constants::DX_UP as i32
            {
                act_turn_up(cn);
                return 0;
            }
            let base_x = Repository::with_characters(|ch| ch[cn].x as usize);
            let base_y = Repository::with_characters(|ch| ch[cn].y as usize);
            let in_id = Repository::with_map(|map| {
                map[base_x + (base_y - 1) * core::constants::SERVER_MAPX as usize].it
            });
            if in_id != 0
                && Repository::with_items(|items| items[in_id as usize].active) == 0
                && Repository::with_items(|items| items[in_id as usize].driver) == 2
            {
                act_use(cn);
                return 0;
            }
            act_move_up(cn);
            return 0;
        }
        _ => return -1,
    }
}

pub fn drv_moveto(cn: usize, x: usize, y: usize) {
    // Mirror C++ drv_moveto
    let ret = char_moveto(cn, x as i32, y as i32, 0, 0, 0);
    if ret != 0 {
        Repository::with_characters_mut(|ch| ch[cn].goto_x = 0);
    }
    if ret == -1 {
        Repository::with_characters_mut(|ch| {
            ch[cn].last_action = core::constants::ERR_FAILED as i8
        });
    } else if ret == 1 {
        Repository::with_characters_mut(|ch| {
            ch[cn].last_action = core::constants::ERR_SUCCESS as i8
        });
    }
}

pub fn drv_turnto(cn: usize, x: usize, y: usize) {
    // Mirror C++ drv_turnto
    let dir = crate::helpers::drv_dcoor2dir(
        x as i32 - Repository::with_characters(|ch| ch[cn].x as i32),
        y as i32 - Repository::with_characters(|ch| ch[cn].y as i32),
    );
    if dir == Repository::with_characters(|ch| ch[cn].dir as i32) {
        Repository::with_characters_mut(|ch| {
            ch[cn].misc_action = core::constants::DR_IDLE as u16;
            ch[cn].last_action = core::constants::ERR_SUCCESS as i8;
        });
    } else {
        if dir != -1 {
            act_turn(cn, dir);
        } else {
            Repository::with_characters_mut(|ch| {
                ch[cn].last_action = core::constants::ERR_FAILED as i8
            });
        }
    }
}

pub fn drv_dropto(cn: usize, x: usize, y: usize) {
    // Mirror C++ drv_dropto
    let ret = char_dropto(cn, x as i32, y as i32);
    if ret != 0 {
        Repository::with_characters_mut(|ch| ch[cn].misc_action = core::constants::DR_IDLE as u16);
    }
    if ret == -1 {
        Repository::with_characters_mut(|ch| {
            ch[cn].last_action = core::constants::ERR_FAILED as i8
        });
    } else if ret == 1 {
        Repository::with_characters_mut(|ch| {
            ch[cn].last_action = core::constants::ERR_SUCCESS as i8
        });
    }
}

pub fn drv_pickupto(cn: usize, x: usize, y: usize) {
    // Mirror C++ drv_pickupto
    let ret = char_pickupto(cn, x as i32, y as i32);
    if ret != 0 {
        Repository::with_characters_mut(|ch| ch[cn].misc_action = core::constants::DR_IDLE as u16);
    }
    if ret == -1 {
        Repository::with_characters_mut(|ch| {
            ch[cn].last_action = core::constants::ERR_FAILED as i8
        });
    } else if ret == 1 {
        Repository::with_characters_mut(|ch| {
            ch[cn].last_action = core::constants::ERR_SUCCESS as i8
        });
    }
}

pub fn drv_useto(cn: usize, x: usize, y: usize) {
    // Mirror C++ drv_useto
    let ret = char_useto(cn, x as i32, y as i32);

    // bounds check as in C++
    let mut xx = x as i32;
    let mut yy = y as i32;
    if xx < 0
        || xx >= core::constants::SERVER_MAPX as i32
        || yy < 0
        || yy >= core::constants::SERVER_MAPY as i32
    {
        xx = 0;
        yy = 0;
    }

    let m = (xx + yy * core::constants::SERVER_MAPX as i32) as usize;
    let in_item = Repository::with_map(|map| map[m].it);

    if ret != 0
        && (in_item == 0 || Repository::with_items(|items| items[in_item as usize].driver) != 25)
    {
        Repository::with_characters_mut(|ch| ch[cn].misc_action = core::constants::DR_IDLE as u16);
    }
    if ret == -1 {
        Repository::with_characters_mut(|ch| {
            ch[cn].last_action = core::constants::ERR_FAILED as i8
        });
    } else if ret == 1 {
        Repository::with_characters_mut(|ch| {
            ch[cn].last_action = core::constants::ERR_SUCCESS as i8
        });
    }
}

pub fn drv_use(cn: usize, nr: i32) {
    // Mirror C++ drv_use
    let in_item = if nr < 20 {
        Repository::with_characters(|ch| ch[cn].worn[nr as usize] as usize)
    } else {
        Repository::with_characters(|ch| ch[cn].item[(nr - 20) as usize] as usize)
    };

    if in_item == 0 {
        Repository::with_characters_mut(|ch| {
            ch[cn].last_action = core::constants::ERR_FAILED as i8;
            ch[cn].use_nr = 0;
        });
        return;
    }

    driver::use_driver(cn, in_item, true);
    Repository::with_characters_mut(|ch| {
        if ch[cn].cerrno == core::constants::ERR_SUCCESS as u16 {
            ch[cn].last_action = core::constants::ERR_SUCCESS as i8;
        }
        if ch[cn].cerrno == core::constants::ERR_FAILED as u16 {
            ch[cn].last_action = core::constants::ERR_FAILED as i8;
        }
        ch[cn].cerrno = core::constants::ERR_NONE as u16;
        ch[cn].use_nr = 0;
    });
}

pub fn drv_attack_char(cn: usize, co: usize) {
    // Mirror C++ drv_attack_char
    let ret = char_attack_char(cn, co);
    if ret == -1 {
        Repository::with_characters_mut(|ch| {
            ch[cn].attack_cn = 0;
            ch[cn].last_action = core::constants::ERR_FAILED as i8;
        });
    } else if ret == 1 {
        Repository::with_characters_mut(|ch| {
            ch[cn].last_action = core::constants::ERR_SUCCESS as i8
        });
    }
}

pub fn drv_give_char(cn: usize, co: usize) {
    // Mirror C++ drv_give_char
    let ret = char_give_char(cn, co);
    if ret != 0 {
        Repository::with_characters_mut(|ch| ch[cn].misc_action = core::constants::DR_IDLE as u16);
    }
    if ret == -1 {
        Repository::with_characters_mut(|ch| {
            ch[cn].last_action = core::constants::ERR_FAILED as i8
        });
    } else if ret == 1 {
        Repository::with_characters_mut(|ch| {
            ch[cn].last_action = core::constants::ERR_SUCCESS as i8
        });
    }
}

pub fn drv_bow(cn: usize) {
    // Mirror C++ drv_bow
    let dir = Repository::with_characters(|ch| ch[cn].dir);
    if dir == core::constants::DX_LEFTUP {
        act_turn_left(cn);
        return;
    }
    if dir == core::constants::DX_LEFTDOWN {
        act_turn_down(cn);
        return;
    }
    if dir == core::constants::DX_RIGHTUP {
        act_turn_up(cn);
        return;
    }
    if dir == core::constants::DX_RIGHTDOWN {
        act_turn_right(cn);
        return;
    }

    act_bow(cn);
    Repository::with_characters_mut(|ch| {
        ch[cn].misc_action = core::constants::DR_IDLE as u16;
        ch[cn].cerrno = core::constants::ERR_NONE as u16;
        ch[cn].last_action = core::constants::ERR_SUCCESS as i8;
    });
}

pub fn drv_wave(cn: usize) {
    // Mirror C++ drv_wave
    let dir = Repository::with_characters(|ch| ch[cn].dir);
    if dir == core::constants::DX_LEFTUP {
        act_turn_left(cn);
        return;
    }
    if dir == core::constants::DX_LEFTDOWN {
        act_turn_down(cn);
        return;
    }
    if dir == core::constants::DX_RIGHTUP {
        act_turn_up(cn);
        return;
    }
    if dir == core::constants::DX_RIGHTDOWN {
        act_turn_right(cn);
        return;
    }

    act_wave(cn);
    Repository::with_characters_mut(|ch| {
        ch[cn].misc_action = core::constants::DR_IDLE as u16;
        ch[cn].cerrno = core::constants::ERR_NONE as u16;
        ch[cn].last_action = core::constants::ERR_SUCCESS as i8;
    });
}

pub fn drv_skill(cn: usize) {
    // Mirror C++ drv_skill
    let dir = Repository::with_characters(|ch| ch[cn].dir);
    if dir == core::constants::DX_LEFTUP {
        act_turn_left(cn);
        return;
    }
    if dir == core::constants::DX_LEFTDOWN {
        act_turn_down(cn);
        return;
    }
    if dir == core::constants::DX_RIGHTUP {
        act_turn_up(cn);
        return;
    }
    if dir == core::constants::DX_RIGHTDOWN {
        act_turn_right(cn);
        return;
    }

    act_skill(cn);
    Repository::with_characters_mut(|ch| {
        ch[cn].skill_target2 = ch[cn].skill_nr;
        ch[cn].skill_nr = 0;
        ch[cn].cerrno = core::constants::ERR_NONE as u16;
        ch[cn].last_action = core::constants::ERR_SUCCESS as i8;
    });
}

pub fn driver_msg(cn: usize, msg_type: i32, dat1: i32, dat2: i32, dat3: i32, dat4: i32) {
    // Mirror C++ driver_msg default handling
    // if stunned -> ignore
    let stunned = Repository::with_characters(|ch| ch[cn].stunned != 0);
    if stunned {
        return;
    }

    let is_player = Repository::with_characters(|ch| {
        (ch[cn].flags
            & (core::constants::CharacterFlags::CF_PLAYER.bits()
                | core::constants::CharacterFlags::CF_USURP.bits()) as u64)
            != 0
    });

    if !is_player {
        if driver::npc_msg(cn, msg_type, dat1, dat2, dat3, dat4) != 0 {
            return;
        }
    }

    let is_ccp = Repository::with_characters(|ch| {
        (ch[cn].flags & core::constants::CharacterFlags::CF_CCP.bits() as u64) != 0
    });
    if is_ccp {
        // TODO: driver_ccp::ccp_msg(cn, msg_type, dat1, dat2, dat3, dat4);
        // Actually this should never be called...
        log::error!("driver_ccp::ccp_msg not implemented for {}", cn);
    }

    match msg_type as u32 {
        x if x == core::constants::NT_GOTHIT as u32 || x == core::constants::NT_GOTMISS as u32 => {
            let attack_cn = Repository::with_characters(|ch| ch[cn].attack_cn as i32);
            let fightback = Repository::with_characters(|ch| {
                ch[cn].data[core::constants::CHD_FIGHTBACK as usize]
            });
            let misc_action = Repository::with_characters(|ch| ch[cn].misc_action);
            if attack_cn == 0 && fightback == 0 && misc_action != core::constants::DR_GIVE as u16 {
                Repository::with_characters_mut(|ch| ch[cn].attack_cn = dat1 as u16);
            }
        }
        _ => {
            // Other message types aren't handled and this is expected so no reason to log anything extra here.
        }
    }
}

pub fn follow_driver(cn: usize, co: usize) -> bool {
    // Bounds and validity checks
    if co == 0 || co >= core::constants::MAXCHARS as usize {
        return false;
    }
    let (tox, toy, dir) =
        Repository::with_characters(|ch| (ch[co].tox as i32, ch[co].toy as i32, ch[co].dir as i32));
    if tox < 5
        || tox > core::constants::SERVER_MAPX as i32 - 6
        || toy < 5
        || toy > core::constants::SERVER_MAPY as i32 - 6
    {
        return false;
    }

    let is_companion = Repository::with_characters(|ch| {
        (ch[cn].temp == core::constants::CT_COMPANION as u16) && ch[cn].data[63] as usize == co
    });
    let can_see = State::with_mut(|state| state.do_char_can_see(cn, co)) != 0;
    if !(is_companion || can_see) {
        return false;
    }

    // Calculate m (map index)
    let mut m = tox + toy * core::constants::SERVER_MAPX as i32;
    let dir_val = dir as u8;
    match dir_val {
        core::constants::DX_UP => m += core::constants::SERVER_MAPX as i32 * 2,
        core::constants::DX_DOWN => m -= core::constants::SERVER_MAPX as i32 * 2,
        core::constants::DX_LEFT => m += 2,
        core::constants::DX_RIGHT => m -= 2,
        core::constants::DX_LEFTUP => m += 2 + core::constants::SERVER_MAPX as i32 * 2,
        core::constants::DX_LEFTDOWN => m += 2 - core::constants::SERVER_MAPX as i32 * 2,
        core::constants::DX_RIGHTUP => m -= 2 - core::constants::SERVER_MAPX as i32 * -2,
        core::constants::DX_RIGHTDOWN => m -= 2 + core::constants::SERVER_MAPX as i32 * 2,
        _ => {}
    }

    // Check adjacency in map
    let map_len = Repository::with_map(|map| map.len());
    let mut is_adjacent = false;
    let check_indices = vec![
        m,
        m + 1,
        m - 1,
        m + core::constants::SERVER_MAPX as i32,
        m - core::constants::SERVER_MAPX as i32,
        m + 1 + core::constants::SERVER_MAPX as i32,
        m + 1 - core::constants::SERVER_MAPX as i32,
        m - 1 + core::constants::SERVER_MAPX as i32,
        m - 1 - core::constants::SERVER_MAPX as i32,
    ];
    for idx in check_indices.iter() {
        if *idx < 0 || *idx as usize >= map_len {
            continue;
        }
        let ch_val = Repository::with_map(|map| map[*idx as usize].ch);
        if ch_val as usize == cn {
            is_adjacent = true;
            break;
        }
    }

    if is_adjacent {
        let cur_dir = Repository::with_characters(|ch| ch[cn].dir as i32);
        if cur_dir as u8 == dir_val {
            Repository::with_characters_mut(|ch| {
                ch[cn].misc_action = core::constants::DR_IDLE as u16
            });
            return true;
        }
        Repository::with_characters_mut(|ch| ch[cn].misc_action = core::constants::DR_TURN as u16);
        let (x, y) = Repository::with_characters(|ch| (ch[cn].x as i32, ch[cn].y as i32));
        match dir_val {
            core::constants::DX_UP => {
                Repository::with_characters_mut(|ch| {
                    ch[cn].misc_target1 = x as u16;
                    ch[cn].misc_target2 = (y - 1) as u16;
                });
            }
            core::constants::DX_DOWN => {
                Repository::with_characters_mut(|ch| {
                    ch[cn].misc_target1 = x as u16;
                    ch[cn].misc_target2 = (y + 1) as u16;
                });
            }
            core::constants::DX_LEFT => {
                Repository::with_characters_mut(|ch| {
                    ch[cn].misc_target1 = (x - 1) as u16;
                    ch[cn].misc_target2 = y as u16;
                });
            }
            core::constants::DX_RIGHT => {
                Repository::with_characters_mut(|ch| {
                    ch[cn].misc_target1 = (x + 1) as u16;
                    ch[cn].misc_target2 = y as u16;
                });
            }
            core::constants::DX_LEFTUP => {
                Repository::with_characters_mut(|ch| {
                    ch[cn].misc_target1 = (x - 1) as u16;
                    ch[cn].misc_target2 = (y - 1) as u16;
                });
            }
            core::constants::DX_LEFTDOWN => {
                Repository::with_characters_mut(|ch| {
                    ch[cn].misc_target1 = (x - 1) as u16;
                    ch[cn].misc_target2 = (y + 1) as u16;
                });
            }
            core::constants::DX_RIGHTUP => {
                Repository::with_characters_mut(|ch| {
                    ch[cn].misc_target1 = (x + 1) as u16;
                    ch[cn].misc_target2 = (y - 1) as u16;
                });
            }
            core::constants::DX_RIGHTDOWN => {
                Repository::with_characters_mut(|ch| {
                    ch[cn].misc_target1 = (x + 1) as u16;
                    ch[cn].misc_target2 = (y + 1) as u16;
                });
            }
            _ => {
                Repository::with_characters_mut(|ch| {
                    ch[cn].misc_action = core::constants::DR_IDLE as u16
                });
            }
        }
        return true;
    }

    // Try to find a valid target tile
    let mut found = false;
    let mut new_m = m;
    let offsets = [
        0,
        1,
        -1,
        core::constants::SERVER_MAPX as i32,
        -core::constants::SERVER_MAPX as i32,
        1 + core::constants::SERVER_MAPX as i32,
        1 - core::constants::SERVER_MAPX as i32,
        -1 + core::constants::SERVER_MAPX as i32,
        -1 - core::constants::SERVER_MAPX as i32,
    ];
    for off in offsets.iter() {
        let try_m = m + off;
        if try_m < 0 || try_m as usize >= map_len {
            continue;
        }
        if player::plr_check_target(try_m as usize) {
            new_m = try_m;
            found = true;
            break;
        }
    }
    if !found {
        return false;
    }
    Repository::with_characters_mut(|ch| {
        ch[cn].goto_x = (new_m % core::constants::SERVER_MAPX as i32) as u16;
        ch[cn].goto_y = (new_m / core::constants::SERVER_MAPX as i32) as u16;
    });
    true
}

pub fn driver(cn: usize) {
    // 1. If not player or usurp -> run NPC high-priority driver
    let is_player_or_usurp = Repository::with_characters(|ch| {
        (ch[cn].flags
            & (core::constants::CharacterFlags::CF_PLAYER.bits()
                | core::constants::CharacterFlags::CF_USURP.bits()) as u64)
            != 0
    });
    if !is_player_or_usurp {
        driver::npc_driver_high(cn);
    }

    // 2. If CCP, run CCP driver (feature-gated)
    let is_ccp = Repository::with_characters(|ch| {
        (ch[cn].flags & core::constants::CharacterFlags::CF_CCP.bits() as u64) != 0
    });
    #[cfg(feature = "REAL_CCP")]
    if is_ccp {
        crate::driver_ccp::ccp_driver(cn);
    }
    #[cfg(not(feature = "REAL_CCP"))]
    if is_ccp {
        log::error!("ccp_driver not implemented for {}", cn);
    }

    // 3. use_nr (highest priority)
    let use_nr = Repository::with_characters(|ch| ch[cn].use_nr);
    if use_nr != 0 {
        drv_use(cn, use_nr as i32);
        return;
    }

    // 4. skill_nr
    let skill_nr = Repository::with_characters(|ch| ch[cn].skill_nr);
    if skill_nr != 0 {
        drv_skill(cn);
        return;
    }

    // 5. If player/usurp and not attacking, run player_driver_med
    let is_player_or_usurp = Repository::with_characters(|ch| {
        (ch[cn].flags
            & (core::constants::CharacterFlags::CF_PLAYER.bits()
                | core::constants::CharacterFlags::CF_USURP.bits()) as u64)
            != 0
    });
    let attack_cn = Repository::with_characters(|ch| ch[cn].attack_cn);
    if is_player_or_usurp && attack_cn == 0 {
        player::player_driver_med(cn);
    }

    // 6. goto_x (moveto)
    let goto_x = Repository::with_characters(|ch| ch[cn].goto_x);
    if goto_x != 0 {
        let goto_y = Repository::with_characters(|ch| ch[cn].goto_y);
        drv_moveto(cn, goto_x as usize, goto_y as usize);
        return;
    }

    // 7. attack_cn
    let attack_cn = Repository::with_characters(|ch| ch[cn].attack_cn);
    if attack_cn != 0 {
        drv_attack_char(cn, attack_cn as usize);
        return;
    }

    // 8. misc_action dispatch
    let misc_action = Repository::with_characters(|ch| ch[cn].misc_action);
    match misc_action as u32 {
        x if x == core::constants::DR_IDLE => {
            let is_player = Repository::with_characters(|ch| {
                (ch[cn].flags
                    & (core::constants::CharacterFlags::CF_PLAYER.bits()
                        | core::constants::CharacterFlags::CF_USURP.bits())
                        as u64)
                    != 0
            });
            if !is_player {
                driver::npc_driver_low(cn);
            }
        }
        x if x == core::constants::DR_DROP => {
            drv_dropto(
                cn,
                Repository::with_characters(|ch| ch[cn].misc_target1 as usize),
                Repository::with_characters(|ch| ch[cn].misc_target2 as usize),
            );
        }
        x if x == core::constants::DR_PICKUP => {
            drv_pickupto(
                cn,
                Repository::with_characters(|ch| ch[cn].misc_target1 as usize),
                Repository::with_characters(|ch| ch[cn].misc_target2 as usize),
            );
        }
        x if x == core::constants::DR_GIVE => {
            drv_give_char(
                cn,
                Repository::with_characters(|ch| ch[cn].misc_target1 as usize),
            );
        }
        x if x == core::constants::DR_USE => {
            drv_useto(
                cn,
                Repository::with_characters(|ch| ch[cn].misc_target1 as usize),
                Repository::with_characters(|ch| ch[cn].misc_target2 as usize),
            );
        }
        x if x == core::constants::DR_BOW => {
            log::debug!("drv_bow called for cn {}", cn);
            drv_bow(cn);
        }
        x if x == core::constants::DR_WAVE => {
            drv_wave(cn);
        }
        x if x == core::constants::DR_TURN => {
            drv_turnto(
                cn,
                Repository::with_characters(|ch| ch[cn].misc_target1 as usize),
                Repository::with_characters(|ch| ch[cn].misc_target2 as usize),
            );
        }
        x if x == core::constants::DR_SINGLEBUILD => {
            // not implemented
        }
        x if x == core::constants::DR_AREABUILD1 => {
            // not implemented
        }
        x if x == core::constants::DR_AREABUILD2 => {
            // not implemented
        }
        _ => {
            log::error!("player_driver(): unknown misc_action {}", misc_action);
            Repository::with_characters_mut(|ch| {
                ch[cn].misc_action = core::constants::DR_IDLE as u16
            });
        }
    }
}
