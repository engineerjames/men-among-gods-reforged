# Men Among Gods — SDL2 Client

The primary game client for Men Among Gods: Reforged, built with [SDL2](https://www.libsdl.org/) via the [Rust SDL2 bindings](https://github.com/Rust-SDL2/rust-sdl2).

## Prerequisites

### macOS

```bash
brew install sdl2 sdl2_image sdl2_ttf sdl2_mixer sdl2_gfx
```

### Linux (Ubuntu / Debian)

```bash
sudo apt-get install -y \
  libsdl2-dev \
  libsdl2-image-dev \
  libsdl2-mixer-dev \
  libsdl2-gfx-dev
```

### Windows

SDL2 dependencies are managed automatically via [cargo-vcpkg](https://crates.io/crates/cargo-vcpkg) with static linking — no DLLs to ship.

```bash
cargo install cargo-vcpkg
cargo vcpkg build --manifest-path sdl_client/Cargo.toml
```

This downloads and builds the SDL2 libraries the first time; subsequent builds reuse the cache.

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
