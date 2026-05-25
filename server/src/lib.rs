//! Server library crate — exposes modules needed by downstream utilities.
//!
//! The `server` crate is primarily a binary (the game server), but this
//! `lib.rs` exposes a small set of modules so that the `server-utils` crate
//! (template viewer, map viewer) can reuse KeyDB connectivity and the
//! points-calculation logic without duplicating code.

/// KeyDB integration: connection helper, persistence layer, snapshot I/O,
/// background saver, and pub/sub patch watchers.
///
/// See [`keydb`] for the full submodule layout. Common entry points:
/// [`keydb::connection::connect`], [`keydb::store::load_all`],
/// [`keydb::snapshot::WorldSnapshot`].
pub mod keydb;

/// Pure functions for calculating character experience points.
///
/// Provides [`points::calculate_points_tot`] for computing the total
/// experience a character template is worth, based on its attributes, HP,
/// endurance, mana, and skills.
pub mod points;
