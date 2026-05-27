//! `AuthUser` extractor: validates the bearer token and resolves account ID.
//!
//! Every protected route used to repeat ~25 lines of boilerplate to pull the
//! token out of headers, verify it, then look up the account ID from the
//! username claim. That boilerplate is replaced by extracting `AuthUser` from
//! the request and reading the fields directly.
use axum::{
    extract::{FromRef, FromRequestParts},
    http::{StatusCode, request::Parts},
};
use log::{error, warn};

use crate::{ApiState, helpers, pipelines};

/// Authenticated user resolved from a valid `Authorization: Bearer <JWT>`.
#[derive(Debug, Clone)]
pub(crate) struct AuthUser {
    /// Numeric account identifier from KeyDB.
    pub account_id: u64,
    /// Lowercase username (matches the JWT `sub` claim and the
    /// `account:username:{lc}` index key).
    pub username_lc: String,
}

impl<S> FromRequestParts<S> for AuthUser
where
    ApiState: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = StatusCode;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let api_state = ApiState::from_ref(state);

        let token = match helpers::get_token_from_headers(&parts.headers) {
            Some(value) => value,
            None => {
                warn!("Unauthorized access attempt: missing Authorization header");
                return Err(StatusCode::UNAUTHORIZED);
            }
        };

        let token_data = match helpers::verify_token(&token, api_state.jwt_secret.as_ref()) {
            Ok(data) => data,
            Err(err) => {
                warn!("Unauthorized access attempt: {}", err);
                return Err(StatusCode::UNAUTHORIZED);
            }
        };

        let username_lc = token_data.claims.sub.trim().to_lowercase();
        let mut con = api_state.con.clone();
        match pipelines::get_account_id_by_username(&mut con, &username_lc).await {
            Ok(Some(account_id)) => Ok(AuthUser {
                account_id,
                username_lc,
            }),
            Ok(None) => {
                warn!(
                    "Unauthorized access attempt: account not found for sub={}",
                    token_data.claims.sub
                );
                Err(StatusCode::UNAUTHORIZED)
            }
            Err(err) => {
                error!("Redis read failed during auth: {}", err);
                Err(StatusCode::INTERNAL_SERVER_ERROR)
            }
        }
    }
}
