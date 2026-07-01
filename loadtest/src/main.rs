//! `mag-loadtest` — game-server load-test client simulator.
//!
//! Spawns up to hundreds of headless bot clients that authenticate via the
//! account API, connect to the game server over TLS, and simulate movement.
//!
//! # Usage
//!
//! ```text
//! mag-loadtest --config loadtest.toml
//! mag-loadtest --config loadtest.toml --clients 50 --duration 120
//! ```

mod api_bootstrap;
mod config;
mod login_gate;
mod metrics;
mod net_impair;
mod protocol;
mod sim_client;

use std::sync::Arc;
use std::time::{Duration, Instant};

use clap::Parser;
use tokio::sync::broadcast;
use tokio::task::JoinSet;
use tokio::time::{MissedTickBehavior, interval, sleep};

use api_bootstrap::RateLimiter;
use config::LoadTestConfig;
use login_gate::LoginGate;
use metrics::Metrics;

// ---------------------------------------------------------------------------
// CLI
// ---------------------------------------------------------------------------

/// Game-server load-test client simulator.
#[derive(Parser, Debug)]
#[command(version, about)]
struct Cli {
    /// Path to the TOML configuration file.
    #[arg(short, long, default_value = "loadtest.toml")]
    config: String,

    /// Override: number of bot clients to simulate.
    #[arg(long)]
    clients: Option<usize>,

    /// Override: total test duration in seconds.
    #[arg(long)]
    duration: Option<f64>,

    /// Override: ramp-up duration in seconds.
    #[arg(long)]
    ramp_up: Option<f64>,

    /// Override: minimum seconds between successive character logins.
    #[arg(long)]
    login_stagger: Option<f64>,

    /// Override: game server host.
    #[arg(long)]
    server_host: Option<String>,

    /// Override: game server port.
    #[arg(long)]
    server_port: Option<u16>,

    /// Override: account API base URL.
    #[arg(long)]
    api_url: Option<String>,

    /// Override: shared account API request rate for all simulated clients.
    #[arg(long)]
    api_rps: Option<u64>,

    /// Override: enable one-shot login dispersion (god password + `/goto`).
    #[arg(long)]
    enable_dispersion: Option<bool>,
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialise logging (respects RUST_LOG env var; defaults to `info`).
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    // Best-effort load of a repo-root `.env` file (matches server/keydb's
    // local-tool convention); ignored if absent.
    let _ = dotenvy::dotenv();

    // Install ring crypto provider for rustls (required before any TLS use).
    let _ = rustls::crypto::ring::default_provider().install_default();

    let cli = Cli::parse();

    // Load TOML config, then apply CLI overrides.
    let mut config = load_config(&cli.config)?;
    apply_overrides(&mut config, &cli);

    // Resolve the god password used for login dispersion. Fails fast (before
    // any client connects) if dispersion is enabled but the password is unset.
    let god_password = resolve_god_password(
        config.movement.enable_dispersion,
        std::env::var("MAG_GOD_PASSWORD").ok(),
    )?;

    log::info!(
        "Starting load test: {} client(s), {:.1}s duration, ramp-up {:.1}s, login stagger {:.1}s",
        config.run.num_clients,
        config.run.duration_secs,
        config.run.ramp_up_secs,
        config.run.login_stagger_secs,
    );
    log::info!(
        "  Server: {}:{} | API: {} ({} req/s shared)",
        config.server.host,
        config.server.port,
        config.api.base_url,
        config.api.requests_per_second,
    );
    log::info!(
        "  Movement: radius={} interval={}ms | Impairment: latency={}ms jitter={}ms drop={:.1}%",
        config.movement.radius,
        config.movement.interval_ms,
        config.impairment.latency_ms,
        config.impairment.jitter_ms,
        config.impairment.drop_pct * 100.0,
    );
    log::info!(
        "  Dispersion: {}",
        if config.movement.enable_dispersion {
            "enabled (god password loaded from MAG_GOD_PASSWORD)"
        } else {
            "disabled"
        },
    );

    let config = Arc::new(config);
    let god_password = Arc::new(god_password);
    let metrics = Arc::new(Metrics::new());
    // Every simulated client shares one source IP, and the API public limiter
    // uses a fixed one-second KeyDB counter. Keep bootstrap deliberately slow.
    let rate_limiter = Arc::new(RateLimiter::new(config.api.requests_per_second));
    // Serializes actual game-server logins so recently spawned characters
    // have time to move out of the crowded spawn area before the next one.
    let login_gate = Arc::new(LoginGate::new(config.run.login_stagger_secs));

    // Shutdown broadcast: fires once when the run duration elapses.
    let (shutdown_tx, _) = broadcast::channel::<()>(1);

    let mut tasks: JoinSet<()> = JoinSet::new();

    // Spawn one task per bot client.
    let num = config.run.num_clients;
    for index in 0..num {
        let config = config.clone();
        let metrics = metrics.clone();
        let rate_limiter = rate_limiter.clone();
        let login_gate = login_gate.clone();
        let god_password = god_password.clone();
        let shutdown_rx = shutdown_tx.subscribe();
        tasks.spawn(async move {
            sim_client::run(
                index,
                config,
                rate_limiter,
                login_gate,
                god_password,
                metrics,
                shutdown_rx,
            )
            .await;
        });
    }

    // Spawn periodic metrics reporter.
    {
        let metrics = metrics.clone();
        let report_interval_secs = config.run.report_interval_secs;
        let mut shutdown_rx = shutdown_tx.subscribe();
        tasks.spawn(async move {
            let mut iv = interval(Duration::from_secs_f64(report_interval_secs.max(1.0)));
            iv.set_missed_tick_behavior(MissedTickBehavior::Skip);
            let start = Instant::now();
            loop {
                tokio::select! {
                    _ = iv.tick() => {
                        metrics.print_periodic(start.elapsed());
                    }
                    _ = shutdown_rx.recv() => break,
                }
            }
        });
    }

    let start = Instant::now();

    // Run for the configured duration.
    sleep(Duration::from_secs_f64(config.run.duration_secs)).await;

    log::info!("Run duration elapsed — sending shutdown signal");
    let _ = shutdown_tx.send(());

    // Wait for all tasks to finish.
    while tasks.join_next().await.is_some() {}

    // Print the final report.
    metrics.print_final(start.elapsed());

    Ok(())
}

// ---------------------------------------------------------------------------
// Config loading
// ---------------------------------------------------------------------------

/// Loads [`LoadTestConfig`] from a TOML file, falling back to defaults if the
/// file is absent.
///
/// # Arguments
///
/// * `path` - File-system path to the TOML config file.
///
/// # Returns
///
/// * `Ok(LoadTestConfig)` on success.
/// * `Err` if the file exists but cannot be read or parsed.
fn load_config(path: &str) -> anyhow::Result<LoadTestConfig> {
    match std::fs::read_to_string(path) {
        Ok(text) => {
            let cfg: LoadTestConfig = toml::from_str(&text)
                .map_err(|e| anyhow::anyhow!("TOML parse error in {path}: {e}"))?;
            log::info!("Loaded config from '{path}'");
            Ok(cfg)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            log::warn!("Config file '{path}' not found — using defaults");
            Ok(LoadTestConfig::default())
        }
        Err(e) => Err(anyhow::anyhow!("Cannot read config '{path}': {e}")),
    }
}

/// Applies CLI overrides on top of the loaded configuration.
///
/// # Arguments
///
/// * `config` - Configuration to mutate.
/// * `cli` - Parsed CLI arguments.
fn apply_overrides(config: &mut LoadTestConfig, cli: &Cli) {
    if let Some(n) = cli.clients {
        config.run.num_clients = n;
    }
    if let Some(d) = cli.duration {
        config.run.duration_secs = d;
    }
    if let Some(r) = cli.ramp_up {
        config.run.ramp_up_secs = r;
    }
    if let Some(s) = cli.login_stagger {
        config.run.login_stagger_secs = s;
    }
    if let Some(ref h) = cli.server_host {
        config.server.host.clone_from(h);
    }
    if let Some(p) = cli.server_port {
        config.server.port = p;
    }
    if let Some(ref u) = cli.api_url {
        config.api.base_url.clone_from(u);
    }
    if let Some(rps) = cli.api_rps {
        config.api.requests_per_second = rps.max(1);
    }
    if let Some(d) = cli.enable_dispersion {
        config.movement.enable_dispersion = d;
    }
}

/// Resolves the god password required for login dispersion.
///
/// # Arguments
///
/// * `enable_dispersion` - Whether `movement.enable_dispersion` is set.
/// * `env_value` - Raw value read from the `MAG_GOD_PASSWORD` environment variable, if set.
///
/// # Returns
///
/// * `Ok(String)` - The password to use; empty when dispersion is disabled.
///
/// # Errors
///
/// * Returns an error when dispersion is enabled but `env_value` is missing or blank,
///   so the whole run fails fast before any client connects.
fn resolve_god_password(
    enable_dispersion: bool,
    env_value: Option<String>,
) -> anyhow::Result<String> {
    if !enable_dispersion {
        return Ok(String::new());
    }
    match env_value {
        Some(v) if !v.trim().is_empty() => Ok(v),
        _ => Err(anyhow::anyhow!(
            "movement.enable_dispersion is true but MAG_GOD_PASSWORD is not set or empty"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_config_defaults_when_file_missing() {
        let cfg = load_config("/tmp/nonexistent-loadtest-file-abc123.toml").unwrap();
        assert_eq!(cfg.server.port, 5555);
    }

    #[test]
    fn apply_overrides_replaces_values() {
        let mut cfg = LoadTestConfig::default();
        let cli = Cli {
            config: "loadtest.toml".into(),
            clients: Some(99),
            duration: Some(120.0),
            ramp_up: None,
            login_stagger: Some(1.5),
            server_host: Some("10.0.0.1".into()),
            server_port: Some(5556),
            api_url: None,
            api_rps: Some(2),
            enable_dispersion: Some(true),
        };
        apply_overrides(&mut cfg, &cli);
        assert_eq!(cfg.run.num_clients, 99);
        assert!((cfg.run.duration_secs - 120.0).abs() < f64::EPSILON);
        assert!((cfg.run.login_stagger_secs - 1.5).abs() < f64::EPSILON);
        assert_eq!(cfg.server.host, "10.0.0.1");
        assert_eq!(cfg.server.port, 5556);
        assert_eq!(cfg.api.requests_per_second, 2);
        assert!(cfg.movement.enable_dispersion);
    }

    #[test]
    fn resolve_god_password_disabled_ignores_env() {
        assert_eq!(resolve_god_password(false, None).unwrap(), "");
        assert_eq!(
            resolve_god_password(false, Some(String::new())).unwrap(),
            ""
        );
    }

    #[test]
    fn resolve_god_password_enabled_requires_value() {
        assert!(resolve_god_password(true, None).is_err());
        assert!(resolve_god_password(true, Some(String::new())).is_err());
        assert!(resolve_god_password(true, Some("   ".into())).is_err());
    }

    #[test]
    fn resolve_god_password_enabled_returns_value() {
        let pw = resolve_god_password(true, Some("devpassword".into())).unwrap();
        assert_eq!(pw, "devpassword");
    }
}
