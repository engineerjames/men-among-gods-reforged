use std::env;
use std::time::Duration;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use crate::helpers;
use crate::pipelines;
use crate::types;

use axum::{extract::Path, extract::State, http::StatusCode, Json};
use jsonwebtoken::EncodingKey;
use jsonwebtoken::Header;
use log::{error, info, warn};
use redis::AsyncCommands;

/// Creates a new character for the authenticated account.
/// Validates the JWT from the `Authorization` header, validates the request payload, and then
/// writes the character data to KeyDB.
///
/// # Arguments
/// * `con` - Multiplexed KeyDB connection provided by Axum state.
/// * `headers` - Request headers used to extract the `Authorization: Bearer <JWT>` token.
/// * `payload` - Character creation fields (name/description/sex/race).
///
/// # Returns
/// * `(StatusCode::OK, CharacterSummary)` on success.
/// * `(StatusCode::UNAUTHORIZED, default)` when the token is missing/invalid or the account is not found.
/// * `(StatusCode::BAD_REQUEST, default)` when the request payload is invalid.
/// * `(StatusCode::INTERNAL_SERVER_ERROR, default)` on KeyDB or internal failures.
pub(crate) async fn create_new_character(
    State(mut con): State<redis::aio::MultiplexedConnection>,
    headers: axum::http::HeaderMap,
    Json(payload): Json<types::CreateCharacterRequest>,
) -> (StatusCode, Json<types::CharacterSummary>) {
    let token = match helpers::get_token_from_headers(&headers).await {
        Some(value) => value,
        None => {
            warn!("Unauthorized access attempt: missing Authorization header");
            return (
                StatusCode::UNAUTHORIZED,
                Json(types::CharacterSummary::default()),
            );
        }
    };

    let token_data = match helpers::verify_token(&token).await {
        Ok(token_data) => token_data,
        Err(err) => {
            warn!("Unauthorized access attempt: {}", err);
            return (
                StatusCode::UNAUTHORIZED,
                Json(types::CharacterSummary::default()),
            );
        }
    };

    if !payload.validate() {
        return (
            StatusCode::BAD_REQUEST,
            Json(types::CharacterSummary::default()),
        );
    }

    let username_lc = token_data.claims.sub.trim().to_lowercase();
    let user_id = match pipelines::find_account_id_by_username_scan(&mut con, &username_lc).await {
        Ok(Some(value)) => value,
        Ok(None) => {
            warn!(
                "Create character rejected: account not found for {}",
                token_data.claims.sub
            );
            return (
                StatusCode::UNAUTHORIZED,
                Json(types::CharacterSummary::default()),
            );
        }
        Err(err) => {
            error!("Redis read failed: {}", err);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(types::CharacterSummary::default()),
            );
        }
    };

    let result = pipelines::insert_new_character(
        &mut con,
        user_id,
        &payload.name,
        payload.description.as_deref(),
        payload.sex,
        payload.race,
    )
    .await;

    match result {
        Ok(character_id) => {
            info!(
                "Character created for account {}: id={}, name={}, sex={:?}, race={:?}",
                token_data.claims.sub, character_id, payload.name, payload.sex, payload.race
            );
            (
                StatusCode::OK,
                Json(types::CharacterSummary {
                    id: character_id,
                    name: payload.name,
                    description: payload.description.unwrap_or_default(),
                    sex: payload.sex,
                    race: payload.race,
                    server_id: None,
                }),
            )
        }
        Err(err) => {
            error!("Failed to create character: {}", err);

            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(types::CharacterSummary::default()),
            )
        }
    }
}

/// Fetches all characters for the authenticated account.
/// Validates the JWT from the `Authorization` header, resolves the account ID via the username
/// index key, then loads character hashes from KeyDB.
///
/// # Arguments
/// * `con` - Multiplexed KeyDB connection provided by Axum state.
/// * `headers` - Request headers used to extract the `Authorization: Bearer <JWT>` token.
///
/// # Returns
/// * `(StatusCode::OK, GetCharactersResponse)` with zero or more characters.
/// * `(StatusCode::UNAUTHORIZED, empty)` when the token is missing/invalid or the account is not found.
/// * `(StatusCode::INTERNAL_SERVER_ERROR, empty)` on KeyDB or internal failures.
pub(crate) async fn get_characters(
    State(mut con): State<redis::aio::MultiplexedConnection>,
    headers: axum::http::HeaderMap,
) -> (StatusCode, Json<types::GetCharactersResponse>) {
    let token = match helpers::get_token_from_headers(&headers).await {
        Some(value) => value,
        None => {
            warn!("Unauthorized access attempt: missing Authorization header");
            return (
                StatusCode::UNAUTHORIZED,
                Json(types::GetCharactersResponse { characters: vec![] }),
            );
        }
    };

    let token_data = match helpers::verify_token(&token).await {
        Ok(token_data) => token_data,
        Err(err) => {
            warn!("Unauthorized access attempt: {}", err);
            return (
                StatusCode::UNAUTHORIZED,
                Json(types::GetCharactersResponse { characters: vec![] }),
            );
        }
    };

    let username_lc = token_data.claims.sub.trim().to_lowercase();
    let user_id = match pipelines::find_account_id_by_username_scan(&mut con, &username_lc).await {
        Ok(Some(value)) => value,
        Ok(None) => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(types::GetCharactersResponse { characters: vec![] }),
            );
        }
        Err(err) => {
            error!("Redis read failed: {}", err);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(types::GetCharactersResponse { characters: vec![] }),
            );
        }
    };

    let characters = match pipelines::list_characters_for_account_scan(&mut con, user_id).await {
        Ok(values) => values,
        Err(err) => {
            error!("Redis read failed: {}", err);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(types::GetCharactersResponse { characters: vec![] }),
            );
        }
    };

    (
        StatusCode::OK,
        Json(types::GetCharactersResponse { characters }),
    )
}

/// Creates a new account and registers username/email index keys.
/// Validates email/username/password formats, enforces uniqueness by scanning KeyDB account hashes,
/// and writes the account hash.
///
/// # Arguments
/// * `con` - Multiplexed KeyDB connection provided by Axum state.
/// * `payload` - Account creation fields (email/username/password).
///
/// # Returns
/// * `(StatusCode::CREATED, CreateAccountResponse)` with `id` set on success.
/// * `(StatusCode::BAD_REQUEST, CreateAccountResponse)` when validation fails.
/// * `(StatusCode::CONFLICT, CreateAccountResponse)` when email or username already exists.
/// * `(StatusCode::INTERNAL_SERVER_ERROR, CreateAccountResponse)` on KeyDB or internal failures.
pub(crate) async fn create_account(
    State(mut con): State<redis::aio::MultiplexedConnection>,
    Json(payload): Json<types::CreateAccountRequest>,
) -> (StatusCode, Json<types::CreateAccountResponse>) {
    let email_lc = payload.email.trim().to_lowercase();
    let username_lc = payload.username.trim().to_lowercase();

    info!(
        "Create account request: username={}, email={}",
        username_lc, email_lc
    );
    let response = types::CreateAccountResponse {
        id: None,
        error: None,
        username: username_lc.clone(),
        email: email_lc.clone(),
    };

    if !helpers::is_valid_email_regex(&email_lc) {
        warn!("Create account rejected: invalid email {}", email_lc);
        return (
            StatusCode::BAD_REQUEST,
            Json(types::CreateAccountResponse {
                error: Some("Invalid email".to_string()),
                ..response
            }),
        );
    }

    if !helpers::is_valid_username(&username_lc) {
        warn!("Create account rejected: invalid username {}", username_lc);
        return (
            StatusCode::BAD_REQUEST,
            Json(types::CreateAccountResponse {
                error: Some("Invalid username".to_string()),
                ..response
            }),
        );
    }

    if !helpers::is_valid_password(&payload.password) {
        warn!("Create account rejected: invalid password format");
        return (
            StatusCode::BAD_REQUEST,
            Json(types::CreateAccountResponse {
                error: Some("Invalid password".to_string()),
                ..response
            }),
        );
    }

    // Ensure uniqueness under concurrency: serialize account creation with a short-lived lock.
    // This avoids Redis transactions/WATCH while still preventing scan-then-write races.
    const ACCOUNT_CREATE_LOCK_KEY: &str = "lock:account:create";
    const ACCOUNT_CREATE_LOCK_TTL_MS: u64 = 5_000;
    let lock_token = match pipelines::acquire_lock_with_retry(
        &mut con,
        ACCOUNT_CREATE_LOCK_KEY,
        ACCOUNT_CREATE_LOCK_TTL_MS,
        10,
        50,
    )
    .await
    {
        Ok(Some(token)) => token,
        Ok(None) => {
            warn!("Create account rejected: lock contention");
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(types::CreateAccountResponse {
                    error: Some("Server busy, please retry".to_string()),
                    ..response
                }),
            );
        }
        Err(err) => {
            error!("Redis lock acquire failed: {}", err);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(types::CreateAccountResponse {
                    error: Some(format!("Redis error: {}", err)),
                    ..response
                }),
            );
        }
    };

    let id_key = "account:next_id";
    let duplicate_check =
        match pipelines::check_account_duplicates_scan(&mut con, &email_lc, &username_lc).await {
            Ok(value) => value,
            Err(err) => {
                let _ =
                    pipelines::release_lock(&mut con, ACCOUNT_CREATE_LOCK_KEY, &lock_token).await;
                error!("Redis read failed: {}", err);
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(types::CreateAccountResponse {
                        error: Some(format!("Redis error: {}", err)),
                        ..response
                    }),
                );
            }
        };

    match duplicate_check {
        pipelines::DuplicateCheckResult::Email => {
            let _ = pipelines::release_lock(&mut con, ACCOUNT_CREATE_LOCK_KEY, &lock_token).await;
            info!("Create account rejected: duplicate email {}", email_lc);
            return (
                StatusCode::CONFLICT,
                Json(types::CreateAccountResponse {
                    error: Some("Email is already in use".to_string()),
                    ..response
                }),
            );
        }
        pipelines::DuplicateCheckResult::Username => {
            let _ = pipelines::release_lock(&mut con, ACCOUNT_CREATE_LOCK_KEY, &lock_token).await;
            info!(
                "Create account rejected: duplicate username {}",
                username_lc
            );
            return (
                StatusCode::CONFLICT,
                Json(types::CreateAccountResponse {
                    error: Some("Username is already in use".to_string()),
                    ..response
                }),
            );
        }
        pipelines::DuplicateCheckResult::None => {}
    }

    let id: u64 = match con.incr(id_key, 1).await {
        Ok(value) => value,
        Err(err) => {
            let _ = pipelines::release_lock(&mut con, ACCOUNT_CREATE_LOCK_KEY, &lock_token).await;
            error!("Redis INCR failed: {}", err);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(types::CreateAccountResponse {
                    error: Some(format!("Redis error: {}", err)),
                    ..response
                }),
            );
        }
    };
    info!("Allocated account id {}", id);

    if let Err(err) =
        pipelines::insert_account_hash(&mut con, id, &email_lc, &username_lc, &payload.password)
            .await
    {
        let _ = pipelines::release_lock(&mut con, ACCOUNT_CREATE_LOCK_KEY, &lock_token).await;
        error!("Redis write failed: {}", err);
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(types::CreateAccountResponse {
                error: Some(format!("Redis error: {}", err)),
                ..response
            }),
        );
    }

    let _ = pipelines::release_lock(&mut con, ACCOUNT_CREATE_LOCK_KEY, &lock_token).await;

    (
        StatusCode::CREATED,
        Json(types::CreateAccountResponse {
            id: Some(id),
            error: None,
            username: username_lc,
            email: email_lc,
        }),
    )
}

/// Updates an existing character owned by the authenticated account.
/// Validates the JWT, checks character ownership via the account's character set, and updates
/// the provided fields on the character hash.
///
/// # Arguments
/// * `con` - Multiplexed KeyDB connection provided by Axum state.
/// * `headers` - Request headers used to extract the `Authorization: Bearer <JWT>` token.
/// * `character_id` - Character ID extracted from the URL path (`/characters/{id}`).
/// * `payload` - Fields to update (`name` and/or `description`).
///
/// # Returns
/// * `StatusCode::OK` on success.
/// * `StatusCode::BAD_REQUEST` when no updatable fields are provided.
/// * `StatusCode::UNAUTHORIZED` when the token is missing/invalid, the account is missing, or the character is not owned.
/// * `StatusCode::INTERNAL_SERVER_ERROR` on KeyDB or internal failures.
pub(crate) async fn update_character(
    State(mut con): State<redis::aio::MultiplexedConnection>,
    headers: axum::http::HeaderMap,
    Path(character_id): Path<u64>,
    Json(payload): Json<types::UpdateCharacterRequest>,
) -> StatusCode {
    let token = match helpers::get_token_from_headers(&headers).await {
        Some(value) => value,
        None => {
            warn!("Unauthorized access attempt: missing Authorization header");
            return StatusCode::UNAUTHORIZED;
        }
    };

    let token_data = match helpers::verify_token(&token).await {
        Ok(token_data) => token_data,
        Err(err) => {
            warn!("Unauthorized access attempt: {}", err);
            return StatusCode::UNAUTHORIZED;
        }
    };

    let username_lc = token_data.claims.sub.trim().to_lowercase();
    let account_id = match pipelines::find_account_id_by_username_scan(&mut con, &username_lc).await
    {
        Ok(Some(id)) => id,
        Ok(None) => {
            warn!(
                "Unauthorized update attempt: account not found for {}",
                token_data.claims.sub
            );
            return StatusCode::UNAUTHORIZED;
        }
        Err(err) => {
            error!("Redis read failed: {}", err);
            return StatusCode::INTERNAL_SERVER_ERROR;
        }
    };

    let character_owner = match pipelines::get_character_account_id(&mut con, character_id).await {
        Ok(value) => value,
        Err(err) => {
            error!("Redis read failed: {}", err);
            return StatusCode::INTERNAL_SERVER_ERROR;
        }
    };
    if character_owner != Some(account_id) {
        warn!(
            "Unauthorized update attempt: character {} does not belong to user {}",
            character_id, token_data.claims.sub
        );
        return StatusCode::UNAUTHORIZED;
    }

    if payload.name.is_none() && payload.description.is_none() {
        warn!(
            "Update character rejected: no fields to update for character {}",
            character_id
        );
        return StatusCode::BAD_REQUEST;
    }

    match pipelines::update_character(
        &mut con,
        character_id,
        payload.name.as_deref(),
        payload.description.as_deref(),
    )
    .await
    {
        Ok(_) => {
            info!(
                "Character {} updated for account {}",
                character_id, token_data.claims.sub
            );
            StatusCode::OK
        }
        Err(err) => {
            error!("Failed to update character: {}", err);
            StatusCode::INTERNAL_SERVER_ERROR
        }
    }
}

/// Deletes an existing character owned by the authenticated account.
/// Validates the JWT, checks character ownership via the account's character set, then deletes
/// the character hash and removes it from the ownership set.
///
/// # Arguments
/// * `con` - Multiplexed KeyDB connection provided by Axum state.
/// * `headers` - Request headers used to extract the `Authorization: Bearer <JWT>` token.
/// * `character_id` - Character ID extracted from the URL path (`/characters/{id}`).
///
/// # Returns
/// * `StatusCode::OK` on success.
/// * `StatusCode::UNAUTHORIZED` when the token is missing/invalid, the account is missing, or the character is not owned.
/// * `StatusCode::INTERNAL_SERVER_ERROR` on KeyDB or internal failures.
pub(crate) async fn delete_character(
    State(mut con): State<redis::aio::MultiplexedConnection>,
    headers: axum::http::HeaderMap,
    Path(character_id): Path<u64>,
) -> StatusCode {
    let token = match helpers::get_token_from_headers(&headers).await {
        Some(value) => value,
        None => {
            warn!("Unauthorized access attempt: missing Authorization header");
            return StatusCode::UNAUTHORIZED;
        }
    };

    let token_data = match helpers::verify_token(&token).await {
        Ok(token_data) => token_data,
        Err(err) => {
            warn!("Unauthorized access attempt: {}", err);
            return StatusCode::UNAUTHORIZED;
        }
    };

    let username_lc = token_data.claims.sub.trim().to_lowercase();
    let account_id = match pipelines::find_account_id_by_username_scan(&mut con, &username_lc).await
    {
        Ok(Some(id)) => id,
        Ok(None) => {
            warn!(
                "Unauthorized delete attempt: account not found for {}",
                token_data.claims.sub
            );
            return StatusCode::UNAUTHORIZED;
        }
        Err(err) => {
            error!("Redis read failed: {}", err);
            return StatusCode::INTERNAL_SERVER_ERROR;
        }
    };

    let character_owner = match pipelines::get_character_account_id(&mut con, character_id).await {
        Ok(value) => value,
        Err(err) => {
            error!("Redis read failed: {}", err);
            return StatusCode::INTERNAL_SERVER_ERROR;
        }
    };
    if character_owner != Some(account_id) {
        warn!(
            "Unauthorized delete attempt: character {} does not belong to user {}",
            character_id, token_data.claims.sub
        );
        return StatusCode::UNAUTHORIZED;
    }

    match pipelines::delete_character(&mut con, character_id).await {
        Ok(_) => {
            info!(
                "Character {} deleted for account {}",
                character_id, token_data.claims.sub
            );
            StatusCode::OK
        }
        Err(err) => {
            error!("Failed to delete character: {}", err);
            StatusCode::INTERNAL_SERVER_ERROR
        }
    }
}

/// Authenticates a user and returns a signed JWT.
/// Resolves the account via the username index, compares the stored password/hash, and creates
/// a JWT signed with `API_JWT_SECRET` that expires in ~1 hour.
///
/// # Arguments
/// * `con` - Multiplexed KeyDB connection provided by Axum state.
/// * `payload` - Login credentials (username/password).
///
/// # Returns
/// * `(StatusCode::OK, LoginResponse)` containing a JWT token on success.
/// * `(StatusCode::BAD_REQUEST, LoginResponse)` when the password format is invalid.
/// * `(StatusCode::UNAUTHORIZED, LoginResponse)` when the username is unknown or the password does not match.
/// * `(StatusCode::INTERNAL_SERVER_ERROR, LoginResponse)` when KeyDB fails or `API_JWT_SECRET` is missing.
pub(crate) async fn login(
    State(mut con): State<redis::aio::MultiplexedConnection>,
    Json(payload): Json<types::LoginRequest>,
) -> (StatusCode, Json<types::LoginResponse>) {
    let username_lc = payload.username.trim().to_lowercase();
    info!("Login request for username={}", username_lc);
    if !helpers::is_valid_password(&payload.password) {
        warn!("Login rejected: invalid password format");
        return (
            StatusCode::BAD_REQUEST,
            Json(types::LoginResponse {
                token: String::new(),
            }),
        );
    }

    let user_id = match pipelines::find_account_id_by_username_scan(&mut con, &username_lc).await {
        Ok(Some(value)) => value,
        Ok(None) => {
            warn!("Login rejected: username not found {}", username_lc);
            return (
                StatusCode::UNAUTHORIZED,
                Json(types::LoginResponse {
                    token: String::new(),
                }),
            );
        }
        Err(err) => {
            error!("Redis read failed: {}", err);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(types::LoginResponse {
                    token: String::new(),
                }),
            );
        }
    };

    let stored_hash: Option<String> =
        match pipelines::get_account_password_hash(&mut con, user_id).await {
            Ok(value) => value,
            Err(err) => {
                error!("Redis read failed: {}", err);
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(types::LoginResponse {
                        token: String::new(),
                    }),
                );
            }
        };

    let stored_hash = match stored_hash {
        Some(value) => value,
        None => {
            warn!(
                "Login rejected: missing password hash for {}",
                payload.username
            );
            return (
                StatusCode::UNAUTHORIZED,
                Json(types::LoginResponse {
                    token: String::new(),
                }),
            );
        }
    };

    if stored_hash != payload.password {
        warn!("Login rejected: password mismatch for {}", username_lc);
        return (
            StatusCode::UNAUTHORIZED,
            Json(types::LoginResponse {
                token: String::new(),
            }),
        );
    }

    let secret = match env::var("API_JWT_SECRET") {
        Ok(value) if !value.trim().is_empty() => value,
        _ => {
            error!("Login failed: API_JWT_SECRET is not set");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(types::LoginResponse {
                    token: String::new(),
                }),
            );
        }
    };

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::from_secs(0))
        .as_secs();
    let claims = types::JwtClaims {
        sub: username_lc,
        exp: (now + 3600) as usize,
    };

    let token = match jsonwebtoken::encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    ) {
        Ok(value) => value,
        Err(err) => {
            error!("JWT encode failed: {}", err);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(types::LoginResponse {
                    token: String::new(),
                }),
            );
        }
    };

    (StatusCode::OK, Json(types::LoginResponse { token }))
}
