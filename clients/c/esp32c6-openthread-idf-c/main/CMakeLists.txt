idf_component_register(
    SRCS "main.c"
    INCLUDE_DIRS "."
    REQUIRES
        sdk-c
        nvs_flash
        esp_event
        esp_netif
        vfs
        openthread
)
