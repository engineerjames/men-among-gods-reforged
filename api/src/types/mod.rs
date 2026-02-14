use serde::{Deserialize, Serialize};

pub use core::types::api::{
    CharacterSummary, CreateAccountRequest, CreateAccountResponse, CreateCharacterRequest,
    CreateGameLoginTicketRequest, CreateGameLoginTicketResponse, GetCharactersResponse,
    LoginRequest, LoginResponse,
};

#[derive(Deserialize, Serialize)]
pub struct JwtClaims {
    pub sub: String,
    pub exp: usize,
}

#[derive(Deserialize)]
pub struct UpdateCharacterRequest {
    pub name: Option<String>,
    pub description: Option<String>,
}
