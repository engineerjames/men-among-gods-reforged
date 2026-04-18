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
    # Enable TCP listening in XQuartz preferences BEFORE starting it so the
    # setting takes effect on this launch.
    defaults write org.xquartz.X11 nolisten_tcp -boolean false 2>/dev/null || true

    # Start XQuartz if it isn't running.
    if ! pgrep -qx Xquartz && ! pgrep -qx "X11.bin"; then
      echo "Starting XQuartz..." >&2
      open -a XQuartz
    fi

    # Wait up to 20s for XQuartz to open TCP port 6000 (display :0).
    echo "Waiting for XQuartz to accept connections..." >&2
    for i in $(seq 1 20); do
      if nc -z 127.0.0.1 6000 2>/dev/null; then
        break
      fi
      if [[ $i -eq 20 ]]; then
        echo "" >&2
        echo "XQuartz is not accepting TCP connections." >&2
        echo "Fix: open XQuartz → Preferences → Security → enable" >&2
        echo "'Allow connections from network clients', quit XQuartz," >&2
        echo "then re-run this script." >&2
        exit 1
      fi
      sleep 1
    done

    # Extract the display number from $DISPLAY.
    # XQuartz may set DISPLAY to a socket path like
    #   /private/tmp/com.apple.launchd.XXX/org.xquartz:0
    # or simply :0.  We only need the ":N" part.
    if [[ -z "${DISPLAY:-}" ]]; then
      DISPLAY=":0"
    fi
    DISPLAY_NUM=":${DISPLAY##*:}"
    DISPLAY_NUM="${DISPLAY_NUM%%.*}"   # strip e.g. ".0" screen suffix
    CONTAINER_DISPLAY="host.docker.internal${DISPLAY_NUM}"

    # Grant the container access to the X server.  Docker Desktop's internal
    # gateway is not 127.0.0.1, so open access control for the local session.
    xhost + &>/dev/null || true
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
  docker compose --profile tools up -d --build "${COMPOSE_SERVICE}"
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
  ${EXTRA_DOCKER_ARGS[@]+"${EXTRA_DOCKER_ARGS[@]}"} \
  "${CONTAINER_ID}" \
  "/usr/local/bin/${TOOL}" "$@"
