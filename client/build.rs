extern crate embed_resource;

fn main() {
    // Embed the Windows app icon into the final .exe at build time.
    // This is required for the correct icon to show in Explorer/taskbar shortcuts.
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os == "windows" {
        embed_resource::compile("icon.rc", embed_resource::NONE);

        println!("cargo:rerun-if-changed=icon.rc");
        println!("cargo:rerun-if-changed=assets/gfx/mag_logo.ico");
    }
}
