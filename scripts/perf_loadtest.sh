#!/usr/bin/env bash
# perf_loadtest.sh — orchestrate an instrumented server performance run.
#
# What it does, end to end:
#   1. Brings up the supporting stack (KeyDB + account API) via docker compose.
#   2. Seeds the world snapshot into KeyDB if it is not seeded yet.
#   3. Builds the game server with the `measure-time` feature on the
#      `profiling` cargo profile (release-like codegen + full debug info).
#   4. Starts that server natively on the host so profilers can attach.
#   5. Runs `mag-loadtest` with the requested client count / duration.
#   6. Samples the server process (CPU/RSS) and captures a macOS `sample`
#      call-tree profile during the steady-state window.
#   7. Shuts the server down cleanly and runs `analyze_perf_log.py` over the
#      collected `server_perf.log` to produce a bottleneck report.
#
# Everything for a single run lands in `perf-runs/<timestamp>/`.
#
# Usage:
#   ./scripts/perf_loadtest.sh [options]
#
# Common examples:
#   ./scripts/perf_loadtest.sh
#   ./scripts/perf_loadtest.sh --clients 400 --duration 600
#   ./scripts/perf_loadtest.sh --clients 50 --duration 120 --skip-build
#   ./scripts/perf_loadtest.sh --analyze-only perf-runs/2026-09-28_10-00-00
#
# Requirements:
#   - A populated `.env` at the repo root (KEYDB_PASSWORD, API_JWT_SECRET,
#     MAG_GOD_PASSWORD at minimum).
#   - docker + docker compose, cargo, python3.
#   - macOS `sample` is optional; the run continues without it.

set -euo pipefail

# ---------------------------------------------------------------------------
# Defaults
# ---------------------------------------------------------------------------

CLIENTS=300
DURATION=420
RAMP_UP=30
LOGIN_STAGGER=0.1
SETTLE_SECS=30
SAMPLE_SECS=60
# The API's public limiter allows 30 req/s per source IP and every bot shares
# one IP, so keep request *starts* well under that. Concurrency only exists to
# hide the API's server-side Argon2 latency, not to exceed the rate budget.
API_RPS=12
API_CONCURRENCY=8
CARGO_PROFILE="profiling"
OUT_ROOT="perf-runs"
LOADTEST_CONFIG="loadtest/loadtest.toml"
SKIP_BUILD=0
KEEP_STACK=1
SKIP_STACK=0
RUN_SAMPLE=1
ANALYZE_ONLY=""

usage() {
    sed -n '2,32p' "$0" | sed 's/^# \{0,1\}//'
    cat <<'EOF'

Options:
  --clients N            Simulated bot clients                (default 300)
  --duration SECS        Load-test wall-clock duration        (default 420)
  --ramp-up SECS         Loadtest connection ramp-up window   (default 30)
  --login-stagger SECS   Seconds between successive logins    (default 0.1)
  --settle SECS          Extra settle time before the steady
                         window opens                         (default 30)
  --sample-secs SECS     Length of the `sample` capture       (default 60)
  --api-rps N            Account-API request starts per second (default 12)
  --api-concurrency N    Concurrent account-API requests during
                         bot bootstrap                        (default 8)
  --no-sample            Skip the macOS `sample` capture
  --profile NAME         Cargo profile for the server build   (default profiling)
  --config PATH          Loadtest TOML config                 (default loadtest/loadtest.toml)
  --out-dir DIR          Root directory for run artifacts     (default perf-runs)
  --skip-build           Reuse an already-built server/loadtest binary
  --skip-stack           Assume KeyDB + API are already up
  --down-stack           Run `docker compose down` when finished
  --analyze-only DIR     Skip the run; just re-analyze an existing run dir
  -h, --help             Show this help
EOF
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --clients)        CLIENTS="${2:?}";        shift 2 ;;
        --duration)       DURATION="${2:?}";       shift 2 ;;
        --ramp-up)        RAMP_UP="${2:?}";        shift 2 ;;
        --login-stagger)  LOGIN_STAGGER="${2:?}";  shift 2 ;;
        --settle)         SETTLE_SECS="${2:?}";    shift 2 ;;
        --sample-secs)    SAMPLE_SECS="${2:?}";    shift 2 ;;
        --api-rps)        API_RPS="${2:?}";        shift 2 ;;
        --api-concurrency) API_CONCURRENCY="${2:?}"; shift 2 ;;
        --no-sample)      RUN_SAMPLE=0;            shift   ;;
        --profile)        CARGO_PROFILE="${2:?}";  shift 2 ;;
        --config)         LOADTEST_CONFIG="${2:?}"; shift 2 ;;
        --out-dir)        OUT_ROOT="${2:?}";       shift 2 ;;
        --skip-build)     SKIP_BUILD=1;            shift   ;;
        --skip-stack)     SKIP_STACK=1;            shift   ;;
        --down-stack)     KEEP_STACK=0;            shift   ;;
        --analyze-only)   ANALYZE_ONLY="${2:?}";   shift 2 ;;
        -h|--help)        usage; exit 0 ;;
        *) echo "Unknown option: $1" >&2; usage >&2; exit 1 ;;
    esac
done

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
cd "${REPO_ROOT}"

ANALYZER="${SCRIPT_DIR}/analyze_perf_log.py"

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
[[ -f "${LOADTEST_CONFIG}" ]] || die "loadtest config not found: ${LOADTEST_CONFIG}"

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

# Each bot holds a persistent socket; raise the soft fd limit when we can.
if [[ "$(ulimit -Sn)" -lt 4096 ]]; then
    ulimit -Sn 4096 2>/dev/null || log "WARNING: could not raise open-file limit above $(ulimit -Sn)"
fi

RUN_STAMP="$(date +%Y-%m-%d_%H-%M-%S)"
RUN_DIR="${OUT_ROOT%/}/${RUN_STAMP}"
SERVER_LOG_DIR="${RUN_DIR}/server-logs"
mkdir -p "${SERVER_LOG_DIR}"
RUN_DIR_ABS="$(cd "${RUN_DIR}" && pwd)"

log "Run directory: ${RUN_DIR}"

# ---------------------------------------------------------------------------
# Cleanup wiring
# ---------------------------------------------------------------------------

SERVER_PID=""
RESOURCE_SAMPLER_PID=""
SAMPLE_PID=""

cleanup() {
    local exit_code=$?
    set +e
    if [[ -n "${RESOURCE_SAMPLER_PID}" ]] && kill -0 "${RESOURCE_SAMPLER_PID}" 2>/dev/null; then
        kill "${RESOURCE_SAMPLER_PID}" 2>/dev/null
    fi
    if [[ -n "${SAMPLE_PID}" ]] && kill -0 "${SAMPLE_PID}" 2>/dev/null; then
        kill "${SAMPLE_PID}" 2>/dev/null
    fi
    if [[ -n "${SERVER_PID}" ]] && kill -0 "${SERVER_PID}" 2>/dev/null; then
        log "Stopping game server (pid ${SERVER_PID})..."
        kill -INT "${SERVER_PID}" 2>/dev/null
        for _ in $(seq 1 120); do
            kill -0 "${SERVER_PID}" 2>/dev/null || break
            sleep 1
        done
        if kill -0 "${SERVER_PID}" 2>/dev/null; then
            log "Server did not exit gracefully; sending SIGKILL."
            kill -9 "${SERVER_PID}" 2>/dev/null
        fi
    fi
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
    # Only keydb/api/certgen are started: the game server runs natively so we
    # can profile it, and seeding is done with a host-built world-snapshot.
    docker compose up -d --wait keydb api > "${RUN_DIR}/compose.log" 2>&1 \
        || { tail -30 "${RUN_DIR}/compose.log" >&2; die "docker compose failed to start keydb/api"; }

    # Every bot shares one source IP, so a leftover per-IP request bucket from
    # an earlier aborted run would 429 the whole bootstrap. These keys are
    # ephemeral counters; dropping them is always safe.
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

if [[ ! -f certs/server.crt || ! -f certs/server.key ]]; then
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
LOADTEST_BIN="target/release/mag-loadtest"

if [[ "${SKIP_BUILD}" -eq 0 ]]; then
    log "Building server (profile=${CARGO_PROFILE}, features=measure-time)..."
    cargo build --profile "${CARGO_PROFILE}" -p server --bins --features measure-time

    log "Building loadtest (profile=release)..."
    cargo build --release -p mag-loadtest
fi

[[ -x "${SERVER_BIN}" ]]   || die "server binary missing: ${SERVER_BIN} (drop --skip-build)"
[[ -x "${SNAPSHOT_BIN}" ]] || die "world-snapshot binary missing: ${SNAPSHOT_BIN}"
[[ -x "${LOADTEST_BIN}" ]] || die "loadtest binary missing: ${LOADTEST_BIN} (drop --skip-build)"

# Fail loudly instead of silently producing a perf log with zero measure rows.
# (`grep -q` would SIGPIPE `strings` and trip `pipefail`, so count instead.)
MEASURE_MARKERS="$(strings -a "${SERVER_BIN}" 2>/dev/null | grep -c 'measure-time' || true)"
if [[ "${MEASURE_MARKERS}" -eq 0 ]]; then
    die "${SERVER_BIN} was not built with --features measure-time"
fi

# ---------------------------------------------------------------------------
# 4. Seed the world
# ---------------------------------------------------------------------------

log "Seeding world snapshot into KeyDB (skipped if already seeded)..."
"${SNAPSHOT_BIN}" import --skip-if-seeded --input server/assets/world_seed.wsnap \
    > "${RUN_DIR}/world-seed.log" 2>&1 \
    || { tail -20 "${RUN_DIR}/world-seed.log" >&2; die "world seeding failed"; }

# ---------------------------------------------------------------------------
# 5. Start the instrumented server
# ---------------------------------------------------------------------------

log "Starting instrumented game server..."
MAG_LOG_DIR="${RUN_DIR_ABS}/server-logs" \
    "${SERVER_BIN}" > "${RUN_DIR}/server.stdout.log" 2>&1 &
SERVER_PID=$!
log "Server pid: ${SERVER_PID}"

wait_for_port() {
    local host="$1" port="$2" tries="$3"
    for _ in $(seq 1 "${tries}"); do
        if (exec 3<>"/dev/tcp/${host}/${port}") 2>/dev/null; then
            exec 3<&- 3>&-
            return 0
        fi
        kill -0 "${SERVER_PID}" 2>/dev/null || return 1
        sleep 1
    done
    return 1
}

if ! wait_for_port 127.0.0.1 5555 180; then
    tail -40 "${RUN_DIR}/server.stdout.log" >&2
    die "game server did not start listening on 127.0.0.1:5555"
fi
log "Game server is listening on 127.0.0.1:5555"

# ---------------------------------------------------------------------------
# 6. Background samplers
# ---------------------------------------------------------------------------

RESOURCE_CSV="${RUN_DIR}/server_resources.csv"
echo "epoch_utc,cpu_pct,rss_kb" > "${RESOURCE_CSV}"
(
    while kill -0 "${SERVER_PID}" 2>/dev/null; do
        line="$(ps -o %cpu=,rss= -p "${SERVER_PID}" 2>/dev/null | tr -s ' ' | sed 's/^ //;s/ /,/')"
        [[ -n "${line}" ]] && echo "$(date -u +%s),${line}" >> "${RESOURCE_CSV}"
        sleep 2
    done
) &
RESOURCE_SAMPLER_PID=$!

# The steady-state window opens once every bot has had time to log in. Logins
# are serialized by login_stagger, so the real ramp is max(ramp_up, N*stagger).
STEADY_DELAY="$(python3 -c "print(int(max(${RAMP_UP}, ${CLIENTS} * ${LOGIN_STAGGER}) + ${SETTLE_SECS}))")"

if [[ "${RUN_SAMPLE}" -eq 1 ]] && command -v sample >/dev/null; then
    (
        sleep "${STEADY_DELAY}"
        kill -0 "${SERVER_PID}" 2>/dev/null || exit 0
        sample "${SERVER_PID}" "${SAMPLE_SECS}" -f "${RUN_DIR_ABS}/sample.txt" >/dev/null 2>&1
    ) &
    SAMPLE_PID=$!
    log "Call-tree sample scheduled at T+${STEADY_DELAY}s for ${SAMPLE_SECS}s"
fi

# ---------------------------------------------------------------------------
# 7. Run the load test
# ---------------------------------------------------------------------------

LOADTEST_START_UTC="$(date -u +%s)"
log "Running load test: ${CLIENTS} clients, ${DURATION}s, ramp ${RAMP_UP}s, stagger ${LOGIN_STAGGER}s"

set +e
RUST_LOG="${RUST_LOG:-info}" "${LOADTEST_BIN}" \
    --config "${LOADTEST_CONFIG}" \
    --clients "${CLIENTS}" \
    --duration "${DURATION}" \
    --ramp-up "${RAMP_UP}" \
    --login-stagger "${LOGIN_STAGGER}" \
    --server-host 127.0.0.1 \
    --server-port 5555 \
    --api-url "https://127.0.0.1:5554" \
    --api-rps "${API_RPS}" \
    --api-concurrency "${API_CONCURRENCY}" \
    2>&1 | tee "${RUN_DIR}/loadtest.log"
LOADTEST_RC="${PIPESTATUS[0]}"
set -e
LOADTEST_END_UTC="$(date -u +%s)"

log "Load test finished (exit ${LOADTEST_RC})"

# ---------------------------------------------------------------------------
# 8. Stop the server so it flushes its perf log
# ---------------------------------------------------------------------------

if [[ -n "${RESOURCE_SAMPLER_PID}" ]]; then
    kill "${RESOURCE_SAMPLER_PID}" 2>/dev/null || true
    RESOURCE_SAMPLER_PID=""
fi

log "Stopping game server..."
kill -INT "${SERVER_PID}" 2>/dev/null || true
for _ in $(seq 1 180); do
    kill -0 "${SERVER_PID}" 2>/dev/null || break
    sleep 1
done
if kill -0 "${SERVER_PID}" 2>/dev/null; then
    log "Server still running after 180s; forcing termination."
    kill -9 "${SERVER_PID}" 2>/dev/null || true
fi
SERVER_PID=""

# ---------------------------------------------------------------------------
# 9. Metadata + analysis
# ---------------------------------------------------------------------------

# The steady-state window should begin only once every bot is actually in the
# world. Prefer the load test's own periodic report over a guess: find the
# first `[ Xs] clients=N/N` line that reaches the requested client count.
RAMP_COMPLETE_SECS="$(
    awk -v target="${CLIENTS}" '
        match($0, /^\[ *([0-9.]+)s\] clients=([0-9]+)\/([0-9]+)/, m) {
            if (m[3] + 0 >= target) { print int(m[1]); exit }
        }
    ' "${RUN_DIR}/loadtest.log" 2>/dev/null
)"
if [[ -z "${RAMP_COMPLETE_SECS}" ]]; then
    # gawk-style `match(..., arr)` is unavailable (BSD awk); fall back to sed.
    RAMP_COMPLETE_SECS="$(
        sed -n "s/^\[ *\([0-9]*\)\.[0-9]*s\] clients=[0-9]*\/${CLIENTS} .*/\1/p" \
            "${RUN_DIR}/loadtest.log" | head -1
    )"
fi

if [[ -n "${RAMP_COMPLETE_SECS}" ]]; then
    log "All ${CLIENTS} clients were connected by T+${RAMP_COMPLETE_SECS}s"
    STEADY_START_UTC=$((LOADTEST_START_UTC + RAMP_COMPLETE_SECS + SETTLE_SECS))
else
    log "WARNING: never observed ${CLIENTS}/${CLIENTS} connected clients; \
using the estimated ramp of ${STEADY_DELAY}s"
    STEADY_START_UTC=$((LOADTEST_START_UTC + STEADY_DELAY))
fi

if [[ "${STEADY_START_UTC}" -ge "${LOADTEST_END_UTC}" ]]; then
    # Run was too short for the observed ramp; fall back to the final third.
    STEADY_START_UTC=$(( LOADTEST_END_UTC - (LOADTEST_END_UTC - LOADTEST_START_UTC) / 3 ))
fi

cat > "${RUN_DIR}/run_meta.json" <<EOF
{
  "run_stamp": "${RUN_STAMP}",
  "clients": ${CLIENTS},
  "duration_secs": ${DURATION},
  "ramp_up_secs": ${RAMP_UP},
  "login_stagger_secs": ${LOGIN_STAGGER},
  "settle_secs": ${SETTLE_SECS},
  "api_rps": ${API_RPS},
  "api_concurrency": ${API_CONCURRENCY},
  "cargo_profile": "${CARGO_PROFILE}",
  "loadtest_config": "${LOADTEST_CONFIG}",
  "loadtest_exit_code": ${LOADTEST_RC},
  "loadtest_start_utc": ${LOADTEST_START_UTC},
  "loadtest_end_utc": ${LOADTEST_END_UTC},
  "steady_start_utc": ${STEADY_START_UTC},
  "steady_end_utc": ${LOADTEST_END_UTC}
}
EOF

log "Analyzing collected perf data..."
python3 "${ANALYZER}" --run-dir "${RUN_DIR}"

log "Done. Artifacts in ${RUN_DIR}"
