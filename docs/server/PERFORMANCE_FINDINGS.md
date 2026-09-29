# Server performance analysis — 400 concurrent players

Measured with `scripts/perf_loadtest.sh` (see
[PERFORMANCE_HARNESS.md](PERFORMANCE_HARNESS.md)) on 2026-09-28/29.

| Run | Clients | Steady window | Artifacts |
| --- | --- | --- | --- |
| A (baseline) | 400 | 590 s | `perf-runs/2026-09-28_22-18-28` |
| B | 130 | 350 s | `perf-runs/2026-09-28_22-40-05` (finer `getmap`/`change` split) |
| C (after fixes) | 400 | 450 s | `perf-runs/2026-09-29_07-33-10` |

Host: Apple Silicon, 12 cores, 36 GB. Server built with `--profile profiling
--features measure-time`; KeyDB and the account API in Docker.

## Results after the first round of fixes

| Metric | Before (A) | After (C) | |
| --- | --- | --- | --- |
| Mean tick time | 30.89 ms | **27.07 ms** | −12% |
| Tick p95 / p99 | 32.80 / 36.00 ms | **28.49 / 30.09 ms** | |
| Ticks over the 27.78 ms budget | 899 / 899 (100%) | **124 / 785 (15.8%)** | |
| Delivered tick rate | 30.59 TPS | **34.93 TPS** | +14% |
| Reported load p50 / p95 | 111% / 118% | **96% / 102%** | |
| `send_normal_state_updates` | 27.82 ms/tick | **24.19 ms/tick** | −13% |
| `compress_ticks` | 0.532 ms/tick | **0.196 ms/tick** | −63% |
| `miniz` in the sampled profile | 5.1% | **absent** | |
| `_platform_memmove` sampled self | 3311 samples | **1932 samples** | −42% |

The server is still CPU-bound at 400 players, but it now very nearly holds its
tick rate instead of missing every single tick.

> **Caveat on comparability.** Run C used `--reset-world`, so its world had a
> different character/item layout and player dispersion than run A. The
> `compress_ticks` and `miniz` numbers are unambiguous (same workload shape,
> direct consequence of the compression-level change); the
> `send_normal_state_updates` delta is directionally right but carries some
> world-layout noise. A controlled A/B on an identical world would tighten it.

### What changed

1. **Removed the per-player `smap` copy** — `plr_getmap_complete` cloned the
   whole `[CMap; 6400]` (~180 KB) into a local and wrote it back element by
   element. It now uses a single 28-byte scratch tile. This was over 2 GB/s of
   pure memcpy at 400 players.
2. **Dropped zlib from level 9 to level 1** for per-player tick payloads
   (`TICK_COMPRESSION_LEVEL` in `server/src/server.rs`). These packets are
   small and repetitive, so the extra search effort bought almost no ratio.
3. **Hoisted per-player light constants** out of the ~5.8k-iteration tile loop
   and reused the already-copied map tile instead of re-indexing `gs.map[mi]`
   and re-reading the character on every tile.
4. **Made `do_area_notify` a tight row-slice scan** that collects occupants
   first and dispatches second, so the hot 25×25 walk is no longer interleaved
   with calls into `driver_msg`.

### Where the remaining time goes (run C)

| Phase | % busy | ms / tick |
| --- | --- | --- |
| `player.getmap` | 49.4% | 14.14 |
| `player.change` | 34.7% | 9.93 |
| `character.main_tick` | 8.3% | 2.37 |
| `handle_network_io` | 5.0% | 1.44 |
| `compress_ticks` | 0.7% | 0.20 |

`plr_getmap` is now clearly the single thing worth attacking next, and the
cheap wins there are used up — what is left is the structural problem below.

## Original analysis (baseline, run A)

At 400 players the server was **saturated on a single core** and could not
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

**Status: partially addressed** (items 1 and 3 above). The remaining
structural problem is unchanged.

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

Note that `plr_change` has the same shape — it diffs all 6400 tiles twice
(once in `plr_change_light`, once in `plr_change_map`) — so a dirty-region
scheme would pay off on both halves.

### 2. The same function copies ~180 KB per player per tick for no reason

**Status: fixed.**

```rust
let mut smap = gs.players[nr].smap;   // copies [CMap; 6400] by value
...
gs.players[nr].smap[n] = smap[n];     // ...then writes each tile back
```

`CMap` is 28 bytes and `TILEX * TILEY` is 6400, so this copied about 180 KB
into a local, mutated it, and copied it back element by element. At 400
players × 30 TPS that was **over 2 GB/s of pure memcpy**.

### 3. Outgoing tick data is zlib-compressed at level 9

**Status: fixed** — now level 1 via `TICK_COMPRESSION_LEVEL`.

### 4. `act_idle` broadcasts over a 25×25 tile block for every character

**Status: partially addressed** — the scan is tighter, but it is still a
625-tile walk.

`server/src/driver/generic.rs::act_idle` calls `do_area_notify` whenever
`(ticker & 15) == (cn & 15)`, and `do_area_notify` scans
`(2 × AREA_SIZE + 1)² = 625` tiles. Every character does this every 16 ticks,
and it grows with both character count and crowding.

A spatial index of occupied tiles (or a per-area occupant list) would replace
the 625-tile scan with an iteration over actual occupants, which is usually a
handful. That was not attempted here because `map[..].ch` is written from
~30 call sites with no single choke point, so maintaining an index safely is a
larger refactor.

### 5. Per-player file logging is unbudgeted I/O on the game thread

**Status: not addressed.**

Run A produced **400 log files of roughly 550 KB each — about 220 MB in 15
minutes** — written synchronously from the tick loop. `write`/`writev` account
for ~4% of sampled game-thread time, and `log4rs` pattern encoding shows up
on other threads too.

This is worth either sampling down, buffering, or moving to a dedicated writer
thread.

## Suggested next steps

1. **Make `plr_getmap` and `plr_change` incremental** — by far the biggest
   remaining prize at 84% of busy time, and the thing that makes cost linear
   in players.
2. **Index `do_area_notify`** — ~14% of sampled game-thread self time.
3. **Get logging off the tick thread** — ~4%, plus less disk pressure.

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
