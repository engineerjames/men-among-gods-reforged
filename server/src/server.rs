use core::constants::MAXPLAYER;
use std::rc::Rc;

use crate::lab9::Labyrinth9;
use crate::network_manager::NetworkManager;
use crate::repository::Repository;
use crate::state::State;

pub struct Server {
    players: [core::types::ServerPlayer; MAXPLAYER],
}

impl Server {
    pub fn new() -> Self {
        Server {
            players: std::array::from_fn(|_| core::types::ServerPlayer::new()),
        }
    }

    pub fn initialize(&mut self) -> Result<(), String> {
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

            State::with_mut(|state| {
                state.logout_player(i, None, crate::enums::LogoutReason::Shutdown);
            });
        }

        // Initialize subsystems
        Labyrinth9::init();
        // state.reset_changed_items();

        // remove lab items from all players (leave this here for a while!)
        // for n in 1..MAXITEM {
        //     if state.it[n].used == USE_EMPTY {
        //         continue;
        //     }
        //     if state.it[n].has_laby_destroy() {
        //         state.tmplabcheck(n);
        //     }
        //     if state.it[n].has_soulstone() {
        //         // Copy from packed struct to avoid unaligned reference
        //         let max_damage = { state.it[n].max_damage };
        //         if max_damage == 0 {
        //             state.it[n].max_damage = 60000;
        //             let name = state.it[n].get_name();
        //             //xlog!(state.logger, "Set {} ({}) max_damage to 60000", name, n);
        //         }
        //     }
        // }

        // Validate character template positions
        // for n in 1..MAXTCHARS {
        //     if state.ch_temp[n].used == USE_EMPTY {
        //         continue;
        //     }

        //     let x = state.ch_temp[n].data[29] % SERVER_MAPX;
        //     let y = state.ch_temp[n].data[29] / SERVER_MAPX;

        //     if x == 0 && y == 0 {
        //         continue;
        //     }

        //     let ch_x = state.ch_temp[n].x as i32;
        //     let ch_y = state.ch_temp[n].y as i32;

        //     if (x - ch_x).abs() + (y - ch_y).abs() > 200 {
        //         // xlog!(state.logger, "RESET {} ({}): {} {} -> {} {}",
        //         //     n,
        //         //     std::str::from_utf8(&state.ch_temp[n].name)
        //         //         .unwrap_or("*unknown*")
        //         //         .trim_end_matches('\0'),
        //         //     ch_x, ch_y, x, y);
        //         state.ch_temp[n].data[29] = state.ch_temp[n].x as i32 + state.ch_temp[n].y as i32 * SERVER_MAPX;
        //     }
        // }

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
