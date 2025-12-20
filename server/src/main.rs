mod enums;
mod network_manager;
mod path_finding;
mod repository;
mod server;
mod state;

use log;
use std::env;
use std::process;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use signal_hook::consts::{SIGHUP, SIGINT, SIGQUIT, SIGTERM};
use signal_hook::iterator::Signals;

use core;

use crate::repository::Repository;

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
}

fn main() {
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

    // Load game data
    let mut repository = Repository::new();
    if let Err(e) = repository.load() {
        log::error!("Failed to load game data: {}. Exiting.", e);
        process::exit(1);
    }

    handle_command_line_args(&args);

    // Check for dirty flag
    if repository.globals.is_dirty() {
        log::error!("Data files were not closed cleanly last time. Exiting.");
        process::exit(1);
    }

    let mut server = server::Server::new(&mut repository);

    server.initialize();

    log::info!("Entering main game loop...");

    while !quit_flag.load(Ordering::SeqCst) {
        server.tick();
    }

    log::info!("Server shutdown complete.");

    handle.close();
    signal_thread.join().unwrap_or_else(|e| {
        log::error!("Failed to join signal handling thread: {:?}", e);
    });
}
