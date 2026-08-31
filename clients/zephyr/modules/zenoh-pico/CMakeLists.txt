# Extrittio extension of zenoh-pico 1.9.0's Zephyr build glue.

if(CONFIG_ZENOH_PICO)
  zephyr_compile_definitions(ZENOH_ZEPHYR)
  zephyr_include_directories(${ZEPHYR_ZENOH_PICO_MODULE_DIR}/include)
  zephyr_library()

  function(extrittio_configure_zenoh_feature config feature)
    if(${config})
      zephyr_compile_definitions(${feature}=1)
    else()
      zephyr_compile_definitions(${feature}=0)
    endif()
  endfunction()

  extrittio_configure_zenoh_feature(CONFIG_ZENOH_PICO_LINK_SERIAL Z_FEATURE_LINK_SERIAL)
  extrittio_configure_zenoh_feature(CONFIG_ZENOH_PICO_MULTI_THREAD Z_FEATURE_MULTI_THREAD)
  extrittio_configure_zenoh_feature(CONFIG_ZENOH_PICO_PUBLICATION Z_FEATURE_PUBLICATION)
  extrittio_configure_zenoh_feature(CONFIG_ZENOH_PICO_SUBSCRIPTION Z_FEATURE_SUBSCRIPTION)
  extrittio_configure_zenoh_feature(CONFIG_ZENOH_PICO_QUERY Z_FEATURE_QUERY)
  extrittio_configure_zenoh_feature(CONFIG_ZENOH_PICO_QUERYABLE Z_FEATURE_QUERYABLE)
  extrittio_configure_zenoh_feature(CONFIG_ZENOH_PICO_RAWETH_TRANSPORT Z_FEATURE_RAWETH_TRANSPORT)
  extrittio_configure_zenoh_feature(CONFIG_ZENOH_PICO_LINK_TCP Z_FEATURE_LINK_TCP)
  extrittio_configure_zenoh_feature(CONFIG_ZENOH_PICO_LINK_TLS Z_FEATURE_LINK_TLS)
  extrittio_configure_zenoh_feature(CONFIG_ZENOH_PICO_LINK_UDP_UNICAST Z_FEATURE_LINK_UDP_UNICAST)
  extrittio_configure_zenoh_feature(CONFIG_ZENOH_PICO_LINK_UDP_MULTICAST Z_FEATURE_LINK_UDP_MULTICAST)
  extrittio_configure_zenoh_feature(CONFIG_ZENOH_PICO_SCOUTING Z_FEATURE_SCOUTING)
  extrittio_configure_zenoh_feature(CONFIG_ZENOH_PICO_LINK_WS Z_FEATURE_LINK_WS)

  file(GLOB_RECURSE zenoh_sources
    "${ZEPHYR_ZENOH_PICO_MODULE_DIR}/src/api/*.c"
    "${ZEPHYR_ZENOH_PICO_MODULE_DIR}/src/collections/*.c"
    "${ZEPHYR_ZENOH_PICO_MODULE_DIR}/src/link/*.c"
    "${ZEPHYR_ZENOH_PICO_MODULE_DIR}/src/net/*.c"
    "${ZEPHYR_ZENOH_PICO_MODULE_DIR}/src/protocol/*.c"
    "${ZEPHYR_ZENOH_PICO_MODULE_DIR}/src/session/*.c"
    "${ZEPHYR_ZENOH_PICO_MODULE_DIR}/src/transport/*.c"
    "${ZEPHYR_ZENOH_PICO_MODULE_DIR}/src/utils/*.c"
    "${ZEPHYR_ZENOH_PICO_MODULE_DIR}/src/system/common/*.c"
    "${ZEPHYR_ZENOH_PICO_MODULE_DIR}/src/system/zephyr/*.c"
  )
  if(CONFIG_ZENOH_PICO_LINK_TLS)
    list(APPEND zenoh_sources
      "${ZEPHYR_ZENOH_PICO_MODULE_DIR}/src/system/unix/tls.c"
    )
  endif()
  zephyr_library_sources(${zenoh_sources})
endif()
