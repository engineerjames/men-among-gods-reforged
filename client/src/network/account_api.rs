use std::env;
use std::time::Duration;

use argon2::password_hash::SaltString;
use argon2::{Argon2, PasswordHasher};
use bevy::prelude::Resource;
use mag_core::traits::{Class, Sex};
use reqwest::blocking::Client;
use reqwest::StatusCode;

pub use mag_core::types::api::CharacterSummary;
use mag_core::types::api::{
    CreateAccountRequest, CreateAccountResponse, CreateCharacterRequest,
    CreateGameLoginTicketRequest, CreateGameLoginTicketResponse, GetCharactersResponse,
    LoginRequest, LoginResponse,
};

/// Stores API connection state and user context.
#[derive(Debug, Default, Clone, Resource)]
pub struct ApiSession {
    pub base_url: String,
    pub token: Option<String>,
    pub username: Option<String>,
    pub pending_notice: Option<String>,
}

impl ApiSession {
    /// Ensures the session has a usable API base URL.
    pub fn ensure_defaults(&mut self) {
        if self.base_url.trim().is_empty() {
            self.base_url = default_api_base_url();
        }
    }
}

/// Returns the API base URL, honoring `MAG_API_BASE_URL` when set.
///
/// # Returns
/// * API base URL as a string.
pub fn default_api_base_url() -> String {
    if let Ok(value) = env::var("MAG_API_BASE_URL") {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }

    if cfg!(debug_assertions) {
        "http://127.0.0.1:5554".to_string()
    } else {
        "http://menamonggods.ddns.net:5554".to_string()
    }
}

/// Hashes a password into Argon2 PHC format using a deterministic salt.
///
/// # Arguments
/// * `username` - Account username (used to derive the salt).
/// * `password` - Raw password input.
///
/// # Returns
/// * `Ok(hash)` containing the PHC string.
/// * `Err(String)` when hashing fails.
fn hash_password(username: &str, password: &str) -> Result<String, String> {
    let username_lc = username.trim().to_lowercase();
    let salt_seed = format!("mag:{}", username_lc);
    let salt = SaltString::b64_encode(salt_seed.as_bytes())
        .map_err(|err| format!("Failed to encode password salt: {err}"))?;
    let password_hash = Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map_err(|err| format!("Failed to hash password: {err}"))?
        .to_string();
    Ok(password_hash)
}

/// Logs in to the account API and returns a JWT on success.
///
/// # Arguments
/// * `base_url` - API base URL.
/// * `username` - Account username.
/// * `password` - Raw password input.
///
/// # Returns
/// * `Ok(token)` containing the JWT.
/// * `Err(String)` when the request or authentication fails.
pub fn login(base_url: &str, username: &str, password: &str) -> Result<String, String> {
    let password_hash = hash_password(username, password)?;
    let client = Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|err| format!("Failed to build HTTP client: {err}"))?;

    let url = format!("{}/login", base_url.trim_end_matches('/'));
    let resp = client
        .post(url)
        .json(&LoginRequest {
            username: username.to_string(),
            password: password_hash,
        })
        .send()
        .map_err(|err| format!("Login request failed: {err}"))?;

    let status = resp.status();
    if status.is_success() {
        let body: LoginResponse = resp
            .json()
            .map_err(|err| format!("Failed to parse login response: {err}"))?;
        if body.token.trim().is_empty() {
            return Err("Login failed: empty token".to_string());
        }
        return Ok(body.token);
    }

    let message = match status {
        StatusCode::BAD_REQUEST => "Invalid password format",
        StatusCode::UNAUTHORIZED => "Invalid username or password",
        StatusCode::INTERNAL_SERVER_ERROR => "Server error",
        _ => "Login failed",
    };

    Err(format!("{message} ({})", status.as_u16()))
}

/// Creates a new account via the account API.
///
/// # Arguments
/// * `base_url` - API base URL.
/// * `email` - Account email address.
/// * `username` - Desired username.
/// * `password` - Raw password input.
///
/// # Returns
/// * `Ok(message)` on success.
/// * `Err(String)` when validation or the request fails.
pub fn create_account(
    base_url: &str,
    email: &str,
    username: &str,
    password: &str,
) -> Result<String, String> {
    let password_hash = hash_password(username, password)?;
    let client = Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|err| format!("Failed to build HTTP client: {err}"))?;

    let url = format!("{}/accounts", base_url.trim_end_matches('/'));
    let resp = client
        .post(url)
        .json(&CreateAccountRequest {
            email: email.to_string(),
            username: username.to_string(),
            password: password_hash,
        })
        .send()
        .map_err(|err| format!("Account creation request failed: {err}"))?;

    let status = resp.status();
    let body: CreateAccountResponse = resp
        .json()
        .map_err(|err| format!("Failed to parse account creation response: {err}"))?;

    if status.is_success() && body.id.is_some() {
        return Ok(format!("Account created for {}", body.username));
    }

    let fallback = match status {
        StatusCode::BAD_REQUEST => "Invalid account details",
        StatusCode::CONFLICT => "Account already exists",
        StatusCode::INTERNAL_SERVER_ERROR => "Server error",
        _ => "Account creation failed",
    };

    Err(body.error.unwrap_or_else(|| fallback.to_string()))
}

/// Creates a new character via the account API.
///
/// # Arguments
/// * `base_url` - API base URL.
/// * `token` - JWT bearer token.
/// * `name` - Character name.
/// * `description` - Character description.
/// * `sex` - Character sex.
/// * `class` - Character class.
///
/// # Returns
/// * `Ok(CharacterSummary)` on success.
/// * `Err(String)` when validation or the request fails.
pub fn create_character(
    base_url: &str,
    token: &str,
    name: &str,
    description: Option<&str>,
    sex: Sex,
    class: Class,
) -> Result<CharacterSummary, String> {
    let client = Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|err| format!("Failed to build HTTP client: {err}"))?;

    let url = format!("{}/characters", base_url.trim_end_matches('/'));
    let resp = client
        .post(url)
        .bearer_auth(token)
        .json(&CreateCharacterRequest {
            name: name.to_string(),
            description: description.map(|value| value.to_string()),
            sex,
            class,
        })
        .send()
        .map_err(|err| format!("Character creation request failed: {err}"))?;

    let status = resp.status();
    if status.is_success() {
        let body: CharacterSummary = resp
            .json()
            .map_err(|err| format!("Failed to parse character creation response: {err}"))?;
        return Ok(body);
    }

    let message = match status {
        StatusCode::BAD_REQUEST => "Invalid character details",
        StatusCode::UNAUTHORIZED => "Unauthorized",
        StatusCode::INTERNAL_SERVER_ERROR => "Server error",
        _ => "Character creation failed",
    };

    Err(format!("{message} ({})", status.as_u16()))
}

/// Retrieves all characters for the authenticated account.
///
/// # Arguments
/// * `base_url` - API base URL.
/// * `token` - JWT bearer token.
///
/// # Returns
/// * `Ok(Vec<CharacterSummary>)` on success.
/// * `Err(String)` when the request fails.
pub fn get_characters(base_url: &str, token: &str) -> Result<Vec<CharacterSummary>, String> {
    let client = Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|err| format!("Failed to build HTTP client: {err}"))?;

    let url = format!("{}/characters", base_url.trim_end_matches('/'));
    let resp = client
        .get(url)
        .bearer_auth(token)
        .send()
        .map_err(|err| format!("Get characters request failed: {err}"))?;

    let status = resp.status();
    if status.is_success() {
        let body: GetCharactersResponse = resp
            .json()
            .map_err(|err| format!("Failed to parse characters response: {err}"))?;
        return Ok(body.characters);
    }

    let message = match status {
        StatusCode::UNAUTHORIZED => "Unauthorized",
        StatusCode::INTERNAL_SERVER_ERROR => "Server error",
        _ => "Get characters failed",
    };

    Err(format!("{message} ({})", status.as_u16()))
}

/// Deletes a character by id for the authenticated account.
///
/// # Arguments
/// * `base_url` - API base URL.
/// * `token` - JWT bearer token.
/// * `character_id` - Character id to delete.
///
/// # Returns
/// * `Ok(())` on success.
/// * `Err(String)` when the request fails.
pub fn delete_character(base_url: &str, token: &str, character_id: u64) -> Result<(), String> {
    let client = Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|err| format!("Failed to build HTTP client: {err}"))?;

    let url = format!(
        "{}/characters/{}",
        base_url.trim_end_matches('/'),
        character_id
    );
    let resp = client
        .delete(url)
        .bearer_auth(token)
        .send()
        .map_err(|err| format!("Delete character request failed: {err}"))?;

    let status = resp.status();
    if status.is_success() {
        return Ok(());
    }

    let message = match status {
        StatusCode::BAD_REQUEST => "Invalid character delete request",
        StatusCode::UNAUTHORIZED => "Unauthorized",
        StatusCode::INTERNAL_SERVER_ERROR => "Server error",
        _ => "Delete character failed",
    };

    Err(format!("{message} ({})", status.as_u16()))
}

/// Creates a short-lived, one-time login ticket for the game server.
///
/// The returned ticket is meant to be sent over the TCP login handshake using `CL_API_LOGIN`.
pub fn create_game_login_ticket(
    base_url: &str,
    token: &str,
    character_id: u64,
) -> Result<u64, String> {
    let client = Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|err| format!("Failed to build HTTP client: {err}"))?;

    let url = format!("{}/game/login_ticket", base_url.trim_end_matches('/'));
    let resp = client
        .post(url)
        .bearer_auth(token)
        .json(&CreateGameLoginTicketRequest { character_id })
        .send()
        .map_err(|err| format!("Create login ticket request failed: {err}"))?;

    let status = resp.status();
    let body: CreateGameLoginTicketResponse = resp
        .json()
        .map_err(|err| format!("Failed to parse create ticket response: {err}"))?;

    if status.is_success() {
        if let Some(ticket) = body.ticket {
            return Ok(ticket);
        }
        return Err("Ticket creation failed: empty ticket".to_string());
    }

    let fallback = match status {
        StatusCode::UNAUTHORIZED => "Unauthorized",
        StatusCode::BAD_REQUEST => "Invalid request",
        StatusCode::INTERNAL_SERVER_ERROR => "Server error",
        _ => "Ticket creation failed",
    };

    Err(body.error.unwrap_or_else(|| fallback.to_string()))
}
