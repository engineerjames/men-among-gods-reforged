use std::path::PathBuf;

pub fn get_asset_directory() -> PathBuf {
    let directory: PathBuf;
    if std::env::var("CARGO_MANIFEST_DIR").is_ok() {
        directory = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets");
    } else {
        directory = PathBuf::from(".").join("assets");
    }

    log::info!("Using asset directory: {}", directory.display());
    directory
}

pub fn get_gfx_zipfile() -> PathBuf {
    get_asset_directory().join("gfx.zip")
}
