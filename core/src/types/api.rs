use bincode::{Decode, Encode};
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
    /// JWT bearer token. Empty when login fails.
    pub token: String,
    /// Optional player-facing error message when login fails.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
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
    /// API character ID the client wants to enter the game with.
    pub character_id: u64,
    /// Client protocol version reported before the game-server TCP login.
    pub client_version: u32,
}

/// Response containing a one-time game login ticket.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CreateGameLoginTicketResponse {
    pub ticket: Option<u64>,
    pub error: Option<String>,
}

/// Metadata stored behind a short-lived game login ticket.
///
/// The API writes this payload into KeyDB after authenticating account
/// ownership and validating protocol compatibility. The game server consumes
/// it atomically during TCP login and uses it to initialize the same transient
/// player fields previously carried by the challenge response packet.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Encode, Decode)]
pub struct GameLoginTicketMetadata {
    /// API account ID that owns the authorized character.
    pub account_id: u64,
    /// API character ID authorized for the one-time game login.
    pub character_id: u64,
    /// Client protocol version accepted by the API.
    pub client_version: u32,
    /// Server-derived character race/template integer for login state.
    pub race: i32,
}

impl GameLoginTicketMetadata {
    /// Encodes this ticket metadata to the canonical bincode byte representation.
    ///
    /// # Returns
    ///
    /// * `Ok(Vec<u8>)` containing the encoded payload.
    /// * `Err(bincode::error::EncodeError)` when bincode encoding fails.
    pub fn to_bytes(&self) -> Result<Vec<u8>, bincode::error::EncodeError> {
        bincode::encode_to_vec(self, bincode::config::standard())
    }

    /// Decodes ticket metadata from the canonical bincode byte representation.
    ///
    /// # Arguments
    ///
    /// * `bytes` - Raw bincode bytes loaded from KeyDB.
    ///
    /// # Returns
    ///
    /// * `Ok(GameLoginTicketMetadata)` when decoding consumes the entire input.
    /// * `Err(bincode::error::DecodeError)` when decoding fails or trailing bytes remain.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, bincode::error::DecodeError> {
        let (metadata, consumed): (Self, usize) =
            bincode::decode_from_slice(bytes, bincode::config::standard())?;
        if consumed != bytes.len() {
            return Err(bincode::error::DecodeError::OtherString(
                "trailing bytes in game login ticket metadata".to_owned(),
            ));
        }
        Ok(metadata)
    }
}

/// Summary of a character owned by an account.
// TODO: Set max lengths for name and description, and enforce them in the database and API validation
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CharacterSummary {
    /// Unique character ID assigned by the database
    pub id: u64,

    /// Character name
    pub name: String,

    /// Character description
    pub description: String,

    /// Current sex used for character selection and game login.
    pub sex: Sex,

    /// Current class used for character selection and game login.
    pub class: Class,

    /// Server-authored sprite ID for the character-selection screen.
    ///
    /// This is optional during rollout so older KeyDB hashes can still be
    /// loaded before the selection metadata backfill has been applied.
    pub selection_sprite_id: Option<u16>,

    /// Server id
    pub server_id: Option<u32>,

    /// Rank index (0–23) derived from `points_tot` by the server, written to the
    /// `character:{id}` hash when selection metadata is synced.  `None` for
    /// characters that have never been loaded by the game server.
    #[serde(default)]
    pub rank_index: Option<u8>,
}

impl Default for CharacterSummary {
    fn default() -> Self {
        Self {
            id: 0,
            name: String::new(),
            description: String::new(),
            sex: Sex::Male,
            class: Class::Mercenary,
            selection_sprite_id: None,
            server_id: None,
            rank_index: None,
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

/// Request to initiate a password reset by providing username and email.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ResetPasswordRequest {
    pub username: String,
    pub email: String,
}

/// Response to a password reset request.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ResetPasswordRequestResponse {
    pub message: String,
}

/// Confirms a password reset using the emailed code and a new password.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ResetPasswordConfirm {
    pub username: String,
    pub code: String,
    pub new_password: String,
}

/// Response to a password reset confirmation.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ResetPasswordConfirmResponse {
    pub message: String,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn character_summary_rank_index_serde_roundtrip() {
        let original = CharacterSummary {
            id: 42,
            name: "TestChar".to_string(),
            description: "desc".to_string(),
            sex: Sex::Female,
            class: Class::Harakim,
            selection_sprite_id: Some(4048),
            server_id: Some(7),
            rank_index: Some(5),
        };
        let json = serde_json::to_string(&original).expect("serialize");
        let decoded: CharacterSummary = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(decoded.rank_index, Some(5));
        assert_eq!(decoded.id, 42);
    }

    #[test]
    fn character_summary_rank_index_none_serde_roundtrip() {
        let original = CharacterSummary::default();
        let json = serde_json::to_string(&original).expect("serialize");
        let decoded: CharacterSummary = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(decoded.rank_index, None);
    }

    #[test]
    fn character_summary_missing_rank_index_deserializes_as_none() {
        // Simulate an older API response that omits rank_index.
        let json = r#"{
            "id": 1,
            "name": "OldChar",
            "description": "",
            "sex": "Male",
            "class": "Mercenary",
            "selection_sprite_id": null,
            "server_id": null
        }"#;
        let decoded: CharacterSummary = serde_json::from_str(json).expect("deserialize");
        assert_eq!(decoded.rank_index, None);
    }

    #[test]
    fn game_login_ticket_metadata_bincode_roundtrip() {
        let metadata = GameLoginTicketMetadata {
            account_id: 7,
            character_id: 42,
            client_version: 0x020E07,
            race: 12,
        };

        let encoded = metadata.to_bytes().expect("metadata should encode");
        let decoded =
            GameLoginTicketMetadata::from_bytes(&encoded).expect("metadata should decode");

        assert_eq!(decoded, metadata);
    }

    #[test]
    fn game_login_ticket_metadata_rejects_trailing_bytes() {
        let metadata = GameLoginTicketMetadata {
            account_id: 7,
            character_id: 42,
            client_version: 0x020E07,
            race: 12,
        };
        let mut encoded = metadata.to_bytes().expect("metadata should encode");
        encoded.push(0);

        assert!(GameLoginTicketMetadata::from_bytes(&encoded).is_err());
    }
}
