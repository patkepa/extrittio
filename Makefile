.PHONY: help install-release install-debug install-hobby-release install-hobby-debug hobby-assets otbr-agent check-otbr-source
.DEFAULT_GOAL := help

help:
	@printf '%s\n' \
		'Extrittio build targets:' \
		'  make help                    Show this list of supported commands.' \
		'  make install-release         Install the PostgreSQL server with release optimizations.' \
		'  make install-debug           Install the PostgreSQL server with debug settings.' \
		'  make install-hobby-release   Install the complete hobby appliance with release optimizations.' \
		'  make install-hobby-debug     Install the complete hobby appliance with debug settings.' \
		'  make otbr-agent              Build the OpenThread Border Router agent and ot-ctl.' \
		'  make check-otbr-source       Verify and initialize the pinned OpenThread source.' \
		'' \
		'Configuration:' \
		'  EXTRITTIO_INSTALL_ROOT        Installation prefix (default: ~/.cargo).'

# The hobby install places both executables under this root:
#   bin/extrittio
#   libexec/extrittio/otbr-agent
# The hobby runtime resolves the latter relative to its own executable.
EXTRITTIO_INSTALL_ROOT ?= $(HOME)/.cargo

OTBR_VERSION := v2026.08.0
OTBR_COMMIT := 337711e7038d0b9c8fb46a1ce888ce7f9c4c0c35
OTBR_REPOSITORY := https://github.com/openthread/ot-br-posix.git
OTBR_SOURCE_DIR := target/openthread/ot-br-posix
OTBR_BUILD_DIR := target/openthread/build
OTBR_AGENT := $(OTBR_BUILD_DIR)/src/agent/otbr-agent
# `ot-ctl` is built by OpenThread itself, which OTBR embeds as a subproject.
OTBR_CTL := $(OTBR_BUILD_DIR)/third_party/openthread/repo/src/posix/ot-ctl
OTBR_MACOS_IPV6_PATCH := patches/otbr-macos-ipv6-bound-if.patch
OTBR_MACOS_IPV6_PATCH_STAMP := $(OTBR_SOURCE_DIR)/.extrittio-macos-ipv6-bound-if-patched
OTBR_MACOS_DNSSD_PATCH := patches/otbr-macos-dnssd-link.patch
OTBR_MACOS_DNSSD_PATCH_STAMP := $(OTBR_SOURCE_DIR)/.extrittio-macos-dnssd-link-patched
OTBR_INSTALL_DIR := $(EXTRITTIO_INSTALL_ROOT)/libexec/extrittio
EXTRITTIO_BIN_DIR := $(EXTRITTIO_INSTALL_ROOT)/bin

# npm ci creates this lockfile inside node_modules. Using it as the Make target
# reruns installation only after the frontend dependency inputs change (or when
# node_modules has been removed).
FRONTEND_NODE_MODULES := apps/frontend/node_modules/.package-lock.json
# Keep the build marker inside dist so deleting the distribution directory also
# invalidates the cache.
FRONTEND_BUILD_STAMP := apps/frontend/dist/.extrittio-build-stamp
FRONTEND_BUILD_INPUTS := \
	$(shell find apps/frontend/src apps/frontend/public ui/packages -type f 2>/dev/null) \
	apps/frontend/index.html \
	apps/frontend/vite.config.ts \
	apps/frontend/tsconfig.json \
	apps/frontend/tsconfig.package.json \
	ui/package.json \
	ui/package-lock.json \
	ui/tsconfig.json \
	ui/tsconfig.package.json

OTBR_CMAKE_OPTIONS := \
	-DCMAKE_BUILD_TYPE=Release \
	-DCMAKE_POLICY_VERSION_MINIMUM=3.5 \
	-DOTBR_DBUS=ON \
	-DOTBR_WEB=OFF \
	-DOTBR_REST=ON \
	-DOT_CHANNEL_MONITOR=ON \
	-DOT_CHANNEL_MONITOR_AUTO_START=ON \
	-DOTBR_NAT64=OFF \
	-DOTBR_OT_SRP_ADV_PROXY=ON \
	-DOTBR_OT_DISCOVERY_PROXY=ON \
	-DOTBR_TREL=OFF \
	-DBUILD_TESTING=OFF

# OpenThread's Backbone Router multicast routing and firewall integrations are
# Linux-specific. Basic Thread Border Router and commissioning functionality
# remain enabled on macOS. Use macOS's mDNSResponder so OTBR's Discovery Proxy
# can resolve the backend's standard host mDNS registration for Thread DNS-SD
# clients. Clear the cached OpenThread mDNS setting when upgrading an existing
# internal-mDNS build; OT permits platform DNS-SD or its own mDNS, not both.
ifeq ($(shell uname -s),Darwin)
OTBR_CMAKE_OPTIONS += \
	-DOTBR_MDNS=mDNSResponder \
	-DOTBR_DNSSD_PLAT=ON \
	-DOT_MDNS=OFF \
	-DOT_MDNS_VERBOSE=OFF \
	-DOTBR_BACKBONE_ROUTER=OFF \
	-DOT_FIREWALL=OFF \
	-DCMAKE_C_FLAGS=-Wno-error=uninitialized-const-pointer
endif

# Install the standard PostgreSQL-backed server. The frontend is not embedded
# in this build; serve it from apps/frontend/dist or EXTRITTIO_UI_DIR.
install-release:
	cargo build --release --locked -p extrittio
	install -d "$(EXTRITTIO_BIN_DIR)"
	install -m 0755 target/release/extrittio "$(EXTRITTIO_BIN_DIR)/extrittio"

install-debug:
	cargo build --locked -p extrittio
	install -d "$(EXTRITTIO_BIN_DIR)"
	install -m 0755 target/debug/extrittio "$(EXTRITTIO_BIN_DIR)/extrittio"

# Build assets shared by the standalone hobby appliance. OTBR intentionally
# stays a release CMake build for a stable host runtime in both Rust modes.
$(FRONTEND_NODE_MODULES): apps/frontend/package.json apps/frontend/package-lock.json apps/frontend/.npmrc
	npm --prefix apps/frontend ci
	test -f "$(FRONTEND_NODE_MODULES)"

$(FRONTEND_BUILD_STAMP): $(FRONTEND_NODE_MODULES) $(FRONTEND_BUILD_INPUTS)
	npm --prefix apps/frontend run build
	touch "$(FRONTEND_BUILD_STAMP)"

hobby-assets: $(FRONTEND_BUILD_STAMP)

# Install the complete single-node hobby appliance, including the OpenThread
# Border Router agent used automatically by `extrittio run`.
install-hobby-release: hobby-assets otbr-agent
	cargo build --release --locked -p extrittio --no-default-features --features hobby
	install -d "$(EXTRITTIO_BIN_DIR)"
	install -m 0755 target/release/extrittio "$(EXTRITTIO_BIN_DIR)/extrittio"
	install -d "$(OTBR_INSTALL_DIR)"
	install -m 0755 "$(OTBR_AGENT)" "$(OTBR_INSTALL_DIR)/otbr-agent"
	install -m 0755 "$(OTBR_CTL)" "$(OTBR_INSTALL_DIR)/ot-ctl"

install-hobby-debug: hobby-assets otbr-agent
	cargo build --locked -p extrittio --no-default-features --features hobby
	install -d "$(EXTRITTIO_BIN_DIR)"
	install -m 0755 target/debug/extrittio "$(EXTRITTIO_BIN_DIR)/extrittio"
	install -d "$(OTBR_INSTALL_DIR)"
	install -m 0755 "$(OTBR_AGENT)" "$(OTBR_INSTALL_DIR)/otbr-agent"
	install -m 0755 "$(OTBR_CTL)" "$(OTBR_INSTALL_DIR)/ot-ctl"

otbr-agent: $(OTBR_AGENT) $(OTBR_CTL)

# The source verification must run before a build, but it must not mark an
# already-built OTBR agent stale on every hobby install.
$(OTBR_AGENT): Makefile $(OTBR_MACOS_IPV6_PATCH_STAMP) $(OTBR_MACOS_DNSSD_PATCH_STAMP) | check-otbr-source
	cmake -S "$(OTBR_SOURCE_DIR)" -B "$(OTBR_BUILD_DIR)" $(OTBR_CMAKE_OPTIONS)
	cmake --build "$(OTBR_BUILD_DIR)" --target otbr-agent ot-ctl --parallel
	test -x "$(OTBR_AGENT)"
	test -x "$(OTBR_CTL)"

$(OTBR_CTL): $(OTBR_AGENT)

$(OTBR_MACOS_IPV6_PATCH_STAMP): $(OTBR_SOURCE_DIR)/.git $(OTBR_MACOS_IPV6_PATCH)
ifeq ($(shell uname -s),Darwin)
	if git -C "$(OTBR_SOURCE_DIR)" apply --check "$(abspath $(OTBR_MACOS_IPV6_PATCH))" 2>/dev/null; then \
		git -C "$(OTBR_SOURCE_DIR)" apply "$(abspath $(OTBR_MACOS_IPV6_PATCH))"; \
	else \
		git -C "$(OTBR_SOURCE_DIR)" apply --reverse --check "$(abspath $(OTBR_MACOS_IPV6_PATCH))"; \
	fi
endif
	touch "$@"

$(OTBR_MACOS_DNSSD_PATCH_STAMP): $(OTBR_SOURCE_DIR)/.git $(OTBR_MACOS_DNSSD_PATCH)
ifeq ($(shell uname -s),Darwin)
	if git -C "$(OTBR_SOURCE_DIR)" apply --check "$(abspath $(OTBR_MACOS_DNSSD_PATCH))" 2>/dev/null; then \
		git -C "$(OTBR_SOURCE_DIR)" apply "$(abspath $(OTBR_MACOS_DNSSD_PATCH))"; \
	else \
		git -C "$(OTBR_SOURCE_DIR)" apply --reverse --check "$(abspath $(OTBR_MACOS_DNSSD_PATCH))"; \
	fi
endif
	touch "$@"

check-otbr-source: $(OTBR_SOURCE_DIR)/.git
	test "$$(git -C "$(OTBR_SOURCE_DIR)" rev-parse HEAD)" = "$(OTBR_COMMIT)"
	git -C "$(OTBR_SOURCE_DIR)" submodule update --init --recursive --depth 1

$(OTBR_SOURCE_DIR)/.git:
	mkdir -p "$(dir $(OTBR_SOURCE_DIR))"
	git clone --depth 1 --branch "$(OTBR_VERSION)" --recurse-submodules "$(OTBR_REPOSITORY)" "$(OTBR_SOURCE_DIR)"
