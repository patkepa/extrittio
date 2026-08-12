.PHONY: help hobby install-extrittio install-extrittio-fast otbr-agent check-otbr-source
.DEFAULT_GOAL := hobby

help:
	@printf '%s\n' \
		'Extrittio build targets:' \
		'  make help                    Show this list of supported commands.' \
		'  make hobby                   Build and install the complete hobby appliance (default).' \
		'  make install-extrittio       Package the frontend, build OTBR, and install Extrittio.' \
		'  make install-extrittio-fast  Quickly rebuild and install the existing hobby package.' \
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
OTBR_INSTALL_DIR := $(EXTRITTIO_INSTALL_ROOT)/libexec/extrittio

OTBR_CMAKE_OPTIONS := \
	-DCMAKE_BUILD_TYPE=Release \
	-DOTBR_DBUS=ON \
	-DOTBR_WEB=OFF \
	-DOTBR_REST=OFF \
	-DOTBR_NAT64=OFF \
	-DOTBR_DNSSD_PLAT=OFF \
	-DOTBR_TREL=OFF \
	-DBUILD_TESTING=OFF

# OpenThread's Backbone Router multicast routing and firewall integrations are
# Linux-specific. Basic Thread Border Router and commissioning functionality
# remain enabled on macOS.
ifeq ($(shell uname -s),Darwin)
OTBR_CMAKE_OPTIONS += \
	-DOTBR_BACKBONE_ROUTER=OFF \
	-DOT_FIREWALL=OFF \
	-DCMAKE_C_FLAGS=-Wno-error=uninitialized-const-pointer
endif

# Build and install the complete single-node hobby appliance, including the
# OpenThread Border Router agent used automatically by `extrittio run`.
hobby: install-extrittio

install-extrittio: otbr-agent
	npm --prefix apps/frontend ci
	npm --prefix apps/frontend run build
	cargo install --root "$(EXTRITTIO_INSTALL_ROOT)" --path apps/extrittio --locked --no-default-features --features hobby
	install -d "$(OTBR_INSTALL_DIR)"
	install -m 0755 "$(OTBR_AGENT)" "$(OTBR_INSTALL_DIR)/otbr-agent"
	install -m 0755 "$(OTBR_CTL)" "$(OTBR_INSTALL_DIR)/ot-ctl"

# Fast local iteration: reuses the workspace Cargo target cache and keeps the
# previously packaged frontend and OpenThread tools. Run `make hobby` after
# frontend, frontend-dependency, or OpenThread changes.
install-extrittio-fast:
	cargo build --profile ci-release -p extrittio --no-default-features --features hobby
	install -d "$(EXTRITTIO_INSTALL_ROOT)/bin"
	install -m 0755 target/ci-release/extrittio "$(EXTRITTIO_INSTALL_ROOT)/bin/extrittio"

otbr-agent: $(OTBR_AGENT) $(OTBR_CTL)

$(OTBR_AGENT): check-otbr-source
	cmake -S "$(OTBR_SOURCE_DIR)" -B "$(OTBR_BUILD_DIR)" $(OTBR_CMAKE_OPTIONS)
	cmake --build "$(OTBR_BUILD_DIR)" --target otbr-agent ot-ctl --parallel
	test -x "$(OTBR_AGENT)"
	test -x "$(OTBR_CTL)"

$(OTBR_CTL): $(OTBR_AGENT)

check-otbr-source: $(OTBR_SOURCE_DIR)/.git
	test "$$(git -C "$(OTBR_SOURCE_DIR)" rev-parse HEAD)" = "$(OTBR_COMMIT)"
	git -C "$(OTBR_SOURCE_DIR)" submodule update --init --recursive --depth 1

$(OTBR_SOURCE_DIR)/.git:
	mkdir -p "$(dir $(OTBR_SOURCE_DIR))"
	git clone --depth 1 --branch "$(OTBR_VERSION)" --recurse-submodules "$(OTBR_REPOSITORY)" "$(OTBR_SOURCE_DIR)"
