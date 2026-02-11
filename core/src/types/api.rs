use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Serialize, Deserialize)]
pub struct LoginResponse {
    pub token: String,
}

#[derive(Serialize, Deserialize)]
pub struct CreateAccountRequest {
    pub email: String,
    pub username: String,
    pub password: String,
}

#[derive(Serialize, Deserialize)]
pub struct CreateAccountResponse {
    pub id: Option<u64>,
    pub error: Option<String>,
    pub username: String,
    pub email: String,
}
