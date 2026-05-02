idf_component_register(
    SRCS "main.c" "wifi.c"
    INCLUDE_DIRS "."
    REQUIRES
        sdk-c
        nvs_flash
        esp_wifi
        esp_netif
        esp_timer
        lwip
)
