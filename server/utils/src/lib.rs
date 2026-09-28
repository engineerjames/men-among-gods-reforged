//! Shared support code for host-native server utility viewers.

use std::sync::Once;

/// Shared CLI, data-source, and snapshot-loading helpers for viewer binaries.
pub mod viewer_support;

/// Blocking HTTP client for the server admin API (template editing).
pub mod admin_client;

pub use admin_client::AdminClient;
pub use viewer_support::{
    DataSource, data_source_from_args, default_graphics_zip_path, graphics_zip_from_args,
    load_world_snapshot, save_world_snapshot,
};

static LOAD_DOTENV_ONCE: Once = Once::new();

/// Loads the project `.env` file into the process environment, once.
///
/// Docker Compose interpolates `.env` for the containerized services, but these
/// utilities run directly on the host and only see variables the shell actually
/// exported. Loading `.env` here means `MAG_ADMIN_API_TOKEN` and friends work
/// the same way for `mag-admin` and the viewers as they do for `docker compose`.
/// Existing environment variables always win, so explicit exports still
/// override the file.
///
/// Must be called before any argument parsing that reads environment fallbacks.
pub fn load_dotenv() {
    LOAD_DOTENV_ONCE.call_once(|| {
        let _ = dotenvy::dotenv();
    });
}
