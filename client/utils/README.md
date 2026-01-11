# client-utils

Small utilities for the `client` codebase. Each utility is a separate binary in `src/bin`.

Currently included:

- `transparency_convert` — Convert images with a specific color key (magenta, `#ff00ff` for example) to use transparency instead, saving as PNG. Used to convert the original game assets to have transparency.

- `idx_convert` — Convert the original IDX data files to a more manageable JSON format for easier loading and editing.