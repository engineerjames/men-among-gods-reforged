//! Serializes game-server logins to avoid overcrowding the spawn area.
//!
//! When many bot clients become ready to log in at nearly the same time
//! (e.g. after a fast ramp-up or a burst of API bootstrap completions), they
//! can flood the server's initial spawn point faster than already-spawned
//! characters can walk out of the way, which makes the server unable to
//! place newly logging-in characters at all. [`LoginGate`] enforces a
//! minimum spacing between successive game-server logins so recently spawned
//! characters have time to move away before the next one arrives.

use std::time::{Duration, Instant};

use tokio::sync::Mutex;

/// Serializes game-server logins with a configurable minimum spacing.
///
/// Unlike simply staggering each client's start delay, this gate reserves an
/// ordered slot for every caller, so the spacing between successive logins
/// stays consistent even if several clients happen to finish account
/// bootstrap at the same moment.
pub struct LoginGate {
    next_allowed: Mutex<Instant>,
    spacing: Duration,
}

impl LoginGate {
    /// Creates a new gate enforcing at least `stagger_secs` seconds between
    /// successive logins.
    ///
    /// # Arguments
    ///
    /// * `stagger_secs` - Minimum seconds between successive logins. A value
    ///   of `0.0` or less disables gating entirely.
    ///
    /// # Returns
    ///
    /// * A new [`LoginGate`] ready to be shared across client tasks.
    pub fn new(stagger_secs: f64) -> Self {
        Self {
            next_allowed: Mutex::new(Instant::now()),
            spacing: Duration::from_secs_f64(stagger_secs.max(0.0)),
        }
    }

    /// Waits until this caller's reserved login slot begins.
    ///
    /// Calls are serialized: each acquisition reserves the next slot at
    /// least `spacing` after the previously reserved slot. Returns
    /// immediately when the gate was constructed with zero spacing.
    pub async fn acquire(&self) {
        if self.spacing.is_zero() {
            return;
        }

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
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[tokio::test]
    async fn disabled_gate_does_not_wait() {
        let gate = LoginGate::new(0.0);
        let start = Instant::now();
        gate.acquire().await;
        gate.acquire().await;
        assert!(start.elapsed() < Duration::from_millis(50));
    }

    #[tokio::test]
    async fn enforces_minimum_spacing_between_acquires() {
        let gate = Arc::new(LoginGate::new(0.05));
        let start = Instant::now();
        gate.acquire().await;
        gate.acquire().await;
        gate.acquire().await;
        // Three sequential acquires spaced 50ms apart should take at least ~100ms.
        assert!(start.elapsed() >= Duration::from_millis(90));
    }

    #[tokio::test]
    async fn concurrent_acquires_are_serialized() {
        let gate = Arc::new(LoginGate::new(0.03));
        let start = Instant::now();
        let mut handles = Vec::new();
        for _ in 0..4 {
            let gate = gate.clone();
            handles.push(tokio::spawn(async move {
                gate.acquire().await;
            }));
        }
        for h in handles {
            h.await.unwrap();
        }
        // Four slots spaced 30ms apart should take at least ~90ms even when
        // all callers arrive concurrently.
        assert!(start.elapsed() >= Duration::from_millis(90));
    }
}
