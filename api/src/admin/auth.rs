//! Admin authentication and abuse-protection middleware.
//!
//! Provides a single Axum middleware [`admin_guard`] that:
//! 1. Rejects requests from IPs currently in the failed-auth lockout window.
//! 2. Validates the `Authorization: Bearer <token>` header against the
//!    operator-supplied token using a constant-time compare.
//! 3. Throttles authenticated requests at a fixed per-IP rate.
//!
//! All state lives in process memory and resets on restart.

use axum::extract::{ConnectInfo, Request, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::middleware::Next;
use axum::response::Response;
use std::collections::HashMap;
use std::env;
use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use subtle::ConstantTimeEq;

/// Environment variable that must be set for admin routes to mount.
pub const ADMIN_TOKEN_ENV: &str = "MAG_ADMIN_API_TOKEN";

/// Minimum acceptable token length, in bytes.
const MIN_TOKEN_LEN: usize = 32;

/// Authenticated requests permitted per-IP per second.
const ADMIN_RATE_PER_SECOND: u32 = 8;

/// Failed-auth attempts allowed inside [`FAILURE_WINDOW`] before lockout.
const FAILURE_THRESHOLD: u32 = 5;

/// Sliding window over which failed-auth attempts are counted.
const FAILURE_WINDOW: Duration = Duration::from_secs(60);

/// Lockout duration applied once an IP exceeds [`FAILURE_THRESHOLD`].
const FAILURE_LOCKOUT: Duration = Duration::from_secs(600);

/// Shared admin middleware state.
#[derive(Clone)]
pub struct AdminState {
    inner: Arc<AdminStateInner>,
}

struct AdminStateInner {
    token: String,
    trackers: Mutex<HashMap<IpAddr, IpTracker>>,
}

#[derive(Default)]
struct IpTracker {
    /// Timestamps of recent failed-auth attempts inside [`FAILURE_WINDOW`].
    failures: Vec<Instant>,
    /// When the IP becomes eligible to retry after lockout.
    locked_until: Option<Instant>,
    /// Authenticated request timestamps inside the trailing 1s window.
    request_times: Vec<Instant>,
}

impl AdminState {
    /// Build [`AdminState`] from the process environment.
    ///
    /// # Returns
    ///
    /// * `Some(AdminState)` when [`ADMIN_TOKEN_ENV`] is set to a value of at
    ///   least [`MIN_TOKEN_LEN`] bytes.
    /// * `None` when the variable is missing, empty, or too short. In that
    ///   case admin routes must not be mounted.
    pub fn from_env() -> Option<Self> {
        let token = env::var(ADMIN_TOKEN_ENV).ok()?;
        if token.len() < MIN_TOKEN_LEN {
            log::warn!(
                "{} is set but shorter than {} bytes; admin routes will NOT be mounted",
                ADMIN_TOKEN_ENV,
                MIN_TOKEN_LEN
            );
            return None;
        }

        Some(Self {
            inner: Arc::new(AdminStateInner {
                token,
                trackers: Mutex::new(HashMap::new()),
            }),
        })
    }

    /// Construct an `AdminState` directly from a token, bypassing env lookup.
    ///
    /// # Arguments
    ///
    /// * `token` - The admin bearer token.
    ///
    /// # Returns
    ///
    /// * A new [`AdminState`].
    #[cfg(test)]
    pub fn for_test(token: impl Into<String>) -> Self {
        Self {
            inner: Arc::new(AdminStateInner {
                token: token.into(),
                trackers: Mutex::new(HashMap::new()),
            }),
        }
    }
}

/// Outcome of a guard check, returned to the middleware so it can shape the
/// HTTP response and emit a single log line.
#[derive(Debug, PartialEq, Eq)]
enum GuardOutcome {
    Allow,
    LockedOut { retry_after: Duration },
    RateLimited,
    Unauthorized,
}

fn extract_bearer_token(headers: &HeaderMap) -> Option<&str> {
    let raw = headers.get(header::AUTHORIZATION)?.to_str().ok()?.trim();
    let token = raw.strip_prefix("Bearer ")?.trim();
    if token.is_empty() { None } else { Some(token) }
}

fn evaluate_guard(
    state: &AdminState,
    ip: IpAddr,
    presented: Option<&str>,
    now: Instant,
) -> GuardOutcome {
    let mut trackers = state
        .inner
        .trackers
        .lock()
        .expect("admin trackers poisoned");
    let entry = trackers.entry(ip).or_default();

    // 1. Lockout check.
    if let Some(until) = entry.locked_until {
        if now < until {
            return GuardOutcome::LockedOut {
                retry_after: until.saturating_duration_since(now),
            };
        }
        entry.locked_until = None;
        entry.failures.clear();
    }

    // 2. Token check.
    let token_ok = match presented {
        Some(t) => {
            let expected = state.inner.token.as_bytes();
            let provided = t.as_bytes();
            expected.len() == provided.len() && expected.ct_eq(provided).unwrap_u8() == 1
        }
        None => false,
    };

    if !token_ok {
        entry
            .failures
            .retain(|ts| now.saturating_duration_since(*ts) < FAILURE_WINDOW);
        entry.failures.push(now);
        if entry.failures.len() as u32 >= FAILURE_THRESHOLD {
            entry.locked_until = Some(now + FAILURE_LOCKOUT);
            entry.failures.clear();
            return GuardOutcome::LockedOut {
                retry_after: FAILURE_LOCKOUT,
            };
        }
        return GuardOutcome::Unauthorized;
    }

    // Successful auth resets failure counter.
    entry.failures.clear();

    // 3. Per-IP rate limit (last 1s).
    entry
        .request_times
        .retain(|ts| now.saturating_duration_since(*ts) < Duration::from_secs(1));
    if entry.request_times.len() as u32 >= ADMIN_RATE_PER_SECOND {
        return GuardOutcome::RateLimited;
    }
    entry.request_times.push(now);

    GuardOutcome::Allow
}

/// Axum middleware enforcing admin auth + abuse limits.
///
/// # Arguments
///
/// * `state`   - Shared admin state injected via `from_fn_with_state`.
/// * `connect` - Peer socket address (used as IP key).
/// * `headers` - Incoming request headers (read for `Authorization`).
/// * `request` - The request being guarded.
/// * `next`    - Downstream service.
///
/// # Returns
///
/// * `Response` either from the downstream handler (allowed) or built
///   directly here for `401`/`429` cases.
pub async fn admin_guard(
    State(state): State<AdminState>,
    ConnectInfo(connect): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    request: Request,
    next: Next,
) -> Response {
    let ip = connect.ip();
    let presented = extract_bearer_token(&headers);
    let outcome = evaluate_guard(&state, ip, presented, Instant::now());

    match outcome {
        GuardOutcome::Allow => next.run(request).await,
        GuardOutcome::Unauthorized => {
            log::warn!("admin auth failure from {}", ip);
            let mut resp = Response::new(axum::body::Body::empty());
            *resp.status_mut() = StatusCode::UNAUTHORIZED;
            resp
        }
        GuardOutcome::LockedOut { retry_after } => {
            log::warn!(
                "admin IP {} locked out (retry in {}s)",
                ip,
                retry_after.as_secs()
            );
            let mut resp = Response::new(axum::body::Body::empty());
            *resp.status_mut() = StatusCode::TOO_MANY_REQUESTS;
            if let Ok(value) = retry_after.as_secs().to_string().parse() {
                resp.headers_mut().insert(header::RETRY_AFTER, value);
            }
            resp
        }
        GuardOutcome::RateLimited => {
            log::warn!("admin rate limit exceeded for {}", ip);
            let mut resp = Response::new(axum::body::Body::empty());
            *resp.status_mut() = StatusCode::TOO_MANY_REQUESTS;
            if let Ok(value) = "1".parse() {
                resp.headers_mut().insert(header::RETRY_AFTER, value);
            }
            resp
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ip() -> IpAddr {
        IpAddr::from([127, 0, 0, 1])
    }

    fn state() -> AdminState {
        AdminState::for_test("a".repeat(MIN_TOKEN_LEN))
    }

    #[test]
    fn allows_with_valid_token() {
        let s = state();
        let outcome = evaluate_guard(&s, ip(), Some(&"a".repeat(MIN_TOKEN_LEN)), Instant::now());
        assert_eq!(outcome, GuardOutcome::Allow);
    }

    #[test]
    fn missing_token_is_unauthorized() {
        let s = state();
        let outcome = evaluate_guard(&s, ip(), None, Instant::now());
        assert_eq!(outcome, GuardOutcome::Unauthorized);
    }

    #[test]
    fn wrong_token_is_unauthorized() {
        let s = state();
        let outcome = evaluate_guard(&s, ip(), Some(&"b".repeat(MIN_TOKEN_LEN)), Instant::now());
        assert_eq!(outcome, GuardOutcome::Unauthorized);
    }

    #[test]
    fn lockout_after_threshold_failures() {
        let s = state();
        let now = Instant::now();
        for _ in 0..(FAILURE_THRESHOLD - 1) {
            let outcome = evaluate_guard(&s, ip(), Some("bad"), now);
            assert_eq!(outcome, GuardOutcome::Unauthorized);
        }
        let outcome = evaluate_guard(&s, ip(), Some("bad"), now);
        match outcome {
            GuardOutcome::LockedOut { retry_after } => {
                assert!(retry_after <= FAILURE_LOCKOUT);
            }
            other => panic!("expected lockout, got {:?}", other),
        }
        // Even a correct token while locked is rejected.
        let outcome = evaluate_guard(&s, ip(), Some(&"a".repeat(MIN_TOKEN_LEN)), now);
        assert!(matches!(outcome, GuardOutcome::LockedOut { .. }));
    }

    #[test]
    fn lockout_clears_after_window() {
        let s = state();
        let now = Instant::now();
        for _ in 0..FAILURE_THRESHOLD {
            let _ = evaluate_guard(&s, ip(), Some("bad"), now);
        }
        // Simulate post-lockout time.
        let later = now + FAILURE_LOCKOUT + Duration::from_secs(1);
        let outcome = evaluate_guard(&s, ip(), Some(&"a".repeat(MIN_TOKEN_LEN)), later);
        assert_eq!(outcome, GuardOutcome::Allow);
    }

    #[test]
    fn rate_limit_kicks_in_after_burst() {
        let s = state();
        let now = Instant::now();
        let token = "a".repeat(MIN_TOKEN_LEN);
        for _ in 0..ADMIN_RATE_PER_SECOND {
            assert_eq!(
                evaluate_guard(&s, ip(), Some(&token), now),
                GuardOutcome::Allow
            );
        }
        assert_eq!(
            evaluate_guard(&s, ip(), Some(&token), now),
            GuardOutcome::RateLimited
        );
        // Window slides.
        let later = now + Duration::from_millis(1100);
        assert_eq!(
            evaluate_guard(&s, ip(), Some(&token), later),
            GuardOutcome::Allow
        );
    }

    #[test]
    fn extract_bearer_token_handles_prefix_and_whitespace() {
        let mut headers = HeaderMap::new();
        headers.insert(header::AUTHORIZATION, "Bearer  hello  ".parse().unwrap());
        assert_eq!(extract_bearer_token(&headers), Some("hello"));

        let mut headers = HeaderMap::new();
        headers.insert(header::AUTHORIZATION, "Basic xyz".parse().unwrap());
        assert_eq!(extract_bearer_token(&headers), None);

        let headers = HeaderMap::new();
        assert_eq!(extract_bearer_token(&headers), None);
    }

    #[test]
    fn from_env_validates_token_length() {
        // Combined to avoid races between parallel tests sharing the same
        // process env var.
        unsafe {
            env::set_var(ADMIN_TOKEN_ENV, "tooshort");
        }
        let short = AdminState::from_env();
        assert!(short.is_none());

        let long_token = "x".repeat(MIN_TOKEN_LEN);
        unsafe {
            env::set_var(ADMIN_TOKEN_ENV, &long_token);
        }
        let long = AdminState::from_env();
        unsafe {
            env::remove_var(ADMIN_TOKEN_ENV);
        }
        assert!(long.is_some());
    }
}
