mod area;
mod driver;
mod effect;
mod enums;
mod god;

#[macro_use]
pub mod helpers;
mod lab9;
mod network_manager;
mod path_finding;
mod player;
mod populate;
mod repository;
mod server;
mod skilltab;
mod state;
mod talk;

use log;
use std::env;
use std::process;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use signal_hook::consts::{SIGHUP, SIGINT, SIGQUIT, SIGTERM};
use signal_hook::iterator::Signals;

use core;

use crate::path_finding::PathFinder;
use crate::repository::Repository;
use crate::server::Server;

fn setup_signal_handling(
    quit_flag: Arc<AtomicBool>,
) -> (std::thread::JoinHandle<()>, signal_hook::iterator::Handle) {
    // Set up signal handling in a separate thread
    let mut signals = Signals::new(&[SIGINT, SIGTERM, SIGQUIT, SIGHUP]).unwrap();
    let handle = signals.handle();

    let signal_thread = std::thread::spawn(move || {
        for sig in signals.forever() {
            match sig {
                SIGINT | SIGTERM | SIGQUIT => {
                    if !quit_flag.load(Ordering::SeqCst) {
                        log::info!("Got signal to terminate. Shutdown initiated...");
                    } else {
                        log::info!("Alright, alright, I'm already terminating!");
                    }
                    quit_flag.store(true, Ordering::SeqCst);
                }
                _ => {
                    log::warn!("Received unsupported signal: {}", sig);
                }
            }
        }
    });

    (signal_thread, handle)
}

fn handle_command_line_args(args: &[String]) {
    if args.len() == 2 {
        let cmd = args[1].to_lowercase();
        match cmd.as_str() {
            "pop" => {
                populate::populate();
                process::exit(0);
            }
            "wipe" => {
                populate::pop_wipe();
                process::exit(0);
            }
            "light" => {
                populate::init_lights();
                process::exit(0);
            }
            "skill" => {
                populate::pop_skill();
                process::exit(0);
            }
            "load" => {
                populate::pop_load_all_chars();
                process::exit(0);
            }
            "save" => {
                populate::pop_save_all_chars();
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

    core::initialize_logger(log::LevelFilter::Debug, Some("server.log")).unwrap_or_else(|e| {
        eprintln!("Failed to initialize logger: {}. Exiting.", e);
        process::exit(1);
    });

    log::info!("Starting Men Among Gods: Reforged Server v0.0.1");
    log::info!("Copyright (C) 2024 The Reforged Project. All rights reserved.");
    log::info!("Process PID: {}", process::id());

    let quit_flag = Arc::new(AtomicBool::new(false));
    let quit_flag_clone = quit_flag.clone();
    let (signal_thread, handle) = setup_signal_handling(quit_flag_clone);

    handle_command_line_args(&args);

    // Initialize the global repository
    if let Err(e) = Repository::initialize() {
        log::error!("Failed to initialize repository: {}. Exiting.", e);
        process::exit(1);
    }

    // Initialize the global pathfinder
    if let Err(e) = PathFinder::initialize() {
        log::error!("Failed to initialize pathfinder: {}. Exiting.", e);
        process::exit(1);
    }

    // Check for dirty flag
    Repository::with_globals(|globals| {
        if globals.is_dirty() {
            log::error!("Data files were not closed cleanly last time. Exiting.");
            process::exit(1);
        }
    });

    let mut server = server::Server::new();

    server.initialize()?;

    log::info!("Entering main game loop...");

    while !quit_flag.load(Ordering::SeqCst) {
        server.tick();
    }

    log::info!("Shutdown signal received, exiting main loop...");
    Server::with_players_mut(|players| {
        for n in 1..core::constants::MAXPLAYER {
            player::plr_logout(players[n].usnr, n, enums::LogoutReason::Shutdown);
        }
    });

    // TODO: Wait some amount of time and forceably close all sockets

    // TODO: Equivalent of saving data back to disk here...

    log::info!("Server shutdown complete.");

    handle.close();
    signal_thread.join().unwrap_or_else(|e| {
        log::error!("Failed to join signal handling thread: {:?}", e);
    });

    Ok(())
}
