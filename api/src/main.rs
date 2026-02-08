pub mod types;

use crate::types::{CreateAccountRequest, CreateAccountResponse, LoginRequest, LoginResponse};
use axum::{extract::State, http::StatusCode, routing::post, Json, Router};
use axum_governor::GovernorLayer;
use lazy_limit::{init_rate_limiter, Duration, RuleConfig};
use lazy_static::lazy_static;
use log::{error, info, warn, LevelFilter};
use real::RealIpLayer;
use redis;
use redis::AsyncCommands;
use regex::Regex;
use std::env;

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

    let client = redis::Client::open("redis://127.0.0.1:5556/")?;
    let con = client.get_multiplexed_async_connection().await?;
    info!("Connected to KeyDB");

    // build our application with a route
    let app = Router::new()
        .route("/login", post(login))
        .route("/accounts", post(create_account))
        .layer(
            tower::ServiceBuilder::new()
                .layer(RealIpLayer::default())
                .layer(GovernorLayer::default()),
        )
        .with_state(con);

    info!("Listening on 0.0.0.0:5554");
    let listener = tokio::net::TcpListener::bind("0.0.0.0:5554").await.unwrap();
    axum::serve(listener, app).await?;

    info!("Server shutdown");
    Ok(())
}

async fn login(
    State(_con): State<redis::aio::MultiplexedConnection>,
    Json(payload): Json<LoginRequest>,
) -> (StatusCode, Json<LoginResponse>) {
    info!("Login request for username={}", payload.username);
    let response = LoginResponse {
        token: "fake_token".to_string(),
    };
    (StatusCode::OK, Json(response))
}

fn is_valid_email_regex(email: &str) -> bool {
    lazy_static! {
        static ref RE: Regex =
            Regex::new(r"^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$").unwrap();
    }
    RE.is_match(email)
}

fn is_valid_username(username: &str) -> bool {
    let len = username.chars().count();
    (3..=20).contains(&len)
}

fn is_valid_password(password: &str) -> bool {
    lazy_static! {
        static ref PHC_RE: Regex = Regex::new(
            r"^\$(argon2(id|i|d)|scrypt|pbkdf2-sha(1|256|512))\$[A-Za-z0-9+/=\-_,.]+\$[A-Za-z0-9+/=\-_,.]+$"
        )
        .unwrap();
    }

    PHC_RE.is_match(password)
}

enum DuplicateCheckResult {
    None,
    Email,
    Username,
}

async fn check_account_duplicates(
    con: &mut redis::aio::MultiplexedConnection,
    email_key: &str,
    username_key: &str,
) -> Result<DuplicateCheckResult, redis::RedisError> {
    let email_exists: Option<u64> = con.get(email_key).await?;
    if email_exists.is_some() {
        return Ok(DuplicateCheckResult::Email);
    }

    let username_exists: Option<u64> = con.get(username_key).await?;
    if username_exists.is_some() {
        return Ok(DuplicateCheckResult::Username);
    }

    Ok(DuplicateCheckResult::None)
}

async fn insert_account_hash(
    con: &mut redis::aio::MultiplexedConnection,
    account_key: &str,
    email_key: &str,
    username_key: &str,
    id: u64,
    email: &str,
    username: &str,
    password: &str,
) -> Result<(), redis::RedisError> {
    info!(
        "Inserting account hash: account_key={}, email_key={}, username_key={}, id={}",
        account_key, email_key, username_key, id
    );
    let mut pipe = redis::pipe();
    pipe.atomic()
        .cmd("HSET")
        .arg(account_key)
        .arg("id")
        .arg(id)
        .arg("email")
        .arg(email)
        .arg("username")
        .arg(username)
        .arg("password")
        .arg(password)
        .cmd("SET")
        .arg(email_key)
        .arg(id)
        .cmd("SET")
        .arg(username_key)
        .arg(id);

    pipe.query_async(con).await.map(|_: Vec<redis::Value>| ())
}

async fn create_account(
    State(mut con): State<redis::aio::MultiplexedConnection>,
    Json(payload): Json<CreateAccountRequest>,
) -> (StatusCode, Json<CreateAccountResponse>) {
    info!(
        "Create account request: username={}, email={}",
        payload.username, payload.email
    );
    let response = CreateAccountResponse {
        id: None,
        error: None,
        username: payload.username.clone(),
        password: payload.password.clone(),
        email: payload.email.clone(),
    };

    if !is_valid_email_regex(&payload.email) {
        warn!("Create account rejected: invalid email {}", payload.email);
        return (
            StatusCode::BAD_REQUEST,
            Json(CreateAccountResponse {
                error: Some("Invalid email".to_string()),
                ..response
            }),
        );
    }

    if !is_valid_username(&payload.username) {
        warn!(
            "Create account rejected: invalid username {}",
            payload.username
        );
        return (
            StatusCode::BAD_REQUEST,
            Json(CreateAccountResponse {
                error: Some("Invalid username".to_string()),
                ..response
            }),
        );
    }

    if !is_valid_password(&payload.password) {
        warn!("Create account rejected: invalid password format");
        return (
            StatusCode::BAD_REQUEST,
            Json(CreateAccountResponse {
                error: Some("Invalid password".to_string()),
                ..response
            }),
        );
    }

    let email_key = format!("account:email:{}", payload.email);
    let username_key = format!("account:username:{}", payload.username);
    let id_key = "account:next_id";
    let account_prefix = "account:";

    const MAX_RETRIES: usize = 5;
    let mut attempts = 0;

    let id = loop {
        attempts += 1;

        let watch_result: Result<(), redis::RedisError> = redis::cmd("WATCH")
            .arg(&[&email_key, &username_key])
            .query_async(&mut con)
            .await;

        if let Err(err) = watch_result {
            error!("Redis WATCH failed: {}", err);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(CreateAccountResponse {
                    error: Some(format!("Redis error: {}", err)),
                    ..response
                }),
            );
        }

        let duplicate_check =
            match check_account_duplicates(&mut con, &email_key, &username_key).await {
                Ok(value) => value,
                Err(err) => {
                    error!("Redis read failed: {}", err);
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(CreateAccountResponse {
                            error: Some(format!("Redis error: {}", err)),
                            ..response
                        }),
                    );
                }
            };

        match duplicate_check {
            DuplicateCheckResult::Email => {
                let _ = redis::cmd("UNWATCH").query_async::<()>(&mut con).await;
                info!("Create account rejected: duplicate email {}", payload.email);
                return (
                    StatusCode::CONFLICT,
                    Json(CreateAccountResponse {
                        error: Some("Email is already in use".to_string()),
                        ..response
                    }),
                );
            }
            DuplicateCheckResult::Username => {
                let _ = redis::cmd("UNWATCH").query_async::<()>(&mut con).await;
                info!(
                    "Create account rejected: duplicate username {}",
                    payload.username
                );
                return (
                    StatusCode::CONFLICT,
                    Json(CreateAccountResponse {
                        error: Some("Username is already in use".to_string()),
                        ..response
                    }),
                );
            }
            DuplicateCheckResult::None => {}
        }

        let new_id: u64 = match con.incr(id_key, 1).await {
            Ok(value) => value,
            Err(err) => {
                error!("Redis INCR failed: {}", err);
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(CreateAccountResponse {
                        error: Some(format!("Redis error: {}", err)),
                        ..response
                    }),
                );
            }
        };
        info!("Allocated account id {}", new_id);

        let account_key = format!("{}{}", account_prefix, new_id);
        let exec_result = insert_account_hash(
            &mut con,
            &account_key,
            &email_key,
            &username_key,
            new_id,
            &payload.email,
            &payload.username,
            &payload.password,
        )
        .await;

        match exec_result {
            Ok(_) => {
                info!(
                    "Account created: id={}, username={}",
                    new_id, payload.username
                );
                break new_id;
            }
            Err(err)
                if err.kind() == redis::ErrorKind::Server(redis::ServerErrorKind::ExecAbort) =>
            {
                if attempts >= MAX_RETRIES {
                    error!(
                        "Account creation retry limit reached for username={}",
                        payload.username
                    );
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(CreateAccountResponse {
                            error: Some(
                                "Failed to create account, retry limit reached".to_string(),
                            ),
                            ..response
                        }),
                    );
                }
                warn!("Account creation retry due to transaction abort");
                continue;
            }
            Err(err) => {
                error!("Redis write failed: {}", err);
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(CreateAccountResponse {
                        error: Some(format!("Redis error: {}", err)),
                        ..response
                    }),
                );
            }
        }
    };

    (
        StatusCode::CREATED,
        Json(CreateAccountResponse {
            id: Some(id),
            error: None,
            username: payload.username,
            password: payload.password,
            email: payload.email,
        }),
    )
}
