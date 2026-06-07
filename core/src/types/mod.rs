//! Data types module - contains all game data structures ported from the original C++ headers
//!
//! Versioned re-exports of the entity types live under [`v1`] (frozen 50-slot
//! skill layout) and [`v2`] (current 75-slot skill layout). Snapshot and
//! migration code should reference types by version path
//! (e.g. `core::types::v1::Character`, `core::types::v2::Character`).
//!
//! Top-level re-exports always point at the **current** schema version (v2).

pub mod v1;
pub mod v2;

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
