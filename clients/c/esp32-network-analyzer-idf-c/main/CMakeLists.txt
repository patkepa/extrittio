idf_component_register(
    SRCS "main.c" "wifi.c" "extrittio_config.c" "ota_handler.c"
    INCLUDE_DIRS "."
    REQUIRES
        sdk-c
        app_update
        esp_http_client
        nvs_flash
        esp_wifi
        esp_netif
        esp_timer
        lwip
        mbedtls
)
