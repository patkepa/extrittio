#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

failures=0

if rg -n "^import (SwiftUI|UIKit|SwiftData|Security|MapKit|os)$" Extrittio/Domain; then
    echo "Domain must not import UI, persistence, security, or logging frameworks."
    failures=$((failures + 1))
fi

if rg -n "APIClient|KeychainHelper|URLSession|UserDefaults" Extrittio/Domain; then
    echo "Domain must not depend on concrete data, storage, or transport implementations."
    failures=$((failures + 1))
fi

if [[ "$failures" -ne 0 ]]; then
    exit 1
fi

echo "Module boundary checks passed"
