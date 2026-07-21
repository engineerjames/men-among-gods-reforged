# Player Deaths and NPC Graves

Player and NPC deaths now follow different persistence paths. Players retain
their items and never leave a grave. NPCs continue to use the original
corpse-to-tombstone system described below.

> Terminology note: in the NPC code you’ll see “grave”, “corpse”, “body”, and
> “tombstone”. The server uses a **corpse/body character** (a `Character` with
> the `Body` flag) to hold loot and ownership metadata, and a **tombstone item**
> (template `170`) as the visible map object players interact with.

## High-level model

- **Player death has no corpse or tombstone**:
  - Backpack, worn, and ordinary cursor items remain on the live character.
  - Eligible deaths remove `PLAYER_DEATH_MONEY_LOSS_PERCENT` of carried money,
    rounded down to the nearest silver.
  - Carried money includes `Character.gold` and cursor-held money encoded in
    `Character.citem`. It excludes the bank balance in `Character.data[13]`.
  - Arena, Guardian Angel, God, and forced-save/deathtrap deaths are exempt.
  - The type `3` death mist still runs at the death location, but receives
    corpse id `0` and performs no midpoint corpse processing.
- **Corpse/body is a character id**:
  - On NPC death, the server converts the NPC into a body entity represented
    by a character slot with `CharacterFlags::Body`.
  - That corpse character id is used for ownership, permissions (`#ALLOW`), and as the container for dropped items/gold.
- **Tombstone is an item**:
  - After a short effect/animation, the server creates a tombstone **item instance** (template `170`).
  - The tombstone item stores the corpse character id in `item.data[0]`.
- **Important**: The corpse character id existing is **intentional**, but it should **not remain occupying `map[m].ch`** after the tombstone is created. The map tile should be “walkable” again once the effect clears the reservation flag.

## Rust flow: where it lives

- **Death handling**: `server/src/state/death.rs`
  - Player death applies the eligible carried-money penalty, destroys temporary
    spell items, and transfers the same character to its temple.
  - NPC death converts the NPC into a corpse/body and marks it with
    `CharacterFlags::Body`.
- **Effect processing**: `server/src/effect.rs`
  - Effect type `3` (“death mist”) is visual-only when its corpse id is `0`;
    otherwise it drives NPC corpse/grave processing.
  - Effect type `4` (“tombstone”) finalizes the grave by dropping item template `170`.

## Player death flow

1. `do_character_killed` records death statistics and metadata.
2. `handle_player_death` checks arena, Guardian Angel, God, and forced-save
   exemptions.
3. An eligible player loses the configured percentage of purse plus
   cursor-held money. Purse money is deducted first; banked money and all items
   remain unchanged.
4. Temporary spell items are destroyed and transient combat state is reset.
5. The player is transferred to their temple and restored with 10 HP. Permanent
   HP and mana are not reduced.
6. A type `3` effect with corpse id `0` displays death mist at the original
   location and expires without creating a body or tombstone.

## Step-by-step: NPC death --> tombstone on the ground

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

### Intended pathfinding behavior

- During tombstone animation: temporary `MF_MOVEBLOCK` is expected.
- After tombstone is dropped: `MF_MOVEBLOCK` should be cleared, and the tombstone item should not set `IF_MOVEBLOCK` (template `170` is driver `7` and is intended to be an interactive marker, not a permanent wall).
