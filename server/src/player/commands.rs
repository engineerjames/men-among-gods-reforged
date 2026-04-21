use core::{
    constants::CharacterFlags, logout_reasons::LogoutReason, server_commands::ServerCommandType,
    string_operations::c_string_to_str, traits,
};

use crate::{
    driver,
    game_state::GameState,
    god::God,
    network_manager,
    player::{
        connection::plr_logout,
        map::{plr_map_remove, plr_map_set},
        notify_character_tile,
    },
};

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
pub fn plr_cmd_look(gs: &mut GameState, nr: usize, autoflag: bool) {
    let co = u16::from_le_bytes([gs.players[nr].inbuf[1], gs.players[nr].inbuf[2]]) as usize;
    let cn = gs.players[nr].usnr;

    // Check if looking at depot (high bit set) or character
    if (co & 0x8000) != 0 {
        // Looking at depot slot
        let depot_slot = co & 0x7fff;
        gs.do_look_depot(cn, depot_slot);
    } else {
        // Looking at character
        let autoflag_int = if autoflag { 1 } else { 0 };
        gs.do_look_char(cn, co, 0, autoflag_int, 0);
    }
}

/// Handle set user data command
///
/// Receives chunks of account/profile data from the client (13-byte
/// fragments) and writes them into the character's `text` buffers. When the
/// final chunk is received for the description/name update it performs
/// validation (name legality, uniqueness, description rules) and either
/// commits changes or reports why they were rejected.
///
/// # Arguments
/// * `_nr` - Player slot index sending the data
pub fn plr_cmd_setuser(gs: &mut GameState, _nr: usize) {
    // Implementation based on original svr_tick.cpp
    // Read subtype, position and 13 bytes of data from player's inbuf
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
            // write 13 bytes into text[0] or text[1]
            let text_idx = if subtype == 0 { 0 } else { 1 };
            gs.characters[cn].text[text_idx][pos..(13 + pos)].copy_from_slice(&chunk);
        }
        2 => {
            // write into text[2]
            gs.characters[cn].text[2][pos..(13 + pos)].copy_from_slice(&chunk);

            // If this was the final chunk (pos == 65) perform validation and possibly
            // commit name/reference/description changes.
            if pos == 65 {
                // Work inside a mutable characters closure to inspect & modify
                {
                    let is_new_user = (gs.characters[cn].flags
                        & core::constants::CharacterFlags::NewUser.bits())
                        != 0;
                    // Name handling: examine text[0]
                    let name_bytes = &mut gs.characters[cn].text[0];
                    let name_end = name_bytes
                        .iter()
                        .position(|&c| c == 0)
                        .unwrap_or(name_bytes.len());
                    // IMPORTANT: Match the C++ gating logic.
                    // Only validate/commit the name when the user is new AND the name length is sane.
                    // Otherwise, do not touch `name`/`reference` (prevents committing empty names).
                    let should_process_name = name_end > 3 && name_end < 38 && is_new_user;

                    let mut name_flag: i32 = 0;
                    if should_process_name {
                        // validate letters only and lowercase
                        for n in 0..name_end {
                            let b = name_bytes[n];
                            if !(b.is_ascii_uppercase() || b.is_ascii_lowercase()) {
                                name_flag = 1;
                                log::warn!(
                                    "plr_cmd_setuser: name contains non-letter char {:02X}",
                                    b
                                );
                                break;
                            }
                            name_bytes[n] = name_bytes[n].to_ascii_lowercase();
                        }

                        if name_flag == 0 {
                            // uppercase first letter
                            if name_end > 0 {
                                name_bytes[0] = name_bytes[0].to_ascii_uppercase();
                            }

                            // check reserved name "Self"
                            let name_str = c_string_to_str(name_bytes).to_string();

                            if name_str == "Self" {
                                log::warn!("plr_cmd_setuser: name \"{}\" is reserved", name_str);
                                name_flag = 2;
                            }

                            // check for duplicate names
                            if name_flag == 0 {
                                for n in 1..core::constants::MAXCHARS {
                                    if n != cn
                                        && gs.characters[n].used != core::constants::USE_EMPTY
                                    {
                                        let mut other_name = gs.characters[n]
                                            .get_name()
                                            .to_string()
                                            .to_ascii_lowercase();

                                        // Uppercase first character safely without indexing into String
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

                            // C++ also rejects names which match mob/template names.
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

                            // TODO: badname check unavailable in Rust port; skip CF_NODESC check here
                        }

                        // If flag set -> report and don't commit name change
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
                            // Commit name -> copy into name and reference (40 bytes)
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
                            // clear CF_NEWUSER flag
                            gs.characters[cn].flags &=
                                !core::constants::CharacterFlags::NewUser.bits();

                            log::info!(
                                "plr_cmd_setuser: committed name change for cn={} to \"{}\"",
                                cn,
                                gs.characters[cn].get_name()
                            );
                        }
                    }

                    // Description handling: copy text[1] and possibly append text[2]
                    let mut desc = c_string_to_str(&gs.characters[cn].text[1]).to_string();
                    if desc.len() > 77 {
                        let add = c_string_to_str(&gs.characters[cn].text[2]).to_string();
                        desc.push_str(&add);
                    }

                    // Validate description
                    let mut desc_reason: Option<String> = None;
                    if desc.len() < 10 {
                        desc_reason = Some("is too short".to_string());
                    } else {
                        // Does description contain name?
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
                        // pick race name
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

                        // fallback description
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
                        // write fallback into description (200 bytes max)
                        let bytes = fallback.as_bytes();
                        for i in 0..200 {
                            gs.characters[cn].description[i] =
                                if i < bytes.len() { bytes[i] } else { 0 };
                        }
                    } else {
                        // commit description
                        let bytes = desc.as_bytes();
                        for i in 0..200 {
                            gs.characters[cn].description[i] =
                                if i < bytes.len() { bytes[i] } else { 0 };
                        }
                    }
                    // Finally acknowledge and request character update
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

/// Handle stat change command
///
/// Applies attribute/HP/endurance/mana/skill raises requested by the
/// client. Validates indices and performs repeated raise operations via
/// `GameState` helpers, then requests a character update.
///
/// # Arguments
/// * `_nr` - Player slot index issuing the stat change
pub fn plr_cmd_stat(gs: &mut GameState, _nr: usize) {
    // Read stat index and value from inbuf and apply raises
    let cn = gs.players[_nr].usnr;
    let n = u16::from_le_bytes([gs.players[_nr].inbuf[1], gs.players[_nr].inbuf[2]]) as usize;
    let v = u16::from_le_bytes([gs.players[_nr].inbuf[3], gs.players[_nr].inbuf[4]]) as usize;

    // sanity checks
    if n > 107 || v > 99 {
        return;
    }

    // perform raises
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

    // request character update
    gs.do_update_char(cn);
}

/// Handle text input commands (1-8)
///
/// Receives a 15-byte chunk of textual input from the client. When the
/// eighth (final) chunk is received the function NUL-terminates the collected
/// input, decodes it to a UTF-8 string, and forwards it to `do_say` for
/// processing as a chat/message.
///
/// # Arguments
/// * `nr` - Player slot index sending the input
/// * `part` - Which 1..8 chunk this call contains
pub fn plr_cmd_input(gs: &mut GameState, nr: usize, part: u8) {
    // Copy 15 bytes of input from inbuf to player input buffer
    let offset = ((part - 1) as usize) * 15;
    for n in 0..15 {
        gs.players[nr].input[offset + n] = gs.players[nr].inbuf[1 + n];
    }

    if part == 8 {
        gs.players[nr].input[105 + 14] = 0;

        let cn = gs.players[nr].usnr;
        let raw = gs.players[nr].input.to_vec();

        let text = c_string_to_str(&raw);

        // Call the server state handler (port of C++ do_say)
        gs.do_say(cn, text);
    }
}

/// Handle client tick update
///
/// Updates server-side bookkeeping for client timing. Reads `rtick` from the
/// client's inbuf, stores it in `players[nr].rtick`, and refreshes the
/// player's `lasttick` timeout to avoid idle/disconnect handling.
///
/// # Arguments
/// * `nr` - Player slot index sending the tick
pub fn plr_cmd_ctick(gs: &mut GameState, nr: usize) {
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
pub fn plr_cmd_ping(gs: &mut GameState, nr: usize) {
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

/// Handle look at item on ground
///
/// Reads coordinates from the client's packet, validates them, and if the
/// tile contains an item calls `do_look_item` to present details to the
/// requesting character.
///
/// # Arguments
/// * `nr` - Player slot index issuing the request
pub fn plr_cmd_look_item(gs: &mut GameState, nr: usize) {
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

/// Handle give item command
///
/// Reads a target character id from the client's packet and sets the
/// giving character's misc action (`DR_GIVE`) and `misc_target1` to
/// perform a give in the next tick.
///
/// # Arguments
/// * `nr` - Player slot index issuing the give
pub fn plr_cmd_give(gs: &mut GameState, nr: usize) {
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

/// Handle turn command
///
/// Reads target coordinates from the client and sets a turn action
/// (`DR_TURN`) so the character will turn toward the specified point on
/// its next action tick. Ignored if the character is in building mode.
///
/// # Arguments
/// * `nr` - Player slot index issuing the turn
pub fn plr_cmd_turn(gs: &mut GameState, nr: usize) {
    let x = u16::from_le_bytes([gs.players[nr].inbuf[1], gs.players[nr].inbuf[2]]) as i32;
    let y = u16::from_le_bytes([gs.players[nr].inbuf[3], gs.players[nr].inbuf[4]]) as i32;
    let cn = gs.players[nr].usnr;

    log::info!("plr_cmd_turn: cn={} turning to {},{}", cn, x, y);

    // If building mode, ignore
    let is_building = gs.characters[cn].is_building();
    if is_building {
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

/// Handle drop item command
///
/// Reads desired drop coordinates from the client and sets the character's
/// `misc_action` to `DR_DROP`, with target coordinates recorded in
/// `misc_target1/2`. Supports special behavior when in building mode.
///
/// # Arguments
/// * `_nr` - Player slot index performing the drop
pub fn plr_cmd_drop(gs: &mut GameState, _nr: usize) {
    let x = u16::from_le_bytes([gs.players[_nr].inbuf[1], gs.players[_nr].inbuf[2]]) as i32;
    let y = u16::from_le_bytes([gs.players[_nr].inbuf[3], gs.players[_nr].inbuf[4]]) as i32;
    let cn = gs.players[_nr].usnr;

    // Building-mode special handling
    let is_building = gs.characters[cn].is_building();
    if is_building {
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

            // Note: actual build_drop per-tile processing not implemented yet.
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
        } else if action == core::constants::DR_SINGLEBUILD as u16 {
            // Single build: would normally place immediately. Not implemented.
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

/// Handle pickup item command
///
/// Reads coordinates of the item to pick up and schedules a `DR_PICKUP`
/// misc action on the character, which will be executed by the per-tick
/// processing. Building-mode special cases are respected.
///
/// # Arguments
/// * `nr` - Player slot index issuing the pickup
pub fn plr_cmd_pickup(gs: &mut GameState, nr: usize) {
    let x = u16::from_le_bytes([gs.players[nr].inbuf[1], gs.players[nr].inbuf[2]]) as i32;
    let y = u16::from_le_bytes([gs.players[nr].inbuf[3], gs.players[nr].inbuf[4]]) as i32;
    let cn = gs.players[nr].usnr;

    // Building-mode: removal in build mode should remove the temporary build object
    let is_building = gs.characters[cn].is_building();
    if is_building {
        // Call the build removal helper (port of C++ build_remove)
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

/// Handle attack command
///
/// Parses the requested target character id and sets the attack variables on
/// the character (`attack_cn`, clears `goto_x`, and resets misc actions)
/// to attempt an attack on subsequent ticks. Also logs the attempt and
/// remembers PvP context.
///
/// # Arguments
/// * `nr` - Player slot index issuing the attack
pub fn plr_cmd_attack(gs: &mut GameState, nr: usize) {
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

/// Handle speed mode command
///
/// Sets the character's movement mode (client-side speed preference). Valid
/// modes are 0..2; after update the character record is refreshed to other
/// clients via `do_update_char`.
///
/// # Arguments
/// * `nr` - Player slot index setting the mode
pub fn plr_cmd_mode(gs: &mut GameState, nr: usize) {
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

/// Handle movement command
///
/// Accepts a coordinate target from the client and writes it into
/// `goto_x/goto_y` for the given character so the movement driver will try
/// to move the character towards that target in subsequent ticks.
///
/// # Arguments
/// * `nr` - Player slot index sending the movement target
pub fn plr_cmd_move(gs: &mut GameState, nr: usize) {
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

/// Handle reset command
///
/// Resets various action-related fields on the character (use/skill/attack/
/// goto/misc) and stamps the timestamp so that the character stops any
/// ongoing activity.
///
/// # Arguments
/// * `nr` - Player slot index requesting the reset
pub fn plr_cmd_reset(gs: &mut GameState, nr: usize) {
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

/// Handle skill use command
///
/// Parses the requested skill index and target character and schedules the
/// skill for execution by setting `skill_nr` and `skill_target1` on the
/// initiating character. Validates indices and existence of the skill.
///
/// # Arguments
/// * `nr` - Player slot index invoking the skill
pub fn plr_cmd_skill(gs: &mut GameState, nr: usize) {
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

    // sanity checks: skill index must be within available skill table
    if n >= core::types::Character::default().skill.len() {
        return;
    }
    if co >= core::constants::MAXCHARS {
        return;
    }

    // ensure skill exists for this character
    let has_skill = gs.characters[cn].skill[n][0] != 0;
    if !has_skill {
        return;
    }

    gs.characters[cn].skill_nr = n as u16;
    gs.characters[cn].skill_target1 = co as u16;
}

/// Handle inventory look command
///
/// Allows the player to inspect their inventory slot or (if building mode)
/// set up area-building operations by selecting a slot as the carried item.
/// Otherwise delegates to `do_look_item` for the item at the selected slot.
///
/// # Arguments
/// * `nr` - Player slot index issuing the command
pub fn plr_cmd_inv_look(gs: &mut GameState, nr: usize) {
    let n = u16::from_le_bytes([gs.players[nr].inbuf[1], gs.players[nr].inbuf[2]]) as usize;
    let cn = gs.players[nr].usnr;

    if n > 39 {
        return;
    }

    let is_building = gs.characters[cn].is_building();
    if is_building {
        // set carried item to the selected inventory slot and enter area-build
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

/// Handle use command
///
/// Reads coordinates from the client and schedules a `DR_USE` misc action
/// so that the item on the specified tile will be used by the character on
/// the next tick.
///
/// # Arguments
/// * `nr` - Player slot index issuing the use
pub fn plr_cmd_use(gs: &mut GameState, nr: usize) {
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
/// Performs the same ownership checks as [`use_bag`]: if the grave belongs to
/// another player who has not issued `#ALLOW`, the transfer is silently
/// rejected.  No shop panel is opened on the client side; the player sees
/// individual "You took a …" log lines for each transferred item.
///
/// # Arguments
///
/// * `gs` - Mutable reference to the full game state.
/// * `nr` - Player slot index issuing the command.
pub fn plr_cmd_autoloot(gs: &mut GameState, nr: usize) {
    let x = u16::from_le_bytes([gs.players[nr].inbuf[1], gs.players[nr].inbuf[2]]) as i32;
    let y = u16::from_le_bytes([gs.players[nr].inbuf[3], gs.players[nr].inbuf[4]]) as i32;
    let cn = gs.players[nr].usnr;

    // Bounds-check the incoming world coordinates.
    if x < 0 || y < 0 || x >= core::constants::SERVER_MAPX || y >= core::constants::SERVER_MAPY {
        return;
    }

    let m = y as usize * core::constants::SERVER_MAPX as usize + x as usize;

    // Verify there is a tombstone item on the tile.
    let item_idx = gs.map[m].it as usize;
    if item_idx == 0 {
        return;
    }
    if gs.items[item_idx].temp != core::constants::IT_TOMBSTONE as u16 {
        return;
    }

    // Verify the player is adjacent (Chebyshev distance ≤ 1).
    let cn_x = gs.characters[cn].x as i32;
    let cn_y = gs.characters[cn].y as i32;
    if (cn_x - x).abs() > 1 || (cn_y - y).abs() > 1 {
        return;
    }

    // Validate the corpse reference stored in the tombstone item.
    let co = gs.items[item_idx].data[0] as usize;
    if !core::types::Character::is_sane_character(co) {
        return;
    }

    // Ownership check (mirrors use_bag).
    let owner = gs.characters[co].data[core::constants::CHD_CORPSEOWNER] as usize;
    if owner != 0 && owner != cn {
        let may_attack = gs.may_attack_msg(cn, owner, false);
        let allowed_cn = gs.characters[owner].data[core::constants::CHD_ALLOW] as usize;
        if !may_attack && allowed_cn != cn {
            return;
        }
    }

    // --- Inventory slots 0..40 ---
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

    // --- Worn slots 0..20 (sent as nr 40..60 in do_shop_char) ---
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

    // --- Gold (slot 61) — always take ---
    if gs.characters[co].gold > 0 {
        gs.do_shop_char(cn, co, 61, 1);
    }
}

/// Handle inventory manipulation command
///
/// Multi-purpose handler for inventory operations (placing/withdrawing
/// items and gold, swapping, selecting use slots, and viewing worn/inv
/// items). The `what` parameter selects the sub-action type while `n` and
/// `co` provide action-specific parameters.
///
/// # Arguments
/// * `nr` - Player slot index issuing the inventory command
pub fn plr_cmd_inv(gs: &mut GameState, nr: usize) {
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

    // what == 0 : normal inventory
    if what == 0 {
        if n > 39 {
            return;
        }

        let stunned = gs.characters[cn].stunned > 0;
        if stunned {
            return;
        }

        // check for lag scroll template on the item
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

        // Now handle citem/gold swap or placing citem into slot
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

    // what == 1 : big inventory swap
    if what == 1 {
        let stunned = gs.characters[cn].stunned > 0;
        if stunned {
            return;
        }
        let _ = gs.do_swap_item(cn, n);
        return;
    }

    // what == 2 : withdraw gold into cursor
    if what == 2 {
        let stunned = gs.characters[cn].stunned > 0;
        if stunned {
            return;
        }
        let citem = gs.characters[cn].citem;
        if citem != 0 {
            return;
        }
        if n as i32 > gs.characters[cn].gold {
            return;
        }
        if n == 0 {
            return;
        }
        gs.characters[cn].citem = 0x80000000 | (n as u32);
        gs.characters[cn].gold -= n as i32;
        gs.do_update_char(cn);
        return;
    }

    // what == 5 : use_nr = n (worn slots)
    if what == 5 {
        if n > 19 {
            return;
        }
        let is_building = gs.characters[cn].is_building();
        if is_building {
            return;
        }
        gs.characters[cn].use_nr = n as u16;
        gs.characters[cn].skill_target1 = co as u16;
        return;
    }

    // what == 6 : use_nr = n + 20 (inventory)
    if what == 6 {
        if n > 39 {
            return;
        }
        let is_building = gs.characters[cn].is_building();
        if is_building {
            return;
        }
        gs.characters[cn].use_nr = (n as u16) + 20;
        gs.characters[cn].skill_target1 = co as u16;
        return;
    }

    // what == 7 : look at worn item
    if what == 7 {
        if n > 19 {
            return;
        }
        let is_building = gs.characters[cn].is_building();
        if is_building {
            return;
        }
        let in_idx = gs.characters[cn].worn[n] as usize;
        if in_idx != 0 {
            gs.do_look_item(cn, in_idx);
        }
        return;
    }

    // what == 8 : look at inventory item
    if what == 8 {
        if n > 39 {
            return;
        }
        let is_building = gs.characters[cn].is_building();
        if is_building {
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

/// Handle exit command (F12)
///
/// Performs an immediate logout for the requesting player slot by
/// calling `plr_logout` with `LogoutReason::Exit`.
///
/// # Arguments
/// * `nr` - Player slot index pressing F12
pub fn plr_cmd_exit(gs: &mut GameState, nr: usize) {
    log::info!("Player {} pressed F12", nr);
    let cn = gs.players[nr].usnr;
    plr_logout(gs, cn, nr, LogoutReason::Exit);
}

/// Handle shop command
///
/// Handles buying/selling interactions with shops or depot operations when
/// the high bit of `co` is set (depot index). Delegates to `do_depot_char`
/// or `do_shop_char` to perform the actual shop/depot logic.
///
/// # Arguments
/// * `nr` - Player slot index issuing the shop command
pub fn plr_cmd_shop(gs: &mut GameState, nr: usize) {
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

/// Move a character by the given delta and reinsert them into the map.
///
/// # Arguments
/// * `gs` - Active game state used for movement bookkeeping.
/// * `cn` - Character index performing the move.
/// * `dx` - Horizontal movement delta.
/// * `dy` - Vertical movement delta.
fn plr_move_by(gs: &mut GameState, cn: usize, dx: i16, dy: i16) {
    plr_map_remove(gs, cn);

    let ch = &mut gs.characters[cn];
    ch.frx = ch.x;
    ch.fry = ch.y;
    ch.x += dx;
    ch.y += dy;
    ch.tox = ch.x;
    ch.toy = ch.y;

    plr_map_set(gs, cn);
    gs.characters[cn].cerrno = core::constants::ERR_SUCCESS as u16;
}

/// Rotate a character and notify nearby clients about the change.
///
/// # Arguments
/// * `gs` - Active game state used for notification dispatch.
/// * `cn` - Character index rotating.
/// * `dir` - New facing direction.
fn plr_turn(gs: &mut GameState, cn: usize, dir: u8) {
    notify_character_tile(gs, cn);
    gs.characters[cn].dir = dir;
    gs.characters[cn].cerrno = core::constants::ERR_SUCCESS as u16;
}

/// Compute the map tile directly in front of the character.
///
/// # Arguments
/// * `gs` - Active game state used to inspect character position.
/// * `cn` - Character index performing the action.
/// * `action` - Action name used for logging invalid directions.
///
/// # Returns
/// * `Some((map_index, x, y))` if the facing direction is valid.
/// * `None` if the direction is invalid and `cerrno` was set.
fn plr_front_tile(gs: &mut GameState, cn: usize, action: &str) -> Option<(usize, i32, i32)> {
    let (mut x, mut y, dir) = (
        gs.characters[cn].x as i32,
        gs.characters[cn].y as i32,
        gs.characters[cn].dir,
    );

    match dir {
        core::constants::DX_UP => y -= 1,
        core::constants::DX_DOWN => y += 1,
        core::constants::DX_LEFT => x -= 1,
        core::constants::DX_RIGHT => x += 1,
        _ => {
            log::error!("{}: unknown dir {} for char {}", action, dir, cn);
            gs.characters[cn].cerrno = core::constants::ERR_FAILED as u16;
            return None;
        }
    }

    let m = (x as usize) + (y as usize) * core::constants::SERVER_MAPX as usize;
    Some((m, x, y))
}

/// Compute the adjacent cardinal tile in front of the character, with bounds checks.
///
/// # Arguments
/// * `gs` - Active game state used to inspect character position.
/// * `cn` - Character index performing the action.
///
/// # Returns
/// * `Some((map_index, x, y))` when the facing tile is on the map.
/// * `None` if the facing direction is diagonal or would leave the map.
fn plr_cardinal_front_tile(gs: &mut GameState, cn: usize) -> Option<(usize, i16, i16)> {
    let ch = gs.characters[cn];

    match ch.dir {
        core::constants::DX_UP if ch.y > 0 => {
            let y = ch.y - 1;
            let m = (ch.x as usize) + (y as usize) * core::constants::SERVER_MAPX as usize;
            Some((m, ch.x, y))
        }
        core::constants::DX_DOWN if ch.y < (core::constants::SERVER_MAPY as i16 - 1) => {
            let y = ch.y + 1;
            let m = (ch.x as usize) + (y as usize) * core::constants::SERVER_MAPX as usize;
            Some((m, ch.x, y))
        }
        core::constants::DX_LEFT if ch.x > 0 => {
            let x = ch.x - 1;
            let m = (x as usize) + (ch.y as usize) * core::constants::SERVER_MAPX as usize;
            Some((m, x, ch.y))
        }
        core::constants::DX_RIGHT if ch.x < (core::constants::SERVER_MAPX as i16 - 1) => {
            let x = ch.x + 1;
            let m = (x as usize) + (ch.y as usize) * core::constants::SERVER_MAPX as usize;
            Some((m, x, ch.y))
        }
        _ => None,
    }
}

/// Port of `plr_move_up` from `svr_act.cpp`
///
/// Performs a move action upwards for the given character. This removes the
/// character from its current tile, updates the previous position (frx,fry),
/// adjusts the y coordinate and target coordinates, then re-inserts the
/// character into the map via `plr_map_set` and marks the action as
/// successful.
///
/// # Arguments
/// * `cn` - Character index performing the move
pub fn plr_move_up(gs: &mut GameState, cn: usize) {
    plr_move_by(gs, cn, 0, -1);
}

/// Port of `plr_move_down` from `svr_act.cpp`
///
/// Performs a move action downwards for the given character and updates
/// internal position state similar to `plr_move_up`.
///
/// # Arguments
/// * `cn` - Character index performing the move
pub fn plr_move_down(gs: &mut GameState, cn: usize) {
    plr_move_by(gs, cn, 0, 1);
}

/// Port of `plr_move_left` from `svr_act.cpp`
///
/// Performs a move action left for the given character and updates
/// position and map state as in other move helpers.
///
/// # Arguments
/// * `cn` - Character index performing the move
pub fn plr_move_left(gs: &mut GameState, cn: usize) {
    plr_move_by(gs, cn, -1, 0);
}

/// Port of `plr_move_right` from `svr_act.cpp`
///
/// Performs a move action right for the given character and updates
/// position and map state as in other move helpers.
///
/// # Arguments
/// * `cn` - Character index performing the move
pub fn plr_move_right(gs: &mut GameState, cn: usize) {
    plr_move_by(gs, cn, 1, 0);
}

/// Port of `plr_move_leftup` from `svr_act.cpp`
///
/// Performs a diagonal up-left move for the character and updates map state.
///
/// # Arguments
/// * `cn` - Character index performing the move
pub fn plr_move_leftup(gs: &mut GameState, cn: usize) {
    plr_move_by(gs, cn, -1, -1);
}

/// Port of `plr_move_leftdown` from `svr_act.cpp`
///
/// Performs a diagonal down-left move for the character and updates map state.
///
/// # Arguments
/// * `cn` - Character index performing the move
pub fn plr_move_leftdown(gs: &mut GameState, cn: usize) {
    plr_move_by(gs, cn, -1, 1);
}

/// Port of `plr_move_rightup` from `svr_act.cpp`
///
/// Performs a diagonal up-right move for the character and updates map state.
///
/// # Arguments
/// * `cn` - Character index performing the move
pub fn plr_move_rightup(gs: &mut GameState, cn: usize) {
    plr_move_by(gs, cn, 1, -1);
}

/// Port of `plr_move_rightdown` from `svr_act.cpp`
///
/// Performs a diagonal down-right move for the character and updates map state.
///
/// # Arguments
/// * `cn` - Character index performing the move
pub fn plr_move_rightdown(gs: &mut GameState, cn: usize) {
    plr_move_by(gs, cn, 1, 1);
}

/// Port of `plr_turn_up` from `svr_act.cpp`
///
/// Sets the character's facing direction to up and notifies nearby
/// observers about the change via area notification.
///
/// # Arguments
/// * `cn` - Character index rotating to face up
pub fn plr_turn_up(gs: &mut GameState, cn: usize) {
    plr_turn(gs, cn, core::constants::DX_UP);
}

/// Port of `plr_turn_leftup` from `svr_act.cpp`
///
/// Sets the character's facing direction to left-up and notifies nearby
/// observers about the change.
///
/// # Arguments
/// * `cn` - Character index rotating to face left-up
pub fn plr_turn_leftup(gs: &mut GameState, cn: usize) {
    plr_turn(gs, cn, core::constants::DX_LEFTUP);
}

/// Port of `plr_turn_leftdown` from `svr_act.cpp`
///
/// Sets the character's facing direction to left-down and notifies nearby
/// observers about the change.
///
/// # Arguments
/// * `cn` - Character index rotating to face left-down
pub fn plr_turn_leftdown(gs: &mut GameState, cn: usize) {
    plr_turn(gs, cn, core::constants::DX_LEFTDOWN);
}

/// Port of `plr_turn_down` from `svr_act.cpp`
///
/// Sets the character's facing direction to down and notifies nearby
/// observers about the change.
///
/// # Arguments
/// * `cn` - Character index rotating to face down
pub fn plr_turn_down(gs: &mut GameState, cn: usize) {
    plr_turn(gs, cn, core::constants::DX_DOWN);
}

/// Port of `plr_turn_rightdown` from `svr_act.cpp`
///
/// Sets the character's facing direction to right-down and notifies nearby
/// observers about the change.
///
/// # Arguments
/// * `cn` - Character index rotating to face right-down
pub fn plr_turn_rightdown(gs: &mut GameState, cn: usize) {
    plr_turn(gs, cn, core::constants::DX_RIGHTDOWN);
}

/// Port of `plr_turn_rightup` from `svr_act.cpp`
///
/// Sets the character's facing direction to right-up and notifies nearby
/// observers about the change.
///
/// # Arguments
/// * `cn` - Character index rotating to face right-up
pub fn plr_turn_rightup(gs: &mut GameState, cn: usize) {
    plr_turn(gs, cn, core::constants::DX_RIGHTUP);
}

/// Port of `plr_turn_left` from `svr_act.cpp`
///
/// Sets the character's facing direction to left and notifies nearby
/// observers about the change.
///
/// # Arguments
/// * `cn` - Character index rotating to face left
pub fn plr_turn_left(gs: &mut GameState, cn: usize) {
    plr_turn(gs, cn, core::constants::DX_LEFT);
}

/// Port of `plr_turn_right` from `svr_act.cpp`
///
/// Sets the character's facing direction to right and notifies nearby
/// observers about the change.
///
/// # Arguments
/// * `cn` - Character index rotating to face right
pub fn plr_turn_right(gs: &mut GameState, cn: usize) {
    plr_turn(gs, cn, core::constants::DX_RIGHT);
}

/// Port of `plr_attack` from `svr_act.cpp`
///
/// Attempts to attack the tile directly in front of the character (based on
/// facing direction). If a valid target character `co` is present and matches
/// the currently set `attack_cn`, the server triggers `do_attack` to perform
/// combat logic. If the target moved away, a message is sent to the attacker.
///
/// # Arguments
/// * `cn` - Attacking character index
/// * `is_surround` - Surround flag passed to `do_attack` (0 or 1)
pub fn plr_attack(gs: &mut GameState, cn: usize, is_surround: bool) {
    notify_character_tile(gs, cn);

    let Some((m, x, y)) = plr_front_tile(gs, cn, "plr_attack") else {
        return;
    };

    let mut co = gs.map[m].ch as usize;

    if co == 0 {
        co = gs.map[m].to_ch as usize;
    }

    if co == 0 {
        let attack_cn = gs.characters[cn].attack_cn as usize;
        if attack_cn > 0
            && gs.characters[attack_cn].frx == x as i16
            && gs.characters[attack_cn].fry == y as i16
        {
            co = attack_cn;
        }
    }

    if co == 0 {
        gs.do_character_log(cn, core::types::FontColor::Red, "Your target moved away!\n");
        return;
    }

    let attack_cn = gs.characters[cn].attack_cn as usize;

    if attack_cn == co {
        gs.do_attack(cn, co, is_surround);
    }
}

/// Port of `plr_give` from `svr_act.cpp`
///
/// Attempts to give the currently carried item to the character in the tile
/// in front of the actor. If the target moved away or the direction is
/// invalid, an error is set; otherwise `do_give` is invoked to handle transfer
/// rules and client updates.
///
/// # Arguments
/// * `cn` - Giver character index
pub fn plr_give(gs: &mut GameState, cn: usize) {
    notify_character_tile(gs, cn);

    let Some((m, _, _)) = plr_front_tile(gs, cn, "plr_give") else {
        return;
    };

    let mut co = gs.map[m].ch as usize;

    if co == 0 {
        co = gs.map[m].to_ch as usize;
    }

    if co == 0 {
        gs.do_character_log(cn, core::types::FontColor::Red, "Your target moved away!\n");
        return;
    }

    gs.do_give(cn, co);
}

/// Emit a simple social action log for the acting character and nearby area.
///
/// # Arguments
/// * `gs` - Active game state used for notifications and logs.
/// * `cn` - Character index performing the action.
/// * `self_text` - Message shown to the acting character.
/// * `area_text` - Message template shown to nearby players.
/// * `log_verb` - Verb used for server logging.
fn plr_social_action(
    gs: &mut GameState,
    cn: usize,
    self_text: &str,
    area_text: &str,
    log_verb: &str,
) {
    let ch = gs.characters[cn];
    let reference = ch.get_reference();
    let area_message = area_text.replace("{}", reference);
    notify_character_tile(gs, cn);
    gs.do_character_log(cn, core::types::FontColor::Red, self_text);
    gs.do_area_log(
        cn,
        0,
        ch.x as i32,
        ch.y as i32,
        core::types::FontColor::Blue,
        &area_message,
    );

    log::info!("Character {} {}", cn, log_verb);

    gs.characters[cn].cerrno = core::constants::ERR_SUCCESS as u16;
}

/// Port of `plr_pickup` from `svr_act.cpp`
///
/// Handles picking up an item from the adjacent tile in the character's
/// facing direction. This checks for available slots, money vs items,
/// step-action items blocking pickup, and updates character inventory,
/// money, and lighting appropriately.
///
/// # Arguments
/// * `cn` - Character index attempting to pick up an item
pub fn plr_pickup(gs: &mut GameState, cn: usize) {
    notify_character_tile(gs, cn);

    let has_citem = gs.characters[cn].citem != 0;

    if has_citem {
        gs.characters[cn].cerrno = core::constants::ERR_FAILED as u16;
        return;
    }

    let ch = gs.characters[cn];
    let (m, x, y) = match ch.dir {
        core::constants::DX_UP if ch.y > 0 => {
            let m = (ch.x as usize) + ((ch.y - 1) as usize) * core::constants::SERVER_MAPX as usize;
            (Some(m), ch.x, ch.y - 1)
        }
        core::constants::DX_DOWN if ch.y < (core::constants::SERVER_MAPY as i16 - 1) => {
            let m = (ch.x as usize) + ((ch.y + 1) as usize) * core::constants::SERVER_MAPX as usize;
            (Some(m), ch.x, ch.y + 1)
        }
        core::constants::DX_LEFT if ch.x > 0 => {
            let m = ((ch.x - 1) as usize) + (ch.y as usize) * core::constants::SERVER_MAPX as usize;
            (Some(m), ch.x - 1, ch.y)
        }
        core::constants::DX_RIGHT if ch.x < (core::constants::SERVER_MAPX as i16 - 1) => {
            let m = ((ch.x + 1) as usize) + (ch.y as usize) * core::constants::SERVER_MAPX as usize;
            (Some(m), ch.x + 1, ch.y)
        }
        _ => (None, 0, 0),
    };

    let Some(m) = m else {
        gs.characters[cn].cerrno = core::constants::ERR_FAILED as u16;
        return;
    };

    let in_id = gs.map[m].it;

    if in_id == 0 {
        gs.characters[cn].cerrno = core::constants::ERR_FAILED as u16;
        return;
    }

    let can_take =
        (gs.items[in_id as usize].flags & core::constants::ItemFlags::IF_TAKE.bits()) != 0;

    if !can_take {
        gs.characters[cn].cerrno = core::constants::ERR_FAILED as u16;
        return;
    }

    gs.characters[cn].cerrno = core::constants::ERR_SUCCESS as u16;

    gs.do_update_char(cn);

    // Check if it's money
    let is_money =
        (gs.items[in_id as usize].flags & core::constants::ItemFlags::IF_MONEY.bits()) != 0;

    if is_money {
        let value = gs.items[in_id as usize].value;

        gs.characters[cn].gold += value as i32;

        let message = format!("You got {}G {}S\n", value / 100, value % 100);
        gs.do_character_log(cn, core::types::FontColor::Red, &message);

        log::info!("Character {} took {}G {}S", cn, value / 100, value % 100);

        gs.map[m].it = 0;

        let active = gs.items[in_id as usize].active;
        let light_active = gs.items[in_id as usize].light[1];
        let light_inactive = gs.items[in_id as usize].light[0];

        gs.items[in_id as usize].used = core::constants::USE_EMPTY;
        gs.items[in_id as usize].x = 0;
        gs.items[in_id as usize].y = 0;

        if active != 0 && light_active != 0 {
            gs.do_add_light(x as i32, y as i32, -(light_active as i32));
        } else if light_inactive != 0 {
            gs.do_add_light(x as i32, y as i32, -(light_inactive as i32));
        }

        return;
    }

    // Non-money item
    gs.map[m].it = 0;

    let is_player = (gs.characters[cn].flags & CharacterFlags::Player.bits()) != 0;

    if is_player {
        let mut slot_found = None;
        for n in 0..40 {
            if gs.characters[cn].item[n] == 0 {
                gs.characters[cn].item[n] = in_id;
                slot_found = Some(n);
                break;
            }
        }

        if slot_found.is_none() {
            gs.characters[cn].citem = in_id;
        }

        let item_name = gs.items[in_id as usize].get_name().to_string();

        log::info!("Character {} took {}", cn, item_name);
    } else {
        gs.characters[cn].citem = in_id;
    }

    let active = gs.items[in_id as usize].active;
    let light_active = gs.items[in_id as usize].light[1];
    let light_inactive = gs.items[in_id as usize].light[0];

    gs.items[in_id as usize].x = 0;
    gs.items[in_id as usize].y = 0;
    gs.items[in_id as usize].carried = cn as u16;

    if active != 0 && light_active != 0 {
        gs.do_add_light(x as i32, y as i32, -(light_active as i32));
    } else if light_inactive != 0 {
        gs.do_add_light(x as i32, y as i32, -(light_inactive as i32));
    }
}

/// Port of `plr_bow` from `svr_act.cpp`
///
/// Handles a social "bow" action: notifies nearby players with an area
/// notification and logs a message for the actor and area. Sets the
/// command result status to success.
///
/// # Arguments
/// * `cn` - Character index performing the bow
pub fn plr_bow(gs: &mut GameState, cn: usize) {
    plr_social_action(gs, cn, "You bow deeply.\n", "{} bows deeply.\n", "bows");
}

/// Port of `plr_wave` from `svr_act.cpp`
///
/// Handles a social "wave" action: notifies nearby players with an area
/// notification and logs a message for the actor and area. Sets the
/// command result status to success.
///
/// # Arguments
/// * `cn` - Character index performing the wave
pub fn plr_wave(gs: &mut GameState, cn: usize) {
    plr_social_action(
        gs,
        cn,
        "You wave happily.\n",
        "{} waves happily.\n",
        "waves",
    );
}

/// Port of `plr_use` from `svr_act.cpp`
///
/// Attempts to use an item placed on the adjacent tile in front of the
/// actor. Validates usage flags and, when implemented, would call the
/// `use_driver` to perform item-specific logic. Currently it validates
/// and logs debug information.
///
/// # Arguments
/// * `cn` - Character index using the item
pub fn plr_use(gs: &mut GameState, cn: usize) {
    notify_character_tile(gs, cn);

    let Some((m, _, _)) = plr_cardinal_front_tile(gs, cn) else {
        gs.characters[cn].cerrno = core::constants::ERR_FAILED as u16;
        return;
    };

    let in_id = gs.map[m].it;

    if in_id == 0 {
        gs.characters[cn].cerrno = core::constants::ERR_FAILED as u16;
        return;
    }

    let flags = gs.items[in_id as usize].flags;
    let can_use = (flags & core::constants::ItemFlags::IF_USE.bits()) != 0
        || (flags & core::constants::ItemFlags::IF_USESPECIAL.bits()) != 0;

    if !can_use {
        gs.characters[cn].cerrno = core::constants::ERR_FAILED as u16;
        return;
    }

    driver::use_driver(gs, cn, in_id as usize, false);
}

/// Port of `plr_skill` from `svr_act.cpp`
///
/// Triggers the skill driver for the character using the current
/// `skill_target2` value. Also sends an area notify for the action.
///
/// # Arguments
/// * `cn` - Character index using the skill
pub fn plr_skill(gs: &mut GameState, cn: usize) {
    notify_character_tile(gs, cn);

    let skill_target = gs.characters[cn].skill_target2;

    driver::skill_driver(gs, cn, skill_target as i32);
}

/// Port of `plr_drop` from `svr_act.cpp`
///
/// Drops the currently carried item (cursor/item in hand) onto the tile in
/// front of the character. Handles special cases for money (creates a
/// money-item template), building-mode drop semantics, step-action
/// blockages, and updates lighting and map item references accordingly.
///
/// # Arguments
/// * `cn` - Character index performing the drop
pub fn plr_drop(gs: &mut GameState, cn: usize) {
    notify_character_tile(gs, cn);

    let in_id = gs.characters[cn].citem;

    if in_id == 0 {
        return;
    }

    let Some((m, x, y)) = plr_cardinal_front_tile(gs, cn) else {
        gs.characters[cn].cerrno = core::constants::ERR_FAILED as u16;
        return;
    };

    // Check for step action items
    let in2 = gs.map[m].it;
    if in2 != 0 {
        let has_step_action =
            (gs.items[in2 as usize].flags & core::constants::ItemFlags::IF_STEPACTION.bits()) != 0;

        if has_step_action {
            driver::step_driver(gs, cn, in2 as usize);
            gs.characters[cn].cerrno = core::constants::ERR_FAILED as u16;
            return;
        }
    }

    // Check if tile is blocked
    let is_blocked = gs.map[m].ch != 0
        || gs.map[m].to_ch != 0
        || gs.map[m].it != 0
        || (gs.map[m].flags & core::constants::MF_MOVEBLOCK as u64) != 0;

    if is_blocked {
        gs.characters[cn].cerrno = core::constants::ERR_FAILED as u16;
        return;
    }

    gs.characters[cn].citem = 0;
    gs.characters[cn].cerrno = core::constants::ERR_SUCCESS as u16;

    gs.do_update_char(cn);

    // Handle money
    let final_in_id = if in_id & 0x80000000 != 0 {
        let tmp = in_id & 0x7FFFFFFF;
        let new_in = God::create_item(gs, 1); // blank template

        if new_in.is_none() {
            gs.characters[cn].cerrno = core::constants::ERR_FAILED as u16;
            return;
        }

        let new_in = new_in.expect("checked for item creation failure above");

        gs.items[new_in].flags |= core::constants::ItemFlags::IF_TAKE.bits()
            | core::constants::ItemFlags::IF_LOOK.bits()
            | core::constants::ItemFlags::IF_MONEY.bits();
        gs.items[new_in].value = tmp;
        let mut reference = [0u8; 40];
        let bytes = "some money".as_bytes();
        let len = bytes.len().min(40);
        reference[..len].copy_from_slice(&bytes[..len]);
        gs.items[new_in].reference = reference;

        let (description, sprite) = if tmp > 999999 {
            ("A huge pile of gold coins", 121)
        } else if tmp > 99999 {
            ("A very large pile of gold coins", 120)
        } else if tmp > 9999 {
            ("A large pile of gold coins", 41)
        } else if tmp > 999 {
            ("A small pile of gold coins", 40)
        } else if tmp > 99 {
            ("Some gold coins", 39)
        } else if tmp > 9 {
            ("A pile of silver coins", 38)
        } else if tmp > 2 {
            ("A few silver coins", 37)
        } else if tmp == 2 {
            ("A couple of silver coins", 37)
        } else {
            ("A lonely silver coin", 37)
        };

        let mut description_bytes = [0u8; 200];
        let bytes = description.as_bytes();
        let len = bytes.len().min(200);
        description_bytes[..len].copy_from_slice(&bytes[..len]);
        gs.items[new_in].description = description_bytes;
        gs.items[new_in].sprite[0] = sprite;

        log::info!("Character {} dropped {}G {}S", cn, tmp / 100, tmp % 100);

        new_in as u32
    } else {
        // Check whether the item is allowed to be given/dropped
        let may_drop = gs.do_maygive(cn, 0, in_id as usize);
        if !may_drop {
            // Restore cursor item and indicate failure
            gs.characters[cn].citem = in_id;
            gs.characters[cn].cerrno = core::constants::ERR_FAILED as u16;
            gs.do_character_log(
                cn,
                core::types::FontColor::Red,
                "You are not allowed to do that!\n",
            );
            return;
        }

        let item_name = gs.items[in_id as usize].get_name().to_string();
        log::info!("Character {} dropped {}", cn, item_name);
        in_id
    };

    gs.map[m].it = final_in_id;

    let active = gs.items[final_in_id as usize].active;
    let light_active = gs.items[final_in_id as usize].light[1];
    let light_inactive = gs.items[final_in_id as usize].light[0];

    gs.items[final_in_id as usize].x = x as u16;
    gs.items[final_in_id as usize].y = y as u16;
    gs.items[final_in_id as usize].carried = 0;

    if active != 0 && light_active != 0 {
        gs.do_add_light(x as i32, y as i32, light_active as i32);
    } else if light_inactive != 0 {
        gs.do_add_light(x as i32, y as i32, light_inactive as i32);
    }
}

/// Port of `plr_misc` from `svr_act.cpp`
///
/// Dispatches the character's misc action (`status2`) to the appropriate
/// action handler (attack, pickup, drop, give, use, bow, wave, skill, ...).
/// Sets character errno on unknown actions.
///
/// # Arguments
/// * `cn` - Character index whose misc action to process
pub fn plr_misc(gs: &mut GameState, cn: usize) {
    let status2 = gs.characters[cn].status2;
    let is_player = gs.characters[cn].is_player();

    match status2 {
        0 => {
            if is_player {
                log::debug!(
                    "plr_misc: attack action (is_surround=false), status2=0 for char {}",
                    cn
                );
            }
            plr_attack(gs, cn, false);
        }
        1 => {
            if is_player {
                log::debug!("plr_misc: pickup action for char {}", cn);
            }
            plr_pickup(gs, cn);
        }
        2 => {
            if is_player {
                log::debug!("plr_misc: drop action for char {}", cn);
            }
            plr_drop(gs, cn);
        }
        3 => {
            if is_player {
                log::debug!("plr_misc: give action for char {}", cn);
            }
            plr_give(gs, cn);
        }
        4 => {
            if is_player {
                log::debug!("plr_misc: use action for char {}", cn);
            }
            plr_use(gs, cn);
        }
        5 => {
            if is_player {
                log::debug!("plr_misc: attack action (is_surround=true) for char {}", cn);
            }
            plr_attack(gs, cn, true);
        }
        6 => {
            if is_player {
                log::debug!(
                    "plr_misc: attack action (is_surround=false) for char {}",
                    cn
                );
            }
            plr_attack(gs, cn, false);
        }
        7 => {
            if is_player {
                log::debug!("plr_misc: bow action for char {}", cn);
            }
            plr_bow(gs, cn);
        }
        8 => {
            if is_player {
                log::debug!("plr_misc: wave action for char {}", cn);
            }
            plr_wave(gs, cn);
        }
        9 => {
            if is_player {
                log::debug!("plr_misc: skill action for char {}", cn);
            }
            plr_skill(gs, cn);
        }
        _ => {
            log::error!("plr_misc: unknown status2 {} for char {}", status2, cn);
            gs.characters[cn].cerrno = core::constants::ERR_FAILED as u16;
        }
    }
}

/// Reset a character's animation status using an explicit game state.
///
/// # Arguments
/// * `gs` - Active game state used to mutate the character.
/// * `cn` - Character index whose status should be reset.
pub fn plr_reset_status(gs: &mut GameState, cn: usize) {
    gs.characters[cn].status = match gs.characters[cn].dir {
        core::constants::DX_UP => 0,
        core::constants::DX_DOWN => 1,
        core::constants::DX_LEFT => 2,
        core::constants::DX_RIGHT => 3,
        core::constants::DX_LEFTUP => 4,
        core::constants::DX_LEFTDOWN => 5,
        core::constants::DX_RIGHTUP => 6,
        core::constants::DX_RIGHTDOWN => 7,
        _ => {
            log::error!(
                "plr_reset_status: illegal value for dir: {} for char {}",
                gs.characters[cn].dir,
                cn
            );
            gs.characters[cn].dir = core::constants::DX_UP;
            0
        }
    };
}

/// Port of `plr_check_target` from `svr_act.cpp`
///
/// Checks whether a map tile is a valid target for placing a character or
/// item: it must not contain characters, and it must not be flagged as
/// movement-blocked; items on the tile are allowed only when they aren't
/// movement-blocking either.
///
/// # Arguments
/// * `m` - Map index to inspect
///
/// # Returns
/// `true` if tile is a valid empty target, `false` otherwise
pub fn plr_check_target(gs: &mut GameState, m: usize) -> bool {
    if gs.map[m].ch != 0 || gs.map[m].to_ch != 0 {
        return false;
    }

    if (gs.map[m].flags & core::constants::MF_MOVEBLOCK as u64) != 0 {
        return false;
    }

    let it_id = gs.map[m].it;
    if it_id != 0 {
        (gs.items[it_id as usize].flags & core::constants::ItemFlags::IF_MOVEBLOCK.bits()) == 0
    } else {
        true
    }
}

/// Port of `plr_set_target` from `svr_act.cpp`
///
/// Marks the provided map tile as targeted by character `cn` by setting
/// `to_ch`. Uses `plr_check_target` to validate the tile first.
///
/// # Arguments
/// * `m` - Map index to set as target
/// * `cn` - Character index that will be the target occupant
///
/// # Returns
/// `true` on success, `false` if tile is not a valid target
pub fn plr_set_target(gs: &mut GameState, m: usize, cn: usize) -> bool {
    if !plr_check_target(gs, m) {
        return false;
    }

    gs.map[m].to_ch = cn as u32;

    true
}

/// Perform the character's current driving action.
///
/// Resets status bits and calls the driver for the character if their
/// action group is active. This is the main per-tick driver entry for
/// active characters.
///
/// # Arguments
/// * `cn` - Character index to perform driver actions for
pub fn plr_doact(gs: &mut GameState, cn: usize) {
    plr_reset_status(gs, cn);
    if gs.characters[cn].group_active() {
        driver::driver(gs, cn);
    }
}
