use crate::enums::CharacterFlags;
use crate::player;
use crate::state::State;
use crate::Repository;
use crate::{core, driver};
use rand::Rng;

pub fn act_idle(cn: usize) {
    let should_notify = Repository::with_globals(|g| (g.ticker & 15) == (cn as u64 & 15));
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

pub fn act_attack(cn: usize) {
    Repository::with_characters_mut(|ch| ch[cn].cerrno = core::constants::ERR_NONE as u16);

    let is_simple = Repository::with_characters(|ch| {
        (ch[cn].flags & CharacterFlags::Simple.bits() as u64) != 0
    });

    let mut v: i8 = 0;
    if !is_simple {
        loop {
            let vv = rand::thread_rng().gen_range(0..=3) as i8;
            let last = Repository::with_characters(|ch| ch[cn].lastattack);
            if vv != last {
                v = vv;
                break;
            }
        }
        if v != 0 {
            v += 4;
        }
        Repository::with_characters_mut(|ch| ch[cn].lastattack = v);
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

pub fn act_turn_up(cn: usize) {}

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

pub fn drv_turncount(dir1: i32, dir2: i32) -> i32 {}
pub fn char_give_char(cn: usize, co: usize) -> i32 {}
pub fn char_attack_char(cn: usize, co: usize) -> i32 {}
pub fn char_useto(cn: usize, x: i32, y: i32) -> i32 {}
pub fn char_pickupto(cn: usize, x: i32, y: i32) -> i32 {}
pub fn char_dropto(cn: usize, x: i32, y: i32) -> i32 {}
pub fn char_moveto(cn: usize, x: i32, y: i32, flag: i32, x2: i32, y2: i32) -> i32 {}

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

    crate::driver_use::use_driver(cn, in_item, true);
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
    let ret = char_attack_char(cn, co as i32);
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
    let ret = char_give_char(cn, co as i32);
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
        _ => {}
    }
}

pub fn driver(cn: usize) {
    // If not player or usurp -> run NPC high-priority driver
    let is_player_or_usurp = Repository::with_characters(|ch| {
        (ch[cn].flags
            & (core::constants::CharacterFlags::CF_PLAYER.bits()
                | core::constants::CharacterFlags::CF_USURP.bits()) as u64)
            != 0
    });
    if !is_player_or_usurp {
        driver::npc_driver_high(cn);
    }

    // CCP handling (not ported) -> log for now
    let is_ccp = Repository::with_characters(|ch| {
        (ch[cn].flags & core::constants::CharacterFlags::CF_CCP.bits() as u64) != 0
    });
    if is_ccp {
        log::debug!("ccp_driver not implemented for {}", cn);
    }

    // use_nr (highest priority)
    let use_nr = Repository::with_characters(|ch| ch[cn].use_nr);
    if use_nr != 0 {
        let in_item = Repository::with_characters(|ch| {
            if use_nr < 20 {
                ch[cn].worn[use_nr as usize] as usize
            } else {
                ch[cn].item[(use_nr - 20) as usize] as usize
            }
        });
        if in_item == 0 {
            Repository::with_characters_mut(|ch| {
                ch[cn].last_action = core::constants::ERR_FAILED as i8;
                ch[cn].use_nr = 0;
            });
            return;
        }

        let carried = use_nr < 20;
        crate::driver_use::use_driver(cn, in_item, carried);

        Repository::with_characters_mut(|ch| {
            if ch[cn].cerrno == core::constants::ERR_SUCCESS as u16 {
                ch[cn].last_action = core::constants::ERR_SUCCESS as i8;
            } else if ch[cn].cerrno == core::constants::ERR_FAILED as u16 {
                ch[cn].last_action = core::constants::ERR_FAILED as i8;
            }
            ch[cn].cerrno = core::constants::ERR_NONE as u16;
            ch[cn].use_nr = 0;
        });
        return;
    }

    // skill_nr
    let skill_nr = Repository::with_characters(|ch| ch[cn].skill_nr);
    if skill_nr != 0 {
        // skill driver not fully ported yet; use plr_skill placeholder
        crate::player::plr_skill(cn);
        Repository::with_characters_mut(|ch| {
            ch[cn].skill_nr = 0;
            ch[cn].last_action = core::constants::ERR_SUCCESS as i8;
            ch[cn].cerrno = core::constants::ERR_NONE as u16;
        });
        return;
    }

    // player medium-priority driver (not fully ported) would go here

    // goto_x (moveto)
    let goto_x = Repository::with_characters(|ch| ch[cn].goto_x);
    if goto_x != 0 {
        let goto_y = Repository::with_characters(|ch| ch[cn].goto_y);
        log::debug!(
            "drv_moveto not implemented; would moveto {}:{} for {}",
            goto_x,
            goto_y,
            cn
        );
        return;
    }

    // attack_cn
    let attack_cn = Repository::with_characters(|ch| ch[cn].attack_cn);
    if attack_cn != 0 {
        log::debug!("drv_attack_char not implemented for {}", cn);
        return;
    }

    // misc_action dispatch
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
                crate::driver::npc_driver_low(cn);
            }
        }
        x if x == core::constants::DR_DROP => {
            crate::player::plr_drop(cn);
        }
        x if x == core::constants::DR_PICKUP => {
            crate::player::plr_pickup(cn);
        }
        x if x == core::constants::DR_GIVE => {
            crate::player::plr_give(cn);
        }
        x if x == core::constants::DR_USE => {
            crate::player::plr_use(cn);
        }
        x if x == core::constants::DR_BOW => {
            crate::player::plr_bow(cn);
            Repository::with_characters_mut(|ch| {
                ch[cn].misc_action = core::constants::DR_IDLE as u16;
                ch[cn].cerrno = core::constants::ERR_NONE as u16;
                ch[cn].last_action = core::constants::ERR_SUCCESS as i8;
            });
        }
        x if x == core::constants::DR_WAVE => {
            crate::player::plr_wave(cn);
            Repository::with_characters_mut(|ch| {
                ch[cn].misc_action = core::constants::DR_IDLE as u16;
                ch[cn].cerrno = core::constants::ERR_NONE as u16;
                ch[cn].last_action = core::constants::ERR_SUCCESS as i8;
            });
        }
        x if x == core::constants::DR_TURN => {
            let tx = Repository::with_characters(|ch| ch[cn].misc_target1 as i32);
            let ty = Repository::with_characters(|ch| ch[cn].misc_target2 as i32);
            let cx = Repository::with_characters(|ch| ch[cn].x as i32);
            let cy = Repository::with_characters(|ch| ch[cn].y as i32);
            let dir = crate::helpers::drv_dcoor2dir(tx - cx, ty - cy);
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
        _ => {
            log::error!("player_driver(): unknown misc_action {}", misc_action);
            Repository::with_characters_mut(|ch| {
                ch[cn].misc_action = core::constants::DR_IDLE as u16
            });
        }
    }
}
