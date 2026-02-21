mod area;
mod driver;
mod effect;
mod enums;
mod god;
mod keydb;
mod types;

#[macro_use]
pub mod helpers;
mod lab9;
mod network_manager;
mod path_finding;
mod player;
mod populate;
mod repository;
mod server;
mod single_thread_cell;
mod state;
mod talk;

use log;
use std::env;
use std::process;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use core;

use crate::path_finding::PathFinder;
use crate::repository::Repository;
use crate::server::Server;

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

    log::info!("Server shutdown complete.");

    Ok(())
}
