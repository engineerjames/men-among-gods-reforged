use std::net::SocketAddr;
use std::time::Duration;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use crate::ApiState;
use crate::admin::routes_bans;
use crate::auth_extractor::AuthUser;
use crate::helpers;
use crate::password;
use crate::pipelines;
use crate::rate_limit;

use axum::response::IntoResponse;
use axum::{Json, extract::ConnectInfo, extract::Path, extract::State, http::StatusCode};
use jsonwebtoken::EncodingKey;
use jsonwebtoken::Header;
use log::{error, info, warn};
use mag_core::types::CharacterSummary;
use mag_core::types::CreateAccountRequest;
use mag_core::types::CreateAccountResponse;
use mag_core::types::CreateCharacterRequest;
use mag_core::types::CreateGameLoginTicketRequest;
use mag_core::types::CreateGameLoginTicketResponse;
use mag_core::types::GameLoginTicketMetadata;
use mag_core::types::GetCharactersResponse;
use mag_core::types::JwtClaims;
use mag_core::types::LoginRequest;
use mag_core::types::LoginResponse;
use mag_core::types::ResetPasswordConfirm;
use mag_core::types::ResetPasswordConfirmResponse;
use mag_core::types::ResetPasswordRequest;
use mag_core::types::ResetPasswordRequestResponse;
use mag_core::types::UpdateCharacterRequest;
use mag_core::{constants, traits};
use rand::RngCore;
use rand::rngs::OsRng;
use redis::AsyncCommands;
use subtle::ConstantTimeEq;

const MAX_CHARACTERS_PER_ACCOUNT: usize = 10;

enum CharacterNameValidationError {
    BadRequest(String),
    Unprocessable(String),
    Internal(String),
}

/// Normalizes a character name and validates global availability constraints.
///
/// # Arguments
/// * `con` - Multiplexed KeyDB connection.
/// * `name` - Raw user-provided character name.
/// * `exclude_character_id` - Character ID to ignore during duplicate checks.
///
/// # Returns
/// * `Ok(String)` with the canonical character name.
/// * `Err(CharacterNameValidationError)` with the route-level rejection reason.
async fn normalize_and_validate_character_name(
    con: &mut redis::aio::ConnectionManager,
    name: &str,
    exclude_character_id: Option<u64>,
) -> Result<String, CharacterNameValidationError> {
    let normalized_name = helpers::normalize_character_name(name)
        .map_err(CharacterNameValidationError::BadRequest)?;

    // O(1) global uniqueness check via the character-name claim index.
    let duplicate_check = pipelines::get_character_id_by_name(con, &normalized_name).await;

    match duplicate_check {
        Ok(Some(existing_id)) if Some(existing_id) != exclude_character_id => {
            return Err(CharacterNameValidationError::Unprocessable(format!(
                "name already taken: {}",
                normalized_name
            )));
        }
        Ok(_) => {}
        Err(err) => return Err(CharacterNameValidationError::Internal(err.to_string())),
    }

    match pipelines::character_template_name_exists(con, &normalized_name).await {
        Ok(true) => {
            return Err(CharacterNameValidationError::Unprocessable(format!(
                "name matches character template: {}",
                normalized_name
            )));
        }
        Ok(false) => {}
        Err(err) => return Err(CharacterNameValidationError::Internal(err.to_string())),
    }

    let bad_names = pipelines::load_bad_names(con)
        .await
        .map_err(|err| CharacterNameValidationError::Internal(err.to_string()))?;
    helpers::validate_character_name_bad_patterns(&normalized_name, &bad_names)
        .map_err(CharacterNameValidationError::BadRequest)?;

    Ok(normalized_name)
}

/// Converts a character-name validation failure into the route status code.
///
/// # Arguments
/// * `context` - Log context for the operation being rejected.
/// * `err` - Name validation error.
///
/// # Returns
/// * `StatusCode` to return from the route.
fn character_name_error_status(context: &str, err: CharacterNameValidationError) -> StatusCode {
    match err {
        CharacterNameValidationError::BadRequest(reason) => {
            warn!("{context} rejected: invalid character name: {reason}");
            StatusCode::BAD_REQUEST
        }
        CharacterNameValidationError::Unprocessable(reason) => {
            warn!("{context} rejected: {reason}");
            StatusCode::UNPROCESSABLE_ENTITY
        }
        CharacterNameValidationError::Internal(reason) => {
            error!("Redis read failed: {reason}");
            StatusCode::INTERNAL_SERVER_ERROR
        }
    }
}

/// Creates a new character for the authenticated account.
/// Validates the JWT from the `Authorization` header, validates the request payload, and then
/// writes the character data to KeyDB.
///
/// # Arguments
/// * `con` - Multiplexed KeyDB connection provided by Axum state.
/// * `headers` - Request headers used to extract the `Authorization: Bearer <JWT>` token.
/// * `payload` - Character creation fields (name/description/sex/class).
///
/// # Returns
/// * `(StatusCode::OK, CharacterSummary)` on success.
/// * `(StatusCode::UNAUTHORIZED, default)` when the token is missing/invalid or the account is not found.
/// * `(StatusCode::BAD_REQUEST, default)` when the request payload is invalid.
/// * `(StatusCode::INTERNAL_SERVER_ERROR, default)` on KeyDB or internal failures.
pub(crate) async fn create_new_character(
    State(state): State<ApiState>,
    auth: AuthUser,
    Json(payload): Json<CreateCharacterRequest>,
) -> (StatusCode, Json<CharacterSummary>) {
    let mut con = state.con.clone();

    if !payload.validate() {
        return (StatusCode::BAD_REQUEST, Json(CharacterSummary::default()));
    }

    let CreateCharacterRequest {
        name,
        description,
        sex,
        class,
    } = payload;

    let name = match normalize_and_validate_character_name(&mut con, &name, None).await {
        Ok(value) => value,
        Err(err) => {
            return (
                character_name_error_status("Create character", err),
                Json(CharacterSummary::default()),
            );
        }
    };

    let description = match description.as_deref().map(str::trim) {
        Some(value) if !value.is_empty() => {
            if let Err(err) = helpers::validate_character_description(&name, value) {
                warn!("Create character rejected: invalid description: {err}");
                return (StatusCode::BAD_REQUEST, Json(CharacterSummary::default()));
            }
            value.to_owned()
        }
        _ => {
            let fallback = helpers::default_character_description(&name);
            // Should always be valid, but keep the check in case we change the template later.
            if let Err(err) = helpers::validate_character_description(&name, &fallback) {
                error!("Default description template invalid: {err}");
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(CharacterSummary::default()),
                );
            }
            fallback
        }
    };

    let username_lc = auth.username_lc.clone();
    let user_id = auth.account_id;

    let character_count = match pipelines::count_characters_for_account(&mut con, user_id).await {
        Ok(value) => value,
        Err(err) => {
            error!("Redis read failed: {}", err);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(CharacterSummary::default()),
            );
        }
    };
    if character_count >= MAX_CHARACTERS_PER_ACCOUNT {
        warn!(
            "Create character rejected: account {} already has {} characters",
            user_id, character_count
        );
        return (StatusCode::CONFLICT, Json(CharacterSummary::default()));
    }

    let result =
        pipelines::insert_new_character(&mut con, user_id, &name, Some(&description), sex, class)
            .await;

    match result {
        Ok(character_id) => {
            info!(
                "Character created for account {}: id={}, name={}, sex={:?}, class={:?}",
                username_lc, character_id, name, sex, class
            );
            (
                StatusCode::OK,
                Json(CharacterSummary {
                    id: character_id,
                    name,
                    description,
                    sex,
                    class,
                    selection_sprite_id: Some(mag_core::traits::get_sprite_id_for_class_and_sex(
                        class, sex,
                    ) as u16),
                    server_id: None,
                    rank_index: None,
                }),
            )
        }
        Err(err) => {
            error!("Failed to create character: {}", err);

            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(CharacterSummary::default()),
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
    State(state): State<ApiState>,
    auth: AuthUser,
) -> (StatusCode, Json<GetCharactersResponse>) {
    let mut con = state.con.clone();
    let user_id = auth.account_id;

    let characters = match pipelines::list_characters_for_account(&mut con, user_id).await {
        Ok(values) => values,
        Err(err) => {
            error!("Redis read failed: {}", err);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(GetCharactersResponse { characters: vec![] }),
            );
        }
    };

    (StatusCode::OK, Json(GetCharactersResponse { characters }))
}

/// Creates a short-lived, one-time login ticket for the game server.
///
/// The client uses its account JWT to mint a ticket for a specific character ID.
/// The game server later consumes the ticket from KeyDB during the TCP login handshake.
pub(crate) async fn create_game_login_ticket(
    State(state): State<ApiState>,
    auth: AuthUser,
    Json(payload): Json<CreateGameLoginTicketRequest>,
) -> (StatusCode, Json<CreateGameLoginTicketResponse>) {
    let mut con = state.con.clone();
    let account_id = auth.account_id;

    if payload.client_version < constants::MINVERSION || payload.client_version > constants::VERSION
    {
        warn!(
            "Create game login ticket rejected: unsupported client version {} (supported {}..={})",
            payload.client_version,
            constants::MINVERSION,
            constants::VERSION
        );
        return (
            StatusCode::BAD_REQUEST,
            Json(CreateGameLoginTicketResponse {
                ticket: None,
                error: Some("Unsupported client version".to_owned()),
            }),
        );
    }

    let owner_id = match pipelines::get_character_account_id(&mut con, payload.character_id).await {
        Ok(value) => value,
        Err(err) => {
            error!("Redis read failed: {}", err);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(CreateGameLoginTicketResponse {
                    ticket: None,
                    error: Some("Server error".to_owned()),
                }),
            );
        }
    };

    if owner_id != Some(account_id) {
        warn!(
            "Create game login ticket rejected: account {} does not own character {}",
            account_id, payload.character_id
        );
        return (
            StatusCode::UNAUTHORIZED,
            Json(CreateGameLoginTicketResponse {
                ticket: None,
                error: Some("Unauthorized".to_owned()),
            }),
        );
    }

    match routes_bans::account_is_banned(&mut con, account_id).await {
        Ok(true) => {
            warn!(
                "Create game login ticket rejected: account {} is banned",
                account_id
            );
            return (
                StatusCode::FORBIDDEN,
                Json(CreateGameLoginTicketResponse {
                    ticket: None,
                    error: Some("Account banned".to_owned()),
                }),
            );
        }
        Ok(false) => {}
        Err(err) => {
            error!("Redis read failed: {}", err);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(CreateGameLoginTicketResponse {
                    ticket: None,
                    error: Some("Server error".to_owned()),
                }),
            );
        }
    }

    match routes_bans::character_is_banned(&mut con, payload.character_id).await {
        Ok(true) => {
            warn!(
                "Create game login ticket rejected: character {} is banned",
                payload.character_id
            );
            return (
                StatusCode::FORBIDDEN,
                Json(CreateGameLoginTicketResponse {
                    ticket: None,
                    error: Some("Character banned".to_owned()),
                }),
            );
        }
        Ok(false) => {}
        Err(err) => {
            error!("Redis read failed: {}", err);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(CreateGameLoginTicketResponse {
                    ticket: None,
                    error: Some("Server error".to_owned()),
                }),
            );
        }
    }

    let (sex, class) =
        match pipelines::get_character_login_traits(&mut con, payload.character_id).await {
            Ok(Some(value)) => value,
            Ok(None) => {
                error!(
                    "Create game login ticket failed: character {} missing login metadata",
                    payload.character_id
                );
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(CreateGameLoginTicketResponse {
                        ticket: None,
                        error: Some("Server error".to_owned()),
                    }),
                );
            }
            Err(err) => {
                error!("Redis read failed: {}", err);
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(CreateGameLoginTicketResponse {
                        ticket: None,
                        error: Some("Server error".to_owned()),
                    }),
                );
            }
        };
    let race = traits::get_race_integer(sex == traits::Sex::Male, class);
    let ticket_metadata = GameLoginTicketMetadata {
        account_id,
        character_id: payload.character_id,
        client_version: payload.client_version,
        race,
    };
    let ticket_bytes = match ticket_metadata.to_bytes() {
        Ok(value) => value,
        Err(err) => {
            error!("Failed to encode game login ticket metadata: {}", err);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(CreateGameLoginTicketResponse {
                    ticket: None,
                    error: Some("Server error".to_owned()),
                }),
            );
        }
    };

    // 30 second, one-time ticket stored as typed bincode metadata at `game_login_ticket:{ticket}`.
    // Uses a random u64 to make guessing infeasible.
    let mut attempts = 0u32;
    loop {
        attempts += 1;
        if attempts > 10 {
            error!("Failed to allocate a unique login ticket after retries");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(CreateGameLoginTicketResponse {
                    ticket: None,
                    error: Some("Server error".to_owned()),
                }),
            );
        }

        let mut ticket = OsRng.next_u64();
        if ticket == 0 {
            ticket = 1;
        }
        let key = format!("game_login_ticket:{}", ticket);
        let result: Option<String> = match redis::cmd("SET")
            .arg(&key)
            .arg(&ticket_bytes)
            .arg("EX")
            .arg(30)
            .arg("NX")
            .query_async(&mut con)
            .await
        {
            Ok(value) => value,
            Err(err) => {
                error!("Redis write failed: {}", err);
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(CreateGameLoginTicketResponse {
                        ticket: None,
                        error: Some("Server error".to_owned()),
                    }),
                );
            }
        };

        if result.is_some() {
            info!(
                "Issued game login ticket for account {} character {} version {} race {}",
                account_id, payload.character_id, payload.client_version, race
            );
            return (
                StatusCode::OK,
                Json(CreateGameLoginTicketResponse {
                    ticket: Some(ticket),
                    error: None,
                }),
            );
        }
    }
}

/// Creates a new account and registers minimal claim keys for username/email uniqueness.
/// Validates email/username/password formats, enforces uniqueness using atomic `SET ... NX` claim
/// keys, and writes the account hash.
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
    State(state): State<ApiState>,
    Json(payload): Json<CreateAccountRequest>,
) -> (StatusCode, Json<CreateAccountResponse>) {
    let mut con = state.con.clone();
    let email_lc = payload.email.trim().to_lowercase();
    let username_lc = payload.username.trim().to_lowercase();

    info!(
        "Create account request: username={}, email={}",
        username_lc, email_lc
    );
    let response = CreateAccountResponse {
        id: None,
        error: None,
        username: username_lc.clone(),
        email: email_lc.clone(),
    };

    if !helpers::is_valid_email_regex(&email_lc) {
        warn!("Create account rejected: invalid email {}", email_lc);
        return (
            StatusCode::BAD_REQUEST,
            Json(CreateAccountResponse {
                error: Some("Invalid email".to_owned()),
                ..response
            }),
        );
    }

    if !helpers::is_valid_username(&username_lc) {
        warn!("Create account rejected: invalid username {}", username_lc);
        return (
            StatusCode::BAD_REQUEST,
            Json(CreateAccountResponse {
                error: Some("Invalid username".to_owned()),
                ..response
            }),
        );
    }

    if !helpers::is_valid_password(&payload.password) {
        warn!("Create account rejected: invalid password format");
        return (
            StatusCode::BAD_REQUEST,
            Json(CreateAccountResponse {
                error: Some("Invalid password".to_owned()),
                ..response
            }),
        );
    }

    // Server-side rehash of the client-submitted PHC string before any
    // KeyDB writes. Storing only this value means a KeyDB compromise never
    // yields a directly-replayable credential against the API.
    let stored_password = match password::hash_for_storage(&payload.password) {
        Ok(value) => value,
        Err(err) => {
            error!("Server-side password hashing failed: {err}");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(CreateAccountResponse {
                    error: Some("Server error".to_owned()),
                    ..response
                }),
            );
        }
    };

    let id_key = "account:next_id";
    let id: u64 = match con.incr(id_key, 1).await {
        Ok(value) => value,
        Err(err) => {
            error!("Redis INCR failed: {}", err);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(CreateAccountResponse {
                    error: Some(format!("Redis error: {}", err)),
                    ..response
                }),
            );
        }
    };
    info!("Allocated account id {}", id);

    let username_claim_key = format!("account:username:{}", username_lc);
    let email_claim_key = format!("account:email:{}", email_lc);

    let username_claimed = match pipelines::claim_username(&mut con, &username_lc, id).await {
        Ok(value) => value,
        Err(err) => {
            error!("Redis claim failed: {}", err);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(CreateAccountResponse {
                    error: Some(format!("Redis error: {}", err)),
                    ..response
                }),
            );
        }
    };
    if !username_claimed {
        info!(
            "Create account rejected: duplicate username {}",
            username_lc
        );
        return (
            StatusCode::CONFLICT,
            Json(CreateAccountResponse {
                error: Some("Username is already in use".to_owned()),
                ..response
            }),
        );
    }

    let email_claimed = match pipelines::claim_email(&mut con, &email_lc, id).await {
        Ok(value) => value,
        Err(err) => {
            let _ = pipelines::release_claim_if_matches(&mut con, &username_claim_key, id).await;
            error!("Redis claim failed: {}", err);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(CreateAccountResponse {
                    error: Some(format!("Redis error: {}", err)),
                    ..response
                }),
            );
        }
    };
    if !email_claimed {
        let _ = pipelines::release_claim_if_matches(&mut con, &username_claim_key, id).await;
        info!("Create account rejected: duplicate email {}", email_lc);
        return (
            StatusCode::CONFLICT,
            Json(CreateAccountResponse {
                error: Some("Email is already in use".to_owned()),
                ..response
            }),
        );
    }

    if let Err(err) =
        pipelines::insert_account_hash(&mut con, id, &email_lc, &username_lc, &stored_password)
            .await
    {
        let _ = pipelines::release_claim_if_matches(&mut con, &username_claim_key, id).await;
        let _ = pipelines::release_claim_if_matches(&mut con, &email_claim_key, id).await;
        error!("Redis write failed: {}", err);
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(CreateAccountResponse {
                error: Some(format!("Redis error: {}", err)),
                ..response
            }),
        );
    }

    (
        StatusCode::CREATED,
        Json(CreateAccountResponse {
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
    State(state): State<ApiState>,
    auth: AuthUser,
    Path(character_id): Path<u64>,
    Json(payload): Json<UpdateCharacterRequest>,
) -> StatusCode {
    let mut con = state.con.clone();
    let account_id = auth.account_id;

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
            character_id, auth.username_lc
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

    let normalized_name = match payload.name.as_deref() {
        Some(name) => {
            match normalize_and_validate_character_name(&mut con, name, Some(character_id)).await {
                Ok(value) => Some(value),
                Err(err) => return character_name_error_status("Update character", err),
            }
        }
        None => None,
    };

    let description_for_validation = match payload.description.as_deref() {
        Some(description) => Some(description.trim().to_owned()),
        None if normalized_name.is_some() => {
            match pipelines::get_character_description(&mut con, character_id).await {
                Ok(Some(value)) => Some(value.trim().to_owned()),
                Ok(None) => {
                    warn!(
                        "Update character rejected: missing stored description for character {}",
                        character_id
                    );
                    return StatusCode::BAD_REQUEST;
                }
                Err(err) => {
                    error!("Redis read failed: {}", err);
                    return StatusCode::INTERNAL_SERVER_ERROR;
                }
            }
        }
        None => None,
    };

    if let Some(description) = description_for_validation.as_deref() {
        let name_for_validation = match normalized_name.as_deref() {
            Some(value) => value.to_owned(),
            None => match pipelines::get_character_name(&mut con, character_id).await {
                Ok(Some(value)) => value.trim().to_owned(),
                Ok(None) => {
                    warn!(
                        "Update character rejected: missing stored name for character {}",
                        character_id
                    );
                    return StatusCode::BAD_REQUEST;
                }
                Err(err) => {
                    error!("Redis read failed: {}", err);
                    return StatusCode::INTERNAL_SERVER_ERROR;
                }
            },
        };

        if payload.description.is_some() {
            let server_id = match pipelines::get_character_server_id(&mut con, character_id).await {
                Ok(value) => value,
                Err(err) => {
                    error!("Redis read failed: {}", err);
                    return StatusCode::INTERNAL_SERVER_ERROR;
                }
            };
            if let Some(server_id) = server_id {
                match pipelines::character_slot_has_no_desc(&mut con, server_id).await {
                    Ok(true) => {
                        warn!(
                            "Update character rejected: NoDesc flag blocks description changes for character {}",
                            character_id
                        );
                        return StatusCode::BAD_REQUEST;
                    }
                    Ok(false) => {}
                    Err(err) => {
                        error!("Redis read failed: {}", err);
                        return StatusCode::INTERNAL_SERVER_ERROR;
                    }
                }
            }
        }

        if let Err(err) = helpers::validate_character_description(&name_for_validation, description)
        {
            warn!(
                "Update character rejected: invalid description for character {}: {}",
                character_id, err
            );
            return StatusCode::BAD_REQUEST;
        }
    }

    match pipelines::update_character(
        &mut con,
        character_id,
        normalized_name.as_deref(),
        payload.description.as_deref(),
    )
    .await
    {
        Ok(_) => {
            info!(
                "Character {} updated for account {}",
                character_id, auth.username_lc
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
    State(state): State<ApiState>,
    auth: AuthUser,
    Path(character_id): Path<u64>,
) -> StatusCode {
    let mut con = state.con.clone();
    let account_id = auth.account_id;

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
            character_id, auth.username_lc
        );
        return StatusCode::UNAUTHORIZED;
    }

    match pipelines::delete_character(&mut con, character_id).await {
        Ok(_) => {
            info!(
                "Character {} deleted for account {}",
                character_id, auth.username_lc
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
    State(state): State<ApiState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Json(payload): Json<LoginRequest>,
) -> axum::response::Response {
    let mut con = state.con.clone();
    let username_lc = payload.username.trim().to_lowercase();
    let ip = addr.ip();
    info!("Login request for username={}", username_lc);

    // Per-IP login lockout (separate from generic per-IP rate limit so that
    // repeated bad credentials specifically can be throttled hard).
    if let rate_limit::LoginGateOutcome::LockedOut { retry_after_secs } =
        rate_limit::check_login_lockout(&mut con, ip).await
    {
        warn!("Login rejected: IP {ip} is locked out for {retry_after_secs}s");
        return rate_limit::login_locked_out_response(retry_after_secs);
    }

    if !helpers::is_valid_password(&payload.password) {
        warn!("Login rejected: invalid password format");
        return (
            StatusCode::BAD_REQUEST,
            login_response(None, Some("Invalid password format")),
        )
            .into_response();
    }

    let user_id = match pipelines::get_account_id_by_username(&mut con, &username_lc).await {
        Ok(Some(value)) => value,
        Ok(None) => {
            warn!("Login rejected: username not found {}", username_lc);
            rate_limit::record_login_failure(&mut con, ip).await;
            return (
                StatusCode::UNAUTHORIZED,
                login_response(None, Some("Invalid username or password")),
            )
                .into_response();
        }
        Err(err) => {
            error!("Redis read failed: {}", err);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                login_response(None, Some("Server error")),
            )
                .into_response();
        }
    };

    let stored_hash: Option<String> =
        match pipelines::get_account_password_hash(&mut con, user_id).await {
            Ok(value) => value,
            Err(err) => {
                error!("Redis read failed: {}", err);
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    login_response(None, Some("Server error")),
                )
                    .into_response();
            }
        };

    let stored_hash = match stored_hash {
        Some(value) => value,
        None => {
            warn!(
                "Login rejected: missing password hash for {}",
                payload.username
            );
            rate_limit::record_login_failure(&mut con, ip).await;
            return (
                StatusCode::UNAUTHORIZED,
                login_response(None, Some("Invalid username or password")),
            )
                .into_response();
        }
    };

    if !password::verify(&stored_hash, &payload.password) {
        warn!("Login rejected: password mismatch for {}", username_lc);
        rate_limit::record_login_failure(&mut con, ip).await;
        return (
            StatusCode::UNAUTHORIZED,
            login_response(None, Some("Invalid username or password")),
        )
            .into_response();
    }

    match routes_bans::account_is_banned(&mut con, user_id).await {
        Ok(true) => {
            warn!("Login rejected: account {} is banned", user_id);
            return (
                StatusCode::FORBIDDEN,
                login_response(None, Some("Account banned")),
            )
                .into_response();
        }
        Ok(false) => {}
        Err(err) => {
            error!("Redis read failed: {}", err);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                login_response(None, Some("Server error")),
            )
                .into_response();
        }
    }

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::from_secs(0))
        .as_secs();
    let claims = JwtClaims {
        sub: username_lc,
        exp: (now + 3600) as usize,
    };

    let token = match jsonwebtoken::encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(state.jwt_secret.as_ref()),
    ) {
        Ok(value) => value,
        Err(err) => {
            error!("JWT encode failed: {}", err);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                login_response(None, Some("Server error")),
            )
                .into_response();
        }
    };

    // Successful login clears the failure counter to keep good clients out of
    // the lockout window.
    rate_limit::clear_login_failures(&mut con, ip).await;

    (StatusCode::OK, login_response(Some(token), None)).into_response()
}

/// Builds the JSON body for a `/login` response.
///
/// # Arguments
///
/// * `token` - Signed JWT on success, `None` on failure.
/// * `error` - Optional human-readable error message.
///
/// # Returns
///
/// * `Json<LoginResponse>` ready to return from a handler.
fn login_response(token: Option<String>, error: Option<&str>) -> Json<LoginResponse> {
    Json(LoginResponse {
        token,
        error: error.map(str::to_owned),
    })
}

/// Maximum number of password reset requests allowed per IP within the rate
/// limit window.
const MAX_RESET_ATTEMPTS_PER_IP: u64 = 3;

/// TTL in seconds for both password reset codes and per-IP attempt counters.
const RESET_TTL_SECS: u64 = 900;

/// Initiates a password reset by sending a 6-digit code to the email on file.
///
/// Always returns 200 with a generic message regardless of whether the
/// username/email matched, to prevent account enumeration.
///
/// # Arguments
///
/// * `state` - Shared application state (KeyDB + optional email sender).
/// * `connect_info` - Client socket address used for per-IP rate limiting.
/// * `payload` - Username and email submitted by the client.
///
/// # Returns
///
/// * `(200, message)` in all normal cases (match or mismatch).
/// * `(429, message)` when the per-IP rate limit is exceeded.
/// * `(503, message)` when SMTP is not configured.
pub(crate) async fn request_password_reset(
    State(state): State<ApiState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Json(payload): Json<ResetPasswordRequest>,
) -> (StatusCode, Json<ResetPasswordRequestResponse>) {
    let mut con = state.con.clone();

    let generic_ok = ResetPasswordRequestResponse {
        message: "If an account with that username and email exists, a reset code has been sent."
            .to_owned(),
    };

    // ── Validate SMTP availability ───────────────────────────────────
    let email_sender = match &state.email_sender {
        Some(sender) => sender.clone(),
        None => {
            warn!("Password reset requested but SMTP is not configured");
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ResetPasswordRequestResponse {
                    message: "Password reset is not available at this time.".to_owned(),
                }),
            );
        }
    };

    // ── Validate input ───────────────────────────────────────────────
    let username_lc = payload.username.trim().to_lowercase();
    let email_lc = payload.email.trim().to_lowercase();

    if username_lc.is_empty() || email_lc.is_empty() {
        return (StatusCode::OK, Json(generic_ok));
    }

    // ── Per-IP rate limit ────────────────────────────────────────────
    // INCR first and check the returned value so that the check and increment
    // are effectively atomic — avoids the TOCTOU race where concurrent requests
    // all read the same count before any of them increments it.
    let ip_key = format!("password_reset_attempts:{}", addr.ip());
    let new_attempt_count: u64 =
        (redis::cmd("INCR").arg(&ip_key).query_async(&mut con).await).unwrap_or(1);
    let _: Result<(), _> = redis::cmd("EXPIRE")
        .arg(&ip_key)
        .arg(RESET_TTL_SECS)
        .query_async(&mut con)
        .await;
    if new_attempt_count > MAX_RESET_ATTEMPTS_PER_IP {
        warn!("Password reset rate limited for IP {}", addr.ip());
        return (
            StatusCode::TOO_MANY_REQUESTS,
            Json(ResetPasswordRequestResponse {
                message: "Too many reset attempts. Please try again later.".to_owned(),
            }),
        );
    }

    // ── Resolve account ──────────────────────────────────────────────
    let account_id = match pipelines::get_account_id_by_username(&mut con, &username_lc).await {
        Ok(Some(id)) => id,
        Ok(None) => {
            info!("Password reset: username not found (generic OK returned)");
            return (StatusCode::OK, Json(generic_ok));
        }
        Err(err) => {
            error!("Redis read failed during password reset: {err}");
            return (StatusCode::OK, Json(generic_ok));
        }
    };

    // ── Verify email matches ─────────────────────────────────────────
    let stored_email: Option<String> =
        match pipelines::get_account_email(&mut con, account_id).await {
            Ok(v) => v,
            Err(err) => {
                error!("Redis read failed during password reset: {err}");
                return (StatusCode::OK, Json(generic_ok));
            }
        };

    match stored_email {
        Some(ref stored) if stored == &email_lc => {}
        _ => {
            info!("Password reset: email mismatch (generic OK returned)");
            return (StatusCode::OK, Json(generic_ok));
        }
    }

    // ── Generate 6-digit code ────────────────────────────────────────
    let code = format!("{:06}", OsRng.next_u32() % 1_000_000);

    // ── Store in KeyDB (one active per account) ──────────────────────
    let reset_key = format!("password_reset:{}", account_id);
    let _: Result<(), _> = con.del(&reset_key).await;
    let _: Result<(), _> = redis::cmd("HSET")
        .arg(&reset_key)
        .arg("code")
        .arg(&code)
        .arg("created_at")
        .arg(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or(Duration::from_secs(0))
                .as_secs(),
        )
        .query_async::<()>(&mut con)
        .await;
    let _: Result<(), _> = redis::cmd("EXPIRE")
        .arg(&reset_key)
        .arg(RESET_TTL_SECS)
        .query_async::<()>(&mut con)
        .await;

    // ── Send email (log failure but return success to prevent enumeration) ─
    if let Err(err) = email_sender.send_reset_code(&email_lc, &code).await {
        error!("Failed to send password reset email: {err}");
    }

    info!("Password reset code issued for account {account_id}");
    (StatusCode::OK, Json(generic_ok))
}

/// Confirms a password reset using the emailed 6-digit code.
///
/// # Arguments
///
/// * `state` - Shared application state (KeyDB connection).
/// * `payload` - Username, 6-digit code, and new password (Argon2 PHC hash).
///
/// # Returns
///
/// * `(200, message)` on success.
/// * `(400, message)` when the code is invalid, expired, or the password format is wrong.
/// * `(500, message)` on internal failure.
pub(crate) async fn confirm_password_reset(
    State(state): State<ApiState>,
    Json(payload): Json<ResetPasswordConfirm>,
) -> (StatusCode, Json<ResetPasswordConfirmResponse>) {
    let mut con = state.con.clone();

    let fail = |msg: &str| {
        (
            StatusCode::BAD_REQUEST,
            Json(ResetPasswordConfirmResponse {
                message: msg.to_owned(),
            }),
        )
    };

    // ── Validate inputs ──────────────────────────────────────────────
    if !helpers::is_valid_password(&payload.new_password) {
        warn!("Password reset confirm rejected: invalid password format");
        return fail("Invalid password format");
    }

    if !helpers::is_valid_reset_code(&payload.code) {
        warn!("Password reset confirm rejected: invalid code format");
        return fail("Invalid or expired reset code");
    }

    let username_lc = payload.username.trim().to_lowercase();
    if username_lc.is_empty() {
        return fail("Invalid or expired reset code");
    }

    // ── Resolve account ──────────────────────────────────────────────
    let account_id = match pipelines::get_account_id_by_username(&mut con, &username_lc).await {
        Ok(Some(id)) => id,
        Ok(None) => {
            warn!("Password reset confirm: username not found");
            return fail("Invalid or expired reset code");
        }
        Err(err) => {
            error!("Redis read failed during password reset confirm: {err}");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ResetPasswordConfirmResponse {
                    message: "Server error".to_owned(),
                }),
            );
        }
    };

    // ── Retrieve stored code ─────────────────────────────────────────
    let reset_key = format!("password_reset:{}", account_id);
    let stored_code: Option<String> = match con.hget(&reset_key, "code").await {
        Ok(v) => v,
        Err(err) => {
            error!("Redis read failed during password reset confirm: {err}");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ResetPasswordConfirmResponse {
                    message: "Server error".to_owned(),
                }),
            );
        }
    };

    let stored_code = match stored_code {
        Some(c) => c,
        None => {
            warn!("Password reset confirm: no active reset code for account {account_id}");
            return fail("Invalid or expired reset code");
        }
    };

    // ── Constant-time comparison ─────────────────────────────────────
    if stored_code
        .as_bytes()
        .ct_eq(payload.code.as_bytes())
        .unwrap_u8()
        != 1
    {
        warn!("Password reset confirm: code mismatch for account {account_id}");
        return fail("Invalid or expired reset code");
    }

    // ── Update password ──────────────────────────────────────────────
    let stored_password = match password::hash_for_storage(&payload.new_password) {
        Ok(value) => value,
        Err(err) => {
            error!("Server-side password hashing failed during reset: {err}");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ResetPasswordConfirmResponse {
                    message: "Server error".to_owned(),
                }),
            );
        }
    };

    if let Err(err) = pipelines::set_account_password(&mut con, account_id, &stored_password).await
    {
        error!("Redis write failed during password reset: {err}");
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ResetPasswordConfirmResponse {
                message: "Server error".to_owned(),
            }),
        );
    }

    // ── Consume the token ────────────────────────────────────────────
    let _: Result<(), _> = con.del(&reset_key).await;

    info!("Password successfully reset for account {account_id}");
    (
        StatusCode::OK,
        Json(ResetPasswordConfirmResponse {
            message: "Password has been reset successfully.".to_owned(),
        }),
    )
}
