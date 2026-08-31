# Override zenoh-pico's upstream Zephyr glue to expose its existing TLS link.
# The pinned 1.9.0 release ships the TLS link and mbedTLS implementation, but
# omits the corresponding option from zephyr/Kconfig.zenoh.
set(ZEPHYR_ZENOH_PICO_CMAKE_DIR ${CMAKE_CURRENT_LIST_DIR}/zenoh-pico)
set(ZEPHYR_ZENOH_PICO_KCONFIG ${CMAKE_CURRENT_LIST_DIR}/zenoh-pico/Kconfig)
