#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source "$ROOT_DIR/Tools/versions.env"

failures=0

check_version() {
    local name="$1"
    local expected="$2"
    local command="$3"

    if ! command -v "$name" >/dev/null 2>&1; then
        echo "Missing $name. Expected version: $expected"
        failures=$((failures + 1))
        return
    fi

    local actual
    actual="$(eval "$command")"
    if [[ "$actual" != "$expected" ]]; then
        echo "$name version mismatch. Expected $expected, got $actual"
        failures=$((failures + 1))
    fi
}

check_version "xcodegen" "$XCODEGEN_VERSION" "xcodegen --version | awk '{print \$NF}'"
check_version "swiftlint" "$SWIFTLINT_VERSION" "swiftlint version"
check_version "swiftformat" "$SWIFTFORMAT_VERSION" "swiftformat --version"

if [[ "$failures" -ne 0 ]]; then
    echo
    echo "Install or update tools, then rerun this command."
    echo "Homebrew example: brew install xcodegen swiftlint swiftformat"
    exit 1
fi

echo "All required tools match Tools/versions.env"
