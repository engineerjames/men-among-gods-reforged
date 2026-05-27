pub mod admin;
pub mod auth_extractor;
pub mod email;
pub mod helpers;
pub mod password;
pub mod pipelines;
pub mod rate_limit;
pub mod routes;

use axum::Router;
use axum::middleware;
use axum::routing::{delete, get, post, put};
use log::{LevelFilter, error, info, warn};
use real::RealIpLayer;
use std::env;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration as StdDuration;
use tokio::time::sleep;

use crate::email::EmailSender;

/// Minimum acceptable JWT secret length, in bytes. Anything shorter forces a
/// startup failure (HS256 is only as strong as its secret).
const MIN_JWT_SECRET_LEN: usize = 32;

/// Shared application state passed to all route handlers via Axum's
/// `State` extractor.
#[derive(Clone)]
pub struct ApiState {
    /// Multiplexed KeyDB connection (via `ConnectionManager` for transparent
    /// reconnect on transient failures).
    pub con: redis::aio::ConnectionManager,
    /// Optional email sender (None when SMTP is not configured).
    pub email_sender: Option<EmailSender>,
    /// HMAC signing secret for HS256 JWTs. Cached once at startup so
    /// per-request handlers never hit `env::var`.
    pub jwt_secret: Arc<Vec<u8>>,
}

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
                Some(trimmed.to_owned())
            }
        }
        Err(_) => Some("api.log".to_owned()),
    }
}

fn resolve_keydb_url() -> String {
    env::var("MAG_KEYDB_URL").unwrap_or_else(|_| "redis://127.0.0.1:5556/".to_owned())
}

fn resolve_api_bind_addr() -> String {
    env::var("API_BIND_ADDR").unwrap_or_else(|_| "0.0.0.0".to_owned())
}

fn resolve_api_port() -> u16 {
    env::var("API_PORT")
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(5554)
}

async fn connect_keydb_with_retry(
    keydb_url: &str,
) -> Result<redis::aio::ConnectionManager, Box<dyn std::error::Error>> {
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

        match redis::aio::ConnectionManager::new(client).await {
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

    Err(last_error.unwrap_or_else(|| Box::new(std::io::Error::other("Failed to connect to KeyDB"))))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Install the ring crypto provider for rustls before any TLS code runs.
    // Required when multiple crates (axum-server, lettre) both use rustls and
    // the process-level provider cannot be auto-detected from features alone.
    rustls::crypto::ring::default_provider()
        .install_default()
        .map_err(|_| "Failed to install rustls ring crypto provider")?;

    let log_level = resolve_log_level();
    let log_file = resolve_log_file();
    mag_core::initialize_logger(log_level, log_file.as_deref())?;

    info!(
        "API v{} starting (level={}, logfile={})",
        env!("CARGO_PKG_VERSION"),
        log_level,
        log_file.as_deref().unwrap_or("none")
    );

    // Ensure the JWT signing secret is configured and reasonably strong.
    let jwt_secret = match env::var("API_JWT_SECRET") {
        Ok(value) => value,
        Err(_) => {
            error!("Environment variable API_JWT_SECRET is not set");
            std::process::exit(1);
        }
    };
    let jwt_secret_trimmed = jwt_secret.trim();
    if jwt_secret_trimmed.len() < MIN_JWT_SECRET_LEN {
        error!(
            "API_JWT_SECRET is too short ({} bytes); refusing to start. Minimum is {} bytes.",
            jwt_secret_trimmed.len(),
            MIN_JWT_SECRET_LEN
        );
        std::process::exit(1);
    }
    let jwt_secret: Arc<Vec<u8>> = Arc::new(jwt_secret_trimmed.as_bytes().to_vec());

    let keydb_url = resolve_keydb_url();
    let con = connect_keydb_with_retry(&keydb_url).await?;
    info!("Connected to KeyDB");

    // One-shot idempotent backfill: per-account character SET + global
    // character-name claim keys. Runs before we bind the listener so the
    // hot-path helpers can rely on the indexes existing.
    {
        let mut con = con.clone();
        if let Err(err) = pipelines::migrate_character_indexes_v1(&mut con).await {
            error!("Character-index migration failed: {err}");
            return Err(Box::new(err) as Box<dyn std::error::Error>);
        }
    }

    let email_sender = EmailSender::from_env();
    if email_sender.is_none() {
        warn!("SMTP not configured — password reset emails will not be sent (set SMTP_HOST)");
    }

    let state = ApiState {
        con,
        email_sender,
        jwt_secret,
    };

    let admin_router = admin::build_admin_router(state.clone());
    if admin_router.is_some() {
        info!(
            "Admin routes enabled at /admin (token via {})",
            admin::auth::ADMIN_TOKEN_ENV
        );
    } else {
        warn!(
            "Admin routes DISABLED ({} unset or token shorter than 32 bytes)",
            admin::auth::ADMIN_TOKEN_ENV
        );
    }

    // build our application with a route
    let public_router = Router::new()
        // Public routes
        .route("/login", post(routes::login))
        .route("/accounts", post(routes::create_account))
        .route(
            "/accounts/reset-password/request",
            post(routes::request_password_reset),
        )
        .route(
            "/accounts/reset-password/confirm",
            post(routes::confirm_password_reset),
        )
        // Token required routes
        .route("/game/login_ticket", post(routes::create_game_login_ticket))
        .route("/characters", get(routes::get_characters))
        .route("/characters", post(routes::create_new_character))
        .route("/characters/{id}", put(routes::update_character))
        .route("/characters/{id}", delete(routes::delete_character))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            rate_limit::per_ip_rate_limit,
        ))
        .with_state(state.clone());

    let app = match admin_router {
        Some(admin) => Router::new()
            .merge(public_router)
            .nest("/admin", admin)
            .layer(RealIpLayer::default()),
        None => Router::new()
            .merge(public_router)
            .layer(RealIpLayer::default()),
    };

    let bind_address = format!("{}:{}", resolve_api_bind_addr(), resolve_api_port());
    info!("Listening on {}", bind_address);

    let tls_cert = std::env::var("API_TLS_CERT").map_err(|_| {
        "API_TLS_CERT environment variable is required (TLS is mandatory)".to_owned()
    })?;
    let tls_key = std::env::var("API_TLS_KEY").map_err(|_| {
        "API_TLS_KEY environment variable is required (TLS is mandatory)".to_owned()
    })?;

    info!("HTTPS enabled (cert={}, key={})", tls_cert, tls_key);
    let tls_config = axum_server::tls_rustls::RustlsConfig::from_pem_file(&tls_cert, &tls_key)
        .await
        .map_err(|e| format!("Failed to load TLS cert/key: {e}"))?;
    let addr: SocketAddr = bind_address
        .parse()
        .map_err(|e| format!("Invalid bind address: {e}"))?;
    axum_server::bind_rustls(addr, tls_config)
        .serve(app.into_make_service_with_connect_info::<SocketAddr>())
        .await?;

    info!("Server shutdown");
    Ok(())
}
