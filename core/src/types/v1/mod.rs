//! Version 1 game data type definitions.
//!
//! This module preserves the **frozen v1 on-disk layout** for serialised game
//! entities so snapshot migrators can decode legacy `.wsnap` files and convert
//! them to the live (`v2`) struct shapes.
//!
//! - `v1::Character` and `v1::Item` are independent struct definitions whose
//!   field layout must never change (50-slot skill matrices).
//! - `Map`, `Effect`, and `Global` have not changed shape since v1, so they
//!   re-export the live structs verbatim. If they ever change, freeze them
//!   here the same way `Character` and `Item` are frozen.
//!
//! # Migration pattern
//!
//! See the v1 -> v2 migration in `server::keydb::snapshot::WorldSnapshot::from_file`
//! for the concrete pattern: detect the legacy schema version, decode into the
//! frozen v1 structs, then convert via `From<v1::Foo> for crate::types::Foo`.

pub mod character;
pub mod item;

pub use character::Character;
pub use item::Item;

pub use super::Effect;
pub use super::Global;
pub use super::Map;
