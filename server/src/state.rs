use core::constants::{CharacterFlags, MAXCHARS, MAXPLAYER};
use core::types::{Character, FontColor, ServerPlayer};
use rand::Rng;
use std::cmp;
use std::sync::{OnceLock, RwLock};

use crate::driver::Driver;
use crate::god::God;
use crate::network_manager::NetworkManager;
use crate::path_finding::PathFinder;
use crate::repository::Repository;
use crate::server::Server;
use crate::{enums, helpers};

static STATE: OnceLock<RwLock<State>> = OnceLock::new();

pub struct State {
    pathfinder: PathFinder,
    _visi: [i8; 40 * 40],
    visi: [i8; 40 * 40],
    see_miss: u64,
    see_hit: u64,
    ox: i32,
    oy: i32,
    is_monster: bool,
}

impl State {
    fn new() -> Self {
        State {
            pathfinder: PathFinder::new(),
            _visi: [0; 40 * 40],
            visi: [0; 40 * 40],
            see_miss: 0,
            see_hit: 0,
            ox: 0,
            oy: 0,
            is_monster: false,
        }
    }

    pub fn initialize() -> Result<(), String> {
        let state = State::new();
        STATE
            .set(RwLock::new(state))
            .map_err(|_| "State already initialized".to_string())?;
        Ok(())
    }

    pub fn with<F, R>(f: F) -> R
    where
        F: FnOnce(&State) -> R,
    {
        let state = STATE.get().expect("State not initialized").read().unwrap();
        f(&*state)
    }

    pub fn with_mut<F, R>(f: F) -> R
    where
        F: FnOnce(&mut State) -> R,
    {
        let mut state = STATE.get().expect("State not initialized").write().unwrap();
        f(&mut *state)
    }

    /// plr_logout from original C++ code
    pub fn logout_player(
        &mut self,
        character_id: usize,
        player: Option<(usize, &mut core::types::ServerPlayer)>,
        reason: enums::LogoutReason,
    ) {
        // Logic to log out the player
        Repository::with_characters(|characters| {
            log::debug!(
                "Logging out character '{}' for reason: {:?}",
                characters[character_id].get_name(),
                reason
            );
        });

        let character_has_player = Repository::with_characters(|characters| {
            characters[character_id].player
                == player
                    .as_ref()
                    .map_or(i32::MAX, |(player_number, _)| *player_number as i32)
        });

        // Handle usurp flag and recursive logout
        if character_has_player {
            let should_logout_co = Repository::with_characters_mut(|characters| {
                let character = &mut characters[character_id];
                if character.flags & enums::CharacterFlags::Usurp.bits() != 0 {
                    character.flags &= !(enums::CharacterFlags::ComputerControlledPlayer
                        | enums::CharacterFlags::Usurp
                        | enums::CharacterFlags::Staff
                        | enums::CharacterFlags::Immortal
                        | enums::CharacterFlags::God
                        | enums::CharacterFlags::Creator)
                        .bits();
                    Some(character.data[97] as usize)
                } else {
                    None
                }
            });

            if let Some(co) = should_logout_co {
                self.logout_player(co, None, enums::LogoutReason::Shutdown);
            }
        }

        // Main logout logic for active players
        if character_has_player {
            let (is_player, is_not_ccp) = Repository::with_characters(|characters| {
                let character = &characters[character_id];
                (
                    character.flags & enums::CharacterFlags::Player.bits() != 0,
                    character.flags & enums::CharacterFlags::ComputerControlledPlayer.bits() == 0,
                )
            });

            if is_player && is_not_ccp {
                // Handle exit punishment
                if reason == enums::LogoutReason::Exit {
                    Repository::with_characters_mut(|characters| {
                        let character = &mut characters[character_id];
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
                                character_id,
                                core::types::FontColor::Red,
                                messages_to_send[i],
                            );
                        }

                        character.a_hp -= (character.hp[5] * 800) as i32;

                        if character.a_hp < 500 {
                            self.do_character_log(
                                character_id,
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
                                    character_id,
                                    core::types::FontColor::Red,
                                    money_stolen_message.as_str(),
                                );

                                character.gold -= character.gold / 10;

                                if character.citem != 0 && character.citem & 0x80000000 == 0 {
                                    self.do_character_log(
                                        character_id,
                                        core::types::FontColor::Red,
                                        "The demon also takes the money in your hand!\n",
                                    );

                                    character.citem = 0;
                                }
                            }
                        }
                    });
                }

                // Clear map positions
                let (map_index, to_map_index, light, character_x, character_y) =
                    Repository::with_characters(|characters| {
                        let character = &characters[character_id];
                        let map_index = (character.y as usize)
                            * core::constants::SERVER_MAPX as usize
                            + (character.x as usize);
                        let to_map_index = (character.toy as usize)
                            * core::constants::SERVER_MAPX as usize
                            + (character.tox as usize);
                        (
                            map_index,
                            to_map_index,
                            character.light,
                            character.x,
                            character.y,
                        )
                    });

                Repository::with_map_mut(|map| {
                    if map[map_index].ch == character_id as u32 {
                        map[map_index].ch = 0;
                        if light != 0 {
                            self.do_add_light(
                                character_x as i32,
                                character_y as i32,
                                -(light as i32),
                            );
                        }
                    }

                    if map[to_map_index].ch == character_id as u32 {
                        map[to_map_index].ch = 0;
                    }
                });

                // Handle lag scroll
                if reason == enums::LogoutReason::IdleTooLong
                    || reason == enums::LogoutReason::Shutdown
                    || reason == enums::LogoutReason::Unknown
                {
                    let (is_close_to_temple, map_index) =
                        Repository::with_characters(|characters| {
                            let character = &characters[character_id];
                            let map_index = (character.y as usize)
                                * core::constants::SERVER_MAPX as usize
                                + (character.x as usize);
                            (character.is_close_to_temple(), map_index)
                        });

                    let should_give = if !is_close_to_temple {
                        Repository::with_map(|map| {
                            map[map_index].flags & core::constants::MF_NOLAG as u64 == 0
                        })
                    } else {
                        false
                    };

                    if should_give {
                        Repository::with_characters(|characters| {
                            log::info!(
                                "Giving lag scroll to character '{}' for idle/logout too long.",
                                characters[character_id].get_name(),
                            );
                        });

                        if let Some(item_id) =
                            God::create_item(core::constants::IT_LAGSCROLL as usize)
                        {
                            let (char_x, char_y) = Repository::with_characters(|characters| {
                                (characters[character_id].x, characters[character_id].y)
                            });

                            Repository::with_items_mut(|items| {
                                items[item_id].data[0] = char_x as u32;
                                items[item_id].data[1] = char_y as u32;
                            });

                            Repository::with_globals(|globals| {
                                Repository::with_items_mut(|items| {
                                    items[item_id].data[2] = globals.ticker as u32;
                                });
                            });

                            God::give_character_item(character_id, item_id);
                        } else {
                            Repository::with_characters(|characters| {
                                log::error!(
                                    "Failed to create lag scroll for character '{}'.",
                                    characters[character_id].get_name(),
                                );
                            });
                        }
                    }
                }

                // Reset character state
                Repository::with_characters_mut(|characters| {
                    let character = &mut characters[character_id];
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

                    character.flags |= enums::CharacterFlags::SaveMe.bits();

                    if character.is_building() {
                        God::build(character, character_id, 0);
                    }
                });
            }
        }

        // Send exit message to player
        if player.is_some() && reason != enums::LogoutReason::Usurp {
            let mut buffer: [u8; 16] = [0; 16];
            buffer[0] = core::constants::SV_EXIT;
            buffer[1] = reason as u8;

            let (player_id, plr) = player.unwrap();

            if plr.state == core::constants::ST_NORMAL {
                NetworkManager::with(|network| {
                    network.xsend(player_id as usize, &buffer, 16);
                });
            } else {
                NetworkManager::with(|network| {
                    network.csend(player_id as usize, &buffer, 16);
                });
            }

            Repository::with_characters_mut(|characters| {
                let character = &mut characters[character_id];
                Repository::with_globals(|globals| {
                    self.player_exit(
                        globals.ticker as u32,
                        (character_id, character),
                        (player_id, plr),
                    );
                });
            });
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
        character_id: usize,
        font: core::types::FontColor,
        message: &str,
    ) {
        Repository::with_characters(|characters| {
            let ch = &characters[character_id];
            if ch.player == 0 && ch.temp != 15 {
                log::warn!(
                    "do_character_log: Character '{}' has no associated player.",
                    ch.get_name(),
                );
                return;
            }

            self.do_log(character_id, font, message);
        });
    }

    pub fn do_log(
        &self, // TODO: Rework these functions to pass in just the ids around
        character_id: usize,
        font: core::types::FontColor,
        message: &str,
    ) {
        let mut buffer: [u8; 16] = [0; 16];

        Repository::with_characters(|characters| {
            let ch = &characters[character_id];

            if !ServerPlayer::is_sane_player(ch.player as usize)
                || (ch.flags & CharacterFlags::CF_PLAYER.bits()) == 0
            {
                let id = ch.player;
                log::error!(
                    "do_log: Invalid player ID {} for character '{}'",
                    id,
                    ch.get_name(),
                );
                return;
            }

            let matching_player_id = Server::with_players(|players| {
                for i in 0..MAXPLAYER as usize {
                    if players[i].usnr == character_id {
                        return Some(i);
                    }
                }

                None
            });

            if matching_player_id.is_none() {
                log::error!(
                    "do_log: No matching player found for character '{}'",
                    ch.get_name(),
                );
                return;
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

                NetworkManager::with(|network| {
                    network.xsend(matching_player_id.unwrap() as usize, &buffer, 16);
                });

                bytes_sent += 15;
            }
        });
    }

    pub fn do_area_log(
        &self,
        cn: usize,
        co: usize,
        xs: i32,
        ys: i32,
        font: core::types::FontColor,
        message: &str,
    ) {
        let x_min = cmp::max(0, xs - 12);
        let x_max = cmp::min(core::constants::SERVER_MAPX as i32, xs + 13);
        let y_min = cmp::max(0, ys - 12);
        let y_max = cmp::min(core::constants::SERVER_MAPY as i32, ys + 13);

        let mut recipients: Vec<usize> = Vec::new();

        Repository::with_map(|map| {
            for y in y_min..y_max {
                let row_base = y * core::constants::SERVER_MAPX as i32;
                for x in x_min..x_max {
                    let idx = (x + row_base) as usize;
                    let cc = map[idx].ch as usize;
                    if cc == 0 || cc == cn || cc == co {
                        continue;
                    }
                    recipients.push(cc);
                }
            }
        });

        let recipients: Vec<usize> = Repository::with_characters(|characters| {
            recipients
                .into_iter()
                .filter(|cc| {
                    *cc < MAXCHARS as usize
                        && characters[*cc].used == core::constants::USE_ACTIVE
                        && characters[*cc].player != 0
                        && (characters[*cc].flags & CharacterFlags::CF_PLAYER.bits()) != 0
                })
                .collect()
        });

        for cc in recipients {
            self.do_character_log(cc, font, message);
        }
    }

    pub fn do_add_light(&mut self, x_center: i32, y_center: i32, mut strength: i32) {
        // First add light to the center
        let center_map_index =
            (y_center as usize) * core::constants::SERVER_MAPX as usize + (x_center as usize);

        Repository::with_map_mut(|map_tiles| {
            map_tiles[center_map_index].add_light(strength);
        });

        let flag = if strength < 0 {
            strength = -strength;
            1
        } else {
            0
        };

        let xs = cmp::max(0, x_center - core::constants::LIGHTDIST);
        let ys = cmp::max(0, y_center - core::constants::LIGHTDIST);
        let xe = cmp::min(
            core::constants::SERVER_MAPX as i32 - 1,
            x_center + 1 + core::constants::LIGHTDIST,
        );
        let ye = cmp::min(
            core::constants::SERVER_MAPY as i32 - 1,
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

                let v = self.can_see(None, x_center, y_center, x, y, core::constants::LIGHTDIST);

                if v != 0 {
                    let d = strength / (v * (x_center - x).abs() + (y_center - y).abs());
                    let map_index =
                        (y as usize) * core::constants::SERVER_MAPX as usize + (x as usize);

                    Repository::with_map_mut(|map_tiles| {
                        if flag == 1 {
                            map_tiles[map_index].add_light(-d);
                        } else {
                            map_tiles[map_index].add_light(d);
                        }
                    });
                }
            }
        }
    }

    fn compute_dlight(&mut self, xc: i32, yc: i32) {
        let xs = cmp::max(0, xc - core::constants::LIGHTDIST);
        let ys = cmp::max(0, yc - core::constants::LIGHTDIST);
        let xe = cmp::min(
            core::constants::SERVER_MAPX as i32 - 1,
            xc + 1 + core::constants::LIGHTDIST,
        );
        let ye = cmp::min(
            core::constants::SERVER_MAPY as i32 - 1,
            yc + 1 + core::constants::LIGHTDIST,
        );

        let mut best: i32 = 0;

        for y in ys..ye {
            for x in xs..xe {
                let dx = xc - x;
                let dy = yc - y;
                if dx * dx + dy * dy > (core::constants::LIGHTDIST * core::constants::LIGHTDIST + 1)
                {
                    continue;
                }

                let m = (x + y * core::constants::SERVER_MAPX as i32) as usize;

                let should_continue = Repository::with_map(|map| {
                    map[m].flags & core::constants::MF_INDOORS as u64 != 0
                });

                if should_continue {
                    continue;
                }

                let v = self.can_see(None, xc, yc, x, y, core::constants::LIGHTDIST);
                if v == 0 {
                    continue;
                }

                let denom = v * (dx.abs() + dy.abs());
                if denom <= 0 {
                    continue;
                }

                let d = 256 / denom;
                if d > best {
                    best = d;
                }
            }
        }

        if best > 256 {
            best = 256;
        }

        let center_index = (xc + yc * core::constants::SERVER_MAPX as i32) as usize;

        Repository::with_map_mut(|map| {
            if center_index < map.len() {
                map[center_index].dlight = best as u16;
            }
        });
    }

    /// Port of `add_lights(int x, int y)` from the original `helper.cpp`.
    pub fn add_lights(&mut self, x: i32, y: i32) {
        let x0 = x;
        let y0 = y;

        let xs = cmp::max(1, x0 - core::constants::LIGHTDIST);
        let ys = cmp::max(1, y0 - core::constants::LIGHTDIST);
        let xe = cmp::min(
            core::constants::SERVER_MAPX as i32 - 2,
            x0 + 1 + core::constants::LIGHTDIST,
        );
        let ye = cmp::min(
            core::constants::SERVER_MAPY as i32 - 2,
            y0 + 1 + core::constants::LIGHTDIST,
        );

        for yy in ys..ye {
            for xx in xs..xe {
                let m = (xx + yy * core::constants::SERVER_MAPX as i32) as usize;

                let item_idx = Repository::with_map(|map| map[m].it as usize);
                let light_value_from_item = Repository::with_items(|items| {
                    if item_idx != 0 && item_idx < items.len() {
                        let it = &items[item_idx];
                        if it.active != 0 {
                            it.light[1]
                        } else {
                            it.light[0]
                        }
                    } else {
                        0
                    }
                });

                if light_value_from_item != 0 {
                    self.do_add_light(xx, yy, light_value_from_item as i32);
                }

                let cn = Repository::with_map(|map| map[m].ch as usize);

                let light_value_from_character = Repository::with_characters(|characters| {
                    if !Character::is_sane_character(cn) {
                        0
                    } else {
                        characters[cn].light
                    }
                });

                if light_value_from_character != 0 {
                    self.do_add_light(xx, yy, light_value_from_character as i32);
                }

                let is_indoors = Repository::with_map(|map| {
                    map[m].flags & core::constants::MF_INDOORS as u64 != 0
                });
                if is_indoors {
                    self.compute_dlight(xx, yy);
                }
            }
        }
    }

    pub fn can_see(
        &mut self,
        character_id: Option<usize>,
        fx: i32,
        fy: i32,
        tx: i32,
        ty: i32,
        max_distance: i32,
    ) -> i32 {
        Repository::with_see_map_mut(|see_map| {
            Repository::with_characters(|characters| {
                match character_id {
                    Some(cn) => {
                        if (fx != see_map[cn].x) || (fy != see_map[cn].y) {
                            self.is_monster =
                                characters[cn].is_monster() && !characters[cn].is_usurp_or_thrall();

                            // Copy the visibility data from see_map to our working buffer
                            self._visi.copy_from_slice(&see_map[cn].vis);

                            self.can_map_see(fx, fy, max_distance);

                            // Copy the updated visibility data back to see_map
                            see_map[cn].vis.copy_from_slice(&self._visi);
                            see_map[cn].x = fx;
                            see_map[cn].y = fy;
                            self.see_miss += 1;
                        } else {
                            // Copy the visibility data from see_map for checking
                            self._visi.copy_from_slice(&see_map[cn].vis);
                            self.see_hit += 1;
                            self.ox = fx;
                            self.oy = fy;
                        }
                    }
                    None => {
                        if (self.ox != fx) || (self.oy != fy) {
                            self.is_monster = false;
                            self.can_map_see(fx, fy, max_distance);
                        }
                    }
                }
            })
        });

        self.check_vis(tx, ty)
    }

    pub fn can_map_go(&mut self, fx: i32, fy: i32, max_distance: i32) {
        // Clear the visibility array
        self._visi.fill(0);

        self.ox = fx;
        self.oy = fy;

        self.add_vis(fx, fy, 1);

        for dist in 1..(max_distance + 1) {
            let xc = fx;
            let yc = fy;

            // Top and bottom horizontal lines
            for x in (xc - dist)..=(xc + dist) {
                let y = yc - dist;
                if self.close_vis_see(x, y, dist as i8) {
                    self.add_vis(x, y, dist + 1);
                }

                let y = yc + dist;
                if self.close_vis_see(x, y, dist as i8) {
                    self.add_vis(x, y, dist + 1);
                }
            }

            // Left and right vertical lines (excluding corners already done)
            for y in (yc - dist + 1)..=(yc + dist - 1) {
                let x = xc - dist;
                if self.close_vis_see(x, y, dist as i8) {
                    self.add_vis(x, y, dist + 1);
                }

                let x = xc + dist;
                if self.close_vis_see(x, y, dist as i8) {
                    self.add_vis(x, y, dist + 1);
                }
            }
        }
    }

    pub fn can_map_see(&mut self, fx: i32, fy: i32, max_distance: i32) {
        // Clear the visibility array
        self._visi.fill(0);

        self.ox = fx;
        self.oy = fy;

        self.add_vis(fx, fy, 1);

        for dist in 1..(max_distance + 1) {
            let xc = fx;
            let yc = fy;

            // Top and bottom horizontal lines
            for x in (xc - dist)..=(xc + dist) {
                let y = yc - dist;
                if self.close_vis_see(x, y, dist as i8) {
                    self.add_vis(x, y, dist + 1);
                }

                let y = yc + dist;
                if self.close_vis_see(x, y, dist as i8) {
                    self.add_vis(x, y, dist + 1);
                }
            }

            // Left and right vertical lines (excluding corners already done)
            for y in (yc - dist + 1)..=(yc + dist - 1) {
                let x = xc - dist;
                if self.close_vis_see(x, y, dist as i8) {
                    self.add_vis(x, y, dist + 1);
                }

                let x = xc + dist;
                if self.close_vis_see(x, y, dist as i8) {
                    self.add_vis(x, y, dist + 1);
                }
            }
        }
    }

    pub fn can_go(&mut self, fx: i32, fy: i32, target_x: i32, target_y: i32) -> bool {
        if self.visi != self._visi {
            self.visi = self._visi.clone();
            self.ox = 0;
            self.oy = 0;
        }

        if (self.ox != fx || self.oy != fy) {
            self.can_map_go(fx, fy, 15);
        }

        let tmp = self.check_vis(target_x, target_y);

        tmp != 0
    }

    pub fn check_dlight(x: usize, y: usize) -> i32 {
        let map_index = x + y * core::constants::SERVER_MAPX as usize;

        Repository::with_map(|map| {
            Repository::with_globals(|globals| {
                if map[map_index].flags & core::constants::MF_INDOORS as u64 == 0 {
                    globals.dlight
                } else {
                    (globals.dlight * map[map_index].dlight as i32) / 256
                }
            })
        })
    }

    // TODO: Combine with check_dlight
    pub fn check_dlightm(map_index: usize) -> i32 {
        Repository::with_map(|map| {
            Repository::with_globals(|globals| {
                if map[map_index].flags & core::constants::MF_INDOORS as u64 == 0 {
                    globals.dlight
                } else {
                    (globals.dlight * map[map_index].dlight as i32) / 256
                }
            })
        })
    }

    pub fn do_character_calculate_light(&self, cn: usize, light: i32) -> i32 {
        Repository::with_characters(|characters| {
            let character = &characters[cn];
            let mut adjusted_light = light;

            if light == 0 && character.skill[core::constants::SK_PERCEPT][5] > 150 {
                adjusted_light = 1;
            }

            adjusted_light = adjusted_light
                * std::cmp::min(character.skill[core::constants::SK_PERCEPT][5] as i32, 10)
                / 10;

            if adjusted_light > 255 {
                adjusted_light = 255;
            }

            if character.flags & CharacterFlags::CF_INFRARED.bits() != 0 && adjusted_light < 5 {
                adjusted_light = 5;
            }

            adjusted_light
        })
    }

    pub fn do_character_can_see(&mut self, cn: usize, co: usize) -> bool {
        if cn == co {
            return true;
        }

        Repository::with_characters(|characters| {
            Repository::with_map(|map| {
                if characters[co].used != core::constants::USE_ACTIVE {
                    return false;
                }

                if characters[co].flags & CharacterFlags::CF_INVISIBLE.bits() != 0
                    && (characters[cn].get_invisibility_level()
                        < characters[co].get_invisibility_level())
                {
                    return false;
                }

                if characters[co].flags & CharacterFlags::CF_BODY.bits() != 0 {
                    return false;
                }

                let d1 = (characters[cn].x - characters[co].x).abs() as i32;
                let d2 = (characters[cn].y - characters[co].y).abs() as i32;

                let rd = d1 * d1 + d2 * d2;
                let mut d = rd;

                if d > 1000 {
                    return false;
                }

                // Modify by perception and stealth
                match characters[co].mode {
                    0 => {
                        d = (d
                            * (characters[co].skill[core::constants::SK_STEALTH][5] as i32 + 20))
                            / 20;
                    }
                    1 => {
                        d = (d
                            * (characters[co].skill[core::constants::SK_STEALTH][5] as i32 + 50))
                            / 50;
                    }
                    _ => {
                        d = (d
                            * (characters[co].skill[core::constants::SK_STEALTH][5] as i32 + 100))
                            / 100;
                    }
                }

                d -= characters[cn].skill[core::constants::SK_PERCEPT][5] as i32 * 2;

                // Modify by light
                if characters[cn].flags & CharacterFlags::CF_INFRARED.bits() == 0 {
                    let map_index = characters[co].x as usize
                        + characters[co].y as usize * core::constants::SERVER_MAPX as usize;
                    let mut light = std::cmp::max(
                        map[map_index].light as i32,
                        State::check_dlight(characters[co].x as usize, characters[co].y as usize),
                    );

                    // TODO: Shouldn't this be co?
                    light = self.do_character_calculate_light(cn, light);

                    if light == 0 {
                        return false;
                    }

                    if light > 64 {
                        light = 64;
                    }

                    d += (64 - light) * 2;
                }

                if rd < 3 && d > 70 {
                    d = 70;
                }

                if d > 200 {
                    return false;
                }

                let can_see = !self
                    .can_see(
                        Some(cn),
                        characters[cn].x as i32,
                        characters[cn].y as i32,
                        characters[co].x as i32,
                        characters[co].y as i32,
                        15,
                    )
                    .ne(&0);

                if !can_see {
                    return false;
                }

                if d < 1 {
                    return true;
                }

                d != 0 // TODO: Should we return the numeric value?
            })
        })
    }

    pub fn do_char_can_see_item(&mut self, cn: usize, in_idx: usize) -> i32 {
        Repository::with_characters(|characters| {
            Repository::with_items(|items| {
                Repository::with_map(|map| {
                    // Check if item is active
                    if items[in_idx].used != core::constants::USE_ACTIVE {
                        return 0;
                    }

                    // Calculate raw distance (squared)
                    let d1 = (characters[cn].x - items[in_idx].x as i16).abs() as i32;
                    let d2 = (characters[cn].y - items[in_idx].y as i16).abs() as i32;

                    let rd = d1 * d1 + d2 * d2;
                    let mut d = rd;

                    // Early exit for far distances
                    if d > 1000 {
                        return 0;
                    }

                    // Modify by perception
                    d += 50 - characters[cn].skill[core::constants::SK_PERCEPT][5] as i32 * 2;

                    // Modify by light (unless character has infrared)
                    if characters[cn].flags & CharacterFlags::CF_INFRARED.bits() == 0 {
                        let map_index = items[in_idx].x as usize
                            + items[in_idx].y as usize * core::constants::SERVER_MAPX as usize;
                        let mut light = std::cmp::max(
                            map[map_index].light as i32,
                            State::check_dlight(items[in_idx].x as usize, items[in_idx].y as usize),
                        );

                        light = self.do_character_calculate_light(cn, light);

                        if light == 0 {
                            return 0;
                        }

                        if light > 64 {
                            light = 64;
                        }

                        d += (64 - light) * 3;
                    }

                    // Check for hidden items
                    if items[in_idx].flags & core::constants::ItemFlags::IF_HIDDEN.bits() != 0 {
                        d += items[in_idx].data[9] as i32;
                    } else if rd < 3 && d > 200 {
                        d = 200;
                    }

                    // Check distance threshold
                    if d > 200 {
                        return 0;
                    }

                    // Check line of sight
                    let can_see = self.can_see(
                        Some(cn),
                        characters[cn].x as i32,
                        characters[cn].y as i32,
                        items[in_idx].x as i32,
                        items[in_idx].y as i32,
                        15,
                    );

                    if can_see == 0 {
                        return 0;
                    }

                    // Return 1 for very close items, otherwise return distance
                    if d < 1 {
                        1
                    } else {
                        d
                    }
                })
            })
        })
    }

    pub fn check_vis(&self, tx: i32, ty: i32) -> i32 {
        let mut best = 99;

        let x = tx - self.ox + 20;
        let y = ty - self.oy + 20;

        // Check all 8 adjacent cells for the best (lowest) visibility value
        let offsets = [
            (1, 0),
            (-1, 0),
            (0, 1),
            (0, -1),
            (1, 1),
            (1, -1),
            (-1, 1),
            (-1, -1),
        ];

        for (dx, dy) in offsets.iter() {
            let nx = x + dx;
            let ny = y + dy;

            if nx >= 0 && nx < 40 && ny >= 0 && ny < 40 {
                let idx = (nx + ny * 40) as usize;
                let val = self._visi[idx];
                if val != 0 && val < best {
                    best = val;
                }
            }
        }

        if best == 99 {
            0
        } else {
            best as i32
        }
    }

    pub fn add_vis(&mut self, x: i32, y: i32, value: i32) {
        let vx = x - self.ox + 20;
        let vy = y - self.oy + 20;

        if vx >= 0 && vx < 40 && vy >= 0 && vy < 40 {
            let idx = (vx + vy * 40) as usize;
            if self._visi[idx] == 0 {
                self._visi[idx] = value as i8;
            }
        }
    }

    pub fn close_vis_see(&self, x: i32, y: i32, value: i8) -> bool {
        if !self.check_map_see(x, y) {
            return false;
        }

        let vx = x - self.ox + 20;
        let vy = y - self.oy + 20;

        if vx < 0 || vx >= 40 || vy < 0 || vy >= 40 {
            return false;
        }

        // Check all 8 adjacent cells
        let offsets = [
            (1, 0),
            (-1, 0),
            (0, 1),
            (0, -1),
            (1, 1),
            (1, -1),
            (-1, 1),
            (-1, -1),
        ];

        for (dx, dy) in offsets.iter() {
            let nx = vx + dx;
            let ny = vy + dy;

            if nx >= 0 && nx < 40 && ny >= 0 && ny < 40 {
                let idx = (nx + ny * 40) as usize;
                if self._visi[idx] == value {
                    return true;
                }
            }
        }

        false
    }

    fn check_map_see(&self, x: i32, y: i32) -> bool {
        // Check boundaries
        if x <= 0
            || x >= core::constants::SERVER_MAPX as i32
            || y <= 0
            || y >= core::constants::SERVER_MAPY as i32
        {
            return false;
        }

        let m = (x + y * core::constants::SERVER_MAPX as i32) as usize;

        // Check if it's a monster and the map blocks monsters
        if self.is_monster {
            let blocked = Repository::with_map(|map| {
                map[m].flags & (core::constants::MF_SIGHTBLOCK | core::constants::MF_NOMONST) as u64
                    != 0
            });
            if blocked {
                return false;
            }
        } else {
            // Check for sight blocking flags
            let blocked = Repository::with_map(|map| {
                map[m].flags & core::constants::MF_SIGHTBLOCK as u64 != 0
            });
            if blocked {
                return false;
            }
        }

        // Check if there's an item that blocks sight
        let blocks_sight = Repository::with_map(|map| {
            let item_idx = map[m].it as usize;
            if item_idx != 0 {
                Repository::with_items(|items| {
                    item_idx < items.len()
                        && items[item_idx].flags & core::constants::ItemFlags::IF_SIGHTBLOCK.bits()
                            != 0
                })
            } else {
                false
            }
        });

        if blocks_sight {
            return false;
        }

        true
    }

    /// Port of original `do_sayx(int cn, char* format, ...)` from `svr_do.cpp`.
    ///
    /// The C++ version formats a message into a local buffer, runs `process_options`,
    /// and then sends a local area log message with different fonts for player/NPC.
    pub fn do_sayx(&self, character_id: usize, message: &str) {
        let mut buf = message.to_string();
        Self::process_options(character_id, &mut buf);

        let (x, y, is_player, name) = Repository::with_characters(|characters| {
            let ch = &characters[character_id];
            (
                ch.x as i32,
                ch.y as i32,
                (ch.flags & CharacterFlags::CF_PLAYER.bits()) != 0,
                ch.get_name().to_string(),
            )
        });

        let name_short: String = name.chars().take(30).collect();
        let msg_short: String = buf.chars().take(300).collect();

        let line = format!("{}: \"{}\"\n", name_short, msg_short);

        let font = if is_player {
            core::types::FontColor::Blue
        } else {
            core::types::FontColor::Yellow
        };

        self.do_area_log(0, 0, x, y, font, &line);
    }

    fn char_play_sound(character_id: usize, sound: i32, vol: i32, pan: i32) {
        let matching_player_id = Server::with_players(|players| {
            for i in 0..MAXPLAYER as usize {
                if players[i].usnr == character_id {
                    return Some(i);
                }
            }
            None
        });

        let Some(player_id) = matching_player_id else {
            return;
        };

        let mut buf: [u8; 16] = [0; 16];
        buf[0] = core::constants::SV_PLAYSOUND;
        buf[1..5].copy_from_slice(&sound.to_le_bytes());
        buf[5..9].copy_from_slice(&vol.to_le_bytes());
        buf[9..13].copy_from_slice(&pan.to_le_bytes());

        NetworkManager::with(|network| {
            network.xsend(player_id, &buf, 13);
        });
    }

    pub fn do_area_sound(cn: usize, co: usize, xs: i32, ys: i32, nr: i32) {
        let x_min = cmp::max(0, xs - 8);
        let x_max = cmp::min(core::constants::SERVER_MAPX as i32, xs + 9);
        let y_min = cmp::max(0, ys - 8);
        let y_max = cmp::min(core::constants::SERVER_MAPY as i32, ys + 9);

        let mut recipients: Vec<(usize, i32, i32)> = Vec::new();

        Repository::with_map(|map| {
            for y in y_min..y_max {
                let row_base = y * core::constants::SERVER_MAPX as i32;
                for x in x_min..x_max {
                    let idx = (x + row_base) as usize;
                    let cc = map[idx].ch as usize;
                    if cc == 0 || cc == cn || cc == co {
                        continue;
                    }

                    let s = ys - y + xs - x;
                    let xpan = if s < 0 {
                        -500
                    } else if s > 0 {
                        500
                    } else {
                        0
                    };

                    let dist2 = (ys - y) * (ys - y) + (xs - x) * (xs - x);
                    let mut xvol = -150 - dist2 * 30;
                    if xvol < -5000 {
                        xvol = -5000;
                    }

                    recipients.push((cc, xvol, xpan));
                }
            }
        });

        let recipients_with_player: Vec<(usize, i32, i32)> =
            Repository::with_characters(|characters| {
                recipients
                    .into_iter()
                    .filter(|(cc, _, _)| characters[*cc].player != 0)
                    .collect()
            });

        for (cc, vol, pan) in recipients_with_player {
            Self::char_play_sound(cc, nr, vol, pan);
        }
    }

    /// Port of original `process_options(int cn, char* buf)` from `svr_do.cpp`.
    ///
    /// Supports a leading `#<digits>###` option prefix:
    /// - Parses the integer sound id after the first '#'
    /// - Strips the `#<digits>` and any additional leading '#' characters
    /// - If the parsed sound id is non-zero, plays it to nearby players (excluding the speaker)
    pub fn process_options(character_id: usize, buf: &mut String) {
        if !buf.starts_with('#') {
            return;
        }

        let bytes = buf.as_bytes();
        let mut idx: usize = 1; // skip initial '#'

        while idx < bytes.len() && bytes[idx].is_ascii_digit() {
            idx += 1;
        }

        let sound_id: i32 = if idx > 1 {
            buf[1..idx].parse::<i32>().unwrap_or(0)
        } else {
            0
        };

        while idx < bytes.len() && bytes[idx] == b'#' {
            idx += 1;
        }

        buf.drain(..idx);

        if sound_id != 0 {
            let (x, y) = Repository::with_characters(|characters| {
                let ch = &characters[character_id];
                (ch.x as i32, ch.y as i32)
            });
            Self::do_area_sound(character_id, 0, x, y, sound_id);
        }
    }

    pub fn reset_go(&mut self, xc: i32, yc: i32) {
        Repository::with_see_map_mut(|see_map| {
            for y in
                std::cmp::max(0, yc - 18)..std::cmp::min(core::constants::SERVER_MAPY - 1, yc + 18)
            {
                for x in std::cmp::max(0, xc - 18)
                    ..std::cmp::min(core::constants::SERVER_MAPX - 1, xc + 18)
                {
                    let cn = Repository::with_map(|map| {
                        map[(x + y * core::constants::SERVER_MAPX) as usize].ch as usize
                    });

                    see_map[cn].x = 0;
                    see_map[cn].y = 0;
                }
            }
        });

        self.ox = 0;
        self.oy = 0;
    }

    pub fn remove_lights(&mut self, x: i32, y: i32) {
        let xs = cmp::max(1, x - core::constants::LIGHTDIST);
        let ys = cmp::max(1, y - core::constants::LIGHTDIST);
        let xe = cmp::min(
            core::constants::SERVER_MAPX as i32 - 2,
            x + 1 + core::constants::LIGHTDIST,
        );
        let ye = cmp::min(
            core::constants::SERVER_MAPY as i32 - 2,
            y + 1 + core::constants::LIGHTDIST,
        );

        for yy in ys..ye {
            for xx in xs..xe {
                let m = (xx + yy * core::constants::SERVER_MAPX as i32) as usize;

                let item_idx = Repository::with_map(|map| map[m].it as usize);
                let light_value_from_item = Repository::with_items(|items| {
                    if item_idx != 0 && item_idx < items.len() {
                        let it = &items[item_idx];
                        if it.active != 0 {
                            it.light[1]
                        } else {
                            it.light[0]
                        }
                    } else {
                        0
                    }
                });

                if light_value_from_item != 0 {
                    self.do_add_light(xx, yy, -(light_value_from_item as i32));
                }

                let cn = Repository::with_map(|map| map[m].ch as usize);

                let light_value_from_character = Repository::with_characters(|characters| {
                    if !Character::is_sane_character(cn) {
                        0
                    } else {
                        characters[cn].light
                    }
                });

                if light_value_from_character != 0 {
                    self.do_add_light(xx, yy, -(light_value_from_character as i32));
                }

                Repository::with_map_mut(|map| {
                    map[m].dlight = 0;
                });
            }
        }
    }

    pub fn do_area_notify(
        &self,
        cn: i32,
        co: i32,
        xs: i32,
        ys: i32,
        notify_type: i32,
        dat1: i32,
        dat2: i32,
        dat3: i32,
        dat4: i32,
    ) {
        Repository::with_map(|map| {
            // 12 = AREASIZE constant TODO: use constant
            for y in
                std::cmp::max(0, ys - 12)..std::cmp::min(core::constants::SERVER_MAPY, ys + 12 + 1)
            {
                let m = y * core::constants::SERVER_MAPX as i32;
                for x in std::cmp::max(0, xs - 12)
                    ..std::cmp::min(core::constants::SERVER_MAPX, xs + 12 + 1)
                {
                    let cc = map[(x + m) as usize].ch;

                    if cc != 0 && cc != cn as u32 && cc != co as u32 {
                        self.do_notify_character(cc, notify_type, dat1, dat2, dat3, dat4);
                    }
                }
            }
        });
    }

    /// Store the carried item (citem) into the first available inventory slot
    /// Port of `do_store_item(int cn)` from `svr_do.cpp`
    ///
    /// Returns:
    /// - The inventory slot number (0-39) where the item was stored on success
    /// - -1 if the operation failed (citem is invalid or inventory is full)
    pub fn do_store_item(&self, cn: usize) -> i32 {
        Repository::with_characters_mut(|characters| {
            let ch = &mut characters[cn];

            // Check if citem has the high bit set (0x80000000), which indicates it's invalid
            if (ch.citem & 0x80000000) != 0 {
                return -1;
            }

            // Find first empty inventory slot
            let mut slot = -1;
            for n in 0..40 {
                if ch.item[n] == 0 {
                    slot = n as i32;
                    break;
                }
            }

            // If no empty slot found, return failure
            if slot == -1 {
                return -1;
            }

            // Store the carried item in the empty slot
            ch.item[slot as usize] = ch.citem;
            ch.citem = 0;

            // Update character to sync with client
            ch.set_do_update_flags();

            slot
        })
    }

    /// Port of `do_char_killed(int cn, int co)` from `svr_do.cpp`
    ///
    /// Handles all aspects of character death including:
    /// - Notifications and logging
    /// - Sound effects
    /// - Alignment changes for killers
    /// - Item dropping and grave creation
    /// - Player resurrection
    /// - NPC respawn
    ///
    /// # Arguments
    /// * `character_id` - The character who died (co in C++)
    /// * `killer_id` - The character who did the killing (cn in C++, can be 0)
    pub fn do_character_killed(&self, character_id: usize, killer_id: usize) {
        // Send death notification
        self.do_notify_character(
            character_id as u32,
            core::constants::NT_DIED as i32,
            killer_id as i32,
            0,
            0,
            0,
        );

        // Log the kill
        if killer_id != 0 {
            // TODO: Implement chlog - character logging function
            log::info!(
                "Character {} killed character {} ({})",
                killer_id,
                character_id,
                Repository::with_characters(|ch| ch[character_id].get_name().to_string())
            );
        } else {
            log::info!("Character {} died", character_id);
        }

        // Get map flags for both characters
        let (co_x, co_y, co_flags, co_temp, co_sound) = Repository::with_characters(|characters| {
            let co = &characters[character_id];
            (co.x, co.y, co.flags, co.temp, co.sound)
        });

        let mut map_flags = Repository::with_map(|map| {
            let idx = (co_x + co_y * core::constants::SERVER_MAPX as i16) as usize;
            map[idx].flags
        });

        if killer_id != 0 {
            let cn_flags = Repository::with_characters(|characters| {
                let cn = &characters[killer_id];
                let idx = (cn.x + cn.y * core::constants::SERVER_MAPX as i16) as usize;
                Repository::with_map(|map| map[idx].flags)
            });
            map_flags &= cn_flags;
        }

        // Play death sound effects
        // Hack for grolms (templates 364-374)
        if co_temp >= 364 && co_temp <= 374 {
            Self::do_area_sound(character_id, 0, co_x as i32, co_y as i32, 17);
            Self::char_play_sound(character_id, 17, -150, 0);
        }
        // Hack for gargoyles (templates 375-381)
        else if co_temp >= 375 && co_temp <= 381 {
            Self::do_area_sound(character_id, 0, co_x as i32, co_y as i32, 18);
            Self::char_play_sound(character_id, 18, -150, 0);
        }
        // Normal death sound
        else {
            let sound = co_sound + 2;
            Self::do_area_sound(character_id, 0, co_x as i32, co_y as i32, sound as i32);
            Self::char_play_sound(character_id, sound as i32, -150, 0);
        }

        // Cleanup for ghost companions
        if co_temp == core::constants::CT_COMPANION as u16 {
            Repository::with_characters_mut(|characters| {
                let cc = characters[character_id].data[63] as usize;
                if Character::is_sane_character(cc)
                    && characters[cc].data[64] == character_id as i32
                {
                    characters[cc].data[64] = 0;
                }
                characters[character_id].data[63] = 0;
            });
        }

        // A player killed someone or something
        if killer_id != 0 && killer_id != character_id {
            let (is_killer_player, is_arena, co_alignment, co_temp, co_is_player) =
                Repository::with_characters(|characters| {
                    let is_killer_player =
                        characters[killer_id].flags & CharacterFlags::CF_PLAYER.bits() != 0;
                    let co_alignment = characters[character_id].alignment;
                    let co_temp = characters[character_id].temp;
                    let co_is_player =
                        characters[character_id].flags & CharacterFlags::CF_PLAYER.bits() != 0;
                    (
                        is_killer_player,
                        map_flags & core::constants::MF_ARENA as u64 == 0,
                        co_alignment,
                        co_temp,
                        co_is_player,
                    )
                });

            if is_killer_player && is_arena {
                // Adjust alignment
                Repository::with_characters_mut(|characters| {
                    characters[killer_id].alignment -= co_alignment / 50;
                    if characters[killer_id].alignment > 7500 {
                        characters[killer_id].alignment = 7500;
                    }
                    if characters[killer_id].alignment < -7500 {
                        characters[killer_id].alignment = -7500;
                    }
                });

                // Check for killing priests (becoming purple)
                if co_temp == core::constants::CT_PRIEST as u16 {
                    let killer_kindred = Repository::with_characters(|ch| ch[killer_id].kindred);

                    if killer_kindred as u32 & core::constants::KIN_PURPLE != 0 {
                        self.do_character_log(
                            killer_id,
                            core::types::FontColor::Yellow,
                            "Ahh, that felt good!\n",
                        );
                    } else {
                        Repository::with_characters_mut(|characters| {
                            Repository::with_globals_mut(|globals| {
                                characters[killer_id].data[67] = globals.ticker;
                            });
                        });
                        self.do_character_log(
                            killer_id,
                            core::types::FontColor::Red,
                            "So, you want to be a player killer, right?\n",
                        );
                        self.do_character_log(
                            killer_id,
                            core::types::FontColor::Red,
                            "To join the purple one and be a killer, type #purple now.\n",
                        );
                        // TODO: Implement fx_add_effect
                        log::info!("TODO: Add effect 6 at position ({}, {})", co_x, co_y);
                    }
                }

                // Check for killing shopkeepers & questgivers (alignment 10000)
                if !co_is_player && co_alignment == 10000 {
                    self.do_character_log(
                        killer_id,
                        core::types::FontColor::Red,
                        "You feel a god look into your soul. He seems to be angry.\n",
                    );

                    Repository::with_characters_mut(|characters| {
                        characters[killer_id].data[40] += 1;
                        let penalty = if characters[killer_id].data[40] < 50 {
                            -characters[killer_id].data[40] * 100
                        } else {
                            -5000
                        };
                        characters[killer_id].luck += penalty;

                        let luck_to_print = characters[killer_id].luck;
                        log::info!(
                            "Reduced luck by {} to {} for killing {} (t={})",
                            penalty,
                            luck_to_print,
                            characters[character_id].get_name(),
                            co_temp
                        );
                    });
                }

                Repository::with_characters_mut(|characters| {
                    // Update statistics
                    let r1: u32 = helpers::points2rank(characters[killer_id].points_tot as u32);
                    let r2: u32 = helpers::points2rank(characters[character_id].points_tot as u32);

                    if (r1 as i32 - r2 as i32).abs() < 3 {
                        // Approximately own rank
                        characters[killer_id].data[24] += 1; // overall counter
                        if characters[character_id].data[42] == 27 {
                            characters[killer_id].data[27] += 1; // black stronghold counter
                        }
                    } else if r2 > r1 {
                        // Above own rank
                        characters[killer_id].data[25] += 1;
                        if characters[character_id].data[42] == 27 {
                            characters[killer_id].data[28] += 1;
                        }
                    } else {
                        // Below own rank
                        characters[killer_id].data[23] += 1;
                        if characters[character_id].data[42] == 27 {
                            characters[killer_id].data[26] += 1;
                        }
                    }

                    if co_is_player {
                        characters[killer_id].data[29] += 1;
                    } else {
                        // TODO: Implement killed_class and get_class_name
                        // Check for first kill of this monster class
                        if characters[character_id].monster_class != 0 {
                            let monster_class_to_log = characters[character_id].monster_class;
                            log::info!(
                                "TODO: Check if first kill of monster class {}",
                                monster_class_to_log,
                            );
                        }
                    }
                });
            }

            // A follower (gargoyle, ghost companion) killed someone
            let follower_owner = Repository::with_characters(|characters| {
                if characters[killer_id].flags & CharacterFlags::CF_PLAYER.bits() == 0 {
                    let cc = characters[killer_id].data[63] as usize;
                    if cc != 0 && Character::is_sane_character(cc) {
                        Some(cc)
                    } else {
                        None
                    }
                } else {
                    None
                }
            });

            if let Some(cc) = follower_owner {
                let is_owner_player = Repository::with_characters(|ch| {
                    ch[cc].flags & CharacterFlags::CF_PLAYER.bits() != 0
                });

                if is_owner_player && !co_is_player && co_alignment == 10000 {
                    self.do_character_log(cc, core::types::FontColor::Red,
                        "A goddess is about to turn your follower into a frog, but notices that you are responsible. You feel her do something to you. Nothing good, that's for sure.\n");

                    Repository::with_characters_mut(|characters| {
                        characters[cc].data[40] += 1;
                        let penalty = if characters[cc].data[40] < 50 {
                            -characters[cc].data[40] * 100
                        } else {
                            -5000
                        };
                        characters[cc].luck += penalty;

                        let luck_to_print = characters[cc].luck;
                        log::info!(
                            "Reduced luck by {} to {} for follower killing {} (t={})",
                            penalty,
                            luck_to_print,
                            characters[character_id].get_name(),
                            co_temp
                        );
                    });
                }

                // Notify area about the kill
                let (cc_x, cc_y) = Repository::with_characters(|ch| (ch[cc].x, ch[cc].y));
                self.do_area_notify(
                    cc as i32,
                    character_id as i32,
                    cc_x as i32,
                    cc_y as i32,
                    core::constants::NT_SEEHIT as i32,
                    cc as i32,
                    character_id as i32,
                    0,
                    0,
                );
            }
        }

        // Handle player death
        let is_player = Repository::with_characters(|ch| {
            ch[character_id].flags & CharacterFlags::CF_PLAYER.bits() != 0
        });

        if is_player {
            // Update player death statistics
            Repository::with_globals_mut(|globals| {
                globals.players_died += 1;
            });

            // Adjust luck if negative
            Repository::with_characters_mut(|characters| {
                if characters[character_id].luck < 0 {
                    characters[character_id].luck =
                        std::cmp::min(0, characters[character_id].luck + 10);
                }

                // Set killed by message
                characters[character_id].data[14] += 1;
                if killer_id != 0 {
                    let is_killer_player =
                        characters[killer_id].flags & CharacterFlags::CF_PLAYER.bits() != 0;
                    if is_killer_player {
                        characters[character_id].data[15] = killer_id as i32 | 0x10000;
                    } else {
                        characters[character_id].data[15] = characters[killer_id].temp as i32;
                    }
                } else {
                    characters[character_id].data[15] = 0;
                }

                Repository::with_globals(|globals| {
                    characters[character_id].data[16] = globals.mdday + globals.mdyear * 300;
                });
                characters[character_id].data[17] =
                    (co_x + co_y * core::constants::SERVER_MAPX as i16) as i32;
            });

            self.handle_player_death(character_id, killer_id, map_flags);
        } else {
            // Handle NPC death
            let is_labkeeper = Repository::with_characters(|ch| {
                ch[character_id].flags & CharacterFlags::CF_LABKEEPER.bits() != 0
            });

            if is_labkeeper {
                self.handle_labkeeper_death(character_id, killer_id);
            } else {
                self.handle_npc_death(character_id, killer_id);
            }
        }

        // Remove from enemy lists
        // TODO: Implement remove_enemy
        log::info!(
            "TODO: Remove character {} from all enemy lists",
            character_id
        );

        // Schedule respawn and show death animation
        // TODO: Implement fx_add_effect for death animation
        log::info!("TODO: Add death effect at position ({}, {})", co_x, co_y);
    }

    /// Handle player death including resurrection and grave creation
    pub fn handle_player_death(&self, co: usize, cn: usize, map_flags: u64) {
        // Remember template if we're to respawn this character
        let temp = Repository::with_characters(|characters| {
            if characters[co].flags & CharacterFlags::CF_RESPAWN.bits() != 0 {
                characters[co].temp
            } else {
                0
            }
        });

        // Check for Guardian Angel (Wimpy skill)
        let wimp = Repository::with_characters(|characters| {
            let mut wimp_power = 0;
            for n in 0..20 {
                let item_idx = characters[co].spell[n] as usize;
                if item_idx != 0 {
                    Repository::with_items(|items| {
                        let power_to_print = items[item_idx].power;
                        if item_idx < items.len() {
                            log::info!(
                                "spell active: {}, power of {}",
                                items[item_idx].get_name(),
                                power_to_print
                            );
                            if items[item_idx].temp == core::constants::SK_WIMPY as u16 {
                                wimp_power = items[item_idx].power / 2;
                            }
                        }
                    });
                }
            }
            wimp_power
        });

        let wimp = if map_flags & core::constants::MF_ARENA as u64 != 0 {
            205
        } else {
            wimp
        };

        // Find free character slot for body/grave
        let cc = Repository::with_characters(|characters| {
            for cc in 1..MAXCHARS {
                if characters[cc].used == core::constants::USE_EMPTY {
                    return Some(cc);
                }
            }
            None
        });

        let Some(cc) = cc else {
            log::error!(
                "Could not clone character {} for grave, all char slots full!",
                co
            );
            return;
        };

        // Clone character to create grave
        Repository::with_characters_mut(|characters| {
            characters[cc] = characters[co].clone();
        });

        // Drop items and money based on wimp chance
        self.handle_item_drops(co, cc, wimp as i32, cn);

        // Move player to temple
        let (temple_x, temple_y, cur_x, cur_y) = Repository::with_characters(|ch| {
            (ch[co].temple_x, ch[co].temple_y, ch[co].x, ch[co].y)
        });

        if cur_x as u16 == temple_x && cur_y as u16 == temple_y {
            God::transfer_char(co, (temple_x + 4) as usize, (temple_y + 4) as usize);
        } else {
            God::transfer_char(co, temple_x as usize, temple_y as usize);
        }

        // Resurrect player with 10 HP
        Repository::with_characters_mut(|characters| {
            characters[co].a_hp = 10000; // 10 HP (stored as 10000)
            characters[co].status = 0;
            characters[co].attack_cn = 0;
            characters[co].skill_nr = 0;
            characters[co].goto_x = 0;
            characters[co].use_nr = 0;
            characters[co].misc_action = 0;
            characters[co].stunned = 0;
            characters[co].retry = 0;
            characters[co].current_enemy = 0;
            for m in 0..4 {
                characters[co].enemy[m] = 0;
            }
        });

        // TODO: Implement plr_reset_status
        log::info!("TODO: Reset player status for character {}", co);

        // Apply permanent stat loss if not a god and no guardian angel
        let is_god =
            Repository::with_characters(|ch| ch[co].flags & CharacterFlags::CF_GOD.bits() != 0);

        if !is_god && wimp == 0 {
            self.apply_death_penalties(co);
        } else if wimp != 0 && map_flags & core::constants::MF_ARENA as u64 == 0 {
            self.do_character_log(
                co,
                core::types::FontColor::Yellow,
                "Sometimes a Guardian Angel is really helpful...\n",
            );
        }

        // Update player character
        // TODO: Implement do_update_char
        Repository::with_characters_mut(|ch| {
            ch[co].set_do_update_flags();
        });

        // Setup the grave (body)
        Repository::with_characters_mut(|characters| {
            // TODO: Implement plr_reset_status
            log::info!("TODO: Reset status for grave {}", cc);

            characters[cc].player = 0;
            characters[cc].flags = CharacterFlags::CF_BODY.bits();
            characters[cc].a_hp = 0;
            characters[cc].data[core::constants::CHD_CORPSEOWNER] = co as i32;
            characters[cc].data[99] = 1;
            characters[cc].data[98] = 0;

            characters[cc].attack_cn = 0;
            characters[cc].skill_nr = 0;
            characters[cc].goto_x = 0;
            characters[cc].use_nr = 0;
            characters[cc].misc_action = 0;
            characters[cc].stunned = 0;
            characters[cc].retry = 0;
            characters[cc].current_enemy = 0;
            for m in 0..4 {
                characters[cc].enemy[m] = 0;
            }
        });

        // Update grave character
        Repository::with_characters_mut(|ch| {
            ch[cc].set_do_update_flags();
        });

        // TODO: Implement plr_map_set
        log::info!("TODO: Set map for grave character {}", cc);
    }

    /// Handle NPC death
    fn handle_npc_death(&self, co: usize, cn: usize) {
        // Update NPC death statistics
        Repository::with_globals_mut(|globals| {
            globals.npcs_died += 1;
        });

        // TODO: Implement plr_reset_status
        log::info!("TODO: Reset NPC status for character {}", co);

        // Check for USURP flag (player controlling NPC)
        let usurp_player = Repository::with_characters(|characters| {
            if characters[co].flags & CharacterFlags::CF_USURP.bits() != 0 {
                let c2 = characters[co].data[97] as usize;
                if Character::is_sane_character(c2) {
                    Some((c2, characters[co].player))
                } else {
                    None
                }
            } else {
                None
            }
        });

        if let Some((c2, player_nr)) = usurp_player {
            Repository::with_characters_mut(|characters| {
                characters[c2].player = player_nr;
                // TODO: Update player[nr].usnr = c2
                log::info!("TODO: Transfer player {} from {} to {}", player_nr, co, c2);
                characters[c2].flags &= !CharacterFlags::CF_CCP.bits();
            });
        } else if let Some((_, player_nr)) = usurp_player {
            // TODO: Implement player_exit
            log::info!("TODO: player_exit for player {}", player_nr);
        }

        log::info!("new npc body");

        // Convert to body
        let should_respawn = Repository::with_characters(|characters| {
            characters[co].flags & CharacterFlags::CF_RESPAWN.bits() != 0
        });

        Repository::with_characters_mut(|characters| {
            if should_respawn {
                characters[co].flags =
                    CharacterFlags::CF_BODY.bits() | CharacterFlags::CF_RESPAWN.bits();
            } else {
                characters[co].flags = CharacterFlags::CF_BODY.bits();
            }

            characters[co].a_hp = 0;

            // Set corpse owner (killer only mode vs all can loot)
            #[cfg(feature = "KILLERONLY")]
            {
                let cc = Repository::with_characters(|ch| {
                    if cn != 0 && !(ch[cn].flags & CharacterFlags::CF_PLAYER.bits() != 0) {
                        let cc = ch[cn].data[63] as usize;
                        if cc != 0 && (ch[cc].flags & CharacterFlags::CF_PLAYER.bits() != 0) {
                            Some(cc)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                });

                if let Some(cc) = cc {
                    characters[co].data[CHD_CORPSEOWNER] = cc as i32;
                } else if cn != 0 {
                    let is_cn_player = characters[cn].flags & CharacterFlags::CF_PLAYER.bits() != 0;
                    if is_cn_player {
                        characters[co].data[CHD_CORPSEOWNER] = cn as i32;
                    } else {
                        characters[co].data[CHD_CORPSEOWNER] = 0;
                    }
                } else {
                    characters[co].data[CHD_CORPSEOWNER] = 0;
                }
            }
            #[cfg(not(feature = "KILLERONLY"))]
            {
                characters[co].data[core::constants::CHD_CORPSEOWNER] = 0;
            }

            characters[co].data[99] = 0;
            characters[co].data[98] = 0;

            characters[co].attack_cn = 0;
            characters[co].skill_nr = 0;
            characters[co].goto_x = 0;
            characters[co].use_nr = 0;
            characters[co].misc_action = 0;
            characters[co].stunned = 0;
            characters[co].retry = 0;
            characters[co].current_enemy = 0;
            for m in 0..4 {
                characters[co].enemy[m] = 0;
            }

            // Destroy active spells
            for n in 0..20 {
                if characters[co].spell[n] != 0 {
                    let item_idx = characters[co].spell[n] as usize;
                    characters[co].spell[n] = 0;
                    Repository::with_items_mut(|items| {
                        if item_idx < items.len() {
                            items[item_idx].used = core::constants::USE_EMPTY;
                        }
                    });
                }
            }
        });

        // If killer is a player, check for special items in grave
        let is_cn_player = if cn != 0 {
            Repository::with_characters(|ch| {
                Character::is_sane_character(cn)
                    && ch[cn].flags & CharacterFlags::CF_PLAYER.bits() != 0
            })
        } else {
            false
        };

        if is_cn_player {
            // TODO: Implement do_ransack_corpse
            log::info!("TODO: Ransack corpse {} by player {}", co, cn);
        }

        // Update character
        Repository::with_characters_mut(|ch| {
            ch[co].set_do_update_flags();
        });
    }

    /// Handle lab keeper death (special case)
    fn handle_labkeeper_death(&self, co: usize, cn: usize) {
        // TODO: Implement plr_map_remove
        log::info!("TODO: Remove character {} from map", co);

        // Destroy all items
        // TODO: Seems like we're getting rid of the items twice?
        God::destroy_items(co);
        Repository::with_characters_mut(|characters| {
            characters[co].citem = 0;
            characters[co].gold = 0;
            for z in 0..40 {
                characters[co].item[z] = 0;
            }
            for z in 0..20 {
                characters[co].worn[z] = 0;
            }
            characters[co].used = core::constants::USE_EMPTY;
        });

        // TODO: Implement use_labtransfer2
        log::info!("TODO: Lab transfer for character {} killed by {}", co, cn);
    }

    /// Handle item drops on death based on wimpy (guardian angel) chance
    fn handle_item_drops(&self, co: usize, cc: usize, wimp: i32, cn: usize) {
        use core::constants::*;

        // Handle gold
        Repository::with_characters_mut(|characters| {
            if characters[co].gold != 0 {
                let mut rng = rand::thread_rng();
                if wimp < rng.gen_range(0..100) {
                    characters[co].gold = 0;
                } else {
                    characters[cc].gold = 0;
                }
            }
        });

        // Handle inventory items
        for n in 0..40 {
            let item_idx = Repository::with_characters(|ch| ch[co].item[n]);
            if item_idx == 0 {
                continue;
            }

            // Check if item may be given
            if !self.do_maygive(cn, 0, item_idx as usize) {
                Repository::with_items_mut(|items| {
                    if (item_idx as usize) < items.len() {
                        items[item_idx as usize].used = USE_EMPTY;
                    }
                });
                Repository::with_characters_mut(|characters| {
                    characters[co].item[n] = 0;
                    characters[cc].item[n] = 0;
                });
                continue;
            }

            let mut rng = rand::thread_rng();
            if wimp <= rng.gen_range(0..100) {
                // Drop in grave
                Repository::with_characters_mut(|characters| {
                    characters[co].item[n] = 0;
                });
                Repository::with_items_mut(|items| {
                    if (item_idx as usize) < items.len() {
                        items[item_idx as usize].carried = cc as u16;

                        let item_template_to_print = items[item_idx as usize].temp;
                        log::info!(
                            "Dropped {} (t={}) in Grave",
                            items[item_idx as usize].get_name(),
                            item_template_to_print,
                        );
                    }
                });
            } else {
                // Player keeps it
                Repository::with_characters_mut(|characters| {
                    characters[cc].item[n] = 0;
                });
            }
        }

        // Handle carried item (citem)
        let citem = Repository::with_characters(|ch| ch[co].citem);
        if citem != 0 {
            if !self.do_maygive(cn, 0, citem as usize) {
                Repository::with_items_mut(|items| {
                    if (citem as usize) < items.len() {
                        items[citem as usize].used = USE_EMPTY;
                    }
                });
                Repository::with_characters_mut(|characters| {
                    characters[co].citem = 0;
                    characters[cc].citem = 0;
                });
            } else {
                let mut rng = rand::thread_rng();
                if wimp <= rng.gen_range(0..100) {
                    Repository::with_characters_mut(|characters| {
                        characters[co].citem = 0;
                    });
                    Repository::with_items_mut(|items| {
                        if (citem as usize) < items.len() {
                            items[citem as usize].carried = cc as u16;
                            let item_template_to_print = items[citem as usize].temp;
                            log::info!(
                                "Dropped {} (t={}) in Grave",
                                items[citem as usize].get_name(),
                                item_template_to_print,
                            );
                        }
                    });
                } else {
                    Repository::with_characters_mut(|characters| {
                        characters[cc].citem = 0;
                    });
                }
            }
        }

        // Handle worn items
        for n in 0..20 {
            let item_idx = Repository::with_characters(|ch| ch[co].worn[n]);
            if item_idx == 0 {
                continue;
            }

            if !self.do_maygive(cn, 0, item_idx as usize) {
                Repository::with_items_mut(|items| {
                    if (item_idx as usize) < items.len() {
                        items[item_idx as usize].used = USE_EMPTY;
                    }
                });
                Repository::with_characters_mut(|characters| {
                    characters[co].worn[n] = 0;
                    characters[cc].worn[n] = 0;
                });
                continue;
            }

            let mut rng = rand::thread_rng();
            if wimp <= rng.gen_range(0..100) {
                Repository::with_characters_mut(|characters| {
                    characters[co].worn[n] = 0;
                });
                Repository::with_items_mut(|items| {
                    if (item_idx as usize) < items.len() {
                        items[item_idx as usize].carried = cc as u16;
                        let item_template = items[item_idx as usize].temp;
                        log::info!(
                            "Dropped {} (t={}) in Grave",
                            items[item_idx as usize].get_name(),
                            item_template,
                        );
                    }
                });
            } else {
                Repository::with_characters_mut(|characters| {
                    characters[cc].worn[n] = 0;
                });
            }
        }

        // Handle active spells - always destroy
        for n in 0..20 {
            let spell_idx = Repository::with_characters(|ch| ch[co].spell[n]);
            if spell_idx != 0 {
                Repository::with_characters_mut(|characters| {
                    characters[co].spell[n] = 0;
                    characters[cc].spell[n] = 0;
                });
                Repository::with_items_mut(|items| {
                    if (spell_idx as usize) < items.len() {
                        items[spell_idx as usize].used = USE_EMPTY;
                    }
                });
            }
        }
    }

    /// Apply permanent stat loss on death
    fn apply_death_penalties(&self, co: usize) {
        Repository::with_characters_mut(|characters| {
            // HP penalty
            let mut hp_tmp = characters[co].hp[0] / 10;
            if characters[co].hp[0] - hp_tmp < 50 {
                hp_tmp = characters[co].hp[0] - 50;
            }
            if hp_tmp > 0 {
                self.do_character_log(
                    co,
                    FontColor::Red,
                    &format!("You lost {} hitpoints permanently.\n", hp_tmp),
                );
                log::info!("Character {} lost {} permanent hitpoints.", co, hp_tmp);
                for _ in 0..hp_tmp {
                    // TODO: Implement do_lower_hp
                }
            } else {
                self.do_character_log(
                    co,
                    FontColor::Red,
                    "You would have lost permanent hitpoints, but you're already at the minimum.\n",
                );
            }

            // Mana penalty
            let mut mana_tmp = characters[co].mana[0] / 10;
            if characters[co].mana[0] - mana_tmp < 50 {
                mana_tmp = characters[co].mana[0] - 50;
            }
            if mana_tmp > 0 {
                self.do_character_log(
                    co,
                    FontColor::Red,
                    &format!("You lost {} mana permanently.\n", mana_tmp),
                );
                log::info!("Character {} lost {} permanent mana.", co, mana_tmp);
                for _ in 0..mana_tmp {
                    // TODO: Implement do_lower_mana
                }
            } else {
                self.do_character_log(
                    co,
                    FontColor::Red,
                    "You would have lost permanent mana, but you're already at the minimum.\n",
                );
            }
        });
    }

    pub fn do_notify_character(
        &self,
        character_id: u32,
        notify_type: i32,
        dat1: i32,
        dat2: i32,
        dat3: i32,
        dat4: i32,
    ) {
        Driver::msg(character_id, notify_type, dat1, dat2, dat3, dat4);
    }

    // use this one sparingly! It uses quite a bit of computation time!
    /* This routine finds the 3 closest NPCs to the one doing the shouting,
    so that they can come to the shouter's rescue or something. */
    pub fn do_npc_shout(
        &self,
        cn: usize,
        shout_type: i32,
        dat1: i32,
        dat2: i32,
        dat3: i32,
        dat4: i32,
    ) {
        Repository::with_characters(|characters| {
            let mut best: [i32; 3] = [99; 3];
            let mut bestn: [i32; 3] = [0; 3];

            if characters[cn].data[52] == 3 {
                for co in 1..core::constants::MAXCHARS {
                    if co != cn
                        && characters[co].used == core::constants::USE_ACTIVE
                        && characters[co].flags & CharacterFlags::CF_BODY.bits() == 0
                    {
                        if characters[co].flags
                            & (CharacterFlags::CF_PLAYER | CharacterFlags::CF_USURP).bits()
                            != 0
                        {
                            continue;
                        }

                        if characters[co].data[53] != characters[cn].data[52] {
                            continue;
                        }

                        // TODO: This distance calculation seems incorrect potentially -- doublecheck
                        let distance = (characters[cn].x as i32 - characters[co].x as i32).abs()
                            + (characters[cn].y as i32 - characters[co].y as i32).abs();

                        if distance < best[0] {
                            best[2] = best[1];
                            bestn[2] = bestn[1];
                            best[1] = best[0];
                            bestn[1] = bestn[0];
                            best[0] = distance;
                            bestn[0] = co as i32;
                        } else if distance < best[1] {
                            best[2] = best[1];
                            bestn[2] = bestn[1];
                            best[1] = distance;
                            bestn[1] = co as i32;
                        }
                        // } else if distance < best[3] {
                        //     // TODO: Pretty sure [3] isn't safe
                        //     best[3] = distance;
                        //     bestn[3] = co as i32;
                        // }
                    }
                }

                for i in 0..bestn.len() {
                    if bestn[i] != 0 {
                        self.do_notify_character(
                            bestn[i] as u32,
                            shout_type,
                            dat1,
                            dat2,
                            dat3,
                            dat4,
                        );
                    }
                }
            } else {
                for co in 1..core::constants::MAXCHARS {
                    if co != cn
                        && characters[co].used == core::constants::USE_ACTIVE
                        && characters[co].flags & CharacterFlags::CF_BODY.bits() == 0
                    {
                        if characters[co].flags
                            & (CharacterFlags::CF_PLAYER | CharacterFlags::CF_USURP).bits()
                            != 0
                        {
                            continue;
                        }

                        if characters[co].data[53] != characters[cn].data[52] {
                            continue;
                        }

                        self.do_notify_character(co as u32, shout_type, dat1, dat2, dat3, dat4);
                    }
                }
            }
        });
    }

    /// Sort character inventory based on order string
    /// Port of do_sort from svr_do.cpp
    pub fn do_sort(&self, cn: usize, order: &str) {
        // Check if character is in building mode
        let is_building = Repository::with_characters(|characters| characters[cn].is_building());

        if is_building {
            // TODO: Add do_char_log to send message to character
            log::info!("Character {} tried to sort while in build mode", cn);
            return;
        }

        // Get a copy of the items array to sort
        let mut items = Repository::with_characters(|characters| characters[cn].item);

        // Sort using custom comparison function based on order string
        items.sort_by(|&a, &b| self.qsort_compare(a as usize, b as usize, order));

        // Write sorted items back
        Repository::with_characters_mut(|characters| {
            characters[cn].item = items;
        });

        // Update character to send changes to client
        // TODO: Implement do_update_char equivalent
        // For now, this will at least sort the inventory in memory
        NetworkManager::with(|nm| {
            let player_id = Repository::with_characters(|characters| characters[cn].player);
            if player_id > 0 && player_id < MAXPLAYER as i32 {
                // TODO: Send character inventory update to client
            }
        });
    }

    /// Comparison function for sorting items
    /// Port of qsort_proc from svr_do.cpp
    fn qsort_compare(&self, in1: usize, in2: usize, order: &str) -> std::cmp::Ordering {
        use std::cmp::Ordering;

        // Handle empty slots - they go to the end
        if in1 == 0 && in2 == 0 {
            return Ordering::Equal;
        }
        if in1 != 0 && in2 == 0 {
            return Ordering::Less;
        }
        if in1 == 0 && in2 != 0 {
            return Ordering::Greater;
        }

        // Compare based on order string criteria
        Repository::with_items(|items| {
            let item1 = &items[in1];
            let item2 = &items[in2];

            for ch in order.chars() {
                match ch {
                    'w' => {
                        // Sort by weapon
                        let is_weapon1 =
                            item1.flags & core::constants::ItemFlags::IF_WEAPON.bits() != 0;
                        let is_weapon2 =
                            item2.flags & core::constants::ItemFlags::IF_WEAPON.bits() != 0;
                        if is_weapon1 && !is_weapon2 {
                            return Ordering::Less;
                        }
                        if !is_weapon1 && is_weapon2 {
                            return Ordering::Greater;
                        }
                    }
                    'a' => {
                        // Sort by armor
                        let is_armor1 =
                            item1.flags & core::constants::ItemFlags::IF_ARMOR.bits() != 0;
                        let is_armor2 =
                            item2.flags & core::constants::ItemFlags::IF_ARMOR.bits() != 0;
                        if is_armor1 && !is_armor2 {
                            return Ordering::Less;
                        }
                        if !is_armor1 && is_armor2 {
                            return Ordering::Greater;
                        }
                    }
                    'p' => {
                        // Sort by usable/consumable (use-destroy)
                        let is_usedestroy1 =
                            item1.flags & core::constants::ItemFlags::IF_USEDESTROY.bits() != 0;
                        let is_usedestroy2 =
                            item2.flags & core::constants::ItemFlags::IF_USEDESTROY.bits() != 0;
                        if is_usedestroy1 && !is_usedestroy2 {
                            return Ordering::Less;
                        }
                        if !is_usedestroy1 && is_usedestroy2 {
                            return Ordering::Greater;
                        }
                    }
                    'h' => {
                        // Sort by HP (higher first)
                        if item1.hp[0] > item2.hp[0] {
                            return Ordering::Less;
                        }
                        if item1.hp[0] < item2.hp[0] {
                            return Ordering::Greater;
                        }
                    }
                    'e' => {
                        // Sort by endurance (higher first)
                        if item1.end[0] > item2.end[0] {
                            return Ordering::Less;
                        }
                        if item1.end[0] < item2.end[0] {
                            return Ordering::Greater;
                        }
                    }
                    'm' => {
                        // Sort by mana (higher first)
                        if item1.mana[0] > item2.mana[0] {
                            return Ordering::Less;
                        }
                        if item1.mana[0] < item2.mana[0] {
                            return Ordering::Greater;
                        }
                    }
                    'v' => {
                        // Sort by value (higher first)
                        if item1.value > item2.value {
                            return Ordering::Less;
                        }
                        if item1.value < item2.value {
                            return Ordering::Greater;
                        }
                    }
                    _ => {
                        // Unknown character, skip
                    }
                }
            }

            // Fall back to sort by value
            if item1.value > item2.value {
                return Ordering::Less;
            }
            if item1.value < item2.value {
                return Ordering::Greater;
            }

            // Finally sort by temp (to maintain stability)
            if item1.temp > item2.temp {
                return Ordering::Greater;
            }
            if item1.temp < item2.temp {
                return Ordering::Less;
            }

            Ordering::Equal
        })
    }

    pub fn do_maygive(&self, cn: usize, co: usize, item_idx: usize) -> bool {}

    pub fn do_give(&self, cn: usize, co: usize) -> bool {}

    pub fn really_update_char(&self, cn: usize) {}

    pub fn do_regenerate(&self, cn: usize) {}

    pub fn do_raise_attrib(&self, cn: usize, attrib: i32) -> bool {}

    pub fn do_raise_hp(&self, cn: usize) -> bool {}

    pub fn do_lower_hp(&self, cn: usize) -> bool {}

    pub fn do_lower_mana(&self, cn: usize) -> bool {}

    pub fn do_raise_end(&self, cn: usize) -> bool {}

    pub fn do_raise_mana(&self, cn: usize) -> bool {}

    pub fn do_raise_skill(&self, cn: usize, skill: i32) -> bool {}

    pub fn do_item_value(&self, item_idx: usize) -> i32 {}

    pub fn do_look_item(&self, cn: usize, item_idx: usize) {}

    pub fn barter(&self, cn: usize, opr: i32, flag: i32) -> i32 {}

    pub fn do_shop_char(&self, cn: usize, co: usize, nr: i32) {}

    pub fn do_depot_cost(&self, item_idx: usize) -> i32 {}

    pub fn do_add_depot(&self, cn: usize, item_idx: usize) -> bool {}

    pub fn do_pay_depot(&self, cn: usize) {}

    pub fn do_depot_char(&self, cn: usize, co: usize, nr: i32) {}

    pub fn do_look_char(&self, cn: usize, co: usize, godflag: i32, autoflag: i32, lootflag: i32) {}

    pub fn do_look_depot(&self, cn: usize, co: usize) {}

    pub fn do_look_player_depot(&self, cn: usize, cv: &str) {}

    pub fn do_look_player_inventory(&self, cn: usize, cv: &str) {}

    pub fn do_look_player_equipment(&self, cn: usize, cv: &str) {}

    pub fn do_steal_player(&self, cn: usize, cv: &str, ci: &str) -> bool {}
}
