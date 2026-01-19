# TODO: Replicate “Magic” Screen-Space Effects (dd_alphaeffect_magic)

This document captures how the original C client implements the “magic aura” / “magic overlay” effects (often referred to as `dd_alphaeffect_magic`) and outlines practical options to replicate them in the Rust/Bevy client.

## 1) What the original effect is

Despite the name, `dd_alphaeffect_magic` is not alpha blending in the modern sense.

It is a **screen-space, per-pixel color warp** applied directly onto the rendered frame buffer (the backbuffer surface). It edits a **64×64** region of pixels around a tile/object and shifts the underlying pixels toward selected color channels (R/G/B) with a shaped falloff.

### Where it is used

- Draw loop calls it when a tile has one or more magic flags set.
- The caller builds a channel mask and a strength value, then applies the effect once per affected map tile.

Relevant source locations:
- `client/src/orig/engine.c` (call site; builds channel mask and strength)
- `client/src/orig/dd.c` (`dd_alphaeffect_magic_0` / `_1` implementations)

## 2) Inputs and meaning

`dd_alphaeffect_magic(int nr, int str, int xpos, int ypos, int xoff, int yoff)`

- `nr`: bitmask of which magic channels are active
  - `nr & 1` → Red channel “magic”
  - `nr & 2` → Green channel “magic”
  - `nr & 4` → Blue channel “magic”

- `str`: strength divisor (higher = weaker)
  - The implementation does: `e /= max(1, str)`.
  - Call site derives `str` from bitfields in `map[m].flags` and takes the maximum among active magic types.

- `xpos`, `ypos`: world-ish tile-space pixel coords (commonly `x*32`, `y*32`) for the tile being affected.

- `xoff`, `yoff`: camera / per-object offsets added to the computed screen coords.

## 3) Coordinate transform (how it finds the 64×64 destination)

The function computes a screen position `(rx, ry)` from `(xpos, ypos)` using the isometric projection math consistent with the rest of the client.

Conceptually:

- `rx` is derived from a combination of `xpos/2` and `ypos/2` plus constants.
- `ry` is derived from `xpos/4 - ypos/4` plus constants.
- There are extra adjustments for negative coordinates to match C integer division / rounding behavior.
- Then `xoff` and `yoff` are added.

`(rx, ry)` becomes the **top-left** corner of the 64×64 region the effect will edit.

Important rendering constraint from the original:
- It skips rows above a UI threshold (`dsty + y < 200`), presumably to avoid drawing into the UI panel region.

## 4) The per-pixel intensity function (shape)

For each pixel `(x, y)` in a 64×64 region:

1. Compute an intensity accumulator `e` starting at 32.
2. Subtract distance-from-center terms:
   - Horizontal: linearly falls away from the center at x≈31/32.
   - Vertical: asymmetric falloff:
     - fades quickly above y<16
     - fades more strongly below y>55 (multiplied by 2)

3. Clamp `e >= 0`, then scale by strength:

- `e = max(0, e) / max(1, str)`

This yields a lopsided “teardrop/flame/aura” blob rather than a symmetric circle.

## 5) The color warp math (what happens to the pixel)

The function reads the existing framebuffer pixel, unpacks RGB channels, then:

- Computes `e2 = e * (# of enabled channels in nr)`.
- Darkens the pixel a bit (to preserve contrast, avoid washout), then boosts selected channels.

In RGB565 mode (the common path):
- Red/Blue are 5-bit (0..31), Green is 6-bit (0..63).
- Green receives different scaling because it has 6 bits:
  - it tends to subtract more on the “darken” step and add `e*2` on the “boost” step.

Net effect:
- The original effect is not just “additive tint”; it both **darkens and re-brightens** selected channels in a shaped mask.

## 6) Pixel formats (why there are two implementations)

`dd_alphaeffect_magic` selects:
- `_0` when RGBM != 1 (common 16-bit RGB565), and
- `_1` when RGBM == 1 (a 15-bit-like layout).

The overall algorithm is the same; only packing/unpacking/scaling differs.

## 7) Replication options in the Rust client

### Option A: Approximate with additive/colored sprites (fastest)

Approach:
- Render a 64×64 (or similar) gradient “aura” sprite centered at the same screen position.
- Tint by enabled channels (R/G/B) based on `nr`.
- Scale opacity/intensity by a function of `str`.

Pros:
- Simple, low-risk, easy to tune.
- Plays nicely with modern renderers.

Cons:
- Does not match the original’s “darken then boost” behavior.
- Multiple overlapping magic sources may look more washed out than the C client.

### Option B: Post-process shader on the rendered frame (closest feel)

Approach:
- Render the scene to an offscreen texture.
- Run a full-screen pass that applies the same per-pixel warp math.
- Provide the shader with a list of active “magic sources” visible this frame:
  - each source includes position, channel mask, and strength.

Pros:
- Can match the original algorithm closely.
- Naturally composes over already-rendered world pixels (like the C code).

Cons:
- Requires a render-to-texture pipeline and a postprocess step.
- Needs a strategy for limiting the number of sources (uniform array limits) or using a texture/SSBO.

### Option C: CPU-side framebuffer mutation (most literal, least desirable)

Approach:
- Read back the frame buffer (or maintain a CPU pixel buffer), apply the same loops, upload back.

Pros:
- Very close to original implementation semantics.

Cons:
- Likely too slow; readbacks stall the GPU.
- Complicates the renderer.

## 8) Suggested TODO plan

### Phase 1: Decide target fidelity

- [ ] Decide between Option A (approx) and Option B (near-1:1 shader).
- [ ] Confirm whether the Rust client also has a “don’t draw into UI area” rule like the `y < 200` skip.

### Phase 2A: Implement Option A (approx sprite overlay)

- [ ] Create a small gradient texture (procedural or asset) approximating the original falloff shape.
- [ ] Implement a system that spawns/draws aura sprites over tiles with magic flags.
- [ ] Derive `nr` and `str` equivalents from the Rust-side map/tile flags.
- [ ] Tune intensity mapping so `str` meaningfully reduces brightness (e.g. `intensity = 1.0 / max(1.0, str)` with clamps).

Acceptance criteria:
- Auras appear at correct tile positions.
- Channel combinations (R/G/B) look plausible.
- Strength scaling matches “bigger str = weaker”.

### Phase 2B: Implement Option B (shader postprocess)

- [ ] Add a render-to-texture stage for the world render.
- [ ] Add a postprocess pass that samples the scene texture and applies the warp.
- [ ] Define a compact `MagicSource` struct:
  - screen position (or world position with projection),
  - channel mask,
  - strength.
- [ ] Gather visible sources each frame and upload to GPU.
- [ ] Implement the intensity function and channel math to match the C code.

Acceptance criteria:
- Per-pixel look matches the original more closely (darken+boost feel).
- Multiple sources combine in a stable, predictable way.

### Phase 3: Verification + tuning

- [ ] Capture side-by-side screenshots vs the original client for a test scene.
- [ ] Validate negative-coordinate behavior if the Rust client can render those cases.
- [ ] Confirm UI masking behavior (original skips `y < 200`).

## 9) Notes / pitfalls

- The original code hardcodes a 64×64 region and applies a custom-shaped falloff; a circular radial gradient will not match the feel.
- Green scaling differs from red/blue due to 6-bit green in RGB565.
- The C effect modifies the already-rendered pixel; pure additive blending will be “too bright” in many cases unless you also apply some form of contrast preservation.
