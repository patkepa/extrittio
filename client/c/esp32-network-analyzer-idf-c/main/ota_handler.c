#include "ota_handler.h"
#include "extrittio/shadow.h"
#include "extrittio/time_utils.h"

#include "esp_http_client.h"
#include "esp_log.h"
#include "esp_ota_ops.h"
#include "mbedtls/sha256.h"
#include "nvs.h"
#include <stdio.h>
#include <string.h>

static const char *TAG = "ota";
static const char *NVS_NAMESPACE = "extrittio";

static void report_ota_status(z_loaned_session_t *session,
                              const char *device_id,
                              const char *status,
                              const char *fw_version,
                              int64_t fw_update_id,
                              const char *error) {
    char ota_json[512];
    int n = extrittio_ota_build_status_json(ota_json, sizeof(ota_json),
                                            status, fw_version,
                                            fw_update_id, error);
    if (n < 0) return;

    char state_json[1024];
    snprintf(state_json, sizeof(state_json), "{\"ota\":%s}", ota_json);

    extrittio_shadow_report_publish(session, device_id,
                                    extrittio_now_millis(), state_json, 0);
}

static void store_firmware_version(const char *fw_version) {
    nvs_handle_t handle;
    esp_err_t err = nvs_open(NVS_NAMESPACE, NVS_READWRITE, &handle);
    if (err != ESP_OK) {
        ESP_LOGW(TAG, "Failed to open NVS for firmware version update: %s",
                 esp_err_to_name(err));
        return;
    }

    err = nvs_set_str(handle, "fw_version", fw_version);
    if (err == ESP_OK) {
        err = nvs_commit(handle);
    }
    nvs_close(handle);

    if (err != ESP_OK) {
        ESP_LOGW(TAG, "Failed to store firmware version in NVS: %s",
                 esp_err_to_name(err));
    }
}

void ota_handle(z_loaned_session_t *session,
                const char *device_id,
                const char *current_fw,
                const extrittio_ota_payload_t *payload) {
    if (strcmp(payload->firmware_version, current_fw) == 0) {
        ESP_LOGI(TAG, "Already running %s, skipping OTA", current_fw);
        return;
    }

    ESP_LOGI(TAG, "Starting OTA: %s -> %s", current_fw, payload->firmware_version);

    report_ota_status(session, device_id, EXTRITTIO_OTA_DOWNLOADING,
                      payload->firmware_version, payload->firmware_update_id, NULL);

    const esp_partition_t *update_partition = esp_ota_get_next_update_partition(NULL);
    if (!update_partition) {
        report_ota_status(session, device_id, EXTRITTIO_OTA_FAILED,
                          payload->firmware_version, payload->firmware_update_id,
                          "No OTA partition");
        return;
    }

    esp_ota_handle_t ota_handle_val = 0;
    esp_err_t err = esp_ota_begin(update_partition, OTA_WITH_SEQUENTIAL_WRITES,
                                  &ota_handle_val);
    if (err != ESP_OK) {
        report_ota_status(session, device_id, EXTRITTIO_OTA_FAILED,
                          payload->firmware_version, payload->firmware_update_id,
                          "esp_ota_begin failed");
        return;
    }

    esp_http_client_config_t http_config = {
        .url = payload->firmware_url,
        .timeout_ms = 30000,
    };
    esp_http_client_handle_t client = esp_http_client_init(&http_config);
    err = esp_http_client_open(client, 0);
    if (err != ESP_OK) {
        esp_http_client_cleanup(client);
        esp_ota_abort(ota_handle_val);
        report_ota_status(session, device_id, EXTRITTIO_OTA_FAILED,
                          payload->firmware_version, payload->firmware_update_id,
                          "HTTP connect failed");
        return;
    }
    esp_http_client_fetch_headers(client);

    mbedtls_sha256_context sha_ctx;
    mbedtls_sha256_init(&sha_ctx);
    mbedtls_sha256_starts(&sha_ctx, 0);

    char buf[4096];
    int total = 0;
    int read_len;
    while ((read_len = esp_http_client_read(client, buf, sizeof(buf))) > 0) {
        err = esp_ota_write(ota_handle_val, buf, read_len);
        if (err != ESP_OK) {
            esp_http_client_cleanup(client);
            esp_ota_abort(ota_handle_val);
            mbedtls_sha256_free(&sha_ctx);
            report_ota_status(session, device_id, EXTRITTIO_OTA_FAILED,
                              payload->firmware_version, payload->firmware_update_id,
                              "OTA write failed");
            return;
        }
        mbedtls_sha256_update(&sha_ctx, (const unsigned char *)buf, read_len);
        total += read_len;
    }
    esp_http_client_cleanup(client);

    ESP_LOGI(TAG, "Downloaded %d bytes", total);

    report_ota_status(session, device_id, EXTRITTIO_OTA_VERIFYING,
                      payload->firmware_version, payload->firmware_update_id, NULL);

    if (payload->sha256[0] != '\0') {
        unsigned char hash[32];
        mbedtls_sha256_finish(&sha_ctx, hash);
        char hex[65] = {0};
        for (int i = 0; i < 32; i++) {
            sprintf(hex + i * 2, "%02x", hash[i]);
        }

        if (strcasecmp(hex, payload->sha256) != 0) {
            esp_ota_abort(ota_handle_val);
            mbedtls_sha256_free(&sha_ctx);
            report_ota_status(session, device_id, EXTRITTIO_OTA_FAILED,
                              payload->firmware_version, payload->firmware_update_id,
                              "SHA-256 mismatch");
            return;
        }
        ESP_LOGI(TAG, "SHA-256 verified");
    }
    mbedtls_sha256_free(&sha_ctx);

    report_ota_status(session, device_id, EXTRITTIO_OTA_INSTALLING,
                      payload->firmware_version, payload->firmware_update_id, NULL);

    err = esp_ota_end(ota_handle_val);
    if (err != ESP_OK) {
        report_ota_status(session, device_id, EXTRITTIO_OTA_FAILED,
                          payload->firmware_version, payload->firmware_update_id,
                          "esp_ota_end failed");
        return;
    }

    err = esp_ota_set_boot_partition(update_partition);
    if (err != ESP_OK) {
        report_ota_status(session, device_id, EXTRITTIO_OTA_FAILED,
                          payload->firmware_version, payload->firmware_update_id,
                          "set boot partition failed");
        return;
    }

    report_ota_status(session, device_id, EXTRITTIO_OTA_SUCCESS,
                      payload->firmware_version, payload->firmware_update_id, NULL);
    store_firmware_version(payload->firmware_version);

    ESP_LOGI(TAG, "OTA complete, restarting...");
    esp_restart();
}
