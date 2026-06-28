//! API bootstrap: account creation, login, character provisioning, and ticket minting.
//!
//! Every bot client calls [`bootstrap_client`] to ensure its account and character exist,
//! then calls [`mint_ticket`] just before connecting to get a fresh 30-second one-time ticket.
//!
//! Rate limiting is handled by a shared [`RateLimiter`] that caps API requests at ~25/s
//! to stay safely under the server's per-IP 30 req/s limit.

use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, anyhow};
use argon2::password_hash::SaltString;
use argon2::{Argon2, PasswordHasher};
use mag_core::constants::VERSION;
use mag_core::types::api::{
    CharacterSummary, CreateAccountRequest, CreateCharacterRequest, CreateGameLoginTicketRequest,
    CreateGameLoginTicketResponse, GetCharactersResponse, LoginRequest, LoginResponse,
};
use reqwest::StatusCode;
use tokio::sync::Semaphore;

use crate::config::LoadTestConfig;

// ---------------------------------------------------------------------------
// Rate limiter
// ---------------------------------------------------------------------------

/// Token-bucket rate limiter backed by a [`Semaphore`].
///
/// A background task refills the bucket at a fixed rate.  Each API call
/// acquires one token before proceeding.
pub struct RateLimiter {
    sem: Arc<Semaphore>,
    /// Keep the refill task alive for the lifetime of the limiter.
    _refill_task: tokio::task::JoinHandle<()>,
}

impl RateLimiter {
    /// Creates a new rate limiter allowing at most `per_second` acquisitions per second.
    ///
    /// # Arguments
    ///
    /// * `per_second` - Maximum allowed requests per second.
    ///
    /// # Returns
    ///
    /// * A new [`RateLimiter`] backed by a Tokio semaphore.
    pub fn new(per_second: u64) -> Self {
        let sem = Arc::new(Semaphore::new(0));
        let sem2 = sem.clone();
        let task = tokio::spawn(async move {
            let interval_us = 1_000_000u64 / per_second.max(1);
            let max_burst = (per_second * 2) as usize;
            let mut iv = tokio::time::interval(Duration::from_micros(interval_us));
            loop {
                iv.tick().await;
                if sem2.available_permits() < max_burst {
                    sem2.add_permits(1);
                }
            }
        });
        Self {
            sem,
            _refill_task: task,
        }
    }

    /// Acquires one token, waiting if the bucket is empty.
    ///
    /// Tokens are consumed (not returned) to enforce the rate cap.
    pub async fn acquire(&self) {
        self.sem.acquire().await.unwrap().forget();
    }
}

// ---------------------------------------------------------------------------
// Password hashing (mirrors client/src/account_api.rs hash_password)
// ---------------------------------------------------------------------------

/// Hashes a password into the Argon2 PHC format expected by the API.
///
/// Uses a deterministic salt derived from the username so the same username
/// always produces the same password hash, enabling idempotent account creation.
///
/// # Arguments
///
/// * `username` - Account username (lowercased for the salt).
/// * `password` - Raw password string.
///
/// # Returns
///
/// * `Ok(phc_string)` on success, or an error if hashing fails.
pub fn hash_password(username: &str, password: &str) -> anyhow::Result<String> {
    let username_lc = username.trim().to_lowercase();
    let salt_seed = format!("mag:{username_lc}");
    let salt = SaltString::encode_b64(salt_seed.as_bytes())
        .map_err(|e| anyhow!("salt encode failed: {e}"))?;
    let hash = Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| anyhow!("hash_password failed: {e}"))?
        .to_string();
    Ok(hash)
}

// ---------------------------------------------------------------------------
// Bootstrap
// ---------------------------------------------------------------------------

/// Ensures the bot account and character exist, and returns the JWT + character ID.
///
/// Idempotent: a `409 Conflict` on account creation means the account already exists
/// (reuses it).  The character named `{prefix}-bot-{index}` is reused if present.
///
/// # Arguments
///
/// * `index` - Bot index, used to derive a unique username.
/// * `config` - Shared load-test configuration.
/// * `rate_limiter` - Shared API rate limiter.
///
/// # Returns
///
/// * `Ok((jwt, character_id))` on success.
/// * `Err` if any API call fails fatally.
pub async fn bootstrap_client(
    index: usize,
    config: &LoadTestConfig,
    rate_limiter: &RateLimiter,
) -> anyhow::Result<(String, u64)> {
    let base = config.api.base_url.trim_end_matches('/').to_owned();
    let username = format!("{}-{}", config.accounts.prefix, index);
    let email = format!("{username}@{}", config.accounts.email_domain);
    let password_hash =
        hash_password(&username, &config.accounts.password).context("hash_password")?;

    let http = build_http_client()?;

    // 1. Ensure account exists (create or tolerate 409 Conflict)
    rate_limiter.acquire().await;
    let resp = http
        .post(format!("{base}/accounts"))
        .json(&CreateAccountRequest {
            email: email.clone(),
            username: username.clone(),
            password: password_hash.clone(),
        })
        .send()
        .await
        .context("create account request")?;

    match resp.status() {
        StatusCode::CONFLICT => {} // account already exists, continue
        s if s.is_success() => {}
        s => {
            let body = resp.text().await.unwrap_or_default();
            return Err(anyhow!(
                "account creation failed for {username}: HTTP {s} — {body}"
            ));
        }
    }

    // 2. Login → JWT
    rate_limiter.acquire().await;
    let resp = http
        .post(format!("{base}/login"))
        .json(&LoginRequest {
            username: username.clone(),
            password: password_hash,
        })
        .send()
        .await
        .context("login request")?;

    if !resp.status().is_success() {
        let s = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(anyhow!("login failed for {username}: HTTP {s} — {body}"));
    }

    let login_body: LoginResponse = resp.json().await.context("parse login response")?;
    let jwt = login_body
        .token
        .filter(|t| !t.trim().is_empty())
        .ok_or_else(|| anyhow!("empty JWT for {username}"))?;

    // 3. Get or create character
    rate_limiter.acquire().await;
    let char_resp = http
        .get(format!("{base}/characters"))
        .bearer_auth(&jwt)
        .send()
        .await
        .context("get characters request")?;

    if !char_resp.status().is_success() {
        let s = char_resp.status();
        return Err(anyhow!("get characters failed for {username}: HTTP {s}"));
    }

    let chars: GetCharactersResponse = char_resp.json().await.context("parse characters")?;

    let character_id = if let Some(ch) = chars.characters.first() {
        ch.id
    } else {
        // No characters yet — create one.
        let char_name = format!("{}-bot-{}", config.accounts.prefix, index);
        let created = create_character(&http, &base, &jwt, &char_name, config, rate_limiter)
            .await
            .with_context(|| format!("create character for {username}"))?;
        created.id
    };

    Ok((jwt, character_id))
}

/// Mints a fresh one-time game-login ticket for the given character.
///
/// Must be called just before connecting to stay within the 30-second TTL.
///
/// # Arguments
///
/// * `jwt` - Bearer token from a successful API login.
/// * `character_id` - Character to mint the ticket for.
/// * `config` - Shared load-test configuration.
/// * `rate_limiter` - Shared API rate limiter.
///
/// # Returns
///
/// * `Ok(ticket)` on success.
/// * `Err` if the API call fails.
pub async fn mint_ticket(
    jwt: &str,
    character_id: u64,
    config: &LoadTestConfig,
    rate_limiter: &RateLimiter,
) -> anyhow::Result<u64> {
    let base = config.api.base_url.trim_end_matches('/').to_owned();
    let http = build_http_client()?;

    rate_limiter.acquire().await;
    let resp = http
        .post(format!("{base}/game/login_ticket"))
        .bearer_auth(jwt)
        .json(&CreateGameLoginTicketRequest {
            character_id,
            client_version: VERSION,
        })
        .send()
        .await
        .context("mint ticket request")?;

    if !resp.status().is_success() {
        let s = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(anyhow!("mint ticket failed: HTTP {s} — {body}"));
    }

    let body: CreateGameLoginTicketResponse = resp.json().await.context("parse ticket response")?;
    body.ticket
        .ok_or_else(|| anyhow!("ticket response contained no ticket"))
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Creates a character via the API, honouring the rate limiter.
async fn create_character(
    http: &reqwest::Client,
    base: &str,
    jwt: &str,
    name: &str,
    config: &LoadTestConfig,
    rate_limiter: &RateLimiter,
) -> anyhow::Result<CharacterSummary> {
    rate_limiter.acquire().await;
    let resp = http
        .post(format!("{base}/characters"))
        .bearer_auth(jwt)
        .json(&CreateCharacterRequest {
            name: name.to_owned(),
            description: None,
            sex: config.accounts.sex(),
            class: config.accounts.class(),
        })
        .send()
        .await
        .context("create character request")?;

    if !resp.status().is_success() {
        let s = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(anyhow!("create character failed: HTTP {s} — {body}"));
    }

    resp.json::<CharacterSummary>()
        .await
        .context("parse create character response")
}

/// Builds an async `reqwest::Client` that accepts self-signed certificates.
///
/// Self-signed certs are expected on local / staging API servers.
///
/// # Returns
///
/// * `Ok(client)` on success, `Err` if the builder fails.
fn build_http_client() -> anyhow::Result<reqwest::Client> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .danger_accept_invalid_certs(true)
        .build()
        .context("build http client")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_password_is_deterministic() {
        let h1 = hash_password("Alice", "secret").unwrap();
        let h2 = hash_password("alice", "secret").unwrap();
        assert_eq!(h1, h2, "hash should be case-insensitive on username");

        let h3 = hash_password("Alice", "other").unwrap();
        assert_ne!(h1, h3, "different password must yield different hash");
    }

    #[test]
    fn hash_password_produces_phc_string() {
        let h = hash_password("test", "pw").unwrap();
        assert!(h.starts_with("$argon2"), "expected PHC format");
    }
}
