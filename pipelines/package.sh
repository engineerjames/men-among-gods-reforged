#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
Usage: package.sh --version <tag> --platform <linux|macos>
EOF
}

VERSION=""
PLATFORM=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --version)
      VERSION="${2:-}"; shift 2 ;;
    --platform)
      PLATFORM="${2:-}"; shift 2 ;;
    -h|--help)
      usage; exit 0 ;;
    *)
      echo "Unknown argument: $1" >&2
      usage
      exit 2
      ;;
  esac
done

if [[ -z "$VERSION" || -z "$PLATFORM" ]]; then
  echo "Missing required args." >&2
  usage
  exit 2
fi

SERVER_DIR="men-among-gods-server-${VERSION}-${PLATFORM}"
CLIENT_DIR="men-among-gods-client-${VERSION}-${PLATFORM}"

rm -rf dist
mkdir -p "dist/${SERVER_DIR}/.dat" "dist/${CLIENT_DIR}/assets"

cp -R server/assets/.dat/. "dist/${SERVER_DIR}/.dat/"
cp "target/release/server" "dist/${SERVER_DIR}/server"

cp -R client/assets/. "dist/${CLIENT_DIR}/assets/"
cp "target/release/client" "dist/${CLIENT_DIR}/client"

(cd dist && zip -r "${SERVER_DIR}.zip" "${SERVER_DIR}")
(cd dist && zip -r "${CLIENT_DIR}.zip" "${CLIENT_DIR}")
