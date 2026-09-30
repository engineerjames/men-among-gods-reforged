# Server performance harness

`scripts/perf_loadtest.sh` runs the whole "measure the server under load" loop in
one command, and `scripts/analyze_perf_log.py` turns the collected data into a
bottleneck report.

## What a run does

1. Brings up KeyDB + the account API with `docker compose` (the game server is
   deliberately *not* containerised so profilers can attach to it).
2. Generates host TLS certificates if `certs/` is empty.
3. Builds the game server on the `profiling` cargo profile with
   `--features measure-time`, which turns every `core::measure!` site into a
   timed entry in `server_perf.log`.
4. Seeds the world snapshot into KeyDB if needed.
5. Starts that server natively and waits for `127.0.0.1:5555`.
6. Runs `mag-loadtest` with the requested client count and duration.
7. Samples the server process (CPU / RSS) throughout, and captures a macOS
   `sample` call-tree during the steady-state window.
8. Stops the server with `SIGINT` so it flushes its logs, then analyses
   everything.

All artifacts land in `perf-runs/<timestamp>/`:

| File | Contents |
| --- | --- |
| `perf_report.md` | The human-readable bottleneck report |
| `perf_summary.json` | Same data, machine-readable |
| `server-logs/server_perf.log` | Raw `measure!` timings, tick times, net I/O |
| `sample.txt` | macOS `sample` call-graph capture |
| `server_resources.csv` | CPU % and RSS over time |
| `loadtest.log` | Client-side metrics |
| `run_meta.json` | Run parameters and the steady-state window |

## Usage

```bash
# Defaults: 300 clients, 420s
./scripts/perf_loadtest.sh

# A serious run
./scripts/perf_loadtest.sh --clients 400 --duration 900

# Quick iteration against an already-built binary and running stack
./scripts/perf_loadtest.sh --clients 50 --duration 120 --skip-build --skip-stack

# Re-analyse an existing run (e.g. after improving the analyzer)
./scripts/perf_loadtest.sh --analyze-only perf-runs/2026-09-28_22-18-28
```

Run `./scripts/perf_loadtest.sh --help` for the full option list.

## Prerequisites

* A populated `.env` at the repo root (`KEYDB_PASSWORD`, `API_JWT_SECRET`,
  `MAG_GOD_PASSWORD`).
* `docker`, `cargo`, `python3`. The macOS `sample` capture is optional.

## Reading the report

* **Headroom** — busy time vs. wall time. The tick loop is single threaded, so
  "100% of one core" means saturated no matter how many cores the box has.
* **Where busy time goes (inclusive)** — each `measure!` label's share of the
  work the tick loop actually does. `server.tick` is excluded from the
  denominator because it also covers the sleep that paces the loop to 36 TPS.
* **Hot spots (exclusive)** — a label's own time minus its instrumented
  children. A large exclusive value means un-instrumented work lives there and
  is a good place to add another `measure!`.
* **Sampled self time** — per-thread, from `sample`. Use it to attribute cost
  *within* a phase. Note that release-profile inlining can fold callees into
  their caller, so a large `Server::game_tick` self time is expected.

## Caveats

* `measure-time` writes a line per instrumented site per tick. That logging is
  itself measurable overhead (a few percent), so treat absolute numbers as
  slightly pessimistic; the *relative* attribution is what matters.
* Avoid per-player `measure!` calls — at 400 players that is tens of thousands
  of log lines per second and will dominate the profile. Accumulate across the
  loop and log once per tick instead (see `player.getmap` / `player.change` in
  `server/src/server.rs`).
* All bots share one source IP, so they collectively hit the API's per-IP
  limiter (30 req/s). `--api-rps` / `--api-concurrency` default to values that
  stay under it while still hiding the API's server-side Argon2 latency.
