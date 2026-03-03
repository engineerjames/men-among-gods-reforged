use core::traits::{Class, Sex};
use core::types::CharacterSummary;
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
    load_dotenv_once();

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
    "redis://127.0.0.1:5556/".to_string()
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

pub fn consume_login_ticket(ticket: u64) -> Result<Option<u64>, String> {
    if ticket == 0 {
        return Ok(None);
    }

    let mut con = connect()?;
    let key = format!("game_login_ticket:{}", ticket);

    // Use Lua to atomically get and delete the ticket.
    let script =
        "local v = redis.call('GET', KEYS[1]); if v then redis.call('DEL', KEYS[1]); end; return v";

    let value: Option<String> = redis::cmd("EVAL")
        .arg(script)
        .arg(1)
        .arg(&key)
        .query(&mut con)
        .map_err(|err| format!("Failed to consume login ticket: {err}"))?;

    let Some(raw) = value else {
        return Ok(None);
    };

    let character_id = raw
        .trim()
        .parse::<u64>()
        .map_err(|_| "Invalid login ticket value".to_string())?;

    Ok(Some(character_id))
}

pub fn load_character(character_id: u64) -> Result<Option<CharacterSummary>, String> {
    let mut con = connect()?;
    let key = format!("character:{}", character_id);

    let raw: redis::Value = redis::cmd("HGETALL")
        .arg(&key)
        .query(&mut con)
        .map_err(|err| format!("Failed to load character from KeyDB: {err}"))?;

    let character_map: HashMap<String, String> =
        redis::from_redis_value(raw).map_err(|_| "Failed to parse character hash".to_string())?;

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
        .ok_or_else(|| "Missing character sex".to_string())?;
    let sex = Sex::from_u32(sex_value).ok_or_else(|| "Invalid character sex".to_string())?;

    let class_value = character_map
        .get("class")
        .and_then(|value| value.parse::<u32>().ok())
        .ok_or_else(|| "Missing character class".to_string())?;
    let class =
        Class::from_u32(class_value).ok_or_else(|| "Invalid character class".to_string())?;

    let server_id = character_map
        .get("server_id")
        .and_then(|value| value.parse::<u32>().ok());

    let id = character_map
        .get("id")
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(0);

    Ok(Some(CharacterSummary {
        id,
        name,
        description,
        sex,
        class,
        server_id,
    }))
}

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
    use std::sync::{Mutex, OnceLock};

    /// Global guard to serialize tests that mutate process environment vars.
    fn env_test_guard() -> std::sync::MutexGuard<'static, ()> {
        static ENV_TEST_MUTEX: OnceLock<Mutex<()>> = OnceLock::new();
        ENV_TEST_MUTEX
            .get_or_init(|| Mutex::new(()))
            .lock()
            .expect("env test mutex poisoned")
    }

    /// `MAG_KEYDB_URL` takes precedence over everything else.
    #[test]
    fn mag_keydb_url_takes_precedence() {
        let _guard = env_test_guard();
        std::env::set_var("MAG_KEYDB_URL", "redis://custom-host:1234/");
        std::env::set_var("KEYDB_PASSWORD", "should-be-ignored");
        let url = keydb_url();
        std::env::remove_var("MAG_KEYDB_URL");
        std::env::remove_var("KEYDB_PASSWORD");
        assert_eq!(url, "redis://custom-host:1234/");
    }

    /// When only `KEYDB_PASSWORD` is set the URL is constructed with auth
    /// against the default local address.
    #[test]
    fn keydb_password_constructs_authenticated_url() {
        let _guard = env_test_guard();
        std::env::remove_var("MAG_KEYDB_URL");
        std::env::set_var("KEYDB_PASSWORD", "s3cr3t");
        let url = keydb_url();
        std::env::remove_var("KEYDB_PASSWORD");
        assert_eq!(url, "redis://:s3cr3t@127.0.0.1:5556/");
    }

    /// Special characters in the password are percent-encoded so the URL
    /// remains valid.
    #[test]
    fn keydb_password_special_chars_are_percent_encoded() {
        let _guard = env_test_guard();
        std::env::remove_var("MAG_KEYDB_URL");
        std::env::set_var("KEYDB_PASSWORD", "p@ss:w/rd!");
        let url = keydb_url();
        std::env::remove_var("KEYDB_PASSWORD");
        assert_eq!(url, "redis://:p%40ss%3Aw%2Frd%21@127.0.0.1:5556/");
    }

    /// When neither variable is set the unauthenticated local fallback is
    /// returned.
    #[test]
    fn unauthenticated_fallback_when_no_env_vars() {
        let _guard = env_test_guard();
        std::env::remove_var("MAG_KEYDB_URL");
        std::env::remove_var("KEYDB_PASSWORD");
        let url = keydb_url();
        assert_eq!(url, "redis://127.0.0.1:5556/");
    }
}
