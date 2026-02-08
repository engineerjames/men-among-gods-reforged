use core::constants::{
    KIN_ARCHHARAKIM, KIN_ARCHTEMPLAR, KIN_FEMALE, KIN_HARAKIM, KIN_MALE, KIN_MERCENARY,
    KIN_SEYAN_DU, KIN_SORCERER, KIN_TEMPLAR, KIN_WARRIOR,
};

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
#[repr(u32)]
pub enum Sex {
    Male = KIN_MALE,
    Female = KIN_FEMALE,
}

#[derive(Serialize)]
#[repr(u32)]
pub enum Race {
    Mercenary = KIN_MERCENARY,
    Templar = KIN_TEMPLAR,
    Harakim = KIN_HARAKIM,
    Sorcerer = KIN_SORCERER,
    Warrior = KIN_WARRIOR,
    ArchTemplar = KIN_ARCHTEMPLAR,
    ArchHarakim = KIN_ARCHHARAKIM,
    SeyanDu = KIN_SEYAN_DU,
}

#[derive(Serialize)]
pub struct CharacterSummary {
    pub id: u64,
    pub name: String,
    pub description: String,
    pub sex: Sex,
    pub race: Race,
}

#[derive(Serialize)]
pub struct GetCharactersResponse {
    pub characters: Vec<CharacterSummary>,
}
