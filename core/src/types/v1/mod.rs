//! Version 1 game data type definitions.
//!
//! This module is the canonical snapshot of the v1 on-disk layout for all
//! serialised game entities.  The actual struct bodies live in the sibling
//! modules one level up (`core::types::*`) and are re-exported here so that:
//!
//! - Existing call sites (`core::types::Character`, etc.) continue to work
//!   without any changes.
//! - Snapshot / migration code can reference types by version path
//!   (`core::types::v1::Character`) to make schema version intent explicit.
//!
//! # Future migration pattern
//!
//! When a struct needs to evolve to v2:
//!
//! 1. Create `core/src/types/v2/mod.rs` with the updated struct definition.
//! 2. Implement `From<v1::Foo> for v2::Foo` for every changed type.
//! 3. Change the top-level re-export in `core/src/types/mod.rs` to point at
//!    `v2::Foo` instead of `v1::Foo`.
//! 4. Bump `SNAPSHOT_SCHEMA_VERSION` in `server/src/snapshot.rs`.
//! 5. Add a migration arm to `WorldSnapshot::from_file` that converts v1
//!    data to v2 types before returning.

pub use super::Character;
pub use super::Effect;
pub use super::Global;
pub use super::Item;
pub use super::Map;
