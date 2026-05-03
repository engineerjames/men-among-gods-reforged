use mag_core::traits::{self, Class, Sex};
use mag_core::{constants, template_store};

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

/// Retrieves the stored email for an account.
///
/// # Arguments
/// * `con` - Multiplexed KeyDB connection.
/// * `account_id` - Account ID whose email field should be read.
///
/// # Returns
/// * `Ok(Some(email))` if present.
/// * `Ok(None)` if missing.
/// * `Err(redis::RedisError)` on KeyDB failure.
pub(crate) async fn get_account_email(
    con: &mut redis::aio::MultiplexedConnection,
    account_id: u64,
) -> Result<Option<String>, redis::RedisError> {
    let account_key = format!("account:{}", account_id);
    con.hget(&account_key, "email").await
}

/// Updates the password field on an account hash.
///
/// # Arguments
/// * `con` - Multiplexed KeyDB connection.
/// * `account_id` - Account ID whose password should be updated.
/// * `new_password` - New password credential to store.
///
/// # Returns
/// * `Ok(())` on success.
/// * `Err(redis::RedisError)` on KeyDB failure.
pub(crate) async fn set_account_password(
    con: &mut redis::aio::MultiplexedConnection,
    account_id: u64,
    new_password: &str,
) -> Result<(), redis::RedisError> {
    let account_key = format!("account:{}", account_id);
    redis::cmd("HSET")
        .arg(&account_key)
        .arg("password")
        .arg(new_password)
        .query_async::<()>(&mut *con)
        .await
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
/// * `class` - Class enum to store.
///
/// # Returns
/// * `Ok(character_id)` on success.
/// * `Err(redis::RedisError)` on KeyDB failure.
pub(crate) async fn insert_new_character(
    con: &mut redis::aio::MultiplexedConnection,
    account_id: u64,
    name: &str,
    description: Option<&str>,
    sex: traits::Sex,
    class: traits::Class,
) -> Result<u64, redis::RedisError> {
    let character_id: u64 = con.incr("character:next_id", 1).await?;
    let character_key = format!("character:{}", character_id);
    let selection_sprite_id = traits::get_sprite_id_for_class_and_sex(class, sex) as u16;

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
        .arg("class")
        .arg(class as u32)
        .arg("selection_sprite_id")
        .arg(selection_sprite_id)
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

/// Gets the sex and class metadata used to derive game-login race data.
///
/// # Arguments
/// * `con` - Multiplexed KeyDB connection.
/// * `character_id` - Character ID whose login metadata should be read.
///
/// # Returns
/// * `Ok(Some((sex, class)))` if both fields exist and decode to known values.
/// * `Ok(None)` if fields are missing or invalid.
/// * `Err(redis::RedisError)` on KeyDB failure.
pub(crate) async fn get_character_login_traits(
    con: &mut redis::aio::MultiplexedConnection,
    character_id: u64,
) -> Result<Option<(Sex, Class)>, redis::RedisError> {
    let character_key = format!("character:{}", character_id);
    let (sex_value, class_value): (Option<u32>, Option<u32>) = redis::cmd("HMGET")
        .arg(&character_key)
        .arg("sex")
        .arg("class")
        .query_async(&mut *con)
        .await?;

    let Some(sex_value) = sex_value else {
        return Ok(None);
    };
    let Some(class_value) = class_value else {
        return Ok(None);
    };

    let Some(sex) = Sex::from_u32(sex_value) else {
        return Ok(None);
    };
    let Some(class) = Class::from_u32(class_value) else {
        return Ok(None);
    };

    Ok(Some((sex, class)))
}

pub(crate) async fn get_character_name(
    con: &mut redis::aio::MultiplexedConnection,
    character_id: u64,
) -> Result<Option<String>, redis::RedisError> {
    let character_key = format!("character:{}", character_id);
    con.hget(&character_key, "name").await
}

/// Gets the stored description for an API character.
///
/// # Arguments
/// * `con` - Multiplexed KeyDB connection.
/// * `character_id` - Character ID whose `description` field should be read.
///
/// # Returns
/// * `Ok(Some(description))` if present.
/// * `Ok(None)` if missing.
/// * `Err(redis::RedisError)` on KeyDB failure.
pub(crate) async fn get_character_description(
    con: &mut redis::aio::MultiplexedConnection,
    character_id: u64,
) -> Result<Option<String>, redis::RedisError> {
    let character_key = format!("character:{}", character_id);
    con.hget(&character_key, "description").await
}

/// Gets the linked game-server character slot for an API character.
///
/// # Arguments
/// * `con` - Multiplexed KeyDB connection.
/// * `character_id` - API character ID whose `server_id` field should be read.
///
/// # Returns
/// * `Ok(Some(server_id))` if present.
/// * `Ok(None)` if missing.
/// * `Err(redis::RedisError)` on KeyDB failure.
pub(crate) async fn get_character_server_id(
    con: &mut redis::aio::MultiplexedConnection,
    character_id: u64,
) -> Result<Option<u32>, redis::RedisError> {
    let character_key = format!("character:{}", character_id);
    con.hget(&character_key, "server_id").await
}

/// Loads banned character-name patterns from game data.
///
/// # Arguments
/// * `con` - Multiplexed KeyDB connection.
///
/// # Returns
/// * `Ok(Vec<String>)` containing banned name substrings.
/// * `Err(redis::RedisError)` on KeyDB failure.
pub(crate) async fn load_bad_names(
    con: &mut redis::aio::MultiplexedConnection,
) -> Result<Vec<String>, redis::RedisError> {
    let bytes: Vec<u8> = con.get("game:badnames").await?;
    let (bad_names, _consumed): (Vec<String>, usize) =
        bincode::decode_from_slice(&bytes, bincode::config::standard()).map_err(|err| {
            redis::RedisError::from((
                redis::ErrorKind::UnexpectedReturnType,
                "Decode game:badnames failed",
                err.to_string(),
            ))
        })?;
    Ok(bad_names)
}

/// Checks whether a name collides with a character template name.
///
/// # Arguments
/// * `con` - Multiplexed KeyDB connection.
/// * `name` - Normalized character name to compare.
///
/// # Returns
/// * `Ok(true)` if a template has the same name, ignoring ASCII case.
/// * `Ok(false)` otherwise.
/// * `Err(redis::RedisError)` on KeyDB failure.
pub(crate) async fn character_template_name_exists(
    con: &mut redis::aio::MultiplexedConnection,
    name: &str,
) -> Result<bool, redis::RedisError> {
    let target_name = name.trim();
    if target_name.is_empty() {
        return Ok(false);
    }

    let pattern = format!("{}*", template_store::CHARACTER_TEMPLATE_KEY_PREFIX);
    let keys = scan_keys_matching(con, &pattern, 400).await?;
    for key in keys {
        let bytes: Option<Vec<u8>> = con.get(&key).await?;
        let Some(bytes) = bytes else {
            continue;
        };

        let Ok(character) = template_store::decode_character_template(&bytes) else {
            continue;
        };

        if character
            .get_name()
            .trim()
            .eq_ignore_ascii_case(target_name)
        {
            return Ok(true);
        }
    }

    Ok(false)
}

/// Checks whether a linked game character has the `NoDesc` flag set.
///
/// # Arguments
/// * `con` - Multiplexed KeyDB connection.
/// * `server_id` - Game-server character slot ID.
///
/// # Returns
/// * `Ok(true)` if the slot exists and has `NoDesc`.
/// * `Ok(false)` if absent, undecodable, or not flagged.
/// * `Err(redis::RedisError)` on KeyDB failure.
pub(crate) async fn character_slot_has_no_desc(
    con: &mut redis::aio::MultiplexedConnection,
    server_id: u32,
) -> Result<bool, redis::RedisError> {
    let key = format!("game:char:{}", server_id);
    let bytes: Option<Vec<u8>> = con.get(key).await?;
    let Some(bytes) = bytes else {
        return Ok(false);
    };

    let Some(character) = mag_core::types::Character::from_bytes(&bytes) else {
        return Ok(false);
    };

    Ok((character.flags & constants::CharacterFlags::NoDesc.bits()) != 0)
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

/// Counts characters belonging to an account by scanning character hashes.
///
/// This is a lightweight helper for enforcing per-account limits without building additional
/// per-account indexes. It scans `character:*` keys, reads only `account_id` via `HGET`, and
/// stops early once `max` matches are found.
///
/// # Arguments
/// * `con` - Multiplexed KeyDB connection.
/// * `account_id` - Account ID to count characters for.
/// * `max` - Early-stop threshold (returns once count >= max).
///
/// # Returns
/// * `Ok(count)` where `count` is in `0..=max`.
/// * `Err(redis::RedisError)` on KeyDB failure.
pub(crate) async fn count_characters_for_account_scan_up_to(
    con: &mut redis::aio::MultiplexedConnection,
    account_id: u64,
    max: usize,
) -> Result<usize, redis::RedisError> {
    if max == 0 {
        return Ok(0);
    }

    let mut cursor: u64 = 0;
    let mut count: usize = 0;

    loop {
        let (next_cursor, keys): (u64, Vec<String>) = redis::cmd("SCAN")
            .arg(cursor)
            .arg("MATCH")
            .arg("character:*")
            .arg("COUNT")
            .arg(400)
            .query_async(&mut *con)
            .await?;

        for key in keys {
            if key == "character:next_id" {
                continue;
            }

            let owner: Option<u64> = con.hget(&key, "account_id").await?;
            if owner == Some(account_id) {
                count += 1;
                if count >= max {
                    return Ok(count);
                }
            }
        }

        cursor = next_cursor;
        if cursor == 0 {
            break;
        }
    }

    Ok(count)
}

/// Checks whether any existing character already uses `name` (case-insensitive).
///
/// Scans `character:*` keys and compares normalized names using ASCII-insensitive
/// matching. Used by create-character flows to enforce global character-name
/// uniqueness.
pub(crate) async fn character_name_exists_scan(
    con: &mut redis::aio::MultiplexedConnection,
    name: &str,
) -> Result<bool, redis::RedisError> {
    character_name_exists_scan_excluding(con, name, None).await
}

/// Checks whether any existing character (other than `exclude_character_id`) already
/// uses `name` (case-insensitive).
pub(crate) async fn character_name_exists_scan_excluding(
    con: &mut redis::aio::MultiplexedConnection,
    name: &str,
    exclude_character_id: Option<u64>,
) -> Result<bool, redis::RedisError> {
    let target_name = name.trim();
    if target_name.is_empty() {
        return Ok(false);
    }

    let keys = scan_keys_matching(con, "character:*", 400).await?;
    for key in keys {
        if key == "character:next_id" {
            continue;
        }

        if let Some(excluded_id) = exclude_character_id {
            if parse_numeric_id("character:", &key) == Some(excluded_id) {
                continue;
            }
        }

        let existing_name: Option<String> = con.hget(&key, "name").await?;
        if let Some(existing_name) = existing_name {
            if existing_name.trim().eq_ignore_ascii_case(target_name) {
                return Ok(true);
            }
        }
    }

    Ok(false)
}

/// Character-name search hit from account-managed character hashes.
pub(crate) struct CharacterNameSearchMatch {
    /// API character id.
    pub id: u64,
    /// Stored character name.
    pub name: String,
    /// Owning account id, when present.
    pub account_id: Option<u64>,
    /// Owning account username, when present.
    pub account_username: Option<String>,
    /// Last linked live server slot id, when present.
    pub server_id: Option<u32>,
}

/// Search account-managed characters by exact or partial name.
///
/// Exact case-insensitive name matches sort first, followed by partial matches
/// sorted by name and id. This supports admin flows where the operator knows a
/// unique character name but not the API character id used by ban records.
pub(crate) async fn search_characters_by_name_scan(
    con: &mut redis::aio::MultiplexedConnection,
    query: &str,
    limit: usize,
) -> Result<Vec<CharacterNameSearchMatch>, redis::RedisError> {
    let query = query.trim();
    if query.is_empty() || limit == 0 {
        return Ok(Vec::new());
    }

    let query_lc = query.to_ascii_lowercase();
    let keys = scan_keys_matching(con, "character:*", 400).await?;
    let mut matches: Vec<(bool, CharacterNameSearchMatch)> = Vec::new();

    for key in keys {
        if key == "character:next_id" {
            continue;
        }

        let Some(character_id) = parse_numeric_id("character:", &key) else {
            continue;
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

        let Some(name) = character_map.get("name").cloned() else {
            continue;
        };
        let name_lc = name.to_ascii_lowercase();
        let exact = name_lc == query_lc;
        if !exact && !name_lc.contains(&query_lc) {
            continue;
        }

        let account_id = character_map
            .get("account_id")
            .and_then(|value| value.parse::<u64>().ok());
        let account_username = match account_id {
            Some(account_id) => {
                let account_key = format!("account:{}", account_id);
                con.hget(&account_key, "username").await?
            }
            None => None,
        };
        let server_id = character_map
            .get("server_id")
            .and_then(|value| value.parse::<u32>().ok());
        matches.push((
            exact,
            CharacterNameSearchMatch {
                id: character_id,
                name,
                account_id,
                account_username,
                server_id,
            },
        ));
    }

    matches.sort_by(|left, right| {
        right
            .0
            .cmp(&left.0)
            .then_with(|| {
                left.1
                    .name
                    .to_ascii_lowercase()
                    .cmp(&right.1.name.to_ascii_lowercase())
            })
            .then(left.1.id.cmp(&right.1.id))
    });
    Ok(matches
        .into_iter()
        .take(limit)
        .map(|(_, character)| character)
        .collect())
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
        let sex = match Sex::from_u32(sex_value) {
            Some(value) => value,
            None => continue,
        };

        let class_value: u32 = match character_map
            .get("class")
            .and_then(|value| value.parse().ok())
        {
            Some(value) => value,
            None => continue,
        };
        let class = match Class::from_u32(class_value) {
            Some(value) => value,
            None => continue,
        };

        let server_id = character_map
            .get("server_id")
            .and_then(|value| value.parse::<u32>().ok());
        let selection_sprite_id = character_map
            .get("selection_sprite_id")
            .and_then(|value| value.parse::<u16>().ok());

        characters.push(types::CharacterSummary {
            id: character_id,
            name,
            description,
            sex,
            class: class,
            selection_sprite_id,
            server_id,
        });
    }

    Ok(characters)
}
