/*************************************************************************

This file is part of 'Mercenaries of Astonia v2'
Copyright (c) 1997-2001 Daniel Brockhaus (joker@astonia.com)
All rights reserved.

Rust port maintains original logic and comments.

**************************************************************************/

//! Mercenaries of Astonia Server - Rust Implementation
//!
//! This is a Rust port of the original C++ server code, maintaining
//! the same execution flow and logic while using Rust idioms.

mod constants;
mod types;
mod player;
mod logging;
mod network;
mod game_loop;
mod profiling;
mod god;
mod population;
mod state_mgmt;
mod player_control;

use std::env;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::process;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use signal_hook::consts::{SIGHUP, SIGINT, SIGQUIT, SIGTERM};
use signal_hook::iterator::Signals;

use constants::*;
use game_loop::{game_loop, GameState};
use logging::Logger;
use network::NetworkManager;

/// Global quit flag - set by signal handlers
static QUIT: AtomicBool = AtomicBool::new(false);

fn main() {
    let args: Vec<String> = env::args().collect();
    
    // nice(5) equivalent - lower process priority
    #[cfg(unix)]
    unsafe {
        libc::nice(5);
    }
    
    // Always run in foreground with stdout logging
    let logger = Logger::new_file("server.log").unwrap_or_else(|e| {
        eprintln!("Failed to open log file: {}. Falling back to stdout.", e);
        Logger::new_stdout()
    });
    
    xlog!(logger, "Mercenaries of Astonia Server v{}.{:02}.{:02}", 
        VERSION >> 16, (VERSION >> 8) & 255, VERSION & 255);
    xlog!(logger, "Copyright (C) 1997-2001 Daniel Brockhaus");
    
    // Create network manager
    let network = match NetworkManager::new() {
        Ok(n) => n,
        Err(e) => {
            xlog!(logger, "Failed to create network manager: {}", e);
            process::exit(1);
        }
    };
    
    // Create game state
    let mut state = GameState::new(logger, network);
    
    // Calibrate profiler
    xlog!(state.logger, "Running speed test...");
    state.profiler.calibrate();
    xlog!(state.logger, "Speed test: {:.0} MHz", state.profiler.cycles_per_sec / 1_000_000.0);
    
    // Set up signal handlers
    // ignore the silly pipe errors:
    // signal(SIGPIPE, SIG_IGN);
    
    let quit_flag = Arc::new(AtomicBool::new(false));
    let quit_flag_clone = quit_flag.clone();
    
    // Set up signal handling in a separate thread
    std::thread::spawn(move || {
        let mut signals = Signals::new(&[SIGINT, SIGTERM, SIGQUIT, SIGHUP]).unwrap();
        for sig in signals.forever() {
            match sig {
                SIGHUP => {
                    // Log rotation would happen here
                    // For now, just note it
                    println!("SIGHUP received");
                }
                SIGINT | SIGTERM | SIGQUIT => {
                    if !quit_flag_clone.load(Ordering::SeqCst) {
                        println!("Got signal to terminate. Shutdown initiated...");
                    } else {
                        println!("Alright, alright, I'm already terminating!");
                    }
                    quit_flag_clone.store(true, Ordering::SeqCst);
                }
                _ => {}
            }
        }
    });
    
    // Load game data
    if !state.load() {
        xlog!(state.logger, "load() failed.");
        process::exit(1);
    }
    
    // Check for dirty flag
    if (state.globs.flags & GF_DIRTY) != 0 {
        xlog!(state.logger, "Data files were not cleanly unmounted.");
        if args.len() != 2 {
            process::exit(1);
        }
    }
    
    // Handle command-line arguments for maintenance tasks
    if args.len() == 2 {
        let cmd = args[1].to_lowercase();
        match cmd.as_str() {
            "pop" => {
                state.populate();
                state.unload();
                process::exit(0);
            }
            "rem" => {
                state.pop_remove();
                state.unload();
                process::exit(0);
            }
            "wipe" => {
                state.pop_wipe();
                state.unload();
                process::exit(0);
            }
            "light" => {
                state.init_lights();
                state.unload();
                process::exit(0);
            }
            "skill" => {
                state.pop_skill();
                state.unload();
                process::exit(0);
            }
            "load" => {
                state.pop_load_all_chars();
                state.unload();
                process::exit(0);
            }
            "save" => {
                state.pop_save_all_chars();
                state.unload();
                process::exit(0);
            }
            _ => {}
        }
    }
    
    // Log out all active characters (cleanup from previous run)
    for n in 1..MAXCHARS {
        if state.ch[n].used == USE_ACTIVE {
            state.plr_logout(n, 0, LO_SHUTDOWN);
        }
    }
    
    // Set up PID file
    if let Ok(mut pidfile) = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open("server.pid")
    {
        let _ = writeln!(pidfile, "{}", process::id());
        #[cfg(unix)]
        let _ = set_permissions_mode("server.pid", 0o664);
    }
    
    // Initialize subsystems
    state.init_node();
    state.init_lab9();
    state.god_init_freelist();
    state.god_init_badnames();
    state.init_badwords();
    state.god_read_banlist();
    state.reset_changed_items();
    
    // remove lab items from all players (leave this here for a while!)
    for n in 1..MAXITEM {
        if state.it[n].used == USE_EMPTY {
            continue;
        }
        if state.it[n].has_laby_destroy() {
            state.tmplabcheck(n);
        }
        if state.it[n].has_soulstone() {
            // Copy from packed struct to avoid unaligned reference
            let max_damage = { state.it[n].max_damage };
            if max_damage == 0 {
                state.it[n].max_damage = 60000;
                let name = state.it[n].get_name();
                xlog!(state.logger, "Set {} ({}) max_damage to 60000", name, n);
            }
        }
    }
    
    // Validate character template positions
    for n in 1..MAXTCHARS {
        if state.ch_temp[n].used == USE_EMPTY {
            continue;
        }
        
        let x = state.ch_temp[n].data[29] % SERVER_MAPX;
        let y = state.ch_temp[n].data[29] / SERVER_MAPX;
        
        if x == 0 && y == 0 {
            continue;
        }
        
        let ch_x = state.ch_temp[n].x as i32;
        let ch_y = state.ch_temp[n].y as i32;
        
        if (x - ch_x).abs() + (y - ch_y).abs() > 200 {
            xlog!(state.logger, "RESET {} ({}): {} {} -> {} {}", 
                n, 
                std::str::from_utf8(&state.ch_temp[n].name)
                    .unwrap_or("*unknown*")
                    .trim_end_matches('\0'),
                ch_x, ch_y, x, y);
            state.ch_temp[n].data[29] = state.ch_temp[n].x as i32 + state.ch_temp[n].y as i32 * SERVER_MAPX;
        }
    }
    
    // Mark data as dirty (in use)
    state.globs.flags |= GF_DIRTY;
    
    state.load_mod();
    
    xlog!(state.logger, "Entering game loop...");
    
    // Main game loop
    let mut doleave = false;
    let mut ltimer = 0;
    
    while !doleave {
        if (state.globs.ticker & 4095) == 0 {
            state.load_mod();
            // update();
        }
        
        game_loop(&mut state);
        
        if quit_flag.load(Ordering::SeqCst) {
            if ltimer == 0 {
                // kick all players
                for n in 1..MAXPLAYER {
                    if state.players.players[n].is_connected() {
                        let usnr = state.players.players[n].usnr;
                        state.plr_logout(usnr, n, LO_SHUTDOWN);
                    }
                }
                xlog!(state.logger, "Sending shutdown message");
                ltimer += 1;
            } else {
                ltimer += 1;
            }
            
            if ltimer > 25 {  // reset this to 250 !!!
                xlog!(state.logger, "Leaving main loop");
                // safety measure only. Players should be out already
                for n in 1..MAXPLAYER {
                    if state.players.players[n].is_connected() {
                        state.players.players[n].disconnect();
                    }
                }
                doleave = true;
            }
        }
    }
    
    // Clean shutdown
    state.globs.flags &= !GF_DIRTY;
    state.unload();
    
    xlog!(state.logger, "Server down ({},{})", state.see_hit, state.see_miss);
    
    // Remove PID file
    let _ = fs::remove_file("server.pid");
}

/// Extension trait for setting file permissions on Unix
#[cfg(unix)]
fn set_permissions_mode(path: &str, mode: u32) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(mode))
}
