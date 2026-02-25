#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
Usage: build_steam_deck_client.sh [--profile <release|debug>]

Builds the client for Steam Deck (SteamOS, x86_64 Linux) inside Docker and
packages output into dist/.

Examples:
  bash pipelines/build_steam_deck_client.sh
  bash pipelines/build_steam_deck_client.sh --profile debug
EOF
}

PROFILE="release"
IMAGE="registry.gitlab.steamos.cloud/steamrt/scout/sdk"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --profile)
      PROFILE="${2:-}"
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "Unknown argument: $1" >&2
      usage
      exit 2
      ;;
  esac
done

if [[ "$PROFILE" != "release" && "$PROFILE" != "debug" ]]; then
  echo "Invalid --profile: $PROFILE (expected release or debug)" >&2
  exit 2
fi

if ! command -v docker >/dev/null 2>&1; then
  echo "docker is required but was not found in PATH." >&2
  exit 1
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

mkdir -p "${REPO_ROOT}/dist"

echo "Building Steam Deck client in Docker image ${IMAGE} (profile=${PROFILE})..."

docker run --rm \
  --platform linux/amd64 \
  -e PROFILE="${PROFILE}" \
  -v "${REPO_ROOT}:/workspace" \
  -v "${HOME}/.cargo/registry:/usr/local/cargo/registry" \
  -v "${HOME}/.cargo/git:/usr/local/cargo/git" \
  -w /workspace \
  "${IMAGE}" \
  bash -lc '
    set -euo pipefail

    apt-get update
    DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
      ca-certificates \
      cmake \
      curl \
      git \
      ninja-build \
      pkg-config \
      unzip \
      zip \
      libasound2-dev \
      libudev-dev
    rm -rf /var/lib/apt/lists/*

    if ! command -v rustup >/dev/null 2>&1; then
      set +e
      curl -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
      RUSTUP_BOOTSTRAP_STATUS=$?
      set -e

      if [[ ${RUSTUP_BOOTSTRAP_STATUS} -ne 0 ]]; then
        echo
        echo "Rust bootstrap failed inside ${IMAGE}." >&2
        echo "This Steam Runtime image uses an older userland where current rustup/rustc bootstrap binaries may not run." >&2
        echo "On Apple Silicon + linux/amd64 emulation this can also fail early." >&2
        echo "Recommended next step: run this script on a native x86_64 Linux host, or switch to a newer Steam Runtime SDK image." >&2
        exit 1
      fi
    fi

    export PATH="$HOME/.cargo/bin:$PATH"

    if ! command -v cargo >/dev/null 2>&1; then
      echo "cargo was not found after rustup bootstrap" >&2
      exit 1
    fi

    rustup target add x86_64-unknown-linux-gnu

    if ! command -v cargo-vcpkg >/dev/null 2>&1; then
      cargo install --locked cargo-vcpkg
    fi

    cargo vcpkg build --manifest-path client/Cargo.toml --target x86_64-unknown-linux-gnu

    if [[ "${PROFILE}" == "release" ]]; then
      cargo build -p client --release --target x86_64-unknown-linux-gnu
      BIN_PATH="target/x86_64-unknown-linux-gnu/release/men-among-gods-client"
    else
      cargo build -p client --target x86_64-unknown-linux-gnu
      BIN_PATH="target/x86_64-unknown-linux-gnu/debug/men-among-gods-client"
    fi

    OUT_DIR="dist/men-among-gods-client-steamdeck"
    rm -rf "${OUT_DIR}"
    mkdir -p "${OUT_DIR}/assets"

    cp -R client/assets/. "${OUT_DIR}/assets/"
    cp "${BIN_PATH}" "${OUT_DIR}/men-among-gods-client"

    tar -C dist -czf dist/men-among-gods-client-steamdeck.tar.gz men-among-gods-client-steamdeck
  '

echo
echo "Done. Artifacts:"
echo "  - dist/men-among-gods-client-steamdeck/"
echo "  - dist/men-among-gods-client-steamdeck.tar.gz"
