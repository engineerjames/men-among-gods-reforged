use crate::types;
use log::info;
use redis::AsyncCommands;

/// Builds the KeyDB claim key used to enforce username uniqueness and resolve usernames
/// to account IDs.
///
/// # Arguments
/// * `username_lc` - Lowercased username.
///
/// # Returns
/// * Claim key in the form `account:username:{username}`.
fn username_claim_key(username_lc: &str) -> String {
    format!("account:username:{}", username_lc)
}

/// Builds the KeyDB claim key used to enforce email uniqueness.
///
/// # Arguments
/// * `email_lc` - Lowercased email.
///
/// # Returns
/// * Claim key in the form `account:email:{email}`.
fn email_claim_key(email_lc: &str) -> String {
    format!("account:email:{}", email_lc)
}

/// Parses a numeric ID from a KeyDB key suffix.
///
/// This is used to distinguish real object keys like `account:{id}` from metadata keys
/// like `account:next_id`.
///
/// # Arguments
/// * `prefix` - Required key prefix (e.g. `"account:"`).
/// * `key` - Full KeyDB key name to parse.
///
/// # Returns
/// * `Some(id)` when `key` starts with `prefix` and the remainder is all digits.
/// * `None` otherwise.
fn parse_numeric_id(prefix: &str, key: &str) -> Option<u64> {
    let rest = key.strip_prefix(prefix)?;
    if rest.is_empty() || !rest.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    rest.parse::<u64>().ok()
}

/// Scans KeyDB for keys matching a glob-style pattern.
///
/// Uses `SCAN` to avoid blocking the server like `KEYS` would.
///
/// # Arguments
/// * `con` - Multiplexed KeyDB connection.
/// * `pattern` - Glob-style match pattern (e.g. `character:*`).
/// * `count` - SCAN count hint per iteration.
///
/// # Returns
/// * `Ok(Vec<String>)` of matching key names.
/// * `Err(redis::RedisError)` on KeyDB failure.
async fn scan_keys_matching(
    con: &mut redis::aio::MultiplexedConnection,
    pattern: &str,
    count: u32,
) -> Result<Vec<String>, redis::RedisError> {
    let mut cursor: u64 = 0;
    let mut out: Vec<String> = Vec::new();

    loop {
        let (next_cursor, keys): (u64, Vec<String>) = redis::cmd("SCAN")
            .arg(cursor)
            .arg("MATCH")
            .arg(pattern)
            .arg("COUNT")
            .arg(count)
            .query_async(&mut *con)
            .await?;
        out.extend(keys);
        cursor = next_cursor;
        if cursor == 0 {
            break;
        }
    }

    Ok(out)
}

/// Attempts to claim a username for a given account ID.
///
/// This uses a single atomic command: `SET account:username:{username} {account_id} NX`.
///
/// # Arguments
/// * `con` - Multiplexed KeyDB connection.
/// * `username_lc` - Lowercased username to claim.
/// * `account_id` - Account ID to store as the claim value.
///
/// # Returns
/// * `Ok(true)` if the claim was created.
/// * `Ok(false)` if the claim already existed.
/// * `Err(redis::RedisError)` on KeyDB failure.
pub(crate) async fn claim_username(
    con: &mut redis::aio::MultiplexedConnection,
    username_lc: &str,
    account_id: u64,
) -> Result<bool, redis::RedisError> {
    let key = username_claim_key(username_lc);
    let result: Option<String> = redis::cmd("SET")
        .arg(key)
        .arg(account_id)
        .arg("NX")
        .query_async(&mut *con)
        .await?;
    Ok(result.is_some())
}

/// Attempts to claim an email for a given account ID.
///
/// This uses a single atomic command: `SET account:email:{email} {account_id} NX`.
///
/// # Arguments
/// * `con` - Multiplexed KeyDB connection.
/// * `email_lc` - Lowercased email to claim.
/// * `account_id` - Account ID to store as the claim value.
///
/// # Returns
/// * `Ok(true)` if the claim was created.
/// * `Ok(false)` if the claim already existed.
/// * `Err(redis::RedisError)` on KeyDB failure.
pub(crate) async fn claim_email(
    con: &mut redis::aio::MultiplexedConnection,
    email_lc: &str,
    account_id: u64,
) -> Result<bool, redis::RedisError> {
    let key = email_claim_key(email_lc);
    let result: Option<String> = redis::cmd("SET")
        .arg(key)
        .arg(account_id)
        .arg("NX")
        .query_async(&mut *con)
        .await?;
    Ok(result.is_some())
}

/// Releases a claim key if (and only if) its stored account ID matches `account_id`.
///
/// This is used to safely clean up claim keys without deleting another account's claim.
///
/// # Arguments
/// * `con` - Multiplexed KeyDB connection.
/// * `key` - Claim key to release (e.g. `account:username:{username}`).
/// * `account_id` - Account ID expected to be stored at `key`.
///
/// # Returns
/// * `Ok(true)` if the key was deleted.
/// * `Ok(false)` if the key did not match `account_id` or did not exist.
/// * `Err(redis::RedisError)` on KeyDB failure.
pub(crate) async fn release_claim_if_matches(
    con: &mut redis::aio::MultiplexedConnection,
    key: &str,
    account_id: u64,
) -> Result<bool, redis::RedisError> {
    let current: Option<u64> = con.get(key).await?;
    if current != Some(account_id) {
        return Ok(false);
    }

    let deleted: u64 = con.del(key).await?;
    Ok(deleted > 0)
}

/// Resolves an account ID by username using the username claim key.
///
/// # Arguments
/// * `con` - Multiplexed KeyDB connection.
/// * `username_lc` - Lowercased username to resolve.
///
/// # Returns
/// * `Ok(Some(account_id))` if the username is claimed.
/// * `Ok(None)` if the username is not found.
/// * `Err(redis::RedisError)` on KeyDB failure.
pub(crate) async fn get_account_id_by_username(
    con: &mut redis::aio::MultiplexedConnection,
    username_lc: &str,
) -> Result<Option<u64>, redis::RedisError> {
    let key = username_claim_key(username_lc);
    con.get(key).await
}

/// Inserts an account hash into KeyDB.
///
/// This performs a single `HSET` with all fields. Uniqueness is expected to be enforced
/// externally (by username/email claim keys).
///
/// # Arguments
/// * `con` - Multiplexed KeyDB connection.
/// * `id` - Newly allocated account ID.
/// * `email_lc` - Lowercased email to store.
/// * `username_lc` - Lowercased username to store.
/// * `password` - Stored password credential (currently the client-provided PHC string).
///
/// # Returns
/// * `Ok(())` on success.
/// * `Err(redis::RedisError)` on KeyDB failure.
pub(crate) async fn insert_account_hash(
    con: &mut redis::aio::MultiplexedConnection,
    id: u64,
    email_lc: &str,
    username_lc: &str,
    password: &str,
) -> Result<(), redis::RedisError> {
    let account_key = format!("account:{}", id);
    info!(
        "Inserting account hash: account_key={}, id={}, username={} email={}",
        account_key, id, username_lc, email_lc
    );

    // Single write command for the hash.
    redis::cmd("HSET")
        .arg(&account_key)
        .arg("id")
        .arg(id)
        .arg("email")
        .arg(email_lc)
        .arg("username")
        .arg(username_lc)
        .arg("password")
        .arg(password)
        .query_async::<()>(&mut *con)
        .await
}

/// Retrieves the stored password credential for an account.
///
/// # Arguments
/// * `con` - Multiplexed KeyDB connection.
/// * `account_id` - Account ID whose password field should be read.
///
/// # Returns
/// * `Ok(Some(password))` if present.
/// * `Ok(None)` if missing.
/// * `Err(redis::RedisError)` on KeyDB failure.
pub(crate) async fn get_account_password_hash(
    con: &mut redis::aio::MultiplexedConnection,
    account_id: u64,
) -> Result<Option<String>, redis::RedisError> {
    let account_key = format!("account:{}", account_id);
    con.hget(&account_key, "password").await
}

/// Inserts a new character hash owned by an account.
///
/// Allocates an ID via `INCR character:next_id`, then writes the character fields via a
/// single `HSET`.
///
/// # Arguments
/// * `con` - Multiplexed KeyDB connection.
/// * `account_id` - Owning account ID to store in `account_id`.
/// * `name` - Character name.
/// * `description` - Optional character description.
/// * `sex` - Sex enum to store.
/// * `race` - Race enum to store.
///
/// # Returns
/// * `Ok(character_id)` on success.
/// * `Err(redis::RedisError)` on KeyDB failure.
pub(crate) async fn insert_new_character(
    con: &mut redis::aio::MultiplexedConnection,
    account_id: u64,
    name: &str,
    description: Option<&str>,
    sex: types::Sex,
    race: types::Race,
) -> Result<u64, redis::RedisError> {
    let character_id: u64 = con.incr("character:next_id", 1).await?;
    let character_key = format!("character:{}", character_id);

    // Single write command for the character.
    redis::cmd("HSET")
        .arg(&character_key)
        .arg("account_id")
        .arg(account_id)
        .arg("name")
        .arg(name)
        .arg("description")
        .arg(description.unwrap_or(""))
        .arg("sex")
        .arg(sex as u32)
        .arg("race")
        .arg(race as u32)
        .query_async::<()>(&mut *con)
        .await?;

    Ok(character_id)
}

/// Gets the owning account ID for a character.
///
/// # Arguments
/// * `con` - Multiplexed KeyDB connection.
/// * `character_id` - Character ID whose `account_id` field should be read.
///
/// # Returns
/// * `Ok(Some(account_id))` if present.
/// * `Ok(None)` if missing.
/// * `Err(redis::RedisError)` on KeyDB failure.
pub(crate) async fn get_character_account_id(
    con: &mut redis::aio::MultiplexedConnection,
    character_id: u64,
) -> Result<Option<u64>, redis::RedisError> {
    let character_key = format!("character:{}", character_id);
    con.hget(&character_key, "account_id").await
}

/// Updates a character hash by setting any provided fields.
///
/// This issues a single `HSET` containing only the fields that are `Some`.
///
/// # Arguments
/// * `con` - Multiplexed KeyDB connection.
/// * `character_id` - Character ID to update.
/// * `name` - Optional name value.
/// * `description` - Optional description value.
///
/// # Returns
/// * `Ok(())` on success.
/// * `Err(redis::RedisError)` on KeyDB failure.
pub(crate) async fn update_character(
    con: &mut redis::aio::MultiplexedConnection,
    character_id: u64,
    name: Option<&str>,
    description: Option<&str>,
) -> Result<(), redis::RedisError> {
    let character_key = format!("character:{}", character_id);

    // Caller enforces at least one field is present.
    let mut cmd = redis::cmd("HSET");
    cmd.arg(&character_key);
    if let Some(name) = name {
        cmd.arg("name").arg(name);
    }
    if let Some(description) = description {
        cmd.arg("description").arg(description);
    }

    cmd.query_async::<()>(&mut *con).await
}

/// Sets the `server_id` field for a character hash.
///
/// This is written by the game server once it assigns an internal character index.
#[allow(dead_code)]
pub(crate) async fn set_character_server_id(
    con: &mut redis::aio::MultiplexedConnection,
    character_id: u64,
    server_id: u32,
) -> Result<(), redis::RedisError> {
    let character_key = format!("character:{}", character_id);
    redis::cmd("HSET")
        .arg(&character_key)
        .arg("server_id")
        .arg(server_id)
        .query_async::<()>(&mut *con)
        .await
}

/// Deletes a character hash from KeyDB.
///
/// # Arguments
/// * `con` - Multiplexed KeyDB connection.
/// * `character_id` - Character ID to delete.
///
/// # Returns
/// * `Ok(())` on success.
/// * `Err(redis::RedisError)` on KeyDB failure.
pub(crate) async fn delete_character(
    con: &mut redis::aio::MultiplexedConnection,
    character_id: u64,
) -> Result<(), redis::RedisError> {
    let character_key = format!("character:{}", character_id);
    con.del(character_key).await
}

/// Lists all characters belonging to an account by scanning character hashes.
///
/// This is intentionally simple (no per-account character sets). It scans `character:*`
/// keys, reads each hash, and filters by `account_id`.
///
/// # Arguments
/// * `con` - Multiplexed KeyDB connection.
/// * `account_id` - Account ID to filter on.
///
/// # Returns
/// * `Ok(Vec<CharacterSummary>)` for all matching characters.
/// * `Err(redis::RedisError)` on KeyDB failure.
pub(crate) async fn list_characters_for_account_scan(
    con: &mut redis::aio::MultiplexedConnection,
    account_id: u64,
) -> Result<Vec<types::CharacterSummary>, redis::RedisError> {
    let keys = scan_keys_matching(con, "character:*", 400).await?;
    let mut characters: Vec<types::CharacterSummary> = Vec::new();

    for key in keys {
        if key == "character:next_id" {
            continue;
        }

        let character_id = match parse_numeric_id("character:", &key) {
            Some(value) => value,
            None => continue,
        };

        let raw: redis::Value = redis::cmd("HGETALL")
            .arg(&key)
            .query_async(&mut *con)
            .await?;

        let character_map: std::collections::HashMap<String, String> =
            match redis::from_redis_value(raw) {
                Ok(value) => value,
                Err(_) => continue,
            };

        let stored_account_id: u64 = match character_map
            .get("account_id")
            .and_then(|value| value.parse().ok())
        {
            Some(value) => value,
            None => continue,
        };
        if stored_account_id != account_id {
            continue;
        }

        let name = match character_map.get("name") {
            Some(value) => value.clone(),
            None => continue,
        };
        let description = character_map
            .get("description")
            .cloned()
            .unwrap_or_default();

        let sex_value: u32 = match character_map
            .get("sex")
            .and_then(|value| value.parse().ok())
        {
            Some(value) => value,
            None => continue,
        };
        let sex = match types::sex_from_u32(sex_value) {
            Some(value) => value,
            None => continue,
        };

        let race_value: u32 = match character_map
            .get("race")
            .and_then(|value| value.parse().ok())
        {
            Some(value) => value,
            None => continue,
        };
        let race = match types::race_from_u32(race_value) {
            Some(value) => value,
            None => continue,
        };

        let server_id = character_map
            .get("server_id")
            .and_then(|value| value.parse::<u32>().ok());

        characters.push(types::CharacterSummary {
            id: character_id,
            name,
            description,
            sex,
            race,
            server_id,
        });
    }

    Ok(characters)
}
