/// Port of `plr_newlogin` from `svr_tick.cpp`
/// Handles new player login (stub - to be implemented)
fn plr_newlogin(gs: &mut GameState, nr: usize) {
    // Port of C++ `plr_newlogin` from `svr_tick.cpp`.

    // version check
    let version = gs.players[nr].version as u32;
    if version < core::constants::MINVERSION {
        log::warn!("Client too old ({}). Logout demanded", version);
        plr_logout(gs, 0, nr, LogoutReason::VersionMismatch);
        return;
    }

    // ban check
    let addr = gs.players[nr].addr;
    if God::is_banned(gs, addr as i32) {
        log::info!("Banned, sent away");
        plr_logout(gs, 0, nr, LogoutReason::Kicked);
        return;
    }

    // TODO: `cap()` handling (player cap/queue) not implemented yet.

    // sanitize race
    let mut temp = gs.players[nr].race;
    if temp != 2 && temp != 3 && temp != 4 && temp != 76 && temp != 77 && temp != 78 {
        temp = 2;
    }

    // create new character from template
    let maybe_cn = God::create_char(gs, temp as usize, true);
    let cn = match maybe_cn {
        Some(v) => v as usize,
        None => {
            log::error!("plr_newlogin: failed to create character");
            plr_logout(gs, 0, nr, LogoutReason::Failure);
            return;
        }
    };

    gs.characters[cn].player = nr as i32;
    gs.characters[cn].temple_x = core::constants::HOME_MERCENARY_X as u16;
    gs.characters[cn].temple_y = core::constants::HOME_MERCENARY_Y as u16;
    gs.characters[cn].tavern_x = core::constants::HOME_MERCENARY_X as u16;
    gs.characters[cn].tavern_y = core::constants::HOME_MERCENARY_Y as u16;
    gs.characters[cn].points = 0;
    gs.characters[cn].points_tot = 0;
    gs.characters[cn].luck = 205;

    gs.globals.players_created += 1;

    // Try dropping the character near the home temple (three attempts)
    if !God::drop_char_fuzzy_large(
        gs,
        cn,
        core::constants::HOME_MERCENARY_X as usize,
        core::constants::HOME_MERCENARY_Y as usize,
        core::constants::HOME_MERCENARY_X as usize,
        core::constants::HOME_MERCENARY_Y as usize,
    ) && !God::drop_char_fuzzy_large(
        gs,
        cn,
        (core::constants::HOME_MERCENARY_X + 3) as usize,
        core::constants::HOME_MERCENARY_Y as usize,
        core::constants::HOME_MERCENARY_X as usize,
        core::constants::HOME_MERCENARY_Y as usize,
    ) && !God::drop_char_fuzzy_large(
        gs,
        cn,
        core::constants::HOME_MERCENARY_X as usize,
        (core::constants::HOME_MERCENARY_Y + 3) as usize,
        core::constants::HOME_MERCENARY_X as usize,
        core::constants::HOME_MERCENARY_Y as usize,
    ) {
        log::error!("plr_newlogin(): could not drop new character");
        plr_logout(gs, cn, nr, LogoutReason::NoRoom);
        gs.characters[cn].used = core::constants::USE_EMPTY;
        return;
    }

    // Set creation/login dates and flags, record address and add to net history
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as u32;

    let ch = &mut gs.characters[cn];
    ch.creation_date = now;
    ch.login_date = now;
    ch.flags |= CharacterFlags::NewUser.bits() | CharacterFlags::Player.bits();
    ch.addr = gs.players[nr].addr;

    // char_add_net behaviour: shift data[80..89] and insert lower 24 bits of addr
    let net = (ch.addr & 0x00ffffff) as i32;
    let mut n = 80usize;
    while n < 89 {
        if (ch.data[n] & 0x00ffffff) == net {
            break;
        }
        n += 1;
    }
    for m in (81..=n).rev() {
        ch.data[m] = ch.data[m - 1];
    }
    ch.data[80] = net;

    ch.mode = 1;

    // update character to clients
    gs.do_update_char(cn);

    // set player mapping and send SV_NEWPLAYER + SV_TICK
    let pass1 = gs.characters[cn].pass1;
    let pass2 = gs.characters[cn].pass2;

    gs.players[nr].usnr = cn;
    gs.players[nr].pass1 = pass1;
    gs.players[nr].pass2 = pass2;

    log::info!(
        "New player logged in as character index={} (players index={})",
        cn,
        nr
    );

    let mut buf: [u8; 16] = [0; 16];
    buf[0] = ServerCommandType::NewPlayer as u8;
    buf[1..5].copy_from_slice(&(cn as u32).to_le_bytes());
    buf[5..9].copy_from_slice(&pass1.to_le_bytes());
    buf[9..13].copy_from_slice(&pass2.to_le_bytes());
    let ver_bytes = core::constants::VERSION.to_le_bytes();
    buf[13] = ver_bytes[0];
    buf[14] = ver_bytes[1];
    buf[15] = ver_bytes[2];

    network_manager::csend(gs, nr, &buf, 16);

    // finalize player state
    let ticker = gs.globals.ticker as u32;
    gs.players[nr].state = core::constants::ST_NORMAL;
    gs.players[nr].lasttick = ticker;
    gs.players[nr].ltick = 0;
    gs.players[nr].ticker_started = 1;

    // send tick
    let mut tbuf: [u8; 2] = [0; 2];
    tbuf[0] = ServerCommandType::Tick as u8;
    tbuf[1] = (gs.globals.ticker as usize % core::constants::CTICK_CYCLE_LEN) as u8;
    network_manager::xsend(gs, nr, &tbuf, 2);

    log::info!("Created new character");

    // intro messages
    let intro1 = "Welcome to Men Among Gods, my friend!\n";
    let intro2 = "May your visit here be... interesting.\n";
    let intro3 = " \n";
    let intro4 = "Use #help (or /help) to get a listing of the text commands.\n";

    gs.do_character_log(cn, core::types::FontColor::Yellow, intro1);
    gs.do_character_log(cn, core::types::FontColor::Yellow, intro3);
    gs.do_character_log(cn, core::types::FontColor::Yellow, intro2);
    gs.do_character_log(cn, core::types::FontColor::Yellow, intro3);
    gs.do_character_log(cn, core::types::FontColor::Yellow, intro4);
    gs.do_character_log(cn, core::types::FontColor::Yellow, intro3);

    // change password if client provided one and character has no CF_PASSWD
    let needs_pass = gs.players[nr].passwd[0] != 0;
    if needs_pass {
        if (gs.characters[cn].flags & CharacterFlags::Passwd.bits()) == 0 {
            let pass = c_string_to_str(&gs.players[nr].passwd).to_string();
            God::change_pass(gs, cn, cn, &pass);
        }
    }

    // announce
    gs.do_announce(cn, 0, "A new player has entered the game.\n");
}

/// Port of `plr_login` from `svr_tick.cpp`
/// Handles existing player login (stub - to be implemented)
fn plr_login(gs: &mut GameState, nr: usize) {
    // version check
    let version = gs.players[nr].version as u32;
    if version < core::constants::MINVERSION {
        log::warn!("Client too old ({}). Logout demanded", version);
        plr_logout(gs, 0, nr, LogoutReason::VersionMismatch);
        return;
    }

    let login_ticket = gs.players[nr].login_ticket;
    let mut is_api_login = false;
    if login_ticket != 0 {
        is_api_login = true;
        let cn = match resolve_api_login_character(gs, nr, login_ticket) {
            Ok(cn) => cn,
            Err(reason) => {
                log::warn!("API login denied: {:?}", reason);
                plr_logout(gs, 0, nr, reason);
                return;
            }
        };

        let (pass1, pass2) = (gs.characters[cn].pass1, gs.characters[cn].pass2);

        gs.players[nr].usnr = cn;
        gs.players[nr].pass1 = pass1;
        gs.players[nr].pass2 = pass2;
        gs.players[nr].login_ticket = 0;
    }

    // get character number requested by player
    let cn = gs.players[nr].usnr;

    if cn == 0 || cn >= core::constants::MAXCHARS {
        log::warn!("Login as {} denied (illegal cn)", cn);
        plr_logout(gs, 0, nr, LogoutReason::ParamsInvalid);
        return;
    }

    if !is_api_login {
        // password/pass1/pass2 check
        let pass_ok = {
            let ch = gs.characters[cn];
            let p1 = ch.pass1;
            let p2 = ch.pass2;
            let player_p1 = gs.players[nr].pass1;
            let player_p2 = gs.players[nr].pass2;
            p1 == player_p1 && p2 == player_p2
        };

        if !pass_ok {
            log::warn!("Login as {} denied (pass1/pass2)", cn);
            plr_logout(gs, 0, nr, LogoutReason::PasswordIncorrect);
            return;
        }

        // If character has explicit password flag, compare stored passwd
        let has_passwd_mismatch = {
            let ch = gs.characters[cn];
            if (ch.flags & CharacterFlags::Passwd.bits()) != 0 {
                let stored = ch.passwd;
                let client = gs.players[nr].passwd;
                stored != client
            } else {
                false
            }
        };

        if has_passwd_mismatch {
            log::warn!("Login as {} denied (password)", cn);
            plr_logout(gs, 0, nr, LogoutReason::PasswordIncorrect);
            return;
        }
    }

    // Deleted account
    let is_deleted = gs.characters[cn].used == core::constants::USE_EMPTY;
    if is_deleted {
        log::warn!("Login as {} denied (deleted)", cn);
        plr_logout(gs, 0, nr, LogoutReason::PasswordIncorrect);
        return;
    }

    // Already active
    // C behavior:
    //   if (ch[cn].used != USE_NONACTIVE && !(ch[cn].flags & CF_CCP)) {
    //       plr_logout(cn, ch[cn].player, LO_IDLE);
    //   }
    // and then continue the login (no early return).
    let already_active = gs.characters[cn].used != core::constants::USE_NONACTIVE
        && (gs.characters[cn].flags & CharacterFlags::ComputerControlledPlayer.bits()) == 0;
    if already_active {
        log::warn!("Login as {} who is already active", cn);
        let active_player = gs.characters[cn].player as usize;
        // Only kick the *other* active player if they still have a live socket.
        // A stale `ch.player` binding can happen after disconnects; never kick ourselves.
        let should_kick = active_player != 0
            && active_player != nr
            && active_player < core::constants::MAXPLAYER
            && gs.players[active_player].sock.is_some();
        if should_kick {
            plr_logout(gs, cn, active_player, LogoutReason::IdleTooLong);
        } else {
            log::warn!(
                "Already-active character {} has stale/invalid active_player={} (current_player={}); continuing",
                cn,
                active_player,
                nr
            );
        }
    }

    // Kicked — deny this reconnection attempt and clear the flag so the player
    // can log back in on a subsequent try.  The kick has already disconnected
    // them; the flag only needs to block the one immediate reconnect.
    let is_kicked = (gs.characters[cn].flags & CharacterFlags::Kicked.bits()) != 0;
    if is_kicked {
        log::warn!("Login as {} denied (kicked)", cn);
        gs.characters[cn].flags &= !CharacterFlags::Kicked.bits();
        plr_logout(gs, 0, nr, LogoutReason::Kicked);
        return;
    }

    // Ban check (skip golden/god)
    let banned = gs.players[nr].addr;
    let exempt = (gs.characters[cn].flags
        & (CharacterFlags::Golden.bits() | CharacterFlags::God.bits()))
        != 0;
    if !exempt && God::is_banned(gs, banned as i32) {
        log::info!("{} is banned, sent away", cn);
        plr_logout(gs, 0, nr, LogoutReason::Kicked);
        return;
    }

    // TODO: cap() handling (player cap/queue) not implemented - skip

    // attach player to character
    gs.characters[cn].player = nr as i32;
    // Ensure the logged-in entity is treated as a player character.
    // API-created characters are spawned from templates and may not carry the Player flag,
    // which would break `/who` visibility and command processing.
    gs.characters[cn].flags |= CharacterFlags::Player.bits();
    // If not CCP and is god, mark invisible
    if (gs.characters[cn].flags & CharacterFlags::ComputerControlledPlayer.bits()) == 0
        && (gs.characters[cn].flags & CharacterFlags::God.bits()) != 0
    {
        gs.characters[cn].flags |= CharacterFlags::Invisible.bits();
    }

    // finalize player state
    let ticker = gs.globals.ticker as u32;
    gs.players[nr].state = core::constants::ST_NORMAL;
    gs.players[nr].lasttick = ticker;
    gs.players[nr].ltick = 0;
    gs.players[nr].ticker_started = 1;

    // send LOGIN_OK
    let mut buf: [u8; 16] = [0; 16];
    buf[0] = ServerCommandType::LoginOk as u8;
    buf[1..5].copy_from_slice(&core::constants::VERSION.to_le_bytes());
    network_manager::csend(gs, nr, &buf, 16);

    // send tick
    let mut tbuf: [u8; 2] = [0; 2];
    tbuf[0] = ServerCommandType::Tick as u8;
    tbuf[1] = (gs.globals.ticker as usize % core::constants::CTICK_CYCLE_LEN) as u8;
    network_manager::xsend(gs, nr, &tbuf, 2);

    // mark active and set login date, addr, add net history
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as u32;

    let ch = &mut gs.characters[cn];
    ch.used = core::constants::USE_ACTIVE;
    ch.login_date = now;
    ch.addr = gs.players[nr].addr;
    ch.current_online_time = 0;

    // char_add_net behaviour: shift data[80..89] and insert lower 24 bits
    let net = (ch.addr & 0x00ffffff) as i32;
    let mut nidx = 80usize;
    while nidx < 89 {
        if (ch.data[nidx] & 0x00ffffff) == net {
            break;
        }
        nidx += 1;
    }
    for m in (81..=nidx).rev() {
        ch.data[m] = ch.data[m - 1];
    }
    ch.data[80] = net;

    // ensure client player mode default
    gs.players[nr].cpl.mode = -1;

    // Try to drop character at tavern/nearby
    let tav_x = gs.characters[cn].tavern_x as usize;
    let tav_y = gs.characters[cn].tavern_y as usize;
    if !God::drop_char_fuzzy_large(gs, cn, tav_x, tav_y, tav_x, tav_y)
        && !God::drop_char_fuzzy_large(gs, cn, tav_x + 3, tav_y, tav_x, tav_y)
        && !God::drop_char_fuzzy_large(gs, cn, tav_x, tav_y + 3, tav_x, tav_y)
    {
        log::error!("plr_login(): could not drop new character");
        plr_logout(gs, cn, nr, LogoutReason::NoRoom);
        return;
    }

    // remove illegal active recall spells
    for i in 0..20usize {
        let has_recall = gs.characters[cn].spell[i] != 0;
        if has_recall {
            let spell_idx = gs.characters[cn].spell[i] as usize;
            let is_recall = gs.items[spell_idx].temp == skills::SK_RECALL as u16;
            if is_recall {
                gs.items[spell_idx].used = core::constants::USE_EMPTY;
                gs.characters[cn].spell[i] = 0;
                gs.do_character_log(
                    cn,
                    core::types::FontColor::Red,
                    "CHEATER: removed active teleport\n",
                );
            }
        }
    }

    // update client about char
    gs.do_update_char(cn);

    log::info!("Login successful");

    // intro messages
    let intro1 = "Welcome to Men Among Gods, my friend!\n";
    let intro2 = "May your visit here be... interesting.\n";
    let intro3 = "\n";
    let intro4 = "Use #help (or /help) to get a listing of the text commands.\n";
    let mut message_of_the_day = gs.latest_message_of_the_day();
    if !message_of_the_day.is_empty() && !message_of_the_day.ends_with('\n') {
        message_of_the_day.push('\n');
    }

    gs.do_character_log(cn, core::types::FontColor::Yellow, intro1);
    gs.do_character_log(cn, core::types::FontColor::Yellow, intro3);
    gs.do_character_log(cn, core::types::FontColor::Yellow, intro2);
    gs.do_character_log(cn, core::types::FontColor::Yellow, intro3);
    gs.do_character_log(cn, core::types::FontColor::Yellow, intro4);
    gs.do_character_log(cn, core::types::FontColor::Yellow, intro3);
    if !message_of_the_day.trim().is_empty() {
        gs.do_character_log(cn, core::types::FontColor::Yellow, "Message of the Day:\n");
        gs.do_character_log(cn, core::types::FontColor::Yellow, &message_of_the_day);
        gs.do_character_log(cn, core::types::FontColor::Yellow, intro3);
    }

    if !is_api_login {
        // do password change if provided
        let needs_pass = gs.players[nr].passwd[0] != 0;
        if needs_pass {
            if (gs.characters[cn].flags & CharacterFlags::Passwd.bits()) == 0 {
                let pass = c_string_to_str(&gs.players[nr].passwd).to_string();
                God::change_pass(gs, cn, cn, &pass);
            }
        }
    }

    // If god, remind invisibility
    if (gs.characters[cn].flags & CharacterFlags::ComputerControlledPlayer.bits()) == 0
        && (gs.characters[cn].flags & CharacterFlags::God.bits()) != 0
    {
        gs.do_character_log(
            cn,
            core::types::FontColor::Blue,
            "Remember, you are invisible!\n",
        );
    }

    // announce
    let name = gs.characters[cn].get_name().to_string();
    gs.do_announce(cn, 0, &format!("{} entered the game.\n", name));
}

fn resolve_api_login_character(
    gs: &mut GameState,
    nr: usize,
    login_ticket: u64,
) -> Result<usize, LogoutReason> {
    let character_id = match keydb::consume_login_ticket(login_ticket) {
        Ok(Some(value)) => value,
        Ok(None) => {
            log::warn!("API login ticket not found or expired");
            return Err(LogoutReason::PasswordIncorrect);
        }
        Err(err) => {
            log::error!("KeyDB ticket consume failed: {}", err);
            return Err(LogoutReason::Failure);
        }
    };

    let character = match keydb::load_character(character_id) {
        Ok(Some(value)) => value,
        Ok(None) => {
            log::warn!("API character {} not found", character_id);
            return Err(LogoutReason::PasswordIncorrect);
        }
        Err(err) => {
            log::error!("KeyDB character load failed: {}", err);
            return Err(LogoutReason::Failure);
        }
    };

    let is_brand_new_character = character.server_id.is_none();

    let cn = match character.server_id {
        Some(server_id) => {
            let candidate = server_id as usize;
            let candidate_is_valid = candidate > 0
                && candidate < core::constants::MAXCHARS
                && gs.characters[candidate].used != core::constants::USE_EMPTY;

            if !candidate_is_valid {
                log::error!(
                    "API character {} has invalid/stale server_id={} (slot missing or empty)",
                    character_id,
                    server_id
                );
                return Err(LogoutReason::Failure);
            }

            candidate
        }
        None => {
            let template_id = get_race_integer(character.sex == Sex::Male, character.class);
            let maybe_cn = God::create_char(gs, template_id as usize, true);
            let cn = match maybe_cn {
                Some(value) => value as usize,
                None => {
                    log::error!("Failed to create character for API id {}", character_id);
                    return Err(LogoutReason::Failure);
                }
            };

            write_ascii_into_fixed(&mut gs.characters[cn].name, &character.name);
            gs.characters[cn].reference = gs.characters[cn].name;
            write_ascii_into_fixed(&mut gs.characters[cn].description, &character.description);

            // Characters created from templates start out "in use" (often `USE_ACTIVE`) because
            // templates represent live world entities. For API-created player characters, we
            // want them to begin offline so the normal login path can attach and activate them.
            gs.characters[cn].used = core::constants::USE_NONACTIVE;
            gs.characters[cn].player = 0;

            if is_brand_new_character {
                // API login does NOT go through `plr_newlogin`, so first-time characters
                // need the same baseline initialization (home temple/tavern, base stats).
                // Without this, `plr_login` can try to drop at (0,0).
                gs.characters[cn].temple_x = core::constants::HOME_MERCENARY_X as u16;
                gs.characters[cn].temple_y = core::constants::HOME_MERCENARY_Y as u16;
                gs.characters[cn].tavern_x = core::constants::HOME_MERCENARY_X as u16;
                gs.characters[cn].tavern_y = core::constants::HOME_MERCENARY_Y as u16;
                gs.characters[cn].points = 0;
                gs.characters[cn].points_tot = 0;
                gs.characters[cn].luck = 205;
                gs.characters[cn].mode = 1;

                // Mark as a player/new user in the same way as `plr_newlogin`.
                gs.characters[cn].flags |=
                    CharacterFlags::NewUser.bits() | CharacterFlags::Player.bits();
            }

            cn
        }
    };

    // Always sync the most recent API-side name/description into the live character slot.
    // This fixes older characters that were created before description persistence and ensures
    // updates made via the API are reflected on the server.
    write_ascii_into_fixed(&mut gs.characters[cn].name, &character.name);
    gs.characters[cn].reference = gs.characters[cn].name;

    let desc = if character.description.trim().is_empty() {
        gs.characters[cn].get_default_description()
    } else {
        character.description.clone()
    };
    write_ascii_into_fixed(&mut gs.characters[cn].description, &desc);

    if is_brand_new_character {
        if let Err(err) = keydb::set_character_server_id(character_id, cn as u32) {
            log::warn!(
                "Failed to persist server_id for API character {}: {}",
                character_id,
                err
            );
        }
    }

    gs.players[nr].api_character_id = character_id;

    if let Err(err) = keydb::sync_character_selection_metadata(character_id, &gs.characters[cn]) {
        log::warn!(
            "Failed to sync selection metadata for API character {}: {}",
            character_id,
            err
        );
    }

    Ok(cn)
}

/// Port of `plr_logout(int cn, int player_id, LogoutReason reason)` from `svr_tick.cpp`
///
/// Handles player logout and cleanup: saves state, removes the player
/// from maps, clears usurp/stoned flags, notifies the client (unless
/// `Usurp`), and applies any exit punishments depending on `reason`.
///
/// # Arguments
/// * `character_id` - Character index being logged out (0 if none, interpreted as "no character")
/// * `player_id` - Associated player slot id (0 if none, interpreted as "any player")
/// * `reason` - Reason for logout (enum)
pub fn plr_logout(gs: &mut GameState, character_id: usize, player_id: usize, reason: LogoutReason) {
    let player_id = if player_id < core::constants::MAXPLAYER {
        player_id
    } else {
        0
    };
    let valid_character = character_id > 0 && character_id < core::constants::MAXCHARS;

    if valid_character && reason != LogoutReason::Shutdown {
        let character_name = gs.characters[character_id].get_name().to_string();
        log::debug!(
            "Logging out character '{}' for reason: {:?}",
            character_name,
            reason
        );
    }

    let character_matches_player = valid_character
        && (player_id == 0 || gs.characters[character_id].player == player_id as i32);

    // Handle usurp flag and recursive logout
    if character_matches_player {
        let character = &mut gs.characters[character_id];
        let should_logout_co = if character.flags & CharacterFlags::Usurp.bits() != 0 {
            character.flags &= !(CharacterFlags::ComputerControlledPlayer
                | CharacterFlags::Usurp
                | CharacterFlags::Staff
                | CharacterFlags::Immortal
                | CharacterFlags::God
                | CharacterFlags::Creator)
                .bits();
            Some(character.data[97] as usize)
        } else {
            None
        };

        if let Some(co) = should_logout_co {
            plr_logout(gs, co, 0, LogoutReason::Shutdown);
        }
    }

    // Main logout logic for active players
    if character_matches_player {
        let character_flags = gs.characters[character_id].flags;
        let (is_player, is_not_ccp) = (
            character_flags & CharacterFlags::Player.bits() != 0,
            character_flags & CharacterFlags::ComputerControlledPlayer.bits() == 0,
        );

        if is_player && is_not_ccp {
            let name = gs.characters[character_id].get_name().to_string();

            // Handle exit punishment
            if reason == LogoutReason::Exit {
                log::warn!(
                    "Character '{}' punished for leaving the game by means of F12.",
                    gs.characters[character_id].get_name(),
                );
                let hp5 = gs.characters[character_id].hp[5];
                let damage_message = format!(
                    "You have been hit by a demon. You lost {} HP.\n",
                    (hp5 * 8 / 10)
                );
                gs.do_character_log(character_id, core::types::FontColor::Red, " \n");
                gs.do_character_log(
                    character_id,
                    core::types::FontColor::Red,
                    "You are being punished for leaving the game without entering a tavern:\n",
                );
                gs.do_character_log(character_id, core::types::FontColor::Red, " \n");
                gs.do_character_log(
                    character_id,
                    core::types::FontColor::Red,
                    damage_message.as_str(),
                );

                gs.characters[character_id].a_hp -= (hp5 * 800) as i32;
                let a_hp = gs.characters[character_id].a_hp;

                if a_hp < 500 {
                    gs.do_character_log(
                        character_id,
                        core::types::FontColor::Red,
                        String::from("The demon killed you.\n \n").as_str(),
                    );
                    gs.do_character_killed(character_id, 0, false);
                } else {
                    let gold_tenth = gs.characters[character_id].gold / 10;
                    if gold_tenth > 0 {
                        let money_stolen_message = format!(
                            " \nA demon grabs your purse and removes {} gold, and {} silver.\n",
                            gold_tenth / 100,
                            gold_tenth % 100
                        );

                        gs.do_character_log(
                            character_id,
                            core::types::FontColor::Red,
                            money_stolen_message.as_str(),
                        );
                        gs.characters[character_id].gold -= gold_tenth;

                        // In the original protocol, the high bit marks "money in hand".
                        let citem = gs.characters[character_id].citem;
                        if citem != 0 && (citem & 0x80000000) != 0 {
                            gs.do_character_log(
                                character_id,
                                core::types::FontColor::Red,
                                "The demon also takes the money in your hand!\n",
                            );

                            gs.characters[character_id].citem = 0;
                        }
                    }
                }
            }

            // Clear map positions
            let ch = gs.characters[character_id];
            let (map_index, to_map_index, light, character_x, character_y) = (
                (ch.y as usize) * core::constants::SERVER_MAPX as usize + (ch.x as usize),
                (ch.toy as usize) * core::constants::SERVER_MAPX as usize + (ch.tox as usize),
                ch.light,
                ch.x,
                ch.y,
            );

            let ch_was_here = gs.map[map_index].ch == character_id as u32;
            if ch_was_here {
                gs.map[map_index].ch = 0;
                if light != 0 {
                    gs.do_add_light(character_x as i32, character_y as i32, -(light as i32));
                }
            }
            if gs.map[to_map_index].to_ch == character_id as u32 {
                gs.map[to_map_index].to_ch = 0;
            }

            // Remove references to this character from other enemies lists.
            gs.remove_enemy(character_id);

            // Handle lag scroll
            if reason == LogoutReason::IdleTooLong
                || reason == LogoutReason::Shutdown
                || reason == LogoutReason::Unknown
            {
                let ch = gs.characters[character_id];
                let (is_close_to_temple, map_index) = (
                    ch.is_close_to_temple(),
                    (ch.y as usize) * core::constants::SERVER_MAPX as usize + (ch.x as usize),
                );

                let should_give = if !is_close_to_temple {
                    gs.map[map_index].flags & core::constants::MF_NOLAG as u64 == 0
                } else {
                    false
                };

                if should_give {
                    log::info!(
                        "Giving lag scroll to character '{}' for idle/logout too long.",
                        gs.characters[character_id].get_name(),
                    );

                    if let Some(item_id) =
                        God::create_item(gs, core::constants::IT_LAGSCROLL as usize)
                    {
                        let (char_x, char_y) =
                            (gs.characters[character_id].x, gs.characters[character_id].y);

                        gs.items[item_id].data[0] = char_x as u32;
                        gs.items[item_id].data[1] = char_y as u32;
                        gs.items[item_id].data[2] = gs.globals.ticker as u32;

                        God::give_character_item(gs, character_id, item_id);
                    } else {
                        log::error!(
                            "Failed to create lag scroll for character '{}'.",
                            gs.characters[character_id].get_name(),
                        );
                    }
                }
            }

            // Reset character state
            {
                let character = &mut gs.characters[character_id];
                character.x = 0;
                character.y = 0;
                character.tox = 0;
                character.toy = 0;
                character.frx = 0;
                character.fry = 0;
                character.player = 0;
                character.status = 0;
                character.status2 = 0;
                // C++ resets dir to 1.
                character.dir = 1;
                character.escape_timer = 0;
                for i in 0..4 {
                    character.enemy[i] = 0;
                }
                character.attack_cn = 0;
                character.skill_nr = 0;
                character.goto_x = 0;
                character.goto_y = 0;
                character.use_nr = 0;
                character.misc_action = 0;
                character.stunned = 0;
                character.retry = 0;

                for i in 0..13 {
                    if i == 11 {
                        continue;
                    }
                    character.data[i] = 0;
                }

                character.data[96] = 0;
                character.used = core::constants::USE_NONACTIVE;
                character.logout_date = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs() as u32;

                character.flags |= CharacterFlags::SaveMe.bits();
            }
            if gs.characters[character_id].is_building() {
                God::build(gs, character_id, 0);
            }

            gs.do_announce(character_id, 0, &format!("{} left the game.\n", name));
        }
    }

    // Send exit message to player (when applicable), and always finalize/clear the player slot.
    //
    // Important: for disconnects (`Unknown`) we still need to run `player_exit` to clear any
    // stale `ch.player` mapping, otherwise later logins can incorrectly think the character is
    // already active and kick the new connection.
    if player_id != 0 {
        if reason != LogoutReason::Unknown && reason != LogoutReason::Usurp {
            let mut buffer: [u8; 16] = [0; 16];
            buffer[0] = ServerCommandType::Exit as u8;
            buffer[1] = reason as u8;

            let player_state = gs.players[player_id].state;

            if player_state == core::constants::ST_NORMAL {
                network_manager::xsend(gs, player_id, &buffer, 2);
            } else {
                network_manager::csend(gs, player_id, &buffer, 2);
            }
        }

        player_exit(gs, player_id);
    }
}

/// Finalize player exit operations and clear player slot state.
///
/// Called after `plr_logout` to complete exit bookkeeping: updates the
/// player's state, clears `ch.player`, and records the last tick.
///
/// # Arguments
/// * `gs` - Active game state used to clear character ownership.
/// * `player_id` - Player slot index
pub fn player_exit(gs: &mut GameState, player_id: usize) {
    if player_id == 0 || player_id >= core::constants::MAXPLAYER {
        log::error!("player_exit: Invalid player id {}", player_id);
        return;
    }

    let ticker = gs.globals.ticker as u32;

    gs.players[player_id].state = core::constants::ST_EXIT;
    gs.players[player_id].lasttick = ticker;
    gs.players[player_id].api_character_id = 0;

    let maybe_char = gs
        .characters
        .iter_mut()
        .find(|ch| ch.player as usize == player_id);

    if let Some(char) = maybe_char {
        log::info!(
            "Player {} exiting for character '{}'",
            player_id,
            char.get_name()
        );

        char.player = 0;
    }
}

/// Port of `plr_challenge_newlogin` from `svr_tick.cpp`
///
/// Initiates a new-login challenge for a connecting client. Generates a random
/// non-zero challenge, stores it on `players[nr]`, sets the player's state to
/// `ST_NEW_CHALLENGE`, timestamps `lasttick`, sends the `SV_CHALLENGE` packet
/// to the client, and sends mod data packets.
///
/// # Arguments
/// * `nr` - Player slot index to challenge
fn plr_challenge_newlogin(gs: &mut GameState, nr: usize) {
    // Generate random challenge value (0x3fffffff max, ensure non-zero)
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
///
/// Verifies the client's response to a previously issued challenge. Reads the
/// response, client version, and race from the inbuf, stores version/race on
/// the player record, validates the response using `xcrypt`, and moves the
/// player through the login state machine on success (or logs them out on
/// failure).
///
/// # Arguments
/// * `nr` - Player slot index handling the challenge response
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

    // Verify the challenge response
    if response != xcrypt(challenge) {
        log::warn!("Player {} challenge failed", nr);
        let usnr = gs.players[nr].usnr;
        plr_logout(gs, usnr, nr, LogoutReason::ChallengeFailed);
        return;
    }

    let ticker = gs.globals.ticker as u32;

    // Update state based on current state
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

/// Handle existing login challenge (port of `plr_challenge_login`)
///
/// Generates a random non-zero challenge, sets the player into the
/// `ST_LOGIN_CHALLENGE` state, validates the requested character index
/// supplied by the client, stores `pass1`/`pass2` fragments and sends the
/// challenge (and mod packets) back to the client.
fn plr_challenge_login(gs: &mut GameState, nr: usize) {
    log::debug!("Player {} challenge_login", nr);

    // Generate random challenge value (0x3fffffff max, ensure non-zero)
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

    // Store chosen character and pass fragments
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
///
/// The client sends `CL_API_LOGIN` with a u64 one-time ticket in the payload.
/// We store the ticket on the player slot and then proceed with the normal
/// `SV_CHALLENGE` / `CL_CHALLENGE` handshake.
fn plr_challenge_api_login(gs: &mut GameState, nr: usize) {
    log::debug!("Player {} challenge_api_login", nr);

    // Generate random challenge value (0x3fffffff max, ensure non-zero)
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

/// Port of `plr_unique` from `svr_tick.cpp`
///
/// Receives the client's unique 8-byte identifier or generates a server-side
/// unique if the client provided none. The server stores the value in
/// `players[nr].unique` and echoes back a generated unique when applicable.
///
/// # Arguments
/// * `nr` - Player slot index sending the unique
fn plr_unique(gs: &mut GameState, nr: usize) {
    // Read unique ID from inbuf (8 bytes as u64)
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

    // If client doesn't have a unique ID, generate one
    if unique == 0 {
        gs.globals.unique = gs.globals.unique.wrapping_add(1);
        let new_unique = gs.globals.unique;

        gs.players[nr].unique = new_unique;

        // Send the new unique ID back to the client
        let mut buf: [u8; 16] = [0; 16];
        buf[0] = ServerCommandType::Unique as u8;
        buf[1..9].copy_from_slice(&new_unique.to_le_bytes());

        network_manager::xsend(gs, nr, &buf, 9);

        log::debug!("Player {} sent unique {:016X}", nr, new_unique);
    }
}

/// Port of `plr_passwd` from `svr_tick.cpp`
///
/// Receives a password fragment from the client and stores it in the
/// player's `passwd` buffer (15 bytes). Computes a lightweight hash for
/// debug/logging parity with original server behavior.
///
/// # Arguments
/// * `nr` - Player slot index sending the password fragment
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

/// Port of `send_mod` from `svr_tick.cpp`
/// Sends mod data to the client (8 packets of 15 bytes each)
fn send_mod(gs: &mut GameState, nr: usize) {
    // TODO: Implement mod sending when mod data is available
    // For now, this is a stub - mod data would be loaded from somewhere
    // In the original code, this sends 8 SV_MOD packets with mod data
    let _mod_data: [u8; 120] = [0; 120]; // placeholder

    for n in 0..8u8 {
        let mut buf: [u8; 16] = [0; 16];
        buf[0] = ServerCommandType::Mod1 as u8 + n;
        // Copy 15 bytes of mod data (placeholder zeros for now)
        // buf[1..16].copy_from_slice(&mod_data[(n as usize * 15)..((n as usize + 1) * 15)]);

        network_manager::csend(gs, nr, &buf, 16);
    }
}

/// Port of `plr_perf_report` from `svr_tick.cpp`
///
/// Parses a client's performance/timing report and uses it to refresh the
/// player's network timeout (`lasttick`). The metric values are parsed for
/// completeness but currently not acted upon.
///
/// # Arguments
/// * `nr` - Player slot index reporting performance
fn plr_perf_report(gs: &mut GameState, nr: usize) {
    // Read performance metrics from inbuf (unused but parsed for completeness)
    let _ticksize = u16::from_le_bytes([gs.players[nr].inbuf[1], gs.players[nr].inbuf[2]]);
    let _skip = u16::from_le_bytes([gs.players[nr].inbuf[3], gs.players[nr].inbuf[4]]);
    let _idle = u16::from_le_bytes([gs.players[nr].inbuf[5], gs.players[nr].inbuf[6]]);

    let ticker = gs.globals.ticker as u32;
    gs.players[nr].lasttick = ticker;

    // Optional: log performance metrics (commented out in original)
    // log::trace!("Player {} perf: ticksize={}, skip={}%, idle={}%", nr, ticksize, skip, idle);
}
