//! Shared support code for host-native server utility viewers.

/// Shared CLI, data-source, and snapshot-loading helpers for viewer binaries.
pub mod viewer_support;

/// Blocking HTTP client for the server admin API (template editing).
pub mod admin_client;

pub use admin_client::AdminClient;
pub use viewer_support::{
    DataSource, data_source_from_args, default_graphics_zip_path, graphics_zip_from_args,
    load_world_snapshot, save_world_snapshot,
};
