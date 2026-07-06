# Map Editor Operations

This runbook covers two supported map update paths:

- Live apply on a running server (default production path).
- Offline snapshot edit and maintenance-window import.

Use live apply when you need a targeted patch without a restart. Use offline import when shipping larger map revisions that should land as a single maintenance event.

## Prerequisites

- Admin API token (`MAG_ADMIN_API_TOKEN`) with access to `/admin/world/map/*`.
- API base URL (`MAG_API_BASE_URL`).
- KeyDB connectivity from API and server.
- A rollback snapshot captured immediately before any production change.

## Path A: Live Apply on Running Server

### 1. Capture rollback snapshot

From repo root:

```bash
./scripts/save_world_snapshot.sh --prefix pre_map_edit
```

Record the generated `.wsnap` path in your change ticket.

### 2. Stage edits

1. Open `map_viewer` in live API mode.
2. Make map edits.
3. Use `Dry-run publish` to validate pending dirty tiles.

Dry run does not enqueue patches; it verifies that a change-set can be built from current edits.

### 3. Publish patches

Choose one:

- `Save to API` for enqueue-only.
- `Publish + Reload` to enqueue and request immediate server reload.

If you used `Save to API`, run `Reload Server Map` after publish.

### 4. Confirm reload status

Use `Poll reload status` in `map_viewer` until status is `applied`.

Equivalent API contract:

- `POST /admin/world/map/reload`
- `GET /admin/world/map/reload/status?request_id=...`

### 5. Verify outcome

- Spot-check changed coordinates in `map_viewer`.
- Optionally compare map version before/after via `GET /admin/world/map/version`.
- Verify in-game rendering with a test character.

### 6. Rollback criteria

Rollback if any of these are true:

- Reload status does not reach `applied` within expected time.
- Functional map flags are incorrect (movement/sight blockers, nofight, deathtrap, etc.).
- Visual correctness cannot be confirmed.

Rollback procedure:

1. Stop server traffic (maintenance mode or server stop).
2. Import rollback snapshot with force.
3. Restart server and re-verify.

```bash
cargo run -p server --bin world-snapshot -- import --input <rollback.wsnap> --force
```

## Path B: Offline Snapshot Edit + Maintenance Import

Use this for bulk updates, wide-area edits, or releases that require deterministic cutover.

### 1. Export baseline snapshot

```bash
./scripts/save_world_snapshot.sh --prefix map_release_baseline
```

### 2. Edit snapshot offline

```bash
cargo run --package server-utils --bin map_viewer -- --snapshot <baseline.wsnap>
```

Save as a new artifact (for example `map_release_candidate.wsnap`).

### 3. Verify snapshot integrity

```bash
cargo run -p server --bin world-snapshot -- verify --input <map_release_candidate.wsnap>
```

### 4. Maintenance cutover

1. Stop server.
2. Import candidate snapshot.
3. Start server.
4. Run smoke checks.

```bash
cargo run -p server --bin world-snapshot -- import --input <map_release_candidate.wsnap> --force
```

### 5. Post-cutover checks

- Login and inspect changed map areas.
- Validate movement/pathing and blocker flags.
- Confirm no unexpected map corruption warnings at startup.

## Operational Notes

- Live map patches write to `game:map:patch_queue` and are applied on the server tick thread.
- Reload requests use `game:map:patch_request` and completion status uses `game:map:patch_status:{request_id}`.
- Live patch apply preserves dynamic tile fields (`ch`, `to_ch`, `it`, `light`, `dlight`).
- Full world import/export remains the `world-snapshot` workflow.

### Item Safety Notes

- Item placement from `map_viewer` is queued as a server world action, so the server allocates a fresh runtime item slot instead of reusing or overwriting an existing one.
- The map viewer shows the placed item immediately, but `Save to API` only submits the queued action; the server still owns the authoritative item slot assignment.
- Clearing an item from a tile queues the inverse server action, which removes the map reference and returns the runtime item slot to `unused`.

## Decision Guide

- Choose live apply when patch size is small and immediate release is needed.
- Choose maintenance import when patch scope is broad or when you need a single atomic cutover.
