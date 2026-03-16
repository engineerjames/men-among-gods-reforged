use serde::{Deserialize, Serialize};

pub use crate::traits::{Class, Sex};

/// Client login credentials.
#[derive(Serialize, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

/// JWT token returned on successful login.
#[derive(Serialize, Deserialize)]
pub struct LoginResponse {
    pub token: String,
}

/// New account registration payload.
#[derive(Serialize, Deserialize)]
pub struct CreateAccountRequest {
    pub email: String,
    pub username: String,
    pub password: String,
}

/// Result of creating a new account.
#[derive(Serialize, Deserialize)]
pub struct CreateAccountResponse {
    pub id: Option<u64>,
    pub error: Option<String>,
    pub username: String,
    pub email: String,
}

/// Request a one-time ticket to log into the game server.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CreateGameLoginTicketRequest {
    pub character_id: u64,
}

/// Response containing a one-time game login ticket.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CreateGameLoginTicketResponse {
    pub ticket: Option<u64>,
    pub error: Option<String>,
}

/// Summary of a character owned by an account.
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

    /// Class of the character
    pub class: Class,

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
            class: Class::Mercenary,
            server_id: None,
        }
    }
}

/// List of characters belonging to an account.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GetCharactersResponse {
    pub characters: Vec<CharacterSummary>,
}

/// Request to create a new character.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CreateCharacterRequest {
    pub name: String,
    pub description: Option<String>,
    pub sex: Sex,
    pub class: Class,
}

impl CreateCharacterRequest {
    /// Validates that the requested class is eligible for creation.
    ///
    /// Advanced classes that can only be achieved in-game are rejected.
    ///
    /// # Returns
    ///
    /// * `true` if the request passes validation.
    pub fn validate(&self) -> bool {
        if [
            Class::SeyanDu,
            Class::Sorcerer,
            Class::Warrior,
            Class::ArchHarakim,
            Class::ArchTemplar,
        ]
        .contains(&self.class)
        {
            log::error!(
                "Invalid class selection: {:?}; Can only be achieved in-game.",
                self.class
            );
            return false;
        }

        true
    }
}
