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

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
#[repr(u32)]
pub enum Sex {
    Male = KIN_MALE,
    Female = KIN_FEMALE,
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Debug, Clone, Copy)]
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

#[derive(Serialize)]
pub struct CharacterSummary {
    pub id: u64,
    pub name: String,
    pub description: String,
    pub sex: Sex,
    pub race: Race,
}

impl Default for CharacterSummary {
    fn default() -> Self {
        Self {
            id: 0,
            name: String::new(),
            description: String::new(),
            sex: Sex::Male,
            race: Race::Mercenary,
        }
    }
}

#[derive(Serialize)]
pub struct GetCharactersResponse {
    pub characters: Vec<CharacterSummary>,
}

#[derive(Deserialize, Debug)]
pub struct CreateCharacterRequest {
    pub name: String,
    pub description: Option<String>,
    pub sex: Sex,
    pub race: Race,
}

impl CreateCharacterRequest {
    pub fn validate(&self) -> bool {
        if [
            Race::SeyanDu,
            Race::Sorcerer,
            Race::Warrior,
            Race::ArchHarakim,
            Race::ArchTemplar,
        ]
        .contains(&self.race)
        {
            log::error!(
                "Invalid race selection: {:?}; Can only be achieved in-game.",
                self.race
            );
            return false;
        }

        true
    }
}

#[derive(Deserialize)]
pub struct UpdateCharacterRequest {
    pub name: String,
    pub description: String,
}

#[derive(Serialize)]
pub struct UpdateCharacterResponse {
    pub id: u64,
    pub error: Option<String>,
    pub name: String,
    pub description: String,
}

#[derive(Deserialize)]
pub struct DeleteCharacterRequest {
    pub id: u64,
}

#[derive(Serialize)]
pub struct DeleteCharacterResponse {
    pub id: u64,
    pub error: Option<String>,
}
