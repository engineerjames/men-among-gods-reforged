# client-utils

Small utilities for the `client` codebase. Each utility is a separate binary in `src/bin`.

Currently included:

- `transparency_convert` — Convert images with a specific color key (magenta, `#ff00ff` for example) to use transparency instead, saving as PNG. Used to convert the original game assets to have transparency.