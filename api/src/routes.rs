use std::env;
use std::time::Duration;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use crate::helpers;
use crate::pipelines;
use crate::pipelines::DuplicateCheckResult;
use crate::types;

use axum::{extract::Path, extract::State, http::StatusCode, Json};
use jsonwebtoken::EncodingKey;
use jsonwebtoken::Header;
use log::{error, info, warn};
use redis::AsyncCommands;

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

    let username_key = format!("account:username:{}", token_data.claims.sub);
    let user_id: Option<u64> = match con.get(&username_key).await {
        Ok(value) => value,
        Err(err) => {
            error!("Redis read failed: {}", err);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(types::CharacterSummary::default()),
            );
        }
    };

    let user_id = match user_id {
        Some(value) => value,
        None => {
            warn!(
                "Create character rejected: account not found for {}",
                token_data.claims.sub
            );
            return (
                StatusCode::UNAUTHORIZED,
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

    let username_key = format!("account:username:{}", token_data.claims.sub);
    let user_id: Option<u64> = match con.get(&username_key).await {
        Ok(value) => value,
        Err(err) => {
            error!("Redis read failed: {}", err);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(types::GetCharactersResponse { characters: vec![] }),
            );
        }
    };

    if user_id.is_none() {
        return (
            StatusCode::UNAUTHORIZED,
            Json(types::GetCharactersResponse { characters: vec![] }),
        );
    }

    let user_id = user_id.unwrap();
    let account_characters_key = format!("account:{}:characters", user_id);
    let character_ids: Vec<u64> = match con.smembers(&account_characters_key).await {
        Ok(values) => values,
        Err(err) => {
            error!("Redis read failed: {}", err);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(types::GetCharactersResponse { characters: vec![] }),
            );
        }
    };

    if character_ids.is_empty() {
        return (
            StatusCode::OK,
            Json(types::GetCharactersResponse { characters: vec![] }),
        );
    }

    let mut pipe = redis::pipe();
    for character_id in &character_ids {
        let character_key = format!("character:{}", character_id);
        pipe.cmd("HGETALL").arg(character_key);
    }

    let results: Vec<redis::Value> = match pipe.query_async(&mut con).await {
        Ok(values) => values,
        Err(err) => {
            error!("Redis read failed: {}", err);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(types::GetCharactersResponse { characters: vec![] }),
            );
        }
    };

    let mut characters = Vec::new();
    for (index, result) in results.iter().enumerate() {
        let character_id = character_ids[index];
        let character_map: std::collections::HashMap<String, String> =
            match redis::from_redis_value(result.clone()) {
                Ok(value) => value,
                Err(err) => {
                    warn!("Failed to decode character {}: {}", character_id, err);
                    continue;
                }
            };

        let name = match character_map.get("name") {
            Some(value) => value.clone(),
            None => {
                warn!("Character {} is missing name", character_id);
                continue;
            }
        };

        let description = character_map
            .get("description")
            .cloned()
            .unwrap_or_default();

        let sex_value: u32 = match character_map
            .get("sex")
            .and_then(|value| value.parse().ok())
        {
            Some(value) => value,
            None => {
                warn!("Character {} is missing sex", character_id);
                continue;
            }
        };

        let race_value: u32 = match character_map
            .get("race")
            .and_then(|value| value.parse().ok())
        {
            Some(value) => value,
            None => {
                warn!("Character {} is missing race", character_id);
                continue;
            }
        };

        let sex = match types::sex_from_u32(sex_value) {
            Some(value) => value,
            None => {
                warn!("Character {} has invalid sex value", character_id);
                continue;
            }
        };

        let race = match types::race_from_u32(race_value) {
            Some(value) => value,
            None => {
                warn!("Character {} has invalid race value", character_id);
                continue;
            }
        };

        characters.push(types::CharacterSummary {
            id: character_id,
            name,
            description,
            sex,
            race,
        });
    }

    (
        StatusCode::OK,
        Json(types::GetCharactersResponse { characters }),
    )
}

pub(crate) async fn create_account(
    State(mut con): State<redis::aio::MultiplexedConnection>,
    Json(payload): Json<types::CreateAccountRequest>,
) -> (StatusCode, Json<types::CreateAccountResponse>) {
    info!(
        "Create account request: username={}, email={}",
        payload.username, payload.email
    );
    let response = types::CreateAccountResponse {
        id: None,
        error: None,
        username: payload.username.clone(),
        password: payload.password.clone(),
        email: payload.email.clone(),
    };

    if !helpers::is_valid_email_regex(&payload.email) {
        warn!("Create account rejected: invalid email {}", payload.email);
        return (
            StatusCode::BAD_REQUEST,
            Json(types::CreateAccountResponse {
                error: Some("Invalid email".to_string()),
                ..response
            }),
        );
    }

    if !helpers::is_valid_username(&payload.username) {
        warn!(
            "Create account rejected: invalid username {}",
            payload.username
        );
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

    let email_key = format!("account:email:{}", payload.email);
    let username_key = format!("account:username:{}", payload.username);
    let id_key = "account:next_id";
    let account_prefix = "account:";

    const MAX_RETRIES: usize = 5;
    let mut attempts = 0;

    let id = loop {
        attempts += 1;

        let watch_result: Result<(), redis::RedisError> = redis::cmd("WATCH")
            .arg(&[&email_key, &username_key])
            .query_async(&mut con)
            .await;

        if let Err(err) = watch_result {
            error!("Redis WATCH failed: {}", err);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(types::CreateAccountResponse {
                    error: Some(format!("Redis error: {}", err)),
                    ..response
                }),
            );
        }

        let duplicate_check =
            match pipelines::check_account_duplicates(&mut con, &email_key, &username_key).await {
                Ok(value) => value,
                Err(err) => {
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
            DuplicateCheckResult::Email => {
                let _ = redis::cmd("UNWATCH").query_async::<()>(&mut con).await;
                info!("Create account rejected: duplicate email {}", payload.email);
                return (
                    StatusCode::CONFLICT,
                    Json(types::CreateAccountResponse {
                        error: Some("Email is already in use".to_string()),
                        ..response
                    }),
                );
            }
            DuplicateCheckResult::Username => {
                let _ = redis::cmd("UNWATCH").query_async::<()>(&mut con).await;
                info!(
                    "Create account rejected: duplicate username {}",
                    payload.username
                );
                return (
                    StatusCode::CONFLICT,
                    Json(types::CreateAccountResponse {
                        error: Some("Username is already in use".to_string()),
                        ..response
                    }),
                );
            }
            DuplicateCheckResult::None => {}
        }

        let new_id: u64 = match con.incr(id_key, 1).await {
            Ok(value) => value,
            Err(err) => {
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
        info!("Allocated account id {}", new_id);

        let account_key = format!("{}{}", account_prefix, new_id);
        let exec_result = pipelines::insert_account_hash(
            &mut con,
            &account_key,
            &email_key,
            &username_key,
            new_id,
            &payload.email,
            &payload.username,
            &payload.password,
        )
        .await;

        match exec_result {
            Ok(_) => {
                info!(
                    "Account created: id={}, username={}",
                    new_id, payload.username
                );
                break new_id;
            }
            Err(err)
                if err.kind() == redis::ErrorKind::Server(redis::ServerErrorKind::ExecAbort) =>
            {
                if attempts >= MAX_RETRIES {
                    error!(
                        "Account creation retry limit reached for username={}",
                        payload.username
                    );
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(types::CreateAccountResponse {
                            error: Some(
                                "Failed to create account, retry limit reached".to_string(),
                            ),
                            ..response
                        }),
                    );
                }
                warn!("Account creation retry due to transaction abort");
                continue;
            }
            Err(err) => {
                error!("Redis write failed: {}", err);
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(types::CreateAccountResponse {
                        error: Some(format!("Redis error: {}", err)),
                        ..response
                    }),
                );
            }
        }
    };

    (
        StatusCode::CREATED,
        Json(types::CreateAccountResponse {
            id: Some(id),
            error: None,
            username: payload.username,
            password: payload.password,
            email: payload.email,
        }),
    )
}

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

    let username_key = format!("account:username:{}", token_data.claims.sub);
    let account_id = match pipelines::get_account_id_by_username(&mut con, &username_key).await {
        Ok(value) => match value {
            Some(id) => id,
            None => {
                warn!(
                    "Unauthorized update attempt: account not found for {}",
                    token_data.claims.sub
                );
                return StatusCode::UNAUTHORIZED;
            }
        },
        Err(err) => {
            error!("Redis read failed: {}", err);
            return StatusCode::INTERNAL_SERVER_ERROR;
        }
    };

    let does_character_belong_to_user: bool =
        match pipelines::check_character_ownership(&mut con, account_id, character_id).await {
            Ok(value) => value,
            Err(err) => {
                error!("Redis read failed: {}", err);
                return StatusCode::INTERNAL_SERVER_ERROR;
            }
        };
    if !does_character_belong_to_user {
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

    let username_key = format!("account:username:{}", token_data.claims.sub);
    let account_id = match pipelines::get_account_id_by_username(&mut con, &username_key).await {
        Ok(value) => match value {
            Some(id) => id,
            None => {
                warn!(
                    "Unauthorized delete attempt: account not found for {}",
                    token_data.claims.sub
                );
                return StatusCode::UNAUTHORIZED;
            }
        },
        Err(err) => {
            error!("Redis read failed: {}", err);
            return StatusCode::INTERNAL_SERVER_ERROR;
        }
    };

    let does_character_belong_to_user: bool =
        match pipelines::check_character_ownership(&mut con, account_id, character_id).await {
            Ok(value) => value,
            Err(err) => {
                error!("Redis read failed: {}", err);
                return StatusCode::INTERNAL_SERVER_ERROR;
            }
        };
    if !does_character_belong_to_user {
        warn!(
            "Unauthorized delete attempt: character {} does not belong to user {}",
            character_id, token_data.claims.sub
        );
        return StatusCode::UNAUTHORIZED;
    }

    match pipelines::delete_character(&mut con, account_id, character_id).await {
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

pub(crate) async fn login(
    State(mut con): State<redis::aio::MultiplexedConnection>,
    Json(payload): Json<types::LoginRequest>,
) -> (StatusCode, Json<types::LoginResponse>) {
    info!("Login request for username={}", payload.username);
    if !helpers::is_valid_password(&payload.password) {
        warn!("Login rejected: invalid password format");
        return (
            StatusCode::BAD_REQUEST,
            Json(types::LoginResponse {
                token: String::new(),
            }),
        );
    }

    let username_key = format!("account:username:{}", payload.username);
    let user_id: Option<u64> = match con.get(&username_key).await {
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

    let user_id = match user_id {
        Some(value) => value,
        None => {
            warn!("Login rejected: username not found {}", payload.username);
            return (
                StatusCode::UNAUTHORIZED,
                Json(types::LoginResponse {
                    token: String::new(),
                }),
            );
        }
    };

    let account_key = format!("account:{}", user_id);
    let stored_hash: Option<String> = match con.hget(&account_key, "password").await {
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
        warn!("Login rejected: password mismatch for {}", payload.username);
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
        sub: payload.username,
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
