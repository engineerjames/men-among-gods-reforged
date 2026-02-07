pub mod types;

use crate::types::{CreateAccountRequest, CreateAccountResponse, LoginRequest, LoginResponse};
use axum::{extract::State, http::StatusCode, routing::post, Json, Router};
use axum_governor::GovernorLayer;
use lazy_limit::{init_rate_limiter, Duration, RuleConfig};
use lazy_static::lazy_static;
use real::RealIpLayer;
use redis;
use redis::AsyncCommands;
use regex::Regex;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    // 1 req/s globally
    init_rate_limiter!(
        default: RuleConfig::new(Duration::seconds(1), 1)
    )
    .await;

    // TODO: Use Redis connection
    let client = redis::Client::open("redis://127.0.0.1:6379/")?;
    let con = client.get_multiplexed_async_connection().await?;

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

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
    Ok(())
}

async fn login(
    State(_con): State<redis::aio::MultiplexedConnection>,
    Json(_payload): Json<LoginRequest>,
) -> (StatusCode, Json<LoginResponse>) {
    // insert your application logic here
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

enum DuplicateCheck {
    None,
    Email,
    Username,
}

async fn check_account_duplicates(
    con: &mut redis::aio::MultiplexedConnection,
    email_key: &str,
    username_key: &str,
) -> Result<DuplicateCheck, redis::RedisError> {
    let email_exists: Option<u64> = con.get(email_key).await?;
    if email_exists.is_some() {
        return Ok(DuplicateCheck::Email);
    }

    let username_exists: Option<u64> = con.get(username_key).await?;
    if username_exists.is_some() {
        return Ok(DuplicateCheck::Username);
    }

    Ok(DuplicateCheck::None)
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
    let response = CreateAccountResponse {
        id: None,
        error: None,
        username: payload.username.clone(),
        password: payload.password.clone(),
        email: payload.email.clone(),
    };

    if !is_valid_email_regex(&payload.email) {
        return (
            StatusCode::BAD_REQUEST,
            Json(CreateAccountResponse {
                error: Some("Invalid email".to_string()),
                ..response
            }),
        );
    }

    if !is_valid_username(&payload.username) {
        return (
            StatusCode::BAD_REQUEST,
            Json(CreateAccountResponse {
                error: Some("Invalid username".to_string()),
                ..response
            }),
        );
    }

    if !is_valid_password(&payload.password) {
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
            DuplicateCheck::Email => {
                let _ = redis::cmd("UNWATCH").query_async::<()>(&mut con).await;
                return (
                    StatusCode::CONFLICT,
                    Json(CreateAccountResponse {
                        error: Some("Email is already in use".to_string()),
                        ..response
                    }),
                );
            }
            DuplicateCheck::Username => {
                let _ = redis::cmd("UNWATCH").query_async::<()>(&mut con).await;
                return (
                    StatusCode::CONFLICT,
                    Json(CreateAccountResponse {
                        error: Some("Username is already in use".to_string()),
                        ..response
                    }),
                );
            }
            DuplicateCheck::None => {}
        }

        let new_id: u64 = match con.incr(id_key, 1).await {
            Ok(value) => value,
            Err(err) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(CreateAccountResponse {
                        error: Some(format!("Redis error: {}", err)),
                        ..response
                    }),
                );
            }
        };

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
            Ok(_) => break new_id,
            Err(err)
                if err.kind() == redis::ErrorKind::Server(redis::ServerErrorKind::ExecAbort) =>
            {
                if attempts >= MAX_RETRIES {
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
                continue;
            }
            Err(err) => {
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

    // this will be converted into a JSON response
    // with a status code of `201 Created`
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
