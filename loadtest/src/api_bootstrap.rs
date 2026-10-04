//! API bootstrap: account creation, login, character provisioning, and ticket minting.
//!
//! Every bot client calls [`bootstrap_client`] to ensure its account and character exist,
//! then calls [`mint_ticket`] just before connecting to get a fresh 30-second one-time ticket.
//! Both take a shared `reqwest::Client` (built once via [`build_http_client`] and reused by
//! every bot task) rather than building their own — a fresh `Client` per call creates its own
//! connection pool/TLS context and can exhaust file descriptors under heavy concurrency.
//!
//! Rate limiting is handled by a shared [`RateLimiter`] that caps API requests at ~25/s
//! to stay safely under the server's per-IP 30 req/s limit.

use std::time::{Duration, Instant};

use anyhow::{Context, anyhow};
use argon2::password_hash::SaltString;
use argon2::{Argon2, PasswordHasher};
use mag_core::constants::VERSION;
use mag_core::types::api::{
    CharacterSummary, Class, CreateAccountRequest, CreateCharacterRequest,
    CreateGameLoginTicketRequest, CreateGameLoginTicketResponse, GetCharactersResponse,
    LoginRequest, LoginResponse, Sex,
};
use reqwest::StatusCode;
use tokio::sync::{Mutex, Semaphore};

use crate::config::LoadTestConfig;

/// Identity of the character a bot plays, as reported by the account API.
///
/// The class and sex come from the API record (not the config), so a
/// character created by an earlier run with a different `accounts.class`
/// setting is still tracked with its real class. This is what downstream
/// behaviour logic (spell selection, future behaviour trees) keys off.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BotCharacter {
    /// API character ID used for ticket minting.
    pub id: u64,
    /// In-game character name.
    pub name: String,
    /// Starting class/race of the character.
    pub class: Class,
    /// Character sex.
    pub sex: Sex,
}

impl From<CharacterSummary> for BotCharacter {
    fn from(summary: CharacterSummary) -> Self {
        Self {
            id: summary.id,
            name: summary.name,
            class: summary.class,
            sex: summary.sex,
        }
    }
}

/// Maximum number of distinct candidate names to try when the server rejects
/// a generated character name (HTTP 400 — e.g. it contains a banned
/// substring loaded from `game:badnames`). Generated names are built by
/// concatenating a prefix and a base-26 suffix (see [`bot_char_name`]), and
/// the resulting text spanning that boundary can accidentally spell a banned
/// word (e.g. prefix `"...test"` + suffix `"he"` → `"...testhe"`, which
/// contains "the"), even though neither piece is banned on its own.
const MAX_CHARACTER_NAME_ATTEMPTS: u32 = 8;

/// Large odd offset used to derive a very different candidate name per retry
/// attempt (see [`character_name_candidate`]), so a rejected name's
/// prefix/suffix boundary doesn't just shift by one letter and hit the same
/// (or another) banned substring again.
const BOT_NAME_RETRY_STRIDE: usize = 104_729;

// ---------------------------------------------------------------------------
// Rate limiter
// ---------------------------------------------------------------------------

/// Strict-spacing rate limiter for account API calls.
///
/// Unlike a token bucket, this limiter cannot accumulate burst capacity while
/// clients are busy hashing passwords or waiting on network I/O. Every acquire
/// reserves one send slot separated by `spacing` from the previous slot.
///
/// `in_flight` additionally caps how many requests may be outstanding at once.
/// It defaults to 1, which makes effective throughput `1 / request_latency` —
/// a hard ceiling of well under 1 req/s once server-side Argon2 hashing is in
/// the path. Raise it for load generation against a throwaway environment.
pub struct RateLimiter {
    next_allowed: Mutex<Instant>,
    spacing: Duration,
    in_flight: Semaphore,
}

impl RateLimiter {
    /// Creates a rate limiter with a request spacing and a concurrency cap.
    ///
    /// # Arguments
    ///
    /// * `per_second` - Maximum allowed request starts per second.
    /// * `max_in_flight` - Maximum simultaneously outstanding requests (min 1).
    ///
    /// # Returns
    ///
    /// * A new [`RateLimiter`] that paces and caps request starts.
    pub fn new(per_second: u64, max_in_flight: usize) -> Self {
        Self {
            next_allowed: Mutex::new(Instant::now()),
            spacing: Duration::from_micros(1_000_000u64 / per_second.max(1)),
            in_flight: Semaphore::new(max_in_flight.max(1)),
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

    /// Executes a single API request under this limiter's concurrency cap.
    ///
    /// At most `max_in_flight` requests are outstanding at a time. The default
    /// of 1 is intentionally conservative: the API's public limit is keyed only
    /// by source IP, so all simulated clients share the same bucket.
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
        let _permit = self
            .in_flight
            .acquire()
            .await
            .context("rate limiter semaphore closed")?;
        self.acquire().await;
        builder.send().await.context("HTTP send")
    }

    /// Like [`RateLimiter::send`] but skips the in-flight queue.
    ///
    /// The in-flight semaphore is FIFO, so a login-ticket mint made while the
    /// login gate is held would otherwise wait behind every pending bootstrap
    /// request from all other bots, serializing logins at one per queue round
    /// trip. Request-start spacing still applies.
    ///
    /// # Arguments
    ///
    /// * `builder` - Request builder to send.
    ///
    /// # Returns
    ///
    /// * `Ok(Response)` for the request response.
    /// * `Err` for network or TLS failures.
    pub async fn send_priority(
        &self,
        builder: reqwest::RequestBuilder,
    ) -> anyhow::Result<reqwest::Response> {
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
    api_send_with(rate_limiter, builder, false).await
}

/// Same as [`api_send`], optionally bypassing the in-flight queue.
///
/// # Arguments
///
/// * `rate_limiter` - Shared strict-spacing rate limiter.
/// * `builder` - Cloneable request builder.
/// * `priority` - When true, uses [`RateLimiter::send_priority`].
///
/// # Returns
///
/// * `Ok(Response)` — the final non-429 response.
/// * `Err` on network or TLS failures.
async fn api_send_with(
    rate_limiter: &RateLimiter,
    builder: reqwest::RequestBuilder,
    priority: bool,
) -> anyhow::Result<reqwest::Response> {
    let mut attempt = 0u32;
    loop {
        let current = builder
            .try_clone()
            .ok_or_else(|| anyhow!("request cannot be cloned for 429 retry"))?;
        let resp = if priority {
            rate_limiter.send_priority(current).await?
        } else {
            rate_limiter.send(current).await?
        };

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

/// Ensures the bot account and character exist, and returns the JWT + character identity.
///
/// Idempotent: a `409 Conflict` on account creation means the account already exists
/// (reuses it).  The account's first existing character is reused if present;
/// otherwise one is created with the class resolved by
/// [`AccountConfig::class_for`](crate::config::AccountConfig::class_for).
///
/// # Arguments
///
/// * `index` - Bot index, used to derive a unique username.
/// * `config` - Shared load-test configuration.
/// * `http` - Shared HTTP client (one per process, reused across all bots).
/// * `rate_limiter` - Shared API rate limiter.
///
/// # Returns
///
/// * `Ok((jwt, character))` on success.
/// * `Err` if any API call fails fatally.
pub async fn bootstrap_client(
    index: usize,
    config: &LoadTestConfig,
    http: &reqwest::Client,
    rate_limiter: &RateLimiter,
) -> anyhow::Result<(String, BotCharacter)> {
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

    let character = if let Some(ch) = chars.characters.into_iter().next() {
        BotCharacter::from(ch)
    } else {
        // No characters yet — create one. The server's name filter (banned
        // substrings, reserved words, etc.) isn't known to this tool and can
        // reject an otherwise well-formed generated name — e.g. the prefix and
        // suffix can spell a banned word only where they join. Retry with a
        // different deterministic candidate on a name-rejection (HTTP 400)
        // before giving up; any other failure is still treated as fatal.
        let class = config.accounts.class_for(index);
        let mut created = None;
        let mut last_candidate = String::new();
        for attempt in 0..MAX_CHARACTER_NAME_ATTEMPTS {
            let candidate = character_name_candidate(&config.accounts.prefix, index, attempt);
            last_candidate.clone_from(&candidate);
            let outcome =
                create_character(http, &base, &jwt, &candidate, class, config, rate_limiter)
                    .await
                    .with_context(|| {
                        format!("create character '{candidate}' (account: {username})")
                    })?;
            match outcome {
                CreateCharacterOutcome::Created(summary) => {
                    created = Some(summary);
                    break;
                }
                CreateCharacterOutcome::NameRejected => {
                    log::debug!(
                        "Client {index}: candidate character name '{candidate}' rejected by server name filter, trying another"
                    );
                }
            }
        }
        let created = created.ok_or_else(|| {
            anyhow!(
                "no acceptable character name found for {username} after {MAX_CHARACTER_NAME_ATTEMPTS} attempts (last tried: '{last_candidate}')"
            )
        })?;
        BotCharacter::from(created)
    };

    Ok((jwt, character))
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
/// * `http` - Shared HTTP client (one per process, reused across all bots).
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
    http: &reqwest::Client,
    rate_limiter: &RateLimiter,
) -> anyhow::Result<u64> {
    let base = config.api.base_url.trim_end_matches('/').to_owned();

    let resp = api_send_with(
        rate_limiter,
        http.post(format!("{base}/game/login_ticket"))
            .bearer_auth(jwt)
            .json(&CreateGameLoginTicketRequest {
                character_id,
                client_version: VERSION,
            }),
        true,
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

/// Outcome of a single character-creation attempt against the API.
enum CreateCharacterOutcome {
    /// The character was created successfully.
    Created(CharacterSummary),
    /// The server rejected the requested name (`HTTP 400` from name
    /// validation — e.g. a banned substring, reserved word, or bad
    /// length/charset). Retryable with a different candidate name.
    NameRejected,
}

/// Creates a character via the API, honouring the rate limiter.
///
/// # Arguments
///
/// * `http` - Shared HTTP client.
/// * `base` - API base URL without a trailing slash.
/// * `jwt` - Bearer token for the owning account.
/// * `name` - Requested character name.
/// * `class` - Starting class to request.
/// * `config` - Shared load-test configuration (sex).
/// * `rate_limiter` - Shared API rate limiter.
///
/// # Returns
///
/// * `Ok(CreateCharacterOutcome::Created(_))` on success.
/// * `Ok(CreateCharacterOutcome::NameRejected)` when the server rejects `name`
///   specifically (`HTTP 400`) — retryable with a different name.
/// * `Err` for any other failure (network, auth, server error, etc.).
#[allow(clippy::too_many_arguments)]
async fn create_character(
    http: &reqwest::Client,
    base: &str,
    jwt: &str,
    name: &str,
    class: Class,
    config: &LoadTestConfig,
    rate_limiter: &RateLimiter,
) -> anyhow::Result<CreateCharacterOutcome> {
    let resp = api_send(
        rate_limiter,
        http.post(format!("{base}/characters"))
            .bearer_auth(jwt)
            .json(&CreateCharacterRequest {
                name: name.to_owned(),
                description: None,
                sex: config.accounts.sex(),
                class,
            }),
    )
    .await
    .context("create character request")?;

    if resp.status() == StatusCode::BAD_REQUEST {
        return Ok(CreateCharacterOutcome::NameRejected);
    }

    if !resp.status().is_success() {
        let s = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(anyhow!("create character failed: HTTP {s} — {body}"));
    }

    let summary = resp
        .json::<CharacterSummary>()
        .await
        .context("parse create character response")?;
    Ok(CreateCharacterOutcome::Created(summary))
}

/// Generates the `attempt`-th API-valid character name candidate for bot `index`.
///
/// `attempt == 0` is identical to [`bot_char_name`]. Later attempts derive a
/// very different suffix by offsetting `index` by `attempt * BOT_NAME_RETRY_STRIDE`
/// before encoding, so a name rejected for spelling a banned substring at the
/// prefix/suffix boundary is unlikely to do so again on retry.
///
/// # Arguments
///
/// * `prefix` - Account prefix from config (non-alpha chars are stripped).
/// * `index` - Bot index used to derive a unique name suffix.
/// * `attempt` - Retry attempt number (`0` for the first try).
///
/// # Returns
///
/// * A unique, API-valid character name candidate string.
fn character_name_candidate(prefix: &str, index: usize, attempt: u32) -> String {
    let offset_index = index.saturating_add(attempt as usize * BOT_NAME_RETRY_STRIDE);
    bot_char_name(prefix, offset_index)
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
/// Self-signed certs are expected on local / staging API servers. Intended to
/// be called **once** per process — the returned client should be shared
/// (cloned, which is cheap: `reqwest::Client` is internally `Arc`-backed)
/// across every bot task rather than rebuilt per request. Each `Client`
/// carries its own connection pool and TLS context, so building one per bot
/// (or per API call) multiplies file-descriptor usage and can exhaust the
/// process's open-file limit under heavy concurrency.
///
/// # Returns
///
/// * `Ok(client)` on success, `Err` if the builder fails.
pub fn build_http_client() -> anyhow::Result<reqwest::Client> {
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
    fn character_name_candidate_attempt_zero_matches_bot_char_name() {
        for i in 0..20 {
            assert_eq!(
                character_name_candidate("loadtest", i, 0),
                bot_char_name("loadtest", i)
            );
        }
    }

    #[test]
    fn character_name_candidate_varies_by_attempt() {
        // Reproduces the reported collision: index 212 with prefix "loadtest"
        // generates "loadtesthe", which contains the banned substring "the"
        // at the prefix/suffix boundary. Retrying with a different attempt
        // must produce a different candidate name.
        let base = bot_char_name("loadtest", 212);
        assert_eq!(base, "loadtesthe");
        let candidates: Vec<String> = (0..MAX_CHARACTER_NAME_ATTEMPTS)
            .map(|attempt| character_name_candidate("loadtest", 212, attempt))
            .collect();
        assert_eq!(candidates[0], base);
        let unique: std::collections::HashSet<_> = candidates.iter().collect();
        assert_eq!(
            unique.len(),
            candidates.len(),
            "retry attempts must produce distinct candidates: {candidates:?}"
        );
    }

    #[test]
    fn character_name_candidate_stays_valid() {
        for attempt in 0..MAX_CHARACTER_NAME_ATTEMPTS {
            let name = character_name_candidate("loadtest", 42, attempt);
            assert!(
                name.chars().all(|c| c.is_ascii_alphabetic()),
                "candidate '{name}' at attempt {attempt} contains non-alpha chars"
            );
            assert!(
                name.len() >= 4 && name.len() <= 15,
                "candidate '{name}' at attempt {attempt} has invalid length {}",
                name.len()
            );
        }
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
