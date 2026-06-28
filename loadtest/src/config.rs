//! TOML configuration types for the load-test runner.

use mag_core::types::api::{Class, Sex};
use serde::Deserialize;

/// Top-level load-test configuration, loaded from a TOML file.
#[derive(Debug, Deserialize, Clone)]
#[serde(default)]
pub struct LoadTestConfig {
    /// Game server connection settings.
    pub server: ServerConfig,
    /// Account API settings.
    pub api: ApiConfig,
    /// Run-time parameters (client count, duration, etc.).
    pub run: RunConfig,
    /// Movement simulation parameters.
    pub movement: MovementConfig,
    /// Network impairment simulation parameters.
    pub impairment: ImpairmentConfig,
    /// CL_PING keepalive settings.
    pub ping: PingConfig,
    /// Bot account/character creation settings.
    pub accounts: AccountConfig,
}

impl Default for LoadTestConfig {
    fn default() -> Self {
        Self {
            server: ServerConfig::default(),
            api: ApiConfig::default(),
            run: RunConfig::default(),
            movement: MovementConfig::default(),
            impairment: ImpairmentConfig::default(),
            ping: PingConfig::default(),
            accounts: AccountConfig::default(),
        }
    }
}

/// Game server connection parameters.
#[derive(Debug, Deserialize, Clone)]
#[serde(default)]
pub struct ServerConfig {
    /// Hostname or IP address of the game server.
    pub host: String,
    /// TCP port of the game server.
    pub port: u16,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".into(),
            port: 5555,
        }
    }
}

/// Account API (auth service) parameters.
#[derive(Debug, Deserialize, Clone)]
#[serde(default)]
pub struct ApiConfig {
    /// Base URL of the account API, e.g. `https://127.0.0.1:5554`.
    pub base_url: String,
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            base_url: "https://127.0.0.1:5554".into(),
        }
    }
}

/// Run-time parameters controlling the load-test scenario.
#[derive(Debug, Deserialize, Clone)]
#[serde(default)]
pub struct RunConfig {
    /// Total number of bot clients to simulate.
    pub num_clients: usize,
    /// Seconds over which all clients ramp up (staggered connections).
    pub ramp_up_secs: f64,
    /// Total wall-clock duration of the test in seconds.
    pub duration_secs: f64,
    /// Seconds between periodic metric reports to stdout.
    pub report_interval_secs: f64,
}

impl Default for RunConfig {
    fn default() -> Self {
        Self {
            num_clients: 10,
            ramp_up_secs: 5.0,
            duration_secs: 60.0,
            report_interval_secs: 10.0,
        }
    }
}

/// Per-client movement simulation parameters.
#[derive(Debug, Deserialize, Clone)]
#[serde(default)]
pub struct MovementConfig {
    /// Maximum tile radius for random movement targets around current position.
    pub radius: i16,
    /// Milliseconds between movement command sends.
    pub interval_ms: u64,
}

impl Default for MovementConfig {
    fn default() -> Self {
        Self {
            radius: 5,
            interval_ms: 500,
        }
    }
}

/// App-level network impairment parameters.
///
/// Applied to outgoing movement and ping commands.  CTick keepalive packets
/// are always sent without impairment to avoid idle-disconnect kicks.
/// NOTE: true packet *loss* on the inbound direction requires OS-level shaping
/// (`dummynet`/`tc`), not this tool.
#[derive(Debug, Deserialize, Clone)]
#[serde(default)]
pub struct ImpairmentConfig {
    /// Fixed added send latency in milliseconds.
    pub latency_ms: u64,
    /// Random jitter added on top of `latency_ms` (uniform ±jitter_ms/2).
    pub jitter_ms: u64,
    /// Probability [0.0, 1.0] that a send is silently dropped.
    pub drop_pct: f64,
}

impl Default for ImpairmentConfig {
    fn default() -> Self {
        Self {
            latency_ms: 0,
            jitter_ms: 0,
            drop_pct: 0.0,
        }
    }
}

/// CL_PING keepalive / RTT measurement settings.
#[derive(Debug, Deserialize, Clone)]
#[serde(default)]
pub struct PingConfig {
    /// Enable periodic CL_PING sends.
    pub enabled: bool,
    /// Seconds between successive pings.
    pub interval_secs: f64,
}

impl Default for PingConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            interval_secs: 5.0,
        }
    }
}

/// Bot account and character creation settings.
#[derive(Debug, Deserialize, Clone)]
#[serde(default)]
pub struct AccountConfig {
    /// Deterministic username prefix.  Bot `i` gets username `{prefix}-{i}`.
    pub prefix: String,
    /// Email domain used for bot account registration.
    pub email_domain: String,
    /// Password shared across all bot accounts.
    pub password: String,
    /// Starting character class.  One of: mercenary, templar, harakim.
    pub class: String,
    /// Character sex.  One of: male, female.
    pub sex: String,
}

impl Default for AccountConfig {
    fn default() -> Self {
        Self {
            prefix: "loadtest".into(),
            email_domain: "example.com".into(),
            password: "loadtest1234".into(),
            class: "mercenary".into(),
            sex: "male".into(),
        }
    }
}

impl AccountConfig {
    /// Parses the configured sex string to a [`Sex`] variant.
    ///
    /// # Returns
    ///
    /// * [`Sex::Female`] when the config string is `"female"`, [`Sex::Male`] otherwise.
    pub fn sex(&self) -> Sex {
        if self.sex.trim().eq_ignore_ascii_case("female") {
            Sex::Female
        } else {
            Sex::Male
        }
    }

    /// Parses the configured class string to a [`Class`] variant.
    ///
    /// Accepts `"mercenary"`, `"templar"`, `"harakim"`.  Defaults to
    /// [`Class::Mercenary`] for any unrecognised string.
    ///
    /// # Returns
    ///
    /// * A [`Class`] variant matching the configured string, or [`Class::Mercenary`].
    pub fn class(&self) -> Class {
        match self.class.trim().to_lowercase().as_str() {
            "templar" => Class::Templar,
            "harakim" => Class::Harakim,
            _ => Class::Mercenary,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_parses() {
        let cfg: LoadTestConfig = toml::from_str("").unwrap();
        assert_eq!(cfg.server.port, 5555);
        assert_eq!(cfg.run.num_clients, 10);
        assert!((cfg.run.duration_secs - 60.0).abs() < f64::EPSILON);
    }

    #[test]
    fn sex_parsing() {
        let mut a = AccountConfig::default();
        assert_eq!(a.sex(), Sex::Male);
        a.sex = "female".into();
        assert_eq!(a.sex(), Sex::Female);
        a.sex = "FEMALE".into();
        assert_eq!(a.sex(), Sex::Female);
    }

    #[test]
    fn class_parsing() {
        let mut a = AccountConfig::default();
        assert!(matches!(a.class(), Class::Mercenary));
        a.class = "templar".into();
        assert!(matches!(a.class(), Class::Templar));
        a.class = "harakim".into();
        assert!(matches!(a.class(), Class::Harakim));
        a.class = "unknown".into();
        assert!(matches!(a.class(), Class::Mercenary));
    }
}
