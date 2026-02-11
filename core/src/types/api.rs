use serde::{Deserialize, Serialize};

use crate::constants::{
    KIN_ARCHHARAKIM, KIN_ARCHTEMPLAR, KIN_FEMALE, KIN_HARAKIM, KIN_MALE, KIN_MERCENARY,
    KIN_SEYAN_DU, KIN_SORCERER, KIN_TEMPLAR, KIN_WARRIOR,
};

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

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CreateGameLoginTicketRequest {
    pub character_id: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CreateGameLoginTicketResponse {
    pub ticket: Option<u64>,
    pub error: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
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

// TODO: Set max lengths for name and description, and enforce them in the database and API validation
#[allow(dead_code)]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CharacterSummary {
    /// Unique character ID assigned by the database
    pub id: u64,

    /// Character name
    pub name: String,

    /// Character description
    pub description: String,

    /// Male or Female
    pub sex: Sex,

    /// Race of the character
    pub race: Race,

    /// Server id
    pub server_id: Option<u32>,
}

impl Default for CharacterSummary {
    fn default() -> Self {
        Self {
            id: 0,
            name: String::new(),
            description: String::new(),
            sex: Sex::Male,
            race: Race::Mercenary,
            server_id: None,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GetCharactersResponse {
    pub characters: Vec<CharacterSummary>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
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
