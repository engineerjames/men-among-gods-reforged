pub mod helpers;
pub mod pipelines;
pub mod routes;
pub mod types;

use axum::routing::{delete, get, post, put};
use axum::Router;
use axum_governor::GovernorLayer;
use lazy_limit::{init_rate_limiter, Duration, RuleConfig};
use log::{error, info, warn, LevelFilter};
use real::RealIpLayer;
use redis;
use std::env;
use std::net::SocketAddr;
use std::time::Duration as StdDuration;
use tokio::time::sleep;

fn parse_log_level(value: &str) -> Option<LevelFilter> {
    match value.to_lowercase().as_str() {
        "off" => Some(LevelFilter::Off),
        "error" => Some(LevelFilter::Error),
        "warn" | "warning" => Some(LevelFilter::Warn),
        "info" => Some(LevelFilter::Info),
        "debug" => Some(LevelFilter::Debug),
        "trace" => Some(LevelFilter::Trace),
        _ => None,
    }
}

fn resolve_log_level() -> LevelFilter {
    env::var("API_LOG_LEVEL")
        .ok()
        .as_deref()
        .and_then(parse_log_level)
        .unwrap_or(LevelFilter::Info)
}

fn resolve_log_file() -> Option<String> {
    match env::var("API_LOG_FILE") {
        Ok(value) => {
            let trimmed = value.trim();
            if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("none") {
                None
            } else {
                Some(trimmed.to_string())
            }
        }
        Err(_) => Some("api.log".to_string()),
    }
}

fn resolve_keydb_url() -> String {
    env::var("MAG_KEYDB_URL").unwrap_or_else(|_| "redis://127.0.0.1:5556/".to_string())
}

fn resolve_api_bind_addr() -> String {
    env::var("API_BIND_ADDR").unwrap_or_else(|_| "0.0.0.0".to_string())
}

fn resolve_api_port() -> u16 {
    env::var("API_PORT")
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(5554)
}

async fn connect_keydb_with_retry(
    keydb_url: &str,
) -> Result<redis::aio::MultiplexedConnection, Box<dyn std::error::Error>> {
    const MAX_RETRIES: u32 = 5;
    const RETRY_DELAY_SECS: u64 = 6;

    let mut last_error: Option<Box<dyn std::error::Error>> = None;

    for attempt in 0..=MAX_RETRIES {
        let client = match redis::Client::open(keydb_url) {
            Ok(client) => client,
            Err(err) => {
                last_error = Some(Box::new(err));
                if attempt < MAX_RETRIES {
                    warn!(
                        "Failed to create KeyDB client (attempt {}/{}), retrying in {}s",
                        attempt + 1,
                        MAX_RETRIES + 1,
                        RETRY_DELAY_SECS
                    );
                    sleep(StdDuration::from_secs(RETRY_DELAY_SECS)).await;
                }
                continue;
            }
        };

        match client.get_multiplexed_async_connection().await {
            Ok(con) => return Ok(con),
            Err(err) => {
                last_error = Some(Box::new(err));
                if attempt < MAX_RETRIES {
                    warn!(
                        "Failed to connect to KeyDB (attempt {}/{}), retrying in {}s",
                        attempt + 1,
                        MAX_RETRIES + 1,
                        RETRY_DELAY_SECS
                    );
                    sleep(StdDuration::from_secs(RETRY_DELAY_SECS)).await;
                }
            }
        }
    }

    Err(last_error.unwrap_or_else(|| {
        Box::new(std::io::Error::new(
            std::io::ErrorKind::Other,
            "Failed to connect to KeyDB",
        ))
    }))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let log_level = resolve_log_level();
    let log_file = resolve_log_file();
    core::initialize_logger(log_level, log_file.as_deref())?;

    info!(
        "API starting (level={}, logfile={})",
        log_level,
        log_file.as_deref().unwrap_or("none")
    );

    // 1 req/s globally
    init_rate_limiter!(
        default: RuleConfig::new(Duration::seconds(1), 1)
    )
    .await;
    info!("Rate limiter initialized: 1 req/s");

    // Ensure the right environment variables exist
    if env::var("API_JWT_SECRET").is_err() {
        error!("Environment variable API_JWT_SECRET is not set");
        std::process::exit(1);
    }

    let keydb_url = resolve_keydb_url();
    let con = connect_keydb_with_retry(&keydb_url).await?;
    info!("Connected to KeyDB");

    // build our application with a route
    let app = Router::new()
        // Public routes
        .route("/login", post(routes::login))
        .route("/accounts", post(routes::create_account))
        // Token required routes
        .route("/game/login_ticket", post(routes::create_game_login_ticket))
        .route("/characters", get(routes::get_characters))
        .route("/characters", post(routes::create_new_character))
        .route("/characters/{id}", put(routes::update_character))
        .route("/characters/{id}", delete(routes::delete_character))
        .layer(GovernorLayer::default())
        .layer(RealIpLayer::default())
        .with_state(con);

    let bind_address = format!("{}:{}", resolve_api_bind_addr(), resolve_api_port());
    info!("Listening on {}", bind_address);

    let tls_cert = std::env::var("API_TLS_CERT").ok();
    let tls_key = std::env::var("API_TLS_KEY").ok();

    match (tls_cert, tls_key) {
        (Some(cert_path), Some(key_path)) => {
            info!("HTTPS enabled (cert={}, key={})", cert_path, key_path);
            let tls_config =
                axum_server::tls_rustls::RustlsConfig::from_pem_file(&cert_path, &key_path)
                    .await
                    .map_err(|e| format!("Failed to load TLS cert/key: {e}"))?;
            let addr: SocketAddr = bind_address
                .parse()
                .map_err(|e| format!("Invalid bind address: {e}"))?;
            axum_server::bind_rustls(addr, tls_config)
                .serve(app.into_make_service_with_connect_info::<SocketAddr>())
                .await?;
        }
        _ => {
            warn!("╔══════════════════════════════════════════════════════════════╗");
            warn!("║  WARNING: API is running WITHOUT TLS encryption!            ║");
            warn!("║  All HTTP traffic is transmitted in plaintext.              ║");
            warn!("║  Set API_TLS_CERT and API_TLS_KEY to enable HTTPS.          ║");
            warn!("╚══════════════════════════════════════════════════════════════╝");
            let listener = tokio::net::TcpListener::bind(&bind_address).await?;
            axum::serve(
                listener,
                app.into_make_service_with_connect_info::<SocketAddr>(),
            )
            .await?;
        }
    }

    info!("Server shutdown");
    Ok(())
}
