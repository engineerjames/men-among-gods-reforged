#!/usr/bin/env bash
# initialize_world.sh — spawn missing NPCs, rebuild lighting, and write a clean
# world_seed.wsnap from the running server's world.
#
# Usage:
#   ./scripts/initialize_world.sh [--output FILE] [--reset-char ID]...
#                                 [--timeout SECS] [--spawn-wait SECS]
#
# Steps:
#   1. mag-admin world populate (or reset-char for each --reset-char ID)
#   2. wait for the 10s respawn timer so spawned NPCs exist in memory
#   3. mag-admin world rebuild-lights (also persists the spawned NPCs to KeyDB)
#   4. world-snapshot export -> clear-players -> verify
#   5. move the verified snapshot to --output
#
# Requirements:
#   The game server, API, and KeyDB must be running, with the new templates
#   already saved (template viewer in LiveApi mode). MAG_ADMIN_API_TOKEN,
#   MAG_API_BASE_URL, and MAG_KEYDB_URL are read from the environment or .env.
#
# Examples:
#   ./scripts/initialize_world.sh
#   ./scripts/initialize_world.sh --reset-char 1172
#   ./scripts/initialize_world.sh --output /tmp/test_seed.wsnap

set -euo pipefail

OUTPUT="server/assets/world_seed.wsnap"
TIMEOUT=180
SPAWN_WAIT=15
RESET_IDS=()
RESET_ALL=false

usage() {
    echo "Usage: $0 [--output FILE] [--reset-char ID]... [--timeout SECS] [--spawn-wait SECS] [--reset-all]" >&2
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --output)
            OUTPUT="${2:?Missing value for --output}"
            shift 2
            ;;
        --reset-char)
            RESET_IDS+=("${2:?Missing value for --reset-char}")
            shift 2
            ;;
        --reset-all)
            RESET_ALL=true
            shift 1
            ;;
        --timeout)
            TIMEOUT="${2:?Missing value for --timeout}"
            shift 2
            ;;
        --spawn-wait)
            SPAWN_WAIT="${2:?Missing value for --spawn-wait}"
            shift 2
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            echo "Unknown option: $1" >&2
            usage
            exit 1
            ;;
    esac
done

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
cd "${REPO_ROOT}"

WORK_DIR="$(mktemp -d)"
trap 'rm -rf "${WORK_DIR}"' EXIT

mag_admin() {
    cargo run -q -p server-utils --bin mag-admin -- --auto "$@"
}

world_snapshot() {
    cargo run -q -p server --bin world-snapshot -- "$@"
}

if [[ ${#RESET_IDS[@]} -gt 0 ]]; then
    for id in "${RESET_IDS[@]}"; do
        echo "==> Resetting character template ${id}"
        mag_admin world reset-char "${id}" --wait --timeout-seconds "${TIMEOUT}"
    done
elif [[ "${RESET_ALL}" == true ]]; then
    echo "==> Resetting all character templates"
    mag_admin world reset-all --wait --timeout-seconds "${TIMEOUT}"
else
    echo "==> Spawning missing character templates"
    mag_admin world populate --wait --timeout-seconds "${TIMEOUT}"
fi

# Respawns are scheduled on a 10s timer; the rebuild below persists the result.
echo "==> Waiting ${SPAWN_WAIT}s for spawned NPCs to appear"
sleep "${SPAWN_WAIT}"

echo "==> Rebuilding map lighting"
mag_admin world rebuild-lights --wait --timeout-seconds "${TIMEOUT}"

echo "==> Exporting world from KeyDB"
world_snapshot export --output "${WORK_DIR}/full.wsnap"

echo "==> Removing player characters"
world_snapshot clear-players --input "${WORK_DIR}/full.wsnap" --output "${WORK_DIR}/seed.wsnap"

echo "==> Verifying snapshot"
world_snapshot verify --input "${WORK_DIR}/seed.wsnap"

mkdir -p "$(dirname "${OUTPUT}")"
mv "${WORK_DIR}/seed.wsnap" "${OUTPUT}"

echo "==> World seed written to ${OUTPUT}"
