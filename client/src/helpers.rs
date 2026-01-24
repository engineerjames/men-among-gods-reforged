use bevy::prelude::*;
use mag_core::constants::{
    LO_CHALLENGE, LO_EXIT, LO_FAILURE, LO_IDLE, LO_KICKED, LO_NONACTIVE, LO_NOROOM, LO_PARAMS,
    LO_PASSWORD, LO_SHUTDOWN, LO_SLOW, LO_TAVERN, LO_USURP, LO_VERSION,
};
use std::path::Path;
use std::process::Command;

pub fn despawn_tree(entity: Entity, children_q: &Query<&Children>, commands: &mut Commands) {
    if let Ok(children) = children_q.get(entity) {
        for child in children.iter() {
            despawn_tree(child, children_q, commands);
        }
    }
    commands.entity(entity).queue_silenced(|e: EntityWorldMut| {
        e.despawn();
    });
}

pub fn exit_reason_string(code: u32) -> &'static str {
    match code as u8 {
        LO_CHALLENGE => "[LO_CHALLENGE] Challenge failure",
        LO_IDLE => "[LO_IDLE] Player idle too long",
        LO_NOROOM => "[LO_NOROOM] No room left on server",
        LO_PARAMS => "[LO_PARAMS] Invalid parameters",
        LO_NONACTIVE => "[LO_NONACTIVE] Player not active",
        LO_PASSWORD => "[LO_PASSWORD] Invalid password",
        LO_SLOW => "[LO_SLOW] Connection too slow",
        LO_FAILURE => "[LO_FAILURE] Login failure",
        LO_SHUTDOWN => "[LO_SHUTDOWN] Server shutting down",
        LO_TAVERN => "[LO_TAVERN] Returned to tavern",
        LO_VERSION => "[LO_VERSION] Version mismatch",
        LO_EXIT => "[LO_EXIT] Client exit",
        LO_USURP => "[LO_USURP] Logged in elsewhere",
        LO_KICKED => "[LO_KICKED] Kicked from server",
        _ => "[UNKNOWN] Unrecognized reason code",
    }
}

pub fn open_dir_in_file_manager(path: &Path) -> Result<(), String> {
    if !path.exists() {
        std::fs::create_dir_all(path)
            .map_err(|e| format!("Failed to create directory {}: {e}", path.display()))?;
    }

    #[cfg(target_os = "macos")]
    let mut cmd = Command::new("open");
    #[cfg(target_os = "windows")]
    let mut cmd = Command::new("explorer");
    #[cfg(all(unix, not(target_os = "macos")))]
    let mut cmd = Command::new("xdg-open");

    cmd.arg(path.as_os_str())
        .spawn()
        .map_err(|e| format!("Failed to open {}: {e}", path.display()))?;
    Ok(())
}
