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
mkdir -p "dist/${SERVER_DIR}/.dat"

cp -R server/assets/.dat/. "dist/${SERVER_DIR}/.dat/"
cp "target/release/server" "dist/${SERVER_DIR}/server"

if [[ "$PLATFORM" == "macos" ]]; then
  # If you double-click a raw executable on macOS, Finder launches it via Terminal.
  # Packaging as an .app bundle avoids the Terminal window.
  APP_NAME="Men Among Gods - Reforged"
  APP_DIR="dist/${CLIENT_DIR}/${APP_NAME}.app"
  MACOS_DIR="${APP_DIR}/Contents/MacOS"

  mkdir -p "${MACOS_DIR}/assets"

  # The client expects assets next to the executable (current_exe()/assets).
  cp -R client/assets/. "${MACOS_DIR}/assets/"

  cp "target/release/client" "${MACOS_DIR}/client"
  chmod +x "${MACOS_DIR}/client"

  cat > "${APP_DIR}/Contents/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key>
  <string>${APP_NAME}</string>
  <key>CFBundleDisplayName</key>
  <string>${APP_NAME}</string>
  <key>CFBundleIdentifier</key>
  <string>com.menamonggods.reforged.client</string>
  <key>CFBundleExecutable</key>
  <string>client</string>
  <key>CFBundlePackageType</key>
  <string>APPL</string>
  <key>CFBundleShortVersionString</key>
  <string>${VERSION}</string>
  <key>CFBundleVersion</key>
  <string>${VERSION}</string>
  <key>NSHighResolutionCapable</key>
  <true/>
</dict>
</plist>
EOF
else
  mkdir -p "dist/${CLIENT_DIR}/assets"
  cp -R client/assets/. "dist/${CLIENT_DIR}/assets/"
  cp "target/release/client" "dist/${CLIENT_DIR}/client"
fi

(cd dist && zip -r "${SERVER_DIR}.zip" "${SERVER_DIR}")
(cd dist && zip -r "${CLIENT_DIR}.zip" "${CLIENT_DIR}")
