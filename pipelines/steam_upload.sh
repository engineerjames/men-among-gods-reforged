#!/usr/bin/env bash
# Uploads unpacked client packages to Steam via SteamPipe (steamcmd).
set -euo pipefail

usage() {
  cat <<'EOF'
Usage: steam_upload.sh --app-id <id> --description <text> --depot <depot_id>=<content_dir> [...]
                       [--branch <beta_branch>] [--username <steam_user>] [--steamcmd <path>] [--dry-run]

  --depot     Repeatable. Maps a Steam depot ID to an unpacked client package directory.
  --branch    Beta branch to set the build live on. Steam does not allow setting the
              'default' branch live from a build script; do that in Steamworks.
  --username  Steam builder account (or STEAM_USERNAME). Credentials must already be
              cached in steamcmd's config/config.vdf.
  --dry-run   Print the generated app and depot build VDFs and exit without uploading.
EOF
}

APP_ID=""
DESCRIPTION=""
BRANCH=""
USERNAME="${STEAM_USERNAME:-}"
STEAMCMD="steamcmd"
DRY_RUN=0
DEPOTS=()

while [[ $# -gt 0 ]]; do
  case "$1" in
    --app-id) APP_ID="${2:-}"; shift 2 ;;
    --description) DESCRIPTION="${2:-}"; shift 2 ;;
    --branch) BRANCH="${2:-}"; shift 2 ;;
    --username) USERNAME="${2:-}"; shift 2 ;;
    --steamcmd) STEAMCMD="${2:-}"; shift 2 ;;
    --depot) DEPOTS+=("${2:-}"); shift 2 ;;
    --dry-run) DRY_RUN=1; shift ;;
    -h|--help) usage; exit 0 ;;
    *) echo "Unknown argument: $1" >&2; usage; exit 2 ;;
  esac
done

if [[ ! "$APP_ID" =~ ^[0-9]+$ || -z "$DESCRIPTION" || ${#DEPOTS[@]} -eq 0 ]]; then
  echo "--app-id (numeric), --description and at least one --depot are required." >&2
  usage
  exit 2
fi

if [[ "$BRANCH" == "default" ]]; then
  echo "Steam does not allow setting the 'default' branch live from a build script." >&2
  exit 2
fi

# VDF strings are double-quoted; drop characters that would break them.
DESCRIPTION="${DESCRIPTION//[\"\\]/}"

BUILD_DIR="$(mktemp -d)"
trap 'rm -rf "${BUILD_DIR}"' EXIT
VDF="${BUILD_DIR}/app_build_${APP_ID}.vdf"

# SteamPipe requires LocalPath to be relative to a ContentRoot, so each depot
# gets its own depot build file rooted at its content directory.
DEPOT_ENTRIES=""
for mapping in "${DEPOTS[@]}"; do
  depot_id="${mapping%%=*}"
  content_dir="${mapping#*=}"
  if [[ ! "$depot_id" =~ ^[0-9]+$ || "$mapping" != *=* ]]; then
    echo "Invalid --depot '${mapping}' (expected <depot_id>=<content_dir>)" >&2
    exit 2
  fi
  if [[ ! -d "$content_dir" ]] || [[ -z "$(find "$content_dir" -type f -print -quit)" ]]; then
    echo "Missing or empty content directory for depot ${depot_id}: ${content_dir}" >&2
    exit 1
  fi
  content_dir="$(cd "$content_dir" && pwd)"
  cat > "${BUILD_DIR}/depot_build_${depot_id}.vdf" <<EOF
"DepotBuild"
{
	"DepotID" "${depot_id}"
	"ContentRoot" "${content_dir}"
	"FileMapping"
	{
		"LocalPath" "*"
		"DepotPath" "."
		"Recursive" "1"
	}
	"FileExclusion" "*.pdb"
}
EOF
  DEPOT_ENTRIES+=$'\t\t'"\"${depot_id}\" \"${BUILD_DIR}/depot_build_${depot_id}.vdf\""$'\n'
done

cat > "$VDF" <<EOF
"AppBuild"
{
	"AppID" "${APP_ID}"
	"Desc" "${DESCRIPTION}"
	"BuildOutput" "${BUILD_DIR}/output"
	"SetLive" "${BRANCH}"
	"Depots"
	{
${DEPOT_ENTRIES}	}
}
EOF

if [[ "$DRY_RUN" -eq 1 ]]; then
  cat "$VDF" "${BUILD_DIR}"/depot_build_*.vdf
  exit 0
fi

if [[ -z "$USERNAME" ]]; then
  echo "Missing Steam username (--username or STEAM_USERNAME)." >&2
  exit 2
fi

LOG="${BUILD_DIR}/steamcmd.log"
set +e
"$STEAMCMD" +login "$USERNAME" +run_app_build "$VDF" +quit 2>&1 | tee "$LOG"
STATUS=${PIPESTATUS[0]}
set -e

# Older steamcmd builds can exit 0 after a failed build; also check the log.
if [[ "$STATUS" -ne 0 ]] || grep -q 'ERROR!' "$LOG"; then
  echo "steamcmd upload failed (exit ${STATUS})." >&2
  exit 1
fi
