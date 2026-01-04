use crate::effect::EffectManager;
use crate::god::God;
use crate::repository::Repository;
use crate::state::State;
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
pub fn npc_stunrun_high(cn: usize) -> i32 {
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
    let mut flee = 0; // should we flee?
    let mut help = 0; // should we help someone?
    let mut stun = 0; // should we stun someone?
    let mut up = 0;
    let mut down = 0;
    let mut left = 0;
    let mut right = 0; // directions to move in
    let mut done = false;

    Repository::with_characters_mut(|ch| ch[cn].data[92] = TICKS * 60);

    // Scan data slots 0-19 for nearby characters
    for n in 0..20 {
        let co = Repository::with_characters(|ch| ch[cn].data[n] as usize);
        if co != 0 && Character::is_sane_character(co) {
            let co_team = Repository::with_characters(|ch| ch[co].data[42]);
            let cn_team = Repository::with_characters(|ch| ch[cn].data[42]);

            if co_team == cn_team {
                // Friendly character
                seen[maxseen].co = co;
                seen[maxseen].dist = driver::npc_dist(cn, co);
                seen[maxseen].is_friend = true;
                seen[maxseen].stun = 0;
                let low_hp =
                    Repository::with_characters(|ch| ch[co].a_hp < (ch[co].hp[5] as i32 * 400));
                seen[maxseen].help = if low_hp { 1 } else { 0 };
                help = help.max(seen[maxseen].help);
                maxseen += 1;
            } else {
                // Enemy character
                seen[maxseen].co = co;
                seen[maxseen].dist = driver::npc_dist(cn, co);
                seen[maxseen].is_friend = false;
                if !driver::npc_is_stunned(co) {
                    let can_stun = Repository::with_characters(|ch| {
                        ch[cn].skill[SK_STUN][5] * 12 > ch[co].skill[SK_RESIST][5] * 10
                    });
                    seen[maxseen].stun = if can_stun { 1 } else { 0 };
                } else {
                    seen[maxseen].stun = 0;
                }
                stun = stun.max(seen[maxseen].stun);
                seen[maxseen].help = 0;
                if seen[maxseen].dist < 6 {
                    flee += 1; // we don't like infights, try to stay away from enemies
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

    // Scan data slots 30-34 for characters that got hit
    for n in 30..35 {
        let co = Repository::with_characters(|ch| ch[cn].data[n] as usize);
        if co != 0 && Character::is_sane_character(co) {
            for m in 0..maxseen {
                if seen[m].co == co {
                    let co_team = Repository::with_characters(|ch| ch[co].data[42]);
                    let cn_team = Repository::with_characters(|ch| ch[cn].data[42]);

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

    // Check who attacked us (data slot 20)
    let co = Repository::with_characters(|ch| ch[cn].data[20] as usize);
    if co != 0 && Character::is_sane_character(co) {
        flee += 5; // we don't like infights, try to flee if attacked
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
            // Not in seen list, add it
            seen[maxseen].co = co;
            seen[maxseen].dist = driver::npc_dist(cn, co);
            seen[maxseen].is_friend = false;
            let can_stun = Repository::with_characters(|ch| {
                ch[cn].skill[SK_STUN][5] * 12 > ch[co].skill[SK_RESIST][5] * 10
            });
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

    // Adjust priorities based on mana
    let low_mana = Repository::with_characters(|ch| ch[cn].a_mana < (ch[cn].mana[5] as i32 * 125));
    if low_mana {
        stun -= 3;
        help -= 3;
        flee += 1;
    }

    // Reset former orders
    Repository::with_characters_mut(|ch| {
        ch[cn].use_nr = 0;
        ch[cn].skill_nr = 0;
        ch[cn].attack_cn = 0;
        ch[cn].goto_x = 0;
        ch[cn].goto_y = 0;
        ch[cn].misc_action = 0;
        ch[cn].cerrno = 0;
    });

    // If low HP, increase flee priority
    let low_hp = Repository::with_characters(|ch| ch[cn].a_hp < (ch[cn].hp[5] as i32 * 666));
    if low_hp {
        flee += 5;
    }

    // Try to heal self if low HP
    if !done && low_hp {
        done = driver::npc_try_spell(cn, cn, SK_HEAL);
    }

    // Set movement mode based on endurance
    let high_endurance = Repository::with_characters(|ch| ch[cn].a_end > 15000);
    Repository::with_characters_mut(|ch| {
        ch[cn].mode = if high_endurance { 1 } else { 0 };
    });

    // Fleeing behavior
    if !done && flee > 1 && flee >= help && flee >= stun {
        Repository::with_characters_mut(|ch| {
            ch[cn].mode = if ch[cn].a_end > 15000 { 2 } else { 1 };
        });

        // Calculate directional weights based on enemy positions
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
            let (cn_x, cn_y, co_x, co_y) =
                Repository::with_characters(|ch| (ch[cn].x, ch[cn].y, ch[co].x, ch[co].y));

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

        let (cn_x, cn_y) = Repository::with_characters(|ch| (ch[cn].x, ch[cn].y));

        // Check if up is free space
        for n in 1..5 {
            if !driver::npc_check_target(cn_x as usize, (cn_y - n) as usize) {
                up -= 20;
                if !driver::npc_check_target((cn_x + 1) as usize, (cn_y - n) as usize) {
                    up -= 20;
                    if !driver::npc_check_target((cn_x - 1) as usize, (cn_y - n) as usize) {
                        up -= 10000;
                        break;
                    }
                }
            }
        }

        // Check if down is free space
        for n in 1..5 {
            if !driver::npc_check_target(cn_x as usize, (cn_y + n) as usize) {
                down -= 20;
                if !driver::npc_check_target((cn_x + 1) as usize, (cn_y + n) as usize) {
                    down -= 20;
                    if !driver::npc_check_target((cn_x - 1) as usize, (cn_y + n) as usize) {
                        down -= 10000;
                        break;
                    }
                }
            }
        }

        // Check if left is free space
        for n in 1..5 {
            if !driver::npc_check_target((cn_x - n) as usize, cn_y as usize) {
                left -= 20;
                if !driver::npc_check_target((cn_x - n) as usize, (cn_y + 1) as usize) {
                    left -= 20;
                    if !driver::npc_check_target((cn_x - n) as usize, (cn_y - n) as usize) {
                        left -= 10000;
                        break;
                    }
                }
            }
        }

        // Check if right is free space
        for n in 1..5 {
            if !driver::npc_check_target((cn_x + n) as usize, cn_y as usize) {
                right -= 20;
                if !driver::npc_check_target((cn_x + n) as usize, (cn_y + 1) as usize) {
                    right -= 20;
                    if !driver::npc_check_target((cn_x + n) as usize, (cn_y - n) as usize) {
                        right -= 10000;
                        break;
                    }
                }
            }
        }

        // Bonus for current direction
        let dir = Repository::with_characters(|ch| ch[cn].dir);
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

        // Move in the best direction
        if !done && up >= down && up >= left && up >= right {
            if driver::npc_check_target(cn_x as usize, (cn_y - 1) as usize) {
                Repository::with_characters_mut(|ch| {
                    ch[cn].goto_x = cn_x as u16;
                    ch[cn].goto_y = (cn_y - 1) as u16;
                });
                done = true;
            } else if driver::npc_check_target((cn_x + 1) as usize, (cn_y - 1) as usize) {
                Repository::with_characters_mut(|ch| {
                    ch[cn].goto_x = (cn_x + 1) as u16;
                    ch[cn].goto_y = (cn_y - 1) as u16;
                });
                done = true;
            } else if driver::npc_check_target((cn_x - 1) as usize, (cn_y - 1) as usize) {
                Repository::with_characters_mut(|ch| {
                    ch[cn].goto_x = (cn_x - 1) as u16;
                    ch[cn].goto_y = (cn_y - 1) as u16;
                });
                done = true;
            }
        }

        if !done && down >= up && down >= left && down >= right {
            if driver::npc_check_target(cn_x as usize, (cn_y + 1) as usize) {
                Repository::with_characters_mut(|ch| {
                    ch[cn].goto_x = cn_x as u16;
                    ch[cn].goto_y = (cn_y + 1) as u16;
                });
                done = true;
            } else if driver::npc_check_target((cn_x + 1) as usize, (cn_y + 1) as usize) {
                Repository::with_characters_mut(|ch| {
                    ch[cn].goto_x = (cn_x + 1) as u16;
                    ch[cn].goto_y = (cn_y + 1) as u16;
                });
                done = true;
            } else if driver::npc_check_target((cn_x - 1) as usize, (cn_y + 1) as usize) {
                Repository::with_characters_mut(|ch| {
                    ch[cn].goto_x = (cn_x - 1) as u16;
                    ch[cn].goto_y = (cn_y + 1) as u16;
                });
                done = true;
            }
        }

        if !done && left >= up && left >= down && left >= right {
            if driver::npc_check_target((cn_x - 1) as usize, cn_y as usize) {
                Repository::with_characters_mut(|ch| {
                    ch[cn].goto_x = (cn_x - 1) as u16;
                    ch[cn].goto_y = cn_y as u16;
                });
                done = true;
            } else if driver::npc_check_target((cn_x - 1) as usize, (cn_y + 1) as usize) {
                Repository::with_characters_mut(|ch| {
                    ch[cn].goto_x = (cn_x - 1) as u16;
                    ch[cn].goto_y = (cn_y + 1) as u16;
                });
                done = true;
            } else if driver::npc_check_target((cn_x - 1) as usize, (cn_y - 1) as usize) {
                Repository::with_characters_mut(|ch| {
                    ch[cn].goto_x = (cn_x - 1) as u16;
                    ch[cn].goto_y = (cn_y - 1) as u16;
                });
                done = true;
            }
        }

        if !done && right >= up && right >= down && right >= left {
            if driver::npc_check_target((cn_x + 1) as usize, cn_y as usize) {
                Repository::with_characters_mut(|ch| {
                    ch[cn].goto_x = (cn_x + 1) as u16;
                    ch[cn].goto_y = cn_y as u16;
                });
                done = true;
            } else if driver::npc_check_target((cn_x + 1) as usize, (cn_y + 1) as usize) {
                Repository::with_characters_mut(|ch| {
                    ch[cn].goto_x = (cn_x + 1) as u16;
                    ch[cn].goto_y = (cn_y + 1) as u16;
                });
                done = true;
            } else if driver::npc_check_target((cn_x + 1) as usize, (cn_y - 1) as usize) {
                Repository::with_characters_mut(|ch| {
                    ch[cn].goto_x = (cn_x + 1) as u16;
                    ch[cn].goto_y = (cn_y - 1) as u16;
                });
                done = true;
            }
        }

        // Panic - attack whoever is attacking us
        if !done {
            let co = Repository::with_characters(|ch| ch[cn].data[20] as usize);
            if co != 0 {
                Repository::with_characters_mut(|ch| ch[cn].attack_cn = co as u16);
                driver::npc_try_spell(cn, co, SK_STUN);
                done = true;
            }
        }
    }

    // Try self-buffs
    if !done {
        done = driver::npc_try_spell(cn, cn, SK_BLESS);
    }
    if !done {
        done = driver::npc_try_spell(cn, cn, SK_MSHIELD);
    }
    if !done {
        done = driver::npc_try_spell(cn, cn, SK_PROTECT);
    }
    if !done {
        done = driver::npc_try_spell(cn, cn, SK_ENHANCE);
    }

    // Stunning behavior
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
            done = driver::npc_try_spell(cn, seen[m].co, SK_STUN);
            if !done {
                done = driver::npc_try_spell(cn, seen[m].co, SK_CURSE);
            }
            let ticker = Repository::with_globals(|g| g.ticker);
            Repository::with_characters_mut(|ch| ch[cn].data[24] = ticker);
        }
    }

    // Helping behavior
    if !done && help > 0 {
        let mut m = 0;
        let mut tmp = 0;
        for n in 0..maxseen {
            if seen[n].help > tmp
                || (seen[n].help != 0 && seen[n].help == tmp && seen[n].dist < seen[m].dist)
            {
                let needs_help = Repository::with_characters(|ch| {
                    !driver::npc_is_blessed(seen[n].co)
                        || ch[seen[n].co].a_hp < (ch[seen[n].co].hp[5] * 400) as i32
                });
                if needs_help {
                    tmp = seen[n].help;
                    m = n;
                }
            }
        }
        if tmp > 0 {
            let low_hp = Repository::with_characters(|ch| {
                ch[seen[m].co].a_hp < (ch[seen[m].co].hp[5] * 400) as i32
            });
            if low_hp {
                done = driver::npc_try_spell(cn, seen[m].co, SK_HEAL);
            }
            if !done {
                done = driver::npc_try_spell(cn, seen[m].co, SK_BLESS);
            }
            if !done {
                done = driver::npc_try_spell(cn, seen[m].co, SK_PROTECT);
            }
            if !done {
                done = driver::npc_try_spell(cn, seen[m].co, SK_ENHANCE);
            }
            let ticker = Repository::with_globals(|g| g.ticker);
            Repository::with_characters_mut(|ch| ch[cn].data[24] = ticker);
        }
    }

    // Patrol state machine
    if !done {
        let state = Repository::with_characters(|ch| ch[cn].data[22]);

        if state == 0 {
            // Staying at home
            let in_item = Repository::with_characters(|ch| ch[cn].citem);
            if in_item != 0 {
                Repository::with_characters_mut(|ch| ch[cn].citem = 0);
                Repository::with_items_mut(|items| items[in_item as usize].used = USE_EMPTY);
            }
            let data_23 = Repository::with_characters(|ch| ch[cn].data[23]);
            if data_23 == 0 {
                let ticker = Repository::with_globals(|g| g.ticker);
                Repository::with_characters_mut(|ch| ch[cn].data[23] = ticker);
            }
            let ticker = Repository::with_globals(|g| g.ticker);
            let data_23 = Repository::with_characters(|ch| ch[cn].data[23]);
            if data_23 + TICKS * 60 * 60 < ticker {
                let mut tmp = 0;
                for y in 322..=332 {
                    if tmp != 0 {
                        break;
                    }
                    for x in 212..=232 {
                        let co = Repository::with_map(|map| map[(x + y * SERVER_MAPX) as usize].ch);
                        if co != 0 {
                            let (co_team, cn_team) = Repository::with_characters(|ch| {
                                (ch[co as usize].data[42], ch[cn].data[42])
                            });
                            if co_team != cn_team {
                                tmp = 1;
                                break;
                            }
                        }
                    }
                }
                if tmp == 0 {
                    Repository::with_characters_mut(|ch| ch[cn].data[22] = 1); // set state for moving towards entry
                }
                Repository::with_characters_mut(|ch| ch[cn].data[23] = ticker);
            }
        }

        let ticker = Repository::with_globals(|g| g.ticker);
        let data_24 = Repository::with_characters(|ch| ch[cn].data[24]);
        if state == 1 && ticker > data_24 + TICKS * 10 {
            // Moving towards entry
            let citem = Repository::with_characters(|ch| ch[cn].citem);
            if citem == 0 {
                let in_item = God::create_item(718);
                // TODO: Check the Some/None case properly here.
                Repository::with_characters_mut(|ch| ch[cn].citem = in_item.unwrap() as u32);
                Repository::with_items_mut(|items| items[in_item.unwrap()].carried = cn as u16);
            }
            let (cn_x, cn_y) = Repository::with_characters(|ch| (ch[cn].x, ch[cn].y));
            if (cn_x as i32 - 264).abs() + (cn_y as i32 - 317).abs() < 20 {
                Repository::with_characters_mut(|ch| {
                    ch[cn].data[22] = 2;
                    ch[cn].data[23] = ticker;
                });
            } else {
                if driver::npc_check_target(264, 317) {
                    Repository::with_characters_mut(|ch| {
                        ch[cn].goto_x = 264;
                        ch[cn].goto_y = 317;
                    });
                } else if driver::npc_check_target(265, 318) {
                    Repository::with_characters_mut(|ch| {
                        ch[cn].goto_x = 265;
                        ch[cn].goto_y = 318;
                    });
                }
                if cn_x > 232 {
                    Repository::with_characters_mut(|ch| ch[cn].data[24] = ticker);
                } else {
                    Repository::with_characters_mut(|ch| ch[cn].data[24] = 0);
                }
            }
        }

        if state == 2 {
            // Moving towards home
            let (cn_x, cn_y) = Repository::with_characters(|ch| (ch[cn].x, ch[cn].y));
            if (cn_x as i32 - 217).abs() + (cn_y as i32 - 349).abs() < 3 {
                let ticker = Repository::with_globals(|g| g.ticker);
                Repository::with_characters_mut(|ch| {
                    ch[cn].data[22] = 0;
                    ch[cn].data[23] = ticker;
                    ch[cn].data[40] += 1;
                });
            } else {
                Repository::with_characters_mut(|ch| {
                    ch[cn].goto_x = 217;
                    ch[cn].goto_y = 349;
                });
            }
        }
    }

    // Clean up old data
    let ticker = Repository::with_globals(|g| g.ticker);
    for n in 0..20 {
        let data_val = Repository::with_characters(|ch| ch[cn].data[n + 50]);
        if data_val + TICKS * 2 < ticker {
            Repository::with_characters_mut(|ch| ch[cn].data[n] = 0); // erase all chars we saw
        }
    }
    for n in 30..35 {
        let data_val = Repository::with_characters(|ch| ch[cn].data[n + 5]);
        if data_val + TICKS * 2 < ticker {
            Repository::with_characters_mut(|ch| ch[cn].data[n] = 0); // erase all fellows that got hit
        }
    }
    let data_21 = Repository::with_characters(|ch| ch[cn].data[21]);
    if data_21 + TICKS * 2 < ticker {
        Repository::with_characters_mut(|ch| ch[cn].data[20] = 0); // forget who hit us
    }

    0
}

pub fn npc_stunrun_low(_cn: usize) -> i32 {
    // Empty function - does nothing in the original C++ implementation
    0
}

fn npc_stunrun_add_seen(cn: usize, co: usize) -> i32 {
    let ticker = Repository::with_globals(|g| g.ticker);

    // Check if co is already in the seen list (data[0-19])
    for n in 0..20 {
        let data_n = Repository::with_characters(|ch| ch[cn].data[n]);
        if data_n == co as i32 {
            Repository::with_characters_mut(|ch| ch[cn].data[n + 50] = ticker);
            return 1;
        }
    }

    // Find an empty slot and add co
    for n in 0..20 {
        let data_n = Repository::with_characters(|ch| ch[cn].data[n]);
        if data_n == 0 {
            Repository::with_characters_mut(|ch| {
                ch[cn].data[n] = co as i32;
                ch[cn].data[n + 50] = ticker;
            });
            break;
        }
    }

    1
}

fn npc_stunrun_gotattack(cn: usize, co: usize) -> i32 {
    npc_stunrun_add_seen(cn, co);
    Repository::with_characters_mut(|ch| ch[cn].data[20] = co as i32);
    1
}

fn npc_stunrun_add_fight(cn: usize, co: usize) {
    let ticker = Repository::with_globals(|g| g.ticker);

    // Check if co is already in the fight list (data[30-34])
    for n in 30..35 {
        let data_n = Repository::with_characters(|ch| ch[cn].data[n]);
        if data_n == co as i32 {
            Repository::with_characters_mut(|ch| ch[cn].data[n + 5] = ticker);
            return;
        }
    }

    // Find an empty slot and add co
    for n in 30..35 {
        let data_n = Repository::with_characters(|ch| ch[cn].data[n]);
        if data_n == 0 {
            Repository::with_characters_mut(|ch| {
                ch[cn].data[n] = co as i32;
                ch[cn].data[n + 5] = ticker;
            });
            break;
        }
    }
}

fn npc_stunrun_seeattack(cn: usize, cc: usize, co: usize) -> i32 {
    // TODO: Double check this implementation now...
    if State::with_mut(|state| state.do_char_can_see(cn, co)) != 0 {
        npc_stunrun_add_seen(cn, co);
        npc_stunrun_add_fight(cn, co);
    }
    if State::with_mut(|state| state.do_char_can_see(cn, cc)) != 0 {
        npc_stunrun_add_seen(cn, cc);
        npc_stunrun_add_fight(cn, cc);
    }
    1
}

fn npc_stunrun_see(cn: usize, co: usize) -> i32 {
    if State::with_mut(|state| state.do_char_can_see(cn, co)) == 0 {
        return 1; // processed it: we cannot see him, so ignore him
    }

    npc_stunrun_add_seen(cn, co);
    1
}

pub fn npc_stunrun_msg(
    cn: usize,
    msg_type: u8,
    dat1: i32,
    dat2: i32,
    _dat3: i32,
    _dat4: i32,
) -> i32 {
    match msg_type {
        NT_GOTHIT => npc_stunrun_gotattack(cn, dat1 as usize),
        NT_GOTMISS => npc_stunrun_gotattack(cn, dat1 as usize),
        NT_DIDHIT => 0,
        NT_DIDMISS => 0,
        NT_DIDKILL => 0,
        NT_GOTEXP => 0,
        NT_SEEKILL => 0,
        NT_SEEHIT => npc_stunrun_seeattack(cn, dat1 as usize, dat2 as usize),
        NT_SEEMISS => npc_stunrun_seeattack(cn, dat1 as usize, dat2 as usize),
        NT_GIVE => 0,
        NT_SEE => npc_stunrun_see(cn, dat1 as usize),
        NT_DIED => 0,
        NT_SHOUT => 0,
        NT_HITME => 0,
        _ => {
            let name =
                Repository::with_characters(|ch| String::from_utf8_lossy(&ch[cn].name).to_string());
            log::warn!("Unknown NPC message for {} ({}): {}", cn, name, msg_type);
            0
        }
    }
}

pub fn npc_cityattack_high(cn: usize) -> i32 {
    // Heal if hurt
    let low_hp = Repository::with_characters(|ch| ch[cn].a_hp < (ch[cn].hp[5] * 600) as i32);
    if low_hp {
        if driver::npc_try_spell(cn, cn, SK_HEAL) {
            return 1;
        }
    }

    // Generic spell management
    let (high_mana, has_medit) = Repository::with_characters(|ch| {
        (
            ch[cn].a_mana > (ch[cn].mana[5] as i32 * 850),
            ch[cn].skill[SK_MEDIT][0] != 0,
        )
    });
    if high_mana && has_medit {
        let very_high_mana = Repository::with_characters(|ch| ch[cn].a_mana > 75000);
        if very_high_mana && driver::npc_try_spell(cn, cn, SK_BLESS) {
            return 1;
        }
        if driver::npc_try_spell(cn, cn, SK_PROTECT) {
            return 1;
        }
        if driver::npc_try_spell(cn, cn, SK_MSHIELD) {
            return 1;
        }
        if driver::npc_try_spell(cn, cn, SK_ENHANCE) {
            return 1;
        }
        if driver::npc_try_spell(cn, cn, SK_BLESS) {
            return 1;
        }
    }

    // Generic endurance management
    let (attack_cn, a_end, current_mode) =
        Repository::with_characters(|ch| (ch[cn].attack_cn, ch[cn].a_end, ch[cn].mode));

    if attack_cn != 0 && a_end > 10000 {
        if current_mode != 2 {
            Repository::with_characters_mut(|ch| {
                ch[cn].mode = 2;
                ch[cn].set_do_update_flags();
            });
        }
    } else if a_end > 10000 {
        if current_mode != 1 {
            Repository::with_characters_mut(|ch| {
                ch[cn].mode = 1;
                ch[cn].set_do_update_flags();
            });
        }
    } else if current_mode != 0 {
        Repository::with_characters_mut(|ch| {
            ch[cn].mode = 0;
            ch[cn].set_do_update_flags();
        });
    }

    // Fight management
    let co = Repository::with_characters(|ch| ch[cn].attack_cn);
    if co != 0 {
        // We're fighting
        let losing = Repository::with_characters(|ch| ch[cn].a_hp < (ch[cn].hp[5] * 600) as i32);
        if losing {
            // We're losing
            if driver::npc_try_spell(cn, co as usize, SK_BLAST) {
                return 1;
            }
        }

        let ticker = Repository::with_globals(|g| g.ticker);
        let data_75 = Repository::with_characters(|ch| ch[cn].data[75]);
        if ticker > data_75 && driver::npc_try_spell(cn, co as usize, SK_STUN) {
            let new_data_75 =
                Repository::with_characters(|ch| ticker + ch[cn].skill[SK_STUN][5] as i32 + 18 * 8);
            Repository::with_characters_mut(|ch| ch[cn].data[75] = new_data_75);
            return 1;
        }

        let very_high_mana = Repository::with_characters(|ch| ch[cn].a_mana > 75000);
        if very_high_mana && driver::npc_try_spell(cn, cn, SK_BLESS) {
            return 1;
        }
        if driver::npc_try_spell(cn, cn, SK_PROTECT) {
            return 1;
        }
        if driver::npc_try_spell(cn, cn, SK_MSHIELD) {
            return 1;
        }
        if driver::npc_try_spell(cn, cn, SK_ENHANCE) {
            return 1;
        }
        if driver::npc_try_spell(cn, cn, SK_BLESS) {
            return 1;
        }
        if driver::npc_try_spell(cn, co as usize, SK_CURSE) {
            return 1;
        }

        let ticker = Repository::with_globals(|g| g.ticker);
        let data_74 = Repository::with_characters(|ch| ch[cn].data[74]);
        if ticker > data_74 + TICKS * 10 && driver::npc_try_spell(cn, co as usize, SK_GHOST) {
            Repository::with_characters_mut(|ch| ch[cn].data[74] = ticker);
            return 1;
        }

        // Blast always if we cannot hurt him otherwise
        let cannot_hurt =
            Repository::with_characters(|ch| ch[co as usize].armor + 5 > ch[cn].weapon);
        if cannot_hurt {
            if driver::npc_try_spell(cn, co as usize, SK_BLAST) {
                return 1;
            }
        }
    }

    0
}

pub fn npc_moveto(cn: usize, x: u16, y: u16) -> i32 {
    let (cn_x, cn_y) = Repository::with_characters(|ch| (ch[cn].x, ch[cn].y));

    // If we're within 3 tiles of the target, we're done
    if (cn_x as i32 - x as i32).abs() < 3 && (cn_y as i32 - y as i32).abs() < 3 {
        Repository::with_characters_mut(|ch| ch[cn].data[1] = 0);
        return 1;
    }

    let data_1 = Repository::with_characters(|ch| ch[cn].data[1]);

    // Try the exact target first
    if data_1 == 0 && driver::npc_check_target(x as usize, y as usize) {
        Repository::with_characters_mut(|ch| {
            ch[cn].data[1] += 1;
            ch[cn].goto_x = x;
            ch[cn].goto_y = y;
        });
        return 0;
    }

    // Try increasingly distant positions around the target
    let mut try_count = 1;
    for dx in 0..3 {
        for dy in 0..3 {
            try_count += 1;
            let data_1 = Repository::with_characters(|ch| ch[cn].data[1]);

            if data_1 < try_count && driver::npc_check_target((x + dx) as usize, (y + dy) as usize)
            {
                Repository::with_characters_mut(|ch| {
                    ch[cn].data[1] += 1;
                    ch[cn].goto_x = x + dx;
                    ch[cn].goto_y = y + dy;
                });
                return 0;
            }
            if data_1 < try_count
                && x >= dx
                && driver::npc_check_target((x - dx) as usize, (y + dy) as usize)
            {
                Repository::with_characters_mut(|ch| {
                    ch[cn].data[1] += 1;
                    ch[cn].goto_x = x - dx;
                    ch[cn].goto_y = y + dy;
                });
                return 0;
            }
            if data_1 < try_count
                && y >= dy
                && driver::npc_check_target((x + dx) as usize, (y - dy) as usize)
            {
                Repository::with_characters_mut(|ch| {
                    ch[cn].data[1] += 1;
                    ch[cn].goto_x = x + dx;
                    ch[cn].goto_y = y - dy;
                });
                return 0;
            }
            if data_1 < try_count
                && x >= dx
                && y >= dy
                && driver::npc_check_target((x - dx) as usize, (y - dy) as usize)
            {
                Repository::with_characters_mut(|ch| {
                    ch[cn].data[1] += 1;
                    ch[cn].goto_x = x - dx;
                    ch[cn].goto_y = y - dy;
                });
                return 0;
            }
        }
    }

    Repository::with_characters_mut(|ch| ch[cn].data[1] = 0);
    0
}

fn npc_cityattack_wait() -> i32 {
    let mdtime = Repository::with_globals(|g| g.mdtime);
    if (mdtime % 28800) < 20 {
        1
    } else {
        0
    }
}

pub fn npc_cityattack_low(cn: usize) -> i32 {
    let state = Repository::with_characters(|ch| ch[cn].data[0]);

    let ret = match state {
        0 => npc_moveto(cn, 456, 356),
        1 => npc_moveto(cn, 447, 356),
        2 => npc_moveto(cn, 447, 362),
        3 => npc_moveto(cn, 474, 362),
        4 => npc_cityattack_wait(),
        5 => npc_moveto(cn, 486, 362),
        6 => npc_moveto(cn, 509, 362),
        7 => npc_moveto(cn, 526, 362),
        8 => npc_moveto(cn, 531, 386),
        9 => npc_moveto(cn, 534, 403),
        _ => 0,
    };

    if ret != 0 {
        Repository::with_characters_mut(|ch| ch[cn].data[0] += 1);
    }

    0
}

fn npc_cityattack_gotattack(_cn: usize, _co: usize) -> i32 {
    1
}

fn npc_cityattack_seeattack(cn: usize, cc: usize, co: usize) -> i32 {
    // Check if cn can see co (does nothing with the result in C++ original)
    State::with_mut(|state| state.do_char_can_see(cn, co));

    // Check if cn can see cc (does nothing with the result in C++ original)
    State::with_mut(|state| state.do_char_can_see(cn, cc));

    1
}

fn npc_cityattack_see(cn: usize, co: usize) -> i32 {
    // Check if cn can see co
    let can_see = State::with_mut(|state| state.do_char_can_see(cn, co));
    if can_see == 0 {
        return 1; // processed it: we cannot see them, so ignore
    }

    // Check if they're on different teams (data[42])
    let (cn_team, co_team) = Repository::with_characters(|ch| (ch[cn].data[42], ch[co].data[42]));

    if cn_team != co_team {
        // Get current attack target
        let cc = Repository::with_characters(|ch| ch[cn].attack_cn as usize);

        // If no current target or co is closer than current target
        if cc == 0 || crate::driver::npc_dist(cn, co) < crate::driver::npc_dist(cn, cc) {
            Repository::with_characters_mut(|ch| {
                ch[cn].attack_cn = co as u16;
                ch[cn].goto_x = 0;
            });
        }
    }

    1
}

pub fn npc_cityattack_msg(
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
        x if x == NT_SEEHIT as i32 => npc_cityattack_seeattack(cn, dat1 as usize, dat2 as usize),
        x if x == NT_SEEMISS as i32 => npc_cityattack_seeattack(cn, dat1 as usize, dat2 as usize),
        x if x == NT_GIVE as i32 => 0,
        x if x == NT_SEE as i32 => npc_cityattack_see(cn, dat1 as usize),
        x if x == NT_DIED as i32 => 0,
        x if x == NT_SHOUT as i32 => 0,
        x if x == NT_HITME as i32 => 0,
        _ => {
            let name =
                Repository::with_characters(|ch| String::from_utf8_lossy(&ch[cn].name).to_string());
            log::warn!("Unknown NPC message for {} ({}): {}", cn, name, msg_type);
            0
        }
    }
}

// This is implemented even though it doesn't seem like it...
pub fn npc_malte_high(_character_id: usize) -> i32 {
    0
}

pub fn npc_malte_low(cn: usize) -> i32 {
    // Check if we need to wait before progressing
    let (ticker, data_2) = Repository::with_globals(|g| {
        let ticker = g.ticker;
        let data_2 = Repository::with_characters(|ch| ch[cn].data[2]);
        (ticker, data_2)
    });

    if ticker < data_2 {
        return 0;
    }

    let co = Repository::with_characters(|ch| ch[cn].data[0] as usize);
    let state = Repository::with_characters(|ch| ch[cn].data[1]);

    match state {
        0 => {
            // Thank the player
            let co_name =
                Repository::with_characters(|ch| String::from_utf8_lossy(&ch[co].name).to_string());
            let message = format!("Thank you so much for saving me, {}!", co_name);
            State::with(|state| state.do_sayx(cn, &message));

            Repository::with_characters_mut(|ch| {
                ch[cn].data[2] = ticker + TICKS * 8;
                ch[cn].data[1] += 1;
                ch[cn].misc_action = DR_TURN as u16;
                ch[cn].misc_target1 = DX_DOWN as u16;
            });
        }
        1 => {
            // Explain about the coin
            State::with(|state| {
                state.do_sayx(
                cn,
                "Before the monsters caught me, I discovered that you need a coin to open certain doors down here."
            )
            });

            Repository::with_characters_mut(|ch| {
                ch[cn].data[2] = ticker + TICKS * 8;
                ch[cn].data[1] += 1;
            });
        }
        2 => {
            // Give coin part and mention Damor
            State::with(|state| {
                state.do_sayx(
                cn,
                "I found this part of the coin, and I heard that Damor in Aston has another one. Ask him for the 'Black Stronghold Coin'."
            )
            });

            // Create and give the coin item
            if let Some(in_item) = crate::god::God::create_item(763) {
                Repository::with_characters_mut(|ch| {
                    ch[cn].citem = in_item as u32;
                    ch[cn].data[2] = ticker + TICKS * 5;
                    ch[cn].data[1] += 1;
                    ch[cn].misc_action = DR_GIVE as u16;
                    ch[cn].misc_target1 = co as u16;
                });
                Repository::with_items_mut(|items| {
                    items[in_item].carried = cn as u16;
                });
            }
        }
        3 => {
            // Mention Shiva
            State::with(|state| {
                state.do_sayx(
                    cn,
                    "Shiva, the mage who creates all the monsters, has the third part of it.",
                )
            });

            Repository::with_characters_mut(|ch| {
                ch[cn].data[2] = ticker + TICKS * 8;
                ch[cn].data[1] += 1;
            });
        }
        4 => {
            // No idea about other parts
            State::with(|state| state.do_sayx(cn, "I have no idea where the other parts are."));

            Repository::with_characters_mut(|ch| {
                ch[cn].data[2] = ticker + TICKS * 8;
                ch[cn].data[1] += 1;
            });
        }
        5 => {
            // Recall announcement
            State::with(|state| {
                state.do_sayx(cn, "I will recall now. I have enough of this prison!")
            });

            let (cn_x, cn_y) = Repository::with_characters(|ch| (ch[cn].x, ch[cn].y));

            EffectManager::fx_add_effect(7, 0, cn_x as i32, cn_y as i32, 0);

            Repository::with_characters_mut(|ch| {
                ch[cn].data[2] = ticker + TICKS * 6;
                ch[cn].data[1] += 1;
            });
        }
        6 => {
            // Final goodbye and disappear
            State::with(|state| {
                state.do_sayx(cn, "Good luck my friend. And thank you for freeing me!")
            });

            player::plr_map_remove(cn);
            God::destroy_items(cn);

            Repository::with_characters_mut(|ch| {
                ch[cn].used = USE_EMPTY;
            });
        }
        _ => {}
    }

    0
}

fn npc_malte_gotattack(cn: usize, co: usize) -> i32 {
    // Check if they're on different teams (data[42])
    let (cn_team, co_team) = Repository::with_characters(|ch| (ch[cn].data[42], ch[co].data[42]));

    if cn_team != co_team {
        // Get current attack target
        let cc = Repository::with_characters(|ch| ch[cn].attack_cn as usize);

        // If no current target or co is closer than current target
        if cc == 0 || crate::driver::npc_dist(cn, co) < crate::driver::npc_dist(cn, cc) {
            Repository::with_characters_mut(|ch| {
                ch[cn].attack_cn = co as u16;
                ch[cn].goto_x = 0;
            });
        }
    }

    1
}

pub fn npc_malte_msg(
    cn: usize,
    msg_type: i32,
    dat1: i32,
    _dat2: i32,
    _dat3: i32,
    _dat4: i32,
) -> i32 {
    match msg_type {
        x if x == NT_GOTHIT as i32 => npc_malte_gotattack(cn, dat1 as usize),
        x if x == NT_GOTMISS as i32 => npc_malte_gotattack(cn, dat1 as usize),
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
            let name =
                Repository::with_characters(|ch| String::from_utf8_lossy(&ch[cn].name).to_string());
            log::warn!("Unknown NPC message for {} ({}): {}", cn, name, msg_type);
            0
        }
    }
}
