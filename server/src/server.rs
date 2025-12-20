use core::constants::MAXPLAYER;

use crate::repository::Repository;

pub struct Server<'a> {
    repository: &'a mut Repository,
    players: [crate::core::types::ServerPlayer; MAXPLAYER],
    state: crate::state::State,
}

impl<'a> Server<'a> {
    pub fn new(repository: &'a mut Repository) -> Self {
        Server {
            repository,
            players: std::array::from_fn(|_| crate::core::types::ServerPlayer::new()),
            state: crate::state::State::new(),
        }
    }

    pub fn initialize(&mut self) {
        // Mark data as dirty (in use)
        self.repository.globals.set_dirty(true);

        // Log out all active characters (cleanup from previous run)
        // for n in 1..MAXCHARS {
        //     if state.ch[n].used == USE_ACTIVE {
        //         state.plr_logout(n, 0, LO_SHUTDOWN);
        //     }
        // }

        // Initialize subsystems
        // state.init_node();
        // state.init_lab9();
        // state.god_init_freelist();
        // state.god_init_badnames();
        // state.init_badwords();
        // state.god_read_banlist();
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

        // state.load_mod();
    }

    pub fn tick(&mut self) {
        // Main server loop implementation goes here
    }
}

impl Drop for Server<'_> {
    fn drop(&mut self) {
        // On server shutdown, clear the dirty flag
        self.repository.globals.set_dirty(false);
    }
}
