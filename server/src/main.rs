mod area;
mod background_saver;
mod driver;
mod effect;
mod enums;
mod game_state;
mod god;
mod keydb;
mod keydb_store;
mod types;

#[macro_use]
pub mod helpers;
mod lab9;
mod network_manager;
mod path_finding;
mod player;
mod points;
mod populate;
mod server;
mod single_thread_cell;
mod state;
mod talk;
mod tls;

use log;
use std::env;
use std::process;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use core;

use crate::game_state::GameState;
use crate::server::Server;

fn handle_command_line_args(args: &[String], gs: &mut GameState) {
    if args.len() == 2 {
        let cmd = args[1].to_lowercase();
        match cmd.as_str() {
            "pop" => {
                populate::populate(gs);
                process::exit(0);
            }
            "wipe" => {
                populate::pop_wipe(gs);
                process::exit(0);
            }
            "light" => {
                populate::init_lights(gs);
                process::exit(0);
            }
            "skill" => {
                populate::pop_skill(gs);
                process::exit(0);
            }
            "load" => {
                populate::pop_load_all_chars(gs);
                process::exit(0);
            }
            "save" => {
                populate::pop_save_all_chars(gs);
                process::exit(0);
            }
            _ => {}
        }
    }
}

fn main() -> Result<(), String> {
    let args: Vec<String> = env::args().collect();

    // The original implementation had a call here for nice(5) to lower process priority.
    // This is platform dependent and omitted for simplicity.

    core::initialize_logger(log::LevelFilter::Info, Some("server.log")).unwrap_or_else(|e| {
        eprintln!("Failed to initialize logger: {}. Exiting.", e);
        process::exit(1);
    });

    log::info!("Starting Men Among Gods: Reforged Server v1.1.0");
    log::info!("Process PID: {}", process::id());

    let quit_flag = Arc::new(AtomicBool::new(false));
    let quit_flag_clone = quit_flag.clone();

    let handler_result = ctrlc::set_handler(move || {
        if !quit_flag_clone.load(Ordering::SeqCst) {
            log::info!("Ctrl-C received. Shutdown initiated...");
        } else {
            log::info!("Alright, alright, I'm already terminating!");
        }
        quit_flag_clone.store(true, Ordering::SeqCst);
    });

    if let Err(e) = handler_result {
        log::error!("Error setting Ctrl-C handler: {}. Exiting.", e);
        process::exit(1);
    }

    // Initialize unified game state (owns its own copy of all world data)
    let mut gs = GameState::initialize().unwrap_or_else(|e| {
        log::error!("Failed to initialize game state: {}. Exiting.", e);
        process::exit(1);
    });

    // Handle CLI subcommands
    handle_command_line_args(&args, &mut gs);

    // Check for dirty flag
    if gs.globals.is_dirty() {
        log::error!("Data files were not closed cleanly last time. Exiting.");
        process::exit(1);
    }

    // Register GameState as a global singleton so that modules not yet converted
    // to receive `gs: &mut GameState` can still access it via State::with / State::with_mut.
    GameState::register_global(gs);

    let mut server = server::Server::new();

    server
        .initialize(GameState::global_mut())
        .unwrap_or_else(|e| {
            log::error!("Failed to initialize server: {}. Exiting.", e);
            process::exit(1);
        });

    log::info!("Entering main game loop...");

    while !quit_flag.load(Ordering::SeqCst) {
        server.tick(GameState::global_mut());
    }

    log::info!("Shutdown signal received, exiting main loop...");
    let mut logout_entries: Vec<(usize, usize)> = Vec::new();
    Server::with_players(|players| {
        for n in 1..core::constants::MAXPLAYER {
            logout_entries.push((players[n].usnr, n));
        }
    });
    {
        let gs = GameState::global_mut();
        for (usnr, n) in &logout_entries {
            player::plr_logout(gs, *usnr, *n, enums::LogoutReason::Shutdown);
        }
    }

    // Shut down background saver thread (flushes pending writes)
    server.shutdown_background_saver();

    // Perform a full synchronous save to the unified game state
    GameState::global_mut().shutdown();

    // TODO: Wait some amount of time and forceably close all sockets

    log::info!("Server shutdown complete.");

    Ok(())
}
