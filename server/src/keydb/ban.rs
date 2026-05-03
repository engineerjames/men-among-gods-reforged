//! Synchronous KeyDB helpers for durable ban enforcement.

use core::ban_store::{BAN_ACTIVE_INDEX_KEY, BAN_VERSION_KEY, BanRecord, BanTarget};
use redis::Commands;

/// Load the active ban record for a target if one exists and has not expired.
///
/// # Arguments
///
/// * `target` - Canonical ban target.
///
/// # Returns
///
/// * `Ok(Some(record))` when an active ban exists.
/// * `Ok(None)` when no active ban exists or the record is expired.
/// * `Err(message)` on KeyDB or decode failure.
pub fn active_ban_for_target(target: &BanTarget) -> Result<Option<BanRecord>, String> {
    let mut con = super::connection::connect()?;
    active_ban_for_target_with_connection(&mut con, target)
}

/// Load an active ban using an existing KeyDB connection.
///
/// # Arguments
///
/// * `con` - Open KeyDB connection.
/// * `target` - Canonical ban target.
///
/// # Returns
///
/// * `Ok(Some(record))` when an active ban exists.
/// * `Ok(None)` when no active ban exists or the record is expired.
/// * `Err(message)` on KeyDB or decode failure.
pub fn active_ban_for_target_with_connection(
    con: &mut redis::Connection,
    target: &BanTarget,
) -> Result<Option<BanRecord>, String> {
    let key = target.active_key();
    let bytes: Option<Vec<u8>> = con
        .get(&key)
        .map_err(|error| format!("failed to read ban {}: {}", key, error))?;
    let Some(bytes) = bytes else {
        return Ok(None);
    };
    let record = BanRecord::from_bytes(&bytes).map_err(|error| error.to_string())?;
    if record.is_active_at(now_secs()) {
        Ok(Some(record))
    } else {
        Ok(None)
    }
}

/// Return whether a target is actively banned.
///
/// # Arguments
///
/// * `target` - Canonical ban target.
///
/// # Returns
///
/// * `Ok(true)` when an active ban exists.
/// * `Ok(false)` when no active ban exists.
/// * `Err(message)` on KeyDB or decode failure.
pub fn target_is_banned(target: &BanTarget) -> Result<bool, String> {
    active_ban_for_target(target).map(|record| record.is_some())
}

/// Store or replace the active durable ban for a target.
///
/// # Arguments
///
/// * `record` - Ban record to persist.
///
/// # Returns
///
/// * `Ok(version)` with the updated ban-store version.
/// * `Err(message)` on KeyDB or encode failure.
pub fn upsert_ban_record(record: &BanRecord) -> Result<u64, String> {
    let mut con = super::connection::connect()?;
    let key = record.target.active_key();
    let bytes = record.to_bytes().map_err(|error| error.to_string())?;
    con.set::<_, _, ()>(&key, bytes)
        .map_err(|error| format!("failed to write ban {}: {}", key, error))?;
    con.sadd::<_, _, ()>(BAN_ACTIVE_INDEX_KEY, &key)
        .map_err(|error| format!("failed to index ban {}: {}", key, error))?;
    con.incr(BAN_VERSION_KEY, 1)
        .map_err(|error| format!("failed to bump ban version: {}", error))
}

/// Remove the active durable ban for a target.
///
/// # Arguments
///
/// * `target` - Canonical ban target to remove.
///
/// # Returns
///
/// * `Ok(true)` when a record was deleted.
/// * `Ok(false)` when no active record existed.
/// * `Err(message)` on KeyDB failure.
pub fn remove_ban_target(target: &BanTarget) -> Result<bool, String> {
    let mut con = super::connection::connect()?;
    let key = target.active_key();
    let removed: usize = con
        .del(&key)
        .map_err(|error| format!("failed to delete ban {}: {}", key, error))?;
    con.srem::<_, _, ()>(BAN_ACTIVE_INDEX_KEY, &key)
        .map_err(|error| format!("failed to unindex ban {}: {}", key, error))?;
    con.incr::<_, _, u64>(BAN_VERSION_KEY, 1)
        .map_err(|error| format!("failed to bump ban version: {}", error))?;
    Ok(removed > 0)
}

/// Return the current Unix timestamp in seconds.
///
/// # Returns
///
/// * Seconds since the Unix epoch, or zero if the system clock is before it.
pub fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expired_record_is_inactive_by_model() {
        let record = BanRecord {
            id: "ban-1".to_string(),
            target: BanTarget::Account { account_id: 1 },
            reason: String::new(),
            created_by: "test".to_string(),
            created_at: 1,
            expires_at: Some(2),
            source: "test".to_string(),
        };

        assert!(!record.is_active_at(2));
    }
}
