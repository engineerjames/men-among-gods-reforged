#!/usr/bin/env bash
# perf_client.sh — orchestrate an instrumented, scripted client run.
#
# The client-side counterpart of perf_loadtest.sh. End to end it:
#   1. Brings up the supporting stack (KeyDB + account API) via docker compose
#      and seeds the world snapshot if needed.
#   2. Builds and starts the game server natively (plain build, it is not the
#      thing being measured) unless --skip-server says one is already running.
#   3. Builds the client on the `profiling` cargo profile with the
#      `measure-time` feature so the main loop writes per-frame timing rows.
#   4. Runs the client against a UI automation script
#      (scripts/automation/*.uiscript) that registers an account, logs in,
#      creates a character, enters the world, starts the in-game render
#      profiler and quits. The client is fully driven; no human input needed.
#   5. Samples the client process (CPU/RSS) and, on macOS, captures a `sample`
#      call tree while the render profiler is active.
#   6. Runs analyze_client_perf.py over the collected logs.
#
# Everything for a single run lands in `perf-runs/client-<timestamp>/`, and
# the client uses an isolated data directory there (MAG_CLIENT_DATA_DIR), so
# your own ~/.men-among-gods profile and known-hosts are never touched.
#
# Usage:
#   ./scripts/perf_client.sh [options]
#
# Common examples:
#   ./scripts/perf_client.sh
#   ./scripts/perf_client.sh --settle 60 --profile-secs 60
#   ./scripts/perf_client.sh --existing-account --username perfbob --password hunter22
#   ./scripts/perf_client.sh --script my_session.uiscript --skip-build
#   ./scripts/perf_client.sh --analyze-only perf-runs/client-2026-09-29_10-00-00
#
# Requirements:
#   - A populated `.env` at the repo root (KEYDB_PASSWORD, API_JWT_SECRET,
#     MAG_GOD_PASSWORD at minimum).
#   - docker + docker compose, cargo, python3, a display (the client opens
#     an SDL window).
#   - macOS `sample` is optional; the run continues without it.

set -euo pipefail

# ---------------------------------------------------------------------------
# Defaults
# ---------------------------------------------------------------------------

SCRIPT_PATH=""
EXISTING_ACCOUNT=0
USERNAME=""
PASSWORD=""
EMAIL=""
CHARACTER=""
SETTLE_SECS=20
PROFILE_SECS=30
SAMPLE_SECS=""
MAX_RUN_SECS=900
CARGO_PROFILE="profiling"
OUT_ROOT="perf-runs"
SKIP_BUILD=0
SKIP_STACK=0
SKIP_SERVER=0
KEEP_STACK=1
RUN_SAMPLE=1
ANALYZE_ONLY=""

usage() {
    sed -n '2,38p' "$0" | sed 's/^# \{0,1\}//'
    cat <<'EOF'

Options:
  --script PATH          UI automation script to run
                         (default scripts/automation/create_account_and_play.uiscript,
                          or login_and_play.uiscript with --existing-account)
  --existing-account     Skip registration; log in with --username/--password
  --username NAME        Account username        (default perf-<timestamp>)
  --password PASS        Account password        (default random)
  --email ADDR           Account e-mail          (default <username>@perf.local)
  --character NAME       Character name, 4-15 ASCII letters (default random)
  --settle SECS          Idle time in-world before the profiler starts (default 20)
  --profile-secs SECS    Render-profiler capture window            (default 30)
  --sample-secs SECS     Length of the macOS `sample` capture      (default = --profile-secs)
  --no-sample            Skip the macOS `sample` capture
  --max-run SECS         Kill the client if it outlives this      (default 900)
  --profile NAME         Cargo profile for the builds             (default profiling)
  --out-dir DIR          Root directory for run artifacts         (default perf-runs)
  --skip-build           Reuse already-built server/client binaries
  --skip-stack           Assume KeyDB + API are already up
  --skip-server          Assume a game server is already listening on 127.0.0.1:5555
  --down-stack           Run `docker compose down` when finished
  --analyze-only DIR     Skip the run; just re-analyze an existing run dir
  -h, --help             Show this help
EOF
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --script)           SCRIPT_PATH="${2:?}";   shift 2 ;;
        --existing-account) EXISTING_ACCOUNT=1;     shift   ;;
        --username)         USERNAME="${2:?}";      shift 2 ;;
        --password)         PASSWORD="${2:?}";      shift 2 ;;
        --email)            EMAIL="${2:?}";         shift 2 ;;
        --character)        CHARACTER="${2:?}";     shift 2 ;;
        --settle)           SETTLE_SECS="${2:?}";   shift 2 ;;
        --profile-secs)     PROFILE_SECS="${2:?}";  shift 2 ;;
        --sample-secs)      SAMPLE_SECS="${2:?}";   shift 2 ;;
        --no-sample)        RUN_SAMPLE=0;           shift   ;;
        --max-run)          MAX_RUN_SECS="${2:?}";  shift 2 ;;
        --profile)          CARGO_PROFILE="${2:?}"; shift 2 ;;
        --out-dir)          OUT_ROOT="${2:?}";      shift 2 ;;
        --skip-build)       SKIP_BUILD=1;           shift   ;;
        --skip-stack)       SKIP_STACK=1;           shift   ;;
        --skip-server)      SKIP_SERVER=1;          shift   ;;
        --down-stack)       KEEP_STACK=0;           shift   ;;
        --analyze-only)     ANALYZE_ONLY="${2:?}";  shift 2 ;;
        -h|--help)          usage; exit 0 ;;
        *) echo "Unknown option: $1" >&2; usage >&2; exit 1 ;;
    esac
done

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
cd "${REPO_ROOT}"

ANALYZER="${SCRIPT_DIR}/analyze_client_perf.py"

# ---------------------------------------------------------------------------
# Analyze-only shortcut
# ---------------------------------------------------------------------------

if [[ -n "${ANALYZE_ONLY}" ]]; then
    exec python3 "${ANALYZER}" --run-dir "${ANALYZE_ONLY}"
fi

# ---------------------------------------------------------------------------
# Preflight
# ---------------------------------------------------------------------------

log() { printf '==> %s\n' "$*"; }
die() { printf 'ERROR: %s\n' "$*" >&2; exit 1; }

command -v cargo   >/dev/null || die "cargo not found on PATH"
command -v python3 >/dev/null || die "python3 not found on PATH"

if [[ -z "${SCRIPT_PATH}" ]]; then
    if [[ "${EXISTING_ACCOUNT}" -eq 1 ]]; then
        SCRIPT_PATH="${SCRIPT_DIR}/automation/login_and_play.uiscript"
    else
        SCRIPT_PATH="${SCRIPT_DIR}/automation/create_account_and_play.uiscript"
    fi
fi
[[ -f "${SCRIPT_PATH}" ]] || die "automation script not found: ${SCRIPT_PATH}"
SCRIPT_PATH="$(cd "$(dirname "${SCRIPT_PATH}")" && pwd)/$(basename "${SCRIPT_PATH}")"

if [[ "${EXISTING_ACCOUNT}" -eq 1 && ( -z "${USERNAME}" || -z "${PASSWORD}" ) ]]; then
    die "--existing-account requires --username and --password"
fi

if [[ ! -f .env ]]; then
    die ".env not found at repo root. Copy .env.example to .env and fill it in."
fi

# Export every key in .env so cargo-run children inherit them.
set -a
# shellcheck disable=SC1091
source .env
set +a

: "${KEYDB_PASSWORD:?KEYDB_PASSWORD must be set in .env}"
: "${MAG_GOD_PASSWORD:?MAG_GOD_PASSWORD must be set in .env}"

export MAG_KEYDB_URL="${MAG_KEYDB_URL:-redis://:${KEYDB_PASSWORD}@127.0.0.1:5556/}"

RUN_STAMP="$(date +%Y-%m-%d_%H-%M-%S)"
RUN_DIR="${OUT_ROOT%/}/client-${RUN_STAMP}"
SERVER_LOG_DIR="${RUN_DIR}/server-logs"
CLIENT_DATA_DIR="${RUN_DIR}/client-data"
mkdir -p "${SERVER_LOG_DIR}" "${CLIENT_DATA_DIR}" "${RUN_DIR}/screens"
RUN_DIR_ABS="$(cd "${RUN_DIR}" && pwd)"

log "Run directory: ${RUN_DIR}"

# Random characters from an alphabet. Reads a fixed chunk of urandom rather
# than streaming it so `tr` never sees SIGPIPE under `pipefail`.
random_chars() {
    local alphabet="$1" count="$2" pool
    pool="$(head -c 2048 /dev/urandom | LC_ALL=C tr -dc "${alphabet}" || true)"
    printf '%s' "${pool:0:${count}}"
}

# Consonant-only letters satisfy the API's ASCII-letters rule for character
# names and cannot spell any of the banned English substrings.
[[ -n "${USERNAME}" ]]  || USERNAME="perf$(date +%y%m%d%H%M%S)"
[[ -n "${PASSWORD}" ]]  || PASSWORD="Pw$(random_chars 'A-Za-z0-9' 16)"
[[ -n "${EMAIL}" ]]     || EMAIL="${USERNAME}@perf.local"
[[ -n "${CHARACTER}" ]] || CHARACTER="Perf$(random_chars 'bcdfghjklmnpqrstvwxz' 6)"
[[ -n "${SAMPLE_SECS}" ]] || SAMPLE_SECS="${PROFILE_SECS}"

# ---------------------------------------------------------------------------
# Cleanup wiring
# ---------------------------------------------------------------------------

SERVER_PID=""
CLIENT_PID=""
RESOURCE_SAMPLER_PID=""
SAMPLE_WATCHER_PID=""

stop_process() {
    local pid="$1" name="$2" grace="$3"
    [[ -n "${pid}" ]] && kill -0 "${pid}" 2>/dev/null || return 0
    log "Stopping ${name} (pid ${pid})..."
    kill -INT "${pid}" 2>/dev/null
    for _ in $(seq 1 "${grace}"); do
        kill -0 "${pid}" 2>/dev/null || return 0
        sleep 1
    done
    log "${name} did not exit gracefully; sending SIGKILL."
    kill -9 "${pid}" 2>/dev/null || true
}

cleanup() {
    local exit_code=$?
    set +e
    for pid in "${RESOURCE_SAMPLER_PID}" "${SAMPLE_WATCHER_PID}"; do
        [[ -n "${pid}" ]] && kill "${pid}" 2>/dev/null
    done
    stop_process "${CLIENT_PID}" "client" 15
    stop_process "${SERVER_PID}" "game server" 120
    if [[ "${KEEP_STACK}" -eq 0 && "${SKIP_STACK}" -eq 0 ]]; then
        log "Tearing down docker compose stack..."
        docker compose down >/dev/null 2>&1
    fi
    exit "${exit_code}"
}
trap cleanup EXIT INT TERM

# ---------------------------------------------------------------------------
# 1. Supporting stack (KeyDB + account API)
# ---------------------------------------------------------------------------

if [[ "${SKIP_STACK}" -eq 0 ]]; then
    command -v docker >/dev/null || die "docker not found on PATH (use --skip-stack to bypass)"

    log "Starting KeyDB + account API (docker compose)..."
    docker compose up -d --wait keydb api > "${RUN_DIR}/compose.log" 2>&1 \
        || { tail -30 "${RUN_DIR}/compose.log" >&2; die "docker compose failed to start keydb/api"; }

    # A leftover per-IP request bucket from an aborted run would 429 the login.
    docker compose exec -T keydb keydb-cli -p 5556 -a "${KEYDB_PASSWORD}" \
        --no-auth-warning --scan --pattern 'rate:public:*' 2>/dev/null \
        | while read -r stale_key; do
            [[ -n "${stale_key}" ]] && docker compose exec -T keydb keydb-cli -p 5556 \
                -a "${KEYDB_PASSWORD}" --no-auth-warning del "${stale_key}" >/dev/null 2>&1
        done
fi

# ---------------------------------------------------------------------------
# 2. Host TLS certificates for the natively-run game server
# ---------------------------------------------------------------------------

if [[ "${SKIP_SERVER}" -eq 0 && ( ! -f certs/server.crt || ! -f certs/server.key ) ]]; then
    log "Generating host TLS certificates..."
    ./scripts/generate_certs.sh --out ./certs > "${RUN_DIR}/certgen.log" 2>&1 \
        || { tail -20 "${RUN_DIR}/certgen.log" >&2; die "certificate generation failed"; }
fi
export SERVER_TLS_CERT="${REPO_ROOT}/certs/server.crt"
export SERVER_TLS_KEY="${REPO_ROOT}/certs/server.key"

# ---------------------------------------------------------------------------
# 3. Build
# ---------------------------------------------------------------------------

SERVER_BIN="target/${CARGO_PROFILE}/server"
SNAPSHOT_BIN="target/${CARGO_PROFILE}/world-snapshot"
CLIENT_BIN="target/${CARGO_PROFILE}/men-among-gods-client"

if [[ "${SKIP_BUILD}" -eq 0 ]]; then
    if [[ "${SKIP_SERVER}" -eq 0 ]]; then
        log "Building server (profile=${CARGO_PROFILE})..."
        cargo build --profile "${CARGO_PROFILE}" -p server --bins
    fi
    log "Building client (profile=${CARGO_PROFILE}, features=measure-time)..."
    cargo build --profile "${CARGO_PROFILE}" -p client --bin men-among-gods-client \
        --features measure-time
fi

if [[ "${SKIP_SERVER}" -eq 0 ]]; then
    [[ -x "${SERVER_BIN}" ]]   || die "server binary missing: ${SERVER_BIN} (drop --skip-build)"
    [[ -x "${SNAPSHOT_BIN}" ]] || die "world-snapshot binary missing: ${SNAPSHOT_BIN}"
fi
[[ -x "${CLIENT_BIN}" ]] || die "client binary missing: ${CLIENT_BIN} (drop --skip-build)"

# Fail loudly instead of silently producing an empty perf log.
MEASURE_MARKERS="$(strings -a "${CLIENT_BIN}" 2>/dev/null | grep -c 'measure-time' || true)"
if [[ "${MEASURE_MARKERS}" -eq 0 ]]; then
    die "${CLIENT_BIN} was not built with --features measure-time"
fi

# ---------------------------------------------------------------------------
# 4. Seed the world and start the server
# ---------------------------------------------------------------------------

wait_for_port() {
    local host="$1" port="$2" tries="$3" guard_pid="${4:-}"
    for _ in $(seq 1 "${tries}"); do
        if (exec 3<>"/dev/tcp/${host}/${port}") 2>/dev/null; then
            exec 3<&- 3>&-
            return 0
        fi
        if [[ -n "${guard_pid}" ]]; then
            kill -0 "${guard_pid}" 2>/dev/null || return 1
        fi
        sleep 1
    done
    return 1
}

if [[ "${SKIP_SERVER}" -eq 0 ]]; then
    log "Seeding world snapshot into KeyDB (skipped if already seeded)..."
    "${SNAPSHOT_BIN}" import --skip-if-seeded --input server/assets/world_seed.wsnap \
        > "${RUN_DIR}/world-seed.log" 2>&1 \
        || { tail -20 "${RUN_DIR}/world-seed.log" >&2; die "world seeding failed"; }

    log "Starting game server..."
    MAG_LOG_DIR="${RUN_DIR_ABS}/server-logs" \
        "${SERVER_BIN}" > "${RUN_DIR}/server.stdout.log" 2>&1 &
    SERVER_PID=$!
    log "Server pid: ${SERVER_PID}"

    if ! wait_for_port 127.0.0.1 5555 180 "${SERVER_PID}"; then
        tail -40 "${RUN_DIR}/server.stdout.log" >&2
        die "game server did not start listening on 127.0.0.1:5555"
    fi
else
    wait_for_port 127.0.0.1 5555 5 || die "no game server listening on 127.0.0.1:5555 (--skip-server)"
fi
log "Game server is listening on 127.0.0.1:5555"

# ---------------------------------------------------------------------------
# 5. Run the scripted client
# ---------------------------------------------------------------------------

CLIENT_LOG="${CLIENT_DATA_DIR}/mag_client.log"
CLIENT_PERF_LOG="${CLIENT_DATA_DIR}/mag_client_perf.log"

log "Automation script: ${SCRIPT_PATH}"
log "Account: ${USERNAME} / character: ${CHARACTER} (existing-account=${EXISTING_ACCOUNT})"
log "Settle ${SETTLE_SECS}s, then profile ${PROFILE_SECS}s"

CLIENT_START_UTC="$(date -u +%s)"

# CARGO_MANIFEST_DIR makes filepaths::get_asset_directory() resolve to the
# in-tree client/assets instead of expecting assets next to the binary.
env \
    MAG_CLIENT_DATA_DIR="${RUN_DIR_ABS}/client-data" \
    MAG_SERVER_IP=127.0.0.1 \
    MAG_API_URL="https://127.0.0.1:5554" \
    CARGO_MANIFEST_DIR="${REPO_ROOT}/client" \
    MAG_AUTOMATION_SCRIPT="${SCRIPT_PATH}" \
    MAG_AUTO_USERNAME="${USERNAME}" \
    MAG_AUTO_PASSWORD="${PASSWORD}" \
    MAG_AUTO_EMAIL="${EMAIL}" \
    MAG_AUTO_CHARACTER="${CHARACTER}" \
    MAG_AUTO_SETTLE_SECS="${SETTLE_SECS}" \
    MAG_AUTO_PROFILE_SECS="${PROFILE_SECS}" \
    MAG_AUTO_RUN_DIR="${RUN_DIR_ABS}" \
    RUST_BACKTRACE=1 \
    "${CLIENT_BIN}" > "${RUN_DIR}/client.stdout.log" 2>&1 &
CLIENT_PID=$!
log "Client pid: ${CLIENT_PID}"

# ---------------------------------------------------------------------------
# 6. Background samplers
# ---------------------------------------------------------------------------

RESOURCE_CSV="${RUN_DIR}/client_resources.csv"
echo "epoch_utc,cpu_pct,rss_kb" > "${RESOURCE_CSV}"
(
    while kill -0 "${CLIENT_PID}" 2>/dev/null; do
        line="$(ps -o %cpu=,rss= -p "${CLIENT_PID}" 2>/dev/null | tr -s ' ' | sed 's/^ //;s/ /,/')"
        [[ -n "${line}" ]] && echo "$(date -u +%s),${line}" >> "${RESOURCE_CSV}"
        sleep 2
    done
) &
RESOURCE_SAMPLER_PID=$!

# The render profiler announces itself in the client log; start `sample` the
# moment that line appears so both captures cover the same window.
if [[ "${RUN_SAMPLE}" -eq 1 ]] && command -v sample >/dev/null; then
    (
        while kill -0 "${CLIENT_PID}" 2>/dev/null; do
            if [[ -f "${CLIENT_LOG}" ]] && grep -q 'Performance profiling started' "${CLIENT_LOG}" 2>/dev/null; then
                sample "${CLIENT_PID}" "${SAMPLE_SECS}" -f "${RUN_DIR_ABS}/sample.txt" >/dev/null 2>&1
                exit 0
            fi
            sleep 1
        done
    ) &
    SAMPLE_WATCHER_PID=$!
    log "Call-tree sample armed (${SAMPLE_SECS}s, starts with the render profiler)"
fi

# ---------------------------------------------------------------------------
# 7. Wait for the script to finish
# ---------------------------------------------------------------------------

CLIENT_RC=""
for _ in $(seq 1 "${MAX_RUN_SECS}"); do
    if ! kill -0 "${CLIENT_PID}" 2>/dev/null; then
        break
    fi
    sleep 1
done
if kill -0 "${CLIENT_PID}" 2>/dev/null; then
    log "WARNING: client still running after ${MAX_RUN_SECS}s; terminating it."
    stop_process "${CLIENT_PID}" "client" 15
    CLIENT_RC=124
fi
set +e
wait "${CLIENT_PID}"
wait_rc=$?
set -e
CLIENT_RC="${CLIENT_RC:-${wait_rc}}"
CLIENT_PID=""
CLIENT_END_UTC="$(date -u +%s)"

if [[ "${CLIENT_RC}" -ne 0 ]]; then
    log "Client exited with status ${CLIENT_RC}. Last log lines:"
    tail -20 "${CLIENT_LOG}" 2>/dev/null || tail -20 "${RUN_DIR}/client.stdout.log"
else
    log "Client finished (exit 0)"
fi

for pid in "${RESOURCE_SAMPLER_PID}" "${SAMPLE_WATCHER_PID}"; do
    [[ -n "${pid}" ]] && kill "${pid}" 2>/dev/null || true
done
RESOURCE_SAMPLER_PID=""
SAMPLE_WATCHER_PID=""

# ---------------------------------------------------------------------------
# 8. Stop the server, write metadata, analyze
# ---------------------------------------------------------------------------

if [[ -n "${SERVER_PID}" ]]; then
    stop_process "${SERVER_PID}" "game server" 180
    SERVER_PID=""
fi

cat > "${RUN_DIR}/run_meta.json" <<EOF
{
  "run_stamp": "${RUN_STAMP}",
  "kind": "client",
  "script": "${SCRIPT_PATH}",
  "existing_account": ${EXISTING_ACCOUNT},
  "username": "${USERNAME}",
  "character": "${CHARACTER}",
  "settle_secs": ${SETTLE_SECS},
  "profile_secs": ${PROFILE_SECS},
  "cargo_profile": "${CARGO_PROFILE}",
  "client_exit_code": ${CLIENT_RC},
  "client_start_utc": ${CLIENT_START_UTC},
  "client_end_utc": ${CLIENT_END_UTC}
}
EOF

log "Analyzing collected perf data..."
python3 "${ANALYZER}" --run-dir "${RUN_DIR}"

log "Done. Artifacts in ${RUN_DIR}"
[[ -f "${CLIENT_PERF_LOG}" ]] || log "NOTE: no ${CLIENT_PERF_LOG}; was the client built with --features measure-time?"
exit "${CLIENT_RC}"
