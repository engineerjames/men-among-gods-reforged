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

For production rollout and rollback procedure, see
`docs/server/MAP_EDITOR_OPERATIONS.md`.

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

An admin API client for operator workflows. By default it opens a guided
interactive menu organized into world effects, badword management, template
management, globals, and quit. Scriptable command mode is explicit: pass
`--auto` before the subcommand. The tool talks to the API and never connects
directly to KeyDB.

**Usage:**
```bash
# Both values are read from the project .env file automatically; exporting
# them in the shell is optional and takes precedence over .env.
export MAG_API_BASE_URL=https://127.0.0.1:5554
export MAG_ADMIN_API_TOKEN=<32+ byte admin token>

# Open the guided interactive menu
cargo run --package server-utils --bin mag-admin

# Search templates and inspect full details
cargo run --package server-utils --bin mag-admin -- --auto templates items search sword
cargo run --package server-utils --bin mag-admin -- --auto templates characters search guard --limit 10
cargo run --package server-utils --bin mag-admin -- --auto templates items show 42
cargo run --package server-utils --bin mag-admin -- --auto templates characters show 100

# View persisted global server counters
cargo run --package server-utils --bin mag-admin -- --auto globals show --format json

# Execute live world actions through the admin API
cargo run --package server-utils --bin mag-admin -- --auto world action populate --wait
cargo run --package server-utils --bin mag-admin -- --auto world action rebuild-lights --wait --format json
cargo run --package server-utils --bin mag-admin -- --auto world action sync-skills --wait
cargo run --package server-utils --bin mag-admin -- --auto world action reset-char 123 --wait
cargo run --package server-utils --bin mag-admin -- --auto world action reset-item 456 --wait
cargo run --package server-utils --bin mag-admin -- --auto world action wipe --wait
cargo run --package server-utils --bin mag-admin -- --auto world action reset-all --wait

# List badwords as JSON
cargo run --package server-utils --bin mag-admin -- --auto badwords list --format json

# Add or remove entries; --wait implies a running-server refresh request
cargo run --package server-utils --bin mag-admin -- --auto badwords add word1 word2 --wait
cargo run --package server-utils --bin mag-admin -- --auto badwords remove word1 --refresh

# Replace from JSON, a {"words":[...]} object, or newline-delimited stdin
printf 'word1\nword2\n' | cargo run --package server-utils --bin mag-admin -- --auto badwords replace --input -

# Export one word per line
cargo run --package server-utils --bin mag-admin -- --auto badwords export --output badwords.txt --format plain

# Explicitly refresh the running server's cached badwords list
cargo run --package server-utils --bin mag-admin -- --auto badwords refresh --wait
```

**Scriptability:** data is written to stdout, diagnostics are written to
stderr, stdin/stdout paths can be `-`, and exit codes are stable: `0` success,
`1` runtime/API failure, `2` command-line usage failure, and `3` for `badwords
get` when the word is not present.

Subcommands require `--auto` so automatable interactions are opt-in. Running
without `--auto` opens the interactive menu, which reuses the same API calls as
the scriptable commands. World actions execute on the running server and are
pollable with `--wait`; destructive menu actions prompt for confirmation.

### Template Search

A CLI for finding item and character template IDs in a `.wsnap` snapshot
(default `server/assets/world_seed.wsnap`), so IDs referenced from code can be
verified against real world data. It can also bulk-edit templates in the
snapshot file; edits are a dry run unless `--write` is given.

**Usage:**
```bash
# Search item templates (name/reference/description substring, flags, IDs)
cargo run --package server-utils --bin template-search -- --name potion
cargo run --package server-utils --bin template-search -- --flag IF_MAGIC --flag IF_WEAPON
cargo run --package server-utils --bin template-search -- --ids 101,102,127

# Search character templates
cargo run --package server-utils --bin template-search -- --chars --name ratling

# Edit matched character templates (dry run, then save)
cargo run --package server-utils --bin template-search -- --chars --ids 364-374 --clear-worn RHAND
cargo run --package server-utils --bin template-search -- --chars --ids 364-374 --clear-worn RHAND --write

# Run a batch of operations from YAML, saving once at the end
cargo run --package server-utils --bin template-search -- --ops ops.yaml --write
```

Each `--ops` entry is a map of flags without the leading `--`; lists repeat a
flag and `true` sets a switch:

```yaml
- ids: 501
  set-wv: 2
- chars: true
  ids: 364-374,1094-1096
  set-armor-bonus: 0
```

Run with `--help` for the full list of filters (`--desc`, `--not-flag`,
`--driver`, `--placement`, `--area`, `--quest-items`, ...) and edits
(`--set-armor-bonus`, `--set-weapon-bonus`, `--set-rank`, `--cap-attrib`,
`--cap-skill`, `--restore-worn`, ...). Use `--output <path>` to write edits to
a new file instead of overwriting `--snapshot`.

### Template Sync

Pushes item/character templates from a `.wsnap` file into live KeyDB. It
prints which templates (and fields) differ, writes only the changed slots,
bumps the template version counters, and asks the running server to
hot-reload them. Only `game:titem:*` / `game:tchar:*` keys are touched.

**Usage:**
```bash
# Preview differences against KeyDB without writing
cargo run --package server-utils --bin template-sync -- --dry-run

# Sync both kinds from the default server/assets/world_seed.wsnap
cargo run --package server-utils --bin template-sync

# Sync only character templates from a custom file
cargo run --package server-utils --bin template-sync -- --file my_templates.wsnap --chars
```

**Optional args:**
- `--file <path>` (source snapshot, default `server/assets/world_seed.wsnap`)
- `--items` / `--chars` (limit to one template kind; default is both)
- `--dry-run` (report differences only)
- `--no-reload` (write to KeyDB without asking the server to reload)

A typical template change is `template-search ... --write` to edit the
snapshot, then `template-sync --dry-run` to review and `template-sync` to apply.

## Local Development Workflow

Run the services in Docker, but run the viewers natively on the host:

```bash
docker compose up -d --build
cargo run --package server-utils --bin map_viewer
cargo run --package server-utils --bin template_viewer
cargo run --package server-utils --bin mag-admin -- --auto badwords list
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
