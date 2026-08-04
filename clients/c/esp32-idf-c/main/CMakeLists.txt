idf_component_register(
    SRCS "main.c" "wifi.c" "ota_handler.c"
    INCLUDE_DIRS "."
    REQUIRES
        sdk-c
        nvs_flash
        esp_wifi
        esp_http_client
        esp_https_ota
        app_update
        mbedtls
)
