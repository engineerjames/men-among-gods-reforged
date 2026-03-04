# GameState Singleton Refactor — Implementation Plan

## Goal

Replace 3 global singletons (`Repository`, `State`, `PathFinder`) with a single
owned `GameState` struct created in `main()` and threaded as `&mut GameState`
through the call chain. This eliminates **~4,434 closure-based accessor calls**
and all nested closure patterns.

Two singletons remain: `PLAYERS` (169 calls) and `NetworkManager` (61 calls).
These are I/O/connection concerns, not game state.

---

## Phase 0: Create GameState struct and migration bridge (2 files)

**Goal:** Get `GameState` compiling alongside the old singletons so we can
migrate incrementally, one module at a time.

### 0A. Create `server/src/game_state.rs`

Define the struct with all fields from `Repository` + `State` + `PathFinder`:

```rust
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

    // -- Pathfinding --
    pub pathfinder: PathFinder,

    // -- Persistence (private) --
    storage_backend: StorageBackend,
    saved_cleanly: bool,
    executable_path: String,
}
```

Implement on `GameState`:
- `pub fn new(backend: StorageBackend) -> Self` — allocate all collections
- `pub fn initialize() -> Result<GameState, String>` — calls `new`, `load`, returns owned struct
- `fn load(&mut self) -> Result<(), String>` — moved from Repository (both dat and keydb paths)
- `pub fn save(&mut self) -> Result<(), String>` — moved from Repository
- `pub fn shutdown(&mut self)` — clears dirty flag, saves, sets `saved_cleanly`
- All file I/O helpers (`get_dat_file_path`, `load_map`, `save_map`, etc.) — moved from Repository
- `normalize_message_of_the_day`, `read_message_of_the_day_from_dat_file` — moved from Repository

**Tests:** `initialize` roundtrip (requires `.dat` or `#[ignore]`), MOTD normalization.

### 0B. Update `server/src/main.rs`

```rust
mod game_state;

fn main() -> Result<(), String> {
    // ...
    let mut gs = game_state::GameState::initialize()?;
    // dirty flag check: gs.globals.is_dirty()
    handle_command_line_args(&args, &mut gs);
    let mut server = Server::new();
    server.initialize(&mut gs)?;
    while !quit_flag.load(Ordering::SeqCst) {
        server.tick(&mut gs);
    }
    // logout all players...
    server.shutdown_background_saver();
    gs.shutdown();
}
```

Remove `Repository::initialize()`, `PathFinder::initialize()`, `State::initialize()` calls.

### 0C. Update `handle_command_line_args`

Change signature to `fn handle_command_line_args(args: &[String], gs: &mut GameState)`.
Pass `gs` to populate subcommands instead of them using Repository statics.

**Checkpoint:** `cargo build` compiles. Old singletons are no longer initialized
but still exist as dead code. Nothing calls them yet because the calling modules
haven't been converted.

---

## Phase 1: Convert `server.rs` (77 combined calls)

**Why first:** This is the entry point that calls everything else. Converting it
gives us the `&mut GameState` parameter flowing down into `game_tick`,
`handle_network_io`, `compress_ticks`, etc.

### Changes

| Method | Change |
|---|---|
| `Server::initialize(&mut self)` | Add `gs: &mut GameState` param. Remove `State::initialize()` call. |
| `Server::tick(&mut self)` | Add `gs: &mut GameState` param. Replace `Repository::with_globals(...)` with `gs.globals`. |
| `Server::game_tick(&mut self)` | Add `gs: &mut GameState` param. Replace all `Repository::with_*` and `State::with*` closures with direct `gs.*` access. Calls into `player::*`, `populate::*`, `driver::*`, `EffectManager::*` still use old singletons (converted later). |
| `Server::maybe_enqueue_background_save(&mut self)` | Add `gs: &GameState` param. Clone from `gs.characters`, `gs.items`, `gs.map`, etc. instead of `Repository::with_*`. |
| `Server::wakeup_character`, `group_active`, `check_expire`, `check_valid`, `global_tick`, `tmplabcheck` | Add `gs: &mut GameState` param. |
| `Server::handle_network_io` | Add `gs: &mut GameState` param. (PLAYERS singleton stays.) |
| `Server::compress_ticks` | No `gs` param needed (only uses PLAYERS). |

**Borrow pattern for background save:** `maybe_enqueue_background_save` only
needs `&GameState` (read-only cloning). Call it before any `&mut` operations in
the same tick, or use split borrows since `self` (Server) and `gs` are separate.

---

## Phase 2: Convert State submodules (818 calls, 12 files, ~146 functions)

**Why next:** These are methods on `State` that take `&self`/`&mut self`. We
change `impl State` → `impl GameState` in each file. Since GameState owns both
the State fields (visi, etc.) and the Repository fields (characters, items,
etc.), every `Repository::with_*` closure inside these methods becomes direct
`self.*` access.

### Conversion pattern

**Before** (e.g., `state/combat.rs`):
```rust
impl State {
    pub(crate) fn get_fight_skill(&self, cn: usize) -> i32 {
        let in_idx = Repository::with_characters(|ch| ch[cn].worn[WN_RHAND] as usize);
        let flags = Repository::with_items(|it| it[in_idx].flags);
        // ...
    }
}
```

**After:**
```rust
impl GameState {
    pub(crate) fn get_fight_skill(&self, cn: usize) -> i32 {
        let in_idx = self.characters[cn].worn[WN_RHAND] as usize;
        let flags = self.items[in_idx].flags;
        // ...
    }
}
```

### File order (by combined call count, descending)

Convert in this order to maximize singleton call elimination per file:

| # | File | Repo calls | State calls | Total |
|---|------|-----------|-------------|-------|
| 1 | `state/stats.rs` | 168 | 2 | 170 |
| 2 | `state/communication.rs` | 113 | 1 | 114 |
| 3 | `state/death.rs` | 71 | 2 | 73 |
| 4 | `state/commerce.rs` | 66 | 0 | 66 |
| 5 | `state/player_actions.rs` | 65 | 0 | 65 |
| 6 | `state/combat.rs` | 60 | 0 | 60 |
| 7 | `state/inventory.rs` | 49 | 4 | 53 |
| 8 | `state/commands.rs` | 38 | 1 | 39 |
| 9 | `state/economy.rs` | 28 | 1 | 29 |
| 10 | `state/logging.rs` | 27 | 0 | 27 |
| 11 | `state/visibility.rs` | 36 | 0 | 36 |
| 12 | `state/admin.rs` | 19 | 0 | 19 |

### Associated functions (no `self` receiver)

A few State functions are associated (no receiver):
- `combat.rs`: `add_enemy`, `remove_enemy`
- `logging.rs`: `char_play_sound`, `do_area_sound`, `process_options`
- `visibility.rs`: `check_dlight`, `check_dlightm`

These already access `Repository` statics directly. Convert to free functions
taking `gs: &mut GameState` (or `&GameState` when read-only). Move them on
`impl GameState` only if they naturally fit as methods.

### `state/mod.rs` changes

- Remove `static STATE: OnceLock<...>`, the `State` struct, `State::new()`,
  `State::initialize()`, `with`/`with_mut` methods.
- Keep the file as a module declaration hub, but change submodule `impl State`
  blocks to `impl GameState`.
- Add `use crate::game_state::GameState;` to each submodule.

### Callers

After converting all submodules, update every external call site:

| Old pattern | New pattern |
|---|---|
| `State::with(\|state\| state.do_say(cn, text))` | `gs.do_say(cn, text)` |
| `State::with_mut(\|state\| state.do_attack(cn, co, false))` | `gs.do_attack(cn, co, false)` |
| `State::with(\|state\| state.do_update_char(cn))` | `gs.do_update_char(cn)` |

External files that call State methods (and their call counts):
- `driver/use_item.rs` (226), `driver/skill.rs` (197), `god.rs` (149),
  `player.rs` (87), `driver/npc.rs` (54), `talk.rs` (35),
  `driver/generic.rs` (15), `lab9.rs` (13), `driver/special.rs` (13),
  `helpers.rs` (6), `driver/look.rs` (4), `populate.rs` (4),
  `server.rs` (3), `effect.rs` (1)

These callers are converted in Phases 3–4 (but the State call sites within them
become `gs.method(...)` calls once the caller has a `gs` param).

---

## Phase 3: Convert standalone modules (9 files, ~1,046 combined calls)

Each module's public functions gain a `gs: &mut GameState` parameter.
`Repository::with_*` closures become direct `gs.*` field access.
`State::with(|s| s.method(...))` becomes `gs.method(...)`.

### File order (by combined call count, descending)

| # | File | Repo | State | S::w_players | NM | Total |
|---|------|------|-------|-------------|-----|-------|
| 1 | `player.rs` | 367 | 87 | 155 | 45 | **454+** |
| 2 | `god.rs` | 155 | 149 | 4 | 0 | **304+** |
| 3 | `populate.rs` | 84 | 4 | 0 | 0 | **88** |
| 4 | `server.rs` | (done in Phase 1) | | | | |
| 5 | `talk.rs` | 42 | 35 | 0 | 0 | **77** |
| 6 | `effect.rs` | 65 | 1 | 0 | 0 | **66** |
| 7 | `lab9.rs` | 34 | 13 | 0 | 0 | **47** |
| 8 | `helpers.rs` | 20 | 6 | 0 | 0 | **26** |
| 9 | `area.rs` | 2 | 0 | 0 | 0 | **2** |
| 10 | `points.rs` | — | — | — | — | (already pure) |

### `player.rs` special considerations

`player.rs` has the highest call count (454+). It also heavily uses
`Server::with_players` (155 calls) and `NetworkManager::with` (45 calls).
Those singletons remain, so those calls stay as-is. Only `Repository::with_*`
and `State::with*` calls are replaced.

Key functions to convert:
- `plr_cmd(n)` → `plr_cmd(gs, n)`
- `plr_login(n)` → `plr_login(gs, n)`
- `plr_logout(usnr, n, reason)` → `plr_logout(gs, usnr, n, reason)`
- `plr_tick(n)` → `plr_tick(gs, n)`
- `plr_state(n)` → `plr_state(gs, n)`
- `plr_act(n)` → `plr_act(gs, n)`
- `plr_getmap(n)` → `plr_getmap(gs, n)`
- `plr_change(n)` → `plr_change(gs, n)`
- `plr_idle(n)` → `plr_idle(gs, n)`

### `god.rs` special considerations

`God` is a struct with associated functions (not instance methods). Convert to:
- `God::give_item(gs, cn, item_id)` instead of `God::give_item(cn, item_id)`
- Or convert to free functions: `god::give_item(gs, cn, item_id)`

### Downstream cascade

Converting `populate.rs` means all `pop_*` functions called from
`handle_command_line_args` and `game_tick` need `gs` param. The command-line
path already has `gs` from Phase 0C.

---

## Phase 4: Convert driver modules (6 files, ~1,660 combined calls)

These are the highest-call-count files. Each driver function gains a
`gs: &mut GameState` parameter.

| # | File | Repo | State | Total |
|---|------|------|-------|-------|
| 1 | `driver/use_item.rs` | 761 | 226 | **987** |
| 2 | `driver/skill.rs` | 486 | 197 | **683** |
| 3 | `driver/npc.rs` | 328 | 54 | **382** |
| 4 | `driver/generic.rs` | 305 | 15 | **320** |
| 5 | `driver/special.rs` | 140 | 13 | **153** |
| 6 | `driver/look.rs` | 6 | 4 | **10** |

### `driver/generic.rs` — PathFinder call

This file has the only external `PathFinder::with_mut` call:
```rust
PathFinder::with_mut(|pf| pf.find_path(...));
```

After conversion:
```rust
// Split borrow: pathfinder is one field, map/items/characters are others
gs.pathfinder.find_path(&gs.map, &gs.items, &gs.characters, ...);
```

This requires modifying `PathFinder::find_path` to take data slices as
parameters instead of calling `Repository::with_*` internally (see Phase 5).

### Entry points

`server.rs::game_tick` calls into drivers via:
```rust
driver::generic_driver(n);  // becomes driver::generic_driver(gs, n)
driver::item_tick();         // becomes driver::item_tick(gs)
```

The `driver/mod.rs` re-exports must be updated to match new signatures.

---

## Phase 5: Convert PathFinder (6 internal Repository calls)

### Changes to `path_finding.rs`

1. Remove `static PATHFINDER: OnceLock<...>`, `PathFinder::initialize()`,
   `PathFinder::with`, `PathFinder::with_mut`.
2. Make `PathFinder` a plain struct (it already is, minus the static).
3. Change `find_path` to take data as parameters:

**Before:**
```rust
pub fn find_path(&mut self, ...) -> bool {
    Repository::with_map(|map| {
        Repository::with_characters(|ch| {
            // search logic
        })
    })
}
```

**After:**
```rust
pub fn find_path(
    &mut self,
    map: &[Map],
    items: &[Item],
    characters: &[Character],
    // ... other needed slices
) -> bool {
    // direct access to map, items, characters
}
```

4. Caller in `driver/generic.rs` uses split borrows:
```rust
gs.pathfinder.find_path(&gs.map, &gs.items, &gs.characters, ...);
```

This compiles because `pathfinder`, `map`, `items`, `characters` are all
separate fields of `GameState`.

---

## Phase 6: Cleanup

### Delete/modify files

| File | Action |
|---|---|
| `server/src/repository.rs` | **Delete entirely** |
| `server/src/state/mod.rs` | Remove `State` struct, keep as re-export hub for submodules |
| `server/src/single_thread_cell.rs` | Keep (still used by `PLAYERS`, `NetworkManager`) |
| `server/src/path_finding.rs` | Remove singleton boilerplate, keep `PathFinder` struct |

### Update `server/src/main.rs` module declarations

- Add `mod game_state;`
- Keep `mod repository;` removal (it's gone)
- Keep `mod state;` (submodules still exist, just `impl GameState` now)

### Update `server/src/lib.rs`

- If `keydb`, `keydb_store`, or `points` remain exposed, no change needed.
- If anything depended on `Repository` publicly, update.

### Remove unused imports

Grep for remaining `use crate::repository::Repository` and
`use crate::state::State` across all files. Remove them.

### Run quality gate

```bash
cargo test && cargo clippy -- -D warnings
```

---

## Phase 7 (Follow-up): Entity-Reference Signatures

*Not part of this plan's scope.* After all phases above are merged and stable,
a separate pass converts suitable functions from index-based (`cn: usize`) to
entity-reference (`&Character` / `&mut Character`) signatures. See the original
plan document for Tier 1/2/3 classification.

---

## Risk Analysis

### Borrow checker — split borrows

Rust allows borrowing different fields of a struct simultaneously. Most
conversions are straightforward (`gs.characters[cn]` + `gs.items[in_idx]`).

**Known risk:** `PathFinder::find_path` needs `&mut gs.pathfinder` +
`&gs.map` + `&gs.characters`. This works via split borrows because they're
separate fields. Verified pattern:

```rust
let pf = &mut gs.pathfinder;
let map = &gs.map;
pf.find_path(map, ...); // compiles: different fields
```

**Known risk:** Functions that call `self.method_a()` which internally accesses
`self.characters`, while the caller also holds a reference to
`self.characters` — this would fail. Solution: these become index-based (pass
`cn: usize`, let the callee access `self.characters[cn]`), which is already
the pattern used everywhere.

### Nested Repository closures

~20 call sites nest `Repository::with_characters(|ch| Repository::with_items(|it| ...))`
(these work today because `SingleThreadCell` uses a `ReentrantMutex`). These
become direct field access (`self.characters[cn]` + `self.items[in_idx]`) and
are actually *simpler* after conversion.

### Command-line subcommands

`handle_command_line_args` calls `populate::populate()`, etc. These currently
use `Repository` statics. After Phase 0C, they receive `gs: &mut GameState`.
The subcommands call `process::exit(0)` so `gs` doesn't need to survive past them.

### Background saver

`maybe_enqueue_background_save` clones data from Repository to send to the
background thread. After conversion, it clones from `gs.characters.clone()`,
`gs.items[..half].to_vec()`, etc. No borrow issue since `.clone()`/`.to_vec()`
creates owned data.

### `State::with` called from within State submodules

A few State submodule methods call `State::with(|s| s.other_method(...))` to
call sibling methods. After conversion to `impl GameState`, these become
`self.other_method(...)` — simpler and more natural.

### `Server::with_players` / `NetworkManager::with` — no change

These remain as singletons. Functions that use both `gs: &mut GameState` and
`Server::with_players(|p| ...)` simply do:
```rust
fn plr_cmd(gs: &mut GameState, n: usize) {
    let usnr = Server::with_players(|p| p[n].usnr);
    gs.characters[usnr].hp += 10;
    // etc.
}
```

No conflict because `gs` and `PLAYERS` are separate memory.

---

## Summary Statistics

| What | Count |
|---|---|
| Files modified | ~30 |
| Files deleted | 1 (`repository.rs`) |
| Files created | 1 (`game_state.rs`) |
| Closure calls eliminated | ~4,434 |
| Functions gaining `gs` param | ~200+ (standalone + driver) |
| Functions changing `impl State` → `impl GameState` | ~146 |
| Singletons removed | 3 (Repository, State, PathFinder) |
| Singletons remaining | 2 (PLAYERS, NetworkManager) |

## Implementation Order Summary

```
Phase 0: GameState struct + main.rs wiring           (~2 files)
Phase 1: server.rs                                    (~1 file, 77 calls)
Phase 2: State submodules (impl State → impl GS)     (~12 files, 818 calls)
Phase 3: Standalone modules                           (~9 files, 1,046 calls)
Phase 4: Driver modules                               (~6 files, 1,660 calls)
Phase 5: PathFinder                                   (~1 file, 6 calls)
Phase 6: Cleanup (delete repo.rs, remove imports)     (~30 files)
Phase 7: Entity-reference signatures                  (future, separate PR)
```

Each phase should compile and pass `cargo test` independently. Phases 2–4 are
the bulk of the work and can each be broken into per-file PRs if desired.
