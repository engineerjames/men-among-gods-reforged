# Men Among Gods — SDL2 Client

The primary game client for Men Among Gods: Reforged, built with [SDL2](https://www.libsdl.org/) via the [Rust SDL2 bindings](https://github.com/Rust-SDL2/rust-sdl2).

## Prerequisites

SDL2 is managed via [cargo-vcpkg](https://crates.io/crates/cargo-vcpkg) on all platforms — no system SDL2 installation required.

```bash
cargo install cargo-vcpkg
cargo vcpkg build --manifest-path sdl_client/Cargo.toml
```

This downloads and compiles SDL2 from source and links it statically. Linux builds also need some Bevy/system headers for the rest of the workspace:

```bash
# Ubuntu / Debian
sudo apt-get install -y pkg-config libasound2-dev libudev-dev cmake ninja-build
```

## Build

```bash
cargo build -p sdl_client
```

## Run

```bash
cargo run -p sdl_client
```

Controls:
- `Esc` or close window to quit.
