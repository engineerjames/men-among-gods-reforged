//! Platform-specific utilities (file manager, OS detection, etc.).

use std::path::Path;

/// Open [directory] in the native file manager.
///
/// Uses `open` on macOS, `xdg-open` on Linux, and `explorer` on Windows.
/// Failures are logged but silently ignored so call-sites never panic.
///
/// # Arguments
///
/// * `directory` - The directory path to reveal in the file manager.
pub fn open_directory_in_file_manager(directory: &Path) {
    let program = if cfg!(target_os = "macos") {
        "open"
    } else if cfg!(target_os = "windows") {
        "explorer"
    } else {
        "xdg-open"
    };

    match std::process::Command::new(program).arg(directory).spawn() {
        Ok(_) => log::info!("Opened directory: {}", directory.display()),
        Err(e) => log::warn!(
            "Failed to open directory {} with {}: {}",
            directory.display(),
            program,
            e,
        ),
    }
}
