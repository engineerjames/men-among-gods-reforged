use std::path::PathBuf;

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

pub fn get_gfx_zipfile() -> PathBuf {
    let zip_file_path = get_asset_directory().join("gfx").join("images.zip");
    log::info!("Using gfx.zip file at: {}", zip_file_path.display());
    zip_file_path
}

pub fn get_sfx_directory() -> PathBuf {
    let sfx_directory = get_asset_directory().join("sfx");
    log::info!("Using sfx directory at: {}", sfx_directory.display());
    sfx_directory
}

pub fn get_music_directory() -> PathBuf {
    let music_directory = get_asset_directory().join("music");
    log::info!("Using music directory at: {}", music_directory.display());
    music_directory
}
