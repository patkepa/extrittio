#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

CONFIGURATION="${CONFIGURATION:-Debug}"
SCHEME="${SCHEME:-Extrittio}"
DESTINATION="${DESTINATION:-generic/platform=iOS Simulator}"

"$ROOT_DIR/Scripts/generate-project.sh"
xcodebuild build \
    -scheme "$SCHEME" \
    -configuration "$CONFIGURATION" \
    -destination "$DESTINATION"
