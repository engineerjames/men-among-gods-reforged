pub mod helpers;
pub mod pipelines;
pub mod routes;
pub mod types;

use axum::routing::{delete, get, post, put};
use axum::Router;
use axum_governor::GovernorLayer;
use lazy_limit::{init_rate_limiter, Duration, RuleConfig};
use log::{error, info, LevelFilter};
use real::RealIpLayer;
use redis;
use std::env;
use std::net::SocketAddr;

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

    let client = redis::Client::open("redis://127.0.0.1:5556/")?;
    let con = client.get_multiplexed_async_connection().await?;
    info!("Connected to KeyDB");

    // build our application with a route
    let app = Router::new()
        .route("/login", post(routes::login))
        .route("/accounts", post(routes::create_account))
        // Token required routes
        .route("/characters", get(routes::get_characters))
        .route("/characters", post(routes::create_new_character))
        .route("/characters/{id}", put(routes::update_character))
        .route("/characters/{id}", delete(routes::delete_character))
        .layer(GovernorLayer::default())
        .layer(RealIpLayer::default())
        .with_state(con);

    info!("Listening on 0.0.0.0:5554");
    let listener = tokio::net::TcpListener::bind("0.0.0.0:5554").await.unwrap();
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await?;

    info!("Server shutdown");
    Ok(())
}
