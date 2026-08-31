# Copyright (c) 2026 Extrittio contributors
# SPDX-License-Identifier: Apache-2.0

cmake_minimum_required(VERSION 3.20.0)

list(APPEND ZEPHYR_EXTRA_MODULES
  "${CMAKE_CURRENT_LIST_DIR}/../c/sdk-c"
)
find_package(Zephyr REQUIRED HINTS $ENV{ZEPHYR_BASE})
project(extrittio_zephyr)

# zenoh-pico 1.9.0 contains an mbedTLS transport but its upstream Zephyr glue
# does not expose it. Compile that existing port into the application and make
# the feature visible consistently to both the library and application sources.
zephyr_compile_definitions(ZENOH_GENERIC)
zephyr_include_directories(include)
target_include_directories(zephyr_interface BEFORE INTERFACE
  ${CMAKE_CURRENT_LIST_DIR}/include
)

target_sources(app PRIVATE
  src/main.c
  src/identity.c
  src/provisioning_store.c
  src/ble_provisioning.c
  src/network.c
  src/client.c
  src/ota.c
  src/zenoh_tls.c
  ${ZEPHYR_ZENOH_PICO_MODULE_DIR}/src/runtime/background_executor.c
  ${ZEPHYR_ZENOH_PICO_MODULE_DIR}/src/runtime/executor.c
)

target_include_directories(app PRIVATE
  include
  ../c/sdk-c/include
)
