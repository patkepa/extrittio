#!/usr/bin/env sh
set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
sdk_dir=$(CDPATH= cd -- "$script_dir/.." && pwd)
repo_root=$(CDPATH= cd -- "$sdk_dir/../../.." && pwd)
generator=${NANOPB_GENERATOR:-nanopb_generator}
proto_dir="$repo_root/crates/common/src/protos"

"$generator" \
  --proto-path="$proto_dir" \
  "$proto_dir/telemetry.proto" \
  --options-file="$sdk_dir/nanopb/telemetry.options" \
  --output-dir="$sdk_dir/generated"
