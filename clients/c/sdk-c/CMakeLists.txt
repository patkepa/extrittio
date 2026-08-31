cmake_minimum_required(VERSION 3.16)

set(
    EXTRITTIO_PROTO_FILE
    "${CMAKE_CURRENT_SOURCE_DIR}/../../../crates/common/src/protos/telemetry.proto"
    CACHE FILEPATH
    "Canonical Extrittio protocol definition"
)
set(EXTRITTIO_NANOPB_OPTIONS "${CMAKE_CURRENT_SOURCE_DIR}/nanopb/telemetry.options")

# When used standalone (not as an ESP-IDF component or Zephyr module)
if(NOT ESP_PLATFORM AND NOT DEFINED ZEPHYR_BASE)
    project(extrittio-sdk-c C)
    set(CMAKE_C_STANDARD 11)
    set(CMAKE_C_STANDARD_REQUIRED ON)
    # CMake 4 removed implicit compatibility below 3.5; some pinned upstream
    # dependencies still declare an older policy floor.
    set(CMAKE_POLICY_VERSION_MINIMUM 3.5)

    # --- Dependencies via FetchContent ---
    include(FetchContent)

    # nanopb runtime
    FetchContent_Declare(
        nanopb
        GIT_REPOSITORY https://github.com/nanopb/nanopb.git
        GIT_TAG        cad3c18ef15a663e30e3e43e3a752b66378adec1 # 0.4.9.1
    )
    FetchContent_MakeAvailable(nanopb)

    # cJSON
    FetchContent_Declare(
        cjson
        GIT_REPOSITORY https://github.com/DaveGamble/cJSON.git
        GIT_TAG        acc76239bee01d8e9c858ae2cab296704e52d916 # v1.7.18
    )
    set(CJSON_BUILD_SHARED_LIBS OFF CACHE BOOL "" FORCE)
    set(ENABLE_CJSON_TEST OFF CACHE BOOL "" FORCE)
    FetchContent_MakeAvailable(cjson)

    # zenoh-pico
    FetchContent_Declare(
        zenohpico
        GIT_REPOSITORY https://github.com/eclipse-zenoh/zenoh-pico.git
        GIT_TAG        e83d39b4415f6897e8d6c3b3b651ca88a05f4a17 # 1.9.0
    )
    FetchContent_MakeAvailable(zenohpico)
endif()

# --- SDK Library ---
set(SDK_SOURCES
    src/sha256.c
    src/provisioning.c
    src/bootstrap.c
    src/contract.c
    src/topics.c
    src/time_utils.c
    src/sensor.c
    src/telemetry.c
    src/heartbeat.c
    src/shadow.c
    src/logs.c
    src/commands.c
    src/ota.c
    generated/telemetry.pb.c
)

if(DEFINED ZEPHYR_BASE)
    if(NOT CONFIG_EXTRITTIO_SDK_C)
        return()
    endif()

    if(NOT ZEPHYR_NANOPB_MODULE_DIR)
        message(FATAL_ERROR "Extrittio SDK requires the nanopb Zephyr module")
    endif()

    set(
        EXTRITTIO_CJSON_ROOT
        "${CMAKE_CURRENT_LIST_DIR}/../../../../modules/lib/cjson"
        CACHE PATH
        "Path to the pinned cJSON source in the west workspace"
    )
    if(NOT EXISTS "${EXTRITTIO_CJSON_ROOT}/cJSON.c")
        message(FATAL_ERROR
            "cJSON was not found at ${EXTRITTIO_CJSON_ROOT}; run west update from the Zephyr workspace"
        )
    endif()

    zephyr_library_named(extrittio_sdk_c)
    zephyr_library_sources(${SDK_SOURCES} "${EXTRITTIO_CJSON_ROOT}/cJSON.c")
    zephyr_library_include_directories(
        include
        generated
        "${EXTRITTIO_CJSON_ROOT}"
        "${ZEPHYR_NANOPB_MODULE_DIR}"
    )
    zephyr_library_compile_options(-Wall -Wextra -Werror=implicit-function-declaration)
elseif(ESP_PLATFORM)
    # ESP-IDF component mode
    idf_component_register(
        SRCS ${SDK_SOURCES}
        INCLUDE_DIRS include generated
        REQUIRES zenohpico
        PRIV_REQUIRES nanopb cjson
    )
else()
    add_library(extrittio_sdk_c ${SDK_SOURCES})
    target_include_directories(extrittio_sdk_c
        PUBLIC include
        PRIVATE generated ${nanopb_SOURCE_DIR} ${cjson_SOURCE_DIR}
    )
    target_link_libraries(extrittio_sdk_c
        PUBLIC zenohpico::lib
        PRIVATE protobuf-nanopb-static cjson
    )

    # --- Tests ---
    enable_testing()

    function(sdk_add_test TEST_NAME TEST_SOURCE)
        add_executable(${TEST_NAME} ${TEST_SOURCE})
        target_link_libraries(${TEST_NAME} PRIVATE extrittio_sdk_c protobuf-nanopb-static cjson)
        target_include_directories(${TEST_NAME} PRIVATE generated ${nanopb_SOURCE_DIR} ${cjson_SOURCE_DIR})
        add_test(NAME ${TEST_NAME} COMMAND ${TEST_NAME})
    endfunction()

    sdk_add_test(test_topics tests/test_topics.c)
    sdk_add_test(test_sha256 tests/test_sha256.c)
    sdk_add_test(test_provisioning tests/test_provisioning.c)
    sdk_add_test(test_bootstrap tests/test_bootstrap.c)
    sdk_add_test(test_contract tests/test_contract.c)
    sdk_add_test(test_sensor tests/test_sensor.c)
    sdk_add_test(test_ota tests/test_ota.c)
    sdk_add_test(test_telemetry tests/test_telemetry.c)
    sdk_add_test(test_heartbeat tests/test_heartbeat.c)
    sdk_add_test(test_shadow tests/test_shadow.c)
    sdk_add_test(test_logs tests/test_logs.c)
    sdk_add_test(test_commands tests/test_commands.c)

    # --- Optional: regenerate nanopb code ---
    find_program(NANOPB_GENERATOR nanopb_generator)
    if(NANOPB_GENERATOR)
        add_custom_target(regenerate_proto
            COMMAND ${NANOPB_GENERATOR}
                ${EXTRITTIO_PROTO_FILE}
                --options-file=${EXTRITTIO_NANOPB_OPTIONS}
                --output-dir=${CMAKE_CURRENT_SOURCE_DIR}/generated/
            DEPENDS ${EXTRITTIO_PROTO_FILE} ${EXTRITTIO_NANOPB_OPTIONS}
            COMMENT "Regenerating nanopb code from the canonical telemetry.proto"
        )
    endif()
endif()
