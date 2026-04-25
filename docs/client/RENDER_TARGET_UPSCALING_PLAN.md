# Render-Target Upscaling Implementation Plan

## Goal

Increase perceived graphical fidelity without filtering individual legacy sprites.

The client currently renders into a `960x540` logical coordinate space and lets SDL scale that directly to the window. Anti-aliasing or linear filtering on individual sprites can create pink outlines because transparent sprite-border pixels may still contain magenta or pink RGB data. When filtering samples those transparent edge pixels, the hidden color bleeds into the visible sprite edge.

The proposed rendering path is:

```text
world sprites + HUD at 960x540
        |
        v
offscreen scene texture, nearest sprite sampling
        |
        v
final window copy using selected upscale mode
        |
        v
present
```

Filtering only the already-composited final scene avoids transparent sprite-border sampling.

## First Implementation Slice

Keep the first change narrow:

1. Request SDL render-target support when creating the window canvas.
2. Create a persistent `960x540` render target texture.
3. Render the existing scene into that texture.
4. Copy the completed texture to the real window.
5. Preserve the existing `pixel_perfect_scaling` setting.
6. Keep the old direct-render path available as a fallback if render targets fail.

## Canvas Setup

Current canvas creation in `client/src/main.rs`:

```rust
let mut canvas = window.into_canvas().build().map_err(|e| e.to_string())?;
```

Target canvas creation should become:

```rust
let mut canvas = window
    .into_canvas()
    .target_texture()
    .build()
    .map_err(|e| e.to_string())?;
```

This enables rendering into a texture via SDL.

## Scene Texture

After creating the texture creator, create a persistent target texture:

```rust
let mut scene_texture = texture_creator
    .create_texture_target(
        Some(sdl2::pixels::PixelFormatEnum::RGBA8888),
        constants::TARGET_WIDTH_INT,
        constants::TARGET_HEIGHT_INT,
    )
    .map_err(|e| e.to_string())?;
```

The scene texture should live in `main()` alongside `gfx_cache`, not inside `AppState`, because both borrow from the same `TextureCreator`.

## Render Loop Shape

Replace the direct logical-size render block with a two-stage render.

Current shape:

```rust
let _ = canvas.set_logical_size(constants::TARGET_WIDTH_INT, constants::TARGET_HEIGHT_INT);
let _ = canvas.set_integer_scale(app_state.settings.pixel_perfect_scaling);
scene_manager.render_world(&mut app_state, &mut canvas);
let _ = canvas.set_integer_scale(false);
let _ = canvas.set_logical_size(0, 0);

canvas.present();
```

New shape:

```rust
canvas.with_texture_canvas(&mut scene_texture, |target_canvas| {
    target_canvas.set_draw_color(sdl2::pixels::Color::RGB(0, 0, 0));
    target_canvas.clear();

    let _ = target_canvas.set_logical_size(0, 0);
    let _ = target_canvas.set_integer_scale(false);

    scene_manager.render_world(&mut app_state, target_canvas)
})??;

canvas.set_draw_color(sdl2::pixels::Color::RGB(0, 0, 0));
canvas.clear();

let _ = canvas.set_logical_size(constants::TARGET_WIDTH_INT, constants::TARGET_HEIGHT_INT);
let _ = canvas.set_integer_scale(app_state.settings.pixel_perfect_scaling);
canvas.copy(&scene_texture, None, None)?;
let _ = canvas.set_integer_scale(false);
let _ = canvas.set_logical_size(0, 0);

canvas.present();
```

The exact error handling may need adjustment depending on the `sdl2` crate API, but the architecture is the important part.

## Viewport Calculation

The first slice can reuse SDL logical sizing for the final copy.

A later cleanup should compute the destination rectangle explicitly using the same math as `client/src/dpi_scaling.rs`, so rendering and input mapping stay aligned across:

- windowed mode
- fullscreen
- borderless fullscreen
- Retina / HiDPI
- Steam Deck `1280x800`
- ultrawide displays

## Scale Modes

After the first render-target path works, add explicit upscale modes:

```rust
pub enum UpscaleMode {
    PixelPerfect,
    Crisp,
    Smooth,
}
```

Suggested behavior:

- `PixelPerfect`: integer scale, nearest sampling, centered letterbox.
- `Crisp`: aspect-preserving non-integer scale, nearest sampling.
- `Smooth`: aspect-preserving non-integer scale, linear filtering on the final scene texture.

Initially, this can be derived from the existing `pixel_perfect_scaling: bool`. Later, replace the checkbox with a selector in the settings UI.

## Filtering Strategy

Do not enable linear filtering globally for sprite textures.

Sprite textures should remain nearest-sampled. Linear filtering should apply only to the final composited scene texture in `Smooth` mode.

If SDL scale quality must be controlled through `SDL_HINT_RENDER_SCALE_QUALITY`, the scene texture may need to be recreated when switching between nearest and linear modes.

## Fallback Plan

Render targets may fail on some SDL backends.

Fallback behavior:

1. Attempt to create a target-texture renderer.
2. Attempt to create the `960x540` scene texture.
3. If either fails, log a warning.
4. Fall back to the current direct-render path.

This keeps the feature low-risk.

## Follow-Up Improvements

Once the basic target pipeline works:

1. Add `UpscaleMode` persistence.
2. Replace the pixel-perfect checkbox with a scaling mode selector.
3. Add explicit destination-rectangle viewport math.
4. Add a final-scene `Smooth` mode.
5. Add a `Crisp` mode for non-integer nearest scaling.
6. Consider optional final post-process passes:
   - subtle sharpening
   - CRT or scanline mode
   - color grading
   - xBRZ or Scale2x-style scaler

## Validation Checklist

Run:

```bash
cargo build -p client
```

Manual checks:

- Launch client at native `960x540`.
- Resize to larger 16:9 window.
- Resize to non-16:9 window and confirm letterboxing.
- Toggle pixel-perfect scaling.
- Confirm mouse hit-testing still matches world tiles.
- Confirm UI click targets still line up.
- Test fullscreen and borderless mode.
- Test macOS Retina / HiDPI behavior.
- Test Steam Deck `1280x800` behavior if available.
- Confirm no pink outlines appear in smooth final upscale mode.
- Run the in-game performance profiler and compare before and after frame timings.

## Main Risks

- SDL render-target support can vary by backend.
- SDL logical size and integer scale state are sticky and must be reset carefully.
- Texture scale-quality hints may apply at texture creation time.
- Input mapping must match the final scene destination rectangle.
- `TextureCreator` lifetimes require keeping the scene texture close to `main()`.

## Recommended Milestone Boundary

Milestone 1 should only prove the render-target pipeline:

- render scene into `960x540` texture
- copy texture to window
- preserve existing scaling toggle
- preserve direct-render fallback

Do not add new UI or new settings modes until Milestone 1 is stable.
