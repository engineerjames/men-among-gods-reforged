//! TOML configuration types for the load-test runner.

use mag_core::types::api::{Class, Sex};
use rand::SeedableRng;
use rand::rngs::StdRng;
use rand::seq::IndexedRandom;
use serde::Deserialize;

/// Top-level load-test configuration, loaded from a TOML file.
#[derive(Debug, Deserialize, Clone)]
#[serde(default)]
#[derive(Default)]
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
    /// In-world behaviour simulation (spell casting, chat, environment use).
    pub behavior: BehaviorConfig,
    /// Periodic slash-commands each client issues on independent intervals.
    pub commands: Vec<CommandEntry>,
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
    /// Shared account-API request rate for all simulated clients.
    ///
    /// Every bot normally comes from the same source IP, so this must be well
    /// below the API's per-IP public limiter. Use `1` when running against the
    /// local Docker stack unless you have deliberately raised the API limit.
    pub requests_per_second: u64,
    /// Maximum number of account-API requests outstanding at any one time.
    ///
    /// The default of `1` makes real throughput `1 / request_latency`, which
    /// the API's server-side Argon2 hashing pins to well under 1 req/s. Raise
    /// this (e.g. 16) to bootstrap hundreds of bots in reasonable time against
    /// a throwaway environment.
    pub max_in_flight: usize,
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            base_url: "https://127.0.0.1:5554".into(),
            requests_per_second: 1,
            max_in_flight: 1,
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
    /// Minimum seconds between successive characters logging into the game
    /// server (spacing spawn events), on top of `ramp_up_secs`.
    pub login_stagger_secs: f64,
}

impl Default for RunConfig {
    fn default() -> Self {
        Self {
            num_clients: 10,
            ramp_up_secs: 5.0,
            duration_secs: 60.0,
            report_interval_secs: 10.0,
            login_stagger_secs: 0.0,
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
    /// Enable one-shot login dispersion: right after a bot's first confirmed
    /// world position, it says the god password (from the `MAG_GOD_PASSWORD`
    /// environment variable) and then `/goto`s to a random in-bounds map
    /// location, before falling back to normal random movement.
    pub enable_dispersion: bool,
}

impl Default for MovementConfig {
    fn default() -> Self {
        Self {
            radius: 5,
            interval_ms: 500,
            enable_dispersion: false,
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

/// In-world behaviour simulation settings (`[behavior]`).
///
/// Each sub-table drives one independent periodic action a bot performs
/// once it has a confirmed world position. All are optional and default to
/// disabled so existing configs keep the old "move only" behaviour.
#[derive(Debug, Deserialize, Clone, Default)]
#[serde(default)]
pub struct BehaviorConfig {
    /// Spell/skill casting (`[behavior.cast]`).
    pub cast: CastConfig,
    /// Free-text chat messages (`[behavior.chat]`).
    pub chat: ChatConfig,
    /// Environment interaction with usable items (`[behavior.interact]`).
    pub interact: InteractConfig,
}

/// Periodic spell/skill casting settings (`[behavior.cast]`).
///
/// A bot only casts skills the server has reported as known for its
/// character (`SV_SETCHARSKILL` with a non-zero base value), filtered to the
/// set of directly-castable, self-targeted skills. When `allow_hostile` is
/// set, hostile skills are also eligible whenever another character is
/// visible on the bot's map window.
#[derive(Debug, Deserialize, Clone)]
#[serde(default)]
pub struct CastConfig {
    /// Enable periodic casting.
    pub enabled: bool,
    /// Milliseconds between cast attempts per client.
    pub interval_ms: u64,
    /// Also cast hostile skills at a random visible character.
    pub allow_hostile: bool,
}

impl Default for CastConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            interval_ms: 8_000,
            allow_hostile: false,
        }
    }
}

/// Periodic chat settings (`[behavior.chat]`).
#[derive(Debug, Deserialize, Clone)]
#[serde(default)]
pub struct ChatConfig {
    /// Enable periodic chat messages.
    pub enabled: bool,
    /// Milliseconds between chat messages per client.
    pub interval_ms: u64,
    /// Pool of messages to pick from at random. Empty falls back to a small
    /// built-in set.
    pub messages: Vec<String>,
}

impl Default for ChatConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            interval_ms: 30_000,
            messages: Vec::new(),
        }
    }
}

/// Periodic environment interaction settings (`[behavior.interact]`).
///
/// The bot scans its visible map window for tiles flagged `ISUSABLE`
/// (doors, levers, portals, shrines, ...) within `radius` tiles and sends a
/// `CL_USE` for a random one; the server path-finds to it and applies the
/// item's use handler, exactly as if a player shift-clicked it.
#[derive(Debug, Deserialize, Clone)]
#[serde(default)]
pub struct InteractConfig {
    /// Enable periodic interaction attempts.
    pub enabled: bool,
    /// Milliseconds between interaction attempts per client.
    pub interval_ms: u64,
    /// Maximum Chebyshev tile distance from the bot to a candidate item.
    pub radius: i32,
}

impl Default for InteractConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            interval_ms: 10_000,
            radius: 8,
        }
    }
}

/// A single periodic slash-command a bot client repeatedly sends.
///
/// Declared as a TOML array of tables under `[[commands]]`, e.g.:
///
/// ```toml
/// [[commands]]
/// command = "/rank"
/// interval_secs = 1.0
/// ```
#[derive(Debug, Deserialize, Clone)]
pub struct CommandEntry {
    /// Command text to send verbatim as chat input, e.g. `"/rank"`.
    pub command: String,
    /// Seconds between successive sends of this command, per client.
    pub interval_secs: f64,
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
    /// Starting character class.  One of: mercenary, templar, harakim, random.
    ///
    /// `random` picks one of the three starting classes per bot, seeded by
    /// the bot index so the assignment is stable across runs.
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

    /// Resolves the starting class for bot `index`.
    ///
    /// Accepts `"mercenary"`, `"templar"`, `"harakim"`, or `"random"`.
    /// `random` picks uniformly from [`STARTING_CLASSES`] using an RNG seeded
    /// by `index`, so the same bot always gets the same class across runs
    /// (its character persists server-side after the first run anyway).
    /// Defaults to [`Class::Mercenary`] for any unrecognised string.
    ///
    /// # Arguments
    ///
    /// * `index` - Bot index, used as the seed for `random`.
    ///
    /// # Returns
    ///
    /// * The [`Class`] to request when creating this bot's character.
    pub fn class_for(&self, index: usize) -> Class {
        match self.class.trim().to_lowercase().as_str() {
            "templar" => Class::Templar,
            "harakim" => Class::Harakim,
            "random" => {
                let mut rng = StdRng::seed_from_u64(index as u64);
                *STARTING_CLASSES
                    .choose(&mut rng)
                    .expect("STARTING_CLASSES is non-empty")
            }
            _ => Class::Mercenary,
        }
    }
}

/// Classes a freshly created character may start as.
pub const STARTING_CLASSES: [Class; 3] = [Class::Mercenary, Class::Templar, Class::Harakim];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_parses() {
        let cfg: LoadTestConfig = toml::from_str("").unwrap();
        assert_eq!(cfg.server.port, 5555);
        assert_eq!(cfg.api.requests_per_second, 1);
        assert_eq!(cfg.run.num_clients, 10);
        assert!((cfg.run.duration_secs - 60.0).abs() < f64::EPSILON);
        assert!(!cfg.movement.enable_dispersion);
        assert!(cfg.commands.is_empty());
    }

    #[test]
    fn commands_section_parses() {
        let cfg: LoadTestConfig = toml::from_str(
            r#"
            [[commands]]
            command = "/rank"
            interval_secs = 1.0

            [[commands]]
            command = "/who"
            interval_secs = 2.0
            "#,
        )
        .unwrap();
        assert_eq!(cfg.commands.len(), 2);
        assert_eq!(cfg.commands[0].command, "/rank");
        assert!((cfg.commands[0].interval_secs - 1.0).abs() < f64::EPSILON);
        assert_eq!(cfg.commands[1].command, "/who");
        assert!((cfg.commands[1].interval_secs - 2.0).abs() < f64::EPSILON);
    }

    #[test]
    fn dispersion_flag_parses() {
        let cfg: LoadTestConfig = toml::from_str("[movement]\nenable_dispersion = true\n").unwrap();
        assert!(cfg.movement.enable_dispersion);
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
        assert!(matches!(a.class_for(0), Class::Mercenary));
        a.class = "templar".into();
        assert!(matches!(a.class_for(0), Class::Templar));
        a.class = "harakim".into();
        assert!(matches!(a.class_for(0), Class::Harakim));
        a.class = "unknown".into();
        assert!(matches!(a.class_for(0), Class::Mercenary));
    }

    #[test]
    fn random_class_is_stable_per_index_and_covers_all_classes() {
        let a = AccountConfig {
            class: "random".into(),
            ..AccountConfig::default()
        };
        for i in 0..50 {
            assert_eq!(a.class_for(i), a.class_for(i));
            assert!(STARTING_CLASSES.contains(&a.class_for(i)));
        }
        let seen: std::collections::HashSet<Class> = (0..200).map(|i| a.class_for(i)).collect();
        assert_eq!(seen.len(), STARTING_CLASSES.len());
    }

    #[test]
    fn behavior_defaults_disabled() {
        let cfg: LoadTestConfig = toml::from_str("").unwrap();
        assert!(!cfg.behavior.cast.enabled);
        assert!(!cfg.behavior.chat.enabled);
        assert!(!cfg.behavior.interact.enabled);
        assert_eq!(cfg.behavior.cast.interval_ms, 8_000);
        assert_eq!(cfg.behavior.interact.radius, 8);
    }

    #[test]
    fn behavior_section_parses() {
        let cfg: LoadTestConfig = toml::from_str(
            r#"
            [behavior.cast]
            enabled = true
            interval_ms = 1500
            allow_hostile = true

            [behavior.chat]
            enabled = true
            messages = ["hi", "lo"]

            [behavior.interact]
            enabled = true
            radius = 3
            "#,
        )
        .unwrap();
        assert!(cfg.behavior.cast.enabled);
        assert_eq!(cfg.behavior.cast.interval_ms, 1500);
        assert!(cfg.behavior.cast.allow_hostile);
        assert!(cfg.behavior.chat.enabled);
        assert_eq!(cfg.behavior.chat.messages, vec!["hi", "lo"]);
        assert_eq!(cfg.behavior.chat.interval_ms, 30_000);
        assert!(cfg.behavior.interact.enabled);
        assert_eq!(cfg.behavior.interact.radius, 3);
    }
}
