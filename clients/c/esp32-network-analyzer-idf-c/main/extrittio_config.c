#include "extrittio_config.h"

#include <string.h>

#include "esp_log.h"
#include "nvs.h"
#include "sdkconfig.h"

static const char *TAG = "extrittio_config";
static const char *NVS_NAMESPACE = "extrittio";

static void copy_default(char *dst, size_t dst_len, const char *value) {
    if (dst_len == 0) {
        return;
    }
    strlcpy(dst, value, dst_len);
}

static void read_nvs_string(nvs_handle_t handle, const char *key, char *dst,
                            size_t dst_len) {
    size_t required = 0;
    esp_err_t err = nvs_get_str(handle, key, NULL, &required);
    if (err == ESP_ERR_NVS_NOT_FOUND) {
        return;
    }
    if (err != ESP_OK) {
        ESP_LOGW(TAG, "Failed to read NVS key %s size: %s", key,
                 esp_err_to_name(err));
        return;
    }
    if (required > dst_len) {
        ESP_LOGW(TAG, "Ignoring NVS key %s: value length %u exceeds buffer %u",
                 key, (unsigned)required, (unsigned)dst_len);
        return;
    }

    err = nvs_get_str(handle, key, dst, &required);
    if (err != ESP_OK) {
        ESP_LOGW(TAG, "Failed to read NVS key %s: %s", key,
                 esp_err_to_name(err));
    }
}

esp_err_t extrittio_runtime_config_load(extrittio_runtime_config_t *config) {
    if (config == NULL) {
        return ESP_ERR_INVALID_ARG;
    }

    copy_default(config->device_id, sizeof(config->device_id),
                 CONFIG_EXTRITTIO_DEVICE_ID);
    copy_default(config->wifi_ssid, sizeof(config->wifi_ssid),
                 CONFIG_EXTRITTIO_WIFI_SSID);
    copy_default(config->wifi_password, sizeof(config->wifi_password),
                 CONFIG_EXTRITTIO_WIFI_PASSWORD);
    copy_default(config->zenoh_connect, sizeof(config->zenoh_connect),
                 CONFIG_EXTRITTIO_ZENOH_CONNECT);
    copy_default(config->firmware_version, sizeof(config->firmware_version),
                 CONFIG_EXTRITTIO_FIRMWARE_VERSION);

    nvs_handle_t handle;
    esp_err_t err = nvs_open(NVS_NAMESPACE, NVS_READONLY, &handle);
    if (err == ESP_ERR_NVS_NOT_FOUND) {
        ESP_LOGI(TAG, "No Extrittio NVS config found; using firmware defaults");
        return ESP_OK;
    }
    if (err != ESP_OK) {
        ESP_LOGW(TAG, "Failed to open Extrittio NVS config: %s",
                 esp_err_to_name(err));
        return ESP_OK;
    }

    read_nvs_string(handle, "device_id", config->device_id,
                    sizeof(config->device_id));
    read_nvs_string(handle, "wifi_ssid", config->wifi_ssid,
                    sizeof(config->wifi_ssid));
    read_nvs_string(handle, "wifi_pass", config->wifi_password,
                    sizeof(config->wifi_password));
    read_nvs_string(handle, "zenoh", config->zenoh_connect,
                    sizeof(config->zenoh_connect));
    read_nvs_string(handle, "fw_version", config->firmware_version,
                    sizeof(config->firmware_version));

    nvs_close(handle);
    ESP_LOGI(TAG, "Loaded Extrittio config for device %s", config->device_id);
    return ESP_OK;
}
