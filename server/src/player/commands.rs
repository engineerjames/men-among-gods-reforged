use core::{
    logout_reasons::LogoutReason, server_commands::ServerCommandType,
    string_operations::c_string_to_str, traits,
};

use crate::{game_state::GameState, network_manager};

use super::legacy::plr_logout;

/// Port of `plr_cmd_look` from `svr_tick.cpp`
///
/// Handles the client's LOOK command. If the high bit of the supplied id
/// (`co`) is set, the player requested to see a depot slot (bank); otherwise
/// it requests a character/NPC look. Delegates to `do_look_depot` or
/// `do_look_char` on the shared `GameState`.
///
/// # Arguments
/// * `nr` - Player slot index issuing the look
/// * `autoflag` - When true, treat the request as an automatic look
pub(super) fn plr_cmd_look(gs: &mut GameState, nr: usize, autoflag: bool) {
    let co = u16::from_le_bytes([gs.players[nr].inbuf[1], gs.players[nr].inbuf[2]]) as usize;
    let cn = gs.players[nr].usnr;

    if (co & 0x8000) != 0 {
        let depot_slot = co & 0x7fff;
        gs.do_look_depot(cn, depot_slot);
    } else {
        let autoflag_int = if autoflag { 1 } else { 0 };
        gs.do_look_char(cn, co, 0, autoflag_int, 0);
    }
}

/// Handle set user data command.
///
/// Receives chunks of account/profile data from the client (13-byte
/// fragments) and writes them into the character's `text` buffers. When the
/// final chunk is received for the description/name update it performs
/// validation (name legality, uniqueness, description rules) and either
/// commits changes or reports why they were rejected.
///
/// # Arguments
/// * `_nr` - Player slot index sending the data
pub(super) fn plr_cmd_setuser(gs: &mut GameState, _nr: usize) {
    let nr = _nr;
    let subtype = gs.players[nr].inbuf[1];
    let pos = gs.players[nr].inbuf[2] as usize;
    let mut chunk = [0u8; 13];
    chunk.copy_from_slice(&gs.players[nr].inbuf[3..(13 + 3)]);

    if pos > 65 {
        return;
    }

    let cn = gs.players[nr].usnr;

    match subtype {
        0 | 1 => {
            let text_idx = if subtype == 0 { 0 } else { 1 };
            gs.characters[cn].text[text_idx][pos..(13 + pos)].copy_from_slice(&chunk);
        }
        2 => {
            gs.characters[cn].text[2][pos..(13 + pos)].copy_from_slice(&chunk);

            if pos == 65 {
                {
                    let is_new_user = (gs.characters[cn].flags
                        & core::constants::CharacterFlags::NewUser.bits())
                        != 0;
                    let name_bytes = &mut gs.characters[cn].text[0];
                    let name_end = name_bytes
                        .iter()
                        .position(|&c| c == 0)
                        .unwrap_or(name_bytes.len());
                    let should_process_name = name_end > 3 && name_end < 38 && is_new_user;

                    let mut name_flag: i32 = 0;
                    if should_process_name {
                        for b in name_bytes.iter_mut().take(name_end) {
                            if !(b.is_ascii_uppercase() || b.is_ascii_lowercase()) {
                                name_flag = 1;
                                log::warn!(
                                    "plr_cmd_setuser: name contains non-letter char {:02X}",
                                    *b
                                );
                                break;
                            }
                            *b = b.to_ascii_lowercase();
                        }

                        if name_flag == 0 {
                            if name_end > 0 {
                                name_bytes[0] = name_bytes[0].to_ascii_uppercase();
                            }

                            let name_str = c_string_to_str(name_bytes).to_string();

                            if name_str == "Self" {
                                log::warn!("plr_cmd_setuser: name \"{}\" is reserved", name_str);
                                name_flag = 2;
                            }

                            if name_flag == 0 {
                                for n in 1..core::constants::MAXCHARS {
                                    if n != cn
                                        && gs.characters[n].used != core::constants::USE_EMPTY
                                    {
                                        let mut other_name = gs.characters[n]
                                            .get_name()
                                            .to_string()
                                            .to_ascii_lowercase();

                                        if let Some(first) = other_name.get_mut(0..1) {
                                            first.make_ascii_uppercase();
                                        }

                                        if other_name == name_str {
                                            log::warn!(
                                                "plr_cmd_setuser: name \"{}\" already used by cn={}",
                                                name_str,
                                                n
                                            );
                                            name_flag = 2;
                                            break;
                                        }
                                    }
                                }
                            }

                            if name_flag == 0 {
                                let mut matches_template = false;
                                for t in 1..core::constants::MAXTCHARS {
                                    if gs.character_templates[t].get_name() == name_str {
                                        matches_template = true;
                                        break;
                                    }
                                }

                                if matches_template {
                                    log::warn!(
                                        "plr_cmd_setuser: name \"{}\" matches template name",
                                        name_str
                                    );
                                    name_flag = 2;
                                }
                            }
                        }

                        if name_flag != 0 {
                            let name_str = c_string_to_str(&gs.characters[cn].text[0]).to_string();
                            let reason = if name_flag == 1 {
                                "contains non-letters. Please choose a more normal-looking name."
                                    .to_string()
                            } else if name_flag == 2 {
                                "is already in use. Please try to choose another name.".to_string()
                            } else {
                                "is deemed inappropriate. Please try to choose another name."
                                    .to_string()
                            };

                            gs.do_character_log(
                                cn,
                                core::types::FontColor::Green,
                                &format!(
                                    "The name \"{}\" you have chosen for your character {}\n",
                                    name_str, reason
                                ),
                            );
                        } else {
                            let name_end = gs.characters[cn].text[0]
                                .iter()
                                .position(|&c| c == 0)
                                .unwrap_or(40);
                            for i in 0..40 {
                                gs.characters[cn].name[i] = if i < name_end {
                                    gs.characters[cn].text[0][i]
                                } else {
                                    0
                                };
                                gs.characters[cn].reference[i] = gs.characters[cn].name[i];
                            }
                            gs.characters[cn].flags &=
                                !core::constants::CharacterFlags::NewUser.bits();

                            log::info!(
                                "plr_cmd_setuser: committed name change for cn={} to \"{}\"",
                                cn,
                                gs.characters[cn].get_name()
                            );
                        }
                    }

                    let mut desc = c_string_to_str(&gs.characters[cn].text[1]).to_string();
                    if desc.len() > 77 {
                        let add = c_string_to_str(&gs.characters[cn].text[2]).to_string();
                        desc.push_str(&add);
                    }

                    let mut desc_reason: Option<String> = None;
                    if desc.len() < 10 {
                        desc_reason = Some("is too short".to_string());
                    } else {
                        let name_str = c_string_to_str(&gs.characters[cn].name).to_string();
                        if !desc.contains(&name_str) {
                            desc_reason = Some("does not contain your name".to_string());
                        } else if desc.contains('"') {
                            desc_reason = Some("contains a double quote".to_string());
                        } else if (gs.characters[cn].flags
                            & core::constants::CharacterFlags::NoDesc.bits())
                            != 0
                        {
                            desc_reason = Some("was blocked because you have been known to enter inappropriate descriptions".to_string());
                        }
                    }

                    if let Some(reason) = desc_reason {
                        let race_name = if (gs.characters[cn].kindred & traits::KIN_TEMPLAR as i32)
                            != 0
                        {
                            "a Templar"
                        } else if (gs.characters[cn].kindred & traits::KIN_HARAKIM as i32) != 0 {
                            "a Harakim"
                        } else if (gs.characters[cn].kindred & traits::KIN_MERCENARY as i32) != 0 {
                            "a Mercenary"
                        } else if (gs.characters[cn].kindred & traits::KIN_SEYAN_DU as i32) != 0 {
                            "a Seyan'Du"
                        } else if (gs.characters[cn].kindred & traits::KIN_ARCHHARAKIM as i32) != 0
                        {
                            "an Arch Harakim"
                        } else if (gs.characters[cn].kindred & traits::KIN_ARCHTEMPLAR as i32) != 0
                        {
                            "an Arch Templar"
                        } else if (gs.characters[cn].kindred & traits::KIN_WARRIOR as i32) != 0 {
                            "a Warrior"
                        } else if (gs.characters[cn].kindred & traits::KIN_SORCERER as i32) != 0 {
                            "a Sorcerer"
                        } else {
                            "a strange figure"
                        };

                        gs.do_character_log(
                            cn,
                            core::types::FontColor::Yellow,
                            &format!(
                                "The description you entered for your character {} was rejected.\n",
                                reason
                            ),
                        );

                        let name_str = c_string_to_str(&gs.characters[cn].name).to_string();
                        let pronoun =
                            if (gs.characters[cn].kindred & traits::KIN_FEMALE as i32) != 0 {
                                "She"
                            } else {
                                "He"
                            };
                        let fallback = format!(
                            "{} is {}. {} looks somewhat nondescript.",
                            name_str, race_name, pronoun
                        );
                        let bytes = fallback.as_bytes();
                        for i in 0..200 {
                            gs.characters[cn].description[i] =
                                if i < bytes.len() { bytes[i] } else { 0 };
                        }
                    } else {
                        let bytes = desc.as_bytes();
                        for i in 0..200 {
                            gs.characters[cn].description[i] =
                                if i < bytes.len() { bytes[i] } else { 0 };
                        }
                    }
                    gs.do_character_log(
                        cn,
                        core::types::FontColor::Yellow,
                        "Account data received.\n",
                    );
                    gs.do_update_char(cn);
                }
            }
        }
        _ => {
            log::warn!("Unknown setuser subtype {}", subtype);
        }
    }
}

/// Handle stat change command.
///
/// Applies attribute/HP/endurance/mana/skill raises requested by the
/// client. Validates indices and performs repeated raise operations via
/// `GameState` helpers, then requests a character update.
///
/// # Arguments
/// * `_nr` - Player slot index issuing the stat change
pub(super) fn plr_cmd_stat(gs: &mut GameState, _nr: usize) {
    let cn = gs.players[_nr].usnr;
    let n = u16::from_le_bytes([gs.players[_nr].inbuf[1], gs.players[_nr].inbuf[2]]) as usize;
    let v = u16::from_le_bytes([gs.players[_nr].inbuf[3], gs.players[_nr].inbuf[4]]) as usize;

    if n > 107 || v > 99 {
        return;
    }

    if n < 5 {
        for _ in 0..v {
            let _ = gs.do_raise_attrib(cn, n as i32);
        }
    } else if n == 5 {
        for _ in 0..v {
            let _ = gs.do_raise_hp(cn);
        }
    } else if n == 6 {
        for _ in 0..v {
            let _ = gs.do_raise_end(cn);
        }
    } else if n == 7 {
        for _ in 0..v {
            let _ = gs.do_raise_mana(cn);
        }
    } else {
        for _ in 0..v {
            let _ = gs.do_raise_skill(cn, (n - 8) as i32);
        }
    }

    gs.do_update_char(cn);
}

/// Handle text input commands (1-8).
///
/// Receives a 15-byte chunk of textual input from the client. When the
/// eighth (final) chunk is received the function NUL-terminates the collected
/// input, decodes it to a UTF-8 string, and forwards it to `do_say` for
/// processing as a chat/message.
///
/// # Arguments
/// * `nr` - Player slot index sending the input
/// * `part` - Which 1..8 chunk this call contains
pub(super) fn plr_cmd_input(gs: &mut GameState, nr: usize, part: u8) {
    let offset = ((part - 1) as usize) * 15;
    for n in 0..15 {
        gs.players[nr].input[offset + n] = gs.players[nr].inbuf[1 + n];
    }

    if part == 8 {
        gs.players[nr].input[105 + 14] = 0;

        let cn = gs.players[nr].usnr;
        let raw = gs.players[nr].input.to_vec();
        let text = c_string_to_str(&raw);
        gs.do_say(cn, text);
    }
}

/// Handle client tick update.
///
/// Updates server-side bookkeeping for client timing. Reads `rtick` from the
/// client's inbuf, stores it in `players[nr].rtick`, and refreshes the
/// player's `lasttick` timeout to avoid idle/disconnect handling.
///
/// # Arguments
/// * `nr` - Player slot index sending the tick
pub(super) fn plr_cmd_ctick(gs: &mut GameState, nr: usize) {
    let ticker = gs.globals.ticker as u32;
    let rtick = u32::from_le_bytes([
        gs.players[nr].inbuf[1],
        gs.players[nr].inbuf[2],
        gs.players[nr].inbuf[3],
        gs.players[nr].inbuf[4],
    ]);
    gs.players[nr].rtick = rtick;
    gs.players[nr].lasttick = ticker;
}

/// Handle client ping request.
///
/// Reads `seq` and `client_time_ms` from the client's inbuf and replies with
/// `SV_PONG`, echoing both values back to the client so it can compute RTT.
pub(super) fn plr_cmd_ping(gs: &mut GameState, nr: usize) {
    let seq = u32::from_le_bytes([
        gs.players[nr].inbuf[1],
        gs.players[nr].inbuf[2],
        gs.players[nr].inbuf[3],
        gs.players[nr].inbuf[4],
    ]);
    let client_time_ms = u32::from_le_bytes([
        gs.players[nr].inbuf[5],
        gs.players[nr].inbuf[6],
        gs.players[nr].inbuf[7],
        gs.players[nr].inbuf[8],
    ]);

    let mut buf = [0u8; 16];
    buf[0] = ServerCommandType::Pong as u8;
    buf[1..5].copy_from_slice(&seq.to_le_bytes());
    buf[5..9].copy_from_slice(&client_time_ms.to_le_bytes());

    network_manager::xsend(gs, nr, &buf, 16);
}

/// Handle look at item on ground.
///
/// Reads coordinates from the client's packet, validates them, and if the
/// tile contains an item calls `do_look_item` to present details to the
/// requesting character.
///
/// # Arguments
/// * `nr` - Player slot index issuing the request
pub(super) fn plr_cmd_look_item(gs: &mut GameState, nr: usize) {
    let x = u16::from_le_bytes([gs.players[nr].inbuf[1], gs.players[nr].inbuf[2]]) as i32;
    let y = u16::from_le_bytes([gs.players[nr].inbuf[3], gs.players[nr].inbuf[4]]) as i32;
    let cn = gs.players[nr].usnr;

    if !(0..core::constants::SERVER_MAPX).contains(&x)
        || !(0..core::constants::SERVER_MAPY).contains(&y)
    {
        log::error!("plr_cmd_look_item: cn={} invalid coords {},{}", cn, x, y);
        return;
    }

    let in_idx = gs.map[(x + y * core::constants::SERVER_MAPX) as usize].it as usize;
    gs.do_look_item(cn, in_idx);
}

/// Handle give item command.
///
/// Reads a target character id from the client's packet and sets the
/// giving character's misc action (`DR_GIVE`) and `misc_target1` to
/// perform a give in the next tick.
///
/// # Arguments
/// * `nr` - Player slot index issuing the give
pub(super) fn plr_cmd_give(gs: &mut GameState, nr: usize) {
    let co = u32::from_le_bytes([
        gs.players[nr].inbuf[1],
        gs.players[nr].inbuf[2],
        gs.players[nr].inbuf[3],
        gs.players[nr].inbuf[4],
    ]) as usize;

    if co >= core::constants::MAXCHARS {
        log::error!("plr_cmd_give: invalid target cn {}", co);
        return;
    }

    let cn = gs.players[nr].usnr;
    let ticker = gs.globals.ticker;
    gs.characters[cn].attack_cn = 0;
    gs.characters[cn].goto_x = 0;
    gs.characters[cn].misc_action = core::constants::DR_GIVE as u16;
    gs.characters[cn].misc_target1 = co as u16;
    gs.characters[cn].cerrno = 0;
    gs.characters[cn].data[12] = ticker;
}

/// Handle turn command.
///
/// Reads target coordinates from the client and sets a turn action
/// (`DR_TURN`) so the character will turn toward the specified point on
/// its next action tick. Ignored if the character is in building mode.
///
/// # Arguments
/// * `nr` - Player slot index issuing the turn
pub(super) fn plr_cmd_turn(gs: &mut GameState, nr: usize) {
    let x = u16::from_le_bytes([gs.players[nr].inbuf[1], gs.players[nr].inbuf[2]]) as i32;
    let y = u16::from_le_bytes([gs.players[nr].inbuf[3], gs.players[nr].inbuf[4]]) as i32;
    let cn = gs.players[nr].usnr;

    log::info!("plr_cmd_turn: cn={} turning to {},{}", cn, x, y);

    if gs.characters[cn].is_building() {
        log::debug!("plr_cmd_turn: cn={} is building, ignoring turn", cn);
        return;
    }

    let ticker = gs.globals.ticker;
    gs.characters[cn].attack_cn = 0;
    gs.characters[cn].goto_x = 0;
    gs.characters[cn].goto_y = 0;
    gs.characters[cn].misc_action = core::constants::DR_TURN as u16;
    gs.characters[cn].misc_target1 = x as u16;
    gs.characters[cn].misc_target2 = y as u16;
    gs.characters[cn].cerrno = 0;
    gs.characters[cn].data[12] = ticker;
}

/// Handle drop item command.
///
/// Reads desired drop coordinates from the client and sets the character's
/// `misc_action` to `DR_DROP`, with target coordinates recorded in
/// `misc_target1/2`. Supports special behavior when in building mode.
///
/// # Arguments
/// * `_nr` - Player slot index performing the drop
pub(super) fn plr_cmd_drop(gs: &mut GameState, _nr: usize) {
    let x = u16::from_le_bytes([gs.players[_nr].inbuf[1], gs.players[_nr].inbuf[2]]) as i32;
    let y = u16::from_le_bytes([gs.players[_nr].inbuf[3], gs.players[_nr].inbuf[4]]) as i32;
    let cn = gs.players[_nr].usnr;

    if gs.characters[cn].is_building() {
        let (action, tx, ty) = (
            gs.characters[cn].misc_action,
            gs.characters[cn].misc_target1,
            gs.characters[cn].misc_target2,
        );

        if action == core::constants::DR_AREABUILD2 as u16 {
            let xs = std::cmp::min(x, tx as i32);
            let ys = std::cmp::min(y, ty as i32);
            let xe = std::cmp::max(x, tx as i32);
            let ye = std::cmp::max(y, ty as i32);

            gs.do_character_log(
                cn,
                core::types::FontColor::Green,
                &format!("Areaend: {},{}\n", x, y),
            );
            gs.do_character_log(
                cn,
                core::types::FontColor::Green,
                &format!("Area: {},{} - {},{}\n", xs, ys, xe, ye),
            );

            gs.characters[cn].misc_action = core::constants::DR_AREABUILD1 as u16;
        } else if action == core::constants::DR_AREABUILD1 as u16 {
            gs.characters[cn].misc_action = core::constants::DR_AREABUILD2 as u16;
            gs.characters[cn].misc_target1 = x as u16;
            gs.characters[cn].misc_target2 = y as u16;
            gs.do_character_log(
                cn,
                core::types::FontColor::Green,
                &format!("Areastart: {},{}\n", x, y),
            );
        }

        return;
    }

    let ticker = gs.globals.ticker;
    gs.characters[cn].attack_cn = 0;
    gs.characters[cn].goto_x = 0;
    gs.characters[cn].misc_action = core::constants::DR_DROP as u16;
    gs.characters[cn].misc_target1 = x as u16;
    gs.characters[cn].misc_target2 = y as u16;
    gs.characters[cn].cerrno = 0;
    gs.characters[cn].data[12] = ticker;
}

/// Handle pickup item command.
///
/// Reads coordinates of the item to pick up and schedules a `DR_PICKUP`
/// misc action on the character, which will be executed by the per-tick
/// processing. Building-mode special cases are respected.
///
/// # Arguments
/// * `nr` - Player slot index issuing the pickup
pub(super) fn plr_cmd_pickup(gs: &mut GameState, nr: usize) {
    let x = u16::from_le_bytes([gs.players[nr].inbuf[1], gs.players[nr].inbuf[2]]) as i32;
    let y = u16::from_le_bytes([gs.players[nr].inbuf[3], gs.players[nr].inbuf[4]]) as i32;
    let cn = gs.players[nr].usnr;

    if gs.characters[cn].is_building() {
        gs.do_build_remove(x, y);
        return;
    }

    let ticker = gs.globals.ticker;
    gs.characters[cn].attack_cn = 0;
    gs.characters[cn].goto_x = 0;
    gs.characters[cn].misc_action = core::constants::DR_PICKUP as u16;
    gs.characters[cn].misc_target1 = x as u16;
    gs.characters[cn].misc_target2 = y as u16;
    gs.characters[cn].cerrno = 0;
    gs.characters[cn].data[12] = ticker;
}

/// Handle attack command.
///
/// Parses the requested target character id and sets the attack variables on
/// the character (`attack_cn`, clears `goto_x`, and resets misc actions)
/// to attempt an attack on subsequent ticks. Also logs the attempt and
/// remembers PvP context.
///
/// # Arguments
/// * `nr` - Player slot index issuing the attack
pub(super) fn plr_cmd_attack(gs: &mut GameState, nr: usize) {
    let co = u32::from_le_bytes([
        gs.players[nr].inbuf[1],
        gs.players[nr].inbuf[2],
        gs.players[nr].inbuf[3],
        gs.players[nr].inbuf[4],
    ]);

    if co as usize >= core::constants::MAXCHARS {
        return;
    }

    let cn = gs.players[nr].usnr;
    let ticker = gs.globals.ticker;

    gs.characters[cn].attack_cn = co as u16;
    gs.characters[cn].goto_x = 0;
    gs.characters[cn].misc_action = 0;
    gs.characters[cn].cerrno = 0;
    gs.characters[cn].data[12] = ticker;

    if (co as usize) < gs.characters.len() {
        log::info!(
            "Trying to attack {} ({})",
            gs.characters[co as usize].get_name(),
            co
        );
    }

    gs.remember_pvp(cn, co as usize);
}

/// Handle speed mode command.
///
/// Sets the character's movement mode (client-side speed preference). Valid
/// modes are 0..2; after update the character record is refreshed to other
/// clients via `do_update_char`.
///
/// # Arguments
/// * `nr` - Player slot index setting the mode
pub(super) fn plr_cmd_mode(gs: &mut GameState, nr: usize) {
    let mode = u16::from_le_bytes([gs.players[nr].inbuf[1], gs.players[nr].inbuf[2]]);

    if mode > 2 {
        log::error!("plr_cmd_mode: invalid mode {}", mode);
        return;
    }

    let cn = gs.players[nr].usnr;
    gs.characters[cn].mode = mode as u8;
    gs.do_update_char(cn);

    log::info!("Player {} set speed mode to {}", cn, mode);
}

/// Handle movement command.
///
/// Accepts a coordinate target from the client and writes it into
/// `goto_x/goto_y` for the given character so the movement driver will try
/// to move the character towards that target in subsequent ticks.
///
/// # Arguments
/// * `nr` - Player slot index sending the movement target
pub(super) fn plr_cmd_move(gs: &mut GameState, nr: usize) {
    let x = u16::from_le_bytes([gs.players[nr].inbuf[1], gs.players[nr].inbuf[2]]);
    let y = u16::from_le_bytes([gs.players[nr].inbuf[3], gs.players[nr].inbuf[4]]);
    let cn = gs.players[nr].usnr;

    let ticker = gs.globals.ticker;

    log::info!(
        "plr_cmd_move: current_position = ({},{})",
        gs.characters[cn].x,
        gs.characters[cn].y,
    );
    gs.characters[cn].attack_cn = 0;
    gs.characters[cn].goto_x = x;
    gs.characters[cn].goto_y = y;
    gs.characters[cn].misc_action = 0;
    gs.characters[cn].cerrno = 0;
    gs.characters[cn].data[12] = ticker;
}

/// Handle reset command.
///
/// Resets various action-related fields on the character (use/skill/attack/
/// goto/misc) and stamps the timestamp so that the character stops any
/// ongoing activity.
///
/// # Arguments
/// * `nr` - Player slot index requesting the reset
pub(super) fn plr_cmd_reset(gs: &mut GameState, nr: usize) {
    let cn = gs.players[nr].usnr;
    let ticker = gs.globals.ticker;
    gs.characters[cn].use_nr = 0;
    gs.characters[cn].skill_nr = 0;
    gs.characters[cn].attack_cn = 0;
    gs.characters[cn].goto_x = 0;
    gs.characters[cn].goto_y = 0;
    gs.characters[cn].misc_action = 0;
    gs.characters[cn].cerrno = 0;
    gs.characters[cn].data[12] = ticker;
}

/// Handle skill use command.
///
/// Parses the requested skill index and target character and schedules the
/// skill for execution by setting `skill_nr` and `skill_target1` on the
/// initiating character. Validates indices and existence of the skill.
///
/// # Arguments
/// * `nr` - Player slot index invoking the skill
pub(super) fn plr_cmd_skill(gs: &mut GameState, nr: usize) {
    let (n, co, cn) = {
        let n = u32::from_le_bytes([
            gs.players[nr].inbuf[1],
            gs.players[nr].inbuf[2],
            gs.players[nr].inbuf[3],
            gs.players[nr].inbuf[4],
        ]) as usize;
        let co = u32::from_le_bytes([
            gs.players[nr].inbuf[5],
            gs.players[nr].inbuf[6],
            gs.players[nr].inbuf[7],
            gs.players[nr].inbuf[8],
        ]) as usize;
        (n, co, gs.players[nr].usnr)
    };

    if n >= core::types::Character::default().skill.len() {
        return;
    }
    if co >= core::constants::MAXCHARS {
        return;
    }

    if gs.characters[cn].skill[n][0] == 0 {
        return;
    }

    gs.characters[cn].skill_nr = n as u16;
    gs.characters[cn].skill_target1 = co as u16;
}

/// Handle inventory look command.
///
/// Allows the player to inspect their inventory slot or (if building mode)
/// set up area-building operations by selecting a slot as the carried item.
/// Otherwise delegates to `do_look_item` for the item at the selected slot.
///
/// # Arguments
/// * `nr` - Player slot index issuing the command
pub(super) fn plr_cmd_inv_look(gs: &mut GameState, nr: usize) {
    let n = u16::from_le_bytes([gs.players[nr].inbuf[1], gs.players[nr].inbuf[2]]) as usize;
    let cn = gs.players[nr].usnr;

    if n > 39 {
        return;
    }

    if gs.characters[cn].is_building() {
        gs.characters[cn].citem = gs.characters[cn].item[n];
        gs.characters[cn].misc_action = core::constants::DR_AREABUILD1 as u16;
        gs.do_character_log(cn, core::types::FontColor::Green, "Area mode\n");
        return;
    }

    let in_idx = gs.characters[cn].item[n] as usize;
    if in_idx != 0 {
        gs.do_look_item(cn, in_idx);
    }
}

/// Handle use command.
///
/// Reads coordinates from the client and schedules a `DR_USE` misc action
/// so that the item on the specified tile will be used by the character on
/// the next tick.
///
/// # Arguments
/// * `nr` - Player slot index issuing the use
pub(super) fn plr_cmd_use(gs: &mut GameState, nr: usize) {
    let x = u16::from_le_bytes([gs.players[nr].inbuf[1], gs.players[nr].inbuf[2]]) as i32;
    let y = u16::from_le_bytes([gs.players[nr].inbuf[3], gs.players[nr].inbuf[4]]) as i32;
    let cn = gs.players[nr].usnr;

    let ticker = gs.globals.ticker;
    gs.characters[cn].attack_cn = 0;
    gs.characters[cn].goto_x = 0;
    gs.characters[cn].misc_action = core::constants::DR_USE as u16;
    gs.characters[cn].misc_target1 = x as u16;
    gs.characters[cn].misc_target2 = y as u16;
    gs.characters[cn].cerrno = 0;
    gs.characters[cn].data[12] = ticker;
}

/// Handle an auto-loot graves command.
///
/// Silently transfers all items whose template ID appears in
/// [`core::constants::AUTOLOOT_ITEM_IDS`] — and unconditionally takes all
/// gold — from the corpse whose tombstone is located at `(x, y)`.
///
/// Performs the same ownership checks as `use_bag`: if the grave belongs to
/// another player who has not issued `#ALLOW`, the transfer is silently
/// rejected. No shop panel is opened on the client side; the player sees
/// individual log lines for each transferred item.
///
/// # Arguments
///
/// * `gs` - Mutable reference to the full game state.
/// * `nr` - Player slot index issuing the command.
pub(super) fn plr_cmd_autoloot(gs: &mut GameState, nr: usize) {
    let x = u16::from_le_bytes([gs.players[nr].inbuf[1], gs.players[nr].inbuf[2]]) as i32;
    let y = u16::from_le_bytes([gs.players[nr].inbuf[3], gs.players[nr].inbuf[4]]) as i32;
    let cn = gs.players[nr].usnr;

    if x < 0 || y < 0 || x >= core::constants::SERVER_MAPX || y >= core::constants::SERVER_MAPY {
        return;
    }

    let m = y as usize * core::constants::SERVER_MAPX as usize + x as usize;
    let item_idx = gs.map[m].it as usize;
    if item_idx == 0 {
        return;
    }
    if gs.items[item_idx].temp != core::constants::IT_TOMBSTONE as u16 {
        return;
    }

    let cn_x = gs.characters[cn].x as i32;
    let cn_y = gs.characters[cn].y as i32;
    if (cn_x - x).abs() > 1 || (cn_y - y).abs() > 1 {
        return;
    }

    let co = gs.items[item_idx].data[0] as usize;
    if !core::types::Character::is_sane_character(co) {
        return;
    }

    let owner = gs.characters[co].data[core::constants::CHD_CORPSEOWNER] as usize;
    if owner != 0 && owner != cn {
        let may_attack = gs.may_attack_msg(cn, owner, false);
        let allowed_cn = gs.characters[owner].data[core::constants::CHD_ALLOW] as usize;
        if !may_attack && allowed_cn != cn {
            return;
        }
    }

    for slot in 0..40usize {
        let it = gs.characters[co].item[slot] as usize;
        if it == 0 {
            continue;
        }
        let temp = gs.items[it].temp;
        if core::constants::AUTOLOOT_ITEM_IDS.contains(&temp) {
            gs.do_shop_char(cn, co, slot as i32, 1);
        }
    }

    for slot in 0..20usize {
        let it = gs.characters[co].worn[slot] as usize;
        if it == 0 {
            continue;
        }
        let temp = gs.items[it].temp;
        if core::constants::AUTOLOOT_ITEM_IDS.contains(&temp) {
            gs.do_shop_char(cn, co, (40 + slot) as i32, 1);
        }
    }

    if gs.characters[co].gold > 0 {
        gs.do_shop_char(cn, co, 61, 1);
    }
}

/// Handle inventory manipulation command.
///
/// Multi-purpose handler for inventory operations (placing/withdrawing
/// items and gold, swapping, selecting use slots, and viewing worn/inv
/// items). The `what` parameter selects the sub-action type while `n` and
/// `co` provide action-specific parameters.
///
/// # Arguments
/// * `nr` - Player slot index issuing the inventory command
pub(super) fn plr_cmd_inv(gs: &mut GameState, nr: usize) {
    let what = u32::from_le_bytes([
        gs.players[nr].inbuf[1],
        gs.players[nr].inbuf[2],
        gs.players[nr].inbuf[3],
        gs.players[nr].inbuf[4],
    ]) as usize;
    let n = u32::from_le_bytes([
        gs.players[nr].inbuf[5],
        gs.players[nr].inbuf[6],
        gs.players[nr].inbuf[7],
        gs.players[nr].inbuf[8],
    ]) as usize;
    let mut co = u32::from_le_bytes([
        gs.players[nr].inbuf[9],
        gs.players[nr].inbuf[10],
        gs.players[nr].inbuf[11],
        gs.players[nr].inbuf[12],
    ]) as usize;
    let cn = gs.players[nr].usnr;

    if !(1..core::constants::MAXCHARS).contains(&co) {
        co = 0;
    }

    if what == 0 {
        if n > 39 {
            return;
        }

        if gs.characters[cn].stunned > 0 {
            return;
        }

        let tmp = gs.characters[cn].item[n] as usize;
        let is_lag = if tmp != 0
            && tmp < gs.items.len()
            && gs.items[tmp].used == core::constants::USE_ACTIVE
        {
            gs.items[tmp].temp as i32 == core::constants::IT_LAGSCROLL
        } else {
            false
        };
        if is_lag {
            return;
        }

        gs.do_update_char(cn);

        if (gs.characters[cn].citem & 0x80000000) != 0 {
            let tmpval = gs.characters[cn].citem & 0x7fffffff;
            if tmpval > 0 {
                gs.characters[cn].gold += tmpval as i32;
            }
            gs.characters[cn].citem = 0;
        } else {
            if !gs.characters[cn].is_building() {
                gs.characters[cn].item[n] = gs.characters[cn].citem;
            } else {
                gs.characters[cn].misc_action = core::constants::DR_SINGLEBUILD as u16;
            }
            gs.characters[cn].citem = tmp as u32;
        }

        return;
    }

    if what == 1 {
        if gs.characters[cn].stunned > 0 {
            return;
        }
        let _ = gs.do_swap_item(cn, n);
        return;
    }

    if what == 2 {
        if gs.characters[cn].stunned > 0 {
            return;
        }
        if gs.characters[cn].citem != 0 {
            return;
        }
        if n as i32 > gs.characters[cn].gold || n == 0 {
            return;
        }
        gs.characters[cn].citem = 0x80000000 | (n as u32);
        gs.characters[cn].gold -= n as i32;
        gs.do_update_char(cn);
        return;
    }

    if what == 5 {
        if n > 19 || gs.characters[cn].is_building() {
            return;
        }
        gs.characters[cn].use_nr = n as u16;
        gs.characters[cn].skill_target1 = co as u16;
        return;
    }

    if what == 6 {
        if n > 39 || gs.characters[cn].is_building() {
            return;
        }
        gs.characters[cn].use_nr = (n as u16) + 20;
        gs.characters[cn].skill_target1 = co as u16;
        return;
    }

    if what == 7 {
        if n > 19 || gs.characters[cn].is_building() {
            return;
        }
        let in_idx = gs.characters[cn].worn[n] as usize;
        if in_idx != 0 {
            gs.do_look_item(cn, in_idx);
        }
        return;
    }

    if what == 8 {
        if n > 39 || gs.characters[cn].is_building() {
            return;
        }
        let in_idx = gs.characters[cn].item[n] as usize;
        if in_idx != 0 {
            gs.do_look_item(cn, in_idx);
        }
        return;
    }

    log::warn!("Unknown CMD-INV-what {}", what);
}

/// Handle exit command (F12).
///
/// Performs an immediate logout for the requesting player slot by
/// calling `plr_logout` with `LogoutReason::Exit`.
///
/// # Arguments
/// * `nr` - Player slot index pressing F12
pub(super) fn plr_cmd_exit(gs: &mut GameState, nr: usize) {
    log::info!("Player {} pressed F12", nr);
    let cn = gs.players[nr].usnr;
    plr_logout(gs, cn, nr, LogoutReason::Exit);
}

/// Handle shop command.
///
/// Handles buying/selling interactions with shops or depot operations when
/// the high bit of `co` is set (depot index). Delegates to `do_depot_char`
/// or `do_shop_char` to perform the actual shop/depot logic.
///
/// # Arguments
/// * `nr` - Player slot index issuing the shop command
pub(super) fn plr_cmd_shop(gs: &mut GameState, nr: usize) {
    let co = u16::from_le_bytes([gs.players[nr].inbuf[1], gs.players[nr].inbuf[2]]) as usize;
    let n = u16::from_le_bytes([gs.players[nr].inbuf[3], gs.players[nr].inbuf[4]]) as i32;
    let cn = gs.players[nr].usnr;

    if (co & 0x8000) != 0 {
        let idx = co & 0x7fff;
        gs.do_depot_char(cn, idx, n);
    } else {
        gs.do_shop_char(cn, co, n, 0);
    }
}

#[cfg(test)]
mod tests {
    use core::constants::{DR_GIVE, DR_TURN, DR_USE};

    use crate::test_helpers::{add_test_player, with_test_gs, write_inbuf};

    use super::{
        plr_cmd_ctick, plr_cmd_give, plr_cmd_move, plr_cmd_reset, plr_cmd_skill, plr_cmd_turn,
        plr_cmd_use,
    };

    #[test]
    fn cmd_ctick_updates_rtick_and_lasttick() {
        with_test_gs(|gs| {
            let (_, nr) = add_test_player(gs);
            gs.globals.ticker = 4321;

            let mut buf = [0u8; 5];
            buf[1..5].copy_from_slice(&1234u32.to_le_bytes());
            write_inbuf(gs, nr, &buf);

            plr_cmd_ctick(gs, nr);

            assert_eq!(gs.players[nr].rtick, 1234);
            assert_eq!(gs.players[nr].lasttick, 4321);
        });
    }

    #[test]
    fn cmd_move_sets_destination_and_clears_attack() {
        with_test_gs(|gs| {
            let (cn, nr) = add_test_player(gs);
            gs.globals.ticker = 77;
            gs.characters[cn].attack_cn = 55;
            gs.characters[cn].misc_action = 9;

            let mut buf = [0u8; 5];
            buf[1..3].copy_from_slice(&25u16.to_le_bytes());
            buf[3..5].copy_from_slice(&26u16.to_le_bytes());
            write_inbuf(gs, nr, &buf);

            plr_cmd_move(gs, nr);

            assert_eq!(gs.characters[cn].attack_cn, 0);
            assert_eq!(gs.characters[cn].goto_x, 25);
            assert_eq!(gs.characters[cn].goto_y, 26);
            assert_eq!(gs.characters[cn].misc_action, 0);
            assert_eq!(gs.characters[cn].cerrno, 0);
            assert_eq!(gs.characters[cn].data[12], 77);
        });
    }

    #[test]
    fn cmd_turn_schedules_turn_action() {
        with_test_gs(|gs| {
            let (cn, nr) = add_test_player(gs);
            gs.globals.ticker = 88;
            gs.characters[cn].attack_cn = 4;
            gs.characters[cn].goto_x = 99;
            gs.characters[cn].goto_y = 100;

            let mut buf = [0u8; 5];
            buf[1..3].copy_from_slice(&12u16.to_le_bytes());
            buf[3..5].copy_from_slice(&13u16.to_le_bytes());
            write_inbuf(gs, nr, &buf);

            plr_cmd_turn(gs, nr);

            assert_eq!(gs.characters[cn].attack_cn, 0);
            assert_eq!(gs.characters[cn].goto_x, 0);
            assert_eq!(gs.characters[cn].goto_y, 0);
            assert_eq!(gs.characters[cn].misc_action, DR_TURN as u16);
            assert_eq!(gs.characters[cn].misc_target1, 12);
            assert_eq!(gs.characters[cn].misc_target2, 13);
            assert_eq!(gs.characters[cn].data[12], 88);
        });
    }

    #[test]
    fn cmd_give_sets_misc_target() {
        with_test_gs(|gs| {
            let (cn, nr) = add_test_player(gs);
            gs.globals.ticker = 99;
            gs.characters[cn].attack_cn = 7;
            gs.characters[cn].goto_x = 41;
            gs.characters[cn].goto_y = 42;

            let mut buf = [0u8; 5];
            buf[1..5].copy_from_slice(&6u32.to_le_bytes());
            write_inbuf(gs, nr, &buf);

            plr_cmd_give(gs, nr);

            assert_eq!(gs.characters[cn].attack_cn, 0);
            assert_eq!(gs.characters[cn].goto_x, 0);
            assert_eq!(gs.characters[cn].goto_y, 42);
            assert_eq!(gs.characters[cn].misc_action, DR_GIVE as u16);
            assert_eq!(gs.characters[cn].misc_target1, 6);
            assert_eq!(gs.characters[cn].data[12], 99);
        });
    }

    #[test]
    fn cmd_use_schedules_use_action() {
        with_test_gs(|gs| {
            let (cn, nr) = add_test_player(gs);
            gs.globals.ticker = 123;
            gs.characters[cn].attack_cn = 8;
            gs.characters[cn].goto_x = 31;
            gs.characters[cn].goto_y = 32;

            let mut buf = [0u8; 5];
            buf[1..3].copy_from_slice(&14u16.to_le_bytes());
            buf[3..5].copy_from_slice(&15u16.to_le_bytes());
            write_inbuf(gs, nr, &buf);

            plr_cmd_use(gs, nr);

            assert_eq!(gs.characters[cn].attack_cn, 0);
            assert_eq!(gs.characters[cn].goto_x, 0);
            assert_eq!(gs.characters[cn].goto_y, 32);
            assert_eq!(gs.characters[cn].misc_action, DR_USE as u16);
            assert_eq!(gs.characters[cn].misc_target1, 14);
            assert_eq!(gs.characters[cn].misc_target2, 15);
            assert_eq!(gs.characters[cn].data[12], 123);
        });
    }

    #[test]
    fn cmd_reset_clears_pending_actions() {
        with_test_gs(|gs| {
            let (cn, nr) = add_test_player(gs);
            gs.globals.ticker = 456;
            gs.characters[cn].use_nr = 1;
            gs.characters[cn].skill_nr = 2;
            gs.characters[cn].attack_cn = 3;
            gs.characters[cn].goto_x = 4;
            gs.characters[cn].goto_y = 5;
            gs.characters[cn].misc_action = 6;

            plr_cmd_reset(gs, nr);

            assert_eq!(gs.characters[cn].use_nr, 0);
            assert_eq!(gs.characters[cn].skill_nr, 0);
            assert_eq!(gs.characters[cn].attack_cn, 0);
            assert_eq!(gs.characters[cn].goto_x, 0);
            assert_eq!(gs.characters[cn].goto_y, 0);
            assert_eq!(gs.characters[cn].misc_action, 0);
            assert_eq!(gs.characters[cn].data[12], 456);
        });
    }

    #[test]
    fn cmd_skill_sets_selected_skill_and_target() {
        with_test_gs(|gs| {
            let (cn, nr) = add_test_player(gs);
            gs.characters[cn].skill[3][0] = 1;

            let mut buf = [0u8; 9];
            buf[1..5].copy_from_slice(&3u32.to_le_bytes());
            buf[5..9].copy_from_slice(&9u32.to_le_bytes());
            write_inbuf(gs, nr, &buf);

            plr_cmd_skill(gs, nr);

            assert_eq!(gs.characters[cn].skill_nr, 3);
            assert_eq!(gs.characters[cn].skill_target1, 9);
        });
    }
}
