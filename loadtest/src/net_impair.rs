//! App-level network impairment model: latency, jitter, and send-drop.
//!
//! Applied only to outgoing movement and ping packets.  CTick keepalive
//! packets bypass impairment to avoid idle-disconnect kicks from the server.

use rand::Rng;
use std::time::Duration;

use crate::config::ImpairmentConfig;

/// Computes whether an outgoing packet should be silently dropped.
///
/// # Arguments
///
/// * `cfg` - Active impairment configuration.
/// * `rng` - Caller-provided RNG instance.
///
/// # Returns
///
/// * `true` if the packet should be dropped.
pub fn should_drop<R: Rng>(cfg: &ImpairmentConfig, rng: &mut R) -> bool {
    cfg.drop_pct > 0.0 && rng.r#gen::<f64>() < cfg.drop_pct.clamp(0.0, 1.0)
}

/// Computes the outgoing send delay for a packet.
///
/// The delay is `latency_ms ± jitter_ms/2`, clamped to zero.
///
/// # Arguments
///
/// * `cfg` - Active impairment configuration.
/// * `rng` - Caller-provided RNG instance.
///
/// # Returns
///
/// * A [`Duration`] to sleep before sending; [`Duration::ZERO`] when no impairment is configured.
pub fn send_delay<R: Rng>(cfg: &ImpairmentConfig, rng: &mut R) -> Duration {
    if cfg.latency_ms == 0 && cfg.jitter_ms == 0 {
        return Duration::ZERO;
    }

    let jitter_offset: i64 = if cfg.jitter_ms > 0 {
        let half = cfg.jitter_ms as i64 / 2;
        rng.gen_range(-half..=half)
    } else {
        0
    };

    let total_ms = cfg.latency_ms as i64 + jitter_offset;
    if total_ms > 0 {
        Duration::from_millis(total_ms as u64)
    } else {
        Duration::ZERO
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand::rngs::StdRng;

    #[test]
    fn no_drop_when_pct_zero() {
        let cfg = ImpairmentConfig {
            latency_ms: 0,
            jitter_ms: 0,
            drop_pct: 0.0,
        };
        let mut rng = StdRng::seed_from_u64(42);
        for _ in 0..100 {
            assert!(!should_drop(&cfg, &mut rng));
        }
    }

    #[test]
    fn always_drop_when_pct_one() {
        let cfg = ImpairmentConfig {
            latency_ms: 0,
            jitter_ms: 0,
            drop_pct: 1.0,
        };
        let mut rng = StdRng::seed_from_u64(42);
        for _ in 0..20 {
            assert!(should_drop(&cfg, &mut rng));
        }
    }

    #[test]
    fn zero_delay_when_no_impairment() {
        let cfg = ImpairmentConfig::default();
        let mut rng = StdRng::seed_from_u64(1);
        assert_eq!(send_delay(&cfg, &mut rng), Duration::ZERO);
    }

    #[test]
    fn latency_bounds() {
        let cfg = ImpairmentConfig {
            latency_ms: 50,
            jitter_ms: 20,
            drop_pct: 0.0,
        };
        let mut rng = StdRng::seed_from_u64(7);
        for _ in 0..50 {
            let d = send_delay(&cfg, &mut rng);
            assert!(d >= Duration::from_millis(40), "delay {d:?} below minimum");
            assert!(d <= Duration::from_millis(60), "delay {d:?} above maximum");
        }
    }
}
