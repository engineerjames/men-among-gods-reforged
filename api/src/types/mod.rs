use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Serialize)]
pub struct LoginResponse {
    pub token: String,
}

#[derive(Deserialize)]
pub struct CreateAccountRequest {
    pub email: String,
    pub username: String,
    pub password: String,
}

#[derive(Serialize)]
pub struct CreateAccountResponse {
    pub id: Option<u64>,
    pub error: Option<String>,
    pub username: String,
    pub password: String,
    pub email: String,
}

#[derive(Deserialize, Serialize)]
pub struct JwtClaims {
    pub sub: String,
    pub exp: usize,
}

#[derive(Serialize)]
pub struct CharacterSummary {
    pub id: u64,
    pub name: String,
}

#[derive(Serialize)]
pub struct GetCharactersResponse {
    pub characters: Vec<CharacterSummary>,
}
