# sqlite-bench

**SQLite In-Memory Latency Benchmark** — Evaluates whether SQLite (`:memory:` with WAL mode)
can serve as a drop-in replacement for the server's `.dat` flat-array storage while meeting
the 36 TPS (27.78 ms/tick) latency budget.

## Purpose

The game server currently loads all world state from `.dat` files into flat `Vec<T>` arrays at
startup, accessing them via direct `O(1)` indexing (`ch[n].x`, `map[idx].ch`, `items[in_id].flags`).
This is extremely fast but provides no ACID guarantees, no incremental persistence, and no
crash recovery.

This benchmark measures whether SQLite in-memory mode can match the required throughput by
simulating a realistic tick workload — ~150K reads and ~5K writes per tick across characters,
items, map tiles, effects, and global state.

## Schema Strategies

Two approaches are benchmarked:

- **BLOB schema** — Array fields (`attrib[5][6]`, `skill[50][6]`, `item[40]`, `data[100]`, etc.)
  stored as fixed-size BLOBs in flat tables. Simpler schema, single row per entity.
- **Normalized schema** — Array fields broken into relational sub-tables with composite primary
  keys (`character_skills(char_id, skill_id, v0..v5)`). More JOINs, but enables per-element
  queries and proper relational integrity.

Both use:
- `PRAGMA journal_mode = WAL`
- `PRAGMA synchronous = OFF`
- `PRAGMA cache_size = -131072` (128 MB)
- `PRAGMA mmap_size = 268435456` (256 MB)
- Partial indexes (`WHERE used != 0`, `WHERE ch != 0`)

## No Impact on Normal Builds

This crate is **excluded** from the workspace (`Cargo.toml` → `exclude = ["server/sqlite-bench"]`).

- `cargo build` / `cargo test` / `cargo clippy` from the workspace root **will not** compile this crate
- `rusqlite` and `libsqlite3-sys` are **never** pulled into the workspace dependency graph
- CI is unaffected

## Usage

```bash
# From the sqlite-bench directory:
cd server/sqlite-bench

# Run benchmarks (Criterion — statistical analysis + HTML reports)
cargo bench

# Run tests (verifies schemas, population, and tick sim correctness)
cargo test

# Use real .dat files instead of synthetic data:
MAG_DAT_PATH=../assets/.dat cargo bench
```

## Interpreting Results

Criterion produces HTML reports in `target/criterion/`:

- `tick/blob/std` — BLOB schema, standard population (50 players, 500 NPCs)
- `tick/blob/stressed` — BLOB schema, stressed population (250 players, 2000 NPCs)
- `tick/normalized/std` — Normalized schema, standard population
- `tick/normalized/stressed` — Normalized schema, stressed population

**Key metrics:**
- **Mean tick time** — must be < 27,778 µs (27.78 ms) to sustain 36 TPS
- **p95/p99** — tail latency spikes that could cause tick drift
- **Max sustainable TPS** — `1,000,000 / mean_tick_µs`

## Population Levels

| Level    | Players | NPCs  | Active Items | Effects | Total Chars |
|----------|---------|-------|-------------|---------|-------------|
| Standard | 50      | 500   | 5,000       | 50      | 550         |
| Stressed | 250     | 2,000 | 20,000      | 200     | 2,250       |

Array capacities match the server constants: 8,192 character slots, 98,304 item slots,
1,048,576 map tiles, 4,096 effect slots.

## Tick Simulation

Each benchmark iteration replays the full server tick access pattern:

1. **Global counter update** (1 write)
2. **Character triage** — `SELECT ... WHERE used != 0` (~600 rows)
3. **Viewport reads** — 34×34 map tile reads per player + character/item lookups for visible entities
4. **Player delta sync** — character stats + equipment slot reads
5. **Regeneration** — per-active-character stat read/write + spell item checks
6. **Movement** — map tile ownership swaps + 21×21 light area updates
7. **NPC AI** — character data reads + item lookups + pathfinding map reads
8. **Really update char** — full stat recalculation from worn items + spells
9. **Effect triage + update** — effect duration decrements
10. **Item expiration** — 4 map row scans + item age checks
11. **Item GC** — 256-item round-robin integrity sweep

Access pattern counts are based on analysis of `server/src/server.rs` `game_tick()`.
