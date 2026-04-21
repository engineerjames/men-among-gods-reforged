use core::{
    client_commands::ClientCommandType,
    constants::{CharacterFlags, TICKS},
    encrypt::xcrypt,
    logout_reasons::LogoutReason,
    server_commands::ServerCommandType,
};

use crate::{game_state::GameState, helpers, network_manager};

use super::{commands, legacy::plr_logout};

/// Port of `plr_tick` from `svr_tick.cpp`
/// Handles player tick processing (lag detection and stoning)
pub fn plr_tick(gs: &mut GameState, nr: usize) {
    gs.players[nr].ltick = gs.players[nr].ltick.wrapping_add(1);

    let (state, cn) = (gs.players[nr].state, gs.players[nr].usnr);

    if state != core::constants::ST_NORMAL {
        return;
    }

    if cn == 0 {
        return;
    }

    let (data_19, flags) = (gs.characters[cn].data[19], gs.characters[cn].flags);

    let is_player = (flags & CharacterFlags::Player.bits()) != 0;
    let is_stoned = (flags & CharacterFlags::Stoned.bits()) != 0;

    if data_19 == 0 || !is_player {
        return;
    }

    let (ltick, rtick) = (gs.players[nr].ltick, gs.players[nr].rtick);

    if ltick > rtick.wrapping_add(data_19 as u32) && !is_stoned {
        let name = gs.characters[cn].get_name().to_string();
        log::info!(
            "Character '{}' turned to stone due to lag ({:.2}s)",
            name,
            (ltick.wrapping_sub(rtick)) as f64 / TICKS as f64
        );
        gs.characters[cn].flags |= CharacterFlags::Stoned.bits();
        stone_gc(gs, cn, true);
    } else if ltick
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

    if ticker.wrapping_sub(lasttick) > (core::constants::TICKS * 60) as u32 {
        log::info!("Player {} idle too long (protocol level)", nr);
        plr_logout(gs, usnr, nr, LogoutReason::IdleTooLong);
    }

    if state == core::constants::ST_EXIT {
        return;
    }

    if ticker.wrapping_sub(lasttick2) > (core::constants::TICKS * 60 * 15) as u32 {
        log::info!("Player {} idle too long (player level)", nr);
        plr_logout(gs, usnr, nr, LogoutReason::IdleTooLong);
    }
}

/// Port of `plr_cmd` from `svr_tick.cpp`
/// Dispatches player commands from inbuf
pub fn plr_cmd(gs: &mut GameState, nr: usize) {
    let cmd = gs.players[nr].inbuf[0];
    let parsed_cmd = ClientCommandType::from(cmd);

    match parsed_cmd {
        ClientCommandType::NewLogin => {
            plr_challenge_newlogin(gs, nr);
        }
        ClientCommandType::Challenge => {
            plr_challenge(gs, nr);
        }
        ClientCommandType::Login => {
            plr_challenge_login(gs, nr);
        }
        ClientCommandType::ApiLogin => {
            plr_challenge_api_login(gs, nr);
        }
        ClientCommandType::CmdUnique => {
            plr_unique(gs, nr);
            return;
        }
        ClientCommandType::Passwd => {
            plr_passwd(gs, nr);
        }
        _ => {}
    }

    let state = gs.players[nr].state;
    if state != core::constants::ST_NORMAL {
        return;
    }

    if parsed_cmd != ClientCommandType::CmdAutoLook
        && parsed_cmd != ClientCommandType::PerfReport
        && parsed_cmd != ClientCommandType::CmdCTick
        && parsed_cmd != ClientCommandType::Ping
    {
        let ticker = gs.globals.ticker as u32;
        gs.players[nr].lasttick2 = ticker;
    }

    match parsed_cmd {
        ClientCommandType::PerfReport => {
            plr_perf_report(gs, nr);
            return;
        }
        ClientCommandType::Ping => {
            commands::plr_cmd_ping(gs, nr);
            return;
        }
        ClientCommandType::CmdLook => {
            log::debug!("PLR_CMD_LOOK received for player {}", nr);
            commands::plr_cmd_look(gs, nr, false);
            return;
        }
        ClientCommandType::CmdAutoLook => {
            commands::plr_cmd_look(gs, nr, true);
            return;
        }
        ClientCommandType::CmdSetUser => {
            log::debug!("PLR_CMD_SETUSER received for player {}", nr);
            commands::plr_cmd_setuser(gs, nr);
            return;
        }
        ClientCommandType::CmdStat => {
            log::debug!("PLR_CMD_STAT received for player {}", nr);
            commands::plr_cmd_stat(gs, nr);
            return;
        }
        ClientCommandType::CmdInput1 => {
            commands::plr_cmd_input(gs, nr, 1);
            return;
        }
        ClientCommandType::CmdInput2 => {
            commands::plr_cmd_input(gs, nr, 2);
            return;
        }
        ClientCommandType::CmdInput3 => {
            commands::plr_cmd_input(gs, nr, 3);
            return;
        }
        ClientCommandType::CmdInput4 => {
            commands::plr_cmd_input(gs, nr, 4);
            return;
        }
        ClientCommandType::CmdInput5 => {
            commands::plr_cmd_input(gs, nr, 5);
            return;
        }
        ClientCommandType::CmdInput6 => {
            commands::plr_cmd_input(gs, nr, 6);
            return;
        }
        ClientCommandType::CmdInput7 => {
            commands::plr_cmd_input(gs, nr, 7);
            return;
        }
        ClientCommandType::CmdInput8 => {
            commands::plr_cmd_input(gs, nr, 8);
            return;
        }
        ClientCommandType::CmdCTick => {
            commands::plr_cmd_ctick(gs, nr);
            return;
        }
        _ => {}
    }

    let cn = gs.players[nr].usnr;
    let is_stunned = gs.characters[cn].stunned > 0;

    if is_stunned {
        gs.do_character_log(
            cn,
            core::types::FontColor::Red,
            "You have been stunned. You cannot move.\n",
        );
    }

    let character_name = gs.characters[cn].get_name().to_string();

    match parsed_cmd {
        ClientCommandType::CmdLookItem => {
            log::debug!("PLR_CMD_LOOK_ITEM received for player {}", character_name);
            commands::plr_cmd_look_item(gs, nr);
            return;
        }
        ClientCommandType::CmdGive => {
            log::debug!("PLR_CMD_GIVE received for player {}", character_name);
            commands::plr_cmd_give(gs, nr);
            return;
        }
        ClientCommandType::CmdTurn => {
            log::debug!("PLR_CMD_TURN received for player {}", character_name);
            commands::plr_cmd_turn(gs, nr);
            return;
        }
        ClientCommandType::CmdDrop => {
            log::debug!("PLR_CMD_DROP received for player {}", character_name);
            commands::plr_cmd_drop(gs, nr);
            return;
        }
        ClientCommandType::CmdPickup => {
            log::debug!("PLR_CMD_PICKUP received for player {}", character_name);
            commands::plr_cmd_pickup(gs, nr);
            return;
        }
        ClientCommandType::CmdAttack => {
            log::debug!("PLR_CMD_ATTACK received for player {}", character_name);
            commands::plr_cmd_attack(gs, nr);
            return;
        }
        ClientCommandType::CmdMode => {
            log::debug!("PLR_CMD_MODE received for player {}", character_name);
            commands::plr_cmd_mode(gs, nr);
            return;
        }
        ClientCommandType::CmdMove => {
            log::debug!("PLR_CMD_MOVE received for player {}", character_name);
            commands::plr_cmd_move(gs, nr);
            return;
        }
        ClientCommandType::CmdReset => {
            log::debug!("PLR_CMD_RESET received for player {}", character_name);
            commands::plr_cmd_reset(gs, nr);
            return;
        }
        ClientCommandType::CmdSkill => {
            log::debug!("PLR_CMD_SKILL received for player {}", character_name);
            commands::plr_cmd_skill(gs, nr);
            return;
        }
        ClientCommandType::CmdInvLook => {
            log::debug!("PLR_CMD_INV_LOOK received for player {}", character_name);
            commands::plr_cmd_inv_look(gs, nr);
            return;
        }
        ClientCommandType::CmdUse => {
            log::debug!("PLR_CMD_USE received for player {}", character_name);
            commands::plr_cmd_use(gs, nr);
            return;
        }
        ClientCommandType::CmdAutoloot => {
            log::debug!("PLR_CMD_AUTOLOOT received for player {}", character_name);
            commands::plr_cmd_autoloot(gs, nr);
            return;
        }
        ClientCommandType::CmdInv => {
            log::debug!("PLR_CMD_INV received for player {}", character_name);
            commands::plr_cmd_inv(gs, nr);
            return;
        }
        ClientCommandType::CmdExit => {
            log::debug!("PLR_CMD_EXIT received for player {}", character_name);
            commands::plr_cmd_exit(gs, nr);
            return;
        }
        _ => {}
    }

    if is_stunned {
        return;
    }

    match parsed_cmd {
        ClientCommandType::CmdShop => {
            commands::plr_cmd_shop(gs, nr);
        }
        _ => {
            log::warn!("Unknown CL command: {} for player {}", cmd, character_name);
        }
    }
}

/// Port of `send_mod` from `svr_tick.cpp`
/// Sends mod data to the client (8 packets of 15 bytes each)
fn send_mod(gs: &mut GameState, nr: usize) {
    let _mod_data: [u8; 120] = [0; 120];

    for n in 0..8u8 {
        let mut buf: [u8; 16] = [0; 16];
        buf[0] = ServerCommandType::Mod1 as u8 + n;
        network_manager::csend(gs, nr, &buf, 16);
    }
}

/// Port of `plr_challenge_newlogin` from `svr_tick.cpp`
fn plr_challenge_newlogin(gs: &mut GameState, nr: usize) {
    let mut tmp = helpers::random_mod(0x3fffffff_u32 - 1) + 1;
    if tmp == 0 {
        tmp = 42;
    }

    let ticker = gs.globals.ticker as u32;

    gs.players[nr].challenge = tmp;
    gs.players[nr].state = core::constants::ST_NEW_CHALLENGE;
    gs.players[nr].lasttick = ticker;

    let mut buf: [u8; 16] = [0; 16];
    buf[0] = ServerCommandType::Challenge as u8;
    buf[1..5].copy_from_slice(&tmp.to_le_bytes());

    network_manager::csend(gs, nr, &buf, 16);

    log::debug!(
        "Player {} challenge_newlogin: sent challenge {:08X}",
        nr,
        tmp
    );

    send_mod(gs, nr);
}

/// Port of `plr_challenge` from `svr_tick.cpp`
fn plr_challenge(gs: &mut GameState, nr: usize) {
    let (challenge, state) = (gs.players[nr].challenge, gs.players[nr].state);

    let response = u32::from_le_bytes([
        gs.players[nr].inbuf[1],
        gs.players[nr].inbuf[2],
        gs.players[nr].inbuf[3],
        gs.players[nr].inbuf[4],
    ]);
    let version = i32::from_le_bytes([
        gs.players[nr].inbuf[5],
        gs.players[nr].inbuf[6],
        gs.players[nr].inbuf[7],
        gs.players[nr].inbuf[8],
    ]);
    let race = i32::from_le_bytes([
        gs.players[nr].inbuf[9],
        gs.players[nr].inbuf[10],
        gs.players[nr].inbuf[11],
        gs.players[nr].inbuf[12],
    ]);

    gs.players[nr].version = version;
    gs.players[nr].race = race;

    log::info!(
        "Player {} challenge: response={:08X}, version={}, race={}",
        nr,
        response,
        version,
        race
    );

    if response != xcrypt(challenge) {
        log::warn!("Player {} challenge failed", nr);
        let usnr = gs.players[nr].usnr;
        plr_logout(gs, usnr, nr, LogoutReason::ChallengeFailed);
        return;
    }

    let ticker = gs.globals.ticker as u32;

    match state {
        state if state == core::constants::ST_NEW_CHALLENGE => {
            gs.players[nr].state = core::constants::ST_NEWLOGIN;
            gs.players[nr].lasttick = ticker;
            log::info!("Player {} login challenge passed for new characters", nr);
        }
        state if state == core::constants::ST_LOGIN_CHALLENGE => {
            gs.players[nr].state = core::constants::ST_LOGIN;
            gs.players[nr].lasttick = ticker;
            log::info!("Player {} login challenge passed", nr);
        }
        state if state == core::constants::ST_CHALLENGE => {
            gs.players[nr].state = core::constants::ST_NORMAL;
            gs.players[nr].lasttick = ticker;
            gs.players[nr].ltick = 0;
            log::info!("Player {} logged in successfully", nr);
        }
        _ => {
            log::warn!(
                "Player {} challenge reply at unexpected state {}",
                nr,
                state
            );
        }
    }

    log::debug!("Player {} challenge ok", nr);
}

/// Handle existing login challenge (port of `plr_challenge_login`).
fn plr_challenge_login(gs: &mut GameState, nr: usize) {
    log::debug!("Player {} challenge_login", nr);

    let mut tmp = helpers::random_mod(0x3fffffff_u32 - 1) + 1;
    if tmp == 0 {
        tmp = 42;
    }

    let ticker = gs.globals.ticker as u32;

    gs.players[nr].challenge = tmp;
    gs.players[nr].state = core::constants::ST_LOGIN_CHALLENGE;
    gs.players[nr].lasttick = ticker;

    let mut buf: [u8; 16] = [0; 16];
    buf[0] = ServerCommandType::Challenge as u8;
    buf[1..5].copy_from_slice(&tmp.to_le_bytes());

    network_manager::csend(gs, nr, &buf, 16);

    log::debug!("Player {} challenge_login: sent challenge {:08X}", nr, tmp);

    let cn = u32::from_le_bytes([
        gs.players[nr].inbuf[1],
        gs.players[nr].inbuf[2],
        gs.players[nr].inbuf[3],
        gs.players[nr].inbuf[4],
    ]) as usize;

    if !(1..core::constants::MAXCHARS).contains(&cn) {
        log::warn!("Player {} sent wrong cn {} in challenge login", nr, cn);
        plr_logout(gs, 0, nr, LogoutReason::ChallengeFailed);
        return;
    }

    let pass1 = u32::from_le_bytes([
        gs.players[nr].inbuf[5],
        gs.players[nr].inbuf[6],
        gs.players[nr].inbuf[7],
        gs.players[nr].inbuf[8],
    ]);
    let pass2 = u32::from_le_bytes([
        gs.players[nr].inbuf[9],
        gs.players[nr].inbuf[10],
        gs.players[nr].inbuf[11],
        gs.players[nr].inbuf[12],
    ]);

    gs.players[nr].usnr = cn;
    gs.players[nr].pass1 = pass1;
    gs.players[nr].pass2 = pass2;
    gs.players[nr].login_ticket = 0;
    gs.players[nr].api_character_id = 0;

    log::info!(
        "Player logged in as character index={} (players index={})",
        cn,
        nr
    );

    send_mod(gs, nr);
}

/// Handle API ticket based login challenge.
fn plr_challenge_api_login(gs: &mut GameState, nr: usize) {
    log::debug!("Player {} challenge_api_login", nr);

    let mut tmp = helpers::random_mod(0x3fffffff_u32 - 1) + 1;
    if tmp == 0 {
        tmp = 42;
    }

    let ticket = u64::from_le_bytes([
        gs.players[nr].inbuf[1],
        gs.players[nr].inbuf[2],
        gs.players[nr].inbuf[3],
        gs.players[nr].inbuf[4],
        gs.players[nr].inbuf[5],
        gs.players[nr].inbuf[6],
        gs.players[nr].inbuf[7],
        gs.players[nr].inbuf[8],
    ]);

    let ticker = gs.globals.ticker as u32;
    gs.players[nr].challenge = tmp;
    gs.players[nr].state = core::constants::ST_LOGIN_CHALLENGE;
    gs.players[nr].lasttick = ticker;
    gs.players[nr].login_ticket = ticket;
    gs.players[nr].usnr = 0;
    gs.players[nr].pass1 = 0;
    gs.players[nr].pass2 = 0;
    gs.players[nr].api_character_id = 0;

    let mut buf: [u8; 16] = [0; 16];
    buf[0] = ServerCommandType::Challenge as u8;
    buf[1..5].copy_from_slice(&tmp.to_le_bytes());
    network_manager::csend(gs, nr, &buf, 16);

    log::info!("Player {} api login challenge issued", nr);

    send_mod(gs, nr);
}

/// Port of `plr_unique` from `svr_tick.cpp`.
fn plr_unique(gs: &mut GameState, nr: usize) {
    let unique = u64::from_le_bytes([
        gs.players[nr].inbuf[1],
        gs.players[nr].inbuf[2],
        gs.players[nr].inbuf[3],
        gs.players[nr].inbuf[4],
        gs.players[nr].inbuf[5],
        gs.players[nr].inbuf[6],
        gs.players[nr].inbuf[7],
        gs.players[nr].inbuf[8],
    ]);

    gs.players[nr].unique = unique;

    log::debug!("Player {} received unique {:016X}", nr, unique);

    if unique == 0 {
        gs.globals.unique = gs.globals.unique.wrapping_add(1);
        let new_unique = gs.globals.unique;

        gs.players[nr].unique = new_unique;

        let mut buf: [u8; 16] = [0; 16];
        buf[0] = ServerCommandType::Unique as u8;
        buf[1..9].copy_from_slice(&new_unique.to_le_bytes());

        network_manager::xsend(gs, nr, &buf, 9);

        log::debug!("Player {} sent unique {:016X}", nr, new_unique);
    }
}

/// Port of `plr_passwd` from `svr_tick.cpp`.
fn plr_passwd(gs: &mut GameState, nr: usize) {
    let src: [u8; 15] = gs.players[nr].inbuf[1..16].try_into().unwrap();
    gs.players[nr].passwd[..15].copy_from_slice(&src);
    gs.players[nr].passwd[15] = 0;

    let mut hash: u32 = 0;
    for n in 0..15 {
        if gs.players[nr].passwd[n] == 0 {
            break;
        }
        hash ^= (gs.players[nr].passwd[n] as u32) << (n * 2);
    }

    log::debug!("Player {} received passwd hash {}", nr, hash);
}

/// Port of `plr_perf_report` from `svr_tick.cpp`.
fn plr_perf_report(gs: &mut GameState, nr: usize) {
    let _ticksize = u16::from_le_bytes([gs.players[nr].inbuf[1], gs.players[nr].inbuf[2]]);
    let _skip = u16::from_le_bytes([gs.players[nr].inbuf[3], gs.players[nr].inbuf[4]]);
    let _idle = u16::from_le_bytes([gs.players[nr].inbuf[5], gs.players[nr].inbuf[6]]);

    let ticker = gs.globals.ticker as u32;
    gs.players[nr].lasttick = ticker;
}
