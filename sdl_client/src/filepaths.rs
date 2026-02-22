use std::path::PathBuf;

/// Returns the base asset directory for the client.
///
/// When `CARGO_MANIFEST_DIR` is set (i.e. during a `cargo run`), the path is
/// resolved relative to the workspace. Otherwise it falls back to a path
/// relative to the current working directory.
///
/// # Returns
/// * `PathBuf` pointing to `client/assets/`.
fn get_asset_directory() -> PathBuf {
    let directory: PathBuf;
    if std::env::var("CARGO_MANIFEST_DIR").is_ok() {
        directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("client")
            .join("assets");
    } else {
        directory = PathBuf::from(".").join("..").join("client").join("assets");
    }
    directory
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
