use crate::effect::EffectManager;
use crate::game_state::GameState;
use crate::god::God;
use crate::{driver, player};
use core::constants::*;
use core::types::Character;

struct Seen {
    co: usize,
    dist: i32,
    is_friend: bool,
    stun: i32,
    help: i32,
}

/// TODO: Document the purpose, inputs, and outputs of this function.
pub fn npc_stunrun_high(gs: &mut GameState, cn: usize) -> i32 {
    let mut seen: [Seen; 30] = [const {
        Seen {
            co: 0,
            dist: 0,
            is_friend: false,
            stun: 0,
            help: 0,
        }
    }; 30];
    let mut maxseen = 0;
    let mut flee = 0;
    let mut help = 0;
    let mut stun = 0;
    let mut up = 0;
    let mut down = 0;
    let mut left = 0;
    let mut right = 0;
    let mut done = false;

    gs.characters[cn].data[92] = TICKS * 60;

    for n in 0..20 {
        let co = gs.characters[cn].data[n] as usize;
        if co != 0 && Character::is_sane_character(co) {
            let co_team = gs.characters[co].data[42];
            let cn_team = gs.characters[cn].data[42];

            if co_team == cn_team {
                seen[maxseen].co = co;
                seen[maxseen].dist = driver::npc_dist(gs, cn, co);
                seen[maxseen].is_friend = true;
                seen[maxseen].stun = 0;
                let low_hp = gs.characters[co].a_hp < (gs.characters[co].hp[5] as i32 * 400);
                seen[maxseen].help = if low_hp { 1 } else { 0 };
                help = help.max(seen[maxseen].help);
                maxseen += 1;
            } else {
                seen[maxseen].co = co;
                seen[maxseen].dist = driver::npc_dist(gs, cn, co);
                seen[maxseen].is_friend = false;
                if !driver::npc_is_stunned(gs, co) {
                    let can_stun = gs.characters[cn].skill[SK_STUN][5] * 12
                        > gs.characters[co].skill[SK_RESIST][5] * 10;
                    seen[maxseen].stun = if can_stun { 1 } else { 0 };
                } else {
                    seen[maxseen].stun = 0;
                }
                stun = stun.max(seen[maxseen].stun);
                seen[maxseen].help = 0;
                if seen[maxseen].dist < 6 {
                    flee += 1;
                }
                if seen[maxseen].dist < 4 {
                    flee += 1;
                }
                if seen[maxseen].dist < 2 {
                    flee += 2;
                    if seen[maxseen].stun != 0 {
                        seen[maxseen].stun += 5;
                        stun = stun.max(seen[maxseen].stun);
                    }
                }
                maxseen += 1;
            }
        }
    }

    for n in 30..35 {
        let co = gs.characters[cn].data[n] as usize;
        if co != 0 && Character::is_sane_character(co) {
            for m in 0..maxseen {
                if seen[m].co == co {
                    let co_team = gs.characters[co].data[42];
                    let cn_team = gs.characters[cn].data[42];

                    if co_team == cn_team {
                        seen[m].help += 1;
                        help = help.max(seen[m].help);
                    } else {
                        if seen[m].stun != 0 {
                            seen[m].stun += 2;
                        }
                        stun = stun.max(seen[m].stun);
                    }
                    break;
                }
            }
        }
    }

    let co = gs.characters[cn].data[20] as usize;
    if co != 0 && Character::is_sane_character(co) {
        flee += 5;
        let mut m = 0;
        for i in 0..maxseen {
            if seen[i].co == co {
                if seen[i].stun != 0 {
                    seen[i].stun += 5;
                } else {
                    flee += 2;
                }
                stun = stun.max(seen[i].stun);
                m = i + 1;
                break;
            }
        }
        if m == 0 {
            seen[maxseen].co = co;
            seen[maxseen].dist = driver::npc_dist(gs, cn, co);
            seen[maxseen].is_friend = false;
            let can_stun = gs.characters[cn].skill[SK_STUN][5] * 12
                > gs.characters[co].skill[SK_RESIST][5] * 10;
            seen[maxseen].stun = if can_stun { 1 } else { 0 };
            if seen[maxseen].stun != 0 {
                seen[maxseen].stun += 5;
            } else {
                flee += 2;
            }
            stun = stun.max(seen[maxseen].stun);
            seen[maxseen].help = 0;
            maxseen += 1;
        }
    }

    let low_mana = gs.characters[cn].a_mana < (gs.characters[cn].mana[5] as i32 * 125);
    if low_mana {
        stun -= 3;
        help -= 3;
        flee += 1;
    }

    gs.characters[cn].use_nr = 0;
    gs.characters[cn].skill_nr = 0;
    gs.characters[cn].attack_cn = 0;
    gs.characters[cn].goto_x = 0;
    gs.characters[cn].goto_y = 0;
    gs.characters[cn].misc_action = 0;
    gs.characters[cn].cerrno = 0;

    let low_hp = gs.characters[cn].a_hp < (gs.characters[cn].hp[5] as i32 * 666);
    if low_hp {
        flee += 5;
    }

    if !done && low_hp {
        done = driver::npc_try_spell(gs, cn, cn, SK_HEAL);
    }

    let high_endurance = gs.characters[cn].a_end > 15000;
    gs.characters[cn].mode = if high_endurance { 1 } else { 0 };

    if !done && flee > 1 && flee >= help && flee >= stun {
        gs.characters[cn].mode = if gs.characters[cn].a_end > 15000 {
            2
        } else {
            1
        };

        for n in 0..maxseen {
            let tmp = if !seen[n].is_friend {
                if seen[n].dist < 6 {
                    -2000
                } else {
                    -1000
                }
            } else {
                150
            };

            let co = seen[n].co;
            let (cn_x, cn_y, co_x, co_y) = (
                gs.characters[cn].x,
                gs.characters[cn].y,
                gs.characters[co].x,
                gs.characters[co].y,
            );

            if co_x > cn_x {
                right += tmp / (co_x - cn_x) as i32;
            }
            if co_x < cn_x {
                left += tmp / (cn_x - co_x) as i32;
            }
            if co_y > cn_y {
                down += tmp / (co_y - cn_y) as i32;
            }
            if co_y < cn_y {
                up += tmp / (cn_y - co_y) as i32;
            }
        }

        let (cn_x, cn_y) = (gs.characters[cn].x, gs.characters[cn].y);

        for n in 1..5 {
            if !driver::npc_check_target(gs, cn_x as usize, (cn_y - n) as usize) {
                up -= 20;
                if !driver::npc_check_target(gs, (cn_x + 1) as usize, (cn_y - n) as usize) {
                    up -= 20;
                    if !driver::npc_check_target(gs, (cn_x - 1) as usize, (cn_y - n) as usize) {
                        up -= 10000;
                        break;
                    }
                }
            }
        }

        for n in 1..5 {
            if !driver::npc_check_target(gs, cn_x as usize, (cn_y + n) as usize) {
                down -= 20;
                if !driver::npc_check_target(gs, (cn_x + 1) as usize, (cn_y + n) as usize) {
                    down -= 20;
                    if !driver::npc_check_target(gs, (cn_x - 1) as usize, (cn_y + n) as usize) {
                        down -= 10000;
                        break;
                    }
                }
            }
        }

        for n in 1..5 {
            if !driver::npc_check_target(gs, (cn_x - n) as usize, cn_y as usize) {
                left -= 20;
                if !driver::npc_check_target(gs, (cn_x - n) as usize, (cn_y + 1) as usize) {
                    left -= 20;
                    if !driver::npc_check_target(gs, (cn_x - n) as usize, (cn_y - n) as usize) {
                        left -= 10000;
                        break;
                    }
                }
            }
        }

        for n in 1..5 {
            if !driver::npc_check_target(gs, (cn_x + n) as usize, cn_y as usize) {
                right -= 20;
                if !driver::npc_check_target(gs, (cn_x + n) as usize, (cn_y + 1) as usize) {
                    right -= 20;
                    if !driver::npc_check_target(gs, (cn_x + n) as usize, (cn_y - n) as usize) {
                        right -= 10000;
                        break;
                    }
                }
            }
        }

        let dir = gs.characters[cn].dir;
        if dir == DX_UP {
            up += 20;
        }
        if dir == DX_DOWN {
            down += 20;
        }
        if dir == DX_LEFT {
            left += 20;
        }
        if dir == DX_RIGHT {
            right += 20;
        }

        if !done && up >= down && up >= left && up >= right {
            if driver::npc_check_target(gs, cn_x as usize, (cn_y - 1) as usize) {
                gs.characters[cn].goto_x = cn_x as u16;
                gs.characters[cn].goto_y = (cn_y - 1) as u16;
                done = true;
            } else if driver::npc_check_target(gs, (cn_x + 1) as usize, (cn_y - 1) as usize) {
                gs.characters[cn].goto_x = (cn_x + 1) as u16;
                gs.characters[cn].goto_y = (cn_y - 1) as u16;
                done = true;
            } else if driver::npc_check_target(gs, (cn_x - 1) as usize, (cn_y - 1) as usize) {
                gs.characters[cn].goto_x = (cn_x - 1) as u16;
                gs.characters[cn].goto_y = (cn_y - 1) as u16;
                done = true;
            }
        }

        if !done && down >= up && down >= left && down >= right {
            if driver::npc_check_target(gs, cn_x as usize, (cn_y + 1) as usize) {
                gs.characters[cn].goto_x = cn_x as u16;
                gs.characters[cn].goto_y = (cn_y + 1) as u16;
                done = true;
            } else if driver::npc_check_target(gs, (cn_x + 1) as usize, (cn_y + 1) as usize) {
                gs.characters[cn].goto_x = (cn_x + 1) as u16;
                gs.characters[cn].goto_y = (cn_y + 1) as u16;
                done = true;
            } else if driver::npc_check_target(gs, (cn_x - 1) as usize, (cn_y + 1) as usize) {
                gs.characters[cn].goto_x = (cn_x - 1) as u16;
                gs.characters[cn].goto_y = (cn_y + 1) as u16;
                done = true;
            }
        }

        if !done && left >= up && left >= down && left >= right {
            if driver::npc_check_target(gs, (cn_x - 1) as usize, cn_y as usize) {
                gs.characters[cn].goto_x = (cn_x - 1) as u16;
                gs.characters[cn].goto_y = cn_y as u16;
                done = true;
            } else if driver::npc_check_target(gs, (cn_x - 1) as usize, (cn_y + 1) as usize) {
                gs.characters[cn].goto_x = (cn_x - 1) as u16;
                gs.characters[cn].goto_y = (cn_y + 1) as u16;
                done = true;
            } else if driver::npc_check_target(gs, (cn_x - 1) as usize, (cn_y - 1) as usize) {
                gs.characters[cn].goto_x = (cn_x - 1) as u16;
                gs.characters[cn].goto_y = (cn_y - 1) as u16;
                done = true;
            }
        }

        if !done && right >= up && right >= down && right >= left {
            if driver::npc_check_target(gs, (cn_x + 1) as usize, cn_y as usize) {
                gs.characters[cn].goto_x = (cn_x + 1) as u16;
                gs.characters[cn].goto_y = cn_y as u16;
                done = true;
            } else if driver::npc_check_target(gs, (cn_x + 1) as usize, (cn_y + 1) as usize) {
                gs.characters[cn].goto_x = (cn_x + 1) as u16;
                gs.characters[cn].goto_y = (cn_y + 1) as u16;
                done = true;
            } else if driver::npc_check_target(gs, (cn_x + 1) as usize, (cn_y - 1) as usize) {
                gs.characters[cn].goto_x = (cn_x + 1) as u16;
                gs.characters[cn].goto_y = (cn_y - 1) as u16;
                done = true;
            }
        }

        if !done {
            let co = gs.characters[cn].data[20] as usize;
            if co != 0 {
                gs.characters[cn].attack_cn = co as u16;
                driver::npc_try_spell(gs, cn, co, SK_STUN);
                done = true;
            }
        }
    }

    if !done {
        done = driver::npc_try_spell(gs, cn, cn, SK_BLESS);
    }
    if !done {
        done = driver::npc_try_spell(gs, cn, cn, SK_MSHIELD);
    }
    if !done {
        done = driver::npc_try_spell(gs, cn, cn, SK_PROTECT);
    }
    if !done {
        done = driver::npc_try_spell(gs, cn, cn, SK_ENHANCE);
    }

    if !done && stun > 1 && stun >= help {
        let mut m = 0;
        let mut tmp = 0;
        for n in 0..maxseen {
            if seen[n].stun > tmp
                || (seen[n].stun != 0 && seen[n].stun == tmp && seen[n].dist < seen[m].dist)
            {
                tmp = seen[n].stun;
                m = n;
            }
        }
        if tmp > 0 {
            done = driver::npc_try_spell(gs, cn, seen[m].co, SK_STUN);
            if !done {
                done = driver::npc_try_spell(gs, cn, seen[m].co, SK_CURSE);
            }
            gs.characters[cn].data[24] = gs.globals.ticker;
        }
    }

    if !done && help > 0 {
        let mut m = 0;
        let mut tmp = 0;
        for n in 0..maxseen {
            if seen[n].help > tmp
                || (seen[n].help != 0 && seen[n].help == tmp && seen[n].dist < seen[m].dist)
            {
                let needs_help = !driver::npc_is_blessed(gs, seen[n].co)
                    || gs.characters[seen[n].co].a_hp
                        < (gs.characters[seen[n].co].hp[5] * 400) as i32;
                if needs_help {
                    tmp = seen[n].help;
                    m = n;
                }
            }
        }
        if tmp > 0 {
            let low_hp =
                gs.characters[seen[m].co].a_hp < (gs.characters[seen[m].co].hp[5] * 400) as i32;
            if low_hp {
                done = driver::npc_try_spell(gs, cn, seen[m].co, SK_HEAL);
            }
            if !done {
                done = driver::npc_try_spell(gs, cn, seen[m].co, SK_BLESS);
            }
            if !done {
                done = driver::npc_try_spell(gs, cn, seen[m].co, SK_PROTECT);
            }
            if !done {
                done = driver::npc_try_spell(gs, cn, seen[m].co, SK_ENHANCE);
            }
            gs.characters[cn].data[24] = gs.globals.ticker;
        }
    }

    if !done {
        let state = gs.characters[cn].data[22];

        if state == 0 {
            let in_item = gs.characters[cn].citem;
            if in_item != 0 {
                gs.characters[cn].citem = 0;
                gs.items[in_item as usize].used = USE_EMPTY;
            }
            if gs.characters[cn].data[23] == 0 {
                gs.characters[cn].data[23] = gs.globals.ticker;
            }
            let ticker = gs.globals.ticker;
            let data_23 = gs.characters[cn].data[23];
            if data_23 + TICKS * 60 * 60 < ticker {
                let mut tmp = 0;
                for y in 322..=332 {
                    if tmp != 0 {
                        break;
                    }
                    for x in 212..=232 {
                        let co = gs.map[(x + y * SERVER_MAPX) as usize].ch;
                        if co != 0 {
                            let co_team = gs.characters[co as usize].data[42];
                            let cn_team = gs.characters[cn].data[42];
                            if co_team != cn_team {
                                tmp = 1;
                                break;
                            }
                        }
                    }
                }
                if tmp == 0 {
                    gs.characters[cn].data[22] = 1;
                }
                gs.characters[cn].data[23] = ticker;
            }
        }

        let ticker = gs.globals.ticker;
        let data_24 = gs.characters[cn].data[24];
        if state == 1 && ticker > data_24 + TICKS * 10 {
            if gs.characters[cn].citem == 0 {
                if let Some(in_item) = God::create_item(gs, 718) {
                    gs.characters[cn].citem = in_item as u32;
                    gs.items[in_item].carried = cn as u16;
                }
            }
            let (cn_x, cn_y) = (gs.characters[cn].x, gs.characters[cn].y);
            if (cn_x as i32 - 264).abs() + (cn_y as i32 - 317).abs() < 20 {
                gs.characters[cn].data[22] = 2;
                gs.characters[cn].data[23] = ticker;
            } else {
                if driver::npc_check_target(gs, 264, 317) {
                    gs.characters[cn].goto_x = 264;
                    gs.characters[cn].goto_y = 317;
                } else if driver::npc_check_target(gs, 265, 318) {
                    gs.characters[cn].goto_x = 265;
                    gs.characters[cn].goto_y = 318;
                }
                if cn_x > 232 {
                    gs.characters[cn].data[24] = ticker;
                } else {
                    gs.characters[cn].data[24] = 0;
                }
            }
        }

        if state == 2 {
            let (cn_x, cn_y) = (gs.characters[cn].x, gs.characters[cn].y);
            if (cn_x as i32 - 217).abs() + (cn_y as i32 - 349).abs() < 3 {
                let ticker = gs.globals.ticker;
                gs.characters[cn].data[22] = 0;
                gs.characters[cn].data[23] = ticker;
                gs.characters[cn].data[40] += 1;
            } else {
                gs.characters[cn].goto_x = 217;
                gs.characters[cn].goto_y = 349;
            }
        }
    }

    let ticker = gs.globals.ticker;
    for n in 0..20 {
        if gs.characters[cn].data[n + 50] + TICKS * 2 < ticker {
            gs.characters[cn].data[n] = 0;
        }
    }
    for n in 30..35 {
        if gs.characters[cn].data[n + 5] + TICKS * 2 < ticker {
            gs.characters[cn].data[n] = 0;
        }
    }
    if gs.characters[cn].data[21] + TICKS * 2 < ticker {
        gs.characters[cn].data[20] = 0;
    }

    0
}

pub fn npc_stunrun_low(_gs: &mut GameState, _cn: usize) -> i32 {
    // Empty function - does nothing in the original C++ implementation
    0
}

fn npc_stunrun_add_seen(gs: &mut GameState, cn: usize, co: usize) -> i32 {
    let ticker = gs.globals.ticker;

    // Check if co is already in the seen list (data[0-19])
    for n in 0..20 {
        let data_n = gs.characters[cn].data[n];
        if data_n == co as i32 {
            gs.characters[cn].data[n + 50] = ticker;
            return 1;
        }
    }

    // Find an empty slot and add co
    for n in 0..20 {
        let data_n = gs.characters[cn].data[n];
        if data_n == 0 {
            gs.characters[cn].data[n] = co as i32;
            gs.characters[cn].data[n + 50] = ticker;
            break;
        }
    }

    1
}

fn npc_stunrun_gotattack(gs: &mut GameState, cn: usize, co: usize) -> i32 {
    npc_stunrun_add_seen(gs, cn, co);
    gs.characters[cn].data[20] = co as i32;
    1
}

fn npc_stunrun_add_fight(gs: &mut GameState, cn: usize, co: usize) {
    let ticker = gs.globals.ticker;

    // Check if co is already in the fight list (data[30-34])
    for n in 30..35 {
        let data_n = gs.characters[cn].data[n];
        if data_n == co as i32 {
            gs.characters[cn].data[n + 5] = ticker;
            return;
        }
    }

    // Find an empty slot and add co
    for n in 30..35 {
        let data_n = gs.characters[cn].data[n];
        if data_n == 0 {
            gs.characters[cn].data[n] = co as i32;
            gs.characters[cn].data[n + 5] = ticker;
            break;
        }
    }
}

fn npc_stunrun_seeattack(gs: &mut GameState, cn: usize, cc: usize, co: usize) -> i32 {
    // TODO: Double check this implementation now...
    if gs.do_char_can_see(cn, co) != 0 {
        npc_stunrun_add_seen(gs, cn, co);
        npc_stunrun_add_fight(gs, cn, co);
    }
    if gs.do_char_can_see(cn, cc) != 0 {
        npc_stunrun_add_seen(gs, cn, cc);
        npc_stunrun_add_fight(gs, cn, cc);
    }
    1
}

fn npc_stunrun_see(gs: &mut GameState, cn: usize, co: usize) -> i32 {
    if gs.do_char_can_see(cn, co) == 0 {
        return 1; // processed it: we cannot see him, so ignore him
    }

    npc_stunrun_add_seen(gs, cn, co);
    1
}

pub fn npc_stunrun_msg(
    gs: &mut GameState,
    cn: usize,
    msg_type: u8,
    dat1: i32,
    dat2: i32,
    _dat3: i32,
    _dat4: i32,
) -> i32 {
    match msg_type {
        NT_GOTHIT => npc_stunrun_gotattack(gs, cn, dat1 as usize),
        NT_GOTMISS => npc_stunrun_gotattack(gs, cn, dat1 as usize),
        NT_DIDHIT => 0,
        NT_DIDMISS => 0,
        NT_DIDKILL => 0,
        NT_GOTEXP => 0,
        NT_SEEKILL => 0,
        NT_SEEHIT => npc_stunrun_seeattack(gs, cn, dat1 as usize, dat2 as usize),
        NT_SEEMISS => npc_stunrun_seeattack(gs, cn, dat1 as usize, dat2 as usize),
        NT_GIVE => 0,
        NT_SEE => npc_stunrun_see(gs, cn, dat1 as usize),
        NT_DIED => 0,
        NT_SHOUT => 0,
        NT_HITME => 0,
        _ => {
            let name = gs.characters[cn].get_name().to_string();
            log::warn!("Unknown NPC message for {} ({}): {}", cn, name, msg_type);
            0
        }
    }
}

pub fn npc_cityattack_high(gs: &mut GameState, cn: usize) -> i32 {
    let low_hp = gs.characters[cn].a_hp < (gs.characters[cn].hp[5] * 600) as i32;
    if low_hp && driver::npc_try_spell(gs, cn, cn, SK_HEAL) {
        return 1;
    }

    let high_mana = gs.characters[cn].a_mana > (gs.characters[cn].mana[5] as i32 * 850);
    let has_medit = gs.characters[cn].skill[SK_MEDIT][0] != 0;
    if high_mana && has_medit {
        let very_high_mana = gs.characters[cn].a_mana > 75000;
        if very_high_mana && driver::npc_try_spell(gs, cn, cn, SK_BLESS) {
            return 1;
        }
        if driver::npc_try_spell(gs, cn, cn, SK_PROTECT) {
            return 1;
        }
        if driver::npc_try_spell(gs, cn, cn, SK_MSHIELD) {
            return 1;
        }
        if driver::npc_try_spell(gs, cn, cn, SK_ENHANCE) {
            return 1;
        }
        if driver::npc_try_spell(gs, cn, cn, SK_BLESS) {
            return 1;
        }
    }

    let attack_cn = gs.characters[cn].attack_cn;
    let a_end = gs.characters[cn].a_end;
    let current_mode = gs.characters[cn].mode;

    if attack_cn != 0 && a_end > 10000 {
        if current_mode != 2 {
            gs.characters[cn].mode = 2;
            gs.characters[cn].set_do_update_flags();
        }
    } else if a_end > 10000 {
        if current_mode != 1 {
            gs.characters[cn].mode = 1;
            gs.characters[cn].set_do_update_flags();
        }
    } else if current_mode != 0 {
        gs.characters[cn].mode = 0;
        gs.characters[cn].set_do_update_flags();
    }

    let co = gs.characters[cn].attack_cn;
    if co != 0 {
        let losing = gs.characters[cn].a_hp < (gs.characters[cn].hp[5] * 600) as i32;
        if losing && driver::npc_try_spell(gs, cn, co as usize, SK_BLAST) {
            return 1;
        }

        let ticker = gs.globals.ticker;
        let data_75 = gs.characters[cn].data[75];
        if ticker > data_75 && driver::npc_try_spell(gs, cn, co as usize, SK_STUN) {
            gs.characters[cn].data[75] =
                ticker + gs.characters[cn].skill[SK_STUN][5] as i32 + TICKS * 8;
            return 1;
        }

        let very_high_mana = gs.characters[cn].a_mana > 75000;
        if very_high_mana && driver::npc_try_spell(gs, cn, cn, SK_BLESS) {
            return 1;
        }
        if driver::npc_try_spell(gs, cn, cn, SK_PROTECT) {
            return 1;
        }
        if driver::npc_try_spell(gs, cn, cn, SK_MSHIELD) {
            return 1;
        }
        if driver::npc_try_spell(gs, cn, cn, SK_ENHANCE) {
            return 1;
        }
        if driver::npc_try_spell(gs, cn, cn, SK_BLESS) {
            return 1;
        }
        if driver::npc_try_spell(gs, cn, co as usize, SK_CURSE) {
            return 1;
        }

        let data_74 = gs.characters[cn].data[74];
        if ticker > data_74 + TICKS * 10 && driver::npc_try_spell(gs, cn, co as usize, SK_GHOST) {
            gs.characters[cn].data[74] = ticker;
            return 1;
        }

        let cannot_hurt = gs.characters[co as usize].armor + 5 > gs.characters[cn].weapon;
        if cannot_hurt && driver::npc_try_spell(gs, cn, co as usize, SK_BLAST) {
            return 1;
        }
    }

    0
}

pub fn npc_moveto(gs: &mut GameState, cn: usize, x: u16, y: u16) -> i32 {
    let (cn_x, cn_y) = (gs.characters[cn].x, gs.characters[cn].y);

    if (cn_x as i32 - x as i32).abs() < 3 && (cn_y as i32 - y as i32).abs() < 3 {
        gs.characters[cn].data[1] = 0;
        return 1;
    }

    let data_1 = gs.characters[cn].data[1];
    if data_1 == 0 && driver::npc_check_target(gs, x as usize, y as usize) {
        gs.characters[cn].data[1] += 1;
        gs.characters[cn].goto_x = x;
        gs.characters[cn].goto_y = y;
        return 0;
    }

    let mut try_count = 1;
    for dx in 0..3 {
        for dy in 0..3 {
            try_count += 1;
            let data_1 = gs.characters[cn].data[1];
            if data_1 < try_count
                && driver::npc_check_target(gs, (x + dx) as usize, (y + dy) as usize)
            {
                gs.characters[cn].data[1] += 1;
                gs.characters[cn].goto_x = x + dx;
                gs.characters[cn].goto_y = y + dy;
                return 0;
            }
            if data_1 < try_count
                && x >= dx
                && driver::npc_check_target(gs, (x - dx) as usize, (y + dy) as usize)
            {
                gs.characters[cn].data[1] += 1;
                gs.characters[cn].goto_x = x - dx;
                gs.characters[cn].goto_y = y + dy;
                return 0;
            }
            if data_1 < try_count
                && y >= dy
                && driver::npc_check_target(gs, (x + dx) as usize, (y - dy) as usize)
            {
                gs.characters[cn].data[1] += 1;
                gs.characters[cn].goto_x = x + dx;
                gs.characters[cn].goto_y = y - dy;
                return 0;
            }
            if data_1 < try_count
                && x >= dx
                && y >= dy
                && driver::npc_check_target(gs, (x - dx) as usize, (y - dy) as usize)
            {
                gs.characters[cn].data[1] += 1;
                gs.characters[cn].goto_x = x - dx;
                gs.characters[cn].goto_y = y - dy;
                return 0;
            }
        }
    }

    gs.characters[cn].data[1] = 0;
    0
}

fn npc_cityattack_wait(gs: &GameState) -> i32 {
    let mdtime = gs.globals.mdtime;
    if (mdtime % 28800) < 20 {
        1
    } else {
        0
    }
}

pub fn npc_cityattack_low(gs: &mut GameState, cn: usize) -> i32 {
    let state = gs.characters[cn].data[0];

    let ret = match state {
        0 => npc_moveto(gs, cn, 456, 356),
        1 => npc_moveto(gs, cn, 447, 356),
        2 => npc_moveto(gs, cn, 447, 362),
        3 => npc_moveto(gs, cn, 474, 362),
        4 => npc_cityattack_wait(gs),
        5 => npc_moveto(gs, cn, 486, 362),
        6 => npc_moveto(gs, cn, 509, 362),
        7 => npc_moveto(gs, cn, 526, 362),
        8 => npc_moveto(gs, cn, 531, 386),
        9 => npc_moveto(gs, cn, 534, 403),
        _ => 0,
    };

    if ret != 0 {
        gs.characters[cn].data[0] += 1;
    }

    0
}

fn npc_cityattack_gotattack(_cn: usize, _co: usize) -> i32 {
    1
}

fn npc_cityattack_seeattack(gs: &mut GameState, cn: usize, cc: usize, co: usize) -> i32 {
    gs.do_char_can_see(cn, co);
    gs.do_char_can_see(cn, cc);
    1
}

fn npc_cityattack_see(gs: &mut GameState, cn: usize, co: usize) -> i32 {
    if gs.do_char_can_see(cn, co) == 0 {
        return 1;
    }

    let cn_team = gs.characters[cn].data[42];
    let co_team = gs.characters[co].data[42];
    if cn_team != co_team {
        let cc = gs.characters[cn].attack_cn as usize;
        if cc == 0 || crate::driver::npc_dist(gs, cn, co) < crate::driver::npc_dist(gs, cn, cc) {
            gs.characters[cn].attack_cn = co as u16;
            gs.characters[cn].goto_x = 0;
        }
    }

    1
}

pub fn npc_cityattack_msg(
    gs: &mut GameState,
    cn: usize,
    msg_type: i32,
    dat1: i32,
    dat2: i32,
    _dat3: i32,
    _dat4: i32,
) -> i32 {
    match msg_type {
        x if x == NT_GOTHIT as i32 => npc_cityattack_gotattack(cn, dat1 as usize),
        x if x == NT_GOTMISS as i32 => npc_cityattack_gotattack(cn, dat1 as usize),
        x if x == NT_DIDHIT as i32 => 0,
        x if x == NT_DIDMISS as i32 => 0,
        x if x == NT_DIDKILL as i32 => 0,
        x if x == NT_GOTEXP as i32 => 0,
        x if x == NT_SEEKILL as i32 => 0,
        x if x == NT_SEEHIT as i32 => {
            npc_cityattack_seeattack(gs, cn, dat1 as usize, dat2 as usize)
        }
        x if x == NT_SEEMISS as i32 => {
            npc_cityattack_seeattack(gs, cn, dat1 as usize, dat2 as usize)
        }
        x if x == NT_GIVE as i32 => 0,
        x if x == NT_SEE as i32 => npc_cityattack_see(gs, cn, dat1 as usize),
        x if x == NT_DIED as i32 => 0,
        x if x == NT_SHOUT as i32 => 0,
        x if x == NT_HITME as i32 => 0,
        _ => {
            let name = gs.characters[cn].get_name().to_string();
            log::warn!("Unknown NPC message for {} ({}): {}", cn, name, msg_type);
            0
        }
    }
}

pub fn npc_malte_high(_gs: &mut GameState, _character_id: usize) -> i32 {
    0
}

pub fn npc_malte_low(gs: &mut GameState, cn: usize) -> i32 {
    let ticker = gs.globals.ticker;
    let data_2 = gs.characters[cn].data[2];
    if ticker < data_2 {
        return 0;
    }

    let co = gs.characters[cn].data[0] as usize;
    let state = gs.characters[cn].data[1];

    match state {
        0 => {
            let co_name = gs.characters[co].get_name().to_string();
            let message = format!("Thank you so much for saving me, {}!", co_name);
            gs.do_sayx(cn, &message);

            gs.characters[cn].data[2] = ticker + TICKS * 8;
            gs.characters[cn].data[1] += 1;
            gs.characters[cn].misc_action = DR_TURN as u16;
            gs.characters[cn].misc_target1 = DX_DOWN as u16;
        }
        1 => {
            gs.do_sayx(
                cn,
                "Before the monsters caught me, I discovered that you need a coin to open certain doors down here.",
            );

            gs.characters[cn].data[2] = ticker + TICKS * 8;
            gs.characters[cn].data[1] += 1;
        }
        2 => {
            gs.do_sayx(
                cn,
                "I found this part of the coin, and I heard that Damor in Aston has another one. Ask him for the 'Black Stronghold Coin'.",
            );

            if let Some(in_item) = crate::god::God::create_item(gs, 763) {
                gs.characters[cn].citem = in_item as u32;
                gs.characters[cn].data[2] = ticker + TICKS * 5;
                gs.characters[cn].data[1] += 1;
                gs.characters[cn].misc_action = DR_GIVE as u16;
                gs.characters[cn].misc_target1 = co as u16;
                gs.items[in_item].carried = cn as u16;
            }
        }
        3 => {
            gs.do_sayx(
                cn,
                "Shiva, the mage who creates all the monsters, has the third part of it.",
            );

            gs.characters[cn].data[2] = ticker + TICKS * 8;
            gs.characters[cn].data[1] += 1;
        }
        4 => {
            gs.do_sayx(cn, "I have no idea where the other parts are.");

            gs.characters[cn].data[2] = ticker + TICKS * 8;
            gs.characters[cn].data[1] += 1;
        }
        5 => {
            gs.do_sayx(cn, "I will recall now. I have enough of this prison!");

            let (cn_x, cn_y) = (gs.characters[cn].x, gs.characters[cn].y);
            EffectManager::fx_add_effect(7, 0, cn_x as i32, cn_y as i32, 0);

            gs.characters[cn].data[2] = ticker + TICKS * 6;
            gs.characters[cn].data[1] += 1;
        }
        6 => {
            gs.do_sayx(cn, "Good luck my friend. And thank you for freeing me!");

            player::plr_map_remove(cn);
            God::destroy_items(gs, cn);
            gs.characters[cn].used = USE_EMPTY;
        }
        _ => {}
    }

    0
}

fn npc_malte_gotattack(gs: &mut GameState, cn: usize, co: usize) -> i32 {
    let cn_team = gs.characters[cn].data[42];
    let co_team = gs.characters[co].data[42];

    if cn_team != co_team {
        let cc = gs.characters[cn].attack_cn as usize;
        if cc == 0 || crate::driver::npc_dist(gs, cn, co) < crate::driver::npc_dist(gs, cn, cc) {
            gs.characters[cn].attack_cn = co as u16;
            gs.characters[cn].goto_x = 0;
        }
    }

    1
}

pub fn npc_malte_msg(
    gs: &mut GameState,
    cn: usize,
    msg_type: i32,
    dat1: i32,
    _dat2: i32,
    _dat3: i32,
    _dat4: i32,
) -> i32 {
    match msg_type {
        x if x == NT_GOTHIT as i32 => npc_malte_gotattack(gs, cn, dat1 as usize),
        x if x == NT_GOTMISS as i32 => npc_malte_gotattack(gs, cn, dat1 as usize),
        x if x == NT_DIDHIT as i32 => 0,
        x if x == NT_DIDMISS as i32 => 0,
        x if x == NT_DIDKILL as i32 => 0,
        x if x == NT_GOTEXP as i32 => 0,
        x if x == NT_SEEKILL as i32 => 0,
        x if x == NT_SEEHIT as i32 => 0,
        x if x == NT_SEEMISS as i32 => 0,
        x if x == NT_GIVE as i32 => 0,
        x if x == NT_SEE as i32 => 0,
        x if x == NT_DIED as i32 => 0,
        x if x == NT_SHOUT as i32 => 0,
        x if x == NT_HITME as i32 => 0,
        _ => {
            let name = gs.characters[cn].get_name().to_string();
            log::warn!("Unknown NPC message for {} ({}): {}", cn, name, msg_type);
            0
        }
    }
}
