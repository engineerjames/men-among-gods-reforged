# Server Utilities

Utility executables for the Men Among Gods Reforged server.

## Available Utilities

### Template Viewer

A graphical tool built with egui for viewing and editing item templates, character templates, item instances, and character instances.

The viewer now supports two world-data sources:

- Live KeyDB at `127.0.0.1:5556`: read-only view of the latest persisted server state.
- `.wsnap` snapshot files: editable offline backups with `Save Snapshot As...`.

Live KeyDB mode intentionally does not write back while the server is running. The game server owns the authoritative in-memory state and periodically persists to KeyDB, so external live edits would be overwritten.

**Usage:**
```bash
# Default: connect to live KeyDB in read-only mode
cargo run --package server-utils --bin template_viewer

# Open an editable world snapshot backup
cargo run --package server-utils --bin template_viewer -- --snapshot server/assets/world_seed.wsnap
```

**Features:**
- Open and browse live persisted data from KeyDB in read-only mode
- Open and edit `.wsnap` backup files
- Save edited backups with `Save Snapshot As...`
- View detailed information about each template including:
  - Basic properties (name, description, sprite, etc.)
  - Attributes and stats
  - Skills
  - Inventory and equipment (for characters)
  - Driver data
- Filter templates by name

**How to Use:**
1. Start the local Docker stack if you want live data: `docker compose up -d --build`
2. Run the application on the host
3. Use `File --> Data Source` to switch between live KeyDB and snapshot mode
4. In snapshot mode, edit data and use `File --> Save Snapshot As...`

**Running from the project root:**
```bash
# Build and run
cargo run --package server-utils --bin template_viewer

# Or build first, then run
cargo build --package server-utils
./target/debug/template_viewer
```

### Map Viewer

An egui tool for viewing and editing the world map using the client sprite archive.

Like the template viewer, it supports two world-data sources:

- Live KeyDB at `127.0.0.1:5556`: read-only view of the latest persisted world state.
- `.wsnap` snapshot files: editable offline backups with `Save Snapshot As...`.

**Usage:**
```bash
# Default: connect to live KeyDB in read-only mode
cargo run --package server-utils --bin map_viewer

# Open an editable world snapshot backup
cargo run --package server-utils --bin map_viewer -- --snapshot server/assets/world_seed.wsnap
```

**Optional args:**
- `--snapshot <path>` (open an editable `.wsnap` backup)
- `--graphics-zip <path>` (path to `images.zip`)

**Controls:**
- `W/A/S/D`: pan
- Drag with mouse: pan
- Left click: inspect tiles
- In snapshot mode, use the palette and tile controls to edit the map

### MAG Admin CLI

A scriptable admin API client for operator workflows. The first supported data
area is badwords CRUD; the tool talks to the API and never connects directly to
KeyDB.

**Usage:**
```bash
export MAG_API_BASE_URL=https://127.0.0.1:5554
export MAG_ADMIN_API_TOKEN=<32+ byte admin token>

# List badwords as JSON
cargo run --package server-utils --bin mag-admin -- badwords list --format json

# Add or remove entries; --wait implies a running-server refresh request
cargo run --package server-utils --bin mag-admin -- badwords add word1 word2 --wait
cargo run --package server-utils --bin mag-admin -- badwords remove word1 --refresh

# Replace from JSON, a {"words":[...]} object, or newline-delimited stdin
printf 'word1\nword2\n' | cargo run --package server-utils --bin mag-admin -- badwords replace --input -

# Export one word per line
cargo run --package server-utils --bin mag-admin -- badwords export --output badwords.txt --format plain

# Explicitly refresh the running server's cached badwords list
cargo run --package server-utils --bin mag-admin -- badwords refresh --wait

# Open the guided interactive menu for the same badwords actions
cargo run --package server-utils --bin mag-admin -- --menu
```

**Scriptability:** data is written to stdout, diagnostics are written to
stderr, stdin/stdout paths can be `-`, and exit codes are stable: `0` success,
`1` runtime/API failure, `2` command-line usage failure, and `3` for `badwords
get` when the word is not present.

`--menu` is intentionally opt-in and interactive. It reuses the same API calls
as the scriptable subcommands, with guided prompts for listing, checking,
adding, removing, replacing, exporting, and refreshing badwords.

## Local Development Workflow

Run the services in Docker, but run the viewers natively on the host:

```bash
docker compose up -d --build
cargo run --package server-utils --bin map_viewer
cargo run --package server-utils --bin template_viewer
cargo run --package server-utils --bin mag-admin -- badwords list
```

The viewers use the same KeyDB URL resolution as the server crate:

- `MAG_KEYDB_URL` if set
- otherwise `KEYDB_PASSWORD` from the repo `.env`
- otherwise unauthenticated `redis://127.0.0.1:5556/`

That means a normal repo-local `.env` file is enough for host-native viewer runs.

## Adding New Utilities

To add a new utility binary:

1. Create a new file in `src/bin/` (e.g., `src/bin/my_utility.rs`)
2. Add a `[[bin]]` section in `Cargo.toml`:
```toml
[[bin]]
name = "my_utility"
path = "src/bin/my_utility.rs"
```
3. Update this README with usage instructions
