# Tick Parallelism + DB Migration Design

## Goals
- Parallelize per-character tick compute while preserving per-character ordering.
- Keep tick rate stable now; allow 30–35 later.
- Decouple persistence from tick logic; remove `.dat` files over time.
- Provide safety and observability (logging, metrics, fallback).

---

## 1) Concurrency Model (Job System + Command Buffers)

### Phases
1. **Snapshot/Read Phase (serial)**
   Prepare immutable views needed by workers (or read-only locks).
2. **Parallel Compute Phase**
   Workers process characters and emit commands to per-worker buffers.
3. **Serial Apply Phase**
   Merge buffers, apply commands in ascending `cn` order, preserving per-character order.
4. **Post-Apply Phase**
   Network updates, map diffs, and any read-after-write tasks.

### Command Buffer Rules
- Workers must not mutate shared state.
- Commands are idempotent where possible.
- Each command records:
  - `origin_cn`
  - `sequence` (monotonic per character)
  - command payload (e.g., `Move`, `Damage`, `SetFlag`, `Spawn`)

### Ordering Policy
- Preserve per-character ordering exactly:
  - Collect commands in per-character vectors, then apply in `cn` order.
- Cross-character ordering remains as today only insofar as it was serialized by `cn` order.

---

## 2) Persistence Strategy (DB Migration)

### Core Principle
Gameplay state is authoritative in memory; DB is write-behind.

### Write-Behind Pipeline
- After Apply Phase, append commands/deltas to a persistence queue.
- A background worker batches writes to the DB.

### Queue Backpressure Strategy
- Hard cap queue size (by count or bytes).
- When exceeding cap:
  - Option A: throttle ticks (add sleep or reduce work).
  - Option B: drop low-priority writes (analytics, cosmetic).
  - Option C: switch to snapshot-only mode temporarily.
- Always log and emit metrics when backpressure activates.

### Shutdown Semantics
- Attempt a graceful drain up to a time budget.
- If not drained, ensure snapshot is written and queue is persisted locally
  (or safe to replay).

---

## 3) Snapshot + Delta Model

- Periodic snapshots (e.g., every N minutes or N ticks).
- Command log between snapshots.
- On startup: load last snapshot + replay log.

This supports:
- Faster recovery.
- Bounded replay time.
- Partial data migration by domain.

---

## 4) DB Technology Options (High-level)

- **Postgres**: primary state tables; durable snapshots.
- **Redis** (optional): cache or transient queue.
- **NATS/Kafka/Redis Streams**: command log transport.
- **RocksDB/LMDB**: embedded local snapshot store (optional).

---

## 5) Migration Plan (Phased)

1. Introduce command buffers (no DB yet).
2. Add persistence queue (still `.dat` authoritative).
3. Dual-write to DB + `.dat`.
4. Switch reads per domain (items, chars, maps).
5. Remove `.dat` once parity validated.

---

## 6) Open Questions
- Which `.dat` domains to migrate first?
- Acceptable replay time on startup?
- Maximum tick delay allowed under backpressure?
- Desired durability (loss-tolerant vs loss-intolerant)?
