use crate::core;
use crate::enums::CharacterFlags;
use crate::player;
use crate::state::State;
use crate::Repository;
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

pub fn driver(cn: usize) {}
