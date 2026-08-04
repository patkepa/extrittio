#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

CONFIGURATION="${CONFIGURATION:-Debug}"
SCHEME="${SCHEME:-Extrittio}"
SIMULATOR_NAME="${SIMULATOR_NAME:-iPhone 17 Pro}"
SIMULATOR_OS="${SIMULATOR_OS:-latest}"
DESTINATION="${DESTINATION:-platform=iOS Simulator,name=$SIMULATOR_NAME,OS=$SIMULATOR_OS}"

"$ROOT_DIR/Scripts/generate-project.sh"
xcodebuild test \
    -scheme "$SCHEME" \
    -configuration "$CONFIGURATION" \
    -destination "$DESTINATION"
