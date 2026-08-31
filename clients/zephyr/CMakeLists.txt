# Copyright (c) 2026 Extrittio contributors
# SPDX-License-Identifier: Apache-2.0

cmake_minimum_required(VERSION 3.20.0)

list(APPEND ZEPHYR_EXTRA_MODULES
  "${CMAKE_CURRENT_LIST_DIR}/../c/sdk-c"
)
list(APPEND MODULE_EXT_ROOT "${CMAKE_CURRENT_LIST_DIR}")
set(MODULE_EXT_ROOT "${MODULE_EXT_ROOT}" CACHE STRING
    "Module integration overrides used by the Extrittio Zephyr client" FORCE)

find_package(Zephyr REQUIRED HINTS $ENV{ZEPHYR_BASE})
project(extrittio_zephyr)

target_sources(app PRIVATE
  src/main.c
  src/identity.c
  src/provisioning_store.c
  src/ble_provisioning.c
  src/network.c
  src/client.c
  src/ota.c
)

target_include_directories(app PRIVATE include)
