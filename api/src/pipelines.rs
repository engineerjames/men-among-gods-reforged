use crate::types;
use log::info;
use redis::AsyncCommands;

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
