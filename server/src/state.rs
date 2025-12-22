use core::constants::{CharacterFlags, MAXCHARS, MAXPLAYER};
use core::types::Character;
use std::cmp;
use std::rc::Rc;
use std::sync::{OnceLock, RwLock};

use crate::enums;
use crate::god::God;
use crate::network_manager::NetworkManager;
use crate::path_finding::PathFinder;
use crate::repository::Repository;

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

                                    character.citem = 0;
                                }
                            }
                        }
                    });
                }

                // Clear map positions
                let (map_index, to_map_index, light) = Repository::with_characters(|characters| {
                    let character = &characters[character_id];
                    let map_index = (character.y as usize) * core::constants::MAPX as usize
                        + (character.x as usize);
                    let to_map_index = (character.toy as usize) * core::constants::MAPX as usize
                        + (character.tox as usize);
                    (map_index, to_map_index, character.light)
                });

                Repository::with_map_mut(|map| {
                    if map[map_index].ch == character_id as u32 {
                        map[map_index].ch = 0;
                        if light != 0 {
                            // TODO: Update lighting here via do_add_light
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
                            let map_index = (character.y as usize) * core::constants::MAPX as usize
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
        &self, // TODO: Rework these functions to pass in just the ids around
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

                NetworkManager::with(|network| {
                    network.xsend(player_id as usize, &buffer, 16);
                });

                bytes_sent += 15;
            }
        }
    }

    pub fn do_add_light(
        &mut self,
        _repository: &Repository,
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

                let v = self.can_see(
                    see_map,
                    None,
                    x_center,
                    y_center,
                    x,
                    y,
                    core::constants::LIGHTDIST,
                );

                if v != 0 {
                    let d = strength / (v * (x_center - x).abs() + (y_center - y).abs());
                    let map_index = (y as usize) * core::constants::MAPX as usize + (x as usize);

                    if flag == 1 {
                        map_tiles[map_index].add_light(-d);
                    } else {
                        map_tiles[map_index].add_light(d);
                    }
                }
            }
        }
    }

    pub fn can_see(
        &mut self,
        see_map: &mut [core::types::SeeMap],
        character_id: Option<usize>,
        fx: i32,
        fy: i32,
        tx: i32,
        ty: i32,
        max_distance: i32,
    ) -> i32 {
        Repository::with_characters(|characters| {
            match character_id {
                Some(cn) => {
                    if (fx != see_map[cn].x) || (fy != see_map[cn].y) {
                        if characters[cn].is_monster() && !characters[cn].is_usurp_or_thrall() {
                            self.is_monster = true;
                        }

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
        let map_index = x + y * core::constants::MAPX;

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
                        + characters[co].y as usize * core::constants::MAPX;
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

                let can_see = Repository::with_see_map_mut(|see_map| {
                    if !self
                        .can_see(
                            see_map,
                            Some(cn),
                            characters[cn].x as i32,
                            characters[cn].y as i32,
                            characters[co].x as i32,
                            characters[co].y as i32,
                            15,
                        )
                        .ne(&0)
                    {
                        return false;
                    }

                    return true;
                });

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
            1
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
            || x >= core::constants::MAPX as i32
            || y <= 0
            || y >= core::constants::MAPY as i32
        {
            return false;
        }

        let m = (x + y * core::constants::MAPX as i32) as usize;

        // Check if it's a monster and the map blocks monsters
        if self.is_monster {
            let blocked = Repository::with_map(|map| {
                map[m].flags & core::constants::MF_MOVEBLOCK as u64 != 0
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
}
