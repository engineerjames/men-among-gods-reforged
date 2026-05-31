mod area;
mod driver;
mod effect;
mod game_state;
mod god;
mod types;

#[cfg(test)]
mod test_helpers;

#[macro_use]
pub mod helpers;
mod lab9;
mod network_manager;
mod path_finding;
mod player;
mod points;
mod populate;
mod server;
mod state;
mod talk;
mod tls;

use core::logout_reasons::LogoutReason;
use std::env;
use std::process;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::game_state::GameState;

fn main() -> Result<(), String> {
    let _: Vec<String> = env::args().collect();

    core::initialize_logger(log::LevelFilter::Info, Some("server.log")).unwrap_or_else(|e| {
        eprintln!("Failed to initialize logger: {}. Exiting.", e);
        process::exit(1);
    });

    log::info!(
        "Starting Men Among Gods: Reforged Server v{}",
        env!("CARGO_PKG_VERSION")
    );
    log::info!("Process PID: {}", process::id());

    let quit_flag = Arc::new(AtomicBool::new(false));
    let quit_flag_clone = quit_flag.clone();

    let handler_result = ctrlc::set_handler(move || {
        if !quit_flag_clone.load(Ordering::SeqCst) {
            log::info!("Shutdown signal received. Shutdown initiated...");
        } else {
            log::info!("Shutdown already in progress.");
        }
        quit_flag_clone.store(true, Ordering::SeqCst);
    });

    if let Err(e) = handler_result {
        log::error!("Error setting Ctrl-C handler: {}. Exiting.", e);
        process::exit(1);
    }

    let god_password = match env::var("MAG_GOD_PASSWORD") {
        Ok(v) if !v.is_empty() => v,
        _ => {
            log::error!("Environment variable MAG_GOD_PASSWORD is not set or is empty. Exiting.");
            process::exit(1);
        }
    };

    let mut gs = GameState::initialize().unwrap_or_else(|e| {
        log::error!("Failed to initialize game state: {}. Exiting.", e);
        process::exit(1);
    });

    if env::var("MAG_PLAYTEST")
        .map(|v| !v.is_empty())
        .unwrap_or(false)
    {
        gs.playtest_mode = true;
        log::info!("Playtest mode enabled (MAG_PLAYTEST is set).");
    }

    gs.god_password = god_password;
    log::info!("God password loaded from MAG_GOD_PASSWORD.");

    if gs.globals.is_dirty() {
        log::error!("Data files were not closed cleanly last time. Exiting.");
        process::exit(1);
    }

    let mut server = server::Server::new();

    server.initialize(&mut gs).unwrap_or_else(|e| {
        log::error!("Failed to initialize server: {}. Exiting.", e);
        process::exit(1);
    });

    log::info!("Entering main game loop...");

    while !quit_flag.load(Ordering::SeqCst) {
        server.drain_template_reloads(&mut gs);
        server.drain_text_reloads(&mut gs);
        server.drain_map_patches(&mut gs);
        server.drain_item_patches(&mut gs);
        server.drain_character_patches(&mut gs);
        server.drain_ban_actions(&mut gs);
        server.drain_world_actions(&mut gs);
        server.tick(&mut gs);
    }

    log::info!("Shutdown signal received, exiting main loop...");
    let mut logout_entries: Vec<(usize, usize)> = Vec::new();
    for player_idx in 1..gs.players.len() {
        logout_entries.push((gs.players[player_idx].usnr, player_idx));
    }
    for (usnr, n) in &logout_entries {
        player::connection::plr_logout(&mut gs, *usnr, *n, LogoutReason::Shutdown);
    }

    log::info!("Enqueueing full save of all game data before shutdown...");
    server.enqueue_full_save(&gs);

    server.shutdown_background_saver();

    gs.shutdown();

    log::info!("Server shutdown complete.");

    Ok(())
}
