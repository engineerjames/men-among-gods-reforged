use serde::{Deserialize, Serialize};

pub use core::types::api::{
    CharacterSummary, CreateAccountRequest, CreateAccountResponse, CreateCharacterRequest,
    GetCharactersResponse, LoginRequest, LoginResponse, Race, Sex,
};

#[derive(Deserialize, Serialize)]
pub struct JwtClaims {
    pub sub: String,
    pub exp: usize,
}

pub(crate) fn sex_from_u32(value: u32) -> Option<Sex> {
    match value {
        value if value == Sex::Male as u32 => Some(Sex::Male),
        value if value == Sex::Female as u32 => Some(Sex::Female),
        _ => None,
    }
}

pub(crate) fn race_from_u32(value: u32) -> Option<Race> {
    match value {
        value if value == Race::Mercenary as u32 => Some(Race::Mercenary),
        value if value == Race::Templar as u32 => Some(Race::Templar),
        value if value == Race::Harakim as u32 => Some(Race::Harakim),
        value if value == Race::Sorcerer as u32 => Some(Race::Sorcerer),
        value if value == Race::Warrior as u32 => Some(Race::Warrior),
        value if value == Race::ArchTemplar as u32 => Some(Race::ArchTemplar),
        value if value == Race::ArchHarakim as u32 => Some(Race::ArchHarakim),
        value if value == Race::SeyanDu as u32 => Some(Race::SeyanDu),
        _ => None,
    }
}

#[derive(Deserialize)]
pub struct UpdateCharacterRequest {
    pub name: Option<String>,
    pub description: Option<String>,
}

#[derive(Deserialize)]
pub struct DeleteCharacterRequest {
    pub id: u64,
}
