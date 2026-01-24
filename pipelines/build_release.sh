#!/usr/bin/env bash
set -euo pipefail

# Build release binaries for server + client.

cargo build --release -p server -p client
