//! Shared library crate for the Men Among Gods client.
//!
//! Re-exports all modules so that both the main client binary and auxiliary

#[allow(dead_code)]
pub mod account_api;
#[allow(dead_code)]
pub mod cert_trust;
pub mod constants;
#[allow(dead_code)]
pub mod dpi_scaling;
pub mod filepaths;
pub mod font_cache;
#[allow(dead_code)]
pub mod game_map;
pub mod gfx_cache;
#[allow(dead_code)]
pub mod hosts;
#[allow(dead_code)]
pub mod legacy_engine;
#[allow(dead_code)]
pub mod network;
#[allow(dead_code)]
pub mod platform;
#[allow(dead_code)]
pub mod player_state;
pub mod preferences;
#[allow(dead_code)]
pub mod scenes;
#[allow(dead_code)]
pub mod sfx_cache;
#[allow(dead_code)]
pub mod state;
pub mod types;
pub mod ui;
