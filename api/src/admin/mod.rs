//! Admin-only API surface.
//!
//! Routes mounted under `/admin/*` are protected by a static bearer token
//! injected via the `MAG_ADMIN_API_TOKEN` environment variable. They are not
//! subject to the public 1 req/s global rate limiter; instead they share a
//! dedicated per-IP bucket plus a per-IP failed-auth lockout, both
//! implemented in [`auth`].
//!
//! Phase 1 scope: item and character templates only. See
//! [`/memories/session/plan.md`] for full design.

pub mod auth;
pub mod routes;
pub mod types;

use crate::ApiState;
use axum::Router;
use axum::routing::{get, post};

/// Build the `/admin` sub-router.
///
/// Returns `None` when admin endpoints are disabled (i.e.
/// `MAG_ADMIN_API_TOKEN` is unset or invalid). The caller should mount the
/// returned router via `Router::nest("/admin", admin_router)` only when this
/// returns `Some`.
///
/// # Arguments
///
/// * `state` - Shared API state passed through to handlers.
///
/// # Returns
///
/// * `Some(Router)` when admin endpoints should be served.
/// * `None` when no token is configured; callers must not mount admin routes.
pub fn build_admin_router(state: ApiState) -> Option<Router> {
    let admin_state = auth::AdminState::from_env()?;

    let router = Router::new()
        .route(
            "/templates/items",
            get(routes::list_item_templates).put(routes::put_item_templates_bulk_unsupported),
        )
        .route(
            "/templates/items/{id}",
            get(routes::get_item_template).put(routes::put_item_template),
        )
        .route(
            "/templates/characters",
            get(routes::list_character_templates),
        )
        .route(
            "/templates/characters/{id}",
            get(routes::get_character_template).put(routes::put_character_template),
        )
        .route("/templates/reload", post(routes::request_reload))
        .route("/templates/reload/status", get(routes::get_reload_status))
        .layer(axum::middleware::from_fn_with_state(
            admin_state.clone(),
            auth::admin_guard,
        ))
        .with_state(state);

    Some(router)
}
