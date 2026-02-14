# Copilot Instructions for men-among-gods-reforged

## Big picture architecture
- This is a Rust workspace with six crates: `core`, `server`, `client`, `api`, `server/utils`, `client/utils`.
- `core` is the shared protocol/types layer used by both `server` and `client`.
- `server` is the real-time game loop (target 36 TPS) and owns `.dat` world data in memory.
- `api` is a separate Axum auth/account service backed by KeyDB/Redis.
- `client` is a Bevy 0.18 app; account/character flows go through `api`, gameplay TCP goes to `server`.
- Read first for boundaries and flow: `docs/server/DESIGN.md`, `api/README.md`, `server/src/server.rs`, `client/src/network/account_api.rs`.

## Service boundaries and data flow
- API (`:5554`) stores account/character metadata in KeyDB via keys like `account:*`, `character:*`.
- Server (`:5555`) loads gameplay state from `.dat` files and does not treat KeyDB as primary world storage.
- Integration bridge is login tickets: API writes one-time `game_login_ticket:{ticket}` keys; server consumes them atomically (`server/src/keydb.rs`).
- Character ownership/management is enforced in API routes; server-side gameplay state is still `.dat`-driven for now, but the long term plan is to migrate to KeyDB as the primary store.

## Required runtime assumptions
- Server expects `.dat` directory next to executable (`<exe_parent>/.dat/*`), see `Repository::get_dat_file_path` in `server/src/repository.rs`.
- In local debug runs, copy assets with the workspace task `copy .dat to target` after build.
- Key env vars:
  - `MAG_KEYDB_URL` (used by both API and server)
  - `API_JWT_SECRET` (API refuses to start without it)
  - `API_BIND_ADDR`, `API_PORT`, `MAG_API_BASE_URL`, `MAG_ASSETS_DIR`, `MAG_LOG_DIR`

## Developer workflows (repo-specific)
- Build all crates: `cargo build`
- Run quality gate matching CI: `cargo test && cargo clippy -- -D warnings`
- Run server: `cargo run -p server`
- Run API: `cargo run -p api`
- Run client binary: `cargo run -p client --bin men-among-gods-client`
- Start full local stack with auth + game + keydb: `docker compose up -d --build`
- API integration tests are Python stdlib scripts (not pytest): `python3 api/tests/api_integration.py --base-url http://127.0.0.1:5554`

## Code patterns to preserve
- Server global mutable state is accessed through closure helpers (`Repository::with_*`, `Server::with_players_*`) guarded by `OnceLock + ReentrantMutex + UnsafeCell`; follow this pattern instead of introducing ad-hoc globals.
- Networking split is intentional: `csend` for immediate control packets vs `xsend` for batched tick payloads (`compress_ticks` flow in `server/src/server.rs` and `docs/server/DESIGN.md`).
- API request limits are strict (global 1 req/sec governor). Client-side code already handles 429 retries with ~1.1s backoff (`client/src/network/account_api.rs`); preserve this behavior.
- Account passwords sent to API are Argon2 PHC strings produced client-side (deterministic salt from username), not plaintext (`client/src/network/account_api.rs`).

## High-value files by area
- Tick loop and network batching: `server/src/server.rs`, `docs/server/DESIGN.md`
- `.dat` load/save and dirty-flag lifecycle: `server/src/repository.rs`, `server/src/main.rs`
- Auth/account routes and key schema assumptions: `api/src/routes.rs`, `api/src/pipelines.rs`, `api/README.md`
- Client state machine and plugin wiring: `client/src/main.rs`, `client/src/network/mod.rs`
- Release packaging/CI: `.github/workflows/rust.yml`, `.github/workflows/release.yml`, `pipelines/README.md`
