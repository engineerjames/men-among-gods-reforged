#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
Usage: create_github_release.sh --version <tag> --artifacts-dir <dir>

Requires:
- gh CLI authenticated via GH_TOKEN
- GH_REPO set (owner/repo)
EOF
}

VERSION=""
ARTIFACTS_DIR=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --version)
      VERSION="${2:-}"; shift 2 ;;
    --artifacts-dir)
      ARTIFACTS_DIR="${2:-}"; shift 2 ;;
    -h|--help)
      usage; exit 0 ;;
    *)
      echo "Unknown argument: $1" >&2
      usage
      exit 2
      ;;
  esac
done

if [[ -z "$VERSION" || -z "$ARTIFACTS_DIR" ]]; then
  echo "Missing required args." >&2
  usage
  exit 2
fi

if [[ -z "${GH_REPO:-}" ]]; then
  echo "GH_REPO is required (e.g. owner/repo)." >&2
  exit 2
fi

mapfile -t FILES < <(find "$ARTIFACTS_DIR" -type f -name '*.zip' -print)
if [[ ${#FILES[@]} -eq 0 ]]; then
  echo "No .zip artifacts found to upload" >&2
  exit 1
fi

if gh release view "$VERSION" --repo "$GH_REPO" >/dev/null 2>&1; then
  gh release upload "$VERSION" "${FILES[@]}" --repo "$GH_REPO" --clobber
else
  gh release create "$VERSION" "${FILES[@]}" --repo "$GH_REPO" \
    --title "$VERSION" \
    --target "${GITHUB_SHA}" \
    --notes "Automated release for $VERSION"
fi
