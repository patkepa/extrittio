#!/usr/bin/env bash
set -euo pipefail

if [[ "$(uname -m)" != "aarch64" ]]; then
  echo "a 64-bit Raspberry Pi OS installation is required (expected uname -m: aarch64)" >&2
  exit 1
fi

if [[ "${EUID}" -ne 0 ]]; then
  echo "run this script with sudo on the Raspberry Pi" >&2
  exit 1
fi

if ! command -v apt-get >/dev/null 2>&1; then
  echo "this bootstrap currently supports Debian/Raspberry Pi OS hosts" >&2
  exit 1
fi

# The packaged OTBR agent links against D-Bus and Avahi. dbus-daemon is also
# required because Extrittio starts an isolated private D-Bus daemon for OTBR.
# Debian Trixie renamed the package for the 64-bit time_t ABI while preserving
# the libprotobuf-lite.so.32 SONAME used by the Bookworm-built appliance.
if apt-cache show libprotobuf-lite32t64 >/dev/null 2>&1; then
  protobuf_lite_package=libprotobuf-lite32t64
else
  protobuf_lite_package=libprotobuf-lite32
fi

apt-get update
apt-get install -y --no-install-recommends \
  dbus \
  libavahi-client3 \
  libavahi-common3 \
  libdbus-1-3 \
  "$protobuf_lite_package" \
  libreadline8 \
  ca-certificates

install -d -m 0755 /opt/extrittio/releases
install -d -m 0700 /var/lib/extrittio
install -d -m 0755 /etc/extrittio

echo "Raspberry Pi prerequisites installed. Install the service template and configure /etc/extrittio/edge.env next."
