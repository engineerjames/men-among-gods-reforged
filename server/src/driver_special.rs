use crate::repository::Repository;

struct Seen {
    co: usize,
    dist: i32,
    is_friend: bool,
    stun: i32,
    help: i32,
}

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
    let mut done = 0;

    Repository::with_characters_mut(|ch| ch[cn].data[92] = TICKS * 60);

    // Scan data slots 0-19 for nearby characters
    for n in 0..20 {
        let co = Repository::with_characters(|ch| ch[cn].data[n] as usize);
        if co != 0 && IS_SANECHAR(co) {
            let co_team = Repository::with_characters(|ch| ch[co].data[42]);
            let cn_team = Repository::with_characters(|ch| ch[cn].data[42]);

            if co_team == cn_team {
                // Friendly character
                seen[maxseen].co = co;
                seen[maxseen].dist = npc_dist(cn, co);
                seen[maxseen].is_friend = true;
                seen[maxseen].stun = 0;
                let low_hp = Repository::with_characters(|ch| ch[co].a_hp < ch[co].hp[5] * 400);
                seen[maxseen].help = if low_hp { 1 } else { 0 };
                help = help.max(seen[maxseen].help);
                maxseen += 1;
            } else {
                // Enemy character
                seen[maxseen].co = co;
                seen[maxseen].dist = npc_dist(cn, co);
                seen[maxseen].is_friend = false;
                if !npc_is_stunned(co) {
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
        if co != 0 && IS_SANECHAR(co) {
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
    if co != 0 && IS_SANECHAR(co) {
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
            seen[maxseen].dist = npc_dist(cn, co);
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
    let low_mana = Repository::with_characters(|ch| ch[cn].a_mana < ch[cn].mana[5] * 125);
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
    let low_hp = Repository::with_characters(|ch| ch[cn].a_hp < ch[cn].hp[5] * 666);
    if low_hp {
        flee += 5;
    }

    // Try to heal self if low HP
    if done == 0 && low_hp {
        done = npc_try_spell(cn, cn, SK_HEAL);
    }

    // Set movement mode based on endurance
    let high_endurance = Repository::with_characters(|ch| ch[cn].a_end > 15000);
    Repository::with_characters_mut(|ch| {
        ch[cn].mode = if high_endurance { 1 } else { 0 };
    });

    // Fleeing behavior
    if done == 0 && flee > 1 && flee >= help && flee >= stun {
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
            if !npc_check_target(cn_x, cn_y - n) {
                up -= 20;
                if !npc_check_target(cn_x + 1, cn_y - n) {
                    up -= 20;
                    if !npc_check_target(cn_x - 1, cn_y - n) {
                        up -= 10000;
                        break;
                    }
                }
            }
        }

        // Check if down is free space
        for n in 1..5 {
            if !npc_check_target(cn_x, cn_y + n) {
                down -= 20;
                if !npc_check_target(cn_x + 1, cn_y + n) {
                    down -= 20;
                    if !npc_check_target(cn_x - 1, cn_y + n) {
                        down -= 10000;
                        break;
                    }
                }
            }
        }

        // Check if left is free space
        for n in 1..5 {
            if !npc_check_target(cn_x - n, cn_y) {
                left -= 20;
                if !npc_check_target(cn_x - n, cn_y + 1) {
                    left -= 20;
                    if !npc_check_target(cn_x - n, cn_y - n) {
                        left -= 10000;
                        break;
                    }
                }
            }
        }

        // Check if right is free space
        for n in 1..5 {
            if !npc_check_target(cn_x + n, cn_y) {
                right -= 20;
                if !npc_check_target(cn_x + n, cn_y + 1) {
                    right -= 20;
                    if !npc_check_target(cn_x + n, cn_y - n) {
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
        if done == 0 && up >= down && up >= left && up >= right {
            if npc_check_target(cn_x, cn_y - 1) {
                Repository::with_characters_mut(|ch| {
                    ch[cn].goto_x = cn_x;
                    ch[cn].goto_y = cn_y - 1;
                });
                done = 1;
            } else if npc_check_target(cn_x + 1, cn_y - 1) {
                Repository::with_characters_mut(|ch| {
                    ch[cn].goto_x = cn_x + 1;
                    ch[cn].goto_y = cn_y - 1;
                });
                done = 1;
            } else if npc_check_target(cn_x - 1, cn_y - 1) {
                Repository::with_characters_mut(|ch| {
                    ch[cn].goto_x = cn_x - 1;
                    ch[cn].goto_y = cn_y - 1;
                });
                done = 1;
            }
        }

        if done == 0 && down >= up && down >= left && down >= right {
            if npc_check_target(cn_x, cn_y + 1) {
                Repository::with_characters_mut(|ch| {
                    ch[cn].goto_x = cn_x;
                    ch[cn].goto_y = cn_y + 1;
                });
                done = 1;
            } else if npc_check_target(cn_x + 1, cn_y + 1) {
                Repository::with_characters_mut(|ch| {
                    ch[cn].goto_x = cn_x + 1;
                    ch[cn].goto_y = cn_y + 1;
                });
                done = 1;
            } else if npc_check_target(cn_x - 1, cn_y + 1) {
                Repository::with_characters_mut(|ch| {
                    ch[cn].goto_x = cn_x - 1;
                    ch[cn].goto_y = cn_y + 1;
                });
                done = 1;
            }
        }

        if done == 0 && left >= up && left >= down && left >= right {
            if npc_check_target(cn_x - 1, cn_y) {
                Repository::with_characters_mut(|ch| {
                    ch[cn].goto_x = cn_x - 1;
                    ch[cn].goto_y = cn_y;
                });
                done = 1;
            } else if npc_check_target(cn_x - 1, cn_y + 1) {
                Repository::with_characters_mut(|ch| {
                    ch[cn].goto_x = cn_x - 1;
                    ch[cn].goto_y = cn_y + 1;
                });
                done = 1;
            } else if npc_check_target(cn_x - 1, cn_y - 1) {
                Repository::with_characters_mut(|ch| {
                    ch[cn].goto_x = cn_x - 1;
                    ch[cn].goto_y = cn_y - 1;
                });
                done = 1;
            }
        }

        if done == 0 && right >= up && right >= down && right >= left {
            if npc_check_target(cn_x + 1, cn_y) {
                Repository::with_characters_mut(|ch| {
                    ch[cn].goto_x = cn_x + 1;
                    ch[cn].goto_y = cn_y;
                });
                done = 1;
            } else if npc_check_target(cn_x + 1, cn_y + 1) {
                Repository::with_characters_mut(|ch| {
                    ch[cn].goto_x = cn_x + 1;
                    ch[cn].goto_y = cn_y + 1;
                });
                done = 1;
            } else if npc_check_target(cn_x + 1, cn_y - 1) {
                Repository::with_characters_mut(|ch| {
                    ch[cn].goto_x = cn_x + 1;
                    ch[cn].goto_y = cn_y - 1;
                });
                done = 1;
            }
        }

        // Panic - attack whoever is attacking us
        if done == 0 {
            let co = Repository::with_characters(|ch| ch[cn].data[20] as usize);
            if co != 0 {
                Repository::with_characters_mut(|ch| ch[cn].attack_cn = co);
                npc_try_spell(cn, co, SK_STUN);
                done = 1;
            }
        }
    }

    // Try self-buffs
    if done == 0 {
        done = npc_try_spell(cn, cn, SK_BLESS);
    }
    if done == 0 {
        done = npc_try_spell(cn, cn, SK_MSHIELD);
    }
    if done == 0 {
        done = npc_try_spell(cn, cn, SK_PROTECT);
    }
    if done == 0 {
        done = npc_try_spell(cn, cn, SK_ENHANCE);
    }

    // Stunning behavior
    if done == 0 && stun > 1 && stun >= help {
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
            done = npc_try_spell(cn, seen[m].co, SK_STUN);
            if done == 0 {
                done = npc_try_spell(cn, seen[m].co, SK_CURSE);
            }
            let ticker = Repository::with_globals(|g| g.ticker);
            Repository::with_characters_mut(|ch| ch[cn].data[24] = ticker);
        }
    }

    // Helping behavior
    if done == 0 && help > 0 {
        let mut m = 0;
        let mut tmp = 0;
        for n in 0..maxseen {
            if seen[n].help > tmp
                || (seen[n].help != 0 && seen[n].help == tmp && seen[n].dist < seen[m].dist)
            {
                let needs_help = Repository::with_characters(|ch| {
                    !npc_is_blessed(seen[n].co) || ch[seen[n].co].a_hp < ch[seen[n].co].hp[5] * 400
                });
                if needs_help {
                    tmp = seen[n].help;
                    m = n;
                }
            }
        }
        if tmp > 0 {
            let low_hp =
                Repository::with_characters(|ch| ch[seen[m].co].a_hp < ch[seen[m].co].hp[5] * 400);
            if low_hp {
                done = npc_try_spell(cn, seen[m].co, SK_HEAL);
            }
            if done == 0 {
                done = npc_try_spell(cn, seen[m].co, SK_BLESS);
            }
            if done == 0 {
                done = npc_try_spell(cn, seen[m].co, SK_PROTECT);
            }
            if done == 0 {
                done = npc_try_spell(cn, seen[m].co, SK_ENHANCE);
            }
            let ticker = Repository::with_globals(|g| g.ticker);
            Repository::with_characters_mut(|ch| ch[cn].data[24] = ticker);
        }
    }

    // Patrol state machine
    if done == 0 {
        let state = Repository::with_characters(|ch| ch[cn].data[22]);

        if state == 0 {
            // Staying at home
            let in_item = Repository::with_characters(|ch| ch[cn].citem);
            if in_item != 0 {
                Repository::with_characters_mut(|ch| ch[cn].citem = 0);
                Repository::with_items_mut(|items| items[in_item].used = USE_EMPTY);
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
                        let co = Repository::with_map(|map| map[x + y * SERVER_MAPX].ch);
                        if co != 0 {
                            let (co_team, cn_team) = Repository::with_characters(|ch| {
                                (ch[co].data[42], ch[cn].data[42])
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
                let in_item = god_create_item(718);
                Repository::with_characters_mut(|ch| ch[cn].citem = in_item);
                Repository::with_items_mut(|items| items[in_item].carried = cn);
            }
            let (cn_x, cn_y) = Repository::with_characters(|ch| (ch[cn].x, ch[cn].y));
            if (cn_x as i32 - 264).abs() + (cn_y as i32 - 317).abs() < 20 {
                Repository::with_characters_mut(|ch| {
                    ch[cn].data[22] = 2;
                    ch[cn].data[23] = ticker;
                });
            } else {
                if npc_check_target(264, 317) {
                    Repository::with_characters_mut(|ch| {
                        ch[cn].goto_x = 264;
                        ch[cn].goto_y = 317;
                    });
                } else if npc_check_target(265, 318) {
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

pub fn npc_stunrun_msg(
    character_id: usize,
    msg_type: i32,
    dat1: i32,
    dat2: i32,
    dat3: i32,
    dat4: i32,
) -> i32 {
    0
}

pub fn npc_cityattack_high(character_id: usize) -> i32 {
    0
}

pub fn npc_cityattack_low(character_id: usize) -> i32 {
    0
}

pub fn npc_cityattack_msg(
    character_id: usize,
    msg_type: i32,
    dat1: i32,
    dat2: i32,
    dat3: i32,
    dat4: i32,
) -> i32 {
    0
}

pub fn npc_malte_high(character_id: usize) -> i32 {
    0
}

pub fn npc_malte_low(character_id: usize) -> i32 {
    0
}

pub fn npc_malte_msg(
    character_id: usize,
    msg_type: i32,
    dat1: i32,
    dat2: i32,
    dat3: i32,
    dat4: i32,
) -> i32 {
    0
}
