---
name: GameState Singleton Refactor
overview: Consolidate the Repository, State, and PathFinder global singletons into a single `GameState` struct that is created in `main()` and passed via `&mut GameState` throughout the call chain. This eliminates ~4,800 closure-based accessor calls and all nested closure patterns.
todos:
  - id: create-game-state
    content: Create `game_state.rs` with `GameState` struct, `initialize()`, `load()`, `save()`, `shutdown()` -- migrate from `repository.rs`
    status: in_progress
  - id: update-main-server
    content: Update `main.rs` and `server.rs` to create GameState and thread `&mut GameState` through tick/initialize
    status: pending
  - id: convert-state-submodules
    content: "Convert all 12 state submodules: change `impl State` to `impl GameState`, replace `Repository::with_*` closures with `self.*` field access"
    status: pending
  - id: convert-standalone-modules
    content: "Convert standalone modules (player, god, populate, effect, area, talk, points, helpers, lab9) to take `gs: &mut GameState` parameter"
    status: pending
  - id: convert-driver-modules
    content: "Convert all 6 driver modules (generic, npc, skill, use_item, special, look) to take `gs: &mut GameState` parameter"
    status: pending
  - id: refactor-pathfinder
    content: Remove PathFinder singleton, make it a field of GameState, update find_path to take data slices as params
    status: pending
  - id: cleanup
    content: Delete repository.rs, clean up State mod.rs, remove unused SingleThreadCell imports, update lib.rs/main.rs module declarations
    status: pending
  - id: entity-ref-pass
    content: (Follow-up pass) Identify and convert Tier 1/2 functions from index-based to entity-reference signatures -- methods on entity types when clearly about one type, free functions otherwise
    status: pending
isProject: false
---

# GameState Singleton Refactor

## Problem

The server uses 5 global singletons (`Repository`, `State`, `Server/PLAYERS`, `NetworkManager`, `PathFinder`) accessed via closure-based static methods (`Repository::with_characters(|ch| ...)`). This causes:

- **Deeply nested closures** when a function needs multiple data slices (e.g., characters + items + map)
- **No testability** since everything is a global singleton
- **Verbose, ceremony-heavy code** -- every data access requires a closure
- **Hidden dependencies** -- functions don't declare what state they need

There are ~4,014 Repository accessor calls, ~818 State calls, and ~2 PathFinder calls across 30+ source files.

## Solution: `GameState` struct

Create a single `GameState` struct that owns all game data. Pass it as `&mut GameState` through the call chain. `NetworkManager` and `Server/PLAYERS` remain as singletons since they are I/O/connection concerns.

### Before vs After

Before (nested closures):

```rust
pub fn get_fight_skill(&self, cn: usize) -> i32 {
    let (in_idx, s_hand, s_karate, ...) =
        Repository::with_characters(|characters| {
            let in_idx = characters[cn].worn[WN_RHAND] as usize;
            (in_idx, characters[cn].skill[SK_HAND][5] as i32, ...)
        });
    let flags = Repository::with_items(|items| items[in_idx].flags);
    // ...
}
```

After (direct field access):

```rust
pub fn get_fight_skill(&self, cn: usize) -> i32 {
    let in_idx = self.characters[cn].worn[WN_RHAND] as usize;
    let s_hand = self.characters[cn].skill[SK_HAND][5] as i32;
    // ...
    let flags = self.items[in_idx].flags;
    // ...
}
```

## Struct Design

```rust
// server/src/game_state.rs
pub struct GameState {
    // -- World data (from Repository) --
    pub map: Vec<Map>,
    pub items: Vec<Item>,
    pub item_templates: Vec<Item>,
    pub characters: Vec<Character>,
    pub character_templates: Vec<Character>,
    pub effects: Vec<Effect>,
    pub globals: Global,
    pub see_map: Vec<SeeMap>,
    pub bad_names: Vec<String>,
    pub bad_words: Vec<String>,
    pub message_of_the_day: String,
    pub ban_list: Vec<Ban>,

    // -- Counters (from Repository) --
    pub last_population_reset_tick: u32,
    pub ice_cloak_clock: u32,
    pub item_tick_gc_off: u32,
    pub item_tick_gc_count: u32,
    pub item_tick_expire_counter: u32,

    // -- Visibility state (from State) --
    pub _visi: [i8; 40 * 40],
    pub visi: [i8; 40 * 40],
    pub vis_is_global: bool,
    pub see_miss: u64,
    pub see_hit: u64,
    pub ox: i32,
    pub oy: i32,
    pub is_monster: bool,
    pub penta_needed: usize,

    // -- Pathfinding (kept as sub-struct) --
    pub pathfinder: PathFinder,

    // -- Persistence --
    storage_backend: StorageBackend,
    saved_cleanly: bool,
    executable_path: String,
}
```

### Key Design Decisions

- **PathFinder stays as a sub-struct** because it has its own internal state (node maps, open set, etc.). Its methods change from calling `Repository::with_`* to receiving data as parameters. Callers use Rust's split borrows:

```rust
  gs.pathfinder.find_path(&gs.map, &gs.items, &gs.characters, ...);
  

```

- **NetworkManager and PLAYERS stay as singletons** since they manage I/O and connections, not game state. Functions that need both will take `&mut GameState` and call `Server::with_players(...)` separately.
- **State submodule methods become `impl GameState`** blocks in the same files. `state/combat.rs` changes from `impl State { ... }` to `impl GameState { ... }`, and accesses `self.characters` directly instead of `Repository::with_characters(|ch| ...)`.
- `**repository.rs` becomes `game_state.rs**` -- loading/saving/initialization logic moves to `GameState` methods.

## Call Chain Changes

```
main.rs:
  let mut game_state = GameState::initialize()?;
  server.initialize(&mut game_state)?;
  server.tick(&mut game_state);            // main loop
  game_state.shutdown();

Server::tick(&mut self, gs: &mut GameState):
  -> handle_network_io(gs)
  -> game_tick(gs)
     -> driver::generic_driver(gs, cn)
     -> gs.do_update_char(cn)             // was State::with_mut(|s| s.do_update_char(...))
     -> populate::populate_tick(gs)
  -> background_save_tick(gs)             // clone from gs instead of Repository

player::plr_cmd(gs: &mut GameState, nr: usize):
  -> gs.do_say(cn, text)                  // was State::with_mut(|s| s.do_say(...))
  -> gs.do_attack(cn, co)
  -> God::give_item(gs, cn, item_id)      // was God::give_item(cn, item_id)
```

## Module-by-Module Changes

### Core infrastructure (create first)

- `**[server/src/game_state.rs](server/src/game_state.rs)**` (new) -- `GameState` struct, `initialize()`, `load()`, `save()`, `shutdown()`. Migrated from `repository.rs`.
- `**[server/src/repository.rs](server/src/repository.rs)**` -- deleted entirely
- `**[server/src/state/mod.rs](server/src/state/mod.rs)**` -- remove `State` struct, `OnceLock`, `SingleThreadCell` usage. Submodules change `impl State` to `impl GameState`.
- `**[server/src/main.rs](server/src/main.rs)**` -- `GameState::initialize()` replaces `Repository::initialize()`, `State::initialize()`, `PathFinder::initialize()`. Pass `&mut game_state` to `server.tick()`.

### State submodules (change `impl State` to `impl GameState`, replace Repository closures with `self.*`)

- `[server/src/state/combat.rs](server/src/state/combat.rs)`
- `[server/src/state/commerce.rs](server/src/state/commerce.rs)`
- `[server/src/state/communication.rs](server/src/state/communication.rs)`
- `[server/src/state/commands.rs](server/src/state/commands.rs)`
- `[server/src/state/death.rs](server/src/state/death.rs)`
- `[server/src/state/economy.rs](server/src/state/economy.rs)`
- `[server/src/state/inventory.rs](server/src/state/inventory.rs)`
- `[server/src/state/logging.rs](server/src/state/logging.rs)`
- `[server/src/state/player_actions.rs](server/src/state/player_actions.rs)`
- `[server/src/state/stats.rs](server/src/state/stats.rs)`
- `[server/src/state/visibility.rs](server/src/state/visibility.rs)`
- `[server/src/state/admin.rs](server/src/state/admin.rs)`

### Standalone modules (add `gs: &mut GameState` param, replace `Repository::with_*` closures)

- `[server/src/server.rs](server/src/server.rs)` -- `tick()`, `game_tick()`, `handle_network_io()` gain `gs` param. PLAYERS singleton stays.
- `[server/src/player.rs](server/src/player.rs)` -- `plr_cmd()`, `plr_login()`, `plr_logout()` gain `gs` param.
- `[server/src/god.rs](server/src/god.rs)` -- `God::*` methods gain `gs` param.
- `[server/src/populate.rs](server/src/populate.rs)` -- all populate functions gain `gs` param.
- `[server/src/effect.rs](server/src/effect.rs)`
- `[server/src/area.rs](server/src/area.rs)`
- `[server/src/talk.rs](server/src/talk.rs)`
- `[server/src/points.rs](server/src/points.rs)`
- `[server/src/helpers.rs](server/src/helpers.rs)`
- `[server/src/lab9.rs](server/src/lab9.rs)`

### Driver modules (add `gs: &mut GameState` param)

- `[server/src/driver/generic.rs](server/src/driver/generic.rs)`
- `[server/src/driver/npc.rs](server/src/driver/npc.rs)`
- `[server/src/driver/skill.rs](server/src/driver/skill.rs)`
- `[server/src/driver/use_item.rs](server/src/driver/use_item.rs)`
- `[server/src/driver/special.rs](server/src/driver/special.rs)`
- `[server/src/driver/look.rs](server/src/driver/look.rs)`

### PathFinder

- `[server/src/path_finding.rs](server/src/path_finding.rs)` -- remove singleton. `PathFinder` becomes a field of `GameState`. Methods like `find_path()` take `map: &[Map]`, `items: &[Item]`, `characters: &[Character]` as explicit parameters instead of accessing `Repository` statics.

### Cleanup

- `[server/src/single_thread_cell.rs](server/src/single_thread_cell.rs)` -- kept only for `PLAYERS` and `NetworkManager` singletons (those remain).
- `[server/src/keydb_store.rs](server/src/keydb_store.rs)` -- no changes needed (already receives data as params).
- `[server/src/background_saver.rs](server/src/background_saver.rs)` -- no changes needed (receives cloned data via channel).

## Borrow Checker Considerations

Rust allows **split borrows** on different struct fields. This is the key enabler:

```rust
// This compiles because characters, items, and map are different fields
let ch = &mut gs.characters;
let it = &gs.items;
let map = &gs.map;
ch[cn].hp -= it[weapon].damage;
```

For PathFinder (a sub-struct), split borrows also work:

```rust
// pathfinder, map, items, characters are all different fields of gs
gs.pathfinder.find_path(&gs.map, &gs.items, &gs.characters, ...);
```

Cases where split borrows are **not** sufficient (same-collection conflicts like `characters[cn]` and `characters[co]`) already exist today and are handled with index-based access or temporary copies.

## Phase 2: Entity-Reference Signatures (follow-up pass)

After the GameState refactor is landed and compiling, a second pass converts suitable functions from index-based (`cn: usize`) to entity-reference (`char: &Character` / `char: &mut Character`) signatures. This further improves testability and makes data dependencies explicit at the type level.

### Function tiers

- **Tier 1 -- pure read on one entity, read others**: convert to free functions or entity methods taking `&Character` / `&Item` + read-only slices. Example: `get_fight_skill(char: &Character, items: &[Item]) -> i32`.
- **Tier 2 -- mutate one entity, read others**: convert to free functions or entity methods taking `&mut Character` + read-only slices. Example: `do_regenerate(char: &mut Character, globals: &Global)`.
- **Tier 3 -- multi-entity mutation**: remain as `&mut GameState` methods with index parameters. Example: `do_attack(gs, cn, co)` needs two characters mutably from the same Vec.

### Placement rules

- Use a **method on the entity type** (e.g., `impl Character { fn get_fight_skill(...) }`) when the function is clearly "about" that entity.
- Use a **free function in the module** (e.g., `combat::get_fight_skill(char, items)`) when the function is cross-cutting or doesn't belong to one entity.

### Why a separate pass

Changing ~5,000 call sites to `GameState` is already a large change. Layering entity-reference conversion on top increases risk. Doing it as a follow-up lets us:

- Verify the GameState refactor compiles and passes tests first
- Convert functions incrementally, one module at a time
- Use the compiler to guide us (functions that trigger borrow issues are natural Tier 1/2 candidates)

## Estimated Impact

- ~4,014 Repository closure calls eliminated (replaced with `self.`* or `gs.`* field access)
- ~818 State closure calls eliminated
- ~2 PathFinder closure calls eliminated
- All nested closure patterns removed
- ~30 files modified
- 3 singletons removed (Repository, State, PathFinder)
- 2 singletons remain (PLAYERS, NetworkManager)

