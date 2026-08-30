idf_component_register(
    SRCS
        "main.c"
        "ble_contact.c"
        "programmable_led.c"
    INCLUDE_DIRS "."
    REQUIRES
        sdk-c
        nvs_flash
        esp_event
        esp_netif
        esp_driver_spi
        bt
        vfs
        openthread
)
