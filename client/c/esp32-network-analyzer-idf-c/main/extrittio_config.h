#pragma once

#include "esp_err.h"

typedef struct {
    char device_id[64];
    char wifi_ssid[33];
    char wifi_password[65];
    char zenoh_connect[128];
    char firmware_version[64];
} extrittio_runtime_config_t;

esp_err_t extrittio_runtime_config_load(extrittio_runtime_config_t *config);
