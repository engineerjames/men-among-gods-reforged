use core::{
    constants::{CharacterFlags, SPEEDTAB, TICKS},
    logout_reasons::LogoutReason,
    server_commands::ServerCommandType,
};

use crate::{
    driver,
    game_state::GameState,
    network_manager,
    player::{
        commands::{
            plr_doact, plr_misc, plr_move_down, plr_move_left, plr_move_leftdown, plr_move_leftup,
            plr_move_right, plr_move_rightdown, plr_move_rightup, plr_move_up, plr_turn_down,
            plr_turn_left, plr_turn_leftdown, plr_turn_leftup, plr_turn_right, plr_turn_rightdown,
            plr_turn_rightup, plr_turn_up,
        },
        connection::{plr_login, plr_logout, plr_newlogin},
        map::{plr_change_light, plr_change_map, plr_change_position},
    },
};

/// Periodic medium-rate driver using an explicit game state.
///
/// # Arguments
/// * `gs` - Active game state used for ticker and follow target lookup.
/// * `cn` - Character index to process.
pub fn player_driver_med(gs: &mut GameState, cn: usize) {
    let ticker = gs.globals.ticker;
    if gs.characters[cn].data[12] + core::constants::TICKS * 15 > ticker {
        return;
    }

    let co = gs.characters[cn].data[10];
    if co != 0 {
        driver::follow_driver(gs, cn, co as usize);
    }
}

/// Port of `plr_act` from `svr_tick.cpp`
///
/// Per-character action state machine executed each tick. Handles stunned/
/// stoned conditions, executes idle/driver actions, advances walking/turning
/// frames based on `speedo`, and triggers move/turn/misc handlers when a
/// frame sequence completes.
///
/// # Arguments
/// * `cn` - Character index to process
pub fn plr_act(gs: &mut GameState, cn: usize) {
    let (stunned, flags, status) = (
        gs.characters[cn].stunned,
        gs.characters[cn].flags,
        gs.characters[cn].status,
    );

    if stunned != 0 {
        driver::act_idle(gs, cn);
        return;
    }

    if flags & CharacterFlags::Stoned.bits() != 0 {
        driver::act_idle(gs, cn);
        return;
    }

    match status {
        // idle states: call idle and driver
        0..=7 => {
            driver::act_idle(gs, cn);
            plr_doact(gs, cn);
        }

        // walk up: 16..22 increment, 23 execute
        16..=22 => {
            if speedo(gs, cn) != 0 {
                gs.characters[cn].status += 1;
            }
        }
        23 => {
            if speedo(gs, cn) != 0 {
                gs.characters[cn].status = 16;
                plr_move_up(gs, cn);
                plr_doact(gs, cn);
            }
        }

        // walk down: 24..30 then 31
        24..=30 => {
            if speedo(gs, cn) != 0 {
                gs.characters[cn].status += 1;
            }
        }
        31 => {
            if speedo(gs, cn) != 0 {
                gs.characters[cn].status = 24;
                plr_move_down(gs, cn);
                plr_doact(gs, cn);
            }
        }

        // walk left: 32..38 then 39
        32..=38 => {
            if speedo(gs, cn) != 0 {
                gs.characters[cn].status += 1;
            }
        }
        39 => {
            if speedo(gs, cn) != 0 {
                gs.characters[cn].status = 32;
                plr_move_left(gs, cn);
                plr_doact(gs, cn);
            }
        }

        // walk right: 40..46 then 47
        40..=46 => {
            if speedo(gs, cn) != 0 {
                gs.characters[cn].status += 1;
            }
        }
        47 => {
            if speedo(gs, cn) != 0 {
                gs.characters[cn].status = 40;
                plr_move_right(gs, cn);
                plr_doact(gs, cn);
            }
        }

        // left+up: 48..58 then 59
        48..=58 => {
            if speedo(gs, cn) != 0 {
                gs.characters[cn].status += 1;
            }
        }
        59 => {
            if speedo(gs, cn) != 0 {
                gs.characters[cn].status = 48;
                plr_move_leftup(gs, cn);
                plr_doact(gs, cn);
            }
        }

        // left+down: 60..70 then 71
        60..=70 => {
            if speedo(gs, cn) != 0 {
                gs.characters[cn].status += 1;
            }
        }
        71 => {
            if speedo(gs, cn) != 0 {
                gs.characters[cn].status = 60;
                plr_move_leftdown(gs, cn);
                plr_doact(gs, cn);
            }
        }

        // right+up: 72..82 then 83
        72..=82 => {
            if speedo(gs, cn) != 0 {
                gs.characters[cn].status += 1;
            }
        }
        83 => {
            if speedo(gs, cn) != 0 {
                gs.characters[cn].status = 72;
                plr_move_rightup(gs, cn);
                plr_doact(gs, cn);
            }
        }

        // right+down: 84..94 then 95
        84..=94 => {
            if speedo(gs, cn) != 0 {
                gs.characters[cn].status += 1;
            }
        }
        95 => {
            if speedo(gs, cn) != 0 {
                gs.characters[cn].status = 84;
                plr_move_rightdown(gs, cn);
                plr_doact(gs, cn);
            }
        }

        // turns: grouped ranges mapping to final turn actions
        96..=98 => {
            if speedo(gs, cn) != 0 {
                gs.characters[cn].status += 1;
            }
        }
        99 => {
            if speedo(gs, cn) != 0 {
                gs.characters[cn].status = 96;
                plr_turn_leftup(gs, cn);
                plr_doact(gs, cn);
            }
        }

        100..=102 => {
            if speedo(gs, cn) != 0 {
                gs.characters[cn].status += 1;
            }
        }
        103 => {
            if speedo(gs, cn) != 0 {
                gs.characters[cn].status = 96;
                plr_turn_left(gs, cn);
                plr_doact(gs, cn);
            }
        }

        104..=106 => {
            if speedo(gs, cn) != 0 {
                gs.characters[cn].status += 1;
            }
        }
        107 => {
            if speedo(gs, cn) != 0 {
                gs.characters[cn].status = 104;
                plr_turn_rightup(gs, cn);
                plr_doact(gs, cn);
            }
        }

        108..=110 => {
            if speedo(gs, cn) != 0 {
                gs.characters[cn].status += 1;
            }
        }
        111 => {
            if speedo(gs, cn) != 0 {
                gs.characters[cn].status = 108;
                plr_turn_right(gs, cn);
                plr_doact(gs, cn);
            }
        }

        112..=114 => {
            if speedo(gs, cn) != 0 {
                gs.characters[cn].status += 1;
            }
        }
        115 => {
            if speedo(gs, cn) != 0 {
                gs.characters[cn].status = 112;
                plr_turn_leftdown(gs, cn);
                plr_doact(gs, cn);
            }
        }

        116..=118 => {
            if speedo(gs, cn) != 0 {
                gs.characters[cn].status += 1;
            }
        }
        119 => {
            if speedo(gs, cn) != 0 {
                gs.characters[cn].status = 116;
                plr_turn_left(gs, cn);
                plr_doact(gs, cn);
            }
        }

        120..=122 => {
            if speedo(gs, cn) != 0 {
                gs.characters[cn].status += 1;
            }
        }
        123 => {
            if speedo(gs, cn) != 0 {
                gs.characters[cn].status = 120;
                plr_turn_rightdown(gs, cn);
                plr_doact(gs, cn);
            }
        }

        124..=126 => {
            if speedo(gs, cn) != 0 {
                gs.characters[cn].status += 1;
            }
        }
        127 => {
            if speedo(gs, cn) != 0 {
                gs.characters[cn].status = 124;
                plr_turn_right(gs, cn);
                plr_doact(gs, cn);
            }
        }

        128..=130 => {
            if speedo(gs, cn) != 0 {
                gs.characters[cn].status += 1;
            }
        }
        131 => {
            if speedo(gs, cn) != 0 {
                gs.characters[cn].status = 128;
                plr_turn_leftup(gs, cn);
                plr_doact(gs, cn);
            }
        }

        132..=134 => {
            if speedo(gs, cn) != 0 {
                gs.characters[cn].status += 1;
            }
        }
        135 => {
            if speedo(gs, cn) != 0 {
                gs.characters[cn].status = 132;
                plr_turn_up(gs, cn);
                plr_doact(gs, cn);
            }
        }

        136..=138 => {
            if speedo(gs, cn) != 0 {
                gs.characters[cn].status += 1;
            }
        }
        139 => {
            if speedo(gs, cn) != 0 {
                gs.characters[cn].status = 136;
                plr_turn_leftdown(gs, cn);
                plr_doact(gs, cn);
            }
        }

        140..=142 => {
            if speedo(gs, cn) != 0 {
                gs.characters[cn].status += 1;
            }
        }
        143 => {
            if speedo(gs, cn) != 0 {
                gs.characters[cn].status = 140;
                plr_turn_down(gs, cn);
                plr_doact(gs, cn);
            }
        }

        144..=146 => {
            if speedo(gs, cn) != 0 {
                gs.characters[cn].status += 1;
            }
        }
        147 => {
            if speedo(gs, cn) != 0 {
                gs.characters[cn].status = 144;
                plr_turn_rightup(gs, cn);
                plr_doact(gs, cn);
            }
        }

        148..=150 => {
            if speedo(gs, cn) != 0 {
                gs.characters[cn].status += 1;
            }
        }
        151 => {
            if speedo(gs, cn) != 0 {
                gs.characters[cn].status = 148;
                plr_turn_up(gs, cn);
                plr_doact(gs, cn);
            }
        }

        152..=154 => {
            if speedo(gs, cn) != 0 {
                gs.characters[cn].status += 1;
            }
        }
        155 => {
            if speedo(gs, cn) != 0 {
                gs.characters[cn].status = 152;
                plr_turn_rightdown(gs, cn);
                plr_doact(gs, cn);
            }
        }

        156..=158 => {
            if speedo(gs, cn) != 0 {
                gs.characters[cn].status += 1;
            }
        }
        159 => {
            if speedo(gs, cn) != 0 {
                gs.characters[cn].status = 156;
                plr_turn_down(gs, cn);
                plr_doact(gs, cn);
            }
        }

        // misc actions: 160..166 increment, 167 execute misc then doact
        160..=166 => {
            if speedo(gs, cn) != 0 {
                gs.characters[cn].status += 1;
            }
        }
        167 => {
            if speedo(gs, cn) != 0 {
                gs.characters[cn].status = 160;
                plr_misc(gs, cn);
                plr_doact(gs, cn);
            }
        }

        // misc down 168..174 then 175
        168..=174 => {
            if speedo(gs, cn) != 0 {
                gs.characters[cn].status += 1;
            }
        }
        175 => {
            if speedo(gs, cn) != 0 {
                gs.characters[cn].status = 168;
                plr_misc(gs, cn);
                plr_doact(gs, cn);
            }
        }

        // misc left 176..182 then 183
        176..=182 => {
            if speedo(gs, cn) != 0 {
                gs.characters[cn].status += 1;
            }
        }
        183 => {
            if speedo(gs, cn) != 0 {
                gs.characters[cn].status = 176;
                plr_misc(gs, cn);
                plr_doact(gs, cn);
            }
        }

        // misc right 184..190 then 191
        184..=190 => {
            if speedo(gs, cn) != 0 {
                gs.characters[cn].status += 1;
            }
        }
        191 => {
            if speedo(gs, cn) != 0 {
                gs.characters[cn].status = 184;
                plr_misc(gs, cn);
                plr_doact(gs, cn);
            }
        }

        _ => {
            let status = gs.characters[cn].status;
            log::error!(
                "plr_act: unknown character status {} for char {}",
                status,
                cn
            );
            gs.characters[cn].status = 0;
        }
    }
}

/// Fast helper to compute the per-tick movement index for a character.
///
/// Uses a precomputed `SPEEDTAB` and the global ticker modulo to determine
/// whether the character moves on the current sub-tick.
///
/// # Arguments
/// * `n` - Character index
pub fn speedo(gs: &mut GameState, n: usize) -> i32 {
    let speed = (gs.characters[n].speed as usize).min(core::constants::MAX_SPEEDTAB_SPEED_INDEX);
    let ctick = gs.globals.ticker as usize % core::constants::CTICK_CYCLE_LEN;
    SPEEDTAB[speed][ctick] as i32
}

/// Port of `plr_state` from `svr_tick.cpp`
/// Handles player state transitions (login, exit, timeouts)
pub fn plr_state(gs: &mut GameState, nr: usize) {
    let ticker = gs.globals.ticker;
    let (lasttick, state) = (gs.players[nr].lasttick as i32, gs.players[nr].state);

    // Handle ST_EXIT timeout - close connection after 15 seconds
    if ticker.wrapping_sub(lasttick) > core::constants::TICKS * 15
        && state == core::constants::ST_EXIT
    {
        log::info!("Connection closed (ST_EXIT) for player {}", nr);
        gs.players[nr].sock = None;
        return;
    }

    // Handle idle timeout - logout after 60 seconds
    if ticker.wrapping_sub(lasttick) > core::constants::TICKS * 60 {
        log::info!("Idle timeout for player {}", nr);
        plr_logout(gs, 0, nr, LogoutReason::IdleTooLong);
        return;
    }

    match state {
        state if state == core::constants::ST_NEWLOGIN => {
            plr_newlogin(gs, nr);
        }
        state if state == core::constants::ST_LOGIN => {
            plr_login(gs, nr);
        }
        state if state == core::constants::ST_NEWCAP => {
            // Timeout after 10 seconds, go back to NEWLOGIN
            if ticker.wrapping_sub(lasttick) > core::constants::TICKS * 10 {
                gs.players[nr].state = core::constants::ST_NEWLOGIN;
            }
        }
        state if state == core::constants::ST_CAP => {
            // Timeout after 10 seconds, go back to LOGIN
            if ticker.wrapping_sub(lasttick) > core::constants::TICKS * 10 {
                gs.players[nr].state = core::constants::ST_LOGIN;
            }
        }
        state if state == core::constants::ST_NEW_CHALLENGE => {
            // Do nothing - waiting for challenge response
        }
        state if state == core::constants::ST_LOGIN_CHALLENGE => {
            // Do nothing - waiting for challenge response
        }
        state if state == core::constants::ST_CONNECT => {
            // Do nothing - initial connection state
        }
        state if state == core::constants::ST_EXIT => {
            // Do nothing - handled above
        }
        _ => {
            log::warn!("UNKNOWN ST: {} for player {}", state, nr);
        }
    }
}

/// Port of `plr_change` from `svr_tick.cpp`
/// Sends changed player data to the client
pub fn plr_change(gs: &mut GameState, nr: usize) {
    let cn = gs.players[nr].usnr;

    if cn == 0 || cn >= core::constants::MAXCHARS {
        log::error!("plr_change: invalid character number {}", cn);
        return;
    }

    let ticker = gs.globals.ticker;
    let should_update = {
        let has_update_flag = (gs.characters[cn].flags & CharacterFlags::Update.bits()) != 0;
        let ticker_match = (cn & 15) == (ticker as usize & 15);
        has_update_flag || ticker_match
    };

    if should_update {
        // Send full player stats update
        plr_change_stats(gs, nr, cn, ticker);
    }

    // Always send combat-related updates
    plr_change_hp(gs, nr, cn);
    plr_change_end(gs, nr, cn);
    plr_change_mana(gs, nr, cn);
    plr_change_dir(gs, nr, cn);
    plr_change_points(gs, nr, cn);
    plr_change_gold(gs, nr, cn);

    // Send god load info every 32 ticks
    plr_change_load(gs, nr, cn, ticker);

    // Send map position and scrolling
    plr_change_position(gs, nr, cn);

    // Send light updates
    plr_change_light(gs, nr);

    // Send tile content updates
    plr_change_map(gs, nr);

    // Send target updates
    plr_change_target(gs, nr, cn);
}

/// Send full stats update to player
fn plr_change_stats(gs: &mut GameState, nr: usize, cn: usize, _ticker: i32) {
    // Send name in three parts if changed
    let name_changed = gs.players[nr].cpl.name[..] != gs.characters[cn].name[..];

    if name_changed {
        let ch = gs.characters[cn];
        // part1: 15 bytes
        let mut buf: [u8; 16] = [0; 16];
        buf[0] = ServerCommandType::SetCharName1 as u8;
        buf[1..16].copy_from_slice(&ch.name[0..15]);
        network_manager::xsend(gs, nr, &buf, 16);

        // part2: next 15 bytes
        let mut buf2: [u8; 16] = [0; 16];
        buf2[0] = ServerCommandType::SetCharName2 as u8;
        buf2[1..16].copy_from_slice(&ch.name[15..30]);
        network_manager::xsend(gs, nr, &buf2, 16);

        // part3: last 10 bytes + temp (u16 -> u32 slot)
        let mut buf3: [u8; 16] = [0; 16];
        buf3[0] = ServerCommandType::SetCharName3 as u8;
        buf3[1..11].copy_from_slice(&ch.name[30..40]);
        let temp_bytes = (ch.temp as u32).to_le_bytes();
        buf3[11..15].copy_from_slice(&temp_bytes[0..4]);
        network_manager::xsend(gs, nr, &buf3, 16);

        gs.players[nr]
            .cpl
            .name
            .copy_from_slice(&gs.characters[cn].name);
    }

    // send mode if different
    let mode = gs.characters[cn].mode as i32;
    if gs.players[nr].cpl.mode != mode {
        let mode = gs.characters[cn].mode;
        let mut buf: [u8; 2] = [0; 2];
        buf[0] = ServerCommandType::SetCharMode as u8;
        buf[1] = mode;
        network_manager::xsend(gs, nr, &buf, 2);
        gs.players[nr].cpl.mode = mode as i32;
    }

    // attribs (5 x 6 bytes)
    for a in 0..5usize {
        let chv = gs.characters[cn].attrib[a];
        let changed = gs.players[nr].cpl.attrib[a] != chv;
        if changed {
            let bytes = gs.characters[cn].attrib[a];
            let mut buf: [u8; 8] = [0; 8];
            buf[0] = ServerCommandType::SetCharAttrib as u8;
            buf[1] = a as u8;
            buf[2..8].copy_from_slice(&bytes);
            network_manager::xsend(gs, nr, &buf, 8);
            gs.players[nr].cpl.attrib[a] = bytes;
        }
    }

    // hp, end, mana arrays (6 u16 each)
    let powers = [
        ServerCommandType::SetCharHp,
        ServerCommandType::SetCharEndur,
        ServerCommandType::SetCharMana,
    ];
    for (idx, code) in powers.iter().enumerate() {
        let ch = gs.characters[cn];
        let different = match idx {
            0 => gs.players[nr].cpl.hp != ch.hp,
            1 => gs.players[nr].cpl.end != ch.end,
            2 => gs.players[nr].cpl.mana != ch.mana,
            _ => false,
        };
        if different {
            let mut buf: [u8; 13] = [0; 13];
            buf[0] = *code as u8;
            let arr: [u16; 6] = match idx {
                0 => ch.hp,
                1 => ch.end,
                2 => ch.mana,
                _ => ch.hp,
            };
            for i in 0..6 {
                let off = 1 + i * 2;
                let v = arr[i];
                buf[off] = (v & 0xff) as u8;
                buf[off + 1] = (v >> 8) as u8;
            }
            network_manager::xsend(gs, nr, &buf, 13);
            match idx {
                0 => gs.players[nr].cpl.hp = ch.hp,
                1 => gs.players[nr].cpl.end = ch.end,
                2 => gs.players[nr].cpl.mana = ch.mana,
                _ => {}
            }
        }
    }

    // skills (0..50)
    for s in 0..50usize {
        let chv = gs.characters[cn].skill[s];
        let changed = gs.players[nr].cpl.skill[s] != chv;
        if changed {
            let bytes = gs.characters[cn].skill[s];
            let mut buf: [u8; 8] = [0; 8];
            buf[0] = ServerCommandType::SetCharSkill as u8;
            buf[1] = s as u8;
            buf[2..8].copy_from_slice(&bytes);
            network_manager::xsend(gs, nr, &buf, 8);
            gs.players[nr].cpl.skill[s] = bytes;
        }
    }

    // items (40)
    for i in 0..40usize {
        let is_building = gs.characters[cn].is_building();
        let in_idx = gs.characters[cn].item[i] as usize;
        let cpl_item = gs.players[nr].cpl.item[i];

        // Check if changed OR if IF_UPDATE is set (but not for building mode)
        let needs_update = if in_idx != 0 && !is_building {
            (cpl_item != in_idx as i32)
                || ((gs.items[in_idx].flags & core::constants::ItemFlags::IF_UPDATE.bits()) != 0)
        } else {
            cpl_item != in_idx as i32
        };

        if needs_update {
            let mut buf: [u8; 9] = [0; 9];
            buf[0] = ServerCommandType::SetCharItem as u8;
            let idx_bytes = (i as u32).to_le_bytes();
            buf[1..5].copy_from_slice(&idx_bytes);

            if in_idx != 0 {
                if is_building {
                    // Building mode - handle special flags and templates
                    if (in_idx & 0x40000000) != 0 {
                        // Map flags
                        let flag = in_idx & 0x0fffffff;
                        let sprite = match flag as u32 {
                            core::constants::MF_MOVEBLOCK => 47,
                            core::constants::MF_SIGHTBLOCK => 83,
                            core::constants::MF_INDOORS => 48,
                            core::constants::MF_UWATER => 50,
                            core::constants::MF_NOMONST => 51,
                            core::constants::MF_BANK => 52,
                            core::constants::MF_TAVERN => 53,
                            core::constants::MF_NOMAGIC => 54,
                            core::constants::MF_DEATHTRAP => 74,
                            core::constants::MF_ARENA => 78,
                            core::constants::MF_NOEXPIRE => 81,
                            core::constants::MF_NOLAG => 49,
                            _ => 0,
                        };
                        buf[5] = (sprite & 0xff) as u8;
                        buf[6] = ((sprite >> 8) & 0xff) as u8;
                        buf[7] = 0;
                        buf[8] = 0;
                    } else if (in_idx & 0x20000000) != 0 {
                        // Direct sprite reference
                        let sprite = (in_idx & 0x0fffffff) as i16;
                        buf[5] = (sprite & 0xff) as u8;
                        buf[6] = ((sprite >> 8) & 0xff) as u8;
                        buf[7] = 0;
                        buf[8] = 0;
                    } else {
                        // Template item
                        let sprite = gs.item_templates[in_idx].sprite[0];
                        buf[5] = (sprite & 0xff) as u8;
                        buf[6] = ((sprite >> 8) & 0xff) as u8;
                        buf[7] = 0;
                        buf[8] = 0;
                    }
                } else {
                    // Normal mode - use item sprite and placement
                    {
                        let it = &gs.items[in_idx];
                        let sprite = if it.active != 0 {
                            it.sprite[1]
                        } else {
                            it.sprite[0]
                        };
                        let placement = it.placement as i16;
                        buf[5] = (sprite & 0xff) as u8;
                        buf[6] = ((sprite >> 8) & 0xff) as u8;
                        buf[7] = (placement & 0xff) as u8;
                        buf[8] = ((placement >> 8) & 0xff) as u8;
                    }
                    // Clear IF_UPDATE flag
                    gs.items[in_idx].flags &= !core::constants::ItemFlags::IF_UPDATE.bits();
                }
            } else {
                buf[5] = 0;
                buf[6] = 0;
                buf[7] = 0;
                buf[8] = 0;
            }

            network_manager::xsend(gs, nr, &buf, 9);
            gs.players[nr].cpl.item[i] = in_idx as i32;
        }
    }

    // worn (20)
    for i in 0..20usize {
        let in_idx = gs.characters[cn].worn[i] as usize;
        let cpl_worn = gs.players[nr].cpl.worn[i];

        // Check if changed OR if IF_UPDATE is set
        let needs_update = if in_idx != 0 {
            (cpl_worn != in_idx as i32)
                || ((gs.items[in_idx].flags & core::constants::ItemFlags::IF_UPDATE.bits()) != 0)
        } else {
            cpl_worn != in_idx as i32
        };

        if needs_update {
            let mut buf: [u8; 9] = [0; 9];
            buf[0] = ServerCommandType::SetCharWorn as u8;
            let idx_bytes = (i as u32).to_le_bytes();
            buf[1..5].copy_from_slice(&idx_bytes);

            if in_idx != 0 {
                {
                    let it = &gs.items[in_idx];
                    let sprite = if it.active != 0 {
                        it.sprite[1]
                    } else {
                        it.sprite[0]
                    };
                    let placement = it.placement as i16;
                    buf[5] = (sprite & 0xff) as u8;
                    buf[6] = ((sprite >> 8) & 0xff) as u8;
                    buf[7] = (placement & 0xff) as u8;
                    buf[8] = ((placement >> 8) & 0xff) as u8;
                }
                // Clear IF_UPDATE flag
                gs.items[in_idx].flags &= !core::constants::ItemFlags::IF_UPDATE.bits();
            } else {
                buf[5] = 0;
                buf[6] = 0;
                buf[7] = 0;
                buf[8] = 0;
            }

            network_manager::xsend(gs, nr, &buf, 9);
            gs.players[nr].cpl.worn[i] = in_idx as i32;
        }
    }

    // spells (20)
    for i in 0..20usize {
        let in_idx = gs.characters[cn].spell[i] as usize;
        let cpl_spell = gs.players[nr].cpl.spell[i];
        let cpl_active = gs.players[nr].cpl.active[i];

        // Calculate current active fraction
        let (current_active_frac, has_update_flag) = if in_idx != 0 {
            let it = &gs.items[in_idx];
            let duration = std::cmp::max(1, it.duration);
            let frac = ((it.active * 16) / duration) as i16;
            let has_flag = (it.flags & core::constants::ItemFlags::IF_UPDATE.bits()) != 0;
            (frac, has_flag)
        } else {
            (0, false)
        };

        // Check if spell changed OR active fraction changed OR IF_UPDATE is set
        let needs_update = (cpl_spell != in_idx as i32)
            || (cpl_active as i16 != current_active_frac)
            || has_update_flag;

        if needs_update {
            let mut buf: [u8; 9] = [0; 9];
            buf[0] = ServerCommandType::SetCharSpell as u8;
            let idx_bytes = (i as u32).to_le_bytes();
            buf[1..5].copy_from_slice(&idx_bytes);

            if in_idx != 0 {
                {
                    let it = &gs.items[in_idx];
                    let sprite = it.sprite[1];
                    let duration = std::cmp::max(1, it.duration);
                    let active_frac = ((it.active * 16) / duration) as i16;

                    buf[5] = (sprite & 0xff) as u8;
                    buf[6] = ((sprite >> 8) & 0xff) as u8;
                    buf[7] = (active_frac & 0xff) as u8;
                    buf[8] = ((active_frac >> 8) & 0xff) as u8;
                }
                // Clear IF_UPDATE flag
                gs.items[in_idx].flags &= !core::constants::ItemFlags::IF_UPDATE.bits();
                gs.players[nr].cpl.spell[i] = in_idx as i32;
                gs.players[nr].cpl.active[i] = current_active_frac as i8;
            } else {
                buf[5] = 0;
                buf[6] = 0;
                buf[7] = 0;
                buf[8] = 0;
                gs.players[nr].cpl.spell[i] = 0;
                gs.players[nr].cpl.active[i] = 0;
            }

            network_manager::xsend(gs, nr, &buf, 9);
        }
    }

    // citem (cursor item)
    let is_building = gs.characters[cn].is_building();
    let in_idx = gs.characters[cn].citem as usize;
    let cpl_citem = gs.players[nr].cpl.citem;

    // Check if changed OR if IF_UPDATE is set (but not for building mode or gold amounts)
    let needs_update = if in_idx != 0 && !is_building && (in_idx & 0x80000000) == 0 {
        (cpl_citem != in_idx as i32)
            || ((gs.items[in_idx].flags & core::constants::ItemFlags::IF_UPDATE.bits()) != 0)
    } else {
        cpl_citem != in_idx as i32
    };

    if needs_update {
        let mut buf: [u8; 5] = [0; 5];
        buf[0] = ServerCommandType::SetCharObj as u8;

        if (in_idx & 0x80000000) != 0 {
            // Gold amount - use special sprites based on amount
            let amount = in_idx & 0x7fffffff;
            let sprite = if amount > 999999 {
                121
            } else if amount > 99999 {
                120
            } else if amount > 9999 {
                41
            } else if amount > 999 {
                40
            } else if amount > 99 {
                39
            } else if amount > 9 {
                38
            } else {
                37
            };
            buf[1] = (sprite & 0xff) as u8;
            buf[2] = ((sprite >> 8) & 0xff) as u8;
            buf[3] = 0;
            buf[4] = 0;
        } else if in_idx != 0 {
            if is_building {
                // Building mode - fixed sprite
                buf[1] = 46;
                buf[2] = 0;
                buf[3] = 0;
                buf[4] = 0;
            } else {
                // Normal item
                {
                    let it = &gs.items[in_idx];
                    let sprite = if it.active != 0 {
                        it.sprite[1]
                    } else {
                        it.sprite[0]
                    };
                    let placement = it.placement as i16;
                    buf[1] = (sprite & 0xff) as u8;
                    buf[2] = ((sprite >> 8) & 0xff) as u8;
                    buf[3] = (placement & 0xff) as u8;
                    buf[4] = ((placement >> 8) & 0xff) as u8;
                }
                // Clear IF_UPDATE flag
                gs.items[in_idx].flags &= !core::constants::ItemFlags::IF_UPDATE.bits();
            }
        } else {
            // Empty cursor
            buf[1] = 0;
            buf[2] = 0;
            buf[3] = 0;
            buf[4] = 0;
        }

        network_manager::xsend(gs, nr, &buf, 5);
        gs.players[nr].cpl.citem = in_idx as i32;
    }
}

/// Send HP change to player
fn plr_change_hp(gs: &mut GameState, nr: usize, cn: usize) {
    let current_hp = (gs.characters[cn].a_hp + 500) / 1000;
    let player_hp = gs.players[nr].cpl.a_hp;

    if current_hp != player_hp {
        let mut buf: [u8; 16] = [0; 16];
        buf[0] = ServerCommandType::SetCharHp as u8;
        buf[1] = current_hp as u8;
        buf[2] = (current_hp >> 8) as u8;

        network_manager::xsend(gs, nr, &buf, 3);
        gs.players[nr].cpl.a_hp = current_hp;
    }
}

/// Send endurance change to player
fn plr_change_end(gs: &mut GameState, nr: usize, cn: usize) {
    let current_end = (gs.characters[cn].a_end + 500) / 1000;
    let player_end = gs.players[nr].cpl.a_end;

    if current_end != player_end {
        let mut buf: [u8; 16] = [0; 16];
        buf[0] = ServerCommandType::SetCharEndur as u8;
        buf[1] = current_end as u8;
        buf[2] = (current_end >> 8) as u8;

        network_manager::xsend(gs, nr, &buf, 3);
        gs.players[nr].cpl.a_end = current_end;
    }
}

/// Send mana change to player
fn plr_change_mana(gs: &mut GameState, nr: usize, cn: usize) {
    let current_mana = (gs.characters[cn].a_mana + 500) / 1000;
    let player_mana = gs.players[nr].cpl.a_mana;

    if current_mana != player_mana {
        let mut buf: [u8; 16] = [0; 16];
        buf[0] = ServerCommandType::SetCharMana as u8;
        buf[1] = current_mana as u8;
        buf[2] = (current_mana >> 8) as u8;

        network_manager::xsend(gs, nr, &buf, 3);
        gs.players[nr].cpl.a_mana = current_mana;
    }
}

/// Send direction change to player
fn plr_change_dir(gs: &mut GameState, nr: usize, cn: usize) {
    let current_dir = gs.characters[cn].dir;
    let player_dir = gs.players[nr].cpl.dir;

    if current_dir as i32 != player_dir {
        let mut buf: [u8; 16] = [0; 16];
        buf[0] = ServerCommandType::SetCharDir as u8;
        buf[1] = current_dir;

        network_manager::xsend(gs, nr, &buf, 2);
        gs.players[nr].cpl.dir = current_dir as i32;
    }
}

/// Send points/kindred change to player
fn plr_change_points(gs: &mut GameState, nr: usize, cn: usize) {
    let points = gs.characters[cn].points;
    let points_tot = gs.characters[cn].points_tot;
    let kindred = gs.characters[cn].kindred;
    let cpl_points = gs.players[nr].cpl.points;
    let cpl_points_tot = gs.players[nr].cpl.points_tot;
    let cpl_kindred = gs.players[nr].cpl.kindred;

    if points != cpl_points || points_tot != cpl_points_tot || kindred != cpl_kindred {
        let mut buf: [u8; 13] = [0; 13];
        buf[0] = ServerCommandType::SetCharPts as u8;
        buf[1..5].copy_from_slice(&points.to_le_bytes());
        buf[5..9].copy_from_slice(&points_tot.to_le_bytes());
        buf[9..13].copy_from_slice(&kindred.to_le_bytes());

        network_manager::xsend(gs, nr, &buf, 13);

        gs.players[nr].cpl.points = points;
        gs.players[nr].cpl.points_tot = points_tot;
        gs.players[nr].cpl.kindred = kindred;
    }
}

/// Send gold/armor/weapon change to player
fn plr_change_gold(gs: &mut GameState, nr: usize, cn: usize) {
    let gold = gs.characters[cn].gold;
    let armor = gs.characters[cn].armor;
    let weapon = gs.characters[cn].weapon;
    let cpl_gold = gs.players[nr].cpl.gold;
    let cpl_armor = gs.players[nr].cpl.armor;
    let cpl_weapon = gs.players[nr].cpl.weapon;

    if gold != cpl_gold || armor as i32 != cpl_armor || weapon as i32 != cpl_weapon {
        let armor32: i32 = armor as i32;
        let weapon32: i32 = weapon as i32;

        let mut buf: [u8; 13] = [0; 13];
        buf[0] = ServerCommandType::SetCharGold as u8;
        buf[1..5].copy_from_slice(&gold.to_le_bytes());
        buf[5..9].copy_from_slice(&armor32.to_le_bytes());
        buf[9..13].copy_from_slice(&weapon32.to_le_bytes());

        network_manager::xsend(gs, nr, &buf, 13);

        gs.players[nr].cpl.gold = gold;
        gs.players[nr].cpl.armor = armor as i32;
        gs.players[nr].cpl.weapon = weapon as i32;
    }
}

/// Send server load info to gods every 32 ticks
fn plr_change_load(gs: &mut GameState, nr: usize, cn: usize, ticker: i32) {
    let is_god = (gs.characters[cn].flags & CharacterFlags::God.bits()) != 0;

    if is_god && (ticker & 31) == 0 {
        let load = gs.globals.load as u32;
        let mut buf: [u8; 5] = [0; 5];
        buf[0] = ServerCommandType::Load as u8;
        buf[1..5].copy_from_slice(&load.to_le_bytes());
        network_manager::xsend(gs, nr, &buf, 5);
    }
}

/// Send target change to player
fn plr_change_target(gs: &mut GameState, nr: usize, cn: usize) {
    let (attack_cn, goto_x, goto_y, misc_action, misc_target1, misc_target2) = (
        gs.characters[cn].attack_cn,
        gs.characters[cn].goto_x,
        gs.characters[cn].goto_y,
        gs.characters[cn].misc_action,
        gs.characters[cn].misc_target1,
        gs.characters[cn].misc_target2,
    );

    let (
        cpl_attack_cn,
        cpl_goto_x,
        cpl_goto_y,
        cpl_misc_action,
        cpl_misc_target1,
        cpl_misc_target2,
    ) = (
        gs.players[nr].cpl.attack_cn,
        gs.players[nr].cpl.goto_x,
        gs.players[nr].cpl.goto_y,
        gs.players[nr].cpl.misc_action,
        gs.players[nr].cpl.misc_target1,
        gs.players[nr].cpl.misc_target2,
    );

    if attack_cn as i32 != cpl_attack_cn
        || goto_x as i32 != cpl_goto_x
        || goto_y as i32 != cpl_goto_y
        || misc_action as i32 != cpl_misc_action
        || misc_target1 as i32 != cpl_misc_target1
        || misc_target2 as i32 != cpl_misc_target2
    {
        let mut buf: [u8; 16] = [0; 16];
        buf[0] = ServerCommandType::SetTarget as u8;

        // attack_cn (2 bytes)
        buf[1] = attack_cn as u8;
        buf[2] = (attack_cn >> 8) as u8;

        // goto_x (2 bytes)
        buf[3] = goto_x as u8;
        buf[4] = (goto_x >> 8) as u8;

        // goto_y (2 bytes)
        buf[5] = goto_y as u8;
        buf[6] = (goto_y >> 8) as u8;

        // misc_action (2 bytes)
        buf[7] = misc_action as u8;
        buf[8] = (misc_action >> 8) as u8;

        // misc_target1 (2 bytes)
        buf[9] = misc_target1 as u8;
        buf[10] = (misc_target1 >> 8) as u8;

        // misc_target2 (2 bytes)
        buf[11] = misc_target2 as u8;
        buf[12] = (misc_target2 >> 8) as u8;

        network_manager::xsend(gs, nr, &buf, 13);

        gs.players[nr].cpl.attack_cn = attack_cn as i32;
        gs.players[nr].cpl.goto_x = goto_x as i32;
        gs.players[nr].cpl.goto_y = goto_y as i32;
        gs.players[nr].cpl.misc_action = misc_action as i32;
        gs.players[nr].cpl.misc_target1 = misc_target1 as i32;
        gs.players[nr].cpl.misc_target2 = misc_target2 as i32;

        log::debug!("plr_change_target: misc_action={}", misc_action);
    }
}

/// Port of `plr_tick` from `svr_tick.cpp`
/// Handles player tick processing (lag detection and stoning)
pub fn plr_tick(gs: &mut GameState, nr: usize) {
    // Increment local tick counter
    gs.players[nr].ltick = gs.players[nr].ltick.wrapping_add(1);

    let (state, cn) = (gs.players[nr].state, gs.players[nr].usnr);

    if state != core::constants::ST_NORMAL {
        return;
    }

    if cn == 0 {
        return;
    }

    // Check lag-based stoning conditions
    let (data_19, flags) = (gs.characters[cn].data[19], gs.characters[cn].flags);

    let is_player = (flags & CharacterFlags::Player.bits()) != 0;
    let is_stoned = (flags & CharacterFlags::Stoned.bits()) != 0;

    if data_19 == 0 || !is_player {
        return;
    }

    let (ltick, rtick) = (gs.players[nr].ltick, gs.players[nr].rtick);

    // Check if player should be stoned due to lag
    if ltick > rtick.wrapping_add(data_19 as u32) && !is_stoned {
        let name = gs.characters[cn].get_name().to_string();
        log::info!(
            "Character '{}' turned to stone due to lag ({:.2}s)",
            name,
            (ltick.wrapping_sub(rtick)) as f64 / TICKS as f64
        );
        gs.characters[cn].flags |= CharacterFlags::Stoned.bits();
        stone_gc(gs, cn, true);
    }
    // Check if player should be unstoned (lag gone)
    else if ltick
        < rtick
            .wrapping_add(data_19 as u32)
            .wrapping_sub(TICKS as u32)
        && is_stoned
    {
        let name = gs.characters[cn].get_name().to_string();
        log::info!("Character '{}' unstoned, lag is gone", name);
        gs.characters[cn].flags &= !CharacterFlags::Stoned.bits();
        stone_gc(gs, cn, false);
    }
}

/// Port of `stone_gc` from `svr_tick.cpp`
/// Handles stoning/unstoning of linked characters (e.g., usurped characters)
fn stone_gc(gs: &mut GameState, cn: usize, mode: bool) {
    let is_player = (gs.characters[cn].flags & CharacterFlags::Player.bits()) != 0;
    let co = gs.characters[cn].data[64] as usize;

    if !is_player {
        return;
    }

    if co == 0 {
        return;
    }

    // Check if co is a valid active character
    let is_valid = co < core::constants::MAXCHARS
        && gs.characters[co].used == core::constants::USE_ACTIVE
        && gs.characters[co].data[63] == cn as i32;

    if !is_valid {
        return;
    }

    if mode {
        gs.characters[co].flags |= CharacterFlags::Stoned.bits();
    } else {
        gs.characters[co].flags &= !CharacterFlags::Stoned.bits();
    }
}

/// Port of `plr_idle` from `svr_tick.cpp`
/// Handles idle timeout checking for players
pub fn plr_idle(gs: &mut GameState, nr: usize) {
    let ticker = gs.globals.ticker as u32;
    let (lasttick, lasttick2, state, usnr) = (
        gs.players[nr].lasttick,
        gs.players[nr].lasttick2,
        gs.players[nr].state,
        gs.players[nr].usnr,
    );

    // Check protocol level idle (60 seconds)
    if ticker.wrapping_sub(lasttick) > (core::constants::TICKS * 60) as u32 {
        log::info!("Player {} idle too long (protocol level)", nr);
        plr_logout(gs, usnr, nr, LogoutReason::IdleTooLong);
    }

    if state == core::constants::ST_EXIT {
        return;
    }

    // Check player level idle (15 minutes)
    if ticker.wrapping_sub(lasttick2) > (core::constants::TICKS * 60 * 15) as u32 {
        log::info!("Player {} idle too long (player level)", nr);
        plr_logout(gs, usnr, nr, LogoutReason::IdleTooLong);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{TcpListener, TcpStream};

    use crate::{
        test_helpers::{add_test_player, with_test_gs},
        tls::GameStream,
    };
    use core::{
        constants::{
            CharacterFlags, ItemFlags, MAX_SPEEDTAB_SPEED_INDEX, ST_CAP, ST_CONNECT, ST_EXIT,
            ST_LOGIN, ST_NEWCAP, ST_NEWLOGIN, ST_NORMAL, USE_ACTIVE, USE_NONACTIVE,
        },
        string_operations::write_ascii_into_fixed,
        types::Map,
    };

    fn attach_test_socket(gs: &mut GameState, nr: usize) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind test listener");
        let addr = listener.local_addr().expect("listener addr");
        let client = TcpStream::connect(addr).expect("connect client");
        let (server, _) = listener.accept().expect("accept client");
        drop(client);
        gs.players[nr].sock = Some(GameStream::Plain(server));
    }

    fn reset_tbuf(gs: &mut GameState, nr: usize) {
        gs.players[nr].tptr = 0;
        gs.players[nr].tbuf.fill(0);
    }

    fn map_index(x: i16, y: i16) -> usize {
        x as usize + y as usize * core::constants::SERVER_MAPX as usize
    }

    fn setup_existing_character(gs: &mut GameState, cn: usize, player: i32, used: u8, name: &str) {
        gs.characters[cn] = core::types::Character::default();
        gs.characters[cn].used = used;
        gs.characters[cn].player = player;
        gs.characters[cn].x = 10;
        gs.characters[cn].y = 10;
        gs.characters[cn].tox = 10;
        gs.characters[cn].toy = 10;
        gs.characters[cn].frx = 10;
        gs.characters[cn].fry = 10;
        gs.characters[cn].tavern_x = 10;
        gs.characters[cn].tavern_y = 10;
        write_ascii_into_fixed(&mut gs.characters[cn].name, name);
        write_ascii_into_fixed(&mut gs.characters[cn].reference, name);
        gs.map[map_index(10, 10)].ch = cn as u32;
    }

    fn seed_character_template(gs: &mut GameState, template_id: usize) {
        gs.character_templates[template_id] = core::types::Character::default();
        gs.character_templates[template_id].used = USE_ACTIVE;
        gs.character_templates[template_id].mode = 1;
        gs.character_templates[template_id].x = 10;
        gs.character_templates[template_id].y = 10;
        gs.character_templates[template_id].tox = 10;
        gs.character_templates[template_id].toy = 10;
    }

    fn active_tick_for_speed(speed: usize) -> usize {
        SPEEDTAB[speed]
            .iter()
            .position(|value| *value != 0)
            .expect("speed has active tick")
    }

    #[test]
    fn speedo_clamps_speed_and_uses_cycle_index() {
        with_test_gs(|gs| {
            let (cn, _) = add_test_player(gs);
            let ctick = 3usize;
            gs.characters[cn].speed = (MAX_SPEEDTAB_SPEED_INDEX + 20) as i16;
            gs.globals.ticker = (core::constants::CTICK_CYCLE_LEN + ctick) as i32;

            assert_eq!(
                speedo(gs, cn),
                SPEEDTAB[MAX_SPEEDTAB_SPEED_INDEX][ctick] as i32
            );
        });
    }

    #[test]
    fn player_driver_med_respects_cooldown_and_runs_follow_driver_when_due() {
        with_test_gs(|gs| {
            let (cn, _) = add_test_player(gs);
            let leader = 2usize;
            setup_existing_character(gs, leader, 0, USE_ACTIVE, "Leader");
            gs.characters[leader].tox = 10;
            gs.characters[leader].toy = 10;
            gs.characters[leader].dir = core::constants::DX_UP;

            gs.characters[cn].temp = core::constants::CT_COMPANION as u16;
            gs.characters[cn].data[63] = leader as i32;
            gs.characters[cn].x = 10;
            gs.characters[cn].y = 12;
            gs.characters[cn].tox = 10;
            gs.characters[cn].toy = 12;
            gs.characters[cn].dir = core::constants::DX_UP;
            gs.map[map_index(10, 12)].ch = cn as u32;
            gs.characters[cn].data[10] = leader as i32;
            gs.characters[cn].data[12] = 1;
            gs.characters[cn].misc_action = 777;
            gs.globals.ticker = TICKS * 15;

            player_driver_med(gs, cn);
            assert_eq!(gs.characters[cn].misc_action, 777);

            gs.characters[cn].data[12] = 0;
            player_driver_med(gs, cn);
            assert_eq!(
                gs.characters[cn].misc_action,
                core::constants::DR_IDLE as u16
            );
        });
    }

    #[test]
    fn plr_act_short_circuits_and_resets_unknown_status() {
        with_test_gs(|gs| {
            let (cn, _) = add_test_player(gs);
            let active_tick = active_tick_for_speed(10);
            gs.characters[cn].speed = 10;
            gs.characters[cn].status = 23;
            gs.characters[cn].stunned = 1;
            gs.map[map_index(10, 10)].ch = cn as u32;
            gs.globals.ticker = active_tick as i32;
            plr_act(gs, cn);
            assert_eq!(gs.characters[cn].status, 23);
            assert_eq!(gs.characters[cn].y, 10);

            gs.characters[cn].stunned = 0;
            gs.characters[cn].flags |= CharacterFlags::Stoned.bits();
            plr_act(gs, cn);
            assert_eq!(gs.characters[cn].status, 23);
            assert_eq!(gs.characters[cn].y, 10);

            gs.characters[cn].flags &= !CharacterFlags::Stoned.bits();
            gs.characters[cn].status = 255;
            plr_act(gs, cn);
            assert_eq!(gs.characters[cn].status, 0);
        });
    }

    #[test]
    fn plr_act_advances_and_executes_move_turn_and_misc_branches() {
        with_test_gs(|gs| {
            let (cn, _) = add_test_player(gs);
            let active_tick = active_tick_for_speed(10);
            gs.characters[cn].speed = 10;
            gs.globals.ticker = active_tick as i32;

            gs.characters[cn].status = 16;
            plr_act(gs, cn);
            assert_eq!(gs.characters[cn].status, 17);

            gs.characters[cn].status = 23;
            gs.characters[cn].x = 10;
            gs.characters[cn].y = 10;
            gs.characters[cn].tox = 10;
            gs.characters[cn].toy = 10;
            gs.map[map_index(10, 10)].ch = cn as u32;
            plr_act(gs, cn);
            assert_eq!(gs.characters[cn].y, 9);
            assert_ne!(gs.characters[cn].status, 23);

            gs.characters[cn].status = 135;
            gs.characters[cn].dir = core::constants::DX_DOWN;
            plr_act(gs, cn);
            assert_eq!(gs.characters[cn].dir, core::constants::DX_UP);
            assert_ne!(gs.characters[cn].status, 135);

            gs.characters[cn].status = 167;
            gs.characters[cn].status2 = 255;
            gs.characters[cn].cerrno = 0;
            plr_act(gs, cn);
            assert_ne!(gs.characters[cn].status, 167);
            assert_eq!(gs.characters[cn].cerrno, core::constants::ERR_FAILED as u16);
        });
    }

    #[test]
    fn plr_state_handles_timeouts_and_cap_transitions() {
        with_test_gs(|gs| {
            let (_, nr) = add_test_player(gs);
            attach_test_socket(gs, nr);
            gs.players[nr].state = ST_EXIT;
            gs.players[nr].lasttick = 0;
            gs.globals.ticker = TICKS * 16;

            plr_state(gs, nr);
            assert!(gs.players[nr].sock.is_none());
        });

        with_test_gs(|gs| {
            let (_, nr) = add_test_player(gs);
            gs.players[nr].state = ST_NEWCAP;
            gs.players[nr].lasttick = 0;
            gs.globals.ticker = TICKS * 11;
            plr_state(gs, nr);
            assert_eq!(gs.players[nr].state, ST_NEWLOGIN);

            gs.players[nr].state = ST_CAP;
            gs.players[nr].lasttick = 0;
            plr_state(gs, nr);
            assert_eq!(gs.players[nr].state, ST_LOGIN);
        });

        with_test_gs(|gs| {
            let (_, nr) = add_test_player(gs);
            attach_test_socket(gs, nr);
            gs.players[nr].state = ST_CONNECT;
            gs.players[nr].lasttick = 0;
            gs.globals.ticker = TICKS * 61;

            plr_state(gs, nr);
            assert_eq!(gs.players[nr].state, ST_EXIT);
        });
    }

    #[test]
    fn plr_state_dispatches_newlogin_and_login_handlers() {
        with_test_gs(|gs| {
            let (_, nr) = add_test_player(gs);
            attach_test_socket(gs, nr);
            seed_character_template(gs, 2);
            gs.players[nr].state = ST_NEWLOGIN;
            gs.players[nr].version = core::constants::VERSION as i32;
            gs.players[nr].race = 2;

            plr_state(gs, nr);

            assert_eq!(gs.players[nr].state, ST_NORMAL);
            assert!(gs.players[nr].usnr > 0);
        });

        with_test_gs(|gs| {
            let (_, nr) = add_test_player(gs);
            attach_test_socket(gs, nr);
            setup_existing_character(gs, 2, 0, USE_NONACTIVE, "LoginTarget");
            gs.characters[2].pass1 = 111;
            gs.characters[2].pass2 = 222;
            gs.players[nr].state = ST_LOGIN;
            gs.players[nr].version = core::constants::VERSION as i32;
            gs.players[nr].usnr = 2;
            gs.players[nr].pass1 = 111;
            gs.players[nr].pass2 = 222;

            plr_state(gs, nr);

            assert_eq!(gs.players[nr].state, ST_NORMAL);
            assert_eq!(gs.characters[2].player, nr as i32);
        });
    }

    #[test]
    fn plr_change_stats_updates_client_mirrors_and_clears_update_flags() {
        with_test_gs(|gs| {
            let (cn, nr) = add_test_player(gs);
            attach_test_socket(gs, nr);
            write_ascii_into_fixed(&mut gs.characters[cn].name, "TickTester");
            gs.characters[cn].temp = 42;
            gs.characters[cn].mode = 2;
            gs.characters[cn].attrib[0] = [1, 2, 3, 4, 5, 6];
            gs.characters[cn].hp = [10, 11, 12, 13, 14, 15];
            gs.characters[cn].end = [20, 21, 22, 23, 24, 25];
            gs.characters[cn].mana = [30, 31, 32, 33, 34, 35];
            gs.characters[cn].skill[3] = [7, 8, 9, 10, 11, 12];

            gs.characters[cn].item[0] = 10;
            gs.items[10].used = USE_ACTIVE;
            gs.items[10].active = 0;
            gs.items[10].sprite[0] = 77;
            gs.items[10].placement = 5;
            gs.items[10].flags = ItemFlags::IF_UPDATE.bits();

            gs.characters[cn].worn[0] = 11;
            gs.items[11].used = USE_ACTIVE;
            gs.items[11].active = 1;
            gs.items[11].sprite[1] = 88;
            gs.items[11].placement = 6;
            gs.items[11].flags = ItemFlags::IF_UPDATE.bits();

            gs.characters[cn].spell[0] = 12;
            gs.items[12].used = USE_ACTIVE;
            gs.items[12].sprite[1] = 99;
            gs.items[12].active = 8;
            gs.items[12].duration = 16;
            gs.items[12].flags = ItemFlags::IF_UPDATE.bits();

            gs.characters[cn].citem = 13;
            gs.items[13].used = USE_ACTIVE;
            gs.items[13].active = 0;
            gs.items[13].sprite[0] = 111;
            gs.items[13].placement = 7;
            gs.items[13].flags = ItemFlags::IF_UPDATE.bits();

            plr_change_stats(gs, nr, cn, 0);

            assert_eq!(
                gs.players[nr].tbuf[0],
                ServerCommandType::SetCharName1 as u8
            );
            assert!(gs.players[nr].tptr > 0);
            assert_eq!(gs.players[nr].cpl.name, gs.characters[cn].name);
            assert_eq!(gs.players[nr].cpl.mode, 2);
            assert_eq!(gs.players[nr].cpl.attrib[0], [1, 2, 3, 4, 5, 6]);
            assert_eq!(gs.players[nr].cpl.hp, gs.characters[cn].hp);
            assert_eq!(gs.players[nr].cpl.end, gs.characters[cn].end);
            assert_eq!(gs.players[nr].cpl.mana, gs.characters[cn].mana);
            assert_eq!(gs.players[nr].cpl.skill[3], gs.characters[cn].skill[3]);
            assert_eq!(gs.players[nr].cpl.item[0], 10);
            assert_eq!(gs.players[nr].cpl.worn[0], 11);
            assert_eq!(gs.players[nr].cpl.spell[0], 12);
            assert_eq!(gs.players[nr].cpl.active[0], 8);
            assert_eq!(gs.players[nr].cpl.citem, 13);
            assert_eq!(gs.items[10].flags & ItemFlags::IF_UPDATE.bits(), 0);
            assert_eq!(gs.items[11].flags & ItemFlags::IF_UPDATE.bits(), 0);
            assert_eq!(gs.items[12].flags & ItemFlags::IF_UPDATE.bits(), 0);
            assert_eq!(gs.items[13].flags & ItemFlags::IF_UPDATE.bits(), 0);
        });
    }

    #[test]
    fn plr_change_stats_handles_building_item_and_gold_cursor_encodings() {
        with_test_gs(|gs| {
            let (cn, nr) = add_test_player(gs);
            attach_test_socket(gs, nr);
            gs.players[nr].cpl.name = gs.characters[cn].name;

            gs.characters[cn].flags |= CharacterFlags::BuildMode.bits();
            gs.characters[cn].item[0] = 0x40000000u32 | core::constants::MF_TAVERN;
            plr_change_stats(gs, nr, cn, 0);
            assert_eq!(gs.players[nr].cpl.item[0], gs.characters[cn].item[0] as i32);
            assert_eq!(gs.players[nr].tbuf[0], ServerCommandType::SetCharItem as u8);
            assert_eq!(gs.players[nr].tbuf[5], 53);

            reset_tbuf(gs, nr);
            gs.characters[cn].flags &= !CharacterFlags::BuildMode.bits();
            gs.characters[cn].item[0] = 0;
            gs.players[nr].cpl.item[0] = 0;
            gs.players[nr].cpl.citem = 0;
            gs.characters[cn].citem = 0x80000064u32;
            plr_change_stats(gs, nr, cn, 0);
            assert_eq!(gs.players[nr].tbuf[0], ServerCommandType::SetCharObj as u8);
            assert_eq!(gs.players[nr].tbuf[1], 39);
        });
    }

    #[test]
    fn scalar_change_helpers_emit_expected_packets() {
        with_test_gs(|gs| {
            let (cn, nr) = add_test_player(gs);
            attach_test_socket(gs, nr);

            gs.characters[cn].a_hp = 2500;
            plr_change_hp(gs, nr, cn);
            assert_eq!(
                gs.players[nr].tbuf[..3],
                [ServerCommandType::SetCharHp as u8, 3, 0]
            );
            assert_eq!(gs.players[nr].cpl.a_hp, 3);

            reset_tbuf(gs, nr);
            gs.characters[cn].a_end = 3500;
            plr_change_end(gs, nr, cn);
            assert_eq!(
                gs.players[nr].tbuf[..3],
                [ServerCommandType::SetCharEndur as u8, 4, 0]
            );
            assert_eq!(gs.players[nr].cpl.a_end, 4);

            reset_tbuf(gs, nr);
            gs.characters[cn].a_mana = 4500;
            plr_change_mana(gs, nr, cn);
            assert_eq!(
                gs.players[nr].tbuf[..3],
                [ServerCommandType::SetCharMana as u8, 5, 0]
            );
            assert_eq!(gs.players[nr].cpl.a_mana, 5);

            reset_tbuf(gs, nr);
            gs.characters[cn].dir = core::constants::DX_LEFT;
            plr_change_dir(gs, nr, cn);
            assert_eq!(
                gs.players[nr].tbuf[..2],
                [
                    ServerCommandType::SetCharDir as u8,
                    core::constants::DX_LEFT
                ]
            );
            assert_eq!(gs.players[nr].cpl.dir, core::constants::DX_LEFT as i32);

            reset_tbuf(gs, nr);
            gs.characters[cn].points = 5;
            gs.characters[cn].points_tot = 10;
            gs.characters[cn].kindred = 15;
            plr_change_points(gs, nr, cn);
            assert_eq!(gs.players[nr].tbuf[0], ServerCommandType::SetCharPts as u8);
            assert_eq!(gs.players[nr].cpl.points, 5);
            assert_eq!(gs.players[nr].cpl.points_tot, 10);
            assert_eq!(gs.players[nr].cpl.kindred, 15);

            reset_tbuf(gs, nr);
            gs.characters[cn].gold = 1234;
            gs.characters[cn].armor = 9;
            gs.characters[cn].weapon = 11;
            plr_change_gold(gs, nr, cn);
            assert_eq!(gs.players[nr].tbuf[0], ServerCommandType::SetCharGold as u8);
            assert_eq!(gs.players[nr].cpl.gold, 1234);
            assert_eq!(gs.players[nr].cpl.armor, 9);
            assert_eq!(gs.players[nr].cpl.weapon, 11);
        });
    }

    #[test]
    fn plr_change_load_and_target_emit_expected_packets() {
        with_test_gs(|gs| {
            let (cn, nr) = add_test_player(gs);
            attach_test_socket(gs, nr);
            gs.characters[cn].flags |= CharacterFlags::God.bits();
            gs.globals.load = 77;

            plr_change_load(gs, nr, cn, 32);
            assert_eq!(gs.players[nr].tbuf[0], ServerCommandType::Load as u8);
            assert_eq!(
                u32::from_le_bytes(gs.players[nr].tbuf[1..5].try_into().unwrap()),
                77
            );

            reset_tbuf(gs, nr);
            gs.characters[cn].attack_cn = 2;
            gs.characters[cn].goto_x = 11;
            gs.characters[cn].goto_y = 12;
            gs.characters[cn].misc_action = 13;
            gs.characters[cn].misc_target1 = 14;
            gs.characters[cn].misc_target2 = 15;
            plr_change_target(gs, nr, cn);
            assert_eq!(gs.players[nr].tbuf[0], ServerCommandType::SetTarget as u8);
            assert_eq!(gs.players[nr].cpl.attack_cn, 2);
            assert_eq!(gs.players[nr].cpl.goto_x, 11);
            assert_eq!(gs.players[nr].cpl.goto_y, 12);
            assert_eq!(gs.players[nr].cpl.misc_action, 13);
            assert_eq!(gs.players[nr].cpl.misc_target1, 14);
            assert_eq!(gs.players[nr].cpl.misc_target2, 15);
        });
    }

    #[test]
    fn plr_change_validates_character_and_runs_update_cycle() {
        with_test_gs(|gs| {
            let (_, nr) = add_test_player(gs);
            attach_test_socket(gs, nr);
            gs.players[nr].usnr = core::constants::MAXCHARS;

            plr_change(gs, nr);
            assert_eq!(gs.players[nr].tptr, 0);
        });

        with_test_gs(|gs| {
            let (cn, nr) = add_test_player(gs);
            attach_test_socket(gs, nr);
            write_ascii_into_fixed(&mut gs.characters[cn].name, "CycleName");
            gs.characters[cn].flags |= CharacterFlags::Update.bits();
            gs.characters[cn].mode = 1;
            gs.characters[cn].a_hp = 2500;
            gs.characters[cn].dir = core::constants::DX_RIGHT;
            gs.characters[cn].attack_cn = 3;
            gs.characters[cn].goto_x = 14;
            gs.characters[cn].goto_y = 15;
            gs.players[nr].cpl.x = gs.characters[cn].x as i32;
            gs.players[nr].cpl.y = gs.characters[cn].y as i32;
            gs.players[nr].smap = gs.players[nr].cmap;
            gs.players[nr].xmap.fill(Map::default());
            let tile = map_index(gs.characters[cn].x, gs.characters[cn].y);
            gs.map[tile] = gs.players[nr].xmap[0];

            plr_change(gs, nr);

            assert!(gs.players[nr].tptr > 0);
            assert_eq!(gs.players[nr].cpl.name, gs.characters[cn].name);
            assert_eq!(gs.players[nr].cpl.a_hp, 3);
            assert_eq!(gs.players[nr].cpl.dir, core::constants::DX_RIGHT as i32);
            assert_eq!(gs.players[nr].cpl.attack_cn, 3);
            assert_eq!(gs.players[nr].cpl.goto_x, 14);
            assert_eq!(gs.players[nr].cpl.goto_y, 15);
        });
    }

    #[test]
    fn plr_tick_and_stone_gc_stone_and_unstone_linked_characters() {
        with_test_gs(|gs| {
            let (cn, nr) = add_test_player(gs);
            let co = 2usize;
            setup_existing_character(gs, co, 0, USE_ACTIVE, "Follower");
            gs.characters[co].data[63] = cn as i32;
            gs.characters[cn].data[64] = co as i32;
            gs.players[nr].state = ST_NORMAL;
            gs.players[nr].usnr = cn;
            gs.players[nr].ltick = 20;
            gs.players[nr].rtick = 0;
            gs.characters[cn].data[19] = 5;

            plr_tick(gs, nr);

            assert_ne!(gs.characters[cn].flags & CharacterFlags::Stoned.bits(), 0);
            assert_ne!(gs.characters[co].flags & CharacterFlags::Stoned.bits(), 0);

            gs.characters[cn].flags |= CharacterFlags::Stoned.bits();
            gs.characters[co].flags |= CharacterFlags::Stoned.bits();
            gs.players[nr].ltick = 0;
            gs.players[nr].rtick = 100;
            gs.characters[cn].data[19] = 100;

            plr_tick(gs, nr);

            assert_eq!(gs.characters[cn].flags & CharacterFlags::Stoned.bits(), 0);
            assert_eq!(gs.characters[co].flags & CharacterFlags::Stoned.bits(), 0);
        });

        with_test_gs(|gs| {
            let (cn, _) = add_test_player(gs);
            let co = 2usize;
            setup_existing_character(gs, co, 0, USE_ACTIVE, "Follower");
            gs.characters[co].data[63] = 999;
            gs.characters[cn].data[64] = co as i32;

            stone_gc(gs, cn, true);
            assert_eq!(gs.characters[co].flags & CharacterFlags::Stoned.bits(), 0);
        });
    }

    #[test]
    fn plr_idle_logs_out_for_protocol_and_player_level_timeouts() {
        with_test_gs(|gs| {
            let (cn, nr) = add_test_player(gs);
            attach_test_socket(gs, nr);
            gs.globals.ticker = TICKS * 61;
            gs.players[nr].lasttick = 0;
            gs.players[nr].lasttick2 = 0;

            plr_idle(gs, nr);
            assert_eq!(gs.players[nr].state, ST_EXIT);
            assert_eq!(gs.characters[cn].player, 0);
        });

        with_test_gs(|gs| {
            let (_, nr) = add_test_player(gs);
            attach_test_socket(gs, nr);
            gs.players[nr].state = ST_NORMAL;
            gs.players[nr].lasttick = gs.globals.ticker as u32;
            gs.players[nr].lasttick2 = 0;
            gs.globals.ticker = TICKS * 60 * 16;

            plr_idle(gs, nr);
            assert_eq!(gs.players[nr].state, ST_EXIT);
        });
    }
}
