/// Server library crate — exposes modules needed by downstream utilities.
///
/// The `server` crate is primarily a binary (the game server), but this
/// `lib.rs` exposes a small set of modules so that the `server-utils` crate
/// (template viewer, map viewer) can reuse KeyDB connectivity and the
/// points-calculation logic without duplicating code.

/// KeyDB/Redis connection helpers.
///
/// Provides [`keydb::connect`] for establishing a synchronous Redis connection
/// to the game-data KeyDB instance.
pub mod keydb;

/// KeyDB-backed persistence layer for game data.
///
/// Contains [`keydb_store::load_all`], individual entity loaders, and
/// save functions for all game data types (map, items, characters, effects,
/// globals, templates, text data).
pub mod keydb_store;

/// Pure functions for calculating character experience points.
///
/// Provides [`points::calculate_points_tot`] for computing the total
/// experience a character template is worth, based on its attributes, HP,
/// endurance, mana, and skills.
pub mod points;
