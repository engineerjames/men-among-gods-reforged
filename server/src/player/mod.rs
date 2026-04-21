pub mod commands;
pub mod connection;
pub mod map;
pub mod tick;

/// Port of `plr_cmd` from `svr_tick.cpp`
/// Dispatches player commands from inbuf
pub fn plr_cmd(gs: &mut GameState, nr: usize) {
    let cmd = gs.players[nr].inbuf[0];

    let parsed_cmd = ClientCommandType::from(cmd);

    // Handle pre-login commands (mirrors the initial switch in the original C++).
    // These generally transition connection state; only `CL_CMD_UNIQUE` returns
    // immediately in the original code.
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
        _ => {
            // No need to log other commands here; they are logged in their handlers.
        }
    }

    // Game state may have changed in the handlers above.
    let state = gs.players[nr].state;

    // Only process other commands if in normal state
    if state != core::constants::ST_NORMAL {
        return;
    }

    // Update lasttick2 for non-automated commands
    if parsed_cmd != ClientCommandType::CmdAutoLook
        && parsed_cmd != ClientCommandType::PerfReport
        && parsed_cmd != ClientCommandType::CmdCTick
        && parsed_cmd != ClientCommandType::Ping
    {
        let ticker = gs.globals.ticker as u32;
        gs.players[nr].lasttick2 = ticker;
    }

    // Handle commands that don't require stun check
    match parsed_cmd {
        ClientCommandType::PerfReport => {
            plr_perf_report(gs, nr);
            return;
        }
        ClientCommandType::Ping => {
            plr_cmd_ping(gs, nr);
            return;
        }
        ClientCommandType::CmdLook => {
            log::debug!("PLR_CMD_LOOK received for player {}", nr);
            plr_cmd_look(gs, nr, false);
            return;
        }
        ClientCommandType::CmdAutoLook => {
            // Don't log auto commands to reduce log spam
            plr_cmd_look(gs, nr, true);
            return;
        }
        ClientCommandType::CmdSetUser => {
            log::debug!("PLR_CMD_SETUSER received for player {}", nr);
            plr_cmd_setuser(gs, nr);
            return;
        }
        ClientCommandType::CmdStat => {
            log::debug!("PLR_CMD_STAT received for player {}", nr);
            plr_cmd_stat(gs, nr);
            return;
        }
        ClientCommandType::CmdInput1 => {
            plr_cmd_input(gs, nr, 1);
            return;
        }
        ClientCommandType::CmdInput2 => {
            plr_cmd_input(gs, nr, 2);
            return;
        }
        ClientCommandType::CmdInput3 => {
            plr_cmd_input(gs, nr, 3);
            return;
        }
        ClientCommandType::CmdInput4 => {
            plr_cmd_input(gs, nr, 4);
            return;
        }
        ClientCommandType::CmdInput5 => {
            plr_cmd_input(gs, nr, 5);
            return;
        }
        ClientCommandType::CmdInput6 => {
            plr_cmd_input(gs, nr, 6);
            return;
        }
        ClientCommandType::CmdInput7 => {
            plr_cmd_input(gs, nr, 7);
            return;
        }
        ClientCommandType::CmdInput8 => {
            plr_cmd_input(gs, nr, 8);
            return;
        }
        ClientCommandType::CmdCTick => {
            plr_cmd_ctick(gs, nr);
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

    // Handle commands that show stun message but still execute
    match parsed_cmd {
        ClientCommandType::CmdLookItem => {
            log::debug!("PLR_CMD_LOOK_ITEM received for player {}", character_name);
            plr_cmd_look_item(gs, nr);
            return;
        }
        ClientCommandType::CmdGive => {
            log::debug!("PLR_CMD_GIVE received for player {}", character_name);
            plr_cmd_give(gs, nr);
            return;
        }
        ClientCommandType::CmdTurn => {
            log::debug!("PLR_CMD_TURN received for player {}", character_name);
            plr_cmd_turn(gs, nr);
            return;
        }
        ClientCommandType::CmdDrop => {
            log::debug!("PLR_CMD_DROP received for player {}", character_name);
            plr_cmd_drop(gs, nr);
            return;
        }
        ClientCommandType::CmdPickup => {
            log::debug!("PLR_CMD_PICKUP received for player {}", character_name);
            plr_cmd_pickup(gs, nr);
            return;
        }
        ClientCommandType::CmdAttack => {
            log::debug!("PLR_CMD_ATTACK received for player {}", character_name);
            plr_cmd_attack(gs, nr);
            return;
        }
        ClientCommandType::CmdMode => {
            log::debug!("PLR_CMD_MODE received for player {}", character_name);
            plr_cmd_mode(gs, nr);
            return;
        }
        ClientCommandType::CmdMove => {
            log::debug!("PLR_CMD_MOVE received for player {}", character_name);
            plr_cmd_move(gs, nr);
            return;
        }
        ClientCommandType::CmdReset => {
            log::debug!("PLR_CMD_RESET received for player {}", character_name);
            plr_cmd_reset(gs, nr);
            return;
        }
        ClientCommandType::CmdSkill => {
            log::debug!("PLR_CMD_SKILL received for player {}", character_name);
            plr_cmd_skill(gs, nr);
            return;
        }
        ClientCommandType::CmdInvLook => {
            log::debug!("PLR_CMD_INV_LOOK received for player {}", character_name);
            plr_cmd_inv_look(gs, nr);
            return;
        }
        ClientCommandType::CmdUse => {
            log::debug!("PLR_CMD_USE received for player {}", character_name);
            plr_cmd_use(gs, nr);
            return;
        }
        ClientCommandType::CmdAutoloot => {
            log::debug!("PLR_CMD_AUTOLOOT received for player {}", character_name);
            plr_cmd_autoloot(gs, nr);
            return;
        }
        ClientCommandType::CmdInv => {
            log::debug!("PLR_CMD_INV received for player {}", character_name);
            plr_cmd_inv(gs, nr);
            return;
        }
        ClientCommandType::CmdExit => {
            log::debug!("PLR_CMD_EXIT received for player {}", character_name);
            plr_cmd_exit(gs, nr);
            return;
        }
        _ => {}
    }

    // Commands blocked by stun
    if is_stunned {
        return;
    }

    match parsed_cmd {
        ClientCommandType::CmdShop => {
            plr_cmd_shop(gs, nr);
        }
        _ => {
            log::warn!("Unknown CL command: {} for player {}", cmd, character_name);
        }
    }
}

/// Notify nearby clients about the character's current tile.
///
/// # Arguments
/// * `gs` - Active game state used for notification dispatch.
/// * `cn` - Character index being announced.
fn notify_character_tile(gs: &mut GameState, cn: usize) {
    let x = gs.characters[cn].x as i32;
    let y = gs.characters[cn].y as i32;
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
