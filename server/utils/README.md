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
- Browse all item templates (titem.dat)
- Browse all character templates (tchar.dat)
- View detailed information about each template including:
  - Basic properties (name, description, sprite, etc.)
  - Attributes and stats
  - Skills
  - Inventory and equipment (for characters)
  - Driver data
- Filter templates by name
- Configurable data directory path

**Default Data Directory:**
The viewer looks for `.dat` files in `./assets/.dat` by default. You can change this path in the UI.

**Running from the project root:**
```bash
# Build and run
cargo run --package server-utils --bin template_viewer

# Or build first, then run
cargo build --package server-utils
./target/debug/template_viewer
```

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
