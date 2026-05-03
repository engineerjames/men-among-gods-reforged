//! KeyDB integration for the server.
//!
//! Consolidates everything that talks to the KeyDB/Redis instance backing
//! the game world:
//!
//! * [`connection`] — synchronous client connection helper.
//! * [`store`] — load/save functions for every persisted entity type.
//! * [`snapshot`] — portable, versioned `.wsnap` world-snapshot format.
//! * [`background_saver`] — rotating saver thread that flushes dirty
//!   game data back to KeyDB on the ~12 minute schedule documented in
//!   `docs/server/DESIGN.md`.
//! * [`template_reload`], [`text_reload`], [`map_patch`], [`item_patch`],
//!   [`character_patch`] — pub/sub watchers that ingest live patches
//!   published to KeyDB by the admin tooling.

/// Synchronous KeyDB/Redis connection helper.
pub mod connection;

/// Load and save functions for every persisted game-data entity type.
pub mod store;

/// Portable, versioned world-snapshot format (`.wsnap`).
pub mod snapshot;

/// Background saver thread that flushes dirty data to KeyDB on a rotating
/// schedule for crash resilience.
pub mod background_saver;

/// KeyDB pub/sub watcher for character-template hot reloads.
pub mod character_patch;

/// KeyDB pub/sub watcher for item-template hot reloads.
pub mod item_patch;

/// KeyDB pub/sub watcher for static-map hot patches.
pub mod map_patch;

/// KeyDB pub/sub watcher for template (item + character) reload requests.
pub mod template_reload;

/// KeyDB watcher for externally managed text-data reload requests.
pub mod text_reload;

/// KeyDB watcher for admin-issued world actions.
pub mod world_action;
