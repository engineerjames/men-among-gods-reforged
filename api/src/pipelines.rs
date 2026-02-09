use crate::types;
use log::info;
use redis::AsyncCommands;

/// Updates a character hash in KeyDB by setting any provided fields.
/// This performs an atomic pipeline update and only touches fields that are `Some`.
///
/// # Arguments
/// * `con` - Multiplexed KeyDB connection used to execute commands.
/// * `character_id` - The character ID whose hash should be updated.
/// * `name` - Optional new character name to set.
/// * `description` - Optional new character description to set.
///
/// # Returns
/// * `Ok(())` if the update pipeline succeeds.
/// * `Err(redis::RedisError)` if any KeyDB operation fails.
pub(crate) async fn update_character(
    con: &mut redis::aio::MultiplexedConnection,
    character_id: u64,
    name: Option<&str>,
    description: Option<&str>,
) -> Result<(), redis::RedisError> {
    let character_key = format!("character:{}", character_id);

    let mut pipe = redis::pipe();
    pipe.atomic();

    if let Some(name) = name {
        pipe.cmd("HSET").arg(&character_key).arg("name").arg(name);
    }

    if let Some(description) = description {
        pipe.cmd("HSET")
            .arg(&character_key)
            .arg("description")
            .arg(description);
    }

    pipe.query_async(con).await.map(|_: Vec<redis::Value>| ())
}

/// Deletes a character from KeyDB and removes its membership from the owning account set.
/// This performs an atomic pipeline containing `DEL` and `SREM`.
///
/// # Arguments
/// * `con` - Multiplexed KeyDB connection used to execute commands.
/// * `account_id` - Account ID whose `account:{account_id}:characters` set will be updated.
/// * `character_id` - Character ID to delete and remove from the ownership set.
///
/// # Returns
/// * `Ok(())` if the delete/removal pipeline succeeds.
/// * `Err(redis::RedisError)` if any KeyDB operation fails.
pub(crate) async fn delete_character(
    con: &mut redis::aio::MultiplexedConnection,
    account_id: u64,
    character_id: u64,
) -> Result<(), redis::RedisError> {
    let character_key = format!("character:{}", character_id);
    let account_characters_key = format!("account:{}:characters", account_id);

    let mut pipe = redis::pipe();
    pipe.atomic()
        .cmd("DEL")
        .arg(&character_key)
        .cmd("SREM")
        .arg(&account_characters_key)
        .arg(character_id);

    pipe.query_async(con).await.map(|_: Vec<redis::Value>| ())
}

/// Looks up an account ID from the username index key.
/// This expects a KeyDB string key in the form `account:username:{username}`.
///
/// # Arguments
/// * `con` - Multiplexed KeyDB connection used to execute commands.
/// * `username_key` - The full username index key to read.
///
/// # Returns
/// * `Ok(Some(account_id))` if the key exists.
/// * `Ok(None)` if the key does not exist.
/// * `Err(redis::RedisError)` if the KeyDB read fails.
pub(crate) async fn get_account_id_by_username(
    con: &mut redis::aio::MultiplexedConnection,
    username_key: &str,
) -> Result<Option<u64>, redis::RedisError> {
    let account_id: Option<u64> = con.get(username_key).await?;
    Ok(account_id)
}

/// Checks whether a character ID is a member of an account's ownership set.
/// This is used to validate that an account is allowed to modify/delete a character.
///
/// # Arguments
/// * `con` - Multiplexed KeyDB connection used to execute commands.
/// * `account_id` - Account ID whose ownership set will be checked.
/// * `character_id` - Character ID to test for membership.
///
/// # Returns
/// * `Ok(true)` if the character ID is a member of the set.
/// * `Ok(false)` if it is not a member.
/// * `Err(redis::RedisError)` if the KeyDB read fails.
pub(crate) async fn check_character_ownership(
    con: &mut redis::aio::MultiplexedConnection,
    account_id: u64,
    character_id: u64,
) -> Result<bool, redis::RedisError> {
    let account_characters_key = format!("account:{}:characters", account_id);
    let is_member: bool = con.sismember(account_characters_key, character_id).await?;
    Ok(is_member)
}

/// Inserts a new character hash and associates it with an account.
/// This allocates a new character ID, writes `character:{id}` fields, and adds the ID to
/// `account:{account_id}:characters` in an atomic pipeline.
///
/// # Arguments
/// * `con` - Multiplexed KeyDB connection used to execute commands.
/// * `account_id` - Account ID that will own the new character.
/// * `name` - Character name to store.
/// * `description` - Optional character description; stored as an empty string when `None`.
/// * `sex` - Character sex value to store.
/// * `race` - Character race value to store.
///
/// # Returns
/// * `Ok(character_id)` if the character was created successfully.
/// * `Err(redis::RedisError)` if any KeyDB operation fails.
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
    let account_characters_key = format!("account:{}:characters", account_id);

    let mut pipe = redis::pipe();
    pipe.atomic()
        .cmd("HSET")
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
        .cmd("SADD")
        .arg(&account_characters_key)
        .arg(character_id);

    pipe.query_async(con)
        .await
        .map(|_: Vec<redis::Value>| character_id)
}

/// Inserts a new account hash and creates username/email index entries.
/// This writes `account:{id}` fields and sets the `account:email:{email}` and
/// `account:username:{username}` keys to the account ID in an atomic pipeline.
///
/// # Arguments
/// * `con` - Multiplexed KeyDB connection used to execute commands.
/// * `account_key` - The full account hash key (e.g. `account:{id}`).
/// * `email_key` - The email index key (e.g. `account:email:{email}`).
/// * `username_key` - The username index key (e.g. `account:username:{username}`).
/// * `id` - Account ID to store.
/// * `email` - Email address to store in the hash.
/// * `username` - Username to store in the hash.
/// * `password` - Password/hash value to store in the hash.
///
/// # Returns
/// * `Ok(())` if the write pipeline succeeds.
/// * `Err(redis::RedisError)` if any KeyDB operation fails.
pub(crate) async fn insert_account_hash(
    con: &mut redis::aio::MultiplexedConnection,
    account_key: &str,
    email_key: &str,
    username_key: &str,
    id: u64,
    email: &str,
    username: &str,
    password: &str,
) -> Result<(), redis::RedisError> {
    info!(
        "Inserting account hash: account_key={}, email_key={}, username_key={}, id={}",
        account_key, email_key, username_key, id
    );
    let mut pipe = redis::pipe();
    pipe.atomic()
        .cmd("HSET")
        .arg(account_key)
        .arg("id")
        .arg(id)
        .arg("email")
        .arg(email)
        .arg("username")
        .arg(username)
        .arg("password")
        .arg(password)
        .cmd("SET")
        .arg(email_key)
        .arg(id)
        .cmd("SET")
        .arg(username_key)
        .arg(id);

    pipe.query_async(con).await.map(|_: Vec<redis::Value>| ())
}

pub(crate) enum DuplicateCheckResult {
    None,
    Email,
    Username,
}

/// Checks whether an email or username index key already exists.
/// This is used prior to account creation to enforce uniqueness.
///
/// # Arguments
/// * `con` - Multiplexed KeyDB connection used to execute commands.
/// * `email_key` - The email index key to check.
/// * `username_key` - The username index key to check.
///
/// # Returns
/// * `Ok(DuplicateCheckResult::Email)` if the email key exists.
/// * `Ok(DuplicateCheckResult::Username)` if the username key exists.
/// * `Ok(DuplicateCheckResult::None)` if neither key exists.
/// * `Err(redis::RedisError)` if any KeyDB read fails.
pub(crate) async fn check_account_duplicates(
    con: &mut redis::aio::MultiplexedConnection,
    email_key: &str,
    username_key: &str,
) -> Result<DuplicateCheckResult, redis::RedisError> {
    let email_exists: Option<u64> = con.get(email_key).await?;
    if email_exists.is_some() {
        return Ok(DuplicateCheckResult::Email);
    }

    let username_exists: Option<u64> = con.get(username_key).await?;
    if username_exists.is_some() {
        return Ok(DuplicateCheckResult::Username);
    }

    Ok(DuplicateCheckResult::None)
}
