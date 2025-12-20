mod repository;

use log;
use std::env;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::process;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use signal_hook::consts::{SIGHUP, SIGINT, SIGQUIT, SIGTERM};
use signal_hook::iterator::Signals;

use core;

use crate::repository::Repository;

fn main() {
    let args: Vec<String> = env::args().collect();

    // The original implementation had a call here for nice(5) to lower process priority.
    // This is platform dependent and omitted for simplicity.

    // Initialize logging
    core::initialize_logger(log::LevelFilter::Debug, Some("server.log")).unwrap_or_else(|e| {
        eprintln!("Failed to initialize logger: {}. Exiting.", e);
        process::exit(1);
    });

    log::info!("Starting Men Among Gods: Reforged Server v0.0.1");

    let quit_flag = Arc::new(AtomicBool::new(false));
    let quit_flag_clone = quit_flag.clone();

    // Set up signal handling in a separate thread
    let mut signals = Signals::new(&[SIGINT, SIGTERM, SIGQUIT, SIGHUP]).unwrap();
    let handle = signals.handle();

    let signal_thread = std::thread::spawn(move || {
        for sig in signals.forever() {
            match sig {
                SIGINT | SIGTERM | SIGQUIT => {
                    if !quit_flag_clone.load(Ordering::SeqCst) {
                        log::info!("Got signal to terminate. Shutdown initiated...");
                    } else {
                        log::info!("Alright, alright, I'm already terminating!");
                    }
                    quit_flag_clone.store(true, Ordering::SeqCst);
                }
                _ => {
                    log::warn!("Received unsupported signal: {}", sig);
                }
            }
        }
    });

    // Load game data
    let mut repository = Repository::new();
    if let Err(e) = repository.load() {
        log::error!("Failed to load game data: {}. Exiting.", e);
        process::exit(1);
    }

    // Check for dirty flag
    if repository.globals.is_dirty() {
        // xlog!(state.logger, "Data files were not cleanly unmounted.");
        if args.len() != 2 {
            process::exit(1);
        }
    }

    // TODO: Handle command-line arguments for maintenance tasks
    if args.len() == 2 {
        let cmd = args[1].to_lowercase();
        match cmd.as_str() {
            "pop" => {
                // state.populate();
                // state.unload();
                process::exit(0);
            }
            "rem" => {
                // state.pop_remove();
                // state.unload();
                process::exit(0);
            }
            "wipe" => {
                // state.pop_wipe();
                // state.unload();
                process::exit(0);
            }
            "light" => {
                // state.init_lights();
                // state.unload();
                process::exit(0);
            }
            "skill" => {
                // state.pop_skill();
                // state.unload();
                process::exit(0);
            }
            "load" => {
                // state.pop_load_all_chars();
                // state.unload();
                process::exit(0);
            }
            "save" => {
                // state.pop_save_all_chars();
                // state.unload();
                process::exit(0);
            }
            _ => {}
        }
    }

    // Log out all active characters (cleanup from previous run)
    // for n in 1..MAXCHARS {
    //     if state.ch[n].used == USE_ACTIVE {
    //         state.plr_logout(n, 0, LO_SHUTDOWN);
    //     }
    // }

    // Set up PID file
    if let Ok(mut pidfile) = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open("server.pid")
    {
        let _ = writeln!(pidfile, "{}", process::id());
    }

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

    // Mark data as dirty (in use)
    repository.globals.set_dirty(true);

    // state.load_mod();

    log::info!("Entering main game loop...");

    // Main game loop
    let mut doleave = false;
    let mut ltimer = 0;

    while !doleave {
        // if (state.globs.ticker & 4095) == 0 {
        //     state.load_mod();
        //     // update();
        // }

        // game_loop(&mut state);

        if quit_flag.load(Ordering::SeqCst) {
            if ltimer == 0 {
                // kick all players
                for _n in 1..core::constants::MAXPLAYER {
                    // if state.players.players[n].is_connected() {
                    //     let usnr = state.players.players[n].usnr;
                    //     state.plr_logout(usnr, n, LO_SHUTDOWN);
                    // }
                }
                //xlog!(state.logger, "Sending shutdown message");
                ltimer += 1;
            } else {
                ltimer += 1;
            }

            if ltimer > 25 {
                // reset this to 250 !!!
                //xlog!(state.logger, "Leaving main loop");
                // safety measure only. Players should be out already
                for _n in 1..core::constants::MAXPLAYER {
                    // if state.players.players[n].is_connected() {
                    //     state.players.players[n].disconnect();
                    // }
                }
                doleave = true;
            }
        }
    }

    // Clean shutdown
    repository.globals.set_dirty(false);
    // state.unload();

    log::info!("Server shutdown complete.");

    // Remove PID file
    let _ = fs::remove_file("server.pid");
    handle.close();
    signal_thread.join().unwrap(); // TODO: Better error handling here
}
