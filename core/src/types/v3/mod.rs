//! Version 3 game data type definitions.
//!
//! This module preserves the **frozen schema-v2 on-disk layout** for
//! serialised game entities so snapshot migrators can decode legacy
//! `.wsnap` files and convert them to the live (`v2`) struct shapes.
//!
//! - `v3::Character` and `v3::Item` are independent struct definitions whose
//!   field layout must never change (75-slot skill matrices with `u8`/`i8`
//!   attribute and skill values).
//! - `Map`, `Effect`, and `Global` have not changed shape since v1, so they
//!   re-export the live structs verbatim.
//!
//! # Migration pattern
//!
//! See the v2 -> v3 migration in `server::keydb::snapshot::WorldSnapshot::from_file`
//! for the concrete pattern: detect the legacy schema version, decode into the
//! frozen v3 structs, then convert via `From<v3::Foo> for crate::types::Foo`.

pub mod character;
pub mod item;

pub use character::Character;
pub use item::Item;

pub use super::Effect;
pub use super::Global;
pub use super::Map;
