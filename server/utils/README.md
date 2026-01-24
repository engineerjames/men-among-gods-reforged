# Server Utilities

Utility executables for the Men Among Gods Reforged server.

## Available Utilities

### Template Viewer

A graphical tool built with egui for viewing and inspecting character and item templates loaded from the game's `.dat` files.

**Usage:**
```bash
cargo run --package server-utils --bin template_viewer
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
1. Run the application
2. Click **File** → **Select Data Directory...**
3. Choose the folder containing the game data files (typically `server/assets/.dat/`)
4. Browse item/character templates using the tabs in the top bar

**Running from the project root:**
```bash
# Build and run
cargo run --package server-utils --bin template_viewer

# Or build first, then run
cargo build --package server-utils
./target/debug/template_viewer
```

### Map Viewer

An egui tool for viewing the world map loaded from `map.dat` using the client sprite archive.

**Usage:**
```bash
cargo run --package server-utils --bin map_viewer
```

**Optional args:**
- `--dat-dir <path>` (directory containing `map.dat`)
- `--graphics-zip <path>` (path to `images.zip`)

**Controls:**
- `W/A/S/D`: pan
- Drag with mouse: pan

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
