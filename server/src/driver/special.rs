use crate::effect::EffectManager;
use crate::game_state::GameState;
use crate::god::God;
use crate::{driver, player};
use core::types::Character;
use core::{constants::*, skills, traits::Class};

struct Seen {
    co: usize,
    dist: i32,
    is_friend: bool,
    stun: i32,
    help: i32,
}

///
/// # Arguments
///
/// * `gs` - Active game state used by this function.
/// * `cn` - Character index used by this function.
///
/// # Returns
///
/// * `true` when `npc_stunrun_high` succeeds or the condition is met, otherwise `false`.
pub fn npc_stunrun_high(gs: &mut GameState, cn: usize) -> bool {
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
                seen[maxseen].dist = driver::npc_dist(&gs.characters[cn], &gs.characters[co]);
                seen[maxseen].is_friend = true;
                seen[maxseen].stun = 0;
                let low_hp = gs.characters[co].a_hp < (i32::from(gs.characters[co].hp[5]) * 400);
                seen[maxseen].help = if low_hp { 1 } else { 0 };
                help = help.max(seen[maxseen].help);
                maxseen += 1;
            } else {
                seen[maxseen].co = co;
                seen[maxseen].dist = driver::npc_dist(&gs.characters[cn], &gs.characters[co]);
                seen[maxseen].is_friend = false;
                if !driver::npc_is_stunned(&gs.characters[co], &gs.items) {
                    let can_stun = i32::from(gs.characters[cn].skill[skills::SK_STUN][5]) * 12
                        > i32::from(gs.characters[co].skill[skills::SK_RESIST][5]) * 10;
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
            for item in seen[..maxseen].iter_mut() {
                if item.co == co {
                    let co_team = gs.characters[co].data[42];
                    let cn_team = gs.characters[cn].data[42];

                    if co_team == cn_team {
                        item.help += 1;
                        help = help.max(item.help);
                    } else {
                        if item.stun != 0 {
                            item.stun += 2;
                        }
                        stun = stun.max(item.stun);
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
        for (i, item) in seen[..maxseen].iter_mut().enumerate() {
            if item.co == co {
                if item.stun != 0 {
                    item.stun += 5;
                } else {
                    flee += 2;
                }
                stun = stun.max(item.stun);
                m = i + 1;
                break;
            }
        }
        if m == 0 {
            seen[maxseen].co = co;
            seen[maxseen].dist = driver::npc_dist(&gs.characters[cn], &gs.characters[co]);
            seen[maxseen].is_friend = false;
            let can_stun = i32::from(gs.characters[cn].skill[skills::SK_STUN][5]) * 12
                > i32::from(gs.characters[co].skill[skills::SK_RESIST][5]) * 10;
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

    let low_mana = gs.characters[cn].a_mana < (i32::from(gs.characters[cn].mana[5]) * 125);
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

    let low_hp = gs.characters[cn].a_hp < (i32::from(gs.characters[cn].hp[5]) * 666);
    if low_hp {
        flee += 5;
    }

    if !done && low_hp {
        done = driver::npc_try_spell(gs, cn, cn, skills::SK_HEAL);
    }

    let high_endurance = gs.characters[cn].a_end > 15000;
    gs.characters[cn].mode = if high_endurance { 1 } else { 0 };

    if !done && flee > 1 && flee >= help && flee >= stun {
        gs.characters[cn].mode = if gs.characters[cn].a_end > 15000 {
            2
        } else {
            1
        };

        for item in &seen[..maxseen] {
            let tmp = if !item.is_friend {
                if item.dist < 6 { -2000 } else { -1000 }
            } else {
                150
            };

            let co = item.co;
            let (cn_x, cn_y, co_x, co_y) = (
                gs.characters[cn].x,
                gs.characters[cn].y,
                gs.characters[co].x,
                gs.characters[co].y,
            );

            if co_x > cn_x {
                right += tmp / i32::from(co_x - cn_x);
            }
            if co_x < cn_x {
                left += tmp / i32::from(cn_x - co_x);
            }
            if co_y > cn_y {
                down += tmp / i32::from(co_y - cn_y);
            }
            if co_y < cn_y {
                up += tmp / i32::from(cn_y - co_y);
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
                driver::npc_try_spell(gs, cn, co, skills::SK_STUN);
                done = true;
            }
        }
    }

    if !done {
        done = driver::npc_try_spell(gs, cn, cn, skills::SK_BLESS);
    }
    if !done {
        done = driver::npc_try_spell(gs, cn, cn, skills::SK_MSHIELD);
    }
    if !done {
        done = driver::npc_try_spell(gs, cn, cn, skills::SK_PROTECT);
    }
    if !done {
        done = driver::npc_try_spell(gs, cn, cn, skills::SK_ENHANCE);
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
            done = driver::npc_try_spell(gs, cn, seen[m].co, skills::SK_STUN);
            if !done {
                done = driver::npc_try_spell(gs, cn, seen[m].co, skills::SK_CURSE);
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
                let needs_help = !driver::npc_is_blessed(&gs.characters[seen[n].co], &gs.items)
                    || gs.characters[seen[n].co].a_hp
                        < i32::from(gs.characters[seen[n].co].hp[5]) * 400;
                if needs_help {
                    tmp = seen[n].help;
                    m = n;
                }
            }
        }
        if tmp > 0 {
            let low_hp =
                gs.characters[seen[m].co].a_hp < i32::from(gs.characters[seen[m].co].hp[5]) * 400;
            if low_hp {
                done = driver::npc_try_spell(gs, cn, seen[m].co, skills::SK_HEAL);
            }
            if !done {
                done = driver::npc_try_spell(gs, cn, seen[m].co, skills::SK_BLESS);
            }
            if !done {
                done = driver::npc_try_spell(gs, cn, seen[m].co, skills::SK_PROTECT);
            }
            if !done {
                done = driver::npc_try_spell(gs, cn, seen[m].co, skills::SK_ENHANCE);
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
            if gs.characters[cn].citem == 0
                && let Some(in_item) = God::create_item(gs, 718)
            {
                gs.characters[cn].citem = in_item as u32;
                gs.items[in_item].carried = cn as u16;
            }
            let (cn_x, cn_y) = (gs.characters[cn].x, gs.characters[cn].y);
            if (i32::from(cn_x) - 264).abs() + (i32::from(cn_y) - 317).abs() < 20 {
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
            if (i32::from(cn_x) - 217).abs() + (i32::from(cn_y) - 349).abs() < 3 {
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

    false
}

/// Runs the low-priority tick for the stunrun NPC driver.
///
/// # Arguments
///
/// * `_gs` - Active game state, unused by this no-op legacy hook.
/// * `_cn` - NPC character index, unused by this no-op legacy hook.
///
/// # Returns
///
/// * Always `false`, matching the original no-op implementation.
pub fn npc_stunrun_low(_gs: &mut GameState, _cn: usize) -> bool {
    // Empty function - does nothing in the original C++ implementation
    false
}

fn npc_stunrun_add_seen(gs: &mut GameState, cn: usize, co: usize) -> bool {
    let ticker = gs.globals.ticker;

    // Check if co is already in the seen list (data[0-19])
    for n in 0..20 {
        let data_n = gs.characters[cn].data[n];
        if data_n == co as i32 {
            gs.characters[cn].data[n + 50] = ticker;
            return true;
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

    true
}

fn npc_stunrun_gotattack(gs: &mut GameState, cn: usize, co: usize) -> bool {
    npc_stunrun_add_seen(gs, cn, co);
    gs.characters[cn].data[20] = co as i32;
    true
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

fn npc_stunrun_seeattack(gs: &mut GameState, cn: usize, cc: usize, co: usize) -> bool {
    // TODO: Double check this implementation now...
    if gs.do_char_can_see(cn, co) != 0 {
        npc_stunrun_add_seen(gs, cn, co);
        npc_stunrun_add_fight(gs, cn, co);
    }
    if gs.do_char_can_see(cn, cc) != 0 {
        npc_stunrun_add_seen(gs, cn, cc);
        npc_stunrun_add_fight(gs, cn, cc);
    }
    true
}

fn npc_stunrun_see(gs: &mut GameState, cn: usize, co: usize) -> bool {
    if gs.do_char_can_see(cn, co) == 0 {
        return true; // processed it: we cannot see him, so ignore him
    }

    npc_stunrun_add_seen(gs, cn, co);
    true
}

/// Handles incoming messages for the stunrun NPC driver.
///
/// # Arguments
///
/// * `gs` - Active game state containing NPC and target state.
/// * `cn` - NPC character index receiving the message.
/// * `msg_type` - Message type constant.
/// * `dat1` - First message payload value.
/// * `dat2` - Second message payload value.
/// * `_dat3` - Third message payload value, currently unused.
/// * `_dat4` - Fourth message payload value, currently unused.
///
/// # Returns
///
/// * `true` when the message was handled as actionable state, otherwise `false`.
///
/// # Panics
///
/// * Panics if `cn` or a message payload interpreted as a character index is invalid.
pub fn npc_stunrun_msg(
    gs: &mut GameState,
    cn: usize,
    msg_type: u8,
    dat1: i32,
    dat2: i32,
    _dat3: i32,
    _dat4: i32,
) -> bool {
    match msg_type {
        NT_GOTHIT => npc_stunrun_gotattack(gs, cn, dat1 as usize),
        NT_GOTMISS => npc_stunrun_gotattack(gs, cn, dat1 as usize),
        NT_DIDHIT => false,
        NT_DIDMISS => false,
        NT_DIDKILL => false,
        NT_GOTEXP => false,
        NT_SEEKILL => false,
        NT_SEEHIT => npc_stunrun_seeattack(gs, cn, dat1 as usize, dat2 as usize),
        NT_SEEMISS => npc_stunrun_seeattack(gs, cn, dat1 as usize, dat2 as usize),
        NT_GIVE => false,
        NT_SEE => npc_stunrun_see(gs, cn, dat1 as usize),
        NT_DIED => false,
        NT_SHOUT => false,
        NT_HITME => false,
        _ => {
            let name = gs.characters[cn].get_name().to_owned();
            log::warn!("Unknown NPC message for {} ({}): {}", cn, name, msg_type);
            false
        }
    }
}

/// Runs the high-priority tick for a city-attack NPC.
///
/// # Arguments
///
/// * `gs` - Active game state containing NPC combat state and skills.
/// * `cn` - NPC character index.
///
/// # Returns
///
/// * `true` when the NPC spent the tick casting or acting, otherwise `false`.
///
/// # Panics
///
/// * Panics if `cn` or its current attack target index is invalid.
pub fn npc_cityattack_high(gs: &mut GameState, cn: usize) -> bool {
    let low_hp = gs.characters[cn].a_hp < i32::from(gs.characters[cn].hp[5]) * 600;
    if low_hp && driver::npc_try_spell(gs, cn, cn, skills::SK_HEAL) {
        return true;
    }

    let high_mana = gs.characters[cn].a_mana > (i32::from(gs.characters[cn].mana[5]) * 850);
    let has_medit = gs.characters[cn].skill[skills::SK_MEDIT][0] != 0;
    if high_mana && has_medit {
        let very_high_mana = gs.characters[cn].a_mana > 75000;
        if very_high_mana && driver::npc_try_spell(gs, cn, cn, skills::SK_BLESS) {
            return true;
        }
        if driver::npc_try_spell(gs, cn, cn, skills::SK_PROTECT) {
            return true;
        }
        if driver::npc_try_spell(gs, cn, cn, skills::SK_MSHIELD) {
            return true;
        }
        if driver::npc_try_spell(gs, cn, cn, skills::SK_ENHANCE) {
            return true;
        }
        if driver::npc_try_spell(gs, cn, cn, skills::SK_BLESS) {
            return true;
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
        let losing = gs.characters[cn].a_hp < i32::from(gs.characters[cn].hp[5]) * 600;
        if losing && driver::npc_try_spell(gs, cn, co as usize, skills::SK_BLAST) {
            return true;
        }

        let ticker = gs.globals.ticker;
        let data_75 = gs.characters[cn].data[75];
        if ticker > data_75 && driver::npc_try_spell(gs, cn, co as usize, skills::SK_STUN) {
            gs.characters[cn].data[75] =
                ticker + i32::from(gs.characters[cn].skill[skills::SK_STUN][5]) + TICKS * 8;
            return true;
        }

        let very_high_mana = gs.characters[cn].a_mana > 75000;
        if very_high_mana && driver::npc_try_spell(gs, cn, cn, skills::SK_BLESS) {
            return true;
        }
        if driver::npc_try_spell(gs, cn, cn, skills::SK_PROTECT) {
            return true;
        }
        if driver::npc_try_spell(gs, cn, cn, skills::SK_MSHIELD) {
            return true;
        }
        if driver::npc_try_spell(gs, cn, cn, skills::SK_ENHANCE) {
            return true;
        }
        if driver::npc_try_spell(gs, cn, cn, skills::SK_BLESS) {
            return true;
        }
        if driver::npc_try_spell(gs, cn, co as usize, skills::SK_CURSE) {
            return true;
        }

        let data_74 = gs.characters[cn].data[74];
        if ticker > data_74 + TICKS * 10
            && driver::npc_try_spell(gs, cn, co as usize, skills::SK_GHOST)
        {
            gs.characters[cn].data[74] = ticker;
            return true;
        }

        let cannot_hurt = gs.characters[co as usize].armor + 5 > gs.characters[cn].weapon;
        if cannot_hurt && driver::npc_try_spell(gs, cn, co as usize, skills::SK_BLAST) {
            return true;
        }
    }

    false
}

/// Attempts to move an NPC toward a target coordinate.
///
/// # Arguments
///
/// * `gs` - Active game state containing NPC movement state and map collision checks.
/// * `cn` - NPC character index.
/// * `x` - Target x coordinate.
/// * `y` - Target y coordinate.
///
/// # Returns
///
/// * `true` when the NPC is close enough to the target, otherwise `false`.
///
/// # Panics
///
/// * Panics if `cn` is not a valid character index.
pub fn npc_moveto(gs: &mut GameState, cn: usize, x: u16, y: u16) -> bool {
    let (cn_x, cn_y) = (gs.characters[cn].x, gs.characters[cn].y);

    if (i32::from(cn_x) - i32::from(x)).abs() < 3 && (i32::from(cn_y) - i32::from(y)).abs() < 3 {
        gs.characters[cn].data[1] = 0;
        return true;
    }

    let data_1 = gs.characters[cn].data[1];
    if data_1 == 0 && driver::npc_check_target(gs, x as usize, y as usize) {
        gs.characters[cn].data[1] += 1;
        gs.characters[cn].goto_x = x;
        gs.characters[cn].goto_y = y;
        return false;
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
                return false;
            }
            if data_1 < try_count
                && x >= dx
                && driver::npc_check_target(gs, (x - dx) as usize, (y + dy) as usize)
            {
                gs.characters[cn].data[1] += 1;
                gs.characters[cn].goto_x = x - dx;
                gs.characters[cn].goto_y = y + dy;
                return false;
            }
            if data_1 < try_count
                && y >= dy
                && driver::npc_check_target(gs, (x + dx) as usize, (y - dy) as usize)
            {
                gs.characters[cn].data[1] += 1;
                gs.characters[cn].goto_x = x + dx;
                gs.characters[cn].goto_y = y - dy;
                return false;
            }
            if data_1 < try_count
                && x >= dx
                && y >= dy
                && driver::npc_check_target(gs, (x - dx) as usize, (y - dy) as usize)
            {
                gs.characters[cn].data[1] += 1;
                gs.characters[cn].goto_x = x - dx;
                gs.characters[cn].goto_y = y - dy;
                return false;
            }
        }
    }

    gs.characters[cn].data[1] = 0;
    false
}

fn npc_cityattack_wait(gs: &GameState) -> bool {
    let mdtime = gs.globals.mdtime;
    (mdtime % 28800) < 20
}

/// Runs the low-priority waypoint state machine for a city-attack NPC.
///
/// # Arguments
///
/// * `gs` - Active game state containing NPC waypoint state.
/// * `cn` - NPC character index.
///
/// # Returns
///
/// * Always `false` after advancing state as needed.
///
/// # Panics
///
/// * Panics if `cn` is not a valid character index.
pub fn npc_cityattack_low(gs: &mut GameState, cn: usize) -> bool {
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
        _ => false,
    };

    if ret {
        gs.characters[cn].data[0] += 1;
    }

    false
}

fn npc_cityattack_gotattack(_cn: usize, _co: usize) -> bool {
    true
}

fn npc_cityattack_seeattack(gs: &mut GameState, cn: usize, cc: usize, co: usize) -> bool {
    gs.do_char_can_see(cn, co);
    gs.do_char_can_see(cn, cc);
    true
}

fn npc_cityattack_see(gs: &mut GameState, cn: usize, co: usize) -> bool {
    if gs.do_char_can_see(cn, co) == 0 {
        return true;
    }

    let cn_team = gs.characters[cn].data[42];
    let co_team = gs.characters[co].data[42];
    if cn_team != co_team {
        let cc = gs.characters[cn].attack_cn as usize;
        if cc == 0
            || crate::driver::npc_dist(&gs.characters[cn], &gs.characters[co])
                < crate::driver::npc_dist(&gs.characters[cn], &gs.characters[cc])
        {
            gs.characters[cn].attack_cn = co as u16;
            gs.characters[cn].goto_x = 0;
        }
    }

    true
}

/// Handles incoming messages for the city-attack NPC driver.
///
/// # Arguments
///
/// * `gs` - Active game state containing NPC and target state.
/// * `cn` - NPC character index receiving the message.
/// * `msg_type` - Message type constant.
/// * `dat1` - First message payload value.
/// * `dat2` - Second message payload value.
/// * `_dat3` - Third message payload value, currently unused.
/// * `_dat4` - Fourth message payload value, currently unused.
///
/// # Returns
///
/// * `true` when the message was handled as actionable state, otherwise `false`.
///
/// # Panics
///
/// * Panics if `cn` or a message payload interpreted as a character index is invalid.
pub fn npc_cityattack_msg(
    gs: &mut GameState,
    cn: usize,
    msg_type: i32,
    dat1: i32,
    dat2: i32,
    _dat3: i32,
    _dat4: i32,
) -> bool {
    match msg_type {
        x if x == i32::from(NT_GOTHIT) => npc_cityattack_gotattack(cn, dat1 as usize),
        x if x == i32::from(NT_GOTMISS) => npc_cityattack_gotattack(cn, dat1 as usize),
        x if x == i32::from(NT_DIDHIT) => false,
        x if x == i32::from(NT_DIDMISS) => false,
        x if x == i32::from(NT_DIDKILL) => false,
        x if x == i32::from(NT_GOTEXP) => false,
        x if x == i32::from(NT_SEEKILL) => false,
        x if x == i32::from(NT_SEEHIT) => {
            npc_cityattack_seeattack(gs, cn, dat1 as usize, dat2 as usize)
        }
        x if x == i32::from(NT_SEEMISS) => {
            npc_cityattack_seeattack(gs, cn, dat1 as usize, dat2 as usize)
        }
        x if x == i32::from(NT_GIVE) => false,
        x if x == i32::from(NT_SEE) => npc_cityattack_see(gs, cn, dat1 as usize),
        x if x == i32::from(NT_DIED) => false,
        x if x == i32::from(NT_SHOUT) => false,
        x if x == i32::from(NT_HITME) => false,
        _ => {
            let name = gs.characters[cn].get_name().to_owned();
            log::warn!("Unknown NPC message for {} ({}): {}", cn, name, msg_type);
            false
        }
    }
}

/// Runs the high-priority tick for Zoetje's NPC driver (special driver 4).
///
/// # Arguments
///
/// * `_gs` - Active game state used by this function.
/// * `_cn` - Zoetje character index.
///
/// # Returns
///
/// * Always `false`; Zoetje has no high-priority actions.
pub fn npc_zoetje_high(_gs: &mut GameState, _cn: usize) -> bool {
    false
}

/// Runs the low-priority tick for Zoetje's NPC driver (special driver 4).
///
/// Tutorial progress is driven entirely by player notifications, so the only
/// idle behaviour is keeping her facing her configured resting direction.
///
/// # Arguments
///
/// * `gs` - Active game state used by this function.
/// * `cn` - Zoetje character index.
///
/// # Returns
///
/// * `true` when a turn toward the resting direction was queued.
pub fn npc_zoetje_low(gs: &mut GameState, cn: usize) -> bool {
    npc_zoetje_face_resting_direction(gs, cn)
}

/// Queues a turn so Zoetje faces the resting direction from `data[30]`.
///
/// `pop_create_char` forces every spawned NPC to `DX_DOWN`, and the generic
/// low-priority driver (which normally applies `data[30]`) is bypassed by
/// special drivers, so Zoetje has to apply it herself.
///
/// # Arguments
///
/// * `gs` - Active game state used by this function.
/// * `cn` - Zoetje character index.
///
/// # Returns
///
/// * `true` when a `DR_TURN` action was queued, otherwise `false`.
fn npc_zoetje_face_resting_direction(gs: &mut GameState, cn: usize) -> bool {
    let wanted = gs.characters[cn].data[30];
    if wanted == 0 || i32::from(gs.characters[cn].dir) == wanted {
        return false;
    }

    let (dx, dy) = match u8::try_from(wanted) {
        Ok(DX_UP) => (0, -1),
        Ok(DX_DOWN) => (0, 1),
        Ok(DX_LEFT) => (-1, 0),
        Ok(DX_RIGHT) => (1, 0),
        Ok(DX_LEFTUP) => (-1, -1),
        Ok(DX_LEFTDOWN) => (-1, 1),
        Ok(DX_RIGHTUP) => (1, -1),
        Ok(DX_RIGHTDOWN) => (1, 1),
        _ => return false,
    };

    let target_x = i32::from(gs.characters[cn].x) + dx;
    let target_y = i32::from(gs.characters[cn].y) + dy;
    if !(0..SERVER_MAPX).contains(&target_x) || !(0..SERVER_MAPY).contains(&target_y) {
        return false;
    }

    gs.characters[cn].misc_action = DR_TURN as u16;
    gs.characters[cn].misc_target1 = target_x as u16;
    gs.characters[cn].misc_target2 = target_y as u16;
    true
}

/// Character field used to persist per-player Zoetje tutorial progress.
const ZOETJE_TUTORIAL_STEP_IDX: usize = 5;
/// Character field used to rate-limit each player's tutorial messages.
const ZOETJE_TUTORIAL_NEXT_TICK_IDX: usize = 6;
/// Minimum interval between consecutive tutorial messages.
const ZOETJE_TUTORIAL_MESSAGE_INTERVAL: i32 = TICKS * 5;

/// Builds one personalized line of Zoetje's tutorial dialogue.
///
/// # Arguments
///
/// * `step` - Tutorial dialogue step to produce.
/// * `player_name` - Name of the player receiving the tutorial.
/// * `class` - Player's character class, used to select the suggested weapon.
///
/// # Returns
///
/// * The dialogue for the requested step, or `None` for a wait/completed step.
fn zoetje_tutorial_dialogue(step: i32, player_name: &str, class: Class) -> Option<String> {
    let weapon = match class {
        Class::Harakim => "Harakim Dagger",
        Class::Templar => "Templar Two-Handed Blade",
        _ => "Mercenary Sword",
    };

    match step {
        0 => Some(format!(
            "{}, welcome to the Temple of Rebirth. You've come back to our lands- a familiar face, reborn as a stranger.",
            player_name
        )),
        1 => Some("• Templar\n• Mercenary\n• Harakim".to_owned()),
        2 => Some(format!(
            "{}, rebirth can be confusing. Are you able to see fully with your new eyes? Please, hold the CTRL key and right-click me to look at me.",
            player_name
        )),
        4 => Some(format!(
            "You've opened the door, and set yourself on this mortal plain once more. But be ready for the dangers ahead of you- please take the armor and a {}. Open your inventory, and equip for battle.",
            weapon
        )),
        5 => Some(
            "To do this once the item is in your inventory, hold shift + left-click to grab the item, then place it in the appropriate slot and press left-click again to set it."
                .to_owned(),
        ),
        6 => Some(
            "Now you're equipped for battle, and moving about! I've given you a flask. Please, take all the flowers from my garden you would like. You can use them with the flask to make a potion."
                .to_owned(),
        ),
        7 => Some("Hold Shift + Left-Click to gather flowers from the garden.".to_owned()),
        8 => Some(
            "In the next room, there is a portal to take you from my lands, and an enemy to practice your attacks. You must remember how to fight! Hold Ctrl + Left-Click to attack."
                .to_owned(),
        ),
        _ => None,
    }
}

/// Sends the next tutorial line privately and advances that player's state.
///
/// # Arguments
///
/// * `gs` - Active game state containing Zoetje and the player.
/// * `cn` - Zoetje's character index.
/// * `player` - Character index receiving the tutorial.
///
/// # Returns
///
/// * `true` when a dialogue line was sent; otherwise `false`.
fn npc_zoetje_advance_tutorial(gs: &mut GameState, cn: usize, player: usize) -> bool {
    let step = gs.characters[player].future3[ZOETJE_TUTORIAL_STEP_IDX];
    let name = gs.characters[player].get_name().to_owned();
    let class = Class::from(gs.characters[player].kindred);
    let Some(message) = zoetje_tutorial_dialogue(step, &name, class) else {
        return false;
    };

    npc_zoetje_tell_player(gs, cn, player, &message);

    let next_step = match step {
        2 => 3, // Wait until the player manually looks at Zoetje.
        8 => 9, // Tutorial complete.
        _ => step + 1,
    };
    gs.characters[player].future3[ZOETJE_TUTORIAL_STEP_IDX] = next_step;
    gs.characters[player].future3[ZOETJE_TUTORIAL_NEXT_TICK_IDX] =
        if next_step == 3 || next_step == 9 {
            0
        } else {
            gs.globals
                .ticker
                .saturating_add(ZOETJE_TUTORIAL_MESSAGE_INTERVAL)
        };
    true
}

/// Sends tutorial text via Zoetje's private tell, splitting only if the tell
/// command's 200-character display limit would otherwise truncate the text.
///
/// # Arguments
///
/// * `gs` - Active game state containing Zoetje and the recipient.
/// * `cn` - Zoetje's character index, used as the tell sender.
/// * `player` - Character index receiving the tutorial.
/// * `message` - Tutorial line to deliver.
fn npc_zoetje_tell_player(gs: &mut GameState, cn: usize, player: usize, message: &str) {
    let player_name = gs.characters[player].get_name().to_owned();
    for part in zoetje_tell_parts(message) {
        gs.do_tell(cn, &player_name, &part);
    }
}

/// Splits a long private tell into bounded chunks, preferring a sentence break.
///
/// # Arguments
///
/// * `message` - Full tutorial message to preserve.
///
/// # Returns
///
/// * One or more strings, each no longer than the tell display limit.
fn zoetje_tell_parts(message: &str) -> Vec<String> {
    const TELL_MAX_CHARS: usize = 200;
    if message.chars().count() <= TELL_MAX_CHARS {
        return vec![message.to_owned()];
    }

    if let Some((first_sentence, remainder)) = message.split_once(". ") {
        let first_sentence = format!("{first_sentence}.");
        if first_sentence.chars().count() <= TELL_MAX_CHARS
            && remainder.chars().count() <= TELL_MAX_CHARS
        {
            return vec![first_sentence, remainder.to_owned()];
        }
    }

    let chars: Vec<char> = message.chars().collect();
    chars
        .chunks(TELL_MAX_CHARS)
        .map(|chunk| chunk.iter().collect())
        .collect()
}

/// Handles nearby-player and manual-look notifications for Zoetje's tutorial.
///
/// # Arguments
///
/// * `gs` - Active game state containing Zoetje and nearby players.
/// * `cn` - Zoetje character index receiving the message.
/// * `msg_type` - Message type constant.
/// * `dat1` - First message payload value, normally the nearby/looked-at character.
/// * `_dat2` - Second message payload value.
/// * `_dat3` - Third message payload value.
/// * `_dat4` - Fourth message payload value.
///
/// # Returns
///
/// * `true` when a tutorial notification was handled, otherwise `false`.
pub fn npc_zoetje_msg(
    gs: &mut GameState,
    cn: usize,
    msg_type: i32,
    dat1: i32,
    _dat2: i32,
    _dat3: i32,
    _dat4: i32,
) -> bool {
    let player = dat1 as usize;
    if player == 0
        || player >= MAXCHARS
        || gs.characters[player].used != USE_ACTIVE
        || gs.characters[player].flags & CharacterFlags::Player.bits() == 0
    {
        return false;
    }

    if msg_type == i32::from(NT_LOOK) {
        if gs.characters[player].future3[ZOETJE_TUTORIAL_STEP_IDX] != 3 {
            return false;
        }
        gs.characters[player].future3[ZOETJE_TUTORIAL_STEP_IDX] = 4;
        gs.characters[player].future3[ZOETJE_TUTORIAL_NEXT_TICK_IDX] = gs.globals.ticker;
        return npc_zoetje_advance_tutorial(gs, cn, player);
    }

    if msg_type != i32::from(NT_SEE) {
        return false;
    }

    let step = gs.characters[player].future3[ZOETJE_TUTORIAL_STEP_IDX];
    if step == 0 {
        return npc_zoetje_advance_tutorial(gs, cn, player);
    }
    if matches!(step, 3 | 9)
        || gs.globals.ticker < gs.characters[player].future3[ZOETJE_TUTORIAL_NEXT_TICK_IDX]
    {
        return false;
    }

    npc_zoetje_advance_tutorial(gs, cn, player)
}

/// Returns the high-priority result for Malte's special NPC driver.
///
/// # Arguments
///
/// * `_gs` - Active game state, unused by this legacy hook.
/// * `_character_id` - NPC character index, unused by this legacy hook.
///
/// # Returns
///
/// * Always `false`.
pub fn npc_malte_high(_gs: &mut GameState, _character_id: usize) -> bool {
    false
}

/// Runs the low-priority scripted state machine for Malte's NPC driver.
///
/// # Arguments
///
/// * `gs` - Active game state containing Malte's script state.
/// * `cn` - Malte character index.
///
/// # Returns
///
/// * `true` when the script consumed the tick, otherwise `false`.
///
/// # Panics
///
/// * Panics if `cn` or the stored target character index is invalid.
pub fn npc_malte_low(gs: &mut GameState, cn: usize) -> bool {
    let ticker = gs.globals.ticker;
    let data_2 = gs.characters[cn].data[2];
    if ticker < data_2 {
        return false;
    }

    let co = gs.characters[cn].data[0] as usize;
    let state = gs.characters[cn].data[1];

    match state {
        0 => {
            let co_name = gs.characters[co].get_name().to_owned();
            let message = format!("Thank you so much for saving me, {}!", co_name);
            gs.do_sayx(cn, &message);

            gs.characters[cn].data[2] = ticker + TICKS * 8;
            gs.characters[cn].data[1] += 1;
            gs.characters[cn].misc_action = DR_TURN as u16;
            gs.characters[cn].misc_target1 = u16::from(DX_DOWN);
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
            EffectManager::fx_add_effect(gs, 7, 0, i32::from(cn_x), i32::from(cn_y), 0);

            gs.characters[cn].data[2] = ticker + TICKS * 6;
            gs.characters[cn].data[1] += 1;
        }
        6 => {
            gs.do_sayx(cn, "Good luck my friend. And thank you for freeing me!");

            player::map::plr_map_remove(gs, cn);
            God::destroy_items(gs, cn);
            gs.characters[cn].used = USE_EMPTY;
        }
        _ => {}
    }

    false
}

fn npc_malte_gotattack(gs: &mut GameState, cn: usize, co: usize) -> bool {
    let cn_team = gs.characters[cn].data[42];
    let co_team = gs.characters[co].data[42];

    if cn_team != co_team {
        let cc = gs.characters[cn].attack_cn as usize;
        if cc == 0
            || crate::driver::npc_dist(&gs.characters[cn], &gs.characters[co])
                < crate::driver::npc_dist(&gs.characters[cn], &gs.characters[cc])
        {
            gs.characters[cn].attack_cn = co as u16;
            gs.characters[cn].goto_x = 0;
        }
    }

    true
}

/// Handles incoming messages for Malte's NPC driver.
///
/// # Arguments
///
/// * `gs` - Active game state containing NPC and target state.
/// * `cn` - Malte character index receiving the message.
/// * `msg_type` - Message type constant.
/// * `dat1` - First message payload value.
/// * `_dat2` - Second message payload value, currently unused.
/// * `_dat3` - Third message payload value, currently unused.
/// * `_dat4` - Fourth message payload value, currently unused.
///
/// # Returns
///
/// * `true` when the message was handled as actionable state, otherwise `false`.
///
/// # Panics
///
/// * Panics if `cn` or a message payload interpreted as a character index is invalid.
pub fn npc_malte_msg(
    gs: &mut GameState,
    cn: usize,
    msg_type: i32,
    dat1: i32,
    _dat2: i32,
    _dat3: i32,
    _dat4: i32,
) -> bool {
    match msg_type {
        x if x == i32::from(NT_GOTHIT) => npc_malte_gotattack(gs, cn, dat1 as usize),
        x if x == i32::from(NT_GOTMISS) => npc_malte_gotattack(gs, cn, dat1 as usize),
        x if x == i32::from(NT_DIDHIT) => false,
        x if x == i32::from(NT_DIDMISS) => false,
        x if x == i32::from(NT_DIDKILL) => false,
        x if x == i32::from(NT_GOTEXP) => false,
        x if x == i32::from(NT_SEEKILL) => false,
        x if x == i32::from(NT_SEEHIT) => false,
        x if x == i32::from(NT_SEEMISS) => false,
        x if x == i32::from(NT_GIVE) => false,
        x if x == i32::from(NT_SEE) => false,
        x if x == i32::from(NT_DIED) => false,
        x if x == i32::from(NT_SHOUT) => false,
        x if x == i32::from(NT_HITME) => false,
        _ => {
            let name = gs.characters[cn].get_name().to_owned();
            log::warn!("Unknown NPC message for {} ({}): {}", cn, name, msg_type);
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{npc_zoetje_low, zoetje_tell_parts, zoetje_tutorial_dialogue};
    use crate::test_helpers::with_test_gs;
    use core::constants::{DR_IDLE, DR_TURN, DX_DOWN, DX_UP, USE_ACTIVE};
    use core::traits::Class;

    #[test]
    fn zoetje_tutorial_dialogue_personalizes_greeting_and_class_weapon() {
        assert_eq!(
            zoetje_tutorial_dialogue(0, "Ada", Class::Templar).as_deref(),
            Some(
                "Ada, welcome to the Temple of Rebirth. You've come back to our lands- a familiar face, reborn as a stranger."
            )
        );
        assert!(
            zoetje_tutorial_dialogue(4, "Ada", Class::Harakim)
                .unwrap()
                .contains("Harakim Dagger")
        );
        assert!(
            zoetje_tutorial_dialogue(4, "Ada", Class::Mercenary)
                .unwrap()
                .contains("Mercenary Sword")
        );
        assert!(
            zoetje_tutorial_dialogue(4, "Ada", Class::Templar)
                .unwrap()
                .contains("Templar Two-Handed Blade")
        );
    }

    #[test]
    fn zoetje_tutorial_dialogue_has_no_line_while_waiting_or_after_completion() {
        assert_eq!(zoetje_tutorial_dialogue(3, "Ada", Class::Mercenary), None);
        assert_eq!(zoetje_tutorial_dialogue(9, "Ada", Class::Mercenary), None);
    }

    #[test]
    fn zoetje_tell_parts_preserve_long_messages_without_exceeding_tell_limit() {
        let message = zoetje_tutorial_dialogue(4, "Ada", Class::Templar).unwrap();
        let parts = zoetje_tell_parts(&message);

        assert_eq!(parts.len(), 2);
        assert!(parts.iter().all(|part| part.chars().count() <= 200));
        assert_eq!(format!("{} {}", parts[0], parts[1]), message);
    }

    #[test]
    fn zoetje_low_turns_toward_resting_direction_then_stops() {
        with_test_gs(|gs| {
            let cn = 1;
            gs.characters[cn] = core::types::Character::default();
            gs.characters[cn].used = USE_ACTIVE;
            gs.characters[cn].x = 484;
            gs.characters[cn].y = 128;
            gs.characters[cn].dir = DX_DOWN;
            gs.characters[cn].misc_action = DR_IDLE as u16;
            gs.characters[cn].data[30] = i32::from(DX_UP);

            assert!(npc_zoetje_low(gs, cn));
            assert_eq!(gs.characters[cn].misc_action, DR_TURN as u16);
            assert_eq!(gs.characters[cn].misc_target1, 484);
            assert_eq!(gs.characters[cn].misc_target2, 127);

            gs.characters[cn].dir = DX_UP;
            gs.characters[cn].misc_action = DR_IDLE as u16;
            assert!(!npc_zoetje_low(gs, cn));
            assert_eq!(gs.characters[cn].misc_action, DR_IDLE as u16);
        });
    }

    #[test]
    fn zoetje_low_does_nothing_without_a_resting_direction() {
        with_test_gs(|gs| {
            let cn = 1;
            gs.characters[cn] = core::types::Character::default();
            gs.characters[cn].used = USE_ACTIVE;
            gs.characters[cn].x = 484;
            gs.characters[cn].y = 128;
            gs.characters[cn].dir = DX_DOWN;
            gs.characters[cn].misc_action = DR_IDLE as u16;
            gs.characters[cn].data[30] = 0;

            assert!(!npc_zoetje_low(gs, cn));
            assert_eq!(gs.characters[cn].misc_action, DR_IDLE as u16);
        });
    }
}
