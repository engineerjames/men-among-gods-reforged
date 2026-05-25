use core::ranks::points2rank;
use core::traits::{Class, Sex, class_from_kindred, sex_from_kindred};
use core::types::api::GameLoginTicketMetadata;
use core::types::{Character, CharacterSummary};
use redis::Commands;
use std::collections::HashMap;
use std::env;
use std::sync::Once;

static LOAD_DOTENV_ONCE: Once = Once::new();

/// Load project `.env` exactly once for local utility binaries.
///
/// Docker Compose reads `.env` for interpolation, but binaries run directly on
/// the host (e.g. map/template viewers) do not automatically inherit those
/// variables. Loading `.env` here bridges that gap while preserving explicit
/// environment variable overrides.
fn load_dotenv_once() {
    LOAD_DOTENV_ONCE.call_once(|| {
        let _ = dotenvy::dotenv();
    });
}

/// Resolve the KeyDB connection URL from process environment.
///
/// Unit tests intentionally skip `.env` loading so they can control the
/// environment deterministically without host-specific leakage.
///
/// # Arguments
///
/// * `load_dotenv` - Whether to load the project `.env` file first.
///
/// # Returns
///
/// * The resolved connection URL string.
fn keydb_url_with_dotenv(load_dotenv: bool) -> String {
    if load_dotenv {
        load_dotenv_once();
    }

    if let Ok(url) = env::var("MAG_KEYDB_URL") {
        return url;
    }
    if let Ok(password) = env::var("KEYDB_PASSWORD") {
        // Percent-encode the password so special characters don't break the URL.
        let encoded: String = password
            .chars()
            .flat_map(|c| match c {
                'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => {
                    vec![c]
                }
                c => format!("%{:02X}", c as u32).chars().collect(),
            })
            .collect();
        return format!("redis://:{encoded}@127.0.0.1:5556/");
    }
    "redis://127.0.0.1:5556/".to_owned()
}

/// Return the KeyDB connection URL.
///
/// Resolution order:
///
/// 1. `MAG_KEYDB_URL` — used verbatim when set (covers all deployment
///    environments where the full URL including credentials is provided).
/// 2. `KEYDB_PASSWORD` — when only the password is available (e.g. a
///    developer sourcing the project `.env` file before running a local
///    utility against the docker-compose KeyDB), the URL is constructed as
///    `redis://:<password>@127.0.0.1:5556/`.
/// 3. Hard-coded `redis://127.0.0.1:5556/` — unauthenticated local fallback
///    for development environments that run KeyDB without a password.
///
/// # Returns
///
/// * The connection URL string.
pub fn keydb_url() -> String {
    keydb_url_with_dotenv(!cfg!(test))
}

/// Open a synchronous Redis/KeyDB connection.
///
/// Uses the URL returned by [`keydb_url`].
///
/// # Returns
///
/// * `Ok(Connection)` on success.
/// * `Err` with a human-readable message on failure.
pub fn connect() -> Result<redis::Connection, String> {
    let url = keydb_url();
    let client = redis::Client::open(url.as_str())
        .map_err(|err| format!("Failed to open KeyDB client: {err}"))?;
    client
        .get_connection()
        .map_err(|err| format!("Failed to connect to KeyDB: {err}"))
}

/// Load the current game MOTD value from KeyDB.
///
/// Reads the `game:motd` key and returns its UTF-8 string payload.
///
/// # Returns
///
/// * `Ok(String)` with the current MOTD when the key exists.
/// * `Err(String)` when connecting to KeyDB or reading the key fails.
pub fn load_message_of_the_day() -> Result<String, String> {
    let mut con = connect()?;
    con.get("game:motd")
        .map_err(|err| format!("Failed to load game MOTD from KeyDB: {err}"))
}

/// Atomically consumes a one-time game login ticket from KeyDB.
///
/// # Arguments
///
/// * `ticket` - Login ticket value issued by the API.
///
/// # Returns
///
/// * `Ok(Some(metadata))` when the ticket exists and decodes successfully.
/// * `Ok(None)` when `ticket` is zero or no ticket key exists.
/// * `Err(String)` when KeyDB access or metadata decoding fails.
pub fn consume_login_ticket(ticket: u64) -> Result<Option<GameLoginTicketMetadata>, String> {
    if ticket == 0 {
        return Ok(None);
    }

    let mut con = connect()?;
    let key = format!("game_login_ticket:{}", ticket);

    // Use Lua to atomically get and delete the ticket.
    let script =
        "local v = redis.call('GET', KEYS[1]); if v then redis.call('DEL', KEYS[1]); end; return v";

    let value: Option<Vec<u8>> = redis::cmd("EVAL")
        .arg(script)
        .arg(1)
        .arg(&key)
        .query(&mut con)
        .map_err(|err| format!("Failed to consume login ticket: {err}"))?;

    let Some(raw) = value else {
        return Ok(None);
    };

    let metadata = GameLoginTicketMetadata::from_bytes(&raw)
        .map_err(|err| format!("Invalid login ticket metadata: {err}"))?;

    Ok(Some(metadata))
}

/// Loads an account-service character summary from KeyDB.
///
/// # Arguments
///
/// * `character_id` - API character id to load.
///
/// # Returns
///
/// * `Ok(Some(CharacterSummary))` when the character hash exists.
/// * `Ok(None)` when no character hash exists.
/// * `Err(String)` when KeyDB access or hash parsing fails.
pub fn load_character(character_id: u64) -> Result<Option<CharacterSummary>, String> {
    let mut con = connect()?;
    let key = format!("character:{}", character_id);

    let raw: redis::Value = redis::cmd("HGETALL")
        .arg(&key)
        .query(&mut con)
        .map_err(|err| format!("Failed to load character from KeyDB: {err}"))?;

    let character_map: HashMap<String, String> =
        redis::from_redis_value(raw).map_err(|_| "Failed to parse character hash".to_owned())?;

    if character_map.is_empty() {
        return Ok(None);
    }

    let name = character_map
        .get("name")
        .cloned()
        .unwrap_or_else(String::new);
    let description = character_map
        .get("description")
        .cloned()
        .unwrap_or_else(String::new);

    let sex_value = character_map
        .get("sex")
        .and_then(|value| value.parse::<u32>().ok())
        .ok_or_else(|| "Missing character sex".to_owned())?;
    let sex = Sex::from_u32(sex_value).ok_or_else(|| "Invalid character sex".to_owned())?;

    let class_value = character_map
        .get("class")
        .and_then(|value| value.parse::<u32>().ok())
        .ok_or_else(|| "Missing character class".to_owned())?;
    let class = Class::from_u32(class_value).ok_or_else(|| "Invalid character class".to_owned())?;

    let server_id = character_map
        .get("server_id")
        .and_then(|value| value.parse::<u32>().ok());
    let selection_sprite_id = character_map
        .get("selection_sprite_id")
        .and_then(|value| value.parse::<u16>().ok());
    let rank_index = character_map
        .get("rank_index")
        .and_then(|value| value.parse::<u8>().ok());

    Ok(Some(CharacterSummary {
        id: character_id,
        name,
        description,
        sex,
        class,
        selection_sprite_id,
        server_id,
        rank_index,
    }))
}

/// Derives character-selection metadata from a live gameplay character slot.
///
/// # Arguments
///
/// * `character` - Live gameplay character whose class, sex, sprite, and rank should be mirrored.
///
/// # Returns
///
/// * `Some((class, sex, selection_sprite_id, rank_index))` when the live character encodes both
///   class and sex.
/// * `None` when the live character does not contain enough metadata to build a selection record.
fn derive_character_selection_metadata(character: &Character) -> Option<(Class, Sex, u16, u8)> {
    let class = class_from_kindred(character.kindred)?;
    let sex = sex_from_kindred(character.kindred)?;
    let rank_index = points2rank(character.points_tot.max(0) as u32) as u8;
    Some((class, sex, character.sprite, rank_index))
}

/// Writes selection metadata fields for an API-side character hash.
///
/// # Arguments
///
/// * `character_id` - API character ID whose `character:{id}` hash should be updated.
/// * `class` - Current class to expose to the selection screen and login flow.
/// * `sex` - Current sex to expose to the selection screen and login flow.
/// * `selection_sprite_id` - Server-authored sprite ID for selection portraits.
/// * `rank_index` - Rank index (0–23) derived from the character's total experience points.
///
/// # Returns
///
/// * `Ok(())` on success.
/// * `Err(String)` when connecting to KeyDB or updating the hash fails.
fn set_character_selection_metadata(
    character_id: u64,
    class: Class,
    sex: Sex,
    selection_sprite_id: u16,
    rank_index: u8,
) -> Result<(), String> {
    let mut con = connect()?;
    let key = format!("character:{}", character_id);
    redis::cmd("HSET")
        .arg(&key)
        .arg("class")
        .arg(class as u32)
        .arg("sex")
        .arg(sex as u32)
        .arg("selection_sprite_id")
        .arg(selection_sprite_id)
        .arg("rank_index")
        .arg(rank_index)
        .query::<()>(&mut con)
        .map_err(|err| format!("Failed to set character selection metadata: {err}"))
}

/// Derives and writes selection metadata for a live gameplay character.
///
/// # Arguments
///
/// * `character_id` - API character ID whose `character:{id}` hash should be updated.
/// * `character` - Live gameplay character whose class, sex, and sprite should be mirrored.
///
/// # Returns
///
/// * `Ok(())` on success.
/// * `Err(String)` when the metadata cannot be derived or the KeyDB update fails.
pub fn sync_character_selection_metadata(
    character_id: u64,
    character: &Character,
) -> Result<(), String> {
    let (class, sex, selection_sprite_id, rank_index) =
        derive_character_selection_metadata(character)
            .ok_or_else(|| "Failed to derive live character selection metadata".to_owned())?;

    set_character_selection_metadata(character_id, class, sex, selection_sprite_id, rank_index)
}

/// Persists the linked gameplay `server_id` for an API-side character hash.
///
/// # Arguments
///
/// * `character_id` - API character ID whose `character:{id}` hash should be updated.
/// * `server_id` - Internal gameplay slot index stored by the server.
///
/// # Returns
///
/// * `Ok(())` on success.
/// * `Err(String)` when connecting to KeyDB or updating the hash fails.
pub fn set_character_server_id(character_id: u64, server_id: u32) -> Result<(), String> {
    let mut con = connect()?;
    let key = format!("character:{}", character_id);
    redis::cmd("HSET")
        .arg(&key)
        .arg("server_id")
        .arg(server_id)
        .query::<()>(&mut con)
        .map_err(|err| format!("Failed to set character server_id: {err}"))
}

// ---------------------------------------------------------------------------
//  Unit Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use core::traits;
    use core::types::Character;
    use std::ffi::OsString;
    use std::sync::{Mutex, OnceLock};

    /// Global guard to serialize tests that mutate process environment vars.
    fn env_test_guard() -> std::sync::MutexGuard<'static, ()> {
        static ENV_TEST_MUTEX: OnceLock<Mutex<()>> = OnceLock::new();
        ENV_TEST_MUTEX
            .get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
    }

    /// Save and restore KeyDB-related environment variables around a test.
    struct KeyDbEnvScope {
        _guard: std::sync::MutexGuard<'static, ()>,
        mag_keydb_url: Option<OsString>,
        keydb_password: Option<OsString>,
    }

    impl KeyDbEnvScope {
        /// Capture the current KeyDB-related environment variables.
        fn capture() -> Self {
            let guard = env_test_guard();
            Self {
                _guard: guard,
                mag_keydb_url: std::env::var_os("MAG_KEYDB_URL"),
                keydb_password: std::env::var_os("KEYDB_PASSWORD"),
            }
        }

        /// Set `MAG_KEYDB_URL` for the current test scope.
        ///
        /// # Arguments
        ///
        /// * `value` - URL value to install.
        fn set_mag_keydb_url(&self, value: &str) {
            // SAFETY: `KeyDbEnvScope` holds the process-wide env mutex for its lifetime.
            unsafe {
                std::env::set_var("MAG_KEYDB_URL", value);
            }
        }

        /// Remove `MAG_KEYDB_URL` for the current test scope.
        fn remove_mag_keydb_url(&self) {
            // SAFETY: `KeyDbEnvScope` holds the process-wide env mutex for its lifetime.
            unsafe {
                std::env::remove_var("MAG_KEYDB_URL");
            }
        }

        /// Set `KEYDB_PASSWORD` for the current test scope.
        ///
        /// # Arguments
        ///
        /// * `value` - Password value to install.
        fn set_keydb_password(&self, value: &str) {
            // SAFETY: `KeyDbEnvScope` holds the process-wide env mutex for its lifetime.
            unsafe {
                std::env::set_var("KEYDB_PASSWORD", value);
            }
        }

        /// Remove `KEYDB_PASSWORD` for the current test scope.
        fn remove_keydb_password(&self) {
            // SAFETY: `KeyDbEnvScope` holds the process-wide env mutex for its lifetime.
            unsafe {
                std::env::remove_var("KEYDB_PASSWORD");
            }
        }
    }

    impl Drop for KeyDbEnvScope {
        fn drop(&mut self) {
            // SAFETY: `KeyDbEnvScope` still holds the process-wide env mutex during drop.
            unsafe {
                match &self.mag_keydb_url {
                    Some(value) => std::env::set_var("MAG_KEYDB_URL", value),
                    None => std::env::remove_var("MAG_KEYDB_URL"),
                }
                match &self.keydb_password {
                    Some(value) => std::env::set_var("KEYDB_PASSWORD", value),
                    None => std::env::remove_var("KEYDB_PASSWORD"),
                }
            }
        }
    }

    /// `MAG_KEYDB_URL` takes precedence over everything else.
    #[test]
    fn mag_keydb_url_takes_precedence() {
        let env_scope = KeyDbEnvScope::capture();
        env_scope.set_mag_keydb_url("redis://custom-host:1234/");
        env_scope.set_keydb_password("should-be-ignored");
        let url = keydb_url();
        assert_eq!(url, "redis://custom-host:1234/");
    }

    /// When only `KEYDB_PASSWORD` is set the URL is constructed with auth
    /// against the default local address.
    #[test]
    fn keydb_password_constructs_authenticated_url() {
        let env_scope = KeyDbEnvScope::capture();
        env_scope.remove_mag_keydb_url();
        env_scope.set_keydb_password("s3cr3t");
        let url = keydb_url();
        assert_eq!(url, "redis://:s3cr3t@127.0.0.1:5556/");
    }

    /// Special characters in the password are percent-encoded so the URL
    /// remains valid.
    #[test]
    fn keydb_password_special_chars_are_percent_encoded() {
        let env_scope = KeyDbEnvScope::capture();
        env_scope.remove_mag_keydb_url();
        env_scope.set_keydb_password("p@ss:w/rd!");
        let url = keydb_url();
        assert_eq!(url, "redis://:p%40ss%3Aw%2Frd%21@127.0.0.1:5556/");
    }

    /// When neither variable is set the unauthenticated local fallback is
    /// returned.
    #[test]
    fn unauthenticated_fallback_when_no_env_vars() {
        let env_scope = KeyDbEnvScope::capture();
        env_scope.remove_mag_keydb_url();
        env_scope.remove_keydb_password();
        let url = keydb_url();
        assert_eq!(url, "redis://127.0.0.1:5556/");
    }

    #[test]
    fn derive_character_selection_metadata_uses_live_values() {
        let character = Character {
            kindred: (traits::KIN_ARCHTEMPLAR | traits::KIN_FEMALE) as i32,
            sprite: 8144,
            points_tot: 0,
            ..Character::default()
        };

        let (class, sex, sprite, rank_index) =
            derive_character_selection_metadata(&character).unwrap();
        assert_eq!(class, Class::ArchTemplar);
        assert_eq!(sex, Sex::Female);
        assert_eq!(sprite, 8144);
        assert_eq!(rank_index, 0);
    }

    #[test]
    fn derive_character_selection_metadata_requires_class_and_sex_bits() {
        let character = Character {
            kindred: traits::KIN_ARCHTEMPLAR as i32,
            sprite: 8144,
            ..Character::default()
        };

        assert_eq!(derive_character_selection_metadata(&character), None);
    }

    #[test]
    fn derive_character_selection_metadata_rank_index_from_points_tot() {
        let mut character = Character {
            kindred: (traits::KIN_MERCENARY | traits::KIN_MALE) as i32,
            sprite: 0,
            ..Character::default()
        };

        // points_tot = 0 → rank 0 (Private).
        character.points_tot = 0;
        let (_, _, _, rank_index) = derive_character_selection_metadata(&character).unwrap();
        assert_eq!(rank_index, 0);

        // points_tot = 50 → rank 1 (Private First Class) per RANK_THRESHOLDS.
        character.points_tot = 50;
        let (_, _, _, rank_index) = derive_character_selection_metadata(&character).unwrap();
        assert_eq!(rank_index, 1);
    }
}
