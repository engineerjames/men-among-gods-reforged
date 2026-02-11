use std::collections::HashMap;
use std::env;

use core::types::api::{Race, Sex};

const DEFAULT_KEYDB_URL: &str = "redis://127.0.0.1:5556/";

fn keydb_url() -> String {
    env::var("MAG_KEYDB_URL").unwrap_or_else(|_| DEFAULT_KEYDB_URL.to_string())
}

fn connect() -> Result<redis::Connection, String> {
    let url = keydb_url();
    let client = redis::Client::open(url.as_str())
        .map_err(|err| format!("Failed to open KeyDB client: {err}"))?;
    client
        .get_connection()
        .map_err(|err| format!("Failed to connect to KeyDB: {err}"))
}

fn sex_from_u32(value: u32) -> Option<Sex> {
    match value {
        value if value == Sex::Male as u32 => Some(Sex::Male),
        value if value == Sex::Female as u32 => Some(Sex::Female),
        _ => None,
    }
}

fn race_from_u32(value: u32) -> Option<Race> {
    match value {
        value if value == Race::Mercenary as u32 => Some(Race::Mercenary),
        value if value == Race::Templar as u32 => Some(Race::Templar),
        value if value == Race::Harakim as u32 => Some(Race::Harakim),
        value if value == Race::Sorcerer as u32 => Some(Race::Sorcerer),
        value if value == Race::Warrior as u32 => Some(Race::Warrior),
        value if value == Race::ArchTemplar as u32 => Some(Race::ArchTemplar),
        value if value == Race::ArchHarakim as u32 => Some(Race::ArchHarakim),
        value if value == Race::SeyanDu as u32 => Some(Race::SeyanDu),
        _ => None,
    }
}

#[derive(Debug, Clone)]
pub struct KeydbCharacter {
    pub name: String,
    pub description: String,
    pub sex: Sex,
    pub race: Race,
    pub server_id: Option<u32>,
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

pub fn load_character(character_id: u64) -> Result<Option<KeydbCharacter>, String> {
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
    let sex = sex_from_u32(sex_value).ok_or_else(|| "Invalid character sex".to_string())?;

    let race_value = character_map
        .get("race")
        .and_then(|value| value.parse::<u32>().ok())
        .ok_or_else(|| "Missing character race".to_string())?;
    let race = race_from_u32(race_value).ok_or_else(|| "Invalid character race".to_string())?;

    let server_id = character_map
        .get("server_id")
        .and_then(|value| value.parse::<u32>().ok());

    Ok(Some(KeydbCharacter {
        name,
        description,
        sex,
        race,
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
