//! Shared library crate for the Men Among Gods client.
//!
//! Re-exports all modules so that both the main client binary and auxiliary

pub mod account_api;
pub mod cert_trust;
pub mod constants;
pub mod dpi_scaling;
pub mod filepaths;
pub mod font_cache;
pub mod game_map;
pub mod gfx_cache;
pub mod hosts;
pub mod legacy_engine;
pub mod network;
pub mod platform;
pub mod player_state;
pub mod preferences;
pub mod scenes;
pub mod sfx_cache;
pub mod state;
pub mod types;
pub mod ui;
