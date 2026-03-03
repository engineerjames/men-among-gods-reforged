# Copilot Instructions for men-among-gods-reforged

## Big picture architecture
- This is a Rust workspace with five crates: `core`, `server`, `client`, `api`, `server/utils`, plus `client/utils`.
- `core` is the shared protocol/types layer used by both `server` and `client`.
- `server` is the real-time game loop (target 36 TPS) and loads world data into memory. Persistence is configurable: legacy `.dat` files or KeyDB (set `MAG_STORAGE_BACKEND=keydb`).
- `api` is a separate Axum auth/account service backed by KeyDB/Redis.
- `client` is an SDL2 app; account/character flows go through `api`, gameplay TCP goes to `server`.
- Read first for boundaries and flow: `docs/server/DESIGN.md`, `api/README.md`, `server/src/server.rs`, `client/src/account_api.rs`.

## Service boundaries and data flow
- API (`:5554`) stores account/character metadata in KeyDB via keys like `account:*`, `character:*`.
- Server (`:5555`) loads gameplay state into memory from either `.dat` files (legacy) or KeyDB (`MAG_STORAGE_BACKEND=keydb`). When using KeyDB, a background saver thread writes data back on a rotating ~60s schedule for crash resilience.
- Integration bridge is login tickets: API writes one-time `game_login_ticket:{ticket}` keys; server consumes them atomically (`server/src/keydb.rs`).
- Character ownership/management is enforced in API routes; game world state persistence is via `server/src/keydb_store.rs` (KeyDB mode) or `server/src/repository.rs` (`.dat` mode).

## Required runtime assumptions
- Server storage backend is selected via `MAG_STORAGE_BACKEND` env var: `dat` (default) uses `.dat` files, `keydb` uses KeyDB.
- `.dat` mode: server expects `.dat` directory next to executable (`<exe_parent>/.dat/*`), see `Repository::get_dat_file_path` in `server/src/repository.rs`.
- KeyDB mode: game data must be seeded first via `cargo run -p server --bin dat-to-keydb`. The background saver thread writes data back every ~10s per data type.
- In local debug runs (`.dat` mode), copy assets with the workspace task `copy .dat to target` after build.
- Key env vars:
  - `MAG_KEYDB_URL` (used by both API and server)
  - `MAG_STORAGE_BACKEND` (`dat` or `keydb`, server only)
  - `API_JWT_SECRET` (API refuses to start without it)
  - `API_BIND_ADDR`, `API_PORT`, `MAG_API_BASE_URL`, `MAG_ASSETS_DIR`, `MAG_LOG_DIR`

## Developer workflows (repo-specific)
- Build all crates: `cargo build`
- Run quality gate matching CI: `cargo test && cargo clippy -- -D warnings`
- Run server: `cargo run -p server`
- Run API: `cargo run -p api`
- Run client binary: `cargo run -p client`
- Start full local stack with auth + game + keydb: `docker compose up -d --build`
- API integration tests are Python stdlib scripts (not pytest): `python3 api/tests/api_integration.py --base-url http://127.0.0.1:5554`

## Code patterns to preserve
- Server global mutable state is accessed through closure helpers (`Repository::with_*`, `Server::with_players_*`) guarded by `OnceLock + ReentrantMutex + UnsafeCell`; follow this pattern instead of introducing ad-hoc globals.
- Networking split is intentional: `csend` for immediate control packets vs `xsend` for batched tick payloads (`compress_ticks` flow in `server/src/server.rs` and `docs/server/DESIGN.md`).
- API request limits are strict (global 1 req/sec governor). Client-side code already handles 429 retries with ~1.1s backoff (`client/src/account_api.rs`); preserve this behavior.
- Account passwords sent to API are Argon2 PHC strings produced client-side (deterministic salt from username), not plaintext (`client/src/account_api.rs`).

## Code quality standards

### Documentation strings
Every public function, struct, enum, trait, and constant **must** have a `///` doc comment. Non-trivial private functions should also be documented. Follow this format:

```rust
/// One-line summary of what this item does.
///
/// Optional extended description providing context, invariants, or
/// implementation notes.
///
/// # Arguments
///
/// * `param_name` - What this parameter represents.
/// * `other_param` - What this parameter represents.
///
/// # Returns
///
/// * Description of the return value or `Result` semantics.
///
/// # Panics (if applicable)
///
/// * Conditions under which this function panics.
```

- Include `# Arguments` whenever the function takes parameters.
- Include `# Returns` whenever the function returns a value (other than `()`).
- Include `# Panics` when the function can panic (e.g. `.unwrap()`, `.expect()`).
- Use backtick-wrapped parameter names in the arguments list: `` * `param` - ... ``.
- Reference from `client/src/gfx_cache.rs` for a working example.

### Unit tests
- Every module that contains testable logic **must** include a `#[cfg(test)] mod tests` block.
- Test encode/decode roundtrips, boundary conditions, default values, and error paths where possible.
- Tests that require external services (KeyDB, network) should be skipped in the default `cargo test` run using `#[ignore]` or feature gates.
- Prefer small, focused tests with descriptive names (e.g. `encode_decode_roundtrip_item`, `double_shutdown_does_not_panic`).

## High-value files by area
- Tick loop and network batching: `server/src/server.rs`, `docs/server/DESIGN.md`
- Persistence and dirty-flag lifecycle: `server/src/repository.rs`, `server/src/main.rs`
- KeyDB game-data persistence: `server/src/keydb_store.rs`, `server/src/background_saver.rs`
- Auth/account routes and key schema assumptions: `api/src/routes.rs`, `api/src/pipelines.rs`, `api/README.md`
- Client state machine and scene management: `client/src/main.rs`, `client/src/network/mod.rs`
- Release packaging/CI: `.github/workflows/rust.yml`, `.github/workflows/release.yml`, `pipelines/README.md`
- Migration tool: `server/src/bin/dat_to_keydb.rs`
