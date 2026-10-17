#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

# rustc sees native paths; Git Bash on Windows reports POSIX-style ones.
native_path() {
    if command -v cygpath >/dev/null 2>&1; then cygpath -w "$1"; else printf '%s' "$1"; fi
}

# Strip build-machine paths from panic messages/debug info. Last matching remap wins.
flags=(
    "--remap-path-prefix=$(native_path "$HOME")=~"
    "--remap-path-prefix=$(native_path "${CARGO_HOME:-$HOME/.cargo}")=/cargo"
    "--remap-path-prefix=$(native_path "$PWD")=."
)

# Env rustflags replace .cargo/config.toml rustflags, so re-add its Windows linker flag.
if [[ "$(rustc -vV | sed -n 's/^host: //p')" == "x86_64-pc-windows-msvc" ]]; then
    flags+=("-Clink-arg=/DEBUG:NONE")
fi

CARGO_ENCODED_RUSTFLAGS="$(IFS=$'\x1f'; printf '%s' "${flags[*]}")" cargo build --release
