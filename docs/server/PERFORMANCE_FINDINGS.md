# Server performance analysis — 400 concurrent players

Measured with `scripts/perf_loadtest.sh` (see
[PERFORMANCE_HARNESS.md](PERFORMANCE_HARNESS.md)) on 2026-09-28.

| Run | Clients | Steady window | Artifacts |
| --- | --- | --- | --- |
| A | 400 | 590 s | `perf-runs/2026-09-28_22-18-28` |
| B | 130 | 350 s | `perf-runs/2026-09-28_22-40-05` (finer `getmap`/`change` split) |

Host: Apple Silicon, 12 cores, 36 GB. Server built with `--profile profiling
--features measure-time`; KeyDB and the account API in Docker.

## Verdict

At 400 players the server is **saturated on a single core** and can no longer
hold its tick rate.

| Metric | Value | Target |
| --- | --- | --- |
| CPU | 99.4% of *one* core | — |
| Mean tick time | 30.89 ms | 27.78 ms |
| Ticks over budget | 899 / 899 (100%) | 0% |
| Delivered tick rate | 30.6 TPS | 36 TPS |
| Reported load | p50 111%, p95 118% | < 100% |
| RSS | 753 MB | — |

The other 11 cores are idle. The tick loop is single threaded, so the machine
is 8% utilised while the game is already degrading.

## Where the time goes

Share of the work the tick loop actually performs (excluding the sleep that
paces it):

| Phase | % busy | ms / tick |
| --- | --- | --- |
| `player.send_normal_state_updates` | **84.9%** | 27.82 |
| `character.main_tick` | 7.5% | 2.46 |
| `handle_network_io` | 4.5% | 1.47 |
| `compress_ticks` | 1.6% | 0.53 |
| `player.process_commands_and_idle` | 1.2% | 0.40 |
| everything else combined | < 0.2% | ~0.02 |

Populate, effects, auras, weather, item driver and background-save scheduling
are all free. **There is exactly one hot spot.**

Run B splits that hot spot:

| Sub-phase | % busy | per player per tick |
| --- | --- | --- |
| `player.getmap` (`plr_getmap_complete`) | 39.4% | ~36 µs |
| `player.change` (`plr_change`) | 29.0% | ~27 µs |

Both scale linearly with player count: ~63 µs/player/tick × 400 players ≈
25 ms, which matches the 27.8 ms measured in run A. **Cost is O(players), and
the constant is what breaks the budget.**

Sampled self time on the game thread (macOS `sample`; note that release
inlining folds callees into `Server::game_tick`):

| Frame | % busy |
| --- | --- |
| `Server::game_tick` (inlined `plr_getmap`/`plr_change` bodies) | 54.5% |
| `_platform_memmove` | 9.2% |
| `GameState::do_area_notify` | 6.6% |
| `miniz_oxide::deflate::core::compress_inner` | 5.1% |
| `npc::npc_try_spell` | 4.8% |
| `GameState::do_regenerate` | 2.5% |
| visibility (`do_char_can_see` + `close_vis_see` + `can_map_see`) | 3.0% |

## The five things holding performance back

### 1. `plr_getmap_complete` rebuilds an 80×80 view from scratch, every player, every tick

`server/src/player/map.rs` walks a 76×76 window (after the edge cuts) for each
player on every tick, regardless of whether anything in it changed. At 400
players and 30 TPS that is **~69 million tile evaluations per second**, and
each one does map lookups, visibility checks, character lookups and item
lookups.

Most of that work is redundant: a player who has not moved, in an area where
nothing moved, produces a byte-identical view.

Directions worth exploring:
* Skip the scan entirely when the player did not move, daylight did not change,
  and no character/item inside the window was touched this tick (a per-area
  dirty counter or version stamp would make this a single comparison).
* Rebuild only the rows/columns that scroll in when the player moves one tile,
  instead of the whole window.
* Stagger full refreshes across ticks so each player gets a full rebuild every
  N ticks and a delta otherwise.

### 2. The same function copies ~180 KB per player per tick for no reason

```rust
let mut smap = gs.players[nr].smap;   // copies [CMap; 6400] by value
...
gs.players[nr].smap[n] = smap[n];     // ...then writes each tile back
```

`CMap` is 28 bytes and `TILEX * TILEY` is 6400, so this copies about 180 KB
into a local, mutates it, and copies it back element by element. At 400
players × 30 TPS that is **over 2 GB/s of pure memcpy**, which is almost
certainly the 9.2% `_platform_memmove`.

This one is nearly free to fix: the copy exists only to dodge a borrow-checker
conflict with `gs`. Splitting the borrow (or indexing `gs.players[nr].smap`
directly where possible) removes it outright.

### 3. Outgoing tick data is zlib-compressed at level 9

`server/src/server.rs` creates every player's encoder with
`ZlibEncoder::new(Vec::new(), Compression::best())`. Level 9 is the slowest
setting in the library and buys very little on the small, repetitive packets
this protocol sends — that is 5.1% of the game thread in `compress_inner`.

`Compression::new(1)` or `new(6)` should cut most of that cost for a marginal
change in bandwidth. Worth measuring both ratio and time before picking.

### 4. `act_idle` broadcasts over a 25×25 tile block for every character

`server/src/driver/generic.rs::act_idle` calls `do_area_notify` whenever
`(ticker & 15) == (cn & 15)`, and `do_area_notify` scans
`(2 × AREA_SIZE + 1)² = 625` tiles, dispatching `driver_msg` to each occupant.
Every character does this every 16 ticks — 6.6% of the game thread, and it
grows with both character count and crowding.

A spatial index of occupied tiles (or a per-area occupant list) would replace
the 625-tile scan with an iteration over actual occupants, which is usually a
handful.

### 5. Per-player file logging is unbudgeted I/O on the game thread

Run A produced **400 log files of roughly 550 KB each — about 220 MB in 15
minutes** — written synchronously from the tick loop. `write`/`writev` account
for 3.7% of sampled game-thread time, and `log4rs` pattern encoding shows up
on other threads too.

This is worth either sampling down, buffering, or moving to a dedicated writer
thread.

## Suggested order of work

1. **Remove the `smap` copy** (#2) — smallest change, immediate ~9% win.
2. **Lower the zlib level** (#3) — one-line change, ~5% win.
3. **Make `plr_getmap` incremental/conditional** (#1) — by far the biggest
   prize; ~40% of busy time and the thing that makes cost linear in players.
4. **Index `do_area_notify`** (#4) — ~7%, and it also helps crowded areas.
5. **Get logging off the tick thread** (#5) — ~4%, plus less disk pressure.

Items 1–3 alone would plausibly bring the 400-player tick back inside budget.
Beyond that, the structural ceiling is that the tick loop is single threaded:
per-player view building is embarrassingly parallel and is the obvious
candidate if the game needs to scale past ~500 players.

## Two defects found while building the harness

* **`api/src/rate_limit.rs` could lock an IP out permanently.** The per-IP
  counter did `INCR` and then `EXPIRE` as two separate round trips, setting the
  TTL only when the count came back as 1. Under load the API's Redis calls time
  out; when the `EXPIRE` was the call that timed out, `rate:public:<ip>` was
  left with **no TTL**, so the counter grew forever and every subsequent
  request from that IP got a 429 until the key was deleted by hand. Observed
  live (`TTL` = -1, value 583). Fixed by making the increment and expiry a
  single atomic Lua script.

* **The load generator could not saturate the server.** `RateLimiter` held a
  global mutex for each request's full duration, so real throughput was
  `1 / request_latency` — and with server-side Argon2 in the path that is well
  under 1 req/s. Bootstrapping 400 bots took longer than the test itself.
  Replaced the mutex with a semaphore and added `[api] max_in_flight` /
  `--api-concurrency` (default still 1, so existing behaviour is unchanged).
