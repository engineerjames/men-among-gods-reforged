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

# Or use the helper launcher from the repo root
./scripts/run_devtool.sh template_viewer
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

# Or use the helper launcher from the repo root
./scripts/run_devtool.sh map_viewer
```

**Optional args:**
- `--snapshot <path>` (open an editable `.wsnap` backup)
- `--graphics-zip <path>` (path to `images.zip`)

**Controls:**
- `W/A/S/D`: pan
- Drag with mouse: pan
- Left click: inspect tiles
- In snapshot mode, use the palette and tile controls to edit the map

## Local Development Workflow

Run the services in Docker, but run the viewers natively on the host:

```bash
docker compose up -d --build
./scripts/run_devtool.sh map_viewer
./scripts/run_devtool.sh template_viewer
```

The viewers use the same KeyDB URL resolution as the server crate:

- `MAG_KEYDB_URL` if set
- otherwise `KEYDB_PASSWORD` from the repo `.env`
- otherwise unauthenticated `redis://127.0.0.1:5556/`

That means a normal repo-local `.env` file is enough for host-native viewer runs.

### DAT Normalizer

A CLI migration tool that converts legacy packed `.dat` files into a normalized, non-packed Rust representation and writes them using `bincode`.

**Usage:**
```bash
cargo run --package server-utils --bin dat_normalizer -- --dat-dir server/assets/.dat
```

**Options:**
- `--dat-dir <path>`: directory containing server `.dat` files
- `--in-place`: replace each `.dat` file with normalized data and create a `.legacy` backup
- `--reverse`: convert normalized output back into legacy packed `.dat` format

**Output behavior:**
- Default mode writes side-by-side files with `.normalized` suffix (e.g. `map.dat.normalized`)
- `--in-place` mode writes normalized data into the original `.dat` paths after creating backups

**Reverse mode behavior:**
- Reads normalized payloads and writes legacy packed bytes back to `.dat`
- Side-by-side reverse mode reads `*.dat.normalized` and writes `*.dat.restored`
- `--reverse --in-place` restores directly to `*.dat` and creates `*.normalized.bak` backups

**Files converted:**
- `map.dat`
- `item.dat`
- `titem.dat`
- `char.dat`
- `tchar.dat`
- `effect.dat`
- `global.dat`

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
