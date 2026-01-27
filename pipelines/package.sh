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
cp "target/release/map_viewer" "dist/${SERVER_DIR}/map_viewer"
cp "target/release/template_viewer" "dist/${SERVER_DIR}/template_viewer"

if [[ "$PLATFORM" == "macos" ]]; then
  # If you double-click a raw executable on macOS, Finder launches it via Terminal.
  # Packaging as an .app bundle avoids the Terminal window.
  APP_NAME="Men Among Gods - Reforged"
  APP_DIR="dist/${CLIENT_DIR}/${APP_NAME}.app"
  MACOS_DIR="${APP_DIR}/Contents/MacOS"
  RESOURCES_DIR="${APP_DIR}/Contents/Resources"
  ICON_BASENAME="AppIcon"

  mkdir -p "${MACOS_DIR}/assets"
  mkdir -p "${RESOURCES_DIR}"

  # The client expects assets next to the executable (current_exe()/assets).
  cp -R client/assets/. "${MACOS_DIR}/assets/"

  cp "target/release/men-among-gods-client" "${MACOS_DIR}/men-among-gods-client"
  chmod +x "${MACOS_DIR}/men-among-gods-client"

  # Bundle a proper macOS app icon.
  ICON_SRC="client/assets/gfx/mag_logo.png"

  if [[ -f "${ICON_SRC}" ]]; then
    if command -v sips >/dev/null 2>&1 && command -v iconutil >/dev/null 2>&1; then
      TMPDIR="$(mktemp -d)"
      trap 'rm -rf "${TMPDIR}"' EXIT

      ICONSET="${TMPDIR}/${ICON_BASENAME}.iconset"
      mkdir -p "${ICONSET}"

      sips -z 16 16       "${ICON_SRC}" --out "${ICONSET}/icon_16x16.png" >/dev/null
      sips -z 32 32       "${ICON_SRC}" --out "${ICONSET}/icon_16x16@2x.png" >/dev/null
      sips -z 32 32       "${ICON_SRC}" --out "${ICONSET}/icon_32x32.png" >/dev/null
      sips -z 64 64       "${ICON_SRC}" --out "${ICONSET}/icon_32x32@2x.png" >/dev/null
      sips -z 128 128     "${ICON_SRC}" --out "${ICONSET}/icon_128x128.png" >/dev/null
      sips -z 256 256     "${ICON_SRC}" --out "${ICONSET}/icon_128x128@2x.png" >/dev/null
      sips -z 256 256     "${ICON_SRC}" --out "${ICONSET}/icon_256x256.png" >/dev/null
      sips -z 512 512     "${ICON_SRC}" --out "${ICONSET}/icon_256x256@2x.png" >/dev/null
      sips -z 512 512     "${ICON_SRC}" --out "${ICONSET}/icon_512x512.png" >/dev/null
      sips -z 1024 1024   "${ICON_SRC}" --out "${ICONSET}/icon_512x512@2x.png" >/dev/null

      iconutil -c icns "${ICONSET}" -o "${TMPDIR}/${ICON_BASENAME}.icns"
      cp "${TMPDIR}/${ICON_BASENAME}.icns" "${RESOURCES_DIR}/${ICON_BASENAME}.icns"
    else
      echo "Warning: sips/iconutil not available; skipping .icns generation" >&2
    fi
  else
    echo "Warning: No icon source found; expected client/assets/gfx/mag_logo.png" >&2
  fi

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
  <string>men-among-gods-client</string>
  <key>CFBundlePackageType</key>
  <string>APPL</string>
  <key>CFBundleShortVersionString</key>
  <string>${VERSION}</string>
  <key>CFBundleVersion</key>
  <string>${VERSION}</string>
  <key>CFBundleIconFile</key>
  <string>${ICON_BASENAME}</string>
  <key>NSHighResolutionCapable</key>
  <true/>
</dict>
</plist>
EOF
else
  mkdir -p "dist/${CLIENT_DIR}/assets"
  cp -R client/assets/. "dist/${CLIENT_DIR}/assets/"
  cp "target/release/men-among-gods-client" "dist/${CLIENT_DIR}/men-among-gods-client"
fi

(cd dist && zip -r "${SERVER_DIR}.zip" "${SERVER_DIR}")
(cd dist && zip -r "${CLIENT_DIR}.zip" "${CLIENT_DIR}")
