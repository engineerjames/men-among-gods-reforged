# Screen Coordinate Systems (Logical vs Physical vs Viewport vs Game)

This project uses a few related-but-different “screen coordinate” spaces.
They tend to get mixed up when you add:

- HiDPI / Retina scaling (logical vs physical pixels)
- camera viewports (letterboxing / pillarboxing)
- a fixed internal resolution (800×600 “game” pixels)

This document describes each coordinate system and how to convert between them.

---

## Summary table

| Name | Units | Origin / axes | Typical source | What it answers |
|---|---|---|---|---|
| **Logical** | logical pixels (“points”) | Top-left is (0,0); +x right; +y down | `Window::cursor_position()` | “Where is the cursor in the window UI space?” |
| **Physical** | physical pixels | Top-left is (0,0); +x right; +y down | `Window::physical_cursor_position()` or `logical * scale_factor` | “Which real pixel is this on the backbuffer?” |
| **Viewport** | physical pixels, *relative to the camera viewport* | Top-left of the camera’s viewport is (0,0) | `Camera::viewport` + physical cursor | “Where is the cursor inside the rendered image?” |
| **Game** | fixed 800×600 pixels (virtual) | Conventionally top-left is (0,0); +x right; +y down | mapping from viewport coords | “Which pixel in the 800×600 ‘internal render’ does this correspond to?” |

Notes:
- Bevy describes `Window::cursor_position()` as “logical pixels”.
- Bevy’s `Window::ime_position` is explicitly documented as *client-area coordinates relative to the top-left*; `cursor_position()` uses the same convention.

---

## 1) Logical coordinates

**Logical coordinates** are what Bevy reports via:

- `Window::cursor_position()` → `Option<Vec2>`

They are in **logical pixels**, which are the platform’s DPI-independent units:

- On standard DPI displays: 1 logical px ≈ 1 physical px.
- On Retina / HiDPI: 1 logical px may correspond to 2×2 physical pixels (scale factor 2.0).

**Key properties**

- Origin is **top-left** of the window’s client area.
- `x` increases to the right.
- `y` increases downward.

**Typical use**

- UI interaction.
- Anything that should feel consistent across DPI.

---

## 2) Physical coordinates

**Physical coordinates** are the “real pixels” on the swapchain/backbuffer.

Sources:

- `Window::physical_cursor_position()` (already in physical px)
- or compute it:

```rust
let logical = window.cursor_position().unwrap();
let physical = logical * window.resolution.scale_factor();
```

Where:

- `scale_factor = window.resolution.scale_factor()`
- `scale_factor = physical_size / logical_size`

**Key properties**

- Same axis convention as logical coords: **top-left origin**, +x right, +y down.
- Physical pixels are what matter for:
  - camera viewports (`Camera::viewport` is expressed in physical pixels)
  - pixel-perfect math (avoiding subpixel sampling)

---

## 3) Viewport coordinates

A **camera viewport** is a rectangular region of the *physical backbuffer* that a camera renders into.

In Bevy, `Camera::viewport` contains:

- `physical_position: UVec2` (top-left of the viewport in physical px)
- `physical_size: UVec2` (width/height of the viewport in physical px)

When you letterbox/pillarbox a fixed-aspect game, you typically:

- keep the window/backbuffer full size
- render the camera into a centered viewport
- leave the rest of the window as black bars

### Converting physical → viewport-local

Given:

- `cursor_physical` (physical cursor coords)
- `vp_pos` and `vp_size` from `camera.viewport`

Compute viewport-local coordinates:

```rust
let in_viewport = cursor_physical - vp_pos;
```

Then:

- If `in_viewport.x` is in `[0, vp_size.x)` and `in_viewport.y` is in `[0, vp_size.y)`, the cursor is inside the rendered image.
- Otherwise, the cursor is in the letterbox/pillarbox bars (input-to-game mapping is usually ignored).

### If there is no explicit viewport

If `camera.viewport == None`, the camera renders to the full window.
You can treat that as:

- `vp_pos = (0, 0)`
- `vp_size = (window.physical_width(), window.physical_height())`

---

## 4) Game coordinates (800×600)

**Game coordinates** in this project mean a *fixed virtual screen* of:

- width = **800**
- height = **600**

This is independent of the window’s actual size.

### Converting viewport-local → game (800×600)

If the cursor is inside the viewport, map it proportionally:

```rust
let game = Vec2::new(
    in_viewport.x / vp_size.x * 800.0,
    in_viewport.y / vp_size.y * 600.0,
);
```

**Interpretation**

- `game.x` and `game.y` tell you “which pixel” (in the 800×600 internal space) the click corresponds to.
- If you want integer pixel coords, `floor()` (or `round()`) these values.

### Relationship to “pixel-perfect rendering”

Mapping input into a fixed 800×600 space is only half the story.
For truly pixel-perfect output you typically also want the viewport size to be an **integer multiple** of 800×600:

- `scale = floor(min(vp_w / 800, vp_h / 600))` (clamped to at least 1)
- set `viewport_w = 800 * scale`, `viewport_h = 600 * scale`

This avoids fractional scaling (which can blur even with nearest-neighbor sampling).

---

## Common gotchas

- **HiDPI mismatch**: `cursor_position()` is logical; `Camera::viewport` is physical. Convert before comparing.
- **No viewport set**: if you rely on `camera.viewport`, you’ll get `None` unless you explicitly set it.
- **Letterbox clicks**: clicks in black bars should not be mapped into game coords.

---

## Where this is used in the code

- The click logger in `client/src/main.rs` prints:
  - logical cursor coords
  - physical cursor coords
  - viewport-relative coords (if applicable)
  - mapped 800×600 game coords
