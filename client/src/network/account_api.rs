use std::env;
use std::time::Duration;

use argon2::password_hash::SaltString;
use argon2::{Argon2, PasswordHasher};
use bevy::prelude::Resource;
use reqwest::blocking::Client;
use reqwest::StatusCode;

use mag_core::types::api::{
    CreateAccountRequest, CreateAccountResponse, LoginRequest, LoginResponse,
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
