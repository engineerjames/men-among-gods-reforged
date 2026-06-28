//! API bootstrap: account creation, login, character provisioning, and ticket minting.
//!
//! Every bot client calls [`bootstrap_client`] to ensure its account and character exist,
//! then calls [`mint_ticket`] just before connecting to get a fresh 30-second one-time ticket.
//!
//! Rate limiting is handled by a shared [`RateLimiter`] that caps API requests at ~25/s
//! to stay safely under the server's per-IP 30 req/s limit.

use std::time::{Duration, Instant};

use anyhow::{Context, anyhow};
use argon2::password_hash::SaltString;
use argon2::{Argon2, PasswordHasher};
use mag_core::constants::VERSION;
use mag_core::types::api::{
    CharacterSummary, CreateAccountRequest, CreateCharacterRequest, CreateGameLoginTicketRequest,
    CreateGameLoginTicketResponse, GetCharactersResponse, LoginRequest, LoginResponse,
};
use reqwest::StatusCode;
use tokio::sync::Mutex;

use crate::config::LoadTestConfig;

// ---------------------------------------------------------------------------
// Rate limiter
// ---------------------------------------------------------------------------

/// Strict-spacing rate limiter for account API calls.
///
/// Unlike a token bucket, this limiter cannot accumulate burst capacity while
/// clients are busy hashing passwords or waiting on network I/O. Every acquire
/// reserves one send slot separated by `spacing` from the previous slot.
pub struct RateLimiter {
    next_allowed: Mutex<Instant>,
    spacing: Duration,
    send_lock: Mutex<()>,
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
    /// * A new [`RateLimiter`] that serializes request starts.
    pub fn new(per_second: u64) -> Self {
        Self {
            next_allowed: Mutex::new(Instant::now()),
            spacing: Duration::from_micros(1_000_000u64 / per_second.max(1)),
            send_lock: Mutex::new(()),
        }
    }

    /// Reserves the next request slot, waiting until that slot begins.
    ///
    /// Calls are serialized so no burst can form, even if many client tasks
    /// wake up at the same time.
    pub async fn acquire(&self) {
        let sleep_for = {
            let mut next_allowed = self.next_allowed.lock().await;
            let now = Instant::now();
            let slot = (*next_allowed).max(now);
            *next_allowed = slot + self.spacing;
            slot.saturating_duration_since(now)
        };

        if !sleep_for.is_zero() {
            tokio::time::sleep(sleep_for).await;
        }
    }

    /// Pushes the next allowable request slot into the future.
    ///
    /// Used when the API returns `429 Too Many Requests`, whose rate-limit
    /// counter is shared by every simulated client because they come from the
    /// same source IP.
    ///
    /// # Arguments
    ///
    /// * `duration` - Minimum shared cooldown before any future API request.
    pub async fn cooldown(&self, duration: Duration) {
        let mut next_allowed = self.next_allowed.lock().await;
        let cooldown_until = Instant::now() + duration;
        if *next_allowed < cooldown_until {
            *next_allowed = cooldown_until;
        }
    }

    /// Executes a single API request under this limiter's global send lock.
    ///
    /// Only one request is in flight at a time.  That is intentionally
    /// conservative: the API's public limit is keyed only by source IP, so all
    /// simulated clients share the same bucket.
    ///
    /// # Arguments
    ///
    /// * `builder` - Request builder to send.
    ///
    /// # Returns
    ///
    /// * `Ok(Response)` for the request response.
    /// * `Err` for network or TLS failures.
    pub async fn send(
        &self,
        builder: reqwest::RequestBuilder,
    ) -> anyhow::Result<reqwest::Response> {
        let _guard = self.send_lock.lock().await;
        self.acquire().await;
        builder.send().await.context("HTTP send")
    }
}

// ---------------------------------------------------------------------------
// API send helper with 429 retry
// ---------------------------------------------------------------------------

/// Sends `builder` through the shared API limiter, retrying on HTTP 429.
///
/// On a 429 the `Retry-After` response header is honoured; a fresh token is
/// re-acquired before each retry so aggregate throughput stays within budget.
/// A 429 is treated as backpressure, not a fatal bootstrap error.
///
/// # Arguments
///
/// * `rate_limiter` - Shared strict-spacing rate limiter.
/// * `builder` - Pre-configured `reqwest::RequestBuilder` (must be cloneable
///   via [`reqwest::RequestBuilder::try_clone`]).
///
/// # Returns
///
/// * `Ok(Response)` — the final non-429 response.
/// * `Err` on network or TLS failures.
async fn api_send(
    rate_limiter: &RateLimiter,
    builder: reqwest::RequestBuilder,
) -> anyhow::Result<reqwest::Response> {
    let mut attempt = 0u32;
    loop {
        let current = builder
            .try_clone()
            .ok_or_else(|| anyhow!("request cannot be cloned for 429 retry"))?;
        let resp = rate_limiter.send(current).await?;

        if resp.status() != StatusCode::TOO_MANY_REQUESTS {
            return Ok(resp);
        }

        attempt = attempt.saturating_add(1);
        // A 429 means the shared source-IP counter is already hot. Add a
        // cushion beyond Retry-After so all waiting clients cool down together
        // instead of immediately entering the next fixed 1-second bucket.
        let wait_ms = retry_after_ms(&resp) + 2_000;
        rate_limiter.cooldown(Duration::from_millis(wait_ms)).await;
        log::warn!("HTTP 429, retrying in {wait_ms}ms (attempt {})", attempt);
        tokio::time::sleep(Duration::from_millis(wait_ms)).await;
    }
}

/// Extracts the retry delay from a 429 `Retry-After` header in milliseconds.
///
/// Defaults to 5 000 ms when the header is absent or unparseable.
fn retry_after_ms(resp: &reqwest::Response) -> u64 {
    resp.headers()
        .get(reqwest::header::RETRY_AFTER)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok())
        .map(|secs| secs * 1000 + 100)
        .unwrap_or(5000)
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
    // Run Argon2 on a blocking thread so it cannot starve the tokio runtime.
    // Argon2 with default parameters takes several seconds in a debug build.
    let username_for_hash = username.clone();
    let password_for_hash = config.accounts.password.clone();
    let password_hash =
        tokio::task::spawn_blocking(move || hash_password(&username_for_hash, &password_for_hash))
            .await
            .context("spawn_blocking hash_password")?
            .context("hash_password")?;

    let http = build_http_client()?;

    // 1. Ensure account exists (create or tolerate 409 Conflict)
    let resp = api_send(
        rate_limiter,
        http.post(format!("{base}/accounts"))
            .json(&CreateAccountRequest {
                email: email.clone(),
                username: username.clone(),
                password: password_hash.clone(),
            }),
    )
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
    let resp = api_send(
        rate_limiter,
        http.post(format!("{base}/login")).json(&LoginRequest {
            username: username.clone(),
            password: password_hash,
        }),
    )
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
    let char_resp = api_send(
        rate_limiter,
        http.get(format!("{base}/characters")).bearer_auth(&jwt),
    )
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
        let char_name = bot_char_name(&config.accounts.prefix, index);
        let created = create_character(&http, &base, &jwt, &char_name, config, rate_limiter)
            .await
            .with_context(|| format!("create character '{char_name}' (account: {username})"))?;
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

    let resp = api_send(
        rate_limiter,
        http.post(format!("{base}/game/login_ticket"))
            .bearer_auth(jwt)
            .json(&CreateGameLoginTicketRequest {
                character_id,
                client_version: VERSION,
            }),
    )
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
    let resp = api_send(
        rate_limiter,
        http.post(format!("{base}/characters"))
            .bearer_auth(jwt)
            .json(&CreateCharacterRequest {
                name: name.to_owned(),
                description: None,
                sex: config.accounts.sex(),
                class: config.accounts.class(),
            }),
    )
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

/// Generates an API-valid character name for bot `index`.
///
/// The API requires ASCII letters only, length 4–15.  Strategy: strip
/// non-alphabetic characters from `prefix`, then append a base-26 letter
/// suffix encoding `index` (`a`=0, `b`=1, …, `z`=25, `aa`=26, …).  The
/// prefix is truncated so the combined name stays within 15 characters.
///
/// # Arguments
///
/// * `prefix` - Account prefix from config (non-alpha chars are stripped).
/// * `index` - Bot index used to derive a unique name suffix.
///
/// # Returns
///
/// * A unique, API-valid character name string.
pub(crate) fn bot_char_name(prefix: &str, index: usize) -> String {
    let suffix = index_to_alpha(index);
    // Keep only alpha chars from the prefix.
    let raw_prefix: String = prefix.chars().filter(|c| c.is_ascii_alphabetic()).collect();
    // Budget: combined name must fit in 15 chars.
    let prefix_budget = 15usize.saturating_sub(suffix.len());
    let trimmed_prefix: String = raw_prefix.chars().take(prefix_budget).collect();
    let combined = format!("{trimmed_prefix}{suffix}");
    // Pad with 'a' to hit the 4-char minimum if necessary.
    if combined.len() < 4 {
        let pad_len = 4 - combined.len();
        format!("{combined}{}", "a".repeat(pad_len))
    } else {
        combined
    }
}

/// Encodes a non-negative integer as a base-26 lowercase letter sequence.
///
/// 0 → "a", 25 → "z", 26 → "aa", 51 → "az", 52 → "ba", and so on.
///
/// # Arguments
///
/// * `n` - Non-negative integer to encode.
///
/// # Returns
///
/// * A non-empty lowercase ASCII string.
fn index_to_alpha(mut n: usize) -> String {
    let mut bytes = Vec::new();
    loop {
        bytes.push(b'a' + (n % 26) as u8);
        if n < 26 {
            break;
        }
        n = n / 26 - 1;
    }
    bytes.iter().rev().map(|&b| b as char).collect()
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
    fn index_to_alpha_base26() {
        assert_eq!(index_to_alpha(0), "a");
        assert_eq!(index_to_alpha(25), "z");
        assert_eq!(index_to_alpha(26), "aa");
        assert_eq!(index_to_alpha(27), "ab");
        assert_eq!(index_to_alpha(51), "az");
        assert_eq!(index_to_alpha(52), "ba");
        assert_eq!(index_to_alpha(701), "zz");
        assert_eq!(index_to_alpha(702), "aaa");
    }

    #[test]
    fn bot_char_name_ascii_letters_only() {
        for i in 0..100 {
            let name = bot_char_name("loadtest", i);
            assert!(
                name.chars().all(|c| c.is_ascii_alphabetic()),
                "name '{name}' at index {i} contains non-alpha chars"
            );
            assert!(
                name.len() >= 4 && name.len() <= 15,
                "name '{name}' at index {i} has invalid length {}",
                name.len()
            );
        }
    }

    #[test]
    fn bot_char_name_unique_per_index() {
        let names: Vec<_> = (0..50).map(|i| bot_char_name("loadtest", i)).collect();
        let unique: std::collections::HashSet<_> = names.iter().collect();
        assert_eq!(names.len(), unique.len(), "duplicate names detected");
    }

    #[test]
    fn bot_char_name_strips_non_alpha_prefix() {
        let name = bot_char_name("load-test-99", 0);
        assert!(name.chars().all(|c| c.is_ascii_alphabetic()));
    }

    #[test]
    fn bot_char_name_pads_short_prefix() {
        // Empty prefix: result must still be ≥4 chars.
        let name = bot_char_name("", 0); // suffix = "a", pad to 4 → "aaaa"
        assert!(name.len() >= 4);
        assert!(name.chars().all(|c| c.is_ascii_alphabetic()));
    }

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
