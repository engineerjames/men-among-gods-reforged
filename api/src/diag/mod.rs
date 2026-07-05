//! Diagnostics route group.
//!
//! Endpoints under this module are intended for client diagnostics and support
//! tooling uploads. Routes are mounted at `/diag/*` by `main.rs`.

pub mod routes;

use crate::ApiState;
use axum::Router;
use axum::extract::DefaultBodyLimit;
use axum::routing::post;

/// Builds the diagnostics sub-router mounted under `/diag`.
///
/// # Returns
///
/// * A router containing diagnostics endpoints.
pub fn router() -> Router<ApiState> {
    Router::new()
        .route("/client-log", post(routes::upload_client_log))
        .route("/network-test/probe", post(routes::network_test_probe))
        .route("/network-test/summary", post(routes::network_test_summary))
        .layer(DefaultBodyLimit::max(
            routes::MAX_COMPRESSED_LOG_B64_LEN + 1024,
        ))
}
