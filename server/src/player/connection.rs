use core::{
    ban_store::BanTarget,
    constants::CharacterFlags,
    logout_reasons::LogoutReason,
    server_commands::ServerCommandType,
    skills,
    string_operations::write_ascii_into_fixed,
    traits::get_race_integer,
    types::{CharacterSummary, Sex, api::GameLoginTicketMetadata},
};

use server::keydb::connection as keydb;

use crate::{game_state::GameState, god::God, network_manager};

/// Port of `plr_login` from `svr_tick.cpp`
/// Handles existing player login (stub - to be implemented)
pub fn plr_login(gs: &mut GameState, nr: usize) {
    let login_ticket = gs.players[nr].login_ticket;
    if login_ticket == 0 {
        log::warn!("Login attempt without API ticket; rejecting");
        plr_logout(gs, 0, nr, LogoutReason::ParamsInvalid);
        return;
    }

    let login_ticket_data =
        match consume_api_login_ticket(login_ticket, keydb::consume_login_ticket) {
            Ok(login_ticket_data) => login_ticket_data,
            Err(reason) => {
                log::warn!("API login ticket denied: {:?}", reason);
                plr_logout(gs, 0, nr, reason);
                return;
            }
        };

    gs.players[nr].version = login_ticket_data.client_version as i32;
    gs.players[nr].race = login_ticket_data.race;
    gs.players[nr].api_account_id = login_ticket_data.account_id;
    gs.players[nr].api_character_id = login_ticket_data.character_id;

    // version check
    let version = gs.players[nr].version as u32;
    if version < core::constants::MINVERSION {
        log::warn!("Client too old ({}). Logout demanded", version);
        plr_logout(gs, 0, nr, LogoutReason::VersionMismatch);
        return;
    }

    let cn = match resolve_api_login_character(gs, nr, login_ticket_data.character_id) {
        Ok(cn) => cn,
        Err(reason) => {
            log::warn!("API login denied: {:?}", reason);
            plr_logout(gs, 0, nr, reason);
            return;
        }
    };

    gs.players[nr].usnr = cn;
    gs.players[nr].login_ticket = 0;

    // get character number requested by player
    let cn = gs.players[nr].usnr;

    if cn == 0 || cn >= core::constants::MAXCHARS {
        log::warn!("Login as {} denied (illegal cn)", cn);
        plr_logout(gs, 0, nr, LogoutReason::ParamsInvalid);
        return;
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

    if login_target_is_banned(gs, nr, cn) {
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

    // send initial talent-tree snapshot so the client can render the
    // talent panel immediately after login.
    crate::player::commands::send_set_char_talents(gs, nr);

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
    let name = gs.characters[cn].get_name().to_owned();
    gs.do_announce(cn, 0, &format!("{} entered the game.\n", name));
}

fn login_target_is_banned(gs: &GameState, nr: usize, cn: usize) -> bool {
    let checks = [
        BanTarget::Account {
            account_id: gs.players[nr].api_account_id,
        },
        BanTarget::Character {
            character_id: gs.players[nr].api_character_id,
        },
        BanTarget::Ipv4 {
            address: gs.players[nr].addr,
        },
    ];

    for target in checks {
        match server::keydb::ban::target_is_banned(&target) {
            Ok(true) => {
                log::info!(
                    "login for character {} denied by {} ban {}",
                    cn,
                    target.scope(),
                    target.value()
                );
                return true;
            }
            Ok(false) => {}
            Err(error) => {
                log::warn!(
                    "login for character {} denied because ban lookup failed for {} {}: {}",
                    cn,
                    target.scope(),
                    target.value(),
                    error
                );
                return true;
            }
        }
    }

    false
}

fn resolve_api_login_character(
    gs: &mut GameState,
    nr: usize,
    character_id: u64,
) -> Result<usize, LogoutReason> {
    resolve_api_login_character_with_ops(
        gs,
        nr,
        character_id,
        keydb::load_character,
        keydb::set_character_server_id,
        keydb::sync_character_selection_metadata,
    )
}

fn consume_api_login_ticket<ConsumeTicket>(
    login_ticket: u64,
    mut consume_ticket: ConsumeTicket,
) -> Result<GameLoginTicketMetadata, LogoutReason>
where
    ConsumeTicket: FnMut(u64) -> Result<Option<GameLoginTicketMetadata>, String>,
{
    match consume_ticket(login_ticket) {
        Ok(Some(value)) => Ok(value),
        Ok(None) => {
            log::warn!("API login ticket not found or expired");
            Err(LogoutReason::PasswordIncorrect)
        }
        Err(err) => {
            log::error!("KeyDB ticket consume failed: {}", err);
            Err(LogoutReason::Failure)
        }
    }
}

fn resolve_api_login_character_with_ops<LoadCharacter, SetServerId, SyncMetadata>(
    gs: &mut GameState,
    nr: usize,
    character_id: u64,
    mut load_character: LoadCharacter,
    mut set_server_id: SetServerId,
    mut sync_selection_metadata: SyncMetadata,
) -> Result<usize, LogoutReason>
where
    LoadCharacter: FnMut(u64) -> Result<Option<CharacterSummary>, String>,
    SetServerId: FnMut(u64, u32) -> Result<(), String>,
    SyncMetadata: FnMut(u64, &core::types::Character) -> Result<(), String>,
{
    let character = match load_character(character_id) {
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

    let (cn, is_brand_new_character) = apply_api_login_character_record(gs, &character)?;

    if is_brand_new_character {
        if let Err(err) = set_server_id(character_id, cn as u32) {
            log::warn!(
                "Failed to persist server_id for API character {}: {}",
                character_id,
                err
            );
        }
    }

    gs.players[nr].api_character_id = character_id;

    if let Err(err) = sync_selection_metadata(character_id, &gs.characters[cn]) {
        log::warn!(
            "Failed to sync selection metadata for API character {}: {}",
            character_id,
            err
        );
    }

    Ok(cn)
}

fn apply_api_login_character_record(
    gs: &mut GameState,
    character: &CharacterSummary,
) -> Result<(usize, bool), LogoutReason> {
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
                    character.id,
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
                    log::error!("Failed to create character for API id {}", character.id);
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

                // API-created characters already have validated profile data, so they do not
                // need the legacy in-game name/description finalization state.
                gs.characters[cn].flags |= CharacterFlags::Player.bits();
                gs.characters[cn].flags &= !CharacterFlags::NewUser.bits();
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

    Ok((cn, is_brand_new_character))
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
        let character_name = gs.characters[character_id].get_name().to_owned();
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
            let name = gs.characters[character_id].get_name().to_owned();

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

                gs.characters[character_id].a_hp -= i32::from(hp5 * 800);
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
                    gs.do_add_light(i32::from(character_x), i32::from(character_y), -i32::from(light));
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
                    gs.map[map_index].flags & u64::from(core::constants::MF_NOLAG) == 0
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
    gs.players[player_id].api_account_id = 0;
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

/// Handle API ticket based login.
///
/// The client sends `CL_API_LOGIN` with a u64 one-time ticket in the payload.
/// We store the ticket on the player slot, enter the login state, and send the
/// login-time mod packets while `plr_login` consumes the typed ticket metadata.
pub fn plr_api_login(gs: &mut GameState, nr: usize) {
    log::debug!("Player {} api_login", nr);

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
    gs.players[nr].state = core::constants::ST_LOGIN;
    gs.players[nr].lasttick = ticker;
    gs.players[nr].login_ticket = ticket;
    gs.players[nr].usnr = 0;
    gs.players[nr].api_account_id = 0;
    gs.players[nr].api_character_id = 0;

    log::info!("Player {} api login ticket accepted for resolution", nr);

    send_mod(gs, nr);
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

#[cfg(test)]
mod tests {
    use super::*;
    use core::{
        constants::{
            CharacterFlags, HOME_MERCENARY_X, HOME_MERCENARY_Y, ST_EXIT, ST_LOGIN, USE_ACTIVE,
            USE_NONACTIVE,
        },
        string_operations::c_string_to_str,
        traits,
        types::Class,
    };
    use std::{
        cell::Cell,
        net::{TcpListener, TcpStream},
    };

    use crate::{
        test_helpers::{add_test_player, with_test_gs, write_inbuf},
        tls::GameStream,
    };

    fn attach_test_socket(gs: &mut GameState, nr: usize) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind test listener");
        let addr = listener.local_addr().expect("listener addr");
        let client = TcpStream::connect(addr).expect("connect client");
        let (server, _) = listener.accept().expect("accept client");
        drop(client);
        gs.players[nr].sock = Some(GameStream::Plain(server));
    }

    fn map_index(x: i16, y: i16) -> usize {
        x as usize + y as usize * core::constants::SERVER_MAPX as usize
    }

    fn seed_character_template(gs: &mut GameState, template_id: usize, kindred: i32) {
        gs.character_templates[template_id] = core::types::Character::default();
        gs.character_templates[template_id].used = USE_ACTIVE;
        gs.character_templates[template_id].kindred = kindred;
        gs.character_templates[template_id].mode = 1;
        gs.character_templates[template_id].x = HOME_MERCENARY_X as i16;
        gs.character_templates[template_id].y = HOME_MERCENARY_Y as i16;
        gs.character_templates[template_id].tox = HOME_MERCENARY_X as i16;
        gs.character_templates[template_id].toy = HOME_MERCENARY_Y as i16;
        gs.character_templates[template_id].dir = core::constants::DX_DOWN;
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
        gs.characters[cn].temple_x = HOME_MERCENARY_X as u16;
        gs.characters[cn].temple_y = HOME_MERCENARY_Y as u16;
        write_ascii_into_fixed(&mut gs.characters[cn].name, name);
        write_ascii_into_fixed(&mut gs.characters[cn].reference, name);
        gs.map[map_index(10, 10)].ch = cn as u32;
    }

    fn count_obuf_packets(gs: &GameState, nr: usize, packet_id: u8) -> usize {
        gs.players[nr].obuf[..gs.players[nr].iptr]
            .chunks(16)
            .filter(|chunk| !chunk.is_empty() && chunk[0] == packet_id)
            .count()
    }

    #[test]
    fn apply_api_login_character_record_covers_existing_and_new_slots() {
        with_test_gs(|gs| {
            setup_existing_character(gs, 5, 0, USE_NONACTIVE, "Stale Name");
            let existing = CharacterSummary {
                id: 44,
                name: "Fresh Name".to_string(),
                description: "Updated description".to_string(),
                sex: Sex::Female,
                class: Class::Mercenary,
                selection_sprite_id: None,
                server_id: Some(5),
                rank_index: None,
            };

            let (cn, is_new) = apply_api_login_character_record(gs, &existing).unwrap();
            assert_eq!(cn, 5);
            assert!(!is_new);
            assert_eq!(gs.characters[5].get_name(), "Fresh Name");
            assert_eq!(
                c_string_to_str(&gs.characters[5].description),
                "Updated description"
            );

            let template_id = get_race_integer(true, Class::Mercenary) as usize;
            seed_character_template(
                gs,
                template_id,
                traits::KIN_MALE as i32 | traits::KIN_WARRIOR as i32,
            );
            let created = CharacterSummary {
                id: 45,
                name: "Api Hero".to_string(),
                description: String::new(),
                sex: Sex::Male,
                class: Class::Mercenary,
                selection_sprite_id: None,
                server_id: None,
                rank_index: None,
            };

            let (new_cn, is_new) = apply_api_login_character_record(gs, &created).unwrap();
            assert!(new_cn > 0);
            assert!(is_new);
            assert_eq!(gs.characters[new_cn].used, USE_NONACTIVE);
            assert_eq!(gs.characters[new_cn].player, 0);
            assert_eq!(gs.characters[new_cn].get_name(), "Api Hero");
            assert_eq!(gs.characters[new_cn].get_reference(), "Api Hero");
            assert_eq!(gs.characters[new_cn].tavern_x, HOME_MERCENARY_X as u16);
            assert_eq!(
                gs.characters[new_cn].flags & CharacterFlags::NewUser.bits(),
                0
            );
            assert_ne!(
                gs.characters[new_cn].flags & CharacterFlags::Player.bits(),
                0
            );
            let description = c_string_to_str(&gs.characters[new_cn].description);
            assert!(description.contains("Api Hero"));
            assert!(description.contains("looks somewhat nondescript"));
        });
    }

    #[test]
    fn resolve_api_login_character_with_ops_handles_error_and_success_paths() {
        with_test_gs(|gs| {
            let (_, nr) = add_test_player(gs);
            let result = resolve_api_login_character_with_ops(
                gs,
                nr,
                123,
                |_| Ok(None),
                |_, _| Ok(()),
                |_, _| Ok(()),
            );
            assert_eq!(result, Err(LogoutReason::PasswordIncorrect));
        });

        with_test_gs(|gs| {
            let (_, nr) = add_test_player(gs);
            let template_id = get_race_integer(true, Class::Mercenary) as usize;
            seed_character_template(
                gs,
                template_id,
                traits::KIN_MALE as i32 | traits::KIN_WARRIOR as i32,
            );
            let saved_server_id = Cell::new(0u32);
            let synced = Cell::new(false);

            let cn = resolve_api_login_character_with_ops(
                gs,
                nr,
                77,
                |_| {
                    Ok(Some(CharacterSummary {
                        id: 77,
                        name: "Ticket Hero".to_string(),
                        description: "Ticket description".to_string(),
                        sex: Sex::Male,
                        class: Class::Mercenary,
                        selection_sprite_id: None,
                        server_id: None,
                        rank_index: None,
                    }))
                },
                |_, server_id| {
                    saved_server_id.set(server_id);
                    Ok(())
                },
                |character_id, character| {
                    synced.set(character_id == 77 && character.get_name() == "Ticket Hero");
                    Ok(())
                },
            )
            .unwrap();

            assert!(cn > 0);
            assert_eq!(saved_server_id.get(), cn as u32);
            assert!(synced.get());
            assert_eq!(gs.players[nr].api_character_id, 77);
        });
    }

    #[test]
    fn consume_api_login_ticket_handles_metadata_and_errors() {
        let metadata = GameLoginTicketMetadata {
            account_id: 11,
            character_id: 77,
            client_version: core::constants::VERSION,
            race: 3,
        };

        assert_eq!(
            consume_api_login_ticket(999, |_| Ok(Some(metadata.clone()))).unwrap(),
            metadata
        );
        assert_eq!(
            consume_api_login_ticket(999, |_| Ok(None)),
            Err(LogoutReason::PasswordIncorrect)
        );
        assert_eq!(
            consume_api_login_ticket(999, |_| Err("decode failed".to_string())),
            Err(LogoutReason::Failure)
        );
    }

    #[test]
    fn plr_logout_handles_unknown_and_idle_timeout_paths() {
        with_test_gs(|gs| {
            let (cn, nr) = add_test_player(gs);
            attach_test_socket(gs, nr);
            gs.globals.ticker = 77;

            plr_logout(gs, cn, nr, LogoutReason::Unknown);

            assert_eq!(gs.players[nr].state, ST_EXIT);
            assert_eq!(gs.players[nr].lasttick, 77);
            assert_eq!(gs.characters[cn].player, 0);
        });

        with_test_gs(|gs| {
            let (cn, nr) = add_test_player(gs);
            attach_test_socket(gs, nr);
            gs.globals.ticker = 1234;
            gs.item_templates[core::constants::IT_LAGSCROLL as usize].used = USE_ACTIVE;
            setup_existing_character(gs, cn, nr as i32, USE_ACTIVE, "Laggy");
            gs.characters[cn].flags = CharacterFlags::Player.bits();
            gs.characters[cn].temple_x = 0;
            gs.characters[cn].temple_y = 0;
            gs.characters[cn].gold = 10;

            plr_logout(gs, cn, nr, LogoutReason::IdleTooLong);

            assert_eq!(gs.players[nr].state, ST_EXIT);
            assert_eq!(gs.characters[cn].used, USE_NONACTIVE);
            assert!(gs.characters[cn].item.iter().any(|&item| item != 0
                && gs.items[item as usize].temp == core::constants::IT_LAGSCROLL as u16));
        });
    }

    #[test]
    fn player_exit_sets_exit_state_and_clears_character_mapping() {
        with_test_gs(|gs| {
            let (cn, nr) = add_test_player(gs);
            gs.globals.ticker = 42;

            player_exit(gs, nr);

            assert_eq!(gs.players[nr].state, ST_EXIT);
            assert_eq!(gs.players[nr].lasttick, 42);
            assert_eq!(gs.players[nr].api_character_id, 0);
            assert_eq!(gs.characters[cn].player, 0);
        });
    }

    #[test]
    fn plr_api_login_stores_ticket_and_sends_mods() {
        with_test_gs(|gs| {
            let (_, nr) = add_test_player(gs);
            attach_test_socket(gs, nr);
            gs.globals.ticker = 301;
            let mut packet = [0u8; 9];
            packet[1..9].copy_from_slice(&0x1122334455667788u64.to_le_bytes());
            write_inbuf(gs, nr, &packet);

            plr_api_login(gs, nr);

            assert_eq!(gs.players[nr].state, ST_LOGIN);
            assert_eq!(gs.players[nr].login_ticket, 0x1122334455667788);
            assert_eq!(gs.players[nr].usnr, 0);
            assert_eq!(gs.players[nr].api_character_id, 0);
            assert_eq!(gs.players[nr].iptr, 16 * 8);
        });
    }

    #[test]
    fn send_mod_queues_all_eight_packets() {
        with_test_gs(|gs| {
            let (_, nr) = add_test_player(gs);
            attach_test_socket(gs, nr);

            send_mod(gs, nr);

            assert_eq!(gs.players[nr].iptr, 16 * 8);
            assert_eq!(count_obuf_packets(gs, nr, ServerCommandType::Mod1 as u8), 1);
            assert_eq!(count_obuf_packets(gs, nr, ServerCommandType::Mod8 as u8), 1);
        });
    }
}
