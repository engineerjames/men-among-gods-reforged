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
pub mod routes_badwords;
pub mod routes_bans;
pub mod routes_characters;
pub mod routes_globals;
pub mod routes_items;
pub mod routes_map;
pub mod routes_templates;
pub mod routes_world_actions;
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
            get(routes_templates::list_item_templates)
                .put(routes_templates::put_item_templates_bulk_unsupported),
        )
        .route(
            "/templates/items/{id}",
            get(routes_templates::get_item_template).put(routes_templates::put_item_template),
        )
        .route(
            "/templates/characters",
            get(routes_templates::list_character_templates),
        )
        .route(
            "/templates/characters/{id}",
            get(routes_templates::get_character_template)
                .put(routes_templates::put_character_template),
        )
        .route("/templates/reload", post(routes_templates::request_reload))
        .route(
            "/templates/reload/status",
            get(routes_templates::get_reload_status),
        )
        .route("/world/map", get(routes_map::get_map_bulk))
        .route("/world/globals", get(routes_globals::get_globals))
        .route("/world/map/version", get(routes_map::get_map_version))
        .route("/world/map/reload", post(routes_map::request_map_reload))
        .route(
            "/world/map/reload/status",
            get(routes_map::get_map_reload_status),
        )
        .route(
            "/world/map/{x}/{y}",
            get(routes_map::get_map_tile).put(routes_map::put_map_tile),
        )
        .route("/world/items", get(routes_items::get_items_bulk))
        .route("/world/items/list", get(routes_items::list_items))
        .route("/world/items/version", get(routes_items::get_items_version))
        .route(
            "/world/items/reload",
            post(routes_items::request_items_reload),
        )
        .route(
            "/world/items/reload/status",
            get(routes_items::get_items_reload_status),
        )
        .route(
            "/world/items/{id}",
            get(routes_items::get_item).put(routes_items::put_item),
        )
        .route(
            "/world/characters",
            get(routes_characters::get_characters_bulk),
        )
        .route(
            "/world/characters/list",
            get(routes_characters::list_characters),
        )
        .route(
            "/world/characters/version",
            get(routes_characters::get_characters_version),
        )
        .route(
            "/world/characters/reload",
            post(routes_characters::request_characters_reload),
        )
        .route(
            "/world/characters/reload/status",
            get(routes_characters::get_characters_reload_status),
        )
        .route(
            "/world/characters/{id}",
            get(routes_characters::get_character).put(routes_characters::put_character),
        )
        .route(
            "/world/actions",
            post(routes_world_actions::request_world_action),
        )
        .route(
            "/world/actions/status",
            get(routes_world_actions::get_world_action_status),
        )
        .route(
            "/bans",
            get(routes_bans::list_bans).post(routes_bans::create_ban),
        )
        .route(
            "/bans/characters/search",
            get(routes_bans::search_characters),
        )
        .route(
            "/bans/account/{account_id}",
            get(routes_bans::get_account_ban).delete(routes_bans::delete_account_ban),
        )
        .route(
            "/bans/character/{character_id}",
            get(routes_bans::get_character_ban).delete(routes_bans::delete_character_ban),
        )
        .route(
            "/bans/ip/{address}",
            get(routes_bans::get_ipv4_ban).delete(routes_bans::delete_ipv4_ban),
        )
        .route(
            "/bans/actions/status",
            get(routes_bans::get_ban_action_status),
        )
        .route(
            "/text/badwords",
            get(routes_badwords::get_badwords)
                .post(routes_badwords::add_badwords)
                .put(routes_badwords::replace_badwords)
                .delete(routes_badwords::remove_badwords),
        )
        .route(
            "/text/badwords/entry",
            get(routes_badwords::get_badword_entry),
        )
        .route("/text/reload", post(routes_badwords::request_text_reload))
        .route(
            "/text/reload/status",
            get(routes_badwords::get_text_reload_status),
        )
        .layer(axum::middleware::from_fn_with_state(
            admin_state.clone(),
            auth::admin_guard,
        ))
        .with_state(state);

    Some(router)
}
