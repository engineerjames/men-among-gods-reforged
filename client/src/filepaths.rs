use std::path::PathBuf;

/// Returns the directory containing the running executable.
///
/// Falls back to `"."` if `current_exe()` cannot be resolved (should be rare).
fn exe_directory() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."))
}

/// Returns the base asset directory for the client.
///
/// When `CARGO_MANIFEST_DIR` is set (i.e. during a `cargo run`), the path is
/// resolved relative to the crate root so that assets are found without
/// copying them next to the debug binary.
///
/// In all other cases (installed binary or macOS .app bundle) assets are
/// expected to sit in `assets/` next to the executable. Using the real
/// executable path rather than the current working directory is essential for
/// macOS .app bundles: when an app is double-clicked (or opened with `open`)
/// the OS sets the CWD to `/`, not to `Contents/MacOS/`.
///
/// # Returns
/// * `PathBuf` pointing to `assets/`.
fn get_asset_directory() -> PathBuf {
    if std::env::var("CARGO_MANIFEST_DIR").is_ok() {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets")
    } else {
        exe_directory().join("assets")
    }
}

/// Returns the path to the `images.zip` sprite archive.
///
/// # Returns
/// * `PathBuf` pointing to `<asset_dir>/gfx/images.zip`.
pub fn get_gfx_zipfile() -> PathBuf {
    let zip_file_path = get_asset_directory().join("gfx").join("images.zip");
    log::info!("Using gfx.zip file at: {}", zip_file_path.display());
    zip_file_path
}

/// Returns the path to the sound-effects directory.
///
/// # Returns
/// * `PathBuf` pointing to `<asset_dir>/sfx/`.
pub fn get_sfx_directory() -> PathBuf {
    let sfx_directory = get_asset_directory().join("sfx");
    log::info!("Using sfx directory at: {}", sfx_directory.display());
    sfx_directory
}

/// Returns the path to the background-music directory.
///
/// # Returns
/// * `PathBuf` pointing to `<asset_dir>/music/`.
pub fn get_music_directory() -> PathBuf {
    let music_directory = get_asset_directory().join("music");
    log::info!("Using music directory at: {}", music_directory.display());
    music_directory
}
