# Server Utilities

Utility executables for the Men Among Gods Reforged server.

## Available Utilities

### Template Viewer

A graphical tool built with egui for viewing and editing character and item templates.

By default it connects to **KeyDB** at `127.0.0.1:5556` and loads all game data (templates, instances, map). Pass `--dat [path]` to fall back to `.dat` files instead. The data source can also be switched at runtime from `File → Data Source`.

When character templates are saved back to KeyDB, `points_tot` is automatically recalculated for every template.

**Usage:**
```bash
# Default: connect to KeyDB
cargo run --package server-utils --bin template_viewer

# Use .dat files from the default assets directory
cargo run --package server-utils --bin template_viewer -- --dat

# Use .dat files from a specific directory
cargo run --package server-utils --bin template_viewer -- --dat /path/to/.dat
```

**Features:**
- Open and browse item templates (titem.dat) by selecting a `.dat` directory
- Open and browse character templates (tchar.dat) by selecting a `.dat` directory
- View detailed information about each template including:
  - Basic properties (name, description, sprite, etc.)
  - Attributes and stats
  - Skills
  - Inventory and equipment (for characters)
  - Driver data
- Filter templates by name
- Native folder picker for selecting the `.dat` directory

**How to Use:**
1. Run the application (connects to KeyDB by default, or pass `--dat` for file mode)
2. Browse item/character templates using the tabs in the top bar
3. To switch data source at runtime: **File** → **Data Source**

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

By default it connects to **KeyDB** at `127.0.0.1:5556` and loads map tiles, item instances, and item templates. Pass `--dat [path]` to fall back to `.dat` files instead. The data source can also be switched at runtime from `File → Data Source`.

**Usage:**
```bash
# Default: connect to KeyDB
cargo run --package server-utils --bin map_viewer

# Use .dat files from the default assets directory
cargo run --package server-utils --bin map_viewer -- --dat

# Use .dat files from a specific directory
cargo run --package server-utils --bin map_viewer -- --dat /path/to/.dat
```

**Optional args:**
- `--dat [path]` (use `.dat` files; optionally specify directory containing `map.dat`)
- `--dat-dir <path>` (alias for `--dat`, directory containing `map.dat`)
- `--graphics-zip <path>` (path to `images.zip`)

**Controls:**
- `W/A/S/D`: pan
- Drag with mouse: pan

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
