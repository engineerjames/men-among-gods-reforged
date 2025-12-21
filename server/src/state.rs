use core::constants::{MAXCHARS, MAXPLAYER};
use std::cmp;
use std::rc::Rc;

use crate::enums;
use crate::god::God;
use crate::network_manager::NetworkManager;
use crate::path_finding::PathFinder;
use crate::repository::Repository;

pub struct State {
    pathfinder: PathFinder,
    network: Rc<NetworkManager>,
    visi: Option<*const [i8; 40 * 40]>,
    _visi: [i8; 40 * 40],
    see_miss: u64,
    see_hit: u64,
    ox: i32,
    oy: i32,
    is_monster: bool,
}

impl State {
    pub fn new(network: Rc<NetworkManager>) -> Self {
        State {
            pathfinder: PathFinder::new(),
            network,
            visi: None,
            _visi: [0; 40 * 40],
            see_miss: 0,
            see_hit: 0,
            ox: 0,
            oy: 0,
            is_monster: false,
        }
    }

    /// plr_logout from original C++ code
    pub fn logout_player(
        &mut self,
        repository: &mut Repository,
        character_id: usize,
        player: Option<(usize, &mut core::types::ServerPlayer)>,
        reason: enums::LogoutReason,
    ) {
        // Logic to log out the player
        log::debug!(
            "Logging out character '{}' for reason: {:?}",
            repository.characters[character_id].get_name(),
            reason
        );

        let mut character = &mut repository.characters[character_id];

        let character_has_player = character.player
            == player
                .as_ref()
                .map_or(i32::MAX, |(player_number, _)| *player_number as i32);

        if character_has_player && character.flags & enums::CharacterFlags::Usurp.bits() != 0 {
            // If this character belongs to the player being logged out and has the Usurp flag set,
            // clear all elevated privilege flags (CCP, Usurp, Staff, Immortal, God, Creator).
            // This ensures that characters who were temporarily granted elevated permissions
            // (e.g., via usurping another character) lose those permissions when logging out.
            character.flags &= !(enums::CharacterFlags::ComputerControlledPlayer
                | enums::CharacterFlags::Usurp
                | enums::CharacterFlags::Staff
                | enums::CharacterFlags::Immortal
                | enums::CharacterFlags::God
                | enums::CharacterFlags::Creator)
                .bits();

            let co = character.data[97];
            // Perform the actual logout operation for the player controlling this character.
            self.logout_player(repository, co as usize, None, enums::LogoutReason::Shutdown);

            character = &mut repository.characters[character_id];
        }

        // TODO: Evaluate if we can use this instead:
        // if character_has_player
        //     && enums::CharacterFlags::from_bits_truncate(repository.characters[character_id].flags)
        //         .contains(enums::CharacterFlags::Player)
        //     && !enums::CharacterFlags::from_bits_truncate(repository.characters[character_id].flags)
        //         .contains(enums::CharacterFlags::ComputerControlledPlayer)
        // {}
        if character_has_player
            && (character.flags & enums::CharacterFlags::Player.bits()) != 0
            && (character.flags & enums::CharacterFlags::ComputerControlledPlayer.bits()) == 0
        {
            if reason == enums::LogoutReason::Exit {
                log::warn!(
                    "Character '{}' punished for leaving the game by means of F12.",
                    character.get_name(),
                );

                let damage_message = format!(
                    "You have been hit by a demon. You lost {} HP.\n",
                    (character.hp[5] * 8 / 10)
                );
                let messages_to_send = [
                    " \n",
                    "You are being punished for leaving the game without entering a tavern:\n",
                    " \n",
                    damage_message.as_str(),
                ];

                for i in 0..messages_to_send.len() {
                    self.do_character_log(
                        Some((character_id, character)),
                        &player,
                        core::types::FontColor::Red,
                        messages_to_send[i],
                    );
                }

                character.a_hp -= (character.hp[5] * 800) as i32;

                if character.a_hp < 500 {
                    self.do_character_log(
                        Some((character_id, character)),
                        &player,
                        core::types::FontColor::Red,
                        String::from("The demon killed you.\n \n").as_str(),
                    );
                    // TODO: Kill the character here
                } else {
                    if character.gold / 10 > 0 {
                        let money_stolen_message = format!(
                            " \nA demon grabs your purse and removes {} gold, and {} silver.\n",
                            (character.gold / 10) / 100,
                            (character.gold / 10) % 100
                        );

                        self.do_character_log(
                            Some((character_id, character)),
                            &player,
                            core::types::FontColor::Red,
                            money_stolen_message.as_str(),
                        );

                        character.gold -= character.gold / 10;

                        if character.citem != 0 && character.citem & 0x80000000 == 0 {
                            self.do_character_log(
                                Some((character_id, character)),
                                &player,
                                core::types::FontColor::Red,
                                "The demon also takes the money in your hand!\n",
                            );

                            // Remove the item from the character
                            character.citem = 0;
                        }
                    }
                    // TODO: Do area log here
                    //    do_area_log( cn, 0, ch[ cn ].x, ch[ cn ].y, 2, "%s left the game without saying goodbye and was punished by the gods.\n",
                    //    ch[ cn ].name );
                }
            }

            let map_index =
                (character.y as usize) * core::constants::MAPX as usize + (character.x as usize);
            if repository.map[map_index].ch == character_id as u32 {
                repository.map[map_index].ch = 0;

                if character.light != 0 {
                    // TODO: Update lighting here via do_add_light
                }
            }

            let to_map_index = (character.toy as usize) * core::constants::MAPX as usize
                + (character.tox as usize);
            if repository.map[to_map_index].ch == character_id as u32 {
                repository.map[to_map_index].ch = 0;
            }

            // TODO: remove_enemy call goes here
            if reason == enums::LogoutReason::IdleTooLong
                || reason == enums::LogoutReason::Shutdown
                || reason == enums::LogoutReason::Unknown
            {
                if !character.is_close_to_temple()
                    && !character.in_no_lag_scroll_area(&repository.map)
                {
                    log::info!(
                        "Giving lag scroll to character '{}' for idle/logout too long.",
                        character.get_name(),
                    );

                    let item_number = God::create_item(
                        &mut repository.items,
                        &repository.item_templates,
                        core::constants::IT_LAGSCROLL as usize,
                    );

                    if let Some(item_id) = item_number {
                        repository.items[item_id].data[0] = character.x as u32;
                        repository.items[item_id].data[1] = character.y as u32;
                        repository.items[item_id].data[2] = repository.globals.ticker as u32;

                        God::give_character_item(
                            character,
                            &mut repository.items[item_id],
                            character_id,
                            item_id,
                        );
                    } else {
                        log::error!(
                            "Failed to create lag scroll for character '{}'.",
                            character.get_name(),
                        );
                    }
                }
            }

            character.x = 0;
            character.y = 0;
            character.tox = 0;
            character.toy = 0;
            character.frx = 0;
            character.fry = 0;
            character.player = 0;
            character.status = 0;
            character.status2 = 0;
            character.dir = 0;
            character.escape_timer = 0;
            for i in 0..4 {
                character.enemy[i] = 0;
            }
            character.attack_cn = 0;
            character.skill_nr = 0;
            character.goto_x = 0;
            character.goto_y = 0; // TODO: This wasn't there originally; mistake?
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
                .as_secs() as u32; // TODO: Evaluate if this duration is appropriate

            character.flags |= enums::CharacterFlags::SaveMe.bits();

            // TODO:
            // if ( IS_BUILDING( cn ) )
            // god_build(cn, 0);

            // TODO: Call do_announce here
        }

        if player.is_some() && reason != enums::LogoutReason::Usurp {
            let mut buffer: [u8; 16] = [0; 16];
            buffer[0] = core::constants::SV_EXIT;
            buffer[1] = reason as u8;

            let (player_id, player) = player.unwrap();

            if player.state == core::constants::ST_NORMAL {
                self.network.xsend(player.usnr as usize, &buffer, 16);
            } else {
                self.network.csend(player.usnr as usize, &buffer, 16);
            }

            self.player_exit(
                repository.globals.ticker as u32,
                (character_id, character),
                (player_id, player),
            );
        }
    }

    pub fn player_exit(
        &self,
        ticker: u32,
        character: (usize, &mut core::types::Character),
        player: (usize, &mut core::types::ServerPlayer),
    ) {
        let (_, ch) = character;
        let (player_id, plr) = player;

        log::info!(
            "Player {} exiting for character '{}'",
            player_id,
            ch.get_name()
        );

        plr.state = core::constants::ST_EXIT;
        plr.lasttick = ticker;

        if plr.usnr > 0 && plr.usnr < MAXCHARS && ch.player as usize == player_id {
            ch.player = 0;
        }
    }

    pub fn do_character_log(
        &self,
        character: Option<(usize, &core::types::Character)>,
        player: &Option<(usize, &mut core::types::ServerPlayer)>,
        font: core::types::FontColor,
        message: &str,
    ) {
        if let Some((character_id, ch)) = character {
            if ch.player == 0 && ch.temp != 15 {
                return;
            }

            self.do_log(Some((character_id, ch)), player, font, message);
        }
    }

    pub fn do_log(
        &self,
        character: Option<(usize, &core::types::Character)>,
        player: &Option<(usize, &mut core::types::ServerPlayer)>,
        font: core::types::FontColor,
        message: &str,
    ) {
        let mut buffer: [u8; 16] = [0; 16];

        if let Some((character_id, ch)) = character {
            let player_id = ch.player;

            if player_id < 1 || player_id as usize >= MAXPLAYER {
                log::error!(
                    "do_log: Invalid player ID {} for character '{}'",
                    player_id,
                    ch.get_name(),
                );
                return;
            }

            if let Some((_, player)) = player {
                if player.usnr != character_id {
                    return;
                }
            } else {
                log::warn!(
                    "do_log: No player reference for character '{}'",
                    ch.get_name(),
                );
            }

            let mut bytes_sent: usize = 0;
            let len = message.len() - 1;

            while bytes_sent <= len {
                buffer[0] = core::constants::SV_LOG + font as u8;

                for i in 0..15 {
                    if bytes_sent + i > len {
                        buffer[i + 1] = 0;
                    } else {
                        buffer[i + 1] = message.as_bytes()[bytes_sent + i];
                    }
                }

                self.network.xsend(player_id as usize, &buffer, 16);

                bytes_sent += 15;
            }
        }
    }

    pub fn do_add_light(
        &self,
        map_tiles: &mut [core::types::Map],
        see_map: &mut [core::types::SeeMap],
        character: &core::types::Character,
        x_center: i32,
        y_center: i32,
        mut strength: i32,
    ) {
        // First add light to the center
        let center_map_index =
            (y_center as usize) * core::constants::MAPX as usize + (x_center as usize);

        map_tiles[center_map_index].add_light(strength);

        let flag = if strength < 0 {
            strength = -strength;
            1
        } else {
            0
        };

        let xs = cmp::max(0, x_center - core::constants::LIGHTDIST);
        let ys = cmp::max(0, y_center - core::constants::LIGHTDIST);
        let xe = cmp::min(
            core::constants::MAPX as i32 - 1,
            x_center + 1 + core::constants::LIGHTDIST,
        );
        let ye = cmp::min(
            core::constants::MAPY as i32 - 1,
            y_center + 1 + core::constants::LIGHTDIST,
        );

        for y in ys..ye {
            for x in xs..xe {
                if x == x_center && y == y_center {
                    continue;
                }

                let dx = (x - x_center).abs();
                let dy = (y - y_center).abs();

                if (dx * dx + dy * dy)
                    > (core::constants::LIGHTDIST * core::constants::LIGHTDIST + 1)
                {
                    continue;
                }
            }
        }
    }

    pub fn can_see(
        &mut self,
        see_map: &mut [core::types::SeeMap],
        character: &core::types::Character,
        character_id: Option<usize>,
        fx: i32,
        fy: i32,
        tx: i32,
        ty: i32,
        max_distance: i32,
    ) {
        match character_id {
            Some(cn) => {
                self.visi = Some(&see_map[cn].vis as *const _);

                if (fx != see_map[cn].x) || (fy != see_map[cn].y) {
                    if character.is_monster() && !character.is_usurp_or_thrall() {
                        self.is_monster = true;
                    }

                    self.can_map_see(fx, fy, max_distance);
                    see_map[cn].x = fx;
                    see_map[cn].y = fy;
                    self.see_miss += 1;
                } else {
                    self.see_hit += 1;
                    self.ox = fx;
                    self.oy = fy;
                }
            }
            None => {
                // TODO: Look up what this actually does...
                if !self.visi.map_or(false, |v| v == &self._visi as *const _) {
                    self.visi = Some(&self._visi as *const _);
                    self.ox = 0;
                    self.oy = 0;
                }

                if (self.ox != fx) || (self.oy != fy) {
                    self.is_monster = false;
                    self.can_map_see(fx, fy, max_distance);
                }
            }
        }

        self.check_vis(tx, ty);
    }

    pub fn can_map_see(&self, fx: i32, fy: i32, max_distance: i32) {
        // Implementation of line-of-sight algorithm goes here
    }

    pub fn check_vis(&self, tx: i32, ty: i32) {
        // Implementation to check visibility of target coordinates goes here
    }

    pub fn add_vis(&self, x: i32, y: i32, value: i32) {
        // Implementation to add visibility value at coordinates goes here
    }

    pub fn close_vis_see(&self, x: i32, y: i32, value: i32) {
        // Implementation to close visibility map goes here
    }
}
