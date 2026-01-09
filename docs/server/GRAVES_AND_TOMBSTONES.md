# Graves & Tombstones (Death → Corpse → Tombstone)

This document captures the current technical understanding of how the server represents “graves” after a character dies, based on the Rust port in `server/src/` and the original (reference) C++ implementation in `server/src/orig/server/`.

> Terminology note: in the code you’ll see “grave”, “corpse”, “body”, and “tombstone”. Practically, the game uses a **corpse/body character** (a `Character` with `Body` flag) to hold loot/ownership metadata, and a **tombstone item** (template `170`) as the visible map object that players interact with.

## High-level model

- **Corpse/body is a character id**:
  - On death, the server produces a “body” entity represented as a **character slot** with `CharacterFlags::Body`.
  - That corpse character id is used for ownership, permissions (`#ALLOW`), and as the container for dropped items/gold.
- **Tombstone is an item**:
  - After a short effect/animation, the server creates a tombstone **item instance** (template `170`).
  - The tombstone item stores the corpse character id in `item.data[0]`.
- **Important**: The corpse character id existing is **intentional**, but it should **not remain occupying `map[m].ch`** after the tombstone is created. The map tile should be “walkable” again once the effect clears the reservation flag.

## Rust flow: where it lives

- **Death handling**: `server/src/state/death.rs`
  - NPC death converts the NPC into a corpse/body (or clones a player into a body slot) and marks it with `CharacterFlags::Body`.
- **Effect processing**: `server/src/effect.rs`
  - Effect type `3` (“death mist”) drives the death/grave animation.
  - Effect type `4` (“tombstone”) finalizes the grave by dropping item template `170`.
- **Movement/pathing**:
  - A* pathfinding: `server/src/path_finding.rs`
  - Target/tile checks: `server/src/player.rs` (`plr_check_target` etc.)

## Step-by-step: NPC death → tombstone on the ground

### 1) NPC becomes a body (corpse character)

In Rust, NPC death sets the NPC’s flags to `Body` (and optionally preserves `Respawn`):

- `server/src/state/death.rs` (NPC branch) assigns:
  - `characters[co].flags = CharacterFlags::Body.bits()` (plus `Respawn` when applicable)
  - `characters[co].a_hp = 0`
  - `characters[co].data[CHD_CORPSEOWNER]` based on build flags (e.g. `KILLERONLY`)
  - timers/markers like `data[98]`, `data[99]` used later

This corpse/body character id is the “grave identity” used later.

### 2) Death mist effect starts (effect type 3)

The effect tick logic uses type `3` to:

- animate graphics on the target tile
- at the “drop moment” (duration tick 9), attempt to place the grave/tomb

At that moment the server **removes the corpse character from the map tile**:

- Rust: `server/src/effect.rs` calls `player::plr_map_remove(co)` in effect type `3`
- C++ reference: `svr_effect.cpp` does `plr_map_remove(fx[n].data[2])` at the same point

This is critical: after this point the corpse character id still exists, but it should not block via `map[m].ch`.

### 3) Reserve the target tile temporarily (movement reservation)

If the corpse has items/gold to preserve, the server reserves the chosen tile by setting:

- `map[map_index].flags |= MF_MOVEBLOCK`

Rust: `server/src/effect.rs` in `handle_grave_creation(...)` sets the flag and schedules effect type `4`:

- `fx_add_effect(4, 0, x, y, co)`

This reservation is meant to be **temporary**: it prevents a character from stepping into the tile during the tombstone animation window.

### 4) Tombstone finalizes (effect type 4)

When effect type `4` reaches its completion tick:

- it clears the tomb graphics flags
- it clears `MF_MOVEBLOCK` (releasing the tile)
- it creates the tombstone item (template `170`)
- it stores the corpse character id on the item

In the C++ reference (`server/src/orig/server/svr_effect.cpp`):

- `map[m].flags &= ~MF_MOVEBLOCK;`
- `in = god_create_item(170);`
- `it[in].data[0] = co;`  ← **corpse character id**
- `god_drop_item(in, x, y);`

Rust mirrors that behavior in `server/src/effect.rs`:

- `let in_id = God::create_item(170);`
- `items[in_id].data[0] = co as u32;`
- `God::drop_item(in_id, x, y);`

## Why a corpse character id is intentional (and not the blocker)

Pathfinding and movement checks treat `map[m].ch != 0` as “occupied”.

However, in the reference implementation:

- the corpse character id is carried by the **tombstone item** (`item.data[0]`)
- the tile is temporarily blocked via `MF_MOVEBLOCK` only during the animation
- the corpse character should not remain as `map[m].ch` after `plr_map_remove(...)`

So “a grave has a character id” is correct and expected; it’s used as a stable handle to the loot container and permissions.

If graves are blocking forever, the likely causes are:

- `MF_MOVEBLOCK` not being cleared correctly, **or**
- the corpse/body character being incorrectly reinserted so `map[m].ch` stays non-zero.

## Movement/pathfinding interactions (why this shows up as “path interference”)

### A* pathfinder checks

In Rust A* (`server/src/path_finding.rs`), a tile is considered impassable if:

- `map[m].flags & mapblock != 0` (and `mapblock` includes `MF_MOVEBLOCK`)
- OR `map[m].ch != 0 || map[m].to_ch != 0`
- OR the tile has a blocking item (`IF_MOVEBLOCK`) that isn’t driver `2`

So either a stuck `MF_MOVEBLOCK` or a stuck `map[m].ch` will cause persistent path avoidance/failure.

### Intended behavior

- During tombstone animation: temporary `MF_MOVEBLOCK` is expected.
- After tombstone is dropped: `MF_MOVEBLOCK` should be cleared, and the tombstone item should not set `IF_MOVEBLOCK` (template `170` is driver `7` and is intended to be an interactive marker, not a permanent wall).

## Rust-port bug discovered: incorrect bitmask clear of `MF_MOVEBLOCK`

We found and fixed a subtle Rust expression-precedence issue in `server/src/effect.rs`.

Bad pattern (do not use):

```rust
map[map_index].flags &= !core::constants::MF_MOVEBLOCK as u64;
```

In Rust, `!X as u64` parses as `(!X) as u64` (negation before cast), which can produce a truncated mask and corrupt higher map flag bits.

Correct pattern:

```rust
map[map_index].flags &= !(core::constants::MF_MOVEBLOCK as u64);
```

This fix was applied to:

- tombstone completion (effect type `4`)
- respawn mist clearing logic (effect type `8`)

If graves were “permanently blocking”, this is a prime root-cause candidate because it can prevent proper clearing of movement-block semantics.

## Cleanup: “lost body” expiration

Both implementations track “body” characters and eventually remove unowned/lost corpses:

- In Rust: `server/src/server.rs` treats `CharacterFlags::Body` specially in the main tick loop and increments `data[98]`. After ~30 minutes it destroys items and frees the character slot.
- C++ reference shows the same logic in `svr_tick.cpp` (“removing lost body”).

This is cleanup and should not be relied on to resolve “blocked forever” problems — if the tile is blocked, it should be fixed at tombstone completion.

## Practical debugging checklist (if this regresses again)

- Confirm the corpse body was removed from the map at death-mist tick:
  - `plr_map_remove(co)` executed and `map[old_idx].ch` cleared.
- Confirm the tombstone completion clears reservation:
  - `map[m].flags & MF_MOVEBLOCK == 0` after effect type `4` completes.
- Confirm the tombstone item is present and references the corpse id:
  - `map[m].it != 0`, `items[map[m].it].temp == 170`, `items[...].data[0] == corpse_id`.
- If the tile is still blocked, check:
  - `map[m].ch` should be `0`
  - other codepaths that may set `MF_MOVEBLOCK` (e.g., populate/reset logic that bakes blocking flags into map tiles)

