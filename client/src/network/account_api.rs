use std::env;
use std::time::Duration;

use bevy::prelude::Resource;
use reqwest::blocking::Client;
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Resource)]
pub struct ApiSession {
    pub base_url: String,
    pub token: Option<String>,
    pub username: Option<String>,
    pub pending_notice: Option<String>,
}

impl ApiSession {
    pub fn ensure_defaults(&mut self) {
        if self.base_url.trim().is_empty() {
            self.base_url = default_api_base_url();
        }
    }
}

#[derive(Serialize)]
struct LoginRequest {
    username: String,
    password: String,
}

#[derive(Deserialize)]
struct LoginResponse {
    token: String,
}

#[derive(Serialize)]
struct CreateAccountRequest {
    email: String,
    username: String,
    password: String,
}

#[derive(Deserialize)]
struct CreateAccountResponse {
    id: Option<u64>,
    error: Option<String>,
    username: String,
    email: String,
}

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

pub fn login(base_url: &str, username: &str, password: &str) -> Result<String, String> {
    let client = Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|err| format!("Failed to build HTTP client: {err}"))?;

    let url = format!("{}/login", base_url.trim_end_matches('/'));
    let resp = client
        .post(url)
        .json(&LoginRequest {
            username: username.to_string(),
            password: password.to_string(),
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

pub fn create_account(
    base_url: &str,
    email: &str,
    username: &str,
    password: &str,
) -> Result<String, String> {
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
            password: password.to_string(),
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
