#!/usr/bin/env bash
# run_devtool.sh — launch a server-utils GUI tool inside the devtools container
# with X11 display forwarding for macOS (XQuartz) and Linux.
#
# Usage:
#   ./scripts/run_devtool.sh <tool> [args...]
#
# Examples:
#   ./scripts/run_devtool.sh map_viewer --snapshot /snapshots/world_seed.wsnap
#   ./scripts/run_devtool.sh template_viewer --snapshot /snapshots/world_seed.wsnap
#   ./scripts/run_devtool.sh map_viewer  # connects to KeyDB

set -euo pipefail

TOOL="${1:?Usage: $0 <tool> [args...]}"
shift

# ---------------------------------------------------------------------------
# Resolve the DISPLAY and any X11 socket mounts needed for the platform.
# ---------------------------------------------------------------------------
EXTRA_DOCKER_ARGS=()

case "$(uname -s)" in
  Darwin)
    # Require XQuartz.  The simplest approach is to forward over TCP to the
    # host's XQuartz server via host.docker.internal.
    if [[ -z "${DISPLAY:-}" ]]; then
      echo "DISPLAY is not set.  Start XQuartz and run:"
      echo "  export DISPLAY=:0"
      echo "  xhost +localhost"
      echo "then re-run this script." >&2
      exit 1
    fi
    # Replace the socket path (e.g. /tmp/...) with the TCP address that the
    # container can reach.
    CONTAINER_DISPLAY="host.docker.internal:0"
    EXTRA_DOCKER_ARGS+=(
      "--add-host=host.docker.internal:host-gateway"
    )
    ;;
  Linux)
    if [[ -z "${DISPLAY:-}" ]]; then
      echo "DISPLAY is not set.  Is an X server running?" >&2
      exit 1
    fi
    CONTAINER_DISPLAY="${DISPLAY}"
    EXTRA_DOCKER_ARGS+=(
      "--volume=/tmp/.X11-unix:/tmp/.X11-unix:rw"
    )
    ;;
  *)
    echo "Unsupported platform: $(uname -s)" >&2
    exit 1
    ;;
esac

# ---------------------------------------------------------------------------
# Ensure the devtools container is running (start it if not).
# ---------------------------------------------------------------------------
COMPOSE_SERVICE="devtools"
CONTAINER_ID=$(docker compose ps -q "${COMPOSE_SERVICE}" 2>/dev/null || true)

if [[ -z "${CONTAINER_ID}" ]]; then
  echo "Starting ${COMPOSE_SERVICE} container..."
  docker compose --profile tools up -d "${COMPOSE_SERVICE}"
  CONTAINER_ID=$(docker compose ps -q "${COMPOSE_SERVICE}")
fi

CONTAINER_STATE=$(docker inspect --format '{{.State.Status}}' "${CONTAINER_ID}" 2>/dev/null || true)
if [[ "${CONTAINER_STATE}" != "running" ]]; then
  echo "Container ${COMPOSE_SERVICE} is not running (state: ${CONTAINER_STATE}). Starting..."
  docker compose --profile tools start "${COMPOSE_SERVICE}"
fi

# ---------------------------------------------------------------------------
# Exec the tool inside the container.
# ---------------------------------------------------------------------------
exec docker exec \
  --interactive \
  --tty \
  --env "DISPLAY=${CONTAINER_DISPLAY}" \
  "${EXTRA_DOCKER_ARGS[@]}" \
  "${CONTAINER_ID}" \
  "/usr/local/bin/${TOOL}" "$@"
