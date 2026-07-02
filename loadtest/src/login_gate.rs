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

use tokio::sync::{Mutex, MutexGuard};

/// Serializes game-server logins with a configurable minimum spacing.
///
/// Grants exclusive, ordered access to the login sequence (see
/// [`LoginGate::acquire`]); the returned [`LoginGateGuard`] must be held for
/// the full mint-ticket/connect/handshake sequence so the next caller's
/// cooldown is measured from actual completion, not a pre-computed schedule.
pub struct LoginGate {
    slot: Mutex<Instant>,
    spacing: Duration,
}

impl LoginGate {
    /// Creates a new gate enforcing at least `stagger_secs` seconds between
    /// the end of one login and the start of the next.
    ///
    /// # Arguments
    ///
    /// * `stagger_secs` - Minimum seconds between successive logins. A value
    ///   of `0.0` or less disables gating entirely (every caller proceeds
    ///   concurrently, with no locking overhead).
    ///
    /// # Returns
    ///
    /// * A new [`LoginGate`] ready to be shared across client tasks.
    pub fn new(stagger_secs: f64) -> Self {
        Self {
            slot: Mutex::new(Instant::now()),
            spacing: Duration::from_secs_f64(stagger_secs.max(0.0)),
        }
    }

    /// Waits for exclusive access to the login sequence.
    ///
    /// Blocks until both (a) every earlier caller has released its guard,
    /// and (b) at least `spacing` has elapsed since the previous holder's
    /// guard was dropped. When the gate was constructed with zero spacing,
    /// returns immediately without any locking (fully concurrent, matching
    /// "disabled" semantics).
    ///
    /// # Returns
    ///
    /// * A [`LoginGateGuard`] that must be held for the duration of the
    ///   login sequence. Dropping it releases the gate and starts the next
    ///   caller's `spacing` cooldown counting from *now*.
    pub async fn acquire(&self) -> LoginGateGuard<'_> {
        if self.spacing.is_zero() {
            return LoginGateGuard {
                guard: None,
                spacing: self.spacing,
            };
        }

        let guard = self.slot.lock().await;
        let now = Instant::now();
        if *guard > now {
            tokio::time::sleep(*guard - now).await;
        }

        LoginGateGuard {
            guard: Some(guard),
            spacing: self.spacing,
        }
    }
}

/// RAII guard granting exclusive access to the login sequence.
///
/// Hold this for as long as the login sequence is in flight (ticket mint
/// through handshake completion). Dropping it — on success, failure, or
/// early return — releases the gate and schedules the next caller's earliest
/// allowed start time as `now + spacing`.
pub struct LoginGateGuard<'a> {
    guard: Option<MutexGuard<'a, Instant>>,
    spacing: Duration,
}

impl Drop for LoginGateGuard<'_> {
    fn drop(&mut self) {
        if let Some(mut guard) = self.guard.take() {
            *guard = Instant::now() + self.spacing;
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
        let _g1 = gate.acquire().await;
        let _g2 = gate.acquire().await;
        assert!(start.elapsed() < Duration::from_millis(50));
    }

    #[tokio::test]
    async fn enforces_minimum_spacing_between_sequential_acquires() {
        let gate = LoginGate::new(0.05);
        let start = Instant::now();
        drop(gate.acquire().await);
        drop(gate.acquire().await);
        drop(gate.acquire().await);
        // Three sequential acquire+release cycles spaced 50ms apart should
        // take at least ~100ms.
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
                drop(gate.acquire().await);
            }));
        }
        for h in handles {
            h.await.unwrap();
        }
        // Four acquire+release cycles spaced 30ms apart should take at least
        // ~90ms even when all callers arrive concurrently.
        assert!(start.elapsed() >= Duration::from_millis(90));
    }

    /// Regression test for the reported burst bug: a slow holder (e.g. an
    /// in-flight login delayed by unrelated API rate-limiting) must delay
    /// the *next* caller by its own real duration plus `spacing` — not just
    /// `spacing` alone, which is what a pre-computed-schedule design would
    /// incorrectly allow (letting a second caller "catch up" and finish at
    /// nearly the same time as the first).
    #[tokio::test]
    async fn next_acquire_waits_for_previous_holder_to_finish_plus_spacing() {
        let gate = Arc::new(LoginGate::new(0.01)); // 10ms spacing
        let first = gate.acquire().await;

        let gate2 = gate.clone();
        let start = Instant::now();
        let second = tokio::spawn(async move {
            let _g2 = gate2.acquire().await;
            start.elapsed()
        });

        // Simulate a slow login sequence (e.g. stuck behind a shared 429
        // cooldown) while holding the first slot.
        tokio::time::sleep(Duration::from_millis(100)).await;
        drop(first);

        let elapsed = second.await.unwrap();
        assert!(
            elapsed >= Duration::from_millis(105),
            "second acquire completed too early ({elapsed:?}); it must wait for \
             the first holder's actual completion, not a pre-scheduled slot"
        );
    }
}
