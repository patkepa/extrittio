#pragma once

#include <stdint.h>

#include "esp_err.h"

/**
 * Initializes the two serial SK6805 LEDs used by the CI ESP32-C6 board.
 *
 * The LEDs share a data line on GPIO 9 and are driven through SPI2 using the
 * timing and encoding from ci-firmware-one-esp32-c6.
 */
esp_err_t programmable_led_init(void);

/** Sets both LEDs to the same RGB color. */
esp_err_t programmable_led_set_all_rgb(uint8_t red, uint8_t green, uint8_t blue);
