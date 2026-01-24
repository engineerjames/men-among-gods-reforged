use bevy::prelude::*;
use mag_core::constants::{
    LO_CHALLENGE, LO_EXIT, LO_FAILURE, LO_IDLE, LO_KICKED, LO_NONACTIVE, LO_NOROOM, LO_PARAMS,
    LO_PASSWORD, LO_SHUTDOWN, LO_SLOW, LO_TAVERN, LO_USURP, LO_VERSION,
};
use std::path::{Path, PathBuf};
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

/// Attempts to determine the base directory for Men Among Gods data files.
/// This is where we place the settings.json file, and logs.
pub fn get_mag_base_dir() -> Option<PathBuf> {
    let suffix = PathBuf::from(".men-among-gods");

    let debug_or_release = if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    };

    let is_windows = cfg!(target_os = "windows");

    // First, check if we are running in a development environment
    // This should give us a directory in target/{debug|release}
    let cargo_directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    if cargo_directory.exists() {
        return Some(
            cargo_directory
                .join("..")
                .join("target")
                .join(debug_or_release),
        );
    }

    // Next, check standard user directories for Unix/Mac OS/Linux
    if !is_windows {
        let environment_vars = ["HOME", "XDG_CONFIG_HOME", "XDG_DATA_HOME"];
        for var in environment_vars.iter() {
            if let Ok(home) = std::env::var(var) {
                return Some(PathBuf::from(home).join(suffix));
            }
        }
    } else {
        // Finally, check APPDATA on Windows
        if let Ok(appdata) = std::env::var("APPDATA") {
            return Some(PathBuf::from(appdata).join(suffix));
        }
    }

    None
}
