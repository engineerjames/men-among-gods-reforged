use crate::types;
use log::info;
use redis::AsyncCommands;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::time::{sleep, Duration};

pub(crate) enum DuplicateCheckResult {
    None,
    Email,
    Username,
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
/// * `con` - Multiplexed KeyDB connection used to issue SCAN commands.
/// * `pattern` - SCAN `MATCH` pattern (e.g. `"account:*"`).
/// * `count` - SCAN `COUNT` hint (not a strict limit).
///
/// # Returns
/// * `Ok(Vec<String>)` with all keys returned by the full scan.
/// * `Err(redis::RedisError)` when KeyDB returns an error.
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
            .query_async(con)
            .await?;

        out.extend(keys);
        if next_cursor == 0 {
            break;
        }
        cursor = next_cursor;
    }

    Ok(out)
}

/// Generates a best-effort unique token value for a lock key.
///
/// This value is stored as the lock key's value so we can safely release only locks
/// we acquired (via a GET-and-compare) without Lua.
fn new_lock_token() -> String {
    let pid = std::process::id();
    let now_ns = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("{}:{}", pid, now_ns)
}

/// Attempts to acquire a short-lived lock using `SET key value NX PX <ttl_ms>`.
///
/// # Arguments
/// * `con` - Multiplexed KeyDB connection used to issue commands.
/// * `key` - Lock key name.
/// * `ttl_ms` - Lock time-to-live in milliseconds.
///
/// # Returns
/// * `Ok(Some(token))` if the lock was acquired; `token` must be used to release.
/// * `Ok(None)` if the lock is already held by someone else.
/// * `Err(redis::RedisError)` on KeyDB failure.
pub(crate) async fn try_acquire_lock(
    con: &mut redis::aio::MultiplexedConnection,
    key: &str,
    ttl_ms: u64,
) -> Result<Option<String>, redis::RedisError> {
    let token = new_lock_token();
    let result: Option<String> = redis::cmd("SET")
        .arg(key)
        .arg(&token)
        .arg("NX")
        .arg("PX")
        .arg(ttl_ms)
        .query_async(&mut *con)
        .await?;

    Ok(result.map(|_| token))
}

/// Acquires a lock with a small retry loop to smooth over brief contention.
///
/// # Arguments
/// * `con` - Multiplexed KeyDB connection used to issue commands.
/// * `key` - Lock key name.
/// * `ttl_ms` - Lock time-to-live in milliseconds.
/// * `max_attempts` - Maximum number of attempts before giving up.
/// * `sleep_ms` - Milliseconds to wait between attempts.
///
/// # Returns
/// * `Ok(Some(token))` if the lock was acquired.
/// * `Ok(None)` if lock could not be acquired after retries.
/// * `Err(redis::RedisError)` on KeyDB failure.
pub(crate) async fn acquire_lock_with_retry(
    con: &mut redis::aio::MultiplexedConnection,
    key: &str,
    ttl_ms: u64,
    max_attempts: usize,
    sleep_ms: u64,
) -> Result<Option<String>, redis::RedisError> {
    for attempt in 0..max_attempts {
        if let Some(token) = try_acquire_lock(con, key, ttl_ms).await? {
            return Ok(Some(token));
        }

        if attempt + 1 < max_attempts {
            sleep(Duration::from_millis(sleep_ms)).await;
        }
    }

    Ok(None)
}

/// Releases a lock if (and only if) the lock value matches the provided token.
///
/// This avoids deleting someone else's lock if our lock expired and was re-acquired.
/// This is not perfectly atomic without Lua, but is safe under the token check.
///
/// # Arguments
/// * `con` - Multiplexed KeyDB connection used to issue commands.
/// * `key` - Lock key name.
/// * `token` - The token returned by `try_acquire_lock`.
///
/// # Returns
/// * `Ok(true)` if the lock was deleted.
/// * `Ok(false)` if the key was missing or owned by a different token.
/// * `Err(redis::RedisError)` on KeyDB failure.
pub(crate) async fn release_lock(
    con: &mut redis::aio::MultiplexedConnection,
    key: &str,
    token: &str,
) -> Result<bool, redis::RedisError> {
    let current: Option<String> = con.get(key).await?;
    if current.as_deref() != Some(token) {
        return Ok(false);
    }

    let deleted: u64 = con.del(key).await?;
    Ok(deleted > 0)
}

/// Checks whether an email or username already exists by scanning account hashes.
///
/// This is intentionally simple (no index keys) and is protected by a short-lived lock
/// at the route layer to ensure uniqueness under concurrency.
///
/// # Arguments
/// * `con` - Multiplexed KeyDB connection.
/// * `email_lc` - Lowercased email to check.
/// * `username_lc` - Lowercased username to check.
///
/// # Returns
/// * `Ok(DuplicateCheckResult::Email)` if the email is already in use.
/// * `Ok(DuplicateCheckResult::Username)` if the username is already in use.
/// * `Ok(DuplicateCheckResult::None)` if neither is in use.
/// * `Err(redis::RedisError)` on KeyDB failure.
pub(crate) async fn check_account_duplicates_scan(
    con: &mut redis::aio::MultiplexedConnection,
    email_lc: &str,
    username_lc: &str,
) -> Result<DuplicateCheckResult, redis::RedisError> {
    // Scan account hashes only: account:{id}
    let keys = scan_keys_matching(con, "account:*", 200).await?;
    for key in keys {
        if key == "account:next_id" {
            continue;
        }
        if parse_numeric_id("account:", &key).is_none() {
            continue;
        }

        let (existing_email, existing_username): (Option<String>, Option<String>) =
            redis::cmd("HMGET")
                .arg(&key)
                .arg(&["email", "username"])
                .query_async(&mut *con)
                .await?;

        if existing_email.as_deref() == Some(email_lc) {
            return Ok(DuplicateCheckResult::Email);
        }
        if existing_username.as_deref() == Some(username_lc) {
            return Ok(DuplicateCheckResult::Username);
        }
    }

    Ok(DuplicateCheckResult::None)
}

/// Resolves an account ID by scanning account hashes for a matching username.
///
/// # Arguments
/// * `con` - Multiplexed KeyDB connection.
/// * `username_lc` - Lowercased username to find.
///
/// # Returns
/// * `Ok(Some(account_id))` if found.
/// * `Ok(None)` if no account matches.
/// * `Err(redis::RedisError)` on KeyDB failure.
pub(crate) async fn find_account_id_by_username_scan(
    con: &mut redis::aio::MultiplexedConnection,
    username_lc: &str,
) -> Result<Option<u64>, redis::RedisError> {
    let keys = scan_keys_matching(con, "account:*", 200).await?;
    for key in keys {
        if key == "account:next_id" {
            continue;
        }
        let account_id = match parse_numeric_id("account:", &key) {
            Some(value) => value,
            None => continue,
        };

        let existing_username: Option<String> = con.hget(&key, "username").await?;
        if existing_username.as_deref() == Some(username_lc) {
            return Ok(Some(account_id));
        }
    }

    Ok(None)
}

/// Inserts an account hash into KeyDB.
///
/// This performs a single `HSET` with all fields. Uniqueness is expected to be enforced
/// externally (by scanning + short-lived lock around the create flow).
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

        characters.push(types::CharacterSummary {
            id: character_id,
            name,
            description,
            sex,
            race,
            server_id: None,
        });
    }

    Ok(characters)
}
