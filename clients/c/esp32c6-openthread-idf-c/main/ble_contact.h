#pragma once

#include "esp_err.h"

/**
 * Starts the Extrittio BLE contact service.
 *
 * The service advertises continuously and exposes non-sensitive device
 * identity fields to a nearby central. It accepts a bounded contact-message
 * write and flashes both CI ESP32-C6 board LEDs green after accepting it. It
 * does not provision credentials or modify the device's Thread configuration.
 */
esp_err_t ble_contact_start(void);
