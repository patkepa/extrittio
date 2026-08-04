if(CMAKE_SCRIPT_MODE_FILE)
    idf_component_register(REQUIRES lwip esp_timer esp_driver_uart)
    return()
endif()

include(FetchContent)

FetchContent_Declare(
    zenohpico_src
    GIT_REPOSITORY https://github.com/eclipse-zenoh/zenoh-pico.git
    GIT_TAG        f9c59bb19c2fb4204e1f6348fb74a45882dd17af # 1.2.1
)
FetchContent_Populate(zenohpico_src)

set(ZENOHPICO_DIR "${zenohpico_src_SOURCE_DIR}")

file(READ "${ZENOHPICO_DIR}/version.txt" ZENOHPICO_VERSION)
string(STRIP "${ZENOHPICO_VERSION}" ZENOHPICO_VERSION)
string(REPLACE "." ";" ZENOHPICO_VERSION_PARTS "${ZENOHPICO_VERSION}")
list(GET ZENOHPICO_VERSION_PARTS 0 ZENOHPICO_VERSION_MAJOR)
list(GET ZENOHPICO_VERSION_PARTS 1 ZENOHPICO_VERSION_MINOR)
list(GET ZENOHPICO_VERSION_PARTS 2 ZENOHPICO_VERSION_PATCH)
set(ZENOHPICO_VERSION_TWEAK 0)

set(ZENOH_PICO "${ZENOHPICO_VERSION}")
set(ZENOH_PICO_MAJOR "${ZENOHPICO_VERSION_MAJOR}")
set(ZENOH_PICO_MINOR "${ZENOHPICO_VERSION_MINOR}")
set(ZENOH_PICO_PATCH "${ZENOHPICO_VERSION_PATCH}")
set(ZENOH_PICO_TWEAK "${ZENOHPICO_VERSION_TWEAK}")
configure_file(
    "${ZENOHPICO_DIR}/include/zenoh-pico.h.in"
    "${ZENOHPICO_DIR}/include/zenoh-pico.h"
    @ONLY
)

file(GLOB_RECURSE ZENOHPICO_SOURCES CONFIGURE_DEPENDS
    "${ZENOHPICO_DIR}/src/api/*.c"
    "${ZENOHPICO_DIR}/src/collections/*.c"
    "${ZENOHPICO_DIR}/src/link/*.c"
    "${ZENOHPICO_DIR}/src/net/*.c"
    "${ZENOHPICO_DIR}/src/protocol/*.c"
    "${ZENOHPICO_DIR}/src/session/*.c"
    "${ZENOHPICO_DIR}/src/transport/*.c"
    "${ZENOHPICO_DIR}/src/utils/*.c"
    "${ZENOHPICO_DIR}/src/system/common/*.c"
    "${ZENOHPICO_DIR}/src/system/espidf/*.c"
)

idf_component_register(
    SRCS ${ZENOHPICO_SOURCES}
    INCLUDE_DIRS "${ZENOHPICO_DIR}/include"
    REQUIRES lwip esp_timer esp_driver_uart
)

target_compile_definitions(${COMPONENT_LIB} PUBLIC ZENOH_ESPIDF=1)
