use core::constants::CharacterFlags;

use crate::game_state::GameState;
use crate::player;
use crate::{core, driver, helpers};

/// Notifies the area of the character's presence if the ticker matches.
///
/// # Arguments
///
/// * `gs` - Mutable reference to the unified game state.
/// * `cn` - Character number (index)
pub fn act_idle(gs: &mut GameState, cn: usize) {
    let should_notify = (gs.globals.ticker & 15) == (cn as i32 & 15);
    if should_notify {
        let (x, y) = (gs.characters[cn].x as i32, gs.characters[cn].y as i32);
        gs.do_area_notify(
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
    }
}

/// Attempts to make the character drop (flee) in the direction they are facing.
///
/// # Arguments
///
/// * `gs` - Mutable reference to the unified game state.
/// * `cn` - Character number (index)
pub fn act_drop(gs: &mut GameState, cn: usize) {
    gs.characters[cn].cerrno = core::constants::ERR_SUCCESS as u16;

    let cannot_flee = gs.do_char_can_flee(cn) == 0;
    let simple = (gs.characters[cn].flags & CharacterFlags::Simple.bits()) != 0;

    if cannot_flee || simple {
        gs.characters[cn].cerrno = core::constants::ERR_FAILED as u16;
        return;
    }

    match gs.characters[cn].dir {
        core::constants::DX_UP => {
            if gs.characters[cn].y > 0 {
                gs.characters[cn].status = 160;
                gs.characters[cn].status2 = 2;
            } else {
                gs.characters[cn].cerrno = core::constants::ERR_FAILED as u16;
            }
        }
        core::constants::DX_DOWN => {
            if gs.characters[cn].y < (core::constants::SERVER_MAPY as i16 - 1) {
                gs.characters[cn].status = 168;
                gs.characters[cn].status2 = 2;
            } else {
                gs.characters[cn].cerrno = core::constants::ERR_FAILED as u16;
            }
        }
        core::constants::DX_LEFT => {
            if gs.characters[cn].x > 0 {
                gs.characters[cn].status = 176;
                gs.characters[cn].status2 = 2;
            } else {
                gs.characters[cn].cerrno = core::constants::ERR_FAILED as u16;
            }
        }
        core::constants::DX_RIGHT => {
            if gs.characters[cn].x < (core::constants::SERVER_MAPX as i16 - 1) {
                gs.characters[cn].status = 184;
                gs.characters[cn].status2 = 2;
            } else {
                gs.characters[cn].cerrno = core::constants::ERR_FAILED as u16;
            }
        }
        _ => gs.characters[cn].cerrno = core::constants::ERR_FAILED as u16,
    }
}

/// Attempts to make the character use an item or interact in the direction they are facing.
///
/// # Arguments
///
/// * `gs` - Mutable reference to the unified game state.
/// * `cn` - Character number (index)
pub fn act_use(gs: &mut GameState, cn: usize) {
    gs.characters[cn].cerrno = core::constants::ERR_SUCCESS as u16;

    let cannot_flee = gs.do_char_can_flee(cn) == 0;
    let simple = (gs.characters[cn].flags & CharacterFlags::Simple.bits()) != 0;

    if cannot_flee || simple {
        gs.characters[cn].cerrno = core::constants::ERR_FAILED as u16;
        return;
    }

    match gs.characters[cn].dir {
        core::constants::DX_UP => {
            if gs.characters[cn].y > 0 {
                gs.characters[cn].status = 160;
                gs.characters[cn].status2 = 4;
            } else {
                gs.characters[cn].cerrno = core::constants::ERR_FAILED as u16;
            }
        }
        core::constants::DX_DOWN => {
            if gs.characters[cn].y < (core::constants::SERVER_MAPY as i16 - 1) {
                gs.characters[cn].status = 168;
                gs.characters[cn].status2 = 4;
            } else {
                gs.characters[cn].cerrno = core::constants::ERR_FAILED as u16;
            }
        }
        core::constants::DX_LEFT => {
            if gs.characters[cn].x > 0 {
                gs.characters[cn].status = 176;
                gs.characters[cn].status2 = 4;
            } else {
                gs.characters[cn].cerrno = core::constants::ERR_FAILED as u16;
            }
        }
        core::constants::DX_RIGHT => {
            if gs.characters[cn].x < (core::constants::SERVER_MAPX as i16 - 1) {
                gs.characters[cn].status = 184;
                gs.characters[cn].status2 = 4;
            } else {
                gs.characters[cn].cerrno = core::constants::ERR_FAILED as u16;
            }
        }
        _ => gs.characters[cn].cerrno = core::constants::ERR_FAILED as u16,
    }
}

/// Attempts to make the character pick up an item in the direction they are facing.
///
/// # Arguments
///
/// * `gs` - Mutable reference to the unified game state.
/// * `cn` - Character number (index)
pub fn act_pickup(gs: &mut GameState, cn: usize) {
    let simple_initial = (gs.characters[cn].flags & CharacterFlags::Simple.bits()) != 0;
    if simple_initial {
        gs.characters[cn].cerrno = core::constants::ERR_FAILED as u16;
        return;
    }

    gs.characters[cn].cerrno = core::constants::ERR_SUCCESS as u16;

    let cannot_flee = gs.do_char_can_flee(cn) == 0;
    let simple = (gs.characters[cn].flags & CharacterFlags::Simple.bits()) != 0;

    if cannot_flee || simple {
        gs.characters[cn].cerrno = core::constants::ERR_FAILED as u16;
        return;
    }

    match gs.characters[cn].dir {
        core::constants::DX_UP => {
            if gs.characters[cn].y > 0 {
                gs.characters[cn].status = 160;
                gs.characters[cn].status2 = 1;
            } else {
                gs.characters[cn].cerrno = core::constants::ERR_FAILED as u16;
            }
        }
        core::constants::DX_DOWN => {
            if gs.characters[cn].y < (core::constants::SERVER_MAPY as i16 - 1) {
                gs.characters[cn].status = 168;
                gs.characters[cn].status2 = 1;
            } else {
                gs.characters[cn].cerrno = core::constants::ERR_FAILED as u16;
            }
        }
        core::constants::DX_LEFT => {
            if gs.characters[cn].x > 0 {
                gs.characters[cn].status = 176;
                gs.characters[cn].status2 = 1;
            } else {
                gs.characters[cn].cerrno = core::constants::ERR_FAILED as u16;
            }
        }
        core::constants::DX_RIGHT => {
            if gs.characters[cn].x < (core::constants::SERVER_MAPX as i16 - 1) {
                gs.characters[cn].status = 184;
                gs.characters[cn].status2 = 1;
            } else {
                gs.characters[cn].cerrno = core::constants::ERR_FAILED as u16;
            }
        }
        _ => gs.characters[cn].cerrno = core::constants::ERR_FAILED as u16,
    }
}

/// Attempts to make the character use a skill in the direction they are facing.
///
/// # Arguments
///
/// * `gs` - Mutable reference to the unified game state.
/// * `cn` - Character number (index)
pub fn act_skill(gs: &mut GameState, cn: usize) {
    let simple = (gs.characters[cn].flags & CharacterFlags::Simple.bits()) != 0;
    if simple {
        gs.characters[cn].cerrno = core::constants::ERR_FAILED as u16;
        return;
    }

    gs.characters[cn].cerrno = core::constants::ERR_SUCCESS as u16;

    match gs.characters[cn].dir {
        core::constants::DX_UP => {
            gs.characters[cn].status = 160;
            gs.characters[cn].status2 = 9;
        }
        core::constants::DX_DOWN => {
            gs.characters[cn].status = 168;
            gs.characters[cn].status2 = 9;
        }
        core::constants::DX_LEFT => {
            gs.characters[cn].status = 176;
            gs.characters[cn].status2 = 9;
        }
        core::constants::DX_RIGHT => {
            gs.characters[cn].status = 184;
            gs.characters[cn].status2 = 9;
        }
        _ => gs.characters[cn].cerrno = core::constants::ERR_FAILED as u16,
    }
}

/// Attempts to make the character perform a wave action in the direction they are facing.
///
/// # Arguments
///
/// * `gs` - Mutable reference to the unified game state.
/// * `cn` - Character number (index)
pub fn act_wave(gs: &mut GameState, cn: usize) {
    let simple = (gs.characters[cn].flags & CharacterFlags::Simple.bits()) != 0;
    if simple {
        gs.characters[cn].cerrno = core::constants::ERR_FAILED as u16;
        return;
    }

    gs.characters[cn].cerrno = core::constants::ERR_SUCCESS as u16;

    match gs.characters[cn].dir {
        core::constants::DX_UP => {
            if gs.characters[cn].y > 0 {
                gs.characters[cn].status = 160;
                gs.characters[cn].status2 = 8;
            } else {
                gs.characters[cn].cerrno = core::constants::ERR_FAILED as u16;
            }
        }
        core::constants::DX_DOWN => {
            if gs.characters[cn].y < (core::constants::SERVER_MAPY as i16 - 1) {
                gs.characters[cn].status = 168;
                gs.characters[cn].status2 = 8;
            } else {
                gs.characters[cn].cerrno = core::constants::ERR_FAILED as u16;
            }
        }
        core::constants::DX_LEFT => {
            if gs.characters[cn].x > 0 {
                gs.characters[cn].status = 176;
                gs.characters[cn].status2 = 8;
            } else {
                gs.characters[cn].cerrno = core::constants::ERR_FAILED as u16;
            }
        }
        core::constants::DX_RIGHT => {
            if gs.characters[cn].x < (core::constants::SERVER_MAPX as i16 - 1) {
                gs.characters[cn].status = 184;
                gs.characters[cn].status2 = 8;
            } else {
                gs.characters[cn].cerrno = core::constants::ERR_FAILED as u16;
            }
        }
        _ => gs.characters[cn].cerrno = core::constants::ERR_FAILED as u16,
    }
}

/// Attempts to make the character perform a bow action in the direction they are facing.
///
/// # Arguments
///
/// * `gs` - Mutable reference to the unified game state.
/// * `cn` - Character number (index)
pub fn act_bow(gs: &mut GameState, cn: usize) {
    let simple = (gs.characters[cn].flags & CharacterFlags::Simple.bits()) != 0;
    if simple {
        gs.characters[cn].cerrno = core::constants::ERR_FAILED as u16;
        return;
    }

    gs.characters[cn].cerrno = core::constants::ERR_SUCCESS as u16;

    match gs.characters[cn].dir {
        core::constants::DX_UP => {
            if gs.characters[cn].y > 0 {
                gs.characters[cn].status = 160;
                gs.characters[cn].status2 = 7;
            } else {
                gs.characters[cn].cerrno = core::constants::ERR_FAILED as u16;
            }
        }
        core::constants::DX_DOWN => {
            if gs.characters[cn].y < (core::constants::SERVER_MAPY as i16 - 1) {
                gs.characters[cn].status = 168;
                gs.characters[cn].status2 = 7;
            } else {
                gs.characters[cn].cerrno = core::constants::ERR_FAILED as u16;
            }
        }
        core::constants::DX_LEFT => {
            if gs.characters[cn].x > 0 {
                gs.characters[cn].status = 176;
                gs.characters[cn].status2 = 7;
            } else {
                gs.characters[cn].cerrno = core::constants::ERR_FAILED as u16;
            }
        }
        core::constants::DX_RIGHT => {
            if gs.characters[cn].x < (core::constants::SERVER_MAPX as i16 - 1) {
                gs.characters[cn].status = 184;
                gs.characters[cn].status2 = 7;
            } else {
                gs.characters[cn].cerrno = core::constants::ERR_FAILED as u16;
            }
        }
        _ => gs.characters[cn].cerrno = core::constants::ERR_FAILED as u16,
    }
}

/// Attempts to make the character give an item in the direction they are facing.
///
/// # Arguments
///
/// * `gs` - Mutable reference to the unified game state.
/// * `cn` - Character number (index)
pub fn act_give(gs: &mut GameState, cn: usize) {
    let simple = (gs.characters[cn].flags & CharacterFlags::Simple.bits()) != 0;
    if simple {
        gs.characters[cn].cerrno = core::constants::ERR_FAILED as u16;
        return;
    }

    gs.characters[cn].cerrno = core::constants::ERR_SUCCESS as u16;

    match gs.characters[cn].dir {
        core::constants::DX_UP => {
            if gs.characters[cn].y > 0 {
                gs.characters[cn].status = 160;
                gs.characters[cn].status2 = 3;
            } else {
                gs.characters[cn].cerrno = core::constants::ERR_FAILED as u16;
            }
        }
        core::constants::DX_DOWN => {
            if gs.characters[cn].y < (core::constants::SERVER_MAPY as i16 - 1) {
                gs.characters[cn].status = 168;
                gs.characters[cn].status2 = 3;
            } else {
                gs.characters[cn].cerrno = core::constants::ERR_FAILED as u16;
            }
        }
        core::constants::DX_LEFT => {
            if gs.characters[cn].x > 0 {
                gs.characters[cn].status = 176;
                gs.characters[cn].status2 = 3;
            } else {
                gs.characters[cn].cerrno = core::constants::ERR_FAILED as u16;
            }
        }
        core::constants::DX_RIGHT => {
            if gs.characters[cn].x < (core::constants::SERVER_MAPX as i16 - 1) {
                gs.characters[cn].status = 184;
                gs.characters[cn].status2 = 3;
            } else {
                gs.characters[cn].cerrno = core::constants::ERR_FAILED as u16;
            }
        }
        _ => gs.characters[cn].cerrno = core::constants::ERR_FAILED as u16,
    }
}

/// Attempts to make the character perform an attack in the direction they are facing.
///
/// # Arguments
///
/// * `gs` - Mutable reference to the unified game state.
/// * `cn` - Character number (index)
pub fn act_attack(gs: &mut GameState, cn: usize) {
    gs.characters[cn].cerrno = core::constants::ERR_NONE as u16;

    let is_simple = (gs.characters[cn].flags & CharacterFlags::Simple.bits()) != 0;

    let mut v: i32;
    if !is_simple {
        let mut vv: i32;
        loop {
            vv = helpers::random_mod_i32(3);
            let last = gs.characters[cn].lastattack;
            if vv != last as i32 {
                break;
            }
        }
        gs.characters[cn].lastattack = vv as i8;

        v = vv;
        if v != 0 {
            v += 4;
        }
    } else {
        v = 0;
    }

    match gs.characters[cn].dir {
        d if d == core::constants::DX_UP => {
            if gs.characters[cn].y > 0 {
                gs.characters[cn].status = 160;
                gs.characters[cn].status2 = v as i16;
            } else {
                gs.characters[cn].cerrno = core::constants::ERR_FAILED as u16;
            }
        }
        d if d == core::constants::DX_DOWN => {
            if gs.characters[cn].y < (core::constants::SERVER_MAPY as i16 - 1) {
                gs.characters[cn].status = 168;
                gs.characters[cn].status2 = v as i16;
            } else {
                gs.characters[cn].cerrno = core::constants::ERR_FAILED as u16;
            }
        }
        d if d == core::constants::DX_LEFT => {
            if gs.characters[cn].x > 0 {
                gs.characters[cn].status = 176;
                gs.characters[cn].status2 = v as i16;
            } else {
                gs.characters[cn].cerrno = core::constants::ERR_FAILED as u16;
            }
        }
        d if d == core::constants::DX_RIGHT => {
            if gs.characters[cn].x < (core::constants::SERVER_MAPX as i16 - 1) {
                gs.characters[cn].status = 184;
                gs.characters[cn].status2 = v as i16;
            } else {
                gs.characters[cn].cerrno = core::constants::ERR_FAILED as u16;
            }
        }
        _ => gs.characters[cn].cerrno = core::constants::ERR_FAILED as u16,
    }
}

/// Turns the character to the specified direction.
///
/// # Arguments
///
/// * `gs` - Mutable reference to the unified game state.
/// * `cn` - Character number (index)
/// * `dir` - Direction to turn to
pub fn act_turn(gs: &mut GameState, cn: usize, dir: i32) {
    gs.characters[cn].cerrno = core::constants::ERR_NONE as u16;

    let same = gs.characters[cn].dir == dir as u8;
    if same {
        gs.characters[cn].cerrno = core::constants::ERR_SUCCESS as u16;
        return;
    }

    match dir as u8 {
        d if d == core::constants::DX_UP => act_turn_up(gs, cn),
        d if d == core::constants::DX_DOWN => act_turn_down(gs, cn),
        d if d == core::constants::DX_RIGHT => act_turn_right(gs, cn),
        d if d == core::constants::DX_LEFT => act_turn_left(gs, cn),
        d if d == core::constants::DX_LEFTUP => act_turn_leftup(gs, cn),
        d if d == core::constants::DX_LEFTDOWN => act_turn_leftdown(gs, cn),
        d if d == core::constants::DX_RIGHTUP => act_turn_rightup(gs, cn),
        d if d == core::constants::DX_RIGHTDOWN => act_turn_rightdown(gs, cn),
        _ => {
            log::error!("act_turn: invalid direction {} for character {}", dir, cn);
            gs.characters[cn].cerrno = core::constants::ERR_FAILED as u16
        }
    }
}

pub fn act_turn_rightdown(gs: &mut GameState, cn: usize) {
    gs.characters[cn].cerrno = core::constants::ERR_NONE as u16;

    let dir = gs.characters[cn].dir;
    if dir == core::constants::DX_LEFTUP {
        act_turn_up(gs, cn);
    } else if dir == core::constants::DX_UP {
        act_turn_rightup(gs, cn);
    } else if dir == core::constants::DX_LEFT {
        act_turn_leftdown(gs, cn);
    } else if dir == core::constants::DX_LEFTDOWN {
        act_turn_down(gs, cn);
    } else if dir == core::constants::DX_RIGHTUP {
        act_turn_right(gs, cn);
    } else if dir == core::constants::DX_DOWN {
        gs.characters[cn].status = 120;
    } else {
        gs.characters[cn].status = 152;
    }
}

pub fn act_turn_rightup(gs: &mut GameState, cn: usize) {
    gs.characters[cn].cerrno = core::constants::ERR_NONE as u16;

    let dir = gs.characters[cn].dir;
    if dir == core::constants::DX_LEFTDOWN {
        act_turn_down(gs, cn);
    } else if dir == core::constants::DX_DOWN {
        act_turn_rightdown(gs, cn);
    } else if dir == core::constants::DX_LEFT {
        act_turn_leftup(gs, cn);
    } else if dir == core::constants::DX_LEFTUP {
        act_turn_up(gs, cn);
    } else if dir == core::constants::DX_RIGHTDOWN {
        act_turn_right(gs, cn);
    } else if dir == core::constants::DX_UP {
        gs.characters[cn].status = 104;
    } else {
        gs.characters[cn].status = 144;
    }
}

pub fn act_turn_leftdown(gs: &mut GameState, cn: usize) {
    gs.characters[cn].cerrno = core::constants::ERR_NONE as u16;

    let dir = gs.characters[cn].dir;
    if dir == core::constants::DX_RIGHTUP {
        act_turn_up(gs, cn);
    } else if dir == core::constants::DX_UP {
        act_turn_leftup(gs, cn);
    } else if dir == core::constants::DX_RIGHT {
        act_turn_rightdown(gs, cn);
    } else if dir == core::constants::DX_RIGHTDOWN {
        act_turn_down(gs, cn);
    } else if dir == core::constants::DX_LEFTUP {
        act_turn_left(gs, cn);
    } else if dir == core::constants::DX_DOWN {
        gs.characters[cn].status = 112;
    } else {
        gs.characters[cn].status = 136;
    }
}

pub fn act_turn_leftup(gs: &mut GameState, cn: usize) {
    gs.characters[cn].cerrno = core::constants::ERR_NONE as u16;

    let dir = gs.characters[cn].dir;
    if dir == core::constants::DX_RIGHTDOWN {
        act_turn_down(gs, cn);
    } else if dir == core::constants::DX_DOWN {
        act_turn_leftdown(gs, cn);
    } else if dir == core::constants::DX_RIGHT {
        act_turn_rightup(gs, cn);
    } else if dir == core::constants::DX_RIGHTUP {
        act_turn_up(gs, cn);
    } else if dir == core::constants::DX_LEFTDOWN {
        act_turn_left(gs, cn);
    } else if dir == core::constants::DX_UP {
        gs.characters[cn].status = 96;
    } else {
        gs.characters[cn].status = 128;
    }
}

pub fn act_turn_right(gs: &mut GameState, cn: usize) {
    gs.characters[cn].cerrno = core::constants::ERR_NONE as u16;

    let dir = gs.characters[cn].dir;
    if dir == core::constants::DX_LEFT {
        act_turn_leftdown(gs, cn);
    } else if dir == core::constants::DX_LEFTUP {
        act_turn_up(gs, cn);
    } else if dir == core::constants::DX_LEFTDOWN {
        act_turn_down(gs, cn);
    } else if dir == core::constants::DX_UP {
        act_turn_rightup(gs, cn);
    } else if dir == core::constants::DX_DOWN {
        act_turn_rightdown(gs, cn);
    } else if dir == core::constants::DX_RIGHTUP {
        gs.characters[cn].status = 108;
    } else {
        gs.characters[cn].status = 124;
    }
}

pub fn act_turn_left(gs: &mut GameState, cn: usize) {
    gs.characters[cn].cerrno = core::constants::ERR_NONE as u16;

    let dir = gs.characters[cn].dir;
    if dir == core::constants::DX_RIGHT {
        act_turn_rightup(gs, cn);
    } else if dir == core::constants::DX_RIGHTUP {
        act_turn_up(gs, cn);
    } else if dir == core::constants::DX_RIGHTDOWN {
        act_turn_down(gs, cn);
    } else if dir == core::constants::DX_UP {
        act_turn_leftup(gs, cn);
    } else if dir == core::constants::DX_DOWN {
        act_turn_leftdown(gs, cn);
    } else if dir == core::constants::DX_LEFTUP {
        gs.characters[cn].status = 100;
    } else {
        gs.characters[cn].status = 116;
    }
}

pub fn act_turn_down(gs: &mut GameState, cn: usize) {
    gs.characters[cn].cerrno = core::constants::ERR_NONE as u16;

    let dir = gs.characters[cn].dir;
    if dir == core::constants::DX_UP {
        act_turn_leftup(gs, cn);
    } else if dir == core::constants::DX_LEFTUP {
        act_turn_left(gs, cn);
    } else if dir == core::constants::DX_RIGHTUP {
        act_turn_right(gs, cn);
    } else if dir == core::constants::DX_LEFT {
        act_turn_leftdown(gs, cn);
    } else if dir == core::constants::DX_RIGHT {
        act_turn_rightdown(gs, cn);
    } else if dir == core::constants::DX_LEFTDOWN {
        gs.characters[cn].status = 140;
    } else {
        gs.characters[cn].status = 156;
    }
}

pub fn act_turn_up(gs: &mut GameState, cn: usize) {
    gs.characters[cn].cerrno = core::constants::ERR_NONE as u16;

    let dir = gs.characters[cn].dir;

    if dir == core::constants::DX_DOWN {
        act_turn_rightdown(gs, cn);
    } else if dir == core::constants::DX_LEFTDOWN {
        act_turn_left(gs, cn);
    } else if dir == core::constants::DX_RIGHTDOWN {
        act_turn_right(gs, cn);
    } else if dir == core::constants::DX_LEFT {
        act_turn_leftup(gs, cn);
    } else if dir == core::constants::DX_RIGHT {
        act_turn_rightup(gs, cn);
    } else if dir == core::constants::DX_LEFTUP {
        gs.characters[cn].status = 132;
    } else {
        gs.characters[cn].status = 148;
    }
}

pub fn act_move_rightdown(gs: &mut GameState, cn: usize) {
    gs.characters[cn].cerrno = core::constants::ERR_NONE as u16;

    let (x, y, dir) = (
        gs.characters[cn].x as i32,
        gs.characters[cn].y as i32,
        gs.characters[cn].dir,
    );

    if x >= core::constants::SERVER_MAPX - 2 {
        gs.characters[cn].cerrno = core::constants::ERR_FAILED as u16;
        return;
    }
    if y >= core::constants::SERVER_MAPY - 2 {
        gs.characters[cn].cerrno = core::constants::ERR_FAILED as u16;
        return;
    }
    if dir != core::constants::DX_RIGHTDOWN {
        gs.characters[cn].cerrno = core::constants::ERR_FAILED as u16;
        return;
    }

    if gs.do_char_can_flee(cn) == 0 {
        gs.characters[cn].cerrno = core::constants::ERR_FAILED as u16;
        return;
    }

    let base = x + y * core::constants::SERVER_MAPX;
    let m1 = (base + core::constants::SERVER_MAPX) as usize;
    let m2 = (base + 1) as usize;
    let target = (base + core::constants::SERVER_MAPX + 1) as usize;

    if !player::plr_check_target(gs, m1) {
        gs.characters[cn].cerrno = core::constants::ERR_FAILED as u16;
        return;
    }
    if !player::plr_check_target(gs, m2) {
        gs.characters[cn].cerrno = core::constants::ERR_FAILED as u16;
        return;
    }
    if !player::plr_set_target(gs, target, cn) {
        gs.characters[cn].cerrno = core::constants::ERR_FAILED as u16;
        return;
    }

    gs.characters[cn].status = 84;
    gs.characters[cn].tox = (x + 1) as i16;
    gs.characters[cn].toy = (y + 1) as i16;
}

pub fn act_move_rightup(gs: &mut GameState, cn: usize) {
    gs.characters[cn].cerrno = core::constants::ERR_NONE as u16;

    let (x, y, dir) = (
        gs.characters[cn].x as i32,
        gs.characters[cn].y as i32,
        gs.characters[cn].dir,
    );

    if x >= core::constants::SERVER_MAPX - 2 {
        gs.characters[cn].cerrno = core::constants::ERR_FAILED as u16;
        return;
    }
    if y < 1 {
        gs.characters[cn].cerrno = core::constants::ERR_FAILED as u16;
        return;
    }
    if dir != core::constants::DX_RIGHTUP {
        gs.characters[cn].cerrno = core::constants::ERR_FAILED as u16;
        return;
    }

    if gs.do_char_can_flee(cn) == 0 {
        gs.characters[cn].cerrno = core::constants::ERR_FAILED as u16;
        return;
    }

    let base = x + y * core::constants::SERVER_MAPX;
    let m1 = (base - core::constants::SERVER_MAPX) as usize;
    let m2 = (base + 1) as usize;
    let target = (base - core::constants::SERVER_MAPX + 1) as usize;

    if !player::plr_check_target(gs, m1) {
        gs.characters[cn].cerrno = core::constants::ERR_FAILED as u16;
        return;
    }
    if !player::plr_check_target(gs, m2) {
        gs.characters[cn].cerrno = core::constants::ERR_FAILED as u16;
        return;
    }
    if !player::plr_set_target(gs, target, cn) {
        gs.characters[cn].cerrno = core::constants::ERR_FAILED as u16;
        return;
    }

    gs.characters[cn].status = 72;
    gs.characters[cn].tox = (x + 1) as i16;
    gs.characters[cn].toy = (y - 1) as i16;
}

pub fn act_move_leftdown(gs: &mut GameState, cn: usize) {
    gs.characters[cn].cerrno = core::constants::ERR_NONE as u16;

    let (x, y, dir) = (
        gs.characters[cn].x as i32,
        gs.characters[cn].y as i32,
        gs.characters[cn].dir,
    );

    if x < 1 {
        gs.characters[cn].cerrno = core::constants::ERR_FAILED as u16;
        return;
    }
    if y >= core::constants::SERVER_MAPY - 2 {
        gs.characters[cn].cerrno = core::constants::ERR_FAILED as u16;
        return;
    }
    if dir != core::constants::DX_LEFTDOWN {
        gs.characters[cn].cerrno = core::constants::ERR_FAILED as u16;
        return;
    }

    if gs.do_char_can_flee(cn) == 0 {
        gs.characters[cn].cerrno = core::constants::ERR_FAILED as u16;
        return;
    }

    let base = x + y * core::constants::SERVER_MAPX;
    let m1 = (base + core::constants::SERVER_MAPX) as usize;
    let m2 = (base - 1) as usize;
    let target = (base + core::constants::SERVER_MAPX - 1) as usize;

    if !player::plr_check_target(gs, m1) {
        gs.characters[cn].cerrno = core::constants::ERR_FAILED as u16;
        return;
    }
    if !player::plr_check_target(gs, m2) {
        gs.characters[cn].cerrno = core::constants::ERR_FAILED as u16;
        return;
    }
    if !player::plr_set_target(gs, target, cn) {
        gs.characters[cn].cerrno = core::constants::ERR_FAILED as u16;
        return;
    }

    gs.characters[cn].status = 60;
    gs.characters[cn].tox = (x - 1) as i16;
    gs.characters[cn].toy = (y + 1) as i16;
}

pub fn act_move_leftup(gs: &mut GameState, cn: usize) {
    gs.characters[cn].cerrno = core::constants::ERR_NONE as u16;

    let (x, y, dir) = (
        gs.characters[cn].x as i32,
        gs.characters[cn].y as i32,
        gs.characters[cn].dir,
    );

    if x < 1 {
        gs.characters[cn].cerrno = core::constants::ERR_FAILED as u16;
        return;
    }
    if y < 1 {
        gs.characters[cn].cerrno = core::constants::ERR_FAILED as u16;
        return;
    }
    if dir != core::constants::DX_LEFTUP {
        gs.characters[cn].cerrno = core::constants::ERR_FAILED as u16;
        return;
    }

    if gs.do_char_can_flee(cn) == 0 {
        gs.characters[cn].cerrno = core::constants::ERR_FAILED as u16;
        return;
    }

    let base = x + y * core::constants::SERVER_MAPX;
    let m1 = (base - core::constants::SERVER_MAPX) as usize;
    let m2 = (base - 1) as usize;
    let target = (base - core::constants::SERVER_MAPX - 1) as usize;

    if !player::plr_check_target(gs, m1) {
        gs.characters[cn].cerrno = core::constants::ERR_FAILED as u16;
        return;
    }
    if !player::plr_check_target(gs, m2) {
        gs.characters[cn].cerrno = core::constants::ERR_FAILED as u16;
        return;
    }
    if !player::plr_set_target(gs, target, cn) {
        gs.characters[cn].cerrno = core::constants::ERR_FAILED as u16;
        return;
    }

    gs.characters[cn].status = 48;
    gs.characters[cn].tox = (x - 1) as i16;
    gs.characters[cn].toy = (y - 1) as i16;
}

pub fn act_move_right(gs: &mut GameState, cn: usize) {
    gs.characters[cn].cerrno = core::constants::ERR_NONE as u16;

    let (x, y, dir) = (
        gs.characters[cn].x as i32,
        gs.characters[cn].y as i32,
        gs.characters[cn].dir,
    );

    if x >= core::constants::SERVER_MAPX - 2 {
        gs.characters[cn].cerrno = core::constants::ERR_FAILED as u16;
        return;
    }
    if dir != core::constants::DX_RIGHT {
        gs.characters[cn].cerrno = core::constants::ERR_FAILED as u16;
        return;
    }

    if gs.do_char_can_flee(cn) == 0 {
        gs.characters[cn].cerrno = core::constants::ERR_FAILED as u16;
        return;
    }

    let base = x + y * core::constants::SERVER_MAPX;
    let target = (base + 1) as usize;

    if !player::plr_set_target(gs, target, cn) {
        gs.characters[cn].cerrno = core::constants::ERR_FAILED as u16;
        return;
    }

    gs.characters[cn].status = 40;
    gs.characters[cn].tox = (x + 1) as i16;
    gs.characters[cn].toy = y as i16;
}

pub fn act_move_left(gs: &mut GameState, cn: usize) {
    gs.characters[cn].cerrno = core::constants::ERR_NONE as u16;

    let (x, y, dir) = (
        gs.characters[cn].x as i32,
        gs.characters[cn].y as i32,
        gs.characters[cn].dir,
    );

    if x < 1 {
        gs.characters[cn].cerrno = core::constants::ERR_FAILED as u16;
        return;
    }
    if dir != core::constants::DX_LEFT {
        gs.characters[cn].cerrno = core::constants::ERR_FAILED as u16;
        return;
    }

    if gs.do_char_can_flee(cn) == 0 {
        gs.characters[cn].cerrno = core::constants::ERR_FAILED as u16;
        return;
    }

    let base = x + y * core::constants::SERVER_MAPX;
    let target = (base - 1) as usize;

    if !player::plr_set_target(gs, target, cn) {
        gs.characters[cn].cerrno = core::constants::ERR_FAILED as u16;
        return;
    }

    gs.characters[cn].status = 32;
    gs.characters[cn].tox = (x - 1) as i16;
    gs.characters[cn].toy = y as i16;
}

pub fn act_move_down(gs: &mut GameState, cn: usize) {
    gs.characters[cn].cerrno = core::constants::ERR_NONE as u16;

    let (x, y, dir) = (
        gs.characters[cn].x as i32,
        gs.characters[cn].y as i32,
        gs.characters[cn].dir,
    );

    if y >= core::constants::SERVER_MAPY - 2 {
        gs.characters[cn].cerrno = core::constants::ERR_FAILED as u16;
        return;
    }
    if dir != core::constants::DX_DOWN {
        gs.characters[cn].cerrno = core::constants::ERR_FAILED as u16;
        return;
    }

    if gs.do_char_can_flee(cn) == 0 {
        gs.characters[cn].cerrno = core::constants::ERR_FAILED as u16;
        return;
    }

    let base = x + y * core::constants::SERVER_MAPX;
    let target = (base + core::constants::SERVER_MAPX) as usize;

    if !player::plr_set_target(gs, target, cn) {
        gs.characters[cn].cerrno = core::constants::ERR_FAILED as u16;
        return;
    }

    gs.characters[cn].status = 24;
    gs.characters[cn].tox = x as i16;
    gs.characters[cn].toy = (y + 1) as i16;
}

pub fn act_move_up(gs: &mut GameState, cn: usize) {
    gs.characters[cn].cerrno = core::constants::ERR_NONE as u16;

    let (x, y, dir) = (
        gs.characters[cn].x as i32,
        gs.characters[cn].y as i32,
        gs.characters[cn].dir,
    );

    if y < 1 {
        gs.characters[cn].cerrno = core::constants::ERR_FAILED as u16;
        return;
    }
    if dir != core::constants::DX_UP {
        gs.characters[cn].cerrno = core::constants::ERR_FAILED as u16;
        return;
    }

    if gs.do_char_can_flee(cn) == 0 {
        gs.characters[cn].cerrno = core::constants::ERR_FAILED as u16;
        return;
    }

    let base = x + y * core::constants::SERVER_MAPX;
    let target = (base - core::constants::SERVER_MAPX) as usize;

    if !player::plr_set_target(gs, target, cn) {
        gs.characters[cn].cerrno = core::constants::ERR_FAILED as u16;
        return;
    }

    gs.characters[cn].status = 16;
    gs.characters[cn].tox = x as i16;
    gs.characters[cn].toy = (y - 1) as i16;
}

pub fn char_give_char(gs: &mut GameState, cn: usize, co: usize) -> i32 {
    let cerrno = gs.characters[cn].cerrno;
    if cerrno == core::constants::ERR_FAILED as u16 {
        gs.characters[cn].cerrno = core::constants::ERR_NONE as u16;
        return -1;
    }

    let co_used = gs.characters[co].used;
    let can_see = gs.do_char_can_see(cn, co);
    if co_used != core::constants::USE_ACTIVE || can_see == 0 || cn == co {
        return -1;
    }

    let citem = gs.characters[cn].citem != 0;
    if !citem {
        return 1;
    }

    let (x, tox, y, toy, ax, ay) = (
        gs.characters[co].x as i32,
        gs.characters[co].tox as i32,
        gs.characters[co].y as i32,
        gs.characters[co].toy as i32,
        gs.characters[cn].x as i32,
        gs.characters[cn].y as i32,
    );

    if (x == ax + 1 && (y == ay + 1 || y == ay - 1))
        || (x == ax - 1 && (y == ay + 1 || y == ay - 1))
    {
        let err = char_moveto(gs, cn, x, y, 2, tox, toy);
        if err == -1 {
            return -1;
        } else {
            return 0;
        }
    }

    if (ax == x - 1 && ay == y) || (ax == tox - 1 && ay == toy) {
        if gs.characters[cn].dir as i32 != core::constants::DX_RIGHT as i32 {
            act_turn_right(gs, cn);
            return 0;
        }
        act_give(gs, cn);
        return 0;
    }
    if (ax == x + 1 && ay == y) || (ax == tox + 1 && ay == toy) {
        if gs.characters[cn].dir as i32 != core::constants::DX_LEFT as i32 {
            act_turn_left(gs, cn);
            return 0;
        }
        act_give(gs, cn);
        return 0;
    }
    if (ax == x && ay == y - 1) || (ax == tox && ay == toy - 1) {
        if gs.characters[cn].dir as i32 != core::constants::DX_DOWN as i32 {
            act_turn_down(gs, cn);
            return 0;
        }
        act_give(gs, cn);
        return 0;
    }
    if (ax == x && ay == y + 1) || (ax == tox && ay == toy + 1) {
        if gs.characters[cn].dir as i32 != core::constants::DX_UP as i32 {
            act_turn_up(gs, cn);
            return 0;
        }
        act_give(gs, cn);
        return 0;
    }

    let err = char_moveto(gs, cn, x, y, 2, tox, toy);
    if err == -1 {
        -1
    } else {
        0
    }
}

pub fn char_attack_char(gs: &mut GameState, cn: usize, co: usize) -> i32 {
    let cerrno = gs.characters[cn].cerrno;
    if cerrno == core::constants::ERR_FAILED as u16 {
        gs.characters[cn].cerrno = core::constants::ERR_NONE as u16;
        return -1;
    }

    let co_used = gs.characters[co].used;
    let can_see = gs.do_char_can_see(cn, co);
    let co_flags = gs.characters[co].flags;
    if co_used != core::constants::USE_ACTIVE
        || can_see == 0
        || cn == co
        || (co_flags & CharacterFlags::Body.bits()) != 0
        || (co_flags & CharacterFlags::Stoned.bits()) != 0
    {
        return -1;
    }

    let (x, tox, y, toy, ax, ay) = (
        gs.characters[co].x as i32,
        gs.characters[co].tox as i32,
        gs.characters[co].y as i32,
        gs.characters[co].toy as i32,
        gs.characters[cn].x as i32,
        gs.characters[cn].y as i32,
    );

    // diagonal adjacency
    if (x == ax + 1 && (y == ay + 1 || y == ay - 1))
        || (x == ax - 1 && (y == ay + 1 || y == ay - 1))
    {
        let err = char_moveto(gs, cn, x, y, 2, tox, toy);
        if err == -1 {
            return -1;
        } else {
            return 0;
        }
    }

    // attack if possible
    if (ax == x - 1 && ay == y) || (ax == tox - 1 && ay == toy) {
        if gs.characters[cn].dir as i32 != core::constants::DX_RIGHT as i32 {
            act_turn_right(gs, cn);
            return 0;
        }
        act_attack(gs, cn);
        return 1;
    }
    if (ax == x + 1 && ay == y) || (ax == tox + 1 && ay == toy) {
        if gs.characters[cn].dir as i32 != core::constants::DX_LEFT as i32 {
            act_turn_left(gs, cn);
            return 0;
        }
        act_attack(gs, cn);
        return 1;
    }
    if (ax == x && ay == y - 1) || (ax == tox && ay == toy - 1) {
        if gs.characters[cn].dir as i32 != core::constants::DX_DOWN as i32 {
            act_turn_down(gs, cn);
            return 0;
        }
        act_attack(gs, cn);
        return 1;
    }
    if (ax == x && ay == y + 1) || (ax == tox && ay == toy + 1) {
        if gs.characters[cn].dir as i32 != core::constants::DX_UP as i32 {
            act_turn_up(gs, cn);
            return 0;
        }
        act_attack(gs, cn);
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

    let err = char_moveto(gs, cn, nx, ny, 2, ntx, nty);
    if err == -1 {
        -1
    } else {
        0
    }
}

pub fn char_dropto(gs: &mut GameState, cn: usize, x: i32, y: i32) -> i32 {
    let cerrno = gs.characters[cn].cerrno;
    if cerrno == core::constants::ERR_FAILED as u16 {
        gs.characters[cn].cerrno = core::constants::ERR_NONE as u16;
        return -1;
    }

    let has_citem = gs.characters[cn].citem != 0;
    if !has_citem {
        return -1;
    }

    let cx = gs.characters[cn].x as i32;
    let cy = gs.characters[cn].y as i32;
    if cx == x - 1 && cy == y {
        if gs.characters[cn].dir as i32 != core::constants::DX_RIGHT as i32 {
            act_turn_right(gs, cn);
            return 0;
        }
        act_drop(gs, cn);
        return 1;
    }
    if cx == x + 1 && cy == y {
        if gs.characters[cn].dir as i32 != core::constants::DX_LEFT as i32 {
            act_turn_left(gs, cn);
            return 0;
        }
        act_drop(gs, cn);
        return 1;
    }
    if cx == x && cy == y - 1 {
        if gs.characters[cn].dir as i32 != core::constants::DX_DOWN as i32 {
            act_turn_down(gs, cn);
            return 0;
        }
        act_drop(gs, cn);
        return 1;
    }
    if cx == x && cy == y + 1 {
        if gs.characters[cn].dir as i32 != core::constants::DX_UP as i32 {
            act_turn_up(gs, cn);
            return 0;
        }
        act_drop(gs, cn);
        return 1;
    }

    if char_moveto(gs, cn, x, y, 1, 0, 0) == -1 {
        return -1;
    }
    0
}

pub fn char_pickup(gs: &mut GameState, cn: usize, x: i32, y: i32) -> i32 {
    let cerrno = gs.characters[cn].cerrno;
    if cerrno == core::constants::ERR_FAILED as u16 {
        gs.characters[cn].cerrno = core::constants::ERR_NONE as u16;
        return -1;
    }

    let cx = gs.characters[cn].x as i32;
    let cy = gs.characters[cn].y as i32;

    if cx == x - 1 && cy == y {
        if gs.characters[cn].dir as i32 != core::constants::DX_RIGHT as i32 {
            act_turn_right(gs, cn);
            return 0;
        }
        act_pickup(gs, cn);
        return 1;
    }
    if cx == x + 1 && cy == y {
        if gs.characters[cn].dir as i32 != core::constants::DX_LEFT as i32 {
            act_turn_left(gs, cn);
            return 0;
        }
        act_pickup(gs, cn);
        return 1;
    }
    if cx == x && cy == y - 1 {
        if gs.characters[cn].dir as i32 != core::constants::DX_DOWN as i32 {
            act_turn_down(gs, cn);
            return 0;
        }
        act_pickup(gs, cn);
        return 1;
    }
    if cx == x && cy == y + 1 {
        if gs.characters[cn].dir as i32 != core::constants::DX_UP as i32 {
            act_turn_up(gs, cn);
            return 0;
        }
        act_pickup(gs, cn);
        return 1;
    }

    -1
}

pub fn char_pickupto(gs: &mut GameState, cn: usize, x: i32, y: i32) -> i32 {
    let cerrno = gs.characters[cn].cerrno;
    if cerrno == core::constants::ERR_FAILED as u16 {
        gs.characters[cn].cerrno = core::constants::ERR_NONE as u16;
        return -1;
    }

    let has_citem = gs.characters[cn].citem != 0;
    if has_citem {
        return -1;
    }

    let ret = char_pickup(gs, cn, x, y);
    if ret == -1 {
        if char_moveto(gs, cn, x, y, 1, 0, 0) == -1 {
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

pub fn char_use(gs: &mut GameState, cn: usize, x: i32, y: i32) -> i32 {
    let cerrno = gs.characters[cn].cerrno;
    if cerrno == core::constants::ERR_FAILED as u16 {
        gs.characters[cn].cerrno = core::constants::ERR_NONE as u16;
        return -1;
    }

    let cx = gs.characters[cn].x as i32;
    let cy = gs.characters[cn].y as i32;

    if cx == x - 1 && cy == y {
        if gs.characters[cn].dir as i32 != core::constants::DX_RIGHT as i32 {
            act_turn_right(gs, cn);
            return 0;
        }
        act_use(gs, cn);
        return 1;
    }
    if cx == x + 1 && cy == y {
        if gs.characters[cn].dir as i32 != core::constants::DX_LEFT as i32 {
            act_turn_left(gs, cn);
            return 0;
        }
        act_use(gs, cn);
        return 1;
    }
    if cx == x && cy == y - 1 {
        if gs.characters[cn].dir as i32 != core::constants::DX_DOWN as i32 {
            act_turn_down(gs, cn);
            return 0;
        }
        act_use(gs, cn);
        return 1;
    }
    if cx == x && cy == y + 1 {
        if gs.characters[cn].dir as i32 != core::constants::DX_UP as i32 {
            act_turn_up(gs, cn);
            return 0;
        }
        act_use(gs, cn);
        return 1;
    }

    -1
}

pub fn char_useto(gs: &mut GameState, cn: usize, x: i32, y: i32) -> i32 {
    let cerrno = gs.characters[cn].cerrno;
    if cerrno == core::constants::ERR_FAILED as u16 {
        gs.characters[cn].cerrno = core::constants::ERR_NONE as u16;
        return -1;
    }

    let ret = char_use(gs, cn, x, y);
    if ret == -1 {
        if char_moveto(gs, cn, x, y, 1, 0, 0) == -1 {
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

pub fn char_moveto(
    gs: &mut GameState,
    cn: usize,
    x: i32,
    y: i32,
    flag: i32,
    x2: i32,
    y2: i32,
) -> i32 {
    let (cx, cy) = (gs.characters[cn].x as i32, gs.characters[cn].y as i32);
    if cx == x && cy == y && flag != 1 && flag != 3 {
        return 1;
    }

    let cerrno = gs.characters[cn].cerrno;
    if cerrno == core::constants::ERR_FAILED as u16 {
        gs.characters[cn].cerrno = core::constants::ERR_NONE as u16;
        return -1;
    }

    let unreach = gs.characters[cn].unreach;
    let unreachx = gs.characters[cn].unreachx;
    let unreachy = gs.characters[cn].unreachy;
    let ticker = gs.globals.ticker as i64;
    if unreach as i64 > ticker && unreachx == x && unreachy == y {
        return -1;
    }

    let dir = {
        let current_tick = gs.globals.ticker as u32;
        gs.pathfinder.find_path(
            &gs.characters[cn],
            &gs.map,
            &gs.items,
            current_tick,
            x as i16,
            y as i16,
            flag as u8,
            x2 as i16,
            y2 as i16,
        )
    };

    if dir.is_none() {
        gs.characters[cn].unreach = gs.globals.ticker + core::constants::TICKS;
        gs.characters[cn].unreachx = x;
        gs.characters[cn].unreachy = y;
        return -1;
    }

    if dir == Some(0) {
        return 0;
    }

    match dir {
        d if d == Some(core::constants::DX_RIGHTDOWN) => {
            if gs.characters[cn].dir as i32 != core::constants::DX_RIGHTDOWN as i32 {
                act_turn_rightdown(gs, cn);
                return 0;
            }
            act_move_rightdown(gs, cn);
            0
        }
        d if d == Some(core::constants::DX_RIGHTUP) => {
            if gs.characters[cn].dir as i32 != core::constants::DX_RIGHTUP as i32 {
                act_turn_rightup(gs, cn);
                return 0;
            }
            act_move_rightup(gs, cn);
            0
        }
        d if d == Some(core::constants::DX_LEFTDOWN) => {
            if gs.characters[cn].dir as i32 != core::constants::DX_LEFTDOWN as i32 {
                act_turn_leftdown(gs, cn);
                return 0;
            }
            act_move_leftdown(gs, cn);
            0
        }
        d if d == Some(core::constants::DX_LEFTUP) => {
            if gs.characters[cn].dir as i32 != core::constants::DX_LEFTUP as i32 {
                act_turn_leftup(gs, cn);
                return 0;
            }
            act_move_leftup(gs, cn);
            0
        }
        d if d == Some(core::constants::DX_RIGHT) => {
            if gs.characters[cn].dir as i32 != core::constants::DX_RIGHT as i32 {
                act_turn_right(gs, cn);
                return 0;
            }
            let base_x = gs.characters[cn].x as usize;
            let base_y = gs.characters[cn].y as usize;
            let in_id = gs.map[(base_x + base_y * core::constants::SERVER_MAPX as usize) + 1].it;
            if in_id != 0
                && gs.items[in_id as usize].active == 0
                && gs.items[in_id as usize].driver == 2
            {
                act_use(gs, cn);
                return 0;
            }
            act_move_right(gs, cn);
            0
        }
        d if d == Some(core::constants::DX_LEFT) => {
            if gs.characters[cn].dir as i32 != core::constants::DX_LEFT as i32 {
                act_turn_left(gs, cn);
                return 0;
            }
            let base_x = gs.characters[cn].x as usize;
            let base_y = gs.characters[cn].y as usize;
            let in_id = gs.map[(base_x + base_y * core::constants::SERVER_MAPX as usize) - 1].it;
            if in_id != 0
                && gs.items[in_id as usize].active == 0
                && gs.items[in_id as usize].driver == 2
            {
                act_use(gs, cn);
                return 0;
            }
            act_move_left(gs, cn);
            0
        }
        d if d == Some(core::constants::DX_DOWN) => {
            if gs.characters[cn].dir as i32 != core::constants::DX_DOWN as i32 {
                act_turn_down(gs, cn);
                return 0;
            }
            let base_x = gs.characters[cn].x as usize;
            let base_y = gs.characters[cn].y as usize;
            let in_id = gs.map[base_x + (base_y + 1) * core::constants::SERVER_MAPX as usize].it;
            if in_id != 0
                && gs.items[in_id as usize].active == 0
                && gs.items[in_id as usize].driver == 2
            {
                act_use(gs, cn);
                return 0;
            }
            act_move_down(gs, cn);
            0
        }
        d if d == Some(core::constants::DX_UP) => {
            if gs.characters[cn].dir as i32 != core::constants::DX_UP as i32 {
                act_turn_up(gs, cn);
                return 0;
            }
            let base_x = gs.characters[cn].x as usize;
            let base_y = gs.characters[cn].y as usize;
            let in_id = gs.map[base_x + (base_y - 1) * core::constants::SERVER_MAPX as usize].it;
            if in_id != 0
                && gs.items[in_id as usize].active == 0
                && gs.items[in_id as usize].driver == 2
            {
                act_use(gs, cn);
                return 0;
            }
            act_move_up(gs, cn);
            0
        }
        _ => -1,
    }
}

pub fn drv_moveto(gs: &mut GameState, cn: usize, x: usize, y: usize) {
    let ret = char_moveto(gs, cn, x as i32, y as i32, 0, 0, 0);
    if ret != 0 {
        gs.characters[cn].goto_x = 0;
    }
    if ret == -1 {
        gs.characters[cn].last_action = core::constants::ERR_FAILED as i8;
    } else if ret == 1 {
        gs.characters[cn].last_action = core::constants::ERR_SUCCESS as i8;
    }
}

pub fn drv_turnto(gs: &mut GameState, cn: usize, x: usize, y: usize) {
    let dir = crate::helpers::drv_dcoor2dir(
        x as i32 - gs.characters[cn].x as i32,
        y as i32 - gs.characters[cn].y as i32,
    );
    if dir == gs.characters[cn].dir as i32 {
        gs.characters[cn].misc_action = core::constants::DR_IDLE as u16;
        gs.characters[cn].last_action = core::constants::ERR_SUCCESS as i8;
    } else {
        if dir != -1 {
            act_turn(gs, cn, dir);
        } else {
            gs.characters[cn].last_action = core::constants::ERR_FAILED as i8;
        }
    }
}

pub fn drv_dropto(gs: &mut GameState, cn: usize, x: usize, y: usize) {
    let ret = char_dropto(gs, cn, x as i32, y as i32);
    if ret != 0 {
        gs.characters[cn].misc_action = core::constants::DR_IDLE as u16;
    }
    if ret == -1 {
        gs.characters[cn].last_action = core::constants::ERR_FAILED as i8;
    } else if ret == 1 {
        gs.characters[cn].last_action = core::constants::ERR_SUCCESS as i8;
    }
}

pub fn drv_pickupto(gs: &mut GameState, cn: usize, x: usize, y: usize) {
    let ret = char_pickupto(gs, cn, x as i32, y as i32);
    if ret != 0 {
        gs.characters[cn].misc_action = core::constants::DR_IDLE as u16;
    }
    if ret == -1 {
        gs.characters[cn].last_action = core::constants::ERR_FAILED as i8;
    } else if ret == 1 {
        gs.characters[cn].last_action = core::constants::ERR_SUCCESS as i8;
    }
}

pub fn drv_useto(gs: &mut GameState, cn: usize, x: usize, y: usize) {
    let ret = char_useto(gs, cn, x as i32, y as i32);

    let mut xx = x as i32;
    let mut yy = y as i32;
    if !(0..core::constants::SERVER_MAPX).contains(&xx)
        || !(0..core::constants::SERVER_MAPY).contains(&yy)
    {
        xx = 0;
        yy = 0;
    }

    let m = (xx + yy * core::constants::SERVER_MAPX) as usize;
    let in_item = gs.map[m].it;

    if ret != 0 && (in_item == 0 || gs.items[in_item as usize].driver != 25) {
        gs.characters[cn].misc_action = core::constants::DR_IDLE as u16;
    }
    if ret == -1 {
        gs.characters[cn].last_action = core::constants::ERR_FAILED as i8;
    } else if ret == 1 {
        gs.characters[cn].last_action = core::constants::ERR_SUCCESS as i8;
    }
}

pub fn drv_use(gs: &mut GameState, cn: usize, nr: i32) {
    let in_item = if nr < 20 {
        gs.characters[cn].worn[nr as usize] as usize
    } else {
        gs.characters[cn].item[(nr - 20) as usize] as usize
    };

    if in_item == 0 {
        gs.characters[cn].last_action = core::constants::ERR_FAILED as i8;
        gs.characters[cn].use_nr = 0;
        return;
    }

    driver::use_driver(gs, cn, in_item, true);
    if gs.characters[cn].cerrno == core::constants::ERR_SUCCESS as u16 {
        gs.characters[cn].last_action = core::constants::ERR_SUCCESS as i8;
    }
    if gs.characters[cn].cerrno == core::constants::ERR_FAILED as u16 {
        gs.characters[cn].last_action = core::constants::ERR_FAILED as i8;
    }
    gs.characters[cn].cerrno = core::constants::ERR_NONE as u16;
    gs.characters[cn].use_nr = 0;
}

pub fn drv_attack_char(gs: &mut GameState, cn: usize, co: usize) {
    let ret = char_attack_char(gs, cn, co);
    if ret == -1 {
        gs.characters[cn].attack_cn = 0;
        gs.characters[cn].last_action = core::constants::ERR_FAILED as i8;
    } else if ret == 1 {
        gs.characters[cn].last_action = core::constants::ERR_SUCCESS as i8;
    }
}

pub fn drv_give_char(gs: &mut GameState, cn: usize, co: usize) {
    let ret = char_give_char(gs, cn, co);
    if ret != 0 {
        gs.characters[cn].misc_action = core::constants::DR_IDLE as u16;
    }
    if ret == -1 {
        gs.characters[cn].last_action = core::constants::ERR_FAILED as i8;
    } else if ret == 1 {
        gs.characters[cn].last_action = core::constants::ERR_SUCCESS as i8;
    }
}

pub fn drv_bow(gs: &mut GameState, cn: usize) {
    let dir = gs.characters[cn].dir;
    if dir == core::constants::DX_LEFTUP {
        act_turn_left(gs, cn);
        return;
    }
    if dir == core::constants::DX_LEFTDOWN {
        act_turn_down(gs, cn);
        return;
    }
    if dir == core::constants::DX_RIGHTUP {
        act_turn_up(gs, cn);
        return;
    }
    if dir == core::constants::DX_RIGHTDOWN {
        act_turn_right(gs, cn);
        return;
    }

    act_bow(gs, cn);
    gs.characters[cn].misc_action = core::constants::DR_IDLE as u16;
    gs.characters[cn].cerrno = core::constants::ERR_NONE as u16;
    gs.characters[cn].last_action = core::constants::ERR_SUCCESS as i8;
}

pub fn drv_wave(gs: &mut GameState, cn: usize) {
    let dir = gs.characters[cn].dir;
    if dir == core::constants::DX_LEFTUP {
        act_turn_left(gs, cn);
        return;
    }
    if dir == core::constants::DX_LEFTDOWN {
        act_turn_down(gs, cn);
        return;
    }
    if dir == core::constants::DX_RIGHTUP {
        act_turn_up(gs, cn);
        return;
    }
    if dir == core::constants::DX_RIGHTDOWN {
        act_turn_right(gs, cn);
        return;
    }

    act_wave(gs, cn);
    gs.characters[cn].misc_action = core::constants::DR_IDLE as u16;
    gs.characters[cn].cerrno = core::constants::ERR_NONE as u16;
    gs.characters[cn].last_action = core::constants::ERR_SUCCESS as i8;
}

pub fn drv_skill(gs: &mut GameState, cn: usize) {
    let dir = gs.characters[cn].dir;
    if dir == core::constants::DX_LEFTUP {
        act_turn_left(gs, cn);
        return;
    }
    if dir == core::constants::DX_LEFTDOWN {
        act_turn_down(gs, cn);
        return;
    }
    if dir == core::constants::DX_RIGHTUP {
        act_turn_up(gs, cn);
        return;
    }
    if dir == core::constants::DX_RIGHTDOWN {
        act_turn_right(gs, cn);
        return;
    }

    act_skill(gs, cn);
    gs.characters[cn].skill_target2 = gs.characters[cn].skill_nr;
    gs.characters[cn].skill_nr = 0;
    gs.characters[cn].cerrno = core::constants::ERR_NONE as u16;
    gs.characters[cn].last_action = core::constants::ERR_SUCCESS as i8;
}

pub fn driver_msg(
    gs: &mut GameState,
    cn: usize,
    msg_type: i32,
    dat1: i32,
    dat2: i32,
    dat3: i32,
    dat4: i32,
) {
    if cn == 0 || cn >= core::constants::MAXCHARS {
        log::warn!("driver_msg: invalid character id {}", cn);
        return;
    }

    let stunned = gs.characters[cn].stunned != 0;
    if stunned {
        return;
    }

    let is_player = (gs.characters[cn].flags
        & (CharacterFlags::Player.bits() | CharacterFlags::Usurp.bits()))
        != 0;

    if !is_player {
        if driver::npc_msg(gs, cn, msg_type, dat1, dat2, dat3, dat4) != 0 {
            return;
        }
    }

    let is_ccp = (gs.characters[cn].flags & CharacterFlags::ComputerControlledPlayer.bits()) != 0;
    if is_ccp {
        log::error!("driver_ccp::ccp_msg not implemented for {}", cn);
    }

    match msg_type as u32 {
        x if x == core::constants::NT_GOTHIT as u32 || x == core::constants::NT_GOTMISS as u32 => {
            let attack_cn = gs.characters[cn].attack_cn as i32;
            let fightback = gs.characters[cn].data[core::constants::CHD_FIGHTBACK];
            let misc_action = gs.characters[cn].misc_action;
            if attack_cn == 0 && fightback == 0 && misc_action != core::constants::DR_GIVE as u16 {
                gs.characters[cn].attack_cn = dat1 as u16;
            }
        }
        _ => {}
    }
}

pub fn follow_driver(gs: &mut GameState, cn: usize, co: usize) -> bool {
    if co == 0 || co >= core::constants::MAXCHARS {
        return false;
    }
    let (tox, toy, dir) = (
        gs.characters[co].tox as i32,
        gs.characters[co].toy as i32,
        gs.characters[co].dir as i32,
    );
    if !(5..=core::constants::SERVER_MAPX - 6).contains(&tox)
        || !(5..=core::constants::SERVER_MAPY - 6).contains(&toy)
    {
        return false;
    }

    let is_companion = (gs.characters[cn].temp == core::constants::CT_COMPANION as u16)
        && gs.characters[cn].data[63] as usize == co;
    let can_see = gs.do_char_can_see(cn, co) != 0;
    if !(is_companion || can_see) {
        return false;
    }

    let mut m = tox + toy * core::constants::SERVER_MAPX;
    let dir_val = dir as u8;
    match dir_val {
        core::constants::DX_UP => m += core::constants::SERVER_MAPX * 2,
        core::constants::DX_DOWN => m -= core::constants::SERVER_MAPX * 2,
        core::constants::DX_LEFT => m += 2,
        core::constants::DX_RIGHT => m -= 2,
        core::constants::DX_LEFTUP => m += 2 + core::constants::SERVER_MAPX * 2,
        core::constants::DX_LEFTDOWN => m += 2 - core::constants::SERVER_MAPX * 2,
        core::constants::DX_RIGHTUP => m -= 2 - core::constants::SERVER_MAPX * -2,
        core::constants::DX_RIGHTDOWN => m -= 2 + core::constants::SERVER_MAPX * 2,
        _ => {}
    }

    let map_len = gs.map.len();
    let mut is_adjacent = false;
    let check_indices = vec![
        m,
        m + 1,
        m - 1,
        m + core::constants::SERVER_MAPX,
        m - core::constants::SERVER_MAPX,
        m + 1 + core::constants::SERVER_MAPX,
        m + 1 - core::constants::SERVER_MAPX,
        m - 1 + core::constants::SERVER_MAPX,
        m - 1 - core::constants::SERVER_MAPX,
    ];
    for idx in check_indices.iter() {
        if *idx < 0 || *idx as usize >= map_len {
            continue;
        }
        let ch_val = gs.map[*idx as usize].ch;
        if ch_val as usize == cn {
            is_adjacent = true;
            break;
        }
    }

    if is_adjacent {
        let cur_dir = gs.characters[cn].dir as i32;
        if cur_dir as u8 == dir_val {
            gs.characters[cn].misc_action = core::constants::DR_IDLE as u16;
            return true;
        }
        gs.characters[cn].misc_action = core::constants::DR_TURN as u16;
        let (x, y) = (gs.characters[cn].x as i32, gs.characters[cn].y as i32);
        match dir_val {
            core::constants::DX_UP => {
                gs.characters[cn].misc_target1 = x as u16;
                gs.characters[cn].misc_target2 = (y - 1) as u16;
            }
            core::constants::DX_DOWN => {
                gs.characters[cn].misc_target1 = x as u16;
                gs.characters[cn].misc_target2 = (y + 1) as u16;
            }
            core::constants::DX_LEFT => {
                gs.characters[cn].misc_target1 = (x - 1) as u16;
                gs.characters[cn].misc_target2 = y as u16;
            }
            core::constants::DX_RIGHT => {
                gs.characters[cn].misc_target1 = (x + 1) as u16;
                gs.characters[cn].misc_target2 = y as u16;
            }
            core::constants::DX_LEFTUP => {
                gs.characters[cn].misc_target1 = (x - 1) as u16;
                gs.characters[cn].misc_target2 = (y - 1) as u16;
            }
            core::constants::DX_LEFTDOWN => {
                gs.characters[cn].misc_target1 = (x - 1) as u16;
                gs.characters[cn].misc_target2 = (y + 1) as u16;
            }
            core::constants::DX_RIGHTUP => {
                gs.characters[cn].misc_target1 = (x + 1) as u16;
                gs.characters[cn].misc_target2 = (y - 1) as u16;
            }
            core::constants::DX_RIGHTDOWN => {
                gs.characters[cn].misc_target1 = (x + 1) as u16;
                gs.characters[cn].misc_target2 = (y + 1) as u16;
            }
            _ => {
                gs.characters[cn].misc_action = core::constants::DR_IDLE as u16;
            }
        }
        return true;
    }

    let mut found = false;
    let mut new_m = m;
    let offsets = [
        0,
        1,
        -1,
        core::constants::SERVER_MAPX,
        (-core::constants::SERVER_MAPX),
        1 + core::constants::SERVER_MAPX,
        1 - core::constants::SERVER_MAPX,
        -1 + core::constants::SERVER_MAPX,
        -1 - core::constants::SERVER_MAPX,
    ];
    for off in offsets.iter() {
        let try_m = m + off;
        if try_m < 0 || try_m as usize >= map_len {
            continue;
        }
        if player::plr_check_target(gs, try_m as usize) {
            new_m = try_m;
            found = true;
            break;
        }
    }
    if !found {
        return false;
    }
    gs.characters[cn].goto_x = (new_m % core::constants::SERVER_MAPX) as u16;
    gs.characters[cn].goto_y = (new_m / core::constants::SERVER_MAPX) as u16;
    true
}

pub fn driver(gs: &mut GameState, cn: usize) {
    let is_player_or_usurp = (gs.characters[cn].flags
        & (CharacterFlags::Player.bits() | CharacterFlags::Usurp.bits()))
        != 0;
    if !is_player_or_usurp {
        driver::npc_driver_high(gs, cn);
    }

    let use_nr = gs.characters[cn].use_nr;
    if use_nr != 0 {
        drv_use(gs, cn, use_nr as i32);
        return;
    }

    let skill_nr = gs.characters[cn].skill_nr;
    if skill_nr != 0 {
        drv_skill(gs, cn);
        return;
    }

    let is_player_or_usurp = (gs.characters[cn].flags
        & (CharacterFlags::Player.bits() | CharacterFlags::Usurp.bits()))
        != 0;
    let attack_cn = gs.characters[cn].attack_cn;
    if is_player_or_usurp && attack_cn == 0 {
        player::player_driver_med(gs, cn);
    }

    let goto_x = gs.characters[cn].goto_x;
    if goto_x != 0 {
        let goto_y = gs.characters[cn].goto_y;
        drv_moveto(gs, cn, goto_x as usize, goto_y as usize);
        return;
    }

    let attack_cn = gs.characters[cn].attack_cn;
    if attack_cn != 0 {
        drv_attack_char(gs, cn, attack_cn as usize);
        return;
    }

    let misc_action = gs.characters[cn].misc_action;
    match misc_action as u32 {
        x if x == core::constants::DR_IDLE => {
            let is_player = (gs.characters[cn].flags
                & (CharacterFlags::Player.bits() | CharacterFlags::Usurp.bits()))
                != 0;
            if !is_player {
                driver::npc_driver_low(gs, cn);
            }
        }
        x if x == core::constants::DR_DROP => {
            let t1 = gs.characters[cn].misc_target1 as usize;
            let t2 = gs.characters[cn].misc_target2 as usize;
            drv_dropto(gs, cn, t1, t2);
        }
        x if x == core::constants::DR_PICKUP => {
            let t1 = gs.characters[cn].misc_target1 as usize;
            let t2 = gs.characters[cn].misc_target2 as usize;
            drv_pickupto(gs, cn, t1, t2);
        }
        x if x == core::constants::DR_GIVE => {
            let t1 = gs.characters[cn].misc_target1 as usize;
            drv_give_char(gs, cn, t1);
        }
        x if x == core::constants::DR_USE => {
            let t1 = gs.characters[cn].misc_target1 as usize;
            let t2 = gs.characters[cn].misc_target2 as usize;
            drv_useto(gs, cn, t1, t2);
        }
        x if x == core::constants::DR_BOW => {
            log::debug!("drv_bow called for cn {}", cn);
            drv_bow(gs, cn);
        }
        x if x == core::constants::DR_WAVE => {
            drv_wave(gs, cn);
        }
        x if x == core::constants::DR_TURN => {
            let t1 = gs.characters[cn].misc_target1 as usize;
            let t2 = gs.characters[cn].misc_target2 as usize;
            drv_turnto(gs, cn, t1, t2);
        }
        x if x == core::constants::DR_SINGLEBUILD => {}
        x if x == core::constants::DR_AREABUILD1 => {}
        x if x == core::constants::DR_AREABUILD2 => {}
        _ => {
            log::error!("player_driver(): unknown misc_action {}", misc_action);
            gs.characters[cn].misc_action = core::constants::DR_IDLE as u16;
        }
    }
}
