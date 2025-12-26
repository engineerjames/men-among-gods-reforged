use core::constants::MAXPLAYER;
use core::types::{Character, ServerPlayer};
use std::net::TcpListener;
use std::sync::{OnceLock, RwLock};

use crate::god::God;
use crate::lab9::Labyrinth9;
use crate::network_manager::NetworkManager;
use crate::repository::Repository;
use crate::state::State;
use crate::{player, populate};

static PLAYERS: OnceLock<RwLock<[core::types::ServerPlayer; MAXPLAYER]>> = OnceLock::new();

pub struct Server {
    sock: Option<TcpListener>,
}

impl Server {
    pub fn new() -> Self {
        Server { sock: None }
    }

    pub fn initialize_players() -> Result<(), String> {
        let players = std::array::from_fn(|_| core::types::ServerPlayer::new());
        PLAYERS
            .set(RwLock::new(players))
            .map_err(|_| "Players already initialized".to_string())?;
        Ok(())
    }

    pub fn with_players<F, R>(f: F) -> R
    where
        F: FnOnce(&[core::types::ServerPlayer]) -> R,
    {
        let players = PLAYERS
            .get()
            .expect("Players not initialized")
            .read()
            .unwrap();
        f(&players[..])
    }

    pub fn with_players_mut<F, R>(f: F) -> R
    where
        F: FnOnce(&mut [core::types::ServerPlayer]) -> R,
    {
        let mut players = PLAYERS
            .get()
            .expect("Players not initialized")
            .write()
            .unwrap();
        f(&mut players[..])
    }

    fn tmplabcheck(item_idx: usize) {
        Repository::with_characters(|ch| {
            Repository::with_items_mut(|it| {
                let cn = it[item_idx].carried as usize;
                if cn == 0 || !ServerPlayer::is_sane_player(cn) {
                    return;
                }

                // player is inside a lab?
                if ch[cn].temple_x != 512 && ch[cn].temple_x != 558 && ch[cn].temple_x != 813 {
                    return;
                }

                God::take_from_char(item_idx, cn);
                it[item_idx].used = core::constants::USE_EMPTY;

                log::warn!(
                    "Removed Lab Item {} from player {}",
                    it[item_idx].get_name(),
                    cn
                );
            });
        });
    }

    pub fn initialize(&mut self) -> Result<(), String> {
        // Create and configure TCP socket (matching server.cpp socket setup)
        let listener = TcpListener::bind("0.0.0.0:5555")
            .map_err(|e| format!("Failed to bind socket: {}", e))?;

        listener
            .set_nonblocking(true)
            .map_err(|e| format!("Failed to set non-blocking mode: {}", e))?;

        self.sock = Some(listener);
        log::info!("Socket bound to port 5555");

        Server::initialize_players()?;
        Repository::initialize()?;
        State::initialize()?;
        NetworkManager::initialize()?;

        // Mark data as dirty (in use)
        Repository::with_globals_mut(|globals| {
            globals.set_dirty(true);
        });

        // Log out all active characters (cleanup from previous run)
        for i in 0..core::constants::MAXCHARS as usize {
            let should_logout = Repository::with_characters(|characters| {
                characters[i].used == core::constants::USE_ACTIVE
            });

            if !should_logout {
                continue;
            }

            Repository::with_characters(|characters| {
                log::info!(
                    "Logging out character '{}' on server startup",
                    characters[i].get_name(),
                );
            });

            player::plr_logout(i, 0, crate::enums::LogoutReason::Shutdown);
        }

        // Initialize subsystems
        Labyrinth9::initialize()?;
        populate::reset_changed_items();

        // remove lab items from all players (leave this here for a while!)
        Repository::with_items_mut(|it| {
            for n in 1..core::constants::MAXITEM {
                if it[n].used == core::constants::USE_EMPTY {
                    continue;
                }
                if it[n].has_laby_destroy() {
                    Self::tmplabcheck(n);
                }
                if it[n].has_soulstone() {
                    // Copy from packed struct to avoid unaligned reference
                    let max_damage = { it[n].max_damage };
                    if max_damage == 0 {
                        it[n].max_damage = 60000;
                        let name = it[n].get_name();
                        log::info!("Set {} ({}) max_damage to 60000", name, n);
                    }
                }
            }
        });

        // Validate character template positions
        Repository::with_character_templates_mut(|ch_temp| {
            for n in 1..core::constants::MAXTCHARS {
                if ch_temp[n].used == core::constants::USE_EMPTY {
                    continue;
                }

                let x = ch_temp[n].data[29] % core::constants::SERVER_MAPX;
                let y = ch_temp[n].data[29] / core::constants::SERVER_MAPX;

                if x == 0 && y == 0 {
                    continue;
                }

                let ch_x = ch_temp[n].x as i32;
                let ch_y = ch_temp[n].y as i32;

                if (x - ch_x).abs() + (y - ch_y).abs() > 200 {
                    log::warn!(
                        "RESET {} ({}): {} {} -> {} {}",
                        n,
                        std::str::from_utf8(&ch_temp[n].name)
                            .unwrap_or("*unknown*")
                            .trim_end_matches('\0'),
                        ch_x,
                        ch_y,
                        x,
                        y
                    );
                    ch_temp[n].data[29] =
                        ch_temp[n].x as i32 + ch_temp[n].y as i32 * core::constants::SERVER_MAPX;
                }
            }
        });

        Ok(())
    }

    pub fn tick(&mut self) {
        // Main server loop implementation goes here
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        log::info!("Server shutting down, marking data as clean.");
        Repository::with_globals_mut(|globals| {
            globals.set_dirty(false);
        });
    }
}
