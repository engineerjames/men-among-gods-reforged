//! Per-IP rate limiting and login lockout backed by KeyDB.
//!
//! Replaces the previous in-process `axum-governor` / `lazy-limit` setup with
//! a shared KeyDB-backed implementation that:
//!
//! * is correct under horizontal scale-out (counters live in KeyDB, not the
//!   process);
//! * keys on the real client IP from `ConnectInfo<SocketAddr>` (already
//!   populated upstream of all public routes by `real::RealIpLayer`);
//! * exposes a dedicated login-failure counter + temporary lockout, so brute
//!   force attempts against `/login` do not just slow down — they get cut off.
use std::net::{IpAddr, SocketAddr};

use axum::{
    extract::{ConnectInfo, Request, State},
    http::{HeaderValue, StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Response},
};
use log::{error, warn};
use redis::AsyncCommands;

use crate::ApiState;

/// Per-IP sliding burst: requests allowed in any 1 second window.
const PUBLIC_RATE_PER_SECOND: u32 = 30;

/// Maximum number of failed login attempts allowed per IP within the
/// observation window before the IP is locked out.
const LOGIN_FAILURE_THRESHOLD: u32 = 10;

/// Sliding window (seconds) over which `LOGIN_FAILURE_THRESHOLD` failures
/// accumulate to trigger a lockout.
const LOGIN_FAILURE_WINDOW_SECS: u64 = 900;

/// Duration (seconds) of the lockout that fires once the threshold is hit.
const LOGIN_LOCKOUT_SECS: u64 = 600;

/// Returns the KeyDB key tracking total requests per IP for the current
/// 1-second bucket.
fn public_rate_key(ip: IpAddr) -> String {
    format!("rate:public:{ip}")
}

/// Returns the KeyDB key tracking consecutive failed login attempts per IP.
fn login_failure_key(ip: IpAddr) -> String {
    format!("login_attempts:{ip}")
}

/// Returns the KeyDB key marking an active login lockout per IP.
fn login_lockout_key(ip: IpAddr) -> String {
    format!("login_lockout:{ip}")
}

/// Builds a `429 Too Many Requests` response with a `Retry-After` header.
fn rate_limited_response(retry_after_secs: u64) -> Response {
    let mut response = (StatusCode::TOO_MANY_REQUESTS, "rate limited").into_response();
    if let Ok(value) = HeaderValue::from_str(&retry_after_secs.to_string()) {
        response.headers_mut().insert(header::RETRY_AFTER, value);
    }
    response
}

/// Axum middleware enforcing a per-IP throughput cap on public routes.
///
/// Fails open: if KeyDB is unreachable the request is allowed through. This
/// matches the priority of keeping the public API available even when the
/// counter backend is degraded; raw correctness of the limit is recovered as
/// soon as KeyDB comes back.
///
/// # Arguments
///
/// * `state` - Shared API state (provides the KeyDB connection).
/// * `connect_info` - Peer socket info (real IP is populated upstream).
/// * `request` - Incoming HTTP request.
/// * `next` - Downstream service.
///
/// # Returns
///
/// * The downstream response when allowed.
/// * `429 Too Many Requests` (with `Retry-After: 1`) when the per-IP cap is
///   exceeded.
pub(crate) async fn per_ip_rate_limit(
    State(state): State<ApiState>,
    ConnectInfo(connect): ConnectInfo<SocketAddr>,
    request: Request,
    next: Next,
) -> Response {
    let ip = connect.ip();
    let mut con = state.con.clone();
    let key = public_rate_key(ip);

    let count: redis::RedisResult<u32> = con.incr(&key, 1_i64).await;
    let count = match count {
        Ok(value) => value,
        Err(err) => {
            warn!("rate-limit INCR failed for {ip}: {err}");
            return next.run(request).await;
        }
    };

    if count == 1
        && let Err(err) = con.expire::<_, ()>(&key, 1).await
    {
        warn!("rate-limit EXPIRE failed for {ip}: {err}");
    }

    if count > PUBLIC_RATE_PER_SECOND {
        warn!("per-IP rate limit exceeded for {ip} (count={count})");
        return rate_limited_response(1);
    }

    next.run(request).await
}

/// Outcome of checking whether an IP is currently allowed to attempt login.
pub(crate) enum LoginGateOutcome {
    /// Caller may proceed with credential verification.
    Allow,
    /// Caller must reject the request with the included `Retry-After` value.
    LockedOut { retry_after_secs: u64 },
}

/// Checks whether an IP is currently locked out from `/login`.
///
/// # Arguments
///
/// * `con` - KeyDB connection.
/// * `ip` - Client IP.
///
/// # Returns
///
/// * `LoginGateOutcome::Allow` when the IP is below the failure threshold.
/// * `LoginGateOutcome::LockedOut` while the lockout key is present.
pub(crate) async fn check_login_lockout(
    con: &mut redis::aio::ConnectionManager,
    ip: IpAddr,
) -> LoginGateOutcome {
    let key = login_lockout_key(ip);

    // EXISTS + TTL pair. On any KeyDB error we fail open (treated as allow) so
    // a degraded counter does not lock everyone out of the world.
    let exists: redis::RedisResult<bool> = con.exists(&key).await;
    match exists {
        Ok(true) => {
            let ttl: redis::RedisResult<i64> = con.ttl(&key).await;
            let retry_after_secs = match ttl {
                Ok(secs) if secs > 0 => secs as u64,
                _ => LOGIN_LOCKOUT_SECS,
            };
            LoginGateOutcome::LockedOut { retry_after_secs }
        }
        Ok(false) => LoginGateOutcome::Allow,
        Err(err) => {
            warn!("login-lockout EXISTS failed for {ip}: {err}");
            LoginGateOutcome::Allow
        }
    }
}

/// Records a failed login attempt and possibly engages the lockout.
///
/// # Arguments
///
/// * `con` - KeyDB connection.
/// * `ip` - Client IP that just submitted a bad credential.
pub(crate) async fn record_login_failure(con: &mut redis::aio::ConnectionManager, ip: IpAddr) {
    let counter_key = login_failure_key(ip);

    let count: redis::RedisResult<u32> = con.incr(&counter_key, 1_i64).await;
    let count = match count {
        Ok(value) => value,
        Err(err) => {
            error!("login-failure INCR failed for {ip}: {err}");
            return;
        }
    };

    if count == 1
        && let Err(err) = con
            .expire::<_, ()>(&counter_key, LOGIN_FAILURE_WINDOW_SECS as i64)
            .await
    {
        warn!("login-failure EXPIRE failed for {ip}: {err}");
    }

    if count >= LOGIN_FAILURE_THRESHOLD {
        let lockout_key = login_lockout_key(ip);
        if let Err(err) = con
            .set_ex::<_, _, ()>(&lockout_key, 1_u8, LOGIN_LOCKOUT_SECS)
            .await
        {
            error!("login-lockout SET failed for {ip}: {err}");
            return;
        }
        warn!(
            "login lockout engaged for {ip} after {count} failures (duration={}s)",
            LOGIN_LOCKOUT_SECS
        );
    }
}

/// Clears the per-IP failed login counter after a successful authentication.
///
/// # Arguments
///
/// * `con` - KeyDB connection.
/// * `ip` - Client IP that just succeeded.
pub(crate) async fn clear_login_failures(con: &mut redis::aio::ConnectionManager, ip: IpAddr) {
    let key = login_failure_key(ip);
    if let Err(err) = con.del::<_, ()>(&key).await {
        warn!("login-failure DEL failed for {ip}: {err}");
    }
}

/// Renders a lockout response.
pub(crate) fn login_locked_out_response(retry_after_secs: u64) -> Response {
    rate_limited_response(retry_after_secs)
}
