# Men Among Gods — Client

The primary game client for Men Among Gods: Reforged, built with [SDL2](https://www.libsdl.org/) via the [Rust SDL2 bindings](https://github.com/Rust-SDL2/rust-sdl2).

## Prerequisites

SDL2 is managed via [cargo-vcpkg](https://crates.io/crates/cargo-vcpkg) on all platforms — no system SDL2 installation required.

```bash
cargo install cargo-vcpkg
cargo vcpkg build --manifest-path client/Cargo.toml
```

This downloads and compiles SDL2 from source and links it statically. Linux builds also need some system headers:

```bash
# Ubuntu / Debian
sudo apt-get install -y pkg-config libasound2-dev libudev-dev cmake ninja-build
```

## Build

```bash
cargo build -p client
```

## Run

```bash
cargo run -p client
```

Controls:
- `Esc` or close window to quit.

## Text rendering

The client supports two text-rendering paths that coexist:

- **Legacy bitmap fonts** (`client/src/font_cache.rs`) — fixed-pitch glyphs
  packed in the gfx atlas. Used by the bulk of the existing UI. Constants:
  `BITMAP_GLYPH_H = 10`, `BITMAP_GLYPH_ADVANCE = 6`.
- **TrueType fonts** (`client/src/text/mod.rs`) — SDL2_ttf-backed engine that
  rasterises glyphs once per `(font, size, char)` and tints/alphas them at
  draw time. The shipped faces are **Noto Sans Regular/Bold** under
  `client/assets/fonts/` (SIL OFL 1.1, see `OFL.txt`).

To draw TrueType text from a widget you already have a `RenderContext`:

```rust
use crate::text::{self, FontHandle};
let handle = FontHandle::ttf(text::UI_BOLD, 18);
text::draw_text(
    ctx.canvas,
    ctx.text,
    ctx.gfx,
    &handle,
    "Hello",
    x,
    y,
    text::Style::centered(),
)?;
```

`FontHandle::bitmap(id)` keeps the legacy path available; the engine measures
and renders both kinds through the same free functions (`text::text_size`,
`text::line_height`, `text::draw_text`, `text::draw_wrapped_text`).

The `Sdl2TtfContext` is created once in `main` and leaked to `'static`, so
the new `text_engine` field on `AppState` only carries the existing texture
lifetime. Logical coordinates are unchanged (1920×1080); the engine handles
DPI scaling internally.
