//! Data types module - contains all game data structures ported from the original C++ headers
//!
//! All five game-entity types (`Character`, `Item`, `Map`, `Effect`, `Global`) are also
//! re-exported through the [`v1`] sub-module so that snapshot and migration code can
//! reference them by version path (e.g. `core::types::v1::Character`).

pub mod v1;

pub mod api;
mod ban;
mod character;
mod client_player;
mod effect;
mod enums;
mod global;
mod item;
mod map;
mod see_map;

// Re-export all types
pub use api::*;
pub use ban::Ban;
pub use character::Character;
pub use client_player::ClientPlayer;
pub use effect::Effect;
pub use enums::*;
pub use global::Global;
pub use item::Item;
pub use map::Map;
pub use see_map::SeeMap;
