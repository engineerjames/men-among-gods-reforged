pub mod types;

use crate::types::{CreateAccountRequest, CreateAccountResponse, LoginRequest, LoginResponse};
use axum::{http::StatusCode, routing::post, Json, Router};
use axum_governor::GovernorLayer;
use lazy_limit::{init_rate_limiter, Duration, RuleConfig};
use lazy_static::lazy_static;
use real::RealIpLayer;
use redis;
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
    let mut _con = client.get_multiplexed_async_connection().await?;

    // build our application with a route
    let app = Router::new()
        .route("/login", post(login))
        .route("/accounts", post(create_account))
        .layer(
            tower::ServiceBuilder::new()
                .layer(RealIpLayer::default())
                .layer(GovernorLayer::default()),
        );

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
    Ok(())
}

async fn login(Json(_payload): Json<LoginRequest>) -> (StatusCode, Json<LoginResponse>) {
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
    password.chars().count() >= 8
        && password.chars().any(|c| c.is_uppercase())
        && password.chars().any(|c| c.is_lowercase())
        && password.chars().any(|c| c.is_digit(10))
        && password.chars().any(|c| !c.is_alphanumeric())
}

async fn create_account(
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

    // TODO: Actually insert the account in the database

    // this will be converted into a JSON response
    // with a status code of `201 Created`
    (
        StatusCode::CREATED,
        Json(CreateAccountResponse {
            id: Some(1),
            error: None,
            username: payload.username,
            password: payload.password,
            email: payload.email,
        }),
    )
}
