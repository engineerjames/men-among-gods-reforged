#!/usr/bin/env bash
# save_world_snapshot.sh — export a timestamped world snapshot from KeyDB.
#
# Usage:
#   ./scripts/save_world_snapshot.sh [--out-dir DIR] [--prefix NAME] [--utc]
#
# Examples:
#   ./scripts/save_world_snapshot.sh
#   ./scripts/save_world_snapshot.sh --out-dir server/backups/manual
#   ./scripts/save_world_snapshot.sh --prefix live --utc
#
# Notes:
#   This exports the latest persisted KeyDB state. While the game server is
#   running, that can lag behind the in-memory world state.

set -euo pipefail

OUT_DIR="server/backups"
PREFIX="world"
USE_UTC=0

while [[ $# -gt 0 ]]; do
    case "$1" in
        --out-dir)
            OUT_DIR="${2:?Missing value for --out-dir}"
            shift 2
            ;;
        --prefix)
            PREFIX="${2:?Missing value for --prefix}"
            shift 2
            ;;
        --utc)
            USE_UTC=1
            shift
            ;;
        *)
            echo "Unknown option: $1" >&2
            echo "Usage: $0 [--out-dir DIR] [--prefix NAME] [--utc]" >&2
            exit 1
            ;;
    esac
done

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

cd "${REPO_ROOT}"
mkdir -p "${OUT_DIR}"

if [[ "${USE_UTC}" -eq 1 ]]; then
    TIMESTAMP="$(date -u +%Y-%m-%dT%H-%M-%SZ)"
else
    TIMESTAMP="$(date +%Y-%m-%d_%H-%M-%S)"
fi

OUTPUT_PATH="${OUT_DIR%/}/${PREFIX}_${TIMESTAMP}.wsnap"

echo "==> Exporting world snapshot to ${OUTPUT_PATH}"

cargo run -p server --bin world-snapshot -- export --output "${OUTPUT_PATH}"

echo "==> Snapshot written to ${OUTPUT_PATH}"
echo "==> Note: this captures the latest persisted KeyDB state, not a true in-memory live snapshot."