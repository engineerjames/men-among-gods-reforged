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

The client renders text through a single module, `client/src/font_cache.rs`,
which exposes two cooperating paths:

- **Legacy bitmap fonts** — fixed-pitch glyphs packed in the gfx atlas. Used
  by the bulk of the existing UI. Constants: `BITMAP_GLYPH_H = 10`,
  `BITMAP_GLYPH_ADVANCE = 6`. Drawn via the standalone `font_cache::draw_text`
  and `font_cache::draw_wrapped_text` helpers (`font: usize` selects the
  sheet).
- **TrueType fonts** — SDL2_ttf-backed engine that rasterises glyphs once
  per `(font, size, char)` and tints/alphas them at draw time. Every `.ttf` /
  `.otf` file under `client/assets/fonts/` is auto-discovered at startup
  (alphabetical order). The shipped faces include **Noto Sans Regular/Bold**
  and the **Matrix Sans** family (SIL OFL 1.1, see `OFL.txt`).

To draw TrueType text from a widget you already have a `RenderContext`:

```rust
use crate::font_cache;
let handle = ctx.text.handle("NotoSans-Bold", 18);
font_cache::draw_text_handle(
    ctx.canvas,
    ctx.text,
    ctx.gfx,
    &handle,
    "Hello",
    x,
    y,
    font_cache::TextStyle::centered(),
)?;
```

`FontHandle::bitmap(id)` keeps the legacy path available; the engine measures
and renders both kinds through the same free functions
(`font_cache::text_size`, `font_cache::line_height`,
`font_cache::draw_text_handle`, `font_cache::draw_wrapped_text_handle`).
Unknown font stems passed to `TextEngine::handle` fall back to bitmap font 0
with a warning, mirroring the soft-failure ergonomics of `GraphicsCache`.

The `Sdl2TtfContext` is created once in `main` and leaked to `'static`, so
the `text_engine` field on `AppState` only carries the existing texture
lifetime. Logical coordinates are unchanged (1920×1080); the engine handles
DPI scaling internally.

Two integration binaries help exercise this stack manually:

```bash
cargo run -p client --bin ui-integration    # widget gallery
cargo run -p client --bin font-integration  # bitmap + TTF font browser
```

`font-integration` shows one sample sentence in bitmap font 0 and the same
sentence in the currently-selected TTF face at several sizes; press the
left/right arrow keys to cycle through every discovered font.
